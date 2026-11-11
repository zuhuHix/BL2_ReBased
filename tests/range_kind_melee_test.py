"""The range's melee per kind against the acceptance table (docs/verification/COMBAT_RANGE.md, "Acceptance table: melee per kind").

Two halves, no editor: the host's per-kind rows (OpenWillowEnemyAI.cpp, KindMeleeRows) must equal tools/check_range.py's ACCEPTANCE, and
the brain must use them; and the checker's re-check of a kind_melee line must accept a host inside the bands and refuse the old stand-ins.
Source-text and synthetic checks only: nothing here runs the host. Each row is one session's live read, UNVERIFIED beyond it.

Run: python tests/range_kind_melee_test.py
"""
import re
import sys
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
SOURCE = REPO / 'host' / 'ue5' / 'OpenWillow' / 'Source' / 'OpenWillow'
sys.path.insert(0, str(REPO / 'tools'))
import check_range  # noqa: E402

ROW = re.compile(r'\{TEXT\("(\w+)"\),\s*([\d.]+)f,\s*([\d.]+)f\}')


def text(name):
    return (SOURCE / name).read_text(encoding='utf-8')


def host_rows():
    """{kind: (cycle, stand)} from the KindMeleeRows table; {} when the table is not there (the code before the acceptance table)."""
    source = text('OpenWillowEnemyAI.cpp')
    if 'KindMeleeRows[] = {' not in source:
        return {}
    block = source[source.index('KindMeleeRows[] = {'):]
    block = block[:block.index('};')]
    return {kind: (float(cycle), float(stand)) for kind, cycle, stand in ROW.findall(block)}


def kind_line(kind='bullymong_adult', cycle=(1.25, 1.267), stand=(201, 201), first=380, start_range=394, max_frame=0.017, enemy='E_3', swings=4):
    return ('kind=%s enemy=%s cycle_min=%.3f cycle_max=%.3f stand_min=%.0f stand_max=%.0f first_swing=%.0f start_range=%.0f max_frame=%.3f swings=%s'
            % (kind, enemy, cycle[0], cycle[1], stand[0], stand[1], first, start_range, max_frame, swings))


def spawn(enemy, kind, source='live_read'):
    return 'OWRANGE enemy %s kind_melee kind=%s swing_interval=1.250 stand=201 source=%s' % (enemy, kind, source)


def graded_kind_melee(*lines):
    """The checker's kind_melee row for a log of these lines (spawn records and results), every other step passing."""
    others = ['OWRANGETEST step=%s result=PASS x=1' % s for s in check_range.EXPECTED if s != 'kind_melee']
    text = '\n'.join(others + list(lines) + ['OWRANGETEST SUMMARY result=PASS pass=1 fail=0 skip=0']) + '\n'
    ok, rows = check_range.grade(*check_range.parse(text))
    return ok, dict((s, (v, d)) for s, v, d in rows)['kind_melee']


class HostRowsTest(unittest.TestCase):
    def test_the_host_has_a_row_for_every_kind_in_the_table_and_no_other(self):
        rows = host_rows()
        self.assertTrue(rows, 'OpenWillowEnemyAI.cpp has no KindMeleeRows table: every kind runs on the stand-ins (1.25 s, a stop 60 uu '
                              'inside its swing range: 334 uu for the adult and 420 for the baby with the manifest of 2026-11-07)')
        self.assertEqual(set(rows), set(check_range.ACCEPTANCE))

    def test_each_host_value_is_the_table_value_and_inside_its_band(self):
        rows = host_rows()
        for kind, row in check_range.ACCEPTANCE.items():
            with self.subTest(kind=kind):
                self.assertIn(kind, rows)
                cycle, stand = rows[kind]
                self.assertAlmostEqual(cycle, row['cycle'], places=6)
                self.assertAlmostEqual(stand, row['stand'], places=6)
                self.assertTrue(row['cycle_band'][0] <= cycle <= row['cycle_band'][1])
                self.assertTrue(row['stand_band'][0] <= stand <= row['stand_band'][1])

    def test_kinds_with_no_live_read_have_no_row(self):
        # The regular bullymong (three of the four default enemies), the slinger's melee and the psycho keep the stand-ins.
        for kind in ('bullymong', 'bullymong_slinger', 'psycho'):
            self.assertNotIn(kind, host_rows())
        self.assertIn('constexpr float SwingInterval = 1.25f;', text('OpenWillowEnemyAI.h'))

    def test_the_adult_cycle_stays_on_the_larger_read(self):
        # Two live reads disagree on the adult (AI_MELEE_TARGETING_LIVE 4: median 1.249 s, 34 gaps; the den read: 1.216 to 1.217 s, 9 gaps).
        # The host keeps the larger read's value until a read explains the difference.
        self.assertEqual(check_range.ACCEPTANCE['bullymong_adult']['cycle'], 1.25)
        self.assertGreater(check_range.ACCEPTANCE['bullymong_adult']['cycle_band'][0], 1.217)


class HostUsesTheRowsTest(unittest.TestCase):
    def test_the_brain_reads_cycle_and_stand_from_its_input(self):
        brain = text('OpenWillowEnemyAI.cpp')
        self.assertIn('Now >= LastSwingStart + In.SwingInterval', brain)
        self.assertIn('const bool bLiveStand = In.StandDistance > 0.f;', brain)
        self.assertNotIn('LastSwingStart + SwingInterval', brain)

    def test_a_live_stand_distance_never_goes_inside_body_contact_and_the_run_stops_on_it(self):
        brain = text('OpenWillowEnemyAI.cpp')
        self.assertIn('const float Contact = In.CollisionRadius + In.MayaRadius;', brain)
        self.assertIn('FMath::Max(In.StandDistance, Contact)', brain)
        # Review r2: the limit is the gap to where Maya is now (FlatDistance), not to ChaseTo, which lags by up to one state update.
        self.assertIn('Orders.StepLimit = (bAware && bLiveStand) ? FMath::Max(0.f, FlatDistance - StopAt) : -1.f;', brain)
        enemy = text('OpenWillowEnemy.cpp')
        self.assertIn('if (StepLimit > 0.f) Step = FMath::Min(Step, StepLimit);', enemy)
        self.assertIn('if (!bRun || StepLimit == 0.f)', enemy)
        self.assertNotIn('Left - StopShort', enemy)
        self.assertIn('is inside body contact', enemy)

    def test_the_enemy_fills_them_from_its_kind_and_says_where_they_came_from(self):
        enemy = text('OpenWillowEnemy.cpp')
        self.assertIn('KindMelee = FindKindMelee(Kind.Name);', enemy)
        self.assertIn('In.SwingInterval = SwingIntervalNow();', enemy)
        self.assertIn('In.StandDistance = KindMelee ? KindMelee->StandDistance : 0.f;', enemy)
        self.assertIn('stand-in_no_live_read', enemy)
        self.assertIn('MeleeEndsAt = Now + SwingIntervalNow();', enemy)

    def test_the_range_test_logs_what_the_checker_reads(self):
        source = text('OpenWillowRange.cpp')
        step = source[source.index('case 94: // kind_melee'):source.index('// ---------------------------------------------------------------------------------------------- loot driver')]
        for field in ('kind=%s', 'cycle_min=%.3f', 'cycle_max=%.3f', 'stand_min=%.0f', 'stand_max=%.0f', 'first_swing=%.0f', 'start_range=%.0f',
                      'max_frame=%.3f'):
            self.assertIn(field, step)
        self.assertIn('kind_melee', check_range.EXPECTED)
        # C2181 (project memory): no brace-less if holding a UE_LOG before an else.
        self.assertNotRegex(step, r'UE_LOG\([^;]*\);\s*\n\s*else')


class CheckerTest(unittest.TestCase):
    def test_a_host_inside_the_bands_passes(self):
        self.assertEqual(check_range.kind_melee_problem(kind_line()), '')
        self.assertEqual(check_range.kind_melee_problem(kind_line('bullymong_baby', (1.133, 1.140), (111, 111), 470, 480)), '')

    def test_the_old_stand_ins_fail(self):
        # Before the table: the adult stopped 60 uu inside its swing range (334), the baby at 420, and the baby swung every 1.25 s.
        self.assertIn('stand', check_range.kind_melee_problem(kind_line(stand=(328, 334))))
        self.assertIn('cycle', check_range.kind_melee_problem(kind_line('bullymong_baby', (1.25, 1.267), (414, 420), 470, 480)))

    def test_the_cycle_allowance_is_the_logged_longest_frame(self):
        # A slow editor frame makes a swing start late by up to that frame; the band's upper edge moves by exactly that much.
        self.assertIn('cycle', check_range.kind_melee_problem(kind_line('bullymong_baby', (1.133, 1.170), (111, 111), 470, 480, 0.017)))
        self.assertEqual(check_range.kind_melee_problem(kind_line('bullymong_baby', (1.133, 1.170), (111, 111), 470, 480, 0.05)), '')
        self.assertIn('max_frame', check_range.kind_melee_problem(kind_line(max_frame=-0.1)))
        self.assertIn('max_frame', check_range.kind_melee_problem(kind_line(max_frame=0)))

    def test_a_stall_matters_only_when_the_allowance_is_needed(self):
        # Review r2: one editor hitch in the step must not fail cycles that sit inside the plain band.
        self.assertEqual(check_range.kind_melee_problem(kind_line(cycle=(1.250, 1.267), max_frame=0.25)), '')
        problem = check_range.kind_melee_problem(kind_line('bullymong_baby', (1.133, 1.170), (111, 111), 470, 480, 0.25))
        self.assertIn('stalled run, repeat it', problem)
        self.assertIn('lacks', check_range.kind_melee_problem(kind_line().replace(' max_frame=0.017', '')))

    def test_a_stand_inside_the_measured_band_needs_no_frame_allowance(self):
        # The host clamps the run's last step to the stand distance, so a frame cannot carry it inside the band.
        self.assertIn('stand', check_range.kind_melee_problem(kind_line(stand=(187, 201), max_frame=0.05)))

    def test_the_den_reads_adult_cycle_is_outside_the_band_the_host_is_held_to(self):
        self.assertIn('cycle', check_range.kind_melee_problem(kind_line(cycle=(1.217, 1.217))))

    def test_a_first_swing_outside_the_stock_range_fails(self):
        self.assertIn('first swing', check_range.kind_melee_problem(kind_line(first=420)))

    def test_a_kind_without_a_row_or_a_line_without_numbers_fails(self):
        self.assertIn('no row', check_range.kind_melee_problem(kind_line('bullymong')))
        self.assertIn('lacks', check_range.kind_melee_problem('kind=bullymong_adult cycle_min=1.25'))

    def test_incomplete_or_impossible_evidence_fails(self):
        # Review r3 (second family): each of these passed before.
        problem = check_range.kind_melee_problem
        self.assertIn('lacks swings', problem(kind_line().replace(' swings=4', '')))
        self.assertIn('swings=0', problem(kind_line(swings=0)))
        self.assertIn('swings=3', problem(kind_line(swings=3)))
        self.assertIn('swings=4.5', problem(kind_line(swings='4.5')))
        self.assertIn('minimum is above its maximum', problem(kind_line(cycle=(99, 0))))
        self.assertIn('minimum is above its maximum', problem(kind_line(stand=(9999, 0))))
        self.assertIn('repeats stand_min', problem(kind_line() + ' stand_min=201'))
        self.assertIn('repeats kind', problem(kind_line() + ' kind=bullymong_baby'))
        self.assertIn('not a finite number', problem(kind_line().replace('stand_max=201', 'stand_max=inf')))
        self.assertIn('not a finite number', problem(kind_line().replace('cycle_min=1.250', 'cycle_min=nan')))
        self.assertIn('not a number', problem(kind_line().replace('first_swing=380', 'first_swing=far')))
        self.assertIn('lacks enemy', problem(kind_line().replace(' enemy=E_3', '')))


class ResultsTiedToSpawnsTest(unittest.TestCase):
    def test_one_result_per_spawned_kind_passes(self):
        ok, (verdict, _) = graded_kind_melee(spawn('E_0', 'bullymong', 'stand-in_no_live_read'), spawn('E_3', 'bullymong_adult'),
                                             'OWRANGETEST step=kind_melee result=PASS ' + kind_line())
        self.assertTrue(ok)
        self.assertEqual(verdict, 'PASS')

    def test_a_result_for_a_kind_that_was_not_spawned_fails(self):
        ok, (verdict, detail) = graded_kind_melee(spawn('E_3', 'bullymong_adult'),
                                                  'OWRANGETEST step=kind_melee result=PASS ' + kind_line(),
                                                  'OWRANGETEST step=kind_melee result=PASS ' + kind_line('bullymong_baby', (1.133, 1.140), (111, 111), 470, 480, enemy='E_9'))
        self.assertFalse(ok)
        self.assertIn('was not spawned as bullymong_baby', detail)

    def test_a_two_kind_run_missing_one_result_fails(self):
        ok, (verdict, detail) = graded_kind_melee(spawn('E_3', 'bullymong_adult'), spawn('E_4', 'bullymong_baby'),
                                                  'OWRANGETEST step=kind_melee result=PASS ' + kind_line())
        self.assertFalse(ok)
        self.assertIn('0 passing results for spawned kind bullymong_baby', detail)

    def test_a_duplicated_result_fails(self):
        line = 'OWRANGETEST step=kind_melee result=PASS ' + kind_line()
        ok, (verdict, detail) = graded_kind_melee(spawn('E_3', 'bullymong_adult'), line, line)
        self.assertFalse(ok)
        self.assertIn('2 passing results for spawned kind bullymong_adult', detail)

    def test_a_result_from_an_enemy_of_another_kind_fails(self):
        ok, (verdict, detail) = graded_kind_melee(spawn('E_0', 'bullymong', 'stand-in_no_live_read'), spawn('E_3', 'bullymong_adult'),
                                                  'OWRANGETEST step=kind_melee result=PASS ' + kind_line(enemy='E_0'))
        self.assertFalse(ok)
        self.assertIn('enemy E_0 was not spawned as bullymong_adult', detail)

    def test_no_spawn_record_fails(self):
        ok, (verdict, detail) = graded_kind_melee('OWRANGETEST step=kind_melee result=PASS ' + kind_line())
        self.assertFalse(ok)
        self.assertIn('its spawn record: None', detail)
        details = ' '.join(d for _, d in check_range.kind_melee_graded([('PASS', kind_line())], {}))
        self.assertIn('no enemy was spawned with a live melee row', details)
        # Steps built by hand, not read from a log, carry no spawns: they fail too.
        self.assertFalse(check_range.grade({'kind_melee': [('PASS', kind_line())]}, ('PASS', ''), expected=['kind_melee'])[0])

    def test_the_host_logs_the_spawn_record_the_checker_reads(self):
        enemy = text('OpenWillowEnemy.cpp')
        self.assertIn('OWRANGE enemy %s kind_melee kind=%s swing_interval=%.3f stand=%s source=%s', enemy)
        self.assertIn('TEXT("live_read")', enemy)
        self.assertIn('kind=%s enemy=%s cycle_min=%.3f', text('OpenWillowRange.cpp'))

    def test_a_host_pass_with_numbers_outside_the_bands_is_graded_fail(self):
        ok, (verdict, detail) = graded_kind_melee(spawn('E_3', 'bullymong_adult'), 'OWRANGETEST step=kind_melee result=PASS ' + kind_line(stand=(328, 334)))
        self.assertFalse(ok)
        self.assertEqual(verdict, 'FAIL')
        self.assertIn('checker:', detail)


if __name__ == '__main__':
    unittest.main()
