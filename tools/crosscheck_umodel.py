"""Cross-check the reader's export tables against UE Viewer (umodel) listings.

umodel is run as an independent, read-only oracle: for every package its
``-list`` output gives one line per export with index, serial offset (hex),
serial size (hex), class name and object name, plus a header with the name,
import and export counts. Those are compared field by field with
``ow-package --exports``. Agreement means two independent readers place every
export at the same byte range with the same class and name; it says nothing
about the meaning of the bytes inside. Game-derived output stays under
``local/``. No umodel code is used or copied; see THIRD_PARTY.md.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
import json
from pathlib import Path
import re
import subprocess
import sys

# Object names may contain spaces (e.g. "fog-of-war-blob t"), class names do not.
LISTING = re.compile(r'^\s*(\d+)\s+([0-9A-Fa-f]+)\s+([0-9A-Fa-f]+)\s+(\S+)\s+(\S.*?)\s*$')
HEADER = re.compile(r'Loading package: (\S+) Ver: (\d+)/(\d+).*?Names: (\d+) Exports: (\d+) Imports: (\d+)')
NORMALIZED = re.compile(r'^__name_\d+__$')


def parse_listing(text):
    """Return (header, rows) from umodel -list output; rows are dicts with 1-based index."""
    header = None
    rows = []
    for line in text.splitlines():
        found = HEADER.search(line)
        if found:
            header = dict(package=found.group(1), version=int(found.group(2)), licensee=int(found.group(3)),
                          names=int(found.group(4)), exports=int(found.group(5)), imports=int(found.group(6)))
            continue
        match = LISTING.match(line)
        if match and header is not None:
            rows.append(dict(index=int(match.group(1)) + 1, offset=int(match.group(2), 16),
                             size=int(match.group(3), 16), cls=match.group(4), name=match.group(5)))
    return header, rows


def normalized_by_umodel(reader_name, umodel_name):
    """True when umodel rewrote a name it considers malformed rather than reading different bytes.

    Observed on the full install: names containing a control or non-ASCII byte come back as
    ``__name_N__`` and a single trailing space is trimmed. The reader keeps the raw bytes.
    """
    if reader_name.rstrip(' ') == umodel_name and reader_name != umodel_name:
        return True
    return bool(NORMALIZED.match(umodel_name)) and any(ord(c) < 0x20 or ord(c) > 0x7E for c in reader_name)


def compare(header, rows, records, package_header=None):
    """Compare umodel rows with reader export records. Returns a per-package report.

    ``mismatches`` holds real disagreements; ``name_normalized`` holds name-only differences
    explained by umodel's name sanitising, which do not count against agreement.
    """
    report = dict(umodel_exports=len(rows), reader_exports=len(records), mismatches=[], name_normalized=[], compared=0,
                  header=header, header_mismatch=None)
    if header is None:
        report['error'] = 'umodel header not found'
        return report
    if header['exports'] != len(rows):
        report['error'] = f"umodel listed {len(rows)} rows but its header says {header['exports']} exports"
    if package_header is not None:
        differences = {key: (header[key], package_header[key]) for key in ('names', 'exports', 'imports', 'version', 'licensee')
                       if key in package_header and header[key] != package_header[key]}
        report['header_mismatch'] = differences or None
    by_index = {record['index']: record for record in records}
    for row in rows:
        record = by_index.get(row['index'])
        if record is None:
            report['mismatches'].append(dict(index=row['index'], field='missing_in_reader', umodel=row))
            continue
        report['compared'] += 1
        reader_class = record['class'].rsplit('.', 1)[-1]
        for field, ours, theirs in (('offset', record['offset'], row['offset']), ('size', record['size'], row['size']),
                                    ('class', reader_class, row['cls']), ('name', record['name'], row['name'])):
            if ours != theirs:
                item = dict(index=row['index'], field=field, reader=ours, umodel=theirs, path=record['path'])
                bucket = 'name_normalized' if field == 'name' and normalized_by_umodel(ours, theirs) else 'mismatches'
                report[bucket].append(item)
    for index in sorted(set(by_index) - {row['index'] for row in rows}):
        report['mismatches'].append(dict(index=index, field='missing_in_umodel', reader=by_index[index]['path']))
    return report


def run(command, encoding='utf-8'):
    result = subprocess.run(command, capture_output=True, text=True, encoding=encoding, errors='replace')
    return result.returncode, result.stdout + result.stderr


def label(package):
    return '/'.join(package.parts[-4:]) if 'DLC' in package.parts else package.name


def check_package(umodel, reader, package, game_tag):
    code, listing = run([str(umodel), '-list', f'-game={game_tag}', str(package)], encoding='latin-1')
    header, rows = parse_listing(listing)
    if code != 0 and header is None:
        return label(package), dict(error='umodel failed: ' + listing.strip().splitlines()[-1][:200] if listing.strip() else 'umodel failed')
    code, exports = run([str(reader), str(package), '--exports'])
    if code != 0:
        return label(package), dict(error='reader failed: ' + exports.strip()[-200:], header=header, umodel_exports=len(rows))
    code, summary = run([str(reader), str(package), '--census'])
    package_header = None
    if code == 0:
        try:
            census = json.loads(summary)
            package_header = dict(exports=census.get('exports'))
        except json.JSONDecodeError:
            package_header = None
    return label(package), compare(header, rows, json.loads(exports), package_header)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--umodel', type=Path, required=True, help='umodel.exe (UE Viewer, MIT)')
    parser.add_argument('--reader', type=Path, required=True, help='ow-package executable')
    parser.add_argument('--game', type=Path, required=True, help='Borderlands 2 install root')
    parser.add_argument('--packages', nargs='*', help='Package names without extension (default: all base .upk)')
    parser.add_argument('--dlc', action='store_true', help='Also scan DLC/*/*/Content/*.upk (1,096 packages on a full install)')
    parser.add_argument('--game-tag', default='border', help='umodel game tag (Borderlands family)')
    parser.add_argument('--workers', type=int, default=4)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()

    cooked = args.game / 'WillowGame' / 'CookedPCConsole'
    if args.packages:
        packages = [cooked / (name + '.upk') for name in args.packages]
    else:
        packages = sorted(cooked.glob('*.upk'))
        if args.dlc:
            packages += sorted((args.game / 'DLC').glob('*/*/Content/*.upk'))
    missing = [p for p in packages if not p.exists()]
    if missing:
        sys.exit('missing packages: ' + ', '.join(str(p) for p in missing))

    results = {}
    with ThreadPoolExecutor(max_workers=args.workers) as pool:
        for name, report in pool.map(lambda p: check_package(args.umodel, args.reader, p, args.game_tag), packages):
            results[name] = report
            if report.get('error'):
                print(f'{name}: ERROR {report["error"]}', flush=True)
            elif report['mismatches'] or report['header_mismatch']:
                print(f'{name}: {len(report["mismatches"])} export mismatches, header {report["header_mismatch"]}', flush=True)

    ok = [n for n, r in results.items() if not r.get('error') and not r['mismatches'] and not r['header_mismatch']]
    errors = [n for n, r in results.items() if r.get('error')]
    mismatched = [n for n, r in results.items() if not r.get('error') and (r['mismatches'] or r['header_mismatch'])]
    compared = sum(r.get('compared', 0) for r in results.values())
    summary = dict(packages=len(results), agreeing_packages=len(ok), mismatched_packages=mismatched, error_packages=errors,
                   exports_compared=compared,
                   mismatched_exports=sum(len(r.get('mismatches', [])) for r in results.values()),
                   name_normalized_exports=sum(len(r.get('name_normalized', [])) for r in results.values()))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(dict(summary=summary, packages=results), indent=1), encoding='utf-8')
    print(f"packages={summary['packages']} agreeing={summary['agreeing_packages']} mismatched={len(mismatched)} "
          f"errors={len(errors)} exports_compared={compared} mismatched_exports={summary['mismatched_exports']} "
          f"name_normalized_by_umodel={summary['name_normalized_exports']}")
    for name in mismatched[:20]:
        for item in results[name]['mismatches'][:5]:
            print(f'  {name}: {item}')
    for name in errors[:20]:
        print(f'  {name}: {results[name]["error"]}')
    return 0 if not mismatched and not errors else 1


if __name__ == '__main__':
    sys.exit(main())
