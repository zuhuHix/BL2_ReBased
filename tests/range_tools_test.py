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
        out = prepare_range.spot_candidates(terrain, {'x': 0.0, 'y': 0.0})
        self.assertEqual([c['x'] for c in out], [-1000.0, -5000.0, 0.0])   # gate terrain first, each group nearest first
        self.assertTrue(out[0]['gate_terrain'])
        self.assertFalse(out[2]['gate_terrain'])
        self.assertEqual(out[2]['plaza_distance'], 1000.0)

    def test_output_must_be_inside_local(self):
        import os
        import tempfile
        with tempfile.TemporaryDirectory() as repo:
            os.makedirs(os.path.join(repo, 'local', 'range'))
            inside = os.path.join(repo, 'local', 'range', 'm.json')
            self.assertEqual(prepare_range.local_output(inside, repo), os.path.realpath(inside))
            for outside in (os.path.join(repo, 'docs', 'm.json'), os.path.join(repo, 'local'),
                            os.path.join(repo, 'local', '..', 'm.json'), os.path.join(repo, 'localx', 'm.json')):
                with self.assertRaises(SystemExit):
                    prepare_range.local_output(outside, repo)
    def test_loot_tables_from_kill_steps(self):
        run = {'pawns': [{'archetype': 'Pkg/Made.Character.Pawn_Beast', 'level': 4}],
               'steps': [{'step': 'spawn:x:4', 'result': {}},
                         {'step': 'hit:0:9:T:S', 'result': {'loot': {'item_stage': 4, 'pools': [
                             {'pool': 'Made.Pool_GunsAndGear', 'chance': 0.1, 'roll': 0.5},
                             {'pool': 'Made.Pool_Money_1or2', 'chance': 0.25},
                             {'pool': 'Made.Pool_Skins', 'chance': 0.05},
                             {'pool': 'Made.AmmoAndResourcePools.Pool_Eridium_Bar', 'chance': 0.01}]}}}]}
        table = prepare_range.loot_tables(run)
        self.assertEqual(table['Beast'][4]['item_stage'], 4)
        self.assertEqual([p['kind'] for p in table['Beast'][4]['pools']], ['weapon', 'money', 'other', 'other'])
        self.assertNotIn('roll', table['Beast'][4]['pools'][0])   # the host rolls its own draw

    def test_stage_range_from_sample_kills(self):
        def kill(index, stage):
            return {'step': 'hit:%d:9:T:S' % index, 'result': {'loot': {'item_stage': stage, 'pools': []}}}
        run = {'pawns': [{'archetype': 'P/M.Pawn_Beast', 'level': 3}] + [{'archetype': 'P/M.Pawn_Beast', 'level': 20}] * 3,
               'steps': [kill(0, 3), kill(1, 18), kill(2, 21), kill(3, 20)]}
        self.assertEqual(prepare_range.stage_offsets(run), {'Beast': (-2, 1)})
        row = prepare_range.loot_tables(run)['Beast'][3]
        self.assertEqual((row['item_stage'], row['item_stage_min'], row['item_stage_max']), (3, 1, 4))   # 3-2 = 1, 3+1 = 4
        self.assertEqual(prepare_range.loot_tables(run)['Beast'][20]['item_stage'], 18)   # the first kill of a level is its row

    def test_scout_centre_from_axis_views(self):
        views = {'views': [
            {'name': 'plaza_a', 'location': [10.0, 500.0, 1.0], 'rotation': [5.0, 90.0, 0.0]},       # looks along Y: gives X
            {'name': 'plaza_b', 'location': [900.0, 20.0, 1.0], 'rotation': [5.0, 0.0, 0.0]},        # looks along X: gives Y
            {'name': 'plaza_c', 'location': [700.0, 40.0, 1.0], 'rotation': [5.0, 180.0, 0.0]},      # looks along X: gives Y
            {'name': 'plaza_d', 'location': [1.0, 1.0, 1.0], 'rotation': [5.0, -114.0, 0.0]},        # diagonal: not used
            {'name': 'other', 'location': [9.0, 9.0, 1.0], 'rotation': [5.0, 90.0, 0.0]}]}           # not a plaza view
        self.assertEqual(prepare_range.scout_centre(views), {'x': 10.0, 'y': 30.0})
        self.assertIsNone(prepare_range.scout_centre({'views': views['views'][:1]}))

    def test_spot_data_is_required(self):
        import os
        import tempfile
        with tempfile.TemporaryDirectory() as folder:
            missing = os.path.join(folder, 'none.json')
            out = os.path.join(prepare_range.REPO, 'local', 'range', 'test_should_not_exist.json')
            with self.assertRaises(SystemExit):
                prepare_range.main(['--no-vm', '--terrain', missing, '--views', missing, '--out', out])
            self.assertFalse(os.path.exists(out))

    def test_spot_data_message_names_the_plaza_views(self):
        import json
        import os
        import tempfile
        with tempfile.TemporaryDirectory() as folder:
            views = os.path.join(folder, 'views.json')
            with open(views, 'w', encoding='utf-8') as f:
                json.dump({'views': [{'name': 'other', 'location': [1, 2, 3], 'rotation': [0, 90, 0]}]}, f)
            out = os.path.join(prepare_range.REPO, 'local', 'range', 'test_should_not_exist.json')
            with self.assertRaises(SystemExit) as stop:
                prepare_range.main(['--no-vm', '--views', views, '--terrain', views, '--out', out])
            self.assertIn('plaza_*', str(stop.exception))
            self.assertIn('--views', str(stop.exception))
            self.assertFalse(os.path.exists(out))

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


class PreflightTest(unittest.TestCase):
    def run_preflight(self, manifest, **kwargs):
        import json
        import os
        import tempfile
        with tempfile.TemporaryDirectory() as folder:
            path = os.path.join(folder, 'manifest.json')
            with open(path, 'w', encoding='utf-8') as f:
                json.dump(manifest, f)
            spot = kwargs.pop('spot', None)
            spot_path = None
            if spot is not None:
                spot_path = os.path.join(folder, 'range.json')
                with open(spot_path, 'w', encoding='utf-8') as f:
                    json.dump(spot, f)
            return check_range.preflight(path, spot_path, **kwargs)

    GOOD = {'spot_candidates': [{'x': 1}, {'x': 2}], 'default_spot_candidate': 1, 'scout_centre': {'x': 0, 'y': 0}}

    def test_good_manifest(self):
        self.assertEqual(self.run_preflight(self.GOOD), '')

    def test_old_manifest_stops_the_run(self):
        self.assertIn('default_spot_candidate', self.run_preflight({'spot_candidates': [{'x': 1}]}))

    def test_index_outside_or_none_stops_the_run(self):
        for index in (-1, 2, None):
            self.assertNotEqual(self.run_preflight(dict(self.GOOD, default_spot_candidate=index)), '')

    def test_no_scout_centre_stops_the_run(self):
        self.assertNotEqual(self.run_preflight(dict(self.GOOD, scout_centre=None)), '')

    def test_explicit_spot_is_allowed(self):
        self.assertEqual(self.run_preflight({}, has_at=True), '')
        self.assertEqual(self.run_preflight({}, spot={'spot': {'x': 1, 'y': 2, 'z': 3}}), '')

    def test_broken_spot_json_does_not_count(self):
        self.assertNotEqual(self.run_preflight({}, spot={'spot': {'x': 1}}), '')
        self.assertNotEqual(self.run_preflight({}, spot=[1, 2]), '')

    def test_survey_needs_only_candidates(self):
        self.assertEqual(self.run_preflight({'spot_candidates': [{'x': 1}]}, survey=True), '')
        self.assertNotEqual(self.run_preflight({}, survey=True), '')

    def test_unreadable_manifest(self):
        self.assertIn('cannot read', check_range.preflight('does/not/exist.json'))


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

    def test_step_status_needs_a_boundary(self):
        text = log(*['OWRANGETEST step=%s result=%s x=1' % (s, 'SKIP_typo' if s == 'ragdoll' else 'PASS') for s in check_range.EXPECTED],
                   'OWRANGETEST SUMMARY result=PASS')
        ok, rows = check_range.grade(*check_range.parse(text))
        self.assertFalse(ok)
        self.assertIn(('ragdoll', 'MISSING', ''), rows)

    def test_summary_status_needs_a_boundary(self):
        steps, summary = self.steps(summary='PASSED')
        self.assertIsNone(summary)
        self.assertFalse(check_range.grade(steps, summary)[0])

    def test_status_at_the_end_of_a_line_counts(self):
        steps, summary = check_range.parse(log('OWRANGETEST step=teleport result=PASS', 'OWRANGETEST SUMMARY result=PASS'))
        self.assertEqual(steps['teleport'], [('PASS', '')])
        self.assertEqual(summary, ('PASS', ''))

    def crit_skip(self, reason):
        lines = ['OWRANGETEST step=%s result=%s' % (s, ('SKIP ' + reason) if s == 'crit_minds_eye' else 'PASS') for s in check_range.EXPECTED]
        return check_range.grade(*check_range.parse(log(*lines, 'OWRANGETEST SUMMARY result=PASS')))

    def test_crit_minds_eye_skips_only_without_the_file(self):
        ok, rows = self.crit_skip('reason=no local skill effects file')
        self.assertTrue(ok)
        self.assertIn(('crit_minds_eye', 'SKIP', 'reason=no local skill effects file'), rows)
        self.assertFalse(self.crit_skip('reason=no walker or skill tree')[0])
        self.assertFalse(self.crit_skip('')[0])

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
