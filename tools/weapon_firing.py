"""Resolve what a weapon's shot does from its parts and firing mode.

Input is a tools/weapon_recipe.py recipe, ideally with tools/weapon_stats.py `stats`. Without
`stats` they are computed here (without localization). The result is written as recipe['firing']
in place, like weapon_stats.main, and printed with --print. Every value is decoded from the
package; class defaults (Default__<Class> in WillowGame.upk, next to --package) fill the fields a
cooked object omits. Rules are UNVERIFIED in game until a running-game check confirms them; each
output lists them in `unverified`, and `sources` names the object each value came from.

- Firing mode: the barrel's CustomFiringModeDefinition, else the weapon type's
  DefaultFiringModeDefinition (weapon_stats' precedence). `firing_mode_source` names the part that
  won. Custom firing modes on other parts are listed in `other_custom_firing_modes` and not used.
- Kind (kind_of()): "projectile" when the firing mode spawns a ProjectileDefinition (its
  FireType is EWWFT_Rocket in all 29 such modes), or when the mode flies its own bullet: it sets
  AirFriction or Acceleration (11 modes in Startup.upk, including the Torgue gyrojets, whose
  Speed is 260 to 1000). Hitscan modes set neither. Beam modes (EWWFT_Beam) are their own kind,
  "beam": the native path adds one weapon beam per shot (NATIVE_WEAPON_FIRING 1.2); the host does
  not support beams (listed in `unsupported`).
  Speed does not decide it: hitscan modes run from 3200 to 45000.
- Bullet speed = FiringModeDefinition.Speed * WeaponProjectileSpeedMultiplier. A projectile's
  speed is its ProjectileDefinition SpeedFormula instead (the firing mode's Speed is then not
  used; UNVERIFIED which one the game moves a rocket with).
- Projectile behaviours are the children of the definition's BehaviorProviderDefinition (for a
  bullet, of the firing mode itself). Bounce, Detonate, SetSpeed, homing, Behavior_Explode (splash)
  and Behavior_SpawnProjectile are read by class name. Formulas are evaluated as the game's
  evaluator does (weapon_stats.price_init); a formula that needs runtime state stays null.
- Splash: the first Behavior_Explode of the chain. Its falloff is on unless
  bDisableRadiusDamageFalloff; no falloff exponent or curve is decoded (null).
- Status: weapon_stats' status rows from the card (chance, duration, DOT damage). Tick interval
  and splash fields come from StatusEffectDefinition.BaseSpread* (no tick field is decoded).
- Pellets, spread, burst, fire interval and additional ricochets are the card attributes.
  Spread is stored in the attribute's own unit (degrees is UNVERIFIED).
- reload_throw: a weapon type whose BehaviorProviderDefinition spawns a projectile (Tediore).
- FiringPatternLines: decoded with the struct name FiringPatternLine, a guess that the reader's
  size check accepts (UNVERIFIED name).
"""
import argparse
import json
from pathlib import Path

import skill_stats
import weapon_stats
from weapon_stats import ATTR

SCHEMA_LINES = weapon_stats.SCHEMA_LINES + [
    'FiringPatternLines=StructProperty:FiringPatternLine',
]
# Weapon-type fields that describe the kick and the accuracy impulse (raw values, not evaluated).
RECOIL_FIELDS = {
    'kick_speed': 'WeaponKickSpeed', 'kick_recovery_time': 'WeaponKickRecoveryTime',
    'kick_left': 'WeaponKickLeft', 'per_shot_accuracy_impulse': 'PerShotAccuracyImpulse',
    'alternative_kick_enabled': 'bAlternativeKickEnabled', 'alternative_kick': 'AlternativeWeaponKick',
    'remove_base_accuracy_from_recoil_scale': 'bRemoveBaseAccuracyFromRecoilScale',
}
DAMAGE_TYPE_EXPLOSIVE = 'DAMAGE_TYPE_Explosive'
ALWAYS_UNVERIFIED = [
    'firing mode precedence (barrel CustomFiringModeDefinition, else type DefaultFiringModeDefinition) is weapon_stats\' rule',
    'kind: ProjectileDefinition or own motion (AirFriction/Acceleration) means a projectile; otherwise a trace',
    'bullet_speed = FiringModeDefinition.Speed * WeaponProjectileSpeedMultiplier (product as in weapon_stats)',
    'a ProjectileDefinition moves at its SpeedFormula; the firing mode Speed is not used for it',
    'spread is WeaponSpread in its stored unit (degrees is UNVERIFIED)',
    'bounces = count of ProjectileBehavior_Bounce; each one bounces with the ElasticityScaleFormula loss',
    'explode_on_impact = a ProjectileBehavior_Detonate in the chain',
    'splash falloff: only bDisableRadiusDamageFalloff is decoded; no exponent or curve field exists in the data',
    'status tick interval: no field found in StatusEffectDefinition (null)',
    'FiringPatternLines struct name FiringPatternLine (guess accepted by the reader size check)',
    'burst count, burst interval and additional ricochets come from weapon_stats attributes; their bases are not in TYPE_BASES',
]


class Source:
    """A cooked package plus WillowGame.upk (class defaults): the reads every rule needs."""

    def __init__(self, package, defaults):
        self.package, self.defaults = package, defaults

    def default_path(self, path):
        name = 'Default__' + self.package.classes.get(path, '').rsplit('.', 1)[-1]
        return name if name in self.defaults.index else None

    def fields(self, path):
        """Cooked properties of `path`, each missing one filled from its class default."""
        if not path or path not in self.package.index:
            return {}
        values = dict(self.package.props(path))
        default = self.default_path(path)
        for key, value in (self.defaults.props(default) if default else {}).items():
            if values.get(key) is None:
                values[key] = value
        return values

    def origin(self, path, key):
        """The object a key's value comes from: the cooked object, its class default, or None."""
        if not path or path not in self.package.index:
            return None
        if self.package.props(path).get(key) is not None:
            return path
        default = self.default_path(path)
        return default if default and self.defaults.props(default).get(key) is not None else None


def children(source, owner):
    """Export paths nested under `owner` (its Behavior_* and ProjectileBehavior_* sub-objects)."""
    prefix = owner + '.'
    found = sorted(p for p in source.package.index if p.startswith(prefix))
    preload = getattr(source.package, 'preload', None)
    if preload:
        preload(found)
    return found


def behaviours(source, owner):
    """{class name: [paths]} for the sub-objects of `owner` (empty when there is no owner)."""
    found = {}
    for path in (children(source, owner) if owner else []):
        found.setdefault(source.package.classes.get(path, '').rsplit('.', 1)[-1], []).append(path)
    return found


def formula(source, init, known, unresolved):
    """An AttributeInitializationData value (weapon_stats.price_init), or None when it is absent or unresolved."""
    if not init:
        return None
    return weapon_stats.price_init(source.package, init, known, unresolved)


def whole(value):
    """An integer attribute as an int (the stack holds floats); None stays None."""
    return None if value is None else int(value)


def kind_of(fm):
    """(kind, rule) from a firing mode's own fields (see the module docstring)."""
    if fm.get('FireType') == 'EWWFT_Beam':
        return 'beam', 'FireType EWWFT_Beam'
    if fm.get('ProjectileDefinition'):
        return 'projectile', 'ProjectileDefinition'
    if fm.get('AirFriction') is not None or fm.get('Acceleration') is not None:
        return 'projectile', 'own motion (AirFriction or Acceleration)'
    return 'instant', 'trace'


def firing_mode_of(source, recipe, weapon_type):
    """(firing mode path, the part it came from, {slot: custom firing mode} for other parts)."""
    parts = recipe.get('parts') or {}
    others = {}
    for slot, choice in parts.items():
        custom = source.fields(choice.get('part')).get('CustomFiringModeDefinition')
        if custom and slot != 'Barrel':
            others[slot] = custom
    barrel = source.fields((parts.get('Barrel') or {}).get('part')).get('CustomFiringModeDefinition')
    if barrel:
        return barrel, 'Barrel', others
    return weapon_type.get('DefaultFiringModeDefinition'), 'WeaponType', others


def falloff_bands(source, damage_type):
    fields = source.fields(damage_type) if damage_type else {}
    return {name: fields.get(name) for name in ('MaxDamageRadius', 'MinDamageRadius', 'MinDamagePercent')}


def splash_of(source, path, definition, fm, fallback_type, known, unresolved):
    """Splash fields from a Behavior_Explode sub-object (None when there is none)."""
    if not path:
        return None
    explosion = source.fields(path)
    damage = explosion.get('DamageFormula') or {}
    return {
        'behaviour': path,
        'explosion': explosion.get('Definition'),
        'radius': formula(source, explosion.get('DamageRadiusFormula'), known, unresolved),
        'damage': formula(source, damage, known, unresolved),
        'damage_attribute': damage.get('BaseValueAttribute'),
        'damage_scale': damage.get('BaseValueScaleConstant'),
        'momentum': formula(source, explosion.get('MomentumFormula'), known, unresolved),
        'falloff': not explosion.get('bDisableRadiusDamageFalloff'),
        'falloff_exponent': None,
        # WillowDamageTypeDefinition.CalcRadiusDamageScale inputs (NATIVE_WEAPON_FIRING 3.1); class defaults 0.2/0.8/0.2.
        'falloff_bands': falloff_bands(source, (definition or {}).get('DefaultDamageTypeDefinition')
                                       or fm.get('DamageTypeDefinition') or fallback_type),
        'damage_type': (definition or {}).get('DefaultDamageTypeDefinition') or fm.get('DamageTypeDefinition')
                       or fallback_type,
        'damage_source': explosion.get('DamageSource'),
        'status_effect_damage': formula(source, explosion.get('StatusEffectDamage'), known, unresolved),
        'status_effect_chance': formula(source, explosion.get('StatusEffectChance'), known, unresolved),
        'instigator_self_damage_scale': explosion.get('InstigatorSelfDamageScale'),
    }


def projectile_of(source, fm_path, fm, definition, bullet_speed, fallback_type, known, unresolved, speed_multiplier=1.0):
    """The projectile section: a ProjectileDefinition, or the firing mode's own bullet (definition None)."""
    pd = source.fields(definition)
    owner = pd.get('BehaviorProviderDefinition') if definition else fm_path
    walk = behaviours(source, owner)
    explosions = walk.get('Behavior_Explode', [])
    homing = walk.get('ProjectileBehavior_FindHomingTarget', []) + walk.get('ProjectileBehavior_SetHomingTarget', [])
    detonate = walk.get('ProjectileBehavior_Detonate', [])
    bounces = walk.get('ProjectileBehavior_Bounce', [])
    splash = splash_of(source, explosions[0] if explosions else None, pd, fm, fallback_type, known, unresolved)
    if splash:
        splash['explosion_count'] = len(explosions)
    speed = formula(source, pd.get('SpeedFormula'), known, unresolved) if definition else bullet_speed
    # MaxSpeed = SpeedFormula x the weapon's ProjectileSpeedMultiplier (NATIVE_WEAPON_FIRING 2.1); confirmed live
    # 2026-10-23 (Bandit and Maliwan rockets 3000 x 2.6 = 7800 uu/s). bullet_speed already carries the multiplier.
    if definition and speed is not None:
        speed = speed * speed_multiplier
    raw_behaviours = {path: source.package.props(path) for names in walk.values() for path in names}
    return {
        'definition': definition,
        'speed': speed,
        # No max-speed field exists in ProjectileDefinition or its behaviours (key census of Startup.upk).
        'max_speed': None,
        'acceleration': fm.get('Acceleration'),
        'air_friction': fm.get('AirFriction'),
        'gravity_scale': pd.get('GravityScaling') if definition else None,
        'lifetime': fm.get('Lifetime'),
        'max_lifespan': pd.get('MaxLifespan') if definition else None,
        'fuse_time': pd.get('FuseTime') if definition else None,
        'bounces': {
            'count': len(bounces),
            'elasticity': [formula(source, source.fields(p).get('ElasticityScaleFormula'), known, unresolved)
                           for p in bounces],
        },
        'ricochets': {'count': fm.get('NumRicochets'), 'friction': fm.get('RicochetFriction'),
                      'response': fm.get('RicochetResponse')},
        'homing': None if not homing else {
            'behaviours': [source.fields(p) for p in homing],
            'search_radius': pd.get('SearchRadius'),
            'turning_radius': pd.get('TurningRadius'),
        },
        'explode_on_impact': bool(detonate),
        'detonate': [{'behaviour': p, 'target_allegiance': source.fields(p).get('TargetAllegiance')}
                     for p in detonate],
        'set_speed': [{'behaviour': p, 'speed_multiplier': source.fields(p).get('SpeedMultiplier'),
                       'new_speed': source.fields(p).get('NewSpeed')}
                      for p in walk.get('ProjectileBehavior_SetSpeed', [])],
        'spawns': [{'behaviour': p, 'definition': source.fields(p).get('ProjectileDefinition')}
                   for p in walk.get('Behavior_SpawnProjectile', [])],
        'splash': splash,
        'raw': {'definition': source.package.props(definition) if definition else None,
                'behaviours': raw_behaviours},
    }


def status_of(source, card, known, unresolved):
    """The status block from the card's status rows (weapon_stats.status_rows) and the status effect."""
    definition = card.get('status_effect_definition')
    if not definition:
        return None
    effect = source.fields(definition)
    return {
        'definition': definition,
        'type': card.get('status_effect'),
        'chance': card.get('status_chance'),
        'chance_by_surface': card.get('status_chance_by_surface'),
        'duration': card.get('status_duration'),
        'dps': card.get('status_dps'),
        'damage_over_time': bool(effect.get('bDoesDamageOverTime')),
        # No tick-interval field in StatusEffectDefinition (census of its keys).
        'tick_interval': None,
        'spread': {
            'time_interval': formula(source, effect.get('BaseSpreadTimeInterval'), known, unresolved),
            'distance': formula(source, effect.get('BaseSpreadDistanceFromSource'), known, unresolved),
            'cap': formula(source, effect.get('BaseSpreadCap'), known, unresolved),
        },
        'on_application': effect.get('OnApplication'),
        'on_duration_end': effect.get('OnDurationEnd'),
    }


def reload_throw(source, weapon_type, fallback_type, known, unresolved):
    """The Tediore throw: the first Behavior_SpawnProjectile under the weapon type, else None."""
    provider = weapon_type.get('BehaviorProviderDefinition')
    spawns = behaviours(source, provider).get('Behavior_SpawnProjectile', [])
    if not spawns:
        return None
    definition = source.fields(spawns[0]).get('ProjectileDefinition')
    return {
        'provider': provider,
        'spawn_count': len(spawns),
        'definition': definition,
        'projectile': projectile_of(source, None, {}, definition, None, fallback_type, known, unresolved),
    }


def unsupported_of(kind, projectile):
    """Host gaps for this gun: beams, and child projectiles spawned by the projectile's behaviours."""
    gaps = []
    if kind == 'beam':
        gaps.append('beam (EWWFT_Beam): the host fires light bullets instead')
    if projectile and projectile.get('spawns'):
        gaps.append(f"child projectiles ({len(projectile['spawns'])} Behavior_SpawnProjectile): not spawned by the host")
    return gaps


def build(source, recipe, stats):
    """The recipe's firing section (see the module docstring)."""
    card, attributes = stats['card'], stats['attributes']
    level = card.get('level') or int(recipe.get('game_stage') or 1)
    unresolved = set()
    known = {ATTR + 'WeaponLevel': float(level)}
    if card.get('damage') is not None:
        known[ATTR + 'WeaponDamage'] = card['damage']
    weapon_type = source.fields(recipe['weapon_type'])
    fm_path, won_by, others = firing_mode_of(source, recipe, weapon_type)
    fm = source.fields(fm_path)
    kind, rule = kind_of(fm)
    multiplier = attributes.get(ATTR + 'WeaponProjectileSpeedMultiplier', 1.0)
    bullet_speed = fm['Speed'] * multiplier if fm.get('Speed') is not None else None
    damage_type = card.get('damage_type')
    attribute = {name: attributes.get(ATTR + name) for name in
                 ('WeaponProjectilesPerShot', 'WeaponSpread', 'WeaponAutomaticBurstCount', 'WeaponBurstInterval',
                  'WeaponFireInterval', 'WeaponAdditionalRicochets', 'WeaponShotCost')}
    # Base values the type declares that weapon_stats does not put into its attribute stack.
    burst_interval = attribute['WeaponBurstInterval']
    if burst_interval is None and weapon_type.get('BurstInterval') is not None:
        burst_interval = weapon_type['BurstInterval']
    burst_count = attribute['WeaponAutomaticBurstCount']
    if burst_count is None:
        burst_count = weapon_type.get('AutomaticBurstCount')
    barrel = source.fields((recipe.get('parts', {}).get('Barrel') or {}).get('part'))
    spin_down = (barrel.get('SpinDownDuration') or {}).get('BaseValueConstant') if barrel.get('bIsSpinningEnabled') else None
    sources = {
        'firing_mode': won_by,
        'bullet_speed': source.origin(fm_path, 'Speed'),
        'tracer_template': source.origin(fm_path, 'PartSysTemplate'),
        'lifetime': source.origin(fm_path, 'Lifetime'),
    }
    projectile = None
    if kind == 'projectile':
        definition = fm.get('ProjectileDefinition')
        projectile = projectile_of(source, fm_path, fm, definition, bullet_speed, damage_type, known, unresolved, multiplier)
        if definition:
            sources['projectile.speed'] = source.origin(definition, 'SpeedFormula')
            sources['projectile.gravity_scale'] = source.origin(definition, 'GravityScaling')
        if projectile['splash']:
            sources['splash'] = projectile['splash']['behaviour']
    status = status_of(source, card, known, unresolved)
    return {
        'schema': 1,
        'firing_mode': fm_path,
        'firing_mode_source': won_by,
        'other_custom_firing_modes': others,
        'kind': kind,
        'kind_rule': rule,
        'fire_type': fm.get('FireType'),
        'bullet_speed': bullet_speed,
        'tracer_template': fm.get('PartSysTemplate'),
        'conditional_tracer_template': fm.get('ConditionalPartSysTemplate'),
        'impact_definition': weapon_type.get('WeaponImpact'),
        'on_any_impact': fm.get('OnAnyImpact'),
        'explosion_override': fm.get('ExplosionOverrideDefinition'),
        'projectile': projectile,
        'pellets': whole(attribute['WeaponProjectilesPerShot']),
        'spread': attribute['WeaponSpread'],
        'spread_unit': 'WeaponSpread as stored (UNVERIFIED: degrees)',
        'burst_count': whole(burst_count),
        'burst_interval': burst_interval,
        'fire_interval': attribute['WeaponFireInterval'],
        'spin_up': card.get('spin_up'),
        # The barrel's SpinDownDuration constant (an attribute initialization; only the constant is read).
        'spin_down': spin_down,
        'trace_range': weapon_type.get('Range'),
        'recoil': {name: weapon_type.get(field) for name, field in RECOIL_FIELDS.items()},
        'additional_ricochets': attribute['WeaponAdditionalRicochets'],
        'shot_cost': attribute['WeaponShotCost'],
        'damage': card.get('damage'),
        'damage_type': damage_type,
        'element': card.get('element'),
        'explosive': source.package.props(damage_type).get('DamageType') == DAMAGE_TYPE_EXPLOSIVE
                     if damage_type and damage_type in source.package.index else False,
        'status': status,
        'reload_throw': reload_throw(source, weapon_type, damage_type, known, unresolved),
        'muzzle_flash_templates': weapon_type.get('MuzzleFlashPSTemplates'),
        'impact_effects': {'weapon_impact': weapon_type.get('WeaponImpact'),
                           'dropped_impact': weapon_type.get('DroppedImpact')},
        'pattern_lines': fm.get('FiringPatternLines'),
        'pattern_fire_randomly': fm.get('bFireRandomlyFromPattern'),
        'pattern_scale_by_spread': fm.get('bScalePatternByWeaponSpread'),
        'pattern_base_spread': fm.get('BasePatternSpread'),
        'pattern_min_scale': fm.get('MinPatternScale'),
        'pattern_reset_each_shot': fm.get('bResetPatternAfterEachShot'),
        'suppress_spread': fm.get('bSuppressWeaponSpread'),
        'sources': sources,
        # What the data asks for that the host does not do (it logs these when it loads the recipe).
        'unsupported': unsupported_of(kind, projectile),
        'unverified': ALWAYS_UNVERIFIED + ([f'unresolved at runtime: {", ".join(sorted(unresolved))}']
                                            if unresolved else []),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--reader', required=True, help='ow-package executable')
    parser.add_argument('--package', required=True, help='cooked package with the balances, e.g. Startup.upk')
    parser.add_argument('--defaults', help='WillowGame.upk for class defaults (default: next to --package)')
    parser.add_argument('--recipe', required=True, help='weapon_recipe.py output; firing is added to it')
    parser.add_argument('--print', action='store_true', help='print the firing section as JSON')
    args = parser.parse_args()
    recipe_path = Path(args.recipe)
    recipe = json.loads(recipe_path.read_text(encoding='utf-8'))
    schema = recipe_path.parent / 'weapon_firing.schema'
    schema.write_text('\n'.join(SCHEMA_LINES) + '\n', encoding='utf-8')
    reader, schema_path = str(Path(args.reader).resolve()), str(schema.resolve())
    package = skill_stats.Package(reader, Path(args.package), schema_path)
    defaults = Path(args.defaults) if args.defaults else Path(args.package).with_name('WillowGame.upk')
    source = Source(package, skill_stats.Package(reader, defaults, schema_path))
    level = int(recipe.get('game_stage') or 1)
    stats = recipe.get('stats') or weapon_stats.evaluate(package, recipe, level)
    firing = build(source, recipe, stats)
    recipe['firing'] = firing
    recipe_path.write_text(json.dumps(recipe, indent=1), encoding='utf-8')
    if args.print:
        print(json.dumps(firing, indent=1))
    else:
        print(f"{firing['kind']} {firing['firing_mode']} (from {firing['firing_mode_source']}); "
              f"bullet_speed {firing['bullet_speed']}")


if __name__ == '__main__':
    main()
