"""Write the weapon generator manifest that src/weapon_gen.cpp (ow-weapongen) reads.

The C++ generator reproduces tools/weapon_recipe.roll() (with CrtRand) and
tools/weapon_stats.evaluate() exactly. It reads no package itself: this tool
decodes Startup.upk with the existing Python helpers and writes every input those
two functions read into one line-based text file under ignored local/
(default local/weapons/gen/weapon_gen_manifest.txt). Nothing here goes into the
repository; the manifest holds game-derived values.

Format: UTF-8, one record per line, fields separated by a tab. In text fields a
backslash escapes '\\t' (tab), '\\n' (newline) and '\\\\' (backslash). '-' means
"none". Numbers are Python repr() of a double (exact round trip). Records refer
to each other by integer id; ids index a table per record kind, and a record may
refer to an id defined on a later line. The V line must come first, and each table's ids must be dense
(0..n-1, every id defined); the C++ reader rejects anything else.

  V  weapon_gen_manifest  <format version 4>   (version 1 lacked MonetaryBase / SaleCap / status_remap;
                                               version 2 wrote maker:<A,B> qualifiers and no NameExpr;
                                               version 3 had no E footer)
  A  <attr id>  <attribute path>  <ConstantValue of its ConstantAttributeValueResolver, or ->
  I  <init id>  <BaseValueConstant>  <attr id or -1>  <definition id or -1>  <BaseValueScaleConstant or ->
       one AttributeInitializationData; init 0 is the empty one ({}, evaluates to 0)
  D  <def id>  <path>  <BaseValueMode or ->  <conditional 0|1>  <formula enabled 0|1>
       <formula terms>  <min enabled 0|1>  <max enabled 0|1>  <clamp terms>  <RoundingMode or ->
       terms: comma list Key:<init id> in the order the package lists them (find_missing order), or -
  O  <object id>  <class>  <path>           every other object: balances, types, parts, name parts,
                                            presentations, damage types, status effects, firing modes, pools
  F  <object id>  <field>  <values...>      one field of an object (list below); repeatable fields repeat
  B  <balance object id>  <type object id>  <manufacturer path or ->  <mission 0|1>
  S  <balance object id>  <slot>  <part object id>  <weight>  <min-stage init or -1>  <max-stage init or -1>
  E  <record count>                         the last line (version 4): the number of records before it, V
                                            included. The C++ reader requires it from version 4 and rejects a
                                            wrong count or a record after it, so a truncated manifest fails to
                                            load; versions 1..3 still load without it.
       merged slot candidates in merge order (root first, EPRM_* applied); weight: 'flat' (empty
       Manufacturers list: 100), 'none' (index out of range), or the init id the weapon's
       manufacturer selects (its own entry, else DefaultWeightIndex), resolved here per balance
  G  <key>  <values...>                     globals: rarity rows, card rounding, accuracy remap, shot cost text,
                                            status_remap <damage type path> <in lo> <in hi> <out lo> <out hi>
                                            (the card chance row's presentation remap per damage type)

Object fields (F): Base <type field> init|num <value>; Effect <attribute> <modifier type> <init>;
SlotUpgrade <slot> <grade> <activate>; SlotEffect <slot> <attribute> <modifier> <base init> <per-grade init>
<external> <enforce min> <min> <enforce max> <max>; PrefixList / TitleList / CustomPresentations /
WeaponCardPresentations <comma object ids>; CardZoom; MonetaryValueMod <attr id>; MonetaryValue <init>;
MonetaryBase <num> (BaseMonetaryValueModifier, class default applied); SaleCap <int> (the FormOfCurrency cap); PriceChecked; Rarity <init>; BaseRarity <init>; SlotBaseGrade <init>; Gestalt; Material; Fragments <comma
names>; CustomDamageType / DefaultDamageType / CustomFiringMode / DefaultFiringMode / StatusEffect <object id>;
Spinning; SpinUpDuration <init>; SpinStartScale <num>; SpinMode; ZoomedEndFOV <num>; UsesLevelRequirement;
Name <text> <priority> <min level> <max level> <qualify: any | never | expr (version 2 also maker:<A,B>)>;
NameExpr <maker|damage> <key> <not matched> <ComparisonOperator> <constant> (one per expression of an 'expr'
name part, all must hold; maker: key = the manufacturer short name, value 1 for that maker else 0, not matched
'-'; damage: key = comma list <damage type object name>:<value if matched>, value = the first entry naming the
weapon's damage type, else <not matched>); Line <text> <color>
<red 0|1> <markup line>; DontDisplayNumber; DamageType <enum>; StatusEffectType; Surface <surface> <init>;
DamageOverTime; BaseDuration <init>; Speed <num>; pools: Quantity <num or ->; Gate <min> <max> (each '-'
absent, 'u' unresolved, or a number); Entry item|pool <object id> <weight or ->; Listed <pool object id or ->
<probability or -> (sources).

Choices (see the report in local/weapons/gen/ and the C++ comments):
- Attributes stay symbolic. A ConstantAttributeValueResolver's value is written next to the attribute, but
  the C++ applies it only where Python would: roll() evaluates weights with no attribute known; evaluate()
  resolves Weapon_Is_<Maker>, WeaponLevel and constants lazily; the price calculator uses its own lookup.
- Pool weights, quantities and gate bounds are evaluated here with loot_pools.Values (one player, not the
  editor context); none depends on the game stage, so the C++ computes the shares per stage itself.
- Presentation text uses the installed English .int overrides like weapon_stats.main().
"""
import argparse
import json
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import loot_pools  # noqa: E402
import skill_stats  # noqa: E402
import weapon_recipe  # noqa: E402
import weapon_stats  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
FORMAT_VERSION = 4
BALANCE_CLASSES = ('WillowGame.WeaponBalanceDefinition', 'WillowGame.MissionWeaponBalanceDefinition')
SOURCE_CLASSES = ('ItemPoolListDefinition', 'AIPawnBalanceDefinition')
RUNTIME_ATTRIBUTES = ('D_Attributes.Weapon.WeaponLevel', weapon_stats.PART_PRICE_TOTAL)


def text(value):
    """A text field with tab, newline and backslash escaped."""
    if value is None:
        return '-'
    return str(value).replace('\\', '\\\\').replace('\t', '\\t').replace('\n', '\\n').replace('\r', '')


def num(value):
    return '-' if value is None else repr(float(value))


def flag(value):
    return '1' if value else '0'


def is_pool_class(cls):
    return 'ItemPoolDefinition' in cls


class Builder:
    def __init__(self, package, localize):
        self.package, self.localize = package, localize
        self.lines = []
        self.attrs, self.inits, self.defs, self.objects = {}, {}, {}, {}
        self.described = set()
        self.init_count = 0
        self.weight_inits = set()
        self.init(None)  # init 0: the empty initializer

    # --- attributes, initializers, definitions -------------------------------------------
    def attr(self, path):
        if path not in self.attrs:
            self.attrs[path] = len(self.attrs)
            constant = weapon_stats.resolver_constant(self.package, path)
            self.lines.append(f'A\t{self.attrs[path]}\t{text(path)}\t{num(constant)}')
        return self.attrs[path]

    def init(self, data):
        data = data if isinstance(data, dict) else {}
        attribute = data.get('BaseValueAttribute')
        definition = data.get('InitializationDefinition')
        key = (data.get('BaseValueConstant') or 0.0, attribute or None, definition or None,
               data.get('BaseValueScaleConstant'))
        if key not in self.inits:
            attr_id = self.attr(attribute) if attribute else -1
            def_id = self.definition(definition) if definition else -1
            self.inits[key] = self.init_count
            self.init_count += 1
            self.lines.append(f'I\t{self.inits[key]}\t{num(key[0])}\t{attr_id}\t{def_id}\t{num(key[3])}')
        return self.inits[key]

    def terms(self, struct):
        """Dict-valued members of a ValueFormula / RangeRestriction, in decoded order."""
        found = [f'{k}:{self.init(v)}' for k, v in (struct or {}).items() if isinstance(v, dict)]
        return ','.join(found) or '-'

    def definition(self, path):
        if path not in self.defs:
            self.defs[path] = len(self.defs)
            props = self.package.props(path)
            formula = props.get('ValueFormula') or {}
            clamp = props.get('RangeRestriction') or {}
            conditional = (props.get('ConditionalInitialization') or {}).get('bEnabled')
            fields = [str(self.defs[path]), text(path), text(props.get('BaseValueMode')), flag(conditional),
                      flag(formula.get('bEnabled')), self.terms(formula),
                      flag(clamp.get('bEnableMinValueRestriction')), flag(clamp.get('bEnableMaxValueRestriction')),
                      self.terms(clamp), text(props.get('RoundingMode'))]
            self.lines.append('D\t' + '\t'.join(fields))
        return self.defs[path]

    # --- objects ------------------------------------------------------------------------
    def obj(self, path):
        if path not in self.objects:
            self.objects[path] = len(self.objects)
            cls = self.package.classes.get(path, '?')
            self.lines.append(f'O\t{self.objects[path]}\t{text(cls)}\t{text(path)}')
        return self.objects[path]

    def field(self, path, name, *values):
        self.lines.append('\t'.join(['F', str(self.obj(path)), name, *values]))

    def ids(self, paths):
        return ','.join(str(self.obj(p)) for p in paths) or '-'

    def common(self, path, props):
        """Fields every gear object may carry: effects, slot upgrades, names, presentations, price, rarity."""
        for effect in props.get('WeaponAttributeEffects') or []:
            self.field(path, 'Effect', text(effect['AttributeToModify']), text(effect['ModifierType']),
                       str(self.init(effect.get('BaseModifierValue'))))
        for upgrade in props.get('AttributeSlotUpgrades') or []:
            self.field(path, 'SlotUpgrade', text(upgrade['SlotName']), num(upgrade.get('GradeIncrease') or 0),
                       flag(upgrade.get('bActivateSlot')))
        for key in ('PrefixList', 'TitleList'):
            names = props.get(key) or []
            if names:
                for name in names:
                    self.name_part(name)
                self.field(path, key, self.ids(names))
        for key in ('CustomPresentations', 'WeaponCardPresentations'):
            shown = [item for item in props.get(key) or [] if item]  # null references have no text
            if shown:
                for item in shown:
                    self.presentation(item)
                self.field(path, key, self.ids(shown))
        if any(entry.get('Attribute') == weapon_stats.ZOOM_ATTRIBUTE for entry in props.get('WeaponCardAttributes') or []):
            self.field(path, 'CardZoom')
        if props.get('MonetaryValueMod'):
            self.field(path, 'MonetaryValueMod', str(self.attr(props['MonetaryValueMod'])))
        if props.get('Rarity'):
            self.field(path, 'Rarity', str(self.init(props['Rarity'])))

    def describe(self, path):
        """True the first time an object is described (its fields are written once)."""
        self.obj(path)
        if path in self.described:
            return False
        self.described.add(path)
        return True

    def name_part(self, path):
        if not self.describe(path):
            return
        props = self.package.props(path)
        expressions = [weapon_recipe.name_expression(self.package, e) for e in props.get('Expressions') or []]
        qualify = 'expr' if expressions else 'any'
        if (not props.get('PartName') or not weapon_recipe.name_priority(self.package, path) > 0
                or None in expressions):
            qualify, expressions = 'never', []
        low, high = props.get('MinExpLevelRequirement'), props.get('MaxExpLevelRequirement')
        self.field(path, 'Name', text(props.get('PartName')), num(weapon_recipe.name_priority(self.package, path)),
                   num(1 if low is None else low), num(100 if high is None else high), qualify)
        for (kind, data), operator, constant in expressions:
            if kind == 'maker':
                key, default = text(data), '-'
            else:
                matched, default = data
                key = ','.join(f'{text(name)}:{num(value)}' for name, value in matched) or '-'
                default = num(default)
            self.field(path, 'NameExpr', kind, key, default, operator, num(constant))
        self.common(path, props)

    def presentation(self, path):
        # A null reference ('None') or a missing export gets an object without fields; Python's
        # evaluate() raises on it, so guns that reach one are reported, not compared.
        if not self.describe(path) or path not in self.package.index:
            return
        props = self.package.props(path)
        line = weapon_stats.presentation_line(self.package, path, self.localize)
        if line:
            self.field(path, 'Line', text(line['text']), text(line['color']),
                       flag(weapon_stats._is_red(line['source_color'])), text(weapon_stats.fun_stats_markup([line])))
        if props.get('bDontDisplayNumber'):
            self.field(path, 'DontDisplayNumber')

    def damage_type(self, path):
        if not self.describe(path):
            return
        props = self.package.props(path)
        if props.get('DamageType'):
            self.field(path, 'DamageType', text(props['DamageType']))
        self.common(path, props)
        effect = props.get('StatusEffect')
        if effect:
            self.field(path, 'StatusEffect', str(self.obj(effect)))
            if self.describe(effect):
                eprops = self.package.props(effect)
                self.field(effect, 'StatusEffectType', text(eprops.get('StatusEffectType')))
                for row in eprops.get('DamageSurfaceChanceModifiers') or []:
                    self.field(effect, 'Surface', text((row.get('SurfaceType') or '').removeprefix('DMGSURFACE_').lower()),
                               str(self.init(row.get('BaseChance'))))
                if eprops.get('bDoesDamageOverTime'):
                    self.field(effect, 'DamageOverTime')
                self.field(effect, 'BaseDuration', str(self.init(eprops.get('BaseDuration'))))

    def firing_mode(self, path):
        if self.describe(path):
            speed = self.package.props(path).get('Speed')
            if speed:
                self.field(path, 'Speed', num(speed))

    def part(self, path):
        if not self.describe(path):
            return
        props = self.package.props(path)
        self.common(path, props)
        fragments = weapon_recipe.part_fragments(self.package, path)
        if fragments:
            self.field(path, 'Fragments', ','.join(text(f) for f in fragments))
        if props.get('Material'):
            self.field(path, 'Material', text(props['Material']))
        if props.get('CustomDamageTypeDefinition'):
            self.damage_type(props['CustomDamageTypeDefinition'])
            self.field(path, 'CustomDamageType', str(self.obj(props['CustomDamageTypeDefinition'])))
        if props.get('CustomFiringModeDefinition'):
            self.firing_mode(props['CustomFiringModeDefinition'])
            self.field(path, 'CustomFiringMode', str(self.obj(props['CustomFiringModeDefinition'])))
        if props.get('bIsSpinningEnabled'):
            self.field(path, 'Spinning')
            self.field(path, 'SpinStartScale', num(props.get('StartingSpinUpFireIntervalMultiplier', 1.0)))
            if props.get('SpinUpDuration'):
                self.field(path, 'SpinUpDuration', str(self.init(props['SpinUpDuration'])))

    def weapon_type(self, path):
        if not self.describe(path):
            return
        props = self.package.props(path)
        self.common(path, props)
        for field_name in weapon_stats.TYPE_BASES:
            raw = props.get(field_name, weapon_stats.TYPE_DEFAULTS.get(field_name))
            if isinstance(raw, dict):
                self.field(path, 'Base', field_name, 'init', str(self.init(raw)))
            elif raw is not None:
                self.field(path, 'Base', field_name, 'num', num(raw))
        for effect in props.get('AttributeSlotEffects') or []:
            self.field(path, 'SlotEffect', text(effect.get('SlotName')), text(effect['AttributeToModify']),
                       text(effect['ModifierType']), str(self.init(effect.get('BaseModifierValue'))),
                       str(self.init(effect.get('PerGradeUpgrade'))), flag(effect.get('bExternalSlot')),
                       flag(effect.get('bEnforceMinimumGrade')), num(effect.get('MinimumGrade') or 0),
                       flag(effect.get('bEnforceMaximumGrade')), num(effect.get('MaximumGrade') or 0))
        self.field(path, 'SlotBaseGrade', str(self.init(props.get('AttributeSlotBaseGrade') or {'BaseValueConstant': 1.0})))
        if props.get('BaseRarity'):
            self.field(path, 'BaseRarity', str(self.init(props['BaseRarity'])))
        self.field(path, 'MonetaryBase', num(float(props.get('BaseMonetaryValueModifier',
                                                             weapon_stats.BASE_MONETARY_VALUE_MODIFIER_DEFAULT))))
        self.field(path, 'SaleCap', str(weapon_stats.sale_value_cap(props)))
        monetary = props.get('MonetaryValue')
        if isinstance(monetary, dict):
            self.field(path, 'MonetaryValue', str(self.init(monetary)))
            if monetary.get('InitializationDefinition') in weapon_stats.PRICE_CALCULATORS_CHECKED:
                self.field(path, 'PriceChecked')
        for key, name in (('GestaltMesh', 'Gestalt'), ('BarrelSpinMode', 'SpinMode')):
            if props.get(key):
                self.field(path, name, text(props[key]))
        if props.get('ZoomedEndFOV') is not None:
            self.field(path, 'ZoomedEndFOV', num(props['ZoomedEndFOV']))
        if props.get('bUsesPlayerLevelRequirement'):
            self.field(path, 'UsesLevelRequirement')
        if props.get('DefaultDamageTypeDefinition'):
            self.damage_type(props['DefaultDamageTypeDefinition'])
            self.field(path, 'DefaultDamageType', str(self.obj(props['DefaultDamageTypeDefinition'])))
        if props.get('DefaultFiringModeDefinition'):
            self.firing_mode(props['DefaultFiringModeDefinition'])
            self.field(path, 'DefaultFiringMode', str(self.obj(props['DefaultFiringModeDefinition'])))

    # --- balances -----------------------------------------------------------------------
    def balance(self, path):
        """B and S records: weapon_recipe.merge() with each entry's init references instead of weights."""
        package = self.package
        chain, cursor = [], path
        while cursor:
            chain.append(cursor)
            cursor = package.props(cursor).get('BaseDefinition')
        chain.reverse()
        weapon_type = next((package.props(b).get('InventoryDefinition') for b in chain
                            if package.props(b).get('InventoryDefinition')), None)
        if not weapon_type:
            return False
        makers = next((package.props(b).get('Manufacturers') for b in chain if package.props(b).get('Manufacturers')), [])
        manufacturer = makers[0].get('Manufacturer') if makers else None
        merged = {}
        for step in chain:
            collection = package.props(step).get('WeaponPartListCollection')
            if collection:
                slots, mode = self.slot_entries(collection, manufacturer)
                merged = weapon_recipe.merge_slots(merged, slots, mode)
        self.weapon_type(weapon_type)
        self.describe(path)
        self.common(path, package.props(path))
        mission = package.classes.get(path, '').rsplit('.', 1)[-1] == weapon_stats.MISSION_BALANCE_CLASS
        self.lines.append(f'B\t{self.obj(path)}\t{self.obj(weapon_type)}\t{text(manufacturer)}\t{flag(mission)}')
        for slot in weapon_recipe.SLOTS:
            for part, weight, low, high in merged.get(slot) or []:
                self.part(part)
                self.lines.append(f'S\t{self.obj(path)}\t{slot}\t{self.obj(part)}\t{weight}\t{low}\t{high}')
        return True

    def slot_entries(self, collection, manufacturer):
        """weapon_recipe.slot_candidates() with stage None, keeping init ids (weight, min, max)."""
        props = self.package.props(collection)
        consolidated = props.get('ConsolidatedAttributeInitData') or []
        slots = {}
        for slot in weapon_recipe.SLOTS:
            data = props.get(f'{slot}PartData')
            if not data or not data.get('bEnabled'):
                continue
            entries = []
            for part in data.get('WeightedParts') or []:
                def at(name, index=None):
                    i = part.get(f'{name}Index', -1) if index is None else index
                    return self.init(consolidated[i]) if 0 <= i < len(consolidated) else None
                low, high = at('MinGameStage'), at('MaxGameStage')
                weight = weapon_recipe.entry_weight(part, manufacturer, at)
                if weight == weapon_recipe.FLAT_WEIGHT and isinstance(weight, float):
                    weight_text = 'flat'
                elif weight is None:
                    weight_text = 'none'
                else:
                    weight_text = str(weight)
                    self.weight_inits.add(weight)
                entries.append((part['Part'], weight_text, -1 if low is None else low, -1 if high is None else high))
            slots[slot] = entries
        return slots, props.get('PartReplacementMode') or 'EPRM_Additive'

    # --- pools --------------------------------------------------------------------------
    def pools(self, values):
        package = self.package
        count = 0
        for path, cls in package.classes.items():
            if is_pool_class(cls):
                self.pool(values, path)
                count += 1
        sources = 0
        for path, cls in package.classes.items():
            if cls.rsplit('.', 1)[-1] in SOURCE_CLASSES:
                try:
                    pairs = loot_pools.source_lists(package, path)
                except (KeyError, ValueError):
                    continue
                self.describe(path)
                for pool, probability in pairs:
                    if pool:
                        self.pool(values, pool)
                    self.field(path, 'Listed', str(self.obj(pool)) if pool else '-', num(values.init(probability)))
                sources += 1
        return count, sources

    def pool(self, values, path):
        if not self.describe(path) or path not in self.package.index:
            return
        props = self.package.props(path)
        self.field(path, 'Quantity', num(values.init(props.get('Quantity') or {'BaseValueConstant': 1.0})))
        bounds = []
        for key in ('MinGameStageRequirement', 'MaxGameStageRequirement'):
            if not props.get(key):
                bounds.append('-')
            else:
                bound = values.attribute(props[key])
                bounds.append('u' if bound is None else num(bound))
        self.field(path, 'Gate', *bounds)
        for entry in loot_pools.pool_entries(self.package, values, path):
            self.field(path, 'Entry', entry['kind'], str(self.obj(entry['path'])), num(entry['weight']))
            if entry['kind'] == 'pool':
                self.pool(values, entry['path'])

    # --- globals ------------------------------------------------------------------------
    def globals(self):
        package = self.package
        table = package.props(weapon_stats.GLOBALS).get('RarityLevelColors') if weapon_stats.GLOBALS in package.index else None
        for row in table or []:
            color = row.get('Color') or {}
            self.lines.append('\t'.join(['G', 'rarity', num(row.get('MinLevel') or 0), num(row.get('MaxLevel') or 0),
                                         text(row.get('RarityRating')),
                                         '#%02X%02X%02X' % (color.get('R', 0), color.get('G', 0), color.get('B', 0))]))
        rounding = weapon_stats.presentation_rounding(package)
        for field_name, (_, default) in weapon_stats.PRESENTATIONS.items():
            mode, precision = rounding.get(field_name, (default, 1))
            self.lines.append(f'G\tround\t{field_name}\t{text(mode)}\t{num(precision)}')
        if weapon_stats.ACCURACY_PRESENTATION in package.index:
            remap = package.props(weapon_stats.ACCURACY_PRESENTATION).get('RemappingData')
            if isinstance(remap, dict):
                values = [(remap.get(n) or {}).get('BaseValueConstant') or 0.0
                          for n in ('InputValueMn', 'InputValueMx', 'OutputValueMn', 'OutputValueMx')]
                self.lines.append('G\taccuracy\t' + '\t'.join(num(v) for v in values))
        for damage_type, bounds in weapon_stats.status_chance_remaps(package).items():
            self.lines.append('\t'.join(['G', 'status_remap', text(damage_type)] + [num(v) for v in bounds]))
        props = weapon_stats._optional_props(package, weapon_stats.SHOT_COST_PRESENTATION)
        prefix = weapon_stats._plain_game_text(props.get('Prefix')) or 'Consumes'
        suffix = weapon_stats._plain_game_text(props.get('Suffix')) or 'ammo per shot.'
        self.lines.append(f'G\tshot_cost_text\t{text(prefix)}\t{text(suffix)}')


def open_package(reader, startup):
    schema = ROOT / 'local/weapons/gen/weapon_gen.schema'
    schema.parent.mkdir(parents=True, exist_ok=True)
    schema.write_text('\n'.join(dict.fromkeys(weapon_stats.SCHEMA_LINES + loot_pools.SCHEMA_LINES)) + '\n',
                      encoding='utf-8')
    package = skill_stats.Package(str(Path(reader).resolve()), Path(startup), str(schema.resolve()))
    roots = [p for p, c in package.classes.items()
             if c in BALANCE_CLASSES or is_pool_class(c) or c.rsplit('.', 1)[-1] in SOURCE_CLASSES]
    package.crawl(roots + [weapon_stats.GLOBALS, weapon_stats.ACCURACY_PRESENTATION, weapon_stats.SHOT_COST_PRESENTATION,
                           weapon_stats.STATUS_CHANCE_MAPPING],
                  follow=lambda path, cls: cls.endswith('Definition') or 'ItemPool' in cls)
    # Attribute definitions, their resolver subobjects and the presentations, in batches.
    package.preload([p for p, c in package.classes.items()
                     if 'Attribute' in c or 'Resolver' in c or 'Presentation' in c or 'Weighting' in p])
    return package


def make_localize(package, game):
    """weapon_stats.main()'s localize: <Package>.int overrides the cooked English string."""
    files = {}

    def localize(path, default, property_name='NoConstraintText'):
        name = path.split('.', 1)[0]
        if name not in files:
            files[name] = skill_stats.localization_for(game, name)
        return files[name].get((skill_stats.localization_key(path, package.classes), property_name), default)
    return localize


def count_attribute_inits(builder):
    """How many slot-weight inits reference an attribute anywhere in their tree."""
    by_id = {}
    for key, ident in builder.inits.items():
        by_id[ident] = key
    def_terms = {}
    for path, ident in builder.defs.items():
        props = builder.package.props(path)
        nested = list((props.get('ValueFormula') or {}).values()) + list((props.get('RangeRestriction') or {}).values())
        def_terms[path] = [v for v in nested if isinstance(v, dict)]

    def uses(data, depth=0):
        if depth > 40:
            return False
        if data.get('BaseValueAttribute'):
            return True
        definition = data.get('InitializationDefinition')
        return bool(definition) and any(uses(t, depth + 1) for t in def_terms.get(definition, []))
    count = 0
    for ident in builder.weight_inits:
        constant, attribute, definition, scale = by_id[ident]
        if uses({'BaseValueAttribute': attribute, 'InitializationDefinition': definition}):
            count += 1
    return count


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', default=str(ROOT / 'build/Release/ow-package.exe'))
    parser.add_argument('--game', type=Path, required=True, help='Borderlands 2 install folder')
    parser.add_argument('--output', type=Path, default=ROOT / 'local/weapons/gen/weapon_gen_manifest.txt')
    args = parser.parse_args()
    if not args.output.resolve().is_relative_to((ROOT / 'local').resolve()):
        parser.error('--output must stay under local/')
    started = time.time()
    package = open_package(args.reader, args.game / 'WillowGame/CookedPCConsole/Startup.upk')
    builder = Builder(package, make_localize(package, args.game))
    builder.lines.insert(0, f'V\tweapon_gen_manifest\t{FORMAT_VERSION}')
    balances = sorted(p for p, c in package.classes.items() if c in BALANCE_CLASSES)
    written = [b for b in balances if builder.balance(b)]
    pools, sources = builder.pools(loot_pools.Values(package))
    builder.globals()
    builder.lines.append(f'E\t{len(builder.lines)}')  # footer: the record count before it (format docstring)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text('\n'.join(builder.lines) + '\n', encoding='utf-8')
    runtime = sorted(p for p in builder.attrs if weapon_stats.resolver_constant(package, p) is None)
    summary = {
        'output': str(args.output), 'seconds': round(time.time() - started, 1),
        'balances': len(written), 'balances_without_type': len(balances) - len(written),
        'objects': len(builder.objects), 'inits': builder.init_count, 'definitions': len(builder.defs),
        'attributes': len(builder.attrs), 'constant_attributes': len(builder.attrs) - len(runtime),
        'runtime_attributes': len(runtime), 'runtime_attribute_paths': runtime,
        'weight_inits': len(builder.weight_inits), 'weight_inits_with_attributes': count_attribute_inits(builder),
        'pools': pools, 'sources': sources, 'lines': len(builder.lines),
    }
    by_class = {}
    for path, ident in builder.objects.items():
        cls = package.classes.get(path, '?').rsplit('.', 1)[-1]
        by_class[cls] = by_class.get(cls, 0) + 1
    summary['objects_by_class'] = dict(sorted(by_class.items(), key=lambda kv: -kv[1]))
    (args.output.parent / 'weapon_gen_manifest.summary.json').write_text(json.dumps(summary, indent=1), encoding='utf-8')
    print(json.dumps({k: v for k, v in summary.items() if k != 'runtime_attribute_paths'}, indent=1))


if __name__ == '__main__':
    main()
