"""Synthetic checks for the per-type reload and arms clip plan (invented names, no game data)."""
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import weapon_reload_library as library


class ReloadPlanTests(unittest.TestCase):
    TYPES = {
        'WT.A': {'weapon_anim_set': 'Anim.RocketLauncher', 'socket_table': 'Pkg.Mesh_Rocket', 'reload_animation': 'Reload_B'},
        'WT.B': {'weapon_anim_set': 'Anim.RocketLauncher', 'socket_table': 'Pkg.Mesh_Rocket', 'reload_animation': 'Reload_A'},
        'WT.C': {'weapon_anim_set': 'Anim.Pistol', 'socket_table': 'Pkg.Mesh_Pistol', 'reload_animation': None},
        'WT.D': {'socket_table': 'Pkg.Mesh_None', 'reload_animation': 'Reload_C'},
    }

    def test_clips_group_by_set_with_the_library_folder_and_mesh(self):
        plan = library.reload_plan(self.TYPES)
        self.assertEqual(list(plan), ['Pistol', 'RocketLauncher'])
        self.assertEqual(plan['RocketLauncher'], {'kind': 'Launcher', 'mesh': 'Mesh_Rocket', 'clips': ['Reload_A', 'Reload_B']})
        self.assertEqual(plan['Pistol'], {'kind': 'Pistol', 'mesh': 'Mesh_Pistol', 'clips': []})

    def test_a_set_of_its_own_gets_the_base_clips_and_the_pistol_only_its_reloads(self):
        self.assertEqual(library.arms_clips('Pistol', ['Reload_A']), ['Reload_A'])
        clips = library.arms_clips('Shotgun', ['Reload_A'])
        self.assertIn('Idle', clips)
        self.assertEqual(clips[-1], 'Reload_A')

    def test_a_clip_not_exported_is_named_and_not_converted(self):
        with tempfile.TemporaryDirectory() as folder:
            (Path(folder) / 'Reload_A.md5anim').write_text('', encoding='utf-8')
            present, missing = library.found_clips(folder, ['Reload_A', 'Reload_B'])
        self.assertEqual(list(present), ['Reload_A'])
        self.assertEqual(missing, ['Reload_B'])


if __name__ == '__main__':
    unittest.main()
