"""Offline tests: no provider requests, credentials, or host installations."""
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock
from urllib.error import HTTPError, URLError

ROOT = Path(__file__).resolve().parents[2]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, ROOT / path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


context = load("context7_publication", "scripts/dev/publish-discovery-context7.py")
remote = load("remote_delivery", "scripts/dev/check-discovery-remote.py")
SOURCE = "https://github.com/BigCactusLabs/blotter/blob/main/README.md"
SNIPPETS = {"codeSnippets": [], "infoSnippets": [{"pageId": SOURCE, "content": "blotter schema"}]}


class PublicationTests(unittest.TestCase):
    def test_plan_needs_no_credentials_or_network(self):
        result = context.execute("plan", None)
        self.assertFalse(result["network_performed"])
        self.assertEqual(result["submit"]["body"], {"docsRepoUrl": context.REPOSITORY, "private": False})

    def test_missing_and_invalid_credentials_fail_before_io(self):
        for key in ("", " ", "fake\nkey"):
            with self.assertRaises(context.PublicationError):
                context.Client(key)

    def test_submission_is_acceptance_not_indexing(self):
        client = Mock()
        client.request.return_value = {"libraryName": "/BigCactusLabs/blotter", "message": "queued"}
        result = context.execute("submit", client)
        self.assertEqual(result["status"], "submission_accepted")
        self.assertFalse(result["index_verified"])
        client.request.assert_called_once()

    def test_wrong_identity_is_not_accepted(self):
        client = Mock()
        client.request.return_value = {"libraryName": "/other/blotter"}
        with self.assertRaises(context.PublicationError):
            context.execute("submit", client)

    def test_unrelated_search_result_is_not_target(self):
        client = Mock()
        client.request.return_value = {"results": [{"id": "/other/blotter", "state": "finalized"}]}
        self.assertEqual(context.execute("verify", client)["status"], "not_returned")
        client.request.assert_called_once()

    def test_processing_does_not_issue_context_requests(self):
        client = Mock()
        client.request.return_value = {"results": [{"id": context.LIBRARY, "state": "processing"}]}
        result = context.execute("verify", client)
        self.assertEqual(result["status"], "not_ready")
        self.assertFalse(result["index_verified"])
        client.request.assert_called_once()

    def test_retrieval_checks_three_topics_with_source_provenance(self):
        client = Mock()
        client.request.side_effect = [{"results": [{"id": context.LIBRARY, "state": "finalized"}]}] + [SNIPPETS] * 3
        result = context.execute("verify", client)
        self.assertEqual(result["status"], "retrieval_available")
        self.assertTrue(result["quality_review_required"])
        self.assertEqual(len(result["retrieval_checks"]), 3)
        self.assertEqual(client.request.call_count, 4)

    def test_empty_snippets_do_not_count_as_retrieval(self):
        client = Mock()
        client.request.side_effect = [{"results": [{"id": context.LIBRARY, "state": "finalized"}]}] + [{"codeSnippets": [], "infoSnippets": []}] * 3
        self.assertEqual(context.execute("verify", client)["status"], "retrieval_incomplete")

    def test_source_boundary_rejects_lookalikes_and_bad_urls(self):
        for url in ("https://github.com/other/blotter/blob/main/x", SOURCE.replace("blotter/", "blotter-other/"),
                    "https://github.com.evil.test/BigCactusLabs/blotter/blob/main/x",
                    "https://user:pass@github.com/BigCactusLabs/blotter/blob/main/x",
                    "http://github.com/BigCactusLabs/blotter/blob/main/x", "https://[bad", "https://github.com:bad/x"):
            self.assertFalse(context.own_source(url), url)
        self.assertTrue(context.own_source(SOURCE))
        self.assertTrue(context.own_source("https://raw.githubusercontent.com/BigCactusLabs/blotter/main/README.md"))

    def test_http_errors_are_not_index_success_and_do_not_leak_body(self):
        for code in (301, 401, 403, 409, 429, 500):
            client = context.Client("synthetic-key")
            client.opener = Mock()
            client.opener.open.side_effect = HTTPError(context.BASE, code, "failure", {}, io.BytesIO(b"synthetic-key"))
            with self.assertRaises(context.PublicationError) as caught:
                client.request("/v2/add/repo/github", {})
            self.assertEqual(caught.exception.http_status, code)
            self.assertNotIn("synthetic-key", str(caught.exception) + json.dumps(client.observations))

    def test_transport_error_is_not_absence(self):
        client = context.Client("synthetic-key")
        client.opener = Mock()
        client.opener.open.side_effect = URLError("synthetic-key")
        with self.assertRaises(context.PublicationError) as caught:
            client.request("/v2/libs/search?q=x")
        self.assertEqual(caught.exception.status, "transport_unavailable")

    def test_json_size_shape_and_html_fail_closed(self):
        for body in (b"[]", b"<html>challenge</html>", b"x" * (context.MAX_BYTES + 1)):
            client = context.Client("synthetic-key")
            response = Mock(status=200)
            response.read.return_value = body
            client.opener = Mock()
            client.opener.open.return_value.__enter__ = Mock(return_value=response)
            client.opener.open.return_value.__exit__ = Mock(return_value=False)
            with self.assertRaises(context.PublicationError):
                client.request("/v2/add/repo/github", {})

    def test_redirect_handler_never_forwards_auth(self):
        self.assertIsNone(context.NoRedirects().redirect_request(None, None, 302, "", {}, "https://other.test"))

    def test_remote_inputs_are_data_not_shell(self):
        sha = "a" * 40
        sources = remote.delivery_sources("discovery/remote-delivery", sha)
        self.assertTrue(sources[0].endswith("/tree/" + sha))
        self.assertTrue(sources[1].endswith("#discovery/remote-delivery"))
        for ref in ("", "--help", "bad ref", "../main", "refs//main", "main.lock", "main\n"):
            with self.assertRaises(ValueError):
                remote.delivery_sources(ref, sha)
        with self.assertRaises(ValueError):
            remote.delivery_sources("main", "short")

    def test_remote_bytes_and_symlink_boundary(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            inside = root / "inside"
            inside.mkdir()
            outside = root / "outside.md"
            outside.write_bytes(b"expected")
            skill = inside / "SKILL.md"
            skill.write_bytes(b"expected")
            self.assertEqual(len(remote.check_skill(skill, inside, b"expected")), 64)
            with self.assertRaises(ValueError):
                remote.check_skill(skill, inside, b"wrong")
            skill.unlink()
            skill.symlink_to(outside)
            with self.assertRaises(ValueError):
                remote.check_skill(skill, inside, b"expected")


if __name__ == "__main__":
    unittest.main()
