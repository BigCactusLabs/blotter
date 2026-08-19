---
id: TASK-37
title: CI gate for the public repo
status: In Progress
assignee: []
created_date: '2026-08-18 22:32'
updated_date: '2026-08-19 03:43'
labels: []
dependencies: []
ordinal: 45000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add .github/workflows running the four pre-commit gates (build, test --all-features, clippy -D warnings, fmt --check) plus blotter doctor --leaks on every push and PR, failing on any finding. Motivation: the repo went public 2026-08-18 with no CI at all, and .blotter.jsonl is a permanent public capture channel — CI is the only always-on guard against a leak or red gate landing on main (publish-review finding). Keep it minimal: one workflow, stable toolchain, no caching cleverness until it earns its place. Consider a second job pinning the MSRV toolchain (1.89) per AGENTS.md.
<!-- SECTION:DESCRIPTION:END -->
