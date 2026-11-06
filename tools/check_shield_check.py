"""Grade a -owshieldcheck run (tools/run_hud_shots.ps1 -StandInShield): the log's steps, in order and with their numbers, and the frames.

AI-assisted (Claude). The file holds no game data. Exit 0 only if all pass. Run: python tools/check_shield_check.py <run.log> --shots <folder>
"""
import argparse
from pathlib import Path
import re
import sys

SHOTS = ('OWShield_full.png', 'OWShield_hit.png', 'OWShield_recharge.png')
RULE_FILE = 'local/slice/stand_in_shield.json'
NUMBER = r'(-?[0-9]+(?:\.[0-9]+)?)'
# Each step, once, in this order. Numbers print to one decimal place, so comparisons allow for the rounding.
STEPS = (('equip', re.compile(r'OWSHIELDCHECK equipped a stat-less shield at level \d+')),
         ('rule', re.compile(r'OpenWillow Maya shield: stand-in rule read from (.+?)\s*$')),
         ('capacity', re.compile(rf'OpenWillow Maya shield Shield Check Stand-in: capacity {NUMBER} \(stand-in rule, UNVERIFIED\)')),
         ('full', re.compile(rf'OWSHIELDCHECK full shield {NUMBER} of {NUMBER}, health {NUMBER}')),
         ('damage', re.compile(rf'OpenWillow Maya took {NUMBER} damage \(shield {NUMBER}, health {NUMBER}\), shield {NUMBER}/{NUMBER} health {NUMBER}/{NUMBER}')),
         ('hit', re.compile(rf'OWSHIELDCHECK hit shield {NUMBER} of {NUMBER}, health {NUMBER}')),
         ('recharge', re.compile(rf'OWSHIELDCHECK recharge shield {NUMBER} of {NUMBER}, health {NUMBER}')),
         ('result', re.compile(r'OWSHIELDCHECK result (pass|fail): (.*)')))
SLACK = 0.11


def close(a, b):
    return abs(a - b) <= SLACK


def read_steps(log):
    """[(name, groups)] of the step lines, in log order."""
    found = []
    for line in log.splitlines():
        for name, pattern in STEPS:
            match = pattern.search(line)
            if match:
                found.append((name, match.groups()))
                break
    return found


def problems(log, shot_names):
    """What is wrong with this run; empty when it passes."""
    found = []
    if re.search(r'OpenWillow Maya shield: .*stand_in_shield\.json.*(ignored|protects nothing)', log):
        found.append('a warning says the rule file is missing or bad')
    for name in SHOTS:
        if name not in shot_names:
            found.append(f'frame {name} missing')
    steps = read_steps(log)
    order = [name for name, _ in steps]
    expected = [name for name, _ in STEPS]
    if order != expected:
        result = next((g for name, g in steps if name == 'result' and g[0] == 'fail'), None)
        if result:
            found.append(f'the check failed: {result[1].strip()}')
        found.append(f'steps must be exactly {expected}, once each, in order; got {order}')
        return found
    values = dict(steps)
    if values['result'][0] != 'pass':
        found.append(f'the check failed: {values["result"][1].strip()}')
    if not values['rule'][0].replace('\\', '/').endswith('/' + RULE_FILE):
        found.append(f'the rule was read from {values["rule"][0]}, not .../{RULE_FILE}')
    capacity = float(values['capacity'][0])
    if capacity <= 0:
        found.append(f'capacity {capacity} is not positive')
    full, full_of, full_health = map(float, values['full'])
    took, to_shield, to_health, after, after_of, damage_health, _max_health = map(float, values['damage'])
    hit, hit_of, hit_health = map(float, values['hit'])
    recharge, recharge_of, recharge_health = map(float, values['recharge'])
    if not all(close(c, capacity) for c in (full_of, after_of, hit_of, recharge_of)):
        found.append(f'capacity changes between steps: {capacity}, {full_of}, {after_of}, {hit_of}, {recharge_of}')
    if not close(full, capacity):
        found.append(f'full shield {full} is not the capacity {capacity}')
    if took <= 0 or to_health != 0 or not close(to_shield, took):
        found.append(f'the hit took {took}: {to_shield} from the shield, {to_health} from health')
    if not close(hit, after) or not close(hit, full - took):
        found.append(f'hit shield {hit} does not match the damage line ({after} after, {took} from {full})')
    if hit >= capacity:
        found.append(f'hit shield {hit} is not below the capacity {capacity}')
    if recharge <= hit or recharge > capacity + SLACK:
        found.append(f'recharge shield {recharge} is not above the hit {hit} and at most {capacity}')
    if not all(close(h, full_health) for h in (damage_health, hit_health, recharge_health)):
        found.append(f'health changed: {full_health}, {damage_health}, {hit_health}, {recharge_health}')
    return found


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('log')
    parser.add_argument('--shots', required=True)
    args = parser.parse_args()
    log = Path(args.log).read_text(encoding='utf-8', errors='replace')
    # UE adds a suffix to a screenshot's name only when one exists; the copied frames keep theirs.
    names = {re.sub(r'\d*\.png$', '.png', p.name) for p in Path(args.shots).glob('OWShield_*.png')}
    found = problems(log, names)
    for line in log.splitlines():
        if any(pattern.search(line) for _, pattern in STEPS):
            print(line.strip())
    for problem in found:
        print('PROBLEM:', problem)
    print('shield check', 'FAILED' if found else 'passed')
    return 1 if found else 0


if __name__ == '__main__':
    sys.exit(main())
