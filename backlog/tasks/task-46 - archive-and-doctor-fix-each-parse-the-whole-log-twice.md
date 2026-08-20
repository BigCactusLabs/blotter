---
id: TASK-46
title: archive and doctor --fix each parse the whole log twice
status: Done
assignee: []
created_date: '2026-08-19 14:43'
updated_date: '2026-08-19 21:46'
labels:
  - performance
dependencies: []
type: enhancement
ordinal: 54000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
plan_archive calls store::fold_bytes (src/commands/archive.rs:150) then store::scan (161) on the same bytes, so every physical line is serde_json-decoded twice; the second pass exists only to recover line numbers and per-ID line groupings the first pass already walked past. Instrumented at 100k: fold 412 ms, second scan 282 ms (+68% on the fold, 41% of plan_archive's total). End-to-end archive --dry-run: 0.070 s / 24 MiB at 10k, 0.780 s / 196 MiB at 100k, 2.120 s / 561 MiB at 300k. During apply the original, kept_bytes and archived_bytes are all resident at once, on top of the fold, inside the exclusive lock. doctor --fix shows the same class of double work (+100% CPU). Found in the 2026-08-19 audit.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 plan_archive runs one parse pass; the fold carries the (line, id, ts) tuples archive needs
- [ ] #2 doctor derives post-fix findings without re-parsing the whole repaired log
- [ ] #3 Output, exit codes, backup and archive file bytes are unchanged on every existing fixture
- [ ] #4 Measured before/after CPU and peak RSS recorded in the baselines doc
- [ ] #5 All four gates pass; store/concurrency-adjacent, so the suite runs five times
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
plan_archive single-parse via fold_bytes_with_lines; doctor --fix derives post-fix findings from pre-fix findings. Byte-identity proven by 234-invocation differential against the pre-change binary. archive CPU 1.52x, doctor --fix CPU 2.00x / RSS -20% at 100k.
<!-- SECTION:NOTES:END -->
