"""Synthetic BSP calibration fixtures only; no installed-game captures."""
import json
import tempfile
import unittest
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
from calibrate_bsp_uv import analyze


SURFACE_A = {'level': 'Synthetic_P', 'model': 'Model_0', 'component': 'ModelComponent_0',
             'node': 1, 'material': 'Synthetic.MaterialA'}
SURFACE_B = {'level': 'Synthetic_P', 'model': 'Model_0', 'component': 'ModelComponent_1',
             'node': 2, 'material': 'Synthetic.MaterialB'}


def sample(name, surface, axis='v', distance=256.0, repeats=4.0,
           original_sign=-1, host_same=-1, host_flipped=1, group=None):
    return {
        'id': name,
        'surface': surface,
        'independence_group': group or name,
        'axis': axis,
        'axis_vector': [1.0, 0.0, 0.0] if axis == 'u' else [0.0, 1.0, 0.0],
        'anchors': {
            'a': {'world_cm': [0.0, 0.0, 0.0],
                  'pixel_original_game': [10.0, 20.0],
                  'pixel_ue5': [12.0, 22.0]},
            'b': {'world_cm': [distance, 0.0, 0.0] if axis == 'u' else [0.0, distance, 0.0],
                  'pixel_original_game': [110.0, 20.0] if axis == 'u' else [10.0, 120.0],
                  'pixel_ue5': [112.0, 22.0] if axis == 'u' else [12.0, 122.0]},
        },
        'captures': {'original_game': 'original.png', 'ue5': 'ue5.png'},
        'original_repeat_count': repeats,
        'original_progress_sign': original_sign,
        'host_progress_sign_by_orientation': {'same': host_same, 'flipped': host_flipped},
    }


class CalibrationTests(unittest.TestCase):
    def files(self):
        directory = tempfile.TemporaryDirectory()
        root = Path(directory.name)
        (root / 'original.png').write_bytes(b'synthetic original capture')
        (root / 'ue5.png').write_bytes(b'synthetic UE5 capture')
        self.addCleanup(directory.cleanup)
        return root

    def test_two_independent_measurements_verify_scale_and_orientation(self):
        root = self.files()
        data = {'schema': 1, 'provenance': {'source': 'synthetic'}, 'measurements': [
            sample('wall-u', SURFACE_A, group='wall'),
            sample('floor-u', SURFACE_B, group='floor'),
        ]}
        report = analyze(data, root, scales=[32.0, 64.0, 128.0])
        self.assertEqual(report['status'], 'VERIFIED')
        self.assertEqual(report['scale']['texel_scale'], 64.0)
        self.assertEqual(report['v_orientation']['v_orientation'], 'same')
        self.assertEqual(report['scale']['candidates'][1]['independent_count'], 2)

    def test_single_surface_does_not_verify(self):
        root = self.files()
        report = analyze({'schema': 1, 'measurements': [sample('one', SURFACE_A)]},
                         root, scales=[64.0])
        self.assertEqual(report['status'], 'UNVERIFIED')
        self.assertIsNone(report['scale']['texel_scale'])
        self.assertIn('Neither V orientation candidate is supported by two independent measurements or surfaces.',
                      report['missing_evidence'])

    def test_conflicting_scale_candidates_stay_unverified(self):
        root = self.files()
        first = sample('first', SURFACE_A, distance=256.0, repeats=4.0, group='first')
        second = sample('second', SURFACE_B, distance=128.0, repeats=4.0, group='second')
        report = analyze({'schema': 1, 'measurements': [first, second]}, root,
                         scales=[32.0, 64.0])
        self.assertEqual(report['scale']['status'], 'UNVERIFIED')
        self.assertIsNone(report['scale']['texel_scale'])
        self.assertIn('No texel-scale candidate is supported by two independent measurements or surfaces.',
                      report['missing_evidence'])

    def test_missing_capture_is_reported_and_not_counted(self):
        root = self.files()
        raw = sample('missing', SURFACE_A)
        raw['captures']['original_game'] = 'does-not-exist.png'
        report = analyze({'schema': 1, 'measurements': [raw]}, root, scales=[64.0])
        self.assertEqual(report['status'], 'UNVERIFIED')
        self.assertEqual(len(report['invalid_measurements']), 0)
        self.assertEqual(report['measurements'][0]['capture_status'], 'missing')
        self.assertIn('Two independent matched captures with original repeat counts are required for texel scale.',
                      report['missing_evidence'])

    def test_each_view_requires_its_own_pixel_anchors(self):
        for legacy in (False, True):
            with self.subTest(legacy=legacy):
                root = self.files()
                raw = sample('unmeasured-view', SURFACE_A)
                for anchor in raw['anchors'].values():
                    anchor.pop('pixel_original_game')
                    if legacy:
                        anchor.pop('pixel_ue5')
                        anchor['pixel'] = [10.0, 20.0]
                report = analyze({'schema': 1, 'measurements': [raw]}, root,
                                 scales=[64.0])
                self.assertEqual(report['measurements'], [])
                self.assertEqual(len(report['invalid_measurements']), 1)
                self.assertIn('pixel_original_game',
                              report['invalid_measurements'][0]['error'])

    def test_scene_inventory_is_preserved(self):
        root = self.files()
        scene = root / 'scene.json'
        scene.write_text(json.dumps({'meshes': {
            'BspMesh': {'bsp': {'model': 'Synthetic_P:Model_0'}, 'source': 'Synthetic_P:SyntheticComponent',
                        'sections': [{'slot': 0, 'material': 'mat', 'bsp_nodes': [1, 2]}]}
        }, 'materials': {'mat': {'source': 'Synthetic.TilingMaterials.Tile'}}}), encoding='utf-8')
        runtime = root / 'bsp-runtime.json'
        runtime.write_text(json.dumps({'models': [{'candidates': [{
            'level': 'Synthetic_P', 'source': 'SyntheticComponent', 'node': 1,
            'start': [0, 0, 0], 'end': [100, 0, 0], 'clearance': 10
        }]}]}), encoding='utf-8')
        report = analyze({'schema': 1, 'measurements': []}, root,
                         scene_path=scene, runtime_path=runtime)
        self.assertEqual(report['surface_inventory'][0]['source'], 'Synthetic_P:SyntheticComponent')
        self.assertEqual(report['surface_inventory'][0]['nodes'], [1, 2])
        self.assertEqual(report['tiled_candidates'][0]['material_source'], 'Synthetic.TilingMaterials.Tile')
        self.assertEqual(report['tiled_candidates'][0]['host_probe']['node'], 1)


if __name__ == '__main__':
    unittest.main()
