# Agent installation

Install the executable and agent guidance separately. A skill tells an agent when and how to use Blotter; it does not supply the `blotter` binary. Start with the [CLI installation methods](../README.md#install), then choose one skill route per agent.

## Install a skill, not a second product

The self-contained source is [skills/blotter/SKILL.md](../skills/blotter/SKILL.md). Inspect it before installation:

```bash
npx skills add BigCactusLabs/blotter --list
npx skills add BigCactusLabs/blotter --skill blotter
```

For an authorized noninteractive installation, select the client explicitly:

```bash
npx skills add BigCactusLabs/blotter --skill blotter --agent codex --yes
```

Node/npm is an installer prerequisite, not a Blotter runtime dependency. The repository's [installer workflow](../.github/workflows/discovery.yml) pins the tested Skills CLI and Claude Code versions; consult that file rather than copying another version table into documentation. Follow the selected host's installation scope. Do not install both the standalone skill and plugin into the same client unless deliberately testing duplicate handling.

The third-party [Skills CLI](https://github.com/vercel-labs/skills) has installation telemetry. Opt out with:

```bash
DISABLE_TELEMETRY=1 npx skills add BigCactusLabs/blotter --skill blotter
```

## Claude Code native plugin

```text
/plugin marketplace add BigCactusLabs/blotter
/plugin install blotter@blotter-tools
```

This adds Blotter's explicit marketplace, not an official marketplace listing. The plugin loads the same canonical skill and adds no binary, hooks, MCP server, background process, or permission grants. The plugin version is independent of the Rust CLI version. [Native plugin documentation](https://code.claude.com/docs/en/plugins-reference).

## Verify the two components

Check that `blotter --version` and `blotter schema` run in the environment where the agent will invoke them. Then inspect the host's installed skills/plugins. Installation alone does not establish that a model will load the skill at the right time.

From a checkout, the packaging check can exercise the CLI without opening your ledger:

```bash
python3 scripts/dev/check-discovery.py --binary target/release/blotter
```

Build that binary first with `cargo build --release`. Omitting `--binary` runs static packaging checks and explicitly skips the executable smoke test. For real host installation checks, use the [publication runbook](publication.md#installer-validation); those tests are separate from model activation.

## Start using it

Use the short [persistent instruction](../README.md#give-your-agents-the-pen) to establish capture in a repository. The [workflow guide](agent-workflows.md) covers admission, review, and later recurrence. The [reference](reference.md) covers behavior; the installed `blotter schema` is the complete machine interface. The [llms.txt](../llms.txt) index points to canonical Markdown rather than a second set of instructions.

## Activation evaluation: useful reach, not indiscriminate triggering

For maintainers, catalog status, submission, and [activation evaluation](publication.md#activation-evaluation) belong in the publication runbook. Public files, successful installs, search visibility, and useful activation are different outcomes.
