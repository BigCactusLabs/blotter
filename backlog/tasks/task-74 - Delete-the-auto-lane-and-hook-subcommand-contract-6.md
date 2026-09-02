---
id: TASK-74
title: Delete the auto lane and hook subcommand (contract 6)
status: In Progress
assignee: []
created_date: '2026-09-01 21:57'
updated_date: '2026-09-02 02:22'
labels:
  - v2
  - breaking
dependencies:
  - TASK-72
  - TASK-73
ordinal: 83000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Phase 2 of docs/plans/2026-09-01-blotter-v2-plan.md. First PR into the v2 integration branch, after TASK-73 merges. Remove is_auto_capture and the partition helper, --include-auto from list/triage/digest/verify/sweep/export and schema entries, the hidden-records warning, retrospect's include-by-default case, src/commands/hook.rs and the hook subcommand incl. the src/main.rs is_hook_exec fast path and the cli.rs/mod.rs types and dispatch, tests/cli/hook.rs and auto_capture.rs plus main.rs declarations, the contract.rs no-op hook assertion, the resolve.rs include-auto guidance and its resolve.rs test, the AGENTS.md invariant bullet, and the README read-command paragraph and Hooks section. CONTRACT 5→6. CHANGELOG entry carries the mandatory hook-removal upgrade step. Codex terra @ max; one pr-reviewer-high pass.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
r49 (2026-09-01): replace the deleted no-op hook assertion with a test pinning hook exec claude-code → exit 2, empty stdout, clap unrecognized-subcommand stderr.
<!-- SECTION:NOTES:END -->
