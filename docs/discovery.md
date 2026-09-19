# Agent installation and programmatic discovery

Research checked: **2026-09-19**. This is a public integration and distribution guide, not a replacement for the CLI contract. Maintainers changing behavior must still follow `AGENTS.md` and the design document; consumers should start with the installed `blotter schema` and the [reference](reference.md).

For task-oriented examples, start with [Keep coding-agent friction from disappearing](agent-workflows.md).

## Publication status — verified September 19, 2026

[PR #37](https://github.com/BigCactusLabs/blotter/pull/37) is merged at `3e32788f77c1efcdc44ca28b81ad615c494d9dca`; its distribution files are now on `main`. Do not describe that PR as awaiting merge.

**Awesome Copilot already has a submission.** [Issue #2944](https://github.com/github/awesome-copilot/issues/2944) was opened on September 4 for plugin 1.0.1 at `e2a179b9e0ab3ff4c745a887b64383241b851bf5`. Its automated manifest, lint, installation, and version checks passed. A maintainer [rejected it on September 8](https://github.com/github/awesome-copilot/issues/2944#issuecomment-5578124028), describing the overlap with built-in Copilot memory and skill creation. This is a product-fit decision, not an unresolved packaging error. Do not create a duplicate submission or call it a pending listing. If reconsideration is warranted, address that feedback in the existing thread using the documented author `/rerun-intake` route, not a new issue.

**Context7 is configured, not confirmed indexed.** On September 19, an attempt to create a public-library assistance request in `upstash/context7` returned HTTP 403 (`Resource not accessible by integration`); no issue was created. This does not establish a problem with Context7's web form or API. The [Add Library page](https://context7.com/add-library) and [authenticated GitHub-repository API](https://context7.com/docs/api-guide) remain the submission routes. An authorized account must complete submission and record the returned library ID. Never treat the proposed `/BigCactusLabs/blotter` identity as an observed successful index.

**Skills visibility remains a bounded observation.** The five-query baseline below did not return Blotter. Installer tests opt out of telemetry and must not be repurposed into install farming. Successful installation, catalog acceptance, and useful model activation remain separate claims.

## Install a skill, not a second product

The canonical skill is [`skills/blotter/SKILL.md`](../skills/blotter/SKILL.md). It is deliberately self-contained: a skill installer may copy that directory without copying this repository's documentation. There are two independent components: the `blotter` executable, and guidance that tells a coding agent when and how to use it. Installing the skill or plugin does **not** install the executable.

Install the CLI using one of the [README installation methods](../README.md#install). Inspect skill discovery before installing:

```bash
npx skills add BigCactusLabs/blotter --list
npx skills add BigCactusLabs/blotter --skill blotter
```

The second command lets the installer select supported target agents. For an already authorized, noninteractive setup, select one client explicitly:

```bash
npx skills add BigCactusLabs/blotter --skill blotter --agent codex --yes
# Alternatives to codex include claude-code and github-copilot.
```

Node/npm is an installer prerequisite, not a Blotter runtime dependency. The tested Skills CLI version is 1.7.0, which declares Node >=22.20.0; the distribution workflow uses Node 24. Follow the installer prompts and the host's scope rules. Do not install both the standalone skill and plugin into the same client unless testing duplicate handling. The [Skills CLI](https://github.com/vercel-labs/skills) supports standard skill locations and client-specific installation; its anonymous install telemetry is separate from Blotter. To disable that third-party telemetry and associated audit requests:

```bash
DISABLE_TELEMETRY=1 npx skills add BigCactusLabs/blotter --skill blotter
```

Do not run repeated installs to manufacture a leaderboard signal. A genuine installation, an index entry, an agent reading a skill, and an agent using Blotter successfully are different events.

### Claude Code native plugin

The same skill is packaged through `.claude-plugin/plugin.json`; the repository's self-hosted marketplace is `.claude-plugin/marketplace.json`:

```text
/plugin marketplace add BigCactusLabs/blotter
/plugin install blotter@blotter-tools
```

This adds a marketplace explicitly; it does not put Blotter in Anthropic's official marketplace. Claude Code discovers `skills/` under the plugin root. The plugin adds no hooks, MCP servers, background processes, or permission grants. Its version is the **skill/plugin package version**, independent of the Rust CLI version. The root `plugin.json` retains the existing cross-ecosystem manifest; CI checks its metadata against the native manifest so they cannot silently diverge. See [Claude plugin reference](https://code.claude.com/docs/en/plugins-reference) and [marketplace documentation](https://code.claude.com/docs/en/plugin-marketplaces).

### Verify without touching a real ledger

From a clone, inspect packaging first:

```bash
python3 scripts/dev/check-discovery.py
```

For an executable smoke test, pass a built binary:

```bash
cargo build --release
python3 scripts/dev/check-discovery.py --binary target/release/blotter
```

The smoke test creates a temporary directory, sets an explicit `BLOTTER_FILE`, and checks schema, capture, list, and resolution there. No skill installer runs in this test; it cannot generate installation telemetry or prove a directory listing exists. Without `--binary`, the CLI smoke is reported as skipped, not passed.

Before a distribution release, also run the real host tools in a disposable installation environment:

```bash
claude plugin validate .
npx skills add . --list
```

The [Agent discovery workflow](../.github/workflows/discovery.yml) now runs those real tools, not just repository-specific schema checks. It pins Skills CLI 1.7.0 and Claude Code 2.1.278. It installs the canonical skill into six disposable targets: Claude Code, Codex, Cursor, GitHub Copilot, OpenCode, and Gemini CLI. It checks installed bytes, strictly validates the native plugin and marketplace, then installs, inspects, and uninstalls the native plugin in an isolated home. Only the pinned Claude package's lifecycle setup runs; suppressing that setup leaves its platform executable unconfigured. The first fully passing run is [35425766789](https://github.com/BigCactusLabs/blotter/actions/runs/35425766789) at commit `84b856b53ed8ce45b4b12188078d07dfc367d9a2` on September 19, 2026; it also caught and led to fixing a missing marketplace description before strict validation passed.

Run the equivalent locally with already installed host tools:

```bash
python3 -m unittest discover -s tests/discovery -p 'test_*.py' -v
python3 scripts/dev/check-discovery-hosts.py \
  --skills "$(command -v skills)" --claude "$(command -v claude)" \
  --report /tmp/blotter-installers.json
```

The test uses local source, telemetry opt-outs, and no inference credentials. It is installer compatibility evidence, not a measured activation rate. Claude can load local-directory plugins in place rather than into a separate cache, so the native test uses a disposable checkout copy. Git-hosted download/caching and actual model behavior remain separate checks. Installer reports include versions, commit, and skill SHA-256; Actions preserves the full outputs and the resolved npm lockfile as artifacts for 14 days. The repository pins direct host versions; the captured lockfile records the actual transitive dependency resolution for each run.

## The discovery model

Treat reach as a chain with independent failure points:

**retrieved for a relevant problem -> correctly understood -> installable -> activated at the right moment -> useful enough to retain.**

A successful manifest check proves only a small part of that chain. Optimize for a coding agent trying to preserve repeated workflow failures or review friction, not just an agent already searching for the word "Blotter". Avoid renaming the product into generic agent memory: the selective admission policy is its value.

| Channel | Actual mechanism | What this repository supplies | What still requires external evidence |
| --- | --- | --- | --- |
| Skills CLI / skills.sh | A standard skill can be installed; skills.sh documents install-telemetry-driven indexing | Canonical `SKILL.md`, semantic description, README install commands | Search visibility and genuine installation on supported hosts |
| Claude Code | A client adds a marketplace and installs a plugin | Native manifest, local marketplace, one canonical skill | Host validation and successful installation; official marketplace approval is separate |
| Context7 | A submitted repository's documentation is parsed for coding-assistant retrieval | `context7.json` with current usage scope and history exclusions | Submission, successful indexing, retrieval quality, and freshness |
| GitHub / web search | Search engines and code-search tools retrieve public source and documentation | Problem-language README, linked reference, explicit integration guide | Non-branded query results; neither public files nor metadata guarantee indexing |
| Explicit documentation fetch | A reader follows an index to the relevant raw Markdown | Root `llms.txt` with a short, curated set of links | A consumer actually fetching it; it is not a ranking or access-control directive |
| awesome-copilot external plugins | A reviewed external-plugin request can enter the catalog | Public plugin/skill packaging that can be pinned for review | Existing rejected submission; immutable commit/tag, product-fit review, and approval |

This guide records repository readiness, **not accepted directory submissions**. It does not assert that Blotter is currently listed by skills.sh, Context7, awesome-copilot, or Anthropic's official marketplace. Verify live state separately; a failed lookup is not proof that a listing is absent.

### Skills: availability is not discovery

The [skills.sh FAQ](https://www.skills.sh/docs/faq) describes automatic indexing through real installations performed with the Skills CLI. Merely adding a `SKILL.md` is not evidence of search placement. Its [API documentation](https://www.skills.sh/docs/api) exposes an authenticated `/api/v1` interface. Separately, the [Skills CLI search implementation](https://github.com/vercel-labs/skills/blob/v1.7.0/src/find.ts) uses `/api/search` and sorts returned candidates by installs. The audit below observes that implementation-level endpoint; it is not a promise of a stable public API or a bypass of the documented API's authentication. Repository-level `skills.sh.json` controls presentation of indexed pages, not an independent registration channel, so it is intentionally omitted for this one-skill package.

The [Agent Skills specification](https://agentskills.io/specification) makes the skill name and description the small initial discovery surface. The full instructions load on activation. That supports a compact description with genuine problem language, followed by precise operational guidance. It does not justify stuffing keywords or telling agents to load the skill on every task.

### Measured catalog baseline and repeatable audit

The first [live run](https://github.com/BigCactusLabs/blotter/actions/runs/35425438957) queried the Skills CLI endpoint on **September 19, 2026, 06:00:35–06:00:39 UTC**. All five requests returned HTTP 200 with valid JSON. Blotter's exact identity was not returned for `blotter` (18 candidates), `friction` (20), `agent friction` (20), `retrospective` (20), or `recurring tool failures` (20). This finite snapshot does not establish global catalog absence, Google visibility, or the cause of nonappearance. The checked-in [baseline summary](../tests/discovery/baseline-2026-09-19.json) preserves timestamps, response hashes, and run provenance; the run logs and artifact contain returned identities.

```bash
python3 scripts/dev/audit-discovery.py --live --report /tmp/blotter-catalog.json
```

This performs five public read-only GETs, never installs or registers anything. Results distinguish `found`, `not_returned`, `unavailable`, and `malformed`. HTTP failures, challenges, bad JSON, and oversized responses do not become false evidence of absence. This matters because the Skills CLI implementation collapses failed requests into an empty result list. The report retains API order and the CLI's install-sorted order separately. It records where Blotter appeared, not a semantic-quality score.

The workflow's catalog job is an observation, not a ranking gate: a completed job does not mean Blotter was found. Read each observation's status. New changes and manual workflow runs can produce comparable snapshots without a cron job or new telemetry in Blotter.

### Context7: help agents retrieve current usage, not old history

[`context7.json`](../context7.json) includes consumer documentation and the canonical skill. It excludes the long design-amendment history, archived/research notes, backlog, maintainer instructions, and changelog from normal usage retrieval. This prevents stale examples such as the retired hook/automatic-capture lane from competing with current guidance. Excluding the maintainer design document from a consumer index does not change its normative role for contributors.

Configuration is only parsing guidance: Context7 still needs an actual library submission and a successful index. The official [Library Owners guide](https://context7.com/docs/library-owners) specifies that root Markdown remains included even when folders are narrowed, and that `excludeFiles` uses filenames rather than full paths. The [Adding Libraries guide](https://context7.com/docs/adding-libraries) documents submission separately. After indexing, test retrieval for installation, `impact`, resolution dispositions, and the no-auto-promotion boundary; do not assume the config was applied just because JSON validates.

### llms.txt and ordinary search

The [llms.txt proposal, revised August 10, 2026](https://llmstxt.org/), describes a concise, request-time documentation index and links to Markdown. This repository exposes raw Markdown rather than duplicating a second set of prose. A repository file does not control GitHub's host-level robots rules or automatically publish a GitHub Pages website.

[Google's AI search guidance](https://developers.google.com/search/docs/appearance/ai-features) says ordinary search fundamentals apply and no special AI text file or special schema is required. Keep content crawlable, understandable, and linked. Do not promise AI-answer placement from this index. A dedicated documentation site can later expose Markdown alternate links and a sitemap, but a new website is not a prerequisite for shipping this distribution layer.

### Reviewed catalogs: submit once, with verifiable provenance

The [awesome-copilot contribution guide](https://github.com/github/awesome-copilot/blob/main/CONTRIBUTING.md) sends external plugins through a reviewed issue workflow, not a direct edit of `plugins/external.json`. The current [issue form](https://github.com/github/awesome-copilot/blob/main/.github/ISSUE_TEMPLATE/external-plugin.yml) and [canonical public-submission validator](https://github.com/github/awesome-copilot/blob/main/eng/external-plugin-validation.mjs) accept **a full commit SHA, a tag, or both**. A separate plugin release is not required for a SHA-pinned submission. The earlier release-first checklist imposed an unnecessary dependency. Never use `main` as an immutable pin.

The existing Blotter submission and rejection are recorded above. Before any reconsideration, make the concrete workflow inspectable: [the synthetic friction-lifecycle demonstration](agent-workflows.md#run-the-friction-lifecycle-demonstration) exercises distinct synthetic records, pre/post-fix boundaries, note-only amendments, and read-only analysis. It does not establish independent corroboration or measure a causal effect. These mechanics already existed; the demonstration is new evidence for understanding them, not a claim that Blotter gained new runtime capabilities or outperformed native memory. A maintainer may still reasonably decide the catalog fit is insufficient.

Do not add a release merely to satisfy a requirement the catalog does not have. Blotter's generated CLI release workflow matches broad version-like tags; any future separately versioned skill release must be designed not to trigger an unintended CLI publication. Do not change the generated release workflow casually.

## Prioritized continuation

1. **Keep installer evidence green; measure activation separately.** The workflow now exercises real installers. Run the trigger evaluation below through an actual model host; installer success is not automatic activation.
2. **Prioritize self-serve discovery and useful evidence.** The config is on `main`; complete Context7 submission through an authorized route and verify its returned content. Pair genuine Skills installations with bounded search observations. Treat Awesome Copilot as a rejected channel unless a maintainer reopens review; do not put the whole distribution plan behind catalog approval.
3. **Measure non-branded discovery.** Keep a dated retrieval baseline for queries such as "coding agent friction log", "recurring agent tool failures", "local agent retrospective", and "verify agent workflow fixes". Use the same providers and settings before and after changes. Store query, UTC timestamp, provider, returned URLs, target rank or not-found, and semantic-fit notes. Search position is provider- and time-dependent; do not treat it as an attribution model.
4. **Publish useful, consented artifacts rather than advertisements.** A genuinely useful guard, test, or engineering note derived from Blotter can carry an appropriate provenance link. Publish only reviewed artifacts and synthetic or explicitly approved examples, never private ledgers. The hypothesis is that useful artifacts bring agents back to the capture tool; validate it instead of adding promotional text to every output.

No Reddit posting is part of this plan. Do not modify users' other repositories or instructions to spread the skill without authorization.

## Activation evaluation: useful reach, not indiscriminate triggering

[`tests/discovery/trigger-queries.json`](../tests/discovery/trigger-queries.json) contains 20 hand-labeled cases: ten expected activations and ten near-miss negatives, split between development and held-out validation. They are evaluation inputs, **not evidence of a measured activation rate**.

Use the [Agent Skills description-optimization guidance](https://agentskills.io/skill-creation/optimizing-descriptions): expose the skill through the real host's normal discovery path, present each query in a fresh session, and observe whether the host actually loads it. Do not pre-read `SKILL.md` into every session. Repeat each case three times to see nondeterministic behavior; record host/model version, skill commit, prompt, activation, and outcome. Tune on development cases, then report validation separately. Do not upload real work logs as evaluation fixtures.

For positive cases, also inspect whether the agent uses the right kind of record, preserves independent recurrences, and respects promotion and privacy boundaries. For negatives, inspect whether it avoids filling the ledger with ordinary execution noise. Read-only analysis requests should not turn into writes. A structural test cannot prove any of these model behaviors.

## Deliberately deferred

**An MCP server only for directory eligibility.** The [official MCP Registry](https://modelcontextprotocol.io/registry/about) is a registry of MCP servers. Blotter is a CLI, not an MCP endpoint. Do not invent a `server.json` or misrepresent the CLI as a server. An adapter is worth considering only when actual consumers require that transport and its permissions, lifecycle, and write semantics are tested.

**CLI-generated plugin advertisements.** Claude Code's [plugin-hint mechanism](https://code.claude.com/docs/en/plugin-hints) is an interesting future channel, but it is limited to plugins in Anthropic's official marketplace. Emitting hints also needs a deliberate review of Blotter's stderr-is-errors contract. There is no hint emitter in this change.

**Artificial growth signals.** No install farming, automated stars, unrequested cross-repository edits, spam submissions, hidden tracking, or automatic publication of findings. The desired outcome is relevant agents finding and correctly using the tool, not a larger counter.