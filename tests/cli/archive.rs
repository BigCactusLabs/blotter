use crate::common::*;

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

fn archive_jsonl(value: Value) -> Vec<u8> {
    let mut line = serde_json::to_vec(&value).unwrap();
    line.push(b'\n');
    line
}

fn archive_cut(ts: &str, text: &str) -> (String, Vec<u8>) {
    let id = compute_id(ts, "archive", text, Impact::Low, &[]);
    let line = archive_jsonl(json!({
        "v": 2,
        "kind": "cut",
        "id": id,
        "ts": ts,
        "agent": "archive",
        "text": text,
        "tags": [],
        "impact": "low",
        "cwd": "/tmp"
    }));
    (id, line)
}

fn archive_dogear(ts: &str, text: &str) -> (String, Vec<u8>) {
    let id = compute_dogear_id(ts, "archive", text, &[]);
    let line = archive_jsonl(json!({
        "v": 2,
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

/// A resolve line for the archive fixtures. A resolve targeting a cut must
/// carry `disposition` and `disposition_ts` or the fold discards it as invalid
/// and the group never closes; a resolve targeting a dogear must carry neither.
/// Every v2 identity is one width (r51), so the kind cannot be read off the ID
/// and the caller states it. A non-`bl_` ID is only ever an orphan here, where
/// validity is never evaluated.
fn archive_resolution(id: &str, ts: &str, dropped: bool, amend: bool) -> Vec<u8> {
    archive_resolution_of(id, ts, dropped, amend, true)
}

fn archive_dogear_resolution(id: &str, ts: &str, dropped: bool, amend: bool) -> Vec<u8> {
    archive_resolution_of(id, ts, dropped, amend, false)
}

fn archive_resolution_of(
    id: &str,
    ts: &str,
    dropped: bool,
    amend: bool,
    disposition: bool,
) -> Vec<u8> {
    let mut value = json!({
        "v": 2,
        "kind": "resolve",
        "id": id,
        "ts": ts,
        "agent": "archive",
        "note": null,
        "dropped": dropped,
        "amend": amend
    });
    if disposition {
        value["disposition"] = json!("fixed");
        value["disposition_ts"] = json!(ts);
    }
    archive_jsonl(value)
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
    let old_drop = archive_dogear_resolution(&old_dogear_id, "2026-07-02T00:00:00Z", false, false);

    let (_, cutoff_cut) = archive_cut("2026-07-01T00:00:00Z", "cutoff is exclusive");
    let cutoff_id = compute_id(
        "2026-07-01T00:00:00Z",
        "archive",
        "cutoff is exclusive",
        Impact::Low,
        &[],
    );
    let cutoff_resolve = archive_resolution(&cutoff_id, cutoff, false, false);

    let orphan = archive_resolution(
        "bl_deadbeef000000000000",
        "2026-07-01T00:00:00Z",
        false,
        false,
    );
    let malformed = b"not json\n".to_vec();
    let unknown = archive_jsonl(json!({"v":2,"kind":"future","ts":"2026-07-01T00:00:00Z"}));
    let foreign = archive_jsonl(json!({
        "v": 2,
        "kind": "cut",
        "id": "zz_a1b2c3d4e5f6",
        "ts": "2026-07-01T00:00:00Z",
        "agent": "foreign",
        "text": "foreign-prefix closed cut",
        "tags": [],
        "impact": "low",
        "cwd": "/tmp"
    }));
    let foreign_resolve =
        archive_resolution("zz_a1b2c3d4e5f6", "2026-07-02T00:00:00Z", false, false);

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
        foreign,
        foreign_resolve,
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
fn archive_skips_the_leading_empty_segment_in_the_kept_count() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let (id, cut) = archive_cut("2026-07-01T00:00:00Z", "leading newline old");
    let resolve = archive_resolution(&id, "2026-07-02T00:00:00Z", false, false);
    let (_, open) = archive_cut("2026-07-03T00:00:00Z", "leading newline open");
    let original = [b"\n".to_vec(), cut, resolve, open.clone()].concat();
    std::fs::write(&file, &original).unwrap();

    let dry: SuccessEnvelope<Value> = success(&run_file(
        &file,
        &["archive", "--before", "2026-08-01T00:00:00Z", "--dry-run"],
    ));
    assert_eq!(dry.data["archived"], 2);
    assert_eq!(dry.data["kept"], 1);

    let apply: SuccessEnvelope<Value> = success(&run_file(
        &file,
        &["archive", "--before", "2026-08-01T00:00:00Z"],
    ));
    assert_eq!(apply.data["archived"], 2);
    assert_eq!(apply.data["kept"], 1);
    // The leading empty segment's byte survives the swap even though it is
    // not a physical line under the scan contract.
    assert_eq!(
        std::fs::read(&file).unwrap(),
        [b"\n".to_vec(), open].concat()
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
        "v": 2,
        "kind": "cut",
        "id": id,
        "ts": "2026-08-02T00:00:00Z",
        "agent": "archive",
        "text": "duplicate blocks archive",
        "tags": [],
        "impact": "low",
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
    let mut archive = spawn_command();
    archive
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
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
