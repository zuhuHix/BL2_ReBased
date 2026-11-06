"""Grade one tools/test_mission_page.ps1 run from its log: the exact expected rows, each once and PASS, and a matching summary.

A summary of "failed=0" alone would pass a run that lost a row or renamed one. A pass says the host did this, not that it matches the
real game (docs/verification/MISSION_SCREEN.md).

Usage: python tools/check_mission_page.py <log>. Exit 0 pass, 1 fail.
"""
from pathlib import Path
import re
import sys

EXPECTED = ('page_ready', 'e_refused_under_a_status_page', 'control_gun_fires_before_the_page', 'first_e_opens_the_page', 'offer_drawn', 'held_trigger_does_not_fire_under_the_page',
            'offer_e_does_nothing', 'offer_escape_closes', 'gun_fires_again_after_the_close', 'e_opens_the_offer_again', 'offer_enter_accepts_and_closes',
            'mission_ready_to_turn_in', 'e_opens_the_turn_in', 'turn_in_drawn', 'turn_in_e_does_nothing', 'enter_shows_the_reward_page', 'reward_waits_for_enter',
            'enter_takes_the_reward_and_closes', 'first_person_back_after_the_reward', 'e_afterwards_opens_nothing')
ROW = re.compile(r'OWMISSIONPAGETEST (\S+) (PASS|FAIL)\b(.*)')
SUMMARY = re.compile(r'OWMISSIONPAGETEST SUMMARY passed=(\d+) failed=(\d+)[ \t\r]*$', re.M)


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
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    rows, problems = grade(Path(sys.argv[1]).read_text(encoding='utf-8', errors='replace'))
    for name, result, detail in rows:
        print(f'{result:<5} {name:<42} {detail}')
    for problem in problems:
        print(f'PROBLEM {problem}')
    print(f'OWMISSIONPAGETEST SUMMARY {"PASS" if not problems else "FAIL"} rows={len(rows)} expected={len(EXPECTED)} problems={len(problems)}')
    return 1 if problems else 0


if __name__ == '__main__':
    sys.exit(main())
