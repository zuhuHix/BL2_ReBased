"""Grade a -owshieldcheck run (tools/run_hud_shots.ps1 -StandInShield): the log and the three frames. Exit 0 only if all pass.

AI-assisted (Claude). The file holds no game data. Run: python tools/check_shield_check.py <run.log> --shots <folder>
"""
import argparse
from pathlib import Path
import re
import sys

SHOTS = ('OWShield_full.png', 'OWShield_hit.png', 'OWShield_recharge.png')


def problems(log, shot_names):
    """What is wrong with this run; empty when it passes."""
    found = []
    path = re.search(r'stand-in rule read from (\S.*stand_in_shield\.json)', log)
    if not path:
        found.append('no "stand-in rule read from ...stand_in_shield.json" line')
    if re.search(r'OpenWillow Maya shield: .*stand_in_shield\.json.*(ignored|protects nothing)', log):
        found.append('a warning says the rule file is missing or bad')
    if not re.search(r'OpenWillow Maya shield Shield Check Stand-in: capacity [1-9][0-9.]* \(stand-in rule, UNVERIFIED\)', log):
        found.append('no capacity line from the stand-in rule for the check shield')
    for step in ('full', 'hit', 'recharge'):
        if f'OWSHIELDCHECK {step} shield' not in log:
            found.append(f'no {step} step')
    if 'OWSHIELDCHECK result pass' not in log:
        result = re.search(r'OWSHIELDCHECK result fail: (.*)', log)
        found.append(f'the check failed: {result.group(1).strip()}' if result else 'no result line')
    for name in SHOTS:
        if name not in shot_names:
            found.append(f'frame {name} missing')
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
    for line in re.findall(r'.*(?:stand-in rule read from|OpenWillow Maya shield Shield Check|OWSHIELDCHECK).*', log):
        print(line.strip())
    for problem in found:
        print('PROBLEM:', problem)
    print('shield check', 'FAILED' if found else 'passed')
    return 1 if found else 0


if __name__ == '__main__':
    sys.exit(main())
