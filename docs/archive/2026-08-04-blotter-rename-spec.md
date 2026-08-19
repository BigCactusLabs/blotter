# Design: rename papercuts → blotter

Status: implemented (0.8.0, 2026-08-05); the pre-rename migration surface was removed in 0.9.0. Archived 2026-08-11 — superseded, kept for provenance. Reviewed by cross-model code-level review (Codex, 2026-08-04) and fresh-eyes design review (Opus, 2026-08-04); all findings resolved below.

## Decision summary

- New name: **blotter** (a blotter is a running log of small incidents — police blotter, desk blotter). Binary `blotter`, GitHub repo `BigCactusLabs/blotter`, crate `blotter-cli` (crates.io `blotter` is already taken by a placeholder crate), Homebrew formula `blotter` (free).
- Record vocabulary **stays**: `cut` (friction) and `dogear` (idea). They are features, not brands.
- Migration is a **clean break with a nudge**: no auto-discovery of legacy paths; a warning points at hand-migration. Hand-moved logs are a supported state — legacy `pc_` IDs remain readable forever.
- Full ID rebrand for new records: `bl_` prefix; dogear hash domain tag `pc1` → `bl1`.
- Envelope `meta.contract` bumps **1 → 2**.
- Version **0.7.0**, breaking.

## 1. Identity & packaging

`Cargo.toml`: package `blotter-cli`; `[lib] name = "blotter"`; `[[bin]] name = "blotter", path = "src/main.rs"` (explicit binding required — target inference under package `blotter-cli` would misname the bin). Description rewritten; repository URL → `https://github.com/BigCactusLabs/blotter`.

The clap command name becomes the literal `"blotter"` in `src/cli.rs` (today it derives from `CARGO_PKG_NAME`, which would render "blotter-cli"). `src/main.rs` and tests import the lib as `blotter`. Tests use exactly `CARGO_BIN_EXE_blotter` (Cargo uses the bin target name verbatim) and `cargo_bin_cmd!("blotter")`.

## 2. Environment contract

`PAPERCUTS_FILE` / `PAPERCUTS_AGENT` / `PAPERCUTS_NOW` → `BLOTTER_FILE` / `BLOTTER_AGENT` / `BLOTTER_NOW`; test-only `PAPERCUTS_BIN` → `BLOTTER_BIN`. No legacy aliases (pre-1.0, agent-only tool).

**Stale-env warning**: when a `PAPERCUTS_FILE`/`PAPERCUTS_AGENT`/`PAPERCUTS_NOW` variable is set and its `BLOTTER_*` counterpart is not, mutating and reading commands emit a `meta.warnings` entry naming the ignored variable. No behavior change otherwise; exit codes unaffected.

`schema` output updates: env names, discovery paths, both ID namespaces (`bl_` emitted, `pc_` accepted read-only), contract 2.

## 3. Discovery & migration

Discovery order unchanged; names change: `--file` > `BLOTTER_FILE` > `<repo-root>/.blotter.jsonl` > `~/.blotter/log.jsonl`.

Legacy paths (`.papercuts.jsonl`, `~/.papercuts/log.jsonl`) are never auto-discovered. **Legacy-file warning**: when discovery resolves to a repo-default or global-fallback path (not an explicit `--file`/`BLOTTER_FILE` path) and the corresponding legacy file exists, commands emit a `meta.warnings` entry suggesting the rename (`mv .papercuts.jsonl .blotter.jsonl`) and reminding that `.gitignore`/`.gitattributes` entries need the same edit. This is a warning, not a doctor finding: it must not turn `doctor` exit 0 into a permanent exit 1. No new error codes.

This repo's own dogfood log: `git mv .papercuts.jsonl .blotter.jsonl`. A rename preserves every byte — the append-only invariant holds.

## 4. IDs

- New records emit `bl_`-prefixed IDs. Cut ID derivation is otherwise **untouched** (cut hashing has no domain tag; only the prefix string changes — existing golden digest carries over as `bl_6d26611bad4c`). Dogear hashing changes its domain tag `pc1` → `bl1`, so new dogear digests differ from legacy ones.
- **Namespace-aware resolve**: input normalizes to `(optional_namespace, hex_prefix)`. Explicit `pc_`/`bl_` constrains matching to that namespace; bare hex searches both. A bare-hex prefix matching more than one record — in one namespace or across both — is the existing multiple-match error, unchanged.
- `pc_` input acceptance is **permanent**: read-only legacy support, never emitted for new records.
- **Doctor**: ID recomputation applies to `bl_` records only. `pc_` records are skipped and surfaced as an informational count in doctor's envelope (a new `legacy_records` field, added to schema — not a finding, not exit-affecting). Rationale: legacy integrity checking is not a requirement; skipping avoids carrying a parallel legacy hasher.
- Fold is unaffected: it keys records and resolutions by full stored ID string. **Accepted consequence**: identical record text re-added post-rename derives a different ID, so no cross-era dedup — for cuts and dogears alike.

## 5. User-facing strings

Every "papercuts" in help text, error messages, warnings, and suggested fixes → "blotter" (~30 sites across `src/error.rs`, `src/store.rs`, `src/commands/*`). The singular record noun "papercut" → "cut" ("matches multiple papercuts" → "matches multiple cuts"; "no papercuts logged" → "no cuts logged"). Error codes and exit codes unchanged. Hook installer temp-file pattern → `.{filename}.blotter-{PID}-{seq}-{attempt}.tmp`.

**Exclusion**: README's fork-provenance paragraph (which uses "papercuts" to mean the upstream `treygoff24/papercuts` project) is a prose rewrite, not part of the mechanical sweep.

## 6. Envelope contract

`meta.contract`: 1 → 2 in `src/output.rs`, `schema`, and tests. The field exists for skew detection; this change moves IDs, env names, discovery paths, and schema values, which is exactly the skew it should signal.

## 7. Hook repair

`hook install` currently treats any command ending in `hook exec claude-code` as already-installed, which would leave a stale absolute path to the old papercuts binary in place with `changed:false`. Change: a managed command (recognized by the suffix) whose embedded executable differs from the current one is **stale** — atomically replace it with the current executable's path and report `changed:true`. Regression test covers the stale-path case. README's claim that executable-path changes are safe is corrected.

## 8. Docs

Living docs rewritten: `README.md` (including the new install line — `cargo install --git https://github.com/BigCactusLabs/blotter blotter-cli` — and a note that crate name and binary name differ) and `AGENTS.md`. The completed manifest checker was later removed; its supporting historical records are archived in `docs/archive/papercuts-remediation/`.

The normative design doc `docs/plans/2026-07-09-papercuts-design.md` gains an **r9 rename amendment** in its Amendments section: name, env vars, ID namespaces, discovery paths, contract 2, warning semantics. History above the amendment is preserved verbatim; the amendment supersedes the live contract. `AGENTS.md` keeps pointing at it with a "(written under the pre-rename name)" note.

At the time of the rename, `scripts/check-manifest.sh` received a **split migration**: live plumbing (binary discovery, `BLOTTER_FILE`, default paths, live-ID acceptance of both prefixes) was renamed; frozen historical `pc_` parsers, fixtures, and the historical manifest stayed untouched. This completed historical gate was later removed with its manifest.

At the time of the rename, `docs/reviews/`, other `docs/plans/`, diagnostic reports, `model-performance-journal.md`, `fresh-eyes-review-2026-07-16.md`, old CHANGELOG entries, and `backlog/` task files were untouched. The completed remediation reports, plans, and reviews now live in `docs/archive/papercuts-remediation/`.

**CHANGELOG 0.7.0 migration note** (complete list): rename binary/crate; `mv` your log file(s); update `.gitignore`/`.gitattributes` entries; replace `PAPERCUTS_*` env vars; `cargo uninstall papercuts` (the old binary otherwise stays on PATH writing the old file); re-run `blotter hook install` (repairs the stale hook path); `meta.contract` is now 2; new records use `bl_` IDs, `pc_` remains accepted as input.

## 9. Repo & infra (final phase)

Runs only after everything above lands and the gate passes: GitHub repo rename `dogear` → `blotter` (`gh repo rename`, redirects preserved) and local directory rename. Both are shared-state actions requiring explicit user confirmation at execution time. Out of scope: crates.io publish (backlog task-2), any contact with the existing crate owner.

## 10. Verification

Full gate: `cargo build --release`, `cargo test --all-features`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo fmt --check`. Test suite run 5× (store.rs discovery constants change). Determinism: byte-identical envelopes under `BLOTTER_NOW`.

Coordinated fixture updates (~50 sites, census in review artifacts): `tests/cli.rs` and the now-removed historical manifest-checker fixtures moved crate/bin/env/path/version fixtures to blotter names; generic `pc_` fixtures became `bl_`; explicit migration-coverage fixtures stayed `pc_`; `src/store.rs` unit fixtures updated. New tests: stale-env warning, legacy-file warning (fires on default paths, silent on explicit paths), namespace-aware resolve (explicit prefix constrains, bare hex searches both, cross-namespace multiple-match errors), doctor legacy skip count, hook stale-path repair.
