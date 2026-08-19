---
id: TASK-4
title: Cut-ID tag-boundary fix (breaking release)
status: Done
assignee: []
created_date: '2026-08-03 20:39'
updated_date: '2026-08-05 12:12'
labels:
  - breaking
dependencies: []
ordinal: 4000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Dogear IDs use per-tag length-prefixed framing; the released cut-ID scheme has the same latent tag-boundary collision, deliberately deferred to a future breaking release (design doc r7 amendment, line ~218). Bundle with the next format-breaking change.
<!-- SECTION:DESCRIPTION:END -->
