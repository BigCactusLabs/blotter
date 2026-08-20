# Scale baselines: fold and analyzers

Date: 2026-08-18. Status: baseline plus TASK-29.3 and TASK-29.2 results, then
the 2026-08-19 verify recurrence-scan results.

## Scope

This records release-mode baselines before TASK-29 changes `fold_bytes`,
`resolve`, `triage`, or `verify`, then records the scoped follow-up results.
The canonical fixtures are generated, not committed; their fixed hashes below
make the measurement input reproducible.

## Approved CPU budget

After the baseline, the orchestrator approved a median CPU budget of **500 ms
per measured command at 10k records**, with **200 ms** as a stretch goal. These
are acceptance criteria for this follow-up, not budgets inferred by this report.
Wall time and peak RSS remain diagnostic only.

## Fixture contract

`scripts/dev/generate-scale-fixtures.py` uses only the Python standard library.
It creates valid `bl_` IDs with the released framed SHA-256 scheme and fixed
RFC3339 timestamps: events start at `2026-01-15T00:00:00.000Z`; measurement
time is `2026-01-16T00:00:00.000Z`. `--check` regenerates bytes in memory and
rejects any difference from the files on disk.

| Physical-line category | 1k | 10k |
| --- | ---: | ---: |
| Unrelated open cuts (includes add/resolve sentinels) | 630 | 6,300 |
| Exact repeated-title open cuts (20/200 groups of four) | 80 | 800 |
| Tagged near-duplicate open cuts (20/200 groups of four) | 80 | 800 |
| Resolved-anchor cut events | 60 | 600 |
| Base resolve events | 60 | 600 |
| Post-resolution recurrence open cuts | 40 | 400 |
| Dogear events | 30 | 300 |
| Byte-identical duplicate cut events | 10 | 100 |
| Byte-identical duplicate resolve events | 5 | 50 |
| Malformed physical lines | 5 | 50 |
| **Total physical lines** | **1,000** | **10,000** |

Folded results are 830/8,300 open cuts, 60/600 resolved cuts, and 30/300
dogears. `triage` finds 40/400 four-member clusters; `verify` finds 40/400
recurrences. `doctor` sees malformed and duplicate-cut findings; duplicate
resolve replays are intentionally a fold warning, not a doctor finding.

| Fixture | Bytes | SHA-256 |
| --- | ---: | --- |
| 1k | 181,587 | `c0073a4af45d67757bee339c2a79bf9016c97ddb6abb636cc568d2b0c91ab963` |
| 10k | 1,815,735 | `fe9db229c3c438a43033c8be065597c301a178ba4067a62b30025fa7a6425a42` |

## Method

```sh
scripts/dev/generate-scale-fixtures.py --output-dir target/scale-fixtures
cargo build --release
scripts/dev/bench-baseline.sh \
  --fixtures-dir target/scale-fixtures --runs 3 --inner 3 \
  --output target/scale-baseline-2026-08-18.tsv
```

`bench-baseline.sh` never invokes Cargo and refuses a missing release binary,
so build time is excluded. It validates fixture bytes before and after timing.
It performs one untimed warm-up per command and fixture, then takes three
batches of three sequential invocations. Wall and CPU values below are
per-invocation batch averages; peak RSS is the undivided peak of a sequential
batch. On macOS the script normalizes `/usr/bin/time -l` resident bytes to KiB.
Stdout is redirected to `/dev/null` after normal serialization. `triage`,
`verify`, and `doctor` are expected to exit 1 because the fixtures deliberately
produce clusters, recurrences, and findings. `add` uses the fixed duplicate
sentinel and stays unchanged; `resolve` appends only to an untimed scratch copy.

Runner: `blotter 0.14.0`, `rustc 1.97.1 (8bab26f4f 2026-07-14)`,
`aarch64-apple-darwin`; release profile is `opt-level=z`, LTO, one codegen
unit, `panic=abort`, and stripped. Hardware: Apple M3 Max, 36 GiB RAM, macOS
26.5.2 (25F84), arm64. Base commit: `78adb5694ebdfc1ce75c23e54ca446cd79d91747`.

## Baseline measurements (before implementation)

Values are `min / median` over three batches. CPU is the primary scaling
signal; use wall time as end-to-end latency evidence, not as proof of CPU work.

| Fixture | Command | Wall ms min / median | CPU ms min / median | Peak RSS KiB min / median |
| --- | --- | ---: | ---: | ---: |
| 1k | list | 10.00 / 10.00 | 3.33 / 6.67 | 4,480 / 4,512 |
| 1k | triage | 70.00 / 70.00 | 63.33 / 66.67 | 5,200 / 5,232 |
| 1k | verify | 10.00 / 10.00 | 6.67 / 6.67 | 5,296 / 5,312 |
| 1k | digest | 70.00 / 70.00 | 66.67 / 66.67 | 5,904 / 5,920 |
| 1k | doctor | 20.00 / 20.00 | 13.33 / 13.33 | 4,432 / 4,448 |
| 1k | add (duplicate) | 6.67 / 10.00 | 3.33 / 6.67 | 4,464 / 4,480 |
| 1k | resolve | 13.33 / 13.33 | 10.00 / 10.00 | 5,776 / 5,792 |
| 10k | list | 63.33 / 66.67 | 63.33 / 63.33 | 19,840 / 19,856 |
| 10k | triage | 7,516.67 / 7,546.67 | 7,493.33 / 7,506.67 | 30,112 / 30,128 |
| 10k | verify | 100.00 / 100.00 | 96.67 / 96.67 | 30,224 / 30,288 |
| 10k | digest | 7,430.00 / 7,533.33 | 7,423.33 / 7,520.00 | 31,760 / 32,208 |
| 10k | doctor | 53.33 / 53.33 | 43.33 / 43.33 | 8,784 / 8,800 |
| 10k | add (duplicate) | 63.33 / 66.67 | 60.00 / 63.33 | 19,840 / 19,840 |
| 10k | resolve | 126.67 / 126.67 | 120.00 / 123.33 | 32,416 / 32,432 |

Median CPU scaling from 1k to 10k: list 9.5x, triage 112.6x, verify 14.5x,
digest 112.8x, doctor 3.3x, duplicate add 9.5x, and resolve 12.3x.

## TASK-29.3 checkpoint (triage and digest)

The same fixture hashes, release profile, runner, `--runs 3 --inner 3` method,
and fixed clocks were used after the change. The optimized analyzer builds
exact-normalized-title and tagged-pool indexes first, then uses an exact
bit-parallel r19 score prefilter. `linked` remains the final pair predicate;
document-frequency rarity is still calculated over all analyzed open cuts.

| Fixture | Triage CPU ms before → after min / median | Digest CPU ms before → after min / median |
| --- | ---: | ---: |
| 1k | 66.67 → 10.00 / 10.00 | 66.67 → 10.00 / 10.00 |
| 10k | 7,506.67 → 183.33 / 183.33 | 7,520.00 → 190.00 / 190.00 |

The 10k CPU medians improve by 40.9x for triage and 39.6x for digest. Their
post-change 1k→10k scaling is 18.3x and 19.0x respectively. Both meet the
500 ms budget and the 200 ms stretch goal. The cost is higher diagnostic RSS:
triage 30,128 → 61,184 KiB and digest 32,208 → 62,544 KiB at 10k median.

`verify` has no production change in TASK-29.3. Its 10k median CPU remains
within both targets (96.67 ms baseline; 93.33 ms in this checkpoint), so the
approved scope deliberately leaves its recurrence scan unchanged. Its existing
contract tests retain one-open-to-many-anchor behavior and exclude pre-resolution,
dogear, dropped, blank-title, and hidden-auto cases.

Fixture semantic checks after the release build confirmed 40/400 triage
clusters, 40/400 verify recurrences, and 40/400 digest chronic entries with
the expected command exits. The generator `--check` passed after measurement.

## TASK-29.2 results (resolve and fold sort)

TASK-29.2 is limited to two changes. `resolve` now materializes its response
records from the fold that made the append decision plus the appended resolution
data, so the post-append read and fold are gone; the exclusive lock still spans
read, decide, and append. `fold_bytes` now parses each record timestamp once
before sorting instead of inside every sort comparison.

Semantics are unchanged. Base resolves stay first-wins, a new base resolve still
activates the latest earlier orphan amend, an appended amend is by definition the
latest amend, and tear healing, rollback, and warnings are untouched. Tear
healing cannot change which records the response sees: the scanner accepts a
final newline-less line whenever its JSON carries a known kind, so the healing
`\n` never makes a previously rejected line parse.

The same fixture hashes, release profile, runner, `--runs 3 --inner 3` method,
and fixed clocks were used. Two consecutive after-runs are reported because the
first ran against a busy machine; the second is the quieter run and is the one
compared below. Peak RSS is diagnostic only.

| Fixture | Command | CPU ms before median | CPU ms after median (run 1 / run 2) | Peak RSS KiB before → after median |
| --- | --- | ---: | ---: | ---: |
| 1k | resolve | 10.00 | 10.00 / 3.33 | 5,792 → 4,496 |
| 1k | list | 6.67 | 6.67 / 3.33 | 4,512 → 4,576 |
| 10k | resolve | 123.33 | 46.67 / 40.00 | 32,432 → 20,208 |
| 10k | list | 63.33 | 50.00 / 40.00 | 19,856 → 21,328 |

At 10k, resolve CPU falls 3.1x and its peak RSS falls about 12.2 MiB, which is
the second whole-log fold no longer being resident. Resolve now costs about the
same as a duplicate `add` on the same input (40.00 ms versus 43.33 ms), where the
baseline had it at about 1.9x that cost. The one-time timestamp parse accounts
for the `list` improvement, since `list` shares `fold_bytes` and changed in no
other way.

Every measured command is inside the 500 ms budget and the 200 ms stretch goal at
10k in run 2: list 40.00, triage 170.00, verify 80.00, digest 186.67, doctor
66.67, duplicate add 43.33, and resolve 40.00 ms median CPU.

### Out of scope, and why

The owned-record/`ListItem` deduplication and the scanner rewrite remain out of
scope: the baseline did not causally isolate either as material, and the approved
budget is met without them. Acceptance criterion 3 of TASK-29.2 is conditional on
such a demonstrated gain, so it is closed as not-triggered rather than done.
`append_unique` still returns the first stored event on duplicates, unchanged.

`verify` also has no production change in TASK-29.2, matching its TASK-29.3
disposition.

## Verify recurrence scan results (2026-08-19)

This section supersedes the TASK-29.3 and TASK-29.2 disposition above: `verify`
now reuses triage's `CandidateIndex` and its bit-parallel prefilter instead of
comparing every resolved anchor against every open cut. `linked` remains the
final pair predicate. The index is built over the open cuts while the token
frequencies stay the open-plus-anchor counts `verify` already computed, which is
what keeps document-frequency rarity — and therefore every `linked` verdict —
unchanged. Because the prefilter returns positions into the `(timestamp, id)`
sorted open vector, the post-resolution cutoff is applied as a starting floor on
the bitset walk rather than as a second pass. `retrospect` inherits the change.

### Fixture and runner deltas

Two larger fixtures were needed. The committed generator emits 1k and 10k only,
so its `build_fixture` was called with larger sizes from a scratch wrapper; the
composition ratios, IDs, clocks, and byte layout are the generator's. These two
files are measurement inputs, not canonical fixtures, and `--check` still passed
for the committed pair before and after measurement.

| Fixture | Bytes | SHA-256 |
| --- | ---: | --- |
| 100k | 18,261,236 | `6f6093d1297b786be4fe4072c8a532b701b7d88a8964e39f61fd611215a882dc` |
| 300k | 55,074,854 | `c690507dc1b688656541d8a07c2179375231e8d07fd516ecd2c55580fd36f7ca` |

Folded shape scales with the ratios: 8,300 / 83,000 / 249,000 open cuts and
600 / 6,000 / 18,000 resolved anchors, giving 400 / 4,000 / 12,000 recurrences.

The release profile is no longer `opt-level = "z"`; it is `opt-level = 3` from
earlier in this batch. **Do not compare the absolute values below to the
opt-level z tables above.** The before and after columns here share one profile,
one binary layout, and one host, so only they are comparable to each other.
Runner: `blotter 0.15.0`, `rustc 1.97.1 (8bab26f4f 2026-07-14)`,
`aarch64-apple-darwin`, Apple M3 Max (Mac15,10), 36 GiB RAM, macOS 26.5.2. Base
commit: `8d0494fe`. Method is the `bench-baseline.sh` method — one untimed
warm-up, then three batches of three sequential invocations, `/usr/bin/time -l`,
per-invocation wall and CPU, undivided peak RSS per batch — run from a
verify-only harness because `bench-baseline.sh` iterates a fixed 1k/10k list.

### Output equivalence

Release-mode `verify` stdout was captured on all three fixtures before and after
the change and compared byte for byte. All three are identical; the SHA-256 of
each is the same value before and after.

| Fixture | stdout SHA-256 (before == after) |
| --- | --- |
| 10k | `838c1161ff3e40b5a826071621ead852aedea8dece7d7188a44f2bab5d4f081d` |
| 100k | `c22f15eef87c11793fa224256326145baeec993ad0e9397abc6056bbf327f8c0` |
| 300k | `a143d1663968bd17f2c51d65826424efe08821c31f3f8fa0f760e906f2f6bfb8` |

Same recurrences, same member order, same counts, same exit 1. The 252-test
suite passed five consecutive times.

### CPU and peak RSS

Values are `min / median` over three batches.

| Fixture | Verify CPU ms before → after min / median | Verify peak RSS KiB before → after min / median |
| --- | ---: | ---: |
| 10k | 43.33 / 46.67 → 43.33 / 46.67 | 34,224 / 34,336 → 38,144 / 38,784 |
| 100k | 1,153.33 / 1,193.33 → 673.33 / 686.67 | 298,768 / 299,072 → 504,176 / 505,264 |
| 300k | 24,963.33 / 25,166.67 → 3,590.00 / 3,660.00 | 889,856 / 901,296 → 2,306,800 / 2,309,648 |

Median CPU improves 1.00x at 10k, 1.74x at 100k, and 6.88x at 300k. Median peak
RSS grows 1.13x at 10k (about 4.3 MiB), 1.69x at 100k (about 201 MiB), and
2.56x at 300k (about 1.34 GiB).

`list` is the reference for whole-log fold plus output with no analyzer, measured
the same way on the same binary: 23.33 / 260.00 / 896.67 ms median CPU and
23,120 / 181,328 / 548,752 KiB median peak RSS. Subtracting it approximates the
analyzer residual — candidate construction, frequencies, index build, scan, and
materialization:

| Fixture | Residual CPU ms before → after (median) | Ratio |
| --- | ---: | ---: |
| 10k | 23.34 → 23.34 | 1.00x |
| 100k | 933.33 → 426.67 | 2.19x |
| 300k | 24,270.00 → 2,763.33 | 8.78x |

The residual's own 10k→300k growth for 30x records falls from 1,040x to 118x.
It is still super-linear because the surviving term is the bitset walk itself:
anchors x open/64 words, 70 M word operations at 300k against 7.8 M at 100k.
That is triage's own residual prefilter cost, and it is filed separately.

### The memory cost is worse than expected

The brief expected the index to roughly double peak RSS, as it did for triage at
10k. At 10k and 100k that holds — 1.13x and 1.69x. At 300k it does not: 2.56x,
and 2.20 GiB in absolute terms. That is the honest headline, and it is a real
acceptance concern for a CLI an agent runs, not a diagnostic footnote.

The cause is that every posting set in the index is a bitset sized to the open
cut count, so each entry costs `open/8` bytes — about 31 KiB per entry at 300k —
and this fixture has a wide indexed vocabulary: roughly 18,000 shared tokens and
24,000 distinct open-cut tags. Two thirds of the growth is `by_tag` and
`by_token` posting sets.

Two things keep this a defensible trade rather than a new failure class. First,
it is the cost triage already ships on the same log: single-run peak RSS for
`triage` on these fixtures is 459,072 KiB at 100k and 1,843,792 KiB at 300k, so
anyone folding a 300k log already meets a 1.8 GiB analyzer. Verify is now at
parity with it, plus the anchors and the recurrence output it also holds.
Second, the alternative at 300k was 25 s of CPU.

The cheapest available reduction was deliberately not taken. In `verify` the
representative is always an anchor, so any indexed token or tag that no anchor
carries is built and never queried — roughly 42% of the index on this fixture.
Filtering the index to the representative vocabulary is provably output-neutral,
because an entry that is never looked up cannot change a prefilter result, but it
changes a shared structure that a separate triage task is already queued to
touch. It is recorded here as the next lever, not applied.

## What the measurements support

- **Triage candidate scan: confirmed material, now remediated.** The baseline
  10x fixture increase produced 112.6x CPU growth (66.67 ms to 7.51 s); digest
  tracked it at 112.8x (66.67 ms to 7.52 s). The indexed r19 prefilter reduces
  both 10k medians below the stretch target without changing pair semantics.
- **Verify candidate scan: immaterial for the approved 10k budget.** Its 600
  anchors compare against 8,300 open cuts (about 4.98 million possible checks),
  yet the baseline was 96.67 ms CPU and the unchanged checkpoint was 93.33 ms.
  The algorithmic risk remains, but this fixture mix does not justify a change.
  **Superseded 2026-08-19:** the risk was real above the 10k budget. At 300k the
  same scan reached 25.17 s CPU, and 10k stayed the only size where it looked
  free. It is now indexed; see the verify recurrence-scan section.
- **Fold double-parse / owned-state amplification: not isolated.** List and
  duplicate add scale to 63.33 ms CPU and about 19.8 MiB RSS at 10k. That shows
  whole-log fold cost but does not assign it to JSON handling, cloning, sorting,
  or output construction; owned-record/`ListItem` dedupe stays out of scope. The
  one sort cost that was source-visible, reparsing each timestamp inside every
  comparison, is now a single parse per record, and 10k list CPU falls to
  40.00 ms.
- **Resolve double-fold: confirmed material, now remediated.** At 10k the
  baseline had resolve at 123.33 ms CPU versus 63.33 ms for duplicate add on the
  same input (about 1.9x), consistent with its source-visible second read/fold
  after append. Removing that second fold brings resolve to 40.00 ms, level with
  duplicate add, and drops its peak RSS by about 12.2 MiB.

## Contention and reproducibility caveats

The run was not CPU-pinned and uses a shared interactive Mac. Wall time includes
process startup, scheduling delay, page-cache state, `/usr/bin/time`, and the
normal doctor git-ignore check (fixtures live under ignored `target/`). Earlier
discarded trial runs had large wall/CPU gaps, so CPU is more reliable for the
scaling conclusion. Re-run from an idle machine and retain the raw TSV before
comparing absolute wall values across hosts; do not compare these values to a
different Rust toolchain, release profile, or fixture hash.

<!-- BEGIN TASK-46 (appended 2026-08-19; independent of the sections above) -->

# TASK-46: archive and doctor --fix single-parse results

Date: 2026-08-19. Status: before/after results for the TASK-46 double-parse
removal. This section was appended whole and reads independently of everything
above it; it shares the fixture generator but not the fixture set or the runner
script.

## What was measured

`plan_archive` folded the log and then ran `store::scan` over the same bytes, so
every physical line was `serde_json`-decoded twice; the second pass existed only
to recover line numbers and per-ID line groupings. `doctor --fix` re-inspected
the whole repaired log to report post-fix findings. Both are now single-pass:
the fold optionally carries a `(line, id, ts)` tuple per record-carrying line
(`store::fold_bytes_with_lines`), and doctor derives the post-fix report from
the pre-fix findings.

## Fixtures

`scripts/dev/generate-scale-fixtures.py` emits only 1k and 10k, so the 100k
fixture was generated by importing that module and calling
`build_fixture(100_000)` — the same generator, the same composition per 1k, the
same fixed clock. Nothing in the script changed.

| Fixture | Physical lines | Bytes | SHA-256 |
| --- | ---: | ---: | --- |
| 10k | 10,000 | 1,815,735 | `fe9db229c3c438a43033c8be065597c301a178ba4067a62b30025fa7a6425a42` |
| 100k | 100,000 | 18,261,236 | `6f6093d1297b786be4fe4072c8a532b701b7d88a8964e39f61fd611215a882dc` |

At 100k with `--before 2026-03-01T00:00:00Z` and `BLOTTER_NOW=2026-06-01T00:00:00.000Z`,
`archive` removes 12,500 lines and keeps 87,500, so the apply path exercises the
full plan, backup, sidecar, and swap. `doctor --fix` quarantines the 500
malformed lines.

## Method

Release binaries built from the same tree, before and after the change. Five
sequential invocations per case, each on a fresh untimed copy of the fixture
because both commands mutate the log; CPU is `user + sys` from `/usr/bin/time -l`
and peak RSS is its `maximum resident set size` normalized to KiB; the median of
the five is reported. Not CPU-pinned, on a shared interactive Mac, so treat CPU
as the comparable figure and wall time as diagnostic, exactly as the sections
above do.

Note the resolution floor: `/usr/bin/time` reports CPU in 10 ms ticks, and a 10k
run is two or three ticks. The 10k row below is therefore a batch measurement —
20 sequential dry-run invocations under one timer, divided by 20, median of
three batches — and it includes process startup. The 100k rows carry the
conclusion.

## Results

Median CPU seconds and median peak RSS in KiB, 100k fixture:

| Case | CPU before | CPU after | Change | RSS before | RSS after | Change |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `archive --dry-run` | 0.41 | 0.27 | 1.52x faster | 195,776 | 201,776 | +3.1% |
| `archive` (apply) | 0.40 | 0.28 | 1.43x faster | 195,792 | 202,832 | +3.6% |
| `doctor --fix` | 0.38 | 0.19 | 2.00x faster | 80,064 | 64,080 | -20.0% |

10k fixture, `archive --dry-run`, batch method: 47.50 ms/invocation before,
37.00 ms/invocation after (medians of three batches; the spread across batches
was 44.00–49.50 before and 34.00–46.00 after, so the 10k figure is directionally
consistent but too noisy to quote as a ratio).

## Reading the memory numbers

`doctor --fix` drops peak RSS because the second `inspect` no longer builds a
second findings vector and a second `records` map of owned raw lines while the
original, repaired, and quarantined byte buffers are all still resident.

`archive` rises slightly, and that is the honest cost of the change: the fold now
retains one `(usize, String, Timestamp)` per record-carrying line — about 6 MiB
at 100k — in exchange for not decoding the log a second time. The peak is
unchanged in shape, since the original, `kept_bytes`, and `archived_bytes` were
already all resident together inside the exclusive lock. The tuples are opt-in
through `fold_bytes_with_lines`, so `list`, `triage`, `verify`, `digest`,
`retrospect`, `sweep`, and `export` allocate nothing for them.

## Output-identity verification

Byte-identity was verified directly, not inferred from a green suite. The
pre-change and post-change release binaries were run over 26 log shapes —
empty, `"\n"`, leading empty segment, resolved and unresolved, mixed-age groups,
non-`bl_` namespace, duplicate cuts and resolves, orphan resolve, amend and
orphan amend, dogears, malformed first/middle, torn tails valid and invalid,
conflict markers, unknown kinds, an all-malformed log, and the 10k and 100k
fixtures — against nine invocations each (`archive` dry-run, apply, apply
archiving nothing, apply archiving everything, `--pretty`; `doctor`, `doctor
--fix --dry-run`, `doctor --fix`, and `doctor --fix` twice in a row). All 234
pairs matched on exit code, stdout bytes, stderr bytes, and the SHA-256 of every
file left behind, including backups and the archive and quarantine sidecars.

A unit test in `src/commands/doctor.rs` locks the doctor half in permanently:
`derived_post_fix_report_matches_a_full_reinspection` asserts the derived
post-fix report equals a full `inspect` of the repaired bytes across the shapes
where line numbers shift, a leading empty segment survives, or the repair empties
the log.

<!-- END TASK-46 -->
<!-- BEGIN TASK-45 (prefilter budget above 10k). Appended as a self-contained
     section; nothing above this line is edited. -->

## Triage and retrospect prefilter budget above 10k (TASK-45, 2026-08-19)

The 10k budget at the top of this document is met and is not the question here.
This section answers a different one: what does the residual prefilter cost
above 10k, and what bounds it. It supersedes the closing note of the verify
section — "that is triage's own residual prefilter cost, and it is filed
separately" — which is this task.

**Do not compare these numbers to the audit that opened the task** (triage
54.85 s / 1,440 MiB at 300k). That audit ran on `opt-level = "z"` and before
`verify` was indexed. Everything below is re-measured on the current profile and
the current batch branch, before and after, on one host and one binary layout.

### Fixture and method

The committed generator emits 1k and 10k only, so its `build_fixture` was called
with larger sizes from a scratch wrapper — composition ratios, IDs, clocks, and
byte layout are the generator's, unchanged:

```python
spec = importlib.util.spec_from_file_location(
    "gen", root / "scripts/dev/generate-scale-fixtures.py")
gen = importlib.util.module_from_spec(spec); spec.loader.exec_module(gen)
for label, size in (("10k", 10_000), ("30k", 30_000),
                    ("100k", 100_000), ("300k", 300_000)):
    (out / f"scale-{label}.jsonl").write_bytes(gen.build_fixture(size)["bytes"])
```

| Fixture | Bytes | SHA-256 |
| --- | ---: | --- |
| 10k | 1,815,735 | `fe9db229c3c438a43033c8be065597c301a178ba4067a62b30025fa7a6425a42` |
| 30k | 5,462,996 | `b1f87249c65f3c03110d814ac85b5b1f04cc6344c76ffc60514243c6baac7975` |
| 100k | 18,261,236 | `6f6093d1297b786be4fe4072c8a532b701b7d88a8964e39f61fd611215a882dc` |
| 300k | 55,074,854 | `c690507dc1b688656541d8a07c2179375231e8d07fd516ecd2c55580fd36f7ca` |

The 10k, 100k, and 300k hashes reproduce the values recorded in the verify
section above, so the two measurement sets share an input. Folded shape scales
with the ratios: 8,300 / 24,900 / 83,000 / 249,000 open cuts.

Method is this document's method — one untimed warm-up per command and fixture,
then three timed invocations, `/usr/bin/time -l`, per-invocation wall and CPU,
peak RSS as reported — run from a triage/retrospect/list harness because
`bench-baseline.sh` iterates a fixed 1k/10k list. `list` is measured alongside as
a **drift control**: it shares the fold and is untouched by this change, so a
move in its numbers is a move in the machine. Runner: `blotter 0.15.0`,
`rustc 1.97.1 (8bab26f4f 2026-07-14)`, `aarch64-apple-darwin`, release profile
`opt-level = 3` + LTO + one codegen unit + `panic=abort` + stripped. Hardware:
Apple M3 Max (Mac15,10), 36 GiB RAM, macOS 26.5.2. Base commit: `e0931cd`.

### What the residual actually was

Per representative, the prefilter did a fixed number of passes over the whole
candidate bitset — clear, tag pool, one bit-sliced add per indexed token, one
`add_at_least` per distinct candidate-token-count bucket, the rare-token pass,
and the tag mask — so the scan cost `Theta(n^2/64)` word operations regardless
of whether any candidate could match. On this fixture 76% of open cuts are
"unrelated" records whose only repeated words are `unrelated` and `fixture`;
each of them paid roughly 29 full-width passes to produce an empty match set.

### The three bounds now applied

All three are bounds on work that was provably not needed, not changes to the
relation. `linked` remains the final pair predicate and is unmoved.

1. **Reachability bound.** Only a token with a posting set can raise a
   candidate's shared-token count, so the count of the representative's indexed
   tokens is an upper bound on every counter the scan can produce. A threshold
   above that bound is unreachable, and the bucket — or the whole scoring path —
   is skipped rather than counted and discarded. The rare path gets the same
   bound over its rare tokens alone. When neither path can run, the tag pool that
   would mask the result is never built either.
2. **Representative floor.** Both callers already discard a prefix of the result
   — triage every candidate at or before its representative, verify everything up
   to the resolution timestamp — so the bit-parallel count now starts at the
   floor's word instead of computing a prefix that is thrown away. This is the
   only bound that scales with the representative's own position, and it is what
   turns the average scan into half the candidate space.
3. **Singleton-tag pools dropped.** A tag counted once across the analyzed
   population is carried by exactly one record: either no representative carries
   it and its pool is never queried, or the sole carrier is the representative,
   whose own bit every caller discards. Such pools are no longer built. This is
   the memory bound, and it is the same rule `by_token` already applied to
   unshared tokens. It requires the tag counts to include representatives that
   are not candidates, which is why `TokenFrequencies` became `CorpusFrequencies`
   and now counts tags as well — a tag shared by one `verify` anchor and one open
   cut must still count as shared.

### Output equivalence

Release-mode stdout was captured for `triage`, `retrospect`, `verify`, and
`digest` on all four fixtures, before and after, and compared byte for byte. All
sixteen pairs are identical — same clusters, candidates, recurrences, member
order, counts, and exit codes.

| Fixture | triage | retrospect | verify | digest |
| --- | --- | --- | --- | --- |
| 10k | `6c0688f8…` | `08051106…` | `1d83f967…` | `2acff472…` |
| 30k | `db8815bb…` | `8a61bee5…` | `b37430f3…` | `027952c6…` |
| 100k | `671c1685…` | `1819e882…` | `5c6731e0…` | `c1941931…` |
| 300k | `ee385c90…` | `a06ae739…` | `d6425439…` | `ee72308a…` |

The full 266-test suite passed five consecutive times (`scripts/dev/gate-5x.sh`).
Two unit tests check the prefilter against the public pair rule directly — one
fixed mixed-pool corpus and 400 random corpora — and a third now does it with
representatives that are not candidates, the shape `verify` has and the one where
a candidate-only tag count would silently drop a real pool.

### CPU and peak RSS

Values are `min / median` over three invocations.

| Fixture | Command | CPU ms before → after | Peak RSS KiB before → after |
| --- | --- | ---: | ---: |
| 10k | list (control) | 20 / 20 → 20 / 20 | 21,936 / 23,696 → 21,936 / 23,264 |
| 10k | triage | 70 / 70 → 40 / 50 | 39,072 / 39,376 → 37,872 / 38,016 |
| 10k | retrospect | 80 / 80 → 80 / 80 | 41,616 / 41,872 → 41,520 / 42,352 |
| 30k | list (control) | 60 / 70 → 70 / 80 | 61,360 / 63,056 → 61,280 / 61,344 |
| 30k | triage | 340 / 350 → 170 / 170 | 114,432 / 116,032 → 108,016 / 108,048 |
| 30k | retrospect | 430 / 440 → 240 / 240 | 120,352 / 121,920 → 110,880 / 111,456 |
| 100k | list (control) | 260 / 260 → 260 / 270 | 181,232 / 181,248 → 181,216 / 181,232 |
| 100k | triage | 2,390 / 2,450 → 660 / 670 | 451,104 / 451,440 → 404,256 / 404,352 |
| 100k | retrospect | 2,890 / 2,890 → 960 / 970 | 520,160 / 521,920 → 488,272 / 521,984 |
| 300k | list (control) | 810 / 830 → 910 / 970 | 546,960 / 547,024 → 547,008 / 547,104 |
| 300k | triage | 18,290 / 18,330 → 2,430 / 2,450 | 1,899,376 / 1,899,440 → 1,518,816 / 1,519,536 |
| 300k | retrospect | 21,050 / 21,220 → 3,340 / 3,390 | 2,325,552 / 2,332,032 → 2,318,880 / 2,330,432 |

Median CPU improves 1.40x / 2.06x / 3.66x / 7.48x for triage and
1.00x / 1.83x / 2.98x / 6.26x for retrospect, at 10k / 30k / 100k / 300k. The
control moved the wrong way at 300k (830 → 970 ms on untouched code), so the
machine was busier during the after-run and these ratios are, if anything,
understated.

Subtracting the control approximates the analyzer residual: triage 17,500 →
1,480 ms at 300k, an 11.8x reduction. For retrospect the subtraction is loose
because it folds the log twice, but 20,390 → 2,420 ms is the same order.

The growth shape is what matters more than any single number. Across the 30x
step from 10k to 300k, triage median CPU grew 262x before and 49x after — an
effective exponent falling from about 1.64 to about 1.14. Peak RSS grows 40x
over the same step, an exponent of about 1.08, so memory is now the more
linear of the two.

Peak RSS falls 1.04x / 1.07x / 1.12x / 1.25x for triage, the gain rising with
scale because singleton tags are a rising share of the tag vocabulary: at 300k
12,000 of the 24,000 distinct open-cut tags name exactly one record, and each
had been costing a full 249,000-bit row. That is 347 MiB returned at 300k.
Retrospect's peak is unchanged, because it is set by holding two folds of the
log at once, not by the index.

### The budget

These are regression ceilings for this fixture, host, and profile — a tripwire
for the quadratic shape coming back, not a promise about anyone's hardware. CPU
carries roughly 2x headroom over the measured median because CPU on a shared
interactive machine is noisy; RSS carries roughly 1.3x because it is not.

| Records | triage CPU | triage peak RSS | retrospect CPU | retrospect peak RSS |
| ---: | ---: | ---: | ---: | ---: |
| 10k | 100 ms | 50 MiB | 160 ms | 55 MiB |
| 30k | 400 ms | 150 MiB | 500 ms | 150 MiB |
| 100k | 1.5 s | 550 MiB | 2.0 s | 700 MiB |
| 300k | 5.0 s | 2.0 GiB | 7.0 s | 3.0 GiB |

The pre-existing 10k budget — 500 ms per command, 200 ms stretch — still holds
and is met with a wide margin, so the row above tightens it rather than
replacing it.

**300k is where the analyzers stop being cheap, and memory is the reason.**
`retrospect` needs about 2.3 GiB resident on a 300k log and this change does not
move that; it is two folds of the log plus both analyzers' working sets, and no
prefilter bound touches it. A log that large should be trimmed with `archive`.
Publishing the ceiling is the honest form of that statement — the numbers say a
300k log works and costs a few seconds and a couple of gigabytes, not that it is
free.

### Levers identified and not taken

- **Sparse posting lists for low-frequency tokens.** Every posting set is a
  bitset sized to the candidate count, about 31 KiB per entry at 300k, so a token
  in two records costs the same as one in every record. A sorted `Vec<u32>` below
  a crossover of roughly one candidate in 64 would cut the remaining index
  memory substantially. It was not taken because it splits the posting
  representation in two and every consumer — the bit-sliced counter above all —
  would need both paths. That is a real redesign of the structure's core for a
  memory win on logs that `archive` already answers.
- **Filtering the index to the representative vocabulary in `verify`** remains
  recorded in the verify section above as its next lever, and remains untaken.
  The singleton-tag rule shipped here is adjacent but not the same thing: it
  removes pools that cannot affect a result for any caller, whereas that lever
  removes pools that this particular caller happens never to query.

<!-- END TASK-45 -->
