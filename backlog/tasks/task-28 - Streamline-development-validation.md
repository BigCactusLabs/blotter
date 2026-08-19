---
id: TASK-28
title: Streamline development validation
status: To Do
assignee: []
created_date: '2026-08-13 03:01'
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
- [ ] #1 Child tasks provide a documented fast iterative command and a maintainable test layout.
- [ ] #2 The final AGENTS.md gate still runs cargo test --all-features, Clippy, formatting, and the release build.
- [ ] #3 No test is removed unless its regression protection is preserved by independent behavior, artifact, schema, determinism, or error-contract coverage.
<!-- AC:END -->
