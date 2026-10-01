"""Cache ow-package --exports for installed packages under ignored local/census/exports.

Incremental: a package is re-read only when its size/mtime changed. Output is
game-derived and stays under local/. Failures are recorded, never skipped silently.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import time


def main():
    root = Path(__file__).resolve().parents[1]
    ap = argparse.ArgumentParser()
    ap.add_argument('--game', default=os.environ.get('OPENWILLOW_BL2'))
    ap.add_argument('--reader', default=str(root / 'build/Release/ow-package.exe'))
    ap.add_argument('--out', default=str(root / 'local/census/exports'))
    ap.add_argument('--only', nargs='*', help='package names without extension')
    args = ap.parse_args()
    out = Path(args.out).resolve()
    if not out.is_relative_to(root / 'local'):
        sys.exit('Output must stay under repository local/')
    out.mkdir(parents=True, exist_ok=True)
    cooked = Path(args.game) / 'WillowGame/CookedPCConsole'
    index_path = out / '_index.json'
    index = json.loads(index_path.read_text()) if index_path.exists() else {}
    failures = {}
    t0 = time.time()
    files = sorted(cooked.glob('*.upk'))
    if args.only:
        files = [f for f in files if f.stem in args.only]
    for n, pkg in enumerate(files):
        st = pkg.stat()
        key = [st.st_size, int(st.st_mtime)]
        if index.get(pkg.stem, {}).get('stat') == key and (out / (pkg.stem + '.json')).exists():
            continue
        run = subprocess.run([args.reader, str(pkg), '--exports'], capture_output=True, encoding='utf-8')
        if run.returncode:
            failures[pkg.stem] = run.stderr.strip()[:300]
            continue
        (out / (pkg.stem + '.json')).write_text(run.stdout, encoding='utf-8')
        index[pkg.stem] = {'stat': key, 'exports': run.stdout.count('"index":')}
        if n % 50 == 0:
            print(n, len(files), round(time.time() - t0), flush=True)
    index_path.write_text(json.dumps(index))
    (out / '_failures.json').write_text(json.dumps(failures, indent=1))
    print('packages', len(index), 'failures', len(failures), 'seconds', round(time.time() - t0))


main()
