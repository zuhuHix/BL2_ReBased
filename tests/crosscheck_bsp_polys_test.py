"""Synthetic Model/Polys fixtures only; no installed-package bytes."""
import struct
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
from bsp_decode import read_surfaces  # noqa: E402
from crosscheck_bsp_polys import decode_polys, compare_model, matching_poly, vertex_normal  # noqa: E402


def pack(fmt, *values):
    return struct.pack('<' + fmt, *values)


def bulk(stride, rows):
    return pack('II', stride, len(rows)) + b''.join(rows)


PLANE = (0., 0., 1., 100.)
BASE, TU, TV = (-2048., -2048., -2048.), (1., 0., 0.), (0., -1., 0.)
QUAD = [(0., 0., 100.), (400., 0., 100.), (400., 400., 100.), (0., 400., 100.)]


def model_payload(index=2, surface_ints=(7, 3584, 3, 0, 1, 2, 0, 0)):
    """A volume-style Model: non-zero 28-byte prefix, one surface on PLANE."""
    vectors = [pack('3f', 0, 0, 1), pack('3f', *TU), pack('3f', *TV)]
    points = [pack('3f', *p) for p in QUAD[:3]] + [pack('3f', *BASE)]
    tail = bytes(range(28)) + bulk(12, vectors) + bulk(12, points)
    tail += bulk(64, [pack('4f12i', *PLANE, 0, 0, 0, 0, 0, -1, -1, -1, -1, 3 << 16, -1, -1)])
    tail += pack('i', index) + pack('I', 1) + pack('8i4f3i', *surface_ints, *PLANE, 0, 0, 0)
    tail += bulk(24, [pack('2i4f', i, -1, 0, 0, 0, 0) for i in range(3)])
    return bytes(12) + tail


def poly_record(base=BASE, normal=(0., 0., 1.), tu=TU, tv=TV, vertices=QUAD, tail=b''):
    return (pack('12f', *base, *normal, *tu, *tv) + pack('i', len(vertices))
            + b''.join(pack('3f', *v) for v in vertices) + tail)


def polys_payload(records, index=5, count=None, capacity=None, owner=None):
    body = b''.join(records)
    n = len(records) if count is None else count
    return bytes(12) + pack('iii', n, n if capacity is None else capacity, index if owner is None else owner) + body


def record(index, kind, prefix, payload, outer=1):
    return {'index': index, 'path': f'TheWorld.PersistentLevel.Volume_0.{kind}_0', 'class': 'Engine.' + kind,
            'outer': outer, 'data': {'property_offset': prefix, 'consumed_bytes': 8,
                                     'trailing_bytes': len(payload) - prefix - 8}}


# A tail that looks like plausible data (a float 32.0, ints 3 and 0, a 1.0) so a
# wrong record size is not rejected for trivial reasons.
TAIL = pack('f', 32.0) + pack('ii', 3, 0) + pack('f', 1.0) + bytes(4)


class PolysDecodeTest(unittest.TestCase):
    def test_recovers_tail_size_and_vectors(self):
        payload = polys_payload([poly_record(tail=TAIL), poly_record(normal=(0., 1., 0.), tail=TAIL)])
        polys, tail = decode_polys(payload, record(5, 'Polys', 4, payload))
        self.assertEqual(tail, len(TAIL))
        self.assertEqual(len(polys), 2)
        self.assertEqual((polys[0]['base'], polys[0]['texture_u'], polys[0]['texture_v']), (BASE, TU, TV))
        self.assertEqual(polys[0]['opaque_tail']['status'], 'UNVERIFIED')
        self.assertFalse(polys[0]['stale_normal'])
        self.assertTrue(polys[1]['stale_normal'])  # stored normal disagrees with the vertices
        self.assertEqual(polys[1]['vertex_normal'], (0., 0., 1.))

    def test_empty_array(self):
        payload = polys_payload([])
        self.assertEqual(decode_polys(payload, record(5, 'Polys', 4, payload)), ([], None))

    def test_rejections(self):
        good = polys_payload([poly_record(tail=TAIL)])
        for payload in (polys_payload([poly_record(tail=TAIL)], owner=6),
                        polys_payload([poly_record(tail=TAIL)], count=2, capacity=2),
                        polys_payload([poly_record(tail=TAIL)], count=1, capacity=0),
                        polys_payload([], count=0) + b'XXXX',
                        good + b'XX',
                        polys_payload([poly_record(normal=(0., 0., 2.), tail=TAIL)]),
                        polys_payload([poly_record(vertices=QUAD[:2], tail=TAIL)])):
            with self.assertRaises(ValueError):
                decode_polys(payload, record(5, 'Polys', 4, payload))
        # Truncation inside the vectors or vertex list always fails; inside the
        # opaque tail it fails unless a whole number of 4-byte words is lost,
        # which the size inference cannot distinguish from a shorter tail.
        vectors_end = 12 + 12 + 52 + 12 * len(QUAD)
        for end in range(12, len(good)):
            r = record(5, 'Polys', 4, good[:end])
            if end >= vectors_end and (len(good) - end) % 4 == 0:
                self.assertEqual(decode_polys(good[:end], r)[1], len(TAIL) - (len(good) - end))
                continue
            with self.assertRaises(ValueError):
                decode_polys(good[:end], r)


class CompareTest(unittest.TestCase):
    def surfaces(self, **kwargs):
        payload = model_payload(**kwargs)
        return read_surfaces(payload, record(2, 'Model', 4, payload))

    def test_agree_and_controls(self):
        vectors, points, nodes, surfaces = self.surfaces()
        polys, _ = decode_polys(polys_payload([poly_record()]), record(5, 'Polys', 4, polys_payload([poly_record()])))
        results, controls = compare_model(vectors, points, surfaces, polys)
        self.assertEqual(results[0]['status'], 'agree')
        self.assertEqual(results[0]['deviation'], {'base': 0.0, 'texture_u': 0.0, 'texture_v': 0.0})
        self.assertEqual(controls['s[2]=base'], 1)
        self.assertEqual(controls['s[4]=texture_u'], 1)
        self.assertEqual(controls['s[5]=texture_v'], 1)
        self.assertNotIn('s[1]=base', controls)

    def test_differ_when_an_axis_is_swapped(self):
        vectors, points, nodes, surfaces = self.surfaces(surface_ints=(7, 3584, 3, 0, 2, 1, 0, 0))
        payload = polys_payload([poly_record()])
        polys, _ = decode_polys(payload, record(5, 'Polys', 4, payload))
        results, _ = compare_model(vectors, points, surfaces, polys)
        self.assertEqual(results[0]['status'], 'differ')
        self.assertGreater(results[0]['deviation']['texture_u'], 0.5)

    def test_invalid_reference_and_ambiguity(self):
        vectors, points, nodes, surfaces = self.surfaces(surface_ints=(7, 3584, 9, 0, 1, 2, 0, 0))
        payload = polys_payload([poly_record()])
        polys, _ = decode_polys(payload, record(5, 'Polys', 4, payload))
        self.assertEqual(compare_model(vectors, points, surfaces, polys)[0][0]['status'], 'invalid_reference')
        payload = polys_payload([poly_record(), poly_record()])
        polys, _ = decode_polys(payload, record(5, 'Polys', 4, payload))
        self.assertIsNone(matching_poly(polys, PLANE))
        vectors, points, nodes, surfaces = self.surfaces()
        self.assertEqual(compare_model(vectors, points, surfaces, polys)[0][0]['status'], 'no_unique_poly')

    def test_matching_uses_vertex_plane_not_stored_normal(self):
        payload = polys_payload([poly_record(normal=(1., 0., 0.))])
        polys, _ = decode_polys(payload, record(5, 'Polys', 4, payload))
        self.assertIs(matching_poly(polys, PLANE), polys[0])
        self.assertIsNone(matching_poly(polys, (0., 0., -1., -100.)))  # back face does not match
        self.assertIsNone(vertex_normal([(0., 0., 0.), (1., 1., 1.), (2., 2., 2.)]))


if __name__ == '__main__':
    unittest.main()
