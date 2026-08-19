---
id: TASK-22
title: 'blotter verify: recurrence tracking for resolved cuts'
status: Done
assignee: []
created_date: '2026-08-09 19:32'
updated_date: '2026-08-09 20:14'
labels:
  - research
  - product
dependencies: []
ordinal: 22000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The differentiator from the 2026-08-09 frontier-search: close the loop from fix to proof. A resolved cut gets an optional recheck (reproduction hint + recurrence window). If the same normalized title reappears after resolution, triage flags it as a failed intervention, not a new cut. Manual prototype is Pamela Fox's revert-and-rerun workflow (HN 47044313). Shares recurrence machinery with retrospect — build this first. Design pass required before implementation (touches fold semantics in store.rs).
<!-- SECTION:DESCRIPTION:END -->
