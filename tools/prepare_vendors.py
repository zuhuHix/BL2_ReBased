"""Prepare the vending machines of a map for the vendors slice (local/p2/vendors/spec.md, section A).

Reads the installed packages with ow-package and writes one JSON document (schema openwillow-vendors/1) under the
ignored local/slice/ folder (game data, docs/LEGAL.md). It lists every vending machine: each
WillowPopulationOpportunityPoint whose PopulationDef is a GD_Population_Shopping.VendingMachine.* population.

How a machine is read (each step follows references; nothing is looked up by a remembered offset):
- the point's PopulationDef -> the population factory exported under it (WillowGame.PopulationFactoryVendingMachine or
  ...PopulationFactoryBlackMarket);
- the factory's ShopType: absent = weapons, SType_Items = items, SType_Health = health; the black-market factory =
  blackmarket, marked supported false (its stock is not modelled in this slice);
- the factory's ObjectBalanceDefinition -> DefaultLoot: the "Inventory" configuration gives the stock pools and the
  "FeaturedItem" configuration the featured pools; each ItemAttachments entry is an ItemPool with a PoolProbability;
- each pool is expanded with tools/loot_pools.py (expand and flatten): the leaf balances with their summed share of the
  pool, every quantity and gate evaluated at --stage. Each leaf keeps its class (WeaponBalanceDefinition or other);
- object paths resolve in the point's own level package, then the map package, then Startup.upk;
- minutes_between_resets is the WillowGame GlobalsDefinition class default (MinutesBetweenShopResets).

What is not read here (UNVERIFIED in game):
- markup and featured_markup (GD_Economy.VendingMachine.Init_MarkupCalc_P1 and the featured item's CommerceMarkup) need
  runtime attribute values the packages do not hold; they are null where they do not resolve. The host keeps its own
  labelled placeholder until a live read;
- fixed_item_cost and fixed_featured_cost are 0: the factories name no fixed cost;
- a pool probability that depends on a runtime attribute is null and listed in unresolved;
- which pool entries the game rolls, and how, is not confirmed (see tools/loot_pools.py).
"""
import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from loot_pools import SCHEMA_LINES, Values, expand, flatten  # noqa: E402
from prepare_slice_world import Reader, levels, vector  # noqa: E402

REPO = Path(__file__).resolve().parents[1]
SCHEMA_PATH = REPO / 'local' / 'slice' / 'vendors.schema'
# The balance's DefaultLoot and its attachments are structs the shared loot schema does not name yet.
SCHEMA_EXTRA = [
    'DefaultLoot=StructProperty:LootConfigurationData',
    'ItemAttachments=StructProperty:LootAttachmentData',
]
SCHEMA_TAG = 'openwillow-vendors/1'
POPULATION = 'GD_Population_Shopping.VendingMachine.'
SHOP_KINDS = {'SType_Items': 'items', 'SType_Health': 'health'}  # an absent ShopType is the weapons machine
# Titles of the three Sanctuary machine kinds (the VendingMachineExGFxMovie titles named in the spec); the black
# market has no title here, so its status-menu header is used.
TITLES = {'weapons': 'Marcus Munitions', 'items': 'The Ammo Dump', 'health': "Dr. Zed's Meds"}


def plain_refs(value):
    """Reader values -> plain Python values: an object reference ({index, path}) becomes its path (or None)."""
    if isinstance(value, list):
        return [plain_refs(v) for v in value]
    if isinstance(value, dict):
        if set(value) == {'index', 'path'}:
            return value['path']
        return {k: plain_refs(v) for k, v in value.items()}
    return value


def local_path(value, repo=REPO):
    """`value` resolved (relative paths against the repository root) and refused unless it lies inside the repository's
    ignored local/ (local/ itself is refused too). Everything this tool writes is game data (docs/LEGAL.md)."""
    local = (Path(repo) / 'local').resolve()
    path = Path(value)
    path = (path if path.is_absolute() else Path(repo) / path).resolve()
    if path == local or local not in path.parents:
        raise ValueError(f'{value}: output must be inside {local}; game data stays in local/')
    return path


class Chain:
    """Object lookups over the level's package, the map package and Startup.upk; the first package holding a path wins.

    Values are plain (object references become path strings), as loot_pools expects. A path no package holds is
    recorded in `missing` and reads as an empty dict; a path that fails to decode is recorded in `failed`.
    """

    def __init__(self, reader, names):
        self.reader = reader
        self.names = list(dict.fromkeys(names))
        self.classes = {}
        for name in self.names:
            for path, export in reader.exports(name)['by_path'].items():
                self.classes.setdefault(path, export['class'])
        self.cache, self.missing, self.failed = {}, set(), {}

    def props(self, path):
        if path not in self.cache:
            owner = next((n for n in self.names if path in self.reader.exports(n)['by_path']), None)
            if owner is None:
                self.missing.add(path)
                self.cache[path] = {}
            else:
                try:
                    self.cache[path] = plain_refs(self.reader.props(owner, path))
                except ValueError as error:
                    self.failed[path] = str(error)[:200]
                    self.cache[path] = {}
        return self.cache[path]


def factory_of(chain, popdef):
    """The population factory exported directly under a population definition, or None."""
    depth = popdef.count('.') + 1
    for path, cls in chain.classes.items():
        if path.startswith(popdef + '.') and path.count('.') == depth and 'PopulationFactory' in cls:
            return path
    return None


def unweighed(tree):
    """Paths of pool entries whose weight did not resolve; such an entry weighs nothing, so it never drops."""
    found = []
    for entry in tree.get('entries', []):
        if entry.get('weight') is None:
            found.append(entry['path'])
        if entry.get('pool'):
            found += unweighed(entry['pool'])
    return found


def stock(chain, values, attachments, machine_id, config, stage, unresolved):
    """ItemAttachments of one configuration -> [{pool, probability, items: [{balance, weight, class}]}]."""
    rows = []
    for attachment in attachments:
        pool = attachment.get('ItemPool')
        if not pool:
            continue

        def note(reason):
            unresolved.append({'machine': machine_id, 'config': config, 'pool': pool, 'reason': reason})

        probability = values.init(attachment.get('PoolProbability'))
        if probability is None:
            note('PoolProbability not resolved from the packages')
        if not chain.props(pool):
            note('pool not found in the packages')
        tree = expand(chain, values, pool, stage)
        if tree.get('quantity') is None:
            note('Quantity not resolved from the packages')
        if tree.get('gate') is None:
            note('stage gate not resolved from the packages')
        for path in unweighed(tree):
            note(f'entry weight not resolved: {path}')
        items = flatten(tree) if tree.get('eligible') is not False else {}
        leaves = []
        for balance, count in sorted(items.items(), key=lambda kv: (-kv[1], kv[0])):
            cls = chain.classes.get(balance)
            if cls is None:
                note(f'leaf balance {balance} is in no package')
            leaves.append({'balance': balance, 'weight': round(count, 6), 'class': (cls or 'unknown').rsplit('.', 1)[-1]})
        rows.append({'pool': pool, 'probability': probability, 'items': leaves})
    return rows


def build_machine(chain, values, machine_id, point, stage, unresolved):
    """One vending machine record, or None (its reason is appended to `unresolved`)."""
    def skip(reason):
        unresolved.append({'machine': machine_id, 'reason': reason})
        return None

    popdef = point['PopulationDef']
    factory_path = factory_of(chain, popdef)
    if factory_path is None:
        return skip(f'{popdef}: no population factory export')
    factory = chain.props(factory_path)
    cls = chain.classes.get(factory_path, '').rsplit('.', 1)[-1]
    if cls == 'PopulationFactoryBlackMarket':
        kind = 'blackmarket'
    elif cls == 'PopulationFactoryVendingMachine':
        shop = factory.get('ShopType')
        if shop is None:
            kind = 'weapons'
        elif shop in SHOP_KINDS:
            kind = SHOP_KINDS[shop]
        else:
            return skip(f'{factory_path}: unknown ShopType {shop}')
    else:
        return skip(f'{factory_path}: unknown factory class {cls}')
    balance_path = factory.get('ObjectBalanceDefinition')
    balance = chain.props(balance_path) if balance_path else {}
    if not balance:
        return skip(f'{factory_path}: ObjectBalanceDefinition did not resolve')
    definition = factory.get('ObjectDefinition') or balance.get('DefaultInteractiveObject')
    header = chain.props(definition).get('StatusMenuMapInfoBoxHeader') if definition else None
    featured = factory.get('FeaturedItem') or {}
    loot = {c.get('ConfigurationName'): c.get('ItemAttachments') or [] for c in balance.get('DefaultLoot') or []}
    rotation = point.get('Rotation') or {}
    markup = factory.get('CommerceMarkup')
    featured_markup = featured.get('CommerceMarkup')
    return {
        'id': machine_id,
        'kind': kind,
        'title': TITLES.get(kind, header),
        'header': header,
        'definition': definition,
        'location': vector(point.get('Location')),
        # Degrees [Pitch, Yaw, Roll] like the slice manifests' "host" blocks (prepare_level.transform); the decoded
        # 65536-per-turn units are kept beside them.
        'rotation': [int(rotation.get(k) or 0) * 360 / 65536 for k in ('Pitch', 'Yaw', 'Roll')],
        'rotation_units': [int(rotation.get(k) or 0) for k in ('Pitch', 'Yaw', 'Roll')],
        'fixed_item_cost': 0,
        'fixed_featured_cost': 0,
        'markup': values.init(markup) if markup else None,
        'featured_markup': values.init(featured_markup) if featured_markup else None,
        'inventory': stock(chain, values, loot.get('Inventory', []), machine_id, 'Inventory', stage, unresolved),
        'featured': stock(chain, values, loot.get('FeaturedItem', []), machine_id, 'FeaturedItem', stage, unresolved),
        'supported': kind != 'blackmarket',
    }


def build_document(reader, persistent, stage):
    """The whole document for one map: its machines, what did not resolve, and notes."""
    minutes = reader.class_default('WillowGame.GlobalsDefinition').get('MinutesBetweenShopResets')
    if minutes is None:
        raise ValueError('WillowGame GlobalsDefinition has no MinutesBetweenShopResets default')
    machines, unresolved = [], []
    attributes, missing = set(), set()
    for level in levels(reader, persistent):
        # The level's own package first: a placed point's path exists in more than one level package.
        chain = Chain(reader, [level, persistent, 'Startup'])
        values = Values(chain)
        for export in list(reader.exports(level)['by_index'].values()):
            path = export['path']
            if path.count('.') != 2 or not path.startswith('TheWorld.PersistentLevel.'):
                continue
            if export['class'].rsplit('.', 1)[-1] != 'WillowPopulationOpportunityPoint':
                continue
            point = chain.props(path)
            if not str(point.get('PopulationDef') or '').startswith(POPULATION):
                continue
            machine = build_machine(chain, values, f'{level}:{path.rsplit(".", 1)[1]}', point, stage, unresolved)
            if machine:
                machines.append(machine)
        attributes |= values.unresolved
        missing |= chain.missing
        for path, error in sorted(chain.failed.items()):
            unresolved.append({'path': path, 'reason': f'did not decode: {error}'})
    notes = [
        f'minutes_between_resets is the WillowGame GlobalsDefinition class default (MinutesBetweenShopResets = {minutes}).',
        'fixed_item_cost and fixed_featured_cost are 0: the population factories name no fixed cost (UNVERIFIED for the shops).',
        'markup and featured_markup are null where the packages do not resolve them (runtime attributes; UNVERIFIED).',
        f'{len(attributes)} attribute paths did not resolve from the packages: {sorted(attributes)}',
        f'{len(missing)} object paths are in none of the packages: {sorted(missing)}',
    ]
    return {'schema': SCHEMA_TAG, 'map': persistent, 'stage': stage, 'minutes_between_resets': minutes,
            'machines': machines, 'unresolved': unresolved, 'notes': notes}


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', required=True, help='ow-package executable')
    parser.add_argument('--game', required=True, help='Borderlands 2 install directory')
    parser.add_argument('--map', default='Sanctuary_P')
    parser.add_argument('--output', default='local/slice/vendors.json')
    parser.add_argument('--stage', type=float, default=8, help='game stage for the pool quantities and gates')
    args = parser.parse_args()
    try:
        output = local_path(args.output)
    except ValueError as error:
        print(error, file=sys.stderr)
        return 2
    SCHEMA_PATH.parent.mkdir(parents=True, exist_ok=True)
    SCHEMA_PATH.write_text('\n'.join(SCHEMA_EXTRA + SCHEMA_LINES) + '\n', encoding='utf-8')
    reader = Reader(args.reader, args.game, str(SCHEMA_PATH))
    document = build_document(reader, args.map, args.stage)
    if not document['machines']:
        print(f'{args.map}: no vending machine found', file=sys.stderr)
        return 1
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(document, indent=1), encoding='utf-8')
    kinds = {}
    for machine in document['machines']:
        kinds[machine['kind']] = kinds.get(machine['kind'], 0) + 1
    print(f'wrote {output}: {len(document["machines"])} machines {kinds}, {len(document["unresolved"])} unresolved')
    return 0


if __name__ == '__main__':
    sys.exit(main())
