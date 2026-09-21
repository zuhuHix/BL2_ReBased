"""Synthetic scene placement and material inheritance regression tests."""
import importlib.util
import math
from pathlib import Path
import struct
import tempfile
import unittest
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))

spec = importlib.util.spec_from_file_location('prepare_level', Path(__file__).parents[1] / 'tools/prepare_level.py')
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)
geometry_spec = importlib.util.spec_from_file_location('scene_geometry', Path(__file__).parents[1] / 'host/ue5/scene_geometry.py')
geometry = importlib.util.module_from_spec(geometry_spec)
geometry_spec.loader.exec_module(geometry)


def tags(**kwargs):
    return [{'name': k, 'value': v, 'status': 'decoded'} for k, v in kwargs.items()]


def record(**kwargs):
    return {'data': {'properties': tags(**kwargs)}}


class SceneTests(unittest.TestCase):
    def test_material_refresh_restores_only_unlit_dome_interior(self):
        from refresh_materials import restore_sky_policy
        manifest = {'actors': [{'mesh': 'dome', 'materials': ['sky']},
                               {'mesh': 'dome', 'materials': ['floor']}],
                    'meshes': {'dome': {'source': 'Map:' + m.NATIVE_SKYBOX_MESH,
                                        'sections': [{'slot': 0, 'material': 'floor'}]}},
                    'materials': {'sky': {'lighting_model': 'MLM_Unlit'},
                                  'floor': {'lighting_model': 'MLM_DefaultLit'}}}
        restore_sky_policy(manifest)
        self.assertTrue(manifest['materials']['sky']['two_sided'])
        self.assertNotIn('two_sided', manifest['materials']['floor'])

    def test_hls_fallback_validates_parent_atlas_and_keeps_normal(self):
        scene = object.__new__(m.Scene)
        paths = {1: 'Prop_SancBuildings.Optimization.Mati_SancBuild4a',
                 2: 'Prop_SancBuildings.Material.Mati_SancBuild4a',
                 3: 'Prop_SancBuildings.Optimization.Sanc_HLS_Master',
                 4: 'Prop_SancBuildings.Textures.SancBuild4a_Dif', 5: 'Normal', 6: 'Other_Dif'}
        rows = {i: {'path': path, 'index': i,
                    'class': 'Engine.MaterialInstanceConstant' if i < 4 else 'Engine.Texture2D',
                    **record()} for i, path in paths.items()}
        rows[1].update(record(Parent={'index': 3}))
        scene.load = lambda package: rows
        scene.resolve = lambda package, ref: (package, ref['index']) if ref else None
        scene.identity = lambda key: key[0] + ':' + paths[key[1]]
        scene.filename = lambda key, suffix: str(key[1]) + suffix
        scene.texture = lambda key, channel: scene.filename(key, '_' + channel + '.png')
        scene.material_metadata = lambda key: {}
        scene.material_parameters = lambda key: ({'p_normal': ('Sanctuary_P', 5),
                                                 'materialexpressiontexturesampleparameter2d_7': ('Sanctuary_P', 6)} if key[1] == 1
                                                else {'p_diffuse': ('Sanctuary_P', 4)})
        scene.materials, scene.issues = {}, []
        scene.material(('Sanctuary_P', 1))
        self.assertEqual(scene.materials['1']['channels'],
                         {'diffuse': '4_diffuse.png', 'normal': '5_normal.png'})
        scene.materials.clear()
        paths[3] = 'UnexpectedParent'
        scene.material(('Sanctuary_P', 1))
        self.assertEqual(scene.materials['1']['channels'], {})
        self.assertIn('inspected HLS parent', scene.issues[-1]['error'])
        paths[3] = 'Prop_SancBuildings.Optimization.Sanc_HLS_Master'
        scene.materials = {'2': {'source': 'Sanctuary_P:' + paths[2],
                                 'channels': {'diffuse': 'unexpected.png'}}}
        scene.material(('Sanctuary_P', 1))
        self.assertEqual(scene.materials['1']['channels'], {})
        self.assertIn('inspected concrete atlas', scene.issues[-1]['error'])

    def test_frozen_lake_color_is_scoped_and_requires_resource(self):
        scene = object.__new__(m.Scene)
        paths = {1: 'Prop_Glacier.Materials.Mat_FrozenLake',
                 2: 'Prop_Glacier.Textures.FrozenLake', 3: 'Prop_Terrain.Textures.Snow_Dif',
                 4: 'Prop_Skybox.MoveMe.Ice_Nrm'}
        scene.materials, scene.issues = {}, []
        scene.filename = lambda key, suffix: str(key[1]) + suffix
        scene.identity = lambda key: key[0] + ':' + paths[key[1]]
        scene.load = lambda package: {i: {'class': 'Engine.Texture2D'} for i in (2, 3, 4)}
        scene.material_metadata = lambda key: {}
        scene.material_parameters = lambda key: {}
        scene.texture = lambda key, channel: str(key[1]) + '.png'
        scene.cooked_material_textures = lambda key: (key, [(key[0], i) for i in (2, 3, 4)], 48)
        scene.material(('Sanctuary_Land', 1))
        self.assertEqual(scene.materials['1']['channels']['diffuse'], '2.png')
        self.assertEqual(scene.materials['1']['channels']['normal'], '4.png')
        self.assertEqual(scene.materials['1']['surface_approximation']['status'], 'partial_unverified')
        scene.materials.clear()
        scene.material(('Other', 1))
        self.assertEqual(scene.materials['1']['channels']['diffuse'], '3.png')
        scene.materials.clear()
        scene.cooked_material_textures = lambda key: (key, [(key[0], 3)], 48)
        scene.material(('Sanctuary_Land', 1))
        self.assertEqual(scene.materials['1']['channels'], {})
        self.assertIn('requires the inspected ice texture', scene.issues[-2]['error'])

    def test_ice_road_color_is_scoped_and_omits_stripped_normal(self):
        scene = object.__new__(m.Scene)
        paths = {1: 'Env_Sanctuary.Materials.Mat_IceRoadSanctuary',
                 2: 'Env_Ice.Textures.BrokenRoad_Dif', 3: 'Prop_Terrain.Textures.Snow_Dif',
                 4: 'Prop_Terrain.Textures.GrasslandsRock_Dif', 5: 'Env_Ice.Textures.BrokenRoad_Alpha'}
        scene.materials, scene.issues = {}, []
        scene.filename = lambda key, suffix: str(key[1]) + suffix
        scene.identity = lambda key: key[0] + ':' + paths[key[1]]
        scene.load = lambda package: {i: {'class': 'Engine.Texture2D'} for i in (2, 3, 4, 5)}
        scene.material_metadata = lambda key: {}
        scene.material_parameters = lambda key: {'p_Normal': None}
        scene.texture = lambda key, channel: str(key[1]) + '.png'
        scene.cooked_material_textures = lambda key: (key, [(key[0], i) for i in (2, 3, 4, 5)], 112)
        scene.material(('Sanctuary_Land', 1))
        self.assertEqual(scene.materials['1']['channels'], {'diffuse': '2.png'})
        approximation = scene.materials['1']['surface_approximation']
        self.assertEqual(approximation['method'], 'ice_road_color_fallback_v1')
        self.assertNotIn('normal_texture', approximation)
        self.assertIn('road color', scene.issues[-1]['error'])
        scene.materials.clear()
        scene.material(('Other', 1))
        self.assertEqual(scene.materials['1']['channels'], {})
        scene.materials.clear()
        scene.cooked_material_textures = lambda key: (key, [(key[0], i) for i in (3, 4, 5)], 112)
        scene.material(('Sanctuary_Land', 1))
        self.assertEqual(scene.materials['1']['channels'], {})
        self.assertIn('requires the inspected road texture', scene.issues[-2]['error'])

    def test_transition_helpers_do_not_hide_ice_or_ordinary_planes(self):
        materials = {'transition': {'source': 'Sanctuary_Land:' + m.HIDDEN_TRANSITION_MATERIAL},
                     'ice': {'source': 'Sanctuary_Land:Prop_Glacier.Materials.Mat_FrozenLake'}}
        plane = 'Sanctuary_Land:' + m.HIDDEN_TRANSITION_MESH
        self.assertTrue(m.hidden_visual_mesh(plane, ['transition'], materials))
        self.assertFalse(m.hidden_visual_mesh(plane, ['transition', 'ice'], materials))
        self.assertFalse(m.hidden_visual_mesh(plane, [], materials))
        self.assertFalse(m.hidden_visual_mesh(plane, ['missing'], materials))
        self.assertFalse(m.hidden_visual_mesh('Sanctuary_Land:Prop_Glacier.Meshes.IcePlate',
                                            ['transition'], materials))

    def test_glacier_primary_layer_is_scoped_and_preserves_instance_scale(self):
        scene = object.__new__(m.Scene)
        base = 'Prop_Glacier.Materials.Mat_Glacier'
        paths = {1: base, 2: 'Prop_Glacier.Materials.Mati_Glacier2x', 3: base + '.Scale',
                 4: 'Prop_Glacier.Textures.GlacierFront_Dif',
                 5: 'Prop_Glacier.Textures.GlacierFront_Nrm',
                 6: 'Prop_Terrain.Textures.Snow_Dif',
                 7: 'Prop_Skybox.MoveMe.R2Tex_SnowTempCubeStaticNegY'}
        rows = {1: record(Expressions=[{'index': 3}]),
                2: record(Parent={'index': 1}, VectorParameterValues=[tags(
                    ParameterName='P_TexScalar_RGMain_BASnow',
                    ParameterValue=dict(R=3, G=3, B=2, A=3))]),
                3: {**record(ParameterName='P_TexScalar_RGMain_BASnow',
                              DefaultValue=dict(R=1, G=1, B=2, A=2)),
                    'class': 'Engine.MaterialExpressionVectorParameter'}}
        rows.update({i: {'class': 'Engine.Texture2D'} for i in range(4, 8)})
        scene.load = lambda package: rows
        scene.identity = lambda key: 'Synthetic:' + paths[key[1]]
        scene.resolve = lambda package, ref: (package, ref['index']) if ref else None
        textures = [('Synthetic', i) for i in range(4, 8)]
        recipe = lambda i, ts=textures: scene.glacier_primary_surface(('Synthetic', i), ('Synthetic', 1), ts)
        self.assertEqual(recipe(1)['scale'], [1, 1, 2, 2])
        self.assertEqual(recipe(2)['scale'], [3, 3, 2, 3])
        self.assertEqual(recipe(2)['normal'], ('Synthetic', 5))
        scene.materials, scene.issues = {}, []
        scene.filename = lambda key, suffix: str(key[1]) + suffix
        scene.material_metadata = lambda key: {}
        scene.material_parameters = lambda key: {}
        scene.cooked_material_textures = lambda key: (('Synthetic', 1), textures, 32)
        scene.texture = lambda key, channel: str(key[1]) + '_' + channel + '.png'
        scene.material(('Synthetic', 2))
        self.assertEqual(scene.materials['2']['channels'], {'diffuse': '4_diffuse.png', 'normal': '5_normal.png'})
        self.assertEqual(scene.materials['2']['channel_uv']['diffuse']['scale'], [3, 3])
        self.assertEqual(scene.materials['2']['surface_approximation']['status'], 'partial_unverified')
        scene.materials.clear()
        scene.material_parameters = lambda key: {'p_diffuse': None}
        scene.material(('Synthetic', 2))
        self.assertNotIn('surface_approximation', scene.materials['2'])
        scene.materials.clear()
        scene.material_parameters = lambda key: {'p_normal': None}
        scene.material(('Synthetic', 2))
        self.assertNotIn('normal', scene.materials['2']['channels'])
        self.assertNotIn('normal', scene.materials['2']['channel_uv'])
        self.assertIsNone(recipe(2, textures[:-1]))
        self.assertIsNone(recipe(2, textures + [textures[0]]))
        rows[4]['class'] = 'Engine.TextureCube'
        self.assertIsNone(recipe(2))
        rows[4]['class'] = 'Engine.Texture2D'
        paths[2] = 'Prop_Glacier.Materials.UninspectedInstance'
        self.assertIsNone(recipe(2))
        paths[2] = 'Prop_Glacier.Materials.Mati_Glacier2x'
        rows[2] = record(Parent={'index': 1}, VectorParameterValues=[tags(
            ParameterName='P_TexScalar_RGMain_BASnow', ParameterValue=dict(R=float('nan'), G=1, B=1, A=1))])
        with self.assertRaisesRegex(ValueError, 'invalid'):
            recipe(2)

    def test_cooked_resource_bounds_and_opaque_tail(self):
        prefix = struct.pack('<3i16s2i', 0, 0, 1, bytes(16), 1, 2)
        payload = bytes(12) + prefix + struct.pack('<2i', 7, -3) + b'opaque'
        data = dict(property_offset=4, consumed_bytes=8, trailing_bytes=len(payload)-12)
        self.assertEqual(m.cooked_texture_references(payload, data), ([7, -3], 6))
        for length in range(12, 56):
            truncated = payload[:length]
            with self.assertRaises(ValueError):
                m.cooked_texture_references(truncated, dict(data, trailing_bytes=length-12))
        for offset, value in ((12, 1), (16, 1), (44, -1), (44, 1000000)):
            corrupt = bytearray(payload)
            struct.pack_into('<i', corrupt, offset, value)
            with self.assertRaises(ValueError):
                m.cooked_texture_references(corrupt, data)
        with self.assertRaises(ValueError):
            m.cooked_texture_references(payload, dict(data, consumed_bytes=9))

    def test_cooked_diffuse_guard_and_provenance(self):
        scene = object.__new__(m.Scene)
        scene.materials, scene.issues = {}, []
        scene.filename = lambda key, suffix: str(key[1]) + suffix
        scene.identity = lambda key: {1: 'Map:Material', 2: 'Map:Building_Dif', 3: 'Map:Other_Dif'}[key[1]]
        scene.material_metadata = lambda key: {}
        scene.texture = lambda key, channel: 'decoded.png'
        scene.load = lambda package: {2: {'class': 'Engine.Texture2D'}, 3: {'class': 'Engine.Texture2D'}}
        scene.material_parameters = lambda key: {}
        scene.cooked_material_textures = lambda key: (key, [('Map', 2)], 32)
        scene.material(('Map', 1))
        self.assertEqual(scene.materials['1']['channels'], {'diffuse': 'decoded.png'})
        self.assertEqual(scene.materials['1']['diffuse_inference_method'], 'sole_cooked_resource_dif_texture')
        scene.materials.clear()
        scene.cooked_material_textures = lambda key: (key, [('Map', 2), ('Map', 3)], 32)
        scene.material(('Map', 1))
        self.assertEqual(scene.materials['1']['channels'], {})
        scene.materials.clear()
        scene.material_parameters = lambda key: {'p_diffuse': None}
        scene.cooked_material_textures = lambda key: self.fail('Explicit null must suppress inference')
        scene.material(('Map', 1))
        self.assertNotIn('diffuse_inference', scene.materials['1'])

    def test_cooked_non_auxiliary_texture_and_unconnected_diffuse_constant(self):
        names = {2: 'Map:Arm_Diff', 3: 'Map:Arm_Nrm', 4: 'Map:Arm_Comp', 5: 'Map:Plain', 6: 'Map:Sky_Cube', 7: 'Map:Rock_Dif'}
        planar = lambda t: 'TextureCube' if t[1] == 6 else 'Texture2D'
        pick = lambda *keys: m.cooked_diffuse_candidate([('Map', k) for k in keys], lambda t: names[t[1]], planar)
        self.assertEqual(pick(2, 3, 4), (('Map', 2), 'sole_cooked_resource_dif_texture'))
        self.assertEqual(pick(5, 3, 6), (('Map', 5), 'sole_cooked_resource_texture'))
        self.assertEqual(pick(3, 4), (None, None))
        self.assertEqual(pick(5, 3, 2, 7), (None, None))  # two *_Dif: ambiguous
        self.assertEqual(pick(5, 3, 4, 2), (('Map', 2), 'sole_cooked_resource_dif_texture'))
        self.assertEqual(pick(), (None, None))
        names[8] = 'Map:DefaultNormal'
        self.assertEqual(pick(5, 8), (('Map', 5), 'sole_cooked_resource_texture'))
        translucent = lambda *keys: m.cooked_diffuse_candidate([('Map', k) for k in keys], lambda t: names[t[1]], planar, 'BLEND_Translucent')
        self.assertEqual(translucent(5, 3), (None, None))
        self.assertEqual(translucent(2, 3), (('Map', 2), 'sole_cooked_resource_dif_texture'))
        unconnected = {'DiffuseColor': [{'name': 'Expression', 'value': {'index': 0}}]}
        connected = {'DiffuseColor': [{'name': 'Expression', 'value': {'index': 0}}, {'name': 'Mask', 'value': 1}]}
        constant = {'DiffuseColor': [{'name': 'Constant', 'value': {'R': 0.25, 'G': 0.5, 'B': 1.0}}]}
        self.assertEqual(m.unconnected_diffuse_constant(unconnected), [0.0, 0.0, 0.0])
        self.assertIsNone(m.unconnected_diffuse_constant(connected))
        self.assertEqual(m.unconnected_diffuse_constant(constant), [0.25, 0.5, 1.0])
        self.assertIsNone(m.unconnected_diffuse_constant({}))
        # Scene-level: opaque + zero textures + unconnected input records a constant, no neutral issue.
        scene = object.__new__(m.Scene)
        scene.materials, scene.issues = {}, []
        scene.filename = lambda key, suffix: str(key[1]) + suffix
        scene.identity = lambda key: 'Map:Master_Black'
        scene.material_metadata = lambda key: {}
        scene.material_parameters = lambda key: {}
        scene.load = lambda package: {1: {'data': {'properties': [
            {'name': 'DiffuseColor', 'status': 'decoded', 'value': [{'name': 'Expression', 'value': {'index': 0}}]}]}}}
        scene.cooked_material_textures = lambda key: (key, [], 32)
        scene.material(('Map', 1))
        self.assertEqual(scene.materials['1']['constant_diffuse'], [0.0, 0.0, 0.0])
        self.assertFalse(any('neutral fallback' in i['error'] for i in scene.issues))
        scene.materials.clear(); scene.issues.clear()
        scene.material_metadata = lambda key: {'blend_mode': 'BLEND_Translucent'}
        scene.material(('Map', 1))
        self.assertNotIn('constant_diffuse', scene.materials['1'])

    def test_cooked_resource_references_and_parent_validation(self):
        scene = object.__new__(m.Scene)
        payload = struct.pack('<3i16s3i', 0, 0, 1, bytes(16), 1, 1, 3)
        rows = {1: {**record(), 'class': 'Engine.Material'},
                2: {**record(Parent={'index': 1}), 'class': 'Engine.MaterialInstanceConstant'},
                3: {'class': 'Engine.TextureCube'}}
        rows[1]['data'].update(property_offset=0, consumed_bytes=0, trailing_bytes=len(payload))
        scene.load = lambda package: rows
        scene.call = lambda *args: list(payload)
        scene.resolve = lambda package, ref: (package, ref['index'] if isinstance(ref, dict) else ref) if ref else None
        self.assertEqual(scene.cooked_material_textures(('Map', 2)), (('Map', 1), [('Map', 3)], 0))
        rows[3]['class'] = 'Engine.StaticMesh'
        with self.assertRaisesRegex(ValueError, 'not a supported texture'):
            scene.cooked_material_textures(('Map', 2))
        del rows[3]
        with self.assertRaisesRegex(ValueError, 'not a supported texture'):
            scene.cooked_material_textures(('Map', 2))
        rows[2] = {**record(Parent={'index': 2}), 'class': 'Engine.MaterialInstanceConstant'}
        with self.assertRaisesRegex(ValueError, 'cycle'):
            scene.cooked_material_textures(('Map', 2))

    def test_material_identity_filters_class_and_accepts_package_local_classes(self):
        rows = {1: {'path': 'Shared.Name', 'class': 'Engine.Texture2D'},
                2: {'path': 'Shared.Name', 'class': 'Material'}}
        self.assertEqual(m.material_index(rows, 'Shared.Name'), 2)
        rows[3] = {'path': 'Shared.Name', 'class': 'Engine.Material'}
        with self.assertRaisesRegex(ValueError, 'ambiguous'):
            m.material_index(rows, 'Shared.Name')

    def test_unnamed_diffuse_is_unique_and_does_not_override_named_channels(self):
        a, b = ('Map', 1), ('Map', 2)
        names = {a: 'Map:Textures.Building_Dif_04', b: 'Map:Textures.Other_Dif'}
        p = {'materialexpressiontexturesampleparameter2d_0': a}
        self.assertEqual(m.unnamed_diffuse_candidate(p, names.__getitem__), a)
        self.assertIsNone(m.unnamed_diffuse_candidate(dict(p, p_diffuse=None), names.__getitem__))
        self.assertIsNone(m.unnamed_diffuse_candidate(dict(p, materialexpressiontexturesampleparameter2d_1=b), names.__getitem__))
        self.assertIsNone(m.unnamed_diffuse_candidate({'mask': a}, names.__getitem__))
        names[a] = 'Map:Textures.Building_Nrm'
        self.assertIsNone(m.unnamed_diffuse_candidate(p, names.__getitem__))

    def test_host_handedness_adapter(self):
        source = 'v 1 2 3\nvn 0 1 0\nvt .25 .75\nf 1/1/1 2/2/2 3/3/3\n'
        self.assertEqual(geometry.host_obj(source),
                         'v 1 -2 3\nvn 0 -1 0\nvt .25 .75\nf 1/1/1 2/2/2 3/3/3\n')
        self.assertEqual(geometry.host_obj(geometry.host_obj(source)), source)
        self.assertNotEqual(geometry.actor_label('Map', 'Actor1.Mesh', 0),
                            geometry.actor_label('Map', 'Actor2.Mesh', 0))

    def test_actor_component_units_and_signed_scale(self):
        t = m.transform({'Location': {'X': 25}, 'Rotation': {'Yaw': 16384, 'Roll': -32768},
                         'DrawScale': 2, 'DrawScale3D': {'X': -3, 'Y': 4, 'Z': 5}})
        self.assertEqual(t, {'location': [25, 0, 0], 'rotation': [0, 90, -180], 'scale': [-6, 8, 10]})
        self.assertEqual(m.transform({}, True)['scale'], [1, 1, 1])

    def test_matinee_first_key_experiment_offsets_only_the_hull(self):
        pose = {'actor': {'location': [501, 222, -124], 'rotation': [0, 90, 0],
                          'scale': [1, 1, 1]},
                'component': m.transform({}, True)}
        shifted, applied = m.apply_matinee_first_key_pose(
            m.MATINEE_FIRST_KEY_COMPONENT, pose)
        self.assertTrue(applied)
        self.assertEqual(shifted['actor']['location'], [17052, -171572, -288])
        self.assertEqual(shifted['actor']['rotation'], [0, 78.75, 0])
        unchanged, applied = m.apply_matinee_first_key_pose('Other.Component', pose)
        self.assertFalse(applied)
        self.assertIs(unchanged, pose)

    def test_game_winding_becomes_outward_obj_face(self):
        # UE game index order opposes the stored outward normal. After the
        # handedness adapter, standard OBJ cross products must agree with it.
        source = ('v 500 -200 200\nv 500 200 200\nv 500 -200 -200\n'
                  'vt 0 1\nvt 1 1\nvt 0 0\nvn -1 0 0\n'
                  'f 3/3/1 2/2/1 1/1/1\n')
        lines = [line.split() for line in geometry.host_obj(source).splitlines()]
        vertices = [list(map(float, line[1:])) for line in lines if line[0] == 'v']
        normal = next(list(map(float, line[1:])) for line in lines if line[0] == 'vn')
        face = next(line[1:] for line in lines if line[0] == 'f')
        p = [vertices[int(corner.split('/')[0]) - 1] for corner in face]
        a = [p[1][k] - p[0][k] for k in range(3)]
        b = [p[2][k] - p[0][k] for k in range(3)]
        cross = [a[1]*b[2] - a[2]*b[1], a[2]*b[0] - a[0]*b[2], a[0]*b[1] - a[1]*b[0]]
        self.assertGreater(sum(cross[k]*normal[k] for k in range(3)), 0)

    def test_numeric_import_uses_loaded_path_and_class_before_global_scan(self):
        scene = object.__new__(m.Scene)
        scene.resolved, scene.imports = {}, {}
        path = 'Common_Textures.Stub.Synthetic'
        scene.records = {'WrongClass': {1: {'index': 1, 'path': path, 'class': 'Engine.Material'}},
                         'Startup': {7: {'index': 7, 'path': path, 'class': 'Engine.Texture2D'}}}
        calls = []

        def call(package, *args):
            calls.append((package, args))
            self.assertEqual(args, ('--imports',))  # global --resolve must not run
            return [{'index': -3, 'path': path, 'class_name': 'Texture2D'}]

        scene.call = call
        self.assertEqual(scene.resolve('DlcMap', -3), ('Startup', 7))
        self.assertEqual(scene.resolve('DlcMap', -3), ('Startup', 7))
        self.assertEqual(len(calls), 1)

    def test_dlc_resolution_expands_only_for_missing_base_target(self):
        scene = object.__new__(m.Scene)
        scene.resolved, scene.imports, scene.records = {}, {}, {}
        scene.include_dlc = True
        scene.cooked, scene.package_root = Path('base'), Path('install')
        roots = []
        failure = 'ow-package: import target not found: Dlc.Asset'

        def call(package, *args):
            if args == ('--imports',):
                return [{'index': -1, 'path': 'Dlc.Asset', 'class_name': 'Texture2D'}]
            roots.append(args[-1])
            if args[-1] == scene.cooked:
                raise ValueError(failure)
            return {'resolved_package': 'Dlc', 'resolved_index': 2}

        scene.call = call
        self.assertEqual(scene.resolve('DlcMap', -1), ('Dlc', 2))
        self.assertEqual(roots, [Path('base'), Path('install')])
        scene.resolved.clear()
        roots.clear()
        failure = 'invalid property bounds'
        with self.assertRaisesRegex(ValueError, 'invalid property bounds'):
            scene.resolve('DlcMap', -1)
        self.assertEqual(roots, [Path('base')])

    def test_collection_preserves_matrix_and_separate_scale(self):
        matrix = [0, 1, 0, 0, -1, 0, 0, 0, 0, 0, 1, 0, 10, 20, 30, 1]
        payload = bytes(12) + struct.pack('<20fi', *matrix, -2, 3, 4, .5, 1)
        data = {'property_offset': 4, 'consumed_bytes': 8}
        t = m.collection_transforms(payload, data, 1)[0]
        self.assertEqual(t['matrix'], matrix)
        self.assertEqual(t['scale'], [-1, 1.5, 2])
        for bad in (payload[:-1], payload + bytes(84)):
            with self.assertRaises(ValueError):
                m.collection_transforms(bad, data, 1)
        corrupt = bytearray(payload)
        struct.pack_into('<f', corrupt, 12, math.nan)
        with self.assertRaises(ValueError):
            m.collection_transforms(corrupt, data, 1)

    def test_collection_scale_prefers_the_component_property(self):
        cooked = [1, 1, 1]
        # No scale properties on the component: the cooked per-entry tail stands.
        self.assertIs(m.collection_scale({}, cooked), cooked)
        # A component that declares its own scale overrides a stale tail copy.
        self.assertEqual(m.collection_scale({'Scale3D': {'X': 0.97}}, cooked), [0.97, 1, 1])
        self.assertEqual(m.collection_scale({'Scale': 2}, cooked), [2, 2, 2])
        self.assertEqual(m.collection_scale({'Scale3D': {'X': 2, 'Y': 3, 'Z': 4}, 'Scale': 0.5}, cooked), [1, 1.5, 2])

    def test_parent_defaults_child_override_and_explicit_null(self):
        scene = object.__new__(m.Scene)
        records = {1: record(Expressions=[{'index': 3}]),
                   2: record(Parent={'index': 1}, TextureParameterValues=[
                       tags(ParameterName='p_Diffuse', ParameterValue={'index': 8}),
                       tags(ParameterName='p_Normal', ParameterValue={'index': 0})]),
                   3: {**record(ParameterName='p_Diffuse', Texture={'index': 7}),
                       'class': 'Engine.MaterialExpressionTextureSampleParameter2D'}}
        scene.load = lambda package: records
        scene.resolve = lambda package, ref: (package, ref['index']) if isinstance(ref, dict) and ref['index'] else None
        self.assertEqual(scene.material_parameters(('Map', 2)), {'p_diffuse': ('Map', 8), 'p_normal': None})
        records[1] = record(Parent={'index': 2})
        with self.assertRaisesRegex(ValueError, 'cycle'):
            scene.material_parameters(('Map', 2))

    def test_sublevel_cycle_and_frozen_dynamic_placement(self):
        class FakeScene(m.Scene):
            def __init__(self, output):
                self.output = output
                self.meshes, self.materials, self.issues = {}, {}, []
                self.data = {'Startup': {}, 'A_P': {
                    1: {'class': 'Engine.LevelStreamingKismet', **record(PackageName='A_Dynamic')}},
                    'A_Dynamic': {
                    1: {'class': 'Engine.LevelStreamingKismet', **record(PackageName='A_P')},
                    2: {'class': 'Engine.InterpActor', **record(Location={'X': 100})},
                    3: {'class': 'Engine.StaticMeshComponent', 'index': 3, 'outer': 2,
                        'path': 'TheWorld.PersistentLevel.InterpActor_0.Mesh',
                        **record(StaticMesh={'index': 4})},
                    4: {'class': 'Engine.StaticMesh', 'path': 'Synthetic.Mesh'}}}

            def load(self, name):
                return self.data[name]

            def package(self, name):
                if name not in self.data:
                    raise ValueError('Missing package')

            def resolve(self, package, ref):
                return package, ref['index']

            def mesh(self, key):
                self.meshes['synthetic_mesh'] = {'sections': [{'slot': 0, 'material': None}]}
                return 'synthetic_mesh'

        with tempfile.TemporaryDirectory() as folder:
            scene = FakeScene(Path(folder))
            result = scene.build('A_P')
            self.assertEqual(result['levels'], ['A_P', 'A_Dynamic'])
            self.assertEqual(len(result['actors']), 1)
            self.assertTrue(result['actors'][0]['static'])
            self.assertEqual(result['actors'][0]['transform']['actor']['location'], [100, 0, 0])
            scene.data['A_P'][1] = {'class': 'Engine.LevelStreamingKismet', **record(PackageName='Missing')}
            with self.assertRaisesRegex(ValueError, 'Missing'):
                scene.build('A_P')

    def sky_scene(self, records):
        scene = object.__new__(m.Scene)
        scene.load = lambda package: records
        scene.resolve = lambda package, ref: (package, ref['index']) if isinstance(ref, dict) and ref['index'] else None
        scene.identity = lambda key: 'Map:' + records[key[1]]['path']
        scene.texture = lambda key, channel: f'{key[1]}_{channel}.png'
        return scene

    def sky_records(self):
        def expression(cls, **kwargs):
            return {**record(**kwargs), 'class': 'Engine.MaterialExpression' + cls}
        return {
            1: {'path': m.NATIVE_SKY_MASTER, 'class': 'Engine.Material',
                **record(LightingModel='MLM_Unlit', Expressions=[{'index': 10 + i} for i in range(9)])},
            2: {'path': 'Prop_Skybox.Materials.Mati_Sky_Dynamic_INST', 'class': 'Engine.MaterialInstanceConstant',
                **record(Parent={'index': 1},
                         TextureParameterValues=[tags(ParameterName='Masks', ParameterValue={'index': 23})],
                         ScalarParameterValues=[tags(ParameterName='Time_of_Day', ParameterValue=170)],
                         VectorParameterValues=[tags(ParameterName='Horizion_track_color_multiplier',
                                                     ParameterValue={'R': 0.2, 'G': 0.2, 'B': 0.2, 'A': 1})])},
            10: expression('TextureSampleParameter2D', ParameterName='Transition_Track', Texture={'index': 20}),
            11: expression('TextureSampleParameter2D', ParameterName='clouds', Texture={'index': 21}),
            12: expression('TextureSampleParameter2D', ParameterName='Masks', Texture={'index': 22}),
            13: expression('ScalarParameter', ParameterName='Time_of_Day', DefaultValue=0),
            14: expression('ScalarParameter', ParameterName='sky_brightness', DefaultValue=1),
            15: expression('ScalarParameter', ParameterName='Sun_spot_brightness', DefaultValue=6),
            16: expression('ScalarParameter', ParameterName='cloud_cap_opacity', DefaultValue=1),
            17: expression('ScalarParameter', ParameterName='p_CouldBrightness', DefaultValue=1),
            18: expression('VectorParameter', ParameterName='Horizion_track_color_multiplier',
                           DefaultValue={'R': 1, 'G': 1, 'B': 1, 'A': 1}),
            20: {'path': 'Prop_Skybox.Textures.Sky_TransitionBL2Default_Dif', 'class': 'Engine.Texture2D'},
            21: {'path': 'Prop_Skybox.Textures.Clouds_01', 'class': 'Engine.Texture2D'},
            22: {'path': 'Prop_Skybox.Textures.Sky_Multi', 'class': 'Engine.Texture2D'},
            23: {'path': 'Prop_Skybox.Textures.Sky_Multi2', 'class': 'Engine.Texture2D'},
            24: {'path': 'Prop_Skybox.Textures.Cube', 'class': 'Engine.TextureCube'}}

    def test_sky_approximation_reads_instance_over_master_and_records_assumptions(self):
        records = self.sky_records()
        scene = self.sky_scene(records)
        sky = scene.native_sky_approximation(('Map', 2))
        self.assertEqual(sky['method'], 'sky_time_of_day_strip_v1')
        self.assertEqual(sky['status'], 'partial_unverified')
        self.assertEqual(sky['master'], 'Map:' + m.NATIVE_SKY_MASTER)
        # Instance overrides win over master defaults, master defaults fill the rest.
        self.assertEqual(sky['textures']['masks']['source'], 'Map:Prop_Skybox.Textures.Sky_Multi2')
        self.assertEqual(sky['textures']['transition_track']['file'], '20_sky_transition_track.png')
        self.assertEqual(sky['textures']['clouds']['file'], '21_sky_clouds.png')
        self.assertEqual(sky['scalars'], {'time_of_day': 170.0, 'sky_brightness': 1.0,
                                          'sun_spot_brightness': 6.0, 'cloud_cap_opacity': 1.0,
                                          'cloud_brightness': 1.0})
        self.assertEqual(sky['vectors']['horizon_track_color_multiplier'], [0.2, 0.2, 0.2, 1.0])
        self.assertAlmostEqual(sky['time_axis']['column_u'], 170 / 256)
        self.assertIn('UNVERIFIED', sky['time_axis']['note'])
        self.assertIn('time-of-day animation', sky['omitted'])
        # Only the observed master qualifies.
        records[1]['path'] = 'Common_Materials.Sky.Mat_OtherSky'
        self.assertIsNone(scene.native_sky_approximation(('Map', 2)))

    def test_sky_approximation_rejects_incomplete_or_invalid_inputs(self):
        records = self.sky_records()
        scene = self.sky_scene(records)
        records[2]['data']['properties'] = tags(Parent={'index': 1},
                                                TextureParameterValues=[tags(ParameterName='Masks', ParameterValue={'index': 24})])
        with self.assertRaisesRegex(ValueError, 'masks is not a Texture2D'):
            scene.native_sky_approximation(('Map', 2))
        records = self.sky_records()
        scene = self.sky_scene(records)
        records[2]['data']['properties'] = tags(Parent={'index': 1},
                                                ScalarParameterValues=[tags(ParameterName='Time_of_Day', ParameterValue=300)])
        with self.assertRaisesRegex(ValueError, 'outside the transition strip'):
            scene.native_sky_approximation(('Map', 2))
        records = self.sky_records()
        scene = self.sky_scene(records)
        del records[14]
        records[1]['data']['properties'] = tags(LightingModel='MLM_Unlit',
                                                Expressions=[{'index': i} for i in (10, 11, 12, 13, 15, 16, 17, 18)])
        with self.assertRaisesRegex(ValueError, 'sky_brightness is missing'):
            scene.native_sky_approximation(('Map', 2))
        records = self.sky_records()
        scene = self.sky_scene(records)
        records[1]['data']['properties'] = tags(Parent={'index': 2})
        with self.assertRaisesRegex(ValueError, 'cycle'):
            scene.native_sky_approximation(('Map', 2))

    def test_outer_shell_policy_replaces_only_teleported_overrides_with_diffuse_defaults(self):
        materials = {'tele': {'source': 'Outer:FX.Mat_Sanctuary_Teleported', 'channels': {}},
                     'drill_tele': {'source': 'Outer:FX.Mat_Sanctuary_Drill_Teleported', 'channels': {}},
                     'rock_tele': {'source': 'Outer:FX.Mat_Sanctuary_Rock_Teleported', 'channels': {}},
                     'other': {'source': 'Outer:FX.Mat_Sanctuary_Other', 'channels': {}},
                     'hull': {'source': 'Outer:Prop_Skybox.Materials.Mat_SancSkyNew', 'channels': {'diffuse': 'a.png'}},
                     'drill': {'source': 'Outer:Prop_Skybox.Materials.Mat_SancDrillNew', 'channels': {'diffuse': 'b.png'}},
                     'bare': {'source': 'Outer:Prop_Glacier.Materials.Mati_GlacierRocks2X', 'channels': {}}}
        sections = [{'slot': 0, 'material': 'hull'}, {'slot': 1, 'material': 'drill'},
                    {'slot': 2, 'material': 'bare'}, {'slot': 3, 'material': 'hull'}]
        kept, replaced = m.outer_shell_overrides(['tele', 'drill_tele', 'rock_tele', 'other'], sections, materials)
        # Slot 2's default has no diffuse; slot 3 is not a phase-in override.
        self.assertEqual(kept, [None, None, 'rock_tele', 'other'])
        self.assertEqual([r['slot'] for r in replaced], [0, 1])
        self.assertEqual(replaced[0], {'slot': 0, 'override': 'Outer:FX.Mat_Sanctuary_Teleported',
                                       'default': 'Outer:Prop_Skybox.Materials.Mat_SancSkyNew'})
        self.assertTrue(m.outer_shell_mesh('Sanctuary_Outer:Prop_Skybox.Meshes.SanctuarySky'))
        self.assertFalse(m.outer_shell_mesh('Sanctuary_Outer:Prop_Skybox.Meshes.SanctuarySky_LOD'))
        self.assertFalse(m.outer_shell_mesh('Sanctuary_Light:Prop_Skybox.Meshes.Sky_Dome'))
        hull = 'Sanctuary_Outer:Prop_Skybox.Meshes.SanctuarySky'
        actor = {'materials': ['tele', 'drill_tele']}
        m.apply_outer_shell_policy(actor, hull, sections, materials, False)
        self.assertEqual(actor['materials'], ['tele', 'drill_tele'])
        self.assertFalse(actor['outer_shell'])
        m.apply_outer_shell_policy(actor, hull, sections, materials, True)
        self.assertEqual(actor['materials'], [None, None])
        self.assertTrue(actor['outer_shell'])
        self.assertEqual(len(actor['outer_shell_replaced']), 2)
        # Re-applying is stable and withdrawing restores the placed overrides.
        m.apply_outer_shell_policy(actor, hull, sections, materials, True)
        self.assertEqual(len(actor['outer_shell_replaced']), 2)
        m.apply_outer_shell_policy(actor, hull, sections, materials, False)
        self.assertEqual(actor['materials'], ['tele', 'drill_tele'])
        self.assertNotIn('outer_shell_replaced', actor)
        ordinary = {'materials': ['tele']}
        m.apply_outer_shell_policy(ordinary, 'Map:Other.Mesh', sections, materials, True)
        self.assertEqual(ordinary, {'materials': ['tele']})

    def test_native_skybox_policy_is_exact_mesh_only(self):
        self.assertTrue(m.native_skybox_mesh('Prop_Skybox:Prop_Skybox.Meshes.Sky_Dome'))
        self.assertFalse(m.native_skybox_mesh('Prop_Skybox:Prop_Skybox.Meshes.SanctuarySky'))
        self.assertFalse(m.native_skybox_mesh('Prop_Skybox:Prop_Skybox.Meshes.Sky_Dome_LOD1'))
        self.assertFalse(m.native_skybox_mesh('Common_Materials.Sky.Mat_SkyTimeOfDay_Master'))
        materials = {'sky': {'lighting_model': 'MLM_Unlit'},
                     'floor': {'lighting_model': 'MLM_DefaultLit'}}
        self.assertTrue(m.native_skybox_placement(
            'Prop_Skybox:Prop_Skybox.Meshes.Sky_Dome', ['sky'], materials))
        self.assertFalse(m.native_skybox_placement(
            'Prop_Skybox:Prop_Skybox.Meshes.Sky_Dome', ['floor'], materials))
        self.assertFalse(m.native_skybox_placement(
            'Prop_Skybox:Prop_Skybox.Meshes.Sky_Dome', [], materials))
        self.assertTrue(m.hidden_visual_mesh(
            'Sanctuary_Dynamic:Common_Meshes.Blocking.Blocking_Cube'))
        self.assertFalse(m.hidden_visual_mesh(
            'Sanctuary_P:Common_Meshes.Blocking.Blocking_Cube_Other'))
        self.assertFalse(m.hidden_visual_mesh(
            'Sanctuary_P:Common_Meshes.Blocking.Blocking_Cube_Other', ['textured']))
        self.assertTrue(m.hidden_visual_mesh(
            'Sanctuary_P:Common_Meshes.Blocking.Blocking_Cube', ['textured']))
        self.assertTrue(m.hidden_visual_mesh(
            'Sanctuary_P:Common_Meshes.CollisionCube', ['collision']))
        materials = {'cloud': {'source': 'Sanctuary_P:Env_Ice.Materials.Mat_CloudLayer_Light'},
                     'cloud01': {'source': 'Sanctuary_Light:Env_Ice.Materials.Mat_CloudLayer_01'},
                     'other': {'source': 'Sanctuary_P:Env_Ice.Materials.Mat_Other'}}
        self.assertTrue(m.hidden_visual_mesh(
            'Sanctuary_P:Common_Meshes.Blocking.Blocking_Plane', ['cloud'], materials))
        self.assertTrue(m.hidden_visual_mesh(
            'Sanctuary_P:Common_Meshes.Blocking.Blocking_Plane', ['cloud01'], materials))
        self.assertFalse(m.hidden_visual_mesh(
            'Sanctuary_P:Common_Meshes.Blocking.Blocking_Plane', ['other'], materials))
        self.assertFalse(m.hidden_visual_mesh(
            'Sanctuary_P:Common_Meshes.Blocking.Blocking_Plane', ['cloud', 'other'], materials))
        from refresh_materials import restore_hidden_visual_policy
        manifest = {'actors': [{'source': 'A', 'mesh': 'plane', 'materials': ['cloud01'], 'hidden_visual': False},
                               {'source': 'B', 'mesh': 'plane', 'materials': ['other'], 'hidden_visual': True}],
                    'meshes': {'plane': {'source': 'Sanctuary_P:Common_Meshes.Blocking.Blocking_Plane',
                                         'sections': [{'slot': 0, 'material': 'other'}]}},
                    'materials': materials}
        restore_hidden_visual_policy(manifest)
        self.assertEqual([a['hidden_visual'] for a in manifest['actors']], [True, False])
        self.assertTrue(m.hidden_visual_mesh(
            'Sanctuary_P:Prop_Garbage.Meshes.BoxLrg', [], {},
            'TheWorld.PersistentLevel.InterpActor_34.StaticMeshComponent_20'))
        self.assertFalse(m.hidden_visual_mesh(
            'Sanctuary_P:Prop_Garbage.Meshes.BoxLrg', [], {},
            'TheWorld.PersistentLevel.InterpActor_33.StaticMeshComponent_20'))


if __name__ == '__main__':
    unittest.main()
