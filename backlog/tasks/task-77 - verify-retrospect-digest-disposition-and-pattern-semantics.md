---
id: TASK-77
title: verify/retrospect/digest disposition and pattern semantics
status: To Do
assignee: []
created_date: '2026-09-01 21:57'
updated_date: '2026-09-02 20:19'
labels:
  - v2
dependencies:
  - TASK-76
  - TASK-79
  - TASK-71
ordinal: 86000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Phase 5 of docs/plans/2026-09-01-blotter-v2-plan.md. Last PR into v2 after TASK-75 and TASK-76. verify anchors only resolved cuts with disposition fixed or promoted; envelope adds disposition to resolution{}. retrospect type becomes pattern recurrent_friction|failed_intervention (two patterns only) plus suggested[]. digest adds one accepted count field: cuts whose winning resolution is accepted and falls in the period; no section, no listing. implementer-opus-med, pr-reviewer-high.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
r49 (2026-09-01): the digest count field is named accepted_cuts, not accepted.

r49 review correction: accepted_cuts is shaped {count}, like new_cuts and open_dogears.

2026-09-02 progress-review rulings (plan §12, amendment r51), all three land in this PR:
1. TASK-79 is absorbed: verify measures recurrence from disposition_ts and exposes it in resolution{} (see TASK-79 for the test shape).
2. triage and digest drop suggested_action entirely. Not renamed to promote: triage detects chronic friction, retrospect interprets the pattern and suggests interventions, promote records a judgment. A raw triage cluster must not instruct an agent to institutionalize anything. Remove the field from TriageCluster, the schema entries for triage and digest, the md renderer if it prints it, and every test that asserts it.
3. TASK-71 is a precision gate, not an open-ended scoring project: after the admission-floor change, rerun the linked-pair measurement across several representative logs (see TASK-71 for the corpus and the v1-log caveat). If unrelated clusters are still produced materially, fix the linkage in this PR under r51's ceiling clause; if not, record the numbers in TASK-71 and close it without a code change. Either way the measurement is written down before Phase 6.

- r52 (2026-09-02): verify's documentation, schema description and --format md text state an empty recurrence set as 'no recurrence observed after disposition_ts' — evidence the intervention held, never 'fixed', 'confirmed' or 'proven'. No envelope change. Land the schema wording in this PR with the disposition_ts boundary. README's retrospect paragraph (line ~182) still uses r27 vocabulary (wrapper_alias/doc_repair/skill_candidate); update it with the pattern/suggested change.
<!-- SECTION:NOTES:END -->
