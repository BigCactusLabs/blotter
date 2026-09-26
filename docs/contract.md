# Current implementation contract

This is the contributor contract for the current implementation, replacing the historical amendment chain. It consolidates existing guarantees; it does not introduce a new CLI contract or stored-record version.

The complete version-specific machine interface is emitted by `blotter schema`, implemented in [schema.rs](../src/commands/schema.rs). The contract number lives in [output.rs](../src/output.rs), the package version in [Cargo.toml](../Cargo.toml), and the error/exit dictionary in [error.rs](../src/error.rs). Do not maintain another numeric version table here. [The reference](reference.md) explains command behavior. If code, schema, and prose diverge, reproduce the discrepancy and reconcile them with regression coverage; historical prose does not silently override the current interface.

## Guarantees, current behavior, and choices

Storage safety, truthful evidence, and explicit authorization are trust guarantees. Experiments do not bypass them. The detailed identity, output, matching, and resolution rules below describe supported behavior today: preserve them until deliberately changed, not because every present choice must last forever.

CLI-first operation without a required service is the product default. Optional local presentations and read-only analysis experiments can explore better ways to use the ledger. Keep them separate from supported interfaces while testing an idea; they need a scoped task or PR, not a new amendment or standing mandate. A useful regression test records current expectations, not a permanent veto on an intentional improvement.

## Product and admission

Blotter is a local, noninteractive ledger, not a transcript, task tracker, server, or publication service. Cuts record friction with at least one transferable, consequential, recurring, misleading, or systemic ground. Impact records consequence after admission. Dogears require all three: one observed finding in the author's own words, useful beyond the task, and understandable without the repository. Promotions record an explicit relationship between source cuts and an approved durable artifact.

Independent recurrences are evidence. Do not deduplicate experiences through a pre-filing lookup, manufacture repeated observations, or auto-promote because a threshold was crossed. `triage`, `verify`, `retrospect`, and `digest` are readers, never hidden writers. Only `promote` writes a promotion; it does not resolve its sources.

## Storage and transaction boundaries

Ordinary mutations append. Under one exclusive lock, probe the stored version, read and fold, validate the entire operation, decide, and append. Do not move state-dependent validation outside that transaction. Shared-lock readers do not modify bytes. Advisory-lock contention is bounded; timeout is retryable exit 75, except the explicitly documented per-repository skip behavior of `sweep`.

Every newly stored record has `"v":2` first. That storage marker is not materialized in ordinary output records. A known-kind record without supported v2 causes the whole log to be refused before a write. There is no implicit upcaster, legacy parser, or migration write.

The fold is first-wins for repeated record IDs. Resolve events have separate base/amend semantics. Preserve the established handling of malformed lines, orphan resolutions, unknown kinds, and duplicate records; do not turn tolerated input into destructive “cleanup.” The normative fold lives in [store.rs](../src/store.rs), with black-box coverage in [tests/cli](../tests/cli).

`doctor --fix` and `archive` are the only log-replacement operations. They preserve the original in a backup and use copy-and-swap, not in-place edits. A failed operation must preserve the original ledger and clean up its own partial artifacts according to the tested rollback rules. Archive preserves original line bytes; it is not a privacy eraser. Symlink/regular-file handling, torn-tail repair, append rollback, and lock timing require dedicated regression tests.

## Identity and deterministic output

Record IDs use the existing `bl2` domain-separated, length-framed hashing rules, with one `bl_` plus 20-lowercase-hex width for cuts, dogears, and promotions. Identity-bearing fields are defined in the implementation and tests, not inferred from field names. Evidence, origin, and the storage marker do not become identity merely because they appear in a record; promotion notes remain outside the identity hash.

ID arguments are optional `bl_` plus at least four hexadecimal digits, case-insensitive. Resolve ambiguity across matching records before kind validation; no exact-full-ID precedence is introduced. Promotion sources are canonical, sorted, and deduplicated before writing.

For the same explicit inputs and fixed `BLOTTER_NOW`, output is deterministic. Preserve ordering, omission versus null behavior, envelope shapes, and warning semantics. Success is one JSON envelope on stdout, errors are structured on stderr. Supported Markdown output and OTLP export are deliberate raw-output exceptions. Empty results succeed. Exit 1 signals findings for commands that document it; it is not a count or a generic fatal error.

## Resolution and recurrence

Resolve batches validate all targets and cross-record references before appending anything. Cut resolutions require a disposition; dogears reject it. Mixed cut/dogear batches are rejected, including amendments. Promotions have no resolution lifecycle.

The first valid non-amend resolution is the base. The latest eligible amend by timestamp determines the materialized correction, not the final physical line after a branch merge. Amendments replace ordinary resolution fields; they are not arbitrary merge patches. The explicit inheritance of disposition/disposition timestamp and promotion-link behavior must remain intact.

`disposition_ts` records when the classification was made. A note-only amendment does not move it. Verification anchors only eligible resolved `fixed` or `promoted` cuts and compares later open cuts strictly after that boundary. Accepted or invalid friction is not a failed-fix anchor. One recurring cut can match several anchors: `count` and `distinct_recurring_cuts` are intentionally different. Absence of a recorded recurrence is not proof of permanent repair.

All analyzers reuse the established title/tag/token linkage rule. `retrospect` separates observed patterns from suggested artifact types and remains read-only. Do not use a matching heuristic as an authorization rule. A resolution's promotion link must be mutual: the referenced promotion already lists every source cut being resolved.

## Privacy and publication

Use existing input bounds and UTF-8-safe truncation. Home-path rewriting and secret-pattern redaction are different operations: cut/dogear text does not receive the full evidence secret pass. Evidence, notes, and artifact references use their documented best-effort sanitation. Neither the writer nor `doctor --leaks` guarantees confidentiality.

Do not introduce telemetry, hidden network writes, automatic publishing, new hooks, or a server as a side effect of capture or discovery. Synthetic fixtures must remain explicitly synthetic. An installed skill does not prove model activation; a built site does not prove deployment; an accepted submission does not prove indexing; returned snippets do not prove accuracy.

## Changing this contract

For a behavioral change, update the affected implementation, executable schema, reference/contract, and regression tests together. A private refactor or an unshipped prototype need not edit unrelated contract prose. Breaking a supported interface requires an explicit compatibility decision, a `meta.contract` change, and the appropriate major release. Additive changes can retain the contract number. The internal Rust library is implementation, not a supported public API.

Record current rules in place. Preserve superseded rationale through [history](history.md), not another append-only amendment section. Acceptance of breaking changes is permission to simplify deliberately, not permission to break storage safety or silently change published semantics.
