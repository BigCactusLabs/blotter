# TASK-71 linkage-precision measurement (Phase 5 gate, r51)

Status: research note, not contract. Produced 2026-09-02 by the TASK-77 measurement leg (sonnet worker, hand-judged clusters), orchestrated for the r51 gate. The 0.15 (contract 5) binary was used because the v2 binary refuses v1 logs; the r44 linkage rules are identical on both. Raw envelopes and cluster dumps stayed in the job scratch directory and are not tracked.

## Caveats

- These five logs predate the v2 admission floor (r-something raising the bar
  on what counts as a cut). They are noisier than a v2 log will be: several
  clusters below are clearly "same generic activity, different specific bug,"
  which the tighter v2 admission floor may reduce but will not eliminate,
  since it does not change token-frequency statistics.
- The blotter repo's own dogfood log had already been rotated to
  `.blotter.jsonl` (fresh v2) by the time this leg ran; the v1 corpus for this
  repo is at `.blotter.v1.jsonl` (116 open cuts, matching the note's "167
  cuts/24 resolves" figure once resolved cuts and 8 dogears are excluded).
- `list --status open` defaults to `--limit 50`; without `--limit 1000` it
  silently truncates the id→text lookup used for cluster judging (not the
  triage scan itself, which has no such limit).

## Methodology

For each log: `blotter triage --min-count 2 --file <log>`. Every reported
cluster of size *k* contributes C(k,2) linked pairs. `N` in `is_rare`'s
ceiling `max(2, ceil(N/divisor))` is `candidate_count` — the count of
**all open cuts scanned** after the `is_open_cut` filter, not the min-count
threshold and not a per-comparison candidate pool.

Every cluster's member texts were read via `list --status open --limit 1000`
(id → text) and judged by hand:
- **RELATED** — all members describe the same underlying friction.
- **UNRELATED** — members share vocabulary/tags but not the friction itself.
- **MIXED** — a related core plus stray member(s); pairs touching a stray
  count as unrelated.

Judging was deliberately strict per the brief: "both about cargo" or "both
about the sandbox blocking a cache dir" does not make two cuts the same
friction unless the specific defect/behavior is the same.

## Per-log table (divisor 4, today's ceiling)

| log | N (open cuts scanned) | ceiling = max(2,⌈N/4⌉) | clusters | pairs total | pairs related | pairs unrelated | unrelated share |
|---|---:|---:|---:|---:|---:|---:|---:|
| blotter (`.blotter.v1.jsonl`) | 116 | 29 | 21 | 57 | 16 | 41 | 71.9% |
| compas | 14 | 4 | 3 | 3 | 0 | 3 | 100% |
| walkmaxx | 11 | 3 | 1 | 1 | 1 | 0 | 0% |
| origin-brands/data-platform | 27 | 7 | 8 | 10 | 6 | 4 | 40% |
| eatmoji/tools/blotter | 1 | 2 | 0 | 0 | 0 | 0 | n/a |
| **Total** | | | **33** | **71** | **23** | **48** | **67.6%** |

## Sensitivity table (divisor 4 / 8 / 16)

| log | pairs @4 | pairs @8 | pairs @16 |
|---|---:|---:|---:|
| blotter | 57 | 40 | 23 |
| compas | 3 | 2 | 2 |
| walkmaxx | 1 | 1 | 1 |
| origin-brands | 10 | 7 | 3 |
| eatmoji | 0 | 0 | 0 |
| **Total** | **71** | **50** | **29** |

Related/unrelated split re-judged at each divisor (cluster membership shifts
as the ceiling tightens, so pairs were re-read, not just recounted):

| divisor | pairs total | pairs related | pairs unrelated | unrelated share |
|---|---:|---:|---:|---:|
| 4 | 71 | 23 | 48 | 67.6% |
| 8 | 50 | 21 | 29 | 58.0% |
| 16 | 29 | 19 | 10 | 34.5% |

**Reading:** unrelated share is 67.6% at divisor 4, 58.0% at 8, 34.5% at 16.
Tightening the ceiling removes unrelated pairs faster than it removes related
ones (related pairs: 23 → 21 → 19, a 17% drop; unrelated pairs: 48 → 29 → 10,
a 79% drop), so the direction of the fix is sound, but even at divisor 16 a
third of the linked pairs are still judged unrelated.

## Recall cost — which RELATED clusters survive tightening

11 clusters were judged fully RELATED (every member genuinely the same
underlying friction) at divisor 4:

| # | log | cluster | members |
|---|---|---|---|
| 1 | blotter | zsh `===` glob-expansion recurrence | 4 |
| 2 | blotter | archive child-process test friction | 3 |
| 3 | blotter | codex background-worker artifact-write friction | 2 |
| 4 | blotter | `gh pr merge --delete-branch` branch-deletion friction | 2 |
| 5 | blotter | workflow-skill cache-root friction (verbatim reword) | 2 |
| 6 | blotter | `[skip ci]` hides a real CI gap | 2 |
| 7 | walkmaxx | TS target excludes modern Array methods | 2 |
| 8 | origin-brands | Makefile/test patch-anchor fragility | 3 |
| 9 | origin-brands | SDD helper scripts not executable | 2 |
| 10 | origin-brands | uv sandbox/cache env friction | 2 |
| 11 | origin-brands | combined-patch missing registry context | 2 |

**At divisor 8:** 10 of 11 survive intact. #2 (archive child-process, 3
members) loses its third member (`bl_15d3cb5bf402`, stream-inheritance
issue) but the remaining pair (PID ownership vs. spawn privacy) still forms
a genuine related 2-cluster — call this "reduced, not lost." **0 fully lost.**

**At divisor 16:** 6 of 11 survive intact (#1, #4, #5, #6, #7, #9).
3 are **fully lost** (no related signal remains at all):
- #3 codex worker artifact-write friction — the surviving member instead
  clusters with an unrelated "codex exec hung on stdin" cut (different
  specific failure mode; judged UNRELATED at div16, so the recluster is a
  net loss, not a lateral move).
- #10 uv sandbox/cache env friction — both members drop out of clustering
  entirely (no cluster containing either at div16).
- #11 registry-context patch friction — same, drops out entirely.

2 are **reduced but still related** (a subset survives as a smaller but
still-genuine related cluster):
- #2 archive child-process friction — 3→2 members, still related.
- #8 Makefile/test patch-anchor fragility — 3→2 members (the schema-diff-test
  member drops, leaving the two identical Makefile-`.PHONY`-anchor cuts,
  which are the closest of the three to begin with).

**Summary:** divisor 8 costs ~0 genuine recall on this corpus. Divisor 16
costs 3 of 11 tracked related clusters outright, plus degrades 2 more from
3-member to 2-member.

## Fixture floor

`cargo test --all-features triage_clusters_reworded_repeats_with_rare_shared_tokens`
was run at each divisor (rebuilding `src/commands/triage.rs` with `is_rare`'s
`div_ceil(4)` changed to `div_ceil(8)` and then `div_ceil(16)`, restoring via
`git checkout -- src/commands/triage.rs` afterward):

| divisor | result |
|---|---|
| 4 (baseline, restored) | **ok** (1 passed) |
| 8 | **ok** (1 passed) |
| 16 | **ok** (1 passed) |

The `.max(2)` floor keeps the N=2 fixture case passing at every divisor
tested, as expected — the floor is independent of the divisor.

Worktree left clean (`git status --short` empty) after restoring the file.

## 10 worst unrelated clusters (divisor 4)

1. **blotter, 5 members, 10/10 unrelated pairs** — tag `backlog`, shared
   tokens likely `backlog`/`task`. Five *different* backlog-CLI defects
   (`.md.md` double-extension bug, Done-with-unchecked-AC, worktree-vs-main
   checkout desync, archived-ID reissue, filenames-with-spaces breaking
   `grep -rl`). No two describe the same bug.
2. **blotter, 4 members, 6/6 unrelated pairs** — tag `tooling`, shared tokens
   likely `clippy`/`gate`/`test`. Four different lint failures under the
   clippy gate (redundant clone, `useless_vec`, a `deny`'d probe condition,
   a modulus-vs-`is_multiple_of` lint) — same gate, different specific lint
   each time.
3. **blotter, 4 members, 4/6 unrelated pairs (2 related)** — tag `tooling`,
   shared tokens likely `zsh`/`compound`/`command`. Conflates two distinct
   zsh-expansion bugs: unquoted `--include=*.rs` glob expansion (2 members,
   genuinely the same bug) and unquoted `===` echo-separator expansion
   (2 members, also genuinely the same bug) — but the two pairs got merged
   into one 4-member cluster because both are "zsh expansion" at the token
   level.
4. **blotter, 3 members, 3/3 unrelated pairs** — tag `tests`, shared tokens
   likely `test`/`fixture`. Three different test breakages (macOS
   `/private/var` vs `/var` path mismatch, a shadowed `command()` helper,
   `include_bytes!` resolving relative to source not test binary).
5. **blotter, 3 members, 3/3 unrelated pairs** — tag `tests`, shared tokens
   likely `test`/`fixture`/`suite`. Three different stale-fixture breakages
   (`serde_json` key ordering, sweep cwd fixture, TASK-24 schema shape).
6. **blotter, 3 members, 3/3 unrelated pairs** — tag `testing`, shared tokens
   likely `test`/`gate`/`run`. Three different testing frictions (stale
   resolve-guidance test, a hand-computed nanosecond literal error, a
   non-reproducing 5x-gate flake with the failing test name lost).
7. **blotter, 3 members, 3/3 unrelated pairs** — tag `tooling`, shared tokens
   likely `worktree`/`branch`/`pr`. Agent-worktree push-branch-naming issue,
   a heredoc-restriction workaround, and an unrelated stale `gh pr view`
   diff-size cache — the third member shares only the "PR/worktree tooling"
   tag, not the actual friction.
8. **compas, 2 members, 1/1 unrelated pair** — tag `docs`, shared tokens
   likely `docs`/`stale`. A stale doc claiming a re-derived engine id is
   still valid, vs. a stale doc claiming a tracked directory is gitignored —
   both "stale doc claim," different doc, different fact.
9. **compas, 2 members, 1/1 unrelated pair** — tag `cli`, shared tokens
   likely `compas`/`doctor`/`workspace`. Missing workspace-reset subcommand
   vs. `doctor` rejecting `-o/--out` — both CLI ergonomics gaps, different
   surface.
10. **compas, 2 members, 1/1 unrelated pair** — tag `ci`, shared tokens
    likely `ci`/`check`/`run`. A path-filter not skipping docs-only pushes
    vs. `[skip ci]` commits reading as CI-clean — both "CI signal is
    misleading," different mechanism.

