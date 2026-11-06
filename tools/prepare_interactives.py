"""Prepare a map's lootable objects, treasure chests and their pickups for the vendors lane (PR 3).

AI-assisted. Clean-room: this file contains no game data. It reads the installed packages with ow-package and writes
one JSON document (schema openwillow-interactives/1) under the ignored local/slice/ folder (game data, docs/LEGAL.md).

What it lists: every WillowPopulationOpportunityPoint whose PopulationDef is a GD_Population_Treasure.* population
(lootables such as cardboard boxes, strong boxes, lockers, dumpsters, laundry machines; the Dahl ammo crate; treasure
chests). Each step follows references; nothing is looked up by a remembered offset:
- point -> PopulationDef -> its PopulationFactoryInteractiveObject -> ObjectBalanceDefinition (an
  InteractiveObjectBalanceDefinition) -> DefaultInteractiveObject (the InteractiveObjectDefinition);
- the balance's loot configurations come from three places: its own DefaultLoot, the LootData of every
  InteractiveObjectLootListDefinition in DefaultIncludedLootLists, and the CustomLoot / IncludedCustomLootLists of the
  grade whose GameStageRequirement holds --stage. Each configuration has a Weight and ItemAttachments
  {ItemPool, PoolProbability, AttachmentPointName}; pools are expanded with tools/loot_pools.py as
  tools/prepare_vendors.py does (leaf balances with their share of the pool at --stage);
- the visual is the definition's mesh components (skeletal mesh, physics asset, anim tree, anim sets and their
  sequence names) and the anim names its Behavior_SimpleAnimPlay behaviours play;
- what it needs: a Behavior_SetUsabilityCost (the golden chest: one CURRENCY_Reserved_A, read as a golden key), the
  missions its BehaviorSequenceEnableByMission objects link to, and any Behavior_UpdateMissionObjective (a mission
  object). Objects with a cost or a mission objective are marked supported false;
- pickups: every non-weapon leaf gets the item block tools/prepare_vendors.py writes (item_block), and the ones that
  are picked up on touch (cash, eridium, ammo, health) get a `pickup` block with the amount rule.

UNVERIFIED in game (none of this has been compared with the running game yet):
- how the game picks one configuration (read here as a weighted choice by Weight) and how DefaultLoot, included lists
  and grade loot combine (listed side by side with their source);
- that the "Open" behaviour animation plays the anim set's "Open" sequence (matched by name);
- that CURRENCY_Reserved_A is the golden key (the Sanctuary point's Description says "Golden Key chest");
- the cash rule: slot value = calc(level) * BaseValueScaleConstant + calc(level) * PerGradeUpgrade scale * grade, with
  calc = GD_Economy.CashPickups.Init_CashPickupCalc and grade = the drawn parts' GradeIncrease; the rounding to whole
  dollars and whether the balance's manufacturer grade adds to the slot grade are not known;
- ammo amounts are evaluated for playthrough 1 and an unshared drop (PlayThroughCount 1, ClonedForSharing 0).
"""
import argparse
import json
import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import prepare_vendors as pv  # noqa: E402
import weapon_recipe  # noqa: E402
import weapon_stats  # noqa: E402
from loot_pools import SCHEMA_LINES, Values  # noqa: E402
from prepare_slice_world import Reader, levels, vector  # noqa: E402

REPO = Path(__file__).resolve().parents[1]
SCHEMA_PATH = REPO / 'local' / 'slice' / 'interactives.schema'
SCHEMA_TAG = 'openwillow-interactives/1'
POPULATION = 'GD_Population_Treasure.'
# Arrays the vendors schema does not name yet (each name once: the reader rejects repeats).
SCHEMA_EXTRA = pv.SCHEMA_EXTRA + [
    'DefaultIncludedLootLists=ObjectProperty',
    'Grades=StructProperty:InteractiveObjectBalanceGrade',
    'IncludedCustomLootLists=ObjectProperty',
    'CustomLoot=StructProperty:LootConfigurationData',
    'LootData=StructProperty:LootConfigurationData',
    'AttributeSlotEffects=StructProperty:AttributeSlotEffectData',
    'AttributeSlotUpgrades=StructProperty:AttributeSlotUpgradeData',
    'AnimSets=ObjectProperty',
    'Sequences=ObjectProperty',
    'SimpleAnimNames=NameProperty',
]
GOLDEN_KEY = 'CURRENCY_Reserved_A'  # the golden chest's usability cost currency (UNVERIFIED as "golden key")
# Attribute context for the pickup amounts: normal mode, a drop that is not a co-op shared copy (UNVERIFIED).
CONTEXT = {'D_Attributes.Balance.PlayThroughCount': 1.0, 'D_Attributes.Inventory.ClonedForSharing': 0.0}
CREDITS = 'D_Attributes.Currency.CreditsOnHand'
ERIDIUM = 'D_Attributes.Currency.EridiumOnHand'
# Item levels at which the cash rule is tabulated for the host (any level can be evaluated from the formula).
CASH_LEVELS = (1, 5, 7, 8, 9, 10, 15, 20, 30, 40, 50, 60, 70, 80)
MESH_CLASSES = ('SkeletalMeshComponent', 'StaticMeshComponent')


def short(cls):
    return (cls or '').rsplit('.', 1)[-1]


def kind_of(popdef):
    """chest (a TreasureChests population), ammo_crate (a lootable population named for ammo) or lootable."""
    name = popdef.rsplit('.', 1)[-1]
    if '.TreasureChests.' in popdef:
        return 'chest'
    return 'ammo_crate' if 'Ammo' in name else 'lootable'


def children(chain, path, depth=None):
    """Sorted export paths under `path` (direct children only when depth is 1)."""
    base = path.count('.')
    return sorted(p for p in chain.classes if p.startswith(path + '.')
                  and (depth is None or p.count('.') - base == depth))


def owner_of(chain, path):
    return next((n for n in chain.names if path in chain.reader.exports(n)['by_path']), None)


def sequence_names(chain, anim_set):
    """SequenceName of every AnimSequence an AnimSet lists. A sequence ends in native key data, so its tagged
    properties are read with the trailing bytes allowed (only SequenceName is used)."""
    names = []
    for sequence in chain.props(anim_set).get('Sequences') or []:
        owner = owner_of(chain, sequence)
        if owner is None:
            continue
        try:
            names.append(pv.plain_refs(chain.reader.props(owner, sequence, allow_tail=True)).get('SequenceName'))
        except ValueError:
            names.append(None)
    return names


def visual(chain, definition):
    """The definition's distinct mesh components, the anims its behaviours play and its particle templates."""
    components, seen = [], set()
    for path in children(chain, definition, depth=1):
        cls = short(chain.classes[path])
        if cls not in MESH_CLASSES:
            continue
        props = chain.props(path)
        entry = {'class': cls, 'skeletal_mesh': props.get('SkeletalMesh'), 'static_mesh': props.get('StaticMesh'),
                 'physics_asset': props.get('PhysicsAsset'), 'anim_tree': props.get('AnimTreeTemplate'),
                 'anim_sets': props.get('AnimSets') or [], 'scale': props.get('Scale', 1.0),
                 'translation': vector(props.get('Translation')) if props.get('Translation') else [0.0, 0.0, 0.0]}
        key = json.dumps(entry, sort_keys=True)
        if key in seen:
            continue  # two identical components (the strong box, the Dahl epic crate) are drawn as one here
        seen.add(key)
        entry['sequences'] = {s: sequence_names(chain, s) for s in entry['anim_sets']}
        entry['tree_anim_names'] = chain.props(entry['anim_tree']).get('SimpleAnimNames') if entry['anim_tree'] else None
        components.append(entry)
    played, particles = set(), set()
    for path in children(chain, definition):
        cls = short(chain.classes[path])
        if cls == 'Behavior_SimpleAnimPlay' and chain.props(path).get('AnimName'):
            played.add(chain.props(path)['AnimName'])
        elif cls == 'ParticleSystemComponent' and chain.props(path).get('Template'):
            particles.add(chain.props(path)['Template'])
    sequences = {n for c in components for names in c['sequences'].values() for n in names if n}
    return {'components': components, 'played_anims': sorted(played),
            'open_animation': 'Open' if 'Open' in played else None,
            # The anim set sequence the "Open" behaviour is taken to play (same name; UNVERIFIED).
            'open_sequence': 'Open' if 'Open' in played and 'Open' in sequences else None,
            'particles': sorted(particles)}


def requirements(chain, definition):
    """{cost, missions, updates_objectives} read from the definition's behaviours."""
    cost, missions, objectives = None, set(), set()
    for path in children(chain, definition):
        cls, props = short(chain.classes[path]), None
        if cls == 'Behavior_SetUsabilityCost':
            props = chain.props(path)
            if props.get('ChangeCost') == 'CHANGE_Enable':
                currency = props.get('CostCurrencyType')
                cost = {'currency': currency, 'amount': props.get('CostAmount'),
                        'golden_key': currency == GOLDEN_KEY}
        elif cls == 'BehaviorSequenceEnableByMission':
            props = chain.props(path)
            if props.get('LinkedMission'):
                missions.add((props['LinkedMission'], props.get('LinkedObjective')))
        elif cls == 'Behavior_UpdateMissionObjective':
            objective = chain.props(path).get('MissionObjective')
            if objective:
                objectives.add(objective)
    return {'cost': cost, 'missions': [{'mission': m, 'objective': o} for m, o in sorted(missions, key=str)],
            'updates_objectives': sorted(objectives)}


def grade_at(balance, stage):
    """The first grade whose GameStageRequirement holds `stage`, or None."""
    for grade in balance.get('Grades') or []:
        need = grade.get('GameStageRequirement') or {}
        if need.get('MinGameStage', 0) <= stage <= need.get('MaxGameStage', 1e9):
            return grade
    return None


def configuration_sources(chain, balance_props, stage):
    """[(source, LootConfigurationData)] in the order the balance names them."""
    found = [('DefaultLoot', c) for c in balance_props.get('DefaultLoot') or []]
    lists = list(balance_props.get('DefaultIncludedLootLists') or [])
    modifiers = (grade_at(balance_props, stage) or {}).get('GradeModifiers') or {}
    found += [('grade CustomLoot', c) for c in modifiers.get('CustomLoot') or []]
    lists += list(modifiers.get('IncludedCustomLootLists') or [])
    for path in lists:
        if path:
            found += [(path, c) for c in chain.props(path).get('LootData') or []]
    return found


def configurations(chain, values, balance_path, stage, unresolved):
    """The balance's loot configurations: name, source, weight, share and the expanded attachments."""
    rows = []
    for source, config in configuration_sources(chain, chain.props(balance_path), stage):
        name = config.get('ConfigurationName')
        weight = values.init(config.get('Weight'))
        if weight is None:
            unresolved.append({'balance': balance_path, 'config': name, 'reason': 'Weight not resolved from the packages'})
        attachments = []
        for attachment in config.get('ItemAttachments') or []:
            found = []
            for row in pv.stock(chain, values, [attachment], balance_path, name, stage, found):
                attachments.append({'point': attachment.get('AttachmentPointName'), **row})
            unresolved += [{'balance': u.pop('machine'), **u} for u in found]
        rows.append({'name': name, 'source': source, 'weight': weight, 'share': None,
                     'stage_variance': config.get('LootGameStageVarianceFormula'), 'attachments': attachments})
    total = sum(r['weight'] for r in rows if r['weight'] is not None)
    for row in rows:
        # Share among the configurations whose weight resolves (one with an unresolved weight is left out).
        if row['weight'] is not None and total > 0:
            row['share'] = round(row['weight'] / total, 6)
    return rows


# ---- Pickups: cash, eridium, ammo and health picked up on touch ------------------------------------------------------
def init_amount(chain, values, init):
    """An AttributeInitializationData evaluated like loot_pools.Values, plus ConditionalInitialization and the
    integer rounding mode (the ammo amount definitions use both). None when it does not resolve."""
    init = init or {}
    definition = init.get('InitializationDefinition')
    if not definition:
        return values.init(init)
    props = chain.props(definition)
    if (props.get('BaseValueMode') or 'BASEVALUE_InitializationDefSetsBaseValue') != \
            'BASEVALUE_InitializationDefSetsBaseValue':
        values.unresolved.add(definition)
        return None
    conditional = props.get('ConditionalInitialization') or {}
    formula = props.get('ValueFormula') or {}
    if conditional.get('bEnabled'):
        value = values.conditional(conditional)
    elif formula.get('bEnabled'):
        terms = {k: init_amount(chain, values, formula.get(k)) for k in ('Multiplier', 'Level', 'Power', 'Offset')}
        value = None if None in terms.values() else weapon_recipe.formula_value(
            terms['Multiplier'], terms['Level'], terms['Power'], terms['Offset'])
    else:
        value = None
    if value is None:
        values.unresolved.add(definition)
        return None
    if props.get('RoundingMode') == 'ATTRROUNDING_IntRound':
        value = math.floor(value + 0.5)
    scale = init.get('BaseValueScaleConstant')
    return value * (1.0 if scale is None else scale)


def slot_grades(chain, definition, slot):
    """{part list slot: [GradeIncrease of each part for `slot`]} over the definition's part lists."""
    found = {}
    for name in pv.ITEM_SLOTS:
        source = chain.props(definition).get(f'{name}Parts')
        if not source:
            continue
        grades = []
        for entry in chain.props(source).get('WeightedParts') or []:
            if not entry.get('Part'):
                continue
            for upgrade in chain.props(entry['Part']).get('AttributeSlotUpgrades') or []:
                if upgrade.get('SlotName') == slot:
                    grades.append(upgrade.get('GradeIncrease', 0))
        if grades:
            found[name] = grades
    return found


def cash_rule(chain, definition, effect):
    """The cash amount rule of one credits slot effect (see the module docstring), or None."""
    base, per_grade = effect.get('BaseModifierValue') or {}, effect.get('PerGradeUpgrade') or {}
    calculator = base.get('InitializationDefinition')
    if not calculator or per_grade.get('InitializationDefinition') not in (None, calculator):
        return None
    level_attributes = [a for a in pv.init_attributes(chain, {'InitializationDefinition': calculator})
                        if pv.object_property(chain, a) == pv.LEVEL_PROPERTY]
    missing = set()

    def calc(level):
        known = {a: float(level) for a in level_attributes}
        return weapon_stats.price_init(chain, {'InitializationDefinition': calculator}, known, missing)

    grades = slot_grades(chain, definition, effect.get('SlotName'))
    low = sum(min(g) for g in grades.values())
    high = sum(max(g) for g in grades.values())
    base_scale = base.get('BaseValueScaleConstant', 1.0)
    grade_scale = per_grade.get('BaseValueScaleConstant', 0.0) if per_grade.get('InitializationDefinition') else 0.0
    table = {}
    for level in CASH_LEVELS:
        value = calc(level)
        if value is None:
            break
        table[str(level)] = [round(value * (base_scale + grade_scale * g), 3) for g in (low, high)]
    formula = chain.props(calculator).get('ValueFormula') or {}
    constants = {k: weapon_stats.resolver_constant(chain, (formula.get(k) or {}).get('BaseValueAttribute'))
                 for k in ('Multiplier', 'Level') if (formula.get(k) or {}).get('BaseValueAttribute')}
    return {'calculator': calculator, 'level_attributes': sorted(level_attributes),
            'multiplier_attribute': (formula.get('Multiplier') or {}).get('BaseValueAttribute'),
            'level_attribute': (formula.get('Level') or {}).get('BaseValueAttribute'),
            'constants': constants, 'base_scale': base_scale, 'per_grade_scale': grade_scale,
            'slot': effect.get('SlotName'), 'slot_grades': grades, 'grade_range': [low, high],
            'amount_at_level': table, 'unresolved': sorted(missing),
            'formula': 'calc(L) * (base_scale + per_grade_scale * grade); '
                       'calc(L) = Multiplier * Level ^ L (L = the item level, ExpLevel)'}


def pickup_block(chain, values, balance, item):
    """{kind, auto_pickup, mesh, ...amount rule} for an item picked up on touch, or None."""
    steps = pv.base_chain(chain, balance)
    definition = next((chain.props(b).get('InventoryDefinition') for b in steps[::-1]
                       if chain.props(b).get('InventoryDefinition')), None)
    props = chain.props(definition) if definition else {}
    if not props.get('bPlayerUseItemOnPickup'):
        return None
    grade = None
    for maker in chain.props(balance).get('Manufacturers') or []:
        for entry in maker.get('Grades') or []:
            grade = (entry.get('GradeModifiers') or {}).get('ExpLevel', grade)
    block = {'definition': definition, 'auto_pickup': bool(props.get('bAutomaticallyPickup')),
             'mesh': props.get('NonCompositeStaticMesh'), 'manufacturer_grade': grade}
    for effect in props.get('AttributeSlotEffects') or []:
        if effect.get('AttributeToModify') == CREDITS:
            block.update(kind='cash', cash=cash_rule(chain, definition, effect))
            return block
    for effect in props.get('ExternalAttributeEffects') or []:
        if effect.get('AttributeToModify') == ERIDIUM:
            block.update(kind='eridium', amount=init_amount(chain, values, effect.get('BaseModifierValue')))
            return block
    if item.get('kind') == 'ammo':
        # The effect whose attribute names an ammo resource, as prepare_vendors.ammo_effect picks it.
        effect = next((e for e in props.get('ExternalAttributeEffects') or [] if e.get('AttributeToModify') and
                       '.AmmoResources.' in str(chain.props(e['AttributeToModify']).get('Resource'))), {})
        if not effect:
            values.unresolved.add(f'{definition}: no ammo resource effect')
        block.update(kind='ammo', ammo_type=item.get('ammo_type'), ammo_pool=item.get('ammo_pool'),
                     amount=init_amount(chain, values, effect['BaseModifierValue']) if effect else None)
        return block
    if item.get('kind') == 'health':
        block.update(kind='health', heal=item.get('heal'))
        return block
    block['kind'] = 'other'
    return block


def describe_items(reader, persistent, balances, stage):
    """{leaf balance: item block (+ pickup)} for every non-weapon leaf of every configuration."""
    chain = pv.Chain(reader, [persistent, 'Startup'])
    values = Values(chain, CONTEXT)
    items = {}
    for record in balances.values():
        for config in record['configurations']:
            for attachment in config['attachments']:
                for leaf in attachment['items']:
                    if leaf['class'] == 'WeaponBalanceDefinition' or leaf['balance'] in items:
                        continue
                    item = pv.item_block(chain, leaf['balance'], stage)
                    pickup = pickup_block(chain, values, leaf['balance'], item)
                    if pickup:
                        item['pickup'] = pickup
                    items[leaf['balance']] = item
    return items, sorted(values.unresolved)


def assets(balances, items):
    """The distinct package objects the host has to import (meshes, physics, animation, particles, pickup meshes)."""
    found = {k: set() for k in ('skeletal_meshes', 'static_meshes', 'physics_assets', 'anim_trees', 'anim_sets',
                                'particles', 'pickup_meshes')}
    for record in balances.values():
        for component in (record['visual'] or {}).get('components', []):
            for key, field in (('skeletal_meshes', 'skeletal_mesh'), ('static_meshes', 'static_mesh'),
                               ('physics_assets', 'physics_asset'), ('anim_trees', 'anim_tree')):
                if component.get(field):
                    found[key].add(component[field])
            found['anim_sets'] |= set(component['anim_sets'])
        found['particles'] |= set((record['visual'] or {}).get('particles', []))
    for item in items.values():
        if (item.get('pickup') or {}).get('mesh'):
            found['pickup_meshes'].add(item['pickup']['mesh'])
    return {k: sorted(v) for k, v in found.items()}


def describe_balance(chain, values, balance_path, stage, unresolved):
    props = chain.props(balance_path)
    definition = props.get('DefaultInteractiveObject')
    return {'definition': definition,
            'configurations': configurations(chain, values, balance_path, stage, unresolved),
            'visual': visual(chain, definition) if definition else None,
            'requires': requirements(chain, definition) if definition else None,
            'hud_icon': chain.props(definition).get('HUDIconDef') if definition else None}


def build_document(reader, persistent, stage):
    objects, balances, unresolved, missing, attributes = [], {}, [], set(), set()
    for level in levels(reader, persistent):
        # The level's own package first: a placed point's path exists in more than one level package.
        chain = pv.Chain(reader, [level, persistent, 'Startup'])
        values = Values(chain, CONTEXT)
        for export in list(reader.exports(level)['by_index'].values()):
            path = export['path']
            if path.count('.') != 2 or not path.startswith('TheWorld.PersistentLevel.'):
                continue
            if short(export['class']) != 'WillowPopulationOpportunityPoint':
                continue
            point = chain.props(path)
            popdef = str(point.get('PopulationDef') or '')
            if not popdef.startswith(POPULATION):
                continue
            object_id = f'{level}:{path.rsplit(".", 1)[1]}'
            factory = pv.factory_of(chain, popdef)
            balance = chain.props(factory).get('ObjectBalanceDefinition') if factory else None
            if not balance or not chain.props(balance):
                unresolved.append({'object': object_id, 'reason': f'{popdef}: no factory balance that resolves'})
                continue
            if balance not in balances:
                balances[balance] = describe_balance(chain, values, balance, stage, unresolved)
            record = balances[balance]
            needs = record['requires'] or {}
            reasons = []
            if needs.get('cost'):
                reasons.append('needs a golden key' if needs['cost']['golden_key'] else 'has a usability cost')
            if needs.get('updates_objectives'):
                reasons.append('mission object: ' + ', '.join(needs['updates_objectives']))
            rotation = point.get('Rotation') or {}
            objects.append({
                'id': object_id, 'kind': kind_of(popdef), 'population': popdef, 'balance': balance,
                'definition': record['definition'],
                'location': vector(point.get('Location')),
                # Degrees [Pitch, Yaw, Roll] like prepare_vendors; the 65536-per-turn units are kept beside them.
                'rotation': [int(rotation.get(k) or 0) * 360 / 65536 for k in ('Pitch', 'Yaw', 'Roll')],
                'rotation_units': [int(rotation.get(k) or 0) for k in ('Pitch', 'Yaw', 'Roll')],
                'game_stage': {'region': point.get('GameStageRegion'), 'min': point.get('MinimumGameStageForRegion'),
                               'max': point.get('MaximumGameStageForRegion')},
                # What this bit means (per-map opened state or mission use) is UNVERIFIED; kept raw.
                'mission_objective_bit': point.get('MissionObjectiveBit'),
                'description': point.get('Description'),
                'supported': not reasons, 'unsupported_reason': '; '.join(reasons) or None})
        attributes |= values.unresolved
        missing |= chain.missing
        for path, error in sorted(chain.failed.items()):
            if '.BehaviorProviderDefinition_' in path and path.count('.') <= 4:
                continue  # behaviour providers hold BehaviorSequences the reader cannot decode; never needed here
            unresolved.append({'path': path, 'reason': f'did not decode: {error}'})
    items, item_attributes = describe_items(reader, persistent, balances, stage)
    kinds = {}
    for obj in objects:
        kinds[obj['kind']] = kinds.get(obj['kind'], 0) + 1
    notes = [
        'configuration share = Weight / sum of the resolved Weights of the same balance (a weighted choice; UNVERIFIED).',
        'items: the tools/prepare_vendors.py item block per non-weapon leaf; pickup = the amount rule of an item picked '
        'up on touch (cash, eridium, ammo at playthrough 1, health). UNVERIFIED in game.',
        f'{len(attributes)} attribute paths did not resolve from the packages: {sorted(attributes)}',
        f'{len(item_attributes)} pickup attribute paths did not resolve: {item_attributes}',
        f'{len(missing)} object paths are in none of the packages: {sorted(missing)}',
    ]
    return {'schema': SCHEMA_TAG, 'map': persistent, 'stage': stage, 'counts': kinds, 'objects': objects,
            'balances': balances, 'items': items, 'assets': assets(balances, items),
            'unresolved': unresolved, 'notes': notes}


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', required=True, help='ow-package executable')
    parser.add_argument('--game', required=True, help='Borderlands 2 install directory')
    parser.add_argument('--map', default='Sanctuary_P')
    parser.add_argument('--output', default='local/slice/interactives.json')
    parser.add_argument('--stage', type=float, default=8, help='game stage for the pool quantities, gates and grades')
    args = parser.parse_args()
    try:
        output = pv.local_path(args.output)
    except ValueError as error:
        print(error, file=sys.stderr)
        return 2
    SCHEMA_PATH.parent.mkdir(parents=True, exist_ok=True)
    SCHEMA_PATH.write_text('\n'.join(SCHEMA_EXTRA + SCHEMA_LINES) + '\n', encoding='utf-8')
    reader = Reader(args.reader, args.game, str(SCHEMA_PATH))
    document = build_document(reader, args.map, args.stage)
    if not document['objects']:
        print(f'{args.map}: no lootable or chest found', file=sys.stderr)
        return 1
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(document, indent=1), encoding='utf-8')
    print(f'wrote {output}: {len(document["objects"])} objects {document["counts"]}, '
          f'{len(document["balances"])} balances, {len(document["items"])} non-weapon items, '
          f'{len(document["unresolved"])} unresolved')
    return 0


if __name__ == '__main__':
    sys.exit(main())
