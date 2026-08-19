---
id: TASK-16
title: 'Unify JSONL parsing: one scanner, one tagged event model'
status: Done
assignee: []
created_date: '2026-08-06 12:24'
updated_date: '2026-08-07 02:59'
labels:
  - refactor
dependencies: []
ordinal: 16000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
store::fold_bytes (store.rs:312, ~150 lines) and doctor::inspect (doctor.rs:87, ~215 lines) are two independent implementations of the same walk: split lines, dispatch on kind, parse into one of three record types, count problems. They already share tail_is_record because they must agree on torn tails -- that shared helper is the tell that the rest should be shared too. Replace both with one line scanner emitting typed events (Parsed/Malformed/Unknown/Torn) with fold and doctor as two consumers: list consumes folded state, doctor presents diagnostics, mutations use the same parser under the exclusive lock. Since a format break is already planned, also collapse CutRecord/DogearRecord/ResolveRecord into a single serde tag=kind enum -- that deletes both hand-written value.get(kind) dispatches and kills ListItem::cut_record/dogear_record (lib.rs:180-190), which currently downcast by round-tripping through serde_json::Value. Do not touch the storage mechanics: bounded try_lock, read-fold-decide-append under one exclusive lock, tear-healing, and rollback all stay as-is. Expect 180-400 fewer lines.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 One parser owns JSON decoding, torn-tail handling, unknown-kind policy, folding, and diagnostics; doctor::inspect no longer parses independently
- [ ] #2 Records are a single serde-tagged enum; ListItem::cut_record and ListItem::dogear_record are deleted along with their Value round-trips
- [ ] #3 fold and doctor cannot disagree on torn tails by construction, not by a shared helper
- [ ] #4 Existing fold_matrix cases and the doctor finding set pass unchanged; output for a fixed input log is byte-identical except where a break is deliberate and documented
- [ ] #5 cargo test --all-features run 5x green (store.rs is in scope)
- [ ] #6 Parser-unification constraints hold: unknown-kind lines still fold as warnings not errors (e.g. a #[serde(other)] variant); the malformed-vs-unknown distinction is preserved (bad fields under a known kind = malformed, unrecognized kind = unknown); the post-deserialize ts validity re-parse gate (store.rs:348/363/378) is retained since ts is a plain String; and any change to read-side tag sorting (fold sorts tags on read at store.rs:353/368, doctor does not) is deliberate and tested
<!-- AC:END -->
