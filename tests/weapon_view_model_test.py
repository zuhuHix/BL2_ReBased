"""Synthetic checks for the cooked weapon view-manifest projection."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import weapon_view_model as view


class WeaponViewModelTests(unittest.TestCase):
    def test_type_values_override_class_defaults(self):
        merged = view.merge_type_properties(
            {'ZoomTime': 0.2, 'ZoomedEndFOV': 70, 'ZoomSocketPreferred': 'EyeSocket1'},
            {'ZoomedEndFOV': 50, 'ZoomSocketPreferred': None},
        )
        self.assertEqual(merged['ZoomTime'], 0.2)
        self.assertEqual(merged['ZoomedEndFOV'], 50)
        self.assertEqual(merged['ZoomSocketPreferred'], 'EyeSocket1')

    def test_reload_fallback_uses_last_unconditioned_type_animation(self):
        animations = view.reload_animation_names([
            {'AnimationName': 'Reload_Jakobs', 'Expression': None},
            {'AnimationName': 'Reload_Jakobs_Fast', 'Expression': 'SomeCondition'},
            {'AnimationName': 'Reload_Jakobs_Var2', 'Expression': None},
        ])
        self.assertEqual(view.choose_default_reload_animation(animations), 'Reload_Jakobs_Var2')

    def test_reload_uses_fast_clip_only_below_measured_threshold(self):
        animations = [
            {'animation_name': 'Reload_Hyperion_Fast', 'expression': 'FastReloadExpression'},
            {'animation_name': 'Reload_Hyperion', 'expression': None},
        ]
        self.assertEqual(view.choose_reload_animation(animations, 2.0769, 2.5), 'Reload_Hyperion_Fast')
        self.assertEqual(view.choose_reload_animation(animations, 2.7, 2.5), 'Reload_Hyperion')
        self.assertEqual(view.choose_reload_animation(animations, 2.0769, None), 'Reload_Hyperion')

    def test_fast_reload_threshold_reads_only_constant_base_value(self):
        self.assertEqual(view.fast_reload_threshold([{
            'Attribute': 'D_Attributes.Weapon.WeaponMaxFastReloadAnimTime',
            'BaseValue': {'BaseValueConstant': 2.5, 'BaseValueScaleConstant': 1.0},
        }]), 2.5)
        self.assertIsNone(view.fast_reload_threshold([{
            'Attribute': 'D_Attributes.Weapon.WeaponMaxFastReloadAnimTime',
            'BaseValue': {'BaseValueConstant': 2.5, 'BaseValueAttribute': 'Other.Attribute'},
        }]))

    def test_reload_rate_spans_item_reload_time(self):
        self.assertAlmostEqual(view.reload_play_rate(1.5, 3.0), 0.5)
        self.assertAlmostEqual(view.reload_play_rate(1.5, 0.0), 15.0)
        self.assertEqual(view.reload_play_rate(float('nan'), 2.0), 1.0)

    def test_socket_resolution_uses_selected_fragment(self):
        sockets = [
            {'SocketName': 'Pistol_Barrel_Torgue_Muzzle', 'BoneName': 'Barrel',
             'RelativeLocation': {'X': 10, 'Y': 2, 'Z': 3},
             'RelativeRotation': {'Pitch': 0, 'Yaw': 16384, 'Roll': 0}},
            {'SocketName': 'Pistol_Barrel_Vladof_Muzzle', 'BoneName': 'Barrel',
             'RelativeLocation': {'X': 99, 'Y': 0, 'Z': 0},
             'RelativeRotation': {'Pitch': 0, 'Yaw': 0, 'Roll': 0}},
        ]
        resolved = view.resolve_fragment_socket(
            sockets, ['GD_Weap_Pistol.Barrel.Pistol_Barrel_Torgue'], 'Muzzle')
        self.assertEqual(resolved['name'], 'Pistol_Barrel_Torgue_Muzzle')
        self.assertEqual(resolved['bone'], 'Barrel')
        self.assertEqual(resolved['location'], [10.0, 2.0, 3.0])
        self.assertAlmostEqual(resolved['rotation'][1], 90.0)

    def test_recoil_mapping_uses_kick_and_defaults_missing_channels_to_zero(self):
        self.assertAlmostEqual(view.recoil_animation_weight(
            {'up': 0.0, 'down': 0.1, 'left': 1.0, 'right': 1.0}), 0.23625)
        self.assertAlmostEqual(view.recoil_animation_weight({'left': 0.5, 'right': 0.5}), 0.1125)
        self.assertEqual(view.recoil_animation_weight({}), 0.0)

    def test_projection_keeps_cooked_zoom_reload_and_effect_data(self):
        entry = view.type_view_fields('synthetic_pistol',
            {'ZoomTime': 0.2, 'MuzzleFlashDuration': 0.33, 'ZoomSocketPreferred': 'EyeSocket1'},
            {
                'weapon_type': 'Synthetic.Type',
                'PlayerViewOffset': {'X': 15, 'Y': 4, 'Z': 2},
                'FirstPersonMeshFOV': 45,
                'ZoomedEndFOV': 50,
                'WeaponReloadAnimations': [
                    {'AnimationName': 'Reload_Jakobs', 'Expression': None, 'AnimSet': 'Anim_1st_Person.Pistol'}],
                'WeaponKickLeft': 0.5,
                'WeaponKickRight': 0.5,
                'MuzzleFlashSocket': 'Muzzle',
                'TracerColor': {'R': 10, 'G': 20, 'B': 30, 'A': 255},
            })
        self.assertEqual(entry['zoom_fov'], 50.0)
        self.assertEqual(entry['zoom_fov_source'], 'cooked type')
        self.assertEqual(entry['zoom_blend_time'], 0.2)
        self.assertEqual(entry['zoom_socket_preferred'], 'EyeSocket1')
        self.assertEqual(entry['reload_animation'], 'Reload_Jakobs')
        self.assertIn('UNVERIFIED selection', entry['reload_animation_source'])
        self.assertTrue(entry['weapon_reload_asset'].endswith('Anim_WeaponReload_Reload_Jakobs.Anim_WeaponReload_Reload_Jakobs'))
        self.assertEqual(entry['muzzle_socket'], 'Muzzle')
        self.assertEqual(entry['muzzle_flash_duration'], 0.33)
        self.assertEqual(entry['tracer_color'], [10, 20, 30, 255])
        self.assertAlmostEqual(entry['recoil_animation_weight'], 0.1125)
        self.assertIn('UNVERIFIED', entry['recoil_animation_weight_source'])

    def test_live_measurements_override_cooked_type_values(self):
        markdown = '''
### slice_smg: Sample SMG
| ZoomedEndFOV | `25.280899047851562` | live read, unheld backpack weapon |
| ZoomedEndFOV | `45.0` | live read, WeaponTypeDefinition |
| PlayerViewOffset | `{"X": 10.0, "Y": 3.0, "Z": 0.0}` | live read, WeaponTypeDefinition |
| ZoomTime | `0.20000000298023224` | live read, WeaponTypeDefinition |
| WeaponKickUp | `0.5` | live read, WeaponTypeDefinition |
| WeaponKickDown | `0.2` | live read, WeaponTypeDefinition |
| WeaponKickLeft | `0.5` | live read, WeaponTypeDefinition |
| WeaponKickRight | `1.0` | live read, WeaponTypeDefinition |
| TracerColor | `{"B": 200, "G": 200, "R": 200, "A": 255}` | live read, WeaponTypeDefinition |
| TracerTemplate | `null` | live read, WeaponTypeDefinition |
| ShellCasingPSCTemplate | `FX_WEP_Assault.Particles.Assault_Shells` | live read, WeaponTypeDefinition |
| AnimThresholdValues | `[{"Attribute":"D_Attributes.Weapon.WeaponMaxFastReloadAnimTime","BaseValue":{"BaseValueConstant":2.5,"BaseValueScaleConstant":1.0}}]` | live read, WeaponTypeDefinition |
'''
        measured = view.parse_live_measurements(markdown)['slice_smg']
        result = view.apply_live_measurement(
            {'zoom_fov': 45, 'weapon_kick': {'up': 0.1}, 'tracer_template': None}, measured)
        self.assertAlmostEqual(result['zoom_fov'], 25.280899047851562)
        self.assertEqual(result['zoom_fov_source'], 'live measurement: unheld weapon ZoomedEndFOV')
        self.assertEqual(result['player_view_offset'], [10.0, 3.0, 0.0])
        self.assertEqual(result['weapon_kick'], {'up': 0.5, 'down': 0.2, 'left': 0.5, 'right': 1.0})
        self.assertAlmostEqual(result['recoil_animation_weight'], 0.2475)
        self.assertFalse(result['tracer_enabled'])
        self.assertEqual(result['tracer_template_source'], 'live measurement: WeaponTypeDefinition')
        self.assertTrue(result['shell_casing_enabled'])
        self.assertEqual(result['shell_casing_template'], 'FX_WEP_Assault.Particles.Assault_Shells')
        self.assertEqual(result['fast_reload_threshold'], 2.5)

    def test_live_parser_ignores_type_zoom_when_item_zoom_exists(self):
        markdown = '''
### slice_pistol: Sample Pistol
| ZoomedEndFOV | `50.0` | live read, unheld backpack weapon |
| ZoomedEndFOV | `60.0` | live read, WeaponTypeDefinition |
| AnimThresholdValues | `[{"Attribute":"D_Attributes.Weapon.WeaponMaxFastReloadAnimTime","BaseValue":{"BaseValueConstant":1.5,"BaseValueScaleConstant":1.0}}]` | live read, WeaponTypeDefinition |
'''
        parsed = view.parse_live_measurements(markdown)['slice_pistol']
        self.assertEqual(parsed['zoom_fov'], 50.0)
        self.assertEqual(parsed['fast_reload_threshold'], 1.5)


if __name__ == '__main__':
    unittest.main()
