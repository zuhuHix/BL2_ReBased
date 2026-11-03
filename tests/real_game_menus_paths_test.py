"""Synthetic test for tools/real_game/scripts/menus_probe.py's output guard (no game or mod SDK needed).

The probe runs inside the game's mod SDK; here it is loaded with stand-in `unrealsdk` modules so `_out`, `mp_write_json` and
`mp_trace_start` can be checked: what the probe writes is game data and must stay under RG_OUT, which must be inside the
repository's ignored local/ (review 1 of PR #64).
"""
import importlib.util
import json
import sys
import tempfile
import types
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / 'tools' / 'real_game' / 'scripts' / 'menus_probe.py'


def load(rg_out):
    sdk = types.ModuleType('unrealsdk')
    hooks = types.ModuleType('unrealsdk.hooks')
    hooks.Type = types.SimpleNamespace(PRE='pre', POST='post')
    hooks.add_hook = hooks.remove_hook = lambda *args: None
    unreal = types.ModuleType('unrealsdk.unreal')
    unreal.UObject = unreal.WrappedArray = unreal.WrappedStruct = type('Stand-in', (), {})
    sdk.hooks, sdk.unreal = hooks, unreal
    sys.modules.update({'unrealsdk': sdk, 'unrealsdk.hooks': hooks, 'unrealsdk.unreal': unreal})
    spec = importlib.util.spec_from_file_location('ow_menus_probe', SCRIPT)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    module.RG_OUT = str(rg_out)   # the command channel's prelude sets this in the shared namespace
    return module


class MenusProbeOutTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.repo = Path(self.tmp.name)
        (self.repo / '.git').write_text('gitdir: elsewhere\n')   # a worktree's .git is a file
        self.out = self.repo / 'local' / 'realgame' / 'menus'
        self.mp = load(self.out)

    def tearDown(self):
        self.tmp.cleanup()

    def test_plain_names_land_under_rg_out(self):
        self.assertEqual(self.mp._out('rows.json'), (self.out / 'rows.json').resolve())
        self.assertEqual(self.mp._out('traces/t1.jsonl'), (self.out / 'traces' / 't1.jsonl').resolve())
        written = Path(self.mp.mp_write_json('probe.json', {'a': 1}))
        self.assertEqual(json.loads(written.read_text(encoding='utf-8')), {'a': 1})

    def test_escaping_names_are_refused(self):
        for name in ('../../../docs/x.json', '..', 'traces/../../x', 'a\\b', '/abs', 'C:x', '', '.hidden', 'a..b',
                     'traces//x', None, 3):
            with self.assertRaises(ValueError, msg=repr(name)):
                self.mp._out(name)
            with self.assertRaises(ValueError, msg=repr(name)):
                self.mp.mp_write_json(name, {})
        self.assertFalse((self.repo / 'docs').exists())

    def test_trace_start_refuses_before_opening(self):
        for name in ('../escape', 'a/../../b', None):
            with self.assertRaises(ValueError, msg=repr(name)):
                self.mp.mp_trace_start(name)
        self.assertFalse(any(self.repo.rglob('escape*')))
        self.assertIsNone(self.mp.MP['file'])

    def test_rg_out_outside_local_is_refused(self):
        for rg_out in (self.repo / 'docs' / 'verification', self.repo / 'local' / '..' / 'docs', self.repo / 'local',
                       self.repo, Path(self.tmp.name).parent / 'local' / 'x'):
            self.mp.RG_OUT = str(rg_out)
            with self.assertRaises(ValueError, msg=str(rg_out)):
                self.mp._out('probe.json')
        self.assertFalse((self.repo / 'docs').exists())


if __name__ == '__main__':
    unittest.main()
