"""Synthetic checks for local-only capture recipe staging."""
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import weapon_capture_items as capture


class WeaponCaptureItemsTests(unittest.TestCase):
    def test_mission_fixture_is_admitted_without_mutating_source(self):
        with tempfile.TemporaryDirectory(dir=capture.ROOT / 'local') as temp:
            root = Path(temp)
            source = root / 'source'
            destination = root / 'output'
            source.mkdir()
            original = {'name': 'Synthetic Mission Item',
                        'provenance': {'kind': 'mission_weapon', 'mission': 'Synthetic.Mission'}}
            (source / 'synthetic.json').write_text(json.dumps(original), encoding='utf-8')
            (source / 'weapon_view.json').write_text('{"synthetic":{}}', encoding='utf-8')

            outputs = capture.stage_capture_items(source, destination, ['synthetic'])

            staged = json.loads((destination / 'synthetic.json').read_text(encoding='utf-8'))
            untouched = json.loads((source / 'synthetic.json').read_text(encoding='utf-8'))
            self.assertEqual(staged['provenance']['kind'], 'host_capture_fixture')
            self.assertEqual(untouched['provenance']['kind'], 'mission_weapon')
            self.assertEqual(len(outputs), 2)
            self.assertEqual((destination / 'load_order.txt').read_text(encoding='utf-8'), 'synthetic\n')

    def test_smg_capture_fixture_uses_host_ammo_category_without_mutating_source(self):
        with tempfile.TemporaryDirectory(dir=capture.ROOT / 'local') as temp:
            root = Path(temp)
            source = root / 'source'
            destination = root / 'output'
            source.mkdir()
            original = {'name': 'Synthetic SMG', 'type': 'Sub-Machine Gun',
                        'provenance': {'kind': 'generated'}}
            (source / 'slice_smg.json').write_text(json.dumps(original), encoding='utf-8')

            capture.stage_capture_items(source, destination, ['slice_smg'])

            staged = json.loads((destination / 'slice_smg.json').read_text(encoding='utf-8'))
            untouched = json.loads((source / 'slice_smg.json').read_text(encoding='utf-8'))
            self.assertEqual(staged['type'], 'SMG')
            self.assertEqual(untouched['type'], 'Sub-Machine Gun')

    def test_output_outside_local_is_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            with self.assertRaisesRegex(ValueError, 'inside ignored local'):
                capture.stage_capture_items(temp, Path(temp) / 'out', ['synthetic'])


if __name__ == '__main__':
    unittest.main()
