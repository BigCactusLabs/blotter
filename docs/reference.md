# blotter reference

The full behaviour of every command, flag, and record rule, written for a person. It is descriptive, not normative: `blotter schema` is the machine contract, and `docs/plans/2026-07-09-papercuts-design.md` (read to its last amendment) is the law when this page disagrees. The [README](../README.md) is the short version.

## Contents

- [Global flags and environment](#global-flags-and-environment)
- [The log file](#the-log-file)
- [Records and IDs](#records-and-ids)
- [Evidence and redaction](#evidence-and-redaction)
- [add](#add)
- [dogear](#dogear)
- [promote](#promote)
- [resolve](#resolve)
- [list](#list)
- [triage](#triage)
- [verify](#verify)
- [retrospect](#retrospect)
- [digest](#digest)
- [sweep](#sweep)
- [export](#export)
- [doctor](#doctor)
- [archive](#archive)
- [schema](#schema)
- [Exit codes](#exit-codes)
- [Team modes](#team-modes)
- [Upgrading from 0.15](#upgrading-from-015)
- [What is stable](#what-is-stable)

## Global flags and environment

Two global flags apply to every subcommand: `--file PATH` overrides log discovery for one invocation (same target as `BLOTTER_FILE`), and `--pretty` indents the JSON envelope for human reading. The one exception is `sweep`, which rejects `--file` because its inputs are its arguments.

Three environment variables: `BLOTTER_FILE` names the log, `BLOTTER_AGENT` names the agent when no `--agent` flag is given (an unset or empty value falls through to detection of `claude-code`, `codex`, or `cursor`, else `unknown`), and `BLOTTER_NOW` pins the clock (full RFC3339) so the same input produces byte-identical output.

## The log file

Records live in an **append-only JSONL file** — by default `.blotter.jsonl` at your repo root, so every entry shows up in `git diff` and travels with the repo. Outside a git repo, records go to `~/.blotter/log.jsonl`. No server, no sync, no telemetry. The file is the product.

- **Agent-first contract**: stdout is data only; one JSON envelope per command; structured errors on stderr with stable codes, documented exit codes, and a paste-ready `suggested_fix`. `blotter schema` returns the whole contract.
- **Concurrency-safe**: multiple agents on one file are fine (advisory locking, atomic appends, self-healing torn lines).
- **Deterministic**: content-addressed IDs — a cut's identity covers its timestamp, agent, text, impact, and sorted tags, so the same text filed under different tags is a different cut — plus stable sort and a reproducible-clock override for tests.
- **Never rewrites history**: `resolve` appends an event; the log is a journal, not a database. The two exceptions, [`archive`](#archive) and [`doctor --fix`](#doctor), never edit in place — each writes a replacement copy and atomically swaps it in, always preserving the original as a timestamped backup.

The log file itself must be a regular file, so a `--file` or `BLOTTER_FILE` naming a FIFO, device, or directory is rejected rather than blocking or growing without bound.

One other path appends besides the write commands: `doctor --fix` appends in the course of a repair. `blotter schema` carries the authoritative `read_only`/`appends` annotation for every command.

## Records and IDs

Every record carries a `bl_`-prefixed ID under the one `bl2` namespace — `bl_` plus 20 lowercase hex, one width for every record kind — and an ID argument is an optional `bl_` plus at least four hexadecimal digits, matched case-insensitively. A prefix that matches nothing is `not_found`; one that matches several records is `ambiguous_id` listing every match, with no exact-full-ID precedence.

Every stored line carries `"v":2` as its first member. New records omit `repo`; their `cwd` is relative to the discovered repository root when possible, and otherwise goes through the same home-path rewrite as evidence — the exact `$HOME`, a generic `/Users/<user>` or `/home/<user>`, and the dash-encoded slug harness scratchpad paths embed all become `~`, so a stored `cwd` does not trip `doctor --leaks`.

The fold is first-wins: the first record seen for an ID is the record, and a later line with the same ID is silently ignored. That is what makes `merge=union` concatenation and duplicate lines safe.

## Evidence and redaction

`add` can attach a failed command (`--cmd`), exit status (`--exit`), UTF-8 stderr file (`--stderr-file`), or free-form note (`--evidence`). Redaction covers every authored free-text field, not just `add`: `dogear --evidence` and `resolve --note`/`--amend` go through the same pass at write time. `--stderr-file` rejects non-regular files and inputs over 1 MiB before sanitized stderr is stored up to 4096 UTF-8 bytes; a symlink is followed to its target, which must itself be a regular file. Redaction is best-effort hygiene, not a security boundary; never feed raw environment dumps. Every input lane carries the same 1 MiB read bound — `--stderr-file` and text piped to `add -` or `dogear -`.

A resolution `--note` goes through the home-path rewrite too, as does a dogear's `--evidence`, so no field blotter invites you to fill can trip its own gate. Blotter rewrites home paths in dogear text at write time; the secret pass covers `--evidence` and notes, not the text.

## add

```bash
blotter add "text"                # file a cut (also: blotter log, or pipe stdin to add -)
blotter add "tool failed" --cmd 'tool --flag' --exit 1 --stderr-file /tmp/stderr
blotter add "bad response" --evidence 'request_id=abc123'
blotter add "text" --tag <area> --impact low|material|blocking
```

A cut is one or two sentences of friction: what you were doing, what got in the way. The admission bar is in the README under [Cuts](../README.md#cuts). Impact describes consequence after that decision, not whether to file: `low` (default) is a qualified cut with limited immediate cost, `material` cost real time or caused incorrect work, `blocking` stopped the task. A low-impact cut is still a cut. Tags group cuts by area; evidence flags capture the failing command without pasting it into the text.

`add` is duplicate-safe: filing the same cut twice under the same clock is `changed:false`. `--dry-run` reports the record that would be written without appending.

## dogear

```bash
blotter dogear "one finding, in your own words" --tag research --evidence "docs/research/2026-09-02-task71-linkage-precision.md"
blotter finding - --tag research  # pipe a dogear from stdin (also: blotter idea)
```

A dogear is a finding: something an agent noticed that is interesting beyond the task in front of it. The admission bar (all three of: one finding in your own words, interesting beyond this task, understandable without the repo) is in the README under [Dogears](../README.md#dogears).

Dogears use the same append-only journal, agent resolution, tags, dry-run, deterministic clock override, and resolve events as cuts. Dogears have no impact or failure-command fields; `list --impact` is therefore accepted only with the default `--kind cut`. `--evidence` carries what makes the finding checkable — a measurement, a link, a command.

A dogear is a lead, not a verified result: `resolve --url` records where a human published it, and `resolve --dropped` records that it did not survive review. Whoever publishes a dogear checks it first.

## promote

```bash
blotter promote --source bl_9f2c --source bl_a81e \
  --artifact-type skill --artifact-ref skills/testing.md \
  --note "Repeated fixture failures promoted into reusable test-authoring guidance."
```

A promotion is the third record kind: durable learning, recorded as "these experiences became this artifact". It names one or more source **cuts**, an artifact type from the closed vocabulary `doc|skill|guard|test|tool|process`, and a reference to where the artifact lives.

Sources are cuts only, open or resolved — a promotion records where learning came from, not whether the friction is closed. A dogear or another promotion as a `--source` is rejected. `--artifact-ref` and `--note` are redacted before hashing and before the append, so the hashed bytes are the stored bytes; the note is deliberately outside the ID hash, so rewording it does not make a different promotion.

`promote` never writes a resolve event, and a promotion is never resolved. Naming a cut's fate stays a separate act: `resolve --disposition promoted --promotion <id>`, whose link must be mutual — the promotion must already list that cut in its `sources`, or the whole resolve batch is refused before anything is appended.

Promotion is an explicit trust boundary: `retrospect` and `triage` are read-only and never append a promotion; `promote` is the only writer of one, and no command, hook, or default calls it. Recurrence is evidence a reader judges, never a threshold that promotes.

## resolve

```bash
blotter resolve bl_9f2c --disposition fixed   # resolve one cut (unique ID prefix ok)
blotter resolve bl_9f2c bl_a81e --disposition fixed   # resolve several atomically
blotter resolve <id> --disposition promoted --pr <url>   # attach structured graduation provenance
blotter resolve <id> --disposition promoted --promotion <promotion id>
blotter resolve <id> --amend --note "..."  # correct a resolution you got wrong
blotter resolve <id> --url <url>  # dogear: where a human published it
blotter resolve <id> --dropped    # dogear: did not survive review
```

`resolve` always returns a `data.records` array, including when only one ID is resolved. `resolve --task`, `--pr`, and `--commit` work for either kind.

**Dispositions.** Every resolution of a cut names its fate: `--disposition fixed|promoted|accepted|invalid` is required for a cut and rejected for a dogear, so a batch naming both is rejected before anything is appended. `fixed` and `promoted` are recurrence anchors; `accepted` is friction deliberately tolerated and `invalid` says the cut was never friction. An `--amend` may change the disposition and otherwise inherits it, along with `disposition_ts` — the moment the classification was made, which a note-only correction does not move.

**Dogear-only flags.** `--url` and `--dropped` are dogear-only, conflict with each other, and reject a mixed cut/dogear batch before anything is appended.

**Amending.** A resolution you got wrong is corrected, not rewritten: `resolve <id> --amend` appends a second resolve event carrying the corrected fields. The first non-amend resolve stays the base event, the latest amend wins the materialized view (`resolution.amended: true`), and every original byte stays in the log. `--amend` needs at least one resolution field and every named record must already be resolved.

"Latest" means the latest **timestamp**, not the last line in the file — a `merge=union` log concatenates branches in branch order, so the two disagree after a merge. An amend written with a clock behind a stored amend therefore does not take over the materialized view, and `resolve` reports the amend that actually won rather than the one it just wrote; `--dry-run` predicts the same answer.

An amend **replaces** the materialized resolution; it does not merge field by field. If the base resolve carried `--pr` and you amend with only `--note`, the materialized `resolution` keeps the note and drops the pull request. Repeat every field you still want:

```bash
blotter resolve <id> --amend --note "corrected" --pr <url>
```

The base resolve is still in the log, as always. It is the materialized view that `list` and `verify` read — the latest amend alone — that loses the field.

## list

```bash
blotter list                      # open cuts, impact-first then newest, JSON envelope
blotter list --format md          # human review digest
blotter list --kind dogear        # open findings, newest first
blotter list --kind promotion     # what friction has already become
blotter list --kind all --format md
blotter list --since 7d
```

The default list is cut-only, so the complaint queue and the findings queue never blur; dogears and promotions appear only with an explicit `--kind`. `--since` takes a full RFC3339 timestamp or an `Nd`/`Nh` duration. `--format md` is one of the two raw-output exceptions to the envelope rule (the other is `export`).

## triage

`triage` is a read-only scan of open cuts. Cuts whose normalized titles are identical always link, regardless of tags. Otherwise, cuts must share a tag (or both be untagged), then link from filtered tokens: 80% overlap with the shorter token set, or at least three shared tokens that appear in no more than `max(2, ceil(scanned / 16))` open cuts. At or below 32 scanned cuts the ceiling is the floor of 2, so a shared token counts only when no other open cut carries it. Filtering removes tokens of two characters or fewer, the Snowball English stopword list normalized the same way tokens are, and four retained filler words the Snowball list omits: `need`, `one`, `use`, `uses`. Only clusters that meet the threshold are reported; resolved cuts and dogears are excluded. Each cluster carries `occurrences` — how many open cuts share the normalized title of the cluster's displayed `text`. Exit 1 means at least one was found.

```bash
blotter triage --min-count 3
```

`--min-count` defaults to 3 and must be at least 2; a threshold of 1 is `invalid_argument`.

## verify

`verify` is a read-only check for cuts that reappear after they were resolved. An anchor is a resolved cut whose winning disposition is `fixed` or `promoted`: `accepted` is excluded because tolerating the friction was the deliberate decision, not a claimed fix, and `invalid` is excluded because it says the anchor was never friction at all. A later open cut recurs when it matches under the same exact-title, tag, and filtered-token linkage rules as `triage`. Dogears, dropped resolutions, and blank normalized resolved titles are ignored. One open cut can be reported against more than one resolved anchor.

```bash
blotter verify
```

The JSON envelope includes each anchor's resolution timestamp, disposition, and optional task, pull request, and commit provenance, plus the later recurrence IDs. The top-level `count` is the number of matched anchors, not the number of live problems — a single recurring cut resembling three resolved anchors reports `count: 3` and `distinct_recurring_cuts: 1`. Exit 1 means one or more recurrences were found; no recurrences is exit 0.

The "later" boundary is the winning resolution's `disposition_ts` — the moment the disposition was decided — not its `ts`, so a note-only `--amend` never moves it. An empty result means no recurrence was observed in this log after `disposition_ts`; it is evidence the intervention held, not proof that the friction is fixed.

## retrospect

`retrospect` is a read-only mining pass over one log. It asks a different question than `triage`: not "what keeps hurting" but "what has hurt often enough to be worth building something for". It reuses triage's clustering and verify's recurrence rules unchanged, then judges each result on two separate axes: what `pattern` the evidence shows, and what kind of artifact is `suggested` to answer it. A chronic cluster is a `recurrent_friction` pattern; within it, a cluster where half or more of its members share one failing leading program suggests `["tool","guard"]`, and one where half or more are tagged `docs` or `documentation` suggests `["doc"]` — the program rule wins when both match, deciding what is suggested, not the pattern. Every recurrence group of two or more members under verify's anchor rules is a `failed_intervention` pattern, suggesting `["skill"]`, because a cut that was resolved and came back is a recovery worth capturing. Only these two patterns ship. A cluster that matches no rule emits nothing and stays an ordinary cut.

```bash
blotter retrospect
```

Retrospect takes no window and no flags: chronic signal is long-horizon, so a window would hide the evidence it looks for.

Each candidate carries its record IDs, first and last timestamps, and bounded evidence: at most 10 member texts and 5 resolution notes, never a record's evidence command, stderr, or note. `occurrences` counts each distinct normalized title in the candidate once, so members that share a title do not multiply the count. Exit 1 means candidates were found, exit 0 means none.

Retrospect never writes anything — no doc, no skill, no alias, and no record in the log. It packages the argument for a promotion; a human decides whether to make it.

## digest

`digest` is the periodic read-only report: what keeps recurring, what is new, and what findings are waiting. It combines three views — chronic clusters (the triage analysis at a threshold of 2), open cuts filed inside the window grouped by tag, and all open dogears. The JSON envelope also carries `accepted_cuts: {count}` — cuts whose winning disposition is `accepted` and whose `disposition_ts` falls inside the window. `--format md` renders nothing for it; `accepted` is the one disposition that hides friction on purpose, and a bare count keeps that hide rate visible.

```bash
blotter digest --since 7d              # JSON envelope, default window
blotter digest --since 30d --format md # raw markdown, pasteable into a review
```

`--since` takes a full RFC3339 timestamp or an `Nd`/`Nh` duration. Output is byte-deterministic for a given log and clock. An empty report is exit 0, not an error.

## sweep

`sweep` rolls several repositories' logs into one read-only view — the answer to "what is annoying my agents everywhere", not just in the repo you are standing in.

```bash
blotter sweep ~/code/api ~/code/web
blotter sweep --registry ~/.config/blotter-repos.txt --since 14d --kind all
```

Each path is a repository directory or a direct JSONL log. A **repository directory** means a directory inside a git working tree: sweep walks up to the nearest `.git` and reads `<repo root>/.blotter.jsonl`. A directory that holds a `.blotter.jsonl` but is not under git is skipped with `not a repository directory` — point sweep at the log file itself in that case. A registry is a plain text file you own with one path per line; blank lines and `#` comments are ignored, and relative paths resolve from the registry file's own directory. Like `--file`, it must name a regular file: a directory, or bytes that are not UTF-8, is `invalid_input` (exit 65) rather than a generic I/O failure, and a missing registry is `not_found` (exit 66). `blotter` never creates or looks for a registry on its own — there is no blotter-owned config file.

Sweep reads one log at a time under a shared lock and never writes. `BLOTTER_FILE` is ignored and the global `--file` flag is rejected, because sweep's inputs are its arguments. A path that is locked, unreadable, or not a repository directory becomes a skip warning and does not fail the run: sweep exits 0 with `totals.repos_skipped` set, deliberately unlike the exit-75 lock-timeout rule elsewhere. Check `totals.repos_swept` against the number of paths you passed — an all-skipped run still exits 0.

## export

`export` is a read-only bridge from folded cuts to OpenTelemetry. It writes one OTLP 1.11.0 `LogsData` JSON object as a single line on stdout — a raw-output exception alongside `--format md`, not the usual envelope, so `--pretty` does not apply. Pipe it to a collector or write it to a file the OTel file exporter reads.

```bash
blotter export --format otlp-json
blotter export --format otlp-json --since 30d > friction.otlp.json
```

`--format otlp-json` is required: a bare `export` is `invalid_argument`, reported before the clock is read. `--since` takes a full RFC3339 timestamp or an `Nd`/`Nh` duration. Only cuts are exported — dogears are out of scope.

Cuts of every status are exported, and the status travels as the `blotter.friction.status` attribute (`open`, `resolved`, or `dropped`) rather than as a selector: there is no flag to export one status. Each cut becomes a log record with `eventName` `blotter.friction.reported`, a decimal-string `timeUnixNano`, the cut text as the body, impact mapped to OTLP (`low`/`material`/`blocking` → `INFO`/`WARN`/`ERROR`), and `blotter.friction.*` attributes for id, impact, status, agent, tags, and `cwd`; a resolved cut also carries `blotter.friction.resolved_ts`.

Evidence fields are never exported. A failed command, its stderr, and free-form evidence notes are the parts of a cut most likely to hold local paths or secrets, so the outward mapping leaves them in the log. Trace and span identity is absent for the same reason — the bridge reports friction, it does not join your traces — and so is `schemaUrl`.

Output is deterministic: records sort by timestamp, then by id, and an empty selection is a stable empty record list at exit 0. OTLP types `timeUnixNano` as an unsigned 64-bit value, so a selected record whose timestamp falls outside that range (pre-1970, or past the ceiling) rejects the **whole** export with `invalid_input` (exit 65), naming the offending record and timestamp. There is no partial output and no silently skipped record; correct that record, or exclude it with `--since`, then export again.

## doctor

`doctor` inspects every physical line of one log and reports what it finds. Exit 1 means findings, exit 0 means healthy. `--fix` repairs only the three unreadable-line kinds; everything else is diagnose-only and needs a human decision:

| Finding | `--fix` | What to do |
|---|---|---|
| `torn_line`, `malformed`, `conflict_marker` | yes | Run `blotter doctor --fix`. Removed lines are quarantined verbatim. |
| `id_conflict` | no | A record's ID does not recompute from its payload, usually because it was written before an ID-format change. Leave it — see below. |
| `duplicate_cut`, `duplicate_dogear`, `duplicate_promotion` | no | First-wins fold warnings. Harmless — compaction is not worth a rewrite. |
| `dangling_source` | no | A promotion names a source that resolves to nothing, or to a dogear or a promotion, in this log. Only a human knows whether the cut was wrongly archived or the promotion wrongly written. |
| `orphan_resolve` | no | Either a resolve event whose ID matches no record — often merge ordering, harmless to the fold — or an amend for a known record that has no base resolve anywhere in the log. The second cannot come from merge ordering, so it points at a truncated or hand-edited log; append the missing base resolve. One finding per orphan line. |
| `unknown_kind` | no | A record kind this build does not know. Left alone for forward compatibility. |
| `invalid_resolution` | no | A resolve event that breaks a rule of the resolution contract (a disposition on a dogear, none on a cut, a `--promotion` link that is not mutual). The fold skips it; the message names the record and every rule broken. Re-resolve the record with a valid event. |
| `unsupported_version` | no | The log was written by 0.15 or earlier. Nothing in it is diagnosed under 1.0.0 rules and nothing is changed. See [Upgrading from 0.15](#upgrading-from-015). |
| `gitignored` | no | Fix `.gitignore`, not the log. |

`doctor --fix` repairs unreadable lines by writing a repaired copy and atomically swapping it in — the original is kept as a timestamped backup and every removed line is preserved verbatim in `<log>.quarantine.jsonl`. `--dry-run` plans the repairs without writing.

`doctor --leaks` adds a public-log gate without changing normal doctor output. A physical line that parses as JSON is scanned as decoded text — every string, at every depth — for current or generic Unix home paths, so a home path hidden behind JSON's own escaping is still caught; a line that does not parse keeps a raw-byte scan of the same rules over the encoded bytes, so malformed lines stay covered. The split is on parse success, not on record validity: any line JSON can parse is scanned decoded, whether or not it is a valid blotter record. Either way it reports a diagnose-only `leak` finding. Use it before a push or in CI; add repeatable `--deny LITERAL` values for other literal substrings your repository must not publish — `--deny` always matches against raw bytes, on every line, regardless of whether it parses. `--deny` requires `--leaks`; both conflict with `--fix`, so the gate stays read-only.

An unhealthy report is therefore not always something to repair. `id_conflict` in particular has **no correction workflow, by design**. There is no event that rewrites a record's ID or payload: the fold keeps the first record it sees for an ID, so appending a line with the same ID is silently ignored, and `resolve --amend` only replaces resolution fields. The record is not broken — it still folds, lists, and resolves by its stored ID. The finding is a note that the ID predates the current hash, not a defect to repair, and it will keep appearing in every `doctor` run. Changing those bytes means editing the log outside `blotter`, which breaks the append-only invariant; back the file up first and treat it as a deliberate exception, not routine maintenance. If a record's *content* is wrong, the append-only answer is to file a corrected cut and resolve the old one — that supersedes the content but leaves the `id_conflict` finding in place.

## archive

`archive` is the retention command: it retires history that is finished and old, and leaves everything else alone. A record group is removed only when **both** conditions hold — its materialized state is resolved or dropped, and every event in the group (the record and its resolves) is older than `--before`. An open cut, or a closed cut whose resolve landed after the cutoff, stays. So do orphan resolves, malformed lines, unknown record kinds, and any record whose ID does not start with `bl_`: only `bl_` groups are eligible. A resolved cut named in any promotion's `sources` is pinned and never archived, however old the group or the promotion — severing that link would turn a durable artifact's justification into a dangling ID. Promotions have no state to close and never archive.

```bash
blotter archive --before 180d --dry-run   # plan only, writes nothing
blotter archive --before 180d             # apply
blotter archive --before 2026-01-01T00:00:00Z
```

`--before` is required and takes the same value grammar as `--since`: a full RFC3339 timestamp or an `Nd`/`Nh` duration. The cutoff is exclusive.

Nothing is destroyed. Applying writes two files next to the log first — a timestamped backup of the original, and `<log>.archive-<ts>.jsonl` holding every removed physical line verbatim, newline-terminated, in original order — and only then atomically swaps the kept lines into place. The envelope reports `archived` and `kept` line counts, the `backup` and `archive_file` paths, and a paste-ready `restore_hint` (a `cp` that puts the original back). If either sidecar write or the swap fails, the files it created are removed and the log is untouched. When nothing is eligible, no file is written: `changed:false`, exit 0.

If the log is a symlink, the swap follows it and lands on the real target, so the link survives.

## schema

```bash
blotter schema
```

Everything an agent needs: commands and flags with read-only/appends annotations, env vars (`BLOTTER_FILE`, `BLOTTER_AGENT`, `BLOTTER_NOW`), record shapes, error codes, the admission bars for cuts and dogears, and the exit-code dictionary. This page paraphrases it; `schema` is the authority.

## Exit codes

| Exit | Meaning |
|---|---|
| 0 | success, including empty results |
| 1 | command findings (`doctor` unhealthy, `triage` cluster, `verify` recurrence, `retrospect` candidate) |
| 2 | usage |
| 65 | bad input |
| 66 | not found |
| 70 | internal |
| 74 | I/O |
| 75 | lock timeout, `retryable:true` |
| 77 | permission denied |
| 78 | config |

Empty results are exit 0, never errors. Exit 1 is not an error — it is a finding count. Each command's own `exit_codes` entry in `blotter schema` says which meaning applies.

## Team modes

**Committed (default).** `.blotter.jsonl` is a normal tracked file — records appear in diffs and PRs. Add this to `.gitattributes` so parallel branches merge cleanly:

```
.blotter.jsonl merge=union
```

Duplicate lines after a merge are harmless — the fold is first-wins and `blotter add` is duplicate-safe.

**Private.** Prefer not to commit them? `echo .blotter.jsonl >> .gitignore`, or point `BLOTTER_FILE` somewhere else entirely. Outside a git repo, records go to `~/.blotter/log.jsonl`.

**Historical papercuts migration.** Earlier releases instructed users to run `mv .papercuts.jsonl .blotter.jsonl` (and update `.gitignore`/`.gitattributes`); a rename preserves every byte. Current releases neither discover `.papercuts.jsonl` nor emit migration warnings, and 1.0.0 refuses any log written before it: those records stay on disk untouched, and the 0.15 binary is the last one that reads them.

## Upgrading from 0.15

1.0.0 breaks two things at once, and both need action before the new binary runs in a repo that used 0.15.

1. **Remove the Claude Code hook.** Delete the `hooks.PostToolUseFailure` entry naming `blotter hook exec claude-code` from your Claude Code settings. The `hook` subcommand is gone, so otherwise every failed tool call puts an `invalid_argument` error envelope (exit 2, unrecognized subcommand) into the host session: Claude Code shows that stderr to the agent on `PostToolUseFailure` and blocks nothing, but it is noise on every failure.
2. **Start a fresh ledger.** Every record carries `"v":2` as its first member, and a log holding any record without it is refused whole with `unsupported_log_version` (exit 65) and left byte-identical — no partial fold, no repair, no backup. Rename the old `.blotter.jsonl` out of the discovery path to a name that does not exist yet; the next `blotter add` creates a fresh v2 log beside it. Keep a 0.15 binary (`cargo install blotter-cli --version 0.15.0 --root ~/.blotter-0.15`) and point it at the old file with `--file` when the history is wanted. Nothing rewrites the old file, ever. Record IDs all change, because every record now hashes under `bl2`.

## What is stable

The 1.x compatibility promise covers the CLI (commands, flags, env vars), the JSON envelopes, the stored JSONL record format, the exit codes, and `blotter schema`; a breaking change to any of them is a `meta.contract` bump; additive surface is a minor release on the same contract number. The Rust library the crate also builds (`blotter::*`) is the binary's implementation, not a supported integration API: its items are public to structure the binary and may change in any release without notice. Integrate through the CLI.
