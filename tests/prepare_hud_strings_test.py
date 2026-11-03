"""Synthetic tests for tools/prepare_hud_strings.py: the output guard and the strings it writes.

The localization file is written here (UTF-16, as the install's) with invented values ("Toy down", "Toy wind", ...);
nothing comes from the game. Outputs that must be accepted go to a fresh folder under this repository's local/, removed
afterwards.
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TOOL = ROOT / 'tools' / 'prepare_hud_strings.py'
INT = """[WillowHUDGFxMovie]
PlayerInjuredString=Toy down
PlayerRevivingString=Toy reviving
SecondWindString=Toy wind
InjuredTooltipString1=Toy hint
[HUDWidget_Missions]
MissionTurnInString=Toy turn in
"""


def run(game, output):
    return subprocess.run([sys.executable, str(TOOL), '--game', str(game), '--output', str(output)],
                          capture_output=True, text=True)


class GuardTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory()
        game = Path(cls.temp.name) / 'game'
        folder = game / 'WillowGame' / 'Localization' / 'INT'
        folder.mkdir(parents=True)
        (folder / 'WillowGame.int').write_text(INT, encoding='utf-16')
        cls.game = game
        cls.inside = ROOT / 'local' / f'_test_prepare_hud_strings_{os.getpid()}'

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()
        shutil.rmtree(cls.inside, ignore_errors=True)

    def test_output_outside_the_repository_is_refused(self):
        outside = Path(self.temp.name) / 'out'
        result = run(self.game, outside)
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertIn('refusing', result.stderr)
        self.assertFalse(outside.exists())

    def test_output_in_the_repository_but_not_local_is_refused(self):
        result = run(self.game, ROOT / 'docs' / '_never_written')
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertFalse((ROOT / 'docs' / '_never_written').exists())

    def test_a_path_that_climbs_out_of_local_is_refused(self):
        result = run(self.game, ROOT / 'local' / '..' / 'tools' / '_never_written')
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertFalse((ROOT / 'tools' / '_never_written').exists())

    def test_output_under_local_is_written(self):
        result = run(self.game, self.inside)
        self.assertEqual(result.returncode, 0, result.stderr)
        written = json.loads((self.inside / 'UI_HUD' / 'hud_strings.json').read_text(encoding='utf-8'))
        self.assertEqual(written, {'injured': 'Toy down', 'reviving': 'Toy reviving', 'secondWind': 'Toy wind',
                                   'injuredTip': 'Toy hint', 'turnIn': 'Toy turn in'})


if __name__ == '__main__':
    unittest.main()
