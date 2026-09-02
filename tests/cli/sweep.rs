use crate::common::*;

#[test]
fn sweep_aggregates_repos_sorts_paths_and_skips_missing() {
    let temp = TempDir::new().unwrap();
    let alpha = temp.path().join("alpha");
    let beta = temp.path().join("beta");
    make_repo(&alpha);
    make_repo(&beta);
    let alpha_file = alpha.join(".blotter.jsonl");
    let beta_file = beta.join(".blotter.jsonl");
    let alpha_cut = add_at(&alpha_file, "2026-07-09T17:00:00Z", "Alpha cut", &["api"]);
    let beta_cut = add_at(
        &beta_file,
        "2026-07-09T16:00:00Z",
        "Beta cut",
        &["api", "build"],
    );
    let missing = temp.path().join("missing.jsonl");

    let sweep: SuccessEnvelope<Value> = success(
        &command()
            .arg("sweep")
            .arg(&beta)
            .arg(&missing)
            .arg(&alpha)
            .output()
            .unwrap(),
    );
    let alpha_path = alpha_file.canonicalize().unwrap();
    let beta_path = beta_file.canonicalize().unwrap();
    assert_eq!(
        sweep.data["repos"],
        json!([
            {
                "path": alpha_path,
                "counts": {"open_cuts": 1, "open_dogears": 0},
                "by_tag": [{"tag": "api", "count": 1}],
                "items": [{
                    "kind": "cut",
                    "id": alpha_cut.data.record.cut_id(),
                    "ts": "2026-07-09T17:00:00.000Z",
                    "agent": "tester",
                    "text": "Alpha cut",
                    "tags": ["api"],
                    "impact": "low",
                    "cwd": alpha_cut.data.record.cut_cwd(),
                    "origin": {"type":"agent"},
                    "status": "open",
                }],
            },
            {
                "path": beta_path,
                "counts": {"open_cuts": 1, "open_dogears": 0},
                "by_tag": [
                    {"tag": "api", "count": 1},
                    {"tag": "build", "count": 1},
                ],
                "items": [{
                    "kind": "cut",
                    "id": beta_cut.data.record.cut_id(),
                    "ts": "2026-07-09T16:00:00.000Z",
                    "agent": "tester",
                    "text": "Beta cut",
                    "tags": ["api", "build"],
                    "impact": "low",
                    "cwd": beta_cut.data.record.cut_cwd(),
                    "origin": {"type":"agent"},
                    "status": "open",
                }],
            },
        ])
    );
    assert_eq!(
        sweep.data["totals"],
        json!({
            "repos_swept": 2,
            "repos_skipped": 1,
            "open_cuts": 2,
            "open_dogears": 0,
        })
    );
    assert!(sweep.meta.warnings.iter().any(|warning| {
        warning.starts_with("skipped ") && warning.contains(missing.to_str().unwrap())
    }));
}

#[test]
fn sweep_registry_uses_relative_paths_and_deduplicates_positionals() {
    let temp = TempDir::new().unwrap();
    let alpha = temp.path().join("repos/alpha");
    let beta = temp.path().join("repos/beta");
    make_repo(&alpha);
    make_repo(&beta);
    add(&alpha.join(".blotter.jsonl"), "Alpha registry cut");
    add(&beta.join(".blotter.jsonl"), "Beta registry cut");
    let registry_dir = temp.path().join("registry");
    std::fs::create_dir_all(&registry_dir).unwrap();
    let registry = registry_dir.join("repos.txt");
    std::fs::write(
        &registry,
        "# known repos\n\n../repos/beta\n../repos/alpha\n",
    )
    .unwrap();

    let sweep: SuccessEnvelope<Value> = success(
        &command()
            .arg("sweep")
            .arg(&alpha)
            .arg("--registry")
            .arg(&registry)
            .output()
            .unwrap(),
    );
    let paths: Vec<_> = sweep.data["repos"]
        .as_array()
        .unwrap()
        .iter()
        .map(|repo| repo["path"].as_str().unwrap())
        .collect();
    assert_eq!(paths.len(), 2);
    assert!(paths.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(sweep.data["totals"]["repos_swept"], 2);
    assert_eq!(sweep.data["totals"]["repos_skipped"], 0);
}

#[test]
fn sweep_requires_paths_and_all_missing_is_successful() {
    let no_paths = run(&["sweep"]);
    let no_paths = error(&no_paths, 2, "invalid_argument");
    assert_eq!(no_paths.error.message, "nothing to sweep");

    let temp = TempDir::new().unwrap();
    let first = temp.path().join("first.jsonl");
    let second = temp.path().join("second.jsonl");
    let sweep: SuccessEnvelope<Value> = success(
        &command()
            .arg("sweep")
            .arg(&first)
            .arg(&second)
            .output()
            .unwrap(),
    );
    assert_eq!(sweep.data["repos"], json!([]));
    assert_eq!(
        sweep.data["totals"],
        json!({
            "repos_swept": 0,
            "repos_skipped": 2,
            "open_cuts": 0,
            "open_dogears": 0,
        })
    );
    assert_eq!(sweep.meta.warnings.len(), 2);
    assert!(
        sweep
            .meta
            .warnings
            .iter()
            .all(|warning| warning.starts_with("skipped "))
    );
}

#[test]
fn sweep_filters_items_and_ignores_blotter_file() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path().join("repo");
    make_repo(&repo);
    let file = repo.join(".blotter.jsonl");
    let old_cut = add_at(&file, "2026-07-01T00:00:00Z", "Old cut", &["old"]);
    let recent_cut = add_at(&file, "2026-07-09T17:00:00Z", "Recent cut", &["recent"]);
    let recent_dogear = dogear_at(&file, "2026-07-09T17:30:00Z", "Recent dogear", &["ideas"]);
    let env_file = temp.path().join("env.jsonl");
    add(&env_file, "Must not appear");

    let default: SuccessEnvelope<Value> = success(
        &command()
            .env("BLOTTER_FILE", &env_file)
            .arg("sweep")
            .arg(&repo)
            .output()
            .unwrap(),
    );
    assert_eq!(
        default.data["repos"][0]["counts"],
        json!({"open_cuts":2,"open_dogears":1})
    );
    assert!(
        default.data["repos"][0]["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|item| item["kind"] == "cut")
    );
    assert!(
        default.data["repos"][0]["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|item| item["text"] != "Must not appear")
    );

    let filtered: SuccessEnvelope<Value> = success(
        &command()
            .env("BLOTTER_FILE", &env_file)
            .arg("sweep")
            .arg(&repo)
            .args(["--kind", "all", "--since", "1d"])
            .output()
            .unwrap(),
    );
    let ids: Vec<_> = filtered.data["repos"][0]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        [
            recent_cut.data.record.cut_id(),
            recent_dogear.data["record"]["id"].as_str().unwrap(),
        ]
    );
    assert!(!ids.contains(&old_cut.data.record.cut_id()));
}

#[test]
fn sweep_caps_items_per_repo_without_capping_tag_counts() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path().join("repo");
    make_repo(&repo);
    let file = repo.join(".blotter.jsonl");
    for number in 0..51 {
        add_at(
            &file,
            "2026-07-09T17:00:00Z",
            &format!("Capped cut {number}"),
            &["build"],
        );
    }

    let sweep: SuccessEnvelope<Value> =
        success(&command().arg("sweep").arg(&repo).output().unwrap());
    assert_eq!(sweep.data["repos"][0]["counts"]["open_cuts"], 51);
    assert_eq!(
        sweep.data["repos"][0]["items"].as_array().unwrap().len(),
        50
    );
    assert_eq!(sweep.data["repos"][0]["truncated"], true);
    assert_eq!(
        sweep.data["repos"][0]["by_tag"],
        json!([{ "tag": "build", "count": 51 }])
    );
}

#[test]
fn sweep_is_byte_deterministic_and_read_only() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path().join("repo");
    make_repo(&repo);
    let file = repo.join(".blotter.jsonl");
    add_at(
        &file,
        "2026-07-09T17:00:00Z",
        "Deterministic cut",
        &["build"],
    );
    let before = std::fs::read(&file).unwrap();

    let first = command().arg("sweep").arg(&repo).output().unwrap();
    let second = command().arg("sweep").arg(&repo).output().unwrap();
    let _: SuccessEnvelope<SweepData> = success(&first);
    let _: SuccessEnvelope<SweepData> = success(&second);
    assert_eq!(first.stdout, second.stdout);
    assert_eq!(std::fs::read(&file).unwrap(), before);
}

#[test]
fn sweep_skips_lock_timeouts_with_retryable_warning() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path().join("repo");
    make_repo(&repo);
    let file = repo.join(".blotter.jsonl");
    add(&file, "locked sweep repo");
    let locked = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&file)
        .unwrap();
    locked.lock().unwrap();
    let output = command().arg("sweep").arg(&repo).output().unwrap();
    locked.unlock().unwrap();

    let sweep: SuccessEnvelope<SweepData> = success(&output);
    assert!(sweep.data.repos.is_empty());
    assert_eq!(sweep.data.totals.repos_skipped, 1);
    assert_eq!(
        sweep.meta.warnings,
        [format!(
            "skipped {}: lock timeout (retryable)",
            file.canonicalize().unwrap().display()
        )]
    );
}

#[test]
fn sweep_rejects_global_file_flag() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path().join("repo");
    make_repo(&repo);
    let output = command()
        .arg("--file")
        .arg(temp.path().join("override.jsonl"))
        .arg("sweep")
        .arg(&repo)
        .output()
        .unwrap();

    let envelope = error(&output, 2, "invalid_argument");
    assert_eq!(envelope.error.message, "--file conflicts with sweep");
    assert!(envelope.error.suggested_fix.contains("repository paths"));
    assert!(envelope.error.suggested_fix.contains("--registry"));
}

#[test]
fn schema_documents_sweep() {
    let schema: SuccessEnvelope<Value> = success(&run(&["schema"]));
    let sweep = &schema.data["commands"]["sweep"];
    assert!(sweep["positional"].as_str().unwrap().contains("PATH"));
    assert!(
        sweep["flags"]["--registry"]
            .as_str()
            .unwrap()
            .contains("relative paths")
    );
    // r35: the rejection is part of the published contract, not just an
    // internal mapping, so an agent can predict 65 without provoking it.
    assert!(
        sweep["flags"]["--registry"]
            .as_str()
            .unwrap()
            .contains("invalid_input (65)")
    );
    assert_eq!(sweep["flags"]["--since"], "full RFC3339|Nd|Nh; optional");
    assert_eq!(sweep["flags"]["--kind"], "cut|dogear|all; default cut");
    assert!(sweep["flags"].get("--include-auto").is_none());
    assert!(sweep["output"].as_str().unwrap().contains("repos_skipped"));
    assert!(
        sweep["semantics"]
            .as_str()
            .unwrap()
            .contains("BLOTTER_FILE is ignored")
    );
    assert!(
        sweep["semantics"]
            .as_str()
            .unwrap()
            .contains("lock timeouts")
    );
    assert!(
        sweep["semantics"]
            .as_str()
            .unwrap()
            .contains("--file conflicts")
    );
    assert_eq!(sweep["read_only"], true);
    assert_eq!(sweep["appends"], false);
    assert_eq!(sweep["destructive"], false);
}

#[test]
fn sweep_missing_registry_is_not_found_66() {
    let temp = TempDir::new().unwrap();
    let missing = temp.path().join("repos.txt");

    let output = command()
        .args(["sweep", "--registry"])
        .arg(&missing)
        .output()
        .unwrap();
    let envelope = error(&output, 66, "not_found");
    assert!(envelope.error.message.contains("registry file not found"));
    assert!(envelope.error.suggested_fix.contains("--registry"));
}

#[cfg(unix)]
#[test]
fn sweep_unreadable_registry_is_permission_denied_77() {
    let temp = TempDir::new().unwrap();
    let registry = temp.path().join("repos.txt");
    std::fs::write(&registry, "").unwrap();
    std::fs::set_permissions(&registry, std::fs::Permissions::from_mode(0o000)).unwrap();

    let output = command()
        .args(["sweep", "--registry"])
        .arg(&registry)
        .output()
        .unwrap();
    std::fs::set_permissions(&registry, std::fs::Permissions::from_mode(0o600)).unwrap();
    error(&output, 77, "permission_denied");
}

/// A registry the decoder cannot read is a wrong input, not a failing
/// filesystem: `read_to_string` reports non-UTF-8 bytes as `InvalidData`, which
/// `from_io` would have published as the generic `io_error` (74).
#[test]
fn sweep_non_utf8_registry_is_invalid_input_65() {
    let temp = TempDir::new().unwrap();
    let registry = temp.path().join("repos.txt");
    std::fs::write(&registry, b"alpha\n\xff\xfe\n").unwrap();

    let output = command()
        .args(["sweep", "--registry"])
        .arg(&registry)
        .output()
        .unwrap();
    let envelope = error(&output, 65, "invalid_input");
    assert!(envelope.error.message.contains("not valid UTF-8"));
    assert!(envelope.error.suggested_fix.contains("UTF-8"));
}

/// A directory answers the same 65 as a non-UTF-8 file and as a directory log
/// path, rather than the 74 the read failure would otherwise carry.
#[test]
fn sweep_directory_registry_is_invalid_input_65() {
    let temp = TempDir::new().unwrap();
    let registry = temp.path().join("registry-directory");
    std::fs::create_dir(&registry).unwrap();

    let output = command()
        .args(["sweep", "--registry"])
        .arg(&registry)
        .output()
        .unwrap();
    let envelope = error(&output, 65, "invalid_input");
    assert!(envelope.error.message.contains("not a regular file"));
    assert!(envelope.error.suggested_fix.contains("--registry"));
}

/// r48: `sweep` names a refused log in its per-log skip warning list and keeps
/// exit 0, exactly as it does for every per-log failure (r13). One v1 log does
/// not abort a multi-log sweep.
#[test]
fn sweep_skips_a_v1_log_and_keeps_exit_zero() {
    let temp = TempDir::new().unwrap();
    let good = temp.path().join("good");
    let stale = temp.path().join("stale");
    make_repo(&good);
    make_repo(&stale);
    let good_file = good.join(".blotter.jsonl");
    let stale_file = stale.join(".blotter.jsonl");
    add_at(&good_file, "2026-07-09T17:00:00Z", "Good cut", &["api"]);
    std::fs::write(&stale_file, format!("{}\n", v1_cut_line())).unwrap();

    let run_sweep = || {
        let output = command()
            .arg("sweep")
            .arg(&good)
            .arg(&stale)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0));
        let envelope: SuccessEnvelope<SweepData> = success(&output);
        envelope
    };

    let swept = run_sweep();
    assert_eq!(swept.data.totals.repos_swept, 1);
    assert_eq!(swept.data.totals.repos_skipped, 1);
    assert_eq!(swept.data.totals.open_cuts, 1);
    assert_eq!(swept.data.totals.open_dogears, 0);
    // No entry for the skipped log appears in repos[].
    assert_eq!(swept.data.repos.len(), 1);
    assert!(!swept.data.repos[0].path.contains("stale"));
    let expected = format!(
        "skipped {}: unsupported log version on line 1: record has no v field",
        stale_file.canonicalize().unwrap().display()
    );
    assert_eq!(swept.meta.warnings, [expected]);

    // Deterministic: two runs over the same inputs produce byte-identical
    // warnings.
    assert_eq!(run_sweep().meta.warnings, swept.meta.warnings);
    assert_eq!(
        std::fs::read_to_string(&stale_file).unwrap(),
        format!("{}\n", v1_cut_line())
    );
}
