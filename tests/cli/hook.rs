//! The retired claude-code auto-capture lane (design doc r32).
//!
//! `hook exec claude-code` stays reachable so an already-installed harness hook cannot fail
//! into its host session, but it files nothing. `hook install claude-code` is gone.

use crate::common::*;

fn claude_bash_failure(command: &str, cwd: &Path) -> Value {
    json!({
        "hook_event_name": "PostToolUseFailure",
        "tool_name": "Bash",
        "tool_input": {"command": command, "description": "run a command"},
        "tool_use_id": "toolu_123",
        "error": "Command exited with non-zero status code 1; API_KEY=super-secret-token",
        "is_interrupt": false,
        "duration_ms": 42,
        "cwd": cwd,
        "session_id": "session_123"
    })
}

fn hook_exec(file: Option<&Path>, stdin: impl Into<Vec<u8>>) -> std::process::Output {
    let mut cmd = command();
    if let Some(file) = file {
        cmd.arg("--file").arg(file);
    }
    cmd.args(["hook", "exec", "claude-code"])
        .write_stdin(stdin)
        .output()
        .unwrap()
}

#[test]
fn hook_exec_files_nothing_and_stays_silent_on_a_capturable_payload() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    // A previously capturable payload: simple command, non-probe program, existing log.
    add(&file, "an unrelated hand-filed cut");
    let before = std::fs::read_to_string(&file).unwrap();

    let output = hook_exec(
        Some(&file),
        claude_bash_failure("cargo build --release", temp.path()).to_string(),
    );

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
}

#[test]
fn hook_exec_never_creates_a_log() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let output = hook_exec(
        Some(&file),
        claude_bash_failure("cargo test", temp.path()).to_string(),
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty());
    assert!(!file.exists());
}

/// Fail-open is the whole reason the receiver survives: nothing an installed hook can hand it
/// may produce a non-zero exit or a byte on stdout.
#[test]
fn hook_exec_fails_open_on_every_unusable_input() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    std::fs::write(&file, "").unwrap();
    let oversized = "x".repeat(2 * 1024 * 1024);
    for stdin in ["", "not json at all", "{}", "[]", oversized.as_str()] {
        let output = hook_exec(Some(&file), stdin);
        assert_eq!(
            output.status.code(),
            Some(0),
            "stdin len {} exited non-zero",
            stdin.len()
        );
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
        assert!(std::fs::read_to_string(&file).unwrap().is_empty());
    }
}

/// The receiver resolves no clock and no agent, so an environment fault that fails every other
/// command cannot reach it.
#[test]
fn hook_exec_ignores_an_unusable_environment() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    std::fs::write(&file, "").unwrap();
    let output = command()
        .env("BLOTTER_NOW", "not-a-timestamp")
        .arg("--file")
        .arg(&file)
        .args(["hook", "exec", "claude-code"])
        .write_stdin(claude_bash_failure("cargo test", temp.path()).to_string())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    assert!(std::fs::read_to_string(&file).unwrap().is_empty());
}

#[test]
fn hook_explain_names_the_retirement_without_touching_stdout() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    std::fs::write(&file, "").unwrap();
    let output = command()
        .env("BLOTTER_HOOK_EXPLAIN", "1")
        .arg("--file")
        .arg(&file)
        .args(["hook", "exec", "claude-code"])
        .write_stdin(claude_bash_failure("cargo build", temp.path()).to_string())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(stderr.lines().count(), 1);
    assert!(stderr.contains("retired"), "stderr={stderr}");
    assert!(stderr.contains("PostToolUseFailure"), "stderr={stderr}");

    // Any other value keeps the receiver silent.
    let quiet = command()
        .env("BLOTTER_HOOK_EXPLAIN", "true")
        .arg("--file")
        .arg(&file)
        .args(["hook", "exec", "claude-code"])
        .write_stdin("{}")
        .output()
        .unwrap();
    assert_eq!(quiet.status.code(), Some(0));
    assert!(quiet.stderr.is_empty());
}

#[test]
fn hook_install_is_removed_and_writes_no_settings() {
    let temp = TempDir::new().unwrap();
    let settings = temp.path().join(".claude/settings.json");
    let output = command()
        .current_dir(temp.path())
        .args(["hook", "install", "claude-code"])
        .output()
        .unwrap();
    error(&output, 2, "invalid_argument");
    assert!(!settings.exists());
}

#[test]
fn schema_documents_the_retired_hook_lane() {
    let schema: SuccessEnvelope<Value> = success(&run(&["schema"]));
    let hook = &schema.data["commands"]["hook"];
    assert!(hook["install"].is_null());
    assert!(hook["exec"]["payload"].is_null());
    assert!(
        hook["semantics"].as_str().unwrap().contains("retired"),
        "{hook}"
    );
    assert_eq!(hook["exec"]["appends"], false);
    assert_eq!(hook["exec"]["read_only"], true);
    assert_eq!(hook["exec"]["output"], "none");
    assert_eq!(hook["exec"]["stdin_max_bytes"], 1_048_576);
    assert!(
        schema.data["records"]["cut"]["source"]
            .as_str()
            .unwrap()
            .contains("no command writes it")
    );
}
