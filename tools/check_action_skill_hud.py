"""Check the host's action-skill HUD state against the real game's timings.

Reads the `OWSKILLHUD t=<game s> cast=<game s> active=<f> cooldown=<f>` lines that
AOpenWillowMayaHUD logs whenever the indicator's inputs change (an -owskillhudshots
run, tools/test_action_skill_hud.ps1), takes the cast time from the
`OWSKILLHUDSHOTS cast t=<s> hit=1` line, and checks the sequence
ready -> cast -> emblem on -> emblem off -> cooldown -> ready against the timings
read from the live HUD movie during a real cast (docs/verification/ACTION_SKILL_HUD.md):
emblem from 0.70 s to 5.70 s with its bar emptying evenly, the cooldown pool full
from the cast until the skill ends, then draining at 1/13 per second, ready at 18.73 s.
`active` is -1 while the emblem is off.

The state lines prove the host's inputs only. Two more groups prove the movie drew them:
- the page's read-back lines `OWSKILLPAGE active=<f> cooldown=<f> emblem=<frame> bar=<frame> icon=<frame>`
  (tools/hud_overlay/index.html logs, after each change, the state it applied and the frames read
  back from the movie's clips; the host logs the page console): every line's frames must equal
  the frames action_skill.js sets for the state on that same line, that state must be one the
  host logged, and together they must show the emblem's bar running out, the icon's cooldown
  frames and the ready frame. Pairing each read-back with its own state needs no lag allowance.
- the run's captures (--shots, required for a PASS): every scheduled `OWSKILLHUDSHOTS shot`
  must exist as a 1280x720 PNG (signature and header checked) and the run must have logged
  `OWSKILLHUDSHOTS done`; three of them, decoded with zlib only, must show the indicator: the
  icon green before the cast, gold with the magenta emblem while the skill runs, red without
  the emblem in the cooldown (colour counts in fixed screen boxes). Whether the captures look
  like the game is the critic's call, not this check's.
Prints one PASS/FAIL line per check and a summary. Exit code 0 only when every check passes;
without --shots the summary is PARTIAL and the exit code 2.
"""
import argparse
import math
import re
import struct
import sys
import zlib
from pathlib import Path

LINE = re.compile(r'OWSKILLHUD t=([-\d.]+) cast=([-\d.]+) active=([-\d.]+) cooldown=([-\d.]+)')
CAST = re.compile(r'OWSKILLHUDSHOTS cast t=([-\d.]+) hit=(\d)')
PAGE = re.compile(r'OWSKILLPAGE active=([-\d.]+) cooldown=([-\d.]+) emblem=(\d+) bar=(\d+) icon=(\d+)')
SHOT = re.compile(r'OWSKILLHUDSHOTS shot (\S+\.png) scheduled ([-+\d.]+)')
DONE = 'OWSKILLHUDSHOTS done'
EXPECTED_SHOTS = 47          # every 0.5 s from 1 s before the cast to 22 s after (OpenWillowMayaHUD.cpp)
SHOT_SIZE = (1280, 720)
# Screen boxes (x0, y0, x1, y1) of the icon and the emblem on the 1280x720 HUD with the 3D-turn stand-in.
ICON_BOX, EMBLEM_BOX = (20, 550, 130, 650), (470, 520, 820, 620)
# Real game, one hit cast on an Adult Bullymong (ACTION_SKILL_HUD.md); host timeline from the same data.
LOCK_AT, SKILL_END, POOL_SECONDS, READY_AT = 0.70, 5.70, 13.0, 18.73


def parse(text):
    rows = [(float(m[1]), float(m[3]), float(m[4])) for m in LINE.finditer(text)]
    cast = CAST.search(text)
    return rows, (float(cast[1]), cast[2] == '1') if cast else (None, False)


def expected_frames(active, cooldown):
    """The movie frames tools/hud_overlay/action_skill.js sets for one state line."""
    if active < 0:
        emblem, bar = 1, 0
    else:
        emblem, bar = 3, min(100, math.floor(1 + 99 * (1 - min(1.0, active))))
    return emblem, bar, min(100, math.floor(2 + 98 * (1 - min(1.0, max(0.0, cooldown)))))


def check_page(text):
    """The page's read-back lines: frames against the state on the same line, state against the host's."""
    states = {(f'{float(m[3]):.2f}', f'{float(m[4]):.2f}') for m in LINE.finditer(text)}
    pages = [(f'{float(m[1]):.2f}', f'{float(m[2]):.2f}', int(m[3]), int(m[4]), int(m[5])) for m in PAGE.finditer(text)]
    wrong = [p for p in pages if p[2:] != expected_frames(float(p[0]), float(p[1]))]
    foreign = [p for p in pages if p[:2] not in states]
    shown = [p[3] for p in pages if p[2] == 3]
    icons = [p[4] for p in pages]
    return [
        ('page_lines', len(pages) >= 20, f'{len(pages)} read-back lines'),
        ('page_frames_match_state', pages and not wrong, f'{len(wrong)} of {len(pages)} differ, first {wrong[:1]}'),
        ('page_state_from_host', pages and not foreign, f'{len(foreign)} states the host did not log, first {foreign[:1]}'),
        ('page_emblem_drawn', shown and min(shown) <= 5 and max(shown) >= 90,   # 90: a capture stall may skip the last values
         f'{len(shown)} lines, bar {min(shown) if shown else None}..{max(shown) if shown else None}'),
        ('page_cooldown_drawn', 2 in icons and any(50 <= f < 100 for f in icons) and icons[-1] == 100,
         f'icon frames {sorted(set(icons))[:3]}..{sorted(set(icons))[-3:]}, last {icons[-1] if icons else None}'),
    ]


PNG_SIGNATURE = b'\x89PNG\r\n\x1a\n'


def read_png(path):
    """(width, height, rows of RGB tuples) of an 8-bit RGB/RGBA non-interlaced PNG; ValueError otherwise."""
    data = Path(path).read_bytes()
    if data[:8] != PNG_SIGNATURE:
        raise ValueError('not a PNG')
    pos, idat, header = 8, [], None
    while pos + 8 <= len(data):
        length, kind = struct.unpack('>I4s', data[pos:pos + 8])
        body = data[pos + 8:pos + 8 + length]
        if kind == b'IHDR':
            header = struct.unpack('>IIBBBBB', body)
        elif kind == b'IDAT':
            idat.append(body)
        pos += 12 + length
    if not header:
        raise ValueError('no IHDR')
    width, height, depth, colour, _, _, interlace = header
    if depth != 8 or colour not in (2, 6) or interlace:
        raise ValueError(f'unsupported PNG (depth {depth}, colour type {colour}, interlace {interlace})')
    bpp = 3 if colour == 2 else 4
    raw, stride, rows, prev = zlib.decompress(b''.join(idat)), width * bpp, [], bytearray(width * bpp)
    for y in range(height):
        start = y * (stride + 1)
        kind, line = raw[start], bytearray(raw[start + 1:start + 1 + stride])
        for i in range(stride):   # the PNG row filters (PNG specification, section 9)
            a = line[i - bpp] if i >= bpp else 0
            b, c = prev[i], (prev[i - bpp] if i >= bpp else 0)
            if kind == 1:
                line[i] = (line[i] + a) & 255
            elif kind == 2:
                line[i] = (line[i] + b) & 255
            elif kind == 3:
                line[i] = (line[i] + (a + b) // 2) & 255
            elif kind == 4:
                pa, pb, pc = abs(b - c), abs(a - c), abs(a + b - 2 * c)
                line[i] = (line[i] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        rows.append([tuple(line[x * bpp:x * bpp + 3]) for x in range(width)])
        prev = line
    return width, height, rows


def colours(rows, box):
    """Pixel counts of the indicator's colours inside a screen box."""
    x0, y0, x1, y1 = box
    px = [rgb for row in rows[y0:y1] for rgb in row[x0:x1]]
    return {'green': sum(1 for r, g, b in px if g > 140 and g > r + 40 and g > b + 40),
            'gold': sum(1 for r, g, b in px if r > 180 and g > 110 and b < 90 and r > g),
            'red': sum(1 for r, g, b in px if r > 110 and g < 70 and b < 70),
            'magenta': sum(1 for r, g, b in px if r > 150 and b > 150 and g < 110)}


def valid_shot(path):
    head = path.read_bytes()[:24] if path.is_file() else b''
    return head[:8] == PNG_SIGNATURE and head[12:16] == b'IHDR' and struct.unpack('>II', head[16:24]) == SHOT_SIZE


def check_shots(text, folder):
    shots = [(name, float(at)) for name, at in SHOT.findall(text)]
    missing = [n for n, _ in shots if not (Path(folder) / n).is_file()]
    bad = [n for n, _ in shots if n not in missing and not valid_shot(Path(folder) / n)]
    results = [('shots_scheduled', len(shots) == EXPECTED_SHOTS and DONE in text,
                f'{len(shots)} of {EXPECTED_SHOTS} scheduled, done logged: {DONE in text}'),
               ('shots_written', bool(shots) and not missing and not bad,
                f'missing {missing[:6]}, not 1280x720 PNG {bad[:6]}' if missing or bad else f'{len(shots)} valid')]
    # Three moments: before the cast, mid-lock (2.5 s) and mid-cooldown (12 s).
    pick = {'ready': lambda at: at < 0, 'active': lambda at: abs(at - 2.5) < 0.3, 'cooldown': lambda at: abs(at - 12) < 0.3}
    seen = {}
    for moment, wanted in pick.items():
        name = next((n for n, at in shots if wanted(at) and n not in missing and n not in bad), None)
        try:
            rows = read_png(Path(folder) / name)[2] if name else None
        except (ValueError, zlib.error):
            rows = None
        seen[moment] = (colours(rows, ICON_BOX), colours(rows, EMBLEM_BOX)) if rows else None
    ready, active, cooling = seen['ready'], seen['active'], seen['cooldown']
    results.append(('shot_ready_drawn', bool(ready) and ready[0]['green'] >= 300 and ready[0]['gold'] < 50
                    and ready[1]['magenta'] < 50, f'icon {ready[0] if ready else None}, emblem {ready[1] if ready else None}'))
    results.append(('shot_active_drawn', bool(active) and active[0]['gold'] >= 300 and active[1]['magenta'] >= 500,
                    f'icon {active[0] if active else None}, emblem {active[1] if active else None}'))
    results.append(('shot_cooldown_drawn', bool(cooling) and cooling[0]['red'] >= 100 and cooling[0]['green'] < 50
                    and cooling[0]['gold'] < 50 and cooling[1]['magenta'] < 50,
                    f'icon {cooling[0] if cooling else None}, emblem {cooling[1] if cooling else None}'))
    return results


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


def run_checks(text, shots=None):
    rows, (cast_at, hit) = parse(text)
    results = check(rows, cast_at, hit) + check_page(text)
    if shots is not None:
        results += check_shots(text, shots)
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('log')
    parser.add_argument('--shots', help='folder holding the OWSkillHud_*.png captures of this run (needed for a PASS)')
    args = parser.parse_args()
    text = Path(args.log).read_text(encoding='utf-8', errors='replace')
    results = run_checks(text, args.shots)
    for name, ok, detail in results:
        print(f'OWSKILLHUDTEST {"PASS" if ok else "FAIL"} {name} {detail}')
    failed = sum(1 for _, ok, _ in results if not ok)
    result = 'FAIL' if failed else 'PASS' if args.shots else 'PARTIAL'
    print(f'OWSKILLHUDTEST SUMMARY result={result} passed={len(results) - failed} failed={failed}'
          + ('' if args.shots else ' (captures not checked: no --shots)'))
    return 1 if failed else 0 if args.shots else 2


if __name__ == '__main__':
    sys.exit(main())
