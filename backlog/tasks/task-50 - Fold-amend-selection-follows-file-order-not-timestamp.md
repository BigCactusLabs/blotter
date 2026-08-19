---
id: TASK-50
title: 'Fold amend selection follows file order, not timestamp'
status: In Progress
assignee: []
created_date: '2026-08-19 14:44'
updated_date: '2026-08-19 15:12'
labels:
  - bug
  - store
dependencies: []
type: bug
ordinal: 58000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
fold_bytes collects amend resolves with a plain amends.insert(id, event) (src/store.rs:731-743), so the amend that wins is whichever appears LAST in the byte stream. The contract says the latest amend wins: design doc L302 (r13) 'the first non-amend resolve remains the base, the latest amend wins the materialized user-set fields' and r16 L328 'the latest amend timestamp when an amend wins'. Nothing in any amendment says file position. .gitattributes recommends merge=union on the log, and a union merge concatenates branches in branch order, not timestamp order, so file order and ts order routinely disagree exactly where it matters. Verified: with a base resolve, an amend at 2026-05-01, then an amend at 2026-03-01 appended last, list --status all materializes the March resolution; verify then reads resolution.ts as its anchor and the same records in a different byte order flip verify from count 0/exit 0 to count 1/exit 1. The existing test resolve_amend_latest_event_supersedes_prior_amend appends in increasing time order, where file order and ts order coincide, so it does not cover this. Found in the 2026-08-19 audit.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The amend that wins is the one with the greatest timestamp, with last-in-file winning an exact tie (>=, not >, so equal-timestamp behaviour under a frozen BLOTTER_NOW is preserved)
- [ ] #2 orphan_amends uses the same comparison so materialized_appended_resolution agrees with a full re-fold
- [ ] #3 Base resolve selection is unchanged: the first non-amend resolve remains the base
- [ ] #4 Tests cover decreasing-timestamp amends, equal-timestamp tie direction, and verify's exit code being invariant to swapping two amend lines
- [ ] #5 All four gates pass; store.rs change, so the suite runs five times
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Fixed in the 2026-08-19 batch PR: fold_bytes keeps amends in a HashMap<String, (jiff::Timestamp, LogEvent)> and replaces the incumbent on >=, so the latest-timestamp amend wins and two amends sharing a timestamp still resolve last-in-file (preserving frozen-clock behaviour). Deduping inside the amends map means orphan_amends can only ever see one amend per ID, so materialized_appended_resolution agrees with a full re-fold without a second comparison. Base resolve selection is untouched. Guard value verified by forcing the old behaviour: both the list test and the verify test fail.
<!-- SECTION:NOTES:END -->
