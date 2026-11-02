"""Synthetic test for tools/real_game/gen_confirm.py: sampling, spawn fields, card expectation vs live text,
classification inputs, chi-square. Invented names and values only (no game data). Run directly:
python tests/gen_confirm_test.py
"""
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tools' / 'real_game'))
import gen_confirm as G  # noqa: E402


def gun(**over):
    base = {
        'balance': 'GD_Test.Bal.Pistol_A', 'stage': 15, 'weapon_type': 'GD_Test.WT_Pistol',
        'manufacturer': 'GD_Test.Maker.Alpha', 'name': 'Quick Demo',
        'parts': {'Body': {'part': 'GD_Test.Body.B1'}, 'Grip': {'part': 'GD_Test.Grip.G2'},
                  'Stock': None, 'Elemental': {'part': 'GD_Test.Elem.Fire'}},
        'prefix': {'part': 'GD_Test.Prefix.Quick', 'text': 'Quick'},
        'title': {'part': 'GD_Test.Title.Demo', 'text': 'Demo'},
        'card': {'level_requirement': 15, 'level_line': True, 'sale_value': 120, 'sale_value_known': True,
                 'rarity_color': '#ffb400', 'projectiles': 1, 'element': 'Fire',
                 'display': {'damage': 49.13, 'magazine': 6.9, 'fire_rate': 2.25, 'reload_time': 1.75,
                             'accuracy': 89.96, 'status_dps': 40.0, 'status_chance': 15.56}},
    }
    base.update(over)
    return base


def live_fill(**over):
    """A card fill shaped like golden_cards.card_fills output (values as the card prints them)."""
    fill = {'seq': 1, 'item': "WillowWeapon'Loader.TheWorld:PersistentLevel.WillowWeapon_9'",
            'compare': None,
            'top_stats': [
                {'label': 'Damage', 'value': '50', 'icon': 'weaponDamage'},
                {'label': 'Accuracy', 'value': '90.0', 'icon': 'weaponAccuracy'},
                {'label': 'Fire Rate', 'value': '2.2', 'icon': 'weaponFireRate'},
                {'label': 'Reload Speed', 'value': '1.8', 'icon': 'weaponsReloadSpeed'},
                {'label': 'Magazine Size', 'value': '7', 'icon': 'weaponClipSize'},
                {'label': 'Fire Damage / sec.', 'value': '40.0', 'icon': 'x'},
                {'label': 'Fire Chance', 'value': '15.6%', 'icon': 'x'}],
            'title': {'Title': 'Quick Demo', 'Rarity': {'R': 255, 'G': 180, 'B': 0, 'A': 255}},
            'value': 120, 'level_line': 'LEVEL REQUIREMENT: 15'}
    fill.update(over)
    return fill


class Sampling(unittest.TestCase):
    def setUp(self):
        balances = []
        for kind in G.TYPES:
            for maker in ('Bandit', 'Dahl', 'Torgue'):
                for i in range(3):
                    balances.append({'balance': f'GD.{kind}.{maker}_{i}', 'type': f'WT_{maker}_{kind}',
                                     'type_short': kind, 'manufacturer': f'GD.Maker.{maker}', 'mission': False,
                                     'rarity': 'Common'})
            balances.append({'balance': f'GD.{kind}.A_Weapons_Legendary.X', 'type': f'WT_{kind}', 'type_short': kind,
                             'manufacturer': 'GD.Maker.Bandit', 'mission': False, 'rarity': 'Legendary'})
        balances.append({'balance': 'GD.Mission.Pistol', 'type': 'WT_Pistol', 'type_short': 'Pistol',
                         'manufacturer': None, 'mission': True, 'rarity': 'Common'})
        self.balances = balances

    def test_type_and_rarity_tags(self):
        self.assertEqual(G.type_short('WeaponType_Maliwan_Pistol'), 'Pistol')
        self.assertEqual(G.type_short('GD_Weap_SMG.A_Weapons.WT_SMG_Hyperion'), 'SMG')
        self.assertEqual(G.rarity_tag('GD_Weap_Pistol.A_Weapons_Legendary.Pistol_X'), 'Legendary')
        self.assertEqual(G.rarity_tag('GD_Weap_Pistol.A_Weapons.Pistol_X'), 'Common')

    def test_plan_covers_every_type_and_skips_missions(self):
        requests = G.plan_sample(self.balances, count=40)
        self.assertEqual(len(requests), 40)
        self.assertEqual({r['type_short'] for r in requests}, set(G.TYPES))
        self.assertNotIn('GD.Mission.Pistol', {r['balance'] for r in requests})
        self.assertEqual({r['stage'] for r in requests}, set(G.STAGES))
        self.assertEqual(len({r['seed'] for r in requests}), 40)
        self.assertGreaterEqual(sum(r['rarity'] == 'Legendary' for r in requests), len(G.TYPES))

    def test_definition_fields_carry_exact_parts_and_names(self):
        fields = G.definition_fields(gun())
        self.assertEqual(fields['GameStage'], 15)
        self.assertEqual(fields['ManufacturerGradeIndex'], 15)
        self.assertEqual(fields['BodyPartDefinition'], 'GD_Test.Body.B1')
        self.assertEqual(fields['ElementalPartDefinition'], 'GD_Test.Elem.Fire')
        self.assertNotIn('StockPartDefinition', fields)  # an empty slot is left out, not set to None
        self.assertEqual(fields['PrefixPartDefinition'], 'GD_Test.Prefix.Quick')
        self.assertEqual(fields['TitlePartDefinition'], 'GD_Test.Title.Demo')


class Comparison(unittest.TestCase):
    def test_expected_card_uses_display_numbers_and_card_rules(self):
        exp = G.expected_card(gun())
        self.assertEqual(exp['damage'], '49')          # the generator shows the card's integer (already rounded)
        self.assertEqual(exp['magazine'], '6')
        self.assertEqual(exp['accuracy'], '90.0')
        self.assertEqual(exp['status_chance'], '15.6%')
        self.assertEqual(exp['level_line'], 'LEVEL REQUIREMENT: 15')
        self.assertEqual(exp['rarity_colour'], '#FFB400')

    def test_live_card_text_matches_expected(self):
        record = {'object': 'WillowWeapon_9', 'name': 'Quick Demo'}
        got = G.live_card(live_fill(), record)
        exp = G.expected_card(gun(card=dict(gun()['card'], display=dict(
            gun()['card']['display'], damage=50.0, magazine=7.0))))
        rows = G.compare_card(exp, got)
        self.assertEqual(rows['rarity_colour']['status'], 'match')
        self.assertEqual(rows['damage']['status'], 'match')
        self.assertEqual(rows['projectiles']['status'], 'match')  # no projectile marker: '1' on both sides
        self.assertEqual(rows['status_chance']['status'], 'match')
        self.assertEqual(rows['magazine']['status'], 'match')
        self.assertEqual(G.compare_card(exp, dict(got, status_chance='15.5%'))['status_chance']['status'], 'mismatch')

    def test_mismatch_and_absent_fields(self):
        exp = G.expected_card(gun(card=dict(gun()['card'], level_line=False, sale_value_known=False)))
        live = G.live_card(live_fill(level_line=''), {'object': 'x'})
        rows = G.compare_card(exp, live)
        self.assertEqual(rows['sale_value']['status'], 'unsupported')  # the card printed 120, the model has none
        self.assertEqual(rows['level_line']['status'], 'match')
        bad = dict(live, damage='51')
        self.assertEqual(G.compare_card(exp, bad)['damage']['status'], 'mismatch')

    def test_unsupported_when_model_has_no_value(self):
        exp = G.expected_card(gun())
        exp['accuracy'] = None
        self.assertEqual(G.compare_card(exp, G.live_card(live_fill(), {'object': 'x'}))['accuracy']['status'],
                         'unsupported')


class Frequency(unittest.TestCase):
    def test_fair_die_is_consistent(self):
        observed = {'a': 250, 'b': 250}
        stat, df, p, hits = G.chi_square(observed, {'a': 0.5, 'b': 0.5})
        self.assertAlmostEqual(stat, 0.0)
        self.assertEqual(df, 1)
        self.assertGreater(p, 0.9)
        self.assertEqual(G.frequency_verdict(stat, df, p, hits), 'consistent')

    def test_skewed_counts_differ(self):
        stat, df, p, hits = G.chi_square({'a': 400, 'b': 100}, {'a': 0.5, 'b': 0.5})
        self.assertLess(p, 0.001)
        self.assertEqual(G.frequency_verdict(stat, df, p, hits), 'differs')

    def test_impossible_category_is_reported(self):
        stat, df, p, hits = G.chi_square({'a': 90, 'z': 10}, {'a': 1.0})
        self.assertEqual(hits, {'z': 10})
        self.assertEqual(G.frequency_verdict(stat, df, p, hits), 'differs (impossible category hit)')

    def test_small_expected_cells_are_pooled(self):
        observed = {'a': 990, 'b': 4, 'c': 3, 'd': 3}
        expected = {'a': 0.99, 'b': 0.004, 'c': 0.003, 'd': 0.003}  # all three rare parts expect < 5
        stat, df, p, hits = G.chi_square(observed, expected)
        self.assertEqual(df, 1)  # 'a' and one pooled cell for the three rare parts
        self.assertEqual(hits, {})


class EvaluatedStats(unittest.TestCase):
    """Live evaluated stats (no card UI) against the generator's unrounded values: relations and classes."""

    def test_float_relation_bitwise_ulp_and_larger(self):
        self.assertEqual(G.float_relation(1.5, 1.5), 'bitwise')
        self.assertEqual(G.float_relation(0.1, 0.1 + 1e-9), 'bitwise')  # below single-precision resolution
        self.assertEqual(G.float_relation(0.1, 0.1 + 1e-6), 'larger')
        nxt = G.weapon_recipe.f32(1.0) + 2 ** -23  # the next single-precision value above 1.0
        self.assertEqual(G.float_relation(1.0, nxt), 'ulp1')
        self.assertEqual(G.float_relation(1.0, 1.0 + 2 ** -20), 'larger')
        self.assertEqual(G.float_relation(None, None), 'na')
        self.assertEqual(G.float_relation(2.0, None), 'absent')

    def test_integer_fields_compare_exactly(self):
        self.assertEqual(G.stat_relation('sale_value', 797, 797.0), 'bitwise')
        self.assertEqual(G.stat_relation('projectiles', 1, 2.0), 'larger')

    def test_live_values_fire_rate_is_inverse_interval_in_single_precision(self):
        row = {'equipped': {'attrs': {'FireInterval': {'Value': 0.2, 'BaseValue': 0.2},
                                      'InstantHitDamage': {'Value': 44.5, 'BaseValue': 40.0},
                                      'StatusEffectChance': 12.0},
                            'methods': {'GetRarityLevel': 9, 'GetControllerPlayerExpLevelRequiredToUse': {'error': 'X'}}}}
        vals = G.live_values(row, {'sale_value': 797})
        self.assertEqual(vals['fire_rate'], G.weapon_recipe.f32(1.0 / 0.2))
        self.assertEqual(vals['damage'], 44.5)  # the evaluated Value, not the base value
        self.assertEqual(vals['status_chance'], 12.0)
        self.assertEqual(vals['rarity_level'], 9.0)
        self.assertIsNone(vals['level_requirement'])  # an error reading is absent, not guessed
        self.assertIsNone(vals['spread'])

    def test_classify_runtime_data_rule_failure_and_unsupported(self):
        self.assertEqual(G.classify_stat('damage', 40.0, 44.5, 44.5), 'runtime-data')
        self.assertEqual(G.classify_stat('damage', 40.0, 41.0, 44.5), 'rule-failure')
        self.assertEqual(G.classify_stat('damage', 40.0, 44.5, None), 'unsupported')
        self.assertIsNone(G.classify_stat('damage', 44.5, 44.5, 44.5))


if __name__ == '__main__':
    unittest.main()
