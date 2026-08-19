---
id: TASK-31
title: Close exit-contract escapes in argument and output handling
status: Done
assignee: []
created_date: '2026-08-17 19:26'
updated_date: '2026-08-19 15:40'
labels:
  - bug
dependencies: []
type: bug
ordinal: 37000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Parent group for two confirmed defects found in a 2026-08-17 bug hunt, both of which let blotter terminate outside its published error contract. The contract says every failure is one structured envelope on stderr plus an exit code listed in ERROR_CONTRACT (src/error.rs:26), and every success is one envelope on stdout. Today an oversized --since value aborts the process with a raw Rust panic and exit 134, and any stdout write failure is discarded so the process claims success with no envelope at all. Both break the same promise an agent consumer relies on: the exit code and the stream contents always agree on what happened. Neither has test coverage. The repository gate was green before and after the hunt (173 tests, Clippy clean), so both are pre-existing.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every reachable failure path exits with a code listed in ERROR_CONTRACT; no path aborts or exits 0 after failing
- [ ] #2 Every non-zero exit is accompanied by a well-formed error envelope on stderr; stdout carries at most one success envelope
- [ ] #3 Each child ships a regression test that fails against the current binary
- [ ] #4 cargo test --all-features, clippy -D warnings, and cargo fmt --check all pass
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Closed by the 2026-08-19 batch PR. A dedicated exit-contract sweep looked for every reachable path that could terminate outside ERROR_CONTRACT: panics/unwrap/expect/indexing/overflow reachable from a CLI invocation (release builds with panic = abort, so a panic is exit 134 with no envelope), clap parse and --help/--version exits, std::process::exit and early returns bypassing the envelope writer, IO failures on stderr/log/backup/sidecar/temp/mkdir, paths that write stdout then fail, non-UTF-8 arguments and file contents, and hostile BLOTTER_NOW/BLOTTER_FILE/BLOTTER_AGENT values. Every candidate was run against the release binary. Six escapes were found beyond the two children already fixed. Five are fixed here: the log path is validated as a regular file on the open handle before the lock (a FIFO used to hang forever with no exit code and no bytes on either stream; an endless device grew the read buffer to ~3 GB and aborted 134), add -/dogear - cap raw stdin at 1 MiB, sweep --registry reports 66 instead of 74, export writes through the shared stdout writer so a broken pipe is not suppressed, non-UTF-8 BLOTTER_AGENT is 78 instead of silently discarded, and list/export validate --since before opening the log. Each ships a regression test that fails against the pre-fix binary. Two behaviour changes are deliberate and in the changelog: BLOTTER_FILE=/dev/null goes from exit 0 to 65, and --file <directory> goes from 74 to 65, matching what --stderr-file already did.
<!-- SECTION:NOTES:END -->
