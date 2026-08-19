# AGENTS.md — blotter

Machine-facing contract for agents working in this repo.

`CLAUDE.md` is a symlink to this file. Edit `AGENTS.md` — some editing tools refuse to write through the symlink.

## What this is

`blotter` is a Rust CLI (clap 4 derive) that lets AI agents log friction into an append-only JSONL file. Agent-only tool: JSON envelopes on stdout, structured errors on stderr, stable exit codes. The normative contract is `docs/plans/2026-07-09-papercuts-design.md` (written under the pre-rename name) — treat it as law. Amendments accumulate and the newest wins, so read to the **last** amendment in the file and work back from there; do not trust a revision number quoted anywhere else, including here. Earlier sections an amendment supersedes are history, not current contract. The Amendments sections record review provenance and deliberate deviations from the rust-agent-cli skill (exit 74 extension, diagnose-only doctor, no --quiet).

Nothing else under `docs/` is contract. `docs/superpowers/specs/` holds specs for behaviour in the **current** release, each carrying a `Status:` line — the directory is often absent, and that is normal: archiving the last spec empties it and git does not track an empty directory, so 0.14.0 and 0.15.0 both shipped with no spec file. Current-release behaviour is then the design doc's newest amendments plus `blotter schema`, not a missing page. `docs/archive/` holds everything superseded — shipped plans and specs whose release is no longer current. (The pre-fork remediation records were deleted in the TASK-33 open-sourcing scrub: they leaked private detail in prose, and provenance does not survive a privacy conflict.) `docs/research/` holds one-off research notes. When any of it disagrees with the design doc, the design doc wins.

Archive a spec when the release it describes stops being the newest, or when the surface it added is gone. Add the archived date to its `Status:` line and leave the body as written — an archived doc is provenance, not a page to keep current.

## Build and gate

```bash
cargo build --release
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --check
```

All four must pass before any commit. Run the test suite 5x when touching store.rs or anything concurrency-adjacent — a single green run proves nothing about races. `scripts/dev/gate-5x.sh` does exactly that and keeps every run's output, so a failure that does not reproduce still names the test that failed; counting `ok` lines and discarding the rest loses the one thing the fifth run exists to capture.

For a fast iterative test loop, run `scripts/dev/test-fast.sh`. It uses `cargo nextest run --all-features` when cargo-nextest is installed and safely falls back to `cargo test --all-features`. This does not replace the required pre-commit `cargo test --all-features` gate above.

The minimum supported Rust version (MSRV) is the `rust-version` in `Cargo.toml`, currently 1.89. It is a compatibility floor, not a moving latest-stable target: raise it only when a required language or dependency feature justifies narrowing installability. After dependency changes and before a release, install the declared toolchain with `rustup toolchain install 1.89.0 --profile minimal` and run `scripts/dev/check-msrv.sh`; the script tests the locked all-features tree with that exact toolchain.

For focused Rust tests, `cargo test` accepts one positional filter. To run several unrelated tests in one invocation, pass the extra filters to libtest after `--`: `cargo test --test cli first_filter -- second_filter` runs the union of both.

## Layout

- `src/store.rs` — file discovery, locking (bounded try_lock → exit 75), append (write_all + tear-heal + rollback), the normative fold. The riskiest file; change with care and tests.
- `src/commands/*.rs` — one file per subcommand; `dogear.rs` is the idea-log mutation parallel to `add.rs`, `triage.rs` is the read-only chronic-cut analyzer over folded open cuts under a shared lock, `digest.rs` reuses that analyzer for the periodic one-log report, `verify.rs` is the read-only recurrence check that flags resolved cuts whose friction reappears (triage's linkage rules, resolved anchors vs later open cuts), `retrospect.rs` is the read-only promotion-mining pass built on triage's clustering and verify's recurrence rules, `sweep.rs` is the only command that reads several logs (one shared lock at a time, arguments only — no `BLOTTER_FILE`, no global `--file`), `export.rs` is the read-only OTLP bridge and the one raw-output command besides `--format md`, `archive.rs` rewrites the log by the same copy-and-swap mechanic as `doctor --fix` and carries the same change-with-care bar as `store.rs`, and `hook.rs` owns fail-open harness integrations plus atomic external-settings installation. The shared `is_auto_capture` predicate filters folded items immediately after the fold and before command-specific analysis. Mutations run read→fold→decide→append inside one exclusive-lock critical section.
- `src/error.rs` — the public error contract (codes ↔ exit codes). Never add an undocumented code.
- `src/output.rs` — envelope types. Every output shape is a serde struct.
- `tests/cli/` — the black-box assert_cmd suite: one integration-test binary (`main.rs` declares the modules) split by subject, so parallel work on different commands does not collide at one file's tail. Add a test to the module that owns the behaviour it asserts — `add`, `dogear`, `resolve`, `list`, `doctor`, `archive`, `triage`, `verify`, `retrospect`, `digest`, `sweep`, `hook`, `export`, `store` (discovery, locking, races, torn tails, non-regular paths), `redaction`, `stderr_file`, `legacy`, `auto_capture`, `docs` (prose drift gates), and `contract` for anything spanning commands (exit matrix, envelope shapes, `--since`). A `schema_documents_*` test belongs with the command it documents, not in `contract`. Shared helpers live in `common.rs` and every module reaches them through one `use crate::common::*;` — move a helper there only when a second module needs it, and leave it in the owning module otherwise. Env via `Command::env` only, never `std::env::set_var` (parallel-test races).

## Task backlog

Project tasks live in `backlog/` (Backlog.md, agent-only). Use the CLI, never hand-edit task files: `backlog task list --plain`, `backlog task create "..."`, `backlog task edit <id> -s "In Progress"`. Always pass `--plain` when reading.

## Dogfood

Important: when you hit a small friction while working here — a tool call that missed and had to be retried, a confusing or undocumented setup step, a flaky command, a stale cache, a misleading error, a non-obvious gotcha — log it in the moment:

```bash
cargo run -- add "what you were doing → what got in the way" --tag <area>
```

Some signals don't feel like friction from the inside; they still count: an error that doesn't point at the fix or blames the wrong file, a stack trace landing somewhere benign instead of the root cause, falling back to memory because the docs didn't answer, the user correcting something you got wrong. And two rules: build failures are friction, not stopping points — log and keep going; don't ask mid-run questions — log the uncertainty instead.

One or two sentences: what you were doing → what got in the way (a guess at the cause/fix is a bonus; put it in `--evidence`). Default severity is `minor` — use `--severity major` only if it cost real time, `blocker` only if it stopped the task. Do this proactively even though none of these are blocking — logged together they show where the repo needs sanding down.

This is distinct from the backlog (`backlog/` — real tracked work) and from dogears (`cargo run -- dogear "..."` — ideas and "we should someday" thoughts, not friction). If it's a genuine bug you're going to fix now, it's a backlog task; if it merely slowed you down, it's a cut.

## Invariants (do not break)

- Append-only: nothing rewrites the log file EXCEPT `doctor --fix` and `archive`, which never edit in place — each writes a replacement copy and atomically swaps, always preserving the original as a timestamped backup. Ordinary mutations only add bytes by appending (including the tear-healing `\n`).
- stdout = data only, one envelope; stderr = errors only. `--format md` and `export --format otlp-json` are the only raw-output exceptions.
- Deterministic: same input + `BLOTTER_NOW` → byte-identical output.
- Empty results are exit 0. Not-found IDs are 66. Lock timeout is 75 + `retryable:true`.
- Cut-only list output stays cut-only unless `--kind dogear` or `--kind all` is explicit; dogear records have no severity.
- Records tagged `auto` are excluded from `list`, `triage`, `digest`, `verify`, `sweep`, and `export` unless `--include-auto` is explicit. On `list`, `--tag auto` implies `--include-auto`.
