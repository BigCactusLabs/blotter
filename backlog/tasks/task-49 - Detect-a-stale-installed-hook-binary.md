---
id: TASK-49
title: Detect a stale installed hook binary
status: To Do
assignee: []
created_date: '2026-08-19 14:43'
labels:
  - hook
  - tooling
dependencies: []
type: enhancement
ordinal: 57000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Claude Code hook runs whatever blotter binary the settings command names, typically ~/.cargo/bin/blotter. On 2026-08-19 that install predated the r29 chain-shape gate by four hours, so chained failed commands were still being auto-captured while the repo build correctly skipped them. Both binaries reported blotter 0.15.0, because r29 shipped unreleased, so nothing surfaced the drift; it was only found by asking why an auto-capture existed that the shipped gate forbids. blotter schema already publishes the full gate set as structured data, so the installed binary can be interrogated for its gates rather than its version. Filed from a dogfood cut.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 There is a way for an agent or operator to see that the installed hook binary's published gate set differs from the repo build's
- [ ] #2 The check does not require a version bump to be meaningful, since unreleased gates share a version
- [ ] #3 The hook lane stays fail-open and the check never disrupts a host session
- [ ] #4 All four gates pass
<!-- AC:END -->
