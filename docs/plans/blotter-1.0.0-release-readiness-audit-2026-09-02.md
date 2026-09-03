# Blotter 1.0.0 — Release Readiness Audit

**Date:** 2026-09-02  
**Branch reviewed:** `v2`  
**Status:** Incorporated 2026-09-02 — §1.1–1.4, §2.1 (policy statement only), §3.1, §3.3–3.5 landed on the `release-hygiene` branch; §3.2 and §4–§8 folded into TASK-78. Archive this doc in the release PR.
**Scope:** Final pre-1.0 audit after all substantive v2 work landed

---

## Executive Summary

Blotter is ready to cut 1.0.0.

No remaining architecture or product work appears necessary before release. The substantive v2 program is complete:

- admission floor is live;
- auto/hook lane is removed;
- v2 fresh-ledger contract is implemented;
- `severity` → `impact` is complete;
- all v2 IDs use one 80-bit width;
- `origin` is structured and intentionally small;
- resolution dispositions are implemented;
- promotion is first-class and provenance-only;
- `verify` uses `disposition_ts`;
- `retrospect` uses pattern + suggested intervention semantics;
- `digest.accepted_cuts` is implemented;
- triage no longer tells consumers to “graduate” records;
- linkage precision was measured across real corpora and tightened under r53.

The remaining work is almost entirely release hygiene.

The main recommendation is:

> **Do not add more product scope. Fix the handful of release/documentation inconsistencies, settle the Rust-library API policy, run the package/publish smoke gates, and cut 1.0.0.**

---

# 1. Release Blockers / Fix Before Release

## 1.1 Reconcile `main` into `v2`

`v2` is ahead of `main`, but `main` still has one commit not present on `v2`.

That commit adds the temporary warning in `AGENTS.md` explaining that contract 6 is unreleased and lives only on the `v2` branch.

Before the final release PR:

1. merge/rebase `main` into `v2`;
2. explicitly remove the temporary “contract 6 is unreleased” paragraph in the release commit.

Do not rely on the final merge to make the paragraph disappear automatically.

---

## 1.2 Fix the stale historical-migration claim in README

The README still contains historical migration language implying that older records remain readable by the current binary.

That is no longer true.

The 1.0 contract deliberately refuses v1 logs.

Rewrite the relevant section so it says:

- pre-1.0 logs remain preserved on disk;
- the 0.15 binary can still read them;
- the 1.0 binary refuses them whole;
- the correct migration is to rename the old ledger out of the discovery path and start a fresh v2 ledger.

This should not be ambiguous anywhere in the 1.0 README.

---

## 1.3 Put both mandatory upgrade actions in one prominent place

The top-level README section:

```text
Upgrade to 1.0.0
```

currently emphasizes removal of the old Claude Code hook.

The fresh-ledger break is documented later.

For a major release, both actions should appear together near the top.

Recommended shape:

### Before upgrading to 1.0.0

1. Remove the old `hooks.PostToolUseFailure` entry that invokes:

   ```text
   blotter hook exec claude-code
   ```

2. Preserve the old ledger by renaming it out of Blotter's discovery path.

3. Let the next `blotter add` create a new v2 ledger.

4. Keep the 0.15 binary available if historical v1 data must still be queried.

The incompatible-log runtime error remains the last line of defense, but the release docs should tell users before they hit it.

---

## 1.4 Fix the shortened admission summary

The README's detailed admission policy correctly names the five valid admission paths:

- transferable;
- consequential;
- recurring;
- misleading;
- systemic.

However, one short summary later narrows the rule back toward consequential / transferable / recurring.

That recreates an ambiguity already caught during earlier review.

Use the same future-value framing everywhere:

> A cut qualifies when it was consequential once, or carries useful knowledge beyond this run — transferable, recurring, misleading, or systemic.

Do not let a convenience summary silently drop `misleading` and `systemic`.

---

# 2. One Pre-1.0 Product-Surface Decision

## 2.1 Decide whether the Rust library API is supported

The published crate is not only a binary.

`Cargo.toml` declares:

```toml
[lib]
name = "blotter"
```

and the library exposes a substantial number of public items:

- `blotter::cli`;
- `blotter::commands`;
- `blotter::store`;
- `LogEvent`;
- `ListItem`;
- `Resolution`;
- ID utilities;
- error/output structures;
- and other implementation types.

The product, however, is explicitly designed around a different stable contract:

```text
CLI
JSON envelopes
stored JSONL format
blotter schema
exit codes
```

That raises a semver question.

If the Rust API is **not** intended as part of Blotter's supported 1.x public contract, make that explicit before publishing `1.0.0`.

At minimum, document:

> The supported compatibility contract is the CLI, JSON envelopes, record format, exit codes, and `blotter schema`. The Rust library surface is internal and not a stable integration API.

A stronger option would be to reduce the public library surface before 1.0.

However, do **not** perform a rushed large internal refactor purely for cleanliness. The important thing is to avoid accidentally implying that every currently `pub` Rust implementation item now carries a long-term 1.x semver promise.

This is the only remaining item that may justify code-level work before the cut.

---

# 3. Release Polish Worth Doing

## 3.1 Update `Cargo.toml` package description

The current package description still reflects the older product:

> A tiny CLI for AI agents to log the cuts they hit during work.

That undersells the 1.0 product.

The current architecture is closer to:

> An append-only experiential learning ledger for AI agents: capture meaningful friction and ideas, detect recurring patterns, record durable learning, and verify whether interventions held.

The final version should remain short enough for crates.io.

Also consider adding appropriate Cargo metadata while already touching the manifest:

```toml
keywords = [...]
categories = [...]
```

Keep these conservative and accurate.

---

## 3.2 Update the GitHub repository description

The GitHub repository description still emphasizes:

```text
cuts
dogears
triage
verify
digest
```

but not:

- promotions;
- durable learning;
- experience → intervention provenance.

Update it for 1.0 so the repository's first impression matches the shipped product.

---

## 3.3 Archive both superseded review/checkpoint docs

TASK-78 currently says to archive “the checkpoint doc.”

There are now at least two superseded v2 planning/review artifacts:

- `blotter-v2-signal-floor-checkpoint-2026-09-01.md`
- `blotter-v2-progress-review-feedback-2026-09-01.md`

Archive both.

The v2 implementation plan can remain in place as the historical implementation/release plan for the current 1.0 boundary unless the repo's documentation policy says otherwise.

---

## 3.4 Update the archived auto-capture design status

TASK-78 already calls this out.

Update:

```text
docs/archive/2026-08-09-auto-capture-default-hidden-design.md
```

so its status clearly notes that the surface was removed in 1.0.0 under r48.

Do not rewrite the archived body; preserve it as provenance.

---

## 3.5 Deliberately decide whether `.blotter.v1.jsonl` remains tracked

The v1 dogfood corpus has been preserved as:

```text
.blotter.v1.jsonl
```

Keeping it is defensible and arguably consistent with Blotter's provenance philosophy.

It also does not ship in the crates.io package because `Cargo.toml` uses an explicit include list.

Recommended approach:

- keep it tracked unless there is a specific privacy/noise reason to remove it;
- run one final leak scan over the archived file before the 1.0 release.

The active CI leak gate now targets the fresh v2 `.blotter.jsonl`, so the archived ledger deserves one deliberate one-time release check.

---

# 4. Version / Release Metadata That Is Correctly Still Uncut

At the moment of this review, the following are still intentionally pre-release:

```text
Cargo.toml version = 0.15.0
CHANGELOG = [Unreleased]
OTLP fixture scope version = 0.15.0
```

That is correct.

TASK-78 should change these together in the release PR:

```text
Cargo.toml → 1.0.0
Cargo.lock → 1.0.0 package entry
CHANGELOG → [1.0.0] - 2026-09-02
OTLP golden → scope version 1.0.0
```

Do not let them drift across separate commits/merges unless tests force the split.

---

# 5. Add Package/Publish Validation to TASK-78

The release plan already includes:

- normal gate;
- MSRV check;
- version bump;
- changelog;
- README read-through.

Add explicit Cargo packaging validation.

Before merging the release PR, run:

```bash
cargo build --release
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --check

scripts/dev/check-msrv.sh

cargo package --locked
cargo publish --dry-run --locked
```

The `cargo package` and `cargo publish --dry-run` steps catch release-only problems that normal CI does not:

- missing packaged files;
- incorrect include/exclude rules;
- manifest issues;
- dependency publication problems;
- crate metadata problems.

Also run:

```bash
backlog doctor
```

before cutting the release.

---

# 6. Final Release-Build Smoke Tests

After building the exact release candidate, smoke the user-facing binary itself.

At minimum:

```bash
blotter --version
blotter schema
blotter --help
blotter add --help
```

Then use a fresh temporary repo/log for a basic v2 lifecycle:

```text
add cut
list
dogear
promote
resolve --disposition promoted --promotion ...
verify
retrospect
digest
doctor
```

The purpose is not to replace the test suite.

It verifies that the final packaged/release binary feels coherent as a product rather than only as individual tested commands.

Also explicitly test the migration boundary once:

```text
point 1.0 binary at a v1 ledger
→ unsupported_log_version
→ zero bytes changed
→ useful suggested_fix
```

---

# 7. CI / MSRV

The repository CI currently runs:

- release build;
- tests;
- clippy;
- fmt;
- dogfood leak gate;
- Rust 1.89 build.

The dedicated release MSRV script is stronger because it executes the full locked test suite under 1.89.

Run it before release:

```bash
rustup toolchain install 1.89.0 --profile minimal
scripts/dev/check-msrv.sh
```

Treat a green stable CI run plus the local/exact MSRV test as part of the release bar.

---

# 8. Recommended Final Cut Sequence

## Step 1 — Reconcile branches

Bring the one main-only commit into `v2`.

Ensure the final `AGENTS.md` does **not** retain the temporary:

```text
Unreleased v2...
```

warning.

---

## Step 2 — Apply release hygiene

In the release PR:

- fix README migration wording;
- consolidate both mandatory upgrade steps;
- fix the shortened admission summary;
- settle/document Rust-library API support policy;
- update Cargo description/metadata;
- update GitHub repo description;
- archive both superseded v2 checkpoint/review docs;
- update archived auto-capture status;
- decide/preserve `.blotter.v1.jsonl`;
- bump Cargo version;
- update Cargo.lock;
- cut CHANGELOG 1.0.0 header;
- update OTLP golden version;
- re-record README quickstart output from the actual release binary if necessary.

---

## Step 3 — Run release gates

Run:

```bash
cargo build --release
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --check

scripts/dev/check-msrv.sh
backlog doctor

cargo package --locked
cargo publish --dry-run --locked
```

Run a one-time leak check over the archived v1 dogfood file as well.

---

## Step 4 — Smoke the release binary

Check:

```bash
blotter --version
blotter schema
blotter --help
```

Then run a fresh v2 lifecycle.

Also confirm v1 refusal behavior.

---

## Step 5 — Open the single `v2 → main` release PR

This should be the one transition where consumers see contract 6.

Require CI green.

Avoid adding any new behavior during review unless the review exposes a release blocker.

---

## Step 6 — Merge and tag the exact commit

After merge, create the annotated tag:

```text
v1.0.0
```

on the exact merged release commit.

The previous `v0.15.0` release used an annotated tag, so preserving that convention is sensible.

---

## Step 7 — Publish the crate

From the exact tagged commit:

```bash
cargo publish --locked
```

Then verify:

- crates.io shows `blotter-cli 1.0.0`;
- docs.rs builds successfully;
- crate metadata/README render correctly;
- install works:

```bash
cargo install blotter-cli
blotter --version
```

---

## Step 8 — Create a GitHub Release

The repo has historically used tags without GitHub Releases.

For `1.0.0`, create a GitHub Release anyway.

It gives the first stable release a durable landing page and a place to make the migration warnings impossible to miss.

The release notes should prominently call out:

1. fresh ledger / v1 refusal;
2. removal of the Claude Code hook lane;
3. `severity` → `impact`;
4. new dispositions;
5. promotions;
6. new 80-bit IDs;
7. contract 6.

---

# 9. Things Not to Change Before Release

Do **not** reopen any of these:

- fresh-ledger strategy;
- promotion architecture;
- admission scoring/classification;
- telemetry ingestion;
- dogear promotion;
- persisted retrospect patterns;
- artifact automation;
- recurrence model beyond r53;
- disposition vocabulary;
- ID width;
- origin schema;
- auto capture.

The architecture has already had enough scrutiny.

Adding another product change now creates more release risk than value.

---

# 10. Final Assessment

Blotter 1.0.0 is substantively ready.

There is no remaining architectural concern that justifies delaying the release.

The final bar should be:

> **Make the release artifact internally consistent, make the migration impossible to misunderstand, make the published metadata match the product, avoid accidentally promising an unsupported Rust API, and validate the exact package that will reach crates.io.**

Then cut it.

The release should feel like a boundary, not another research iteration.
