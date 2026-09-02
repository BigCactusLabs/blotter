---
id: TASK-75
title: >-
  Record model break: severity to impact, structured origin, resolution
  disposition, v: 2
status: To Do
assignee: []
created_date: '2026-09-01 21:57'
updated_date: '2026-09-01 22:26'
labels:
  - v2
  - breaking
  - store
dependencies: []
ordinal: 84000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Phase 3 of docs/plans/2026-09-01-blotter-v2-plan.md. PR into v2 after TASK-72 and TASK-74. impact replaces severity everywhere incl. compute_id (bl2 framing per r48) and the export OTLP map; origin replaces source and is carried through list/triage/digest/verify; resolve requires --disposition for cuts, rejects it for dogears, amend inherits it; every record carries v: 2 and a pre-fold in-lock probe refuses a log lacking it on every open path with zero bytes written (no tear-heal), doctor reports non-fixable unsupported_version and --fix refuses, archive refuses, sweep per-log skip. Delete the v1 hash, pc_ namespace, IdNamespace (incl. archive.rs uses), source fold. Delete tests/cli/legacy.rs but port its three source provenance tests to new tests/cli/origin.rs. Update scripts/dev/generate-scale-fixtures.py, scripts/dev/bench-baseline.sh, tests/fixtures/export-otlp-json-golden.jsonl. Tests in contract.rs (refusal, byte-identical after), doctor.rs, store.rs (no tear-heal). implementer-opus-med, pre-implementation critique, pr-reviewer-xhigh + codex review, gate-5x.
<!-- SECTION:DESCRIPTION:END -->
