use crate::common::*;

/// A file under `tests/cli/` enters the crate only through a `mod` line in
/// `main.rs`. Cargo auto-discovers test targets for `tests/*.rs` and for a
/// directory holding `main.rs`, so an undeclared sibling is compiled by nothing:
/// no error, no warning, no clippy diagnostic, and a gate that still reports
/// green while every test in it silently never runs. That is the same
/// silently-not-running-test failure the split exists to prevent, so it gets a
/// sentinel rather than a convention.
#[test]
fn every_test_module_file_is_declared_in_main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cli");
    let main = std::fs::read_to_string(dir.join("main.rs")).unwrap();

    let mut declared = main
        .lines()
        .filter_map(|line| line.strip_prefix("mod ")?.strip_suffix(';'))
        .map(str::to_string)
        .collect::<Vec<_>>();
    let mut present = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter_map(|name| Some(name.strip_suffix(".rs")?.to_string()))
        .filter(|stem| stem != "main")
        .collect::<Vec<_>>();
    declared.sort();
    present.sort();

    assert_eq!(
        declared, present,
        "tests/cli/main.rs must declare exactly the module files beside it; \
         an undeclared file is never compiled and its tests never run"
    );
}

/// Every test in this suite runs the uplifted binary, so a binary that cargo
/// failed to relink after a source edit makes the whole suite assert the old
/// behaviour and report green. That happened once (2026-09-03): `cargo build`
/// reported everything Fresh while `target/debug/blotter` sat seven hours
/// behind `src/cli.rs`, and only `cargo clean -p blotter-cli` recovered it.
/// A binary older than any source it is built from is never a correct build,
/// so the suite refuses to run against one.
#[test]
fn binary_under_test_is_newer_than_every_source_file() {
    let bin = Path::new(env!("CARGO_BIN_EXE_blotter"));
    let bin_mtime = std::fs::metadata(bin).unwrap().modified().unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));

    let mut pending = vec![root.join("src")];
    let mut sources = vec![root.join("Cargo.toml"), root.join("Cargo.lock")];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                sources.push(path);
            }
        }
    }

    for source in sources {
        let mtime = std::fs::metadata(&source).unwrap().modified().unwrap();
        assert!(
            mtime <= bin_mtime,
            "{} is newer than the binary under test ({}); cargo did not relink it, \
             so every test here would assert stale behaviour. Run \
             `cargo clean -p blotter-cli` and rebuild.",
            source.display(),
            bin.display()
        );
    }
}
