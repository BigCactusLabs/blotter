---
id: TASK-67
title: >-
  resolve --dry-run omits its dry-run warning when any requested ID is already
  resolved
status: Done
assignee: []
created_date: '2026-08-31 14:21'
updated_date: '2026-08-31 17:30'
labels: []
dependencies: []
ordinal: 77000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found by bug hunt (2026-08-31), reproduced on 0.15.0.

The warning chain in src/commands/resolve.rs:181-197 is exclusive, so either already-resolved branch consumes the dry-run branch:

  both-open dry run : changed=false warnings=['dry run; no resolve event appended']
  mixed dry run     : changed=false warnings=['already resolved: 1 ID (bl_...)']
  mixed real run    : changed=true  warnings=['already resolved: 1 ID (bl_...)']

A mixed dry run and a mixed real run emit identical warnings, and records[].status reads 'resolved' in both. Only data.changed separates them. The same applies when every requested ID is already resolved: the envelope says 'already resolved' and never says the run was a dry run.

No state is wrong and nothing is appended -- the warning set is incomplete, so a consumer reading warnings alone cannot tell a preview from an applied batch. Fix: push the dry-run warning independently of the already-resolved branches rather than as the final else-if arm.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A dry run always warns that no resolve event was appended, whatever the mix of open and already-resolved IDs
- [x] #2 The already-resolved warnings keep their current wording and counts
- [x] #3 Regression test in tests/cli/resolve.rs covers the mixed and all-resolved dry runs
<!-- AC:END -->
