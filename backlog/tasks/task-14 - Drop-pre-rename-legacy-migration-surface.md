---
id: TASK-14
title: Drop pre-rename legacy migration surface
status: Done
assignee: []
created_date: '2026-08-06 12:23'
updated_date: '2026-08-07 02:31'
labels:
  - breaking
dependencies: []
ordinal: 14000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Migration aids for the papercuts-to-blotter rename are permanent fixtures with no sunset. compute_id_legacy_v1 (lib.rs:305) exists solely so doctor can label pre-0.8.0 records legacy; IdNamespace::Pc threads through doctor.rs and resolve.rs; stale_env_warnings (store.rs:100) warns about PAPERCUTS_FILE/AGENT/NOW on every command; legacy_file_warning (store.rs:118) probes for .papercuts.jsonl on every discovery. Set a cutoff at the next major and remove them. Reading old logs must still work -- pc_ IDs stay valid opaque strings for resolve and list; what goes is the recompute/classify/warn machinery.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 compute_id_legacy_v1 and doctor legacy classification (the id_namespace match arms in doctor.rs) are removed; resolve RETAINS prefix parsing and namespace-equality matching so an explicit pc_/bl_ prefix still constrains matches and tests/cli.rs phase_2a_resolve_is_namespace_aware still passes
- [ ] #2 PAPERCUTS_* env warnings and .papercuts.jsonl discovery probing are removed from store.rs
- [ ] #3 Existing logs containing pc_ records still fold, list, and resolve by prefix without error
- [ ] #4 doctor no longer reports legacy_records; the field is dropped from DoctorData, from the schema envelope string (schema.rs:33), and the asserting tests (tests/cli.rs:2323, 2350, 3588, 3594) are updated
- [ ] #5 CHANGELOG documents the removal under the shared breaking heading with TASK-19/21, plus the migration command users should have already run
<!-- AC:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-08-06 13:33
---
Review 2026-08-06: original AC #1 (remove IdNamespace / reduce to opaque-ID check) contradicted AC #3 — match_id (resolve.rs:222-235) uses id_namespace for explicit-prefix constraining; removing it would make pc_abcd match bl_ records and turn unambiguous resolves into ambiguous_id. Rescoped to remove only recompute/classify/warn machinery.
---
<!-- COMMENTS:END -->
