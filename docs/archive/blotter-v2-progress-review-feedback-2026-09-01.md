# Blotter v2 Progress Review — Feedback Checkpoint

**Date:** 2026-09-01  
**Project:** Blotter  
**Scope:** Review of current v2 progress after the admission-floor policy, r48 contract work, and TASK-72/73 completion.  
**Status:** Review feedback, incorporated into the v2 plan. Archived 2026-09-02 — kept for provenance.  

## Executive Summary

Blotter is on the right track.

The project has moved from a promising architectural direction into a coherent release design. The strongest improvement is that v2 is not turning into a large rewrite. The current sequencing removes historical complexity first, introduces the record-model break second, adds promotion only after the new foundation exists, and updates the learning loop last.

The design is becoming **simpler despite gaining one new first-class primitive**.

The current direction can be summarized as:

```text
capture selected experience
        ↓
detect patterns
        ↓
record learning
        ↓
test whether the learning held
```

Cuts, dogears, and promotions fit this model naturally. Auto-capture, generic resolution semantics, legacy identity support, and solution-specific retrospect categories do not.

The primary recommendation going into TASK-74–77 is:

> **Keep deleting. Do not let the richness of the v2 contract make the implementation richer than the product's mental model.**

## 1. Current State

At this checkpoint:

- The higher admission-floor policy has landed.
- The normative v2 contract, r48, has landed.
- TASK-72 and TASK-73 are complete.
- TASK-74 is the active implementation phase.
- The major behavioral record-model changes are still ahead.
- `main` and the `v2` branch are currently aligned at the same commit.

The semantic work is therefore ahead of the implementation work, which is the right order for this kind of break.

## 2. Admission-Floor Work

The new capture policy is substantially stronger than the previous model.

The key framing:

> **Blotter is a selective ledger, not a transcript.**

is the correct product center.

The five admission paths are also clear and practical:

- transferable;
- consequential;
- recurring;
- misleading;
- systemic.

The explicit skip list is equally important because it removes ambiguity around common execution exhaust:

- typos;
- shell mistakes;
- bad first guesses;
- stale patch context;
- expected compiler/linter catches;
- malformed fixtures;
- one-off oversized queries;
- transient tactical mistakes.

The most useful line in the revised policy is effectively:

> Friction that does not clear the admission floor is not a minor cut. It is nothing.

That successfully separates **admission** from **impact**. This distinction should remain one of the central invariants of v2.

## 3. Policy Placement Is Correct

The admission policy is being pushed into the surfaces agents actually use rather than remaining buried in the design documentation.

This includes:

- `AGENTS.md`;
- README guidance;
- `blotter add --help`;
- schema-related documentation.

That is important because capture quality will be determined by runtime agent behavior, not by the quality of the architecture document.

The PR review process already caught a real weakness here: the first `add --help` version did not contain enough of the admission policy to actually change filing behavior. Fixing that before merge was the correct move.

## 4. Implementation Sequencing

The current phase sequence is strong:

```text
TASK-74
delete dead architecture

        ↓

TASK-75
break the record model cleanly

        ↓

TASK-76
introduce promotion

        ↓

TASK-77
update verify / retrospect / digest semantics

        ↓

TASK-78
release 1.0
```

This is much better than attempting a monolithic v2 rewrite. Each phase now has a clear architectural purpose.

## 5. TASK-74 — Delete the Auto Lane

Removing the auto lane first is the right decision.

The current product still carries architectural scars from an already-retired experiment:

- `is_auto_capture`;
- auto partitioning;
- `--include-auto`;
- hidden-record warnings;
- hook receiver code;
- hook command types;
- retrospect's special auto behavior;
- auto-specific tests;
- documentation describing the old lane.

Deleting this before introducing new v2 concepts reduces the number of interacting rules the later phases need to support.

The hook receiver should also go. Keeping a dead fail-open receiver forever would make the new stable release carry behavior solely to preserve an experiment the project has explicitly rejected. The mandatory upgrade instruction is sufficient.

## 6. TASK-75 — Fresh Ledger and Record Model Break

The decision to use a **fresh v2 ledger with no upcaster** is the right one.

General event-store practice often favors upcasting, but Blotter is in a rare position where the old history is specifically the noisy corpus the redesign is trying to stop preserving as first-class active memory.

Keeping an upcaster would require retaining:

- the old hash;
- `severity`;
- legacy `pc_` IDs;
- the old `source` representation;
- compatibility branches;
- legacy test surface.

That would immediately weaken the simplicity benefit of v2.

The planned deletion of the v1 hash, `pc_` namespace, `IdNamespace`, source fold, and legacy compatibility tests is therefore a feature, not a migration deficiency.

## 7. In-Band Record Versioning

Adding:

```json
"v": 2
```

to every v2 record is a very good addition.

JSONL has no file header, so per-record versioning gives future breaks an explicit boundary.

The pre-fold version probe is also particularly strong:

```text
open log
   ↓
version probe
   ↓
unsupported?
   ├─ yes → refuse
   └─ no  → fold
```

This should happen before tear healing, mutation, backup creation, archive rewriting, or partial state materialization.

That level of strictness is worth preserving.

## 8. Integration Branch / Single Contract Bump

The `v2` integration branch is a good design decision.

r48 defines one complete contract even though several reviewed PRs implement it.

The important invariant is:

> No released binary should advertise contract 6 while only part of contract 6 exists.

That prevents consumers from observing an intermediate semantic state.

## 9. Promotion Is Staying Appropriately Small

Promotion was the concept most likely to cause product bloat. The current design has avoided that.

A promotion essentially means:

```text
these cuts
    ↓
became this durable artifact
```

with a small artifact vocabulary:

```text
doc
skill
guard
test
tool
process
```

This is good.

Blotter should record source experiences, resulting artifact type, artifact reference, and provenance. It should **not** generate the artifact, manage its lifecycle, own skill installation, become a docs manager, become a task system, or become a workflow engine.

The current proposal respects that boundary.

## 10. Promotion Source Pinning

Archive pinning for cuts referenced by promotions is an especially good addition.

A promotion exists to preserve the provenance relationship:

```text
experience → durable learning
```

If archive later deletes the source experiences, the promotion loses the reason it exists.

Therefore promotion source cuts should be archive-pinned.

## 11. Promotion and Resolution Should Stay Separate

The decision to keep promotion and resolution as independent events is correct.

They answer different questions.

Promotion says:

> We turned this experience into durable knowledge or an intervention.

Resolution says:

> We now consider this friction's lifecycle closed under a specific disposition.

One should not imply the other automatically.

## 12. Retrospect Got Better by Getting Smaller

The earlier solution-specific candidate types:

```text
wrapper_alias
doc_repair
skill_candidate
```

were too close to proposed remediation.

The new approach is better:

```text
recurrent_friction
failed_intervention
```

These are actual detected patterns.

Suggested remedies can remain suggestions.

The decision not to ship `repeated_recovery` and `documentation_gap` without deterministic emission rules is strong product discipline.

Do not expand the pattern vocabulary until there is actual detectable evidence for a new type.

## 13. Review Process Is Working

The PR review process is producing real value.

The r48 review caught, among other things:

1. a mismatch where a recovery hint could not actually show promotion records because of `--status` behavior;
2. missing machine-readable dependencies between implementation phases.

Both were corrected before merge.

Maintain the current level of review rigor for the persistence and identity phases.

## 14. Remaining Concern #1 — `origin` May Be Overdesigned

This is the one part of the v2 schema worth reconsidering before TASK-75 lands.

The original requirement was simple:

> Leave a structured provenance seam so externally discovered qualified experience can eventually enter Blotter.

The design now appears to be moving toward reserving explicit OpenTelemetry-style fields such as:

```text
trace_id
span_id
trace_flags
```

with width validation.

That may be more future-proofing than the product currently needs.

There is still no external trace/telemetry admission path.

A smaller v2 surface may be preferable, such as:

```json
{
  "origin": {
    "type": "agent"
  }
}
```

or:

```json
{
  "origin": {
    "type": "external",
    "provider": "otel",
    "ref": "..."
  }
}
```

Then add formal trace context when a real producer needs it.

**Recommendation:** Re-evaluate whether the full OTel field reservation is necessary for v2. Preserve the seam, but avoid implementing future telemetry semantics before a producer exists.

## 15. Remaining Concern #2 — Upgrade Experience Must Be Excellent

The full break is worth it, but the 0.15 → 1.0 path intentionally contains friction.

A user may have:

- a legacy v1 log;
- an installed old Claude Code hook;
- both.

The first run of 1.0 should produce exceptionally clear repair guidance.

The structured error should tell the operator exactly what to do: preserve/rename the current ledger, start a fresh v2 ledger, and remove any old hook entry before upgrading.

This should be contract-tested, not left as README-only guidance.

**Recommendation:** Treat the migration error message and suggested fix as part of the 1.0 product surface.

## 16. Remaining Concern #3 — Digest `accepted` Naming

The planned digest field:

```text
accepted
```

appears to count cuts whose **winning resolution** received the `accepted` disposition during the reporting period.

That is not the same as friction that occurred during the period and is accepted.

The current name may therefore be too ambiguous for machine consumers.

Consider something more explicit:

```text
accepted_resolutions
```

or:

```text
accepted_in_period
```

The exact choice is less important than avoiding semantic ambiguity.

## 17. Remaining Concern #4 — `list` and Promotions

Promotions do not have lifecycle status. Cuts and dogears do.

The current solution:

```text
--status all
```

retains promotions, while:

```text
--status open
--status resolved
```

remain lifecycle filters.

This is coherent.

However, it is the first sign that a unified `list` command could start accumulating kind-specific exceptions.

**Recommendation:** Keep the current design, but watch implementation complexity closely. If `list` begins accumulating many special cases for promotions, prefer a cleaner reading surface over increasingly clever generic selector semantics.

Do not preemptively redesign it now.

## 18. Product Simplification

The project is becoming simpler faster than expected.

The current codebase includes significant machinery whose purpose is explaining earlier machinery:

```text
auto captures
    ↓
include-auto flags
    ↓
hidden-record warnings
    ↓
special retrospect behavior
    ↓
no-op hook compatibility
    ↓
source provenance from retired hook
```

plus:

```text
pc_ compatibility
legacy hash
legacy fold rules
legacy tests
```

TASK-74 and TASK-75 delete most of this.

Promotion then adds one explicit primitive for a behavior the system already performs implicitly.

That is a good trade:

```text
many exceptions removed
        +
one meaningful primitive added
```

## 19. The Emerging Stable Product Model

The strongest mental model for Blotter now is:

```text
EXPERIENCE
    ↓
ADMISSION
    ↓
CUT
    ↓
PATTERN
    ↓
PROMOTION
    ↓
INTERVENTION
    ↓
VERIFY
```

with:

```text
DOGEAR
```

as the parallel hypothesis / idea channel.

The core semantic distinctions are:

- **Cut:** selected experience worth retaining.
- **Dogear:** an idea or hypothesis worth revisiting.
- **Pattern:** derived understanding over experience.
- **Promotion:** a durable record that experience became institutional knowledge or an intervention.
- **Resolution:** a lifecycle decision about a cut.
- **Verify:** evidence about whether the claimed intervention held.

That is a strong architecture.

## 20. Guidance for TASK-74–77

### Keep deleting

The main value of this break is removing historical special cases.

### Keep promotion passive

Promotion records provenance. It does not orchestrate remediation.

### Keep pattern vocabulary evidence-driven

No new retrospect type without a deterministic emission rule.

### Keep admission qualitative

Do not add salience scores, importance scores, LLM admission classifiers, or actionability scoring.

### Keep telemetry external

Blotter begins where raw runtime activity becomes meaningful experience.

### Keep the mental model smaller than the contract

The design document can be precise. The product itself should remain understandable in a few verbs.

## 21. Final Assessment

The work so far is strong.

The major improvements are:

- the admission floor is now explicit and operational;
- the implementation phases are well isolated;
- legacy compatibility is being deliberately removed rather than preserved reflexively;
- promotion is narrow and useful;
- retrospect is moving from solution ontology toward evidence-backed pattern detection;
- the review process is catching real contract problems;
- the project is becoming conceptually simpler.

The main pre-TASK-75 question worth revisiting is whether `origin` needs its current level of future telemetry detail.

Everything else is mostly execution discipline.

The intended 1.0 outcome should not be judged by how many new features v2 adds.

It should be judged by whether Blotter reaches a stable state where its core behavior is easy to explain:

> **Capture selected experience. Detect patterns. Record learning. Verify whether the learning worked.**

That is a much stronger product than the original complaint-log framing.
