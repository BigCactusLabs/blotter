---
id: TASK-29.2
title: Reduce fold and resolve read amplification
status: Done
assignee: []
created_date: '2026-08-13 03:03'
updated_date: '2026-08-18 19:21'
labels:
  - performance
  - refactor
dependencies:
  - TASK-29.1
modified_files:
  - src/store.rs
  - src/commands/resolve.rs
  - src/lib.rs
parent_task_id: TASK-29
type: chore
ordinal: 33000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Remove redundant full-log work identified by the audit, subject to TASK-29.1 measurements. resolve currently reads and folds once to decide, appends, then reads and folds the full file again to build its response. fold_bytes also reparses timestamps during sort comparisons and retains owned records before cloning them into ListItem values. Keep the scanner rewrite out of scope unless the baseline proves its double Value-to-LogEvent handling material.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 After a successful resolve append, materialize the returned records from the already-folded state and appended resolution data without a second full read and fold.
- [x] #2 Parse each record timestamp once for fold ordering rather than inside sort comparisons; serialized timestamp strings and output order stay byte-identical.
- [ ] #3 Reduce duplicate owned record/ListItem state only when TASK-29.1 shows a meaningful memory or CPU gain, while append_unique still returns the first stored event on duplicates.
- [x] #4 No lock is released between read, decide, and append, and tear healing, rollback, first-wins resolution, amend behavior, and warnings remain unchanged.
- [x] #5 Fixed-clock output fixtures remain byte-identical and cargo test --all-features passes five consecutive runs.
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Resolve now materializes its response from the deciding fold plus the appended resolution; fold_bytes parses each timestamp once before sorting. AC #3 is closed as not-triggered: the TASK-29.1 baseline did not isolate owned-record/ListItem duplication as material, and the 500 ms budget is met without it, so the scanner rewrite and dedupe stay out of scope. append_unique duplicate behaviour is unchanged. Measurements and dispositions: docs/research/2026-08-18-scale-baselines.md.
<!-- SECTION:NOTES:END -->
