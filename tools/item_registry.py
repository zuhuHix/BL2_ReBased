"""Registry of every Borderlands 2 item kind, generated from the player's installed packages (local/items/registry/).

AI-assisted (Claude Code). Reads every installed package (base game and each DLC folder) with `ow-package` and writes one
JSON registry of every inventory balance with its base chain, manufacturers and grade windows, definitions, the merged part
candidates per slot (each with its weight source and game-stage window), name parts and gestalt, plus a census of what
resolved and what was skipped. The registry is game data (docs/LEGAL.md): it goes under the ignored local/ folder only.

Usage (from anywhere; paths in the arguments are taken as given):
    python tools/item_registry.py --reader build/Release/ow-package.exe --game "<Borderlands 2 install>"
writes local/items/registry/registry.json and census.json and prints counts only (no object names).

Reused by import, not copied: weapon_catalog (package list, export scan, Census: the first package holding a path owns it),
weapon_recipe (SLOTS, merge_slots, attribute_value, FLAT_WEIGHT, MIN_WEIGHT, merge, part_fragments), prepare_vendors
(ITEM_SLOTS, item_parts, base_chain, local_path) and prepare_stock_shield.candidates (the stock shield check).

Rules (assumptions about the data; the ones that are game rules are UNVERIFIED for items, see `rules` in the output):
- A balance's chain is BaseDefinition followed from the balance (leaf first) until a step has none ("root"), a step repeats
  ("cycle") or a step is in no package or does not decode ("missing" / "decode_failed").
- Manufacturers, InventoryDefinition and ClassModDefinitions: the first step, leaf first, that sets them (as
  prepare_vendors.item_parts reads them). weapon_recipe.merge takes the root-most step instead; where the two differ the
  root-first value is written too (`*_root_first`) and counted in the census.
- Part candidates. Weapons: each step's WeaponPartListCollection, root first, merged with weapon_recipe.merge_slots (Additive
  appends, Selective replaces an enabled slot, Complete replaces every slot; a missing mode is Additive). Items: the
  definition's <Slot>Parts lists first, then each step's PartListCollection or ItemPartListCollection, merged the same way
  (prepare_vendors.item_parts). Weapon merge is checked in game for part identity (WEAPON_PARTS_CATALOG.md); for items it is
  reused unchanged and UNVERIFIED. A class mod balance with several ClassModDefinitions gets one variant per definition.
- Entries are kept before any stage or weight is applied. Each keeps its weight source: "flat" (empty Manufacturers list:
  weapon_recipe.FLAT_WEIGHT) or "manufacturer" (per manufacturer weight, then DefaultWeight for the others), and its
  game-stage window. A value is a number when it needs no runtime state, {"init": <AttributeInitializationData>} when it
  does, null when the entry names no index (no bound). candidates_at() applies them as weapon_recipe and
  prepare_stock_shield do.

Registry shape, schema "openwillow-item-registry/1" (field names are this tool's own):
  schema, schema_line, generated, game, tool, rules[], slots {weapon: [...], item: [...]}
  balances {path: {class, kind, package, dlc, packages_holding, chain[leaf..root], chain_end, chain_missing,
      manufacturers [{manufacturer, grades [{min_stage, max_stage, exp_level, definition, spawn_probability {min, max}}]}],
      manufacturers_from, inventory_definition, class_mod_definitions[], definitions[], gestalt,
      variants [{definition, slots {slot: [candidate]}, merge [{source, list, mode, slots[]}]}],
      name_parts {prefix[], title[]}}}
    candidate: {part, list, weight {source: flat, value} | {source: manufacturer, by_manufacturer {maker: value}, default},
                min_stage, max_stage}
  definitions {path: {class, kind, package, dlc, part_lists {slot: path}, prefix[], title[], gestalt, manufacturer_override,
      required_player_class, customization, balances[]}}
  parts {path: {class, kind, package, dlc, part_type, manufacturer_scope, prefix[], title[], gestalt_fragments[], material,
      balances[]}}
  part_lists, name_parts, manufacturers, gestalts {path: {class, package, dlc}}
The census (census.json) holds counts, the skipped objects with their reason and the references that did not resolve.
"""
import argparse
import concurrent.futures
import datetime
import json
import math
import sys
import time
from collections import Counter, defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import loot_pools  # noqa: E402
import prepare_stock_shield as ps  # noqa: E402
import prepare_vendors as pv  # noqa: E402
import weapon_catalog  # noqa: E402
import weapon_recipe  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
SCHEMA_TAG = 'openwillow-item-registry/1'
SCHEMA_LINE = (f'{SCHEMA_TAG}: balances, definitions, parts, part_lists, name_parts, manufacturers, gestalts keyed by object '
               'path; candidates unevaluated (weights and stage windows as numbers or attribute init data); see the '
               'docstring of tools/item_registry.py')
DEFAULT_OUT = 'local/items/registry'
DEFAULT_READER = ROOT / 'build/Release/ow-package.exe'
# The manufacturer grade list of a balance (ManufacturerGradeData.Grades): tagged struct elements, like every other array here.
SCHEMA_LINES = list(dict.fromkeys(loot_pools.SCHEMA_LINES + pv.SCHEMA_EXTRA + ['Grades=StructProperty:GradeData']))

WEAPON_BALANCES = {'WeaponBalanceDefinition', 'MissionWeaponBalanceDefinition'}
ITEM_BALANCES = {'InventoryBalanceDefinition', 'ClassModBalanceDefinition', 'ItemBalanceDefinition'}
BALANCE_CLASSES = WEAPON_BALANCES | ITEM_BALANCES
DEFINITION_KINDS = {
    'ShieldDefinition': 'shield', 'GrenadeModDefinition': 'grenade_mod', 'ClassModDefinition': 'class_mod',
    'CrossDLCClassModDefinition': 'class_mod', 'ArtifactDefinition': 'relic', 'UsableItemDefinition': 'usable',
    'UsableCustomizationItemDefinition': 'customization', 'MissionItemDefinition': 'mission_item',
    'MarketingUnlockInventoryDefinition': 'marketing_unlock'}
PART_KINDS = {
    'WeaponPartDefinition': 'weapon', 'ShieldPartDefinition': 'shield', 'GrenadeModPartDefinition': 'grenade_mod',
    'ClassModPartDefinition': 'class_mod', 'ArtifactPartDefinition': 'relic', 'UsableItemPartDefinition': 'usable'}
LIST_CLASSES = {'ItemPartListDefinition', 'ItemPartListCollectionDefinition', 'WeaponPartListDefinition',
                'WeaponPartListCollectionDefinition'}
NAME_PART_CLASSES = {'ItemNamePartDefinition', 'WeaponNamePartDefinition'}
WEAPON_SLOTS = list(weapon_recipe.SLOTS)
ITEM_SLOTS = list(pv.ITEM_SLOTS)
# Stages the census evaluates the candidates at to compare them with the existing tools (any three will do: low, mid, cap).
CHECK_STAGES = (1, 30, 80)
SHIELD_CHECK_LEVEL = ps.SLICE_LEVEL
RULES = [
    'balance chain: BaseDefinition from the balance, leaf first, until a step has none',
    'manufacturers, InventoryDefinition, ClassModDefinitions: the leaf-most step that sets them (prepare_vendors)',
    'weapon part merge: WeaponPartListCollection per step, root first, weapon_recipe.merge_slots (checked in game for part '
    'identity, WEAPON_PARTS_CATALOG.md)',
    'item part merge: definition <Slot>Parts, then each step\'s collection, root first, weapon_recipe.merge_slots '
    '(UNVERIFIED for items: not confirmed in the running game)',
    'entry weight: flat 100 for an empty Manufacturers list, else the manufacturer\'s weight, else DefaultWeight '
    '(read from native code for weapons; UNVERIFIED in game for items)',
    'game-stage window per entry from the list\'s ConsolidatedAttributeInitData (UNVERIFIED for items)',
    'the relic per-type weight hook (WEAPON_PARTS_CATALOG.md section 2) is not read: relic weights are open',
    'name parts listed are the candidates (definition or weapon type lists, then every candidate part\'s lists); the '
    'choice among them is weapon_recipe.best_name_part and not made here',
    'first package holding a path owns it (base game, then DLC folders in name order); other holders are listed',
]


def short(cls):
    return cls.rsplit('.', 1)[-1] if cls else cls


def is_weapon_type(name):
    return name.endswith('WeaponTypeDefinition')


def relevant(name):
    """The classes the registry lists (unqualified class name)."""
    return (name in BALANCE_CLASSES or name in DEFINITION_KINDS or is_weapon_type(name) or name in PART_KINDS
            or name in LIST_CLASSES or name in NAME_PART_CLASSES or name == 'ManufacturerDefinition'
            or name == 'GestaltSkeletalMeshDefinition')


def definition_kind(name):
    return 'weapon' if is_weapon_type(name) else DEFINITION_KINDS.get(name)


class RegistryCensus(weapon_catalog.Census):
    """weapon_catalog.Census, owning every export path (attribute definitions are looked up too) and keeping what it skips."""

    def __init__(self, reader, schema):
        super().__init__(reader, schema)
        self.holders = defaultdict(list)  # registry-class path -> every package file holding it, in scan order
        self.class_defaults = Counter()  # registry class -> Default__ objects skipped
        self.failed = {}  # path -> decode error
        self.null_part_entries = Counter()  # part list -> WeightedParts rows that name no part (skipped, counted)
        self.null_name_entries = Counter()  # definition or part -> PrefixList / TitleList entries that are empty

    def add(self, path, cls, file):
        name = short(cls)
        if weapon_catalog.is_class_default(path):
            if relevant(name):
                self.class_defaults[name] += 1
            return
        if relevant(name) or path in self.holders:
            self.holders[path].append(file)
        if path in self.classes:
            self.duplicates += 1
            if short(self.classes[path]) != name and (relevant(name) or relevant(short(self.classes[path]))):
                self.conflicts.append({'path': path, 'first': self.classes[path], 'second': cls,
                                       'second_package': str(file)})
            return
        self.classes[path] = cls
        self.owner[path] = file

    def get(self, path):
        """Decoded properties, or None (no package holds the path, or it does not decode; the reason is kept)."""
        if not path or path not in self.owner:
            return None
        try:
            return self.props(path)
        except (RuntimeError, KeyError, ValueError) as error:
            self.failed[path] = str(error)[:200]
            return None


# ---- values -------------------------------------------------------------------------------------------------------------
def init_value(census, consolidated, index):
    """One ConsolidatedAttributeInitData entry: a number, {'init': data} when it needs runtime state, None without an index."""
    if index is None or not 0 <= index < len(consolidated):
        return None
    init = consolidated[index]
    try:
        value = weapon_recipe.attribute_value(census, init, None)
    except (KeyError, RuntimeError, ValueError):  # an init definition no package holds or that does not decode
        value = None
    if value is not None:
        return value
    out = {'init': init}
    constant = attribute_constant(census, init.get('BaseValueAttribute'))
    if constant is not None and not init.get('InitializationDefinition'):
        out['attribute_constant'] = constant
    return out


def attribute_constant(census, attribute):
    """ConstantValue of an attribute whose ValueResolverChain is exactly one ConstantAttributeValueResolver, else None.

    A hint for later lanes, not applied here: weapon_recipe.attribute_value (and so the existing item tools) treats such a
    base as runtime state. That the game reads the resolver's constant unchanged is UNVERIFIED."""
    props = census.get(attribute) if attribute else None
    chain = (props or {}).get('ValueResolverChain') or []
    if len(chain) != 1 or not short(census.classes.get(chain[0], '')).endswith('ConstantAttributeValueResolver'):
        return None
    return (census.get(chain[0]) or {}).get('ConstantValue')


def value_at(census, value, stage):
    """A registry value at one game stage (weapon_recipe.attribute_value for an init), or None."""
    if value is None or isinstance(value, (int, float)):
        return value
    try:
        return weapon_recipe.attribute_value(census, value['init'], stage)
    except (KeyError, RuntimeError, ValueError):
        return None


def describe_entries(census, rows, consolidated, list_path):
    """WeightedParts rows -> registry candidates (unevaluated)."""
    out = []
    for row in rows or []:
        if not row.get('Part'):
            census.null_part_entries[list_path] += 1
            continue
        makers = row.get('Manufacturers') or []
        if makers:
            weight = {'source': 'manufacturer',
                      'by_manufacturer': {m.get('Manufacturer'): init_value(census, consolidated, m.get('DefaultWeightIndex', -1))
                                          for m in makers},
                      'default': init_value(census, consolidated, row.get('DefaultWeightIndex', -1))}
        else:
            weight = {'source': 'flat', 'value': weapon_recipe.FLAT_WEIGHT}
        out.append({'part': row['Part'], 'list': list_path, 'weight': weight,
                    'min_stage': init_value(census, consolidated, row.get('MinGameStageIndex', -1)),
                    'max_stage': init_value(census, consolidated, row.get('MaxGameStageIndex', -1))})
    return out


def list_slots(census, path, slots):
    """({slot: [candidate]}, mode) of one part list collection (enabled <Slot>PartData only), or (None, None)."""
    props = census.get(path)
    if props is None:
        return None, None
    consolidated = props.get('ConsolidatedAttributeInitData') or []
    found = {}
    for slot in slots:
        data = props.get(f'{slot}PartData')
        if data and data.get('bEnabled'):
            found[slot] = describe_entries(census, data.get('WeightedParts'), consolidated, path)
    return found, props.get('PartReplacementMode') or 'EPRM_Additive'


def single_list(census, path):
    """[candidate] of one ItemPartListDefinition (its WeightedParts fill the slot the definition names), or None."""
    props = census.get(path)
    if props is None:
        return None
    return describe_entries(census, props.get('WeightedParts'), props.get('ConsolidatedAttributeInitData') or [], path)


def weight_at(census, candidate, stage, manufacturer):
    """weapon_recipe.entry_weight over a registry candidate."""
    weight = candidate['weight']
    if weight['source'] == 'flat' or not manufacturer:
        return weapon_recipe.FLAT_WEIGHT
    if manufacturer in weight['by_manufacturer']:
        return value_at(census, weight['by_manufacturer'][manufacturer], stage)
    return value_at(census, weight['default'], stage)


def candidates_at(census, slots, stage, manufacturer):
    """{slot: {part: weight}} at one stage for one manufacturer: the stage window as weapon_recipe.slot_candidates applies it,
    then entries at or below MIN_WEIGHT dropped and a repeated part keeping its later weight at its first position
    (weapon_recipe.pick). A weight that needs runtime state stays None."""
    out = {}
    for slot, entries in slots.items():
        live = {}
        for candidate in entries:
            low, high = value_at(census, candidate['min_stage'], stage), value_at(census, candidate['max_stage'], stage)
            if (low is not None and stage < int(low)) or (high is not None and int(high) < stage):
                continue
            weight = weight_at(census, candidate, stage, manufacturer)
            if weight is None or weight > weapon_recipe.MIN_WEIGHT:
                live[candidate['part']] = weight
        if live:
            out[slot] = live
    return out


# ---- balances -----------------------------------------------------------------------------------------------------------
def walk_chain(census, balance):
    """(steps leaf first, end, missing path or None)."""
    steps, cursor = [], balance
    while True:
        if cursor in steps:
            return steps, 'cycle', cursor
        if cursor not in census.owner:
            return steps, 'missing', cursor
        props = census.get(cursor)
        if props is None:
            return steps, 'decode_failed', cursor
        steps.append(cursor)
        cursor = props.get('BaseDefinition')
        if not cursor:
            return steps, 'root', None


def first_set(census, steps, key):
    """(value, step) of the first step in `steps` whose `key` is set and not empty."""
    for step in steps:
        value = (census.get(step) or {}).get(key)
        if value:
            return value, step
    return None, None


def grades_of(census, maker):
    rows = []
    for grade in maker.get('Grades') or []:
        need = grade.get('GameStageRequirement') or {}
        modifiers = grade.get('GradeModifiers') or {}
        rows.append({'min_stage': need.get('MinGameStage'), 'max_stage': need.get('MaxGameStage'),
                     'exp_level': modifiers.get('ExpLevel'), 'definition': modifiers.get('CustomInventoryDefinition'),
                     'spawn_probability': {
                         key: init_value(census, [grade[field]], 0) if grade.get(field) else None
                         for key, field in (('min', 'MinSpawnProbabilityModifier'), ('max', 'MaxSpawnProbabilityModifier'))}})
    return rows


def variant_slots(census, steps_root_first, definition, weapon):
    """({slot: [candidate]}, merge history, problems) for one definition (None: the collections only)."""
    merged, history, problems = {}, [], []
    slots = WEAPON_SLOTS if weapon else ITEM_SLOTS
    if definition and not weapon:
        props = census.get(definition) or {}
        for slot in ITEM_SLOTS:
            source = props.get(f'{slot}Parts')
            if not source:
                continue
            entries = single_list(census, source)
            if entries is None:
                problems.append(('part list', source))
                continue
            merged[slot] = entries
            history.append({'source': definition, 'list': source, 'mode': 'definition', 'slots': [slot]})
    for step in steps_root_first:
        props = census.get(step) or {}
        collection = props.get('WeaponPartListCollection') if weapon else (
            props.get('PartListCollection') or props.get('ItemPartListCollection'))
        if not collection:
            continue
        found, mode = list_slots(census, collection, slots)
        if found is None:
            problems.append(('part list', collection))
            continue
        history.append({'source': step, 'list': collection, 'mode': mode, 'slots': sorted(found)})
        merged = weapon_recipe.merge_slots(merged, found, mode)
    return {slot: merged[slot] for slot in slots if slot in merged}, history, problems


def balance_record(census, path, dlc_of):
    cls = short(census.classes[path])
    weapon = cls in WEAPON_BALANCES
    steps, end, missing = walk_chain(census, path)
    root_first = steps[::-1]
    makers, makers_from = first_set(census, steps, 'Manufacturers')
    makers_root, _ = first_set(census, root_first, 'Manufacturers')
    definition, _ = first_set(census, steps, 'InventoryDefinition')
    definition_root, _ = first_set(census, root_first, 'InventoryDefinition')
    class_mods, _ = first_set(census, steps, 'ClassModDefinitions')
    manufacturers = [{'manufacturer': m.get('Manufacturer'), 'grades': grades_of(census, m)} for m in makers or []]
    definitions = []
    for value in [definition] + list(class_mods or []) + [g['definition'] for m in manufacturers for g in m['grades']]:
        if value and value not in definitions:
            definitions.append(value)
    if weapon:
        variant_defs = [definition]
    elif class_mods:
        variant_defs = list(dict.fromkeys(class_mods))
    else:
        variant_defs = list(dict.fromkeys([definition] + [d for d in definitions if d != definition]))
    variants, problems = [], []
    for item in variant_defs:
        slots, history, trouble = variant_slots(census, root_first, item, weapon)
        problems += trouble
        variants.append({'definition': item, 'slots': slots, 'merge': history})
    kinds = {definition_kind(short(census.classes.get(d, ''))) for d in definitions} - {None}
    if weapon:
        kind = 'weapon'
    elif kinds == {'weapon'}:
        kind = 'weapon_by_inventory_balance'  # an inventory balance naming a weapon type (vehicle, turret, enemy weapons)
    elif cls == 'ClassModBalanceDefinition':
        kind = 'class_mod'
    elif len(kinds) == 1:
        kind = next(iter(kinds))
    else:
        kind = 'mixed' if kinds else 'no_definition'
    gestalt = None
    if definition:
        gestalt = (census.get(definition) or {}).get('GestaltMesh')
    record = {
        'class': cls, 'kind': kind, 'package': rel(census, path), 'dlc': dlc_of(path),
        'packages_holding': len(census.holders.get(path, [])),
        'chain': steps, 'chain_end': end, 'chain_missing': missing,
        'manufacturers': manufacturers, 'manufacturers_from': makers_from,
        'inventory_definition': definition, 'class_mod_definitions': list(class_mods or []),
        'definitions': definitions, 'gestalt': gestalt, 'variants': variants}
    if definition_root != definition:
        record['inventory_definition_root_first'] = definition_root
    if makers_root != makers:
        record['manufacturers_root_first'] = [m.get('Manufacturer') for m in makers_root or []]
    return record, problems


def rel(census, path):
    return Path(census.owner[path]).relative_to(census.game).as_posix()


# ---- the registry -------------------------------------------------------------------------------------------------------
def names_of(census, path, props):
    """(prefix list, title list) without empty entries (each empty entry is counted on the census)."""
    out = []
    for key in ('PrefixList', 'TitleList'):
        values = props.get(key) or []
        census.null_name_entries[path] += sum(1 for v in values if not v)
        out.append([v for v in values if v])
    return out[0], out[1]


def build(census, dlc_by_file):
    """(registry, census report) from a filled RegistryCensus. `dlc_by_file`: {package file: dlc name}."""
    def dlc_of(path):
        return dlc_by_file.get(census.owner[path], '?')

    by_class = defaultdict(list)
    for path, cls in census.classes.items():
        if relevant(short(cls)):
            by_class[short(cls)].append(path)
    for paths in by_class.values():
        paths.sort()
    decode = [p for name, paths in by_class.items()
              if name in BALANCE_CLASSES or definition_kind(name) or name in PART_KINDS or name in LIST_CLASSES
              for p in paths]
    census.preload(decode)

    skipped = Counter()
    skipped_examples = defaultdict(list)

    def skip(reason, path):
        skipped[reason] += 1
        if len(skipped_examples[reason]) < 5:
            skipped_examples[reason].append(path)

    # Definitions and weapon types.
    definitions = {}
    for name, paths in by_class.items():
        kind = definition_kind(name)
        if not kind:
            continue
        for path in paths:
            props = census.get(path)
            if props is None:
                skip(f'{name}: does not decode', path)
                continue
            prefix, title = names_of(census, path, props)
            lists = {slot: props.get(f'{slot}Parts') for slot in (WEAPON_SLOTS if kind == 'weapon' else ITEM_SLOTS)
                     if props.get(f'{slot}Parts')}
            definitions[path] = {
                'class': name, 'kind': kind, 'package': rel(census, path), 'dlc': dlc_of(path), 'part_lists': lists,
                'prefix': prefix, 'title': title, 'gestalt': props.get('GestaltMesh'),
                'manufacturer_override': props.get('ManufacturerOverride'),
                'required_player_class': props.get('RequiredPlayerClass'),
                'customization': props.get('CustomizationDef'), 'balances': []}

    # Parts.
    parts = {}
    for name, kind in PART_KINDS.items():
        for path in by_class.get(name, []):
            props = census.get(path)
            if props is None:
                skip(f'{name}: does not decode', path)
                continue
            prefix, title = names_of(census, path, props)
            parts[path] = {
                'class': name, 'kind': kind, 'package': rel(census, path), 'dlc': dlc_of(path),
                'part_type': props.get('PartType') or props.get('ShieldPartType'),
                'manufacturer_scope': props.get('ManufacturerDefinitionForScopeMovie'),
                'prefix': prefix, 'title': title,
                'gestalt_fragments': weapon_recipe.part_fragments(census.package(census.owner[path]), path),
                'material': props.get('Material'), 'balances': []}

    # Balances.
    balances, problems = {}, []
    for name in sorted(BALANCE_CLASSES):
        for path in by_class.get(name, []):
            if census.get(path) is None:
                skip(f'{name}: does not decode', path)
                continue
            record, trouble = balance_record(census, path, dlc_of)
            problems += [(path, what, ref) for what, ref in trouble]
            type_names = ([], [])
            for item in record['definitions']:
                if item in definitions:
                    definitions[item]['balances'].append(path)
                    type_names[0].extend(definitions[item]['prefix'])
                    type_names[1].extend(definitions[item]['title'])
            prefix, title = list(type_names[0]), list(type_names[1])
            for variant in record['variants']:
                for entries in variant['slots'].values():
                    for candidate in entries:
                        part = parts.get(candidate['part'])
                        if part is not None:
                            if not part['balances'] or part['balances'][-1] != path:
                                part['balances'].append(path)
                            prefix += part['prefix']
                            title += part['title']
            record['name_parts'] = {'prefix': list(dict.fromkeys(prefix)), 'title': list(dict.fromkeys(title))}
            balances[path] = record
    for record in list(definitions.values()) + list(parts.values()):
        record['balances'] = sorted(set(record['balances']))

    def table(names):
        return {p: {'class': short(census.classes[p]), 'package': rel(census, p), 'dlc': dlc_of(p)}
                for name in names for p in by_class.get(name, [])}

    registry = {
        'schema': SCHEMA_TAG, 'schema_line': SCHEMA_LINE,
        'generated': datetime.datetime.now().isoformat(timespec='seconds'), 'game': str(census.game),
        'tool': 'tools/item_registry.py', 'rules': RULES,
        'slots': {'weapon': WEAPON_SLOTS, 'item': ITEM_SLOTS},
        'balances': balances, 'definitions': definitions, 'parts': parts,
        'part_lists': table(LIST_CLASSES), 'name_parts': table(NAME_PART_CLASSES),
        'manufacturers': table(['ManufacturerDefinition']), 'gestalts': table(['GestaltSkeletalMeshDefinition'])}
    report = census_report(census, registry, by_class, skipped, skipped_examples, problems)
    return registry, report


def resolve_problem(census, path, expect):
    """None when `path` is an export of a class `expect` accepts, else the reason."""
    if path not in census.classes:
        return 'no installed package exports it'
    if not expect(short(census.classes[path])):
        others = sorted({c['second'] for c in census.conflicts if c['path'] == path})
        extra = f' (another package holds it as {", ".join(short(o) for o in others)})' if others else ''
        return f'class {short(census.classes[path])} is not the expected kind{extra}'
    return None


def census_report(census, registry, by_class, skipped, skipped_examples, problems):
    """Counts, unresolved references, skipped objects and the per-kind slot coverage (written to census.json)."""
    balances, definitions, parts = registry['balances'], registry['definitions'], registry['parts']
    unresolved = defaultdict(list)

    def check(what, path, expect, owner):
        reason = resolve_problem(census, path, expect)
        if reason:
            unresolved[what].append({'path': path, 'named_by': owner, 'reason': reason})

    for owner, what, path in problems:
        unresolved[f'{what} (does not decode)'].append({'path': path, 'named_by': owner, 'reason': 'does not decode'})
    for path, record in balances.items():
        for variant in record['variants']:
            for entries in variant['slots'].values():
                for candidate in entries:
                    check('part', candidate['part'], lambda n: n in PART_KINDS, path)
                    check('part list', candidate['list'], lambda n: n in LIST_CLASSES, path)
            for step in variant['merge']:
                check('part list', step['list'], lambda n: n in LIST_CLASSES, path)
        for item in record['definitions']:
            check('definition', item, lambda n: bool(definition_kind(n)), path)
        for maker in record['manufacturers']:
            check('manufacturer', maker['manufacturer'], lambda n: n == 'ManufacturerDefinition', path)
        for name in record['name_parts']['prefix'] + record['name_parts']['title']:
            check('name part', name, lambda n: n in NAME_PART_CLASSES, path)
        if record['gestalt']:
            check('gestalt', record['gestalt'], lambda n: n == 'GestaltSkeletalMeshDefinition', path)
    for path, record in definitions.items():
        for source in record['part_lists'].values():
            check('part list', source, lambda n: n in LIST_CLASSES, path)
        for name in record['prefix'] + record['title']:
            check('name part', name, lambda n: n in NAME_PART_CLASSES, path)
    for path, record in parts.items():
        for name in record['prefix'] + record['title']:
            check('name part', name, lambda n: n in NAME_PART_CLASSES, path)
    # De-duplicate (one reference named by several owners counts once, with its owners).
    merged_unresolved = {}
    for what, rows in unresolved.items():
        by_path = {}
        for row in rows:
            entry = by_path.setdefault(row['path'], {'path': row['path'], 'reason': row['reason'], 'named_by': []})
            if row['named_by'] not in entry['named_by']:
                entry['named_by'].append(row['named_by'])
        merged_unresolved[what] = sorted(by_path.values(), key=lambda r: r['path'])

    per_kind = defaultdict(Counter)
    slot_cover = defaultdict(lambda: defaultdict(set))  # kind -> slot -> distinct parts
    slot_empty = defaultdict(Counter)  # kind -> slot -> variants whose enabled slot ended with no candidate
    no_slots = Counter()
    for record in balances.values():
        kind = record['kind']
        per_kind[kind]['balances'] += 1
        per_kind[kind][f'chain_{record["chain_end"]}'] += 1
        per_kind[kind]['variants'] += len(record['variants'])
        if not record['manufacturers']:
            per_kind[kind]['balances_without_manufacturer'] += 1
        if 'inventory_definition_root_first' in record:
            per_kind[kind]['definition_differs_root_first'] += 1
        if 'manufacturers_root_first' in record:
            per_kind[kind]['manufacturers_differ_root_first'] += 1
        if not any(v['slots'] for v in record['variants']):
            no_slots[kind] += 1
        for variant in record['variants']:
            for slot, entries in variant['slots'].items():
                if entries:
                    slot_cover[kind][slot].update(c['part'] for c in entries)
                else:
                    slot_empty[kind][slot] += 1
    unresolved_values = Counter()
    constant_hints = Counter()
    for record in balances.values():
        for variant in record['variants']:
            for entries in variant['slots'].values():
                for c in entries:
                    for key in ('min_stage', 'max_stage'):
                        if isinstance(c[key], dict):
                            unresolved_values[key] += 1
                            constant_hints[key] += 'attribute_constant' in c[key]
                    weight = c['weight']
                    if weight['source'] == 'manufacturer':
                        values = list(weight['by_manufacturer'].values()) + [weight['default']]
                        if any(isinstance(v, dict) for v in values):
                            unresolved_values['weight'] += 1
    for kind in per_kind:
        per_kind[kind]['balances_with_no_part_slot'] = no_slots[kind]
    definitions_by_kind = Counter(r['kind'] for r in definitions.values())
    definitions_unused = Counter(r['kind'] for r in definitions.values() if not r['balances'])
    parts_by_kind = Counter(r['kind'] for r in parts.values())
    parts_unlisted = Counter(r['kind'] for r in parts.values() if not r['balances'])
    per_dlc = defaultdict(Counter)
    for record in balances.values():
        per_dlc[record['dlc']][f'balances_{record["kind"]}'] += 1
    for record in definitions.values():
        per_dlc[record['dlc']][f'definitions_{record["kind"]}'] += 1
    for record in parts.values():
        per_dlc[record['dlc']][f'parts_{record["kind"]}'] += 1
    for table_name in ('part_lists', 'name_parts'):
        for record in registry[table_name].values():
            per_dlc[record['dlc']][table_name] += 1
    holders_many = Counter()
    for path, files in census.holders.items():
        if len(files) > 1:
            holders_many[short(census.classes.get(path, '?'))] += 1
    skipped_all = dict(skipped)
    for name, count in census.class_defaults.items():
        skipped_all[f'{name}: class default object (Default__), not a game object'] = count
    return {
        'schema': SCHEMA_TAG + '/census', 'generated': registry['generated'],
        'objects_by_class': {name: len(paths) for name, paths in sorted(by_class.items())},
        'balances_by_kind': {k: dict(sorted(v.items())) for k, v in sorted(per_kind.items())},
        'definitions_by_kind': dict(sorted(definitions_by_kind.items())),
        'definitions_without_balance_by_kind': dict(sorted(definitions_unused.items())),
        'parts_by_kind': dict(sorted(parts_by_kind.items())),
        'parts_no_balance_lists_by_kind': dict(sorted(parts_unlisted.items())),
        'parts_per_slot': {k: {s: len(v[s]) for s in (WEAPON_SLOTS if k == 'weapon' else ITEM_SLOTS) if s in v}
                           for k, v in sorted(slot_cover.items())},
        'enabled_slots_left_empty': {k: dict(v) for k, v in sorted(slot_empty.items())},
        'candidate_values_needing_runtime_state': dict(unresolved_values),
        'of_which_attribute_constant_hint': dict(constant_hints),
        'by_dlc': {d: dict(sorted(c.items())) for d, c in sorted(per_dlc.items())},
        'paths_in_several_packages_by_class': dict(sorted(holders_many.items())),
        'paths_with_two_classes': census.conflicts,
        'skipped': dict(sorted(skipped_all.items())), 'skipped_examples': dict(skipped_examples),
        'decode_failures': census.failed,
        'part_list_rows_naming_no_part': {'rows': sum(census.null_part_entries.values()),
                                           'lists': dict(sorted(census.null_part_entries.items()))},
        'name_list_entries_empty': {'entries': sum(census.null_name_entries.values()),
                                    'owners': {k: v for k, v in sorted(census.null_name_entries.items()) if v}},
        'unresolved_counts': {k: len(v) for k, v in sorted(merged_unresolved.items())},
        'unresolved': merged_unresolved,
    }


# ---- checks against the existing tools ----------------------------------------------------------------------------------
def dedupe(entries):
    """[(part, weight)] -> {part: weight} as weapon_recipe.pick keeps them (later weight, first position, > MIN_WEIGHT)."""
    live = {}
    for part, weight in entries:
        if weight is None or weight > weapon_recipe.MIN_WEIGHT:
            live[part] = weight
    return live


def check_weapons(census, registry, stages=CHECK_STAGES):
    """Each weapon balance's candidates at `stages` against weapon_recipe.merge (parts and weights per slot)."""
    result = Counter()
    differ = []
    for path, record in registry['balances'].items():
        if record['kind'] != 'weapon' or record['chain_end'] != 'root':
            continue
        for stage in stages:
            try:
                theirs = weapon_recipe.merge(census, path, stage)
            except (KeyError, RuntimeError, AttributeError) as error:
                result['merge_raised'] += 1
                differ.append({'balance': path, 'stage': stage, 'error': str(error)[:200]})
                continue
            expected = {s: d for s, d in ((s, dedupe(e)) for s, e in theirs['merged'].items()) if d}
            ours = candidates_at(census, record['variants'][0]['slots'], stage, theirs['manufacturer'])
            # weapon_recipe keeps a row that names no part as a (None, weight) candidate; the registry counts and skips it.
            without_null = {s: {p: w for p, w in d.items() if p is not None} for s, d in expected.items()}
            without_null = {s: d for s, d in without_null.items() if d}
            if ours == expected:
                result['equal'] += 1
            elif ours == without_null:
                result['equal_after_rows_naming_no_part'] += 1
            else:
                result['different'] += 1
                differ.append({'balance': path, 'stage': stage})
    return {'counts': dict(result), 'different': differ}


def check_items(census, registry, stages=CHECK_STAGES):
    """Each single-variant item balance's drawable parts at `stages` against prepare_vendors.item_parts (parts per slot)."""
    result = Counter()
    differ = []
    for path, record in registry['balances'].items():
        if record['kind'] == 'weapon' or record['chain_end'] != 'root':
            continue
        if not record['inventory_definition']:
            result['not_compared_no_definition'] += 1
            continue
        if len(record['variants']) != 1 or record['variants'][0]['definition'] != record['inventory_definition']:
            result['not_compared_several_variants'] += 1
            continue
        steps = pv.base_chain(census, path)
        makers = [m['manufacturer'] for m in record['manufacturers'] if m['manufacturer']] or [None]
        for stage in stages:
            theirs = pv.item_parts(census, steps, record['inventory_definition'], stage)
            expected = {s: list(dict.fromkeys(p)) for s, p in theirs.items() if p}
            ours = {}
            for maker in makers:
                for slot, live in candidates_at(census, record['variants'][0]['slots'], stage, maker).items():
                    ours.setdefault(slot, {}).update({p: True for p in live})
            # prepare_vendors keeps a part any manufacturer can draw, in list order: rebuild that order from the merged list.
            ordered = {}
            for slot, entries in record['variants'][0]['slots'].items():
                keep = [c['part'] for c in entries if c['part'] in ours.get(slot, {})]
                if keep:
                    ordered[slot] = list(dict.fromkeys(keep))
            if ordered == expected:
                result['equal'] += 1
            else:
                result['different'] += 1
                differ.append({'balance': path, 'stage': stage})
    return {'counts': dict(result), 'different': differ}


def check_stock_shield(census, registry, balance=ps.DEFAULT_BALANCE, level=SHIELD_CHECK_LEVEL):
    """The registry's entry for the stock shield balance against prepare_stock_shield.candidates at the slice level."""
    record = registry['balances'].get(balance)
    if record is None:
        return {'balance': balance, 'present': False}
    theirs = ps.candidates(census, balance, level)
    maker = record['manufacturers'][0]['manufacturer'] if record['manufacturers'] else None
    ours = candidates_at(census, record['variants'][0]['slots'], level, maker)
    return {'balance': balance, 'present': True, 'level': level, 'equal': ours == theirs['slots'],
            'parts_per_slot': {s: len(v) for s, v in ours.items()},
            'combinations': math.prod(len(v) for v in ours.values()) if ours else 0,
            'tool_combinations': math.prod(len(v) for v in theirs['slots'].values()) if theirs['slots'] else 0}


# ---- driver -------------------------------------------------------------------------------------------------------------
def open_census(reader, game, schema, jobs=8, files=None):
    """Scan every package (weapon_catalog.package_files) into a RegistryCensus. -> (census, {file: dlc}, scan errors)."""
    game = Path(game)
    files = weapon_catalog.package_files(game) if files is None else files
    with concurrent.futures.ThreadPoolExecutor(max_workers=jobs) as pool:
        scanned = list(pool.map(lambda item: (item, weapon_catalog.scan(reader, item[0])), files))
    census = RegistryCensus(reader, schema)
    census.game = game
    dlc_by_file, errors = {}, {}
    for (file, dlc), (exports, error) in scanned:
        dlc_by_file[file] = dlc
        if error:
            errors[Path(file).relative_to(game).as_posix()] = error
            continue
        for path, cls in exports:
            census.add(path, cls, file)
    return census, dlc_by_file, errors


def run(reader, game, out_dir, jobs=8, checks=True):
    """Build, check and write the registry. Returns the census report (also written)."""
    started = time.perf_counter()
    out_dir.mkdir(parents=True, exist_ok=True)
    schema = out_dir / 'item_registry.schema'
    schema.write_text('\n'.join(SCHEMA_LINES) + '\n', encoding='utf-8')
    census, dlc_by_file, errors = open_census(reader, game, schema.resolve(), jobs)
    registry, report = build(census, dlc_by_file)
    report['packages_scanned'] = len(dlc_by_file)
    report['scan_errors'] = errors
    report['duplicate_export_paths_all_classes'] = census.duplicates
    if checks:
        report['check_weapons_against_weapon_recipe'] = check_weapons(census, registry)
        report['check_items_against_prepare_vendors'] = check_items(census, registry)
        report['check_stock_shield'] = check_stock_shield(census, registry)
    report['decode_failures'] = census.failed
    report['seconds'] = round(time.perf_counter() - started, 1)
    write_json(out_dir / 'registry.json', registry)
    write_json(out_dir / 'census.json', report)
    return report


def write_json(path, data):
    temporary = path.with_suffix(path.suffix + '.tmp')
    temporary.write_text(json.dumps(data, ensure_ascii=False, separators=(',', ':')), encoding='utf-8')
    temporary.replace(path)


def summary_lines(report):
    """Counts only (no object names): what the record docs/verification/ITEM_REGISTRY.md may quote."""
    lines = [f"packages scanned {report.get('packages_scanned')}, scan errors {len(report.get('scan_errors') or {})}, "
             f"seconds {report.get('seconds')}"]
    for kind, counts in report['balances_by_kind'].items():
        lines.append(f'balances {kind}: ' + ', '.join(f'{k} {v}' for k, v in counts.items()))
    lines.append('definitions by kind: ' + json.dumps(report['definitions_by_kind']))
    lines.append('definitions without a balance: ' + json.dumps(report['definitions_without_balance_by_kind']))
    lines.append('parts by kind: ' + json.dumps(report['parts_by_kind']))
    lines.append('parts no balance lists: ' + json.dumps(report['parts_no_balance_lists_by_kind']))
    for kind, slots in report['parts_per_slot'].items():
        lines.append(f'parts per slot {kind}: ' + json.dumps(slots))
    lines.append('enabled slots left empty: ' + json.dumps(report['enabled_slots_left_empty']))
    lines.append('candidate values needing runtime state: ' + json.dumps(report['candidate_values_needing_runtime_state']))
    for dlc, counts in report['by_dlc'].items():
        lines.append(f'dlc {dlc}: ' + json.dumps(counts))
    lines.append('objects by class: ' + json.dumps(report['objects_by_class']))
    lines.append('paths held by several packages, by class: ' + json.dumps(report['paths_in_several_packages_by_class']))
    lines.append(f"paths with two classes: {len(report['paths_with_two_classes'])}")
    lines.append('skipped: ' + json.dumps(report['skipped']))
    lines.append(f"decode failures: {len(report['decode_failures'])}")
    lines.append(f"part list rows naming no part: {report['part_list_rows_naming_no_part']['rows']} in "
                 f"{len(report['part_list_rows_naming_no_part']['lists'])} lists; empty name list entries: "
                 f"{report['name_list_entries_empty']['entries']}")
    lines.append('of which an attribute constant hint: ' + json.dumps(report['of_which_attribute_constant_hint']))
    lines.append('unresolved references: ' + json.dumps(report['unresolved_counts']))
    for key in ('check_weapons_against_weapon_recipe', 'check_items_against_prepare_vendors'):
        if key in report:
            lines.append(f'{key}: ' + json.dumps(report[key]['counts']))
    if 'check_stock_shield' in report:
        shield = dict(report['check_stock_shield'])
        shield.pop('balance', None)
        lines.append('check_stock_shield: ' + json.dumps(shield))
    return lines


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', default=str(DEFAULT_READER), help='ow-package executable')
    parser.add_argument('--game', default=str(weapon_catalog.DEFAULT_GAME), help='Borderlands 2 install directory')
    parser.add_argument('--out', default=DEFAULT_OUT, help='output folder inside local/ (default %(default)s)')
    parser.add_argument('--jobs', type=int, default=8)
    parser.add_argument('--no-checks', action='store_true', help='skip the comparisons with the existing tools')
    args = parser.parse_args(argv)
    try:
        out_dir = pv.local_path(args.out)
    except ValueError as error:
        print(error, file=sys.stderr)
        return 2
    reader = Path(args.reader).resolve()
    if not reader.is_file():
        print(f'{reader}: ow-package not found (build the core first)', file=sys.stderr)
        return 2
    if not (Path(args.game) / 'WillowGame' / 'CookedPCConsole').is_dir():
        print(f'{args.game}: not a Borderlands 2 install (no WillowGame/CookedPCConsole)', file=sys.stderr)
        return 2
    report = run(str(reader), Path(args.game), out_dir, args.jobs, checks=not args.no_checks)
    for line in summary_lines(report):
        print(line)
    print(f'wrote {out_dir / "registry.json"} and {out_dir / "census.json"}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
