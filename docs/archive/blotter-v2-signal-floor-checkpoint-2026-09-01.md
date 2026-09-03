# Blotter Checkpoint — Higher Signal Floor + v2 Learning Architecture

**Date:** 2026-09-01  
**Status:** Design checkpoint. Archived 2026-09-02 — superseded by docs/plans/2026-09-01-blotter-v2-plan.md and the design doc's amendments from r48 on; kept for provenance.  
**Project:** Blotter  
**Theme:** Raise the cut floor, simplify the product, and clarify Blotter as a selective experiential learning system.

---

## 1. Why this checkpoint exists

Blotter is generally working well, but the current capture policy has drifted too far toward recall at the expense of signal.

The practical symptom is that the ledger contains useful friction mixed with operational exhaust:

- one-off shell mistakes;
- overly broad source reads;
- linter/compiler catches on newly written code;
- stale patch context;
- malformed fixtures;
- first-attempt command/API mistakes;
- transient tactical errors specific to one run.

These are real execution events, but they are not always useful product knowledge.

The goal is **not** to make Blotter conservative or sterile. The goal is to raise the floor slightly so a surfaced cut actually matters.

A small recurring footgun should still be retained. A random mistake should not.

---

## 2. Core diagnosis

The current model collapses two different questions:

1. **Should this experience be retained at all?**
2. **How much impact did it have?**

Today, the default `minor` severity often acts as permission to retain nearly anything mildly annoying.

That is the conceptual leak.

The revised model separates:

> **Admission = information value.**  
> **Impact = consequence.**

Something can be low impact yet highly worth retaining.

Something can also be annoying but not worth retaining at all.

---

## 3. Research validation

A prior-art/frontier research pass compared Blotter against:

- incident management and observability systems;
- developer-friction / papercut programs;
- agent observability;
- agent memory and experiential learning systems.

The resulting conclusion is strongly validating.

### 3.1 Mature systems separate telemetry from issues

Strong observability systems do not treat every raw event as a human-facing incident.

They tend to follow a layered model:

```text
raw event / trace
        ↓
qualification / grouping
        ↓
meaningful issue
        ↓
recurrence / prioritization
        ↓
intervention
        ↓
verification
```

This supports Blotter remaining a semantic layer rather than becoming a telemetry store.

### 3.2 Current agent systems converge on the same loop

Frontier agent-observability systems increasingly follow:

```text
trace
  ↓
recurring failure
  ↓
issue
  ↓
diagnosis / pattern
  ↓
fix / evaluator
  ↓
reopen if recurrence returns
```

Blotter already maps closely:

```text
cut
 ↓
triage
 ↓
retrospect
 ↓
resolve
 ↓
verify
```

That structure is fundamentally sound.

### 3.3 Agent-learning research validates the cut / insight separation

Systems such as Reflexion and ExpeL do not preserve every raw failure as reusable knowledge.

They distinguish:

- trajectory / experience;
- reflection / insight;
- reusable learned guidance.

That supports Blotter's existing separation between individual cuts and retrospective promotion.

### 3.4 Papercut prior art also had a higher floor

The original "papercut" model did not mean "log every annoyance."

A papercut still had to be:

- a real usability issue;
- plausibly encountered by others;
- meaningfully degrading the experience.

The key lesson:

> **Small impact is fine. Insignificant evidence is not.**

---

## 4. Revised product positioning

Blotter should no longer be thought of primarily as an "agent complaint log."

A stronger definition is:

> **Blotter is a selective experiential learning ledger for software systems.**

It records the subset of agent/user experience with enough information value to potentially change the environment.

The system then:

1. retains meaningful experience;
2. discovers recurrence;
3. identifies systemic patterns;
4. promotes those patterns into durable improvements;
5. verifies whether the improvement worked.

A compact product sentence:

> **Blotter records meaningful friction and ideas, discovers recurring patterns, and tracks when those patterns become durable improvements.**

---

## 5. New admission rule

A cut should be retained when there is a strong reason to believe the observation has future information value.

A cut normally qualifies when one or more of the following is true.

### Transferable

Another competent agent or user could plausibly encounter the same problem.

### Consequential

It caused:

- meaningful delay;
- incorrect work;
- multiple retries;
- a context switch;
- substantial recovery work;
- inability to proceed.

### Recurring

The same underlying friction has happened more than once.

Even individually tiny problems become meaningful when recurrence reveals a structural issue.

### Misleading

The system:

- pointed at the wrong cause;
- hid the actual cause;
- behaved contrary to the apparent contract;
- produced an error that discouraged the correct fix.

### Systemic

The event reveals:

- a missing affordance;
- a documentation gap;
- a brittle interface;
- a recurring process defect;
- an unstable workflow;
- a reusable footgun.

---

## 6. What ordinarily should not become a cut

Usually skip:

- typos;
- shell quoting mistakes;
- a bad first guess;
- using the wrong command/API once;
- a patch missing because context was stale;
- a linter correctly catching newly written code;
- a compiler correctly rejecting newly written code;
- a malformed test fixture authored during the task;
- a one-off broad query returning too much output;
- a transient tactical mistake specific to the current agent run.

These should cross into cut territory only when recurrence or system behavior makes them meaningful.

A useful rule:

> **A cut must be consequential once or meaningful because it is transferable or recurring.**

---

## 7. Recurrence nuance

The admission rule must not eliminate the ability to discover small chronic friction.

Example:

```text
broad source query truncates output
```

Once:

```text
ignore
```

Repeated several times within the observable task/session context:

```text
Repeated large source reads caused output truncation and forced additional bounded reads; retrieval guidance or tooling should steer agents toward smaller source ranges.
```

Now it is a useful cut.

This is preferable to three records saying essentially:

```text
query too broad
```

### 7.1 Cross-session weak signals

There is one real architectural limitation:

If individually trivial friction happens across separate sessions and none of those events are retained, Blotter cannot detect the recurrence.

This is worth preserving as a future seam, but not solving yet.

Long-term:

```text
OTel / agent traces / runtime telemetry
                ↓
          weak-signal mining
                ↓
       recurrence / anomaly
                ↓
          admission boundary
                ↓
               CUT
```

The raw telemetry should remain outside the cut ledger.

---

## 8. Architectural conclusion

The research does **not** justify rebuilding Blotter.

The existing core is strong:

- append-only journal;
- cuts;
- dogears;
- evidence;
- tags;
- deterministic folding;
- triage;
- digest;
- retrospect;
- resolution;
- recurrence verification.

However, because breaking changes are acceptable, a clean v2 should make the conceptual model explicit and remove legacy scars.

---

# 9. v2 proposal

## 9.1 Make admission a first-class architectural boundary

Current conceptual model:

```text
friction
   ↓
 cut
   ↓
severity
```

v2:

```text
EXPERIENCE
    ↓
ADMISSION
"is this worth retaining?"
    ↓
CUT
    ↓
IMPACT
```

Admission does not need to be stored as a numeric field or implemented as an LLM classifier.

It is a normative semantic boundary.

---

## 9.2 Rename `severity` to `impact`

Current vocabulary:

```text
minor
major
blocker
```

Proposed:

```text
low
material
blocking
```

Definitions:

### low

Qualified friction with limited immediate cost.

### material

Lost meaningful time, caused incorrect work, or required substantial recovery.

### blocking

The task could not proceed.

Critical principle:

> **Low impact still means cut-worthy. It does not mean trivial.**

This removes the current conceptual overlap between severity and admission.

---

## 9.3 Delete the `auto` lane

Auto-capture has already been retired conceptually.

v2 should remove the remaining architecture:

- `auto` application semantics;
- `--include-auto`;
- auto-specific filtering;
- auto-specific warnings;
- retrospect special-casing;
- privileged tag behavior.

Tags should go back to being plain tags.

Old auto-captured records can remain in legacy v1 history or be excluded from migration.

Do not carry the experiment forward into the v2 contract.

---

## 9.4 Add `promotion` as a first-class event

This is the most important new primitive.

The current system can discover patterns, but the transition from:

```text
experience
```

to:

```text
durable institutional knowledge
```

is not represented in the ledger.

Add:

```text
promotion
```

Conceptually:

```json
{
  "kind": "promotion",
  "id": "blp_...",
  "ts": "...",
  "agent": "...",
  "sources": [
    "bl_cut1",
    "bl_cut2",
    "bl_cut3"
  ],
  "artifact": {
    "type": "skill",
    "ref": "skills/testing.md"
  },
  "note": "Repeated fixture failures promoted into reusable test-authoring guidance."
}
```

Initial artifact types can stay small:

```text
doc
skill
guard
test
tool
process
```

Blotter should record where the learning went.

It should **not** become the workflow engine that manages those artifacts.

---

## 9.5 Three first-class knowledge primitives

v2 becomes easy to reason about.

### Cut

> "We experienced something worth retaining."

Episodic friction.

### Dogear

> "We noticed something worth thinking about."

Hypothesis / idea / possibility.

### Promotion

> "We learned something worth changing the environment around."

Durable knowledge / intervention.

This is a stronger and cleaner ontology than the current two-record model plus implicit external promotion.

---

## 9.6 Replace loose resolution with explicit disposition

Current resolution semantics mix:

- fixed;
- dropped;
- closed;
- amended;
- linked task/PR/commit;
- implicit dismissal.

v2 should make the outcome explicit.

Proposed dispositions:

```text
fixed
promoted
accepted
invalid
```

### fixed

The environment/system was changed to remove the friction.

Should become a recurrence anchor.

### promoted

The friction was converted into reusable guidance or another durable improvement.

Should become a recurrence anchor.

### accepted

The friction is real but intentionally tolerated.

Recurrence is expected and should not be treated as a failed intervention.

### invalid

The original cut was incorrect, irrelevant, or not actually environmental friction.

Should not be used as a recurrence anchor.

This gives `verify` much stronger semantics.

---

## 9.7 Generalize retrospect one level

Current retrospective candidate types such as:

```text
wrapper_alias
doc_repair
skill_candidate
```

mix:

- pattern detection;
- proposed intervention.

v2 should separate them.

Potential pattern types:

```text
recurrent_friction
failed_intervention
repeated_recovery
documentation_gap
```

Then provide suggested interventions separately:

```text
suggested:
  - doc
  - skill
  - guard
```

This preserves deterministic heuristics while avoiding solution-specific ontology.

---

## 9.8 Add structured origin

Current `source` is too loose.

v2 should use an optional structured origin:

```json
{
  "origin": {
    "type": "agent"
  }
}
```

Future-compatible shape:

```json
{
  "origin": {
    "type": "trace",
    "provider": "otel",
    "trace_id": "..."
  }
}
```

This does not imply telemetry ingestion now.

It simply creates a clean seam for qualified externally detected experience later.

---

# 10. Proposed v2 lifecycle

```text
                         EXPERIENCE
                              │
                              ▼
                       ┌─────────────┐
                       │  ADMISSION  │
                       │ worth keeping?
                       └──────┬──────┘
                              │
                              ▼
                            CUT
                    qualified experience
                              │
                       impact + evidence
                              │
                ┌─────────────┴─────────────┐
                ▼                           ▼
             TRIAGE                       DIGEST
                │
                └─────────────┬─────────────┘
                              ▼
                          RETROSPECT
                         pattern mining
                              │
                              ▼
                     promotion candidate
                              │
                         judgment gate
                              │
                              ▼
                         PROMOTION
                   experience → knowledge
                              │
                              ▼
                           RESOLVE
              fixed / promoted / accepted / invalid
                              │
                              ▼
                            VERIFY
                              │
                  did the friction return?
```

Parallel idea stream:

```text
DOGEAR
   │
hypothesis / possibility
   │
independent stream
```

---

# 11. Product simplification

Although v2 adds one first-class event (`promotion`), the overall product becomes simpler.

The simplification comes from deleting ambiguity and special cases.

### Remove

- `auto` as privileged behavior;
- `--include-auto`;
- auto-specific filtering;
- auto-specific warnings;
- retrospect's special auto lane;
- `severity` as overloaded admission/impact language;
- loose "resolved means something happened" semantics;
- intervention-specific retrospect ontology.

### Clarify

- admission decides whether something enters Blotter;
- impact describes consequence after admission;
- tags are just tags;
- cut means meaningful observed friction;
- dogear means idea/hypothesis;
- promotion means experience became durable knowledge;
- resolution disposition tells us what actually happened;
- verify only evaluates claims where recurrence is meaningful.

The product gains one explicit concept while losing several implicit ones.

That is a net simplification.

---

# 12. Proposed CLI direction

Potential v2 surface:

```text
blotter add
blotter dogear
blotter list
blotter triage
blotter digest
blotter retrospect
blotter promote
blotter resolve
blotter verify
```

Supporting operational commands can remain where needed, but the primary mental model should fit this surface.

Avoid making `promote` a workflow manager.

It records:

- what experience produced the learning;
- what artifact/intervention resulted;
- where that artifact lives.

The external system still owns the artifact.

---

# 13. Things explicitly not to build yet

## No importance score

Do not add:

```text
importance: 0.72
salience: 5
actionability: high
confidence: 0.84
```

Blotter does not currently need an elaborate ranking model.

The binary semantic decision at capture is preferable:

```text
worth retaining
/
not worth retaining
```

## No LLM admission classifier

Fix the policy before adding mechanism.

The current signal problem is largely explained by instructions that explicitly encourage trivial filing.

## No new raw `event` / `signal` record

Do not recreate auto-capture under another name.

If exhaustive runtime telemetry eventually matters, it should exist beneath Blotter in an observability layer.

## No telemetry ingestion yet

Only preserve the architectural seam (`origin`) so this can be explored later.

## No generalized promotion plugin framework

Start with a small artifact vocabulary and actual demonstrated needs.

## No conversion of Blotter into task/project management

Backlogs, docs, skills, tests, PRs and guards remain external artifacts.

Blotter records learning provenance; it does not own the entire remediation workflow.

---

# 14. What should remain unchanged

Preserve the parts that are already structurally strong:

- append-only storage philosophy;
- deterministic behavior;
- cut and dogear terminology;
- one-line agent-oriented writing path;
- evidence support;
- tags;
- chronological provenance;
- clustering;
- recurrence detection;
- human/agent judgment before promotion;
- CLI-first architecture;
- closed-loop verification.

The goal is not a redesign for its own sake.

---

# 15. Design principles for v2

### Signal over exhaust

Blotter is not a transcript.

### Admission before impact

First decide whether an experience deserves persistence.

Then describe how much it hurt.

### Preserve observations; derive patterns

Do not destroy individual cut evidence just because triage can cluster it.

### Recurrence can elevate small friction

Low impact does not imply low information value.

### Patterns are not interventions

Retrospect identifies systemic structure.

Promotion records what was actually learned or changed.

### Learning requires provenance

If several experiences become a skill, doc, guard, test or tool, Blotter should retain the connection.

### Verification closes the loop

A purported fix or promoted learning is not successful merely because it was created.

Its value is partly demonstrated by the friction not recurring.

### Telemetry stays below Blotter

Blotter begins where raw execution becomes meaningful experience.

---

# 16. Suggested implementation sequence

Because breaking changes are acceptable, prefer a clean release boundary over compatibility scaffolding.

### Phase 1 — normative v2 spec

Define:

- admission policy;
- new `impact` semantics;
- cut/dogear/promotion ontology;
- resolution dispositions;
- promotion artifact schema;
- origin schema;
- recurrence behavior by disposition.

Do this before implementation.

### Phase 2 — remove legacy scars

Delete:

- `auto` semantics;
- auto-specific read flags;
- filters/warnings;
- retired hook artifacts that do not need compatibility retention in v2.

### Phase 3 — record model break

Change:

```text
severity → impact
```

Add:

```text
promotion
structured origin
resolution disposition
```

Update IDs/contract version if necessary rather than preserving awkward compatibility.

### Phase 4 — analysis semantics

Update:

- triage;
- digest;
- retrospect;
- verify.

Retrospect should emit pattern + suggested interventions rather than solution types as the fundamental pattern ontology.

Verify should only treat `fixed` and `promoted` resolutions as intervention anchors.

### Phase 5 — migration decision

Choose explicitly between:

1. v2 reads v1 records through a legacy parser;
2. one-time migration;
3. fresh v2 ledger with v1 history retained separately.

Do not allow migration requirements to weaken the v2 model.

Given the small project scale and willingness to break, a clean ledger boundary is viable.

---

# 17. Open questions

These should be answered in the v2 spec rather than improvised during implementation.

### Promotion identity

Should promotion IDs share the normal `bl_` namespace or use a distinct prefix?

### Promotion cardinality

Can one promotion reference:

- multiple cuts?
- multiple patterns?
- dogears?
- earlier promotions?

Likely yes for multiple cuts; keep recursive promotion conservative initially.

### Resolution relationship

Does creating a promotion automatically resolve its source cuts as `promoted`, or should promotion and resolution remain separate explicit events?

Recommendation: keep them separate so provenance and lifecycle remain independently auditable.

### Pattern persistence

Should retrospect patterns themselves ever become persisted records?

Recommendation for now: **no**.

Patterns remain derived views. Promotion is the durable judgment.

### Dogear promotion

Can a dogear be promoted directly into an artifact?

Probably yes eventually, but dogears represent hypotheses rather than observed friction, so this should not be conflated with cut-driven learning without explicit semantics.

### Accepted recurrence

Should accepted friction stay visible in triage/digest?

Likely yes in historical views, but it should not produce verify failure.

### Cross-session weak-signal mining

Keep this deferred until real evidence shows the higher admission floor is causing important recurrence to disappear.

---

# 18. Decision summary

## Adopt

- Raise the admission floor for cuts.
- Define admission separately from impact.
- Reposition Blotter as a selective experiential learning ledger.
- Use a breaking v2 release to clean up semantics.
- Rename `severity` to `impact`.
- Use `low / material / blocking`.
- Delete the `auto` lane and special behavior.
- Add `promotion` as a first-class durable event.
- Replace loose resolution semantics with explicit dispositions.
- Generalize retrospect from solution-specific candidate types toward pattern detection + suggested intervention.
- Structure `origin` for future externally discovered qualified experience.
- Preserve raw telemetry as an external lower layer.

## Do not adopt yet

- raw event records;
- auto-capture resurrection;
- AI admission scoring;
- importance/salience scores;
- LLM admission classifier;
- generalized promotion framework;
- integrated workflow/project management;
- direct OTel/trace ingestion.

---

# 19. Final model

Blotter v2 should represent a simple closed loop:

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

as the parallel hypothesis/idea channel.

The essential distinction is:

> **Cuts are selected experience. Patterns are derived understanding. Promotions are durable learning.**

This is both more expressive and simpler than the current product.

The architecture was not fundamentally wrong. The project has now been used enough to reveal what its true abstraction is.
