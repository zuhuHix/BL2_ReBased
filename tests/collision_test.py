"""Synthetic geometry checks, independent of source game assets."""
import copy
from pathlib import Path
import sys
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
from collision_geometry import hulls


def tags(**values):
    return [{'name': k, 'value': v, 'status': 'decoded'} for k, v in values.items()]


def fixture(kind, shape):
    return tags(AggGeom=tags(**{kind: [tags(**shape)]}))


class Collision(unittest.TestCase):
    def test_rotated_translated_box(self):
        matrix = dict(zip(('XPlane', 'YPlane', 'ZPlane', 'WPlane'),
            [dict(zip(('X','Y','Z','W'), v)) for v in ((0,1,0,0),(-1,0,0,0),(0,0,1,0),(11,22,33,1))]))
        result = hulls(fixture('BoxElems', dict(TM=matrix, X=2, Y=4, Z=6)))[0]['vertices']
        self.assertEqual([min(v[i] for v in result) for i in range(3)], [9,21,30])
        self.assertEqual([max(v[i] for v in result) for i in range(3)], [13,23,36])
        matrix['WPlane']['W'] = 0
        with self.assertRaisesRegex(ValueError, 'Non-affine'):
            hulls(fixture('BoxElems', dict(TM=matrix, X=2,Y=4,Z=6)))

    def test_convex_bounds_and_indices(self):
        shape = {'VertexData': [dict(zip(('X','Y','Z'), p)) for p in ((0,0,0),(2,0,0),(0,3,0),(0,0,4))],
                 'FaceTriData': [0,2,1,0,1,3,0,3,2,1,2,3],
                 'ElemBox': {'Min': dict(X=0,Y=0,Z=0), 'Max': dict(X=2,Y=3,Z=4), 'IsValid': 1}}
        self.assertEqual(len(hulls(fixture('ConvexElems', shape))), 1)
        for field, value, message in [('FaceTriData', [0,1,4], 'indices'),
                                      ('VertexData', [dict(X=0,Y=0,Z=0)]*4, 'Degenerate')]:
            changed = copy.deepcopy(shape); changed[field] = value
            with self.assertRaisesRegex(ValueError, message): hulls(fixture('ConvexElems', changed))
        shape['ElemBox']['Max']['X'] = 1
        with self.assertRaisesRegex(ValueError, 'outside'): hulls(fixture('ConvexElems', shape))

    def test_unsupported_is_explicit(self):
        with self.assertRaisesRegex(ValueError, 'Unsupported collision shape'):
            hulls(fixture('SphereElems', dict(Radius=10)))
        bad = tags(AggGeom=[{'name': 'ConvexElems', 'status': 'unsupported', 'value': None}])
        with self.assertRaisesRegex(ValueError, 'Undecoded'): hulls(bad)


if __name__ == '__main__': unittest.main()
