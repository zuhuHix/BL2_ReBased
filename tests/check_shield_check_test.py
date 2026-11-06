"""Synthetic tests for tools/check_shield_check.py (invented log lines; no game data). Run: python tests/check_shield_check_test.py"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import check_shield_check as c  # noqa: E402

LINES = ['LogTemp: Display: OWSHIELDCHECK equipped a stat-less shield at level 5',
         'LogTemp: Display: OpenWillow Maya shield: stand-in rule read from D:/repo/local/slice/stand_in_shield.json',
         'LogTemp: Display: OpenWillow Maya shield Shield Check Stand-in: capacity 100.0 (stand-in rule, UNVERIFIED), recharge 2.00/s after 1.000 s',
         'LogTemp: Display: OWSHIELDCHECK full shield 100.0 of 100.0, health 50.0',
         'LogTemp: Display: OWSHIELDCHECK shot full t=8.000',
         'LogTemp: Display: OpenWillow Maya took 50.0 damage (shield 50.0, health 0.0), shield 50.0/100.0 health 50.0/50.0',
         'LogTemp: Display: OWSHIELDCHECK hit shield 50.0 of 100.0, health 50.0',
         'LogTemp: Display: OWSHIELDCHECK recharge shield 60.0 of 100.0, health 50.0',
         'LogTemp: Display: OWSHIELDCHECK result pass: full, hit and recharged']
GOOD = '\n'.join(LINES)
SHOTS = set(c.SHOTS)


def fails(log, text):
    found = c.problems(log, SHOTS)
    assert any(text in p for p in found), (text, found)


def test_a_good_run_passes():
    assert c.problems(GOOD, SHOTS) == []
    assert c.problems(GOOD.replace('D:/repo/local/slice/', 'D:\\repo\\local\\slice\\'), SHOTS) == []    # Windows separators


def test_every_step_is_required():
    for needle in ('equipped a stat-less', 'rule read from', 'capacity 100.0', 'full shield', 'took 50.0', 'hit shield', 'recharge shield', 'result pass'):
        fails('\n'.join(l for l in LINES if needle not in l), 'steps must be')
    assert any('frame' in p for p in c.problems(GOOD, SHOTS - {'OWShield_hit.png'}))


def test_steps_once_each_and_in_order():
    fails('\n'.join(LINES + [LINES[3]]), 'steps must be')                       # a repeated step
    fails('\n'.join(LINES + [LINES[-1]]), 'steps must be')                      # a repeated result
    fails('\n'.join(LINES[:5] + [LINES[6], LINES[5]] + LINES[7:]), 'steps must be')   # hit before the damage line
    fails('\n'.join([LINES[-1]] + LINES[:-1]), 'steps must be')                 # the result is not last
    fails('\n'.join(LINES[:6] + [LINES[5]] + LINES[6:]), 'steps must be')       # a second damage line


def test_the_numbers_must_agree():
    cases = (
        ('capacity 100.0 (', 'capacity 0.0 (', 'not positive'),
        ('full shield 100.0 of', 'full shield 90.0 of', 'not the capacity'),
        ('full shield 100.0 of 100.0', 'full shield 100.0 of 120.0', 'capacity changes'),
        ('hit shield 50.0 of 100.0', 'hit shield 100.0 of 100.0', 'does not match'),
        ('hit shield 50.0 of 100.0', 'hit shield 40.0 of 100.0', 'does not match'),
        ('hit shield 50.0 of 100.0', 'hit shield 50.0 of 90.0', 'capacity changes'),
        ('(shield 50.0, health 0.0)', '(shield 40.0, health 10.0)', 'from health'),
        ('recharge shield 60.0', 'recharge shield 50.0', 'recharge shield'),
        ('recharge shield 60.0', 'recharge shield 130.0', 'recharge shield'),
        ('hit shield 50.0 of 100.0, health 50.0', 'hit shield 50.0 of 100.0, health 49.0', 'health changed'),
        ('recharge shield 60.0 of 100.0, health 50.0', 'recharge shield 60.0 of 100.0, health 51.0', 'health changed'),
        ('health 50.0/50.0', 'health 40.0/50.0', 'health changed'),
        ('D:/repo/local/slice/stand_in_shield.json', 'D:/elsewhere/stand_in_shield.json', 'rule was read from'),
    )
    for old, new, text in cases:
        assert GOOD.count(old) == 1, old
        fails(GOOD.replace(old, new), text)


def test_a_failed_result_or_a_warning_does_not_pass():
    fails(GOOD.replace('result pass: full, hit and recharged', 'result fail: the shield did not recharge'), 'did not recharge')
    fails(GOOD + '\nOpenWillow Maya shield: no D:/repo/local/slice/stand_in_shield.json, so a shield without a capacity stat protects nothing', 'warning')
    fails(GOOD + '\nOpenWillow Maya shield: D:/repo/local/slice/stand_in_shield.json ignored, so a shield protects nothing: expected', 'warning')


if __name__ == '__main__':
    for name, test in list(globals().items()):
        if name.startswith('test_'):
            test()
    print('ok')
