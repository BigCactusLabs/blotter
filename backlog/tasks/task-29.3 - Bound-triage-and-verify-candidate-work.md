---
id: TASK-29.3
title: Bound triage and verify candidate work
status: Done
assignee: []
created_date: '2026-08-13 03:03'
updated_date: '2026-08-18 20:49'
labels:
  - performance
  - analysis
dependencies:
  - TASK-29.1
modified_files:
  - src/commands/triage.rs
  - src/commands/verify.rs
parent_task_id: TASK-29
type: enhancement
ordinal: 34000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Meet the scale budgets established by TASK-29.1 without changing triage representative semantics or verify recurrence semantics. triage currently compares each unclaimed representative with later candidates; verify compares each resolved anchor with every open candidate. Exact-title and tag indexes can reduce candidate sets, but untagged similarity and one-open-cut-to-many-anchor behavior must remain correct.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Use TASK-29.1 fixtures and budgets as the before/after measurement; if the existing implementation already meets the approved budgets, document that result and close without production churn.
- [x] #2 When optimization is justified, index exact normalized titles and tagged candidate pools before considering more complex structures.
- [x] #3 Preserve earliest-unclaimed representative order, below-threshold member release, direct-link-only clustering, exact-title override, Jaccard threshold, and deterministic sorting.
- [x] #4 Verify still permits one open cut to recur against multiple resolved anchors and still excludes pre-resolution, dogear, dropped, blank-title, and hidden-auto records.
- [x] #5 Outputs and exit codes remain byte-identical on the existing contract corpus, and new scale fixtures demonstrate the budget.
<!-- AC:END -->
