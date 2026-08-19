pub use assert_cmd::Command;
pub use blotter::commands::add::AddData;
pub use blotter::commands::doctor::DoctorData;
pub use blotter::commands::list::ListData;
pub use blotter::commands::resolve::ResolveData;
pub use blotter::commands::sweep::SweepData;
pub use blotter::error::exit_code_map;
pub use blotter::output::{ErrorEnvelope, SuccessEnvelope};
pub use blotter::{Evidence, ItemStatus, LogEvent, Severity, compute_dogear_id, compute_id};
pub use serde::de::DeserializeOwned;
pub use serde_json::{Value, json};
pub use std::collections::HashMap;
pub use std::fs::OpenOptions;
pub use std::io::Write;
#[cfg(unix)]
pub use std::os::unix::fs::PermissionsExt;
pub use std::path::Path;
#[cfg(unix)]
pub use std::process::Stdio;
pub use std::sync::{Arc, Barrier};
pub use std::thread;
pub use tempfile::TempDir;

pub const NOW: &str = "2026-07-09T18:30:00.123456Z";

pub trait CutEventExt {
    fn cut_id(&self) -> &str;
    fn cut_ts(&self) -> &str;
    fn cut_agent(&self) -> &str;
    fn cut_tags(&self) -> &[String];
    fn cut_cwd(&self) -> &str;
    fn cut_evidence(&self) -> Option<&Evidence>;
}

impl CutEventExt for LogEvent {
    fn cut_id(&self) -> &str {
        match self {
            LogEvent::Cut { id, .. } => id,
            _ => panic!("add responses must contain cut events"),
        }
    }

    fn cut_ts(&self) -> &str {
        match self {
            LogEvent::Cut { ts, .. } => ts,
            _ => panic!("add responses must contain cut events"),
        }
    }

    fn cut_agent(&self) -> &str {
        match self {
            LogEvent::Cut { agent, .. } => agent,
            _ => panic!("add responses must contain cut events"),
        }
    }

    fn cut_tags(&self) -> &[String] {
        match self {
            LogEvent::Cut { tags, .. } => tags,
            _ => panic!("add responses must contain cut events"),
        }
    }

    fn cut_cwd(&self) -> &str {
        match self {
            LogEvent::Cut { cwd, .. } => cwd,
            _ => panic!("add responses must contain cut events"),
        }
    }

    fn cut_evidence(&self) -> Option<&Evidence> {
        match self {
            LogEvent::Cut { evidence, .. } => evidence.as_ref(),
            _ => panic!("add responses must contain cut events"),
        }
    }
}

pub fn command() -> Command {
    let mut command = assert_cmd::cargo::cargo_bin_cmd!("blotter");
    command
        .env("BLOTTER_NOW", NOW)
        .env_remove("BLOTTER_FILE")
        .env_remove("BLOTTER_AGENT")
        .env_remove("BLOTTER_HOOK_EXPLAIN")
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
    command
}

pub fn run(args: &[&str]) -> std::process::Output {
    command().args(args).output().unwrap()
}

pub fn run_file(file: &Path, args: &[&str]) -> std::process::Output {
    command()
        .arg("--file")
        .arg(file)
        .args(args)
        .output()
        .unwrap()
}

#[cfg(unix)]
pub fn permissions_mode(path: &Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

pub fn temp_has_git_ancestor(temp: &TempDir) -> bool {
    temp.path()
        .ancestors()
        .any(|ancestor| ancestor.join(".git").exists())
}

pub fn make_repo(path: &Path) {
    std::fs::create_dir_all(path).unwrap();
    std::fs::write(path.join(".git"), "gitdir: elsewhere\n").unwrap();
}

pub fn success<T: DeserializeOwned>(output: &std::process::Output) -> SuccessEnvelope<T> {
    assert!(
        output.status.success(),
        "status={:?}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    serde_json::from_slice(&output.stdout).unwrap()
}

pub fn error(output: &std::process::Output, exit: i32, code: &str) -> ErrorEnvelope {
    assert_eq!(output.status.code(), Some(exit));
    assert!(output.stdout.is_empty());
    let envelope: ErrorEnvelope = serde_json::from_slice(&output.stderr).unwrap();
    assert!(!envelope.ok);
    assert_eq!(envelope.error.code, code);
    assert!(!envelope.error.suggested_fix.is_empty());
    assert_eq!(envelope.meta.contract, 5);
    envelope
}

pub fn doctor_response(output: &std::process::Output, exit: i32) -> SuccessEnvelope<DoctorData> {
    assert_eq!(
        output.status.code(),
        Some(exit),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    serde_json::from_slice(&output.stdout).unwrap()
}

pub fn add(file: &Path, text: &str) -> SuccessEnvelope<AddData> {
    let output = run_file(file, &["add", text, "--agent", "tester"]);
    success(&output)
}

pub fn add_at(file: &Path, now: &str, text: &str, tags: &[&str]) -> SuccessEnvelope<AddData> {
    let mut cmd = command();
    cmd.env("BLOTTER_NOW", now)
        .arg("--file")
        .arg(file)
        .args(["add", text, "--agent", "tester"]);
    for tag in tags {
        cmd.arg("--tag").arg(*tag);
    }
    success(&cmd.output().unwrap())
}

pub fn add_with_cmd_at(
    file: &Path,
    now: &str,
    text: &str,
    tags: &[&str],
    failed_command: &str,
) -> SuccessEnvelope<AddData> {
    let mut cmd = command();
    cmd.env("BLOTTER_NOW", now)
        .arg("--file")
        .arg(file)
        .args(["add", text, "--agent", "tester", "--cmd"])
        .arg(failed_command);
    for tag in tags {
        cmd.arg("--tag").arg(*tag);
    }
    success(&cmd.output().unwrap())
}

pub fn dogear_at(file: &Path, now: &str, text: &str, tags: &[&str]) -> SuccessEnvelope<Value> {
    let mut cmd = command();
    cmd.env("BLOTTER_NOW", now)
        .arg("--file")
        .arg(file)
        .args(["dogear", text, "--agent", "tester"]);
    for tag in tags {
        cmd.arg("--tag").arg(*tag);
    }
    success(&cmd.output().unwrap())
}

pub fn triage_success(output: &std::process::Output, exit: i32) -> SuccessEnvelope<Value> {
    assert_eq!(
        output.status.code(),
        Some(exit),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let envelope: SuccessEnvelope<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert!(envelope.ok);
    envelope
}

pub fn verify_success(output: &std::process::Output, exit: i32) -> SuccessEnvelope<Value> {
    triage_success(output, exit)
}

pub fn retrospect_success(output: &std::process::Output, exit: i32) -> SuccessEnvelope<Value> {
    triage_success(output, exit)
}

pub fn resolve_at(file: &Path, now: &str, id: &str, args: &[&str]) -> SuccessEnvelope<ResolveData> {
    let mut cmd = command();
    cmd.env("BLOTTER_NOW", now)
        .arg("--file")
        .arg(file)
        .arg("resolve")
        .arg(id)
        .args(args);
    success(&cmd.output().unwrap())
}

pub fn resolve_line(id: &str, ts: &str, note: &str, amend: bool) -> String {
    let mut value = json!({"kind":"resolve","id":id,"ts":ts,"agent":"fixer","note":note});
    if amend {
        value["amend"] = json!(true);
    }
    value.to_string()
}

pub fn append_lines(file: &Path, lines: &[String]) {
    let mut log = std::fs::read_to_string(file).unwrap();
    for line in lines {
        log.push_str(line);
        log.push('\n');
    }
    std::fs::write(file, log).unwrap();
}

pub fn hook_exec_claude_code(file: &Path, stdin: impl Into<Vec<u8>>) -> std::process::Output {
    command()
        .arg("--file")
        .arg(file)
        .args(["hook", "exec", "claude-code"])
        .write_stdin(stdin)
        .output()
        .unwrap()
}

pub fn hook_exec_is_silent(output: &std::process::Output) {
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

pub fn claude_bash_failure(command: &str, cwd: &Path) -> Value {
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
