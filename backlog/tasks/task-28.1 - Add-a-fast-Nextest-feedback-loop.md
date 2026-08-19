---
id: TASK-28.1
title: Add a fast Nextest feedback loop
status: Done
assignee: []
created_date: '2026-08-13 03:01'
updated_date: '2026-08-13 03:15'
labels:
  - testing
  - performance
dependencies: []
modified_files:
  - AGENTS.md
  - scripts/dev/test-fast.sh
parent_task_id: TASK-28
type: chore
ordinal: 29000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add an optional fast local test command without replacing the normative cargo test gate. The audit measured a 10.70s Nextest test phase versus 22.38s for the standard integration phase on the same tree. Shared Cargo contention made whole-command wall time noisy, so the task must compare runner test phases rather than contaminated build wait time.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A repository-owned command or documented invocation runs cargo nextest with all features for iterative work.
- [x] #2 AGENTS.md clearly distinguishes the fast iterative command from the required pre-commit cargo test --all-features gate.
- [x] #3 The command has a safe fallback when cargo-nextest is not installed, or the prerequisite and install path are explicit.
- [x] #4 Fresh measurements record standard-runner and Nextest test-phase times on the same unchanged build.
<!-- AC:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-08-13 03:15
---
2026-08-12 fresh same-tree measurement after cargo test --all-features --no-run, using rustc/cargo 1.97.1 and cargo-nextest 0.9.143: standard Cargo test phases were 0.10s unit + 19.78s integration = 19.88s for 173 tests; Nextest reported 15.889s for the same 173 tests. Build time was excluded from both phase totals.
---
<!-- COMMENTS:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added scripts/dev/test-fast.sh as the repository-owned iterative test command. It uses cargo nextest run --all-features when available and falls back to cargo test --all-features. AGENTS.md keeps the standard Cargo suite as the required pre-commit gate. Shell syntax and the complete repository gate pass.
<!-- SECTION:FINAL_SUMMARY:END -->
