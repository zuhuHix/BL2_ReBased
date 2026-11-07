"""Synthetic tests for tools/check_phaselock_kill.py (invented log lines; no game data). Run: python tests/check_phaselock_kill_test.py"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import check_phaselock_kill as c  # noqa: E402
from check_action_skill_hud import LINE  # noqa: E402


def hud(t, active, cooldown=0.9):
    return f'LogTemp: Display: OWSKILLHUD t={t:.3f} cast=10.000 active={active:.2f} cooldown={cooldown:.2f}'


def page(emblem, active=0.5):
    return f'OWSKILLPAGE active={active:.2f} cooldown=0.90 emblem={emblem} bar=20 icon=2'


LINES = ['LogTemp: Display: OWPHASELOCKKILL cast t=10.000 hit=1',
         hud(10.800, 1.0),
         page(3),
         hud(11.500, 0.7),
         'LogTemp: Display: OWPHASELOCKKILL held=1 t=12.000 remaining=13.000',
         'LogTemp: Display: OWPHASELOCKKILL kill t=12.000 dead=1',
         hud(12.020, -1.0),
         page(1, -1.0),
         'LogTemp: Display: OWPHASELOCKKILL after active=0 reason=the held target died remaining=12.800 since_cast=2.200 t=12.200',
         hud(12.500, -1.0, 0.8),
         'LogTemp: Display: OWPHASELOCKKILL drain remaining=11.800 was=12.800 t=13.200 rate=1.000',
         'LogTemp: Display: OWPHASELOCKKILL result pass: ended on the kill, cooldown draining']
GOOD = '\n'.join(LINES)
SHOTS = {c.SHOT}


def fails(log, text, shots=SHOTS):
    found = c.problems(log, shots)
    assert any(text in p for p in found), (text, found)


def test_the_stub_lines_are_what_the_hud_checker_reads():
    assert len(list(LINE.finditer(GOOD))) == 4 and len(list(c.PAGE.finditer(GOOD))) == 2


def test_a_good_run_passes():
    assert c.problems(GOOD, SHOTS) == []


def test_every_step_once_and_in_order():
    for needle in ('cast t=', 'held=1', 'kill t=', 'after active', 'drain', 'result pass'):
        fails('\n'.join(l for l in LINES if needle not in l), 'steps must be')
    fails('\n'.join(LINES + [LINES[5]]), 'steps must be')
    fails('\n'.join([LINES[-1]] + LINES[:-1]), 'steps must be')
    fails(GOOD, 'frame', set())


def test_the_numbers_must_agree():
    for old, new, text in (('hit=1', 'hit=0', 'did not hit'), ('held=1', 'held=0', 'not held'), ('dead=1', 'dead=0', 'not dead'),
                           ('after active=0', 'after active=1', 'still active'), ('reason=the held target died', 'reason=skill constraint X', 'another reason'),
                           ('since_cast=2.200', 'since_cast=5.800', 'past the timeline'), ('drain remaining=11.800', 'drain remaining=12.800', 'the cooldown drained'),
                           ('drain remaining=11.800', 'drain remaining=12.799', 'the cooldown drained'),    # any decrease is not enough
                           ('drain remaining=11.800', 'drain remaining=10.800', 'the cooldown drained'),    # nor too fast
                           ('was=12.800', 'was=20.000', 'the drain step read'), ('result pass: ended', 'result fail: ended', 'the check failed'),
                           ('remaining=12.800 since_cast', 'remaining=13.000 since_cast', 'starting at the kill')):
        assert GOOD.count(old) == 1, old
        fails(GOOD.replace(old, new), text)


def test_the_movie_must_read_back_its_frames():
    fails(GOOD.replace(page(3), page(1)), 'running emblem frame')
    fails(GOOD.replace(page(3), page(3, -1.0)), 'running emblem frame')       # frame 3 with the emblem said to be off
    fails(GOOD.replace(page(1, -1.0), page(3, -1.0)), 'off emblem frame')
    fails(GOOD.replace(page(1, -1.0), page(1, 0.5)), 'off emblem frame')      # frame 1 with the emblem said to be active
    fails('\n'.join(l for l in LINES if 'OWSKILLPAGE' not in l or 'emblem=3' in l), 'off emblem frame')


def test_the_drain_is_judged_at_the_logged_rate():
    # A cooldown modifier: half the rate drains half as much, and starts from the kill at that rate.
    slow = GOOD.replace('rate=1.000', 'rate=0.500').replace('remaining=12.800 since_cast', 'remaining=12.900 since_cast').replace('was=12.800', 'was=12.900').replace('drain remaining=11.800', 'drain remaining=12.400')
    assert c.problems(slow, SHOTS) == []
    fails(GOOD.replace('rate=1.000', 'rate=0.500'), 'the cooldown drained')    # the same numbers at a rate they don't fit
    fails(GOOD.replace('rate=1.000', 'rate=0.000'), 'logged rate')


def test_a_destroyed_target_names_its_own_reason():
    gone = GOOD.replace('reason=the held target died', 'reason=the held target is gone')
    assert c.problems(gone, SHOTS, 'held target is gone') == []
    assert any('another reason' in p for p in c.problems(GOOD, SHOTS, 'held target is gone'))


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
