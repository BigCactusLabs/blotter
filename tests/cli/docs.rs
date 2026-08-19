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
