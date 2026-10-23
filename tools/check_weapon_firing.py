"""Check an -owfiringtest run: one PASS/FAIL per gun from the OWFIRING log lines and the recipes' firing blocks.

tools/test_weapon_firing.ps1 starts the host (Sanctuary_P, -owfiringtest) and passes its log here. The recipes are looked
up by id in the same folders the host loads, in the same order (-owitems first, then -owitemdir), so the first match wins
as it does in the host. Counts come from the host's own counters (FOpenWillowFiringStats); this script only checks them
against the recipe. Output: a table and an exit code (0 every gun PASS, 1 any FAIL, 2 no results or bad arguments).

Expected values, mirrored from the host's FOpenWillowFiringData::Parse (rules UNVERIFIED in game; see
docs/verification/NATIVE_WEAPON_FIRING.md and WEAPON_PROJECTILES.md when it exists):
- kind: Rocket for fire_type EWWFT_Rocket, Trace for EWWFT_HitScan, Bullet otherwise; Trace without a firing block.
- pulls per gun: 5 at the dummy, 3 at a wall, plus 20 at the dummy when the firing block has a status block; a gun
  with burst_count N > 1 fires N shots per pull.
- a gun whose shots explode (a splash block on a bullet, or a rocket that detonates on impact) must splash the dummy.
- a Tediore gun (reload_throw) is reloaded with 5 rounds left: the throw hits, and the refill moves a full magazine.
- a burst gun is reloaded, and respawned, right after a burst's first shot: no shot may follow either.
- pellets = shots x card projectiles; launches = pellets for Bullet and Rocket, 0 for Trace.
- direct hit damage = card damage per direct hit (Trace and Bullet); a rocket does no direct damage.
- splash damage per explosion hit <= the firing block's splash damage (its card-damage fallback when null).
- status: procs only with a status block; ticks only after a proc; each proc with a duration and dps ticks at least once.
"""
import argparse
import json
import math
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
DEFAULT_ITEM_DIRS = [REPO / 'local/items', REPO / 'local/items/slice', REPO / 'local/items/firing/coverage',
                     REPO / 'local/items/firing/fixtures']
TICK_INTERVAL = 0.33  # DOT period measured live (weapon_firing::DotPeriod); a final partial tick is the +1
PHASE_A_PULLS = 5
WALL_PULLS = 3
STATUS_PULLS = 20

RESULT = re.compile(r'OWFIRING result (.*)$')
SEGMENT = re.compile(r'OWFIRING segment (.*)$')
THROW = re.compile(r'OWFIRING throw (.*)$')
LIFECYCLE = re.compile(r'OWFIRING lifecycle (.*)$')
SKIPPED = re.compile(r'OWFIRING skipped id=(\S+) reason=(.*)$')
NO_WALL = re.compile(r'OWFIRING no wall in reach for id=(\S+)')
PARSED = re.compile(r'firing test parsed \d+ recipe IDs: (\S+)')
KV = re.compile(r'(\w+)=(\S+)')


def fields(body):
    return {key: value for key, value in KV.findall(body)}


def read_log(text):
    """(results by id, phase-a segments by id, skipped {id: reason}, no-wall ids, gun list from the run's own parse,
    Tediore throw lines by id, burst lifecycle lines {id: {case: fields}})."""
    results, segments, skipped, no_wall, guns = {}, {}, {}, set(), None
    throws, lifecycles = {}, {}
    for line in text.splitlines():
        if match := THROW.search(line):
            body = fields(match.group(1))
            throws[body['id']] = body
            continue
        if match := LIFECYCLE.search(line):
            body = fields(match.group(1))
            lifecycles.setdefault(body['id'], {})[body['case']] = body
            continue
        if match := RESULT.search(line):
            body = fields(match.group(1))
            results[body['id']] = body
        elif match := SEGMENT.search(line):
            body = fields(match.group(1))
            if body.get('index') == '0' and body.get('wall') == '0':
                segments[body['id']] = body
        elif match := SKIPPED.search(line):
            skipped[match.group(1)] = match.group(2).strip()
        elif match := NO_WALL.search(line):
            no_wall.add(match.group(1))
        elif guns is None and (match := PARSED.search(line)):
            guns = [gun for gun in match.group(1).split(',') if gun and gun != 'none']
    return results, segments, skipped, no_wall, guns, throws, lifecycles


def find_recipe(item_dirs, gun):
    for folder in item_dirs:
        path = Path(folder) / f'{gun}.json'
        if path.is_file():
            return json.loads(path.read_text(encoding='utf-8')), str(path)
    return None, None


def expectations(recipe):
    """What the host should do with this recipe (see the module docstring)."""
    card = recipe['stats']['card']
    firing = recipe.get('firing') or {}
    fire_type = firing.get('fire_type')
    if not firing:
        kind = 'Trace'
    else:
        kind = {'EWWFT_Rocket': 'Rocket', 'EWWFT_HitScan': 'Trace'}.get(fire_type, 'Bullet')
    splash = (firing.get('projectile') or {}).get('splash')
    splash_damage = None
    if splash:
        splash_damage = splash.get('damage')
        if splash_damage is None:
            splash_damage = float(card['damage']) * float(splash.get('damage_scale') or 1)
    status = firing.get('status') or None
    projectile = firing.get('projectile') or {}
    return {
        'kind': kind,
        'burst': max(1, int(firing.get('burst_count') or 0)),
        # Bullets with an explode behaviour always explode; rockets when the definition detonates on impact.
        'explodes': splash is not None and (kind != 'Rocket' or bool(projectile.get('explode_on_impact'))),
        'throw': bool(firing.get('reload_throw')),
        'projectiles': int(card.get('projectiles') or 1),
        'damage': float(card.get('damage') or 0),
        'splash': splash is not None,
        'splash_damage': splash_damage,
        'status': status is not None,
        'duration': float((status or {}).get('duration') or 0),
        'dps': float((status or {}).get('dps') or 0),
    }


def num(value):
    return float(value) if value is not None else None


def check_gun(gun, log, seg, expected, skipped_reason, wall_skipped, throw=None, lifecycle=None):
    """(list of failure reasons, notes, values for the table) for one gun."""
    fails, notes = [], []
    if skipped_reason is not None:
        return [f'skipped by the host: {skipped_reason}'], notes, {}
    if log is None:
        return ['no OWFIRING result line for this gun'], notes, {}
    shots, pellets = int(log['shots']), int(log['pellets'])
    launches, impacts = int(log['launches']), int(log['impacts'])
    target_hits = int(log['target_hits'])
    damage, direct, splash = num(log['damage']), num(log['direct']), num(log['splash'])
    splash_hits = int(log['splash_hits'])
    if expected['burst'] > 1:
        for case in ('reload', 'respawn'):
            entry = (lifecycle or {}).get(case)
            if entry is None:
                fails.append(f'burst gun: no OWFIRING lifecycle line for {case}')
            elif entry['first_shot'] != '1':
                fails.append(f'burst lifecycle {case}: the burst never started')
            elif entry['extra_shots'] != '0':
                fails.append(f"burst lifecycle {case}: {entry['extra_shots']} shots after the {case}")
    if expected['throw']:
        if throw is None:
            fails.append('Tediore gun: no OWFIRING throw line')
        else:
            moved, magazine, size = int(throw['moved']), int(throw['magazine']), int(throw['size'])
            if moved != size or magazine != size:
                fails.append(f'throw refill moved {moved}, magazine {magazine}, expected a full {size} from the reserve')
            if int(throw['target_hits']) + int(throw['splash_hits']) == 0:
                fails.append('the thrown gun hit nothing')
            # The throw's hits are not bullets: take them out of the shot checks below.
            target_hits -= int(throw['target_hits'])
            direct -= num(throw['direct'])
            splash -= num(throw['splash'])
            splash_hits -= int(throw['splash_hits'])
    procs, rolls, ticks = int(log['procs']), int(log['status_rolls']), int(log['ticks'])
    if log['kind'] != expected['kind']:
        fails.append(f"kind {log['kind']} != expected {expected['kind']}")
    pulls = PHASE_A_PULLS + (0 if gun in wall_skipped else WALL_PULLS) + (STATUS_PULLS if expected['status'] else 0)
    # A burst gun also fires one shot in each of its two lifecycle cases (reload, respawn) before the interruption.
    want = pulls * expected['burst'] + (2 if expected['burst'] > 1 else 0)
    if shots != want:
        fails.append(f"shots {shots} != pulls {pulls} x burst {expected['burst']} (+2 lifecycle shots for a burst gun) "
                     '(a pull without a shot, a short burst, or a skipped segment)')
    if pellets != shots * expected['projectiles']:
        fails.append(f"pellets {pellets} != shots {shots} x card projectiles {expected['projectiles']}")
    if expected['kind'] == 'Trace':
        if launches != 0:
            fails.append(f'trace launched {launches} projectiles')
    elif launches != pellets:
        fails.append(f'launches {launches} != pellets {pellets}')
    if target_hits > impacts:
        fails.append(f'{target_hits} target hits but {impacts} impacts')
    if gun in wall_skipped:
        notes.append('no wall in reach: wall pulls skipped')
    # Phase (a): the five pulls at the dummy, logged on their own (OWFIRING segment).
    phase_a_hits = int(seg['target_hits']) if seg else None
    if seg is None:
        fails.append('no phase (a) segment line (the five dummy pulls)')
    elif phase_a_hits == 0:
        if expected['kind'] == 'Rocket':
            notes.append('rocket missed the dummy in phase (a)')
        else:
            fails.append('no direct hit on the dummy in phase (a)')
    # Direct damage: the card damage per direct hit, nothing for a rocket.
    if expected['kind'] == 'Rocket':
        if direct > 0.005:
            fails.append(f'rocket did direct damage {direct:.2f}')
    else:
        want = target_hits * expected['damage']
        if abs(direct - want) > 0.01 * target_hits + 0.006:
            fails.append(f'direct damage {direct:.2f} != {target_hits} hits x card damage {expected["damage"]:.2f}')
    # Splash.
    if not expected['splash']:
        if splash > 0.005 or splash_hits:
            fails.append('splash damage or hits without a splash block')
    elif expected['explodes'] and splash_hits == 0:
        fails.append('shots aimed at the dummy explode but splashed nothing')
    elif splash_hits:
        per_hit = splash / splash_hits
        if per_hit > expected['splash_damage'] + 0.01:
            fails.append(f'splash per hit {per_hit:.2f} > firing splash damage {expected["splash_damage"]:.2f}')
    # Status effects.
    if not expected['status']:
        if procs or rolls or ticks:
            fails.append('status counts without a status block')
    else:
        if procs > rolls:
            fails.append(f'procs {procs} > rolls {rolls}')
        if ticks and not procs:
            fails.append(f'{ticks} ticks without a proc')
        if procs and expected['duration'] > 0 and expected['dps'] > 0 and ticks < 1:
            fails.append(f'{procs} procs but no damage tick')
        if procs and expected['duration'] <= 0:
            notes.append('status has no duration in the recipe: the host applies nothing (procs only)')
        if expected['duration'] > 0:
            most = procs * (math.floor(expected['duration'] / TICK_INTERVAL) + 1)
            if ticks > most:
                fails.append(f'ticks {ticks} > {most} possible for {procs} procs of {expected["duration"]:g} s')
        if rolls and not procs:
            notes.append(f'no proc in {rolls} status rolls')
    values = {'kind': log['kind'], 'shots': shots, 'pellets': pellets, 'launches': launches, 'hits_a': phase_a_hits,
              'target_hits': target_hits, 'damage': damage, 'splash': splash, 'procs': procs, 'ticks': ticks}
    return fails, notes, values


def table(rows):
    head = ['GUN', 'KIND', 'SHOTS', 'PELLETS', 'LAUNCH', 'HITS(a)', 'HITS', 'DAMAGE', 'SPLASH', 'PROCS', 'TICKS', 'RESULT']
    lines = ['  '.join(f'{h:<{w}}' for h, w in zip(head, WIDTHS))]
    for row in rows:
        lines.append('  '.join(f'{str(value):<{w}}' for value, w in zip(row, WIDTHS)))
    return '\n'.join(lines)


WIDTHS = [24, 8, 6, 7, 7, 8, 6, 10, 10, 6, 6, 7]


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    parser.add_argument('log', help='the host log (Saved log from -abslog)')
    parser.add_argument('--items', action='append', default=None,
                        help='recipe folder, in the host load order (repeat; default: local/items, local/items/slice, '
                             'local/items/firing/coverage)')
    parser.add_argument('--guns', default=None, help='comma-separated recipe ids (default: the run\'s own list)')
    args = parser.parse_args(argv)
    item_dirs = [Path(p) for p in args.items] if args.items else DEFAULT_ITEM_DIRS
    text = Path(args.log).read_text(encoding='utf-8', errors='replace')
    results, segments, skipped, no_wall, run_guns, throws, lifecycles = read_log(text)
    guns = args.guns.split(',') if args.guns else run_guns
    if not guns:
        print('no gun list: the log has no "firing test parsed" line and --guns is missing', file=sys.stderr)
        return 2
    if not results and not skipped:
        print('no OWFIRING result lines in the log', file=sys.stderr)
        return 2
    rows, failed = [], 0
    for gun in guns:
        recipe, path = find_recipe(item_dirs, gun)
        if recipe is None:
            failed += 1
            rows.append([gun, '-', '-', '-', '-', '-', '-', '-', '-', '-', '-', 'FAIL'])
            print(f'{gun}: FAIL no recipe in {[str(p) for p in item_dirs]}')
            continue
        expected = expectations(recipe)
        fails, notes, values = check_gun(gun, results.get(gun), segments.get(gun), expected, skipped.get(gun), no_wall,
                                         throws.get(gun), lifecycles.get(gun))
        verdict = 'FAIL' if fails else 'PASS'
        failed += bool(fails)
        if values:
            rows.append([gun, values['kind'], values['shots'], values['pellets'], values['launches'], values['hits_a'],
                         values['target_hits'], f"{values['damage']:.2f}", f"{values['splash']:.2f}", values['procs'],
                         values['ticks'], verdict])
        else:
            rows.append([gun, expected['kind'], '-', '-', '-', '-', '-', '-', '-', '-', '-', verdict])
        for reason in fails:
            print(f'{gun}: FAIL {reason}')
        for note in notes:
            print(f'{gun}: note {note}')
    print(table(rows))
    print(f'{len(guns) - failed} PASS, {failed} FAIL of {len(guns)} guns')
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
