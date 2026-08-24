---
id: TASK-58
title: 'hook install: detect competing hook registrations across settings layers'
status: To Do
assignee: []
created_date: '2026-08-24 16:38'
labels: []
dependencies: []
ordinal: 68000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Graduated from cut bl_0d725decd5af. During the publish re-gate, a stale user-level hook in global settings pointed at an old cargo-installed binary (0.13.1) that skipped note-lane redaction, while the project-level hook was freshly repaired. Two hook layers mean fixing one binary does not fix the capture path. hook install (or doctor) should detect and warn about competing registrations across settings layers. Detection/warning only — no behavior change to capture.
<!-- SECTION:DESCRIPTION:END -->
