"""Grade the rows of `ow-package --slice-gate` (the headless slice gate in ParityMode::Off) as ok / notyet / error.

AI-assisted (Claude Code), lane SLICE-RUN-OFF, decision 2026-11-19-08. `ow-package --slice-gate --cooked <dir>` prints one
JSON object per line for each of the 14 links of docs/verification/SLICE_GATE.md, in gate order (crates/ow-cli/src/cmd/
slice_gate.rs gives the fields). This checker reads those lines, checks their structure, prints the per-link table and
the counts, and grades the run:

- **Structure.** Exactly 14 rows, `n` 1 to 14 in order, the names of tools/check_slice_gate.py's LINKS in that order; every
  row has every field with the right type; `result` is ok, notyet or error; one `grading` for all rows; a `stop` is null
  exactly when the result is ok; an ok row, and every row under strict grading, has no `error_lines`; a `blocked_by`
  names an earlier link that is not ok, and the row carries that link's result. A row off this structure is a malformed run (exit 2).
- **The gate** is 14 ok, 0 notyet, 0 error, under `strict` grading (exit 0). A `past-errors` run is informational: it
  never passes (exit 1), whatever its counts. Any other run exits 1.

The drivers are `combat-run`, `slice-run`, `equip-run` (link 8) and `phaselock-cast` (link 9, lane
SLICE-GATE-DRIVERS, decision 2026-11-19-39), or `none`. `equip-run` and `phaselock-cast` drive their own link
only, and a `phaselock-cast` row is never ok until an evidence check is chosen for it.

A pass says the Rust port's headless drivers did each link in Off; it says nothing about the real game (UNVERIFIED).

    ow-package --slice-gate --cooked <dir> > rows.jsonl
    python tools/check_slice_run_off.py rows.jsonl [--markdown table.md] [--json summary.json]
"""
import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from check_slice_gate import NAMES  # noqa: E402  (the gate's 14 link names, in order)

RESULTS = ('ok', 'notyet', 'error')
GRADINGS = ('strict', 'past-errors')
DRIVERS = ('combat-run', 'slice-run', 'equip-run', 'phaselock-cast', 'none')
# The drivers that run one link only, and the link each runs (decision 2026-11-19-39).
DRIVER_LINK = {'equip-run': 'equip_gun', 'phaselock-cast': 'phaselock_enemy'}
# Drivers with no evidence check chosen yet: a row they drive is never ok.
NO_EVIDENCE_YET = ('phaselock-cast',)
STOP_FIELDS = ('step', 'native', 'record', 'text')


class Malformed(Exception):
    """The rows are not a --slice-gate run."""


def _string(value):
    return isinstance(value, str)


def parse(lines):
    """The rows of `lines` (JSON objects, blank lines skipped), checked (module documentation). Raises Malformed."""
    rows = []
    for number, line in enumerate(lines, 1):
        if not line.strip():
            continue
        try:
            row = json.loads(line)
        except json.JSONDecodeError as e:
            raise Malformed(f'line {number}: not JSON ({e.msg})') from None
        if not isinstance(row, dict):
            raise Malformed(f'line {number}: not an object')
        rows.append(row)
    if len(rows) != len(NAMES):
        raise Malformed(f'{len(rows)} rows, expected {len(NAMES)}')
    seen = {}
    for i, row in enumerate(rows):
        where = f'row {i + 1}'
        if row.get('n') != i + 1 or row.get('link') != NAMES[i]:
            raise Malformed(f'{where}: expected n={i + 1} link={NAMES[i]}, got n={row.get("n")} link={row.get("link")}')
        if row.get('result') not in RESULTS:
            raise Malformed(f'{where}: result {row.get("result")!r}')
        if row.get('grading') not in GRADINGS or row['grading'] != rows[0].get('grading'):
            raise Malformed(f'{where}: grading {row.get("grading")!r} (one grading for all rows)')
        if row.get('driver') not in DRIVERS:
            raise Malformed(f'{where}: driver {row.get("driver")!r}')
        if DRIVER_LINK.get(row['driver'], row['link']) != row['link']:
            raise Malformed(f'{where}: driver {row["driver"]} runs {DRIVER_LINK[row["driver"]]} only')
        if not isinstance(row.get('steps'), list) or not all(_string(s) for s in row['steps']):
            raise Malformed(f'{where}: steps is not a list of strings')
        if not _string(row.get('scope')):
            raise Malformed(f'{where}: scope is not a string')
        lines_ = row.get('error_lines')
        if not isinstance(lines_, list) or not all(
                isinstance(e, dict) and _string(e.get('step')) and _string(e.get('text')) for e in lines_):
            raise Malformed(f'{where}: error_lines is not a list of {{step, text}}')
        if lines_ and row['result'] == 'ok':
            raise Malformed(f'{where}: an ok row has error lines')
        if lines_ and row['grading'] == 'strict':
            raise Malformed(f'{where}: a strict row has error lines (strict grading stops at the first one)')
        stop = row.get('stop', 'missing')
        if row['result'] == 'ok':
            if stop is not None:
                raise Malformed(f'{where}: an ok row has a stop')
        elif not (isinstance(stop, dict) and all(_string(stop.get(k)) for k in STOP_FIELDS)):
            raise Malformed(f'{where}: a {row["result"]} row needs a stop with {", ".join(STOP_FIELDS)}')
        for key in ('blocked_by', 'after_errors_in'):
            if key not in row:
                raise Malformed(f'{where}: no {key}')
            name = row[key]
            if name is None:
                continue
            if name not in seen:
                raise Malformed(f'{where}: {key} names {name!r}, not an earlier link')
            if seen[name]['result'] == 'ok':
                raise Malformed(f'{where}: {key} names {name}, which is ok')
            if key == 'blocked_by' and row['result'] != seen[name]['result']:
                raise Malformed(f'{where}: blocked by {name} but its result differs')
        if row['driver'] == 'none' and row['result'] == 'ok':
            raise Malformed(f'{where}: a link without a driver cannot be ok')
        if row['driver'] in NO_EVIDENCE_YET and row['result'] == 'ok':
            raise Malformed(f'{where}: {row["driver"]} has no evidence check yet, so it cannot be ok')
        seen[row['link']] = row
    return rows


def counts(rows):
    return {r: sum(1 for row in rows if row['result'] == r) for r in RESULTS}


def passes(rows):
    """Whether the run is the gate: 14 ok under strict grading."""
    return rows[0]['grading'] == 'strict' and counts(rows)['ok'] == len(NAMES)


def _cell(text, limit=110):
    text = ' '.join(str(text).split()).replace('|', '\\|')
    return text if len(text) <= limit else text[:limit - 3] + '...'


def table(rows):
    """The per-link table, as markdown."""
    c = counts(rows)
    out = [f'Grading: {rows[0]["grading"]}. ok {c["ok"]}, notyet {c["notyet"]}, error {c["error"]} of {len(rows)}.', '',
           '| # | Link | Result | Driver | Stop step | Native or subject | Record | Note |',
           '|---|---|---|---|---|---|---|---|']
    for row in rows:
        stop = row['stop'] or {}
        note = []
        if row['blocked_by']:
            note.append(f'blocked by {row["blocked_by"]}')
        if row['after_errors_in']:
            note.append(f'after error lines in {row["after_errors_in"]}')
        if row['error_lines']:
            note.append(f'{len(row["error_lines"])} error line(s)')
        if row['driver'] == 'none':
            note.append(stop.get('text', ''))
        out.append('| {} | {} | {} | {} | {} | {} | {} | {} |'.format(
            row['n'], row['link'], row['result'], row['driver'], _cell(stop.get('step', '')),
            _cell(stop.get('native', '') or ('' if row['driver'] == 'none' else stop.get('text', ''))),
            _cell(stop.get('record', '')), _cell('; '.join(note))))
    return '\n'.join(out) + '\n'


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('rows', type=Path, help='the --slice-gate output (one JSON row per line)')
    parser.add_argument('--markdown', type=Path, help='also write the table here')
    parser.add_argument('--json', type=Path, help='also write the counts and the grade here')
    args = parser.parse_args(argv)
    try:
        rows = parse(args.rows.read_text(encoding='utf-8').splitlines())
    except (OSError, Malformed) as e:
        print(f'check_slice_run_off: {e}', file=sys.stderr)
        return 2
    text = table(rows)
    ok = passes(rows)
    sys.stdout.write(text)
    print('GATE PASS' if ok else 'GATE NOT PASSED')
    if args.markdown:
        args.markdown.write_text(text, encoding='utf-8')
    if args.json:
        summary = {'grading': rows[0]['grading'], 'counts': counts(rows), 'pass': ok,
                   'links': [{'n': r['n'], 'link': r['link'], 'result': r['result']} for r in rows]}
        args.json.write_text(json.dumps(summary, indent=2) + '\n', encoding='utf-8')
    return 0 if ok else 1


if __name__ == '__main__':
    sys.exit(main())
