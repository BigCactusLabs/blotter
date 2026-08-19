---
id: TASK-31.2
title: Surface stdout write failures instead of discarding them
status: Done
assignee: []
created_date: '2026-08-17 19:28'
updated_date: '2026-08-18 20:48'
labels:
  - bug
dependencies: []
modified_files:
  - src/output.rs
  - src/commands/list.rs
  - src/commands/digest.rs
  - tests/cli.rs
parent_task_id: TASK-31
priority: medium
type: bug
ordinal: 39000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Every stdout writer in the tool wraps the locked handle in a BufWriter that is never explicitly flushed: output::write_success (src/output.rs:65), list::write_markdown (src/commands/list.rs:105), and digest::write_markdown (src/commands/digest.rs:175). For any envelope small enough to stay in the 8 KiB buffer, the writes and the trailing writeln all return Ok without touching the file descriptor. The real write happens in BufWriter::drop, which flushes and discards the error. The command then reports Ok(0).

Confirmed against the release binary by duping stdout from a read-only descriptor so writes fail EBADF:

  blotter list                  0</dev/null 1>&0   exit 0, no output
  blotter list --format md      0</dev/null 1>&0   exit 0, no output
  blotter digest --format md    0</dev/null 1>&0   exit 0, no output
  blotter schema                0</dev/null 1>&0   exit 0, no output
  blotter add TEXT --file x     0</dev/null 1>&0   exit 0, no output, record still appended

Controls on the same descriptor: /bin/echo exits 1, cat exits 1, python3 exits 120. Only blotter reports success.

The worst case is add. The record lands in the log, the envelope is lost, and the caller sees exit 0 with empty stdout, so it cannot learn the ID and cannot distinguish success from silence. A retry files a near-duplicate cut under a fresh timestamp. This is the failure class io_error / exit 74 exists for, and it also breaks the stdout is data only, one envelope invariant.

Fix is an explicit flush at the end of each writer, propagated through the existing io::Result path. In the two write_markdown functions the flush belongs inside the closure that already collects io::Result. write_error (src/output.rs:90) keeps its let _ deliberately: there is no way to report a failure to report a failure.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 output::write_success flushes explicitly and propagates the flush error, so a failing stdout yields io_error and exit 74
- [ ] #2 list::write_markdown and digest::write_markdown flush explicitly and propagate the error through their existing io::Result path
- [ ] #3 Regression test: with stdout duped from a read-only descriptor, list, list --format md, digest --format md, schema, and add all exit 74 and emit an error envelope on stderr
- [ ] #4 The add regression case asserts the mismatch is gone: the command no longer reports exit 0 after appending a record it could not report
- [ ] #5 write_error keeps ignoring its own stderr failures, with the reason stated in a comment
- [ ] #6 Successful runs produce byte-identical stdout to today for a fixed BLOTTER_NOW
<!-- AC:END -->
