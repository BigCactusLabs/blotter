use crate::common::*;

/// `origin` is optional, so a stored record without one folds, lists, and is
/// left byte-identical — the provenance-field port of the pre-v2 `source` test.
#[test]
fn records_without_origin_fold_and_list_byte_identically() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("no-origin.jsonl");
    let id = compute_id(
        "2026-08-01T00:00:00.000Z",
        "porter",
        "record without origin",
        Impact::Low,
        &["provenance".into()],
    );
    let stored = json!({
        "v": 2,
        "kind": "cut",
        "id": id,
        "ts": "2026-08-01T00:00:00.000Z",
        "agent": "porter",
        "text": "record without origin",
        "tags": ["provenance"],
        "impact": "low",
        "cwd": "/tmp"
    });
    let stored_bytes = format!("{stored}\n");
    std::fs::write(&file, &stored_bytes).unwrap();

    let output = run_file(&file, &["list"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let file_json = serde_json::to_string(&file.to_string_lossy()).unwrap();
    let expected = format!(
        r#"{{"ok":true,"data":{{"items":[{{"kind":"cut","id":"{id}","ts":"2026-08-01T00:00:00.000Z","agent":"porter","text":"record without origin","tags":["provenance"],"impact":"low","cwd":"/tmp","status":"open"}}],"count":1,"total":1,"truncated":false}},"meta":{{"contract":6,"file":{file_json}}}}}"#
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("{expected}\n")
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), stored_bytes);
}

/// An unrecognized `origin.type` folds and lists unchanged, carried through as
/// the string it is: no command in 1.0.0 validates the value.
#[test]
fn unknown_stored_origin_type_round_trips_through_fold_and_list() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("unknown-origin.jsonl");
    let id = compute_id(
        "2026-08-01T00:00:00.000Z",
        "future",
        "future origin",
        Impact::Low,
        &[],
    );
    let stored = json!({
        "v": 2,
        "kind": "cut",
        "id": id,
        "ts": "2026-08-01T00:00:00.000Z",
        "agent": "future",
        "text": "future origin",
        "tags": [],
        "impact": "low",
        "cwd": "/tmp",
        "origin": {"type": "detector", "provider": "otel", "ref": "opaque"}
    });
    let stored_bytes = format!("{stored}\n");
    std::fs::write(&file, &stored_bytes).unwrap();

    let listed: SuccessEnvelope<Value> = success(&run_file(&file, &["list"]));
    assert_eq!(
        listed.data["items"][0]["origin"],
        json!({"type":"detector","provider":"otel","ref":"opaque"})
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), stored_bytes);
}

/// `origin` is carried everywhere `source` was: triage clusters, digest chronic
/// entries, and verify recurrences.
#[test]
fn origin_propagates_to_triage_digest_and_verify_json() {
    let temp = TempDir::new().unwrap();
    let analysis_file = temp.path().join("analysis-origin.jsonl");
    let analysis_text = "origin provenance analysis";
    let first_ts = "2026-07-09T18:29:00.000Z";
    let second_ts = "2026-07-09T18:30:00.000Z";
    let tags = vec!["provenance".into()];
    let origin = json!({"type": "agent", "provider": "detector"});
    let first = json!({
        "v": 2,
        "kind": "cut",
        "id": compute_id(first_ts, "detector", analysis_text, Impact::Low, &tags),
        "ts": first_ts,
        "agent": "detector",
        "text": analysis_text,
        "tags": tags,
        "impact": "low",
        "cwd": "/tmp",
        "origin": origin
    });
    let second = json!({
        "v": 2,
        "kind": "cut",
        "id": compute_id(second_ts, "detector", analysis_text, Impact::Low, &tags),
        "ts": second_ts,
        "agent": "detector",
        "text": analysis_text,
        "tags": tags,
        "impact": "low",
        "cwd": "/tmp",
        "origin": origin
    });
    std::fs::write(&analysis_file, format!("{first}\n{second}\n")).unwrap();

    let triage = triage_success(
        &run_file(&analysis_file, &["triage", "--min-count", "2"]),
        1,
    );
    assert_eq!(triage.data["clusters"][0]["origin"], origin);
    let digest: SuccessEnvelope<Value> =
        success(&run_file(&analysis_file, &["digest", "--since", "1d"]));
    assert_eq!(digest.data["chronic"][0]["origin"], origin);

    let verify_file = temp.path().join("verify-origin.jsonl");
    let resolved_text = "origin provenance recurrence";
    let resolved_ts = "2026-07-09T16:00:00.000Z";
    let recurrence_ts = "2026-07-09T16:20:00.000Z";
    let resolved_id = compute_id(resolved_ts, "detector", resolved_text, Impact::Low, &[]);
    let recurrence_id = compute_id(
        recurrence_ts,
        "self-report",
        resolved_text,
        Impact::Low,
        &[],
    );
    let resolved = json!({
        "v": 2,
        "kind": "cut",
        "id": resolved_id,
        "ts": resolved_ts,
        "agent": "detector",
        "text": resolved_text,
        "tags": [],
        "impact": "low",
        "cwd": "/tmp",
        "origin": origin
    });
    let resolution = json!({
        "v": 2,
        "kind": "resolve",
        "id": resolved_id,
        "ts": "2026-07-09T16:10:00.000Z",
        "agent": "tester",
        "note": null,
        "disposition": "fixed",
        "disposition_ts": "2026-07-09T16:10:00.000Z"
    });
    let recurrence = json!({
        "v": 2,
        "kind": "cut",
        "id": recurrence_id,
        "ts": recurrence_ts,
        "agent": "self-report",
        "text": resolved_text,
        "tags": [],
        "impact": "low",
        "cwd": "/tmp"
    });
    std::fs::write(
        &verify_file,
        format!("{resolved}\n{resolution}\n{recurrence}\n"),
    )
    .unwrap();

    let verify = verify_success(&run_file(&verify_file, &["verify"]), 1);
    assert_eq!(verify.data["recurrences"][0]["origin"], origin);
}

/// r49: a published member that is JSON `null` reads as absent. That is plain
/// deserialization of a typed optional, not a validation rule.
#[test]
fn origin_null_members_read_as_absent() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("null-members.jsonl");
    let id = compute_id(
        "2026-08-01T00:00:00.000Z",
        "porter",
        "null origin members",
        Impact::Low,
        &[],
    );
    let stored = json!({
        "v": 2,
        "kind": "cut",
        "id": id,
        "ts": "2026-08-01T00:00:00.000Z",
        "agent": "porter",
        "text": "null origin members",
        "tags": [],
        "impact": "low",
        "cwd": "/tmp",
        "origin": {"type": "agent", "provider": null, "ref": null}
    });
    let stored_bytes = format!("{stored}\n");
    std::fs::write(&file, &stored_bytes).unwrap();

    let listed: SuccessEnvelope<Value> = success(&run_file(&file, &["list"]));
    assert!(listed.meta.warnings.is_empty());
    assert_eq!(listed.data["items"][0]["origin"], json!({"type":"agent"}));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), stored_bytes);
}

/// r49: a published member that is not a string does not deserialize, so the
/// line is malformed under the existing rule — a fold warning and a `doctor`
/// finding, never a failed read command.
#[test]
fn origin_non_string_member_is_a_malformed_line() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("non-string-member.jsonl");
    let stored = json!({
        "v": 2,
        "kind": "cut",
        "id": "bl_aaaaaaaaaaaaaaaaaaaa",
        "ts": "2026-08-01T00:00:00.000Z",
        "agent": "porter",
        "text": "non-string origin member",
        "tags": [],
        "impact": "low",
        "cwd": "/tmp",
        "origin": {"type": "agent", "provider": 7}
    });
    std::fs::write(&file, format!("{stored}\n")).unwrap();

    let listed: SuccessEnvelope<ListData> = success(&run_file(&file, &["list"]));
    assert_eq!(listed.data.items.len(), 0);
    assert!(
        listed
            .meta
            .warnings
            .contains(&"skipped 1 malformed line".to_owned())
    );

    let doctor = doctor_response(&run_file(&file, &["doctor"]), 1);
    assert_eq!(doctor.data.findings.len(), 1);
    assert_eq!(doctor.data.findings[0].kind, "malformed");
    assert!(doctor.data.findings[0].fixable);
}

/// The published-members-only promise: an unknown member inside a stored
/// `origin` survives in the log's bytes, because nothing rewrites a record, and
/// reaches no envelope.
#[test]
fn unknown_origin_member_is_dropped_from_the_envelope_but_kept_in_the_bytes() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("unknown-member.jsonl");
    let id = compute_id(
        "2026-08-01T00:00:00.000Z",
        "porter",
        "unknown origin member",
        Impact::Low,
        &[],
    );
    let stored = json!({
        "v": 2,
        "kind": "cut",
        "id": id,
        "ts": "2026-08-01T00:00:00.000Z",
        "agent": "porter",
        "text": "unknown origin member",
        "tags": [],
        "impact": "low",
        "cwd": "/tmp",
        "origin": {"type": "agent", "trace_id": "0af7651916cd43dd8448eb211c80319c"}
    });
    let stored_bytes = format!("{stored}\n");
    std::fs::write(&file, &stored_bytes).unwrap();

    let listed: SuccessEnvelope<Value> = success(&run_file(&file, &["list"]));
    assert_eq!(listed.data["items"][0]["origin"], json!({"type":"agent"}));
    assert!(listed.data["items"][0]["origin"]["trace_id"].is_null());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), stored_bytes);
}

/// `add` and `dogear` write `{"type":"agent"}`, on the stored line and in the
/// reported record alike.
#[test]
fn add_and_dogear_write_the_agent_origin() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("written-origin.jsonl");
    let cut: SuccessEnvelope<Value> = success(&run_file(
        &file,
        &["add", "origin on write", "--agent", "tester"],
    ));
    assert_eq!(cut.data["record"]["origin"], json!({"type":"agent"}));
    let dogear: SuccessEnvelope<Value> = success(&run_file(
        &file,
        &["dogear", "origin on write too", "--agent", "tester"],
    ));
    assert_eq!(dogear.data["record"]["origin"], json!({"type":"agent"}));

    for line in std::fs::read_to_string(&file).unwrap().lines() {
        let stored: Value = serde_json::from_str(line).unwrap();
        assert_eq!(stored["origin"], json!({"type":"agent"}));
    }
}
