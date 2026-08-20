---
id: TASK-28
title: Streamline development validation
status: Done
assignee: []
created_date: '2026-08-13 03:01'
updated_date: '2026-08-20 15:34'
labels:
  - testing
  - performance
dependencies: []
type: chore
ordinal: 28000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Parent group for shortening local feedback while preserving the agent-facing CLI contract. The 2026-08-12 audit measured 162 black-box CLI tests in a 7,001-line integration file: the standard integration phase took 22.38s, while Nextest ran all 173 tests in 10.70s. The required cargo test gate remains authoritative.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Child tasks provide a documented fast iterative command and a maintainable test layout.
- [x] #2 The final AGENTS.md gate still runs cargo test --all-features, Clippy, formatting, and the release build.
- [x] #3 No test is removed unless its regression protection is preserved by independent behavior, artifact, schema, determinism, or error-contract coverage.
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Close-out verification 2026-08-20: scripts/dev/test-fast.sh present and documented in AGENTS.md (nextest fast loop, TASK-28.1); tests/cli/ split into 21 declared modules with every_test_module_file_is_declared_in_main guard in docs.rs (TASK-28.2); AGENTS.md gate still requires cargo test --all-features, clippy -D warnings, fmt --check, release build. Both subtasks Done. No tests removed.
<!-- SECTION:NOTES:END -->
