---
id: TASK-55
title: Redactor/doctor divergence on dash-encoded home after a redacted home path
status: In Progress
assignee: []
created_date: '2026-08-24 15:23'
updated_date: '2026-08-24 16:00'
labels: []
dependencies: []
ordinal: 65000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Pre-existing (PR 8 review, P4): with HOME=/Users/alice, input '/Users/alice/x/-Users-bob-y' redacts to '~/x/-Users-bob-y' because token_end skips a tail with no delimiter, while contains_home_path flags it (generic_home_path_end accepts a preceding '/' for the dash form). Blotter's own write can trip its own --leaks gate — the defect class r30 closed. Not introduced by PR 8; slightly harder to reach after it.
<!-- SECTION:DESCRIPTION:END -->
