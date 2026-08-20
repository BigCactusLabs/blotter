---
id: TASK-54
title: >-
  Polish nits from PR #4 review: resolve.rs redundant current_dir, hook exec
  --help empty about + dead --file/--pretty advertised, hook drain bound note
status: Done
assignee: []
created_date: '2026-08-19 23:45'
updated_date: '2026-08-20 15:58'
labels: []
dependencies: []
ordinal: 64000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
From the 2026-08-19 adversarial review of PR #4 (all nit severity, none regressions): (1) src/commands/resolve.rs calls std::env::current_dir() although store::discover already did — two syscalls and a second io_error attribution site; dogear.rs has the same pre-existing shape. (2) blotter hook --help lists exec with an empty description, and --file/--pretty are advertised on hook exec while silently ignored since r32 — one #[command(about=...)] plus flag notes. (3) hook exec drains only 1 MiB of stdin; a larger payload takes EPIPE despite the module doc claiming drain prevents closed-pipe writes — either raise the bound or correct the doc comment.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Fixed on batch/2026-08-20-polish commit a9a5c6d (merged): ResolvedFile gains pub cwd populated in discover_from; resolve.rs/dogear.rs drop the duplicate current_dir call; hook exec gains about/long_about documenting the retired no-op receiver and ignored --file/--pretty; hook.rs drain doc corrected to state the deliberate 1 MiB bound. Worker gates green incl. gate-5x 5/5; batch-branch gate re-run green. Lands via batch PR.
<!-- SECTION:NOTES:END -->
