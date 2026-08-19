---
id: TASK-13
title: Archive completed papercuts remediation machinery
status: Done
assignee: []
created_date: '2026-08-06 12:23'
updated_date: '2026-08-07 01:37'
labels:
  - chore
dependencies: []
ordinal: 13000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The July-2025 remediation wave shipped; its ratchet is still in the tree and still runs on every test invocation. tests/manifest_checker.rs (198 lines) plus scripts/check-manifest.sh gate docs/plans/papercuts-remediation-manifest.md against hardcoded pc_ IDs for work that is done. Alongside it, 12 tracked papercuts-era docs (docs/papercuts-diagnostic-report-2026-07-15.md alone is 5154 lines), root-level fresh-eyes-review-2026-07-16.md, and a 122KB model-performance-journal.md sit where every agent listing the repo root sees them. Git preserves all of it; tag the last audited revision, then remove. Also drop tests/** from the Cargo.toml include list -- the published crate does not need the black-box suite.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 tests/manifest_checker.rs, scripts/check-manifest.sh, and docs/plans/papercuts-remediation-manifest.md are deleted
- [ ] #2 Historical papercuts docs, fresh-eyes-review-2026-07-16.md, and model-performance-journal.md are moved out of the repo root and docs/ top level (archive dir or removed entirely)
- [ ] #3 docs/plans/2026-07-09-papercuts-design.md is retained -- AGENTS.md still designates it normative
- [ ] #4 Cargo.toml include no longer lists tests/**; cargo package --list confirms the archive contents
- [ ] #5 cargo test --all-features passes with the manifest_checker target gone
<!-- AC:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-08-06 13:35
---
Review 2026-08-06: all figures verified exactly (198-line checker, 5154-line report, 122KB journal, tests/** in Cargo.toml include). Nothing in src/ references the deleted files. Care points: three records in .blotter.jsonl cite scripts/check-manifest.sh in evidence.cmd -- append-only data, leave untouched; deleting the remediation plan/review docs leaves cosmetic dangling references in docs/superpowers/specs/2026-08-04-blotter-rename-design.md.
---
<!-- COMMENTS:END -->
