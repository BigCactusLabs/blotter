pub use assert_cmd::Command;
pub use blotter::commands::add::AddData;
pub use blotter::commands::doctor::DoctorData;
pub use blotter::commands::list::ListData;
pub use blotter::commands::resolve::ResolveData;
pub use blotter::commands::sweep::SweepData;
pub use blotter::error::exit_code_map;
pub use blotter::output::{ErrorEnvelope, SuccessEnvelope};
pub use blotter::{
    Disposition, Evidence, Impact, ItemStatus, LogEvent, Origin, compute_dogear_id, compute_id,
};
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
    Command::from_std(spawn_command())
}

/// The same clean environment as `command()`, as a `std::process::Command`,
/// for tests that must spawn blotter and act while it runs.
pub fn spawn_command() -> std::process::Command {
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
    assert_eq!(envelope.meta.contract, 6);
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

/// Resolve one record at a fixed clock. A cut needs `--disposition`, so this
/// supplies `fixed` unless the caller is exercising a lane that must not carry
/// one: an explicit `--disposition`, an `--amend` (where it is optional and
/// inherited), or a dogear's `--url`/`--dropped` lifecycle.
pub fn resolve_at(file: &Path, now: &str, id: &str, args: &[&str]) -> SuccessEnvelope<ResolveData> {
    let mut cmd = command();
    cmd.env("BLOTTER_NOW", now)
        .arg("--file")
        .arg(file)
        .arg("resolve");
    if !args
        .iter()
        .any(|arg| matches!(*arg, "--disposition" | "--amend" | "--url" | "--dropped"))
    {
        cmd.args(["--disposition", "fixed"]);
    }
    cmd.arg(id).args(args);
    success(&cmd.output().unwrap())
}

/// A resolve line for a **cut**: it carries `disposition` and `disposition_ts`,
/// without which the fold discards it as invalid. Every v2 identity is one
/// width (r51), so the kind cannot be read off the ID.
pub fn resolve_line(id: &str, ts: &str, note: &str, amend: bool) -> String {
    let mut value = json!({"v":2,"kind":"resolve","id":id,"ts":ts,"agent":"fixer","note":note});
    if amend {
        value["amend"] = json!(true);
    }
    value["disposition"] = json!("fixed");
    value["disposition_ts"] = json!(ts);
    value.to_string()
}

/// A 0.15 (v1) cut line: an object whose raw `kind` is one the probe knows,
/// carrying no `v`. Any log holding one is refused whole.
pub fn v1_cut_line() -> String {
    json!({
        "kind": "cut",
        "id": "bl_a1b2c3d4e5f6",
        "ts": "2026-07-09T00:00:00.000Z",
        "agent": "legacy",
        "text": "v1 cut",
        "tags": [],
        "severity": "minor",
        "cwd": "/tmp"
    })
    .to_string()
}

/// A v2 cut line, for the mixed-log case.
pub fn v2_cut_line(text: &str) -> String {
    let id = compute_id("2026-07-09T00:00:00.000Z", "tester", text, Impact::Low, &[]);
    json!({
        "v": 2,
        "kind": "cut",
        "id": id,
        "ts": "2026-07-09T00:00:00.000Z",
        "agent": "tester",
        "text": text,
        "tags": [],
        "impact": "low",
        "cwd": "/tmp"
    })
    .to_string()
}

/// The exact `suggested_fix` the upgrade refusal carries. It names the resolved
/// path, instructs a rename to a path that does not yet exist followed by
/// `blotter add`, and carries no literal `mv` (r48, r49).
pub fn unsupported_version_fix(file: &Path) -> String {
    format!(
        "Rename {} to a path that does not yet exist, then run `blotter add` to create a fresh v2 log.",
        file.display()
    )
}

/// Every path in a directory, sorted — used to prove a refusal created no
/// backup, quarantine, or archive sidecar beside the log.
pub fn directory_entries(directory: &Path) -> Vec<String> {
    let mut names = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    names.sort();
    names
}

/// The stored line for an envelope record: `v` first, then the record's own
/// members. `v` is a storage marker and appears in no envelope (r50).
pub fn stored_line(record: &Value) -> Value {
    let mut line = serde_json::Map::new();
    line.insert("v".into(), json!(2));
    for (key, value) in record.as_object().expect("records are JSON objects") {
        line.insert(key.clone(), value.clone());
    }
    Value::Object(line)
}

pub fn append_lines(file: &Path, lines: &[String]) {
    let mut log = std::fs::read_to_string(file).unwrap();
    for line in lines {
        log.push_str(line);
        log.push('\n');
    }
    std::fs::write(file, log).unwrap();
}
