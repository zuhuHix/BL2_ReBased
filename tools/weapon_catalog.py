"""Census of Borderlands 2's weapon-generation objects, per package and per DLC, with cross-checks.

Subcommands (run from the repository root):
- packages: `ow-package --exports` on every .upk under WillowGame\\CookedPCConsole (base) and
  DLC\\<name>\\ (one DLC per folder). Weapon objects are decoded in batches through weapon_recipe.Package and
  written to local/weapons/catalog/package_catalog.json: counts per class, package and DLC; each part with its
  slot, manufacturer, gestalt fragments, material instances and the balances whose part lists name it; each
  balance with its parts per slot; pools that reach weapon balances.
- crosscheck: compares the package census with the OpenBLCMM object list (names only, from the data.db that
  tools/blcmm_dumps.py reads) and with the live catalog written by
  tools/real_game/scripts/weapon_parts_census.py (local/realgame/weapons/live_catalog.json). Writes
  local/weapons/catalog/coverage.json: counts per source, only-in-X lists and the parts no balance lists.
- summary: counts-only tables from the two files, for a docs record (no object names).

Rules used (each is an assumption about the data, not a game rule):
- A part's slot is taken from the <Slot>PartData list that names it when exactly one slot does; otherwise from
  PartType (WP_<Slot>), otherwise from the path's second segment (GD_Weap_X.<Slot>.Name). Accessory1 and
  Accessory2 are reported as one group, 'Accessory'.
- A part's manufacturer is the one manufacturer of the balances that list it; if none lists it, the part's
  ManufacturerDefinitionForScopeMovie field; otherwise unknown (candidates kept).
- A balance's parts are the union of the part lists of its BaseDefinition chain. This is not the merged
  candidate set of weapon_recipe.merge (that needs the game's merge rules); disabled slots are included and
  flagged.
- Fragments and materials use weapon_recipe.part_fragments and the material instances a part references.

Nothing here writes into the repository; all output goes under the ignored local/.
"""
import argparse
import concurrent.futures
import datetime
import json
import os
import re
import sqlite3
import subprocess
import sys
import time
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(Path(__file__).resolve().parent))
import loot_pools  # noqa: E402  (SCHEMA_LINES: weapon_recipe's schema plus pool and balance arrays)
import weapon_recipe  # noqa: E402

OUT = ROOT / 'local/weapons/catalog'
LIVE_CATALOG = ROOT / 'local/realgame/weapons/live_catalog.json'
DEFAULT_READER = ROOT / 'build/Release/ow-package.exe'
DEFAULT_GAME = Path(os.environ.get('OPENWILLOW_BL2', r'C:\Program Files (x86)\Steam\steamapps\common\Borderlands 2'))
DEFAULT_DB = Path(os.environ.get('LOCALAPPDATA', '')) / 'OpenBLCMM' / 'extracted-data' / 'BL2' / 'data.db'

SLOT_GROUPS = ('Body', 'Grip', 'Barrel', 'Sight', 'Stock', 'Elemental', 'Accessory', 'Material')
CATEGORY_NAMES = ('balance', 'part', 'weapon_type', 'manufacturer', 'name_part', 'part_list',
                  'part_list_definition', 'gestalt', 'item_pool', 'item_pool_list')
# Keys are the unqualified class name, so the same rule applies to exports ("WillowGame.X") and OpenBLCMM names.
EXACT_CATEGORY = {
    'WeaponPartDefinition': 'part', 'ManufacturerDefinition': 'manufacturer',
    'WeaponNamePartDefinition': 'name_part', 'WeaponPartListCollectionDefinition': 'part_list',
    'WeaponPartListDefinition': 'part_list_definition', 'GestaltSkeletalMeshDefinition': 'gestalt',
}


def category(cls):
    """Catalog category of a class name (qualified or not), or None when the class is not weapon data."""
    name = cls.rsplit('.', 1)[-1]
    if name.endswith('WeaponBalanceDefinition'):
        return 'balance'
    if name.endswith('WeaponTypeDefinition'):
        return 'weapon_type'
    if name.endswith('ItemPoolListDefinition'):
        return 'item_pool_list'
    if name.endswith('ItemPoolDefinition'):
        return 'item_pool'
    return EXACT_CATEGORY.get(name)


def short(cls):
    """Class name without its package ('Engine.ManufacturerDefinition' -> 'ManufacturerDefinition')."""
    return cls.rsplit('.', 1)[-1]


def is_class_default(path):
    """Class-default objects (Default__X, also Default__X.PartList) are in the exports but are not game objects."""
    return any(segment.startswith('Default__') for segment in path.split('.'))


def is_runtime_list(name):
    """OpenBLCMM's numbered per-balance lists (Balance.WeaponPartListCollectionDefinition_220): made at load time."""
    return re.search(r'\.WeaponPartListCollectionDefinition_\d+$', name) is not None


def slot_group(slot):
    """Accessory1 and Accessory2 are one group in reports; other slots are unchanged."""
    return 'Accessory' if slot and slot.startswith('Accessory') else slot


def slot_from_part_type(part_type):
    """'WP_Barrel' -> 'Barrel'; None when the value is not a slot name."""
    if not part_type or not part_type.startswith('WP_'):
        return None
    name = slot_group(part_type[3:])
    return name if name in SLOT_GROUPS else None


def slot_from_path(path):
    """GD_Weap_AssaultRifle.Barrel.AR_Barrel_Dahl -> 'Barrel' (None when the second segment is not a slot)."""
    parts = path.split('.')
    return parts[1] if len(parts) > 2 and parts[1] in SLOT_GROUPS else None


def choose_slot(listed, part_type_slot, path_slot):
    """(slot, source) for a part. `listed` is the set of slot groups whose part list names the part."""
    if len(listed) == 1:
        return next(iter(listed)), 'part_list'
    for value, source in ((part_type_slot, 'part_type'), (path_slot, 'path')):
        if value and (not listed or value in listed):
            return value, source
    return None, 'unknown'


def choose_manufacturer(balance_makers, scope_field):
    """(manufacturer, source, candidates): the single manufacturer of the listing balances, else the scope field."""
    makers = sorted({m for m in balance_makers if m})
    if len(makers) == 1:
        return makers[0], 'balances', makers
    if not makers and scope_field:
        return scope_field, 'scope_movie_field', []
    return None, ('ambiguous' if makers else 'unknown'), makers


def is_material_instance(cls):
    return 'MaterialInstance' in cls.rsplit('.', 1)[-1]


def reach_balances(pool, children, balances, _seen=None):
    """Weapon balances a pool reaches directly or through sub-pools. children: {pool: [child paths]}."""
    seen = set() if _seen is None else _seen
    found = set()
    for child in children.get(pool, []):
        if child in seen:
            continue
        seen.add(child)
        if child in balances:
            found.add(child)
        else:
            found |= reach_balances(child, children, balances, seen)
    return found


def pool_children(props):
    """Direct children of an item pool (BalancedItems) or item pool list (ItemPools)."""
    kids = [e.get('ItmPoolDefinition') or e.get('InvBalanceDefinition') for e in props.get('BalancedItems') or []]
    kids += [e.get('ItemPool') for e in props.get('ItemPools') or []]
    return [k for k in kids if k]


def compare(first, second):
    """Two sets of object names compared: counts and sorted only-in lists."""
    return {'first': len(first), 'second': len(second), 'both': len(first & second),
            'only_first': sorted(first - second), 'only_second': sorted(second - first)}


def unlisted(parts, listed):
    """Parts that no balance lists (sorted)."""
    return sorted(set(parts) - set(listed))


def package_files(game):
    """(file, dlc) for every package: base game first, then each DLC folder."""
    base = game / 'WillowGame' / 'CookedPCConsole'
    dlc_root = game / 'DLC'
    files = [(p, 'base') for p in sorted(base.rglob('*.upk'))]
    files += [(p, p.relative_to(dlc_root).parts[0]) for p in sorted(dlc_root.rglob('*.upk'))]
    return files


def scan(reader, file):
    """ow-package --exports for one package: ([(path, class)], error text or None)."""
    try:
        out = subprocess.run([str(reader), str(file), '--exports'], capture_output=True, text=True, check=True).stdout
    except subprocess.CalledProcessError as error:
        return [], error.stderr.strip() or f'exit {error.returncode}'
    return [(e['path'], e['class']) for e in json.loads(out)], None


class Census:
    """Owner package of every export, and decoded properties of the weapon objects through weapon_recipe.Package."""

    def __init__(self, reader, schema):
        self.reader, self.schema = str(reader), str(schema)
        self.classes = {}  # every export path -> class (first owner wins; needed to classify references)
        self.owner = {}  # weapon object path -> package file
        self.packages = {}  # package file -> weapon_recipe.Package, opened on demand
        self.duplicates = 0
        self.conflicts = []  # same path, different weapon category in another package (first owner kept)

    def add(self, path, cls, file):
        if is_class_default(path):
            return
        if path in self.classes:
            self.duplicates += 1
            if category(cls) and category(cls) != category(self.classes[path]):
                self.conflicts.append({'path': path, 'first': self.classes[path], 'second': cls})
            return
        self.classes[path] = cls
        if category(cls):
            self.owner[path] = file

    def package(self, file):
        if file not in self.packages:
            self.packages[file] = weapon_recipe.Package(self.reader, file, self.schema)
        return self.packages[file]

    def props(self, path):
        return self.package(self.owner[path]).props(path)

    def preload(self, paths):
        groups = defaultdict(list)
        for path in paths:
            if path in self.owner:
                groups[self.owner[path]].append(path)
        for file, group in groups.items():
            self.package(file).preload(group)


def chain(census, balance):
    """The balance and its BaseDefinition ancestors (root last); guards against cycles."""
    out, cursor = [], balance
    while cursor and cursor not in out and cursor in census.owner:
        out.append(cursor)
        cursor = census.props(cursor).get('BaseDefinition')
    return out


def balance_listing(census, balance):
    """{raw slot name: [part paths]} from every WeaponPartListCollection in the balance's chain (enabled or not)."""
    listing = defaultdict(list)
    for step in chain(census, balance):
        collection = census.props(step).get('WeaponPartListCollection')
        if not collection or collection not in census.owner:
            continue
        props = census.props(collection)
        for key, value in props.items():
            if not key.endswith('PartData') or not isinstance(value, dict):
                continue
            raw = key[:-len('PartData')]
            for entry in value.get('WeightedParts') or []:
                if entry.get('Part'):
                    listing[raw].append(entry['Part'])
    return listing


def build(census, game):
    """Structured records for every weapon object the census can decode."""
    targets = sorted(census.owner)
    census.preload(targets)
    by_category = defaultdict(list)
    for path in targets:
        by_category[category(census.classes[path])].append(path)

    balances = {}
    listing = defaultdict(lambda: {'slots': set(), 'balances': set()})
    for path in by_category['balance']:
        props_chain = [census.props(step) for step in chain(census, path)]
        makers = next((p.get('Manufacturers') for p in props_chain if p.get('Manufacturers')), [])
        weapon_type = next((p.get('InventoryDefinition') for p in props_chain if p.get('InventoryDefinition')), None)
        slots = {}
        for raw, parts in balance_listing(census, path).items():
            group = slot_group(raw)
            slots.setdefault(group, set()).update(parts)
            for part in parts:
                listing[part]['slots'].add(group)
                listing[part]['balances'].add(path)
        balances[path] = {
            'class': census.classes[path], 'package': census.owner[path].relative_to(game).as_posix(),
            'manufacturer': makers[0].get('Manufacturer') if makers else None, 'weapon_type': weapon_type,
            'slots': {slot: sorted(parts) for slot, parts in sorted(slots.items())}}

    balance_makers = {b: r['manufacturer'] for b, r in balances.items()}
    parts = []
    for path in by_category['part']:
        props = census.props(path)
        listed = listing[path]['slots']
        listing_balances = sorted(listing[path]['balances'])
        slot, slot_source = choose_slot(listed, slot_from_part_type(props.get('PartType')), slot_from_path(path))
        maker, maker_source, candidates = choose_manufacturer(
            [balance_makers.get(b) for b in listing_balances], props.get('ManufacturerDefinitionForScopeMovie'))
        package = census.package(census.owner[path])
        refs = list(weapon_recipe.references(props))
        parts.append({
            'path': path, 'class': census.classes[path], 'slot': slot, 'slot_source': slot_source,
            'part_type': props.get('PartType'), 'manufacturer': maker, 'manufacturer_source': maker_source,
            'manufacturer_candidates': candidates,
            'fragments': weapon_recipe.part_fragments(package, path),
            'materials': sorted({r for r in refs if is_material_instance(census.classes.get(r, ''))}),
            'balances': listing_balances,
            'package': census.owner[path].relative_to(game).as_posix()})

    weapon_types = [{'path': path, 'class': census.classes[path],
                     'gestalts': sorted({r for r in weapon_recipe.references(census.props(path))
                                         if category(census.classes.get(r, '')) == 'gestalt'}),
                     'package': census.owner[path].relative_to(game).as_posix()}
                    for path in by_category['weapon_type']]

    pools = {}
    children = {}
    for path in by_category['item_pool'] + by_category['item_pool_list']:
        children[path] = pool_children(census.props(path))
    for path in by_category['item_pool'] + by_category['item_pool_list']:
        reached = reach_balances(path, children, set(balances))
        if reached:
            pools[path] = {'class': census.classes[path], 'package': census.owner[path].relative_to(game).as_posix(),
                           'weapon_balances': sorted(reached)}

    listed_parts = {part for part, record in listing.items() if record['balances']}
    part_paths = {p['path'] for p in parts}
    return {
        'objects': {cat: sorted(paths) for cat, paths in by_category.items() if cat},
        'parts': parts, 'balances': balances, 'pools': pools,
        'weapon_types': weapon_types,
        'manufacturers': sorted(by_category['manufacturer']),
        'name_parts': [{'path': p, 'kind': name_kind(p)} for p in by_category['name_part']],
        'part_lists': sorted(by_category['part_list']),
        'part_list_definitions': sorted(by_category['part_list_definition']),
        'gestalts': sorted(by_category['gestalt']),
        'listed_not_in_census': sorted(listed_parts - part_paths),
        'parts_unlisted': unlisted(part_paths, listed_parts),
    }


def name_kind(path):
    """Prefix or title name part, from the object's own name."""
    name = path.rsplit('.', 1)[-1]
    if name.startswith('Prefix'):
        return 'prefix'
    if name.startswith('Title'):
        return 'title'
    return 'other'


def cmd_packages(args):
    game = Path(args.game)
    files = package_files(game)
    started = time.perf_counter()
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
        scanned = list(pool.map(lambda item: (item, scan(args.reader, item[0])), files))
    OUT.mkdir(parents=True, exist_ok=True)
    schema = OUT / 'weapon_catalog.schema'
    schema.write_text('\n'.join(loot_pools.SCHEMA_LINES) + '\n', encoding='utf-8')
    census = Census(args.reader, schema.resolve())
    by_package, dlc_of, errors = {}, {}, {}
    for (file, dlc), (exports, error) in scanned:
        rel = file.relative_to(game).as_posix()
        dlc_of[rel] = dlc
        if error:
            errors[rel] = error
            continue
        counts = Counter(short(cls) for path, cls in exports if category(cls) and not is_class_default(path))
        for path, cls in exports:
            census.add(path, cls, file)
        if counts:
            by_package[rel] = {'dlc': dlc, 'counts': dict(sorted(counts.items()))}
    records = build(census, game)

    per_class = Counter(short(census.classes[p]) for p in census.owner)
    per_dlc = defaultdict(Counter)
    per_dlc_category = defaultdict(Counter)
    for path, file in census.owner.items():
        dlc = dlc_of[file.relative_to(game).as_posix()]
        per_dlc[dlc][short(census.classes[path])] += 1
        per_dlc_category[dlc][category(census.classes[path])] += 1
    totals_by_category = Counter()
    for cls, n in per_class.items():
        totals_by_category[category(cls)] += n
    parts_by_slot = Counter(p['slot'] or 'unknown' for p in records['parts'])
    parts_by_manufacturer = Counter(p['manufacturer'] or 'unknown' for p in records['parts'])
    parts_by_dlc_slot = defaultdict(Counter)
    for p in records['parts']:
        parts_by_dlc_slot[dlc_of[p['package']]][p['slot'] or 'unknown'] += 1
    catalog = {
        'owner_dlc': {path: dlc_of[file.relative_to(game).as_posix()] for path, file in census.owner.items()},
        'generated': datetime.datetime.now().isoformat(timespec='seconds'), 'game': str(game),
        'packages_scanned': len(files), 'packages_with_weapon_data': len(by_package), 'scan_errors': errors,
        'duplicate_export_paths_skipped_all_classes': census.duplicates,
        'duplicate_paths_with_other_weapon_class': census.conflicts, 'seconds': round(time.perf_counter() - started, 1),
        'totals_by_class': dict(sorted(per_class.items())),
        'totals_by_category': totals_by_category,
        'by_dlc': {dlc: dict(sorted(c.items())) for dlc, c in sorted(per_dlc.items())},
        'by_dlc_category': {dlc: dict(sorted(c.items())) for dlc, c in sorted(per_dlc_category.items())},
        'by_package': dict(sorted(by_package.items())),
        'parts_by_slot': dict(sorted(parts_by_slot.items())),
        'parts_by_manufacturer': dict(sorted(parts_by_manufacturer.items())),
        'parts_by_dlc_slot': {dlc: dict(sorted(c.items())) for dlc, c in sorted(parts_by_dlc_slot.items())},
        'parts_unlisted_count': len(records['parts_unlisted']),
        'listed_not_in_census_count': len(records['listed_not_in_census']),
        **records,
    }
    out = OUT / 'package_catalog.json'
    out.write_text(json.dumps(catalog, indent=1, ensure_ascii=False), encoding='utf-8')
    print(json.dumps({'packages_scanned': catalog['packages_scanned'], 'with_weapon_data': catalog['packages_with_weapon_data'],
                      'errors': len(errors), 'totals_by_class': catalog['totals_by_class'],
                      'seconds': catalog['seconds'], 'wrote': str(out)}, indent=1))
    return 0


def openblcmm_objects(db):
    """({category: set of object names}, {category: runtime count}) from OpenBLCMM's object table (names only)."""
    connection = sqlite3.connect(str(db))
    try:
        found, runtime = defaultdict(set), Counter()
        names = [name for (name,) in connection.execute('select name from class') if category(name)]
        for obj, cls in connection.execute(
                f"select o.name, c.name from object o join class c on c.id = o.class "
                f"where c.name in ({','.join('?' * len(names))})", names):
            if is_class_default(obj):
                continue
            if obj.startswith('Transient.') or is_runtime_list(obj.replace(':', '.')):  # made while the game runs
                runtime[category(cls)] += 1
                continue
            found[category(cls)].add(obj.replace(':', '.'))  # OpenBLCMM writes sub-objects as Owner:Name
        return found, runtime
    finally:
        connection.close()


def live_objects(path):
    """({category: set of object paths}, {category: runtime count}, state) from weapon_parts_census.py's output."""
    data = json.loads(Path(path).read_text(encoding='utf-8'))
    found, runtime = defaultdict(set), Counter()
    for obj, record in data['objects'].items():
        cat = category(record['class'])
        if not cat or is_class_default(obj):
            continue
        name = obj.replace(':', '.')  # the game writes sub-objects as Owner:Name
        if obj.startswith('Transient.') or is_runtime_list(name):
            runtime[cat] += 1
            continue
        found[cat].add(name)
    return found, runtime, data.get('state')


def cmd_crosscheck(args):
    catalog = json.loads((OUT / 'package_catalog.json').read_text(encoding='utf-8'))
    package = {cat: set(paths) for cat, paths in catalog['objects'].items()}
    dlc_of = catalog['owner_dlc']
    blcmm, blcmm_runtime = openblcmm_objects(args.db)
    live, live_runtime, live_state = live_objects(args.live) if args.live.exists() else (None, None, None)
    per_category, lists = {}, {}
    for cat in CATEGORY_NAMES:
        first, second = package.get(cat, set()), blcmm.get(cat, set())
        cmp = compare(first, second)
        entry = {'package': cmp['first'], 'openblcmm': cmp['second'], 'both_package_openblcmm': cmp['both'],
                 'package_not_openblcmm': len(cmp['only_first']), 'openblcmm_not_package': len(cmp['only_second'])}
        lists[cat] = {'package_not_openblcmm': cmp['only_first'], 'openblcmm_not_package': cmp['only_second']}
        if live is not None:
            cmp_live = compare(first, live.get(cat, set()))
            entry.update(live=cmp_live['second'], both_package_live=cmp_live['both'],
                         package_not_live=len(cmp_live['only_first']), live_not_package=len(cmp_live['only_second']),
                         live_runtime_instances_not_compared=live_runtime.get(cat, 0),
                         package_not_live_by_dlc=dict(sorted(Counter(
                             dlc_of.get(p, '?') for p in cmp_live['only_first']).items())))
            lists[cat].update(package_not_live=cmp_live['only_first'], live_not_package=cmp_live['only_second'])
        per_category[cat] = entry
    package_union = {b: {p for paths in rec['slots'].values() for p in paths} for b, rec in catalog['balances'].items()}
    runtime_check = None
    if live is not None:
        live_records = json.loads(args.live.read_text(encoding='utf-8'))['objects']
        runtime = {b: {p for paths in rec['runtime_parts'].values() for p in paths}
                   for b, rec in live_records.items() if rec.get('runtime_parts') is not None}
        both = [b for b in runtime if b in package_union]
        runtime_check = {
            'balances_compared': len(both),
            'identical_sets': sum(1 for b in both if runtime[b] == package_union[b]),
            'runtime_part_not_in_package_union': sum(1 for b in both if runtime[b] - package_union[b]),
            'package_union_not_in_runtime': sum(1 for b in both if package_union[b] - runtime[b]),
            'lists': {b: {'runtime_only': sorted(runtime[b] - package_union[b]),
                          'package_only': sorted(package_union[b] - runtime[b])}
                      for b in both if runtime[b] != package_union[b]}}
    unlisted_parts = [p['path'] for p in catalog['parts'] if not p['balances']]
    unlisted_by_package = Counter(p['package'] for p in catalog['parts'] if not p['balances'])
    coverage = {
        'generated': datetime.datetime.now().isoformat(timespec='seconds'),
        'live_state': live_state, 'live_catalog': str(args.live) if live is not None else None,
        'per_category': per_category,
        'openblcmm_runtime_instances_not_compared': dict(blcmm_runtime),
        'parts_unlisted_by_any_balance': {'count': len(unlisted_parts),
                                          'by_package': dict(sorted(unlisted_by_package.items()))},
        'listed_parts_not_in_census': catalog['listed_not_in_census_count'],
        'runtime_part_lists': runtime_check,
        'lists': {'parts_unlisted_by_any_balance': unlisted_parts, **lists},
    }
    OUT.mkdir(parents=True, exist_ok=True)
    out = OUT / 'coverage.json'
    out.write_text(json.dumps(coverage, indent=1, ensure_ascii=False), encoding='utf-8')
    print(json.dumps({'per_category': per_category, 'wrote': str(out),
                      'runtime_part_lists': {k: v for k, v in (runtime_check or {}).items() if k != 'lists'}},
                     indent=1))
    return 0


def table(rows, header):
    widths = [max(len(str(r[i])) for r in rows + [header]) for i in range(len(header))]
    line = '| ' + ' | '.join(str(h).ljust(w) for h, w in zip(header, widths)) + ' |'
    rule = '|' + '|'.join('-' * (w + 2) for w in widths) + '|'
    body = ['| ' + ' | '.join(str(v).ljust(w) for v, w in zip(r, widths)) + ' |' for r in rows]
    return '\n'.join([line, rule] + body)


def cmd_summary(args):
    catalog = json.loads((OUT / 'package_catalog.json').read_text(encoding='utf-8'))
    print('Objects per class (unique, owner package):\n')
    print(table(sorted(catalog['totals_by_class'].items()), ['class', 'count']))
    print('\nPer DLC (objects owned by the DLC packages, by category):\n')
    cats = sorted(catalog['totals_by_category'])
    print(table([[d] + [catalog['by_dlc_category'][d].get(c, 0) for c in cats]
                 for d in sorted(catalog['by_dlc_category'])], ['dlc'] + cats))
    print('\nParts per slot (unknown = not derivable):\n')
    print(table(sorted(catalog['parts_by_slot'].items()), ['slot', 'parts']))
    print('\nParts per manufacturer (unknown = not derivable):\n')
    by_maker = Counter()
    for maker, n in catalog['parts_by_manufacturer'].items():
        by_maker[short(maker) if maker != 'unknown' else maker] += n
    print(table(sorted(by_maker.items()), ['manufacturer', 'parts']))
    print(f"\nPackages scanned: {catalog['packages_scanned']}, with weapon data: {catalog['packages_with_weapon_data']}, "
          f"scan errors: {len(catalog['scan_errors'])}, duplicate export paths skipped (all classes): {catalog['duplicate_export_paths_skipped_all_classes']}, "
          f"duplicate paths with another weapon class: {len(catalog['duplicate_paths_with_other_weapon_class'])}")
    path = OUT / 'coverage.json'
    if path.exists():
        coverage = json.loads(path.read_text(encoding='utf-8'))
        columns = ['package', 'openblcmm', 'live', 'package_not_openblcmm', 'openblcmm_not_package',
                   'package_not_live', 'live_not_package']
        cats = coverage['per_category']
        print('\nCoverage per category (counts; "-" = that source was not run):\n')
        print(table([[c] + [cats[c].get(k, '-') for k in columns] for c in cats], ['category'] + columns))
        print(f"\nParts no balance lists: {coverage['parts_unlisted_by_any_balance']['count']}")
        runtime = coverage.get('runtime_part_lists')
        if runtime:
            print(f"Live balances compared with package part sets: {runtime['balances_compared']}, "
                  f"identical: {runtime['identical_sets']}")
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest='command', required=True)
    walk = sub.add_parser('packages', help='census of every package and DLC')
    walk.add_argument('--reader', default=str(DEFAULT_READER))
    walk.add_argument('--game', default=str(DEFAULT_GAME))
    walk.add_argument('--jobs', type=int, default=8)
    check = sub.add_parser('crosscheck', help='compare with OpenBLCMM names and the live catalog')
    check.add_argument('--db', type=Path, default=DEFAULT_DB)
    check.add_argument('--live', type=Path, default=LIVE_CATALOG)
    sub.add_parser('summary', help='counts-only tables from the written JSON')
    args = parser.parse_args()
    if args.command == 'packages':
        return cmd_packages(args)
    if args.command == 'crosscheck':
        return cmd_crosscheck(args)
    return cmd_summary(args)


if __name__ == '__main__':
    sys.exit(main())

