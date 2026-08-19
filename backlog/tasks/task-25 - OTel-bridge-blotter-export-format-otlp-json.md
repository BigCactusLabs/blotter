---
id: TASK-25
title: 'OTel bridge: blotter export --format otlp-json'
status: Done
assignee: []
created_date: '2026-08-09 19:32'
updated_date: '2026-08-18 20:48'
labels:
  - product
  - release
dependencies: []
ordinal: 25000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
From 2026-08-09 frontier-search. Batch export mapping cuts to a custom blotter.friction.reported event. Bridge, not backend: do NOT adopt gen_ai.* as internal schema — GenAI semconv is unstable, moved to dedicated repo v1.42.0 June 2026, no pinnable schema (john-hodge.com July 2026 status). Keep the mapping in one versioned module; own schema, map outward. Never ingest prompt bodies or tool output by default. Lowest priority of the four research tasks.
<!-- SECTION:DESCRIPTION:END -->
