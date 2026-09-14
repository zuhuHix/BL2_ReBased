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
                         'v 1 -2 3\nvn 0 -1 0\nvt .25 .75\nf 3/3/3 2/2/2 1/1/1\n')
        self.assertEqual(geometry.host_obj(geometry.host_obj(source)), source)
        self.assertNotEqual(geometry.actor_label('Map', 'Actor1.Mesh', 0),
                            geometry.actor_label('Map', 'Actor2.Mesh', 0))

    def test_actor_component_units_and_signed_scale(self):
        t = m.transform({'Location': {'X': 25}, 'Rotation': {'Yaw': 16384, 'Roll': -32768},
                         'DrawScale': 2, 'DrawScale3D': {'X': -3, 'Y': 4, 'Z': 5}})
        self.assertEqual(t, {'location': [25, 0, 0], 'rotation': [0, 90, -180], 'scale': [-6, 8, 10]})
        self.assertEqual(m.transform({}, True)['scale'], [1, 1, 1])

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
                        **record(StaticMesh={'index': 4})}}}

            def load(self, name):
                return self.data[name]

            def package(self, name):
                if name not in self.data:
                    raise ValueError('Missing package')

            def resolve(self, package, ref):
                return package, ref['index']

            def mesh(self, key):
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


if __name__ == '__main__':
    unittest.main()
