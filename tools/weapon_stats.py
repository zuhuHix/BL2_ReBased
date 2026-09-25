"""Evaluate item-card stats for a weapon recipe from installed attribute data.

Input is a tools/weapon_recipe.py recipe. The weapon type supplies base
values; every chosen part's WeaponAttributeEffects and AttributeSlotUpgrades,
plus the type's AttributeSlotEffects, modify them. All inputs are decoded from
the package; the combination rules below are UNVERIFIED (no public spec) and
are listed in every output.

- Per attribute: (base + sum PreAdd) * (1 + sum Scale) + sum PostAdd, with the
  bracket clamped at 0 before PostAdd.
- A slot effect applies BaseModifierValue + PerGradeUpgrade * grade, where the
  grade sums the parts' GradeIncrease for that SlotName.
- Attribute operands resolve for Weapon_Is_<Maker> (1 for the weapon's own
  manufacturer, else 0), WeaponLevel (the requested level) and attributes with
  a ConstantAttributeValueResolver. Anything else is reported as unresolved.
- Rarity is the highest Rarity value among the chosen parts (1 Common ..
  5 Legendary), each resolved through its ItemRarity attribute constant.
- Manufacturer grades on the balance and skill/class effects are not applied.
"""
import argparse
import json
from pathlib import Path

import weapon_recipe

ATTR = 'D_Attributes.Weapon.'
# WeaponTypeDefinition property -> attribute it initializes.
TYPE_BASES = {
    'InstantHitDamage': ATTR + 'WeaponDamage',
    'FireRate': ATTR + 'WeaponFireInterval',
    'ClipSize': ATTR + 'WeaponClipSize',
    'ReloadTime': ATTR + 'WeaponReloadSpeed',
    'Spread': ATTR + 'WeaponSpread',
}


def resolver_constant(package, attribute):
    """ConstantValue of an attribute's ConstantAttributeValueResolver subobject."""
    prefix = attribute + '.ConstantAttributeValueResolver_'
    for path in package.index:
        if path.startswith(prefix):
            return package.props(path).get('ConstantValue')
    return None


def evaluate(package, recipe, level):
    maker = (recipe.get('manufacturer') or '').rsplit('.', 1)[-1]
    known = {'D_Attributes.Weapon.WeaponLevel': float(level)}
    unresolved = set()

    def value(init):
        while True:
            result = weapon_recipe.attribute_value(package, init or {}, level, known)
            if result is not None:
                return result
            # Resolve whichever attribute blocked evaluation, then retry.
            missing = find_missing(init or {})
            if not missing:
                return None
            if '.Weapon_Is_' in missing:
                known[missing] = 1.0 if missing.rsplit('Weapon_Is_', 1)[1] == maker else 0.0
            else:
                constant = resolver_constant(package, missing)
                if constant is None:
                    unresolved.add(missing)
                    return None
                known[missing] = constant

    def find_missing(init):
        attribute = init.get('BaseValueAttribute')
        if attribute and attribute not in known:
            return attribute
        definition = init.get('InitializationDefinition')
        if definition:
            props = package.props(definition)
            nested = list((props.get('ValueFormula') or {}).values()) + list((props.get('RangeRestriction') or {}).values())
            for term in nested:
                if isinstance(term, dict):
                    found = find_missing(term)
                    if found:
                        return found
        return None

    weapon_type = package.props(recipe['weapon_type'])
    base = {}
    for field, attribute in TYPE_BASES.items():
        raw = weapon_type.get(field)
        base[attribute] = value(raw) if isinstance(raw, dict) else raw
    base[ATTR + 'WeaponShotCost'] = 1.0
    barrel = package.props(recipe['parts']['Barrel']['part']) if 'Barrel' in recipe['parts'] else {}
    if barrel.get('bIsSpinningEnabled') and barrel.get('SpinUpDuration'):
        base[ATTR + 'WeaponBarrelSpinUpDuration'] = value(barrel['SpinUpDuration'])

    mods, sources = {}, []

    def add(attribute, kind, amount, source):
        if amount is None:
            return
        bucket = mods.setdefault(attribute, {'MT_PreAdd': 0.0, 'MT_Scale': 0.0, 'MT_PostAdd': 0.0})
        bucket[kind] = bucket.get(kind, 0.0) + amount
        if amount:
            sources.append({'attribute': attribute, 'type': kind, 'value': amount, 'source': source})

    grades = {}
    for slot, choice in recipe['parts'].items():
        part = package.props(choice['part'])
        for effect in part.get('WeaponAttributeEffects') or []:
            add(effect['AttributeToModify'], effect['ModifierType'], value(effect.get('BaseModifierValue')), choice['part'])
        for upgrade in part.get('AttributeSlotUpgrades') or []:
            if upgrade.get('bActivateSlot', True):
                grades[upgrade['SlotName']] = grades.get(upgrade['SlotName'], 0) + (upgrade.get('GradeIncrease') or 0)
    for effect in weapon_type.get('AttributeSlotEffects') or []:
        grade = grades.get(effect.get('SlotName'), 0)
        per_grade = value(effect.get('PerGradeUpgrade')) or 0.0
        amount = (value(effect.get('BaseModifierValue')) or 0.0) + per_grade * grade
        add(effect['AttributeToModify'], effect['ModifierType'], amount, f"slot {effect.get('SlotName')} grade {grade}")

    final = {}
    for attribute in sorted(set(base) | set(mods)):
        start = base.get(attribute)
        if start is None:
            continue
        m = mods.get(attribute, {})
        final[attribute] = max(0.0, (start + m.get('MT_PreAdd', 0.0)) * (1.0 + m.get('MT_Scale', 0.0))) + m.get('MT_PostAdd', 0.0)

    def stat(name):
        return final.get(ATTR + name)
    interval = stat('WeaponFireInterval')
    elemental = recipe['parts'].get('Elemental', {}).get('part', '')
    rarities = [value(package.props(choice['part']).get('Rarity'))
                for choice in recipe['parts'].values() if package.props(choice['part']).get('Rarity')]
    rarities = [r for r in rarities if r is not None]
    card = {
        'name': recipe.get('name'),
        'level': level,
        'damage': stat('WeaponDamage'),
        'fire_rate': 1.0 / interval if interval else None,
        'reload_time': stat('WeaponReloadSpeed'),
        'magazine': stat('WeaponClipSize'),
        'spread': stat('WeaponSpread'),
        'shot_cost': stat('WeaponShotCost'),
        'spin_up': stat('WeaponBarrelSpinUpDuration'),
        'element': elemental.rsplit('_', 1)[-1] if elemental else None,
        'rarity': int(max(rarities)) if rarities else None,
        'manufacturer': maker or None,
    }
    return {
        'card': card, 'attributes': final, 'grades': grades, 'modifiers': sources,
        'unresolved_attributes': sorted(unresolved),
        'unverified_rules': ['(base + PreAdd) * (1 + Scale) + PostAdd with 0 clamp',
                             'slot grade = sum of GradeIncrease', 'rarity = max over parts',
                             'manufacturer grades not applied',
                             'accuracy percentage not derived from spread'],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', required=True)
    parser.add_argument('--package', required=True)
    parser.add_argument('--recipe', required=True, help='tools/weapon_recipe.py output; stats are added to it')
    parser.add_argument('--level', type=int, default=30)
    args = parser.parse_args()
    recipe_path = Path(args.recipe)
    recipe = json.loads(recipe_path.read_text(encoding='utf-8'))
    schema = recipe_path.parent / 'weapon_stats.schema'
    schema.write_text('\n'.join(weapon_recipe.SCHEMA_LINES + [
        'WeaponAttributeEffects=StructProperty:AttributeEffectData',
        'AttributeSlotEffects=StructProperty:AttributeSlotEffectData',
        'AttributeSlotUpgrades=StructProperty:AttributeSlotUpgradeData',
    ]) + '\n', encoding='utf-8')
    package = weapon_recipe.Package(str(Path(args.reader).resolve()), Path(args.package), str(schema.resolve()))
    recipe['stats'] = evaluate(package, recipe, args.level)
    recipe_path.write_text(json.dumps(recipe, indent=1), encoding='utf-8')
    print(json.dumps(recipe['stats']['card'], indent=1))
    if recipe['stats']['unresolved_attributes']:
        print('unresolved:', recipe['stats']['unresolved_attributes'])


if __name__ == '__main__':
    main()
