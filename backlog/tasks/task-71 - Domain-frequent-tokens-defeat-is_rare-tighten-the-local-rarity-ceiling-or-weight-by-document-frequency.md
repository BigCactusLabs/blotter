---
id: TASK-71
title: >-
  Domain-frequent tokens defeat is_rare: tighten the local-rarity ceiling or
  weight by document frequency
status: To Do
assignee: []
created_date: '2026-08-31 15:35'
updated_date: '2026-09-02 15:11'
labels:
  - v2
dependencies: []
ordinal: 86200
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found by an independent research leg during the TASK-64 batch (2026-08-31), and deliberately kept out of that task's scope.

TASK-64 fixes the *English function word* half of verify's false-recurrence problem by replacing r19's 16-word stopword list with 122 entries derived from Snowball. That is the right fix at fixture scale, and it is provably the only one available there: a token shared by the two candidates under test has document frequency >= 2, and `is_rare`'s ceiling `max(2, ceil(N/4))` has a floor of 2, so in a corpus small enough to write as a test fixture every shared token is rare and no ratio can separate filler from content.

But it does not close the class. No general-English list will ever remove `cargo`, `clippy`, `gate`, `test`, or `run`, and those are exactly the tokens a *development-friction* corpus is dense in. Measured on this repo's own log at n=169 non-auto records, `is_rare` at `div_ceil(4)` admits any token under DF 36, so `test` (DF 32), `gate` (26), `not` (25) and `only` (23) all count as "locally rare". Three such tokens plus a shared tag link two unrelated cuts — the same defect TASK-64 reports, reached through domain vocabulary instead of English filler.

Rare-threshold sensitivity, linked pairs on the same 169-record corpus, current-16 list vs a Snowball-derived list:

  divisor  4 (today, DF<=36):  115 vs 102
  divisor  8            :       74 vs  71
  divisor 16            :       43 vs  39
  divisor 34 (DF<=5)    :       22 vs  22

The two lists converge exactly as the ceiling tightens, which is the finding: past some calibration the stopword list stops mattering and the document-frequency mechanism is doing all the work. The literature points the same way — Manning, Raghavan & Schutze record the IR trend from large stop lists to small ones to none at all, with idf weighting taking over (https://nlp.stanford.edu/IR-book/html/htmledition/dropping-common-terms-stop-words-1.html), and De Boom et al. on very short texts name document frequency as the mechanism that handles non-informative overlap (https://arxiv.org/pdf/1512.00765). Short one-or-two-sentence titles are that case.

Open questions this task should answer:
- What is the right ceiling? `div_ceil(4)` is very permissive at large N and inert at small N (for N <= 8 it is pinned at the floor of 2). A single divisor may be the wrong shape.
- Should the rare path weight tokens by document frequency rather than counting them past a boolean threshold?
- The `.max(2)` floor is what keeps the rare path alive on small corpora and is depended on by `triage_clusters_reworded_repeats_with_rare_shared_tokens` (N=2). Any change must keep fixture-scale behaviour working.
- Measure across several logs via `sweep`, not one repo's dogfood log. The n=169 numbers above are from a single corpus.

Contract note: `is_rare`'s ceiling is normative text, stated in r19 and restated unchanged in r44. Changing it needs a new amendment.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A tightened or reshaped local-rarity rule is chosen and justified with measurements across more than one log
- [ ] #2 Domain-frequent tokens such as cargo, clippy, gate and test no longer link unrelated cuts on the rare path
- [ ] #3 Fixture-scale behaviour still works: the N=2 reworded-repeat case in triage_clusters_reworded_repeats_with_rare_shared_tokens still links
- [ ] #4 The new rule lands as a design-doc amendment superseding r19/r44's ceiling
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-09-02 progress-review ruling: promoted from optional follow-up to a Phase 5 precision gate (dependency of TASK-77). Retrospect consumes the same clustering at a threshold of two linked records, so a bad rare-token linkage is no longer a wonky triage cluster; it becomes input to the promotion layer. Gate procedure: measure linked pairs on several representative logs, not one. Caveat: the v2 binary refuses v1 logs, and the existing dogfood logs are v1, so measure with the last 0.15 binary (main) or a throwaway copy converted to v2 for measurement only; the r44 linkage rules are identical on both. Decide fix-or-close on the numbers, and write them into this task either way.

Measurement corpus located 2026-09-02 (all v1 logs, no v:2 anywhere; several carry pc_ IDs): blotter/.blotter.jsonl 167 cuts/24 resolves; compas/.blotter.jsonl 132 cuts/70 resolves (pc_ present); walkmaxx/.blotter.jsonl 97 cuts/97 resolves (pc_); origin-brands-workspace/data-platform/.blotter.jsonl 105 cuts/64 resolves; eatmoji/tools/blotter/.blotter.jsonl 58 cuts/57 resolves (pc_). All under ~/Documents/GitHub. Smaller logs (Aski 15, moode 15, pantone-pipe 16) are fixture-scale and only useful for the floor check. Use these five, via the 0.15 binary on main.
<!-- SECTION:NOTES:END -->
