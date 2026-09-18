"""Synthetic glTF/OBJ/.mat fixtures for the umodel asset cross-check."""
import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
from crosscheck_umodel_assets import (choose_mapping, compare_material, compare_mesh, load_gltf, load_obj,
                                      map_position, match_fraction, parse_mat, position_scale)

# A quad in UE centimetres: X,Y,Z. umodel writes metres with Y and Z swapped.
QUAD = [(0.0, 0.0, 0.0), (100.0, 0.0, 0.0), (100.0, 200.0, 50.0), (0.0, 200.0, 50.0)]
UVS = [(0.0, 0.0), (1.0, 0.0), (1.0, 0.75), (0.0, 0.75)]
FACES = [(0, 1, 2), (0, 2, 3)]
SWAP = ((0, 2, 1), (1, 1, 1))


def write_gltf(directory, name, points, uvs, faces, material='Mat_Synth'):
    positions = b''.join(struct.pack('<3f', p[0] / 100.0, p[2] / 100.0, p[1] / 100.0) for p in points)
    texcoords = b''.join(struct.pack('<2f', *uv) for uv in uvs)
    indices = b''.join(struct.pack('<H', i) for face in faces for i in face)
    binary = indices + positions + texcoords
    gltf = dict(
        asset=dict(version='2.0'),
        materials=[dict(name=material)],
        meshes=[dict(name=name, primitives=[dict(attributes=dict(POSITION=1, TEXCOORD_0=2), indices=0, material=0)])],
        buffers=[dict(uri=name + '.bin', byteLength=len(binary))],
        bufferViews=[dict(buffer=0, byteOffset=0, byteLength=len(indices)),
                     dict(buffer=0, byteOffset=len(indices), byteLength=len(positions)),
                     dict(buffer=0, byteOffset=len(indices) + len(positions), byteLength=len(texcoords))],
        accessors=[dict(bufferView=0, componentType=5123, count=len(indices) // 2, type='SCALAR'),
                   dict(bufferView=1, componentType=5126, count=len(points), type='VEC3'),
                   dict(bufferView=2, componentType=5126, count=len(uvs), type='VEC2')])
    (directory / (name + '.gltf')).write_text(json.dumps(gltf), encoding='utf-8')
    (directory / (name + '.bin')).write_bytes(binary)
    return directory / (name + '.gltf')


def write_obj(directory, name, points, uvs, faces, material='Synth.Materials.Mat_Synth'):
    lines = ['# OpenWillow local extraction; UE coordinates X,Y,Z; centimeters', f'# material {material}']
    lines += [f'v {x} {y} {z}' for x, y, z in points]
    lines += [f'vt {u} {v}' for u, v in uvs]
    lines += [f'f {a + 1}/{a + 1}/{a + 1} {b + 1}/{b + 1}/{b + 1} {c + 1}/{c + 1}/{c + 1}' for a, b, c in faces]
    path = directory / (name + '.obj')
    path.write_text('\n'.join(lines) + '\n', encoding='utf-8')
    return path


class GeometryTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def test_gltf_and_obj_round_trip(self):
        primitives = load_gltf(write_gltf(self.dir, 'Quad', QUAD, UVS, FACES))
        self.assertEqual(len(primitives), 1)
        self.assertEqual((primitives[0]['vertices'], primitives[0]['triangles'], primitives[0]['material']), (4, 2, 'Mat_Synth'))
        obj = load_obj(write_obj(self.dir, 'Quad_s0', QUAD, UVS, FACES))
        self.assertEqual((len(obj['positions']), len(obj['uvs']), obj['faces'], obj['material']),
                         (4, 4, FACES, 'Synth.Materials.Mat_Synth'))

    def test_axis_mapping_is_recovered_from_bounding_boxes(self):
        primitives = load_gltf(write_gltf(self.dir, 'Quad', QUAD, UVS, FACES))
        mapping, error = choose_mapping([(QUAD, primitives[0]['positions'])])
        self.assertEqual(mapping, SWAP)
        self.assertAlmostEqual(error, 0.0, places=3)
        self.assertEqual(map_position((1.0, 0.5, 2.0), SWAP), (100.0, 200.0, 50.0))

    def test_match_fraction_tolerates_float_noise_but_not_offsets(self):
        noisy = [(x + 0.0004, y - 0.0004, z) for x, y, z in QUAD]
        self.assertEqual(match_fraction(QUAD, noisy, 100.0), 1.0)
        shifted = [(x + 1.0, y, z) for x, y, z in QUAD]
        self.assertEqual(match_fraction(QUAD, shifted, 100.0), 0.0)
        self.assertIsNone(match_fraction(QUAD, [], 100.0))

    def test_compare_mesh_agrees_on_identical_geometry(self):
        primitives = load_gltf(write_gltf(self.dir, 'Quad', QUAD, UVS, FACES))
        obj = load_obj(write_obj(self.dir, 'Quad_s0', QUAD, UVS, FACES))
        report = compare_mesh([dict(slot=0, obj=obj, material_name='Mat_Synth')], primitives, SWAP)
        self.assertEqual(report['mismatches'], [])
        section = report['section_reports'][0]
        self.assertEqual((section['triangles'], section['vertices'], section['position_match']), (2, 4, 1.0))
        self.assertEqual(section['uv_match_identity'], 1.0)
        self.assertLess(section['uv_match_vflip'], 1.0)

    def test_compare_mesh_reports_each_kind_of_disagreement(self):
        primitives = load_gltf(write_gltf(self.dir, 'Quad', QUAD, UVS, FACES, material='Mat_Other'))
        moved = [(x, y, z + 10.0) for x, y, z in QUAD]
        obj = load_obj(write_obj(self.dir, 'Quad_s0', moved, UVS, FACES[:1]))
        report = compare_mesh([dict(slot=0, obj=obj, material_name='Mat_Synth')], primitives, SWAP)
        # Only one face references three of the four vertices, so the vertex count differs as well.
        self.assertEqual(report['mismatches'], ['triangles[0]', 'vertices[0]', 'positions[0]', 'material[0]'])
        report = compare_mesh([], primitives, SWAP)
        self.assertEqual(report['mismatches'], ['section_count'])

    def test_dummy_material_is_not_an_oracle(self):
        primitives = load_gltf(write_gltf(self.dir, 'Quad', QUAD, UVS, FACES, material='dummy_material_0'))
        obj = load_obj(write_obj(self.dir, 'Quad_s0', QUAD, UVS, FACES))
        report = compare_mesh([dict(slot=0, obj=obj, material_name='Mat_Synth')], primitives, SWAP)
        self.assertEqual(report['mismatches'], [])
        self.assertIsNone(report['section_reports'][0]['umodel_material'])

    def test_position_cells_widen_for_skybox_sized_meshes(self):
        self.assertEqual(position_scale(QUAD), 100.0)
        self.assertEqual(position_scale([(-150000.0, 0.0, 0.0)]), 10.0)
        far = [(x + 150000.0, y, z) for x, y, z in QUAD]
        # float32 metres carry about 0.02 cm of noise at 1.5 km; that must still match.
        noisy = [(x + 0.02, y, z) for x, y, z in far]
        self.assertEqual(match_fraction(far, noisy, position_scale(far)), 1.0)


class MaterialTest(unittest.TestCase):
    MAT = 'Diffuse=House_Dif\nNormal=House_Nrm\nOther[0]=House_Comp\n'

    def test_parse_mat(self):
        self.assertEqual(parse_mat(self.MAT), {'Diffuse': 'House_Dif', 'Normal': 'House_Nrm', 'Other[0]': 'House_Comp'})

    def test_compare_material_statuses(self):
        mat = parse_mat(self.MAT)
        result = compare_material({'diffuse': 'House_Dif', 'emissive': 'House_Comp', 'specular': 'House_Spec'}, mat)
        self.assertEqual({k: v['status'] for k, v in result.items()},
                         {'diffuse': 'agree', 'normal': 'umodel_only', 'emissive': 'ours_in_umodel_other', 'specular': 'ours_only'})
        self.assertEqual(compare_material({'diffuse': 'Other_Dif'}, mat)['diffuse']['status'], 'differ')
        self.assertEqual(compare_material({}, {}), {})


if __name__ == '__main__':
    unittest.main()
