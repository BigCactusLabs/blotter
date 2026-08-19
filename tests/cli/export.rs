use crate::common::*;

#[test]
fn export_otlp_json_golden_maps_all_statuses_without_evidence() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let records = [
        json!({
            "kind": "dogear", "id": "bl_72f5a7e2d02e", "ts": "2026-08-18T10:00:00.126Z",
            "agent": "dogear", "text": "idea only", "tags": [], "cwd": "docs",
            "evidence": "dogear evidence must not export"
        }),
        json!({
            "kind": "cut", "id": "bl_839a74dab188", "ts": "2026-08-18T10:00:00.125Z",
            "agent": "carol", "text": "blocker report", "tags": [], "severity": "blocker",
            "cwd": "src/c"
        }),
        json!({
            "kind": "resolve", "id": "bl_839a74dab188", "ts": "2026-08-18T10:06:00.000Z",
            "agent": "resolver", "note": "private dropped note", "dropped": true
        }),
        json!({
            "kind": "cut", "id": "bl_e4189e9ff71d", "ts": "2026-08-18T10:00:00.124Z",
            "agent": "bob", "text": "major report", "tags": ["release"], "severity": "major",
            "cwd": "src/b"
        }),
        json!({
            "kind": "resolve", "id": "bl_e4189e9ff71d", "ts": "2026-08-18T10:05:00.000Z",
            "agent": "resolver", "note": "private resolved note"
        }),
        json!({
            "kind": "cut", "id": "bl_d39a975de893", "ts": "2026-08-18T10:00:00.123Z",
            "agent": "alice", "text": "minor report", "tags": ["alpha", "ops"], "severity": "minor",
            "cwd": "src/a",
            "evidence": {
                "cmd": "do-not-export command", "exit": 9,
                "stderr": "do-not-export stderr", "note": "do-not-export evidence note"
            }
        }),
    ];
    let mut input = records
        .into_iter()
        .map(|record| record.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    input.push('\n');
    input.push_str("not json\n");
    std::fs::write(&file, input).unwrap();
    let before = std::fs::read(&file).unwrap();

    let output = command()
        .env("BLOTTER_NOW", "2026-08-19T00:00:00Z")
        .arg("--file")
        .arg(&file)
        .args(["export", "--format", "otlp-json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        output.stdout,
        include_bytes!("../fixtures/export-otlp-json-golden.jsonl")
    );
    assert_eq!(std::fs::read(&file).unwrap(), before);

    let exported: Value = serde_json::from_slice(&output.stdout).unwrap();
    let records = exported["resourceLogs"][0]["scopeLogs"][0]["logRecords"]
        .as_array()
        .unwrap();
    assert_eq!(records.len(), 3);
    assert_eq!(
        records
            .iter()
            .map(|record| record["timeUnixNano"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "1787047200123000000",
            "1787047200124000000",
            "1787047200125000000",
        ]
    );
    assert_eq!(
        records
            .iter()
            .map(|record| record["severityNumber"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        [9, 13, 17]
    );
    assert_eq!(
        records
            .iter()
            .map(|record| record["severityText"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["INFO", "WARN", "ERROR"]
    );
    assert!(records.iter().all(|record| {
        record["eventName"] == "blotter.friction.reported"
            && record["timeUnixNano"].is_string()
            && record.get("traceId").is_none()
            && record.get("spanId").is_none()
            && record.get("schemaUrl").is_none()
    }));
    let attributes = records
        .iter()
        .map(|record| record["attributes"].as_array().unwrap())
        .collect::<Vec<_>>();
    assert!(
        attributes[0]
            .iter()
            .any(|attribute| attribute["key"] == "blotter.friction.status"
                && attribute["value"]["stringValue"] == "open")
    );
    assert!(
        attributes[1]
            .iter()
            .any(|attribute| attribute["key"] == "blotter.friction.status"
                && attribute["value"]["stringValue"] == "resolved")
    );
    assert!(
        attributes[2]
            .iter()
            .any(|attribute| attribute["key"] == "blotter.friction.status"
                && attribute["value"]["stringValue"] == "dropped")
    );
    assert!(
        attributes[1]
            .iter()
            .any(|attribute| attribute["key"] == "blotter.friction.resolved_ts")
    );
    assert!(
        attributes[0]
            .iter()
            .all(|attribute| attribute["key"] != "blotter.friction.resolved_ts")
    );
    assert!(
        attributes[2]
            .iter()
            .all(|attribute| attribute["key"] != "blotter.friction.resolved_ts")
    );

    let raw = String::from_utf8(output.stdout).unwrap();
    assert!(raw.contains("\"resourceLogs\""));
    assert!(raw.contains("\"timeUnixNano\""));
    assert!(!raw.contains("resource_logs"));
    assert!(!raw.contains("time_unix_nano"));
    for forbidden in [
        "bl_72f5a7e2d02e",
        "do-not-export command",
        "do-not-export stderr",
        "do-not-export evidence note",
        "private resolved note",
        "private dropped note",
        "\"cmd\"",
        "\"stderr\"",
        "\"note\"",
    ] {
        assert!(!raw.contains(forbidden), "export leaked {forbidden}");
    }
}

#[test]
fn export_otlp_json_hides_auto_captures_unless_requested() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let manual = add_at(
        &file,
        "2026-08-18T10:00:00Z",
        "manual export record",
        &["manual"],
    );
    let auto = add_at(
        &file,
        "2026-08-18T10:01:00Z",
        "auto export record",
        &["auto"],
    );

    let default = run_file(&file, &["export", "--format", "otlp-json"]);
    assert!(default.status.success());
    assert!(default.stderr.is_empty());
    let default: Value = serde_json::from_slice(&default.stdout).unwrap();
    let default_ids = default["resourceLogs"][0]["scopeLogs"][0]["logRecords"]
        .as_array()
        .unwrap()
        .iter()
        .map(|record| {
            record["attributes"][0]["value"]["stringValue"]
                .as_str()
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(default_ids, [manual.data.record.cut_id()]);

    let included = run_file(
        &file,
        &["export", "--format", "otlp-json", "--include-auto"],
    );
    assert!(included.status.success());
    assert!(included.stderr.is_empty());
    let included: Value = serde_json::from_slice(&included.stdout).unwrap();
    let included_ids = included["resourceLogs"][0]["scopeLogs"][0]["logRecords"]
        .as_array()
        .unwrap()
        .iter()
        .map(|record| {
            record["attributes"][0]["value"]["stringValue"]
                .as_str()
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        included_ids,
        [manual.data.record.cut_id(), auto.data.record.cut_id()]
    );
}

#[test]
fn export_otlp_json_empty_and_missing_default_are_stable_empty_lines() {
    let expected = format!(
        "{{\"resourceLogs\":[{{\"resource\":{{}},\"scopeLogs\":[{{\"scope\":{{\"name\":\"blotter\",\"version\":\"{}\"}},\"logRecords\":[]}}]}}]}}\n",
        env!("CARGO_PKG_VERSION")
    );
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("empty.jsonl");
    std::fs::write(&file, "").unwrap();
    let empty = run_file(&file, &["export", "--format", "otlp-json"]);
    assert_eq!(empty.status.code(), Some(0));
    assert!(empty.stderr.is_empty());
    assert_eq!(empty.stdout, expected.as_bytes());

    let repo = temp.path().join("repo");
    make_repo(&repo);
    let missing = command()
        .current_dir(&repo)
        .args(["export", "--format", "otlp-json"])
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(0));
    assert!(missing.stderr.is_empty());
    assert_eq!(missing.stdout, expected.as_bytes());
    assert!(!repo.join(".blotter.jsonl").exists());
}

#[test]
fn export_requires_otlp_json_format() {
    let output = run(&["export"]);
    let error = error(&output, 2, "invalid_argument");
    assert!(error.error.message.contains("--format otlp-json"));
    assert!(error.error.suggested_fix.contains("--format otlp-json"));
}

#[test]
fn export_missing_format_is_reported_before_the_clock_is_resolved() {
    let output = command()
        .env("BLOTTER_NOW", "not a timestamp")
        .arg("export")
        .output()
        .unwrap();
    let error = error(&output, 2, "invalid_argument");
    assert!(error.error.message.contains("--format otlp-json"));
}

#[test]
fn export_rejects_records_outside_the_otlp_nanosecond_range() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let records = [
        json!({
            "kind": "cut", "id": "bl_d39a975de893", "ts": "2026-08-18T10:00:00.123Z",
            "agent": "alice", "text": "modern report", "tags": [], "severity": "minor",
            "cwd": "src/a"
        }),
        json!({
            "kind": "cut", "id": "bl_0ff01ce4a5e5", "ts": "1969-07-20T20:17:00.000Z",
            "agent": "alice", "text": "pre-epoch report", "tags": [], "severity": "minor",
            "cwd": "src/a"
        }),
    ];
    let mut input = records
        .into_iter()
        .map(|record| record.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    input.push('\n');
    std::fs::write(&file, input).unwrap();

    let output = run_file(&file, &["export", "--format", "otlp-json"]);
    let error = error(&output, 65, "invalid_input");
    assert!(error.error.message.contains("bl_0ff01ce4a5e5"));
    assert!(error.error.message.contains("1969-07-20T20:17:00.000Z"));
    assert!(error.error.suggested_fix.contains("--since"));

    // Deterministic: the same input rejects with byte-identical bytes.
    let again = run_file(&file, &["export", "--format", "otlp-json"]);
    assert_eq!(again.stderr, output.stderr);

    // The in-range record still exports once the offender is filtered out.
    let filtered = run_file(
        &file,
        &[
            "export",
            "--format",
            "otlp-json",
            "--since",
            "2026-01-01T00:00:00Z",
        ],
    );
    assert_eq!(filtered.status.code(), Some(0));
    let exported: Value = serde_json::from_slice(&filtered.stdout).unwrap();
    let logs = exported["resourceLogs"][0]["scopeLogs"][0]["logRecords"]
        .as_array()
        .unwrap();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0]["timeUnixNano"], "1787047200123000000");
}

#[test]
fn export_otlp_json_since_matches_list() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let excluded = add_at(&file, "2026-07-09T16:30:00.122Z", "old export record", &[]);
    let included = add_at(&file, "2026-07-09T16:30:00.123Z", "new export record", &[]);

    let listed: SuccessEnvelope<ListData> = success(
        &command()
            .arg("--file")
            .arg(&file)
            .args(["list", "--since", "2h"])
            .output()
            .unwrap(),
    );
    let exported = command()
        .arg("--file")
        .arg(&file)
        .args(["export", "--format", "otlp-json", "--since", "2h"])
        .output()
        .unwrap();
    assert!(exported.status.success());
    assert!(exported.stderr.is_empty());
    let exported: Value = serde_json::from_slice(&exported.stdout).unwrap();
    let mut export_ids = exported["resourceLogs"][0]["scopeLogs"][0]["logRecords"]
        .as_array()
        .unwrap()
        .iter()
        .map(|record| {
            record["attributes"][0]["value"]["stringValue"]
                .as_str()
                .unwrap()
        })
        .collect::<Vec<_>>();
    let mut list_ids = listed
        .data
        .items
        .iter()
        .map(|item| item.id.as_str())
        .collect::<Vec<_>>();
    export_ids.sort_unstable();
    list_ids.sort_unstable();
    assert_eq!(export_ids, list_ids);
    assert_eq!(export_ids, [included.data.record.cut_id()]);
    assert_ne!(excluded.data.record.cut_id(), included.data.record.cut_id());
}

#[test]
fn schema_documents_export_otlp_bridge() {
    let schema: SuccessEnvelope<Value> = success(&run(&["schema"]));
    let export = &schema.data["commands"]["export"];
    assert_eq!(export["flags"]["--format"], "otlp-json; required");
    assert_eq!(export["flags"]["--since"], "full RFC3339|Nd|Nh");
    assert!(
        export["flags"]["--include-auto"]
            .as_str()
            .unwrap()
            .contains("include records tagged auto")
    );
    assert_eq!(export["eventName"], "blotter.friction.reported");
    assert!(export["output"].as_str().unwrap().contains("raw"));
    assert!(export["output"].as_str().unwrap().contains("LogsData"));
    assert_eq!(export["read_only"], true);
    assert_eq!(export["appends"], false);
    assert_eq!(export["destructive"], false);
}
