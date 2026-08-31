---
id: TASK-69
title: >-
  Retire finished tasks with 'backlog task complete', not 'backlog task archive'
  -- archive releases the ID for reuse
status: Done
assignee: []
created_date: '2026-08-31 14:41'
updated_date: '2026-08-31 15:03'
labels: []
dependencies: []
type: chore
ordinal: 79000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`backlog task archive <id>` releases the task ID for reuse. `backlog task complete <id>` does not. The repo used `archive`, so every retired ID was a landmine, and two had already fired.

Mechanism (Backlog.md 1.50.1, verified 2026-08-31): the allocator issues max+1 over the IDs it finds in `tasks/` and `completed/`, and never reads `archive/`. Reproduction is four commands -- create alpha, beta, gamma (task-1/2/3), `backlog task archive TASK-3`, then create delta, which is issued task-3 again; `backlog task view TASK-3` then resolves to delta and gamma is unreachable by ID. A second probe confirms the allocator reads the *contents* of `completed/`, not a record of what the CLI retired: hand-placing a `task-40` file there pushed the next create to task-41. So a plain file move reserves an ID; `backlog task complete` is not required, and cannot be used on an already-archived task ("not found").

This is intentional upstream, not a bug. PR #789 ("Permanent task IDs: unify draft+task pool, stop recycling archived IDs") proposed exactly the fix and was closed unmerged on 2026-07-15: "Backlog.md has an explicit product contract here: archive is soft delete. Archiving ends the task ID's active identity and intentionally releases that ID for future use." No open upstream issue tracks it, and upstream has instead shipped duplicate-ID detection (#740, #749, #781). Nothing to file upstream; the fix is a local convention.

Damage before the fix: a `task-66` in the archive collided with a newly created active task-66 on 2026-08-31 (cleared by renumbering the new one to TASK-68), and the archive already held two different `task-58` files. The cost is that every `TASK-N` reference in a commit message was only as durable as the archive policy -- commit cc5cbfa says "park TASK-66 as a dogear" and that ID had stopped resolving to it.

Done: AGENTS.md now names `backlog task complete` as the retire path, and all 8 files moved from `backlog/archive/tasks/` to `backlog/completed/`, reserving IDs 6, 7, 8, 9, 58, 58, 61 and 66. Those 8 are addressable by ID again, which `archive/` had prevented.

Caveat to keep in view: none of the 8 were finished work. All carry `status: To Do`, and the archiving commits show why -- 91ba2fa superseded one task-58, d8c02ef called the other moot, a08e149 retired TASK-61, cc5cbfa parked TASK-66 as a dogear, and task-6/7/8/9 are the Frog prior-art shortlist deferred at the initial public release. `backlog/archive/tasks/` was the deferred/won't-do pile, so `backlog/completed/` now holds it. The mislabel is the directory name only: their status fields are untouched, `backlog task view` reports "To Do", and they stay out of `backlog task list` including `--status "To Do"`, exactly as they did under `archive/`.

Resolved: moving the duplicate `task-58` pair into a scanned directory made the collision visible, and it is now repaired. `backlog doctor --fix` proposed renaming the hook-registration file, but `created_date` shows that file is the original TASK-58 (2026-08-24 16:38) and the entropy-residual file is the reissue the archive bug produced 17 minutes later (16:55), already superseded by TASK-59 before any work started. The reissue was renumbered to TASK-70 instead, by hand: doctor offers no flag to choose which file moves, and `backlog task edit` cannot reach `completed/` at all -- only `backlog task view` resolves there. TASK-58 again names the hook-registration task alone, TASK-70 names the superseded residual, and `backlog doctor` reports no duplicate IDs. The prose cross-references in TASK-59 and TASK-70 were updated by hand for the same reason.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 AGENTS.md 'Task backlog' section names 'backlog task complete' as the retire path and states that 'backlog task archive' releases the ID for reuse
- [x] #2 The 8 tasks in backlog/archive/tasks/ are moved to backlog/completed/ so their IDs are reserved
- [x] #3 The duplicate TASK-58 pair in backlog/completed/ is repaired via 'backlog doctor --fix' or explicitly accepted as historical, and 'backlog doctor' reports no blocking collision
<!-- AC:END -->
