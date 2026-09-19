"""Offline fixture tests; the workflow separately builds the actual repository."""
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from urllib.parse import urlsplit
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("site_builder", ROOT / "scripts/dev/build-discovery-site.py")
SITE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SITE)
SHA = "a" * 40


class SiteTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name) / "repo"
        self.root.mkdir()
        self.output = Path(self.tmp.name) / "output"
        self.write("site/pages.json", (ROOT / "site/pages.json").read_text())
        self.write("site/site.css", "body { margin: 0; }\n")
        self.write("Cargo.toml", '[package]\nversion = "1.1.1"\n')
        for source in SITE.SOURCES:
            self.write(source, "# Blotter\n\nHello.\n\n## A heading\n\nA paragraph.\n")
        self.write("skills/blotter/SKILL.md", "---\nname: blotter\ndescription: Record meaningful friction.\n---\n\n# Blotter\n\n## Use\n\nRun `blotter schema`.\n")

    def write(self, path, text):
        destination = self.root / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(text, encoding="utf-8")

    def build(self, base=SITE.DEFAULT_URL):
        return SITE.build(self.root, self.output, base, SHA)

    def test_allowlist_and_no_ledger_publication(self):
        self.write(".blotter.jsonl", "PRIVATE_SENTINEL")
        self.write("docs/research/private.md", "PRIVATE_SENTINEL")
        self.write("AGENTS.md", "PRIVATE_SENTINEL")
        result = self.build()
        self.assertEqual(result["page_count"], 5)
        self.assertFalse(result["deployment_tested"])
        for path in self.output.rglob("*"):
            if path.is_file():
                self.assertNotIn(b"PRIVATE_SENTINEL", path.read_bytes())
        self.assertEqual(len(list(self.output.rglob("*.html"))), 5)

    def test_canonical_metadata_and_markdown_alternate(self):
        self.build()
        text = (self.output / "workflows/index.html").read_text()
        self.assertIn(f'href="{SITE.DEFAULT_URL}workflows/"', text)
        self.assertIn(f'{SITE.RAW}/{SHA}/docs/agent-workflows.md', text)
        self.assertIn('type="text/markdown"', text)
        self.assertIn('name="description"', text)

    def test_html_and_javascript_not_executed_and_images_not_fetched(self):
        self.write("README.md", '# Title\n\n<script>alert(1)</script>\n\n[evil](javascript:alert(1))\n\n![pixel](https://tracker.example/pixel.png)\n')
        self.build()
        text = (self.output / "index.html").read_text()
        self.assertNotIn('<script>alert', text)
        self.assertNotIn('href="javascript:', text)
        self.assertNotIn('<img', text)
        self.assertNotIn('src="https://tracker.example', text)
        self.assertEqual(text.count('<script'), 1)  # JSON-LD data only.

    def test_heading_ids_are_unique_prefixed_and_links_rewritten(self):
        self.write("README.md", '# Main\n\n## Test\n\n[local](#test)\n\n## Test\n\n[repeat](#test-1)\n\n## Test-1\n\n[ref](docs/reference.md#a-heading)\n')
        self.build()
        text = (self.output / "index.html").read_text()
        for expected in ('id="doc-main"', 'id="doc-test"', 'id="doc-test-1"', 'id="doc-test-1-1"', 'href="#doc-test"', 'reference/#doc-a-heading'):
            self.assertIn(expected, text)

    def test_nonconsumer_links_use_pinned_github_source(self):
        self.write("README.md", "# Start\n\n[Maintainers](AGENTS.md)\n\n[Index](llms.txt)\n")
        self.build()
        text = (self.output / "index.html").read_text()
        self.assertIn(f'{SITE.REPO}/blob/{SHA}/AGENTS.md', text)
        self.assertIn(f'{SITE.DEFAULT_URL}llms.txt', text)

    def test_fragment_typo_fails_before_output(self):
        self.write("README.md", "# Start\n\n[wrong](docs/reference.md#does-not-exist)\n")
        with self.assertRaisesRegex(ValueError, "broken anchor"):
            self.build()
        self.assertFalse(self.output.exists())

    def test_link_escape_rejected(self):
        for link in ("../outside", "%2e%2e/outside", "//elsewhere.test/page", "/absolute"):
            with self.subTest(link=link), self.assertRaises(ValueError):
                SITE.resolve_link(link, "README.md", {}, SITE.DEFAULT_URL, SHA)

    def test_skill_digest_is_exact_served_bytes(self):
        self.build()
        index = json.loads((self.output / "agent-skills.json").read_text())
        entry = index["skills"][0]
        raw = (self.root / "skills/blotter/SKILL.md").read_bytes()
        self.assertEqual(index["$schema"], SITE.SCHEMA)
        self.assertEqual(entry["digest"], "sha256:" + hashlib.sha256(raw).hexdigest())
        self.assertEqual(entry["description"], "Record meaningful friction.")
        self.assertEqual((self.output / "skills/blotter/SKILL.md").read_bytes(), raw)
        self.assertEqual(entry["type"], "skill-md")

    def test_project_path_does_not_claim_root_discovery_or_robots(self):
        result = self.build()
        self.assertEqual(result["skill_discovery_mode"], "explicit-index-only")
        self.assertFalse((self.output / ".well-known").exists())
        self.assertFalse((self.output / "robots.txt").exists())

    def test_root_hosted_build_has_well_known_and_robots(self):
        result = self.build("https://docs.example.org")
        self.assertEqual(result["skill_discovery_mode"], "origin-root-draft")
        self.assertEqual((self.output / "agent-skills.json").read_bytes(), (self.output / ".well-known/agent-skills/index.json").read_bytes())
        self.assertIn('Sitemap: https://docs.example.org/sitemap.xml', (self.output / "robots.txt").read_text())

    def test_output_refuses_existing_directories_and_symlinks(self):
        self.output.mkdir()
        sentinel = self.output / "keep.txt"
        sentinel.write_text("keep")
        with self.assertRaisesRegex(ValueError, "refusing to overwrite"):
            self.build()
        self.assertEqual(sentinel.read_text(), "keep")
        sentinel.unlink()
        self.output.rmdir()
        self.output.symlink_to(Path(self.tmp.name) / "missing")
        with self.assertRaisesRegex(ValueError, "refusing to overwrite"):
            self.build()

    def test_symlinked_inputs_fail(self):
        path = self.root / "docs/reference.md"
        path.unlink()
        path.symlink_to(self.root / "README.md")
        with self.assertRaisesRegex(ValueError, "symlink"):
            self.build()

    def test_invalid_and_duplicate_routes_rejected(self):
        original = json.loads((self.root / "site/pages.json").read_text())
        for route in ("../escape/", "reference/", "foo/index.html", "//evil/"):
            config = json.loads(json.dumps(original))
            config[1]["route"] = route
            self.write("site/pages.json", json.dumps(config))
            with self.subTest(route=route), self.assertRaisesRegex(ValueError, "route"):
                self.build()

    def test_nonallowlisted_source_rejected(self):
        config = json.loads((self.root / "site/pages.json").read_text())
        config.append({"source": ".blotter.jsonl", "route": "ledger/"})
        self.write("site/pages.json", json.dumps(config))
        with self.assertRaisesRegex(ValueError, "allowlist"):
            self.build()

    def test_base_url_and_commit_rejected(self):
        for url in ("http://example.org/", "https://user:pass@example.org/", "https://example.org/a/../b", "https://example.org/%2e", "https://example.org/?q=x", "https://example.org/#hash", "https://example.org:443/", "https://example.org//bad"):
            with self.subTest(url=url), self.assertRaises(ValueError):
                SITE.base_url(url)
        with self.assertRaisesRegex(ValueError, "commit SHA"):
            SITE.build(self.root, self.output, SITE.DEFAULT_URL, "main")

    def test_skill_resources_require_explicit_packaging_review(self):
        self.write("skills/blotter/references/new.md", "# Extra")
        with self.assertRaisesRegex(ValueError, "single-file"):
            self.build()

    def test_frontmatter_is_not_rendered(self):
        self.build()
        text = (self.output / "skill/index.html").read_text()
        self.assertNotIn("name: blotter", text)
        self.assertIn("doc-use", text)

    def test_json_sections_and_sitemap_match_pages(self):
        self.build()
        data = json.loads((self.output / "docs.json").read_text())
        self.assertEqual(data["source_commit"], SHA)
        self.assertEqual(len(data["documents"]), 5)
        for doc in data["documents"]:
            self.assertIn(SHA, doc["markdown_url"])
            self.assertTrue(doc["sections"])
            self.assertEqual(len(doc["source_sha256"]), 64)
        xml = ET.parse(self.output / "sitemap.xml")
        urls = [node.text for node in xml.iter("{http://www.sitemaps.org/schemas/sitemap/0.9}loc")]
        self.assertEqual(urls, [doc["url"] for doc in data["documents"]])

    def test_build_is_reproducible_and_manifest_hashes_match(self):
        result = self.build()
        other = Path(self.tmp.name) / "another"
        self.assertEqual(result, SITE.build(self.root, other, SITE.DEFAULT_URL, SHA))
        for path, metadata in result["files"].items():
            content = (self.output / path).read_bytes()
            self.assertEqual(content, (other / path).read_bytes())
            self.assertEqual(metadata["bytes"], len(content))
            self.assertEqual(metadata["sha256"], hashlib.sha256(content).hexdigest())


if __name__ == "__main__":
    unittest.main()
