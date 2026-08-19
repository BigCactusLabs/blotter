use crate::common::*;

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
