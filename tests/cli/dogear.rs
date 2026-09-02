use crate::common::*;

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
    assert!(record.get("impact").is_none());
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
        &run_file(&file, &["list", "--kind", "dogear", "--impact", "low"]),
        2,
        "invalid_argument",
    );

    let malformed = temp.path().join("malformed-dogear.jsonl");
    std::fs::write(
        &malformed,
        "{\"v\":2,\"kind\":\"dogear\",\"id\":\"bl_bad\",\"ts\":\"not-a-time\"}\n",
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

    let cut_two_tags = compute_id(ts, "x", "t", Impact::Low, &["a".into(), "b".into()]);
    let cut_one_comma_tag = compute_id(ts, "x", "t", Impact::Low, &["a,b".into()]);
    assert_ne!(cut_two_tags, cut_one_comma_tag);

    // Dogear ids are 80-bit (bl_ + 20 hex) and, being a different length from
    // the 48-bit cut id, can never collide with the cut namespace.
    assert_eq!(two_tags.len(), 3 + 20);
    assert_eq!(cut_two_tags.len(), 3 + 12);
    assert_ne!(two_tags, cut_two_tags);
}
