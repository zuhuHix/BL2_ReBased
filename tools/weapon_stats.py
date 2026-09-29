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

Card fields beyond the five main stats (docs/verification/INVENTORY_CARD_STATS.md):
- accuracy: the game's own presentation for WeaponSpread remaps spread linearly
  (0 -> 100, RemappingData.InputValueMx -> 0). Only the spread input is
  UNVERIFIED (see accuracy_known); the remap itself is read from the package.
- sale_value: the weapon type's MonetaryValue calculator, fed the product of the
  chosen parts' MonetaryValueMod and the item level, rounded down. Reproduces
  real cards exactly only for the shotgun, assault rifle and SMG calculators;
  see PRICE_CALCULATORS_CHECKED.
- fun_stats: the red flavour line, the title part's CustomPresentations text.
  White stat lines (zoom, ammo per shot, ...) are not derived.
"""
import argparse
import json
import math
from pathlib import Path

import skill_stats
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

ACCURACY_PRESENTATION = 'GD_AttributePresentation.Weapons.AttrPresent_WeaponSpread'
PART_PRICE_TOTAL = 'D_Attributes.Inventory.InventoryPartMonetaryValueModifierTotal'
# Price calculators whose output was compared with real item cards (observed
# 2026-09-29 through the UI trace, Maya level 45 player, parts unknown so every
# part combination was tried): an integer-exact match exists for each observed
# shotgun, assault rifle and SMG; the two launchers (Nukem, Pyrophobia) have
# none; pistols and sniper rifles were not observed. Only checked ones are known.
PRICE_CALCULATORS_CHECKED = {'GD_Economy.PriceCalc.Init_Gun_' + name + '_PriceCalculator'
                             for name in ('Shotguns', 'AssaultRifles', 'SMG')}
# Title parts' CustomPresentations carry the red flavour line, TextColor (220, 70, 70).
RED_TEXT_COLOR = {'R': 220, 'G': 70, 'B': 70}


def resolver_constant(package, attribute):
    """ConstantValue of an attribute's ConstantAttributeValueResolver subobject."""
    prefix = attribute + '.ConstantAttributeValueResolver_'
    for path in package.index:
        if path.startswith(prefix):
            return package.props(path).get('ConstantValue')
    return None


def accuracy_percent(package, spread):
    """Spread -> the card's Accuracy percent via the game's own remap, or None.

    AttrPresent_WeaponSpread remaps [InputValueMn (0), InputValueMx (15)] onto
    [OutputValueMn (100), OutputValueMx (0)]. Orientation (input minimum ->
    output minimum) is inferred: the reverse would put every real card near 10%.
    Clamping to the output range is UNVERIFIED.
    """
    if spread is None or ACCURACY_PRESENTATION not in package.index:
        return None
    remap = package.props(ACCURACY_PRESENTATION).get('RemappingData')
    if not isinstance(remap, dict):
        return None

    def constant(name):
        return (remap.get(name) or {}).get('BaseValueConstant') or 0.0
    in_low, in_high = constant('InputValueMn'), constant('InputValueMx')
    out_low, out_high = constant('OutputValueMn'), constant('OutputValueMx')
    if in_high == in_low:
        return None
    t = min(1.0, max(0.0, (spread - in_low) / (in_high - in_low)))
    return out_low + (out_high - out_low) * t


def price_init(package, init, known, unresolved):
    """AttributeInitializationData -> float for the price calculators, or None.

    Differs from weapon_recipe.attribute_value in that the definition may scale
    a base attribute (BASEVALUE_InitializationDefScalesBaseValue). Range
    restrictions and conditional initialization are ignored (UNVERIFIED).
    """
    init = init or {}
    attribute, definition = init.get('BaseValueAttribute'), init.get('InitializationDefinition')
    base = init.get('BaseValueConstant') or 0.0
    if attribute:
        base = known[attribute] if attribute in known else resolver_constant(package, attribute)
        if base is None:
            unresolved.add(attribute)
            return None
    if definition:
        props = package.props(definition)
        mode = props.get('BaseValueMode') or 'BASEVALUE_InitializationDefSetsBaseValue'
        formula = props.get('ValueFormula') or {}
        value = 0.0
        if formula.get('bEnabled'):
            terms = {k: price_init(package, formula.get(k), known, unresolved)
                     for k in ('Multiplier', 'Level', 'Power', 'Offset')}
            if None in terms.values():
                return None
            value = terms['Multiplier'] * terms['Level'] ** terms['Power'] + terms['Offset']
        if mode == 'BASEVALUE_InitializationDefScalesBaseValue':
            base *= value
        elif mode == 'BASEVALUE_InitializationDefSetsBaseValue':
            base = value
        else:
            unresolved.add(f'{definition}: {mode}')
            return None
    scale = init.get('BaseValueScaleConstant')
    return base * (1.0 if scale is None else scale)


def sale_value(package, recipe, weapon_type, level, unresolved):
    """(rounded-down sale price, whether the calculator was checked), or (None, False)."""
    init = weapon_type.get('MonetaryValue')
    if not isinstance(init, dict):
        return None, False
    total = 1.0
    for choice in recipe['parts'].values():
        modifier = package.props(choice['part']).get('MonetaryValueMod')
        if not modifier:
            continue
        factor = resolver_constant(package, modifier)
        if factor is None:
            unresolved.add(modifier)
            return None, False
        total *= factor
    known = {PART_PRICE_TOTAL: total, ATTR + 'WeaponLevel': float(level)}
    value = price_init(package, init, known, unresolved)
    if value is None:
        return None, False
    return math.floor(value), init.get('InitializationDefinition') in PRICE_CALCULATORS_CHECKED


def red_text(package, recipe, localize=None):
    """The title part's red flavour line(s), joined with '; ' (the gear schema), or None.

    `localize(presentation_path, default)` may return the installed English
    override for NoConstraintText. ';' inside a line becomes ',' because the
    HUD splits funStats on it.
    """
    title = (recipe.get('title') or {}).get('part')
    if not title:
        return None
    lines = []
    for path in package.props(title).get('CustomPresentations') or []:
        props = package.props(path)
        color = props.get('TextColor') or {}
        text = props.get('NoConstraintText')
        if localize:
            text = localize(path, text)
        if text and all(color.get(k) == v for k, v in RED_TEXT_COLOR.items()):
            lines.append(text.replace(';', ','))
    return '; '.join(lines) or None


def evaluate(package, recipe, level, localize=None):
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
    spin = bool(barrel.get('bIsSpinningEnabled'))
    if spin and barrel.get('SpinUpDuration'):
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
    price, price_checked = sale_value(package, recipe, weapon_type, level, unresolved)
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
        # How the spin-up gates firing, from the weapon type; None if the barrel does not spin.
        'spin_mode': weapon_type.get('BarrelSpinMode') if spin else None,
        # Cooked data omits the class default, 1 on Default__WeaponPartDefinition.
        'spin_start_interval_scale': barrel.get('StartingSpinUpFireIntervalMultiplier', 1.0) if spin else None,
        'element': elemental.rsplit('_', 1)[-1] if elemental else None,
        'rarity': int(max(rarities)) if rarities else None,
        'manufacturer': maker or None,
        'accuracy': accuracy_percent(package, stat('WeaponSpread')),
        # False: no real card reproduced the modelled spread (see the verification doc).
        'accuracy_known': False,
        'sale_value': price,
        'sale_value_known': price_checked,
        'fun_stats': red_text(package, recipe, localize),
    }
    return {
        'card': card, 'attributes': final, 'grades': grades, 'modifiers': sources,
        'unresolved_attributes': sorted(unresolved),
        'unverified_rules': ['(base + PreAdd) * (1 + Scale) + PostAdd with 0 clamp',
                             'slot grade = sum of GradeIncrease', 'rarity = max over parts',
                             'manufacturer grades not applied',
                             'accuracy = presentation remap of the UNVERIFIED spread, clamped to 0..100',
                             'sale value = floor(price calculator(part MonetaryValueMod product, level))',
                             'white stat lines of the fun text are not derived'],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', required=True)
    parser.add_argument('--package', required=True)
    parser.add_argument('--recipe', required=True, help='tools/weapon_recipe.py output; stats are added to it')
    parser.add_argument('--game', help='Borderlands 2 install; its INT files override the red text '
                        '(default: derived from --package when that folder exists)')
    parser.add_argument('--level', type=int, help="item level; defaults to the recipe's game_stage (the level it was rolled at)")
    args = parser.parse_args()
    recipe_path = Path(args.recipe)
    recipe = json.loads(recipe_path.read_text(encoding='utf-8'))
    schema = recipe_path.parent / 'weapon_stats.schema'
    schema.write_text('\n'.join(weapon_recipe.SCHEMA_LINES + [
        'WeaponAttributeEffects=StructProperty:AttributeEffectData',
        'AttributeSlotEffects=StructProperty:AttributeSlotEffectData',
        'AttributeSlotUpgrades=StructProperty:AttributeSlotUpgradeData',
        'CustomPresentations=ObjectProperty',
    ]) + '\n', encoding='utf-8')
    package = skill_stats.Package(str(Path(args.reader).resolve()), Path(args.package), str(schema.resolve()))
    level = args.level if args.level is not None else int(recipe.get('game_stage') or 1)
    game = Path(args.game) if args.game else Path(args.package).resolve().parents[2]
    files = {}

    def localize(path, default):
        # <Package>.int overrides the cooked English string (skill_stats does the same).
        name = path.split('.', 1)[0]
        if name not in files:
            files[name] = skill_stats.localization_for(game, name)
        return files[name].get((skill_stats.localization_key(path, package.classes), 'NoConstraintText'), default)
    recipe['stats'] = evaluate(package, recipe, level, localize)
    recipe_path.write_text(json.dumps(recipe, indent=1), encoding='utf-8')
    print(json.dumps(recipe['stats']['card'], indent=1))
    if recipe['stats']['unresolved_attributes']:
        print('unresolved:', recipe['stats']['unresolved_attributes'])


if __name__ == '__main__':
    main()
