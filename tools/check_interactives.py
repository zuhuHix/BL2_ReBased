"""Grade one -owinteractives run of the in-engine lootables suite (tools/test_interactives.ps1).

Reads the host's log lines (host/ue5/OpenWillow/Source/OpenWillow/OpenWillowInteractive.cpp and
OpenWillowInteractivePickup.cpp) and the automation log of OpenWillow.Interactive.Loot. Prints one line per check:
    CHECK <name> PASS|FAIL|MISSING <detail>
then SUMMARY passed=N failed=M missing=K. Exit 0 when nothing FAILs, 1 on any FAIL, 2 on bad arguments.

MISSING means the log does not carry the value a check needs (for example no pickup was collected). It is reported,
not counted as a failure.

Expected values come from the interactives.json file the run was given (the same file the host reads):
- lootables spawned = the supported objects in the file (the host skips "supported": false ones);
- a cash drop's value must lie in the range of the cash formula at its level (the constants are read from the file's
  cash item), and its level must lie in the object's game stage;
- a pickup line with ok=1 must have moved its counter by the rule of its kind (cash and eridium by the amount, ammo up to
  the pool's cap, health up to the max health by the fraction).
The host's rules are UNVERIFIED in game beyond what its own comments say; this script only compares the logged numbers.
"""
import argparse
import json
import math
import re
import sys
from pathlib import Path

SPAWNED = re.compile(r'OpenWillow interactives: (\d+) lootable\(s\) spawned, (\d+) unsupported')
OPEN = re.compile(r'OWLOOT open id=(\S+) config=(\S+) drops=(\d+)')
EMPTY = re.compile(r'OWLOOT empty id=(\S+) reason=(.*)$', re.M)
DROP = re.compile(r'OWLOOT drop id=(\S+) kind=(\S+) balance=(\S+) level=(\d+) value=(-?\d+)')
PICKUP = re.compile(r'OWPICKUP kind=(\S+) amount=(-?[\d.]+) before=(-?[\d.]+) after=(-?[\d.]+) cap=(-?[\d.]+) '
                    r'ok=([01]) reason=(.*)$', re.M)
LID_PHASES = ('closed', 'mid', 'opened')
DROP_KIND = re.compile(r'OWLOOT drop id=(\S+) kind=(\w+)')
PICKUP_KINDS = ('cash', 'ammo', 'eridium', 'health')
LID_QUEUE = re.compile(r'OWLOOTLID queue=(\d+)')
LID_SHOT = re.compile(r'OWLOOTLID shot balance=(\S+) phase=(\w+)')
LID_BLOCKED = re.compile(r'OWLOOTLID view balance=(\S+) clear=0')
TEST_OPENED = re.compile(r'OWINTERACTIVETEST opened (\d+) lootable')
TEST_DONE = re.compile(r'OWINTERACTIVETEST done spawned=(\d+) opened=(\d+)')
TEST_FAIL = re.compile(r'OWINTERACTIVETEST FAIL (.*)$', re.M)
AUTOMATION_TEST = 'OpenWillow.Interactive.Loot'
INT32_MAX = 2147483647
TOLERANCE = 0.02  # the host prints floats with two decimals


class Results:
    def __init__(self):
        self.rows = []

    def add(self, name, status, detail):
        self.rows.append((name, status, detail))

    def print(self):
        passed = sum(1 for _, s, _ in self.rows if s == 'PASS')
        failed = sum(1 for _, s, _ in self.rows if s == 'FAIL')
        missing = sum(1 for _, s, _ in self.rows if s == 'MISSING')
        skipped = sum(1 for _, s, _ in self.rows if s == 'SKIPPED')
        for name, status, detail in self.rows:
            print(f'CHECK {name} {status} {detail}')
        print(f'SUMMARY passed={passed} failed={failed} missing={missing} skipped={skipped}')
        # Required evidence that is absent fails the run. SKIPPED is only for what a run may legitimately not have.
        return failed + missing


def supported_objects(doc):
    return [o for o in doc.get('objects', []) if o.get('supported') is True]


def cash_constants(doc):
    """The constants of the cash formula, from the first item that carries a cash block (None when there is none)."""
    for item in (doc.get('items') or {}).values():
        cash = (item.get('pickup') or {}).get('cash')
        if cash:
            constants = cash.get('constants') or {}
            grade_range = cash.get('grade_range') or [0, 0]
            return {'multiplier': float(constants['Multiplier']), 'level': float(constants['Level']),
                    'base': float(cash['base_scale']), 'per_grade': float(cash['per_grade_scale']),
                    'grade_max': int(grade_range[1])}
    return None


def cash_range(constants, level):
    """Lowest and highest whole-dollar cash value at this level: calc(L) x (base + per_grade x grade), rounded down."""
    calc = constants['multiplier'] * constants['level'] ** level
    low = math.floor(calc * constants['base'])
    high = math.floor(calc * (constants['base'] + constants['per_grade'] * constants['grade_max']))
    return low, high


def grade_spawned(log_text, doc, out):
    expected = len(supported_objects(doc))
    found = SPAWNED.findall(log_text)
    if not found:
        out.add('lootables_spawned', 'FAIL', 'no "lootable(s) spawned" line: the subsystem did not run (-owinteractives?)')
        return None
    spawned, unsupported = int(found[-1][0]), int(found[-1][1])
    if spawned == expected:
        out.add('lootables_spawned', 'PASS', f'{spawned} of {expected} supported objects ({unsupported} unsupported not spawned)')
    else:
        out.add('lootables_spawned', 'FAIL', f'spawned {spawned}, the file has {expected} supported objects')
    return spawned


def grade_opens(log_text, objects_by_id, out):
    opens = OPEN.findall(log_text)
    if not opens:
        out.add('opened_lootables', 'MISSING', 'no "OWLOOT open" line (the test did not open a lootable)')
        return
    unknown = [oid for oid, _, _ in opens if oid not in objects_by_id]
    if unknown:
        out.add('opened_lootables', 'FAIL', f'{len(unknown)} open(s) of an id that is not a supported object, first: {unknown[0]}')
    elif len({oid for oid, _, _ in opens}) != len(opens):
        out.add('opened_lootables', 'FAIL', f'a lootable opened more than once: {len(opens)} opens of {len({oid for oid, _, _ in opens})} ids')
    else:
        out.add('opened_lootables', 'PASS', f'{len(opens)} lootable(s) opened, all supported objects')
    # The sequence aims at three different balances (fewer only when the file has fewer).
    wanted = min(3, len({o.get('balance') for o in objects_by_id.values()}))
    reported = TEST_OPENED.search(log_text)
    balances = {objects_by_id[oid].get('balance') for oid, _, _ in opens if oid in objects_by_id}
    if reported is None:
        out.add('opened_balances', 'MISSING', 'no "OWINTERACTIVETEST opened" line')
    elif int(reported.group(1)) != wanted:
        out.add('opened_balances', 'FAIL', f'the sequence reports {reported.group(1)} balance(s) opened, it aims at {wanted}')
    elif len(balances) < wanted:
        out.add('opened_balances', 'FAIL', f'the opens cover {len(balances)} balance(s), expected at least {wanted}')
    else:
        out.add('opened_balances', 'PASS', f'{len(balances)} balances opened; the sequence reports {reported.group(1)}')
    empty_reasons = {oid: reason for oid, reason in EMPTY.findall(log_text)}
    bad = []
    for oid, config, drops in opens:
        if int(drops) == 0 and oid not in empty_reasons:
            bad.append(oid)
    if bad:
        out.add('open_drops', 'FAIL', f'{len(bad)} open(s) with no drop and no "OWLOOT empty" reason, first: {bad[0]}')
    else:
        empties = f'; {len(empty_reasons)} with a logged empty roll' if empty_reasons else ''
        out.add('open_drops', 'PASS', f'every open has a drop or a logged empty roll{empties}')


def grade_drops(log_text, objects_by_id, constants, out):
    drops = DROP.findall(log_text)
    cash = [d for d in drops if d[1] == 'cash']
    if not cash:
        # Loot is rolled: a run may legitimately have no cash.
        out.add('cash_drops', 'SKIPPED', f'no cash drop among {len(drops)} drop line(s)')
        return
    if constants is None:
        out.add('cash_drops', 'FAIL', 'the file has no cash item: the formula range cannot be checked')
        return
    problems = []
    for oid, _, balance, level, value in cash:
        stage = (objects_by_id.get(oid) or {}).get('game_stage') or {}
        level, value = int(level), int(value)
        if not (stage.get('min', 0) <= level <= stage.get('max', -1)):
            problems.append(f'{oid} level {level} outside the stage {stage.get("min")}..{stage.get("max")}')
            continue
        low, high = cash_range(constants, level)
        if not (low <= value <= high):
            problems.append(f'{oid} value {value} outside {low}..{high} at level {level}')
    if problems:
        out.add('cash_drops', 'FAIL', f'{len(problems)} of {len(cash)} cash drop(s); first: {problems[0]}')
    else:
        out.add('cash_drops', 'PASS', f'{len(cash)} cash drop(s) inside the formula range at their level and stage')


def check_pickup(kind, amount, before, after, cap, ok):
    """None when the logged numbers follow the rule of the kind, else the reason they do not."""
    if not ok:
        return None if abs(after - before) < TOLERANCE else f'refused but moved {before} -> {after}'
    if kind in ('cash', 'eridium'):
        expected = min(before + amount, INT32_MAX)   # money and eridium saturate at int32
        return None if abs(after - expected) < TOLERANCE else f'{kind} {before} -> {after}, expected {expected:g}'
    if kind == 'ammo':
        expected = min(cap, before + amount)
        return None if abs(after - expected) < TOLERANCE else f'ammo {before} -> {after}, expected {expected:g} (cap {cap:g})'
    if kind == 'health':
        expected = min(cap, before + amount * cap)
        return None if abs(after - expected) < TOLERANCE else f'health {before} -> {after}, expected {expected:.2f} (max {cap:g})'
    return f'unknown kind {kind}'


def grade_pickups(log_text, out):
    rows = PICKUP.findall(log_text)
    if not rows:
        # Required when a pickup dropped; a roll may legitimately drop only guns and gear.
        dropped = [kind for _, kind in DROP_KIND.findall(log_text) if kind in PICKUP_KINDS]
        if dropped:
            out.add('pickups', 'MISSING', f'{len(dropped)} pickup drop(s) but no "OWPICKUP" line (none was collected)')
        else:
            out.add('pickups', 'SKIPPED', 'no cash, ammo, eridium or health pickup dropped in this run')
        return
    problems = []
    for kind, amount, before, after, cap, ok, reason in rows:
        problem = check_pickup(kind, float(amount), float(before), float(after), float(cap), ok == '1')
        if problem:
            problems.append(f'{kind}: {problem}')
    accepted = sum(1 for r in rows if r[5] == '1')
    refused = len(rows) - accepted
    if problems:
        out.add('pickups', 'FAIL', f'{len(problems)} of {len(rows)} pickup line(s) break their rule; first: {problems[0]}')
    else:
        out.add('pickups', 'PASS', f'{len(rows)} pickup line(s) follow their rule ({accepted} taken, {refused} refused)')


def grade_lid_capture(log_text, out, imported_clips=None):
    """The lid frames were taken. PASS means only that: whether a lid opens the right way is judged on the frames."""
    shots = LID_SHOT.findall(log_text)
    queue = LID_QUEUE.search(log_text)
    if queue is None:
        out.add('lid_capture', 'MISSING', 'no "OWLOOTLID queue" line')
        return
    if int(queue.group(1)) == 0:
        if imported_clips:
            out.add('lid_capture', 'FAIL', f'the host found no open clip, but the import report lists {imported_clips}: asset paths disagree?')
        else:
            out.add('lid_capture', 'SKIPPED', 'no balance has an imported open clip (tools/import_lootables.ps1)')
        return
    phases = {}
    for balance, phase in shots:
        phases.setdefault(balance, []).append(phase)
    bad = [f'{balance}: {sorted(got)}' for balance, got in sorted(phases.items()) if sorted(got) != sorted(LID_PHASES)]
    if len(phases) != int(queue.group(1)):
        out.add('lid_capture', 'FAIL', f'frames of {len(phases)} lootable types, the queue had {queue.group(1)}')
    elif bad:
        out.add('lid_capture', 'FAIL', f'{len(bad)} type(s) without exactly one each of {LID_PHASES}; first: {bad[0]}')
    else:
        blocked = sorted(LID_BLOCKED.findall(log_text))
        note = f'; no clear view of {", ".join(blocked)}' if blocked else ''
        out.add('lid_capture', 'PASS', f'{len(shots)} frames of {len(phases)} lootable types{note}; lid direction UNVERIFIED until judged')


def grade_test_sequence(log_text, supported, out):
    if TEST_FAIL.search(log_text):
        out.add('test_sequence', 'FAIL', TEST_FAIL.search(log_text).group(1).strip())
        return
    done = TEST_DONE.findall(log_text)
    if not done:
        out.add('test_sequence', 'MISSING', 'no "OWINTERACTIVETEST done" line (the -owinteractivetest sequence did not finish)')
    else:
        spawned, opened = (int(value) for value in done[-1])
        lines = len(OPEN.findall(log_text))
        expected = len(supported)
        if spawned != expected:
            out.add('test_sequence', 'FAIL', f'the sequence ended with {spawned} spawned, the file has {expected} supported')
        elif opened != lines:
            out.add('test_sequence', 'FAIL', f'the sequence counts {opened} opened, the log has {lines} "OWLOOT open" lines')
        else:
            out.add('test_sequence', 'PASS', f'sequence finished: {opened} opened of {spawned} spawned')


def automation_status(text, name):
    """The Result={...} of one automation test from its log line, or None when the line is absent."""
    pattern = (rf'Result=\{{(\w+)\}}[^\n]*{re.escape(name)}(?![\w.])|'
               rf'{re.escape(name)}(?![\w.])[^\n]*Result=\{{(\w+)\}}')
    found = re.search(pattern, text)
    return (found.group(1) or found.group(2)) if found else None


def grade_automation(automation_text, out, skip=False):
    if skip:
        out.add('automation', 'SKIPPED', 'the run was told to skip the automation test (--skip-automation)')
        return
    if automation_text is None:
        out.add('automation', 'MISSING', 'no automation log given')
        return
    status = automation_status(automation_text, AUTOMATION_TEST)
    if status is None:
        out.add('automation', 'FAIL', f'no result line for {AUTOMATION_TEST} (the test did not run)')
    else:
        # UE 5.8 prints Result={Success}; older builds printed Passed.
        out.add('automation', 'PASS' if status in ('Success', 'Passed') else 'FAIL', f'{AUTOMATION_TEST} Result={status}')


def imported_open_clips(report):
    """How many Open clips an import_report.json says were imported; None without a report."""
    if report is None:
        return None
    return sum(1 for row in report.get('assets') or [] if row.get('kind') == 'anim_sequence' and row.get('imported')
               and str(row.get('asset', '')).endswith('_Open'))


def grade(log_text, automation_text, doc, skip_automation=False, import_report=None):
    """Return a Results object with one row per check. doc is the parsed interactives.json."""
    out = Results()
    objects = supported_objects(doc)
    objects_by_id = {o['id']: o for o in objects}
    grade_spawned(log_text, doc, out)
    grade_opens(log_text, objects_by_id, out)
    grade_drops(log_text, objects_by_id, cash_constants(doc), out)
    grade_pickups(log_text, out)
    grade_lid_capture(log_text, out, imported_open_clips(import_report))
    grade_test_sequence(log_text, objects, out)
    grade_automation(automation_text, out, skip_automation)
    return out


def main(argv):
    parser = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    parser.add_argument('--log', required=True, help='the game log of the -owinteractivetest run')
    parser.add_argument('--interactives', required=True, help='the interactives.json the run was given')
    parser.add_argument('--automation-log', help='the log of the UE automation run (OpenWillow.Interactive.Loot)')
    parser.add_argument('--import-report', help="tools/import_lootables.ps1's import_report.json, when there is one")
    parser.add_argument('--skip-automation', action='store_true', help='report the automation test as skipped, not missing')
    args = parser.parse_args(argv)
    try:
        log_text = Path(args.log).read_text(encoding='utf-8', errors='replace')
        doc = json.loads(Path(args.interactives).read_text(encoding='utf-8'))
        report = json.loads(Path(args.import_report).read_text(encoding='utf-8-sig')) if args.import_report else None
        automation_text = (Path(args.automation_log).read_text(encoding='utf-8', errors='replace')
                           if args.automation_log else None)
    except (OSError, json.JSONDecodeError) as error:
        print(f'cannot read the inputs: {error}', file=sys.stderr)
        return 2
    return 1 if grade(log_text, automation_text, doc, args.skip_automation, report).print() else 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
