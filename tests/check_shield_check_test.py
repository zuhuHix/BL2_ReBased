"""Synthetic tests for tools/check_shield_check.py (invented log lines; no game data). Run: python tests/check_shield_check_test.py"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import check_shield_check as c  # noqa: E402

GOOD = """OpenWillow Maya shield: stand-in rule read from C:/repo/local/slice/stand_in_shield.json
OpenWillow Maya shield Shield Check Stand-in: capacity 100.0 (stand-in rule, UNVERIFIED), recharge 2.00/s after 1.000 s
OWSHIELDCHECK full shield 100.0 of 100.0, health 50.0
OWSHIELDCHECK hit shield 50.0 of 100.0, health 50.0
OWSHIELDCHECK recharge shield 60.0 of 100.0
OWSHIELDCHECK result pass: full, hit and recharged
"""
SHOTS = set(c.SHOTS)


def test_a_good_run_passes():
    assert c.problems(GOOD, SHOTS) == []


def test_each_missing_or_bad_part_is_named():
    for line, text in (('stand-in rule read from', 'read from'), ('capacity 100.0', 'capacity line'), ('OWSHIELDCHECK full', 'full step'),
                       ('OWSHIELDCHECK hit', 'hit step'), ('OWSHIELDCHECK recharge', 'recharge step'), ('OWSHIELDCHECK result', 'no result')):
        log = '\n'.join(l for l in GOOD.splitlines() if line not in l)
        assert any(text in p for p in c.problems(log, SHOTS)), line
    assert any('frame' in p for p in c.problems(GOOD, SHOTS - {'OWShield_hit.png'}))


def test_a_failed_or_empty_shield_does_not_pass():
    assert any('capacity line' in p for p in c.problems(GOOD.replace('capacity 100.0', 'capacity 0.0'), SHOTS))
    failed = GOOD.replace('result pass: full, hit and recharged', 'result fail: the shield did not recharge')
    assert any('did not recharge' in p for p in c.problems(failed, SHOTS))
    warned = GOOD + 'OpenWillow Maya shield: no C:/repo/local/slice/stand_in_shield.json, so a shield without a capacity stat protects nothing\n'
    assert any('warning' in p for p in c.problems(warned, SHOTS))
    bad = GOOD + 'OpenWillow Maya shield: C:/repo/local/slice/stand_in_shield.json ignored, so ...: expected\n'
    assert any('warning' in p for p in c.problems(bad, SHOTS))


if __name__ == '__main__':
    for name, test in list(globals().items()):
        if name.startswith('test_'):
            test()
    print('ok')
