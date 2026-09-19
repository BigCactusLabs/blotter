# Changelog

User-visible changes belong here. Current behavior is documented in the [reference](docs/reference.md); historical release prose does not override it. The [unabridged pre-audit changelog](https://github.com/BigCactusLabs/blotter/blob/7047eb56a3eba45e3e2b83f033691b75c98298ec/CHANGELOG.md) preserves detailed release notes and pre-1.0 history.

## [Unreleased]

### Documentation and development

- Replace mandatory historical-amendment reading with a current implementation contract and task-oriented documentation map.
- Shorten README and agent instructions; add CONTRIBUTING with build, test, task, and release ownership.
- Separate agent installation from catalog operations and consumer-site deployment.
- Correct the quickstart's assumed ID, text-redaction claims, and exit-1 interpretation.
- Add source Markdown links/anchors/navigation checks, schema/reference command coverage, and an isolated executable README smoke test to CI.
- Remove the superseded design document from the active tree and condense release history, retaining immutable historical links. No runtime, ledger, CLI version, contract number, or skill/plugin version change.

## [1.1.1] - 2026-09-04

- Add one-line help descriptions for all subcommands.
- Add cargo-dist binary archives and shell, PowerShell, and Homebrew installers.
- Add the canonical Agent Skill and cross-ecosystem plugin manifest; the independently versioned skill/plugin package is 1.0.1.
- Introduce the dedicated CLI reference and reorganize the README for installation and orientation.

## [1.1.0] - 2026-09-03

- Define dogears as observed findings useful beyond the task, not generic ideas or backlog chores.
- Add visible `finding` alias alongside `idea`, plus admission guidance in schema/help.
- Clarify that additive changes need not bump the contract. No record/envelope break; contract remains 6.

## [1.0.0] - 2026-09-03

Breaking release: contract 6, v2 records, `impact` replacing `severity`, unified `bl2` IDs, structured origin, explicit cut resolution dispositions, and promotion records. Verification uses disposition timestamps so note-only amendments do not hide recurrence. The automatic hook lane and legacy parser are removed.

Before upgrading from 0.15 or earlier, remove the old Claude Code hook and move the old ledger out of the discovery path without overwriting anything. There is no automatic migration. Follow [the complete upgrade procedure](docs/reference.md#upgrading-from-015). Detailed behavioral changes and performance evidence remain in the immutable unabridged history linked above.
