"""Synthetic tests for tools/check_phaselock_kill.py (invented log lines; no game data). Run: python tests/check_phaselock_kill_test.py"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import check_phaselock_kill as c  # noqa: E402
from check_action_skill_hud import LINE  # noqa: E402


def hud(t, active, cooldown=0.9):
    return f'LogTemp: Display: OWSKILLHUD t={t:.3f} cast=10.000 active={active:.2f} cooldown={cooldown:.2f}'


LINES = ['LogTemp: Display: OWPHASELOCKKILL cast t=10.000 hit=1',
         hud(10.800, 1.0),
         hud(11.500, 0.7),
         'LogTemp: Display: OWPHASELOCKKILL held=1 t=12.000 remaining=13.000',
         'LogTemp: Display: OWPHASELOCKKILL kill t=12.000 dead=1',
         hud(12.020, -1.0),
         'LogTemp: Display: OWPHASELOCKKILL after active=0 reason=the held target died remaining=12.900 since_cast=2.200',
         hud(12.500, -1.0, 0.8),
         'LogTemp: Display: OWPHASELOCKKILL drain remaining=11.900 was=12.900',
         'LogTemp: Display: OWPHASELOCKKILL result pass: ended on the kill, cooldown draining']
GOOD = '\n'.join(LINES)
SHOTS = {c.SHOT}


def fails(log, text, shots=SHOTS):
    found = c.problems(log, shots)
    assert any(text in p for p in found), (text, found)


def test_the_stub_lines_are_what_the_hud_checker_reads():
    assert len(list(LINE.finditer(GOOD))) == 4


def test_a_good_run_passes():
    assert c.problems(GOOD, SHOTS) == []


def test_every_step_once_and_in_order():
    for needle in ('cast t=', 'held=1', 'kill t=', 'after active', 'drain', 'result pass'):
        fails('\n'.join(l for l in LINES if needle not in l), 'steps must be')
    fails('\n'.join(LINES + [LINES[4]]), 'steps must be')
    fails('\n'.join([LINES[-1]] + LINES[:-1]), 'steps must be')
    fails(GOOD, 'frame', set())


def test_the_numbers_must_agree():
    for old, new, text in (('hit=1', 'hit=0', 'did not hit'), ('held=1', 'held=0', 'not held'), ('dead=1', 'dead=0', 'not dead'),
                           ('after active=0', 'after active=1', 'still active'), ('reason=the held target died', 'reason=skill constraint X', 'another reason'),
                           ('since_cast=2.200', 'since_cast=5.800', 'past the timeline'), ('drain remaining=11.900', 'drain remaining=12.900', 'did not drain'),
                           ('was=12.900', 'was=20.000', 'did not drain'), ('result pass: ended', 'result fail: ended', 'the check failed')):
        assert GOOD.count(old) == 1, old
        fails(GOOD.replace(old, new), text)


def test_the_hud_emblem_must_run_then_leave_on_the_kill():
    fails(GOOD.replace('active=1.00', 'active=-1.00').replace('active=0.70', 'active=-1.00'), 'running before the kill')
    # The bug this guards: the emblem runs on to the timeline's end.
    late = [l for l in LINES if 'active=-1.00' not in l]
    late.insert(5, hud(12.500, 0.5))
    late.insert(6, hud(15.700, -1.0))
    fails('\n'.join(late), 'more than')
    fails('\n'.join(l for l in LINES if 'active=-1.00' not in l), 'gone after the kill')
    fails(GOOD + '\n' + hud(13.000, 0.4), 'came back')


if __name__ == '__main__':
    for name, test in list(globals().items()):
        if name.startswith('test_'):
            test()
    print('ok')
