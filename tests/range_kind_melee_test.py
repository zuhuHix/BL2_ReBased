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


def kind_line(kind='bullymong_adult', cycle=(1.25, 1.267), stand=(195, 201), first=380, start_range=394):
    return ('kind=%s cycle_min=%.3f cycle_max=%.3f stand_min=%.0f stand_max=%.0f first_swing=%.0f start_range=%.0f swings=4'
            % (kind, cycle[0], cycle[1], stand[0], stand[1], first, start_range))


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
        self.assertIn('const float StopAt = In.StandDistance > 0.f ? In.StandDistance', brain)
        self.assertNotIn('LastSwingStart + SwingInterval', brain)

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
        for field in ('kind=%s', 'cycle_min=%.3f', 'cycle_max=%.3f', 'stand_min=%.0f', 'stand_max=%.0f', 'first_swing=%.0f', 'start_range=%.0f'):
            self.assertIn(field, step)
        self.assertIn('kind_melee', check_range.EXPECTED)
        # C2181 (project memory): no brace-less if holding a UE_LOG before an else.
        self.assertNotRegex(step, r'UE_LOG\([^;]*\);\s*\n\s*else')


class CheckerTest(unittest.TestCase):
    def test_a_host_inside_the_bands_passes(self):
        self.assertEqual(check_range.kind_melee_problem(kind_line()), '')
        self.assertEqual(check_range.kind_melee_problem(kind_line('bullymong_baby', (1.133, 1.150), (100, 111), 470, 480)), '')

    def test_the_old_stand_ins_fail(self):
        # Before the table: the adult stopped 60 uu inside its swing range (334), the baby at 420, and the baby swung every 1.25 s.
        self.assertIn('stand', check_range.kind_melee_problem(kind_line(stand=(328, 334))))
        self.assertIn('cycle', check_range.kind_melee_problem(kind_line('bullymong_baby', (1.25, 1.267), (414, 420), 470, 480)))

    def test_the_den_reads_adult_cycle_is_outside_the_band_the_host_is_held_to(self):
        self.assertIn('cycle', check_range.kind_melee_problem(kind_line(cycle=(1.217, 1.217))))

    def test_a_first_swing_outside_the_stock_range_fails(self):
        self.assertIn('first swing', check_range.kind_melee_problem(kind_line(first=420)))

    def test_a_kind_without_a_row_or_a_line_without_numbers_fails(self):
        self.assertIn('no row', check_range.kind_melee_problem(kind_line('bullymong')))
        self.assertIn('lacks', check_range.kind_melee_problem('kind=bullymong_adult cycle_min=1.25'))

    def test_a_host_pass_with_numbers_outside_the_bands_is_graded_fail(self):
        lines = ['OWRANGETEST step=%s result=PASS %s' % (s, kind_line(stand=(328, 334)) if s == 'kind_melee' else 'x=1') for s in check_range.EXPECTED]
        lines.append('OWRANGETEST SUMMARY result=PASS pass=1 fail=0 skip=0')
        ok, rows = check_range.grade(*check_range.parse('\n'.join(lines) + '\n'))
        self.assertFalse(ok)
        self.assertIn('checker:', dict((s, d) for s, _, d in rows)['kind_melee'])


if __name__ == '__main__':
    unittest.main()
