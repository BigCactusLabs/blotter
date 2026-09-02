---
id: TASK-80
title: 'list --format md: multi-warning ordering coverage'
status: Done
assignee: []
created_date: '2026-09-02 03:14'
updated_date: '2026-09-02 20:35'
labels: []
dependencies: []
ordinal: 89000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
TASK-74 deleted list_hides_auto_captures_and_composes_with_selectors, the only test asserting the order of two trailing > note: lines in list --format md. The single-warning case was rewritten (list_md_renders_warnings_as_trailing_note_lines). Restore the ordering case once the v2 tree has a second deterministic warning source (a torn tail plus a malformed line, or the unknown-kind warning). Low priority; test-only.
<!-- SECTION:DESCRIPTION:END -->
