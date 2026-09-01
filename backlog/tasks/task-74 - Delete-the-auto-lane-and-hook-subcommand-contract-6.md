---
id: TASK-74
title: Delete the auto lane and hook subcommand (contract 6)
status: To Do
assignee: []
created_date: '2026-09-01 21:57'
labels:
  - v2
  - breaking
dependencies: []
ordinal: 83000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Phase 2 of docs/plans/2026-09-01-blotter-v2-plan.md. Remove is_auto_capture and every --include-auto flag, src/commands/hook.rs, tests/cli/hook.rs and tests/cli/auto_capture.rs plus their main.rs declarations, the 'auto' tag exclusion rules, and the r32 fail-open receiver. Bump output::CONTRACT to 6. Codex terra @ max; one PR.
<!-- SECTION:DESCRIPTION:END -->
