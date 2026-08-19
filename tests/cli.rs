use assert_cmd::Command;
use blotter::commands::add::AddData;
use blotter::commands::doctor::DoctorData;
use blotter::commands::list::ListData;
use blotter::commands::resolve::ResolveData;
use blotter::commands::sweep::SweepData;
use blotter::error::exit_code_map;
use blotter::output::{ErrorEnvelope, SuccessEnvelope};
use blotter::{Evidence, ItemStatus, LogEvent, Severity, compute_dogear_id, compute_id};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
#[cfg(unix)]
use std::process::Stdio;
use std::sync::{Arc, Barrier};
use std::thread;
use tempfile::TempDir;

const NOW: &str = "2026-07-09T18:30:00.123456Z";

trait CutEventExt {
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

fn command() -> Command {
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

fn run(args: &[&str]) -> std::process::Output {
    command().args(args).output().unwrap()
}

fn run_file(file: &Path, args: &[&str]) -> std::process::Output {
    command()
        .arg("--file")
        .arg(file)
        .args(args)
        .output()
        .unwrap()
}

#[cfg(unix)]
fn permissions_mode(path: &Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

fn temp_has_git_ancestor(temp: &TempDir) -> bool {
    temp.path()
        .ancestors()
        .any(|ancestor| ancestor.join(".git").exists())
}

fn make_repo(path: &Path) {
    std::fs::create_dir_all(path).unwrap();
    std::fs::write(path.join(".git"), "gitdir: elsewhere\n").unwrap();
}

fn success<T: DeserializeOwned>(output: &std::process::Output) -> SuccessEnvelope<T> {
    assert!(
        output.status.success(),
        "status={:?}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    serde_json::from_slice(&output.stdout).unwrap()
}

fn error(output: &std::process::Output, exit: i32, code: &str) -> ErrorEnvelope {
    assert_eq!(output.status.code(), Some(exit));
    assert!(output.stdout.is_empty());
    let envelope: ErrorEnvelope = serde_json::from_slice(&output.stderr).unwrap();
    assert!(!envelope.ok);
    assert_eq!(envelope.error.code, code);
    assert!(!envelope.error.suggested_fix.is_empty());
    assert_eq!(envelope.meta.contract, 5);
    envelope
}

fn doctor_response(output: &std::process::Output, exit: i32) -> SuccessEnvelope<DoctorData> {
    assert_eq!(
        output.status.code(),
        Some(exit),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    serde_json::from_slice(&output.stdout).unwrap()
}

fn add(file: &Path, text: &str) -> SuccessEnvelope<AddData> {
    let output = run_file(file, &["add", text, "--agent", "tester"]);
    success(&output)
}

fn add_at(file: &Path, now: &str, text: &str, tags: &[&str]) -> SuccessEnvelope<AddData> {
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

fn add_with_cmd_at(
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

fn dogear_at(file: &Path, now: &str, text: &str, tags: &[&str]) -> SuccessEnvelope<Value> {
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

fn triage_success(output: &std::process::Output, exit: i32) -> SuccessEnvelope<Value> {
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

fn verify_success(output: &std::process::Output, exit: i32) -> SuccessEnvelope<Value> {
    triage_success(output, exit)
}

fn retrospect_success(output: &std::process::Output, exit: i32) -> SuccessEnvelope<Value> {
    triage_success(output, exit)
}

fn resolve_at(file: &Path, now: &str, id: &str, args: &[&str]) -> SuccessEnvelope<ResolveData> {
    let mut cmd = command();
    cmd.env("BLOTTER_NOW", now)
        .arg("--file")
        .arg(file)
        .arg("resolve")
        .arg(id)
        .args(args);
    success(&cmd.output().unwrap())
}

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
fn add_evidence_flags_are_redacted_and_stderr_stays_bounded_at_4096() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let stderr_file = temp.path().join("stderr.txt");
    std::fs::write(&stderr_file, format!("{}éafter-boundary", "x".repeat(4095))).unwrap();
    let output = command()
        .arg("--file")
        .arg(&file)
        .args([
            "add",
            "tool failed",
            "--agent",
            "tester",
            "--cmd",
            "curl -H 'Authorization: abc123'",
            "--exit",
            "7",
            "--stderr-file",
        ])
        .arg(&stderr_file)
        .args([
            "--evidence",
            "API_KEY=sk_live_secret token: abc password='hunter2' ghp_AbCdEf0123456789GhIjKlMnOpQrStUv monkey=keep tokenized=keep",
        ])
        .output()
        .unwrap();
    let added: SuccessEnvelope<AddData> = success(&output);
    let evidence = added.data.record.cut_evidence().unwrap();
    assert_eq!(evidence.exit, Some(7));
    assert!(!evidence.cmd.as_deref().unwrap().contains("abc123"));
    assert!(!evidence.note.as_deref().unwrap().contains("sk_live_secret"));
    assert!(
        !evidence
            .note
            .as_deref()
            .unwrap()
            .contains("ghp_AbCdEf0123456789GhIjKlMnOpQrStUv")
    );
    assert!(evidence.note.as_deref().unwrap().contains("monkey=keep"));
    assert!(evidence.note.as_deref().unwrap().contains("tokenized=keep"));
    assert_eq!(evidence.stderr.as_deref().unwrap().len(), 4095);
    assert!(
        !evidence
            .stderr
            .as_deref()
            .unwrap()
            .contains("after-boundary")
    );

    let absent = add(&temp.path().join("absent.jsonl"), "no evidence");
    let absent_json = serde_json::to_value(&absent.data.record).unwrap();
    assert!(absent_json.get("evidence").is_none());

    let missing_stderr = run_file(
        &temp.path().join("missing-stderr.jsonl"),
        &["add", "missing stderr", "--stderr-file", "does-not-exist"],
    );
    let missing = error(&missing_stderr, 66, "not_found");
    assert!(
        missing
            .error
            .message
            .starts_with("stderr evidence file not found:")
    );
    assert!(missing.error.suggested_fix.contains("--stderr-file PATH"));
}

#[test]
fn add_text_keeps_secret_shaped_authored_content() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let text = "authored description: authorization: Bearer authored-secret";

    let added: SuccessEnvelope<AddData> = success(
        &command()
            .env("HOME", "/Users/alice")
            .arg("--file")
            .arg(&file)
            .args(["add", text, "--agent", "tester"])
            .output()
            .unwrap(),
    );
    let record = serde_json::to_value(&added.data.record).unwrap();
    assert_eq!(record["text"], text);
    assert_eq!(
        record["id"],
        compute_id(
            record["ts"].as_str().unwrap(),
            "tester",
            text,
            Severity::Minor,
            &[]
        )
    );
    assert!(
        std::fs::read_to_string(&file)
            .unwrap()
            .contains("authored-secret")
    );
}

#[test]
fn padded_standalone_base64_is_redacted_in_stdout_and_jsonl() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let tokens = [
        "AbCdEf0123456789GhIjKlMnOpQrStUvWxYz+/=",
        "ZyXwVu9876543210TsRqPoNmLkJiHgFeDcBa+/==",
    ];
    for (index, token) in tokens.iter().enumerate() {
        let text = format!("padded token {index}");
        let output = command()
            .arg("--file")
            .arg(&file)
            .args(["add", &text, "--agent", "tester", "--evidence"])
            .arg(token)
            .output()
            .unwrap();
        let added: SuccessEnvelope<AddData> = success(&output);
        assert_eq!(
            added.data.record.cut_evidence().unwrap().note.as_deref(),
            Some("<redacted>")
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(!stdout.contains(token));
    }

    let lines: Vec<Value> = std::fs::read_to_string(&file)
        .unwrap()
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(lines.len(), tokens.len());
    for (line, token) in lines.iter().zip(tokens) {
        assert_eq!(line["evidence"]["note"], "<redacted>");
        assert!(!line.to_string().contains(token));
    }
}

#[test]
fn padded_base64_crossing_stderr_storage_boundary_leaves_no_prefix() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let stderr_file = temp.path().join("stderr.txt");
    let token = "AbCdEf0123456789GhIjKlMnOpQrStUvWxYz+/==";
    std::fs::write(&stderr_file, format!("{} {token}", "p".repeat(4085))).unwrap();

    let added: SuccessEnvelope<AddData> = success(
        &command()
            .arg("--file")
            .arg(&file)
            .args([
                "add",
                "boundary token",
                "--agent",
                "tester",
                "--stderr-file",
            ])
            .arg(&stderr_file)
            .output()
            .unwrap(),
    );
    let expected = format!("{} <redacted>", "p".repeat(4085));
    assert_eq!(
        added.data.record.cut_evidence().unwrap().stderr.as_deref(),
        Some(expected.as_str())
    );
}

#[test]
fn leading_hyphen_evidence_and_resolve_note_work_through_binary() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let added: SuccessEnvelope<AddData> = success(
        &command()
            .arg("--file")
            .arg(&file)
            .args(["add", "hyphen evidence", "--agent", "tester", "--cmd"])
            .arg("--tool --flag with spaces")
            .args(["--evidence"])
            .arg("--detail with spaces")
            .output()
            .unwrap(),
    );
    let evidence = added.data.record.cut_evidence().unwrap();
    assert_eq!(evidence.cmd.as_deref(), Some("--tool --flag with spaces"));
    assert_eq!(evidence.note.as_deref(), Some("--detail with spaces"));

    let resolved: SuccessEnvelope<ResolveData> = success(
        &command()
            .arg("--file")
            .arg(&file)
            .args([
                "resolve",
                added.data.record.cut_id(),
                "--agent",
                "fixer",
                "--note",
            ])
            .arg("--retry after timeout")
            .output()
            .unwrap(),
    );
    assert_eq!(resolved.data.records.len(), 1);
    assert_eq!(
        resolved.data.records[0]
            .resolution
            .as_ref()
            .unwrap()
            .note
            .as_deref(),
        Some("--retry after timeout")
    );
}

#[test]
fn evidence_redacts_sensitive_assignment_values_in_cmd_note_and_stderr() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let stderr_file = temp.path().join("stderr.txt");
    std::fs::write(&stderr_file, "password=stderr-secret").unwrap();
    let output = command()
        .arg("--file")
        .arg(&file)
        .args([
            "add",
            "tool failed",
            "--agent",
            "tester",
            "--cmd",
            "api_key=cmd-secret",
            "--stderr-file",
        ])
        .arg(&stderr_file)
        .args([
            "--evidence",
            "\"access_token\":\"note-secret\" authorization: header-secret",
        ])
        .output()
        .unwrap();
    let added: SuccessEnvelope<AddData> = success(&output);
    let evidence = added.data.record.cut_evidence().unwrap();
    assert_eq!(evidence.cmd.as_deref(), Some("api_key=<redacted>"));
    assert_eq!(evidence.stderr.as_deref(), Some("password=<redacted>"));
    assert_eq!(
        evidence.note.as_deref(),
        Some("\"access_token\":\"<redacted>\" authorization: <redacted>")
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stored = std::fs::read_to_string(&file).unwrap();
    for secret in [
        "cmd-secret",
        "stderr-secret",
        "note-secret",
        "header-secret",
    ] {
        assert!(!stdout.contains(secret));
        assert!(!stored.contains(secret));
    }
}

#[test]
fn evidence_rewrites_home_paths_in_cmd_stderr_and_note_without_midword_rewrites() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let stderr_file = temp.path().join("stderr.txt");
    let home = std::path::PathBuf::from("/private/alice");
    let home_text = home.to_string_lossy();
    let cmd = format!("run {home_text}/cmd /home/other/cache /Users/alice/desk {home_text}x");
    let stderr =
        format!("stderr {home_text}/logs /home/other/cache /Users/alice/desk {home_text}x");
    let note = format!("note {home_text}/notes /home/other/cache /Users/alice/desk {home_text}x");
    std::fs::write(&stderr_file, &stderr).unwrap();

    let added: SuccessEnvelope<AddData> = success(
        &command()
            .env("HOME", &home)
            .arg("--file")
            .arg(&file)
            .args(["add", "home paths", "--agent", "tester", "--cmd"])
            .arg(&cmd)
            .arg("--stderr-file")
            .arg(&stderr_file)
            .arg("--evidence")
            .arg(&note)
            .output()
            .unwrap(),
    );
    let evidence = added.data.record.cut_evidence().unwrap();
    assert_eq!(
        evidence.cmd.as_deref(),
        Some(format!("run ~/cmd ~/cache ~/desk {home_text}x").as_str())
    );
    assert_eq!(
        evidence.stderr.as_deref(),
        Some(format!("stderr ~/logs ~/cache ~/desk {home_text}x").as_str())
    );
    assert_eq!(
        evidence.note.as_deref(),
        Some(format!("note ~/notes ~/cache ~/desk {home_text}x").as_str())
    );
}

#[test]
fn generic_home_paths_require_a_token_start_in_evidence_and_doctor() {
    let temp = TempDir::new().unwrap();
    let evidence_file = temp.path().join("evidence.jsonl");
    let input = "/tmp/Users/bob/x /mnt/home/shared //Users/eve/z /Users/alice/real /home/bob/real";
    let expected = "/tmp/Users/bob/x /mnt/home/shared //Users/eve/z ~/real ~/real";

    let added: SuccessEnvelope<AddData> = success(
        &command()
            .env_remove("HOME")
            .arg("--file")
            .arg(&evidence_file)
            .args(["add", "generic paths", "--agent", "tester", "--evidence"])
            .arg(input)
            .output()
            .unwrap(),
    );
    assert_eq!(
        added.data.record.cut_evidence().unwrap().note.as_deref(),
        Some(expected)
    );

    let log = temp.path().join("doctor.jsonl");
    let paths = [
        "/tmp/Users/bob/x",
        "/mnt/home/shared",
        "//Users/eve/z",
        "/Users/alice/real",
        "/home/bob/real",
    ];
    let records = paths
        .iter()
        .map(|path| {
            let text = format!("path {path}");
            json!({
                "kind": "cut",
                "id": compute_id(NOW, "tester", &text, Severity::Minor, &[]),
                "ts": NOW,
                "agent": "tester",
                "text": text,
                "tags": [],
                "severity": "minor",
                "cwd": "."
            })
            .to_string()
        })
        .collect::<Vec<_>>();
    std::fs::write(&log, format!("{}\n", records.join("\n"))).unwrap();
    let doctor = doctor_response(
        &command()
            .env_remove("HOME")
            .arg("--file")
            .arg(&log)
            .args(["doctor", "--leaks"])
            .output()
            .unwrap(),
        1,
    );
    let leak_lines: Vec<_> = doctor
        .data
        .findings
        .iter()
        .filter(|finding| finding.kind == "leak")
        .map(|finding| finding.line)
        .collect();
    assert_eq!(leak_lines, [4, 5]);
}

#[test]
fn generic_home_paths_are_rewritten_when_home_is_unset_or_empty() {
    let temp = TempDir::new().unwrap();
    for (name, home) in [("unset", None), ("empty", Some(""))] {
        let file = temp.path().join(format!("{name}.jsonl"));
        let mut invocation = command();
        match home {
            Some(home) => {
                invocation.env("HOME", home);
            }
            None => {
                invocation.env_remove("HOME");
            }
        }
        let added: SuccessEnvelope<AddData> = success(
            &invocation
                .arg("--file")
                .arg(&file)
                .args(["add", "generic paths", "--agent", "tester", "--evidence"])
                .arg("/Users/alice/cache /home/bob/log")
                .output()
                .unwrap(),
        );
        assert_eq!(
            added.data.record.cut_evidence().unwrap().note.as_deref(),
            Some("~/cache ~/log")
        );
    }
}

#[test]
fn evidence_rewrite_normalizes_home_and_preserves_nested_path_suffixes() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("root");
    let home = root.join("home");
    std::fs::create_dir_all(&home).unwrap();
    let root = root.canonicalize().unwrap();
    let home = root.join("home");
    let file = temp.path().join("cuts.jsonl");
    let evidence = format!("{}/Users/bob /Users/alice/Users/bob", home.display());

    let added: SuccessEnvelope<AddData> = success(
        &command()
            .env("HOME", "home/")
            .current_dir(&root)
            .arg("--file")
            .arg(&file)
            .args(["add", "home paths", "--agent", "tester", "--evidence"])
            .arg(&evidence)
            .output()
            .unwrap(),
    );
    assert_eq!(
        added.data.record.cut_evidence().unwrap().note.as_deref(),
        Some("~/Users/bob ~/Users/bob")
    );

    let leaking = temp.path().join("leaking.jsonl");
    std::fs::write(
        &leaking,
        format!("not JSON {}/private /home/other/private\n", home.display()),
    )
    .unwrap();
    let doctor = doctor_response(
        &command()
            .env("HOME", "home/")
            .current_dir(&root)
            .arg("--file")
            .arg(&leaking)
            .args(["doctor", "--leaks"])
            .output()
            .unwrap(),
        1,
    );
    let leaks: Vec<_> = doctor
        .data
        .findings
        .iter()
        .filter(|finding| finding.kind == "leak")
        .collect();
    assert_eq!(leaks.len(), 1);
    assert_eq!(leaks[0].line, 1);
    assert!(!leaks[0].fixable);
    assert!(leaks[0].message.contains("home path"));
    assert!(
        doctor
            .data
            .findings
            .iter()
            .any(|finding| finding.kind == "malformed")
    );
}

#[test]
fn evidence_merges_overlapping_redaction_spans_without_leaking_tails() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let added = add_evidence_note(
        &file,
        "password=aB3defghijklmnopqrstuvwx@rest-of-secret-tail",
    );
    // The entropy token starts at "password" (`=` is a token character), so the
    // merged span covers the key too — over-redaction in the safe direction.
    assert_eq!(
        added.data.record.cut_evidence().unwrap().note.as_deref(),
        Some("<redacted>")
    );
}

#[test]
fn evidence_redacts_authorization_scheme_and_credential() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let added = add_evidence_note(
        &file,
        "Authorization: Bearer 0123456789abcdef0123456789abcdef",
    );
    assert_eq!(
        added.data.record.cut_evidence().unwrap().note.as_deref(),
        Some("Authorization: <redacted>")
    );
}

#[test]
fn evidence_redacts_token_only_userinfo_and_preserves_query_fragment_tails() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let added = add_evidence_note(
        &file,
        "push failed https://ghp_TOKENVALUE@example.test/a?b=c#d",
    );
    assert_eq!(
        added.data.record.cut_evidence().unwrap().note.as_deref(),
        Some("push failed https://<redacted>@example.test/a?b=c#d")
    );
}

fn add_evidence_note(file: &Path, note: &str) -> SuccessEnvelope<AddData> {
    success(
        &command()
            .arg("--file")
            .arg(file)
            .args([
                "add",
                "evidence case",
                "--agent",
                "tester",
                "--evidence",
                note,
            ])
            .output()
            .unwrap(),
    )
}

#[test]
fn evidence_redacts_common_compound_names_in_stdout_and_jsonl() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let input = "DB_PASSWORD=value-one client_secret=value-two \"access_token\":\"value-three\" client-secret=value-four api_key=value-five monkey=keep keynotes=keep tokenized=keep";
    let expected = "DB_PASSWORD=<redacted> client_secret=<redacted> \"access_token\":\"<redacted>\" client-secret=<redacted> api_key=<redacted> monkey=keep keynotes=keep tokenized=keep";
    let output = command()
        .arg("--file")
        .arg(&file)
        .args([
            "add",
            "compound names",
            "--agent",
            "tester",
            "--evidence",
            input,
        ])
        .output()
        .unwrap();
    let added: SuccessEnvelope<AddData> = success(&output);
    assert_eq!(
        added
            .data
            .record
            .cut_evidence()
            .as_ref()
            .unwrap()
            .note
            .as_deref(),
        Some(expected)
    );
    let stdout: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(stdout["data"]["record"]["evidence"]["note"], expected);
    let stored: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(stored["evidence"]["note"], expected);
}

#[test]
fn evidence_redacts_lowercase_compound_credential_keys_in_stdout_and_jsonl() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let input = "clientsecret=short-value accesskey=short-value authtoken=short-value dbpassword=also-short monkey=keep keynotes=keep tokenized=keep";
    let expected = "clientsecret=<redacted> accesskey=<redacted> authtoken=<redacted> dbpassword=<redacted> monkey=keep keynotes=keep tokenized=keep";
    let output = command()
        .arg("--file")
        .arg(&file)
        .args([
            "add",
            "lowercase compound names",
            "--agent",
            "tester",
            "--evidence",
        ])
        .arg(input)
        .output()
        .unwrap();
    let added: SuccessEnvelope<AddData> = success(&output);
    assert_eq!(
        added
            .data
            .record
            .cut_evidence()
            .as_ref()
            .unwrap()
            .note
            .as_deref(),
        Some(expected)
    );
    let stdout: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(stdout["data"]["record"]["evidence"]["note"], expected);
    let stored: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(stored["evidence"]["note"], expected);
}

#[test]
fn evidence_redacts_entropy_shaped_relative_paths() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let path = "src/cache/AbCdEf0123456789GhIjKlMnOpQrStUv.json";
    let added: SuccessEnvelope<AddData> = success(
        &command()
            .arg("--file")
            .arg(&file)
            .args([
                "add",
                "entropy path",
                "--agent",
                "tester",
                "--evidence",
                path,
            ])
            .output()
            .unwrap(),
    );
    assert_eq!(
        added.data.record.cut_evidence().unwrap().note.as_deref(),
        Some("<redacted>")
    );
}

#[test]
fn evidence_redacts_quoted_values_and_url_userinfo_in_stdout_and_jsonl() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let input = r#"api_key="tail-secret" endpoint=https://user:credential@host.test/path"#;
    let expected = r#"api_key="<redacted>" endpoint=https://<redacted>@host.test/path"#;
    let output = command()
        .arg("--file")
        .arg(&file)
        .args([
            "add",
            "quoted evidence",
            "--agent",
            "tester",
            "--evidence",
            input,
        ])
        .output()
        .unwrap();
    let added: SuccessEnvelope<AddData> = success(&output);
    assert_eq!(
        added.data.record.cut_evidence().unwrap().note.as_deref(),
        Some(expected)
    );
    let stdout: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(stdout["data"]["record"]["evidence"]["note"], expected);
    let stored: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(stored["evidence"]["note"], expected);
}

#[test]
fn add_help_describes_stderr_redaction_as_best_effort() {
    let output = command().args(["add", "--help"]).output().unwrap();
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(help.contains("best-effort redaction"));
}

#[cfg(unix)]
#[test]
fn stderr_file_requires_a_regular_file_and_follows_regular_file_symlinks() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let target = temp.path().join("stderr.txt");
    let link = temp.path().join("stderr-link.txt");
    std::fs::write(&target, "ordinary stderr").unwrap();
    symlink(&target, &link).unwrap();
    let added: SuccessEnvelope<AddData> = success(
        &command()
            .arg("--file")
            .arg(&file)
            .args(["add", "symlink evidence", "--stderr-file"])
            .arg(&link)
            .output()
            .unwrap(),
    );
    assert_eq!(
        added.data.record.cut_evidence().unwrap().stderr.as_deref(),
        Some("ordinary stderr")
    );

    let fifo = temp.path().join("stderr.fifo");
    let made_fifo = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .is_ok_and(|status| status.success());
    if made_fifo {
        let mut child = std::process::Command::new(assert_cmd::cargo::cargo_bin!("blotter"))
            .env("BLOTTER_NOW", NOW)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .arg("--file")
            .arg(&file)
            .args([
                "add",
                "fifo evidence",
                "--stderr-file",
                fifo.to_str().unwrap(),
            ])
            .spawn()
            .unwrap();
        // Generous guard: the O_NONBLOCK open cannot block on the FIFO, so this
        // only trips if a blocking open is reintroduced. One second flaked under
        // parallel-suite load (spawn latency alone can exceed it).
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if std::time::Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("FIFO evidence read blocked");
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        assert_eq!(status.code(), Some(65));
        let rejected = child.wait_with_output().unwrap();
        let envelope = error(&rejected, 65, "invalid_input");
        assert!(envelope.error.message.contains("not a regular file"));
        assert!(envelope.error.suggested_fix.contains("FIFOs and devices"));
    }
}

#[cfg(unix)]
#[test]
fn stderr_file_errors_are_structured_and_specific() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let invoke = |path: &Path| {
        run_file(
            &file,
            &[
                "add",
                "bad evidence",
                "--stderr-file",
                path.to_str().unwrap(),
            ],
        )
    };

    let oversized = temp.path().join("oversized.txt");
    std::fs::write(&oversized, vec![b'x'; 1024 * 1024 + 1]).unwrap();
    let oversized = error(&invoke(&oversized), 65, "invalid_input");
    assert!(
        oversized
            .error
            .message
            .contains("exceeds the 1048576-byte read limit")
    );
    assert!(
        oversized
            .error
            .suggested_fix
            .contains("smaller stderr file")
    );

    let invalid_utf8 = temp.path().join("invalid-utf8.txt");
    std::fs::write(&invalid_utf8, [0xff]).unwrap();
    let invalid_utf8 = error(&invoke(&invalid_utf8), 65, "invalid_input");
    assert!(invalid_utf8.error.message.contains("not valid UTF-8"));
    assert!(
        invalid_utf8
            .error
            .suggested_fix
            .contains("UTF-8 stderr file")
    );

    let directory = temp.path().join("stderr-directory");
    std::fs::create_dir(&directory).unwrap();
    let directory_error = error(&invoke(&directory), 65, "invalid_input");
    assert!(directory_error.error.message.contains("not a regular file"));
    assert!(
        directory_error
            .error
            .suggested_fix
            .contains("regular UTF-8 file")
    );

    let link = temp.path().join("stderr-directory-link");
    symlink(&directory, &link).unwrap();
    let link = error(&invoke(&link), 65, "invalid_input");
    assert!(link.error.message.contains("not a regular file"));
    assert!(link.error.suggested_fix.contains("FIFOs and devices"));

    let unreadable = temp.path().join("unreadable.txt");
    std::fs::write(&unreadable, "stderr").unwrap();
    std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o000)).unwrap();
    let output = invoke(&unreadable);
    std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o600)).unwrap();
    let unreadable = error(&output, 77, "permission_denied");
    assert!(
        unreadable
            .error
            .message
            .starts_with("permission denied reading stderr evidence file")
    );
    assert!(
        unreadable
            .error
            .suggested_fix
            .contains("Grant read permission")
    );
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
fn resolve_single_id_always_returns_a_records_array() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let added = add(&file, "resolve one");

    let resolved: Value =
        serde_json::from_slice(&run_file(&file, &["resolve", added.data.record.cut_id()]).stdout)
            .unwrap();
    let data = resolved["data"].as_object().unwrap();
    assert!(data.get("record").is_none());
    let records = data["records"].as_array().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["id"], added.data.record.cut_id());
}

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
    assert_eq!(schema.data["contract"], 5);
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
fn dogear_duplicate_tags_are_stored_as_the_deduped_identity_tag_set() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("duplicate-dogear-tags.jsonl");
    let added: SuccessEnvelope<Value> = success(
        &command()
            .arg("--file")
            .arg(&file)
            .args([
                "dogear",
                "same dogear",
                "--agent",
                "tester",
                "--tag",
                "b",
                "--tag",
                "a",
                "--tag",
                "a",
            ])
            .output()
            .unwrap(),
    );
    assert_eq!(added.data["record"]["tags"], json!(["a", "b"]));
    let stored: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
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
fn dogear_kind_add_alias_stdin_dry_run_and_tags() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let added: SuccessEnvelope<Value> = success(
        &command()
            .arg("--file")
            .arg(&file)
            .args([
                "dogear",
                "surprising measurement",
                "--agent",
                "researcher",
                "--tag",
                "zeta",
                "--tag",
                "alpha",
                "--evidence",
                "benchmark run 42",
            ])
            .output()
            .unwrap(),
    );
    let record = &added.data["record"];
    assert!(added.data["changed"].as_bool().unwrap());
    assert_eq!(record["kind"], "dogear");
    assert_eq!(record["agent"], "researcher");
    assert_eq!(record["tags"], json!(["alpha", "zeta"]));
    assert_eq!(record["evidence"], "benchmark run 42");
    assert!(record.get("severity").is_none());
    assert!(record.get("cmd").is_none());

    let alias: SuccessEnvelope<Value> = success(
        &command()
            .arg("--file")
            .arg(&file)
            .args(["idea", "-", "--agent", "researcher", "--tag", "stdin"])
            .write_stdin("empty prior-art niche\n")
            .output()
            .unwrap(),
    );
    assert_eq!(alias.data["record"]["kind"], "dogear");
    assert_eq!(alias.data["record"]["text"], "empty prior-art niche");

    let before = std::fs::read(&file).unwrap();
    let dry_run: SuccessEnvelope<Value> = success(&run_file(
        &file,
        &[
            "dogear",
            "reusable pattern",
            "--agent",
            "researcher",
            "--dry-run",
        ],
    ));
    assert!(!dry_run.data["changed"].as_bool().unwrap());
    assert_eq!(std::fs::read(&file).unwrap(), before);
}

#[test]
fn dogear_kind_list_resolve_doctor_schema_and_filter_contract() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let cut = add(&file, "ordinary friction");
    let dogear: SuccessEnvelope<Value> = success(&run_file(
        &file,
        &[
            "dogear",
            "a useful blog post dogear",
            "--agent",
            "researcher",
            "--tag",
            "writing",
        ],
    ));
    let dogear_id = dogear.data["record"]["id"].as_str().unwrap().to_owned();

    let default: SuccessEnvelope<Value> = success(&run_file(&file, &["list"]));
    assert_eq!(default.data["items"].as_array().unwrap().len(), 1);
    assert_eq!(default.data["items"][0]["kind"], "cut");
    assert_eq!(default.data["items"][0]["id"], cut.data.record.cut_id());

    let dogears: SuccessEnvelope<Value> = success(&run_file(&file, &["list", "--kind", "dogear"]));
    assert_eq!(dogears.data["items"].as_array().unwrap().len(), 1);
    assert_eq!(dogears.data["items"][0]["kind"], "dogear");
    assert_eq!(dogears.data["items"][0]["id"], dogear_id);

    let all: SuccessEnvelope<Value> = success(&run_file(&file, &["list", "--kind", "all"]));
    assert_eq!(
        all.data["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["kind"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["cut", "dogear"]
    );
    let markdown = run_file(&file, &["list", "--kind", "all", "--format", "md"]);
    assert!(markdown.status.success());
    let markdown = String::from_utf8(markdown.stdout).unwrap();
    assert!(markdown.contains("ordinary friction"));
    assert!(markdown.contains("## Dogears\n"));
    assert!(markdown.contains("a useful blog post dogear"));

    let resolved: SuccessEnvelope<Value> = success(&run_file(
        &file,
        &[
            "resolve", &dogear_id, "--agent", "writer", "--note", "assigned",
        ],
    ));
    assert!(resolved.data["changed"].as_bool().unwrap());
    assert_eq!(resolved.data["records"][0]["kind"], "dogear");
    assert_eq!(resolved.data["records"][0]["status"], "resolved");
    assert_eq!(
        resolved.data["records"][0]["resolution"]["note"],
        "assigned"
    );

    let doctor: SuccessEnvelope<DoctorData> = success(&run_file(&file, &["doctor"]));
    assert!(doctor.data.healthy);
    let schema: SuccessEnvelope<Value> = success(&run(&["schema"]));
    assert_eq!(schema.data["commands"]["dogear"]["alias"], json!(["idea"]));
    assert_eq!(schema.data["records"]["dogear"]["kind"], "dogear");
    assert!(
        schema.data["commands"]["list"]["flags"]["--kind"]
            .as_str()
            .unwrap()
            .contains("cut|dogear|all")
    );

    error(
        &run_file(&file, &["list", "--kind", "dogear", "--severity", "minor"]),
        2,
        "invalid_argument",
    );

    let malformed = temp.path().join("malformed-dogear.jsonl");
    std::fs::write(
        &malformed,
        "{\"kind\":\"dogear\",\"id\":\"bl_bad\",\"ts\":\"not-a-time\"}\n",
    )
    .unwrap();
    let malformed_doctor = run_file(&malformed, &["doctor"]);
    assert_eq!(malformed_doctor.status.code(), Some(1));
    let malformed: SuccessEnvelope<DoctorData> =
        serde_json::from_slice(&malformed_doctor.stdout).unwrap();
    assert!(
        malformed
            .data
            .findings
            .iter()
            .any(|finding| finding.kind == "malformed")
    );
}

#[test]
fn resolve_records_structured_graduation_fields() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let cut = add(&file, "graduate this fix");

    let resolved: SuccessEnvelope<Value> = success(&run_file(
        &file,
        &[
            "resolve",
            cut.data.record.cut_id(),
            "--task",
            "TASK-2",
            "--pr",
            "https://example.com/dogear/pull/2",
            "--commit",
            "d34db33fd34db33f",
            "--note",
            "graduated",
        ],
    ));
    let resolution = &resolved.data["records"][0]["resolution"];
    assert_eq!(resolution["note"], "graduated");
    assert_eq!(resolution["task"], "TASK-2");
    assert_eq!(resolution["pr"], "https://example.com/dogear/pull/2");
    assert_eq!(resolution["commit"], "d34db33fd34db33f");

    let listed: SuccessEnvelope<Value> =
        success(&run_file(&file, &["list", "--status", "resolved"]));
    assert_eq!(listed.data["items"][0]["resolution"], *resolution);

    for flag in ["--task", "--pr", "--commit"] {
        let output = command()
            .arg("--file")
            .arg(&file)
            .arg("resolve")
            .arg(cut.data.record.cut_id())
            .arg(flag)
            .arg(" \t")
            .output()
            .unwrap();
        error(&output, 65, "invalid_input");
    }

    let doctor: SuccessEnvelope<DoctorData> = success(&run_file(&file, &["doctor"]));
    assert!(doctor.data.healthy);
}

#[test]
fn resolve_records_dogear_publish_and_drop_lifecycle() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let publish: SuccessEnvelope<Value> = success(&run_file(
        &file,
        &["dogear", "publish this idea", "--agent", "researcher"],
    ));
    let publish_id = publish.data["record"]["id"].as_str().unwrap().to_owned();
    let published: SuccessEnvelope<Value> = success(&run_file(
        &file,
        &[
            "resolve",
            &publish_id,
            "--url",
            "https://example.com/posts/blotter",
        ],
    ));
    assert_eq!(
        published.data["records"][0]["resolution"]["url"],
        "https://example.com/posts/blotter"
    );

    let drop: SuccessEnvelope<Value> = success(&run_file(
        &file,
        &["dogear", "drop this idea", "--agent", "researcher"],
    ));
    let drop_id = drop.data["record"]["id"].as_str().unwrap().to_owned();
    let dropped: SuccessEnvelope<Value> =
        success(&run_file(&file, &["resolve", &drop_id, "--dropped"]));
    assert_eq!(dropped.data["records"][0]["resolution"]["dropped"], true);

    let doctor: SuccessEnvelope<DoctorData> = success(&run_file(&file, &["doctor"]));
    assert!(doctor.data.healthy);
}

#[test]
fn resolve_help_marks_dogear_only_lifecycle_flags() {
    let help = run(&["resolve", "--help"]);
    assert!(help.status.success());
    assert!(help.stderr.is_empty());
    let stdout = String::from_utf8_lossy(&help.stdout);
    assert!(stdout.contains("Published destination (dogear records only)"));
    assert!(stdout.contains("Mark dropped (dogear records only)"));
}

#[test]
fn resolve_dogear_lifecycle_flags_reject_cuts_without_partial_batches() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let cut = add(&file, "cut cannot publish");
    let before = std::fs::read(&file).unwrap();
    error(
        &run_file(
            &file,
            &[
                "resolve",
                cut.data.record.cut_id(),
                "--url",
                "https://example.com/posts/blotter",
            ],
        ),
        2,
        "invalid_argument",
    );
    assert_eq!(std::fs::read(&file).unwrap(), before);

    let dogear: SuccessEnvelope<Value> = success(&run_file(
        &file,
        &[
            "dogear",
            "dogear cannot be partially dropped",
            "--agent",
            "writer",
        ],
    ));
    let dogear_id = dogear.data["record"]["id"].as_str().unwrap().to_owned();
    let before = std::fs::read(&file).unwrap();
    error(
        &run_file(
            &file,
            &["resolve", cut.data.record.cut_id(), &dogear_id, "--dropped"],
        ),
        2,
        "invalid_argument",
    );
    assert_eq!(std::fs::read(&file).unwrap(), before);

    let listed: SuccessEnvelope<Value> = success(&run_file(
        &file,
        &["list", "--kind", "dogear", "--status", "all"],
    ));
    assert_eq!(listed.data["items"].as_array().unwrap().len(), 1);
    assert_eq!(listed.data["items"][0]["id"], dogear_id);
    assert_eq!(listed.data["items"][0]["status"], "open");

    let conflict = run_file(
        &file,
        &[
            "resolve",
            &dogear_id,
            "--url",
            "https://example.com/posts/blotter",
            "--dropped",
        ],
    );
    assert_eq!(conflict.status.code(), Some(2));
    assert!(conflict.stdout.is_empty());
    assert_eq!(std::fs::read(&file).unwrap(), before);
}

#[test]
fn resolve_without_new_flags_omits_graduation_and_lifecycle_keys() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let cut = add(&file, "ordinary resolve response");

    let resolved: SuccessEnvelope<Value> =
        success(&run_file(&file, &["resolve", cut.data.record.cut_id()]));
    let resolution = resolved.data["records"][0]["resolution"]
        .as_object()
        .unwrap();
    for key in ["task", "pr", "commit", "url", "dropped", "amended"] {
        assert!(!resolution.contains_key(key));
    }
    assert_eq!(
        resolved.data["records"][0]["resolution"],
        json!({"agent":"unknown","note":null,"ts":"2026-07-09T18:30:00.123Z"})
    );
    let event = std::fs::read_to_string(&file)
        .unwrap()
        .lines()
        .last()
        .unwrap()
        .to_owned();
    let event_json: Value = serde_json::from_str(&event).unwrap();
    let event_json = event_json.as_object().unwrap();
    for key in ["task", "pr", "commit", "url", "dropped", "amend"] {
        assert!(!event_json.contains_key(key));
    }
    assert_eq!(
        event,
        format!(
            "{{\"kind\":\"resolve\",\"id\":\"{}\",\"ts\":\"2026-07-09T18:30:00.123Z\",\"agent\":\"unknown\",\"note\":null}}",
            cut.data.record.cut_id()
        )
    );
}

#[test]
fn resolve_amend_replaces_base_resolution_and_preserves_history() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let cut = add(&file, "correct this resolution");
    let id = cut.data.record.cut_id().to_owned();
    let _: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &[
            "resolve", &id, "--agent", "base", "--note", "original", "--task", "TASK-12",
        ],
    ));
    let before_amend = std::fs::read(&file).unwrap();

    let amended: SuccessEnvelope<Value> = success(&run_file(
        &file,
        &[
            "resolve",
            &id,
            "--amend",
            "--agent",
            "corrector",
            "--note",
            "corrected",
        ],
    ));
    let resolution = &amended.data["records"][0]["resolution"];
    assert_eq!(resolution["agent"], "corrector");
    assert_eq!(resolution["note"], "corrected");
    assert_eq!(resolution["amended"], true);
    assert!(resolution.get("task").is_none());

    let after_amend = std::fs::read(&file).unwrap();
    assert!(after_amend.starts_with(&before_amend));
    let after_text = String::from_utf8(after_amend).unwrap();
    assert_eq!(after_text.lines().count(), 3);
    let lines: Vec<_> = after_text.lines().collect();
    assert!(lines[1].contains("\"note\":\"original\""));
    assert_eq!(
        serde_json::from_str::<Value>(lines[2]).unwrap()["amend"],
        true
    );

    let listed: SuccessEnvelope<Value> =
        success(&run_file(&file, &["list", "--status", "resolved"]));
    assert_eq!(listed.data["items"][0]["resolution"], *resolution);
}

#[test]
fn resolve_amend_latest_event_supersedes_prior_amend() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let cut = add(&file, "latest correction wins");
    let id = cut.data.record.cut_id().to_owned();
    let _: SuccessEnvelope<ResolveData> =
        success(&run_file(&file, &["resolve", &id, "--note", "base"]));
    let _: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &[
            "resolve",
            &id,
            "--amend",
            "--agent",
            "first",
            "--note",
            "first correction",
        ],
    ));
    let _: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &[
            "resolve",
            &id,
            "--amend",
            "--agent",
            "second",
            "--note",
            "second correction",
        ],
    ));

    let listed: SuccessEnvelope<ListData> =
        success(&run_file(&file, &["list", "--status", "resolved"]));
    let resolution = listed.data.items[0].resolution.as_ref().unwrap();
    assert_eq!(resolution.agent, "second");
    assert_eq!(resolution.note.as_deref(), Some("second correction"));
    assert!(resolution.amended);
    assert_eq!(std::fs::read_to_string(&file).unwrap().lines().count(), 4);
}

#[test]
fn resolve_amend_requires_a_resolved_record_and_changing_fields() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let cut = add(&file, "unresolved correction");
    let id = cut.data.record.cut_id().to_owned();
    let before = std::fs::read(&file).unwrap();

    let unresolved = error(
        &run_file(&file, &["resolve", &id, "--amend", "--note", "corrected"]),
        65,
        "invalid_input",
    );
    assert!(
        unresolved
            .error
            .suggested_fix
            .contains("Resolve each record without --amend first")
    );
    assert_eq!(std::fs::read(&file).unwrap(), before);

    error(
        &run_file(&file, &["resolve", &id, "--amend"]),
        65,
        "invalid_input",
    );
    assert_eq!(std::fs::read(&file).unwrap(), before);

    error(
        &run_file(
            &file,
            &["resolve", "deadbeef", "--amend", "--note", "corrected"],
        ),
        66,
        "not_found",
    );
    assert_eq!(std::fs::read(&file).unwrap(), before);
}

#[test]
fn resolve_amend_batches_require_every_record_to_be_resolved() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let first = add(&file, "first batch correction")
        .data
        .record
        .cut_id()
        .to_owned();
    let second = add(&file, "second batch correction")
        .data
        .record
        .cut_id()
        .to_owned();
    let _: SuccessEnvelope<ResolveData> =
        success(&run_file(&file, &["resolve", &first, "--note", "base"]));
    let before = std::fs::read(&file).unwrap();

    error(
        &run_file(
            &file,
            &[
                "resolve",
                &first,
                &second,
                "--amend",
                "--note",
                "batch correction",
            ],
        ),
        65,
        "invalid_input",
    );
    assert_eq!(std::fs::read(&file).unwrap(), before);

    let _: SuccessEnvelope<ResolveData> =
        success(&run_file(&file, &["resolve", &second, "--note", "base"]));
    let amended: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &[
            "resolve",
            &first,
            &second,
            "--amend",
            "--note",
            "batch correction",
        ],
    ));
    assert!(amended.data.changed);
    assert!(
        amended
            .data
            .records
            .iter()
            .all(
                |record| record.resolution.as_ref().is_some_and(|resolution| {
                    resolution.amended && resolution.note.as_deref() == Some("batch correction")
                })
            )
    );
}

#[test]
fn resolve_amend_supports_dogear_lifecycle_fields() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let published: SuccessEnvelope<Value> = success(&run_file(
        &file,
        &["dogear", "published correction", "--agent", "writer"],
    ));
    let published_id = published.data["record"]["id"].as_str().unwrap().to_owned();
    let _: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &[
            "resolve",
            &published_id,
            "--url",
            "https://example.com/original",
        ],
    ));
    let amended_url: SuccessEnvelope<Value> = success(&run_file(
        &file,
        &[
            "resolve",
            &published_id,
            "--amend",
            "--url",
            "https://example.com/corrected",
        ],
    ));
    assert_eq!(
        amended_url.data["records"][0]["resolution"]["url"],
        "https://example.com/corrected"
    );
    assert_eq!(
        amended_url.data["records"][0]["resolution"]["amended"],
        true
    );

    let dropped: SuccessEnvelope<Value> = success(&run_file(
        &file,
        &["dogear", "dropped correction", "--agent", "writer"],
    ));
    let dropped_id = dropped.data["record"]["id"].as_str().unwrap().to_owned();
    let _: SuccessEnvelope<ResolveData> =
        success(&run_file(&file, &["resolve", &dropped_id, "--dropped"]));
    let amended_dropped: SuccessEnvelope<Value> = success(&run_file(
        &file,
        &["resolve", &dropped_id, "--amend", "--dropped"],
    ));
    assert_eq!(
        amended_dropped.data["records"][0]["resolution"]["dropped"],
        true
    );
    assert_eq!(
        amended_dropped.data["records"][0]["resolution"]["amended"],
        true
    );

    let cut = add(&file, "cut cannot amend a URL");
    let cut_id = cut.data.record.cut_id().to_owned();
    let _: SuccessEnvelope<ResolveData> =
        success(&run_file(&file, &["resolve", &cut_id, "--note", "base"]));
    let before = std::fs::read(&file).unwrap();
    error(
        &run_file(
            &file,
            &[
                "resolve",
                &cut_id,
                "--amend",
                "--url",
                "https://example.com/cuts",
            ],
        ),
        2,
        "invalid_argument",
    );
    assert_eq!(std::fs::read(&file).unwrap(), before);
}

#[test]
fn resolve_amend_dry_run_materializes_without_appending() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let cut = add(&file, "preview correction");
    let id = cut.data.record.cut_id().to_owned();
    let _: SuccessEnvelope<ResolveData> =
        success(&run_file(&file, &["resolve", &id, "--note", "base"]));
    let before = std::fs::read(&file).unwrap();

    let preview: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &["resolve", &id, "--amend", "--note", "preview", "--dry-run"],
    ));
    assert!(!preview.data.changed);
    let resolution = preview.data.records[0].resolution.as_ref().unwrap();
    assert_eq!(resolution.note.as_deref(), Some("preview"));
    assert!(resolution.amended);
    assert_eq!(
        preview.meta.warnings,
        ["dry run; no resolve event appended"]
    );
    assert_eq!(std::fs::read(&file).unwrap(), before);
}

#[test]
fn amended_list_output_is_byte_deterministic_under_blotter_now() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let cut = add(&file, "deterministic correction");
    let id = cut.data.record.cut_id().to_owned();
    let _: SuccessEnvelope<ResolveData> =
        success(&run_file(&file, &["resolve", &id, "--note", "base"]));
    let _: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &["resolve", &id, "--amend", "--note", "corrected"],
    ));

    let first = run_file(&file, &["list", "--status", "resolved"]);
    let second = run_file(&file, &["list", "--status", "resolved"]);
    assert!(first.status.success());
    assert!(second.status.success());
    assert!(first.stderr.is_empty());
    assert!(second.stderr.is_empty());
    assert_eq!(first.stdout, second.stdout);
    let listed: SuccessEnvelope<Value> = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(listed.data["items"][0]["resolution"]["amended"], true);
}

#[test]
fn orphan_resolve_amends_warn_in_the_fold_but_not_doctor() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let cut = add(&file, "orphan amend fixture");
    let id = cut.data.record.cut_id().to_owned();
    let original = std::fs::read_to_string(&file).unwrap();
    let orphan_amend = json!({
        "kind": "resolve",
        "id": id,
        "ts": "2026-07-09T18:30:00.123Z",
        "agent": "fixture",
        "note": "orphan amend",
        "amend": true
    });
    std::fs::write(&file, format!("{original}{orphan_amend}\n")).unwrap();

    let listed: SuccessEnvelope<ListData> = success(&run_file(&file, &["list", "--status", "all"]));
    assert_eq!(listed.data.items[0].status, ItemStatus::Open);
    assert_eq!(listed.meta.warnings, ["skipped 1 orphan resolve"]);
    let doctor: SuccessEnvelope<DoctorData> = success(&run_file(&file, &["doctor"]));
    assert!(doctor.data.healthy);
}

#[test]
fn resolve_reports_materialized_orphan_amend_after_base_append() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let cut = add(&file, "activate stale amend");
    let id = cut.data.record.cut_id().to_owned();
    let original = std::fs::read_to_string(&file).unwrap();
    let orphan_amend = json!({
        "kind": "resolve",
        "id": id,
        "ts": "2026-07-09T18:31:00.000Z",
        "agent": "stale-amend",
        "note": "stale correction",
        "task": "TASK-STALE",
        "amend": true
    });
    std::fs::write(&file, format!("{original}{orphan_amend}\n")).unwrap();

    let resolved: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &[
            "resolve",
            &id,
            "--agent",
            "base",
            "--note",
            "base resolution",
        ],
    ));
    let listed: SuccessEnvelope<ListData> =
        success(&run_file(&file, &["list", "--status", "resolved"]));

    assert_eq!(resolved.data.records, listed.data.items);
    let resolution = resolved.data.records[0].resolution.as_ref().unwrap();
    assert_eq!(resolution.agent, "stale-amend");
    assert_eq!(resolution.note.as_deref(), Some("stale correction"));
    assert_eq!(resolution.task.as_deref(), Some("TASK-STALE"));
    assert!(resolution.amended);
}

#[test]
fn multiple_orphan_amends_for_one_record_count_once() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let cut = add(&file, "orphan amend count fixture");
    let id = cut.data.record.cut_id().to_owned();
    let original = std::fs::read_to_string(&file).unwrap();
    let first = json!({
        "kind": "resolve",
        "id": id,
        "ts": "2026-07-09T18:30:00.123Z",
        "agent": "fixture",
        "note": "first orphan amend",
        "amend": true
    });
    let second = json!({
        "kind": "resolve",
        "id": id,
        "ts": "2026-07-09T18:31:00.123Z",
        "agent": "fixture",
        "note": "second orphan amend",
        "amend": true
    });
    std::fs::write(&file, format!("{original}{first}\n{second}\n")).unwrap();

    let listed: SuccessEnvelope<ListData> = success(&run_file(&file, &["list", "--status", "all"]));
    assert_eq!(listed.meta.warnings, ["skipped 1 orphan resolve"]);

    // The base append activates the latest orphan amend, and resolve reports it
    // from the deciding fold without reading the log a second time.
    let resolved: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &["resolve", &id, "--agent", "base", "--note", "base"],
    ));
    let listed: SuccessEnvelope<ListData> =
        success(&run_file(&file, &["list", "--status", "resolved"]));
    assert_eq!(resolved.data.records, listed.data.items);
    let resolution = resolved.data.records[0].resolution.as_ref().unwrap();
    assert_eq!(resolution.note.as_deref(), Some("second orphan amend"));
    assert!(resolution.amended);
}

#[test]
fn doctor_reports_orphan_amend_for_unknown_record() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let orphan_amend = json!({
        "kind": "resolve",
        "id": "bl_deadbeef0000",
        "ts": "2026-07-09T18:30:00.123Z",
        "agent": "fixture",
        "note": "unknown record amend",
        "amend": true
    });
    std::fs::write(&file, format!("{orphan_amend}\n")).unwrap();

    let output = run_file(&file, &["doctor"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let doctor: SuccessEnvelope<DoctorData> = serde_json::from_slice(&output.stdout).unwrap();
    assert!(!doctor.data.healthy);
    assert_eq!(doctor.data.findings.len(), 1);
    assert_eq!(doctor.data.findings[0].kind, "orphan_resolve");
}

#[test]
fn doctor_accepts_amend_for_existing_resolved_record() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let cut = add(&file, "valid amend fixture");
    let id = cut.data.record.cut_id().to_owned();
    let _: SuccessEnvelope<ResolveData> = success(&run_file(&file, &["resolve", &id]));
    let _: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &["resolve", &id, "--amend", "--note", "corrected"],
    ));

    let doctor: SuccessEnvelope<DoctorData> = success(&run_file(&file, &["doctor"]));
    assert!(doctor.data.healthy);
}

#[test]
fn schema_documents_resolve_graduation_and_dogear_lifecycle() {
    let schema: SuccessEnvelope<Value> = success(&run(&["schema"]));
    let flags = schema.data["commands"]["resolve"]["flags"]
        .as_object()
        .unwrap();
    assert!(flags.contains_key("--task"));
    assert!(flags.contains_key("--pr"));
    assert!(flags.contains_key("--commit"));
    assert!(flags["--url"].as_str().unwrap().contains("dogear-only"));
    assert!(
        flags["--dropped"]
            .as_str()
            .unwrap()
            .contains("conflicts with --url")
    );
    assert!(schema.data["records"]["resolve"].get("task").is_some());
    assert!(schema.data["records"]["resolve"].get("url").is_some());
    assert!(
        schema.data["records"]["list_item"]["resolution"]
            .as_str()
            .unwrap()
            .contains("dropped")
    );
}

#[test]
fn schema_documents_resolve_amend_history() {
    let schema: SuccessEnvelope<Value> = success(&run(&["schema"]));
    let resolve = &schema.data["commands"]["resolve"];
    assert!(
        resolve["flags"]["--amend"]
            .as_str()
            .unwrap()
            .contains("already-resolved")
    );
    assert!(
        resolve["amend_fold"]
            .as_str()
            .unwrap()
            .contains("first non-amend resolve")
    );
    assert!(
        resolve["amend_fold"]
            .as_str()
            .unwrap()
            .contains("latest amend")
    );
    assert!(schema.data["records"]["resolve"].get("amend").is_some());
    assert!(
        schema.data["records"]["list_item"]["resolution"]
            .as_str()
            .unwrap()
            .contains("amended")
    );
}

#[test]
fn dogear_and_cut_ids_are_collision_safe_across_tag_boundaries_and_namespaces() {
    let ts = "2026-07-24T00:00:00.000Z";
    // Each tag is hashed as its own length-prefixed field, so a comma in a tag
    // can no longer forge a different tag set's id.
    let two_tags = compute_dogear_id(ts, "x", "t", &["a".into(), "b".into()]);
    let one_comma_tag = compute_dogear_id(ts, "x", "t", &["a,b".into()]);
    assert_ne!(two_tags, one_comma_tag);
    // Duplicate tags collapse rather than perturb the id.
    let deduped = compute_dogear_id(ts, "x", "t", &["a".into(), "a".into()]);
    let single = compute_dogear_id(ts, "x", "t", &["a".into()]);
    assert_eq!(deduped, single);

    let cut_two_tags = compute_id(ts, "x", "t", Severity::Minor, &["a".into(), "b".into()]);
    let cut_one_comma_tag = compute_id(ts, "x", "t", Severity::Minor, &["a,b".into()]);
    assert_ne!(cut_two_tags, cut_one_comma_tag);

    // Dogear ids are 80-bit (bl_ + 20 hex) and, being a different length from
    // the 48-bit cut id, can never collide with the cut namespace.
    assert_eq!(two_tags.len(), 3 + 20);
    assert_eq!(cut_two_tags.len(), 3 + 12);
    assert_ne!(two_tags, cut_two_tags);
}

#[test]
fn valid_final_record_without_newline_is_accepted_not_resurrected() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    // A complete, valid cut record with NO trailing newline: a crash after the
    // object bytes but before the newline. JSON Lines permits this.
    let id = compute_id(
        "2026-07-09T00:00:00.000Z",
        "a",
        "kept",
        Severity::Minor,
        &[],
    );
    let record = json!({
        "kind": "cut", "id": id, "ts": "2026-07-09T00:00:00.000Z", "agent": "a",
        "text": "kept", "tags": [], "severity": "minor", "cwd": "/tmp", "repo": null
    })
    .to_string();
    std::fs::write(&file, &record).unwrap();
    // The fold accepts it immediately (no "torn" ignore that a later append
    // would resurrect), and doctor agrees a valid tail is healthy.
    let listed: SuccessEnvelope<ListData> = success(&run_file(&file, &["list"]));
    assert_eq!(listed.data.items.len(), 1);
    assert_eq!(listed.data.items[0].text, "kept");
    let doctor: SuccessEnvelope<DoctorData> =
        serde_json::from_slice(&run_file(&file, &["doctor"]).stdout).unwrap();
    assert!(doctor.data.healthy, "findings: {:?}", doctor.data.findings);
    // Appending terminates the tail cleanly and both records survive.
    let added = add(&file, "second");
    assert!(added.data.changed);
    let bytes = std::fs::read(&file).unwrap();
    assert!(bytes.ends_with(b"\n"));
    let listed_again: SuccessEnvelope<ListData> = success(&run_file(&file, &["list"]));
    assert_eq!(listed_again.data.items.len(), 2);
}

#[test]
fn list_filters_sorts_limits_since_and_markdown() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let cases = [
        ("2026-07-01T00:00:00Z", "old blocker", "blocker", "ops"),
        ("2026-07-09T17:00:00Z", "new minor", "minor", "shell"),
        ("2026-07-09T18:00:00Z", "new major", "major", "ops"),
    ];
    for (now, text, severity, tag) in cases {
        let output = command()
            .env("BLOTTER_NOW", now)
            .arg("--file")
            .arg(&file)
            .args([
                "add",
                text,
                "--agent",
                "tester",
                "--severity",
                severity,
                "--tag",
                tag,
            ])
            .output()
            .unwrap();
        success::<AddData>(&output);
    }
    let limited: SuccessEnvelope<ListData> = success(&run_file(&file, &["list", "--limit", "1"]));
    assert_eq!(limited.data.items[0].text, "old blocker");
    assert_eq!(limited.data.total, 3);
    assert!(limited.data.truncated);

    let since: SuccessEnvelope<ListData> = success(
        &command()
            .env("BLOTTER_NOW", "2026-07-09T19:00:00Z")
            .arg("--file")
            .arg(&file)
            .args(["list", "--since", "2h", "--tag", "ops"])
            .output()
            .unwrap(),
    );
    assert_eq!(since.data.items.len(), 1);
    assert_eq!(since.data.items[0].text, "new major");

    let markdown = run_file(&file, &["list", "--format", "md", "--severity", "major"]);
    assert!(markdown.status.success());
    assert!(markdown.stderr.is_empty());
    let markdown = String::from_utf8(markdown.stdout).unwrap();
    assert!(markdown.starts_with("## Major\n"));
    assert!(markdown.contains("new major — tester"));
    assert!(serde_json::from_str::<Value>(&markdown).is_err());
    error(
        &run_file(&file, &["list", "--since", "2026-07-09"]),
        2,
        "invalid_argument",
    );
}

#[test]
fn list_limit_zero_does_not_emit_empty_result_warning() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    add(&file, "limited cut");
    dogear_at(&file, NOW, "limited dogear", &[]);

    for (kind, total) in [("cut", 1), ("dogear", 1), ("all", 2)] {
        let listed: SuccessEnvelope<ListData> =
            success(&run_file(&file, &["list", "--kind", kind, "--limit", "0"]));
        assert_eq!(listed.data.count, 0);
        assert_eq!(listed.data.total, total);
        assert!(listed.data.truncated);
        assert!(listed.meta.warnings.is_empty());
    }
}

#[test]
fn list_empty_results_emit_kind_specific_warnings() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    std::fs::write(&file, "").unwrap();

    for (kind, warning) in [
        (
            "cut",
            "no cuts matched; try --status all or broader filters",
        ),
        (
            "dogear",
            "no dogears matched; try --status all or broader filters",
        ),
        (
            "all",
            "no records matched; try --status all or broader filters",
        ),
    ] {
        let listed: SuccessEnvelope<ListData> =
            success(&run_file(&file, &["list", "--kind", kind]));
        assert_eq!(listed.data.count, 0);
        assert_eq!(listed.data.total, 0);
        assert!(!listed.data.truncated);
        assert_eq!(listed.meta.warnings, [warning]);
    }
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
fn list_markdown_collapses_multiline_text_into_one_bullet() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let added = add(&file, "  first line\nsecond\tline  third line  ");

    let output = run_file(&file, &["list", "--status", "open", "--format", "md"]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!(
            "## Minor\n- [{}] first line second line third line — tester, 2026-07-09T18:30:00.123Z\n",
            added.data.record.cut_id()
        )
    );
}

#[test]
fn list_markdown_renders_resolution_note_and_graduation_fields() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let added = add(&file, "the cut");
    let _: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &[
            "resolve",
            added.data.record.cut_id(),
            "--agent",
            "resolver",
            "--commit",
            "d34db33fd34db33f",
            "--pr",
            "https://github.com/BigCactusLabs/blotter/pull/25",
            "--task",
            "TASK-25",
            "--note",
            "fixed it",
        ],
    ));

    let output = run_file(&file, &["list", "--status", "resolved", "--format", "md"]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!(
            "## Minor\n- [~~{}~~] the cut — tester, 2026-07-09T18:30:00.123Z\n  - resolved 2026-07-09T18:30:00.123Z by resolver (d34db33fd34db33f) pr https://github.com/BigCactusLabs/blotter/pull/25 task TASK-25: fixed it\n",
            added.data.record.cut_id()
        )
    );
}

#[test]
fn list_markdown_collapses_multiline_resolution_metadata() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let added = add(&file, "the cut");
    let _: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &[
            "resolve",
            added.data.record.cut_id(),
            "--agent",
            "multi\nline resolver",
            "--commit",
            "d34db33f\n## heading",
            "--task",
            "TASK\n25",
        ],
    ));

    let output = run_file(&file, &["list", "--status", "resolved", "--format", "md"]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!(
            "## Minor\n- [~~{}~~] the cut — tester, 2026-07-09T18:30:00.123Z\n  - resolved 2026-07-09T18:30:00.123Z by multi line resolver (d34db33f ## heading) task TASK 25\n",
            added.data.record.cut_id()
        )
    );
}

#[test]
fn list_markdown_collapses_multiline_resolution_note() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let added = add(&file, "the cut");
    let note = "  first line\nsecond\tline  third line  ";
    let _: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &[
            "resolve",
            added.data.record.cut_id(),
            "--agent",
            "resolver",
            "--note",
            note,
        ],
    ));

    let listed: SuccessEnvelope<ListData> =
        success(&run_file(&file, &["list", "--status", "resolved"]));
    assert_eq!(
        listed.data.items[0]
            .resolution
            .as_ref()
            .unwrap()
            .note
            .as_deref(),
        Some(note)
    );

    let output = run_file(&file, &["list", "--status", "resolved", "--format", "md"]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!(
            "## Minor\n- [~~{}~~] the cut — tester, 2026-07-09T18:30:00.123Z\n  - resolved 2026-07-09T18:30:00.123Z by resolver: first line second line third line\n",
            added.data.record.cut_id()
        )
    );
}

#[test]
fn list_sorts_rfc3339_offsets_by_instant_not_text() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("offsets.jsonl");
    let earlier = json!({"kind":"cut","id":"bl_111111111111","ts":"2026-07-09T10:00:00+02:00","agent":"a","text":"earlier","tags":[],"severity":"minor","cwd":"/tmp","repo":null});
    let later = json!({"kind":"cut","id":"bl_222222222222","ts":"2026-07-09T09:00:00Z","agent":"a","text":"later","tags":[],"severity":"minor","cwd":"/tmp","repo":null});
    std::fs::write(&file, format!("{earlier}\n{later}\n")).unwrap();
    let listed: SuccessEnvelope<ListData> = success(&run_file(&file, &["list"]));
    assert_eq!(listed.data.items[0].text, "later");
}

#[test]
fn resolve_prefix_errors_and_idempotence_are_structured() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let added = add(&file, "resolve me");
    let id = added.data.record.cut_id();
    let prefix = &id[3..7];
    let first: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &["resolve", &prefix.to_ascii_uppercase(), "--agent", "fixer"],
    ));
    assert!(first.data.changed);
    let second: SuccessEnvelope<ResolveData> =
        success(&run_file(&file, &["resolve", id, "--agent", "fixer"]));
    assert!(!second.data.changed);
    assert_eq!(second.meta.warnings, ["already resolved"]);

    error(&run_file(&file, &["resolve", "abc"]), 2, "invalid_argument");
    let unknown = error(&run_file(&file, &["resolve", "deadbeef"]), 66, "not_found");
    assert_eq!(unknown.error.message, "no cut matches ID prefix 'deadbeef'");
    assert_eq!(
        unknown.error.suggested_fix,
        "Run `blotter list --status all --include-auto` and retry with a listed ID."
    );

    let missing = temp.path().join("missing.jsonl");
    let missing = error(
        &run_file(&missing, &["resolve", "deadbeef"]),
        66,
        "not_found",
    );
    assert_eq!(
        missing.error.message,
        format!(
            "blotter file not found: {}",
            temp.path().join("missing.jsonl").display()
        )
    );
    assert_eq!(
        missing.error.suggested_fix,
        "Run `blotter add` to create the file or pass an existing --file PATH."
    );

    let ambiguous = temp.path().join("ambiguous.jsonl");
    let lines = ["bl_abcd00000000", "bl_abcd11111111"]
        .map(|id| {
            json!({"kind":"cut","id":id,"ts":"2026-07-09T00:00:00.000Z","agent":"a","text":id,"tags":[],"severity":"minor","cwd":"/tmp","repo":null}).to_string()
        })
        .join("\n")
        + "\n";
    std::fs::write(&ambiguous, lines).unwrap();
    let envelope = error(
        &run_file(&ambiguous, &["resolve", "abcd"]),
        65,
        "ambiguous_id",
    );
    assert_eq!(
        envelope.error.details["candidates"],
        json!(["bl_abcd00000000", "bl_abcd11111111"])
    );
}

#[test]
fn multi_resolve_is_atomic_deterministic_and_idempotent() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let first = add(&file, "multi first").data.record.cut_id().to_owned();
    let second = add(&file, "multi second").data.record.cut_id().to_owned();
    let before = std::fs::read(&file).unwrap();

    let invalid = run_file(&file, &["resolve", &first, "deadbeef", "--agent", "fixer"]);
    error(&invalid, 66, "not_found");
    assert_eq!(std::fs::read(&file).unwrap(), before);

    let resolved: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &[
            "resolve", &second, &first, "--agent", "fixer", "--note", "batch",
        ],
    ));
    assert!(resolved.data.changed);
    assert_eq!(resolved.data.records.len(), 2);
    let mut expected = vec![first.clone(), second.clone()];
    expected.sort();
    assert_eq!(
        resolved
            .data
            .records
            .iter()
            .map(|record| record.id.clone())
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap().lines().count(), 4);

    let events: Vec<Value> = std::fs::read_to_string(&file)
        .unwrap()
        .lines()
        .skip(2)
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        events
            .iter()
            .map(|event| event["kind"].as_str())
            .collect::<Vec<_>>(),
        [Some("resolve"), Some("resolve")]
    );
    assert_eq!(
        events
            .iter()
            .map(|event| event["id"].as_str())
            .collect::<Vec<_>>(),
        expected
            .iter()
            .map(|id| Some(id.as_str()))
            .collect::<Vec<_>>()
    );
    for event in &events {
        assert_eq!(
            event
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["kind", "id", "ts", "agent", "note"]
        );
        assert_eq!(event["ts"], "2026-07-09T18:30:00.123Z");
        assert_eq!(event["agent"], "fixer");
        assert_eq!(event["note"], "batch");
    }
    let listed: SuccessEnvelope<ListData> =
        success(&run_file(&file, &["list", "--status", "resolved"]));
    assert_eq!(listed.data.items.len(), 2);
    assert!(listed.data.items.iter().all(|item| {
        item.resolution.as_ref().is_some_and(|resolution| {
            resolution.agent == "fixer" && resolution.note.as_deref() == Some("batch")
        })
    }));

    let duplicate: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &["resolve", &first, &first, "--agent", "fixer"],
    ));
    assert!(!duplicate.data.changed);
    assert_eq!(duplicate.data.records.len(), 1);
    assert_eq!(duplicate.meta.warnings, ["already resolved"]);
    assert_eq!(std::fs::read_to_string(&file).unwrap().lines().count(), 4);
}

#[test]
fn mixed_multi_resolve_warns_with_sorted_already_resolved_ids() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let first = add(&file, "mixed first").data.record.cut_id().to_owned();
    let second = add(&file, "mixed second").data.record.cut_id().to_owned();
    let _: SuccessEnvelope<ResolveData> =
        success(&run_file(&file, &["resolve", &first, "--agent", "fixer"]));

    let mixed: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &["resolve", &second, &first, "--agent", "fixer"],
    ));
    assert!(mixed.data.changed);
    assert_eq!(
        mixed.meta.warnings,
        [format!("already resolved: 1 ID ({first})")]
    );

    let all: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &["resolve", &first, &second, "--agent", "fixer"],
    ));
    assert!(!all.data.changed);
    assert_eq!(all.meta.warnings, ["already resolved"]);
}

#[test]
fn multi_resolve_with_ambiguous_prefix_is_atomic_and_returns_sorted_candidates() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let valid = add(&file, "valid multi resolve")
        .data
        .record
        .cut_id()
        .to_owned();
    let ambiguous = ["bl_abcd11111111", "bl_abcd00000000"]
        .map(|id| {
            json!({"kind":"cut","id":id,"ts":"2026-07-09T00:00:00.000Z","agent":"a","text":id,"tags":[],"severity":"minor","cwd":"/tmp","repo":null}).to_string()
        })
        .join("\n");
    let mut log = OpenOptions::new().append(true).open(&file).unwrap();
    writeln!(log, "{ambiguous}").unwrap();
    drop(log);
    let before = std::fs::read(&file).unwrap();

    let envelope = error(
        &run_file(&file, &["resolve", &valid, "abcd"]),
        65,
        "ambiguous_id",
    );
    assert_eq!(
        envelope.error.details["candidates"],
        json!(["bl_abcd00000000", "bl_abcd11111111"])
    );
    assert_eq!(std::fs::read(&file).unwrap(), before);
}

#[test]
fn multi_resolve_heals_a_torn_tail_and_keeps_first_resolution() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let first = add(&file, "first torn batch")
        .data
        .record
        .cut_id()
        .to_owned();
    let second = add(&file, "second torn batch")
        .data
        .record
        .cut_id()
        .to_owned();
    let mut torn = OpenOptions::new().append(true).open(&file).unwrap();
    write!(torn, "{{\"kind\":").unwrap();
    drop(torn);
    let _: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &[
            "resolve", &second, &first, "--agent", "fixer", "--note", "first",
        ],
    ));
    let log = std::fs::read_to_string(&file).unwrap();
    assert!(log.ends_with('\n'));
    let listed: SuccessEnvelope<ListData> =
        success(&run_file(&file, &["list", "--status", "resolved"]));
    assert_eq!(listed.data.items.len(), 2);
    assert!(listed.data.items.iter().all(|item| {
        item.resolution
            .as_ref()
            .is_some_and(|resolution| resolution.note.as_deref() == Some("first"))
    }));
    let first_resolution = json!({"kind":"resolve","id":first,"ts":"2026-07-09T18:30:00.123Z","agent":"later","note":"later"});
    std::fs::write(&file, format!("{log}{first_resolution}\n")).unwrap();
    let listed: SuccessEnvelope<ListData> =
        success(&run_file(&file, &["list", "--status", "resolved"]));
    let first_item = listed
        .data
        .items
        .iter()
        .find(|item| item.id == first)
        .unwrap();
    assert_eq!(
        first_item.resolution.as_ref().unwrap().note.as_deref(),
        Some("first")
    );
}

#[test]
fn concurrent_multi_resolves_share_one_critical_section() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let first = add(&file, "concurrent multi first")
        .data
        .record
        .cut_id()
        .to_owned();
    let second = add(&file, "concurrent multi second")
        .data
        .record
        .cut_id()
        .to_owned();
    let barrier = Arc::new(Barrier::new(4));
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let file = file.clone();
            let first = first.clone();
            let second = second.clone();
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let output = run_file(&file, &["resolve", &first, &second, "--agent", "race"]);
                let envelope: SuccessEnvelope<ResolveData> = success(&output);
                envelope.data.changed
            })
        })
        .collect();
    let changed = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .filter(|changed| *changed)
        .count();
    assert_eq!(changed, 1);
    assert_eq!(std::fs::read_to_string(&file).unwrap().lines().count(), 4);
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
    assert_eq!(schema.data["contract"], 5);
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

#[cfg(unix)]
#[test]
fn permission_denied_is_exit_77() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    std::fs::write(&file, "{}\n").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o000)).unwrap();
    let output = run_file(&file, &["list"]);
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    error(&output, 77, "permission_denied");
}

#[test]
fn lock_timeout_is_retryable_exit_75() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    add(&file, "locked");
    let locked = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&file)
        .unwrap();
    locked.lock().unwrap();
    let output = run_file(&file, &["list"]);
    locked.unlock().unwrap();
    let envelope = error(&output, 75, "lock_timeout");
    assert!(envelope.error.retryable);
}

#[test]
fn doctor_reports_all_core_findings_and_recomputed_ids() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let good = add(&file, "valid").data.record;
    let good_line = std::fs::read_to_string(&file).unwrap();
    let bad_id = json!({"kind":"cut","id":"bl_000000000000","ts":good.cut_ts(),"agent":"tester","text":"bad","tags":[],"severity":"minor","cwd":"/tmp","repo":null});
    let mut writer = OpenOptions::new().append(true).open(&file).unwrap();
    writeln!(writer, "{good_line}{}", bad_id).unwrap();
    writeln!(writer, "{{\"kind\":\"future\"}}").unwrap();
    writeln!(writer, "{{\"kind\":\"resolve\",\"id\":\"bl_deadbeef0000\",\"ts\":\"2026-07-09T00:00:00.000Z\",\"agent\":\"a\",\"note\":null}}").unwrap();
    writeln!(writer, "<<<<<<< HEAD").unwrap();
    write!(writer, "{{\"kind\":").unwrap();
    drop(writer);
    let output = run_file(&file, &["doctor"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let envelope: SuccessEnvelope<DoctorData> = serde_json::from_slice(&output.stdout).unwrap();
    let kinds: Vec<_> = envelope
        .data
        .findings
        .iter()
        .map(|finding| finding.kind.as_str())
        .collect();
    for kind in [
        "duplicate_cut",
        "id_conflict",
        "unknown_kind",
        "orphan_resolve",
        "conflict_marker",
        "torn_line",
    ] {
        assert!(kinds.contains(&kind), "missing {kind}: {kinds:?}");
    }
    assert!(!envelope.data.healthy);
}

#[test]
fn doctor_fix_dry_run_reports_quarantine_plan_without_writing() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    add(&file, "valid");
    let mut writer = OpenOptions::new().append(true).open(&file).unwrap();
    writer.write_all(b"not-json\n<<<<<<< HEAD\n").unwrap();
    drop(writer);
    let before = std::fs::read(&file).unwrap();

    let doctor = doctor_response(&run_file(&file, &["doctor", "--fix", "--dry-run"]), 1);
    let fix = doctor.data.fix.as_ref().unwrap();
    assert!(!fix.changed);
    assert!(fix.dry_run);
    assert!(fix.backup.is_none());
    assert!(fix.quarantine.is_none());
    assert!(fix.restore_hint.is_none());
    assert_eq!(
        fix.applied
            .iter()
            .map(|applied| (applied.line, applied.kind.as_str(), applied.action.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (2, "malformed", "quarantined"),
            (3, "conflict_marker", "quarantined"),
        ]
    );
    assert!(doctor.data.findings.iter().all(|finding| finding.fixable));
    assert_eq!(std::fs::read(&file).unwrap(), before);
    assert!(!std::path::PathBuf::from(format!("{}.quarantine.jsonl", file.display())).exists());
}

#[test]
fn doctor_fix_quarantines_torn_fragment_and_preserves_exact_backup() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    add(&file, "valid");
    #[cfg(unix)]
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    let complete = std::fs::read(&file).unwrap();
    let mut writer = OpenOptions::new().append(true).open(&file).unwrap();
    writer.write_all(b"{\"kind\":").unwrap();
    drop(writer);
    let original = std::fs::read(&file).unwrap();

    let doctor = doctor_response(&run_file(&file, &["doctor", "--fix"]), 0);
    let fix = doctor.data.fix.as_ref().unwrap();
    assert!(doctor.data.healthy, "findings: {:?}", doctor.data.findings);
    assert!(fix.changed);
    assert!(!fix.dry_run);
    assert_eq!(fix.applied.len(), 1);
    assert_eq!(fix.applied[0].line, 2);
    assert_eq!(fix.applied[0].kind, "torn_line");
    assert_eq!(fix.applied[0].action, "quarantined");
    let backup = std::path::PathBuf::from(fix.backup.as_ref().unwrap());
    let quarantine = std::path::PathBuf::from(fix.quarantine.as_ref().unwrap());
    assert_eq!(
        backup,
        std::path::PathBuf::from(format!("{}.bak-20260709T183000123Z", file.display()))
    );
    assert_eq!(std::fs::read(&backup).unwrap(), original);
    assert_eq!(std::fs::read(&quarantine).unwrap(), b"{\"kind\":\n");
    assert_eq!(std::fs::read(&file).unwrap(), complete);
    #[cfg(unix)]
    for output in [&backup, &quarantine, &file] {
        assert_eq!(permissions_mode(output), 0o600, "{}", output.display());
    }
    let expected_restore = format!("cp '{}' '{}'", backup.display(), file.display());
    assert_eq!(fix.restore_hint.as_deref(), Some(expected_restore.as_str()));
}

#[test]
fn doctor_fix_quarantines_malformed_and_conflict_marker_lines() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    add(&file, "valid");
    let complete = std::fs::read(&file).unwrap();
    let mut writer = OpenOptions::new().append(true).open(&file).unwrap();
    writer.write_all(b"not-json\n<<<<<<< HEAD\n").unwrap();
    drop(writer);
    let original = std::fs::read(&file).unwrap();

    let doctor = doctor_response(&run_file(&file, &["doctor", "--fix"]), 0);
    let fix = doctor.data.fix.as_ref().unwrap();
    assert!(doctor.data.healthy, "findings: {:?}", doctor.data.findings);
    assert_eq!(
        fix.applied
            .iter()
            .map(|applied| (applied.line, applied.kind.as_str(), applied.action.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (2, "malformed", "quarantined"),
            (3, "conflict_marker", "quarantined"),
        ]
    );
    let backup = std::path::PathBuf::from(fix.backup.as_ref().unwrap());
    let quarantine = std::path::PathBuf::from(fix.quarantine.as_ref().unwrap());
    assert_eq!(std::fs::read(&backup).unwrap(), original);
    assert_eq!(
        std::fs::read(&quarantine).unwrap(),
        b"not-json\n<<<<<<< HEAD\n"
    );
    assert_eq!(std::fs::read(&file).unwrap(), complete);
    assert!(
        doctor_response(&run_file(&file, &["doctor"]), 0)
            .data
            .healthy
    );
}

#[test]
fn doctor_fix_leaves_diagnose_only_findings_unchanged() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let good = add(&file, "valid").data.record;
    let good_line = std::fs::read_to_string(&file).unwrap();
    let bad_id = json!({
        "kind": "cut",
        "id": "bl_000000000000",
        "ts": good.cut_ts(),
        "agent": "tester",
        "text": "bad",
        "tags": [],
        "severity": "minor",
        "cwd": "/tmp",
        "repo": null
    });
    let mut writer = OpenOptions::new().append(true).open(&file).unwrap();
    writer.write_all(good_line.as_bytes()).unwrap();
    writeln!(writer, "{{\"kind\":\"future\"}}").unwrap();
    writeln!(writer, "{{\"kind\":\"resolve\",\"id\":\"bl_deadbeef0000\",\"ts\":\"2026-07-09T00:00:00.000Z\",\"agent\":\"a\",\"note\":null}}").unwrap();
    writeln!(writer, "{bad_id}").unwrap();
    drop(writer);
    let before = std::fs::read(&file).unwrap();

    let doctor = doctor_response(&run_file(&file, &["doctor", "--fix"]), 1);
    let fix = doctor.data.fix.as_ref().unwrap();
    assert!(!fix.changed);
    assert!(!fix.dry_run);
    assert!(fix.applied.is_empty());
    assert!(fix.backup.is_none());
    assert!(fix.quarantine.is_none());
    for kind in [
        "unknown_kind",
        "orphan_resolve",
        "duplicate_cut",
        "id_conflict",
    ] {
        assert!(
            doctor
                .data
                .findings
                .iter()
                .any(|finding| finding.kind == kind),
            "missing {kind}: {:?}",
            doctor.data.findings
        );
    }
    assert!(doctor.data.findings.iter().all(|finding| !finding.fixable));
    assert_eq!(std::fs::read(&file).unwrap(), before);
}

#[test]
fn doctor_fix_rejects_a_backup_collision_without_changing_the_log() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let original = b"not-json\n";
    std::fs::write(&file, original).unwrap();
    let backup = std::path::PathBuf::from(format!("{}.bak-20260709T183000123Z", file.display()));
    std::fs::write(&backup, b"existing backup").unwrap();

    error(&run_file(&file, &["doctor", "--fix"]), 74, "io_error");
    assert_eq!(std::fs::read(&file).unwrap(), original);
    assert_eq!(std::fs::read(&backup).unwrap(), b"existing backup");
    assert!(!std::path::PathBuf::from(format!("{}.quarantine.jsonl", file.display())).exists());
}

#[test]
fn doctor_fix_is_deterministic_for_repeated_identical_input() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let original = b"not-json\n";
    std::fs::write(&file, original).unwrap();

    let first = run_file(&file, &["doctor", "--fix"]);
    let first_data = doctor_response(&first, 0);
    let first_fix = first_data.data.fix.as_ref().unwrap();
    let first_repaired = std::fs::read(&file).unwrap();
    let first_backup = std::path::PathBuf::from(first_fix.backup.as_ref().unwrap());
    let first_quarantine = std::path::PathBuf::from(first_fix.quarantine.as_ref().unwrap());
    let first_backup_name = first_backup.file_name().unwrap().to_owned();
    let first_backup_bytes = std::fs::read(&first_backup).unwrap();

    std::fs::write(&file, original).unwrap();
    std::fs::remove_file(&first_backup).unwrap();
    std::fs::remove_file(&first_quarantine).unwrap();

    let second = run_file(&file, &["doctor", "--fix"]);
    let second_data = doctor_response(&second, 0);
    let second_fix = second_data.data.fix.as_ref().unwrap();
    let second_backup = std::path::PathBuf::from(second_fix.backup.as_ref().unwrap());
    assert_eq!(first.stdout, second.stdout);
    assert_eq!(std::fs::read(&file).unwrap(), first_repaired);
    assert_eq!(std::fs::read(&second_backup).unwrap(), first_backup_bytes);
    assert_eq!(
        second_backup.file_name().unwrap(),
        first_backup_name.as_os_str()
    );
}

#[test]
fn doctor_fix_times_out_under_an_exclusive_lock() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    add(&file, "locked");
    let locked = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&file)
        .unwrap();
    locked.lock().unwrap();
    let output = run_file(&file, &["doctor", "--fix"]);
    locked.unlock().unwrap();
    let envelope = error(&output, 75, "lock_timeout");
    assert!(envelope.error.retryable);
}

#[test]
fn doctor_dry_run_requires_fix() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    error(
        &run_file(&file, &["doctor", "--dry-run"]),
        2,
        "invalid_argument",
    );
}

#[test]
fn doctor_deny_requires_leaks() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let envelope = error(
        &run_file(&file, &["doctor", "--deny", "credential"]),
        2,
        "invalid_argument",
    );
    assert!(envelope.error.message.contains("--leaks"));
}

#[test]
fn doctor_rejects_empty_deny_literal() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let envelope = error(
        &run_file(&file, &["doctor", "--leaks", "--deny", ""]),
        2,
        "invalid_argument",
    );
    assert!(envelope.error.message.contains("empty"));
    assert!(envelope.error.suggested_fix.contains("non-empty"));
}

// --- archive retention ---

#[test]
fn archive_invalid_before_names_archive_flag() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");

    let envelope = error(
        &run_file(&file, &["archive", "--before", "garbage"]),
        2,
        "invalid_argument",
    );
    assert_eq!(envelope.error.message, "invalid --before value 'garbage'");
    assert!(envelope.error.suggested_fix.contains("--before"));
    assert!(!envelope.error.message.contains("--since"));
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

fn archive_jsonl(value: Value) -> Vec<u8> {
    let mut line = serde_json::to_vec(&value).unwrap();
    line.push(b'\n');
    line
}

fn archive_cut(ts: &str, text: &str) -> (String, Vec<u8>) {
    let id = compute_id(ts, "archive", text, Severity::Minor, &[]);
    let line = archive_jsonl(json!({
        "kind": "cut",
        "id": id,
        "ts": ts,
        "agent": "archive",
        "text": text,
        "tags": [],
        "severity": "minor",
        "cwd": "/tmp"
    }));
    (id, line)
}

fn archive_dogear(ts: &str, text: &str) -> (String, Vec<u8>) {
    let id = compute_dogear_id(ts, "archive", text, &[]);
    let line = archive_jsonl(json!({
        "kind": "dogear",
        "id": id,
        "ts": ts,
        "agent": "archive",
        "text": text,
        "tags": [],
        "cwd": "/tmp"
    }));
    (id, line)
}

fn archive_resolution(id: &str, ts: &str, dropped: bool, amend: bool) -> Vec<u8> {
    archive_jsonl(json!({
        "kind": "resolve",
        "id": id,
        "ts": ts,
        "agent": "archive",
        "note": null,
        "dropped": dropped,
        "amend": amend
    }))
}

fn physical_line_multiset(bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut lines = bytes
        .split_inclusive(|byte| *byte == b'\n')
        .map(Vec::from)
        .collect::<Vec<_>>();
    lines.sort();
    lines
}

#[test]
fn archive_removes_only_closed_wholly_old_current_groups() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let cutoff = "2026-08-01T00:00:00Z";

    let (old_cut_id, old_cut) = archive_cut("2026-07-01T00:00:00Z", "old resolved cut");
    let old_resolve = archive_resolution(&old_cut_id, "2026-07-02T00:00:00Z", false, false);

    let (late_resolve_id, late_resolve_cut) =
        archive_cut("2026-07-01T00:00:00Z", "old cut with late resolve");
    let late_resolve = archive_resolution(&late_resolve_id, "2026-08-02T00:00:00Z", false, false);

    let (late_amend_id, late_amend_cut) =
        archive_cut("2026-07-01T00:00:00Z", "old cut with late amend");
    let late_amend_resolve =
        archive_resolution(&late_amend_id, "2026-07-02T00:00:00Z", false, false);
    let late_amend = archive_resolution(&late_amend_id, "2026-08-02T00:00:00Z", false, true);

    let (_, old_open_cut) = archive_cut("2026-07-01T00:00:00Z", "old open cut");

    let (old_dogear_id, old_dogear) = archive_dogear("2026-07-01T00:00:00Z", "old resolved dogear");
    let old_drop = archive_resolution(&old_dogear_id, "2026-07-02T00:00:00Z", false, false);

    let (_, cutoff_cut) = archive_cut("2026-07-01T00:00:00Z", "cutoff is exclusive");
    let cutoff_id = compute_id(
        "2026-07-01T00:00:00Z",
        "archive",
        "cutoff is exclusive",
        Severity::Minor,
        &[],
    );
    let cutoff_resolve = archive_resolution(&cutoff_id, cutoff, false, false);

    let orphan = archive_resolution("bl_deadbeef0000", "2026-07-01T00:00:00Z", false, false);
    let malformed = b"not json\n".to_vec();
    let unknown = archive_jsonl(json!({"kind":"future","ts":"2026-07-01T00:00:00Z"}));
    let legacy = archive_jsonl(json!({
        "kind": "cut",
        "id": "pc_a1b2c3d4e5f6",
        "ts": "2026-07-01T00:00:00Z",
        "agent": "legacy",
        "text": "legacy closed cut",
        "tags": [],
        "severity": "minor",
        "cwd": "/tmp"
    }));
    let legacy_resolve =
        archive_resolution("pc_a1b2c3d4e5f6", "2026-07-02T00:00:00Z", false, false);

    let lines = vec![
        old_cut.clone(),
        old_resolve.clone(),
        late_resolve_cut,
        late_resolve,
        late_amend_cut,
        late_amend_resolve,
        late_amend,
        old_open_cut,
        old_dogear.clone(),
        old_drop.clone(),
        cutoff_cut,
        cutoff_resolve,
        orphan,
        malformed,
        unknown,
        legacy,
        legacy_resolve,
    ];
    let original = lines.concat();
    std::fs::write(&file, &original).unwrap();

    let archive: SuccessEnvelope<Value> =
        success(&run_file(&file, &["archive", "--before", cutoff]));
    assert_eq!(archive.data["changed"], true);
    assert_eq!(archive.data["archived"], 4);
    assert_eq!(archive.data["kept"], 13);

    let backup = format!("{}.bak-20260709T183000123Z", file.display());
    let archive_file = format!("{}.archive-20260709T183000123Z.jsonl", file.display());
    assert_eq!(archive.data["backup"], Value::String(backup.clone()));
    assert_eq!(
        archive.data["archive_file"],
        Value::String(archive_file.clone())
    );
    assert_eq!(
        archive.data["restore_hint"],
        Value::String(format!("cp '{backup}' '{}'", file.display()))
    );
    assert_eq!(std::fs::read(&backup).unwrap(), original);
    assert_eq!(
        std::fs::read(&archive_file).unwrap(),
        [old_cut, old_resolve, old_dogear, old_drop].concat()
    );

    let removed = [0usize, 1, 8, 9];
    let kept = lines
        .iter()
        .enumerate()
        .filter(|(index, _)| !removed.contains(index))
        .flat_map(|(_, line)| line.iter().copied())
        .collect::<Vec<_>>();
    assert_eq!(std::fs::read(&file).unwrap(), kept);
    for warning in [
        "skipped 1 malformed line",
        "skipped 1 unknown event",
        "skipped 1 orphan resolve",
    ] {
        assert!(archive.meta.warnings.contains(&warning.into()));
    }
}

#[test]
fn archive_copy_and_swap_preserves_terminated_line_bytes_and_is_deterministic() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let (old_id, old_cut) = archive_cut("2026-07-01T00:00:00Z", "archive this");
    let old_resolve = archive_resolution(&old_id, "2026-07-02T00:00:00Z", false, false);
    let (_, kept_open) = archive_cut("2026-07-03T00:00:00Z", "keep this open");
    let lines = [kept_open, old_cut, b"not json\n".to_vec(), old_resolve];
    let original = lines.concat();
    std::fs::write(&file, &original).unwrap();
    #[cfg(unix)]
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();

    let first = run_file(&file, &["archive", "--before", "2026-08-01T00:00:00Z"]);
    let first_data: SuccessEnvelope<Value> = success(&first);
    let first_backup = std::path::PathBuf::from(first_data.data["backup"].as_str().unwrap());
    let first_archive = std::path::PathBuf::from(first_data.data["archive_file"].as_str().unwrap());
    let first_kept = std::fs::read(&file).unwrap();
    let first_sidecar = std::fs::read(&first_archive).unwrap();
    assert_eq!(std::fs::read(&first_backup).unwrap(), original);
    #[cfg(unix)]
    for output in [&first_backup, &first_archive, &file] {
        assert_eq!(permissions_mode(output), 0o600, "{}", output.display());
    }
    assert_eq!(
        physical_line_multiset(&original),
        physical_line_multiset(&[first_kept.as_slice(), first_sidecar.as_slice()].concat())
    );

    std::fs::write(&file, &original).unwrap();
    std::fs::remove_file(&first_backup).unwrap();
    std::fs::remove_file(&first_archive).unwrap();

    let second = run_file(&file, &["archive", "--before", "2026-08-01T00:00:00Z"]);
    let second_data: SuccessEnvelope<Value> = success(&second);
    assert_eq!(first.stdout, second.stdout);
    assert_eq!(std::fs::read(&file).unwrap(), first_kept);
    assert_eq!(
        std::fs::read(second_data.data["archive_file"].as_str().unwrap()).unwrap(),
        first_sidecar
    );
}

#[test]
fn archive_dry_run_reports_the_plan_without_writing() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let (id, cut) = archive_cut("2026-07-01T00:00:00Z", "dry run");
    let resolve = archive_resolution(&id, "2026-07-02T00:00:00Z", false, false);
    let original = [cut, resolve].concat();
    std::fs::write(&file, &original).unwrap();

    let archive: SuccessEnvelope<Value> = success(&run_file(
        &file,
        &["archive", "--before", "2026-08-01T00:00:00Z", "--dry-run"],
    ));
    assert_eq!(archive.data["changed"], false);
    assert_eq!(archive.data["archived"], 2);
    assert_eq!(archive.data["kept"], 0);
    assert_eq!(archive.data["archive_file"], Value::Null);
    assert_eq!(archive.data["backup"], Value::Null);
    assert_eq!(archive.data["restore_hint"], Value::Null);
    assert_eq!(std::fs::read(&file).unwrap(), original);
    assert!(
        !std::path::PathBuf::from(format!("{}.bak-20260709T183000123Z", file.display())).exists()
    );
    assert!(
        !std::path::PathBuf::from(format!(
            "{}.archive-20260709T183000123Z.jsonl",
            file.display()
        ))
        .exists()
    );
}

#[test]
fn archive_with_no_eligible_lines_leaves_no_backup_or_sidecar() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let (_, open) = archive_cut("2026-07-01T00:00:00Z", "old but open");
    let original = [open, b"not json\n".to_vec()].concat();
    std::fs::write(&file, &original).unwrap();

    let archive: SuccessEnvelope<Value> = success(&run_file(
        &file,
        &["archive", "--before", "2026-08-01T00:00:00Z"],
    ));
    assert_eq!(archive.data["changed"], false);
    assert_eq!(archive.data["archived"], 0);
    assert_eq!(archive.data["kept"], 2);
    assert_eq!(archive.data["archive_file"], Value::Null);
    assert_eq!(archive.data["backup"], Value::Null);
    assert_eq!(std::fs::read(&file).unwrap(), original);
    assert!(
        !std::path::PathBuf::from(format!("{}.bak-20260709T183000123Z", file.display())).exists()
    );
    assert!(
        !std::path::PathBuf::from(format!(
            "{}.archive-20260709T183000123Z.jsonl",
            file.display()
        ))
        .exists()
    );
}

#[test]
fn archive_keeps_duplicate_group_when_a_duplicate_is_post_cutoff() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let (id, cut) = archive_cut("2026-07-01T00:00:00Z", "duplicate blocks archive");
    let resolve = archive_resolution(&id, "2026-07-02T00:00:00Z", false, false);
    let post_cutoff_duplicate = archive_jsonl(json!({
        "kind": "cut",
        "id": id,
        "ts": "2026-08-02T00:00:00Z",
        "agent": "archive",
        "text": "duplicate blocks archive",
        "tags": [],
        "severity": "minor",
        "cwd": "/tmp"
    }));
    let original = [cut, resolve, post_cutoff_duplicate].concat();
    std::fs::write(&file, &original).unwrap();

    let archive: SuccessEnvelope<Value> = success(&run_file(
        &file,
        &["archive", "--before", "2026-08-01T00:00:00Z"],
    ));
    assert_eq!(archive.data["changed"], false);
    assert_eq!(archive.data["archived"], 0);
    assert_eq!(std::fs::read(&file).unwrap(), original);
    assert!(
        !std::path::PathBuf::from(format!("{}.bak-20260709T183000123Z", file.display())).exists()
    );
}

#[test]
fn archive_keeps_ineligible_unterminated_final_fragment_byte_exact() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let original = b"{\"kind\":";
    std::fs::write(&file, original).unwrap();

    let archive: SuccessEnvelope<Value> = success(&run_file(
        &file,
        &["archive", "--before", "2026-08-01T00:00:00Z"],
    ));
    assert_eq!(archive.data["changed"], false);
    assert_eq!(archive.data["archived"], 0);
    assert_eq!(archive.data["kept"], 1);
    assert_eq!(std::fs::read(&file).unwrap(), original);
    assert!(
        !std::path::PathBuf::from(format!(
            "{}.archive-20260709T183000123Z.jsonl",
            file.display()
        ))
        .exists()
    );
}

#[test]
fn archive_newline_terminates_an_archivable_final_line_in_the_sidecar() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let (_, kept) = archive_cut("2026-08-02T00:00:00Z", "keep open");
    let (id, cut) = archive_cut("2026-07-01T00:00:00Z", "unterminated resolved");
    let mut resolve = archive_resolution(&id, "2026-07-02T00:00:00Z", false, false);
    assert_eq!(resolve.pop(), Some(b'\n'));
    let original = [kept.clone(), cut.clone(), resolve.clone()].concat();
    std::fs::write(&file, &original).unwrap();

    let archive: SuccessEnvelope<Value> = success(&run_file(
        &file,
        &["archive", "--before", "2026-08-01T00:00:00Z"],
    ));
    let sidecar = std::path::PathBuf::from(archive.data["archive_file"].as_str().unwrap());
    assert_eq!(archive.data["archived"], 2);
    assert_eq!(std::fs::read(&file).unwrap(), kept);
    assert_eq!(
        std::fs::read(&sidecar).unwrap(),
        [cut, resolve, b"\n".to_vec()].concat()
    );
}

#[test]
fn archive_apply_missing_discovered_default_reports_empty_warning() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path().join("repo");
    let nested = repo.join("nested");
    make_repo(&repo);
    std::fs::create_dir(&nested).unwrap();

    let output = command()
        .current_dir(&nested)
        .args(["archive", "--before", "2026-08-01T00:00:00Z"])
        .output()
        .unwrap();
    let archive: SuccessEnvelope<Value> = success(&output);
    assert_eq!(archive.data["changed"], false);
    assert_eq!(archive.data["archived"], 0);
    assert_eq!(archive.data["kept"], 0);
    assert_eq!(
        archive.meta.warnings,
        vec!["no blotter file yet; archive has nothing to remove"]
    );
    assert!(!repo.join(".blotter.jsonl").exists());
}

#[test]
fn archive_rejects_a_sidecar_collision_without_changing_the_log() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let (id, cut) = archive_cut("2026-07-01T00:00:00Z", "collision");
    let resolve = archive_resolution(&id, "2026-07-02T00:00:00Z", false, false);
    let original = [cut, resolve].concat();
    std::fs::write(&file, &original).unwrap();
    let sidecar = std::path::PathBuf::from(format!(
        "{}.archive-20260709T183000123Z.jsonl",
        file.display()
    ));
    std::fs::write(&sidecar, b"existing sidecar").unwrap();

    error(
        &run_file(&file, &["archive", "--before", "2026-08-01T00:00:00Z"]),
        74,
        "io_error",
    );
    assert_eq!(std::fs::read(&file).unwrap(), original);
    assert_eq!(std::fs::read(&sidecar).unwrap(), b"existing sidecar");
    assert!(
        !std::path::PathBuf::from(format!("{}.bak-20260709T183000123Z", file.display())).exists()
    );
}

#[test]
fn archive_cleans_created_outputs_when_replacement_fails() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let (id, cut) = archive_cut("2026-07-01T00:00:00Z", "replacement collision");
    let resolve = archive_resolution(&id, "2026-07-02T00:00:00Z", false, false);
    let original = [cut, resolve].concat();
    std::fs::write(&file, &original).unwrap();
    let locked = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&file)
        .unwrap();
    locked.lock().unwrap();
    let mut archive = std::process::Command::new(assert_cmd::cargo::cargo_bin!("blotter"));
    archive
        .env("BLOTTER_NOW", NOW)
        .env_remove("BLOTTER_FILE")
        .env_remove("BLOTTER_AGENT")
        .env_remove("BLOTTER_HOOK_EXPLAIN")
        .env_remove("PAPERCUTS_FILE")
        .env_remove("PAPERCUTS_AGENT")
        .env_remove("PAPERCUTS_NOW")
        .env_remove("CLAUDECODE")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("CODEX_")
            || key.to_string_lossy().starts_with("CURSOR_")
        {
            archive.env_remove(key);
        }
    }
    let child = archive
        .arg("--file")
        .arg(&file)
        .args(["archive", "--before", "2026-08-01T00:00:00Z"])
        .spawn()
        .unwrap();
    let temporary =
        std::path::PathBuf::from(format!("{}.tmp-archive-{}", file.display(), child.id()));
    std::fs::write(&temporary, b"existing temporary").unwrap();
    locked.unlock().unwrap();
    let output = child.wait_with_output().unwrap();

    error(&output, 74, "io_error");
    assert_eq!(std::fs::read(&file).unwrap(), original);
    assert_eq!(std::fs::read(&temporary).unwrap(), b"existing temporary");
    assert!(
        !std::path::PathBuf::from(format!("{}.bak-20260709T183000123Z", file.display())).exists()
    );
    assert!(
        !std::path::PathBuf::from(format!(
            "{}.archive-20260709T183000123Z.jsonl",
            file.display()
        ))
        .exists()
    );
}

#[test]
fn archive_times_out_under_an_exclusive_lock() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let (id, cut) = archive_cut("2026-07-01T00:00:00Z", "locked");
    let resolve = archive_resolution(&id, "2026-07-02T00:00:00Z", false, false);
    std::fs::write(&file, [cut, resolve].concat()).unwrap();
    let locked = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&file)
        .unwrap();
    locked.lock().unwrap();
    let output = run_file(&file, &["archive", "--before", "2026-08-01T00:00:00Z"]);
    locked.unlock().unwrap();
    let envelope = error(&output, 75, "lock_timeout");
    assert!(envelope.error.retryable);
}

#[test]
fn archive_schema_documents_conditional_copy_and_swap() {
    let schema: SuccessEnvelope<Value> = success(&run(&["schema"]));
    let archive = &schema.data["commands"]["archive"];
    assert_eq!(archive["flags"]["--before"], "full RFC3339|Nd|Nh; required");
    assert_eq!(
        archive["flags"]["--dry-run"],
        "boolean; plan without writes"
    );
    assert_eq!(archive["read_only"], true);
    assert_eq!(archive["destructive"], false);
    assert_eq!(archive["apply"]["read_only"], false);
    assert_eq!(archive["apply"]["destructive"], true);
    assert!(
        archive["apply"]["semantics"]
            .as_str()
            .unwrap()
            .contains("names derive from BLOTTER_NOW")
    );
    assert!(
        archive["apply"]["semantics"]
            .as_str()
            .unwrap()
            .contains("reruns under an identical clock fail with io_error by design")
    );
}

#[test]
fn doctor_leaks_reports_home_paths() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("leaking.jsonl");
    let id = compute_id(NOW, "tester", "leaking", Severity::Minor, &[]);
    let record = json!({
        "kind": "cut",
        "id": id,
        "ts": NOW,
        "agent": "tester",
        "text": "leaking",
        "tags": [],
        "severity": "minor",
        "cwd": "/Users/alice/private/repo"
    });
    std::fs::write(&file, format!("{record}\n")).unwrap();
    let output = command()
        .env("HOME", "/Users/alice")
        .arg("--file")
        .arg(&file)
        .args(["doctor", "--leaks"])
        .output()
        .unwrap();
    let doctor = doctor_response(&output, 1);
    assert_eq!(doctor.data.findings.len(), 1);
    let finding = &doctor.data.findings[0];
    assert_eq!(finding.line, 1);
    assert_eq!(finding.kind, "leak");
    assert!(!finding.fixable);
    assert!(finding.message.contains("line 1"));
    assert!(finding.message.contains("home path"));
}

#[test]
fn doctor_leaks_reports_dash_encoded_home_slugs() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("leaking.jsonl");
    let leaking = leak_record("/private/tmp/agent-501/-Users-someuser-somerepo/scratchpad");
    let benign = leak_record("relative/dir-Users-someuser-somerepo/scratchpad");
    std::fs::write(&file, format!("{leaking}\n{benign}\n")).unwrap();
    let output = command()
        .env("HOME", "/Users/alice")
        .arg("--file")
        .arg(&file)
        .args(["doctor", "--leaks"])
        .output()
        .unwrap();
    let doctor = doctor_response(&output, 1);
    let leaks: Vec<_> = doctor
        .data
        .findings
        .iter()
        .filter(|finding| finding.kind == "leak")
        .collect();
    assert_eq!(leaks.len(), 1);
    assert_eq!(leaks[0].line, 1);
    assert!(leaks[0].message.contains("home path"));
}

fn leak_record(cwd: &str) -> serde_json::Value {
    let id = compute_id(NOW, "tester", cwd, Severity::Minor, &[]);
    json!({
        "kind": "cut",
        "id": id,
        "ts": NOW,
        "agent": "tester",
        "text": cwd,
        "tags": [],
        "severity": "minor",
        "cwd": cwd
    })
}

#[test]
fn add_redacts_dash_encoded_home_slugs_in_evidence() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    // Digit-free so the entropy heuristic cannot mask the home-slug rewrite.
    let added = add_evidence_note(
        &file,
        "wrote /private/tmp/agent/-Users-someuser-somerepo/scratchpad/out.txt",
    );
    assert_eq!(
        added.data.record.cut_evidence().unwrap().note.as_deref(),
        Some("wrote /private/tmp/agent/~-somerepo/scratchpad/out.txt")
    );
}

#[test]
fn doctor_leaks_reports_dash_encoded_current_home_outside_generic_prefixes() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("leaking.jsonl");
    // A custom $HOME outside /Users and /home only leaks through the exact
    // dash-encoded current-home rule; the generic prefixes cannot match it.
    let leaking = leak_record("/private/tmp/agent/-var-root-somerepo/scratchpad");
    std::fs::write(&file, format!("{leaking}\n")).unwrap();
    let output = command()
        .env("HOME", "/var/root")
        .arg("--file")
        .arg(&file)
        .args(["doctor", "--leaks"])
        .output()
        .unwrap();
    let doctor = doctor_response(&output, 1);
    let leaks: Vec<_> = doctor
        .data
        .findings
        .iter()
        .filter(|finding| finding.kind == "leak")
        .collect();
    assert_eq!(leaks.len(), 1);
    assert_eq!(leaks[0].line, 1);
}

#[test]
fn add_redacts_dash_encoded_dashed_username_fully() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    // A dash inside the username must not truncate the redaction: the exact
    // dash-encoded current home wins over the generic first-component rule.
    let added: SuccessEnvelope<AddData> = success(
        &command()
            .env("HOME", "/Users/jane-doe")
            .arg("--file")
            .arg(&file)
            .args([
                "add",
                "evidence case",
                "--agent",
                "tester",
                "--evidence",
                "wrote /private/tmp/agent/-Users-jane-doe-somerepo/scratchpad/out.txt",
            ])
            .output()
            .unwrap(),
    );
    assert_eq!(
        added.data.record.cut_evidence().unwrap().note.as_deref(),
        Some("wrote /private/tmp/agent/~-somerepo/scratchpad/out.txt")
    );
}

#[test]
fn add_and_dogear_redact_home_text_before_identity_hashing() {
    let temp = TempDir::new().unwrap();
    let home = "/Users/jane-doe";
    let cases = [
        (
            "slash",
            "failed under /Users/jane-doe/workspace",
            "failed under ~/workspace",
        ),
        (
            "dash",
            "failed under /private/tmp/session/-Users-jane-doe-workspace/log",
            "failed under /private/tmp/session/~-workspace/log",
        ),
    ];

    for (name, input, expected) in cases {
        let add_file = temp.path().join(format!("{name}-add.jsonl"));
        let added: SuccessEnvelope<AddData> = success(
            &command()
                .env("HOME", home)
                .arg("--file")
                .arg(&add_file)
                .args(["add", input, "--agent", "tester"])
                .output()
                .unwrap(),
        );
        let add_record = serde_json::to_value(&added.data.record).unwrap();
        assert_eq!(add_record["text"], expected);
        assert_eq!(
            add_record["id"],
            compute_id(
                add_record["ts"].as_str().unwrap(),
                "tester",
                expected,
                Severity::Minor,
                &[]
            )
        );
        let stored_add: Value =
            serde_json::from_str(&std::fs::read_to_string(&add_file).unwrap()).unwrap();
        assert_eq!(stored_add, add_record);

        let dogear_file = temp.path().join(format!("{name}-dogear.jsonl"));
        let dogear: SuccessEnvelope<Value> = success(
            &command()
                .env("HOME", home)
                .arg("--file")
                .arg(&dogear_file)
                .args(["dogear", input, "--agent", "tester"])
                .output()
                .unwrap(),
        );
        let dogear_record = dogear.data["record"].clone();
        assert_eq!(dogear_record["text"], expected);
        assert_eq!(
            dogear_record["id"],
            compute_dogear_id(
                dogear_record["ts"].as_str().unwrap(),
                "tester",
                expected,
                &[]
            )
        );
        let stored_dogear: Value =
            serde_json::from_str(&std::fs::read_to_string(&dogear_file).unwrap()).unwrap();
        assert_eq!(stored_dogear, dogear_record);
    }
}

#[test]
fn add_and_dogear_preserve_non_home_text_and_identity() {
    let temp = TempDir::new().unwrap();
    let home = "/Users/alice";
    let text = "cargo test failed after compiling /opt/build";

    let add_file = temp.path().join("add.jsonl");
    let added: SuccessEnvelope<AddData> = success(
        &command()
            .env("HOME", home)
            .arg("--file")
            .arg(&add_file)
            .args(["add", text, "--agent", "tester"])
            .output()
            .unwrap(),
    );
    let add_record = serde_json::to_value(&added.data.record).unwrap();
    assert_eq!(add_record["text"], text);
    assert_eq!(
        add_record["id"],
        compute_id(
            add_record["ts"].as_str().unwrap(),
            "tester",
            text,
            Severity::Minor,
            &[]
        )
    );

    let dogear_file = temp.path().join("dogear.jsonl");
    let dogear: SuccessEnvelope<Value> = success(
        &command()
            .env("HOME", home)
            .arg("--file")
            .arg(&dogear_file)
            .args(["dogear", text, "--agent", "tester"])
            .output()
            .unwrap(),
    );
    let dogear_record = &dogear.data["record"];
    assert_eq!(dogear_record["text"], text);
    assert_eq!(
        dogear_record["id"],
        compute_dogear_id(dogear_record["ts"].as_str().unwrap(), "tester", text, &[])
    );
}

#[test]
fn add_deduplicates_texts_that_only_differ_in_redacted_home_prefix() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let first: SuccessEnvelope<AddData> = success(
        &command()
            .env("HOME", "/Users/alice")
            .arg("--file")
            .arg(&file)
            .args([
                "add",
                "failed under /Users/alice/workspace",
                "--agent",
                "tester",
            ])
            .output()
            .unwrap(),
    );
    let second: SuccessEnvelope<AddData> = success(
        &command()
            .env("HOME", "/Users/alice")
            .arg("--file")
            .arg(&file)
            .args([
                "add",
                "failed under /home/other/workspace",
                "--agent",
                "tester",
            ])
            .output()
            .unwrap(),
    );

    assert!(first.data.changed);
    assert!(!second.data.changed);
    assert_eq!(second.data.record.cut_id(), first.data.record.cut_id());
    assert_eq!(
        second.meta.warnings,
        ["duplicate cut; existing record returned"]
    );
    let stored: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(stored["text"], "failed under ~/workspace");
    assert_eq!(std::fs::read_to_string(&file).unwrap().lines().count(), 1);
}

#[test]
fn doctor_leaks_conflicts_with_fix_modes() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("leaking.jsonl");
    let original = b"{ malformed /Users/alice/private\n";
    std::fs::write(&file, original).unwrap();

    for args in [
        &["doctor", "--leaks", "--fix"][..],
        &["doctor", "--leaks", "--fix", "--dry-run"][..],
    ] {
        let envelope = error(&run_file(&file, args), 2, "invalid_argument");
        assert!(envelope.error.message.contains("--leaks"));
        assert!(envelope.error.message.contains("--fix"));
        assert!(envelope.error.suggested_fix.contains("blotter --help"));
        assert_eq!(std::fs::read(&file).unwrap(), original);
    }
}

#[test]
fn doctor_leaks_is_clean_without_paths_and_deny_matches_literals() {
    let temp = TempDir::new().unwrap();
    let clean = temp.path().join("clean.jsonl");
    let clean_id = compute_id(NOW, "tester", "clean", Severity::Minor, &[]);
    let clean_record = json!({
        "kind": "cut",
        "id": clean_id,
        "ts": NOW,
        "agent": "tester",
        "text": "clean",
        "tags": [],
        "severity": "minor",
        "cwd": "~"
    });
    std::fs::write(&clean, format!("{clean_record}\n")).unwrap();
    let clean_output = command()
        .env("HOME", "/Users/alice")
        .arg("--file")
        .arg(&clean)
        .args(["doctor", "--leaks"])
        .output()
        .unwrap();
    assert!(doctor_response(&clean_output, 0).data.findings.is_empty());

    let denied = temp.path().join("denied.jsonl");
    let denied_id = compute_id(NOW, "tester", "literal credential", Severity::Minor, &[]);
    let denied_record = json!({
        "kind": "cut",
        "id": denied_id,
        "ts": NOW,
        "agent": "tester",
        "text": "literal credential",
        "tags": [],
        "severity": "minor",
        "cwd": "~"
    });
    std::fs::write(&denied, format!("{denied_record}\n")).unwrap();
    let denied_output = command()
        .arg("--file")
        .arg(&denied)
        .args([
            "doctor",
            "--leaks",
            "--deny",
            "does-not-occur",
            "--deny",
            "literal credential",
        ])
        .output()
        .unwrap();
    let denied = doctor_response(&denied_output, 1);
    assert_eq!(denied.data.findings.len(), 1);
    assert_eq!(denied.data.findings[0].kind, "leak");
    assert!(!denied.data.findings[0].fixable);
    assert!(denied.data.findings[0].message.contains("deny pattern"));
    assert!(
        denied.data.findings[0]
            .message
            .contains("literal credential")
    );
}

#[test]
fn plain_doctor_healthy_output_remains_byte_identical() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    add(&file, "valid");

    let output = run_file(&file, &["doctor"]);
    let mut expected = serde_json::to_vec(&json!({
        "ok": true,
        "data": {"healthy": true, "findings": [], "checked_lines": 1},
        "meta": {"contract": 5, "file": file.to_string_lossy()},
    }))
    .unwrap();
    expected.push(b'\n');
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, expected);
}

#[test]
fn doctor_reports_pre_framing_bl_ids_as_conflicts_after_legacy_fallback_removal() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("legacy-v1.jsonl");
    // Frozen v1 hash for this exact comma-joined, non-deduplicated tag fixture.
    let legacy_cut = json!({
        "kind": "cut",
        "id": "bl_d7e14e635d21",
        "ts": "2026-07-10T00:00:00.000Z",
        "agent": "legacy",
        "text": "legacy v1 cut",
        "tags": ["a", "a", "b"],
        "severity": "major",
        "cwd": "/tmp",
        "repo": null
    });
    std::fs::write(&file, format!("{legacy_cut}\n")).unwrap();

    let output = run_file(&file, &["doctor"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let doctor: SuccessEnvelope<DoctorData> = serde_json::from_slice(&output.stdout).unwrap();
    assert!(!doctor.data.healthy);
    assert_eq!(doctor.data.findings.len(), 1);
    assert_eq!(doctor.data.findings[0].kind, "id_conflict");
    assert_eq!(doctor.data.checked_lines, 1);
}

#[test]
fn torn_tail_self_heals_on_add() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    std::fs::write(&file, b"{\"kind\":\"cut\"").unwrap();
    let added = add(&file, "after tear");
    assert!(added.data.changed);
    let bytes = std::fs::read(&file).unwrap();
    assert!(bytes.ends_with(b"\n"));
    assert_eq!(bytes.split(|byte| *byte == b'\n').count(), 3);
    let listed: SuccessEnvelope<ListData> = success(&run_file(&file, &["list"]));
    assert_eq!(listed.data.items.len(), 1);
    assert_eq!(listed.data.items[0].text, "after tear");
    assert!(
        listed
            .meta
            .warnings
            .iter()
            .any(|warning| warning.contains("malformed"))
    );
}

#[test]
fn doctor_finding_counts_match_fold_bytes_warning_counts() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let valid_id = compute_id(
        "2026-07-09T00:00:00.000Z",
        "a",
        "valid",
        Severity::Minor,
        &[],
    );
    let malformed = json!({
        "kind": "cut",
        "id": "bl_000000000000",
        "ts": "not-a-time",
        "agent": "a",
        "text": "malformed",
        "tags": [],
        "severity": "minor",
        "cwd": "/tmp",
        "repo": null
    })
    .to_string();
    let valid = json!({
        "kind": "cut",
        "id": valid_id,
        "ts": "2026-07-09T00:00:00.000Z",
        "agent": "a",
        "text": "valid",
        "tags": [],
        "severity": "minor",
        "cwd": "/tmp",
        "repo": null
    })
    .to_string();
    let orphan = json!({
        "kind": "resolve",
        "id": "bl_deadbeef0000",
        "ts": "2026-07-09T00:00:00.000Z",
        "agent": "a",
        "note": null
    })
    .to_string();
    let unknown = json!({"kind": "future"}).to_string();
    let fixture = format!("{malformed}\n{valid}\n{orphan}\n{valid}\n{unknown}\n{{\"kind\":");
    std::fs::write(&file, fixture).unwrap();

    let folded = blotter::store::fold_bytes(&std::fs::read(&file).unwrap());
    let doctor_output = run_file(&file, &["doctor"]);
    assert_eq!(doctor_output.status.code(), Some(1));
    assert!(doctor_output.stderr.is_empty());
    let doctor: SuccessEnvelope<DoctorData> =
        serde_json::from_slice(&doctor_output.stdout).unwrap();

    let fold_counts = fold_warning_counts(&folded.warnings);
    let doctor_counts = doctor_finding_counts(&doctor.data.findings);
    let expected: HashMap<String, usize> = [
        ("malformed", 1),
        ("unknown", 1),
        ("duplicate_cut", 1),
        ("orphan_resolve", 1),
        ("torn", 1),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    assert_eq!(
        fold_counts, expected,
        "fold warnings: {:?}",
        folded.warnings
    );
    assert_eq!(
        doctor_counts, expected,
        "doctor findings: {:?}",
        doctor.data.findings
    );
}

fn fold_warning_counts(warnings: &[String]) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for warning in warnings {
        let parts: Vec<_> = warning.splitn(3, ' ').collect();
        let count: usize = parts[1].parse().unwrap();
        let label = parts[2].trim_end_matches('s');
        let key = if label.starts_with("malformed line") {
            "malformed"
        } else if label.starts_with("torn final line") {
            "torn"
        } else if label.starts_with("unknown event") {
            "unknown"
        } else if label.starts_with("duplicate cut") {
            "duplicate_cut"
        } else if label.starts_with("duplicate resolve") {
            "duplicate_resolve"
        } else if label.starts_with("orphan resolve") {
            "orphan_resolve"
        } else {
            panic!("unknown fold warning label: {label}")
        };
        counts.insert(key.to_string(), count);
    }
    counts
}

fn doctor_finding_counts(
    findings: &[blotter::commands::doctor::Finding],
) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for finding in findings {
        let key = match finding.kind.as_str() {
            "malformed" => "malformed",
            "torn_line" => "torn",
            "unknown_kind" => "unknown",
            "duplicate_cut" => "duplicate_cut",
            "orphan_resolve" => "orphan_resolve",
            _ => continue,
        };
        *counts.entry(key.to_string()).or_insert(0) += 1;
    }
    counts
}

#[test]
fn discovery_precedence_virtual_empty_and_git_file_root() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("repo");
    let nested = root.join("a/b");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(root.join(".git"), "gitdir: elsewhere\n").unwrap();
    let env_file = temp.path().join("env.jsonl");
    let flag_file = temp.path().join("flag.jsonl");

    let walk: SuccessEnvelope<AddData> = success(
        &command()
            .current_dir(&nested)
            .args(["add", "x", "--agent", "a", "--dry-run"])
            .output()
            .unwrap(),
    );
    let canonical_root = root.canonicalize().unwrap();
    assert_eq!(
        walk.meta.file.as_deref(),
        Some(canonical_root.join(".blotter.jsonl").to_str().unwrap())
    );
    let empty_env: SuccessEnvelope<AddData> = success(
        &command()
            .current_dir(&nested)
            .env("BLOTTER_FILE", "")
            .args(["add", "x", "--agent", "a", "--dry-run"])
            .output()
            .unwrap(),
    );
    assert_eq!(empty_env.meta.file, walk.meta.file);

    let env: SuccessEnvelope<AddData> = success(
        &command()
            .current_dir(&nested)
            .env("BLOTTER_FILE", &env_file)
            .args(["add", "x", "--agent", "a", "--dry-run"])
            .output()
            .unwrap(),
    );
    assert_eq!(env.meta.file.as_deref(), Some(env_file.to_str().unwrap()));

    let flag: SuccessEnvelope<AddData> = success(
        &command()
            .current_dir(&nested)
            .env("BLOTTER_FILE", &env_file)
            .arg("--file")
            .arg(&flag_file)
            .args(["add", "x", "--agent", "a", "--dry-run"])
            .output()
            .unwrap(),
    );
    assert_eq!(flag.meta.file.as_deref(), Some(flag_file.to_str().unwrap()));

    let empty: SuccessEnvelope<ListData> =
        success(&command().current_dir(&nested).arg("list").output().unwrap());
    assert!(empty.data.items.is_empty());
    assert!(
        empty
            .meta
            .warnings
            .iter()
            .any(|warning| warning.contains("no blotter file"))
    );

    if !temp_has_git_ancestor(&temp) {
        let outside = temp.path().join("outside");
        let home = temp.path().join("home");
        std::fs::create_dir_all(&outside).unwrap();
        let home_result: SuccessEnvelope<AddData> = success(
            &command()
                .current_dir(&outside)
                .env("HOME", &home)
                .args(["add", "x", "--agent", "a", "--dry-run"])
                .output()
                .unwrap(),
        );
        assert_eq!(
            home_result.meta.file.as_deref(),
            Some(home.join(".blotter/log.jsonl").to_str().unwrap())
        );
        assert!(
            !home.exists(),
            "dry run must not create the home fallback directory"
        );
        let no_home = command()
            .current_dir(&outside)
            .env_remove("HOME")
            .arg("list")
            .output()
            .unwrap();
        error(&no_home, 78, "config_error");
    } else {
        eprintln!(
            "skipping home-fallback assertions because the temporary directory is inside a git checkout"
        );
    }
}

#[test]
fn fixed_clock_fresh_state_is_byte_deterministic_and_retry_is_duplicate_safe() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let first = run_file(&file, &["add", "same", "--agent", "tester"]);
    assert!(first.status.success());
    std::fs::remove_file(&file).unwrap();
    let fresh = run_file(&file, &["add", "same", "--agent", "tester"]);
    assert_eq!(first.stdout, fresh.stdout);
    let retry: SuccessEnvelope<AddData> =
        success(&run_file(&file, &["add", "same", "--agent", "tester"]));
    assert!(!retry.data.changed);
}

#[test]
fn home_path_output_is_byte_deterministic_with_a_fixed_clock() {
    let temp = TempDir::new().unwrap();
    if temp_has_git_ancestor(&temp) {
        eprintln!("skipping home determinism assertion inside a git checkout");
        return;
    }
    let home = temp.path().join("home");
    let cwd = home.join("project");
    let file = temp.path().join("cuts.jsonl");
    std::fs::create_dir_all(&cwd).unwrap();
    let home = home.canonicalize().unwrap();
    let cwd = cwd.canonicalize().unwrap();
    let evidence = format!("failed under {}/logs", home.display());

    let first = command()
        .env("HOME", &home)
        .current_dir(&cwd)
        .arg("--file")
        .arg(&file)
        .args(["add", "same", "--agent", "tester", "--evidence"])
        .arg(&evidence)
        .output()
        .unwrap();
    assert!(first.status.success());
    let first_data: SuccessEnvelope<AddData> = success(&first);
    assert_eq!(first_data.data.record.cut_cwd(), "~/project");
    assert_eq!(
        first_data
            .data
            .record
            .cut_evidence()
            .unwrap()
            .note
            .as_deref(),
        Some("failed under ~/logs")
    );
    std::fs::remove_file(&file).unwrap();
    let fresh = command()
        .env("HOME", &home)
        .current_dir(&cwd)
        .arg("--file")
        .arg(&file)
        .args(["add", "same", "--agent", "tester", "--evidence"])
        .arg(&evidence)
        .output()
        .unwrap();
    assert_eq!(first.stdout, fresh.stdout);
}

#[test]
fn eight_way_distinct_add_race_loses_no_lines() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let barrier = Arc::new(Barrier::new(8));
    let handles: Vec<_> = (0..8)
        .map(|thread_id| {
            let file = file.clone();
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                for item in 0..4 {
                    let text = format!("thread-{thread_id}-item-{item}");
                    let output = run_file(&file, &["add", &text, "--agent", "race"]);
                    assert!(
                        output.status.success(),
                        "{}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
    let contents = std::fs::read_to_string(&file).unwrap();
    assert_eq!(contents.lines().count(), 32);
    for line in contents.lines() {
        serde_json::from_str::<Value>(line).unwrap();
    }
}

#[test]
fn eight_way_identical_add_race_appends_once() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let barrier = Arc::new(Barrier::new(8));
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let file = file.clone();
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let envelope: SuccessEnvelope<AddData> =
                    success(&run_file(&file, &["add", "identical", "--agent", "race"]));
                envelope.data.changed
            })
        })
        .collect();
    let changed = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .filter(|changed| *changed)
        .count();
    assert_eq!(changed, 1);
    assert_eq!(std::fs::read_to_string(&file).unwrap().lines().count(), 1);
}

#[test]
fn eight_way_resolve_race_appends_once() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let id = add(&file, "resolve race").data.record.cut_id().to_owned();
    let barrier = Arc::new(Barrier::new(8));
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let file = file.clone();
            let id = id.clone();
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let envelope: SuccessEnvelope<ResolveData> =
                    success(&run_file(&file, &["resolve", &id, "--agent", "race"]));
                envelope.data.changed
            })
        })
        .collect();
    let changed = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .filter(|changed| *changed)
        .count();
    assert_eq!(changed, 1);
    assert_eq!(std::fs::read_to_string(&file).unwrap().lines().count(), 2);
}

#[test]
fn hash_length_prefix_and_tag_sort_are_pinned() {
    let a = compute_id(
        "2026-07-09T18:30:00.123Z",
        "tester",
        "ouch",
        Severity::Major,
        &["a".into(), "z".into()],
    );
    let b = compute_id(
        "2026-07-09T18:30:00.123Z",
        "tester",
        "ouc",
        Severity::Major,
        &["z".into(), "ha".into()],
    );
    let unsorted = compute_id(
        "2026-07-09T18:30:00.123Z",
        "tester",
        "ouch",
        Severity::Major,
        &["z".into(), "a".into()],
    );
    assert_eq!(a, "bl_a43e5b0b30aa");
    assert_eq!(a, unsorted);
    assert_ne!(a, b);
}

#[test]
fn env_blotter_file_nonexistent_returns_not_found() {
    let temp = TempDir::new().unwrap();
    let missing = temp.path().join("missing.jsonl");
    let output = command()
        .env("BLOTTER_FILE", &missing)
        .arg("list")
        .output()
        .unwrap();
    error(&output, 66, "not_found");
}

#[test]
fn relative_file_resolves_against_cwd() {
    let temp = TempDir::new().unwrap();
    let output = command()
        .current_dir(temp.path())
        .arg("--file")
        .arg("rel/path.jsonl")
        .args(["add", "x", "--agent", "a", "--dry-run"])
        .output()
        .unwrap();
    let envelope: SuccessEnvelope<AddData> = success(&output);
    let temp_canonical = temp.path().canonicalize().unwrap();
    assert!(
        Path::new(envelope.meta.file.as_deref().unwrap()).starts_with(&temp_canonical),
        "meta.file = {:?}",
        envelope.meta.file
    );
}

#[test]
fn markdown_format_is_byte_deterministic() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let added = add(&file, "determinism");
    let first = run_file(&file, &["list", "--format", "md"]);
    assert!(first.status.success());
    assert!(!first.stdout.is_empty());
    let first_text = String::from_utf8_lossy(&first.stdout);
    assert!(first_text.contains("determinism"));
    assert!(first_text.contains(added.data.record.cut_id()));
    let second = run_file(&file, &["list", "--format", "md"]);
    assert!(second.status.success());
    assert_eq!(first.stdout, second.stdout);
}

#[test]
fn doctor_reports_gitignored_finding() {
    let git_available = std::process::Command::new("git")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success());
    if !git_available {
        return;
    }

    let temp = TempDir::new().unwrap();
    let repo = temp.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    assert!(
        std::process::Command::new("git")
            .arg("-C")
            .arg(&repo)
            .arg("init")
            .output()
            .unwrap()
            .status
            .success()
    );
    std::fs::write(repo.join(".gitignore"), ".blotter.jsonl\n").unwrap();

    let empty_output = command().current_dir(&repo).arg("doctor").output().unwrap();
    let empty: SuccessEnvelope<DoctorData> = success(&empty_output);
    assert!(empty.data.healthy);
    assert!(
        empty
            .data
            .findings
            .iter()
            .all(|finding| finding.kind != "gitignored")
    );

    let output = command()
        .current_dir(&repo)
        .args(["add", "gitignored cut", "--agent", "a"])
        .output()
        .unwrap();
    success::<AddData>(&output);

    let doctor_output = command().current_dir(&repo).arg("doctor").output().unwrap();
    assert_eq!(doctor_output.status.code(), Some(1));
    assert!(doctor_output.stderr.is_empty());
    let doctor: SuccessEnvelope<DoctorData> =
        serde_json::from_slice(&doctor_output.stdout).unwrap();
    assert!(!doctor.data.healthy);
    assert!(
        doctor
            .data
            .findings
            .iter()
            .any(|finding| finding.kind == "gitignored")
    );
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
fn triage_clusters_three_near_duplicate_open_cuts() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let first = add_at(
        &file,
        "2026-07-09T18:30:00Z",
        "Cargo test fails because config is missing",
        &["api", "tooling"],
    );
    let second = add_at(
        &file,
        "2026-07-09T18:31:00Z",
        "cargo-test fails because config is missing!",
        &["tooling"],
    );
    let third = add_at(
        &file,
        "2026-07-09T18:32:00Z",
        "Cargo test fails because config is missing again",
        &["api"],
    );

    let triage = triage_success(&run_file(&file, &["triage"]), 1);
    assert_eq!(triage.data["count"], 1);
    assert_eq!(triage.data["scanned"], 3);
    assert_eq!(
        triage.data["clusters"],
        json!([{
            "count": 3,
            // Keyed on the displayed text ("…missing again"), which is unique;
            // the representative's title would count 2 against a title the
            // consumer never sees.
            "occurrences": 1,
            "ids": [
                first.data.record.cut_id(),
                second.data.record.cut_id(),
                third.data.record.cut_id(),
            ],
            "tags": ["api", "tooling"],
            "text": "Cargo test fails because config is missing again",
            "suggested_action": "graduate",
        }])
    );
}

#[test]
fn triage_clusters_reworded_repeats_with_rare_shared_tokens() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let first = add_at(
        &file,
        "2026-07-09T18:30:00Z",
        "tsx -e emits CommonJS here and rejects top-level await; async one-off compiler probes need an explicit async IIFE.",
        &["tooling"],
    );
    let second = add_at(
        &file,
        "2026-07-09T18:31:00Z",
        "tsx -e uses CJS output, so a one-line offline diagnostics probe cannot use top-level await; wrap it in an async function.",
        &["tooling"],
    );

    let first_run = run_file(&file, &["triage", "--min-count", "2"]);
    let second_run = run_file(&file, &["triage", "--min-count", "2"]);
    assert_eq!(first_run.stdout, second_run.stdout);

    let triage = triage_success(&first_run, 1);
    assert_eq!(triage.data["count"], 1);
    assert_eq!(triage.data["scanned"], 2);
    assert_eq!(
        triage.data["clusters"][0]["ids"],
        json!([first.data.record.cut_id(), second.data.record.cut_id()])
    );
}

#[test]
fn triage_does_not_cluster_common_filler_with_a_shared_tag() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    add_at(
        &file,
        "2026-07-09T18:30:00Z",
        "need to update the config file for the build",
        &["tooling"],
    );
    add_at(
        &file,
        "2026-07-09T18:31:00Z",
        "need to check the readme file for the release",
        &["tooling"],
    );

    let triage = triage_success(&run_file(&file, &["triage", "--min-count", "2"]), 0);
    assert_eq!(triage.data["count"], 0);
    assert_eq!(triage.data["clusters"], json!([]));
}

#[test]
fn triage_does_not_cluster_empty_scoring_tokens() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    add_at(&file, "2026-07-09T18:30:00Z", "the and for", &["tooling"]);
    add_at(
        &file,
        "2026-07-09T18:31:00Z",
        "this with need",
        &["tooling"],
    );

    let triage = triage_success(&run_file(&file, &["triage", "--min-count", "2"]), 0);
    assert_eq!(triage.data["count"], 0);
    assert_eq!(triage.data["clusters"], json!([]));
}

#[test]
fn triage_surfaces_repeated_normalized_title_occurrences() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let first = add_at(
        &file,
        "2026-07-09T18:30:00Z",
        "Workspace cache missing during build",
        &["build"],
    );
    let second = add_at(
        &file,
        "2026-07-09T18:31:00Z",
        "Workspace cache missing during build",
        &["build"],
    );
    let third = add_at(
        &file,
        "2026-07-09T18:32:00Z",
        "Workspace cache missing during build",
        &["build"],
    );

    let triage = triage_success(&run_file(&file, &["triage"]), 1);
    assert_eq!(
        triage.data["clusters"],
        json!([{
            "count": 3,
            "occurrences": 3,
            "ids": [
                first.data.record.cut_id(),
                second.data.record.cut_id(),
                third.data.record.cut_id(),
            ],
            "tags": ["build"],
            "text": "Workspace cache missing during build",
            "suggested_action": "graduate",
        }])
    );
}

#[test]
fn a_blank_line_after_a_record_is_malformed() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    add(&file, "valid");
    let mut writer = OpenOptions::new().append(true).open(&file).unwrap();
    writeln!(writer).unwrap();
    drop(writer);

    let listed: SuccessEnvelope<ListData> = success(&run_file(&file, &["list"]));
    assert_eq!(listed.meta.warnings, ["skipped 1 malformed line"]);

    let output = run_file(&file, &["doctor"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let doctor: SuccessEnvelope<DoctorData> = serde_json::from_slice(&output.stdout).unwrap();
    assert!(!doctor.data.healthy);
    assert_eq!(doctor.data.checked_lines, 2);
    assert_eq!(doctor.data.findings.len(), 1);
    assert_eq!(doctor.data.findings[0].line, 2);
    assert_eq!(doctor.data.findings[0].kind, "malformed");
    assert_eq!(doctor.data.findings[0].message, "line is not valid JSON");
}

#[test]
fn a_file_holding_only_a_newline_folds_with_no_line_warnings() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    std::fs::write(&file, "\n").unwrap();
    let listed: SuccessEnvelope<Value> = success(&run_file(&file, &["list", "--status", "all"]));
    let warnings = listed.meta.warnings;
    assert!(
        !warnings.iter().any(|warning| warning.contains("malformed")),
        "a lone trailing newline is a terminator, not a malformed line: {warnings:?}"
    );
}

#[test]
fn triage_releases_members_of_below_threshold_clusters() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let first = add_at(&file, "2026-07-09T18:30:00Z", "alpha bravo charlie", &[]);
    let second = add_at(
        &file,
        "2026-07-09T18:31:00Z",
        "alpha bravo charlie delta echo foxtrot golf hotel india",
        &[],
    );
    let third = add_at(&file, "2026-07-09T18:32:00Z", "delta echo foxtrot", &[]);
    let fourth = add_at(&file, "2026-07-09T18:33:00Z", "golf hotel india", &[]);

    // The earliest cut links only the second one, a below-threshold pair. Its
    // members must stay free so the second cut can represent the real
    // three-member cluster.
    let triage = triage_success(&run_file(&file, &["triage", "--min-count", "3"]), 1);
    assert_eq!(triage.data["count"], 1);
    assert_eq!(triage.data["scanned"], 4);
    assert_eq!(
        triage.data["clusters"][0]["ids"],
        json!([
            second.data.record.cut_id(),
            third.data.record.cut_id(),
            fourth.data.record.cut_id()
        ])
    );
    assert!(
        !triage.data["clusters"][0]["ids"]
            .as_array()
            .unwrap()
            .iter()
            .any(|id| id.as_str() == Some(first.data.record.cut_id())),
        "the released below-threshold representative must not join the cluster"
    );
}

#[test]
fn triage_links_identical_titles_with_disjoint_nonempty_tags() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let first = add_at(
        &file,
        "2026-07-09T18:30:00Z",
        "Workspace cache missing during build",
        &["alpha"],
    );
    let second = add_at(
        &file,
        "2026-07-09T18:31:00Z",
        "Workspace cache missing during build",
        &["beta"],
    );
    let third = add_at(
        &file,
        "2026-07-09T18:32:00Z",
        "Workspace cache missing during build",
        &["gamma"],
    );

    let triage = triage_success(&run_file(&file, &["triage"]), 1);
    assert_eq!(triage.data["count"], 1);
    assert_eq!(
        triage.data["clusters"],
        json!([{
            "count": 3,
            "occurrences": 3,
            "ids": [
                first.data.record.cut_id(),
                second.data.record.cut_id(),
                third.data.record.cut_id(),
            ],
            "tags": ["alpha", "beta", "gamma"],
            "text": "Workspace cache missing during build",
            "suggested_action": "graduate",
        }])
    );
}

#[test]
fn triage_does_not_transitively_merge_an_untagged_similarity_chain() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let bridge = add_at(&file, "2026-07-09T18:31:00Z", "alpha beta gamma delta", &[]);
    let third = add_at(&file, "2026-07-09T18:32:00Z", "gamma delta", &[]);
    let first = add_at(&file, "2026-07-09T18:30:00Z", "alpha beta", &[]);

    let triage = triage_success(&run_file(&file, &["triage", "--min-count", "2"]), 1);
    assert_eq!(triage.data["count"], 1);
    assert_eq!(triage.data["scanned"], 3);
    assert_eq!(
        triage.data["clusters"],
        json!([{
            "count": 2,
            "occurrences": 1,
            "ids": [first.data.record.cut_id(), bridge.data.record.cut_id()],
            "tags": [],
            "text": "alpha beta gamma delta",
            "suggested_action": "graduate",
        }])
    );
    assert!(
        !triage.data["clusters"][0]["ids"]
            .as_array()
            .unwrap()
            .iter()
            .any(|id| id.as_str() == Some(third.data.record.cut_id())),
        "the disjoint tail of the similarity chain must not join the cluster"
    );
}

#[test]
fn triage_two_similar_cuts_are_not_chronic_at_the_default_threshold() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    add_at(
        &file,
        "2026-07-09T18:30:00Z",
        "Missing local cache during compile",
        &[],
    );
    add_at(
        &file,
        "2026-07-09T18:31:00Z",
        "missing local cache during compile!",
        &[],
    );

    let triage = triage_success(&run_file(&file, &["triage"]), 0);
    assert_eq!(triage.data["clusters"], json!([]));
    assert_eq!(triage.data["count"], 0);
    assert_eq!(triage.data["scanned"], 2);
}

#[test]
fn triage_excludes_resolved_cuts_and_dogears() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let first = add_at(
        &file,
        "2026-07-09T18:30:00Z",
        "Cache restore fails after deploy",
        &["ops"],
    );
    add_at(
        &file,
        "2026-07-09T18:31:00Z",
        "cache restore fails after deploy!",
        &["ops"],
    );
    add_at(
        &file,
        "2026-07-09T18:32:00Z",
        "cache restore fails after deploy again",
        &["ops"],
    );
    success::<ResolveData>(&run_file(
        &file,
        &["resolve", first.data.record.cut_id(), "--agent", "fixer"],
    ));
    let dogear: SuccessEnvelope<Value> = success(
        &command()
            .arg("--file")
            .arg(&file)
            .args([
                "dogear",
                "cache restore fails after deploy again",
                "--agent",
                "researcher",
                "--tag",
                "ops",
            ])
            .output()
            .unwrap(),
    );
    assert_eq!(dogear.data["record"]["kind"], "dogear");

    let triage = triage_success(&run_file(&file, &["triage"]), 0);
    assert_eq!(triage.data["clusters"], json!([]));
    assert_eq!(triage.data["count"], 0);
    assert_eq!(triage.data["scanned"], 2);
}

#[test]
fn triage_does_not_link_similar_but_nonidentical_cuts_with_disjoint_nonempty_tags() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    for (now, text, tag) in [
        (
            "2026-07-09T18:30:00Z",
            "The cache restore endpoint returns an error",
            "alpha",
        ),
        (
            "2026-07-09T18:31:00Z",
            "The cache restore endpoint returns an error again",
            "beta",
        ),
        (
            "2026-07-09T18:32:00Z",
            "The cache restore endpoint still returns an error",
            "gamma",
        ),
    ] {
        add_at(&file, now, text, &[tag]);
    }

    let triage = triage_success(&run_file(&file, &["triage"]), 0);
    assert_eq!(triage.data["clusters"], json!([]));
    assert_eq!(triage.data["count"], 0);
    assert_eq!(triage.data["scanned"], 3);
}

#[test]
fn triage_min_count_two_flags_a_pair_and_rejects_one() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let first = add_at(
        &file,
        "2026-07-09T18:30:00Z",
        "The command output is missing a summary",
        &[],
    );
    let second = add_at(
        &file,
        "2026-07-09T18:31:00Z",
        "the command output is missing a summary!",
        &[],
    );

    let triage = triage_success(&run_file(&file, &["triage", "--min-count", "2"]), 1);
    assert_eq!(triage.data["count"], 1);
    assert_eq!(triage.data["scanned"], 2);
    assert_eq!(
        triage.data["clusters"][0],
        json!({
            "count": 2,
            "occurrences": 2,
            "ids": [first.data.record.cut_id(), second.data.record.cut_id()],
            "tags": [],
            "text": "the command output is missing a summary!",
            "suggested_action": "graduate",
        })
    );
    error(
        &run_file(&file, &["triage", "--min-count", "1"]),
        2,
        "invalid_argument",
    );
}

#[test]
fn schema_documents_triage() {
    let schema: SuccessEnvelope<Value> = success(&run(&["schema"]));
    let triage = &schema.data["commands"]["triage"];
    assert_eq!(
        triage["flags"]["--min-count"],
        "N; default 3; must be at least 2"
    );
    assert!(
        triage["flags"]["--include-auto"]
            .as_str()
            .unwrap()
            .contains("include records tagged auto")
    );
    assert_eq!(
        triage["output"],
        "{clusters:[{count,occurrences,ids,tags,text,source?,suggested_action}],count,scanned}"
    );
    assert!(
        triage["semantics"]
            .as_str()
            .unwrap()
            .contains("filtered-token linkage")
    );
    assert_eq!(
        triage["exit_codes"],
        json!({"0":"no chronic clusters","1":"chronic clusters found"})
    );
    assert_eq!(triage["read_only"], true);
    assert_eq!(triage["appends"], false);
    assert_eq!(triage["destructive"], false);
}

#[test]
fn verify_no_recurrence_is_empty_and_successful() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let resolved = add_at(
        &file,
        "2026-07-09T18:30:00Z",
        "Cache configuration missing",
        &["ops"],
    );
    let _: SuccessEnvelope<ResolveData> = resolve_at(
        &file,
        "2026-07-09T18:31:00Z",
        resolved.data.record.cut_id(),
        &["--agent", "fixer"],
    );
    add_at(
        &file,
        "2026-07-09T18:32:00Z",
        "Deploy credentials expired",
        &["ops"],
    );
    let before = std::fs::read(&file).unwrap();

    let verify = verify_success(&run_file(&file, &["verify"]), 0);
    assert_eq!(
        verify.data,
        json!({"recurrences": [], "count": 0, "scanned": 1})
    );
    assert_eq!(std::fs::read(&file).unwrap(), before);
}

#[test]
fn verify_reports_an_exact_title_recurrence_with_the_full_envelope() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let resolved = add_at(
        &file,
        "2026-07-09T18:30:00Z",
        "Cache configuration missing",
        &["alpha"],
    );
    let _: SuccessEnvelope<ResolveData> = resolve_at(
        &file,
        "2026-07-09T18:31:00Z",
        resolved.data.record.cut_id(),
        &[
            "--agent",
            "fixer",
            "--task",
            "TASK-VERIFY",
            "--pr",
            "https://github.com/BigCactusLabs/blotter/pull/99",
            "--commit",
            "abc123",
        ],
    );
    let recurring = add_at(
        &file,
        "2026-07-09T18:32:00Z",
        "cache-configuration missing!",
        &["beta"],
    );

    let verify = verify_success(&run_file(&file, &["verify"]), 1);
    let file = file.to_string_lossy().into_owned();
    assert_eq!(
        serde_json::to_value(verify).unwrap(),
        json!({
            "ok": true,
            "data": {
                "recurrences": [{
                    "resolved_id": resolved.data.record.cut_id(),
                    "resolved_text": "Cache configuration missing",
                    "resolution": {
                        "ts": "2026-07-09T18:31:00.000Z",
                        "task": "TASK-VERIFY",
                        "pr": "https://github.com/BigCactusLabs/blotter/pull/99",
                        "commit": "abc123",
                    },
                    "recurrence_ids": [recurring.data.record.cut_id()],
                    "count": 1,
                    "first_recurrence_ts": "2026-07-09T18:32:00.000Z",
                }],
                "count": 1,
                "scanned": 1,
            },
            "meta": {"contract": 5, "file": file},
        })
    );
}

#[test]
fn verify_excludes_an_open_cut_that_predates_the_resolution() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let resolved = add_at(
        &file,
        "2026-07-09T18:30:00Z",
        "Cache configuration missing",
        &["ops"],
    );
    add_at(
        &file,
        "2026-07-09T18:31:00Z",
        "cache configuration missing",
        &["ops"],
    );
    let _: SuccessEnvelope<ResolveData> = resolve_at(
        &file,
        "2026-07-09T18:32:00Z",
        resolved.data.record.cut_id(),
        &["--agent", "fixer"],
    );

    let verify = verify_success(&run_file(&file, &["verify"]), 0);
    assert_eq!(verify.data["recurrences"], json!([]));
    assert_eq!(verify.data["scanned"], 1);
}

#[test]
fn verify_excludes_a_dropped_resolution_anchor() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let resolved = add_at(
        &file,
        "2026-07-09T18:30:00Z",
        "Cache configuration missing",
        &["ops"],
    );
    let original = std::fs::read_to_string(&file).unwrap();
    let dropped = json!({
        "kind": "resolve",
        "id": resolved.data.record.cut_id(),
        "ts": "2026-07-09T18:31:00.000Z",
        "agent": "fixture",
        "note": null,
        "dropped": true,
    });
    std::fs::write(&file, format!("{original}{dropped}\n")).unwrap();
    add_at(
        &file,
        "2026-07-09T18:32:00Z",
        "cache configuration missing",
        &["ops"],
    );

    let verify = verify_success(&run_file(&file, &["verify"]), 0);
    assert_eq!(verify.data["recurrences"], json!([]));
    assert_eq!(verify.data["scanned"], 1);
}

#[test]
fn verify_excludes_an_empty_normalized_resolved_title() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let resolved = add_at(&file, "2026-07-09T18:30:00Z", "!!!", &[]);
    let _: SuccessEnvelope<ResolveData> = resolve_at(
        &file,
        "2026-07-09T18:31:00Z",
        resolved.data.record.cut_id(),
        &["--agent", "fixer"],
    );
    add_at(&file, "2026-07-09T18:32:00Z", "???", &[]);

    let verify = verify_success(&run_file(&file, &["verify"]), 0);
    assert_eq!(verify.data["recurrences"], json!([]));
    assert_eq!(verify.data["scanned"], 1);
}

#[test]
fn verify_links_tagged_near_duplicates() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let resolved = add_at(
        &file,
        "2026-07-09T18:30:00Z",
        "Cache restore endpoint returns an error",
        &["ops"],
    );
    let _: SuccessEnvelope<ResolveData> = resolve_at(
        &file,
        "2026-07-09T18:31:00Z",
        resolved.data.record.cut_id(),
        &["--agent", "fixer"],
    );
    let recurring = add_at(
        &file,
        "2026-07-09T18:32:00Z",
        "Cache restore endpoint still returns an error",
        &["ops"],
    );

    let verify = verify_success(&run_file(&file, &["verify"]), 1);
    assert_eq!(verify.data["count"], 1);
    assert_eq!(
        verify.data["recurrences"][0]["recurrence_ids"],
        json!([recurring.data.record.cut_id()])
    );
}

#[test]
fn verify_ignores_dogear_noise() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let resolved = add_at(
        &file,
        "2026-07-09T18:30:00Z",
        "Cache configuration missing",
        &["alpha"],
    );
    let _: SuccessEnvelope<ResolveData> = resolve_at(
        &file,
        "2026-07-09T18:31:00Z",
        resolved.data.record.cut_id(),
        &["--agent", "fixer"],
    );
    let recurring = add_at(
        &file,
        "2026-07-09T18:32:00Z",
        "cache configuration missing",
        &["beta"],
    );
    dogear_at(
        &file,
        "2026-07-09T18:33:00Z",
        "cache configuration missing",
        &["research"],
    );

    let verify = verify_success(&run_file(&file, &["verify"]), 1);
    assert_eq!(verify.data["scanned"], 1);
    assert_eq!(
        verify.data["recurrences"][0]["recurrence_ids"],
        json!([recurring.data.record.cut_id()])
    );
}

#[test]
fn verify_uses_the_materialized_amend_resolution_timestamp() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let resolved = add_at(
        &file,
        "2026-07-09T18:30:00Z",
        "Cache configuration missing",
        &["ops"],
    );
    let _: SuccessEnvelope<ResolveData> = resolve_at(
        &file,
        "2026-07-09T18:31:00Z",
        resolved.data.record.cut_id(),
        &["--agent", "fixer", "--note", "base"],
    );
    add_at(
        &file,
        "2026-07-09T18:32:00Z",
        "cache configuration missing",
        &["ops"],
    );
    let _: SuccessEnvelope<ResolveData> = resolve_at(
        &file,
        "2026-07-09T18:33:00Z",
        resolved.data.record.cut_id(),
        &["--amend", "--agent", "corrector", "--note", "corrected"],
    );
    let recurring = add_at(
        &file,
        "2026-07-09T18:34:00Z",
        "cache configuration missing",
        &["ops"],
    );

    let verify = verify_success(&run_file(&file, &["verify"]), 1);
    assert_eq!(verify.data["scanned"], 2);
    assert_eq!(
        verify.data["recurrences"][0]["resolution"]["ts"],
        "2026-07-09T18:33:00.000Z"
    );
    assert_eq!(
        verify.data["recurrences"][0]["recurrence_ids"],
        json!([recurring.data.record.cut_id()])
    );
}

#[test]
fn verify_allows_one_open_cut_to_recur_against_two_resolved_anchors() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let first = add_at(
        &file,
        "2026-07-09T18:30:00Z",
        "Cache configuration missing",
        &["alpha"],
    );
    let _: SuccessEnvelope<ResolveData> = resolve_at(
        &file,
        "2026-07-09T18:31:00Z",
        first.data.record.cut_id(),
        &["--agent", "first-fixer"],
    );
    let second = add_at(
        &file,
        "2026-07-09T18:32:00Z",
        "cache configuration missing",
        &["beta"],
    );
    let _: SuccessEnvelope<ResolveData> = resolve_at(
        &file,
        "2026-07-09T18:33:00Z",
        second.data.record.cut_id(),
        &["--agent", "second-fixer"],
    );
    let recurring = add_at(
        &file,
        "2026-07-09T18:34:00Z",
        "CACHE CONFIGURATION MISSING!",
        &["gamma"],
    );

    let verify = verify_success(&run_file(&file, &["verify"]), 1);
    assert_eq!(verify.data["count"], 2);
    assert_eq!(verify.data["scanned"], 1);
    let recurrences = verify.data["recurrences"].as_array().unwrap();
    let mut expected_resolved_ids = vec![
        first.data.record.cut_id().to_owned(),
        second.data.record.cut_id().to_owned(),
    ];
    expected_resolved_ids.sort();
    assert_eq!(
        recurrences
            .iter()
            .map(|recurrence| recurrence["resolved_id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        expected_resolved_ids
    );
    for recurrence in recurrences {
        assert_eq!(
            recurrence["recurrence_ids"],
            json!([recurring.data.record.cut_id()])
        );
        assert_eq!(recurrence["count"], 1);
        assert_eq!(
            recurrence["first_recurrence_ts"],
            "2026-07-09T18:34:00.000Z"
        );
    }
}

#[test]
fn verify_sorts_recurrence_ids_and_anchors_by_first_recurrence() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let first_anchor = add_at(
        &file,
        "2026-07-09T18:30:00Z",
        "Cache configuration missing",
        &["alpha"],
    );
    let _: SuccessEnvelope<ResolveData> = resolve_at(
        &file,
        "2026-07-09T18:31:00Z",
        first_anchor.data.record.cut_id(),
        &["--agent", "first-fixer"],
    );
    let second_anchor = add_at(
        &file,
        "2026-07-09T18:30:00Z",
        "Build cache checksum mismatch",
        &["build"],
    );
    let _: SuccessEnvelope<ResolveData> = resolve_at(
        &file,
        "2026-07-09T18:31:00Z",
        second_anchor.data.record.cut_id(),
        &["--agent", "second-fixer"],
    );
    let late_first = add_at(
        &file,
        "2026-07-09T18:35:00Z",
        "cache configuration missing",
        &["beta"],
    );
    let first = add_at(
        &file,
        "2026-07-09T18:33:00Z",
        "CACHE CONFIGURATION MISSING!",
        &["gamma"],
    );
    let second = add_at(
        &file,
        "2026-07-09T18:34:00Z",
        "build cache checksum mismatch",
        &["build"],
    );

    let verify = verify_success(&run_file(&file, &["verify"]), 1);
    assert_eq!(verify.data["count"], 2);
    assert_eq!(
        verify.data["recurrences"],
        json!([
            {
                "resolved_id": first_anchor.data.record.cut_id(),
                "resolved_text": "Cache configuration missing",
                "resolution": {"ts": "2026-07-09T18:31:00.000Z"},
                "recurrence_ids": [first.data.record.cut_id(), late_first.data.record.cut_id()],
                "count": 2,
                "first_recurrence_ts": "2026-07-09T18:33:00.000Z",
            },
            {
                "resolved_id": second_anchor.data.record.cut_id(),
                "resolved_text": "Build cache checksum mismatch",
                "resolution": {"ts": "2026-07-09T18:31:00.000Z"},
                "recurrence_ids": [second.data.record.cut_id()],
                "count": 1,
                "first_recurrence_ts": "2026-07-09T18:34:00.000Z",
            },
        ])
    );
}

#[test]
fn schema_documents_verify() {
    let schema: SuccessEnvelope<Value> = success(&run(&["schema"]));
    let verify = &schema.data["commands"]["verify"];
    assert!(
        verify["flags"]["--include-auto"]
            .as_str()
            .unwrap()
            .contains("include records tagged auto")
    );
    assert_eq!(
        verify["output"],
        "{recurrences:[{resolved_id,resolved_text,source?,resolution:{ts,task?,pr?,commit?},recurrence_ids,count,first_recurrence_ts}],count,scanned}"
    );
    assert!(
        verify["semantics"]
            .as_str()
            .unwrap()
            .contains("filtered-token rule")
    );
    assert_eq!(
        verify["exit_codes"],
        json!({"0":"no recurrences","1":"recurrences found"})
    );
    assert_eq!(verify["read_only"], true);
    assert_eq!(verify["appends"], false);
    assert_eq!(verify["destructive"], false);
}

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

#[test]
fn triage_stdout_is_byte_deterministic() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    for (now, text) in [
        (
            "2026-07-09T18:30:00Z",
            "Metadata cache missing during build",
        ),
        (
            "2026-07-09T18:31:00Z",
            "metadata-cache missing during build",
        ),
        (
            "2026-07-09T18:32:00Z",
            "metadata cache missing during build again",
        ),
    ] {
        add_at(&file, now, text, &["build"]);
    }

    let before = std::fs::read(&file).unwrap();
    let first = run_file(&file, &["triage"]);
    let second = run_file(&file, &["triage"]);
    let _: SuccessEnvelope<Value> = triage_success(&first, 1);
    let _: SuccessEnvelope<Value> = triage_success(&second, 1);
    assert_eq!(first.stdout, second.stdout);
    assert_eq!(std::fs::read(&file).unwrap(), before);
}

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
    assert!(
        digest["flags"]["--include-auto"]
            .as_str()
            .unwrap()
            .contains("include records tagged auto")
    );
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
                    "severity": "minor",
                    "cwd": alpha_cut.data.record.cut_cwd(),
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
                    "severity": "minor",
                    "cwd": beta_cut.data.record.cut_cwd(),
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
fn default_log_path_uses_repo_default_name() {
    let temp = TempDir::new().unwrap();
    assert_eq!(
        blotter::store::default_log_path(temp.path()),
        temp.path().join(".blotter.jsonl")
    );
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
    assert_eq!(sweep["flags"]["--since"], "full RFC3339|Nd|Nh; optional");
    assert_eq!(sweep["flags"]["--kind"], "cut|dogear|all; default cut");
    assert!(
        sweep["flags"]["--include-auto"]
            .as_str()
            .unwrap()
            .contains("include records tagged auto")
    );
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

fn hook_exec_claude_code(file: &Path, stdin: impl Into<Vec<u8>>) -> std::process::Output {
    command()
        .arg("--file")
        .arg(file)
        .args(["hook", "exec", "claude-code"])
        .write_stdin(stdin)
        .output()
        .unwrap()
}

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

fn hook_exec_is_silent(output: &std::process::Output) {
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
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

    hook_exec_is_silent(&hook_exec_claude_code(&file, payload));
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

// --- legacy record compatibility ---

#[test]
fn old_format_cuts_without_source_fold_and_list_byte_identically() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("old-format.jsonl");
    let id = compute_id(
        "2026-08-01T00:00:00.000Z",
        "legacy",
        "old-format cut",
        Severity::Minor,
        &["legacy".into()],
    );
    let stored = json!({
        "kind": "cut",
        "id": id,
        "ts": "2026-08-01T00:00:00.000Z",
        "agent": "legacy",
        "text": "old-format cut",
        "tags": ["legacy"],
        "severity": "minor",
        "cwd": "/tmp"
    });
    let stored_bytes = format!("{stored}\n");
    std::fs::write(&file, &stored_bytes).unwrap();

    let output = run_file(&file, &["list"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let file_json = serde_json::to_string(&file.to_string_lossy()).unwrap();
    let expected = format!(
        r#"{{"ok":true,"data":{{"items":[{{"kind":"cut","id":"{id}","ts":"2026-08-01T00:00:00.000Z","agent":"legacy","text":"old-format cut","tags":["legacy"],"severity":"minor","cwd":"/tmp","status":"open"}}],"count":1,"total":1,"truncated":false}},"meta":{{"contract":5,"file":{file_json}}}}}"#
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("{expected}\n")
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), stored_bytes);
}

#[test]
fn unknown_stored_source_round_trips_through_fold_and_list() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("unknown-source.jsonl");
    let id = compute_id(
        "2026-08-01T00:00:00.000Z",
        "future",
        "future source",
        Severity::Minor,
        &[],
    );
    let stored = json!({
        "kind": "cut",
        "id": id,
        "ts": "2026-08-01T00:00:00.000Z",
        "agent": "future",
        "text": "future source",
        "tags": [],
        "severity": "minor",
        "cwd": "/tmp",
        "source": "other"
    });
    let stored_bytes = format!("{stored}\n");
    std::fs::write(&file, &stored_bytes).unwrap();

    let listed: SuccessEnvelope<Value> = success(&run_file(&file, &["list"]));
    assert_eq!(listed.data["items"][0]["source"], "other");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), stored_bytes);
}

#[test]
fn source_propagates_to_triage_digest_and_verify_json() {
    let temp = TempDir::new().unwrap();
    let analysis_file = temp.path().join("analysis-source.jsonl");
    let analysis_text = "source provenance analysis";
    let first_ts = "2026-07-09T18:29:00.000Z";
    let second_ts = "2026-07-09T18:30:00.000Z";
    let tags = vec!["provenance".into()];
    let first = json!({
        "kind": "cut",
        "id": compute_id(first_ts, "hook", analysis_text, Severity::Minor, &tags),
        "ts": first_ts,
        "agent": "hook",
        "text": analysis_text,
        "tags": tags,
        "severity": "minor",
        "cwd": "/tmp",
        "source": "hook"
    });
    let second = json!({
        "kind": "cut",
        "id": compute_id(second_ts, "hook", analysis_text, Severity::Minor, &tags),
        "ts": second_ts,
        "agent": "hook",
        "text": analysis_text,
        "tags": tags,
        "severity": "minor",
        "cwd": "/tmp",
        "source": "hook"
    });
    std::fs::write(&analysis_file, format!("{first}\n{second}\n")).unwrap();

    let triage = triage_success(
        &run_file(&analysis_file, &["triage", "--min-count", "2"]),
        1,
    );
    assert_eq!(triage.data["clusters"][0]["source"], "hook");
    let digest: SuccessEnvelope<Value> =
        success(&run_file(&analysis_file, &["digest", "--since", "1d"]));
    assert_eq!(digest.data["chronic"][0]["source"], "hook");

    let verify_file = temp.path().join("verify-source.jsonl");
    let resolved_text = "source provenance recurrence";
    let resolved_ts = "2026-07-09T16:00:00.000Z";
    let recurrence_ts = "2026-07-09T16:20:00.000Z";
    let resolved_id = compute_id(resolved_ts, "hook", resolved_text, Severity::Minor, &[]);
    let recurrence_id = compute_id(
        recurrence_ts,
        "self-report",
        resolved_text,
        Severity::Minor,
        &[],
    );
    let resolved = json!({
        "kind": "cut",
        "id": resolved_id,
        "ts": resolved_ts,
        "agent": "hook",
        "text": resolved_text,
        "tags": [],
        "severity": "minor",
        "cwd": "/tmp",
        "source": "hook"
    });
    let resolution = json!({
        "kind": "resolve",
        "id": resolved_id,
        "ts": "2026-07-09T16:10:00.000Z",
        "agent": "tester",
        "note": null
    });
    let recurrence = json!({
        "kind": "cut",
        "id": recurrence_id,
        "ts": recurrence_ts,
        "agent": "self-report",
        "text": resolved_text,
        "tags": [],
        "severity": "minor",
        "cwd": "/tmp"
    });
    std::fs::write(
        &verify_file,
        format!("{resolved}\n{resolution}\n{recurrence}\n"),
    )
    .unwrap();

    let verify = verify_success(&run_file(&verify_file, &["verify"]), 1);
    assert_eq!(verify.data["recurrences"][0]["source"], "hook");
}

#[test]
fn hook_dedupe_matches_open_commands_regardless_of_source() {
    for source in [None, Some("hook")] {
        let temp = TempDir::new().unwrap();
        let file = temp.path().join("dedupe-source.jsonl");
        let command = "cargo test --workspace";
        let id = compute_id(
            NOW,
            "existing",
            command,
            Severity::Minor,
            &["auto".into(), "claude-code".into()],
        );
        let mut existing = json!({
            "kind": "cut",
            "id": id,
            "ts": NOW,
            "agent": "existing",
            "text": command,
            "tags": ["auto", "claude-code"],
            "severity": "minor",
            "cwd": "/tmp",
            "evidence": {"cmd": command}
        });
        if let Some(source) = source {
            existing["source"] = json!(source);
        }
        std::fs::write(&file, format!("{existing}\n")).unwrap();

        hook_exec_is_silent(&hook_exec_claude_code(
            &file,
            claude_bash_failure(command, temp.path()).to_string(),
        ));
        assert_eq!(
            std::fs::read_to_string(&file).unwrap().lines().count(),
            1,
            "source={source:?} must not affect open-command dedupe"
        );
    }
}

#[test]
fn legacy_migration_inputs_are_ignored_without_warnings() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("repo");
    let nested = root.join("nested");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::create_dir(root.join(".git")).unwrap();
    std::fs::write(root.join(".papercuts.jsonl"), "").unwrap();
    let canonical_root = root.canonicalize().unwrap();

    let output: SuccessEnvelope<AddData> = success(
        &command()
            .current_dir(&nested)
            .env_remove("BLOTTER_FILE")
            .env_remove("BLOTTER_AGENT")
            .env("PAPERCUTS_FILE", temp.path().join("legacy-env.jsonl"))
            .env("PAPERCUTS_AGENT", "legacy-agent")
            .env("PAPERCUTS_NOW", NOW)
            .args(["add", "stale environment", "--dry-run"])
            .output()
            .unwrap(),
    );
    assert_eq!(output.data.record.cut_agent(), "unknown");
    assert_eq!(output.data.record.cut_ts(), "2026-07-09T18:30:00.123Z");
    assert_eq!(
        output.meta.file.as_deref(),
        Some(canonical_root.join(".blotter.jsonl").to_str().unwrap())
    );
    assert_eq!(output.meta.warnings, ["dry run; no record appended"]);
}

#[test]
fn legacy_pc_records_still_fold_and_list() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("legacy.jsonl");
    let cut = json!({
        "kind": "cut",
        "id": "pc_a1b2c3d4e5f6",
        "ts": "2026-07-09T00:00:00.000Z",
        "agent": "legacy",
        "text": "legacy cut",
        "tags": ["old"],
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
        "tags": ["old"],
        "cwd": "/tmp",
        "repo": "/tmp/repo"
    });
    std::fs::write(&file, format!("{cut}\n{dogear}\n")).unwrap();

    let listed: SuccessEnvelope<ListData> = success(&run_file(
        &file,
        &["list", "--kind", "all", "--status", "all"],
    ));
    assert_eq!(listed.data.items.len(), 2);
    assert_eq!(
        listed
            .data
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["pc_a1b2c3d4e5f6", "pc_b1c2d3e4f5a6"]
    );
}

#[test]
fn phase_2a_resolve_is_namespace_aware() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("mixed-namespaces.jsonl");
    let pc = "pc_a1b2c3d4e5f6";
    let bl = "bl_b1c2d3e4f5a6";
    let bare_pc = "pc_cafe12345678";
    let pc_collision = "pc_dead00000000";
    let bl_collision = "bl_dead11111111";
    let fixture = [pc, bl, bare_pc, pc_collision, bl_collision]
        .into_iter()
        .map(|id| {
            json!({
                "kind": "cut",
                "id": id,
                "ts": "2026-07-09T00:00:00.000Z",
                "agent": "legacy",
                "text": id,
                "tags": [],
                "severity": "minor",
                "cwd": "/tmp",
                "repo": null
            })
            .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&file, format!("{fixture}\n")).unwrap();

    let pc_input = pc.to_ascii_uppercase();
    let resolved_pc: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &["resolve", &pc_input, "--agent", "fixer"],
    ));
    assert!(resolved_pc.data.changed);
    assert_eq!(resolved_pc.data.records.len(), 1);
    assert_eq!(resolved_pc.data.records[0].id, pc);

    let bl_input = bl.to_ascii_uppercase();
    let resolved_bl: SuccessEnvelope<ResolveData> = success(&run_file(
        &file,
        &["resolve", &bl_input, "--agent", "fixer"],
    ));
    assert!(resolved_bl.data.changed);
    assert_eq!(resolved_bl.data.records.len(), 1);
    assert_eq!(resolved_bl.data.records[0].id, bl);

    let resolved_bare: SuccessEnvelope<ResolveData> =
        success(&run_file(&file, &["resolve", "cafe", "--agent", "fixer"]));
    assert!(resolved_bare.data.changed);
    assert_eq!(resolved_bare.data.records.len(), 1);
    assert_eq!(resolved_bare.data.records[0].id, bare_pc);

    let ambiguous = error(
        &run_file(&file, &["resolve", "dead", "--agent", "fixer"]),
        65,
        "ambiguous_id",
    );
    assert_eq!(
        ambiguous.error.details["candidates"],
        json!([bl_collision, pc_collision])
    );
}

#[test]
fn doctor_accepts_legacy_pc_records_without_a_migration_field() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("mixed-log.jsonl");
    add(&file, "current record");
    let legacy_cut = json!({
        "kind": "cut",
        "id": "pc_000000000000",
        "ts": "2026-07-09T00:00:00.000Z",
        "agent": "legacy",
        "text": "legacy cut",
        "tags": [],
        "severity": "minor",
        "cwd": "/tmp",
        "repo": null
    });
    let legacy_dogear = json!({
        "kind": "dogear",
        "id": "pc_11111111111111111111",
        "ts": "2026-07-09T00:00:00.000Z",
        "agent": "legacy",
        "text": "legacy dogear",
        "tags": [],
        "cwd": "/tmp",
        "repo": null
    });
    let mut writer = OpenOptions::new().append(true).open(&file).unwrap();
    writeln!(writer, "{legacy_cut}").unwrap();
    writeln!(writer, "{legacy_dogear}").unwrap();
    drop(writer);

    let output = run_file(&file, &["doctor"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let doctor: SuccessEnvelope<DoctorData> = serde_json::from_slice(&output.stdout).unwrap();
    assert!(doctor.data.healthy, "findings: {:?}", doctor.data.findings);
    assert!(doctor.data.findings.is_empty());
    assert_eq!(doctor.data.checked_lines, 3);

    let schema: SuccessEnvelope<Value> = success(&run(&["schema"]));
    assert_eq!(
        schema.data["commands"]["doctor"]["output"],
        "{healthy,findings:[{line,kind,message,fixable}],checked_lines,fix?:{changed,applied:[{line,kind,action}],backup?,quarantine?,restore_hint?,dry_run}}"
    );
    assert_eq!(schema.data["commands"]["doctor"]["read_only"], true);
    assert_eq!(
        schema.data["commands"]["doctor"]["fix"]["destructive"],
        true
    );
    assert_eq!(schema.data["commands"]["doctor"]["fix"]["read_only"], false);
    assert_eq!(
        schema.data["id"]["accepted_prefixes"],
        json!(["bl_", "pc_"])
    );
}

// --- phase 2b: hook stale-path repair ---

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
fn list_hides_auto_captures_and_composes_with_selectors() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let hand_first = add_at(
        &file,
        "2026-07-09T17:00:00Z",
        "Hand-filed first cut",
        &["manual"],
    );
    let hand_second = add_at(
        &file,
        "2026-07-09T17:01:00Z",
        "Hand-filed second cut",
        &["manual"],
    );
    let auto_first = add_at(
        &file,
        "2026-07-09T17:02:00Z",
        "Auto current first cut",
        &["auto", "claude-code"],
    );
    let auto_second = add_at(
        &file,
        "2026-07-09T17:03:00Z",
        "Auto current second cut",
        &["auto"],
    );
    let auto_old = add_at(&file, "2026-07-01T17:00:00Z", "Auto old cut", &["auto"]);
    let auto_resolved = add_at(
        &file,
        "2026-07-09T17:04:00Z",
        "Auto resolved cut",
        &["auto"],
    );
    let _: SuccessEnvelope<ResolveData> = resolve_at(
        &file,
        "2026-07-09T17:05:00Z",
        auto_resolved.data.record.cut_id(),
        &["--note", "handled"],
    );
    let auto_dogear = dogear_at(&file, "2026-07-09T17:06:00Z", "Auto dogear", &["auto"]);
    let auto_dogear_id = auto_dogear.data["record"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let hand_dogear = dogear_at(
        &file,
        "2026-07-09T17:07:00Z",
        "Hand-filed dogear",
        &["manual"],
    );
    let hand_dogear_id = hand_dogear.data["record"]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let default: SuccessEnvelope<ListData> = success(&run_file(&file, &["list"]));
    assert_eq!(default.data.count, 2);
    assert_eq!(default.data.total, 2);
    assert!(!default.data.truncated);
    assert_eq!(
        default.meta.warnings,
        ["3 auto-captured records hidden; use --include-auto to include them"]
    );
    assert!(
        default
            .data
            .items
            .iter()
            .all(|item| !item.tags.iter().any(|tag| tag == "auto"))
    );

    let included: SuccessEnvelope<ListData> =
        success(&run_file(&file, &["list", "--include-auto"]));
    assert_eq!(included.data.count, 5);
    assert_eq!(included.data.total, 5);
    assert!(!included.data.truncated);
    assert!(included.meta.warnings.is_empty());

    let tagged: SuccessEnvelope<ListData> = success(&run_file(&file, &["list", "--tag", "auto"]));
    assert_eq!(tagged.data.count, 3);
    assert_eq!(tagged.data.total, 3);
    assert!(tagged.meta.warnings.is_empty());
    assert!(
        tagged
            .data
            .items
            .iter()
            .all(|item| item.tags.iter().any(|tag| tag == "auto"))
    );

    let limited: SuccessEnvelope<ListData> = success(&run_file(&file, &["list", "--limit", "1"]));
    assert_eq!(limited.data.count, 1);
    assert_eq!(limited.data.total, 2);
    assert!(limited.data.truncated);
    assert_eq!(
        limited.meta.warnings,
        ["3 auto-captured records hidden; use --include-auto to include them"]
    );

    let all_statuses: SuccessEnvelope<ListData> =
        success(&run_file(&file, &["list", "--status", "all"]));
    assert_eq!(all_statuses.data.total, 2);
    assert_eq!(
        all_statuses.meta.warnings,
        ["4 auto-captured records hidden; use --include-auto to include them"]
    );
    assert!(
        !all_statuses
            .data
            .items
            .iter()
            .any(|item| item.id == auto_resolved.data.record.cut_id())
    );
    let all_statuses_included: SuccessEnvelope<ListData> = success(&run_file(
        &file,
        &["list", "--status", "all", "--include-auto"],
    ));
    assert_eq!(all_statuses_included.data.total, 6);
    assert!(
        all_statuses_included
            .data
            .items
            .iter()
            .any(|item| item.id == auto_resolved.data.record.cut_id())
    );

    let dogears: SuccessEnvelope<ListData> = success(&run_file(
        &file,
        &["list", "--kind", "dogear", "--include-auto"],
    ));
    assert_eq!(dogears.data.total, 2);
    assert!(
        dogears
            .data
            .items
            .iter()
            .any(|item| item.id == auto_dogear_id)
    );
    assert!(
        dogears
            .data
            .items
            .iter()
            .any(|item| item.id == hand_dogear_id)
    );
    let cuts: SuccessEnvelope<ListData> = success(&run_file(
        &file,
        &["list", "--kind", "cut", "--include-auto"],
    ));
    assert!(cuts.data.items.iter().all(|item| item.id != auto_dogear_id));

    let recent: SuccessEnvelope<ListData> = success(&run_file(&file, &["list", "--since", "1d"]));
    assert_eq!(recent.data.total, 2);
    assert_eq!(
        recent.meta.warnings,
        ["2 auto-captured records hidden; use --include-auto to include them"]
    );
    let recent_included: SuccessEnvelope<ListData> = success(&run_file(
        &file,
        &["list", "--since", "1d", "--include-auto"],
    ));
    assert_eq!(recent_included.data.total, 4);
    assert!(
        !recent_included
            .data
            .items
            .iter()
            .any(|item| item.id == auto_old.data.record.cut_id())
    );

    let hand_only: SuccessEnvelope<ListData> =
        success(&run_file(&file, &["list", "--tag", "manual"]));
    assert!(hand_only.meta.warnings.is_empty());
    assert_eq!(hand_only.data.total, 2);

    let mut writer = OpenOptions::new().append(true).open(&file).unwrap();
    writeln!(writer, "not json").unwrap();
    drop(writer);
    let warned: SuccessEnvelope<ListData> = success(&run_file(&file, &["list", "--since", "1d"]));
    assert_eq!(
        warned.meta.warnings,
        [
            "skipped 1 malformed line",
            "2 auto-captured records hidden; use --include-auto to include them",
        ]
    );
    let markdown = run_file(&file, &["list", "--since", "1d", "--format", "md"]);
    assert!(markdown.status.success());
    assert!(markdown.stderr.is_empty());
    assert!(String::from_utf8(markdown.stdout).unwrap().ends_with(
        "> note: skipped 1 malformed line\n> note: 2 auto-captured records hidden; use --include-auto to include them\n"
    ));

    assert!(
        included
            .data
            .items
            .iter()
            .any(|item| item.id == hand_first.data.record.cut_id())
    );
    assert!(
        included
            .data
            .items
            .iter()
            .any(|item| item.id == hand_second.data.record.cut_id())
    );
    assert!(
        included
            .data
            .items
            .iter()
            .any(|item| item.id == auto_first.data.record.cut_id())
    );
    assert!(
        included
            .data
            .items
            .iter()
            .any(|item| item.id == auto_second.data.record.cut_id())
    );
}

#[test]
fn triage_hides_auto_only_clusters_until_requested() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    add_at(
        &file,
        "2026-07-09T17:00:00Z",
        "Auto-only chronic cluster",
        &["auto"],
    );
    add_at(
        &file,
        "2026-07-09T17:01:00Z",
        "Auto-only chronic cluster",
        &["auto"],
    );

    let hidden = triage_success(&run_file(&file, &["triage", "--min-count", "2"]), 0);
    assert_eq!(hidden.data["scanned"], 0);
    assert_eq!(hidden.data["count"], 0);
    assert_eq!(
        hidden.meta.warnings,
        ["2 auto-captured records hidden; use --include-auto to include them"]
    );

    let included = triage_success(
        &run_file(&file, &["triage", "--min-count", "2", "--include-auto"]),
        1,
    );
    assert_eq!(included.data["scanned"], 2);
    assert_eq!(included.data["count"], 1);
    assert!(included.meta.warnings.is_empty());
}

#[test]
fn verify_hides_auto_anchors_and_recurrence_evidence() {
    let temp = TempDir::new().unwrap();
    let auto_recurrence_file = temp.path().join("auto-recurrence.jsonl");
    let hand_anchor = add_at(
        &auto_recurrence_file,
        "2026-07-09T16:00:00Z",
        "Dependency metadata unavailable",
        &["build"],
    );
    let _: SuccessEnvelope<ResolveData> = resolve_at(
        &auto_recurrence_file,
        "2026-07-09T16:05:00Z",
        hand_anchor.data.record.cut_id(),
        &["--note", "fixed"],
    );
    add_at(
        &auto_recurrence_file,
        "2026-07-09T17:00:00Z",
        "Dependency metadata unavailable",
        &["auto", "build"],
    );

    let hidden = verify_success(&run_file(&auto_recurrence_file, &["verify"]), 0);
    assert_eq!(hidden.data["scanned"], 0);
    assert_eq!(hidden.data["count"], 0);
    assert_eq!(
        hidden.meta.warnings,
        ["1 auto-captured record hidden; use --include-auto to include them"]
    );
    let included = verify_success(
        &run_file(&auto_recurrence_file, &["verify", "--include-auto"]),
        1,
    );
    assert_eq!(included.data["scanned"], 1);
    assert_eq!(included.data["count"], 1);

    let auto_anchor_file = temp.path().join("auto-anchor.jsonl");
    let auto_anchor = add_at(
        &auto_anchor_file,
        "2026-07-09T16:00:00Z",
        "Repository index unavailable",
        &["auto", "build"],
    );
    let _: SuccessEnvelope<ResolveData> = resolve_at(
        &auto_anchor_file,
        "2026-07-09T16:05:00Z",
        auto_anchor.data.record.cut_id(),
        &["--note", "fixed"],
    );
    add_at(
        &auto_anchor_file,
        "2026-07-09T17:00:00Z",
        "Repository index unavailable",
        &["build"],
    );

    let hidden_anchor = verify_success(&run_file(&auto_anchor_file, &["verify"]), 0);
    assert_eq!(hidden_anchor.data["scanned"], 1);
    assert_eq!(hidden_anchor.data["count"], 0);
    assert_eq!(
        hidden_anchor.meta.warnings,
        ["1 auto-captured record hidden; use --include-auto to include them"]
    );
    let included_anchor = verify_success(
        &run_file(&auto_anchor_file, &["verify", "--include-auto"]),
        1,
    );
    assert_eq!(included_anchor.data["scanned"], 1);
    assert_eq!(included_anchor.data["count"], 1);
}

#[test]
fn verify_hidden_warning_counts_only_eligible_auto_cuts() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let dropped = add_at(
        &file,
        "2026-07-09T16:00:00Z",
        "Dropped auto anchor",
        &["auto"],
    );
    let _: SuccessEnvelope<ResolveData> = resolve_at(
        &file,
        "2026-07-09T16:01:00Z",
        dropped.data.record.cut_id(),
        &["--note", "initial resolution"],
    );
    let dropped_amend = LogEvent::Resolve {
        id: dropped.data.record.cut_id().to_owned(),
        ts: "2026-07-09T16:02:00Z".into(),
        agent: "fixture".into(),
        note: None,
        task: None,
        pr: None,
        commit: None,
        url: None,
        dropped: true,
        amend: true,
    };
    let mut log = OpenOptions::new().append(true).open(&file).unwrap();
    writeln!(log, "{}", serde_json::to_string(&dropped_amend).unwrap()).unwrap();

    let empty = add_at(&file, "2026-07-09T16:03:00Z", "!!!", &["auto"]);
    let _: SuccessEnvelope<ResolveData> = resolve_at(
        &file,
        "2026-07-09T16:04:00Z",
        empty.data.record.cut_id(),
        &["--note", "fixed"],
    );

    let hidden_ineligible = verify_success(&run_file(&file, &["verify"]), 0);
    assert_eq!(hidden_ineligible.data["scanned"], 0);
    assert_eq!(hidden_ineligible.data["count"], 0);
    assert!(hidden_ineligible.meta.warnings.is_empty());

    let eligible = add_at(
        &file,
        "2026-07-09T16:05:00Z",
        "Eligible auto anchor",
        &["auto"],
    );
    let _: SuccessEnvelope<ResolveData> = resolve_at(
        &file,
        "2026-07-09T16:06:00Z",
        eligible.data.record.cut_id(),
        &["--note", "fixed"],
    );

    let hidden_eligible = verify_success(&run_file(&file, &["verify"]), 0);
    assert_eq!(hidden_eligible.data["scanned"], 0);
    assert_eq!(hidden_eligible.data["count"], 0);
    assert_eq!(
        hidden_eligible.meta.warnings,
        ["1 auto-captured record hidden; use --include-auto to include them"]
    );
}

#[test]
fn digest_hides_auto_captures_from_every_section_and_markdown() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    add_at(
        &file,
        "2026-07-09T17:00:00Z",
        "Hand-filed digest cut",
        &["manual"],
    );
    add_at(
        &file,
        "2026-07-09T17:01:00Z",
        "Auto digest chronic cut",
        &["auto", "claude-code"],
    );
    add_at(
        &file,
        "2026-07-09T17:02:00Z",
        "Auto digest chronic cut",
        &["auto", "claude-code"],
    );
    dogear_at(
        &file,
        "2026-07-09T17:03:00Z",
        "Hand-filed digest dogear",
        &["manual"],
    );
    dogear_at(
        &file,
        "2026-07-09T17:04:00Z",
        "Auto digest dogear",
        &["auto"],
    );

    let hidden: SuccessEnvelope<Value> = success(&run_file(&file, &["digest"]));
    assert_eq!(hidden.data["chronic"], json!([]));
    assert_eq!(hidden.data["new_cuts"]["count"], 1);
    assert_eq!(
        hidden.data["new_cuts"]["by_tag"].as_array().unwrap().len(),
        1
    );
    assert_eq!(hidden.data["new_cuts"]["by_tag"][0]["tag"], "manual");
    assert_eq!(hidden.data["new_cuts"]["by_tag"][0]["count"], 1);
    assert_eq!(hidden.data["open_dogears"]["count"], 1);
    assert_eq!(
        hidden.data["open_dogears"]["items"][0]["text"],
        "Hand-filed digest dogear"
    );
    assert_eq!(
        hidden.meta.warnings,
        ["3 auto-captured records hidden; use --include-auto to include them"]
    );

    let included: SuccessEnvelope<Value> = success(&run_file(&file, &["digest", "--include-auto"]));
    assert_eq!(included.data["chronic"].as_array().unwrap().len(), 1);
    assert_eq!(included.data["new_cuts"]["count"], 3);
    assert!(
        included.data["new_cuts"]["by_tag"]
            .as_array()
            .unwrap()
            .iter()
            .any(|group| group["tag"] == "auto" && group["count"] == 2)
    );
    assert_eq!(included.data["open_dogears"]["count"], 2);
    assert!(included.meta.warnings.is_empty());

    let markdown = run_file(&file, &["digest", "--format", "md"]);
    assert!(markdown.status.success());
    assert!(markdown.stderr.is_empty());
    assert!(
        String::from_utf8(markdown.stdout).unwrap().ends_with(
            "> note: 3 auto-captured records hidden; use --include-auto to include them\n"
        )
    );
}

#[test]
fn sweep_hides_auto_captures_with_one_aggregate_warning() {
    let temp = TempDir::new().unwrap();
    let alpha = temp.path().join("alpha");
    let beta = temp.path().join("beta");
    make_repo(&alpha);
    make_repo(&beta);
    let alpha_file = alpha.join(".blotter.jsonl");
    let beta_file = beta.join(".blotter.jsonl");
    add_at(
        &alpha_file,
        "2026-07-09T17:00:00Z",
        "Hand-filed sweep cut",
        &["manual"],
    );
    add_at(
        &alpha_file,
        "2026-07-09T17:01:00Z",
        "Auto alpha sweep cut",
        &["auto", "claude-code"],
    );
    dogear_at(
        &alpha_file,
        "2026-07-09T17:02:00Z",
        "Auto alpha sweep dogear",
        &["auto"],
    );
    add_at(
        &beta_file,
        "2026-07-09T17:03:00Z",
        "Auto beta sweep cut",
        &["auto"],
    );

    let hidden: SuccessEnvelope<SweepData> = success(
        &command()
            .arg("sweep")
            .arg(&beta)
            .arg(&alpha)
            .arg(&alpha)
            .output()
            .unwrap(),
    );
    assert_eq!(hidden.data.repos.len(), 2);
    assert_eq!(hidden.data.totals.open_cuts, 1);
    assert_eq!(hidden.data.totals.open_dogears, 0);
    assert!(hidden.data.repos.iter().all(|repo| {
        repo.by_tag
            .iter()
            .all(|group| group.tag != "auto" && group.tag != "claude-code")
    }));
    assert_eq!(
        hidden.meta.warnings,
        ["3 auto-captured records hidden; use --include-auto to include them"]
    );

    let included: SuccessEnvelope<SweepData> = success(
        &command()
            .arg("sweep")
            .arg(&alpha)
            .arg(&beta)
            .arg("--include-auto")
            .output()
            .unwrap(),
    );
    assert_eq!(included.data.totals.open_cuts, 3);
    assert_eq!(included.data.totals.open_dogears, 1);
    assert!(included.data.repos.iter().any(|repo| {
        repo.by_tag
            .iter()
            .any(|group| group.tag == "auto" && group.count == 1)
    }));
    assert!(included.meta.warnings.is_empty());

    let dogears: SuccessEnvelope<SweepData> = success(
        &command()
            .arg("sweep")
            .arg(&alpha)
            .args(["--kind", "dogear", "--include-auto"])
            .output()
            .unwrap(),
    );
    assert_eq!(dogears.data.repos[0].items.len(), 1);
    assert_eq!(dogears.data.repos[0].items[0].kind, "dogear");
}

#[test]
fn sweep_caps_auto_records_when_included() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path().join("repo");
    make_repo(&repo);
    let file = repo.join(".blotter.jsonl");
    for number in 0..51 {
        dogear_at(
            &file,
            "2026-07-09T17:00:00Z",
            &format!("Auto capped dogear {number}"),
            &["auto"],
        );
    }

    let sweep: SuccessEnvelope<SweepData> = success(
        &command()
            .arg("sweep")
            .arg(&repo)
            .args(["--kind", "dogear", "--include-auto"])
            .output()
            .unwrap(),
    );
    assert_eq!(sweep.data.repos[0].counts.open_dogears, 51);
    assert_eq!(sweep.data.repos[0].items.len(), 50);
    assert!(sweep.data.repos[0].truncated);
    assert_eq!(sweep.data.repos[0].by_tag.len(), 1);
    assert_eq!(sweep.data.repos[0].by_tag[0].tag, "auto");
    assert_eq!(sweep.data.repos[0].by_tag[0].count, 51);
}

#[test]
fn auto_capture_contract_is_discoverable_in_schema_and_help() {
    let schema: SuccessEnvelope<Value> = success(&run(&["schema"]));
    assert_eq!(schema.meta.contract, 5);
    assert_eq!(schema.data["contract"], 5);
    for command in ["list", "triage", "digest", "verify", "sweep"] {
        assert!(
            schema.data["commands"][command]["flags"]["--include-auto"]
                .as_str()
                .unwrap()
                .contains("include records tagged auto")
        );
        assert!(
            schema.data["commands"][command]["semantics"]
                .as_str()
                .unwrap()
                .contains("records tagged auto are excluded by default")
        );

        let help = run(&[command, "--help"]);
        assert!(help.status.success());
        assert!(help.stderr.is_empty());
        assert!(
            String::from_utf8(help.stdout)
                .unwrap()
                .contains("--include-auto")
        );
    }
    assert!(
        schema.data["commands"]["list"]["semantics"]
            .as_str()
            .unwrap()
            .contains("--tag auto implies --include-auto")
    );
}

#[test]
fn resolve_guidance_includes_auto_captures() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    add(&file, "Present cut for not-found guidance");
    let invalid = error(
        &run_file(&file, &["resolve", "bad!"]),
        2,
        "invalid_argument",
    );
    assert!(invalid.error.suggested_fix.contains("--include-auto"));

    let missing = error(&run_file(&file, &["resolve", "bl_dead"]), 66, "not_found");
    assert!(missing.error.suggested_fix.contains("--include-auto"));
}

#[test]
fn export_otlp_json_golden_maps_all_statuses_without_evidence() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let records = [
        json!({
            "kind": "dogear", "id": "bl_72f5a7e2d02e", "ts": "2026-08-18T10:00:00.126Z",
            "agent": "dogear", "text": "idea only", "tags": [], "cwd": "docs",
            "evidence": "dogear evidence must not export"
        }),
        json!({
            "kind": "cut", "id": "bl_839a74dab188", "ts": "2026-08-18T10:00:00.125Z",
            "agent": "carol", "text": "blocker report", "tags": [], "severity": "blocker",
            "cwd": "src/c"
        }),
        json!({
            "kind": "resolve", "id": "bl_839a74dab188", "ts": "2026-08-18T10:06:00.000Z",
            "agent": "resolver", "note": "private dropped note", "dropped": true
        }),
        json!({
            "kind": "cut", "id": "bl_e4189e9ff71d", "ts": "2026-08-18T10:00:00.124Z",
            "agent": "bob", "text": "major report", "tags": ["release"], "severity": "major",
            "cwd": "src/b"
        }),
        json!({
            "kind": "resolve", "id": "bl_e4189e9ff71d", "ts": "2026-08-18T10:05:00.000Z",
            "agent": "resolver", "note": "private resolved note"
        }),
        json!({
            "kind": "cut", "id": "bl_d39a975de893", "ts": "2026-08-18T10:00:00.123Z",
            "agent": "alice", "text": "minor report", "tags": ["alpha", "ops"], "severity": "minor",
            "cwd": "src/a",
            "evidence": {
                "cmd": "do-not-export command", "exit": 9,
                "stderr": "do-not-export stderr", "note": "do-not-export evidence note"
            }
        }),
    ];
    let mut input = records
        .into_iter()
        .map(|record| record.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    input.push('\n');
    input.push_str("not json\n");
    std::fs::write(&file, input).unwrap();
    let before = std::fs::read(&file).unwrap();

    let output = command()
        .env("BLOTTER_NOW", "2026-08-19T00:00:00Z")
        .arg("--file")
        .arg(&file)
        .args(["export", "--format", "otlp-json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        output.stdout,
        include_bytes!("fixtures/export-otlp-json-golden.jsonl")
    );
    assert_eq!(std::fs::read(&file).unwrap(), before);

    let exported: Value = serde_json::from_slice(&output.stdout).unwrap();
    let records = exported["resourceLogs"][0]["scopeLogs"][0]["logRecords"]
        .as_array()
        .unwrap();
    assert_eq!(records.len(), 3);
    assert_eq!(
        records
            .iter()
            .map(|record| record["timeUnixNano"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "1787047200123000000",
            "1787047200124000000",
            "1787047200125000000",
        ]
    );
    assert_eq!(
        records
            .iter()
            .map(|record| record["severityNumber"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        [9, 13, 17]
    );
    assert_eq!(
        records
            .iter()
            .map(|record| record["severityText"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["INFO", "WARN", "ERROR"]
    );
    assert!(records.iter().all(|record| {
        record["eventName"] == "blotter.friction.reported"
            && record["timeUnixNano"].is_string()
            && record.get("traceId").is_none()
            && record.get("spanId").is_none()
            && record.get("schemaUrl").is_none()
    }));
    let attributes = records
        .iter()
        .map(|record| record["attributes"].as_array().unwrap())
        .collect::<Vec<_>>();
    assert!(
        attributes[0]
            .iter()
            .any(|attribute| attribute["key"] == "blotter.friction.status"
                && attribute["value"]["stringValue"] == "open")
    );
    assert!(
        attributes[1]
            .iter()
            .any(|attribute| attribute["key"] == "blotter.friction.status"
                && attribute["value"]["stringValue"] == "resolved")
    );
    assert!(
        attributes[2]
            .iter()
            .any(|attribute| attribute["key"] == "blotter.friction.status"
                && attribute["value"]["stringValue"] == "dropped")
    );
    assert!(
        attributes[1]
            .iter()
            .any(|attribute| attribute["key"] == "blotter.friction.resolved_ts")
    );
    assert!(
        attributes[0]
            .iter()
            .all(|attribute| attribute["key"] != "blotter.friction.resolved_ts")
    );
    assert!(
        attributes[2]
            .iter()
            .all(|attribute| attribute["key"] != "blotter.friction.resolved_ts")
    );

    let raw = String::from_utf8(output.stdout).unwrap();
    assert!(raw.contains("\"resourceLogs\""));
    assert!(raw.contains("\"timeUnixNano\""));
    assert!(!raw.contains("resource_logs"));
    assert!(!raw.contains("time_unix_nano"));
    for forbidden in [
        "bl_72f5a7e2d02e",
        "do-not-export command",
        "do-not-export stderr",
        "do-not-export evidence note",
        "private resolved note",
        "private dropped note",
        "\"cmd\"",
        "\"stderr\"",
        "\"note\"",
    ] {
        assert!(!raw.contains(forbidden), "export leaked {forbidden}");
    }
}

#[test]
fn export_otlp_json_hides_auto_captures_unless_requested() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let manual = add_at(
        &file,
        "2026-08-18T10:00:00Z",
        "manual export record",
        &["manual"],
    );
    let auto = add_at(
        &file,
        "2026-08-18T10:01:00Z",
        "auto export record",
        &["auto"],
    );

    let default = run_file(&file, &["export", "--format", "otlp-json"]);
    assert!(default.status.success());
    assert!(default.stderr.is_empty());
    let default: Value = serde_json::from_slice(&default.stdout).unwrap();
    let default_ids = default["resourceLogs"][0]["scopeLogs"][0]["logRecords"]
        .as_array()
        .unwrap()
        .iter()
        .map(|record| {
            record["attributes"][0]["value"]["stringValue"]
                .as_str()
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(default_ids, [manual.data.record.cut_id()]);

    let included = run_file(
        &file,
        &["export", "--format", "otlp-json", "--include-auto"],
    );
    assert!(included.status.success());
    assert!(included.stderr.is_empty());
    let included: Value = serde_json::from_slice(&included.stdout).unwrap();
    let included_ids = included["resourceLogs"][0]["scopeLogs"][0]["logRecords"]
        .as_array()
        .unwrap()
        .iter()
        .map(|record| {
            record["attributes"][0]["value"]["stringValue"]
                .as_str()
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        included_ids,
        [manual.data.record.cut_id(), auto.data.record.cut_id()]
    );
}

#[test]
fn export_otlp_json_empty_and_missing_default_are_stable_empty_lines() {
    let expected = format!(
        "{{\"resourceLogs\":[{{\"resource\":{{}},\"scopeLogs\":[{{\"scope\":{{\"name\":\"blotter\",\"version\":\"{}\"}},\"logRecords\":[]}}]}}]}}\n",
        env!("CARGO_PKG_VERSION")
    );
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("empty.jsonl");
    std::fs::write(&file, "").unwrap();
    let empty = run_file(&file, &["export", "--format", "otlp-json"]);
    assert_eq!(empty.status.code(), Some(0));
    assert!(empty.stderr.is_empty());
    assert_eq!(empty.stdout, expected.as_bytes());

    let repo = temp.path().join("repo");
    make_repo(&repo);
    let missing = command()
        .current_dir(&repo)
        .args(["export", "--format", "otlp-json"])
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(0));
    assert!(missing.stderr.is_empty());
    assert_eq!(missing.stdout, expected.as_bytes());
    assert!(!repo.join(".blotter.jsonl").exists());
}

#[test]
fn export_requires_otlp_json_format() {
    let output = run(&["export"]);
    let error = error(&output, 2, "invalid_argument");
    assert!(error.error.message.contains("--format otlp-json"));
    assert!(error.error.suggested_fix.contains("--format otlp-json"));
}

#[test]
fn export_missing_format_is_reported_before_the_clock_is_resolved() {
    let output = command()
        .env("BLOTTER_NOW", "not a timestamp")
        .arg("export")
        .output()
        .unwrap();
    let error = error(&output, 2, "invalid_argument");
    assert!(error.error.message.contains("--format otlp-json"));
}

#[test]
fn export_rejects_records_outside_the_otlp_nanosecond_range() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let records = [
        json!({
            "kind": "cut", "id": "bl_d39a975de893", "ts": "2026-08-18T10:00:00.123Z",
            "agent": "alice", "text": "modern report", "tags": [], "severity": "minor",
            "cwd": "src/a"
        }),
        json!({
            "kind": "cut", "id": "bl_0ff01ce4a5e5", "ts": "1969-07-20T20:17:00.000Z",
            "agent": "alice", "text": "pre-epoch report", "tags": [], "severity": "minor",
            "cwd": "src/a"
        }),
    ];
    let mut input = records
        .into_iter()
        .map(|record| record.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    input.push('\n');
    std::fs::write(&file, input).unwrap();

    let output = run_file(&file, &["export", "--format", "otlp-json"]);
    let error = error(&output, 65, "invalid_input");
    assert!(error.error.message.contains("bl_0ff01ce4a5e5"));
    assert!(error.error.message.contains("1969-07-20T20:17:00.000Z"));
    assert!(error.error.suggested_fix.contains("--since"));

    // Deterministic: the same input rejects with byte-identical bytes.
    let again = run_file(&file, &["export", "--format", "otlp-json"]);
    assert_eq!(again.stderr, output.stderr);

    // The in-range record still exports once the offender is filtered out.
    let filtered = run_file(
        &file,
        &[
            "export",
            "--format",
            "otlp-json",
            "--since",
            "2026-01-01T00:00:00Z",
        ],
    );
    assert_eq!(filtered.status.code(), Some(0));
    let exported: Value = serde_json::from_slice(&filtered.stdout).unwrap();
    let logs = exported["resourceLogs"][0]["scopeLogs"][0]["logRecords"]
        .as_array()
        .unwrap();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0]["timeUnixNano"], "1787047200123000000");
}

#[test]
fn export_otlp_json_since_matches_list() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let excluded = add_at(&file, "2026-07-09T16:30:00.122Z", "old export record", &[]);
    let included = add_at(&file, "2026-07-09T16:30:00.123Z", "new export record", &[]);

    let listed: SuccessEnvelope<ListData> = success(
        &command()
            .arg("--file")
            .arg(&file)
            .args(["list", "--since", "2h"])
            .output()
            .unwrap(),
    );
    let exported = command()
        .arg("--file")
        .arg(&file)
        .args(["export", "--format", "otlp-json", "--since", "2h"])
        .output()
        .unwrap();
    assert!(exported.status.success());
    assert!(exported.stderr.is_empty());
    let exported: Value = serde_json::from_slice(&exported.stdout).unwrap();
    let mut export_ids = exported["resourceLogs"][0]["scopeLogs"][0]["logRecords"]
        .as_array()
        .unwrap()
        .iter()
        .map(|record| {
            record["attributes"][0]["value"]["stringValue"]
                .as_str()
                .unwrap()
        })
        .collect::<Vec<_>>();
    let mut list_ids = listed
        .data
        .items
        .iter()
        .map(|item| item.id.as_str())
        .collect::<Vec<_>>();
    export_ids.sort_unstable();
    list_ids.sort_unstable();
    assert_eq!(export_ids, list_ids);
    assert_eq!(export_ids, [included.data.record.cut_id()]);
    assert_ne!(excluded.data.record.cut_id(), included.data.record.cut_id());
}

#[test]
fn schema_documents_export_otlp_bridge() {
    let schema: SuccessEnvelope<Value> = success(&run(&["schema"]));
    let export = &schema.data["commands"]["export"];
    assert_eq!(export["flags"]["--format"], "otlp-json; required");
    assert_eq!(export["flags"]["--since"], "full RFC3339|Nd|Nh");
    assert!(
        export["flags"]["--include-auto"]
            .as_str()
            .unwrap()
            .contains("include records tagged auto")
    );
    assert_eq!(export["eventName"], "blotter.friction.reported");
    assert!(export["output"].as_str().unwrap().contains("raw"));
    assert!(export["output"].as_str().unwrap().contains("LogsData"));
    assert_eq!(export["read_only"], true);
    assert_eq!(export["appends"], false);
    assert_eq!(export["destructive"], false);
}

#[cfg(unix)]
#[test]
fn archive_resolves_symlinked_log_and_preserves_the_link() {
    let temp = TempDir::new().unwrap();
    let target = temp.path().join("real.jsonl");
    let link = temp.path().join("link.jsonl");
    let (old_id, old_cut) = archive_cut("2026-07-01T00:00:00Z", "old resolved cut");
    let old_resolve = archive_resolution(&old_id, "2026-07-02T00:00:00Z", false, false);
    let (_, open_cut) = archive_cut("2026-07-01T00:00:00Z", "still open");
    std::fs::write(&target, [old_cut, old_resolve, open_cut.clone()].concat()).unwrap();
    std::os::unix::fs::symlink("real.jsonl", &link).unwrap();

    let archive: SuccessEnvelope<Value> = success(&run_file(
        &link,
        &["archive", "--before", "2026-08-01T00:00:00Z"],
    ));
    assert_eq!(archive.data["changed"], true);
    assert_eq!(archive.data["archived"], 2);
    let backup = format!("{}.bak-20260709T183000123Z", target.display());
    let archive_file = format!("{}.archive-20260709T183000123Z.jsonl", target.display());
    assert_eq!(archive.data["backup"], Value::String(backup.clone()));
    assert_eq!(archive.data["archive_file"], Value::String(archive_file));
    assert_eq!(
        archive.data["restore_hint"],
        Value::String(format!("cp '{backup}' '{}'", target.display()))
    );
    assert!(
        std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(std::fs::read(&target).unwrap(), open_cut);
    assert_eq!(std::fs::read(&link).unwrap(), open_cut);
}

#[cfg(unix)]
#[test]
fn doctor_fix_resolves_symlinked_log_and_preserves_the_link() {
    let temp = TempDir::new().unwrap();
    let target = temp.path().join("real.jsonl");
    let link = temp.path().join("link.jsonl");
    add(&target, "valid");
    let complete = std::fs::read(&target).unwrap();
    let mut writer = OpenOptions::new().append(true).open(&target).unwrap();
    writer.write_all(b"{\"kind\":").unwrap();
    drop(writer);
    std::os::unix::fs::symlink("real.jsonl", &link).unwrap();

    let doctor = doctor_response(&run_file(&link, &["doctor", "--fix"]), 0);
    let fix = doctor.data.fix.as_ref().unwrap();
    assert!(fix.changed);
    assert_eq!(
        fix.backup.as_deref(),
        Some(format!("{}.bak-20260709T183000123Z", target.display()).as_str())
    );
    assert!(
        std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(std::fs::read(&target).unwrap(), complete);
}

#[test]
fn archive_sole_newline_log_has_zero_physical_lines() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    std::fs::write(&file, b"\n").unwrap();

    let archive: SuccessEnvelope<Value> = success(&run_file(
        &file,
        &["archive", "--before", "2026-08-01T00:00:00Z"],
    ));
    assert_eq!(archive.data["changed"], false);
    assert_eq!(archive.data["archived"], 0);
    assert_eq!(archive.data["kept"], 0);
    assert_eq!(std::fs::read(&file).unwrap(), b"\n");
}

// --- store.rs log-path guard, amend ordering by timestamp, cwd redaction, and
// --- the add/dogear stdin raw gate.

#[cfg(unix)]
fn spawn_blotter(file: &Path, args: &[&str]) -> std::process::Child {
    let mut child = std::process::Command::new(assert_cmd::cargo::cargo_bin!("blotter"));
    child
        .env("BLOTTER_NOW", NOW)
        .env_remove("BLOTTER_FILE")
        .env_remove("BLOTTER_AGENT")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("--file")
        .arg(file)
        .args(args);
    child.spawn().unwrap()
}

/// Wait with a deadline. The pre-fix FIFO behaviour was an unbounded block, so a
/// regression must fail this test rather than wedge the whole suite.
#[cfg(unix)]
fn wait_bounded(mut child: std::process::Child, what: &str) -> std::process::Output {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if child.try_wait().unwrap().is_some() {
            return child.wait_with_output().unwrap();
        }
        if std::time::Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("{what} blocked on a non-regular log path");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

fn non_regular_log_cases() -> [(&'static str, Vec<&'static str>); 5] {
    [
        ("list", vec!["list"]),
        ("triage", vec!["triage"]),
        ("digest", vec!["digest"]),
        ("add", vec!["add", "non-regular log", "--agent", "tester"]),
        ("doctor", vec!["doctor"]),
    ]
}

fn assert_non_regular_log(output: &std::process::Output, what: &str) {
    let envelope = error(output, 65, "invalid_input");
    assert!(
        envelope.error.message.contains("not a regular file"),
        "{what}: {}",
        envelope.error.message
    );
    assert!(
        envelope.error.suggested_fix.contains("FIFOs and devices"),
        "{what}: {}",
        envelope.error.suggested_fix
    );
}

#[cfg(unix)]
#[test]
fn a_fifo_log_path_is_rejected_without_blocking() {
    let temp = TempDir::new().unwrap();
    let fifo = temp.path().join("log.fifo");
    let made_fifo = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .is_ok_and(|status| status.success());
    if !made_fifo {
        eprintln!("skipping FIFO log assertion; mkfifo unavailable");
        return;
    }
    for (what, args) in non_regular_log_cases() {
        let output = wait_bounded(spawn_blotter(&fifo, &args), what);
        assert_non_regular_log(&output, what);
    }
}

#[cfg(unix)]
#[test]
fn a_device_log_path_is_rejected_before_an_unbounded_read() {
    let device = Path::new("/dev/zero");
    if !device.exists() {
        eprintln!("skipping device log assertion; /dev/zero unavailable");
        return;
    }
    for (what, args) in non_regular_log_cases() {
        assert_non_regular_log(&run_file(device, &args), what);
    }
    // Deliberate behaviour change: /dev/null used to fold as an empty log and
    // exit 0. It is a character device, so it is invalid_input like the rest.
    assert_non_regular_log(
        &run_file(Path::new("/dev/null"), &["list"]),
        "list /dev/null",
    );
}

fn resolve_line(id: &str, ts: &str, note: &str, amend: bool) -> String {
    let mut value = json!({"kind":"resolve","id":id,"ts":ts,"agent":"fixer","note":note});
    if amend {
        value["amend"] = json!(true);
    }
    value.to_string()
}

fn append_lines(file: &Path, lines: &[String]) {
    let mut log = std::fs::read_to_string(file).unwrap();
    for line in lines {
        log.push_str(line);
        log.push('\n');
    }
    std::fs::write(file, log).unwrap();
}

#[test]
fn resolve_amend_with_the_latest_timestamp_wins_over_file_order() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let cut = add(&file, "merge reordered amends");
    let id = cut.data.record.cut_id().to_owned();
    // A union merge concatenates branches in branch order, so the older amend
    // can land last in the byte stream. Timestamp decides, not file position.
    append_lines(
        &file,
        &[
            resolve_line(&id, "2026-07-10T00:00:00.000Z", "base", false),
            resolve_line(&id, "2026-07-12T00:00:00.000Z", "later", true),
            resolve_line(&id, "2026-07-11T00:00:00.000Z", "earlier", true),
        ],
    );

    let listed: SuccessEnvelope<ListData> = success(&run_file(&file, &["list", "--status", "all"]));
    let resolution = listed.data.items[0].resolution.as_ref().unwrap();
    assert_eq!(resolution.note.as_deref(), Some("later"));
    assert_eq!(resolution.ts, "2026-07-12T00:00:00.000Z");
    assert!(resolution.amended);
}

#[test]
fn resolve_amends_sharing_a_timestamp_keep_the_last_in_file_order() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let cut = add(&file, "frozen clock amends");
    let id = cut.data.record.cut_id().to_owned();
    // Two amends under one frozen BLOTTER_NOW are reachable from the CLI; the
    // comparison is >=, so the last one in file order still wins the tie.
    append_lines(
        &file,
        &[
            resolve_line(&id, "2026-07-10T00:00:00.000Z", "base", false),
            resolve_line(&id, "2026-07-11T00:00:00.000Z", "first", true),
            resolve_line(&id, "2026-07-11T00:00:00.000Z", "second", true),
        ],
    );

    let listed: SuccessEnvelope<ListData> = success(&run_file(&file, &["list", "--status", "all"]));
    let resolution = listed.data.items[0].resolution.as_ref().unwrap();
    assert_eq!(resolution.note.as_deref(), Some("second"));
}

#[test]
fn verify_exit_code_is_stable_when_amend_lines_are_swapped() {
    for (name, amends_in_clock_order) in [("clock order", true), ("merge order", false)] {
        let temp = TempDir::new().unwrap();
        let file = temp.path().join("cuts.jsonl");
        let anchor = add_at(
            &file,
            "2026-07-09T18:30:00Z",
            "Cache configuration missing",
            &[],
        );
        let id = anchor.data.record.cut_id().to_owned();
        add_at(
            &file,
            "2026-07-09T18:35:00Z",
            "Cache configuration missing",
            &[],
        );
        let early = resolve_line(&id, "2026-07-09T18:32:00.000Z", "early", true);
        let late = resolve_line(&id, "2026-07-09T18:40:00.000Z", "late", true);
        let mut lines = vec![resolve_line(&id, "2026-07-09T18:31:00.000Z", "base", false)];
        if amends_in_clock_order {
            lines.extend([early, late]);
        } else {
            lines.extend([late, early]);
        }
        append_lines(&file, &lines);

        // The later open cut predates the latest amend either way, so byte order
        // must not flip verify between exit 0 and exit 1.
        let verify = verify_success(&run_file(&file, &["verify"]), 0);
        assert_eq!(verify.data["count"], 0, "{name}");
        assert_eq!(verify.data["recurrences"], json!([]), "{name}");
    }
}

#[test]
fn cwd_under_a_dash_encoded_home_slug_is_redacted() {
    let temp = TempDir::new().unwrap();
    if temp_has_git_ancestor(&temp) {
        eprintln!("skipping dash-encoded cwd assertion inside a git checkout");
        return;
    }
    let home = temp.path().join("fakehome");
    let scratchpad = temp
        .path()
        .join("claude-501")
        .join("-Users-alice-Documents-GitHub-blotter")
        .join("sess")
        .join("scratchpad");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&scratchpad).unwrap();
    let scratchpad = scratchpad.canonicalize().unwrap();
    // The r23 rule rewrites only the matched prefix and keeps the rest of the
    // dash-encoded token verbatim.
    let expected = scratchpad.to_string_lossy().replace("-Users-alice", "~");
    assert!(expected.contains("/~-Documents-GitHub-blotter/"));

    let file = temp.path().join("cuts.jsonl");
    let added: SuccessEnvelope<AddData> = success(
        &command()
            .env("HOME", &home)
            .current_dir(&scratchpad)
            .arg("--file")
            .arg(&file)
            .args(["add", "dash-encoded cwd", "--agent", "tester"])
            .output()
            .unwrap(),
    );
    assert_eq!(added.data.record.cut_cwd(), expected);

    let doctor = doctor_response(
        &command()
            .env("HOME", &home)
            .arg("--file")
            .arg(&file)
            .args(["doctor", "--leaks"])
            .output()
            .unwrap(),
        0,
    );
    assert!(doctor.data.healthy);
}

#[test]
fn cwd_under_a_generic_home_root_is_redacted() {
    let cwd = std::env::current_dir().unwrap();
    let cwd_text = cwd.to_string_lossy().into_owned();
    let Some(rest) = ["/Users/", "/home/"]
        .into_iter()
        .find_map(|prefix| cwd_text.strip_prefix(prefix))
    else {
        eprintln!("skipping generic home cwd assertion outside /Users and /home");
        return;
    };
    if cwd_text.contains(char::is_whitespace) {
        eprintln!("skipping generic home cwd assertion; the path is not one token");
        return;
    }
    let expected = match rest.split_once('/') {
        Some((_, tail)) => format!("~/{tail}"),
        None => "~".to_owned(),
    };

    let temp = TempDir::new().unwrap();
    let home = temp.path().join("fakehome");
    std::fs::create_dir_all(&home).unwrap();
    // The log lives outside the repo, so the cwd is not repo-relative, and $HOME
    // is elsewhere: only the generic /Users/ and /home/ rule can redact it.
    let file = temp.path().join("cuts.jsonl");
    let added: SuccessEnvelope<AddData> = success(
        &command()
            .env("HOME", &home)
            .current_dir(&cwd)
            .arg("--file")
            .arg(&file)
            .args(["add", "generic home cwd", "--agent", "tester"])
            .output()
            .unwrap(),
    );
    assert_eq!(added.data.record.cut_cwd(), expected);

    let doctor = doctor_response(
        &command()
            .env("HOME", &home)
            .arg("--file")
            .arg(&file)
            .args(["doctor", "--leaks"])
            .output()
            .unwrap(),
        0,
    );
    assert!(doctor.data.healthy);
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
