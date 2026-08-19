---
id: TASK-10
title: 'Triage occurrence counting: same normalized title logged N times'
status: Done
assignee: []
created_date: '2026-08-05 21:47'
updated_date: '2026-08-07 01:47'
labels: []
dependencies:
  - TASK-20
ordinal: 10000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Frog shortlist item 9. Derive occurrence counts at fold time inside triage's existing shared-lock fold (append-only forbids upserts). TriageCluster already carries count/ids/suggested_action; add normalized-title recurrence as an explicit chronic-cut signal, superset of current Jaccard clustering. No cross-era pc_/bl_ dedup (r9). Size M.
<!-- SECTION:DESCRIPTION:END -->
