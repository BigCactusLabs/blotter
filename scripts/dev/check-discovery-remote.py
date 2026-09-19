#!/usr/bin/env python3
"""Test Git-hosted skill and marketplace delivery, with telemetry disabled.

This downloads public source; it neither invokes models nor registers a catalog
listing. Requires the already-installed pinned host tools used by discovery CI.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import subprocess
import tempfile
from urllib.parse import quote

ROOT = Path(__file__).resolve().parents[2]
REPO = "https://github.com/BigCactusLabs/blotter"


def delivery_sources(ref: str, sha: str) -> tuple[str, str]:
    if not re.fullmatch(r"[0-9a-f]{40}", sha):
        raise ValueError("expected a full lowercase commit SHA")
    # Branch names are data, never shell fragments. Reject malformed Git refs.
    if (not ref or ref.startswith(("-", "/")) or ref.endswith(("/", "."))
            or any(part in ("", ".", "..") or part.startswith(".")
                   or part.endswith(".lock") for part in ref.split("/"))
            or any(token in ref for token in ("..", "@{", "//"))
            or re.search(r"[\s\x00-\x20\x7f~^:?*\[\\]", ref)):
        raise ValueError("expected a valid branch or tag name")
    return f"{REPO}/tree/{sha}", f"{REPO}.git#{quote(ref, safe='/.-_')}"


def check_skill(path: Path, boundary: Path, expected: bytes) -> str:
    actual = path.resolve(strict=True)
    if not actual.is_relative_to(boundary.resolve()):
        raise ValueError("downloaded skill escaped its disposable boundary")
    content = actual.read_bytes()
    if content != expected:
        raise ValueError("downloaded skill differs from the checked-out commit")
    return hashlib.sha256(content).hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--skills", type=Path, required=True)
    parser.add_argument("--claude", type=Path, required=True)
    parser.add_argument("--ref", required=True, help="published branch/tag for the native marketplace")
    parser.add_argument("--expected-sha", required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    report = {"schema_version": 1, "status": "failed", "checks": [],
              "model_activation_tested": False, "catalog_registration_performed": False,
              "telemetry_disabled": True, "expected_sha": args.expected_sha}
    try:
        skill_source, marketplace_source = delivery_sources(args.ref, args.expected_sha)
        helper_spec = importlib.util.spec_from_file_location("discovery_hosts", ROOT / "scripts/dev/check-discovery-hosts.py")
        helper = importlib.util.module_from_spec(helper_spec)
        helper_spec.loader.exec_module(helper)
        tools = {"skills": str(args.skills.resolve()), "claude": str(args.claude.resolve()), "git": "git"}
        canonical = (ROOT / "skills/blotter/SKILL.md").read_bytes()
        manifest = json.loads((ROOT / "plugin.json").read_text())
        report.update(skill_source=skill_source, marketplace_source=marketplace_source)

        with tempfile.TemporaryDirectory(prefix="blotter-remote-") as temporary:
            base = Path(temporary)
            project = base / "project"
            project.mkdir()
            env = helper.isolated_env(base / "home")
            env["GIT_TERMINAL_PROMPT"] = "0"

            def run(name: str, tool: str, *params: str) -> str:
                completed = subprocess.run([tools[tool], *params], cwd=project, env=env,
                                           capture_output=True, text=True, timeout=180, check=False)
                report["checks"].append({"name": name, "exit_code": completed.returncode,
                                         "stdout": completed.stdout[-6000:], "stderr": completed.stderr[-3000:]})
                if completed.returncode:
                    raise RuntimeError(f"{name} failed; inspect the bounded output in this report")
                return completed.stdout

            report["skills_version"] = run("skills-version", "skills", "--version").strip()
            report["claude_version"] = run("claude-version", "claude", "--version").strip()
            run("git-hosted-skill-install", "skills", "add", skill_source, "--skill", "blotter",
                "--agent", "codex", "--copy", "--yes")
            report["skill_sha256"] = check_skill(project / ".agents/skills/blotter/SKILL.md", base, canonical)
            run("git-marketplace-add", "claude", "plugin", "marketplace", "add", marketplace_source)
            markets = json.loads(run("git-marketplace-list", "claude", "plugin", "marketplace", "list", "--json"))
            if not isinstance(markets, list):
                raise ValueError("unexpected marketplace-list shape")
            matches = [row for row in markets if isinstance(row, dict) and row.get("name") == "blotter-tools"]
            if len(matches) != 1:
                raise ValueError("expected exactly one downloaded Blotter marketplace")
            checkout = Path(matches[0]["installLocation"]).resolve(strict=True)
            if not checkout.is_relative_to(base):
                raise ValueError("marketplace checkout escaped the disposable boundary")
            revision = run("downloaded-revision", "git", "-C", str(checkout), "rev-parse", "HEAD").strip()
            report["downloaded_sha"] = revision
            if revision != args.expected_sha:
                raise ValueError("remote ref moved or resolved to a different commit; do not infer a pass")
            run("git-plugin-install", "claude", "plugin", "install", "blotter@blotter-tools", "--scope", "user")
            rows = helper.plugin_rows(json.loads(run("git-plugin-list", "claude", "plugin", "list", "--json")))
            matches = [row for row in rows if row.get("id") == "blotter@blotter-tools"]
            if (len(matches) != 1 or matches[0].get("version") != manifest["version"]
                    or matches[0].get("enabled") is not True or matches[0].get("errors")):
                raise ValueError("remote plugin is not uniquely installed, enabled and error-free")
            cache = Path(matches[0]["installPath"]).resolve(strict=True)
            if cache == checkout or cache.is_relative_to(checkout):
                raise ValueError("remote plugin must be copied to a separate cache, not loaded from checkout")
            report["cached_skill_sha256"] = check_skill(cache / "skills/blotter/SKILL.md", base, canonical)
            run("git-plugin-uninstall", "claude", "plugin", "uninstall", "blotter@blotter-tools", "--scope", "user")
            rows = helper.plugin_rows(json.loads(run("git-plugin-list-after-uninstall", "claude", "plugin", "list", "--json")))
            if any(row.get("id") == "blotter@blotter-tools" for row in rows):
                raise ValueError("uninstall left a plugin registration")
            run("git-marketplace-remove", "claude", "plugin", "marketplace", "remove", "blotter-tools")
            report["status"] = "passed"
    except (OSError, ValueError, KeyError, TypeError, RuntimeError, subprocess.SubprocessError) as exc:
        report["error"] = str(exc)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    summary = report | {"checks": [{key: value for key, value in check.items() if key not in ("stdout", "stderr")}
                                  for check in report["checks"]]}
    print(json.dumps(summary, indent=2))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
