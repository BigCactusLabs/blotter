"""Offline tests of demonstration assertions; real CLI evidence comes from CI."""
import importlib.util
import os
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch

PATH = Path(__file__).resolve().parents[2] / "scripts/dev/demo-discovery-proof.py"
spec = importlib.util.spec_from_file_location("proof", PATH)
proof = importlib.util.module_from_spec(spec)
spec.loader.exec_module(proof)


class ProofTests(unittest.TestCase):
    def result(self, code=0, out=None, err=""):
        return subprocess.CompletedProcess([], code, '{"ok":true,"data":{}}' if out is None else out, err)

    def test_successful_envelope(self):
        self.assertEqual(proof.decode_result(self.result(), 0), {})

    def test_findings_exit_is_expected_not_generic_failure(self):
        self.assertEqual(proof.decode_result(self.result(code=1), 1), {})
        with self.assertRaises(RuntimeError):
            proof.decode_result(self.result(code=1), 0)

    def test_errors_and_bad_shapes_are_not_success(self):
        for out in ('[]', '{"ok":false,"data":{}}', '{"ok":1,"data":{}}', '{"ok":true,"data":[]}'):
            with self.subTest(out=out), self.assertRaises(RuntimeError):
                proof.decode_result(self.result(out=out), 0)
        with self.assertRaises(ValueError):
            proof.decode_result(self.result(out="not JSON"), 0)
        with self.assertRaises(RuntimeError):
            proof.decode_result(self.result(err="unexpected"), 0)

    def test_process_environment_does_not_inherit_destinations_or_credentials(self):
        with patch.dict(os.environ, {"PATH": "/bin", "HOME": "/real", "BLOTTER_FILE": "/real/log",
                                    "BLOTTER_NOW": "invalid", "GITHUB_TOKEN": "test-token",
                                    "ANTHROPIC_API_KEY": "test-key"}, clear=True):
            env = proof.isolated_env(Path("/disposable/home"))
        self.assertEqual(env["PATH"], "/bin")
        self.assertEqual(env["HOME"], "/disposable/home")
        for key in ("BLOTTER_FILE", "BLOTTER_NOW", "GITHUB_TOKEN", "ANTHROPIC_API_KEY"):
            self.assertNotIn(key, env)

    def test_report_does_not_claim_independent_corroboration(self):
        source = PATH.read_text()
        self.assertIn('"synthetic_records_before_fix":', source)
        self.assertNotIn('"independent_observations_before_fix":', source)
        self.assertIn('"independent_corroboration_tested": False', source)
        self.assertIn('"causal_effect_tested": False', source)

    def snapshot(self):
        return {"count": 1, "distinct_recurring_cuts": 2, "recurrences": [
            {"resolved_id": "anchor", "recurrence_ids": ["later-a", "later-b"],
             "resolution": {"disposition_ts": "original-boundary"}}]}

    def test_verification_counts_anchors_and_distinct_occurrences_separately(self):
        data = self.snapshot()
        proof.verify_snapshot(data, "anchor", ["later-b", "later-a"], "original-boundary")
        data["count"] = 2
        with self.assertRaises(RuntimeError):
            proof.verify_snapshot(data, "anchor", ["later-a", "later-b"], "original-boundary")

    def test_shifted_boundary_and_pre_fix_matches_fail(self):
        for key, value in (("recurrence_ids", ["earlier", "later-b"]),
                           ("resolution", {"disposition_ts": "amend-time"})):
            data = self.snapshot()
            data["recurrences"][0][key] = value
            with self.subTest(key=key), self.assertRaises(RuntimeError):
                proof.verify_snapshot(data, "anchor", ["later-a", "later-b"], "original-boundary")


if __name__ == "__main__":
    unittest.main()
