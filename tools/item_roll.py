"""Roll shields, grenade mods, class mods and relics from the item registry, and prove each roll legal (local/items/roll/).

AI-assisted (Claude Code). `roll()` rolls one item of one balance at one game stage from a seed; `check()` proves a roll legal
against the registry; `--census` rolls every balance of the four kinds many times over the player's own install and checks
every roll. The registry is tools/item_registry.py's output (docs/verification/ITEM_REGISTRY.md). The rules are those of
docs/verification/NATIVE_ITEM_RULES.md, NATIVE_LOOT.md and NATIVE_WEAPON_RULES.md, read from data and native code; NONE IS
CONFIRMED IN THE RUNNING GAME. Output is game data (docs/LEGAL.md) and goes under the ignored local/ folder only.

Usage:
    python tools/item_roll.py --census [--rolls 20] [--registry local/items/registry/registry.json]
        [--reader build/Release/ow-package.exe] [--game "<Borderlands 2 install>"]
writes local/items/roll/census.json and prints counts only (no object names).

Reused by import, not copied: weapon_recipe (CrtRand, pick, best_name_part, name_priority, attribute_value, MIN_WEIGHT),
item_registry (candidates_at, attribute_constant, open_census, SCHEMA_LINES, ITEM_SLOTS), weapon_stats (GLOBALS,
SCHEMA_LINES for RarityLevelColors), prepare_stock_shield (SCHEMA_EXTRA, the stock shield comparison).

`lookup` is anything with props(path), get(path) (None when no package holds it) and classes {path: class}:
item_registry's census over the install, or a synthetic one in tests. The registry carries the part candidates; the lookup
answers what the registry does not hold (attribute values, name parts, part rarities, slot upgrades, globals).

The roll, in order (one CrtRand(seed) stream, as prepare_stock_shield and weapon_recipe draw):
1. Manufacturer and grade (NATIVE_LOOT.md, "Balance candidate expansion"): for each manufacturer entry of the registry's
   balance, the first grade whose [MinGameStage, MaxGameStage] holds the stage; its weight is the spawn modifier
   interpolated across the window. One option: no draw. Several: one weapon_recipe.pick among them. STAND-IN: the game
   makes this choice in the pool roll, one candidate per manufacturer, not in the balance roll (UNVERIFIED). No option:
   the balance yields nothing at that stage (NotRollable). The registry keeps only the leaf-most Manufacturers list; the
   game's walk to a base balance when that list has no covering grade is not modelled.
2. Definition: a balance that interpolates its level (bInterpolateExpLevel, class default true) yields its own
   InventoryDefinition; one that does not yields the grade's own definition when set. Class mods: STAND-IN, one uniform
   weapon_recipe.pick among the balance's ClassModDefinitions (the game's ClassModBalanceDefinition override is not read;
   NATIVE_ITEM_RULES.md "Open"). No draw when there is one.
3. Parts: item_registry.candidates_at for the definition's variant at the stage for the manufacturer (the weapon merge,
   weights and stage windows reused for items; UNVERIFIED). A window bound that needs runtime state is no bound (as
   prepare_vendors and prepare_stock_shield read it); a weight that needs runtime state is taken as 1 with a note.
   The relic hook (NATIVE_ITEM_RULES.md section 1): when the definition sets bAllowInventoryDefToModifyPartWeight (class
   default true for ArtifactDefinition only), a part with slot upgrades, none naming a slot in the definition's
   AttributeSlotEffects, weighs 0. Then weapon_recipe.pick per slot, Alpha .. Theta, Material; a slot with no candidate
   takes no part and no draw. EPRM_Complete: the registry's merge (Complete replaces every slot), which is reading (b) of
   NATIVE_ITEM_RULES.md section 1; reading (a) is not modelled (UNVERIFIED either way).
4. Level: the stage for an interpolating balance (NATIVE_LOOT.md, GetExpLevelFromManufacturerData), else the grade's
   ExpLevel. No stage cap is applied here (the pool spawn caps it, weapon_recipe.default_stage_cap).
5. Name parts: the definition's PrefixList / TitleList, then each chosen part's in slot order, by
   weapon_recipe.best_name_part; a later list wins on equal or higher priority (prepare_stock_shield.name_parts over the
   registry's lists, which leave empty entries out). No damage type: a name expression on one excludes the name part.
6. Rarity (NATIVE_ITEM_RULES.md section 2): level = trunc(definition BaseRarity) + trunc(Rarity) of each chosen part and,
   STAND-IN (the reading the note says the data supports), of the two name parts. The band is the RarityRating of the first
   GlobalsDefinition.RarityLevelColors entry whose [MinLevel, MaxLevel] holds the level, by name only (no colours: decision
   2026-11-08-02). A level in no entry has no band. Not implemented: the note's mission-item exception (a
   MissionItemDefinition takes GlobalsDefinition.MissionItemRarityLevel, 500, instead of the sum); no definition of the four
   kinds rolled here is a mission item.

check() restates every rule above from the registry's raw entries and the raw properties, without the roll's rule helpers;
what it shares with roll() is listed in the comment above it.
"""
import argparse
import json
import math
import sys
import time
from collections import Counter, defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import item_registry  # noqa: E402
import prepare_stock_shield as ps  # noqa: E402
import prepare_vendors as pv  # noqa: E402
import weapon_catalog  # noqa: E402
import weapon_recipe  # noqa: E402
import weapon_stats  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
KINDS = ('shield', 'grenade_mod', 'class_mod', 'relic')
SLOTS = list(item_registry.ITEM_SLOTS)
GLOBALS = weapon_stats.GLOBALS
DEFAULT_REGISTRY = 'local/items/registry/registry.json'
DEFAULT_OUT = 'local/items/roll'
CENSUS_STAGES = (1, 8, 15, 30, 50, 80)
DEFAULT_ROLLS = 20
# Definition classes whose class default sets bAllowInventoryDefToModifyPartWeight (NATIVE_ITEM_RULES.md section 1).
HOOK_CLASS_DEFAULT = {'ArtifactDefinition': True}
# STAND-IN (NATIVE_ITEM_RULES.md section 2, "What an implementer needs to decide" 2): name parts count in the rarity sum.
NAME_PARTS_IN_RARITY = True
UNVERIFIED = [
    'manufacturer and grade drawn here with the spawn modifier as weight; the game draws them in the pool roll',
    'class mod definition: uniform pick among ClassModDefinitions (the game\'s override is not read)',
    'part merge, entry weights and stage windows reused from weapons for items',
    'EPRM_Complete merge: the registry\'s (every slot replaced), reading (b); reading (a) not modelled',
    'relic weight hook as read from native code',
    'name parts counted in the rarity sum',
    'level = stage for an interpolating balance, no stage cap',
    'the game\'s random sequence is not reproduced; seeded CrtRand',
]


class NotRollable(ValueError):
    """The balance yields no item at this stage (no covering grade, no definition, ...); `reason` is a short key."""

    def __init__(self, reason, detail=''):
        super().__init__(f'{reason}: {detail}' if detail else reason)
        self.reason = reason


# ---- values -------------------------------------------------------------------------------------------------------------
def props_of(lookup, path):
    return (lookup.get(path) if path else None) or {}


def number(lookup, init, level):
    """A number, or an AttributeInitializationData evaluated with weapon_recipe.attribute_value; a base attribute whose only
    resolver is a constant resolver gives that constant (item_registry.attribute_constant). None when it needs runtime
    state."""
    if init is None or isinstance(init, (int, float)):
        return init
    try:
        value = weapon_recipe.attribute_value(lookup, init, level)
        attribute = init.get('BaseValueAttribute')
        if value is None and attribute:
            constant = item_registry.attribute_constant(lookup, attribute)
            if constant is not None:
                value = weapon_recipe.attribute_value(lookup, init, level, {attribute: constant})
    except (KeyError, RuntimeError, ValueError):
        return None
    return value


# ---- 1. manufacturer and grade ------------------------------------------------------------------------------------------
def grade_weight(grade, stage):
    """The spawn modifier interpolated across the grade's window (NATIVE_LOOT.md), single precision."""
    low = grade['min_stage'] or 0
    high = grade['max_stage'] or 0
    spawn = grade.get('spawn_probability') or {}
    first, last = spawn.get('min'), spawn.get('max')
    first = 1.0 if not isinstance(first, (int, float)) else first
    last = first if not isinstance(last, (int, float)) else last
    span = high - low
    if span > 0:
        return weapon_recipe.f32(first + (stage - low) / span * (last - first))
    return weapon_recipe.f32(first)


def manufacturer_options(record, stage):
    """[(manufacturer, grade index, weight)]: per manufacturer entry, the first grade whose window holds the stage, when its
    weight is above weapon_recipe.MIN_WEIGHT (a candidate the pool can never draw is left out)."""
    options = []
    for entry in record['manufacturers']:
        if not entry['manufacturer']:
            continue
        for index, grade in enumerate(entry['grades']):
            if (grade['min_stage'] or 0) <= stage <= (grade['max_stage'] or 0):
                weight = grade_weight(grade, stage)
                if weight > weapon_recipe.MIN_WEIGHT:
                    options.append((entry['manufacturer'], index, weight))
                break
    return options


def interpolates(lookup, balance):
    value = props_of(lookup, balance).get('bInterpolateExpLevel')
    return True if value is None else bool(value)


# ---- 2. definition ------------------------------------------------------------------------------------------------------
def definition_options(record, grade, interpolating):
    """The definitions this manufacturer and grade can yield (class mods: every ClassModDefinition)."""
    if record['kind'] == 'class_mod':
        return list(dict.fromkeys(record['class_mod_definitions']))
    if not interpolating and grade is not None and grade.get('definition'):
        return [grade['definition']]
    return [record['inventory_definition']] if record['inventory_definition'] else []


def variant_of(record, definition):
    return next((v for v in record['variants'] if v['definition'] == definition), None)


# ---- 3. parts -----------------------------------------------------------------------------------------------------------
def hook_applies(registry, lookup, definition):
    cls = (registry['definitions'].get(definition) or {}).get('class', '')
    flag = props_of(lookup, definition).get('bAllowInventoryDefToModifyPartWeight')
    return HOOK_CLASS_DEFAULT.get(cls, False) if flag is None else bool(flag)


def slot_names(rows):
    """Slot names of the rows; an empty name or the name None names no slot."""
    return {row.get('SlotName') for row in rows or []} - {None, '', 'None'}


def hook_keeps(lookup, defined, part):
    """The relic answer: a part with no slot upgrades, or one naming a slot the relic defines, keeps its weight."""
    upgrades = slot_names(props_of(lookup, part).get('AttributeSlotUpgrades'))
    return not upgrades or bool(upgrades & defined)


def legal_candidates(registry, lookup, definition, variant, stage, manufacturer):
    """{slot: {part: weight}} a roll may draw from, after the stage window, the entry weight and the relic hook.

    -> (candidates, parts the hook removed)."""
    found = item_registry.candidates_at(lookup, variant['slots'], stage, manufacturer)
    removed = []
    if hook_applies(registry, lookup, definition):
        defined = slot_names(props_of(lookup, definition).get('AttributeSlotEffects'))
        for slot in list(found):
            keep = {}
            for part, weight in found[slot].items():
                if hook_keeps(lookup, defined, part):
                    keep[part] = weight
                else:
                    removed.append((slot, part))
            if keep:
                found[slot] = keep
            else:
                del found[slot]
    return found, removed


# ---- 5. name parts ------------------------------------------------------------------------------------------------------
def name_parts(registry, lookup, definition, parts, manufacturer, level):
    """(prefix, title): the definition's lists first, then each chosen part's in slot order (prepare_stock_shield.name_parts)."""
    chosen = []
    for key in ('prefix', 'title'):
        own = (registry['definitions'].get(definition) or {}).get(key) or []
        current = weapon_recipe.best_name_part(lookup, own, level, manufacturer)
        for slot in SLOTS:
            part = parts.get(slot)
            if not part:
                continue
            names = (registry['parts'].get(part) or {}).get(key) or []
            candidate = weapon_recipe.best_name_part(lookup, names, level, manufacturer)
            if current is None or (candidate is not None and weapon_recipe.name_priority(lookup, current)
                                   <= weapon_recipe.name_priority(lookup, candidate)):
                current = candidate
        chosen.append(current)
    return chosen[0], chosen[1]


# ---- 6. rarity ----------------------------------------------------------------------------------------------------------
def rarity_terms(lookup, definition, parts, names, level):
    """[(source, truncated value or None)]: the definition's BaseRarity, each part's Rarity, the name parts' (stand-in)."""
    base = props_of(lookup, definition).get('BaseRarity')
    terms = [(definition, 0 if base is None else number(lookup, base, level))]
    sources = [parts[s] for s in SLOTS if parts.get(s)]
    if NAME_PARTS_IN_RARITY:
        sources += [n for n in names if n]
    for source in sources:
        value = props_of(lookup, source).get('Rarity')
        terms.append((source, 0 if value is None else number(lookup, value, level)))
    return [(source, None if value is None else math.trunc(value)) for source, value in terms]


def rarity_band(lookup, level):
    """(RarityRating name without its prefix, or None when no entry holds the level)."""
    for row in props_of(lookup, GLOBALS).get('RarityLevelColors') or []:
        if (row.get('MinLevel') or 0) <= level <= (row.get('MaxLevel') or 0):
            rating = row.get('RarityRating')
            return rating.split('_', 1)[1] if rating and '_' in rating else rating
    return None


def rarity(lookup, definition, parts, names, level):
    terms = rarity_terms(lookup, definition, parts, names, level)
    total = sum(v for _, v in terms if v is not None)
    return {'level': total, 'band': rarity_band(lookup, total),
            'unresolved': [s for s, v in terms if v is None], 'name_parts_counted': NAME_PARTS_IN_RARITY}


# ---- roll and check -----------------------------------------------------------------------------------------------------
def balance_record(registry, balance):
    record = registry['balances'].get(balance)
    if record is None:
        raise NotRollable('unknown_balance', balance)
    if record['kind'] not in KINDS:
        raise NotRollable('other_kind', record['kind'])
    return record


def roll(registry, balance, stage, seed, lookup, manufacturer=None):
    """One item of `balance` at `stage` from CrtRand(seed). `manufacturer` skips the manufacturer draw (it must be an option
    at this stage). Raises NotRollable when the balance yields nothing at this stage."""
    record = balance_record(registry, balance)
    rng = weapon_recipe.CrtRand(seed)
    notes = []
    options = manufacturer_options(record, stage)
    if not record['manufacturers']:
        maker, grade_index, grade = None, None, None
    elif not options:
        raise NotRollable('no_drawable_grade_at_stage', f'stage {stage}')
    else:
        if manufacturer is not None:
            match = [o for o in options if o[0] == manufacturer]
            if not match:
                raise NotRollable('manufacturer_not_an_option', manufacturer)
            maker, grade_index, _ = match[0]
        elif len(options) == 1:
            maker, grade_index, _ = options[0]
        else:
            maker, grade_index = weapon_recipe.pick([((m, g), w) for m, g, w in options], rng)[0]
        grade = next(e for e in record['manufacturers'] if e['manufacturer'] == maker)['grades'][grade_index]
    interpolating = interpolates(lookup, balance)
    definitions = definition_options(record, grade, interpolating)
    if not definitions:
        raise NotRollable('no_definition')
    if len(definitions) == 1:
        definition = definitions[0]
    else:
        definition, _ = weapon_recipe.pick([(d, 1.0) for d in definitions], rng)
    variant = variant_of(record, definition)
    if variant is None:
        raise NotRollable('definition_has_no_variant', definition)
    candidates, removed = legal_candidates(registry, lookup, definition, variant, stage, maker)
    parts = {}
    for slot in SLOTS:
        live = candidates.get(slot)
        if not live:
            continue
        if any(w is None for w in live.values()):
            notes.append(f'{slot}: a weight needs runtime state; taken as 1')
        part, _ = weapon_recipe.pick([(p, 1.0 if w is None else w) for p, w in live.items()], rng)
        parts[slot] = part
    level = stage if interpolating or grade is None else (grade.get('exp_level') or 1)
    prefix, title = name_parts(registry, lookup, definition, parts, maker, level)
    return {
        'balance': balance, 'kind': record['kind'], 'seed': seed, 'stage': stage, 'level': level,
        'manufacturer': maker, 'grade': grade_index, 'definition': definition,
        'parts': parts, 'name_parts': {'prefix': prefix, 'title': title},
        'rarity': rarity(lookup, definition, parts, (prefix, title), level),
        'hook_removed': len(removed), 'notes': notes, 'unverified': UNVERIFIED,
    }


# ---- the checker ---------------------------------------------------------------------------------------------------------
# check() does not call the roll's rule helpers (manufacturer_options, grade_weight, interpolates, definition_options,
# legal_candidates, item_registry.candidates_at, hook_applies / hook_keeps / slot_names, rarity_terms / rarity / rarity_band):
# it restates each rule from the registry's raw entries and the raw BaseRarity, Rarity, AttributeSlotUpgrades and
# AttributeSlotEffects properties, so a census of check() over roll() tests the rules as written twice, not only the draws.
# Shared on purpose, so not tested by the census: the attribute evaluator (weapon_recipe.attribute_value, and number() for
# rarity values), the name-part rule (name_parts over weapon_recipe.best_name_part), and the registry's own merge of the part
# lists (item_registry), which both sides read as given.
def _named_slots(rows):
    """Slot names of AttributeSlotUpgrades / AttributeSlotEffects rows; an empty name or the name None is no slot."""
    return {row.get('SlotName') for row in rows or []} - {None, '', 'None'}


def _check_value(lookup, value, stage):
    """A registry value as the roll's stand-in reads it: a number, an init evaluated without runtime state, else None."""
    if value is None or isinstance(value, (int, float)):
        return value
    try:
        return weapon_recipe.attribute_value(lookup, value['init'], stage)
    except (KeyError, RuntimeError, ValueError):
        return None


def _check_grade(record, maker, stage):
    """(grade index, grade) of `maker`'s first grade whose window holds the stage, if its spawn weight is above 0; else None."""
    for entry in record['manufacturers']:
        if entry['manufacturer'] != maker:
            continue
        for index, grade in enumerate(entry['grades']):
            low, high = grade['min_stage'] or 0, grade['max_stage'] or 0
            if not low <= stage <= high:
                continue
            spawn = grade.get('spawn_probability') or {}
            first = spawn.get('min') if isinstance(spawn.get('min'), (int, float)) else 1.0
            last = spawn.get('max') if isinstance(spawn.get('max'), (int, float)) else first
            weight = first + (stage - low) / (high - low) * (last - first) if high > low else first
            return (index, grade) if weight > weapon_recipe.MIN_WEIGHT else None
        return None
    return None


def _check_hook_drops(registry, lookup, definition, part):
    """True when the relic hook gives `part` weight 0 (NATIVE_ITEM_RULES.md section 1), restated."""
    raw = props_of(lookup, definition)
    if 'bAllowInventoryDefToModifyPartWeight' in raw:
        enabled = bool(raw['bAllowInventoryDefToModifyPartWeight'])
    else:
        enabled = (registry['definitions'].get(definition) or {}).get('class') == 'ArtifactDefinition'
    if not enabled:
        return False
    upgrades = _named_slots(props_of(lookup, part).get('AttributeSlotUpgrades'))
    return bool(upgrades) and not upgrades & _named_slots(raw.get('AttributeSlotEffects'))


def _check_drawable(registry, lookup, definition, entries, stage, maker):
    """The parts of one slot's raw candidate entries that are in their stage window, weigh above 0 and survive the hook."""
    drawable = set()
    for entry in entries:
        low, high = _check_value(lookup, entry['min_stage'], stage), _check_value(lookup, entry['max_stage'], stage)
        if (low is not None and stage < int(low)) or (high is not None and stage > int(high)):
            continue
        weight = entry['weight']
        if weight['source'] == 'flat' or maker is None:
            amount = weapon_recipe.FLAT_WEIGHT
        else:
            amount = _check_value(lookup, weight['by_manufacturer'].get(maker, weight['default']), stage)
        if amount is not None and amount <= weapon_recipe.MIN_WEIGHT:
            continue
        if not _check_hook_drops(registry, lookup, definition, entry['part']):
            drawable.add(entry['part'])
    return drawable


def _check_rarity(lookup, definition, parts, names, level):
    """(level, band): trunc(BaseRarity) + trunc(Rarity) of every chosen part and name part; the first band entry holding it."""
    total = 0
    sources = [(definition, 'BaseRarity')] + [(p, 'Rarity') for p in parts.values() if p]
    if NAME_PARTS_IN_RARITY:
        sources += [(n, 'Rarity') for n in names if n]
    for path, key in sources:
        value = props_of(lookup, path).get(key)
        amount = None if value is None else number(lookup, value, level)
        total += math.trunc(amount) if amount is not None else 0
    band = None
    for row in props_of(lookup, GLOBALS).get('RarityLevelColors') or []:
        if (row.get('MinLevel') or 0) <= total <= (row.get('MaxLevel') or 0):
            band = (row.get('RarityRating') or '').replace('RARITY_', '', 1) or None
            break
    return total, band


def check(registry, item, lookup):
    """Every reason `item` is not a legal roll of its balance at its stage ([] when it is legal). Independent of the roll's
    rule helpers except where the block comment above says."""
    record = registry['balances'].get(item['balance'])
    if record is None or record['kind'] not in KINDS:
        return [f'{item["balance"]} is not a balance of the four kinds']
    problems, stage, maker = [], item['stage'], item['manufacturer']
    grade = None
    if record['manufacturers']:
        found = _check_grade(record, maker, stage)
        if found is None:
            return [f'manufacturer {maker} has no drawable grade at stage {stage}']
        if found[0] != item['grade']:
            problems.append(f'grade {item["grade"]} is not the first grade holding stage {stage} ({found[0]})')
        grade = found[1]
    elif maker is not None:
        problems.append(f'manufacturer {maker} on a balance that lists none')
    flag = props_of(lookup, item['balance']).get('bInterpolateExpLevel')
    interpolating = flag is None or bool(flag)
    if record['kind'] == 'class_mod':
        allowed = set(record['class_mod_definitions'])
    elif grade is not None and not interpolating and grade.get('definition'):
        allowed = {grade['definition']}
    else:
        allowed = {record['inventory_definition']} - {None}
    if item['definition'] not in allowed:
        return problems + [f'definition {item["definition"]} is not one this balance yields']
    expected_level = stage if interpolating or grade is None else (grade.get('exp_level') or 1)
    if item['level'] != expected_level:
        problems.append(f'level {item["level"]} is not {expected_level}')
    variant = next((v for v in record['variants'] if v['definition'] == item['definition']), {'slots': {}})
    drawable = {slot: _check_drawable(registry, lookup, item['definition'], entries, stage, maker)
                for slot, entries in variant['slots'].items()}
    for slot, part in item['parts'].items():
        if slot not in SLOTS:
            problems.append(f'{slot}: not an item slot')
        elif part not in drawable.get(slot, set()):
            problems.append(f'{slot}: {part} is not a candidate at stage {stage}')
    for slot, parts in drawable.items():
        if parts and not item['parts'].get(slot):
            problems.append(f'{slot}: empty although {len(parts)} candidates exist')
    names = item['name_parts']
    for key in ('prefix', 'title'):
        name = names.get(key)
        if name is None:
            continue
        lists = [(registry['definitions'].get(item['definition']) or {}).get(key) or []]
        lists += [(registry['parts'].get(p) or {}).get(key) or [] for p in item['parts'].values()]
        if not any(name in names_list for names_list in lists):
            problems.append(f'{key} {name} is in no list of the definition or a chosen part')
        elif not weapon_recipe.name_part_qualifies(lookup, name, item['level'], maker):
            problems.append(f'{key} {name} does not qualify at level {item["level"]}')
    chosen = name_parts(registry, lookup, item['definition'], item['parts'], maker, item['level'])
    if (names.get('prefix'), names.get('title')) != chosen:
        problems.append('name parts are not the ones the name rule chooses')
    level, band = _check_rarity(lookup, item['definition'], item['parts'], (names.get('prefix'), names.get('title')),
                                item['level'])
    if item['rarity']['level'] != level:
        problems.append(f'rarity level {item["rarity"]["level"]} is not the sum {level}')
    if item['rarity']['band'] != band:
        problems.append(f'band {item["rarity"]["band"]} is not {band}')
    return problems


def none_slot_names(registry, lookup, balances):
    """Rows of AttributeSlotUpgrades (candidate parts) and AttributeSlotEffects (definitions), and how many name no slot
    (an empty name or literally None): the roll and the checker both read such a row as naming no slot."""
    parts, definitions = set(), set()
    for balance in balances:
        record = registry['balances'][balance]
        for variant in record['variants']:
            if variant['definition']:
                definitions.add(variant['definition'])
            for entries in variant['slots'].values():
                parts.update(c['part'] for c in entries)
    out = Counter()
    for key, paths in (('AttributeSlotUpgrades', parts), ('AttributeSlotEffects', definitions)):
        for path in paths:
            for row in props_of(lookup, path).get(key) or []:
                out[f'{key} rows'] += 1
                if row.get('SlotName') in (None, '', 'None'):
                    out[f'{key} rows naming no slot'] += 1
    return dict(out)


# ---- census over the install --------------------------------------------------------------------------------------------
def schema_lines():
    """item_registry's lines plus the slot and rarity lines; one line per property name (the reader rejects repeats)."""
    lines = {}
    for line in item_registry.SCHEMA_LINES + ps.SCHEMA_EXTRA + weapon_stats.SCHEMA_LINES:
        lines.setdefault(line.split('=', 1)[0], line)
    return list(lines.values())


def warm(census, registry, balances):
    """Preload what the rolls read, in batches per package (only a speed-up; props() decodes the rest on demand)."""
    paths = {GLOBALS}
    for balance in balances:
        record = registry['balances'][balance]
        paths.add(balance)
        paths.update(record['definitions'])
        for variant in record['variants']:
            for entries in variant['slots'].values():
                paths.update(c['part'] for c in entries)
    for path in list(paths):
        for table in ('definitions', 'parts'):
            entry = registry[table].get(path) or {}
            paths.update(entry.get('prefix') or [])
            paths.update(entry.get('title') or [])
    seen, frontier = set(), paths
    for _ in range(4):  # then the attributes, resolvers and init definitions they name (rarity, weights, windows)
        frontier = {p for p in frontier if p in census.owner and p not in seen}
        if not frontier:
            break
        census.preload(sorted(frontier))
        seen |= frontier
        found = set()
        for path in frontier:
            for ref in weapon_recipe.references(census.package(census.owner[path]).cache.get(path)):
                cls = item_registry.short(census.classes.get(ref, ''))
                if ref in census.owner and (cls.startswith('Attribute') or cls.endswith('Resolver')):
                    found.add(ref)
        frontier = found


def spread_seeds(count):
    """`count` seeds spread over the 32-bit range. CrtRand's first draw grows by about 3.3 per seed step, so consecutive small
    seeds (1, 2, 3, ...) all draw near 0 first and favour each list's first candidate; spread seeds do not."""
    return [(index * 0x9E3779B1) & 0xFFFFFFFF for index in range(1, count + 1)]


def run_census(registry, lookup, rolls=DEFAULT_ROLLS, stages=CENSUS_STAGES):
    """Every balance of the four kinds, `rolls` spread seeds at each stage; check() on each roll."""
    counts = defaultdict(Counter)
    failures, not_rollable = [], Counter()
    slots = defaultdict(lambda: defaultdict(Counter))  # kind -> slot -> part -> times chosen
    filled = defaultdict(Counter)  # kind -> slot -> rolls with the slot filled
    bands = defaultdict(Counter)
    notes = Counter()
    for balance, record in sorted(registry['balances'].items()):
        if record['kind'] not in KINDS:
            continue
        kind = record['kind']
        counts[kind]['balances'] += 1
        open_window = {c['part'] for v in record['variants'] for entries in v['slots'].values() for c in entries
                       if isinstance(c['min_stage'], dict) or isinstance(c['max_stage'], dict)}
        for stage in stages:
            for seed in spread_seeds(rolls):
                try:
                    item = roll(registry, balance, stage, seed, lookup)
                except NotRollable as error:
                    not_rollable[(kind, error.reason)] += 1
                    counts[kind]['not_rollable'] += 1
                    continue
                counts[kind]['rolls'] += 1
                problems = check(registry, item, lookup)
                if problems:
                    counts[kind]['failures'] += 1
                    failures.append({'balance': balance, 'stage': stage, 'seed': seed, 'problems': problems})
                for slot, part in item['parts'].items():
                    slots[kind][slot][part] += 1
                    filled[kind][slot] += 1
                bands[kind][str(item['rarity']['band'])] += 1
                if item['rarity']['unresolved']:
                    notes['rarity_term_unresolved'] += 1
                if open_window & set(item['parts'].values()):
                    notes['rolls_drawing_a_part_whose_stage_window_needs_runtime_state'] += 1
                if item['hook_removed']:
                    notes['relic_rolls_where_the_hook_removed_parts'] += 1
                for note in item['notes']:
                    notes[note.split(':', 1)[1].strip()] += 1
    return {
        'counts': {k: dict(v) for k, v in counts.items()},
        'not_rollable': {f'{k}/{r}': n for (k, r), n in sorted(not_rollable.items())},
        'failures': failures,
        'slot_filled': {k: dict(v) for k, v in filled.items()},
        'distinct_parts_drawn': {k: {s: len(p) for s, p in v.items()} for k, v in slots.items()},
        'slot_frequencies': {k: {s: dict(p.most_common()) for s, p in v.items()} for k, v in slots.items()},
        'bands': {k: dict(v) for k, v in bands.items()},
        'notes': dict(notes),
    }


def stock_shield(registry, lookup, card_path=ROOT / 'local/slice/stock_shield_gear.json'):
    """roll() for prepare_stock_shield's balance, level and seed, against that tool's own roll and its card file."""
    balance, stage, seed = ps.DEFAULT_BALANCE, ps.SLICE_LEVEL, ps.DEFAULT_SEED
    theirs = ps.roll(ps.candidates(lookup, balance, stage), seed)
    record = registry['balances'][balance]
    first = record['manufacturers'][0]['manufacturer'] if record['manufacturers'] else None
    forced = roll(registry, balance, stage, seed, lookup, manufacturer=first)
    drawn = roll(registry, balance, stage, seed, lookup)
    out = {'manufacturers_at_stage': len(manufacturer_options(record, stage)),
           'first_manufacturer_equal_parts': forced['parts'] == theirs,
           'drawn_manufacturer_is_first': drawn['manufacturer'] == first,
           'drawn_equal_parts': drawn['parts'] == theirs,
           'slots_differing_when_drawn': sorted(s for s in SLOTS if drawn['parts'].get(s) != theirs.get(s))}
    if card_path.is_file():
        card = json.loads(card_path.read_text(encoding='utf-8'))['items'][0]['provenance']
        out['card_present'] = True
        out['card_equal_parts'] = card['parts'] == forced['parts']
        out['card_equal_name_parts'] = card['name_parts'] == forced['name_parts']
    else:
        out['card_present'] = False
    return out


def summary_lines(report):
    """Counts only (no object names)."""
    lines = [f"seconds {report['seconds']} (census open {report['open_seconds']}), rolls per balance and stage "
             f"{report['rolls_per_stage']}, stages {report['stages']}"]
    for kind, counts in report['census']['counts'].items():
        lines.append(f'{kind}: ' + ', '.join(f'{k} {v}' for k, v in counts.items()))
    lines.append('not rollable: ' + json.dumps(report['census']['not_rollable']))
    lines.append(f"checker failures: {len(report['census']['failures'])}")
    lines.append('slots filled: ' + json.dumps(report['census']['slot_filled']))
    lines.append('distinct parts drawn: ' + json.dumps(report['census']['distinct_parts_drawn']))
    lines.append('bands: ' + json.dumps(report['census']['bands']))
    lines.append('notes: ' + json.dumps(report['census']['notes']))
    lines.append('stock shield: ' + json.dumps(report['stock_shield']))
    lines.append('slot name rows: ' + json.dumps(report['slot_name_rows']))
    return lines


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--census', action='store_true', help='roll and check every balance of the four kinds')
    parser.add_argument('--registry', default=DEFAULT_REGISTRY)
    parser.add_argument('--reader', default=str(item_registry.DEFAULT_READER), help='ow-package executable')
    parser.add_argument('--game', default=str(weapon_catalog.DEFAULT_GAME), help='Borderlands 2 install directory')
    parser.add_argument('--out', default=DEFAULT_OUT, help='output folder inside local/ (default %(default)s)')
    parser.add_argument('--rolls', type=int, default=DEFAULT_ROLLS, help='seeds per balance and stage')
    parser.add_argument('--jobs', type=int, default=8)
    args = parser.parse_args(argv)
    if not args.census:
        parser.error('nothing to do: pass --census')
    try:
        out_dir = pv.local_path(args.out)
    except ValueError as error:
        print(error, file=sys.stderr)
        return 2
    registry_path = Path(args.registry)
    if not registry_path.is_absolute():
        registry_path = ROOT / registry_path
    if not registry_path.is_file():
        print(f'{registry_path}: no registry (run tools/item_registry.py first)', file=sys.stderr)
        return 2
    reader = Path(args.reader).resolve()
    if not reader.is_file():
        print(f'{reader}: ow-package not found (build the core first)', file=sys.stderr)
        return 2
    started = time.perf_counter()
    registry = json.loads(registry_path.read_text(encoding='utf-8'))
    out_dir.mkdir(parents=True, exist_ok=True)
    schema = out_dir / 'item_roll.schema'
    schema.write_text('\n'.join(schema_lines()) + '\n', encoding='utf-8')
    census, _, errors = item_registry.open_census(str(reader), Path(args.game), schema.resolve(), args.jobs)
    opened = time.perf_counter() - started
    balances = [b for b, r in registry['balances'].items() if r['kind'] in KINDS]
    warm(census, registry, balances)
    report = {'rolls_per_stage': args.rolls, 'stages': list(CENSUS_STAGES), 'scan_errors': len(errors),
              'census': run_census(registry, census, args.rolls),
              'stock_shield': stock_shield(registry, census),
              'slot_name_rows': none_slot_names(registry, census, balances)}
    report['open_seconds'] = round(opened, 1)
    report['seconds'] = round(time.perf_counter() - started, 1)
    item_registry.write_json(out_dir / 'census.json', report)
    for line in summary_lines(report):
        print(line)
    print(f'wrote {out_dir / "census.json"}')
    return 1 if report['census']['failures'] else 0


if __name__ == '__main__':
    sys.exit(main())
