---
id: TASK-12
title: Test discipline and hook init hygiene additions
status: Done
assignee: []
created_date: '2026-08-05 21:47'
updated_date: '2026-08-07 01:44'
labels: []
dependencies: []
ordinal: 12000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Frog shortlist items 11+12. (a) Never-touch-the-store test: --help/--version/schema with BLOTTER_FILE at a nonexistent/unreadable path. (b) Clap help-coverage gate: every arg gets help text (~8 of ~40 today). (c) hook.rs: confirm temp file creation is O_EXCL; surface created-vs-amended instead of one changed bool. Optional stretch: #![warn(missing_docs)] in lib.rs (adds M of doc writing). Size S without stretch.
<!-- SECTION:DESCRIPTION:END -->
