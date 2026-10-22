"""Synthetic guards for Sanctuary visual import rules; no package assets used."""
import sys
import unittest
import struct
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'host' / 'ue5'))
import prepare_level as prepare
from scene_import_rules import (temporary_sky_policy, ue3_color_linear,
                                SANCTUARY_GRAFITTO_INK_COLOR,
                                SANCTUARY_GRAFITTO_INK_LUMINANCE_CEILING,
                                SANCTUARY_GRAFITTO_INK_OPACITY_GAIN,
                                SANCTUARY_LIGHT_BRIGHTNESS_CEILING,
                                SANCTUARY_ORANGE_POINT_LIGHT_INTENSITY_SCALE,
                                SANCTUARY_WORLD_MAP_PALETTE,
                                ue3_decal_dimensions_to_ue5,
                                ue3_grafitto_dimensions_to_ue5,
                                ue3_grafitto_projection_to_ue5,
                                ue3_directional_brightness_to_ue5,
                                ue3_sky_light_color_to_ue5,
                                ue3_world_post_process_to_ue5,
                                sanctuary_grafitto_opacity,
                                sanctuary_light_calibration,
                                sanctuary_world_map_color)


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
                'emissive_calibration_scale': 2.0,
                'emissive_calibration_evidence': 'synthetic matched frame',
                'emissive_layers': [{'texture': 'Synthetic:Swirl', 'channel': 'RGB',
                                     'color': [0.1, 0.5, 1.0], 'srgb': True}],
                'mask_texture': 'Synthetic:MapMask',
                'mask_channels': {'land': 'R', 'grid': 'G', 'markers': 'B'},
                'palette': {'background': [0.1, 0.2, 0.3], 'land': [0.2, 0.3, 0.4],
                            'grid': [0.01, 0.02, 0.03], 'markers': [0.04, 0.05, 0.06]},
                'omitted': ['synthetic graph'], 'issue': 'UNVERIFIED synthetic approximation',
            },
        }
        try:
            scene = object.__new__(prepare.Scene)
            identities = {('Synthetic', 1): 'Synthetic:Panel',
                          ('Synthetic', 2): 'Synthetic:Base',
                          ('Synthetic', 3): 'Synthetic:Banner',
                          ('Synthetic', 4): 'Synthetic:Swirl',
                          ('Synthetic', 5): 'Synthetic:MapMask'}
            scene.identity = lambda key: identities[key]
            scene.load = lambda package: {i: {'class': 'Engine.Texture2D'} for i in (3, 4, 5)}
            scene.texture = lambda key, channel: f'{key[1]}_{channel}.png'
            result = scene.inspected_scene_material_recipe(
                ('Synthetic', 1), ('Synthetic', 1), [('Synthetic', 3), ('Synthetic', 4), ('Synthetic', 5)])
            self.assertEqual(result['diffuse'], '3_diffuse.png')
            self.assertEqual(result['emissive_layers'][0]['file'], '4_recipe_emissive_layers_0.png')
            self.assertEqual(result['mask_texture_file'], '5_recipe_mask.png')
            self.assertEqual(result['mask_texture_source'], 'Synthetic:MapMask')
            self.assertEqual(result['mask_channels'], {'land': 'R', 'grid': 'G', 'markers': 'B'})
            self.assertEqual(result['palette']['background'], [0.1, 0.2, 0.3])
            self.assertEqual(result['emissive_layers'][0]['color'], [0.2, 1.0, 2.0])
            self.assertEqual(result['emissive_calibration']['scale'], 2.0)
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
            7: {'index': 7, 'path': 'TheWorld.PersistentLevel.StaticLightCollectionActor_0',
                'class': 'Engine.StaticLightCollectionActor'},
            8: {'index': 8, 'path': 'World.PointLightComponent_0', 'class': 'Engine.PointLightComponent', 'outer': 7},
            9: {'index': 9, 'path': 'World.SpotLightComponent_0', 'class': 'Engine.SpotLightComponent', 'outer': 7},
            10: {'index': 10, 'path': 'World.DominantDirectionalLight_0',
                 'class': 'Engine.DominantDirectionalLight'},
            11: {'index': 11, 'path': 'World.DominantDirectionalLight_0.Component',
                 'class': 'Engine.DominantDirectionalLightComponent'},
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
            6: property_tags(Tag='synthetic', DefaultPostProcessSettings=[
                {'name': 'Scene_TonemapperScale', 'value': 2.0, 'status': 'decoded'},
                {'name': 'Scene_MidTones', 'value': {'X': 1.1, 'Y': 1.2, 'Z': 1.3}, 'status': 'decoded'},
                {'name': 'Scene_Shadows', 'value': {'X': 0.02, 'Y': 0.03, 'Z': 0.04}, 'status': 'decoded'},
                {'name': 'Bloom_Scale', 'value': 1.5, 'status': 'decoded'},
            ]),
            10: property_tags(Location={'X': 10, 'Y': 11, 'Z': 12},
                              Rotation={'Pitch': 0, 'Yaw': 16384, 'Roll': 0},
                              LightComponent={'index': 11}),
        }
        scene = object.__new__(prepare.Scene)
        scene.load = lambda level: rows
        scene.resolve = lambda level, reference: (level, reference['index'])
        scene.anchored_properties = lambda package, index, anchor, required: (
            17, {'Brightness': 1.75, 'LightColor': {'R': 255, 'G': 192, 'B': 128, 'A': 255}}, [])
        scene.light_component_class_defaults = lambda: {}
        scene.static_light_collection_records = lambda level, package_rows, defaults, issues: ([
            {'source': 'SyntheticLevel:PointLightComponent_0', 'kind': 'Point',
             'status': 'candidate', 'position': [0.0, 0.0, 0.0], 'radius': 256.0,
             'brightness': 1.0, 'light_color_rgb': [1.0, 0.5, 0.25]},
            {'source': 'SyntheticLevel:SpotLightComponent_0', 'kind': 'Spot',
             'status': 'candidate', 'position': [2000.0, 0.0, 0.0], 'radius': 512.0,
             'brightness': 0.5, 'light_color_rgb': [0.2, 0.8, 1.0]},
        ], {'collection_actors': 1, 'components': 2, 'membership_errors': []})

        def call(level, *args):
            index = int(args[1])
            return {'trailing_bytes': 0, 'properties': props_by_index[index]}

        scene.call = call
        result = scene.lighting_manifest(['SyntheticLevel'])
        self.assertEqual(result['sky_light']['light_color_rgb'], [128 / 255, 64 / 255, 32 / 255])
        self.assertIsNone(result['sky_light']['light_brightness'])
        self.assertEqual(result['sky_light']['light_brightness_status'],
                         'not serialized on this component')
        self.assertEqual(result['height_fog']['density'], 0.002)
        self.assertEqual(result['post_process_volumes'][0]['settings'], {'Scene_Desaturation': 0.2})
        self.assertIsNone(result['post_process_volumes'][0]['b_unbound'])
        self.assertEqual(result['directional_light']['component_property_offset'], 17)
        self.assertEqual(result['directional_light']['light_brightness'], 1.75)
        self.assertEqual(result['directional_light']['light_color_rgb'], [1.0, 192 / 255, 128 / 255])
        self.assertEqual(len(result['world_info']), 1)
        self.assertEqual(result['world_info'][0]['default_post_process_settings']['Bloom_Scale'], 1.5)
        self.assertEqual(result['static_light_collections'], {
            'actors': 1, 'point_components': 1, 'spot_components': 1})
        self.assertEqual(result['static_light_import']['counts']['imported'], 2)
        self.assertEqual([item['status'] for item in result['static_light_import']['components']],
                         ['selected', 'selected'])
        self.assertEqual(result['placed_actor_counts']['SkyLightToggleable'], 1)
        self.assertEqual(result['placed_actor_counts']['HeightFog'], 1)
        self.assertEqual(result['placed_actor_counts']['PostProcessVolume'], 1)

    def test_exact_package_name_anchor_and_host_calibration_rules(self):
        payload = b'prefix' + struct.pack('<ii', 3, 0) + b'suffix'
        self.assertEqual(prepare.property_name_offsets(payload, ['X', 'Y', 'Z', 'LightmassSettings'],
                                                        'LightmassSettings'), [6])
        self.assertEqual(prepare.decoded_texture_properties({
            'property_offset': 8,
            'properties': property_tags(CompressionNoAlpha=False),
            'trailing_bytes': 3064,
        }), {'CompressionNoAlpha': False})
        self.assertEqual(prepare.decal_opacity_source('BLEND_Modulate',
                                                       {'CompressionNoAlpha': False}),
                         'inverse_rgb_luminance_v1')
        self.assertIsNone(prepare.decal_opacity_source('BLEND_Modulate',
                                                       {'CompressionNoAlpha': True}))
        self.assertEqual(prepare.decal_opacity_source('BLEND_Modulate'),
                         'inverse_rgb_luminance_v1')
        self.assertEqual(prepare.decal_opacity_source('BLEND_Masked'),
                         'Texture2D alpha channel')
        self.assertIsNone(prepare.decal_opacity_source('BLEND_Translucent'))
        self.assertEqual(ue3_directional_brightness_to_ue5(1.75), 8.75)
        sky_fill = ue3_sky_light_color_to_ue5([89 / 255, 134 / 255, 195 / 255])
        self.assertAlmostEqual(sky_fill[0], 0.296, places=3)
        self.assertAlmostEqual(sky_fill[1], 0.510, places=3)
        self.assertAlmostEqual(sky_fill[2], 0.554, places=3)
        with self.assertRaisesRegex(ValueError, 'three normalized finite channels'):
            ue3_sky_light_color_to_ue5([1.1, 0.5, 0.2])
        with self.assertRaises(ValueError):
            ue3_directional_brightness_to_ue5(float('inf'))
        mapped = ue3_world_post_process_to_ue5({
            'Scene_TonemapperScale': 2.0,
            'Scene_MidTones': {'X': 1.1, 'Y': 1.2, 'Z': 1.3},
            'Scene_Shadows': {'X': 0.02, 'Y': 0.03, 'Z': 0.04},
            'Bloom_Scale': 1.5,
        })
        self.assertAlmostEqual(mapped['auto_exposure_bias'], -0.5)
        self.assertEqual(mapped['color_gain'], [1.1, 1.2, 1.3])
        self.assertEqual(mapped['color_offset'], [0.02, 0.03, 0.04])
        self.assertEqual(mapped['bloom_intensity'], 1.5)
        with self.assertRaisesRegex(ValueError, 'decoded structs'):
            ue3_world_post_process_to_ue5({'Scene_MidTones': [1.0, 1.0, 1.0]})
        self.assertEqual(ue3_decal_dimensions_to_ue5({
            'width': 500, 'height': 400, 'thickness': 10, 'near_plane': 0, 'far_plane': 300,
        }), [150.0, 250.0, 200.0])
        with self.assertRaisesRegex(ValueError, 'far plane'):
            ue3_decal_dimensions_to_ue5({
                'width': 1, 'height': 1, 'thickness': 1, 'near_plane': 2, 'far_plane': 1,
            })

    def test_anchored_property_stream_requires_exact_name_and_complete_decode(self):
        names = ['None', 'LightmassSettings']
        payload = [0, 0, 0, 0] + list(struct.pack('<ii', 1, 0)) + [0, 0, 0]
        scene = object.__new__(prepare.Scene)
        def call(package, *args):
            if args == ('--names',):
                return names
            if args == ('--payload', 17):
                return payload
            if args[0] == '--properties':
                return {'trailing_bytes': 0,
                        'properties': property_tags(LightmassSettings={'synthetic': True},
                                                     Brightness=2.0, LightColor={'R': 1, 'G': 2, 'B': 3})}
            raise AssertionError(args)
        scene.call = call
        offset, decoded, properties = scene.anchored_properties(
            'Synthetic', 17, 'LightmassSettings', {'Brightness', 'LightColor'})
        self.assertEqual(offset, 4)
        self.assertEqual(decoded['Brightness'], 2.0)
        self.assertEqual(properties[0]['name'], 'LightmassSettings')

        def incomplete(package, *args):
            if args == ('--names',):
                return names
            if args == ('--payload', 17):
                return payload
            if args[0] == '--properties':
                return {'trailing_bytes': 1, 'properties': property_tags(
                    LightmassSettings=True, Brightness=2.0,
                    LightColor={'R': 1, 'G': 2, 'B': 3})}
            raise AssertionError(args)
        scene.call = incomplete
        with self.assertRaisesRegex(ValueError, 'complete LightmassSettings'):
            scene.anchored_properties('Synthetic', 17, 'LightmassSettings',
                                      {'Brightness', 'LightColor'})

    def test_decal_manifest_exports_supported_and_unsupported_counts(self):
        rows = {
            1: {'index': 1, 'path': 'TheWorld.PersistentLevel.DecalActor_0',
                'class': 'Engine.DecalActor'},
            2: {'index': 2, 'path': 'TheWorld.PersistentLevel.DecalActor_0.DecalComponent_0',
                'class': 'Engine.DecalComponent'},
        }
        scene = object.__new__(prepare.Scene)
        scene.load = lambda package: rows
        scene.decal_component_defaults = lambda: ({
            'Width': 200, 'Height': 200, 'Thickness': 10, 'NearPlane': 0,
            'FarPlane': 300, 'FieldOfView': 80,
        }, {name: 'synthetic default' for name in ('Width', 'Height', 'Thickness',
                                                   'NearPlane', 'FarPlane', 'FieldOfView')})
        scene.properties_batch = lambda package, indices, offset, label: (
            {1: {'properties': property_tags(Decal={'index': 2},
                                             Location={'X': 10, 'Y': 20, 'Z': 30},
                                             Rotation={'Pitch': 0, 'Yaw': 0, 'Roll': 0})}}
            if offset == 26 else
            {2: {'properties': property_tags(DecalMaterial={'index': 3},
                                             Width=500, Height=400, HitNormal={'X': 0, 'Y': -1, 'Z': 0},
                                             HitBinormal={'X': 0, 'Y': 0, 'Z': 1})}})
        allowlisted_source = 'Sanctuary_P:Prop_Decals.DMat_GrafittoBurrito'
        material_identity = [allowlisted_source]
        scene.resolve = lambda package, ref: ((package, ref['index']) if ref.get('index') == 2
                                              else ('Sanctuary_P', ref['index']))
        scene.identity = lambda key: material_identity[0] if key[0] == 'Sanctuary_P' else f'{key[0]}:DecalMaterial'
        scene.filename = lambda key, suffix: 'synthetic_decal'
        scene.decal_material_definition = lambda key: ('synthetic_decal', {
            'status': 'partial_unverified', 'source': allowlisted_source,
            'method': 'grafitto_dark_ink_deferred_decal_v1'})
        result = scene.decals_manifest(['SyntheticLevel'])
        self.assertEqual(result['census'], {
            'actors': 1, 'components': 1, 'supported_import_candidates': 1,
            'excluded_import_candidates': 0, 'unsupported': 0, 'unique_materials': 1,
            'supported_materials': 1, 'excluded_materials': 0,
            'unresolved_materials': 0})
        self.assertEqual(result['actors'][0]['dimensions']['width'], 500.0)
        self.assertEqual(result['actors'][0]['projection_basis']['HitNormal'], [0.0, -1.0, 0.0])
        self.assertEqual(result['actors'][0]['status'], 'partial_unverified')

        scene.decal_material_definition = lambda key: ('synthetic_noise', {
            'status': 'partial_unverified', 'source': 'SyntheticMat:NoiseMaterial',
            'method': 'single_texture_deferred_decal_v1'})
        material_identity[0] = 'SyntheticMat:NoiseMaterial'
        denied = scene.decals_manifest(['SyntheticLevel'])
        self.assertEqual(denied['census']['supported_import_candidates'], 0)
        self.assertEqual(denied['census']['excluded_import_candidates'], 1)
        self.assertEqual(denied['census']['supported_materials'], 0)
        self.assertEqual(denied['census']['excluded_materials'], 1)
        self.assertEqual(denied['actors'][0]['status'], 'excluded_default_deny')
        self.assertIn('round-3 default-deny', denied['actors'][0]['exclusion_reason'])

    def test_graffito_mask_and_projection_are_tightly_calibrated(self):
        self.assertEqual(sanctuary_grafitto_opacity([1.0, 1.0, 1.0]), 0.0)
        self.assertEqual(sanctuary_grafitto_opacity([0.0, 0.0, 0.0]), 1.0)
        self.assertEqual(sanctuary_grafitto_opacity([0.6, 0.6, 0.6]), 0.0)
        self.assertEqual(SANCTUARY_GRAFITTO_INK_LUMINANCE_CEILING, 0.55)
        self.assertEqual(SANCTUARY_GRAFITTO_INK_OPACITY_GAIN, 6.0)
        self.assertEqual(SANCTUARY_GRAFITTO_INK_COLOR, (0.001, 0.003, 0.008))
        self.assertEqual(ue3_grafitto_dimensions_to_ue5({
            'width': 500, 'height': 400, 'thickness': 10,
            'near_plane': 0, 'far_plane': 300}), [64.0, 250.0, 200.0])
        self.assertEqual(ue3_grafitto_dimensions_to_ue5({
            'width': 500, 'height': 400, 'thickness': 10,
            'near_plane': 0, 'far_plane': 806}), [64.0, 250.0, 200.0])
        location, forward = ue3_grafitto_projection_to_ue5(
            [100.0, 200.0, 300.0], [0.0, -1.0, 0.0], {
                'width': 500, 'height': 400, 'thickness': 10,
                'near_plane': 0, 'far_plane': 300})
        self.assertEqual(location, [100.0, 264.0, 300.0])
        self.assertEqual(forward, [0.0, -1.0, 0.0])
        with self.assertRaises(ValueError):
            sanctuary_grafitto_opacity([1.2, 0.0, 0.0])
        with self.assertRaises(ValueError):
            ue3_grafitto_projection_to_ue5([0, 0, 0], [0, 0, 0], {
                'width': 100, 'height': 100, 'thickness': 10,
                'near_plane': 0, 'far_plane': 100})

    def test_world_map_palette_has_cyan_backing_and_dark_mask_channels(self):
        palette = prepare.INSPECTED_SCENE_MATERIAL_RECIPES[
            'Sanctuary_P:Prop_Electronics.Materials.Mat_WorldMap']['palette']
        for masks, expected in (([0.0, 0.0, 0.0], palette['background']),
                                ([1.0, 0.0, 0.0], palette['land']),
                                ([0.0, 1.0, 0.0], palette['grid']),
                                ([0.0, 0.0, 1.0], palette['markers'])):
            for actual_channel, expected_channel in zip(sanctuary_world_map_color(masks), expected):
                self.assertAlmostEqual(actual_channel, expected_channel, places=7)
        self.assertGreater(palette['background'][2], palette['background'][0])
        self.assertLess(palette['grid'][1], palette['land'][1])
        self.assertGreater(palette['land'][1] - palette['background'][1], 0.3)
        self.assertLess(palette['markers'][1], palette['background'][1] * 0.1)
        self.assertEqual({key: tuple(value) for key, value in palette.items()},
                         SANCTUARY_WORLD_MAP_PALETTE)
        with self.assertRaises(ValueError):
            sanctuary_world_map_color([0.0, float('nan'), 0.0])

    def test_static_light_selection_enforces_thresholds_spatial_quota_and_global_cap(self):
        candidates = [
            {'source': 'same-a', 'kind': 'Point', 'status': 'candidate', 'position': [0, 0, 0],
             'radius': 256.0, 'brightness': 10.0, 'light_color_rgb': [1.0, 1.0, 1.0]},
            {'source': 'same-b', 'kind': 'Point', 'status': 'candidate', 'position': [1, 0, 0],
             'radius': 256.0, 'brightness': 9.0, 'light_color_rgb': [1.0, 1.0, 1.0]},
            {'source': 'same-c', 'kind': 'Point', 'status': 'candidate', 'position': [2, 0, 0],
             'radius': 256.0, 'brightness': 8.0, 'light_color_rgb': [1.0, 1.0, 1.0]},
            {'source': 'small', 'kind': 'Point', 'status': 'candidate', 'position': [0, 5000, 0],
             'radius': 64.0, 'brightness': 1.0, 'light_color_rgb': [1.0, 1.0, 1.0]},
            {'source': 'unsupported', 'kind': 'Spot', 'status': 'unsupported',
             'skip_reason': 'missing_values'},
        ]
        candidates.extend(
            {'source': f'grid-{index:03d}', 'kind': 'Point', 'status': 'candidate',
             'position': [index * 2000.0, 50000.0, 0.0], 'radius': 256.0,
             'brightness': 1.0, 'light_color_rgb': [1.0, 1.0, 1.0]}
            for index in range(130))
        result, counts = prepare.select_sanctuary_light_components(candidates)
        self.assertEqual(counts['source_components'], 135)
        self.assertEqual(counts['imported'], 128)
        self.assertEqual(counts['skipped'], 7)
        by_source = {item['source']: item for item in result}
        self.assertEqual(by_source['same-a']['status'], 'selected')
        self.assertEqual(by_source['same-b']['status'], 'selected')
        self.assertEqual(by_source['same-c']['skip_reason'], 'spatial_cell_quota')
        self.assertEqual(by_source['small']['skip_reason'], 'below_radius_or_brightness_threshold')
        self.assertEqual(by_source['unsupported']['skip_reason'], 'missing_values')
        self.assertEqual(counts['skip_reasons']['global_cap'], 4)
        self.assertEqual(sanctuary_light_calibration('Point', 2.0), 50.0)
        self.assertEqual(sanctuary_light_calibration('Spot', 2.0), 50.0)
        self.assertEqual(sanctuary_light_calibration('Point', 40.0),
                         SANCTUARY_LIGHT_BRIGHTNESS_CEILING * 25.0)
        self.assertEqual(sanctuary_light_calibration('Point', 40.0, [1.0, 0.44, 0.0]),
                         SANCTUARY_LIGHT_BRIGHTNESS_CEILING * 25.0
                         * SANCTUARY_ORANGE_POINT_LIGHT_INTENSITY_SCALE)
        self.assertEqual(sanctuary_light_calibration('Spot', 40.0, [1.0, 0.44, 0.0]),
                         SANCTUARY_LIGHT_BRIGHTNESS_CEILING * 25.0)

    def test_decal_like_mesh_candidates_stay_on_verified_mesh_path(self):
        old = prepare.DECAL_LIKE_STATIC_MESH_IDENTITIES
        prepare.DECAL_LIKE_STATIC_MESH_IDENTITIES = {'Synthetic:DecalPlane'}
        try:
            scene = object.__new__(prepare.Scene)
            scene.meshes = {'mesh-hash': {'source': 'Synthetic:DecalPlane',
                                          'sections': [{'material': 'material-hash'}]}}
            scene.materials = {'material-hash': {'source': 'Synthetic:Water'}}
            placements = [
                {'mesh': 'mesh-hash', 'level': 'Synthetic_P', 'materials': ['material-hash']},
                {'mesh': 'mesh-hash', 'level': 'Synthetic_FX', 'materials': [None]},
            ]
            result = scene.decal_like_static_meshes(placements)
            self.assertEqual(result[0]['placements'], 2)
            self.assertEqual(result[0]['placements_by_level'], {'Synthetic_P': 1, 'Synthetic_FX': 1})
            self.assertEqual(result[0]['material_sources'], ['Synthetic:Water'])
            self.assertEqual(result[0]['status'], 'already_imported_as_static_mesh')
        finally:
            prepare.DECAL_LIKE_STATIC_MESH_IDENTITIES = old

    def test_ue3_byte_color_is_decoded_with_gamma_2_2(self):
        # Byte 0 and 255 stay black and white; a mid byte becomes darker in linear space.
        self.assertEqual(ue3_color_linear([0.0, 1.0, 0.5])[:2], [0.0, 1.0])
        self.assertAlmostEqual(ue3_color_linear([0.5])[0], 0.5 ** 2.2)

    def test_xmas_bulb_recipe_is_warm_emissive_from_its_own_atlas(self):
        # Synthetic check of the table shape only; the atlas itself is game data
        # and stays under local/.
        entry = prepare.INSPECTED_SCENE_MATERIAL_RECIPES[
            'Sanctuary_P:Prop_Wiretech.MAT_WireXMasWhite']
        self.assertEqual(entry['base'], 'Sanctuary_P:Prop_Wiretech.MAT_WireXMasWhite')
        self.assertNotIn('diffuse', entry)
        layers = entry['emissive_layers']
        self.assertEqual(len(layers), 1)
        self.assertEqual(layers[0]['texture'], 'Sanctuary_P:Prop_Wiretech.XmasLightBulbs_Dif')
        red, green, blue = layers[0]['color']
        self.assertGreater(red, green)
        self.assertGreater(green, blue)
        self.assertGreater(entry['emissive_calibration_scale'], 1.0)

    def test_diffuse_tint_is_scoped_by_base_parameter_and_lit_only(self):
        # Synthetic fixture only: names mirror the rule, values are invented.
        old = prepare.INSPECTED_DIFFUSE_TINTS
        prepare.INSPECTED_DIFFUSE_TINTS = {
            ('Synthetic:Base', 'p_tint'): {'method': 'synthetic_diffuse_tint_v1', 'scope': 'Synthetic:Scoped.',
                                           'omitted': ['graph']},
        }
        try:
            scene = object.__new__(prepare.Scene)
            identities = {('S', 1): 'Synthetic:Scoped.Inst', ('S', 2): 'Synthetic:Base',
                          ('S', 3): 'Synthetic:Other.Inst'}
            scene.identity = lambda key: identities[key]
            chain = {}
            scene.named_chain_parameters = lambda key: chain[key]
            lit = {'lighting_model': 'MLM_Lit'}
            chain[('S', 1)] = (('S', 2), {}, {}, {'p_tint': {'R': 1.0, 'G': 0.5, 'B': 0.25, 'A': 1.0}})
            record = scene.diffuse_tint(('S', 1), lit)
            self.assertEqual(record['rgb'], [1.0, 0.5, 0.25])
            self.assertEqual(record['status'], 'partial_unverified')
            self.assertEqual(record['source_material'], 'Synthetic:Base')
            # Outside the scope, or without the parameter, adds nothing.
            chain[('S', 3)] = (('S', 2), {}, {}, {'p_tint': {'R': 1.0, 'G': 1.0, 'B': 1.0}})
            self.assertIsNone(scene.diffuse_tint(('S', 3), lit))
            chain[('S', 1)] = (('S', 2), {}, {}, {})
            self.assertIsNone(scene.diffuse_tint(('S', 1), lit))
            # Unlit instances keep their own colour path, and bad values are rejected.
            chain[('S', 1)] = (('S', 2), {}, {}, {'p_tint': {'R': 1.0, 'G': 1.0, 'B': 1.0}})
            self.assertIsNone(scene.diffuse_tint(('S', 1), {'lighting_model': 'MLM_Unlit'}))
            chain[('S', 1)] = (('S', 2), {}, {}, {'p_tint': {'R': -1.0, 'G': 1.0, 'B': 1.0}})
            with self.assertRaisesRegex(ValueError, 'invalid'):
                scene.diffuse_tint(('S', 1), lit)
        finally:
            prepare.INSPECTED_DIFFUSE_TINTS = old
    def test_sky_cloud_opacity_is_scaled_and_bounded(self):
        from scene_import_rules import sanctuary_sky_cloud_opacity
        self.assertAlmostEqual(sanctuary_sky_cloud_opacity(1.0), 0.14)
        self.assertAlmostEqual(sanctuary_sky_cloud_opacity(0.0), 0.0)
        self.assertAlmostEqual(sanctuary_sky_cloud_opacity(10.0), 1.0)
        for bad in (-0.1, float('nan'), True, 'one'):
            with self.assertRaises(ValueError):
                sanctuary_sky_cloud_opacity(bad)


if __name__ == '__main__':
    unittest.main()
