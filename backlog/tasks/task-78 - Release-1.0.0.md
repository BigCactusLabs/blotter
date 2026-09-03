---
id: TASK-78
title: Release 1.0.0
status: To Do
assignee: []
created_date: '2026-09-01 21:57'
updated_date: '2026-09-03 02:26'
labels:
  - v2
  - release
dependencies:
  - TASK-77
ordinal: 87000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Phase 6 of docs/plans/2026-09-01-blotter-v2-plan.md: the single v2 → main release PR, then tag and publish. Release hygiene from docs/plans/blotter-1.0.0-release-readiness-audit-2026-09-02.md §1–§3 landed ahead of this task on the release-hygiene branch; what remains is the cut itself (audit §4–§8). No new product scope in the release PR unless review exposes a blocker (audit §9).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Cargo.toml, Cargo.lock, CHANGELOG header, and OTLP golden all read 1.0.0 in one PR
- [ ] #2 AGENTS.md no longer carries the 'Unreleased v2' paragraph after the merge
- [ ] #3 check-msrv.sh on 1.89.0, cargo package --locked, and cargo publish --dry-run --locked all pass before merge
- [ ] #4 Release-binary smoke: full v2 lifecycle in a temp repo, and a v1 ledger is refused with zero bytes changed
- [ ] #5 Annotated tag v1.0.0 on the merge commit; crate published; cargo install blotter-cli gives 1.0.0
- [ ] #6 GitHub Release v1.0.0 exists with the migration warnings at the top; repo description updated
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. In the release PR (v2 → main): bump Cargo.toml and Cargo.lock to 1.0.0; CHANGELOG [Unreleased] → [1.0.0] - <date> keeping the two mandatory upgrade steps at the top; OTLP golden scope version → 1.0.0; remove the temporary 'Unreleased v2' paragraph from AGENTS.md explicitly (do not rely on the merge); re-record README quickstart output from the release binary if it drifted; archive docs/plans/blotter-1.0.0-release-readiness-audit-2026-09-02.md with an archived date.
2. Gates: the four-command gate; rustup toolchain install 1.89.0 --profile minimal && scripts/dev/check-msrv.sh; backlog doctor; cargo package --locked; cargo publish --dry-run --locked.
3. Smoke the release binary: --version, schema, --help, add --help; then in a fresh temp repo run add → list → dogear → promote → resolve --disposition promoted → verify → retrospect → digest → doctor; then point it at a v1 ledger and confirm unsupported_log_version, zero bytes changed, useful suggested_fix.
4. Merge with CI green; annotated tag v1.0.0 on the exact merge commit (matches v0.15.0 convention).
5. cargo publish --locked from the tagged commit; confirm crates.io shows blotter-cli 1.0.0, docs.rs builds, cargo install blotter-cli && blotter --version.
6. Create a GitHub Release for v1.0.0 (first stable; the repo has used bare tags before). Notes call out: fresh ledger / v1 refusal, hook lane removal, severity → impact, dispositions, promotions, 80-bit IDs, contract 6.
7. Update the GitHub repository description to name promotions and durable learning, not just cuts/dogears/triage/verify/digest.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Housekeeping already done on release-hygiene (2026-09-02): auto-capture archive status line, both v2 checkpoint docs archived, README upgrade/migration/admission fixes, Rust-API policy, Cargo description/keywords/categories, one-time leak scan of .blotter.v1.jsonl (keep it tracked; it is outside the crate include list).
<!-- SECTION:NOTES:END -->
