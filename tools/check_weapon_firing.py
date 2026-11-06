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
- momentum (OWFIRING push, after the five dummy pulls): the gun's momentum is 100 x the block's instant_hit_momentum
  (class default 10 when null); every momentum-carrying hit changed the dummy's velocity by momentum / mass; a bullet
  or trace gun without splash passed exactly its momentum; the dummy moved along the shots when it was hit.
- flight timing (result timed / on_time, when logged): every constant-speed light bullet arrives after distance / speed
  seconds, within the frame that reaches it.
- splash momentum (OWFIRING splash, each splash hit, given to the next result line's gun): momentum = the recipe's splash
  momentum (MomentumFormula; the reload throw's own for source=throw) x the hit's radius scale, and not zero when the
  recipe's is not. There is one splash line per splash hit of the result (shots and throw), the throw's lines match its
  summary, and every logged or expected number must be finite.
- wide bullet (OWFIRING wide; single straight pellets: no pattern, no own motion (air friction or acceleration), no splash,
  no burst): a pull beside the
  dummy as a line misses it, the same pull with a TraceExtent box hits it. Its two pulls count in the shots.
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
WIDE_PULLS = 2

RESULT = re.compile(r'OWFIRING result (.*)$')
SEGMENT = re.compile(r'OWFIRING segment (.*)$')
THROW = re.compile(r'OWFIRING throw (.*)$')
LIFECYCLE = re.compile(r'OWFIRING lifecycle (.*)$')
PUSH = re.compile(r'OWFIRING push (.*)$')
WIDE = re.compile(r'OWFIRING wide (.*)$')
SWAP = re.compile(r'OWFIRING swap (.*)$')
GRENADE = re.compile(r'OWFIRING grenade (thrown=.*)$')
SPLASH = re.compile(r'OWFIRING splash \S+ distance (\S+) scale (\S+) momentum (\S+) source (\w+)')
SKIPPED = re.compile(r'OWFIRING skipped id=(\S+) reason=(.*)$')
NO_WALL = re.compile(r'OWFIRING no wall in reach for id=(\S+)')
PARSED = re.compile(r'firing test parsed \d+ recipe IDs: (\S+)')
KV = re.compile(r'(\w+)=(\S+)')


def fields(body):
    return {key: value for key, value in KV.findall(body)}


def read_log(text, extra=None):
    """(results by id, phase-a segments by id, skipped {id: reason}, no-wall ids, gun list from the run's own parse,
    Tediore throw lines by id, burst lifecycle lines {id: {case: fields}}). With `extra` (a dict), the push and wide
    lines by id go into extra['push'] and extra['wide']."""
    results, segments, skipped, no_wall, guns = {}, {}, {}, set(), None
    throws, lifecycles = {}, {}
    extra = {} if extra is None else extra
    extra.setdefault('push', {})
    extra.setdefault('wide', {})
    extra.setdefault('splash', {})
    extra.setdefault('swap', {})
    pending_splash = []   # splash lines since the last result line: they belong to the next gun's result
    for line in text.splitlines():
        if match := SPLASH.search(line):
            pending_splash.append({'scale': float(match.group(2)), 'momentum': float(match.group(3)),
                                   'source': match.group(4)})
            continue
        if match := PUSH.search(line):
            body = fields(match.group(1))
            extra['push'][body['id']] = body
            continue
        if match := WIDE.search(line):
            body = fields(match.group(1))
            extra['wide'][body['id']] = body
            continue
        if match := GRENADE.search(line):
            extra['grenade'] = fields(match.group(1))
            continue
        if match := SWAP.search(line):
            body = fields(match.group(1))
            extra['swap'][body['id']] = body
            continue
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
            extra['splash'][body['id']] = pending_splash
            pending_splash = []
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
    # A projectile's own splash, else a trace or light-bullet firing mode's impact explosion (its Behavior_Explode on
    # every impact; tools/weapon_firing.py impact_explosion). The host plays either as the gun's splash.
    splash = (firing.get('projectile') or {}).get('splash') or firing.get('impact_explosion')
    splash_damage = None
    if splash:
        splash_damage = splash.get('damage')
        if splash_damage is None:
            splash_damage = float(card['damage']) * float(splash.get('damage_scale') or 1)
    status = firing.get('status') or None
    projectile = firing.get('projectile') or {}
    momentum = firing.get('instant_hit_momentum')
    acceleration = projectile.get('acceleration') or {}
    accelerates = isinstance(acceleration, dict) and any(float(value or 0) != 0 for value in acceleration.values())
    throw_splash = (((firing.get('reload_throw') or {}).get('projectile') or {}).get('splash')) or {}
    projectiles = int(card.get('projectiles') or 1)
    burst = max(1, int(firing.get('burst_count') or 0))
    return {
        'has_firing': bool(firing),
        'momentum': 100.0 * float(10 if momentum is None else momentum),
        'wide': bool(firing) and kind != 'Rocket' and projectiles <= 1 and not firing.get('pattern_lines')
                and not projectile.get('air_friction') and not accelerates and splash is None and burst == 1,
        'splash_momentum': float((splash or {}).get('momentum') or 0),
        'throw_splash_momentum': float(throw_splash.get('momentum') or 0),
        'kind': kind,
        'burst': burst,
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


def not_finite(**values):
    """Names of the values that are not finite numbers (NaN or infinite would slip through every tolerance test)."""
    bad = []
    for name, value in values.items():
        try:
            if not math.isfinite(float(value)):
                bad.append(name)
        except (TypeError, ValueError):
            bad.append(name)
    return bad


def check_momentum(push, wide, expected, phase_a_hits, splash_lines=None, splash_hits=None, throw=None):
    """Failure reasons for the momentum (push, splash) and wide-bullet (wide) lines of one gun. `splash_hits` is the result
    line's total (shots and throw); `throw` the Tediore throw summary line, when there is one."""
    fails = []
    splash_lines = splash_lines or []
    bad = not_finite(momentum=expected['momentum'], splash_momentum=expected['splash_momentum'],
                     throw_splash_momentum=expected['throw_splash_momentum'])
    for line in splash_lines:
        bad += [f'splash {name}' for name in not_finite(scale=line['scale'], momentum=line['momentum'])]
    if push is not None:
        bad += [f'push {name}' for name in not_finite(**{key: push.get(key) for key in
                                                         ('momentum_rule', 'momentum_last', 'mass', 'moved')})]
    if wide is not None:
        bad += [f'wide {name}' for name in not_finite(offset=wide.get('offset'), half=wide.get('half'))]
    if bad:
        return fails + [f"not a finite number: {', '.join(sorted(set(bad)))}"]
    if splash_hits is not None and len(splash_lines) != splash_hits:
        fails.append(f'{splash_hits} splash hits but {len(splash_lines)} OWFIRING splash lines')
    throw_lines = sum(line['source'] == 'throw' for line in splash_lines)
    if throw is not None and throw_lines != int(throw['splash_hits']):
        fails.append(f"the throw logged {throw['splash_hits']} splash hits but {throw_lines} throw splash lines")
    shot_lines = 0
    for line in splash_lines:
        recipe = expected['throw_splash_momentum'] if line['source'] == 'throw' else expected['splash_momentum']
        want = recipe * line['scale']
        shot_lines += line['source'] != 'throw'
        if abs(line['momentum'] - want) > 0.1 + 1e-3 * want:
            fails.append(f"splash momentum {line['momentum']:g} != recipe {recipe:g} x scale {line['scale']:g} = {want:.1f}")
            break
    if expected['explodes'] and expected['splash_momentum'] > 0 and not shot_lines:
        fails.append('the shots explode with splash momentum but no OWFIRING splash line was logged')
    if push is None:
        fails.append('no OWFIRING push line (momentum after the dummy pulls)')
    else:
        rule, last = float(push['momentum_rule']), float(push['momentum_last'])
        hits, pushes, moved = int(push['momentum_hits']), int(push['pushes']), float(push['moved'])
        if abs(rule - expected['momentum']) > 0.05:
            # The host logs 0 for a gun it holds without a firing block: usually a recipe whose block was never generated.
            hint = (' (the recipe has no firing block: stale local data? run tools/weapon_firing_coverage.py --game <BL2> '
                    '--only-firing)') if rule == 0 and not expected.get('has_firing', True) else ''
            fails.append(f"momentum {rule:g} != 100 x instant_hit_momentum = {expected['momentum']:g}{hint}")
        if pushes != hits:
            fails.append(f'{hits - pushes} of {hits} momentum hits did not change the dummy velocity by momentum / mass')
        if expected['kind'] != 'Rocket' and phase_a_hits:
            if hits < phase_a_hits:
                fails.append(f'{phase_a_hits} direct hits but {hits} carried momentum')
            if not expected['splash'] and abs(last - expected['momentum']) > 0.05:
                fails.append(f"hit momentum {last:g} != the gun's {expected['momentum']:g}")
        if hits and moved <= 0:
            fails.append(f'the dummy did not move along the shots ({moved:.3f} uu)')
    if expected['wide']:
        if wide is None:
            fails.append('no OWFIRING wide line (wide-bullet check)')
        elif wide['line_hits'] != '0':
            fails.append(f"wide check inconclusive: the line beside the dummy hit it ({wide['line_hits']})")
        elif int(wide['box_hits']) < 1:
            fails.append('a TraceExtent box beside the dummy missed it')
    return fails


def finite(line, key):
    """A grenade line field as a finite number, or None when it is missing or not one."""
    try:
        value = float(line[key])
    except (KeyError, TypeError, ValueError):
        return None
    return value if math.isfinite(value) else None


def check_grenade(line):
    """The firing test's one grenade at the dummy, thrown by pressing G (OWFIRING grenade): it was thrown, took one grenade
    from the pool, has positive damage, exploded on the dummy with positive damage and no falloff, its first contact started
    the fuse it then exploded on, the dummy (not only a hit count: an invulnerable target records hits) lost health or died
    during the throw, every splash hit is a hit event with Source Grenade, and with the pool emptied a second press is
    refused."""
    if line is None:
        return ['no OWFIRING grenade line (the grenade check did not run)']
    fails = []
    if line.get('thrown') != '1':
        return ['the grenade was not thrown (no grenade data: tools/grenade_mod.py, or no grenade in the pool)']
    if int(line['ammo_after']) != int(line['ammo_before']) - 1:
        fails.append(f"grenade ammo {line['ammo_before']} -> {line['ammo_after']}, expected one fewer")
    # The mod's loaded damage and what actually landed: a zero-damage explosion still counts as hit events.
    if float(line.get('damage') or 0) <= 0:
        fails.append(f"the grenade's damage is not positive ({line.get('damage')})")
    if float(line.get('splash') or 0) <= 0:
        fails.append(f"the grenade's splash damage is not positive ({line.get('splash')})")
    expected = float(line.get('damage') or 0) * int(line['splash_hits'])
    if abs(float(line.get('splash') or 0) - expected) > 0.01 * max(expected, 1.0):
        fails.append(f"the grenade's splash {line.get('splash')} is not its full damage ({expected:.2f}): it has no falloff")
    contact, fuse, contact_fuse = finite(line, 'contact_age'), finite(line, 'fuse_age'), finite(line, 'contact_fuse')
    if contact_fuse is None or contact_fuse <= 0:
        fails.append(f"the grenade's contact fuse is not a positive number ({line.get('contact_fuse')})")
    elif contact is None or fuse is None or contact < 0 or fuse < 0:
        fails.append(f"no grenade contact or fuse explosion (contact_age={line.get('contact_age')}, fuse_age={line.get('fuse_age')})")
    elif abs(fuse - contact - contact_fuse) > 0.2:
        fails.append(f'the grenade exploded {fuse - contact:.2f} s after its first contact, its fuse is {contact_fuse} s')
    # The dummy's own health: a death counts (it respawns full, so the difference would not show it) only if it was alive.
    dummy, before, after = line.get('dummy', 'the dummy'), finite(line, 'health_before'), finite(line, 'health_after')
    if before is None or after is None or line.get('dummy_died') not in ('0', '1'):
        fails.append(f"no health read for {dummy} (health_before={line.get('health_before')}, health_after={line.get('health_after')}, dummy_died={line.get('dummy_died')})")
    elif before <= 0:
        fails.append(f'{dummy} had no health before the throw ({before})')
    elif line['dummy_died'] != '1' and not after < before:
        fails.append(f'{dummy} lost no health to the grenade ({before} -> {after}) and did not die')
    if line.get('refused_empty') != '1':
        fails.append('a throw with an empty grenade pool was not refused')
    if int(line['splash_hits']) < 1:
        fails.append(f"the grenade splashed nothing (first contact: {line.get('first_contact', 'not logged')})")
    if line['grenade_hits'] != line['splash_hits']:
        fails.append(f"{line['grenade_hits']} grenade hit events for {line['splash_hits']} splash hits")
    if int(line['unknown_sources']):
        fails.append(f"{line['unknown_sources']} grenade hit events with an Unknown source")
    return fails


def check_hit_events(log, expected, throw=None, required=True, gun=None, swap=None):
    """Hit events (OpenWillowHitEvent.h: one per damage application, by kind) against the counts of the same run: a
    direct event per bullet or trace hit (none for a rocket body), a splash event per splash hit, a tick event per status
    tick, a throw event per direct hit of a thrown gun. Every status tick must carry the weapon id of the gun that caused
    the proc (`tick_weapons`), also after the test's weapon swap (`swap`). The fields are required; only a log from
    before the hit-event API (graded with --legacy, `required` False) may lack them."""
    if 'hit_events' not in log:
        return ['no hit_events fields in the result line (a log from before the hit-event API: pass --legacy)'] if required else []
    fails = []
    kinds = {k: int(log[f'hit_{k}']) for k in ('direct', 'splash', 'tick', 'throw')}
    throw_hits = int(throw['target_hits']) if throw is not None else 0
    want = {
        'direct': 0 if expected['kind'] == 'Rocket' else int(log['target_hits']) - throw_hits,
        'splash': int(log['splash_hits']),
        'tick': int(log['ticks']),
        'throw': throw_hits,
    }
    for kind, count in kinds.items():
        if count != want[kind]:
            fails.append(f'{count} {kind} hit events, expected {want[kind]}')
    # Every firing path names its damage source (EOpenWillowHitSource): the field is required (only --legacy logs, from
    # before the hit-event API, skip this whole check).
    if 'unknown_sources' not in log:
        fails.append('no unknown_sources field in the result line')
    elif int(log['unknown_sources']):
        fails.append(f"{log['unknown_sources']} hit events with an Unknown source")
    if int(log['hit_events']) != sum(kinds.values()):
        fails.append(f"hit_events {log['hit_events']} != the sum of the kinds {sum(kinds.values())}")
    if 'tick_weapons' not in log:
        if required:
            fails.append('no tick_weapons field in the result line')
        return fails
    ticks = {}
    for entry in ([] if log['tick_weapons'] == '-' else log['tick_weapons'].split(',')):
        weapon, _, count = entry.rpartition(':')
        ticks[weapon] = ticks.get(weapon, 0) + int(count)
    if sum(ticks.values()) != int(log['ticks']) or sum(ticks.values()) != kinds['tick']:
        fails.append(f"tick_weapons total {sum(ticks.values())} != ticks {log['ticks']} / hit_tick {kinds['tick']}")
    wrong = {weapon: count for weapon, count in ticks.items() if gun is not None and weapon != gun}
    if wrong:
        after = '' if swap is None else f" (after the swap to {swap.get('to')})"
        fails.append(f'status ticks credited to another weapon{after}: {wrong}')
    # Every status gun whose effect ticks (duration and dps above 0) swaps to another gun while an effect runs (the test
    # pulls extra shots until one does), and at least one tick must land after the swap; a missing or failed swap fails.
    # A status without damage ticks (slag: no DOT; a recipe duration that did not resolve) has nothing to attribute: the
    # host logs skipped=no_ticks, accepted only when the recipe agrees.
    ticking = expected['duration'] > 0 and expected['dps'] > 0
    if expected['status'] and required and swap is not None and swap.get('skipped'):
        if ticking:
            fails.append(f"weapon swap skipped ({swap['skipped']}) for a status that ticks in the recipe")
    elif expected['status'] and required and ticking:
        if swap is None:
            fails.append('status gun without an OWFIRING swap line (the source-weapon check did not run)')
        elif 'failed' in swap or not swap.get('to'):
            fails.append(f"weapon swap failed: {swap.get('failed', 'no target gun')}")
        elif int(log['ticks']) <= int(swap.get('ticks_before') or 0):
            fails.append('weapon swap during the status ticks, but no tick landed after it')
    return fails


def check_gun(gun, log, seg, expected, skipped_reason, wall_skipped, throw=None, lifecycle=None, push=None, wide=None,
              momentum=False, splash_lines=None, hit_events=True, swap=None):
    """(list of failure reasons, notes, values for the table) for one gun. With `momentum`, the push and wide lines are
    checked too (check_momentum)."""
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
    pulls = PHASE_A_PULLS + (0 if gun in wall_skipped else WALL_PULLS) + (STATUS_PULLS if expected['status'] else 0) \
        + (WIDE_PULLS if expected['wide'] else 0) + int((swap or {}).get('extra_pulls') or 0)
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
    fails += check_hit_events(log, expected, throw, hit_events, gun, swap)
    if momentum:
        fails += check_momentum(push, wide, expected, phase_a_hits, splash_lines, int(log['splash_hits']), throw)
    if 'timed' in log and log['timed'] != log.get('on_time'):
        fails.append(f"{int(log['timed']) - int(log.get('on_time') or 0)} of {log['timed']} bullets did not arrive after "
                     'distance / speed')
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
    parser.add_argument('--legacy', action='store_true',
                        help='grade a log from before the momentum or hit-event checks: momentum and wide are skipped '
                             'when it has no push lines, the hit events when its result lines have no hit_events')
    args = parser.parse_args(argv)
    item_dirs = [Path(p) for p in args.items] if args.items else DEFAULT_ITEM_DIRS
    text = Path(args.log).read_text(encoding='utf-8', errors='replace')
    extra = {}
    results, segments, skipped, no_wall, run_guns, throws, lifecycles = read_log(text, extra)
    # The push, splash and wide lines are required; only --legacy grades a run from before the momentum checks without them.
    momentum = not (args.legacy and not extra['push'])
    if not momentum:
        print('note: no OWFIRING push lines in this log (a run from before the momentum checks): momentum not checked')
    # The hit-event fields are required; only --legacy grades result lines from before the hit-event API without them.
    hit_events = not (args.legacy and not any('hit_events' in body for body in results.values()))
    if not hit_events:
        print('note: no hit_events fields in this log (a run from before the hit-event API): hit events not checked')
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
                                         throws.get(gun), lifecycles.get(gun), extra['push'].get(gun),
                                         extra['wide'].get(gun), momentum, extra['splash'].get(gun), hit_events,
                                         extra['swap'].get(gun))
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
    # The grenade line is required; only --legacy grades a run from before the grenade check without it.
    grenade_fails = [] if args.legacy and 'grenade' not in extra else check_grenade(extra.get('grenade'))
    for reason in grenade_fails:
        print(f'grenade: FAIL {reason}')
    if 'grenade' in extra and not grenade_fails:
        print('grenade: PASS')
    print(f'{len(guns) - failed} PASS, {failed} FAIL of {len(guns)} guns')
    return 1 if failed or grenade_fails else 0


if __name__ == '__main__':
    sys.exit(main())
