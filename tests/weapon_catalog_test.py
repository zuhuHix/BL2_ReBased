"""Synthetic test for tools/weapon_catalog.py: classification, slot and manufacturer choice, pool reach, coverage diff.

Invented names and values only (no game data). Run directly: python tests/weapon_catalog_test.py
"""
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import weapon_catalog as C  # noqa: E402
import weapon_recipe  # noqa: E402


class FakePackage:
    """The two members weapon_recipe.part_fragments reads: props() and extra_names."""

    def __init__(self, props, extra):
        self._props, self.extra_names = props, extra

    def props(self, path):
        return self._props[path]


class Classification(unittest.TestCase):
    def test_balance_subclasses_and_exact_classes(self):
        self.assertEqual(C.category('WillowGame.WeaponBalanceDefinition'), 'balance')
        self.assertEqual(C.category('WillowGame.MissionWeaponBalanceDefinition'), 'balance')
        self.assertEqual(C.category('WeaponBalanceDefinition'), 'balance')  # OpenBLCMM uses unqualified names
        self.assertEqual(C.category('WillowGame.WeaponPartDefinition'), 'part')
        self.assertEqual(C.category('WillowGame.ShieldPartDefinition'), None)  # not a weapon part
        self.assertEqual(C.category('WillowGame.BuzzaxeWeaponTypeDefinition'), 'weapon_type')
        self.assertEqual(C.category('Engine.ManufacturerDefinition'), 'manufacturer')
        self.assertEqual(C.category('GearboxFramework.GestaltSkeletalMeshDefinition'), 'gestalt')
        self.assertEqual(C.category('WillowGame.WeaponPartListDefinition'), 'part_list_definition')

    def test_pool_suffixes_do_not_collide(self):
        self.assertEqual(C.category('WillowGame.ItemPoolListDefinition'), 'item_pool_list')
        self.assertEqual(C.category('WillowGame.ItemPoolDefinition'), 'item_pool')
        self.assertEqual(C.category('WillowGame.KeyedItemPoolDefinition'), 'item_pool')
        self.assertEqual(C.category('WillowGame.CrossDLCItemPoolDefinition'), 'item_pool')
        self.assertEqual(C.category('WillowGame.WeaponNamePartDefinition'), 'name_part')
        self.assertEqual(C.category('WillowGame.WeaponPartListCollectionDefinition'), 'part_list')


class Slots(unittest.TestCase):
    def test_part_type_and_group(self):
        self.assertEqual(C.slot_from_part_type('WP_Barrel'), 'Barrel')
        self.assertEqual(C.slot_from_part_type('WP_Accessory2'), 'Accessory')
        self.assertIsNone(C.slot_from_part_type('WP_Unknown'))
        self.assertIsNone(C.slot_from_part_type(None))

    def test_path_segment(self):
        self.assertEqual(C.slot_from_path('GD_Weap_Synth.Stock.SYN_Stock_Alpha'), 'Stock')
        self.assertIsNone(C.slot_from_path('GD_Weap_Synth.Misc.SYN_Thing'))
        self.assertIsNone(C.slot_from_path('SYN_Thing'))

    def test_choose_slot_prefers_the_single_list(self):
        self.assertEqual(C.choose_slot({'Sight'}, 'Barrel', 'Stock'), ('Sight', 'part_list'))

    def test_choose_slot_falls_back_in_order(self):
        self.assertEqual(C.choose_slot(set(), 'Barrel', 'Stock'), ('Barrel', 'part_type'))
        self.assertEqual(C.choose_slot(set(), None, 'Stock'), ('Stock', 'path'))
        self.assertEqual(C.choose_slot({'Grip', 'Body'}, 'Barrel', 'Stock'), (None, 'unknown'))
        self.assertEqual(C.choose_slot({'Grip', 'Body'}, 'Grip', None), ('Grip', 'part_type'))

    def test_accessory_groups_merge(self):
        self.assertEqual(C.slot_group('Accessory1'), 'Accessory')
        self.assertEqual(C.slot_group('Accessory2'), 'Accessory')
        self.assertEqual(C.slot_group('Body'), 'Body')


class Manufacturer(unittest.TestCase):
    def test_single_listing_manufacturer(self):
        self.assertEqual(C.choose_manufacturer(['Acme', 'Acme', None], None), ('Acme', 'balances', ['Acme']))

    def test_scope_field_only_when_nothing_lists_the_part(self):
        self.assertEqual(C.choose_manufacturer([], 'Acme'), ('Acme', 'scope_movie_field', []))
        self.assertEqual(C.choose_manufacturer(['Acme'], 'Other'), ('Acme', 'balances', ['Acme']))

    def test_ambiguous_and_unknown(self):
        self.assertEqual(C.choose_manufacturer(['Acme', 'Other'], None), (None, 'ambiguous', ['Acme', 'Other']))
        self.assertEqual(C.choose_manufacturer([], None), (None, 'unknown', []))


class Fragments(unittest.TestCase):
    def test_none_parts_draw_nothing(self):
        package = FakePackage({'GD_X.Sight.SYN_Sight_None': {'GestaltModeSkeletalMeshName': 'SYN_Sight_None'}}, {})
        self.assertEqual(weapon_recipe.part_fragments(package, 'GD_X.Sight.SYN_Sight_None'), [])

    def test_primary_and_additional_names_in_order(self):
        path = 'GD_X.Body.SYN_Body_A'
        package = FakePackage({path: {'GestaltModeSkeletalMeshName': 'SYN_Body_A'}},
                              {path: {'AdditionalGestaltModeSkeletalMeshNames': ['SYN_Body_A_Var1', 'None']}})
        self.assertEqual(weapon_recipe.part_fragments(package, path), ['SYN_Body_A', 'SYN_Body_A_Var1'])


class Pools(unittest.TestCase):
    def test_sub_pools_are_followed_and_cycles_stop(self):
        children = {'P_Top': ['P_Sub', 'SYN_Balance_A'], 'P_Sub': ['SYN_Balance_B', 'P_Top'], 'P_Other': ['SYN_Ghost']}
        balances = {'SYN_Balance_A', 'SYN_Balance_B'}
        self.assertEqual(C.reach_balances('P_Top', children, balances), {'SYN_Balance_A', 'SYN_Balance_B'})
        self.assertEqual(C.reach_balances('P_Other', children, balances), set())

    def test_pool_children_read_both_array_shapes(self):
        props = {'BalancedItems': [{'InvBalanceDefinition': 'SYN_B'}, {'ItmPoolDefinition': 'P_Sub'}, {}],
                 'ItemPools': [{'ItemPool': 'P_List'}]}
        self.assertEqual(C.pool_children(props), ['SYN_B', 'P_Sub', 'P_List'])


class Coverage(unittest.TestCase):
    def test_compare_reports_both_directions(self):
        result = C.compare({'a', 'b', 'c'}, {'b', 'd'})
        self.assertEqual(result, {'first': 3, 'second': 2, 'both': 1,
                                  'only_first': ['a', 'c'], 'only_second': ['d']})

    def test_unlisted_parts(self):
        self.assertEqual(C.unlisted(['p1', 'p2', 'p3'], {'p2'}), ['p1', 'p3'])

    def test_material_instance_names(self):
        self.assertTrue(C.is_material_instance('Engine.MaterialInstanceConstant'))
        self.assertFalse(C.is_material_instance('WillowGame.WeaponPartDefinition'))

    def test_name_kind(self):
        self.assertEqual(C.name_kind('GD_X.Name.Prefix.Prefix__Acme'), 'prefix')
        self.assertEqual(C.name_kind('GD_X.Name.Title.Title_Acme'), 'title')
        self.assertEqual(C.name_kind('GD_X.Name.Misc.Thing'), 'other')


if __name__ == '__main__':
    unittest.main()
