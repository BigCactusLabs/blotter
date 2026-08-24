use crate::common::*;

#[test]
fn resolve_single_id_always_returns_a_records_array() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let added = add(&file, "resolve one");

    let resolved: Value =
        serde_json::from_slice(&run_file(&file, &["resolve", added.data.record.cut_id()]).stdout)
            .unwrap();
    let data = resolved["data"].as_object().unwrap();
    assert!(data.get("record").is_none());
    let records = data["records"].as_array().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["id"], added.data.record.cut_id());
}

#[test]
fn resolve_records_structured_graduation_fields() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let cut = add(&file, "graduate this fix");

    let resolved: SuccessEnvelope<Value> = success(&run_file(
        &file,
        &[
            "resolve",
            cut.data.record.cut_id(),
            "--task",
            "TASK-2",
            "--pr",
            "https://example.com/dogear/pull/2",
            "--commit",
            "d34db33fd34db33f",
            "--note",
            "graduated",
        ],
    ));
    let resolution = &resolved.data["records"][0]["resolution"];
    assert_eq!(resolution["note"], "graduated");
    assert_eq!(resolution["task"], "TASK-2");
    assert_eq!(resolution["pr"], "https://example.com/dogear/pull/2");
    assert_eq!(resolution["commit"], "d34db33fd34db33f");

    let listed: SuccessEnvelope<Value> =
        success(&run_file(&file, &["list", "--status", "resolved"]));
    assert_eq!(listed.data["items"][0]["resolution"], *resolution);

    for flag in ["--task", "--pr", "--commit"] {
        let output = command()
            .arg("--file")
            .arg(&file)
            .arg("resolve")
            .arg(cut.data.record.cut_id())
            .arg(flag)
            .arg(" \t")
            .output()
            .unwrap();
        error(&output, 65, "invalid_input");
    }

    let doctor: SuccessEnvelope<DoctorData> = success(&run_file(&file, &["doctor"]));
    assert!(doctor.data.healthy);
}

#[test]
fn resolve_records_dogear_publish_and_drop_lifecycle() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let publish: SuccessEnvelope<Value> = success(&run_file(
        &file,
        &["dogear", "publish this idea", "--agent", "researcher"],
    ));
    let publish_id = publish.data["record"]["id"].as_str().unwrap().to_owned();
    let published: SuccessEnvelope<Value> = success(&run_file(
        &file,
        &[
            "resolve",
            &publish_id,
            "--url",
            "https://example.com/posts/blotter",
        ],
    ));
    assert_eq!(
        published.data["records"][0]["resolution"]["url"],
        "https://example.com/posts/blotter"
    );

    let drop: SuccessEnvelope<Value> = success(&run_file(
        &file,
        &["dogear", "drop this idea", "--agent", "researcher"],
    ));
    let drop_id = drop.data["record"]["id"].as_str().unwrap().to_owned();
    let dropped: SuccessEnvelope<Value> =
        success(&run_file(&file, &["resolve", &drop_id, "--dropped"]));
    assert_eq!(dropped.data["records"][0]["resolution"]["dropped"], true);

    let doctor: SuccessEnvelope<DoctorData> = success(&run_file(&file, &["doctor"]));
    assert!(doctor.data.healthy);
}

#[test]
fn resolve_help_marks_dogear_only_lifecycle_flags() {
    let help = run(&["resolve", "--help"]);
    assert!(help.status.success());
    assert!(help.stderr.is_empty());
    let stdout = String::from_utf8_lossy(&help.stdout);
    assert!(stdout.contains("Published destination (dogear records only)"));
    assert!(stdout.contains("Mark dropped (dogear records only)"));
}

#[test]
fn resolve_dogear_lifecycle_flags_reject_cuts_without_partial_batches() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let cut = add(&file, "cut cannot publish");
    let before = std::fs::read(&file).unwrap();
    error(
        &run_file(
            &file,
            &[
                "resolve",
                cut.data.record.cut_id(),
                "--url",
                "https://example.com/posts/blotter",
            ],
        ),
        2,
        "invalid_argument",
    );
    assert_eq!(std::fs::read(&file).unwrap(), before);

    let dogear: SuccessEnvelope<Value> = success(&run_file(
        &file,
        &[
            "dogear",
            "dogear cannot be partially dropped",
            "--agent",
            "writer",
        ],
    ));
    let dogear_id = dogear.data["record"]["id"].as_str().unwrap().to_owned();
    let before = std::fs::read(&file).unwrap();
    error(
        &run_file(
            &file,
            &["resolve", cut.data.record.cut_id(), &dogear_id, "--dropped"],
        ),
        2,
        "invalid_argument",
    );
    assert_eq!(std::fs::read(&file).unwrap(), before);

    let listed: SuccessEnvelope<Value> = success(&run_file(
        &file,
        &["list", "--kind", "dogear", "--status", "all"],
    ));
    assert_eq!(listed.data["items"].as_array().unwrap().len(), 1);
    assert_eq!(listed.data["items"][0]["id"], dogear_id);
    assert_eq!(listed.data["items"][0]["status"], "open");

    let conflict = run_file(
        &file,
        &[
            "resolve",
            &dogear_id,
            "--url",
            "https://example.com/posts/blotter",
            "--dropped",
        ],
    );
    assert_eq!(conflict.status.code(), Some(2));
    assert!(conflict.stdout.is_empty());
    assert_eq!(std::fs::read(&file).unwrap(), before);
}

#[test]
fn resolve_without_new_flags_omits_graduation_and_lifecycle_keys() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let cut = add(&file, "ordinary resolve response");

    let resolved: SuccessEnvelope<Value> =
        success(&run_file(&file, &["resolve", cut.data.record.cut_id()]));
    let resolution = resolved.data["records"][0]["resolution"]
        .as_object()
        .unwrap();
    for key in ["task", "pr", "commit", "url", "dropped", "amended"] {
        assert!(!resolution.contains_key(key));
    }
    assert_eq!(
        resolved.data["records"][0]["resolution"],
        json!({"agent":"unknown","note":null,"ts":"2026-07-09T18:30:00.123Z"})
    );
    let event = std::fs::read_to_string(&file)
        .unwrap()
        .lines()
        .last()
        .unwrap()
        .to_owned();
    let event_json: Value = serde_json::from_str(&event).unwrap();
    let event_json = event_json.as_object().unwrap();
    for key in ["task", "pr", "commit", "url", "dropped", "amend"] {
        assert!(!event_json.contains_key(key));
    }
    assert_eq!(
        event,
        format!(
            "{{\"kind\":\"resolve\",\"id\":\"{}\",\"ts\":\"2026-07-09T18:30:00.123Z\",\"agent\":\"unknown\",\"note\":null}}",
            cut.data.record.cut_id()
        )
    );
}

#[test]
fn resolve_amend_replaces_base_resolution_and_preserves_history() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let cut = add(&file, "correct this resolution");
    let id = cut.data.record.cut_id().to_owned();
    let _: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &[
            "resolve", &id, "--agent", "base", "--note", "original", "--task", "TASK-12",
        ],
    ));
    let before_amend = std::fs::read(&file).unwrap();

    let amended: SuccessEnvelope<Value> = success(&run_file(
        &file,
        &[
            "resolve",
            &id,
            "--amend",
            "--agent",
            "corrector",
            "--note",
            "corrected",
        ],
    ));
    let resolution = &amended.data["records"][0]["resolution"];
    assert_eq!(resolution["agent"], "corrector");
    assert_eq!(resolution["note"], "corrected");
    assert_eq!(resolution["amended"], true);
    assert!(resolution.get("task").is_none());

    let after_amend = std::fs::read(&file).unwrap();
    assert!(after_amend.starts_with(&before_amend));
    let after_text = String::from_utf8(after_amend).unwrap();
    assert_eq!(after_text.lines().count(), 3);
    let lines: Vec<_> = after_text.lines().collect();
    assert!(lines[1].contains("\"note\":\"original\""));
    assert_eq!(
        serde_json::from_str::<Value>(lines[2]).unwrap()["amend"],
        true
    );

    let listed: SuccessEnvelope<Value> =
        success(&run_file(&file, &["list", "--status", "resolved"]));
    assert_eq!(listed.data["items"][0]["resolution"], *resolution);
}

#[test]
fn resolve_amend_latest_event_supersedes_prior_amend() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let cut = add(&file, "latest correction wins");
    let id = cut.data.record.cut_id().to_owned();
    let _: SuccessEnvelope<ResolveData> =
        success(&run_file(&file, &["resolve", &id, "--note", "base"]));
    let _: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &[
            "resolve",
            &id,
            "--amend",
            "--agent",
            "first",
            "--note",
            "first correction",
        ],
    ));
    let _: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &[
            "resolve",
            &id,
            "--amend",
            "--agent",
            "second",
            "--note",
            "second correction",
        ],
    ));

    let listed: SuccessEnvelope<ListData> =
        success(&run_file(&file, &["list", "--status", "resolved"]));
    let resolution = listed.data.items[0].resolution.as_ref().unwrap();
    assert_eq!(resolution.agent, "second");
    assert_eq!(resolution.note.as_deref(), Some("second correction"));
    assert!(resolution.amended);
    assert_eq!(std::fs::read_to_string(&file).unwrap().lines().count(), 4);
}

#[test]
fn resolve_amend_requires_a_resolved_record_and_changing_fields() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let cut = add(&file, "unresolved correction");
    let id = cut.data.record.cut_id().to_owned();
    let before = std::fs::read(&file).unwrap();

    let unresolved = error(
        &run_file(&file, &["resolve", &id, "--amend", "--note", "corrected"]),
        65,
        "invalid_input",
    );
    assert!(
        unresolved
            .error
            .suggested_fix
            .contains("Resolve each record without --amend first")
    );
    assert_eq!(std::fs::read(&file).unwrap(), before);

    error(
        &run_file(&file, &["resolve", &id, "--amend"]),
        65,
        "invalid_input",
    );
    assert_eq!(std::fs::read(&file).unwrap(), before);

    error(
        &run_file(
            &file,
            &["resolve", "deadbeef", "--amend", "--note", "corrected"],
        ),
        66,
        "not_found",
    );
    assert_eq!(std::fs::read(&file).unwrap(), before);
}

#[test]
fn resolve_amend_batches_require_every_record_to_be_resolved() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let first = add(&file, "first batch correction")
        .data
        .record
        .cut_id()
        .to_owned();
    let second = add(&file, "second batch correction")
        .data
        .record
        .cut_id()
        .to_owned();
    let _: SuccessEnvelope<ResolveData> =
        success(&run_file(&file, &["resolve", &first, "--note", "base"]));
    let before = std::fs::read(&file).unwrap();

    error(
        &run_file(
            &file,
            &[
                "resolve",
                &first,
                &second,
                "--amend",
                "--note",
                "batch correction",
            ],
        ),
        65,
        "invalid_input",
    );
    assert_eq!(std::fs::read(&file).unwrap(), before);

    let _: SuccessEnvelope<ResolveData> =
        success(&run_file(&file, &["resolve", &second, "--note", "base"]));
    let amended: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &[
            "resolve",
            &first,
            &second,
            "--amend",
            "--note",
            "batch correction",
        ],
    ));
    assert!(amended.data.changed);
    assert!(
        amended
            .data
            .records
            .iter()
            .all(
                |record| record.resolution.as_ref().is_some_and(|resolution| {
                    resolution.amended && resolution.note.as_deref() == Some("batch correction")
                })
            )
    );
}

#[test]
fn resolve_amend_supports_dogear_lifecycle_fields() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let published: SuccessEnvelope<Value> = success(&run_file(
        &file,
        &["dogear", "published correction", "--agent", "writer"],
    ));
    let published_id = published.data["record"]["id"].as_str().unwrap().to_owned();
    let _: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &[
            "resolve",
            &published_id,
            "--url",
            "https://example.com/original",
        ],
    ));
    let amended_url: SuccessEnvelope<Value> = success(&run_file(
        &file,
        &[
            "resolve",
            &published_id,
            "--amend",
            "--url",
            "https://example.com/corrected",
        ],
    ));
    assert_eq!(
        amended_url.data["records"][0]["resolution"]["url"],
        "https://example.com/corrected"
    );
    assert_eq!(
        amended_url.data["records"][0]["resolution"]["amended"],
        true
    );

    let dropped: SuccessEnvelope<Value> = success(&run_file(
        &file,
        &["dogear", "dropped correction", "--agent", "writer"],
    ));
    let dropped_id = dropped.data["record"]["id"].as_str().unwrap().to_owned();
    let _: SuccessEnvelope<ResolveData> =
        success(&run_file(&file, &["resolve", &dropped_id, "--dropped"]));
    let amended_dropped: SuccessEnvelope<Value> = success(&run_file(
        &file,
        &["resolve", &dropped_id, "--amend", "--dropped"],
    ));
    assert_eq!(
        amended_dropped.data["records"][0]["resolution"]["dropped"],
        true
    );
    assert_eq!(
        amended_dropped.data["records"][0]["resolution"]["amended"],
        true
    );

    let cut = add(&file, "cut cannot amend a URL");
    let cut_id = cut.data.record.cut_id().to_owned();
    let _: SuccessEnvelope<ResolveData> =
        success(&run_file(&file, &["resolve", &cut_id, "--note", "base"]));
    let before = std::fs::read(&file).unwrap();
    error(
        &run_file(
            &file,
            &[
                "resolve",
                &cut_id,
                "--amend",
                "--url",
                "https://example.com/cuts",
            ],
        ),
        2,
        "invalid_argument",
    );
    assert_eq!(std::fs::read(&file).unwrap(), before);
}

#[test]
fn resolve_amend_dry_run_materializes_without_appending() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let cut = add(&file, "preview correction");
    let id = cut.data.record.cut_id().to_owned();
    let _: SuccessEnvelope<ResolveData> =
        success(&run_file(&file, &["resolve", &id, "--note", "base"]));
    let before = std::fs::read(&file).unwrap();

    let preview: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &["resolve", &id, "--amend", "--note", "preview", "--dry-run"],
    ));
    assert!(!preview.data.changed);
    let resolution = preview.data.records[0].resolution.as_ref().unwrap();
    assert_eq!(resolution.note.as_deref(), Some("preview"));
    assert!(resolution.amended);
    assert_eq!(
        preview.meta.warnings,
        ["dry run; no resolve event appended"]
    );
    assert_eq!(std::fs::read(&file).unwrap(), before);
}

#[test]
fn amended_list_output_is_byte_deterministic_under_blotter_now() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let cut = add(&file, "deterministic correction");
    let id = cut.data.record.cut_id().to_owned();
    let _: SuccessEnvelope<ResolveData> =
        success(&run_file(&file, &["resolve", &id, "--note", "base"]));
    let _: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &["resolve", &id, "--amend", "--note", "corrected"],
    ));

    let first = run_file(&file, &["list", "--status", "resolved"]);
    let second = run_file(&file, &["list", "--status", "resolved"]);
    assert!(first.status.success());
    assert!(second.status.success());
    assert!(first.stderr.is_empty());
    assert!(second.stderr.is_empty());
    assert_eq!(first.stdout, second.stdout);
    let listed: SuccessEnvelope<Value> = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(listed.data["items"][0]["resolution"]["amended"], true);
}

// Reverses the divergence this test previously pinned: doctor now fails on an
// amend that has no base resolve anywhere in the log (design doc r35).
#[test]
fn orphan_resolve_amends_warn_in_the_fold_and_fail_doctor() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let cut = add(&file, "orphan amend fixture");
    let id = cut.data.record.cut_id().to_owned();
    let original = std::fs::read_to_string(&file).unwrap();
    let orphan_amend = json!({
        "kind": "resolve",
        "id": id,
        "ts": "2026-07-09T18:30:00.123Z",
        "agent": "fixture",
        "note": "orphan amend",
        "amend": true
    });
    std::fs::write(&file, format!("{original}{orphan_amend}\n")).unwrap();

    let listed: SuccessEnvelope<ListData> = success(&run_file(&file, &["list", "--status", "all"]));
    assert_eq!(listed.data.items[0].status, ItemStatus::Open);
    assert_eq!(listed.meta.warnings, ["skipped 1 orphan resolve"]);
    let doctor = doctor_response(&run_file(&file, &["doctor"]), 1);
    assert!(!doctor.data.healthy);
    assert_eq!(doctor.data.findings.len(), 1);
    assert_eq!(doctor.data.findings[0].kind, "orphan_resolve");
}

#[test]
fn resolve_reports_materialized_orphan_amend_after_base_append() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let cut = add(&file, "activate stale amend");
    let id = cut.data.record.cut_id().to_owned();
    let original = std::fs::read_to_string(&file).unwrap();
    let orphan_amend = json!({
        "kind": "resolve",
        "id": id,
        "ts": "2026-07-09T18:31:00.000Z",
        "agent": "stale-amend",
        "note": "stale correction",
        "task": "TASK-STALE",
        "amend": true
    });
    std::fs::write(&file, format!("{original}{orphan_amend}\n")).unwrap();

    let resolved: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &[
            "resolve",
            &id,
            "--agent",
            "base",
            "--note",
            "base resolution",
        ],
    ));
    let listed: SuccessEnvelope<ListData> =
        success(&run_file(&file, &["list", "--status", "resolved"]));

    assert_eq!(resolved.data.records, listed.data.items);
    let resolution = resolved.data.records[0].resolution.as_ref().unwrap();
    assert_eq!(resolution.agent, "stale-amend");
    assert_eq!(resolution.note.as_deref(), Some("stale correction"));
    assert_eq!(resolution.task.as_deref(), Some("TASK-STALE"));
    assert!(resolution.amended);
}

#[test]
fn multiple_orphan_amends_for_one_record_count_once() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let cut = add(&file, "orphan amend count fixture");
    let id = cut.data.record.cut_id().to_owned();
    let original = std::fs::read_to_string(&file).unwrap();
    let first = json!({
        "kind": "resolve",
        "id": id,
        "ts": "2026-07-09T18:30:00.123Z",
        "agent": "fixture",
        "note": "first orphan amend",
        "amend": true
    });
    let second = json!({
        "kind": "resolve",
        "id": id,
        "ts": "2026-07-09T18:31:00.123Z",
        "agent": "fixture",
        "note": "second orphan amend",
        "amend": true
    });
    std::fs::write(&file, format!("{original}{first}\n{second}\n")).unwrap();

    let listed: SuccessEnvelope<ListData> = success(&run_file(&file, &["list", "--status", "all"]));
    assert_eq!(listed.meta.warnings, ["skipped 1 orphan resolve"]);

    // Deliberate granularity divergence: the fold warns once per unresolved ID,
    // while doctor is line-granular and reports one finding per orphan line.
    let doctor = doctor_response(&run_file(&file, &["doctor"]), 1);
    assert!(!doctor.data.healthy);
    let orphans: Vec<_> = doctor
        .data
        .findings
        .iter()
        .filter(|finding| finding.kind == "orphan_resolve")
        .collect();
    assert_eq!(orphans.len(), 2, "findings: {:?}", doctor.data.findings);
    assert_eq!(orphans[0].line + 1, orphans[1].line);

    // The base append activates the latest orphan amend, and resolve reports it
    // from the deciding fold without reading the log a second time.
    let resolved: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &["resolve", &id, "--agent", "base", "--note", "base"],
    ));
    let listed: SuccessEnvelope<ListData> =
        success(&run_file(&file, &["list", "--status", "resolved"]));
    assert_eq!(resolved.data.records, listed.data.items);
    let resolution = resolved.data.records[0].resolution.as_ref().unwrap();
    assert_eq!(resolution.note.as_deref(), Some("second orphan amend"));
    assert!(resolution.amended);
}

#[test]
fn schema_documents_resolve_graduation_and_dogear_lifecycle() {
    let schema: SuccessEnvelope<Value> = success(&run(&["schema"]));
    let flags = schema.data["commands"]["resolve"]["flags"]
        .as_object()
        .unwrap();
    assert!(flags.contains_key("--task"));
    assert!(flags.contains_key("--pr"));
    assert!(flags.contains_key("--commit"));
    assert!(flags["--url"].as_str().unwrap().contains("dogear-only"));
    assert!(
        flags["--dropped"]
            .as_str()
            .unwrap()
            .contains("conflicts with --url")
    );
    assert!(schema.data["records"]["resolve"].get("task").is_some());
    assert!(schema.data["records"]["resolve"].get("url").is_some());
    assert!(
        schema.data["records"]["list_item"]["resolution"]
            .as_str()
            .unwrap()
            .contains("dropped")
    );
}

#[test]
fn schema_documents_resolve_amend_history() {
    let schema: SuccessEnvelope<Value> = success(&run(&["schema"]));
    let resolve = &schema.data["commands"]["resolve"];
    assert!(
        resolve["flags"]["--amend"]
            .as_str()
            .unwrap()
            .contains("already-resolved")
    );
    assert!(
        resolve["amend_fold"]
            .as_str()
            .unwrap()
            .contains("first non-amend resolve")
    );
    assert!(
        resolve["amend_fold"]
            .as_str()
            .unwrap()
            .contains("latest amend")
    );
    assert!(schema.data["records"]["resolve"].get("amend").is_some());
    assert!(
        schema.data["records"]["list_item"]["resolution"]
            .as_str()
            .unwrap()
            .contains("amended")
    );
}

#[test]
fn resolve_prefix_errors_and_idempotence_are_structured() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let added = add(&file, "resolve me");
    let id = added.data.record.cut_id();
    let prefix = &id[3..7];
    let first: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &["resolve", &prefix.to_ascii_uppercase(), "--agent", "fixer"],
    ));
    assert!(first.data.changed);
    let second: SuccessEnvelope<ResolveData> =
        success(&run_file(&file, &["resolve", id, "--agent", "fixer"]));
    assert!(!second.data.changed);
    assert_eq!(second.meta.warnings, ["already resolved"]);

    error(&run_file(&file, &["resolve", "abc"]), 2, "invalid_argument");
    let unknown = error(&run_file(&file, &["resolve", "deadbeef"]), 66, "not_found");
    assert_eq!(unknown.error.message, "no cut matches ID prefix 'deadbeef'");
    assert_eq!(
        unknown.error.suggested_fix,
        "Run `blotter list --status all --include-auto` and retry with a listed ID."
    );

    let missing = temp.path().join("missing.jsonl");
    let missing = error(
        &run_file(&missing, &["resolve", "deadbeef"]),
        66,
        "not_found",
    );
    assert_eq!(
        missing.error.message,
        format!(
            "blotter file not found: {}",
            temp.path().join("missing.jsonl").display()
        )
    );
    assert_eq!(
        missing.error.suggested_fix,
        "Run `blotter add` to create the file or pass an existing --file PATH."
    );

    let ambiguous = temp.path().join("ambiguous.jsonl");
    let lines = ["bl_abcd00000000", "bl_abcd11111111"]
        .map(|id| {
            json!({"kind":"cut","id":id,"ts":"2026-07-09T00:00:00.000Z","agent":"a","text":id,"tags":[],"severity":"minor","cwd":"/tmp","repo":null}).to_string()
        })
        .join("\n")
        + "\n";
    std::fs::write(&ambiguous, lines).unwrap();
    let envelope = error(
        &run_file(&ambiguous, &["resolve", "abcd"]),
        65,
        "ambiguous_id",
    );
    assert_eq!(
        envelope.error.details["candidates"],
        json!(["bl_abcd00000000", "bl_abcd11111111"])
    );
}

#[test]
fn multi_resolve_is_atomic_deterministic_and_idempotent() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let first = add(&file, "multi first").data.record.cut_id().to_owned();
    let second = add(&file, "multi second").data.record.cut_id().to_owned();
    let before = std::fs::read(&file).unwrap();

    let invalid = run_file(&file, &["resolve", &first, "deadbeef", "--agent", "fixer"]);
    error(&invalid, 66, "not_found");
    assert_eq!(std::fs::read(&file).unwrap(), before);

    let resolved: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &[
            "resolve", &second, &first, "--agent", "fixer", "--note", "batch",
        ],
    ));
    assert!(resolved.data.changed);
    assert_eq!(resolved.data.records.len(), 2);
    let mut expected = vec![first.clone(), second.clone()];
    expected.sort();
    assert_eq!(
        resolved
            .data
            .records
            .iter()
            .map(|record| record.id.clone())
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap().lines().count(), 4);

    let events: Vec<Value> = std::fs::read_to_string(&file)
        .unwrap()
        .lines()
        .skip(2)
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        events
            .iter()
            .map(|event| event["kind"].as_str())
            .collect::<Vec<_>>(),
        [Some("resolve"), Some("resolve")]
    );
    assert_eq!(
        events
            .iter()
            .map(|event| event["id"].as_str())
            .collect::<Vec<_>>(),
        expected
            .iter()
            .map(|id| Some(id.as_str()))
            .collect::<Vec<_>>()
    );
    for event in &events {
        assert_eq!(
            event
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["kind", "id", "ts", "agent", "note"]
        );
        assert_eq!(event["ts"], "2026-07-09T18:30:00.123Z");
        assert_eq!(event["agent"], "fixer");
        assert_eq!(event["note"], "batch");
    }
    let listed: SuccessEnvelope<ListData> =
        success(&run_file(&file, &["list", "--status", "resolved"]));
    assert_eq!(listed.data.items.len(), 2);
    assert!(listed.data.items.iter().all(|item| {
        item.resolution.as_ref().is_some_and(|resolution| {
            resolution.agent == "fixer" && resolution.note.as_deref() == Some("batch")
        })
    }));

    let duplicate: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &["resolve", &first, &first, "--agent", "fixer"],
    ));
    assert!(!duplicate.data.changed);
    assert_eq!(duplicate.data.records.len(), 1);
    assert_eq!(duplicate.meta.warnings, ["already resolved"]);
    assert_eq!(std::fs::read_to_string(&file).unwrap().lines().count(), 4);
}

#[test]
fn mixed_multi_resolve_warns_with_sorted_already_resolved_ids() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let first = add(&file, "mixed first").data.record.cut_id().to_owned();
    let second = add(&file, "mixed second").data.record.cut_id().to_owned();
    let _: SuccessEnvelope<ResolveData> =
        success(&run_file(&file, &["resolve", &first, "--agent", "fixer"]));

    let mixed: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &["resolve", &second, &first, "--agent", "fixer"],
    ));
    assert!(mixed.data.changed);
    assert_eq!(
        mixed.meta.warnings,
        [format!("already resolved: 1 ID ({first})")]
    );

    let all: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &["resolve", &first, &second, "--agent", "fixer"],
    ));
    assert!(!all.data.changed);
    assert_eq!(all.meta.warnings, ["already resolved"]);
}

#[test]
fn multi_resolve_with_ambiguous_prefix_is_atomic_and_returns_sorted_candidates() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let valid = add(&file, "valid multi resolve")
        .data
        .record
        .cut_id()
        .to_owned();
    let ambiguous = ["bl_abcd11111111", "bl_abcd00000000"]
        .map(|id| {
            json!({"kind":"cut","id":id,"ts":"2026-07-09T00:00:00.000Z","agent":"a","text":id,"tags":[],"severity":"minor","cwd":"/tmp","repo":null}).to_string()
        })
        .join("\n");
    let mut log = OpenOptions::new().append(true).open(&file).unwrap();
    writeln!(log, "{ambiguous}").unwrap();
    drop(log);
    let before = std::fs::read(&file).unwrap();

    let envelope = error(
        &run_file(&file, &["resolve", &valid, "abcd"]),
        65,
        "ambiguous_id",
    );
    assert_eq!(
        envelope.error.details["candidates"],
        json!(["bl_abcd00000000", "bl_abcd11111111"])
    );
    assert_eq!(std::fs::read(&file).unwrap(), before);
}

#[test]
fn multi_resolve_heals_a_torn_tail_and_keeps_first_resolution() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let first = add(&file, "first torn batch")
        .data
        .record
        .cut_id()
        .to_owned();
    let second = add(&file, "second torn batch")
        .data
        .record
        .cut_id()
        .to_owned();
    let mut torn = OpenOptions::new().append(true).open(&file).unwrap();
    write!(torn, "{{\"kind\":").unwrap();
    drop(torn);
    let _: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &[
            "resolve", &second, &first, "--agent", "fixer", "--note", "first",
        ],
    ));
    let log = std::fs::read_to_string(&file).unwrap();
    assert!(log.ends_with('\n'));
    let listed: SuccessEnvelope<ListData> =
        success(&run_file(&file, &["list", "--status", "resolved"]));
    assert_eq!(listed.data.items.len(), 2);
    assert!(listed.data.items.iter().all(|item| {
        item.resolution
            .as_ref()
            .is_some_and(|resolution| resolution.note.as_deref() == Some("first"))
    }));
    let first_resolution = json!({"kind":"resolve","id":first,"ts":"2026-07-09T18:30:00.123Z","agent":"later","note":"later"});
    std::fs::write(&file, format!("{log}{first_resolution}\n")).unwrap();
    let listed: SuccessEnvelope<ListData> =
        success(&run_file(&file, &["list", "--status", "resolved"]));
    let first_item = listed
        .data
        .items
        .iter()
        .find(|item| item.id == first)
        .unwrap();
    assert_eq!(
        first_item.resolution.as_ref().unwrap().note.as_deref(),
        Some("first")
    );
}

#[test]
fn concurrent_multi_resolves_share_one_critical_section() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let first = add(&file, "concurrent multi first")
        .data
        .record
        .cut_id()
        .to_owned();
    let second = add(&file, "concurrent multi second")
        .data
        .record
        .cut_id()
        .to_owned();
    let barrier = Arc::new(Barrier::new(4));
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let file = file.clone();
            let first = first.clone();
            let second = second.clone();
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let output = run_file(&file, &["resolve", &first, &second, "--agent", "race"]);
                let envelope: SuccessEnvelope<ResolveData> = success(&output);
                envelope.data.changed
            })
        })
        .collect();
    let changed = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .filter(|changed| *changed)
        .count();
    assert_eq!(changed, 1);
    assert_eq!(std::fs::read_to_string(&file).unwrap().lines().count(), 4);
}

#[test]
fn resolve_amend_with_the_latest_timestamp_wins_over_file_order() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let cut = add(&file, "merge reordered amends");
    let id = cut.data.record.cut_id().to_owned();
    // A union merge concatenates branches in branch order, so the older amend
    // can land last in the byte stream. Timestamp decides, not file position.
    append_lines(
        &file,
        &[
            resolve_line(&id, "2026-07-10T00:00:00.000Z", "base", false),
            resolve_line(&id, "2026-07-12T00:00:00.000Z", "later", true),
            resolve_line(&id, "2026-07-11T00:00:00.000Z", "earlier", true),
        ],
    );

    let listed: SuccessEnvelope<ListData> = success(&run_file(&file, &["list", "--status", "all"]));
    let resolution = listed.data.items[0].resolution.as_ref().unwrap();
    assert_eq!(resolution.note.as_deref(), Some("later"));
    assert_eq!(resolution.ts, "2026-07-12T00:00:00.000Z");
    assert!(resolution.amended);
}

#[test]
fn resolve_amends_sharing_a_timestamp_keep_the_last_in_file_order() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let cut = add(&file, "frozen clock amends");
    let id = cut.data.record.cut_id().to_owned();
    // Two amends under one frozen BLOTTER_NOW are reachable from the CLI; the
    // comparison is >=, so the last one in file order still wins the tie.
    append_lines(
        &file,
        &[
            resolve_line(&id, "2026-07-10T00:00:00.000Z", "base", false),
            resolve_line(&id, "2026-07-11T00:00:00.000Z", "first", true),
            resolve_line(&id, "2026-07-11T00:00:00.000Z", "second", true),
        ],
    );

    let listed: SuccessEnvelope<ListData> = success(&run_file(&file, &["list", "--status", "all"]));
    let resolution = listed.data.items[0].resolution.as_ref().unwrap();
    assert_eq!(resolution.note.as_deref(), Some("second"));
}

#[test]
fn resolve_amend_response_reports_the_timestamp_winning_amend() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let id = add_at(
        &file,
        "2026-01-01T00:00:00.000Z",
        "backdated amend case",
        &[],
    )
    .data
    .record
    .cut_id()
    .to_string();
    resolve_at(
        &file,
        "2026-02-01T00:00:00.000Z",
        &id,
        &["--agent", "tester", "--note", "base"],
    );
    resolve_at(
        &file,
        "2026-05-01T00:00:00.000Z",
        &id,
        &["--agent", "tester", "--amend", "--note", "may amend"],
    );

    // A backdated amend appends, but the fold keeps the later stored amend. The
    // dry run predicts the same thing, so it cannot promise what apply will not do.
    let planned = resolve_at(
        &file,
        "2026-03-01T00:00:00.000Z",
        &id,
        &[
            "--agent",
            "tester",
            "--amend",
            "--note",
            "march amend",
            "--dry-run",
        ],
    );
    let planned = planned.data.records[0].resolution.as_ref().unwrap();
    assert_eq!(planned.note.as_deref(), Some("may amend"));
    assert_eq!(planned.ts, "2026-05-01T00:00:00.000Z");

    let applied = resolve_at(
        &file,
        "2026-03-01T00:00:00.000Z",
        &id,
        &["--agent", "tester", "--amend", "--note", "march amend"],
    );
    let applied = applied.data.records[0].resolution.as_ref().unwrap();
    assert_eq!(applied.note.as_deref(), Some("may amend"));
    assert_eq!(applied.ts, "2026-05-01T00:00:00.000Z");

    let listed = success::<ListData>(&run_file(&file, &["list", "--status", "all"]));
    let listed = listed.data.items[0].resolution.as_ref().unwrap();
    assert_eq!(listed.note.as_deref(), Some("may amend"));
    assert_eq!(listed.ts, "2026-05-01T00:00:00.000Z");

    // A later amend still wins, and an exact tie still falls to the appended
    // event as the last in file order.
    let later = resolve_at(
        &file,
        "2026-09-01T00:00:00.000Z",
        &id,
        &["--agent", "tester", "--amend", "--note", "september amend"],
    );
    assert_eq!(
        later.data.records[0]
            .resolution
            .as_ref()
            .unwrap()
            .note
            .as_deref(),
        Some("september amend")
    );
    let tie = resolve_at(
        &file,
        "2026-09-01T00:00:00.000Z",
        &id,
        &["--agent", "tester", "--amend", "--note", "tie amend"],
    );
    assert_eq!(
        tie.data.records[0]
            .resolution
            .as_ref()
            .unwrap()
            .note
            .as_deref(),
        Some("tie amend")
    );
    let listed = success::<ListData>(&run_file(&file, &["list", "--status", "all"]));
    assert_eq!(
        listed.data.items[0]
            .resolution
            .as_ref()
            .unwrap()
            .note
            .as_deref(),
        Some("tie amend")
    );
}
