# Working on Blotter

Blotter is a Rust CLI for selective, append-only agent friction and findings. Keep the product local, inspectable, deterministic, and noninteractive. `CLAUDE.md` is a symlink to this file; edit `AGENTS.md`.

## Read only what the change needs

| Need | Source |
| --- | --- |
| Current implementation rules | [docs/contract.md](docs/contract.md) |
| Installed CLI flags, output, errors | `blotter schema`; implementation in [src/commands/schema.rs](src/commands/schema.rs) |
| User-visible behavior | [docs/reference.md](docs/reference.md) |
| Build, test ownership, backlog, releases | [CONTRIBUTING.md](CONTRIBUTING.md) |
| Distribution and site operations | [docs/publication.md](docs/publication.md), [docs/discovery-site.md](docs/discovery-site.md) |
| Rationale for an old decision | [docs/history.md](docs/history.md), only when needed |

Do not reconstruct today's contract by reading historical amendments backwards. Update the current contract, reference, executable schema, and relevant tests together when behavior changes. If they disagree, reproduce the behavior and fix the discrepancy; do not silently use obsolete prose to justify a change.

## Gates

```bash
cargo build --release
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --check
python3 -m unittest discover -s tests/docs -v
python3 scripts/dev/check-docs.py --binary target/release/blotter
python3 scripts/dev/check-discovery.py --binary target/release/blotter
```

The Python documentation check uses the build-only dependencies in `site/requirements.txt`; setup and additional targeted checks are in CONTRIBUTING. Run the gates before landing. When the environment cannot run one, report it as not run and use the PR's actual CI result, not a guessed pass.

For storage, archive, locking, or other concurrency changes, also run `scripts/dev/gate-5x.sh`; retain failure output. Use `scripts/dev/test-fast.sh` for iteration, not as a replacement for the standard gates. Read the Rust floor from `Cargo.toml`; it is a compatibility floor, not a latest-stable target.

## Boundaries worth keeping in working memory

- Ordinary mutations perform read → version probe/fold → validation → append under one exclusive lock. Shared-lock readers never promote or resolve anything.
- Only `archive` and `doctor --fix` replace a log, through backup-preserving copy-and-swap. Do not edit real ledger history by hand.
- Stored records use v2; `v` never appears in a normal output record. An unsupported known-kind log is rejected before writing any bytes.
- Stdout is data, stderr is structured errors. Raw Markdown and OTLP export are explicit exceptions. Empty is success; finding exit 1 is not an error count; lock timeout is retryable exit 75.
- `resolve` batches validate completely before appending. Amendments replace ordinary resolution fields; disposition inheritance and recurrence timestamps are deliberate exceptions.
- Promotion is an explicit trust boundary. Recurrence is evidence, never authorization to create an artifact, promote, or publish it.
- Redaction is best-effort. Text and evidence lanes differ; do not add secrets, private logs, environment dumps, or invented observations to fixtures or documentation.

## Work organization

Use the existing Backlog CLI for tracked tasks, with `--plain` on reads. Never hand-edit task files. Retire terminal tasks with `backlog task complete`, not `archive`: retired IDs must remain in `backlog/completed/`. Do not create a second backlog in a roadmap or handoff document.

Put black-box tests in the owning `tests/cli/` module and declare new modules in `main.rs`. Share a helper only when a second module needs it. Set test environment through `Command::env`, never process-global `std::env::set_var`.

Log genuinely qualified friction while working here, using the [skill's admission rules](skills/blotter/SKILL.md). Do not turn implementation typos or expected compiler feedback into dogfood noise. Use an explicit temporary ledger for smoke tests. Keep implementation, tests, docs, and task state in the same reviewable change; review the finished diff before landing.
