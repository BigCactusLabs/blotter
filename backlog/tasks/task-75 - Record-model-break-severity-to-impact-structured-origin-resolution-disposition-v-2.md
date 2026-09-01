---
id: TASK-75
title: >-
  Record model break: severity to impact, structured origin, resolution
  disposition, v: 2
status: To Do
assignee: []
created_date: '2026-09-01 21:57'
labels:
  - v2
  - breaking
  - store
dependencies: []
ordinal: 84000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Phase 3 of docs/plans/2026-09-01-blotter-v2-plan.md. impact low/material/blocking replaces severity everywhere incl. compute_id hash and the export OTLP severity map; origin{provider,trace_id,span_id,trace_flags} replaces source; resolve requires --disposition for cuts and rejects it for dogears; every record carries v: 2 and the scanner refuses a log whose records lack it with a named error (never silent skip, never partial fold). Delete the v1 ID hash, the pc_ namespace, IdNamespace, r12's forever promise, the source fold, tests/cli/legacy.rs. implementer-opus-med, pre-implementation critique leg, pr-reviewer-xhigh + codex review, scripts/dev/gate-5x.sh. Depends on TASK-72 and TASK-74.
<!-- SECTION:DESCRIPTION:END -->
