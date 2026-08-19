---
id: TASK-5
title: Mine parked prior-art shortlist (private notes)
status: Done
assignee: []
created_date: '2026-08-05 21:29'
updated_date: '2026-08-19 01:53'
labels: []
dependencies: []
ordinal: 5000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Research notes on a comparable prior-art tool live in private notes (blotter-private-notes/, outside this repo). The 'Shortlist: pull / adapt' section has 14 items roughly ordered by leverage-to-effort — top candidates: arbitrary-JSON context field on records, structured deferred[] in success envelopes, machine-readable CTAs in envelopes, list --since <git-ref>, authoring-time dedup with --force. Triage the shortlist into concrete tasks; the 'Considered and rejected' section documents why we keep append-only/local-first and should not be revisited.
<!-- SECTION:DESCRIPTION:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Triaged all 14 shortlist items. Adopted as TASK-6..12 (context field; envelope contract-4 CTAs+partial outcomes; list --since git-ref; dedup warn-with-pointer; triage occurrence counting; docs quick wins; test discipline+init hygiene). Demoted to dogears: token pagination (item 8), stdin shape (item 6), verify-diff boundary (item 14). Items 13 and most of 10: already practiced, no work. Research inputs: repo mapping (contract-cited, per-item landing sites + size), dual-track web research — key findings: no ecosystem deferred[] standard exists; incur meta.cta.commands[] + arcjet remediation/confirmCommand are the emerging CTA pattern (structured argv over shell strings); token pagination is incur-only experimental; Microsoft experiment: JSON output valuable, JSON input harmful (keep clap flags).
<!-- SECTION:NOTES:END -->
