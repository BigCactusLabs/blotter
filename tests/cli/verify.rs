use crate::common::*;

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
        json!({"recurrences": [], "count": 0, "distinct_recurring_cuts": 0, "scanned": 1})
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
                    "origin": {"type":"agent"},
                    "resolved_text": "Cache configuration missing",
                    "resolution": {
                        "ts": "2026-07-09T18:31:00.000Z",
                        "disposition": "fixed",
                        "disposition_ts": "2026-07-09T18:31:00.000Z",
                        "task": "TASK-VERIFY",
                        "pr": "https://github.com/BigCactusLabs/blotter/pull/99",
                        "commit": "abc123",
                    },
                    "recurrence_ids": [recurring.data.record.cut_id()],
                    "count": 1,
                    "first_recurrence_ts": "2026-07-09T18:32:00.000Z",
                }],
                "count": 1,
                "distinct_recurring_cuts": 1,
                "scanned": 1,
            },
            "meta": {"contract": 6, "file": file},
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
        "v": 2,
        "kind": "resolve",
        "id": resolved.data.record.cut_id(),
        "ts": "2026-07-09T18:31:00.000Z",
        "agent": "fixture",
        "note": null,
        "dropped": true,
        "disposition": "fixed",
        "disposition_ts": "2026-07-09T18:31:00.000Z",
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
fn verify_excludes_an_accepted_anchor_with_a_later_linked_open_cut() {
    // r48: accepted is a deliberate decision to tolerate friction, so its
    // recurrence is expected, not a failed intervention. An accepted
    // resolution is never an anchor.
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
        &["--agent", "fixer", "--disposition", "accepted"],
    );
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
fn verify_excludes_an_invalid_anchor_with_a_later_linked_open_cut() {
    // r48: invalid says the anchor was never friction at all.
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
        &["--agent", "fixer", "--disposition", "invalid"],
    );
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
fn verify_fixed_and_promoted_anchors_both_report_a_recurrence() {
    let temp = TempDir::new().unwrap();

    let fixed_file = temp.path().join("fixed.jsonl");
    let fixed_resolved = add_at(
        &fixed_file,
        "2026-07-09T18:30:00Z",
        "Cache configuration missing",
        &["ops"],
    );
    let _: SuccessEnvelope<ResolveData> = resolve_at(
        &fixed_file,
        "2026-07-09T18:31:00Z",
        fixed_resolved.data.record.cut_id(),
        &["--agent", "fixer", "--disposition", "fixed"],
    );
    add_at(
        &fixed_file,
        "2026-07-09T18:32:00Z",
        "cache configuration missing",
        &["ops"],
    );
    let fixed = verify_success(&run_file(&fixed_file, &["verify"]), 1);
    assert_eq!(fixed.data["count"], 1);
    assert_eq!(
        fixed.data["recurrences"][0]["resolution"]["disposition"],
        "fixed"
    );

    let promoted_file = temp.path().join("promoted.jsonl");
    let promoted_resolved = add_at(
        &promoted_file,
        "2026-07-09T18:30:00Z",
        "Cache configuration missing",
        &["ops"],
    );
    let promotion: SuccessEnvelope<PromoteData> = success(&promote_at(
        &promoted_file,
        "2026-07-09T18:31:00Z",
        &[
            "--source",
            promoted_resolved.data.record.cut_id(),
            "--artifact-type",
            "doc",
            "--artifact-ref",
            "docs/cache.md",
        ],
    ));
    let promotion_record_id = promotion_id(&promotion.data.record);
    let _: SuccessEnvelope<ResolveData> = resolve_at(
        &promoted_file,
        "2026-07-09T18:32:00Z",
        promoted_resolved.data.record.cut_id(),
        &[
            "--agent",
            "fixer",
            "--disposition",
            "promoted",
            "--promotion",
            &promotion_record_id,
        ],
    );
    add_at(
        &promoted_file,
        "2026-07-09T18:33:00Z",
        "cache configuration missing",
        &["ops"],
    );
    let promoted = verify_success(&run_file(&promoted_file, &["verify"]), 1);
    assert_eq!(promoted.data["count"], 1);
    assert_eq!(
        promoted.data["recurrences"][0]["resolution"]["disposition"],
        "promoted"
    );
}

#[test]
fn verify_disposition_amend_from_fixed_to_accepted_removes_the_anchor_and_back_restores_it() {
    // A1(4): an amend that moves fixed -> accepted removes the anchor, and
    // accepted -> fixed (re-deciding, new disposition_ts) restores it with
    // the boundary at the new disposition_ts.
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
        "2026-07-09T18:31:00Z", // T1
        resolved.data.record.cut_id(),
        &["--agent", "fixer", "--disposition", "fixed"],
    );
    let recurring = add_at(
        &file,
        "2026-07-09T18:32:00Z", // T2 > T1
        "cache configuration missing",
        &["ops"],
    );

    let fixed = verify_success(&run_file(&file, &["verify"]), 1);
    assert_eq!(fixed.data["count"], 1);

    let _: SuccessEnvelope<ResolveData> = resolve_at(
        &file,
        "2026-07-09T18:33:00Z", // T3 > T2, disposition-changing amend
        resolved.data.record.cut_id(),
        &[
            "--amend",
            "--disposition",
            "accepted",
            "--agent",
            "corrector",
        ],
    );

    let accepted = verify_success(&run_file(&file, &["verify"]), 0);
    assert_eq!(accepted.data["recurrences"], json!([]));

    let _: SuccessEnvelope<ResolveData> = resolve_at(
        &file,
        "2026-07-09T18:34:00Z", // T4 > T3, re-deciding amend back to fixed
        resolved.data.record.cut_id(),
        &[
            "--amend",
            "--disposition",
            "fixed",
            "--agent",
            "re-corrector",
        ],
    );

    // The boundary now sits at T4, strictly after the T2 open cut, so that
    // cut no longer links even though the anchor is eligible again; it takes
    // a cut logged after T4 to recur.
    let too_early = verify_success(&run_file(&file, &["verify"]), 0);
    assert_eq!(too_early.data["recurrences"], json!([]));
    let _ = recurring;

    let later_recurring = add_at(
        &file,
        "2026-07-09T18:35:00Z", // T5 > T4
        "cache configuration missing",
        &["ops"],
    );

    let restored = verify_success(&run_file(&file, &["verify"]), 1);
    assert_eq!(restored.data["count"], 1);
    assert_eq!(
        restored.data["recurrences"][0]["resolution"]["disposition_ts"],
        "2026-07-09T18:34:00.000Z"
    );
    assert_eq!(
        restored.data["recurrences"][0]["recurrence_ids"],
        json!([later_recurring.data.record.cut_id()])
    );
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
fn verify_does_not_link_filler_words_but_keeps_a_content_recurrence() {
    // TASK-64. Four records, every one tagged `tooling`, so a shared tag is
    // present and provably not what separates the pairs. N is 4 — two open cuts
    // plus two eligible anchors — so `rare_limit` is `max(2, ceil(4/4))` = 2 and
    // every shared token is locally rare. That is the regime where a frequency
    // ceiling cannot tell filler from content, which is why r44 moves the
    // filter and leaves the ceiling alone.
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    // Under r19 this anchor and the open cut below shared exactly
    // `{would, not, only}`: three rare tokens and one tag, and a reported
    // recurrence.
    let filler_anchor = add_at(
        &file,
        "2026-07-09T18:30:00Z",
        "backlog fetch would not write only reference",
        &["tooling"],
    );
    let _: SuccessEnvelope<ResolveData> = resolve_at(
        &file,
        "2026-07-09T18:31:00Z",
        filler_anchor.data.record.cut_id(),
        &["--agent", "fixer"],
    );
    let filler_open = add_at(
        &file,
        "2026-07-09T18:32:00Z",
        "patch apply would not reverse only diff",
        &["tooling"],
    );
    // Eight shared content tokens, none of them in any stopword list. Overlap
    // is 8/11 = 0.727, below the 4/5 bar, so the rare path alone carries this
    // pair before and after the change.
    let content_anchor = add_at(
        &file,
        "2026-07-09T18:33:00Z",
        "raster codec module map lookup returns stale entry rebuild index cache",
        &["tooling"],
    );
    let _: SuccessEnvelope<ResolveData> = resolve_at(
        &file,
        "2026-07-09T18:34:00Z",
        content_anchor.data.record.cut_id(),
        &["--agent", "fixer"],
    );
    let content_open = add_at(
        &file,
        "2026-07-09T18:35:00Z",
        "raster codec module map lookup returns stale entry differs probe journal",
        &["tooling"],
    );

    let verify = verify_success(&run_file(&file, &["verify"]), 1);
    assert_eq!(verify.data["count"], 1);
    assert_eq!(verify.data["scanned"], 2);
    let recurrences = verify.data["recurrences"].as_array().unwrap();
    assert_eq!(
        recurrences[0]["resolved_id"],
        json!(content_anchor.data.record.cut_id())
    );
    assert_eq!(
        recurrences[0]["recurrence_ids"],
        json!([content_open.data.record.cut_id()])
    );
    assert!(
        !recurrences.iter().any(|recurrence| {
            recurrence["recurrence_ids"]
                .as_array()
                .unwrap()
                .iter()
                .any(|id| id.as_str() == Some(filler_open.data.record.cut_id()))
        }),
        "a pair sharing one tag and three English function words is not a recurrence"
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
fn verify_note_only_amend_does_not_move_the_disposition_ts_boundary() {
    // r51: verify partitions against the winning resolution's disposition_ts,
    // not its ts. A note-only --amend changes ts but inherits disposition_ts
    // unchanged, so a cut logged between the fix and the note must still be a
    // recurrence, and the rest of the envelope must not move either.
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
        "2026-07-09T18:31:00Z", // T1
        resolved.data.record.cut_id(),
        &["--agent", "fixer", "--note", "base"],
    );
    let recurring = add_at(
        &file,
        "2026-07-09T18:32:00Z", // T2 > T1
        "cache configuration missing",
        &["ops"],
    );

    let before = verify_success(&run_file(&file, &["verify"]), 1);

    let _: SuccessEnvelope<ResolveData> = resolve_at(
        &file,
        "2026-07-09T18:33:00Z", // T3 > T2, note-only amend
        resolved.data.record.cut_id(),
        &["--amend", "--agent", "corrector", "--note", "corrected"],
    );

    let after = verify_success(&run_file(&file, &["verify"]), 1);

    let mut expected = before.data.clone();
    expected["recurrences"][0]["resolution"]["ts"] = json!("2026-07-09T18:33:00.000Z");
    assert_eq!(after.data, expected);

    assert_eq!(
        after.data["recurrences"][0]["resolution"]["disposition_ts"],
        "2026-07-09T18:31:00.000Z"
    );
    assert_eq!(
        after.data["recurrences"][0]["recurrence_ids"],
        json!([recurring.data.record.cut_id()])
    );
    let first_recurrence_ts = after.data["recurrences"][0]["first_recurrence_ts"]
        .as_str()
        .unwrap();
    let resolution_ts = after.data["recurrences"][0]["resolution"]["ts"]
        .as_str()
        .unwrap();
    assert!(
        first_recurrence_ts < resolution_ts,
        "first_recurrence_ts {first_recurrence_ts} should precede the displayed resolution.ts {resolution_ts} after a note-only amend"
    );
}

#[test]
fn verify_disposition_changing_amend_moves_the_disposition_ts_boundary() {
    // r51: when an amend re-decides the disposition, disposition_ts moves
    // with it, and the boundary the recurrence scan uses moves too.
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
        "2026-07-09T18:31:00Z", // T1
        resolved.data.record.cut_id(),
        &["--agent", "fixer", "--note", "base"],
    );
    add_at(
        &file,
        "2026-07-09T18:32:00Z", // T2 > T1
        "cache configuration missing",
        &["ops"],
    );
    let _: SuccessEnvelope<ResolveData> = resolve_at(
        &file,
        "2026-07-09T18:33:00Z", // T3 > T2, disposition-changing amend
        resolved.data.record.cut_id(),
        &[
            "--amend",
            "--disposition",
            "fixed",
            "--agent",
            "corrector",
            "--note",
            "corrected",
        ],
    );
    let recurring = add_at(
        &file,
        "2026-07-09T18:34:00Z", // T4 > T3
        "cache configuration missing",
        &["ops"],
    );

    let verify = verify_success(&run_file(&file, &["verify"]), 1);
    // Both open cuts (the one at T2 that no longer recurs, and the one at T4
    // that does) are scanned; disposition_ts only changes which ones link.
    assert_eq!(verify.data["scanned"], 2);
    assert_eq!(
        verify.data["recurrences"][0]["resolution"]["disposition_ts"],
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
fn verify_reports_one_open_cut_against_every_matching_anchor_and_link_path() {
    // The recurrence scan prefilters open cuts per anchor, so this pins the
    // parts a prefilter could quietly drop: an anchor whose tags miss the open
    // cut still matches on an identical title, an anchor with a different title
    // still matches through shared tokens plus a shared tag, no anchor claims
    // the open cut away from the others, and an open cut that predates every
    // resolution is still excluded even though it matches on title.
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let title_anchor = add_at(
        &file,
        "2026-07-09T18:30:00Z",
        "Cache configuration missing",
        &["alpha"],
    );
    let other_tag_anchor = add_at(
        &file,
        "2026-07-09T18:30:10Z",
        "cache configuration MISSING",
        &["beta"],
    );
    let token_anchor = add_at(
        &file,
        "2026-07-09T18:30:20Z",
        "Cache configuration missing entirely",
        &["gamma"],
    );
    let before_resolution = add_at(
        &file,
        "2026-07-09T18:30:30Z",
        "cache configuration missing",
        &["delta"],
    );
    for (index, anchor) in [&title_anchor, &other_tag_anchor, &token_anchor]
        .iter()
        .enumerate()
    {
        let _: SuccessEnvelope<ResolveData> = resolve_at(
            &file,
            &format!("2026-07-09T18:31:{:02}Z", index * 10),
            anchor.data.record.cut_id(),
            &["--agent", "fixer"],
        );
    }
    let recurring = add_at(
        &file,
        "2026-07-09T18:32:00Z",
        "CACHE CONFIGURATION MISSING!",
        &["gamma"],
    );

    let verify = verify_success(&run_file(&file, &["verify"]), 1);
    assert_eq!(verify.data["count"], 3);
    assert_eq!(verify.data["scanned"], 2);
    let recurrences = verify.data["recurrences"].as_array().unwrap();
    let mut expected_resolved_ids = vec![
        title_anchor.data.record.cut_id().to_owned(),
        other_tag_anchor.data.record.cut_id().to_owned(),
        token_anchor.data.record.cut_id().to_owned(),
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
            "2026-07-09T18:32:00.000Z"
        );
        assert_ne!(
            recurrence["recurrence_ids"][0],
            json!(before_resolution.data.record.cut_id())
        );
    }
}

#[test]
fn verify_counts_anchors_and_distinct_recurring_cuts() {
    // TASK-65. r16 makes every eligible resolved cut an independent anchor, so
    // one returning problem reports once per historical cut it resembles.
    // `count` stays that anchor count -- a top-level `count` is the length of
    // the primary array in every blotter envelope -- and
    // `distinct_recurring_cuts` is the number of live problems behind it.
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let first_anchor = add_at(
        &file,
        "2026-07-09T18:30:00Z",
        "Cache warmer drops the sled index",
        &["ops"],
    );
    let _: SuccessEnvelope<ResolveData> = resolve_at(
        &file,
        "2026-07-09T18:31:00Z",
        first_anchor.data.record.cut_id(),
        &["--agent", "fixer"],
    );
    let second_anchor = add_at(
        &file,
        "2026-07-09T18:32:00Z",
        "cache warmer drops the sled index!",
        &["ops"],
    );
    let _: SuccessEnvelope<ResolveData> = resolve_at(
        &file,
        "2026-07-09T18:33:00Z",
        second_anchor.data.record.cut_id(),
        &["--agent", "fixer"],
    );
    // Different raw text from both anchors' resolutions onward, so a distinct
    // ID, but the same normalized title: it links to each anchor on the
    // exact-title override.
    let recurring = add_at(
        &file,
        "2026-07-09T18:34:00Z",
        "Cache warmer drops the sled index",
        &["ops"],
    );

    let verify = verify_success(&run_file(&file, &["verify"]), 1);
    assert_eq!(verify.data["count"], 2);
    assert_eq!(verify.data["distinct_recurring_cuts"], 1);
    assert_eq!(verify.data["scanned"], 1);
    for recurrence in verify.data["recurrences"].as_array().unwrap() {
        assert_eq!(
            recurrence["recurrence_ids"],
            json!([recurring.data.record.cut_id()])
        );
    }

    // A second live problem with its own anchor, proving the field counts
    // distinct cuts rather than clamping to one. It needs a third anchor: the
    // two above have identical normalized titles and identical tags, so no cut
    // can link to one of them without linking to the other.
    let third_anchor = add_at(
        &file,
        "2026-07-09T18:35:00Z",
        "Compaction thread stalls on the write barrier",
        &["ops"],
    );
    let _: SuccessEnvelope<ResolveData> = resolve_at(
        &file,
        "2026-07-09T18:36:00Z",
        third_anchor.data.record.cut_id(),
        &["--agent", "fixer"],
    );
    let second_recurring = add_at(
        &file,
        "2026-07-09T18:37:00Z",
        "compaction thread stalls on the write barrier",
        &["ops"],
    );

    let verify = verify_success(&run_file(&file, &["verify"]), 1);
    assert_eq!(verify.data["count"], 3);
    assert_eq!(verify.data["distinct_recurring_cuts"], 2);
    assert_eq!(verify.data["scanned"], 2);
    let recurrences = verify.data["recurrences"].as_array().unwrap();
    assert_eq!(recurrences.len(), 3);
    for recurrence in &recurrences[..2] {
        assert_eq!(
            recurrence["recurrence_ids"],
            json!([recurring.data.record.cut_id()])
        );
    }
    assert_eq!(
        recurrences[2]["resolved_id"],
        json!(third_anchor.data.record.cut_id())
    );
    assert_eq!(
        recurrences[2]["recurrence_ids"],
        json!([second_recurring.data.record.cut_id()])
    );
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
                "origin": {"type":"agent"},
                    "resolved_text": "Cache configuration missing",
                "resolution": {
                    "ts": "2026-07-09T18:31:00.000Z",
                    "disposition": "fixed",
                    "disposition_ts": "2026-07-09T18:31:00.000Z",
                },
                "recurrence_ids": [first.data.record.cut_id(), late_first.data.record.cut_id()],
                "count": 2,
                "first_recurrence_ts": "2026-07-09T18:33:00.000Z",
            },
            {
                "resolved_id": second_anchor.data.record.cut_id(),
                "origin": {"type":"agent"},
                    "resolved_text": "Build cache checksum mismatch",
                "resolution": {
                    "ts": "2026-07-09T18:31:00.000Z",
                    "disposition": "fixed",
                    "disposition_ts": "2026-07-09T18:31:00.000Z",
                },
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
    assert!(verify["flags"].get("--include-auto").is_none());
    assert_eq!(
        verify["output"],
        "{recurrences:[{resolved_id,resolved_text,origin?,resolution:{ts,disposition,disposition_ts,task?,pr?,commit?},recurrence_ids,count,first_recurrence_ts}],count,distinct_recurring_cuts,scanned}"
    );
    let semantics = verify["semantics"].as_str().unwrap();
    assert!(semantics.contains("filtered-token rule"));
    assert!(semantics.contains("disposition_ts"));
    assert!(semantics.contains("no recurrence was observed"));
    assert!(semantics.contains(
        "anchors are resolved cuts whose winning disposition is fixed or promoted; accepted and invalid are excluded"
    ));
    assert_eq!(
        verify["exit_codes"],
        json!({"0":"no recurrences","1":"recurrences found"})
    );
    assert_eq!(verify["read_only"], true);
    assert_eq!(verify["appends"], false);
    assert_eq!(verify["destructive"], false);
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
