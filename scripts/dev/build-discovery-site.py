#!/usr/bin/env python3
"""Build an allowlisted, script-free documentation site from canonical sources.

Python 3.11+; dependencies in site/requirements.txt. No network or CLI invocation.
Output must be a new directory: this command never clears an existing directory.
"""
from __future__ import annotations

import argparse
import hashlib
from html import escape
from html.parser import HTMLParser
import json
from pathlib import Path, PurePosixPath
import posixpath
import re
import subprocess
import sys
import tomllib
from urllib.parse import quote, unquote, urljoin, urlsplit

from markdown_it import MarkdownIt

ROOT = Path(__file__).resolve().parents[2]
REPO = "https://github.com/BigCactusLabs/blotter"
RAW = "https://raw.githubusercontent.com/BigCactusLabs/blotter"
SCHEMA = "https://schemas.agentskills.io/discovery/0.2.0/schema.json"
DEFAULT_URL = "https://bigcactuslabs.github.io/blotter/"
# Adding pages requires a reviewed code change, not a recursive docs copy.
SOURCES = {"README.md", "docs/choose-blotter.md", "docs/agent-workflows.md",
           "docs/reference.md", "skills/blotter/SKILL.md"}


def base_url(value: str) -> str:
    parts = urlsplit(value)
    if (parts.scheme != "https" or not parts.hostname or parts.username or parts.password
            or parts.query or parts.fragment or parts.port or parts.netloc != parts.hostname
            or not re.fullmatch(r"[a-z0-9.-]+", parts.hostname)
            or not re.fullmatch(r"/[A-Za-z0-9_/-]*", parts.path or "/")
            or "//" in parts.path or "." in parts.path):
        raise ValueError("base URL must be HTTPS with a plain host and safe optional path")
    return value.rstrip("/") + "/"


def read_source(root: Path, path: str) -> bytes:
    relative = PurePosixPath(path)
    if relative.is_absolute() or ".." in relative.parts:
        raise ValueError("unsafe source path")
    current = root
    for part in relative.parts:
        current = current / part
        if current.is_symlink():
            raise ValueError(f"symlink is not a publication source: {path}")
    if not current.is_file() or current.stat().st_size > 512_000:
        raise ValueError(f"missing or oversized source: {path}")
    return current.read_bytes()


def skill_fields(text: str) -> tuple[str, str, str]:
    match = re.match(r"\A---\r?\n(.*?)\r?\n---\r?\n", text, re.S)
    if not match:
        raise ValueError("skill frontmatter missing")
    fields = []
    for key in ("name", "description"):
        values = re.findall(rf"^{key}: ([^\r\n]+)$", match[1], re.M)
        if len(values) != 1:
            raise ValueError(f"expected one single-line skill {key}")
        fields.append(values[0])
    name, description = fields
    if (not re.fullmatch(r"[a-z0-9]+(?:-[a-z0-9]+)*", name) or len(name) > 64
            or not 1 <= len(description) <= 1024):
        raise ValueError("invalid discovery metadata")
    return name, description, text[match.end():]


def plain(token) -> str:
    if token.children:
        return "".join(plain(child) for child in token.children)
    if token.type in {"softbreak", "hardbreak"}:
        return " "
    return token.content if token.type in {"text", "code_inline", "fence", "code_block"} else ""


def headings(tokens) -> list[dict]:
    seen: set[str] = set()
    result = []
    for index, token in enumerate(tokens):
        if token.type != "heading_open":
            continue
        title = plain(tokens[index + 1])
        stem = re.sub(r"[^\w\- ]", "", title.lower()).replace(" ", "-") or "section"
        slug, suffix = stem, 0
        while slug in seen:
            suffix += 1
            slug = f"{stem}-{suffix}"
        seen.add(slug)
        token.attrSet("id", "doc-" + slug)
        result.append({"id": "doc-" + slug, "title": title, "level": int(token.tag[1:]), "index": index})
    return result


def resolve_link(href: str, source: str, routes: dict[str, str], base: str, revision: str) -> str:
    parsed = urlsplit(href)
    if parsed.scheme:
        if parsed.scheme not in {"https", "http", "mailto"}:
            raise ValueError("unsupported link scheme")
        return href
    if parsed.netloc:
        raise ValueError("protocol-relative links are not supported")
    if not parsed.path:
        return ("?" + parsed.query if parsed.query else "") + ("#doc-" + parsed.fragment if parsed.fragment else "")
    path = posixpath.normpath(posixpath.join(posixpath.dirname(source), unquote(parsed.path)))
    if path.startswith("/") or path == ".." or path.startswith("../") or "\\" in path:
        raise ValueError(f"link escapes repository: {href}")
    if path in routes:
        target = urljoin(base, routes[path])
    elif path == "llms.txt":
        target = urljoin(base, "llms.txt")
    else:
        target = f"{REPO}/blob/{revision}/{quote(path, safe='/')}"
    return target + ("?" + parsed.query if parsed.query else "") + ("#" + ("doc-" if path in routes else "") + parsed.fragment if parsed.fragment else "")


def rewrite_tokens(tokens, source: str, routes: dict, base: str, revision: str) -> None:
    for token in tokens:
        if token.type == "link_open":
            token.attrSet("href", resolve_link(token.attrGet("href") or "", source, routes, base, revision))
        if token.type == "image":
            # No third-party image requests, tracking pixels, or remote assets.
            token.type, token.tag, token.nesting = "html_inline", "", 0
            token.content = escape(plain(token) or "[image]")
            token.children = None
        elif token.children:
            rewrite_tokens(token.children, source, routes, base, revision)


class Links(HTMLParser):
    def __init__(self):
        super().__init__()
        self.ids: set[str] = set()
        self.urls: list[str] = []

    def handle_starttag(self, tag, attrs):
        data = dict(attrs)
        if "id" in data:
            if data["id"] in self.ids:
                raise ValueError(f"duplicate anchor: {data['id']}")
            self.ids.add(data["id"])
        for key in ("href", "src"):
            if data.get(key):
                self.urls.append(data[key])


def validate_links(files: dict[str, bytes], base: str) -> None:
    pages = {}
    for path, content in files.items():
        if path.endswith(".html"):
            parser = Links()
            parser.feed(content.decode("utf-8"))
            pages[path] = parser
    for path, parser in pages.items():
        for href in parser.urls:
            absolute = urljoin(base + path, href)
            if not absolute.startswith(base):
                continue
            parsed = urlsplit(absolute)
            relative = unquote(parsed.path[len(urlsplit(base).path):])
            target = relative + "index.html" if not relative or relative.endswith("/") else relative
            if target not in files:
                raise ValueError(f"broken internal link in {path}: {href}")
            if parsed.fragment and target in pages and unquote(parsed.fragment) not in pages[target].ids:
                raise ValueError(f"broken anchor in {path}: {href}")


def json_bytes(value) -> bytes:
    return (json.dumps(value, ensure_ascii=False, indent=2) + "\n").encode("utf-8")


def build(root: Path, destination: Path, url: str, revision: str) -> dict:
    base = base_url(url)
    if not re.fullmatch(r"[0-9a-f]{40}", revision):
        raise ValueError("source revision must be a full lowercase commit SHA")
    if destination.exists() or destination.is_symlink():
        raise ValueError("output must be a new directory; refusing to overwrite")
    config = json.loads(read_source(root, "site/pages.json"))
    if not isinstance(config, list) or {page["source"] for page in config} != SOURCES or len(config) != len(SOURCES):
        raise ValueError("pages must exactly match the consumer source allowlist")
    routes = {page["source"]: page["route"] for page in config}
    if (len(set(routes.values())) != len(routes) or routes["README.md"] != ""
            or any(not re.fullmatch(r"(?:[a-z0-9]+(?:-[a-z0-9]+)*/)?", route) for route in routes.values())):
        raise ValueError("unsafe or duplicate page route")
    skill = read_source(root, "skills/blotter/SKILL.md")
    name, description, skill_body = skill_fields(skill.decode("utf-8"))
    if name != "blotter":
        raise ValueError("unexpected skill identity")
    skill_dir = root / "skills/blotter"
    if list(skill_dir.iterdir()) != [skill_dir / "SKILL.md"]:
        raise ValueError("skill-md publication requires a self-contained, single-file skill")
    package = tomllib.loads(read_source(root, "Cargo.toml").decode("utf-8"))["package"]
    files = {"assets/site.css": read_source(root, "site/site.css"),
             "skills/blotter/SKILL.md": skill, ".nojekyll": b""}
    md = MarkdownIt("commonmark", {"html": False}).enable("table")
    documents = []
    nav = "".join(f'<a href="{escape(base + routes[src])}">{label}</a>' for src, label in (
        ("README.md", "Start"), ("docs/choose-blotter.md", "Why Blotter"),
        ("docs/agent-workflows.md", "Workflows"), ("docs/reference.md", "Reference"),
        ("skills/blotter/SKILL.md", "Skill")))
    for page in config:
        source, route = page["source"], page["route"]
        content = read_source(root, source)
        tokens = md.parse(skill_body if source.endswith("SKILL.md") else content.decode("utf-8"))
        outline = headings(tokens)
        rewrite_tokens(tokens, source, routes, base, revision)
        canonical = base + route
        markdown = f"{RAW}/{revision}/{source}"
        sections = []
        for offset, item in enumerate(outline):
            end = outline[offset + 1]["index"] if offset + 1 < len(outline) else len(tokens)
            text = "\n".join(plain(token) for token in tokens[item["index"] + 1:end]
                             if token.type in {"inline", "fence", "code_block"})
            sections.append({"title": item["title"], "url": canonical + "#" + item["id"], "text": text})
        documents.append({"title": page["title"], "url": canonical, "markdown_url": markdown,
                          "source_path": source, "source_sha256": hashlib.sha256(content).hexdigest(), "sections": sections})
        toc = "".join(f'<li><a href="#{escape(item["id"])}">{escape(item["title"])}</a></li>'
                      for item in outline if item["level"] == 2)
        structured = {"@context": "https://schema.org", "@type": "SoftwareSourceCode", "name": "Blotter",
                      "codeRepository": REPO, "programmingLanguage": "Rust", "version": package["version"],
                      "license": REPO + "/blob/" + revision + "/LICENSE", "url": base,
                      "description": config[0]["description"]}
        data_script = ('<script type="application/ld+json">' + json.dumps(structured).replace("<", "\\u003c") + '</script>') if not route else ""
        html = f'''<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>{escape(page['title'])}</title><meta name="description" content="{escape(page['description'])}">
<meta name="referrer" content="no-referrer"><link rel="canonical" href="{escape(canonical)}">
<link rel="alternate" type="text/markdown" href="{escape(markdown)}" title="Canonical source Markdown">
<link rel="stylesheet" href="{base}assets/site.css">{data_script}</head>
<body><a class="skip" href="#main">Skip to content</a>
<header><a class="wordmark" href="{base}">blotter<span> / field notes for agents</span></a><nav aria-label="Primary">{nav}</nav></header>
<div class="layout"><main id="main"><article>{md.renderer.render(tokens, md.options, {})}</article>
<section class="machine" aria-labelledby="machine-title"><h2 id="machine-title">For agents and tooling</h2>
<p>Read the <a href="{base}llms.txt">documentation index</a>, <a href="{base}docs.json">section-level JSON</a>, or <a href="{base}agent-skills.json">draft skill index</a>. Use <code>blotter schema</code> for the installed CLI contract. Finding a skill does not authorize installing or running it.</p></section></main>
<aside><details open><summary>On this page</summary><nav aria-label="Contents"><ul>{toc}</ul></nav></details></aside></div>
<footer><a href="{escape(markdown)}">Source Markdown</a> · <a href="{REPO}/blob/{revision}/{source}">Source at {revision[:7]}</a>
<p>Local ledger. No dashboard. This documentation site loads no executable JavaScript, analytics, or remote fonts.</p></footer></body></html>'''
        files[route + "index.html"] = html.encode("utf-8")
    index = {"$schema": SCHEMA, "skills": [{"name": name, "description": description, "type": "skill-md",
             "url": base + "skills/blotter/SKILL.md", "digest": "sha256:" + hashlib.sha256(skill).hexdigest()}]}
    files["agent-skills.json"] = json_bytes(index)
    root_hosted = urlsplit(base).path == "/"
    # /.well-known and /robots.txt are origin-root conventions, not project paths.
    if root_hosted:
        files[".well-known/agent-skills/index.json"] = json_bytes(index)
        files["robots.txt"] = f"User-agent: *\nAllow: /\nSitemap: {base}sitemap.xml\n".encode()
    files["docs.json"] = json_bytes({"schema_version": 1, "source_commit": revision, "documents": documents})
    lines = ["# Blotter", "", "> A repository-owned friction ledger for coding agents.", "",
             "Use blotter schema for the installed executable. Installation and writes require authorization.",
             "These are commit-pinned source documents; relative links inside them are repository-relative.", "", "## Documentation", ""]
    lines += [f"- [{page['title']}]({RAW}/{revision}/{page['source']}): {page['description']}" for page in config]
    lines += ["", "## Optional", "", f"- [Section-level documentation JSON]({base}docs.json): Local schema version 1, not an industry protocol.",
              f"- [Draft skill index]({base}agent-skills.json): Explicit-fetch discovery with a content digest, not proof of host activation."]
    files["llms.txt"] = ("\n".join(lines) + "\n").encode()
    files["sitemap.xml"] = ('<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n'
                            + "".join(f"<url><loc>{escape(base + page['route'])}</loc></url>\n" for page in config) + "</urlset>\n").encode()
    validate_links(files, base)
    report = {"schema_version": 1, "source_commit": revision, "base_url": base, "page_count": len(config),
              "skill_discovery_mode": "origin-root-draft" if root_hosted else "explicit-index-only",
              "deployment_tested": False, "files": {path: {"sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)}
                                                    for path, data in sorted(files.items())}}
    files["site-manifest.json"] = json_bytes(report)
    destination.mkdir(parents=True)
    for path, content in sorted(files.items()):
        target = destination / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(content)
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--base-url", default=DEFAULT_URL)
    args = parser.parse_args()
    try:
        revision = subprocess.check_output(["git", "-C", str(ROOT), "rev-parse", "HEAD"], text=True).strip()
        # Source links must name the actual committed content, not a dirty preview.
        tracked = sorted(SOURCES | {"site", "Cargo.toml", "scripts/dev/build-discovery-site.py"})
        dirty = subprocess.check_output(["git", "-C", str(ROOT), "status", "--porcelain", "--", *tracked], text=True)
        if dirty:
            raise ValueError("commit publication inputs before producing a commit-pinned site")
        report = build(ROOT, args.output, args.base_url, revision)
    except (ValueError, OSError, KeyError, TypeError, subprocess.SubprocessError) as exc:
        print(f"Site build failed: {exc}", file=sys.stderr)
        return 1
    print(json.dumps(report, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
