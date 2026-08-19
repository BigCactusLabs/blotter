---
id: TASK-8
title: list --since accepts a git ref
status: To Do
assignee: []
created_date: '2026-08-05 21:47'
labels: []
dependencies: []
ordinal: 8000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Frog shortlist item 4. Filter list output by records appended since a git ref's commit date. Contract pins --since to RFC3339/Nd/Nh with reject-don't-guess; ref syntax must be unambiguous (bare hex collides with ID prefixes — consider an explicit form like --since-ref or ref: prefix). Git via subprocess only, precedented by doctor; missing-git/unknown-ref map to existing error codes. Size M.
<!-- SECTION:DESCRIPTION:END -->
