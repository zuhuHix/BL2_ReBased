"""Deterministic installed-package census. Reports failures; never hides omissions."""
import argparse
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
import json
from pathlib import Path
import subprocess


def census(reader, game, workers):
    paths = sorted((p for p in game.rglob('*') if p.suffix.lower() in {'.upk', '.umap', '.u'}),
                   key=lambda p: p.relative_to(game).as_posix())
    def inspect(path):
        # These two UHD support files have a .upk suffix but contain custom
        # texture metadata/payload, not a UE package header or export table.
        sidecars = {'Mancana_Textures_Data.upk': b'\x4d\x00\x00\x00\x06\x00\x00\x00SizeX\x00',
                    'Mancana_Textures_Meta.upk': b'\x1a\x00\x00\x00Mancana_Textures_Data'}
        signature = sidecars.get(path.name)
        if signature and path.parent.name == 'Remaster':
            with path.open('rb') as source:
                if source.read(len(signature)) == signature:
                    return {'package': path.relative_to(game).as_posix(),
                            'classification': 'UHD texture sidecar; no UE export table', 'bytes': path.stat().st_size}
        result = subprocess.run([str(reader), str(path), '--census'], capture_output=True, text=True, encoding='utf-8')
        record = {'package': path.relative_to(game).as_posix()}
        if result.returncode:
            record['error'] = result.stderr.strip()
        else:
            record.update(json.loads(result.stdout))
        return record
    with ThreadPoolExecutor(max_workers=workers) as pool:
        records = list(pool.map(inspect, paths))
    totals = Counter()
    for record in records:
        totals.update(record.get('classes', {}))
    return {'schema': 1, 'count_semantics': 'serialized exports, including copies across packages; not unique assets',
            'files_found': len(paths), 'sidecars': sum('classification' in r for r in records),
            'packages_found': sum('classification' not in r for r in records),
            'packages_read': sum('exports' in r for r in records),
            'exports': sum(totals.values()), 'classes': dict(sorted(totals.items())), 'packages': records}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reader', type=Path, required=True)
    parser.add_argument('--game', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--workers', type=int, default=2)
    args = parser.parse_args()
    if not 1 <= args.workers <= 16:
        parser.error('--workers must be between 1 and 16')
    if not args.reader.is_file():
        parser.error('--reader does not exist')
    if not (args.game / 'WillowGame' / 'CookedPCConsole' / 'Core.upk').is_file():
        parser.error('--game must point to a BL2 installation containing Core.upk')
    report = census(args.reader.resolve(), args.game.resolve(), args.workers)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
    print(json.dumps({k: v for k, v in report.items() if k not in {'classes', 'packages'}}))
    raise SystemExit(0 if report['packages_found'] == report['packages_read'] else 1)
