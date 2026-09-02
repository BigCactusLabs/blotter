---
id: TASK-79
title: 'verify: measure recurrence from disposition_ts, not the winning resolution ts'
status: To Do
assignee: []
created_date: '2026-09-02 03:14'
updated_date: '2026-09-02 15:10'
labels:
  - v2
dependencies: []
ordinal: 86500
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
r48 §digest records this as a known follow-up: verify still measures recurrence from the winning resolution's ts (r16), so a note-only --amend moves the recurrence cutoff without re-deciding anything, while digest.accepted_cuts reads disposition_ts. Align verify on disposition_ts once Phase 5 lands so both commands read the same moment. Contract amendment first (small, corrective), then a one-command change with a test in tests/cli/verify.rs. Not part of 1.0.0 unless Phase 5 review says it must be.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-09-02 progress-review ruling: this is part of the 1.0.0 bar, not a post-1.0 follow-up. It lands inside the TASK-77 (Phase 5) PR as its own commit, under amendment r51: verify partitions later open cuts against the winning resolution's disposition_ts, not its ts, so a note-only --amend never moves the recurrence boundary; and VerifyResolution exposes disposition_ts beside ts, because after a note-only amend a recurrence can legitimately predate the displayed resolution.ts and would otherwise look impossible to a consumer. Test in tests/cli/verify.rs: an amend that changes only the note leaves recurrences byte-identical; an amend that changes the disposition moves them.
<!-- SECTION:NOTES:END -->
