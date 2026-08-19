---
id: TASK-51
title: 'record_cwd applies only the exact-HOME rule, so cwd leaks the home directory'
status: Done
assignee: []
created_date: '2026-08-19 14:44'
updated_date: '2026-08-19 15:40'
labels:
  - bug
  - redaction
  - store
dependencies: []
type: bug
ordinal: 59000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
record_cwd (src/store.rs:153) does an exact HOME strip_prefix and, on failure, stores the absolute path verbatim. It never applies the r22 generic /Users/<user>/ and /home/<user>/ rule, nor the r23 dash-encoded form (/private/tmp/<session>/-Users-<user>-<repo>/...) that harness scratchpad paths embed. doctor::contains_home_path (src/commands/doctor.rs:541) implements all three against raw bytes, so the leak gate flags exactly the bytes the write path emitted, and --leaks conflicts with --fix, so the log stays permanently doctor-unhealthy with no repair. Verified both ways: a cwd under a generic Unix home stores absolute while the same literal passed as --evidence rewrites to ~/...; a dash-encoded scratchpad cwd stores the slug verbatim and doctor --leaks reports leaks, exit 1. Found in the 2026-08-19 audit.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 cwd goes through the same whole-string rewrite_home_paths scanner as evidence, covering all three home rules; a prefix-anchored matcher is not sufficient because the dash-encoded home appears mid-path
- [ ] #2 The repo-relative branch is unchanged and still wins when the cwd is inside the discovered repository
- [ ] #3 The accepted spelling is r23's matched-prefix rewrite, so a dash-encoded cwd stores as /private/tmp/<session-root>/~-<rest>/...; cwd and evidence never spell the same path two ways
- [ ] #4 compute_id still ignores cwd, so IDs and dedupe do not move, and stored history is never rewritten
- [ ] #5 schema's published cwd description and the test pinning it are updated; a design-doc amendment records the change
- [ ] #6 All four gates pass; store.rs change, so the suite runs five times
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Fixed in the 2026-08-19 batch PR: rewrite_home_paths and its helpers moved verbatim from src/commands/add.rs to a new src/redact.rs, and record_cwd runs the cwd through it after the repo-relative branch. Design doc r30 records the rule and the accepted matched-prefix spelling. The published schema string for cwd changed for both cut and dogear records, with its pinned assertion updated. Confirmed rather than assumed: the existing adjacent-home test (<temp>/Users/alicex) still passes — the exact-home branch fails path_prefix_boundary on the trailing x and the generic rule fails the preceding-character boundary, so that path stays absolute. Both new cwd tests also assert doctor --leaks reports healthy.
<!-- SECTION:NOTES:END -->
