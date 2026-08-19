---
id: TASK-45
title: Bound triage's residual prefilter cost at 300k records
status: To Do
assignee: []
created_date: '2026-08-19 14:43'
labels:
  - performance
dependencies: []
type: enhancement
ordinal: 53000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
TASK-29.3 replaced triage's quadratic candidate scan with exact-normalized-title and tagged-pool indexing, and the 2026-08-18 baselines doc records the 10k budget as met. The 2026-08-19 audit measured the residual Theta(n^2/64) bitset prefilter beyond that fixture: triage 0.14 s / 38 MiB at 10k, 6.37 s / 415 MiB at 100k, 54.85 s / 1,440 MiB at 300k. retrospect inherits it, so retrospect at 300k measured 80 s CPU / 1.9 GB. The 10k budget in the baselines doc is still met; this task is about whether a budget should exist above it and what bounds the prefilter if so. Benchmark-first, like the rest of TASK-29.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A reproducible budget exists for triage and retrospect above 10k, recorded in docs/research/2026-08-18-scale-baselines.md
- [ ] #2 Any algorithm change keeps triage and retrospect output byte-identical on every existing fixture
- [ ] #3 Peak RSS is an acceptance criterion alongside CPU
- [ ] #4 All four gates pass
<!-- AC:END -->
