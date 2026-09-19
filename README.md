# blotter

**Your agents have complaints. Give them somewhere to write them down.**

A tiny Rust CLI that gives AI agents a blotter — the pad on the desk where you note the thing before it's gone. Nothing on it is a commit, a ticket, or a chat message. Agents jot three kinds of records into one append-only journal:

- **Cuts:** friction worth keeping. A misleading error, a broken setup step, a recurring tool failure.
- **Dogears:** findings worth returning to. One observed engineering quirk, measurement, or lead, in the agent's own words.
- **Promotions:** durable learning. These experiences became this doc, skill, guard, test, tool, or process change.

Agents silently push through friction and drop interesting findings mid-task. Every one was a sentence away from being useful. Blotter gives them a one-line home, then helps you review what recurs and what happened after a fix.

JSON envelopes on stdout. Structured errors on stderr. Stable exit codes. `blotter schema` tells an agent how the installed executable works. There is no dashboard. There is not going to be a dashboard.

The friction-log idea comes from [a tool Steve Ruiz built](https://x.com/steveruizok) for his own repos: give agents somewhere to complain and the workflow defects stop disappearing.

## Install

Homebrew, on macOS or Linux:

```bash
brew install BigCactusLabs/tap/blotter
```

Or use a prebuilt installer:

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/BigCactusLabs/blotter/releases/latest/download/blotter-cli-installer.sh | sh
```

```powershell
irm https://github.com/BigCactusLabs/blotter/releases/latest/download/blotter-cli-installer.ps1 | iex
```

Inspect remote installer scripts before executing them. [Release archives](https://github.com/BigCactusLabs/blotter/releases/latest) are available for manual installation. To compile from crates.io:

```bash
cargo install blotter-cli --locked
```

The crate is `blotter-cli`; the executable is plain `blotter`. Upgrading from 0.15 or earlier requires removing the retired hook and starting a new ledger; read [the upgrade steps](docs/reference.md#upgrading-from-015) first.

## Install the agent skill

Install the CLI separately, then install the [canonical skill](skills/blotter/SKILL.md):

```bash
npx skills add BigCactusLabs/blotter --list
npx skills add BigCactusLabs/blotter --skill blotter
```

For Claude Code's native plugin manager:

```text
/plugin marketplace add BigCactusLabs/blotter
/plugin install blotter@blotter-tools
```

Choose one skill route per agent. Neither installs the binary, a hook, or a server. The third-party Skills CLI has its own installation telemetry; `DISABLE_TELEMETRY=1` opts out. Blotter itself has no telemetry. See [agent installation](docs/discovery.md) for scope, prerequisites, and verification.

## Two minutes

Inside a repository, with Blotter installed. No init step; the first record creates `.blotter.jsonl` at the repository root. The two records below describe friction and a finding actually observed while building Blotter; use a disposable repository when trying them unchanged.

<!-- blotter:quickstart -->
```bash
blotter add "Edited src/ and rebuilt, but cargo reported every unit Fresh and ran the stale binary; cargo clean -p blotter-cli was the recovery" --tag build --impact material
blotter dogear "Backlog.md issues task IDs as max+1 over the tasks and completed directories, but never reads archive, so archiving a task lets a later create reuse its number" --tag tooling
blotter list --format md
blotter list --kind dogear
```

To close a cut after fixing it, use its **actual returned ID** with `blotter resolve ID --disposition fixed`. The [workflow guide](docs/agent-workflows.md#check-whether-a-workflow-fix-held) shows resolution and recurrence checks. No example ID is guaranteed to exist in your ledger.

For committed logs, add `.blotter.jsonl merge=union` to `.gitattributes` once. Duplicate lines from concatenated branches are harmless. For private logs, ignore the file or select another path with `BLOTTER_FILE`.

The file is the product, and `cat` is a supported client. No account, server, or sync. Ordinary writes append; `archive` and `doctor --fix` are explicit copy-and-swap maintenance operations that preserve backups. Redaction is best-effort, not a confidentiality guarantee. [Storage and privacy details](docs/reference.md#evidence-and-redaction).

## The commands

| Job | Commands |
| --- | --- |
| Capture and close | `add`, `dogear`, `promote`, `resolve` |
| Read and analyze | `list`, `triage`, `verify`, `retrospect`, `digest`, `sweep`, `export` |
| Maintain the file | `doctor`, `archive` |
| Inspect the contract | `schema` |

```bash
blotter digest --since 7d --format md
blotter triage
blotter verify
blotter retrospect
```

Those review commands are read-only. `triage`, `verify`, and `retrospect` use exit **1** to signal findings, not execution failure or the number of findings. Use the [reference](docs/reference.md) for semantics and `blotter schema` for every flag and output shape.

## Cuts

Write what you were doing and what got in the way. File a cut when **any one** applies: the knowledge is transferable; the consequence was meaningful; the problem recurred; the error was misleading; or it exposes a systemic gap or footgun.

Skip ordinary one-off typos, quoting slips, bad first guesses, stale patches, and a compiler correctly rejecting code you just wrote. They become useful only when recurrence or system behavior gives them meaning beyond the execution slip. Blotter is a selective ledger, not a transcript, and nobody reads transcripts.

Impact describes consequence **after admission**: `low` is qualified friction with limited immediate cost, `material` cost real time or caused incorrect work, and `blocking` stopped progress. A low-impact cut is still a cut. Independent recurrences are useful evidence; do not search for a duplicate before filing an actual occurrence.

## Dogears

A dogear is the page-corner you fold down because you will want it later, not because it annoyed you. All three must hold: **one observed finding in your own words; interesting beyond this task; understandable without this repository**. Two to six sentences is usually enough.

Chores, task notes, untested guesses, and “we should someday” items belong in a backlog or nowhere. A dogear is a lead, not a verified result. A human checks it before publication; `resolve --url` records where it was published, and `resolve --dropped` records that it did not survive review. The default list remains cut-only.

## Promotions

A promotion records which cuts became an approved artifact that actually exists. `retrospect` supplies candidates, not permission. `promote` is the only writer of promotion records, and it does not resolve source cuts automatically. [Promotion and resolution rules](docs/reference.md#promote).

## Give your agents the pen

Add this short instruction to your project's existing `AGENTS.md` or `CLAUDE.md`. The skill carries the detailed procedure; do not paste the entire reference into every prompt.

```markdown
## Blotter

Log repository friction in the moment, then keep working. A cut needs
transferable, consequential, recurring, misleading, or systemic value.
Skip ordinary execution slips; record independent recurrences without a
pre-filing lookup. Use `blotter add "observation" --tag AREA` and choose
low, material, or blocking impact according to consequence.

Use `blotter dogear` only for one observed finding in your own words that
is interesting beyond this task and understandable without the repo.
Keep chores in the backlog. Resolve actual cut IDs after fixing them.

Read `blotter schema` for the installed contract. Do not attach secrets
or environment dumps. Review commands are read-only; recurrence never
authorizes automatic promotion or publication.
```

Then come back with `blotter digest --since 7d --format md`. Fix what the agents keep tripping over. The first week is humbling.

## Contract

The supported interface is the CLI, JSON envelopes, stored records, exit codes, and `blotter schema` — not the internal Rust library. Breaking interface changes require an explicit contract change and appropriate major release; additive features do not automatically change `meta.contract`. [Compatibility details](docs/reference.md#what-is-stable).

For readers: [documentation map](docs/README.md), [when to choose Blotter](docs/choose-blotter.md), [agent workflows](docs/agent-workflows.md), and the compact [llms.txt](llms.txt) index. For contributors: [CONTRIBUTING.md](CONTRIBUTING.md) and the [current implementation contract](docs/contract.md).

MIT licensed. See [LICENSE](LICENSE).
