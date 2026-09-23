"""Synthetic inspection-view generation checks; no game data or UE required."""
import importlib.util
from pathlib import Path
import unittest


spec = importlib.util.spec_from_file_location(
    'prepare_inspection_views', Path(__file__).parents[1] / 'tools/prepare_inspection_views.py')
views = importlib.util.module_from_spec(spec)
spec.loader.exec_module(views)


class InspectionViewTests(unittest.TestCase):
    def setUp(self):
        self.source = {'views': [
            {'name': 'spawn', 'location': [0, 0, 1000], 'rotation': [0, 0, 0], 'fov': 75},
            {'name': 'Synthetic_Terrain_10', 'location': [100, 200, 800],
             'rotation': [-35, 0, 0], 'fov': 75},
        ]}
        self.runtime = {'probes': [{
            'source': 'Synthetic:TheWorld.PersistentLevel.Terrain_10',
            'stands': [{'point': [1000, 2000, 900], 'surface': [700, 800]}],
        }]}

    def test_repair_preserves_baseline_and_adds_bounded_candidates(self):
        output = views.prepare_views(self.source, self.runtime, repair_obstructed=True)
        self.assertEqual(len(output['views']), 2)
        self.assertEqual(output['views'][0], self.source['views'][0])
        repaired = output['views'][1]
        self.assertEqual(repaired['target'], [1000, 2000, 800])
        self.assertEqual(repaired['target_actor_contains'], 'Terrain_10')
        self.assertEqual(repaired['candidate_policy'], views.OBSTRUCTION_POLICY)
        self.assertEqual(len(repaired['candidates']), 5)
        for candidate in repaired['candidates']:
            self.assertEqual(len(candidate['location']), 3)
            self.assertEqual(len(candidate['rotation']), 3)
            self.assertEqual(candidate['fov'], 75)

    def test_look_at_points_forward(self):
        self.assertAlmostEqual(views.look_at([0, 0, 0], [100, 0, 0])[0], 0)
        self.assertAlmostEqual(views.look_at([0, 0, 0], [0, 100, 0])[1], 90)
        self.assertAlmostEqual(views.look_at([0, 0, 0], [0, 0, 100])[0], 90)

    def test_missing_target_evidence_is_rejected(self):
        with self.assertRaisesRegex(ValueError, 'no terrain runtime stand'):
            views.prepare_views(self.source, {'probes': []}, repair_obstructed=True)


if __name__ == '__main__':
    unittest.main()
