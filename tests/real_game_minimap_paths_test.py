"""Synthetic test for tools/real_game/scripts/minimap.py's output guard (no game or mod SDK needed).

The capture script runs inside the game's mod SDK; here it is loaded with stand-in `unrealsdk` modules so `mm_out` can
be checked: what a capture writes is game data and must stay directly under RG_OUT (review 1 of PR #53).
"""
import importlib.util
import sys
import tempfile
import types
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / 'tools' / 'real_game' / 'scripts' / 'minimap.py'


def load(rg_out):
    sdk = types.ModuleType('unrealsdk')
    hooks = types.ModuleType('unrealsdk.hooks')
    hooks.Type = types.SimpleNamespace(PRE='pre', POST='post')
    hooks.add_hook = hooks.remove_hook = lambda *args: None
    sdk.hooks = hooks
    sys.modules['unrealsdk'], sys.modules['unrealsdk.hooks'] = sdk, hooks
    spec = importlib.util.spec_from_file_location('ow_minimap_capture', SCRIPT)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    module.RG_OUT = str(rg_out)   # the command channel's prelude sets this in the shared namespace
    return module


class MmOutTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.repo = Path(self.tmp.name)
        (self.repo / '.git').write_text('gitdir: elsewhere'+chr(10))   # a worktree's .git is a file
        self.out = self.repo / 'local' / 'realgame' / 'minimap'
        self.mm = load(self.out)

    def tearDown(self):
        self.tmp.cleanup()

    def test_plain_labels_land_in_rg_out(self):
        self.assertEqual(self.mm.mm_out('spot0_yaw90'), (self.out / 'spot0_yaw90.json').resolve())
        self.assertEqual(self.mm.mm_out('turn_rec', '.jsonl'), (self.out / 'turn_rec.jsonl').resolve())
        self.assertTrue(self.out.is_dir())

    def test_escaping_labels_are_refused(self):
        for label in ('../../../docs/x', '..', 'a/b', 'a\\b', '/abs', 'C:x', '', '.hidden', 'a..b', None, 3):
            with self.assertRaises(ValueError, msg=repr(label)):
                self.mm.mm_out(label)
        self.assertFalse((Path(self.tmp.name) / 'docs').exists())

    def test_rg_out_outside_local_is_refused(self):
        # Review 2: realgame.ps1 builds RG_OUT from -OutFolder, e.g. '..\..\docs\verification'.
        for rg_out in (self.repo / 'docs' / 'verification', self.repo / 'local' / '..' / 'docs',
                       self.repo / 'local', self.repo, Path(self.tmp.name).parent / 'local' / 'x'):
            self.mm.RG_OUT = str(rg_out)
            with self.assertRaises(ValueError, msg=str(rg_out)):
                self.mm.mm_out('probe1')
        self.assertFalse((self.repo / 'docs').exists())

    def test_local_without_a_repository_is_refused(self):
        bare = Path(self.tmp.name) / 'bare'
        self.mm.RG_OUT = str(bare / 'local' / 'realgame')
        with self.assertRaises(ValueError):
            self.mm.mm_out('probe1')

    def test_main_checkout_git_folder_is_accepted(self):
        main = Path(self.tmp.name) / 'main'
        (main / '.git').mkdir(parents=True)
        self.mm.RG_OUT = str(main / 'local' / 'realgame' / 'minimap')
        self.assertEqual(self.mm.mm_out('probe1'), (main / 'local' / 'realgame' / 'minimap' / 'probe1.json').resolve())

    def test_capture_functions_use_the_guard(self):
        # mm_start opens its file through mm_out: a bad label fails before any hook is added or file is written.
        with self.assertRaises(ValueError):
            self.mm.mm_start('../escape')
        self.assertFalse(any(Path(self.tmp.name).rglob('escape*')))


if __name__ == '__main__':
    unittest.main()
