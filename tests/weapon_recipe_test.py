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
        'Weight.Common': {'ValueFormula': {'bEnabled': True, 'Multiplier': const(100), 'Level': const(1), 'Power': const(1)},
                          'RangeRestriction': {'MinValue': const(100)}},
        # Like GD_Balance.Weighting.Weight_2_Uncommon: min 100 present but disabled.
        'Weight.Uncommon': {'ValueFormula': {'bEnabled': True, 'Multiplier': const(10), 'Level': const(1), 'Power': const(1),
                                             'Offset': const(0)},
                            'RangeRestriction': {'MinValue': const(100)}},
        # Invented values: 2 x (3^2 + 1) = 20 under the native order, 19 with the offset outside.
        'Weight.Offset': {'ValueFormula': {'bEnabled': True, 'Multiplier': const(2), 'Level': const(3), 'Power': const(2),
                                           'Offset': const(1)}},
        'Weight.Clamped': {'ValueFormula': {'bEnabled': True, 'Multiplier': const(10), 'Level': const(1), 'Power': const(1)},
                           'RangeRestriction': {'bEnableMinValueRestriction': True, 'MinValue': const(100)}},
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
    def test_formula_weight_uses_multiplier_level_power(self):
        package = FakePackage(world())
        self.assertEqual(w.attribute_value(package, const(0, 'Weight.Common'), 30), 100)

    def test_offset_is_added_before_the_multiplier(self):
        package = FakePackage(world())
        self.assertEqual(w.attribute_value(package, const(0, 'Weight.Offset'), 30), 20)
        self.assertEqual(w.formula_value(2, 3, 2, 1), 20)
        self.assertEqual(w.formula_value(1, 3, 2, 1), 10)  # Multiplier 1: both orders agree

    def test_range_restriction_applies_only_when_enabled(self):
        package = FakePackage(world())
        self.assertEqual(w.attribute_value(package, const(0, 'Weight.Uncommon'), 30), 10)
        self.assertEqual(w.attribute_value(package, const(0, 'Weight.Clamped'), 30), 100)

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

    def test_stage_none_lists_every_stage_and_merge_modes(self):
        merged = w.merge(FakePackage(world()), 'Leaf', None)
        self.assertEqual(merged['chain'], ['Root', 'Leaf'])
        self.assertEqual([p for p, _ in merged['merged']['Grip']], ['Grip.A', 'Grip.Late'])
        base = {'Body': [('A', 1)], 'Grip': [('G', 1)]}
        self.assertEqual(w.merge_slots(base, {'Body': [('B', 1)]}, 'EPRM_Additive')['Body'], [('A', 1), ('B', 1)])
        self.assertEqual(w.merge_slots(base, {'Body': [('B', 1)]}, 'EPRM_Selective'),
                         {'Body': [('B', 1)], 'Grip': [('G', 1)]})
        self.assertEqual(w.merge_slots(base, {'Body': [('B', 1)]}, 'EPRM_Complete'), {'Body': [('B', 1)]})
        self.assertEqual(base['Body'], [('A', 1)])  # inputs are not modified

    def test_entry_without_manufacturer_list_weighs_flat_100(self):
        # Body.Unique's DefaultWeightIndex points at 0, but it has no Manufacturers list.
        recipe = w.roll(FakePackage(world()), 'Leaf', seed=1, stage=2)
        self.assertEqual(recipe['parts']['Body']['weight'], w.FLAT_WEIGHT)
        self.assertEqual(recipe['gestalt_fragments'], ['Frag_Body', 'Frag_Grip'])

    def test_manufacturer_list_weights_and_none_is_not_a_wildcard(self):
        stage = [const(1), const(100), const(0), const(7), const(3)]

        def at(name, index=None):
            return stage[{'DefaultWeight': 2}[name] if index is None else index]
        own = {'Manufacturers': [{'Manufacturer': 'Makers.Other', 'DefaultWeightIndex': 4},
                                 {'Manufacturer': 'Makers.Vladof', 'DefaultWeightIndex': 3}]}
        none_only = {'Manufacturers': [{'Manufacturer': None, 'DefaultWeightIndex': 3}]}
        weight = lambda part, maker: w.entry_weight(part, maker, lambda n, i=None: at(n, i)['BaseValueConstant'])
        self.assertEqual(weight(own, 'Makers.Vladof'), 7)
        self.assertEqual(weight(own, 'Makers.Bandit'), 0)     # not listed: DefaultWeightIndex
        self.assertEqual(weight(none_only, 'Makers.Vladof'), 0)
        self.assertEqual(weight(own, None), w.FLAT_WEIGHT)    # no manufacturer known
        self.assertEqual(weight({}, 'Makers.Vladof'), w.FLAT_WEIGHT)

    def test_stage_window_uses_truncated_bounds(self):
        objects = world()
        objects['Root.List']['ConsolidatedAttributeInitData'] = [const(4.9), const(100), const(0), const(0), const(50)]
        objects['Root.List']['GripPartData']['WeightedParts'] = [weighted('Grip.A', 1)]
        slots, _ = w.slot_candidates(FakePackage(objects), 'Root.List', 4)
        self.assertEqual(slots['Grip'], [('Grip.A', w.FLAT_WEIGHT)])  # trunc(4.9) = 4 <= stage 4
        slots, _ = w.slot_candidates(FakePackage(objects), 'Root.List', 3)
        self.assertEqual(slots['Grip'], [])

    def test_pick_walks_running_intervals(self):
        class Fixed:
            def __init__(self, value):
                self.value = value

            def randint(self, low, high):
                return self.value
        entries = [('A', 0.0), ('B', 1.0), ('C', 3.0), ('B', 2.0), ('C', 0.0)]
        # Zero weights are skipped (a zero repeat does not remove C); B keeps its later weight 2.
        part, candidates = w.pick(entries, Fixed(0))
        self.assertEqual((part, candidates), ('B', {'B': 2.0, 'C': 3.0}))
        self.assertEqual(w.pick(entries, Fixed(32767))[0], 'C')
        self.assertEqual(w.pick(entries, Fixed(13106))[0], 'B')   # r = 5 * 13106 / 32767, just under 2.0
        self.assertEqual(w.pick(entries, Fixed(13107))[0], 'C')   # just over 2.0
        self.assertEqual(w.pick([('A', 0.0)], Fixed(0)), (None, {}))

    def test_zero_weight_slot_is_left_empty(self):
        objects = world()
        objects['Root.List']['GripPartData']['WeightedParts'] = [
            dict(weighted('Grip.A', 2), Manufacturers=[{'Manufacturer': 'Makers.Vladof', 'DefaultWeightIndex': 2}])]
        recipe = w.roll(FakePackage(objects), 'Leaf', seed=1, stage=2)
        self.assertNotIn('Grip', recipe['parts'])
        self.assertTrue(any(n.startswith('Grip: every candidate weighs 0') for n in recipe['notes']))

    def test_name_parts_type_first_later_part_wins_ties(self):
        objects = world()
        objects.update({
            'Name.TypePrefix': {'PartName': 'Typed', 'Priority': 2},
            'Name.Tie': {'PartName': 'Tied'},                      # Priority omitted: class default 1
            'Name.Late': {'PartName': 'Too Late', 'Priority': 9, 'MinExpLevelRequirement': 20},
            'Name.Zero': {'PartName': 'Never', 'Priority': 0},
        })
        objects['Type']['PrefixList'] = ['Name.TypePrefix']
        objects['Body.A']['PrefixList'] = ['Name.Late', 'Name.Zero']
        objects['Grip.A']['PrefixList'] = ['Name.Tie']
        package = FakePackage(objects)
        parts = {'Body': 'Body.A', 'Grip': 'Grip.A'}
        prefix, title = w.choose_name_parts(package, 'Type', parts, 'Makers.Vladof', 5)
        self.assertEqual((prefix, title), ('Name.TypePrefix', 'Name.TypeTitle'))  # 2 beats the default 1
        objects['Type']['PrefixList'] = ['Name.Generic']                         # 0.2 < 1: the grip's wins
        self.assertEqual(w.choose_name_parts(package, 'Type', parts, 'Makers.Vladof', 5)[0], 'Name.Tie')
        self.assertEqual(w.choose_name_parts(package, 'Type', parts, 'Makers.Vladof', 20)[0], 'Name.Late')
        objects['Name.Generic']['Priority'] = 1                                  # tie: the later list wins
        self.assertEqual(w.choose_name_parts(package, 'Type', parts, 'Makers.Vladof', 5)[0], 'Name.Tie')

    def test_evaluator_order_mode_scale_clamp_round(self):
        objects = world()
        objects['Def.Scales'] = {'BaseValueMode': 'BASEVALUE_InitializationDefScalesBaseValue',
                                 'ValueFormula': {'bEnabled': True, 'Multiplier': const(3), 'Level': const(2),
                                                  'Power': const(1)},
                                 'RangeRestriction': {'bEnableMaxValueRestriction': True, 'MaxValue': const(50)},
                                 'RoundingMode': 'ATTRROUNDING_IntCeil'}
        package = FakePackage(objects)
        init = {'BaseValueConstant': 2.5, 'InitializationDefinition': 'Def.Scales', 'BaseValueScaleConstant': 1.1}
        # 2.5 * (3 * 2) = 15, * 1.1 = 16.5, under the max 50, ceil -> 17.
        self.assertEqual(w.attribute_value(package, init, 1), 17)
        init['BaseValueScaleConstant'] = 10.0  # 150 clamps to 50 after the scale
        self.assertEqual(w.attribute_value(package, init, 1), 50)
        self.assertEqual(w.f32(0.1), 0.10000000149011612)


if __name__ == '__main__':
    unittest.main()
