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

    def test_a_missing_or_doubled_summary_fails(self):
        self.assertIn('0 summaries, expected one', cc.grade(log(summary=''))[1])
        self.assertIn('2 summaries, expected one', cc.grade(log() + '\n' + log(rows=[]).replace('passed=0', f'passed={len(cc.EXPECTED)}'))[1])

    def test_the_page_did_not_open_note_is_not_a_row(self):
        self.assertEqual(cc.grade(log() + '\nOWCONTROLS map page did not open')[1], [])


if __name__ == '__main__':
    unittest.main()
