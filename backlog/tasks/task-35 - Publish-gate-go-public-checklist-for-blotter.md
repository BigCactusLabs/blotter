---
id: TASK-35
title: 'Publish gate: go-public checklist for blotter'
status: Done
assignee: []
created_date: '2026-08-18 16:55'
updated_date: '2026-08-18 22:25'
labels: []
dependencies: []
ordinal: 43000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Blocked-until-publish work, deliberately kept out of TASK-34 (scanner code fix). Steps, in order, against the final pre-publish tree: (1) the owner decides history posture — flatten into a fresh repo (leaned toward, 2026-08-18) vs git-filter-repo rewrite; verified 2026-08-18 that all upstream-authored commits are already public upstream with identical SHAs, so either choice adds no authorship exposure. (2) Re-run the deny gate with a pinned scanner version: doctor --leaks with the private deny list (names deliberately not recorded in the repo) plus a tree-wide grep with NO backlog exclusion; do not use word-boundary escapes in the grep — they silently false-negative here. (3) Adversarial prose read of every doc predating the fork — TASK-33's lesson is that leaks live in prose, not path tokens, and pattern scans cannot catch them. (4) Confirm LICENSE attribution stands (settled 2026-08-18: yes, upstream author is public). (5) If TASK-34 has landed, the dash-encoded-slug check is part of the gate; if not, grep for the dash-encoded form manually.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Gate run 2026-08-18 (batch PR with TASK-34; scanner = the TASK-34 build): (2) doctor --leaks + private deny list → healthy:true, 0 findings; tree-wide fixed-string grep (tracked files, no backlog exclusion, no word-boundary escapes) clean outside accepted upstream attribution. (3) Adversarial prose read complete: findings all remediated in the same batch (log scrub with recomputed IDs — sanctioned append-only exception; backlog prose neutralized; design-doc examples and provenance lines trimmed; test fixtures renamed). (4) LICENSE attribution confirmed. (5) dash-encoded-slug check is now part of doctor --leaks.

Posture resolved 2026-08-18: flatten — a brand-new public repo with a single commit; the existing repo is renamed to a private archive and kept private. Rename severs nothing until the old name is recreated, at which point redirects to the archive stop (documented GitHub behavior, intended here). Multi-scanner leg complete: trufflehog 3.97.0 + kingfisher 1.113.0 (both brew core), each validated against a canary corpus first — canaries must be format-exact (kingfisher checksum-validates GitHub PATs), and vendor doc example keys are allowlisted by scanners, so never use them as canaries. Staged flat tree scanned clean; sole trufflehog finding is the deliberate synthetic redaction fixture in tests/cli.rs (accepted).

Adversarial plan review (Opus, 2026-08-18 evening) found the earlier gate had not covered log records appended after it ran, plus prose leaks in the pre-fork era of the log. Remediation batch (task/35-publish-remediation): pre-fork pc_ era dropped from the working log (archive history retains it); two hook-captured records redacted with recomputed IDs; the stale hook binary path repaired (it had been writing unredacted paths); README install section rewritten for public reality; changelog closed as 0.15.0 with version bump; private-repo fixture URL in tests neutralized. Review also corrected two mechanics claims: push protection IS on by default for new public personal repos (since 2024-03), and a blocked create retry shows HTTP 422, not 404.

Remaining, in order: (a) re-run all four gate legs against a fresh re-stage of this branch's tree and record line count + tree hash together; (b) owner merges the remediation PR; (c) rename repo to the private archive name, update local remote, create the new public repo (--disable-wiki), push the flat commit, tag v0.15.0 (never push old tags); (d) publish blotter-cli to crates.io at flip time to close the namesquat window; (e) post-flip: verify push protection, set GITHUB_TOKEN read-only, add branch ruleset blocking force-push and stray branch creation, re-file open issue #27 publicly, freeze the archive (block pushes). Owner decision recorded: development moves to a fresh clone of the public repo; the archive is frozen absolutely — any archive-side work cherry-picks forward through the full gate.

Flip executed 2026-08-18 evening, all steps: repo renamed to blotter-archive (private) and frozen via GitHub repo-archive (rulesets need Pro on private repos — 403 on free plan; repo archiving is the free-tier freeze and is reversible); public BigCactusLabs/blotter created (--disable-wiki), flat commit pushed, v0.15.0 tagged; secret scanning + push protection confirmed enabled by default; GITHUB_TOKEN set read-only; protect-history ruleset (non_fast_forward + deletion, all branches) active on public; issue #27 re-filed as public #1 (body scanned clean first); blotter-cli 0.15.0 published to crates.io. Development home is now a fresh clone of the public repo; the old local clone points at the read-only archive and must never gain the public remote.
<!-- SECTION:NOTES:END -->
