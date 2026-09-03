# Hide hook auto-captures from default reads

Date: 2026-08-09
Issue: [BigCactusLabs/blotter#19](https://github.com/BigCactusLabs/blotter/issues/19)
Task: TASK-26
Status: implemented (0.13.0, PR #20, 2026-08-09). Archived 2026-08-19 — 0.13.0 is no longer the current release; kept for provenance. For current behaviour see the design doc (r17, and r29 for the hook's eligibility gates) and `blotter schema`, not this file. Surface removed in 1.0.0 (design doc r48).

## Problem

`blotter hook exec claude-code` files a `cut` for every eligible failed `Bash` call. The
record `text` is the raw command string and `evidence.note` is the raw stderr. Nothing
states what the friction was or what would have prevented it.

In the reporting session, 3 of 8 open cuts in one log were hook auto-captures, and all
three were ordinary command failures that the agent fixed on its very next call:

| ID | Captured command | Failure |
| --- | --- | --- |
| `bl_7702529d8063` | `gh pr merge 19 --merge && git pull --ff-only origin main` | PR was still a draft |
| `bl_0d83d9bf3b85` | `gh pr ready 19 && gh pr merge 19 --merge && git pull …` | dirty tree aborted the `--ff-only` pull |
| `bl_5229a4fdca16` | `git worktree add … chore/test-value-sweep` | branch already checked out elsewhere |

Hand-filed cuts and auto-captures share the same kind, tag surface, and default severity,
so a reader cannot tell them apart without opening each record. The `auto` tag exists but
no read command acts on it.

## Rejected alternatives

- **Filter benign failure classes at capture time.** Hardcodes `gh` / `git` / worktree
  string patterns into a general-purpose tool, and still misses the general case: most
  single command failures are trial-and-error, not friction.
- **A third record kind (`event`).** Structurally the cleanest split, but it breaks the
  record format and touches the fold, `list --kind`, `triage`, `digest`, `verify`,
  `doctor`, and `schema`. That belongs with TASK-19 (next-major breaks).
- **File only on a repeat failure.** Attacks the root cause but needs new state to
  remember failure #1, which the append-only log has no cheap place for.
- **Stop filing; make the hook a nudge.** Discards evidence that is occasionally useful
  and depends on agent follow-through the hook cannot enforce.

## Design

### The rule

A record is an **auto-capture** when its `tags` contain `auto`.

The five reporting commands — `list`, `triage`, `digest`, `verify`, `sweep` — exclude
auto-captures by default. `--include-auto` turns the exclusion off. On `list`, an explicit
`--tag auto` implies `--include-auto`, because asking for the thing and receiving nothing
is a defect; `list` is the only affected command with a `--tag` flag, so no other command
needs that rule.

The `auto` tag is written only by `hook exec claude-code` today, but a hand-filed
`blotter add --tag auto` also drops out of the default view. This is intended: the tag
means "machine-filed, not analysed", and applying it deliberately is a valid signal.

The tag is the discriminator rather than a new `source` field because records already in
the wild carry only the tag; a new field would need the tag as a fallback anyway.

### Filter placement

One shared predicate keeps the rule in a single place:

```rust
pub fn is_auto_capture(tags: &[String]) -> bool
```

**Each command applies it once, to the folded item list, immediately after the fold and
before any command-specific analysis.** This is the whole mechanism; there is no
per-command filter logic.

Filtering later would be wrong in at least one command: `digest` derives its `chronic`
clusters from `triage::triage(items.clone(), 2)` at `digest.rs:91`, before the open-item
loop at `digest.rs:97`, so a filter placed at the loop would leave `chronic` unfiltered.
Filtering the input list is the only placement that is uniformly correct.

Concretely, the filter goes on `folded.items` before it reaches `digest(...)`
(`digest.rs:76`), `verify(...)` (`verify.rs:77`), `triage::triage(...)` (`triage.rs:79`),
`list`'s filter chain (`list.rs:46`), and each per-log item vector in `sweep`.

### Auto filtering is orthogonal

Auto filtering composes with, and never overrides, every other selector. `--include-auto`
widens nothing except the auto exclusion:

- It does not change the selected kind. `list --kind cut --include-auto` still returns
  cuts only. An auto-tagged **dogear** is hidden by default like any other auto record and
  appears only under both `--kind dogear`/`--kind all` and `--include-auto`.
- It does not change status. A **resolved** auto-capture is still hidden under
  `--status all` unless `--include-auto` is also given.
- `--agent`, `--severity`, and `--since` are unaffected.

### Per-command effect

| Command | Effect |
| --- | --- |
| `list` | `count`, `total`, and `truncated` all describe the filtered set |
| `triage` | clusters never form out of near-identical command strings; `scanned` counts what the analyzer saw, i.e. post-filter |
| `digest` | `chronic`, `new_cuts.count`, `new_cuts.by_tag`, and `open_dogears` all exclude auto records |
| `verify` | an auto-capture is neither a resolved anchor nor a later open cut, so it can be neither the anchor nor the recurrence evidence; `scanned` is post-filter |
| `sweep` | per-log `counts`, `by_tag`, `items`, and the aggregate `totals` all exclude auto records; `auto` and `claude-code` leave `by_tag` by default |

`triage` and `verify` exit 1 when they find something and 0 when they do not. Filtering can
therefore flip a run from exit 1 to exit 0. That is the intended consequence and must be
covered by tests.

### Visibility

Hiding records silently trades one blind spot for another. When a command drops records it
appends this `meta.warnings` line, with `N` the count of dropped records:

```
N auto-captured records hidden; use --include-auto to include them
```

Exact semantics, so no implementer has to guess:

- **Hidden** means: the record passed every other command-specific filter (kind, status,
  agent, tag, severity, since) and was dropped only because it is an auto-capture.
- The count is taken **before** any output truncation, so `list --limit` does not change
  it.
- The warning appears only when `N > 0`, and is appended **after** the existing discovery
  and fold warnings, so warning order stays stable.
- `sweep` emits **one** aggregate line summing dropped records across the distinct
  canonical logs it actually swept. `sweep` deduplicates canonical log paths before
  reading (`sweep.rs:68`–`99`), so each log contributes once; there is no cross-log record
  ID deduplication.
- `list --format md` and `digest --format md` both carry the line through their existing
  trailing `> note:` warning path.

### Discoverability

`schema` is the machine contract and the surface an unfamiliar agent reads first, so the
change must appear there. `schema all` lists `--include-auto` in the flag map of `list`,
`triage`, `digest`, `verify`, and `sweep`, and each of those five commands gains a
`semantics` note stating that records tagged `auto` are excluded by default, plus, for
`list`, that `--tag auto` implies `--include-auto`.

`resolve`'s not-found and invalid-ID guidance currently tells the reader to run
`blotter list --status all` (`resolve.rs:213`, `resolve.rs:241`), which would now hide the
very record they are hunting. Both fixes become
`blotter list --status all --include-auto`.

### Unchanged

Record format, `compute_id`, the fold, and the append path. `hook exec claude-code` still
files, still dedupes against open cuts by `evidence.cmd`, still tags `auto` and
`claude-code`, and stays fail-open at exit 0.

Two read paths deliberately keep seeing everything:

- **`doctor`** inspects physical lines for integrity and must see every record, filtered
  or not. Filtering it would make `checked_lines` lie.
- **The hook's own open-command dedupe** (`hook.rs:223`–`243`) must still see open auto
  cuts, or every replayed failure would file a fresh duplicate.

`add`, `dogear`, and the `resolve` record path are untouched, and `resolve <id>` still
resolves an auto-capture by ID.

All four repository invariants hold: append-only, stdout carries one envelope, output stays
deterministic under `BLOTTER_NOW`, and cut-only output stays cut-only.

### Contract

`meta.contract` bumps **4 → 5** (`src/output.rs:6`).

The repo's own precedent settles this. r12 (`plan:284`) bumped 3 → 4 for a behaviour
change; r13 (`plan:298`) explicitly held at 4 because "every change below is additive …
existing logs, commands, and output shapes are unchanged." This change is not additive:
five commands return fewer records by default. Contract version exists so a consumer can
detect skew (`plan:67`), and a consumer parsing `blotter list` is exactly who skews here.

This lands as an amendment to `docs/plans/2026-07-09-papercuts-design.md` and must be
called out in the release notes as a behaviour break.

## Documentation

Documentation ships in the same change, not as follow-up. Four surfaces, none optional:

### `docs/plans/2026-07-09-papercuts-design.md` — normative

A new amendment `### r17 (2026-08-09, 0.13.0 auto-capture default exclusion)`, appended
after r16, following the existing amendment style: normative prose, no bullet-point
summary. It must state the `auto`-tag rule, the five affected commands, the
`--include-auto` flag, `list`'s `--tag auto` implication, the filter's placement above all
command-specific analysis, the exact warning text and its pre-truncation count, `sweep`'s
single aggregate line, the deliberate exclusion of `doctor` and the hook's own dedupe, and
the `4 → 5` contract bump. Because amendments accumulate and the newest wins, r17 is the
authority for anything it contradicts in earlier sections.

### `CHANGELOG.md`

A `## [0.13.0]` entry. The one-line preamble on every recent entry states additivity; this
one must state the opposite, in the same position and voice:

> Breaking: envelope `meta.contract` bumps 4 → 5. `list`, `triage`, `digest`, `verify`,
> and `sweep` exclude records tagged `auto` by default; pass `--include-auto` for the
> previous behaviour.

Then a `### Changed` section describing the default-read change and the contract bump, and
an `### Added` section for the `--include-auto` flag, each citing design doc r17.

### `README.md`

Six edits, because the README describes each read command independently:

- The command tour (lines 30–35) — annotate that the five reads show hand-filed records by
  default.
- The hook section (line 73), which today explains what the hook files and its two noise
  guards. It gains a third paragraph: auto-captured cuts are hidden from the five reads by
  default, and the reasoning — the hook captures that a command failed, not why it
  mattered, so those records are evidence rather than analysis.
- `triage` (line 95), `verify` (line 103), `digest` (line 113), and `sweep` (lines
  124–133) — one sentence each naming the default exclusion and `--include-auto`.
- The workflow line at 156 (`periodically: blotter list --format md`) — no change needed,
  but confirm the surrounding advice still reads correctly once auto records are hidden.

### `AGENTS.md`

Edit `AGENTS.md`, never the `CLAUDE.md` symlink. Two edits:

- **Invariants** — a new line beside the existing cut-only rule, in the same voice:
  "Records tagged `auto` are excluded from `list`, `triage`, `digest`, `verify`, and
  `sweep` unless `--include-auto` is explicit."
- **Layout** — the `src/commands/*.rs` bullet already characterises `triage`, `digest`,
  `verify`, and `sweep`. Add the shared predicate and its placement so the next agent does
  not reintroduce a per-command filter.

The Dogfood section needs no change: it instructs agents to file cuts by hand, and
hand-filed cuts are exactly what stays visible.

## Testing

Black-box tests in `tests/cli.rs`, environment set through `Command::env` only. A fixture
log mixing hand-filed and auto-capture records — cuts and dogears, open and resolved —
then:

**`list`**
- Default hides auto-captures; `--include-auto` shows both; `count`, `total`, and
  `truncated` are correct in each case.
- `--tag auto` returns the auto-captures without the flag.
- `--limit` smaller than the match count does not change the hidden count.
- `--status all` still hides a resolved auto-capture; `--include-auto` reveals it.
- `--kind dogear --include-auto` reveals an auto-tagged dogear; `--kind cut
  --include-auto` does not.
- `--since` composes with the filter.
- The hidden-count warning is present when records were dropped, absent when none were,
  and ordered after discovery/fold warnings.
- `--format md` prints the note.

**`triage` / `verify`**
- A cluster that would form only out of auto-captures does not form by default and does
  form with `--include-auto`; the exit code flips 0 → 1 accordingly.
- `scanned` reflects the post-filter count.
- `verify`: no recurrence when the only later open cut is an auto-capture; no recurrence
  when the only resolved anchor is an auto-capture; both appear with `--include-auto`.

**`digest`**
- Default excludes auto records from `chronic`, `new_cuts.count`, `new_cuts.by_tag`, and
  `open_dogears`; `--include-auto` restores each.
- `--format md` prints the note.

**`sweep`**
- Per-repo `counts`, `by_tag`, `items`, and `totals` exclude auto-captures by default.
- `--kind` composes with the filter, and the per-repo 50-item cap still applies.
- One aggregate hidden-count warning, not one per repo.

**Contract and discovery**
- `meta.contract` is 5.
- `schema all` lists `--include-auto` for all five commands.
- `--help` for each of the five names the flag.

Documentation is verified by review, not by test: before the change is called done, confirm
r17 exists in the design doc, the `0.13.0` CHANGELOG entry leads with the breaking notice,
the six README edits are present, and `AGENTS.md` carries the new invariant line.

## Out of scope

Issue observation #2 — the hook dirtying `.blotter.jsonl` and aborting a `git pull
--ff-only` — is repo hygiene in the reporting repository, where the log is tracked by git.
The fix belongs there, not in blotter. If blotter should help, a `doctor` finding for "log
file is tracked and a hook is installed" is a separate task.
