---
id: TASK-41
title: 'doctor --fix: roll back sidecars on abort and diagnose inside the lock'
status: Done
assignee: []
created_date: '2026-08-19 14:42'
updated_date: '2026-08-19 20:52'
labels:
  - bug
  - doctor
dependencies: []
type: bug
ordinal: 49000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Two defects in apply_fixes (src/commands/doctor.rs:213-233), found in the 2026-08-19 audit. (1) The backup, quarantine append, and swap each use ? with no cleanup, so a failure after the backup is written leaves a <log>.bak-<ts> that claims to back up a repair which never happened; r15 makes a pre-existing backup path an io_error with no log change, so the retry then fails on the leftover instead of the real cause. archive::apply_archive already solves this with remove_created_outputs (src/commands/archive.rs:125-140) and doctor has no equivalent. (2) r15 states apply mode re-reads and re-inspects inside the critical section. It does not: replace_log renames a new inode over the path, so the exclusive flock now covers the old unlinked inode, and doctor.rs:231 fs::read(path) reads the new inode unlocked. A peer that grabs the new file's lock in that window can be mid write_all, so the unlocked read can observe a partial line and report a spurious finding.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 apply_fixes removes every sidecar it created when any later step fails, mirroring archive::remove_created_outputs
- [ ] #2 The pre-existing-backup case carries its own suggested_fix naming the leftover file
- [ ] #3 The post-fix diagnosis uses the in-memory repaired bytes instead of re-reading the path, so it stays inside the lock's logical scope
- [ ] #4 Regression tests cover a failure at the quarantine step and at the swap step
- [ ] #5 All four gates pass; store/concurrency-adjacent, so the suite runs five times
<!-- AC:END -->
