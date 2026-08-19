use crate::common::*;

fn hook_exec_claude_code_with_explain(
    file: &Path,
    stdin: impl Into<Vec<u8>>,
    explain: &str,
) -> std::process::Output {
    command()
        .env("BLOTTER_HOOK_EXPLAIN", explain)
        .arg("--file")
        .arg(file)
        .args(["hook", "exec", "claude-code"])
        .write_stdin(stdin)
        .output()
        .unwrap()
}

fn hook_exec_explains(output: &std::process::Output, expected: &str) {
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        format!("{expected}\n")
    );
}

#[test]
fn hook_exec_claude_code_files_valid_failure_silently_with_redacted_evidence() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    std::fs::write(&file, "").unwrap();
    let command = "cargo test --package blotter-cli";
    let output =
        hook_exec_claude_code(&file, claude_bash_failure(command, temp.path()).to_string());
    hook_exec_is_silent(&output);

    let lines: Vec<Value> = std::fs::read_to_string(&file)
        .unwrap()
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(lines.len(), 1);
    let record = &lines[0];
    assert_eq!(record["kind"], "cut");
    assert_eq!(record["text"], command);
    assert_eq!(record["severity"], "minor");
    assert_eq!(record["tags"], json!(["auto", "claude-code"]));
    assert_eq!(
        record["cwd"],
        json!(temp.path().to_string_lossy().into_owned())
    );
    assert!(record.get("repo").is_none());
    assert_eq!(record["evidence"]["cmd"], command);
    assert!(record["evidence"].get("exit").is_none());
    let note = record["evidence"]["note"].as_str().unwrap();
    assert!(!note.contains("super-secret-token"));
    assert!(note.contains("<redacted>"));
}

#[test]
fn hook_and_add_provenance_serializes_source_only_when_present() {
    let temp = TempDir::new().unwrap();
    let hook_file = temp.path().join("hook-cuts.jsonl");
    std::fs::write(&hook_file, "").unwrap();

    hook_exec_is_silent(&hook_exec_claude_code(
        &hook_file,
        claude_bash_failure("cargo test --workspace", temp.path()).to_string(),
    ));

    let hook_stored: Value = serde_json::from_str(
        std::fs::read_to_string(&hook_file)
            .unwrap()
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(hook_stored["source"], "hook");
    // Hook exec is intentionally silent; list is its public JSON record view.
    let hook_listed: SuccessEnvelope<Value> =
        success(&run_file(&hook_file, &["list", "--include-auto"]));
    assert_eq!(hook_listed.data["items"][0]["source"], "hook");

    let add_file = temp.path().join("add-cuts.jsonl");
    let added: SuccessEnvelope<Value> = success(&run_file(
        &add_file,
        &["add", "hand-filed cut", "--agent", "tester"],
    ));
    assert!(added.data["record"].get("source").is_none());
    let add_stored: Value = serde_json::from_str(
        std::fs::read_to_string(&add_file)
            .unwrap()
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    assert!(add_stored.get("source").is_none());
}

#[test]
fn hook_exec_files_home_relative_cwd_and_redacts_home_paths_in_its_note() {
    let temp = TempDir::new().unwrap();
    if temp_has_git_ancestor(&temp) {
        eprintln!("skipping hook home assertion inside a git checkout");
        return;
    }
    let home = temp.path().join("home");
    let cwd = home.join("nested");
    let file = temp.path().join("cuts.jsonl");
    std::fs::create_dir_all(&cwd).unwrap();
    std::fs::write(&file, "").unwrap();
    let mut payload = claude_bash_failure("cargo test", &cwd);
    payload["error"] = json!(format!(
        "failed under {}/nested and /home/other/log",
        home.display()
    ));

    let output = command()
        .env("HOME", &home)
        .arg("--file")
        .arg(&file)
        .args(["hook", "exec", "claude-code"])
        .write_stdin(payload.to_string())
        .output()
        .unwrap();
    hook_exec_is_silent(&output);

    let record: Value = serde_json::from_str(
        std::fs::read_to_string(&file)
            .unwrap()
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(record["cwd"], "~/nested");
    assert_eq!(
        record["evidence"]["note"],
        "failed under ~/nested and ~/log"
    );
}

#[test]
fn hook_exec_redacts_command_text_and_truncates_failure_note_on_utf8_boundary() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let cwd = home.join("workspace");
    let file = temp.path().join("cuts.jsonl");
    std::fs::create_dir_all(&cwd).unwrap();
    std::fs::write(&file, "").unwrap();
    let home = home.canonicalize().unwrap();
    let cwd = cwd.canonicalize().unwrap();
    let raw_command = format!("cargo test {}/project", home.display());
    let expected = "cargo test ~/project";
    let mut payload = claude_bash_failure(&raw_command, &cwd);
    payload["error"] = json!(format!("{}éafter-boundary", "x".repeat(1023)));

    let output = command()
        .env("HOME", &home)
        .arg("--file")
        .arg(&file)
        .args(["hook", "exec", "claude-code"])
        .write_stdin(payload.to_string())
        .output()
        .unwrap();
    hook_exec_is_silent(&output);

    let record: Value = serde_json::from_str(
        std::fs::read_to_string(&file)
            .unwrap()
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        record["evidence"]["note"].as_str().unwrap(),
        "x".repeat(1023)
    );
    assert_eq!(record["text"], expected);
    assert_eq!(record["evidence"]["cmd"], expected);
    assert_eq!(
        record["id"],
        compute_id(
            record["ts"].as_str().unwrap(),
            record["agent"].as_str().unwrap(),
            expected,
            Severity::Minor,
            &["auto".into(), "claude-code".into()]
        )
    );
}

#[test]
fn hook_exec_redacts_machine_captured_command_text_and_evidence() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    std::fs::write(&file, "").unwrap();
    let raw_command = "cargo run authorization: Bearer hook-secret";
    let expected = "cargo run authorization: <redacted>";

    let output = command()
        .env("HOME", "/Users/alice")
        .arg("--file")
        .arg(&file)
        .args(["hook", "exec", "claude-code"])
        .write_stdin(claude_bash_failure(raw_command, temp.path()).to_string())
        .output()
        .unwrap();
    hook_exec_is_silent(&output);

    let record: Value = serde_json::from_str(
        std::fs::read_to_string(&file)
            .unwrap()
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(record["text"], expected);
    assert_eq!(record["evidence"]["cmd"], expected);
    assert_eq!(
        record["id"],
        compute_id(
            record["ts"].as_str().unwrap(),
            record["agent"].as_str().unwrap(),
            expected,
            Severity::Minor,
            &["auto".into(), "claude-code".into()]
        )
    );
    assert!(!record.to_string().contains("hook-secret"));
}

#[test]
fn hook_exec_deduplicates_redacted_command_text() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    std::fs::write(&file, "").unwrap();
    let first = claude_bash_failure("cargo test /Users/alice/workspace", temp.path());
    let second = claude_bash_failure("cargo test /home/other/workspace", temp.path());

    for payload in [first, second] {
        let output = command()
            .env("HOME", "/Users/alice")
            .arg("--file")
            .arg(&file)
            .args(["hook", "exec", "claude-code"])
            .write_stdin(payload.to_string())
            .output()
            .unwrap();
        hook_exec_is_silent(&output);
    }

    let lines: Vec<Value> = std::fs::read_to_string(&file)
        .unwrap()
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["text"], "cargo test ~/workspace");
}

#[test]
fn hook_exec_command_byte_gate_uses_the_raw_command_before_redaction() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    std::fs::write(&file, "").unwrap();
    let raw_command = format!("echo {}", "/Users/alice ".repeat(40));
    let redacted = format!("echo {}", "~ ".repeat(40));
    assert!(raw_command.len() > 500);
    assert!(redacted.len() <= 500);

    let output = command()
        .env("HOME", "/Users/alice")
        .env("BLOTTER_HOOK_EXPLAIN", "1")
        .arg("--file")
        .arg(&file)
        .args(["hook", "exec", "claude-code"])
        .write_stdin(claude_bash_failure(&raw_command, temp.path()).to_string())
        .output()
        .unwrap();
    hook_exec_explains(
        &output,
        &format!(
            "hook exec: tool_input.command is {} bytes; exceeds the 500-byte limit; skipped",
            raw_command.len()
        ),
    );
    assert!(std::fs::read_to_string(&file).unwrap().is_empty());
}

#[test]
fn hook_exec_claude_code_skips_probe_commands_and_explains_the_gate() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    std::fs::write(&file, "").unwrap();
    let command = "grep -r missing_symbol src/";

    hook_exec_explains(
        &hook_exec_claude_code_with_explain(
            &file,
            claude_bash_failure(command, temp.path()).to_string(),
            "1",
        ),
        "hook exec: grep is a read-only probe; non-zero exit is an expected answer; skipped",
    );
    assert!(std::fs::read_to_string(&file).unwrap().is_empty());
}

#[test]
fn hook_exec_claude_code_skips_probe_commands_after_leading_env_assignments() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    std::fs::write(&file, "").unwrap();
    let command = "FOO=bar grep x y";

    hook_exec_explains(
        &hook_exec_claude_code_with_explain(
            &file,
            claude_bash_failure(command, temp.path()).to_string(),
            "1",
        ),
        "hook exec: grep is a read-only probe; non-zero exit is an expected answer; skipped",
    );
    assert!(std::fs::read_to_string(&file).unwrap().is_empty());
}

#[test]
fn hook_exec_claude_code_files_non_probe_commands() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    std::fs::write(&file, "").unwrap();
    let command = "cargo build --release";

    hook_exec_is_silent(&hook_exec_claude_code(
        &file,
        claude_bash_failure(command, temp.path()).to_string(),
    ));
    let lines: Vec<Value> = std::fs::read_to_string(&file)
        .unwrap()
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["text"], command);
    assert_eq!(lines[0]["evidence"]["cmd"], command);
}

const HOOK_CHAIN_EXPLANATION: &str = "hook exec: tool_input.command is not a simple command (chain, substitution, or unterminated quote); its exit does not name the friction; skipped";

#[test]
fn hook_exec_claude_code_skips_chained_commands_and_explains_the_gate() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    std::fs::write(&file, "").unwrap();
    let command = "cargo build --release && cargo test";

    hook_exec_explains(
        &hook_exec_claude_code_with_explain(
            &file,
            claude_bash_failure(command, temp.path()).to_string(),
            "1",
        ),
        HOOK_CHAIN_EXPLANATION,
    );
    assert!(std::fs::read_to_string(&file).unwrap().is_empty());
}

#[test]
fn hook_exec_claude_code_skips_piped_and_sequenced_commands() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    std::fs::write(&file, "").unwrap();

    for command in [
        "cargo test | tail -3",
        "cargo build; cargo test",
        "cargo build || cargo test",
        "cargo build\ncargo test",
        "echo $(date)",
        "echo `date`",
    ] {
        hook_exec_explains(
            &hook_exec_claude_code_with_explain(
                &file,
                claude_bash_failure(command, temp.path()).to_string(),
                "1",
            ),
            HOOK_CHAIN_EXPLANATION,
        );
        assert!(
            std::fs::read_to_string(&file).unwrap().is_empty(),
            "command={command:?} must not be filed"
        );
    }
}

#[test]
fn hook_exec_claude_code_skips_commands_that_end_inside_a_quote() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    std::fs::write(&file, "").unwrap();

    // A scan that ends mid-quote cannot rule out an operator the quote hides,
    // so the ambiguity resolves toward skipping.
    for command in [
        r#"git commit -m "unterminated"#,
        r#"jq '.[] | {id} report.json"#,
        // A trailing backslash inside the double-quoted span consumes the end of
        // input, so the span never closes.
        r#"git commit -m "trailing escape \"#,
    ] {
        hook_exec_explains(
            &hook_exec_claude_code_with_explain(
                &file,
                claude_bash_failure(command, temp.path()).to_string(),
                "1",
            ),
            HOOK_CHAIN_EXPLANATION,
        );
        assert!(
            std::fs::read_to_string(&file).unwrap().is_empty(),
            "command={command:?} must not be filed"
        );
    }
}

#[test]
fn hook_exec_claude_code_files_commands_whose_operators_are_quoted() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    std::fs::write(&file, "").unwrap();

    // jq stands in for the design's `gh api --jq` example: gh is a read-only
    // probe, so it never reaches the filing path whatever the shape gate says.
    for command in [
        r#"git commit -m "a && b""#,
        r#"jq '.[] | {id}' report.json"#,
        r#"git commit -m "escaped \" quote; still simple""#,
    ] {
        hook_exec_is_silent(&hook_exec_claude_code(
            &file,
            claude_bash_failure(command, temp.path()).to_string(),
        ));
    }

    let lines: Vec<Value> = std::fs::read_to_string(&file)
        .unwrap()
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0]["text"], r#"git commit -m "a && b""#);
    assert_eq!(lines[1]["text"], r#"jq '.[] | {id}' report.json"#);
    assert_eq!(
        lines[2]["text"],
        r#"git commit -m "escaped \" quote; still simple""#
    );
}

#[test]
fn hook_exec_claude_code_files_redirected_simple_commands() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    std::fs::write(&file, "").unwrap();
    let command = "cargo test 2>&1";

    hook_exec_is_silent(&hook_exec_claude_code(
        &file,
        claude_bash_failure(command, temp.path()).to_string(),
    ));
    let lines: Vec<Value> = std::fs::read_to_string(&file)
        .unwrap()
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["text"], command);

    // The same command piped is a chain.
    hook_exec_explains(
        &hook_exec_claude_code_with_explain(
            &file,
            claude_bash_failure("cargo test 2>&1 | tail -3", temp.path()).to_string(),
            "1",
        ),
        HOOK_CHAIN_EXPLANATION,
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap().lines().count(), 1);
}

#[test]
fn hook_exec_claude_code_reports_the_byte_gate_before_the_shape_gate() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    std::fs::write(&file, "").unwrap();

    let over_limit = format!("cargo build && {}", "x".repeat(500));
    hook_exec_explains(
        &hook_exec_claude_code_with_explain(
            &file,
            claude_bash_failure(&over_limit, temp.path()).to_string(),
            "1",
        ),
        &format!(
            "hook exec: tool_input.command is {} bytes; exceeds the 500-byte limit; skipped",
            over_limit.len()
        ),
    );

    // The shape gate does not shadow the program gate for a simple probe.
    hook_exec_explains(
        &hook_exec_claude_code_with_explain(
            &file,
            claude_bash_failure("grep -r x src/", temp.path()).to_string(),
            "1",
        ),
        "hook exec: grep is a read-only probe; non-zero exit is an expected answer; skipped",
    );
    assert!(std::fs::read_to_string(&file).unwrap().is_empty());
}

#[test]
fn hook_exec_claude_code_stores_repo_relative_payload_cwd_without_repo() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("repo");
    let nested = root.join("nested");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::create_dir(root.join(".git")).unwrap();
    let file = root.join(".blotter.jsonl");
    std::fs::write(&file, "").unwrap();

    let output = command()
        .current_dir(&root)
        .arg("--file")
        .arg(&file)
        .args(["hook", "exec", "claude-code"])
        .write_stdin(claude_bash_failure("cargo test --workspace", &nested).to_string())
        .output()
        .unwrap();
    hook_exec_is_silent(&output);

    let record: Value = serde_json::from_str(
        std::fs::read_to_string(&file)
            .unwrap()
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(record["cwd"], "nested");
    assert!(record.get("repo").is_none());
}

#[test]
fn hook_exec_claude_code_ignores_nonfailures_and_missing_logs() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    std::fs::write(&file, "").unwrap();
    let command = "cargo test";

    let interrupted = json!({
        "hook_event_name": "PostToolUseFailure",
        "tool_name": "Bash",
        "tool_input": {"command": command},
        "is_interrupt": true
    });
    hook_exec_is_silent(&hook_exec_claude_code(&file, interrupted.to_string()));

    let edit = json!({
        "hook_event_name": "PostToolUseFailure",
        "tool_name": "Edit",
        "tool_input": {"command": command}
    });
    hook_exec_is_silent(&hook_exec_claude_code(&file, edit.to_string()));

    let empty_command = json!({
        "hook_event_name": "PostToolUseFailure",
        "tool_name": "Bash",
        "tool_input": {"command": ""}
    });
    hook_exec_is_silent(&hook_exec_claude_code(&file, empty_command.to_string()));

    hook_exec_is_silent(&hook_exec_claude_code(&file, b"{not valid json}".to_vec()));
    let oversized = json!({
        "hook_event_name": "PostToolUseFailure",
        "tool_name": "Bash",
        "tool_input": {"command": command},
        "padding": "x".repeat(1024 * 1024)
    });
    hook_exec_is_silent(&hook_exec_claude_code(&file, oversized.to_string()));
    assert!(std::fs::read_to_string(&file).unwrap().is_empty());

    let missing = temp.path().join("no-log/cuts.jsonl");
    hook_exec_is_silent(&hook_exec_claude_code(
        &missing,
        claude_bash_failure(command, temp.path()).to_string(),
    ));
    assert!(!missing.exists());
    assert!(!missing.parent().unwrap().exists());
}

#[test]
fn hook_exec_claude_code_explains_rejection_paths_and_outcomes() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    std::fs::write(&file, "").unwrap();
    let command = "cargo test";

    let wrong_event = json!({
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": command}
    });
    hook_exec_explains(
        &hook_exec_claude_code_with_explain(&file, wrong_event.to_string(), "1"),
        r#"hook exec: hook_event_name was "PreToolUse"; expected "PostToolUseFailure"; skipped"#,
    );

    let non_bash = json!({
        "hook_event_name": "PostToolUseFailure",
        "tool_name": "Edit",
        "tool_input": {"command": command}
    });
    hook_exec_explains(
        &hook_exec_claude_code_with_explain(&file, non_bash.to_string(), "1"),
        r#"hook exec: tool_name was "Edit"; expected "Bash"; skipped"#,
    );

    let interrupted = json!({
        "hook_event_name": "PostToolUseFailure",
        "tool_name": "Bash",
        "tool_input": {"command": command},
        "is_interrupt": true
    });
    hook_exec_explains(
        &hook_exec_claude_code_with_explain(&file, interrupted.to_string(), "1"),
        "hook exec: is_interrupt is true; skipped",
    );

    let empty_command = json!({
        "hook_event_name": "PostToolUseFailure",
        "tool_name": "Bash",
        "tool_input": {"command": ""}
    });
    hook_exec_explains(
        &hook_exec_claude_code_with_explain(&file, empty_command.to_string(), "1"),
        "hook exec: tool_input.command is missing or empty; skipped",
    );

    let rejected_command = json!({
        "hook_event_name": "PostToolUseFailure",
        "tool_name": "Bash",
        "tool_input": {"command": "x".repeat(10_001)}
    });
    // The 500-byte hook gate now precedes add::validate_text, whose own limit is 10000.
    hook_exec_explains(
        &hook_exec_claude_code_with_explain(&file, rejected_command.to_string(), "1"),
        "hook exec: tool_input.command is 10001 bytes; exceeds the 500-byte limit; skipped",
    );

    let missing = temp.path().join("no-log/cuts.jsonl");
    hook_exec_explains(
        &hook_exec_claude_code_with_explain(
            &missing,
            claude_bash_failure(command, temp.path()).to_string(),
            "1",
        ),
        &format!("hook exec: resolved log file {missing:?} is not an existing file; skipped"),
    );

    let payload = claude_bash_failure(command, temp.path()).to_string();
    let filed = hook_exec_claude_code_with_explain(&file, payload.clone(), "1");
    let record: Value = serde_json::from_str(
        std::fs::read_to_string(&file)
            .unwrap()
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    let id = record["id"].as_str().unwrap();
    hook_exec_explains(&filed, &format!("hook exec: filed cut {id}"));
    hook_exec_explains(
        &hook_exec_claude_code_with_explain(&file, payload, "1"),
        &format!("hook exec: duplicate open command matches cut {id}; skipped"),
    );
}

#[test]
fn hook_exec_claude_code_explains_invalid_and_oversized_payloads() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    std::fs::write(&file, "").unwrap();

    hook_exec_explains(
        &hook_exec_claude_code_with_explain(&file, b"{not valid json}".to_vec(), "1"),
        "hook exec: stdin is not valid JSON; skipped",
    );
    let oversized = json!({
        "hook_event_name": "PostToolUseFailure",
        "tool_name": "Bash",
        "tool_input": {"command": "cargo test"},
        "padding": "x".repeat(1024 * 1024)
    });
    hook_exec_explains(
        &hook_exec_claude_code_with_explain(&file, oversized.to_string(), "1"),
        "hook exec: stdin exceeds the 1048576-byte limit; skipped",
    );
}

#[test]
fn hook_exec_claude_code_skips_commands_over_the_length_limit() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    std::fs::write(&file, "").unwrap();

    let at_limit = "e".repeat(500);
    hook_exec_is_silent(&hook_exec_claude_code(
        &file,
        claude_bash_failure(&at_limit, temp.path()).to_string(),
    ));
    assert_eq!(
        std::fs::read_to_string(&file).unwrap().lines().count(),
        1,
        "a command exactly at the limit must still be filed"
    );

    let over_limit = "x".repeat(501);
    hook_exec_explains(
        &hook_exec_claude_code_with_explain(
            &file,
            claude_bash_failure(&over_limit, temp.path()).to_string(),
            "1",
        ),
        "hook exec: tool_input.command is 501 bytes; exceeds the 500-byte limit; skipped",
    );
    assert_eq!(
        std::fs::read_to_string(&file).unwrap().lines().count(),
        1,
        "the oversized command must not be appended"
    );
}

#[test]
fn hook_exec_claude_code_explains_an_unusable_clock() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    std::fs::write(&file, "").unwrap();
    let payload = claude_bash_failure("cargo test --package blotter-cli", temp.path()).to_string();

    let output = command()
        .env("BLOTTER_HOOK_EXPLAIN", "1")
        .env("BLOTTER_NOW", "not-a-timestamp")
        .arg("--file")
        .arg(&file)
        .args(["hook", "exec", "claude-code"])
        .write_stdin(payload.clone())
        .output()
        .unwrap();
    hook_exec_explains(
        &output,
        "hook exec: clock could not be resolved (\"BLOTTER_NOW must be a full RFC3339 timestamp\"); skipped",
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "");

    let silent = command()
        .env("BLOTTER_NOW", "not-a-timestamp")
        .arg("--file")
        .arg(&file)
        .args(["hook", "exec", "claude-code"])
        .write_stdin(payload)
        .output()
        .unwrap();
    hook_exec_is_silent(&silent);
}

#[test]
fn hook_exec_explain_is_silent_unless_enabled_exactly() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    std::fs::write(&file, "").unwrap();
    let wrong_event = json!({
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": "cargo test"}
    })
    .to_string();

    hook_exec_is_silent(&hook_exec_claude_code(&file, wrong_event.clone()));
    hook_exec_is_silent(&hook_exec_claude_code_with_explain(&file, wrong_event, "0"));
}

#[test]
fn hook_exec_claude_code_dedupes_open_commands_but_refiles_resolved_cuts() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    std::fs::write(&file, "").unwrap();
    let payload = claude_bash_failure("cargo test", temp.path()).to_string();

    hook_exec_is_silent(&hook_exec_claude_code(&file, payload.clone()));
    hook_exec_is_silent(&hook_exec_claude_code(&file, payload.clone()));
    assert_eq!(std::fs::read_to_string(&file).unwrap().lines().count(), 1);

    let first: Value = serde_json::from_str(
        std::fs::read_to_string(&file)
            .unwrap()
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    let id = first["id"].as_str().unwrap();
    let resolved: SuccessEnvelope<ResolveData> =
        success(&run_file(&file, &["resolve", id, "--agent", "tester"]));
    assert!(resolved.data.changed);

    let refiled = command()
        .env("BLOTTER_NOW", "2026-07-10T18:30:00.123456Z")
        .arg("--file")
        .arg(&file)
        .args(["hook", "exec", "claude-code"])
        .write_stdin(payload)
        .output()
        .unwrap();
    hook_exec_is_silent(&refiled);
    assert_eq!(std::fs::read_to_string(&file).unwrap().lines().count(), 3);
}

#[test]
fn hook_install_claude_code_creates_idempotently_and_preserves_settings() {
    let temp = TempDir::new().unwrap();
    let settings = temp.path().join("nested/settings.json");
    let first: SuccessEnvelope<Value> = success(
        &command()
            .args(["hook", "install", "claude-code", "--settings"])
            .arg(&settings)
            .output()
            .unwrap(),
    );
    assert_eq!(first.data["changed"], true);
    assert!(
        first
            .meta
            .warnings
            .iter()
            .any(|warning| warning == "hook created")
    );
    assert_eq!(
        first.data["settings_path"],
        json!(settings.to_string_lossy().into_owned())
    );
    let hook_command = first.data["command"].as_str().unwrap();
    assert!(hook_command.ends_with("hook exec claude-code"));
    assert!(Path::new(hook_command.strip_suffix(" hook exec claude-code").unwrap()).is_absolute());
    let configured: Value = serde_json::from_slice(&std::fs::read(&settings).unwrap()).unwrap();
    let entry = &configured["hooks"]["PostToolUseFailure"][0];
    assert_eq!(entry["matcher"], "Bash");
    assert_eq!(entry["hooks"][0]["type"], "command");
    assert_eq!(entry["hooks"][0]["command"], hook_command);

    let before = std::fs::read(&settings).unwrap();
    let repeat: SuccessEnvelope<Value> = success(
        &command()
            .args(["hook", "install", "claude-code", "--settings"])
            .arg(&settings)
            .output()
            .unwrap(),
    );
    assert_eq!(repeat.data["changed"], false);
    assert_eq!(std::fs::read(&settings).unwrap(), before);

    let unrelated = temp.path().join("existing.json");
    std::fs::write(
        &unrelated,
        json!({"permissions":{"allow":["Bash"]},"custom":{"keep":true}}).to_string(),
    )
    .unwrap();
    let preserved: SuccessEnvelope<Value> = success(
        &command()
            .args(["hook", "install", "claude-code", "--settings"])
            .arg(&unrelated)
            .output()
            .unwrap(),
    );
    assert_eq!(preserved.data["changed"], true);
    let configured: Value = serde_json::from_slice(&std::fs::read(&unrelated).unwrap()).unwrap();
    assert_eq!(configured["permissions"]["allow"], json!(["Bash"]));
    assert_eq!(configured["custom"], json!({"keep":true}));

    let invalid = temp.path().join("invalid.json");
    std::fs::write(&invalid, "{not valid json}").unwrap();
    let invalid_before = std::fs::read(&invalid).unwrap();
    let invalid_output = command()
        .args(["hook", "install", "claude-code", "--settings"])
        .arg(&invalid)
        .output()
        .unwrap();
    error(&invalid_output, 65, "invalid_input");
    assert_eq!(std::fs::read(&invalid).unwrap(), invalid_before);
}

#[test]
fn hook_install_preserves_existing_key_order() {
    let temp = TempDir::new().unwrap();
    let settings = temp.path().join("settings.json");
    let initial = r#"{
  "model": "sonnet",
  "env": {
    "FIRST": "one",
    "SECOND": "two"
  },
  "cleanupPeriodDays": 30,
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "echo pre-existing"
          }
        ]
      }
    ]
  }
}"#;
    std::fs::write(&settings, initial).unwrap();

    let mut expected: Value = serde_json::from_str(initial).unwrap();
    let installed: SuccessEnvelope<Value> = success(
        &command()
            .args(["hook", "install", "claude-code", "--settings"])
            .arg(&settings)
            .output()
            .unwrap(),
    );
    assert_eq!(installed.data["changed"], true);
    let hook_command = installed.data["command"].as_str().unwrap();
    expected["hooks"]["PostToolUseFailure"] = json!([{
        "matcher": "Bash",
        "hooks": [{"type": "command", "command": hook_command}],
    }]);

    let configured: Value = serde_json::from_slice(&std::fs::read(&settings).unwrap()).unwrap();
    assert_eq!(configured, expected);

    let top_level_keys: Vec<_> = configured
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        top_level_keys,
        ["model", "env", "cleanupPeriodDays", "hooks"]
    );
    let hook_keys: Vec<_> = configured["hooks"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(hook_keys, ["PreToolUse", "PostToolUseFailure"]);
    let existing_hook_keys: Vec<_> = configured["hooks"]["PreToolUse"][0]["hooks"][0]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(existing_hook_keys, ["type", "command"]);
}

#[test]
fn hook_install_dry_run_contract() {
    let temp = TempDir::new().unwrap();
    let settings = temp.path().join("dry-run/settings.json");
    let dry_run: SuccessEnvelope<Value> = success(
        &command()
            .args(["hook", "install", "claude-code", "--settings"])
            .arg(&settings)
            .arg("--dry-run")
            .output()
            .unwrap(),
    );
    assert_eq!(dry_run.data["changed"], true);
    assert!(
        dry_run
            .meta
            .warnings
            .iter()
            .any(|warning| warning.contains("dry run"))
    );
    assert!(
        dry_run
            .meta
            .warnings
            .iter()
            .any(|warning| warning == "dry run; hook would be created")
    );
    assert!(!settings.exists());
    assert!(!settings.parent().unwrap().exists());
}

#[test]
fn hook_help_lists_only_claude_code_target() {
    let output = command()
        .args(["hook", "install", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("claude-code"));
    assert!(!stdout.to_lowercase().contains("codex"));
}

#[test]
fn schema_documents_hook_command_family() {
    let schema: SuccessEnvelope<Value> = success(&run(&["schema"]));
    let hook = &schema.data["commands"]["hook"];
    assert_eq!(hook["targets"], json!(["claude-code"]));
    assert_eq!(hook["install"]["positional"], "claude-code");
    assert_eq!(hook["exec"]["positional"], "claude-code");
    assert!(!hook.to_string().to_lowercase().contains("codex"));
    assert!(
        hook["install"]["flags"]["--settings"]
            .as_str()
            .unwrap()
            .contains("PATH")
    );
    assert!(
        hook["install"]["flags"]["--global"]
            .as_str()
            .unwrap()
            .contains("conflicts")
    );
    assert!(
        hook["exec"]["contract"]
            .as_str()
            .unwrap()
            .contains("exit 0")
    );
    assert!(
        hook["exec"]["contract"]
            .as_str()
            .unwrap()
            .contains("stdout")
    );
}

#[test]
fn schema_documents_hook_only_source_provenance() {
    let schema: SuccessEnvelope<Value> = success(&run(&["schema"]));
    assert_eq!(
        schema.data["records"]["cut"]["source"],
        "optional opaque string; omitted for self-reports; hook exec claude-code writes hook"
    );
    assert!(
        schema.data["commands"]["add"]["flags"]
            .get("--source")
            .is_none()
    );
    assert!(
        schema.data["commands"]["hook"]["exec"]["contract"]
            .as_str()
            .unwrap()
            .contains("source hook")
    );
}

#[test]
fn schema_documents_hook_exec_payload_contract_and_explain_env() {
    let schema: SuccessEnvelope<Value> = success(&run(&["schema"]));
    let exec = &schema.data["commands"]["hook"]["exec"];
    assert_eq!(
        exec["payload"],
        json!({
            "read_fields": [
                "hook_event_name",
                "tool_name",
                "tool_input.command",
                "error",
                "is_interrupt",
                "cwd"
            ],
            "required_fields": ["hook_event_name", "tool_name", "tool_input.command"],
            "stdin_max_bytes": 1_048_576,
            "gates": {
                "hook_event_name": "must equal PostToolUseFailure",
                "tool_name": "must equal Bash",
                "is_interrupt": "must not be true",
                "tool_input.command": "must be non-empty",
                "tool_input.command_bytes": "must be at most 500; longer commands are noise and are skipped",
                "tool_input.command_shape": "best-effort scan with single- and double-quote state must find no unquoted &&, ||, ;, |, newline, $(, or backtick and must not end inside a quote; a chain's exit does not name the friction, so chained, substituting, and unterminated-quote commands are skipped",
                "tool_input.command_program": "best-effort first program after leading VAR=value assignments (basename only) must not be a read-only probe; non-zero exit is an expected answer and grep, rg, ls, find, tail, head, cat, stat, test, [, which, curl, and gh are skipped",
                "resolved_log_file": "must already exist"
            }
        })
    );
    assert_eq!(
        exec["explain"],
        json!({
            "env": "BLOTTER_HOOK_EXPLAIN",
            "enabled_value": "1",
            "contract": "when set exactly to 1, writes one best-effort human-readable reason to stderr; stdout remains empty and exit remains 0"
        })
    );
    assert_eq!(
        schema.data["env"]["BLOTTER_HOOK_EXPLAIN"],
        "hook exec diagnostics when set exactly to 1; one best-effort stderr line, stdout empty, exit 0"
    );
}

#[test]
fn hook_install_repairs_stale_executable_path_without_touching_unmanaged_commands() {
    let temp = TempDir::new().unwrap();
    let settings = temp.path().join("settings.json");
    let unmanaged_command = "printf leave-me-alone";
    std::fs::write(
        &settings,
        json!({
            "permissions": {"allow": ["Bash"]},
            "hooks": {
                "PostToolUseFailure": [{
                    "matcher": "Bash",
                    "hooks": [
                        {
                            "type": "command",
                            "command": "/old/path/papercuts hook exec claude-code"
                        },
                        {"type": "command", "command": unmanaged_command}
                    ]
                }],
                "PreToolUse": [{
                    "matcher": "Bash",
                    "hooks": [{"type": "command", "command": "echo pre-tool"}]
                }]
            }
        })
        .to_string(),
    )
    .unwrap();

    let repaired: SuccessEnvelope<Value> = success(
        &command()
            .args(["hook", "install", "claude-code", "--settings"])
            .arg(&settings)
            .output()
            .unwrap(),
    );
    assert_eq!(repaired.data["changed"], true);
    assert!(
        repaired
            .meta
            .warnings
            .iter()
            .any(|warning| warning == "hook amended")
    );
    let current_command = repaired.data["command"].as_str().unwrap();
    let expected_current_command = format!(
        "{} hook exec claude-code",
        Path::new(env!("CARGO_BIN_EXE_blotter")).display()
    );
    assert_eq!(current_command, expected_current_command);
    assert!(current_command.ends_with("hook exec claude-code"));

    let repaired_settings: Value =
        serde_json::from_slice(&std::fs::read(&settings).unwrap()).unwrap();
    let post_tool_hooks = &repaired_settings["hooks"]["PostToolUseFailure"][0]["hooks"];
    assert_eq!(post_tool_hooks[0]["command"], current_command);
    assert_eq!(post_tool_hooks[1]["command"], unmanaged_command);
    assert_eq!(
        repaired_settings["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
        "echo pre-tool"
    );
    assert_eq!(repaired_settings["permissions"]["allow"], json!(["Bash"]));

    let after_repair = std::fs::read(&settings).unwrap();
    let reinstall: SuccessEnvelope<Value> = success(
        &command()
            .args(["hook", "install", "claude-code", "--settings"])
            .arg(&settings)
            .output()
            .unwrap(),
    );
    assert_eq!(reinstall.data["changed"], false);
    assert_eq!(std::fs::read(&settings).unwrap(), after_repair);
}

#[test]
fn hook_exec_never_appends_a_second_line_with_an_existing_cut_id() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    std::fs::write(&file, "").unwrap();
    let payload = claude_bash_failure("cargo test", temp.path()).to_string();

    hook_exec_is_silent(&hook_exec_claude_code(&file, payload.clone()));
    let first: Value = serde_json::from_str(
        std::fs::read_to_string(&file)
            .unwrap()
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    let id = first["id"].as_str().unwrap().to_owned();
    let resolved: SuccessEnvelope<ResolveData> =
        success(&run_file(&file, &["resolve", &id, "--agent", "tester"]));
    assert!(resolved.data.changed);

    // Same frozen clock, same command: the replay recomputes the resolved cut's
    // ID, so the append is skipped rather than duplicating the ID.
    let replay = hook_exec_claude_code_with_explain(&file, payload, "1");
    hook_exec_explains(
        &replay,
        &format!("hook exec: computed cut ID {id} already exists in the log; skipped"),
    );
    let cut_lines = std::fs::read_to_string(&file)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|line| line["id"] == id.as_str() && line["kind"] == "cut")
        .count();
    assert_eq!(cut_lines, 1);
    assert_eq!(std::fs::read_to_string(&file).unwrap().lines().count(), 2);
}
