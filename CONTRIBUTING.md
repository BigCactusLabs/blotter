# Contributing

Start with [the documentation map](docs/README.md). The [current contract](docs/contract.md) separates trust guarantees, supported behavior, and revisitable choices. Historical discussions are optional context.

## Set up

Use a Rust toolchain at or above `package.rust-version` in [Cargo.toml](Cargo.toml), with Clippy and rustfmt. Python 3.11+ supports documentation and experiments, not the installed CLI.

```bash
python3 -m venv .venv
. .venv/bin/activate
python -m pip install --only-binary=:all: -r site/requirements.txt
```

Keep the environment untracked. Do not run host installers, credentialed publication scripts, or real-ledger writes merely to test documentation.

## Validate a change

Iterate with owning tests; `scripts/dev/test-fast.sh` uses nextest when available, otherwise cargo test. Before landing, run the checks for affected surfaces and inspect actual CI results. Checking whether a surface is affected does not require editing it. CI retains the full Rust suite.

| Changed surface | Validation |
| --- | --- |
| Rust behavior, helpers, dependencies | Rust suite; applicable interface checks |
| Storage, locking, archive, repair, concurrency | Rust suite plus `scripts/dev/gate-5x.sh`; retain all five outputs |
| Markdown or docs tooling | Source docs checks; runtime checks for CLI examples/interface; site tests for published inputs/rendering |
| Skill, plugin, distribution tooling | Discovery checks; real installers for installation/delivery changes |
| Presentation experiment | Owning Python tests and real-binary integration tests |

### Rust

```bash
cargo build --release
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --check
```

After dependency changes and before releases, run `scripts/dev/check-msrv.sh` against the locked tree using the declared Rust floor. The fast loop does not replace the full suite. Retain the five-run storage gate until a targeted replacement demonstrates equivalent relevant coverage.

### Documentation and site

```bash
python3 -m unittest discover -s tests/docs -v
python3 scripts/dev/check-docs.py
```

These check links, anchors, navigation, entry-point size, and the `CLAUDE.md` symlink. For changed CLI examples or interfaces, build the release binary and run:

```bash
python3 scripts/dev/check-docs.py --binary target/release/blotter
```

This compares command coverage to `schema` and runs the marked README quickstart in a disposable repository, not arbitrary Markdown commands. Missing binary coverage is explicitly skipped.

For consumer-site inputs/rendering, run `python3 -m unittest discover -s tests/site -v`. The builder checks rendered links; inputs must be committed and the destination absent:

```bash
python3 scripts/dev/build-discovery-site.py --output /tmp/blotter-docs-preview
```

### Discovery

```bash
python3 -m unittest discover -s tests/discovery -p 'test_*.py' -v
python3 scripts/dev/check-discovery.py --binary target/release/blotter
```

Build that binary first. Follow [installer validation](docs/publication.md#installer-validation) for host installation/delivery changes. Ordinary docs edits no longer trigger real installers. Manual input `check` in **Agent discovery** selects `installers`, `catalog`, or `all`; catalog observations do not run on pushes or PRs.

Report what ran locally, ran in CI, failed, or remains unverified. Environment limitations are not passes. Do not change required-status settings to accommodate skipped work.

## Try an idea

Use the existing task or PR for the hypothesis, scope, and keep-or-delete evidence. No ADR or experiment registry is needed. Preserve durable rationale for costly-to-reverse choices or new trust boundaries; a past deferral is not a permanent veto.

[Patterns before prescriptions](experiments/patterns.py) combines existing readers without changing the stable CLI:

```bash
python3 experiments/patterns.py --binary target/release/blotter --file /path/to/review.jsonl
python3 -m unittest discover -s tests/experiments -v
BLOTTER_TEST_BINARY="$PWD/target/release/blotter" python3 -m unittest discover -s tests/experiments -v
```

Choose an authorized ledger. The preview uses `triage --min-count 2`, matching retrospect's threshold without changing triage's default. Unknown remedies no longer hide patterns; advice joins only identical source-ID sets. It writes no records. `--format json` is experimental, not a supported schema. Use a quiescent ledger: two reads are not an atomic snapshot, and detected changes fail with a retry message. Review private text before sharing.

Missing `BLOTTER_TEST_BINARY` explicitly skips integration tests; CI supplies it. Tests use synthetic ledgers. Keep the prototype only if useful extra patterns outweigh misleading matches, not merely because it emits more results.

## Find the implementation

| Area | Files |
| --- | --- |
| CLI parsing and flags | [src/cli.rs](src/cli.rs) |
| Envelope and contract number | [src/output.rs](src/output.rs) |
| Errors and exits | [src/error.rs](src/error.rs) |
| Executable schema | [src/commands/schema.rs](src/commands/schema.rs) |
| Discovery, locks, fold, append, repair | [src/store.rs](src/store.rs) |
| Commands and black-box tests | [src/commands](src/commands), [tests/cli](tests/cli) |
| Documentation/distribution tooling | [scripts/dev](scripts/dev) |

## Test ownership and troubleshooting

Put black-box tests in the owning `tests/cli/` module and declare new modules in `main.rs`. Use existing cross-cutting modules instead of duplicating cases. Share helpers when a second module needs them. Set subprocess environment through `Command::env`, never process-global mutation.

For suspected stale binaries, inspect timestamps, then `cargo clean -p blotter-cli`, rebuild, and rerun the failing test. Never use `.blotter.jsonl` as a smoke-test fixture.

## Keep documentation single-purpose

README introduces; the skill teaches agent procedure; reference explains behavior; `schema` inventories the machine interface; contract records guarantees and compatibility-sensitive semantics. Runbooks own external operations. Link instead of copying whole sections.

Update affected surfaces for behavior changes, including Unreleased for shipped changes. Private refactors need no contract edit; moving prose needs no changelog entry. Use concrete runnable examples and returned IDs, not enum notation or invented quickstart IDs. Keep schema inventories executable and site prose single-source.

Every current `docs/` page must be linked directly from the documentation map; other current pages must be reachable from it. The docs checker enforces this. Keep current decisions in their owning page, superseded rationale in Git, and historical reports out of mandatory reading.

## Backlog

Use the Backlog CLI, not direct Markdown edits:

```bash
backlog task list --plain
backlog task create "A concrete unit of work"
backlog task edit TASK_ID -s "In Progress"
```

Use real IDs. Set the configured terminal status before `backlog task complete TASK_ID`. Do not retire with `backlog task archive`: allocation reads `tasks/` and `completed/`, not `archive/`, so archiving can release IDs for reuse. Retired files retain their real status, not proof every proposal shipped. Use the active-task view and run `backlog doctor` after structural moves.

## Releases and distribution

CLI version: [Cargo.toml](Cargo.toml), synchronized with Cargo.lock. Skill/plugin version: [plugin.json](plugin.json), [.claude-plugin/plugin.json](.claude-plugin/plugin.json), and skill metadata; independent of CLI version. [Distribution checks](scripts/dev/check-discovery.py) guard consistency.

The [release workflow](.github/workflows/release.yml) is generated from [dist-workspace.toml](dist-workspace.toml) and has a broad version-like tag trigger. Inspect it before tagging; regenerate generated steps with the configured cargo-dist version. Documentation cleanup and unshipped prototypes need no tag or version bump.

Before publishing, run all applicable groups, MSRV and installer/site tests; inspect `cargo package --list`. Follow [publication operations](docs/publication.md). No commit implicitly authorizes a tag, catalog submission, credentialed API request, or Pages deployment.
