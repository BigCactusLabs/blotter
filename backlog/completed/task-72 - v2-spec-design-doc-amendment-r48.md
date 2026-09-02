---
id: TASK-72
title: 'v2 spec: design-doc amendment r48'
status: Done
assignee: []
created_date: '2026-09-01 21:57'
updated_date: '2026-09-02 02:00'
labels:
  - v2
  - spec
dependencies: []
ordinal: 81000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Phase 0 of docs/plans/2026-09-01-blotter-v2-plan.md. Amend the design doc with r48 restating every §3 rule as contract: admission floor; impact low/material/blocking; exact identity framing per kind (bl2 domain, field order, digest width, v and origin excluded); auto lane deleted and r32 fail-open superseded with an upgrade step; origin{provider,trace_id,span_id,trace_flags} width-validated and carried wherever source was; disposition fixed/promoted/accepted/invalid with amend inheritance and promotion-link clearing; promotion record with source pinning in archive, doctor source validation, r34 redaction of artifact.ref and note, PromotionItem list union and a split --kind enum; verify anchors fixed/promoted only; retrospect two patterns + suggested; digest accepted count by winning-resolution timestamp; v: 2 with a pre-fold in-lock probe refusing v1 logs (unsupported_log_version, exit 65, zero bytes written, doctor/archive refuse, sweep per-log skip); contract 6; crate 1.0.0. Full chain: design-judge-opus-med draft, orchestrator edit, Codex review.
<!-- SECTION:DESCRIPTION:END -->
