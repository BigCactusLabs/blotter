---
id: TASK-59
title: 'doctor --leaks residual: entropy pass composes with the home marker'
status: Done
assignee: []
created_date: '2026-08-24 17:11'
updated_date: '2026-08-31 15:03'
labels: []
dependencies: []
ordinal: 69000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Replacement for an ordinal that was reissued as TASK-58 while that number already named an archived hook task; the reissued record is now TASK-70 and the collision is logged as a cut. Residual of TASK-56, found by cross-model review: the entropy pass can replace the suffix after an emitted home marker, so blotter's own output '/Users/~<redacted>' (HOME=/Users/alice, evidence '/Users//Users/alice/<32-char high-entropy token>') carries component '~<redacted>', which the exact-tilde rule does not accept; doctor --leaks flags a line holding no home bytes. Reproduced 2026-08-24. Needs its own design pass on marker-composition shapes before widening the scanner rule.
<!-- SECTION:DESCRIPTION:END -->
