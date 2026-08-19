---
id: TASK-32
title: list --limit 0 warns no cuts matched while total is non-zero
status: Done
assignee: []
created_date: '2026-08-17 19:28'
updated_date: '2026-08-18 20:48'
labels:
  - bug
dependencies: []
modified_files:
  - src/commands/list.rs
  - tests/cli.rs
priority: low
type: bug
ordinal: 40000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The empty-result warning in list::run (src/commands/list.rs:57) keys on data.items.is_empty(), which is measured after truncation, instead of on total, which is measured after filtering. With --limit 0 the envelope reports total: 2 and simultaneously warns no cuts matched; try --status all or broader filters.

  $ blotter list --limit 0
  {"data":{"items":[],"count":0,"total":2,"truncated":true},
   "meta":{"warnings":["no cuts matched; try --status all or broader filters"]}}

The counts themselves are correct and match the design doc, which pins truncated as total > count (docs/plans/2026-07-09-papercuts-design.md:51). Only the warning is wrong, and it contradicts the data in its own envelope.

--limit 0 is the natural way for an agent to ask how many are there without paying for the items, so the misleading advice lands exactly where a caller is least able to sanity-check it. Fix is to gate the warning on total == 0.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The no records matched warning is emitted only when total is 0, for every --kind variant
- [ ] #2 blotter list --limit 0 against a non-empty log emits no empty-result warning and still reports total and truncated as it does today
- [ ] #3 A genuinely empty result (filters match nothing) still emits the kind-specific warning
- [ ] #4 Regression test covers both cases: limit 0 with matches, and a filter that matches nothing
<!-- AC:END -->
