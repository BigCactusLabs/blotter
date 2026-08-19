---
id: TASK-19
title: Record-format and envelope breaks for the next major
status: Done
assignee: []
created_date: '2026-08-06 12:24'
updated_date: '2026-08-07 02:31'
labels:
  - breaking
dependencies:
  - TASK-14
ordinal: 19000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Four independent correctness/contract warts, each under an hour, bundled per the TASK-4 precedent of holding format breaks for one release. (1) Tag dedup asymmetry: compute_id dedups tags before hashing (lib.rs:287-289) but add.rs:47-48 only sorts before storing, so blotter add x --tag x --tag x stores tags:[x,x] with an ID computed from [x] -- the record disagrees with its own identity. The same defect exists in dogear: compute_dogear_id dedups (lib.rs:330-333) while dogear.rs:54-55 only sorts. The fold layer re-sorts but never dedups on read, so the mismatch persists into list output. Dedup at all four sites. (2) resolve response shape varies by arity: resolve.rs:189-201 emits record for one ID and records for many, forcing every consumer to branch; always emit records as an array. (3) cwd and repo store absolute paths (add.rs:59-66, dogear.rs:65-72; hook.rs:81 takes cwd from the payload), so the committed .blotter.jsonl publishes /Users/<name>/... on every record; store cwd relative to the repo root and drop repo. Fallback: for the global ~/.blotter/log.jsonl outside any repo (find_repo_root None), and for hook payload cwds outside the discovered repo, keep absolute cwd -- those logs are machine-local, not committed. cwd/repo are not compute_id inputs, so none of this perturbs identity or dedup. (4) main.rs:25 evaluates effective_now for every command; make the clock lazy for schema. Note --version already exits before the clock is read (main.rs:9-11), so only schema changes. Bundle with TASK-4's already-shipped ID framing and with TASK-14.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Stored tags are sorted and deduped identically to the tag set used for ID computation in BOTH add and dogear; regression tests cover repeated --tag on both commands
- [ ] #2 resolve always emits records as an array regardless of ID count; schema, README, and tests/cli.rs:969 (response-shape compatibility test) reflect it
- [ ] #3 Records inside a repo store repo-relative cwd and no repo field; records outside any repo keep absolute cwd (stated fallback); folding an existing log with the old fields still works
- [ ] #4 schema does not read or parse BLOTTER_NOW -- this deliberately changes BLOTTER_NOW=invalid blotter schema from exit 78 config_error to exit 0; tests/cli.rs:2148-2155 is updated and the exit-code change is listed under the breaking heading
- [ ] #5 Contract version in output.rs is bumped and every change is listed in CHANGELOG under a breaking heading
<!-- AC:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-08-06 13:34
---
Review 2026-08-06: added the dogear half of the tag-dedup bug, defined the no-repo cwd fallback (keep absolute for machine-local logs), flagged that lazy-clock schema is an exit-code contract change contradicting an existing test, and noted --version needs no change. Verified cwd/repo never feed compute_id, so the path change cannot affect record identity.
---
<!-- COMMENTS:END -->
