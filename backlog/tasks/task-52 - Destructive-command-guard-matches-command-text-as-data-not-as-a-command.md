---
id: TASK-52
title: 'Destructive-command guard matches command text as data, not as a command'
status: To Do
assignee: []
created_date: '2026-08-19 17:35'
labels:
  - tooling
  - hooks
dependencies: []
ordinal: 60000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The user-level PreToolUse Bash guard in ~/.claude/settings.json greps the whole .tool_input.command string for its destructive patterns. It has no notion of what is being EXECUTED versus what is merely quoted data, so it blocks benign commands whose payload happens to mention a guarded pattern.

Five occurrences, now the top cluster in blotter triage:
- bl_9d1b5e0a095f (2026-08-19) writing a verification script: the executed command was 'cat > file', but a recursive delete inside the HEREDOC BODY matched and the whole write was refused.
- bl_ddf8641d9e53 (2026-08-19) a compound gate command: the pattern matched an unrelated scratch-dir setup step and the entire chain was refused.
- bl_484c4032a11e (2026-08-18) 'blotter add' was itself blocked because the CUT TEXT named a guarded command. Logging friction about these commands is impossible.
- bl_f334bf71a138 (2026-08-18) a recursive delete targeting the session scratchpad, documented as prompt-free, was blocked anyway.
- bl_0a62d7a789af (2026-08-19) filing THIS task was blocked, because the description quoted the pattern names as data.

SCOPE: the hook lives at ~/.claude/settings.json, OUTSIDE this repo. Filed here because this is where the evidence accumulated and where the tracker lives. The fix is to the global config; no blotter source changes.

The failure is asymmetric. A false negative lets a real destructive command through; a false positive teaches the agent to route around the guard, which is worse long-run. The current wording correctly forbids retrying variants, so each false positive costs a full turn and, in the last case, made the problem unreportable.

Cheapest credible fix: match on command position rather than anywhere in the string. Strip heredoc bodies and quoted arguments before matching, or match only the first word of each command segment. A scratchpad-path allowlist separately fixes bl_f334bf71a138.
<!-- SECTION:DESCRIPTION:END -->
