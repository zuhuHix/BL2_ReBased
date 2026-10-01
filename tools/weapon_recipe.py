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
- Weights using an AttributeInitializationDefinition ValueFormula are
  evaluated as Multiplier * Level ^ Power, clamped by RangeRestriction.

UNVERIFIED rules (no public spec; flagged in every recipe):
- Merge order root -> leaf. EPRM_Selective replaces enabled slots, EPRM_Additive
  appends to them, EPRM_Complete replaces every slot. A missing mode is Additive.
  Checked 2026-10-01 against the running game's RuntimePartListCollection for 243
  of 249 Startup balances (tools/weapon_balance.py crosscheck); Complete does not
  occur there and stays unchecked.
- A slot whose candidates all weigh 0 picks uniformly among them.
- A part's Manufacturers weight overrides DefaultWeight only when it names
  the weapon's manufacturer; manufacturer grade restrictions are ignored.
- Name: highest-Priority title from the parts' TitleList (weapon type as
  fallback) and highest-Priority prefix from their PrefixList, ties broken by
  the seed. Name parts whose Expressions test Weapon_Is_<Manufacturer> == 1
  are kept only for the root balance's first manufacturer; any other
  expression form excludes the name part.
"""
import argparse
import json
import os
import random
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


class Package:
    def __init__(self, reader, path, schema):
        self.reader, self.path, self.schema = reader, path, schema
        exports = json.loads(self.run('--exports'))
        self.index = {e['path']: e['index'] for e in exports}
        self.classes = {e['path']: e['class'] for e in exports}
        self.cache = {}

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


def attribute_value(package, init, level, attributes=None):
    """AttributeInitializationData -> float, or None if it needs runtime state.

    The base is the InitializationDefinition's ValueFormula result if set,
    else the BaseValueAttribute's value from `attributes` if set, else
    BaseValueConstant; the base is then multiplied by BaseValueScaleConstant.
    `level` is kept for callers that only need constant weights.
    """
    attributes = attributes or {}
    definition = init.get('InitializationDefinition')
    attribute = init.get('BaseValueAttribute')
    if definition:
        props = package.props(definition)
        # Only the plain formula path is evaluated; other modes need runtime state.
        if (props.get('BaseValueMode') or 'BASEVALUE_InitializationDefSetsBaseValue') != 'BASEVALUE_InitializationDefSetsBaseValue':
            return None
        if (props.get('ConditionalInitialization') or {}).get('bEnabled'):
            return None
        formula = props.get('ValueFormula') or {}
        if not formula.get('bEnabled'):
            return None
        terms = {k: attribute_value(package, formula.get(k) or {}, level, attributes)
                 for k in ('Multiplier', 'Level', 'Power', 'Offset')}
        if None in terms.values():
            return None
        base = terms['Multiplier'] * (terms['Level'] ** terms['Power']) + terms['Offset']
        # Cooked data omits false flags; a restriction applies only when enabled
        # (the OpenBLCMM dump of Weight_2_Uncommon shows bEnable...=False).
        clamp = props.get('RangeRestriction') or {}
        if clamp.get('bEnableMinValueRestriction') and clamp.get('MinValue'):
            low = attribute_value(package, clamp['MinValue'], level, attributes)
            if low is not None:
                base = max(base, low)
        if clamp.get('bEnableMaxValueRestriction') and clamp.get('MaxValue'):
            high = attribute_value(package, clamp['MaxValue'], level, attributes)
            if high is not None:
                base = min(base, high)
    elif attribute:
        if attribute not in attributes:
            return None
        base = attributes[attribute]
    else:
        base = init.get('BaseValueConstant') or 0.0
    scale = init.get('BaseValueScaleConstant')
    return base * (1.0 if scale is None else scale)


def slot_candidates(package, collection, stage, manufacturer=None):
    """{slot: [(part, weight or None)]} for enabled slots of one part list.

    A WeightedPart's Manufacturers entry overrides DefaultWeightIndex only when
    it names the weapon's manufacturer. Entries with Manufacturer=None are not
    treated as wildcards: that would make DefaultWeight's rarity formulas dead
    data, while BL2 elemental-chance mods work by editing those formulas.
    UNVERIFIED precedence.
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
            # stage None lists every part regardless of game stage (crosschecks, legal-part lists).
            if stage is not None and ((low is not None and stage < low) or (high is not None and stage > high)):
                continue
            weight = at('DefaultWeight')
            for override in part.get('Manufacturers') or []:
                if manufacturer and override.get('Manufacturer') == manufacturer:
                    weight = at('DefaultWeight', override.get('DefaultWeightIndex', -1))
                    break
            entries.append((part['Part'], weight))
        slots[slot] = entries
    return slots, props.get('PartReplacementMode') or 'EPRM_Additive'


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
        weights = [w for _, w in entries]
        if any(w is None for w in weights):
            notes.append(f'{slot}: some weights need runtime attributes; treated as 1')
            weights = [1.0 if w is None else w for w in weights]
        if sum(weights) <= 0:
            weights = [1.0] * len(entries)
            notes.append(f'{slot}: all candidates weigh 0; picked uniformly (UNVERIFIED)')
        choice = rng.choices(range(len(entries)), weights)[0]
        parts[slot] = {'part': entries[choice][0], 'weight': weights[choice],
                       'candidates': [{'part': p, 'weight': w} for (p, _), w in zip(entries, weights)]}

    def applies(name_part):
        for expression in package.props(name_part).get('Expressions') or []:
            attribute = expression.get('AttributeOperand1') or ''
            if (expression.get('ComparisonOperator') != 'OPERATOR_EqualTo' or expression.get('AttributeOperand2')
                    or expression.get('ConstantOperand2') != 1 or '.Weapon_Is_' not in attribute):
                return False
            if not manufacturer or attribute.rsplit('Weapon_Is_', 1)[1] != manufacturer.rsplit('.', 1)[1]:
                return False
        return True

    def best(list_name):
        found = []
        for slot in parts.values():
            for name_part in package.props(slot['part']).get(list_name) or []:
                found.append(name_part)
        if not found and weapon_type:
            found = package.props(weapon_type).get(list_name) or []
        scored = [(package.props(n).get('Priority') or 0, package.props(n).get('PartName'), n)
                  for n in found if applies(n)]
        scored = [s for s in scored if s[1]]
        if not scored:
            return None
        top = max(s[0] for s in scored)
        return rng.choice(sorted(s for s in scored if s[0] == top))

    title, prefix = best('TitleList'), best('PrefixList')
    fragments = {slot: package.props(p['part']).get('GestaltModeSkeletalMeshName') for slot, p in parts.items()}
    material = package.props(parts['Material']['part']).get('Material') if 'Material' in parts else None
    return {
        'balance': balance, 'manufacturer': manufacturer, 'seed': seed, 'game_stage': stage, 'weapon_type': weapon_type,
        'gestalt': package.props(weapon_type).get('GestaltMesh') if weapon_type else None,
        'name': ' '.join(x for x in (prefix[1] if prefix else None, title[1] if title else None) if x),
        'title': title and {'part': title[2], 'text': title[1], 'priority': title[0]},
        'prefix': prefix and {'part': prefix[2], 'text': prefix[1], 'priority': prefix[0]},
        'parts': parts,
        'gestalt_fragments': sorted({f for f in fragments.values() if f}),
        'material': material,
        'merge': history,
        'notes': notes,
        'unverified_rules': ['part replacement semantics', 'uniform pick at zero weight',
                             'manufacturer grades ignored', 'name priority rule'],
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
