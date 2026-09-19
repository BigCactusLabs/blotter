# Keep coding-agent friction from disappearing

Blotter is a local friction log for AI coding agents. Use it when a misleading error, recurring tool failure, missing setup step, or brittle interface contains knowledge the next agent should not have to rediscover. The ledger stays in the repository; the agent can resume its real task immediately after writing a qualified observation.

This is selective experience capture, not conversation memory or issue tracking. Keep product tasks in your issue tracker, chat history in your transcript, and meaningful workflow friction in Blotter. For exact flags and output contracts, run `blotter schema` or read the [CLI reference](reference.md).

## Run the friction-lifecycle demonstration

From a clone, with Python 3.10+ and an already installed Blotter:

```bash
python3 scripts/dev/demo-discovery-proof.py
# Or use the executable you just built:
python3 scripts/dev/demo-discovery-proof.py --binary target/release/blotter
```

The [demonstration](../scripts/dev/demo-discovery-proof.py) creates a disposable workspace and uses an explicit ledger path. It downloads nothing, invokes no model, and never opens your real ledger. Every observation is a synthetic fixture; the three agent names are labels, not three running AI systems.

It exercises an inspectable sequence rather than asking a model whether the tool sounds useful:

1. Three independently identified observations of one workflow defect form a chronic cluster.
2. One cut is marked fixed while two matching **pre-fix** cuts remain open. Those older cuts must not count as later recurrences.
3. Two **post-fix** observations cause `verify` to report one resolved anchor and two distinct recurring cuts. Anchors and occurrences are different counts.
4. A later note-only amendment must not move the original fix boundary or hide those recurrences.
5. `retrospect` surfaces failed-intervention evidence, while the read commands leave the ledger byte-identical and create no promotion records.

The script exits nonzero if an assertion fails and emits JSON evidence on success. CI runs it against the real compiled executable. This demonstrates existing CLI behavior; it is not new functionality, a model-activation benchmark, a productivity measurement, or proof that another memory system cannot implement a similar workflow.

The concrete reason to choose Blotter is an inspectable, host-independent record of **what recurred after which intervention**. A useful integration can pair that record with an agent's native memory rather than trying to replace it.

## Set up Claude Code, Codex, Cursor, or another coding agent

Install the [CLI](../README.md#install) and [agent skill](discovery.md#install-a-skill-not-a-second-product) separately. The skill teaches admission and review rules; it does not bundle the executable. The same workflow works with any agent allowed to invoke the CLI in the target repository. A successful skill installation does not establish that the host will automatically load it for every relevant situation.

For continuous capture, keep the short instruction block in the README's [Give your agents the pen](../README.md#give-your-agents-the-pen) section in the project's existing `AGENTS.md` or `CLAUDE.md`. A short persistent instruction can establish when to write; the skill carries the fuller procedure. Do not paste the entire reference into every prompt. Review any proposed instruction change like other project configuration.

## Preserve a recurring tool failure

A qualifying cut needs at least one reason to keep it: transferable, consequential, recurring, misleading, or systemic. A correct compiler error in code just written is normally noise. A misleading setup command that repeatedly sends fresh clones down the wrong path is not.

For an actual observation matching this example, from the repository where it happened:

```bash
blotter add "The documented root-level test command finds no tests because the runner starts in apps/web" --tag tests --impact material
```

Use your own observed text, not the example verbatim. File the occurrence and continue working. Do not run `blotter list` first to decide whether it is new: independent recurrences help reveal chronic problems. Do not manufacture multiple records from one observation.

Attach the failed command, exit status, or bounded stderr only when it makes the observation more useful. Keep suspected causes in evidence rather than presenting them as established facts. Do not attach secrets or full environment dumps.

## Review a week of agent friction

```bash
blotter digest --since 7d --format md
blotter triage
```

The digest combines chronic open friction, new cuts in the window, and open findings. Its chronic section is not limited to the last seven days. Triage groups recurring open cuts. Both commands are read-only; neither creates issues, rewrites instructions, or implements a fix.

Ask the reviewing agent: "Which recurring problem would a small documentation fix, guard, or test remove? Show the recorded evidence." Choose an intervention from the evidence, not from the number of log entries alone.

## Check whether a workflow fix held

After fixing the actual defect, resolve its real cut identifier:

```bash
blotter resolve CUT_ID --disposition fixed
blotter verify
```

Replace `CUT_ID` with the identifier returned by Blotter. `verify` looks for later open cuts linked to resolved friction. No matches means no recurrence was observed in the ledger, not proof that the system can never fail again. For automation, use the installed schema's exit-code dictionary: `verify` uses exit 1 when it finds recurrences, not only for execution failures.

## Turn repeated friction into a durable improvement

```bash
blotter retrospect
```

Inspect the evidence and choose whether to build a doc, skill, guard, test, tool, or process change. `retrospect` only proposes candidates. After an approved artifact exists, `blotter promote` records its relationship to source cuts. Promotion does not automatically resolve those cuts, and repetition never authorizes automatic promotion. See the [promotion reference](reference.md#promote) for the explicit write and resolution steps.

## Preserve an engineering finding instead of a complaint

Use a dogear for one observation in your own words that is interesting beyond this task and understandable without this repository. A surprising measurement can qualify; an untested performance guess or backlog chore does not. Use `blotter dogear`, then review before publishing. Recording a finding does not verify it or grant permission to publish the underlying code or data.

## Keep private work private

Blotter normally writes `.blotter.jsonl` at the repository root. Follow the team's choice to commit that file or keep it private. `blotter doctor --leaks` is a useful pre-publication check, not a confidentiality guarantee. Synthetic examples are enough to demonstrate a workflow; real customer logs are unnecessary.

The expected outcome is concrete: qualified experiences remain available to future agents, maintainers can identify repeated friction, and fixes can be checked against later observations. Discoverability and model activation still need their own [evaluation](discovery.md#activation-evaluation-useful-reach-not-indiscriminate-triggering).
