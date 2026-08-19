---
id: TASK-52.1
title: >-
  Match the destructive-command guard on command position, not anywhere in the
  string
status: Done
assignee: []
created_date: '2026-08-19 17:38'
updated_date: '2026-08-19 20:52'
labels:
  - tooling
  - hooks
dependencies: []
parent_task_id: TASK-52
ordinal: 61000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Replace the inline grep in the PreToolUse Bash hook (~/.claude/settings.json) with a small script beside ~/.claude/hooks/ensure-output-dir.py that decides using the command's structure instead of a substring scan.

The current check reads the whole tool_input.command string, so any guarded phrase appearing as DATA trips it. All five logged occurrences are this: a heredoc body, an unrelated step in a compound chain, the text of a friction log entry, a scratchpad path, and the description of the task reporting the bug.

Required behaviour: tokenize the command, drop heredoc bodies and single/double-quoted arguments, split on chain operators, and test only the head token of each segment against the guarded set. Keep the guarded set unchanged - history-rewriting pushes, hard tree resets, recursive force deletes, and PR closure.

Acceptance: all five logged false positives (bl_9d1b5e0a095f, bl_ddf8641d9e53, bl_484c4032a11e, bl_f334bf71a138, bl_0a62d7a789af) pass, and a fixture set of genuine destructive commands - including ones hidden behind leading env assignments, inside a chain, and with flags in either order - is still caught. Test both directions before installing; a regression here fails open on real destructive commands.

SCOPE: the hook is global config outside this repo. No blotter source changes.
<!-- SECTION:DESCRIPTION:END -->
