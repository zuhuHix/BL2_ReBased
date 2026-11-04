"""Synthetic tests for tools/prepare_vendors.py: invented packages, no game data, no ow-package needed."""
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import prepare_vendors as pv  # noqa: E402


def ref(path, index=1):
    """An object reference as Reader.props() returns it."""
    return {'index': index, 'path': path}


def const(value):
    """An AttributeInitializationData with a constant base value, as Reader.props() returns it."""
    return {'BaseValueConstant': value, 'BaseValueAttribute': {'index': 0, 'path': None},
            'InitializationDefinition': {'index': 0, 'path': None}, 'BaseValueScaleConstant': 1}


def unresolved_attribute(path):
    """An AttributeInitializationData whose base comes from an attribute the packages do not value."""
    return {'BaseValueConstant': 1, 'BaseValueAttribute': ref(path), 'InitializationDefinition': {'index': 0, 'path': None},
            'BaseValueScaleConstant': 1}


class FakeReader:
    """Stands in for prepare_slice_world.Reader: packages = {name: {path: (class, props)}}."""

    def __init__(self, packages, cooked):
        self.packages = packages
        self.cooked = Path(cooked)
        self.defaults = {'WillowGame.GlobalsDefinition': {'MinutesBetweenShopResets': 20}}

    def exports(self, pkg):
        items = self.packages[pkg]
        by_path = {}
        for i, (path, (cls, _)) in enumerate(items.items(), start=1):
            by_path[path] = {'index': i, 'name': path.rsplit('.', 1)[-1], 'path': path, 'class': cls,
                             'outer_index': 0, 'archetype_index': 0}
        return {'by_path': by_path, 'by_index': {x['index']: x for x in by_path.values()}}

    def props(self, pkg, path):
        return self.packages[pkg][path][1]

    def class_default(self, cls):
        return self.defaults[cls]


def pool(path, balances):
    """An ItemPoolDefinition with equal-probability balance entries given as {balance: weight}."""
    entries = [{'InvBalanceDefinition': ref(b), 'Probability': const(w)} for b, w in balances.items()]
    return {path: ('WillowGame.ItemPoolDefinition', {'BalancedItems': entries, 'Quantity': const(1)})}


def machine_packages(ammo_probability=1.0):
    """A map 'Map_P' with four machines on one point each, plus Startup holding the balances.

    Machines: weapons (no ShopType), items (SType_Items), health (SType_Health, definition taken from the balance),
    black market (its own factory class), and one point with an unknown ShopType and one non-vending point.
    The stock pool lives in the map package and its balances in Startup (the two-package fallback).
    """
    startup = {
        'GD_Weapons.Pistol.Balance': ('WillowGame.WeaponBalanceDefinition', {}),
        'GD_Weapons.SMG.Balance': ('WillowGame.WeaponBalanceDefinition', {}),
        'GD_Items.Ammo.Grade': ('WillowGame.InventoryBalanceDefinition', {}),
        'GD_Items.Health.Grade': ('WillowGame.InventoryBalanceDefinition', {}),
    }
    map_pkg = {}
    map_pkg.update(pool('GD_ItemPools_Shop.Weapons.Stock', {'GD_Weapons.Pistol.Balance': 1.0,
                                                            'GD_Weapons.SMG.Balance': 3.0}))
    map_pkg.update(pool('GD_ItemPools_Shop.Ammo.Stock', {'GD_Items.Ammo.Grade': 1.0}))
    map_pkg.update(pool('GD_ItemPools_Shop.Health.Stock', {'GD_Items.Health.Grade': 1.0}))
    map_pkg.update(pool('GD_ItemPools_Shop.Featured.Gun', {'GD_Weapons.SMG.Balance': 1.0}))
    map_pkg['GD_Balance_Shopping.Att_Probability'] = ('Engine.AttributeDefinition', {})

    def factory(popdef, cls, props):
        return {f'{popdef}.PopulationFactoryVendingMachine_0': (cls, props)}

    def balance(path, definition, loot):
        return {path: ('WillowGame.InteractiveObjectBalanceDefinition',
                       {'DefaultInteractiveObject': ref(definition), 'DefaultLoot': loot})}

    def definition(path, header):
        return {path: ('WillowGame.InteractiveObjectDefinition', {'StatusMenuMapInfoBoxHeader': header})}

    def loot(pool_path, probability=None):
        return [{'ConfigurationName': 'Inventory',
                 'ItemAttachments': [{'ItemPool': ref(pool_path),
                                      'PoolProbability': probability or const(1.0)}]},
                {'ConfigurationName': 'FeaturedItem',
                 'ItemAttachments': [{'ItemPool': ref('GD_ItemPools_Shop.Featured.Gun'),
                                      'PoolProbability': const(1.0)}]}]

    def point(index, popdef, x):
        return {f'TheWorld.PersistentLevel.WillowPopulationOpportunityPoint_{index}': (
            'WillowGame.WillowPopulationOpportunityPoint',
            {'PopulationDef': ref(popdef), 'Location': {'X': x, 'Y': 2.0, 'Z': 3.0},
             'Rotation': {'Pitch': 0, 'Yaw': 16384, 'Roll': 0}})}

    # Weapons machine: no ShopType; the definition is the factory's own ObjectDefinition.
    weapons_def = 'GD_Population_Shopping.VendingMachine.VendingMachine_Weapons'
    map_pkg.update(factory(weapons_def, 'WillowGame.PopulationFactoryVendingMachine', {
        'ObjectBalanceDefinition': ref('GD_Balance_Shopping.Grades.Weapons'),
        'ObjectDefinition': ref('GD_Balance_Shopping.Defs.Weapons'),
        'CommerceMarkup': {'InitializationDefinition': ref('GD_Economy.Init_Markup'), 'BaseValueConstant': 1,
                           'BaseValueAttribute': {'index': 0, 'path': None},
                           'BaseValueScaleConstant': 1},
        'FeaturedItem': {'CommerceMarkup': {'InitializationDefinition': ref('GD_Economy.Init_Markup')}}}))
    map_pkg.update(balance('GD_Balance_Shopping.Grades.Weapons', 'GD_Balance_Shopping.Defs.Weapons',
                           loot('GD_ItemPools_Shop.Weapons.Stock')))
    map_pkg.update(definition('GD_Balance_Shopping.Defs.Weapons', 'Test Weapons Vending Machine'))
    map_pkg.update(point(1, weapons_def, 10.0))
    map_pkg.update({weapons_def: ('GearboxFramework.PopulationDefinition', {})})

    # Items machine: ShopType SType_Items.
    items_def = 'GD_Population_Shopping.VendingMachine.VendingMachine_Items'
    map_pkg.update({items_def: ('GearboxFramework.PopulationDefinition', {})})
    map_pkg.update(factory(items_def, 'WillowGame.PopulationFactoryVendingMachine', {
        'ShopType': 'SType_Items', 'ObjectBalanceDefinition': ref('GD_Balance_Shopping.Grades.Items'),
        'ObjectDefinition': ref('GD_Balance_Shopping.Defs.Items')}))
    map_pkg.update(balance('GD_Balance_Shopping.Grades.Items', 'GD_Balance_Shopping.Defs.Items',
                           loot('GD_ItemPools_Shop.Ammo.Stock', unresolved_attribute('GD_Balance_Shopping.Att_Probability')
                                if ammo_probability is None else const(ammo_probability))))
    map_pkg.update(definition('GD_Balance_Shopping.Defs.Items', 'Test Ammo Dump'))
    map_pkg.update(point(2, items_def, 20.0))

    # Health machine: no ObjectDefinition on the factory, so the balance's DefaultInteractiveObject is the definition.
    health_def = 'GD_Population_Shopping.VendingMachine.VendingMachine_Health'
    map_pkg.update({health_def: ('GearboxFramework.PopulationDefinition', {})})
    map_pkg.update(factory(health_def, 'WillowGame.PopulationFactoryVendingMachine', {
        'ShopType': 'SType_Health', 'ObjectBalanceDefinition': ref('GD_Balance_Shopping.Grades.Health')}))
    map_pkg.update(balance('GD_Balance_Shopping.Grades.Health', 'GD_Balance_Shopping.Defs.Health',
                           loot('GD_ItemPools_Shop.Health.Stock')))
    map_pkg.update(definition('GD_Balance_Shopping.Defs.Health', 'Test Health Vending Machine'))
    map_pkg.update(point(3, health_def, 30.0))

    # Black market: its own factory class, no stock.
    black_def = 'GD_Population_Shopping.VendingMachine.Black_Market'
    map_pkg.update({black_def: ('GearboxFramework.PopulationDefinition', {})})
    map_pkg.update({f'{black_def}.PopulationFactoryBlackMarket_0': (
        'WillowGame.PopulationFactoryBlackMarket', {'ObjectBalanceDefinition': ref('GD_Balance_Shopping.Grades.Black')})})
    map_pkg.update(balance('GD_Balance_Shopping.Grades.Black', 'GD_Balance_Shopping.Defs.Black', []))
    map_pkg.update(definition('GD_Balance_Shopping.Defs.Black', 'Test Black Market'))
    map_pkg.update(point(4, black_def, 40.0))

    # A machine with a ShopType this tool does not know: reported, not invented.
    odd_def = 'GD_Population_Shopping.VendingMachine.Odd'
    map_pkg.update({odd_def: ('GearboxFramework.PopulationDefinition', {})})
    map_pkg.update(factory(odd_def, 'WillowGame.PopulationFactoryVendingMachine', {
        'ShopType': 'SType_Mystery', 'ObjectBalanceDefinition': ref('GD_Balance_Shopping.Grades.Weapons')}))
    map_pkg.update(point(5, odd_def, 50.0))

    # A point that spawns something other than a vending machine.
    map_pkg.update(point(6, 'GD_Population_Other.Thing', 60.0))

    return {'Map_P': map_pkg, 'Startup': startup}


def attr_init(path):
    """An AttributeInitializationData whose base is an attribute."""
    return {'BaseValueConstant': 1, 'BaseValueAttribute': ref(path), 'InitializationDefinition': {'index': 0, 'path': None},
            'BaseValueScaleConstant': 1}


def def_init(path):
    """An AttributeInitializationData that runs an AttributeInitializationDefinition."""
    return {'InitializationDefinition': ref(path)}


def formula(multiplier, level, power):
    """An AttributeInitializationDefinition: Multiplier * Level ^ Power (no offset)."""
    return ('Engine.AttributeInitializationDefinition',
            {'ValueFormula': {'bEnabled': True, 'Multiplier': multiplier, 'Level': level, 'Power': power}})


def constant_attribute(path, value):
    """An attribute with a ConstantAttributeValueResolver subobject (cooked attributes omit ValueResolverChain)."""
    return {path: ('Engine.AttributeDefinition', {}),
            f'{path}.ConstantAttributeValueResolver_0': ('Engine.ConstantAttributeValueResolver', {'ConstantValue': value})}


def property_attribute(path, name):
    """An attribute read from an object property (ExpLevel, MonetaryValueModifierTotal)."""
    resolver = f'{path}.ObjectPropertyAttributeValueResolver_0'
    return {path: ('Engine.AttributeDefinition', {'ValueResolverChain': [ref(resolver)]}),
            resolver: ('Engine.ObjectPropertyAttributeValueResolver', {'PropertyName': name})}


def item_packages():
    """Invented Startup objects for the non-weapon stock: a shield, an ammo item, a health item and a class mod.

    Price calculators have the packages' shape with invented constants: scaler 1.5, shield base cost 10 and a part
    modifier 0.5, ammo base cost 2, class mod base costs 4 (Alpha) and 6 (Beta).
    """
    objects = {}
    for path, value in [('Econ.Scaler', 1.5), ('Econ.Cost_Shield', 10.0), ('Econ.Cost_Ammo', 2.0),
                        ('Econ.Cost_Alpha', 4.0), ('Econ.Cost_Beta', 6.0), ('Econ.Rarity_Half', 0.5),
                        ('Ammo.Amount_Shop', 40.0)]:
        objects.update(constant_attribute(path, value))
    objects.update(property_attribute('Attr.Total', 'MonetaryValueModifierTotal'))
    objects.update(property_attribute('Attr.ItemLevel', 'ExpLevel'))
    one = const(1)
    objects['Econ.Init_Universal'] = formula(attr_init('Attr.Total'), attr_init('Econ.Scaler'), attr_init('Attr.ItemLevel'))
    objects['Econ.Init_Shield'] = formula(def_init('Econ.Init_Universal'), attr_init('Econ.Cost_Shield'), one)
    objects['Econ.Init_Ammo'] = formula(attr_init('Econ.Cost_Ammo'), attr_init('Econ.Scaler'), attr_init('Attr.ItemLevel'))
    for name in ('Alpha', 'Beta'):
        objects[f'Econ.Init_{name}'] = formula(def_init('Econ.Init_Universal'), attr_init(f'Econ.Cost_{name}'), one)
    # Shield: one material part with a monetary modifier, from the definition's own part list.
    objects['Parts.Material_Half'] = ('WillowGame.ShieldPartDefinition', {'MonetaryValueMod': ref('Econ.Rarity_Half')})
    objects['Parts.List_Material'] = ('WillowGame.ItemPartListDefinition', {
        'WeightedParts': [{'Part': ref('Parts.Material_Half'), 'Manufacturers': [], 'MinGameStageIndex': 0,
                           'MaxGameStageIndex': 1, 'DefaultWeightIndex': 0}],
        'ConsolidatedAttributeInitData': [const(1), const(100)]})
    objects['Items.Shield_Def'] = ('WillowGame.ShieldDefinition', {
        'ItemName': 'Test Shield', 'MaterialParts': ref('Parts.List_Material'), 'BaseMonetaryValueModifier': 1,
        'MonetaryValue': def_init('Econ.Init_Shield')})
    objects['Grades.Shield'] = ('WillowGame.InventoryBalanceDefinition', {'InventoryDefinition': ref('Items.Shield_Def')})
    # Ammo: the pickup effect adds the shop amount attribute to an ammo pool attribute.
    objects['Attr.Ammo_SMG_Current'] = ('WillowGame.ResourcePoolAttributeDefinition',
                                        {'Resource': ref('D_Resources.AmmoResources.Ammo_Patrol_SMG')})
    objects['Items.Ammo_Def'] = ('WillowGame.UsableItemDefinition', {
        'ItemName': 'Test Ammo', 'MonetaryValue': def_init('Econ.Init_Ammo'),
        'ExternalAttributeEffects': [{'AttributeToModify': ref('Attr.Ammo_SMG_Current'), 'ModifierType': 'MT_PostAdd',
                                      'BaseModifierValue': attr_init('Ammo.Amount_Shop')}]})
    objects['GD_Items.Ammo.Grade'] = ('WillowGame.InventoryBalanceDefinition', {'InventoryDefinition': ref('Items.Ammo_Def')})
    # Health: a multiply behaviour under the item's behaviour provider.
    objects['Items.Health_Def'] = ('WillowGame.UsableItemDefinition', {
        'ItemName': 'Test Health', 'MonetaryValue': def_init('Econ.Init_Ammo'),
        'BehaviorProviderDefinition': ref('Items.Health_Def.BPD_0')})
    objects['Items.Health_Def.BPD_0'] = ('GearboxFramework.BehaviorProviderDefinition', {})
    objects['Items.Health_Def.BPD_0.Behavior_SimpleMath_0'] = ('WillowGame.Behavior_SimpleMath',
                                                               {'Operation': 'BINARYMATH_Mul', 'B': 0.3})
    objects['GD_Items.Health.Grade'] = ('WillowGame.InventoryBalanceDefinition',
                                        {'InventoryDefinition': ref('Items.Health_Def')})
    # Class mod: no pool lists it; two definitions with their own calculators.
    for name in ('Alpha', 'Beta'):
        objects[f'ClassMods.ClassMod_Test_{name}'] = ('WillowGame.ClassModDefinition', {
            'BaseMonetaryValueModifier': 1, 'MonetaryValue': def_init(f'Econ.Init_{name}')})
    objects['GD_ItemGrades.ClassMods.BalDef_ClassMod_Test_01_Common'] = ('WillowGame.ClassModBalanceDefinition', {
        'ClassModDefinitions': [ref('ClassMods.ClassMod_Test_Alpha'), ref('ClassMods.ClassMod_Test_Beta')]})
    return objects


def item_document(oracle=None):
    """build_document over machine_packages() with item_packages() in Startup (the ammo and health leaves gain items)."""
    packages = machine_packages()
    packages['Startup'].update(item_packages())
    packages['Map_P'].update(pool('GD_ItemPools_Shop.Health.Stock', {'GD_Items.Health.Grade': 1.0, 'Grades.Shield': 1.0}))
    return pv.build_document(FakeReader(packages, tempfile.gettempdir()), 'Map_P', 8.0, oracle)


def leaf_items(doc, kind):
    """{balance: leaf} of the first machine of a kind."""
    machine = next(m for m in doc['machines'] if m['kind'] == kind)
    return {leaf['balance']: leaf for row in machine['inventory'] for leaf in row['items']}


class NonWeaponStockTest(unittest.TestCase):
    def test_shield_value_uses_part_total_scaler_and_base_cost(self):
        item = leaf_items(item_document(), 'health')['Grades.Shield']['item']
        self.assertEqual(item['kind'], 'shield')
        self.assertEqual(item['name'], 'Test Shield')
        value = item['value']
        self.assertEqual(value['base_cost'], 10.0)
        self.assertEqual(value['scaler'], 1.5)
        self.assertEqual(value['part_totals'], [0.5])
        self.assertTrue(value['shape_checked'])
        # 0.5 * 1.5 ^ 2 * 10 = 11.25 -> 11
        chain = pv.Chain(FakeReader({'Startup': item_packages()}, tempfile.gettempdir()), ['Startup'])
        self.assertEqual(pv.value_at(chain, value, 2, 0.5), 11)

    def test_ammo_item_names_pool_type_and_amount(self):
        item = leaf_items(item_document(), 'items')['GD_Items.Ammo.Grade']['item']
        self.assertEqual(item['kind'], 'ammo')
        self.assertEqual(item['ammo_pool'], 'D_Resources.AmmoResources.Ammo_Patrol_SMG')
        self.assertEqual(item['ammo_type'], 'smg')
        self.assertEqual(item['amount'], 40.0)
        self.assertIsNone(item['value']['part_total_attribute'])
        self.assertEqual(item['value']['part_totals'], [1.0])

    def test_health_item_heals_a_fraction(self):
        item = leaf_items(item_document(), 'health')['GD_Items.Health.Grade']['item']
        self.assertEqual(item['kind'], 'health')
        self.assertAlmostEqual(item['heal']['fraction_of_max_health'], 0.3)

    def test_class_mods_are_unpooled_with_variants(self):
        doc = item_document()
        unpooled = {u['balance']: u for u in doc['unpooled_items']}
        item = unpooled['GD_ItemGrades.ClassMods.BalDef_ClassMod_Test_01_Common']['item']
        self.assertEqual(item['kind'], 'class_mod')
        self.assertIsNone(item['value'])
        self.assertEqual([v['key'] for v in item['variants']], ['Alpha', 'Beta'])
        self.assertEqual([v['value']['base_cost'] for v in item['variants']], [4.0, 6.0])

    def test_oracle_counts_exact_matches_and_lists_misses(self):
        # Shield at level 2: 11 (match) and 12 (miss); class mod Beta at level 2: 0.5 is not applied (no parts), so
        # 1 * 1.5 ^ 2 * 6 = 13.5 -> 13.
        oracle = {'machines': [{'stock': [
            {'class': 'WillowShield', 'balance': 'Grades.Shield', 'exp_level': 2, 'monetary_value': 11, 'name': 'A'},
            {'class': 'WillowShield', 'balance': 'Grades.Shield', 'exp_level': 2, 'monetary_value': 12, 'name': 'B'},
            {'class': 'WillowWeapon', 'balance': 'GD_Weapons.SMG.Balance', 'exp_level': 2, 'monetary_value': 5},
            {'class': '<error>', 'balance': 'x'},
            {'class': 'WillowClassMod', 'balance': 'GD_ItemGrades.ClassMods.BalDef_ClassMod_Test_01_Common',
             'exp_level': 2, 'monetary_value': 13, 'name': 'Beta Class Mod'},
            {'class': 'WillowClassMod', 'balance': 'GD_DLC.ClassMods.Unknown', 'exp_level': 2, 'monetary_value': 1,
             'name': 'Other Class Mod'}],
            'featured': None}]}
        doc = item_document(oracle)
        shield = leaf_items(doc, 'health')['Grades.Shield']['item']
        self.assertEqual(shield['verified'], {'value': '1 of 2 live rows'})
        mod = doc['unpooled_items'][0]['item']
        self.assertEqual(mod['verified'], {'value': '1 of 1 live rows'})
        ammo = leaf_items(doc, 'items')['GD_Items.Ammo.Grade']['item']
        self.assertTrue(ammo['verified']['value'].startswith('UNVERIFIED'))
        misses = [u for u in doc['unresolved'] if u.get('balance') == 'Grades.Shield']
        self.assertEqual(len(misses), 1)
        self.assertIn('live value 12', misses[0]['reason'])
        self.assertIn('GD_DLC.ClassMods.Unknown', ' '.join(doc['notes']))

    def test_without_oracle_items_are_unverified(self):
        doc = item_document()
        item = leaf_items(doc, 'items')['GD_Items.Ammo.Grade']['item']
        self.assertIn('UNVERIFIED', item['verified']['value'])
        json.dumps(doc)  # must be plain JSON


class PrepareVendorsTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.cooked = Path(self.tmp.name)

    def document(self, ammo_probability=1.0):
        reader = FakeReader(machine_packages(ammo_probability), self.cooked)
        return pv.build_document(reader, 'Map_P', 8.0)

    def test_kind_mapping_and_skipped_points(self):
        doc = self.document()
        kinds = {m['id'].rsplit(':', 1)[1]: m['kind'] for m in doc['machines']}
        self.assertEqual(kinds, {'WillowPopulationOpportunityPoint_1': 'weapons',
                                 'WillowPopulationOpportunityPoint_2': 'items',
                                 'WillowPopulationOpportunityPoint_3': 'health',
                                 'WillowPopulationOpportunityPoint_4': 'blackmarket'})
        self.assertFalse([m for m in doc['machines'] if m['kind'] == 'blackmarket'][0]['supported'])
        self.assertTrue(all(m['supported'] for m in doc['machines'] if m['kind'] != 'blackmarket'))
        reasons = [u['reason'] for u in doc['unresolved'] if u.get('machine', '').endswith('_5')]
        self.assertEqual(len(reasons), 1)
        self.assertIn('unknown ShopType SType_Mystery', reasons[0])

    def test_two_package_fallback_resolves_startup_balances(self):
        doc = self.document()
        weapons = doc['machines'][0]
        leaves = {leaf['balance']: leaf for leaf in weapons['inventory'][0]['items']}
        self.assertEqual(set(leaves), {'GD_Weapons.Pistol.Balance', 'GD_Weapons.SMG.Balance'})
        self.assertEqual(leaves['GD_Weapons.SMG.Balance']['class'], 'WeaponBalanceDefinition')

    def test_shares_are_weight_fractions(self):
        doc = self.document()
        leaves = {leaf['balance']: leaf['weight'] for leaf in doc['machines'][0]['inventory'][0]['items']}
        self.assertAlmostEqual(leaves['GD_Weapons.Pistol.Balance'], 0.25)
        self.assertAlmostEqual(leaves['GD_Weapons.SMG.Balance'], 0.75)

    def test_leaf_classes_kept_and_featured_listed(self):
        doc = self.document()
        weapons = doc['machines'][0]
        self.assertEqual(weapons['featured'][0]['pool'], 'GD_ItemPools_Shop.Featured.Gun')
        self.assertEqual(weapons['featured'][0]['items'][0]['class'], 'WeaponBalanceDefinition')
        ammo = doc['machines'][1]
        self.assertEqual(ammo['inventory'][0]['items'][0]['class'], 'InventoryBalanceDefinition')

    def test_definition_and_header_fallbacks(self):
        doc = self.document()
        by_kind = {m['kind']: m for m in doc['machines']}
        self.assertEqual(by_kind['weapons']['definition'], 'GD_Balance_Shopping.Defs.Weapons')
        self.assertEqual(by_kind['weapons']['title'], 'Marcus Munitions')
        self.assertEqual(by_kind['health']['definition'], 'GD_Balance_Shopping.Defs.Health')
        self.assertEqual(by_kind['health']['header'], 'Test Health Vending Machine')
        self.assertEqual(by_kind['blackmarket']['title'], 'Test Black Market')

    def test_unresolved_probability_is_null_and_listed(self):
        doc = self.document(ammo_probability=None)
        ammo = doc['machines'][1]
        self.assertIsNone(ammo['inventory'][0]['probability'])
        pool_notes = [u for u in doc['unresolved'] if u.get('pool') == 'GD_ItemPools_Shop.Ammo.Stock']
        self.assertTrue(any('PoolProbability' in u['reason'] for u in pool_notes))
        self.assertIn('GD_Balance_Shopping.Att_Probability', ' '.join(doc['notes']))

    def test_markup_is_null_without_runtime_values(self):
        doc = self.document()
        self.assertIsNone(doc['machines'][0]['markup'])
        self.assertIsNone(doc['machines'][0]['featured_markup'])
        self.assertEqual(doc['machines'][0]['fixed_item_cost'], 0)

    def test_document_shape(self):
        doc = self.document()
        self.assertEqual(doc['schema'], 'openwillow-vendors/1')
        self.assertEqual(doc['map'], 'Map_P')
        self.assertEqual(doc['stage'], 8.0)
        self.assertEqual(doc['minutes_between_resets'], 20)
        machine = doc['machines'][0]
        self.assertEqual(set(machine), {'id', 'kind', 'title', 'header', 'definition', 'location', 'rotation', 'rotation_units',
                                        'fixed_item_cost', 'fixed_featured_cost', 'markup', 'featured_markup',
                                        'inventory', 'featured', 'supported'})
        self.assertEqual(machine['location'], [10.0, 2.0, 3.0])
        self.assertEqual(machine['rotation'], [0, 90.0, 0])
        self.assertEqual(machine['rotation_units'], [0, 16384, 0])
        self.assertEqual(set(machine['inventory'][0]), {'pool', 'probability', 'items'})
        self.assertEqual(set(machine['inventory'][0]['items'][0]), {'balance', 'weight', 'class'})
        json.dumps(doc)  # must be plain JSON

    def test_output_refused_outside_local(self):
        repo = self.cooked / 'repo'
        (repo / 'local').mkdir(parents=True)
        self.assertEqual(pv.local_path('local/slice/vendors.json', repo),
                         (repo / 'local' / 'slice' / 'vendors.json').resolve())
        for bad in ['vendors.json', 'local', '../escape.json', 'docs/vendors.json', 'local/../vendors.json']:
            with self.subTest(bad=bad), self.assertRaises(ValueError):
                pv.local_path(bad, repo)
        with self.assertRaises(ValueError):
            pv.local_path(str(repo / 'elsewhere.json'), repo)


if __name__ == '__main__':
    unittest.main()
