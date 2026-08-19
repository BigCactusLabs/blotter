---
id: TASK-17
title: Shrink the redaction engine to a documented best-effort pass
status: Done
assignee: []
created_date: '2026-08-06 12:24'
updated_date: '2026-08-07 02:31'
labels:
  - refactor
dependencies: []
ordinal: 17000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
add.rs:191-608 is roughly 418 lines -- 61 percent of the file and about 11 percent of all src -- of hand-rolled secret detection: camelCase key-segment splitting, two separate entropy category heuristics, authorization scheme span parsing, schemeless-URL detection, and path-vs-secret disambiguation. Coverage is one unit test in add.rs plus 16 redaction-focused black-box tests in tests/cli.rs (lines 114-768), several of which pin exactly the heuristics slated for deletion -- shrinking means deleting or rewriting roughly 10 CLI tests, not one. The pass is also not add-only: redact_and_truncate is pub(crate) and hook.rs:110 runs it on hook-filed stderr, the unreviewed path where redaction matters most. No crate can absorb this: the secret* crates (secrecy, redact, secret-string) are Debug-redacting wrapper types solving a different problem, and secretscan, the only in-text scanner on crates.io, is 0.2.2 with 2826 downloads and no release since 2025-07-30. The literature on regex-plus-entropy detection (arxiv 2307.00714, 2410.23657) reports high recall and poor precision with no path to closing the gap by adding heuristics, so this engine is climbing a curve that has already flattened. Narrow the promise instead of extending the code: keep a roughly 60-line pass covering the sensitive-key list, = and : value spans, URL userinfo, and one entropy rule; drop key_segments, looks_like_relative_path, looks_like_schemeless_url, plausible_extension, authorization_value_span, and the dual-category entropy scoring. Then state in the README what redaction is -- best-effort hygiene, not a security boundary -- which is already how the --stderr-file help text frames it. Keep --evidence, --cmd, and --stderr-file: AGENTS.md instructs agents to use them.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Redaction is under ~80 lines and every retained rule has a test
- [ ] #2 key_segments, looks_like_relative_path, looks_like_schemeless_url, plausible_extension, authorization_value_span and the dual-category entropy scoring are gone
- [ ] #3 README and --stderr-file help state redaction is best-effort hygiene, not a security boundary
- [ ] #4 The evidence flags themselves are retained; only the detection heuristics shrink
- [ ] #5 Any redaction cases dropped are listed in the CHANGELOG so the behavior change is explicit rather than silent
- [ ] #6 The hook-filed stderr path (hook.rs:110) still runs the shrunk pass and keeps at least one black-box test; the ~10 CLI tests pinning deleted heuristics are removed or rewritten deliberately, not left asserting behavior that no longer exists
<!-- AC:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-08-06 13:34
---
Review 2026-08-06: description corrected -- the original claimed one unit test (there are 16 black-box redaction tests in tests/cli.rs) and 15 percent of src (actual: 418/3882 = 10.8 percent), and omitted that hook.rs:110 shares the redaction pass. External claims verified: secretscan 0.2.2 / 2826 downloads / last release 2025-07-30; arxiv 2307.00714 and 2410.23657 both support the flattened-curve framing.
---
<!-- COMMENTS:END -->
