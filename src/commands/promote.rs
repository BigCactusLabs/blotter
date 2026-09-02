use crate::cli::PromoteArgs;
use crate::commands::add::redact_evidence;
use crate::commands::resolve::{Candidate, candidates, match_id, normalize_prefix};
use crate::error::{AppError, AppResult};
use crate::output::{self, Meta};
use crate::store;
use crate::{
    Artifact, LogEvent, Origin, compute_promotion_id, format_timestamp, normalized,
    resolve_agent_checked,
};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize)]
pub struct PromoteData {
    pub changed: bool,
    pub record: LogEvent,
}

pub fn run(
    args: PromoteArgs,
    file: Option<PathBuf>,
    pretty: bool,
    now: Timestamp,
) -> AppResult<i32> {
    let PromoteArgs {
        sources,
        artifact_type,
        artifact_ref,
        note,
        agent,
        dry_run,
    } = args;
    let prefixes = sources
        .iter()
        .map(|source| normalize_prefix(source))
        .collect::<AppResult<Vec<_>>>()?;
    let resolved = store::discover(file)?;
    let home = store::home_dir(&resolved.cwd);
    // Redacted before hashing and before append, so the hashed bytes are the
    // stored bytes and a dry run predicts them exactly (r48).
    let artifact_ref = redact_evidence(strip_trailing_newlines(&artifact_ref), home.as_deref());
    validate_free_text(&artifact_ref, "artifact ref", "--artifact-ref REF")?;
    let note = note
        .map(|note| {
            let note = redact_evidence(strip_trailing_newlines(&note), home.as_deref());
            validate_free_text(&note, "note", "--note TEXT")?;
            Ok(note)
        })
        .transpose()?;
    let (agent, agent_source) = resolve_agent_checked(agent, true)?;
    let ts = format_timestamp(now);
    let cwd = store::record_cwd(&resolved.cwd, resolved.cwd_repo(), home.as_deref());

    // Read → fold → validate → append inside one critical section, after the
    // version probe. Unlike `add --dry-run`, a dry run opens the log: validating
    // every `--source` requires the fold (r48).
    let action = |log: &mut std::fs::File| -> AppResult<(bool, LogEvent)> {
        let bytes = store::read_bytes(log, &resolved.path)?;
        store::check_version(&bytes, &resolved.path)?;
        let folded = store::fold_bytes(&bytes);
        let candidates = candidates(&folded);
        let mut source_ids = prefixes
            .iter()
            .map(|prefix| {
                let Candidate { id, kind } = match_id(prefix, &candidates)?;
                if kind != "cut" {
                    return Err(AppError::invalid_argument(
                        format!("--source {id} is a {kind}, not a cut"),
                        "Promotion sources are cuts only; pass cut IDs to --source.",
                    ));
                }
                Ok(id)
            })
            .collect::<AppResult<Vec<_>>>()?;
        source_ids = normalized(&source_ids);
        let record = LogEvent::Promotion {
            id: compute_promotion_id(
                &ts,
                &agent,
                &source_ids,
                artifact_type.as_str(),
                &artifact_ref,
            ),
            ts: ts.clone(),
            agent: agent.clone(),
            sources: source_ids,
            artifact: Artifact {
                kind: artifact_type,
                r#ref: artifact_ref.clone(),
            },
            note: note.clone(),
            origin: Some(Origin::agent()),
            cwd: cwd.clone(),
        };
        let id = record.id().expect("new promotions have IDs");
        // Duplicates follow the cut and dogear rules exactly: first-wins, the
        // existing record returned, nothing appended.
        if let Some(existing) = folded.record(id) {
            return match existing {
                LogEvent::Promotion { .. } => Ok((false, existing.clone())),
                _ => Err(AppError::internal(
                    "promotion ID collides with an existing non-promotion record",
                )),
            };
        }
        if dry_run {
            return Ok((false, record));
        }
        store::append_json(log, &resolved.path, &bytes, &record)?;
        Ok((true, record))
    };
    let (changed, record) = if dry_run {
        store::with_shared(&resolved.path, action)
    } else {
        store::with_exclusive(&resolved.path, false, action)
    }?;

    let mut meta = Meta::new();
    meta.file = Some(resolved.path.to_string_lossy().into_owned());
    meta.agent_source = Some(agent_source.into());
    meta.warnings = resolved.warnings.clone();
    if dry_run {
        meta.warnings.push("dry run; no record appended".into());
    } else if !changed {
        meta.warnings
            .push("duplicate promotion; existing record returned".into());
    }
    output::write_success(PromoteData { changed, record }, pretty, meta)
        .map_err(|error| AppError::from_io(error, Path::new("stdout")))?;
    Ok(0)
}

fn strip_trailing_newlines(value: &str) -> &str {
    value.trim_end_matches(['\n', '\r'])
}

/// `artifact.ref` and `note` are bounded at 10,000 bytes after redaction — the
/// bound `add` applies to cut text — and reject empty and whitespace-only
/// values (r48).
fn validate_free_text(value: &str, name: &str, flag: &str) -> AppResult<()> {
    if value.trim().is_empty() {
        return Err(AppError::invalid_input(
            format!("promotion {name} cannot be empty or whitespace-only"),
            format!("Pass a non-empty {flag}."),
        ));
    }
    if value.len() > 10_000 {
        return Err(AppError::invalid_input(
            format!(
                "promotion {name} is {} bytes; the maximum is 10000",
                value.len()
            ),
            format!("Shorten {flag} to at most 10000 UTF-8 bytes."),
        ));
    }
    Ok(())
}
