use crate::common::*;

#[test]
fn valid_final_record_without_newline_is_accepted_not_resurrected() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    // A complete, valid cut record with NO trailing newline: a crash after the
    // object bytes but before the newline. JSON Lines permits this.
    let id = compute_id("2026-07-09T00:00:00.000Z", "a", "kept", Impact::Low, &[]);
    let record = json!({
        "v": 2,
        "kind": "cut", "id": id, "ts": "2026-07-09T00:00:00.000Z", "agent": "a",
        "text": "kept", "tags": [], "impact": "low", "cwd": "/tmp", "repo": null
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
                let envelope: SuccessEnvelope<ResolveData> = success(&run_file(
                    &file,
                    &["resolve", "--disposition", "fixed", &id, "--agent", "race"],
                ));
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
        Impact::Material,
        &["a".into(), "z".into()],
    );
    let b = compute_id(
        "2026-07-09T18:30:00.123Z",
        "tester",
        "ouc",
        Impact::Material,
        &["z".into(), "ha".into()],
    );
    let unsorted = compute_id(
        "2026-07-09T18:30:00.123Z",
        "tester",
        "ouch",
        Impact::Material,
        &["z".into(), "a".into()],
    );
    assert_eq!(a, "bl_edc887c6923de81fabd7");
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

#[cfg(unix)]
#[test]
fn a_parent_link_resolves_dot_dot_the_way_the_os_does() {
    let temp = TempDir::new().unwrap();
    let inner = temp.path().join("real/inner");
    std::fs::create_dir_all(&inner).unwrap();
    let work = temp.path().join("work");
    std::fs::create_dir(&work).unwrap();
    std::os::unix::fs::symlink(&inner, work.join("link")).unwrap();

    // `link/../cuts.jsonl`: the OS resolves `..` against the link's target, so
    // the log belongs beside the link target in real/, not in work/.
    let output = command()
        .current_dir(&work)
        .arg("--file")
        .arg("link/../cuts.jsonl")
        .args(["add", "through a symlinked parent", "--agent", "a"])
        .output()
        .unwrap();
    let envelope: SuccessEnvelope<AddData> = success(&output);
    let expected = temp
        .path()
        .canonicalize()
        .unwrap()
        .join("real")
        .join("cuts.jsonl");
    assert_eq!(
        Path::new(envelope.meta.file.as_deref().unwrap()),
        expected,
        "meta.file = {:?}",
        envelope.meta.file
    );
    assert!(expected.exists());
    assert!(!work.join("cuts.jsonl").exists());

    // A `..` under a component that does not exist yet still resolves: only the
    // nonexistent tail folds lexically.
    let planned = command()
        .current_dir(&work)
        .arg("--file")
        .arg("missing/../planned.jsonl")
        .args(["add", "not yet", "--agent", "a", "--dry-run"])
        .output()
        .unwrap();
    let planned: SuccessEnvelope<AddData> = success(&planned);
    assert_eq!(
        Path::new(planned.meta.file.as_deref().unwrap()),
        work.canonicalize().unwrap().join("planned.jsonl"),
        "meta.file = {:?}",
        planned.meta.file
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
fn adding_to_a_file_holding_only_a_newline_keeps_the_log_healthy() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    std::fs::write(&file, "\n").unwrap();
    let added = add(&file, "after the lone newline");
    assert!(added.data.changed);

    let output = run_file(&file, &["doctor"]);
    assert_eq!(output.status.code(), Some(0));
    let doctor: SuccessEnvelope<DoctorData> = serde_json::from_slice(&output.stdout).unwrap();
    assert!(doctor.data.healthy, "findings: {:?}", doctor.data.findings);
    assert_eq!(doctor.data.checked_lines, 1);

    let listed: SuccessEnvelope<ListData> = success(&run_file(&file, &["list"]));
    assert_eq!(listed.data.items.len(), 1);
    assert!(
        !listed
            .meta
            .warnings
            .iter()
            .any(|warning| warning.contains("malformed")),
        "warnings: {:?}",
        listed.meta.warnings
    );
}

#[test]
fn default_log_path_uses_repo_default_name() {
    let temp = TempDir::new().unwrap();
    assert_eq!(
        blotter::store::default_log_path(temp.path()),
        temp.path().join(".blotter.jsonl")
    );
}

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

/// Wait with a deliberately generous deadline so machine load cannot expire it.
/// The pre-fix FIFO behaviour was an unbounded block, so a true regression must
/// still fail this test rather than wedge the whole suite.
#[cfg(unix)]
fn wait_bounded(mut child: std::process::Child, what: &str) -> std::process::Output {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
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

/// r31 requires a directory log path to answer `invalid_input` (65) like every
/// other non-regular type. The read commands reached that answer through the
/// opened-handle check, but a mutation opens read+append, where the OS rejects a
/// directory before there is a handle to stat — so `add` alone answered 74.
#[cfg(unix)]
#[test]
fn a_directory_log_path_is_rejected_on_read_and_mutation_alike() {
    let temp = TempDir::new().unwrap();
    let directory = temp.path().join("log-directory");
    std::fs::create_dir(&directory).unwrap();
    for (what, args) in non_regular_log_cases() {
        assert_non_regular_log(&run_file(&directory, &args), what);
    }
}

/// The tear-healing newline is a byte, and a refusal writes none. A v1 log whose
/// final line is unterminated must come back byte-identical.
#[test]
fn a_refused_log_is_not_tear_healed_before_the_probe() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let original = v1_cut_line();
    assert!(!original.ends_with('\n'));
    std::fs::write(&file, &original).unwrap();

    error(
        &run_file(&file, &["add", "new cut", "--agent", "tester"]),
        65,
        "unsupported_log_version",
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), original);
}

/// An empty file — no bytes, or the single newline `scan` reads as a terminator
/// rather than a line (r26/r33) — is a fresh v2 log and passes the probe.
#[test]
fn an_empty_log_passes_the_probe() {
    let temp = TempDir::new().unwrap();
    for (name, bytes) in [("empty.jsonl", &b""[..]), ("newline.jsonl", &b"\n"[..])] {
        let file = temp.path().join(name);
        std::fs::write(&file, bytes).unwrap();
        let listed: SuccessEnvelope<ListData> = success(&run_file(&file, &["list"]));
        assert_eq!(listed.data.items.len(), 0, "{name}");
        let added: SuccessEnvelope<AddData> =
            success(&run_file(&file, &["add", "fresh", "--agent", "tester"]));
        assert!(added.data.changed, "{name}");
    }
}

/// r50: what passes the probe is a log holding **no line with a known raw
/// kind** — not-JSON lines, JSON with an unknown `kind`, JSON with no `kind`, a
/// torn tail, or any mixture. The scan still classifies them exactly as before,
/// so `doctor --fix` can still repair the one thing it exists to repair.
#[test]
fn a_log_without_a_known_kind_passes_the_probe() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    std::fs::write(
        &file,
        "not json\n{\"kind\":\"future\"}\n{\"id\":\"bl_aaaaaaaaaaaaaaaaaaaa\"}\n{\"kind\":",
    )
    .unwrap();

    let listed: SuccessEnvelope<ListData> = success(&run_file(&file, &["list"]));
    assert_eq!(listed.data.items.len(), 0);
    assert!(
        listed
            .meta
            .warnings
            .iter()
            .all(|warning| !warning.contains("version")),
        "warnings: {:?}",
        listed.meta.warnings
    );

    let doctor = doctor_response(&run_file(&file, &["doctor"]), 1);
    assert!(
        doctor
            .data
            .findings
            .iter()
            .all(|finding| finding.kind != "unsupported_version"),
        "findings: {:?}",
        doctor.data.findings
    );

    // The residual r48 states rather than papers over: such a log is repairable
    // by the one command written to repair it.
    let fixed = doctor_response(&run_file(&file, &["doctor", "--fix"]), 1);
    assert!(fixed.data.fix.as_ref().unwrap().changed);

    let added: SuccessEnvelope<AddData> = success(&run_file(
        &file,
        &["add", "after repair", "--agent", "tester"],
    ));
    assert!(added.data.changed);
}
