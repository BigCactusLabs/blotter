---
id: TASK-24
title: Hook-drafted cuts with source provenance
status: Done
assignee: []
created_date: '2026-08-09 19:32'
updated_date: '2026-08-18 20:48'
labels:
  - product
dependencies: []
ordinal: 24000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
From 2026-08-09 frontier-search. Extend hook.rs with a fail-open hook that drafts a cut from harness-observable facts (tool failure, retry, non-zero exit), tagged source: hook vs source: self_report. Rationale: LLM self-reports lack privileged self-access (arxiv 2508.14802) — triangulate agent narration with objective telemetry. Record-format addition; coordinate with TASK-19 (next-major envelope breaks).
<!-- SECTION:DESCRIPTION:END -->
