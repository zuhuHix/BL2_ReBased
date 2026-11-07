"""Synthetic checks for the cooked weapon view-manifest projection."""
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import weapon_view_model as view


class ViewTableTests(unittest.TestCase):
    TABLES = {
        'types': {'WT.Gun': {'weapon_type': 'WT.Gun', 'first_person_fov': 40.0, 'muzzle_socket': 'Muzzle',
                             'shell_casing_socket': 'Port', 'zoom_socket_preferred': 'Eye1',
                             'zoom_socket_fallback': 'Eye2', 'socket_table': 'Mesh'}},
        'sockets': {'Mesh': {'barrel_a_muzzle': {'name': 'Barrel_A_Muzzle', 'bone': 'B', 'fragment': ''},
                             'port': {'name': 'Port', 'bone': 'R', 'fragment': ''},
                             'scope_a_eye2': {'name': 'Scope_A_Eye2', 'bone': 'S', 'fragment': ''}}},
        'parts': {'P.Barrel': {'fragments': ['Barrel_A'], 'scope_overlay': False},
                  'P.Sight': {'fragments': ['Scope_A'], 'scope_overlay': True}},
    }

    def test_sockets_follow_the_parts_and_the_table_key_stays_out(self):
        entry = view.compose_entry(self.TABLES, 'WT.Gun', {'Barrel': 'P.Barrel', 'Sight': 'P.Sight', 'Stock': 'P.Unknown'})
        self.assertEqual(entry['first_person_fov'], 40.0)
        self.assertNotIn('socket_table', entry)
        self.assertEqual(entry['muzzle_socket_transform']['fragment'], 'Barrel_A')
        self.assertEqual(entry['shell_casing_socket_transform']['name'], 'Port')
        self.assertEqual(entry['zoom_socket_transform']['name'], 'Scope_A_Eye2')
        self.assertTrue(entry['scope_overlay'])

    def test_iron_sights_raise_the_gun_by_the_gap_to_the_front_sight(self):
        tables = {
            'types': {'WT.Gun': {'weapon_type': 'WT.Gun', 'zoom_socket_preferred': 'Eye1', 'zoom_socket_fallback': 'Eye2',
                                 'zoom_position_fit_cm': [0.0, 1.0, 0.0], 'socket_table': 'Mesh', 'zoom_sight_line': True},
                      'WT.Plain': {'weapon_type': 'WT.Plain', 'zoom_socket_preferred': 'Eye1', 'zoom_socket_fallback': 'Eye2',
                                   'zoom_position_fit_cm': [0.0, 1.0, 0.0], 'socket_table': 'Mesh', 'zoom_sight_line': False}},
            'sockets': {'Mesh': {'body_a_eye2': {'name': 'Body_A_Eye2', 'bone': 'R', 'location': [0.0, 9.0, 19.0]},
                                 'barrel_a_frontsight': {'name': 'Barrel_A_FrontSight', 'bone': 'R', 'location': [0.0, -9.0, 13.0]},
                                 'barrel_b_frontsight': {'name': 'Barrel_B_FrontSight', 'bone': 'Q', 'location': [0.0, -9.0, 13.0]},
                                 'scope_a_eye1': {'name': 'Scope_A_Eye1', 'bone': 'R', 'location': [0.0, 9.0, 15.0]}}},
            'parts': {'P.BarrelA': {'fragments': ['Barrel_A']}, 'P.BarrelB': {'fragments': ['Barrel_B']},
                      'P.Body': {'fragments': ['Body_A']}, 'P.Sight': {'fragments': ['Scope_A']}},
        }

        def fit(**slots):
            return view.compose_entry(tables, 'WT.Gun', {k: f'P.{v}' for k, v in slots.items()})['zoom_position_fit_cm']
        self.assertEqual(fit(Body='Body', Barrel='BarrelA'), [0.0, 1.0, 6.0])
        plain = view.compose_entry(tables, 'WT.Plain', {'Body': 'P.Body', 'Barrel': 'P.BarrelA'})
        self.assertEqual(plain['zoom_position_fit_cm'], [0.0, 1.0, 0.0], 'a class outside the fit stays on the eye socket')
        self.assertEqual(plain['zoom_socket_transform']['name'], 'Body_A_Eye2')
        self.assertEqual(fit(Body='Body', Barrel='BarrelA', Sight='Sight'), [0.0, 1.0, 0.0], 'a sight part has its own eye socket')
        self.assertEqual(fit(Body='Body', Barrel='BarrelB'), [0.0, 1.0, 0.0], 'different bones are not compared')
        self.assertEqual(fit(Body='Body', Barrel='Unknown'), [0.0, 1.0, 0.0], 'a barrel outside the table has no front sight')
        self.assertEqual(tables['types']['WT.Gun']['zoom_position_fit_cm'], [0.0, 1.0, 0.0], 'the type table is not changed')

    def test_unresolved_socket_and_unknown_type(self):
        entry = view.compose_entry(self.TABLES, 'WT.Gun', {})
        self.assertIsNone(entry['muzzle_socket_transform'])
        self.assertEqual(entry['muzzle_socket_transform_source'], 'unresolved')
        self.assertFalse(entry['scope_overlay'])
        self.assertIsNone(view.compose_entry(self.TABLES, 'WT.Other', {}))

    def test_a_part_outside_the_table_gets_no_plain_socket_but_an_absent_one_does(self):
        tables = dict(self.TABLES, sockets={'Mesh': dict(self.TABLES['sockets']['Mesh'],
                                                         muzzle={'name': 'Muzzle', 'bone': 'M', 'fragment': ''})})
        self.assertEqual(view.compose_entry(tables, 'WT.Gun', {})['muzzle_socket_transform']['name'], 'Muzzle')
        unknown = view.compose_entry(tables, 'WT.Gun', {'Barrel': 'P.Unknown'})
        self.assertIsNone(unknown['muzzle_socket_transform'])
        self.assertEqual(unknown['muzzle_socket_transform_source'], 'unresolved')
        self.assertEqual(view.compose_entry(tables, 'WT.Gun', {'Barrel': 'P.Barrel'})['muzzle_socket_transform']['fragment'], 'Barrel_A')
        self.assertIsNone(view.compose_entry(tables, 'WT.Gun', {'Body': 'P.Unknown'})['shell_casing_socket_transform'])

    def test_a_sight_outside_the_table_gets_no_plain_zoom_but_an_empty_slot_does(self):
        tables = dict(self.TABLES, sockets={'Mesh': dict(self.TABLES['sockets']['Mesh'],
                                                         eye2={'name': 'Eye2', 'bone': 'E', 'fragment': ''})})
        self.assertEqual(view.compose_entry(tables, 'WT.Gun', {})['zoom_socket_transform']['name'], 'Eye2')
        self.assertIsNone(view.compose_entry(tables, 'WT.Gun', {'Sight': 'P.Unknown'})['zoom_socket_transform'])
        self.assertEqual(view.compose_entry(tables, 'WT.Gun', {'Sight': 'P.Sight'})['zoom_socket_transform']['name'], 'Scope_A_Eye2')

    def check_folder(self, recipes, listed):
        with tempfile.TemporaryDirectory() as name:
            folder = Path(name)
            for recipe_id, recipe in recipes.items():
                (folder / f'{recipe_id}.json').write_text(json.dumps(recipe), encoding='utf-8')
            (folder / 'weapon_view.json').write_text(json.dumps(listed), encoding='utf-8')
            return view.check_folder(self.TABLES, folder)

    def test_check_counts_recipes_and_sees_a_missing_line_or_recipe(self):
        recipe = {'weapon_type': 'WT.Gun', 'parts': {'Barrel': {'part': 'P.Barrel'}}}
        entry = view.compose_entry(self.TABLES, 'WT.Gun', {'Barrel': 'P.Barrel'})
        self.assertEqual(self.check_folder({'gun': recipe, 'sample_index': {'guns': []}}, {'gun': entry}), (1, []))
        self.assertEqual(self.check_folder({}, {}), (0, []))
        total, problems = self.check_folder({'gun': recipe}, {})
        self.assertEqual(total, 1)
        self.assertIn('no line in weapon_view.json', problems[0])
        total, problems = self.check_folder({}, {'gone': entry})
        self.assertEqual(total, 1)
        self.assertIn('no recipe in the folder', problems[0])

    def test_oracle_names_a_difference_and_ignores_the_gun_only_fields(self):
        recipe = {'parts': {'Barrel': {'part': 'P.Barrel'}}}
        same = view.compose_entry(self.TABLES, 'WT.Gun', {'Barrel': 'P.Barrel'})
        listed = dict(same, card_reload_time=2.0, weapon_reload_asset='anywhere')
        self.assertEqual(view.oracle_differences(self.TABLES, recipe, listed), [])
        self.assertEqual(view.oracle_differences(self.TABLES, recipe, dict(listed, first_person_fov=50.0)), ['first_person_fov'])


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
        self.assertEqual(view.zoom_position_fit_cm('GD_Weap_Pistol.A_Weapons.WT'),
                         ([0.0, 1.1, 0.0], 'UNVERIFIED fit to lane E ADS frames'))
        self.assertEqual(view.zoom_position_fit_cm('GD_Weap_SMG.A_Weapons.WT'),
                         ([0.0, 0.0, 0.0], 'no fit for this weapon class'))

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


class MuzzleFlashFxTests(unittest.TestCase):
    """An element whose particle system has no layers is left out (the host then draws its procedural flash)."""

    def test_empty_layer_lists_are_dropped(self):
        fx = view.weapon_muzzle_fx
        saved = fx.muzzle_collection, fx.particle_system_layers
        try:
            fx.muzzle_collection = lambda package, path: {'Default': 'Fake.Sys_A', 'Shock': 'Fake.Sys_B'}
            fx.particle_system_layers = lambda package, path: [{'name': 'A'}] if path == 'Fake.Sys_A' else []
            self.assertEqual(view.muzzle_flash_fx_for(None, 'Fake.Coll', {}),
                             {'Default': {'system': 'Fake.Sys_A', 'layers': [{'name': 'A'}]}})
            fx.particle_system_layers = lambda package, path: []
            self.assertIsNone(view.muzzle_flash_fx_for(None, 'Fake.Coll', {}))
        finally:
            fx.muzzle_collection, fx.particle_system_layers = saved


if __name__ == '__main__':
    unittest.main()
