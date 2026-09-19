# Publish Blotter where coding agents can find it

Status checked: September 19, 2026. This is the operational companion to
[the discovery research and catalog history](discovery.md), not a new runtime contract.

## What is shipped and what is still external

[PR #38](https://github.com/BigCactusLabs/blotter/pull/38) is merged at
`38eab7ac2230d3a1d99a237fe1f4cd29c0938502`. Its corrected synthetic demonstration is
on `main`, alongside the distribution layer merged in #37. The final correction
was directly reviewed after the external review service reached its account limit;
do not describe the unavailable automated re-review as completed.

Public Git files, tested installation, accepted submission, searchable listing,
retrievable documentation, and useful model activation are distinct milestones.
Do not treat CI installation as a real-user adoption count or run installations
to inflate a directory ranking.

## Git-hosted delivery, not just local copies

The Agent discovery workflow now includes `check-discovery-remote.py` in addition
to the local host checks. It downloads the skill through Skills CLI at a full
commit SHA, fetches the native Claude marketplace by its published branch/ref,
checks the clone's resolved SHA, and verifies the plugin's separate cache contains
exactly the expected skill bytes. It then uninstalls the plugin and removes the
marketplace from its disposable home. A moving ref fails rather than being
silently treated as the expected build. The live run result, not this description,
is the evidence of success.

```bash
python3 scripts/dev/check-discovery-remote.py \
  --skills "$(command -v skills)" --claude "$(command -v claude)" \
  --ref main --expected-sha "$(git rev-parse HEAD)" \
  --report /tmp/blotter-remote.json
```

Run against a clean checkout of the named, published ref. The workflow supplies
the actual PR head or push SHA. Fork PRs retain local validation; their branches
are not falsely assumed to exist in the fixed upstream repository. Tests disable
host telemetry, inherit no inference credentials, and invoke no model. A remote
installation is not an automatic catalog registration claim.

Sources: [Skills CLI SHA-fetch implementation](https://github.com/vercel-labs/skills/blob/v1.7.0/src/git.ts)
and [Claude marketplace source, ref, and caching documentation](https://code.claude.com/docs/en/plugin-marketplaces).

## Context7: one credential, explicit submission, separate verification

Use the supported [Add Library page](https://context7.com/add-library), or the
repository's **Context7 publication** GitHub Actions workflow. The workflow uses
Context7's authenticated API, not the previously denied GitHub issue route.
No successful live submission or indexing is claimed merely because the workflow
exists. No usable Context7 connection or credential was available during this
implementation; the earlier 403 did not establish that the provider rejects Blotter.

An authorized maintainer adds a repository Actions secret named
`CONTEXT7_API_KEY`, then runs **Context7 publication** from `main` with action
`submit`. The secret must be a Context7 API key, not a GitHub token; keep it out of
chat, command arguments, committed files, and workflow inputs. This submits only
`https://github.com/BigCactusLabs/blotter` with `private: false`. It does not upload
local files, customer data, or private ledgers.

After the provider processes it, run the workflow with action `verify`. Verification
first requires the exact library identity in a search response and a `finalized`
provider state. It then requests installation, friction-capture, and recurrence
examples and requires nonempty snippets attributed to this repository for each.
The report records response hashes, timestamps, and source URLs, not private
teamspace rules. Human review still needs to check whether the retrieved examples
are current and correct; source attribution alone does not establish relevance.

The equivalent local commands use an already-provisioned environment key:

```bash
# Offline default: inspect the one public submission payload; no key needed.
python3 scripts/dev/publish-discovery-context7.py

# Each action is explicit; submit once, not on every CI run.
python3 scripts/dev/publish-discovery-context7.py --action submit --report /tmp/context7-submit.json
python3 scripts/dev/publish-discovery-context7.py --action verify --report /tmp/context7-verify.json
```

| Report status | Meaning |
| --- | --- |
| `planned` | No network request occurred. |
| `submission_accepted` | Provider returned the expected library ID; indexing has not been verified. |
| `not_ready` | Exact identity found, but the provider is not finalized. |
| `not_returned` | Exact identity was absent from this search response; not proof of global absence. |
| `retrieval_available` | Finalized metadata and own-repository snippets returned for all three probes; not an accuracy or activation score. |
| `retrieval_incomplete` | Finalized metadata, but at least one probe lacked attributable nonempty snippets. |
| `missing_or_invalid_credential` / `unauthorized` / `forbidden` | Credential/authorization problem; no success inferred. |
| `already_exists_unverified` | Submission returned 409; run verification rather than treating the conflict as successful indexing. |
| `rate_limited` / `transport_unavailable` / `malformed_response` | Observation failed; do not label the library absent. |

`index_verified: false` means this run did not verify indexing, not that the library
is necessarily unindexed. Only `planned`, `submission_accepted`, and
`retrieval_available` exit 0; inspect the action/status, not just a green job.
Requests have time and size bounds, refuse redirects, and do not retry writes.
The workflow never runs on PRs, pushes, or a schedule, and is restricted to this
repository's `main`. Provider limits or usage charges may apply to authorized API
requests. Do not silently enable paid API calls in ordinary CI.

Sources: [Adding libraries](https://context7.com/docs/adding-libraries),
[API guide](https://context7.com/docs/api-guide), and
[canonical OpenAPI contract](https://context7.com/docs/openapi.json), checked
September 19, 2026. The API guide calls for authentication; this integration
requires it for all live actions even though some GET operations in the OpenAPI
also describe anonymous access. No undocumented anonymous write route is used.

## Catalog decisions

Awesome Copilot's existing rejected [issue #2944](https://github.com/github/awesome-copilot/issues/2944)
is a product-fit decision, not a packaging failure. Do not create a duplicate
submission or make self-serve distribution depend on reconsideration. Skills.sh
[documents real-user installation telemetry](https://www.skills.sh/docs/faq) as its
listing mechanism; our disabled-telemetry CI installs are not a substitute for
users choosing the tool.
