---
id: TASK-2
title: 'Distribution: cargo-dist / homebrew / curl installer'
status: To Do
assignee: []
created_date: '2026-08-03 20:39'
updated_date: '2026-08-13 03:04'
labels:
  - release
dependencies:
  - TASK-27
ordinal: 2000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Deferred from v0.1 scope in docs/plans/2026-07-09-papercuts-design.md (line ~168). Contract is stable at 0.5.0; natural next release theme.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Research complete 2026-08-06 (frontier-search + codex sweep). DECISION-READY RECOMMENDATION: adopt dist (upstream axodotdev/cargo-dist) as release engine + shell/powershell installers + auto-published Homebrew tap (BigCactusLabs/homebrew-tap) + cargo-binstall metadata. release-plz deferred (its default GITHUB_TOKEN won't trigger the dist workflow — needs PAT). Key facts: axo.dev web properties dead but OSS project alive (v0.32.0 2026-05-22, docs at axodotdev.github.io/cargo-dist/book/); astral-sh fork ARCHIVED Dec 2025, merged upstream; bus factor ~1 is the real risk, tolerable since dist emits plain committed GH Actions YAML. Blotter gotchas: (1) crate name blotter-cli vs bin name blotter — set formula = "blotter" explicitly; (2) no .github/workflows yet; (3) Homebrew 6.0.0 tap-trust makes third-party tap installs a 2-step prompt flow. Full findings: session artifact task-2-frontier.md. IMPLEMENTATION BLOCKED ON USER: needs tap repo creation + GitHub-side setup.

Codex cross-model sweep completed (task-2-codex-sweep.md): confirms frontier findings on every substantive point (dist alive v0.32.0; Astral fork gone; dist+binstall+GH Releases stack; release-plz optional with single-tag-owner caveat). Divergences: sweep reads maintenance healthier (227 commits/yr incl. woodruffw, eegli, not just dependabot); sweep would defer the tap until macOS demand vs frontier's day-one auto-publish. Both: you own the generated YAML, so tool death is survivable. Sweep adds: dist supplies the curl|sh surface with checksums+attestations, so no hand-rolled installer.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: codex
created: 2026-08-13 03:04
---
2026-08-12 streamline audit: distribution now depends on TASK-27 because installer and packaged-release support claims need a verified MSRV and current dependency baseline.
---
<!-- COMMENTS:END -->
