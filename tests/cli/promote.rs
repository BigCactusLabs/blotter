//! `blotter promote` and the `promotion` record (r48, r50, r51).

use crate::common::*;

fn promote(file: &Path, args: &[&str]) -> std::process::Output {
    promote_at(file, NOW, args)
}

fn promotion_of(record: &LogEvent) -> (&str, &[String], &Artifact, Option<&str>, &str) {
    match record {
        LogEvent::Promotion {
            id,
            sources,
            artifact,
            note,
            cwd,
            ..
        } => (id, sources, artifact, note.as_deref(), cwd),
        _ => panic!("promote responses must contain promotion events"),
    }
}

fn log_lines(file: &Path) -> Vec<Value> {
    std::fs::read_to_string(file)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[test]
fn promote_appends_a_promotion_whose_id_is_twenty_hex() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("log.jsonl");
    let first = add_at(&file, "2026-07-01T00:00:00Z", "first friction", &[]);
    let second = add_at(&file, "2026-07-02T00:00:00Z", "second friction", &[]);
    let (first, second) = (
        first.data.record.cut_id().to_owned(),
        second.data.record.cut_id().to_owned(),
    );

    // Deliberately unsorted and repeated: the stored set is sorted, deduplicated
    // and hashed in exactly that form (r48).
    let output = promote(
        &file,
        &[
            "--source",
            &second,
            "--source",
            &first,
            "--source",
            &second,
            "--artifact-type",
            "skill",
            "--artifact-ref",
            "skills/testing.md",
            "--note",
            "Repeated fixture failures promoted into guidance.",
        ],
    );
    let envelope: SuccessEnvelope<PromoteData> = success(&output);
    assert!(envelope.data.changed);
    let (id, sources, artifact, note, _) = promotion_of(&envelope.data.record);

    // r51: every v2 identity is `bl_` plus 20 lowercase hex.
    assert_eq!(id.len(), 23, "{id}");
    assert!(id.starts_with("bl_"));
    assert!(
        id[3..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    );
    let mut expected_sources = vec![first, second];
    expected_sources.sort();
    assert_eq!(sources, expected_sources.as_slice());
    assert_eq!(
        *id,
        compute_promotion_id(
            "2026-07-09T18:30:00.123Z",
            "tester",
            &expected_sources,
            "skill",
            "skills/testing.md"
        )
    );
    assert_eq!(artifact.kind, ArtifactType::Skill);
    assert_eq!(artifact.r#ref, "skills/testing.md");
    assert_eq!(
        note,
        Some("Repeated fixture failures promoted into guidance.")
    );
    assert_eq!(envelope.meta.agent_source.as_deref(), Some("flag"));

    // The stored line is the envelope record plus `"v":2` first (r50).
    let record = serde_json::to_value(&envelope.data.record).unwrap();
    assert!(record.get("v").is_none());
    let lines = log_lines(&file);
    assert_eq!(*lines.last().unwrap(), stored_line(&record));
    let stored = std::fs::read_to_string(&file).unwrap();
    let last = stored.lines().next_back().unwrap();
    assert!(last.starts_with(r#"{"v":2,"kind":"promotion","#));
    assert!(last.contains(r#""origin":{"type":"agent"}"#));
}

#[test]
fn the_note_is_outside_the_promotion_hash() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("log.jsonl");
    let cut = add_at(&file, "2026-07-01T00:00:00Z", "friction", &[]);
    let cut = cut.data.record.cut_id().to_owned();
    let base = ["--source", cut.as_str(), "--artifact-type", "doc"];

    let without: SuccessEnvelope<PromoteData> = success(&promote(
        &file,
        &[&base[..], &["--artifact-ref", "docs/x.md"]].concat(),
    ));
    let with_note: SuccessEnvelope<PromoteData> = success(&promote(
        &file,
        &[
            &base[..],
            &["--artifact-ref", "docs/x.md", "--note", "reworded"],
        ]
        .concat(),
    ));
    assert_eq!(
        promotion_of(&without.data.record).0,
        promotion_of(&with_note.data.record).0
    );
    // Same ID means the second call is a duplicate, so the stored note stays the
    // first record's — absent.
    assert!(!with_note.data.changed);
    assert_eq!(promotion_of(&with_note.data.record).3, None);
    assert_eq!(
        with_note.meta.warnings,
        ["duplicate promotion; existing record returned"]
    );
    assert_eq!(log_lines(&file).len(), 2);
}

#[test]
fn a_source_that_is_not_a_cut_is_rejected_by_kind() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("log.jsonl");
    let cut = add_at(&file, "2026-07-01T00:00:00Z", "friction", &[]);
    let cut = cut.data.record.cut_id().to_owned();
    let dogear: SuccessEnvelope<Value> = dogear_at(&file, "2026-07-02T00:00:00Z", "idea", &[]);
    let dogear = dogear.data["record"]["id"].as_str().unwrap().to_owned();
    let promotion: SuccessEnvelope<PromoteData> = success(&promote(
        &file,
        &[
            "--source",
            &cut,
            "--artifact-type",
            "doc",
            "--artifact-ref",
            "docs/x.md",
        ],
    ));
    let promotion = promotion_of(&promotion.data.record).0.to_owned();

    for (id, kind) in [(&dogear, "dogear"), (&promotion, "promotion")] {
        let envelope = error(
            &promote(
                &file,
                &[
                    "--source",
                    id,
                    "--artifact-type",
                    "doc",
                    "--artifact-ref",
                    "docs/y.md",
                ],
            ),
            2,
            "invalid_argument",
        );
        assert!(envelope.error.message.contains(id.as_str()));
        assert!(envelope.error.message.contains(kind));
    }
    // Recursive promotion and dogear promotion append nothing.
    assert_eq!(log_lines(&file).len(), 3);
}

#[test]
fn source_prefixes_answer_through_the_single_resolution_rule() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("log.jsonl");
    let cut = add_at(&file, "2026-07-01T00:00:00Z", "friction", &[]);
    let cut = cut.data.record.cut_id().to_owned();
    let artifact = ["--artifact-type", "doc", "--artifact-ref", "docs/x.md"];

    error(
        &promote(&file, &[&["--source", "ffffffff"][..], &artifact].concat()),
        66,
        "not_found",
    );
    error(
        &promote(&file, &[&["--source", "abc"][..], &artifact].concat()),
        2,
        "invalid_argument",
    );

    // Two records whose IDs share a four-hex prefix cannot be manufactured from
    // real hashes, so the ambiguity is built from a hand-written pair.
    let ambiguous = temp.path().join("ambiguous.jsonl");
    std::fs::write(&ambiguous, "").unwrap();
    let sibling = format!("{}{}", &cut[..7], "0".repeat(16));
    append_lines(
        &ambiguous,
        &[
            json!({"v":2,"kind":"cut","id":cut,"ts":"2026-07-01T00:00:00.000Z","agent":"tester","text":"a","tags":[],"impact":"low","cwd":"."}).to_string(),
            json!({"v":2,"kind":"cut","id":sibling,"ts":"2026-07-01T00:00:00.000Z","agent":"tester","text":"b","tags":[],"impact":"low","cwd":"."}).to_string(),
        ],
    );
    let envelope = error(
        &promote(
            &ambiguous,
            &[&["--source", &cut[3..7]][..], &artifact].concat(),
        ),
        65,
        "ambiguous_id",
    );
    let candidates = envelope.error.details["candidates"].as_array().unwrap();
    assert_eq!(candidates.len(), 2);
}

#[test]
fn a_dry_run_folds_the_log_and_writes_nothing() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("log.jsonl");
    let cut = add_at(&file, "2026-07-01T00:00:00Z", "friction", &[]);
    let cut = cut.data.record.cut_id().to_owned();
    let before = std::fs::read(&file).unwrap();

    let envelope: SuccessEnvelope<PromoteData> = success(&promote(
        &file,
        &[
            "--source",
            &cut,
            "--artifact-type",
            "guard",
            "--artifact-ref",
            "scripts/gate.sh",
            "--dry-run",
        ],
    ));
    assert!(!envelope.data.changed);
    assert_eq!(envelope.meta.warnings, ["dry run; no record appended"]);
    assert_eq!(std::fs::read(&file).unwrap(), before);

    // The apply appends exactly the record the dry run predicted.
    let applied: SuccessEnvelope<PromoteData> = success(&promote(
        &file,
        &[
            "--source",
            &cut,
            "--artifact-type",
            "guard",
            "--artifact-ref",
            "scripts/gate.sh",
        ],
    ));
    assert!(applied.data.changed);
    assert_eq!(applied.data.record, envelope.data.record);

    // Unlike `add --dry-run`, promote opens the log: against a missing one its
    // sources cannot resolve (r48).
    let missing = temp.path().join("missing.jsonl");
    error(
        &promote(
            &missing,
            &[
                "--source",
                &cut,
                "--artifact-type",
                "doc",
                "--artifact-ref",
                "docs/x.md",
                "--dry-run",
            ],
        ),
        66,
        "not_found",
    );
    assert!(!missing.exists());
}

#[test]
fn the_ref_and_note_are_bounded_and_reject_empty_values() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("log.jsonl");
    let cut = add_at(&file, "2026-07-01T00:00:00Z", "friction", &[]);
    let cut = cut.data.record.cut_id().to_owned();
    let long = "x".repeat(10_001);

    for extra in [
        vec!["--artifact-ref", "   "],
        vec!["--artifact-ref", long.as_str()],
        vec!["--artifact-ref", "docs/x.md", "--note", "  \t "],
        vec!["--artifact-ref", "docs/x.md", "--note", long.as_str()],
    ] {
        let args = [
            &["--source", cut.as_str(), "--artifact-type", "doc"][..],
            &extra,
        ]
        .concat();
        error(&promote(&file, &args), 65, "invalid_input");
    }
    assert_eq!(log_lines(&file).len(), 1);
}

#[test]
fn byte_identical_duplicate_promotion_lines_are_first_wins() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("log.jsonl");
    let cut = add_at(&file, "2026-07-01T00:00:00Z", "friction", &[]);
    let cut = cut.data.record.cut_id().to_owned();
    success::<PromoteData>(&promote(
        &file,
        &[
            "--source",
            &cut,
            "--artifact-type",
            "tool",
            "--artifact-ref",
            "bin/wrap",
        ],
    ));
    let stored = std::fs::read_to_string(&file).unwrap();
    let promotion_line = stored.lines().next_back().unwrap().to_owned();
    append_lines(&file, &[promotion_line]);

    let listed: SuccessEnvelope<ListData> =
        success(&run_file(&file, &["list", "--kind", "promotion"]));
    assert_eq!(listed.data.count, 1);
    assert_eq!(listed.meta.warnings, ["skipped 1 duplicate promotion"]);
}

#[test]
fn a_promotion_is_never_a_resolve_target() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("log.jsonl");
    let cut = add_at(&file, "2026-07-01T00:00:00Z", "friction", &[]);
    let cut = cut.data.record.cut_id().to_owned();
    let promotion: SuccessEnvelope<PromoteData> = success(&promote(
        &file,
        &[
            "--source",
            &cut,
            "--artifact-type",
            "process",
            "--artifact-ref",
            "docs/process.md",
        ],
    ));
    let promotion = promotion_of(&promotion.data.record).0.to_owned();

    let envelope = error(
        &run_file(
            &file,
            &[
                "resolve",
                &promotion,
                "--disposition",
                "fixed",
                "--agent",
                "x",
            ],
        ),
        2,
        "invalid_argument",
    );
    assert!(envelope.error.message.contains(&promotion));
    assert!(envelope.error.message.contains("promotion"));
    assert_eq!(log_lines(&file).len(), 2);
}

#[test]
fn a_v1_log_is_refused_before_any_byte_is_written() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("log.jsonl");
    std::fs::write(&file, format!("{}\n", v1_cut_line())).unwrap();
    let before = std::fs::read(&file).unwrap();

    for extra in [vec![], vec!["--dry-run"]] {
        let args = [
            &[
                "--source",
                "a1b2",
                "--artifact-type",
                "doc",
                "--artifact-ref",
                "docs/x.md",
            ][..],
            &extra,
        ]
        .concat();
        let envelope = error(&promote(&file, &args), 65, "unsupported_log_version");
        assert_eq!(envelope.error.details["line"], 1);
        assert!(envelope.error.details.get("found_version").is_none());
        assert_eq!(envelope.error.suggested_fix, unsupported_version_fix(&file));
    }
    assert_eq!(std::fs::read(&file).unwrap(), before);
    assert_eq!(directory_entries(temp.path()), ["log.jsonl"]);
}

#[test]
fn schema_documents_promote() {
    let schema: SuccessEnvelope<Value> = success(&run(&["schema"]));
    let promote = &schema.data["commands"]["promote"];
    assert_eq!(promote["read_only"], false);
    assert_eq!(promote["appends"], true);
    assert_eq!(promote["destructive"], false);
    assert_eq!(promote["output"], "{changed,record}");
    for flag in [
        "--source",
        "--artifact-type",
        "--artifact-ref",
        "--note",
        "--agent",
        "--dry-run",
    ] {
        assert!(promote["flags"][flag].is_string(), "{flag}");
    }
    assert!(
        promote["semantics"]
            .as_str()
            .unwrap()
            .contains("duplicate promotion; existing record returned")
    );
    assert!(promote["dry_run"].as_str().unwrap().contains("not_found"));

    let record = &schema.data["records"]["promotion"];
    assert_eq!(record["v"], 2);
    assert_eq!(record["kind"], "promotion");
    assert_eq!(record["id"], "bl_<20 lowercase hex>");
    assert_eq!(
        record["artifact"]["type"],
        "doc|skill|guard|test|tool|process"
    );

    assert_eq!(
        schema.data["artifact_types"]["values"],
        json!(["doc", "skill", "guard", "test", "tool", "process"])
    );
    let identity = &schema.data["id"]["promotion"];
    assert_eq!(identity["hex_digits"], 20);
    assert_eq!(identity["hash"], "SHA-256 first 10 bytes");
    assert!(identity["excluded"].as_str().unwrap().contains("note"));
}

#[test]
fn a_dry_run_over_a_duplicate_reports_both_warnings() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("log.jsonl");
    let cut = add_at(&file, "2026-07-01T00:00:00Z", "friction", &[]);
    let cut = cut.data.record.cut_id().to_owned();
    let args = [
        "--source",
        cut.as_str(),
        "--artifact-type",
        "doc",
        "--artifact-ref",
        "docs/x.md",
    ];
    let applied: SuccessEnvelope<PromoteData> = success(&promote(&file, &args));
    assert!(applied.data.changed);
    let before = std::fs::read(&file).unwrap();

    // The dry run folds, finds the duplicate, and returns the existing record.
    // Reporting only "dry run" would say `changed:false` with no reason and
    // promise something the apply would not produce.
    let dry: SuccessEnvelope<PromoteData> =
        success(&promote(&file, &[&args[..], &["--dry-run"]].concat()));
    assert!(!dry.data.changed);
    assert_eq!(dry.data.record, applied.data.record);
    assert_eq!(
        dry.meta.warnings,
        [
            "duplicate promotion; existing record returned",
            "dry run; no record appended"
        ]
    );
    assert_eq!(std::fs::read(&file).unwrap(), before);

    // The apply reports the duplicate alone.
    let again: SuccessEnvelope<PromoteData> = success(&promote(&file, &args));
    assert_eq!(
        again.meta.warnings,
        ["duplicate promotion; existing record returned"]
    );
}

#[test]
fn an_apply_against_a_missing_log_is_not_found_and_creates_nothing() {
    let temp = TempDir::new().unwrap();
    let missing = temp.path().join("missing.jsonl");
    // A promotion cannot be the first record in a log — it needs a cut to cite —
    // so `promote` never creates one, on either lane.
    error(
        &promote(
            &missing,
            &[
                "--source",
                "abcd",
                "--artifact-type",
                "doc",
                "--artifact-ref",
                "docs/x.md",
            ],
        ),
        66,
        "not_found",
    );
    assert!(!missing.exists());
    assert!(directory_entries(temp.path()).is_empty());
}
