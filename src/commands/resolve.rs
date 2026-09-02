use crate::cli::ResolveArgs;
use crate::commands::add::redact_evidence;
use crate::error::{AppError, AppResult};
use crate::output::{self, Meta};
use crate::store;
use crate::{
    Disposition, ItemStatus, ListItem, LogEvent, format_timestamp, is_bl_id, resolve_agent_checked,
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
        disposition,
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
        && disposition.is_none()
    {
        return Err(AppError::invalid_input(
            "--amend requires at least one resolution field",
            "Pass --note, --task, --pr, --commit, --url, --dropped, or --disposition with --amend.",
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
        store::check_version(&bytes, &resolved.path)?;
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
        // Every disposition rule needs the folded kind of each named record, so
        // all three sit inside the critical section, after ID matching and
        // before any append. A batch that fails one appends nothing at all.
        let has_cut = items.iter().any(|item| item.kind == "cut");
        let has_dogear = items.iter().any(|item| item.kind == "dogear");
        if has_cut && has_dogear {
            return Err(AppError::invalid_argument(
                "a resolve batch cannot name both cut and dogear records",
                "Resolve cuts and dogears in separate commands; a cut requires --disposition and a dogear rejects it.",
            ));
        }
        if disposition.is_some() && has_dogear {
            return Err(AppError::invalid_argument(
                "--disposition may only resolve cut records",
                "Use --disposition only with cut IDs; a dogear's lifecycle is --url or --dropped.",
            ));
        }
        if disposition.is_none() && has_cut && !amend {
            return Err(AppError::invalid_argument(
                "--disposition is required when resolving a cut",
                "Pass --disposition fixed|promoted|accepted|invalid.",
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
                let (disposition, disposition_ts) =
                    event_disposition(item, disposition, ts.as_str());
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
                    disposition,
                    disposition_ts,
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
                    let (disposition, disposition_ts) =
                        event_disposition(item, disposition, ts.as_str());
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
                        disposition,
                        disposition_ts,
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
    }
    // The two warnings answer different questions — which IDs were already
    // resolved, and whether anything was appended — so a dry run says so
    // whatever the mix. Chained behind the already-resolved arms, a mixed dry
    // run and a mixed real run emitted an identical warning set, and only
    // `data.changed` told a consumer which one it was reading.
    if dry_run {
        meta.warnings
            .push("dry run; no resolve event appended".into());
    }
    output::write_success(ResolveData { changed, records }, pretty, meta)
        .map_err(|error| AppError::from_io(error, std::path::Path::new("stdout")))?;
    Ok(0)
}

/// The disposition an event carries, and the moment that disposition was
/// decided. An explicit `--disposition` stamps this event's own `ts`; an amend
/// that omits it copies both from the **pre-append folded winner** — what `list`
/// shows the instant before this command runs (r50) — so inheritance is visible
/// in the stored bytes and needs no fold rule to reconstruct.
fn event_disposition(
    item: &ListItem,
    requested: Option<Disposition>,
    ts: &str,
) -> (Option<Disposition>, Option<String>) {
    if item.kind != "cut" {
        return (None, None);
    }
    match requested {
        Some(disposition) => (Some(disposition), Some(ts.to_owned())),
        None => {
            let resolution = item.resolution.as_ref();
            (
                resolution.and_then(|resolution| resolution.disposition),
                resolution.and_then(|resolution| resolution.disposition_ts.clone()),
            )
        }
    }
}

/// An ID prefix: optional `bl_`, then at least 4 hex digits, matched
/// case-insensitively against the one `bl2` namespace (r48). No exact-full-ID
/// precedence: a complete ID that also prefixes a longer one is `ambiguous_id`
/// (r50).
#[derive(Debug)]
struct IdPrefix {
    hex: String,
}

fn normalize_prefix(input: &str) -> AppResult<IdPrefix> {
    let hex = if is_bl_id(input) { &input[3..] } else { input };
    if hex.len() < 4 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(AppError::invalid_argument(
            format!("invalid record ID prefix '{input}'"),
            "Use `blotter list --kind all --status all` and pass at least 4 hexadecimal digits, with an optional bl_ prefix.",
        ));
    }
    Ok(IdPrefix {
        hex: hex.to_ascii_lowercase(),
    })
}

fn match_id(prefix: &IdPrefix, items: &[ListItem]) -> AppResult<String> {
    let mut candidates: Vec<_> = items
        .iter()
        .map(|item| item.id.clone())
        .filter(|id| {
            is_bl_id(id)
                && id
                    .get(3..)
                    .is_some_and(|hex| hex.to_ascii_lowercase().starts_with(&prefix.hex))
        })
        .collect();
    candidates.sort();
    match candidates.as_slice() {
        [] => Err(AppError::not_found(
            format!("no record matches ID prefix '{}'", prefix.hex),
            "Run `blotter list --kind all --status all` and retry with a listed ID.",
        )),
        [id] => Ok(id.clone()),
        _ => Err(AppError::ambiguous_id(&prefix.hex, candidates)),
    }
}
