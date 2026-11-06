"""Synthetic tests for tools/check_weapon_firing.py: invented ids, recipes and numbers, no game data.

Run: python tests/check_weapon_firing_test.py
"""
import contextlib
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import check_weapon_firing as check  # noqa: E402


def recipe(card_damage, projectiles=1, firing=None):
    data = {'name': 'Made-up gun', 'stats': {'card': {'damage': card_damage, 'projectiles': projectiles}}}
    if firing is not None:
        data['firing'] = firing
    return data


def write_recipes(folder, recipes):
    for gun, data in recipes.items():
        (Path(folder) / f'{gun}.json').write_text(json.dumps(data), encoding='utf-8')


def result(gun, **values):
    base = {'kind': 'Trace', 'pellets': '8', 'shots': '8', 'launches': '0', 'impacts': '8', 'target_hits': '8',
            'damage': '80.00', 'direct': '80.00', 'splash': '0.00', 'splash_hits': '0', 'status_rolls': '0',
            'procs': '0', 'ticks': '0'}
    base.update({key: str(value) for key, value in values.items()})
    if not values.pop('legacy', False) and 'hit_events' not in values:
        # Hit-event fields consistent with the counts (a direct event per non-rocket target hit, splash and tick events).
        direct = 0 if base['kind'] == 'Rocket' else int(base['target_hits'])
        hits = {'hit_direct': direct, 'hit_splash': int(base['splash_hits']), 'hit_tick': int(base['ticks']), 'hit_throw': 0}
        base.update({k: str(v) for k, v in hits.items()})
        base['hit_events'] = str(sum(hits.values()))
    if 'legacy' not in base:
        base.setdefault('unknown_sources', '0')
        base.setdefault('tick_weapons', f"{gun}:{base['ticks']}" if int(base['ticks']) else '-')
    base.pop('legacy', None)
    return f"LogTemp: Display: OWFIRING result id={gun} " + ' '.join(f'{k}={v}' for k, v in base.items() if k != 'id')


def segment(gun, index, wall, target_hits):
    return f'LogTemp: Display: OWFIRING segment id={gun} index={index} wall={wall} shots=5 target_hits={target_hits}'


def log(*lines):
    return '\n'.join(lines) + '\n'


class Expectations(unittest.TestCase):
    def test_kind_follows_fire_type_then_firing_block(self):
        self.assertEqual(check.expectations(recipe(10))['kind'], 'Trace')
        self.assertEqual(check.expectations(recipe(10, firing={'fire_type': None}))['kind'], 'Bullet')
        self.assertEqual(check.expectations(recipe(10, firing={'fire_type': 'EWWFT_Rocket'}))['kind'], 'Rocket')
        self.assertEqual(check.expectations(recipe(10, firing={'fire_type': 'EWWFT_HitScan'}))['kind'], 'Trace')

    def test_splash_falls_back_to_card_damage_when_null(self):
        firing = {'projectile': {'splash': {'damage': None, 'damage_scale': 0.5}}}
        self.assertAlmostEqual(check.expectations(recipe(40, firing=firing))['splash_damage'], 20.0)

    def test_impact_explosion_is_the_bullet_splash(self):
        firing = {'fire_type': None, 'impact_explosion': {'damage': 12.0, 'momentum': 300.0}}
        out = check.expectations(recipe(15, firing=firing))
        self.assertTrue(out['splash'] and out['explodes'])
        self.assertEqual(out['splash_momentum'], 300.0)
        self.assertFalse(out['wide'])


class Gun(unittest.TestCase):
    def setUp(self):
        self.folder = tempfile.TemporaryDirectory()
        self.dirs = [self.folder.name]
        write_recipes(self.folder.name, {
            'made_trace': recipe(10.0),
            'made_rocket': recipe(50.0, firing={'fire_type': 'EWWFT_Rocket',
                                                 'projectile': {'splash': {'damage': 30.0, 'radius': 200}},
                                                 'status': {'duration': 2.0, 'dps': 5.0}}),
            'made_detonating': recipe(50.0, firing={'fire_type': 'EWWFT_Rocket', 'projectile': {
                'explode_on_impact': True, 'splash': {'damage': 30.0, 'radius': 200}}}),
            'made_burst': recipe(10.0, firing={'fire_type': None, 'burst_count': 3}),
            'made_throw': recipe(10.0, firing={'fire_type': None, 'reload_throw': {'projectile': {}}}),
            # A status without damage ticks (an invented slag-like effect: no duration resolved, no dps).
            'made_debuff': recipe(50.0, firing={'fire_type': 'EWWFT_Rocket',
                                                'projectile': {'splash': {'damage': 30.0, 'radius': 200}},
                                                'status': {'duration': 0.0, 'dps': 0.0}}),
        })

    def tearDown(self):
        self.folder.cleanup()

    def run_check(self, text, guns, hit_events=True):
        extra = {}
        results, segments, skipped, no_wall, _, throws, lifecycles = check.read_log(text, extra)
        out = []
        for gun in guns:
            rec, _ = check.find_recipe(self.dirs, gun)
            expected = check.expectations(rec)
            out.append(check.check_gun(gun, results.get(gun), segments.get(gun), expected, skipped.get(gun), no_wall,
                                       throws.get(gun), lifecycles.get(gun), hit_events=hit_events,
                                       swap=extra['swap'].get(gun)))
        return out

    def test_trace_gun_passes(self):
        text = log(segment('made_trace', 0, 0, 5), result('made_trace'))
        fails, _, values = self.run_check(text, ['made_trace'])[0]
        self.assertEqual(fails, [])
        self.assertEqual(values['hits_a'], 5)

    def test_wrong_direct_damage_fails(self):
        text = log(segment('made_trace', 0, 0, 5), result('made_trace', direct='70.00', damage='70.00'))
        fails, _, _ = self.run_check(text, ['made_trace'])[0]
        self.assertTrue(any('direct damage' in reason for reason in fails), fails)

    def test_rocket_with_status_passes_and_direct_damage_is_zero(self):
        text = log(
            segment('made_rocket', 0, 0, 0),
            'LogTemp: Display: OWFIRING swap id=made_rocket to=made_trace ticks_before=1 extra_pulls=0',
            result('made_rocket', kind='Rocket', shots=28, pellets=28, launches=28, impacts=28, target_hits=0,
                   damage='60.00', direct='0.00', splash='60.00', splash_hits=2, status_rolls=2, procs=1, ticks=3),
        )
        fails, notes, _ = self.run_check(text, ['made_rocket'])[0]
        self.assertEqual(fails, [], fails)
        self.assertTrue(any('rocket missed' in note for note in notes), notes)

    def test_missing_hit_fields_fail_unless_legacy(self):
        stale = result('made_trace', legacy=True)
        fails, _, _ = self.run_check(log(segment('made_trace', 0, 0, 5), stale), ['made_trace'])[0]
        self.assertTrue(any('pass --legacy' in reason for reason in fails), fails)
        fails, _, _ = self.run_check(log(segment('made_trace', 0, 0, 5), stale), ['made_trace'], hit_events=False)[0]
        self.assertEqual(fails, [], fails)

    def test_ticks_after_a_weapon_swap_must_name_the_proc_gun(self):
        swap = 'LogTemp: Display: OWFIRING swap id=made_rocket to=made_trace ticks_before=1 extra_pulls=0'
        common = dict(kind='Rocket', shots=28, pellets=28, launches=28, impacts=28, target_hits=0, damage='60.00',
                      direct='0.00', splash='60.00', splash_hits=2, status_rolls=2, procs=1, ticks=3)
        good = result('made_rocket', **common)
        fails, _, _ = self.run_check(log(segment('made_rocket', 0, 0, 0), swap, good), ['made_rocket'])[0]
        self.assertEqual(fails, [], fails)
        wrong = result('made_rocket', **common, hit_events=7, hit_direct=0, hit_splash=2, hit_tick=3, hit_throw=0,
                       tick_weapons='made_rocket:1,made_trace:2')
        fails, _, _ = self.run_check(log(segment('made_rocket', 0, 0, 0), swap, wrong), ['made_rocket'])[0]
        self.assertTrue(any('credited to another weapon (after the swap to made_trace)' in r for r in fails), fails)
        late = result('made_rocket', **dict(common, ticks=1), hit_events=5, hit_direct=0, hit_splash=2, hit_tick=1,
                      hit_throw=0, tick_weapons='made_rocket:1')
        fails, _, _ = self.run_check(log(segment('made_rocket', 0, 0, 0), swap, late), ['made_rocket'])[0]
        self.assertTrue(any('no tick landed after it' in r for r in fails), fails)

    def test_a_status_without_ticks_skips_the_swap(self):
        common = dict(kind='Rocket', shots=28, pellets=28, launches=28, impacts=28, target_hits=0, damage='60.00',
                      direct='0.00', splash='60.00', splash_hits=2, status_rolls=2, procs=1, ticks=0)
        skip = 'LogTemp: Display: OWFIRING swap id=made_debuff skipped=no_ticks duration=0.00 dps=0.00'
        fails, _, _ = self.run_check(log(segment('made_debuff', 0, 0, 0), skip, result('made_debuff', **common)),
                                     ['made_debuff'])[0]
        self.assertEqual(fails, [], fails)
        # No swap line at all is fine too for a status that cannot tick (nothing to attribute).
        fails, _, _ = self.run_check(log(segment('made_debuff', 0, 0, 0), result('made_debuff', **common)),
                                     ['made_debuff'])[0]
        self.assertEqual(fails, [], fails)

    def test_a_ticking_status_may_not_skip_the_swap(self):
        common = dict(kind='Rocket', shots=28, pellets=28, launches=28, impacts=28, target_hits=0, damage='60.00',
                      direct='0.00', splash='60.00', splash_hits=2, status_rolls=2, procs=1, ticks=3)
        skip = 'LogTemp: Display: OWFIRING swap id=made_rocket skipped=no_ticks duration=0.00 dps=0.00'
        fails, _, _ = self.run_check(log(segment('made_rocket', 0, 0, 0), skip, result('made_rocket', **common)),
                                     ['made_rocket'])[0]
        self.assertTrue(any('skipped (no_ticks) for a status that ticks' in r for r in fails), fails)

    def test_status_gun_without_a_successful_swap_fails(self):
        common = dict(kind='Rocket', shots=28, pellets=28, launches=28, impacts=28, target_hits=0, damage='60.00',
                      direct='0.00', splash='60.00', splash_hits=2, status_rolls=2, procs=1, ticks=3)
        fails, _, _ = self.run_check(log(segment('made_rocket', 0, 0, 0), result('made_rocket', **common)), ['made_rocket'])[0]
        self.assertTrue(any('without an OWFIRING swap line' in r for r in fails), fails)
        failed = 'LogTemp: Warning: OWFIRING swap id=made_rocket failed=no_effect_running ticks_before=3 extra_pulls=40'
        fails, _, _ = self.run_check(log(segment('made_rocket', 0, 0, 0), failed,
                                         result('made_rocket', **dict(common, shots=68, pellets=68, launches=68))),
                                     ['made_rocket'])[0]
        self.assertTrue(any('weapon swap failed: no_effect_running' in r for r in fails), fails)

    def test_extra_pulls_count_and_tick_totals_must_match(self):
        common = dict(kind='Rocket', shots=30, pellets=30, launches=30, impacts=30, target_hits=0, damage='60.00',
                      direct='0.00', splash='60.00', splash_hits=2, status_rolls=2, procs=1, ticks=3)
        swap = 'LogTemp: Display: OWFIRING swap id=made_rocket to=made_trace ticks_before=1 extra_pulls=2'
        fails, _, _ = self.run_check(log(segment('made_rocket', 0, 0, 0), swap, result('made_rocket', **common)),
                                     ['made_rocket'])[0]
        self.assertEqual(fails, [], fails)
        empty = result('made_rocket', **common, hit_events=5, hit_direct=0, hit_splash=2, hit_tick=3, hit_throw=0,
                       tick_weapons='-')
        fails, _, _ = self.run_check(log(segment('made_rocket', 0, 0, 0), swap, empty), ['made_rocket'])[0]
        self.assertTrue(any('tick_weapons total 0 != ticks 3' in r for r in fails), fails)

    def test_hit_events_match_the_counts(self):
        good = result('made_trace', hit_events=8, hit_direct=8, hit_splash=0, hit_tick=0, hit_throw=0)
        fails, _, _ = self.run_check(log(segment('made_trace', 0, 0, 5), good), ['made_trace'])[0]
        self.assertEqual(fails, [], fails)
        short = result('made_trace', hit_events=7, hit_direct=7, hit_splash=0, hit_tick=0, hit_throw=0)
        fails, _, _ = self.run_check(log(segment('made_trace', 0, 0, 5), short), ['made_trace'])[0]
        self.assertTrue(any('7 direct hit events, expected 8' in reason for reason in fails), fails)
        rocket = result('made_rocket', kind='Rocket', shots=28, pellets=28, launches=28, impacts=28, target_hits=2,
                        damage='60.00', direct='0.00', splash='60.00', splash_hits=2, status_rolls=2, procs=1, ticks=3,
                        hit_events=7, hit_direct=2, hit_splash=2, hit_tick=3, hit_throw=0)
        fails, _, _ = self.run_check(log(segment('made_rocket', 0, 0, 0), rocket), ['made_rocket'])[0]
        self.assertTrue(any('2 direct hit events, expected 0' in reason for reason in fails), fails)
        missing = result('made_trace').replace(' unknown_sources=0', '')
        fails, _, _ = self.run_check(log(segment('made_trace', 0, 0, 5), missing), ['made_trace'])[0]
        self.assertTrue(any('no unknown_sources field' in reason for reason in fails), fails)
        unknown = result('made_trace', unknown_sources=2)
        fails, _, _ = self.run_check(log(segment('made_trace', 0, 0, 5), unknown), ['made_trace'])[0]
        self.assertTrue(any('2 hit events with an Unknown source' in reason for reason in fails), fails)
        bad_sum = result('made_trace', hit_events=9, hit_direct=8, hit_splash=0, hit_tick=0, hit_throw=0)
        fails, _, _ = self.run_check(log(segment('made_trace', 0, 0, 5), bad_sum), ['made_trace'])[0]
        self.assertTrue(any('sum of the kinds' in reason for reason in fails), fails)

    def test_status_ticks_without_a_proc_fail(self):
        text = log(segment('made_rocket', 0, 0, 0),
                   result('made_rocket', kind='Rocket', shots=28, pellets=28, launches=28, impacts=28, target_hits=0,
                          damage='6.00', direct='0.00', splash='0.00', status_rolls=3, procs=0, ticks=2))
        fails, _, _ = self.run_check(text, ['made_rocket'])[0]
        self.assertTrue(any('without a proc' in reason for reason in fails), fails)

    def test_skipped_and_missing_results_fail(self):
        text = log(segment('made_trace', 0, 0, 5), 'LogTemp: Error: OWFIRING skipped id=made_rocket reason=not in the backpack')
        skipped_fail, _, _ = self.run_check(text, ['made_rocket'])[0]
        self.assertTrue(any('not in the backpack' in reason for reason in skipped_fail), skipped_fail)
        missing_fail, _, _ = self.run_check(text, ['made_trace'])[0]
        self.assertTrue(any('no OWFIRING result' in reason for reason in missing_fail), missing_fail)

    def test_burst_gun_needs_burst_count_shots_per_pull(self):
        bullets = dict(kind='Bullet', launches=8, impacts=8)
        short = log(segment('made_burst', 0, 0, 5), result('made_burst', **bullets))
        fails, _, _ = self.run_check(short, ['made_burst'])[0]
        self.assertTrue(any('burst 3' in reason for reason in fails), fails)
        lines = ['OWFIRING lifecycle id=made_burst case=reload first_shot=1 extra_shots=0',
                 'OWFIRING lifecycle id=made_burst case=respawn first_shot=1 extra_shots=0']
        full = log(segment('made_burst', 0, 0, 5), result('made_burst', kind='Bullet', shots=26, pellets=26, launches=26,
                                                          impacts=26), *lines)
        fails, _, _ = self.run_check(full, ['made_burst'])[0]
        self.assertEqual(fails, [], fails)

    def test_burst_that_fires_on_after_a_reload_or_respawn_fails(self):
        bad = ['OWFIRING lifecycle id=made_burst case=reload first_shot=1 extra_shots=2']
        text = log(segment('made_burst', 0, 0, 5), result('made_burst', kind='Bullet', shots=26, pellets=26, launches=26,
                                                          impacts=26), *bad)
        fails, _, _ = self.run_check(text, ['made_burst'])[0]
        self.assertTrue(any('2 shots after the reload' in reason for reason in fails), fails)
        self.assertTrue(any('lifecycle line for respawn' in reason for reason in fails), fails)

    def test_detonating_rocket_that_never_splashes_fails(self):
        text = log(segment('made_detonating', 0, 0, 0),
                   result('made_detonating', kind='Rocket', launches=8, target_hits=0, direct='0.00', damage='0.00'))
        fails, _, _ = self.run_check(text, ['made_detonating'])[0]
        self.assertTrue(any('splashed nothing' in reason for reason in fails), fails)

    def test_tediore_throw_needs_a_full_refill(self):
        # A single straight bullet gun: 5 + 3 pulls plus the two wide-bullet pulls.
        bullets = dict(kind='Bullet', shots=10, pellets=10, launches=10, impacts=10, target_hits=9, direct='85.00', damage='85.00',
                       hit_events=9, hit_direct=8, hit_splash=0, hit_tick=0, hit_throw=1)
        missing = log(segment('made_throw', 0, 0, 5), result('made_throw', **bullets))
        fails, _, _ = self.run_check(missing, ['made_throw'])[0]
        self.assertTrue(any('no OWFIRING throw' in reason for reason in fails), fails)
        kept = 'OWFIRING throw id=made_throw rounds=5 moved=7 magazine=12 size=12 target_hits=1 direct=5.00 splash_hits=0 splash=0.00'
        fails, _, _ = self.run_check(log(segment('made_throw', 0, 0, 5), result('made_throw', **bullets), kept),
                                     ['made_throw'])[0]
        self.assertTrue(any('full 12' in reason for reason in fails), fails)
        full = kept.replace('moved=7', 'moved=12')
        fails, _, _ = self.run_check(log(segment('made_throw', 0, 0, 5), result('made_throw', **bullets), full),
                                     ['made_throw'])[0]
        self.assertEqual(fails, [], fails)

    def test_missing_phase_a_line_fails(self):
        text = log(result('made_trace'))
        fails, _, _ = self.run_check(text, ['made_trace'])[0]
        self.assertTrue(any('phase (a)' in reason for reason in fails), fails)


class Cli(unittest.TestCase):
    def setUp(self):
        self.folder = tempfile.TemporaryDirectory()
        write_recipes(self.folder.name, {'made_trace': recipe(10.0)})
        self.log = Path(self.folder.name) / 'run.log'

    def tearDown(self):
        self.folder.cleanup()

    def main(self, text, *extra):
        self.log.write_text(text, encoding='utf-8')
        out = io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(io.StringIO()):
            code = check.main([str(self.log), '--items', self.folder.name, *extra])
        return code, out.getvalue()

    def test_exit_codes(self):
        code, out = self.main(log('LogTemp: Display: OpenWillow firing test parsed 1 recipe IDs: made_trace',
                                  segment('made_trace', 0, 0, 5), push('made_trace'), result('made_trace'),
                                  'OWFIRING grenade thrown=1 ammo_before=3 ammo_after=2 refused_empty=1 damage=9.00 splash_hits=1 splash=9.00 grenade_hits=1 '
                                  'unknown_sources=0 contact_age=0.20 fuse_age=2.00 contact_fuse=1.80 dummy=Dummy_0 health_before=500.00 health_after=491.00 dummy_died=0 aimed=1 after=0.50',
                                  *ROLLED_LINES, ROLLED_DAMAGE_LINE))
        self.assertEqual(code, 0, out)
        self.assertIn('1 PASS, 0 FAIL of 1 guns', out)
        self.assertIn('grenade: PASS', out)
        self.assertIn('rolled damage: PASS', out)
        self.assertIn('rolled: PASS', out)
        code, out = self.main(log('LogTemp: Display: OpenWillow firing test parsed 1 recipe IDs: made_trace',
                                  segment('made_trace', 0, 0, 5), push('made_trace'), result('made_trace')))
        self.assertEqual(code, 1, out)
        self.assertIn('no OWFIRING grenade line', out)
        self.assertIn('no OWFIRING rolled_damage line', out)
        self.assertIn('no OWFIRING rolled line', out)
        code, out = self.main(log(result('made_trace', direct='1.00')), '--guns', 'made_trace')
        self.assertEqual(code, 1, out)
        code, _ = self.main('no firing lines here\n', '--guns', 'made_trace')
        self.assertEqual(code, 2)


ROLLED_DAMAGE_LINE = ('LogTemp: Display: OWFIRING rolled_damage id=gen_a has_stack=1 skills_empty=1 damage=20.0000 rebuilt=20.0000 '
                      'zero=20.0000 active=36.0000 expired=20.0000 expected=36.0000 stand_in=40.0000')


class RolledDamage(unittest.TestCase):
    GOOD = {'id': 'gen_a', 'has_stack': '1', 'skills_empty': '1', 'damage': '20.0000', 'rebuilt': '20.0000', 'zero': '20.0000',
            'active': '36.0000', 'expired': '20.0000', 'expected': '36.0000', 'stand_in': '40.0000'}

    def fails(self, **change):
        return check.check_rolled_damage(dict(self.GOOD, **change))

    def test_good_line_passes(self):
        self.assertEqual(self.fails(), [])

    def test_read_from_the_log(self):
        extra = {}
        check.read_log(ROLLED_DAMAGE_LINE + '\n', extra)
        self.assertEqual(check.check_rolled_damage(extra['rolled_damage']), [])

    def test_the_old_stand_in_fails(self):
        # What rolled guns did before: card damage x (1 + Scale).
        self.assertTrue(any('stack formula' in reason for reason in self.fails(active='40.0000')))

    def test_a_missing_stack_fails(self):
        self.assertTrue(any('no damage stack' in reason for reason in self.fails(has_stack='0')))

    def test_terms_that_do_not_rebuild_fail(self):
        self.assertTrue(any('rebuild' in reason for reason in self.fails(rebuilt='25.0000')))

    def test_zero_and_expired_scales(self):
        self.assertTrue(any('no Scale' in reason for reason in self.fails(zero='36.0000')))
        self.assertTrue(any('expired' in reason for reason in self.fails(expired='36.0000')))

    def test_a_gun_that_cannot_tell_them_apart_fails(self):
        self.assertTrue(any('cannot tell' in reason for reason in self.fails(active='40.0000', expected='40.0000')))

    def test_running_skills_fail(self):
        self.assertTrue(any('skill' in reason for reason in self.fails(skills_empty='0')))

    def test_missing_lines_and_numbers(self):
        self.assertTrue(check.check_rolled_damage(None))
        self.assertTrue(any('weapon_gen_manifest.py' in reason for reason in check.check_rolled_damage({'unavailable': 'no_manifest'})))
        self.assertTrue(any('no number' in reason for reason in self.fails(active='nan')))
        line = dict(self.GOOD)
        del line['expired']
        self.assertTrue(any('no number' in reason for reason in check.check_rolled_damage(line)))


ROLLED_LINES = ('LogTemp: Display: OWFIRING rolled want=1 id=gen_a type=Pistol burst=1 from_manifest=1 interval=0.100 hold=0.400 firing_loaded=1 magazine=6 shots=1 launches=0 damage=100.00 expected_damage=100.00 dummy_died=0 dummy_max=20000.00 pool_min=2.0000 pool_max=12.0000 pool_rate=-8.0000 card_min=2.0000 card_max=12.0000 card_rate=-8.0000 base_min=2.0000 base_max=12.0000 base_rate=-8.0000',
                'LogTemp: Display: OWFIRING rolled want=0 id=gen_b type=Assault_Rifle burst=0 from_manifest=1 interval=0.100 hold=0.400 firing_loaded=1 magazine=20 shots=4 launches=0 damage=400.00 expected_damage=400.00 dummy_died=0 dummy_max=20000.00 pool_min=2.0000 pool_max=12.0000 pool_rate=-8.0000 card_min=2.0000 card_max=12.0000 card_rate=-8.0000 base_min=2.0000 base_max=12.0000 base_rate=-8.0000',
                'LogTemp: Display: OWFIRING rolled want=-1 id=gen_c type=Launcher burst=0 from_manifest=1 firing_loaded=1 interval=0.500 hold=1.600 magazine=4 shots=2 launches=2 impacts=2 damage=50.00 expected_damage=50.00 dummy_died=0 dummy_max=20000.00 pool_min=2.0000 pool_max=12.0000 pool_rate=-8.0000 card_min=2.0000 card_max=12.0000 card_rate=-8.0000 base_min=2.0000 base_max=12.0000 base_rate=-8.0000',
                'LogTemp: Display: OWFIRING rolled want=-2 id=gen_d type=Pistol burst=0 from_manifest=1 firing_loaded=1 interval=0.200 hold=0.700 magazine=9 shots=3 launches=3 impacts=3 damage=9.00 expected_damage=9.00 dummy_died=0 dummy_max=20000.00 pool_min=1.5000 pool_max=15.0000 pool_rate=-6.4000 card_min=1.5000 card_max=15.0000 card_rate=-6.4000 base_min=2.0000 base_max=12.0000 base_rate=-8.0000')


POOL_FIELDS = {'pool_min': '2.0000', 'pool_max': '12.0000', 'pool_rate': '-8.0000', 'card_min': '2.0000', 'card_max': '12.0000',
               'card_rate': '-8.0000', 'base_min': '2.0000', 'base_max': '12.0000', 'base_rate': '-8.0000'}


class RolledLines(unittest.TestCase):
    ONE = {'want': '1', 'id': 'gen_a', 'type': 'Pistol', 'burst': '1', 'from_manifest': '1', 'interval': '0.100', 'hold': '0.400', 'firing_loaded': '1', 'magazine': '6', 'shots': '1', 'launches': '0', 'damage': '100.00', 'expected_damage': '100.00', 'dummy_died': '0', 'dummy_max': '20000.00', **POOL_FIELDS}
    ZERO = {'want': '0', 'id': 'gen_b', 'type': 'Assault_Rifle', 'burst': '0', 'from_manifest': '1', 'interval': '0.100', 'hold': '0.400', 'firing_loaded': '1', 'magazine': '20', 'shots': '4', 'launches': '0', 'damage': '400.00', 'expected_damage': '400.00', 'dummy_died': '0', 'dummy_max': '20000.00', **POOL_FIELDS}
    ROCKET = {'want': '-1', 'id': 'gen_c', 'type': 'Launcher', 'burst': '0', 'from_manifest': '1', 'interval': '0.500', 'hold': '1.600', 'firing_loaded': '1', 'magazine': '4', 'shots': '2', 'launches': '2', 'impacts': '2', 'damage': '50.00', 'expected_damage': '50.00', 'dummy_died': '0', 'dummy_max': '20000.00', **POOL_FIELDS}
    MOVED = {'want': '-2', 'id': 'gen_d', 'type': 'Pistol', 'burst': '0', 'from_manifest': '1', 'interval': '0.200', 'hold': '0.700', 'firing_loaded': '1',
             'magazine': '9', 'shots': '3', 'launches': '3', 'impacts': '3', 'damage': '9.00', 'expected_damage': '9.00', 'dummy_died': '0', 'dummy_max': '20000.00', 'pool_min': '1.5000', 'pool_max': '15.0000', 'pool_rate': '-6.4000',
             'card_min': '1.5000', 'card_max': '15.0000', 'card_rate': '-6.4000', 'base_min': '2.0000', 'base_max': '12.0000', 'base_rate': '-8.0000'}

    def fails(self, one=None, zero=None, rocket=None, moved=None):
        return check.check_rolled([dict(self.ONE, **(one or {})), dict(self.ZERO, **(zero or {})), dict(self.ROCKET, **(rocket or {})), dict(self.MOVED, **(moved or {}))])

    def test_good_lines_pass(self):
        self.assertEqual(self.fails(), [])

    def test_read_from_the_log(self):
        extra = {}
        check.read_log('\n'.join(ROLLED_LINES) + '\n', extra)
        self.assertEqual([row['want'] for row in extra['rolled']], ['1', '0', '-1', '-2'])
        self.assertEqual(check.check_rolled(extra['rolled']), [])

    def test_the_original_bug_fails(self):
        # What the playtest saw: a burst-1 type firing on while held.
        self.assertTrue(any('8 shots' in reason for reason in self.fails(one={'shots': '8'})))

    def test_burst_zero_needs_more_than_one(self):
        self.assertTrue(any('1 shots' in reason for reason in self.fails(zero={'shots': '1'})))

    def test_burst_one_needs_a_shot(self):
        self.assertTrue(self.fails(one={'shots': '0'}))

    def test_a_short_hold_fails_even_with_one_shot(self):
        self.assertTrue(any('does not cover' in reason for reason in self.fails(one={'hold': '0.150'})))
        for bad in ('nan', 'x', '0.000'):
            self.assertTrue(self.fails(one={'interval': bad}), bad)

    def test_stale_or_missing_manifest_names_the_tool(self):
        for reason in ('stale_manifest', 'no_manifest'):
            fails = check.check_rolled([{'want': '1', 'unavailable': reason}, dict(self.ZERO), dict(self.ROCKET), dict(self.MOVED)])
            self.assertTrue(any('weapon_gen_manifest.py' in text and reason in text for text in fails), reason)
        self.assertTrue(any('weapon_gen_manifest.py' in text for text in self.fails(one={'from_manifest': '0'})))

    def test_other_unavailable_reasons_fail(self):
        self.assertTrue(check.check_rolled([{'want': '1', 'unavailable': 'not_equipped'}, dict(self.ZERO), dict(self.ROCKET), dict(self.MOVED)]))

    def test_missing_templates_name_the_tool(self):
        fails = check.check_rolled([{'want': '1', 'unavailable': 'no_firing_templates'}, dict(self.ZERO), dict(self.ROCKET), dict(self.MOVED)])
        self.assertTrue(any('weapon_gen_firing.py' in text for text in fails))
        self.assertTrue(any('weapon_gen_firing.py' in text for text in self.fails(zero={'firing_loaded': '0'})))

    def test_a_rolled_launcher_must_launch(self):
        self.assertTrue(any('projectiles launched' in text for text in self.fails(rocket={'launches': '0'})))

    def test_a_missed_shot_on_a_live_dummy_fails(self):
        # What H16 showed before each row reset the dummy: 3 of 4 hits.
        self.assertTrue(any('should deal' in text for text in self.fails(zero={'damage': '300.00'})))

    def test_a_dummy_that_was_sent_its_health_and_died_passes(self):
        died = {'damage': '23434.40', 'expected_damage': '23434.40', 'dummy_died': '1'}
        self.assertEqual(self.fails(rocket=died), [])

    def test_a_dummy_that_died_too_early_fails(self):
        self.assertTrue(any('full health' in text for text in self.fails(rocket={'damage': '17575.80', 'expected_damage': '23434.40', 'dummy_died': '1'})))

    def test_more_than_the_shots_should_deal_fails(self):
        self.assertTrue(self.fails(rocket={'damage': '30000.00', 'expected_damage': '23434.40', 'dummy_died': '1'}))

    def test_damage_over_time_is_not_said(self):
        self.assertEqual(self.fails(zero={'damage': '500.00', 'expected_damage': '-1.00'}), [])

    def test_missing_damage_numbers_fail(self):
        for field in ('damage', 'expected_damage', 'dummy_max', 'dummy_died'):
            line = dict(self.ZERO)
            del line[field]
            fails = check.check_rolled([dict(self.ONE), line, dict(self.ROCKET), dict(self.MOVED)])
            self.assertTrue(any('no damage numbers' in text for text in fails), field)

    def test_the_same_gun_must_deal_the_same(self):
        # H16's Bandit rows: the same gun and shots, 2921.56 and 2191.17. Only -1 expected, so the per-row rule can't catch it here.
        lines = [dict(self.ONE), dict(self.ZERO, id='same', damage='2921.56', expected_damage='-1.00'), dict(self.ROCKET),
                 dict(self.MOVED, id='same', shots='4', damage='2191.17', expected_damage='-1.00')]
        self.assertTrue(any('same gun' in text for text in check.check_rolled(lines)))

    def test_the_idle_rate_sign_is_checked(self):
        self.assertTrue(any('another sign' in text for text in self.fails(one={'pool_rate': '0.0004', 'card_rate': '-0.0004'})))

    def test_one_rule_for_near(self):
        self.assertTrue(check.pool_near(2.0, 2.0019))
        self.assertFalse(check.pool_near(2.0, 2.0021))
        self.assertTrue(check.pool_near(0.0, 0.0009))
        self.assertFalse(check.pool_near(0.0, 0.0011))

    def test_a_skipped_pool_merge_fails(self):
        # What the host does with -owrolledskippoolmerge: the held gun keeps the template's base.
        skipped = {'pool_min': '2.0000', 'pool_max': '12.0000', 'pool_rate': '-8.0000'}
        self.assertTrue(any('is not its card' in text and 'gen_d' in text for text in self.fails(moved=skipped)))

    def test_a_gun_on_the_base_cannot_show_a_skipped_merge(self):
        on_base = {'card_min': '2.0000', 'card_max': '12.0000', 'card_rate': '-8.0000', 'pool_min': '2.0000', 'pool_max': '12.0000', 'pool_rate': '-8.0000'}
        self.assertTrue(any('cannot show' in text for text in self.fails(moved=on_base)))

    def test_any_held_gun_off_its_card_fails(self):
        self.assertTrue(any('is not its card' in text for text in self.fails(one={'pool_min': '3.0000'})))
        self.assertTrue(any('is not its card' in text for text in self.fails(rocket={'pool_rate': '-1.0000'})))

    def test_missing_pool_values_fail(self):
        self.assertTrue(any('on the held gun' in text for text in self.fails(zero={'pool_max': 'nan'})))
        self.assertTrue(any('weapon_gen_manifest.py' in text for text in self.fails(zero={'card_min': 'nan'})))
        self.assertTrue(self.fails(moved={'base_min': 'nan'}))

    def test_the_pool_gun_is_required(self):
        self.assertTrue(any('off the base' in text for text in check.check_rolled([dict(self.ONE), dict(self.ZERO), dict(self.ROCKET)])))

    def test_a_launch_alone_is_not_enough(self):
        self.assertTrue(any('no impact or damage' in text for text in self.fails(rocket={'impacts': '0', 'damage': '0.00'})))
        self.assertTrue(any('no impact or damage' in text for text in self.fails(rocket={'impacts': 'nan', 'damage': 'nan'})))
        self.assertEqual(self.fails(rocket={'impacts': '1', 'damage': '0.00', 'expected_damage': '-1.00'}), [])
        self.assertEqual(self.fails(rocket={'impacts': '0', 'damage': '5.00', 'expected_damage': '-1.00'}), [])

    def test_the_launcher_line_still_needs_a_launch(self):
        self.assertTrue(check.check_rolled([dict(self.ONE), dict(self.ZERO)]))

    def test_a_missing_gun_fails(self):
        self.assertTrue(check.check_rolled([dict(self.ZERO)]))
        self.assertTrue(check.check_rolled([]))
        self.assertTrue(self.fails(one={'burst': '0'}))


def push(gun, rule=1000, last=1000, hits=5, pushes=5, moved=0.4):
    return (f'LogTemp: Display: OWFIRING push id={gun} momentum_rule={rule} momentum_last={last} momentum_hits={hits} '
            f'pushes={pushes} mass=100.0 moved={moved}')


def wide(gun, line_hits=0, box_hits=1):
    return f'LogTemp: Display: OWFIRING wide id={gun} offset=40.0 half=24.0 line_hits={line_hits} box_hits={box_hits}'


class GrenadeLine(unittest.TestCase):
    GOOD = {'thrown': '1', 'ammo_before': '3', 'ammo_after': '2', 'refused_empty': '1', 'damage': '100.00', 'splash_hits': '1', 'splash': '100.00',
            'grenade_hits': '1', 'unknown_sources': '0', 'contact_age': '0.20', 'fuse_age': '2.00', 'contact_fuse': '1.80', 'first_contact': 'Dummy_0',
            'dummy': 'Dummy_0', 'health_before': '500.00', 'health_after': '400.00', 'dummy_died': '0', 'aimed': '1', 'after': '0.50'}

    def test_good_line_passes(self):
        self.assertEqual(check.check_grenade(dict(self.GOOD)), [])

    def test_read_from_the_log(self):
        extra = {}
        check.read_log('LogTemp: Display: OWFIRING grenade ' + ' '.join(f'{k}={v}' for k, v in self.GOOD.items()) + '\n', extra)
        self.assertEqual(extra['grenade']['thrown'], '1')

    def test_failures(self):
        self.assertTrue(check.check_grenade(None))
        self.assertTrue(check.check_grenade(dict(self.GOOD, thrown='0')))
        self.assertTrue(check.check_grenade(dict(self.GOOD, ammo_after='3')))
        self.assertTrue(check.check_grenade(dict(self.GOOD, refused_empty='0')))
        for field in ('damage', 'splash'):
            for bad in ('0.00', '-1.00', None):
                line = dict(self.GOOD, **{field: bad})
                if bad is None:
                    del line[field]
                self.assertTrue(any('not positive' in reason for reason in check.check_grenade(line)), (field, bad))
        self.assertTrue(check.check_grenade(dict(self.GOOD, splash_hits='0', grenade_hits='0')))
        self.assertTrue(check.check_grenade(dict(self.GOOD, grenade_hits='0')))
        self.assertTrue(check.check_grenade(dict(self.GOOD, unknown_sources='1')))

    def test_a_miss_names_what_the_grenade_touched(self):
        reasons = check.check_grenade(dict(self.GOOD, splash_hits='0', grenade_hits='0', splash='0.00', first_contact='Floor_7'))
        self.assertTrue(any('splashed nothing' in reason and 'Floor_7' in reason for reason in reasons), reasons)

    def test_contact_fuse_is_required(self):
        # An immediate contact explosion: both ages equal, which a missing field read as 0 would have passed.
        immediate = dict(self.GOOD, contact_age='0.20', fuse_age='0.20')
        del immediate['contact_fuse']
        self.assertTrue(any('contact fuse' in reason for reason in check.check_grenade(immediate)))
        for bad in ('0.000', '-1.000', 'nan', 'inf', 'x'):
            line = dict(self.GOOD, contact_age='0.20', fuse_age='0.20', contact_fuse=bad)
            self.assertTrue(any('contact fuse' in reason for reason in check.check_grenade(line)), bad)

    def test_non_finite_ages_fail(self):
        for field in ('contact_age', 'fuse_age'):
            self.assertTrue(check.check_grenade(dict(self.GOOD, **{field: 'nan'})), field)

    def test_the_dummy_must_lose_health(self):
        reasons = check.check_grenade(dict(self.GOOD, health_after='500.00'))
        self.assertTrue(any('Dummy_0' in reason and 'lost no health' in reason for reason in reasons), reasons)
        self.assertTrue(check.check_grenade(dict(self.GOOD, health_after='501.00')))

    def test_a_dummy_that_died_and_respawned_passes(self):
        self.assertEqual(check.check_grenade(dict(self.GOOD, health_after='500.00', dummy_died='1')), [])

    def test_a_dummy_dead_before_the_throw_fails(self):
        self.assertTrue(check.check_grenade(dict(self.GOOD, health_before='0.00', health_after='0.00', dummy_died='1')))

    def test_missing_health_fields_fail(self):
        for field in ('health_before', 'health_after', 'dummy_died'):
            line = dict(self.GOOD)
            del line[field]
            self.assertTrue(any('no health read' in reason for reason in check.check_grenade(line)), field)

    def test_a_skipped_aim_fails(self):
        for bad in ('0', None):
            line = dict(self.GOOD, aimed=bad)
            if bad is None:
                del line['aimed']
            self.assertTrue(any('aim' in reason for reason in check.check_grenade(line)), bad)

    def test_falloff_fails(self):
        self.assertTrue(any('falloff' in reason for reason in check.check_grenade(dict(self.GOOD, splash='60.00'))))

    def test_fuse_from_the_first_contact(self):
        self.assertTrue(check.check_grenade(dict(self.GOOD, contact_age='-1.000')))
        self.assertTrue(check.check_grenade(dict(self.GOOD, fuse_age='-1.000')))
        self.assertTrue(check.check_grenade(dict(self.GOOD, fuse_age='6.00')))
        self.assertEqual(check.check_grenade(dict(self.GOOD, fuse_age='2.10')), [])


class Momentum(unittest.TestCase):
    def test_rule_is_100_times_the_type_value_or_the_class_default(self):
        self.assertEqual(check.expectations(recipe(10, firing={'instant_hit_momentum': 8}))['momentum'], 800)
        self.assertEqual(check.expectations(recipe(10, firing={'instant_hit_momentum': None}))['momentum'], 1000)

    def test_wide_only_for_single_straight_pellets(self):
        self.assertTrue(check.expectations(recipe(10, firing={'fire_type': None}))['wide'])
        self.assertFalse(check.expectations(recipe(10, projectiles=7, firing={'fire_type': None}))['wide'])
        self.assertFalse(check.expectations(recipe(10, firing={'fire_type': None, 'pattern_lines': [{}]}))['wide'])
        self.assertFalse(check.expectations(recipe(10, firing={'fire_type': None,
                                                              'projectile': {'air_friction': -6}}))['wide'])
        self.assertFalse(check.expectations(recipe(10, firing={'fire_type': 'EWWFT_Rocket'}))['wide'])
        self.assertFalse(check.expectations(recipe(10))['wide'])
        self.assertFalse(check.expectations(recipe(10, firing={'fire_type': None, 'burst_count': 3}))['wide'])

    def expected(self, **firing):
        return check.expectations(recipe(10, firing=dict({'fire_type': None, 'instant_hit_momentum': 8}, **firing)))

    def test_good_lines_pass(self):
        e = self.expected()
        self.assertEqual(check.check_momentum(fields(push('g', 800, 800)), fields(wide('g')), e, 5), [])

    def test_wrong_rule_wrong_velocity_and_no_move_fail(self):
        e = self.expected()
        self.assertTrue(check.check_momentum(fields(push('g', 1000, 1000)), fields(wide('g')), e, 5))
        self.assertTrue(check.check_momentum(fields(push('g', 800, 800, pushes=4)), fields(wide('g')), e, 5))
        self.assertTrue(check.check_momentum(fields(push('g', 800, 800, moved=0.0)), fields(wide('g')), e, 5))
        self.assertTrue(check.check_momentum(fields(push('g', 800, 800, hits=3, pushes=3)), fields(wide('g')), e, 5))

    def test_a_recipe_without_a_firing_block_names_stale_data(self):
        # The host logs momentum_rule 0 for a gun it holds without a firing block; the recipe the checker reads has none
        # either (class default 10 x 100 expected): the failure names the likely cause.
        stale = check.expectations(recipe(10))
        fails = check.check_momentum(fields(push('g', 0, 0)), fields(wide('g')), stale, 5)
        self.assertTrue(any('no firing block: stale local data' in reason for reason in fails), fails)
        fails = check.check_momentum(fields(push('g', 0, 0)), fields(wide('g')), self.expected(), 5)
        self.assertFalse(any('stale' in reason for reason in fails), fails)

    def test_wide_line_must_miss_and_box_must_hit(self):
        e = self.expected()
        good = fields(push('g', 800, 800))
        self.assertTrue(check.check_momentum(good, None, e, 5))
        self.assertTrue(any('inconclusive' in r for r in check.check_momentum(good, fields(wide('g', 1, 1)), e, 5)))
        self.assertTrue(any('missed' in r for r in check.check_momentum(good, fields(wide('g', 0, 0)), e, 5)))

    def test_late_bullets_fail(self):
        folder = tempfile.TemporaryDirectory()
        self.addCleanup(folder.cleanup)
        write_recipes(folder.name, {'made_trace': recipe(10.0)})
        results, segments, *_ = check.read_log(log(segment('made_trace', 0, 0, 5), result('made_trace', timed=8, on_time=7)))
        fails, _, _ = check.check_gun('made_trace', results['made_trace'], segments['made_trace'],
                                      check.expectations(recipe(10.0)), None, set())
        self.assertTrue(any('distance / speed' in reason for reason in fails), fails)

    def test_old_logs_without_push_lines_are_graded_without_momentum(self):
        extra = {}
        check.read_log(log(segment('x', 0, 0, 5), result('x')), extra)
        self.assertEqual(extra['push'], {})

    def test_wide_excludes_accelerating_bullets(self):
        accelerating = {'fire_type': None, 'projectile': {'acceleration': {'X': 500.0, 'Y': 0, 'Z': 0}}}
        self.assertFalse(check.expectations(recipe(10, firing=accelerating))['wide'])
        still = {'fire_type': None, 'projectile': {'acceleration': {'X': 0, 'Y': 0, 'Z': 0}}}
        self.assertTrue(check.expectations(recipe(10, firing=still))['wide'])


def splash_line(scale, momentum, source='shot'):
    return (f'LogTemp: Display: OWFIRING splash Dummy distance 120 scale {scale} momentum {momentum} source {source}')


class SplashMomentum(unittest.TestCase):
    def expected(self, momentum=10000.0, throw=0.0):
        firing = {'fire_type': 'EWWFT_Rocket', 'instant_hit_momentum': 10,
                  'projectile': {'explode_on_impact': True, 'splash': {'damage': 100, 'momentum': momentum}}}
        if throw:
            firing['reload_throw'] = {'projectile': {'splash': {'momentum': throw}}}
        return check.expectations(recipe(10, firing=firing))

    def lines(self, *body):
        extra = {}
        check.read_log(log(*body, result('g', kind='Rocket')), extra)
        return extra['splash']['g']

    def test_recipe_times_scale_passes(self):
        lines = self.lines(splash_line(1.0, 10000.0), splash_line(0.604, 6040.0))
        self.assertEqual(check.check_momentum(fields(push('g')), None, self.expected(), 0, lines), [])

    def test_zero_or_wrong_splash_momentum_fails(self):
        for bad in (splash_line(1.0, 0.0), splash_line(0.5, 10000.0)):
            fails = check.check_momentum(fields(push('g')), None, self.expected(), 0, self.lines(bad))
            self.assertTrue(any('splash momentum' in reason for reason in fails), fails)

    def test_exploding_shots_without_splash_lines_fail(self):
        fails = check.check_momentum(fields(push('g')), None, self.expected(), 0, self.lines())
        self.assertTrue(any('no OWFIRING splash line' in reason for reason in fails), fails)

    def test_throw_lines_use_the_throw_momentum(self):
        lines = self.lines(splash_line(1.0, 10000.0), splash_line(0.5, 250.0, 'throw'))
        self.assertEqual(check.check_momentum(fields(push('g')), None, self.expected(throw=500.0), 0, lines), [])

    def test_splash_lines_belong_to_the_next_result(self):
        extra = {}
        check.read_log(log(splash_line(1.0, 1.0), result('a'), splash_line(1.0, 2.0), splash_line(1.0, 3.0), result('b')),
                       extra)
        self.assertEqual([line['momentum'] for line in extra['splash']['a']], [1.0])
        self.assertEqual([line['momentum'] for line in extra['splash']['b']], [2.0, 3.0])


class SplashCountsAndFinite(unittest.TestCase):
    def tediore(self):
        firing = {'fire_type': None, 'instant_hit_momentum': 5,
                  'reload_throw': {'projectile': {'splash': {'momentum': 0}}}}
        return check.expectations(recipe(10, firing=firing))

    def lines(self, *body):
        extra = {}
        check.read_log(log(*body, result('g')), extra)
        return extra['splash']['g']

    def throw(self, splash_hits):
        return {'splash_hits': str(splash_hits)}

    def test_throw_splash_hit_without_a_line_fails_even_at_zero_momentum(self):
        e = self.tediore()
        fails = check.check_momentum(fields(push('g', 500, 500)), None, e, 5, self.lines(), 1, self.throw(1))
        self.assertTrue(any('splash lines' in reason for reason in fails), fails)
        good = self.lines(splash_line(0.6, 0.0, 'throw'))
        self.assertEqual(check.check_momentum(fields(push('g', 500, 500)), fields(wide('g')), e, 5, good, 1, self.throw(1)), [])

    def test_splash_line_count_must_equal_the_result_splash_hits(self):
        e = SplashMomentum().expected()
        lines = self.lines(splash_line(1.0, 10000.0))
        fails = check.check_momentum(fields(push('g')), None, e, 0, lines, 2)
        self.assertTrue(any('2 splash hits but 1' in reason for reason in fails), fails)

    def test_nan_values_fail(self):
        e = SplashMomentum().expected()
        cases = [
            (fields(push('g', rule='nan')), self.lines(splash_line(1.0, 10000.0))),
            (fields(push('g', last='nan')), self.lines(splash_line(1.0, 10000.0))),
            (fields(push('g', moved='nan')), self.lines(splash_line(1.0, 10000.0))),
            (fields(push('g')), self.lines(splash_line(1.0, 'nan'))),
            (fields(push('g')), self.lines(splash_line('nan', 10000.0))),
        ]
        for push_line, lines in cases:
            fails = check.check_momentum(push_line, None, e, 0, lines, len(lines))
            self.assertTrue(any('not a finite number' in reason for reason in fails), fails)

    def test_nan_expected_value_fails(self):
        e = dict(SplashMomentum().expected(), splash_momentum=float('nan'))
        fails = check.check_momentum(fields(push('g')), None, e, 0, self.lines(splash_line(1.0, 10000.0)), 1)
        self.assertTrue(any('not a finite number' in reason for reason in fails), fails)


class RequiredMarkers(unittest.TestCase):
    def setUp(self):
        self.folder = tempfile.TemporaryDirectory()
        write_recipes(self.folder.name, {'made_trace': recipe(10.0)})
        self.log = Path(self.folder.name) / 'run.log'
        self.log.write_text(log(segment('made_trace', 0, 0, 5), result('made_trace')), encoding='utf-8')

    def tearDown(self):
        self.folder.cleanup()

    def run_main(self, *extra):
        out = io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(io.StringIO()):
            code = check.main([str(self.log), '--items', self.folder.name, '--guns', 'made_trace', *extra])
        return code, out.getvalue()

    def test_missing_push_lines_fail_in_normal_mode(self):
        code, out = self.run_main()
        self.assertEqual(code, 1, out)
        self.assertIn('no OWFIRING push line', out)

    def test_legacy_flag_grades_an_old_log_without_them(self):
        code, out = self.run_main('--legacy')
        self.assertEqual(code, 0, out)


def fields(line):
    return check.fields(line.split('OWFIRING ', 1)[1].split(' ', 1)[1])


if __name__ == '__main__':
    unittest.main()
