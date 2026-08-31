---
id: TASK-65
title: 'verify''s count reports matched anchors, not distinct recurring cuts'
status: Done
assignee: []
created_date: '2026-08-24 20:53'
updated_date: '2026-08-31 17:30'
labels: []
dependencies: []
ordinal: 75000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
In the same 66-record audit, `blotter verify` returned `count: 6`, but only 2 distinct open cuts were involved: bl_7cc82e0fc989 matched 3 separate resolved anchors and bl_9c4c0f664f64 matched 3.

`verify()` sets `count = recurrences.len()`, and `recurrence_groups` pushes one entry per anchor that matches at least one open cut. A single recurring problem is therefore reported once per old cut that resembles it. A reader takes "6 recurrences" to mean six problems came back, when two did.

This is independent of the false-positive matching in the sibling task: even after the 3 false pairs are removed, 3 remaining anchors still collapse to 2 distinct open cuts.

Likely fix: report the distinct recurring cut ids alongside the anchor count, or add a `distinct_cuts` field, so the headline number matches the number of live problems rather than the number of historical cuts that resemble them.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 verify output distinguishes the number of matched anchors from the number of distinct recurring cuts
- [x] #2 A test covers the many-anchors-to-one-recurring-cut shape
- [x] #3 README and schema are updated if the output envelope changes
<!-- AC:END -->
