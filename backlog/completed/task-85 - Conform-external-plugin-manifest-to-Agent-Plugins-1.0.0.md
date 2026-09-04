---
id: TASK-85
title: Conform external plugin manifest to Agent Plugins 1.0.0
status: Done
assignee: []
created_date: '2026-09-04 20:49'
updated_date: '2026-09-04 20:52'
labels: []
dependencies: []
references:
  - 'https://github.com/github/awesome-copilot/issues/2944'
type: bug
ordinal: 81000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Awesome Copilot issue #2944 passed blocking checks but reported a legacy manifest location, missing schema, and unsupported top-level fields.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Root plugin.json passes the Agent Plugins 1.0.0 schema and skill lint.
- [x] #2 Copilot installs the corrected plugin and retains the canonical blotter skill.
- [x] #3 Required repository build, test, lint, and format gates pass.
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Move the manifest to the plugin root; add the schema; remove legacy fields; bump plugin metadata to 1.0.1. Validate schema, skill discovery and isolated installation, then publish and update issue #2944.
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Moved the manifest to root plugin.json, added the Agent Plugins 1.0.0 schema, removed unsupported fields, and bumped plugin metadata to 1.0.1. The unchanged skills/blotter directory is discovered by convention. Official JSON schema and Vally 0.15.0 skill lint passed. An isolated Copilot CLI 1.0.83 marketplace installation found one skill and enabled version 1.0.1. Release build, full tests (374 passed), Clippy and format checks passed; the initial sandboxed device-path test returned permission denied, and the full suite passed outside the sandbox.
<!-- SECTION:FINAL_SUMMARY:END -->
