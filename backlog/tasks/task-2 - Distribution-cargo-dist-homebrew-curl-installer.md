---
id: TASK-2
title: 'Distribution: cargo-dist / homebrew / curl installer'
status: In Progress
assignee: []
created_date: '2026-08-03 20:39'
updated_date: '2026-09-04 21:03'
labels:
  - release
dependencies:
  - TASK-27
ordinal: 2000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Ship the first Blotter CLI release with cargo-dist binary archives, shell and PowerShell installers, and the Homebrew formula. The distribution workflow is on main; v1.1.0 predates it and has no release assets.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 CLI v1.1.1 passes stable and MSRV checks and cargo-dist planning.
- [ ] #2 HOMEBREW_TAP_TOKEN is configured with Contents write access limited to BigCactusLabs/homebrew-tap.
- [ ] #3 The tagged release publishes binary archives and installers, and the tap publishes blotter.rb.
- [ ] #4 Published installation paths are verified before README commands are advertised.
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Prepare version and changelog changes and validate them. Configure the scoped tap publishing credential. Publish the release tag, verify release assets and installation, then update README commands and close this task.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-09-04 current state: cargo-dist 0.32.0 workflow and BigCactusLabs/homebrew-tap already exist. CLI v1.1.1 is prepared. Release build, stable tests (374), Clippy, formatting, Rust 1.89 tests (374), and dist plan --tag v1.1.1 all passed. The plan includes six platform archives, shell/PowerShell installers, and blotter.rb. HOMEBREW_TAP_TOKEN is not configured yet; GitHub requires identity verification before token creation. Do not push the release tag until the scoped token is stored. Verify published assets and installation before adding README installer commands.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: codex
created: 2026-08-13 03:04
---
2026-08-12 streamline audit: distribution now depends on TASK-27 because installer and packaged-release support claims need a verified MSRV and current dependency baseline.
---
<!-- COMMENTS:END -->
