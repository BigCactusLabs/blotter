---
id: TASK-62
title: >-
  End-boundary adjacency under-redaction: exact home followed by a non-boundary
  byte stores home bytes verbatim
status: To Do
assignee: []
created_date: '2026-08-24 18:15'
labels: []
dependencies: []
ordinal: 72000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found by cross-model review of TASK-59/60 (2026-08-24): path_prefix_boundary rejects a match whose next byte is not /, the separator, or a delimiter, so with HOME=/Users/alice the evidence -Users-/Users/alice-Users-alice keeps /Users/alice verbatim (after TASK-60 the tail redacts, storing -Users-/Users/alice~ — the slash home still leaks). Redactor and scanner agree, so the gate passes real home bytes. Opposite direction of TASK-59's class; needs a design pass on the end-boundary rule; changing the redactor side moves record IDs (r25).
<!-- SECTION:DESCRIPTION:END -->
