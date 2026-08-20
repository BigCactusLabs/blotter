---
id: TASK-43
title: >-
  open_locked: sleep on the identity-mismatch retry and report a vanished log as
  66
status: Done
assignee: []
created_date: '2026-08-19 14:43'
updated_date: '2026-08-19 20:52'
labels:
  - bug
  - store
dependencies: []
type: bug
ordinal: 51000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Two defects in the open_locked retry loop (src/store.rs:272-316), found in the 2026-08-19 audit. (1) The WouldBlock branch sleeps LOCK_DELAY before the next attempt (309-311), but the path-identity-mismatch branch (298-303) falls straight through with no sleep. The design pins the budget as 50 x 100ms with exhaustion reporting lock_timeout/75 retryable:true; if the mismatch path is taken repeatedly (a peer looping doctor --fix or archive, or a checkout churning the log) the 50 attempts are consumed in microseconds and the caller gets exit 75 having waited ~0 ms, so the advised retry fires straight back into the same storm. (2) Once the loop has reopened and the log has vanished, exhaustion still reports lock_timeout/75 instead of not_found/66.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every non-returning iteration of the loop costs LOCK_DELAY, so the published 5-second bound actually holds
- [ ] #2 Loop exhaustion whose last failure was NotFound reports not_found/66, and only real contention reports 75
- [ ] #3 Regression test for each path
- [ ] #4 All four gates pass; store.rs change, so the suite runs five times
<!-- AC:END -->
