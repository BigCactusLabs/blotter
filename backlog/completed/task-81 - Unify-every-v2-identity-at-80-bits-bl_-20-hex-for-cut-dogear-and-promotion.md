---
id: TASK-81
title: 'Unify every v2 identity at 80 bits: bl_ + 20 hex for cut, dogear and promotion'
status: Done
assignee: []
created_date: '2026-09-02 15:10'
updated_date: '2026-09-02 15:36'
labels:
  - v2
dependencies: []
ordinal: 84500
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Progress-review feedback (2026-09-02, recorded in docs/plans/2026-09-01-blotter-v2-plan.md §12) on the ID widths r48 carried into v2: cut 48-bit/12 hex, dogear 80-bit/20 hex, promotion planned at 64-bit/16 hex, all under one bl_ namespace with the r48 rule that ambiguity is decided before kind and an exact full ID takes no precedence. A complete shorter ID can therefore be a prefix of a longer record ID and become unresolvable (r50 records this and accepts it). The differing widths are residue from when width separated namespaces; bl2 plus the kind field now does that inside the hash. Decision: every v2 identity is the first 10 digest bytes, bl_ + 20 lowercase hex. Lands as amendment r51 (supersedes the r48 identity table widths and the r50 48-bit birthday note), then one implementation PR into v2 BEFORE TASK-76 so promotion never introduces a third width. Scope: compute_id for cuts to 10 bytes; schema record examples and the identity table it publishes; every test fixture, golden file and script that hard-codes a 12-hex cut ID; README examples; CHANGELOG entry under the unreleased 1.0.0 heading. Fold, prefix rule and probe are untouched. implementer-opus-med in a worktree off v2, one pr-reviewer-high pass (identity is a silent-failure domain), single gate run (no store.rs mechanics change).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 r51 lands in the design doc: one width, 20 hex, for all three kinds, with the r48 table restated
- [ ] #2 compute cut IDs at 10 digest bytes; schema publishes bl_<20 lowercase hex> for cut, dogear, resolve and promotion
- [ ] #3 no fixture, golden file or doc example carries a 12-hex bl_ ID; a contract test pins the width for both existing kinds
- [ ] #4 cargo test/clippy/fmt gate green; CHANGELOG names the width change under 1.0.0
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
PR #20 into v2 opened 2026-09-02 after one pr-reviewer-high pass (REWORK, all findings applied). Awaiting Quinn's review.
<!-- SECTION:NOTES:END -->
