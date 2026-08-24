---
id: TASK-63
title: >-
  doctor --leaks raw-layer residual: JSON escaping and stderr truncation can
  break a marker component
status: To Do
assignee: []
created_date: '2026-08-24 18:41'
labels: []
dependencies: []
ordinal: 73000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found by cross-model review of the TASK-59/60 diff (2026-08-24), both shapes pre-existing and failing safe (a leak flag on a line holding no home bytes, never a missed leak). (1) The raw scanner reads JSONL-encoded bytes, and the encoder inserts a backslash ahead of an escaped delimiter, so a marker standing directly against a double quote stores as ~\" and the component reads ~\ — reproducible on main via the slash form (/Users//Users/alice" -> /Users/~\" flagged). (2) The stderr excerpt truncates to 4096 bytes AFTER redaction (redact_and_truncate, src/commands/add.rs:159), so the cut can split the emitted <redacted> marker and leave a component such as ~<re. r41's marker-composition rule accepts neither. Needs a design pass on whether the rule widens (escaped-delimiter awareness, truncation-aware acceptance) or the write side changes (truncate before redacting; avoid marker-splitting cuts).
<!-- SECTION:DESCRIPTION:END -->
