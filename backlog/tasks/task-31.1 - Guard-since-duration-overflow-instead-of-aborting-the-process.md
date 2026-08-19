---
id: TASK-31.1
title: Guard --since duration overflow instead of aborting the process
status: Done
assignee: []
created_date: '2026-08-17 19:27'
updated_date: '2026-08-18 20:48'
labels:
  - bug
dependencies: []
modified_files:
  - src/lib.rs
  - tests/cli.rs
parent_task_id: TASK-31
priority: high
type: bug
ordinal: 38000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
parse_since (src/lib.rs:270) guards the Nd-to-hours multiply with checked_mul(24), then hands the result to jiff SignedDuration::from_hours (src/lib.rs:294), which panics when hours times 3600 overflows i64 (jiff-0.2.35/src/signed_duration.rs:753). The guard covers the wrong step. With panic = abort in the release profile the process dies on SIGABRT: a raw Rust panic message on stderr where an error envelope belongs, and exit 134, which is not in ERROR_CONTRACT.

Confirmed against the release binary. Exact boundary: --since 2562047788015215h exits 2 cleanly through the existing checked_sub path; --since 2562047788015216h exits 134. Reachable from every --since consumer: list, digest, and sweep. Equivalent day values overflow at 106751991167301d.

Not a purely theoretical input. The realistic trigger is a caller computing a window in the wrong unit, for example --since $(date +%s%N)h, which passes a nanosecond epoch as hours and aborts.

Fix is to use SignedDuration::try_from_hours and route None into the invalid_argument error parse_since already raises for oversized day values, so both overflow steps produce the same exit 2 and the same suggested_fix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 parse_since returns invalid_argument (exit 2) rather than panicking for any all-digit Nd or Nh value, including i64::MAX
- [ ] #2 The overflow error reuses the existing --since value is too large message and suggested_fix, so callers see one behaviour for both overflow steps
- [ ] #3 Regression tests assert exit 2 and a well-formed error envelope for list, digest, and sweep at --since 2562047788015216h and --since 106751991167301d
- [ ] #4 A test pins the boundary: 2562047788015215h and 2562047788015216h both exit 2, neither aborts
- [ ] #5 Valid relative windows (7d, 12h, 0d) and absolute RFC3339 values keep their current results byte-for-byte
<!-- AC:END -->
