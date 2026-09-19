# Consumer site runbook

The website is generated from canonical Markdown. Do not maintain a second prose tree. [site/pages.json](../site/pages.json) and the builder's explicit allowlist select the consumer pages; [site/site.css](../site/site.css) supplies the presentation. [Publication operations](publication.md) cover catalogs and retrieval separately.

## Publication boundary

The build emits HTML, commit-pinned Markdown alternate links, `llms.txt`, section-level `docs.json`, `sitemap.xml`, a self-contained skill artifact, and `agent-skills.json` with that artifact's SHA-256. `site-manifest.json` records the commit, base URL, and generated file hashes. These are build evidence, not proof of deployment or indexing.

No executable JavaScript, remote fonts, external images, telemetry, or runtime server is added. The allowlist excludes ledgers, maintainer instructions, historical design material, backlog, and credentials. Existing links to public GitHub files can remain; exclusion from a site bundle is not access control for a public repository.

The `docs.json` format is Blotter-local, version 1. Skill discovery follows the experimental [Cloudflare 0.2.0 draft](https://github.com/cloudflare/agent-skills-discovery-rfc/blob/main/README.md); it is not an MCP endpoint or a claim of client support. A digest is byte-integrity evidence, not a publisher signature, safety review, or installation permission.

## Build and inspect

Python 3.11+; dependencies are build-only and pinned in [site/requirements.txt](../site/requirements.txt):

```bash
python3 -m venv /tmp/blotter-site-env
/tmp/blotter-site-env/bin/pip install --only-binary=:all: -r site/requirements.txt
/tmp/blotter-site-env/bin/python -m unittest discover -s tests/site -v
/tmp/blotter-site-env/bin/python scripts/dev/build-discovery-site.py \
  --output /tmp/blotter-public-site
```

The destination must not exist. The builder refuses to clear it and refuses dirty publication inputs, so source-revision links match the committed bytes. For preview, serve the generated directory under its configured base path. The **Discovery site** workflow packages a reviewable Pages artifact even when deployment is disabled.

Rendered heading IDs are prefixed, and internal page/fragment links are rewritten and checked. The repository documentation check validates source Markdown separately; run it through [the contributor gates](../CONTRIBUTING.md#validate-a-change).

## Enable Pages deliberately

A passing build does not establish a live site. Authorized one-time setup:

1. In repository Settings → Pages, select **GitHub Actions** as the source.
2. Set repository Actions variable `BLOTTER_DOCS_DEPLOY=true`.
3. Run **Discovery site** from `main`.

Relevant later main pushes may deploy; pull requests may only build. The workflow obtains the actual configured Pages base URL, including a custom hostname, rather than fabricating canonical URLs. Only the deploy job has Pages write and OIDC permissions, using the `github-pages` environment. Do not add an administrative credential or bypass a protected environment to avoid setup. Turning the variable off stops future deployment; it does not unpublish existing content.

Inspect the returned live URL: navigation destinations, unknown-path 404, `docs.json`, `llms.txt`, `agent-skills.json`, and the downloaded skill hash. Check GET/HEAD content types: JSON should be `application/json`; the skill should be `text/markdown` or `text/plain` for the discovery draft. Record actual responses and source commit. Only after verification should the repository homepage point there or an authorized search-console account submit its sitemap.

## Project path versus origin root

A project site under `/blotter/` cannot publish at the host's `/.well-known/`. Its build therefore exposes an explicitly linked `agent-skills.json`, not a false zero-configuration origin index or project-local robots directive.

A build configured for a real root-hosted HTTPS URL also emits `/.well-known/agent-skills/index.json` and `/robots.txt`. The hidden-file-aware artifact uploader preserves `.well-known`. Building with a hostname does not configure DNS, custom domains, content types, or client discovery. Those require separate authorized setup and live verification.

Sources: [GitHub Pages custom workflows](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages) and [Pages publishing source](https://docs.github.com/en/pages/getting-started-with-github-pages/configuring-a-publishing-source-for-your-github-pages-site).
