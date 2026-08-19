---
id: TASK-21
title: 'Decide the fate of HookTarget::Codex'
status: Done
assignee: []
created_date: '2026-08-06 12:25'
updated_date: '2026-08-07 02:31'
labels:
  - chore
dependencies: []
ordinal: 21000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
hook exec codex returns Ok(()) unconditionally (hook.rs:56-59) and hook install codex errors out, both because openai/codex#21753 blocks failure detection -- Codex hook payloads do not expose shell exit status. The behavior is honest and documented, but it is a CLI enum value that cannot do anything, and it costs a ValueEnum variant, two match arms, help text, and test coverage. Make it a decision rather than a leftover: either keep it deliberately as a signpost with a comment saying so, or drop the variant until upstream lands and re-add it then. Cheap either way; the point is that the current state reads as unfinished work rather than a choice. Check whether openai/codex#21753 has moved before deciding.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 openai/codex#21753 status is checked and recorded in the task notes
- [ ] #2 Either the variant is removed -- updating cli.rs:196 (ValueEnum), cli.rs:289 (command table), both hook.rs match arms, schema.rs:31 targets list, README.md:65, and tests/cli.rs:3303/3345 -- or the existing why-it-cannot-work comment at hook.rs:56 is extended to state that retaining the variant is deliberate (a comment already exists there; the AC is about intent, not presence)
- [ ] #3 If removed: the schema envelope targets list changes, so the removal ships in the TASK-14/19 breaking bundle, not as a standalone chore. If kept: blotter hook install --help no longer advertises a target that cannot be installed
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
openai/codex#21753 (Full Claude Code Hook Parity (29+)): still OPEN as of 2026-08-06, last activity 2026-07-30, 29 comments, labels enhancement/hooks. Shell exit status still not exposed to hooks, so exec failure detection remains unimplementable upstream. Removal is NOT the free chore the original framing implied -- the variant surfaces in six places including the schema stdout envelope, making removal a contract change; keep-with-comment is the genuinely cheap option, removal belongs in the breaking bundle.
<!-- SECTION:NOTES:END -->
