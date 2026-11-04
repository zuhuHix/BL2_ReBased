"""Synthetic tests for tools/prepare_map_screen.py: token resolution and the local/ output guard (no game data)."""
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / 'tools'))
import prepare_map_screen as pms  # noqa: E402


class ResolveTest(unittest.TestCase):
    def test_nested_aliases_and_strings(self):
        aliases = {'bar': '<Strings:Menu.sec.Text>', 'Close': '[Esc]'}
        strings = {'menu.sec.text': '<StringAliasMap:Close> Leave\nnow'}
        self.assertEqual(pms.resolve('<StringAliasMap:bar>', strings, aliases), '[Esc] Leave\nnow')

    def test_unknown_tokens_are_kept(self):
        self.assertEqual(pms.resolve('<StringAliasMap:nope> <Strings:A.b.C>', {}, {}), '<StringAliasMap:nope> <Strings:A.b.C>')

    def test_a_loop_stops(self):
        aliases = {'a': '<StringAliasMap:b>', 'b': '<StringAliasMap:a>'}
        self.assertIn('StringAliasMap', pms.resolve('<StringAliasMap:a>', {}, aliases))


class OutputTest(unittest.TestCase):
    def test_output_must_be_under_local(self):
        with tempfile.TemporaryDirectory() as repo:
            (Path(repo) / 'local').mkdir()
            self.assertEqual(pms.output_file(Path(repo) / 'local/ui/run', repo), (Path(repo) / 'local/ui/run/map.json').resolve())
            with self.assertRaises(SystemExit):
                pms.output_file(Path(repo) / 'docs', repo)


if __name__ == '__main__':
    unittest.main()
