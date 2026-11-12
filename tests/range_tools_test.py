"""Synthetic tests for tools/prepare_range.py and tools/check_range.py: invented rows, names and logs, no game data.

Run: python tests/range_tools_test.py
"""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import check_range  # noqa: E402
import prepare_range  # noqa: E402


# A kind_melee PASS line must carry numbers inside the acceptance bands (tools/check_range.py); this one does (invented, adult).
KIND_MELEE_OK = ('kind=bullymong_adult enemy=E_3 cycle_min=1.250 cycle_max=1.267 stand_min=201 stand_max=201 first_swing=380 start_range=394 '
                 'max_frame=0.017 swings=4')
SPAWN_ADULT = 'OWRANGE enemy E_3 kind_melee kind=bullymong_adult swing_interval=1.250 stand=201 source=live_read'


def passing(step):
    # kind_melee's result is tied to a spawn record of the same log, so its line comes with one (invented names).
    if step == 'kind_melee':
        return SPAWN_ADULT + '\nOWRANGETEST step=kind_melee result=PASS ' + KIND_MELEE_OK
    return 'OWRANGETEST step=%s result=PASS x=1' % step


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
    def test_relative_paths_resolve_from_the_repository_not_the_folder(self):
        import os
        import tempfile
        from unittest import mock
        seen = {}

        def fake_vm(reader, levels):
            seen['reader'] = reader
            return {}

        out = os.path.join(prepare_range.REPO, 'local', 'range', 'test_relative_manifest.json')
        self.addCleanup(lambda: os.path.exists(out) and os.remove(out))
        here = os.getcwd()
        with tempfile.TemporaryDirectory() as folder:
            os.chdir(folder)
            try:
                with mock.patch.object(prepare_range, 'run_vm', fake_vm), mock.patch.object(prepare_range, 'start_ranges', lambda reader: {}):
                    prepare_range.main(['--levels', '1', '--reader', os.path.join('build', 'Release', 'ow-package.exe'),
                                        '--allow-missing-spot-data', '--views', os.path.join('local', 'none.json'),
                                        '--terrain', os.path.join('local', 'none.json'), '--target', os.path.join('local', 'none.json'),
                                        '--out', os.path.join('local', 'range', 'test_relative_manifest.json')])
            finally:
                os.chdir(here)
        self.assertEqual(seen['reader'], os.path.join(prepare_range.REPO, 'build', 'Release', 'ow-package.exe'))
        self.assertTrue(os.path.isfile(out))
        absolute = os.path.abspath(os.path.join(here, 'elsewhere', 'ow-package.exe'))
        self.assertEqual(prepare_range.from_repo(absolute), absolute)   # an absolute path is kept as given

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

    def test_main_writes_a_manifest(self):
        import json
        import os
        import tempfile
        views = {'views': [{'name': 'plaza_a', 'location': [10.0, 500.0, 1.0], 'rotation': [5.0, 90.0, 0.0]},
                           {'name': 'plaza_b', 'location': [900.0, 20.0, 1.0], 'rotation': [5.0, 0.0, 0.0]}]}
        with tempfile.TemporaryDirectory() as folder:
            views_path = os.path.join(folder, 'views.json')
            with open(views_path, 'w', encoding='utf-8') as f:
                json.dump(views, f)
            out = os.path.join(prepare_range.REPO, 'local', 'range', 'test_main_manifest.json')
            self.addCleanup(lambda: os.path.exists(out) and os.remove(out))
            none = os.path.join(folder, 'none.json')
            prepare_range.main(['--no-vm', '--allow-missing-spot-data', '--views', views_path, '--terrain', none, '--target', none, '--out', out])
            with open(out, encoding='utf-8') as f:
                manifest = json.load(f)
            self.assertTrue(manifest['assets'].startswith('missing'))
            target = os.path.join(folder, 'target.json')
            with open(target, 'w', encoding='utf-8') as f:
                json.dump({'Bullymong': {'mesh': '/Game/M.M', 'idle': '', 'death': '', 'collision_radius': 130.0, 'collision_height': 70.0}}, f)
            prepare_range.main(['--no-vm', '--allow-missing-spot-data', '--views', views_path, '--terrain', none, '--target', target, '--out', out])
            with open(out, encoding='utf-8') as f:
                manifest = json.load(f)
            self.assertEqual(manifest['assets'], 'imported')
            # The baby borrows the asset but keeps its own cylinder; the kinds that own the asset's cylinder keep the asset's.
            baby, adult = manifest['kinds']['bullymong_baby'], manifest['kinds']['bullymong_adult']
            self.assertEqual((baby['mesh'], baby['collision_radius'], baby['collision_height']), ('/Game/M.M', 64.0, 48.0))
            self.assertEqual((adult['collision_radius'], adult['collision_height']), (130.0, 70.0))
            self.assertEqual((adult['run_speed'], baby['run_speed'], manifest['kinds']['bullymong']['run_speed']), (500.0, 300.0, 400.0))

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

    def test_spawn_step_named_and_by_path(self):
        self.assertEqual(prepare_range.spawn_step('regular', 4), 'spawn:regular:4')
        path = 'Ice_Dynamic/GD_Psycho.Character.Pawn_Psycho'
        self.assertEqual(prepare_range.spawn_step(path, 4), 'spawn:%s:4:4:%s' % (path, prepare_range.BALANCES[path]))

    def test_every_kind_has_a_swing(self):
        for vm_kind, _ in prepare_range.KINDS.values():
            self.assertIn(vm_kind, prepare_range.SWINGS)

    def test_melee_status_labels_vm_only_kinds(self):
        self.assertIn('live rows', prepare_range.melee_status('bullymong'))
        self.assertIn('UNVERIFIED', prepare_range.melee_status('psycho'))

    def test_every_kind_names_its_asset_and_the_psycho_does_not_borrow(self):
        self.assertEqual(set(prepare_range.KIND_ASSETS), set(prepare_range.KINDS))
        self.assertEqual(prepare_range.KIND_ASSETS['psycho'], 'Psycho')

    def test_psycho_spawns_only_with_its_own_mesh_and_last(self):
        without = prepare_range.spawns_for({'psycho': {'mesh': ''}})
        self.assertEqual(without, prepare_range.SPAWNS)
        with_mesh = prepare_range.spawns_for({'psycho': {'mesh': '/Game/Made.Made'}})
        self.assertEqual(with_mesh[:len(prepare_range.SPAWNS)], prepare_range.SPAWNS)
        self.assertEqual(with_mesh[-1]['kind'], 'psycho')

    def test_batches_fit_the_command_line_and_keep_every_kind(self):
        levels = list(range(1, 51))
        batches = prepare_range.step_batches(levels, 200)
        self.assertEqual(sorted(k for b in batches for k in b), sorted(v for v, _ in prepare_range.KINDS.values()))
        for batch in batches:
            steps, first = [], 0
            for kind in batch:
                steps += prepare_range.kind_steps(kind, levels, first)
                first = sum(1 for s in steps if s.startswith('spawn:'))
            self.assertLess(len(' '.join(steps)) + 200, 32767)

    def test_kind_steps_count_pawns_from_first(self):
        steps = prepare_range.kind_steps('regular', [1, 2], 5)
        self.assertIn('swing:5:melee', steps)
        self.assertIn('swing:6:melee', steps)
        self.assertEqual(steps.count('hit:7:999999:' + prepare_range.LETHAL), 1)

    def test_merge_runs_moves_the_hit_indices(self):
        one = {'rows': {'a': 1}, 'pawns': [{'level': 1}], 'steps': [{'step': 'hit:0:9:T:S', 'result': {}}]}
        two = {'rows': {'b': 2}, 'pawns': [{'level': 2}], 'steps': [{'step': 'hit:0:9:T:S', 'result': {}}]}
        merged = prepare_range.merge_runs([one, two])
        self.assertEqual(merged['rows'], {'a': 1, 'b': 2})
        self.assertEqual([s['step'] for s in merged['steps']], ['hit:0:9:T:S', 'hit:1:9:T:S'])
        self.assertEqual(len(merged['pawns']), 2)

    def test_swing_range_is_the_farthest_plain_melee(self):
        actions = [('Made_Throw', 3000.0), ('Made_Melee', 320.0), ('MadeMeleeTwo', 394.0), ('Made_SlagMelee', 700.0), ('Made_Melee', 0.0)]
        self.assertEqual(prepare_range.melee_start_range(actions), 394.0)
        self.assertEqual(prepare_range.melee_start_range([('Made_Throw', 3000.0)]), 0.0)

    def test_attack_actions_read_through_the_nodes(self):
        objects = {'Def': {'nodelist': [{'Action': 'Def.Action_AnimAttack_1'}, {'Action': 'Def.Action_AnimAttack_1'}, {'Action': 'Def.Action_Patrol_1'},
                                        {'Action': None}, {'Action': 'Def.Action_AnimAttack_2'}]},
                   'Def.Action_AnimAttack_1': {'attackanim': 'Pkg.Anims.Made_Melee', 'limits': ['Def.A1.AttackLocation_Range_0', 'Def.A1.AttackLocation_Height_0']},
                   'Def.A1.AttackLocation_Range_0': {'range': {'MinVal': 0, 'MaxVal': 111}},
                   'Def.A1.AttackLocation_Height_0': {'height': 5},
                   'Def.Action_AnimAttack_2': {'attackanim': 'Pkg.Anims.Made_Throw', 'limits': ['Def.A2.RangeLimit']},
                   'Def.A2.RangeLimit': {'range': {'MinVal': 9, 'MaxVal': 222}}}
        self.assertEqual(prepare_range.read_attack_actions('Def', objects.__getitem__), [('Made_Melee', 111.0), ('Made_Throw', 222.0)])

    def test_every_kind_has_its_ai_definition(self):
        self.assertEqual(set(prepare_range.AI_DEFINITIONS), set(prepare_range.KINDS))

    def test_kind_carries_what_the_pawn_wears(self):
        worn = [{'mesh': '/Game/Made.Made', 'bone': 'Made', 'location': [0, 0, 0], 'quat': [0, 0, 0, 1], 'scale': [1, 1, 1]}]
        target = {'mesh': '/Game/M.M', 'idle': '/Game/I.I', 'death': '/Game/D.D', 'attachments': worn}
        self.assertEqual(prepare_range.kind_entry('B', target, {}, 't', listing=[])['attachments'], worn)
        del target['attachments']
        self.assertNotIn('attachments', prepare_range.kind_entry('B', target, {}, 't', listing=[]))

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

    def test_the_baby_has_its_own_cylinder_not_the_borrowed_one(self):
        # Live read 2026-11-12: radius 64, half height 48 (every baby pawn read). Going back to the adult's 150 breaks its stand row.
        self.assertEqual(prepare_range.OBSERVED_COLLISION, {'bullymong_baby': (64.0, 48.0)})
        entry = prepare_range.apply_observed('bullymong_baby', {'collision_radius': 150.0, 'collision_height': 90.0})
        self.assertEqual((entry['collision_radius'], entry['collision_height']), (64.0, 48.0))
        self.assertIn('live read', entry['collision_source'])
        untouched = prepare_range.apply_observed('bullymong', {'collision_radius': 150.0, 'collision_height': 90.0})
        self.assertEqual(untouched, {'collision_radius': 150.0, 'collision_height': 90.0})

    def test_run_and_walk_per_kind_and_the_regular_keeps_the_stand_in(self):
        # Live read 2026-11-12: GroundSpeed 500 (adult, slinger) and 300 (baby), WalkingPct 0.25. The regular's chase read 375, unexplained.
        speeds = {'bullymong_adult': (500.0, 125.0), 'bullymong_slinger': (500.0, 125.0), 'bullymong_baby': (300.0, 75.0)}
        self.assertEqual(prepare_range.OBSERVED_SPEED, speeds)
        for kind, (run, walk) in speeds.items():
            with self.subTest(kind=kind):
                entry = prepare_range.apply_observed(kind, prepare_range.kind_entry('Made', None, {}, 'test'))
                self.assertEqual((entry['run_speed'], entry['walk_speed']), (run, walk))
                self.assertEqual(walk, run * 0.25)
        for kind in ('bullymong', 'psycho'):
            with self.subTest(kind=kind):
                entry = prepare_range.apply_observed(kind, prepare_range.kind_entry('Made', None, {}, 'test'))
                self.assertEqual((entry['run_speed'], entry['walk_speed']), (400.0, 150.0))
                self.assertNotIn('speed_source', entry)

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

    def test_a_spawned_kind_must_be_able_to_swing(self):
        whole = {'melee_arrival_by_level': {'1': 2.0}, 'melee_ratio_by_level': {'1': 0.5}, 'melee_start_range': 111.0}
        check = lambda kind: self.run_preflight({'spawns': [{'kind': 'made'}], 'kinds': {'made': kind, 'unused': {}}}, has_at=True)
        self.assertEqual(check(whole), '')
        for missing in whole:
            self.assertIn('made', check({k: v for k, v in whole.items() if k != missing}), missing)
        self.assertIn('made', check(dict(whole, melee_start_range=0.0)))
        self.assertIn('made', check(dict(whole, melee_start_range=-1.0)))
        self.assertIn('made', check({}))   # a --no-vm manifest: its enemies would approach and never attack
        self.assertIn('nokind', self.run_preflight({'spawns': [{'kind': 'nokind'}], 'kinds': {'made': whole}}, has_at=True))

    def test_a_survey_needs_an_accepted_candidate(self):
        self.assertTrue(check_range.survey_ok(log('OWRANGESURVEY done candidates=8 accepted=1')))
        self.assertTrue(check_range.survey_ok(log('OWRANGESURVEY done candidates=8 accepted=12')))
        self.assertFalse(check_range.survey_ok(log('OWRANGESURVEY done candidates=8 accepted=0')))
        self.assertFalse(check_range.survey_ok(log('OWRANGESURVEY done candidates=8')))
        self.assertFalse(check_range.survey_ok(log('OWRANGESURVEY index=0 accepted=1 at=1,2,3')))

    def test_a_survey_keeps_the_manifest_rule(self):
        self.assertIn('made', self.run_preflight({'spot_candidates': [{'x': 1}], 'spawns': [{'kind': 'made'}], 'kinds': {'made': {}}}, survey=True))

    def test_the_kill_row_needs_each_gun_to_have_fired(self):
        good = lambda gun, shots=3, result='PASS': 'OWRANGEKILL gun=%d result=%s "Made" (pistol, level 1): %d shots, rose 1 uu, drifted 1 uu, fastest body 1 uu/s (Made_Bone, 0.10 s in), 0 speed caps, at rest 1 to 9 uu above the floor, measured 1' % (gun, result, shots)
        done = 'OWRANGEKILL done guns=2 fail=0'
        self.assertTrue(check_range.kills_ok(log(good(0), good(1), done))[0])
        self.assertFalse(check_range.kills_ok(log(good(0), good(1)))[0])                     # never finished
        self.assertFalse(check_range.kills_ok(log(good(0), done))[0])                        # a total alone is not each gun
        self.assertFalse(check_range.kills_ok(log(good(0), good(1, shots=0), done))[0])      # a gun that never fired
        self.assertFalse(check_range.kills_ok(log(good(0), good(1, result='FAIL'), done))[0])
        self.assertFalse(check_range.kills_ok(log(good(0), good(1), 'OWRANGEKILL done guns=2 fail=1'))[0])

    def test_a_psycho_skip_needs_the_missing_mesh_reason_and_is_named(self):
        import contextlib
        import io
        import os
        import tempfile
        passes = [passing(s) for s in check_range.EXPECTED if s != 'psycho_melee']
        tail = ['OWRANGETEST SUMMARY result=PASS']

        def run(reason, *flags):
            text = log(*passes, 'OWRANGETEST step=psycho_melee result=SKIP reason=' + reason, *tail)
            with tempfile.TemporaryDirectory() as folder:
                path = os.path.join(folder, 'run.log')
                with open(path, 'w', encoding='utf-8') as f:
                    f.write(text)
                out = io.StringIO()
                with contextlib.redirect_stdout(out):
                    code = check_range.main([path, *flags])
            return code, out.getvalue()
        code, out = run('no psycho on this range (its mesh is not imported)')
        self.assertEqual(code, 0)
        self.assertIn('no Psycho exercised', out)
        self.assertEqual(run('no psycho on this range (its mesh is not imported)', '--require-psycho')[0], 1)
        self.assertEqual(run('no enemy')[0], 1)

    def test_ragdoll_steps_skip_only_when_no_stock_mesh_is_asked_for(self):
        passes = [passing(s) for s in check_range.EXPECTED if s not in ('ragdoll', 'corpse_limp', 'corpse_cap')]
        tail = ['OWRANGETEST SUMMARY result=PASS']
        graded = lambda reason: check_range.grade(*check_range.parse(log(
            *passes, *['OWRANGETEST step=%s result=SKIP reason=%s' % (s, reason) for s in ('ragdoll', 'corpse_limp', 'corpse_cap')], *tail)))[0]
        self.assertTrue(graded('the manifest names no stock mesh'))
        self.assertFalse(graded('engine shapes, no stock mesh'))

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
        lines = [passing(s) if overrides.get(s, 'PASS') == 'PASS' else 'OWRANGETEST step=%s result=%s x=1' % (s, overrides[s])
                 for s in check_range.EXPECTED if overrides.get(s) != 'DROP']
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
        self.assertTrue(check_range.grade(*self.steps({'loot_weapon_item': 'SKIP'}))[0])
        self.assertFalse(check_range.grade(*self.steps({'death': 'SKIP'}))[0])

    def test_step_status_needs_a_boundary(self):
        text = log(*['OWRANGETEST step=ragdoll result=SKIP_typo x=1' if s == 'ragdoll' else passing(s) for s in check_range.EXPECTED],
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
        lines = ['OWRANGETEST step=crit_minds_eye result=SKIP ' + reason if s == 'crit_minds_eye' else passing(s) for s in check_range.EXPECTED]
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
