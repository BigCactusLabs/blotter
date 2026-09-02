use crate::common::*;

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
            "origin": {"type":"agent"},
        }])
    );
}

#[test]
fn triage_cluster_carries_no_suggested_action() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    add_at(
        &file,
        "2026-07-09T18:30:00Z",
        "Cargo test fails because config is missing",
        &["api", "tooling"],
    );
    add_at(
        &file,
        "2026-07-09T18:31:00Z",
        "cargo-test fails because config is missing!",
        &["tooling"],
    );
    add_at(
        &file,
        "2026-07-09T18:32:00Z",
        "Cargo test fails because config is missing again",
        &["api"],
    );

    let triage = triage_success(&run_file(&file, &["triage"]), 1);
    let cluster = &triage.data["clusters"][0];
    assert!(
        cluster
            .as_object()
            .unwrap()
            .get("suggested_action")
            .is_none(),
        "suggested_action was withdrawn (r51) and must not appear on a triage cluster"
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

/// Pins r53: the local-rarity ceiling is `max(2, ceil(N / 16))`, not
/// `max(2, ceil(N / 4))`. Both corpora scan N = 16 open cuts, so the ceiling
/// is 4 under the retired divisor and 2 under r53. Two target cuts share
/// exactly three non-stopword tokens (`frobnicator`, `gadget`, `timeout`)
/// that fail the overlap-coefficient path (3 shared of 5 tokens each is
/// below 80%) and so depend entirely on the rare-token path, which needs
/// `MIN_RARE_SHARED_TOKENS = 3`.
///
/// In the first corpus a third, differently-tagged cut also carries all
/// three tokens, so their document frequency is 3: rare under divisor 4
/// (3 <= 4) but not under divisor 16 (3 > 2), so the targets must not link.
/// In the second corpus that third cut is replaced by a token-disjoint filler
/// (keeping N = 16), so the shared tokens' document frequency drops to 2 and
/// they are rare under both floors — the targets link, showing this is a
/// boundary case rather than an absence of clustering altogether.
#[test]
fn triage_rare_ceiling_is_one_sixteenth_of_scanned_cuts() {
    fn padding_cuts(file: &std::path::Path, start_minute: u32, count: u32) {
        for i in 0..count {
            add_at(
                file,
                &format!("2026-07-09T19:{:02}:00Z", start_minute + i),
                &format!("padding filler note paddertoken{i} notetoken{i}"),
                &[],
            );
        }
    }

    // First corpus: the shared tokens also occur in a third, off-tag cut, so
    // their document frequency is 3.
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let first = add_at(
        &file,
        "2026-07-09T18:30:00Z",
        "frobnicator gadget timeout alpha1token beta1token",
        &["tooling"],
    );
    let second = add_at(
        &file,
        "2026-07-09T18:31:00Z",
        "frobnicator gadget timeout gamma2token delta2token",
        &["tooling"],
    );
    add_at(
        &file,
        "2026-07-09T18:32:00Z",
        "frobnicator gadget timeout epsilon3token zeta3token",
        &["other"],
    );
    padding_cuts(&file, 0, 13);

    let not_linked = triage_success(&run_file(&file, &["triage", "--min-count", "2"]), 0);
    assert_eq!(not_linked.data["scanned"], 16);
    let has_both_targets = not_linked.data["clusters"]
        .as_array()
        .expect("clusters is an array")
        .iter()
        .any(|cluster| {
            let ids = cluster["ids"].as_array().expect("ids is an array");
            ids.contains(&json!(first.data.record.cut_id()))
                && ids.contains(&json!(second.data.record.cut_id()))
        });
    assert!(
        !has_both_targets,
        "targets must not cluster while the shared tokens' df is 3: {:?}",
        not_linked.data["clusters"]
    );

    // Second corpus: same N and same targets, but the shared tokens occur in
    // no other cut, so their document frequency is 2 and they clear the r53
    // floor as well as the retired ceiling.
    let temp2 = TempDir::new().unwrap();
    let file2 = temp2.path().join("cuts.jsonl");
    let first2 = add_at(
        &file2,
        "2026-07-09T18:30:00Z",
        "frobnicator gadget timeout alpha1token beta1token",
        &["tooling"],
    );
    let second2 = add_at(
        &file2,
        "2026-07-09T18:31:00Z",
        "frobnicator gadget timeout gamma2token delta2token",
        &["tooling"],
    );
    padding_cuts(&file2, 0, 14);

    let linked = triage_success(&run_file(&file2, &["triage", "--min-count", "2"]), 1);
    assert_eq!(linked.data["scanned"], 16);
    assert_eq!(
        linked.data["clusters"][0]["ids"],
        json!([first2.data.record.cut_id(), second2.data.record.cut_id()])
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
fn triage_does_not_cluster_english_filler_with_a_shared_tag() {
    // Linkage lives in triage, so the r44 stopword list is pinned here as well
    // as at the verify surface TASK-64 names. Under r19 these two cuts shared
    // exactly `would`, `not` and `only` — three tokens that are locally rare in
    // any corpus small enough to write as a fixture — plus one tag, which is
    // the reported defect.
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    add_at(
        &file,
        "2026-07-09T18:30:00Z",
        "backlog fetch would not write only reference",
        &["tooling"],
    );
    add_at(
        &file,
        "2026-07-09T18:32:00Z",
        "patch apply would not reverse only diff",
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
            "origin": {"type":"agent"},
        }])
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
            "origin": {"type":"agent"},
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
            "origin": {"type":"agent"},
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
        &[
            "resolve",
            "--disposition",
            "fixed",
            first.data.record.cut_id(),
            "--agent",
            "fixer",
        ],
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
            "origin": {"type":"agent"},
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
    assert!(triage["flags"].get("--include-auto").is_none());
    assert_eq!(
        triage["output"],
        "{clusters:[{count,occurrences,ids,tags,text,origin?}],count,scanned}"
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
