use crate::common::*;

#[cfg(unix)]
#[test]
fn stderr_file_requires_a_regular_file_and_follows_regular_file_symlinks() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let target = temp.path().join("stderr.txt");
    let link = temp.path().join("stderr-link.txt");
    std::fs::write(&target, "ordinary stderr").unwrap();
    symlink(&target, &link).unwrap();
    let added: SuccessEnvelope<AddData> = success(
        &command()
            .arg("--file")
            .arg(&file)
            .args(["add", "symlink evidence", "--stderr-file"])
            .arg(&link)
            .output()
            .unwrap(),
    );
    assert_eq!(
        added.data.record.cut_evidence().unwrap().stderr.as_deref(),
        Some("ordinary stderr")
    );

    let fifo = temp.path().join("stderr.fifo");
    let made_fifo = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .is_ok_and(|status| status.success());
    if made_fifo {
        let mut child = std::process::Command::new(assert_cmd::cargo::cargo_bin!("blotter"))
            .env("BLOTTER_NOW", NOW)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .arg("--file")
            .arg(&file)
            .args([
                "add",
                "fifo evidence",
                "--stderr-file",
                fifo.to_str().unwrap(),
            ])
            .spawn()
            .unwrap();
        // Generous guard: the O_NONBLOCK open cannot block on the FIFO, so this
        // only trips if a blocking open is reintroduced. One second flaked under
        // parallel-suite load (spawn latency alone can exceed it).
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if std::time::Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("FIFO evidence read blocked");
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        assert_eq!(status.code(), Some(65));
        let rejected = child.wait_with_output().unwrap();
        let envelope = error(&rejected, 65, "invalid_input");
        assert!(envelope.error.message.contains("not a regular file"));
        assert!(envelope.error.suggested_fix.contains("FIFOs and devices"));
    }
}

#[cfg(unix)]
#[test]
fn stderr_file_errors_are_structured_and_specific() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let temp = TempDir::new().unwrap();
    let file = temp.path().join("cuts.jsonl");
    let invoke = |path: &Path| {
        run_file(
            &file,
            &[
                "add",
                "bad evidence",
                "--stderr-file",
                path.to_str().unwrap(),
            ],
        )
    };

    let oversized = temp.path().join("oversized.txt");
    std::fs::write(&oversized, vec![b'x'; 1024 * 1024 + 1]).unwrap();
    let oversized = error(&invoke(&oversized), 65, "invalid_input");
    assert!(
        oversized
            .error
            .message
            .contains("exceeds the 1048576-byte read limit")
    );
    assert!(
        oversized
            .error
            .suggested_fix
            .contains("smaller stderr file")
    );

    let invalid_utf8 = temp.path().join("invalid-utf8.txt");
    std::fs::write(&invalid_utf8, [0xff]).unwrap();
    let invalid_utf8 = error(&invoke(&invalid_utf8), 65, "invalid_input");
    assert!(invalid_utf8.error.message.contains("not valid UTF-8"));
    assert!(
        invalid_utf8
            .error
            .suggested_fix
            .contains("UTF-8 stderr file")
    );

    let directory = temp.path().join("stderr-directory");
    std::fs::create_dir(&directory).unwrap();
    let directory_error = error(&invoke(&directory), 65, "invalid_input");
    assert!(directory_error.error.message.contains("not a regular file"));
    assert!(
        directory_error
            .error
            .suggested_fix
            .contains("regular UTF-8 file")
    );

    let link = temp.path().join("stderr-directory-link");
    symlink(&directory, &link).unwrap();
    let link = error(&invoke(&link), 65, "invalid_input");
    assert!(link.error.message.contains("not a regular file"));
    assert!(link.error.suggested_fix.contains("FIFOs and devices"));

    let unreadable = temp.path().join("unreadable.txt");
    std::fs::write(&unreadable, "stderr").unwrap();
    std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o000)).unwrap();
    let output = invoke(&unreadable);
    std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o600)).unwrap();
    let unreadable = error(&output, 77, "permission_denied");
    assert!(
        unreadable
            .error
            .message
            .starts_with("permission denied reading stderr evidence file")
    );
    assert!(
        unreadable
            .error
            .suggested_fix
            .contains("Grant read permission")
    );
}
