---
id: TASK-47
title: 'hook exec: skip the full fold in the exclusive-lock dedupe check'
status: To Do
assignee: []
created_date: '2026-08-19 14:43'
labels:
  - performance
  - hook
dependencies: []
type: enhancement
ordinal: 55000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
hook exec claude-code (src/commands/hook.rs:307) folds the whole log to answer one question: is there an open cut whose text equals this command. Measured 0.43 s CPU / 167 MiB at 100k, all of it inside the EXCLUSIVE lock, on every failed Bash tool call; with LOCK_DELAY at 100 ms a competing writer burns 4-5 of its 50 retry attempts per hook fire. The records-only fold added for append_unique is most of the answer, but the hook also needs a resolve set, and that set must replicate fold_bytes' amend rule exactly: an amend resolve only enters resolves when a base resolve for that id already exists, and an amend with no base lands in orphan_amends leaving the record OPEN. A naive collect-every-Resolve-id set would mark such a record resolved and flip the hook from skipping to appending a duplicate auto-capture. Deliberately deferred from the 2026-08-19 batch for that reason.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The hook's dedupe predicate produces the same verdict as the full fold on every existing fixture, including the orphan-amend case
- [ ] #2 A regression test covers a cut whose only resolve is an orphan amend
- [ ] #3 Measured before/after CPU and peak RSS at 100k
- [ ] #4 Lane stays fail-open: stdout empty, exit 0
- [ ] #5 All four gates pass; store/concurrency-adjacent, so the suite runs five times
<!-- AC:END -->
