#!/usr/bin/env python3
"""Check active repository Markdown; optionally exercise the README against a CLI.

Python 3.11+, using the existing site/requirements.txt dependencies. No network,
package installation, model invocation, or writes to a real ledger.
"""
from __future__ import annotations

import argparse
from collections import deque
from html.parser import HTMLParser
import json
import os
from pathlib import Path
import posixpath
import re
import shlex
import subprocess
import sys
import tempfile
from urllib.parse import unquote, urlsplit

from markdown_it import MarkdownIt

ROOT = Path(__file__).resolve().parents[2]
ENTRY_LIMITS = {"README.md": 10_000, "AGENTS.md": 6_000}
HISTORY = {"CHANGELOG.md", "docs/history.md"}
MAP = "docs/README.md"
MARKER = "<!-- blotter:quickstart -->"


def plain(token) -> str:
    if token.children:
        return "".join(plain(child) for child in token.children)
    if token.type in {"softbreak", "hardbreak"}:
        return " "
    return token.content if token.type in {"text", "code_inline"} else ""


class HtmlLinks(HTMLParser):
    def __init__(self):
        super().__init__()
        self.links: list[str] = []
        self.ids: set[str] = set()

    def handle_starttag(self, tag, attrs):
        data = dict(attrs)
        for key in ("href", "src"):
            if data.get(key):
                self.links.append(data[key])
        for key in ("id", "name"):
            if data.get(key):
                self.ids.add(data[key])


def markdown(text: str) -> tuple[set[str], list[str], set[str]]:
    """Use the site's heading slug convention, including duplicate suffixes."""
    tokens = MarkdownIt("commonmark").parse(text)
    anchors: set[str] = set()
    links: list[str] = []
    h2: set[str] = set()
    for i, token in enumerate(tokens):
        if token.type == "heading_open":
            title = plain(tokens[i + 1])
            if token.tag == "h2":
                h2.add(title)
            stem = re.sub(r"[^\w\- ]", "", title.lower()).replace(" ", "-") or "section"
            slug, suffix = stem, 0
            while slug in anchors:
                suffix += 1
                slug = f"{stem}-{suffix}"
            anchors.add(slug)

    def visit(items):
        for token in items:
            if token.type == "link_open":
                links.append(token.attrGet("href") or "")
            elif token.type == "image":
                links.append(token.attrGet("src") or "")
            elif token.type in {"html_block", "html_inline"}:
                parser = HtmlLinks()
                parser.feed(token.content)
                links.extend(parser.links)
                anchors.update(parser.ids)
            if token.children:
                visit(token.children)

    visit(tokens)
    return anchors, links, h2


def repository_files(root: Path) -> set[str]:
    result = subprocess.run(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"],
        cwd=root, capture_output=True, check=True, timeout=15)
    return set(result.stdout.decode("utf-8").rstrip("\0").split("\0")) - {""}


def active_documents(paths: set[str]) -> set[str]:
    return {path for path in paths if path.endswith(".md")
            and path not in HISTORY and path != "CLAUDE.md"
            and ("/" not in path or path.startswith("docs/") or path == "skills/blotter/SKILL.md")
            and not path.startswith(("docs/archive/", "docs/research/"))}


def local_target(source: str, href: str) -> tuple[str, str] | None:
    parsed = urlsplit(href)
    if parsed.scheme or parsed.netloc:
        if parsed.scheme not in {"http", "https", "mailto"} or parsed.netloc and not parsed.scheme:
            raise ValueError(f"unsupported link: {href}")
        return None
    target = source if not parsed.path else posixpath.normpath(
        posixpath.join(posixpath.dirname(source), unquote(parsed.path)))
    if target.startswith("/") or target == ".." or target.startswith("../") or "\\" in target:
        raise ValueError(f"link escapes repository: {href}")
    return target, unquote(parsed.fragment)


def quickstart(text: str) -> list[list[str]]:
    if text.count(MARKER) != 1:
        raise ValueError("README must contain exactly one marked quickstart")
    tail = text.split(MARKER, 1)[1].lstrip()
    tokens = MarkdownIt("commonmark").parse(tail)
    if not tokens or tokens[0].type != "fence" or tokens[0].info != "bash":
        raise ValueError("quickstart marker must immediately precede one bash fence")
    commands = []
    for line in tokens[0].content.splitlines():
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        args = shlex.split(line)
        if len(args) < 2 or args[0] != "blotter" or args[1] not in {"add", "dogear", "list", "schema"}:
            raise ValueError("quickstart permits only direct blotter add/dogear/list/schema commands")
        if any(arg in {"--file", "--help", "--version", "--dry-run"} or arg.startswith("--file=") for arg in args[2:]):
            raise ValueError("quickstart must exercise capture without overriding its isolated ledger")
        commands.append(args)
    operations = {args[1] for args in commands}
    if not {"add", "dogear", "list"} <= operations:
        raise ValueError("quickstart must demonstrate a cut, a finding, and a read")
    return commands


def audit(root: Path, paths: set[str] | None = None) -> list[str]:
    root = root.resolve()
    paths = repository_files(root) if paths is None else paths
    active = active_documents(paths)
    errors: list[str] = []
    parsed: dict[str, tuple[set[str], list[str], set[str]]] = {}
    graph: dict[str, set[str]] = {}

    def read(path):
        target = root / path
        if not target.resolve().is_relative_to(root):
            raise ValueError(f"source escapes repository through symlink: {path}")
        if path not in parsed:
            parsed[path] = markdown(target.read_text(encoding="utf-8"))
        return parsed[path]

    for path in sorted(active):
        try:
            _, links, _ = read(path)
            graph[path] = set()
            for href in links:
                try:
                    match = local_target(path, href)
                    if match is None:
                        continue
                    target, anchor = match
                    dest = root / target
                    if not dest.resolve().is_relative_to(root):
                        raise ValueError(f"link escapes repository through symlink: {href}")
                    available = target in paths or any(p.startswith(target.rstrip("/") + "/") for p in paths)
                    if not available or not dest.exists():
                        raise ValueError(f"missing repository target: {href}")
                    if target in active:
                        graph[path].add(target)
                    if anchor and target.endswith(".md") and anchor not in read(target)[0]:
                        raise ValueError(f"missing Markdown anchor: {href}")
                except (OSError, ValueError) as exc:
                    errors.append(f"{path}: {exc}")
        except (OSError, ValueError) as exc:
            errors.append(f"{path}: {exc}")

    seen: set[str] = set()
    pending = deque([MAP])
    while pending:
        path = pending.popleft()
        if path not in seen:
            seen.add(path)
            pending.extend(graph.get(path, set()) - seen)
    errors.extend(f"unreachable active document: {p}" for p in sorted(active - seen))
    listed = graph.get(MAP, set())
    errors.extend(f"active document missing from the documentation map: {p}"
                  for p in sorted(active - listed - {MAP}) if p.startswith("docs/"))
    for path, maximum in ENTRY_LIMITS.items():
        if path not in active:
            errors.append(f"missing entry point: {path}")
        elif (root / path).stat().st_size > maximum:
            errors.append(f"{path}: exceeds {maximum}-byte entry-point budget; move detail to its owning page")
    link = root / "CLAUDE.md"
    if not link.is_symlink() or os.readlink(link) != "AGENTS.md":
        errors.append("CLAUDE.md must remain a symlink to AGENTS.md")
    if (root / "docs/plans/2026-07-09-papercuts-design.md").exists():
        errors.append("superseded amendment chain returned; keep history in Git, not the active contract")
    try:
        quickstart((root / "README.md").read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        errors.append(str(exc))
    try:
        excluded = set(json.loads((root / "context7.json").read_text())["excludeFiles"])
        for name in ("AGENTS.md", "CLAUDE.md", "CONTRIBUTING.md", "CHANGELOG.md", "contract.md", "history.md", "publication.md", "discovery-site.md"):
            if name not in excluded:
                errors.append(f"context7.json must exclude maintainer/history file {name}")
    except (OSError, ValueError, KeyError) as exc:
        errors.append(f"context7.json: {exc}")
    return errors


def runtime_checks(root: Path, binary: Path) -> int:
    binary = binary.resolve(strict=True)
    commands = quickstart((root / "README.md").read_text())
    reference_commands = markdown((root / "docs/reference.md").read_text())[2]
    with tempfile.TemporaryDirectory(prefix="blotter-docs-") as tmp:
        workspace = Path(tmp)
        env = {k: v for k, v in os.environ.items() if not k.startswith(("BLOTTER_", "GIT_"))}
        home = workspace / "home"
        home.mkdir()
        repo = workspace / "repo"
        repo.mkdir()
        subprocess.run(["git", "init", "--quiet", str(repo)], env=env, check=True, capture_output=True, timeout=15)
        ledger = repo / "quickstart.jsonl"
        env.update(HOME=str(home), USERPROFILE=str(home), XDG_CONFIG_HOME=str(home / ".config"),
                   BLOTTER_FILE=str(ledger), BLOTTER_AGENT="docs-smoke",
                   BLOTTER_NOW="2026-09-19T12:00:00Z")

        def run(args):
            result = subprocess.run([str(binary), "--file", str(ledger), *args], cwd=repo,
                                    env=env, capture_output=True, text=True, timeout=20)
            if result.returncode != 0 or result.stderr:
                raise ValueError(f"quickstart {args!r}: exit {result.returncode}: {result.stderr}")
            return result.stdout

        schema = json.loads(run(["schema"]))
        expected = set(schema["data"]["commands"])
        missing = expected - reference_commands
        if missing:
            raise ValueError(f"reference lacks command sections: {sorted(missing)}")
        for command in commands:
            result = run(command[1:])
            if "md" not in command:
                if not json.loads(result).get("ok"):
                    raise ValueError(f"quickstart returned a non-success envelope: {command}")
            elif not result.strip():
                raise ValueError("quickstart returned empty Markdown")
        records = json.loads(run(["list", "--kind", "all", "--status", "all"]))["data"]["items"]
        if {record["kind"] for record in records} != {"cut", "dogear"}:
            raise ValueError("quickstart did not retain both record kinds")
        if not ledger.is_file():
            raise ValueError("quickstart did not create the explicit temporary ledger")
        return len(commands)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, help="built executable; enables schema/README runtime checks")
    args = parser.parse_args()
    try:
        errors = audit(ROOT)
        if errors:
            print("Documentation check failed:\n" + "\n".join(f"- {e}" for e in errors), file=sys.stderr)
            return 1
        print("PASS: active Markdown links, anchors, navigation, entry points, and retrieval boundaries")
        if args.binary:
            count = runtime_checks(ROOT, args.binary)
            print(f"PASS: schema command coverage and {count} README commands in a disposable ledger")
        else:
            print("SKIP: schema/README runtime checks (pass --binary to enable)")
        return 0
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as exc:
        print(f"Documentation check failed: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
