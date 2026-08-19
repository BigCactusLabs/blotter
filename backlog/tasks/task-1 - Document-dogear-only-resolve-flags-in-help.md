---
id: TASK-1
title: Document dogear-only resolve flags in --help
status: Done
assignee: []
created_date: '2026-08-03 20:38'
updated_date: '2026-08-03 20:53'
labels:
  - cli-help
dependencies: []
ordinal: 1000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
resolve --help presents --dropped and --url as general flags but they are dogear-only (invalid_argument on cuts). Tracked as cut pc_9a013ee8e9c4 in .papercuts.jsonl — resolve it when this lands.
<!-- SECTION:DESCRIPTION:END -->
