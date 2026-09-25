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
        'Init.Damage': {'ValueFormula': {'Multiplier': const(10), 'Level': const(0, 'Scaler'),
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


if __name__ == '__main__':
    unittest.main()
