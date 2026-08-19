use crate::common::*;

#[test]
fn duplicate_add_warns_that_later_evidence_was_cut() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let first = command()
        .arg("--file")
        .arg(&file)
        .args(["add", "same", "--agent", "tester", "--evidence", "first"])
        .output()
        .unwrap();
    let first: SuccessEnvelope<AddData> = success(&first);
    let second = command()
        .arg("--file")
        .arg(&file)
        .args(["add", "same", "--agent", "tester", "--evidence", "later"])
        .output()
        .unwrap();
    let second: SuccessEnvelope<AddData> = success(&second);
    assert!(!second.data.changed);
    assert_eq!(second.data.record.cut_id(), first.data.record.cut_id());
    assert_eq!(second.meta.warnings.len(), 1);
    assert!(second.meta.warnings[0].starts_with("duplicate_cut:"));
    assert!(second.meta.warnings[0].contains("later evidence was not stored"));
    assert_eq!(
        second.data.record.cut_evidence().unwrap().note.as_deref(),
        Some("first")
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap().lines().count(), 1);
}

#[test]
fn duplicate_add_without_evidence_preserves_pre_range_warning() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    add(&file, "same");
    let no_evidence: SuccessEnvelope<AddData> = success(
        &command()
            .arg("--file")
            .arg(&file)
            .args(["add", "same", "--agent", "tester"])
            .output()
            .unwrap(),
    );
    assert_eq!(
        no_evidence.meta.warnings,
        ["duplicate cut; existing record returned"]
    );
}

#[test]
fn add_resolution_text_warns_without_blocking() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let added: SuccessEnvelope<AddData> = success(
        &command()
            .arg("--file")
            .arg(&file)
            .args(["add", "  RESOLVED: fixed", "--agent", "tester"])
            .output()
            .unwrap(),
    );
    assert!(added.data.changed);
    assert!(added.meta.warnings.iter().any(|warning| {
        warning.starts_with("resolution_text:") && warning.contains("blotter resolve <id>")
    }));
}

#[test]
fn add_stdin_validation_duplicate_and_exact_id() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let mut stdin = command();
    let output = stdin
        .arg("--file")
        .arg(&file)
        .args([
            "add",
            "-",
            "--agent",
            "tester",
            "--severity",
            "major",
            "--tag",
            "z",
            "--tag",
            "a",
        ])
        .write_stdin("ouch\n")
        .output()
        .unwrap();
    let first: SuccessEnvelope<AddData> = success(&output);
    assert_eq!(first.data.record.cut_id(), "bl_a43e5b0b30aa");
    assert_eq!(first.data.record.cut_tags(), ["a", "z"]);

    let second: SuccessEnvelope<AddData> = success(
        &command()
            .arg("--file")
            .arg(&file)
            .args([
                "add",
                "ouch",
                "--agent",
                "tester",
                "--severity",
                "major",
                "--tag",
                "z",
                "--tag",
                "a",
            ])
            .output()
            .unwrap(),
    );
    assert!(!second.data.changed);
    assert_eq!(second.meta.warnings.len(), 1);
    assert_eq!(std::fs::read_to_string(&file).unwrap().lines().count(), 1);

    let blank = command()
        .arg("--file")
        .arg(&file)
        .arg("add")
        .write_stdin(" \n")
        .output()
        .unwrap();
    error(&blank, 65, "invalid_input");
    let large = "x".repeat(10_001);
    error(&run_file(&file, &["add", &large]), 65, "invalid_input");
}

#[test]
fn add_duplicate_tags_share_the_deduped_cut_id() {
    let temp = TempDir::new().unwrap();
    let duplicate_file = temp.path().join("duplicate-tags.jsonl");
    let unique_file = temp.path().join("unique-tags.jsonl");
    let duplicate: SuccessEnvelope<AddData> = success(&run_file(
        &duplicate_file,
        &[
            "add", "same cut", "--agent", "tester", "--tag", "a", "--tag", "a", "--tag", "b",
        ],
    ));
    let unique: SuccessEnvelope<AddData> = success(&run_file(
        &unique_file,
        &[
            "add", "same cut", "--agent", "tester", "--tag", "a", "--tag", "b",
        ],
    ));

    assert_eq!(duplicate.data.record.cut_id(), unique.data.record.cut_id());
    assert_eq!(duplicate.data.record.cut_tags(), ["a", "b"]);
    assert_eq!(unique.data.record.cut_tags(), ["a", "b"]);
    let stored: Value =
        serde_json::from_str(&std::fs::read_to_string(&duplicate_file).unwrap()).unwrap();
    assert_eq!(stored["tags"], json!(["a", "b"]));
}

#[test]
fn fold_deduplicates_tags_from_existing_cut_and_dogear_records() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("legacy-duplicate-tags.jsonl");
    let cut = json!({
        "kind": "cut",
        "id": "pc_a1b2c3d4e5f6",
        "ts": "2026-07-09T00:00:00.000Z",
        "agent": "legacy",
        "text": "legacy cut",
        "tags": ["b", "a", "a"],
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
        "tags": ["b", "a", "a"],
        "cwd": "/tmp",
        "repo": "/tmp/repo"
    });
    std::fs::write(&file, format!("{cut}\n{dogear}\n")).unwrap();

    let listed: SuccessEnvelope<ListData> = success(&run_file(
        &file,
        &["list", "--kind", "all", "--status", "all"],
    ));
    assert!(listed.data.items.iter().all(|item| item.tags == ["a", "b"]));
}

#[test]
fn records_inside_a_repo_store_relative_cwd_without_repo() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("repo");
    let nested = root.join("nested");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::create_dir(root.join(".git")).unwrap();

    let add: SuccessEnvelope<Value> = success(
        &command()
            .current_dir(&nested)
            .args(["add", "inside repo", "--agent", "tester"])
            .output()
            .unwrap(),
    );
    let dogear: SuccessEnvelope<Value> = success(
        &command()
            .current_dir(&nested)
            .args(["dogear", "inside repo idea", "--agent", "tester"])
            .output()
            .unwrap(),
    );
    for record in [&add.data["record"], &dogear.data["record"]] {
        assert_eq!(record["cwd"], "nested");
        assert!(record.get("repo").is_none());
    }

    let stored: Vec<Value> = std::fs::read_to_string(root.join(".blotter.jsonl"))
        .unwrap()
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(stored.len(), 2);
    for record in stored {
        assert_eq!(record["cwd"], "nested");
        assert!(record.get("repo").is_none());
    }
}

#[test]
fn records_outside_a_repo_keep_absolute_cwd_without_repo() {
    let temp = TempDir::new().unwrap();
    if temp_has_git_ancestor(&temp) {
        eprintln!("skipping no-repo cwd assertion inside a git checkout");
        return;
    }
    let outside = temp.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    let file = outside.join("cuts.jsonl");
    let expected_cwd = outside
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned();

    let add: SuccessEnvelope<Value> = success(
        &command()
            .current_dir(&outside)
            .arg("--file")
            .arg(&file)
            .args(["add", "outside repo", "--agent", "tester"])
            .output()
            .unwrap(),
    );
    let dogear: SuccessEnvelope<Value> = success(
        &command()
            .current_dir(&outside)
            .arg("--file")
            .arg(&file)
            .args(["dogear", "outside repo idea", "--agent", "tester"])
            .output()
            .unwrap(),
    );
    for record in [&add.data["record"], &dogear.data["record"]] {
        assert_eq!(record["cwd"], expected_cwd);
        assert!(record.get("repo").is_none());
    }
}

#[test]
fn records_under_home_use_tilde_cwd_without_crossing_component_boundaries() {
    let temp = TempDir::new().unwrap();
    if temp_has_git_ancestor(&temp) {
        eprintln!("skipping home cwd assertion inside a git checkout");
        return;
    }
    let users = temp.path().join("Users");
    let home = users.join("alice");
    let nested = home.join("project");
    let adjacent = users.join("alicex");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::create_dir_all(&adjacent).unwrap();
    let home = home.canonicalize().unwrap();
    let nested = nested.canonicalize().unwrap();
    let adjacent = adjacent.canonicalize().unwrap();

    for (name, cwd, expected) in [("nested", &nested, "~/project"), ("home", &home, "~")] {
        let file = temp.path().join(format!("{name}.jsonl"));
        let added: SuccessEnvelope<AddData> = success(
            &command()
                .env("HOME", &home)
                .current_dir(cwd)
                .arg("--file")
                .arg(&file)
                .args(["add", "home cwd", "--agent", "tester"])
                .output()
                .unwrap(),
        );
        assert_eq!(added.data.record.cut_cwd(), expected);
    }

    let dogear: SuccessEnvelope<Value> = success(
        &command()
            .env("HOME", &home)
            .current_dir(&nested)
            .arg("--file")
            .arg(temp.path().join("dogear.jsonl"))
            .args(["dogear", "home cwd", "--agent", "tester"])
            .output()
            .unwrap(),
    );
    assert_eq!(dogear.data["record"]["cwd"], "~/project");

    let file = temp.path().join("adjacent.jsonl");
    let added: SuccessEnvelope<AddData> = success(
        &command()
            .env("HOME", &home)
            .current_dir(&adjacent)
            .arg("--file")
            .arg(&file)
            .args(["add", "adjacent cwd", "--agent", "tester"])
            .output()
            .unwrap(),
    );
    assert_eq!(
        added.data.record.cut_cwd(),
        adjacent.to_string_lossy().as_ref()
    );
}

#[test]
fn add_stores_absolute_cwd_when_the_log_lives_outside_the_repo() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("repo");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir(root.join(".git")).unwrap();
    let outside = temp.path().join("machine-local.jsonl");

    // A log outside the repo is machine-local: repo-relative cwd would strip
    // the only provenance the record has now that repo fields are gone.
    let added: SuccessEnvelope<AddData> = success(
        &command()
            .current_dir(&root)
            .arg("--file")
            .arg(&outside)
            .args(["add", "outside log case", "--agent", "tester"])
            .output()
            .unwrap(),
    );
    let cwd = added.data.record.cut_cwd();
    assert!(
        std::path::Path::new(cwd).is_absolute(),
        "expected absolute cwd, got {cwd}"
    );
}

#[test]
fn duplicate_add_returns_the_existing_record_with_normalized_tags() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let args = [
        "add",
        "legacy tag order",
        "--agent",
        "tester",
        "--tag",
        "zeta",
        "--tag",
        "alpha",
    ];
    let first: SuccessEnvelope<AddData> = success(&run_file(&file, &args));
    assert_eq!(first.data.record.cut_tags(), ["alpha", "zeta"]);

    // Rewrite the stored line with a legacy unsorted tag array. The ID hashes
    // sorted tags, so the duplicate still matches, and the sentinel record the
    // append path returns must come back normalized.
    let mut stored: Value =
        serde_json::from_str(std::fs::read_to_string(&file).unwrap().trim()).unwrap();
    stored["tags"] = json!(["zeta", "alpha"]);
    std::fs::write(&file, format!("{stored}\n")).unwrap();

    let duplicate: SuccessEnvelope<AddData> = success(&run_file(&file, &args));
    assert!(!duplicate.data.changed);
    assert_eq!(duplicate.data.record.cut_tags(), ["alpha", "zeta"]);
    assert_eq!(std::fs::read_to_string(&file).unwrap().lines().count(), 1);
}

#[test]
fn stdin_text_over_10000_bytes_that_redacts_smaller_is_accepted() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("fakehome");
    std::fs::create_dir_all(&home).unwrap();
    let file = temp.path().join("cuts.jsonl");
    let raw = "/Users/verylongusername/deep/path ".repeat(400);
    assert!(raw.len() > 10_000);

    // r25: the text is redacted first, and `validate_text`'s 10000-byte limit
    // measures the redacted text, so the raw read cannot be capped at 10000.
    let added: SuccessEnvelope<Value> = success(
        &command()
            .env("HOME", &home)
            .arg("--file")
            .arg(&file)
            .args(["add", "-", "--agent", "tester"])
            .write_stdin(raw)
            .output()
            .unwrap(),
    );
    assert_eq!(added.data["changed"], true);
    assert_eq!(
        added.data["record"]["text"].as_str().unwrap(),
        "~/deep/path ".repeat(400)
    );
}

#[test]
fn stdin_text_over_the_raw_read_limit_is_rejected() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let oversized = vec![b'x'; 1024 * 1024 + 1];

    let output = command()
        .arg("--file")
        .arg(&file)
        .args(["add", "-", "--agent", "tester"])
        .write_stdin(oversized)
        .output()
        .unwrap();
    let envelope = error(&output, 65, "invalid_input");
    assert!(
        envelope
            .error
            .message
            .contains("exceeds the 1048576-byte read limit")
    );
    assert!(!file.exists());
}

#[test]
fn stdin_text_at_the_read_limit_followed_by_a_newline_and_more_input_is_rejected() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    // The reader stops at the cap having consumed a trailing newline. Trimming
    // that newline before the length test would drop the buffer back to the
    // limit and accept, silently discarding everything past it.
    let mut oversized = vec![b'x'; 1024 * 1024];
    oversized.push(b'\n');
    oversized.extend_from_slice(b"discarded suffix");

    let output = command()
        .arg("--file")
        .arg(&file)
        .args(["add", "-", "--agent", "tester"])
        .write_stdin(oversized)
        .output()
        .unwrap();
    let envelope = error(&output, 65, "invalid_input");
    assert!(
        envelope
            .error
            .message
            .contains("exceeds the 1048576-byte read limit")
    );
    assert!(!file.exists());
}
