"""Synthetic tests for the lootable capture (no game or mod SDK needed): its output guard, its keeping to the safety pattern,
and its tracked files holding no placed position or object id."""
import importlib.util
from pathlib import Path
import re
import sys
import tempfile
import types
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / 'tools/real_game/scripts/loot_capture.py'
CAPTURE = ROOT / 'tools/real_game/loot_capture.ps1'


def load(rg_out):
    sdk = types.ModuleType('unrealsdk')
    sys.modules['unrealsdk'] = sdk
    spec = importlib.util.spec_from_file_location('ow_loot_capture', SCRIPT)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    module.RG_OUT = str(rg_out)   # the command channel's prelude sets this in the shared namespace
    return module


class OutputGuardTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.repo = Path(self.tmp.name)
        (self.repo / '.git').write_text('gitdir: elsewhere\n')   # a worktree's .git is a file
        self.out = self.repo / 'local/realgame/loot_capture'
        self.lc = load(self.out)

    def test_a_plain_label_lands_in_rg_out(self):
        self.assertEqual(self.lc.lc_out('materials'), (self.out / 'materials.json').resolve())

    def test_escaping_labels_are_refused(self):
        for label in ('../../../docs/x', '..', 'a/b', 'a\\b', '/abs', 'C:x', '', '.hidden', None, 3):
            with self.subTest(label=label), self.assertRaises(ValueError):
                self.lc.lc_out(label)

    def test_rg_out_outside_local_is_refused(self):
        for rg_out in (self.repo / 'docs', self.repo / 'local', self.repo, self.repo / 'local/../docs'):
            self.lc.RG_OUT = str(rg_out)
            with self.subTest(rg_out=rg_out), self.assertRaises(ValueError):
                self.lc.lc_out('materials')
        self.assertFalse((self.repo / 'docs').exists())


class CaptureScriptTest(unittest.TestCase):
    def setUp(self):
        self.text = CAPTURE.read_text(encoding='utf-8')

    def test_it_keeps_the_safety_pattern(self):
        for required in ('Backup-Saves', '-BlockSavesAtStart', 'block_saves()', 'Restore-Tree', 'Stop-OwnGame $proc $started $marker',
                         'Enter-RunLock', 'Exit-RunLock', 'Remove-Driver', 'Get-GameProcess'):
            self.assertIn(required, self.text)

    def test_cleanup_is_in_the_finally_block(self):
        cleanup = self.text[self.text.index('} finally {\n    if ($proc'):]
        for required in ('Stop-OwnGame', 'Remove-Driver', 'Restore-Tree', 'Exit-RunLock', 'Remove-Item $HubLock'):
            self.assertIn(required, cleanup)

    def test_a_failed_game_script_stops_the_step(self):
        # The command channel prints an exception as the reply instead of raising it.
        self.assertIn("-match 'Traceback (most recent call last)'".replace('(', r'\(').replace(')', r'\)'), self.text)
        self.assertIn('throw "game script failed', self.text)

    def test_it_never_kills_by_image_name(self):
        self.assertIsNone(re.search(r'taskkill|Stop-Process\s+-Name|pkill|killall', self.text, re.I))

    def test_no_key_is_sent_before_a_focus_check(self):
        # Each function or stage that sends a key checks focus first.
        for start in ('function Capture-Type', 'function Capture-Pickup'):
            body = self.text[self.text.index(start):]
            first_key = min(i for i in (body.find('Send-Key'),) if i >= 0)
            self.assertLess(body.index('Assert-Focus'), first_key)
        for line in self.text.splitlines():
            if line.lstrip().startswith(('Start-Sleep 5;', 'Assert-Focus;')) and 'Send-Key Enter' in line:
                self.assertIn('Assert-Focus', line)

    def test_the_script_saves_nothing(self):
        code = SCRIPT.read_text(encoding='utf-8')
        self.assertIsNone(re.search(r'SaveGame|SaveRawData|\bsavegame\b', code + self.text, re.I))


class NoPlacedDataTest(unittest.TestCase):
    def test_no_object_id_or_coordinate_is_tracked(self):
        for path in (SCRIPT, CAPTURE):
            text = path.read_text(encoding='utf-8')
            with self.subTest(path=path.name):
                self.assertNotIn('OpportunityPoint', text)
                self.assertNotIn('Sanctuary_Dynamic', text)
                # A world coordinate is a number of four or more digits; the window size, rotator units and date are named here.
                numbers = [n for n in re.findall(r'-?\b\d{4,}(?:\.\d+)?\b', text) if n not in ('1280', '65536', '65536.0', '2026')]
                self.assertEqual(numbers, [])


if __name__ == '__main__':
    unittest.main()
