"""Synthetic checks for grouping selected reload clips onto first-person arms sets."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import weapon_reload_anims as reloads


class WeaponReloadAnimTests(unittest.TestCase):
    def test_selected_normal_and_fast_clips_group_by_arms_anim_set(self):
        view = {
            'slice_pistol': {'weapon_anim_set': 'Anim_1st_Person.Pistol',
                             'reload_animation': 'Reload_Jakobs'},
            'slice_mission_pistol_fire': {'weapon_anim_set': 'Anim_1st_Person.Pistol',
                                          'reload_animation': 'Reload_Maliwan'},
            'slice_assault_rifle': {'weapon_anim_set': 'Anim_1st_Person.AssaultRifle',
                                    'reload_animation': 'Reload_Jakobs'},
            'slice_smg': {'weapon_anim_set': 'Anim_1st_Person.SMG',
                          'reload_animation': 'Reload_Hyperion_Fast'},
        }

        self.assertEqual(reloads.arms_reload_groups(view), {
            'AssaultRifle': ('Reload_Jakobs',),
            'Pistol': ('Reload_Jakobs', 'Reload_Maliwan'),
            'SMG': ('Reload_Hyperion_Fast',),
        })

    def test_missing_clip_fails_instead_of_selecting_another_animation(self):
        with self.assertRaisesRegex(ValueError, 'missing or unsupported'):
            reloads.arms_reload_groups({'slice_smg': {
                'weapon_anim_set': 'Anim_1st_Person.SMG', 'reload_animation': None,
            }}, ('slice_smg',))


if __name__ == '__main__':
    unittest.main()
