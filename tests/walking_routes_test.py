"""Synthetic checks for tools/walking_routes.py; no game data."""
import math
from pathlib import Path
import sys
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
from walking_routes import candidates, ground, placement_point, ramp_probe, summarize


def pose(location=(0, 0, 0), rotation=(0, 0, 0), scale=(1, 1, 1)):
    return {'location': list(location), 'rotation': list(rotation), 'scale': list(scale)}


IDENTITY = {'actor': pose(), 'component': pose()}
# A wedge rising 200 cm over 300 cm along local +X, 100 cm wide.
RAMP = [[0, -50, 0], [0, 50, 0], [300, -50, 0], [300, 50, 0], [300, -50, 200], [300, 50, 200]]


class Placement(unittest.TestCase):
    def test_actor_component_and_matrix_agree(self):
        # Yaw 90: local +X becomes world +Y; then the actor moves it.
        turned = {'actor': pose((10, 20, 30), (0, 90, 0)), 'component': pose(scale=(2, 2, 2))}
        p = placement_point(turned, [1, 0, 0])
        self.assertTrue(all(abs(a - b) < 1e-9 for a, b in zip(p, [10, 22, 30])))
        matrix = {'matrix': [0, 1, 0, 0, -1, 0, 0, 0, 0, 0, 1, 0, 10, 20, 30, 1], 'scale': [2, 2, 2]}
        q = placement_point(matrix, [1, 0, 0])
        self.assertTrue(all(abs(a - b) < 1e-9 for a, b in zip(q, p)))


class Ramps(unittest.TestCase):
    def test_probe_runs_low_to_high_outside_the_ramp(self):
        probe = ramp_probe(RAMP, IDENTITY)
        (sx, sy, sz), (ex, ey, ez) = probe['points']
        self.assertEqual((sx, sy, ex, ey), (-80, 0, 380, 0))
        self.assertEqual((sz, ez), (100, 300))
        self.assertEqual((probe['rise'], probe['run']), (200, 300))

    def test_reversed_ramp_and_flat_box(self):
        flipped = {'actor': pose(rotation=(0, 180, 0)), 'component': pose()}
        (sx, _, _), (ex, _, _) = ramp_probe(RAMP, flipped)['points']
        self.assertEqual((round(sx), round(ex)), (80, -380))
        box = [[x, y, z] for x in (0, 300) for y in (-50, 50) for z in (0, 10)]
        self.assertIsNone(ramp_probe(box, IDENTITY))

    def test_candidates_need_one_hull_and_enabled_collision(self):
        scene = {'meshes': {'m': {'source': 'P:Prop_Stairs.Meshes.Sanc_Stairs',
                                  'collision': {'hulls': [{'vertices': RAMP}]}},
                            'two': {'source': 'P:Prop_Stairs.Meshes.Sanc_Stairs',
                                    'collision': {'hulls': [{'vertices': RAMP}] * 2}}},
                 'actors': [{'mesh': 'm', 'transform': IDENTITY, 'collision_enabled': True, 'level': 'L', 'source': 'A'},
                            {'mesh': 'm', 'transform': IDENTITY, 'collision_enabled': False, 'level': 'L', 'source': 'B'},
                            {'mesh': 'two', 'transform': IDENTITY, 'collision_enabled': True, 'level': 'L', 'source': 'C'}]}
        found = candidates(scene)
        self.assertEqual([(c['name'], c['source']) for c in found], [('stair_00', 'L:A')])


class Summary(unittest.TestCase):
    def test_reached_and_extremes(self):
        rows = [{'t': 1.0, 'p': [0, 0, 190], 'feet': 100, 'phys': '1', 'floor': [0, 0, 1]},
                {'t': 1.5, 'p': [200, 0, 150], 'feet': 60, 'phys': '2', 'floor': [0, 0, 1]},
                {'t': 3.0, 'p': [370, 10, 390], 'feet': 300, 'phys': '1', 'floor': [0, 0, 1]}]
        s = summarize(rows, [[-80, 0, 100], [380, 0, 300]])
        self.assertTrue(s['reached'])
        # Row 1 is an initial drop; heights count from the landing at row 2.
        self.assertEqual((s['min_feet'], s['max_feet'], s['final_feet'], s['duration'], s['falling_samples']),
                         (300, 300, 300, 2.0, 1))
        self.assertAlmostEqual(s['landing_distance'], round(math.hypot(450, 10), 1))
        steady = [dict(r, phys='1') for r in rows]
        s = summarize(steady, [[-80, 0, 100], [380, 0, 300]])
        self.assertEqual((s['min_feet'], s['landing_distance']), (60, 80))
        self.assertFalse(summarize(rows[:2], [[-80, 0, 100], [380, 0, 300]])['reached'])

    def test_ground_under_centre_on_a_slope(self):
        # Walking on a 45 degree slope: the rim stands one radius above the surface under the centre.
        row = {'feet': 100, 'phys': '1', 'floor': [0, -math.sqrt(.5), math.sqrt(.5)]}
        self.assertAlmostEqual(ground(row, 42), 58)
        self.assertEqual(ground(dict(row, phys='2'), 42), 100)   # falling: no surface contact
        self.assertEqual(ground(dict(row, floor=None), 42), 100)


if __name__ == '__main__':
    unittest.main()
