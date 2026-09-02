use crate::common::*;

#[test]
fn digest_json_reports_chronic_windowed_cuts_and_open_dogears() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let first = add_at(
        &file,
        "2026-07-08T18:00:00Z",
        "Cache config missing",
        &["common", "api"],
    );
    let second = add_at(
        &file,
        "2026-07-09T17:00:00Z",
        "Cache config missing",
        &["api"],
    );
    let untagged = add_at(&file, "2026-07-09T16:00:00Z", "Untagged new cut", &[]);
    let resolved_cut = add_at(
        &file,
        "2026-07-09T15:00:00Z",
        "Resolved cut exclusion",
        &["api"],
    );
    let _: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &[
            "resolve",
            "--disposition",
            "fixed",
            resolved_cut.data.record.cut_id(),
            "--note",
            "done",
        ],
    ));

    let open_dogear = dogear_at(&file, "2026-06-01T00:00:00Z", "Standing idea", &["ideas"]);
    let open_dogear_id = open_dogear.data["record"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let resolved_dogear = dogear_at(
        &file,
        "2026-06-02T00:00:00Z",
        "Resolved idea exclusion",
        &["ideas"],
    );
    let resolved_dogear_id = resolved_dogear.data["record"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let _: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &["resolve", &resolved_dogear_id, "--note", "done"],
    ));

    let before = std::fs::read(&file).unwrap();
    let digest: SuccessEnvelope<Value> = success(&run_file(&file, &["digest"]));
    let mut api_ids = vec![
        first.data.record.cut_id().to_owned(),
        second.data.record.cut_id().to_owned(),
    ];
    api_ids.sort();
    assert_eq!(
        digest.data,
        json!({
            "chronic": [{
                "count": 2,
                "occurrences": 2,
                "ids": [first.data.record.cut_id(), second.data.record.cut_id()],
                "tags": ["api", "common"],
                "text": "Cache config missing",
                "origin": {"type":"agent"},
                "suggested_action": "graduate",
            }],
            "new_cuts": {
                "count": 3,
                "by_tag": [
                    {"tag": "api", "count": 2, "ids": api_ids},
                    {"tag": "", "count": 1, "ids": [untagged.data.record.cut_id()]},
                    {"tag": "common", "count": 1, "ids": [first.data.record.cut_id()]},
                ],
            },
            "open_dogears": {
                "count": 1,
                "items": [{
                    "id": open_dogear_id,
                    "ts": "2026-06-01T00:00:00.000Z",
                    "text": "Standing idea",
                    "tags": ["ideas"],
                }],
            },
            "window": {
                "since": "2026-07-02T18:30:00.123Z",
                "until": "2026-07-09T18:30:00.123Z",
            },
        })
    );
    assert_eq!(std::fs::read(&file).unwrap(), before);
}

#[test]
fn digest_since_excludes_old_cuts_but_keeps_them_chronic() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let old = add_at(
        &file,
        "2026-06-01T00:00:00Z",
        "Workspace cache missing",
        &["build"],
    );
    let recent = add_at(
        &file,
        "2026-07-09T17:00:00Z",
        "Workspace cache missing",
        &["build"],
    );

    let digest: SuccessEnvelope<Value> = success(&run_file(&file, &["digest", "--since", "1d"]));
    assert_eq!(
        digest.data["chronic"],
        json!([{
            "count": 2,
            "occurrences": 2,
            "ids": [old.data.record.cut_id(), recent.data.record.cut_id()],
            "tags": ["build"],
            "text": "Workspace cache missing",
            "origin": {"type":"agent"},
            "suggested_action": "graduate",
        }])
    );
    assert_eq!(
        digest.data["new_cuts"],
        json!({
            "count": 1,
            "by_tag": [{"tag": "build", "count": 1, "ids": [recent.data.record.cut_id()]}],
        })
    );
    assert_eq!(
        digest.data["window"],
        json!({"since":"2026-07-08T18:30:00.123Z","until":"2026-07-09T18:30:00.123Z"})
    );
}

#[test]
fn digest_markdown_is_byte_deterministic_and_empty_is_successful() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let first = add_at(&file, "2026-07-08T18:30:00Z", "Cache missing", &["build"]);
    let second = add_at(&file, "2026-07-09T18:30:00Z", "Cache missing", &["build"]);
    let dogear = dogear_at(&file, "2026-07-01T00:00:00Z", "Read the docs", &["docs"]);
    let dogear_id = dogear.data["record"]["id"].as_str().unwrap();
    let mut new_cut_ids = [
        first.data.record.cut_id().to_owned(),
        second.data.record.cut_id().to_owned(),
    ];
    new_cut_ids.sort();
    let expected = format!(
        "## Chronic\n- Cache missing (2): {}, {}\n\n## New cuts\n### build (2)\n- {}\n- {}\n\n## Open dogears\n- [{}] Read the docs — 2026-07-01T00:00:00.000Z (docs)\n",
        first.data.record.cut_id(),
        second.data.record.cut_id(),
        new_cut_ids[0],
        new_cut_ids[1],
        dogear_id,
    );
    let first_output = run_file(&file, &["digest", "--format", "md"]);
    assert!(first_output.status.success());
    assert!(first_output.stderr.is_empty());
    assert_eq!(first_output.stdout, expected.as_bytes());
    let second_output = run_file(&file, &["digest", "--format", "md"]);
    assert!(second_output.status.success());
    assert_eq!(first_output.stdout, second_output.stdout);

    let empty = temp.path().join("empty.jsonl");
    std::fs::write(&empty, "").unwrap();
    let empty_output = run_file(&empty, &["digest", "--format", "md"]);
    assert_eq!(empty_output.status.code(), Some(0));
    assert!(empty_output.stderr.is_empty());
    assert_eq!(empty_output.stdout, b"No friction in window.\n");
}

#[test]
fn digest_markdown_collapses_multiline_text() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let first = add_at(
        &file,
        "2026-07-08T18:30:00Z",
        "Cache\n\tmissing",
        &["build"],
    );
    let second = add_at(
        &file,
        "2026-07-09T18:30:00Z",
        "Cache\n\tmissing",
        &["build"],
    );
    let dogear = dogear_at(&file, "2026-07-01T00:00:00Z", "Read\n\tthe docs", &["docs"]);
    let dogear_id = dogear.data["record"]["id"].as_str().unwrap();

    let output = run_file(&file, &["digest", "--format", "md"]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let markdown = String::from_utf8(output.stdout).unwrap();
    assert!(markdown.contains(&format!(
        "- Cache missing (2): {}, {}",
        first.data.record.cut_id(),
        second.data.record.cut_id()
    )));
    assert!(markdown.contains(&format!(
        "- [{dogear_id}] Read the docs — 2026-07-01T00:00:00.000Z (docs)"
    )));
}

#[test]
fn digest_markdown_reports_fold_warnings() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    std::fs::write(&file, "not json\n").unwrap();

    let output = run_file(&file, &["digest", "--format", "md"]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .ends_with("> note: skipped 1 malformed line\n")
    );
}

#[test]
fn digest_is_read_only() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    add(&file, "Read-only digest");
    dogear_at(&file, "2026-07-01T00:00:00Z", "Read-only idea", &[]);
    let before = std::fs::read(&file).unwrap();

    let _: SuccessEnvelope<Value> = success(&run_file(&file, &["digest"]));
    let markdown = run_file(&file, &["digest", "--format", "md"]);
    assert!(markdown.status.success());
    assert!(markdown.stderr.is_empty());
    assert_eq!(std::fs::read(&file).unwrap(), before);
}

#[test]
fn schema_documents_digest() {
    let schema: SuccessEnvelope<Value> = success(&run(&["schema"]));
    let digest = &schema.data["commands"]["digest"];
    assert_eq!(digest["flags"]["--since"], "full RFC3339|Nd|Nh; default 7d");
    assert!(digest["flags"].get("--include-auto").is_none());
    assert_eq!(digest["flags"]["--format"], "json|md; default json");
    assert!(digest["output"].as_str().unwrap().contains("new_cuts"));
    assert!(digest["output"].as_str().unwrap().contains("open_dogears"));
    assert!(
        digest["semantics"]
            .as_str()
            .unwrap()
            .contains("min_count 2")
    );
    assert!(digest["format"]["md"].as_str().unwrap().contains("raw"));
    assert_eq!(digest["read_only"], true);
    assert_eq!(digest["appends"], false);
    assert_eq!(digest["destructive"], false);
}
