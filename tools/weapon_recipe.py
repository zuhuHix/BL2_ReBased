"""Roll a BL2 weapon's parts from its installed balance data.

Reads a WeaponBalanceDefinition from a cooked package through `ow-package
--properties` and writes a JSON recipe: chosen part per slot, gestalt mesh
fragments, material instance, name and the rules that produced them. Nothing
is copied from the game into the repository; recipes go under ignored local/.

Data used (all decoded from the package):
- BaseDefinition chain; the root's InventoryDefinition (WeaponTypeDefinition).
- Each balance's WeaponPartListCollection: <Slot>PartData {bEnabled,
  WeightedParts[]} and PartReplacementMode, and ConsolidatedAttributeInitData,
  which MinGameStageIndex / MaxGameStageIndex / DefaultWeightIndex point into.
- AttributeInitializationData is evaluated as the game's evaluator does
  (attribute_value(); docs/verification/NATIVE_PROGRESSION.md section 1 and
  NATIVE_WEAPON_RULES.md section 1): ValueFormula Multiplier * (Level ^ Power +
  Offset), BaseValueMode, then BaseValueScaleConstant, then the enabled
  RangeRestriction, then RoundingMode, in single precision (f32()).

Rules read from the executable (docs/verification/NATIVE_WEAPON_RULES.md;
UNVERIFIED in game, flagged in every recipe):
- Merge order root -> leaf. EPRM_Selective replaces enabled slots, EPRM_Additive
  appends to them, EPRM_Complete replaces every slot. A missing mode is Additive.
  Checked 2026-10-01 against the running game's RuntimePartListCollection for 243
  of 249 Startup balances (tools/weapon_balance.py crosscheck); Complete does not
  occur there and stays unchecked.
- Part weight (entry_weight()): an entry with an empty Manufacturers list weighs
  a flat 100 (its DefaultWeightIndex is not read); otherwise the entry for the
  weapon's manufacturer (exact match, None is not a wildcard), else
  DefaultWeightIndex. Outside [trunc(min stage), trunc(max stage)] it weighs 0.
- Pick (pick()): entries weighing <= 1e-8 are dropped; a part listed twice keeps
  its later weight; r = total * u with u = rand()/32767, and the first entry with
  running sum <= r <= running sum + weight wins. A slot with no candidate left
  gets no part. The game's own random sequence is not reproduced (seeded here).
- Name (choose_name_parts()): deterministic, no random draw. Start from the
  weapon type's PrefixList/TitleList, then each part in slot order; within a list
  the qualifying entry (MinExpLevelRequirement <= level <= MaxExpLevelRequirement,
  Priority > 0, Expressions true) with the highest Priority wins, a later one on
  ties; a later list replaces the current pick when its Priority is >= the
  current one. Class defaults: Priority 1, level window 1..100. Only expressions
  of the form Weapon_Is_<Manufacturer> == 1 are evaluated here; any other form
  excludes the name part (UNVERIFIED shortcut, the game evaluates them all).
"""
import argparse
import json
import math
import os
import random
import struct
import subprocess
import tempfile
from pathlib import Path

SLOTS = ['Body', 'Grip', 'Barrel', 'Sight', 'Stock', 'Elemental', 'Accessory1', 'Accessory2', 'Material']
SCHEMA_LINES = [
    'WeightedParts=StructProperty:WeightedPart',
    'ConsolidatedAttributeInitData=StructProperty:AttributeInitializationData',
    'Manufacturers=StructProperty:ManufacturerGradeData',
    'TitleList=ObjectProperty',
    'PrefixList=ObjectProperty',
    'Expressions=StructProperty:AttributeExpressionData',
]


def plain(value):
    """ow-package property JSON -> Python values (structs become dicts)."""
    if isinstance(value, list):
        if value and all(isinstance(v, dict) and 'name' in v for v in value):
            return {v['name']: None if v.get('status') == 'unsupported' else plain(v.get('value')) for v in value}
        return [plain(v) for v in value]
    if isinstance(value, dict) and 'path' in value:
        return value['path']
    return value


def decoded(data):
    """One ow-package --properties JSON object -> {name: plain value}."""
    return {p['name']: None if p.get('status') == 'unsupported' else plain(p['value']) for p in data['properties']}


def static_arrays(data):
    """{name: [values in element order]} for every property; a fixed array reports one entry per element.

    decoded() keeps one value per name, so a fixed array there keeps only its last element.
    """
    found = {}
    for p in data['properties']:
        if p.get('status') != 'unsupported':
            found.setdefault(p['name'], []).append((p.get('array_index') or 0, plain(p['value'])))
    return {name: [value for _, value in sorted(items, key=lambda item: item[0])] for name, items in found.items()}


def part_fragments(package, part):
    """Gestalt fragment names one part draws, in the order the game lists them.

    The name in GestaltModeSkeletalMeshName plus the non-'None' AdditionalGestaltModeSkeletalMeshNames
    (body variants such as Pistol_Body_Maliwan_Var1). A part whose definition is named *_None (no sight,
    no elemental, no accessory) draws nothing even though its fields name a fragment.
    Evidence 2026-10-04 (UNVERIFIED rule, matched on the 6 slice guns): the running game's gestalt data of
    each spawned gun lists exactly these fragments and the triangle totals agree
    (docs/verification/WEAPON_VISUALS.md).
    """
    if part.rsplit('.', 1)[-1].lower().endswith('_none'):
        return []
    props = package.props(part)
    names = [props.get('GestaltModeSkeletalMeshName')]
    names += getattr(package, 'extra_names', {}).get(part, {}).get('AdditionalGestaltModeSkeletalMeshNames', [])
    return [n for n in names if n and n != 'None']


class Package:
    def __init__(self, reader, path, schema):
        self.reader, self.path, self.schema = reader, path, schema
        exports = json.loads(self.run('--exports'))
        self.index = {e['path']: e['index'] for e in exports}
        self.classes = {e['path']: e['class'] for e in exports}
        self.cache = {}
        self.extra_names = {}  # path -> {static-array property: [values in element order]}

    def run(self, *args):
        result = subprocess.run([self.reader, str(self.path), *map(str, args)], capture_output=True, text=True)
        if result.returncode:
            raise RuntimeError(f'ow-package {args}: {result.stderr.strip()}')
        return result.stdout

    def props(self, path):
        if path not in self.cache:
            if path not in self.index:
                raise KeyError(f'{path} is not an export of {self.path.name}')
            data = json.loads(self.run('--properties', self.index[path], '--property-offset', 4,
                                       '--array-schema', self.schema))
            self.cache[path] = decoded(data)
            self.extra_names[path] = static_arrays(data)
        return self.cache[path]

    def preload(self, paths):
        """Decode many exports in one ow-package process (--properties-batch) into the cache.

        Only a speed-up: the decoder is the same as props(). Exports that fail
        stay uncached, so props() raises for them as before.
        """
        todo = [p for p in dict.fromkeys(paths) if p in self.index and p not in self.cache]
        if not todo:
            return
        handle, name = tempfile.mkstemp(suffix='.txt')
        try:
            with os.fdopen(handle, 'w') as stream:
                stream.write('\n'.join(str(self.index[p]) for p in todo) + '\n')
            out = self.run('--properties-batch', name, '--property-offset', 4, '--array-schema', self.schema)
        finally:
            os.unlink(name)
        for line in out.splitlines():
            data = json.loads(line)
            if 'error' not in data:
                self.cache[data['path']] = decoded(data)
                self.extra_names[data['path']] = static_arrays(data)

    def crawl(self, roots, follow=lambda path, cls: True, limit=100000):
        """Preload `roots` and every export they reference (transitively) that `follow` accepts.

        Subobjects nobody references by path (an attribute's ConstantAttributeValueResolver_0)
        are not reached; callers look those up by prefix.
        """
        seen, frontier = set(), [r for r in roots if r in self.index]
        while frontier and len(seen) < limit:
            seen.update(frontier)
            self.preload(frontier)
            found = set()
            for path in frontier:
                for ref in references(self.cache.get(path)):
                    if ref in self.index and ref not in seen and follow(ref, self.classes.get(ref, '')):
                        found.add(ref)
            frontier = sorted(found)
        return seen


def references(value):
    """Every string inside a decoded value (candidate object paths)."""
    if isinstance(value, str):
        yield value
    elif isinstance(value, dict):
        for v in value.values():
            yield from references(v)
    elif isinstance(value, list):
        for v in value:
            yield from references(v)


def f32(value):
    """Round a Python float to IEEE single precision, as the game stores attribute values."""
    if value is None or math.isinf(value) or math.isnan(value):
        return value
    try:
        return struct.unpack('<f', struct.pack('<f', value))[0]
    except OverflowError:
        return math.copysign(math.inf, value)


def formula_value(multiplier, level, power, offset):
    """ValueFormula result: Multiplier * (Level ^ Power + Offset), in single precision.

    The offset is added before the multiplier and the power is skipped when it
    is 1, as the native evaluator reached from
    AttributeInitializationDefinition.EvaluateInitializationData does
    (docs/verification/NATIVE_PROGRESSION.md section 1; read from native code,
    UNVERIFIED in game). Each step is rounded to a float like the game's.
    """
    raised = level if power == 1 else f32(math.pow(level, power) if level >= 0 else level ** power)
    return f32(multiplier * f32(raised + offset))


ROUNDING = {
    # AttributeInitializationDefinition.EAttributeInitializationRounding, decoded from Engine.upk.
    'ATTRROUNDING_Float': lambda x: x,
    'ATTRROUNDING_IntRound': lambda x: float(math.floor(x + 0.5)),
    'ATTRROUNDING_IntFloor': lambda x: float(math.floor(x)),
    'ATTRROUNDING_IntCeil': lambda x: float(math.ceil(x)),
}
BASE_VALUE_MODES = {
    # EBaseValueMode (Engine.upk): how a definition's result f combines with the base b.
    'BASEVALUE_InitializationDefSetsBaseValue': lambda b, f: f,
    'BASEVALUE_InitializationDefAddsToBaseValue': lambda b, f: b + f,
    'BASEVALUE_InitializationDefScalesBaseValue': lambda b, f: b * f,
    'BASEVALUE_InitializationDefOffsetByBaseValue': lambda b, f: f - b,
}


def attribute_value(package, init, level, attributes=None):
    """AttributeInitializationData -> float, or None if it needs runtime state.

    Order read from the game's evaluator (NATIVE_PROGRESSION.md section 1):
    base = BaseValueAttribute's value when given (from `attributes`, else None),
    else BaseValueConstant; a definition's ValueFormula result f is combined with
    the base by BaseValueMode (a definition with neither a formula nor a
    conditional leaves the base alone); then times BaseValueScaleConstant; then,
    only with a definition, the enabled RangeRestriction min and max and the
    RoundingMode. Single precision throughout. ConditionalInitialization needs
    expression evaluation and returns None here. `level` is kept for callers.
    """
    attributes = attributes or {}
    definition = init.get('InitializationDefinition')
    attribute = init.get('BaseValueAttribute')
    if attribute:
        # None when the attribute needs runtime state; harmless if a definition replaces the base.
        base = f32(attributes[attribute]) if attribute in attributes else None
    else:
        base = f32(init.get('BaseValueConstant') or 0.0)
    props = package.props(definition) if definition else None
    if props is not None:
        if (props.get('ConditionalInitialization') or {}).get('bEnabled'):
            return None
        formula = props.get('ValueFormula') or {}
        if formula.get('bEnabled'):
            terms = {k: attribute_value(package, formula.get(k) or {}, level, attributes)
                     for k in ('Multiplier', 'Level', 'Power', 'Offset')}
            if None in terms.values():
                return None
            mode = props.get('BaseValueMode') or 'BASEVALUE_InitializationDefSetsBaseValue'
            if mode not in BASE_VALUE_MODES or (base is None and mode != 'BASEVALUE_InitializationDefSetsBaseValue'):
                return None
            base = f32(BASE_VALUE_MODES[mode](base, formula_value(
                terms['Multiplier'], terms['Level'], terms['Power'], terms['Offset'])))
    if base is None:
        return None
    scale = init.get('BaseValueScaleConstant')
    value = f32(base * (1.0 if scale is None else scale))
    if props is None:
        return value
    # Cooked data omits false flags; a restriction applies only when enabled
    # (the OpenBLCMM dump of Weight_2_Uncommon shows bEnable...=False).
    clamp = props.get('RangeRestriction') or {}
    if clamp.get('bEnableMinValueRestriction') and clamp.get('MinValue'):
        low = attribute_value(package, clamp['MinValue'], level, attributes)
        if low is not None:
            value = max(value, low)
    if clamp.get('bEnableMaxValueRestriction') and clamp.get('MaxValue'):
        high = attribute_value(package, clamp['MaxValue'], level, attributes)
        if high is not None:
            value = min(value, high)
    return f32(ROUNDING.get(props.get('RoundingMode') or 'ATTRROUNDING_Float', ROUNDING['ATTRROUNDING_Float'])(value))


# Weight of a WeightedPart whose Manufacturers list is empty (read from the part chooser;
# NATIVE_WEAPON_RULES.md section 3). 1,572 of 2,473 Startup weapon entries have such a list.
FLAT_WEIGHT = 100.0
# Entries at or below this weight never become candidates.
MIN_WEIGHT = 1e-8


def entry_weight(part, manufacturer, at):
    """One WeightedPart's weight before the game-stage window (UNVERIFIED in game).

    An empty Manufacturers list, or no known manufacturer, gives FLAT_WEIGHT.
    Otherwise the entry naming the weapon's manufacturer gives its weight index,
    and a list without it falls back to DefaultWeightIndex. Manufacturer=None
    entries are not wildcards (the game compares the pointer exactly).
    """
    overrides = part.get('Manufacturers') or []
    if not overrides or not manufacturer:
        return FLAT_WEIGHT
    for override in overrides:
        if override.get('Manufacturer') == manufacturer:
            return at('DefaultWeight', override.get('DefaultWeightIndex', -1))
    return at('DefaultWeight')


def slot_candidates(package, collection, stage, manufacturer=None):
    """{slot: [(part, weight or None)]} for enabled slots of one part list.

    Weights follow entry_weight(); a part outside [trunc(min), trunc(max)] of its
    game-stage window weighs 0 and is left out. Stage None lists every part
    regardless of game stage (crosschecks, legal-part lists).
    """
    props = package.props(collection)
    consolidated = props.get('ConsolidatedAttributeInitData') or []
    slots = {}
    for slot in SLOTS:
        data = props.get(f'{slot}PartData')
        if not data or not data.get('bEnabled'):
            continue
        entries = []
        for part in data.get('WeightedParts') or []:
            def at(name, index=None):
                i = part.get(f'{name}Index', -1) if index is None else index
                return attribute_value(package, consolidated[i], stage) if 0 <= i < len(consolidated) else None
            low, high = at('MinGameStage'), at('MaxGameStage')
            if stage is not None and ((low is not None and stage < int(low)) or (high is not None and int(high) < stage)):
                continue
            entries.append((part['Part'], entry_weight(part, manufacturer, at)))
        slots[slot] = entries
    return slots, props.get('PartReplacementMode') or 'EPRM_Additive'


def pick(entries, rng):
    """The game's weighted pick in one slot -> (part or None, candidates {part: weight}).

    Entries weighing <= MIN_WEIGHT are skipped (a zero-weight repeat does not
    remove an earlier entry); a part listed twice keeps its later positive
    weight at its first position; u is quantized like rand()/32767; the first
    entry whose running interval [sum, sum + weight] holds r = total * u wins.
    The game's random sequence itself is not reproduced (UNVERIFIED rule).
    """
    candidates = {}
    for part, weight in entries:
        if weight is not None and weight > MIN_WEIGHT:
            candidates[part] = weight
    total = sum(candidates.values())
    if not candidates:
        return None, candidates
    r = total * rng.randint(0, 32767) / 32767.0
    running = 0.0
    for part, weight in candidates.items():
        if running <= r <= running + weight:
            return part, candidates
        running += weight
    return part, candidates  # r == total after float rounding: the last entry


def name_part_qualifies(package, name_part, level, manufacturer):
    """Level window, positive priority and the manufacturer expression form (see module docstring)."""
    props = package.props(name_part)
    if not props.get('PartName'):
        return False
    low = props.get('MinExpLevelRequirement')
    high = props.get('MaxExpLevelRequirement')
    low = 1 if low is None else low
    high = 100 if high is None else high
    if level is not None and not (low <= level <= high):
        return False
    if not name_priority(package, name_part) > 0:
        return False
    for expression in props.get('Expressions') or []:
        attribute = expression.get('AttributeOperand1') or ''
        if (expression.get('ComparisonOperator') != 'OPERATOR_EqualTo' or expression.get('AttributeOperand2')
                or expression.get('ConstantOperand2') != 1 or '.Weapon_Is_' not in attribute):
            return False
        if not manufacturer or attribute.rsplit('Weapon_Is_', 1)[1] != manufacturer.rsplit('.', 1)[1]:
            return False
    return True


def name_priority(package, name_part):
    """Priority with the class default 1 that cooked data omits (Default__WeaponNamePartDefinition)."""
    value = package.props(name_part).get('Priority')
    return 1.0 if value is None else value


def best_name_part(package, names, level, manufacturer):
    """The qualifying entry of one TitleList/PrefixList with the highest priority, a later one on ties."""
    best = None
    for name_part in names or []:
        if name_part_qualifies(package, name_part, level, manufacturer) and (
                best is None or name_priority(package, best) <= name_priority(package, name_part)):
            best = name_part
    return best


def choose_name_parts(package, weapon_type, parts, manufacturer, level):
    """(prefix, title) name parts for {slot: part path}: type lists first, then parts in slot order."""
    chosen = {}
    for list_name in ('PrefixList', 'TitleList'):
        current = best_name_part(package, package.props(weapon_type).get(list_name), level, manufacturer) if weapon_type else None
        for slot in SLOTS:
            part = parts.get(slot)
            if not part:
                continue
            candidate = best_name_part(package, package.props(part).get(list_name), level, manufacturer)
            if current is None or (candidate is not None and
                                   name_priority(package, current) <= name_priority(package, candidate)):
                current = candidate
        chosen[list_name] = current
    return chosen['PrefixList'], chosen['TitleList']


def merge_slots(merged, slots, mode):
    """Apply one part list's enabled slots on top of `merged` (UNVERIFIED rule, see docstring)."""
    merged = {} if mode == 'EPRM_Complete' else dict(merged)
    for slot, entries in slots.items():
        merged[slot] = merged.get(slot, []) + entries if mode == 'EPRM_Additive' else list(entries)
    return merged


def merge(package, balance, stage):
    """Follow BaseDefinition to the root and merge every part list, root first.

    Returns {chain, weapon_type, manufacturer, merged: {slot: [(part, weight)]}, history}.
    The merge rule is the UNVERIFIED one in the module docstring; tools/weapon_balance.py
    compares its result with the game's own RuntimePartListCollection.
    """
    chain, cursor = [], balance
    while cursor:
        chain.append(cursor)
        cursor = package.props(cursor).get('BaseDefinition')
    chain.reverse()
    weapon_type = next((package.props(b).get('InventoryDefinition') for b in chain
                        if package.props(b).get('InventoryDefinition')), None)
    makers = next((package.props(b).get('Manufacturers') for b in chain if package.props(b).get('Manufacturers')), [])
    manufacturer = makers[0].get('Manufacturer') if makers else None
    merged, history = {}, []
    for step in chain:
        collection = package.props(step).get('WeaponPartListCollection')
        if not collection:
            continue
        slots, mode = slot_candidates(package, collection, stage, manufacturer)
        history.append({'balance': step, 'part_list': collection, 'mode': mode, 'slots': sorted(slots)})
        merged = merge_slots(merged, slots, mode)
    return {'chain': chain, 'weapon_type': weapon_type, 'manufacturer': manufacturer, 'merged': merged,
            'history': history, 'manufacturers': makers}


def roll(package, balance, seed, stage):
    merged_chain = merge(package, balance, stage)
    weapon_type, manufacturer = merged_chain['weapon_type'], merged_chain['manufacturer']
    merged, history = merged_chain['merged'], merged_chain['history']

    rng = random.Random(seed)
    parts, notes = {}, []
    for slot in SLOTS:
        entries = merged.get(slot) or []
        if not entries:
            continue
        if any(w is None for _, w in entries):
            notes.append(f'{slot}: some weights need runtime attributes; treated as 1')
            entries = [(p, 1.0 if w is None else w) for p, w in entries]
        choice, candidates = pick(entries, rng)
        if choice is None:
            notes.append(f'{slot}: every candidate weighs 0; the game leaves the slot empty')
            continue
        parts[slot] = {'part': choice, 'weight': candidates[choice],
                       'candidates': [{'part': p, 'weight': w} for p, w in candidates.items()]}

    # Item level = the game stage it was spawned at (bInterpolateExpLevel, the class default).
    prefix, title = choose_name_parts(package, weapon_type, {s: p['part'] for s, p in parts.items()},
                                      manufacturer, int(stage) if stage is not None else None)

    def described(name_part):
        return name_part and {'part': name_part, 'text': package.props(name_part).get('PartName'),
                              'priority': name_priority(package, name_part)}

    fragments = {slot: part_fragments(package, p['part']) for slot, p in parts.items()}
    material = package.props(parts['Material']['part']).get('Material') if 'Material' in parts else None
    title, prefix = described(title), described(prefix)
    return {
        'balance': balance, 'manufacturer': manufacturer, 'seed': seed, 'game_stage': stage, 'weapon_type': weapon_type,
        'gestalt': package.props(weapon_type).get('GestaltMesh') if weapon_type else None,
        'name': ' '.join(x for x in (prefix['text'] if prefix else None, title['text'] if title else None) if x),
        'title': title,
        'prefix': prefix,
        'parts': parts,
        'gestalt_fragments': sorted({f for names in fragments.values() for f in names}),
        'material': material,
        'merge': history,
        'notes': notes,
        'unverified_rules': ['part replacement semantics', 'part weights and pick read from native code',
                             'manufacturer grades ignored', 'name choice read from native code (expressions simplified)'],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', required=True, help='ow-package executable')
    parser.add_argument('--package', required=True, help='cooked package holding the balance, e.g. Startup.upk')
    parser.add_argument('--balance', required=True, help='e.g. GD_Weap_Pistol.A_Weapons_Legendary.Pistol_Vladof_5_Infinity')
    parser.add_argument('--seed', type=int, default=1)
    parser.add_argument('--stage', type=float, default=30, help='game stage (level) used for part filtering')
    parser.add_argument('--output', help='write JSON here instead of stdout')
    args = parser.parse_args()
    schema = Path(args.output or 'local/weapon_recipe.json').parent / 'weapon_recipe.schema'
    schema.parent.mkdir(parents=True, exist_ok=True)
    schema.write_text('\n'.join(SCHEMA_LINES) + '\n', encoding='utf-8')
    package = Package(str(Path(args.reader).resolve()), Path(args.package), str(schema.resolve()))
    recipe = roll(package, args.balance, args.seed, args.stage)
    text = json.dumps(recipe, indent=1)
    if args.output:
        Path(args.output).write_text(text, encoding='utf-8')
    else:
        print(text)


if __name__ == '__main__':
    main()
