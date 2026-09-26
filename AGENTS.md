# Working on Blotter

Blotter is a Rust CLI for selective, append-only agent friction and findings. Keep the core local, inspectable, deterministic, and noninteractive. `CLAUDE.md` is a symlink to this file; edit `AGENTS.md`.

## Read only what the change needs

| Need | Source |
| --- | --- |
| Current guarantees and supported behavior | [docs/contract.md](docs/contract.md) |
| Installed CLI flags, output, errors | `blotter schema`; implementation in [src/commands/schema.rs](src/commands/schema.rs) |
| User-visible behavior | [docs/reference.md](docs/reference.md) |
| Validation, test ownership, backlog, releases | [CONTRIBUTING.md](CONTRIBUTING.md) |
| Distribution and site operations | [docs/publication.md](docs/publication.md), [docs/discovery-site.md](docs/discovery-site.md) |
| Rationale for an old decision | [docs/history.md](docs/history.md), only when needed |

Do not reconstruct today's contract from historical amendments. If implementation, schema, and prose disagree, reproduce and reconcile the discrepancy. Old task criteria are context, not current instructions.

## Make the requested change

Within the requested task, try reversible ideas without creating an ADR or another approval stage. Put the hypothesis, scope, and keep-or-delete evidence in the existing task or PR. Optional presentations and analysis experiments are allowed; do not silently change supported CLI behavior or use an experiment to bypass trust boundaries.

Update only affected implementation, tests, and documentation. A private refactor does not require a contract edit; a prototype does not become a supported interface merely by existing. Use the [validation guidance](CONTRIBUTING.md#validate-a-change) for the surfaces touched. Report checks as run, failed, or not run; never infer a CI pass. Review the finished diff before landing.

## Boundaries worth keeping in working memory

- Mutations lock, read/fold, validate, and append as one transaction. Readers never secretly write.
- Only explicit maintenance replaces ledger bytes, with backup-preserving copy-and-swap. Never edit real ledger history by hand.
- Preserve supported storage, identity, output, and resolution semantics until deliberately changed. The contract explains the details.
- Recurrence is evidence, not authorization to create, promote, or publish an artifact. Redaction is best-effort; keep secrets and invented observations out of records and fixtures.

## Work organization

Use the Backlog CLI for tracked tasks, with `--plain` on reads. Never hand-edit task files. Retire terminal tasks with `backlog task complete`, not `archive`: retired IDs must remain in `backlog/completed/`. Do not create a second backlog in a roadmap or handoff document.

Log genuinely qualified friction using the [skill's admission rules](skills/blotter/SKILL.md), not implementation typos or expected compiler feedback. Use explicit temporary ledgers for smoke tests. Keep relevant task state with the implementation change.
