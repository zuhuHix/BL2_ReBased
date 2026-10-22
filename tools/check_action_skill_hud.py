"""Check the host's action-skill HUD state against the real game's timings.

Reads the `OWSKILLHUD t=<game s> cast=<game s> active=<f> cooldown=<f>` lines that
AOpenWillowMayaHUD logs whenever the indicator's inputs change (an -owskillhudshots
run, tools/test_action_skill_hud.ps1), takes the cast time from the
`OWSKILLHUDSHOTS cast t=<s> hit=1` line, and checks the sequence
ready -> cast -> emblem on -> emblem off -> cooldown -> ready against the timings
read from the live HUD movie during a real cast (docs/verification/ACTION_SKILL_HUD.md):
emblem from 0.70 s to 5.70 s with its bar emptying evenly, the cooldown pool full
from the cast until the skill ends, then draining at 1/13 per second, ready at 18.73 s.
`active` is -1 while the emblem is off. Prints one PASS/FAIL line per check and a
summary; exit code 0 only when every check passes.
"""
import argparse
import re
import sys

LINE = re.compile(r'OWSKILLHUD t=([-\d.]+) cast=([-\d.]+) active=([-\d.]+) cooldown=([-\d.]+)')
CAST = re.compile(r'OWSKILLHUDSHOTS cast t=([-\d.]+) hit=(\d)')
# Real game, one hit cast on an Adult Bullymong (ACTION_SKILL_HUD.md); host timeline from the same data.
LOCK_AT, SKILL_END, POOL_SECONDS, READY_AT = 0.70, 5.70, 13.0, 18.73


def parse(text):
    rows = [(float(m[1]), float(m[3]), float(m[4])) for m in LINE.finditer(text)]
    cast = CAST.search(text)
    return rows, (float(cast[1]), cast[2] == '1') if cast else (None, False)


def check(rows, cast_at, hit, tolerance=0.15):
    results = []

    def record(name, ok, detail=''):
        results.append((name, bool(ok), detail))

    record('cast_logged', cast_at is not None and hit, f'cast={cast_at} hit={hit}')
    if cast_at is None or not rows:
        record('state_lines', bool(rows), f'{len(rows)} lines')
        return results
    rel = [(t - cast_at, a, c) for t, a, c in rows]
    before = [r for r in rel if r[0] < -0.05]
    record('ready_before_cast', before and before[-1][1] < 0 and before[-1][2] == 0.0,
           f'last before cast {before[-1] if before else None}')
    after = [r for r in rel if r[0] >= -0.05]
    full = next((r for r in after if r[2] >= 0.999), None)
    record('pool_full_at_cast', full is not None and full[0] <= tolerance, f'first full {full}')
    on = [r for r in after if r[1] >= 0]
    record('emblem_on_at_lock', on and abs(on[0][0] - LOCK_AT) <= tolerance and on[0][1] >= 0.95,
           f'first on {on[0] if on else None}')
    off = next((r for r in after if on and r[0] > on[-1][0]), None)
    record('emblem_off_at_end', off is not None and abs(off[0] - SKILL_END) <= tolerance, f'first off {off}')
    record('emblem_drains', all(b[1] <= a[1] + 1e-6 for a, b in zip(on, on[1:])) and on and on[-1][1] <= 0.05,
           f'{len(on)} lines, last {on[-1] if on else None}')
    held = [r for r in after if full and full[0] <= r[0] < SKILL_END - tolerance]
    record('pool_held_while_active', held and all(r[2] >= 0.999 for r in held), f'{len(held)} lines')
    draining = [r for r in after if SKILL_END + tolerance <= r[0] and 0.02 < r[2] < 0.98]
    if len(draining) >= 2:
        (t0, _, c0), (t1, _, c1) = draining[0], draining[-1]
        rate = (c0 - c1) * POOL_SECONDS / (t1 - t0)
    else:
        rate = 0.0
    record('pool_drain_rate', abs(rate - 1.0) <= 0.03, f'{rate:.3f} pool s per s over {len(draining)} lines')
    ready = next((r for r in after if r[0] > SKILL_END and r[2] == 0.0), None)
    record('ready_again', ready is not None and abs(ready[0] - READY_AT) <= 2 * tolerance, f'first ready {ready}')
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('log')
    args = parser.parse_args()
    rows, (cast_at, hit) = parse(open(args.log, encoding='utf-8', errors='replace').read())
    results = check(rows, cast_at, hit)
    for name, ok, detail in results:
        print(f'OWSKILLHUDTEST {"PASS" if ok else "FAIL"} {name} {detail}')
    failed = sum(1 for _, ok, _ in results if not ok)
    print(f'OWSKILLHUDTEST SUMMARY result={"PASS" if not failed else "FAIL"} passed={len(results) - failed} failed={failed}')
    return 0 if not failed else 1


if __name__ == '__main__':
    sys.exit(main())
