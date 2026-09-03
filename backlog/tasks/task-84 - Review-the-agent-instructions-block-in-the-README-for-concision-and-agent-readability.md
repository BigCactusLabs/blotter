---
id: TASK-84
title: >-
  Review the agent-instructions block in the README for concision and agent
  readability
status: To Do
assignee: []
created_date: '2026-09-03 14:24'
labels:
  - docs
  - agent-ux
dependencies: []
ordinal: 80000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The 'Paste this into your CLAUDE.md / AGENTS.md' block in README (also mirrored in this repo's AGENTS.md § Dogfood and the dogear help text) reads as prose written for a person deciding whether to install. Agents parse it into rules, and several of those rules are hard to extract. Observed on 2026-09-03 while upgrading walkmaxx to 1.1.0: the consumer rewrote the block locally to about half the length.

Specific friction in the current wording:
- The five admission grounds (transferable, consequential, recurring, misleading, systemic) sit in one five-clause sentence. A list matches faster.
- 'Do not add global, system, or internal friction' is the first line, detached from the skip list it belongs to.
- The dogear bar, 'all three, where a cut needs any one of its grounds', is the sentence most likely to be misread; state the three conditions directly.
- --impact is explained in prose but not shown on the add command line; --disposition on resolve is not shown at all, though it is mandatory.
- The block has no resolve guidance, so consumers bolt it on themselves.
- Ordering: an agent decides check → decide → file → resolve; the block is ordered as pitch → bar → command.

Suggested shape (from the walkmaxx rewrite): one-line framing, bulleted grounds, skip list including the global/system/internal rule, add command with --impact shown, one paragraph on impact and tool-failure flags, dogear bar as three plain conditions plus command, resolve command with --disposition shown. Keep the block copy-pasteable and under ~200 words. Sync AGENTS.md § Dogfood, dogear/resolve --help after-help, and schema admission strings to the same wording if they change.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 README agent-instructions block lists the five grounds as bullets and states the dogear bar as three plain conditions
- [ ] #2 add and resolve command lines in the block show --impact and --disposition respectively
- [ ] #3 Block stays under ~200 words and remains copy-pasteable as-is
- [ ] #4 AGENTS.md § Dogfood, dogear/resolve help text, and schema strings agree with the revised wording
<!-- AC:END -->
