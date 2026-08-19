use crate::common::*;

#[test]
fn retrospect_types_shared_program_clusters_as_wrapper_aliases() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let first = add_with_cmd_at(
        &file,
        "2026-07-09T18:30:00Z",
        "Build cache compiler fails",
        &["build"],
        "BUILD_MODE=ci /opt/tools/cargo build --release",
    );
    let second = add_with_cmd_at(
        &file,
        "2026-07-09T18:31:00Z",
        "build cache compiler fails again",
        &["build"],
        "/opt/tools/cargo test --workspace",
    );

    let retrospect = retrospect_success(&run_file(&file, &["retrospect"]), 1);
    assert_eq!(
        retrospect.data,
        json!({
            "candidates": [{
                "type": "wrapper_alias",
                "title": "Build cache compiler fails",
                "program": "cargo",
                "record_ids": [first.data.record.cut_id(), second.data.record.cut_id()],
                "occurrences": 2,
                "first_ts": "2026-07-09T18:30:00.000Z",
                "last_ts": "2026-07-09T18:31:00.000Z",
                "evidence": {
                    "texts": ["Build cache compiler fails", "build cache compiler fails again"],
                    "resolution_notes": [],
                },
            }],
            "count": 1,
            "scanned": 2,
        })
    );
}

#[test]
fn retrospect_types_docs_clusters_and_gives_wrapper_precedence() {
    let temp = TempDir::new().unwrap();
    let docs_file = temp.path().join("docs.jsonl");
    let first = add_at(
        &docs_file,
        "2026-07-09T18:30:00Z",
        "Deployment documentation is unclear",
        &["docs"],
    );
    let second = add_at(
        &docs_file,
        "2026-07-09T18:31:00Z",
        "deployment documentation is unclear again",
        &["docs", "documentation"],
    );

    let docs = retrospect_success(&run_file(&docs_file, &["retrospect"]), 1);
    assert_eq!(
        docs.data["candidates"],
        json!([{
            "type": "doc_repair",
            "title": "Deployment documentation is unclear",
            "record_ids": [first.data.record.cut_id(), second.data.record.cut_id()],
            "occurrences": 2,
            "first_ts": "2026-07-09T18:30:00.000Z",
            "last_ts": "2026-07-09T18:31:00.000Z",
            "evidence": {
                "texts": [
                    "Deployment documentation is unclear",
                    "deployment documentation is unclear again",
                ],
                "resolution_notes": [],
            },
        }])
    );

    let both_file = temp.path().join("both.jsonl");
    add_with_cmd_at(
        &both_file,
        "2026-07-09T18:32:00Z",
        "Release docs command fails",
        &["docs", "documentation"],
        "DOCS=1 /usr/local/bin/cargo doc",
    );
    add_with_cmd_at(
        &both_file,
        "2026-07-09T18:33:00Z",
        "release docs command fails again",
        &["documentation"],
        "/usr/local/bin/cargo doc --no-deps",
    );
    let both = retrospect_success(&run_file(&both_file, &["retrospect"]), 1);
    assert_eq!(both.data["count"], 1);
    assert_eq!(both.data["candidates"][0]["type"], "wrapper_alias");
    assert_eq!(both.data["candidates"][0]["program"], "cargo");
}

#[test]
fn retrospect_leaves_unmatched_chronic_clusters_as_ordinary_cuts() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    add_at(
        &file,
        "2026-07-09T18:30:00Z",
        "Service startup command fails",
        &["ops"],
    );
    add_at(
        &file,
        "2026-07-09T18:31:00Z",
        "service startup command fails again",
        &["ops"],
    );

    let retrospect = retrospect_success(&run_file(&file, &["retrospect"]), 0);
    assert_eq!(
        retrospect.data,
        json!({"candidates": [], "count": 0, "scanned": 2})
    );
}

#[test]
fn retrospect_promotes_repeated_resolved_recurrences_without_deduping_open_candidates() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let anchor = add_at(
        &file,
        "2026-07-09T18:30:00Z",
        "Cache recovery guide failed",
        &["ops"],
    );
    let _: SuccessEnvelope<ResolveData> = resolve_at(
        &file,
        "2026-07-09T18:31:00Z",
        anchor.data.record.cut_id(),
        &["--agent", "fixer", "--note", "Applied cache recovery guide"],
    );
    let first = add_with_cmd_at(
        &file,
        "2026-07-09T18:32:00Z",
        "cache recovery guide failed again",
        &["ops"],
        "cargo recover-cache",
    );
    let second = add_with_cmd_at(
        &file,
        "2026-07-09T18:33:00Z",
        "cache recovery guide failed twice",
        &["ops"],
        "cargo recover-cache --retry",
    );

    let retrospect = retrospect_success(&run_file(&file, &["retrospect"]), 1);
    assert_eq!(retrospect.data["count"], 2);
    let candidates = retrospect.data["candidates"].as_array().unwrap();
    let wrapper = candidates
        .iter()
        .find(|candidate| candidate["type"] == "wrapper_alias")
        .unwrap();
    let skill = candidates
        .iter()
        .find(|candidate| candidate["type"] == "skill_candidate")
        .unwrap();
    assert_eq!(
        wrapper["record_ids"],
        json!([first.data.record.cut_id(), second.data.record.cut_id()])
    );
    assert_eq!(
        skill,
        &json!({
            "type": "skill_candidate",
            "title": "Cache recovery guide failed",
            "record_ids": [first.data.record.cut_id(), second.data.record.cut_id()],
            "resolved_anchor_ids": [anchor.data.record.cut_id()],
            "occurrences": 2,
            "first_ts": "2026-07-09T18:32:00.000Z",
            "last_ts": "2026-07-09T18:33:00.000Z",
            "evidence": {
                "texts": [
                    "cache recovery guide failed again",
                    "cache recovery guide failed twice",
                ],
                "resolution_notes": ["Applied cache recovery guide"],
            },
        })
    );
}

#[test]
fn retrospect_does_not_promote_a_single_recurrence() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let anchor = add_at(
        &file,
        "2026-07-09T18:30:00Z",
        "Cache recovery guide failed",
        &["ops"],
    );
    let _: SuccessEnvelope<ResolveData> = resolve_at(
        &file,
        "2026-07-09T18:31:00Z",
        anchor.data.record.cut_id(),
        &["--agent", "fixer"],
    );
    add_at(
        &file,
        "2026-07-09T18:32:00Z",
        "cache recovery guide failed again",
        &["ops"],
    );

    let retrospect = retrospect_success(&run_file(&file, &["retrospect"]), 0);
    assert_eq!(
        retrospect.data,
        json!({"candidates": [], "count": 0, "scanned": 1})
    );
}

#[test]
fn retrospect_includes_auto_captures_without_a_flag() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    // The hand-filed text must differ from the captured command: r25 skips a
    // capture whose command equals an open cut's text. Linkage rides a shared
    // tag instead, and the cut stays untagged by auto so scanned == 2 still
    // proves the auto capture is included without a flag.
    let hand_filed = add_at(
        &file,
        "2026-07-09T18:30:00Z",
        "cargo build --release fails on a clean tree",
        &["claude-code"],
    );
    hook_exec_is_silent(&hook_exec_claude_code(
        &file,
        claude_bash_failure("cargo build --release", temp.path()).to_string(),
    ));
    let auto: Value = serde_json::from_str(
        std::fs::read_to_string(&file)
            .unwrap()
            .lines()
            .last()
            .unwrap(),
    )
    .unwrap();

    let retrospect = retrospect_success(&run_file(&file, &["retrospect"]), 1);
    assert_eq!(retrospect.meta.warnings, Vec::<String>::new());
    assert_eq!(retrospect.data["scanned"], 2);
    assert_eq!(retrospect.data["count"], 1);
    assert_eq!(retrospect.data["candidates"][0]["type"], "wrapper_alias");
    assert_eq!(retrospect.data["candidates"][0]["program"], "cargo");
    assert_eq!(
        retrospect.data["candidates"][0]["record_ids"],
        json!([hand_filed.data.record.cut_id(), auto["id"]])
    );
}

#[test]
fn retrospect_bounds_evidence_without_losing_cluster_occurrences() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    for index in 0..12 {
        let timestamp = format!("2026-07-09T18:{:02}:00Z", 30 + index);
        let text = format!("Release build command failed case{index:02}");
        add_with_cmd_at(
            &file,
            &timestamp,
            &text,
            &["build"],
            "BUILD_MODE=ci /opt/tools/cargo build --release",
        );
    }

    let retrospect = retrospect_success(&run_file(&file, &["retrospect"]), 1);
    let candidate = &retrospect.data["candidates"][0];
    assert_eq!(candidate["type"], "wrapper_alias");
    assert_eq!(candidate["record_ids"].as_array().unwrap().len(), 12);
    assert_eq!(candidate["occurrences"], 12);
    assert_eq!(candidate["evidence"]["texts"].as_array().unwrap().len(), 10);
    assert_eq!(
        candidate["evidence"]["texts"][0],
        "Release build command failed case00"
    );
    assert_eq!(candidate["evidence"]["resolution_notes"], json!([]));
}

#[test]
fn retrospect_counts_each_repeated_title_once_in_cluster_occurrences() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    for index in 0..3 {
        let timestamp = format!("2026-07-09T18:{:02}:00Z", 30 + index);
        add_with_cmd_at(
            &file,
            &timestamp,
            "Release build command failed",
            &["build"],
            "BUILD_MODE=ci /opt/tools/cargo build --release",
        );
    }

    let retrospect = retrospect_success(&run_file(&file, &["retrospect"]), 1);
    let candidate = &retrospect.data["candidates"][0];
    assert_eq!(candidate["type"], "wrapper_alias");
    assert_eq!(candidate["record_ids"].as_array().unwrap().len(), 3);
    // One distinct normalized title with a global count of 3, not 3 x 3.
    assert_eq!(candidate["occurrences"], 3);
}

#[test]
fn retrospect_sums_distinct_titles_in_mixed_clusters() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    add_with_cmd_at(
        &file,
        "2026-07-09T18:30:00Z",
        "Release build command failed",
        &["build"],
        "BUILD_MODE=ci /opt/tools/cargo build --release",
    );
    add_with_cmd_at(
        &file,
        "2026-07-09T18:31:00Z",
        "Release build command failed",
        &["build"],
        "/opt/tools/cargo build --release",
    );
    add_with_cmd_at(
        &file,
        "2026-07-09T18:32:00Z",
        "release build command failed slowly",
        &["build"],
        "/opt/tools/cargo test --workspace",
    );

    let retrospect = retrospect_success(&run_file(&file, &["retrospect"]), 1);
    let candidate = &retrospect.data["candidates"][0];
    assert_eq!(candidate["record_ids"].as_array().unwrap().len(), 3);
    // Two distinct normalized titles: the repeated one counts 2, the other 1.
    assert_eq!(candidate["occurrences"], 3);
}

#[test]
fn schema_exit_codes_name_retrospect_candidates_as_findings() {
    let schema: SuccessEnvelope<Value> = success(&run(&["schema", "exit-codes"]));
    assert_eq!(
        schema.data["exit_codes"]["1"],
        "command findings: doctor unhealthy, triage clusters, verify recurrences, or retrospect candidates"
    );
}

#[test]
fn retrospect_is_deterministic_read_only_and_missing_default_is_empty() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    add_with_cmd_at(
        &file,
        "2026-07-09T18:30:00Z",
        "Build cache compiler fails",
        &["build"],
        "cargo build --release",
    );
    add_with_cmd_at(
        &file,
        "2026-07-09T18:31:00Z",
        "build cache compiler fails again",
        &["build"],
        "cargo build --release",
    );
    let before = std::fs::read(&file).unwrap();
    let first = run_file(&file, &["retrospect"]);
    let second = run_file(&file, &["retrospect"]);
    let _: SuccessEnvelope<Value> = retrospect_success(&first, 1);
    let _: SuccessEnvelope<Value> = retrospect_success(&second, 1);
    assert_eq!(first.stdout, second.stdout);
    assert_eq!(std::fs::read(&file).unwrap(), before);

    let missing_root = temp.path().join("missing-repo");
    let home = temp.path().join("home");
    make_repo(&missing_root);
    std::fs::create_dir_all(&home).unwrap();
    let missing = command()
        .current_dir(&missing_root)
        .env("HOME", &home)
        .arg("retrospect")
        .output()
        .unwrap();
    let missing = retrospect_success(&missing, 0);
    assert_eq!(
        missing.data,
        json!({"candidates": [], "count": 0, "scanned": 0})
    );
    assert_eq!(
        missing.meta.warnings,
        ["no blotter file yet; blotter add creates it"]
    );
}

#[test]
fn schema_documents_retrospect_and_its_no_window_posture() {
    let schema: SuccessEnvelope<Value> = success(&run(&["schema"]));
    let retrospect = &schema.data["commands"]["retrospect"];
    assert_eq!(retrospect["flags"], json!({}));
    assert_eq!(
        retrospect["output"],
        "{candidates:[{type,title,program?,record_ids,resolved_anchor_ids?,occurrences,first_ts,last_ts,evidence:{texts:[...],resolution_notes:[...]}}],count,scanned}"
    );
    assert_eq!(
        retrospect["candidate_types"],
        json!(["wrapper_alias", "doc_repair", "skill_candidate"])
    );
    assert!(
        retrospect["semantics"]
            .as_str()
            .unwrap()
            .contains("retrospect takes no window: chronic signal is long-horizon by design")
    );
    assert!(
        retrospect["semantics"]
            .as_str()
            .unwrap()
            .contains("auto-captures are included by default")
    );
    assert_eq!(
        retrospect["exit_codes"],
        json!({"0":"no promotion candidates","1":"promotion candidates found"})
    );
    assert_eq!(retrospect["read_only"], true);
    assert_eq!(retrospect["appends"], false);
    assert_eq!(retrospect["destructive"], false);

    let help = run(&["retrospect", "--help"]);
    assert!(help.status.success());
    let help = String::from_utf8(help.stdout).unwrap();
    assert!(!help.contains("--since"));
    assert!(!help.contains("--format"));
    error(
        &run(&["retrospect", "--since", "1d"]),
        2,
        "invalid_argument",
    );
    error(
        &run(&["retrospect", "--format", "md"]),
        2,
        "invalid_argument",
    );
}
