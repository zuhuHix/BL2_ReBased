"""Synthetic checks for the cooked weapon view-manifest projection."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import weapon_view_model as view


class WeaponViewModelTests(unittest.TestCase):
    def test_type_values_override_class_defaults(self):
        merged = view.merge_type_properties(
            {'ZoomTime': 0.4, 'ZoomedEndFOV': 82, 'ZoomSocketPreferred': 'EyeSocket1'},
            {'ZoomedEndFOV': 62, 'ZoomSocketPreferred': None},
        )
        self.assertEqual(merged['ZoomTime'], 0.4)
        self.assertEqual(merged['ZoomedEndFOV'], 62)
        self.assertEqual(merged['ZoomSocketPreferred'], 'EyeSocket1')

    def test_cooked_view_projection_has_no_recipe_specific_mesh_scale(self):
        entry = view.type_view_fields('slice_smg', {}, {
            'PlayerViewOffset': {'X': 12.0, 'Y': 4.0, 'Z': 2.0},
            'FirstPersonMeshFOV': 57.0,
        })
        self.assertEqual(entry['player_view_offset'], [12.0, 4.0, 2.0])
        self.assertEqual(entry['first_person_fov'], 57.0)
        self.assertNotIn('first_person_scale_fit', entry)

    def test_ads_frame_fit_is_separate_from_the_selected_cooked_socket(self):
        self.assertEqual(view.zoom_position_fit_cm('slice_pistol'),
                         ([0.0, -3.2, 11.0], 'UNVERIFIED fit to lane E ADS frame'))
        self.assertEqual(view.zoom_position_fit_cm('slice_assault_rifle'),
                         ([0.0, 0.0, 0.0], 'no recipe-specific fit'))

    def test_reload_fallback_uses_last_unconditioned_type_animation(self):
        animations = view.reload_animation_names([
            {'AnimationName': 'Reload_Jakobs', 'Expression': None},
            {'AnimationName': 'Reload_Jakobs_Fast', 'Expression': 'SomeCondition'},
            {'AnimationName': 'Reload_Jakobs_Var2', 'Expression': None},
        ])
        self.assertEqual(view.choose_default_reload_animation(animations), 'Reload_Jakobs_Var2')

    def test_reload_uses_fast_clip_only_below_synthetic_threshold(self):
        animations = [
            {'animation_name': 'Reload_Hyperion_Fast', 'expression': 'FastReloadExpression'},
            {'animation_name': 'Reload_Hyperion', 'expression': None},
        ]
        self.assertEqual(view.choose_reload_animation(animations, 2.75, 3.0), 'Reload_Hyperion_Fast')
        self.assertEqual(view.choose_reload_animation(animations, 3.25, 3.0), 'Reload_Hyperion')
        self.assertEqual(view.choose_reload_animation(animations, 2.75, None), 'Reload_Hyperion')

    def test_fast_reload_threshold_reads_only_constant_base_value(self):
        self.assertEqual(view.fast_reload_threshold([{
            'Attribute': 'D_Attributes.Weapon.WeaponMaxFastReloadAnimTime',
            'BaseValue': {'BaseValueConstant': 3.5, 'BaseValueScaleConstant': 1.0},
        }]), 3.5)
        self.assertIsNone(view.fast_reload_threshold([{
            'Attribute': 'D_Attributes.Weapon.WeaponMaxFastReloadAnimTime',
            'BaseValue': {'BaseValueConstant': 3.5, 'BaseValueAttribute': 'Other.Attribute'},
        }]))

    def test_reload_rate_spans_item_reload_time(self):
        self.assertAlmostEqual(view.reload_play_rate(1.5, 3.0), 0.5)
        self.assertAlmostEqual(view.reload_play_rate(1.5, 0.0), 15.0)
        self.assertEqual(view.reload_play_rate(float('nan'), 2.0), 1.0)

    def test_socket_resolution_uses_selected_fragment(self):
        sockets = [
            {'SocketName': 'Pistol_Barrel_Torgue_Muzzle', 'BoneName': 'Barrel',
             'RelativeLocation': {'X': 11, 'Y': 4, 'Z': 6},
             'RelativeRotation': {'Pitch': 0, 'Yaw': 8192, 'Roll': 0}},
            {'SocketName': 'Pistol_Barrel_Vladof_Muzzle', 'BoneName': 'Barrel',
             'RelativeLocation': {'X': 97, 'Y': 8, 'Z': 1},
             'RelativeRotation': {'Pitch': 0, 'Yaw': 0, 'Roll': 0}},
        ]
        resolved = view.resolve_fragment_socket(
            sockets, ['GD_Weap_Pistol.Barrel.Pistol_Barrel_Torgue'], 'Muzzle')
        self.assertEqual(resolved['name'], 'Pistol_Barrel_Torgue_Muzzle')
        self.assertEqual(resolved['bone'], 'Barrel')
        self.assertEqual(resolved['location'], [11.0, 4.0, 6.0])
        self.assertAlmostEqual(resolved['rotation'][1], 45.0)

    def test_recoil_mapping_uses_kick_and_defaults_missing_channels_to_zero(self):
        self.assertAlmostEqual(view.recoil_animation_weight(
            {'up': 0.2, 'down': 0.4, 'left': 0.6, 'right': 0.8}), 0.225)
        self.assertAlmostEqual(view.recoil_animation_weight({'left': 0.6, 'right': 0.6}), 0.135)
        self.assertEqual(view.recoil_animation_weight({}), 0.0)

    def test_projection_keeps_cooked_zoom_reload_and_effect_data(self):
        entry = view.type_view_fields('synthetic_pistol',
            {'ZoomTime': 0.35, 'MuzzleFlashDuration': 0.4, 'ZoomSocketPreferred': 'EyeSocket1'},
            {
                'weapon_type': 'Synthetic.Type',
                'PlayerViewOffset': {'X': 12, 'Y': 4, 'Z': 2},
                'FirstPersonMeshFOV': 57,
                'ZoomedEndFOV': 52,
                'ReloadCompletePercent': 0.4,
                'WeaponReloadAnimations': [
                    {'AnimationName': 'Reload_Jakobs', 'Expression': None, 'AnimSet': 'Anim_1st_Person.Pistol'}],
                'WeaponKickUp': 0.75,
                'WeaponKickDown': 0.25,
                'WeaponKickLeft': 0.4,
                'WeaponKickRight': 0.6,
                'MuzzleFlashSocket': 'Muzzle',
                'TracerColor': {'R': 31, 'G': 63, 'B': 127, 'A': 255},
            })
        self.assertEqual(entry['zoom_fov'], 52.0)
        self.assertEqual(entry['zoom_fov_source'], 'cooked type')
        self.assertEqual(entry['zoom_blend_time'], 0.35)
        self.assertNotIn('first_person_scale_fit', entry)
        self.assertEqual(entry['zoom_position_fit_cm'], [0.0, 0.0, 0.0])
        self.assertEqual(entry['zoom_socket_preferred'], 'EyeSocket1')
        self.assertEqual(entry['reload_complete_percent'], 0.4)
        self.assertEqual(entry['reload_complete_percent_source'], 'cooked type')
        self.assertEqual(entry['reload_animation'], 'Reload_Jakobs')
        self.assertIn('UNVERIFIED selection', entry['reload_animation_source'])
        self.assertTrue(entry['weapon_reload_asset'].endswith('Anim_WeaponReload_Reload_Jakobs.Anim_WeaponReload_Reload_Jakobs'))
        self.assertEqual(entry['muzzle_socket'], 'Muzzle')
        self.assertEqual(entry['muzzle_flash_duration'], 0.4)
        self.assertEqual(entry['tracer_color'], [31, 63, 127, 255])
        self.assertAlmostEqual(entry['recoil_animation_weight'], 0.225)
        self.assertIn('UNVERIFIED', entry['recoil_animation_weight_source'])

    def test_reload_completion_percent_defaults_to_legacy_full_duration(self):
        entry = view.type_view_fields('synthetic_pistol', {}, {
            'PlayerViewOffset': {'X': 12, 'Y': 4, 'Z': 2},
        })
        self.assertEqual(entry['reload_complete_percent'], 1.0)
        self.assertEqual(entry['reload_complete_percent_source'],
                         'default: missing cooked type field; legacy full-duration refill')

    def test_live_measurements_override_cooked_type_values(self):
        markdown = '''
### slice_smg: Sample SMG
| ZoomedEndFOV | `33.5` | live read, unheld backpack weapon; synthetic fixture |
| ZoomedEndFOV | `47.0` | live read, WeaponTypeDefinition; synthetic fixture |
| PlayerViewOffset | `{"X": 12.0, "Y": 4.0, "Z": 2.0}` | live read, WeaponTypeDefinition; synthetic fixture |
| ZoomTime | `0.35` | live read, WeaponTypeDefinition; synthetic fixture |
| ReloadCompletePercent | `0.375` | live read, WeaponTypeDefinition; synthetic fixture |
| WeaponKickUp | `0.75` | live read, WeaponTypeDefinition; synthetic fixture |
| WeaponKickDown | `0.25` | live read, WeaponTypeDefinition; synthetic fixture |
| WeaponKickLeft | `0.5` | live read, WeaponTypeDefinition; synthetic fixture |
| WeaponKickRight | `0.75` | live read, WeaponTypeDefinition; synthetic fixture |
| TracerColor | `{"B": 127, "G": 63, "R": 31, "A": 255}` | live read, WeaponTypeDefinition; synthetic fixture |
| TracerTemplate | `null` | live read, WeaponTypeDefinition |
| ShellCasingPSCTemplate | `FX_WEP_Assault.Particles.Assault_Shells` | live read, WeaponTypeDefinition |
| AnimThresholdValues | `[{"Attribute":"D_Attributes.Weapon.WeaponMaxFastReloadAnimTime","BaseValue":{"BaseValueConstant":3.5,"BaseValueScaleConstant":1.0}}]` | live read, WeaponTypeDefinition; synthetic fixture |
'''
        measured = view.parse_live_measurements(markdown)['slice_smg']
        result = view.apply_live_measurement(
            {'zoom_fov': 47.0, 'weapon_kick': {'up': 0.2}, 'tracer_template': None}, measured)
        self.assertEqual(result['zoom_fov'], 33.5)
        self.assertEqual(result['zoom_fov_source'], 'live measurement: unheld weapon ZoomedEndFOV')
        self.assertEqual(result['player_view_offset'], [12.0, 4.0, 2.0])
        self.assertEqual(result['reload_complete_percent'], 0.375)
        self.assertEqual(result['reload_complete_percent_source'], 'live measurement: WeaponTypeDefinition')
        self.assertEqual(result['weapon_kick'], {'up': 0.75, 'down': 0.25, 'left': 0.5, 'right': 0.75})
        self.assertAlmostEqual(result['recoil_animation_weight'], 0.253125)
        self.assertFalse(result['tracer_enabled'])
        self.assertEqual(result['tracer_template_source'], 'live measurement: WeaponTypeDefinition')
        self.assertTrue(result['shell_casing_enabled'])
        self.assertEqual(result['shell_casing_template'], 'FX_WEP_Assault.Particles.Assault_Shells')
        self.assertEqual(result['fast_reload_threshold'], 3.5)

    def test_live_parser_ignores_type_zoom_when_item_zoom_exists(self):
        markdown = '''
### slice_pistol: Sample Pistol
| ZoomedEndFOV | `49.0` | live read, unheld backpack weapon; synthetic fixture |
| ZoomedEndFOV | `61.0` | live read, WeaponTypeDefinition; synthetic fixture |
| AnimThresholdValues | `[{"Attribute":"D_Attributes.Weapon.WeaponMaxFastReloadAnimTime","BaseValue":{"BaseValueConstant":4.0,"BaseValueScaleConstant":1.0}}]` | live read, WeaponTypeDefinition; synthetic fixture |
'''
        parsed = view.parse_live_measurements(markdown)['slice_pistol']
        self.assertEqual(parsed['zoom_fov'], 49.0)
        self.assertEqual(parsed['fast_reload_threshold'], 4.0)


if __name__ == '__main__':
    unittest.main()
