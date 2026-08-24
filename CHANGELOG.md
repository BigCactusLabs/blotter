# Changelog

## [Unreleased]

### Removed

- The Claude Code auto-capture lane is retired (design doc r32). `blotter hook install claude-code` is removed and `blotter hook exec claude-code` no longer files cuts. Over roughly ten days of dogfooding the lane filed 27 captures and nothing ever read them: r17 hid them from `list`, `triage`, `digest`, `verify`, `sweep`, and `export` by default, and `retrospect` — the one command that opts in — mined nothing from them, because r29's own measurement already showed the captures were one-shot novelty rather than repetition. What the lane stored was a failed command line with no statement of why the failure mattered, and no gate can turn a non-zero exit into that claim. Retiring it also closes the write path most likely to store an unredacted local path or secret, the full fold taken inside the exclusive lock on every failed Bash call in a host session, and the settings entry naming an absolute executable path that drifts when the binary moves.

  Read-side `auto` filtering is unchanged: the log is append-only, so `is_auto_capture`, the default exclusion on those six commands, `--include-auto`, `--tag auto` implying `--include-auto` on `list`, and `retrospect`'s inverted default all stay exactly as written and now describe stored history. `source` is still readable and opaque; no command writes it.

  `blotter hook exec claude-code` survives as a no-op receiver, because a settings file installed against an older binary keeps firing that exact command line and a clap rejection there would put a usage error and a non-zero exit into a host session's hook channel — which the lane's fail-open rule forbids. It reads and discards stdin under the same 1 MiB bound, resolves no clock, opens no log, takes no lock, resolves no agent, writes nothing, and always exits 0 with empty stdout; `BLOTTER_HOOK_EXPLAIN=1` writes one stderr line naming the retirement. `hook install` is the opposite case — a deliberate operator invocation whose purpose was to create the installation being retired — so it is removed outright and exits 2. Delete the `hooks.PostToolUseFailure` entry naming `blotter hook exec claude-code` from your Claude Code settings; blotter no longer writes another program's configuration at all.

### Fixed

- `doctor --leaks` no longer reports a leak on a bare redaction marker standing as the username component of a generic home prefix (TASK-56). The redactor replaces a matched home prefix with `~` and keeps the surrounding bytes, so a generic prefix whose own username component is empty survives with the marker behind it: with `$HOME=/Users/alice`, the evidence `/Users//Users/alice/x` stores as `/Users/~/x` and `-Users-/Users/alice/x` stores as `-Users-~/x`. The scanner read that `~` as a first component and flagged a line holding no home bytes. A generic home prefix whose username component is exactly `~` is now not a leak, in both the slash and dash-encoded forms; a component that merely starts with `~`, such as `/Users/~abc`, is a real directory name and still reports. Detection of the exact current home, its dash-encoded form, and every other generic-prefix component is unchanged, and `doctor` stays diagnose-only for `--leaks`. Because `doctor --leaks` is a CI gate, a log holding one of these two shapes and no other finding now passes at exit 0 where it previously failed at exit 1; no stored bytes change. One residual of the same class survives, tracked as TASK-58, the entropy-marker residual: the secret pass runs after home rewriting and can replace the bytes right behind an emitted marker, so with `$HOME=/Users/alice` the evidence `/Users//Users/alice/<32-char high-entropy token>` stores as `/Users/~<redacted>`, whose component is not the bare marker and still flags. Design doc r39.

- Evidence redaction now catches a home form nested in the tail of a token whose head it already redacted, so blotter's own write output no longer trips its own `doctor --leaks` gate on these shapes (TASK-55; the residual scanner false positive on the redaction marker itself is fixed above, TASK-56). With `$HOME=/Users/alice`, the evidence `/Users/alice/x/-Users-bob-y` used to store as `~/x/-Users-bob-y` — the redactor skipped to the token's end after a match while the leak scanner scans every position, and the dash-form boundary rule accepts a preceding `/` — so doctor flagged a line the redactor had stored as clean; a nested exact home (`/Users/alice/backup/Users/alice`) survived the same way. The redactor now resumes scanning immediately after each replaced prefix under unchanged boundary rules, storing `~/x/~-y` and `~/backup~`, which doctor passes. The scanner side is untouched, so stored history reads exactly as before. One compatibility consequence: redaction runs before the identity hash, so a raw input holding a nested home form now computes a different record ID than it did before this change. Design doc r38.

- Evidence redaction and `doctor --leaks` now recognize colon as a home-path list boundary, so every home entry in a colon-separated Unix path list (`PATH=/Users/alice/bin:/Users/bob/bin`) is redacted and flagged, not just the first. The r23 home-path boundary class previously stopped at dash, slash, or evidence delimiter, so the scan ran past a colon into the next entry and left every home past the first unredacted and unflagged. The general evidence-delimiter class used for secret-value spans and URL parsing is unchanged, so `key:value` assignments and `scheme://host` URLs still parse as before; the new terminator is scoped to home-path matching alone. Two compatibility consequences: `doctor --leaks` is a CI gate, so a log holding such a list can now fail at exit 1 with no new bytes, and because redaction runs before the identity hash, a cut or dogear whose text holds such a list can now compute a different record ID than the same raw text did before this change. Stored history is never rewritten. Design doc r37.

- `doctor` now diagnoses a resolution amendment that has no base resolve. An amend whose `id` names a known record but for which no non-amend resolve exists anywhere in the log is reported as an `orphan_resolve` finding, one per orphan line, joining the existing record-missing class under the same kind. The fold is unchanged — it still warns `skipped N orphan resolve` once per ID and leaves the record open — so this closes a gap where the fold flagged bytes that `doctor` called healthy. Record-missing orphans stay harmless because `merge=union` can order a resolve ahead of its record; a base-missing amend cannot arise that way, because a union merge never drops a line, so it implies truncation or hand-editing. `orphan_resolve` stays diagnose-only in both classes and `--fix` still never touches it. Compatibility break: a log carrying such an amend previously passed `doctor` at exit 0 and now fails at exit 1, including where `doctor` is a CI gate; appending the missing base resolve clears the finding. Design doc r36.

- Two input paths that could never work are now `invalid_input` (exit 65) instead of the generic `io_error` (74). A log path naming a directory answered 65 on the read commands but 74 on `add` and every other writer: a mutation opens read+append, so the OS rejects a directory at the `open` call, before the opened-handle regular-file check r31 added can run — one input answered two ways depending on the lane that reached it. And a `sweep --registry` file is now 65 when its bytes are not UTF-8, or when the path names a directory, matching r31's ruling that a missing registry is `not_found` (66) because `--registry` names a file as explicitly as `--file` does. Every other I/O mapping is unchanged; an unreadable-by-permission registry stays `permission_denied` (77). The directory answers are Unix-only by nature — Windows reports a directory opened for write as `PermissionDenied`, not `IsADirectory` (rust-lang/rust#134893). Design doc r35.

- An `add` to a log holding exactly one newline no longer renders it permanently unhealthy (TASK-42). The tear-heal predicate treated `"\n"` as a terminated non-empty file and appended without a separator, and `scan` then counted the leading empty segment as a malformed line. A leading empty segment is now a terminator, never a physical line — extending r26's rule, encoded by the appender's tear-heal predicate and `scan`'s skip of an empty first segment alike. `archive` follows the same contract: the segment's byte survives a swap verbatim but its `kept` count no longer includes it. An empty segment after a record is still malformed. Design doc r33.

- The lock retry budget actually spans its published five-second bound (TASK-43). The path-identity-mismatch branch retried without sleeping, so a peer looping copy-and-swap could exhaust all 50 attempts in microseconds and the caller was told to retry having waited for nothing; every non-returning iteration now pays the 100 ms delay. And when the log has vanished by exhaustion, the error is `not_found` (exit 66) instead of `lock_timeout` (exit 75) blaming contention that never happened; on a non-explicit default path that answer folds into the empty-log case (exit 0), as discovery has always answered a log that never existed. Design doc r33.

- A relative log path crossing a directory symlink resolves to the file the OS resolves (TASK-44). `..` was folded textually, so `--file link/../cuts.jsonl` where `link` points elsewhere named a different file than every other tool opens — and that spelling became the locked, appended, backed-up, and reported path. A path carrying `..` now canonicalizes its longest existing ancestor and folds only the not-yet-existing tail lexically; the final component stays unresolved, and a path with no `..` keeps its lexical spelling. Design doc r33.

- An aborted `doctor --fix` no longer leaves sidecars that sabotage the retry (TASK-41). A failure after the backup was written left a `.bak-<ts>` claiming a repair that never happened, and the retry then failed on the leftover instead of the real cause. Every sidecar the aborted repair created is now removed — a pre-existing quarantine is truncated back, never deleted — the pre-existing-backup error names the leftover file in its `suggested_fix`, and the post-fix diagnosis inspects the in-memory repaired bytes, since after the swap the held lock covers an unlinked inode and a fresh read of the path was outside it. Design doc r33.

- `blotter` no longer hangs or aborts when the log path is not a regular file (TASK-31). A FIFO used to block forever in `File::open` — no exit code, no bytes on either stream — and a `BLOTTER_FILE` pointed at an endless device grew the read buffer until the process aborted with no envelope. The path is now validated on the open handle, before the lock and before any read, and rejected with `invalid_input` (exit 65); both log opens also set `O_NONBLOCK` on Unix so the open itself cannot block. Two behaviour changes follow: `BLOTTER_FILE=/dev/null` exited 0 with an empty envelope and is now exit 65, and `--file` pointed at a directory was `io_error` (exit 74) and is now the same `invalid_input` (exit 65) that `--stderr-file` already returns for a directory. Design doc r31.

- The fold picked the last amend in file order instead of the latest amend by timestamp (TASK-50), contradicting r13 and r16. Because `.gitattributes` recommends `merge=union` on the log, a union merge could concatenate an older amend last, materialize the wrong resolution, and flip `verify` between exit 0 and exit 1 on the same records in a different byte order. The amend with the latest timestamp now wins; two amends sharing a timestamp still resolve to the last one in file order, so behaviour under a frozen `BLOTTER_NOW` is unchanged. Base resolve selection is untouched: the first non-amend resolve remains the base. `resolve` reports its result from the fold that made the append decision, so it applies the same rule to the event it appends: an amend backdated behind a stored amend does not win, and the envelope reports the stored winner rather than a note no read command agrees with. `--dry-run` predicts through the identical rule, so a plan cannot promise a resolution the apply would not produce. Design doc r31.

- `add` and `dogear` no longer store a home path in `cwd` (TASK-51). The stored cwd applied only an exact `$HOME` prefix match, so a path under a different user's home, or under a dash-encoded harness scratchpad slug such as `-Users-<user>-<repo>`, was written verbatim — and `doctor --leaks` then flagged bytes that `--fix` cannot repair, leaving the log permanently unhealthy. The cwd now goes through the same whole-string home-path scanner the evidence fields use, extracted to `src/redact.rs` so `store` and the text lanes share one implementation. `compute_id` ignores `cwd`, so record IDs and dedupe do not move, and no stored record is rewritten. Design doc r30.

- `blotter add -` and `blotter dogear -` no longer buffer unbounded stdin (TASK-31). An endless producer grew the allocation until it failed, which under `panic = "abort"` is an abort with no envelope. The raw read is capped at 1 MiB, matching `--stderr-file` and the hook payload, and `schema` publishes the cap on both positionals. The budget is on bytes read, so it is measured before the trailing newline is trimmed: trimming first left a hole exactly at the boundary, where a stream of the full limit followed by a newline and then more data filled the reader to its cap, lost the newline to the trim, and was accepted while everything past it was silently discarded. Oversized input that redacts below the 10000-byte text limit is still accepted, as r25 requires. Design doc r31.

- `sweep --registry` reports a missing registry file as `not_found` (exit 66) instead of `io_error` (exit 74), matching `--file` and `--stderr-file`; permission failures still exit 77 (TASK-31). Design doc r31.

- `export --format otlp-json` writes stdout through the shared writer, so a broken pipe or a failed write is reported as a structured error instead of being suppressed — completing TASK-31.2, which had left this one raw-stdout path out. Design doc r31.

- A non-UTF-8 `BLOTTER_AGENT` is a `config_error` (exit 78) instead of being silently discarded and the record filed under a detected or default agent, matching how `BLOTTER_NOW` already handles the same case (TASK-31). Design doc r31.

- `list` and `export` validate `--since` before opening the log, so an invalid value exits 2 in every command that accepts the flag; `list` also checks its `--severity`/`--kind` conflict first. This is r28's stated principle, which `digest`, `archive`, and `sweep` already followed (TASK-31). Design doc r31.

- `hook exec` no longer appends a second line carrying an existing cut ID. Only reachable under a frozen clock, where a resolved command replayed at the same instant recomputes the same ID: the append is skipped and `BLOTTER_HOOK_EXPLAIN=1` names the reason. A resolved command filed at a later instant still refiles, as r8 and r25 intend. Design doc r31.

### Changed

- Every authored free-text field is now redacted at write time (TASK-48). `dogear --evidence` and a resolution's `--note` (with or without `--amend`) join the surface `add`'s evidence lanes already used, going through the same pass: the exact `$HOME` rule, the generic `/Users/<user>` and `/home/<user>` rules, the dash-encoded harness slug forms, then the span-based secret pass. This supersedes r25's named deferral of resolution notes and closes r22's enumeration with one rule instead of a list. The behaviour break is real but narrow: a dogear evidence value or a resolution note quoting an absolute home path now stores the rewritten spelling, so a caller that reads the note back gets `~/repo/src` where it previously got the raw path. Nothing else moves — `compute_dogear_id` never hashed `evidence`, and a resolution's `id` is the target record's ID rather than a hash over the note, so IDs, dedupe, determinism, and the r13 amend fold are untouched, and the redaction runs once before the critical section so the base path, the amend path, and `--dry-run` all report the bytes the append stores. No bounds are added; both fields stay unbounded. The reason is r30's: `doctor --leaks` is a diagnose-only CI gate whose only repair path for an appended line is resolve-then-archive, which relocates the bytes to a committed sidecar rather than removing them, and a field the tool invites an operator to fill should not be the field that trips the tool's own gate. Stored history is never rewritten. Design doc r34.

- `archive` and `doctor --fix` each parse the log once instead of twice (TASK-46). `plan_archive` folded the bytes and then rescanned the same bytes only to recover line numbers and per-ID line groupings the fold had already walked past; the fold now carries the `(line, id, ts)` tuple of every record-carrying physical line, and `archive` groups those tuples instead. `doctor --fix` re-inspected the whole repaired log to report post-fix findings; it now derives that report from the pre-fix findings, since a repair only drops whole quarantined lines and every quarantined line is a scan error that contributed no record, duplicate payload, or resolve target. Median CPU at 100k records falls 1.52x for `archive` (0.41 s → 0.27 s dry-run, 0.40 s → 0.28 s applying) and 2.00x for `doctor --fix` (0.38 s → 0.19 s); peak RSS falls 20% for `doctor --fix` (80,064 → 64,080 KiB) and rises about 3% for `archive` (195,776 → 201,776 KiB), which is the retained line tuples. Verified byte-identical against the pre-change binary — exit code, stdout, stderr, and every backup, archive, and quarantine file — over 234 invocation pairs spanning 26 log shapes including the 10k and 100k fixtures. The line tuples are opt-in through `fold_bytes_with_lines`, so no other command pays for them, and no fold verdict moves. Measurements and method are recorded in `docs/research/2026-08-18-scale-baselines.md`. No envelope, ordering, or exit-code changes.
- `triage`, `digest`, `verify`, and `retrospect` bound the candidate prefilter instead of scanning the whole candidate bitset once per representative (TASK-45). Three bounds, none of which touches the relation — `linked` is still the final pair predicate. A representative can never out-score the number of its own indexed tokens, so a threshold above that bound is skipped rather than counted and discarded, and when no scoring path can run the tag pool that would mask it is never built. Both callers already discard a prefix of the result — triage every candidate at or before its representative, verify everything up to the resolution — so the bit-parallel count now starts at that floor's word. And a tag carried by exactly one record across the analyzed population gets no posting set, the rule `by_token` already applied to unshared tokens; `TokenFrequencies` becomes `CorpusFrequencies` and counts tags too, because a tag shared by one `verify` anchor and one open cut must still count as shared.

  Release-mode output is byte-identical for all four commands on the 10k, 30k, 100k, and 300k fixtures — same clusters, candidates, recurrences, member order, counts, and exit codes. Median CPU falls 7.48x at 300k (18.33 s → 2.45 s), 3.66x at 100k, 2.06x at 30k, and 1.40x at 10k for `triage`, and 6.26x / 2.98x / 1.83x / 1.00x for `retrospect`. Growth across the 30x step from 10k to 300k drops from 262x to 49x. Peak memory falls with it rather than paying for it: `triage` 1.25x lower at 300k (1.81 → 1.45 GiB), because half the tag vocabulary on that fixture names a single record and each such tag had been costing a full 249,000-bit row. `retrospect`'s peak is unchanged at about 2.3 GiB, which is two folds of the log and not the index. Measurements, method, fixture hashes, the published budget above 10k, and the two levers deliberately not taken are in `docs/research/2026-08-18-scale-baselines.md`. No envelope, ordering, or exit-code changes.

- `verify` — and `retrospect`, which inherits its scan — replaces the anchor-by-open-cut recurrence comparison with triage's candidate index and bit-parallel prefilter, keeping `linked` as the final pair predicate (TASK-29). Release-mode output is byte-identical on the 10k, 100k, and 300k fixtures (same recurrences, member order, counts, and exit codes): the index is built over the open cuts while the token frequencies stay the open-plus-anchor counts `verify` already computed, so document-frequency rarity and every `linked` verdict are unchanged, and the post-resolution cutoff becomes a starting floor on the bitset walk rather than a second pass. Median CPU falls 6.88x at 300k (25.17 s → 3.66 s) and 1.74x at 100k (1.19 s → 0.69 s), and is unchanged at 10k. The index costs peak memory — 1.13x at 10k, 1.69x at 100k, 2.56x at 300k (0.88 → 2.20 GiB) — which is parity with what `triage` already needs on the same log rather than a new limit. Measurements, method, fixture hashes, and the reduction deliberately not taken are recorded in `docs/research/2026-08-18-scale-baselines.md`. No envelope, ordering, or exit-code changes.

- `add` and `dogear` fold only the records they need to dedupe against, instead of building and sorting a full list view and discarding it inside the exclusive lock (TASK-29). On a 100k-record log this cut CPU about 15% and peak memory about 38%, and shortened lock hold time by the same work. The records-only fold keeps the tag sort and dedup, because the duplicate branch returns the stored record straight into the response envelope.

- The release profile builds at `opt-level = 3` instead of `"z"`. Measured at 100k records, optimizing for size cost 2.15x CPU on `triage` (6.61 s vs 3.07 s), 1.76x on `list`, and 1.40x on `verify`, for 558 KiB of binary (988,016 → 1,546,336 bytes); peak RSS was unchanged. Analyzer CPU on large logs is the stated scale risk.

- The published record schema describes `cwd` as home-redacted rather than "absolute path otherwise", for both cut and dogear records.

- Documentation: the auto-exclusion invariant now names `export` alongside `list`, `triage`, `digest`, `verify`, and `sweep` (r28 scope) in `AGENTS.md` and both README enumerations; the raw-output invariant names `export --format otlp-json` beside `--format md`; the README's exit-1 summary includes `retrospect` for promotion candidates, matching `blotter schema` and r27; and the `AGENTS.md` layout map names `archive.rs`, `export.rs`, and `retrospect.rs` — `archive.rs` being the log-rewriting command the invariants already referenced but the map never located.

- Documentation: `AGENTS.md` no longer sends readers to `docs/superpowers/specs/` without warning. The directory is normally absent, because archiving the last spec empties it and git does not track an empty directory; current-release behaviour is the design doc's newest amendments plus `blotter schema`. The design doc's own status line now points at the last Amendments section instead of quoting a revision number, so it cannot go stale again.

- Tooling: `scripts/dev/gate-5x.sh` runs the five-times test gate AGENTS.md requires after a store or concurrency-adjacent change, keeping every run's full output so a failure that does not reproduce still names the test that failed.

- Tests: `readme_hook_prose_describes_every_published_hook_gate` (TASK-40) fails, naming the gate, when a `hook exec` gate published by `blotter schema` has no README prose, and derives the documented noise-guard count from the published gates. It would have caught r29 shipping while the paragraph still read "Three noise guards apply". Deliberately undocumented gates need an entry in one explicit allowlist, which is empty today.

- Tests: `tests/cli.rs` is split into `tests/cli/` subject modules under one integration-test binary (TASK-28.2). The 11,312-line single file was one append target, so parallel branches conflicted at its tail on every merge — the top cluster in `blotter triage`. The move is mechanical: the same 270 tests before and after, none removed, merged, renamed, or reworded, plus one new guard for 271 in total. `AGENTS.md` carries the module map and the placement rule, and `every_test_module_file_is_declared_in_main` fails if a module file is not declared in `main.rs`, which cargo would otherwise leave uncompiled with its tests silently never running.

- The 0.13.0 auto-capture spec (`2026-08-09-auto-capture-default-hidden-design.md`) moved from `docs/superpowers/specs/` to `docs/archive/`: 0.13.0 is no longer the current release, and `docs/superpowers/specs/` holds only specs for the current one. Its `Status:` line carries the archived date and a pointer to the design doc; the body is unchanged. Documentation only.

- `hook exec claude-code` gains a fourth noise guard (TASK-39): a failed command that is not a simple command is skipped instead of filed. The gate scans the raw bytes with single- and double-quote state and skips on an unquoted `&&`, `||`, `;`, `|`, newline, `$(`, or backtick, or when the scan ends inside a quote. The failed command becomes the cut's text verbatim, and a chain's non-zero exit names neither the failing step nor the friction, so the entry read as an unreadable one-liner; measured against this repository's own log, all 25 auto-captures filed to 2026-08-18 were chains, the r20 probe gate matched none of them, and fingerprint normalization collapsed nothing. It runs after the 500-byte gate and before the probe gate, does not parse the shell (bare `&`, heredocs, `$'...'`, and nested substitution are not recognized), and resolves an ambiguous scan toward skipping. Published in `schema` as `tool_input.command_shape`; skipping stays fail-open (stdout empty, exit 0) and `BLOTTER_HOOK_EXPLAIN=1` names the gate. Envelope `meta.contract` stays 5. Design doc r29.

## [0.15.0] - 2026-08-18

### Added

- `blotter export --format otlp-json` (TASK-25): a read-only bridge that emits the selected records as one raw OTLP 1.11.0 `LogsData` JSON line (lowerCamelCase fields, decimal-string `timeUnixNano`), with `eventName` `blotter.friction.reported` and `blotter.friction.*` attributes. Honors `--since` and the auto-capture exclusion; cuts of every status are exported, with the status mapped into the `blotter.friction.status` attribute (`open`/`resolved`/`dropped`) rather than selectable; never exports evidence, trace IDs, or a `schemaUrl`; deterministic sort and stable empty output. A record whose timestamp cannot be represented as an unsigned 64-bit nanosecond value rejects the whole export with `invalid_input` (exit 65), naming the record — no partial output. Design doc r28.

- `blotter retrospect` (TASK-23): a read-only promotion-mining pass over one log that turns chronic signal into typed candidates for a human to judge. It reuses triage's clustering and verify's recurrence rules unchanged, and types each open-cut cluster by evidence shape — half-or-more sharing one failing leading program becomes `wrapper_alias`, half-or-more tagged `docs`/`documentation` becomes `doc_repair` (wrapper wins when both match), everything else emits nothing. Every recurrence of count two or more becomes a `skill_candidate`. The envelope carries bounded evidence (at most 10 member texts, 5 resolution notes; never evidence `cmd`, `stderr`, or `note`), so the CLI never writes a doc, skill, or alias. Retrospect takes no window — chronic signal is long-horizon — and deliberately includes auto-captures, inverting r17 for this one command. Exit 1 with candidates, 0 without, matching triage. Envelope `meta.contract` stays 5. Design doc r27.

- `blotter archive --before <value>` trims closed history without breaking the journal (TASK-30). It removes only `bl_` record groups whose materialized state is resolved or dropped *and* whose every event predates the cutoff; open records, orphan resolves, malformed lines, unknown kinds, and legacy `pc_` records stay in the log verbatim. `--before` takes the same RFC3339-or-`Nd`/`Nh` grammar as `--since`, cutoff exclusive. Apply mode uses the copy-and-swap mechanic r15 established for `doctor --fix`: the original is kept as a timestamped backup, every removed physical line is written verbatim and newline-terminated in original order to `<log>.archive-<ts>.jsonl`, and the kept lines are atomically swapped in. The envelope carries `archived`/`kept` counts, `backup`, `archive_file`, and a paste-ready `restore_hint`. `--dry-run` plans under the shared lock and writes nothing. Nothing eligible means no rewrite, no files, `changed:false`, exit 0. The append-only invariant now names `archive` and `doctor --fix` as its only exceptions. Design doc r26.

- TASK-24: cut records gain an optional `source` provenance field, serialized only when present; the sole writer is `hook exec claude-code`, which stamps `"hook"`. `add` cannot set it, `compute_id` ignores it, no selector keys on it, and it propagates through `list`, `triage`/`digest` clusters, and `verify` recurrences. Stored history is unchanged and unknown stored values pass through opaquely. Design doc r24.

### Fixed

- `doctor --fix` no longer strands a symlinked log. The log was locked and read through the link, but the atomic swap renamed the temporary over the link pathname, replacing the link with a regular file and leaving the real target untouched. Final-component symlinks are now resolved before the backup, sidecar, and replacement paths are derived, so the swap lands on the target and the link survives; parent components keep their spelling so envelope paths are unchanged for regular files. `archive` uses the same resolution. Design doc r26.
- TASK-31.1: a `--since` duration whose hour count overflows the internal representation (for example `--since 99999999999999h`) no longer aborts the process; it reports the documented `invalid_argument` error (exit 2) like any other invalid `--since` value.
- TASK-31.2: stdout write failures are no longer silently discarded. Envelope and `--format md` output flush explicitly and report the documented `io_error` (exit 74) when stdout cannot be written, so a broken pipe or closed descriptor can no longer produce a successful exit with lost output. On Unix and redirected Windows handles, output goes through a duplicated file descriptor so suppressed errors surface; interactive Windows consoles keep the console-aware writer, preserving non-ASCII output.
- TASK-32: `list --limit 0` no longer emits the "no records matched" warning when the log has matching records; the empty-result warning now reflects the unfiltered total, not the truncated item count.
- TASK-36: new `add` and `dogear` text rewrites r22/r23 home paths before validation and identity hashing; hook machine-captured command text and `evidence.cmd` use full evidence redaction, so the ID and text-keyed open dedupe use the same sanitized command. Hook failure notes now retain 1024 bytes after redaction instead of 4096; user-supplied `add --stderr-file` evidence remains bounded at 4096. No envelope, selector, exit-code, or flag changes.
- Home-path leak detection and write-time evidence redaction now recognize the dash-encoded home slug that harness scratchpad and session paths embed, such as `/private/tmp/<session>/-Users-<name>-<repo>/...` (TASK-34). `doctor --leaks` reports it as a `leak` finding and `add`/`hook` evidence rewrites the slug prefix to `~`, matching the existing `/Users/<name>` and `/home/<name>` handling. No envelope, error-code, or contract changes.

### Changed

- Performance at scale (TASK-29.1/29.2/29.3): triage and digest replace their quadratic candidate scan with exact-normalized-title and tagged-pool indexing (7.5 s → ~0.18 s CPU at 10k records; identical outputs and exit codes), resolve materializes its response from the deciding fold instead of a second full read+fold, and fold ordering parses each timestamp once. All measured commands now run within a 500 ms CPU budget (met at the 200 ms stretch) at 10k records; baselines, budgets, and dispositions are recorded in docs/research/2026-08-18-scale-baselines.md. Triage/digest trade 2-4x peak memory at 10k for the speedup, depending on how much distinct vocabulary the record text carries; the index skips tokens that appear in only one candidate, which cannot affect any shared-token count, so index size tracks the shared vocabulary rather than growing with every new word. No envelope, ordering, or exit-code changes.

- Publish-gate run and follow-up scrub (TASK-35): with the TASK-34 scanner, `doctor --leaks` plus the private deny list reports fully healthy, and a tree-wide token grep is clean outside accepted upstream attribution. Sanctioned append-only exception (owner-approved, 2026-08-18): five log records rewritten to the redacted form the new write-time redaction produces (IDs recomputed, one resolve ref updated), and one hook failure-note that had captured pre-scrub backlog prose neutralized. Backlog prose, the design doc's worked examples and review-provenance lines, and test fixtures no longer carry private identifiers.
- One-time open-sourcing scrub of the tracked tree (TASK-33, PR #30): home-directory paths rewritten to `~/` in `.blotter.jsonl` and remaining docs, private identifiers redacted, and the tracked `_scratch/` drafts, the pre-fork remediation archive (`docs/archive/papercuts-remediation/`), and a dangling skill symlink removed. The dogfood log's content-derived cut IDs were recomputed where the scrub changed record text — a sanctioned one-time exception to append-only; `doctor` reports fully healthy. No code, envelope, or contract changes. History untouched; posture decided at publish time.

## [0.14.0] - 2026-08-18

Envelope `meta.contract` stays 5. Triage/verify linkage, raw `--format md` output, public-log storage, evidence hygiene, and flag-gated doctor inspection change observably; JSON envelope shapes, selectors, ordering, and exit codes are unchanged. Design doc r19–r22.

### Fixed

- `triage` (and `verify`/`digest`, which share its linkage) now clusters reworded repeats of the same friction (issue #22). Scoring drops tokens of two characters or fewer plus a fixed sixteen-word stopword list, then links on 80% overlap with the shorter token set or at least three shared locally rare tokens (document frequency ≤ `max(2, ceil(N/4))`, computed within the analyzed log — no new dependencies). The exact-title fast path, the tag gate, and the non-transitive representative clustering are unchanged. Design doc r19.
- `list --format md` no longer breaks the Markdown list on multi-line record text (issue #24). Every interpolated field — text, agent, tags, timestamps, and all resolution fields, which accept embedded newlines — is whitespace-collapsed per rendered line, in `list` and `digest` md output alike. Design doc r21.
- `list --format md` renders a resolved cut's resolution (issue #25): a nested sub-bullet with the resolve timestamp, agent, `--commit`/`--pr`/`--task` graduation provenance, and the note, all collapsed the same way. Design doc r21.

### Added

- `doctor --leaks` is a pre-push/CI gate for public logs: it scans every raw physical line for absolute home paths and reports diagnose-only `leak` findings. Repeat `--deny LITERAL` to block an additional literal substring; `--deny` requires `--leaks`, and both flags conflict with `doctor --fix` so the gate stays read-only. Per-repository deny-pattern configuration files are deliberately not added. Design doc r22.
- `hook exec claude-code` gains a third noise guard (issue #23): read-only probe commands (`grep`, `rg`, `ls`, `find`, `tail`, `head`, `cat`, `stat`, `test`, `[`, `which`, `curl`, `gh`) are skipped at write time, matched best-effort on the first program word after leading `VAR=value` assignments. Their non-zero exit is an expected answer, and dogfood logs showed them dominating the auto-capture lane. Published in `schema` as the `tool_input.command_program` gate; `BLOTTER_HOOK_EXPLAIN=1` names it. Design doc r20.
- The retention stance is written down (issue #23): append-only stays, no command trims the log in this release, the auto lane is bounded at the write side, and an archive/rotation command is deliberately deferred as backlog TASK-30. Design doc r20.

### Changed

- New cut and dogear records outside their discovered repository store `cwd` relative to `$HOME` as `~` or `~/…`; non-home paths remain absolute and existing records are unchanged. Evidence commands, stderr, and notes rewrite current and generic Unix home paths before the existing best-effort secret pass, so hook-filed failure notes receive the same hygiene. Design doc r22.

## [0.13.1] - 2026-08-11

Copy-only: envelope `meta.contract` stays 5 and no selector, fold, ordering, or exit code changes. Two published strings change; callers matching them by exact text need updating, but matching by numeric exit code and leading count is unaffected. Design doc r18.

### Fixed

- The global exit-code dictionary in `blotter schema` described exit 1 as `doctor findings`. `triage` (r7) and `verify` (r16) also exit 1, and both already published their own `exit_codes` entry. The global entry now reads `command findings: doctor unhealthy, triage clusters, or verify recurrences`.
- The auto-capture warning agrees in number: a count of 1 now reads `1 auto-captured record hidden`, not `1 auto-captured records hidden`. Every other count is unchanged.
- Documentation drift across the whole doc set, found by replaying every documented command against the 0.13.0 binary:
  - `README.md`'s opening example printed `"contract":4`; the current envelope is 5.
  - The README exit-code dictionary omitted exit 1 entirely.
  - `resolve --amend` replaces the materialized resolution rather than merging field by field, so amending with only `--note` drops a previously recorded `--pr`. The README described it as carrying "the corrected fields", which reads as a merge; it now shows the whole-resolution replacement and how to repeat fields you want to keep.
  - `sweep`'s directory form requires a git working tree and silently skips a non-git directory that holds a `.blotter.jsonl`, exiting 0 with an empty roll-up. The README now says so and names `totals.repos_swept` as the check.
  - The README claimed both global flags apply to every subcommand; `sweep` rejects `--file`.
  - `AGENTS.md` pinned the normative design doc at "through r14" while the file had reached r17. It no longer quotes a revision number, and points readers at the last amendment in the file instead.
  - The design doc's own status line said r16 while r17 was present.
  - Undocumented details now stated: `list --tag auto` implies `--include-auto`, `triage --min-count` must be at least 2, and `--stderr-file` follows a symlink to its target.

### Added

- A README `Doctor` section listing all nine finding kinds, which three `--fix` repairs, and what to do about the six it will not. Notably `id_conflict`, which this repository's own log carries twice from records filed before the `TASK-4` cut-ID change and which are correct history.
- `.gitattributes` with the `.blotter.jsonl merge=union` setting the README has always recommended to users but the repository itself did not apply.

### Changed

- The shipped `angle` implementation plan moved to `docs/archive/` with a header noting it shipped as `dogear`; its original wording is preserved. The blotter rename spec moved there too — its `Status:` line said "approved for planning" long after the rename shipped in 0.8.0, and its migration surface was removed in 0.9.0. `docs/superpowers/specs/` now holds only specs for the current release, and the design doc's r9 pointer to the rename spec follows it to its new path. `AGENTS.md` states which paths under `docs/` are contract, which are history, and when to archive a spec.

## [0.13.0] - 2026-08-09

Breaking: envelope `meta.contract` bumps 4 → 5. `list`, `triage`, `digest`, `verify`, and `sweep` exclude records tagged `auto` by default; pass `--include-auto` for the previous behaviour.

### Changed

- The five reporting commands now omit auto-captured records from their default reads and report how many records they hid, so machine-filed failed commands remain available as evidence without diluting hand-filed analysis. Design doc r17.
- Bump envelope `meta.contract` from 4 to 5 for the default-read behaviour change. Design doc r17.

### Added

- `--include-auto` restores auto-tagged records to `list`, `triage`, `digest`, `verify`, and `sweep`; `list --tag auto` implies it. Design doc r17.

## [0.12.0] - 2026-08-09

Additive: envelope `meta.contract` stays 4; existing logs, commands, and output shapes are unchanged.

### Added

- `verify` — a read-only recurrence check over one log. Each eligible resolved cut anchors later matching open cuts using the exact `triage` linkage rule; dogears, dropped resolutions, and empty normalized resolved titles are excluded. Results preserve resolution timestamp and optional task/PR/commit provenance, sort deterministically, and exit 1 when one or more recurrences are found. One open cut can recur against more than one resolved anchor. `schema` publishes the output and exit convention. Design doc r16.

## [0.11.0] - 2026-08-09

Additive: envelope `meta.contract` stays 4; existing logs, commands, and output shapes are unchanged apart from one new always-present field.

### Added

- `doctor --fix` — a bounded repair path for the three unreadable-line findings (`torn_line`, `malformed`, `conflict_marker`), each removed from a repaired copy and preserved verbatim in `<log>.quarantine.jsonl`. Every repair backs the original up to `<log>.bak-<timestamp>` (collision is `io_error` with the log untouched), fsyncs backup and quarantine, writes a same-directory temp file, and atomically renames it in; the envelope reports the post-fix diagnosis with a paste-ready restore hint. `--fix --dry-run` plans without writing. All other findings stay diagnose-only. `Finding` gains an always-serialized `fixable` field, and `schema` documents the destructive `fix` sub-entry. Lock acquisition now re-verifies path identity after locking, so writers serialized behind a repair land on the post-repair file instead of an orphaned inode. This is the append-only invariant's sole exception; design doc r15.

### Changed

- `hook exec claude-code` skips a failed command longer than 500 UTF-8 bytes instead of filing it. The command becomes the cut's text verbatim, so a long debugging one-liner produced an entry that diluted the log rather than describing friction. `schema` publishes the new gate as `tool_input.command_bytes`; skipping stays fail-open (stdout empty, exit 0) and `BLOTTER_HOOK_EXPLAIN=1` names the gate. Design doc r14.

### Fixed

- Documentation drift that made the design doc contradict the code: the r6 amendment's pre-rename `pc_`/`pc1` dogear identity is now marked superseded inline (current code emits `bl_` and hashes `bl1`), r10's pointer to that text named r7 instead of r6, and the doc's status line said r12. `AGENTS.md` now states that amendments accumulate and the newest wins, and that `CLAUDE.md` is a symlink to it.

## [0.10.0] - 2026-08-08

Additive only: envelope `meta.contract` stays 4 and every existing log, command, and output shape is unchanged.

### Added

- `digest` — a read-only periodic friction report over one log: chronic clusters (the triage analysis at min-count 2), open cuts filed inside the `--since` window grouped by tag, and all open dogears. `--since` defaults to `7d` and accepts a full RFC3339 timestamp or an `Nd`/`Nh` duration. `--format md` emits raw markdown and joins `list` as a raw-output exception; empty reports are exit 0.
- `sweep` — a read-only roll-up across several repositories. Paths are repository directories or direct JSONL logs; `--registry FILE` reads a user-owned list of paths (one per line, `#` comments ignored, relative paths resolved from the registry's directory). `blotter` never creates or discovers a registry. Each log is read under its own shared lock, one at a time. `BLOTTER_FILE` is ignored and the global `--file` flag is rejected. A locked or unreadable repository becomes a skip warning with exit 0 and a `totals.repos_skipped` count — a deliberate, sweep-scoped deviation from the exit-75 lock-timeout rule.
- `resolve --amend` — correctable resolutions. An amend appends a second resolve event instead of rewriting anything: the first non-amend resolve stays the base, the latest amend supplies the materialized user-set fields, and `resolution.amended` becomes `true`. `--amend` requires at least one resolution field and requires every named record to be resolved already. Orphan amends warn and do not materialize. Legacy logs fold byte-identically.
- `BLOTTER_HOOK_EXPLAIN=1` — opts `hook exec claude-code` into one best-effort stderr line naming the gate it stopped at, the duplicate cut it found, an unresolvable clock, or the id it filed. stdout stays empty and the exit code stays 0. `schema` now publishes the hook payload contract (read fields, gates, 1 MiB stdin cap) and this variable.

### Fixed

- `hook install` preserves the existing key order of `settings.json` instead of rewriting it sorted (`serde_json/preserve_order`). The crate deliberately allows `clippy::result_large_err`: the resulting `IndexMap` grows `serde_json::Value` past the lint threshold, and boxing 53 `AppResult` signatures is not worth it for a short-lived CLI.

## [0.9.0] - 2026-08-06

### Changed (breaking)

- Remove the temporary pre-rename migration surface: `PAPERCUTS_*` variables no longer produce warnings, `.papercuts.jsonl` is not probed, and `doctor` no longer emits `legacy_records` or recomputes the pre-0.8.0 cut-ID format. Existing `pc_` records remain opaque data that folds, lists, and resolves by explicit prefix.
- The historical migration was the manual `mv .papercuts.jsonl .blotter.jsonl` rename (plus matching `.gitignore`/`.gitattributes` edits). In 0.8.0, `blotter doctor` reported `legacy_records` as informational and not affecting health or exit codes; users should already have completed that guidance before this release.
- Store the sorted, deduplicated tag set in both cut and dogear records, and normalize duplicate tags while folding old records, so stored tags now agree with their identity hash.
- Make every `resolve` response use `{changed,records:[...]}`, including a one-ID invocation; consumers no longer branch on `record` versus `records`.
- Remove stored `repo` fields. New records use repository-relative `cwd` when their cwd is inside the discovered repository; global logs and hook payloads outside that repository retain absolute cwd. Existing logs with absolute cwd and `repo` fields still fold and resolve.
- Make `schema` skip `BLOTTER_NOW` parsing. `BLOTTER_NOW=invalid blotter schema` now exits 0 with its contract envelope instead of exit 78 `config_error`; other commands retain clock validation.
- Bump the crate to 0.9.0 and envelope `meta.contract` from 3 to 4.
- Remove the unusable `codex` hook target until Codex exposes shell exit status (openai/codex#21753 remains open). `hook install codex` and `hook exec codex` are no longer accepted.

### Redaction narrowed (TASK-17)

- Redaction is now a best-effort hygiene pass, not a security boundary. It keeps direct sensitive-key `=`/`:` values, HTTP(S) URL userinfo, and one mixed-case-and-digit entropy rule.
- Dropped camelCase and acronym key-segment inference; only the documented sensitive-key list is scanned.
- Dropped per-scheme authorization parsing. An `authorization` assignment still covers the token after the scheme word, so `Authorization: Bearer <credential>` redacts the credential, not just the scheme.
- Dropped `*_file`/`*_path` handling, space-separated CLI option values, and structural path, URL, extension, and schemeless-host exceptions.
- Dropped escaped-quote and fullwidth-separator parsing. Entropy redaction no longer covers long single-category tokens.

### Changed

- `triage` clusters on direct similarity to a stable representative instead of a transitive union, and each cluster reports a new `occurrences` field: how many open cuts share the normalized title of the cluster's displayed `text`. Cuts with identical non-empty normalized titles now always link, regardless of tags. Same-input triage output and exit codes can differ from 0.8.0.
- A file ending in one trailing newline is a terminated log, not a malformed final line: a file holding only `"\n"` folds with no warnings. A blank line following a record is still malformed.
- `hook install` reports its outcome through `meta.warnings`: `hook created`, `hook amended`, or `dry run; hook would be <action>`.
- Release builds abort on panic (`panic = "abort"`): a hypothetical panic now terminates via SIGABRT instead of exit 101. The release profile also enables LTO, single codegen unit, and symbol stripping (binary: 2.1 MB → ~0.85 MB).
- The published crate no longer packages `tests/**`.

## [0.8.0] - 2026-08-05

### Changed (breaking)

- Frame cut identity per tag, matching the dogear scheme: the digest now covers a `bl1` version literal, the `cut` kind, `ts`, `agent`, `text`, `severity`, a decimal tag count, and each sorted-unique tag as its own length-prefixed field. This closes the tag-boundary collision (`["a","b"]` vs `["a,b"]`) that the r7 amendment deliberately deferred, and the `cut` literal supplies domain separation from dogears. IDs stay `bl_` plus 12 hex (48 bits), so cut and dogear ID widths remain provably disjoint.
- Duplicate tags no longer perturb cut identity — tags are deduplicated before hashing, as they already were for dogears.
- The same cut text filed under different tags now yields different IDs, so it is filed as a separate cut rather than deduplicated.
- Bump envelope `meta.contract` from 2 to 3.

### Changed

- `doctor` recomputes `bl_` cut IDs with the new scheme, then falls back to the frozen comma-joined v1 recomputation. A record matching the old scheme counts toward the informational `legacy_records` total instead of raising an `id_conflict`; only IDs matching neither scheme are conflicts. Exit codes are unaffected.

### Migration

- Existing `bl_` cut records stay valid and are never rewritten. `doctor` reports them under `legacy_records`, which is informational and does not affect health or exit codes.
- Resolving an existing cut by ID or prefix is unchanged — recomputation is a `doctor` concern only.
- Expect `meta.contract` 3 in envelopes; pin any consumer that asserts on the contract number.
- If you re-file an already-logged cut with a different tag set, expect a new record rather than `changed:false`.

## [0.7.0] - 2026-08-04

### Changed (breaking)

- Rename the project from papercuts to blotter: the binary is `blotter`, the crate is `blotter-cli` (crates.io `blotter` is squatted; the bin target is bound explicitly so the installed binary stays `blotter`).
- Rename the environment contract: `PAPERCUTS_FILE`/`PAPERCUTS_AGENT`/`PAPERCUTS_NOW` → `BLOTTER_FILE`/`BLOTTER_AGENT`/`BLOTTER_NOW` (test-only `PAPERCUTS_BIN` → `BLOTTER_BIN`), with no legacy aliases. A set `PAPERCUTS_*` variable whose `BLOTTER_*` counterpart is unset triggers a `meta.warnings` stale-env entry.
- Rename default discovery paths to `.blotter.jsonl` (repo) and `~/.blotter/log.jsonl` (global). Legacy `.papercuts.jsonl`/`~/.papercuts/log.jsonl` are never auto-discovered; when a legacy file sits next to a discovered default path, commands emit a `meta.warnings` migration nudge.
- Emit `bl_`-prefixed IDs for new records; the dogear hash domain tag moves `pc1` → `bl1`. Legacy `pc_` IDs remain accepted as input, read-only, forever. `resolve` is namespace-aware: explicit `pc_`/`bl_` constrains matching, bare hex searches both namespaces.
- Bump envelope `meta.contract` from 1 to 2.

### Added

- `doctor` reports skipped legacy `pc_` records as an informational `legacy_records` count (not a finding; exit codes unaffected), published through `schema`.
- `hook install` repairs a stale executable path: a managed `hook exec claude-code` command pointing at a moved or renamed binary is atomically replaced and reported as `changed:true`.

### Migration

- Reinstall under the new name: `cargo install --git https://github.com/BigCactusLabs/blotter blotter-cli`.
- `mv` your log file(s): `.papercuts.jsonl` → `.blotter.jsonl`, `~/.papercuts/log.jsonl` → `~/.blotter/log.jsonl`.
- Update `.gitignore`/`.gitattributes` entries to the new file name.
- Replace `PAPERCUTS_*` environment variables with their `BLOTTER_*` counterparts.
- `cargo uninstall papercuts` — the old binary otherwise stays on PATH writing the old file.
- Re-run `blotter hook install` to repair the stale hook path in your settings.
- Expect `meta.contract` 2 in envelopes; new records use `bl_` IDs, and `pc_` remains accepted as input.

## [0.6.0] - 2026-08-03

### Added

- Add `hook exec claude-code`, a silent, fail-open target for Claude Code `PostToolUseFailure` Bash events. It files eligible failures as minor cuts with `auto` and `claude-code` tags, command evidence, and redacted failure notes without ever creating a new log from a hook.
- Add `hook install claude-code` with idempotent, atomic settings updates, explicit/global settings selection, and dry-run support.
- Publish the hook install/exec contract, target support, and silent exit-0 behavior through `schema`.

### Changed

- Make the current Codex hook limitation explicit: `hook exec codex` is a silent no-op and `hook install codex` explains that Codex 0.146.x does not expose shell exit status to hooks (openai/codex#21753).
- Mark `--url` and `--dropped` as dogear-only in `resolve --help`, matching the runtime restriction.

## [0.5.0] - 2026-08-03

### Added

- Add read-only `triage` chronic-cut detection for connected clusters of similar open cuts, with a configurable `--min-count` threshold and a `graduate` suggested action.
- Publish the `triage` flags, output shape, and its 0-empty / 1-findings exit convention through `schema`.

## [0.4.0] - 2026-08-03

### Added

- Record optional task, pull-request, and commit provenance with `resolve --task`, `--pr`, and `--commit` for cuts or dogears.
- Add dogear lifecycle resolution with `--url` for a published destination or `--dropped` for an explicit discard; mixed cut/dogear batches reject the lifecycle flags atomically.
- Publish the new resolve flags, event fields, and materialized resolution fields through `schema`.

## [0.3.0] - 2026-07-23 (fork)

### Added

- Add append-only `dogear` records for research and blog-post ideas with the `dogear` command (alias: `idea`).
- Add `list --kind cut|dogear|all`; cuts remain the default, and `all` renders cuts before dogears.
- Allow `resolve` and `doctor` to process valid dogear records, and publish the dogear contract through `schema`.

### Changed

- Reject `--severity` with `list --kind dogear` or `list --kind all`; severity is a cut-only property.
- A dogear ID is an 80-bit SHA-256 digest (`pc_` + 20 hex) over a version literal, the kind, `ts`, `agent`, `text`, a tag count, and each sorted-unique tag as its own length-prefixed field. Per-tag framing prevents tag-boundary collisions (`["a","b"]` vs `["a,b"]`), and the wider digest keeps dogear IDs disjoint from the 48-bit cut namespace. Cut IDs are unchanged.
- Accept a complete, valid final record that lacks a trailing newline instead of treating it as torn, so a crash-truncated-but-complete tail can never be silently resurrected by the next append.

### Known limitations

- The released cut ID keeps its 48-bit comma-joined-tags scheme; the same tag-boundary edge case remains latent there and is deferred to a future breaking release to preserve byte-compatibility.

## [0.2.0] - 2026-07-16

### Added

- Attach bounded evidence to `add` with `--cmd`, `--exit`, `--stderr-file`, and `--evidence`.
- Resolve multiple IDs or unique prefixes atomically in one `resolve` command.

### Changed

- Redact common credential assignments, authorization values, high-entropy tokens, and URL userinfo before evidence is returned or stored. Redaction remains best-effort.
- Reject non-regular, non-UTF-8, and larger-than-1-MiB stderr inputs; truncate sanitized stored stderr to 4096 UTF-8 bytes.
- Preserve the single-ID resolve response while returning sorted, deduplicated records for multi-ID resolves and warnings for already-resolved IDs.
- Expand `schema` with the evidence record and multi-ID resolve contracts.
- Expand doctor/fold test coverage for malformed records, duplicate cuts, ID conflicts, orphan resolves, conflict markers, torn lines, and append recovery.
- Limit the published crate to source, tests, and release documentation.

### Fixed

- Roll back failed batch appends so partial multi-resolve writes do not corrupt the append-only log.
- Accept leading-hyphen values for evidence and resolution notes without swallowing later options.
- Preserve paths and URLs during best-effort credential redaction while covering token-only URL userinfo and lowercase compound credential keys.

## [0.1.0] - 2026-07-10

- Initial release.

[0.2.0]: https://github.com/treygoff24/papercuts/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/treygoff24/papercuts/releases/tag/v0.1.0
