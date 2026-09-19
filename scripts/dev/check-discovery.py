#!/usr/bin/env python3
"""Offline distribution consistency checks; optional isolated CLI smoke.

This checks this repository's conventions, not the complete third-party schemas,
real host activation, or external catalog acceptance. Python 3.10+, stdlib only.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
BINARY: Path | None = None
REPO = "https://github.com/BigCactusLabs/blotter"
RAW = "https://raw.githubusercontent.com/BigCactusLabs/blotter/main/"


def unique_object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def load_json(relative: str):
    return json.loads((ROOT / relative).read_text(), object_pairs_hook=unique_object)


def frontmatter_scalar(text: str, name: str) -> str:
    """Read an intentionally single-line top-level field, not arbitrary YAML."""
    parts = text.split("---\n", 2)
    if len(parts) != 3 or parts[0]:
        raise ValueError("SKILL.md must start with delimited YAML frontmatter")
    matches = re.findall(rf"^{re.escape(name)}: (.+)$", parts[1], re.MULTILINE)
    if len(matches) != 1:
        raise ValueError(f"expected one single-line {name} field")
    return matches[0]


class DiscoveryTests(unittest.TestCase):
    def test_json_reader_rejects_duplicate_keys(self):
        with self.assertRaises(ValueError):
            json.loads('{"name":"first","name":"second"}', object_pairs_hook=unique_object)

    def test_plugin_metadata_matches_across_ecosystems(self):
        generic = load_json("plugin.json")
        native = load_json(".claude-plugin/plugin.json")
        self.assertEqual(generic.pop("$schema"), "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json")
        self.assertEqual(native.pop("$schema"), "https://json.schemastore.org/claude-code-plugin-manifest.json")
        self.assertEqual(generic, native)
        self.assertEqual(native["name"], "blotter")
        self.assertEqual(native["repository"], REPO)
        self.assertEqual(native["license"], "MIT")
        self.assertRegex(native["version"], r"^\d+\.\d+\.\d+$")
        self.assertIsInstance(native["keywords"], list)
        self.assertTrue(all(isinstance(item, str) for item in native["keywords"]))

    def test_plugin_is_skill_only(self):
        native = load_json(".claude-plugin/plugin.json")
        self.assertEqual(set(native), {"$schema", "name", "description", "version", "author", "homepage", "repository", "license", "keywords"})
        for path in ("hooks/hooks.json", ".mcp.json", ".lsp.json"):
            self.assertFalse((ROOT / path).exists(), f"review distribution scope before adding {path}")
        self.assertTrue((ROOT / "skills/blotter/SKILL.md").is_file())
        self.assertFalse((ROOT / ".claude-plugin/skills").exists())

    def test_marketplace_resolves_to_canonical_plugin_root(self):
        marketplace = load_json(".claude-plugin/marketplace.json")
        self.assertEqual(marketplace["name"], "blotter-tools")
        self.assertEqual(marketplace["owner"], {"name": load_json("plugin.json")["author"]["name"]})
        self.assertEqual(len(marketplace["plugins"]), 1)
        entry = marketplace["plugins"][0]
        self.assertEqual(entry["name"], "blotter")
        self.assertEqual(entry["source"], "./")
        self.assertNotIn("version", entry, "plugin.json owns the plugin version")
        self.assertEqual((ROOT / entry["source"]).resolve(), ROOT)

    def test_skill_identity_description_and_size(self):
        skill = (ROOT / "skills/blotter/SKILL.md").read_text()
        self.assertEqual(frontmatter_scalar(skill, "name"), "blotter")
        self.assertEqual(frontmatter_scalar(skill, "license"), "MIT")
        self.assertTrue(1 <= len(frontmatter_scalar(skill, "description")) <= 1024)
        self.assertTrue(1 <= len(frontmatter_scalar(skill, "compatibility")) <= 500)
        self.assertLess(len(skill.splitlines()), 500)
        version = re.findall(r'^  version: "([^"]+)"$', skill.split("---\n", 2)[1], re.MULTILINE)
        self.assertEqual(version, [load_json("plugin.json")["version"]])

    def test_skill_keeps_recurrence_and_trust_boundaries(self):
        skill = (ROOT / "skills/blotter/SKILL.md").read_text()
        self.assertIn("Do not run `blotter list` as a prerequisite to filing.", skill)
        self.assertIn("Independent recurrences are evidence", skill)
        self.assertIn("Recurrence is never permission to auto-promote.", skill)
        self.assertIn("install only when package installation is authorized", skill)
        self.assertNotIn("Prefer one record that names the underlying recurrence", skill)
        self.assertNotIn("check what is already known before filing", skill)
        self.assertNotIn("--impact low|material|blocking", skill)
        self.assertIn("blotter schema", skill)

    def test_context7_indexes_usage_not_maintainer_history(self):
        config = load_json("context7.json")
        self.assertEqual(config["$schema"], "https://context7.com/schema/context7.json")
        self.assertEqual(config["folders"], ["docs", "skills/blotter"])
        for path in ("docs/plans", "docs/archive", "docs/research", "backlog", "tests"):
            self.assertIn(path, config["excludeFolders"])
        self.assertNotIn("skills", config["excludeFolders"])
        self.assertNotIn("skills/blotter", config["excludeFolders"])
        for path in ("AGENTS.md", "CLAUDE.md", "CHANGELOG.md", "discovery.md"):
            self.assertIn(path, config["excludeFiles"])
        self.assertTrue(all("/" not in name for name in config["excludeFiles"]))
        self.assertTrue(all(isinstance(rule, str) and rule for rule in config["rules"]))

    def test_llms_index_has_only_curated_raw_markdown_targets(self):
        text = (ROOT / "llms.txt").read_text()
        self.assertTrue(text.startswith("# Blotter\n\n> "))
        urls = re.findall(r"\]\((https://[^)]+)\)", text)
        self.assertEqual(urls, [RAW + path for path in (
            "README.md", "docs/reference.md", "skills/blotter/SKILL.md", "docs/discovery.md")])
        self.assertLess(len(text), 2500)

    def test_readme_links_to_installable_skill_and_guide(self):
        text = (ROOT / "README.md").read_text()
        for snippet in ("skills/blotter/SKILL.md", "docs/discovery.md", "llms.txt",
                        "npx skills add BigCactusLabs/blotter --skill blotter",
                        "/plugin install blotter@blotter-tools", "DISABLE_TELEMETRY=1"):
            self.assertIn(snippet, text)
        self.assertTrue((ROOT / "docs/discovery.md").is_file())

    def test_trigger_queries_are_balanced_and_have_holdout(self):
        data = load_json("tests/discovery/trigger-queries.json")
        self.assertEqual(data["schema_version"], 1)
        cases = data["cases"]
        self.assertEqual(len(cases), 20)
        self.assertEqual(len({case["id"] for case in cases}), 20)
        self.assertEqual(len({case["query"] for case in cases}), 20)
        for case in cases:
            self.assertIs(type(case["should_trigger"]), bool)
            self.assertIn(case["split"], ("development", "validation"))
            self.assertTrue(case["query"].strip() and case["reason"].strip())
            self.assertIn(case["expected_operation"], ("cut", "dogear", "review", "resolve", "none"))
            self.assertEqual(case["expected_operation"] == "none", not case["should_trigger"])
        for split in ("development", "validation"):
            for label in (True, False):
                self.assertEqual(sum(case["split"] == split and case["should_trigger"] is label for case in cases), 5)

    def test_isolated_cli_smoke(self):
        if BINARY is None:
            self.skipTest("pass --binary for the isolated CLI smoke; static checks only")
        self.assertTrue(BINARY.is_file(), f"binary not found: {BINARY}")
        with tempfile.TemporaryDirectory(prefix="blotter-discovery-") as tmp:
            env = {key: value for key, value in os.environ.items() if not key.startswith("BLOTTER_")}
            # --file also overrides any inherited ledger settings. No real log is read.
            ledger = str(Path(tmp) / "smoke.jsonl")
            env["BLOTTER_FILE"] = ledger

            def run(*args: str):
                result = subprocess.run(
                    [str(BINARY), "--file", ledger, *args], cwd=tmp, env=env,
                    capture_output=True, text=True, timeout=15, check=False)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stderr, "")
                value = json.loads(result.stdout)
                self.assertTrue(value["ok"])
                return value

            run("schema")
            cut = run("add", "Synthetic discovery smoke fixture", "--tag", "discovery-smoke")
            cut_id = cut["data"]["record"]["id"]
            self.assertIn(cut_id, json.dumps(run("list")))
            run("resolve", cut_id, "--disposition", "fixed")
            self.assertNotIn(cut_id, json.dumps(run("list")))
            self.assertTrue(Path(ledger).is_file())


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, help="built Blotter binary; enables isolated CLI smoke")
    args = parser.parse_args()
    BINARY = args.binary.resolve() if args.binary else None
    unittest.main(argv=[__file__], verbosity=2)
