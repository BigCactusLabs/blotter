---
id: TASK-29.1
title: Establish fold and analyzer scale baselines
status: Done
assignee: []
created_date: '2026-08-13 03:02'
updated_date: '2026-08-18 20:48'
labels:
  - performance
  - testing
dependencies: []
parent_task_id: TASK-29
type: spike
ordinal: 32000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Create reproducible release-mode measurements for the read and analysis paths before optimizing them. The audit found double JSON handling and owned-state amplification in fold_bytes, plus quadratic worst cases in triage and verify, but the current small dogfood log does not justify guessing at the payoff.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Generate deterministic 1k and 10k record fixtures covering mostly unrelated open cuts, repeated titles, tagged near-duplicates, resolved anchors, dogears, malformed lines, and duplicate events.
- [ ] #2 Measure list, triage, verify, digest, doctor, add duplicate detection, and resolve on unchanged fixtures with build time excluded.
- [ ] #3 Report CPU time, peak memory when available, fixture composition, toolchain, and exact commands; flag runner contention separately from program work.
- [ ] #4 Define explicit budgets for TASK-29 child work and state which production optimizations are justified.
- [ ] #5 The benchmark harness adds no production dependency and does not weaken deterministic contract tests.
<!-- AC:END -->
