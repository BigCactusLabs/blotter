---
id: TASK-9
title: 'Authoring-time dedup: warn-with-pointer on normalized-title match'
status: To Do
assignee: []
created_date: '2026-08-05 21:47'
labels: []
dependencies: []
ordinal: 9000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Frog shortlist item 5. Phase 1 (contract-compatible): inside add's existing lock, compare normalized text (reuse triage normalized_tokens) against folded open cuts; on match emit warning with existing ID — extends the current exact-ID duplicate_cut warning. Phase 2 (needs contract amendment: new error code + --force flag; research says refuse-with-force is novel — GitHub agentic-workflows duplicate_dropped with matched-title pointer is the closest model): refuse-with-pointer, record when bypass used. Contract non-goal 'no dedup/clustering of cuts' must be amended for phase 2. Size L total, phase 1 M.
<!-- SECTION:DESCRIPTION:END -->
