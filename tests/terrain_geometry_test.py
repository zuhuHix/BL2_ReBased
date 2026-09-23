"""Synthetic component vertex/strip decoding and floor emission; no game bytes.

The fixture mirrors the observed tail layout only structurally: a u16 record
array with its opaque stride, the 72-byte block ending in (1, export index),
the 8-byte vertex array, two retained words and the index strip.
"""
from pathlib import Path
import struct
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
from terrain_decode import (OPAQUE_BLOCK_BYTES, OPAQUE_RECORD_STRIDE, decode_component_geometry,
                            strip_triangles, validate_leaf_coverage)
from prepare_terrain import cell_faces, component_obj, mapping_scale, runtime_probes

INDEX = 77
# 3 x 3 patches (4 x 4 samples); cell (1,1) is a hole, cell (2,0) is flipped.
WIDTH, HEIGHT = 4, 4
HEIGHTS = [32768 + 128 * (x + 2 * y) for y in range(HEIGHT) for x in range(WIDTH)]
FLAGS = [0] * (WIDTH * HEIGHT)
FLAGS[1 * WIDTH + 1] = 1
FLAGS[0 * WIDTH + 2] = 2


def terrain():
    return {'width': WIDTH, 'height': HEIGHT, 'heights': HEIGHTS, 'flags': FLAGS}


def strip(cells, lw):
    """Strip of two triangles per cell with degenerate restarts, like the observed data."""
    out = []
    for x, y, flipped in cells:
        a = y * lw + x
        b, c, d = a + 1, a + lw, a + lw + 1
        run = [a, c, b, d] if flipped else [c, a, d, b]
        if out:
            out += [out[-1], run[0]]
        out += run
    return out


def component(section=(0, 0, 3, 3), records=2, cells=None, index=INDEX, vertex_heights=None, words=(0, 6)):
    x0, y0, sx, sy = section
    lw = sx + 1
    if cells is None:
        cells = [(x, y, bool(FLAGS[(y0 + y) * WIDTH + x0 + x] & 2)) for y in range(sy) for x in range(sx)
                 if not FLAGS[(y0 + y) * WIDTH + x0 + x] & 1]
    tail = struct.pack('<II', 2, records) + bytes(2 * records) + bytes(OPAQUE_RECORD_STRIDE * records)
    tail += bytes(OPAQUE_BLOCK_BYTES - 8) + struct.pack('<II', 1, index)
    tail += struct.pack('<II', 8, lw * (sy + 1))
    for y in range(sy + 1):
        for x in range(sx + 1):
            h = (vertex_heights or HEIGHTS)[(y0 + y) * WIDTH + x0 + x]
            tail += struct.pack('<BBHhh', x, y, h, 0, 0)
    indices = strip(cells, lw)
    tail += struct.pack('<II', *words) + struct.pack('<II', 2, len(indices)) + struct.pack('<%dH' % len(indices), *indices)
    payload = bytes(40) + tail + b'opaque'
    diagnostic = {'opaque_tail_offset': 40, 'properties': {'SectionBaseX': x0, 'SectionBaseY': y0,
                                                           'SectionSizeX': sx, 'SectionSizeY': sy},
                  'nodes': [{'leaf': False, 'children': [1, 2, 65535, 65535]},
                            {'leaf': True, 'grid': [0, 0, 2, 3]}, {'leaf': True, 'grid': [2, 0, 1, 3]}]}
    return payload, diagnostic


class GeometryTests(unittest.TestCase):
    def test_strip_cells_follow_flags(self):
        payload, diagnostic = component()
        geometry = decode_component_geometry(payload, {}, terrain(), diagnostic, INDEX)
        self.assertEqual(geometry['cells'], [[0, 0, False], [1, 0, False], [2, 0, True],
                                             [0, 1, False], [2, 1, False],
                                             [0, 2, False], [1, 2, False], [2, 2, False]])
        self.assertEqual(geometry['opaque_tail_bytes'], 6)
        self.assertEqual(geometry['retained_words'], [0, 6])
        self.assertEqual(validate_leaf_coverage(diagnostic, geometry)['leaves'], 2)

    def test_strip_parity_and_degenerates(self):
        self.assertEqual(list(strip_triangles([0, 1, 2, 2, 5, 5, 6, 7])), [(0, 1, 2), (6, 5, 7)])

    def test_rejections(self):
        base, diagnostic = component()
        cases = {
            'self reference': component(index=INDEX + 1)[0],
            'record count': struct.pack('<II', 2, 3) + base[48:],
            'vertex height': component(vertex_heights=[h + 1 for h in HEIGHTS])[0],
            'hole covered': component(cells=[(x, y, False) for y in range(3) for x in range(3)])[0],
            'wrong diagonal': component(cells=[(x, y, False) for y in range(3) for x in range(3) if (x, y) != (1, 1)])[0],
            'missing cell': component(cells=[(0, 0, False)])[0],
        }
        for name, payload in cases.items():
            with self.subTest(name=name), self.assertRaises(ValueError):
                decode_component_geometry(payload, {}, terrain(), diagnostic, INDEX)
        spanning = bytearray(base)
        strip_start = len(base) - 6 - 2 * (8 * 6 + 2 * 7)
        struct.pack_into('<H', spanning, strip_start, 15)
        with self.assertRaises(ValueError):
            decode_component_geometry(bytes(spanning), {}, terrain(), diagnostic, INDEX)
        for length in range(40, len(base) - 6):
            with self.subTest(length=length), self.assertRaises(ValueError):
                decode_component_geometry(base[:length], {}, terrain(), diagnostic, INDEX)

    def test_leaf_coverage_rejections(self):
        payload, diagnostic = component()
        geometry = decode_component_geometry(payload, {}, terrain(), diagnostic, INDEX)
        for nodes in ([{'leaf': True, 'grid': [0, 0, 3, 4]}],
                      [{'leaf': True, 'grid': [0, 0, 2, 3]}],
                      [{'leaf': True, 'grid': [0, 0, 3, 3]}, {'leaf': True, 'grid': [2, 0, 1, 1]}],
                      [{'leaf': True, 'grid': [0, 0, 1, 1]}, {'leaf': True, 'grid': [1, 1, 1, 1]},
                       {'leaf': True, 'grid': [2, 0, 1, 3]}, {'leaf': True, 'grid': [0, 1, 1, 2]},
                       {'leaf': True, 'grid': [1, 0, 1, 1]}, {'leaf': True, 'grid': [1, 2, 1, 1]}]):
            with self.subTest(nodes=nodes), self.assertRaises(ValueError):
                validate_leaf_coverage({'nodes': nodes}, geometry)

    def test_floor_obj_winding_and_uv(self):
        payload, diagnostic = component()
        geometry = decode_component_geometry(payload, {}, terrain(), diagnostic, INDEX)
        text, triangles = component_obj(terrain(), geometry['section'], geometry['cells'], [0.25, 0.5])
        self.assertEqual(triangles, 16)
        lines = text.splitlines()
        vertices = [tuple(map(float, l.split()[1:])) for l in lines if l.startswith('v ')]
        uvs = [tuple(map(float, l.split()[1:])) for l in lines if l.startswith('vt ')]
        self.assertEqual(vertices[5], (1.0, 1.0, 3.0))
        self.assertEqual(uvs[5], (0.25, 0.5))
        faces = [[int(c.split('/')[0]) - 1 for c in l.split()[1:]] for l in lines if l.startswith('f ')]
        for face in faces:
            p = [vertices[i] for i in face]
            cross = (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[1][1] - p[0][1]) * (p[2][0] - p[0][0])
            self.assertLess(cross, 0)
        self.assertEqual(cell_faces(2, 0, True, 4, 0, 0), ((2, 6, 3), (3, 6, 7)))
        with self.assertRaises(ValueError):
            component_obj(terrain(), geometry['section'], [[3, 0, False]], [1, 1])

    def test_runtime_probe_candidates(self):
        # 12 x 12 patches split into two components; a large hole block gives
        # the spaced runtime selector several independent candidates.
        w = 13
        flat = {'width': w, 'height': w, 'heights': [32768] * (w * w), 'flags': [0] * (w * w)}
        hole_area = {(x, y) for y in range(3, 10) for x in range(3, 10)}
        for x, y in hole_area:
            flat['flags'][y * w + x] = 1
        cells = [(x, y) for y in range(12) for x in range(12) if not flat['flags'][y * w + x] & 1]
        components = [{'section': [0, 0, 6, 12], 'cells': [[x, y, False] for x, y in cells if x < 6]},
                      {'section': [6, 0, 6, 12], 'cells': [[x, y, False] for x, y in cells if x >= 6]}]
        pose = {'location': [100, 200, 300], 'rotation': [0, 0, 0], 'scale': [128, 128, 256]}
        probes = runtime_probes(flat, pose, 'T', components)
        self.assertEqual(probes['stand'], probes['stands'][0])
        self.assertEqual(len(probes['stands']), 6)
        chosen = [tuple(s['cell']) for s in probes['stands']]
        for i, a in enumerate(chosen):
            self.assertNotIn(a, hole_area)
            for b in chosen[i + 1:]:
                self.assertGreaterEqual(max(abs(a[0] - b[0]), abs(a[1] - b[1])), 4)
        self.assertIn(tuple(probes['hole']['cell']), hole_area)
        self.assertEqual(probes['holes'], 49)
        self.assertEqual(probes['hole'], probes['hole_candidates'][0])
        self.assertEqual(probes['hole_probe_policy'], 'three_spaced_adjacent_cells_v1')
        self.assertEqual(len(probes['hole_candidates']), 3)
        hole_cells = [tuple(h['cell']) for h in probes['hole_candidates']]
        for i, a in enumerate(hole_cells):
            self.assertIn(a, hole_area)
            for b in hole_cells[i + 1:]:
                self.assertGreaterEqual(max(abs(a[0] - b[0]), abs(a[1] - b[1])), 4)
        self.assertEqual(probes['stand']['surface'], [300.0, 300.0])
        self.assertEqual(probes['stand']['point'][2], 450.0)
        self.assertEqual(probes['seam']['cells'][0][0] + 1, probes['seam']['cells'][1][0])

    def test_mapping_scale(self):
        diagonal = {'LocalToMapping': {'XPlane': {'X': .25, 'Y': 0, 'Z': 0, 'W': 0},
                                       'YPlane': {'X': 0, 'Y': .25, 'Z': 0, 'W': 0},
                                       'ZPlane': {'X': 0, 'Y': 0, 'Z': .25, 'W': 0},
                                       'WPlane': {'X': 0, 'Y': 0, 'Z': 0, 'W': 1}}}
        self.assertEqual(mapping_scale(diagonal), [.25, .25])
        rotated = {'LocalToMapping': {**diagonal['LocalToMapping'], 'XPlane': {'X': .25, 'Y': .1, 'Z': 0, 'W': 0}}}
        self.assertIsNone(mapping_scale(rotated))
        self.assertIsNone(mapping_scale({}))


if __name__ == '__main__':
    unittest.main()
