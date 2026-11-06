"""Resolve the stock grenade mod into a `grenade` block for the UE host.

Input: the cooked Startup.upk (through tools/skill_stats.py) and WillowGame.upk for class defaults. The mod is
GD_GrenadeMods.A_Item.GrenadeMod_Standard (WillowGame.GrenadeModDefinition). One part is chosen per part list
(`--part Trigger=<part>`; by default the first WeightedParts entry of each list), then:

- projectile: DefaultProjectileDefinition. Speed is its SpeedFormula (no speed modifier from the chosen parts is
  applied; a part's ProjectileBehavior_SetSpeed is listed, not applied). Gravity scale and physics mode are read
  from the definition. Its behaviours come from tools/weapon_firing.py projectile_of().
- slots: each chosen part's AttributeSlotUpgrades adds GradeIncrease to its slot's grade and activates it. Every
  activated AttributeSlotEffects entry of the mod gives slot_value(grade, base, per grade) (weapon_stats rules:
  grade = round-half-up(base grade 1) + increases; value = (grade - 1) x PerGradeUpgrade + BaseModifierValue).
- fuse_time: the FuseTime slot total, starting from 0, with the projectile's Behavior_ModifyTimer delays listed beside it.
- contact (under projectile): what the projectile's behaviour graph does on contact and at spawn, read by event, see
  contact_rules(). Counting behaviours does not say this: a Detonate that only a timer reaches is not an impact fuse.
- radius: the Behavior_Explode DamageRadiusFormula, plus the BlastRadius slot total (MT_PreAdd).
- damage: Init_ProjectileDamage evaluated with ProjectileLevel = --level, times the projectile's Damage
  BaseValueScaleConstant, then x (1 + the GrenadeDamage Scale total of the Damage slot).
- child_count: the NumberOfChildProjectiles slot total. damage_type comes from the explosion definition.
- fx: tools/weapon_projectile_fx.py build() on the explosion and the in-flight trail.

Every rule that is inferred rather than read from a field is listed in `unverified`. Unresolved values are null with a
`reason`. The output is written to local/items/grenades/<id>.json (ignored game-derived data); a path outside the
repository's local/ folder is refused before anything is read.

  python tools/grenade_mod.py --game "<BL2 install>" --level 8 [--part Trigger=GD_GrenadeMods.Trigger.Trigger_Grade5]
"""
import argparse
import json
import os
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tools'))
import skill_stats  # noqa: E402
import weapon_firing  # noqa: E402
import weapon_projectile_fx  # noqa: E402
import weapon_recipe  # noqa: E402
import weapon_stats  # noqa: E402
from prepare_action_skill import read_provider  # noqa: E402

# The projectile's behaviour graph (prepare_action_skill.read_provider decodes it).
PROVIDER_SCHEMA_LINES = [
    'BehaviorSequences=StructProperty:BehaviorSequenceData',
    'EventData2=StructProperty:BehaviorEventData',
    'BehaviorData2=StructProperty:BehaviorData',
    'ConsolidatedOutputLinkData=StructProperty:BehaviorOutputLinkData',
    'VariableData=StructProperty:BehaviorVariableData',
    'ConsolidatedVariableLinkData=StructProperty:BehaviorVariableLinkData',
    'ConsolidatedLinkedVariables=IntProperty',
]
SCHEMA_LINES = list(dict.fromkeys(weapon_firing.SCHEMA_LINES + weapon_projectile_fx.SCHEMA_LINES + PROVIDER_SCHEMA_LINES))
GRENADE_MOD = 'GD_GrenadeMods.A_Item.GrenadeMod_Standard'
# Part kind (the --part key) -> the GrenadeModDefinition field that names its part list.
PART_LISTS = {
    'Payload': 'AlphaParts', 'Delivery': 'BetaParts', 'Trigger': 'GammaParts', 'Accessory': 'DeltaParts',
    'Damage': 'EpsilonParts', 'DamageRadius': 'ZetaParts', 'ChildCount': 'EtaParts',
    'StatusDamage': 'ThetaParts', 'Material': 'MaterialParts',
}
ATTR = 'D_Attributes.GrenadeMod.'
FUSE_ATTRIBUTE = ATTR + 'FuseTime'
RADIUS_ATTRIBUTE = ATTR + 'BlastRadius'
DAMAGE_ATTRIBUTE = ATTR + 'GrenadeDamage'
CHILDREN_ATTRIBUTE = ATTR + 'NumberOfChildProjectiles'
STATUS_ATTRIBUTE = ATTR + 'GrenadeModStatusEffectDamage'
PROJECTILE_LEVEL = 'D_Attributes.Projectile.ProjectileLevel'
# Element slots (AttributeSlotEffects names) -> the host's element names (as weapon_firing names them).
ELEMENT_SLOTS = {'IsExplosive': 'Explosive', 'IsIncendiary': 'Fire', 'IsCorrosive': 'Corrosive',
                 'IsShock': 'Shock', 'IsSlag': 'Slag'}
ALWAYS_UNVERIFIED = [
    'slot grade: base grade 1 (GrenadeModDefinition has no AttributeSlotBaseGrade; the weapon default is used)',
    'FuseTime starts from 0 (the attribute has no base field in the mod); fuse = 0 + PreAdd slot total',
    'each projectile timer fires the event of its own number, an omitted TimerId being the first (read from the packages)',
    'the FuseTime slot total is not applied to the projectile timers in contact (how a mod changes them is not measured)',
    'BlastRadius is added to the Behavior_Explode DamageRadiusFormula (PreAdd on that constant)',
    'GrenadeDamage is a multiplier on the projectile damage, starting at 1 (Scale of the Damage slot)',
    'ProjectileLevel equals --level (the attribute has no constant resolver)',
    'the projectile Damage RangeRestriction applies to the scaled value (weapon_recipe rule)',
    'speed: SpeedFormula, with no speed modifier from the chosen parts and no weapon speed multiplier',
    'child count: the NumberOfChildProjectiles total is rounded to the nearest integer',
    'element: from the activated Is* slot; the explosion damage type is read as written',
    'status: StatusEffectDamage is evaluated only when its initialization resolves (price_init rules)',
]


def local_path(path, repo=ROOT):
    """The path resolved (symlinks followed); refuses anything not strictly inside the repository's local/ folder."""
    resolved = Path(path).resolve()
    local = (Path(repo) / 'local').resolve()
    if local not in resolved.parents:
        raise SystemExit(f'refusing {path}: output must be inside {local} (ignored game-derived data)')
    return resolved


def short(path):
    """The last name of an object path, or None."""
    return path.rsplit('.', 1)[-1] if path else None


def choose_parts(package, mod, overrides):
    """{kind: part path}: the --part choice, else the first WeightedParts entry of the list (None when empty)."""
    chosen = {}
    for kind, field in PART_LISTS.items():
        list_path = mod.get(field)
        candidates = []
        if list_path and list_path in package.index:
            candidates = [entry['Part'] for entry in package.props(list_path).get('WeightedParts') or []]
        if kind in overrides:
            if overrides[kind] not in candidates:
                raise SystemExit(f'{overrides[kind]} is not in {list_path} ({kind})')
            chosen[kind] = overrides[kind]
        else:
            chosen[kind] = candidates[0] if candidates else None
    return chosen


def slot_totals(source, mod_path, parts, unresolved):
    """(slots, totals): the activated slots of the mod and each attribute's modifier sums.

    slots: {SlotName: {grade, attribute, modifier, amount}}. totals: {attribute: {modifier type: sum}}.
    An effect whose value is unresolved is skipped and its name added to `unresolved`.
    """
    grades, activated = {}, set()
    for part in parts:
        for upgrade in (source.package.props(part).get('AttributeSlotUpgrades') or []):
            name = upgrade['SlotName']
            grades[name] = grades.get(name, 0) + (upgrade.get('GradeIncrease') or 0)
            if upgrade.get('bActivateSlot'):
                activated.add(name)
    slots, totals = {}, {}
    for effect in source.package.props(mod_path).get('AttributeSlotEffects') or []:
        name = effect.get('SlotName')
        if name not in activated or effect.get('bExternalSlot'):
            continue
        base = weapon_firing.formula(source, effect.get('BaseModifierValue'), {}, unresolved)
        per = weapon_firing.formula(source, effect.get('PerGradeUpgrade'), {}, unresolved)
        if base is None or per is None:
            unresolved.add(f'slot {name}')
            continue
        grade = weapon_stats.slot_grade(1.0, grades.get(name, 0), effect)
        amount = weapon_stats.slot_value(grade, base, per)
        attribute, kind = effect['AttributeToModify'], effect['ModifierType']
        slots[name] = {'grade': grade, 'attribute': attribute, 'modifier': kind, 'amount': amount}
        bucket = totals.setdefault(attribute, {})
        bucket[kind] = bucket.get(kind, 0.0) + amount
    return slots, totals


def modified(start, bucket):
    """start with the bucket's PreAdd and PostAdd, and its Scale applied (weapon_stats.combine's split rule)."""
    scale = bucket.get('MT_Scale', 0.0)
    return weapon_stats.combine(start, {
        'MT_PreAdd': bucket.get('MT_PreAdd', 0.0), 'MT_PostAdd': bucket.get('MT_PostAdd', 0.0),
        'scale_up': max(scale, 0.0), 'scale_down': max(-scale, 0.0)})


def init_attributes(package, resolver, init, attributes, unresolved):
    """Fill `attributes` with every attribute name the initialization reads (formula terms and range limits)."""
    if not isinstance(init, dict):
        return
    name = init.get('BaseValueAttribute')
    if name and name not in attributes:
        value = resolver.attribute(name)
        if value is None:
            unresolved.add(name)
        else:
            attributes[name] = value
    definition = init.get('InitializationDefinition')
    owner = next((p for p in resolver.packages if definition and definition in p.index), None)
    if owner is not None:
        props = owner.props(definition)
        formula = props.get('ValueFormula') or {}
        for key in ('Multiplier', 'Level', 'Power', 'Offset'):
            init_attributes(package, resolver, formula.get(key), attributes, unresolved)
        clamp = props.get('RangeRestriction') or {}
        for key in ('MinValue', 'MaxValue'):
            init_attributes(package, resolver, clamp.get(key), attributes, unresolved)


def projectile_damage(source, projectile, level):
    """(value, reason): the projectile's Damage at ProjectileLevel = level, before the mod's Damage slot. The field may
    be inherited from the class default, and its attributes may live in either package."""
    package = source.package
    init = source.fields(projectile).get('Damage') or {}
    definition = init.get('InitializationDefinition')
    if not definition:
        return None, 'the projectile has no Damage initialization'
    resolver = skill_stats.Resolver([package, source.defaults])
    # An inherited definition may exist only in the class defaults' package.
    if definition not in package.index:
        package = source.defaults
    if definition not in package.index:
        return None, f'the Damage initialization {definition} is in neither package'
    attributes = {PROJECTILE_LEVEL: float(level)}
    unresolved = set()
    init_attributes(package, resolver, init, attributes, unresolved)
    if unresolved:
        return None, 'unresolved attribute(s): ' + ', '.join(sorted(unresolved))
    value = weapon_recipe.attribute_value(package, init, level, attributes)
    if value is None:
        return None, 'Damage initialization needs runtime state'
    return value, None


def reachable(sequence, links):
    """The behaviours a link list reaches, each once, nearest first."""
    found, queue, seen = [], [link['behavior'] for link in links], set()
    while queue:
        index = queue.pop(0)
        if index not in seen:
            seen.add(index)
            behavior = sequence['behaviors'][index]
            found.append(behavior)
            queue.extend(link['behavior'] for link in behavior['links'])
    return found


def of_class(behaviors, name):
    return [b for b in behaviors if str(b['class']).rsplit('.', 1)[-1] == name]


def timer_event(timer_id):
    """The event a Behavior_ModifyTimer's timer fires when it runs out. Cooked data omits the id of the first timer. Read from
    the pairing in the packages (omitted -> Timer1, BTIMER_2 -> Timer2, BTIMER_3 -> Timer3); not confirmed in the running game."""
    return f"TIMEREVENT_Timer{(timer_id or 'BTIMER_1').rsplit('_', 1)[-1]}Elapsed"


def contact_rules(source, sequences, known, unresolved):
    """What the projectile's behaviour graph does, by event: the elasticity of a bounce off the world and off a damagable
    object, the timer the first contact starts, the timer the spawn starts, whether a contact detonates, and whether its
    explosion has no falloff. None unless the graph has both bounces and two different timers, each of which reaches a
    Detonate through its own event: the host explodes at the earlier deadline of the two."""
    events = {}
    for sequence in sequences:
        for event in sequence['events']:
            if event['enabled']:
                events.setdefault(event['name'], []).extend(reachable(sequence, event['links']))

    def value(behaviors, name, field):
        found = of_class(behaviors, name)
        return weapon_firing.formula(source, found[0]['properties'].get(field), known, unresolved) if found else None

    world, target = events.get('OnHitTheWorld', []), events.get('OnHitDamagableObject', [])
    after_contact = value(world, 'Behavior_ModifyTimer', 'NewTimerDelay')
    after_spawn = value(events.get('OnSpawn', []), 'Behavior_ModifyTimer', 'NewTimerDelay')
    world_bounce = value(world, 'ProjectileBehavior_Bounce', 'ElasticityScaleFormula')
    target_bounce = value(target, 'ProjectileBehavior_Bounce', 'ElasticityScaleFormula')
    if None in (after_contact, after_spawn, world_bounce, target_bounce):
        return None

    def timer_id(behaviors):
        return of_class(behaviors, 'Behavior_ModifyTimer')[0]['properties'].get('TimerId')

    if value(target, 'Behavior_ModifyTimer', 'NewTimerDelay') != after_contact or timer_id(target) != timer_id(world):
        return None   # the two contacts start different timers: not the shape the host models
    contact_timer, spawn_timer = timer_id(world), timer_id(events['OnSpawn'])
    if timer_event(contact_timer) == timer_event(spawn_timer):
        return None   # one timer set twice: a contact would replace the spawn's deadline, not race it
    if not all(of_class(events.get(timer_event(timer), []), 'ProjectileBehavior_Detonate') for timer in (contact_timer, spawn_timer)):
        return None
    explode = of_class(events.get('OnExplode', []), 'Behavior_Explode')
    return {
        'world_elasticity': world_bounce,
        'target_elasticity': target_bounce,
        'fuse_after_contact': after_contact,
        'fuse_after_spawn': after_spawn,
        'contact_detonates': bool(of_class(world + target, 'ProjectileBehavior_Detonate')),
        'full_damage': bool(explode and explode[0]['properties'].get('bDisableRadiusDamageFalloff')),
    }


def resolve(package, defaults, overrides, level, mod_path=GRENADE_MOD):
    """The grenade block for the mod (mod_path) with the chosen parts."""
    source = weapon_firing.Source(package, defaults)
    unresolved = set()
    mod = package.props(mod_path)
    parts = choose_parts(package, mod, overrides)
    chosen = [part for part in parts.values() if part]
    slots, totals = slot_totals(source, mod_path, chosen, unresolved)
    projectile_path = mod.get('DefaultProjectileDefinition')
    known = {}
    projectile = weapon_firing.projectile_of(source, None, {}, projectile_path, None, None, known, unresolved)
    pd = source.fields(projectile_path)   # with what the class default fills in (gravity, lifespan, behaviours)
    explosion = (projectile.get('splash') or {}).get('explosion')
    explode_radius = (projectile.get('splash') or {}).get('radius')

    fuse = modified(0.0, totals.get(FUSE_ATTRIBUTE, {}))
    timers = []
    provider = pd.get('BehaviorProviderDefinition')
    for path in weapon_firing.behaviours(source, provider).get('Behavior_ModifyTimer', []):
        props = package.props(path)
        timers.append({'behaviour': short(path), 'timer': props.get('TimerId'),
                       'delay': weapon_firing.formula(source, props.get('NewTimerDelay'), known, unresolved)})
    sequences = read_provider(package, provider)['sequences'] if provider else []
    contact = contact_rules(source, sequences, known, unresolved)
    speed_setters = []
    for part in chosen:
        part_provider = package.props(part).get('BehaviorProviderDefinition')
        for path in weapon_firing.behaviours(source, part_provider).get('ProjectileBehavior_SetSpeed', []):
            speed_setters.append({'part': short(part), 'behaviour': short(path)})

    value, reason = projectile_damage(source, projectile_path, level)
    multiplier = modified(1.0, totals.get(DAMAGE_ATTRIBUTE, {}))
    damage = {'value': None if value is None else value * multiplier, 'reason': reason,
              'projectile_initialization': (pd.get('Damage') or {}).get('InitializationDefinition'),
              'damage_multiplier': multiplier, 'slot': slots.get('Damage')}
    if reason is None and damage['value'] is None:
        damage['reason'] = 'not resolvable'

    radius = None
    if explode_radius is not None:
        radius = modified(explode_radius, totals.get(RADIUS_ATTRIBUTE, {}))

    children = modified(0.0, totals.get(CHILDREN_ATTRIBUTE, {}))
    elements = [ELEMENT_SLOTS[name] for name in slots if name in ELEMENT_SLOTS]
    status_value, status_reason = None, None
    if not parts.get('StatusDamage'):
        status_reason = 'no StatusDamage part chosen'
    elif STATUS_ATTRIBUTE in totals:
        status_value = modified(0.0, totals[STATUS_ATTRIBUTE])
    else:
        status_reason = 'no activated status slot'
    fx = weapon_projectile_fx.build(package, {'projectile': {
        'definition': projectile_path, 'splash': {'explosion': explosion, 'radius': radius}}})
    damage_type = source.fields(explosion).get('DamageTypeDef') if explosion else None

    return {
        'definition': mod_path,
        'level': level,
        'parts': dict(parts),
        'payload': short(parts.get('Payload')),
        'delivery': short(parts.get('Delivery')),
        'projectile': {
            'definition': projectile_path,
            'speed': projectile.get('speed'),
            'speed_formula': 'SpeedFormula (no part speed modifier applied)',
            'speed_setters_not_applied': speed_setters,
            'gravity_scale': pd.get('GravityScaling'),
            'physics_mode': pd.get('PhysicsMode'),
            'max_lifespan': pd.get('MaxLifespan'),
            'upward_velocity_bonus': pd.get('UpwardVelocityBonus'),
            'explode_on_impact': contact['contact_detonates'] if contact else projectile.get('explode_on_impact'),
            'contact': contact,
        },
        'fuse_time': {
            'value': fuse,
            'source': 'FuseTime slot total (GrenadeModDefinition.AttributeSlotEffects, grade from the Trigger part)',
            'slot': slots.get('FuseTime'),
            'projectile_timers': timers,
        },
        'damage': damage,
        'radius': {
            'value': radius,
            'source': 'Behavior_Explode.DamageRadiusFormula + BlastRadius slot total',
            'explode_radius': explode_radius,
            'slot': slots.get('DamageRadius'),
            'reason': None if radius is not None else 'Behavior_Explode has no DamageRadiusFormula value',
        },
        'damage_type': damage_type,
        'element': elements[0] if elements else None,
        'status': {
            'part': short(parts.get('StatusDamage')),
            'effect_damage': status_value,
            'reason': status_reason,
        },
        'child_count': None if children is None else int(round(children)),
        'explosion': {'definition': explosion, 'radius': radius,
                      'bands': fx['explosion']['bands'] if fx.get('explosion') else []},
        'fx': fx,
        'unresolved': sorted(unresolved),
        'unverified': list(ALWAYS_UNVERIFIED),
    }


def write_block(out, block):
    """Writes the block for the host, or stops first: a block the host refuses must not replace a good one."""
    if not (block['damage']['value'] or 0) > 0:
        raise SystemExit(f"no positive damage: {block['damage']['reason']}")
    if not block['projectile']['contact']:
        raise SystemExit('the projectile behaviour graph has no contact bounce and timers: the host does not load this grenade')
    out.write_text(json.dumps({'grenade': block}, indent=1), encoding='utf-8')


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', default=str(ROOT / 'build/Release/ow-package.exe'))
    parser.add_argument('--game', type=Path, required=True, help='Borderlands 2 install folder')
    parser.add_argument('--level', type=int, default=8, help='ProjectileLevel for the damage (default 8)')
    parser.add_argument('--part', action='append', default=[], metavar='KIND=PART',
                        help='choose a part, e.g. Trigger=GD_GrenadeMods.Trigger.Trigger_Grade5 (repeatable)')
    parser.add_argument('--id', help='output name (default grenade_mod_standard_l<level>)')
    parser.add_argument('--out', type=Path, help='output file, inside local/ (default local/items/grenades/<id>.json)')
    args = parser.parse_args()
    overrides = {}
    for item in args.part:
        kind, _, part = item.partition('=')
        if kind not in PART_LISTS or not part:
            raise SystemExit(f'--part wants KIND=PART with KIND one of {", ".join(PART_LISTS)}')
        overrides[kind] = part
    name = args.id or f'grenade_mod_standard_l{args.level}'
    out = local_path(args.out or ROOT / 'local' / 'items' / 'grenades' / f'{name}.json')   # refused before any read
    scratch = out.parent
    scratch.mkdir(parents=True, exist_ok=True)
    handle, schema_name = tempfile.mkstemp(suffix='.schema', dir=scratch)
    os.close(handle)
    schema = Path(schema_name)
    try:
        schema.write_text('\n'.join(SCHEMA_LINES) + '\n', encoding='utf-8')
        reader = str(Path(args.reader).resolve())
        game = args.game / 'WillowGame/CookedPCConsole'
        package = skill_stats.Package(reader, game / 'Startup.upk', str(schema.resolve()))
        defaults = skill_stats.Package(reader, game / 'WillowGame.upk', str(schema.resolve()))
        block = resolve(package, defaults, overrides, args.level)
    finally:
        schema.unlink(missing_ok=True)
    write_block(out, block)
    print(f"wrote {out}")
    print(f"fuse {block['fuse_time']['value']}, damage {block['damage']['value']}, radius {block['radius']['value']}, "
          f"speed {block['projectile']['speed']}, gravity {block['projectile']['gravity_scale']}, "
          f"contact {block['projectile']['contact']}")


if __name__ == '__main__':
    main()
