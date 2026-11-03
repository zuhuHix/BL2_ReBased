"""Check a down-state or mission-tracker HUD run (tools/run_hud_shots.ps1 -Down / -Tracker) from its log and captures.

-Down (`-owhuddownshots`): the host's down state and what the HUD movie showed for it.
- down_sequence: the Walker's lines in order: down, bled out, respawned, down again, second wind.
- down_timing: bled out 2.28 s + the logged bleed-out total after the injury (within 0.1 s), the respawn 5.76 s after
  that (within 0.1 s); the first injury's total 12 s. These are the host's own constants (COMBAT_DOWN_RESPAWN.md has the
  live numbers they come from); the check catches the host drifting from them, not the game.
- second_wind_health: the second wind leaves health at 25 % of the maximum (within 1 point).
- down_states: the OWHUDDOWN lines go injured (the bleed-out falling), dead, up, injured, then up with secondWind=1.
- down_page_echo: every OWHUDDOWN state from the first one the page saw has a later OWHUDDOWNPAGE line echoing it, with
  the dying clip on its `fight` frame (2) when injured and off (1) otherwise, and the bleed-out bar on the frame the
  bleed maps to (frame 1 full, 100 empty).
- down_page_texts: with the strings file loaded (`OpenWillow HUD strings <n>`), every injured title the page shows equals the
  `injured` value of that file (--strings, default local/ui/run/UI_HUD/hud_strings.json), escaped as the page writes it.
- second_wind_banner: after secondWind=1 the page shows the banner clip past its first frame.
- down_shots: OWHudDown_{up,down_start,down_mid,dead,respawned,second_wind}.png are complete 1280x720 PNGs, and the
  down panel is in them: the bleed-out bar's red fills a fixed box in down_start and down_mid (the bar shorter at
  down_mid) and is absent from the other four (box and colour measured on this lane's captures, HUD_PARITY.md).
-Tracker (`-owquest -owquesttest -owhudtrackshots`): every OWHUDTRACK host state with a mission has a later
OWHUDTRACKPAGE line whose Module1 shows the mission name, one module per objective line in the state the line has
(`[x]` Complete, `[o]` Optional, `[ ]` NotComplete) with its text, or one TurnIn module when ready to turn in; a state without a mission has every
module off. The run must show the mission go from its first objective to ready to turn in.
Both: the movie path and the page identity (check_hud_shots.check_identity). Prints `OWHUDDOWNTEST PASS|FAIL <name>
<detail>` per check and a summary; exit code 0 only when every check passes.
"""
import argparse
import json
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from check_hud_shots import PAGE_SUFFIX, check_identity, count, is_health, read_shot  # noqa: E402

TARGETED_DELAY, DEATH_CAMERA, SECOND_WIND = 2.28, 5.76, 0.25   # the host's constants (OpenWillowWalker.cpp)
DOWN_SHOTS = ('up', 'down_start', 'down_mid', 'dead', 'respawned', 'second_wind')
# The inner clip's frame for each tracker label, read in the Ruffle bench (hud_fields.js trackerCalls).
INNER_FRAMES = {'MissionName': 29, 'NotComplete': 8, 'Complete': 1, 'Optional': 23, 'TurnIn': 51}
MODULE_SHOWN = 5   # the module's `NoMovement` frame
CONSOLE = 'OpenWillow Flash HUD console: '
STRINGS_FILE = Path(__file__).resolve().parents[1] / 'local' / 'ui' / 'run' / 'UI_HUD' / 'hud_strings.json'
# The down panel's bleed-out bar (1280x720): its red in this box only while injured (measured on the 2026-11-03 captures:
# about 3500 pixels at down_start, 2300 at down_mid, none in the other four).
BLEED_BOX, BLEED_SHOWN, BLEED_HIDDEN = (400, 380, 900, 580), 1000, 100

WALKER = re.compile(r'OpenWillow Maya (?:(?P<down>down) t=(?P<t1>[0-9.]+) bleedout=(?P<total>[0-9.]+)s'
                    r'|(?P<bled>bled out) t=(?P<t2>[0-9.]+)'
                    r'|(?P<wind>second wind) t=(?P<t3>[0-9.]+) health (?P<health>[0-9.]+)/(?P<max>[0-9.]+)'
                    r'|(?P<respawn>died and respawned))')
HOST_DOWN = re.compile(r'OWHUDDOWN t=(?P<t>[0-9.]+) phase=(?P<phase>\w+) bleed=(?P<bleed>[0-9.]+) secondWind=(?P<wind>\d+)\s*$')
PAGE_DOWN = re.compile(r'OWHUDDOWNPAGE phase=(?P<phase>\w+) bleed=(?P<bleed>[0-9.]+) secondWind=(?P<wind>\d+) '
                       r'dying=(?P<dying>\S+) bleedout=(?P<bar>\S+) title=(?P<title>.*?) windFrame=(?P<windFrame>\S+)$')
HOST_TRACK = re.compile(r'OWHUDTRACK t=[0-9.]+ (?P<rest>.*)$')
PAGE_TRACK = re.compile(r'OWHUDTRACKPAGE mission=(?P<mission>.*?) turnIn=(?P<turnIn>[01]) lines=(?P<lines>\d+) (?P<modules>m1=.*)$')
STRINGS = re.compile(r'OpenWillow HUD strings (\S+)')
LOG_TIME = re.compile(r'^\[(?P<stamp>[^\]]+)\]')


def esc(text):
    """The page's escaping of a text in its lines (hud_fields.js esc)."""
    return ''.join(c if 0x21 <= ord(c) <= 0x7e else f'\\u{ord(c):04x}' for c in text.replace('\r', ''))


def bar_frame(fill):
    return 1 + round((1 - min(1.0, max(0.0, fill))) * 99)


def page_lines(lines, pattern):
    """(line number, match) of the page's console lines of one kind, without the console's (http://...) tail."""
    out = []
    for number, line in enumerate(lines):
        if CONSOLE in line:
            text = PAGE_SUFFIX.sub('', line.split(CONSOLE, 1)[1])
            found = pattern.fullmatch(text)
            if found:
                out.append((number, found))
    return out


def check_path(lines):
    movie = [l for l in lines if 'OpenWillow HUD path: movie' in l]
    canvas = [l for l in lines if 'OpenWillow HUD path: canvas' in l]
    return 'movie_path', bool(movie) and not canvas, (movie or canvas or ['no HUD path line'])[-1].strip()[-120:]


def check_down(lines, strings_file=STRINGS_FILE):
    results = []
    walker = []
    for line in lines:
        found = WALKER.search(line)
        if found:
            walker.append(found)
    kinds = ['down' if w['down'] else 'bled' if w['bled'] else 'wind' if w['wind'] else 'respawn' for w in walker]
    results.append(('down_sequence', kinds == ['down', 'bled', 'respawn', 'down', 'wind'], ' '.join(kinds) or 'no Walker lines'))
    timing_ok, detail = False, 'sequence incomplete'
    if kinds[:3] == ['down', 'bled', 'respawn']:
        down, bled = walker[0], walker[1]
        total, t_down, t_bled = float(down['total']), float(down['t1']), float(bled['t2'])
        expected = t_down + TARGETED_DELAY + total
        respawn_t = [float(m['t']) for _, m in [(0, HOST_DOWN.search(l)) for l in lines] if m and m['phase'] == 'up'
                     and float(m['t']) > t_bled]
        timing_ok = abs(total - 12.0) < 1e-3 and abs(t_bled - expected) <= 0.1 and bool(respawn_t) \
            and abs(respawn_t[0] - t_bled - DEATH_CAMERA) <= 0.1
        detail = (f'total {total:.2f} s; bled out {t_bled - t_down:.3f} s after the injury (expected {TARGETED_DELAY + total:.2f}); '
                  f'respawn {respawn_t[0] - t_bled if respawn_t else float("nan"):.3f} s after (expected {DEATH_CAMERA})')
    results.append(('down_timing', timing_ok, detail))
    wind = [w for w in walker if w['wind']]
    if wind:
        health, maximum = float(wind[0]['health']), float(wind[0]['max'])
        results.append(('second_wind_health', abs(health - SECOND_WIND * maximum) <= 1.0, f'{health:.0f} of {maximum:.0f}'))
    else:
        results.append(('second_wind_health', False, 'no second wind line'))

    host = [(n, m) for n, line in enumerate(lines) for m in [HOST_DOWN.search(line)] if m]
    phases = []
    for _, m in host:
        key = (m['phase'], m['wind'])
        if not phases or phases[-1] != key:
            phases.append(key)
    want = [('up', '0'), ('injured', '0'), ('dead', '0'), ('up', '0'), ('injured', '0'), ('up', '1')]
    bleeds = [float(m['bleed']) for _, m in host if m['phase'] == 'injured' and m['wind'] == '0']
    first = bleeds[:next((i for i in range(1, len(bleeds)) if bleeds[i] > bleeds[i - 1]), len(bleeds))]
    falling = len(first) > 10 and first[0] == 1.0 and first[-1] <= 0.02
    results.append(('down_states', phases == want and falling,
                    f'{" ".join(p + "/" + w for p, w in phases)}; first bleed-out {len(first)} states {first[0] if first else "-"} -> {first[-1] if first else "-"}'))

    pages = page_lines(lines, PAGE_DOWN)
    problems, acked = [], 0
    start = next((i for i, (n, _) in enumerate(host) if any(p > n for p, _ in pages)), len(host))
    for i in range(start, len(host)):
        n, m = host[i]
        echoes = [p for pn, p in pages if n < pn and (p['phase'], p['bleed'], p['wind']) == (m['phase'], m['bleed'], m['wind'])]
        if not echoes:
            problems.append(f'no page echo of line {n + 1} ({m["phase"]} {m["bleed"]} {m["wind"]})')
            continue
        p = echoes[0]
        acked += 1
        injured = m['phase'] == 'injured'
        if p['dying'] != ('2' if injured else '1'):
            problems.append(f'line {n + 1}: dying frame {p["dying"]}')
        if injured and p['bar'] != str(bar_frame(float(m['bleed']))):
            problems.append(f'line {n + 1}: bleed-out frame {p["bar"]}, expected {bar_frame(float(m["bleed"]))}')
    results.append(('down_page_echo', bool(pages) and not problems and acked > 0,
                    f'{acked} of {len(host) - start} states echoed' + (f'; {problems[:3]}' if problems else '')))
    strings = [STRINGS.search(l)[1] for l in lines if STRINGS.search(l)]
    titles = {p['title'] for _, p in pages if p['phase'] == 'injured'}
    expected = None
    try:
        expected = json.loads(Path(strings_file).read_text(encoding='utf-8')).get('injured')
    except (OSError, ValueError):
        pass
    if not strings or strings[-1] == 'none' or expected is None:
        results.append(('down_page_texts', False, 'no hud_strings.json loaded or readable (tools/prepare_hud_strings.py): local data missing'))
    else:
        # The values are the install's own text: report only whether they match, never the text itself.
        results.append(('down_page_texts', bool(titles) and titles == {esc(expected)},
                        f'strings {strings[-1]}; {len(titles)} distinct injured title(s), '
                        + ('equal to the strings file' if titles == {esc(expected)} else 'NOT equal to the strings file')))
    banner = [p for _, p in pages if p['wind'] == '1' and p['windFrame'] not in ('1', 'null', 'undefined')]
    results.append(('second_wind_banner', bool(banner), f'{len(banner)} page lines with the banner playing'))
    return results


def bleed_bar(shot):
    """(red pixels in the bleed-out box, the rightmost red column or -1)."""
    rows, bpp = shot
    x0, y0, x1, y1 = BLEED_BOX
    right = max((x for row in rows[y0:y1] for x in range(x0, x1) if is_health(row[x * bpp], row[x * bpp + 1], row[x * bpp + 2])),
                default=-1)
    return count(shot, BLEED_BOX, is_health), right


def check_down_shots(folder):
    bad, bars = [], {}
    for name in DOWN_SHOTS:
        try:
            bars[name] = bleed_bar(read_shot(Path(folder) / f'OWHudDown_{name}.png'))
        except ValueError as error:
            bad.append(f'{name}: {error}')
    for name, (red, _) in bars.items():
        shown = name in ('down_start', 'down_mid')
        if shown and red < BLEED_SHOWN or not shown and red > BLEED_HIDDEN:
            bad.append(f'{name}: {red} bleed-bar pixels ({"panel missing" if shown else "panel shown"})')
    if 'down_start' in bars and 'down_mid' in bars and not bars['down_mid'][1] < bars['down_start'][1] - 40:
        bad.append(f'bar not shorter at down_mid (right edge {bars["down_mid"][1]} against {bars["down_start"][1]})')
    summary = ' '.join(f'{n}={r}' for n, (r, _) in bars.items())
    return 'down_shots', not bad, '; '.join(bad) or f'{len(DOWN_SHOTS)} captures complete; bleed-bar pixels {summary}'


def expected_modules(rest):
    """[(label, text)] per shown module for a host OWHUDTRACK line's text, and whether it shows a mission."""
    if rest.strip() == 'none':
        return [], False
    found = re.fullmatch(r'mission=(?P<mission>.*) turnIn=(?P<turnIn>[01]) lines=(?P<lines>.*)', rest)
    rows = [('MissionName', found['mission'])]
    if found['turnIn'] == '1':
        rows.append(('TurnIn', None))
    else:
        for item in filter(None, found['lines'].split('|')):
            rows.append(({'[x]': 'Complete', '[o]': 'Optional'}.get(item[:3], 'NotComplete'), item[3:]))
    return rows[:6], True


def check_tracker(lines):
    host = [(n, m['rest'].strip()) for n, line in enumerate(lines) for m in [HOST_TRACK.search(line)] if m]
    pages = page_lines(lines, PAGE_TRACK)
    problems, checked, shown = [], 0, []
    for i, (n, rest) in enumerate(host):
        rows, has_mission = expected_modules(rest)
        if has_mission:
            shown.append(rest)
        after = [p for pn, p in pages if pn > n and (pn < host[i + 1][0] if i + 1 < len(host) else True)]
        if not after:
            problems.append(f'no page line after host line {n + 1}')
            continue
        p = after[-1]
        modules = dict(re.findall(r'm(\d)=(\S+)', p['modules']))
        checked += 1
        for k in range(1, 7):
            got = modules.get(str(k), '')
            if k > len(rows):
                if got.split('/')[0] != '1':
                    problems.append(f'line {n + 1}: module {k} shown ({got})')
                continue
            label, text = rows[k - 1]
            parts = got.split('/', 2)
            if len(parts) < 3 or parts[0] != str(MODULE_SHOWN) or parts[1] != str(INNER_FRAMES[label]) \
                    or (text is not None and parts[2] != esc(text)):
                problems.append(f'line {n + 1}: module {k} {got}, expected {label} {text!r}')
    results = [('tracker_page', checked > 0 and not problems, f'{checked} of {len(host)} states checked' + (f'; {problems[:3]}' if problems else ''))]
    turn_in = any('turnIn=1' in s for s in shown)
    results.append(('tracker_progress', len(shown) >= 3 and turn_in, f'{len(shown)} mission states, turn-in shown: {turn_in}'))
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('log')
    parser.add_argument('--mode', choices=('down', 'tracker'), required=True)
    parser.add_argument('--shots', help='capture folder (down mode)')
    parser.add_argument('--strings', default=str(STRINGS_FILE), help='the hud_strings.json the page loaded (down mode)')
    args = parser.parse_args()
    lines = Path(args.log).read_text(encoding='utf-8', errors='replace').splitlines()
    results = [check_path(lines), check_identity(lines)]
    if args.mode == 'down':
        results += check_down(lines, args.strings)
        if args.shots:
            results.append(check_down_shots(args.shots))
    else:
        results += check_tracker(lines)
    for name, ok, detail in results:
        print(f'OWHUDDOWNTEST {"PASS" if ok else "FAIL"} {name} {detail}')
    passed = sum(1 for _, ok, _ in results if ok)
    print(f'OWHUDDOWNTEST {passed}/{len(results)} passed')
    return 0 if passed == len(results) else 1


if __name__ == '__main__':
    sys.exit(main())
