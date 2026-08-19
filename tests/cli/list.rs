use crate::common::*;

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
