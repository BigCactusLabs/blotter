---
id: TASK-29.4
title: Consolidate folded-log loading for read commands
status: Done
assignee: []
created_date: '2026-08-13 03:03'
updated_date: '2026-08-13 03:15'
labels:
  - refactor
  - maintenance
dependencies: []
modified_files:
  - src/store.rs
  - src/commands/list.rs
  - src/commands/triage.rs
  - src/commands/digest.rs
  - src/commands/verify.rs
parent_task_id: TASK-29
type: chore
ordinal: 35000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
list, triage, digest, and verify repeat the same discovery result handling, missing-default-file fallback, shared-lock read, fold, and fold-warning merge. Add one narrow store or command-layer helper for this common path. Keep command-specific filtering, hidden-auto counts, empty-result guidance, formats, and exit codes in their command modules.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 One helper owns shared-lock read, virtual-empty behavior for discovered missing files, fold invocation, and fold-warning propagation for list, triage, digest, and verify.
- [x] #2 Explicit missing files still return not_found/66, while discovered missing files remain successful virtual-empty reads with the existing warning text.
- [x] #3 Command-specific auto-capture eligibility, filters, markdown output, meta.file, and exit behavior remain unchanged.
- [x] #4 Do not generalize sweep or doctor into this helper: sweep has per-repository skip semantics and doctor has different missing-file and diagnostic behavior.
- [x] #5 The diff is a net simplification and all fixed-clock outputs remain byte-identical.
<!-- AC:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-08-13 03:15
---
The new store::load_folded helper is limited to list, triage, digest, and verify. It merges discovery and fold warnings in the prior order, uses the existing read_or_empty lock and explicit/discovered missing-file rules, and returns only folded items plus warnings. Sweep and doctor remain on their separate paths. The implementation diff for this task is +46/-60 Rust lines, a net reduction of 14 lines.
---
<!-- COMMENTS:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Consolidated the repeated shared-lock read, virtual-empty fallback, fold, and warning propagation into store::load_folded. Command-specific validation, auto filtering, Markdown, meta.file, and exit behavior stay local. Five consecutive cargo test --all-features runs and the complete build, Clippy, and format gate pass.
<!-- SECTION:FINAL_SUMMARY:END -->
