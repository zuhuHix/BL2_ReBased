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
        self.assertEqual(card['display']['damage'], 156)
        self.assertAlmostEqual(card['fire_rate'], 4.0)

    def test_negative_scales_divide_and_manufacturer_operands(self):
        card = s.evaluate(FakePackage(world()), recipe(), level=1)['card']
        # Default 'split' rule: foreign-maker +3 is 0; 10 / (1 + 1000), then PostAdd +1.
        self.assertAlmostEqual(card['magazine'], 10 / 1001 + 1)
        self.assertAlmostEqual(card['reload_time'], 2.0 / 1.5)  # own-maker -50% divides
        self.assertEqual(card['display']['magazine'], 1)      # the card rounds the magazine down

    def test_sum_rule_still_available(self):
        previous, s.SCALE_RULE = s.SCALE_RULE, 'sum'
        try:
            card = s.evaluate(FakePackage(world()), recipe(), level=1)['card']
        finally:
            s.SCALE_RULE = previous
        self.assertEqual(card['magazine'], 1.0)       # clamp at 0, then +1
        self.assertAlmostEqual(card['reload_time'], 1.0)

    def test_combine_splits_positive_and_negative_scales(self):
        bucket = {'MT_PreAdd': 2.0, 'MT_PostAdd': 1.0, 'scale_up': 0.6, 'scale_down': 0.2}
        self.assertAlmostEqual(s.combine(10.0, bucket), 12.0 * 1.6 / 1.2 + 1.0)

    def test_type_effects_projectiles_and_shot_cost_defaults(self):
        objects = world()
        objects['Type'].update({'ProjectilesPerShot': 3, 'WeaponAttributeEffects': [
            {'AttributeToModify': A + 'WeaponShotCost', 'ModifierType': 'MT_PreAdd', 'BaseModifierValue': const(1)},
            {'AttributeToModify': A + 'WeaponProjectilesPerShot', 'ModifierType': 'MT_PreAdd',
             'BaseModifierValue': const(2)}]})
        card = s.evaluate(FakePackage(objects), recipe(), level=1)['card']
        self.assertEqual(card['shot_cost'], 2.0)       # WillowWeapon default 1 + type PreAdd 1
        self.assertEqual(card['projectiles'], 5)
        self.assertEqual(card['display']['projectiles'], 5)
        plain = s.evaluate(FakePackage(world()), recipe(), level=1)['card']
        self.assertEqual((plain['shot_cost'], plain['projectiles']), (1.0, 1))

    def test_status_rows_from_damage_type(self):
        objects = world()
        objects['Type'].update({'StatusEffectDamage': const(100.0), 'BaseStatusEffectChanceModifier': const(0.6),
                                'WeaponAttributeEffects': [
                                    {'AttributeToModify': A + 'WeaponStatusEffectDamage', 'ModifierType': 'MT_PreAdd',
                                     'BaseModifierValue': const(0.2)},
                                    {'AttributeToModify': A + 'WeaponStatusEffectChanceModifier',
                                     'ModifierType': 'MT_Scale', 'BaseModifierValue': const(0.5)}]})
        objects['Fire'] = {'CustomDamageTypeDefinition': 'DmgType.Fire'}
        objects['DmgType.Fire'] = {'StatusEffect': 'Status.Burn'}
        objects['Status.Burn'] = {'StatusEffectType': 'STATUS_EFFECT_Ignite', 'bDoesDamageOverTime': True,
                                  'BaseDuration': const(5), 'DamageSurfaceChanceModifiers': [
                                      {'SurfaceType': 'DMGSURFACE_Generic', 'BaseChance': const(20)},
                                      {'SurfaceType': 'DMGSURFACE_Flesh', 'BaseChance': const(30)}]}
        item = recipe()
        item['parts']['Elemental'] = {'part': 'Fire'}
        card = s.evaluate(FakePackage(objects), item, level=1)['card']
        self.assertEqual(card['element'], 'Fire')
        self.assertEqual(card['status_effect'], 'STATUS_EFFECT_Ignite')
        self.assertAlmostEqual(card['status_chance'], 20 * 0.6 * 1.5)  # Generic chance * base * modifier
        self.assertAlmostEqual(card['status_chance_by_surface']['flesh'], 30 * 0.6 * 1.5)
        self.assertAlmostEqual(card['status_dps'], 100.2)
        self.assertEqual(card['display']['status_dps'], 100.2)
        self.assertEqual(card['status_duration'], 5)
        objects['Status.Burn']['bDoesDamageOverTime'] = False
        self.assertIsNone(s.evaluate(FakePackage(objects), item, level=1)['card']['status_dps'])

    def test_barrel_firing_mode_overrides_the_type_and_speed_scales(self):
        objects = world()
        objects['Type'].update({'DefaultFiringModeDefinition': 'Mode.Default', 'WeaponAttributeEffects': [
            {'AttributeToModify': A + 'WeaponProjectileSpeedMultiplier', 'ModifierType': 'MT_PreAdd',
             'BaseModifierValue': const(0.5)}]})
        objects.update({'Mode.Default': {'Speed': 12000}, 'Mode.Barrel': {'Speed': 4000}})
        card = s.evaluate(FakePackage(objects), recipe(), level=1)['card']
        self.assertEqual((card['firing_mode'], card['projectile_speed']), ('Mode.Default', 18000))
        objects['Barrel']['CustomFiringModeDefinition'] = 'Mode.Barrel'
        card = s.evaluate(FakePackage(objects), recipe(), level=1)['card']
        self.assertEqual((card['firing_mode'], card['projectile_speed']), ('Mode.Barrel', 6000))

    def test_no_damage_type_means_no_status_rows(self):
        card = s.evaluate(FakePackage(world()), recipe(), level=1)['card']
        self.assertNotIn('status_chance', card)
        self.assertIsNone(card['damage_type'])

    def test_display_rounding(self):
        shown = s.display({'damage': 6971.38, 'magazine': 30.8, 'fire_rate': 3.3613, 'reload_time': 4.86,
                           'accuracy': 87.35, 'projectiles': 1})
        self.assertEqual(shown, {'damage': 6972, 'magazine': 30, 'fire_rate': 3.4, 'reload_time': 4.9,
                                 'accuracy': 87.4})

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
UNCHECKED_CALC = 'GD_Economy.PriceCalc.Init_Gun_Launchers_PriceCalculator'


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

    def test_accuracy_is_the_presentation_remap_of_spread(self):
        card = self.card()
        self.assertAlmostEqual(card['accuracy'], 100 * (1 - 2.0 / 15))  # spread 2.0
        self.assertTrue(card['accuracy_known'])  # known only under the default 'split' rule

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
        objects['Type']['MonetaryValue']['InitializationDefinition'] = UNCHECKED_CALC
        objects[UNCHECKED_CALC] = objects[CALC]
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
