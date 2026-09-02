use crate::common::*;

#[test]
fn list_filters_sorts_limits_since_and_markdown() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let cases = [
        ("2026-07-01T00:00:00Z", "old blocking", "blocking", "ops"),
        ("2026-07-09T17:00:00Z", "new low", "low", "shell"),
        ("2026-07-09T18:00:00Z", "new material", "material", "ops"),
    ];
    for (now, text, impact, tag) in cases {
        let output = command()
            .env("BLOTTER_NOW", now)
            .arg("--file")
            .arg(&file)
            .args([
                "add", text, "--agent", "tester", "--impact", impact, "--tag", tag,
            ])
            .output()
            .unwrap();
        success::<AddData>(&output);
    }
    let limited: SuccessEnvelope<ListData> = success(&run_file(&file, &["list", "--limit", "1"]));
    assert_eq!(limited.data.items[0].record().text, "old blocking");
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
    assert_eq!(since.data.items[0].record().text, "new material");

    let markdown = run_file(&file, &["list", "--format", "md", "--impact", "material"]);
    assert!(markdown.status.success());
    assert!(markdown.stderr.is_empty());
    let markdown = String::from_utf8(markdown.stdout).unwrap();
    assert!(markdown.starts_with("## Material\n"));
    assert!(markdown.contains("new material — tester"));
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
fn list_md_renders_warnings_as_trailing_note_lines() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let added = add(&file, "valid markdown row");
    let mut log = std::fs::read(&file).unwrap();
    log.extend_from_slice(b"{ malformed physical line\n");
    std::fs::write(&file, log).unwrap();

    let output = run_file(&file, &["list", "--format", "md"]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let markdown = String::from_utf8(output.stdout).unwrap();
    let row = format!("- [{}] valid markdown row", added.data.record.cut_id());
    let note = "> note: skipped 1 malformed line\n";
    assert!(markdown.contains(&row));
    assert!(markdown.ends_with(note), "{markdown}");
    assert!(markdown.find(note).unwrap() > markdown.find(&row).unwrap());
}

#[test]
fn list_md_renders_multiple_warnings_in_envelope_order() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let added = add(&file, "valid markdown row");
    let mut log = std::fs::read(&file).unwrap();
    // A malformed physical line (complete, but not parseable JSON), followed by
    // a torn final line (an incomplete record with no trailing newline). The
    // fold counts these separately (`counts.malformed`, `counts.torn`), and
    // `store::fold` emits warnings in a fixed order — torn before malformed —
    // so this pins md-follows-envelope rather than an incidental ordering.
    log.extend_from_slice(b"{ malformed physical line\n");
    log.extend_from_slice(b"{\"kind\":\"cut\"");
    std::fs::write(&file, log).unwrap();

    let json_output: SuccessEnvelope<ListData> = success(&run_file(&file, &["list"]));
    assert_eq!(
        json_output.meta.warnings,
        ["skipped 1 torn final line", "skipped 1 malformed line"]
    );

    let output = run_file(&file, &["list", "--format", "md"]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let markdown = String::from_utf8(output.stdout).unwrap();
    let row = format!("- [{}] valid markdown row", added.data.record.cut_id());
    let torn_note = "> note: skipped 1 torn final line\n";
    let malformed_note = "> note: skipped 1 malformed line\n";
    assert!(markdown.contains(&row));
    assert!(markdown.ends_with(malformed_note), "{markdown}");
    assert!(
        markdown.find(torn_note).unwrap() < markdown.find(malformed_note).unwrap(),
        "{markdown}"
    );
    assert!(markdown.find(torn_note).unwrap() > markdown.find(&row).unwrap());
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
            "## Low\n- [{}] first line second line third line — tester, 2026-07-09T18:30:00.123Z\n",
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
            "--disposition",
            "fixed",
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
            "## Low\n- [~~{}~~] the cut — tester, 2026-07-09T18:30:00.123Z\n  - resolved 2026-07-09T18:30:00.123Z by resolver (d34db33fd34db33f) pr https://github.com/BigCactusLabs/blotter/pull/25 task TASK-25: fixed it\n",
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
            "--disposition",
            "fixed",
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
            "## Low\n- [~~{}~~] the cut — tester, 2026-07-09T18:30:00.123Z\n  - resolved 2026-07-09T18:30:00.123Z by multi line resolver (d34db33f ## heading) task TASK 25\n",
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
            "--disposition",
            "fixed",
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
            .record()
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
            "## Low\n- [~~{}~~] the cut — tester, 2026-07-09T18:30:00.123Z\n  - resolved 2026-07-09T18:30:00.123Z by resolver: first line second line third line\n",
            added.data.record.cut_id()
        )
    );
}

#[test]
fn list_sorts_rfc3339_offsets_by_instant_not_text() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("offsets.jsonl");
    let earlier = json!({"v":2,"kind":"cut","id":"bl_11111111111111111111","ts":"2026-07-09T10:00:00+02:00","agent":"a","text":"earlier","tags":[],"impact":"low","cwd":"/tmp","repo":null});
    let later = json!({"v":2,"kind":"cut","id":"bl_22222222222222222222","ts":"2026-07-09T09:00:00Z","agent":"a","text":"later","tags":[],"impact":"low","cwd":"/tmp","repo":null});
    std::fs::write(&file, format!("{earlier}\n{later}\n")).unwrap();
    let listed: SuccessEnvelope<ListData> = success(&run_file(&file, &["list"]));
    assert_eq!(listed.data.items[0].record().text, "later");
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

/// r48: `--severity` is removed rather than aliased, so clap rejects it as an
/// unknown argument on both the commands that carried it, and `--impact` takes
/// its place with the new vocabulary.
#[test]
fn severity_is_removed_and_impact_replaces_it() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    for args in [
        &["add", "x", "--severity", "minor"][..],
        &["list", "--severity", "minor"][..],
    ] {
        error(&run_file(&file, args), 2, "invalid_argument");
    }
    for value in ["minor", "major", "blocker"] {
        error(
            &run_file(&file, &["add", "x", "--impact", value]),
            2,
            "invalid_argument",
        );
    }

    // The default is `low`, and the sort ranks blocking > material > low.
    add_at(&file, "2026-07-09T18:00:00Z", "default impact", &[]);
    let listed: SuccessEnvelope<ListData> = success(&run_file(&file, &["list"]));
    assert_eq!(
        serde_json::to_value(listed.data.items[0].record().impact).unwrap(),
        json!("low")
    );

    let blocking = run_file(
        &file,
        &[
            "add", "stopped", "--agent", "tester", "--impact", "blocking",
        ],
    );
    success::<AddData>(&blocking);
    let listed: SuccessEnvelope<ListData> = success(&run_file(&file, &["list"]));
    assert_eq!(listed.data.items[0].record().text, "stopped");
}

/// A log holding one open cut, one dogear, and two promotions.
fn mixed_log(file: &Path) -> (String, String, String) {
    let cut = add_at(file, "2026-07-01T00:00:00Z", "friction", &["build"]);
    let cut = cut.data.record.cut_id().to_owned();
    dogear_at(file, "2026-07-02T00:00:00Z", "idea", &["build"]);
    let first: SuccessEnvelope<PromoteData> = success(&promote_at(
        file,
        "2026-07-03T00:00:00Z",
        &[
            "--source",
            &cut,
            "--artifact-type",
            "doc",
            "--artifact-ref",
            "docs/a.md",
        ],
    ));
    let second: SuccessEnvelope<PromoteData> = success(&promote_at(
        file,
        "2026-07-04T00:00:00Z",
        &[
            "--source",
            &cut,
            "--artifact-type",
            "skill",
            "--artifact-ref",
            "skills/b.md",
            "--note",
            "second",
        ],
    ));
    (
        cut,
        promotion_id(&first.data.record),
        promotion_id(&second.data.record),
    )
}

#[test]
fn kind_promotion_lists_promotions_only_newest_first() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("log.jsonl");
    let (cut, older, newer) = mixed_log(&file);

    let listed: SuccessEnvelope<ListData> =
        success(&run_file(&file, &["list", "--kind", "promotion"]));
    let promotions = list_promotions(&listed.data.items);
    assert_eq!(listed.data.count, 2);
    assert_eq!(promotions.len(), 2);
    // ts descending, then id ascending (r48).
    assert_eq!(promotions[0].id, newer);
    assert_eq!(promotions[1].id, older);
    assert_eq!(promotions[0].kind, "promotion");
    assert_eq!(promotions[0].sources, [cut]);
    assert_eq!(promotions[0].artifact.kind, ArtifactType::Skill);
    assert_eq!(promotions[0].note.as_deref(), Some("second"));
    assert_eq!(promotions[0].origin, Some(Origin::agent()));

    // A promotion item carries no lifecycle or friction members at all.
    let raw: SuccessEnvelope<Value> = success(&run_file(&file, &["list", "--kind", "promotion"]));
    let item = &raw.data["items"][0];
    for absent in ["status", "resolution", "text", "tags", "impact", "evidence"] {
        assert!(item.get(absent).is_none(), "{absent}");
    }
}

#[test]
fn kind_all_appends_promotions_after_cuts_and_dogears() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("log.jsonl");
    let (_, older, newer) = mixed_log(&file);

    let listed: SuccessEnvelope<Value> = success(&run_file(&file, &["list", "--kind", "all"]));
    let kinds: Vec<_> = listed.data["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["cut", "dogear", "promotion", "promotion"]);
    assert_eq!(listed.data["items"][2]["id"], newer.as_str());
    assert_eq!(listed.data["items"][3]["id"], older.as_str());

    // `--status all` also retains them; an explicit lifecycle status does not.
    let all: SuccessEnvelope<ListData> = success(&run_file(
        &file,
        &["list", "--kind", "all", "--status", "all"],
    ));
    assert_eq!(list_promotions(&all.data.items).len(), 2);
    for status in ["open", "resolved"] {
        let filtered: SuccessEnvelope<ListData> = success(&run_file(
            &file,
            &["list", "--kind", "all", "--status", status],
        ));
        assert!(list_promotions(&filtered.data.items).is_empty(), "{status}");
    }
    // `--tag` excludes them under `--kind all`.
    let tagged: SuccessEnvelope<ListData> = success(&run_file(
        &file,
        &["list", "--kind", "all", "--tag", "build"],
    ));
    assert!(list_promotions(&tagged.data.items).is_empty());
}

#[test]
fn promotion_filters_that_cannot_select_are_rejected() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("log.jsonl");
    mixed_log(&file);

    for args in [
        vec!["list", "--kind", "promotion", "--status", "open"],
        vec!["list", "--kind", "promotion", "--status", "resolved"],
        vec!["list", "--kind", "promotion", "--tag", "build"],
        vec!["list", "--kind", "promotion", "--impact", "low"],
    ] {
        error(&run_file(&file, &args), 2, "invalid_argument");
    }
    // `--status all` is accepted and is a no-op.
    let listed: SuccessEnvelope<ListData> = success(&run_file(
        &file,
        &["list", "--kind", "promotion", "--status", "all"],
    ));
    assert_eq!(listed.data.count, 2);
}

#[test]
fn promotions_honour_agent_and_since_and_have_their_own_empty_hint() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("log.jsonl");
    mixed_log(&file);

    let since: SuccessEnvelope<ListData> = success(&run_file(
        &file,
        &[
            "list",
            "--kind",
            "promotion",
            "--since",
            "2026-07-04T00:00:00Z",
        ],
    ));
    assert_eq!(since.data.count, 1);

    let empty: SuccessEnvelope<ListData> = success(&run_file(
        &file,
        &["list", "--kind", "promotion", "--agent", "nobody"],
    ));
    assert_eq!(empty.data.count, 0);
    // No `--status` in the hint: promotions have none.
    assert_eq!(
        empty.meta.warnings,
        ["no promotions matched; try broader filters"]
    );
}

#[test]
fn markdown_renders_promotions_after_dogears() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("log.jsonl");
    let (_, older, newer) = mixed_log(&file);

    let output = run_file(&file, &["list", "--kind", "all", "--format", "md"]);
    assert!(output.status.success());
    let rendered = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<_> = rendered.lines().collect();
    let promotions = lines
        .iter()
        .position(|line| *line == "## Promotions")
        .unwrap();
    let dogears = lines.iter().position(|line| *line == "## Dogears").unwrap();
    assert!(dogears < promotions);
    assert_eq!(
        lines[promotions + 1],
        format!("- [{newer}] skill: skills/b.md — tester, 2026-07-04T00:00:00.000Z")
    );
    assert_eq!(lines[promotions + 2], "  - second");
    assert_eq!(
        lines[promotions + 3],
        format!("- [{older}] doc: docs/a.md — tester, 2026-07-03T00:00:00.000Z")
    );
    assert_eq!(lines.len(), promotions + 4);
}

/// The union is untagged, so serde picks its arm structurally rather than from
/// the `kind` string. Disjointness is what makes that safe, and it is pinned
/// here rather than assumed: only a lifecycle record carries `status`, and only
/// a promotion carries `sources`.
#[test]
fn the_items_union_arms_are_structurally_disjoint() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("log.jsonl");
    mixed_log(&file);

    let raw: SuccessEnvelope<Value> = success(&run_file(
        &file,
        &["list", "--kind", "all", "--status", "all"],
    ));
    let items = raw.data["items"].as_array().unwrap();
    let cut = items
        .iter()
        .find(|item| item["kind"] == "cut")
        .unwrap()
        .clone();
    let promotion = items
        .iter()
        .find(|item| item["kind"] == "promotion")
        .unwrap()
        .clone();

    // Each arm deserializes only as itself.
    assert!(serde_json::from_value::<ListItem>(cut.clone()).is_ok());
    assert!(serde_json::from_value::<PromotionItem>(cut.clone()).is_err());
    assert!(serde_json::from_value::<PromotionItem>(promotion.clone()).is_ok());
    assert!(serde_json::from_value::<ListItem>(promotion.clone()).is_err());

    // And the union routes each to the arm its shape names, with `Record` tried
    // first, so a promotion that fell through to it would be caught here.
    let entries: Vec<ListEntry> = serde_json::from_value(json!([cut, promotion])).unwrap();
    assert!(entries[0].as_record().is_some());
    assert!(entries[0].as_promotion().is_none());
    assert!(entries[1].as_promotion().is_some());
    assert!(entries[1].as_record().is_none());
}
