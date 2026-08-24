use crate::cli::DoctorArgs;
use crate::error::{AppError, AppResult};
use crate::output::{self, Meta};
use crate::store;
use crate::{LogEvent, compute_dogear_id, compute_id};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const EMPTY_WARNING: &str = "no blotter file yet; healthy empty state";
const EMPTY_FIX: &str = "Pass an existing --file PATH or omit --file to inspect discovered state.";
// Byte mirror of `commands::add::EVIDENCE_DELIMITERS` for raw leak scans.
// A slash is a path parent, not a delimiter.
const EVIDENCE_DELIMITERS: &[u8] = b",;)]}&#\"'";
// Byte mirror of `commands::add::HOME_PREFIXES`.
const HOME_PREFIXES: [&[u8]; 4] = [b"/Users/", b"/home/", b"-Users-", b"-home-"];

struct LeakScan<'a> {
    home: Option<Vec<u8>>,
    dash_home: Option<Vec<u8>>,
    deny: &'a [String],
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DoctorData {
    pub healthy: bool,
    pub findings: Vec<Finding>,
    pub checked_lines: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix: Option<FixData>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Finding {
    pub line: usize,
    pub kind: String,
    pub message: String,
    #[serde(default)]
    pub fixable: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FixData {
    pub changed: bool,
    pub applied: Vec<AppliedFix>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quarantine: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restore_hint: Option<String>,
    pub dry_run: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppliedFix {
    pub line: usize,
    pub kind: String,
    pub action: String,
}

pub fn run(
    args: DoctorArgs,
    file: Option<PathBuf>,
    pretty: bool,
    now: Timestamp,
) -> AppResult<i32> {
    if args.dry_run && !args.fix {
        return Err(AppError::invalid_argument(
            "--dry-run requires --fix for doctor",
            "Run `blotter doctor --fix --dry-run` to preview repairs.",
        ));
    }
    if args.leaks && args.fix {
        return Err(AppError::invalid_argument(
            "--leaks conflicts with --fix for doctor",
            "Run `blotter doctor --leaks` without --fix; the gate is read-only.",
        ));
    }
    if !args.deny.is_empty() && !args.leaks {
        return Err(AppError::invalid_argument(
            "--deny requires --leaks for doctor",
            "Run `blotter doctor --leaks --deny LITERAL` to scan a literal deny pattern.",
        ));
    }
    if args.deny.iter().any(|pattern| pattern.is_empty()) {
        return Err(AppError::invalid_argument(
            "--deny requires a non-empty literal",
            "Run `blotter doctor --leaks --deny LITERAL` with a non-empty literal.",
        ));
    }
    let leak_scan = args.leaks.then(|| {
        let home = current_home_path();
        let dash_home = home.as_ref().map(|home| {
            home.iter()
                .map(|byte| if *byte == b'/' { b'-' } else { *byte })
                .collect()
        });
        LeakScan {
            home,
            dash_home,
            deny: &args.deny,
        }
    });
    let resolved = store::discover(file)?;
    let mut warnings = resolved.warnings.clone();
    let (mut data, file_existed) = match (args.fix, args.dry_run) {
        (false, _) => diagnose_shared(&resolved, &mut warnings, leak_scan.as_ref())?,
        (true, true) => {
            let (mut data, file_existed) =
                diagnose_shared(&resolved, &mut warnings, leak_scan.as_ref())?;
            data.fix = Some(FixData {
                changed: false,
                applied: planned_fixes(&data.findings),
                backup: None,
                quarantine: None,
                restore_hint: None,
                dry_run: true,
            });
            (data, file_existed)
        }
        (true, false) => diagnose_and_fix(&resolved, &mut warnings, now)?,
    };
    add_gitignored_finding(&mut data, &resolved, file_existed);
    let exit = i32::from(!data.healthy);
    let mut meta = Meta::new();
    meta.file = Some(resolved.path.to_string_lossy().into_owned());
    meta.warnings = warnings;
    output::write_success(data, pretty, meta)
        .map_err(|error| AppError::from_io(error, Path::new("stdout")))?;
    Ok(exit)
}

fn diagnose_shared(
    resolved: &store::ResolvedFile,
    warnings: &mut Vec<String>,
    leak_scan: Option<&LeakScan<'_>>,
) -> AppResult<(DoctorData, bool)> {
    store::read_or_empty(
        &resolved.path,
        resolved.explicit,
        warnings,
        EMPTY_WARNING,
        EMPTY_FIX,
        empty_data,
        |log| {
            let bytes = store::read_bytes(log, &resolved.path)?;
            Ok(inspect(&bytes, leak_scan))
        },
    )
}

fn diagnose_and_fix(
    resolved: &store::ResolvedFile,
    warnings: &mut Vec<String>,
    now: Timestamp,
) -> AppResult<(DoctorData, bool)> {
    match store::with_exclusive(&resolved.path, false, |log| {
        apply_fixes(log, &resolved.path, now)
    }) {
        Ok(data) => Ok((data, true)),
        Err(error) if error.code == "not_found" && error.exit_code == 66 && !resolved.explicit => {
            warnings.push(EMPTY_WARNING.into());
            let mut data = empty_data();
            data.fix = Some(FixData {
                changed: false,
                applied: Vec::new(),
                backup: None,
                quarantine: None,
                restore_hint: None,
                dry_run: false,
            });
            Ok((data, false))
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

fn apply_fixes(log: &mut File, path: &Path, now: Timestamp) -> AppResult<DoctorData> {
    let original = store::read_bytes(log, path)?;
    let before = inspect(&original, None);
    let applied = planned_fixes(&before.findings);
    if applied.is_empty() {
        return Ok(with_fix(
            before,
            FixData {
                changed: false,
                applied,
                backup: None,
                quarantine: None,
                restore_hint: None,
                dry_run: false,
            },
        ));
    }

    let permissions = log
        .metadata()
        .map_err(|error| AppError::from_io(error, path))?
        .permissions();
    // A symlinked log is locked and read through the link; the swap must land
    // on the target, not replace the link with a regular file.
    let path = &store::resolve_symlinked_log(path)?;
    let backup_path = store::suffixed_path(path, &format!(".bak-{}", store::backup_timestamp(now)));
    if backup_path.exists() {
        return Err(AppError::stale_backup(&backup_path));
    }
    let backup = store::write_new_file(&backup_path, &original, &permissions)?;
    let quarantine_path = store::suffixed_path(path, ".quarantine.jsonl");
    // `append_file` extends an existing quarantine sidecar, so its rollback
    // restores the prior length instead of deleting an earlier repair's lines.
    let quarantine_len = fs::metadata(&quarantine_path)
        .ok()
        .map(|metadata| metadata.len());
    let quarantined = quarantined_bytes(&original, &applied);
    let quarantine = match store::append_file(&quarantine_path, &quarantined, &permissions) {
        Ok(quarantine) => quarantine,
        Err(error) => {
            // A failed append into a pre-existing sidecar leaves partial
            // bytes behind; truncate back alongside removing the backup.
            undo_created_outputs(&[
                (backup.as_path(), None),
                (quarantine_path.as_path(), quarantine_len),
            ]);
            return Err(error);
        }
    };
    let repaired = repaired_bytes(&original, &applied);
    if let Err(error) = store::replace_log(
        path,
        &repaired,
        &permissions,
        &format!(".tmp-fix-{}", std::process::id()),
    ) {
        undo_created_outputs(&[
            (backup.as_path(), None),
            (quarantine.as_path(), quarantine_len),
        ]);
        return Err(error);
    }
    // The swap renames a new inode over the path, so the held lock no longer
    // covers the file there; diagnose the bytes just written, never a reread —
    // and derive that diagnosis from the pre-fix findings rather than parsing
    // those bytes a second time.
    Ok(with_fix(
        post_fix_data(&before, &applied),
        FixData {
            changed: true,
            applied,
            backup: Some(backup.to_string_lossy().into_owned()),
            quarantine: Some(quarantine.to_string_lossy().into_owned()),
            restore_hint: Some(store::restore_hint(&backup, path)),
            dry_run: false,
        },
    ))
}

/// Mirror of `archive::remove_created_outputs` for an aborted repair: a
/// sidecar this run created is removed, and one it only extended is truncated
/// back. Without it a failed repair leaves a backup that claims a repair which
/// never happened, and the retry then fails on that leftover.
fn undo_created_outputs(outputs: &[(&Path, Option<u64>)]) {
    for (path, previous_len) in outputs {
        match previous_len {
            None => {
                let _ = fs::remove_file(path);
            }
            Some(len) => {
                let _ = OpenOptions::new()
                    .write(true)
                    .open(path)
                    .and_then(|file| file.set_len(*len));
            }
        }
    }
}

/// What `inspect` would report on the repaired bytes, derived from the pre-fix
/// report instead of decoding the whole log again.
///
/// `repaired_bytes` only drops whole quarantined lines, so the surviving
/// findings are exactly the ones no fix removed, each renumbered by the lines
/// dropped ahead of it. Nothing that survives depended on a dropped line: only a
/// scan error is fixable, a scan error is the only finding its line can carry,
/// and a line that failed to parse contributes no record, no duplicate payload,
/// no resolve target and no base resolve. Dropping a line also cannot tear the tail or orphan
/// the leading empty segment, because each dropped line takes its own newline.
fn post_fix_data(before: &DoctorData, applied: &[AppliedFix]) -> DoctorData {
    let mut removed: Vec<usize> = applied
        .iter()
        .filter(|fix| fix.action == "quarantined")
        .map(|fix| fix.line)
        .collect();
    removed.sort_unstable();
    let findings: Vec<Finding> = before
        .findings
        .iter()
        .filter(|finding| removed.binary_search(&finding.line).is_err())
        .map(|finding| Finding {
            line: finding.line - removed.partition_point(|line| *line < finding.line),
            kind: finding.kind.clone(),
            message: finding.message.clone(),
            fixable: finding.fixable,
        })
        .collect();
    DoctorData {
        healthy: findings.is_empty(),
        findings,
        checked_lines: before.checked_lines - removed.len(),
        fix: None,
    }
}

fn with_fix(mut data: DoctorData, fix: FixData) -> DoctorData {
    data.fix = Some(fix);
    data
}

fn repaired_bytes(bytes: &[u8], applied: &[AppliedFix]) -> Vec<u8> {
    let remove_lines: HashSet<_> = applied
        .iter()
        .filter(|fix| fix.action == "quarantined")
        .map(|fix| fix.line)
        .collect();
    let mut repaired = Vec::new();
    for (index, raw) in bytes.split_inclusive(|byte| *byte == b'\n').enumerate() {
        if !remove_lines.contains(&(index + 1)) {
            repaired.extend_from_slice(raw);
        }
    }
    repaired
}

fn quarantined_bytes(bytes: &[u8], applied: &[AppliedFix]) -> Vec<u8> {
    let remove_lines: HashSet<_> = applied
        .iter()
        .filter(|fix| fix.action == "quarantined")
        .map(|fix| fix.line)
        .collect();
    let mut quarantined = Vec::new();
    for (index, raw) in bytes.split_inclusive(|byte| *byte == b'\n').enumerate() {
        if remove_lines.contains(&(index + 1)) {
            quarantined.extend_from_slice(raw);
            if !raw.ends_with(b"\n") {
                quarantined.push(b'\n');
            }
        }
    }
    quarantined
}

fn planned_fixes(findings: &[Finding]) -> Vec<AppliedFix> {
    findings
        .iter()
        .filter(|finding| finding.fixable)
        .map(|finding| AppliedFix {
            line: finding.line,
            kind: finding.kind.clone(),
            action: "quarantined".into(),
        })
        .collect()
}

fn add_gitignored_finding(
    data: &mut DoctorData,
    resolved: &store::ResolvedFile,
    file_existed: bool,
) {
    if file_existed
        && let Some(repo) = resolved.repo.as_ref()
        && resolved.path.starts_with(repo)
        && Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["check-ignore", "-q", "--"])
            .arg(&resolved.path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    {
        data.findings.push(finding(
            0,
            "gitignored",
            "blotter file is gitignored; blotter will not appear in diffs",
        ));
        data.healthy = false;
    }
}

fn empty_data() -> DoctorData {
    DoctorData {
        healthy: true,
        findings: Vec::new(),
        checked_lines: 0,
        fix: None,
    }
}

fn finding(line: usize, kind: impl Into<String>, message: impl Into<String>) -> Finding {
    let kind = kind.into();
    Finding {
        line,
        fixable: matches!(kind.as_str(), "torn_line" | "malformed" | "conflict_marker"),
        kind,
        message: message.into(),
    }
}

fn inspect(bytes: &[u8], leak_scan: Option<&LeakScan<'_>>) -> DoctorData {
    let mut findings = Vec::new();
    let mut leak_findings = Vec::new();
    let mut records = HashMap::<String, Vec<u8>>::new();
    let mut record_ids = HashSet::new();
    let mut base_resolve_ids = HashSet::new();
    let mut resolves = Vec::<(usize, String, bool)>::new();
    let mut checked_lines = 0;
    for scanned in store::scan(bytes) {
        checked_lines += 1;
        let line = scanned.line;
        if let Some(leak_scan) = leak_scan {
            add_leak_findings(&mut leak_findings, line, scanned.raw, leak_scan);
        }
        match scanned.event {
            Err(store::ScanIssue::Torn) => findings.push(finding(
                line,
                "torn_line",
                "final physical line is not newline-terminated",
            )),
            Err(store::ScanIssue::Malformed(message)) => {
                if scanned.raw.starts_with(b"<<<<<<< ") || scanned.raw.starts_with(b">>>>>>> ") {
                    findings.push(finding(
                        line,
                        "conflict_marker",
                        "complete git conflict-marker line found",
                    ));
                } else {
                    findings.push(finding(line, "malformed", message));
                }
            }
            Err(store::ScanIssue::Unknown(kind)) => findings.push(finding(
                line,
                "unknown_kind",
                kind.map_or_else(
                    || "event has no string kind field".into(),
                    |kind| format!("unknown event kind '{kind}'"),
                ),
            )),
            Ok(event) => match event {
                LogEvent::Cut {
                    id,
                    ts,
                    agent,
                    text,
                    tags,
                    severity,
                    ..
                } => {
                    if id
                        .get(..3)
                        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("bl_"))
                    {
                        let expected = compute_id(&ts, &agent, &text, severity, &tags);
                        if id != expected {
                            findings.push(finding(
                                line,
                                "id_conflict",
                                format!("cut ID {id} does not recompute to {expected}"),
                            ));
                        }
                    }
                    if let Some(first) = records.get(&id) {
                        let (kind, message) = if first == scanned.raw {
                            (
                                "duplicate_cut",
                                format!("byte-identical duplicate cut {id}"),
                            )
                        } else {
                            (
                                "id_conflict",
                                format!(
                                    "cut {id} has a different payload than its first occurrence"
                                ),
                            )
                        };
                        findings.push(finding(line, kind, message));
                    } else {
                        records.insert(id.clone(), scanned.raw.to_vec());
                    }
                    record_ids.insert(id);
                }
                LogEvent::Dogear {
                    id,
                    ts,
                    agent,
                    text,
                    tags,
                    ..
                } => {
                    if id
                        .get(..3)
                        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("bl_"))
                    {
                        let mut tags = tags;
                        tags.sort();
                        let expected = compute_dogear_id(&ts, &agent, &text, &tags);
                        if id != expected {
                            findings.push(finding(
                                line,
                                "id_conflict",
                                format!("dogear ID {id} does not recompute to {expected}"),
                            ));
                        }
                    }
                    if let Some(first) = records.get(&id) {
                        let (kind, message) = if first == scanned.raw {
                            (
                                "duplicate_dogear",
                                format!("byte-identical duplicate dogear {id}"),
                            )
                        } else {
                            (
                                "id_conflict",
                                format!(
                                    "dogear {id} has a different payload than its first occurrence"
                                ),
                            )
                        };
                        findings.push(finding(line, kind, message));
                    } else {
                        records.insert(id.clone(), scanned.raw.to_vec());
                    }
                    record_ids.insert(id);
                }
                LogEvent::Resolve { id, amend, .. } => {
                    if !amend {
                        base_resolve_ids.insert(id.clone());
                    }
                    resolves.push((line, id, amend));
                }
                LogEvent::Unknown => unreachable!("scanner classifies unknown events"),
            },
        }
    }
    for (line, id, amend) in resolves {
        let message = if !record_ids.contains(&id) {
            Some(format!("resolve references unknown record {id}"))
        } else if amend && !base_resolve_ids.contains(&id) {
            Some(format!(
                "amend references record {id} without a base resolve"
            ))
        } else {
            None
        };
        if let Some(message) = message {
            findings.push(finding(line, "orphan_resolve", message));
        }
    }
    findings.extend(leak_findings);
    DoctorData {
        healthy: findings.is_empty(),
        findings,
        checked_lines,
        fix: None,
    }
}

fn current_home_path() -> Option<Vec<u8>> {
    let cwd = std::env::current_dir().ok()?;
    store::home_dir(&cwd)
        .filter(|home| home.is_absolute())
        .and_then(|home| home.to_str().map(|home| home.as_bytes().to_vec()))
}

fn evidence_delimiter(byte: u8) -> bool {
    byte.is_ascii_whitespace() || EVIDENCE_DELIMITERS.contains(&byte)
}

fn path_prefix_boundary(bytes: &[u8], end: usize, separator: u8) -> bool {
    bytes
        .get(end)
        .is_none_or(|byte| *byte == b'/' || *byte == separator || evidence_delimiter(*byte))
}

fn dash_start_boundary(bytes: &[u8], start: usize) -> bool {
    start == 0
        || bytes
            .get(start - 1)
            .is_some_and(|byte| evidence_delimiter(*byte) || *byte == b'/')
}

fn generic_home_path_end(bytes: &[u8], start: usize) -> Option<usize> {
    let prefix = HOME_PREFIXES
        .into_iter()
        .find(|prefix| bytes[start..].starts_with(prefix))?;
    let separator = prefix[0];
    // Generic aliases only start a token. Unlike exact $HOME matching, a
    // preceding slash makes the slash form a nested path such as
    // /mnt/home/shared; a dash-encoded slug normally does follow a slash.
    if start != 0
        && !bytes
            .get(start - 1)
            .is_some_and(|byte| evidence_delimiter(*byte) || (separator == b'-' && *byte == b'/'))
    {
        return None;
    }
    let component_start = start + prefix.len();
    let mut component_end = component_start;
    while let Some(byte) = bytes.get(component_end) {
        if *byte == b'/' || *byte == separator || evidence_delimiter(*byte) {
            break;
        }
        component_end += 1;
    }
    (component_end > component_start && path_prefix_boundary(bytes, component_end, separator))
        .then_some(component_end)
}

fn contains_home_path(bytes: &[u8], home: Option<&[u8]>, dash_home: Option<&[u8]>) -> bool {
    let mut start = 0;
    while start < bytes.len() {
        let home_end = home
            .filter(|home| bytes[start..].starts_with(home))
            .map(|home| start + home.len())
            .filter(|end| path_prefix_boundary(bytes, *end, b'/'));
        // Exact current home in dash-encoded form; mirrors the redaction-side
        // precedence so dashed usernames and non-generic homes are caught.
        let dash_home_end = dash_home
            .filter(|_| dash_start_boundary(bytes, start))
            .filter(|dash| bytes[start..].starts_with(dash))
            .map(|dash| start + dash.len())
            .filter(|end| path_prefix_boundary(bytes, *end, b'-'));
        if home_end.is_some()
            || dash_home_end.is_some()
            || generic_home_path_end(bytes, start).is_some()
        {
            return true;
        }
        start += 1;
    }
    false
}

fn add_leak_findings(
    findings: &mut Vec<Finding>,
    line: usize,
    raw: &[u8],
    leak_scan: &LeakScan<'_>,
) {
    if contains_home_path(
        raw,
        leak_scan.home.as_deref(),
        leak_scan.dash_home.as_deref(),
    ) {
        findings.push(finding(
            line,
            "leak",
            format!("line {line} contains home path"),
        ));
    }
    for pattern in leak_scan.deny {
        let matches = raw
            .windows(pattern.len())
            .any(|candidate| candidate == pattern.as_bytes());
        if matches {
            findings.push(finding(
                line,
                "leak",
                format!("line {line} contains deny pattern {pattern:?}"),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cut(id: &str) -> String {
        format!(
            r#"{{"kind":"cut","id":"{id}","ts":"2026-01-15T00:00:00.000Z","agent":"t","text":"x","tags":[],"severity":"minor","cwd":"."}}"#
        )
    }

    fn resolve(id: &str) -> String {
        format!(
            r#"{{"kind":"resolve","id":"{id}","ts":"2026-01-15T00:00:01.000Z","agent":"t","note":null}}"#
        )
    }

    /// The derived post-fix report must equal a full reinspection of the
    /// repaired bytes on every shape, including the ones where line numbers
    /// shift, the log keeps a leading empty segment, or the repair empties it.
    #[test]
    fn derived_post_fix_report_matches_a_full_reinspection() {
        let cases: Vec<Vec<u8>> = vec![
            b"".to_vec(),
            b"\n".to_vec(),
            b"not-json\n".to_vec(),
            b"not-json".to_vec(),
            format!("\n{}\nnot-json\n", cut("bl_aaaaaaaaaaaa")).into_bytes(),
            format!("not-json\n{}\n{}\n", cut("bl_a"), cut("bl_a")).into_bytes(),
            format!("{}\nnot-json\n{}\n", cut("bl_a"), cut("bl_a")).into_bytes(),
            format!("{}\n{}\nnot-json", cut("bl_a"), resolve("bl_b")).into_bytes(),
            format!(
                "<<<<<<< HEAD\n{}\n>>>>>>> other\n{}\n",
                cut("bl_a"),
                resolve("bl_zzz")
            )
            .into_bytes(),
            br#"{"kind":"nope"}"#.to_vec(),
            format!("{{\"kind\":\"nope\"}}\nnot-json\n{}\n", cut("bl_a")).into_bytes(),
            b"not-json\nnot-json\nnot-json\n".to_vec(),
            format!("{}\n{}\n", cut("bl_a"), resolve("bl_a")).into_bytes(),
        ];
        for bytes in cases {
            let before = inspect(&bytes, None);
            let applied = planned_fixes(&before.findings);
            let repaired = repaired_bytes(&bytes, &applied);
            let expected = serde_json::to_value(inspect(&repaired, None)).unwrap();
            let derived = serde_json::to_value(post_fix_data(&before, &applied)).unwrap();
            assert_eq!(
                derived,
                expected,
                "derived report drifted for {:?}",
                String::from_utf8_lossy(&bytes)
            );
        }
    }
}
