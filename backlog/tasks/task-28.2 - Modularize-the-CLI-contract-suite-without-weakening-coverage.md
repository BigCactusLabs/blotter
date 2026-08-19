---
id: TASK-28.2
title: Modularize the CLI contract suite without weakening coverage
status: To Do
assignee: []
created_date: '2026-08-13 03:02'
labels:
  - testing
  - refactor
dependencies: []
modified_files:
  - tests/cli.rs
parent_task_id: TASK-28
type: chore
ordinal: 30000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Make tests/cli.rs easier and cheaper to work in while keeping one integration-test crate. The file is 7,001 lines and mixes startup, evidence, storage, triage, verify, digest, sweep, hook, schema, and migration contracts. Detailed pure-algorithm matrices can move closer to their implementations, but public envelope, exit-code, filesystem, determinism, and concurrency behavior must remain black-box.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Split tests/cli.rs into command or behavior modules under one integration-test binary so the refactor does not add one linked crate per module.
- [ ] #2 Move only pure algorithm matrices, such as detailed triage linkage or redaction cases, to unit tests.
- [ ] #3 Retain black-box sentinels for every public command, envelope shape, exit code, stdout/stderr rule, filesystem mutation, deterministic output, and concurrency invariant.
- [ ] #4 Record the integration test count before and after, and justify every removed or merged test by the independent behavior it still protects.
- [ ] #5 The full gate passes; store or concurrency-adjacent movement triggers five cargo test --all-features runs.
<!-- AC:END -->
