use crate::common::*;

#[test]
fn help_version_and_schema_never_touch_blotter_file() {
    fn assert_startup_commands_ignore_store(file: &Path) {
        for args in [&["--help"][..], &["--version"][..], &["schema"][..]] {
            let output = command()
                .env("BLOTTER_FILE", file)
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "args={args:?} status={:?} stderr={}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                output.stderr.is_empty(),
                "args={args:?} stderr={}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                !output.stdout.is_empty(),
                "args={args:?} produced no stdout"
            );
        }
    }

    let temp = TempDir::new().unwrap();
    let nonexistent = temp.path().join("does-not-exist.jsonl");
    assert_startup_commands_ignore_store(&nonexistent);
    assert!(!nonexistent.exists());

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let unreadable = temp.path().join("unreadable.jsonl");
        let sentinel = b"do not touch this store";
        std::fs::write(&unreadable, sentinel).unwrap();
        let original_permissions = std::fs::metadata(&unreadable).unwrap().permissions();
        let mut unreadable_permissions = original_permissions.clone();
        unreadable_permissions.set_mode(0o000);
        std::fs::set_permissions(&unreadable, unreadable_permissions).unwrap();

        assert_startup_commands_ignore_store(&unreadable);

        std::fs::set_permissions(&unreadable, original_permissions).unwrap();
        assert_eq!(std::fs::read(&unreadable).unwrap(), sentinel);
    }
}

#[test]
fn evidence_and_resolve_response_shapes_are_exactly_compatible() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let added = add(&file, "no evidence");
    let add_json: Value =
        serde_json::from_slice(&run_file(&file, &["add", "another", "--agent", "tester"]).stdout)
            .unwrap();
    let add_data = add_json["data"].as_object().unwrap();
    assert_eq!(
        add_data.keys().map(String::as_str).collect::<Vec<_>>(),
        ["changed", "record"]
    );
    let record = add_data["record"].as_object().unwrap();
    assert_eq!(
        record.keys().map(String::as_str).collect::<Vec<_>>(),
        [
            "kind", "id", "ts", "agent", "text", "tags", "severity", "cwd"
        ]
    );
    assert!(record.get("repo").is_none());
    assert!(record.get("evidence").is_none());
    let log_text = std::fs::read_to_string(&file).unwrap();
    let log: Value = serde_json::from_str(log_text.lines().next().unwrap()).unwrap();
    assert_eq!(log, serde_json::to_value(&added.data.record).unwrap());

    let partial: SuccessEnvelope<AddData> = success(&run_file(
        &file,
        &[
            "add",
            "partial evidence",
            "--agent",
            "tester",
            "--exit",
            "1",
        ],
    ));
    assert_eq!(
        serde_json::to_value(partial.data.record.cut_evidence().unwrap())
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["exit"]
    );

    let one: Value =
        serde_json::from_slice(&run_file(&file, &["resolve", added.data.record.cut_id()]).stdout)
            .unwrap();
    assert_eq!(
        one["data"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["changed", "records"]
    );
    let one_records = one["data"]["records"].as_array().unwrap();
    assert_eq!(one_records.len(), 1);
    let one_record = one_records[0].as_object().unwrap();
    assert_eq!(
        one_record.keys().map(String::as_str).collect::<Vec<_>>(),
        [
            "kind",
            "id",
            "ts",
            "agent",
            "text",
            "tags",
            "severity",
            "cwd",
            "status",
            "resolution"
        ]
    );
    assert_eq!(one_record["kind"], "cut");
    assert_eq!(one_record["id"], added.data.record.cut_id());
    assert_eq!(one_record["ts"], "2026-07-09T18:30:00.123Z");
    assert_eq!(one_record["agent"], "tester");
    assert_eq!(one_record["text"], "no evidence");
    assert_eq!(one_record["tags"], json!([]));
    assert_eq!(one_record["severity"], "minor");
    assert_eq!(one_record["cwd"], added.data.record.cut_cwd());
    assert!(one_record.get("repo").is_none());
    assert_eq!(one_record["status"], "resolved");
    assert_eq!(
        one_record["resolution"],
        json!({"agent":"unknown","note":null,"ts":"2026-07-09T18:30:00.123Z"})
    );
    let second = partial.data.record.cut_id();
    let third: SuccessEnvelope<AddData> =
        success(&run_file(&file, &["add", "third", "--agent", "tester"]));
    let many: Value = serde_json::from_slice(
        &run_file(&file, &["resolve", second, third.data.record.cut_id()]).stdout,
    )
    .unwrap();
    assert_eq!(
        many["data"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["changed", "records"]
    );
    let records = many["data"]["records"].as_array().unwrap();
    assert_eq!(records.len(), 2);
    for record in records {
        let record = record.as_object().unwrap();
        assert_eq!(record["kind"], "cut");
        assert_eq!(record["status"], "resolved");
        assert_eq!(record["severity"], "minor");
        assert_eq!(record["tags"], json!([]));
        assert_eq!(record["ts"], "2026-07-09T18:30:00.123Z");
        assert_eq!(record["agent"], "tester");
        assert_eq!(record["resolution"]["ts"], "2026-07-09T18:30:00.123Z");
        assert_eq!(record["resolution"]["agent"], "unknown");
        assert_eq!(record["resolution"]["note"], Value::Null);
        match record["id"].as_str().unwrap() {
            id if id == second => {
                assert_eq!(
                    record.keys().map(String::as_str).collect::<Vec<_>>(),
                    [
                        "kind",
                        "id",
                        "ts",
                        "agent",
                        "text",
                        "tags",
                        "severity",
                        "cwd",
                        "evidence",
                        "status",
                        "resolution"
                    ]
                );
                assert_eq!(record["text"], "partial evidence");
                assert_eq!(record["cwd"], partial.data.record.cut_cwd());
                assert!(record.get("repo").is_none());
                assert_eq!(record["evidence"], json!({"exit": 1}));
            }
            id if id == third.data.record.cut_id() => {
                assert_eq!(
                    record.keys().map(String::as_str).collect::<Vec<_>>(),
                    [
                        "kind",
                        "id",
                        "ts",
                        "agent",
                        "text",
                        "tags",
                        "severity",
                        "cwd",
                        "status",
                        "resolution"
                    ]
                );
                assert_eq!(record["text"], "third");
                assert_eq!(record["cwd"], third.data.record.cut_cwd());
                assert!(record.get("repo").is_none());
                assert!(!record.contains_key("evidence"));
            }
            id => panic!("unexpected resolved record {id}"),
        }
    }
}

#[test]
fn every_command_success_envelope_deserializes() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let added = add(&file, "first cut");
    assert!(added.ok);
    assert!(added.data.changed);
    assert_eq!(added.data.record.cut_ts(), "2026-07-09T18:30:00.123Z");
    assert_eq!(added.meta.agent_source.as_deref(), Some("flag"));

    let listed: SuccessEnvelope<ListData> = success(&run_file(&file, &["list", "--status", "all"]));
    assert_eq!(listed.data.count, 1);

    let digest: SuccessEnvelope<Value> = success(&run_file(&file, &["digest"]));
    assert_eq!(digest.data["new_cuts"]["count"], 1);

    let verify: SuccessEnvelope<Value> = success(&run_file(&file, &["verify"]));
    assert_eq!(verify.data["count"], 0);

    let sweep: SuccessEnvelope<Value> =
        success(&command().arg("sweep").arg(&file).output().unwrap());
    assert_eq!(sweep.data["totals"]["repos_swept"], 1);

    let resolved: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &[
            "resolve",
            added.data.record.cut_id(),
            "--agent",
            "fixer",
            "--note",
            "fixed",
        ],
    ));
    assert!(resolved.data.changed);
    assert_eq!(resolved.data.records.len(), 1);
    assert_eq!(resolved.data.records[0].status, ItemStatus::Resolved);
    assert_eq!(
        resolved.data.records[0]
            .resolution
            .as_ref()
            .unwrap()
            .note
            .as_deref(),
        Some("fixed")
    );

    let doctor_output = run_file(&file, &["doctor"]);
    let doctor: SuccessEnvelope<DoctorData> = success(&doctor_output);
    assert!(doctor.data.healthy);
    assert_eq!(doctor.data.checked_lines, 2);

    let schema: SuccessEnvelope<Value> = success(&run(&["schema"]));
    assert_eq!(schema.data["contract"], 6);
    assert_eq!(schema.data["exit_codes"]["74"], "I/O error");
    assert_eq!(schema.data["commands"]["doctor"]["read_only"], true);
    assert!(
        schema.data["commands"]["doctor"]["flags"]["--leaks"]
            .as_str()
            .unwrap()
            .contains("home")
    );
    assert!(
        schema.data["commands"]["doctor"]["flags"]["--leaks"]
            .as_str()
            .unwrap()
            .contains("conflicts")
    );
    assert!(
        schema.data["commands"]["doctor"]["flags"]["--deny"]
            .as_str()
            .unwrap()
            .contains("--leaks")
    );
    assert!(
        schema.data["commands"]["doctor"]["flags"]["--deny"]
            .as_str()
            .unwrap()
            .contains("non-empty")
    );
    assert!(
        schema.data["commands"]["doctor"]["flags"]["--deny"]
            .as_str()
            .unwrap()
            .contains("conflicts")
    );
    assert!(
        schema.data["commands"]["doctor"]["semantics"]
            .as_str()
            .unwrap()
            .contains("leak")
    );
    assert!(
        schema.data["commands"]["doctor"]["semantics"]
            .as_str()
            .unwrap()
            .contains("read-only")
    );
    assert_eq!(
        schema.data["records"]["cut"]["cwd"],
        "repo-relative path when cwd is inside the discovered repo; ~-relative when under the home directory; otherwise the absolute path with home prefixes rewritten to ~"
    );
    assert!(
        schema.data["commands"]["add"]["flags"]["--stderr-file"]
            .as_str()
            .unwrap()
            .contains("4096")
    );
    assert_eq!(
        schema.data["commands"]["resolve"]["output"],
        "{changed,records:[...]}; always an array, including one ID; IDs are canonicalized, sorted, and duplicate inputs collapse; non-amend mixed already-resolved IDs warn with a sorted count/list; amend requires every requested record already be resolved and appends the batch atomically"
    );

    let expected = serde_json::to_value(exit_code_map()).unwrap();
    assert_eq!(schema.data["exit_codes"], expected);
    assert_eq!(
        schema.data["id"]["cut"]["fields_in_order"],
        json!([
            "literal bl1",
            "literal cut",
            "ts",
            "agent",
            "text",
            "severity",
            "tag count",
            "each sorted unique tag as its own field"
        ])
    );
}

#[test]
fn since_duration_overflow_returns_invalid_argument_for_read_commands() {
    fn assert_too_large_since(output: &std::process::Output, value: &str) {
        let envelope = error(output, 2, "invalid_argument");
        assert_eq!(
            envelope.error.message,
            format!("--since value '{value}' is too large")
        );
        assert_eq!(
            envelope.error.suggested_fix,
            "Use a smaller Nd or Nh duration."
        );
    }

    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    std::fs::write(&file, "").unwrap();
    let repo = temp.path().join("repo");
    make_repo(&repo);
    std::fs::write(repo.join(".blotter.jsonl"), "").unwrap();

    for value in [
        "2562047788015216h",
        "106751991167301d",
        "9223372036854775807h",
        "9223372036854775807d",
    ] {
        assert_too_large_since(&run_file(&file, &["list", "--since", value]), value);
        assert_too_large_since(&run_file(&file, &["digest", "--since", value]), value);
        assert_too_large_since(
            &command()
                .arg("sweep")
                .arg(&repo)
                .args(["--since", value])
                .output()
                .unwrap(),
            value,
        );
    }
}

#[test]
fn since_duration_boundary_and_valid_windows_are_stable() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    for (now, text) in [
        ("2026-07-02T18:30:00.123Z", "seven-day boundary"),
        ("2026-07-09T06:30:00.123Z", "twelve-hour boundary"),
        ("2026-07-09T18:30:00.123Z", "zero-day boundary"),
    ] {
        add_at(&file, now, text, &[]);
    }

    for value in ["2562047788015215h", "2562047788015216h"] {
        error(
            &run_file(&file, &["list", "--since", value]),
            2,
            "invalid_argument",
        );
    }

    for (relative, absolute) in [
        ("7d", "2026-07-02T18:30:00.123Z"),
        ("12h", "2026-07-09T06:30:00.123Z"),
        ("0d", "2026-07-09T18:30:00.123Z"),
    ] {
        let relative_output = run_file(&file, &["list", "--status", "all", "--since", relative]);
        let absolute_output = run_file(&file, &["list", "--status", "all", "--since", absolute]);
        let _: SuccessEnvelope<ListData> = success(&relative_output);
        let _: SuccessEnvelope<ListData> = success(&absolute_output);
        assert_eq!(relative_output.stdout, absolute_output.stdout);
    }
}

#[cfg(unix)]
#[test]
fn stdout_write_failures_are_structured_for_all_output_paths() {
    fn command_with_read_only_stdout(stdout: &Path) -> std::process::Command {
        let mut command = std::process::Command::new(assert_cmd::cargo::cargo_bin!("blotter"));
        command
            .env("BLOTTER_NOW", NOW)
            .env_remove("BLOTTER_FILE")
            .env_remove("BLOTTER_AGENT")
            .env_remove("PAPERCUTS_FILE")
            .env_remove("PAPERCUTS_AGENT")
            .env_remove("PAPERCUTS_NOW")
            .env_remove("CLAUDECODE");
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("CODEX_")
                || key.to_string_lossy().starts_with("CURSOR_")
            {
                command.env_remove(key);
            }
        }
        command.stdout(Stdio::from(std::fs::File::open(stdout).unwrap()));
        command
    }

    fn assert_stdout_write_error(output: &std::process::Output) {
        let envelope = error(output, 74, "io_error");
        assert!(envelope.error.message.contains("stdout"));
    }

    let temp = TempDir::new().unwrap();
    let read_only_stdout = temp.path().join("read-only-stdout");
    std::fs::write(&read_only_stdout, "").unwrap();
    let file = temp.path().join("cuts.jsonl");
    std::fs::write(&file, "").unwrap();

    let list = command_with_read_only_stdout(&read_only_stdout)
        .arg("--file")
        .arg(&file)
        .arg("list")
        .output()
        .unwrap();
    assert_stdout_write_error(&list);

    let list_markdown = command_with_read_only_stdout(&read_only_stdout)
        .arg("--file")
        .arg(&file)
        .args(["list", "--format", "md"])
        .output()
        .unwrap();
    assert_stdout_write_error(&list_markdown);

    let digest_markdown = command_with_read_only_stdout(&read_only_stdout)
        .arg("--file")
        .arg(&file)
        .args(["digest", "--format", "md"])
        .output()
        .unwrap();
    assert_stdout_write_error(&digest_markdown);

    let export = command_with_read_only_stdout(&read_only_stdout)
        .arg("--file")
        .arg(&file)
        .args(["export", "--format", "otlp-json"])
        .output()
        .unwrap();
    assert_stdout_write_error(&export);

    let schema = command_with_read_only_stdout(&read_only_stdout)
        .arg("schema")
        .output()
        .unwrap();
    assert_stdout_write_error(&schema);

    let add_file = temp.path().join("add.jsonl");
    let add = command_with_read_only_stdout(&read_only_stdout)
        .arg("--file")
        .arg(&add_file)
        .args(["add", "stdout write failed", "--agent", "tester"])
        .output()
        .unwrap();
    assert_stdout_write_error(&add);
    assert_eq!(
        std::fs::read_to_string(add_file).unwrap().lines().count(),
        1
    );
}

#[test]
fn structured_error_exit_matrix_and_help_exceptions() {
    let temp = TempDir::new().unwrap();
    let missing = temp.path().join("missing.jsonl");
    error(&run_file(&missing, &["list"]), 66, "not_found");
    error(&run(&["list", "--format", "jsonl"]), 2, "invalid_argument");
    let schema: SuccessEnvelope<Value> = success(
        &command()
            .env("BLOTTER_NOW", "not-a-time")
            .args(["schema"])
            .output()
            .unwrap(),
    );
    assert_eq!(schema.data["contract"], 6);
    error(
        &command()
            .env("BLOTTER_NOW", "not-a-time")
            .arg("list")
            .output()
            .unwrap(),
        78,
        "config_error",
    );
    error(
        &run_file(&missing, &["add", " ", "--agent", "tester"]),
        65,
        "invalid_input",
    );
    let invalid_utf8 = command()
        .arg("--file")
        .arg(&missing)
        .args(["add", "-", "--agent", "tester"])
        .write_stdin(vec![0xff])
        .output()
        .unwrap();
    error(&invalid_utf8, 65, "invalid_input");
    // A directory is not a regular file, so the log-open guard rejects it at the
    // same code the sibling --stderr-file lane uses, ahead of the EISDIR read.
    let directory_error = run_file(temp.path(), &["list"]);
    let directory_envelope = error(&directory_error, 65, "invalid_input");
    assert!(
        directory_envelope
            .error
            .message
            .contains("not a regular file")
    );

    let help = run(&["--help"]);
    assert!(help.status.success());
    assert!(help.stderr.is_empty());
    assert!(String::from_utf8_lossy(&help.stdout).contains("Usage:"));
    let version = run(&["--version"]);
    assert!(version.status.success());
    assert_eq!(
        String::from_utf8_lossy(&version.stdout),
        format!("blotter {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn agent_resolution_order_and_sources_are_pinned() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("unused.jsonl");
    let invoke = |command: &mut Command| -> SuccessEnvelope<AddData> {
        success(
            &command
                .arg("--file")
                .arg(&file)
                .args(["add", "x", "--dry-run"])
                .output()
                .unwrap(),
        )
    };

    let default = invoke(&mut command());
    assert_eq!(default.data.record.cut_agent(), "unknown");
    assert_eq!(default.meta.agent_source.as_deref(), Some("default"));

    let claude = invoke(command().env("CLAUDECODE", "1"));
    assert_eq!(claude.data.record.cut_agent(), "claude-code");
    assert_eq!(claude.meta.agent_source.as_deref(), Some("detected"));

    let codex = invoke(command().env("CODEX_TEST", "1").env("CURSOR_TEST", "1"));
    assert_eq!(codex.data.record.cut_agent(), "codex");

    let cursor = invoke(command().env("CURSOR_TEST", "1"));
    assert_eq!(cursor.data.record.cut_agent(), "cursor");

    let env = invoke(
        command()
            .env("BLOTTER_AGENT", "from-env")
            .env("CLAUDECODE", "1"),
    );
    assert_eq!(env.data.record.cut_agent(), "from-env");
    assert_eq!(env.meta.agent_source.as_deref(), Some("env"));

    let flag: SuccessEnvelope<AddData> = success(
        &command()
            .env("BLOTTER_AGENT", "from-env")
            .arg("--file")
            .arg(&file)
            .args(["add", "x", "--agent", "from-flag", "--dry-run"])
            .output()
            .unwrap(),
    );
    assert_eq!(flag.data.record.cut_agent(), "from-flag");
    assert_eq!(flag.meta.agent_source.as_deref(), Some("flag"));
    assert!(!file.exists());
}

#[test]
fn shared_agent_validation_preserves_append_and_resolve_policies() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    for args in [
        &["add", "x", "--agent", " "][..],
        &["dogear", "x", "--agent", " "][..],
    ] {
        let invalid = error(&run_file(&file, args), 65, "invalid_input");
        assert_eq!(
            invalid.error.message,
            "agent name cannot be empty or whitespace-only"
        );
        assert_eq!(
            invalid.error.suggested_fix,
            "Pass a non-empty --agent NAME or omit the flag."
        );
    }

    let id = add(&file, "resolve with a whitespace environment agent")
        .data
        .record
        .cut_id()
        .to_owned();
    let resolved: SuccessEnvelope<ResolveData> = success(
        &command()
            .env("BLOTTER_AGENT", " ")
            .arg("--file")
            .arg(&file)
            .args(["resolve", &id])
            .output()
            .unwrap(),
    );
    assert_eq!(
        resolved.data.records[0].resolution.as_ref().unwrap().agent,
        " "
    );
}

#[test]
fn shared_empty_state_preserves_warnings_suggested_fixes_and_doctor_file_state() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    std::fs::create_dir(&home).unwrap();

    let list: SuccessEnvelope<ListData> = success(
        &command()
            .current_dir(temp.path())
            .env("HOME", &home)
            .arg("list")
            .output()
            .unwrap(),
    );
    assert_eq!(
        list.meta.warnings[0],
        "no blotter file yet; blotter add creates it"
    );

    let triage = triage_success(
        &command()
            .current_dir(temp.path())
            .env("HOME", &home)
            .arg("triage")
            .output()
            .unwrap(),
        0,
    );
    assert_eq!(
        triage.meta.warnings,
        ["no blotter file yet; blotter add creates it"]
    );

    let doctor: SuccessEnvelope<DoctorData> = success(
        &command()
            .current_dir(temp.path())
            .env("HOME", &home)
            .arg("doctor")
            .output()
            .unwrap(),
    );
    assert!(doctor.data.healthy);
    assert_eq!(doctor.data.checked_lines, 0);
    assert_eq!(
        doctor.meta.warnings,
        ["no blotter file yet; healthy empty state"]
    );

    let missing = temp.path().join("missing.jsonl");
    for args in [&["list"][..], &["triage"][..], &["doctor"][..]] {
        let missing_error = error(&run_file(&missing, args), 66, "not_found");
        assert_eq!(
            missing_error.error.message,
            format!("blotter file not found: {}", missing.display())
        );
        assert_eq!(
            missing_error.error.suggested_fix,
            if args == ["doctor"] {
                "Pass an existing --file PATH or omit --file to inspect discovered state."
            } else {
                "Pass an existing --file PATH or run `blotter add` to create a discovered default file."
            }
        );
    }
}

#[test]
fn tagged_event_serialization_preserves_record_field_order() {
    let cut = LogEvent::Cut {
        id: "bl_123456789abc".into(),
        ts: "2026-08-01T00:00:00.000Z".into(),
        agent: "fixture".into(),
        text: "cut".into(),
        tags: vec!["a".into()],
        severity: Severity::Major,
        cwd: ".".into(),
        source: None,
        evidence: Some(Evidence {
            cmd: Some("cmd".into()),
            exit: Some(7),
            stderr: Some("stderr".into()),
            note: Some("note".into()),
        }),
    };
    assert_eq!(
        serde_json::to_string(&cut).unwrap(),
        r#"{"kind":"cut","id":"bl_123456789abc","ts":"2026-08-01T00:00:00.000Z","agent":"fixture","text":"cut","tags":["a"],"severity":"major","cwd":".","evidence":{"cmd":"cmd","exit":7,"stderr":"stderr","note":"note"}}"#,
    );

    let dogear = LogEvent::Dogear {
        id: "bl_12345678901234567890".into(),
        ts: "2026-08-02T00:00:00.000Z".into(),
        agent: "fixture".into(),
        text: "dogear".into(),
        tags: vec!["a".into()],
        evidence: Some("note".into()),
        cwd: ".".into(),
    };
    assert_eq!(
        serde_json::to_string(&dogear).unwrap(),
        r#"{"kind":"dogear","id":"bl_12345678901234567890","ts":"2026-08-02T00:00:00.000Z","agent":"fixture","text":"dogear","tags":["a"],"evidence":"note","cwd":"."}"#,
    );

    let resolve = LogEvent::Resolve {
        id: "bl_123456789abc".into(),
        ts: "2026-08-03T00:00:00.000Z".into(),
        agent: "fixture".into(),
        note: None,
        task: Some("TASK-16".into()),
        pr: Some("#16".into()),
        commit: Some("deadbeef".into()),
        url: Some("https://example.test".into()),
        dropped: true,
        amend: false,
    };
    assert_eq!(
        serde_json::to_string(&resolve).unwrap(),
        r##"{"kind":"resolve","id":"bl_123456789abc","ts":"2026-08-03T00:00:00.000Z","agent":"fixture","note":null,"task":"TASK-16","pr":"#16","commit":"deadbeef","url":"https://example.test","dropped":true}"##,
    );
    assert_eq!(
        serde_json::from_str::<LogEvent>(r#"{"kind":"future","extra":true}"#).unwrap(),
        LogEvent::Unknown,
    );
}

#[test]
fn scanner_classification_remains_distinct_through_list_and_doctor() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let invalid_cut = json!({
        "kind":"cut", "id":"bl_bad", "ts":"2026-07-09T00:00:00.000Z",
        "agent":"a", "text":"x", "tags":[], "severity":"future", "cwd":"/tmp"
    });
    let invalid_timestamp = json!({
        "kind":"cut", "id":"bl_bad", "ts":"not-a-time",
        "agent":"a", "text":"x", "tags":[], "severity":"minor", "cwd":"/tmp"
    });
    std::fs::write(
        &file,
        format!(
            "{invalid_cut}\n{{\"kind\":\"future\"}}\n{invalid_timestamp}\n{{\"kind\":\"cut\"}}\n{{\"kind\":"
        ),
    )
    .unwrap();

    let listed: SuccessEnvelope<ListData> = success(&run_file(&file, &["list", "--kind", "all"]));
    assert_eq!(
        listed.meta.warnings,
        [
            "skipped 1 torn final line",
            "skipped 3 malformed lines",
            "skipped 1 unknown event",
            "no records matched; try --status all or broader filters",
        ]
    );

    let output = run_file(&file, &["doctor"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let doctor: SuccessEnvelope<DoctorData> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(doctor.data.checked_lines, 5);
    assert_eq!(
        doctor
            .data
            .findings
            .iter()
            .map(|finding| finding.kind.as_str())
            .collect::<Vec<_>>(),
        [
            "malformed",
            "unknown_kind",
            "malformed",
            "malformed",
            "torn_line"
        ]
    );
    assert!(
        doctor.data.findings[0]
            .message
            .starts_with("invalid cut record:")
    );
    assert_eq!(
        doctor.data.findings[2].message,
        "cut ts is not a full RFC3339 timestamp"
    );
}

#[test]
fn mutation_dry_runs_do_not_write() {
    let temp = TempDir::new().unwrap();
    let dry_add = temp.path().join("nested/cuts.jsonl");
    let added: SuccessEnvelope<AddData> = success(&run_file(
        &dry_add,
        &["add", "preview", "--agent", "a", "--dry-run"],
    ));
    assert!(!added.data.changed);
    assert!(!dry_add.exists());

    let file = temp.path().join("cuts.jsonl");
    let id = add(&file, "resolve preview")
        .data
        .record
        .cut_id()
        .to_owned();
    let before = std::fs::read(&file).unwrap();
    let resolved: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &["resolve", &id, "--agent", "a", "--dry-run"],
    ));
    assert!(!resolved.data.changed);
    assert_eq!(resolved.data.records.len(), 1);
    assert_eq!(resolved.data.records[0].status, ItemStatus::Resolved);
    assert_eq!(std::fs::read(&file).unwrap(), before);
}

#[test]
fn since_invalid_argument_strings_remain_byte_identical() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    add(&file, "parser fixture");
    for (value, message, suggested_fix) in [
        (
            "garbage",
            "invalid --since value 'garbage'",
            "Use a full RFC3339 timestamp such as 2026-07-09T18:30:00Z, or a relative value such as 7d or 12h.",
        ),
        (
            "9223372036854775807d",
            "--since value '9223372036854775807d' is too large",
            "Use a smaller Nd or Nh duration.",
        ),
        (
            "1000000000000000h",
            "--since value '1000000000000000h' is outside the supported range",
            "Use a smaller Nd or Nh duration.",
        ),
    ] {
        let envelope = error(
            &run_file(&file, &["list", "--since", value]),
            2,
            "invalid_argument",
        );
        assert_eq!(envelope.error.message, message);
        assert_eq!(envelope.error.suggested_fix, suggested_fix);
    }
}

#[test]
fn error_envelope_matrix() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let missing = temp.path().join("missing.jsonl");
    let outside = temp.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();

    let ambiguous = temp.path().join("ambiguous.jsonl");
    let lines = ["bl_abcd00000000", "bl_abcd11111111"]
        .map(|id| {
            json!({"kind":"cut","id":id,"ts":"2026-07-09T00:00:00.000Z","agent":"a","text":id,"tags":[],"severity":"minor","cwd":"/tmp","repo":null}).to_string()
        })
        .join("\n")
        + "\n";
    std::fs::write(&ambiguous, lines).unwrap();

    error(&run(&["list", "--format", "jsonl"]), 2, "invalid_argument");
    error(
        &run_file(&file, &["add", " ", "--agent", "tester"]),
        65,
        "invalid_input",
    );
    error(&run_file(&missing, &["list"]), 66, "not_found");
    if temp_has_git_ancestor(&temp) {
        eprintln!(
            "skipping HOME/config-78 assertion because the temporary directory is inside a git checkout"
        );
    } else {
        error(
            &command()
                .current_dir(&outside)
                .env("HOME", "")
                .arg("list")
                .output()
                .unwrap(),
            78,
            "config_error",
        );
    }
    error(
        &run_file(&ambiguous, &["resolve", "abcd"]),
        65,
        "ambiguous_id",
    );
}

#[test]
fn invalid_since_is_reported_before_the_log_file_for_every_command() {
    let temp = TempDir::new().unwrap();
    let missing = temp.path().join("missing.jsonl");

    for args in [
        &["list", "--since", "banana"][..],
        &["export", "--format", "otlp-json", "--since", "banana"][..],
        &["digest", "--since", "banana"][..],
    ] {
        let envelope = error(&run_file(&missing, args), 2, "invalid_argument");
        assert!(
            envelope.error.message.contains("--since"),
            "message={}",
            envelope.error.message
        );
    }

    // sweep rejects --file, so its equivalent missing input is --registry.
    let sweep = command()
        .args(["sweep", "--since", "banana", "--registry"])
        .arg(temp.path().join("repos.txt"))
        .output()
        .unwrap();
    let envelope = error(&sweep, 2, "invalid_argument");
    assert!(envelope.error.message.contains("--since"));
}

#[cfg(unix)]
#[test]
fn non_utf8_blotter_agent_and_removed_hook_are_rejected_before_filing() {
    use std::os::unix::ffi::OsStrExt;

    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let agent = std::ffi::OsStr::from_bytes(b"agent-\xff");

    let output = command()
        .env("BLOTTER_AGENT", agent)
        .arg("--file")
        .arg(&file)
        .args(["add", "a cut filed under an unreadable agent"])
        .output()
        .unwrap();
    let envelope = error(&output, 78, "config_error");
    assert!(envelope.error.message.contains("BLOTTER_AGENT"));
    assert!(!file.exists());

    let hook = command()
        .env("BLOTTER_AGENT", agent)
        .arg("--file")
        .arg(&file)
        .args(["hook", "exec", "claude-code"])
        .write_stdin("{}")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&hook.stderr);
    assert!(
        stderr.contains("unrecognized subcommand 'hook'"),
        "{stderr}"
    );
    error(&hook, 2, "invalid_argument");
    assert!(!file.exists());
}
