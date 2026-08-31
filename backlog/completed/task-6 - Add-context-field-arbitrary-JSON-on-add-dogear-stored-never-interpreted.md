---
id: TASK-6
title: 'Add context field: arbitrary JSON on add/dogear, stored never interpreted'
status: To Do
assignee: []
created_date: '2026-08-05 21:47'
labels: []
dependencies: []
ordinal: 6000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Frog shortlist item 1 (prior-art shortlist, private notes — see TASK-5). Optional --context <json> on add/dogear, stored as opaque serde_json::Value like evidence, skip_serializing_if for legacy byte-compat. MUST be excluded from ID inputs (design doc: evidence exclusion precedent, r10 identity). Landing: lib.rs records, cli.rs args, schema.rs, doctor accepts as healthy. Size S.
<!-- SECTION:DESCRIPTION:END -->
