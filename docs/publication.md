# Publication runbook

Use this page for distribution operations, not ordinary installation. Consumers start with [agent installation](discovery.md). Build/deploy operations for the consumer site are in [the site runbook](discovery-site.md).

## Evidence and status

The following is a **September 19, 2026 repository audit snapshot**, not a live status service:

| Surface | Evidence available | Not established by that evidence |
| --- | --- | --- |
| Skills and native Claude plugin | Packaging plus local/remote installer checks are implemented in the discovery workflow | Automatic model activation or useful adoption |
| Skills catalog | [Five-query baseline](../tests/discovery/baseline-2026-09-19.json) did not return Blotter | Global absence or the cause of nonappearance |
| Awesome Copilot | [Existing issue #2944](https://github.com/github/awesome-copilot/issues/2944) was rejected for product fit | A pending listing or a packaging failure needing a duplicate submission |
| Context7 | Configuration and explicit submit/verify workflow exist | Successful submission, finalized indexing, or correct retrieval |
| Consumer site | An allowlisted site bundle builds | A live deployment, correct hosted responses, or search indexing |

Keep new observations in the relevant issue/PR or generated report, with the source commit and timestamp. Do not accumulate merged-PR narration or stale “next step” lists in the installation guide. Successful installation, accepted submission, searchable listing, useful retrieval, and model activation remain separate claims.

## Installer validation

Use disposable environments and the versions pinned in [.github/workflows/discovery.yml](../.github/workflows/discovery.yml):

```bash
python3 -m unittest discover -s tests/discovery -p 'test_*.py' -v
python3 scripts/dev/check-discovery-hosts.py \
  --skills "$(command -v skills)" --claude "$(command -v claude)" \
  --report /tmp/blotter-installers.json
```

The local test checks skill bytes across supported targets and the native plugin/marketplace in an isolated home. It opts out of telemetry, invokes no model, and uses no inference credentials. Inspect its report rather than treating tool presence as a successful installation.

Test Git-hosted delivery against a clean checkout of a **published** ref:

```bash
python3 scripts/dev/check-discovery-remote.py \
  --skills "$(command -v skills)" --claude "$(command -v claude)" \
  --ref main --expected-sha "$(git rev-parse HEAD)" \
  --report /tmp/blotter-remote.json
```

The expected SHA must match that ref. The test rejects a moving ref, compares cached skill bytes, and cleans up the disposable installation. Fork pull requests retain local validation rather than pretending their branches exist upstream. Installer success is not a catalog registration or an activation score.

## Context7 submission and verification

Use the supported [Add Library page](https://context7.com/add-library), or the main-only **Context7 publication** workflow. The earlier denied GitHub-issue route is not a provider rejection and should not be retried as the publication mechanism.

An authorized maintainer provisions the Actions secret `CONTEXT7_API_KEY`, then explicitly runs action `submit`. Keep the key out of chat, committed files, command arguments, and workflow inputs. This submits only the public Blotter repository URL, not local files or private ledgers. Submission may incur provider usage charges; ordinary CI must not silently enable live requests.

After processing, run action `verify`. Verification requires the exact library identity, finalized metadata, and attributable nonempty snippets for three probes. Inspect the snippets for current installation, capture, and recurrence guidance; attribution alone is not correctness.

```bash
# Offline payload preview; no key or network request.
python3 scripts/dev/publish-discovery-context7.py

# Explicit live actions, using an already-provisioned environment key.
python3 scripts/dev/publish-discovery-context7.py --action submit --report /tmp/context7-submit.json
python3 scripts/dev/publish-discovery-context7.py --action verify --report /tmp/context7-verify.json
```

`planned` means offline only; `submission_accepted` is not verified indexing; `retrieval_available` means all three attribution probes returned, not that a model used them correctly. `not_ready`, `not_returned`, and `retrieval_incomplete` are bounded observations. A 409 is `already_exists_unverified`: verify rather than inferring success. Credential, rate-limit, transport, and malformed-response errors are not evidence of absence. Only planned, accepted, and retrieval-available reports exit 0; read the action and status, not just the job color.

The script uses bounded requests, refuses redirects, and does not retry writes. The workflow is manual, not triggered by pull requests, pushes, or a schedule. Sources: [Context7 API guide](https://context7.com/docs/api-guide) and [adding libraries](https://context7.com/docs/adding-libraries).

## Retrieval observations

```bash
python3 scripts/dev/audit-discovery.py --live --report /tmp/blotter-catalog.json
```

This performs five read-only public requests, not installs or submissions. Reports distinguish found, not returned, unavailable, and malformed; include timestamps, response hashes, and returned order. A finite query result is not proof of global catalog absence. Do not turn the observational job into a ranking gate.

For non-branded web retrieval, retain the provider, date, query, returned identities, and semantic-fit notes. Compare like-for-like settings. A public `llms.txt` is a fetch index, not a ranking guarantee. Ordinary crawlability and useful linked content remain the foundation; see [Google's AI search guidance](https://developers.google.com/search/docs/appearance/ai-features).

## Activation evaluation

[The trigger queries](../tests/discovery/trigger-queries.json) contain 20 labeled positive/negative cases split into development and held-out validation. They are inputs, not a measured activation rate.

Expose the skill through the real host's normal discovery path. Use fresh sessions without preloading the skill, repeat each case three times, and record host/model version, skill commit, prompt, activation, and outcome. Tune on development cases and report validation separately. Positive cases must use the right record type and preserve privacy/promotion boundaries; negatives should avoid ordinary execution noise. Read-only requests must remain read-only.

The [synthetic lifecycle demonstration](agent-workflows.md#run-the-friction-lifecycle-demonstration) tests CLI mechanics, not independent observations, causal productivity gains, or model behavior. Sources: [Agent Skills description evaluation](https://agentskills.io/skill-creation/optimizing-descriptions).

## Publication boundaries

Do not create a duplicate Awesome Copilot request; address the existing product-fit feedback only when reconsideration is justified. Do not manufacture Skills telemetry through CI installations. Do not submit an MCP server manifest for a CLI, add promotional output to the runtime, spread instructions into unrelated repositories, or publish findings/private ledgers automatically.

Useful reviewed artifacts can carry appropriate provenance. A source hash establishes byte consistency, not independent trust or installation permission. Follow [release guidance](../CONTRIBUTING.md#releases-and-distribution) before creating any tag: the generated CLI release workflow has a broad version-like trigger, and skill versions are independent.
