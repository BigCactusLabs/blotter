---
id: TASK-79
title: 'verify: measure recurrence from disposition_ts, not the winning resolution ts'
status: To Do
assignee: []
created_date: '2026-09-02 03:14'
labels: []
dependencies: []
ordinal: 88000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
r48 §digest records this as a known follow-up: verify still measures recurrence from the winning resolution's ts (r16), so a note-only --amend moves the recurrence cutoff without re-deciding anything, while digest.accepted_cuts reads disposition_ts. Align verify on disposition_ts once Phase 5 lands so both commands read the same moment. Contract amendment first (small, corrective), then a one-command change with a test in tests/cli/verify.rs. Not part of 1.0.0 unless Phase 5 review says it must be.
<!-- SECTION:DESCRIPTION:END -->
