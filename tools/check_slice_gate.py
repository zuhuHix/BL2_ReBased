"""Check a slice-gate run (the host's `-owslicegate` run) from its Unreal log.

The host logs `OWSLICEGATE start ...`, then `OWSLICEGATE link=<name> result=<PASS|FAIL|NOT_AVAILABLE> fixtures=[<a; b>]
detail=<text>` for each link it reaches, in run order, and `OWSLICEGATE SUMMARY pass=<n> fail=<n> na=<n>` when the run
finishes. `fixtures=[...]` names the test fixtures the link relied on (empty or absent: none). This checker lists the
links in order, marks every link the host never logged as FAIL ("not reached"), and checks the run's structure: one
start line, then each link at most once and in the expected order, then one SUMMARY line that agrees with the link lines.
Every link line must carry the fixtures field.

Exit codes: 0 when no link is FAIL, the structure holds and the SUMMARY is there and agrees; 1 otherwise; 2 when the log
has no `OWSLICEGATE start` line. NOT_AVAILABLE means a missing capability or precondition and does not fail the run, so
it is overruled to FAIL where the capability is there: `enemy_attacks_maya` in a run with the range
(`range_command=1` on the start line), `loot_drop_on_kill` with the gun roll forced, and a card link whose ids show a
pickup attempt was made. A NOT_AVAILABLE whose detail starts "test setup gap" is this test's own setup or local data, not
something the host lacks: it is owned by slice-gate and counted on its own line.
"""
import argparse
import json
import re
import sys
from collections import Counter
from pathlib import Path

# The expected links in run order: (name, owning lane, used when the link is not PASS).
LINKS = [
    ('spawn_maya', 'maya'),
    ('reach_fight_area', 'range'),
    ('kill_enemy', 'range'),
    ('enemy_attacks_maya', 'range'),
    ('loot_drop_on_kill', 'range'),
    ('pick_up_gun', 'range'),
    ('pick_up_aimed_not_nearest', 'inventory'),
    ('equip_gun', 'maya'),
    ('phaselock_enemy', 'maya'),
    ('mission_accept', 'slice-gate'),
    ('mission_objective', 'slice-gate'),
    ('mission_turn_in', 'slice-gate'),
    ('go_down', 'maya'),
    ('bleed_out_respawn', 'maya'),
]
NAMES = [name for name, _ in LINKS]
RESULTS = ('PASS', 'FAIL', 'NOT_AVAILABLE')
NOT_REACHED = 'not reached: the run stopped before this link'
NO_SUMMARY = 'no SUMMARY line: the run did not finish (crash or timeout)'
BOM = '﻿'

START = re.compile(r'OWSLICEGATE start\b')
RANGE_START = re.compile(r'OWSLICEGATE start\b.*\brange_command=1\b')
LINK = re.compile(r'OWSLICEGATE link=(?P<name>\S+) result=(?P<result>PASS|FAIL|NOT_AVAILABLE)'
                  r'(?: fixtures=\[(?P<fixtures>[^\]]*)\])? detail=(?P<detail>.*)$')
LINK_ANY = re.compile(r'OWSLICEGATE link=')
SUMMARY = re.compile(r'OWSLICEGATE SUMMARY pass=(?P<pass>\d+) fail=(?P<fail>\d+) na=(?P<na>\d+)')


def read_log(path):
    """The log's lines; a UTF-8 byte order mark at the start of the file is dropped."""
    text = Path(path).read_text(encoding='utf-8', errors='replace')
    return text.lstrip(BOM).splitlines()


def parse_log(lines):
    """Return (starts, logged, summary, errors).

    starts is the number of start lines; logged maps each link name to (result, detail, fixtures) in log order; summary
    is the (pass, fail, na) tuple of the SUMMARY line, or None. errors lists every break of the run's structure: a second
    start or SUMMARY line, a link line outside start..SUMMARY, an unknown, repeated or out-of-order link, an unreadable
    link line. Any error fails the run.
    """
    starts = 0
    logged = {}
    summary = None
    errors = []
    for n, line in enumerate(lines, 1):
        if START.search(line):
            starts += 1
            if starts > 1:
                errors.append(f'line {n}: a second start line (one run per log)')
            continue
        match = SUMMARY.search(line)
        if match:
            if summary is not None:
                errors.append(f'line {n}: a second SUMMARY line')
            if starts == 0:
                errors.append(f'line {n}: SUMMARY before the start line')
            summary = (int(match['pass']), int(match['fail']), int(match['na']))
            continue
        match = LINK.search(line)
        if not match:
            if LINK_ANY.search(line):
                errors.append(f'line {n}: unreadable link line')
            continue
        name = match['name']
        if starts == 0:
            errors.append(f'line {n}: link {name} before the start line')
        if summary is not None:
            errors.append(f'line {n}: link {name} after the SUMMARY line')
        if name not in NAMES:
            errors.append(f'line {n}: link {name} is not in the expected list')
            continue
        if name in logged:
            errors.append(f'line {n}: link {name} logged a second time')
            continue
        expected = NAMES[len(logged)]
        if name != expected:
            errors.append(f'line {n}: link {name} out of order (expected {expected} next)')
        if match['fixtures'] is None:
            errors.append(f'line {n}: link {name} has no fixtures field')
        fixtures = [f.strip() for f in (match['fixtures'] or '').split(';') if f.strip()]
        logged[name] = (match['result'], match['detail'], fixtures)
    return starts, logged, summary, errors


# The pickup links name the aimed drop, the card's item at the use-key press and the item E picked up
# (`aimed_drop=... card_id=... picked_id=...`). A PASS needs all three to be the same item: the checker reads them itself
# rather than trusting the host's verdict.
CARD_LINKS = ('pick_up_gun', 'pick_up_aimed_not_nearest')
CARD_IDS = re.compile(r'\bcard_id=(?P<card>\S+) .*?\bpicked_id=(?P<picked>[^\s:,]+)')
AIMED_ID = re.compile(r'\baimed_drop=(?P<aimed>[^\s:,]+)')
ACCEPTED = re.compile(r'\baccepted=1\b')
DECOY_ID = re.compile(r'\bnearer_decoy=(?P<decoy>[^\s:,]+)')
DECOY_LINK = 'pick_up_aimed_not_nearest'
# The fixture that forces a gun from a kill.
FORCED_LOOT = 'guaranteed loot roll'
# A NOT_AVAILABLE from this test's own setup or local data, not from something the host lacks.
SETUP_GAP = 'test setup gap'
SETUP_GAP_OWNER = 'slice-gate'
# The fixture on link 4 when Maya went down before the selected enemy had hit her: a later hit from it proves nothing.
HOLD_ENDED_BY_DOWN = 'fire held, ended by Maya down'
# The fixture on link 4 when the selected enemy died with no hit on her: a PASS would contradict it.
HOLD_ENDED_BY_DEATH = 'fire held, ended by enemy died'


def card_link_problem(row):
    """Why a PASS on a card link cannot stand, or None. Two rolls can share an id, so the decoy link needs them to differ."""
    match = CARD_IDS.search(row['detail'])
    if not match:
        return 'no card_id/picked_id in the detail'
    if match['card'] != match['picked'] or match['card'] == 'none':
        return f'card_id {match["card"]} is not picked_id {match["picked"]}'
    aimed = AIMED_ID.search(row['detail'])
    if not aimed:
        return 'no aimed_drop in the detail'
    if aimed['aimed'] != match['picked']:
        return f'aimed_drop {aimed["aimed"]} is not picked_id {match["picked"]}'
    if not ACCEPTED.search(row['detail']):
        return 'the pickup was not accepted (no accepted=1 in the detail)'
    if row['name'] == DECOY_LINK:
        decoy = DECOY_ID.search(row['detail'])
        if not decoy or decoy['decoy'] == 'none':
            return 'no nearer_decoy id in the detail'
        if decoy['decoy'] == match['picked']:
            return f'nearer_decoy has the same id as the picked-up drop ({decoy["decoy"]})'
    return None


def card_attempt_seen(row):
    """Why a NOT_AVAILABLE on a card link cannot stand, or None: its ids show that the pickup attempt was made."""
    match = CARD_IDS.search(row['detail'])
    if not match:
        return None
    decoy = DECOY_ID.search(row['detail'])
    if decoy and decoy['decoy'] == match['picked']:
        return 'the ids show E took the nearer decoy'
    return 'the ids show the pickup attempt was made'


def cross_check(rows, errors, range_mode):
    """A PASS on a card link without matching, accepted ids becomes a FAIL, and so does a NOT_AVAILABLE where the
    capability was there."""
    for row in rows:
        if row['name'] in CARD_LINKS and row['result'] == 'PASS':
            why = card_link_problem(row)
            if why:
                errors.append(f'link {row["name"]}: PASS overruled, {why}')
                row['result'] = 'FAIL'
                row['detail'] = f'checker: {why}; host said PASS: {row["detail"]}'
        elif row['name'] == 'enemy_attacks_maya' and row['result'] == 'PASS' and (HOLD_ENDED_BY_DOWN in row['fixtures'] or HOLD_ENDED_BY_DEATH in row['fixtures']):
            why = ('the hold on fire ended with Maya down before the selected enemy had hit her' if HOLD_ENDED_BY_DOWN in row['fixtures']
                   else 'the hold on fire ended with the selected enemy dead and no hit from it')
            errors.append(f'link {row["name"]}: PASS overruled, {why}')
            row['result'] = 'FAIL'
            row['detail'] = f'checker: {why}; host said PASS: {row["detail"]}'
        elif row['result'] == 'NOT_AVAILABLE':
            if row['name'] == 'enemy_attacks_maya' and range_mode:
                why = 'the range was there, so its enemies can attack'
            elif row['name'] == 'loot_drop_on_kill' and FORCED_LOOT in row['fixtures']:
                why = 'the gun roll was forced'
            elif row['name'] in CARD_LINKS:
                why = card_attempt_seen(row)
            else:
                why = None
            if not why:
                continue
            errors.append(f'link {row["name"]}: NOT_AVAILABLE overruled, {why}')
            row['result'] = 'FAIL'
            row['detail'] = f'checker: {why}; host said NOT_AVAILABLE: {row["detail"]}'


def build_rows(logged):
    """One row per expected link, in order. A link that was never logged is a FAIL (not reached)."""
    rows = []
    for index, (name, owner) in enumerate(LINKS, 1):
        result, detail, fixtures = logged.get(name, ('FAIL', NOT_REACHED, []))
        if detail.startswith(SETUP_GAP):
            owner = SETUP_GAP_OWNER
        rows.append({'index': index, 'name': name, 'result': result, 'fixtures': fixtures, 'owner': owner, 'detail': detail})
    return rows


def result_cell(row):
    """The result with the fixtures it relied on, e.g. 'PASS (fixture: phaselock point granted)'."""
    if not row['fixtures']:
        return row['result']
    return f'{row["result"]} (fixture: {"; ".join(row["fixtures"])})'


def owner_cell(row):
    """The owner is shown only for a link that is not PASS."""
    return row['owner'] if row['result'] != 'PASS' else '-'


def table_text(rows):
    name_width = max(len('link'), *(len(row['name']) for row in rows))
    owner_width = max(len('owner'), *(len(owner_cell(row)) for row in rows))
    result_width = max(len('result'), *(len(result_cell(row)) for row in rows))
    out = [f'{"#":<2} {"link":<{name_width}}  {"result":<{result_width}}  {"owner":<{owner_width}}  detail']
    for row in rows:
        out.append(f'{row["index"]:<2} {row["name"]:<{name_width}}  {result_cell(row):<{result_width}}  '
                   f'{owner_cell(row):<{owner_width}}  {row["detail"]}')
    return '\n'.join(out)


def markdown_text(rows):
    out = ['| # | Link | Result | Owner (when not PASS) | Detail |', '|---|---|---|---|---|']
    for row in rows:
        detail = row['detail'].replace('|', '\\|')
        result = result_cell(row).replace('|', '\\|')
        out.append(f'| {row["index"]} | {row["name"]} | {result} | {owner_cell(row)} | {detail} |')
    return '\n'.join(out) + '\n'


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('log')
    parser.add_argument('--json', metavar='OUT', help='write the rows and totals as JSON')
    parser.add_argument('--markdown', metavar='OUT', help='write the rows as a markdown table')
    args = parser.parse_args(argv)

    lines = read_log(args.log)
    starts, logged, summary, errors = parse_log(lines)
    if not starts:
        print('no slice-gate run in this log')
        return 2

    rows = build_rows(logged)
    cross_check(rows, errors, any(RANGE_START.search(line) for line in lines))
    totals = {result: sum(1 for row in rows if row['result'] == result) for result in RESULTS}
    logged_counts = Counter(result for result, _, _ in logged.values())
    if summary is None:
        errors.append(NO_SUMMARY)
    else:
        said = {'PASS': summary[0], 'FAIL': summary[1], 'NOT_AVAILABLE': summary[2]}
        if any(said[result] != logged_counts[result] for result in RESULTS):
            errors.append(
                f'SUMMARY says pass={summary[0]} fail={summary[1]} na={summary[2]}, but the link lines say '
                f'pass={logged_counts["PASS"]} fail={logged_counts["FAIL"]} na={logged_counts["NOT_AVAILABLE"]}')
    failed = totals['FAIL'] > 0 or bool(errors)

    print(table_text(rows))
    print(f'slice gate: {totals["PASS"]} PASS, {totals["FAIL"]} FAIL, '
          f'{totals["NOT_AVAILABLE"]} NOT_AVAILABLE of {len(LINKS)}')
    gaps = sum(1 for row in rows if row['result'] == 'NOT_AVAILABLE' and row['detail'].startswith(SETUP_GAP))
    if gaps:
        print(f'{gaps} of those NOT_AVAILABLE are test setup gaps, not missing host capabilities: the link did not test the host')
    for error in errors:
        print(f'error: {error}')

    if args.json:
        report = {
            'links': [{'index': row['index'], 'name': row['name'], 'result': row['result'],
                       'fixtures': row['fixtures'], 'owner': row['owner'], 'detail': row['detail']} for row in rows],
            'pass': totals['PASS'],
            'fail': totals['FAIL'],
            'not_available': totals['NOT_AVAILABLE'],
            'summary_seen': summary is not None,
            'errors': errors,
        }
        Path(args.json).write_text(json.dumps(report, indent=1) + '\n', encoding='utf-8')
    if args.markdown:
        Path(args.markdown).write_text(markdown_text(rows), encoding='utf-8')
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
