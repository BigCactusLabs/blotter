---
id: TASK-42
title: Tear-heal disagrees with scan on a log holding exactly one newline
status: Done
assignee: []
created_date: '2026-08-19 14:43'
updated_date: '2026-08-19 20:52'
labels:
  - bug
  - store
dependencies: []
type: bug
ordinal: 50000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
scan deliberately treats a file of exactly "\n" as zero physical lines (src/store.rs:600-612), and r26 makes that normative. The tear-heal predicate in append_bytes_with is !prior.is_empty() && !prior.ends_with(b"\n") (src/store.rs:571-573), which reads the same bytes as a properly terminated non-empty file and appends with no separator. The result is "\n{record}\n", whose leading empty segment is no longer the sole segment, so scan emits it as a real physical line and it parses as malformed. The log goes from healthy to permanently unhealthy on a plain add, repairable only by doctor --fix. Found in the 2026-08-19 audit.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A log whose entire content is one newline accepts an add and stays healthy
- [ ] #2 The append path and scan agree on what counts as an empty log
- [ ] #3 Regression test asserts doctor reports healthy after add on a newline-only log
- [ ] #4 All four gates pass; store.rs change, so the suite runs five times
<!-- AC:END -->
