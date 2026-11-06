"""Synthetic tests for tools/grenade_mod.py: invented objects and numbers, no game data.

FakePackage stands in for skill_stats.Package (props(), index, classes). Paths and values are made up.
"""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import grenade_mod as grenade
import weapon_firing

ROOT = Path(__file__).resolve().parents[1]
ATTR = grenade.ATTR
MOD = 'Fake.Mod.Grenade'


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
        return path

    def props(self, path):
        return self.objects[path]


def part_list(package, path, parts):
    package.add(path, 'WillowGame.ItemPartListDefinition',
                {'WeightedParts': [{'Part': part} for part in parts]})
    return path


def world():
    """Invented grenade mod: trigger (fuse), damage (scale), radius (PreAdd), one projectile with a timer."""
    package = FakePackage()
    add = package.add
    add('Fake.Trig.G0', 'WillowGame.GrenadeModPartDefinition', {
        'AttributeSlotUpgrades': [{'SlotName': 'FuseTime', 'GradeIncrease': -1, 'bActivateSlot': True}]})
    add('Fake.Trig.G2', 'WillowGame.GrenadeModPartDefinition', {
        'AttributeSlotUpgrades': [{'SlotName': 'FuseTime', 'GradeIncrease': 2, 'bActivateSlot': True}]})
    add('Fake.Dmg.D1', 'WillowGame.GrenadeModPartDefinition', {
        'AttributeSlotUpgrades': [{'SlotName': 'Damage', 'GradeIncrease': 1, 'bActivateSlot': True}]})
    add('Fake.Rad.R1', 'WillowGame.GrenadeModPartDefinition', {
        'AttributeSlotUpgrades': [{'SlotName': 'DamageRadius', 'GradeIncrease': 1, 'bActivateSlot': True}]})
    lists = {
        'AlphaParts': part_list(package, 'Fake.Lists.Payload', ['Fake.Payload.Standard', 'Fake.Payload.Mirv']),
        'BetaParts': part_list(package, 'Fake.Lists.Delivery', ['Fake.Delivery.Lob']),
        'GammaParts': part_list(package, 'Fake.Lists.Trigger', ['Fake.Trig.G0', 'Fake.Trig.G2']),
        'DeltaParts': part_list(package, 'Fake.Lists.Accessory', ['Fake.Acc.None']),
        'EpsilonParts': part_list(package, 'Fake.Lists.Damage', ['Fake.Dmg.D1', 'Fake.Dmg.D0']),
        'ZetaParts': part_list(package, 'Fake.Lists.Radius', ['Fake.Rad.R1', 'Fake.Rad.R0']),
        'EtaParts': part_list(package, 'Fake.Lists.Child', ['Fake.Child.C0']),
        'ThetaParts': part_list(package, 'Fake.Lists.Status', ['Fake.Status.S0']),
        'MaterialParts': part_list(package, 'Fake.Lists.Material', ['Fake.Material.M0']),
    }
    add('Fake.Mod.Grenade', 'WillowGame.GrenadeModDefinition', dict(lists, **{
        'DefaultProjectileDefinition': 'Fake.Proj.P',
        'AttributeSlotEffects': [
            {'SlotName': 'FuseTime', 'AttributeToModify': ATTR + 'FuseTime', 'ModifierType': 'MT_PreAdd',
             'BaseModifierValue': const(0.5), 'PerGradeUpgrade': const(0.25), 'bExternalSlot': False},
            {'SlotName': 'Damage', 'AttributeToModify': ATTR + 'GrenadeDamage', 'ModifierType': 'MT_Scale',
             'BaseModifierValue': const(0), 'PerGradeUpgrade': const(0.1), 'bExternalSlot': False},
            {'SlotName': 'DamageRadius', 'AttributeToModify': ATTR + 'BlastRadius', 'ModifierType': 'MT_PreAdd',
             'BaseModifierValue': const(0), 'PerGradeUpgrade': const(30), 'bExternalSlot': False},
        ]}))
    for name in ('Fake.Payload.Standard', 'Fake.Payload.Mirv', 'Fake.Delivery.Lob', 'Fake.Acc.None', 'Fake.Dmg.D0',
                 'Fake.Rad.R0', 'Fake.Child.C0', 'Fake.Status.S0', 'Fake.Material.M0'):
        add(name, 'WillowGame.GrenadeModPartDefinition', {})   # parts with no slot upgrades
    add('Fake.Attr.Scaler.ConstantAttributeValueResolver_0', 'WillowGame.ConstantAttributeValueResolver',
        {'ConstantValue': 1.5})
    add('Fake.Init.Dmg', 'Engine.AttributeInitializationDefinition', {'ValueFormula': {
        'bEnabled': True, 'Multiplier': const(2), 'Level': {'BaseValueAttribute': 'Fake.Attr.Scaler'},
        'Power': {'BaseValueAttribute': grenade.PROJECTILE_LEVEL}}})
    add('Fake.Proj.P', 'WillowGame.ProjectileDefinition', {
        'SpeedFormula': const(1000), 'GravityScaling': 0.5, 'PhysicsMode': 'PHYS_Falling',
        'Damage': {'InitializationDefinition': 'Fake.Init.Dmg', 'BaseValueScaleConstant': 3},
        'BehaviorProviderDefinition': 'Fake.Proj.P.BPD'})
    add('Fake.Proj.P.BPD', 'GearboxFramework.BehaviorProviderDefinition', {})
    add('Fake.Proj.P.BPD.Behavior_Explode_0', 'WillowGame.Behavior_Explode', {
        'DamageRadiusFormula': const(100), 'DamageFormula': {'BaseValueAttribute': 'Fake.Attr.ProjDamage'},
        'Definition': 'Fake.Expl.E'})
    add('Fake.Proj.P.BPD.Behavior_ModifyTimer_0', 'GearboxFramework.Behavior_ModifyTimer', {
        'TimerId': 'BTIMER_2', 'NewTimerDelay': const(1.8)})
    add('Fake.Expl.E', 'WillowGame.ExplosionDefinition', {'DamageTypeDef': 'Fake.DT.Explosive'})
    return package


def defaults():
    return FakePackage()


# FakePackage has no reader: its provider holds the decoded sequences directly (none unless a test adds them).
grenade.read_provider = lambda package, path: {'sequences': package.objects[path].get('sequences', [])}


def link(index, output=0):
    return {'behavior': index, 'outputId': output, 'delay': 0.0}


def graph(world_detonates=False):
    """An invented behaviour graph with the shape of prepare_action_skill.read_provider."""
    def behavior(index, cls, props, *links):
        return {'index': index, 'id': f'Fake.B{index}', 'class': cls, 'properties': props, 'variables': {}, 'links': list(links)}
    behaviors = [
        behavior(0, 'WillowGame.ProjectileBehavior_Bounce', {'ElasticityScaleFormula': const(0.3)}, link(1)),
        behavior(1, 'GearboxFramework.Behavior_Gate', {'GateCount': 1}, link(2)),
        behavior(2, 'GearboxFramework.Behavior_ModifyTimer', {'TimerId': 'BTIMER_2', 'NewTimerDelay': const(1.5)}),
        behavior(3, 'WillowGame.ProjectileBehavior_Bounce', {'ElasticityScaleFormula': const(0.6)}, link(1)),
        behavior(4, 'WillowGame.ProjectileBehavior_Detonate', {}),
        behavior(5, 'GearboxFramework.Behavior_ModifyTimer', {'NewTimerDelay': const(9)}),
        behavior(6, 'WillowGame.Behavior_Explode', {'bDisableRadiusDamageFalloff': True}),
    ]
    def event(name, *links):
        return {'name': name, 'enabled': True, 'outputs': {}, 'links': list(links)}
    world = [link(3)] + ([link(4)] if world_detonates else [])
    return [{'name': 'Default', 'enabledOnSpawn': True, 'behaviors': behaviors, 'check': {}, 'events': [
        event('TIMEREVENT_Timer1Elapsed', link(4)), event('TIMEREVENT_Timer2Elapsed', link(4)), event('OnExplode', link(6)), event('OnHitTheWorld', *world),
        event('OnHitDamagableObject', link(0)), event('OnSpawn', link(5))]}]


class ContactRulesTest(unittest.TestCase):
    def rules(self, sequences):
        return grenade.contact_rules(weapon_firing.Source(FakePackage(), defaults()), sequences, {}, set())

    def test_read_by_event(self):
        rules = self.rules(graph())
        self.assertAlmostEqual(rules['world_elasticity'], 0.6)
        self.assertAlmostEqual(rules['target_elasticity'], 0.3)
        self.assertAlmostEqual(rules['fuse_after_contact'], 1.5)
        self.assertAlmostEqual(rules['fuse_after_spawn'], 9)
        self.assertFalse(rules['contact_detonates'])
        self.assertTrue(rules['full_damage'])

    def test_a_detonate_only_a_timer_reaches_is_no_impact_fuse(self):
        self.assertTrue(self.rules(graph(world_detonates=True))['contact_detonates'])

    def test_graph_without_the_shape_is_none(self):
        self.assertIsNone(self.rules([]))
        sequences = graph()
        sequences[0]['events'] = [e for e in sequences[0]['events'] if not e['name'].startswith('TIMEREVENT_')]
        self.assertIsNone(self.rules(sequences))

    def without_event(self, name):
        sequences = graph()
        sequences[0]['events'] = [e for e in sequences[0]['events'] if e['name'] != name]
        return self.rules(sequences)

    def test_timer_names_its_event(self):
        self.assertEqual(grenade.timer_event(None), 'TIMEREVENT_Timer1Elapsed')
        self.assertEqual(grenade.timer_event('BTIMER_2'), 'TIMEREVENT_Timer2Elapsed')
        self.assertEqual(grenade.timer_event('BTIMER_3'), 'TIMEREVENT_Timer3Elapsed')

    def test_contact_timer_event_missing_is_none(self):
        self.assertIsNone(self.without_event('TIMEREVENT_Timer2Elapsed'))

    def test_spawn_timer_event_missing_is_none(self):
        self.assertIsNone(self.without_event('TIMEREVENT_Timer1Elapsed'))

    def test_a_timer_event_that_does_not_detonate_is_none(self):
        sequences = graph()
        next(e for e in sequences[0]['events'] if e['name'] == 'TIMEREVENT_Timer2Elapsed')['links'] = []
        self.assertIsNone(self.rules(sequences))

    def test_an_event_of_another_timer_does_not_count(self):
        sequences = graph()
        next(b for b in sequences[0]['behaviors'] if b['index'] == 2)['properties']['TimerId'] = 'BTIMER_3'
        self.assertIsNone(self.rules(sequences))

    def test_one_timer_set_by_both_is_none(self):
        sequences = graph()
        next(b for b in sequences[0]['behaviors'] if b['index'] == 5)['properties']['TimerId'] = 'BTIMER_2'
        self.assertIsNone(self.rules(sequences))

    def test_resolve_writes_contact_and_full_damage_into_the_block(self):
        package = world()
        package.objects['Fake.Proj.P.BPD']['sequences'] = graph()
        block = grenade.resolve(package, defaults(), {}, 2, MOD)
        self.assertAlmostEqual(block['projectile']['contact']['fuse_after_contact'], 1.5)
        self.assertFalse(block['projectile']['explode_on_impact'])


class PartChoiceTest(unittest.TestCase):
    def test_first_entry_is_default(self):
        package = world()
        parts = grenade.choose_parts(package, package.props(MOD), {})
        self.assertEqual(parts['Trigger'], 'Fake.Trig.G0')
        self.assertEqual(parts['Payload'], 'Fake.Payload.Standard')
        self.assertEqual(parts['StatusDamage'], 'Fake.Status.S0')

    def test_override_is_used(self):
        package = world()
        parts = grenade.choose_parts(package, package.props(MOD), {'Trigger': 'Fake.Trig.G2'})
        self.assertEqual(parts['Trigger'], 'Fake.Trig.G2')

    def test_override_not_in_list_is_refused(self):
        package = world()
        with self.assertRaises(SystemExit):
            grenade.choose_parts(package, package.props(MOD), {'Trigger': 'Fake.Dmg.D1'})


class SlotResolutionTest(unittest.TestCase):
    def resolve(self, overrides=None, level=2):
        package = world()
        source = weapon_firing.Source(package, defaults())
        return grenade.resolve(package, defaults(), overrides or {}, level, MOD), package, source

    def test_fuse_from_trigger_grade(self):
        # Grade 3 (one step above the base grade 1 plus two increases): 0.5 + 2 x 0.25.
        block, _, _ = self.resolve({'Trigger': 'Fake.Trig.G2'})
        self.assertAlmostEqual(block['fuse_time']['value'], 1.0, places=4)
        self.assertEqual(block['fuse_time']['slot']['grade'], 3)
        # Grade 0 with the default trigger: 0.5 + (0 - 1) x 0.25.
        block, _, _ = self.resolve()
        self.assertAlmostEqual(block['fuse_time']['value'], 0.25, places=4)
        self.assertEqual(block['fuse_time']['projectile_timers'][0]['timer'], 'BTIMER_2')
        self.assertAlmostEqual(block['fuse_time']['projectile_timers'][0]['delay'], 1.8, places=4)

    def test_radius_is_explosion_formula_plus_blast_slot(self):
        # Explosion radius 100, plus the DamageRadius slot at grade 2 (one step x 30).
        block, _, _ = self.resolve()
        self.assertAlmostEqual(block['radius']['value'], 130.0, places=4)
        self.assertEqual(block['radius']['explode_radius'], 100)

    def test_damage_is_init_at_level_times_scale_times_slot(self):
        # Init: 2 x (1.5 ^ level 2) = 4.5; x scale 3 = 13.5; Damage slot grade 2 gives x (1 + 0.1).
        block, _, _ = self.resolve({'Damage': 'Fake.Dmg.D1'}, level=2)
        self.assertIsNone(block['damage']['reason'])
        self.assertAlmostEqual(block['damage']['value'], 13.5 * 1.1, places=3)

    def test_unresolved_damage_is_null_with_reason(self):
        package = world()
        package.objects['Fake.Init.Dmg']['ValueFormula']['Level'] = {'BaseValueAttribute': 'Fake.Attr.Missing'}
        block = grenade.resolve(package, defaults(), {}, 2, MOD)
        self.assertIsNone(block['damage']['value'])
        self.assertIn('Fake.Attr.Missing', block['damage']['reason'])

    def test_projectile_speed_gravity_and_child_count(self):
        block, _, _ = self.resolve()
        self.assertEqual(block['projectile']['speed'], 1000)
        self.assertEqual(block['projectile']['gravity_scale'], 0.5)
        self.assertEqual(block['projectile']['physics_mode'], 'PHYS_Falling')
        self.assertEqual(block['child_count'], 0)
        self.assertEqual(block['payload'], 'Standard')
        self.assertEqual(block['damage_type'], 'Fake.DT.Explosive')
        self.assertTrue(block['unverified'])


class InheritedFieldsTest(unittest.TestCase):
    def inherited(self):
        package, class_defaults = world(), defaults()
        props = package.objects['Fake.Proj.P']
        cls = package.classes['Fake.Proj.P'].rsplit('.', 1)[-1]
        moved = {key: props.pop(key) for key in ('GravityScaling', 'Damage') if key in props}
        class_defaults.add('Default__' + cls, package.classes['Fake.Proj.P'], dict(moved, MaxLifespan=33.0, UpwardVelocityBonus=44.0))
        return package, class_defaults, moved

    def test_gravity_lifespan_bonus_and_damage_from_the_class_default(self):
        package, class_defaults, moved = self.inherited()
        self.assertIn('Damage', moved)
        block = grenade.resolve(package, class_defaults, {}, 8, MOD)
        self.assertEqual(block['projectile']['gravity_scale'], moved.get('GravityScaling'))
        self.assertEqual(block['projectile']['max_lifespan'], 33.0)
        self.assertEqual(block['projectile']['upward_velocity_bonus'], 44.0)
        self.assertGreater(block['damage']['value'], 0)
        self.assertIsNone(block['damage']['reason'])

    def test_damage_definition_only_in_the_class_defaults_package(self):
        package, class_defaults, _ = self.inherited()
        for path in ('Fake.Init.Dmg', 'Fake.Attr.Scaler.ConstantAttributeValueResolver_0'):
            class_defaults.add(path, package.classes[path], package.objects.pop(path))
        block = grenade.resolve(package, class_defaults, {}, 8, MOD)
        self.assertGreater(block['damage']['value'], 0)
        self.assertIsNone(block['damage']['reason'])

    def test_damage_definition_in_neither_package_is_a_reason(self):
        package, class_defaults, _ = self.inherited()
        del package.objects['Fake.Init.Dmg']
        block = grenade.resolve(package, class_defaults, {}, 8, MOD)
        self.assertIsNone(block['damage']['value'])
        self.assertIn('neither package', block['damage']['reason'])

    def test_the_projectile_own_value_wins_over_the_default(self):
        package, class_defaults, _ = self.inherited()
        package.objects['Fake.Proj.P']['MaxLifespan'] = 2.5
        self.assertEqual(grenade.resolve(package, class_defaults, {}, 8, MOD)["projectile"]["max_lifespan"], 2.5)


class OutputPathTest(unittest.TestCase):
    def test_path_outside_local_is_refused(self):
        with self.assertRaises(SystemExit):
            grenade.local_path(ROOT / 'tools' / 'grenade.json')
        with self.assertRaises(SystemExit):
            grenade.local_path(ROOT / 'local' / '..' / 'tools' / 'grenade.json')

    def test_path_inside_local_is_accepted(self):
        resolved = grenade.local_path(ROOT / 'local' / 'items' / 'grenades' / 'test.json')
        # Resolved on both sides: local/ may itself be a junction into another folder.
        self.assertIn((ROOT / 'local').resolve(), resolved.parents)


@unittest.skipUnless(os.name == 'nt', 'junctions are a Windows feature')
class JunctionPathTest(unittest.TestCase):
    """The output check on real junctions: a worktree's local/ may be one (pc-hummer uses them)."""

    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.base = Path(temp.name).resolve()
        self.repo = self.base / 'repo'
        self.store = self.base / 'store'
        self.repo.mkdir()
        self.store.mkdir()
        self.junction(self.repo / 'local', self.store)

    def junction(self, link, target):
        subprocess.run(['cmd', '/c', 'mklink', '/J', str(link), str(target)], check=True, capture_output=True)
        self.addCleanup(os.rmdir, link)   # removes the link only; runs before the folder's own cleanup

    def test_output_behind_a_junctioned_local_is_accepted(self):
        resolved = grenade.local_path(self.repo / 'local' / 'items' / 'test.json', repo=self.repo)
        self.assertEqual(resolved, self.store / 'items' / 'test.json')

    def test_path_outside_is_still_refused(self):
        with self.assertRaises(SystemExit):
            grenade.local_path(self.repo / 'tools' / 'test.json', repo=self.repo)
        with self.assertRaises(SystemExit):
            grenade.local_path(self.repo / 'local' / '..' / 'tools' / 'test.json', repo=self.repo)

    def test_a_junction_that_leaves_local_is_refused(self):
        outside = self.base / 'outside'
        outside.mkdir()
        self.junction(self.store / 'escape', outside)
        with self.assertRaises(SystemExit):
            grenade.local_path(self.repo / 'local' / 'escape' / 'test.json', repo=self.repo)


if __name__ == '__main__':
    unittest.main()
