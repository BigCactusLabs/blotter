---
id: TASK-7
title: 'Envelope contract 4 design: structured CTAs plus partial-outcome reporting'
status: To Do
assignee: []
created_date: '2026-08-05 21:47'
labels: []
dependencies: []
ordinal: 7000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Frog shortlist items 2+3, one contract bump. (a) CTAs: machine-readable next steps on success and error, modeled on incur meta.cta.commands[] with structured argv components (command/args/options), generalizing the existing suggested_fix prose and error details.candidates shape. (b) Partial outcomes: structured deferred/skipped array superseding meta.warnings; keep ok command-level (research: no ecosystem standard exists — AWS sibling arrays vs cargo-fresh nonzero-on-any-failure disagree on exit semantics; design deliberately). Driver: resolve.rs multi-ID; hook exec emits no envelope so frog's hook claim does not transfer. Research: scratchpad codex-sweep + arcjet blog (June 2026). Size M-L, needs contract amendment.
<!-- SECTION:DESCRIPTION:END -->
