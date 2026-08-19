---
id: TASK-29
title: Scale-proof log reads and analysis
status: In Progress
assignee: []
created_date: '2026-08-13 03:02'
updated_date: '2026-08-19 14:44'
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
