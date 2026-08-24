---
id: TASK-60
title: >-
  Dash-form under-redaction: '-Users--Users-alice-y' stores real home bytes
  verbatim and passes doctor --leaks
status: Done
assignee: []
created_date: '2026-08-24 17:11'
updated_date: '2026-08-24 18:56'
labels: []
dependencies: []
ordinal: 70000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Pre-existing, surfaced by the 2026-08-24 batch review one byte from the TASK-56 change. With HOME=/Users/alice, evidence '-Users--Users-alice-y' stores verbatim and doctor --leaks exits 0: dash_start_boundary rejects a preceding '-' in both redact.rs and doctor.rs (src/commands/doctor.rs:598-604), so redactor and scanner agree while real home bytes sit in the log. Symmetric by construction but a live under-redaction hole in the dash form — the opposite direction of the false-positive class TASK-56 closed. Needs a design pass on the dash-form boundary rule; changing it moves record IDs (redaction precedes the identity hash, r25).
<!-- SECTION:DESCRIPTION:END -->
