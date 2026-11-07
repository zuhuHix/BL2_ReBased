"""Grade a -owphaselockkillcheck run (tools/test_action_skill_hud.ps1 -KillHeld): the skill and the HUD emblem end when the held target dies.

AI-assisted (Claude). The file holds no game data. Exit 0 only if all pass. Run: python tools/check_phaselock_kill.py <run.log> --shots <folder>
"""
import argparse
from pathlib import Path
import re
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
from check_action_skill_hud import LINE  # noqa: E402

NUMBER = r'([0-9]+(?:\.[0-9]+)?)'
STEPS = (('cast', re.compile(rf'OWPHASELOCKKILL cast t={NUMBER} hit=(\d)')),
         ('held', re.compile(rf'OWPHASELOCKKILL held=(\d) t={NUMBER} remaining={NUMBER}')),
         ('kill', re.compile(rf'OWPHASELOCKKILL kill t={NUMBER} dead=(\d)')),
         ('after', re.compile(rf'OWPHASELOCKKILL after active=(\d) reason=(.*) remaining={NUMBER} since_cast={NUMBER}')),
         ('drain', re.compile(rf'OWPHASELOCKKILL drain remaining={NUMBER} was={NUMBER}')),
         ('result', re.compile(r'OWPHASELOCKKILL result (pass|fail): (.*)')))
SHOT = 'OWPhaselockKill_after.png'
# The HUD state goes out with the frame that ended the skill. The real game's emblem left about 0.05 s after its release (one kill).
EMBLEM_WITHIN = 0.25
SKILL_END = 5.7     # the full timeline: nothing here may wait for it


def read_steps(log):
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
    if SHOT not in shot_names:
        found.append(f'frame {SHOT} missing')
    steps = read_steps(log)
    expected = [name for name, _ in STEPS]
    if [name for name, _ in steps] != expected:
        failed = next((g[1] for name, g in steps if name == 'result' and g[0] == 'fail'), None)
        if failed:
            found.append(f'the check failed: {failed.strip()}')
        found.append(f'steps must be exactly {expected}, once each, in order; got {[name for name, _ in steps]}')
        return found
    values = dict(steps)
    if values['result'][0] != 'pass':
        found.append(f'the check failed: {values["result"][1].strip()}')
    cast_t, hit = float(values['cast'][0]), values['cast'][1]
    kill_t = float(values['kill'][0])
    if hit != '1':
        found.append('the cast did not hit')
    if values['held'][0] != '1':
        found.append('the target was not held before the kill')
    if values['kill'][1] != '1':
        found.append('the target was not dead after the hit')
    if values['after'][0] != '0':
        found.append('the skill was still active after the kill')
    if 'held target died' not in values['after'][1]:
        found.append(f'the skill ended for another reason: {values["after"][1]}')
    remaining_after, since_cast = float(values['after'][2]), float(values['after'][3])
    if since_cast >= SKILL_END:
        found.append(f'the after step came {since_cast} s after the cast, past the timeline')
    if float(values['drain'][0]) >= remaining_after or not float(values['drain'][1]) == remaining_after:
        found.append(f'the cooldown did not drain: {values["drain"]}')
    # The HUD: the emblem state as the page was given it, on the same clock.
    rows = [(float(m[1]), float(m[3])) for m in LINE.finditer(log)]
    before = [active for t, active in rows if cast_t <= t < kill_t]
    if not any(active >= 0 for active in before):
        found.append('no HUD state showed the emblem running before the kill')
    gone = next((t for t, active in rows if t >= kill_t and active < 0), None)
    if gone is None:
        found.append('no HUD state showed the emblem gone after the kill')
    elif gone - kill_t > EMBLEM_WITHIN:
        found.append(f'the emblem left {gone - kill_t:.3f} s after the kill, more than {EMBLEM_WITHIN} s')
    if any(active >= 0 for t, active in rows if t > kill_t + EMBLEM_WITHIN):
        found.append('the emblem came back after the kill')
    return found


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('log')
    parser.add_argument('--shots', required=True)
    args = parser.parse_args()
    log = Path(args.log).read_text(encoding='utf-8', errors='replace')
    names = {re.sub(r'\d*\.png$', '.png', p.name) for p in Path(args.shots).glob('OWPhaselockKill_*.png')}
    found = problems(log, names)
    for line in log.splitlines():
        if 'OWPHASELOCKKILL' in line:
            print(line.strip())
    for problem in found:
        print('PROBLEM:', problem)
    print('phaselock kill check', 'FAILED' if found else 'passed')
    return 1 if found else 0


if __name__ == '__main__':
    sys.exit(main())
