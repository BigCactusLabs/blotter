use crate::cli::ResolveArgs;
use crate::commands::add::redact_evidence;
use crate::error::{AppError, AppResult};
use crate::output::{self, Meta};
use crate::store;
use crate::{
    IdNamespace, ItemStatus, ListItem, LogEvent, format_timestamp, id_namespace,
    resolve_agent_checked,
};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct ResolveData {
    pub changed: bool,
    pub records: Vec<ListItem>,
}

pub fn run(
    args: ResolveArgs,
    file: Option<PathBuf>,
    pretty: bool,
    now: Timestamp,
) -> AppResult<i32> {
    let ResolveArgs {
        ids,
        note,
        agent: requested_agent,
        task,
        pr,
        commit,
        url,
        dropped,
        amend,
        dry_run,
    } = args;
    let prefixes: Vec<_> = ids
        .iter()
        .map(|id| normalize_prefix(id))
        .collect::<AppResult<_>>()?;
    let resolved = store::discover(file)?;
    for (flag, value) in [
        ("task", task.as_deref()),
        ("pr", pr.as_deref()),
        ("commit", commit.as_deref()),
        ("url", url.as_deref()),
    ] {
        if value.is_some_and(|value| value.trim().is_empty()) {
            return Err(AppError::invalid_input(
                format!("{flag} cannot be empty or whitespace-only"),
                format!("Pass a non-empty --{flag} VALUE or omit the flag."),
            ));
        }
    }
    if amend
        && note.is_none()
        && task.is_none()
        && pr.is_none()
        && commit.is_none()
        && url.is_none()
        && !dropped
    {
        return Err(AppError::invalid_input(
            "--amend requires at least one resolution field",
            "Pass --note, --task, --pr, --commit, --url, or --dropped with --amend.",
        ));
    }
    // Redact once, ahead of the critical section, so the base and amend paths —
    // and the dry-run prediction — all carry the same bytes the append stores.
    let home = store::home_dir(&resolved.cwd);
    let note = note.map(|value| redact_evidence(&value, home.as_deref()));
    let (agent, source) = resolve_agent_checked(requested_agent, false)?;
    let ts = format_timestamp(now);
    let action = |log: &mut std::fs::File| -> AppResult<(bool, Vec<String>, Vec<ListItem>)> {
        let bytes = store::read_bytes(log, &resolved.path)?;
        let folded = store::fold_bytes(&bytes);
        let mut ids = prefixes
            .iter()
            .map(|prefix| match_id(prefix, &folded.items))
            .collect::<AppResult<Vec<_>>>()?;
        ids.sort();
        ids.dedup();
        let mut items = ids
            .iter()
            .map(|id| {
                folded
                    .items
                    .iter()
                    .find(|item| item.id == *id)
                    .cloned()
                    .ok_or_else(|| AppError::internal("matched cut disappeared during resolution"))
            })
            .collect::<AppResult<Vec<_>>>()?;
        if (url.is_some() || dropped) && items.iter().any(|item| item.kind != "dogear") {
            return Err(AppError::invalid_argument(
                "--url and --dropped may only resolve dogear records",
                "Use --url or --dropped only with dogear IDs, or resolve cuts without those flags.",
            ));
        }
        if amend && items.iter().any(|item| item.status != ItemStatus::Resolved) {
            return Err(AppError::invalid_input(
                "--amend requires every requested record to be resolved",
                "Resolve each record without --amend first, then retry with --amend.",
            ));
        }
        let already_resolved_ids: Vec<_> = if amend {
            Vec::new()
        } else {
            ids.iter()
                .zip(&items)
                .filter(|(_, item)| item.status == ItemStatus::Resolved)
                .map(|(id, _)| id.clone())
                .collect()
        };
        let mut changed = false;
        if !dry_run {
            let mut events = Vec::new();
            let mut updated_item_indexes = Vec::new();
            for (item_index, (id, item)) in ids.iter().zip(&items).enumerate() {
                if !amend && item.status == ItemStatus::Resolved {
                    continue;
                }
                events.push(LogEvent::Resolve {
                    id: id.clone(),
                    ts: ts.clone(),
                    agent: agent.clone(),
                    note: note.clone(),
                    task: task.clone(),
                    pr: pr.clone(),
                    commit: commit.clone(),
                    url: url.clone(),
                    dropped,
                    amend,
                });
                updated_item_indexes.push(item_index);
            }
            if !events.is_empty() {
                store::append_json_batch(log, &resolved.path, &bytes, &events)?;
                changed = true;
                for (item_index, event) in updated_item_indexes.into_iter().zip(&events) {
                    let item = &mut items[item_index];
                    item.status = ItemStatus::Resolved;
                    item.resolution = Some(folded.materialized_appended_resolution(event));
                }
            }
        } else {
            // Predict through the same rule the apply path uses, so a dry run
            // cannot promise a resolution the real append would not produce —
            // an amend backdated behind a stored one does not win.
            for (id, item) in ids.iter().zip(&mut items) {
                if amend || item.status == ItemStatus::Open {
                    let candidate = LogEvent::Resolve {
                        id: id.clone(),
                        ts: ts.clone(),
                        agent: agent.clone(),
                        note: note.clone(),
                        task: task.clone(),
                        pr: pr.clone(),
                        commit: commit.clone(),
                        url: url.clone(),
                        dropped,
                        amend,
                    };
                    item.status = ItemStatus::Resolved;
                    item.resolution = Some(folded.materialized_appended_resolution(&candidate));
                }
            }
        }
        Ok((changed, already_resolved_ids, items))
    };
    let (changed, already_resolved_ids, records) = if dry_run {
        store::with_shared(&resolved.path, action)
    } else {
        store::with_exclusive(&resolved.path, false, action)
    }?;
    let mut meta = Meta::new();
    meta.file = Some(resolved.path.to_string_lossy().into_owned());
    meta.agent_source = Some(source.into());
    meta.warnings = resolved.warnings.clone();
    if already_resolved_ids.len() == records.len() {
        meta.warnings.push("already resolved".into());
    } else if !already_resolved_ids.is_empty() {
        let noun = if already_resolved_ids.len() == 1 {
            "ID"
        } else {
            "IDs"
        };
        meta.warnings.push(format!(
            "already resolved: {} {noun} ({})",
            already_resolved_ids.len(),
            already_resolved_ids.join(", ")
        ));
    } else if dry_run {
        meta.warnings
            .push("dry run; no resolve event appended".into());
    }
    output::write_success(ResolveData { changed, records }, pretty, meta)
        .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
    Ok(0)
}

#[derive(Debug)]
struct IdPrefix {
    namespace: Option<IdNamespace>,
    hex: String,
}

fn normalize_prefix(input: &str) -> AppResult<IdPrefix> {
    let namespace = id_namespace(input);
    let hex = namespace.map_or(input, |_| &input[3..]);
    if hex.len() < 4 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(AppError::invalid_argument(
            format!("invalid cut ID prefix '{input}'"),
            "Use `blotter list --status all --include-auto` and pass at least 4 hexadecimal digits, with optional bl_ or pc_ prefix.",
        ));
    }
    Ok(IdPrefix {
        namespace,
        hex: hex.to_ascii_lowercase(),
    })
}

fn match_id(prefix: &IdPrefix, items: &[ListItem]) -> AppResult<String> {
    let mut candidates: Vec<_> = items
        .iter()
        .map(|item| item.id.clone())
        .filter(|id| {
            id_namespace(id).is_some_and(|namespace| {
                prefix
                    .namespace
                    .is_none_or(|expected| expected == namespace)
                    && id
                        .get(3..)
                        .is_some_and(|hex| hex.to_ascii_lowercase().starts_with(&prefix.hex))
            })
        })
        .collect();
    candidates.sort();
    match candidates.as_slice() {
        [] => Err(AppError::not_found(
            format!("no cut matches ID prefix '{}'", prefix.hex),
            "Run `blotter list --status all --include-auto` and retry with a listed ID.",
        )),
        [id] => Ok(id.clone()),
        _ => Err(AppError::ambiguous_id(&prefix.hex, candidates)),
    }
}
