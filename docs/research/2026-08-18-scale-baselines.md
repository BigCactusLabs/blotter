# Scale baselines: fold and analyzers

Date: 2026-08-18. Status: baseline plus TASK-29.3 and TASK-29.2 results.

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

## What the measurements support

- **Triage candidate scan: confirmed material, now remediated.** The baseline
  10x fixture increase produced 112.6x CPU growth (66.67 ms to 7.51 s); digest
  tracked it at 112.8x (66.67 ms to 7.52 s). The indexed r19 prefilter reduces
  both 10k medians below the stretch target without changing pair semantics.
- **Verify candidate scan: immaterial for the approved 10k budget.** Its 600
  anchors compare against 8,300 open cuts (about 4.98 million possible checks),
  yet the baseline was 96.67 ms CPU and the unchanged checkpoint was 93.33 ms.
  The algorithmic risk remains, but this fixture mix does not justify a change.
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
