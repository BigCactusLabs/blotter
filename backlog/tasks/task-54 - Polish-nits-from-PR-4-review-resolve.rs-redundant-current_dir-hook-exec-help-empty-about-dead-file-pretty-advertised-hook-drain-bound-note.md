---
id: TASK-54
title: >-
  Polish nits from PR #4 review: resolve.rs redundant current_dir, hook exec
  --help empty about + dead --file/--pretty advertised, hook drain bound note
status: To Do
assignee: []
created_date: '2026-08-19 23:45'
labels: []
dependencies: []
ordinal: 64000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
From the 2026-08-19 adversarial review of PR #4 (all nit severity, none regressions): (1) src/commands/resolve.rs calls std::env::current_dir() although store::discover already did — two syscalls and a second io_error attribution site; dogear.rs has the same pre-existing shape. (2) blotter hook --help lists exec with an empty description, and --file/--pretty are advertised on hook exec while silently ignored since r32 — one #[command(about=...)] plus flag notes. (3) hook exec drains only 1 MiB of stdin; a larger payload takes EPIPE despite the module doc claiming drain prevents closed-pipe writes — either raise the bound or correct the doc comment.
<!-- SECTION:DESCRIPTION:END -->
