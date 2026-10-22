"""Synthetic guards for Sanctuary visual import rules; no package assets used."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'host' / 'ue5'))
import prepare_level as prepare
from scene_import_rules import temporary_sky_policy, ue3_color_linear


def property_tags(**values):
    return [{'name': key, 'value': value, 'status': 'decoded'} for key, value in values.items()]


class SanctuaryVisualRulesTest(unittest.TestCase):
    def test_byte_color_and_post_process_array_require_decoded_fields(self):
        self.assertEqual(prepare.byte_color_rgb({'R': 255, 'G': 128, 'B': 0, 'A': 1}),
                         [1.0, 128 / 255, 0.0])
        self.assertEqual(prepare.decoded_settings([
            {'name': 'Scene_Desaturation', 'value': 0.2, 'status': 'decoded'},
            {'name': 'Unknown', 'value': None, 'status': 'unsupported'},
        ]), {'Scene_Desaturation': 0.2})
        with self.assertRaises(ValueError):
            prepare.byte_color_rgb({'R': 256, 'G': 0, 'B': 0})

    def test_native_dome_disables_temporary_sky_shell_and_atmosphere(self):
        scene = {
            'actors': [{'native_skybox': True, 'mesh': 'dome', 'materials': ['sky']}],
            'meshes': {'dome': {'sections': [{'material': 'sky'}]}},
            'materials': {'sky': {'sky_approximation': {'status': 'partial_unverified'}}},
        }
        self.assertEqual(temporary_sky_policy(scene), {
            'native_sky_approximated': True,
            'spawn_sky_fallback': False,
            'spawn_sky_atmosphere': False,
        })
        scene['materials']['sky'].pop('sky_approximation')
        self.assertEqual(temporary_sky_policy(scene), {
            'native_sky_approximated': False,
            'spawn_sky_fallback': True,
            'spawn_sky_atmosphere': True,
        })

    def test_scene_recipe_requires_exact_base_and_each_cooked_texture(self):
        old = prepare.INSPECTED_SCENE_MATERIAL_RECIPES
        prepare.INSPECTED_SCENE_MATERIAL_RECIPES = {
            'Synthetic:Panel': {
                'method': 'synthetic_panel_v1', 'base': 'Synthetic:Panel',
                'diffuse': 'Synthetic:Banner',
                'emissive_layers': [{'texture': 'Synthetic:Swirl', 'channel': 'RGB',
                                     'color': [0.1, 0.5, 1.0], 'srgb': True}],
                'omitted': ['synthetic graph'], 'issue': 'UNVERIFIED synthetic approximation',
            },
        }
        try:
            scene = object.__new__(prepare.Scene)
            identities = {('Synthetic', 1): 'Synthetic:Panel',
                          ('Synthetic', 2): 'Synthetic:Base',
                          ('Synthetic', 3): 'Synthetic:Banner',
                          ('Synthetic', 4): 'Synthetic:Swirl'}
            scene.identity = lambda key: identities[key]
            scene.load = lambda package: {i: {'class': 'Engine.Texture2D'} for i in (3, 4)}
            scene.texture = lambda key, channel: f'{key[1]}_{channel}.png'
            result = scene.inspected_scene_material_recipe(
                ('Synthetic', 1), ('Synthetic', 1), [('Synthetic', 3), ('Synthetic', 4)])
            self.assertEqual(result['diffuse'], '3_diffuse.png')
            self.assertEqual(result['emissive_layers'][0]['file'], '4_recipe_emissive_layers_0.png')
            with self.assertRaisesRegex(ValueError, 'inspected base material'):
                scene.inspected_scene_material_recipe(
                    ('Synthetic', 1), ('Synthetic', 2), [('Synthetic', 3), ('Synthetic', 4)])
            with self.assertRaisesRegex(ValueError, 'inspected texture'):
                scene.inspected_scene_material_recipe(('Synthetic', 1), ('Synthetic', 1), [('Synthetic', 3)])
        finally:
            prepare.INSPECTED_SCENE_MATERIAL_RECIPES = old

    def test_named_rgb_parameter_is_read_from_exact_material_chain(self):
        old = prepare.INSPECTED_EMISSIVE_MULTIPLIERS
        prepare.INSPECTED_EMISSIVE_MULTIPLIERS = {
            'Synthetic:Instance': {'base': 'Synthetic:Base', 'parameter': 'p_scale',
                                   'method': 'synthetic_emissive_scale_v1', 'omitted': ['graph']},
        }
        try:
            scene = object.__new__(prepare.Scene)
            identities = {('Synthetic', 1): 'Synthetic:Instance', ('Synthetic', 2): 'Synthetic:Base'}
            scene.identity = lambda key: identities[key]
            scene.named_chain_parameters = lambda key: (
                ('Synthetic', 2), {}, {}, {'p_scale': {'R': 1.0, 'G': 4.0, 'B': 8.0, 'A': 1.0}})
            record = scene.inspected_color_parameter(
                ('Synthetic', 1), {}, prepare.INSPECTED_EMISSIVE_MULTIPLIERS)
            self.assertEqual(record['rgb'], [1.0, 4.0, 8.0])
            scene.named_chain_parameters = lambda key: (
                ('Synthetic', 1), {}, {}, {'p_scale': {'R': 1.0, 'G': 1.0, 'B': 1.0}})
            with self.assertRaisesRegex(ValueError, 'inspected base material'):
                scene.inspected_color_parameter(
                    ('Synthetic', 1), {}, prepare.INSPECTED_EMISSIVE_MULTIPLIERS)
        finally:
            prepare.INSPECTED_EMISSIVE_MULTIPLIERS = old

    def test_lighting_manifest_exports_decoded_light_and_post_process_records(self):
        rows = {
            1: {'index': 1, 'path': 'World.SkyLight_0', 'class': 'Engine.SkyLightToggleable'},
            2: {'index': 2, 'path': 'World.SkyLight_0.Component', 'class': 'Engine.SkyLightComponent'},
            3: {'index': 3, 'path': 'World.HeightFog_0', 'class': 'Engine.HeightFog'},
            4: {'index': 4, 'path': 'World.HeightFog_0.Component', 'class': 'Engine.HeightFogComponent'},
            5: {'index': 5, 'path': 'World.PostProcessVolume_0', 'class': 'Engine.PostProcessVolume'},
            6: {'index': 6, 'path': 'World.WorldInfo_0', 'class': 'Engine.WorldInfo'},
            7: {'index': 7, 'path': 'World.StaticLightCollectionActor_0',
                'class': 'Engine.StaticLightCollectionActor'},
            8: {'index': 8, 'path': 'World.PointLightComponent_0', 'class': 'Engine.PointLightComponent', 'outer': 7},
            9: {'index': 9, 'path': 'World.SpotLightComponent_0', 'class': 'Engine.SpotLightComponent', 'outer': 7},
        }
        props_by_index = {
            1: property_tags(Location={'X': 1, 'Y': 2, 'Z': 3}, LightComponent={'index': 2}),
            2: property_tags(LightColor={'R': 128, 'G': 64, 'B': 32, 'A': 255},
                             LowerColor={'R': 10, 'G': 20, 'B': 30, 'A': 255},
                             LowerBrightness=0.25, bPrecomputedLightingIsValid=False),
            3: property_tags(Location={'X': 4, 'Y': 5, 'Z': 6}, Component={'index': 4}),
            4: property_tags(Height=6.0, Density=0.002, LightBrightness=0.5,
                             LightColor={'R': 255, 'G': 128, 'B': 0, 'A': 255},
                             StartDistance=100.0, ExtinctionDistance=1000.0),
            5: property_tags(Location={'X': 7, 'Y': 8, 'Z': 9}, Priority=2,
                             Settings=[{'name': 'Scene_Desaturation', 'value': 0.2,
                                        'status': 'decoded'}]),
            6: property_tags(Tag='synthetic'),
        }
        scene = object.__new__(prepare.Scene)
        scene.load = lambda level: rows
        scene.resolve = lambda level, reference: (level, reference['index'])

        def call(level, *args):
            index = int(args[1])
            return {'trailing_bytes': 0, 'properties': props_by_index[index]}

        scene.call = call
        result = scene.lighting_manifest(['SyntheticLevel'])
        self.assertEqual(result['sky_light']['light_color_rgb'], [128 / 255, 64 / 255, 32 / 255])
        self.assertEqual(result['height_fog']['density'], 0.002)
        self.assertEqual(result['post_process_volumes'][0]['settings'], {'Scene_Desaturation': 0.2})
        self.assertEqual(len(result['world_info']), 1)
        self.assertEqual(result['static_light_collections'], {
            'actors': 1, 'point_components': 1, 'spot_components': 1})
        self.assertEqual(result['placed_actor_counts']['SkyLightToggleable'], 1)
        self.assertEqual(result['placed_actor_counts']['HeightFog'], 1)
        self.assertEqual(result['placed_actor_counts']['PostProcessVolume'], 1)

    def test_ue3_byte_color_is_decoded_with_gamma_2_2(self):
        # Byte 0 and 255 stay black and white; a mid byte becomes darker in linear space.
        self.assertEqual(ue3_color_linear([0.0, 1.0, 0.5])[:2], [0.0, 1.0])
        self.assertAlmostEqual(ue3_color_linear([0.5])[0], 0.5 ** 2.2)


if __name__ == '__main__':
    unittest.main()
