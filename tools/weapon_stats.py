"""Evaluate item-card stats for a weapon recipe from installed attribute data.

Input is a tools/weapon_recipe.py recipe. The weapon type supplies base
values; the type's WeaponAttributeEffects, every chosen part's, the activated
attribute slots and the prefix/title name parts' effects modify them. All
inputs are decoded from the package. The rules were read from the game's own
code (docs/verification/NATIVE_WEAPON_RULES.md, 2026-10-02) and are UNVERIFIED
until a running-game check confirms them; each output lists them.

- Per attribute (combine(); NATIVE_WEAPON_RULES.md section 1): the modifier
  stack sums PreAdd, PostAdd, positive Scales and non-positive Scales separately
  (single precision), then value = (base + PreAdd) * (1 + up) / (1 - down) +
  PostAdd. No clamp. Integer attributes (INT_ATTRIBUTES: clip size, projectiles,
  shot cost, burst count) truncate toward zero. Before 2026-10-02 this was a rule
  fitted on cards ('split'); it is now the rule the game's code applies.
- Effect order (script WillowWeapon.InitializeInternal): type, parts in slot
  order, attribute slots, then prefix and title name parts. The type's
  WeaponAttributeEffects apply like a part's. Bases a type leaves unset come from
  the class defaults (TYPE_DEFAULTS, WEAPON_DEFAULTS). Plain float bases enter
  in single precision, like every other value in the stack.
- A slot effect applies BaseModifierValue + PerGradeUpgrade * grade to slots
  some upgrade activated (bActivateSlot); the grade sums every GradeIncrease for
  that SlotName on the type and the parts. The game also adds the type's
  AttributeSlotBaseGrade (default 1) when a slot is first activated; how that
  enters the value is not read (SLOT_BASE_GRADE_IN_VALUE, see the note).
- Attribute operands resolve for Weapon_Is_<Maker> (1 for the weapon's own
  manufacturer, else 0), WeaponLevel (the requested level) and attributes with
  a ConstantAttributeValueResolver. Anything else is reported as unresolved.
- Rarity (rarity_of()): level = trunc(type BaseRarity) + sum of trunc(part
  Rarity); the tier is the first GlobalsDefinition.RarityLevelColors entry whose
  [MinLevel, MaxLevel] holds it. `rarity` keeps the host's 1..5 scale.
- Manufacturer grades on the balance and skill/class effects are not applied.
- Status effect rows (status_rows()) and projectile count reproduce real cards;
  `display` holds the numbers rounded as the card prints them (present()).

Card fields beyond the five main stats (docs/verification/INVENTORY_CARD_STATS.md):
- accuracy: the game's own presentation for WeaponSpread remaps spread linearly
  (0 -> 100, RemappingData.InputValueMx -> 0), the input clamped to its range.
- sale_value: the weapon type's MonetaryValue calculator, fed the product of
  every part's MonetaryValueMod including the prefix and title name parts (the
  game recomputes the part value after choosing them) and the item level,
  truncated. See PRICE_CALCULATORS_CHECKED.
- level_requirement / level_line (level_requirement()): mission balances 0,
  else the item level minus the player's bonus, at least 1; the line is
  printed only above 1. Read from script; pass the recipe's `balance`.
- fun_stats: the red flavour line, the title part's CustomPresentations text.
  White stat lines (zoom, ammo per shot, ...) are not derived.
"""
import argparse
import json
import math
from pathlib import Path

import skill_stats
import weapon_recipe
from weapon_recipe import f32

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
# among the combinations that reproduce every other stat of the card. 2026-10-02: with
# the name parts' MonetaryValueMod in the part total (NATIVE_WEAPON_RULES.md section 6)
# both observed launchers match too. Only checked ones are known.
PRICE_CALCULATORS_CHECKED = {'GD_Economy.PriceCalc.Init_Gun_' + name + '_PriceCalculator'
                             for name in ('Shotguns', 'AssaultRifles', 'SMG', 'Pistols', 'SniperRifles', 'Launchers')}
# WillowWeapon properties declared IntAttributeProperty in WillowGame.upk: their stack
# result is truncated toward zero (NATIVE_WEAPON_RULES.md section 1).
INT_ATTRIBUTES = {ATTR + name for name in ('WeaponClipSize', 'WeaponProjectilesPerShot', 'WeaponShotCost',
                                           'WeaponAutomaticBurstCount')}
GLOBALS = 'GD_Globals.General.Globals'
# GlobalsDefinition.RarityLevelColors RarityRating -> the host's 1..5 rarity number
# (E-tech, rarity level 6, is rated VeryRare with its own colour; rarity_color carries it).
RARITY_NUMBER = {'RARITY_Common': 1, 'RARITY_Uncommon': 2, 'RARITY_Rare': 3, 'RARITY_VeryRare': 4,
                 'RARITY_Legendary': 5}
# The card's presentation per field, decoded from GD_AttributePresentation.Weapons (local), with the
# rounding mode and FloatPrecision the data sets (class defaults: IntRound, precision 1). evaluate()
# re-reads them from the package when the presentation exists; these are the fallbacks.
PRESENTATIONS = {
    'damage': ('GD_AttributePresentation.Weapons.AttrPresent_WeaponDamage', 'ATTRROUNDING_IntCeil'),
    'magazine': ('GD_AttributePresentation.Weapons.AttrPresent_WeaponClipSize', 'ATTRROUNDING_IntFloor'),
    'fire_rate': ('GD_AttributePresentation.Weapons.AttrPresent_WeaponFireRate', 'ATTRROUNDING_Float'),
    'reload_time': ('GD_AttributePresentation.Weapons.AttrPresent_WeaponReloadSpeed', 'ATTRROUNDING_Float'),
    'accuracy': (ACCURACY_PRESENTATION, 'ATTRROUNDING_Float'),
}
# The status rows' presentations (GD_AttributePresentation.Weapons_ElementalDamage, decoded locally):
# AttrPresent_Weapon<Element>CombinedStatusEffectChance and ...StatusEffectDamage, both RoundingMode
# Float with the default precision 1. The chance row remaps WeaponCombinedStatusEffectChanceModifier
# from [0, 5] onto [0, 100] (slope 20, equal to the Generic BaseChance status_rows() uses for fire,
# shock and corrosive; slag's is [0, 3.33], slope 30.03 against a BaseChance of 30, UNVERIFIED).
STATUS_ROUNDING = 'ATTRROUNDING_Float'
# How the activation base grade (type AttributeSlotBaseGrade, default 1) enters a slot's value
# is not read. False: value = BaseModifierValue + PerGradeUpgrade * sum(GradeIncrease), the form
# that reproduces the audited cards. True adds the base grade (compare with the audit).
SLOT_BASE_GRADE_IN_VALUE = False
SCHEMA_LINES = weapon_recipe.SCHEMA_LINES + [
    'WeaponAttributeEffects=StructProperty:AttributeEffectData',
    'AttributeSlotEffects=StructProperty:AttributeSlotEffectData',
    'AttributeSlotUpgrades=StructProperty:AttributeSlotUpgradeData',
    'CustomPresentations=ObjectProperty',
    'DamageSurfaceChanceModifiers=StructProperty:StatusEffectChanceModifier',
    'RarityLevelColors=StructProperty:RarityLevelColor',
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
    [OutputValueMn (100), OutputValueMx (0)]: the presentation's remap (read from
    native code, NATIVE_WEAPON_RULES.md section 2) clamps the input to its range
    and maps InputValueMn to OutputValueMn linearly; an empty input range maps
    everything to OutputValueMn. UNVERIFIED in game beyond the audited cards.
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
    if in_high <= in_low:
        return f32(out_low)
    slope = f32((out_high - out_low) / (in_high - in_low))
    clamped = min(in_high, max(in_low, spread))
    return f32((clamped - in_low) * slope + out_low)


def price_init(package, init, known, unresolved):
    """AttributeInitializationData -> float for the price calculators, or None.

    Like weapon_recipe.attribute_value (BaseValueMode, single precision), but it
    resolves base attributes from `known` or their constant resolvers and records
    what it cannot resolve. Range restrictions, rounding and conditional
    initialization are not applied (none occurs in the gun price chain).
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
        # A definition without an enabled formula leaves the base alone (the game's evaluator).
        if formula.get('bEnabled'):
            terms = {k: price_init(package, formula.get(k), known, unresolved)
                     for k in ('Multiplier', 'Level', 'Power', 'Offset')}
            if None in terms.values():
                return None
            value = weapon_recipe.formula_value(terms['Multiplier'], terms['Level'], terms['Power'], terms['Offset'])
            if mode not in weapon_recipe.BASE_VALUE_MODES:
                unresolved.add(f'{definition}: {mode}')
                return None
            base = f32(weapon_recipe.BASE_VALUE_MODES[mode](f32(base), value))
    scale = init.get('BaseValueScaleConstant')
    return f32(base * (1.0 if scale is None else scale))


def sale_value(package, recipe, weapon_type, level, unresolved):
    """(rounded-down sale price, whether the calculator was checked), or (None, False)."""
    init = weapon_type.get('MonetaryValue')
    if not isinstance(init, dict):
        return None, False
    total = 1.0
    # Every part in slot order, then the prefix and title name parts: the game recomputes the
    # part value after choosing the name parts (script WillowWeapon.InitializeInternal), and
    # launcher prefixes carry a MonetaryValueMod (Att_Price_RarityMultiplier_01_Common).
    for path in ordered_parts(recipe) + [p for p in (name_part_path(recipe.get('prefix')),
                                                     name_part_path(recipe.get('title'))) if p]:
        modifier = package.props(path).get('MonetaryValueMod')
        if not modifier:
            continue
        factor = resolver_constant(package, modifier)
        if factor is None:
            unresolved.add(modifier)
            return None, False
        total = f32(total * factor)
    known = {PART_PRICE_TOTAL: total, ATTR + 'WeaponLevel': float(level)}
    value = price_init(package, init, known, unresolved)
    if value is None:
        return None, False
    # The stored MonetaryValue is an integer; the conversion truncates (consistent with every card).
    return math.trunc(value), init.get('InitializationDefinition') in PRICE_CALCULATORS_CHECKED


def ordered_parts(recipe):
    """The recipe's part paths in the game's slot order (Body .. Material)."""
    parts = recipe.get('parts') or {}
    return [parts[slot]['part'] for slot in weapon_recipe.SLOTS if slot in parts and parts[slot].get('part')]


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


def combine(start, m, integer=False):
    """One attribute's value from its base and summed modifiers.

    'split' (default; read from the game's attribute properties, NATIVE_WEAPON_RULES.md
    section 1): positive Scales multiply and non-positive ones divide,
    (base + PreAdd) * (1 + up) / (1 + |down|) + PostAdd, no clamp. The sums arrive in
    single precision; the result is stored as a float, or truncated toward zero for an
    integer attribute (whose base is an integer).
    'sum' (the pre-2026-10-01 guess, kept for comparison): (base + PreAdd) * (1 + sum of
    Scale), clamped at 0, + PostAdd.
    """
    pre, post = m.get('MT_PreAdd', 0.0), m.get('MT_PostAdd', 0.0)
    if SCALE_RULE != 'split':
        return max(0.0, (start + pre) * (1.0 + m.get('MT_Scale', 0.0))) + post
    factor = (1.0 + m.get('scale_up', 0.0)) / (1.0 + m.get('scale_down', 0.0))
    if integer:
        return float(math.trunc(factor * (float(math.trunc(start)) + pre) + post))
    return f32((start + pre) * factor + post)


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
    """floor(number * 10^digits + 0.5) / 10^digits with every step in single precision.

    The game's rounding (NATIVE_WEAPON_RULES.md section 2) scales, adds 0.5 and floors on the x87
    unit, which runs at single precision in the game (UNVERIFIED how it is set; Direct3D 9 sets it
    so by default). So the scaled value is itself rounded to a float before the half is added:
    a stored 33.349998 times 10 is a tie that rounds to the even 333.5 and prints 33.4 (seen on a
    card), while a stored 1.7499998 times 10 stays below 17.5 and prints 1.7.
    """
    scale = f32(10.0 ** digits)
    return math.floor(f32(f32(number * scale) + 0.5)) / scale


def present(value, rounding, precision=1):
    """A stat as the card prints it (UAttributePresentationDefinition rounding, NATIVE_WEAPON_RULES.md section 2).

    Values under 1e-8 in size print as 0. ATTRROUNDING_Float rounds half up to
    `precision` decimals (FloatPrecision, clamped to 0..10) in single precision
    (half_up()); IntRound half up; IntFloor down; IntCeil up. The value is the
    stored single-precision float.
    """
    value = f32(value)
    if abs(value) < 1e-8:
        value = 0.0
    if rounding == 'ATTRROUNDING_IntCeil':
        return math.ceil(value)
    if rounding == 'ATTRROUNDING_IntFloor':
        return math.floor(value)
    if rounding == 'ATTRROUNDING_IntRound':
        return math.floor(value + 0.5)
    return half_up(value, max(0, min(10, precision)))


def display(card, rounding=None):
    """The numbers as the real item card prints them.

    `rounding` maps a card field to (rounding mode, precision); missing fields use
    the decoded presentation data in PRESENTATIONS (damage IntCeil, magazine
    IntFloor, fire rate/reload/accuracy Float with one decimal). Status rows use
    one decimal half up (STATUS_ROUNDING, from their presentation data).
    """
    rounding = rounding or {}
    shown = {}
    for field, (_, default) in PRESENTATIONS.items():
        if card.get(field) is not None:
            mode, precision = rounding.get(field, (default, 1))
            shown[field] = present(card[field], mode, precision)
    for field in ('status_chance', 'status_dps'):
        if card.get(field) is not None:
            shown[field] = present(card[field], STATUS_ROUNDING, 1)
    if card.get('projectiles') and card['projectiles'] > 1:
        shown['projectiles'] = card['projectiles']
    return shown


def presentation_rounding(package):
    """{card field: (rounding mode, FloatPrecision)} read from the package's presentations, where present."""
    found = {}
    for field, (path, default) in PRESENTATIONS.items():
        if path in getattr(package, 'index', {}):
            props = package.props(path)
            precision = props.get('FloatPrecision')
            found[field] = (props.get('RoundingMode') or 'ATTRROUNDING_IntRound', 1 if precision is None else precision)
    return found


def rarity_of(package, weapon_type, parts, value):
    """(rarity level, RarityRating, colour) from the type's BaseRarity and every part's Rarity (each truncated).

    The tier table is GlobalsDefinition.RarityLevelColors (first entry whose
    [MinLevel, MaxLevel] holds the level); without it the rating is None.
    """
    level = 0
    base = value(weapon_type.get('BaseRarity')) if weapon_type.get('BaseRarity') else 0.0
    level += math.trunc(base or 0.0)
    for part in parts:
        rarity = package.props(part).get('Rarity')
        amount = value(rarity) if rarity else None
        level += math.trunc(amount or 0.0)
    table = package.props(GLOBALS).get('RarityLevelColors') if GLOBALS in getattr(package, 'index', {}) else None
    for row in table or []:
        if (row.get('MinLevel') or 0) <= level <= (row.get('MaxLevel') or 0):
            color = row.get('Color') or {}
            return level, row.get('RarityRating'), '#%02X%02X%02X' % (color.get('R', 0), color.get('G', 0), color.get('B', 0))
    return level, None, None


MISSION_BALANCE_CLASS = 'MissionWeaponBalanceDefinition'


def level_requirement(package, recipe, weapon_type, level, level_bonus=0.0):
    """The level the item card asks for; the card prints its level line only when this is above 1.

    Read from script (NATIVE_WEAPON_RULES.md section 4, "Level requirement"): a weapon whose
    balance is a MissionWeaponBalanceDefinition requires 0; so does a type without
    bUsesPlayerLevelRequirement (every base-game weapon type sets it). Otherwise the item level
    minus floor(PlayerUseLevelBonus), at least 1. That bonus is evaluated on the player: in this
    data it is the player's GearLevelRequirementBonus attribute, 0 unless a skill or item raises
    it, so it is an argument here. The recipe's `balance` names the balance; without one the
    mission rule cannot apply. Over-level (OP) lines and DLC-restricted messages are not modelled.
    """
    balance = recipe.get('balance')
    balance_class = (getattr(package, 'classes', None) or {}).get(balance, '') if balance else ''
    if balance_class.rsplit('.', 1)[-1] == MISSION_BALANCE_CLASS:
        return 0
    if not weapon_type.get('bUsesPlayerLevelRequirement'):
        return 0
    return max(int(level) - math.floor(level_bonus), 1)


def name_part_path(entry):
    """A recipe's prefix/title entry ({'part': path} or a path) -> path or None."""
    if isinstance(entry, dict):
        return entry.get('part')
    return entry or None


def evaluate(package, recipe, level, localize=None, level_bonus=0.0):
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
        # A plain type field is a float property: its base enters the stack as a single-precision
        # value (2.1 is 2.0999999). This decides a half: a 2.1 reload under a -20 % scale is
        # 1.7499998 and prints 1.7, as the game prints it, where the double 2.1 gives 1.75 and 1.8.
        base[attribute] = value(raw) if isinstance(raw, dict) else (f32(raw) if raw is not None else None)
    for attribute, default in WEAPON_DEFAULTS.items():
        if base.get(attribute) is None:
            base[attribute] = default
    barrel = package.props(recipe['parts']['Barrel']['part']) if 'Barrel' in recipe['parts'] else {}
    spin = bool(barrel.get('bIsSpinningEnabled'))
    if spin and barrel.get('SpinUpDuration'):
        base[ATTR + 'WeaponBarrelSpinUpDuration'] = value(barrel['SpinUpDuration'])

    mods, sources = {}, []

    def add(attribute, kind, amount, source):
        # The game sums each modifier kind in single precision (NATIVE_WEAPON_RULES.md section 1).
        if amount is None:
            return
        amount = f32(amount)
        bucket = mods.setdefault(attribute, {'MT_PreAdd': 0.0, 'MT_Scale': 0.0, 'MT_PostAdd': 0.0,
                                             'scale_up': 0.0, 'scale_down': 0.0})
        bucket[kind] = f32(bucket.get(kind, 0.0) + amount)
        if kind == 'MT_Scale':
            side = 'scale_up' if amount > 0 else 'scale_down'
            bucket[side] = f32(bucket[side] + abs(amount))
        if amount:
            sources.append({'attribute': attribute, 'type': kind, 'value': amount, 'source': source})

    def apply_effects(path, props):
        for effect in props.get('WeaponAttributeEffects') or []:
            add(effect['AttributeToModify'], effect['ModifierType'], value(effect.get('BaseModifierValue')), path)

    # Order of script WillowWeapon.InitializeInternal: the type's own effects (e.g. Maliwan
    # +status chance, +1 shot cost), each part in slot order, the attribute slots, then the
    # prefix and title name parts.
    apply_effects(recipe['weapon_type'], weapon_type)
    parts = ordered_parts(recipe)
    grades, activated = {}, set()
    for path in [recipe['weapon_type']] + parts:
        props = weapon_type if path == recipe['weapon_type'] else package.props(path)
        if path != recipe['weapon_type']:
            apply_effects(path, props)
        # The type's own AttributeSlotUpgrades count first, then each part's (InitializeAttributeSlots).
        for upgrade in props.get('AttributeSlotUpgrades') or []:
            name = upgrade['SlotName']
            grades[name] = grades.get(name, 0) + (upgrade.get('GradeIncrease') or 0)
            if upgrade.get('bActivateSlot'):
                activated.add(name)
    base_grade = value(weapon_type.get('AttributeSlotBaseGrade') or {'BaseValueConstant': 1.0})
    for effect in weapon_type.get('AttributeSlotEffects') or []:
        name = effect.get('SlotName')
        # Only activated internal slots modify the weapon (external ones go to the owner's pools).
        if name not in activated or effect.get('bExternalSlot'):
            continue
        grade = grades.get(name, 0)
        if SLOT_BASE_GRADE_IN_VALUE:
            grade += math.floor((base_grade or 0.0) + 0.5)
        if effect.get('bEnforceMinimumGrade'):
            grade = max(grade, effect.get('MinimumGrade') or 0)
        if effect.get('bEnforceMaximumGrade'):
            grade = min(grade, effect.get('MaximumGrade') or 0)
        per_grade = value(effect.get('PerGradeUpgrade')) or 0.0
        amount = f32((value(effect.get('BaseModifierValue')) or 0.0) + f32(per_grade * grade))
        add(effect['AttributeToModify'], effect['ModifierType'], amount, f"slot {name} grade {grade}")
    for path in [p for p in (name_part_path(recipe.get('prefix')), name_part_path(recipe.get('title'))) if p]:
        apply_effects(path, package.props(path))

    final = {}
    for attribute in sorted(set(base) | set(mods)):
        start = base.get(attribute)
        if start is None:
            continue
        final[attribute] = combine(start, mods.get(attribute, {}), attribute in INT_ATTRIBUTES)

    def stat(name):
        return final.get(ATTR + name)
    interval = stat('WeaponFireInterval')
    elemental = recipe['parts'].get('Elemental', {}).get('part', '')
    price, price_checked = sale_value(package, recipe, weapon_type, level, unresolved)
    rarity_level, rarity_rating, rarity_color = rarity_of(package, weapon_type, parts, value)
    damage_type = damage_type_of(package, recipe, weapon_type)
    status = status_rows(package, damage_type, final, value)
    projectiles = stat('WeaponProjectilesPerShot')
    # A barrel's CustomFiringModeDefinition replaces the type's default (62 barrels carry one).
    firing_mode = barrel.get('CustomFiringModeDefinition') or weapon_type.get('DefaultFiringModeDefinition')
    bullet_speed = package.props(firing_mode).get('Speed') if firing_mode else None
    speed_scale = stat('WeaponProjectileSpeedMultiplier')
    if rarity_rating in RARITY_NUMBER:
        rarity = RARITY_NUMBER[rarity_rating]
    else:  # no table (synthetic data) or a level outside it: the old 1..5 clamp
        rarity = max(1, min(5, rarity_level)) if rarity_level else None
    required = level_requirement(package, recipe, weapon_type, level, level_bonus)
    card = {
        'name': recipe.get('name'),
        'level': level,
        # The card's level line: shown only when the requirement is above 1 (script, section 4).
        'level_requirement': required,
        'level_line': required > 1,
        'damage': stat('WeaponDamage'),
        # The card shows 1 / FireInterval (the presentation's bDisplayAsInverse).
        'fire_rate': f32(1.0 / interval) if interval else None,
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
        # Host scale 1 Common .. 5 Legendary; rarity_level / rarity_rating / rarity_color are the game's terms.
        'rarity': rarity,
        'rarity_level': rarity_level,
        'rarity_rating': rarity_rating,
        'rarity_color': rarity_color,
        'manufacturer': maker or None,
        'accuracy': accuracy_percent(package, stat('WeaponSpread')),
        # The spread model and the remap are read from the game's code (NATIVE_WEAPON_RULES.md).
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
    card['display'] = display(card, presentation_rounding(package))
    return {
        'card': card, 'attributes': final, 'grades': grades, 'modifiers': sources,
        'unresolved_attributes': sorted(unresolved),
        'unverified_rules': [f'scale rule {SCALE_RULE!r}: read from native code (see combine())',
                             'slot value = BaseModifierValue + PerGradeUpgrade * sum(GradeIncrease); '
                             'base-grade term not read (SLOT_BASE_GRADE_IN_VALUE)',
                             'rarity = sum of truncated part rarities, tier from RarityLevelColors (read)',
                             'manufacturer grades not applied',
                             'accuracy = presentation remap of the spread, input clamped (read)',
                             'sale value = trunc(price calculator(part and name-part MonetaryValueMod product, level))',
                             'white stat lines of the fun text are not derived',
                             'status chance = Generic BaseChance * base * chance modifiers; per-shot use is native',
                             'display rounding from the presentation data (read), in single precision '
                             '(x87 precision setting inferred from a card); status rows one decimal (data)',
                             'level line: requirement = 0 for mission balances, else max(level - player bonus, 1); '
                             'shown above 1 (read from script; player bonus assumed 0 unless given)',
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
