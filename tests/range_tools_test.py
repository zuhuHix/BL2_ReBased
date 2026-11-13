"""Synthetic tests for tools/prepare_range.py and tools/check_range.py: invented rows, names and logs, no game data.

Run: python tests/range_tools_test.py
"""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import check_range  # noqa: E402
import prepare_range  # noqa: E402
sys.path.insert(0, str(Path(__file__).resolve().parent))
import range_fixtures  # noqa: E402


# A kind_melee PASS line must carry numbers inside the acceptance bands (tools/check_range.py); this one does (invented, adult).
KIND_MELEE_OK = ('kind=bullymong_adult enemy=E_3 cycle_min=1.222 cycle_max=1.239 stand_min=201 stand_max=201 first_swing=380 start_range=394 '
                 'max_frame=0.017 swings=4')
SPAWN_ADULT = 'OWRANGE enemy E_3 kind_melee kind=bullymong_adult swing_interval=1.222 stand=201 source=live_read'


def passing(step):
    # kind_melee's result is tied to a spawn record of the same log, so its line comes with one (invented names).
    if step == 'kind_melee':
        return SPAWN_ADULT + '\nOWRANGETEST step=kind_melee result=PASS ' + KIND_MELEE_OK
    # The leap and throw results too (since 2026-11-13): lines that follow the note's rule, with their spawn records.
    if step in ('leap', 'throw'):
        return range_fixtures.ranged_passing(step)
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
            # The import's identity sits next to the target: this invented pawn wears nothing, so nothing is expected.
            with open(os.path.join(folder, 'identity.json'), 'w', encoding='utf-8') as f:
                json.dump({'targets': {'Bullymong': {'attachments': []}}}, f)
            prepare_range.main(['--no-vm', '--allow-missing-spot-data', '--views', views_path, '--terrain', none, '--target', target, '--out', out])
            with open(out, encoding='utf-8') as f:
                manifest = json.load(f)
            self.assertEqual(manifest['assets'], 'imported')
            # The target names no attachments list, so no kind carries one (the host warns about such an entry).
            self.assertNotIn('attachments', manifest['kinds']['bullymong_adult'])
            # A target whose pawn wears nothing writes an empty list; the manifest keeps it, so the host attaches nothing and does not warn.
            with open(target, 'w', encoding='utf-8') as f:
                json.dump({'Bullymong': {'mesh': '/Game/M.M', 'idle': '', 'death': '', 'collision_radius': 130.0, 'collision_height': 70.0,
                                         'attachments': []}}, f)
            prepare_range.main(['--no-vm', '--allow-missing-spot-data', '--views', views_path, '--terrain', none, '--target', target, '--out', out])
            with open(out, encoding='utf-8') as f:
                manifest = json.load(f)
            self.assertEqual(manifest['kinds']['bullymong_adult']['attachments'], [])
            self.assertEqual(manifest['kinds']['bullymong_adult']['attachments_expected'], 0)
            # The baby borrows the asset but keeps its own cylinder; the kinds that own the asset's cylinder keep the asset's.
            baby, adult = manifest['kinds']['bullymong_baby'], manifest['kinds']['bullymong_adult']
            self.assertEqual((baby['mesh'], baby['collision_radius'], baby['collision_height']), ('/Game/M.M', 64.0, 48.0))
            self.assertEqual((adult['collision_radius'], adult['collision_height']), (130.0, 70.0))
            self.assertEqual((adult['run_speed'], baby['run_speed'], manifest['kinds']['bullymong']['run_speed']), (500.0, 300.0, 400.0))
            self.assertEqual(adult['attachments_expected'], 0)
            self.assertEqual(manifest['kinds']['psycho']['attachments_expected'], 0)   # no imported psycho: engine shapes wear nothing

    def write_import(self, folder, placed, worn):
        """An invented import: host_target.json with `placed` placements on one asset, identity.json with `worn` pieces on it."""
        import json
        import os
        target = os.path.join(folder, 'host_target.json')
        with open(target, 'w', encoding='utf-8') as f:
            json.dump({'Bullymong': {'mesh': '/Game/M.M', 'idle': '', 'death': '',
                                     'attachments': [{'mesh': '/Game/P%d.P%d' % (i, i), 'bone': 'b%d' % i} for i in range(placed)]}}, f)
        with open(os.path.join(folder, 'identity.json'), 'w', encoding='utf-8') as f:
            json.dump({'targets': {'Bullymong': {'attachments': [{'component': 'C%d' % i} for i in range(worn)]}}}, f)
        return target

    def prepare(self, target):
        import os
        out = os.path.join(prepare_range.REPO, 'local', 'range', 'test_worn_manifest.json')
        self.addCleanup(lambda: os.path.exists(out) and os.remove(out))
        none = os.path.join(os.path.dirname(target), 'none.json')
        prepare_range.main(['--no-vm', '--allow-missing-spot-data', '--views', none, '--terrain', none, '--target', target, '--out', out])
        return out

    def test_placements_lost_from_the_target_write_no_manifest(self):
        # 2026-11-12: an editor job had rewritten host_target.json without placements; the manifest came out with empty attachments and
        # eight suites ran green on enemies without their worn parts.
        import os
        import tempfile
        with tempfile.TemporaryDirectory() as folder:
            target = self.write_import(folder, 0, 3)
            with self.assertRaises(SystemExit) as stop:
                self.prepare(target)
            self.assertIn('-Steps attach', str(stop.exception))
            self.assertIn('Bullymong: 0 attachment placements', str(stop.exception))
            self.assertFalse(os.path.exists(os.path.join(prepare_range.REPO, 'local', 'range', 'test_worn_manifest.json')))
            os.remove(os.path.join(folder, 'identity.json'))
            with self.assertRaises(SystemExit) as stop:   # no identity: the expectation is unknown, so no manifest either
                self.prepare(target)
            self.assertIn('no entry in', str(stop.exception))

    def test_an_identity_without_the_attachments_key_is_not_wears_nothing(self):
        # Review r1 of PR #250: an identity.json from before 2026-11-06 has no attachments key; with a target without placements that read
        # as 0 expected, and the bare enemies passed.
        import json
        import os
        import tempfile
        with tempfile.TemporaryDirectory() as folder:
            target = self.write_import(folder, 0, 0)
            identity = os.path.join(folder, 'identity.json')
            with open(identity, 'w', encoding='utf-8') as f:
                json.dump({'targets': {'Bullymong': {'pawn': 'P'}}}, f)
            with self.assertRaises(SystemExit) as stop:
                prepare_range.worn_counts(json.load(open(target, encoding='utf-8')), json.load(open(identity, encoding='utf-8')), target, identity)
            self.assertIn('Bullymong: no attachments list in', str(stop.exception))
            self.assertIn('-Steps attach', str(stop.exception))
            with self.assertRaises(SystemExit):
                self.prepare(target)

    def test_an_unreadable_identity_stops_with_the_attach_step(self):
        import os
        import tempfile
        with tempfile.TemporaryDirectory() as folder:
            target = self.write_import(folder, 3, 3)
            identity = os.path.join(folder, 'identity.json')
            for content, word in (('{not json', 'cannot read'), ('[1, 2]', 'not a JSON object')):
                with self.subTest(content=content):
                    with open(identity, 'w', encoding='utf-8') as f:
                        f.write(content)
                    with self.assertRaises(SystemExit) as stop:
                        self.prepare(target)
                    self.assertIn(word, str(stop.exception))
                    self.assertIn('-Steps attach', str(stop.exception))

    def test_the_manifest_carries_the_worn_count_from_the_import(self):
        import json
        import tempfile
        with tempfile.TemporaryDirectory() as folder:
            with open(self.prepare(self.write_import(folder, 3, 3)), encoding='utf-8') as f:
                kinds = json.load(f)['kinds']
            for name in ('bullymong', 'bullymong_adult', 'bullymong_baby', 'bullymong_slinger'):
                self.assertEqual((len(kinds[name]['attachments']), kinds[name]['attachments_expected']), (3, 3), name)
            self.assertEqual(kinds['psycho']['attachments_expected'], 0)

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

    def test_a_manifest_from_before_the_live_read_constants_stops_the_run(self):
        # Review r1: an old manifest (run 400, walk 150, the baby on the borrowed radius 150, no source fields) passed and ran the old values.
        old = {'bullymong_adult': {'run_speed': 400.0, 'walk_speed': 150.0},
               'bullymong_baby': {'run_speed': 400.0, 'walk_speed': 150.0, 'collision_radius': 150.0, 'collision_height': 90.0}}
        problem = self.run_preflight(dict(self.GOOD, kinds=old))
        self.assertIn('predates the live-read constants', problem)
        self.assertIn('re-run python tools/prepare_range.py', problem)
        self.assertIn('bullymong_adult speed', problem)
        self.assertIn('bullymong_baby collision', problem)
        fresh = {name: prepare_range.apply_observed(name, {'run_speed': 400.0, 'walk_speed': 150.0, 'collision_radius': 150.0,
                                                           'collision_height': 90.0}) for name in old}
        self.assertEqual(self.run_preflight(dict(self.GOOD, kinds=fresh)), '')
        # A source field with numbers that are not the table's is stale too; kinds outside the tables are not checked.
        fresh['bullymong_adult']['run_speed'] = 400.0
        self.assertIn('bullymong_adult speed', self.run_preflight(dict(self.GOOD, kinds=fresh)))
        self.assertEqual(self.run_preflight(dict(self.GOOD, kinds={'bullymong': {'run_speed': 400.0}})), '')

    WHOLE = {'mesh': '/Game/M.M', 'melee_arrival_by_level': {'1': 2.0}, 'melee_ratio_by_level': {'1': 0.5}, 'melee_start_range': 111.0}

    def worn(self, kind):
        return self.run_preflight({'spawns': [{'kind': 'made'}], 'kinds': {'made': dict(self.WHOLE, **kind)}}, has_at=True)

    def test_enemies_without_their_worn_attachments_stop_the_run(self):
        problem = self.worn({'attachments': [], 'attachments_expected': 6})
        self.assertIn('without their worn attachments (made 0 of 6 pieces)', problem)
        self.assertIn('import_phaselock_target.ps1 -Steps attach', problem)
        self.assertIn('prepare_phaselock_target.py attachments', problem)
        self.assertIn('made 5 of 6', self.worn({'attachments': [{}] * 5, 'attachments_expected': 6}))

    def test_a_manifest_with_its_worn_attachments_passes(self):
        self.assertEqual(self.worn({'attachments': [{}] * 6, 'attachments_expected': 6}), '')

    def test_a_kind_that_wears_nothing_passes(self):
        self.assertEqual(self.worn({'attachments_expected': 0}), '')
        self.assertEqual(self.worn({'mesh': '', 'attachments_expected': 0}), '')
        self.assertEqual(self.worn({'mesh': ''}), '')   # engine shapes wear nothing, whatever the manifest's age

    def test_a_manifest_from_before_the_worn_check_stops_the_run(self):
        for expected in (None, '6', -1, True):
            kind = {'attachments': [{}] * 6} if expected is None else {'attachments': [{}] * 6, 'attachments_expected': expected}
            self.assertIn('predates the worn-attachment check', self.worn(kind), expected)

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


    # The host's line for a stock pawn with no attachments list (invented log lines around the checker's own marker text).
    BARE = '[2026.11.13-10.00.00:000][  1]LogTemp: Warning: ' + check_range.MISSING_ATTACHMENTS + ' (old or broken prep data), so it wears nothing.'
    WORN = '[2026.11.13-10.00.00:000][  1]LogTemp: Display: OpenWillow target: 2 of 2 attachments on the stock pawn'
    NONE_WORN = '[2026.11.13-10.00.00:000][  1]LogTemp: Display: OpenWillow target: 0 of 0 attachments on the stock pawn'

    def with_lines(self, *extra):
        lines = [passing(s) for s in check_range.EXPECTED] + list(extra) + ['OWRANGETEST SUMMARY result=PASS pass=1 fail=0 skip=0']
        return check_range.grade(*check_range.parse(log(*lines)))

    def test_a_pawn_without_its_attachments_list_fails_a_green_run(self):
        ok, rows = self.with_lines(self.WORN, self.BARE, self.BARE)
        self.assertFalse(ok)
        row = [r for r in rows if r[0] == 'attachments']
        self.assertEqual(len(row), 1)
        self.assertEqual(row[0][1], 'FAIL')
        self.assertIn('2 stock pawn(s)', row[0][2])
        self.assertIn('prepare_range.py', row[0][2])

    def test_worn_and_empty_attachment_lines_pass(self):
        ok, rows = self.with_lines(self.WORN, self.NONE_WORN)
        self.assertTrue(ok)
        self.assertNotIn('attachments', [r[0] for r in rows])

    def test_survey_and_kill_logs_fail_on_a_bare_pawn(self):
        import contextlib
        import io
        import os
        import tempfile
        good = 'OWRANGEKILL gun=%d result=PASS "Made" (pistol, level 1): 3 shots'
        cases = [('--survey-log', ['OWRANGESURVEY done candidates=8 accepted=1'], 2),
                 ('--kill-log', [good % 0, good % 1, 'OWRANGEKILL done guns=2 fail=0'], 1)]
        with tempfile.TemporaryDirectory() as folder:
            path = os.path.join(folder, 'run.log')
            for flag, lines, code in cases:
                for extra, want in (([], 0), ([self.BARE], code)):
                    with open(path, 'w', encoding='utf-8') as f:
                        f.write(log(*(lines + extra)))
                    out = io.StringIO()
                    with contextlib.redirect_stdout(out):
                        got = check_range.main([path, flag])
                    self.assertEqual(got, want, (flag, extra, out.getvalue()))
                    if extra:
                        self.assertIn('no attachments list', out.getvalue())

    def test_the_marker_is_the_host_line_and_the_gate_checker_text(self):
        import check_slice_gate
        source = (Path(__file__).resolve().parents[1] / 'host/ue5/OpenWillow/Source/OpenWillow/OpenWillowCombatTarget.cpp').read_text(encoding='utf-8')
        self.assertIn('TEXT("' + check_range.MISSING_ATTACHMENTS, source)
        self.assertEqual(check_range.MISSING_ATTACHMENTS, check_slice_gate.MISSING_ATTACHMENTS)
        # The worn line the host logs for a list (also an empty one) must not contain the marker.
        self.assertIn('OpenWillow target: %d of %d attachments on the stock pawn', source)
        self.assertNotIn(check_range.MISSING_ATTACHMENTS, self.NONE_WORN)


# ---- Leaps and thrown rocks (2026-11-13, NATIVE_AI_LEAP_THROW.md) -------------------------------------------------------------------------
SOURCE = Path(__file__).resolve().parents[1] / 'host' / 'ue5' / 'OpenWillow' / 'Source' / 'OpenWillow'


def source(name):
    return (SOURCE / name).read_text(encoding='utf-8')


def flat_row(kind):
    """The checker's ATTACKS row for kind, flattened in the order of the host's initializer (OpenWillowLeapThrow.h)."""
    import itertools
    row = check_range.ATTACKS[kind]
    leap = row['leap'] or (0.0, 0.0, 0.0, 0.0, 0.0, False, 0.0, 0.0)
    throw = row['throw'] or (0.0, 0.0, 0.0, 0.0, 0.0, False, False, ())
    clips = list(itertools.chain.from_iterable([seconds, len(moments)] + list(moments) + [0.0] * (2 - len(moments)) for seconds, moments in throw[7]))
    return [row['close']] + list(row['weights']) + [float(v) for v in leap] + [float(v) for v in throw[:7]] + [float(len(throw[7]))] + [float(v) for v in clips]


def host_rows():
    """{kind: [numbers]} from the one-line rows of OpenWillowFindKindAttacks: every number, true / false as 1 / 0, in order."""
    import re
    text = source('OpenWillowLeapThrow.h')
    block = text[text.index('static const FOpenWillowKindAttacks Rows[] = {'):]
    block = block[:block.index('};')]
    rows = {}
    for line in block.splitlines():
        m = re.match(r'\s*\{TEXT\("(\w+)"\),(.*)\},\s*$', line)
        if m:
            tokens = re.findall(r'true|false|-?\d+(?:\.\d+)?', m.group(2))
            rows[m.group(1)] = [1.0 if t == 'true' else 0.0 if t == 'false' else float(t) for t in tokens]
    return rows


class LeapThrowRowsTest(unittest.TestCase):
    def test_the_host_rows_are_the_checkers_number_by_number(self):
        rows = host_rows()
        self.assertEqual(set(rows), set(check_range.ATTACKS))
        for kind in check_range.ATTACKS:
            with self.subTest(kind=kind):
                self.assertEqual(rows[kind], flat_row(kind))

    def test_the_rows_are_the_notes(self):
        # A few values straight from the note's section 2.1, 3.2 and 4 (the full rows are in the checker, held to the host above).
        a = check_range.ATTACKS
        self.assertEqual(a['bullymong_adult']['weights'], (1.0, 0.25, 0.25))
        self.assertEqual(a['bullymong']['weights'][:2], (1.0, 0.5))
        self.assertEqual(a['bullymong']['leap'][:5], (1024.0, 2048.0, 2000.0, 0.4, 0.0))
        self.assertIsNone(a['bullymong']['throw'])
        self.assertEqual(a['bullymong_adult']['throw'][:5], (512.0, 3072.0, 2000.0, 0.3, 480.0))
        self.assertIsNone(a['bullymong_slinger']['leap'])
        self.assertEqual(a['bullymong_baby']['leap'][:5], (480.0, 2048.0, 2500.0, 0.45, 128.0))

    def test_the_solver_reproduces_the_notes_worked_table(self):
        # Note 3.2 and 4.2, flat ground, no lead: seconds to 0.01 and peaks (d tan(a) / 4) to 1 uu, as the note prints them.
        import math
        table = [(2500, 0.4, [(1024, 0.51, 186), (1600, 0.79, 291), (2048, 1.01, 372)]),
                 (2000, 0.4, [(1024, 0.63, 186), (1600, 0.99, 291), (2048, 1.27, 372)]),
                 (2500, 0.45, [(1024, 0.54, 219), (1600, 0.84, 342), (2048, 1.08, 437)]),
                 (2000, 0.3, [(1024, 0.57, 130), (1600, 0.90, 204), (2048, 1.15, 261), (3000, 1.68, 382)])]
        for speed, angle_pct, rows in table:
            for distance, flight, peak in rows:
                angle, _, solved = check_range.solve_arc(distance, 0.0, distance, speed, angle_pct)
                self.assertAlmostEqual(angle, angle_pct * 90.0, places=6)
                self.assertLessEqual(abs(solved - flight), 0.005, (speed, angle_pct, distance))
                self.assertLessEqual(abs(distance * math.tan(math.radians(angle)) / 4.0 - peak), 0.5, (speed, angle_pct, distance))

    def test_every_valid_leap_falls_inside_l4s_attack_to_landed(self):
        # The launch clip plus the flight over the band: the shortest (the adult at 1024) and the longest (the regular or the baby at 2048).
        launch = 0.833
        shortest = launch + check_range.solve_arc(1024, 0, 1024, 2500, 0.4)[2]
        longest = launch + max(check_range.solve_arc(2048, 0, 2048, 2000, 0.4)[2], check_range.solve_arc(2048, 0, 2048, 2500, 0.45)[2])
        low, high = check_range.LIVE_ATTACK_TO_LANDED
        self.assertTrue(low <= shortest and longest <= high, (shortest, longest))


class LeapThrowCheckerTest(unittest.TestCase):
    leap = staticmethod(range_fixtures.leap_detail)
    throw = staticmethod(range_fixtures.throw_detail)

    def test_good_lines_pass(self):
        self.assertEqual(check_range.leap_problem(self.leap()), '')
        self.assertEqual(check_range.leap_problem(self.leap('bullymong_adult', 'E_3', lead=60.0)), '')
        self.assertEqual(check_range.leap_problem(self.leap('bullymong_baby', 'E_9', distance=600.0)), '')
        self.assertEqual(check_range.throw_problem(self.throw()), '')
        self.assertEqual(check_range.throw_problem(self.throw('bullymong_slinger', 'E_8', distance=2000.0, release=1.665, release_due=1.660)), '')

    def test_a_leap_outside_its_band_fails(self):
        self.assertIn('outside its band', check_range.leap_problem(self.leap(start_distance='900')))
        self.assertIn('outside its band', check_range.leap_problem(self.leap(start_distance='2100')))

    def test_the_launch_clip_and_the_flight_must_last_what_the_rule_says(self):
        self.assertIn('launch clip lasted', check_range.leap_problem(self.leap(windup=0.70)))
        self.assertIn('launch clip lasted', check_range.leap_problem(self.leap(windup=0.90)))   # more than a frame late
        self.assertIn('the flight lasted', check_range.leap_problem(self.leap(late=0.05)))
        self.assertIn('the flight lasted', check_range.leap_problem(self.leap(late=-0.01)))
        self.assertIn('attack_to_landed', check_range.leap_problem(self.leap(attack_to_landed='1.000')))

    def test_an_arc_that_is_not_the_notes_solver_fails(self):
        self.assertIn("not the note's solver", check_range.leap_problem(self.leap(flight_predicted='0.5000')))
        self.assertIn("not the note's solver", check_range.throw_problem(self.throw(flight_predicted='0.5000')))
        self.assertIn('over the cap', check_range.leap_problem(self.leap(lead=40.0)))                         # the regular has no lead
        self.assertIn('over the cap', check_range.leap_problem(self.leap('bullymong_adult', 'E_3', lead=120.0)))

    def test_a_row_that_is_not_the_notes_fails(self):
        self.assertIn("not the note's", check_range.leap_problem(self.leap(speed='2500')))
        self.assertIn("not the note's", check_range.leap_problem(self.leap(band_min='900')))
        self.assertIn('no leap', check_range.leap_problem(self.leap().replace('kind=bullymong ', 'kind=bullymong_slinger ')))
        self.assertIn('no throw', check_range.throw_problem(self.throw().replace('kind=bullymong_adult ', 'kind=bullymong ')))

    def test_the_landing_and_the_knock_back(self):
        self.assertIn('did not hit', check_range.leap_problem(self.leap(hit='0')))
        self.assertIn('came down', check_range.leap_problem(self.leap(land_error='5.0')))
        self.assertIn('melee numbers', check_range.leap_problem(self.leap(applied='12.00', pools_lost='12.00')))
        self.assertIn('melee numbers', check_range.leap_problem(self.leap(pools_lost='0.00')))
        self.assertIn('knock', check_range.leap_problem(self.leap(knock='62')))
        self.assertIn('forced', check_range.leap_problem(self.leap(forced='0')))

    def test_a_rock_must_leave_at_a_release_moment_and_touch_maya(self):
        self.assertIn('release moment', check_range.throw_problem(self.throw(release=1.20, release_due=1.20)))
        self.assertIn('the rock left', check_range.throw_problem(self.throw(release=1.030)))
        self.assertIn('came down on world', check_range.throw_problem(self.throw(what='world', hit='0')))
        self.assertIn('the rock flew', check_range.throw_problem(self.throw(early=0.2)))
        self.assertIn('the rock flew', check_range.throw_problem(self.throw(early=-0.05)))
        self.assertIn('touched', check_range.throw_problem(self.throw(miss='80.0')))
        self.assertIn('knock', check_range.throw_problem(self.throw(knock='1000')))
        self.assertIn('outside its band', check_range.throw_problem(self.throw(start_distance='400')))

    def test_a_leap_outside_l4s_live_window_fails(self):
        # No leap of a valid row reaches this check with L-4's 1.08 to 3.48 s (LeapThrowRowsTest), so the window is narrowed here to reach it.
        from unittest import mock
        with mock.patch.object(check_range, 'LIVE_ATTACK_TO_LANDED', (1.08, 1.5)):
            self.assertIn("outside L-4's", check_range.leap_problem(self.leap()))
        self.assertEqual(check_range.leap_problem(self.leap()), '')

    def test_a_rock_lead_over_its_cap_fails(self):
        self.assertIn('over the cap', check_range.throw_problem(self.throw(lead='600.0')))
        self.assertEqual(check_range.throw_problem(self.throw(lead='400.0')), '')

    def test_a_rock_that_delivered_other_numbers_fails(self):
        self.assertIn('melee numbers', check_range.throw_problem(self.throw(applied='12.00', pools_lost='12.00')))
        self.assertIn('melee numbers', check_range.throw_problem(self.throw(pools_lost='0.00')))
        self.assertIn('melee numbers', check_range.throw_problem(self.throw(expected='0.00', applied='0.00', pools_lost='0.00')))

    def test_incomplete_or_impossible_lines_fail(self):
        self.assertIn('lacks windup', check_range.leap_problem(self.leap().replace(' windup=0.840', '')))
        self.assertIn('lacks what', check_range.throw_problem(self.throw(what=None)))
        self.assertIn('repeats', check_range.leap_problem(self.leap() + ' hit=1'))
        self.assertIn('not a finite number', check_range.leap_problem(self.leap(d0='nan')))
        self.assertIn('not a number', check_range.throw_problem(self.throw(miss='far')))
        self.assertIn('stalled run', check_range.leap_problem(self.leap(max_frame=0.25)))
        self.assertIn('max_frame', check_range.leap_problem(self.leap(max_frame=0.0)))


class LeapThrowTiedToSpawnsTest(unittest.TestCase):
    def graded(self, step, *lines):
        steps, _ = check_range.parse('\n'.join(lines))
        return check_range.ranged_graded(step, steps.get(step, []), steps.spawns, steps.attacks)

    def verdicts(self, step, *lines):
        return [v for v, _ in self.graded(step, *lines)]

    def test_the_default_range_needs_both_leapers_and_the_thrower(self):
        self.assertEqual(self.verdicts('leap', range_fixtures.ranged_passing('leap')), ['PASS', 'PASS'])
        self.assertEqual(self.verdicts('throw', range_fixtures.ranged_passing('throw')), ['PASS'])
        only_regular = '\n'.join(range_fixtures.DEFAULT_SPAWNS + ['OWRANGETEST step=leap result=PASS ' + range_fixtures.leap_detail()])
        details = ' '.join(d for _, d in self.graded('leap', only_regular))
        self.assertIn('0 passing results for spawned kind bullymong_adult', details)

    def test_a_result_from_an_enemy_of_another_kind_fails(self):
        line = 'OWRANGETEST step=throw result=PASS ' + range_fixtures.throw_detail(enemy='E_0')
        details = ' '.join(d for _, d in self.graded('throw', *range_fixtures.DEFAULT_SPAWNS, line))
        self.assertIn('enemy E_0 was not spawned as bullymong_adult', details)

    def test_a_duplicated_result_fails(self):
        line = 'OWRANGETEST step=throw result=PASS ' + range_fixtures.throw_detail()
        details = ' '.join(d for _, d in self.graded('throw', *range_fixtures.DEFAULT_SPAWNS, line, line))
        self.assertIn('2 passing results for spawned kind bullymong_adult', details)

    def test_a_logged_row_that_is_not_the_notes_fails(self):
        spawns = [s.replace('leap_speed=2000', 'leap_speed=2200') for s in range_fixtures.DEFAULT_SPAWNS]
        details = ' '.join(d for _, d in self.graded('leap', *spawns, *range_fixtures.ranged_passing('leap').splitlines()[4:]))
        self.assertIn('leap_speed=2200 (note 2000)', details)

    def test_no_spawn_record_fails(self):
        details = ' '.join(d for _, d in self.graded('leap', 'OWRANGETEST step=leap result=PASS ' + range_fixtures.leap_detail()))
        self.assertIn('no enemy was spawned with a leap', details)

    def test_a_kind_with_no_row_is_plain_melee_only(self):
        steps, _ = check_range.parse('OWRANGE enemy E_4 attacks kind=psycho source=none (plain melee only)')
        self.assertEqual(steps.attacks, {'E_4': ('psycho', '', 'none')})

    def test_the_fight_counts_read_the_hosts_lines(self):
        text = '\n'.join(['OWRANGE attack_pick enemy=E_0 kind=bullymong pick=leap distance=1500 leap_ok=1 throw_ok=0 forced=0 t=3.00',
                          'OWRANGE leap_start enemy=E_0 kind=bullymong t=3.000 distance=1500',
                          'OWRANGE leap_land enemy=E_0 kind=bullymong t=4.900 flight=1.0 predicted=1.0 attack_to_landed=1.9 land_error=0.0 hit=1 expected=1 applied=1 knock=1000',
                          'OWRANGE rock_release enemy=E_3 kind=bullymong_adult t=5.000 release=1.064 due=1.064',
                          'OWRANGE rock_impact enemy=E_3 kind=bullymong_adult t=5.800 what=maya flight=0.8 predicted=0.8 miss=50.0 hit=1 damage=1'])
        counts = check_range.fight_counts(text)
        self.assertEqual((counts['leap_starts'], counts['leap_lands'], counts['leap_hits'], counts['rocks'], counts['rock_hits']), (1, 1, 1, 1, 1))
        self.assertEqual(counts['picks'], {('bullymong', 'leap'): 1})


class LeapThrowHostTest(unittest.TestCase):
    def test_the_host_logs_what_the_checker_reads(self):
        rng = source('OpenWillowRange.cpp')
        step = rng[rng.index('case 96: // leap and throw'):rng.index('// ---------------------------------------------------------------------------------------------- loot driver')]
        for key in check_range.LEAP_FIELDS + check_range.THROW_FIELDS:
            self.assertIn(' %s=' % key if key != 'kind' else 'kind=%s', step, key)
        enemy = source('OpenWillowEnemy.cpp')
        self.assertIn('OWRANGE enemy %s attacks kind=%s close=%.0f weights=%.2f:%.2f:%.2f leap=%.0f-%.0f', enemy)
        for key in ('leap_speed', 'leap_angle_pct', 'leap_lead_cap', 'leap_resource', 'leap_clips', 'throw_speed', 'throw_angle_pct', 'throw_lead_cap',
                    'throw_front', 'throw_resource', 'throw_clips'):
            self.assertIn(' %s=' % key, enemy)
        self.assertIn('source=none (plain melee only)', enemy)

    def test_kind_melee_measures_the_plain_swing_alone(self):
        rng = source('OpenWillowRange.cpp')
        step = rng[rng.index('case 94: // kind_melee'):rng.index('case 95:')]
        self.assertIn('Enemy->SetRangedAttacks(false);', step)
        self.assertEqual(check_range.EXPECTED.index('leap'), check_range.EXPECTED.index('kind_melee') + 1)

    def test_the_notice_chase_and_swing_steps_run_without_leaps_or_rocks(self):
        # Review r1: approach put Maya inside the regular's leap band and passed on a point of a leap's arc. The shared reset of steps 16-36
        # turns E0's leaps and throws off; the leap and throw steps turn their enemy's back on, and the end of those steps every enemy's.
        rng = source('OpenWillowRange.cpp')
        reset = rng[rng.index('auto Reset = [this, Pawn, E0, Now](const TCHAR* Why)'):rng.index('auto OnlyActive')]
        self.assertIn('E0->SetRangedAttacks(false);', reset)
        self.assertLess(reset.index('E0->ResetForAITest('), reset.index('E0->SetRangedAttacks(false);'))
        ranged = rng[rng.index('case 96: // leap and throw'):rng.index('case 97:')]
        self.assertLess(ranged.index('Reset(StepName);'), ranged.index('Enemy->ResetForAITest('))
        self.assertIn('Each->SetRangedAttacks(true);', ranged)
        self.assertIn('bRangedAttacks = true;', source('OpenWillowEnemy.cpp'))   # ResetForAITest turns them back on

    def test_the_rock_traces_the_world_before_it_tests_maya(self):
        # Review r1: a wall between Maya and the rock inside one tick's step must stop it.
        enemy = source('OpenWillowEnemy.cpp')
        tick = enemy[enemy.index('void AOpenWillowRangeRock::Tick'):enemy.index('void AOpenWillowRangeRock::Finish')]
        self.assertLess(tick.index('LineTraceSingleByObjectType'), tick.index('SegmentDistToSegmentSafe(From, End,'))

    def test_the_brain_draws_once_per_finished_attack_and_gives_resources_back(self):
        brain = source('OpenWillowEnemyAI.cpp')
        self.assertIn('if (bStateUpdate && bAware && In.Attacks && !bSwingPending)', brain)
        self.assertIn('if (Pick != EOpenWillowAttack::None) return;', brain)
        self.assertIn('if (Pick == EOpenWillowAttack::Melee) Pick = EOpenWillowAttack::None;', brain)
        self.assertIn('HeldDen->LeapUsers = FMath::Max(0, HeldDen->LeapUsers - 1);', brain)
        self.assertIn('EndAttack(TEXT("stopped"));', brain)
        self.assertIn('constexpr int32 LeapUsersMax = 1;', source('OpenWillowLeapThrow.h'))
        self.assertIn('constexpr int32 ThrowUsersMax = 3;', source('OpenWillowLeapThrow.h'))

    def test_no_brace_less_if_with_a_ue_log_before_an_else(self):
        # C2181 (project memory): a brace-less if holding a UE_LOG and followed by else does not compile.
        import re
        for name in ('OpenWillowEnemyAI.cpp', 'OpenWillowEnemy.cpp', 'OpenWillowRange.cpp'):
            self.assertNotRegex(source(name), r'\bif \([^\n]*\)\s*\n\s*UE_LOG\([^;]*\);\s*\n\s*else', name)


class LeapThrowClipRolesTest(unittest.TestCase):
    def test_leap_and_throw_clips_get_their_roles_and_are_not_the_melee(self):
        names = ['Idle', 'Attack_Leap_Start', 'Attack_Leap_Idle', 'Attack_Leap_End', 'Attack_ThrowRock_var1', 'Attack_Swipe']
        clips = prepare_range.find_clips('/Game/X/Idle.Idle', names)
        self.assertEqual(clips['leap_start'], '/Game/X/Attack_Leap_Start.Attack_Leap_Start')
        self.assertEqual(clips['leap_idle'], '/Game/X/Attack_Leap_Idle.Attack_Leap_Idle')
        self.assertEqual(clips['leap_end'], '/Game/X/Attack_Leap_End.Attack_Leap_End')
        self.assertEqual(clips['throw'], '/Game/X/Attack_ThrowRock_var1.Attack_ThrowRock_var1')
        self.assertEqual(clips['melee'], '/Game/X/Attack_Swipe.Attack_Swipe')

    def test_an_import_without_them_leaves_the_roles_empty(self):
        clips = prepare_range.find_clips('/Game/X/Idle.Idle', ['Idle', 'Walk_F'])
        self.assertEqual((clips['leap_start'], clips['leap_idle'], clips['leap_end'], clips['throw']), ('', '', '', ''))
        self.assertEqual(prepare_range.kind_entry('B', None, {}, 't')['throw'], '')


if __name__ == '__main__':
    unittest.main()
