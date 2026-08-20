---
id: TASK-48
title: Decide whether dogear --evidence joins the redaction surface
status: Done
assignee: []
created_date: '2026-08-19 14:43'
updated_date: '2026-08-19 20:58'
labels:
  - redaction
dependencies: []
type: enhancement
ordinal: 56000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
add --evidence is redacted; dogear --evidence is stored verbatim. Verified: add stores "note ~/secretdir and <redacted>", dogear stores the raw home path and the raw token, and doctor --leaks then flags the dogear line as fixable:false with --leaks conflicting with --fix. This is NOT a defect against the current contract, and a 2026-08-19 audit reviewer refuted it as filed: r22 is a closed enumeration ('The rewrite applies to add command, stderr, and note evidence, including the hook failure-note lane', design doc line 374) naming four lanes, none of them dogear evidence; r25 extended redaction to dogear TEXT while deliberately leaving its evidence alone with that write lane open; and r25 already blesses the same shape for resolution --note/--amend ('a named deferral, not an oversight'). src/commands/schema.rs:9 and README both scope the redaction claim to add, so the published contract describes shipped behaviour. The open question is whether the deferral should stand now that doctor --leaks is a CI gate, since an operator pasting a token into a dogear turns the gate red with no repair path. Either extend redaction with an amendment, or write the deferral down where a reader of the dogear docs will see it.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A decision is recorded as a design-doc amendment: dogear evidence is redacted, or the deferral is stated explicitly alongside the resolution --note deferral
- [ ] #2 If redaction is extended, schema and README claims move with it and a regression test pins the new lane
- [ ] #3 If the deferral stands, the dogear docs say so, so the doctor --leaks result is not a surprise
- [ ] #4 All four gates pass
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Decision: extend. dogear --evidence and resolve --note/--amend join the write-time redaction surface (design doc r34), superseding r25's resolution deferral; one rule now covers every authored free-text field.
<!-- SECTION:NOTES:END -->
