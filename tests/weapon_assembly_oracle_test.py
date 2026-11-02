"""Synthetic tests for tools/weapon_assembly_oracle.py (no game data; the gun ids and numbers are invented)."""
import json
from pathlib import Path
import sys
import tempfile
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import weapon_assembly_oracle as O

LOG = """\
[2026.10.22-10.00.00] LogOpenWillow: unrelated noise line
OpenWillow weapon assembly GunA_01: type=Pistol fragments=2 (parts) missing=[] triangles=1200 skin=MI_Red material=/Game/Synth/MI_Red hidden=[] shown=[FragA,FragB] cold 3.10 ms
OpenWillow weapon assembly GunA_01: type=Pistol fragments=2 (parts) missing=[] triangles=1300 skin=MI_Red material=/Game/Synth/MI_Red hidden=[Clip] shown=[FragA,FragB] warm 0.40 ms
OpenWillow weapon assembly GunB_02: type=SMG fragments=4 (parts) missing=[FragQ,FragR] triangles=800 skin=None material=None
OpenWillow weapon assembly GunC_03: fallback to the pre-built mesh none (part P.Body.Unknown (Body) is not in the library)
OpenWillow weapon assembly GunE_05: type=Pistol fragments=2 (parts) missing=[] triangles=1300 skin=None material=None hidden=[] shown=[FragA,FragC] warm 0.2 ms
"""


def live(names, triangles):
    return {'gestalt_fields': {'GestaltData': {'PartMeshNames': names}}, 'triangles': triangles}


class HostReportsTests(unittest.TestCase):
    def setUp(self):
        self.reports = O.host_reports(LOG)

    def test_last_report_per_gun_wins(self):
        self.assertEqual(self.reports['GunA_01']['triangles'], 1300)

    def test_fields_are_parsed(self):
        report = self.reports['GunA_01']
        self.assertTrue(report['assembled'])
        self.assertEqual((report['type'], report['fragments'], report['skin'], report['material']),
                         ('Pistol', 2, 'MI_Red', '/Game/Synth/MI_Red'))
        self.assertEqual(report['shown'], ['FragA', 'FragB'])

    def test_missing_fragments_are_split(self):
        self.assertEqual(self.reports['GunB_02']['missing'], ['FragQ', 'FragR'])
        self.assertIsNone(self.reports['GunB_02']['shown'])

    def test_fallback_lines_are_reported_with_their_reason(self):
        report = self.reports['GunC_03']
        self.assertFalse(report['assembled'])
        self.assertIn('not in the library', report['reason'])

    def test_empty_log_gives_no_reports(self):
        self.assertEqual(O.host_reports(''), {})


class CompareTests(unittest.TestCase):
    def setUp(self):
        self.host = O.host_reports(LOG)

    def verdict(self, entry, gun='GunA_01', live_data=None):
        return O.compare({gun: entry}, self.host, live_data)[0]

    def test_pass_when_prediction_matches_names_and_triangles(self):
        row = self.verdict({'fragments': ['FragB', 'FragA'], 'expected_triangles': 1300})
        self.assertEqual(row['verdict'], 'pass')

    def test_pass_with_live_reference_too(self):
        row = self.verdict({'fragments': ['FragA', 'FragB'], 'expected_triangles': 1300},
                           live_data={'GunA_01': live(['FragA', 'FragB'], 1300)})
        self.assertEqual(row['verdict'], 'pass')
        self.assertTrue(row['live_checked'])

    def test_same_total_different_fragments_fails(self):
        # GunE_05 shows FragA+FragC with the same triangle total the reference gives for FragA+FragB.
        row = self.verdict({'fragments': ['FragA', 'FragB'], 'expected_triangles': 1300}, gun='GunE_05')
        self.assertEqual(row['verdict'], 'fail')
        self.assertTrue(any('fragments differ' in p for p in row['problems']))

    def test_live_names_disagreeing_fail(self):
        row = self.verdict({'fragments': ['FragA', 'FragB'], 'expected_triangles': 1300},
                           live_data={'GunA_01': live(['FragA', 'FragZ'], 1300)})
        self.assertEqual(row['verdict'], 'fail')

    def test_live_triangles_disagreeing_fail(self):
        row = self.verdict({'expected_triangles': 1300}, live_data={'GunA_01': live(None, 1299)})
        self.assertEqual(row['verdict'], 'fail')

    def test_prediction_triangles_disagreeing_fail(self):
        self.assertEqual(self.verdict({'expected_triangles': 1299})['verdict'], 'fail')

    def test_missing_fragments_fail(self):
        self.assertEqual(self.verdict({'expected_triangles': 800}, gun='GunB_02')['verdict'], 'fail')

    def test_fallback_is_its_own_verdict(self):
        row = self.verdict({'expected_triangles': 10}, gun='GunC_03')
        self.assertEqual(row['verdict'], 'fallback')

    def test_not_shown(self):
        self.assertEqual(self.verdict({'expected_triangles': 10}, gun='GunZ_99')['verdict'], 'not_shown')

    def test_no_reference_is_incomplete(self):
        self.assertEqual(self.verdict({})['verdict'], 'no_reference')

    def test_rows_are_sorted_by_id(self):
        rows = O.compare({'GunE_05': {}, 'GunA_01': {}}, self.host)
        self.assertEqual([r['id'] for r in rows], ['GunA_01', 'GunE_05'])


class MainExitTests(unittest.TestCase):
    def run_main(self, index, extra=()):
        root = Path(O.ROOT) / 'local' / 'tmp_oracle_test'
        root.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=root) as folder:
            folder = Path(folder)
            (folder / 'run.log').write_text(LOG, encoding='utf-8')
            (folder / 'index.json').write_text(json.dumps(index), encoding='utf-8')
            return O.main(['--log', str(folder / 'run.log'), '--index', str(folder / 'index.json'),
                           '--output', str(folder / 'oracle.json'), *extra])

    def test_all_pass_exits_zero(self):
        self.assertEqual(self.run_main({'GunA_01': {'fragments': ['FragA', 'FragB'], 'expected_triangles': 1300}}), 0)

    def test_empty_index_exits_one(self):
        self.assertEqual(self.run_main({}), 1)

    def test_expected_count_is_enforced(self):
        index = {'GunA_01': {'fragments': ['FragA', 'FragB'], 'expected_triangles': 1300}}
        self.assertEqual(self.run_main(index, ['--expect', '1']), 0)
        self.assertEqual(self.run_main(index, ['--expect', '2']), 1)

    def test_any_non_pass_exits_one(self):
        self.assertEqual(self.run_main({'GunA_01': {'fragments': ['FragA', 'FragB'], 'expected_triangles': 1300},
                                        'GunA_02': {}}), 1)
        self.assertEqual(self.run_main({'GunA_01': {}}), 1)  # no_reference alone is not a pass


if __name__ == '__main__':
    unittest.main()
