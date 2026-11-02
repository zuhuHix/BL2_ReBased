"""Check an in-engine minimap run (tools/test_hud_minimap.ps1) against the rules read from the real game.

Our own code; docs/verification/HUD_MINIMAP.md. Reads the run's UE log:
- the host's `OWMINIMAP manifest ...` line and its `OWMINIMAP t=<s> x=<uu> y=<uu> yaw=<deg> icons=<n>` state lines;
- the page's read-back lines (`OWMINIMAPPAGE x= y= yaw= map=<x>,<y>,<rotation>,<xscale> mapw=<w> north=<x>,<y>
  icons=<n> [objective=<visible>,<x>,<y>,<scale>]`, written by tools/hud_overlay/minimap.js after each change from
  values read back from the movie's clips), its `OWMINIMAPPAGE fog {...}` line and `OWMINIMAPPAGE echo ...`;
- `OWMINIMAPSHOTS shot <file> spot <n> at (x, y, z) yaw <deg>` and `OWMINIMAPSHOTS done`.

The expected clip values are recomputed here from the live-game rules (not from minimap.js): the map turns by -yaw
about the player at 1.024 scale, its origin the tactical map volume's centre (manifest), 128 uu per map unit; the "N"
marker on the 58 x 48 box edge plus (-16, -18.8). With --shots, the run's captures must be complete 1280x720 PNGs
and show the minimap panel (at least 300 cyan map-line pixels in the top-right box) and the player arrow (at least 40
gold pixels at its centre; the real game's frames have 78-106).
Prints OWMINIMAPTEST lines and a SUMMARY; exit 0 pass, 1 fail, 2 partial (no --shots).
"""
import argparse
import math
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from check_action_skill_hud import read_png, valid_shot  # noqa: E402

NUM = r'(-?[\d.]+|NaN)'
MANIFEST = re.compile(r'OWMINIMAP manifest .*?centre \((-?[\d.]+), (-?[\d.]+)\), (\d+) icons')
STATE = re.compile(r'OWMINIMAP t=([-\d.]+) x=(-?\d+) y=(-?\d+) yaw=(-?[\d.]+) icons=(\d+)')
PAGE = re.compile(rf'OWMINIMAPPAGE x={NUM} y={NUM} yaw={NUM} map={NUM},{NUM},{NUM},{NUM} mapw={NUM} '
                  rf'north={NUM},{NUM} icons=(\d+)')
FOG = re.compile(r'OWMINIMAPPAGE fog (\{.*?\})')
SHOT = re.compile(r'OWMINIMAPSHOTS shot (\S+\.png) spot (\d+) at \((-?\d+), (-?\d+), (-?\d+)\) yaw (-?[\d.]+)')
DONE = 'OWMINIMAPSHOTS done'
SPOTS = [(8109, 2930, 0), (8109, 2930, 90), (8109, 2930, 180), (8109, 2930, 270),
         (10300, 4800, 0), (10300, 4800, 90), (10300, 4800, 200), (10300, 4800, 300)]
HALF_W, HALF_H, UU_PER_UNIT, UU_PER_MAP_UNIT = 58.0, 48.0, 125.0, 128.0
MINIMAP_BOX = (1110, 25, 1265, 150)     # the panel at 1280x720 in the real game's frames (and the host's)
ARROW_BOX = (1168, 70, 1202, 110)       # around the player arrow at the panel's centre (real frames: 78-106 gold px)


def turn(x, y, yaw):
    t = math.radians(-yaw)
    return x * math.cos(t) - y * math.sin(t), x * math.sin(t) + y * math.cos(t)


def expected(x, y, yaw, centre):
    """Clip values the game's widget would write for a player at (x, y) looking along yaw degrees."""
    scale = UU_PER_MAP_UNIT / UU_PER_UNIT
    px, py = (y - centre[1]) / UU_PER_MAP_UNIT * scale, -(x - centre[0]) / UU_PER_MAP_UNIT * scale
    mx, my = turn(px, py, yaw)
    rotation = (-yaw) % 360
    if rotation >= 180:
        rotation -= 360
    nx, ny = turn(0, -1, yaw)
    f = min(HALF_W / abs(nx) if abs(nx) > 1e-9 else math.inf, HALF_H / abs(ny) if abs(ny) > 1e-9 else math.inf)
    return {'map': (-mx, -my, rotation, scale * 100), 'north': (nx * f - 16, ny * f - 18.8)}


def angle_diff(a, b):
    return abs((a - b + 180) % 360 - 180)


def run_checks(text, shots=None):
    results = []

    def check(name, ok, detail=''):
        results.append((name, bool(ok), detail))

    manifest = MANIFEST.search(text)
    check('manifest_loaded', manifest, manifest.group(0) if manifest else 'no OWMINIMAP manifest line')
    centre = (float(manifest.group(1)), float(manifest.group(2))) if manifest else (0.0, 0.0)
    states = [(float(t), int(x), int(y), float(yaw), int(n)) for t, x, y, yaw, n in STATE.findall(text)]
    check('host_state_logged', len(states) >= 8, f'{len(states)} state lines')
    pages = []
    for m in PAGE.finditer(text):
        v = [float(g) if g != 'NaN' else math.nan for g in m.groups()[:10]]
        pages.append({'x': v[0], 'y': v[1], 'yaw': v[2], 'map': v[3:7], 'mapw': v[7], 'north': v[8:10],
                      'icons': int(m.group(11))})
    check('page_readbacks', len(pages) >= 8, f'{len(pages)} read-backs')
    loaded = [p for p in pages if p['mapw'] > 50]
    check('map_movie_loaded', loaded, f'{len(loaded)} read-backs with the map movie drawn (width > 50)')
    worst_pos = worst_rot = worst_north = 0.0
    for p in pages:
        e = expected(p['x'], p['y'], p['yaw'], centre)
        worst_pos = max(worst_pos, math.hypot(p['map'][0] - e['map'][0], p['map'][1] - e['map'][1]))
        worst_rot = max(worst_rot, angle_diff(p['map'][2], e['map'][2]))
        worst_north = max(worst_north, math.hypot(p['north'][0] - e['north'][0], p['north'][1] - e['north'][1]))
        if abs(p['map'][3] - e['map'][3]) > 0.05:
            worst_pos = math.inf
    # Flash keeps positions in twips (0.05) and the state is rounded to 1 uu (0.008 units).
    check('map_follows_player', pages and worst_pos <= 0.1, f'worst map position error {worst_pos:.3f} units')
    check('map_turns_with_view', pages and worst_rot <= 0.05, f'worst map rotation error {worst_rot:.3f} deg')
    check('north_on_box_edge', pages and worst_north <= 0.1, f'worst N marker error {worst_north:.3f} units')
    host = {(x, y, round(yaw, 1)) for _, x, y, yaw, _ in states}
    stray = [p for p in pages if (round(p['x']), round(p['y']), round(p['yaw'], 1)) not in host]
    check('readbacks_echo_host_state', pages and not stray, f'{len(stray)} read-backs of a state the host never logged')
    fog = FOG.search(text)
    check('fog_initialised', fog, fog.group(1) if fog else 'the map movie never asked for its fog (onLoadInit)')
    check('icons_shown', any(p['icons'] > 0 for p in loaded), f'max {max((p["icons"] for p in pages), default=0)} icons visible')
    shot_rows = [(name, int(i), int(x), int(y), int(z), float(yaw)) for name, i, x, y, z, yaw in SHOT.findall(text)]
    spot_ids = [r[1] for r in shot_rows]
    names = [r[0].casefold() for r in shot_rows]   # Windows file names are case-insensitive
    # Every spot exactly once, under its own file: eight shots of one spot are not eight spots.
    every_spot = sorted(spot_ids) == list(range(len(SPOTS))) and len(set(names)) == len(names)
    check('shots_logged', every_spot and DONE in text,
          f'{len(shot_rows)} shots for spots {sorted(spot_ids)} (want each of 0..{len(SPOTS) - 1} once), '
          f'{len(set(names))} distinct files, done={DONE in text}')
    bad_spots = [r for r in shot_rows if r[1] >= len(SPOTS) or math.hypot(r[2] - SPOTS[r[1]][0], r[3] - SPOTS[r[1]][1]) > 60
                 or angle_diff(r[5], SPOTS[r[1]][2]) > 0.5]
    check('shots_at_capture_spots', shot_rows and not bad_spots, f'{len(bad_spots)} shots away from their spot')
    applied = [r for r in shot_rows if any(abs(p['x'] - r[2]) <= 1 and abs(p['y'] - r[3]) <= 1
                                           and angle_diff(p['yaw'], r[5]) <= 0.1 and p['mapw'] > 50 for p in pages)]
    check('shot_states_applied', shot_rows and len(applied) == len(shot_rows),
          f'{len(applied)} of {len(shot_rows)} shot states read back from the movie')
    if shots is not None:
        folder = Path(shots)
        files = [folder / r[0] for r in shot_rows]
        valid = [f for f in files if valid_shot(f)]
        check('captures_valid', files and len(valid) == len(files), f'{len(valid)} of {len(files)} complete 1280x720 PNGs')
        drawn = []
        for f in valid:
            _, _, rows = read_png(f)
            x0, y0, x1, y1 = MINIMAP_BOX
            px = [rgb for row in rows[y0:y1] for rgb in row[x0:x1]]
            cyan = sum(1 for r, g, b in px if b > 150 and g > 120 and r < 140)
            x0, y0, x1, y1 = ARROW_BOX
            px = [rgb for row in rows[y0:y1] for rgb in row[x0:x1]]
            gold = sum(1 for r, g, b in px if r > 180 and g > 110 and b < 90 and r > g)
            drawn.append((f.name, cyan, gold))
        good = [d for d in drawn if d[1] >= 300 and d[2] >= 40]
        check('captures_show_minimap', drawn and len(good) == len(drawn),
              '; '.join(f'{n} cyan {c} gold {g}' for n, c, g in drawn))
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('log')
    parser.add_argument('--shots', help='folder with this run\'s OWMinimap_*.png captures')
    args = parser.parse_args()
    text = Path(args.log).read_text(encoding='utf-8', errors='replace')
    results = run_checks(text, args.shots)
    for name, ok, detail in results:
        print(f'OWMINIMAPTEST {"PASS" if ok else "FAIL"} {name} {detail}')
    failed = [r for r in results if not r[1]]
    result = 'FAIL' if failed else ('PASS' if args.shots else 'PARTIAL')
    print(f'OWMINIMAPTEST SUMMARY result={result} checks={len(results)} failed={len(failed)}')
    return 1 if failed else (0 if args.shots else 2)


if __name__ == '__main__':
    sys.exit(main())
