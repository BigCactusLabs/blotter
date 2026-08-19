---
id: TASK-39
title: 'Hook chain-shape gate: skip compound commands in claude-code auto-capture'
status: Done
assignee: []
created_date: '2026-08-19 04:44'
updated_date: '2026-08-19 13:40'
labels: []
dependencies: []
ordinal: 47000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Design pass complete (2026-08-19): add a structural eligibility gate to hook exec claude-code that skips commands containing an unquoted chain/substitution operator (&&, ||, ;, |, newline, $(, backtick). Evidence: 25/25 auto-captures were chains; probe gate never fired; fingerprint dedup collapses nothing. Lands as amendment r29, additive, contract 5. Follow-up idea (dogeared): store error note as cut text, command as evidence.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Design pass + implementation complete 2026-08-19. PR #1 merged as 20d40ac (squash). Review follow-up 7d4bfd3: the scan now skips when it ends inside a quote (unterminated quote or trailing backslash in a double-quoted span) — r29 requires an ambiguous scan to resolve toward skipping. Docs updated with the merge: README fourth noise guard, CHANGELOG Unreleased entry, r29 prose, schema gate text. All four gates green plus CI (Gates stable, MSRV 1.89); 251 tests. Follow-up transform idea dogeared as bl_9326a63aaff6. Remote branch task-39-hook-chain-shape-gate could not be deleted: the protect-history ruleset applies a deletion rule to ~ALL branches.
<!-- SECTION:NOTES:END -->
