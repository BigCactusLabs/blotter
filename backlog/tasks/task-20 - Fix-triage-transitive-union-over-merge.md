---
id: TASK-20
title: Fix triage transitive-union over-merge
status: Done
assignee: []
created_date: '2026-08-06 12:25'
updated_date: '2026-08-07 01:39'
labels:
  - bug
dependencies: []
ordinal: 20000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
triage.rs:133-139 computes pairwise linked() then feeds every match into union-find, which takes the transitive closure of the similarity relation. If cut A resembles B and B resembles C, A and C land in the same cluster even when they share no tags and no tokens. For a chronic-cut detector whose whole output is you have hit this N times, silently over-merging unrelated cuts is a correctness defect, not a tuning knob -- and it gets worse as the log grows, because longer similarity chains are more likely. Cluster on direct similarity instead: either require every member to be linked to a cluster representative, or keep union-find but raise the link bar so chaining is unlikely and document the residual. Note the pairwise loop is also O(n^2) over open cuts, which is fine at current log sizes and not worth addressing yet. Related to TASK-10, which adds normalized-title occurrence counting on top of this clustering -- fix the merge semantics first so occurrence counts are not built on over-merged groups.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A cut linked to B but not to C never appears in a cluster with C
- [ ] #2 A regression test builds an explicit A~B~C chain with A and C disjoint and asserts they do not co-cluster
- [ ] #3 Existing triage tests still pass or their expectations are updated with a stated reason
- [ ] #4 triage stdout remains byte-deterministic for a fixed input log
- [ ] #5 If the fix clusters via a representative, the representative is chosen by a stable rule (earliest timestamp, then lowest ID) -- membership must not depend on iteration order, or AC #4 determinism breaks
- [ ] #6 The untagged-untagged clause in linked() (triage.rs:185-189: two untagged cuts always pass the tag conjunct) is either kept deliberately with the chaining exposure documented, or tightened -- untagged cuts are where transitive chains are most likely in practice
<!-- AC:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-08-06 13:34
---
Review 2026-08-06: defect confirmed at triage.rs:133-139; linked() requires shared tag AND Jaccard >= 0.5, so organic chaining is rarer than the description implies except among untagged cuts, where the tag conjunct is free. Determinism today rests on BTreeMap grouping plus fixed union order (triage.rs:141-162) -- the representative-based fix is the one place it could silently regress.
---
<!-- COMMENTS:END -->
