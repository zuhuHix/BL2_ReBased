"""Synthetic tests for tools/weapon_stats.py attribute combination."""
from pathlib import Path
import json
import math
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
        # Default 'split' rule: foreign-maker +3 is 0; 10 / (1 + 1000), then PostAdd +1 = 1.00999;
        # the clip size is an integer attribute, so the game truncates it to 1.
        self.assertEqual(card['magazine'], 1.0)
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
        # Generic chance * base * modifier (single precision, hence places=5).
        self.assertAlmostEqual(card['status_chance'], 20 * 0.6 * 1.5, places=5)
        self.assertAlmostEqual(card['status_chance_by_surface']['flesh'], 30 * 0.6 * 1.5, places=5)
        self.assertAlmostEqual(card['status_dps'], 100.2, places=4)
        self.assertEqual(card['display']['status_dps'], 100.2)
        self.assertEqual(card['status_duration'], 5)
        objects['Status.Burn']['bDoesDamageOverTime'] = False
        self.assertIsNone(s.evaluate(FakePackage(objects), item, level=1)['card']['status_dps'])

    def test_status_card_chance_is_the_mapped_presentation_remap(self):
        objects = world()
        objects['Type']['BaseStatusEffectChanceModifier'] = const(0.6)
        objects['Type']['WeaponAttributeEffects'] = [{'AttributeToModify': A + 'WeaponStatusEffectChanceModifier',
                                                      'ModifierType': 'MT_Scale', 'BaseModifierValue': const(0.5)}]
        objects['Slag'] = {'CustomDamageTypeDefinition': 'DmgType.Amp'}
        objects['DmgType.Amp'] = {'StatusEffect': 'Status.Amp'}
        objects['Status.Amp'] = {'StatusEffectType': 'STATUS_EFFECT_Amp', 'DamageSurfaceChanceModifiers': [
            {'SurfaceType': 'DMGSURFACE_Generic', 'BaseChance': const(30)}]}
        objects['Present.Amp'] = {'bValueRemappingEnabled': True, 'RemappingData': {
            'InputValueMx': {'BaseValueConstant': 3.32999992}, 'OutputValueMx': {'BaseValueConstant': 100}}}
        item = dict(recipe(), parts=dict(recipe()['parts'], Elemental={'part': 'Slag'}))
        card = s.evaluate(FakePackage(objects), item, level=1)['card']
        self.assertAlmostEqual(card['status_chance'], 27.0, places=5)   # no mapping: Generic 30 * 0.9
        objects[s.STATUS_CHANCE_MAPPING] = {'MappingData': [
            {'DamageType': 'DmgType.Amp', 'Presentation': 'Present.Amp'}]}
        card = s.evaluate(FakePackage(objects), item, level=1)['card']
        f32 = s.weapon_recipe.f32
        slope = f32(100 / f32(3.33))                                    # 30.03003
        self.assertEqual(card['status_chance'], f32(f32(f32(0.6) * 1.5) * slope))
        self.assertEqual(card['display']['status_chance'], 27.0)        # 27.027 prints 27.0
        self.assertAlmostEqual(card['status_chance_by_surface']['generic'], 27.0, places=5)  # unchanged
        objects['Type']['BaseStatusEffectChanceModifier'] = const(6.0)  # input clamps at 3.33
        card = s.evaluate(FakePackage(objects), item, level=1)['card']
        self.assertEqual(card['status_chance'], 100.0)
        self.assertAlmostEqual(card['status_chance_by_surface']['generic'], 270.0, places=3)

    def test_element_badge_uses_type_default_when_elemental_part_is_none(self):
        objects = world()
        objects['Type']['DefaultDamageTypeDefinition'] = 'Damage.Explosive'
        objects['Damage.Explosive'] = {'DamageType': 'DAMAGE_TYPE_Explosive'}
        objects['Element.None'] = {}
        item = recipe()
        item['parts']['Elemental'] = {'part': 'Element.None'}

        card = s.evaluate(FakePackage(objects), item, level=1)['card']

        self.assertEqual(card['damage_type'], 'Damage.Explosive')
        self.assertEqual(card['element'], 'Explosive')
        without_elemental_part = s.evaluate(FakePackage(objects), recipe(), level=1)['card']
        self.assertEqual(without_elemental_part['element'], 'Explosive')

    def test_element_badge_maps_damage_type_enum_to_existing_icon_names(self):
        objects = world()
        objects['Damage.Fire'] = {'DamageType': 'DAMAGE_TYPE_Incindiary'}
        objects['Element.Fire'] = {'CustomDamageTypeDefinition': 'Damage.Fire'}
        item = recipe()
        item['parts']['Elemental'] = {'part': 'Element.Fire'}

        card = s.evaluate(FakePackage(objects), item, level=1)['card']

        self.assertEqual(card['element'], 'Fire')

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
                           'accuracy': 87.36, 'projectiles': 1})
        self.assertEqual(shown, {'damage': 6972, 'magazine': 30, 'fire_rate': 3.4, 'reload_time': 4.9,
                                 'accuracy': 87.4})
        # Single precision throughout (invented values): 87.35 is stored as 87.3499985, but times 10
        # in single precision it is 873.5 again, so it prints 87.4 (it printed 87.3 before the
        # 2026-10-03 rule). A stored 33.349998 times 10 is an exact tie that rounds to the even
        # 333.5 and prints 33.4; 1.7499998 times 10 stays under 17.5 and prints 1.7.
        self.assertEqual(s.display({'accuracy': 87.35})['accuracy'], 87.4)
        self.assertEqual(s.present(33.349998474121094, 'ATTRROUNDING_Float', 1), 33.4)
        self.assertEqual(s.present(1.7499998807907104, 'ATTRROUNDING_Float', 1), 1.7)
        self.assertEqual(s.present(1.7499998807907104, 'ATTRROUNDING_Float', 2), 1.75)

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
UNCHECKED_CALC = 'Invented.PriceCalc.Init_Unchecked_PriceCalculator'


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
    # Every player gun type sets 1; the class default (unset) is 0.
    objects['Type']['BaseMonetaryValueModifier'] = 1.0
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
        self.assertAlmostEqual(card['accuracy'], 100 * (1 - 2.0 / 15), places=4)  # spread 2.0
        self.assertTrue(card['accuracy_known'])  # known only under the default 'split' rule

    def test_accuracy_clamps_and_is_absent_without_the_presentation(self):
        objects = card_world()
        objects['Type']['Spread'] = 20.0
        # The input clamps to 15; the single-precision slope leaves 2.4e-6, which prints as 0.0.
        self.assertAlmostEqual(self.card(objects)['accuracy'], 0.0, places=4)
        self.assertEqual(self.card(objects)['display']['accuracy'], 0.0)
        del objects[s.ACCURACY_PRESENTATION]
        self.assertIsNone(self.card(objects)['accuracy'])

    def test_sale_value_scales_the_maker_attribute_and_rounds_down(self):
        # 1.51 * (2.0 * 1.25 * level 3 * 100 ^ 1) = 1132.5
        card = self.card()
        self.assertEqual(card['sale_value'], 1132)
        self.assertTrue(card['sale_value_known'])  # Shotguns calculator was checked

    def test_price_formula_adds_the_offset_before_the_multiplier(self):
        objects = card_world()
        objects['Init.Offset'] = {'ValueFormula': {'bEnabled': True, 'Multiplier': const(2), 'Level': const(3),
                                                   'Power': const(2), 'Offset': const(1)}}
        unresolved = set()
        self.assertEqual(s.price_init(FakePackage(objects), const(0, init='Init.Offset'), {}, unresolved), 20)
        self.assertEqual(unresolved, set())

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

    def test_name_parts_add_effects_and_their_value_modifier(self):
        objects = card_world()
        objects['Prefix.Cheap'] = {'PartName': 'Cheap', 'MonetaryValueMod': 'ModC', 'WeaponAttributeEffects': [
            {'AttributeToModify': A + 'WeaponDamage', 'ModifierType': 'MT_Scale', 'BaseModifierValue': const(0.5)}]}
        objects['Title.Cheap'] = {'MonetaryValueMod': 'ModD'}
        objects.update({'ModC': {}, 'ModC.ConstantAttributeValueResolver_0': {'ConstantValue': 0.5},
                        'ModD': {}, 'ModD.ConstantAttributeValueResolver_0': {'ConstantValue': 0.7}})
        plain = self.card(objects)
        named = self.card(objects, prefix={'part': 'Prefix.Cheap'})
        fully_named = self.card(objects, prefix={'part': 'Prefix.Cheap'}, title={'part': 'Title.Cheap'})
        # 120 * (1 + 0.3 slot + 0.5 prefix) at level 3 vs 120 * 1.3; the value halves (1132.5 -> 566.25).
        self.assertAlmostEqual(plain['damage'], 120 * 1.3, places=3)
        self.assertAlmostEqual(named['damage'], 120 * 1.8, places=3)
        self.assertEqual((plain['sale_value'], named['sale_value']), (1132, 566))
        self.assertEqual(fully_named['sale_value'], 396)  # both name-part modifiers multiply before truncation

    def test_sale_value_part_total_starts_at_the_type_modifier(self):
        objects = card_world()
        objects['Type']['BaseMonetaryValueModifier'] = 2.0
        # 1.51 * (2.0 type * 2.0 * 1.25 * level 3 * 100) = 2265 (2265.0002 in single precision)
        self.assertEqual(self.card(objects)['sale_value'], 2265)
        del objects['Type']['BaseMonetaryValueModifier']
        self.assertEqual(self.card(objects)['sale_value'], 0)  # class default 0

    def test_sale_value_is_capped_by_the_currency(self):
        objects = card_world()
        objects['Type']['FormOfCurrency'] = 'CURRENCY_Eridium'
        self.assertEqual(self.card(objects)['sale_value'], 500)
        objects['Type']['FormOfCurrency'] = 'CURRENCY_SeraphCrystals'
        self.assertEqual(self.card(objects)['sale_value'], 999)
        objects['Type']['FormOfCurrency'] = 'CURRENCY_MAX'
        self.assertEqual(self.card(objects)['sale_value'], 0)
        objects['Type']['FormOfCurrency'] = 'CURRENCY_Credits'
        objects['Type']['BaseMonetaryValueModifier'] = 1e6
        self.assertEqual(self.card(objects)['sale_value'], 99999999)

    def test_fun_stats_uses_the_localized_override(self):
        objects = card_world()
        stats = s.evaluate(FakePackage(objects), dict(recipe(), title={'part': 'Title'}), level=1,
                           localize=lambda path, default: 'Overridden' if path == 'Title.Red' else default)
        self.assertEqual(stats['card']['fun_stats'], 'Overridden')


class CardTextTests(unittest.TestCase):
    """Invented fixtures for card text sources, ordering, localization and colors."""

    def test_zoom_uses_type_fov_and_activated_sight_grade(self):
        objects = card_world()
        objects['Type']['ZoomedEndFOV'] = 45.0
        objects['Type']['AttributeSlotEffects'].append({
            'SlotName': 'WeaponZoomFOV', 'AttributeToModify': s.ZOOM_ATTRIBUTE,
            'ModifierType': 'MT_Scale', 'BaseModifierValue': const(0),
            'PerGradeUpgrade': const(-0.13), 'bEnforceMinimumGrade': True,
        })
        objects['Sight'] = {
            'AttributeSlotUpgrades': [{'SlotName': 'WeaponZoomFOV', 'GradeIncrease': 7, 'bActivateSlot': True}],
            'WeaponCardAttributes': [{'Attribute': s.ZOOM_ATTRIBUTE, 'PriorityIncrease': 0.3}],
        }
        item = recipe()
        item['parts']['Sight'] = {'part': 'Sight'}
        card = s.evaluate(FakePackage(objects), item, level=1)['card']
        # Current model: 80 / (45 / (1 + 0.13 * 7)); resolver conversion is UNVERIFIED.
        self.assertAlmostEqual(card['weapon_zoom'], 80.0 / (45.0 / 1.91), places=5)
        self.assertEqual(card['display']['weapon_zoom'], 3.4)
        self.assertIn('3.4x Weapon Zoom', card['fun_stats'])
        self.assertEqual(card['fun_stats_lines'], [{'text': '3.4x Weapon Zoom', 'color': '#FFFFFF'}])

    def test_card_lines_keep_source_order_and_definition_colors(self):
        objects = card_world()
        objects['Type'].update({'CustomPresentations': ['Type.Text']})
        objects['Type.Text'] = {'Description': 'A weapon type note.', 'bDontDisplayNumber': True}
        objects['Sight'] = {'WeaponCardAttributes': [{'Attribute': s.ZOOM_ATTRIBUTE, 'PriorityIncrease': 0.3}]}
        objects['Type']['ZoomedEndFOV'] = 40.0
        objects['Type']['AttributeSlotEffects'].append({
            'SlotName': 'WeaponZoomFOV', 'AttributeToModify': s.ZOOM_ATTRIBUTE,
            'ModifierType': 'MT_Scale', 'BaseModifierValue': const(0), 'PerGradeUpgrade': const(0),
        })
        objects['Barrel']['WeaponCardAttributes'] = [{'Attribute': s.SHOT_COST_ATTRIBUTE, 'PriorityIncrease': 0.5}]
        objects['Barrel']['WeaponAttributeEffects'].append({
            'AttributeToModify': s.SHOT_COST_ATTRIBUTE, 'ModifierType': 'MT_PreAdd', 'BaseModifierValue': const(1),
        })
        objects['FirePart'] = {'CustomDamageTypeDefinition': 'DamageType.Fire'}
        objects['DamageType.Fire'] = {'WeaponCardPresentations': ['DamageType.Effectiveness']}
        objects['DamageType.Effectiveness'] = {
            'Description': 'Effective against a target type.', 'bDontDisplayNumber': True,
            'TextColor': {'R': 255, 'G': 128, 'B': 0, 'A': 255},
        }
        item = recipe()
        item.update({'title': {'part': 'Title'}})
        item['parts'].update({'Sight': {'part': 'Sight'}, 'Elemental': {'part': 'FirePart'}})
        card = s.evaluate(FakePackage(objects), item, level=1)['card']
        self.assertEqual([line['text'] for line in card['fun_stats_lines']], [
            'Red, line', '2.0x Weapon Zoom', 'Consumes 2 ammo per shot.',
            'A weapon type note.', 'Effective against a target type.',
        ])
        self.assertEqual(card['fun_stats_lines'][-1]['color'], '#FF8000')
        self.assertIn('<font color="#DC4646">Red, line</font>', card['fun_stats_markup'])
        self.assertIn('<font color="#FF8000">Effective against a target type.</font>', card['fun_stats_markup'])

    def test_inline_damage_tag_uses_the_definition_color_when_line_color_is_disabled(self):
        objects = card_world()
        objects['Type']['DefaultDamageTypeDefinition'] = 'DamageType.Basic'
        objects['DamageType.Basic'] = {'WeaponCardPresentations': ['DamageType.Bonus']}
        objects['DamageType.Bonus'] = {
            'Description': 'Deals bonus [explosive]accent[-explosive] damage.',
            'bDontDisplayNumber': True, 'bEnableTextColor': False,
            'TextColor': {'R': 255, 'G': 255, 'B': 0, 'A': 255},
        }
        card = s.evaluate(FakePackage(objects), recipe(), level=1)['card']
        self.assertEqual(card['fun_stats'], 'Deals bonus accent damage.')
        self.assertIn('<font color="#FFFFFF">Deals bonus ', card['fun_stats_markup'])
        self.assertIn('<font color="#FFFF00">accent</font>', card['fun_stats_markup'])

    def test_part_presentation_supplies_reduced_ammo_text(self):
        objects = card_world()
        objects['Barrel']['CustomPresentations'] = ['Barrel.Text']
        objects['Barrel.Text'] = {
            'NoConstraintText': 'A part grants reduced ammunition use.', 'bDontDisplayNumber': True,
        }
        card = s.evaluate(FakePackage(objects), recipe(), level=1)['card']
        self.assertEqual(card['shot_cost'], 1.0)
        self.assertEqual(card['fun_stats'], 'A part grants reduced ammunition use.')

    def test_description_localization_uses_its_own_property_key(self):
        objects = card_world()
        objects['Type']['CustomPresentations'] = ['Type.Text']
        objects['Type.Text'] = {'Description': 'Default note.', 'bDontDisplayNumber': True}
        observed = []

        def localize(path, default, property_name):
            observed.append(property_name)
            return 'Localized note.' if property_name == 'Description' else default

        card = s.evaluate(FakePackage(objects), recipe(), level=1, localize=localize)['card']
        self.assertIn('Description', observed)
        self.assertEqual(card['fun_stats'], 'Localized note.')

    def test_red_flavour_can_be_owned_by_a_balance_and_collapses_whitespace(self):
        objects = card_world()
        objects['Balance'] = {'CustomPresentations': ['Balance.Red']}
        objects['Balance.Red'] = {
            'NoConstraintText': 'A   compact     flavour line.',
            'TextColor': {'R': 220, 'G': 70, 'B': 70, 'A': 255},
        }
        item = dict(recipe(), balance='Balance')
        card = s.evaluate(FakePackage(objects), item, level=1)['card']
        self.assertEqual(card['fun_stats'], 'A compact flavour line.')
        self.assertEqual(card['fun_stats_lines'][0]['color'], '#DC4646')


class NativeRuleTests(unittest.TestCase):
    """Rules read from the game's code (NATIVE_WEAPON_RULES.md), on invented values."""

    def test_combine_has_no_clamp_and_integers_truncate(self):
        self.assertAlmostEqual(s.combine(10.0, {'MT_PreAdd': -20.0}), -10.0)       # not clamped at 0
        self.assertEqual(s.combine(10.0, {'MT_PostAdd': 0.99}, integer=True), 10.0)
        self.assertEqual(s.combine(10.0, {'MT_PostAdd': -10.5}, integer=True), 0.0)  # -0.5 truncates toward 0
        bucket = {'scale_up': 0.5, 'scale_down': 0.25}
        self.assertEqual(s.combine(7.9, bucket, integer=True), 8.0)                 # base 7: 7 * 1.5 / 1.25 = 8.4 -> 8

    def test_single_precision_decides_the_ceiling(self):
        self.assertEqual(s.present(100.000001, 'ATTRROUNDING_IntCeil'), 100)  # f32 is exactly 100
        self.assertEqual(s.present(100.01, 'ATTRROUNDING_IntCeil'), 101)
        self.assertEqual(s.present(2.45, 'ATTRROUNDING_Float', 1), 2.5)
        self.assertEqual(s.present(2.5, 'ATTRROUNDING_IntRound'), 3)
        self.assertEqual(s.present(-2.5, 'ATTRROUNDING_IntFloor'), -3)
        self.assertEqual(s.present(5e-9, 'ATTRROUNDING_IntCeil'), 0)

    def test_shared_card_rounding_cases(self):
        # The same invented cases check the page's rounding (tests/inventory_navigation_test.js).
        # 'pending_page_change' holds the cases the page does not print this way yet (Python only).
        data = json.loads((Path(__file__).parent / 'card_rounding_cases.json').read_text(encoding='utf-8'))
        for case in data['cases'] + data.get('pending_page_change', []):
            shown = s.display({case['field']: case['value']})[case['field']]
            decimals = 0 if isinstance(shown, int) else 1
            self.assertEqual(f'{shown:.{decimals}f}', case['text'], case)

    def test_presentation_data_sets_the_display_rounding(self):
        objects = world()
        objects[s.PRESENTATIONS['damage'][0]] = {'RoundingMode': 'ATTRROUNDING_IntFloor'}
        objects[s.PRESENTATIONS['reload_time'][0]] = {'RoundingMode': 'ATTRROUNDING_Float', 'FloatPrecision': 2}
        card = s.evaluate(FakePackage(objects), recipe(), level=1)['card']
        # level 1: 10 * 2 * 1.5 = 30, * 1.3 = 39.0 (floor keeps 38 if f32 lands below 39)
        self.assertEqual(card['display']['damage'], math.floor(card['damage']))
        self.assertEqual(card['display']['reload_time'], round(card['reload_time'], 2))

    def test_slot_effects_need_activation_and_count_every_increase(self):
        objects = world()
        objects['Grip']['AttributeSlotUpgrades'] = [{'SlotName': 'WeaponDamage', 'GradeIncrease': 2}]
        card = s.evaluate(FakePackage(objects), recipe(), level=3)['card']
        self.assertAlmostEqual(card['damage'], 120 * 1.5, places=3)  # grade 3 + 2 (not activating)
        objects['Barrel']['AttributeSlotUpgrades'][0]['bActivateSlot'] = False
        card = s.evaluate(FakePackage(objects), recipe(), level=3)['card']
        self.assertAlmostEqual(card['damage'], 120.0, places=3)      # nothing activated the slot

    def test_slot_base_grade_enters_the_grade_and_one_is_subtracted(self):
        objects = world()
        objects['Type']['AttributeSlotBaseGrade'] = const(1.5)  # rounds half up to 2
        card = s.evaluate(FakePackage(objects), recipe(), level=3)['card']
        # grade 2 + 3 = 5; value (5 - 1) * 0.1 = 0.4
        self.assertAlmostEqual(card['damage'], 120 * 1.4, places=3)

    def test_slot_clamps_act_on_the_full_grade_maximum_first(self):
        effect = {'bEnforceMaximumGrade': True, 'MaximumGrade': 2, 'bEnforceMinimumGrade': True, 'MinimumGrade': 4}
        self.assertEqual(s.slot_grade(1.0, 9, effect), 4)          # 10 -> max 2 -> min 4
        self.assertEqual(s.slot_grade(None, -2, {'bEnforceMinimumGrade': True, 'MinimumGrade': 0}), 0)
        self.assertEqual(s.slot_grade(1.0, -2, {}), -1)
        self.assertEqual(s.slot_value(0, 0.0, -0.13), s.weapon_recipe.f32(0.13))

    def test_negative_zoom_sum_clamps_to_minimum_grade_zero(self):
        objects = card_world()
        objects['Type']['ZoomedEndFOV'] = 45.0
        objects['Type']['AttributeSlotEffects'].append({
            'SlotName': 'WeaponZoomFOV', 'AttributeToModify': s.ZOOM_ATTRIBUTE, 'ModifierType': 'MT_Scale',
            'BaseModifierValue': const(0), 'PerGradeUpgrade': const(-0.13), 'bEnforceMinimumGrade': True})
        objects['Sight'] = {
            'AttributeSlotUpgrades': [{'SlotName': 'WeaponZoomFOV', 'GradeIncrease': -2, 'bActivateSlot': True}],
            'WeaponCardAttributes': [{'Attribute': s.ZOOM_ATTRIBUTE}]}
        item = dict(recipe(), parts=dict(recipe()['parts'], Sight={'part': 'Sight'}))
        card = s.evaluate(FakePackage(objects), item, level=1)['card']
        # grade 1 - 2 = -1 clamps to 0: value (0 - 1) * -0.13 = +0.13, FOV 45 * 1.13
        self.assertAlmostEqual(card['weapon_zoom'], 80.0 / (45.0 * 1.13), places=5)

    def test_rarity_is_a_sum_looked_up_in_the_globals_table(self):
        objects = world()
        objects['Barrel']['Rarity'] = const(4.9)
        objects['Grip']['Rarity'] = const(2.0)
        objects['Type']['BaseRarity'] = const(0.5)
        card = s.evaluate(FakePackage(objects), recipe(), level=1)['card']
        self.assertEqual((card['rarity_level'], card['rarity_rating'], card['rarity']), (6, None, 5))  # no table
        objects[s.GLOBALS] = {'RarityLevelColors': [
            {'MinLevel': 1, 'MaxLevel': 5, 'Color': {'R': 1, 'G': 2, 'B': 3}, 'RarityRating': 'RARITY_Uncommon'},
            {'MinLevel': 6, 'MaxLevel': 9, 'Color': {'R': 255, 'G': 0, 'B': 16}, 'RarityRating': 'RARITY_Legendary'}]}
        card = s.evaluate(FakePackage(objects), recipe(), level=1)['card']
        self.assertEqual((card['rarity_level'], card['rarity'], card['rarity_color']), (6, 5, '#FF0010'))

    def test_plain_float_base_is_single_precision(self):
        # Invented: a 2.1 s reload under a -20 % scale. As a float 2.1 is 2.0999999, so the stack
        # lands just under 1.75 and the card prints 1.7; the double 2.1 would give 1.75 and 1.8.
        objects = world()
        objects['Type']['ReloadTime'] = 2.1
        objects['Grip']['WeaponAttributeEffects'][1]['BaseModifierValue'] = const(-0.2)
        card = s.evaluate(FakePackage(objects), recipe(), level=1)['card']
        self.assertLess(card['reload_time'], 1.75)
        self.assertAlmostEqual(card['reload_time'], 1.75, places=6)
        self.assertEqual(card['display']['reload_time'], 1.7)

    def test_level_line_rule(self):
        objects = world()
        objects['Type']['bUsesPlayerLevelRequirement'] = True
        package = FakePackage(objects)
        package.classes = {'Bal.Mission': 'WillowGame.MissionWeaponBalanceDefinition',
                           'Bal.Normal': 'WillowGame.WeaponBalanceDefinition'}

        def card(level, balance='Bal.Normal', **kw):
            return s.evaluate(package, dict(recipe(), balance=balance), level=level, **kw)['card']
        self.assertEqual((card(8)['level_requirement'], card(8)['level_line']), (8, True))
        self.assertEqual((card(1)['level_requirement'], card(1)['level_line']), (1, False))   # level 1: no line
        self.assertEqual((card(8, 'Bal.Mission')['level_requirement'], card(8, 'Bal.Mission')['level_line']), (0, False))
        self.assertEqual(card(8, level_bonus=2.7)['level_requirement'], 6)                      # floor of the bonus
        self.assertEqual(card(2, level_bonus=5)['level_requirement'], 1)                        # at least 1
        del objects['Type']['bUsesPlayerLevelRequirement']
        self.assertEqual(card(8)['level_line'], False)


if __name__ == '__main__':
    unittest.main()
