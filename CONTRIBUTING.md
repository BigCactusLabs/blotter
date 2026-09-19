# Contributing

Start with [the documentation map](docs/README.md). The [current contract](docs/contract.md) records implementation invariants; the executable's `schema` describes the version being run. Historical design discussions are optional context, not an onboarding prerequisite.

## Set up

Use a Rust toolchain at or above `package.rust-version` in [Cargo.toml](Cargo.toml), with Clippy and rustfmt. Python 3.11+ is needed for documentation tooling, not for the installed CLI.

```bash
python3 -m venv .venv
. .venv/bin/activate
python -m pip install --only-binary=:all: -r site/requirements.txt
cargo build --release
```

Keep the local environment untracked. Do not run host installers, credentialed publication scripts, or writes against a real ledger merely to test documentation.

## Validate a change

```bash
cargo build --release
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --check
python3 -m unittest discover -s tests/docs -v
python3 scripts/dev/check-docs.py --binary target/release/blotter
python3 scripts/dev/check-discovery.py --binary target/release/blotter
python3 -m unittest discover -s tests/discovery -p 'test_*.py' -v
python3 -m unittest discover -s tests/site -v
```

`check-docs.py` validates active Markdown links, heading anchors, navigation, entry-point size, and the `CLAUDE.md` symlink. With `--binary`, it compares reference command coverage to `schema` and runs the marked README quickstart in a disposable repository with an explicit temporary ledger. It never shells out to arbitrary Markdown commands. Without a binary, those runtime checks are reported as skipped.

The consumer-site builder validates rendered links separately. Its publication inputs must be committed, and its destination must not exist:

```bash
python3 scripts/dev/build-discovery-site.py --output /tmp/blotter-docs-preview
```

For a fast Rust iteration loop, `scripts/dev/test-fast.sh` uses nextest when available and falls back to cargo test. It does not replace the standard cargo test gate. For storage, locking, archive, or concurrency changes, run `scripts/dev/gate-5x.sh` and preserve all five outputs. After dependency changes and before a release, install the declared Rust floor and run `scripts/dev/check-msrv.sh` against the locked dependency tree.

An environment limitation is not a passing check. State what ran locally, what ran in CI, and what remains unverified.

## Find the implementation

| Area | Files |
| --- | --- |
| CLI parsing and flags | [src/cli.rs](src/cli.rs) |
| Envelope and contract number | [src/output.rs](src/output.rs) |
| Error/exit dictionary | [src/error.rs](src/error.rs) |
| Executable schema | [src/commands/schema.rs](src/commands/schema.rs) |
| File discovery, locks, fold, append and repair mechanics | [src/store.rs](src/store.rs) |
| Command behavior | [src/commands](src/commands) |
| Black-box regression tests | [tests/cli](tests/cli) |
| Documentation/installation/site tooling | [scripts/dev](scripts/dev) |

Mutations belong inside the established lock/read/fold/validate/append transaction. `archive` and `doctor --fix` have the same change-with-care bar as `store.rs`. Do not trade failure atomicity for a shorter implementation.

## Test ownership and troubleshooting

Add a black-box test to the module owning the behavior. Put cross-cutting cases in `contract`, `store`, `redaction`, or the other existing subject module rather than duplicating them per command. `tests/cli/main.rs` must declare every sibling Rust module; otherwise its tests never run. `common.rs` is for helpers used by multiple modules. Use subprocess-local environment variables, not process-global mutation.

Cargo accepts one positional test filter; pass additional filters to the test harness after `--`. When a source edit appears to have no effect, inspect the binary timestamp before doubting the edit. The suite has a stale-binary sentinel; recover with `cargo clean -p blotter-cli`, rebuild, and rerun the failing test.

Do not run examples against `.blotter.jsonl` as test fixtures. The discovery smoke and lifecycle demonstration already create disposable ledgers.

## Keep documentation single-purpose

The README introduces the product and installation. The skill owns the self-contained agent procedure. The reference explains current behavior; `schema` owns the executable's complete flag and output inventory. The contract records implementation invariants. Publication and site runbooks own external operations. Link between these instead of copying whole sections.

When changing behavior, update the implementation, schema, contract/reference, regression tests, and Unreleased changelog together. Keep runnable examples concrete: shell pipes are not a notation for enum alternatives, and a made-up ID is not a usable quickstart result. Do not add a hand-maintained schema snapshot or a second set of site prose. Site pages come from the existing source allowlist.

New current docs must be reachable from the documentation map. Put durable decisions in the relevant current page; use Git history for superseded specifications, release archaeology, and completed handoffs. Do not reintroduce “newest amendment wins” governance or mandatory local-only documents.

## Backlog

Use the Backlog CLI, not direct Markdown edits:

```bash
backlog task list --plain
backlog task create "A concrete unit of work"
backlog task edit TASK_ID -s "In Progress"
```

Replace `TASK_ID` with a real task ID. Set the configured terminal status before `backlog task complete TASK_ID`. Do not use `backlog task archive` to retire work: ID allocation reads `backlog/tasks/` and `backlog/completed/`, not `backlog/archive/`, so archiving can release an ID for reuse. Retained completed files preserve their real status; they are not proof that every retired proposal shipped. Use `backlog doctor` after structural task moves.

## Releases and distribution

CLI version: [Cargo.toml](Cargo.toml), synchronized with Cargo.lock. Skill/plugin version: [plugin.json](plugin.json), [.claude-plugin/plugin.json](.claude-plugin/plugin.json), and skill metadata; it is independent of the CLI version. [Distribution checks](scripts/dev/check-discovery.py) guard their consistency.

The [release workflow](.github/workflows/release.yml) is generated from [dist-workspace.toml](dist-workspace.toml). Its version-like tag trigger is broad. Do not invent a separate plugin tag or edit generated release steps without inspecting the trigger and regenerating with the configured cargo-dist version. A documentation cleanup needs no release tag or runtime version bump.

Before publishing, run all gates, the declared MSRV check, and applicable installer/site tests; inspect the package file list with `cargo package --list`. Then follow [publication operations](docs/publication.md). No tag, catalog submission, credentialed API request, or Pages deployment is implied by a documentation commit.
