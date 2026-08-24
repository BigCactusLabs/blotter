---
id: TASK-66
title: No way to reopen a cut that was resolved on a false claim
status: To Do
assignee: []
created_date: '2026-08-24 20:53'
labels: []
dependencies: []
ordinal: 76000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A cut closed on a claim that later proves false cannot be returned to the open queue.

`resolve --amend` appends a correcting resolve record, but the folded status stays `resolved` (verified on a scratch log: status resolved, amended true), and there is no `reopen` subcommand.

Real case from the walkmaxx log: pc_94e284f52022 was closed 2026-07-19 with "Resolved upstream: codex exec resume now supports the required resume workflow flags." Re-checked 2026-08-24: `codex exec resume --help` still has no `--cd` (only `-o, --output-last-message`), so half the cut is unfixed.

Consequence: verify can never flag it, triage never clusters it, and the only remedy is to file a duplicate cut under a new id, which breaks the recurrence link that verify depends on. An audit can detect a false closure but cannot correct it.

Likely fix: a `reopen <ID>` command that appends a record clearing the resolution in the fold, keeping the append-only log intact.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 blotter reopen <ID> returns a resolved cut to open status in the folded view
- [ ] #2 The append-only log retains the original cut, resolve and reopen records
- [ ] #3 A reopened cut is eligible for verify and triage again
- [ ] #4 Reopening an already-open cut fails with a clear error
<!-- AC:END -->
