"""Check a slice-gate run (the host's `-owslicegate` run) from its Unreal log.

The host logs `OWSLICEGATE link=<name> result=<PASS|FAIL|NOT_AVAILABLE> detail=<text>` for each link it reaches, and
`OWSLICEGATE SUMMARY pass=<n> fail=<n> na=<n>` when the run finishes. This checker lists the 13 links in order, marks
every link the host never logged as FAIL ("not reached"), and checks the SUMMARY against the link lines.

Exit codes: 0 when no link is FAIL and the SUMMARY is there and agrees; 1 when a link is FAIL, the SUMMARY is missing
or disagrees; 2 when the log has no `OWSLICEGATE start` line. NOT_AVAILABLE is a known gap and does not fail the run.
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
LINK = re.compile(r'OWSLICEGATE link=(?P<name>\S+) result=(?P<result>PASS|FAIL|NOT_AVAILABLE) detail=(?P<detail>.*)$')
LINK_ANY = re.compile(r'OWSLICEGATE link=')
SUMMARY = re.compile(r'OWSLICEGATE SUMMARY pass=(?P<pass>\d+) fail=(?P<fail>\d+) na=(?P<na>\d+)')


def read_log(path):
    """The log's lines; a UTF-8 byte order mark at the start of the file is dropped."""
    text = Path(path).read_text(encoding='utf-8', errors='replace')
    return text.lstrip(BOM).splitlines()


def parse_log(lines):
    """Return (started, logged, summary, warnings).

    logged maps each expected link name to its (result, detail) from the last line that names it. summary is the
    (pass, fail, na) tuple of the last SUMMARY line, or None.
    """
    started = False
    logged = {}
    summary = None
    warnings = []
    for n, line in enumerate(lines, 1):
        if START.search(line):
            started = True
            continue
        match = SUMMARY.search(line)
        if match:
            summary = (int(match['pass']), int(match['fail']), int(match['na']))
            continue
        match = LINK.search(line)
        if match:
            name = match['name']
            if name not in NAMES:
                warnings.append(f'line {n}: link {name} is not in the expected list, ignored')
                continue
            if name in logged:
                warnings.append(f'line {n}: link {name} logged again, the last result wins')
            logged[name] = (match['result'], match['detail'])
        elif LINK_ANY.search(line):
            warnings.append(f'line {n}: unreadable link line, ignored')
    return started, logged, summary, warnings


def build_rows(logged):
    """One row per expected link, in order. A link that was never logged is a FAIL (not reached)."""
    rows = []
    for index, (name, owner) in enumerate(LINKS, 1):
        if name in logged:
            result, detail = logged[name]
        else:
            result, detail = 'FAIL', NOT_REACHED
        rows.append({'index': index, 'name': name, 'result': result, 'owner': owner, 'detail': detail})
    return rows


def owner_cell(row):
    """The owner is shown only for a link that is not PASS."""
    return row['owner'] if row['result'] != 'PASS' else '-'


def table_text(rows):
    name_width = max(len('link'), *(len(row['name']) for row in rows))
    owner_width = max(len('owner'), *(len(owner_cell(row)) for row in rows))
    out = [f'{"#":<2} {"link":<{name_width}}  {"result":<13}  {"owner":<{owner_width}}  detail']
    for row in rows:
        out.append(f'{row["index"]:<2} {row["name"]:<{name_width}}  {row["result"]:<13}  '
                   f'{owner_cell(row):<{owner_width}}  {row["detail"]}')
    return '\n'.join(out)


def markdown_text(rows):
    out = ['| # | Link | Result | Owner (when not PASS) | Detail |', '|---|---|---|---|---|']
    for row in rows:
        detail = row['detail'].replace('|', '\\|')
        out.append(f'| {row["index"]} | {row["name"]} | {row["result"]} | {owner_cell(row)} | {detail} |')
    return '\n'.join(out) + '\n'


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('log')
    parser.add_argument('--json', metavar='OUT', help='write the rows and totals as JSON')
    parser.add_argument('--markdown', metavar='OUT', help='write the rows as a markdown table')
    args = parser.parse_args(argv)

    started, logged, summary, warnings = parse_log(read_log(args.log))
    if not started:
        print('no slice-gate run in this log')
        return 2

    rows = build_rows(logged)
    totals = {result: sum(1 for row in rows if row['result'] == result) for result in RESULTS}
    logged_counts = Counter(result for result, _ in logged.values())
    mismatch = False
    if summary is None:
        warnings.append(NO_SUMMARY)
    else:
        said = {'PASS': summary[0], 'FAIL': summary[1], 'NOT_AVAILABLE': summary[2]}
        if any(said[result] != logged_counts[result] for result in RESULTS):
            mismatch = True
            warnings.append(
                f'SUMMARY says pass={summary[0]} fail={summary[1]} na={summary[2]}, but the link lines say '
                f'pass={logged_counts["PASS"]} fail={logged_counts["FAIL"]} na={logged_counts["NOT_AVAILABLE"]}')
    failed = totals['FAIL'] > 0 or summary is None or mismatch

    print(table_text(rows))
    print(f'slice gate: {totals["PASS"]} PASS, {totals["FAIL"]} FAIL, '
          f'{totals["NOT_AVAILABLE"]} NOT_AVAILABLE of {len(LINKS)}')
    for warning in warnings:
        print(f'warning: {warning}')

    if args.json:
        report = {
            'links': [{'index': row['index'], 'name': row['name'], 'result': row['result'],
                       'owner': row['owner'], 'detail': row['detail']} for row in rows],
            'pass': totals['PASS'],
            'fail': totals['FAIL'],
            'not_available': totals['NOT_AVAILABLE'],
            'summary_seen': summary is not None,
            'warnings': warnings,
        }
        Path(args.json).write_text(json.dumps(report, indent=1) + '\n', encoding='utf-8')
    if args.markdown:
        Path(args.markdown).write_text(markdown_text(rows), encoding='utf-8')
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
