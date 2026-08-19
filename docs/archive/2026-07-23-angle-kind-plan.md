# First-Class Angle Record Kind Implementation Plan

> **Archived 2026-08-11. Shipped, not pending.** Every step below is complete. The record kind this plan calls `angle` shipped under the name `dogear` (design doc r6); the command is `blotter dogear`, aliased `blotter idea`. The original wording is kept as written — read `angle` as `dogear` throughout. For current behaviour see the design doc and `blotter schema`, not this file.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add append-only `angle` idea records without changing the bytes or behavior produced by the default cut-only workflow.

**Architecture:** `angle` will be an independent mutation command that shares add's input, metadata, lock, append, and duplicate-safe patterns. The fold will recognize both record types, apply resolve events to either, and retain the existing cut sort while appending a separately sorted angle group only when requested.

**Tech Stack:** Rust 2024, clap derive, serde/serde_json, jiff, sha2, assert_cmd.

## Global Constraints

- Preserve append-only JSONL, single-envelope stdout, structured stderr, stable public error codes, and bounded file locks.
- Use `PAPERCUTS_NOW` for deterministic records and tests.
- The default `list` remains cut-only and byte-compatible with the pre-fork surface.
- Reject `list --kind angle|all --severity …` as `invalid_argument`; a severity filter has no unambiguous angle meaning.
- Run the mandated full gate and repeat `cargo test --all-features` five times because `src/store.rs` changes.

---

### Task 1: Establish the public behavior tests

**Files:**
- Modify: `tests/cli.rs`
- Modify: `src/cli.rs`

**Interfaces:**
- Consumes: current `add`, `list`, `resolve`, `doctor`, and schema envelopes.
- Produces: failing black-box assertions for `angle`/`idea`, stdin and dry-run behavior, kind selection, Markdown, resolving, doctor, schema, and incompatible severity filtering.

- [x] **Step 1: Write failing black-box tests**

Add one focused CLI test that exercises:

```rust
let angle = run_file(&file, &["angle", "surprising measurement", "--agent", "tester", "--tag", "research"]);
let listed: SuccessEnvelope<ListData> = success(&run_file(&file, &["list", "--kind", "angle"]));
assert_eq!(listed.data.items[0].kind(), "angle");
```

Add assertions that default `list` excludes the angle, `--kind all` keeps cuts before angles, Markdown contains `## Angles`, `resolve` marks the angle resolved, `doctor` stays healthy, schema documents the command, and `list --kind angle --severity minor` exits 2 with `invalid_argument`.

- [x] **Step 2: Run the focused test to verify it fails**

Run: `cargo test angle_kind -- --nocapture`

Expected: failure because clap has no `angle` subcommand and no `--kind` list filter.

### Task 2: Add record, command, and fold support

**Files:**
- Create: `src/commands/angle.rs`
- Modify: `src/lib.rs`
- Modify: `src/cli.rs`
- Modify: `src/commands/mod.rs`
- Modify: `src/store.rs`
- Modify: `src/commands/add.rs`
- Modify: `src/commands/list.rs`
- Modify: `src/commands/resolve.rs`
- Modify: `src/commands/doctor.rs`
- Modify: `src/commands/schema.rs`

**Interfaces:**
- Produces: `AngleRecord`, `compute_angle_id`, `angle::run`, `ListKind`, and a list-item representation for either record kind.
- Consumes: existing exclusive/shared lock helpers and `ResolveRecord` events.

- [x] **Step 1: Implement the minimal public model and parser**

Define an angle event with exactly `kind`, `id`, `ts`, `agent`, `text`, `tags`, optional string `evidence`, `cwd`, and `repo`. Define its ID as the same length-prefixed SHA-256 scheme over `ts`, `agent`, `text`, and sorted tags (no severity field). Add `angle` with alias `idea`, accepting only TEXT, agent, repeatable tag, evidence, and dry-run.

- [x] **Step 2: Implement mutation and fold behavior**

Use add's discovery, input, agent, and append patterns. During the exclusive lock, fold existing records and return the first record on same-ID duplicate. Teach the fold to validate, deduplicate, resolve, and sort angles by timestamp descending then ID ascending; retain the existing cut severity-first ordering and emit all cuts before angles.

- [x] **Step 3: Implement list, resolve, doctor, and schema integration**

Filter `list` by `--kind cut|angle|all`, defaulting to cut. Apply common status/agent/tag/since filters to either type, reject severity except for `cut`, and render angles after a `## Angles` Markdown heading. Extend resolve-prefix candidates across both first-wins record types. Validate/recompute angle IDs in doctor. Publish the command, filter decision, and both record/list shapes in schema.

- [x] **Step 4: Run focused tests to verify they pass**

Run: `cargo test angle_kind -- --nocapture`

Expected: all new angle test assertions pass.

### Task 3: Document and release the fork behavior

**Files:**
- Modify: `Cargo.toml`
- Modify: `CHANGELOG.md`
- Modify: `README.md`
- Modify: `AGENTS.md`
- Modify: `docs/plans/2026-07-09-papercuts-design.md`

**Interfaces:**
- Produces: published 0.3.0 fork contract and user-facing angle examples.

- [x] **Step 1: Bump and document the release**

Set package version to `0.3.0`. Add a `0.3.0 (fork)` changelog section, a concise README `## Angles` section with `angle`, `idea`, and `list --kind` examples, and the new command file to AGENTS layout notes.

- [x] **Step 2: Amend the normative design doc**

Append an `r5` amendment dated `2026-07-23`, explicitly naming fork `BigCactusLabs/papercuts`, its rationale (an idea-log channel alongside friction), the selection/sort/resolve/doctor behavior, and the rejected severity combination.

### Task 4: Dogfood, verify, and commit

**Files:**
- Modify: `.papercuts.jsonl` if the repository tracks its dogfood log.

- [x] **Step 1: File required dogfood records**

After the binary builds, append one genuine friction cut encountered during implementation and one `angle` record using the new subcommand. Verify they can be listed with their intended kind.

- [x] **Step 2: Run the mandatory gate**

Run:

```bash
cargo build --release
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --check
for run in 1 2 3 4 5; do cargo test --all-features || exit $?; done
```

Expected: every command exits 0.

- [x] **Step 3: Inspect scope and commit**

Run `git diff --check` and `git status --short`, inspect the final diff, then commit the intended files with:

```bash
git add AGENTS.md CHANGELOG.md Cargo.toml README.md docs src tests .papercuts.jsonl
git commit -m "feat(angle): first-class angle record kind for research/blog ideas" -m "Add append-only angle records, kind-selectable listing, resolution, doctor, schema, and fork documentation while preserving the default cut-only list surface."
```
