"""Compare the C++ weapon generator (ow-weapongen) with the Python one, field by field.

Python side: weapon_recipe.roll(..., rng=CrtRand(seed)) and weapon_stats.evaluate() (with the
installed English .int overrides, like weapon_stats.main()), plus loot_pools.roll_pool_crt() /
roll_crt() for pools and sources. C++ side: one ow-weapongen --batch run over the same
requests, reading the manifest tools/weapon_gen_manifest.py wrote from the same package.

Equality is exact: strings, integers and printed numbers must be equal, unrounded floats must
be bitwise equal (a difference is reported with its ULP distance), and a number must have the
same JSON kind (integer or float). Card fields compared: every key weapon_stats.evaluate()
puts on the card except EXCLUDED_CARD_FIELDS. Guns whose Python evaluation raises are counted
and listed, not compared. The report goes to local/weapons/gen/ (game-derived; ignored).
--stage-cap N rolls every gun (balances and pool drops) with weapon_recipe.roll(stage_cap=N) on the
Python side and the batch line's cap field on the C++ side; pool gates keep the spawn stage.
The identity fields ow-weapongen echoes (balance, stage, seed, spawn stage, cap; pool, stage, seed) are checked
against each request. Exit status 1 when any field mismatches, any Python evaluation raises, fewer comparisons
than requested were made, or the C++ output has too few or too many lines (verdict()); 0 otherwise.
"""
import argparse
import json
import math
import struct
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import loot_pools  # noqa: E402
import weapon_gen_manifest  # noqa: E402
import weapon_recipe  # noqa: E402
import weapon_stats  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
STAGES = (1, 8, 15, 30, 50, 80)
# Card fields the C++ port does not produce: the per-line list duplicates fun_stats/fun_stats_markup.
EXCLUDED_CARD_FIELDS = ('fun_stats_lines',)
# Float properties Python copies straight from the decoded data: the decoder prints a whole value as an
# integer (3), the manifest keeps the float (3.0). Compared as floats (as the name-part priority is).
FLOAT_PROPERTY_FIELDS = ('spin_start_interval_scale',)
# Request identity the C++ echoes back (the card's level carries the capped stage).


def python_gun(package, localize, balance, stage, seed, stage_cap=None):
    """The Python generator's result in ow-weapongen's JSON shape."""
    recipe = weapon_recipe.roll(package, balance, seed, stage, rng=weapon_recipe.CrtRand(seed), stage_cap=stage_cap)
    card = weapon_stats.evaluate(package, recipe, recipe['game_stage'], localize)['card']
    for field in FLOAT_PROPERTY_FIELDS:
        if isinstance(card.get(field), int):
            card[field] = float(card[field])

    def name(entry):
        # Priority is a float property; the decoder prints a whole value as an integer (10), the manifest
        # stores it as the float 10.0. Compared as a float.
        return entry and {'part': entry['part'], 'text': entry['text'], 'priority': float(entry['priority'])}
    return {
        'manufacturer': recipe['manufacturer'], 'weapon_type': recipe['weapon_type'], 'gestalt': recipe['gestalt'],
        'name': recipe['name'], 'prefix': name(recipe['prefix']), 'title': name(recipe['title']),
        'parts': {slot: {'part': p['part'], 'weight': p['weight'], 'candidates': len(p['candidates'])}
                  for slot, p in recipe['parts'].items()},
        'gestalt_fragments': recipe['gestalt_fragments'], 'material': recipe['material'], 'notes': recipe['notes'],
        'card': {k: v for k, v in card.items() if k not in EXCLUDED_CARD_FIELDS},
    }


def ulps(a, b):
    """Distance in units in the last place between two doubles of the same sign."""
    def key(x):
        bits = struct.unpack('<q', struct.pack('<d', x))[0]
        return bits if bits >= 0 else -(bits & 0x7FFFFFFFFFFFFFFF)
    return abs(key(a) - key(b))


class Diff:
    def __init__(self):
        self.fields, self.mismatches, self.examples = 0, 0, []
        self.by_field = {}

    def compare(self, where, path, ours, theirs):
        """Leaf-by-leaf comparison; dict keys must match, list items compare in order."""
        if isinstance(ours, dict) and isinstance(theirs, dict):
            for key in sorted(set(ours) | set(theirs)):
                if key not in ours or key not in theirs:
                    self.record(where, f'{path}.{key}', ours.get(key, '<absent>'), theirs.get(key, '<absent>'), 'key')
                else:
                    self.compare(where, f'{path}.{key}', ours[key], theirs[key])
            return
        if isinstance(ours, list) and isinstance(theirs, list) and len(ours) == len(theirs):
            for i, (a, b) in enumerate(zip(ours, theirs)):
                self.compare(where, f'{path}[{i}]', a, b)
            return
        self.fields += 1
        if type(ours) is not type(theirs):
            self.record(where, path, ours, theirs, 'kind')
        elif isinstance(ours, float):
            if struct.pack('<d', ours) != struct.pack('<d', theirs) and not (math.isnan(ours) and math.isnan(theirs)):
                self.record(where, path, ours, theirs, f'{ulps(ours, theirs)} ulp')
        elif ours != theirs:
            self.record(where, path, ours, theirs, 'value')

    def record(self, where, path, ours, theirs, kind):
        self.mismatches += 1
        field = path.split('[')[0]
        self.by_field[field] = self.by_field.get(field, 0) + 1
        if len(self.examples) < 60:
            self.examples.append({'request': where, 'field': path, 'python': ours, 'cpp': theirs, 'kind': kind})


def expected_identity(path, stage, seed, stage_cap):
    """The identity fields ow-weapongen must echo for a gun rolled from `path` (a balance) at `stage` with `seed`."""
    if stage_cap is None:
        return {'balance': path, 'stage': stage, 'seed': seed, 'spawn_stage': None, 'stage_cap': None}
    return {'balance': path, 'stage': min(stage, stage_cap), 'seed': seed, 'spawn_stage': stage, 'stage_cap': stage_cap}


def check_identity(diff, where, cpp, expected):
    """Compare the echoed identity fields with the request, then drop them so the gun comparison sees only results."""
    echoed = {key: cpp.pop(key, None) for key in expected}
    diff.compare(where, 'identity', expected, echoed)


def verdict(report, requested_guns, requested_pools):
    """Problems that make the run fail (exit 1): mismatches, Python errors, fewer comparisons than requested,
    or C++ output that does not line up with the requests. An empty list means the run passed."""
    problems = []
    if report['mismatches']:
        problems.append(f"{report['mismatches']} mismatching field(s)")
    if report['python_errors']:
        problems.append(f"{report['python_errors']} Python error(s)")
    if report['guns_compared'] < requested_guns:
        problems.append(f"{report['guns_compared']} of {requested_guns} balance rolls compared")
    if report['pool_rolls_compared'] < requested_pools:
        problems.append(f"{report['pool_rolls_compared']} of {requested_pools} pool rolls compared")
    if report.get('output_problem'):
        problems.append(report['output_problem'])
    return problems


def pool_drops(package, values, pool, stage, seed):
    """Python CRT pool roll: a source (list/pawn) via roll_crt, a pool via roll_pool_crt."""
    rng = weapon_recipe.CrtRand(seed)
    cls = package.classes.get(pool, '')
    if cls.rsplit('.', 1)[-1] in weapon_gen_manifest.SOURCE_CLASSES:
        loot = loot_pools.table(package, values, pool, stage)
        return [d['item'] for d in loot_pools.roll_crt(loot, rng)]
    tree = loot_pools.expand(package, values, pool, stage)
    return [d['item'] for d in loot_pools.roll_pool_crt(tree, rng)]


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', default=str(ROOT / 'build/Release/ow-package.exe'))
    parser.add_argument('--weapongen', default=str(ROOT / 'build/Release/ow-weapongen.exe'))
    parser.add_argument('--game', type=Path, required=True, help='Borderlands 2 install folder')
    parser.add_argument('--manifest', type=Path, default=ROOT / 'local/weapons/gen/weapon_gen_manifest.txt')
    parser.add_argument('--seeds', type=int, default=20, help='seeds per balance and stage (1..N)')
    parser.add_argument('--pool-seeds', type=int, default=20)
    parser.add_argument('--limit', type=int, help='only the first N balances (quick runs)')
    parser.add_argument('--stage-cap', type=int, help='cap every gun\'s stage like a pool spawn (e.g. 50)')
    parser.add_argument('--output', type=Path, default=ROOT / 'local/weapons/gen/weapon_gen_parity.json')
    args = parser.parse_args()
    if not args.output.resolve().is_relative_to((ROOT / 'local').resolve()):
        parser.error('--output must stay under local/')
    started = time.time()
    package = weapon_gen_manifest.open_package(args.reader, args.game / 'WillowGame/CookedPCConsole/Startup.upk')
    localize = weapon_gen_manifest.make_localize(package, args.game)
    values = loot_pools.Values(package)
    balances = sorted(p for p, c in package.classes.items() if c in weapon_gen_manifest.BALANCE_CLASSES)[:args.limit]
    pools = sorted(p for p, c in package.classes.items()
                   if (weapon_gen_manifest.is_pool_class(c) and '.WeaponPools.' in p)
                   or c.rsplit('.', 1)[-1] == 'ItemPoolListDefinition')
    requests = [('balance', b, stage, seed) for b in balances for stage in STAGES for seed in range(1, args.seeds + 1)]
    requests += [('pool', p, stage, seed) for p in pools for stage in STAGES for seed in range(1, args.pool_seeds + 1)]

    batch = args.output.with_suffix('.batch.txt')
    batch.parent.mkdir(parents=True, exist_ok=True)
    cap = '' if args.stage_cap is None else f' {args.stage_cap}'
    batch.write_text(''.join(f'{k} {p} {s} {seed}{cap}\n' for k, p, s, seed in requests), encoding='utf-8')
    run = subprocess.run([args.weapongen, str(args.manifest), '--batch', str(batch)], capture_output=True, text=True,
                         encoding='utf-8')
    if run.returncode:
        raise SystemExit(f'ow-weapongen failed: {run.stderr.strip()}')
    cpp_lines = iter(run.stdout.splitlines())

    diff, errors = Diff(), []
    guns = pools_compared = pool_guns = 0
    output_problem = None
    try:
        for kind, path, stage, seed in requests:
            where = f'{kind} {path} stage {stage} seed {seed}'
            if kind == 'balance':
                cpp = json.loads(next(cpp_lines))
                try:
                    ours = python_gun(package, localize, path, stage, seed, args.stage_cap)
                except Exception as error:  # noqa: BLE001 - every Python failure is reported, none hidden
                    errors.append({'request': where, 'error': f'{type(error).__name__}: {error}'})
                    continue
                guns += 1
                check_identity(diff, where, cpp, expected_identity(path, stage, seed, args.stage_cap))
                diff.compare(where, 'gun', ours, cpp)
                continue
            header = json.loads(next(cpp_lines))
            diff.compare(where, 'pool.identity', {'pool': path, 'stage': stage, 'seed': seed},
                         {key: header.get(key) for key in ('pool', 'stage', 'seed')})
            cpp_guns = [json.loads(next(cpp_lines)) for d in header['drops']
                        if package.classes.get(d, '') in weapon_gen_manifest.BALANCE_CLASSES]
            try:
                drops = pool_drops(package, values, path, stage, seed)
            except Exception as error:  # noqa: BLE001
                errors.append({'request': where, 'error': f'{type(error).__name__}: {error}'})
                continue
            pools_compared += 1
            diff.compare(where, 'pool.drops', drops, header['drops'])
            if drops != header['drops']:
                continue
            weapons = [(k, d) for k, d in enumerate(drops) if package.classes.get(d, '') in weapon_gen_manifest.BALANCE_CLASSES]
            for (k, drop), cpp in zip(weapons, cpp_guns):
                try:
                    ours = python_gun(package, localize, drop, stage, seed + k, args.stage_cap)
                except Exception as error:  # noqa: BLE001
                    errors.append({'request': f'{where} drop {k}', 'error': f'{type(error).__name__}: {error}'})
                    continue
                pool_guns += 1
                check_identity(diff, f'{where} drop {k}', cpp, expected_identity(drop, stage, seed + k, args.stage_cap))
                diff.compare(f'{where} drop {k}', 'gun', ours, cpp)
    except StopIteration:
        output_problem = 'ow-weapongen printed fewer lines than the requests need'
    leftover = sum(1 for _ in cpp_lines)
    if leftover and not output_problem:
        output_problem = f'ow-weapongen printed {leftover} line(s) more than the requests need'

    report = {
        'balances': len(balances), 'stages': list(STAGES), 'stage_cap': args.stage_cap, 'seeds': args.seeds,
        'pools': len(pools),
        'pool_seeds': args.pool_seeds, 'guns_compared': guns, 'pool_rolls_compared': pools_compared,
        'pool_guns_compared': pool_guns, 'fields_compared': diff.fields, 'mismatches': diff.mismatches,
        'mismatches_by_field': diff.by_field, 'python_errors': len(errors),
        'excluded_card_fields': list(EXCLUDED_CARD_FIELDS), 'seconds': round(time.time() - started, 1),
        'examples': diff.examples, 'errors': errors[:40],
        'error_kinds': sorted({e['error'] for e in errors})[:20],
        'output_problem': output_problem,
    }
    args.output.write_text(json.dumps(report, indent=1, default=str), encoding='utf-8')
    print(json.dumps({k: v for k, v in report.items() if k not in ('examples', 'errors')}, indent=1, default=str))
    for example in diff.examples[:10]:
        print(json.dumps(example, default=str))
    problems = verdict(report, sum(1 for r in requests if r[0] == 'balance'), sum(1 for r in requests if r[0] == 'pool'))
    if problems:
        print('PARITY FAILED: ' + '; '.join(problems))
        sys.exit(1)
    print('PARITY PASSED')


if __name__ == '__main__':
    main()
