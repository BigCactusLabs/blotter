use crate::common::*;

#[test]
fn doctor_reports_orphan_amend_for_unknown_record() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let orphan_amend = json!({
        "v": 2,
        "kind": "resolve",
        "id": "bl_deadbeef0000",
        "ts": "2026-07-09T18:30:00.123Z",
        "agent": "fixture",
        "note": "unknown record amend",
        "amend": true
    });
    std::fs::write(&file, format!("{orphan_amend}\n")).unwrap();

    let output = run_file(&file, &["doctor"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let doctor: SuccessEnvelope<DoctorData> = serde_json::from_slice(&output.stdout).unwrap();
    assert!(!doctor.data.healthy);
    assert_eq!(doctor.data.findings.len(), 1);
    assert_eq!(doctor.data.findings[0].kind, "orphan_resolve");
}

#[test]
fn doctor_accepts_amend_for_existing_resolved_record() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let cut = add(&file, "valid amend fixture");
    let id = cut.data.record.cut_id().to_owned();
    let _: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &["resolve", "--disposition", "fixed", &id],
    ));
    let _: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &["resolve", &id, "--amend", "--note", "corrected"],
    ));

    let doctor: SuccessEnvelope<DoctorData> = success(&run_file(&file, &["doctor"]));
    assert!(doctor.data.healthy);
}

/// The two halves of design doc r36: an amend with no base resolve anywhere in
/// the log is a diagnose-only `orphan_resolve`, and a base resolve appearing
/// after the amend clears both the finding and the fold's orphan warning.
#[test]
fn doctor_reports_an_amend_whose_record_has_no_base_resolve() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let cut = add(&file, "amend without a base");
    let id = cut.data.record.cut_id().to_owned();
    let original = std::fs::read_to_string(&file).unwrap();
    let amend = json!({
        "v": 2,
        "kind": "resolve",
        "id": id,
        "ts": "2026-07-09T18:30:00.123Z",
        "agent": "fixture",
        "note": "base-missing amend",
        "amend": true,
        "disposition": "fixed",
        "disposition_ts": "2026-07-09T18:30:00.123Z"
    });
    std::fs::write(&file, format!("{original}{amend}\n")).unwrap();

    let doctor = doctor_response(&run_file(&file, &["doctor"]), 1);
    assert!(!doctor.data.healthy);
    assert_eq!(doctor.data.findings.len(), 1);
    let finding = &doctor.data.findings[0];
    assert_eq!(finding.kind, "orphan_resolve");
    assert_eq!(finding.line, 2);
    assert!(
        finding.message.contains(&id) && finding.message.contains("base resolve"),
        "message: {}",
        finding.message
    );
    assert!(!finding.fixable);
}

#[test]
fn doctor_accepts_an_amend_that_precedes_its_base_resolve() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let cut = add(&file, "amend ahead of its base");
    let id = cut.data.record.cut_id().to_owned();
    let original = std::fs::read_to_string(&file).unwrap();
    let amend = json!({
        "v": 2,
        "kind": "resolve",
        "id": id,
        "ts": "2026-07-09T18:30:00.123Z",
        "agent": "fixture",
        "note": "amend written first",
        "amend": true,
        "disposition": "fixed",
        "disposition_ts": "2026-07-09T18:30:00.123Z"
    });
    let base = json!({
        "v": 2,
        "kind": "resolve",
        "id": id,
        "ts": "2026-07-09T18:29:00.000Z",
        "agent": "fixture",
        "note": "base written second",
        "disposition": "fixed",
        "disposition_ts": "2026-07-09T18:29:00.000Z"
    });
    std::fs::write(&file, format!("{original}{amend}\n{base}\n")).unwrap();

    let doctor: SuccessEnvelope<DoctorData> = success(&run_file(&file, &["doctor"]));
    assert!(doctor.data.healthy, "findings: {:?}", doctor.data.findings);

    // Doctor and the fold agree in this direction too: no orphan warning.
    let listed: SuccessEnvelope<ListData> = success(&run_file(&file, &["list", "--status", "all"]));
    assert_eq!(listed.data.items[0].status, ItemStatus::Resolved);
    assert!(
        listed.meta.warnings.is_empty(),
        "warnings: {:?}",
        listed.meta.warnings
    );
}

#[test]
fn doctor_reports_all_core_findings_and_recomputed_ids() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let good = add(&file, "valid").data.record;
    let good_line = std::fs::read_to_string(&file).unwrap();
    let bad_id = json!({"v":2,"kind":"cut","id":"bl_000000000000","ts":good.cut_ts(),"agent":"tester","text":"bad","tags":[],"impact":"low","cwd":"/tmp","repo":null});
    let mut writer = OpenOptions::new().append(true).open(&file).unwrap();
    writeln!(writer, "{good_line}{}", bad_id).unwrap();
    writeln!(writer, "{{\"kind\":\"future\"}}").unwrap();
    writeln!(writer, "{{\"v\":2,\"kind\":\"resolve\",\"id\":\"bl_deadbeef0000\",\"ts\":\"2026-07-09T00:00:00.000Z\",\"agent\":\"a\",\"note\":null}}").unwrap();
    writeln!(writer, "<<<<<<< HEAD").unwrap();
    write!(writer, "{{\"kind\":").unwrap();
    drop(writer);
    let output = run_file(&file, &["doctor"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let envelope: SuccessEnvelope<DoctorData> = serde_json::from_slice(&output.stdout).unwrap();
    let kinds: Vec<_> = envelope
        .data
        .findings
        .iter()
        .map(|finding| finding.kind.as_str())
        .collect();
    for kind in [
        "duplicate_cut",
        "id_conflict",
        "unknown_kind",
        "orphan_resolve",
        "conflict_marker",
        "torn_line",
    ] {
        assert!(kinds.contains(&kind), "missing {kind}: {kinds:?}");
    }
    assert!(!envelope.data.healthy);
}

#[test]
fn doctor_fix_dry_run_reports_quarantine_plan_without_writing() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    add(&file, "valid");
    let mut writer = OpenOptions::new().append(true).open(&file).unwrap();
    writer.write_all(b"not-json\n<<<<<<< HEAD\n").unwrap();
    drop(writer);
    let before = std::fs::read(&file).unwrap();

    let doctor = doctor_response(&run_file(&file, &["doctor", "--fix", "--dry-run"]), 1);
    let fix = doctor.data.fix.as_ref().unwrap();
    assert!(!fix.changed);
    assert!(fix.dry_run);
    assert!(fix.backup.is_none());
    assert!(fix.quarantine.is_none());
    assert!(fix.restore_hint.is_none());
    assert_eq!(
        fix.applied
            .iter()
            .map(|applied| (applied.line, applied.kind.as_str(), applied.action.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (2, "malformed", "quarantined"),
            (3, "conflict_marker", "quarantined"),
        ]
    );
    assert!(doctor.data.findings.iter().all(|finding| finding.fixable));
    assert_eq!(std::fs::read(&file).unwrap(), before);
    assert!(!std::path::PathBuf::from(format!("{}.quarantine.jsonl", file.display())).exists());
}

#[test]
fn doctor_fix_quarantines_torn_fragment_and_preserves_exact_backup() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    add(&file, "valid");
    #[cfg(unix)]
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    let complete = std::fs::read(&file).unwrap();
    let mut writer = OpenOptions::new().append(true).open(&file).unwrap();
    writer.write_all(b"{\"kind\":").unwrap();
    drop(writer);
    let original = std::fs::read(&file).unwrap();

    let doctor = doctor_response(&run_file(&file, &["doctor", "--fix"]), 0);
    let fix = doctor.data.fix.as_ref().unwrap();
    assert!(doctor.data.healthy, "findings: {:?}", doctor.data.findings);
    assert!(fix.changed);
    assert!(!fix.dry_run);
    assert_eq!(fix.applied.len(), 1);
    assert_eq!(fix.applied[0].line, 2);
    assert_eq!(fix.applied[0].kind, "torn_line");
    assert_eq!(fix.applied[0].action, "quarantined");
    let backup = std::path::PathBuf::from(fix.backup.as_ref().unwrap());
    let quarantine = std::path::PathBuf::from(fix.quarantine.as_ref().unwrap());
    assert_eq!(
        backup,
        std::path::PathBuf::from(format!("{}.bak-20260709T183000123Z", file.display()))
    );
    assert_eq!(std::fs::read(&backup).unwrap(), original);
    assert_eq!(std::fs::read(&quarantine).unwrap(), b"{\"kind\":\n");
    assert_eq!(std::fs::read(&file).unwrap(), complete);
    #[cfg(unix)]
    for output in [&backup, &quarantine, &file] {
        assert_eq!(permissions_mode(output), 0o600, "{}", output.display());
    }
    let expected_restore = format!("cp '{}' '{}'", backup.display(), file.display());
    assert_eq!(fix.restore_hint.as_deref(), Some(expected_restore.as_str()));
}

#[test]
fn doctor_fix_quarantines_malformed_and_conflict_marker_lines() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    add(&file, "valid");
    let complete = std::fs::read(&file).unwrap();
    let mut writer = OpenOptions::new().append(true).open(&file).unwrap();
    writer.write_all(b"not-json\n<<<<<<< HEAD\n").unwrap();
    drop(writer);
    let original = std::fs::read(&file).unwrap();

    let doctor = doctor_response(&run_file(&file, &["doctor", "--fix"]), 0);
    let fix = doctor.data.fix.as_ref().unwrap();
    assert!(doctor.data.healthy, "findings: {:?}", doctor.data.findings);
    assert_eq!(
        fix.applied
            .iter()
            .map(|applied| (applied.line, applied.kind.as_str(), applied.action.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (2, "malformed", "quarantined"),
            (3, "conflict_marker", "quarantined"),
        ]
    );
    let backup = std::path::PathBuf::from(fix.backup.as_ref().unwrap());
    let quarantine = std::path::PathBuf::from(fix.quarantine.as_ref().unwrap());
    assert_eq!(std::fs::read(&backup).unwrap(), original);
    assert_eq!(
        std::fs::read(&quarantine).unwrap(),
        b"not-json\n<<<<<<< HEAD\n"
    );
    assert_eq!(std::fs::read(&file).unwrap(), complete);
    assert!(
        doctor_response(&run_file(&file, &["doctor"]), 0)
            .data
            .healthy
    );
}

#[test]
fn doctor_fix_leaves_diagnose_only_findings_unchanged() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let good = add(&file, "valid").data.record;
    let good_line = std::fs::read_to_string(&file).unwrap();
    let bad_id = json!({
        "v": 2,
        "kind": "cut",
        "id": "bl_000000000000",
        "ts": good.cut_ts(),
        "agent": "tester",
        "text": "bad",
        "tags": [],
        "impact": "low",
        "cwd": "/tmp",
        "repo": null
    });
    let mut writer = OpenOptions::new().append(true).open(&file).unwrap();
    writer.write_all(good_line.as_bytes()).unwrap();
    writeln!(writer, "{{\"kind\":\"future\"}}").unwrap();
    writeln!(writer, "{{\"v\":2,\"kind\":\"resolve\",\"id\":\"bl_deadbeef0000\",\"ts\":\"2026-07-09T00:00:00.000Z\",\"agent\":\"a\",\"note\":null}}").unwrap();
    writeln!(writer, "{bad_id}").unwrap();
    drop(writer);
    let before = std::fs::read(&file).unwrap();

    let doctor = doctor_response(&run_file(&file, &["doctor", "--fix"]), 1);
    let fix = doctor.data.fix.as_ref().unwrap();
    assert!(!fix.changed);
    assert!(!fix.dry_run);
    assert!(fix.applied.is_empty());
    assert!(fix.backup.is_none());
    assert!(fix.quarantine.is_none());
    for kind in [
        "unknown_kind",
        "orphan_resolve",
        "duplicate_cut",
        "id_conflict",
    ] {
        assert!(
            doctor
                .data
                .findings
                .iter()
                .any(|finding| finding.kind == kind),
            "missing {kind}: {:?}",
            doctor.data.findings
        );
    }
    assert!(doctor.data.findings.iter().all(|finding| !finding.fixable));
    assert_eq!(std::fs::read(&file).unwrap(), before);
}

#[test]
fn doctor_fix_rejects_a_backup_collision_without_changing_the_log() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let original = b"not-json\n";
    std::fs::write(&file, original).unwrap();
    let backup = std::path::PathBuf::from(format!("{}.bak-20260709T183000123Z", file.display()));
    std::fs::write(&backup, b"existing backup").unwrap();

    let envelope = error(&run_file(&file, &["doctor", "--fix"]), 74, "io_error");
    assert!(
        envelope
            .error
            .suggested_fix
            .contains(&format!("{}", backup.display())),
        "suggested_fix must name the leftover backup: {}",
        envelope.error.suggested_fix
    );
    assert_eq!(std::fs::read(&file).unwrap(), original);
    assert_eq!(std::fs::read(&backup).unwrap(), b"existing backup");
    assert!(!std::path::PathBuf::from(format!("{}.quarantine.jsonl", file.display())).exists());
}

#[test]
fn doctor_fix_removes_the_backup_when_the_quarantine_append_fails() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let original = b"not-json\n";
    std::fs::write(&file, original).unwrap();
    // A directory at the quarantine path fails the append after the backup.
    let quarantine = std::path::PathBuf::from(format!("{}.quarantine.jsonl", file.display()));
    std::fs::create_dir(&quarantine).unwrap();

    error(&run_file(&file, &["doctor", "--fix"]), 74, "io_error");
    assert_eq!(std::fs::read(&file).unwrap(), original);
    assert!(
        !std::path::PathBuf::from(format!("{}.bak-20260709T183000123Z", file.display())).exists(),
        "aborted repair must leave no backup"
    );
    assert!(quarantine.is_dir());
}

#[test]
fn doctor_fix_undoes_created_outputs_when_replacement_fails() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let original = b"not-json\n";
    std::fs::write(&file, original).unwrap();
    let quarantine = std::path::PathBuf::from(format!("{}.quarantine.jsonl", file.display()));
    std::fs::write(&quarantine, b"earlier-quarantine\n").unwrap();
    let locked = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&file)
        .unwrap();
    locked.lock().unwrap();
    let mut doctor = spawn_command();
    doctor
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let child = doctor
        .arg("--file")
        .arg(&file)
        .args(["doctor", "--fix"])
        .spawn()
        .unwrap();
    let temporary = std::path::PathBuf::from(format!("{}.tmp-fix-{}", file.display(), child.id()));
    std::fs::write(&temporary, b"existing temporary").unwrap();
    locked.unlock().unwrap();
    let output = child.wait_with_output().unwrap();

    error(&output, 74, "io_error");
    assert_eq!(std::fs::read(&file).unwrap(), original);
    assert_eq!(std::fs::read(&temporary).unwrap(), b"existing temporary");
    assert!(
        !std::path::PathBuf::from(format!("{}.bak-20260709T183000123Z", file.display())).exists(),
        "aborted repair must leave no backup"
    );
    assert_eq!(
        std::fs::read(&quarantine).unwrap(),
        b"earlier-quarantine\n",
        "aborted repair must not extend the quarantine sidecar"
    );
}

#[test]
fn doctor_fix_is_deterministic_for_repeated_identical_input() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let original = b"not-json\n";
    std::fs::write(&file, original).unwrap();

    let first = run_file(&file, &["doctor", "--fix"]);
    let first_data = doctor_response(&first, 0);
    let first_fix = first_data.data.fix.as_ref().unwrap();
    let first_repaired = std::fs::read(&file).unwrap();
    let first_backup = std::path::PathBuf::from(first_fix.backup.as_ref().unwrap());
    let first_quarantine = std::path::PathBuf::from(first_fix.quarantine.as_ref().unwrap());
    let first_backup_name = first_backup.file_name().unwrap().to_owned();
    let first_backup_bytes = std::fs::read(&first_backup).unwrap();

    std::fs::write(&file, original).unwrap();
    std::fs::remove_file(&first_backup).unwrap();
    std::fs::remove_file(&first_quarantine).unwrap();

    let second = run_file(&file, &["doctor", "--fix"]);
    let second_data = doctor_response(&second, 0);
    let second_fix = second_data.data.fix.as_ref().unwrap();
    let second_backup = std::path::PathBuf::from(second_fix.backup.as_ref().unwrap());
    assert_eq!(first.stdout, second.stdout);
    assert_eq!(std::fs::read(&file).unwrap(), first_repaired);
    assert_eq!(std::fs::read(&second_backup).unwrap(), first_backup_bytes);
    assert_eq!(
        second_backup.file_name().unwrap(),
        first_backup_name.as_os_str()
    );
}

#[test]
fn doctor_fix_times_out_under_an_exclusive_lock() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    add(&file, "locked");
    let locked = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&file)
        .unwrap();
    locked.lock().unwrap();
    let output = run_file(&file, &["doctor", "--fix"]);
    locked.unlock().unwrap();
    let envelope = error(&output, 75, "lock_timeout");
    assert!(envelope.error.retryable);
}

#[test]
fn doctor_dry_run_requires_fix() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    error(
        &run_file(&file, &["doctor", "--dry-run"]),
        2,
        "invalid_argument",
    );
}

#[test]
fn doctor_deny_requires_leaks() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let envelope = error(
        &run_file(&file, &["doctor", "--deny", "credential"]),
        2,
        "invalid_argument",
    );
    assert!(envelope.error.message.contains("--leaks"));
}

#[test]
fn doctor_rejects_empty_deny_literal() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let envelope = error(
        &run_file(&file, &["doctor", "--leaks", "--deny", ""]),
        2,
        "invalid_argument",
    );
    assert!(envelope.error.message.contains("empty"));
    assert!(envelope.error.suggested_fix.contains("non-empty"));
}

#[test]
fn plain_doctor_healthy_output_remains_byte_identical() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    add(&file, "valid");

    let output = run_file(&file, &["doctor"]);
    let mut expected = serde_json::to_vec(&json!({
        "ok": true,
        "data": {"healthy": true, "findings": [], "checked_lines": 1},
        "meta": {"contract": 6, "file": file.to_string_lossy()},
    }))
    .unwrap();
    expected.push(b'\n');
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, expected);
}

#[test]
fn doctor_reports_pre_framing_bl_ids_as_conflicts_after_legacy_fallback_removal() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("legacy-v1.jsonl");
    // Frozen v1 hash for this exact comma-joined, non-deduplicated tag fixture.
    let legacy_cut = json!({
        "v": 2,
        "kind": "cut",
        "id": "bl_d7e14e635d21",
        "ts": "2026-07-10T00:00:00.000Z",
        "agent": "legacy",
        "text": "legacy v1 cut",
        "tags": ["a", "a", "b"],
        "impact": "material",
        "cwd": "/tmp",
        "repo": null
    });
    std::fs::write(&file, format!("{legacy_cut}\n")).unwrap();

    let output = run_file(&file, &["doctor"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let doctor: SuccessEnvelope<DoctorData> = serde_json::from_slice(&output.stdout).unwrap();
    assert!(!doctor.data.healthy);
    assert_eq!(doctor.data.findings.len(), 1);
    assert_eq!(doctor.data.findings[0].kind, "id_conflict");
    assert_eq!(doctor.data.checked_lines, 1);
}

#[test]
fn doctor_finding_counts_match_fold_bytes_warning_counts() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let valid_id = compute_id("2026-07-09T00:00:00.000Z", "a", "valid", Impact::Low, &[]);
    let malformed = json!({
        "v": 2,
        "kind": "cut",
        "id": "bl_000000000000",
        "ts": "not-a-time",
        "agent": "a",
        "text": "malformed",
        "tags": [],
        "impact": "low",
        "cwd": "/tmp",
        "repo": null
    })
    .to_string();
    let valid = json!({
        "v": 2,
        "kind": "cut",
        "id": valid_id,
        "ts": "2026-07-09T00:00:00.000Z",
        "agent": "a",
        "text": "valid",
        "tags": [],
        "impact": "low",
        "cwd": "/tmp",
        "repo": null
    })
    .to_string();
    let orphan = json!({
        "v": 2,
        "kind": "resolve",
        "id": "bl_deadbeef0000",
        "ts": "2026-07-09T00:00:00.000Z",
        "agent": "a",
        "note": null
    })
    .to_string();
    let unknown = json!({"v":2,"kind": "future"}).to_string();
    let fixture = format!("{malformed}\n{valid}\n{orphan}\n{valid}\n{unknown}\n{{\"kind\":");
    std::fs::write(&file, fixture).unwrap();

    let folded = blotter::store::fold_bytes(&std::fs::read(&file).unwrap());
    let doctor_output = run_file(&file, &["doctor"]);
    assert_eq!(doctor_output.status.code(), Some(1));
    assert!(doctor_output.stderr.is_empty());
    let doctor: SuccessEnvelope<DoctorData> =
        serde_json::from_slice(&doctor_output.stdout).unwrap();

    let fold_counts = fold_warning_counts(&folded.warnings);
    let doctor_counts = doctor_finding_counts(&doctor.data.findings);
    let expected: HashMap<String, usize> = [
        ("malformed", 1),
        ("unknown", 1),
        ("duplicate_cut", 1),
        ("orphan_resolve", 1),
        ("torn", 1),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    assert_eq!(
        fold_counts, expected,
        "fold warnings: {:?}",
        folded.warnings
    );
    assert_eq!(
        doctor_counts, expected,
        "doctor findings: {:?}",
        doctor.data.findings
    );
}

fn fold_warning_counts(warnings: &[String]) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for warning in warnings {
        let parts: Vec<_> = warning.splitn(3, ' ').collect();
        let count: usize = parts[1].parse().unwrap();
        let label = parts[2].trim_end_matches('s');
        let key = if label.starts_with("malformed line") {
            "malformed"
        } else if label.starts_with("torn final line") {
            "torn"
        } else if label.starts_with("unknown event") {
            "unknown"
        } else if label.starts_with("duplicate cut") {
            "duplicate_cut"
        } else if label.starts_with("duplicate resolve") {
            "duplicate_resolve"
        } else if label.starts_with("orphan resolve") {
            "orphan_resolve"
        } else {
            panic!("unknown fold warning label: {label}")
        };
        counts.insert(key.to_string(), count);
    }
    counts
}

fn doctor_finding_counts(
    findings: &[blotter::commands::doctor::Finding],
) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for finding in findings {
        let key = match finding.kind.as_str() {
            "malformed" => "malformed",
            "torn_line" => "torn",
            "unknown_kind" => "unknown",
            "duplicate_cut" => "duplicate_cut",
            "orphan_resolve" => "orphan_resolve",
            _ => continue,
        };
        *counts.entry(key.to_string()).or_insert(0) += 1;
    }
    counts
}

#[test]
fn doctor_reports_gitignored_finding() {
    let git_available = std::process::Command::new("git")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success());
    if !git_available {
        return;
    }

    let temp = TempDir::new().unwrap();
    let repo = temp.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    assert!(
        std::process::Command::new("git")
            .arg("-C")
            .arg(&repo)
            .arg("init")
            .output()
            .unwrap()
            .status
            .success()
    );
    std::fs::write(repo.join(".gitignore"), ".blotter.jsonl\n").unwrap();

    let empty_output = command().current_dir(&repo).arg("doctor").output().unwrap();
    let empty: SuccessEnvelope<DoctorData> = success(&empty_output);
    assert!(empty.data.healthy);
    assert!(
        empty
            .data
            .findings
            .iter()
            .all(|finding| finding.kind != "gitignored")
    );

    let output = command()
        .current_dir(&repo)
        .args(["add", "gitignored cut", "--agent", "a"])
        .output()
        .unwrap();
    success::<AddData>(&output);

    let doctor_output = command().current_dir(&repo).arg("doctor").output().unwrap();
    assert_eq!(doctor_output.status.code(), Some(1));
    assert!(doctor_output.stderr.is_empty());
    let doctor: SuccessEnvelope<DoctorData> =
        serde_json::from_slice(&doctor_output.stdout).unwrap();
    assert!(!doctor.data.healthy);
    assert!(
        doctor
            .data
            .findings
            .iter()
            .any(|finding| finding.kind == "gitignored")
    );
}

#[cfg(unix)]
#[test]
fn doctor_fix_resolves_symlinked_log_and_preserves_the_link() {
    let temp = TempDir::new().unwrap();
    let target = temp.path().join("real.jsonl");
    let link = temp.path().join("link.jsonl");
    add(&target, "valid");
    let complete = std::fs::read(&target).unwrap();
    let mut writer = OpenOptions::new().append(true).open(&target).unwrap();
    writer.write_all(b"{\"kind\":").unwrap();
    drop(writer);
    std::os::unix::fs::symlink("real.jsonl", &link).unwrap();

    let doctor = doctor_response(&run_file(&link, &["doctor", "--fix"]), 0);
    let fix = doctor.data.fix.as_ref().unwrap();
    assert!(fix.changed);
    assert_eq!(
        fix.backup.as_deref(),
        Some(format!("{}.bak-20260709T183000123Z", target.display()).as_str())
    );
    assert!(
        std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(std::fs::read(&target).unwrap(), complete);
}

/// r48: on a v1 log `doctor` reports the file as one non-fixable
/// `unsupported_version` finding on the first offending line and emits no other
/// record-model finding — the log is not diagnosable under v2 rules. The
/// envelope keeps its shape: `healthy:false`, honest `checked_lines`, exit 1,
/// and `fix` absent on a diagnose-only run.
#[test]
fn doctor_reports_a_v1_log_as_one_unsupported_version_finding() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    // Three physical lines, two of which the v2 scan would otherwise call
    // malformed or unknown, so an honest count is distinguishable from one.
    std::fs::write(
        &file,
        format!("not json\n{}\n{{\"kind\":\"future\"}}\n", v1_cut_line()),
    )
    .unwrap();

    let doctor = doctor_response(&run_file(&file, &["doctor"]), 1);
    assert!(!doctor.data.healthy);
    assert_eq!(doctor.data.findings.len(), 1);
    let finding = &doctor.data.findings[0];
    assert_eq!(finding.kind, "unsupported_version");
    assert_eq!(finding.line, 2);
    assert!(!finding.fixable);
    assert_eq!(
        finding.message,
        "unsupported log version on line 2: record has no v field"
    );
    // The probe read every physical line, so the count is honest.
    assert_eq!(doctor.data.checked_lines, 3);
    assert!(doctor.data.fix.is_none());
}

/// `--fix` needs no special case: one non-fixable finding plans nothing, so the
/// `fix` object is present and inert on both the apply and the dry-run path, and
/// neither creates a sidecar.
#[test]
fn doctor_fix_is_inert_on_a_v1_log_and_creates_no_sidecar() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let original = format!("{}\n", v1_cut_line());
    std::fs::write(&file, &original).unwrap();

    for args in [
        &["doctor", "--fix"][..],
        &["doctor", "--fix", "--dry-run"][..],
    ] {
        let doctor = doctor_response(&run_file(&file, args), 1);
        let fix = doctor
            .data
            .fix
            .as_ref()
            .expect("--fix reports a fix object");
        assert!(!fix.changed, "{args:?}");
        assert!(fix.applied.is_empty(), "{args:?}");
        assert!(fix.backup.is_none(), "{args:?}");
        assert!(fix.quarantine.is_none(), "{args:?}");
        assert!(fix.restore_hint.is_none(), "{args:?}");
        assert_eq!(fix.dry_run, args.contains(&"--dry-run"), "{args:?}");
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            original,
            "{args:?}"
        );
        assert_eq!(directory_entries(temp.path()), ["cuts.jsonl"], "{args:?}");
    }
}

/// r50 supersedes r48's "that single entry": "no other findings" means no
/// finding that classifies a record under the v2 record model. `gitignored` is
/// about version-control status and `--leaks` is a byte-level privacy audit, so
/// both survive the version refusal, ordered `unsupported_version`, then leak
/// findings in line order, then `gitignored`.
#[test]
fn doctor_keeps_gitignored_and_leak_findings_on_a_refused_log() {
    let git_available = std::process::Command::new("git")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success());
    if !git_available {
        return;
    }
    let temp = TempDir::new().unwrap();
    let repo = temp.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    assert!(
        std::process::Command::new("git")
            .arg("-C")
            .arg(&repo)
            .arg("init")
            .output()
            .unwrap()
            .status
            .success()
    );
    std::fs::write(repo.join(".gitignore"), ".blotter.jsonl\n").unwrap();
    std::fs::write(repo.join(".blotter.jsonl"), format!("{}\n", v1_cut_line())).unwrap();

    let doctor: SuccessEnvelope<DoctorData> = serde_json::from_slice(
        &command()
            .current_dir(&repo)
            .arg("doctor")
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    assert!(!doctor.data.healthy);
    assert_eq!(
        doctor
            .data
            .findings
            .iter()
            .map(|finding| finding.kind.as_str())
            .collect::<Vec<_>>(),
        ["unsupported_version", "gitignored"]
    );

    // The same file with a deny pattern: the leak finding sits between them.
    let leaks: SuccessEnvelope<DoctorData> = serde_json::from_slice(
        &command()
            .current_dir(&repo)
            .args(["doctor", "--leaks", "--deny", "legacy"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    assert_eq!(
        leaks
            .data
            .findings
            .iter()
            .map(|finding| finding.kind.as_str())
            .collect::<Vec<_>>(),
        ["unsupported_version", "leak", "gitignored"]
    );
}

/// r48 rules (1)-(3), which Phase 3 implements: one non-fixable
/// `invalid_resolution` finding per invalid event, naming the record ID and
/// every rule the event breaks in the numbered order.
#[test]
fn doctor_reports_invalid_resolutions_for_rules_one_to_three() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let cut = add(&file, "invalid resolution fixture");
    let cut_id = cut.data.record.cut_id().to_owned();
    let dogear: SuccessEnvelope<Value> = success(&run_file(
        &file,
        &["dogear", "invalid resolution dogear", "--agent", "tester"],
    ));
    let dogear_id = dogear.data["record"]["id"].as_str().unwrap().to_owned();
    let original = std::fs::read_to_string(&file).unwrap();

    // (1) a cut resolve with no disposition.
    let no_disposition = json!({
        "v": 2, "kind": "resolve", "id": cut_id,
        "ts": "2026-07-09T18:31:00.000Z", "agent": "fixture", "note": null
    });
    // (2) a dogear resolve that carries one, which also breaks nothing else.
    let dogear_disposition = json!({
        "v": 2, "kind": "resolve", "id": dogear_id,
        "ts": "2026-07-09T18:32:00.000Z", "agent": "fixture", "note": null,
        "disposition": "fixed", "disposition_ts": "2026-07-09T18:32:00.000Z"
    });
    // (3) a disposition with no disposition_ts. The cut arm of (1) is satisfied,
    // so this event breaks exactly one rule.
    let half_disposition = json!({
        "v": 2, "kind": "resolve", "id": cut_id,
        "ts": "2026-07-09T18:33:00.000Z", "agent": "fixture", "note": null,
        "disposition": "fixed"
    });
    std::fs::write(
        &file,
        format!("{original}{no_disposition}\n{dogear_disposition}\n{half_disposition}\n"),
    )
    .unwrap();

    let doctor = doctor_response(&run_file(&file, &["doctor"]), 1);
    let findings = &doctor.data.findings;
    assert_eq!(findings.len(), 3, "findings: {findings:?}");
    assert!(
        findings
            .iter()
            .all(|finding| finding.kind == "invalid_resolution")
    );
    assert!(findings.iter().all(|finding| !finding.fixable));
    assert_eq!(
        findings[0].message,
        format!("invalid resolution for {cut_id}: resolve targets a cut without a disposition")
    );
    assert_eq!(
        findings[1].message,
        format!("invalid resolution for {dogear_id}: resolve targets a dogear with a disposition")
    );
    assert_eq!(
        findings[2].message,
        format!(
            "invalid resolution for {cut_id}: disposition and disposition_ts must be present together"
        )
    );
    assert_eq!(
        findings
            .iter()
            .map(|finding| finding.line)
            .collect::<Vec<_>>(),
        [3, 4, 5]
    );

    // The fold discards all three, so both records read open and the warning
    // counts events, not rules.
    let listed: SuccessEnvelope<ListData> = success(&run_file(
        &file,
        &["list", "--kind", "all", "--status", "all"],
    ));
    assert!(
        listed
            .data
            .items
            .iter()
            .all(|item| item.status == ItemStatus::Open)
    );
    assert_eq!(listed.meta.warnings, ["skipped 3 invalid resolutions"]);
}
