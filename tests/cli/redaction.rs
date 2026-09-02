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
            Impact::Low,
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
                "--disposition",
                "fixed",
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
                "v": 2,
                "kind": "cut",
                "id": compute_id(NOW, "tester", &text, Impact::Low, &[]),
                "ts": NOW,
                "agent": "tester",
                "text": text,
                "tags": [],
                "impact": "low",
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
fn colon_separated_path_lists_rewrite_every_home_in_evidence_and_doctor() {
    let temp = TempDir::new().unwrap();
    let evidence_file = temp.path().join("evidence.jsonl");
    let list = "PATH=/Users/alice/bin:/home/bob/bin:-Users-carol-work/cache:/Users/dave/bin";
    let redacted = "PATH=~/bin:~/bin:~-work/cache:~/bin";

    let added: SuccessEnvelope<AddData> = success(
        &command()
            .env("HOME", "/Users/alice")
            .arg("--file")
            .arg(&evidence_file)
            .args(["add", "path list", "--agent", "tester", "--evidence"])
            .arg(list)
            .output()
            .unwrap(),
    );
    assert_eq!(
        added.data.record.cut_evidence().unwrap().note.as_deref(),
        Some(redacted)
    );

    let log = temp.path().join("doctor.jsonl");
    let texts = [
        "/opt/bin:/home/bob/bin",
        "/opt/bin:-Users-carol-work/cache",
        "/opt/bin:/mnt/home/shared",
        list,
        redacted,
    ];
    let records = texts
        .iter()
        .map(|text| {
            json!({
                "v": 2,
                "kind": "cut",
                "id": compute_id(NOW, "tester", text, Impact::Low, &[]),
                "ts": NOW,
                "agent": "tester",
                "text": text,
                "tags": [],
                "impact": "low",
                "cwd": "."
            })
            .to_string()
        })
        .collect::<Vec<_>>();
    std::fs::write(&log, format!("{}\n", records.join("\n"))).unwrap();
    let doctor = doctor_response(
        &command()
            .env("HOME", "/Users/alice")
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
    assert_eq!(leak_lines, [1, 2, 4]);
}

#[test]
fn home_forms_nested_in_a_redacted_token_tail_agree_with_doctor() {
    let temp = TempDir::new().unwrap();
    let evidence_file = temp.path().join("evidence.jsonl");
    let input =
        "/Users/alice/x/-Users-bob-y /Users/alice/backup/Users/alice/z /Users/alice/-Users-alice";
    let redacted = "~/x/~-y ~/backup~/z ~/~";

    let added: SuccessEnvelope<AddData> = success(
        &command()
            .env("HOME", "/Users/alice")
            .arg("--file")
            .arg(&evidence_file)
            .args(["add", "nested tails", "--agent", "tester", "--evidence"])
            .arg(input)
            .output()
            .unwrap(),
    );
    assert_eq!(
        added.data.record.cut_evidence().unwrap().note.as_deref(),
        Some(redacted)
    );

    // The first two texts are what the pre-r38 redactor stored for these
    // inputs; doctor must flag them, and must pass the r38 output.
    let log = temp.path().join("doctor.jsonl");
    let texts = ["~/x/-Users-bob-y", "~/backup/Users/alice/z", redacted];
    let records = texts
        .iter()
        .map(|text| {
            json!({
                "v": 2,
                "kind": "cut",
                "id": compute_id(NOW, "tester", text, Impact::Low, &[]),
                "ts": NOW,
                "agent": "tester",
                "text": text,
                "tags": [],
                "impact": "low",
                "cwd": "."
            })
            .to_string()
        })
        .collect::<Vec<_>>();
    std::fs::write(&log, format!("{}\n", records.join("\n"))).unwrap();
    let doctor = doctor_response(
        &command()
            .env("HOME", "/Users/alice")
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
    assert_eq!(leak_lines, [1, 2]);
}

#[test]
fn colon_path_list_boundaries_leave_secret_and_url_parsing_unchanged() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let input = "TOKEN:abc:def api_key=one:two url=https://user:credential@host.test:8443/path";
    let expected = "TOKEN:<redacted> api_key=<redacted> url=https://<redacted>@host.test:8443/path";
    let added: SuccessEnvelope<AddData> = success(
        &command()
            .env("HOME", "/Users/alice")
            .arg("--file")
            .arg(&file)
            .args(["add", "colon secrets", "--agent", "tester", "--evidence"])
            .arg(input)
            .output()
            .unwrap(),
    );
    assert_eq!(
        added.data.record.cut_evidence().unwrap().note.as_deref(),
        Some(expected)
    );
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
    let id = compute_id(NOW, "tester", "leaking", Impact::Low, &[]);
    let record = json!({
        "v": 2,
        "kind": "cut",
        "id": id,
        "ts": NOW,
        "agent": "tester",
        "text": "leaking",
        "tags": [],
        "impact": "low",
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

#[test]
fn doctor_leaks_accepts_the_redaction_marker_after_a_generic_home_prefix() {
    let temp = TempDir::new().unwrap();
    // The redactor leaves a `~` behind a generic prefix whose own username
    // component was empty: only the nested exact home matched. Both shapes are
    // blotter's own output, so its own gate must accept them (r39).
    let cases = [
        ("slash", "/Users//Users/alice/x", "/Users/~/x"),
        ("dash", "-Users-/Users/alice/x", "-Users-~/x"),
    ];
    for (name, input, expected) in cases {
        let file = temp.path().join(format!("{name}.jsonl"));
        let added: SuccessEnvelope<AddData> = success(
            &command()
                .env("HOME", "/Users/alice")
                .arg("--file")
                .arg(&file)
                .args([
                    "add",
                    "evidence case",
                    "--agent",
                    "tester",
                    "--evidence",
                    input,
                ])
                .output()
                .unwrap(),
        );
        assert_eq!(
            added.data.record.cut_evidence().unwrap().note.as_deref(),
            Some(expected),
            "{name}"
        );
        let output = command()
            .env("HOME", "/Users/alice")
            .arg("--file")
            .arg(&file)
            .args(["doctor", "--leaks"])
            .output()
            .unwrap();
        let doctor = doctor_response(&output, 0);
        assert!(doctor.data.findings.is_empty(), "{name}");
    }
}

#[test]
fn doctor_leaks_still_reports_real_usernames_after_a_generic_home_prefix() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("leaking.jsonl");
    // Only the bare marker is blotter's own output. A component that merely
    // starts with `~` is a real directory name and stays a leak.
    let plain = leak_record("/Users/alice/x");
    let dashed = leak_record("/private/tmp/-Users-alice-x/y");
    let tilde_prefixed = leak_record("/Users/~abc/x");
    std::fs::write(&file, format!("{plain}\n{dashed}\n{tilde_prefixed}\n")).unwrap();
    let output = command()
        .env("HOME", "/var/root")
        .arg("--file")
        .arg(&file)
        .args(["doctor", "--leaks"])
        .output()
        .unwrap();
    let doctor = doctor_response(&output, 1);
    let leak_lines: Vec<_> = doctor
        .data
        .findings
        .iter()
        .filter(|finding| finding.kind == "leak")
        .map(|finding| finding.line)
        .collect();
    assert_eq!(leak_lines, [1, 2, 3]);
}

fn leak_record(cwd: &str) -> serde_json::Value {
    let id = compute_id(NOW, "tester", cwd, Impact::Low, &[]);
    json!({
        "v": 2,
        "kind": "cut",
        "id": id,
        "ts": NOW,
        "agent": "tester",
        "text": cwd,
        "tags": [],
        "impact": "low",
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
        // r40 removed the dash-form start boundary, so this text now redacts
        // and its record ID moves with the redacted bytes.
        (
            "dash_after_a_dash",
            "failed under -Users--Users-jane-doe-y",
            "failed under -Users-~-y",
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
                Impact::Low,
                &[]
            )
        );
        let stored_add: Value =
            serde_json::from_str(&std::fs::read_to_string(&add_file).unwrap()).unwrap();
        assert_eq!(stored_add, stored_line(&add_record));

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
        assert_eq!(stored_dogear, stored_line(&dogear_record));
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
            Impact::Low,
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
    let clean_id = compute_id(NOW, "tester", "clean", Impact::Low, &[]);
    let clean_record = json!({
        "v": 2,
        "kind": "cut",
        "id": clean_id,
        "ts": NOW,
        "agent": "tester",
        "text": "clean",
        "tags": [],
        "impact": "low",
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
    let denied_id = compute_id(NOW, "tester", "literal credential", Impact::Low, &[]);
    let denied_record = json!({
        "v": 2,
        "kind": "cut",
        "id": denied_id,
        "ts": NOW,
        "agent": "tester",
        "text": "literal credential",
        "tags": [],
        "impact": "low",
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

#[test]
fn dogear_evidence_is_redacted_across_home_forms_and_leaves_other_text_verbatim() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let home = "/private/alice";
    let evidence = format!(
        "read {home}/notes /Users/other/desk /private/tmp/agent/-Users-someuser-somerepo/scratchpad and bench run 42"
    );
    let added: SuccessEnvelope<Value> = success(
        &command()
            .env("HOME", home)
            .arg("--file")
            .arg(&file)
            .args(["dogear", "an idea", "--agent", "tester", "--evidence"])
            .arg(&evidence)
            .output()
            .unwrap(),
    );
    let expected = "read ~/notes ~/desk /private/tmp/agent/~-somerepo/scratchpad and bench run 42";
    assert_eq!(added.data["record"]["evidence"], expected);
    let stored = std::fs::read_to_string(&file).unwrap();
    let line: Value = serde_json::from_str(stored.trim_end()).unwrap();
    assert_eq!(line["evidence"], expected);
}

#[test]
fn dogear_evidence_without_a_home_path_stays_byte_identical() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let evidence = "benchmark run 42 in ./target/criterion";
    let added: SuccessEnvelope<Value> = success(
        &command()
            .env("HOME", "/private/alice")
            .arg("--file")
            .arg(&file)
            .args(["dogear", "an idea", "--agent", "tester", "--evidence"])
            .arg(evidence)
            .output()
            .unwrap(),
    );
    assert_eq!(added.data["record"]["evidence"], evidence);
}

#[test]
fn dogear_evidence_and_resolve_note_run_the_secret_span_pass() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let added: SuccessEnvelope<Value> = success(
        &command()
            .env("HOME", "/private/alice")
            .arg("--file")
            .arg(&file)
            .args([
                "dogear",
                "an idea",
                "--agent",
                "tester",
                "--evidence",
                "retry with api_key=abcdef0123456789 next time",
            ])
            .output()
            .unwrap(),
    );
    assert_eq!(
        added.data["record"]["evidence"],
        "retry with api_key=<redacted> next time"
    );

    let cut: SuccessEnvelope<AddData> = success(
        &command()
            .env("HOME", "/private/alice")
            .arg("--file")
            .arg(&file)
            .args(["add", "a cut", "--agent", "tester"])
            .output()
            .unwrap(),
    );
    let id = cut.data.record.cut_id().to_string();
    let resolved: SuccessEnvelope<Value> = success(
        &command()
            .env("HOME", "/private/alice")
            .arg("--file")
            .arg(&file)
            .args([
                "resolve",
                "--disposition",
                "fixed",
                &id,
                "--agent",
                "tester",
                "--note",
                "rotated DB_PASSWORD=hunter22hunter22 afterwards",
            ])
            .output()
            .unwrap(),
    );
    // The env-assignment shape redacts as one span, key included.
    assert_eq!(
        resolved.data["records"][0]["resolution"]["note"],
        "rotated <redacted> afterwards"
    );
}

#[test]
fn resolve_dry_run_note_reports_the_same_bytes_an_apply_stores() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let home = "/private/alice";
    let added: SuccessEnvelope<AddData> = success(
        &command()
            .env("HOME", home)
            .arg("--file")
            .arg(&file)
            .args(["add", "a cut", "--agent", "tester"])
            .output()
            .unwrap(),
    );
    let id = added.data.record.cut_id().to_string();
    let before = std::fs::read(&file).unwrap();

    let note = format!("fixed in {home}/repo and api_key=abcdef0123456789");
    let dry: SuccessEnvelope<Value> = success(
        &command()
            .env("HOME", home)
            .arg("--file")
            .arg(&file)
            .args([
                "resolve",
                "--disposition",
                "fixed",
                &id,
                "--agent",
                "tester",
                "--dry-run",
                "--note",
            ])
            .arg(&note)
            .output()
            .unwrap(),
    );
    let dry_note = dry.data["records"][0]["resolution"]["note"].clone();
    assert_eq!(dry_note, "fixed in ~/repo and api_key=<redacted>");
    assert_eq!(std::fs::read(&file).unwrap(), before);

    success::<Value>(
        &command()
            .env("HOME", home)
            .arg("--file")
            .arg(&file)
            .args([
                "resolve",
                "--disposition",
                "fixed",
                &id,
                "--agent",
                "tester",
                "--note",
            ])
            .arg(&note)
            .output()
            .unwrap(),
    );
    let stored = std::fs::read_to_string(&file).unwrap();
    let last: Value = serde_json::from_str(stored.lines().last().unwrap()).unwrap();
    assert_eq!(last["note"], dry_note);
}

#[test]
fn resolve_note_is_redacted_in_the_base_event_and_in_an_amend() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let home = "/private/alice";
    let added: SuccessEnvelope<AddData> = success(
        &command()
            .env("HOME", home)
            .arg("--file")
            .arg(&file)
            .args(["add", "a cut", "--agent", "tester"])
            .output()
            .unwrap(),
    );
    let id = added.data.record.cut_id().to_string();

    let base_note = format!("fixed in {home}/repo/src and /Users/other/fork");
    let resolved: SuccessEnvelope<Value> = success(
        &command()
            .env("HOME", home)
            .arg("--file")
            .arg(&file)
            .args([
                "resolve",
                "--disposition",
                "fixed",
                &id,
                "--agent",
                "tester",
                "--note",
            ])
            .arg(&base_note)
            .output()
            .unwrap(),
    );
    assert_eq!(
        resolved.data["records"][0]["resolution"]["note"],
        "fixed in ~/repo/src and ~/fork"
    );

    let amend_note = "reopened; see /private/tmp/agent/-Users-someuser-somerepo/scratchpad";
    let amended: SuccessEnvelope<Value> = success(
        &command()
            .env("HOME", home)
            .arg("--file")
            .arg(&file)
            .args(["resolve", &id, "--agent", "tester", "--amend", "--note"])
            .arg(amend_note)
            .output()
            .unwrap(),
    );
    let expected_amend = "reopened; see /private/tmp/agent/~-somerepo/scratchpad";
    assert_eq!(
        amended.data["records"][0]["resolution"]["note"],
        expected_amend
    );

    let stored = std::fs::read_to_string(&file).unwrap();
    assert!(
        !stored.contains(home),
        "raw home leaked into the log: {stored}"
    );
    assert!(!stored.contains("/Users/other/fork"));
    assert!(stored.contains(expected_amend));
}

#[test]
fn resolve_note_without_a_home_path_stays_byte_identical() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let added: SuccessEnvelope<AddData> = success(
        &command()
            .env("HOME", "/private/alice")
            .arg("--file")
            .arg(&file)
            .args(["add", "a cut", "--agent", "tester"])
            .output()
            .unwrap(),
    );
    let id = added.data.record.cut_id().to_string();
    let note = "fixed in src/store.rs; see PR 12";
    let resolved: SuccessEnvelope<Value> = success(
        &command()
            .env("HOME", "/private/alice")
            .arg("--file")
            .arg(&file)
            .args([
                "resolve",
                "--disposition",
                "fixed",
                &id,
                "--agent",
                "tester",
                "--note",
            ])
            .arg(note)
            .output()
            .unwrap(),
    );
    assert_eq!(resolved.data["records"][0]["resolution"]["note"], note);
}

// The entropy heuristic wants >=24 bytes, >=12 distinct bytes, and mixed case
// plus a digit. Shared by the r40/r41 cases that involve the secret pass.
const ENTROPY_TOKEN: &str = "aB3xY7zQ9wE2rT5yU8iO1pA4sD6fG0hJ";

fn redacted_evidence(file: &Path, home: &str, note: &str) -> String {
    let added: SuccessEnvelope<AddData> = success(
        &command()
            .env("HOME", home)
            .arg("--file")
            .arg(file)
            .args(["add", "evidence case", "--agent", "tester", "--evidence"])
            .arg(note)
            .output()
            .unwrap(),
    );
    added
        .data
        .record
        .cut_evidence()
        .unwrap()
        .note
        .clone()
        .unwrap()
}

fn leaks_exit_zero(file: &Path, home: &str, label: &str) {
    let output = command()
        .env("HOME", home)
        .arg("--file")
        .arg(file)
        .args(["doctor", "--leaks"])
        .output()
        .unwrap();
    let doctor = doctor_response(&output, 0);
    assert!(doctor.data.findings.is_empty(), "{label}");
}

fn leak_lines(file: &Path, home: &str) -> Vec<usize> {
    let output = command()
        .env("HOME", home)
        .arg("--file")
        .arg(file)
        .args(["doctor", "--leaks"])
        .output()
        .unwrap();
    let doctor = doctor_response(&output, 1);
    doctor
        .data
        .findings
        .iter()
        .filter(|finding| finding.kind == "leak")
        .map(|finding| finding.line)
        .collect()
}

#[test]
fn exact_dash_home_matches_after_a_dash_and_mid_token() {
    let temp = TempDir::new().unwrap();
    // r40: the exact dash-encoded current home has no start boundary, exactly
    // like its slash spelling. A doubled separator, a mid-token hit, and a home
    // nested in an encoded path all redact.
    let cases = [
        ("doubled", "-Users--Users-alice-y", "-Users-~-y"),
        ("mid_token", "x-Users-alice", "x~"),
        ("nested", "-tmp-backup-Users-alice-y", "-tmp-backup~-y"),
    ];
    for (name, input, expected) in cases {
        let file = temp.path().join(format!("{name}.jsonl"));
        assert_eq!(
            redacted_evidence(&file, "/Users/alice", input),
            expected,
            "{name}"
        );
        leaks_exit_zero(&file, "/Users/alice", name);
    }

    // The same three shapes in a hand-written log, i.e. written before this
    // rule: the gate reports each one.
    let raw = temp.path().join("raw.jsonl");
    let lines: Vec<String> = cases
        .iter()
        .map(|(_, input, _)| leak_record(input).to_string())
        .collect();
    std::fs::write(&raw, format!("{}\n", lines.join("\n"))).unwrap();
    assert_eq!(leak_lines(&raw, "/Users/alice"), [1, 2, 3]);
}

#[test]
fn dash_home_matching_stops_at_the_component_end() {
    let temp = TempDir::new().unwrap();
    // The end boundary is untouched by r40: a longer or shorter component is a
    // different directory. The leading `x` keeps the generic `-Users-` prefix
    // out of it, so only the exact-home branch is under test.
    for (name, input) in [("longer", "x-Users-alicexyz"), ("shorter", "x-Users-alic")] {
        let file = temp.path().join(format!("{name}.jsonl"));
        assert_eq!(
            redacted_evidence(&file, "/Users/alice", input),
            input,
            "{name}"
        );
        leaks_exit_zero(&file, "/Users/alice", name);
    }
}

#[test]
fn generic_dash_prefixes_keep_their_start_boundary() {
    let temp = TempDir::new().unwrap();
    // r23's start-boundary rule survives r40 for the generic prefixes: a
    // generic prefix after a dash, and a bare mid-token hit, stay unrewritten
    // and unflagged on both sides.
    for (name, input) in [
        ("after_dash", "-Users--home-bob-x"),
        ("mid_token", "dir-Users-bob-y"),
    ] {
        let file = temp.path().join(format!("{name}.jsonl"));
        assert_eq!(
            redacted_evidence(&file, "/Users/alice", input),
            input,
            "{name}"
        );
        leaks_exit_zero(&file, "/Users/alice", name);
    }
}

#[test]
fn real_harness_slugs_with_doubled_separators_redact_once() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let note = redacted_evidence(
        &file,
        "/Users/alice",
        "/private/tmp/claude-501/-Users-alice--claude-skills-x/y",
    );
    assert_eq!(note, "/private/tmp/claude-501/~--claude-skills-x/y");
    leaks_exit_zero(&file, "/Users/alice", "harness slug");
}

#[test]
fn dash_home_inside_an_entropy_token_splits_the_secret() {
    let temp = TempDir::new().unwrap();
    // The accepted r40 ordering cost: home rewriting runs before the secret
    // pass (r25), so the emitted `~` splits a high-entropy token and the
    // fragments fall below the thresholds. The dash spelling now pays exactly
    // what the slash spelling has always paid.
    for (name, input, expected) in [
        ("dash", "AbC1defx-Users-alice-Z9yX8w", "AbC1defx~-Z9yX8w"),
        ("slash", "AbC1defx/Users/alice/Z9yX8w", "AbC1defx~/Z9yX8w"),
    ] {
        let file = temp.path().join(format!("{name}.jsonl"));
        assert_eq!(
            redacted_evidence(&file, "/Users/alice", input),
            expected,
            "{name}"
        );
        leaks_exit_zero(&file, "/Users/alice", name);
    }
}

#[test]
fn doctor_leaks_accepts_composed_redaction_markers() {
    let temp = TempDir::new().unwrap();
    // r41: r38's resume-after-match and the secret pass (r25) let the redactor
    // write `~~`, `~<redacted>`, and their compositions behind a generic home
    // prefix. Every one of these is blotter's own output, so its own gate must
    // accept it.
    let cases = [
        (
            "slash_marker_secret",
            format!("/Users//Users/alice/{ENTROPY_TOKEN}"),
            "/Users/~<redacted>",
        ),
        (
            "dash_marker_secret",
            format!("-Users-/Users/alice/{ENTROPY_TOKEN}"),
            "-Users-~<redacted>",
        ),
        (
            "slash_doubled_marker",
            "/Users//Users/alice/Users/alice".into(),
            "/Users/~~",
        ),
        (
            "dash_doubled_marker",
            "-Users--Users-alice-Users-alice-y".into(),
            "-Users-~~-y",
        ),
        (
            "doubled_marker_then_secret",
            format!("/Users//Users/alice/Users/alice/{ENTROPY_TOKEN}"),
            "/Users/~~<redacted>",
        ),
        (
            "secret_then_marker",
            format!("/Users//Users/alice/{ENTROPY_TOKEN}/Users/alice"),
            "/Users/~<redacted>~",
        ),
        (
            "secret_then_tail",
            format!("/Users//Users/alice/{ENTROPY_TOKEN}@bob"),
            "/Users/~<redacted>@bob",
        ),
    ];
    for (name, input, expected) in cases {
        let file = temp.path().join(format!("{name}.jsonl"));
        assert_eq!(
            redacted_evidence(&file, "/Users/alice", &input),
            expected,
            "{name}"
        );
        leaks_exit_zero(&file, "/Users/alice", name);
    }
}

#[test]
fn doctor_leaks_still_reports_home_bytes_in_a_marker_component_tail() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("leaking.jsonl");
    // Accepting the tail behind a second marker costs no detection: the scan
    // matches at every index, and under r40 the exact dash home carries no
    // start boundary, so the nested home still reports at its own position.
    let record = leak_record("-Users-~<redacted>!-Users-alice");
    std::fs::write(&file, format!("{record}\n")).unwrap();
    assert_eq!(leak_lines(&file, "/Users/alice"), [1]);
}

#[test]
fn root_home_has_no_dash_form() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    // HOME=/ dash-encodes to a bare `-`, which is not an encoding: without
    // this guard every hyphen ahead of a boundary would read as home bytes
    // once r40 dropped the start boundary. Hyphenated text stays verbatim and
    // the gate stays quiet on both the fresh write and the raw line.
    let note = redacted_evidence(&file, "/", "artifact- plus x--y done");
    assert_eq!(note, "artifact- plus x--y done");
    leaks_exit_zero(&file, "/", "root home");
}

#[test]
fn exact_home_ends_at_a_structural_dash() {
    let temp = TempDir::new().unwrap();
    // r42: a `-` ends an exact slash-form home when the bytes it begins are
    // themselves a dash-encoded home form. The dash spelling always ends there,
    // because a `-` is its separator.
    for (name, home, input, expected) in [
        (
            "dash_home_follows",
            "/Users/alice",
            "-Users-/Users/alice-Users-alice",
            "-Users-~~",
        ),
        (
            "generic_dash_prefix_follows",
            "/Users/alice",
            "-Users-/Users/alice-Users-bob-x",
            "-Users-~-Users-bob-x",
        ),
        (
            "home_outside_the_generic_roots",
            "/var/root",
            "/var/root-Users-alice-x",
            "~-Users-alice-x",
        ),
    ] {
        let file = temp.path().join(format!("{name}.jsonl"));
        assert_eq!(redacted_evidence(&file, home, input), expected, "{name}");
        leaks_exit_zero(&file, home, name);
    }
}

#[test]
fn ordinary_dash_names_are_left_alone() {
    let temp = TempDir::new().unwrap();
    // In a slash path a `-` is an ordinary name byte, so a sibling account or a
    // longer component is a different directory and keeps its bytes. r40's
    // `-Users-alicexyz` is untouched too.
    for (name, input) in [
        ("sibling_branch", "feature/Users/alice-backup"),
        ("url_tail", "https://host/Users/alice-old"),
        ("digit_tail", "x/Users/alice2"),
        ("dot_tail", "x/Users/alice.bak"),
        ("dash_longer", "x-Users-alicexyz"),
    ] {
        let file = temp.path().join(format!("{name}.jsonl"));
        assert_eq!(
            redacted_evidence(&file, "/Users/alice", input),
            input,
            "{name}"
        );
        leaks_exit_zero(&file, "/Users/alice", name);
    }
}

#[test]
fn doctor_leaks_reports_the_r40_era_stored_line() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("r40-era.jsonl");
    // The spelling r40 stored for `-Users-/Users/alice-Users-alice`: the dash
    // home redacted, the slash home left standing against the marker that
    // replaced it. Those are real home bytes and the gate must name them.
    let record = leak_record("-Users-/Users/alice~");
    std::fs::write(&file, format!("{record}\n")).unwrap();
    assert_eq!(leak_lines(&file, "/Users/alice"), [1]);
}

#[test]
fn exact_home_ends_at_the_redaction_marker() {
    let temp = TempDir::new().unwrap();
    // The marker is blotter's own output, so home bytes standing against one
    // must still redact — otherwise an r40-era line can never be rewritten.
    for (name, input, expected) in [
        ("bare", "x/Users/alice~y", "x~~y"),
        ("behind_generic", "/Users//Users/alice~y", "/Users/~~y"),
    ] {
        let file = temp.path().join(format!("{name}.jsonl"));
        assert_eq!(
            redacted_evidence(&file, "/Users/alice", input),
            expected,
            "{name}"
        );
        leaks_exit_zero(&file, "/Users/alice", name);
    }
}

#[test]
fn doctor_leaks_accepts_a_marker_before_a_generic_dash_prefix() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("marker-dash.jsonl");
    // r42's one new composition: a slash-form match whose accepted `-` begins a
    // generic dash prefix leaves the marker inside a slash-form component,
    // because a `-` does not terminate one.
    assert_eq!(
        redacted_evidence(&file, "/Users/alice", "/Users//Users/alice-Users-bob-x"),
        "/Users/~-Users-bob-x"
    );
    leaks_exit_zero(&file, "/Users/alice", "marker before a generic dash prefix");

    // The acceptance costs no current-home detection: the exact dash home in an
    // accepted tail still reports at its own index.
    let leaking = temp.path().join("leaking.jsonl");
    let record = leak_record("/Users/~-Users-alice-x");
    std::fs::write(&leaking, format!("{record}\n")).unwrap();
    assert_eq!(leak_lines(&leaking, "/Users/alice"), [1]);
}

// A hand-written record whose text is JSON-encoded with every `/` escaped as
// `\/`, so the decoded value holds a home path that never appears as literal
// bytes on the physical line. Valid JSON; blotter's own encoder never writes it.
fn escaped_leak_line(text: &str) -> String {
    let id = compute_id(NOW, "tester", text, Impact::Low, &[]);
    let escaped = text
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('/', "\\/");
    format!(
        "{{\"v\":2,\"kind\":\"cut\",\"id\":\"{id}\",\"ts\":\"{NOW}\",\"agent\":\"tester\",\"text\":\"{escaped}\",\"tags\":[],\"impact\":\"low\",\"cwd\":\"/tmp/x\"}}"
    )
}

#[test]
fn doctor_leaks_reports_an_escaped_home_behind_an_accepted_marker() {
    let temp = TempDir::new().unwrap();
    // r43: the scanner reads the decoded layer, where an escaped home path is
    // literal `/Users/alice` again. On the raw layer these lines carry no
    // literal home bytes at all, and no widening of the raw rules reaches them.
    for (name, text) in [
        ("escaped_slashes", "/Users/~-/Users/alice"),
        ("escaped_newline", "/Users/~\n/Users/alice"),
    ] {
        let file = temp.path().join(format!("{name}.jsonl"));
        std::fs::write(&file, format!("{}\n", escaped_leak_line(text))).unwrap();
        assert_eq!(leak_lines(&file, "/Users/alice"), [1], "{name}");
    }
}

#[test]
fn doctor_leaks_accepts_encoder_escapes_on_a_valid_line() {
    let temp = TempDir::new().unwrap();
    // The encoder's backslash is not part of the contract: on the decoded layer
    // the component simply ends at the delimiter and the bare marker is
    // accepted, whatever the physical line spells.
    let file = temp.path().join("quote.jsonl");
    assert_eq!(
        redacted_evidence(&file, "/Users/alice", "/Users//Users/alice\""),
        "/Users/~\""
    );
    let raw = std::fs::read_to_string(&file).unwrap();
    assert!(
        raw.contains("~\\\""),
        "physical line carries the escape: {raw}"
    );
    leaks_exit_zero(&file, "/Users/alice", "marker against an escaped quote");

    let newline = temp.path().join("newline.jsonl");
    let stderr_file = temp.path().join("stderr.txt");
    std::fs::write(&stderr_file, "/Users//Users/alice\nrest").unwrap();
    let added: SuccessEnvelope<AddData> = success(
        &command()
            .env("HOME", "/Users/alice")
            .arg("--file")
            .arg(&newline)
            .args(["add", "stderr case", "--agent", "tester", "--stderr-file"])
            .arg(&stderr_file)
            .output()
            .unwrap(),
    );
    assert_eq!(
        added
            .data
            .record
            .cut_evidence()
            .unwrap()
            .stderr
            .as_deref()
            .unwrap(),
        "/Users/~\nrest"
    );
    let raw = std::fs::read_to_string(&newline).unwrap();
    assert!(
        raw.contains("~\\n"),
        "physical line carries the escape: {raw}"
    );
    leaks_exit_zero(
        &newline,
        "/Users/alice",
        "marker against an escaped newline",
    );
}

#[test]
fn doctor_leaks_still_scans_a_malformed_line_raw() {
    let temp = TempDir::new().unwrap();
    // A line that does not parse keeps today's raw scan, rules unchanged: r22
    // has the gate cover malformed lines, and that coverage survives r43.
    let leaking = temp.path().join("malformed-leak.jsonl");
    std::fs::write(&leaking, "{\"kind\":\"cut\" /Users/alice\n").unwrap();
    assert_eq!(leak_lines(&leaking, "/Users/alice"), [1]);

    // r41's raw acceptances are unchanged there: the doubled marker still passes.
    let accepted = temp.path().join("malformed-marker.jsonl");
    std::fs::write(&accepted, "{\"kind\":\"cut\" /Users/~~\n").unwrap();
    assert_eq!(leak_lines(&accepted, "/Users/alice"), Vec::<usize>::new());
}

#[test]
fn doctor_leaks_scans_unknown_fields_on_a_valid_line() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("unknown-field.jsonl");
    // r24 has unknown stored values pass through opaquely, so the decoded scan
    // walks the parsed value rather than the typed record: a field a future
    // release adds is covered without a change here.
    let id = compute_id(NOW, "tester", "note", Impact::Low, &[]);
    let line = format!(
        "{{\"v\":2,\"kind\":\"cut\",\"id\":\"{id}\",\"ts\":\"{NOW}\",\"agent\":\"tester\",\"text\":\"note\",\"tags\":[],\"impact\":\"low\",\"cwd\":\"/tmp/x\",\"future_field\":\"\\/Users\\/alice\"}}"
    );
    std::fs::write(&file, format!("{line}\n")).unwrap();
    assert_eq!(leak_lines(&file, "/Users/alice"), [1]);
}

#[test]
fn stderr_truncation_never_splits_the_secret_marker() {
    let temp = TempDir::new().unwrap();
    // The redacted text is "x"*n + " /Users/~" + "<redacted>", so the emitted
    // marker starts at n + 9. These two lengths put the 4096-byte cut one byte
    // and nine bytes inside it; the cut backtracks to the span's start, so no
    // partial marker is ever stored. The space before `/Users/` is required or
    // r23's start boundary declines the generic prefix.
    for (name, n) in [("cut_offset_1", 4086usize), ("cut_offset_9", 4078usize)] {
        let file = temp.path().join(format!("{name}.jsonl"));
        let stderr_file = temp.path().join(format!("{name}.txt"));
        std::fs::write(
            &stderr_file,
            format!("{} /Users//Users/alice/{ENTROPY_TOKEN}", "x".repeat(n)),
        )
        .unwrap();
        let added: SuccessEnvelope<AddData> = success(
            &command()
                .env("HOME", "/Users/alice")
                .arg("--file")
                .arg(&file)
                .args(["add", "stderr case", "--agent", "tester", "--stderr-file"])
                .arg(&stderr_file)
                .output()
                .unwrap(),
        );
        let stderr = added
            .data
            .record
            .cut_evidence()
            .unwrap()
            .stderr
            .clone()
            .unwrap();
        // r46 re-runs the home pass over the capped bytes, so the `/Users/~`
        // the backtrack left collapses one step further. The backtrack itself
        // is unchanged: the cut still lands at the span start, and no partial
        // marker is ever stored.
        assert_eq!(stderr, format!("{} ~", "x".repeat(n)), "{name}");
        assert!(stderr.len() <= 4096, "{name}: {}", stderr.len());
        assert!(!stderr.contains("<red"), "{name}: {stderr}");
        leaks_exit_zero(&file, "/Users/alice", name);
    }
}

#[test]
fn stderr_truncation_never_ends_in_an_exact_home() {
    let temp = TempDir::new().unwrap();
    // r46: the 4096-byte cap manufactures an end-of-input boundary the home
    // pass never judged, promoting r42's declined `x/Users/alice2` class into a
    // match after the only pass is over. The cap re-runs the home pass over the
    // bytes it kept, so the promoted home redacts instead of reaching the log.
    for (name, home_form) in [("slash", "/Users/alice"), ("dash", "-Users-alice")] {
        let file = temp.path().join(format!("{name}.jsonl"));
        let stderr_file = temp.path().join(format!("{name}.txt"));
        let padding = "z".repeat(4096 - home_form.len());
        std::fs::write(&stderr_file, format!("{padding}{home_form}XXXX")).unwrap();
        let added: SuccessEnvelope<AddData> = success(
            &command()
                .env("HOME", "/Users/alice")
                .arg("--file")
                .arg(&file)
                .args(["add", "stderr case", "--agent", "tester", "--stderr-file"])
                .arg(&stderr_file)
                .output()
                .unwrap(),
        );
        let stderr = added
            .data
            .record
            .cut_evidence()
            .unwrap()
            .stderr
            .clone()
            .unwrap();
        assert_eq!(
            stderr,
            format!("{}~", "z".repeat(4096 - home_form.len())),
            "{name}"
        );
        assert!(stderr.len() <= 4096, "{name}: {}", stderr.len());
        leaks_exit_zero(&file, "/Users/alice", name);
    }
}

#[test]
fn untruncated_stderr_keeps_the_r42_declined_home_class() {
    let temp = TempDir::new().unwrap();
    // The control the promotion is measured against: the same bytes, short
    // enough to skip the cap, store verbatim and pass the gate.
    //
    // This does NOT pin the `len <= max_bytes` gate. These bytes are a fixed
    // point of `rewrite_home_paths`, so one pass and two agree and the test
    // stays green with the gate removed. The gate is pinned incidentally by
    // `doctor_leaks_accepts_encoder_escapes_on_a_valid_line`, whose
    // `--stderr-file` value stores `/Users/~\nrest` and collapses to `~\nrest`
    // without it — the only failure in the suite under that mutation.
    for (name, home_form) in [("slash", "/Users/alice"), ("dash", "-Users-alice")] {
        let file = temp.path().join(format!("short_{name}.jsonl"));
        let stderr_file = temp.path().join(format!("short_{name}.txt"));
        let raw = format!("zzzzzzzzzz{home_form}XXXX");
        std::fs::write(&stderr_file, &raw).unwrap();
        let added: SuccessEnvelope<AddData> = success(
            &command()
                .env("HOME", "/Users/alice")
                .arg("--file")
                .arg(&file)
                .args(["add", "stderr case", "--agent", "tester", "--stderr-file"])
                .arg(&stderr_file)
                .output()
                .unwrap(),
        );
        let stderr = added
            .data
            .record
            .cut_evidence()
            .unwrap()
            .stderr
            .clone()
            .unwrap();
        assert_eq!(stderr, raw, "{name}");
        leaks_exit_zero(&file, "/Users/alice", name);
    }
}

#[test]
fn stderr_truncation_backtrack_survives_the_home_pass() {
    let temp = TempDir::new().unwrap();
    // AC#3, pinned on a shape the r46 home pass cannot touch: no `/` and no `-`
    // anywhere near the cut, so the stored bytes are the backtrack's alone.
    let file = temp.path().join("backtrack.jsonl");
    let stderr_file = temp.path().join("backtrack.txt");
    // The entropy rule spans `token=<value>` as one token starting at 4087, so
    // the marker occupies 4087..4097 and the 4096-byte cut falls inside it.
    std::fs::write(
        &stderr_file,
        format!("{} token={ENTROPY_TOKEN}", "x".repeat(4086)),
    )
    .unwrap();
    let added: SuccessEnvelope<AddData> = success(
        &command()
            .env("HOME", "/Users/alice")
            .arg("--file")
            .arg(&file)
            .args(["add", "stderr case", "--agent", "tester", "--stderr-file"])
            .arg(&stderr_file)
            .output()
            .unwrap(),
    );
    let stderr = added
        .data
        .record
        .cut_evidence()
        .unwrap()
        .stderr
        .clone()
        .unwrap();
    assert_eq!(stderr, format!("{} ", "x".repeat(4086)));
    assert!(!stderr.contains("<red"));
    leaks_exit_zero(&file, "/Users/alice", "backtrack");
}

#[test]
fn stderr_truncation_keeps_evidence_that_merely_looks_like_a_marker() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("lookalike.jsonl");
    let stderr_file = temp.path().join("stderr.txt");
    // The backtrack keys on provenance, not on the marker's spelling: no marker
    // was emitted here, so authentic evidence ending `~<red` keeps its bytes.
    std::fs::write(&stderr_file, format!("{}~<redZ", "x".repeat(4091))).unwrap();
    let added: SuccessEnvelope<AddData> = success(
        &command()
            .env("HOME", "/Users/alice")
            .arg("--file")
            .arg(&file)
            .args(["add", "stderr case", "--agent", "tester", "--stderr-file"])
            .arg(&stderr_file)
            .output()
            .unwrap(),
    );
    let stderr = added
        .data
        .record
        .cut_evidence()
        .unwrap()
        .stderr
        .clone()
        .unwrap();
    assert_eq!(stderr, format!("{}~<red", "x".repeat(4091)));
    assert_eq!(stderr.len(), 4096);
}

#[test]
fn doctor_leaks_still_reports_a_component_that_only_looks_like_a_marker() {
    let temp = TempDir::new().unwrap();
    // Every component outside the decoded enumeration is bytes the redactor
    // never wrote, so a near-miss of the secret marker stays a leak.
    for (name, cwd) in [
        ("username", "/Users/~abc"),
        ("marker_prefix", "/Users/~<reda"),
        ("marker_lookalike", "/Users/~<redx"),
    ] {
        let file = temp.path().join(format!("{name}.jsonl"));
        let record = leak_record(cwd);
        std::fs::write(&file, format!("{record}\n")).unwrap();
        assert_eq!(leak_lines(&file, "/Users/alice"), [1], "{name}");
    }
}

#[test]
fn doctor_leaks_scans_a_home_path_nested_in_an_unknown_structure() {
    let temp = TempDir::new().unwrap();
    // The decoded walk descends: an unknown field's inner objects, arrays, and
    // object keys are scanned at every depth, not just the line's top level.
    let id = compute_id(NOW, "tester", "note", Impact::Low, &[]);
    for (name, field) in [
        ("nested_value", "{\"a\":[{\"b\":\"\\/Users\\/alice\"}]}"),
        ("nested_key", "{\"a\":[{\"\\/Users\\/alice\":\"b\"}]}"),
    ] {
        let file = temp.path().join(format!("{name}.jsonl"));
        let line = format!(
            "{{\"v\":2,\"kind\":\"cut\",\"id\":\"{id}\",\"ts\":\"{NOW}\",\"agent\":\"tester\",\"text\":\"note\",\"tags\":[],\"impact\":\"low\",\"cwd\":\"/tmp/x\",\"future_field\":{field}}}"
        );
        std::fs::write(&file, format!("{line}\n")).unwrap();
        assert_eq!(leak_lines(&file, "/Users/alice"), [1], "{name}");
    }
}

#[test]
fn r42_marker_acceptance_does_not_reach_the_raw_layer() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("malformed.jsonl");
    // The raw layer's rule set is frozen at what shipped before r43, so the
    // `~-` member r42 adds on the decoded layer must not widen it: this
    // component still reports on a line that does not parse.
    std::fs::write(&file, "{\"kind\":\"cut\" /Users/~-x\n").unwrap();
    assert_eq!(leak_lines(&file, "/Users/alice"), [1]);
}
