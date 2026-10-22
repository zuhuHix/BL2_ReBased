"""Check a gameplay-HUD capture run (tools/run_hud_shots.ps1, -owhudshots) from its log and its two captures.

The log (local/hudshots/<stamp>/run.log) must show BL2's HUD movie on the Ruffle warp path (the
`OpenWillow HUD path: movie` line, a `HUD 3D path warp` line, and no canvas or affine fallback), the page's identity
(the overlay URL carries this run's `owid=` id and the page's ready line echoes exactly that id), at least one
OWHUDSTATE line and every one in the host's format (fractions with three decimals, weaponOut 0 or 1, healthText an
integer, level two digits or more, ammoText and grenadeText with the no-break spaces the host writes as the JSON
escape, shield and grenades null or a fraction), the `OWHUDSHOTS fire 1` lines with the magazine dropping by one each
time and the reserve unchanged, and the `OWHUDSHOTS done` line. The page read-back (OWHUDPAGE, logged through the page
console) must come from at least three page reads. Each page read is paired with the OWHUDSTATE line logged just before
it and must echo that state (page_state_from_host); every state from the first paired one on must have a page read
after it (page_acks_each_state); the frames and texts must be the ones the page should show for its own echo
(page_frames_match_state); and the last page must show the final magazine (page_follows_fire). With --expect-shield
the log must also show the shield being equipped. The captures (OWHud_rest.png and OWHud_fired.png, 1280x720) must be
complete PNGs (every chunk's CRC, IHDR first, IEND last, the IDAT stream inflating to the image's size), not blank, and
show the health bar (red) and the ammo bar (blue/violet) in fixed screen boxes. Whether the captures look like the game
is the critic's call (docs/verification/HUD_PARITY.md), not this check's.
Prints one `OWHUDSHOTSTEST PASS|FAIL|SKIP <name> <detail>` line per check and a summary. Exit code 0 only when every
check passes; without --shots the summary is PARTIAL and the exit code 2.
"""
import argparse
import math
import re
import struct
import sys
import zlib
from pathlib import Path

PNG_SIGNATURE = b'\x89PNG\r\n\x1a\n'
SHOT_SIZE = (1280, 720)
SHOTS = ('OWHud_rest.png', 'OWHud_fired.png')
FIRE_SHOTS = 3                     # run_hud_shots.ps1 fires three rounds
PAGE_MIN = 3                       # OWHUDPAGE read-backs a run must log
# Screen boxes (x0, y0, x1, y1) of the health bar and the ammo bar on the 1280x720 HUD, and the pixel counts
# the colour tests need inside them. The real run local/hudshots/20261022-173219 has about 3040 red pixels in the
# health box and 2800 to 4300 blue/violet in the ammo box; the minimum is about a third of the lowest of those.
HEALTH_BOX, AMMO_BOX = (0, 640, 300, 700), (1000, 640, 1250, 700)
HEALTH_MIN, AMMO_MIN = 1000, 1000
NOT_BLANK_MIN_COLOURS = 64
# The host writes each no-break space as this six-character JSON escape, not as a character.
NBSP = r'\\u00a0'
FRACTION = r'[0-9]+\.[0-9]{3}'
# The 11 fields OWHUDSTATE and the echo part of OWHUDPAGE share, in the host's order.
STATE_FIELDS = ('health', 'shield', 'ammo', 'grenades', 'weaponOut', 'weaponType',
                'healthText', 'shieldText', 'ammoText', 'grenadeText', 'level')
STATE = re.compile(r'OWHUDSTATE health=(?P<health>\S*) shield=(?P<shield>\S*) ammo=(?P<ammo>\S*) '
                   r'grenades=(?P<grenades>\S*) weaponOut=(?P<weaponOut>\S*) weaponType=(?P<weaponType>\S*) '
                   r'healthText=(?P<healthText>\S*) shieldText=(?P<shieldText>\S*) ammoText=(?P<ammoText>\S*) '
                   r'grenadeText=(?P<grenadeText>\S*) level=(?P<level>\S*)\s*$')
# The page's echo of the state, then what the page showed: the frames of the health, shield, bullets and grenade
# rings, the texts, and the weapon's outer, spread and icon frames. Texts may be empty and are checked by equality.
PAGE = re.compile(r'OWHUDPAGE health=(?P<health>\S*) shield=(?P<shield>\S*) ammo=(?P<ammo>\S*) '
                  r'grenades=(?P<grenades>\S*) weaponOut=(?P<weaponOut>\S*) weaponType=(?P<weaponType>\S*) '
                  r'healthText=(?P<healthText>.*?) shieldText=(?P<shieldText>.*?) ammoText=(?P<ammoText>.*?) '
                  r'grenadeText=(?P<grenadeText>.*?) level=(?P<level>\S*) '
                  r'fHealth=(?P<fHealth>\S*) fShield=(?P<fShield>\S*) fBullets=(?P<fBullets>\S*) '
                  r'fGrenades=(?P<fGrenades>\S*) tHealth=(?P<tHealth>.*?) tShield=(?P<tShield>.*?) '
                  r'tAmmo=(?P<tAmmo>.*?) tGrenade=(?P<tGrenade>.*?) tLevel=(?P<tLevel>.*?) '
                  r'outer=(?P<outer>\S*) spread=(?P<spread>\S*) icon=(?P<icon>\S*)\s*$')
PAGE_SUFFIX = re.compile(r'\s*\(https?://[^)\s]*\)\s*$')      # the console's " (http://...:<line>)" tail
# The overlay URL the host logs (its last query value is the run's owid, 32 letters or digits) and the page's ready line
# (`id=` is the owid the page echoes, or `-` when the page has none to echo).
OVERLAY = re.compile(r'OpenWillow Flash HUD overlay: \S*?[?&]owid=(?P<owid>[0-9A-Za-z]{32})(?![0-9A-Za-z])')
# Only the page's own ready line counts: the host's HUD-console envelope, then the line exactly as the page writes it
# (tools/hud_overlay/hud_fields.js readyLine) with the console's " (http://...)" tail; an older page writes no id.
READY_ENVELOPE = re.compile(r'LogTemp: Display: OpenWillow Flash HUD console: OpenWillow HUD movie ready(?: |$)')
READY = re.compile(r'LogTemp: Display: OpenWillow Flash HUD console: OpenWillow HUD movie ready'
                   r'(?: id=(?P<id>\S+) pages=.*?)? \(https?://[^)\s]*\)\s*$')
SHAPES = {'pistol': ('3', '9', '3'), 'shotgun': ('7', '17', '5')}   # weaponType: outer, spread, icon frames
FORMATS = {
    'health': re.compile(FRACTION),
    'shield': re.compile(rf'null|{FRACTION}'),
    'ammo': re.compile(FRACTION),
    'grenades': re.compile(rf'null|{FRACTION}'),
    'weaponOut': re.compile(r'[01]'),
    'weaponType': re.compile(r'[A-Za-z0-9_]+|-'),
    'healthText': re.compile(r'[0-9]+'),
    'shieldText': re.compile(r'[0-9]*'),
    'ammoText': re.compile(rf'(?:[0-9]+(?:{NBSP}/{NBSP}[0-9]+)?)?'),
    'grenadeText': re.compile(rf'(?:[0-9]+{NBSP}/{NBSP}[0-9]+)?'),
    'level': re.compile(r'[0-9]{2}|[1-9][0-9]{2,}'),
}
AMMO_PAIR = re.compile(rf'([0-9]+){NBSP}/{NBSP}([0-9]+)')
FIRE = re.compile(r'OWHUDSHOTS fire 1\b')
SHIELD = re.compile(r'OWHUDSHOTS shield (.+?)\s*$')
DONE = 'OWHUDSHOTS done'


def where(lines, needle):
    return [number for number, line in enumerate(lines, 1) if needle in line]


def magazine(ammo):
    """(magazine, reserve) of an ammoText value: reserve is None for digits only, magazine None when empty."""
    pair = AMMO_PAIR.fullmatch(ammo)
    if pair:
        return int(pair[1]), int(pair[2])
    if re.fullmatch(r'[0-9]+', ammo):
        return int(ammo), None
    return None, None


def frame(value):
    """The frame a fraction shows, as the page computes it: 1 + round((1 - clamp(f, 0, 1)) * 99), round half up (JS Math.round)."""
    if not re.fullmatch(FRACTION, value):
        return None
    clamped = min(max(float(value), 0.0), 1.0)
    return 1 + math.floor((1 - clamped) * 99 + 0.5)


def read_log(text):
    """The state lines (format problems included) and the fire and shield lines, in the order the log has them."""
    lines = text.splitlines()
    events, bad = [], []
    for number, line in enumerate(lines, 1):
        if 'OWHUDSTATE' in line:
            found = STATE.search(line)
            if not found:
                bad.append(f'line {number}: not the host\'s state format')
                events.append(('state', number, None, (None, None), None))
                continue
            fields = found.groupdict()
            problems = [name for name in STATE_FIELDS if not FORMATS[name].fullmatch(fields[name])]
            if problems:
                bad.append(f'line {number}: {", ".join(problems)}')
            events.append(('state', number, fields['shield'], magazine(fields['ammoText']), fields))
        elif FIRE.search(line):
            events.append(('fire', number))
        elif SHIELD.search(line):
            events.append(('shield', number, SHIELD.search(line)[1]))
    return lines, events, bad


def fire_steps(events):
    """(before, after) (magazine, reserve) around each fire line; (None, None) where no state brackets it."""
    steps = []
    for index, event in enumerate(events):
        if event[0] != 'fire':
            continue
        before = next((e[3] for e in reversed(events[:index]) if e[0] == 'state'), (None, None))
        after = next((e[3] for e in events[index + 1:] if e[0] == 'state'), (None, None))
        steps.append((before, after))
    return steps


def page_problems(page):
    """What one OWHUDPAGE read-back shows wrongly for its own echoed state (an empty list when it is right)."""
    problems = []

    def want(name, got, expected):
        if got != str(expected):
            problems.append(f'{name} {got} (want {expected})')

    def above_hundred(name, got):
        if not (got.isdigit() and int(got) > 100):
            problems.append(f'{name} {got} (want over 100)')

    want('fHealth', page['fHealth'], frame(page['health']))
    if page['shield'] == 'null':
        above_hundred('fShield', page['fShield'])
    else:
        want('fShield', page['fShield'], frame(page['shield']))
    if page['grenades'] == 'null':
        above_hundred('fGrenades', page['fGrenades'])
    else:
        want('fGrenades', page['fGrenades'], frame(page['grenades']))
    if page['weaponOut'] == '1':
        want('fBullets', page['fBullets'], frame(page['ammo']))
    else:
        above_hundred('fBullets', page['fBullets'])
    for shown, echoed in (('tHealth', 'healthText'), ('tShield', 'shieldText'), ('tAmmo', 'ammoText'),
                          ('tGrenade', 'grenadeText'), ('tLevel', 'level')):
        if page[shown] != page[echoed]:
            problems.append(f'{shown} "{page[shown]}" != {echoed} "{page[echoed]}"')
    if page['weaponType'] in SHAPES:
        outer, spread, icon = SHAPES[page['weaponType']]
        want('outer', page['outer'], outer)
        want('spread', page['spread'], spread)
        want('icon', page['icon'], icon)
    return problems


def pair_pages(lines, states):
    """Each OWHUDPAGE line as (line number, parsed echo or None, index in states of the OWHUDSTATE logged just before it)."""
    pairs = []
    for number, line in enumerate(lines, 1):
        if 'OWHUDPAGE' in line:
            body = PAGE_SUFFIX.sub('', line[line.index('OWHUDPAGE'):])
            before = [index for index, (state_line, _) in enumerate(states) if state_line < number]
            pairs.append((number, PAGE.match(body), before[-1] if before else None))
    return pairs


def describe(fields):
    return f'health {fields["health"]} ammoText {fields["ammoText"]}' if fields else 'not the host format'


def check_identity(lines):
    """page_identity: the overlay URL carries this run's owid and the page's ready line echoes exactly that id."""
    overlay = [line for line in lines if 'OpenWillow Flash HUD overlay:' in line]
    ready = [line for line in lines if READY_ENVELOPE.search(line)]
    if not overlay:
        return 'page_identity', False, 'no "OpenWillow Flash HUD overlay:" line'
    found = OVERLAY.search(overlay[-1])
    if not found:
        return 'page_identity', False, f'overlay URL has no owid=<32 letters or digits>: {overlay[-1].strip()[-120:]}'
    owid = found['owid']
    if not ready:
        return 'page_identity', False, 'no "OpenWillow HUD movie ready" line'
    form = READY.search(ready[-1])
    if not form:
        return 'page_identity', False, f"ready line not in the page's form: {ready[-1].strip()[-120:]}"
    echoed = form['id']
    if echoed is None:
        return 'page_identity', False, 'ready line has no id= (older pages)'
    if echoed != owid:
        return 'page_identity', False, (f'ready line id {echoed}, overlay owid {owid} '
                                        '(the page did not echo this run\'s id)')
    return 'page_identity', True, f'ready line echoes the overlay owid {owid}'


def check_pages(lines, states):
    """The OWHUDPAGE read-backs against the host's states; states is a list of (line, fields dict or None)."""
    pages = pair_pages(lines, states)
    results = [('page_lines', len(pages) >= PAGE_MIN, f'{len(pages)} OWHUDPAGE lines, need at least {PAGE_MIN}')]
    if not pages:
        for name in ('page_state_from_host', 'page_acks_each_state', 'page_frames_match_state', 'page_follows_fire'):
            results.append((name, False, 'no OWHUDPAGE lines'))
        return results
    malformed = [number for number, found, _ in pages if not found]
    problems = []
    for number, found, index in pages:
        if not found:
            problems.append(f'line {number} not the page format')
        elif index is None:
            problems.append(f'line {number} has no OWHUDSTATE line before it')
        elif states[index][1] is None:
            problems.append(f'line {number} follows line {states[index][0]}, not the host format')
        else:
            host = states[index][1]
            differ = [name for name in STATE_FIELDS if found[name] != host[name]]
            if differ:
                problems.append(f'line {number} differs from state line {states[index][0]} in {", ".join(differ)}')
    results.append(('page_state_from_host', not problems,
                    f'{len(pages)} page echoes match the state logged before them' if not problems
                    else f'{len(problems)} wrong, first ' + '; '.join(problems[:3])))
    if not states:
        results.append(('page_acks_each_state', False, 'no OWHUDSTATE line'))
    elif pages[0][2] is None:
        results.append(('page_acks_each_state', False,
                        f'first OWHUDPAGE line {pages[0][0]} has no OWHUDSTATE line before it'))
    else:
        first = pages[0][2]     # the states logged before the first page were superseded before it existed
        acked = {index for _, _, index in pages if index is not None}
        missing = [index for index in range(first, len(states)) if index not in acked]
        results.append(('page_acks_each_state', not missing,
                        f'every OWHUDSTATE from line {states[first][0]} on has a page line' if not missing else
                        f'{len(missing)} states with no page line, first ' + '; '.join(
                            f'line {states[index][0]} ({describe(states[index][1])})' for index in missing[:3])))
    parsed = [(number, found) for number, found, _ in pages if found]
    wrong = [(number, problems) for number, found in parsed for problems in [page_problems(found)] if problems]
    frame_detail = (f'{len(wrong)} of {len(parsed)} pages off, first ' + '; '.join(
        f'line {number}: {", ".join(problems)}' for number, problems in wrong[:3])) if wrong else \
        f'{len(parsed)} pages show the frames and texts of their state'
    results.append(('page_frames_match_state', bool(parsed) and not malformed and not wrong, frame_detail))
    last_page = pages[-1][1]
    last_state = states[-1][1] if states else None
    if last_page is None:
        follow = (False, f'last OWHUDPAGE line (line {pages[-1][0]}) is not the page format')
    elif not states:
        follow = (False, 'no OWHUDSTATE line to compare')
    elif last_state is None:
        follow = (False, f'last OWHUDSTATE line (line {states[-1][0]}) is not the host format')
    else:
        follow = (last_page['ammoText'] == last_state['ammoText'],
                  f'last page ammoText "{last_page["ammoText"]}", last state "{last_state["ammoText"]}"')
    results.append(('page_follows_fire', follow[0], follow[1]))
    return results


def check_log(text, expect_shield):
    lines, events, bad = read_log(text)
    states = [e for e in events if e[0] == 'state']
    movie, canvas = where(lines, 'OpenWillow HUD path: movie'), where(lines, 'OpenWillow HUD path: canvas')
    warp, affine = where(lines, 'OpenWillow HUD 3D path warp'), where(lines, 'OpenWillow HUD 3D path affine')
    results = [
        ('movie_path', bool(movie) and not canvas,
         f'movie line {movie[:1]}, canvas lines {canvas[:3]}'),
        ('warp_path', bool(warp) and not affine,
         f'warp line {warp[:1]}' if not affine else
         f'affine fallback at line {affine[0]}: {lines[affine[0] - 1].strip()[-160:]}'),
        check_identity(lines),
        ('state_lines', bool(states), f'{len(states)} OWHUDSTATE lines'),
        ('state_format', bool(states) and not bad,
         f'{len(bad)} of {len(states)} off-format, first {bad[:2]}' if bad else f'{len(states)} lines in the host format'),
    ]
    results += check_pages(lines, [(e[1], e[4]) for e in states])
    if expect_shield:
        shields = [e for e in events if e[0] == 'shield']
        drawn = [e for e in events if e[0] == 'state' and e[2] is not None and e[2] != 'null'
                 and any(s[1] < e[1] for s in shields)]
        results.append(('shield_shown', bool(shields) and bool(drawn),
                        f'shield line {shields[0][2] if shields else None}, {len(drawn)} later states with a shield'))
    else:
        results.append(('shield_shown', None, 'not requested (--expect-shield)'))
    steps = fire_steps(events)
    shown = [f'{b[0]}->{a[0]}' for b, a in steps]
    ok = len(steps) == FIRE_SHOTS and all(
        b[0] is not None and a[0] == b[0] - 1 and b[1] == a[1] for b, a in steps)
    ok = ok and steps[-1][1][0] == steps[0][0][0] - FIRE_SHOTS
    results.append(('fire_counts', ok,
                    f'{len(steps)} fire lines, magazine {", ".join(shown)}' if steps else 'no fire lines'))
    results.append(('done', DONE in text, f'{DONE} logged: {DONE in text}'))
    return results


def read_shot(path):
    """(rows, bytes per pixel) of a complete 1280x720 8-bit RGB/RGBA non-interlaced PNG; ValueError with the reason otherwise."""
    data = path.read_bytes() if path.is_file() else b''
    if data[:8] != PNG_SIGNATURE:
        raise ValueError('not a PNG')
    pos, kinds, idat, header = 8, [], [], None
    while pos < len(data):
        if pos + 8 > len(data):
            raise ValueError('truncated chunk header')
        length, kind = struct.unpack('>I4s', data[pos:pos + 8])
        if pos + 12 + length > len(data):
            raise ValueError(f'truncated {kind.decode("ascii", "replace")} chunk')
        body = data[pos + 8:pos + 8 + length]
        if zlib.crc32(kind + body) != struct.unpack('>I', data[pos + 8 + length:pos + 12 + length])[0]:
            raise ValueError(f'CRC fails in {kind.decode("ascii", "replace")}')
        kinds.append(kind)
        if kind == b'IHDR' and length == 13:
            header = struct.unpack('>IIBBBBB', body)
        elif kind == b'IDAT':
            idat.append(body)
        pos += 12 + length
    if not kinds or kinds[0] != b'IHDR' or kinds[-1] != b'IEND' or not idat or header is None:
        raise ValueError('chunks out of order or missing')
    width, height, depth, colour, _, _, interlace = header
    if (width, height) != SHOT_SIZE:
        raise ValueError(f'{width}x{height}, not 1280x720')
    if depth != 8 or colour not in (2, 6) or interlace:
        raise ValueError(f'unsupported PNG (depth {depth}, colour type {colour}, interlace {interlace})')
    bpp = 3 if colour == 2 else 4
    stride = width * bpp
    try:
        raw = zlib.decompress(b''.join(idat))
    except zlib.error:
        raise ValueError('IDAT does not inflate')
    if len(raw) != height * (1 + stride):
        raise ValueError('IDAT inflates to the wrong size')
    rows, prev = [], bytearray(stride)
    for y in range(height):   # the PNG row filters (PNG specification, section 9)
        start = y * (stride + 1)
        kind, line = raw[start], bytearray(raw[start + 1:start + 1 + stride])
        if kind > 4:
            raise ValueError(f'unknown filter {kind}')
        for i in range(stride if kind else 0):
            a = line[i - bpp] if i >= bpp else 0
            b, c = prev[i], (prev[i - bpp] if i >= bpp else 0)
            if kind == 1:
                line[i] = (line[i] + a) & 255
            elif kind == 2:
                line[i] = (line[i] + b) & 255
            elif kind == 3:
                line[i] = (line[i] + (a + b) // 2) & 255
            else:
                pa, pb, pc = abs(b - c), abs(a - c), abs(a + b - 2 * c)
                line[i] = (line[i] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        rows.append(line)
        prev = line
    return rows, bpp


def count(shot, box, test):
    """Pixels inside a screen box whose (r, g, b) passes the test."""
    rows, bpp = shot
    x0, y0, x1, y1 = box
    return sum(1 for row in rows[y0:y1] for x in range(x0, x1)
               if test(row[x * bpp], row[x * bpp + 1], row[x * bpp + 2]))


def is_health(r, g, b):
    return r > 150 and g < 80 and b < 90


def is_ammo(r, g, b):
    return b > 150 and r < 140


def check_shots(folder):
    """The captures: complete 1280x720 PNGs, not blank, with the health bar and the ammo bar drawn."""
    folder = Path(folder)
    shots, errors = {}, []
    for name in SHOTS:
        try:
            shots[name] = read_shot(folder / name)
        except (ValueError, OSError) as error:
            errors.append(f'{name}: {error}')
    results = [('shots_valid', not errors and len(shots) == len(SHOTS),
                f'both 1280x720 PNGs, complete' if not errors else '; '.join(errors))]
    colours = {}
    for name, (rows, bpp) in shots.items():
        colours[name] = len({bytes(row[i:i + bpp]) for row in rows[::4] for i in range(0, len(row), bpp)})
    results.append(('shots_not_blank', bool(colours) and all(c >= NOT_BLANK_MIN_COLOURS for c in colours.values()),
                    f'distinct colours (sampled rows) {colours}'))
    red = {n: count(s, HEALTH_BOX, is_health) for n, s in shots.items()}
    ammo = {n: count(s, AMMO_BOX, is_ammo) for n, s in shots.items()}
    results.append(('health_bar_drawn', len(red) == len(SHOTS) and all(v > HEALTH_MIN for v in red.values()),
                    f'red pixels in the health box {red}, need > {HEALTH_MIN}'))
    results.append(('ammo_bar_drawn', len(ammo) == len(SHOTS) and all(v > AMMO_MIN for v in ammo.values()),
                    f'blue/violet pixels in the ammo box {ammo}, need > {AMMO_MIN}'))
    return results


def run_checks(text, shots=None, expect_shield=False):
    results = check_log(text, expect_shield)
    if shots is not None:
        results += check_shots(shots)
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('log')
    parser.add_argument('--shots', help=f'folder holding {" and ".join(SHOTS)} of this run (needed for a PASS)')
    parser.add_argument('--expect-shield', action='store_true', help='the run equipped a shield (run_hud_shots.ps1 -Shield)')
    args = parser.parse_args()
    text = Path(args.log).read_text(encoding='utf-8-sig', errors='replace')
    results = run_checks(text, args.shots, args.expect_shield)
    for name, ok, detail in results:
        if ok is None:
            print(f'OWHUDSHOTSTEST SKIP {name} {detail}')
        else:
            print(f'OWHUDSHOTSTEST {"PASS" if ok else "FAIL"} {name} {detail}')
    failed = sum(1 for _, ok, _ in results if ok is False)
    passed = sum(1 for _, ok, _ in results if ok)
    result = 'FAIL' if failed else 'PASS' if args.shots else 'PARTIAL'
    print(f'OWHUDSHOTSTEST SUMMARY result={result} passed={passed} failed={failed}'
          + ('' if args.shots else ' (captures not checked: no --shots)'))
    return 1 if failed else 0 if args.shots else 2


if __name__ == '__main__':
    sys.exit(main())
