"""Synthetic callback transactions: interleaving, incomplete cards and unknown types."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]/'tools'))
from prepare_inventory_gear import observed_cards


def row(seq, obj, method, args):
    return {'seq': seq, 'obj': obj, 'phase': 'call',
            'func': 'Synthetic.ItemCardGFxObject:'+method, 'args': args}


class GearObservationTest(unittest.TestCase):
    def test_observed_flavour_formatting_is_preserved(self):
        markup = '• <font color="#ffffff">Bonus</font>\n• <font color="#dc4646">Flavour</font>'
        rows = [row(1, 'A', 'SetCardUIStats', {'TopStats':[]}),
                row(2, 'A', 'SetTitle', {'Title':'Fixture','TypeIcon':'comm'}),
                row(3, 'A', 'SetFunStats', {'FunStatsText':markup}),
                row(4, 'A', 'SetHeight', {})]
        card = observed_cards(rows, 'synthetic')[0]
        self.assertEqual(card['funStatsMarkup'], markup)
        self.assertEqual(card['funStats'], 'Bonus; Flavour')

    def test_interleaved_objects_do_not_mix_stats(self):
        rows = [row(1, 'A', 'SetCardUIStats', {'TopStats':[{'LabelText':'A stat','ValueText':'+2%','IconName':'A_icon'}]}),
                row(2, 'B', 'SetCardUIStats', {'TopStats':[]}),
                row(3, 'A', 'SetTitle', {'Title':'Fixture A','TypeIcon':'Artifact'}),
                row(4, 'B', 'SetTitle', {'Title':'Fixture B','TypeIcon':'comm'}),
                row(5, 'A', 'SetValue', {'Amount':12}),
                row(6, 'B', 'SetHeight', {}), row(7, 'A', 'SetHeight', {})]
        cards = observed_cards(rows, 'synthetic')
        self.assertEqual([c['name'] for c in cards], ['Fixture B','Fixture A'])
        self.assertEqual(cards[0]['stats'], [])
        self.assertEqual(cards[1]['stats'], [{'label':'A stat','value':'+2%','icon':'A_icon'}])
        self.assertNotIn('saleValue', cards[0])
        self.assertEqual(cards[1]['saleValue'], 12)

    def test_new_transaction_discards_incomplete_fields(self):
        rows = [row(1, 'A', 'SetCardUIStats', {'TopStats':[]}),
                row(2, 'A', 'SetValue', {'Amount':999}),
                row(3, 'A', 'SetCardUIStats', {'TopStats':[]}),
                row(4, 'A', 'SetTitle', {'Title':'New fixture','TypeIcon':'Shield'}),
                row(5, 'A', 'SetLevelRequirement', {'bHasRequirement':True,'RequirementText':'LEVEL REQUIREMENT: 7'}),
                row(6, 'A', 'SetHeight', {})]
        card = observed_cards(rows, 'synthetic')[0]
        self.assertNotIn('saleValue', card)
        self.assertNotIn('assetId', card)
        self.assertEqual(card['level'], 7)
        self.assertFalse(card['provenance']['packageResolved'])

    def test_unknown_or_incomplete_card_is_not_exported(self):
        rows = [row(1, 'A', 'SetCardUIStats', {'TopStats':[]}),
                row(2, 'A', 'SetTitle', {'Title':'Unknown','TypeIcon':'Unknown'}),
                row(3, 'A', 'SetHeight', {}),
                row(4, 'B', 'SetCardUIStats', {'TopStats':[]}),
                row(5, 'B', 'SetTitle', {'Title':'Incomplete','TypeIcon':'comm'})]
        self.assertEqual(observed_cards(rows, 'synthetic'), [])


if __name__ == '__main__':
    unittest.main()
