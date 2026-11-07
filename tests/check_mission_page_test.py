"""Synthetic tests for tools/check_mission_page.py (invented log lines). Run: python tests/check_mission_page_test.py"""
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
from check_mission_page import EXPECTED, grade  # noqa: E402


def log(names=EXPECTED, result='PASS', summary=None):
    lines = [f'[x] Display: OWMISSIONPAGETEST {name} {result} detail' for name in names]
    lines.append(summary if summary is not None else f'[x] Display: OWMISSIONPAGETEST SUMMARY passed={len(names)} failed=0')
    return '\n'.join(lines) + '\n'


assert grade(log())[1] == [], grade(log())[1]
assert len(EXPECTED) == 27, len(EXPECTED)
# The HUD behind the page and after each way out are required rows.
for name in ('hud_hidden_behind_the_offer', 'hud_hidden_behind_the_turn_in', 'hud_hidden_behind_the_reward', 'hud_back_after_escape', 'hud_back_after_accepting', 'hud_back_after_the_reward'):
    assert any(f'{name} appears 0 times' in p for p in grade(log(tuple(n for n in EXPECTED if n != name)))[1]), name
# failed=0 with a lost row: the old exit rule passed this.
lost = grade(log(EXPECTED[1:]))[1]
assert any('page_ready appears 0 times' in p for p in lost), lost
assert any('expected passed=' in p for p in lost), lost
# A repeated, a renamed and a failing row.
assert any('appears 2 times' in p for p in grade(log(EXPECTED + EXPECTED[:1]))[1])
assert any('unexpected row other' in p for p in grade(log(EXPECTED + ('other',)))[1])
assert any('FAIL' in p for p in grade(log(result='FAIL', summary=f'x OWMISSIONPAGETEST SUMMARY passed=0 failed={len(EXPECTED)}'))[1])
# No summary, or two.
assert any('0 summaries' in p for p in grade(log(summary=''))[1])
assert any('2 summaries' in p for p in grade(log() + 'OWMISSIONPAGETEST SUMMARY passed=0 failed=0\n')[1])
print('ok')
