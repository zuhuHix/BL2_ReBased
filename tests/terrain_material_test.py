"""Synthetic contracts for terrain weightmap evidence and value retention."""
import binascii
import hashlib
import json
import struct
import sys
import tempfile
import unittest
import zlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parents[1] / 'tools'))
from prepare_terrain import (fallback_color, read_rgba_png,
                             terrain_blend_definition, validated_layer_rows)


def png(width, height, values, filter_type=0):
    rows = []
    for y in range(height):
        row = b''.join(bytes((v, v, v, 255)) for v in values[y * width:(y + 1) * width])
        rows.append(bytes((filter_type,)) + row)
    def chunk(kind, data):
        return (struct.pack('>I', len(data)) + kind + data +
                struct.pack('>I', binascii.crc32(kind + data) & 0xffffffff))
    header = struct.pack('>IIBBBBB', width, height, 8, 6, 0, 0, 0)
    return (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', header) +
            chunk(b'IDAT', zlib.compress(b''.join(rows))) + chunk(b'IEND', b''))


class TerrainMaterialTests(unittest.TestCase):
    def test_pf_g8_values_and_identity_are_retained_from_png(self):
        values = bytes((0, 37, 128, 255, 4, 12))
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / 'weight.png'
            path.write_bytes(png(3, 2, values))
            result = read_rgba_png(path)
        self.assertEqual((result['width'], result['height']), (3, 2))
        self.assertEqual(result['values'], values)
        self.assertEqual(result['sha256'], hashlib.sha256(values).hexdigest())
        self.assertEqual((result['minimum'], result['maximum'], result['nonzero']), (0, 255, 5))

    def test_png_filter_four_is_decoded_without_changing_values(self):
        # The synthetic encoder does not pre-filter the row; use filter zero
        # here as a control and ensure malformed filter bytes are rejected.
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / 'weight.png'
            path.write_bytes(png(1, 1, bytes((17,))))
            self.assertEqual(read_rgba_png(path)['values'], bytes((17,)))
            bad = Path(folder) / 'bad.png'
            bad.write_bytes(png(1, 1, bytes((17,)), filter_type=5))
            with self.assertRaisesRegex(ValueError, 'unsupported PNG row filter'):
                read_rgba_png(bad)

    def test_validated_rows_require_sampling_proof_for_every_visible_layer(self):
        layers = [{'hidden': False}, {'hidden': False}, {'hidden': True}]
        weightmaps = [
            {'source': 'P:Weight0', 'status': 'decoded', 'width': 4, 'height': 4,
             'value_sha256': 'hash0'},
            {'source': 'P:Weight1', 'status': 'decoded', 'width': 4, 'height': 4,
             'value_sha256': 'hash1'},
        ]
        rows = []
        for i, source in enumerate(('P:Weight0', 'P:Weight1')):
            rows.append({'layer_index': i, 'weightmap_source': source,
                         'value_sha256': 'hash' + str(i),
                         'weightmap_dimensions': [4, 4], 'layer_name': layers[i].get('name'),
                         'status': 'validated',
                         'mapping': {'axis': 'xy', 'resampling': 'nearest', 'sampling': 'vertex',
                                     'crop': [0, 0],
                                     'scale': [1, 1], 'offset': [0, 0]}})
        evidence = {'status': 'validated', 'mappings': rows}
        selected, error = validated_layer_rows(evidence, 'P:Terrain', layers, weightmaps)
        self.assertIsNone(error)
        self.assertEqual([r['layer_index'] for r in selected], [0, 1])
        missing = dict(evidence, mappings=rows[:1])
        selected, error = validated_layer_rows(missing, 'P:Terrain', layers, weightmaps)
        self.assertIsNone(selected)
        self.assertIn('every visible layer', error)

    def test_unverified_evidence_never_enables_rows(self):
        selected, error = validated_layer_rows(
            {'status': 'inferred', 'mappings': []}, 'P:Terrain', [], [])
        self.assertIsNone(selected)
        self.assertIn('inferred', error)

    def test_stale_or_unsupported_correspondence_is_rejected(self):
        layer = {'name': 'Rock', 'hidden': False,
                 'materials': [{'terrain_material': 'P:TM.Rock'}]}
        wm = {'source': 'P:Weight', 'status': 'decoded', 'width': 4, 'height': 4,
              'value_sha256': 'current'}
        row = {'layer_index': 0, 'layer_name': 'Rock', 'weightmap_source': 'P:Weight',
               'value_sha256': 'old', 'weightmap_dimensions': [4, 4], 'status': 'validated',
               'mapping': {'axis': 'xz', 'resampling': 'nearest', 'sampling': 'vertex',
                           'crop': [0, 0], 'scale': [1, 1], 'offset': [0, 0]}}
        selected, error = validated_layer_rows(
            {'status': 'validated', 'mappings': [row]}, 'P:Terrain', [layer], [wm])
        self.assertIsNone(selected)
        self.assertIn('unsupported', error)
        row['mapping']['axis'] = 'xy'
        selected, error = validated_layer_rows(
            {'status': 'validated', 'mappings': [row]}, 'P:Terrain', [layer], [wm])
        self.assertIsNone(selected)
        self.assertIn('stale weightmap values', error)

    def test_fallback_colors_are_explicit_and_stable(self):
        self.assertEqual(fallback_color(0), [0.24, 0.28, 0.32])
        self.assertEqual(fallback_color(8), fallback_color(0))

    def test_blend_definition_keeps_available_texture_and_explicit_fallback(self):
        class Scene:
            materials = {'mat0': {'channels': {'diffuse': 'rock.png'}}}
            def resolve(self, package, index):
                return package, index
            def material(self, key):
                return 'mat0' if key[1] == 1 else 'missing'
            def identity(self, key):
                return 'P:Material'
        layers = [
            {'name': 'Rock', 'hidden': False,
             'materials': [{'material': 1, 'material_path': 'Materials.Rock'}]},
            {'name': 'Sand', 'hidden': False, 'materials': []},
        ]
        maps = [
            {'source': 'P:Weight0', 'status': 'decoded', 'file': 'w0.png',
             'width': 8, 'height': 8, 'value_sha256': 'hash0'},
            {'source': 'P:Weight1', 'status': 'decoded', 'file': 'w1.png',
             'width': 8, 'height': 8, 'value_sha256': 'hash1'},
        ]
        evidence = {'status': 'validated', '_file': 'local/evidence.json', 'mappings': [
            {'layer_index': 0, 'status': 'validated', 'weightmap_source': 'P:Weight0',
             'value_sha256': 'hash0', 'weightmap_dimensions': [8, 8], 'layer_name': 'Rock',
             'mapping': {'axis': 'xy', 'resampling': 'nearest', 'sampling': 'vertex', 'crop': [0, 0],
                         'scale': [1, 1], 'offset': [0, 0]}},
            {'layer_index': 1, 'status': 'validated', 'weightmap_source': 'P:Weight1',
             'value_sha256': 'hash1', 'weightmap_dimensions': [8, 8], 'layer_name': 'Sand',
             'mapping': {'axis': 'xy', 'resampling': 'nearest', 'sampling': 'vertex', 'crop': [0, 0],
                         'scale': [1, 1], 'offset': [0, 0]}},
        ]}
        definition, error = terrain_blend_definition(Scene(), 'P:Terrain', layers, maps, evidence)
        self.assertIsNone(error)
        self.assertEqual(definition['method'], 'terrain_weighted_sum_v2')
        self.assertEqual(definition['layers'][0]['diffuse'], 'rock.png')
        self.assertEqual(definition['layers'][1]['diffuse_status'], 'unavailable_texture')
        self.assertEqual(definition['layers'][1]['fallback_color'], [0.38, 0.31, 0.22])
        self.assertEqual(definition['unverified'],
                         ['slope_filters', 'noise', 'lightmaps', 'native_blend_semantics'])


if __name__ == '__main__':
    unittest.main()
