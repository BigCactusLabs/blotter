---
id: TASK-44
title: >-
  store::absolute normalizes .. lexically, diverging from the OS across
  symlinked directories
status: To Do
assignee: []
created_date: '2026-08-19 14:43'
labels:
  - bug
  - store
dependencies: []
type: bug
ordinal: 52000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
absolute (src/store.rs:167-184) folds Component::ParentDir with normalized.pop(), a purely textual normalization. When a component of a user-supplied relative --file or BLOTTER_FILE path is a symlink to a directory elsewhere, .. must resolve against the link's TARGET, not its spelling, so blotter's resolved path silently diverges from what every other tool resolves. discover_from feeds this straight into ResolvedFile.path (src/store.rs:101-112) and it becomes the path that is locked, read, appended, backed up, and reported in meta.file: a log can be written where the user's own cat of the same argument cannot find it. Found in the 2026-08-19 audit.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A relative path whose parent component is a directory symlink resolves to the same file the OS resolves
- [ ] #2 The final component stays unresolved so the existing final-component symlink policy in resolve_symlinked_log is unchanged
- [ ] #3 Paths that do not yet exist still resolve, with the lexical fold kept only for the trailing nonexistent components
- [ ] #4 Regression test covers the symlinked-parent case; all four gates pass, suite runs five times
<!-- AC:END -->
