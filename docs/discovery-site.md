# A public documentation surface for humans and agents

Research checked September 19, 2026. This expands self-serve discovery without
another runtime, model integration, or copy of the product documentation to maintain.
It does not replace the [publication runbook](publication.md) or the
[discovery baseline and catalog history](discovery.md).

## What the build actually publishes

The builder reads an explicit five-file consumer allowlist: the root README,
`docs/choose-blotter.md`, `docs/agent-workflows.md`, `docs/reference.md`, and the
canonical `skills/blotter/SKILL.md`. It renders those into five HTML pages with
navigation, accessible headings, a table of contents, canonical URLs, descriptions,
and links to exact source revisions. The README's prose remains unchanged.

`site/pages.json` defines routes and page summaries; `site/site.css` supplies a
small responsive, light/dark stylesheet. No executable JavaScript, telemetry,
external images, remote fonts, or service is needed to read the pages. The home
page includes Schema.org `SoftwareSourceCode` data matching visible project
information. This is descriptive markup, not eligibility for a special search result.

The build also emits:

| File | Consumer and purpose |
| --- | --- |
| `llms.txt` | Compact fetch index pointing to commit-pinned source Markdown, not a ranking directive. |
| `docs.json` | Section-level text with titles, page anchors, source URLs, and hashes. This is a Blotter-local schema, version 1, for explicit retrieval; not a new standard. |
| `agent-skills.json` | Draft discovery 0.2.0 metadata: canonical skill name/description, `skill-md` type, artifact URL, and SHA-256 of the served bytes. |
| `skills/blotter/SKILL.md` | Byte-identical canonical skill artifact. Fetching it does not install the binary or authorize executing anything. |
| `sitemap.xml` | The five consumer HTML URLs; no fabricated update timestamps or priorities. |
| `site-manifest.json` | Source commit, base URL, publication mode, and hashes/sizes of generated files, excluding the manifest itself. Build evidence, not deployment or indexing evidence. |

HTML Markdown-alternate links point directly to the immutable source Markdown on
GitHub. This avoids rewritten Markdown diverging from its source. Relative links
inside source Markdown retain repository-relative semantics. In HTML, consumer
links resolve to the corresponding site page, while other repository references
point to the same source commit on GitHub. Generated heading IDs are prefixed;
the builder rewrites and validates internal fragment links.

Ledgers, legacy logs, `AGENTS.md`, `CLAUDE.md`, the backlog, research notes, and
publication credentials are not copied into the site or its section index. Links
to an already-public maintainer file may remain as GitHub links; excluding a file
from this bundle is not access control for its existing public repository copy.

## Build, inspect, then enable publication

The **Discovery site** workflow builds and checks PRs and relevant pushes. Its
Pages artifact can be inspected even when deployment is disabled. The builder
has no network calls and does not run Blotter against a ledger. Its two pinned
Python packages are build-only dependencies, isolated from the Rust product.

To reproduce a committed build with Python 3.11+:

```bash
python3 -m venv /tmp/blotter-site-env
/tmp/blotter-site-env/bin/pip install --only-binary=:all: -r site/requirements.txt
/tmp/blotter-site-env/bin/python -m unittest discover -s tests/site -v
/tmp/blotter-site-env/bin/python scripts/dev/build-discovery-site.py \
  --output /tmp/blotter-public-site
```

The output directory must not exist. The builder refuses to clear or overwrite it.
It also refuses dirty publication inputs, so source revision links describe the
committed files actually built. For a local preview, serve the generated directory
under the configured base path; a root-hosted preview can be built using a root
HTTPS base URL and served with a local static preview server.

**The repository reported `has_pages: false` when this work began. A generated
bundle is not a live website.** One-time authorized setup:

1. In repository Settings -> Pages, select **GitHub Actions** as the build source.
2. Add the repository Actions variable **`BLOTTER_DOCS_DEPLOY=true`**.
3. Run **Discovery site** from `main`. Subsequent relevant main-branch pushes
   publish automatically. Pull requests can build artifacts but cannot deploy.

The publishing build obtains its real base URL from `actions/configure-pages`,
so a configured custom hostname is not silently assigned project-path canonical
URLs. That action does not enable Pages with the ordinary `GITHUB_TOKEN`; do not
add an administrative token or bypass a protected environment to avoid setup.
Only the deploy job has `pages: write` and `id-token: write`. It uses the
`github-pages` environment. Turning the variable off stops future deployments;
it does not unpublish an already hosted site.

After deployment, check the returned Pages URL, the five navigation destinations,
`docs.json`, `llms.txt`, `agent-skills.json`, the downloaded skill hash, and an
unknown URL's 404. Check GET and HEAD content types: JSON must be
`application/json`, and the skill must be `text/markdown` or `text/plain` for the
draft discovery protocol. A successful deployment does not prove these responses
are correct or that any external index has ingested them. Record live checks in
an issue or PR; do not turn build logs into a claim of increased reach.

Once the site is live and verified, set the repository homepage to the returned
URL and link it from the root README. Then submit its sitemap through an authorized
search-console account. Do not link a proposed hostname as though it is deployed.
Context7 publication remains the separate credentialed workflow already documented.

## Experimental discovery without a false domain-root claim

Cloudflare's **Agent Skills Discovery via Well-Known URIs** is still a draft,
version 0.2.0, updated March 12, 2026. It specifies an origin-root index at
`/.well-known/agent-skills/index.json`, and digest-checked artifacts. It is not the
same format as the older `/.well-known/skills/` draft. The builder implements the
current index shape, not an invented server card or MCP endpoint.

A project site at `https://bigcactuslabs.github.io/blotter/` cannot publish metadata
at `https://bigcactuslabs.github.io/.well-known/`. Therefore the default build emits
an explicitly linked `agent-skills.json` only. It does **not** claim zero-configuration
origin discovery or publish a misleading project-local `robots.txt`.

When built for an actual root-hosted URL, such as an authorized dedicated hostname,
the builder also emits `/.well-known/agent-skills/index.json` and `/robots.txt`.
The Pages artifact uses the hidden-file-aware uploader so `.well-known` is not
silently dropped. No DNS or custom-domain configuration is performed by this code.

A matching digest demonstrates consistency between a fetched artifact and the
index. It is not a publisher signature, independent trust evidence, safety review,
or permission to install. A compromised publisher could replace both. Client
support, cache behavior, content types, and useful activation must be checked with
actual consumers after publication; no new host compatibility is asserted here.

## Why this is the next discovery surface

Current Google guidance for AI search says ordinary crawlability, useful textual
content, internal linking, and reliable page information still apply; no special
AI text file or special schema is required. The site is valuable as a readable,
linked home for the actual workflows, independent of adoption of the skills draft.

The new [choice guide](choose-blotter.md) addresses a concrete problem-language
query: how friction logging differs from remembered context and assigned work.
It describes what to inspect after a fix and when another ledger is unnecessary.
It does not claim competitors cannot implement the same workflow or claim that
synthetic tests prove productivity. This makes the positioning more specific
without changing the CLI or exaggerating the evidence.

The next evaluation is retrieval, not another metadata format: after publication,
check whether non-branded searches return the choice/workflow pages, whether fetched
snippets are current, and whether an authorized agent uses the skill correctly.
Compare against the existing baseline, recording provider, date, query, returned
identities, and failures separately. No install farming or auto-promotion is involved.

## Primary sources

- [Google: AI features and your website](https://developers.google.com/search/docs/appearance/ai-features).
- [Cloudflare discovery draft 0.2.0](https://github.com/cloudflare/agent-skills-discovery-rfc/blob/main/README.md).
- [Cloudflare: agent readiness and protocol discovery](https://blog.cloudflare.com/agent-readiness/).
- [GitHub: custom Pages workflows](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages).
- [GitHub: Pages publishing source](https://docs.github.com/en/pages/getting-started-with-github-pages/configuring-a-publishing-source-for-your-github-pages-site).
- [configure-pages action permissions](https://github.com/actions/configure-pages/blob/main/action.yml).
- [upload-pages-artifact v5 hidden-file support](https://github.com/actions/upload-pages-artifact/releases/tag/v5.0.0).
- [markdown-it-py security and raw HTML](https://markdown-it-py.readthedocs.io/en/latest/security.html).
- [Schema.org SoftwareSourceCode](https://schema.org/SoftwareSourceCode).
