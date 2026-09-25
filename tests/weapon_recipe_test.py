"""Synthetic tests for tools/weapon_recipe.py part merging, weights and names."""
from pathlib import Path
import sys
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import weapon_recipe as w


def const(value, init=None):
    return {'BaseValueConstant': value, 'BaseValueAttribute': None,
            'InitializationDefinition': init, 'BaseValueScaleConstant': 1.0}


def weighted(part, weight_index, low=0, high=1):
    return {'Part': part, 'MinGameStageIndex': low, 'MaxGameStageIndex': high, 'DefaultWeightIndex': weight_index}


class FakePackage:
    """Only the props() lookup that roll() uses; no package bytes."""
    def __init__(self, objects):
        self.objects = objects

    def props(self, path):
        return self.objects[path]


def world():
    stage = [const(1), const(100), const(0), const(0, 'Weight.Common'), const(50)]
    return {
        'Weight.Common': {'ValueFormula': {'Multiplier': const(100), 'Level': const(1), 'Power': const(1)},
                          'RangeRestriction': {'MinValue': const(100)}},
        'Type': {'GestaltMesh': 'Gestalt', 'TitleList': ['Name.TypeTitle']},
        'Root': {'WeaponPartListCollection': 'Root.List', 'InventoryDefinition': 'Type',
                 'Manufacturers': [{'Manufacturer': 'Makers.Vladof'}]},
        'Root.List': {'ConsolidatedAttributeInitData': stage, 'BodyPartData': {
            'bEnabled': True, 'WeightedParts': [weighted('Body.A', 2)]}, 'GripPartData': {
            'bEnabled': True, 'WeightedParts': [weighted('Grip.A', 3), weighted('Grip.Late', 3, low=4)]}},
        'Leaf': {'WeaponPartListCollection': 'Leaf.List', 'BaseDefinition': 'Root'},
        'Leaf.List': {'PartReplacementMode': 'EPRM_Selective', 'ConsolidatedAttributeInitData': stage,
                      'BodyPartData': {'bEnabled': True, 'WeightedParts': [weighted('Body.Unique', 2)]},
                      'GripPartData': {'bEnabled': False, 'WeightedParts': [weighted('Grip.Ignored', 2)]}},
        'Body.A': {'GestaltModeSkeletalMeshName': 'Frag_Body'},
        'Body.Unique': {'GestaltModeSkeletalMeshName': 'Frag_Body', 'TitleList': ['Name.Unique'],
                        'PrefixList': ['Name.Generic', 'Name.ForBandit', 'Name.ForVladof']},
        'Grip.A': {'GestaltModeSkeletalMeshName': 'Frag_Grip'},
        'Grip.Late': {'GestaltModeSkeletalMeshName': 'Frag_Late'},
        'Name.TypeTitle': {'PartName': 'Pistol', 'Priority': 1},
        'Name.Unique': {'PartName': 'Unique', 'Priority': 10},
        'Name.Generic': {'PartName': 'Plain', 'Priority': 0.2},
        'Name.ForBandit': {'PartName': 'Bandit Word', 'Priority': 0.6, 'Expressions': [
            {'AttributeOperand1': 'Attr.Weapon_Is_Bandit', 'ComparisonOperator': 'OPERATOR_EqualTo',
             'AttributeOperand2': None, 'ConstantOperand2': 1}]},
        'Name.ForVladof': {'PartName': 'Vladof Word', 'Priority': 0.5, 'Expressions': [
            {'AttributeOperand1': 'Attr.Weapon_Is_Vladof', 'ComparisonOperator': 'OPERATOR_EqualTo',
             'AttributeOperand2': None, 'ConstantOperand2': 1}]},
    }


class WeaponRecipeTests(unittest.TestCase):
    def test_formula_weight_uses_multiplier_level_power_and_minimum(self):
        package = FakePackage(world())
        self.assertEqual(w.attribute_value(package, const(0, 'Weight.Common'), 30), 100)

    def test_selective_replaces_only_enabled_slots(self):
        recipe = w.roll(FakePackage(world()), 'Leaf', seed=1, stage=2)
        self.assertEqual(recipe['parts']['Body']['part'], 'Body.Unique')
        self.assertEqual(recipe['parts']['Grip']['part'], 'Grip.A')

    def test_game_stage_filters_candidates(self):
        early = w.roll(FakePackage(world()), 'Leaf', seed=1, stage=2)
        self.assertEqual([c['part'] for c in early['parts']['Grip']['candidates']], ['Grip.A'])
        late = w.roll(FakePackage(world()), 'Leaf', seed=1, stage=60)
        self.assertEqual(len(late['parts']['Grip']['candidates']), 2)

    def test_manufacturer_expressions_gate_name_parts(self):
        recipe = w.roll(FakePackage(world()), 'Leaf', seed=1, stage=2)
        self.assertEqual(recipe['manufacturer'], 'Makers.Vladof')
        self.assertEqual(recipe['name'], 'Vladof Word Unique')

    def test_zero_weight_slot_is_flagged(self):
        recipe = w.roll(FakePackage(world()), 'Leaf', seed=1, stage=2)
        self.assertTrue(any(n.startswith('Body: all candidates weigh 0') for n in recipe['notes']))
        self.assertEqual(recipe['gestalt_fragments'], ['Frag_Body', 'Frag_Grip'])


if __name__ == '__main__':
    unittest.main()
