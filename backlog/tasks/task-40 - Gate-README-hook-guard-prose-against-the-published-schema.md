---
id: TASK-40
title: Gate README hook-guard prose against the published schema
status: In Progress
assignee: []
created_date: '2026-08-19 13:54'
updated_date: '2026-08-19 14:44'
labels:
  - docs
  - tooling
dependencies: []
type: chore
ordinal: 48000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The README paragraph describing hook exec claude-code's noise guards drifts silently: r14's byte gate, r20's probe gate, and r29's shape gate each had to be hand-added to it after the fact, and r29 shipped in PR #1 with the paragraph still reading 'Three noise guards apply' until the merge session caught it. blotter schema already publishes every gate as structured data (payload.gates keys under hook.exec), so the drift is mechanically detectable. Add a black-box test that reads README.md and asserts every published tool_input.* gate key is named in the hook section, failing the build instead of drifting. Scope is the README hook prose only.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A test in tests/cli.rs reads the repository README.md and the schema envelope, and fails when a published hook payload gate key has no mention in the README hook section
- [ ] #2 The test names the missing gate in its failure message, so the fix is obvious without reading the test
- [ ] #3 The test passes on the current tree unchanged (r14 byte gate, r20 probe gate, r29 shape gate all already documented)
- [ ] #4 Deliberately undocumented gates, if any are ever wanted, have a single explicit allowlist in the test rather than silent omission
- [ ] #5 All four gates pass; no src/ behaviour, envelope, or contract change
<!-- AC:END -->
