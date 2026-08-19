---
id: TASK-30
title: 'blotter archive --before <date>: retention via copy-and-swap'
status: Done
assignee: []
created_date: '2026-08-17 15:59'
updated_date: '2026-08-18 20:48'
labels: []
dependencies: []
ordinal: 36000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Issue #23 follow-up, direction approved 2026-08-17: add an archive command that trims records older than a cutoff by writing a repaired copy and atomically swapping (same mechanic doctor --fix owns), always leaving a timestamped sidecar with the removed records. Touches persistence: lands as its own reviewed PR, never a direct merge.
<!-- SECTION:DESCRIPTION:END -->
