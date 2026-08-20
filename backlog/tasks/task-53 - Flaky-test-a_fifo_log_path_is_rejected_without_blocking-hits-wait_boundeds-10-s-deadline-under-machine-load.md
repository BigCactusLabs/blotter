---
id: TASK-53
title: >-
  Flaky test: a_fifo_log_path_is_rejected_without_blocking hits wait_bounded's
  10 s deadline under machine load
status: Done
assignee: []
created_date: '2026-08-19 21:45'
updated_date: '2026-08-20 15:43'
labels:
  - bug
  - tests
dependencies: []
ordinal: 63000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found during TASK-46 gate runs: 2-of-5 gate-5x failures and 1-of-15 solo failures while benchmarks loaded the machine; 27/27 green on the base commit once quiet, 20/20 green on the branch warm. Tracks load, not code — add rejects a non-regular path at open, before any fold. Either the 10 s wait_bounded deadline is too tight for a loaded machine or the test needs a load-insensitive synchronization. Dogfood cut bl_7de50eb69de4.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Fixed on batch/2026-08-20-polish commit 100d725: wait_bounded deadline widened 10s -> 60s with comment explaining the generous bound still catches a true unbounded-block regression. Worker gates green incl. gate-5x 5/5 (274 tests each). Lands via batch PR.
<!-- SECTION:NOTES:END -->
