"""Synthetic tests for tools/loot_pools.py (no game data)."""
from pathlib import Path
import random
import sys
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import loot_pools as L


def const(value, attribute=None, init=None, scale=1.0):
    return {'BaseValueConstant': value, 'BaseValueAttribute': attribute, 'InitializationDefinition': init,
            'BaseValueScaleConstant': scale}


class FakePackage:
    def __init__(self, objects, classes):
        self.objects, self.classes = objects, classes
        self.index = {k: i for i, k in enumerate(objects)}

    def props(self, path):
        return self.objects[path]


def world():
    objects = {
        'W.Common': {'ValueFormula': {'bEnabled': True, 'Multiplier': const(100), 'Level': const(1), 'Power': const(1)}},
        'W.Rare': {'ValueFormula': {'bEnabled': True, 'Multiplier': const(1), 'Level': const(1), 'Power': const(1)},
                   # Cooked data omits false flags; a disabled restriction must not clamp.
                   'RangeRestriction': {'MinValue': const(100)}},
        'W.Mod': {'ValueFormula': {'bEnabled': True, 'Multiplier': const(0, 'D.CommonMod'),
                                   'Level': const(0, init='W.Common'), 'Power': const(1)}},
        'D.CommonMod': {'BaseValue': const(1)},
        'Odds': {}, 'Odds.Cond_0': {'ValueExpressions': {'ConditionalExpressionList': [
            {'BaseValueIfTrue': const(0.5), 'Expressions': [
                {'AttributeOperand1': L.NUMBER_OF_PLAYERS, 'ComparisonOperator': 'OPERATOR_EqualTo',
                 'ConstantOperand2': 1}]},
            {'BaseValueIfTrue': const(0.25), 'Expressions': [
                {'AttributeOperand1': L.NUMBER_OF_PLAYERS, 'ComparisonOperator': 'OPERATOR_GreaterThan',
                 'ConstantOperand2': 1}]}]}},
        'Stage2': {}, 'Stage2.Const_0': {'ConstantValue': 2},
        'Stage9': {}, 'Stage9.Const_0': {'ConstantValue': 9},
        'Pool.Guns': {'BalancedItems': [
            {'ItmPoolDefinition': 'Pool.Common', 'Probability': const(0, init='W.Mod'), 'ProbabilityDisplayString': '0%'},
            {'ItmPoolDefinition': 'Pool.Rare', 'Probability': const(0, init='W.Rare'), 'ProbabilityDisplayString': '100.00%'},
            {'ItmPoolDefinition': 'Pool.Late', 'Probability': const(0, init='W.Rare'), 'ProbabilityDisplayString': '0%'}]},
        'Pool.Common': {'MinGameStageRequirement': 'Stage2', 'BalancedItems': [
            {'InvBalanceDefinition': 'Gun.A', 'Probability': const(3)},
            {'InvBalanceDefinition': 'Gun.B', 'Probability': const(1)}]},
        'Pool.Rare': {'Quantity': const(2), 'BalancedItems': [{'InvBalanceDefinition': 'Gun.R', 'Probability': const(1)}]},
        'Pool.Late': {'MinGameStageRequirement': 'Stage9',
                      'BalancedItems': [{'InvBalanceDefinition': 'Gun.L', 'Probability': const(1)}]},
        'List': {'ItemPools': [{'ItemPool': 'Pool.Guns', 'PoolProbability': const(0, 'Odds')},
                               {'ItemPool': 'Pool.Rare', 'PoolProbability': const(0, 'Unknown.Attr')}]},
        'Pawn': {'DefaultItemPoolIncludedLists': ['List'], 'PlayThroughs': [
            {'CustomItemPoolList': [{'ItemPool': 'Pool.Late', 'PoolProbability': const(1)}]}]},
        'Unknown.Attr': {},
    }
    classes = {k: 'Engine.AttributeDefinition' for k in ('Odds', 'Stage2', 'Stage9', 'Unknown.Attr')}
    classes.update({'Odds.Cond_0': 'GearboxFramework.ConditionalAttributeValueResolver',
                    'Stage2.Const_0': 'GearboxFramework.ConstantAttributeValueResolver',
                    'Stage9.Const_0': 'GearboxFramework.ConstantAttributeValueResolver',
                    'D.CommonMod': 'WillowGame.DesignerAttributeDefinition',
                    'List': 'WillowGame.ItemPoolListDefinition', 'Pawn': 'WillowGame.AIPawnBalanceDefinition'})
    for pool in ('Pool.Guns', 'Pool.Common', 'Pool.Rare', 'Pool.Late'):
        classes[pool] = 'WillowGame.ItemPoolDefinition'
    for gun in ('Gun.A', 'Gun.B', 'Gun.R', 'Gun.L'):
        classes[gun] = 'WillowGame.WeaponBalanceDefinition'
    return FakePackage(objects, classes)


class ValuesTests(unittest.TestCase):
    def test_formula_designer_and_disabled_clamp(self):
        values = L.Values(world())
        self.assertEqual(values.init(const(0, init='W.Rare')), 1.0)   # MinValue 100 not enabled
        self.assertEqual(values.init(const(0, init='W.Mod')), 100.0)  # designer BaseValue 1 * 100
        self.assertEqual(L.Values(world(), editor=True).init(const(0, init='W.Mod')), 0.0)

    def test_conditional_resolver_uses_player_count(self):
        self.assertEqual(L.Values(world()).init(const(0, 'Odds')), 0.5)
        self.assertEqual(L.Values(world(), {L.NUMBER_OF_PLAYERS: 3}).init(const(0, 'Odds')), 0.25)

    def test_unknown_attribute_is_unresolved_not_guessed(self):
        values = L.Values(world())
        self.assertIsNone(values.init(const(0, 'Unknown.Attr')))
        self.assertIn('Unknown.Attr', values.unresolved)


class TableTests(unittest.TestCase):
    def test_expand_shares_gates_and_quantity(self):
        package = world()
        tree = L.expand(package, L.Values(package), 'Pool.Guns', stage=5)
        shares = {e['path']: round(e['share'], 4) for e in tree['entries']}
        # Late is gated off (stage 5 < 9); Common 100 vs Rare 1.
        self.assertEqual(shares, {'Pool.Common': round(100 / 101, 4), 'Pool.Rare': round(1 / 101, 4), 'Pool.Late': 0.0})
        items = L.flatten(tree)
        self.assertAlmostEqual(items['Gun.A'], 100 / 101 * 0.75)
        self.assertAlmostEqual(items['Gun.R'], 2 / 101)  # Quantity 2
        self.assertNotIn('Gun.L', items)

    def test_pawn_source_lists_and_unresolved_probability(self):
        package = world()
        loot = L.table(package, L.Values(package), 'Pawn', stage=10)
        self.assertEqual([p['pool'] for p in loot['pools']], ['Pool.Guns', 'Pool.Rare', 'Pool.Late'])
        self.assertEqual([p['probability'] for p in loot['pools']], [0.5, None, 1.0])
        self.assertIn('Unknown.Attr', loot['unresolved'])
        # Expected items per firing at stage 10: Common 100/102, Rare 2 x 1/102 (Quantity 2), Late 1/102.
        self.assertAlmostEqual(loot['pools'][0]['by_class']['WeaponBalanceDefinition'], 103 / 102)

    def test_seeded_roll_is_deterministic_and_skips_unresolved_pools(self):
        package = world()
        loot = L.table(package, L.Values(package), 'Pawn', stage=10)
        first, again = L.roll(loot, 7), L.roll(loot, 7)
        self.assertEqual(first, again)
        self.assertTrue(any(d['item'] == 'Gun.L' for d in first))  # probability 1 always fires
        self.assertTrue(all(d['chain'][0] != 'Pool.Rare' for d in first))  # unresolved list entry never fires

    def test_roll_pool_respects_quantity(self):
        package = world()
        tree = L.expand(package, L.Values(package), 'Pool.Rare', stage=1)
        self.assertEqual([d['item'] for d in L.roll_pool(tree, random.Random(1))], ['Gun.R', 'Gun.R'])

    def test_display_check_counts_agreement(self):
        package = world()
        report = L.display_check(package, ['Pool.Guns'])
        # Editor-like context: designer attribute 0, so Common 0%, Rare and Late 1 each = 50%: one of three agrees.
        self.assertEqual((report['entries'], report['agree'], report['differ']), (3, 1, 2))


if __name__ == '__main__':
    unittest.main()
