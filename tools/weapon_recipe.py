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
- A slot whose candidates all weigh 0 picks uniformly among them.
- Manufacturer grade restrictions and attribute-based weights are ignored.
- Name: highest-Priority title from the parts' TitleList (weapon type as
  fallback) and highest-Priority prefix from their PrefixList, ties broken by
  the seed. Name parts whose Expressions test Weapon_Is_<Manufacturer> == 1
  are kept only for the root balance's first manufacturer; any other
  expression form excludes the name part.
"""
import argparse
import json
import random
import subprocess
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


class Package:
    def __init__(self, reader, path, schema):
        self.reader, self.path, self.schema = reader, path, schema
        exports = json.loads(self.run('--exports'))
        self.index = {e['path']: e['index'] for e in exports}
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
            self.cache[path] = {p['name']: None if p.get('status') == 'unsupported' else plain(p['value'])
                                for p in data['properties']}
        return self.cache[path]


def attribute_value(package, init, level):
    """AttributeInitializationData -> float, or None if it needs runtime state."""
    if init.get('BaseValueAttribute'):
        return None
    value = (init.get('BaseValueConstant') or 0.0) * (init.get('BaseValueScaleConstant', 1.0) or 1.0)
    definition = init.get('InitializationDefinition')
    if definition:
        formula = package.props(definition).get('ValueFormula') or {}
        terms = {k: attribute_value(package, formula.get(k) or {}, level) for k in ('Multiplier', 'Level', 'Power')}
        if None in terms.values():
            return None
        value = terms['Multiplier'] * (terms['Level'] ** terms['Power'])
        clamp = package.props(definition).get('RangeRestriction') or {}
        low = attribute_value(package, clamp.get('MinValue') or {}, level) if clamp.get('MinValue') else None
        high = attribute_value(package, clamp.get('MaxValue') or {}, level) if clamp.get('MaxValue') else None
        if low is not None:
            value = max(value, low)
        if high is not None:
            value = min(value, high)
    return value


def slot_candidates(package, collection, stage):
    """{slot: [(part, weight or None)]} for enabled slots of one part list."""
    props = package.props(collection)
    consolidated = props.get('ConsolidatedAttributeInitData') or []
    slots = {}
    for slot in SLOTS:
        data = props.get(f'{slot}PartData')
        if not data or not data.get('bEnabled'):
            continue
        entries = []
        for part in data.get('WeightedParts') or []:
            def at(name):
                i = part.get(f'{name}Index', -1)
                return attribute_value(package, consolidated[i], stage) if 0 <= i < len(consolidated) else None
            low, high = at('MinGameStage'), at('MaxGameStage')
            if (low is not None and stage < low) or (high is not None and stage > high):
                continue
            entries.append((part['Part'], at('DefaultWeight')))
        slots[slot] = entries
    return slots, props.get('PartReplacementMode') or 'EPRM_Additive'


def roll(package, balance, seed, stage):
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
        slots, mode = slot_candidates(package, collection, stage)
        history.append({'balance': step, 'part_list': collection, 'mode': mode, 'slots': sorted(slots)})
        if mode == 'EPRM_Complete':
            merged = {}
        for slot, entries in slots.items():
            merged[slot] = merged.get(slot, []) + entries if mode == 'EPRM_Additive' else list(entries)

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
