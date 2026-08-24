---
id: TASK-58
title: >-
  doctor --leaks residual: entropy pass composes with the home marker
  (/Users/~<redacted>)
status: To Do
assignee: []
created_date: '2026-08-24 16:55'
labels: []
dependencies: []
ordinal: 68000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Residual of TASK-56, found by cross-model review. The entropy pass can replace the suffix after an emitted home marker, so blotter's own output '/Users/~<redacted>' (HOME=/Users/alice, evidence '/Users//Users/alice/<32-char high-entropy token>') carries component '~<redacted>', which the exact-tilde rule does not accept; doctor --leaks flags a line holding no home bytes. Reproduced 2026-08-24 on the TASK-56 branch. Same r30 defect class; needs its own design-pass on marker-composition shapes before widening the scanner rule.
<!-- SECTION:DESCRIPTION:END -->
