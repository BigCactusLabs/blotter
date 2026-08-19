---
id: TASK-34
title: 'doctor --leaks: detect dash-encoded home-directory slugs'
status: Done
assignee: []
created_date: '2026-08-18 16:16'
updated_date: '2026-08-18 18:04'
labels: []
dependencies: []
ordinal: 42000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The leak scan matches /Users/<name> with path boundaries but misses the dash-encoded form -Users-<name>- that harness scratchpad/session slugs embed (e.g. /private/tmp/claude-501/-Users-<name>-<repo>/...). Demonstrated live during TASK-33: a dogfood cut recorded a scratchpad path whose slug carried the username straight past the gate. Also related prior art (from TASK-33 research): Windows crash reporters rewrite C:\Users\<name> before upload. Fix: extend the leak scanner (and write-time redaction) to recognize dash-encoded home slugs; add a regression test with a scratchpad-style path.
<!-- SECTION:DESCRIPTION:END -->
