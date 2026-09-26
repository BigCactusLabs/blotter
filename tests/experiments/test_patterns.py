"""Offline presentation checks plus opt-in real-CLI tests using synthetic ledgers."""
from __future__ import annotations

import copy
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "experiments/patterns.py"
spec = importlib.util.spec_from_file_location("patterns", SCRIPT)
patterns = importlib.util.module_from_spec(spec)
spec.loader.exec_module(patterns)
A, B, C = ("bl_" + digit * 20 for digit in "abc")


def triage():
    return {"clusters": [{"count": 2, "occurrences": 2, "ids": [A, B],
                          "tags": ["ops"], "text": "Service startup command fails"}],
            "count": 1, "scanned": 2}


def retrospect(candidates=()):
    return {"candidates": list(candidates), "count": len(candidates), "scanned": 2}


def candidate():
    return {"pattern": "recurrent_friction", "suggested": ["doc"],
            "record_ids": [B, A], "title": "Service startup command fails", "occurrences": 2}


def envelope(data, code=None, warnings=()):
    return subprocess.CompletedProcess([], int(data["count"] > 0) if code is None else code,
                                       json.dumps({"ok": True, "data": data, "meta": {"warnings": list(warnings)}}), "")


class PresentationTests(unittest.TestCase):
    def test_unknown_remedy_keeps_pattern(self):
        report = patterns.compose(triage(), retrospect())
        self.assertEqual(report["patterns"][0]["record_ids"], [A, B])
        self.assertEqual(report["patterns"][0]["suggested"], [])
        self.assertEqual(report["min_count"], 2)

    def test_exact_sources_attach_advice_independent_of_order(self):
        report = patterns.compose(triage(), retrospect([candidate()]))
        self.assertEqual(report["patterns"][0]["suggested"], ["doc"])
        self.assertEqual(report["patterns"][0]["record_ids"], [A, B])

    def test_partial_source_overlap_cannot_attach_advice(self):
        item = candidate()
        item["record_ids"] = [A, C]
        with self.assertRaisesRegex(patterns.PreviewError, "source sets disagree"):
            patterns.compose(triage(), retrospect([item]))

    def test_interventions_remain_separate(self):
        item = candidate()
        item.update(pattern="failed_intervention", suggested=["skill"], resolved_anchor_ids=[C])
        report = patterns.compose(triage(), retrospect([item]))
        self.assertEqual(report["patterns"][0]["suggested"], [])
        self.assertEqual(report["failed_interventions"][0]["resolved_anchor_ids"], [C])

    def test_input_is_not_mutated(self):
        left, right = triage(), retrospect([candidate()])
        originals = copy.deepcopy((left, right))
        patterns.compose(left, right)
        self.assertEqual((left, right), originals)

    def test_empty_is_success(self):
        report = patterns.compose({"clusters": [], "count": 0, "scanned": 0},
                                  {"candidates": [], "count": 0, "scanned": 0})
        report["warnings"] = []
        self.assertIn("No qualifying open patterns", patterns.markdown(report))

    def test_bad_collections_counts_and_sources_fail_closed(self):
        for field, value in (("count", True), ("scanned", -1), ("clusters", None),
                             ("count", 99), ("scanned", 5)):
            with self.subTest(field=field, value=value):
                bad = triage()
                bad[field] = value
                with self.assertRaises(patterns.PreviewError):
                    patterns.compose(bad, retrospect())
        for ids in ([], [A, A], ["made-up", B], [A, 1]):
            with self.subTest(ids=ids):
                bad = triage()
                bad["clusters"][0]["ids"] = ids
                with self.assertRaises(patterns.PreviewError):
                    patterns.compose(bad, retrospect())

    def test_unknown_pattern_and_duplicate_advice_fail_closed(self):
        bad = candidate()
        bad["pattern"] = "future-pattern"
        for items in ([bad], [candidate(), candidate()]):
            with self.subTest(items=items), self.assertRaises(patterns.PreviewError):
                patterns.compose(triage(), retrospect(items))

    def test_markdown_treats_ledger_text_as_data(self):
        report = patterns.compose(triage(), retrospect())
        report["warnings"] = ["<script>bad</script>"]
        report["patterns"][0]["title"] = "[click](https://example.com)\n# heading\x1b[2J"
        rendered = patterns.markdown(report)
        self.assertNotIn("<script>", rendered)
        self.assertNotIn("[click](", rendered)
        self.assertNotIn("\n# heading", rendered)
        self.assertNotIn("\x1b", rendered)
        self.assertIn("Unknown", rendered)
        self.assertIn(A, rendered)

    def test_reads_only_two_explicit_commands_and_retains_warnings(self):
        with tempfile.TemporaryDirectory() as tmp:
            ledger = Path(tmp) / "ledger.jsonl"
            ledger.write_text("synthetic fixture\n")
            with patch.dict(os.environ, {"BLOTTER_FILE": "/wrong", "BLOTTER_NOW": "invalid"}), patch.object(
                    patterns.subprocess, "run", side_effect=[envelope(triage(), warnings=["skipped malformed line"]),
                                                             envelope(retrospect())]) as run:
                report = patterns.preview("/bin/blotter", ledger)
            self.assertEqual(report["warnings"], ["skipped malformed line"])
            commands = [call.args[0] for call in run.call_args_list]
            self.assertEqual(commands, [["/bin/blotter", "--file", str(ledger), "triage", "--min-count", "2"],
                                        ["/bin/blotter", "--file", str(ledger), "retrospect"]])
            for call in run.call_args_list:
                self.assertFalse(any(key.startswith("BLOTTER_") for key in call.kwargs["env"]))
                self.assertNotIn("shell", call.kwargs)
            self.assertEqual(ledger.read_text(), "synthetic fixture\n")

    def test_changed_file_refuses_mixed_review(self):
        with tempfile.TemporaryDirectory() as tmp:
            ledger = Path(tmp) / "ledger.jsonl"
            ledger.write_text("synthetic\n")
            def read(_binary, file, command):
                if command[0] == "retrospect":
                    file.write_text("synthetic changed fixture\n")
                return (triage() if command[0] == "triage" else retrospect()), []
            with patch.object(patterns, "read_analysis", side_effect=read), self.assertRaisesRegex(
                    patterns.PreviewError, "Ledger changed"):
                patterns.preview("blotter", ledger)

    def test_missing_file_does_not_invoke_binary_or_create_log(self):
        with tempfile.TemporaryDirectory() as tmp:
            ledger = Path(tmp) / "absent.jsonl"
            with patch.object(patterns, "read_analysis") as run, self.assertRaises(FileNotFoundError):
                patterns.preview("blotter", ledger)
            run.assert_not_called()
            self.assertFalse(ledger.exists())

    def test_reader_failures_are_not_empty_success(self):
        cases = [subprocess.CompletedProcess([], 75, "", "locked"),
                 subprocess.CompletedProcess([], 2, "", "invalid"),
                 subprocess.CompletedProcess([], 0, "not json", ""),
                 subprocess.CompletedProcess([], 1, '{"ok":false}', ""),
                 envelope(triage(), code=0),
                 subprocess.CompletedProcess([], 1, envelope(triage()).stdout, "unexpected")]
        for result in cases:
            with self.subTest(result=result), patch.object(patterns.subprocess, "run", return_value=result):
                with self.assertRaises(patterns.PreviewError) as caught:
                    patterns.read_analysis("blotter", Path("ledger"), ["triage", "--min-count", "2"])
                self.assertEqual(caught.exception.exit_code, 75 if result.returncode == 75 else 2)


class BinaryTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        binary = os.environ.get("BLOTTER_TEST_BINARY")
        if not binary:
            raise unittest.SkipTest("set BLOTTER_TEST_BINARY for real-CLI integration tests")
        cls.binary = str(Path(binary).resolve(strict=True))  # A bad CI path fails, not skips.

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.ledger = self.root / "synthetic.jsonl"
        self.trap = self.root / ".blotter.jsonl"
        self.trap.write_bytes(b"unrelated sentinel: must never be opened or changed\n")
        self.env = {k: v for k, v in os.environ.items() if not k.startswith(("BLOTTER_", "GIT_"))}
        self.env.update(HOME=str(self.root), USERPROFILE=str(self.root), BLOTTER_FILE=str(self.trap),
                        BLOTTER_AGENT="synthetic-fixture")
        self.minute = 0

    def cli(self, *args):
        self.minute += 1
        env = dict(self.env, BLOTTER_NOW=f"2026-09-26T12:{self.minute:02d}:00Z")
        result = subprocess.run([self.binary, "--file", str(self.ledger), *args], cwd=self.root,
                                env=env, capture_output=True, text=True, timeout=30)
        self.assertIn(result.returncode, (0, 1), result.stderr)
        self.assertEqual(result.stderr, "")
        return json.loads(result.stdout)["data"]

    def render(self, format="json"):
        before = self.ledger.read_bytes()
        result = subprocess.run([sys.executable, str(SCRIPT), "--binary", self.binary,
                                 "--file", str(self.ledger), "--format", format], cwd=self.root,
                                env=self.env, capture_output=True, text=True, timeout=70)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stderr, "")
        self.assertEqual(self.ledger.read_bytes(), before)
        self.assertEqual(self.trap.read_bytes(), b"unrelated sentinel: must never be opened or changed\n")
        return json.loads(result.stdout) if format == "json" else result.stdout

    def test_unmatched_pattern_visible_without_changing_retrospect(self):
        for text in ("Service startup command fails", "service startup command fails again"):
            self.cli("add", text, "--tag", "ops")
        stable = self.cli("retrospect")
        self.assertEqual(stable["candidates"], [])
        report = self.render()
        self.assertEqual(len(report["patterns"]), 1)
        self.assertEqual(report["patterns"][0]["suggested"], [])
        self.assertEqual(report["patterns"][0]["record_count"], 2)
        self.assertEqual(self.cli("retrospect"), stable)
        self.assertEqual(self.render(), report)
        self.assertIn("Unknown", self.render("md"))

    def test_docs_advice_survives_exact_source_join(self):
        for text in ("Deployment documentation is unclear", "deployment documentation is unclear again"):
            self.cli("add", text, "--tag", "docs")
        self.assertEqual(self.render()["patterns"][0]["suggested"], ["doc"])

    def test_unrelated_observations_do_not_form_a_pattern(self):
        self.cli("add", "Zebra import truncates Unicode identifiers", "--tag", "parser")
        self.cli("add", "Release credentials expire during upload", "--tag", "deploy")
        report = self.render()
        self.assertEqual(report["patterns"], [])
        self.assertEqual(report["failed_interventions"], [])

    def test_intervention_remains_evidence_not_automatic_action(self):
        anchor = self.cli("add", "Cache recovery guide failed", "--tag", "ops")["record"]["id"]
        self.cli("resolve", anchor, "--disposition", "fixed", "--note", "Synthetic repair")
        for text in ("cache recovery guide failed again", "cache recovery guide failed twice"):
            self.cli("add", text, "--tag", "ops", "--cmd", "cargo recover-cache")
        report = self.render()
        self.assertEqual(report["failed_interventions"][0]["resolved_anchor_ids"], [anchor])
        self.assertEqual(len(report["patterns"]), 1)
        self.assertEqual(self.cli("list", "--kind", "promotion", "--status", "all")["items"], [])
        self.assertIn("do not add their counts together", self.render("md"))


if __name__ == "__main__":
    unittest.main()
