"""Synthetic tests for tools/prepare_interactives.py: invented packages, no game data, no ow-package needed.

The fixture helpers (FakeReader, ref, const, constant_attribute, property_attribute, attr_init, def_init, formula, pool)
are the ones tests/prepare_vendors_test.py builds its invented packages with.
"""
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
sys.path.insert(0, str(Path(__file__).resolve().parent))
import prepare_interactives as pi  # noqa: E402
from prepare_vendors_test import (FakeReader, attr_init, const, constant_attribute, def_init, formula,  # noqa: E402
                                  pool, property_attribute, ref, unresolved_attribute)


class Reader(FakeReader):
    """FakeReader that also takes the allow_tail flag the AnimSequence reads pass."""

    def props(self, pkg, path, allow_tail=False):
        return super().props(pkg, path)


def config(name, weight, attachments):
    return {'ConfigurationName': name, 'Weight': weight,
            'ItemAttachments': [{'ItemPool': ref(p), 'PoolProbability': const(prob), 'AttachmentPointName': point}
                                for point, p, prob in attachments]}


def point(index, popdef, x, extra=None):
    return {f'TheWorld.PersistentLevel.WillowPopulationOpportunityPoint_{index}': (
        'WillowGame.WillowPopulationOpportunityPoint',
        {'PopulationDef': ref(popdef), 'Location': {'X': x, 'Y': 2.0, 'Z': 3.0},
         'Rotation': {'Pitch': 0, 'Yaw': -16384, 'Roll': 0}, 'GameStageRegion': ref('Stages.Town'),
         'MinimumGameStageForRegion': 7, 'MaximumGameStageForRegion': 9, **(extra or {})})}


def population(popdef, balance):
    objects = {popdef: ('GearboxFramework.PopulationDefinition', {})}
    if balance:
        objects[f'{popdef}.PopulationFactoryInteractiveObject_0'] = (
            'WillowGame.PopulationFactoryInteractiveObject', {'ObjectBalanceDefinition': ref(balance)})
    return objects


def definition(path, mesh, behaviours=None, components=1):
    """An InteractiveObjectDefinition with `components` identical skeletal mesh components and its behaviours."""
    objects = {path: ('WillowGame.InteractiveObjectDefinition',
                      {'BehaviorProviderDefinition': ref(f'{path}.BPD_0'), 'HUDIconDef': ref('Icons.Open')}),
               f'{path}.BPD_0': ('GearboxFramework.BehaviorProviderDefinition', {}),
               f'{path}.ParticleSystemComponent_0': ('Engine.ParticleSystemComponent', {'Template': ref('FX.Dust')})}
    for i in range(components):
        objects[f'{path}.SkeletalMeshComponent_{i}'] = ('Engine.SkeletalMeshComponent', {
            'SkeletalMesh': ref(mesh), 'PhysicsAsset': ref(mesh + '_Phys'), 'AnimTreeTemplate': ref('Anim.Tree'),
            'AnimSets': [ref('Anim.Set_Box')], 'Scale': 2.0})
    for i, (cls, props) in enumerate(behaviours or []):
        objects[f'{path}.BPD_0.{cls}_{i}'] = (f'WillowGame.{cls}', props)
    return objects


OPEN_ANIMS = [('Behavior_SimpleAnimPlay', {'Tree': ref('Anim.Tree'), 'AnimName': 'Open'}),
              ('Behavior_SimpleAnimPlay', {'Tree': ref('Anim.Tree'), 'AnimName': 'Closed'})]


def map_packages():
    """Map 'Map_P' with five treasure points, one point of another population and one point without a balance.

    Points: 1 box (lootable), 2 a second box sharing the balance, 3 an ammo crate (loot from an included list),
    4 a golden chest (usability cost), 5 a mission chest (updates an objective), 6 not treasure, 7 no balance.
    """
    m = {}
    m.update(population('GD_Population_Treasure.Lootables.Box', 'Grades.Box'))
    m.update(population('GD_Population_Treasure.Lootables.Crate_Ammo', 'Grades.Crate'))
    m.update(population('GD_Population_Treasure.TreasureChests.Chest_Gold', 'Grades.Gold'))
    m.update(population('GD_Population_Treasure.TreasureChests.Chest_Mission', 'Grades.Mission'))
    m.update(population('GD_Population_Treasure.Lootables.Broken', None))
    m.update(pool('Pools.Cash', {'Grades.Cash': 3.0, 'Grades.Ammo': 1.0}))
    m.update(pool('Pools.Guns', {'Weapons.Pistol': 1.0}))
    m['Weights.Runtime'] = ('Engine.AttributeDefinition', {})
    # Box: two weighted configurations, one whose weight is a runtime attribute, and a grade outside stage 8.
    m['Grades.Box'] = ('WillowGame.InteractiveObjectBalanceDefinition', {
        'DefaultInteractiveObject': ref('Defs.Box'),
        'DefaultLoot': [config('Box_Cash', const(3), [('Ammo1', 'Pools.Cash', 1.0)]),
                        config('Box_Gun', const(1), [('Gun', 'Pools.Guns', 0.5)]),
                        config('Box_Runtime', unresolved_attribute('Weights.Runtime'), [('Ammo1', 'Pools.Cash', 1.0)])],
        'Grades': [{'GameStageRequirement': {'MinGameStage': 1, 'MaxGameStage': 5},
                    'GradeModifiers': {'CustomLoot': [config('Early', const(9), [('Ammo1', 'Pools.Cash', 1.0)])]}},
                   {'GameStageRequirement': {'MinGameStage': 6, 'MaxGameStage': 100}, 'GradeModifiers': {}}]})
    m.update(definition('Defs.Box', 'Props.Skel_Box', OPEN_ANIMS, components=2))
    m['Props.Skel_Box'] = ('Engine.SkeletalMesh', {})
    m['Props.Skel_Box.SkeletalMeshSocket_1'] = ('Engine.SkeletalMeshSocket', {
        'SocketName': 'Gun', 'BoneName': 'Lid', 'RelativeLocation': {'X': 1.5, 'Z': 3.0}})
    m['Props.Skel_Box.SkeletalMeshSocket_0'] = ('Engine.SkeletalMeshSocket', {
        'SocketName': 'Ammo1', 'BoneName': 'Base', 'RelativeLocation': {'X': 7.0, 'Y': -2.0, 'Z': 4.0}})
    m['Props.Skel_Box.SomethingElse_0'] = ('Engine.Object', {'SocketName': 'NotASocket'})
    # Crate: no DefaultLoot; its loot comes from an InteractiveObjectLootListDefinition.
    m['Grades.Crate'] = ('WillowGame.InteractiveObjectBalanceDefinition', {
        'DefaultInteractiveObject': ref('Defs.Crate'), 'DefaultIncludedLootLists': [ref('Lists.Crate')]})
    m['Lists.Crate'] = ('WillowGame.InteractiveObjectLootListDefinition', {
        'LootData': [config('Crate', const(100), [('Ammo1', 'Pools.Cash', 1.0), ('Ammo2', 'Pools.Cash', 0.8)])]})
    m.update(definition('Defs.Crate', 'Props.Skel_Crate', OPEN_ANIMS))
    # Golden chest: a usability cost of one CURRENCY_Reserved_A.
    m['Grades.Gold'] = ('WillowGame.InteractiveObjectBalanceDefinition', {
        'DefaultInteractiveObject': ref('Defs.Gold'),
        'DefaultLoot': [config('Gold', const(1), [('Gun1', 'Pools.Guns', 1.0)])]})
    m.update(definition('Defs.Gold', 'Props.Skel_Chest', OPEN_ANIMS + [
        ('Behavior_SetUsabilityCost', {'ChangeCost': 'CHANGE_Enable', 'CostCurrencyType': 'CURRENCY_Reserved_A',
                                       'CostAmount': 1})]))
    # Mission chest: linked to a mission and completes one of its objectives.
    m['Grades.Mission'] = ('WillowGame.InteractiveObjectBalanceDefinition', {
        'DefaultInteractiveObject': ref('Defs.Mission'),
        'DefaultLoot': [config('Mission', const(1), [('Gun1', 'Pools.Guns', 1.0)])]})
    m.update(definition('Defs.Mission', 'Props.Skel_Chest', OPEN_ANIMS + [
        ('BehaviorSequenceEnableByMission', {'LinkedMission': ref('Missions.M_Test'),
                                             'LinkedObjective': ref('Missions.M_Test.Open')}),
        ('Behavior_UpdateMissionObjective', {'MissionObjective': ref('Missions.M_Test.Open')})]))
    # Animation: the anim set lists its sequences; the tree names its simple anims.
    m['Anim.Tree'] = ('WillowGame.WillowAnimTree', {'SimpleAnimNames': ['Open', 'Opened', 'Closed']})
    m['Anim.Set_Box'] = ('Engine.AnimSet', {'Sequences': [ref('Anim.Set_Box.Seq_0'), ref('Anim.Set_Box.Seq_1')]})
    m['Anim.Set_Box.Seq_0'] = ('Engine.AnimSequence', {'SequenceName': 'Closed_Idle'})
    m['Anim.Set_Box.Seq_1'] = ('Engine.AnimSequence', {'SequenceName': 'Open'})
    m.update(point(1, 'GD_Population_Treasure.Lootables.Box', 10.0, {'MissionObjectiveBit': 4}))
    m.update(point(2, 'GD_Population_Treasure.Lootables.Box', 20.0))
    m.update(point(3, 'GD_Population_Treasure.Lootables.Crate_Ammo', 30.0))
    m.update(point(4, 'GD_Population_Treasure.TreasureChests.Chest_Gold', 40.0, {'Description': 'Test key chest'}))
    m.update(point(5, 'GD_Population_Treasure.TreasureChests.Chest_Mission', 50.0))
    m.update(point(6, 'GD_Population_Shopping.VendingMachine.Shop', 60.0))
    m.update(point(7, 'GD_Population_Treasure.Lootables.Broken', 70.0))
    return {'Map_P': m, 'Startup': startup_packages()}


def startup_packages():
    """Invented pickups: cash (slot effect, two part lists), an ammo item (conditional amount) and a pistol.

    Cash: calc(L) = 2 * 1.5 ^ L; base scale 1, per-grade scale 0.5; Alpha parts grade 0 or 10, Beta parts 0..2.
    Ammo: amount = round(30 (playthrough 1; 60 at playthrough 2) * 1 (not a shared copy)).
    """
    s = {'Weapons.Pistol': ('WillowGame.WeaponBalanceDefinition', {})}
    s.update(constant_attribute('Econ.Cash_Base', 2.0))
    s.update(constant_attribute('Econ.Scaler', 1.5))
    s.update(property_attribute('Attr.CurrencyLevel', 'ExpLevel'))
    s['Econ.Init_Cash'] = formula(attr_init('Econ.Cash_Base'), attr_init('Econ.Scaler'), attr_init('Attr.CurrencyLevel'))
    for name, grades in (('Alpha', [0, 10]), ('Beta', [0, 1, 2])):
        parts = []
        for g in grades:
            part = f'Money.{name}_{g}'
            s[part] = ('WillowGame.UsableItemPartDefinition',
                       {'AttributeSlotUpgrades': [{'SlotName': 'BaseCredits', 'GradeIncrease': g}]})
            parts.append({'Part': ref(part), 'Manufacturers': []})
        s[f'Money.{name}Parts'] = ('WillowGame.ItemPartListDefinition', {'WeightedParts': parts})
    s['Items.Cash'] = ('WillowGame.UsableItemDefinition', {
        'ItemName': 'Test Money', 'bPlayerUseItemOnPickup': True, 'bAutomaticallyPickup': True,
        'NonCompositeStaticMesh': ref('Pickups.Cash'), 'AlphaParts': ref('Money.AlphaParts'),
        'BetaParts': ref('Money.BetaParts'),
        'AttributeSlotEffects': [{'SlotName': 'BaseCredits', 'AttributeToModify': ref(pi.CREDITS),
                                  'BaseModifierValue': {**def_init('Econ.Init_Cash'), 'BaseValueScaleConstant': 1},
                                  'PerGradeUpgrade': {**def_init('Econ.Init_Cash'), 'BaseValueScaleConstant': 0.5}}]})
    s['Grades.Cash'] = ('WillowGame.InventoryBalanceDefinition', {
        'InventoryDefinition': ref('Items.Cash'),
        'Manufacturers': [{'Grades': [{'GradeModifiers': {'ExpLevel': 1}}]}]})
    # Ammo: AmmoAmount is 30, or 60 when PlayThroughCount == 2; the shared-copy percentage defaults to 1.
    s['Ammo.Amount'] = ('WillowGame.InventoryAttributeDefinition', {})
    s['Ammo.Amount.ConditionalAttributeValueResolver_0'] = ('GearboxFramework.ConditionalAttributeValueResolver', {
        'ValueExpressions': {'ConditionalExpressionList': [{
            'BaseValueIfTrue': const(60), 'Expressions': [{
                'AttributeOperand1': ref('D_Attributes.Balance.PlayThroughCount'),
                'ComparisonOperator': 'OPERATOR_EqualTo', 'ConstantOperand2': 2}]}],
            'DefaultBaseValue': const(30)}})
    s['Ammo.Init_Percentage'] = ('Engine.AttributeInitializationDefinition', {'ConditionalInitialization': {
        'bEnabled': True, 'ConditionalExpressionList': [{
            'BaseValueIfTrue': const(0.5), 'Expressions': [{
                'AttributeOperand1': ref('D_Attributes.Inventory.ClonedForSharing'),
                'ComparisonOperator': 'OPERATOR_EqualTo', 'ConstantOperand2': 1}]}],
        'DefaultBaseValue': const(1)}})
    s['Ammo.Init_Amount'] = ('Engine.AttributeInitializationDefinition', {
        'RoundingMode': 'ATTRROUNDING_IntRound',
        'ValueFormula': {'bEnabled': True, 'Multiplier': attr_init('Ammo.Amount'),
                         'Level': def_init('Ammo.Init_Percentage'), 'Power': const(1)}})
    s['Attr.Ammo_SMG_Current'] = ('WillowGame.ResourcePoolAttributeDefinition',
                                  {'Resource': ref('D_Resources.AmmoResources.Ammo_Patrol_SMG')})
    s['Items.Ammo'] = ('WillowGame.UsableItemDefinition', {
        'ItemName': 'Test Ammo', 'bPlayerUseItemOnPickup': True, 'bAutomaticallyPickup': True,
        'NonCompositeStaticMesh': ref('Pickups.Ammo'),
        'ExternalAttributeEffects': [{'AttributeToModify': ref('Attr.Ammo_SMG_Current'), 'ModifierType': 'MT_PostAdd',
                                      'BaseModifierValue': {'BaseValueConstant': 24, **def_init('Ammo.Init_Amount'),
                                                            'BaseValueScaleConstant': 1}}]})
    s['Grades.Ammo'] = ('WillowGame.InventoryBalanceDefinition', {'InventoryDefinition': ref('Items.Ammo')})
    return s


class PrepareInteractivesTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        cls.doc = pi.build_document(Reader(map_packages(), cls.tmp.name), 'Map_P', 8.0)

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def objects(self):
        return {o['id'].rsplit('_', 1)[1]: o for o in self.doc['objects']}

    def test_kinds_and_skipped_points(self):
        kinds = {k: o['kind'] for k, o in self.objects().items()}
        self.assertEqual(kinds, {'1': 'lootable', '2': 'lootable', '3': 'ammo_crate', '4': 'chest', '5': 'chest'})
        self.assertEqual(self.doc['counts'], {'lootable': 2, 'ammo_crate': 1, 'chest': 2})
        broken = [u for u in self.doc['unresolved'] if u.get('object', '').endswith('_7')]
        self.assertEqual(len(broken), 1)
        self.assertIn('no factory balance', broken[0]['reason'])

    def test_object_record(self):
        box = self.objects()['1']
        self.assertEqual(box['location'], [10.0, 2.0, 3.0])
        self.assertEqual(box['rotation'], [0, -90.0, 0])
        self.assertEqual(box['rotation_units'], [0, -16384, 0])
        self.assertEqual(box['balance'], 'Grades.Box')
        self.assertEqual(box['definition'], 'Defs.Box')
        self.assertEqual(box['game_stage'], {'region': 'Stages.Town', 'min': 7, 'max': 9})
        self.assertEqual(box['mission_objective_bit'], 4)
        self.assertTrue(box['supported'])
        self.assertIsNone(box['unsupported_reason'])

    def test_balance_shared_by_two_points_is_described_once(self):
        self.assertEqual(self.objects()['2']['balance'], 'Grades.Box')
        self.assertEqual(set(self.doc['balances']), {'Grades.Box', 'Grades.Crate', 'Grades.Gold', 'Grades.Mission'})

    def test_golden_chest_needs_a_key_and_mission_chest_a_mission(self):
        gold, mission = self.objects()['4'], self.objects()['5']
        self.assertFalse(gold['supported'])
        self.assertEqual(gold['unsupported_reason'], 'needs a golden key')
        self.assertEqual(gold['description'], 'Test key chest')
        cost = self.doc['balances']['Grades.Gold']['requires']['cost']
        self.assertEqual(cost, {'currency': 'CURRENCY_Reserved_A', 'amount': 1, 'golden_key': True})
        self.assertFalse(mission['supported'])
        self.assertIn('Missions.M_Test.Open', mission['unsupported_reason'])
        needs = self.doc['balances']['Grades.Mission']['requires']
        self.assertEqual(needs['missions'], [{'mission': 'Missions.M_Test', 'objective': 'Missions.M_Test.Open'}])

    def test_configuration_weights_shares_and_attachments(self):
        configs = {c['name']: c for c in self.doc['balances']['Grades.Box']['configurations']}
        # The grade's CustomLoot holds stage 1..5 only, so it is not listed at stage 8.
        self.assertEqual(set(configs), {'Box_Cash', 'Box_Gun', 'Box_Runtime'})
        self.assertEqual(configs['Box_Cash']['weight'], 3)
        self.assertAlmostEqual(configs['Box_Cash']['share'], 0.75)
        self.assertAlmostEqual(configs['Box_Gun']['share'], 0.25)
        self.assertIsNone(configs['Box_Runtime']['weight'])
        self.assertIsNone(configs['Box_Runtime']['share'])
        self.assertTrue(any(u.get('config') == 'Box_Runtime' and 'Weight' in u['reason'] for u in self.doc['unresolved']))
        gun = configs['Box_Gun']['attachments'][0]
        self.assertEqual((gun['point'], gun['pool'], gun['probability']), ('Gun', 'Pools.Guns', 0.5))
        self.assertEqual(gun['items'], [{'balance': 'Weapons.Pistol', 'weight': 1.0, 'class': 'WeaponBalanceDefinition'}])
        cash = {leaf['balance']: leaf['weight'] for leaf in configs['Box_Cash']['attachments'][0]['items']}
        self.assertEqual(cash, {'Grades.Cash': 0.75, 'Grades.Ammo': 0.25})

    def test_early_grade_loot_is_listed_at_its_stage(self):
        early = pi.build_document(Reader(map_packages(), self.tmp.name), 'Map_P', 3.0)
        configs = {c['name']: c for c in early['balances']['Grades.Box']['configurations']}
        self.assertEqual(configs['Early']['source'], 'grade CustomLoot')

    def test_included_loot_list(self):
        crate = self.doc['balances']['Grades.Crate']['configurations']
        self.assertEqual([(c['name'], c['source'], c['share']) for c in crate], [('Crate', 'Lists.Crate', 1.0)])
        self.assertEqual([(a['point'], a['probability']) for a in crate[0]['attachments']],
                         [('Ammo1', 1.0), ('Ammo2', 0.8)])

    def test_visual(self):
        visual = self.doc['balances']['Grades.Box']['visual']
        self.assertEqual(len(visual['components']), 1)  # two identical components drawn once
        component = visual['components'][0]
        self.assertEqual(component['skeletal_mesh'], 'Props.Skel_Box')
        self.assertEqual(component['physics_asset'], 'Props.Skel_Box_Phys')
        self.assertEqual(component['anim_sets'], ['Anim.Set_Box'])
        self.assertEqual(component['sequences'], {'Anim.Set_Box': ['Closed_Idle', 'Open']})
        self.assertEqual(component['tree_anim_names'], ['Open', 'Opened', 'Closed'])
        self.assertEqual(component['scale'], 2.0)
        self.assertEqual(visual['played_anims'], ['Closed', 'Open'])
        self.assertEqual((visual['open_animation'], visual['open_sequence']), ('Open', 'Open'))
        self.assertEqual(visual['particles'], ['FX.Dust'])
        self.assertEqual(self.doc['balances']['Grades.Box']['hud_icon'], 'Icons.Open')

    def test_a_meshes_sockets_are_listed_by_name(self):
        component = self.doc['balances']['Grades.Box']['visual']['components'][0]
        self.assertEqual(component['sockets'], [
            {'name': 'Ammo1', 'bone': 'Base', 'location': [7.0, -2.0, 4.0]},
            {'name': 'Gun', 'bone': 'Lid', 'location': [1.5, 0.0, 3.0]}])    # a value left out reads as 0

    def test_a_mesh_without_sockets_lists_none(self):
        self.assertEqual(self.doc['balances']['Grades.Crate']['visual']['components'][0]['sockets'], [])

    def test_cash_pickup_rule(self):
        item = self.doc['items']['Grades.Cash']
        cash = item['pickup']['cash']
        self.assertEqual(item['pickup']['kind'], 'cash')
        self.assertTrue(item['pickup']['auto_pickup'])
        self.assertEqual(item['pickup']['mesh'], 'Pickups.Cash')
        self.assertEqual(item['pickup']['manufacturer_grade'], 1)
        self.assertEqual(cash['constants'], {'Multiplier': 2.0, 'Level': 1.5})
        self.assertEqual(cash['slot_grades'], {'Alpha': [0, 10], 'Beta': [0, 1, 2]})
        self.assertEqual(cash['grade_range'], [0, 12])
        self.assertEqual(cash['level_attributes'], ['Attr.CurrencyLevel'])
        # calc(5) = 2 * 1.5 ^ 5; amount = calc(5) * (1 + 0.5 * grade) for grade 0 and 12.
        low, high = cash['amount_at_level']['5']
        calc5 = 2 * 1.5 ** 5
        self.assertAlmostEqual(low, round(calc5, 3), places=2)
        self.assertAlmostEqual(high, round(calc5 * 7, 3), places=2)

    def test_ammo_pickup_amount_at_playthrough_one(self):
        pickup = self.doc['items']['Grades.Ammo']['pickup']
        self.assertEqual((pickup['kind'], pickup['ammo_type'], pickup['amount']), ('ammo', 'smg', 30))
        self.assertEqual(pickup['ammo_pool'], 'D_Resources.AmmoResources.Ammo_Patrol_SMG')

    def test_assets_list(self):
        assets = self.doc['assets']
        self.assertEqual(assets['skeletal_meshes'], ['Props.Skel_Box', 'Props.Skel_Chest', 'Props.Skel_Crate'])
        self.assertEqual(assets['anim_sets'], ['Anim.Set_Box'])
        self.assertEqual(assets['anim_trees'], ['Anim.Tree'])
        self.assertEqual(assets['pickup_meshes'], ['Pickups.Ammo', 'Pickups.Cash'])

    def test_quick_change_machine_is_found_by_its_definition(self):
        packages = {'Map_P': {
            'TheWorld.PersistentLevel.WillowInteractiveObject_1': ('WillowGame.WillowInteractiveObject', {
                'InteractiveObjectDefinition': 'GD_Invented.Things.CharacterCustomizationMachine',
                'Location': {'X': 1.0, 'Y': 2.0, 'Z': 3.0}, 'Rotation': {'Yaw': 16384}}),
            'TheWorld.PersistentLevel.WillowInteractiveObject_2': ('WillowGame.WillowInteractiveObject', {
                'InteractiveObjectDefinition': 'GD_Invented.Things.SomethingElse'})}}
        reader = Reader(packages, self.tmp.name)
        found = pi.placed_stations(reader, pi.pv.Chain(reader, ['Map_P']), 'Map_P')
        self.assertEqual([s['id'] for s in found], ['Map_P:WillowInteractiveObject_1'])
        self.assertEqual(found[0]['location'], [1.0, 2.0, 3.0])
        self.assertEqual(found[0]['rotation'], [0.0, 90.0, 0.0])

    def test_document_shape(self):
        self.assertEqual(self.doc['schema'], 'openwillow-interactives/1')
        self.assertEqual((self.doc['map'], self.doc['stage']), ('Map_P', 8.0))
        self.assertEqual(set(self.doc), {'schema', 'map', 'stage', 'counts', 'objects', 'stations', 'balances', 'items', 'assets',
                                         'unresolved', 'notes'})
        self.assertEqual(set(self.objects()['1']), {
            'id', 'kind', 'population', 'balance', 'definition', 'location', 'rotation', 'rotation_units',
            'game_stage', 'mission_objective_bit', 'description', 'supported', 'unsupported_reason'})
        self.assertNotIn('Weapons.Pistol', self.doc['items'])  # weapons get no item block
        json.dumps(self.doc)  # must be plain JSON

    def test_output_refused_outside_local(self):
        repo = Path(self.tmp.name) / 'repo'
        (repo / 'local').mkdir(parents=True, exist_ok=True)
        self.assertEqual(pi.pv.local_path('local/slice/interactives.json', repo),
                         (repo / 'local' / 'slice' / 'interactives.json').resolve())
        for bad in ['interactives.json', 'local', '../escape.json', 'docs/interactives.json']:
            with self.subTest(bad=bad), self.assertRaises(ValueError):
                pi.pv.local_path(bad, repo)


if __name__ == '__main__':
    unittest.main()
