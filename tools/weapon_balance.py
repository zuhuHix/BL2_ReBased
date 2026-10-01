"""Legal parts of a BL2 weapon balance, and a crosscheck of our part-list merge.

`parts`: resolve a (Mission)WeaponBalanceDefinition through its BaseDefinition
chain with tools/weapon_recipe.py's merge and list, per slot, every legal part
with its weight and share of the slot. A slot with one candidate is fixed by
the balance; anything else is a roll.

`crosscheck`: the running game keeps the merged list of every balance in its
RuntimePartListCollection (not cooked: it is built when the game loads). The
OpenBLCMM object dumps (tools/blcmm_dumps.py, the game's own `obj dump` output,
local only) contain it. This compares, for every weapon balance in a package,
our merged slots with the game's: enabled flag, part order, and each entry's
stage and weight data (resolved through each list's own
ConsolidatedAttributeInitData, so differently numbered indices still compare).

Nothing here is copied into the repository; reports go under ignored local/.
"""
import argparse
import json
import re
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import weapon_recipe  # noqa: E402

BALANCE_CLASSES = ('WillowGame.WeaponBalanceDefinition', 'WillowGame.MissionWeaponBalanceDefinition')
INIT_DEFAULTS = {'BaseValueConstant': 0.0, 'BaseValueAttribute': None, 'InitializationDefinition': None,
                 'BaseValueScaleConstant': 1.0}


def legal_parts(package, balance, stage=None):
    """{balance, chain, weapon_type, manufacturer, slots: {slot: {fixed, candidates}}}.

    stage None keeps parts of every game stage. Shares are weight / slot total;
    an all-zero slot is reported with share None (the uniform pick in
    weapon_recipe.roll is UNVERIFIED).
    """
    result = weapon_recipe.merge(package, balance, stage)
    slots = {}
    for slot in weapon_recipe.SLOTS:
        entries = result['merged'].get(slot)
        if not entries:
            continue
        weights = [w for _, w in entries]
        total = sum(w for w in weights if w is not None)
        candidates = [{'part': part, 'weight': weight,
                       'share': (weight / total) if weight is not None and total > 0 else None}
                      for part, weight in entries]
        distinct = sorted({part for part, _ in entries})
        slots[slot] = {'fixed': len(distinct) == 1, 'all_zero_weight': total <= 0, 'candidates': candidates}
    return {'balance': balance, 'stage': stage, 'chain': result['chain'], 'weapon_type': result['weapon_type'],
            'manufacturer': result['manufacturer'], 'history': result['history'], 'slots': slots,
            'combinations': _product(len(s['candidates']) for s in slots.values())}


def _product(values):
    total = 1
    for v in values:
        total *= v
    return total


# ---------------------------------------------------------------- crosscheck with the game's runtime list

def normal_init(init):
    """AttributeInitializationData with defaults filled in, floats rounded (dump text has 6 decimals)."""
    merged = {**INIT_DEFAULTS, **{k: v for k, v in (init or {}).items() if k in INIT_DEFAULTS}}
    for key in ('BaseValueConstant', 'BaseValueScaleConstant'):
        merged[key] = round(float(merged[key] or 0.0), 5)
    for key in ('BaseValueAttribute', 'InitializationDefinition'):
        value = merged[key]
        merged[key] = value.replace(':', '.') if isinstance(value, str) else value
    return merged


def entry_key(part, min_init, max_init, weight_init, overrides):
    return json.dumps([part, min_init, max_init, weight_init, overrides], sort_keys=True)


def our_slots(package, collection):
    """{slot: (enabled, [entry_key])} for one cooked part list."""
    props = package.props(collection)
    consolidated = props.get('ConsolidatedAttributeInitData') or []

    def at(index):
        return normal_init(consolidated[index]) if 0 <= index < len(consolidated) else None
    slots = {}
    for slot in weapon_recipe.SLOTS:
        data = props.get(f'{slot}PartData')
        if not data or not data.get('bEnabled'):
            continue
        keys = []
        for part in data.get('WeightedParts') or []:
            overrides = [[o.get('Manufacturer'), at(o.get('DefaultWeightIndex', -1))]
                         for o in part.get('Manufacturers') or []]
            keys.append(entry_key(part['Part'], at(part.get('MinGameStageIndex', -1)),
                                  at(part.get('MaxGameStageIndex', -1)), at(part.get('DefaultWeightIndex', -1)),
                                  overrides))
        slots[slot] = keys
    return slots, props.get('PartReplacementMode') or 'EPRM_Additive'


def our_merge(package, balance):
    merged = {}
    for step in weapon_recipe.merge(package, balance, None)['chain']:
        collection = package.props(step).get('WeaponPartListCollection')
        if collection:
            slots, mode = our_slots(package, collection)
            merged = weapon_recipe.merge_slots(merged, slots, mode)
    return merged


DUMP_ENTRY = re.compile(r"\(Part=(?:\w+'([^']*)'|None),Manufacturers=(\(\(.*?\)\))?,"
                        r"MinGameStageIndex=(\d+),MaxGameStageIndex=(\d+),DefaultWeightIndex=(\d+)\)")
DUMP_OVERRIDE = re.compile(r"\(Manufacturer=(?:\w+'([^']*)'|None),DefaultWeightIndex=(\d+)\)")
DUMP_INIT = re.compile(r"BaseValueConstant=([-\d.]+),BaseValueAttribute=(?:\w+'([^']*)'|None),"
                       r"InitializationDefinition=(?:\w+'([^']*)'|None),BaseValueScaleConstant=([-\d.]+)")


def runtime_slots(text):
    """Parse a dumped WeaponPartListCollectionDefinition into {slot: [entry_key]} (enabled slots only)."""
    lines = {}
    consolidated = {}
    for line in text.splitlines():
        key, _, value = line.partition('=')
        match = re.fullmatch(r'ConsolidatedAttributeInitData\((\d+)\)', key)
        if match:
            init = DUMP_INIT.search(value)
            consolidated[int(match.group(1))] = normal_init({
                'BaseValueConstant': float(init.group(1)), 'BaseValueAttribute': init.group(2),
                'InitializationDefinition': init.group(3), 'BaseValueScaleConstant': float(init.group(4))})
        elif key.endswith('PartData'):
            lines[key[:-len('PartData')]] = value

    def at(index):
        return consolidated.get(int(index))
    slots = {}
    for slot, value in lines.items():
        if not value.startswith('(bEnabled=True'):
            continue
        keys = []
        for part, overrides, low, high, weight in DUMP_ENTRY.findall(value):
            keys.append(entry_key(part.replace(':', '.') or None, at(low), at(high), at(weight),
                                  [[maker.replace(':', '.') or None, at(index)]
                                   for maker, index in DUMP_OVERRIDE.findall(overrides or '')]))
        slots[slot] = keys
    return slots


def crosscheck(package, dumps, balances):
    """Per balance: 'match', or the differing slots; plus totals."""
    report = {'balances': len(balances), 'match': 0, 'differ': 0, 'no_dump': 0, 'slots_compared': 0,
              'entries_compared': 0, 'details': {}}
    for balance in balances:
        dumped = dumps.dump(balance)
        runtime = (dumped or {}).get('properties', {}).get('RuntimePartListCollection')
        text = dumps.text(runtime['path']) if isinstance(runtime, dict) else None
        if not text:
            report['no_dump'] += 1
            report['details'][balance] = 'no runtime part list in the dumps'
            continue
        ours, theirs = our_merge(package, balance), runtime_slots(text)
        differences = {}
        for slot in sorted(set(ours) | set(theirs)):
            a, b = ours.get(slot), theirs.get(slot)
            report['slots_compared'] += 1
            report['entries_compared'] += len(b or [])
            if a != b:
                differences[slot] = {'ours': [json.loads(k)[0] for k in a or []],
                                     'game': [json.loads(k)[0] for k in b or []],
                                     'same_parts_in_order': [json.loads(k)[0] for k in a or []]
                                     == [json.loads(k)[0] for k in b or []]}
        if differences:
            report['differ'] += 1
            report['details'][balance] = differences
        else:
            report['match'] += 1
    return report


def open_package(reader, path, schema_path):
    schema_path.parent.mkdir(parents=True, exist_ok=True)
    schema_path.write_text('\n'.join(weapon_recipe.SCHEMA_LINES) + '\n', encoding='utf-8')
    return weapon_recipe.Package(str(Path(reader).resolve()), Path(path), str(schema_path.resolve()))


def preload_balances(package, balances):
    """Batch-decode balances, their chains and their part lists (speed only)."""
    package.crawl(balances, follow=lambda path, cls: cls in BALANCE_CLASSES
                  or cls == 'WillowGame.WeaponPartListCollectionDefinition')


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', required=True)
    parser.add_argument('--package', required=True, help='e.g. .../CookedPCConsole/Startup.upk')
    sub = parser.add_subparsers(dest='command', required=True)
    parts = sub.add_parser('parts', help='legal parts of one balance')
    parts.add_argument('balance')
    parts.add_argument('--stage', type=float, help='filter by game stage (default: every stage)')
    check = sub.add_parser('crosscheck', help='compare merges with the game runtime lists (OpenBLCMM dumps)')
    check.add_argument('--balance', action='append', help='only these balances (default: all in the package)')
    parser.add_argument('--output', type=Path, help='write JSON here (under local/)')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    package = open_package(args.reader, args.package, root / 'local/weapon_balance/weapon_balance.schema')
    started = time.perf_counter()
    if args.command == 'parts':
        preload_balances(package, [args.balance])
        result = legal_parts(package, args.balance, args.stage)
    else:
        import blcmm_dumps
        balances = args.balance or sorted(p for p, c in package.classes.items() if c in BALANCE_CLASSES)
        preload_balances(package, balances)
        result = crosscheck(package, blcmm_dumps.Dumps(), balances)
        result['elapsed_seconds'] = round(time.perf_counter() - started, 1)
    text = json.dumps(result, indent=1)
    if args.output:
        if not args.output.resolve().is_relative_to(root / 'local'):
            parser.error('--output must stay under local/')
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(text, encoding='utf-8')
        summary = {k: v for k, v in result.items() if k not in ('details', 'slots', 'history', 'chain')}
        print(json.dumps(summary, indent=1))
    else:
        print(text)


if __name__ == '__main__':
    main()
