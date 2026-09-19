"""Offline regression tests; never query a real catalog or install a package."""
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from urllib.error import HTTPError, URLError

ROOT = Path(__file__).resolve().parents[2]


def load(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts/dev" / (name + ".py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


audit = load("audit-discovery")
hosts = load("check-discovery-hosts")
TARGET = {"id": audit.TARGET, "source": "BigCactusLabs/blotter", "name": "blotter", "installs": 1}


class DistributionTests(unittest.TestCase):
    def test_empty_success_means_not_returned(self):
        self.assertEqual(audit.classify({"skills": []})["status"], "not_returned")

    def test_preserves_api_and_client_ranks(self):
        other = {"id": "other/tool/friction", "source": "other/tool", "name": "friction", "installs": 100}
        result = audit.classify({"skills": [TARGET, other]})
        self.assertEqual((result["api_rank"], result["cli_rank"]), (1, 2))
        self.assertEqual(result["status"], "found")

    def test_identity_is_case_insensitive_but_not_name_only(self):
        row = TARGET | {"id": audit.TARGET.upper()}
        self.assertEqual(audit.classify({"skills": [row]})["status"], "found")
        row = TARGET | {"id": "other/blotter/blotter", "source": "other/blotter"}
        self.assertEqual(audit.classify({"skills": [row]})["status"], "not_returned")

    def test_malformed_shapes_are_not_absence(self):
        for value in ([], {}, {"skills": None}, {"skills": [None]}, {"skills": [{}]}, {"skills": [TARGET | {"installs": True}]}):
            with self.subTest(value=value), self.assertRaises(ValueError):
                audit.classify(value)

    def test_http_failures_are_not_absence(self):
        for status in (401, 403, 404, 429, 503):
            error = HTTPError("https://skills.sh/api/search", status, "error", {}, None)
            with self.subTest(status=status), patch.object(audit, "urlopen", side_effect=error):
                result = audit.probe("friction")
                self.assertEqual(result["status"], "unavailable")
                self.assertEqual(result["http_status"], status)

    def test_transport_failure_is_not_absence(self):
        with patch.object(audit, "urlopen", side_effect=URLError("offline")):
            self.assertEqual(audit.probe("friction")["status"], "unavailable")

    def test_response_is_bounded_and_html_is_malformed(self):
        for body in (b"<html>challenge</html>", b"x" * (audit.MAX_BYTES + 1)):
            response = io.BytesIO(body)
            response.status = 200
            with patch.object(audit, "urlopen", return_value=response):
                self.assertEqual(audit.probe("friction")["status"], "malformed")

    def test_success_includes_evidence_hash(self):
        response = io.BytesIO(json.dumps({"skills": [TARGET]}).encode())
        response.status = 200
        with patch.object(audit, "urlopen", return_value=response):
            result = audit.probe("blotter")
            self.assertEqual(result["status"], "found")
            self.assertEqual(len(result["response_sha256"]), 64)

    def test_isolation_drops_credentials_and_host_overrides(self):
        with tempfile.TemporaryDirectory() as tmp, patch.dict(os.environ, {
            "ANTHROPIC_API_KEY": "not-a-real-key", "GITHUB_TOKEN": "fake", "SKILLS_API_URL": "https://example.invalid"}):
            env = hosts.isolated_env(Path(tmp))
            for name in ("ANTHROPIC_API_KEY", "GITHUB_TOKEN", "SKILLS_API_URL"):
                self.assertNotIn(name, env)
            self.assertEqual(env["HOME"], tmp)
            self.assertEqual(env["DISABLE_TELEMETRY"], "1")

    def test_native_listing_shape(self):
        self.assertEqual(hosts.plugin_rows([]), [])
        self.assertEqual(hosts.plugin_rows({"installed": []}), [])
        for value in ("not a listing", {}, {"installed": None}):
            with self.subTest(value=value), self.assertRaises(ValueError):
                hosts.plugin_rows(value)


if __name__ == "__main__":
    unittest.main()
