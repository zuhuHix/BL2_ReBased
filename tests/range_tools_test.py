"""Synthetic tests for tools/prepare_range.py and tools/check_range.py: invented rows, names and logs, no game data.

Run: python tests/range_tools_test.py
"""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import check_range  # noqa: E402
import prepare_range  # noqa: E402


class PrepareRangeTest(unittest.TestCase):
    def test_levels(self):
        self.assertEqual(prepare_range.parse_levels('3'), [3])
        self.assertEqual(prepare_range.parse_levels('2-4'), [2, 3, 4])
        with self.assertRaises(ValueError):
            prepare_range.parse_levels('5-2')

    def test_health_table_takes_only_its_class(self):
        run = {'rows': {'max_health.Beast.L2': 20.5, 'max_health.Beast.L10': 99.0, 'health.Beast.L2': 1.0,
                        'max_health.BeastBig.L2': 7.0, 'max_health.Beast.Lx': 3.0}}
        self.assertEqual(prepare_range.health_table(run, 'Beast'), {2: 20.5, 10: 99.0})

    def test_melee_rows(self):
        run = {'rows': {'arrival.Beast.L3': 9.5, 'ratio.Beast.L3': 0.7, 'max_health.Beast.L3': 50.0}}
        self.assertEqual(prepare_range.health_table(run, 'Beast', 'arrival'), {3: 9.5})
        self.assertEqual(prepare_range.health_table(run, 'Beast', 'ratio'), {3: 0.7})
        entry = prepare_range.kind_entry('B', None, {3: 50.0}, 't', arrival={3: 9.5}, ratio={3: 0.7})
        self.assertEqual(entry['melee_arrival_by_level'], {'3': 9.5})
        self.assertEqual(entry['melee_ratio_by_level'], {'3': 0.7})

    def test_spot_candidates_gate_first(self):
        terrain = {'probes': [
            {'source': 'Made:Level.Terrain_9', 'stands': [{'point': [0.0, 1000.0, 10.0]}, {'point': [1.0]}]},
            {'source': prepare_range.GATE_TERRAIN, 'stands': [{'point': [-5000.0, 640.0, 20.0]}, {'point': [-1000.0, 640.0, 30.0]}]}]}
        out = prepare_range.spot_candidates(terrain)
        self.assertEqual([c['x'] for c in out], [-1000.0, -5000.0, 0.0])   # gate terrain first, each group nearest first
        self.assertTrue(out[0]['gate_terrain'])
        self.assertFalse(out[2]['gate_terrain'])
        self.assertEqual(out[2]['plaza_distance'], 1000.0)

    def test_clip_pick_by_role(self):
        names = ['Idle_Made', 'Walk_F_Made', 'Run_Made', 'Flinch_Front', 'Melee_Swipe', 'PhaseLock_Walk', 'Death_Made']
        clips = prepare_range.find_clips('/Game/X/Anims/Idle_Made.Idle_Made', names)
        self.assertEqual(clips['walk'], '/Game/X/Anims/Walk_F_Made.Walk_F_Made')
        self.assertEqual(clips['run'], '/Game/X/Anims/Run_Made.Run_Made')
        self.assertEqual(clips['hit'], '/Game/X/Anims/Flinch_Front.Flinch_Front')
        self.assertEqual(clips['melee'], '/Game/X/Anims/Melee_Swipe.Melee_Swipe')

    def test_clip_missing_is_empty(self):
        clips = prepare_range.find_clips('/Game/X/Idle.Idle', ['Idle'])
        self.assertEqual(set(clips.values()), {''})

    def test_kind_entry_without_assets(self):
        entry = prepare_range.kind_entry('Made Beast', None, {1: 10.0}, 'test')
        self.assertEqual(entry['mesh'], '')
        self.assertEqual(entry['health_by_level'], {'1': 10.0})


def log(*lines):
    return '\n'.join('[t] LogTemp: Display: ' + line for line in lines)


class CheckRangeTest(unittest.TestCase):
    def steps(self, overrides=None, summary='PASS'):
        overrides = overrides or {}
        lines = ['OWRANGETEST step=%s result=%s x=1' % (s, overrides.get(s, 'PASS')) for s in check_range.EXPECTED
                 if overrides.get(s) != 'DROP']
        lines.append('OWRANGETEST SUMMARY result=%s pass=1 fail=0 skip=0' % summary)
        return check_range.parse(log(*lines))

    def test_all_pass(self):
        ok, rows = check_range.grade(*self.steps())
        self.assertTrue(ok)
        self.assertEqual({r for _, r, _ in rows}, {'PASS'})

    def test_fail_fails(self):
        ok, _ = check_range.grade(*self.steps({'crit_hit': 'FAIL'}, summary='FAIL'))
        self.assertFalse(ok)

    def test_missing_step_fails(self):
        ok, rows = check_range.grade(*self.steps({'respawn': 'DROP'}))
        self.assertFalse(ok)
        self.assertIn(('respawn', 'MISSING', ''), rows)

    def test_skip_only_where_allowed(self):
        self.assertTrue(check_range.grade(*self.steps({'ragdoll': 'SKIP'}))[0])
        self.assertFalse(check_range.grade(*self.steps({'death': 'SKIP'}))[0])

    def test_no_summary_fails(self):
        steps, _ = self.steps()
        self.assertFalse(check_range.grade(steps, None)[0])

    def test_repeated_step_fail_wins(self):
        text = log('OWRANGETEST step=teleport result=PASS', 'OWRANGETEST step=teleport result=FAIL d=900',
                   'OWRANGETEST SUMMARY result=FAIL')
        ok, rows = check_range.grade(*check_range.parse(text), expected=['teleport'])
        self.assertFalse(ok)
        self.assertEqual(rows[0][:2], ('teleport', 'FAIL'))


if __name__ == '__main__':
    unittest.main()
