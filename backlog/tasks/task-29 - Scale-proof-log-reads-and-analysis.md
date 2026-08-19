---
id: TASK-29
title: Scale-proof log reads and analysis
status: Done
assignee: []
created_date: '2026-08-13 03:02'
updated_date: '2026-08-19 15:40'
labels:
  - performance
  - refactor
dependencies: []
type: enhancement
ordinal: 31000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Parent group for reducing work that grows with the append-only log while preserving byte-deterministic output and locking semantics. The current 171-line, roughly 74 KiB dogfood log is fast in CPU terms, so this tranche is benchmark-first and targets structural amplification rather than a broad rewrite.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Child tasks establish reproducible scale budgets before changing analyzer algorithms.
- [ ] #2 The read-fold-decide-append exclusive-lock invariant, first-wins fold, tear healing, rollback, exit codes, and output order remain unchanged.
- [ ] #3 Every store.rs or concurrency-adjacent change passes the full suite five times in addition to release, Clippy, and formatting gates.
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Closed by the 2026-08-19 batch PR. AC1 was met by TASK-29.1's baselines doc. The remaining structural amplification found by a benchmark-first audit above the 10k fixture: verify's recurrence scan was O(anchors x open cuts) — 29 s CPU at 300k — and every append folded the whole log into a sorted list view it discarded inside the exclusive lock. Both are fixed here. verify reuses triage's candidate index and bit-parallel prefilter with linked still the final predicate; release output is byte-identical on the 10k/100k/300k fixtures and CPU falls 6.88x at 300k, at 2.56x peak memory (parity with what triage already needs on the same log). append_unique uses a records-only fold: -15% CPU and -38% peak RSS at 100k, and less lock hold time. AC2 holds: the exclusive-lock critical section, first-wins base resolve, tear healing, rollback, exit codes and output order are unchanged — the amend-ordering change in this same PR is TASK-50, a correction to match r13/r16, not a TASK-29 change. AC3 holds: gate-5x.sh ran 5/5 green. Residual amplification is filed as TASK-45 (triage's own prefilter at 300k), TASK-46 (archive and doctor --fix double-parsing) and TASK-47 (the hook's full fold under the lock).
<!-- SECTION:NOTES:END -->
