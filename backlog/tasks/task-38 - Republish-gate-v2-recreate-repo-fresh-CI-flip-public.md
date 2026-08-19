---
id: TASK-38
title: 'Republish gate v2: recreate repo fresh, CI, flip public'
status: To Do
assignee: []
created_date: '2026-08-19 02:56'
updated_date: '2026-08-19 03:10'
labels: []
dependencies: []
ordinal: 46000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Successor to TASK-35: delete and recreate the GitHub repo with a single fresh initial commit, land CI (TASK-37), then flip public. Driven by the 2026-08-19 sensitivity scrub (PR #2) and its review: material removed from HEAD remains reachable through pre-scrub history, and HEAD-only deletion is not a scrub, so the repo is republished from a fresh object store (the TASK-35 fresh-repo playbook; no ruleset lift needed). Full operational detail — what was removed, where private copies live, review provenance — is deliberately NOT in this task: it lives in blotter-private-notes/ outside the repo, because this file ships in the published tree. Record execution results in this task's implementation notes, not in the dogfood log. Accepted losses: existing issues and PRs (bodies preserved privately per AC 1) and repo settings, re-applied per AC. Known facts: the crates.io Repository link is URL-addressed and survives same-name recreation; the published crate tarball is governed by the Cargo.toml include allowlist and is clean — that allowlist is now load-bearing for privacy, comment it accordingly; GitHub keeps a deleted repo owner-restorable for ~90 days — that window is residual exposure, never a fallback: do not restore, and same-name recreation makes restore practically unavailable anyway; deleting a private repo deletes its forks (count is 0); no cooldown on reusing your own repo name; deletion does not retract clones or event-firehose traces from the original public window — the fresh repo closes future reachability, not past exposure. Actions default workflow permissions are already read-only (verify, do not re-apply). Settings for the new repo: description and topics as recorded pre-delete; wiki disabled; secret scanning + push protection; homepage https://crates.io/crates/blotter-cli (new, not re-applied); protect-history ruleset (non_fast_forward + deletion, all branches) — apply AFTER the public flip, the API 403s on free-plan private repos for read and write alike; Codex review app re-installed before the TASK-37 PR so it gets reviewed. Owner go is required immediately before the delete and immediately before the flip; TASK-37 CI must be green before any visibility change.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Issue #1, PR #1 and PR #2 bodies AND the PR #2 review comment threads saved verbatim to blotter-private-notes/ before deletion
- [x] #2 Description and topics captured from the API before deletion; ruleset spec taken from its written definition (rulesets API is 403 while private on free plan)
- [x] #3 Residue pass on HEAD decided and executed per owner direction before the tree is staged (see private notes for the itemized list)
- [ ] #4 Fresh commit built in a NEW directory (git init, tracked tree copied without .git), deterministic author/committer dates; the old clone never gains the new remote and no --tags/--follow-tags/--mirror push is ever used; local v0.15.0 tag deleted from the staging path before any push
- [ ] #5 All four gates + doctor --leaks green on the staged tree BEFORE the first push
- [ ] #6 Owner gave explicit go immediately before repo deletion
- [ ] #7 Repo deleted and recreated PRIVATE as BigCactusLabs/blotter with wiki disabled; description/topics/homepage applied; Actions workflow permissions verified read-only
- [ ] #8 Single fresh initial commit pushed; rev-list --count HEAD is 1 on the remote; fresh v0.15.0 tag created on the new root and pushed explicitly by name
- [ ] #9 Codex review app re-installed on the new repo
- [ ] #10 TASK-37 CI workflow landed and green before any visibility change
- [ ] #11 Owner gave explicit go immediately before the public flip
- [ ] #12 Flipped public; protect-history ruleset applied post-flip and verified active; secret scanning + push protection confirmed on
- [ ] #13 From a clean temporary clone: git fetch origin b8012f9 rejected AND https://github.com/BigCactusLabs/blotter/commit/b8012f9 returns 404; crates.io repo link resolves
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
AC1-2 done 2026-08-19: issue #1 body, PR #2 body + review threads + reviews, and settings snapshot saved to private notes (github-preservation-2026-08-19.md). PR #1 does not exist — issue #1 holds that number. AC3: owner decided leave references as-is; no tree changes. Amendments from the adversarial plan review applied same day; review detail in private notes (task-38-review-2026-08-19.md).
<!-- SECTION:NOTES:END -->
