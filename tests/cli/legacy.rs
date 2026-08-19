use crate::common::*;

#[test]
fn old_format_cuts_without_source_fold_and_list_byte_identically() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("old-format.jsonl");
    let id = compute_id(
        "2026-08-01T00:00:00.000Z",
        "legacy",
        "old-format cut",
        Severity::Minor,
        &["legacy".into()],
    );
    let stored = json!({
        "kind": "cut",
        "id": id,
        "ts": "2026-08-01T00:00:00.000Z",
        "agent": "legacy",
        "text": "old-format cut",
        "tags": ["legacy"],
        "severity": "minor",
        "cwd": "/tmp"
    });
    let stored_bytes = format!("{stored}\n");
    std::fs::write(&file, &stored_bytes).unwrap();

    let output = run_file(&file, &["list"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let file_json = serde_json::to_string(&file.to_string_lossy()).unwrap();
    let expected = format!(
        r#"{{"ok":true,"data":{{"items":[{{"kind":"cut","id":"{id}","ts":"2026-08-01T00:00:00.000Z","agent":"legacy","text":"old-format cut","tags":["legacy"],"severity":"minor","cwd":"/tmp","status":"open"}}],"count":1,"total":1,"truncated":false}},"meta":{{"contract":5,"file":{file_json}}}}}"#
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("{expected}\n")
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), stored_bytes);
}

#[test]
fn unknown_stored_source_round_trips_through_fold_and_list() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("unknown-source.jsonl");
    let id = compute_id(
        "2026-08-01T00:00:00.000Z",
        "future",
        "future source",
        Severity::Minor,
        &[],
    );
    let stored = json!({
        "kind": "cut",
        "id": id,
        "ts": "2026-08-01T00:00:00.000Z",
        "agent": "future",
        "text": "future source",
        "tags": [],
        "severity": "minor",
        "cwd": "/tmp",
        "source": "other"
    });
    let stored_bytes = format!("{stored}\n");
    std::fs::write(&file, &stored_bytes).unwrap();

    let listed: SuccessEnvelope<Value> = success(&run_file(&file, &["list"]));
    assert_eq!(listed.data["items"][0]["source"], "other");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), stored_bytes);
}

#[test]
fn source_propagates_to_triage_digest_and_verify_json() {
    let temp = TempDir::new().unwrap();
    let analysis_file = temp.path().join("analysis-source.jsonl");
    let analysis_text = "source provenance analysis";
    let first_ts = "2026-07-09T18:29:00.000Z";
    let second_ts = "2026-07-09T18:30:00.000Z";
    let tags = vec!["provenance".into()];
    let first = json!({
        "kind": "cut",
        "id": compute_id(first_ts, "hook", analysis_text, Severity::Minor, &tags),
        "ts": first_ts,
        "agent": "hook",
        "text": analysis_text,
        "tags": tags,
        "severity": "minor",
        "cwd": "/tmp",
        "source": "hook"
    });
    let second = json!({
        "kind": "cut",
        "id": compute_id(second_ts, "hook", analysis_text, Severity::Minor, &tags),
        "ts": second_ts,
        "agent": "hook",
        "text": analysis_text,
        "tags": tags,
        "severity": "minor",
        "cwd": "/tmp",
        "source": "hook"
    });
    std::fs::write(&analysis_file, format!("{first}\n{second}\n")).unwrap();

    let triage = triage_success(
        &run_file(&analysis_file, &["triage", "--min-count", "2"]),
        1,
    );
    assert_eq!(triage.data["clusters"][0]["source"], "hook");
    let digest: SuccessEnvelope<Value> =
        success(&run_file(&analysis_file, &["digest", "--since", "1d"]));
    assert_eq!(digest.data["chronic"][0]["source"], "hook");

    let verify_file = temp.path().join("verify-source.jsonl");
    let resolved_text = "source provenance recurrence";
    let resolved_ts = "2026-07-09T16:00:00.000Z";
    let recurrence_ts = "2026-07-09T16:20:00.000Z";
    let resolved_id = compute_id(resolved_ts, "hook", resolved_text, Severity::Minor, &[]);
    let recurrence_id = compute_id(
        recurrence_ts,
        "self-report",
        resolved_text,
        Severity::Minor,
        &[],
    );
    let resolved = json!({
        "kind": "cut",
        "id": resolved_id,
        "ts": resolved_ts,
        "agent": "hook",
        "text": resolved_text,
        "tags": [],
        "severity": "minor",
        "cwd": "/tmp",
        "source": "hook"
    });
    let resolution = json!({
        "kind": "resolve",
        "id": resolved_id,
        "ts": "2026-07-09T16:10:00.000Z",
        "agent": "tester",
        "note": null
    });
    let recurrence = json!({
        "kind": "cut",
        "id": recurrence_id,
        "ts": recurrence_ts,
        "agent": "self-report",
        "text": resolved_text,
        "tags": [],
        "severity": "minor",
        "cwd": "/tmp"
    });
    std::fs::write(
        &verify_file,
        format!("{resolved}\n{resolution}\n{recurrence}\n"),
    )
    .unwrap();

    let verify = verify_success(&run_file(&verify_file, &["verify"]), 1);
    assert_eq!(verify.data["recurrences"][0]["source"], "hook");
}

#[test]
fn legacy_migration_inputs_are_ignored_without_warnings() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("repo");
    let nested = root.join("nested");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::create_dir(root.join(".git")).unwrap();
    std::fs::write(root.join(".papercuts.jsonl"), "").unwrap();
    let canonical_root = root.canonicalize().unwrap();

    let output: SuccessEnvelope<AddData> = success(
        &command()
            .current_dir(&nested)
            .env_remove("BLOTTER_FILE")
            .env_remove("BLOTTER_AGENT")
            .env("PAPERCUTS_FILE", temp.path().join("legacy-env.jsonl"))
            .env("PAPERCUTS_AGENT", "legacy-agent")
            .env("PAPERCUTS_NOW", NOW)
            .args(["add", "stale environment", "--dry-run"])
            .output()
            .unwrap(),
    );
    assert_eq!(output.data.record.cut_agent(), "unknown");
    assert_eq!(output.data.record.cut_ts(), "2026-07-09T18:30:00.123Z");
    assert_eq!(
        output.meta.file.as_deref(),
        Some(canonical_root.join(".blotter.jsonl").to_str().unwrap())
    );
    assert_eq!(output.meta.warnings, ["dry run; no record appended"]);
}

#[test]
fn legacy_pc_records_still_fold_and_list() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("legacy.jsonl");
    let cut = json!({
        "kind": "cut",
        "id": "pc_a1b2c3d4e5f6",
        "ts": "2026-07-09T00:00:00.000Z",
        "agent": "legacy",
        "text": "legacy cut",
        "tags": ["old"],
        "severity": "minor",
        "cwd": "/tmp",
        "repo": "/tmp/repo"
    });
    let dogear = json!({
        "kind": "dogear",
        "id": "pc_b1c2d3e4f5a6",
        "ts": "2026-07-09T00:00:00.000Z",
        "agent": "legacy",
        "text": "legacy dogear",
        "tags": ["old"],
        "cwd": "/tmp",
        "repo": "/tmp/repo"
    });
    std::fs::write(&file, format!("{cut}\n{dogear}\n")).unwrap();

    let listed: SuccessEnvelope<ListData> = success(&run_file(
        &file,
        &["list", "--kind", "all", "--status", "all"],
    ));
    assert_eq!(listed.data.items.len(), 2);
    assert_eq!(
        listed
            .data
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["pc_a1b2c3d4e5f6", "pc_b1c2d3e4f5a6"]
    );
}

#[test]
fn phase_2a_resolve_is_namespace_aware() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("mixed-namespaces.jsonl");
    let pc = "pc_a1b2c3d4e5f6";
    let bl = "bl_b1c2d3e4f5a6";
    let bare_pc = "pc_cafe12345678";
    let pc_collision = "pc_dead00000000";
    let bl_collision = "bl_dead11111111";
    let fixture = [pc, bl, bare_pc, pc_collision, bl_collision]
        .into_iter()
        .map(|id| {
            json!({
                "kind": "cut",
                "id": id,
                "ts": "2026-07-09T00:00:00.000Z",
                "agent": "legacy",
                "text": id,
                "tags": [],
                "severity": "minor",
                "cwd": "/tmp",
                "repo": null
            })
            .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&file, format!("{fixture}\n")).unwrap();

    let pc_input = pc.to_ascii_uppercase();
    let resolved_pc: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &["resolve", &pc_input, "--agent", "fixer"],
    ));
    assert!(resolved_pc.data.changed);
    assert_eq!(resolved_pc.data.records.len(), 1);
    assert_eq!(resolved_pc.data.records[0].id, pc);

    let bl_input = bl.to_ascii_uppercase();
    let resolved_bl: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &["resolve", &bl_input, "--agent", "fixer"],
    ));
    assert!(resolved_bl.data.changed);
    assert_eq!(resolved_bl.data.records.len(), 1);
    assert_eq!(resolved_bl.data.records[0].id, bl);

    let resolved_bare: SuccessEnvelope<ResolveData> =
        success(&run_file(&file, &["resolve", "cafe", "--agent", "fixer"]));
    assert!(resolved_bare.data.changed);
    assert_eq!(resolved_bare.data.records.len(), 1);
    assert_eq!(resolved_bare.data.records[0].id, bare_pc);

    let ambiguous = error(
        &run_file(&file, &["resolve", "dead", "--agent", "fixer"]),
        65,
        "ambiguous_id",
    );
    assert_eq!(
        ambiguous.error.details["candidates"],
        json!([bl_collision, pc_collision])
    );
}

#[test]
fn doctor_accepts_legacy_pc_records_without_a_migration_field() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("mixed-log.jsonl");
    add(&file, "current record");
    let legacy_cut = json!({
        "kind": "cut",
        "id": "pc_000000000000",
        "ts": "2026-07-09T00:00:00.000Z",
        "agent": "legacy",
        "text": "legacy cut",
        "tags": [],
        "severity": "minor",
        "cwd": "/tmp",
        "repo": null
    });
    let legacy_dogear = json!({
        "kind": "dogear",
        "id": "pc_11111111111111111111",
        "ts": "2026-07-09T00:00:00.000Z",
        "agent": "legacy",
        "text": "legacy dogear",
        "tags": [],
        "cwd": "/tmp",
        "repo": null
    });
    let mut writer = OpenOptions::new().append(true).open(&file).unwrap();
    writeln!(writer, "{legacy_cut}").unwrap();
    writeln!(writer, "{legacy_dogear}").unwrap();
    drop(writer);

    let output = run_file(&file, &["doctor"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let doctor: SuccessEnvelope<DoctorData> = serde_json::from_slice(&output.stdout).unwrap();
    assert!(doctor.data.healthy, "findings: {:?}", doctor.data.findings);
    assert!(doctor.data.findings.is_empty());
    assert_eq!(doctor.data.checked_lines, 3);

    let schema: SuccessEnvelope<Value> = success(&run(&["schema"]));
    assert_eq!(
        schema.data["commands"]["doctor"]["output"],
        "{healthy,findings:[{line,kind,message,fixable}],checked_lines,fix?:{changed,applied:[{line,kind,action}],backup?,quarantine?,restore_hint?,dry_run}}"
    );
    assert_eq!(schema.data["commands"]["doctor"]["read_only"], true);
    assert_eq!(
        schema.data["commands"]["doctor"]["fix"]["destructive"],
        true
    );
    assert_eq!(schema.data["commands"]["doctor"]["fix"]["read_only"], false);
    assert_eq!(
        schema.data["id"]["accepted_prefixes"],
        json!(["bl_", "pc_"])
    );
}
