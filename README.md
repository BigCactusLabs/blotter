# blotter

**Your agents have complaints. Give them somewhere to write them down.**

A tiny Rust CLI that gives AI agents a blotter — the pad on the desk where you note the thing before it's gone. Nothing on it is a commit, a ticket, or a chat message. Agents jot three kinds of records into one append-only journal:

- **Cuts** — friction worth keeping. A dead-end tool call, a broken link, a misleading error, a footgun config. Filed at the moment it happens, with optional evidence (the failed command, its exit code, its stderr).
- **Dogears** — findings. Something an agent noticed that is interesting beyond the task: a surprising measurement, an engineering quirk, a gap in prior art. A dogear is the page-corner you fold to come back to; here it marks a lead worth writing up in public.
- **Promotions** — durable learning. "These experiences became this artifact": a doc, a skill, a guard, a test, a tool, or a process change.

Agents hit friction constantly and silently push through; the signal evaporates. They also notice interesting things mid-task and drop them for the same reason. Every one of those was a sentence away from being useful. `blotter` gives all three a one-line home, and gives you (or another agent) the commands to review, cluster, and act on the backlog.

```
$ blotter add "yarn web:test with a root-relative path finds no files; the workspace test cwd is apps/web" --tag tooling
{"ok":true,"data":{"changed":true,"record":{"kind":"cut","id":"bl_9f2c41d0a8b39f2c41d0","ts":"2026-07-09T21:14:03.412Z","agent":"claude-code","text":"yarn web:test with a root-relative path finds no files; the workspace test cwd is apps/web","tags":["tooling"],"impact":"low","cwd":"apps/web","origin":{"type":"agent"}}},"meta":{"contract":6,"file":"/repo/.blotter.jsonl","agent_source":"detected"}}

$ blotter dogear "On five real friction logs, a rare-token linkage rule with ceiling N/4 produced two-thirds unrelated pairs; tightening to N/16 removed 79% of the false links for 17% of the true ones" --tag research
```

It is an agent-only tool by design: JSON envelopes on stdout, structured errors on stderr, stable exit codes, and a `blotter schema` command that returns the whole machine contract so agents self-orient without reading docs. You read the log; the agents write it. There is no dashboard. There is not going to be a dashboard.

The friction-log idea comes from [a tool Steve Ruiz built](https://x.com/steveruizok) for his own repos: once agents had a place to complain, they immediately surfaced real workflow defects — quoting bugs, wrong test working directories, YAML footguns — that they'd been eating silently for months.

## Install

```bash
cargo install blotter-cli
```

The crate is named `blotter-cli` because someone claimed `blotter` on crates.io, published a placeholder, and went home. The installed binary is plain `blotter`. To build from the latest source instead: `cargo install --git https://github.com/BigCactusLabs/blotter blotter-cli`.

Coming from 0.15? 1.0.0 needs two manual steps before the new binary runs in an old repo: remove the Claude Code hook and start a fresh ledger. Both are in [Upgrading from 0.15](docs/reference.md#upgrading-from-015).

## Two minutes

Inside any git repository. No init step; the first record creates the file.

```bash
# 1. Let branches merge the log by concatenation instead of conflicting.
echo '.blotter.jsonl merge=union' >> .gitattributes

# 2. File a cut and a finding. The log appears at the repo root.
blotter add "cargo test on a fresh clone fails until the fixtures submodule is pulled" --tag onboarding --impact material
blotter dogear "A single 1 MiB read bound on every input lane made three separate DoS guards unnecessary" --tag design

# 3. Read them back.
blotter list --format md          # open cuts, worst first
blotter list --kind dogear        # open findings, newest first

# 4. Close one out.
blotter resolve bl_9f2c --disposition fixed --pr https://github.com/you/repo/pull/12
```

Then paste the block under [Give your agents the pen](#give-your-agents-the-pen) into your agent instructions, and come back in a week with `blotter digest --since 7d --format md`.

Records live in an **append-only JSONL file** — by default `.blotter.jsonl` at your repo root, so every entry shows up in `git diff` and travels with the repo. No server, no sync, no telemetry, no account. The file is the product, and `cat` is a supported client. Multiple agents on one file are fine; nothing ever rewrites history; evidence is bounded and home paths and obvious secrets are redacted at write time. The mechanics are in the [reference](docs/reference.md#the-log-file).

## The commands

Fourteen subcommands, four jobs, none of them interactive. Every one is described in full in the [reference](docs/reference.md).

**Write** — append records to the log:

```bash
blotter add "text" --tag <area>   # file a cut (also: blotter log, or pipe stdin to add -)
blotter dogear "one finding, in your own words" --tag <area>   # file a finding (also: finding, idea)
blotter promote --source bl_9f2c --artifact-type skill --artifact-ref skills/testing.md  # record durable learning
blotter resolve bl_9f2c --disposition fixed   # resolve a cut; --url / --dropped for a dogear
```

**Read and analyze** — read-only views over the log:

```bash
blotter list                      # open cuts, impact-first then newest (--format md for humans)
blotter list --kind dogear        # open findings; --kind promotion, --kind all
blotter triage                    # chronic clusters of similar open cuts
blotter verify                    # resolved cuts whose friction came back
blotter retrospect                # what has hurt often enough to build something for
blotter digest --since 7d         # periodic report: chronic, new, open findings
blotter sweep ~/code/a ~/code/b   # roll-up across several repositories
blotter export --format otlp-json # one OTLP LogsData line for a collector
```

**Maintain** — the log file itself:

```bash
blotter doctor                    # validate the log (--leaks before a public push, --fix for torn lines)
blotter archive --before 180d     # move fully closed, fully old history to a sidecar
```

**Contract**:

```bash
blotter schema                    # the whole machine contract — agents self-orient with this
```

## Cuts

A cut is one or two sentences of friction: what you were doing, what got in the way. Not every stumble is a cut. Blotter is a selective ledger, not a transcript, and nobody reads transcripts. A cut is a claim that the friction has future value. File one when at least one of these holds:

- **Transferable** — another competent agent or user would plausibly hit it.
- **Consequential** — it cost real time, produced incorrect work, forced retries, or stopped the task.
- **Recurring** — small, but it has happened before. One cut naming the recurrence beats three saying the same thing.
- **Misleading** — the error pointed at the wrong cause or discouraged the correct fix.
- **Systemic** — a missing affordance, a documentation gap, a brittle interface, a reusable footgun.

Skip typos, shell quoting mistakes, a bad first guess, using the wrong command or API once, a patch that missed because context was stale, a linter or compiler correctly rejecting code you just wrote, a malformed fixture authored during the task, one broad query that returned too much, and any transient mistake specific to the current run. These are execution events, not knowledge, unless recurrence or system behaviour makes them one.

Impact describes consequence after that decision, not whether to file: `low` (default) is a qualified cut with limited immediate cost, `material` cost real time or caused incorrect work, `blocking` stopped the task. A low-impact cut is still a cut.

Every resolution names the cut's fate — `fixed`, `promoted`, `accepted` (friction deliberately tolerated), or `invalid` (never friction) — and a resolution you got wrong is corrected with `--amend`, never rewritten. Details: [resolve](docs/reference.md#resolve).

## Dogears

A dogear is a finding: something an agent noticed that is interesting beyond the task in front of it. The corner of the page you fold down because you'll want it later, not because it annoyed you. Dogears are deliberately separate from friction — the default list stays cut-only so the complaint queue and the findings queue never blur.

File a dogear when all three hold. A cut needs any one of its five grounds; a dogear needs all three.

- **One finding, in your own words.** A single observation or lead, not a list, not a paste.
- **Interesting or possibly novel beyond this task.** A surprising measurement, a quirk with a mechanism behind it, a gap in prior art, a pattern with no name yet. Repo-local is fine; repo-bound is not.
- **Understandable without the repo.** A reader who has never seen this codebase can follow it. Two to six sentences, the scale of a TIL post.

Skip task notes, chores and "we should someday" items (those belong in a backlog, or nowhere), anything derivable from the docs, and anything you have not actually observed. A dogear is a lead, not a verified result: `resolve --url` records where a human published it, and `resolve --dropped` records that it did not survive review. Whoever publishes a dogear checks it first. Details: [dogear](docs/reference.md#dogear).

## Promotions

A promotion is durable learning, recorded as "these experiences became this artifact". It names one or more source cuts, an artifact type (`doc|skill|guard|test|tool|process`), and where the artifact lives. `retrospect` packages the argument for one; a human decides whether to make it, and `promote` is the only command that writes one. Details: [promote](docs/reference.md#promote).

## Give your agents the pen

Paste this into your `CLAUDE.md` / `AGENTS.md` / system prompt:

```markdown
## Blotter

Run `blotter list` first to see what is already known. Do not add global,
system, or internal friction.

Blotter is a selective ledger, not a transcript. File a cut when friction
clears the floor: another agent would plausibly hit it (transferable), it
cost real time or produced wrong work (consequential), it has happened
before (recurring), the error pointed at the wrong cause (misleading), or
it reveals a doc gap, a brittle interface, or a footgun (systemic). Skip
typos, quoting slips, a bad first guess, a linter or compiler correctly
rejecting code you just wrote, and one-off mistakes specific to this run.

    blotter add "<what you hit and what would have prevented it>" --tag <area>

When you notice something interesting beyond this task — one finding, in your
own words, that a reader without this repo could follow; all three, where a cut
needs any one of its grounds — dogear it the same way:

    blotter dogear "<the finding>" --tag <area>

Don't stop working; file it and push through. Impact is consequence, not
admission: blocking if you could not proceed, material if you lost real time or
did wrong work, low (default) for a qualified cut with limited cost. Run
`blotter schema` once if you need the full contract. Attach `--cmd`, `--exit`,
or `--stderr-file` when filing tool failures; never feed raw environment dumps.
```

Then periodically: `blotter list --format md` and fix what your agents keep tripping over, and `blotter list --kind dogear` to see what they found worth writing up. The first week is humbling.

## Team setup

**Committed (default).** `.blotter.jsonl` is a normal tracked file — records appear in diffs and PRs. Add `.blotter.jsonl merge=union` to `.gitattributes` so parallel branches merge cleanly; duplicate lines after a merge are harmless.

**Private.** Prefer not to commit them? `echo .blotter.jsonl >> .gitignore`, or point `BLOTTER_FILE` somewhere else entirely. Outside a git repo, records go to `~/.blotter/log.jsonl`.

**Public.** Run `blotter doctor --leaks` before pushing a log to a public repository. It flags home paths and any `--deny` literal you name. It cannot flag the thing you didn't think to name, which is what `--deny` is for.

## Contract

Everything an agent needs is in `blotter schema`: commands and flags with read-only/appends annotations, env vars, record shapes, error codes, and the exit-code dictionary. Empty results are exit 0, never errors, and exit 1 is a finding count, not a failure. The 1.x promise covers the CLI, the JSON envelopes, the stored record format, the exit codes, and `blotter schema`; the Rust library the crate also builds is the binary's implementation, not a supported API. The exit codes and the stability clause are in the [reference](docs/reference.md#exit-codes).

## What it is not

- **Not a bug tracker.** A cut has no assignee, no priority field, no state machine beyond open and resolved. When one grows up, it becomes a ticket somewhere else and a `resolve` here.
- **Not telemetry.** Nothing leaves the repo unless you run `export` and point it at a collector yourself.
- **Not a transcript.** Agents that log everything are as useful as agents that log nothing. The admission floor is the feature.

## Lineage

This project began as a fork of [treygoff24/papercuts](https://github.com/treygoff24/papercuts) and owes its core design — the append-only journal, the agent-first envelope contract, the concurrency model — to that upstream project. The fork added dogears, structured resolve provenance, and a Claude Code hook integration (since retired), and chronic-cut triage with its analysis family, then took the name **blotter** to stand on its own. `cargo install papercuts` still installs the upstream crate, which has none of those additions. Other tools explore the same space with different bets — e.g. wevm's frog takes a remote-canonical approach where blotter stays local and append-only.

## License

MIT
