---
id: TASK-33
title: >-
  Open-sourcing leak remediation: scrub tracked home paths, decide history
  posture
status: Done
assignee: []
created_date: '2026-08-18 14:27'
updated_date: '2026-08-18 22:02'
labels: []
dependencies: []
ordinal: 41000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Blotter is intended to be open-sourced (owner decision, 2026-08-18). At filing, the dogfood log and several tracked docs contained private identifiers. Issue #28 (doctor --leaks + write-time redaction) protects future writes only. Work before going public: (1) scrub or rewrite existing tracked files, (2) run the doctor --leaks deny-list gate, (3) decide history posture — fresh history vs git-filter-repo rewrite (destructive; owner decides), (4) audit archived material for anything else private before publish. (Identifier literals and finding specifics are deliberately not recorded in this file.)
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Read-only audit 2026-08-18 covered all tracked files and history; findings were classed and either remediated or deliberately accepted. Specifics are deliberately not recorded here. Clear: no live secrets, no hostnames, no private infra in scripts, no .github.

Frontier-search verification 2026-08-18: GitHub's sensitive-data procedure uses git-filter-repo >=2.47 with --sensitive-data-removal, rotate secrets first; clones/forks/caches retain old data after rewrite. Fresh history is situational risk reduction, not the standard. Pin the scanner version used for any gate.

Decisions (owner, 2026-08-18): defer history posture — scrub the tree now, decide at publish (likely flatten into a fresh repo); prune the affected archive material; scrub the dogfood log in place keeping the real log; upstream attribution and LICENSE copyright intentionally kept — verified same day via GitHub API that the upstream repo is public and all upstream-authored commits (identical SHAs, author email included) are already public there, so keeping that authorship adds zero new exposure.

Execution 2026-08-18: deleted the private scratch and archive material and a dangling symlink; rewrote home-path prefixes to ~/ across the log and archived docs; redacted the remaining third-party identifiers; recomputed 7 content-derived cut IDs broken by text edits and updated 2 resolve refs. Gates: doctor --leaks with the private deny list → healthy:true, 0 findings; tree grep clean outside synthetic fixtures; build/test/clippy/fmt all green. Found during execution: doctor --leaks misses dash-encoded home slugs (-Users-name-) — filed as its own task. Remaining before publish: history posture decision; re-run the deny gate with a pinned scanner as the final pre-publish step.
<!-- SECTION:NOTES:END -->
