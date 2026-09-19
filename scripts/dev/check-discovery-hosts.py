#!/usr/bin/env python3
"""Exercise real installers in disposable homes; no inference or catalog installs."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
TARGETS = {
    "claude-code": ".claude/skills", "codex": ".agents/skills",
    "cursor": ".agents/skills", "github-copilot": ".agents/skills",
    "opencode": ".agents/skills", "gemini-cli": ".agents/skills",
}


def isolated_env(home: Path) -> dict[str, str]:
    home.mkdir(parents=True, exist_ok=True)
    return {
        "PATH": os.environ.get("PATH", ""), "HOME": str(home),
        "XDG_CONFIG_HOME": str(home / ".config"),
        "CLAUDE_CONFIG_DIR": str(home / ".claude"),
        "TMPDIR": str(home), "CI": "1", "NO_COLOR": "1",
        "DISABLE_TELEMETRY": "1", "DO_NOT_TRACK": "1",
        "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC": "1",
        "DISABLE_AUTOUPDATER": "1", "DISABLE_ERROR_REPORTING": "1",
        "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": os.devnull,
    }


def plugin_rows(value: object) -> list[dict]:
    rows = value.get("installed", []) if isinstance(value, dict) else value
    if not isinstance(rows, list) or not all(isinstance(row, dict) for row in rows):
        raise ValueError("unexpected Claude plugin-list shape")
    return rows


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--skills", type=Path, required=True)
    parser.add_argument("--claude", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    tools = {name: str(getattr(args, name).resolve()) for name in ("skills", "claude")}
    report: dict = {"schema_version": 1, "checks": [], "inference_tested": False}
    report["commit"] = subprocess.check_output(["git", "-C", str(ROOT), "rev-parse", "HEAD"], text=True).strip()
    canonical = (ROOT / "skills/blotter/SKILL.md").read_bytes()
    report["skill_sha256"] = hashlib.sha256(canonical).hexdigest()

    def run(label: str, tool: str, cwd: Path, env: dict, *params: str) -> str:
        result = subprocess.run([tools[tool], *params], cwd=cwd, env=env,
                                capture_output=True, text=True, timeout=120, check=False)
        report["checks"].append({"name": label, "exit_code": result.returncode,
                                 "stdout": result.stdout[-6000:], "stderr": result.stderr[-3000:]})
        if result.returncode:
            raise RuntimeError(f"{label}: exit {result.returncode}: {result.stderr} {result.stdout}")
        return result.stdout

    try:
        with tempfile.TemporaryDirectory(prefix="blotter-hosts-") as tmp:
            base = Path(tmp)
            for name in tools:
                report[name + "_version"] = run(name + "-version", name, base,
                    isolated_env(base / "versions"), "--version").strip()
            for agent, destination in TARGETS.items():
                project = base / agent / "project"
                project.mkdir(parents=True)
                env = isolated_env(base / agent / "home")
                # Local source avoids both download ambiguity and fake install counts.
                run(agent + "-discover", "skills", project, env, "add", str(ROOT), "--list")
                run(agent + "-install", "skills", project, env, "add", str(ROOT),
                    "--skill", "blotter", "--agent", agent, "--yes", "--copy")
                skill = project / destination / "blotter/SKILL.md"
                if not skill.is_file() or skill.read_bytes() != canonical:
                    raise RuntimeError(f"{agent}: installed skill does not match source")
                if len(list((project / destination).glob("*/SKILL.md"))) != 1:
                    raise RuntimeError(f"{agent}: unexpected skills in isolated target")
                report["checks"].append({"name": agent + "-content", "passed": True})

            native = base / "native"
            native.mkdir()
            env = isolated_env(base / "native-home")
            # Validate a plugin-only copy as well as the actual marketplace root.
            staged = native / "plugin-only"
            (staged / ".claude-plugin").mkdir(parents=True)
            shutil.copy2(ROOT / ".claude-plugin/plugin.json", staged / ".claude-plugin/plugin.json")
            shutil.copytree(ROOT / "skills", staged / "skills")
            for label, path in (("native-manifest", staged), ("marketplace", ROOT)):
                run(label, "claude", native, env, "plugin", "validate", str(path), "--strict", "--json")
            run("marketplace-add", "claude", native, env, "plugin", "marketplace", "add", str(ROOT))
            run("native-install", "claude", native, env, "plugin", "install", "blotter@blotter-tools", "--scope", "user")
            rows = plugin_rows(json.loads(run("native-list", "claude", native, env, "plugin", "list", "--json")))
            matches = [row for row in rows if row.get("id") == "blotter@blotter-tools"]
            expected_version = json.loads((ROOT / "plugin.json").read_text())["version"]
            if len(matches) != 1 or matches[0].get("version") != expected_version or matches[0].get("errors") or matches[0].get("enabled") is not True:
                raise RuntimeError("native plugin not uniquely installed, enabled, and error-free")
            installed = Path(matches[0]["installPath"]).resolve()
            if not installed.is_relative_to(base) or (installed / "skills/blotter/SKILL.md").read_bytes() != canonical:
                raise RuntimeError("native cache escaped isolation or changed the skill bytes")
            run("native-uninstall", "claude", native, env, "plugin", "uninstall", "blotter@blotter-tools", "--scope", "user")
            rows = plugin_rows(json.loads(run("native-list-after-uninstall", "claude", native, env, "plugin", "list", "--json")))
            if any(row.get("id") == "blotter@blotter-tools" for row in rows):
                raise RuntimeError("native uninstall left an installed registration")
        report["status"] = "passed"
    except (OSError, ValueError, KeyError, subprocess.SubprocessError, RuntimeError) as exc:
        report.update(status="failed", error=str(exc))
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
