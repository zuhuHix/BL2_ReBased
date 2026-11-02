"""Triangle and fragment oracle for the weapon assembly: game = host, per gun.

Host side: the UE log lines the assembly writes when it shows a gun (OpenWillowWeaponAssembly.cpp):
  OpenWillow weapon assembly <id>: type=<T> fragments=<n> (parts) missing=[] triangles=<N> skin=<MIC> material=<asset>
      hidden=[...] shown=[<fragment>,...] <cold|warm> <ms> ms
  OpenWillow weapon assembly <id>: fallback to the pre-built mesh <path> (<reason>)
Game side, two references:
  - predicted: tools/weapon_assembly_sample.py's sample_index.json (the gun's fragments and the cooked gestalt table summed
    over them; the table equals the running game's, checked 2026-10-22 for all six weapon gestalts);
  - live: tools/real_game/scripts/weapon_part_capture.py's live_guns.json (the spawned gun's own gestalt fragment names
    and triangle ranges, read from the running game), where a gun was spawned.
A gun passes when the host showed it assembled and every reference it has agrees on both the exact fragment-name set and
the triangle total. Verdicts: pass; fail (a reference disagrees); fallback (the host used the pre-built mesh);
not_shown (no host line); no_reference (shown, but nothing to compare with: incomplete, not a pass). Exit status 1 when
any gun is not a pass, when the index is empty, or when it does not hold --expect guns. Writes a JSON table (ignored local/). Our own code; no game data is stored in the repository.

  python tools/weapon_assembly_oracle.py --log <UE log> --index local/wpa/sample/sample_index.json \\
      [--live local/realgame/wpa/live_guns.json] --output local/wpa/oracle.json
"""
import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LINE = re.compile(r'OpenWillow weapon assembly (?P<id>[A-Za-z0-9_]+): type=(?P<type>\S*) fragments=(?P<n>\d+) \((?P<source>\w*)\) '
                  r'missing=\[(?P<missing>[^\]]*)\] triangles=(?P<tri>\d+) skin=(?P<skin>\S*) material=(?P<material>\S*)'
                  r'(?: hidden=\[(?P<hidden>[^\]]*)\])?(?: shown=\[(?P<shown>[^\]]*)\])?')
FALLBACK = re.compile(r'OpenWillow weapon assembly (?P<id>[A-Za-z0-9_]+): fallback to the pre-built mesh (?P<mesh>\S*) \((?P<reason>.*)\)')


def split_list(text):
    return [item for item in (text or '').split(',') if item]


def host_reports(text):
    """Last assembly report per gun id in a UE log (a later line for the same gun replaces an earlier one)."""
    found = {}
    for line in text.splitlines():
        match = LINE.search(line)
        if match:
            found[match['id']] = {'assembled': True, 'type': match['type'], 'fragments': int(match['n']),
                                  'source': match['source'], 'missing': split_list(match['missing']),
                                  'triangles': int(match['tri']), 'skin': match['skin'], 'material': match['material'],
                                  'shown': split_list(match['shown']) if match['shown'] is not None else None}
            continue
        match = FALLBACK.search(line)
        if match:
            found[match['id']] = {'assembled': False, 'mesh': match['mesh'], 'reason': match['reason']}
    return found


def live_fragments(record):
    """(fragment names, triangles) of one live_guns.json record, or (None, None)."""
    if not record:
        return None, None
    data = ((record.get('gestalt_fields') or {}).get('GestaltData') or {})
    names = data.get('PartMeshNames')
    return (list(names) if names is not None else None), record.get('triangles')


def compare(index, host, live=None):
    """One row per gun of the index (see the module docstring for the verdicts)."""
    rows = []
    for gun, entry in sorted(index.items()):
        report = host.get(gun)
        live_names, live_triangles = live_fragments((live or {}).get(gun))
        references = []
        if entry.get('expected_triangles') is not None or entry.get('fragments') is not None:
            references.append(('predicted', entry.get('fragments'), entry.get('expected_triangles')))
        if live_names is not None or live_triangles is not None:
            references.append(('live', live_names, live_triangles))
        problems = []
        if report is None:
            verdict = 'not_shown'
        elif not report['assembled']:
            verdict = 'fallback'
            problems.append(report['reason'])
        elif report['missing']:
            verdict = 'fail'
            problems.append('missing fragments ' + ','.join(report['missing']))
        elif not references:
            verdict = 'no_reference'
        else:
            for name, fragments, triangles in references:
                if triangles is not None and triangles != report['triangles']:
                    problems.append(f'{name} triangles {triangles} != host {report["triangles"]}')
                if fragments is not None:
                    if report['shown'] is None:
                        problems.append(f'host log has no fragment names to compare with {name}')
                    elif sorted(fragments) != sorted(report['shown']):
                        problems.append(f'{name} fragments differ: only {name} {sorted(set(fragments) - set(report["shown"]))}, '
                                        f'only host {sorted(set(report["shown"]) - set(fragments))}')
            verdict = 'fail' if problems else 'pass'
        rows.append({'id': gun, 'gestalt': entry.get('gestalt'), 'manufacturer': entry.get('manufacturer'),
                     'set': entry.get('set'), 'predicted': entry.get('expected_triangles'), 'live': live_triangles,
                     'live_checked': live_names is not None, 'host': report and report.get('triangles'),
                     'material': report and report.get('material'), 'verdict': verdict, 'problems': problems})
    return rows


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--log', type=Path, nargs='+', required=True)
    parser.add_argument('--index', type=Path, required=True)
    parser.add_argument('--live', type=Path)
    parser.add_argument('--output', type=Path, default=ROOT / 'local/wpa/oracle.json')
    parser.add_argument('--expect', type=int, help='number of guns the index must hold (default: at least one)')
    args = parser.parse_args(argv)
    if not args.output.resolve().is_relative_to((ROOT / 'local').resolve()):
        parser.error('output must stay under local/')
    host = {}
    for log in args.log:
        host.update(host_reports(log.read_text(encoding='utf-8', errors='replace')))
    index = json.loads(args.index.read_text(encoding='utf-8'))
    index = index.get('guns', index)
    live = json.loads(args.live.read_text(encoding='utf-8')) if args.live and args.live.exists() else None
    rows = compare(index, host, live)
    counts = {}
    for row in rows:
        counts[row['verdict']] = counts.get(row['verdict'], 0) + 1
    live_passes = sum(1 for row in rows if row['live_checked'] and row['verdict'] == 'pass')
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps({'counts': counts, 'live_pass': live_passes, 'rows': rows}, indent=1), encoding='utf-8')
    print(f'oracle: {counts}; live-checked passes {live_passes}')
    for row in rows:
        if row['verdict'] != 'pass':
            detail = '; '.join(row['problems']) or ('incomplete: no reference to compare with' if row['verdict'] == 'no_reference'
                                                    else 'no host report')
            print(f"  {row['id']}: {row['verdict']} ({detail})")
    if not rows:
        print('  no guns in the index: nothing was checked')
        return 1
    if args.expect is not None and len(rows) != args.expect:
        print(f'  the index holds {len(rows)} guns, expected {args.expect}')
        return 1
    return 0 if all(row['verdict'] == 'pass' for row in rows) else 1


if __name__ == '__main__':
    sys.exit(main())
