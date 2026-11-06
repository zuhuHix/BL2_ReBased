"""tools/check_audio.py on invented logs. No game data."""
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / 'tools'))
import check_audio as ca  # noqa: E402

PLAY = 'LogTemp: Display: OWAUDIO play role=test_tone channels=1 rate=48000 bytes=96000 (the host called play; not heard)'


def log(rows=None, summary=None, play=PLAY, drained='queued=96000 left=0'):
    rows = list(ca.EXPECTED) if rows is None else rows
    lines = [play] if play else []
    for name in rows:
        lines.append(f'LogTemp: Display: OWAUDIOTEST PASS {name}' + (f' {drained}' if name == 'mixer_pulled_all_bytes' else ''))
    if summary is None:
        summary = f'result=PASS passed={len(rows)} failed=0 (nothing was listened to)'
    if summary:
        lines.append(f'LogTemp: Display: OWAUDIOTEST SUMMARY {summary}')
    return '\n'.join(lines)


class CheckAudioTest(unittest.TestCase):
    def problems(self, text):
        return ca.grade(text)[1]

    def assert_problem(self, text, words):
        problems = self.problems(text)
        self.assertTrue(any(words in p for p in problems), problems)

    def test_a_full_run_passes(self):
        self.assertEqual(self.problems(log()), [])

    def test_a_summary_alone_does_not_pass(self):
        self.assert_problem(log(rows=[], summary='result=PASS passed=16 failed=0'), 'appears 0 times')

    def test_a_lost_row_fails_even_with_a_pass_summary(self):
        text = log(list(ca.EXPECTED)[:-1], summary='result=PASS passed=16 failed=0')
        self.assert_problem(text, 'appears 0 times')

    def test_a_renamed_row_fails(self):
        rows = ['other_name' if name == 'parse_good_wav' else name for name in ca.EXPECTED]
        self.assert_problem(log(rows), 'unexpected row other_name')

    def test_a_duplicate_row_fails(self):
        self.assert_problem(log(list(ca.EXPECTED) + ['parse_good_wav']), 'appears 2 times')

    def test_a_failed_row_fails(self):
        text = log().replace('OWAUDIOTEST PASS refuse_non_pcm', 'OWAUDIOTEST FAIL refuse_non_pcm')
        self.assert_problem(text, 'row refuse_non_pcm FAIL')

    def test_wrong_counts_fail(self):
        self.assert_problem(log(summary='result=PASS passed=15 failed=0'), 'summary result=PASS passed=15')
        self.assert_problem(log(summary='result=PASS passed=16 failed=1'), 'failed=1')
        self.assert_problem(log(summary='result=FAIL passed=16 failed=0'), 'summary result=FAIL')

    def test_a_missing_or_second_summary_fails(self):
        self.assert_problem(log(summary=''), '0 summaries')
        self.assert_problem(log() + '\n' + log(rows=[]), 'summaries')

    def test_the_tone_must_have_played_once(self):
        self.assert_problem(log(play=''), '0 tone play lines')

    def test_a_mixer_pass_needs_its_numbers(self):
        self.assert_problem(log(drained='queued=0 left=0'), 'mixer row')
        self.assert_problem(log(drained='queued=96000 left=512'), 'mixer row')
        self.assert_problem(log(drained=''), 'mixer row')


if __name__ == '__main__':
    unittest.main()
