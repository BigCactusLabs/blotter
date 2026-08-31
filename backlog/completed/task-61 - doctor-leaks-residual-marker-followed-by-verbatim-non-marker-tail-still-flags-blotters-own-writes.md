---
id: TASK-61
title: >-
  doctor --leaks residual: marker followed by verbatim non-marker tail still
  flags blotter's own writes
status: To Do
assignee: []
created_date: '2026-08-24 18:15'
labels: []
dependencies: []
ordinal: 71000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Residual of TASK-59/r41: shapes where a home match leaves ~ directly ahead of surviving verbatim bytes that are neither ~ nor <redacted> — e.g. /Users//-Users-alice-foo stores as /Users/~-foo (component ~-foo), and assignment/URL secret spans produce ~-token=<redacted>. The r41 rule accepts only ~ followed by ~ or <redacted>, so these remain gate false positives on lines holding no home bytes (r30 class, gate at fault). Widening to any leading ~ reverses r39's real-directory-name decision; needs its own design pass.
<!-- SECTION:DESCRIPTION:END -->
