# Blotter CLI reference

Operational behavior for this checkout. Run `blotter schema` for the complete flags, record shapes, errors, and annotations of the **installed executable**; `blotter COMMAND --help` gives its usage. Contributors use the [current implementation contract](contract.md), not superseded design amendments. Start with the [README](../README.md) for installation or [workflows](agent-workflows.md) for task-oriented examples.

## Global flags and environment

`--file PATH` overrides log discovery for one invocation; `--pretty` indents JSON. `sweep` rejects `--file` because it owns its input paths.

| Variable | Meaning |
| --- | --- |
| `BLOTTER_FILE` | Explicit ledger path, overridden by `--file` |
| `BLOTTER_AGENT` | Agent name when no `--agent` is supplied; unset/empty falls through to detection, then `unknown` |
| `BLOTTER_NOW` | Full RFC3339 clock override for deterministic tests |

Agent detection recognizes the supported Claude Code, Codex, and Cursor environments. Use `--agent` when identity must be explicit. Relative time values accepted by applicable commands are `Nd` or `Nh`; absolute values are full RFC3339 timestamps, not bare dates.

## The log file

The default is `.blotter.jsonl` at the discovered Git repository root, or `~/.blotter/log.jsonl` outside a repository. The first write creates it. The log must be a regular file; FIFOs, devices, and directories are rejected. Multiple agents use advisory locks rather than independent read/modify/write cycles.

Ordinary writes append; resolutions are additional events, not edits to old records. `archive` and `doctor --fix` are explicit exceptions: each creates backup-preserving replacement files and atomically swaps the result into place. They never edit the log in place. Read-only commands do not create promotions or silently repair history.

Success is normally one JSON envelope on stdout; errors are structured on stderr. Raw Markdown output and OTLP export are the documented exceptions. Empty results succeed. Inspect warning and count fields as well as the exit status, especially with `sweep`.

## Records and IDs

Stored records have `"v":2` as their first member. This storage marker is omitted from normal output records. A known-kind record with an unsupported or missing version causes the log to be refused whole and left unchanged; there is no implicit migration.

Cuts, dogears, and promotions use `bl_` plus 20 lowercase hexadecimal characters. An ID argument may omit `bl_` and must supply at least four hexadecimal characters. Matching is case-insensitive; no matches is `not_found`, and multiple matches is `ambiguous_id`. A full ID does not receive special precedence over other prefix matches.

The first record for an ID wins; later duplicate records are ignored. Resolve events use the separate base/amend rules below. This makes duplicate lines from `merge=union` harmless. Do not hand-edit stored IDs or assume that changing evidence changes identity.

`cwd` is repository-relative when possible; otherwise home-path rewriting applies. `origin` is optional structured provenance. Only its published members reach envelopes; unknown stored members are not automatically exposed. Promotions have no status or resolution lifecycle.

## Evidence and redaction

`add` accepts `--cmd`, `--exit`, `--stderr-file`, and `--evidence`. `dogear` accepts an evidence string; resolutions accept notes; promotions accept artifact references and notes.

**Home-path rewriting is not secret detection.** Cut/dogear text receives home-path rewriting, not the full evidence secret-pattern pass. Evidence, resolution notes, and promotion artifact references/notes use their documented best-effort redaction. None is a confidentiality boundary. Never attach credentials, private environment dumps, or material you are not authorized to retain.

Text read from stdin and `--stderr-file` has a 1 MiB input bound. The stderr source must be a regular UTF-8 file; symlinks follow their regular-file targets. Sanitized stored stderr is limited to 4096 UTF-8 bytes. Record text and promotion reference/note limits are described by the installed schema; do not extrapolate one field's bound to every evidence field.

`doctor --leaks` diagnoses selected patterns and explicitly named deny literals. It does not certify privacy or remove secrets. Archive relocates historical bytes into a sidecar, so resolve-then-archive is not secret erasure.

## add

```bash
blotter add "The documented test command fails because the fixture setup step is missing" --tag tests --impact material
blotter add "The tool returned a misleading error" --cmd 'tool --flag' --exit 1 --stderr-file /tmp/tool-stderr
blotter add - --tag tests
```

The `log` alias also writes a cut. Omit text or pass `-` to read piped stdin. Repeat `--tag` as needed. Admission requires at least one useful ground, described under [Cuts](../README.md#cuts); impact is `low` (default), `material`, or `blocking` after admission.

`--dry-run` reports the prospective record without appending. The same identity under the same fixed clock is duplicate-safe and returns `changed:false`; independently observed recurrences should still be filed without a pre-filing search.

## dogear

```bash
blotter dogear "One observed finding, understandable without this repository" --tag research --evidence "How to reproduce the observation"
blotter finding - --tag research
```

Aliases: `finding` and `idea`. The name does not relax the admission bar: [all three finding criteria](../README.md#dogears) are required. Evidence should make the observation checkable, not turn an untested guess into a claim.

Dogears share the journal, agent selection, tags, deterministic clock, dry run, and resolve-event mechanics. They have no impact or failed-command fields. `resolve --url` records where a human published the finding; `resolve --dropped` records rejection after review. A URL is provenance, not validation of the finding or a request to fetch it.

## promote

```text
blotter promote --source CUT_ID --artifact-type doc --artifact-ref docs/testing.md
blotter resolve CUT_ID --disposition promoted --promotion PROMOTION_ID
```

Replace IDs with actual records and the path with an approved artifact that exists. Artifact types are `doc`, `skill`, `guard`, `test`, `tool`, and `process`. Repeat `--source` for multiple cuts; sources may be open or resolved, but not dogears or promotions. IDs are canonicalized, sorted, and deduplicated.

Artifact references and notes are redacted before storage; the reference participates in identity, the note does not. An existing promotion identity returns the existing record with `changed:false`. Unlike `add` and `dogear`, `promote --dry-run` reads/folds the log to validate sources, so a missing source is still an error.

Promotion does not resolve anything automatically. A `resolve --promotion` link is permitted only with promoted disposition and must be mutual: that promotion must already list every cut being resolved. Validation rejects the whole batch before any append. Recurrence never authorizes automatic promotion, artifact creation, or publication.

## resolve

```text
blotter resolve CUT_ID --disposition fixed
blotter resolve CUT_ID OTHER_CUT_ID --disposition accepted
blotter resolve CUT_ID --amend --note "Corrected explanation" --pr PR_URL
blotter resolve DOGEAR_ID --url PUBLICATION_URL
blotter resolve DOGEAR_ID --dropped
```

ID tokens above are placeholders. Resolve validates the complete batch and appends atomically. Its response always uses `data.records`, including a single target. `--task`, `--pr`, and `--commit` provide provenance for cuts or dogears; promotions cannot be resolve targets.

Every base cut resolution requires `--disposition fixed|promoted|accepted|invalid`. Accepted means deliberately tolerated; invalid means never qualifying friction. Dogears reject disposition. `--url` and `--dropped` are dogear-only, mutually exclusive, and rejected for mixed-kind batches. Mixed cut/dogear batches are rejected even under `--amend`.

An amendment requires already-resolved targets and at least one resolution field. The first non-amend resolve remains the base; the latest eligible amend **by timestamp**, not physical file order, wins the materialized view. A clock behind an existing amend does not take over; output reports the actual winner.

Amendments **replace ordinary fields**, not merge them: a note-only amend drops an earlier pull-request field unless you repeat `--pr`. Disposition and its timestamp are special: omitting disposition inherits the current classification and `disposition_ts`; supplying it stamps a new classification time. Consequently, correcting a note does not move the recurrence boundary. The promotion link is retained while disposition remains promoted and cleared when changing to another disposition.

## list

```bash
blotter list
blotter list --format md
blotter list --kind dogear
blotter list --kind promotion
blotter list --kind all --status all
blotter list --since 7d --limit 100
```

Default: open cuts, limit 50. Cuts sort by impact (`blocking`, `material`, `low`), then timestamp descending, then ID ascending. Dogears follow cuts and promotions follow dogears, each timestamp-descending/ID-ascending.

`--impact` is accepted only for cut kind. `--tag` and explicit `--status open|resolved` are rejected with promotion kind. With `--kind all`, the implicit default includes promotions, but an explicit open/resolved status excludes them; `--status all` includes all folded records. Inspect `count`, `total`, and `truncated` rather than assuming the returned page is the whole log.

## triage

```bash
blotter triage --min-count 3
```

Read-only clusters of open cuts; default minimum 3, minimum allowed 2. Equal nonempty normalized titles link regardless of tags. Otherwise cuts must share a tag, or both be untagged, and satisfy either 80% overlap of the shorter filtered token set or three shared locally rare tokens. The rarity ceiling is `max(2, ceil(scanned / 16))`. Short tokens, normalized Snowball stopwords, and retained fillers `need`, `one`, `use`, and `uses` are excluded.

Clustering uses the established representative-linkage implementation, not a promise that every member matches every other member. `occurrences` counts members sharing the displayed normalized title. Resolved cuts and dogears are excluded. Exit 1 signals at least one qualifying cluster; exit 0 means none.

## verify

```bash
blotter verify
```

Read-only recurrence detection. Anchors are eligible resolved cuts whose winning disposition is fixed or promoted; accepted, invalid, dropped, blank-title, and dogear records are excluded. A recurrence is a matching open cut strictly **after `disposition_ts`**, using the same title/tag/token relation as triage. Note-only amendments do not move that boundary.

One open cut can match several resolved anchors. `count` counts matched anchors; `distinct_recurring_cuts` counts unique later open cuts. Exit 1 signals recurrences. No matches means none were observed in this ledger, not proof that a fix can never fail again.

## retrospect

```bash
blotter retrospect
```

Read-only candidate mining, with no time-window flag. `pattern` describes evidence; `suggested` lists potential artifact types. Chronic open-cut clusters use a two-record candidate threshold. A shared failing program in at least half the cluster suggests tool/guard; otherwise a qualifying docs-tagged cluster suggests doc. The program rule has precedence. These are `recurrent_friction` candidates.

Qualifying verification groups with at least two recurrences produce `failed_intervention` candidates suggesting a skill. A cluster matching no candidate rule emits nothing. Evidence consists of bounded texts and resolution notes, not captured evidence commands, stderr, or evidence notes. Exit 1 signals candidates. The command creates no artifact and appends no promotion.

## digest

```bash
blotter digest --since 7d --format md
```

Default window: seven days. Chronic friction includes all open cuts at a two-record threshold, not only cuts in the window. New cuts are open cuts inside the inclusive since/until window, grouped by tag, with an empty tag for untagged records. Open findings are not restricted to the window.

JSON also reports accepted cuts classified inside the window, using `disposition_ts`, not creation or amendment time. This accepted count is not rendered in Markdown. Markdown omits empty sections; a fully empty report says “No friction in window.” Warnings can appear as trailing notes. Digest is read-only and empty results succeed.

## sweep

```bash
blotter sweep ~/code/project-a ~/code/project-b
blotter sweep --registry /path/to/repos.txt --since 7d --kind all
```

Inputs are repository directories or direct log paths. A registry is an explicit user-owned UTF-8 regular file, one path per line; blank lines and `#` comments are ignored, and relative entries resolve from its directory. At least one path is required across arguments and registry. No registry is discovered automatically.

Sweep ignores `BLOTTER_FILE` and rejects global `--file`. Its kind vocabulary is cut/dogear/all, **not promotion**, unlike list. Each log is read under its own shared lock and repositories sort by canonical log path. Counts include all open items; returned items and tag groups respect filters before the per-repository 50-item cap.

Unreadable or locked repositories become skip warnings with exit 0, even when all are skipped. Inspect `repos_swept`, `repos_skipped`, and warnings. A successful invocation is not proof that every requested ledger was read. Sweep never writes.

## export

```bash
blotter export --format otlp-json --since 7d
```

The required format emits one newline-terminated OTLP LogsData JSON object, with no success envelope or warnings. It exports folded cuts of all statuses, not dogears or promotions, sorted timestamp-ascending then ID-ascending. Captured evidence and origin fields are not emitted. No collector request, trace/span inference, or external publication happens automatically.

Impact maps to OTLP INFO/9, WARN/13, and ERROR/17. Selected timestamps outside the unsigned 64-bit Unix-nanosecond range reject the export before partial output. Authorization to retain a ledger is not authorization to transmit it to a collector.

## doctor

```bash
blotter doctor
blotter doctor --leaks
blotter doctor --leaks --deny 'literal-that-must-not-appear'
blotter doctor --fix --dry-run
```

Read-only by default. Exit 1 signals findings, not their count. Use the installed schema for the full finding vocabulary and fixability annotations. Identity conflicts, orphan resolutions, unknown kinds, and invalid resolutions are not permission to rewrite historical bytes.

`--fix` quarantines repairable torn/malformed/conflict-marker lines verbatim, preserves a backup, and swaps a repaired copy. Dry run does not write. It does not redact secrets, reconcile semantic conflicts, or migrate an unsupported log version.

`--leaks` examines decoded strings for supported home-path patterns, with a raw fallback for malformed input. Repeatable `--deny` literals are checked as raw text; specify escaped spellings too when relevant. Deny requires leaks; leaks conflicts with fix. Review the data yourself before sharing it. A clean result is not a confidentiality certificate.

## archive

```bash
blotter archive --before 180d --dry-run
blotter archive --before 180d
```

`--before` is required, accepts RFC3339 or `Nd`/`Nh`, and is exclusive. Eligible record groups must be closed and every relevant event older than the cutoff. Open records, orphan resolutions, malformed/unknown material, and non-Blotter identity lines remain. Promotions are retained; source cuts referenced by promotions are pinned rather than orphaning provenance.

Applying first writes a timestamped backup and an archive sidecar of removed physical lines verbatim, then atomically replaces the log. The response reports line counts, backup/archive paths, and a restore hint. If sidecar creation or swap fails, newly created artifacts are removed and the original log is preserved. Nothing eligible means `changed:false`, exit 0, and no new files. A log symlink is preserved while its target is replaced.

Archive is reversible housekeeping, not secret removal or a mechanism to revise a false historical claim.

## schema

```bash
blotter schema
blotter schema record
blotter schema error
blotter schema exit-codes
```

The complete machine interface includes commands, flags, record shapes, environment variables, errors, exit codes, admission guidance, and read-only/appends annotations. Query the installed executable rather than relying on an example copied from another release.

## Exit codes

| Exit | Meaning |
| --- | --- |
| 0 | Success, including empty results |
| 1 | Documented command findings: doctor, triage, verify, retrospect |
| 2 | Usage error |
| 65 | Invalid input, including unsupported known-kind log versions |
| 66 | Not found |
| 70 | Internal error |
| 74 | I/O error |
| 75 | Lock timeout; retryable |
| 77 | Permission denied |
| 78 | Configuration error |

Exit 1 is a **finding status, not the number of findings**. Parse each command's payload and schema annotation. Do not treat every nonzero result as execution failure or every zero as complete coverage of a multi-repository sweep.

## Team modes

Committed ledgers travel with the repository. Add this once to `.gitattributes`:

```gitattributes
.blotter.jsonl merge=union
```

For private ledgers, ignore `.blotter.jsonl` or select another path with `BLOTTER_FILE`. Review logs before a public push; use `doctor --leaks` as an additional bounded check, not a guarantee. Existing `.papercuts.jsonl` files are historical data, not the current discovery filename.

## Upgrading from 0.15

Two explicit steps are required before using a current binary in an old repository:

1. Remove the Claude Code `hooks.PostToolUseFailure` entry naming `blotter hook exec claude-code`. The hook subcommand was removed; leaving that entry produces invalid-argument noise on tool failures.
2. Move the old `.blotter.jsonl` out of the discovery path to a destination that **does not already exist**. Preserve its bytes; the next add creates a new v2 ledger. Do not overwrite an existing backup or hand-edit version fields.

A legacy known-kind log is refused whole with `unsupported_log_version` at exit 65. There is no migration command, upcaster, partial fold, or implicit repair. To inspect legacy history, retain a separate 0.15 binary and select that old file explicitly:

```bash
cargo install blotter-cli --version 0.15.0 --root ~/.blotter-0.15
```

The current format uses new identities and impact/disposition fields; renaming a file alone does not convert its records. See [historical release notes](history.md) for the complete original transition rationale.

## What is stable

The supported interface is commands/flags/environment, JSON envelopes, stored JSONL, exit codes, and executable schema. A breaking change requires an explicit contract-number change and the appropriate major release; additive surface can be a minor release on the same contract. The Rust library `blotter::*` structures the implementation and is not a supported integration API. Integrate through the CLI.
