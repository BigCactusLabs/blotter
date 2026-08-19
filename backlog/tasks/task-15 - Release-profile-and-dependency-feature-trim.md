---
id: TASK-15
title: Release profile and dependency feature trim
status: Done
assignee: []
created_date: '2026-08-06 12:23'
updated_date: '2026-08-07 01:51'
labels:
  - chore
dependencies: []
ordinal: 15000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Cargo.toml has no [profile.release] section at all; the release binary is 2.1 MB. Add lto, codegen-units=1, panic=abort, strip and measure opt-level 2/3/s/z rather than assuming size modes win (Cargo docs explicitly warn s/z are not necessarily smaller). Separately, cli.rs:13 already sets color = ColorChoice::Never, which makes clap's default anstream/anstyle/anstyle-parse/colorchoice chain dead weight -- disable clap default features and enable only derive, std, help, usage, error-context, suggestions. Same argument for jiff with default-features=false: this CLI formats and parses timestamps and never touches the bundled tzdb. Finally, add rust-version = 1.89, the release that stabilized the std File::try_lock APIs store.rs depends on. Binary-size deltas are unmeasured -- measure before and after rather than assuming.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 [profile.release] added; opt-level variants benchmarked and the chosen value justified in the commit message
- [x] #2 clap default features disabled, enabling exactly derive, std, help, usage, error-context, suggestions; jiff set to default-features = false WITH features = ["std"] re-enabled (bare default-features = false drops std and does not compile — Timestamp::now needs it; keep perf-inline if it measures well); cargo tree confirms anstream, anstyle-parse, colorchoice, and jiff-tzdb are gone; anstyle itself may remain because clap_builder 4.6 requires it under std
- [x] #3 rust-version = 1.89 present in Cargo.toml
- [x] #4 Before/after binary sizes recorded; all four gate commands still pass
- [x] #5 No CLI behavior change: help text, error rendering, and timestamp formatting are byte-identical
<!-- AC:END -->

## Comments

<!-- COMMENTS:BEGIN -->
created: 2026-08-06 13:33
---
Review 2026-08-06: verified no tz/tzdb/Zoned surface anywhere in src (only Timestamp, SignedDuration, Unit), store.rs:204-206 uses the std try_lock APIs stabilized in Rust 1.89.0, and the proposed clap set is exactly clap 4 defaults minus color plus derive. Only fix: jiff needs std re-enabled.
---

created: 2026-08-07 01:50
---
Decision 2026-08-06: AC #2 revised by user. anstyle may remain because clap_builder 4.6 requires it under the mandated std feature; anstream, anstyle-parse, colorchoice, and jiff-tzdb must be absent. A Clap downgrade or fork is not authorized.
---
<!-- COMMENTS:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Implemented release size trim: z won the 2/3/s/z benchmark at 854448 bytes from a 2178176-byte baseline; perf-inline tied at the winning size and was retained. Byte-compatible help, parse-error, and fixed-clock timestamp fixtures passed. Revised dependency tree criterion passed: anstream, anstyle-parse, colorchoice, and jiff-tzdb are absent; only clap_builder-required anstyle remains.
<!-- SECTION:FINAL_SUMMARY:END -->
