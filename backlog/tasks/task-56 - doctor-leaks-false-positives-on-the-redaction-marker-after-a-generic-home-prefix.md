---
id: TASK-56
title: >-
  doctor --leaks false-positives on the redaction marker after a generic home
  prefix
status: In Progress
assignee: []
created_date: '2026-08-24 16:15'
updated_date: '2026-08-24 16:40'
labels: []
dependencies: []
ordinal: 66000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Pre-existing, surfaced by the TASK-55 review's differential harness: the redactor can emit a '~' immediately after a generic prefix that did not itself match (empty component, e.g. HOME=/Users/alice, evidence '/Users//Users/alice/x' stores '/Users/~/x'). doctor's generic_home_path_end then reads '~' as a non-empty username component and flags '/Users/~' or '-Users-~' as a leak. Span analysis showed zero surviving real home bytes in these shapes — it is a scanner false positive on blotter's own redaction marker, the r30 defect class in miniature. Likely fix: doctor's generic rule rejects a component that is exactly '~' (or starts with it), plus a black-box parity test.
<!-- SECTION:DESCRIPTION:END -->
