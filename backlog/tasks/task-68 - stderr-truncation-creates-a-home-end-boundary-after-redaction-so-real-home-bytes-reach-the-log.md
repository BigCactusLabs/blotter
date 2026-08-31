---
id: TASK-68
title: >-
  stderr truncation creates a home end boundary after redaction, so real home
  bytes reach the log
status: To Do
assignee: []
created_date: '2026-08-31 14:40'
labels: []
dependencies: []
ordinal: 78000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found by bug hunt (2026-08-31), reproduced on 0.15.0.

--stderr-file evidence is redacted first and capped to 4096 bytes second (redact_and_truncate, src/commands/add.rs:163, applied at src/commands/add.rs:99). The exact-home rule accepts end of input as an end boundary (src/redact.rs:45-47, mirrored at src/commands/doctor.rs:611-613). The 4096-byte cut therefore creates a boundary that did not exist when the redactor ran, and promotes a home occurrence the redactor correctly declined -- r42's deliberate 'x/Users/alice2' class, where the byte after the home blocks the match and the byte before it blocks the generic prefix -- into one that now matches. The only redaction pass is already over, so nothing rewrites it.

Repro with HOME=/Users/alice: write ('z' * (4096 - len(HOME))) + HOME + 'XXXX' to a file, then blotter add "x" --stderr-file thatfile. Stored evidence.stderr ends '.../Users/alice'; doctor --leaks exits 1 with 'line 1 contains home path'. Both spellings reproduce: the slash form and the dash-encoded form. Control: the same bytes in a file short enough to skip truncation store verbatim and pass the gate at exit 0, which is r42's documented behaviour.

Fails unsafe, unlike TASK-63. Real home bytes reach an append-only log, and a leak finding is not fixable (src/commands/doctor.rs:418 lists only torn_line, malformed, conflict_marker), so there is no repair path short of hand-editing. doctor --leaks is the r22 CI gate, so blotter fails its own gate on a file blotter wrote -- the r30 defect class, but with the writer at fault, not the gate. TASK-63 anticipated only the marker-splitting consequence of truncate-after-redact; this is the boundary-creating one, and r42's own rule already states that an exact home ends at the end of input.

Candidate fix, verified by hand but not implemented: re-run rewrite_home_paths after the cap. It can only shorten, so the 4096-byte ceiling holds; r25's redact-then-truncate ordering is untouched, so an entropy token is still seen whole by the secret pass; and it cannot split a marker. Feeding the post-truncation shape back through add stores 'zzz~' and the gate passes at exit 0. Needs a design pass on whether the write side or the boundary rule moves. Evidence is not part of the ID (design doc line 42), so unlike TASK-62 no record ID moves either way.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Stored evidence.stderr never ends in an exact-home match, in either spelling, after the 4096-byte cap
- [ ] #2 doctor --leaks exits 0 on any log written by add --stderr-file
- [ ] #3 The 4096-byte ceiling still holds and the secret-marker backtrack of TASK-63 is unchanged
- [ ] #4 Regression test lands in tests/cli/redaction.rs beside stderr_truncation_never_splits_the_secret_marker, covering the slash and dash spellings
<!-- AC:END -->
