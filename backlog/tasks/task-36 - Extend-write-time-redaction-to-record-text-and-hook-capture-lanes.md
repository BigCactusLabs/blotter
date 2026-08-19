---
id: TASK-36
title: Extend write-time redaction to record text and hook capture lanes
status: Done
assignee: []
created_date: '2026-08-18 17:58'
updated_date: '2026-08-18 18:45'
labels:
  - enhancement
dependencies: []
ordinal: 44000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Two lanes still store content verbatim, and both produced live leaks during the TASK-34/35 batch (PR #31): (1) record text is never redacted at write time — a cut or hook auto-capture whose text quotes a path or identifier stores it as-is; only doctor --leaks catches it after the fact. (2) The hook failure-note lane embeds a failed command's full stdout, which re-ingested pre-scrub prose past redaction (logged as a major cut, 2026-08-18). Proposal: run the same home-path rewrite (r23 rules) over record text at write time for both add and hook lanes, and either redact or bound hook failure-note stdout. Design question to settle first: text is part of the content-derived ID, so write-time text rewriting changes IDs relative to what the caller supplied — that is fine at creation time (the ID is computed after redaction) but must be specified in an amendment. Related r23 accepted residual: foreign dashed usernames redact only their first component; revisit only if a real case shows up.
<!-- SECTION:DESCRIPTION:END -->
