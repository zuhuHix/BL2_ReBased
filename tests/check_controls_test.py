"""tools/check_controls.py on invented logs. No game data."""
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / 'tools'))
import check_controls as cc


def log(rows=None, summary=None):
    rows = list(cc.EXPECTED) if rows is None else rows
    lines = [f'LogTemp: Display: OWCONTROLS {name} PASS map=0 missions=0 page=0' for name in rows]
    if summary is None:
        summary = f'passed={len(rows)} failed=0'
    if summary:
        lines.append(f'LogTemp: Display: OWCONTROLS SUMMARY {summary}')
    return '\n'.join(lines)


class MeleeClipTest(unittest.TestCase):
    PRESS = 'LogTemp: Display: OWMELEE press t=12.50 duration={} clip={}\n'

    def test_the_clip_at_its_length_passes(self):
        self.assertEqual(cc.grade_melee_clip(self.PRESS.format('0.70', 1) * 2), [])

    def test_the_stand_in_fails_though_its_duration_matches(self):
        self.assertTrue(cc.grade_melee_clip(self.PRESS.format('0.70', 0)))

    def test_a_clip_of_another_length_fails(self):
        self.assertTrue(cc.grade_melee_clip(self.PRESS.format('1.20', 1)))

    def test_a_duration_that_is_not_finite_fails(self):
        for duration in ('nan', 'NaN', 'inf', '-inf'):
            self.assertTrue(cc.grade_melee_clip(self.PRESS.format(duration, 1)), duration)

    def test_a_malformed_press_fails_beside_a_valid_one(self):
        good = self.PRESS.format('0.70', 1)
        for bad in ('OWMELEE press t=12.50 duration=0.70\n', 'OWMELEE press t=12.50 duration=abc clip=1\n', 'OWMELEE press t=nan duration=0.70 clip=1\n',
                    'OWMELEE press t=12.50 duration=0.70 clip=1 extra\n', 'OWMELEE press t=12.50 duration=0.70 clip=2\n', 'OWMELEE press\n'):
            self.assertTrue(cc.grade_melee_clip(good + f'LogTemp: Display: {bad}' + good), bad)

    def test_a_refusal_line_is_not_a_press(self):
        refused = 'LogTemp: Display: OWMELEE press refused: swing in progress t=12.60\n'
        self.assertEqual(cc.grade_melee_clip(self.PRESS.format('0.70', 1) + refused), [])
        self.assertTrue(cc.grade_melee_clip(refused))

    def test_a_windows_line_ending_is_not_malformed(self):
        self.assertEqual(cc.grade_melee_clip(self.PRESS.format('0.70', 1).replace('\n', '\r\n')), [])

    def test_no_press_fails(self):
        self.assertTrue(cc.grade_melee_clip('LogTemp: Display: nothing\n'))

    def test_the_missing_clip_warning_fails(self):
        missing = 'LogTemp: Warning: OpenWillow arms animation not found: /Game/X/Anim_SirenCombat_Melee\n'
        self.assertTrue(cc.grade_melee_clip(missing + self.PRESS.format('0.70', 1)))

    def test_another_clip_missing_is_not_this_ones_problem(self):
        other = 'LogTemp: Warning: OpenWillow arms animation not found: /Game/X/Anim_SirenCombat_Phase_Lock_Fail\n'
        self.assertEqual(cc.grade_melee_clip(other + self.PRESS.format('0.70', 1)), [])


class CheckControlsTest(unittest.TestCase):
    def test_a_full_run_passes(self):
        self.assertEqual(cc.grade(log())[1], [])

    def test_a_lost_row_fails_even_with_failed_zero(self):
        problems = cc.grade(log(list(cc.EXPECTED)[:-1]))[1]
        self.assertTrue(any('appears 0 times' in p for p in problems), problems)
        self.assertTrue(any('summary passed=' in p for p in problems), problems)

    def test_a_renamed_row_fails(self):
        rows = ['other_name' if name == 'm_opens_map' else name for name in cc.EXPECTED]
        problems = cc.grade(log(rows))[1]
        self.assertTrue(any('unexpected row other_name' in p for p in problems), problems)
        self.assertTrue(any('m_opens_map appears 0' in p for p in problems), problems)

    def test_a_repeated_row_fails(self):
        problems = cc.grade(log(list(cc.EXPECTED) + ['m_opens_map'], f'passed={len(cc.EXPECTED)} failed=0'))[1]
        self.assertTrue(any('m_opens_map appears 2' in p for p in problems), problems)

    def test_a_failed_row_fails(self):
        problems = cc.grade(log().replace('m_opens_map PASS', 'm_opens_map FAIL'))[1]
        self.assertTrue(any('row m_opens_map FAIL' in p for p in problems), problems)

    def test_a_short_summary_fails(self):
        problems = cc.grade(log(summary='passed=4 failed=0'))[1]
        self.assertTrue(any('summary passed=4' in p for p in problems), problems)

    def test_a_summary_with_anything_after_it_fails(self):
        good = f'passed={len(cc.EXPECTED)} failed=0'
        self.assertIn('0 summaries, expected one', cc.grade(log(summary=f'{good} failed=1'))[1])
        self.assertEqual(cc.grade(log(summary=good).replace('\n', '\r\n') + '\r\n')[1], [])   # a Windows log's line ends still pass

    def test_a_missing_or_doubled_summary_fails(self):
        self.assertIn('0 summaries, expected one', cc.grade(log(summary=''))[1])
        self.assertIn('2 summaries, expected one', cc.grade(log() + '\n' + log(rows=[]).replace('passed=0', f'passed={len(cc.EXPECTED)}'))[1])

    def test_the_page_did_not_open_note_is_not_a_row(self):
        self.assertEqual(cc.grade(log() + '\nOWCONTROLS map page did not open')[1], [])


if __name__ == '__main__':
    unittest.main()
