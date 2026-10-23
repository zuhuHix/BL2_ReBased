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
        })

    def tearDown(self):
        self.folder.cleanup()

    def run_check(self, text, guns):
        results, segments, skipped, no_wall, _, throws, lifecycles = check.read_log(text)
        out = []
        for gun in guns:
            rec, _ = check.find_recipe(self.dirs, gun)
            expected = check.expectations(rec)
            out.append(check.check_gun(gun, results.get(gun), segments.get(gun), expected, skipped.get(gun), no_wall,
                                       throws.get(gun), lifecycles.get(gun)))
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
            result('made_rocket', kind='Rocket', shots=28, pellets=28, launches=28, impacts=28, target_hits=0,
                   damage='60.00', direct='0.00', splash='60.00', splash_hits=2, status_rolls=2, procs=1, ticks=3),
        )
        fails, notes, _ = self.run_check(text, ['made_rocket'])[0]
        self.assertEqual(fails, [], fails)
        self.assertTrue(any('rocket missed' in note for note in notes), notes)

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
        bullets = dict(kind='Bullet', launches=8, target_hits=9, direct='85.00', damage='85.00')
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
                                  segment('made_trace', 0, 0, 5), result('made_trace')))
        self.assertEqual(code, 0, out)
        self.assertIn('1 PASS, 0 FAIL of 1 guns', out)
        code, out = self.main(log(result('made_trace', direct='1.00')), '--guns', 'made_trace')
        self.assertEqual(code, 1, out)
        code, _ = self.main('no firing lines here\n', '--guns', 'made_trace')
        self.assertEqual(code, 2)


if __name__ == '__main__':
    unittest.main()
