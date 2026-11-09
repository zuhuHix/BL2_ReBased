"""The four MD5 importers give UE a clip of N keys N - 1 frame intervals (docs/verification/CONTROLS.md, "Clip length").

That is UE's sample spacing for bone-track keys; the game's keys / rate is bridged in the host (OpenWillowClipTime.h), not here.
The importers run inside the UE editor, so this reads their source. Nothing game-derived is read.
"""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[1]
IMPORTERS = {'host/ue5/import_character_anims.py': 'data', 'host/ue5/import_weapon_reload_anims.py': 'data',
             'tools/slice_npc_editor.py': 'clip', 'tools/lootables_editor.py': 'clip'}
CALL = re.compile(r'set_number_of_frames\((.*)\)\s*$', re.M)


class FrameCountRuleTest(unittest.TestCase):
    def test_each_importer_sets_keys_minus_one_once(self):
        for path, name in IMPORTERS.items():
            with self.subTest(path=path):
                calls = CALL.findall((ROOT / path).read_text(encoding='utf-8'))
                self.assertEqual(calls, [f"unreal.FrameNumber(max(1, {name}['frames'] - 1))"])


if __name__ == '__main__':
    unittest.main()
