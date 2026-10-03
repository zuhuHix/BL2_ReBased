"""Synthetic tests for tools/weapon_balance.py and tools/weapon_card_audit.py (no game data)."""
from pathlib import Path
import sys
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import weapon_balance as B
import weapon_card_audit as A


def const(value):
    return {'BaseValueConstant': value, 'BaseValueAttribute': None, 'InitializationDefinition': None,
            'BaseValueScaleConstant': 1.0}


def part(name, weight_index=2, makers=()):
    return {'Part': name, 'Manufacturers': [{'Manufacturer': m, 'DefaultWeightIndex': i} for m, i in makers],
            'MinGameStageIndex': 0, 'MaxGameStageIndex': 1, 'DefaultWeightIndex': weight_index}


class FakePackage:
    def __init__(self, objects):
        self.objects = objects

    def props(self, path):
        return self.objects[path]


def world():
    consolidated = [const(1), const(100), const(0), const(10)]
    return FakePackage({
        'Root': {'InventoryDefinition': 'Type', 'WeaponPartListCollection': 'Root.PartList',
                 'Manufacturers': [{'Manufacturer': 'Maker'}]},
        'Root.PartList': {'ConsolidatedAttributeInitData': consolidated,
                          'BodyPartData': {'bEnabled': True, 'WeightedParts': [part('Body.A')]},
                          'GripPartData': {'bEnabled': True, 'WeightedParts': [part('Grip.A'), part('Grip.B')]},
                          'SightPartData': {'bEnabled': True, 'WeightedParts': [part('Sight.A', 3)]}},
        'Leaf': {'BaseDefinition': 'Root', 'WeaponPartListCollection': 'Leaf.PartList'},
        'Leaf.PartList': {'PartReplacementMode': 'EPRM_Selective', 'ConsolidatedAttributeInitData': consolidated,
                          'SightPartData': {'bEnabled': True, 'WeightedParts': [
                              part('Sight.B', 3, [(None, 1)]), part('Sight.C', 3)]}},
    })


RUNTIME_DUMP = """*** Property dump for object 'WeaponPartListCollectionDefinition Leaf:WeaponPartListCollectionDefinition_1' ***
=== WeaponPartListCollectionDefinition properties ===
BodyPartData=(bEnabled=True,WeightedParts=((Part=WeaponPartDefinition'Body.A',Manufacturers=,MinGameStageIndex=0,MaxGameStageIndex=1,DefaultWeightIndex=2)))
GripPartData=(bEnabled=True,WeightedParts=((Part=WeaponPartDefinition'Grip.A',Manufacturers=,MinGameStageIndex=0,MaxGameStageIndex=1,DefaultWeightIndex=2),(Part=WeaponPartDefinition'Grip.B',Manufacturers=,MinGameStageIndex=0,MaxGameStageIndex=1,DefaultWeightIndex=2)))
SightPartData=(bEnabled=True,WeightedParts=((Part=WeaponPartDefinition'Sight.B',Manufacturers=((Manufacturer=None,DefaultWeightIndex=1)),MinGameStageIndex=0,MaxGameStageIndex=1,DefaultWeightIndex=3),(Part=WeaponPartDefinition'Sight.C',Manufacturers=,MinGameStageIndex=0,MaxGameStageIndex=1,DefaultWeightIndex=3)))
StockPartData=(bEnabled=False,WeightedParts=)
=== InventoryPartListCollectionDefinition properties ===
PartReplacementMode=EPRM_Additive
ConsolidatedAttributeInitData(0)=(BaseValueConstant=1.000000,BaseValueAttribute=None,InitializationDefinition=None,BaseValueScaleConstant=1.000000)
ConsolidatedAttributeInitData(1)=(BaseValueConstant=100.000000,BaseValueAttribute=None,InitializationDefinition=None,BaseValueScaleConstant=1.000000)
ConsolidatedAttributeInitData(2)=(BaseValueConstant=0.000000,BaseValueAttribute=None,InitializationDefinition=None,BaseValueScaleConstant=1.000000)
ConsolidatedAttributeInitData(3)=(BaseValueConstant=10.000000,BaseValueAttribute=None,InitializationDefinition=None,BaseValueScaleConstant=1.000000)
"""


class FakeDumps:
    def __init__(self, runtime_text):
        self.runtime_text = runtime_text

    def dump(self, name):
        return {'properties': {'RuntimePartListCollection': {'path': name + ':Runtime'}}}

    def text(self, name):
        return self.runtime_text


class LegalPartsTests(unittest.TestCase):
    def test_fixed_and_rolled_slots(self):
        result = B.legal_parts(world(), 'Leaf')
        self.assertEqual(result['chain'], ['Root', 'Leaf'])
        self.assertTrue(result['slots']['Body']['fixed'])
        self.assertFalse(result['slots']['Grip']['fixed'])
        # No Manufacturers list: the game's flat weight 100, whatever DefaultWeightIndex says (index 2 -> 0).
        self.assertFalse(result['slots']['Grip']['all_zero_weight'])
        self.assertEqual([c['share'] for c in result['slots']['Grip']['candidates']], [0.5, 0.5])
        self.assertEqual([c['part'] for c in result['slots']['Sight']['candidates']], ['Sight.B', 'Sight.C'])
        # Sight.B lists only Manufacturer=None (not a wildcard): its DefaultWeightIndex 3 -> 10; Sight.C -> 100.
        self.assertEqual([c['weight'] for c in result['slots']['Sight']['candidates']], [10, 100])
        self.assertEqual(result['combinations'], 1 * 2 * 2)

    def test_listed_manufacturer_with_zero_weight_empties_the_slot(self):
        package = world()
        package.objects['Root.PartList']['GripPartData']['WeightedParts'] = [
            part('Grip.A', 2, [('Maker', 2)]), part('Grip.B', 2, [('Other', 3)])]
        result = B.legal_parts(package, 'Leaf')
        self.assertTrue(result['slots']['Grip']['all_zero_weight'])


class CrosscheckTests(unittest.TestCase):
    def test_runtime_dump_parse_matches_our_merge(self):
        report = B.crosscheck(world(), FakeDumps(RUNTIME_DUMP), ['Leaf'])
        self.assertEqual((report['match'], report['differ']), (1, 0))
        self.assertEqual(report['entries_compared'], 5)

    def test_a_different_runtime_list_is_reported(self):
        changed = RUNTIME_DUMP.replace("Part=WeaponPartDefinition'Sight.C'", "Part=WeaponPartDefinition'Sight.X'")
        report = B.crosscheck(world(), FakeDumps(changed), ['Leaf'])
        self.assertEqual(report['differ'], 1)
        self.assertEqual(report['details']['Leaf']['Sight']['game'], ['Sight.B', 'Sight.X'])

    def test_weight_difference_is_reported_even_with_same_parts(self):
        changed = RUNTIME_DUMP.replace('ConsolidatedAttributeInitData(3)=(BaseValueConstant=10.000000',
                                       'ConsolidatedAttributeInitData(3)=(BaseValueConstant=20.000000')
        report = B.crosscheck(world(), FakeDumps(changed), ['Leaf'])
        self.assertTrue(report['details']['Leaf']['Sight']['same_parts_in_order'])


def trace_rows():
    obj = "ItemCardGFxObject'Transient.Card_0'"
    call = lambda method, args: {'phase': 'call', 'func': f'WillowGame.ItemCardGFxObject:{method}', 'obj': obj, 'args': args}
    return [
        call('SetCardUIStats', {'TopStats': [
            {'LabelText': 'Damage', 'ValueText': '120[projectilecount]x3[-projectilecount]'},
            {'LabelText': 'Fire Rate', 'ValueText': '2.5'},
            {'LabelText': 'Burn Damage / sec.', 'ValueText': '10.5'},
            {'LabelText': 'Ignite Chance', 'ValueText': '12.0%'}]}),
        call('SetTitle', {'Title': 'Test Gun', 'TypeIcon': 'Pistol', 'Manufacturer': 'maker', 'ElementalIcon': 'Fire'}),
        call('SetValue', {'Amount': 99}),
        call('SetLevelRequirement', {'RequirementText': 'LEVEL REQUIREMENT: 7'}),
        call('SetHeight', {}),
    ]


class CardAuditTests(unittest.TestCase):
    def test_weapon_cards_are_parsed_from_trace_rows(self):
        cards = A.weapon_cards(trace_rows() + trace_rows())  # duplicates collapse
        self.assertEqual(len(cards), 1)
        observed = A.observed(cards[0])
        self.assertEqual(observed, {'damage': 120.0, 'fire_rate': 2.5, 'projectiles': 3, 'status_dps': 10.5,
                                    'status_chance': 12.0, 'sale_value': 99, 'name': 'Test Gun'})
        self.assertEqual((cards[0]['type_icon'], cards[0]['element'], cards[0]['level']), ('pistol', 'fire', 7.0))

    def test_agreement_uses_the_printed_rounding(self):
        card = {'damage': 119.2, 'projectiles': 3, 'display': {'damage': 120, 'fire_rate': 2.5}}
        self.assertTrue(A.agrees('damage', card, 120.0))
        self.assertTrue(A.agrees('projectiles', card, 3))
        self.assertFalse(A.agrees('fire_rate', card, 2.4))
        self.assertFalse(A.agrees('status_chance', card, 12.0))


if __name__ == '__main__':
    unittest.main()
