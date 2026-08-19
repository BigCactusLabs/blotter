---
id: TASK-52.2
title: Change the destructive-command guard from deny to ask
status: To Do
assignee: []
created_date: '2026-08-19 17:39'
labels:
  - tooling
  - hooks
dependencies: []
parent_task_id: TASK-52
ordinal: 62000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Switch the PreToolUse hook's permissionDecision from deny to ask, AFTER the command-position matcher lands.

Rationale: CLAUDE.md already says to confirm before any destructive or shared-state action, each time. A hook that asks implements that rule exactly. A hook that denies implements a stricter rule that was never written, and turns every false positive into a lost turn plus an instruction not to retry - which is how the guard came to block its own bug report.

Economics change in the right direction: a false positive costs one keystroke instead of a whole turn, and a true positive becomes the confirmation the rule already asks for.

ORDER MATTERS: do not do this before the matcher fix. While away from the keyboard, a false-positive prompt stalls the session entirely, which is worse than a refusal the agent can adapt around. The matcher fix is what makes ask safe.

Also revisit the message text. The current wording forbids retrying variants, which is correct under deny but wrong under ask - there the right instruction is to explain what the command does and why it is needed.

SCOPE: the hook is global config outside this repo. No blotter source changes.
<!-- SECTION:DESCRIPTION:END -->
