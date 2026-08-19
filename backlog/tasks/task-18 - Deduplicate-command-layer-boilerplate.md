---
id: TASK-18
title: Deduplicate command-layer boilerplate
status: Done
assignee: []
created_date: '2026-08-06 12:24'
updated_date: '2026-08-07 03:30'
labels:
  - refactor
dependencies:
  - TASK-16
ordinal: 18000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Three copies of the same not-found fallback: list.rs:33-43, triage.rs:93-103, and doctor.rs:38-55 each hand-write the if error is not_found and the file was not explicit, warn and use empty state branch, with three near-identical warning strings. Collapse to one store helper. Separately, dogear.rs (110 lines) is a near-verbatim transcription of add.rs's mutation flow: agent validation, text length validation, dry-run branch, fold-then-dedup-then-append under the exclusive lock, and warning assembly. The shared shape is an append_unique(record, id) -> (changed, record) on the store side plus one validate-agent helper. Lands cleanest after the parser unification, since the dedup check currently goes through ListItem::cut_record/dogear_record. Expect roughly 120 fewer lines and removes the drift risk of parallel edits.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 One store-level helper serves the missing-file empty-state path; list, triage, and doctor call it
- [ ] #2 The add and dogear mutation flows share one append-unique path; per-command code is limited to record construction and command-specific validation
- [ ] #3 Agent validation exists once, not in add.rs, dogear.rs, and resolve.rs separately
- [ ] #4 Warning text for each command is unchanged, or changes are deliberate and covered by tests
- [ ] #5 The empty-state helper is generic over the empty value, parameterized on the warning and suggested_fix strings (doctor uses different text than list/triage: healthy-empty-state wording and omit-file suggestion), and reports file existence -- doctor.rs:58-60 branches on file_existed
- [ ] #6 resolve agent-validation behavior is unchanged: it lacks the post-resolve_agent whitespace check that add.rs:41 and dogear.rs:48 share; a unified helper must not silently add it to resolve unless that change is deliberate and tested
<!-- AC:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-08-06 13:34
---
Review 2026-08-06: list and triage not-found branches are byte-identical; doctor differs in warning text, suggested_fix, and its file_existed return. The ListItem cut_record/dogear_record round-trip dependency on TASK-16 is confirmed (add.rs:87, dogear.rs:84).
---
<!-- COMMENTS:END -->
