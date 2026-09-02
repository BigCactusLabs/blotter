---
id: TASK-76
title: promotion record and blotter promote
status: To Do
assignee: []
created_date: '2026-09-01 21:57'
updated_date: '2026-09-02 15:10'
labels:
  - v2
dependencies:
  - TASK-81
ordinal: 85000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Phase 4 of docs/plans/2026-09-01-blotter-v2-plan.md. PR into v2 after TASK-75. New record kind promotion (bl_ id hashing ts/agent/sorted-unique sources/artifact type/ref; sources[] of existing cut IDs, 66 if missing, invalid_argument if a dogear; artifact{type,ref} type doc|skill|guard|test|tool|process; note). artifact.ref and note redacted per r34 before hash and append. blotter promote under the exclusive lock like add. list --kind promotion|all shows PromotionItem in the union, newest first then id; --kind enum split so sweep stays cut/dogear. resolve --disposition promoted --promotion <id> links. archive pins any cut a promotion names; doctor validates sources[]. New tests/cli/promote.rs declared in main.rs, archive-pinning case, redaction case. implementer-opus-med, pr-reviewer-high, gate-5x.
<!-- SECTION:DESCRIPTION:END -->
