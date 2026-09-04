---
name: blotter
description: Capture and review coding-agent friction, tool failures, workflow footguns, surprising findings, and durable learnings in a local append-only ledger. Use when an AI coding agent hits recurring, consequential, misleading, systemic, or transferable developer-experience problems; when running agent retrospectives; when checking whether a fix actually held; or when turning repeated friction into docs, tests, guards, skills, tools, or process changes.
license: MIT
compatibility: Requires the blotter CLI on PATH. Intended for coding agents working in or around git repositories.
metadata:
  author: BigCactusLabs
  version: "1.0"
---

# Blotter

Use Blotter as a selective memory for coding-agent experience. It is not a transcript, telemetry stream, bug tracker, or place to dump every failed attempt.

## Orient first

If `blotter` is not on `PATH`, install it when package installation is allowed:

```bash
cargo install blotter-cli
```

If installation is not allowed, say that Blotter is unavailable and continue the underlying task without inventing records.

When Blotter is available, check what is already known before filing new material:

```bash
blotter list
```

Use `blotter schema` when you need the exact machine contract, flags, record shapes, or exit-code semantics instead of guessing.

## Decide what deserves a record

### File a cut when any one of these is true

- **Transferable** — another competent agent or user would plausibly hit the same problem.
- **Consequential** — it cost real time, forced retries, produced incorrect work, or blocked progress.
- **Recurring** — the same underlying friction has happened before.
- **Misleading** — an error, document, or interface pointed at the wrong cause or discouraged the right fix.
- **Systemic** — it exposes a missing affordance, documentation gap, brittle interface, flaky workflow, or reusable footgun.

Skip ordinary execution noise: typos, quoting slips, a bad first guess, stale patch context, a compiler or linter correctly rejecting new code, a malformed fixture you just wrote, or a one-off tactical mistake. Those become cuts only if recurrence or system behavior makes them useful beyond the current run.

File the cut and keep working:

```bash
blotter add "<what you were doing -> what got in the way and what would have prevented it>" --tag <area> --impact low|material|blocking
```

For tool failures, attach bounded evidence when useful instead of pasting an environment dump:

```bash
blotter add "<failure and useful context>" --tag tooling --cmd "<command>" --exit <code> --stderr-file <path>
```

Impact describes consequence after the cut qualifies. `low` is still meaningful friction; non-qualifying noise is not a low-impact cut.

### File a dogear only when all three are true

- It is one finding in your own words.
- It is interesting or possibly novel beyond the task in front of you.
- A reader who has never seen this repository can understand it.

Dogears are for observations and leads, not chores or "we should someday" items.

```bash
blotter dogear "<one self-contained finding>" --tag <area>
```

### Record a promotion only after the durable artifact exists

A promotion means selected experience became something durable: a doc, skill, guard, test, tool, or process change. Recurrence is evidence for a human or agent to inspect; it is never permission to auto-promote.

```bash
blotter promote --source <cut-id> --artifact-type doc|skill|guard|test|tool|process --artifact-ref <path-or-ref>
```

## Review and learn

Use the narrowest read command that answers the question:

```bash
blotter list --format md
blotter list --kind dogear
blotter triage
blotter verify
blotter retrospect
blotter digest --since 7d --format md
```

- `triage` finds chronic clusters of open cuts.
- `verify` checks whether friction returned after a cut was resolved.
- `retrospect` packages recurring evidence into promotion candidates; it does not create promotions.
- `digest` gives a periodic view of chronic friction, new cuts, and open findings.

## Resolve deliberately

Close a cut with the outcome that actually happened:

```bash
blotter resolve <cut-id> --disposition fixed|promoted|accepted|invalid
```

Resolve dogears with the publication URL when they become public, or drop them when review kills the lead. Use `blotter schema` for the exact flags.

## Privacy and scope

- Keep records useful without pasting secrets, credentials, full environment dumps, or unnecessary absolute paths.
- Run `blotter doctor --leaks` before publishing a log from a private or sensitive repository.
- Do not log global, system, or private model-internal friction into a repository ledger.
- Prefer one record that names the underlying recurrence over several near-duplicate records.

The goal is a small, high-signal ledger that helps future agents avoid the same friction and helps maintainers see what is worth fixing.