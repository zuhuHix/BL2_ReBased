"""Grade one tools/test_controls.ps1 run from its log: the exact expected rows, each once and PASS, and a matching summary.

A summary of "failed=0" alone would pass a run that lost a row or renamed one. A pass says the host did this, not that it matches the
real game (docs/verification/CONTROLS.md).

Usage: python tools/check_controls.py [--melee-clip] <log>. Exit 0 pass, 1 fail.
--melee-clip also requires the imported first-person melee clip, for a run after the import (docs/verification/CONTROLS.md).
"""
from pathlib import Path
import re
import sys

EXPECTED = ('start_closed', 'm_opens_map', 'map_closed', 'l_opens_missions', 'melee_damage_rule', 'melee_cone_rule', 'melee_swing_rule', 'use_prompt_rule',
            'v_not_hit_before_the_delay', 'v_hits_target_ahead', 'v_misses_target_behind', 'v_misses_target_out_of_reach',
            'page_does_not_cancel_swing', 'v_refused_in_grenade_lockout', 'g_refused_during_swing', 'holster_refused_during_swing', 'v_stops_reload_before_refill',
            'v_keeps_refill_after_it', 'burst_ends_on_swing', 'prompt_nothing_in_open_air', 'prompt_marcus_over_world_target', 'prompt_aimed_pickup',
            'e_takes_the_named_pickup', 'down_cancels_swing', 'prompt_nothing_while_down', 'm_refused_while_down', 'l_refused_while_down', 'route_refused_while_down', 'closed_at_end')
ROW = re.compile(r'OWCONTROLS (\S+) (PASS|FAIL)\b(.*)')
SUMMARY = re.compile(r'OWCONTROLS SUMMARY passed=(\d+) failed=(\d+)[ \t\r]*$', re.M)
MELEE_PRESS = re.compile(r'OWMELEE press t=\S+ duration=(\S+) clip=(\d+)')
MELEE_CLIP_MISSING = re.compile(r'arms animation not found:[^\r\n]*Melee')
MELEE_CLIP_SECONDS = 0.70   # the game holds the weapons this long, read live (OpenWillowMelee::NoClipDuration)


def grade_melee_clip(text):
    """Problems with the imported clip: not found, or a swing without it, or of another length. The stand-in duration is also 0.70, so clip=1 is what tells them apart."""
    problems = []
    if MELEE_CLIP_MISSING.search(text):
        problems.append('the melee clip was not found')
    presses = MELEE_PRESS.findall(text)
    if not presses:
        problems.append('no melee press logged')
    for duration, clip in presses:
        if clip != '1':
            problems.append(f'a swing ran without the clip: duration={duration} clip={clip}')
        elif abs(float(duration) - MELEE_CLIP_SECONDS) > 0.02:
            problems.append(f'the clip runs {duration} s, the game held the weapons {MELEE_CLIP_SECONDS:.2f} s')
    return problems


def grade(text):
    """Returns (rows, problems). rows: (name, result, detail) as logged, in order."""
    rows = [(m.group(1), m.group(2), m.group(3).strip()) for m in ROW.finditer(text)]
    problems = []
    names = [name for name, _, _ in rows]
    for name in EXPECTED:
        count = names.count(name)
        if count != 1:
            problems.append(f'row {name} appears {count} times, expected once')
    for name in sorted(set(names) - set(EXPECTED)):
        problems.append(f'unexpected row {name}')
    for name, result, detail in rows:
        if result != 'PASS':
            problems.append(f'row {name} {result}: {detail}')
    summaries = SUMMARY.findall(text)
    if len(summaries) != 1:
        problems.append(f'{len(summaries)} summaries, expected one')
    elif (int(summaries[0][0]), int(summaries[0][1])) != (len(EXPECTED), 0):
        problems.append(f'summary passed={summaries[0][0]} failed={summaries[0][1]}, expected passed={len(EXPECTED)} failed=0')
    return rows, problems


def main():
    args = sys.argv[1:]
    melee_clip = '--melee-clip' in args
    if melee_clip:
        args.remove('--melee-clip')
    if len(args) != 1:
        sys.exit(__doc__)
    text = Path(args[0]).read_text(encoding='utf-8', errors='replace')
    rows, problems = grade(text)
    if melee_clip:
        problems += grade_melee_clip(text)
    for name, result, detail in rows:
        print(f'{result:<5} {name:<24} {detail}')
    for problem in problems:
        print(f'PROBLEM {problem}')
    print(f'OWCONTROLS SUMMARY {"PASS" if not problems else "FAIL"} rows={len(rows)} expected={len(EXPECTED)} problems={len(problems)}')
    return 1 if problems else 0


if __name__ == '__main__':
    sys.exit(main())
