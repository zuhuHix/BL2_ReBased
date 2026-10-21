"""Synthetic geometry checks, independent of source game assets."""
import copy
from pathlib import Path
import sys
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
from collision_geometry import hulls, shapes
from collision_volumes import apply_live_state, live_observations, volume_record


def tags(**values):
    return [{'name': k, 'value': v, 'status': 'decoded'} for k, v in values.items()]


def fixture(kind, shape, key='AggGeom'):
    return tags(**{key: tags(**{kind: [tags(**shape)]})})


def matrix(x=(1,0,0), y=(0,1,0), z=(0,0,1), origin=(0,0,0)):
    return dict(zip(('XPlane', 'YPlane', 'ZPlane', 'WPlane'),
        [dict(zip(('X','Y','Z','W'), (*v, w))) for v, w in ((x,0),(y,0),(z,0),(origin,1))]))


BOX = {'VertexData': [dict(zip(('X','Y','Z'), p)) for p in
                      ((-8,-8,-8),(8,-8,-8),(-8,8,-8),(8,8,-8),(-8,-8,8),(8,-8,8),(-8,8,8),(8,8,8))]}


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

    def test_sphere_and_capsule(self):
        sphere = shapes(fixture('SphereElems', dict(TM=matrix(origin=(1,2,3)), Radius=10)))
        self.assertEqual(sphere['spheres'], [{'center': [1,2,3], 'radius': 10}])
        capsule = shapes(fixture('SphylElems', dict(TM=matrix((0,0,1),(0,1,0),(-1,0,0),(4,5,6)), Radius=6, Length=168)))
        self.assertEqual(capsule['capsules'][0]['axes'][2], [-1,0,0])
        self.assertEqual((capsule['capsules'][0]['radius'], capsule['capsules'][0]['length']), (6, 168))
        # A UE5 sphere/capsule element has no scale: refuse rather than drop it.
        with self.assertRaisesRegex(ValueError, 'Scaled'):
            shapes(fixture('SphereElems', dict(TM=matrix((2,0,0)), Radius=10)))
        with self.assertRaisesRegex(ValueError, 'radius'):
            shapes(fixture('SphereElems', dict(TM=matrix(), Radius=0)))

    def test_unsupported_is_explicit(self):
        # hulls() (used by prepare_level) still refuses what it cannot carry.
        with self.assertRaisesRegex(ValueError, 'Unsupported collision shape'):
            hulls(fixture('SphereElems', dict(TM=matrix(), Radius=10)))
        with self.assertRaisesRegex(ValueError, 'Unsupported collision shape'):
            hulls(fixture('MeshElems', dict(Radius=10)))
        bad = tags(AggGeom=[{'name': 'ConvexElems', 'status': 'unsupported', 'value': None}])
        with self.assertRaisesRegex(ValueError, 'Undecoded'): hulls(bad)



class Volumes(unittest.TestCase):
    DEFAULTS = {'bCollideActors': True, 'bBlockActors': True}

    def component(self):
        return {'_tags': fixture('ConvexElems', BOX, 'BrushAggGeom')}

    def test_blocking_volume_at_location(self):
        r = volume_record('Engine.BlockingVolume', {'Location': dict(X=10,Y=20,Z=30)}, self.component(), self.DEFAULTS)
        self.assertTrue(r['blocks_players'])
        self.assertFalse(r['dynamic'])
        self.assertEqual((r['location'], len(r['hulls'])), ([10,20,30], 1))

    def test_class_default_and_override_flags(self):
        # PathBlockingVolume's default bCollideActors=False: no brush component needed.
        r = volume_record('Engine.PathBlockingVolume', {'Location': dict(X=0,Y=0,Z=0)}, {'_tags': []},
                          {'bCollideActors': False, 'bBlockActors': True})
        self.assertEqual((r['blocks_players'], r['hulls']), (False, []))
        r = volume_record('Engine.BlockingVolume', {'Location': dict(X=0,Y=0,Z=0), 'bBlockActors': False},
                          self.component(), self.DEFAULTS)
        self.assertFalse(r['blocks_players'])

    def test_refuses_transforms_and_missing_geometry(self):
        for field in ('Rotation', 'DrawScale3D', 'PrePivot', 'DrawScale'):
            with self.assertRaisesRegex(ValueError, field):
                volume_record('Engine.BlockingVolume', {'Location': dict(X=0,Y=0,Z=0), field: 1},
                              self.component(), self.DEFAULTS)
        with self.assertRaisesRegex(ValueError, 'convex-only'):
            volume_record('Engine.BlockingVolume', {'Location': dict(X=0,Y=0,Z=0)}, {'_tags': []}, self.DEFAULTS)

    def test_dynamic_volume_needs_observation(self):
        r = volume_record('Engine.DynamicBlockingVolume', {'Location': dict(X=0,Y=0,Z=0)}, self.component(),
                          dict(self.DEFAULTS, bEnabled=True))
        self.assertTrue(r['dynamic'])
        volumes = [dict(r, status='dynamic_unobserved', level='L', source='TheWorld.PersistentLevel.D_0'),
                   dict(r, status='dynamic_unobserved', level='L', source='TheWorld.PersistentLevel.D_1')]
        apply_live_state(volumes, {'L:TheWorld.PersistentLevel.D_0': {'collide': False, 'block': True}})
        self.assertEqual([v['status'] for v in volumes], ['non_blocking', 'dynamic_unobserved'])

    def test_live_observations(self):
        dump = {'volumes': [{'path': 'L.TheWorld:PersistentLevel.B_0', 'bCollideActors': True, 'bBlockActors': True,
                             'component': {'CollideActors': True, 'BlockActors': True, 'BlockZeroExtent': False,
                                           'BlockNonZeroExtent': True}},
                            {'path': 'Default__BlockingVolume', 'bCollideActors': True}]}
        observed = live_observations(dump)
        self.assertEqual(list(observed), ['L:TheWorld.PersistentLevel.B_0'])
        r = volume_record('Engine.BlockingVolume', {'Location': dict(X=0,Y=0,Z=0)}, self.component(), self.DEFAULTS)
        volumes = [dict(r, status='blocking', level='L', source='TheWorld.PersistentLevel.B_0')]
        apply_live_state(volumes, observed)
        self.assertEqual((volumes[0]['status'], volumes[0]['live_state']['block_zero_extent']), ('blocking', False))
        # A static volume the running game shows as not colliding is kept out, not silently trusted.
        observed['L:TheWorld.PersistentLevel.B_0']['collide'] = False
        apply_live_state(volumes, observed)
        self.assertEqual(volumes[0]['status'], 'live_mismatch')


if __name__ == '__main__': unittest.main()
