---
id: TASK-31
title: Close exit-contract escapes in argument and output handling
status: In Progress
assignee: []
created_date: '2026-08-17 19:26'
updated_date: '2026-08-19 14:44'
labels:
  - bug
dependencies: []
type: bug
ordinal: 37000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Parent group for two confirmed defects found in a 2026-08-17 bug hunt, both of which let blotter terminate outside its published error contract. The contract says every failure is one structured envelope on stderr plus an exit code listed in ERROR_CONTRACT (src/error.rs:26), and every success is one envelope on stdout. Today an oversized --since value aborts the process with a raw Rust panic and exit 134, and any stdout write failure is discarded so the process claims success with no envelope at all. Both break the same promise an agent consumer relies on: the exit code and the stream contents always agree on what happened. Neither has test coverage. The repository gate was green before and after the hunt (173 tests, Clippy clean), so both are pre-existing.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every reachable failure path exits with a code listed in ERROR_CONTRACT; no path aborts or exits 0 after failing
- [ ] #2 Every non-zero exit is accompanied by a well-formed error envelope on stderr; stdout carries at most one success envelope
- [ ] #3 Each child ships a regression test that fails against the current binary
- [ ] #4 cargo test --all-features, clippy -D warnings, and cargo fmt --check all pass
<!-- AC:END -->
