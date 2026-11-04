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
import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import weapon_recipe  # noqa: E402
import weapon_stats  # noqa: E402
from loot_pools import SCHEMA_LINES, Values, expand, flatten  # noqa: E402
from prepare_slice_world import Reader, levels, vector  # noqa: E402

REPO = Path(__file__).resolve().parents[1]
SCHEMA_PATH = REPO / 'local' / 'slice' / 'vendors.schema'
# The balance's DefaultLoot and its attachments are structs the shared loot schema does not name yet.
SCHEMA_EXTRA = [
    'DefaultLoot=StructProperty:LootConfigurationData',
    'ItemAttachments=StructProperty:LootAttachmentData',
    # Non-weapon stock (section 6 of docs/verification/VENDORS.md): an ammo item's pickup effect and the
    # definitions a class mod balance chooses from.
    'ExternalAttributeEffects=StructProperty:AttributeEffectData',
    'ClassModDefinitions=ObjectProperty',
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
        # weapon_stats.resolver_constant() looks an attribute's resolver subobject up in `index` (its keys only).
        self.index = self.classes

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


# ---- Non-weapon stock: what the host needs to sell an item (docs/verification/VENDORS.md section 6) ----------------
# An item definition's part lists, in the order the definition names them (ItemDefinition.AlphaParts .. MaterialParts).
ITEM_SLOTS = ['Alpha', 'Beta', 'Gamma', 'Delta', 'Epsilon', 'Zeta', 'Eta', 'Theta', 'Material']
ITEM_KINDS = {'ShieldDefinition': 'shield', 'GrenadeModDefinition': 'grenade_mod', 'ClassModDefinition': 'class_mod'}
# D_Resources.AmmoResources.<name> -> the host's ammo type key (UOpenWillowInventory::AmmoTypeKey).
AMMO_TYPES = {'Ammo_Repeater_Pistol': 'pistol', 'Ammo_Patrol_SMG': 'smg', 'Ammo_Combat_Rifle': 'ar',
              'Ammo_Combat_Shotgun': 'shotgun', 'Ammo_Sniper_Rifle': 'sniper', 'Ammo_Rocket_Launcher': 'launcher',
              'Ammo_Grenade_Protean': 'grenade'}
PART_TOTAL = 'MonetaryValueModifierTotal'  # the property InventoryPartMonetaryValueModifierTotal reads
LEVEL_PROPERTY = 'ExpLevel'  # the property every item-level attribute of the price calculators reads
CHECK_LEVELS = range(1, 81)
# A part total is tried for at most this many part combinations; above it only the lowest and highest are kept.
MAX_TOTALS = 64


def base_chain(chain, balance):
    """The balance and its BaseDefinition ancestors, root first."""
    steps, cursor = [], balance
    while cursor and cursor not in steps and len(steps) < 16:
        steps.append(cursor)
        cursor = chain.props(cursor).get('BaseDefinition')
    return steps[::-1]


def item_slot_parts(chain, source, stage, manufacturers):
    """({slot: [part]}, mode) of one part list: the parts some manufacturer can draw at `stage`.

    `source` is a definition's ItemPartListDefinition (its WeightedParts fill one slot; the caller names the slot) or a
    balance's ItemPartListCollectionDefinition (<Slot>PartData). Weights and game-stage windows follow
    weapon_recipe.slot_candidates (UNVERIFIED rules read for weapons); a part weighing 0 for every manufacturer is out.
    """
    props = chain.props(source)
    consolidated = props.get('ConsolidatedAttributeInitData') or []

    def parts(entries):
        found = []
        for part in entries or []:
            def at(name, index=None):
                i = part.get(f'{name}Index', -1) if index is None else index
                return weapon_recipe.attribute_value(chain, consolidated[i], stage) if 0 <= i < len(consolidated) else None
            low, high = at('MinGameStage'), at('MaxGameStage')
            if (low is not None and stage < int(low)) or (high is not None and int(high) < stage):
                continue
            weights = [weapon_recipe.entry_weight(part, maker, at) for maker in (manufacturers or [None])]
            if part.get('Part') and any(w is None or w > weapon_recipe.MIN_WEIGHT for w in weights):
                found.append(part['Part'])
        return found

    if 'WeightedParts' in props:
        return {None: parts(props['WeightedParts'])}, 'EPRM_Selective'
    slots = {}
    for slot in ITEM_SLOTS:
        data = props.get(f'{slot}PartData')
        if data and data.get('bEnabled'):
            slots[slot] = parts(data.get('WeightedParts'))
    return slots, props.get('PartReplacementMode') or 'EPRM_Additive'


def item_parts(chain, steps, definition, stage):
    """{slot: [part]} an item can draw: the definition's part lists, then each balance's collection, root first.

    The merge is weapon_recipe.merge_slots (Selective replaces an enabled slot, Additive appends); that items merge
    like weapons, and that a collection overrides the definition's own lists, is UNVERIFIED.
    """
    makers = next((chain.props(b).get('Manufacturers') for b in steps[::-1] if chain.props(b).get('Manufacturers')), [])
    manufacturers = [m.get('Manufacturer') for m in makers if m.get('Manufacturer')]
    merged = {}
    for slot in ITEM_SLOTS:
        source = chain.props(definition).get(f'{slot}Parts')
        if source:
            merged[slot] = item_slot_parts(chain, source, stage, manufacturers)[0][None]
    for step in steps:
        props = chain.props(step)
        collection = props.get('PartListCollection') or props.get('ItemPartListCollection')
        if collection:
            slots, mode = item_slot_parts(chain, collection, stage, manufacturers)
            merged = weapon_recipe.merge_slots(merged, slots, mode)
    return merged


def part_totals(chain, definition, parts, unresolved):
    """Sorted distinct part totals: BaseMonetaryValueModifier times each drawn part's MonetaryValueMod.

    One part per non-empty slot, in slot order, single precision after each step (as weapon_stats.sale_value).
    Name parts are not included: no item name part in these lists carries a MonetaryValueMod (checked by
    name_part_modifiers). Too many combinations keep only the lowest and highest total.
    """
    totals = {weapon_recipe.f32(chain.props(definition).get('BaseMonetaryValueModifier',
                                                            weapon_stats.BASE_MONETARY_VALUE_MODIFIER_DEFAULT))}
    for slot in ITEM_SLOTS:
        factors = set()
        for part in parts.get(slot) or []:
            modifier = chain.props(part).get('MonetaryValueMod')
            factor = weapon_stats.resolver_constant(chain, modifier) if modifier else 1.0
            if factor is None:
                unresolved.add(modifier)
                return []
            factors.add(factor)
        if factors:
            totals = {weapon_recipe.f32(t * f) for t in totals for f in factors}
            if len(totals) > MAX_TOTALS:
                totals = {min(totals), max(totals)}
    return sorted(totals)


def name_part_modifiers(chain, definition, parts):
    """MonetaryValueMod paths on the prefix and title name parts the definition and its parts list (normally [])."""
    holders = [definition] + [p for slot in parts.values() for p in slot]
    names = {n for h in holders for key in ('PrefixList', 'TitleList') for n in (chain.props(h).get(key) or []) if n}
    return sorted({chain.props(n).get('MonetaryValueMod') for n in names} - {None})


def init_attributes(chain, init, found=None):
    """Every BaseValueAttribute an AttributeInitializationData tree reads (through its InitializationDefinitions)."""
    found = set() if found is None else found
    if not isinstance(init, dict):
        return found
    if init.get('BaseValueAttribute'):
        found.add(init['BaseValueAttribute'])
    definition = init.get('InitializationDefinition')
    if definition:
        formula = chain.props(definition).get('ValueFormula') or {}
        for key in ('Multiplier', 'Level', 'Power', 'Offset'):
            init_attributes(chain, formula.get(key), found)
    return found


def object_property(chain, attribute):
    """PropertyName of an attribute's ObjectPropertyAttributeValueResolver (ExpLevel, MonetaryValueModifierTotal...)."""
    resolvers = chain.props(attribute).get('ValueResolverChain') or [
        p for p, cls in chain.classes.items() if p.startswith(attribute + '.') and 'ValueResolver' in cls]
    for resolver in resolvers:
        if chain.classes.get(resolver, '').endswith('ObjectPropertyAttributeValueResolver'):
            return chain.props(resolver).get('PropertyName')
    return None


def item_value(chain, init, totals, unresolved):
    """The monetary value block of one definition, or None when its calculator does not resolve.

    value(level) = trunc(calculator), evaluated with weapon_stats.price_init (the evaluator that reproduces 69/69 gun
    cards); the item's level and its part total are the only inputs. Every calculator met here reduces to
    part_total * scaler ^ level * base_cost (BaseCost attribute; part_total 1 when the calculator does not read it);
    `shape_checked` says the reduced form gives the evaluator's integer at every level 1..80 for every total.
    """
    if not isinstance(init, dict) or not init.get('InitializationDefinition'):
        return None
    reads = {object_property(chain, a): a for a in init_attributes(chain, init)}
    level_attributes = [a for a in init_attributes(chain, init) if object_property(chain, a) == LEVEL_PROPERTY]
    total_attribute = reads.get(PART_TOTAL)
    totals = totals if total_attribute else [1.0]

    def evaluate(level, total):
        known = {a: float(level) for a in level_attributes}
        if total_attribute:
            known[total_attribute] = total
        missing = set()
        value = weapon_stats.price_init(chain, init, known, missing)
        unresolved.update(missing)
        return value

    first = evaluate(0, 1.0)
    if first is None or not totals:
        return None
    scaler = weapon_recipe.f32(evaluate(1, 1.0) / first) if first else None
    shape = bool(scaler) and all(
        math.trunc(evaluate(level, t)) == math.trunc(weapon_recipe.f32(weapon_recipe.f32(
            t * weapon_recipe.f32(scaler ** level)) * first)) for t in totals for level in CHECK_LEVELS)
    return {'calculator': init['InitializationDefinition'], 'base_cost': first, 'scaler': scaler,
            'level_attributes': sorted(level_attributes), 'part_total_attribute': total_attribute,
            'part_totals': totals, 'shape_checked': shape,
            'formula': 'trunc(part_total * scaler ^ item_level * base_cost)'}


def value_at(chain, block, level, total):
    """The integer monetary value of a value block at an item level and part total (the evaluator, not the shape)."""
    known = {a: float(level) for a in block['level_attributes']}
    if block['part_total_attribute']:
        known[block['part_total_attribute']] = total
    value = weapon_stats.price_init(chain, {'InitializationDefinition': block['calculator']}, known, set())
    return None if value is None else math.trunc(value)


def ammo_effect(chain, definition, unresolved):
    """(resource path, host ammo key, rounds) from the first ExternalAttributeEffects entry on an ammo pool, or None.

    The rounds are the effect's BaseModifierValue (its attribute's constant when one is named, as the evaluator
    takes it; the AmmoAmountShop_* attributes). That buying applies the pickup effect is UNVERIFIED.
    """
    for effect in chain.props(definition).get('ExternalAttributeEffects') or []:
        target = effect.get('AttributeToModify')
        resource = chain.props(target).get('Resource') if target else None
        if not resource or '.AmmoResources.' not in resource:
            continue
        missing = set()
        amount = weapon_stats.price_init(chain, effect.get('BaseModifierValue'), {}, missing)
        unresolved.update(missing)
        return resource, AMMO_TYPES.get(resource.rsplit('.', 1)[-1]), amount
    return None


def heal_fraction(chain, definition):
    """The multiplier of a Behavior_SimpleMath Mul under the definition's behaviours (Health Now!: 0.25), or None.

    The behaviour graph that applies it is not decoded (BehaviorSequences); the item's own card text ("heals 25% of
    your Max Health") agrees. UNVERIFIED as the game's heal.
    """
    provider = chain.props(definition).get('BehaviorProviderDefinition')
    if not provider:
        return None
    for path, cls in sorted(chain.classes.items()):
        if path.startswith(provider + '.') and cls.endswith('Behavior_SimpleMath'):
            props = chain.props(path)
            if props.get('Operation') == 'BINARYMATH_Mul' and props.get('B'):
                return props['B']
    return None


def item_block(chain, balance, stage):
    """{kind, name, ammo_pool?, ammo_type?, amount?, heal?, value, variants?, unresolved} for one non-weapon balance."""
    unresolved = set()
    steps = base_chain(chain, balance)
    definition = next((chain.props(b).get('InventoryDefinition') for b in steps[::-1]
                       if chain.props(b).get('InventoryDefinition')), None)
    definitions = [definition] if definition else next(
        (chain.props(b).get('ClassModDefinitions') for b in steps[::-1] if chain.props(b).get('ClassModDefinitions')), [])
    definitions = [d for d in definitions if d]
    if not definitions:
        return {'kind': 'unknown', 'name': None, 'value': None, 'unresolved': ['no InventoryDefinition']}
    variants = []
    for path in definitions:
        props = chain.props(path)
        parts = item_parts(chain, steps, path, stage)
        totals = part_totals(chain, path, parts, unresolved)
        block = item_value(chain, props.get('MonetaryValue'), totals, unresolved)
        if block is None:
            unresolved.add(f'{path}: no MonetaryValue calculator that resolves from the packages')
        if name_part_modifiers(chain, path, parts):
            unresolved.add(f'{path}: a name part carries a MonetaryValueMod (not in the part total)')
        # A class mod definition names no ItemName; its object name ends in the card's name (ClassMod_Siren_Nurse).
        variants.append({'definition': path, 'name': props.get('ItemName'), 'key': path.rsplit('_', 1)[-1],
                         'value': block})
    first = chain.props(definitions[0])
    cls = chain.classes.get(definitions[0], '').rsplit('.', 1)[-1]
    item = {'kind': ITEM_KINDS.get(cls, 'usable' if cls == 'UsableItemDefinition' else 'unknown'),
            'definition_class': cls, 'name': first.get('ItemName'), 'value': variants[0]['value']}
    if cls == 'UsableItemDefinition':
        ammo = ammo_effect(chain, definitions[0], unresolved)
        heal = heal_fraction(chain, definitions[0])
        if ammo:
            item.update(kind='ammo', ammo_pool=ammo[0], ammo_type=ammo[1], amount=ammo[2])
        elif heal is not None:
            item.update(kind='health', heal={'fraction_of_max_health': heal})
    if len(variants) > 1:
        item['variants'] = variants
        item['value'] = None  # the class mod's definition is one of `variants`, chosen at spawn
    item['unresolved'] = sorted(unresolved)
    return item


def oracle_rows(oracle):
    """Non-weapon stock and featured rows of a live dump: [{balance, level, value, name}] (malformed rows skipped)."""
    rows = []
    for machine in oracle.get('machines') or []:
        stock = machine.get('stock') if isinstance(machine.get('stock'), list) else []
        for row in stock + [machine.get('featured')]:
            if not isinstance(row, dict) or not isinstance(row.get('class'), str) or row['class'] == 'WillowWeapon':
                continue
            if not isinstance(row.get('balance'), str) or not isinstance(row.get('monetary_value'), int):
                continue
            rows.append({'balance': row['balance'], 'level': row.get('exp_level'), 'value': row['monetary_value'],
                         'name': row.get('name') or ''})
    return rows


def predicted_values(chain, item, row):
    """Integer values the item can take at the row's level (a class mod narrowed by the name the row shows)."""
    variants = item.get('variants') or [{'name': item.get('name'), 'value': item.get('value')}]
    words = row['name'].lower().split()
    named = [v for v in variants if v.get('key') and v['key'].lower() in words]
    found = set()
    for variant in named or variants:
        block = variant.get('value')
        if block and isinstance(row['level'], int):
            found |= {value_at(chain, block, row['level'], t) for t in block['part_totals']}
    return found - {None}


def verify_items(chain, items, oracle):
    """Set item['verified'] = {'value': 'n of m live rows'} (exact integer match) and return [(balance, row, values)]."""
    misses = []
    counts = {}
    for row in oracle_rows(oracle):
        item = items.get(row['balance'])
        if item is None:
            continue
        values = predicted_values(chain, item, row)
        ok = len(values) == 1 and row['value'] in values
        hit, seen = counts.get(row['balance'], (0, 0))
        counts[row['balance']] = (hit + ok, seen + 1)
        if not ok:
            misses.append((row['balance'], row, sorted(values)))
    for balance, item in items.items():
        hit, seen = counts.get(balance, (0, 0))
        item['verified'] = {'value': f'{hit} of {seen} live rows' if seen else 'UNVERIFIED (no live row)'}
    return misses


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


# Class mod balances the shop pools name but cannot list: GD_Itempools.ClassModPools.Pool_ClassMod_01_Common and
# _02_Uncommon hold no entries in the packages (filled at runtime; how is UNVERIFIED). Their Startup balances are
# described under `unpooled_items` so the host can stock them by hand.
CLASS_MOD_GRADES = ('_01_Common', '_02_Uncommon')


def describe_items(reader, persistent, machines, stage, oracle=None):
    """Attach an `item` block to every non-weapon leaf; return (unpooled class mod items, oracle misses, notes)."""
    chain = Chain(reader, [persistent, 'Startup'])
    leaves = [leaf for m in machines for config in ('inventory', 'featured') for row in m[config]
              for leaf in row['items'] if leaf['class'] != 'WeaponBalanceDefinition']
    items = {}
    for leaf in leaves:
        if leaf['balance'] not in items:
            items[leaf['balance']] = item_block(chain, leaf['balance'], stage)
        leaf['item'] = items[leaf['balance']]
    class_mods = sorted(p for p, cls in chain.classes.items() if cls.endswith('.ClassModBalanceDefinition')
                        and p.rsplit('.', 1)[-1].startswith('BalDef_ClassMod_') and p.endswith(CLASS_MOD_GRADES))
    unpooled = []
    for balance in class_mods:
        items[balance] = item_block(chain, balance, stage)
        unpooled.append({'balance': balance, 'reason': 'class mod pools are empty in the packages',
                         'item': items[balance]})
    misses, notes = [], []
    if oracle is not None:
        misses = verify_items(chain, items, oracle)
        absent = sorted({r['balance'] for r in oracle_rows(oracle)} - set(items))
        notes.append(f'{len(absent)} live non-weapon balances are not described (outside the map package and '
                     f'Startup, e.g. DLC class mods): {absent}')
    else:
        for item in items.values():
            item['verified'] = {'value': 'UNVERIFIED (not checked: no --oracle)'}
    return unpooled, misses, notes


def build_document(reader, persistent, stage, oracle=None):
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
    unpooled, misses, item_notes = describe_items(reader, persistent, machines, stage, oracle)
    for balance, row, predicted in misses:
        unresolved.append({'balance': balance, 'reason': f'live value {row["value"]} at level {row["level"]} '
                                                         f'({row["name"]}) not reproduced; predicted {predicted}'})
    notes += item_notes + [
        'item blocks: value(level) = trunc(part_total * scaler ^ item_level * base_cost), evaluated by the price '
        "calculator; the part total is BaseMonetaryValueModifier times every drawn part's MonetaryValueMod "
        '(docs/verification/VENDORS.md section 6). verified.value counts exact live matches.',
    ]
    return {'schema': SCHEMA_TAG, 'map': persistent, 'stage': stage, 'minutes_between_resets': minutes,
            'machines': machines, 'unpooled_items': unpooled, 'unresolved': unresolved, 'notes': notes}


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', required=True, help='ow-package executable')
    parser.add_argument('--game', required=True, help='Borderlands 2 install directory')
    parser.add_argument('--map', default='Sanctuary_P')
    parser.add_argument('--output', default='local/slice/vendors.json')
    parser.add_argument('--stage', type=float, default=8, help='game stage for the pool quantities and gates')
    parser.add_argument('--oracle', help='live vendor dump (local/realgame/vendors/*.json) to count exact value matches')
    args = parser.parse_args()
    try:
        output = local_path(args.output)
    except ValueError as error:
        print(error, file=sys.stderr)
        return 2
    SCHEMA_PATH.parent.mkdir(parents=True, exist_ok=True)
    SCHEMA_PATH.write_text('\n'.join(SCHEMA_EXTRA + SCHEMA_LINES) + '\n', encoding='utf-8')
    reader = Reader(args.reader, args.game, str(SCHEMA_PATH))
    oracle = json.loads(Path(args.oracle).read_text(encoding='utf-8')) if args.oracle else None
    document = build_document(reader, args.map, args.stage, oracle)
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
