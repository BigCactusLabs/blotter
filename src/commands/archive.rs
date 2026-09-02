use crate::cli::ArchiveArgs;
use crate::error::{AppError, AppResult};
use crate::output::{self, Meta};
use crate::store;
use crate::{ItemStatus, is_bl_id, parse_before};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::path::{Path, PathBuf};

const EMPTY_WARNING: &str = "no blotter file yet; archive has nothing to remove";
const EMPTY_FIX: &str = "Pass an existing --file PATH or omit --file to archive discovered state.";

#[derive(Debug, Serialize, Deserialize)]
pub struct ArchiveData {
    pub changed: bool,
    pub archived: usize,
    pub kept: usize,
    pub archive_file: Option<String>,
    pub backup: Option<String>,
    pub restore_hint: Option<String>,
}

struct ArchivePlan {
    data: ArchiveData,
    kept_bytes: Vec<u8>,
    archived_bytes: Vec<u8>,
    warnings: Vec<String>,
}

pub fn run(
    args: ArchiveArgs,
    file: Option<PathBuf>,
    pretty: bool,
    now: Timestamp,
) -> AppResult<i32> {
    let cutoff = parse_before(&args.before, now)?;
    let resolved = store::discover(file)?;
    let mut warnings = resolved.warnings.clone();
    let data = if args.dry_run {
        dry_run(&resolved, &mut warnings, cutoff)?
    } else {
        apply(&resolved, &mut warnings, cutoff, now)?
    };
    let mut meta = Meta::new();
    meta.file = Some(resolved.path.to_string_lossy().into_owned());
    meta.warnings = warnings;
    output::write_success(data, pretty, meta)
        .map_err(|error| AppError::from_io(error, Path::new("stdout")))?;
    Ok(0)
}

fn dry_run(
    resolved: &store::ResolvedFile,
    warnings: &mut Vec<String>,
    cutoff: Timestamp,
) -> AppResult<ArchiveData> {
    let (plan, _) = store::read_or_empty(
        &resolved.path,
        resolved.explicit,
        warnings,
        EMPTY_WARNING,
        EMPTY_FIX,
        empty_plan,
        |log| {
            let bytes = store::read_bytes(log, &resolved.path)?;
            store::check_version(&bytes, &resolved.path)?;
            Ok(plan_archive(&bytes, cutoff))
        },
    )?;
    warnings.extend(plan.warnings);
    Ok(plan.data)
}

fn apply(
    resolved: &store::ResolvedFile,
    warnings: &mut Vec<String>,
    cutoff: Timestamp,
    now: Timestamp,
) -> AppResult<ArchiveData> {
    match store::with_exclusive(&resolved.path, false, |log| {
        apply_archive(log, &resolved.path, cutoff, now)
    }) {
        Ok((data, plan_warnings)) => {
            warnings.extend(plan_warnings);
            Ok(data)
        }
        Err(error) if error.code == "not_found" && error.exit_code == 66 && !resolved.explicit => {
            warnings.push(EMPTY_WARNING.into());
            Ok(empty_plan().data)
        }
        Err(error) if error.code == "not_found" && error.exit_code == 66 => {
            Err(AppError::not_found(
                format!("blotter file not found: {}", resolved.path.display()),
                EMPTY_FIX,
            ))
        }
        Err(error) => Err(error),
    }
}

fn apply_archive(
    log: &mut File,
    path: &Path,
    cutoff: Timestamp,
    now: Timestamp,
) -> AppResult<(ArchiveData, Vec<String>)> {
    let original = store::read_bytes(log, path)?;
    // Before the plan, and so before any backup or sidecar: a refused log is
    // byte-identical afterwards and gains no files beside it.
    store::check_version(&original, path)?;
    let mut plan = plan_archive(&original, cutoff);
    if plan.data.archived == 0 {
        return Ok((plan.data, plan.warnings));
    }

    let permissions = log
        .metadata()
        .map_err(|error| AppError::from_io(error, path))?
        .permissions();
    // A symlinked log is locked and read through the link; the swap must land
    // on the target, not replace the link with a regular file.
    let path = &store::resolve_symlinked_log(path)?;
    let timestamp = store::backup_timestamp(now);
    let backup_path = store::suffixed_path(path, &format!(".bak-{timestamp}"));
    let archive_path = store::suffixed_path(path, &format!(".archive-{timestamp}.jsonl"));
    let backup = store::write_new_file(&backup_path, &original, &permissions)?;
    let archive = match store::write_new_file(&archive_path, &plan.archived_bytes, &permissions) {
        Ok(archive) => archive,
        Err(error) => {
            remove_created_outputs(&[backup.as_path()]);
            return Err(error);
        }
    };
    if let Err(error) = store::replace_log(
        path,
        &plan.kept_bytes,
        &permissions,
        &format!(".tmp-archive-{}", std::process::id()),
    ) {
        remove_created_outputs(&[backup.as_path(), archive.as_path()]);
        return Err(error);
    }

    plan.data.changed = true;
    plan.data.backup = Some(backup.to_string_lossy().into_owned());
    plan.data.archive_file = Some(archive.to_string_lossy().into_owned());
    plan.data.restore_hint = Some(store::restore_hint(&backup, path));
    Ok((plan.data, plan.warnings))
}

fn plan_archive(bytes: &[u8], cutoff: Timestamp) -> ArchivePlan {
    // One parse pass: the fold carries the (line, id, ts) tuple of every
    // record-carrying physical line, so the line groupings below cost a walk
    // over those tuples instead of a second decode of the whole log.
    let folded = store::fold_bytes_with_lines(bytes);
    let closed_ids = folded
        .items
        .iter()
        .filter(|item| item.status == ItemStatus::Resolved && is_bl_id(&item.id))
        .map(|item| item.id.clone())
        .collect::<HashSet<_>>();

    let mut group_lines = HashMap::<&str, Vec<(usize, bool)>>::new();
    for folded_line in folded.lines() {
        if !is_bl_id(&folded_line.id) {
            continue;
        }
        group_lines
            .entry(folded_line.id.as_str())
            .or_default()
            .push((folded_line.line, folded_line.ts < cutoff));
    }

    let eligible_ids = closed_ids
        .iter()
        .map(String::as_str)
        .filter(|id| {
            group_lines
                .get(id)
                .is_some_and(|events| events.iter().all(|(_, is_old)| *is_old))
        })
        .collect::<HashSet<_>>();
    let removed_lines = group_lines
        .into_iter()
        .filter(|(id, _)| eligible_ids.contains(id))
        .flat_map(|(_, lines)| lines.into_iter().map(|(line, _)| line))
        .collect::<HashSet<_>>();

    let mut kept_bytes = Vec::new();
    let mut archived_bytes = Vec::new();
    let mut archived = 0;
    let mut kept = 0;
    // A file holding only "\n" has zero physical lines under the scan
    // contract; split_inclusive would otherwise count one kept line.
    let body: &[u8] = if bytes == b"\n" { b"" } else { bytes };
    for (index, raw) in body.split_inclusive(|byte| *byte == b'\n').enumerate() {
        if removed_lines.contains(&(index + 1)) {
            archived_bytes.extend_from_slice(raw);
            if !raw.ends_with(b"\n") {
                archived_bytes.push(b'\n');
            }
            archived += 1;
        } else {
            kept_bytes.extend_from_slice(raw);
            // A leading empty segment has zero physical lines under the scan
            // contract (r33/TASK-42): its byte survives, its count does not.
            if !(index == 0 && raw == b"\n") {
                kept += 1;
            }
        }
    }

    ArchivePlan {
        data: ArchiveData {
            changed: false,
            archived,
            kept,
            archive_file: None,
            backup: None,
            restore_hint: None,
        },
        kept_bytes,
        archived_bytes,
        warnings: folded.warnings,
    }
}

fn empty_plan() -> ArchivePlan {
    ArchivePlan {
        data: ArchiveData {
            changed: false,
            archived: 0,
            kept: 0,
            archive_file: None,
            backup: None,
            restore_hint: None,
        },
        kept_bytes: Vec::new(),
        archived_bytes: Vec::new(),
        warnings: Vec::new(),
    }
}

fn remove_created_outputs(paths: &[&Path]) {
    for path in paths {
        let _ = fs::remove_file(path);
    }
}
