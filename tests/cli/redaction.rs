use crate::common::*;

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
