# Blotter v2 — frontier corroboration after Phase 4

**Date:** 2026-09-02  
**Status:** Research note; not contract  
**Branch reviewed:** `v2`  
**Question:** Does current frontier work on agent memory, self-evolving skills, production observability, and experience distillation corroborate Blotter v2's architecture, or suggest a structural change before 1.0?

## Executive summary

The fresh research pass corroborates the current Blotter v2 architecture more strongly than the earlier prior-art round did.

No source found argues for collapsing Blotter back toward raw telemetry, exhaustive memory, automatic capture, or direct automatic skill generation. The strongest current work instead converges on four ideas already present in v2:

1. **Experience should be admitted selectively.** Stored experience strongly steers future agent behavior, so low-quality retained experience can propagate errors.
2. **Repeated failures should be abstracted into patterns before they become guidance.** Raw trajectories are weaker memory than distilled, cross-instance structure.
3. **Experience → durable instruction is a real trust boundary.** Recent security work shows that repeated or causally framed trajectories can poison self-evolving skill systems.
4. **A learned intervention is not proven merely because it was created.** Later task outcomes, evaluators, and recurrence checks are the evidence that the intervention held.

That maps cleanly to Blotter's current direction:

```text
RAW EXECUTION / TRACE
        │
        │ outside Blotter
        ▼
     ADMISSION
        │
        ▼
       CUT
selected experience
        │
        ▼
     TRIAGE
recurrence evidence
        │
        ▼
   RETROSPECT
pattern hypothesis
        │
        ▼
   JUDGMENT GATE
is the evidence sufficient,
independent enough, and generalizable?
        │
        ▼
    PROMOTION
experience → durable intervention provenance
        │
        ▼
     VERIFY
later evidence supports or contradicts it
```

The only conceptual addition from this research round is to make the **judgment gate** between `retrospect` and `promote` explicit in the design language. Blotter already has this operationally because `promote` is an explicit mutation. It does **not** require a new record, field, score, or command for 1.0.

## 1. Selective admission is increasingly well-supported

### ACL 2026: experience-following and error propagation

Xiong et al., *How Memory Management Impacts LLM Agents: An Empirical Study of Experience-Following Behavior*, ACL 2026, studies memory addition/deletion and finds that retrieved experience strongly shapes subsequent agent outputs. The paper identifies two important failure modes:

- **error propagation** — inaccuracies in retained experience compound over time;
- **misaligned experience replay** — even apparently successful executions can later provide misleading guidance.

The authors explicitly argue for regulating experience quality in the memory bank and show that future task evaluations can act as quality labels for stored memory.

Source: https://aclanthology.org/2026.acl-long.27/

### Blotter implication

This strengthens the current admission-floor positioning:

> **Blotter is a selective ledger, not a transcript.**

The admission floor is not merely about making `digest` nicer to read. Once retained experience can influence later agent behavior or promotion decisions, admission quality becomes part of the reliability boundary.

This supports the existing decision to reject ordinary tactical exhaust rather than trying to rank it away later.

## 2. `cut → cluster → pattern` closely matches current research

### Findings of ACL 2026: Mistake Notebook Learning

Su et al., *Mistake Notebook Learning: Batch-Clustered Failures for Training-Free Agent Adaptation*, does not treat raw failed instances as reusable guidance. Its workflow is roughly:

```text
failures
   ↓
batch clustering
   ↓
shared error patterns
   ↓
structured mistake notes
   ↓
external memory update only when batch performance improves
```

The paper explicitly frames structured mistake abstraction as more useful than raw instance-level storage.

Source: https://aclanthology.org/2026.findings-acl.719/

### Blotter implication

This is unusually close to:

```text
cut
 ↓
triage
 ↓
retrospect
 ↓
promotion candidate
 ↓
promotion
```

The important difference is favorable to Blotter: Blotter stops before automatically mutating persistent instructions. `retrospect` interprets evidence; an explicit `promote` mutation records a judgment.

That separation should be protected.

## 3. Promotion is emerging as the most important trust boundary

The strongest new information in this research round comes from August 2026 security work on self-evolving agent skill systems.

### Trajectory poisoning

Chen et al., *When Experience Becomes Instruction: Trajectory Poisoning in Self-Evolving Agent Skill Systems*, studies systems that distill trajectories into persistent skills. The paper's core finding is that the promotion process can be manipulated: repeated, causally framed, domain-aligned trajectories can make attacker-chosen behavior appear generalizable enough to enter durable skills.

The authors explicitly characterize **evidence promotion as a security boundary**.

Source: https://arxiv.org/abs/2608.05563

### SkillJack

Ying et al., *SkillJack: Persistent Skill Backdoors in Self-Evolving Agents*, studies a related experience-to-skill attack surface. A key result is that malicious behavior can become harder to detect after skill extraction and can persist even after the original poisoned source records are removed.

This motivates provenance-aware skill lifecycle protection.

Source: https://arxiv.org/abs/2608.03509

### Confidence level

These are recent preprints, not settled peer-reviewed consensus. They should be treated as strong directional security evidence rather than final quantitative truth.

### Blotter implication

Blotter's current Phase-4 shape looks better in light of this work:

- `retrospect` is read-only;
- `promote` is explicit;
- promotion stores source-cut provenance;
- promotion does not mutate the referenced artifact;
- source cuts are retained/pinned so the provenance cannot silently disappear.

A useful design invariant is now:

> **Retrospect may recommend. It must never automatically call promote.**

No new implementation is needed for 1.0. The existing explicit mutation boundary already enforces the important part.

## 4. Recurrence is evidence, not independent corroboration

### When Not to Write Memory

Qi, Xu, and Li, *When Not to Write Memory: Governing False Promotion from Correlated Agent Traces*, focuses specifically on false promotion from correlated evidence. Its key observation is that several apparently separate observations may all derive from the same shared source, prompt, stale state, or narrow context.

The paper proposes dependency-aware support and a `promote / reject / needs-review` governance model.

Sources:

- DOI / record: https://dblp.org/rec/journals/corr/abs-2607-02579.html
- Accessible abstract: https://www.researchgate.net/publication/408523133_When_Not_to_Write_Memory_Governing_False_Promotion_from_Correlated_Agent_Traces

### Confidence level

This is a preprint / CoRR work. Its exact reported effect sizes should not be treated as established. The conceptual warning is nevertheless directly relevant and consistent with the trajectory-poisoning results above.

### Blotter implication

The important refinement is:

```text
three related cuts
```

is not automatically equivalent to:

```text
three independent pieces of evidence
```

Three cuts from independent tasks, agents, environments, or sources are stronger evidence than three cuts induced by the same stale instruction or shared upstream context.

Blotter should therefore preserve this distinction conceptually:

> **Recurrence supports promotion only insofar as the underlying evidence is sufficiently independent and generalizable.**

For 1.0, this remains a judgment concern rather than a scoring mechanism.

Do **not** add:

- an independence score;
- a promotion confidence score;
- a provenance-diversity classifier;
- an LLM promotion gate.

`sources[]`, `origin`, timestamps, agent identity, and the explicit judgment step already preserve enough provenance to support more sophisticated reasoning later if it becomes necessary.

## 5. Promotion should remain provenance, not skill management

### Demystifying Agent Skills

Jiang et al., *Demystifying Agent Skills: Why They Work—Until They Don't*, analyzes 8,135 trials and finds that skills primarily help as **procedural anchors**, not as factual knowledge injection. The paper reports that procedural anchoring accounts for 65.7% of observed skill mechanisms versus 4.5% for explicit knowledge injection.

It also finds a retrieval bottleneck: as tested skill pools grow from 5 to 100, actual-use precision falls from 29.6% to 3.3%.

Source: https://arxiv.org/abs/2608.14036

### Blotter implication

This supports keeping promotion artifacts small and procedural:

```text
doc
skill
guard
test
tool
process
```

It also strengthens the decision that Blotter should **not own the artifact library**.

If Blotter began managing skill selection, activation, supersession, retrieval, version compatibility, or pruning, it would inherit an entirely different product's complexity.

The current promotion abstraction is better:

```text
these experiences
      ↓
became this durable intervention
```

Blotter records the provenance edge and stops there.

## 6. Verification as a separate stage is strongly corroborated

Several systems now explicitly separate memory creation from subsequent verification.

### ACL memory-management result

The ACL 2026 experience-following study notes that future task evaluations can serve as quality labels for stored memories.

Source: https://aclanthology.org/2026.acl-long.27/

### ProcMEM

Mi et al., *ProcMEM: Learning Reusable Procedural Memory from Experience via Non-Parametric PPO for LLM Agents*, transforms episodic experience into executable skills and introduces a separate PPO Gate for skill verification plus score-based maintenance.

Source: https://arxiv.org/abs/2602.01869

### Compiled Memory

Rhodes and Kang, *Compiled Memory: Not More Information, but More Precise Instructions for Language Agents*, explicitly uses a multi-step promotion gate before accumulated experience rewrites persistent agent instructions.

Source: https://arxiv.org/abs/2603.15666

### Blotter implication

The architecture should continue to distinguish:

```text
promotion ≠ success
```

and instead use:

```text
promotion
   ↓
later experience
   ↓
verify
```

This supports the Phase-5 decision to make recurrence semantics depend on `disposition_ts`, not a later note-only amendment timestamp.

One wording caveat should remain in documentation and mental models:

> **Absence of recurrence is evidence that an intervention held, not proof that it is universally fixed.**

The opportunity to encounter the failure may simply not have arisen again.

No command rename is warranted; `verify` is still an appropriate product term as long as its semantics remain explicit.

## 7. Production agent tooling is converging on the same closed loop

### LangSmith Engine

LangSmith Engine describes a continuous-improvement loop:

```text
production traces
   ↓
recurring issue
   ↓
root-cause diagnosis
   ↓
proposed fix
   ↓
evaluator
   ↓
automatic reopen if recurrence returns
```

It also creates offline evaluation examples from production traces.

Source: https://docs.langchain.com/langsmith/engine

### Arize Signal

Arize Signal reviews production traces on a recurring schedule, groups recurring failure patterns into prioritized issues, surfaces evidence and likely cause, and can carry an investigation into a repository. Proposed changes still pass through datasets, evaluators, experiments, and engineer review; the managed agent does not deploy its own changes unchecked.

Sources:

- https://arize.com/blog/debug-production-ai-agents-with-signal-tutorial/
- https://arize.com/blog/from-signal-to-pr/
- https://arize.com/blog/building-ai-factory-self-improving-agents-arize-ax/

### Blotter implication

These production systems reinforce the same layering:

```text
telemetry
   ↓
meaningful issue / experience
   ↓
pattern
   ↓
reviewable intervention
   ↓
regression evidence
```

Blotter's distinguishing decision remains sensible: it starts **above raw telemetry**, at selected experience.

If exhaustive runtime traces become important later, they should remain a lower observability layer feeding qualified evidence into Blotter rather than becoming another Blotter record type.

## 8. Updated architecture assessment

The frontier pass increases confidence in the current v2 architecture.

### Strongly corroborated

- **Higher cut admission floor.** Retained experience can steer future behavior, so low-value/noisy writes have downstream cost.
- **Cuts as episodic selected experience.** Raw execution remains outside the semantic ledger.
- **Triage as recurrence evidence.** Repetition is useful signal, especially when paired with provenance and judgment.
- **Retrospect as pattern interpretation.** Current research repeatedly favors distillation/abstraction over raw trajectory replay.
- **Explicit promotion.** The experience→durable-instruction boundary increasingly looks like a trust/security boundary.
- **Promotion provenance.** Source retention matters because skill extraction can otherwise sever the evidence trail.
- **Promotion remaining passive.** Blotter should record where learning went, not own the resulting skill/doc/test/tool lifecycle.
- **Verify as a separate downstream stage.** Later outcomes are necessary evidence about whether learned interventions are actually useful.

### Newly sharpened

- **Recurrence is not equivalent to independent evidence.** Correlated cuts can manufacture apparent generality.
- **The judgment gate should be explicit in product philosophy.** `retrospect` produces a hypothesis; `promote` records a deliberate trust decision.
- **Verification language should remain probabilistic.** No recurrence means no observed recurrence, not universal proof of correctness.

## 9. 1.0 recommendations

### Add to the design language, not the code

Two short invariants are worth carrying forward:

> **Promotion is an explicit trust boundary. Derived patterns may recommend an intervention; they never create durable instruction automatically.**

> **Recurrence supports promotion only when the evidence is sufficiently independent and generalizable; repetition from shared context is not independent corroboration.**

These do not require a new record field or command.

### Keep the current Phase-5 plan

The current TASK-77 direction remains the right 1.0 bar:

- use `disposition_ts` for recurrence timing and expose it in verify output;
- remove `suggested_action` from raw triage/digest output;
- treat TASK-71 as a measured precision gate before retrospect's v2 pattern semantics ship;
- keep retrospect's vocabulary small and evidence-backed;
- keep promotion explicit and passive.

### Do not add for 1.0

- automatic promotion;
- LLM admission or promotion classifiers;
- promotion confidence/salience scores;
- provenance-independence scores;
- raw event/trace records;
- telemetry ingestion;
- automatic skill generation or mutation;
- persisted pattern objects;
- dogear promotion;
- a skill/artifact retrieval system inside Blotter.

## 10. Net conclusion

The fresh frontier research does not uncover a structural reason to change course.

If anything, the newest work makes Blotter's restraint look more important:

- it does not treat every execution event as memory;
- it does not treat every recurrence as a rule;
- it does not let pattern detection silently become durable instruction;
- it records provenance when experience becomes an intervention;
- it keeps downstream verification separate from the act of promotion.

The most defensible current model is therefore:

```text
Cut       = selected experience
Pattern   = derived understanding
Promotion = explicit, provenance-bearing learning decision
Verify    = later evidence that supports or contradicts the intervention
```

The remaining frontier risk is not that Blotter retains too little. It is that future versions might make the jump from **pattern** to **promotion** too automatic. v2's explicit promotion command is therefore not merely a workflow choice; it is increasingly well-supported as the correct architectural trust boundary.

## Sources

Peer-reviewed / conference proceedings:

- Xiong et al. (ACL 2026), *How Memory Management Impacts LLM Agents: An Empirical Study of Experience-Following Behavior* — https://aclanthology.org/2026.acl-long.27/
- Su et al. (Findings of ACL 2026), *Mistake Notebook Learning: Batch-Clustered Failures for Training-Free Agent Adaptation* — https://aclanthology.org/2026.findings-acl.719/

Recent preprints / frontier research; directional, not settled consensus:

- Chen et al., *When Experience Becomes Instruction: Trajectory Poisoning in Self-Evolving Agent Skill Systems* — https://arxiv.org/abs/2608.05563
- Ying et al., *SkillJack: Persistent Skill Backdoors in Self-Evolving Agents* — https://arxiv.org/abs/2608.03509
- Qi et al., *When Not to Write Memory: Governing False Promotion from Correlated Agent Traces* — https://dblp.org/rec/journals/corr/abs-2607-02579.html
- Jiang et al., *Demystifying Agent Skills: Why They Work—Until They Don't* — https://arxiv.org/abs/2608.14036
- Mi et al., *ProcMEM: Learning Reusable Procedural Memory from Experience via Non-Parametric PPO for LLM Agents* — https://arxiv.org/abs/2602.01869
- Rhodes & Kang, *Compiled Memory: Not More Information, but More Precise Instructions for Language Agents* — https://arxiv.org/abs/2603.15666

Production / practitioner systems:

- LangSmith Engine — https://docs.langchain.com/langsmith/engine
- Arize Signal tutorial — https://arize.com/blog/debug-production-ai-agents-with-signal-tutorial/
- Arize Signal launch / Signal-to-PR — https://arize.com/blog/from-signal-to-pr/
- Arize self-improving agent loop — https://arize.com/blog/building-ai-factory-self-improving-agents-arize-ax/
