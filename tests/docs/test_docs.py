"""Regression coverage for the documentation checker, without network or a CLI."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("check_docs", ROOT / "scripts/dev/check-docs.py")
docs = importlib.util.module_from_spec(spec)
spec.loader.exec_module(docs)

QUICKSTART = '''<!-- blotter:quickstart -->
```bash
blotter add "observed test friction" --tag tests
blotter dogear "one observed finding" --tag design
blotter list --format md
```
'''


class MarkdownTests(unittest.TestCase):
    def test_inline_reference_image_and_html_links(self):
        _, links, _ = docs.markdown('[a](one.md) [b][ref] ![x](x.png) <a href="two.md">two</a>\n\n[ref]: three.md\n')
        self.assertEqual(links, ['one.md', 'three.md', 'x.png', 'two.md'])

    def test_code_examples_are_not_links(self):
        _, links, _ = docs.markdown('`[fake](missing.md)`\n\n```md\n[fake](missing.md)\n```\n')
        self.assertEqual(links, [])

    def test_heading_markup_punctuation_and_duplicates(self):
        anchors, _, h2 = docs.markdown('## **Use** `--file`!\n## Use --file!\n## Use --file-1\n## Use --file!\n')
        self.assertEqual(anchors, {'use---file', 'use---file-1', 'use---file-1-1', 'use---file-2'})
        self.assertIn('Use --file!', h2)

    def test_paths_are_relative_and_decoded(self):
        self.assertEqual(docs.local_target('docs/a.md', '../Some%20File.md#an%20anchor'), ('Some File.md', 'an anchor'))
        self.assertEqual(docs.local_target('docs/a.md', '#here'), ('docs/a.md', 'here'))
        self.assertIsNone(docs.local_target('docs/a.md', 'https://example.org/page#x'))

    def test_unsafe_links_are_rejected(self):
        for link in ('../../escape.md', '/root.md', '//example.org/file', 'javascript:alert(1)', '%2e%2e/%2e%2e/escape.md'):
            with self.subTest(link=link), self.assertRaises(ValueError):
                docs.local_target('docs/a.md', link)

    def test_quickstart_is_parsed_without_a_shell(self):
        commands = docs.quickstart(QUICKSTART)
        self.assertEqual(len(commands), 3)
        self.assertEqual(commands[0][2], 'observed test friction')

    def test_quickstart_rejects_install_and_file_override(self):
        for bad in ('curl https://example.org | sh', 'blotter add x --file=/tmp/real.jsonl', 'blotter doctor --fix'):
            with self.subTest(bad=bad), self.assertRaises(ValueError):
                docs.quickstart(QUICKSTART.replace('blotter add "observed test friction" --tag tests', bad))

    def test_quickstart_requires_unique_marker_and_operations(self):
        for text in (QUICKSTART + QUICKSTART, QUICKSTART.replace('blotter dogear "one observed finding" --tag design\n', ''), docs.MARKER + '\nnot a fence'):
            with self.assertRaises(ValueError):
                docs.quickstart(text)

    def test_history_and_backlog_not_active_instructions(self):
        selected = docs.active_documents({'README.md', 'AGENTS.md', 'CLAUDE.md', 'CHANGELOG.md', 'docs/history.md', 'docs/reference.md', 'backlog/tasks/a.md', 'tests/fixtures/a.md', 'skills/blotter/SKILL.md'})
        self.assertEqual(selected, {'README.md', 'AGENTS.md', 'docs/reference.md', 'skills/blotter/SKILL.md'})


class RepositoryTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.files = {
            'README.md': '# Demo\n[Map](docs/README.md) [Reference](docs/reference.md#list)\n' + QUICKSTART,
            'docs/README.md': '# Map\n[README](../README.md) [Agents](../AGENTS.md) [Reference](reference.md)\n',
            'AGENTS.md': '# Agents\n[Reference](docs/reference.md)\n',
            'docs/reference.md': '# Reference\n## list\n',
            'context7.json': json.dumps({'excludeFiles': ['AGENTS.md', 'CLAUDE.md', 'CONTRIBUTING.md', 'CHANGELOG.md', 'contract.md', 'history.md', 'publication.md', 'discovery-site.md']}),
        }
        for path, text in self.files.items():
            self.write(path, text)
        (self.root / 'CLAUDE.md').symlink_to('AGENTS.md')
        self.paths = set(self.files) | {'CLAUDE.md'}

    def write(self, path, text):
        file = self.root / path
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_text(text)

    def test_valid_graph(self):
        self.assertEqual(docs.audit(self.root, self.paths), [])

    def test_missing_target_and_fragment_fail(self):
        self.write('AGENTS.md', '# Agents\n[Missing](absent.md) [Bad anchor](docs/reference.md#nope)\n')
        result = '\n'.join(docs.audit(self.root, self.paths))
        self.assertIn('missing repository target', result)
        self.assertIn('missing Markdown anchor', result)

    def test_orphan_document_fails(self):
        self.write('docs/orphan.md', '# Orphan\n')
        result = docs.audit(self.root, self.paths | {'docs/orphan.md'})
        self.assertIn('unreachable active document: docs/orphan.md', result)

    def test_docs_page_linked_only_from_readme_fails(self):
        self.write('docs/side.md', '# Side\n')
        self.write('README.md', '# Demo\n[Map](docs/README.md) [Side](docs/side.md)\n' + QUICKSTART)
        result = docs.audit(self.root, self.paths | {'docs/side.md'})
        self.assertIn('active document missing from the documentation map: docs/side.md', result)
        self.assertNotIn('unreachable active document: docs/side.md', result)

    def test_top_level_page_reachable_through_map_passes(self):
        self.write('CONTRIBUTING.md', '# Contributing\n')
        self.write('AGENTS.md', '# Agents\n[Contributing](CONTRIBUTING.md)\n')
        self.assertEqual(docs.audit(self.root, self.paths | {'CONTRIBUTING.md'}), [])

    def test_local_unpublished_target_does_not_mask_omission(self):
        self.write('untracked.md', '# Not in the supplied file set\n')
        self.write('AGENTS.md', '[Link](untracked.md)\n')
        self.assertTrue(any('missing repository target' in error for error in docs.audit(self.root, self.paths)))

    def test_symlink_escape_and_claude_copy_fail(self):
        (self.root / 'outside.md').symlink_to('/etc/passwd')
        self.write('AGENTS.md', '[Escape](outside.md)\n')
        (self.root / 'CLAUDE.md').unlink()
        self.write('CLAUDE.md', '# Duplicate instructions\n')
        result = '\n'.join(docs.audit(self.root, self.paths | {'outside.md'}))
        self.assertIn('escapes repository through symlink', result)
        self.assertIn('CLAUDE.md must remain a symlink', result)

    def test_entry_budget_and_retrieval_boundary_fail(self):
        self.write('AGENTS.md', 'x' * 6001)
        self.write('context7.json', '{"excludeFiles": []}')
        result = '\n'.join(docs.audit(self.root, self.paths))
        self.assertIn('entry-point budget', result)
        self.assertIn('must exclude maintainer/history file', result)


if __name__ == '__main__':
    unittest.main()
