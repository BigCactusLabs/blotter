# Friction logs, agent memory, and issue trackers

The same setup defect has tripped three coding sessions. Someone fixed it. A week
later, another session records matching friction. The useful question is not just
"does the agent remember the workaround?" It is **"what was recorded after the fix,
and which intervention does that recurrence relate to?"**

Blotter keeps those records in the repository and gives agents read-only commands
to inspect them. That is the reason to use it alongside a host's memory or a team's
issue tracker, not a reason to replace either one.

## Choose the record you actually need

| Need | Keep it here |
| --- | --- |
| A preference, remembered convention, or reusable context | Your agent's existing memory or project instructions |
| Assigned work, priority, milestones, and acceptance criteria | Your issue tracker |
| Every message, command, or span | A transcript or tracing system |
| A qualified workflow failure and its later recurrence | A Blotter cut |
| An observed engineering finding worth revisiting | A Blotter dogear |
| The relationship between recorded experience and an approved durable artifact | A Blotter promotion |

These are workflow distinctions, not claims that other tools are incapable of
storing the same information. Blotter's contribution is an explicit local format
and CLI for this particular job. The [reference](reference.md) describes the stored
records and commands; the installed `blotter schema` is the executable's contract.

## Keep a defect from disappearing between sessions

A misleading error, missing setup step, or brittle interface can be worth keeping
even after the agent recovers and completes its task. File the qualified occurrence,
then continue working. Do not require a search for existing entries first: a genuinely
new occurrence of familiar friction still carries signal.

That is not permission to manufacture duplicates. Several distinct record IDs,
agent labels, or similar texts do not prove independent corroboration. Keep actual
observations separate from hypotheses about causes and fixes.

The [capture workflow](agent-workflows.md#preserve-a-recurring-tool-failure) shows
how to record a cut without turning the ledger into a transcript.

## Review recurrence, not just recollection

`blotter triage` groups chronic open cuts. `blotter verify` relates later open
matching cuts to resolved ones. `blotter retrospect` packages recorded evidence
for a possible durable improvement. Those analysis commands are read-only.

Matching is a heuristic, not a causal experiment. No recurrence in the ledger
means none was observed there, not that a fix can never fail. Conversely, a later
matching record is a reason to inspect an intervention, not proof that the
intervention caused the failure.

The [runnable synthetic demonstration](agent-workflows.md#run-the-friction-lifecycle-demonstration)
checks pre/post-fix boundaries, note amendments, distinct recurring records, and
read-only behavior against the actual CLI. It is a mechanics demonstration, not
real-agent productivity evidence.

## Share evidence across agent hosts

A repository-owned JSONL file is inspectable without a proprietary memory viewer.
Agents that can invoke the CLI can work with the same ledger; the skill teaches
when to write, while `blotter schema` provides the installed command contract.
The [installation instructions](../README.md#install-the-agent-skill) keep the
executable and agent guidance separate.

This does not automatically make different hosts agree on what is worth recording.
Use the same admission rules, review noise, and evaluate actual activation in the
host you use. Installation compatibility and model behavior are different tests.

## Do not adopt another system without a reason

Blotter is unnecessary when a short project instruction already solves the problem,
when nobody will review the ledger, or when all you need is a task list. It should
not collect typos, every failed first attempt, invented observations, or private
model-internal commentary. Keep credentials and customer data out of examples and
review any ledger before sharing it.

Start with one repository, qualified real observations, and a later read-only
review. Judge whether the resulting evidence helps choose a concrete fix. Do not
use entry counts, repeated test installs, or directory listings as a substitute
for that judgment.
