# papercuts — design doc

2026-07-09. Coordinator-authored. Status: see the last Amendments section for review provenance and deliberate contract additions. Amendments accumulate and the newest wins; where an amendment contradicts an earlier section, the later text is the contract and the earlier is history.

## Thesis and provenance

Agents hit friction constantly — dead-end tool calls, broken links, missing helpers, footgun configs — and silently push through without telling anyone. The signal evaporates. `papercuts` is a tiny agent-first CLI that gives agents a one-line way to file the complaint at the moment they hit it, and gives humans (and other agents) a way to review and burn down the backlog.

Provenance: Steve Ruiz shipped a private version of this inside his repo (X post, 2026-07-09) and reported it immediately surfaced real workflow defects the agents had been eating silently. Every one is an actionable fix a human would never have heard about otherwise. This is a validated behavior pattern, not a speculative product.

Why a CLI and not an MCP server or harness feature: every agent harness (Claude Code, Codex, Cursor, Droid, anything) can shell out. A single static binary with a JSON contract is the lowest common denominator and needs zero per-harness integration. One line in an AGENTS.md/CLAUDE.md activates it.

## External contract

Binary and crate: `papercuts` (crates.io name verified free 2026-07-09; bare `papercut` is taken by an image tool). Repo: `treygoff24/papercuts`. License: MIT.

### Commands

```text
papercuts add <TEXT | ->        # file a papercut ('-' reads text from stdin)
papercuts list                  # read papercuts (default: open only, severity-first then newest)
papercuts resolve <ID>...       # mark one or more papercuts resolved (append-only events)
papercuts hook install claude-code # install an opt-in Claude Code failure hook
papercuts hook exec claude-code # silent Claude Code hook target
papercuts schema [all|record|error|exit-codes]   # machine contract, self-orientation
papercuts doctor                # validate the log file (diagnose-only)
```

`log` is an alias of `add` (the verb people will guess first); `add` is canonical.

Global flags: `--file <PATH>` (explicit log file, overrides discovery; relative paths resolve against cwd) and `--pretty` (pretty-print JSON output; no-op for `--format md`). No `--quiet` in v1 (cut — its interaction with data output was underspecified; warnings ride in `meta`, which is already machine-skippable). No color anywhere, ever (agent-only tool; there is nothing to colorize — output is JSON).

### `add`

- **Duplicate-safe, not retry-idempotent** (r3 correction): the content-addressed ID exists for determinism and merge self-healing, NOT as an exactly-once mechanism — a retry at a later wall-clock second produces a new ts, hence a new ID and a second cut, and that is accepted v1 behavior. If the computed ID already exists in the log (fixed-clock tests, post-merge duplicate lines, byte-identical racing adds), nothing is appended and the existing record is returned with `data.changed: false` plus a warning. A caller-supplied idempotency key is an explicit non-goal for v1.
- The ID check and the append happen inside one exclusive-lock critical section (lock → read+fold → decide → append) — two racing identical adds cannot both append.
- `--dry-run`: full discovery, agent resolution, validation, and record construction; reports the would-be record with `data.changed: false`; creates no file and no directory.
- Text is bounded: max 10,000 bytes after trailing-newline strip; larger is `invalid_input` (exit 65).
- Positional `TEXT` (or `-` for stdin; stdin also used when text is omitted and stdin is non-TTY).
- `--agent <NAME>`: reporter identity. Resolution order: flag → `PAPERCUTS_AGENT` env → harness detection (`CLAUDECODE`→`claude-code`, `CODEX_*`→`codex`, `CURSOR_*`→`cursor`) → `"unknown"`. The resolved value AND its source (`flag|env|detected|default`) are echoed in output meta — no silent ambient inference.
- `--tag <TAG>` (repeatable), `--severity minor|major|blocker` (default `minor`).
- Evidence flags are optional: `--cmd TEXT`, `--exit N`, `--stderr-file PATH` (read at filing time and stored as at most 4096 UTF-8 bytes), and `--evidence TEXT`. Evidence is best-effort redacted at write time; never feed raw environment dumps. Evidence is not part of the ID.
- A missing stderr path is `not_found` (66), permission failures are `permission_denied` (77), other read failures are `io_error` (74), and invalid UTF-8 is `invalid_input` (65); no private error codes are emitted.
- A text value beginning with optional whitespace followed by `RESOLUTION` or `RESOLVED` succeeds with a `resolution_text` warning suggesting `papercuts resolve <id>`; it is never blocked.
- Captures `cwd` and repo root automatically (filesystem walk for `.git`; no libgit2).
- Output: success envelope containing the full record + `meta.file` (resolved log path) + `meta.agent_source`.

### `list`

- Filters: `--status open|resolved|all` (default `open`), `--agent`, `--tag`, `--severity`, `--since <RFC3339 | Nd | Nh>`. All filters inspect **cut** fields (`--since` compares the cut's `ts`, never the resolve's).
- `--limit N` (default 50) — bounded output by default; envelope carries `count` (items returned), `total` (matches before limit), `truncated` (`total > count`). The limit slices AFTER the normative sort, so `--limit 1` returns the highest-severity-then-newest match.
- `--format json|md` (default `json`; `jsonl` cut in r3 — one envelope is the contract). `md` is the **sole raw-output exception** in the tool: raw Markdown on stdout, no envelope, warnings as a trailing `> note:` blockquote.
- Empty result is exit 0 with an empty array and a hint in `meta.warnings` — never exit 1. A **missing log file at a discovered default location** is virtual empty state (exit 0, warning `"no papercuts file yet; papercuts add creates it"`); a missing file at an **explicit** `--file`/`PAPERCUTS_FILE` path is exit 66.

### `resolve`

- `papercuts resolve <ID>... [--note <TEXT>] [--agent <NAME>] [--task <ID>] [--pr <URL>] [--commit <SHA>] [--url <URL>|--dropped] [--dry-run]`. One ID keeps the `{changed,record}` output. Two or more IDs return `{changed,records:[...]}` in canonical ID order, with duplicate inputs collapsed. Appends one `resolve` event per open ID; never rewrites history. `--dry-run` reports what would be appended without writing. Output includes `data.changed: bool`. Task, PR, and commit provenance work for either record kind; the mutually exclusive URL and dropped lifecycle flags are dogear-only.
- ID syntax normalization and format validation may happen before discovery and locking. All state-dependent ID/prefix matching, status checks, and append decisions run under one exclusive lock before any event is appended. Any invalid, ambiguous, or missing ID aborts the whole command with no partial append.
- The existence/status check and the append run inside one exclusive-lock critical section — two racing resolves of the same cut yield one `changed:true` and one already-resolved.
- Unknown ID → structured `not_found` error, exit 66, with a hint naming `papercuts list --status all`.
- Already-resolved ID → **idempotent success**, `data.changed: false`, `meta.warnings: ["already resolved"]`.
- Mixed multi-ID resolution reports already-resolved IDs with a deterministic count/list warning; all-already-resolved requests retain `meta.warnings: ["already resolved"]`.
- ID prefix matching (normative): candidates are the distinct folded cut and dogear IDs (first-wins, including resolved records; orphan resolves are never candidates). A prefix is `pc_` optional + ≥4 hex digits, matched case-insensitively. Unique → resolves; ambiguous → `ambiguous_id` error listing full candidate IDs sorted ascending; <4 hex digits → `invalid_argument`.

### `schema`

Prints the full machine contract as JSON: contract version, every command/flag, record schemas, error codes, exit-code dictionary. This is the self-orientation surface; an agent that has never seen the tool runs `papercuts schema` and knows everything.

### `doctor` (v1: diagnose-only)

- Validates the log file: every line parses as a known event, IDs verified by **recomputation** (id must equal the hash of the record's fields); reports torn last line, git conflict-marker lines, unknown kinds, orphan resolves, duplicate cut lines — each as a structured finding `{line, kind, message}` with line numbers. A missing file at a discovered default is healthy-empty (exit 0 + note).
- Conflict-marker detection matches only complete physical marker lines (`<<<<<<< `/`>>>>>>> ` prefixes) — a cut whose *text* mentions conflict markers parses as valid JSON and is never flagged.
- Byte-identical duplicate cut lines are a **warning, not an error** (expected after git concat-merges; `list` folds them first-wins). Same-ID lines with **different payloads** (or an ID that fails recomputation) are an `id_conflict` finding — corruption, not a benign duplicate.
- If the `git` binary is available and the log lives in a repo, warns when the log path is gitignored (the diff-visibility feature silently off).
- **No `--fix` in v1** (review finding: an unguarded quarantine that eats a mis-judged line is worse than no fix; a safe fix path needs backup/undo/dry-run — v2). Exit dictionary: 0 healthy / 1 findings, published in `schema`.

### Envelope and exit codes

Success: `{"ok":true,"data":{…},"meta":{…}}` on stdout, single line (or pretty with `--pretty`).
Error: `{"ok":false,"error":{"code":"…","message":"…","details":{…},"retryable":bool,"suggested_fix":"paste-ready command"},"meta":{"contract":1}}` on **stderr** — the `meta` block (with `contract`) rides on error envelopes too.

Clap integration: `try_parse`; parse failures are rewritten into the error envelope (code `invalid_argument`, exit 2, carrying clap's did-you-mean hint when present). The two documented plaintext exceptions: explicit `--help` and `--version` print clap's human text on stdout, exit 0.

Exit codes follow the rust-agent-cli skill dictionary: 0 success/empty, 2 usage, 65 bad input data, 66 missing file / not-found ID, 70 internal, 75 lock timeout (`retryable:true`), 77 permission denied, 78 config — plus **74 (I/O error) as a documented extension** to the skill table (deliberate deviation, published in `schema`; implementer must not "fix" this back). `std::io::ErrorKind::PermissionDenied` maps to 77/`permission_denied`; other I/O failures to 74/`io_error`. Doctor uses its own published dictionary (0 healthy / 1 findings).

Every envelope (success and error) carries `meta.contract: 1` so consumers can detect contract skew. `schema` output includes an env-var inventory (`PAPERCUTS_FILE`, `PAPERCUTS_AGENT`, `PAPERCUTS_NOW`) and per-command `read_only`/`appends`/`destructive` annotations.

### Record shapes (contract v1)

Cut event:

```json
{"kind":"cut","id":"pc_a1b2c3d4e5f6","ts":"2026-07-09T18:30:00.123Z","agent":"claude-code","text":"rg failed: unquoted zsh glob expanded before rg ran; quote globs or use --files","tags":["shell","rg"],"severity":"minor","cwd":"/Users/x/proj/apps/web","repo":"/Users/x/proj"}
```

Resolve event:

```json
{"kind":"resolve","id":"pc_a1b2c3d4e5f6","ts":"2026-07-10T09:00:00.000Z","agent":"trey","note":"added rg wrapper to CLAUDE.md"}
```

- `id` = `pc_` + first 12 lowercase hex of SHA-256 over the **length-prefixed** field sequence `len(ts) ts len(agent) agent len(text) text len(severity) severity len(tags.join(","))  tags.join(",")` (each len a u32-LE of the UTF-8 byte count; tags sorted) — content-addressed and unambiguous (no delimiter injection), covering every identity-bearing field; evidence is deliberately excluded so two same-instant records differing only in severity/tags get distinct IDs while evidence-only retries deduplicate.

### Materialized output shapes (normative)

`add` data: `{"changed":bool,"record":{cut fields}}`. `resolve` with one ID returns `{"changed":bool,"record":{cut plus resolution}}`; with two or more IDs it returns `{"changed":bool,"records":[cut plus resolution...]}`.
`list` data: `{"items":[ListItem…],"count":N,"total":M,"truncated":bool}` where `ListItem` = all cut or dogear fields + `"status":"open"|"resolved"` + `"resolution":{"ts","agent","note","task?","pr?","commit?","url?","dropped?"}` (present only when resolved; `note` is null when absent, other optional values are omitted when absent, and `dropped` is omitted when false).
`doctor` data: `{"healthy":bool,"findings":[{"line":N,"kind":"torn_line|malformed|unknown_kind|orphan_resolve|duplicate_cut|id_conflict|conflict_marker|gitignored","message":"…"}],"checked_lines":N}`.
`schema` data: the contract object (version, commands with `read_only`/`appends`/`destructive` flags, env vars, error codes, exit codes, record + ListItem shapes). Representative instances of every shape are pinned by deserialization tests.
- `ts` = UTC RFC3339 milliseconds. `PAPERCUTS_NOW` env (RFC3339) overrides the clock for reproducible tests — documented, not hidden.
- Unknown `kind` values are skipped by `list` with a `meta.warnings` count (forward compatibility) but flagged by `doctor`.

## Storage

**Append-only JSONL, event-sourced.** Per the state-and-persistence reference: the check-then-act each mutation needs is serialized by the exclusive file lock (see Concurrency), so JSONL beats SQLite here. `resolve` is an appended event, not a rewrite; **nothing rewrites the file in v1** (the only in-place bytes ever added are appends, including the tear-healing `\n`). `list` folds cut+resolve events into current state at read time — trivial at the scale of a papercuts log (thousands of lines, single-digit ms).

File discovery order:

1. `--file PATH` flag
2. `PAPERCUTS_FILE` env
3. Walk up from cwd to the git repo root; use `<repo-root>/.papercuts.jsonl` (created on first `add`)
4. No repo → `~/.papercuts/log.jsonl`

The per-repo default is the point: the log travels with the repo, and every `add` shows up in `git diff` — that diff is how the log surfaces in review (the green block IS the diff). Teams see papercuts in review for free. This is deliberately committed-by-default (owner decision, review risk acknowledged); the README documents the opt-out (`echo .papercuts.jsonl >> .gitignore` + `PAPERCUTS_FILE`) and recommends `.papercuts.jsonl merge=union` in `.gitattributes` so branch merges concat instead of conflicting. The fold rules below make concat-merges (including duplicated lines) safe.

Repo-root detection treats `.git` as a root marker whether it is a **directory or a file** (worktrees and submodules use a `.git` file).

Concurrency (r3-hardened): mutations may perform syntactic normalization and format validation before discovery and locking; once state is consulted, they open read+append, acquire an exclusive `std::fs::File` lock via **bounded `try_lock` retries** (50 × 100ms; exhaustion → `lock_timeout`, exit 75, `retryable:true`), and run the whole state-dependent read → fold → decide → append sequence inside that one critical section. The append serializes the full line to one buffer and lands it with `write_all`; on a mid-write error the file is truncated back to its pre-append length (we hold the lock and captured the length). If the file is nonempty and its last byte is not `\n`, the writer first appends a lone `\n` — terminating a previously torn fragment so it becomes one skippable malformed line and the new record stays intact (self-healing, never wedged). Reads take a shared lock with the same bounded retries. Durability is best-effort (no fsync per append — documented; a papercut lost to a power cut is acceptable). Advisory locks are only claimed for **local filesystems**; network mounts (NFS/SMB) are documented as unsupported. First `add` creates the file (and `~/.papercuts/` when at the home fallback) race-safely via `create_dir_all` + open `create|read|append`. Empty `PAPERCUTS_FILE`/`PAPERCUTS_AGENT` env values are treated as unset; an unresolvable home directory is a config error (78).

### `list` fold algorithm (normative)

1. Read lines in file order. A final line without a trailing `\n` is **torn**: skip it, count it in `meta.warnings`, never fail the whole read.
2. Lines that fail to parse, or parse to an unknown `kind`, are skipped and counted in `meta.warnings` (forward compatibility; `doctor` reports them with line numbers).
3. `cut` events: **first occurrence of an ID wins**; later duplicates are ignored (this is what makes git concat-merges and idempotent-add races self-healing). Evidence is excluded from the ID, so duplicate-ID adds keep the first cut and do not store later evidence.
4. `resolve` events: mark the ID resolved, recording the **first** resolve's `ts`/`agent`/`note` and optional `task`/`pr`/`commit`/`url`/`dropped` fields. A resolve whose ID has not been seen *by end of file* is an **orphan**: counted in `meta.warnings`, otherwise ignored (a resolve line may legitimately precede its cut line after a merge, so resolution status is computed after the full scan).
5. Sort for output: severity rank (blocker > major > minor), then `ts` descending, then `id` ascending; tags sorted within each record. Same ordering for every format — `md` output is deterministic.

`--since` semantics: relative durations (`Nd`/`Nh`) are computed against the effective now (`PAPERCUTS_NOW` if set, else wall clock UTC). Absolute values must be full RFC3339 with offset (`Z` accepted); date-only input is rejected with a `suggested_fix` showing both forms (ambiguous timezone — reject, don't guess).

## Dependencies (each justified)

- `clap` 4 (derive) — parser, per skill.
- `serde` + `serde_json` — every output shape is a struct.
- `thiserror` — typed public error contract.
- `jiff` — RFC3339 UTC timestamps, parsing `--since`. (Frozen choice — implementer must not substitute.)
- `sha2` — content-addressed IDs.
- `libc` — direct Unix `O_NONBLOCK` flag for safely opening evidence paths before validating the opened handle.
- Dev: `assert_cmd`, `predicates`, `tempfile`.

Nothing else. No tokio, no color crates, no config-file crate, no git library.

## Testing strategy

- Parser unit tests via `Cli::try_parse_from` (conflicts, defaults, bad values).
- Black-box CLI tests via `assert_cmd`: every command's success shape deserialized into its envelope struct; every error path asserts code + exit code + that the `suggested_fix` hint survives (pinned per the error-rewriting craft).
- **Table-driven fold matrix**: adversarial event orderings — resolve-before-cut, orphan resolve, duplicate cuts (identical and id-conflicting), duplicate resolves, torn tail, unknown kinds, interleavings of all of the above — each row asserting folded state + warning counts.
- Concurrency tests: (a) N threads × M distinct `add`s against one file → exactly N×M valid lines; (b) racing **identical** adds → exactly one line, one `changed:true`; (c) racing resolves of one cut → one `changed:true`, one already-resolved.
- Torn-tail self-heal test: truncated final line, then `add` → fragment terminated, new record intact, `list` shows it with one malformed-line warning.
- Discovery precedence tests: `--file` beats env beats walk-up beats home; explicit-missing = 66 vs discovered-missing = virtual empty; `.git`-as-file root.
- Determinism test: two identical invocations with `PAPERCUTS_NOW` fixed against **identical fresh state** produce byte-identical stdout; the fixed-clock retry case is asserted separately (`changed:true` then `changed:false`).
- Quality gate: `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, `cargo build --release`. 5x test sweep before any commit.
- Live acceptance (coordinator-driven): drive the real binary through the full agent lifecycle including empty states, malformed file, ambiguous prefixes, concurrent adds, stdin path.

## Distribution / ship plan

- Public GitHub repo `treygoff24/papercuts`, README written for two audiences: the human installing it, and the agent using it (an AGENTS.md-ready snippet to paste into any repo's agent instructions).
- `cargo install papercuts` as the v0.1.0 install path; `cargo publish` at ship.
- cargo-dist/homebrew/curl-installer deferred to a follow-up release (lens playbook exists; not v0.1 scope).

## Non-goals (v1)

- No server, sync, or telemetry — the file is the product.
- No TUI, no interactive anything.
- No dedup/clustering/AI summarization of cuts (the reviewing agent can do that; this tool is the substrate).
- No Windows CI (nothing platform-specific in the design; just untested).
- No `edit`/`delete` of history — append-only is a feature; nothing rewrites the file in v1 (`doctor --fix` deferred to v2 with backup/undo/dry-run).
- No papercuts-owned config file (the opt-in hook installer only manages an explicit external harness settings file).
- No `--correlation-id` (single-shot local CLI with no logs to correlate — echo-only ceremony; revisit if a long-running mode ever exists).

## Amendments (r2, from adversarial review 2026-07-09)

Reviewers: Cursor (Grok 4.5) `safe`, delivered; Codex GPT-5.6 Sol xhigh unavailable at review time — re-run post re-auth or substituted per lane availability. Triage of all Cursor findings:

**Accepted (folded into the doc above):** torn-last-line handling (single-write append + skip-with-warning on read); idempotent `add` resolving the duplicate-ID/determinism contradiction; normative fold algorithm (first-cut-wins, orphan resolves, post-scan status); `.git`-file root detection (worktrees); exit-74-as-documented-extension; `meta.contract` version on every envelope; `--dry-run` + `changed:bool` on mutations; doctor demoted to diagnose-only (cut `--fix`); doctor gitignore check; `--since` semantics pinned; deterministic md sort; jiff frozen; local-fs-only locking note; best-effort durability note; `merge=union` README guidance.

**Accepted-reduced:** NFS handling = documentation only, no runtime network-fs detection (unreliable heuristics); prefix-resolve stays ≥4 chars but all emitted examples use full IDs.

**Rejected with reasons:** `--correlation-id` (see non-goals); `meta.ignored_by_git` on every `add`/`list` (spawning `git check-ignore` per invocation buys little — doctor covers it); runtime `tempfile` dep (moot — no rewrite path in v1); Windows lock-behavior work (stays a documented non-goal).

## Amendments (r3, from Codex GPT-5.6 Sol xhigh review 2026-07-09)

Second decorrelated review: 1 blocker, 12 major, 1 minor. Triage:

**Accepted (folded in):** F2 mutations lock-then-fold-then-append in one critical section + race tests; F3 `write_all` + truncate-on-error + tear-healing `\n` + 10KB text bound; F4 bounded `try_lock` (50×100ms) → exit 75 retryable; F5 purged all `doctor --fix` ghosts; F6 `add --dry-run`; F7 cut `--format jsonl` and `--quiet`, md documented as sole raw-output exception; F8 normative materialized shapes (ListItem, doctor findings, mutation data) + filter/limit semantics; F9 `meta.contract` on error envelopes, clap `try_parse` rewriting + help/version exceptions, PermissionDenied → 77; F10 severity-first-then-newest is the one normative sort (synopsis fixed, limit-slice pinned); F11 normative prefix candidate set (folded distinct cuts, ≥4 hex after optional `pc_`, case-insensitive); F12 virtual-empty for discovered-missing vs 66 for explicit-missing, race-safe creation, env-empty=unset, no-home=78; F13 conflict markers as physical lines only, `id_conflict` for same-ID-different-payload, doctor recomputes IDs; F14 fold matrix + the concurrency/discovery/format test additions.

**Accepted-reduced:** F1 (the blocker) — the retry-idempotence *claim* was wrong and is retracted; the fix is honest reframing (duplicate-safe content addressing for determinism + merge self-healing), a length-prefixed all-field hash (kills delimiter injection and the fixed-clock severity-collapse), and NOT a caller idempotency key (explicit v1 non-goal — complaint logging does not need exactly-once, and a key registry contradicts append-only simplicity). F3's "refuse appends on missing final newline" reduced to tear-healing (a wedged log with no rewrite path would be strictly worse).

**Rejected:** none — every finding drew blood. Round 2 of plan review closes here (two decorrelated rounds, findings converged from design contradictions to contract precision; residual risk moves to the code-review wave).

## Amendments (r4, Wave 2 implementation 2026-07-15)

Wave 2 adds optional cut evidence without changing the v1 identity or fold rules. A cut may carry `evidence` with optional `cmd`, integer `exit`, `stderr`, and `note` fields; absent fields are omitted during serialization, and stderr is read and redacted in full before its sanitized value is capped at 4096 valid UTF-8 bytes. To keep memory bounded, `--stderr-file` rejects regular files over 1 MiB rather than raw-truncating them before redaction. On Unix it opens the path nonblocking, then validates metadata from that opened handle; symlinks therefore resolve to a regular-file handle when accepted, while a FIFO, device, directory, or a symlink resolving to one is rejected. Evidence strings pass a deterministic best-effort redactor for assignment/header forms involving key, token, secret, password, authorization, and bearer, plus long high-entropy token shapes. It preserves structurally obvious paths and URLs, including repository-relative paths and schemeless hostnames, but this remains heuristic and does not make raw environment dumps safe to submit.

Evidence is not included in the content-addressed ID. Duplicate cut events remain first-cut-wins, and duplicate-ID `add` returns the first record with a `duplicate_cut` warning stating that later evidence was not stored. Resolve events remain first-resolve-wins. Multi-ID `resolve` may normalize and format-validate arguments before discovery and locking, then performs all state-dependent matching, status checks, and append decisions under one exclusive-lock critical section; one ID retains `{changed,record}`, while two or more IDs return `{changed,records:[...]}` in canonical ID order. Validation failures append nothing.

## Amendments (r5, BigCactusLabs/papercuts fork divergence 2026-07-23)

This fork adds a first-class append-only `dogear` record kind: an idea-log channel alongside the friction log, for surprising measurements, empty prior-art niches, and reusable patterns that merit research or writing. `papercuts dogear` (alias `idea`) mirrors `add`'s TEXT/stdin, agent, tags, evidence, file, pretty, and dry-run ergonomics but accepts neither severity nor failure-command evidence. Its record is `{kind:"dogear",id,ts,agent,text,tags,evidence?,cwd,repo}`; `evidence`, when present, is one string. Its identity uses the existing length-prefixed SHA-256 scheme over `ts`, `agent`, `text`, and sorted tags, omitting the cut-only severity field.

`list --kind cut|dogear|all` defaults to `cut`, preserving the existing cut-only output exactly. Dogear lists sort by timestamp descending then ID ascending. `all` retains the existing cut severity-first ordering and then appends the dogear order. Markdown places the latter under `## Dogears`. Common status, agent, tag, and since filters apply to both kinds; `--severity` with `--kind dogear` or `--kind all` is rejected as the existing `invalid_argument` contract rather than silently applying an ambiguous partial filter.

Resolve-prefix candidates include both first-wins kinds, so `resolve` appends the same resolve event for a dogear. The fold recognizes dogear as healthy (not an unknown forward-compat event), and doctor parses dogear records, verifies their IDs, and reports malformed or conflicting dogear lines with the same finding model used for cuts. Schema is the authoritative machine-readable surface for this divergence.

## Amendments (r6, dogear rename + id/durability hardening 2026-07-24)

The idea-log kind is renamed from `angle` to `dogear` (command, `kind` string, `--kind` filter, and the `## Dogears` markdown heading); the `idea` alias is retained. The kind never shipped, so no on-disk migration is required beyond the repository's own dogfood log.

Supersedes r5's dogear identity. **Itself superseded: the `pc_` prefix and `pc1` literal below are pre-rename history. Current code emits `bl_` and hashes `bl1` (r9 renamed the prefix, r10 the literal; see `compute_dogear_id` in `src/lib.rs`).** A cross-model review flagged two seams in the r5 scheme. The dogear id is now an 80-bit SHA-256 digest (`pc_` plus 20 hex) over a `pc1` version literal, the `dogear` kind, `ts`, `agent`, `text`, a tag count, and each sorted-unique tag as its own length-prefixed field. Per-tag framing closes a tag-boundary collision (`["a","b"]` vs `["a,b"]`); the version and kind supply domain separation; and the wider digest, being a different length from the 48-bit cut id, keeps the dogear and cut id namespaces provably disjoint. The released cut id scheme is deliberately left unchanged to preserve byte-compatibility — the identical latent tag-boundary edge case there is deferred to a future breaking release.

The fold and doctor now accept a complete, valid final record that lacks a trailing newline (JSON Lines permits it) instead of ignoring it as torn. This removes an inconsistency in which a valid-but-unterminated tail was skipped by the fold yet resurrected by the next append's leading newline. A final line that does not parse as a record is still reported torn.

## Amendments (r6 addendum, resolve graduation + dogear lifecycle 2026-08-03)

`resolve` gains optional `--task <ID>`, `--pr <URL>`, and `--commit <SHA>` graduation provenance. These flags are valid for cuts and dogears, combine with one another and `--note`, and reject empty or whitespace-only values as `invalid_input`. Resolve events and materialized resolutions carry the optional `task`, `pr`, and `commit` fields; absent values are omitted so legacy resolves retain byte-compatible serialization and older logs remain valid.

Dogears gain two mutually exclusive lifecycle outcomes: `--url <URL>` records the published destination, while `--dropped` records an explicit discard. Both flags are dogear-only. After all requested IDs are matched inside the existing exclusive-lock critical section, a batch containing any cut fails with `invalid_argument` before any resolve event is appended; this prevents a mixed batch from partially resolving. The `url` and `dropped` fields follow the same append/fold contract (`url` omitted when absent and `dropped` omitted when false), and `doctor` treats resolve records containing them as healthy.

## Amendments (r7, chronic-cut triage 2026-08-03)

`papercuts triage [--min-count N]` is a read-only command that discovers the log, takes the existing bounded shared lock, and folds it without changing any bytes. It considers only materialized open `cut` records: resolved cuts and every dogear are excluded. A missing log at the discovered default is therefore an empty scan (`scanned: 0`, exit 0), consistent with `list`.

For clustering, each cut text is lowercased, every non-alphanumeric character becomes a space, whitespace is split, and the resulting tokens are deduplicated. Two cuts link exactly when they share at least one tag (or both have zero tags) and their token-set Jaccard similarity is at least 0.5; two empty token sets have similarity 0. Chronic groups are connected components of that link relation, and their default minimum size is 3. `--min-count` accepts only values of 2 or greater; smaller values return `invalid_argument` (exit 2).

The JSON envelope returns only chronic clusters as `{clusters:[{count,ids,tags,text,suggested_action}],count,scanned}`. Member IDs sort by timestamp ascending then ID ascending; tags are the sorted union; `text` is from the latest member (highest ID breaks a timestamp tie); and every suggested action is `graduate`. Clusters sort by count descending, then oldest member timestamp ascending, then first ID ascending. Exit 0 means no chronic clusters and exit 1 means one or more chronic clusters; these command-specific exit codes are published in `schema`.

## Amendments (r8, Claude Code failure hooks 2026-08-03)

`papercuts hook exec claude-code` is a fail-open hook target for Claude Code `PostToolUseFailure` events. It consumes at most 1 MiB of one JSON stdin payload and silently ignores malformed payloads, non-Bash tools, interrupts, or missing/empty commands. For an eligible failure, it discovers from payload `cwd` (or the process cwd), but acts only when the resolved papercuts log already exists: a hook never creates a log or directory. It appends a minor cut with `auto` and `claude-code` tags, command text/evidence, and a best-effort-redacted error note capped at 4096 bytes. The read → fold → open-command-dedupe → append operation runs inside the existing bounded exclusive lock; only an open cut with exactly matching `evidence.cmd` suppresses a replay, so a resolved command may file again.

Hook execution is the deliberate exception to the normal envelope rule: stdout is always empty and every runtime path exits 0, including lock, filesystem, clock, and payload failures, so a logging problem cannot break a host agent session. `hook exec codex` is also a silent exit-0 no-op because Codex 0.146.x does not expose shell exit status in hook payloads (openai/codex#21753).

`papercuts hook install claude-code [--settings PATH | --global] [--dry-run]` writes an opt-in `PostToolUseFailure` Bash command hook. Its default target is `<repo-root-or-cwd>/.claude/settings.json`; `--global` targets `~/.claude/settings.json`. Existing settings are parsed as JSON values so unknown fields remain intact, malformed existing JSON is rejected as `invalid_input`, and the update is atomic (same-directory temporary file plus rename). Installation is idempotent when any existing `PostToolUseFailure` entry has a command ending in `hook exec claude-code`; a repeat reports `changed:false` and does not rewrite the file. `hook install codex` instead returns `invalid_argument` with a README pointer until failure payloads carry usable exit status.

## Amendments (r9, papercuts → blotter rename 2026-08-04)

The project is renamed from papercuts to **blotter** (design spec: `docs/archive/2026-08-04-blotter-rename-spec.md`, archived 2026-08-11). The binary is `blotter` and the crate is `blotter-cli` (crates.io `blotter` is already taken by a placeholder crate; the `[[bin]]` target is bound explicitly so the installed binary stays `blotter`). This document remains normative under its pre-rename name; everything above this amendment is preserved verbatim, and where names conflict this amendment supersedes.

The environment contract becomes `BLOTTER_FILE` / `BLOTTER_AGENT` / `BLOTTER_NOW` (test-only `PAPERCUTS_BIN` → `BLOTTER_BIN`), with no legacy aliases. When a `PAPERCUTS_FILE`/`PAPERCUTS_AGENT`/`PAPERCUTS_NOW` variable is set and its `BLOTTER_*` counterpart is not, mutating and reading commands emit a `meta.warnings` entry naming the ignored variable; behavior and exit codes are otherwise unchanged.

Discovery order is unchanged with renamed defaults: `--file` > `BLOTTER_FILE` > `<repo-root>/.blotter.jsonl` > `~/.blotter/log.jsonl`. Legacy paths (`.papercuts.jsonl`, `~/.papercuts/log.jsonl`) are never auto-discovered. When discovery resolves to a repo-default or global-fallback path (not an explicit `--file`/`BLOTTER_FILE` path) and the corresponding legacy file exists, commands emit a `meta.warnings` nudge suggesting `mv .papercuts.jsonl .blotter.jsonl` and the matching `.gitignore`/`.gitattributes` edits. This is a warning, never a doctor finding, and introduces no new error codes.

New records emit `bl_`-prefixed IDs. Cut ID derivation is otherwise untouched (only the prefix string changes); the dogear hash domain tag moves `pc1` → `bl1`, so new dogear digests differ from legacy ones. Legacy `pc_` IDs are accepted as input read-only, forever, and never emitted for new records. `resolve` is namespace-aware: an explicit `pc_`/`bl_` prefix constrains matching to that namespace, bare hex searches both, and a bare-hex prefix matching more than one record — within one namespace or across both — remains the existing multiple-match error. The fold keys by full stored ID string and is unaffected; the accepted consequence is no cross-era dedup for identical re-added text. `doctor` recomputes IDs for `bl_` records only and surfaces skipped `pc_` records as an informational `legacy_records` count in its envelope (published in `schema`, not a finding, not exit-affecting).

Envelope `meta.contract` bumps 1 → 2. `hook install` gains stale-path repair: a managed `hook exec claude-code` command whose embedded executable path differs from the current one is atomically replaced and reported as `changed:true`, instead of being left stale behind an idempotency check.

## Wave plan

Reduced multi-agent config: one lane authors and fixes, a cross-family lane reviews, and the coordinator independently gates and reads the riskiest files.

- **Plan review** (this doc): author-family and cross-family reviews in parallel; coordinator triages all findings in writing; doc amended.
- **Wave 1 — the whole CLI, one lane** (task-clustering: ~1000 LOC sharing one design; splitting would fragment coherence). Layout per skill: `main.rs`/`cli.rs`/`commands/`/`output.rs`/`error.rs`/`lib.rs`/`tests/`.
- **Review wave**: cross-family adversarial review of the diff + coordinator riskiest-file read (locking/append path, ID fold logic in `list`, torn-line handling). Triage → author fix round → coordinator verifies every fix landed → re-review until dry (3-round cap).
- **Acceptance**: coordinator drives the real binary. Zero unexplained failures.
- **Ship**: README/AGENTS.md, GitHub repo + push, tag v0.1.0, `cargo publish`.

## Amendments (r10, cut-ID framing breaking release 2026-08-05)

Supersedes r9's statement that cut ID derivation is otherwise untouched. Cut identity now uses the dogear-style framed sequence: the `bl1` version literal, `cut` kind, `ts`, `agent`, `text`, `severity`, decimal tag count, and each sorted-unique tag as its own u32-LE length-prefixed UTF-8 field. The SHA-256 digest remains its first 6 bytes (48 bits, 12 lowercase hex after `bl_`), preserving the cut/dogear width-disjointness argument; the kind field adds domain separation. Duplicate tags no longer perturb cut identity, matching dogears.

Doctor recomputes `bl_` cuts with the r10 scheme. If that fails but the frozen comma-joined v1 recomputation matches, it counts the record in `legacy_records` rather than emitting `id_conflict`; only IDs that match neither scheme are conflicts. Envelope `meta.contract` bumps 2 → 3, and the crate is 0.8.0. The r6 amendment's historical `pc1` literal is stale post-rename; the code literal is `bl1`.

## Amendments (r11, triage representative clustering + occurrences 2026-08-06)

Supersedes r7's connected-component partitioning. Chronic clusters are now built around stable representatives: candidates sort by timestamp ascending then ID ascending; the earliest unclaimed candidate is a representative, and its members are every later unclaimed candidate **directly** linked to it. There is no transitive closure — an A~B~C chain no longer merges A and C through B. Members are claimed only when the cluster reaches `--min-count`; a sub-threshold group leaves its members free to join a later representative, so a real chronic cluster is never suppressed by an earlier near-miss.

The r7 link rule gains one override: two candidates with equal, non-empty normalized titles always link, regardless of tags. Exact title recurrence is the strongest chronic signal and disjoint tag sets must not suppress it (TASK-10). The empty normalized title (text of only non-alphanumerics) never links on this rule. Otherwise the r7 rule is unchanged: shared tag (or both untagged) and token-set Jaccard ≥ 0.5.

Each cluster adds an `occurrences` field: the number of scanned open cuts whose normalized title equals that of the cluster's displayed `text` (the latest member, so the count and the text describe the same recurrence). This is a recurrence signal, not ID deduplication — independently materialized `pc_`/`bl_` records with matching titles all count (r9's no-cross-era-dedup stands). Cluster member ordering, tag union, sorting, exit codes, and `suggested_action` are unchanged from r7; same-input output and exit codes may differ from r7 semantics. Provenance: 2026-08-06 cross-model review (Codex) of the triage correctness batch.

## Amendments (r12, 0.9.0 breaking bundle 2026-08-06)

Envelope `meta.contract` bumps 3 → 4; the crate is 0.9.0. The changes below are the normative contract wherever they contradict earlier sections or amendments.

`resolve` now returns `{changed,records:[...]}` for every invocation, including a single ID — superseding the one-ID `{changed,record}` shape stated in the resolve section, the output-shape table, and r4. Consumers no longer branch on `record` versus `records`.

Stored records persist the sorted, deduplicated tag set for both cuts and dogears, matching what the identity hash covers; the fold normalizes duplicate tags in old records. The stored `repo` field is removed: new records carry a repository-relative `cwd` when the record's cwd is inside the discovered repository, and an absolute `cwd` otherwise (global logs, hook payloads outside the repo). Existing records with absolute `cwd` and `repo` fields still fold and resolve.

`schema` no longer parses `BLOTTER_NOW`; its envelope is static, so an invalid clock exits 0 there while every other command retains clock validation (exit 78). The pre-rename migration surface is removed — superseding r9's stale-env warnings, legacy-path nudges, and doctor `legacy_records`, and r10's v1 recomputation fallback: `PAPERCUTS_*` variables are ignored, `.papercuts.jsonl` is never probed, and doctor verifies `bl_` IDs against the r10 scheme only. Legacy `pc_` records remain opaque data that folds, lists, and resolves by explicit prefix, forever. The `codex` hook target (r8-adjacent) is removed until Codex exposes shell exit status.

Evidence redaction is narrowed to a best-effort hygiene pass and is explicitly not a security boundary — superseding r4's redaction scope. It keeps direct sensitive-key `=`/`:` assignment values (with `authorization` covering the token after a scheme word), HTTP(S) URL userinfo, and one mixed-case-and-digit entropy rule; key-segment inference, per-scheme parsing, `*_file`/`*_path` handling, CLI option values, structural path/URL exceptions, and escaped-quote/fullwidth parsing are dropped.

A scan-layer clarification, not a break: one trailing newline terminates the log (a file of exactly `"\n"` has no lines), but a blank line following any record is a malformed physical line, as it was pre-0.9.0. Provenance: 2026-08-06 cross-model reviews (Claude Opus, Codex) of the 0.9.0 batch.

### r13 (2026-08-08, 0.10.0 additive bundle)

Every change below is additive: envelope `meta.contract` stays 4 and existing logs, commands, and output shapes are unchanged. Shipped in crate 0.10.0.

- `hook install` now preserves existing settings key order via `serde_json` `preserve_order` (issue #13); write remains atomic and content-preserving. The crate deliberately allows `clippy::result_large_err`: `preserve_order`'s IndexMap grows `serde_json::Value` past the lint threshold, and boxing 53 `AppResult` signatures is not worth it for a short-lived CLI.

- `BLOTTER_HOOK_EXPLAIN=1` opts `hook exec claude-code` into a best-effort one-line stderr diagnostic; stdout remains empty and exit remains 0. `schema` publishes the Claude Code payload fields, 1 MiB stdin cap, and eligibility gates (issue #14). The diagnostic also covers pre-dispatch failures: an unusable `BLOTTER_NOW` reports `clock could not be resolved` instead of exiting silently.

- Resolutions are correctable through appended `resolve --amend` events: the first non-amend resolve remains the base, the latest amend wins the materialized user-set fields, superseding the write-once reading of L135/L204 while the append-only invariant remains intact (issue #12).

- `digest` is a read-only periodic friction report: JSON combines chronic open-cut clusters, windowed open cuts grouped by tag, and all open dogears. Its `--format md` output is a raw Markdown exception shared with `list`, superseding the earlier “sole raw-output exception” wording (issue #7). Its raw Markdown output carries discovery and fold warnings as trailing `> note:` lines.

- `sweep` aggregates explicitly listed repositories read-only. It has no blotter-owned config file: any registry is a user-owned file passed per invocation, and `BLOTTER_FILE` is ignored (issue #3). Per-repository lock timeouts and unreadable logs become skip warnings with exit 0, a deliberate sweep-scoped exception to the exit-75 contract. The global `--file` flag conflicts with `sweep`; list repository paths directly or use `--registry FILE`.

### r14 (2026-08-08)

Additive: envelope `meta.contract` stays 4.

- `hook exec claude-code` skips a failed command longer than 500 UTF-8 bytes, a new eligibility gate published in `schema` as `tool_input.command_bytes`. The command becomes the cut's text verbatim, so a long debugging one-liner produces an entry that dilutes the log rather than describing friction; measured against this repository's own log, filed cuts have a p90 length of 235 bytes and the sole entry above 300 was a 713-byte hook-filed shell command. The gate precedes `add::validate_text`, whose empty and 10000-byte rules the hook gates now subsume; the shared validator is retained as a backstop and stays authoritative for cut text. Skipping remains fail-open: stdout empty, exit 0, one `BLOTTER_HOOK_EXPLAIN` line naming the gate.

### r15 (2026-08-08, 0.11.0 doctor --fix v2)

`doctor --fix` adds a bounded repair path. The only fixable findings are `torn_line`, `malformed`, and `conflict_marker` (the malformed subclass); each removes and quarantines the physical line. A valid final record without a trailing newline is already healthy, so a `torn_line` always contains an invalid fragment: appending `\n` would only convert it into `malformed`, not make the log healthy. One quarantine mechanism therefore handles all three kinds. `unknown_kind` remains diagnose-only for forward compatibility; `orphan_resolve` remains harmless to the fold and can result from merge ordering; `duplicate_cut` and `duplicate_dogear` remain first-wins fold warnings where compaction is not worth a rewrite; `id_conflict` requires a human to choose the true payload; and `gitignored` lives in `.gitignore`, not the log.

`doctor --fix --dry-run` reads under the normal shared lock, plans the applicable actions, reports `changed:false`, and writes nothing. `--dry-run` without `--fix` is `invalid_argument`. Apply mode takes the existing exclusive lock, re-reads and re-inspects inside that critical section, and reports the post-fix diagnosis. Every repair copies the original bytes to `<log>.bak-<YYYYMMDDTHHMMSSmmmZ>` using the effective clock, appends the removed physical lines verbatim (with trailing newlines) to `<log>.quarantine.jsonl`, writes a repaired same-directory temporary file, fsyncs it, and atomically renames it over the log; directory fsync is best-effort. A pre-existing backup path is an `io_error` with no log change. The success envelope includes the backup and a paste-ready restore hint whenever a backup was made.

Lock acquisition re-verifies path identity after locking, so writers serialized behind `doctor --fix` always land on the post-repair file; backup and quarantine files are fsynced before the swap, while directory fsync remains best-effort.

The append-only invariant is amended: nothing rewrites the log file EXCEPT `doctor --fix`, which never edits in place — it writes a repaired copy and atomically swaps, always preserving the original as a timestamped backup. `Finding.fixable` is always serialized. In `schema`, the base `doctor` command remains `read_only:true`; its `fix` sub-entry documents `--fix` as the destructive, conditional mode so the normal diagnosis contract stays explicit.

### r16 (2026-08-09, 0.12.0 verify recurrence)

Additive: envelope `meta.contract` stays 4. `blotter verify` is a read-only recurrence check: it discovers one log, takes the existing bounded shared lock, and completes one full fold without changing bytes. A missing discovered-default log is an empty scan with exit 0, consistent with `triage`.

Each eligible materialized resolved cut is an independent anchor. A materialized open cut recurs against an anchor only when its cut timestamp is strictly after the anchor's materialized resolution timestamp (the latest amend timestamp when an amend wins) and the pair links under the exact `triage` rule: equal non-empty normalized titles link regardless of tags; otherwise tags overlap (or both tag sets are empty) and the normalized token-set Jaccard similarity is at least 0.5; two empty token sets never link. Dogears, dropped resolutions, and resolved cuts with empty normalized titles are excluded. One open cut can recur against more than one resolved anchor.

The JSON envelope returns `{recurrences:[{resolved_id,resolved_text,resolution:{ts,task?,pr?,commit?},recurrence_ids,count,first_recurrence_ts}],count,scanned}`. `recurrence_ids` sort by cut timestamp ascending then ID ascending; recurrences sort by first recurrence timestamp ascending then resolved ID ascending; `scanned` has the same materialized-open-cut meaning as `triage`. Exit 0 means no recurrences and exit 1 means one or more recurrences; `schema` publishes the command and its exit convention.

Resolve-side recheck fields and a reopen event kind are deliberate deferrals. Verification remains a derived read-only view over append-only cut and resolve events.

### r17 (2026-08-09, 0.13.0 auto-capture default exclusion)

A record whose tags contain `auto` is an auto-capture, including a hand-filed record deliberately given that tag. `list`, `triage`, `digest`, `verify`, and `sweep` exclude auto-captures by default; `--include-auto` restores the prior visibility without widening any other selector. On `list`, `--tag auto` implies `--include-auto`. Each command applies the shared auto predicate to its folded item vector immediately after the fold and before every command-specific selector or analysis, so auto-captures cannot contribute to a list count, a triage cluster, a digest section, a recurrence, or a sweep aggregate unless included explicitly.

When a command drops records only because they are auto-captures after its other selectors have matched, it appends `N auto-captured records hidden; use --include-auto to include them` after discovery and fold warnings, with `N` taken before list truncation. `sweep` emits one such aggregate line over the distinct canonical logs it actually swept, rather than one line per log. `doctor` continues to inspect every physical line, and the hook's open-command dedupe continues to see auto-captures, because neither path is a reporting read. Envelope `meta.contract` bumps 4 → 5 because these five default reads are a behavior break for consumers.

### r18 (2026-08-11, 0.13.1 documentation refresh)

Copy-only. Envelope `meta.contract` stays 5; no selector, fold, ordering, or exit code changes. Two published strings were wrong and are corrected here.

The global exit-code dictionary described exit 1 as `doctor findings`. Exit 1 has meant "findings" for `triage` since r7 and for `verify` since r16, and each of those commands already published its own `exit_codes` entry. The global entry is now `command findings: doctor unhealthy, triage clusters, or verify recurrences`. Per-command entries remain authoritative for which meaning applies; the global entry names the class.

The auto-capture warning from r17 now agrees in number: a count of 1 reads `1 auto-captured record hidden; use --include-auto to include them` and every other count keeps the plural `records`. The rest of the string, its position after discovery and fold warnings, and the pre-truncation count are unchanged from r17.

Neither correction moves a contract boundary, so consumers pinned to contract 5 stay valid. Callers matching the old exit-1 description or the singular warning by exact string will not match; matching on the numeric exit code and the leading count is the supported reading.

### r19 (2026-08-17, issue #22 reworded triage linkage)

This corrects the scoring rule in r7, r11, and r16. Envelope `meta.contract` stays 5, and output shapes, the representative-based clustering algorithm, ordering, and the tag gate are unchanged.

Equal, non-empty normalized titles still link before the tag gate. Otherwise, candidates must share a tag (or both have no tags). For scoring, begin with the normalized token set but remove every token of two or fewer Unicode scalar values and this fixed stopword list: `and`, `are`, `but`, `cannot`, `for`, `from`, `into`, `need`, `one`, `that`, `the`, `this`, `to`, `use`, `uses`, `with`. A scoring path with an empty filtered set never links.

For the remaining sets, candidates link when either their overlap coefficient (intersection divided by the smaller set size) is at least `4/5`, or they share at least three locally rare tokens. A token is locally rare when it occurs in no more than `max(2, ceil(N / 4))` candidates, where `N` is the number of candidates in that analysis. Triage and digest count their analyzed open-cut candidates; verify counts its eligible resolved anchors and open cuts. Frequency is document frequency: one candidate contributes at most once per token. This preserves a strict path for near-duplicates while letting independently worded descriptions link on multiple distinctive tokens.

### r20 (2026-08-17, issue #23 hook probe filter)

Additive: envelope `meta.contract` stays 5. The append-only retention stance remains: no command trims the log in this release. The auto-capture lane is instead bounded at write time. After its existing command gates and before filing, `hook exec claude-code` best-effort matches only the first program word: it skips leading `VAR=value` environment assignments, takes the program basename, and deliberately does not parse pipelines, `&&` chains, or subshells. It skips `grep`, `rg`, `ls`, `find`, `tail`, `head`, `cat`, `stat`, `test`, `[`, `which`, `curl`, and `gh`: read-only interrogation commands whose non-zero exit is an expected answer. The gate remains fail-open (stdout empty, exit 0, and `BLOTTER_HOOK_EXPLAIN=1` names the program and gate) and is published in `schema` as `tool_input.command_program`, alongside r14's `tool_input.command_bytes` gate.

An archive/rotation command is deliberately deferred and tracked as backlog TASK-30.

### r21 (2026-08-17, issues #24/#25 md renderer collapse and resolution rendering)

Raw-output change only: envelope `meta.contract` stays 5 and no JSON shape moves. In `list` and `digest` `--format md` output, every rendered line is whitespace-collapsed as a whole — runs of spaces, tabs, and newlines in any interpolated field (text, agent, tags, timestamps, and all resolution fields, which validation accepts with embedded newlines) become single spaces — so one record is always one list item. For `--status resolved`/`all`, a resolved cut renders a nested sub-bullet: `resolved <ts> by <agent>` plus `(<commit>)`, `pr <pr>`, `task <task>`, and `: <note>` when present. Full uncollapsed detail remains in `--format json`. Callers parsing the raw md by exact line shape need updating; md remains a human review surface, not a machine contract.

### r22 (2026-08-18, issue #28 public-log posture)

Additive: envelope `meta.contract` stays 5. No envelope shape, selector, or exit-code changes: `cwd` remains a descriptive stored field and the new doctor behavior is flag-gated.

New cut and dogear records keep repository-relative `cwd` when the existing discovery rule says they are inside the discovered repository. Otherwise, a cwd under the current non-empty `$HOME` stores as `~` at the home root or `~/…` below it; component-aware path comparison prevents sibling names such as `/Users/alicex` from matching `$HOME=/Users/alice`. That component-aware rule applies only to the exact-home check: `/Users/alicex` still matches the intended generic Unix-home rule and rewrites to `~` (or `~/…` with a suffix). Non-home cwd values remain absolute, and historical stored cwd values are never rewritten.

Evidence redaction first rewrites a current-home path prefix to `~`, then applies the existing span-based best-effort secret pass. Generic `/Users/<user>` and `/home/<user>` prefixes use the same boundary rule so imported stderr and other-machine paths receive the same treatment. The rewrite applies to add command, stderr, and note evidence, including the hook failure-note lane; it is hygiene, not a security boundary.

`doctor --leaks` scans the raw bytes of every physical line, valid or not, for the current absolute home path and those generic Unix home patterns. A leaking line receives a diagnose-only, non-fixable `leak` finding. `--leaks` (and therefore `--deny`) conflicts with `--fix`: the leak gate is read-only, because repairing a malformed leaking line would otherwise move the flagged bytes to backup/quarantine while clearing its diagnosis. Repeatable `--deny <literal>` adds one diagnose-only leak finding for each matching literal on a physical line and requires `--leaks`. This preserves doctor’s existing finding exit convention and leaves unflagged output byte-identical. Configurable per-repository deny-pattern files are deliberately rejected: deny values are supplied per invocation, matching sweep’s no-config philosophy.

### r23 (2026-08-18, dash-encoded home slugs)

Additive: envelope `meta.contract` stays 5. No envelope shape, selector, exit-code, or flag changes.

Harness scratchpad and session paths embed the home directory in a dash-encoded slug, such as `/private/tmp/<session>/-Users-<user>-<repo>/…`. Evidence redaction and the `doctor --leaks` raw-line scan both recognize this form alongside the r22 slash forms, with three rules sharing one precedence order:

1. **Exact current home, slash form** (unchanged from r22).
2. **Exact current home, dash-encoded**: the current `$HOME` with every `/` replaced by `-`. This rule outranks the generic rule so a dash inside the username (for example `$HOME=/Users/jane-doe`, slug `-Users-jane-doe-<repo>`) redacts the whole encoded home rather than truncating after its first dash-separated component, and so a home outside `/Users` and `/home` (for example `/var/root` as `-var-root-…`) is still caught.
3. **Generic dash prefixes** `-Users-<user>-` and `-home-<user>-`, mirroring the generic slash prefixes: the user component is the first run of bytes after the prefix up to the next dash, slash, or evidence delimiter, and must be non-empty.

Boundary rules per form: a slash-form generic match still rejects a preceding `/` (nested paths such as `/mnt/home/shared` stay unflagged); a dash-form match requires token start, a preceding evidence delimiter, or a preceding `/`, because a slug normally follows a path separator, and bare mid-token hits such as `dir-Users-x` stay unflagged. Redaction replaces the matched home prefix with `~` and keeps the rest of the token verbatim, exactly as the slash forms do.

Known limitation, accepted: for a foreign dashed username (another machine's slug, not the current `$HOME`), the generic rule cannot know where the username ends — `-Users-jane-doe-<repo>` from someone else's machine redacts only `jane`. Detection still fires on the prefix, so `doctor --leaks` flags the line either way; only the rewrite is component-bounded. The exact-home rule closes this for the local user, which is the leak the feature exists for.

### r24 (2026-08-18, TASK-24 hook source provenance)

Additive: envelope `meta.contract` stays 5. Cut records gain an optional `source` field, serialized only when present; the sole writer is `hook exec claude-code`, which stamps `"hook"`. `add` cannot set it: absence means self-report, and provenance stays unforgeable through the normal filing lane because self-narration and machine-observed telemetry are different evidence classes. The field is descriptive, not identity: `compute_id` ignores it, no selector keys on it, and tags remain the filtering surface. Stored history is unchanged and unknown stored values pass through opaquely.

### r25 (2026-08-18, TASK-36 write-time text redaction)

Additive: envelope `meta.contract` stays 5. No envelope shape, selector, exit-code, or flag changes.

Record text joins the redaction surface at write time. Authored `add` text and `dogear` text use the r22 slash-form and r23 dash-form home rules only — same precedence and boundary rules — before anything else consumes the text. The span-based secret pass does not apply to those authored descriptions: the entropy rule's false positives cost more there than they save.

The hook auto-capture's command-derived text is machine-captured command bytes, not an authored description. After its eligibility gates judge the raw command, both hook `text` and `evidence.cmd` receive the full evidence redaction: home-path rewrite first, then the span-based secret pass. This keeps the stored command fields, identity hash, and open-command dedupe on the same sanitized bytes.

Ordering is normative: text is redacted first, then validated, then the identity hash is computed over the redacted text. The stored record and its ID therefore describe the lane's redacted text: a caller that quotes a home path receives an ID for the redacted spelling, byte-identical inputs on the same clock still produce byte-identical records, and raw texts that differ only inside a rewritten home prefix now store the same text and collide as duplicates, which is the desired dedupe behavior. `validate_text`'s 10000-byte limit measures the redacted text, so an oversized raw input that redacts below that limit is accepted (subject to any earlier lane-specific raw gate). The hook lane redacts after its eligibility gates, which continue to judge the raw command; its open-command dedupe compares redacted text against stored text. Hook records stored before r25 keep their raw text, so a recurring command can file one further duplicate beside a pre-r25 open record before dedupe re-engages on the newly redacted record; this is accepted. Stored history is never rewritten.

The hook failure-note bound tightens from 4096 to 1024 bytes, redact-then-truncate as before. Auto-ingested command output re-ingests surrounding prose that no redaction rule can recognize; a 1 KiB head keeps the diagnostic value without archiving the transcript. The `add` command's user-supplied `--stderr-file` evidence keeps its 4096-byte bound: user-curated evidence is a deliberately different trust lane. Resolution `--note`/`--amend` fields remain unredacted at write time — a named deferral, not an oversight; `doctor --leaks` still scans every physical line.

### r26 (2026-08-18, TASK-30 archive retention)

Additive: envelope `meta.contract` stays 5. `blotter archive --before <value>` trims closed history by copy-and-swap, the mechanic r15 established for `doctor --fix`; the append-only invariant now names both commands as its only exceptions. `--before` accepts exactly the `--since` value grammar, cutoff exclusive. A group archives only when its materialized state is resolved or dropped and every event in the group predates the cutoff; open records never archive, and orphan resolves, malformed lines, unknown kinds, and legacy `pc_` records stay in the log verbatim. Removed physical lines land as verbatim newline-terminated physical lines in original order in `<log>.archive-<ts>.jsonl`, alongside the r15-style timestamped backup of the original; nothing eligible means no rewrite, no files, `changed:false`, exit 0. `--dry-run` plans under the shared lock and writes nothing. `schema` documents apply mode as destructive and conditional, exactly as `doctor --fix`.

Two properties of the shared copy-and-swap, binding on `archive` and `doctor --fix` alike: a final-component symlink in the log path is resolved before the backup, sidecar, and replacement paths are derived, so the swap lands on the link's target and the link survives — parent components keep their spelling, leaving envelope paths unchanged for regular files. And a log holding exactly one newline has zero physical lines, per the scan contract, so it archives as zero kept and zero archived.

### r27 (2026-08-18, TASK-23 retrospect promotion mining)

Additive: envelope `meta.contract` stays 5. `blotter retrospect` is a read-only mining pass that folds chronic signal into typed promotion candidates for a human to judge: it reuses triage's clustering and verify's recurrence rules unchanged, types each open-cut cluster by deterministic evidence shape — half-or-more shared failing program becomes `wrapper_alias`, half-or-more `docs`-tagged members becomes `doc_repair`, first match wins — and every recurrence of count two or more becomes a `skill_candidate`, because a resolved-then-returned cut is a recovery sequence worth promoting. Clusters matching no rule stay ordinary cuts and emit nothing. The envelope carries bounded evidence — capped member texts and resolution notes, never evidence command, stderr, or note fields — so an external agent can reason over a small package while the CLI itself never writes a doc, skill, or alias; the human gate is the product. Retrospect deliberately includes auto-captures, inverting r17's default for this one command: the repeated-command-failure signal it mines lives in the auto lane. Retrospect takes no window: chronic signal is long-horizon by design. Exit 1 signals candidates, exit 0 none, matching triage's convention; the global exit-code dictionary entry for 1 becomes `command findings: doctor unhealthy, triage clusters, verify recurrences, or retrospect candidates`, superseding the r18 wording. A cluster candidate's `occurrences` is the sum of its members' r11 title occurrence counts with each distinct normalized title counted once, because members sharing a title each carry the same global count.

### r28 (2026-08-18, TASK-25 OTel export bridge)

Additive: envelope `meta.contract` stays 5. `blotter export --format otlp-json` is a read-only batch bridge from folded cuts to one OTLP `LogsData` JSON object on stdout — a raw-output exception alongside `--format md`, one newline-terminated line, zero warning channel. The mapping targets OTLP 1.11.0 JSON encoding and the OTel file-exporter JSONL format as a compatibility snapshot, not a dependency: blotter's own record schema stays internal, the outward mapping lives in one versioned module, and no OpenTelemetry crate is added. Each cut becomes a log record with top-level `eventName` `blotter.friction.reported`, decimal-string `timeUnixNano`, mapped severity, the text as body, and typed `blotter.friction.*` attributes; evidence fields are never exported, dogears are out of scope, auto-captures follow r17's default exclusion with `--include-auto`, and trace identity and `schemaUrl` are deliberately absent. GenAI semantic conventions remain unadopted: they are development-status with no pinnable schema as of August 2026, which is the reason the bridge maps outward from an owned schema rather than storing a foreign one.

Timestamp policy: OTLP types `timeUnixNano` as an unsigned fixed64, so a selected record whose timestamp falls outside the unsigned 64-bit nanosecond range (pre-1970, or beyond the u64 ceiling) rejects the whole export with `invalid_input` (exit 65), naming the offending record ID and timestamp — never a partial export, a silently skipped record, or a zero placeholder. Argument validation precedes environment resolution: a bare `export` without `--format` reports `invalid_argument` before the clock is read, so an unusable `BLOTTER_NOW` cannot mask a missing flag.


### r29 (2026-08-19, hook chain-shape gate)

Additive: envelope `meta.contract` stays 5. `hook exec claude-code` gains one eligibility gate, evaluated after r14's byte gate and before r20's program gate, and published in `schema` as `tool_input.command_shape`.

A command that chains or substitutes is skipped. The gate scans the raw command bytes with single- and double-quote state (backslash escapes honored inside double quotes only) and skips the capture when `&&`, `||`, `;`, `|`, a newline, `$(`, or a backtick appears outside quotes. Bare `&`, heredocs, `$'…'`, and nested substitution are deliberately not recognized; as in r20 the hook does not parse the shell, and an ambiguous scan resolves toward skipping. A scan that reaches the end of the command still inside a quoted span — an unterminated quote, or a trailing backslash inside a double-quoted one — is exactly that ambiguity, because the operators it may hide cannot be ruled out, and skips.

The rationale is r14's, generalized. The failed command becomes the cut's text verbatim, and a chain's non-zero exit names neither the failing step nor the friction: the Claude Code payload carries no per-segment status, so the stored text is an unreadable one-liner rather than a description. Measured against this repository's own log, every one of the 25 auto-captures filed to 2026-08-18 was a chain, r20's program gate matched none of them, and normalizing paths, quoted strings, and integers produced 17 distinct fingerprints from the 17 records filed after r20 — chained failures here are one-shot novelty, not repetition, so neither an extended program list nor fingerprint dedup bounds the lane. Restricting auto-capture to simple commands accepts a much smaller lane in exchange for entries whose text describes what failed. Skipping remains fail-open: stdout empty, exit 0, one `BLOTTER_HOOK_EXPLAIN` line naming the gate. No selector, output shape, or exit code changes, and stored history is untouched.

### r30 (2026-08-19, cwd redaction parity)

Corrective: envelope `meta.contract` stays 5. No selector, output-shape, or exit-code change.

`record_cwd` implemented only the exact-`$HOME` strip, although r22's own cwd paragraph already ruled that a sibling such as `/Users/alicex` falls to the generic Unix-home rule. A cwd under a generic home that is not the current `$HOME`, or under an r23 dash-encoded harness slug, was therefore stored verbatim, and `doctor --leaks` flagged a line blotter itself wrote — with no repair path, because `--leaks` is diagnose-only and conflicts with `--fix`, the record is already appended, and no event rewrites a stored payload. A tool whose own writes trip its public-log gate is the defect, not the gate.

The stored `cwd` therefore joins the redaction surface at write time, under the same whole-string `rewrite_home_paths` scanner that evidence already uses and in the same precedence order: r22 exact current home, r22 generic `/Users/<user>` and `/home/<user>`, then r23's dash-encoded forms with their boundary rules unchanged. The repo-relative branch is untouched and still decides first — a cwd inside the discovered repository stores as a repo-relative path and never reaches the scanner. The span-based secret pass does not apply, as it does not apply to authored text under r25.

The resulting spelling is accepted deliberately. The r23 rule rewrites only the matched prefix and keeps the rest of the token verbatim, so a dash-encoded cwd stores as `/private/tmp/claude-501/~-Documents-GitHub-blotter/<session>/scratchpad` rather than collapsing the whole slug. It looks odd, and it is still the better answer: a whole-token rule for `cwd` alone would make one path spell two different ways depending on whether it arrived as a working directory or as evidence, and one scanner with one output is worth more than a tidier string.

What does not move: `compute_id` ignores `cwd`, so IDs, dedupe, and the determinism guarantee are unaffected. The change is write-time only and stored history is never rewritten, so an already-leaking `cwd` stays as written and `doctor --leaks` keeps reporting it, correctly, as a line that predates the fix. `schema`'s published `cwd` description is updated to name the redaction.

### r31 (2026-08-19, exit-contract closure and the raw stdin gate)

Corrective and additive: envelope `meta.contract` stays 5. No output shape, selector, or flag changes. This amendment records the contract-visible results of a sweep for paths that terminate outside `ERROR_CONTRACT`, and one new lane gate.

**The log path must be a regular file.** Every command validated the log's *contents* but never its *type*, so two inputs escaped the contract entirely: a FIFO blocked the open until a writer appeared — no exit code, no bytes on either stream, which for an agent consumer is an unsignalled stall rather than an error — and an endless character device grew the read buffer until allocation failed, which under `panic = "abort"` is a bare abort. The type is now checked by `fstat` on the opened handle, inside the locking helper, before the lock is attempted and before any read; a non-regular file is `invalid_input` (exit 65). The check precedes the lock deliberately: `flock` on a FIFO reports `ENOTSUP` on some platforms, which would answer with the wrong code, and a check after the lock would first spend the whole bounded retry budget on a path that can never be valid. On Unix both log opens also set `O_NONBLOCK`, which does not reject anything by itself — it only stops the open from blocking so the type check can run. This makes the log path agree with `--stderr-file`, which has rejected FIFOs and devices since r4. Two consequences are deliberate: a log path of `/dev/null` was an empty read at exit 0 and is now exit 65, and a log path naming a directory was `io_error` (74) and is now the same `invalid_input` (65) that `--stderr-file` already returned.

**`add -` and `dogear -` gate the raw stdin read at 1 MiB.** r25 fixed that `validate_text`'s 10000-byte limit measures the *redacted* text, so it cannot bound the read: input that redacts below the limit is accepted however large it arrived. The parenthetical there anticipated a lane-specific raw gate; this is it, at the same 1 MiB as `--stderr-file` and the hook payload. The budget is on bytes **read**, and is therefore measured before any trailing newline is trimmed — trimming first would leave a hole exactly at the boundary, where a stream of the full limit followed by a newline and then more data fills the reader to its cap, loses the newline to the trim, and is accepted while everything the reader never reached is silently discarded. Oversize input is `invalid_input` (exit 65) naming the read limit, reported before UTF-8 decoding so a stream cut mid-codepoint at the cap does not answer with a misleading encoding error. Trailing newlines are still stripped from accepted input, and r25's rule is unchanged: text is redacted first, then validated, and oversized raw input that redacts below 10000 bytes is still accepted. `schema` publishes the cap on the `add` and `dogear` positional.

**A materialized response must agree with a later fold.** r13 gives the base resolve to the first non-amend event and the materialized user-set fields to the latest amend. `resolve` reports its result from the fold that made the append decision rather than re-reading, so it must apply that same rule to the event it is about to append: an amend carrying a clock earlier than a stored amend does not win, and the response reports the stored winner. Only an exact timestamp tie falls to the appended event, as the last in file order. `--dry-run` predicts through the identical rule, so a plan cannot promise a resolution the apply would not produce. Reachable whenever the clock moves backwards, which `BLOTTER_NOW` makes ordinary.

**Three exit codes corrected, one ordering aligned, one writer invariant closed.** A missing `--registry` file is `not_found` (66), not `io_error` (74): it is as explicit a user-supplied path as `--file` and `--stderr-file`, which have always answered 66. A `BLOTTER_AGENT` that is not valid UTF-8 is `config_error` (78) instead of being discarded in favour of harness detection, matching how `BLOTTER_NOW` has always answered for the same case — silently filing under a *detected* agent contradicted the operator's own configuration while reporting success. `list` and `export` validate `--since` before discovering the log, so an invalid value answers `invalid_argument` (2) in every command that accepts the flag; this is r28's stated principle, which `digest`, `archive`, and `sweep` already followed. And `hook exec` no longer appends a second physical line carrying an existing cut ID: it now applies the same first-wins identity guard as `add` and `dogear`, so one ID means one line for every writer. That collision is only reachable under a frozen clock, where a resolved command replayed at the same instant recomputes the same ID; a resolved command filed at a later instant still refiles, as r8 and r25 intend. The lane stays fail-open, and `BLOTTER_HOOK_EXPLAIN=1` names the skip.

`export`'s raw stdout path also moved onto the shared writer every other stdout producer uses, so a failed write there is reported as `io_error` (74) rather than suppressed. That writer exists so a redirected descriptor surfaces its write errors and an interactive Windows console still renders non-ASCII; `export --format otlp-json` was the one path that had opted out of both halves. Nothing about the OTLP mapping in r28 changes.

### r32 (2026-08-19, retiring the claude-code auto-capture lane)

Subtractive: envelope `meta.contract` stays 5. No envelope shape, selector, or exit-code changes for any read command.

**The auto-capture write lane is retired.** `hook exec claude-code` no longer files cuts, and `hook install claude-code` is removed. Nothing in blotter writes a record tagged `auto` any more; the friction channel is the manual `add`/`dogear` lane and nothing else.

The lane never earned its keep. Over roughly ten days of dogfooding it filed 27 captures into this repository's own log, and no consumer ever read them: r17 hid them from `list`, `triage`, `digest`, `verify`, `sweep`, and `export` by default, and the one command that opts in — `retrospect`, per r27 — mined nothing from them, because r29's own measurement already showed the captures were one-shot novelty rather than repetition. What the lane stored was a failed command line with no statement of why the failure mattered: uninterpreted transient noise, which is the opposite of the thing this tool exists to collect. A cut is a claim that something got in the way; a non-zero exit is not that claim, and no gate can turn one into the other. r14's byte gate, r20's program gate, and r29's chain-shape gate were each an attempt to filter signal out of the exhaust, and each narrowed the lane without ever making a captured record worth reading. Beyond the noise, the lane carried costs the retirement also closes: it was the write path most likely to store an unredacted local path or secret (r25, r30), it paid a full fold inside the exclusive lock on every failed Bash call in the host session, and it depended on a settings-file entry naming an absolute executable path that drifts whenever the binary moves.

**Read-side `auto` filtering is retained, unchanged.** The log is append-only, so every `auto` record already written stays written, and a read surface that stopped honouring the tag would surface a decade of stale exhaust the moment this ships. `is_auto_capture`, the default exclusion in `list`, `triage`, `digest`, `verify`, `sweep`, and `export`, the `--include-auto` flag on each of them, `--tag auto` implying `--include-auto` on `list`, and `retrospect`'s inverted default from r27 all survive exactly as written. They now describe legacy history rather than a live lane, and that is the only thing about them that changes. `add` still cannot set `source`; the field stays readable and opaque, and `hook` is now a value only stored history carries.

**`hook exec claude-code` survives as a retired no-op; `hook install claude-code` does not.** The two halves are answered differently on purpose, by the fail-open rule the lane has had since r14. `hook exec claude-code` is fired by a harness, not by a person: a settings file installed against an older binary keeps firing that exact command line after this ships, and a clap rejection there would put a usage error and a non-zero exit into a host session's hook channel — a logging feature breaking the session it was meant to observe, which is precisely what fail-open forbids. The retired command therefore reads and discards stdin under the same 1 MiB bound, writes nothing to stdout and nothing to the log, and exits 0. It resolves no clock, discovers no log, takes no lock, and resolves no agent, so no environment fault can reach it. `BLOTTER_HOOK_EXPLAIN=1` still writes exactly one stderr line, now naming the retirement, so an operator debugging a still-installed hook is told why it stopped filing rather than left with silence. `hook install claude-code` is the opposite case — a deliberate operator invocation whose whole purpose was to create the installation this amendment retires — so it is removed outright and clap rejects it as an unknown subcommand with `invalid_argument` (exit 2). Removing the installer while keeping the receiver is the asymmetry the fail-open rule asks for: stop making new installations, never punish an existing one.

Operators should delete the `hooks.PostToolUseFailure` entry naming `blotter hook exec claude-code` from their Claude Code settings. Nothing in blotter removes it, because blotter no longer writes another program's configuration at all.

`schema` republishes `hook` with the `exec` no-op contract and no `install` entry, drops the whole `payload`/`gates` block, and records the `source` field as legacy-only. The README's hook gate prose and the test that gated it against the published gate list go with the gates they described.
