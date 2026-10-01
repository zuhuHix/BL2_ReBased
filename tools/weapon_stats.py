"""Evaluate item-card stats for a weapon recipe from installed attribute data.

Input is a tools/weapon_recipe.py recipe. The weapon type supplies base
values; every chosen part's WeaponAttributeEffects and AttributeSlotUpgrades,
plus the type's AttributeSlotEffects, modify them. All inputs are decoded from
the package; the combination rules below are UNVERIFIED (no public spec) and
are listed in every output.

- Per attribute (combine()): (base + sum PreAdd) * (1 + sum of positive Scales)
  / (1 + sum of |negative Scales|) + sum PostAdd. Fitted 2026-10-01: it reproduces
  every printed stat of four of six real cards with the parts their names imply,
  where the older (1 + sum Scale) rule reproduced one
  (docs/verification/WEAPON_BALANCE_DECODE.md). Still UNVERIFIED: the native
  code is not readable.
- The weapon type's own WeaponAttributeEffects apply like a part's. Bases a type
  leaves unset come from the class defaults (TYPE_DEFAULTS, WEAPON_DEFAULTS).
- A slot effect applies BaseModifierValue + PerGradeUpgrade * grade, where the
  grade sums the parts' GradeIncrease for that SlotName.
- Attribute operands resolve for Weapon_Is_<Maker> (1 for the weapon's own
  manufacturer, else 0), WeaponLevel (the requested level) and attributes with
  a ConstantAttributeValueResolver. Anything else is reported as unresolved.
- Rarity is the highest Rarity value among the chosen parts (1 Common ..
  5 Legendary), each resolved through its ItemRarity attribute constant.
- Manufacturer grades on the balance and skill/class effects are not applied.
- Status effect rows (status_rows()) and projectile count reproduce real cards;
  `display` holds the numbers rounded as the card prints them.

Card fields beyond the five main stats (docs/verification/INVENTORY_CARD_STATS.md):
- accuracy: the game's own presentation for WeaponSpread remaps spread linearly
  (0 -> 100, RemappingData.InputValueMx -> 0). With the 'split' rule the spread
  input reproduces all six audited cards (accuracy_known).
- sale_value: the weapon type's MonetaryValue calculator, fed the product of the
  chosen parts' MonetaryValueMod and the item level, rounded down. Reproduces
  real cards exactly for every weapon class except launchers; see
  PRICE_CALCULATORS_CHECKED.
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
    'ProjectilesPerShot': ATTR + 'WeaponProjectilesPerShot',
    'StatusEffectDamage': ATTR + 'WeaponStatusEffectDamage',
    'BaseStatusEffectChanceModifier': ATTR + 'WeaponBaseStatusEffectChanceModifier',
}
# The attribute -> WillowWeapon property names above are decoded from each attribute's
# ObjectPropertyAttributeValueResolver.PropertyName (D_Attributes.Weapon in Startup.upk);
# only FireRate -> FireInterval is a different name and stays an inference.
# Cooked types omit fields equal to the class default. These are the values of
# WillowGame.upk's Default__WeaponTypeDefinition, decoded with ow-package --properties
# (2026-10-01); the Maliwan pistol type, for one, sets neither FireRate nor ReloadTime.
TYPE_DEFAULTS = {
    'FireRate': 0.4,
    'ReloadTime': 2.1,
    'Spread': 0.01,
    'ProjectilesPerShot': 1,
    'BaseStatusEffectChanceModifier': {'BaseValueConstant': 1.0},
}
# WillowWeapon properties no weapon type sets, as printed by the game's own `obj dump`
# of WillowGame.Default__WillowWeapon (OpenBLCMM dumps, local oracle). The cooked
# Default__WillowWeapon holds them as IntAttributeProperty/FloatAttributeProperty tags,
# which ow-package does not decode yet, so the values are restated here.
WEAPON_DEFAULTS = {
    ATTR + 'WeaponShotCost': 1.0,
    ATTR + 'WeaponStatusEffectChanceModifier': 1.0,
    ATTR + 'WeaponProjectileSpeedMultiplier': 1.0,
}

ACCURACY_PRESENTATION = 'GD_AttributePresentation.Weapons.AttrPresent_WeaponSpread'
PART_PRICE_TOTAL = 'D_Attributes.Inventory.InventoryPartMonetaryValueModifierTotal'
# Price calculators whose output was compared with real item cards (parts unknown,
# so every part combination was tried): 2026-09-29, an integer-exact match exists for
# each observed shotgun, assault rifle and SMG; 2026-10-01 (tools/weapon_card_audit.py,
# docs/verification/WEAPON_BALANCE_DECODE.md) also for one pistol and one sniper rifle
# among the combinations that reproduce every other stat of the card. Launchers
# (Nukem, Pyrophobia, Big Badaboom) have none. Only checked ones are known.
PRICE_CALCULATORS_CHECKED = {'GD_Economy.PriceCalc.Init_Gun_' + name + '_PriceCalculator'
                             for name in ('Shotguns', 'AssaultRifles', 'SMG', 'Pistols', 'SniperRifles')}
SCHEMA_LINES = weapon_recipe.SCHEMA_LINES + [
    'WeaponAttributeEffects=StructProperty:AttributeEffectData',
    'AttributeSlotEffects=StructProperty:AttributeSlotEffectData',
    'AttributeSlotUpgrades=StructProperty:AttributeSlotUpgradeData',
    'CustomPresentations=ObjectProperty',
    'DamageSurfaceChanceModifiers=StructProperty:StatusEffectChanceModifier',
]
# Title parts' CustomPresentations carry the red flavour line, TextColor (220, 70, 70).
RED_TEXT_COLOR = {'R': 220, 'G': 70, 'B': 70}


def resolver_constant(package, attribute):
    """ConstantValue of an attribute's ConstantAttributeValueResolver subobject."""
    resolvers = getattr(package, '_constant_resolvers', None)
    if resolvers is None:
        # attribute path -> its resolver subobject path, built once (the export list is long).
        resolvers = {}
        for path in package.index:
            owner, _, name = path.rpartition('.')
            if name.startswith('ConstantAttributeValueResolver_'):
                resolvers.setdefault(owner, path)
        package._constant_resolvers = resolvers
    path = resolvers.get(attribute)
    return package.props(path).get('ConstantValue') if path else None


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


SCALE_RULE = 'split'


def combine(start, m):
    """One attribute's value from its base and summed modifiers (UNVERIFIED rule).

    'sum': (base + PreAdd) * (1 + sum of Scale), clamped at 0, + PostAdd.
    'split': positive Scales multiply, negative ones divide:
             (base + PreAdd) * (1 + sum up) / (1 + sum |down|), + PostAdd.
    """
    pre, post = m.get('MT_PreAdd', 0.0), m.get('MT_PostAdd', 0.0)
    if SCALE_RULE == 'split':
        scale = (1.0 + m.get('scale_up', 0.0)) / (1.0 + m.get('scale_down', 0.0))
    else:
        scale = 1.0 + m.get('MT_Scale', 0.0)
    return max(0.0, (start + pre) * scale) + post


def damage_type_of(package, recipe, weapon_type):
    """The elemental part's CustomDamageTypeDefinition, else any part's, else the type's default.

    Precedence between parts is UNVERIFIED; in the decoded data only elemental parts
    and a few unique barrels carry one.
    """
    ordered = sorted(recipe['parts'].items(), key=lambda item: item[0] != 'Elemental')
    for _, choice in ordered:
        custom = package.props(choice['part']).get('CustomDamageTypeDefinition')
        if custom:
            return custom
    return weapon_type.get('DefaultDamageTypeDefinition')


def status_rows(package, damage_type, final, value):
    """Status-effect card rows for the weapon's damage type ({} when it applies none).

    Reproduces the observed cards (docs/verification/WEAPON_BALANCE_DECODE.md):
    chance % = the status effect's Generic-surface BaseChance
               * WeaponBaseStatusEffectChanceModifier * WeaponStatusEffectChanceModifier,
    damage / sec = WeaponStatusEffectDamage (only for damage-over-time effects).
    The per-surface chances and the duration are listed for gameplay use; how the
    native code applies them per shot (e.g. GetFireIntervalChanceModifier) is UNVERIFIED.
    """
    effect_path = package.props(damage_type).get('StatusEffect') if damage_type else None
    if not effect_path:
        return {}
    effect = package.props(effect_path)
    factor = (final.get(ATTR + 'WeaponBaseStatusEffectChanceModifier', 1.0)
              * final.get(ATTR + 'WeaponStatusEffectChanceModifier', 1.0))
    surfaces = {}
    for row in effect.get('DamageSurfaceChanceModifiers') or []:
        chance = value(row.get('BaseChance'))
        if chance is not None:
            surfaces[(row.get('SurfaceType') or '').removeprefix('DMGSURFACE_').lower()] = chance * factor
    return {
        'status_effect': effect.get('StatusEffectType'),
        'status_effect_definition': effect_path,
        'status_chance': surfaces.get('generic'),
        'status_chance_by_surface': surfaces,
        'status_dps': final.get(ATTR + 'WeaponStatusEffectDamage') if effect.get('bDoesDamageOverTime') else None,
        'status_duration': value(effect.get('BaseDuration')),
    }


def half_up(number, digits=1):
    scale = 10 ** digits
    return math.floor(number * scale + 0.5) / scale


def display(card):
    """The numbers as the real item card prints them (rounding observed on 6 real cards, UNVERIFIED rule):
    damage rounded up, magazine rounded down, the rest to one decimal (half up)."""
    shown = {}
    if card.get('damage') is not None:
        shown['damage'] = math.ceil(card['damage'] - 1e-6)
    if card.get('magazine') is not None:
        shown['magazine'] = math.floor(card['magazine'] + 1e-6)
    for field in ('fire_rate', 'reload_time', 'accuracy', 'status_chance', 'status_dps'):
        if card.get(field) is not None:
            shown[field] = half_up(card[field])
    if card.get('projectiles') and card['projectiles'] > 1:
        shown['projectiles'] = card['projectiles']
    return shown


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
        raw = weapon_type.get(field, TYPE_DEFAULTS.get(field))
        base[attribute] = value(raw) if isinstance(raw, dict) else raw
    for attribute, default in WEAPON_DEFAULTS.items():
        if base.get(attribute) is None:
            base[attribute] = default
    barrel = package.props(recipe['parts']['Barrel']['part']) if 'Barrel' in recipe['parts'] else {}
    spin = bool(barrel.get('bIsSpinningEnabled'))
    if spin and barrel.get('SpinUpDuration'):
        base[ATTR + 'WeaponBarrelSpinUpDuration'] = value(barrel['SpinUpDuration'])

    mods, sources = {}, []

    def add(attribute, kind, amount, source):
        if amount is None:
            return
        bucket = mods.setdefault(attribute, {'MT_PreAdd': 0.0, 'MT_Scale': 0.0, 'MT_PostAdd': 0.0,
                                             'scale_up': 0.0, 'scale_down': 0.0})
        bucket[kind] = bucket.get(kind, 0.0) + amount
        if kind == 'MT_Scale':
            bucket['scale_up' if amount > 0 else 'scale_down'] += abs(amount)
        if amount:
            sources.append({'attribute': attribute, 'type': kind, 'value': amount, 'source': source})

    # The type's own effects (e.g. Maliwan +status chance, +1 shot cost). Applied like a
    # part's; the Maliwan sniper's StatusEffectDamage PreAdd reproduces a real card exactly.
    for effect in weapon_type.get('WeaponAttributeEffects') or []:
        add(effect['AttributeToModify'], effect['ModifierType'], value(effect.get('BaseModifierValue')),
            recipe['weapon_type'])
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
        final[attribute] = combine(start, mods.get(attribute, {}))

    def stat(name):
        return final.get(ATTR + name)
    interval = stat('WeaponFireInterval')
    elemental = recipe['parts'].get('Elemental', {}).get('part', '')
    price, price_checked = sale_value(package, recipe, weapon_type, level, unresolved)
    rarities = [value(package.props(choice['part']).get('Rarity'))
                for choice in recipe['parts'].values() if package.props(choice['part']).get('Rarity')]
    rarities = [r for r in rarities if r is not None]
    damage_type = damage_type_of(package, recipe, weapon_type)
    status = status_rows(package, damage_type, final, value)
    projectiles = stat('WeaponProjectilesPerShot')
    # A barrel's CustomFiringModeDefinition replaces the type's default (62 barrels carry one).
    firing_mode = barrel.get('CustomFiringModeDefinition') or weapon_type.get('DefaultFiringModeDefinition')
    bullet_speed = package.props(firing_mode).get('Speed') if firing_mode else None
    speed_scale = stat('WeaponProjectileSpeedMultiplier')
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
        # True since 2026-10-01: with the 'split' scale rule the modelled spread reproduces the
        # printed accuracy on all six audited real cards (WEAPON_BALANCE_DECODE.md); 'sum' did not.
        'accuracy_known': SCALE_RULE == 'split',
        'sale_value': price,
        'sale_value_known': price_checked,
        'fun_stats': red_text(package, recipe, localize),
        'projectiles': int(projectiles) if projectiles is not None else None,
        'firing_mode': firing_mode,
        # FiringModeDefinition.Speed (unreal units/s) * WeaponProjectileSpeedMultiplier: UNVERIFIED product.
        'projectile_speed': bullet_speed * speed_scale if bullet_speed and speed_scale is not None else None,
        'damage_type': damage_type,
        **status,
    }
    card['display'] = display(card)
    return {
        'card': card, 'attributes': final, 'grades': grades, 'modifiers': sources,
        'unresolved_attributes': sorted(unresolved),
        'unverified_rules': [f'scale rule {SCALE_RULE!r} (see combine())',
                             'slot grade = sum of GradeIncrease', 'rarity = max over parts',
                             'manufacturer grades not applied',
                             'accuracy = presentation remap of the UNVERIFIED spread, clamped to 0..100',
                             'sale value = floor(price calculator(part MonetaryValueMod product, level))',
                             'white stat lines of the fun text are not derived',
                             'status chance = Generic BaseChance * base * chance modifiers; per-shot use is native',
                             'display rounding (damage up, magazine down, rest one decimal)',
                             'damage type and firing-mode precedence; projectile speed product'],
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
    schema.write_text('\n'.join(SCHEMA_LINES) + '\n', encoding='utf-8')
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
