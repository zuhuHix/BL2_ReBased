"""Generate one shield card from installed package data for the slice gate's -Shield run (local/slice/).

AI-assisted (Claude Code). Reads the installed packages with ow-package and writes one gear manifest (schemaVersion 1, one
`shield` item) under the ignored local/ folder: its numbers are game data (docs/LEGAL.md). The card is ONE seeded roll of ONE
balance under rules read from script and from the native notes. Its numbers are DATA-DERIVED AND UNVERIFIED IN GAME. It is not
"the shield Maya would have": the game rolls its own parts, with its own random sequence.

What is read (tagged properties only; no offsets):
- The balance's BaseDefinition chain, its InventoryDefinition (a ShieldDefinition) and Manufacturers. The manufacturer is the
  first Manufacturers entry; the game's pick among several entries is not modelled (UNVERIFIED).
- Part lists: the definition's <Slot>Parts (ItemPartListDefinition) and then each balance's PartListCollection
  (<Slot>PartData), merged as weapons merge (weapon_recipe.merge_slots; UNVERIFIED for items). Part weights are read with
  weapon_recipe.entry_weight; the game-stage window from each list's ConsolidatedAttributeInitData is applied with the item
  level as the game stage (UNVERIFIED).
- The roll: weapon_recipe.pick per slot in slot order (Alpha .. Material), drawing from weapon_recipe.CrtRand(seed).
- Names: the definition's then each chosen part's PrefixList / TitleList through weapon_recipe.best_name_part.

How the three numbers are formed (script WillowItem.InitializeInternal and ApplyAllExternalAttributeEffects, read with
research/script_disasm.py; the slot natives are the ones the weapon rules use, docs/verification/NATIVE_ATTRIBUTES.md):
- Each pool attribute (D_Attributes.ShieldResourcePool.ShieldMaxValue, ...OnIdleRegenerationDelay, ...OnIdleRegenerationRate)
  starts at the shield pool's base, taken as 0: D_Resourcepools.PlayerPools.ShieldPool names no base maximum, delay or rate
  (read from data; UNVERIFIED in game).
- The definition's ExternalAttributeEffects, then every part's (name parts included), each evaluated at the item level
  (ShieldItemLevel reads ExpLevel) with weapon_recipe.attribute_value.
- Attribute slots: the definition's AttributeSlotUpgrades, then each part's in slot order, then the name parts' (the script
  runs a second slot pass for the name parts, InitializeAttributeSlotsForNameParts; weapon_stats does not count them). An
  entry adds its GradeIncrease; the first activating entry also adds the base grade (the definition's AttributeSlotBaseGrade,
  class default 1, rounded half up); max then min clamp (weapon_stats.slot_grade). At most AttributeSlotMaxActivated slots
  activate (class default 19; the script passes GetAttributeSlotMaxActivated to the slot natives). Every AttributeSlotEffects
  entry of an activated slot applies, as in weapon_stats. Value = weapon_stats.slot_value. Only external slots that are not
  bRunEffectsAsSkill reach the owner's pool (ApplyExternalSlotEffectModifiers).
- The stack: weapon_stats.combine, (base + PreAdd) * (1 + up) / (1 + down) + PostAdd in single precision.

Refusals (nothing is written): an object that did not decode, any object the tool looked up that no package holds (every
lookup feeds the roll or the range over all combinations, so none is unrelated), a stat of 0 or less or not finite, more than
MAX_COMBINATIONS combinations. The range check is a consistency check, not an oracle: the roll is one of the combinations it
enumerates. Values are written to three decimals, not as the game's card rounds them (the shield card presentation was not
read). Keep or refuse, never degrade: an existing output that differs is not written over unless --replace says so.
"""
import argparse
import itertools
import json
import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import prepare_vendors as pv  # noqa: E402
import weapon_recipe  # noqa: E402
import weapon_stats  # noqa: E402
from loot_pools import SCHEMA_LINES  # noqa: E402

REPO = Path(__file__).resolve().parents[1]
DEFAULT_BALANCE = 'GD_ItemGrades.Shields.ItemGrade_Gear_Shield_Standard_01_Common'
DEFAULT_OUTPUT = 'local/slice/stock_shield_gear.json'
SCHEMA_PATH = REPO / 'local' / 'slice' / 'stock_shield.schema'
SCHEMA_EXTRA = ['AttributeSlotEffects=StructProperty:AttributeSlotEffectData',
                'AttributeSlotUpgrades=StructProperty:AttributeSlotUpgradeData']
# Maya's level in the slice (Sanctuary's playthrough-1 band is 7-9; tools/weapon_slice_gear.py uses 8). A card above it
# would not equip under the normal level rule, which is what this card is for.
SLICE_LEVEL = 8
# The seed the slice guns use (docs/TOOLING.md: weapon_slice_gear.py --seed 1). Not chosen for how the roll fights.
DEFAULT_SEED = 1
ITEM_SLOTS = pv.ITEM_SLOTS
POOL = 'D_Attributes.ShieldResourcePool.'
STATS = [('Capacity', POOL + 'ShieldMaxValue'), ('Recharge Rate', POOL + 'ShieldOnIdleRegenerationRate'),
         ('Recharge Delay', POOL + 'ShieldOnIdleRegenerationDelay')]
POOL_BASE = 0.0  # the shield pool's base for all three (ShieldPool names none; UNVERIFIED in game)
MAX_SLOTS_DEFAULT = 19  # AttributeSlotMaxActivated class default (NATIVE_WEAPON_RULES.md section 1)
MAX_COMBINATIONS = 200000
STATUS = 'DATA-DERIVED, UNVERIFIED IN GAME'
RULES = [
    'part lists merged as weapons merge (UNVERIFIED for items)',
    'part weights and pick as read for weapons from native code (UNVERIFIED)',
    'the part lists\' game-stage windows use the item level as the game stage (UNVERIFIED)',
    'the manufacturer is the balance\'s first Manufacturers entry; no weighted pick (UNVERIFIED)',
    'the game random sequence is not reproduced; seeded CrtRand',
    'slot grade and value as read for weapons (WillowInventory slot natives; UNVERIFIED in game)',
    'name parts\' slot upgrades count (script InitializeAttributeSlotsForNameParts; UNVERIFIED in game)',
    'at most AttributeSlotMaxActivated slots activate, class default 19 (UNVERIFIED in game)',
    'every slot effect entry of an activated slot applies, as in weapon_stats (UNVERIFIED in game)',
    'external effects and external slots reach the shield pool (script WillowItem.ApplyAllExternalAttributeEffects)',
    'the shield pool\'s base is 0 for capacity, delay and rate (read from data; UNVERIFIED in game)',
    'modifier stack as read for weapons (UNVERIFIED in game)',
    'name choice as read for weapons (UNVERIFIED for items)',
    'item level = the requested level; how the game picks a level is not modelled',
    'values written to three decimals, not as the game card rounds them (presentation not read)',
]


class RefusedError(ValueError):
    """The data does not resolve the way this tool needs; nothing is written."""


def short(path):
    return path.rsplit('.', 1)[-1] if path else None


def weighted_list(chain, source, stage, manufacturer):
    """({slot: [(part, weight)]}, mode) of one part list; weights by weapon_recipe.entry_weight, stage window applied.

    A copy of prepare_vendors.item_slot_parts that keeps each part's weight (that function keeps only the part, for the price)
    and weighs for one manufacturer. tests/prepare_stock_shield_test.py requires both to give the same parts in the same order.
    `source` is a definition's ItemPartListDefinition (WeightedParts; the caller names the slot, returned under None) or a
    balance's ItemPartListCollectionDefinition (<Slot>PartData). A weight that needs runtime state is None.
    """
    props = chain.props(source)
    consolidated = props.get('ConsolidatedAttributeInitData') or []

    def entries(rows):
        found = []
        for part in rows or []:
            def at(name, index=None):
                i = part.get(f'{name}Index', -1) if index is None else index
                return weapon_recipe.attribute_value(chain, consolidated[i], stage) if 0 <= i < len(consolidated) else None
            low, high = at('MinGameStage'), at('MaxGameStage')
            if (low is not None and stage < int(low)) or (high is not None and int(high) < stage):
                continue
            if part.get('Part'):
                found.append((part['Part'], weapon_recipe.entry_weight(part, manufacturer, at)))
        return found

    if 'WeightedParts' in props:
        return {None: entries(props['WeightedParts'])}, 'EPRM_Selective'
    slots = {}
    for slot in ITEM_SLOTS:
        data = props.get(f'{slot}PartData')
        if data and data.get('bEnabled'):
            slots[slot] = entries(data.get('WeightedParts'))
    return slots, props.get('PartReplacementMode') or 'EPRM_Additive'


def candidates(chain, balance, stage):
    """{steps, definition, manufacturer, slots: {slot: {part: weight}}} for one shield balance.

    A copy of prepare_vendors.item_parts that keeps the weights (see weighted_list); the drift test pins the two together.
    """
    steps = pv.base_chain(chain, balance)
    definition = next((chain.props(b).get('InventoryDefinition') for b in steps[::-1]
                       if chain.props(b).get('InventoryDefinition')), None)
    if not definition or not chain.classes.get(definition, '').endswith('ShieldDefinition'):
        raise RefusedError(f'{balance}: its InventoryDefinition is not a ShieldDefinition ({definition})')
    makers = next((chain.props(b).get('Manufacturers') for b in steps[::-1] if chain.props(b).get('Manufacturers')), [])
    manufacturer = makers[0].get('Manufacturer') if makers else None
    merged = {}
    for slot in ITEM_SLOTS:
        source = chain.props(definition).get(f'{slot}Parts')
        if source:
            merged[slot] = weighted_list(chain, source, stage, manufacturer)[0][None]
    for step in steps:
        props = chain.props(step)
        collection = props.get('PartListCollection') or props.get('ItemPartListCollection')
        if collection:
            slots, mode = weighted_list(chain, collection, stage, manufacturer)
            merged = weapon_recipe.merge_slots(merged, slots, mode)
    out = {}
    for slot in ITEM_SLOTS:
        rows = merged.get(slot) or []
        if any(w is None for _, w in rows):
            raise RefusedError(f'{balance}: a {slot} weight needs runtime state')
        live = {}
        for part, weight in rows:  # a part listed twice keeps its later weight (weapon_recipe.pick)
            if weight > weapon_recipe.MIN_WEIGHT:
                live[part] = weight
        if live:
            out[slot] = live
    return {'steps': steps, 'definition': definition, 'manufacturer': manufacturer, 'slots': out}


def roll(cands, seed):
    """{slot: part}: weapon_recipe.pick per slot in slot order from CrtRand(seed)."""
    rng = weapon_recipe.CrtRand(seed)
    parts = {}
    for slot in ITEM_SLOTS:
        if slot in cands['slots']:
            parts[slot] = weapon_recipe.pick(list(cands['slots'][slot].items()), rng)[0]
    return parts


def name_parts(chain, definition, parts, manufacturer, level):
    """(prefix, title): the definition's lists first, then each part's in slot order; a later list wins on >= priority.

    A copy of weapon_recipe.choose_name_parts over the item slots (that function walks the weapon slots and resolves a weapon
    damage type, which a shield has none of). The drift test pins the two together.
    """
    chosen = []
    for key in ('PrefixList', 'TitleList'):
        current = weapon_recipe.best_name_part(chain, chain.props(definition).get(key), level, manufacturer)
        for slot in ITEM_SLOTS:
            part = parts.get(slot)
            if not part:
                continue
            candidate = weapon_recipe.best_name_part(chain, chain.props(part).get(key), level, manufacturer)
            if current is None or (candidate is not None and
                                   weapon_recipe.name_priority(chain, current) <= weapon_recipe.name_priority(chain, candidate)):
                current = candidate
        chosen.append(current)
    return chosen[0], chosen[1]


class Evaluator:
    """AttributeInitializationData -> float at one item level; refuses what the packages do not value."""

    def __init__(self, chain, level):
        self.chain, self.level, self.cache = chain, float(level), {}

    def attributes(self, init):
        known = {}
        for attribute in pv.init_attributes(self.chain, init):
            if attribute in self.cache:
                known[attribute] = self.cache[attribute]
                continue
            if pv.object_property(self.chain, attribute) == pv.LEVEL_PROPERTY:
                value = self.level
            else:
                value = weapon_stats.resolver_constant(self.chain, attribute)
            if value is None:
                raise RefusedError(f'attribute {attribute} has no package value (not a constant, not the item level)')
            self.cache[attribute] = known[attribute] = float(value)
        return known

    def value(self, init):
        if not isinstance(init, dict):
            return float(init or 0.0)
        result = weapon_recipe.attribute_value(self.chain, init, self.level, self.attributes(init))
        if result is None:
            raise RefusedError(f'an initialization did not resolve: {init}')
        return result


def stats(chain, definition, parts, names, level):
    """{label: value} of the three pool attributes for one part set, plus the slots that reached the pool."""
    ev = Evaluator(chain, level)
    holders = [definition] + [parts[s] for s in ITEM_SLOTS if parts.get(s)] + [n for n in names if n]
    mods = {}

    def add(attribute, kind, amount):
        amount = weapon_recipe.f32(amount)
        bucket = mods.setdefault(attribute, {'MT_PreAdd': 0.0, 'MT_Scale': 0.0, 'MT_PostAdd': 0.0,
                                             'scale_up': 0.0, 'scale_down': 0.0})
        bucket[kind] = weapon_recipe.f32(bucket.get(kind, 0.0) + amount)
        if kind == 'MT_Scale':
            side = 'scale_up' if amount > 0 else 'scale_down'
            bucket[side] = weapon_recipe.f32(bucket[side] + abs(amount))

    for holder in holders:
        for effect in chain.props(holder).get('ExternalAttributeEffects') or []:
            add(effect.get('AttributeToModify'), effect.get('ModifierType') or 'MT_Scale', ev.value(effect.get('BaseModifierValue')))
    dprops = chain.props(definition)
    base_grade = ev.value(dprops['AttributeSlotBaseGrade']) if dprops.get('AttributeSlotBaseGrade') else 1.0
    limit = dprops.get('AttributeSlotMaxActivated')
    limit = MAX_SLOTS_DEFAULT if not isinstance(limit, int) or not 1 <= limit <= 19 else limit
    increases, active = {}, []
    for holder in holders:
        for upgrade in chain.props(holder).get('AttributeSlotUpgrades') or []:
            name = upgrade.get('SlotName')
            increases[name] = increases.get(name, 0) + (upgrade.get('GradeIncrease') or 0)
            if upgrade.get('bActivateSlot') and name not in active and len(active) < limit:
                active.append(name)
    reached = []
    for effect in dprops.get('AttributeSlotEffects') or []:  # every entry of an activated slot, as weapon_stats applies them
        name = effect.get('SlotName')
        if name not in active or not effect.get('AttributeToModify'):
            continue
        if not effect.get('bExternalSlot') or effect.get('bRunEffectsAsSkill'):
            continue  # internal slots land on the item itself; skill-run slots go through the skill machinery
        # slot_grade rounds the base grade half up and adds the increases; the activation's base grade is in the rounding.
        grade = weapon_stats.slot_grade(base_grade, increases.get(name, 0), effect)
        value = weapon_stats.slot_value(grade, ev.value(effect.get('BaseModifierValue')), ev.value(effect.get('PerGradeUpgrade')))
        add(effect['AttributeToModify'], effect.get('ModifierType') or 'MT_Scale', value)
        reached.append({'slot': name, 'grade': grade})
    out = {label: weapon_stats.combine(POOL_BASE, mods.get(attribute, {})) for label, attribute in STATS}
    for label, value in out.items():
        if value is None or not math.isfinite(value) or value <= 0.0:
            # A capacity of 0 equips as no shield; a rate or delay of 0 or less is ignored by the host's ReadShieldStats,
            # which would then use its invented stand-ins while the card is reported as the source.
            raise RefusedError(f'{label} evaluates to {value} for parts {sorted(parts.values())}; it must be above 0')
    return out, reached


def combinations(cands):
    slots = [s for s in ITEM_SLOTS if s in cands['slots']]
    count = math.prod(len(cands['slots'][s]) for s in slots)
    if count > MAX_COMBINATIONS:
        raise RefusedError(f'{count} part combinations; more than {MAX_COMBINATIONS} are not enumerated')
    for combo in itertools.product(*(cands['slots'][s] for s in slots)):
        yield dict(zip(slots, combo))


def stat_range(chain, cands, level):
    """{label: (min, max)} over every drawable combination, and the combination count."""
    low, high, count = {}, {}, 0
    for parts in combinations(cands):
        names = name_parts(chain, cands['definition'], parts, cands['manufacturer'], level)
        values, _ = stats(chain, cands['definition'], parts, names, level)
        count += 1
        for label, value in values.items():
            low[label] = min(low.get(label, value), value)
            high[label] = max(high.get(label, value), value)
    return {label: [low[label], high[label]] for label, _ in STATS}, count


def position(value, span):
    low, high = span
    return 0.0 if high == low else (value - low) / (high - low)


def check_lookups(chain):
    """Refuse when an object did not decode or a looked-up object is in no package (pv.Chain reads both as {})."""
    failed = sorted(chain.failed)
    missing = sorted(p for p in chain.missing if p)
    if failed or missing:
        raise RefusedError(f'objects that did not decode: {failed}; objects in no package: {missing}')


def card(chain, balance, level, seed):
    """The gear-manifest document with one shield item."""
    try:
        cands = candidates(chain, balance, level)
        parts = roll(cands, seed)
        prefix, title = name_parts(chain, cands['definition'], parts, cands['manufacturer'], level)
        values, reached = stats(chain, cands['definition'], parts, (prefix, title), level)
        span, count = stat_range(chain, cands, level)
    except RefusedError:
        check_lookups(chain)  # an object read as {} is the likelier cause; name it first
        raise
    check_lookups(chain)
    for label, value in values.items():  # a consistency check: the roll is one of the enumerated combinations
        if not span[label][0] <= value <= span[label][1]:
            raise RefusedError(f'{label} {value} lies outside its own range {span[label]} (the tool disagrees with itself)')
    words = [chain.props(n).get('PartName') for n in (prefix, title) if n]
    name = ' '.join(w for w in words if w) or chain.props(cands['definition']).get('ItemName')
    if not name:
        raise RefusedError(f'{balance}: no name resolves')
    item = {
        'id': f'stock_shield_{short(balance)}_l{level}_s{seed}'.lower(),
        'assetId': '',  # present but empty: no verified mesh preview (tools/hud_overlay/inventory.js)
        'itemType': 'shield',
        'name': name,
        'manufacturer': short(cands['manufacturer']) or '',
        'level': level,
        'stats': [{'label': label, 'value': f'{values[label]:.3f}'.rstrip('0').rstrip('.')} for label, _ in STATS],
        'provenance': {
            'status': STATUS,
            'what': 'one seeded roll of one balance under rules read from script and the native notes; not a shield the '
                    'game gave anyone, and not the shield Maya would have',
            'tool': 'tools/prepare_stock_shield.py',
            'balance': balance, 'definition': cands['definition'], 'manufacturer': cands['manufacturer'],
            'seed': seed, 'level': level,
            'parts': {slot: parts[slot] for slot in ITEM_SLOTS if slot in parts},
            'name_parts': {'prefix': prefix, 'title': title},
            'weights': 'read from the part lists (weapon_recipe.entry_weight; UNVERIFIED rule)',
            'candidates': cands['slots'],
            'slots_reaching_the_pool': reached,
            'combinations': count,
            'range': span,
            'range_check': 'consistency only: the roll is one of the enumerated combinations',
            'position_in_range': {label: round(position(values[label], span[label]), 4) for label, _ in STATS},
            'values': 'written to three decimals, not as the game card rounds them',
            'rules': RULES,
        },
    }
    return {'schemaVersion': 1, 'items': [item]}


def replace_problem(out, document, replace):
    """Why this run must not write `document` over `out` ("" when it may): keep or refuse, never degrade."""
    if replace or not out.exists():
        return ''
    try:
        old = json.loads(out.read_text(encoding='utf-8'))
    except (OSError, ValueError) as error:
        return f'{out} exists but cannot be read ({error!r}); nothing was written. Look at it, then pass --replace, or use --output'
    if old == document:
        return ''
    return (f'{out} holds a different card; nothing was written. Pass --replace to write over it, or write elsewhere with '
            f'--output')


def write(out, document):
    """Through a temporary file next to `out`; a failure part-way leaves `out` as it was and removes the temporary file."""
    temporary = out.with_name(out.name + '.tmp')
    try:
        temporary.write_text(json.dumps(document, indent=1) + '\n', encoding='utf-8')
        temporary.replace(out)
    except BaseException:
        temporary.unlink(missing_ok=True)
        raise


def open_chain(reader, game):
    """The package lookups over Startup.upk (tests replace this)."""
    from prepare_slice_world import Reader
    SCHEMA_PATH.parent.mkdir(parents=True, exist_ok=True)
    SCHEMA_PATH.write_text('\n'.join(pv.SCHEMA_EXTRA + SCHEMA_LINES + SCHEMA_EXTRA) + '\n', encoding='utf-8')
    return pv.Chain(Reader(reader, game, str(SCHEMA_PATH)), ['Startup'])


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', default=str(REPO / 'build' / 'Release' / 'ow-package.exe'), help='ow-package executable')
    parser.add_argument('--game', required=True, help='Borderlands 2 install directory')
    parser.add_argument('--balance', default=DEFAULT_BALANCE)
    parser.add_argument('--level', type=int, default=SLICE_LEVEL, help=f'item level, 1..{SLICE_LEVEL}')
    parser.add_argument('--seed', type=int, default=DEFAULT_SEED,
                        help=f'CrtRand seed (default {DEFAULT_SEED}, the seed the slice guns use)')
    parser.add_argument('--output', default=DEFAULT_OUTPUT)
    parser.add_argument('--replace', action='store_true', help='write over an existing card that differs')
    args = parser.parse_args(argv)
    if not 1 <= args.level <= SLICE_LEVEL:
        parser.error(f'--level must be 1..{SLICE_LEVEL} (Maya\'s slice level; a card above it does not equip)')
    try:
        out = pv.local_path(args.output)
    except ValueError as error:
        print(error, file=sys.stderr)
        return 2
    chain = open_chain(args.reader, args.game)
    try:
        document = card(chain, args.balance, args.level, args.seed)
    except RefusedError as error:
        print(f'refused: {error}', file=sys.stderr)
        return 1
    problem = replace_problem(out, document, args.replace)
    if problem:
        print(problem, file=sys.stderr)
        return 1
    out.parent.mkdir(parents=True, exist_ok=True)
    write(out, document)
    item = document['items'][0]
    where = ', '.join(f'{k} {v:.2f}' for k, v in item['provenance']['position_in_range'].items())
    print(f'wrote {out}: shield {item["id"]} (level {args.level}, seed {args.seed}), {STATUS}')
    print(f'  {item["provenance"]["combinations"]} drawable part combinations; the roll sits at these fractions of each '
          f'range (a consistency check, not an oracle): {where}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
