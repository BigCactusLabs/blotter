---
id: TASK-64
title: >-
  verify's rare-token fallback matches generic English words, producing false
  recurrences
status: Done
assignee: []
created_date: '2026-08-24 20:53'
updated_date: '2026-08-31 17:30'
labels: []
dependencies: []
ordinal: 74000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while auditing a real 66-record log from another project. `blotter verify` reported 6 recurrences; 3 are false.

`similar_enough` (src/commands/triage.rs) accepts a pair on its second path when they share at least MIN_RARE_SHARED_TOKENS (3) tokens that `is_rare` accepts, and `linked` only additionally requires one shared tag. Two causes compound:

1. STOPWORDS has 16 entries and omits common function words longer than two characters — not, would, have, only, was, has, its, been, when, which — so `scoring_tokens` keeps them.
2. `is_rare` uses `candidate_count.div_ceil(4).max(2)`, so a token present in up to 26% of the corpus counts as rare (17 of 66 here). "not" at exactly 17 occurrences passes.

Result: two unrelated cuts that share one tag and three filler words are reported as a recurrence.

Evidence (66-doc corpus, rare_limit 17):
- pc_c7677b2ef7ec (backlog git fetch cannot write FETCH_HEAD) x bl_9c4c0f664f64 (patch tool rejects a reverse git diff) — matched on ["git","not","only"], document frequencies [4,17,9], shared tag "tooling"
- pc_048616bff4b5 (module map for the raster codec path) x bl_7cc82e0fc989 (large multi-file apply_patch context mismatch) — matched on ["failed","not","would"], frequencies [7,17,16], shared tag "docs"
- pc_6b5b659bc09c (stale rg path) x bl_7cc82e0fc989 — matched on ["failed","have","would"], frequencies [7,4,16]

The genuine recurrence in the same run (pc_3f6e9f2aa7ca x bl_7cc82e0fc989) shares 8 tokens including context, patch, multi, smaller and large, so a stricter rule keeps it.

Likely fix: extend STOPWORDS with the common function words above, and/or tighten `is_rare` (div_ceil(4) is very permissive at small n), and/or require shared rare tokens to clear a specificity bar rather than only a document-frequency ceiling.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A regression test builds a corpus where two unrelated cuts share one tag and three filler words, and asserts verify reports no recurrence for that pair
- [x] #2 The true-positive pair with 8 shared content tokens still links in the same test corpus
- [x] #3 The chosen rule for STOPWORDS and/or is_rare is recorded in a design doc or code comment with the frequency reasoning
<!-- AC:END -->
