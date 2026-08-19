---
id: TASK-28.2
title: Modularize the CLI contract suite without weakening coverage
status: Done
assignee: []
created_date: '2026-08-13 03:02'
updated_date: '2026-08-19 17:24'
labels:
  - testing
  - refactor
dependencies: []
modified_files:
  - tests/cli.rs
parent_task_id: TASK-28
type: chore
ordinal: 30000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Make tests/cli.rs easier and cheaper to work in while keeping one integration-test crate. The file is 7,001 lines and mixes startup, evidence, storage, triage, verify, digest, sweep, hook, schema, and migration contracts. Detailed pure-algorithm matrices can move closer to their implementations, but public envelope, exit-code, filesystem, determinism, and concurrency behavior must remain black-box.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Split tests/cli.rs into command or behavior modules under one integration-test binary so the refactor does not add one linked crate per module.
- [x] #2 Move only pure algorithm matrices, such as detailed triage linkage or redaction cases, to unit tests.
- [x] #3 Retain black-box sentinels for every public command, envelope shape, exit code, stdout/stderr rule, filesystem mutation, deterministic output, and concurrency invariant.
- [x] #4 Record the integration test count before and after, and justify every removed or merged test by the independent behavior it still protects.
- [x] #5 The full gate passes; store or concurrency-adjacent movement triggers five cargo test --all-features runs.
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
--------------------------------------------------
Split into tests/cli/main.rs + common.rs + 20 subject modules; one integration-test binary, no new linked crates.

Test count: 270 before, 270 after. Zero tests removed, merged, renamed, or reworded — the split extracted exact line ranges from 335 computed top-level blocks that tile the original file with no gap or overlap. Verified two ways: the sorted --list name set diffs empty, and all 10,602 non-blank source lines are byte-identical after normalising module wiring. AC #4 therefore has no removals to justify.

AC #2 satisfied by moving nothing to unit tests. The criterion constrains what MAY move; the merge-conflict cost this task exists to remove is fixed by the module split alone, and converting black-box CLI tests to unit tests would trade away the coverage AC #3 protects. Extracting pure matrices (triage linkage, redaction cases) into src unit tests remains available as separate work.

AC #5: scripts/dev/gate-5x.sh 5/5 passed, 285 tests per run (270 integration + 15 unit); race, locking, discovery and torn-tail tests moved into tests/cli/store.rs.

AGENTS.md now carries the module map and the placement rule, so new tests land in the module that owns the behaviour instead of at one file's tail.

Review round (Opus 5 xhigh, PR #3): approve_with_nits, no behaviour finding. Five items fixed in 693a7f4 — six detached // --- banners deleted, single-consumer helpers moved out of common.rs, the stray auto-exclusion test moved to auto_capture.rs, AGENTS.md gained the cross-cutting tiebreak and the module-registration rule, CHANGELOG entry added.

The split introduced one failure mode the single file did not have: a tests/cli/*.rs file not declared in main.rs is compiled by nothing and its tests silently never run, with no error, warning, clippy diagnostic, or gate failure. Guarded by every_test_module_file_is_declared_in_main and verified by dropping in an undeclared file holding a panicking test. Final counts: 271 integration tests (270 original + that guard), gate-5x 5/5 at 286 per run.

Known residual, not fixed by this task: contract.rs holds single large tables that every new command edits in place, which conflicts as hard as tail appends. Needs data-driven rows, not a file split.
<!-- SECTION:NOTES:END -->
