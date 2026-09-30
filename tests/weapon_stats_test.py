"""Synthetic tests for tools/weapon_stats.py attribute combination."""
from pathlib import Path
import sys
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import weapon_stats as s

A = 'D_Attributes.Weapon.'


def const(value, attribute=None, scale=1.0, init=None):
    return {'BaseValueConstant': value, 'BaseValueAttribute': attribute,
            'InitializationDefinition': init, 'BaseValueScaleConstant': scale}


class FakePackage:
    def __init__(self, objects):
        self.objects = objects
        self.index = {k: i for i, k in enumerate(objects)}

    def props(self, path):
        return self.objects[path]


def world():
    return {
        'Scaler': {}, 'Scaler.ConstantAttributeValueResolver_0': {'ConstantValue': 2.0},
        'Init.Damage': {'ValueFormula': {'bEnabled': True, 'Multiplier': const(10), 'Level': const(0, 'Scaler'),
                                         'Power': const(0, A + 'WeaponLevel')}},
        'Type': {'InstantHitDamage': const(0, init='Init.Damage', scale=1.5), 'FireRate': 0.25, 'ClipSize': 10,
                 'ReloadTime': 2.0, 'Spread': 2.0, 'AttributeSlotEffects': [
                     {'SlotName': 'WeaponDamage', 'AttributeToModify': A + 'WeaponDamage', 'ModifierType': 'MT_Scale',
                      'BaseModifierValue': const(0), 'PerGradeUpgrade': const(0.1)}]},
        'Barrel': {'WeaponAttributeEffects': [
            {'AttributeToModify': A + 'WeaponClipSize', 'ModifierType': 'MT_Scale', 'BaseModifierValue': const(-1000)},
            {'AttributeToModify': A + 'WeaponClipSize', 'ModifierType': 'MT_PostAdd', 'BaseModifierValue': const(1)}],
            'AttributeSlotUpgrades': [{'SlotName': 'WeaponDamage', 'GradeIncrease': 3, 'bActivateSlot': True}]},
        'Grip': {'WeaponAttributeEffects': [
            {'AttributeToModify': A + 'WeaponClipSize', 'ModifierType': 'MT_PreAdd',
             'BaseModifierValue': const(0, 'Attr.Weapon_Is_Other', 3)},
            {'AttributeToModify': A + 'WeaponReloadSpeed', 'ModifierType': 'MT_Scale',
             'BaseModifierValue': const(0, 'Attr.Weapon_Is_Maker', -0.5)}]},
    }


def recipe():
    return {'name': 'Test', 'manufacturer': 'Makers.Maker', 'weapon_type': 'Type',
            'parts': {'Barrel': {'part': 'Barrel'}, 'Grip': {'part': 'Grip'}}}


class WeaponStatsTests(unittest.TestCase):
    def test_level_formula_type_scale_and_slot_grades(self):
        card = s.evaluate(FakePackage(world()), recipe(), level=3)['card']
        # 10 * 2^3 = 80, * 1.5 type scale = 120, * (1 + 0.1 * 3 grades).
        self.assertAlmostEqual(card['damage'], 156.0)
        self.assertAlmostEqual(card['fire_rate'], 4.0)

    def test_clamp_before_post_add_and_manufacturer_operands(self):
        card = s.evaluate(FakePackage(world()), recipe(), level=1)['card']
        self.assertEqual(card['magazine'], 1.0)       # foreign-maker +3 is 0; clamp then +1
        self.assertAlmostEqual(card['reload_time'], 1.0)  # own-maker -50%

    def test_spin_mode_and_default_start_scale(self):
        objects = world()
        self.assertIsNone(s.evaluate(FakePackage(objects), recipe(), level=1)['card']['spin_mode'])
        objects['Type']['BarrelSpinMode'] = 'BSM_SpinUpToFullFireRate'
        objects['Barrel'].update({'bIsSpinningEnabled': True, 'SpinUpDuration': const(0.8)})
        card = s.evaluate(FakePackage(objects), recipe(), level=1)['card']
        self.assertEqual(card['spin_mode'], 'BSM_SpinUpToFullFireRate')
        self.assertEqual(card['spin_start_interval_scale'], 1.0)  # omitted class default
        self.assertAlmostEqual(card['spin_up'], 0.8)


CALC = 'GD_Economy.PriceCalc.Init_Gun_Shotguns_PriceCalculator'


def card_world():
    """world() plus the objects the accuracy, price and red-text fields read."""
    objects = world()
    objects[s.ACCURACY_PRESENTATION] = {'RemappingData': {
        'InputValueMx': {'BaseValueConstant': 15.0}, 'OutputValueMn': {'BaseValueConstant': 100.0}}}
    objects.update({
        'Att.Maker': {}, 'Att.Maker.ConstantAttributeValueResolver_0': {'ConstantValue': 1.51},
        'Att.Base': {}, 'Att.Base.ConstantAttributeValueResolver_0': {'ConstantValue': 100.0},
        'ModA': {}, 'ModA.ConstantAttributeValueResolver_0': {'ConstantValue': 2.0},
        'ModB': {}, 'ModB.ConstantAttributeValueResolver_0': {'ConstantValue': 1.25},
        # Universal calculator sets total * level; the type's calculator scales the
        # maker attribute by universal * Att.Base ^ 1.
        'Init.Universal': {'ValueFormula': {'bEnabled': True, 'Multiplier': const(0, s.PART_PRICE_TOTAL),
                                            'Level': const(0, A + 'WeaponLevel'), 'Power': const(1)}},
        CALC: {'BaseValueMode': 'BASEVALUE_InitializationDefScalesBaseValue', 'ValueFormula': {
            'bEnabled': True, 'Multiplier': const(0, init='Init.Universal'),
            'Level': const(0, 'Att.Base'), 'Power': const(1)}},
        'Title': {'CustomPresentations': ['Title.Red', 'Title.White']},
        'Title.Red': {'NoConstraintText': 'Red; line', 'TextColor': {'R': 220, 'G': 70, 'B': 70, 'A': 255}},
        'Title.White': {'NoConstraintText': 'Not red', 'TextColor': {'R': 255, 'G': 255, 'B': 255, 'A': 255}},
    })
    objects['Type']['MonetaryValue'] = {'BaseValueAttribute': 'Att.Maker', 'InitializationDefinition': CALC}
    objects['Barrel']['MonetaryValueMod'] = 'ModA'
    objects['Grip']['MonetaryValueMod'] = 'ModB'
    return objects


class CardFieldTests(unittest.TestCase):
    def card(self, objects=None, **recipe_changes):
        item = recipe()
        item.update(recipe_changes)
        return s.evaluate(FakePackage(objects or card_world()), item, level=3)['card']

    def test_accuracy_is_the_presentation_remap_of_spread_and_never_known(self):
        card = self.card()
        self.assertAlmostEqual(card['accuracy'], 100 * (1 - 2.0 / 15))  # spread 2.0
        self.assertFalse(card['accuracy_known'])

    def test_accuracy_clamps_and_is_absent_without_the_presentation(self):
        objects = card_world()
        objects['Type']['Spread'] = 20.0
        self.assertEqual(self.card(objects)['accuracy'], 0.0)
        del objects[s.ACCURACY_PRESENTATION]
        self.assertIsNone(self.card(objects)['accuracy'])

    def test_sale_value_scales_the_maker_attribute_and_rounds_down(self):
        # 1.51 * (2.0 * 1.25 * level 3 * 100 ^ 1) = 1132.5
        card = self.card()
        self.assertEqual(card['sale_value'], 1132)
        self.assertTrue(card['sale_value_known'])  # Shotguns calculator was checked

    def test_unchecked_calculator_is_flagged_not_hidden(self):
        objects = card_world()
        objects['Type']['MonetaryValue']['InitializationDefinition'] = 'Other.Calculator'
        objects['Other.Calculator'] = objects[CALC]
        card = self.card(objects)
        self.assertEqual(card['sale_value'], 1132)
        self.assertFalse(card['sale_value_known'])

    def test_sale_value_absent_when_an_attribute_cannot_be_resolved(self):
        objects = card_world()
        del objects['ModA.ConstantAttributeValueResolver_0']
        card = self.card(objects)
        self.assertIsNone(card['sale_value'])
        self.assertFalse(card['sale_value_known'])

    def test_no_monetary_value_means_no_sale_value(self):
        card = s.evaluate(FakePackage(world()), recipe(), level=1)['card']
        self.assertIsNone(card['sale_value'])
        self.assertIsNone(card['accuracy'])
        self.assertIsNone(card['fun_stats'])

    def test_fun_stats_is_only_the_red_title_presentation(self):
        card = self.card(title={'part': 'Title'})
        self.assertEqual(card['fun_stats'], 'Red, line')  # ';' would split the HUD line

    def test_fun_stats_uses_the_localized_override(self):
        objects = card_world()
        stats = s.evaluate(FakePackage(objects), dict(recipe(), title={'part': 'Title'}), level=1,
                           localize=lambda path, default: 'Overridden' if path == 'Title.Red' else default)
        self.assertEqual(stats['card']['fun_stats'], 'Overridden')


if __name__ == '__main__':
    unittest.main()
