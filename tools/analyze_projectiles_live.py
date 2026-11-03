"""Numbers from a projectiles_capture.ps1 session (ignored local/realgame/pj/projectiles/*.json): accuracy pool bloom and
recovery, view kick, spread draws. Prints counts and fitted numbers only; the dumps stay under local/.

  python tools/analyze_projectiles_live.py [folder]   (default local/realgame/pj/projectiles)

Per <gun>_<case>.json:
- pool: min / max seen (GetMinValue / GetMaxValue), the jump at each shot (impulse), the hold before the value falls
  and the fall rate while it falls (per second).
- kick: per shot the view pitch before the shot, its largest change in the next 0.5 s and the change left 1.0 s later
  (left ~ 0: the kick is an offset that returns; left ~ largest: it moved the view for good).
- spread (AddSpread rows): offset = returned aim - base aim, in units of A/2 (A = Spread in rotator units): the largest
  |yaw| and radius, the yaw spread's standard deviation (uniform on [-1, 1]: 0.577), and the pitch / chord ratio's (the
  rule's chord draw: also 0.577; a disc-area draw gives other values).
"""
import json
import math
import statistics
import sys
from pathlib import Path


def wrap(units):
    """Signed rotator difference in [-32768, 32768)."""
    return (int(units) + 32768) % 65536 - 32768


def rot(value):
    if isinstance(value, dict):
        return value.get('Pitch', 0), value.get('Yaw', 0)
    if isinstance(value, (list, tuple)) and len(value) >= 2:
        return value[0], value[1]
    return None


def pool_numbers(frames, fires):
    rows = [(f['t'], f['pool']) for f in frames if f.get('pool') and f['pool'].get('current') is not None and f.get('t') is not None]
    if not rows:
        return {}
    cur = [p['current'] for _, p in rows]
    out = {'frames': len(rows), 'current_min': min(cur), 'current_max': max(cur),
           'GetMinValue': sorted({round(p.get('GetMinValue') or 0, 4) for _, p in rows}),
           'GetMaxValue': sorted({round(p.get('GetMaxValue') or 0, 4) for _, p in rows})}
    jumps, holds, rates = [], [], []
    for i in range(1, len(rows)):
        d = rows[i][1]['current'] - rows[i - 1][1]['current']
        if abs(d) > 1e-4 and any(rows[i - 1][0] - 0.05 <= t <= rows[i][0] + 0.001 for t in fires):
            jumps.append(round(d, 4))
            # hold: frames after the jump until the value moves again; rate: the slope over the following fall
            j = i + 1
            while j < len(rows) and abs(rows[j][1]['current'] - rows[i][1]['current']) < 1e-5:
                j += 1
            if j < len(rows):
                holds.append(round(rows[j - 1][0] - rows[i][0], 3))
                k = j
                while k + 1 < len(rows) and rows[k + 1][1]['current'] < rows[k][1]['current'] - 1e-6:
                    k += 1
                if k > j and rows[k][0] > rows[j - 1][0]:
                    rates.append(round((rows[k][1]['current'] - rows[j - 1][1]['current']) / (rows[k][0] - rows[j - 1][0]), 3))
    out.update({'shot_jumps': jumps[:20], 'holds': holds[:20], 'fall_rates': rates[:20]})
    return out


def kick_numbers(frames, fires):
    views = [(f['t'], f['view']) for f in frames if f.get('view') and f.get('t') is not None]
    out = []
    for t0 in fires[:12]:
        before = [v for t, v in views if t <= t0]
        if not before:
            continue
        p0, y0 = before[-1][0], before[-1][1]
        after = [(t - t0, wrap(v[0] - p0), wrap(v[1] - y0)) for t, v in views if t0 < t <= t0 + 1.5]
        if not after:
            continue
        peak = max(after[:max(1, sum(1 for a in after if a[0] <= 0.5))], key=lambda a: abs(a[1]))
        late = [a for a in after if a[0] >= 1.0]
        out.append({'peak_pitch': peak[1], 'peak_at': round(peak[0], 3), 'left_after_1s': late[0][1] if late else None})
    return out


def spread_numbers(calls, frames=()):
    yaws, chords, radii = [], [], []
    times = [(f['t'], f.get('spread')) for f in frames if f.get('t') is not None and f.get('spread')]
    for c in calls:
        if c.get('kind') != 'spread':
            continue
        base, ret = rot(c.get('BaseAim')), rot(c.get('ret'))
        ret = rot(c.get('ReturnValue')) if c.get('ReturnValue') and any(rot(c['ReturnValue'])) else ret
        # AddSpread takes no spread argument: the held weapon's Spread in the last frame before the call.
        spread = c.get('Spread') or next((v for t, v in reversed(times) if t <= c['t'] + 1e-4), None)
        if not base or not ret:
            continue
        if not spread:
            continue
        half = math.trunc(spread * 65536 / 360) / 2
        if half <= 0:
            continue
        dy, dp = wrap(ret[1] - base[1]) / half, wrap(ret[0] - base[0]) / half
        yaws.append(dy)
        radii.append(math.hypot(dy, dp))
        chord = math.sqrt(max(0.0, 1 - dy * dy))
        if chord > 0.2:
            chords.append(dp / chord)
    if not yaws:
        return {}
    return {'n': len(yaws), 'max_abs_yaw': round(max(map(abs, yaws)), 3), 'max_radius': round(max(radii), 3),
            'yaw_sd': round(statistics.pstdev(yaws), 3), 'pitch_over_chord_sd': round(statistics.pstdev(chords), 3) if chords else None,
            'outside_disc': sum(1 for r in radii if r > 1.02)}


def main(argv=None):
    argv = sys.argv[1:] if argv is None else argv
    folder = Path(argv[0] if argv else 'local/realgame/pj/projectiles')
    for path in sorted(folder.glob('*.json')):
        if path.name.startswith('fields_'):
            continue
        data = json.loads(path.read_text(encoding='utf-8'))
        frames, calls = data.get('frames', []), data.get('calls', [])
        fires = [c['t'] for c in calls if c.get('kind') == 'fire' and c.get('t') is not None]
        print(path.stem, json.dumps({'fires': len(fires), 'kicks': sum(1 for c in calls if c.get('kind') == 'kick'),
                                     'pool': pool_numbers(frames, fires), 'kick': kick_numbers(frames, fires),
                                     'spread': spread_numbers(calls, frames)}))
    return 0


if __name__ == '__main__':
    sys.exit(main())
