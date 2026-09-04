---
id: TASK-2
title: 'Distribution: cargo-dist / homebrew / curl installer'
status: Done
assignee: []
created_date: '2026-08-03 20:39'
updated_date: '2026-09-04 21:15'
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
- [x] #2 HOMEBREW_TAP_TOKEN is configured with Contents write access limited to BigCactusLabs/homebrew-tap.
- [x] #3 The tagged release publishes binary archives and installers, and the tap publishes blotter.rb.
- [x] #4 Published installation paths are verified before README commands are advertised.
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Prepare version and changelog changes and validate them. Configure the scoped tap publishing credential. Publish the release tag, verify release assets and installation, then update README commands and close this task.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-09-04: Release preparation merged in PR #35; v1.1.1 published at d5dce8bc61767bcf42073157ff7258e40322ff71. Release workflow 33919701051 succeeded across all six platform builds and Homebrew publishing. All 19 release asset SHA-256 digests verified; six archives inspected for their binaries; four Homebrew target URLs and checksums matched the archives. Shell and Homebrew installations on Apple Silicon macOS reported blotter 1.1.1; shell-installed schema command passed. Latest shell and PowerShell URLs match the verified release assets. Linux and Windows installers were not executed locally. Stable tests (374), Rust 1.89 tests (374), release build, Clippy, formatting, and dist plan passed. README commands added only after live installation validation. HOMEBREW_TAP_TOKEN has Contents read/write limited to BigCactusLabs/homebrew-tap and expires 2026-12-03.
<!-- SECTION:NOTES:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: codex
created: 2026-08-13 03:04
---
2026-08-12 streamline audit: distribution now depends on TASK-27 because installer and packaged-release support claims need a verified MSRV and current dependency baseline.
---
<!-- COMMENTS:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Published v1.1.1 binaries and shell/PowerShell installers for six platforms, enabled automatic Homebrew formula publishing, and documented verified installation paths. Release checks and macOS shell/Homebrew installation tests passed.
<!-- SECTION:FINAL_SUMMARY:END -->
