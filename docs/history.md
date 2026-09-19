# History and documentation migration

Historical material is available in Git, not required reading for current work. The [current contract](contract.md), [reference](reference.md), and installed `blotter schema` replace the former instruction to read amendments backwards.

## September 19, 2026 documentation audit

The audit baseline is commit `7047eb56a3eba45e3e2b83f033691b75c98298ec`. The cleanup changes documentation organization and validation, not CLI behavior, record formats, ledger data, release tags, or external publication state.

| Finding | Resolution |
| --- | --- |
| A 230,603-byte design file with 54 amendments served as mandatory current law | Removed from the active tree; current guarantees consolidated in `contract.md`, with implementation/schema/test pointers |
| 14,118-byte agent instructions mixed contract, absent local-only notes, task history, and operational procedure | Replaced with a focused routing file; build/testing/backlog procedure moved to CONTRIBUTING |
| README duplicated large instruction blocks and resolved a made-up ID in its quickstart | Kept the product voice, shortened repetition, and marked a real executable quickstart for an isolated smoke test |
| The reference overstated text redaction and called finding exit 1 a count | Documented actual text/evidence differences and finding-status semantics |
| Installation mixed research, merged-PR narration, rejected submissions, and future plans | Separated consumer installation from a dated operational publication runbook |
| Documentation could drift without a source-level regression check | Added Markdown link/anchor/reachability checks, focused entry-point limits, command coverage, and optional executable quickstart validation |
| Release history was being treated as current instructions | Kept a compact changelog and immutable pointers to the full historical record |

Real ledgers, legacy ledger bytes, and Backlog task IDs are deliberately untouched. Existing skill/plugin metadata and the five-page site allowlist retain their distribution scope. Documentation history is not a reason to delete product data.

## Immutable records

- [Full design and all 54 amendments at the audit baseline](https://github.com/BigCactusLabs/blotter/blob/7047eb56a3eba45e3e2b83f033691b75c98298ec/docs/plans/2026-07-09-papercuts-design.md).
- [Unabridged release history at the audit baseline](https://github.com/BigCactusLabs/blotter/blob/7047eb56a3eba45e3e2b83f033691b75c98298ec/CHANGELOG.md).
- [Discovery research and dated channel observations](https://github.com/BigCactusLabs/blotter/blob/7047eb56a3eba45e3e2b83f033691b75c98298ec/docs/discovery.md).

These snapshots may describe removed commands, superseded rules, unavailable private notes, or past channel status. They are provenance, not installation instructions or live availability claims. Use the Git blame/history of the relevant source and regression test when investigating a specific old decision.
