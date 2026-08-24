# blotter

A tiny Rust CLI that gives AI agents a blotter — a desk pad for the things that don't fit in a commit. Agents jot two kinds of records into one append-only journal:

- **Cuts** — friction. A dead-end tool call, a broken link, a misleading error, a footgun config. Filed at the moment it happens, with optional evidence (the failed command, its exit code, its stderr).
- **Dogears** — ideas. A surprising measurement, a gap in prior art, a pattern worth writing up. The page-corner you fold to come back to later.

Agents hit friction constantly and silently push through; the signal evaporates. They also have ideas mid-task and drop them for the same reason. `blotter` gives both a one-line home, and gives you (or another agent) the commands to review, cluster, and act on the backlog.

```
$ blotter add "yarn web:test with a root-relative path finds no files; the workspace test cwd is apps/web" --tag tooling
{"ok":true,"data":{"changed":true,"record":{"kind":"cut","id":"bl_9f2c41d0a8b3","ts":"2026-07-09T21:14:03.412Z","agent":"claude-code","text":"yarn web:test with a root-relative path finds no files; the workspace test cwd is apps/web","tags":["tooling"],"severity":"minor",...}},"meta":{"contract":5,"file":"/repo/.blotter.jsonl","agent_source":"detected"}}

$ blotter dogear "The retry-backoff pattern in our fetch helper would make a good standalone write-up" --tag research
```

It is an agent-only tool by design: JSON envelopes on stdout, structured errors on stderr, stable exit codes, and a `blotter schema` command that returns the whole machine contract so agents self-orient without reading docs.

The friction-log idea comes from [a tool Steve Ruiz built](https://x.com/steveruizok) for his own repos: once agents had a place to complain, they immediately surfaced real workflow defects — quoting bugs, wrong test working directories, YAML footguns — that they'd been eating silently for months.

This project began as a fork of [treygoff24/papercuts](https://github.com/treygoff24/papercuts) and owes its core design — the append-only journal, the agent-first envelope contract, the concurrency model — to that upstream project. The fork added dogears, structured resolve provenance, chronic-cut triage and its analysis family, and a Claude Code hook integration (since retired), then took the name **blotter** to stand on its own. `cargo install papercuts` still installs the upstream crate, which has none of those additions. Other tools explore the same space with different bets — e.g. wevm's frog takes a remote-canonical approach where blotter stays local and append-only.

## Install

```bash
cargo install blotter-cli
```

The crate is named `blotter-cli` (the crates.io name `blotter` is already taken by a placeholder crate), but the installed binary is plain `blotter`. To build from the latest source instead: `cargo install --git https://github.com/BigCactusLabs/blotter blotter-cli`.

## How it works

Records live in an **append-only JSONL file** — by default `.blotter.jsonl` at your repo root, so every entry shows up in `git diff` and travels with the repo. No server, no sync, no telemetry. The file is the product.

- **Agent-first contract**: stdout is data only; one JSON envelope per command; structured errors on stderr with stable codes, documented exit codes, and a paste-ready `suggested_fix`. `blotter schema` returns the whole contract.
- **Concurrency-safe**: multiple agents on one file are fine (advisory locking, atomic appends, self-healing torn lines).
- **Deterministic**: content-addressed IDs — a cut's identity covers its timestamp, agent, text, severity, and sorted tags, so the same text filed under different tags is a different cut — plus stable sort and a reproducible-clock override for tests.
- **Never rewrites history**: `resolve` appends an event; the log is a journal, not a database. The two exceptions, [`archive`](#archive) and [`doctor --fix`](#doctor), never edit in place — each writes a replacement copy and atomically swaps it in, always preserving the original as a timestamped backup.
- **Evidence is bounded and redacted**: `add` can attach a failed command (`--cmd`), exit status (`--exit`), UTF-8 stderr file (`--stderr-file`), or free-form note (`--evidence`). Redaction covers every authored free-text field, not just `add`: `dogear --evidence` and `resolve --note`/`--amend` go through the same pass at write time. `--stderr-file` rejects non-regular files and inputs over 1 MiB before sanitized stderr is stored up to 4096 UTF-8 bytes; a symlink is followed to its target, which must itself be a regular file. Redaction is best-effort hygiene, not a security boundary; never feed raw environment dumps. Every input lane carries the same 1 MiB read bound — `--stderr-file`, the hook payload, and text piped to `add -` or `dogear -` — and the log file itself must be a regular file, so a `--file` or `BLOTTER_FILE` naming a FIFO, device, or directory is rejected rather than blocking or growing without bound.

Two global flags apply to every subcommand: `--file PATH` overrides log discovery for one invocation (same target as `BLOTTER_FILE`), and `--pretty` indents the JSON envelope for human reading. The one exception is `sweep`, which rejects `--file` because its inputs are its arguments.

## The commands

Fourteen subcommands, four jobs:

**Write** — append records to the log by hand:

```bash
blotter add "text"                # file a cut (also: blotter log, or pipe stdin to add -)
blotter add "tool failed" --cmd 'tool --flag' --exit 1 --stderr-file /tmp/stderr
blotter add "bad response" --evidence 'request_id=abc123'
blotter dogear "idea worth keeping" --tag research   # file a dogear (also: blotter idea)
blotter resolve bl_9f2c           # mark one record fixed (unique ID prefix ok)
blotter resolve bl_9f2c bl_a81e   # resolve several atomically
blotter resolve <id> --pr <url>   # attach structured graduation provenance
blotter resolve <id> --amend --note "..."  # correct a resolution you got wrong
```

**Read and analyze** — read-only views over the folded log:

```bash
blotter list                      # open cuts, severity-first then newest, JSON envelope
blotter list --format md          # human review digest
blotter list --kind dogear        # the idea backlog, newest first
blotter triage                    # identify chronic clusters of similar open cuts
blotter verify                    # check resolved cuts for later recurrences
blotter retrospect                # mine chronic signal for typed promotion candidates
blotter digest --since 7d         # periodic report: chronic, new, open ideas
blotter sweep ~/code/a ~/code/b   # roll-up across several repositories
blotter export --format otlp-json # one OTLP LogsData JSON line for a collector
```

**Maintain** — the log file itself:

```bash
blotter doctor                    # validate the log file
blotter doctor --leaks            # scan raw lines for public home-path leaks
blotter doctor --fix              # quarantine unreadable lines (backup + atomic swap)
blotter archive --before 180d     # move fully closed, fully old history to a sidecar
```

**Integrate** — harness plumbing and the machine contract:

```bash
blotter schema                    # full machine contract — agents self-orient with this
```

One other path appends besides the write commands: `doctor --fix` appends in the course of a repair. `blotter schema` carries the authoritative `read_only`/`appends` annotation for every command.

Six read commands — `list`, `triage`, `verify`, `digest`, `sweep`, and `export` — show hand-filed records by default; pass `--include-auto` to include records tagged `auto`. On `list`, `--tag auto` implies `--include-auto`, so you can ask for auto records by name without the extra flag. Nothing writes an `auto` record any more — see [Hooks](#hooks) — so those flags now reach stored history only.

## Cuts

A cut is one or two sentences of friction: what you were doing, what got in the way. Default severity is `minor`; `major` means it cost real time, `blocker` means it stopped the task. Tags group cuts by area; evidence flags capture the failing command without pasting it into the text.

A resolution you got wrong is corrected, not rewritten: `resolve <id> --amend` appends a second resolve event carrying the corrected fields. The first non-amend resolve stays the base event, the latest amend wins the materialized view (`resolution.amended: true`), and every original byte stays in the log. `--amend` needs at least one resolution field and every named record must already be resolved.

"Latest" means the latest **timestamp**, not the last line in the file — a `merge=union` log concatenates branches in branch order, so the two disagree after a merge. An amend written with a clock behind a stored amend therefore does not take over the materialized view, and `resolve` reports the amend that actually won rather than the one it just wrote; `--dry-run` predicts the same answer.

An amend **replaces** the materialized resolution; it does not merge field by field. If the base resolve carried `--pr` and you amend with only `--note`, the materialized `resolution` keeps the note and drops the pull request. Repeat every field you still want:

```bash
blotter resolve <id> --amend --note "corrected" --pr <url>
```

The base resolve is still in the log, as always. It is the materialized view that `list` and `verify` read — the latest amend alone — that loses the field.

`resolve` always returns a `data.records` array, including when only one ID is resolved. New records omit `repo`; their `cwd` is relative to the discovered repository root when possible, and otherwise goes through the same home-path rewrite as evidence — the exact `$HOME`, a generic `/Users/<user>` or `/home/<user>`, and the dash-encoded slug harness scratchpad paths embed all become `~`, so a stored `cwd` does not trip `doctor --leaks`. A resolution `--note` goes through that rewrite too, as does a dogear's `--evidence`, so no field blotter invites you to fill can trip its own gate.

New records carry `bl_`-prefixed IDs. Legacy `pc_` records remain readable as opaque historical data: existing logs fold and list normally, and `resolve` accepts explicit `pc_` IDs or prefixes. New records never use the prefix.

## Dogears

Dogears are the idea half of the blotter: append-only entries for a surprising measurement, a gap in prior art, or a reusable pattern worth turning into research or writing. They are deliberately separate from friction — the default list stays cut-only so the complaint queue and the idea queue never blur.

```bash
blotter dogear "The retry-backoff pattern in our fetch helper would make a good standalone write-up" --tag research --evidence "seen in three modules"
blotter idea - --tag blog-post    # pipe a dogear from stdin
blotter list --kind dogear         # dogear backlog, newest first
blotter list --kind all --format md
blotter resolve bl_9f2c           # promoted to writing work, or dropped
blotter resolve <id> --url <url>  # dogear published at a URL
blotter resolve <id> --dropped    # dogear intentionally dropped
```

Dogears use the same append-only journal, agent resolution, tags, dry-run, deterministic clock override, and resolve events as cuts. `resolve --task`, `--pr`, and `--commit` work for either kind. `--url` and `--dropped` are dogear-only, conflict with each other, and reject a mixed cut/dogear batch before anything is appended. Dogears have no severity or failure-command fields; `list --severity` is therefore accepted only with the default `--kind cut`.

## Triage

`triage` is a read-only scan of open cuts. Cuts whose normalized titles are identical always link, regardless of tags. Otherwise, cuts must share a tag (or both be untagged), then link from filtered tokens: 80% overlap with the shorter token set, or at least three shared tokens that appear in no more than `max(2, ceil(scanned / 4))` open cuts. Filtering removes tokens of two characters or fewer and a small function-word list. Only clusters that meet the threshold are reported; resolved cuts and dogears are excluded. Each cluster carries `occurrences` — how many open cuts share the normalized title of the cluster's displayed `text`. The JSON output suggests `graduate` for each chronic cluster, and exit 1 means at least one was found. Records tagged `auto` are excluded by default; pass `--include-auto` to include them.

```bash
blotter triage --min-count 3
```

`--min-count` defaults to 3 and must be at least 2; a threshold of 1 is `invalid_argument`.

## Verify

`verify` is a read-only check for cuts that reappear after they were resolved. Each eligible resolved cut is an anchor. A later open cut recurs when it matches under the same exact-title, tag, and filtered-token linkage rules as `triage`. Dogears, dropped resolutions, and blank normalized resolved titles are ignored. One open cut can be reported against more than one resolved anchor. Records tagged `auto` are excluded by default; pass `--include-auto` to include them.

```bash
blotter verify
```

The JSON envelope includes each anchor's resolution timestamp and optional task, pull request, and commit provenance, plus the later recurrence IDs. Exit 1 means one or more recurrences were found; no recurrences is exit 0.

## Retrospect

`retrospect` is a read-only mining pass over one log. It asks a different question than `triage`: not "what keeps hurting" but "what has hurt often enough to be worth building something for". It reuses triage's clustering and verify's recurrence rules unchanged, then types the result by evidence shape. A chronic cluster becomes a `wrapper_alias` candidate when half or more of its members share one failing leading program, or a `doc_repair` candidate when half or more are tagged `docs` or `documentation`; the wrapper type wins when both match. Every recurrence group of two or more members becomes a `skill_candidate`, because a cut that was resolved and came back is a recovery worth capturing. A cluster that matches no rule emits nothing and stays an ordinary cut.

```bash
blotter retrospect
```

Retrospect takes no window and no flags: chronic signal is long-horizon, so a window would hide the evidence it looks for. It also **includes auto-captured records by default**, inverting the rule the other read commands follow — the repeated-command-failure signal behind `wrapper_alias` lives in the `auto` lane, so excluding it would remove the point of the command. That lane is [retired](#hooks) and no longer grows, so this default now reaches stored history only.

Each candidate carries its record IDs, first and last timestamps, and bounded evidence: at most 10 member texts and 5 resolution notes, never a record's evidence command, stderr, or note. `occurrences` counts each distinct normalized title in the candidate once, so members that share a title do not multiply the count. Exit 1 means candidates were found, exit 0 means none.

Retrospect never writes anything — no doc, no skill, no alias, and no record in the log. It packages the argument for a promotion; a human decides whether to make it.

## Digest

`digest` is the periodic read-only report: what keeps recurring, what is new, and what ideas are waiting. It combines three views — chronic clusters (the triage analysis at a threshold of 2), open cuts filed inside the window grouped by tag, and all open dogears. Records tagged `auto` are excluded by default; pass `--include-auto` to include them.

```bash
blotter digest --since 7d              # JSON envelope, default window
blotter digest --since 30d --format md # raw markdown, pasteable into a review
```

`--since` takes a full RFC3339 timestamp or an `Nd`/`Nh` duration. Output is byte-deterministic for a given log and clock. An empty report is exit 0, not an error.

## Sweep

`sweep` rolls several repositories' logs into one read-only view — the answer to "what is annoying my agents everywhere", not just in the repo you are standing in.

```bash
blotter sweep ~/code/api ~/code/web
blotter sweep --registry ~/.config/blotter-repos.txt --since 14d --kind all
```

Each path is a repository directory or a direct JSONL log. A **repository directory** means a directory inside a git working tree: sweep walks up to the nearest `.git` and reads `<repo root>/.blotter.jsonl`. A directory that holds a `.blotter.jsonl` but is not under git is skipped with `not a repository directory` — point sweep at the log file itself in that case. A registry is a plain text file you own with one path per line; blank lines and `#` comments are ignored, and relative paths resolve from the registry file's own directory. Like `--file`, it must name a regular file: a directory, or bytes that are not UTF-8, is `invalid_input` (exit 65) rather than a generic I/O failure, and a missing registry is `not_found` (exit 66). `blotter` never creates or looks for a registry on its own — there is no blotter-owned config file.

Sweep reads one log at a time under a shared lock and never writes. `BLOTTER_FILE` is ignored and the global `--file` flag is rejected, because sweep's inputs are its arguments. A path that is locked, unreadable, or not a repository directory becomes a skip warning and does not fail the run: sweep exits 0 with `totals.repos_skipped` set, deliberately unlike the exit-75 lock-timeout rule elsewhere. Check `totals.repos_swept` against the number of paths you passed — an all-skipped run still exits 0. Records tagged `auto` are excluded by default; pass `--include-auto` to include them.

## Export

`export` is a read-only bridge from folded cuts to OpenTelemetry. It writes one OTLP 1.11.0 `LogsData` JSON object as a single line on stdout — a raw-output exception alongside `--format md`, not the usual envelope, so `--pretty` does not apply. Pipe it to a collector or write it to a file the OTel file exporter reads.

```bash
blotter export --format otlp-json
blotter export --format otlp-json --since 30d > friction.otlp.json
```

`--format otlp-json` is required: a bare `export` is `invalid_argument`, reported before the clock is read. `--since` takes a full RFC3339 timestamp or an `Nd`/`Nh` duration. Records tagged `auto` are excluded by default; pass `--include-auto` to include them. Only cuts are exported — dogears are out of scope.

Cuts of every status are exported, and the status travels as the `blotter.friction.status` attribute (`open`, `resolved`, or `dropped`) rather than as a selector: there is no flag to export one status. Each cut becomes a log record with `eventName` `blotter.friction.reported`, a decimal-string `timeUnixNano`, the cut text as the body, severity mapped to OTLP (`minor`/`major`/`blocker` → `INFO`/`WARN`/`ERROR`), and `blotter.friction.*` attributes for id, severity, status, agent, tags, and `cwd`; a resolved cut also carries `blotter.friction.resolved_ts`.

Evidence fields are never exported. A failed command, its stderr, and free-form evidence notes are the parts of a cut most likely to hold local paths or secrets, so the outward mapping leaves them in the log. Trace and span identity is absent for the same reason — the bridge reports friction, it does not join your traces — and so is `schemaUrl`.

Output is deterministic: records sort by timestamp, then by id, and an empty selection is a stable empty record list at exit 0. OTLP types `timeUnixNano` as an unsigned 64-bit value, so a selected record whose timestamp falls outside that range (pre-1970, or past the ceiling) rejects the **whole** export with `invalid_input` (exit 65), naming the offending record and timestamp. There is no partial output and no silently skipped record; correct that record, or exclude it with `--since`, then export again.

## Hooks

**The Claude Code auto-capture lane is retired.** `blotter hook install claude-code` no longer exists, and `blotter hook exec claude-code` files nothing.

It used to auto-file a minor `auto`-tagged cut for every failed Bash tool call. Ten days of dogfooding produced 27 such records and no reader: they were hidden from every read command by default, and what they stored was a failed command line with no statement of why the failure mattered. A non-zero exit is not a claim that something got in the way, and successive gates narrowed the lane without ever making a captured record worth reading. File friction by hand with `blotter add` — one or two sentences saying what you were doing and what got in the way. That is the channel.

Records already tagged `auto` stay in the log, because the log is append-only. They remain hidden from `list`, `triage`, `digest`, `verify`, `sweep`, and `export` by default; pass `--include-auto` to read them. `retrospect` still includes them without a flag.

`blotter hook exec claude-code` survives as a no-op receiver so a settings file installed against an older binary cannot break the session it was meant to observe: it reads and discards stdin, resolves no clock, opens no log, keeps stdout empty, and always exits 0. `BLOTTER_HOOK_EXPLAIN=1` makes it write one stderr line naming the retirement; any other value keeps it silent. Delete the `hooks.PostToolUseFailure` entry naming `blotter hook exec claude-code` from your Claude Code settings — blotter no longer writes another program's configuration at all.

## Doctor

`doctor` inspects every physical line of one log and reports what it finds. Exit 1 means findings, exit 0 means healthy. `--fix` repairs only the three unreadable-line kinds; everything else is diagnose-only and needs a human decision:

| Finding | `--fix` | What to do |
|---|---|---|
| `torn_line`, `malformed`, `conflict_marker` | yes | Run `blotter doctor --fix`. Removed lines are quarantined verbatim. |
| `id_conflict` | no | A record's ID does not recompute from its payload, usually because it was written before an ID-format change. Leave it — see below. |
| `duplicate_cut`, `duplicate_dogear` | no | First-wins fold warnings. Harmless — compaction is not worth a rewrite. |
| `orphan_resolve` | no | Either a resolve event whose ID matches no record — often merge ordering, harmless to the fold — or an amend for a known record that has no base resolve anywhere in the log. The second cannot come from merge ordering, so it points at a truncated or hand-edited log; append the missing base resolve. One finding per orphan line. |
| `unknown_kind` | no | A record kind this build does not know. Left alone for forward compatibility. |
| `gitignored` | no | Fix `.gitignore`, not the log. |

`doctor --fix` repairs unreadable lines by writing a repaired copy and atomically swapping it in — the original is kept as a timestamped backup and every removed line is preserved verbatim in `<log>.quarantine.jsonl`. `--dry-run` plans the repairs without writing.

`doctor --leaks` adds a public-log gate without changing normal doctor output. It scans the raw bytes of every physical line, including malformed lines, for current or generic Unix home paths and reports a diagnose-only `leak` finding. Use it before a push or in CI; add repeatable `--deny LITERAL` values for other literal substrings your repository must not publish. `--deny` requires `--leaks`; both conflict with `--fix`, so the gate stays read-only.

An unhealthy report is therefore not always something to repair. `id_conflict` in particular has **no correction workflow, by design**. There is no event that rewrites a record's ID or payload: the fold keeps the first record it sees for an ID, so appending a line with the same ID is silently ignored, and `resolve --amend` only replaces resolution fields. The record is not broken — it still folds, lists, and resolves by its stored ID. The finding is a note that the ID predates the current hash, not a defect to repair, and it will keep appearing in every `doctor` run. Changing those bytes means editing the log outside `blotter`, which breaks the append-only invariant; back the file up first and treat it as a deliberate exception, not routine maintenance. If a record's *content* is wrong, the append-only answer is to file a corrected cut and resolve the old one — that supersedes the content but leaves the `id_conflict` finding in place.

## Archive

`archive` is the retention command: it retires history that is finished and old, and leaves everything else alone. A record group is removed only when **both** conditions hold — its materialized state is resolved or dropped, and every event in the group (the record and its resolves) is older than `--before`. An open cut, or a closed cut whose resolve landed after the cutoff, stays. So do orphan resolves, malformed lines, unknown record kinds, and legacy `pc_` records: only `bl_` groups are eligible.

```bash
blotter archive --before 180d --dry-run   # plan only, writes nothing
blotter archive --before 180d             # apply
blotter archive --before 2026-01-01T00:00:00Z
```

`--before` is required and takes the same value grammar as `--since`: a full RFC3339 timestamp or an `Nd`/`Nh` duration. The cutoff is exclusive.

Nothing is destroyed. Applying writes two files next to the log first — a timestamped backup of the original, and `<log>.archive-<ts>.jsonl` holding every removed physical line verbatim, newline-terminated, in original order — and only then atomically swaps the kept lines into place. The envelope reports `archived` and `kept` line counts, the `backup` and `archive_file` paths, and a paste-ready `restore_hint` (a `cp` that puts the original back). If either sidecar write or the swap fails, the files it created are removed and the log is untouched. When nothing is eligible, no file is written: `changed:false`, exit 0.

If the log is a symlink, the swap follows it and lands on the real target, so the link survives.

## Give your agents the pen

Paste this into your `CLAUDE.md` / `AGENTS.md` / system prompt:

```markdown
## Blotter

Run `blotter list` first to see what is already known. Do not add global,
system, or internal friction.

When you hit friction during work — a dead-end tool call, a broken link, a
misleading doc, a footgun config, a missing helper — file it before moving on:

    blotter add "<what you hit and what would have prevented it>" --tag <area>

When you have an idea worth keeping — a measurement, a gap, a pattern — dogear
it the same way:

    blotter dogear "<the idea>" --tag <area>

Don't stop working; file it and push through. Severity: blocker if you could
not proceed, major if you lost real time, minor (default) for a papercut. Run
`blotter schema` once if you need the full contract. Attach `--cmd`, `--exit`,
or `--stderr-file` when filing tool failures; never feed raw environment dumps.
```

Then periodically: `blotter list --format md` and fix what your agents keep tripping over, and `blotter list --kind dogear` to see what they've been thinking.

## Team modes

**Committed (default).** `.blotter.jsonl` is a normal tracked file — records appear in diffs and PRs. Add this to `.gitattributes` so parallel branches merge cleanly:

```
.blotter.jsonl merge=union
```

Duplicate lines after a merge are harmless — the fold is first-wins and `blotter add` is duplicate-safe.

**Private.** Prefer not to commit them? `echo .blotter.jsonl >> .gitignore`, or point `BLOTTER_FILE` somewhere else entirely. Outside a git repo, records go to `~/.blotter/log.jsonl`.

**Historical papercuts migration.** Earlier releases instructed users to run `mv .papercuts.jsonl .blotter.jsonl` (and update `.gitignore`/`.gitattributes`); a rename preserves every byte. Current releases neither discover `.papercuts.jsonl` nor emit migration warnings. Existing records remain readable after that cutoff.

## Contract

Everything an agent needs is in `blotter schema`: commands and flags with read-only/appends annotations, env vars (`BLOTTER_FILE`, `BLOTTER_AGENT`, `BLOTTER_NOW`, `BLOTTER_HOOK_EXPLAIN`), record shapes, error codes, and the exit-code dictionary (0 success · 1 command findings · 2 usage · 65 bad input · 66 not found · 70 internal · 74 I/O · 75 lock timeout, retryable · 77 permission denied · 78 config). Empty results are exit 0, never errors.

Exit 1 is not an error — it is a finding count. `doctor` returns it for an unhealthy log, `triage` for at least one chronic cluster, `verify` for at least one recurrence, and `retrospect` for at least one promotion candidate. Each command's own `exit_codes` entry in `blotter schema` says which meaning applies.

## License

MIT
