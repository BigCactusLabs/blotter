---
id: TASK-57
title: >-
  Reproducible scale fixtures: promote 100k/300k into generate-scale-fixtures.py
  and add fixture/command selection to bench-baseline.sh
status: Done
assignee: []
created_date: '2026-08-24 16:38'
updated_date: '2026-08-24 17:34'
labels: []
dependencies: []
ordinal: 67000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Graduated from cut bl_c1881224c389 (triage cluster, 4 occurrences). scripts/dev/generate-scale-fixtures.py hardcodes FIXTURES to 1k and 10k, and bench-baseline.sh hardcodes the same two labels, so the 100k/300k numbers cited in the repo are not reproducible by anyone else. Promote the larger sizes into the FIXTURES tuple and teach bench-baseline.sh fixture and command selection flags.
<!-- SECTION:DESCRIPTION:END -->
