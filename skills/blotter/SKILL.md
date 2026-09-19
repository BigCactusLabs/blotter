---
name: blotter
description: Capture and review coding-agent friction, tool failures, misleading errors, workflow footguns, surprising findings, and durable learning in a local append-only ledger. Use when an agent encounters recurring, consequential, transferable, misleading, or systemic repository problems; when reviewing agent friction, checking whether fixes held, or selecting evidence for docs, tests, guards, skills, tools, or process improvements. Not for ordinary task tracking, transcripts, or every failed attempt.
license: MIT
compatibility: Requires the blotter CLI on PATH and permission to write to the target repository. Installing this skill does not install the binary. Normal CLI use is local and needs no account or server.
metadata:
  author: BigCactusLabs
  version: "1.1.0"
---

# Blotter

Keep a selective record of coding-agent experience in the repository where it happened. Blotter is not a transcript, telemetry stream, bug tracker, or general personal memory.

## Orient first

Use the installed binary's contract instead of guessing flags or record shapes:

```bash
blotter schema
```

Do not run `blotter list` as a prerequisite to filing. Independent recurrences are evidence: record a qualifying experience even when a similar cut already exists. Do not manufacture repeated records of the same observation to inflate a cluster. Review the ledger when the task calls for review, not as an admission gate.

If the binary is missing, install only when package installation is authorized. Choose one existing package manager:

```bash
# Homebrew on macOS or Linux; no Rust compiler required.
brew install BigCactusLabs/tap/blotter

# Alternative: build from crates.io when Rust is available.
cargo install blotter-cli
```

Prebuilt archives and platform installers are available at https://github.com/BigCactusLabs/blotter/releases/latest. Review the source and follow the environment's installation policy; never bypass approval, sandboxing, or network restrictions. If installation is unavailable, report that fact and continue the underlying task without inventing records. The package is `blotter-cli`; the executable is `blotter`.

## File a cut when friction clears the floor

A cut needs at least one of these grounds:

- **Transferable** — another competent agent or user could hit the same problem.
- **Consequential** — it cost real time, forced retries, produced incorrect work, or blocked progress.
- **Recurring** — the same underlying friction happened before; a new occurrence is useful evidence.
- **Misleading** — an error, document, or interface hid the cause or discouraged the right fix.
- **Systemic** — a documentation gap, missing affordance, brittle interface, or reusable footgun.

Skip ordinary execution noise: a typo, quoting slip, bad first guess, stale patch context, a compiler correctly rejecting new code, or a malformed fixture just authored during the task. Recurrence or system behavior can make an otherwise small problem worth keeping.

File one or two sentences and keep working. Replace example text with what actually happened:

```bash
blotter add "The documented root-level test command finds no tests; the runner actually starts in apps/web" --tag tests --impact material
```

Impact is consequence, not admission: `low` is qualified friction with limited cost; `material` cost real time or produced wrong work; `blocking` stopped progress. Non-qualifying noise is not a low-impact cut.

For a failed command, use `--cmd`, `--exit`, and `--stderr-file` with that command's output only. Put a hypothesis in `--evidence`, separate from the observation. Redaction is best-effort: never attach credentials or a full environment dump.

## File a dogear for an observed finding

All three must hold: one finding in your own words; interesting beyond the immediate task; understandable to a reader who has never seen the repository. A dogear is a lead, not a verified result. Do not invent measurements or turn chores and "we should someday" thoughts into findings.

```bash
blotter dogear "In this parser, bounding input before deserialization removed three separate downstream size checks without changing the accepted fixtures" --tag design
```

Only record an example like this after observing it yourself.

## Review, resolve, and learn

Use the narrowest read command that answers the question:

```bash
blotter list --format md
blotter list --kind dogear
blotter triage
blotter verify
blotter retrospect
blotter digest --since 7d --format md
```

`triage` groups chronic open cuts; `verify` checks whether resolved friction returned; `retrospect` packages evidence for durable improvements. These analysis commands are read-only. Recurrence is never permission to auto-promote.

Close what your change actually fixes:

```bash
blotter resolve CUT_ID --disposition fixed
```

Replace `CUT_ID` with a real identifier. Other cut dispositions are `promoted`, `accepted` (deliberately tolerated), and `invalid` (never friction). A promotion records a human-approved durable artifact that already exists; do not generate one just to satisfy a threshold:

```bash
blotter promote --source CUT_ID --artifact-type doc --artifact-ref docs/testing.md
```

The artifact types are `doc`, `skill`, `guard`, `test`, `tool`, and `process`. A promotion record does not by itself resolve its source cut. Resolve dogears with their publication URL or drop them when review rejects the lead; consult `blotter schema` for the exact flags.

## Scope and privacy

The default ledger is `.blotter.jsonl` at the repository root. Use the target repository's working directory, or an explicitly authorized file destination. Do not write global, unrelated-repository, or private model-internal friction into a project's ledger. Do not publish a ledger or export it to a collector without authorization.

Run `blotter doctor --leaks` before publishing a log, and review it yourself: the check is not a guarantee that every secret has been removed. For automated evaluation, use a disposable directory and an explicit `BLOTTER_FILE`, never a real project's log.

Use the JSON envelope and documented exit semantics, not a blanket "nonzero means failure" rule. Empty results succeed; some analysis commands use exit 1 for findings. The installed `blotter schema` is authoritative for the installed version.
