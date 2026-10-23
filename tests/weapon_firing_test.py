"""Synthetic tests for tools/weapon_firing.py: invented objects and numbers, no game data.

FakePackage stands in for skill_stats.Package (props(), index, classes). Paths and values are made up.
"""
from pathlib import Path
import sys
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import weapon_firing as firing
from weapon_stats import ATTR


def const(value, attribute=None, scale=1.0):
    return {'BaseValueConstant': value, 'BaseValueAttribute': attribute,
            'InitializationDefinition': None, 'BaseValueScaleConstant': scale}


class FakePackage:
    def __init__(self):
        self.objects, self.classes = {}, {}
        self.index = self.objects

    def add(self, path, cls, props=None):
        self.objects[path] = dict(props or {})
        self.classes[path] = cls
        self.index = self.objects
        return path

    def props(self, path):
        return self.objects[path]


def world():
    """Invented weapon: a type with a default firing mode, a barrel, a rocket and a Tediore throw."""
    package, defaults = FakePackage(), FakePackage()
    defaults.add('Default__FiringModeDefinition', 'WillowGame.FiringModeDefinition', {'Speed': 2000, 'Lifetime': 5})
    defaults.add('Default__ProjectileDefinition', 'WillowGame.ProjectileDefinition', {'GravityScaling': 1, 'MaxLifespan': 0})
    defaults.add('Default__StatusEffectDefinition', 'WillowGame.StatusEffectDefinition', {'BaseDuration': const(0)})
    add = package.add
    add('Fake.Types.WT_Fake', 'WillowGame.WeaponTypeDefinition', {
        'DefaultFiringModeDefinition': 'Fake.FiringModes.Bullet_Default', 'WeaponImpact': 'Fake.Impacts.Impact_A',
        'MuzzleFlashPSTemplates': ['Fake.Fx.Muzzle_A'], 'BurstInterval': 0.5, 'WeaponKickSpeed': 4,
        'BehaviorProviderDefinition': 'Fake.Types.WT_Fake.BehaviorProviderDefinition_0'})
    add('Fake.Barrel.Barrel_A', 'WillowGame.WeaponPartDefinition')
    add('Fake.Barrel.Barrel_Custom', 'WillowGame.WeaponPartDefinition',
        {'CustomFiringModeDefinition': 'Fake.FiringModes.Bullet_Custom'})
    add('Fake.Grip.Grip_Custom', 'WillowGame.WeaponPartDefinition',
        {'CustomFiringModeDefinition': 'Fake.FiringModes.Bullet_Custom'})
    add('Fake.FiringModes.Bullet_Default', 'WillowGame.FiringModeDefinition', {'PartSysTemplate': 'Fake.Fx.Tracer_A'})
    add('Fake.FiringModes.Bullet_Custom', 'WillowGame.FiringModeDefinition', {'Speed': 9000, 'AirFriction': -8})
    add('Fake.FiringModes.FM_Rocket', 'WillowGame.FiringModeDefinition', {
        'FireType': 'EWWFT_Rocket', 'ProjectileDefinition': 'Fake.Projectiles.Proj_Rocket', 'Lifetime': 3})
    add('Fake.FiringModes.FM_Beam', 'WillowGame.FiringModeDefinition', {'FireType': 'EWWFT_Beam', 'Speed': 500})
    add('Fake.Projectiles.Proj_Rocket', 'WillowGame.ProjectileDefinition', {
        'SpeedFormula': const(3000), 'GravityScaling': 0.1, 'MaxLifespan': 0,
        'DefaultDamageTypeDefinition': 'Fake.DamageTypes.Explosive',
        'BehaviorProviderDefinition': 'Fake.Projectiles.Proj_Rocket.BehaviorProviderDefinition_0'})
    provider = 'Fake.Projectiles.Proj_Rocket.BehaviorProviderDefinition_0'
    add(provider, 'GearboxFramework.BehaviorProviderDefinition')
    add(provider + '.ProjectileBehavior_Bounce_1', 'WillowGame.ProjectileBehavior_Bounce',
        {'ElasticityScaleFormula': const(0.5)})
    add(provider + '.ProjectileBehavior_Detonate_2', 'WillowGame.ProjectileBehavior_Detonate', {})
    add(provider + '.ProjectileBehavior_SetSpeed_3', 'WillowGame.ProjectileBehavior_SetSpeed', {'SpeedMultiplier': 0.5})
    add(provider + '.ProjectileBehavior_FindHomingTarget_4', 'WillowGame.ProjectileBehavior_FindHomingTarget',
        {'TargetSearchRange': 1000})
    add(provider + '.Behavior_Explode_5', 'WillowGame.Behavior_Explode', {
        'DamageRadiusFormula': const(250), 'MomentumFormula': const(10000), 'Definition': 'Fake.Explosions.Boom',
        'DamageFormula': const(0, 'D_Attributes.Projectile.ProjectileDamage', 1.5),
        'bDisableRadiusDamageFalloff': True})
    add('Fake.Types.WT_Fake.BehaviorProviderDefinition_0', 'GearboxFramework.BehaviorProviderDefinition')
    add('Fake.DamageTypes.Explosive', 'WillowGame.DamageTypeDefinition', {'DamageType': 'DAMAGE_TYPE_Explosive'})
    add('Fake.Status.Shock', 'WillowGame.StatusEffectDefinition', {
        'bDoesDamageOverTime': True, 'BaseSpreadTimeInterval': const(0.5),
        'BaseSpreadDistanceFromSource': const(100), 'BaseSpreadCap': const(3)})
    add('Fake.Projectiles.Proj_Throw', 'WillowGame.ProjectileDefinition', {
        'SpeedFormula': const(1200), 'BehaviorProviderDefinition': 'Fake.Projectiles.Proj_Throw.BehaviorProviderDefinition_0'})
    add('Fake.Projectiles.Proj_Throw.BehaviorProviderDefinition_0', 'GearboxFramework.BehaviorProviderDefinition')
    add('Fake.Projectiles.Proj_Throw.BehaviorProviderDefinition_0.Behavior_Explode_1', 'WillowGame.Behavior_Explode',
        {'DamageRadiusFormula': const(300)})
    return package, defaults


def source_of():
    package, defaults = world()
    return firing.Source(package, defaults)


def recipe(barrel='Fake.Barrel.Barrel_A', parts=None):
    chosen = {'Barrel': {'part': barrel}}
    chosen.update(parts or {})
    return {'weapon_type': 'Fake.Types.WT_Fake', 'parts': chosen, 'game_stage': 8}


def stats(damage=20.0, status=True, attributes=None):
    card = {'level': 8, 'damage': damage, 'damage_type': 'Fake.DamageTypes.Explosive', 'element': 'Explosive',
            'spin_up': None}
    if status:
        card.update({'status_effect': 'STATUS_EFFECT_Shock', 'status_effect_definition': 'Fake.Status.Shock',
                     'status_chance': 17.0, 'status_chance_by_surface': {'generic': 17.0},
                     'status_dps': 3.0, 'status_duration': 2.0})
    values = {ATTR + 'WeaponProjectileSpeedMultiplier': 1.5, ATTR + 'WeaponProjectilesPerShot': 8.0,
              ATTR + 'WeaponSpread': 4.0, ATTR + 'WeaponFireInterval': 0.25}
    values.update(attributes or {})
    return {'card': card, 'attributes': values}


def shot(rec=None, st=None):
    return firing.build(source_of(), rec or recipe(), st or stats())


class FiringModePrecedenceTests(unittest.TestCase):
    def test_barrel_custom_firing_mode_wins_over_type_default(self):
        out = shot(recipe(barrel='Fake.Barrel.Barrel_Custom'))
        self.assertEqual(out['firing_mode'], 'Fake.FiringModes.Bullet_Custom')
        self.assertEqual(out['firing_mode_source'], 'Barrel')
        # Speed 9000 times the stat WeaponProjectileSpeedMultiplier 1.5.
        self.assertAlmostEqual(out['bullet_speed'], 13500.0)

    def test_type_default_when_no_part_has_a_custom_mode(self):
        out = shot()
        self.assertEqual(out['firing_mode'], 'Fake.FiringModes.Bullet_Default')
        self.assertEqual(out['firing_mode_source'], 'WeaponType')
        # The cooked mode has no Speed, so the class default 2000 applies (WillowGame.upk).
        self.assertAlmostEqual(out['bullet_speed'], 3000.0)
        self.assertEqual(out['sources']['bullet_speed'], 'Default__FiringModeDefinition')

    def test_other_parts_custom_modes_are_listed_not_used(self):
        out = shot(recipe(parts={'Grip': {'part': 'Fake.Grip.Grip_Custom'}}))
        self.assertEqual(out['other_custom_firing_modes'], {'Grip': 'Fake.FiringModes.Bullet_Custom'})
        self.assertEqual(out['firing_mode'], 'Fake.FiringModes.Bullet_Default')


class KindTests(unittest.TestCase):
    def test_projectile_definition_is_a_projectile(self):
        self.assertEqual(firing.kind_of({'ProjectileDefinition': 'x'}), ('projectile', 'ProjectileDefinition'))

    def test_own_motion_is_a_projectile(self):
        self.assertEqual(firing.kind_of({'Speed': 900, 'AirFriction': -8})[0], 'projectile')
        self.assertEqual(firing.kind_of({'Acceleration': {'Z': -1300}})[0], 'projectile')

    def test_plain_speed_is_a_trace(self):
        self.assertEqual(firing.kind_of({'Speed': 20000}), ('instant', 'trace'))

    def test_beam_is_its_own_unsupported_kind(self):
        self.assertEqual(firing.kind_of({'FireType': 'EWWFT_Beam', 'Speed': 500})[0], 'beam')
        self.assertTrue(any('beam' in gap for gap in firing.unsupported_of('beam', None)))

    def test_child_projectiles_are_listed_unsupported(self):
        gaps = firing.unsupported_of('projectile', {'spawns': [{'behaviour': 'Fake.B', 'definition': 'Fake.P'}]})
        self.assertEqual(len(gaps), 1)
        self.assertIn('child projectiles', gaps[0])
        self.assertEqual(firing.unsupported_of('projectile', {'spawns': []}), [])

    def test_shot_reports_projectile_only_for_projectile_kinds(self):
        self.assertIsNone(shot()['projectile'])
        rocket = shot(recipe(barrel='Fake.Barrel.Barrel_A'))  # type default is the trace mode
        self.assertEqual(rocket['kind'], 'instant')


class ProjectileWalkTests(unittest.TestCase):
    def rocket(self):
        package, defaults = world()
        package.objects['Fake.Types.WT_Fake'] = dict(package.objects['Fake.Types.WT_Fake'],
                                                     DefaultFiringModeDefinition='Fake.FiringModes.FM_Rocket')
        source = firing.Source(package, defaults)
        return firing.build(source, recipe(), stats())

    def test_speed_gravity_and_lifetime_from_the_definition(self):
        out = self.rocket()
        self.assertEqual(out['kind'], 'projectile')
        proj = out['projectile']
        self.assertEqual(proj['definition'], 'Fake.Projectiles.Proj_Rocket')
        # SpeedFormula 3000 x the stats' WeaponProjectileSpeedMultiplier 1.5.
        self.assertEqual(proj['speed'], 4500.0)
        self.assertAlmostEqual(proj['gravity_scale'], 0.1)
        self.assertEqual(proj['lifetime'], 3)
        self.assertIsNone(proj['max_speed'])

    def test_bounce_detonate_setspeed_and_homing_are_read_by_class(self):
        proj = self.rocket()['projectile']
        self.assertEqual(proj['bounces']['count'], 1)
        self.assertEqual(proj['bounces']['elasticity'], [0.5])
        self.assertTrue(proj['explode_on_impact'])
        self.assertEqual(proj['set_speed'][0]['speed_multiplier'], 0.5)
        self.assertEqual(proj['homing']['behaviours'][0]['TargetSearchRange'], 1000)

    def test_splash_radius_falloff_and_damage_type(self):
        splash = self.rocket()['projectile']['splash']
        self.assertEqual(splash['radius'], 250.0)
        self.assertFalse(splash['falloff'])
        self.assertIsNone(splash['falloff_exponent'])
        self.assertEqual(splash['damage_type'], 'Fake.DamageTypes.Explosive')
        self.assertEqual(splash['explosion_count'], 1)

    def test_unresolved_splash_damage_is_null_and_named(self):
        out = self.rocket()
        splash = out['projectile']['splash']
        self.assertIsNone(splash['damage'])
        self.assertEqual(splash['damage_attribute'], 'D_Attributes.Projectile.ProjectileDamage')
        self.assertTrue(any('ProjectileDamage' in rule for rule in out['unverified']))

    def test_raw_keeps_the_definition_and_behaviours(self):
        raw = self.rocket()['projectile']['raw']
        self.assertEqual(raw['definition']['SpeedFormula']['BaseValueConstant'], 3000)
        self.assertEqual(len(raw['behaviours']), 5)


class StatusAndThrowTests(unittest.TestCase):
    def test_status_block_from_the_card_and_effect(self):
        status = shot()['status']
        self.assertEqual(status['definition'], 'Fake.Status.Shock')
        self.assertEqual(status['chance'], 17.0)
        self.assertEqual(status['dps'], 3.0)
        self.assertEqual(status['duration'], 2.0)
        self.assertTrue(status['damage_over_time'])
        self.assertIsNone(status['tick_interval'])
        self.assertEqual(status['spread'], {'time_interval': 0.5, 'distance': 100.0, 'cap': 3.0})

    def test_no_status_is_null(self):
        self.assertIsNone(shot(st=stats(status=False))['status'])

    def test_tediore_throw_under_the_weapon_type(self):
        package, defaults = world()
        package.add('Fake.Types.WT_Fake.BehaviorProviderDefinition_0.Behavior_SpawnProjectile_7',
                    'WillowGame.Behavior_SpawnProjectile', {'ProjectileDefinition': 'Fake.Projectiles.Proj_Throw'})
        out = firing.build(firing.Source(package, defaults), recipe(), stats())
        throw = out['reload_throw']
        self.assertEqual(throw['definition'], 'Fake.Projectiles.Proj_Throw')
        self.assertEqual(throw['spawn_count'], 1)
        self.assertEqual(throw['projectile']['speed'], 1200.0)
        self.assertEqual(throw['projectile']['splash']['radius'], 300.0)

    def test_no_spawn_chain_means_no_throw(self):
        self.assertIsNone(shot()['reload_throw'])


class NullHandlingTests(unittest.TestCase):
    def test_absent_attributes_are_null_not_zero(self):
        out = shot(st=stats(attributes={ATTR + 'WeaponSpread': None}))
        self.assertIsNone(out['spread'])
        self.assertIsNone(out['additional_ricochets'])
        self.assertIsNone(out['burst_count'])

    def test_burst_interval_falls_back_to_the_type_field_and_says_so(self):
        out = shot()
        self.assertEqual(out['burst_interval'], 0.5)
        self.assertTrue(any('burst' in rule for rule in out['unverified']))

    def test_integer_attributes_are_whole_numbers(self):
        self.assertEqual(shot()['pellets'], 8)
        self.assertIsInstance(shot()['pellets'], int)
        self.assertIsNone(firing.whole(None))

    def test_defaults_come_from_the_class_default_when_cooked_omits_them(self):
        source = source_of()
        self.assertEqual(source.fields('Fake.FiringModes.Bullet_Default')['Speed'], 2000)
        self.assertEqual(source.origin('Fake.FiringModes.Bullet_Default', 'Speed'), 'Default__FiringModeDefinition')
        self.assertIsNone(source.origin('Fake.FiringModes.Bullet_Default', 'AirFriction'))


if __name__ == '__main__':
    unittest.main()
