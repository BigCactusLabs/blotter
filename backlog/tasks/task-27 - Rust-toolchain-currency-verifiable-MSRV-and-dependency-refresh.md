---
id: TASK-27
title: 'Rust toolchain currency: verifiable MSRV and dependency refresh'
status: Done
assignee: []
created_date: '2026-08-11 15:54'
updated_date: '2026-08-13 03:31'
labels:
  - maintenance
  - build
dependencies: []
modified_files:
  - AGENTS.md
  - Cargo.toml
  - Cargo.lock
  - scripts/dev/check-msrv.sh
ordinal: 27000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Edition is already maxed -- rustc 1.97.1 reports the valid set as <2015|2018|2021|2024|future>, so edition = "2024" is the newest shippable value and needs no change (next edition expected 2027). The real gaps are MSRV and dependency drift.

MSRV is an unverified claim. Cargo.toml declares rust-version = "1.89" while the only installed toolchain is 1.97.1, there is no rust-toolchain.toml, and the repo has no CI. Nothing anywhere proves the crate still builds on 1.89, so the declared floor could already be wrong and no one would find out until a user on an older toolchain failed to install.

Decision to make: track latest stable (bump rust-version with each release, simplest for an agent-only tool) versus an N-minus-k policy that keeps older toolchains working. Raising the floor is a narrowing, not a free upgrade -- it buys nothing on its own and costs installability, so it should be paid for by a feature actually adopted.

Dependency drift as of 2026-08-11: sha2 0.10.9 locked against 0.11.0 available -- the only major bump, RustCrypto 0.11 line, needs an API review. clap 4.6.1 -> 4.6.6, jiff 0.2.32 -> 0.2.35, thiserror 2.0.18 -> 2.0.20 are all in-range patch refreshes that cargo update takes for free.

Related: TASK-2 (distribution) shares the installability concern -- whatever MSRV policy lands here constrains what a packaged installer can promise.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 MSRV policy is decided and written into AGENTS.md, with the reason
- [x] #2 Cargo.toml rust-version matches a floor that was actually built and tested, not assumed
- [x] #3 A repeatable way to verify the MSRV exists (pinned toolchain, documented cargo +<ver> check, or CI) so the claim cannot silently rot again
- [x] #4 In-range dependency patches applied; full gate green
- [x] #5 sha2 0.11 assessed explicitly -- adopted, or declined with the reason recorded
- [x] #6 Edition left at 2024; task notes why no edition work is possible
<!-- AC:END -->

## Comments

<!-- COMMENTS:BEGIN -->
author: codex
created: 2026-08-13 03:04
---
2026-08-12 streamline audit reconfirmed the task boundary: rustc/cargo 1.97.1 is installed, Cargo.toml still declares 1.89, no rust-toolchain file or CI workflow exists, cargo tree reports no duplicate dependency versions, and the release binary is 937,824 bytes. Do not reopen release-profile or feature trimming; finish verifiable MSRV policy and dependency currency here before distribution.
---

created: 2026-08-13 03:31
---
Decision: keep rust-version 1.89 as a compatibility floor, not a latest-stable policy. Rust 1.89.0 built and passed all 173 locked all-features tests before and after the dependency refresh. scripts/dev/check-msrv.sh derives the declared floor from Cargo.toml, requires that exact rustup toolchain, and runs the locked suite. Adopted sha2 0.11.0: upstream declares Rust 1.85, keeps the Digest/Sha256 incremental API used here, and the pinned ID/hash tests pass unchanged. Upstream evidence: https://docs.rs/crate/sha2/0.11.0/source/CHANGELOG.md and https://docs.rs/crate/sha2/0.11.0/source/Cargo.toml. Edition remains 2024 because it is the newest stable edition; there is no edition migration to perform.
---
<!-- COMMENTS:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Verified and retained MSRV 1.89, added a repeatable locked MSRV test script, documented the compatibility policy, refreshed all compatible lockfile dependencies, and adopted sha2 0.11.0. Rust 1.89.0 and stable both pass all 173 tests. Release build, Clippy with warnings denied, formatting, shell syntax, and diff checks pass.
<!-- SECTION:FINAL_SUMMARY:END -->
