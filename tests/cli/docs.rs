use crate::common::*;

/// Every published `hook exec claude-code` gate paired with a distinctive phrase
/// that must survive in the README's `## Hooks` section. `blotter schema` is the
/// source of truth; a gate that ships without prose is the drift this catches.
const HOOK_GATE_README_MARKERS: &[(&str, &str)] = &[
    ("hook_event_name", "after a failed Bash tool call"),
    ("tool_name", "failed Bash tool call"),
    ("is_interrupt", "ignores interrupts"),
    ("tool_input.command", "inapplicable payloads"),
    ("tool_input.command_bytes", "longer than 500 bytes"),
    ("tool_input.command_shape", "not a simple command"),
    ("tool_input.command_program", "read-only probe commands"),
    ("resolved_log_file", "never creates a blotter log"),
];

/// Published gates deliberately left out of the README prose. Every entry needs a
/// written reason beside it; empty means the prose covers the whole contract.
const HOOK_GATE_README_UNDOCUMENTED: &[&str] = &[];

fn readme_section(heading: &str) -> String {
    let readme =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md")).unwrap();
    let mut lines = readme.lines().skip_while(|line| line.trim_end() != heading);
    let found = lines
        .next()
        .unwrap_or_else(|| panic!("README has no {heading} section"));
    std::iter::once(found)
        .chain(lines.take_while(|line| !line.starts_with("## ")))
        .collect::<Vec<_>>()
        .join("\n")
}

fn number_word(count: usize) -> &'static str {
    match count {
        1 => "One",
        2 => "Two",
        3 => "Three",
        4 => "Four",
        5 => "Five",
        6 => "Six",
        7 => "Seven",
        8 => "Eight",
        9 => "Nine",
        10 => "Ten",
        other => panic!("no number word for {other} noise guards"),
    }
}

#[test]
fn readme_hook_prose_describes_every_published_hook_gate() {
    let schema: SuccessEnvelope<Value> = success(&run(&["schema"]));
    let gates = schema.data["commands"]["hook"]["exec"]["payload"]["gates"]
        .as_object()
        .unwrap();
    let markers: HashMap<&str, &str> = HOOK_GATE_README_MARKERS.iter().copied().collect();
    let section = readme_section("## Hooks");

    for gate in gates.keys() {
        if HOOK_GATE_README_UNDOCUMENTED.contains(&gate.as_str()) {
            continue;
        }
        let marker = markers.get(gate.as_str()).copied().unwrap_or_else(|| {
            panic!(
                "published hook gate {gate} has no README marker: describe it under ## Hooks and add it to HOOK_GATE_README_MARKERS"
            )
        });
        assert!(
            section.contains(marker),
            "README ## Hooks no longer describes hook gate {gate}: expected the phrase {marker:?}"
        );
    }

    // The README counts guards a reader can act on, not published gates: the three
    // `tool_input.command_*` gates plus the open-cut dedupe guard, which skips a
    // command an open cut already holds and is not a payload gate at all.
    let guards = gates
        .keys()
        .filter(|gate| gate.starts_with("tool_input.command_"))
        .count()
        + 1;
    let phrase = format!("{} noise guards apply", number_word(guards));
    assert!(
        section.contains(&phrase),
        "README ## Hooks must say {phrase:?}: one guard per published tool_input.command_* gate plus the open-cut dedupe guard"
    );
}

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
