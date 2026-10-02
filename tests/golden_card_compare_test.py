"""Synthetic tests for tools/real_game/golden_card_compare.py (invented values, no game data)."""
from pathlib import Path
import sys
import unittest
ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tools'))
sys.path.insert(0, str(ROOT / 'tools/real_game'))
import golden_card_compare as C  # noqa: E402


def entry(stats, value=100, title='Foo Bar', level_line='LEVEL REQUIREMENT: 5'):
    return {'cards': [{'top_stats': [{'label': k, 'value': v} for k, v in stats.items()],
                       'title': {'Title': title}, 'value': value, 'level_line': level_line}]}


def model(**display):
    return {'display': display, 'sale_value': 100}


class RealFields(unittest.TestCase):
    def test_projectile_count_and_status_rows(self):
        real = C.real_fields(entry({'Damage': '21[projectilecount]x7[-projectilecount]', 'Accuracy': '62.1',
                                    'Fire Rate': '17.7', 'Reload Speed': '2.2', 'Magazine Size': '2',
                                    'Burn Damage / sec.': '25.5', 'Ignite Chance': '15.6%'}))
        self.assertEqual(real['damage'], '21')
        self.assertEqual(real['projectiles'], '7')
        self.assertEqual(real['status_dps'], '25.5')
        self.assertEqual(real['status_chance'], '15.6%')
        self.assertEqual(real['sale_value'], '100')
        self.assertEqual(real['name'], 'Foo Bar')

    def test_number(self):
        self.assertEqual(C.number('15.6%'), 15.6)
        self.assertEqual(C.number('21x7'), 21)
        self.assertIsNone(C.number(None))


class Render(unittest.TestCase):
    def test_text_follows_card_style(self):
        text = C.render(model(damage=50, accuracy=93.5, fire_rate=1.7, reload_time=2.2, magazine=7,
                              status_chance=15.6, status_dps=25.5, projectiles=7), 8)
        self.assertEqual(text['damage'], '50')
        self.assertEqual(text['projectiles'], '7')
        self.assertEqual(text['fire_rate'], '1.7')
        self.assertEqual(text['magazine'], '7')
        self.assertEqual(text['status_chance'], '15.6%')
        self.assertEqual(text['level_line'], 'LEVEL REQUIREMENT: 8')

    def test_no_projectile_count_means_one_and_missing_values_stay_none(self):
        text = C.render({'display': {}, 'sale_value': None}, 3)
        self.assertEqual(text['projectiles'], '1')
        self.assertIsNone(text['damage'])
        self.assertIsNone(text['sale_value'])

    def test_name_skips_absent_part(self):
        self.assertEqual(C.name_from_parts('Red', 'Gun'), 'Red Gun')
        self.assertEqual(C.name_from_parts(None, 'Gun'), 'Gun')
        self.assertEqual(C.name_from_parts('Red', None), 'Red')


class Compare(unittest.TestCase):
    def rows(self, stats, **model_display):
        text = C.render(model(**model_display), 5)
        text['name'] = 'Foo Bar'
        return C.compare(text, C.real_fields(entry(stats)))

    def test_match_mismatch_and_numeric_diff(self):
        rows = self.rows({'Damage': '50', 'Fire Rate': '1.3', 'Magazine Size': '10'},
                         damage=50, fire_rate=1.2, magazine=9)
        self.assertEqual(rows['damage']['status'], 'match')
        self.assertEqual(rows['fire_rate']['status'], 'mismatch')
        self.assertAlmostEqual(rows['fire_rate']['diff'], -0.1)
        self.assertEqual(rows['magazine']['diff'], -1)
        self.assertEqual(rows['projectiles']['status'], 'match')

    def test_model_without_value_is_unsupported(self):
        text = C.render({'display': {}, 'sale_value': None}, 5)
        rows = C.compare(text, C.real_fields(entry({'Damage': '50'})))
        self.assertEqual(rows['damage']['status'], 'unsupported')
        self.assertEqual(rows['sale_value']['status'], 'unsupported')

    def test_status_rows_only_when_a_side_has_them(self):
        rows = self.rows({'Damage': '50'}, damage=50)
        self.assertNotIn('status_dps', rows)
        rows = self.rows({'Damage': '50'}, damage=50, status_dps=3.0)
        self.assertEqual(rows['status_dps']['status'], 'mismatch')

    def test_level_line_absent_on_the_real_card_is_a_mismatch(self):
        text = C.render(model(damage=50), 1)
        text['name'] = 'Foo Bar'
        rows = C.compare(text, C.real_fields(entry({'Damage': '50'}, level_line='')))
        self.assertEqual(rows['level_line']['status'], 'mismatch')
        self.assertEqual(rows['level_line']['real'], '')

    def test_host_name_only_when_given(self):
        text = C.render(model(damage=50), 5)
        text['name'] = 'Foo Bar'
        real = C.real_fields(entry({'Damage': '50'}))
        self.assertNotIn('host_name', C.compare(text, real))
        self.assertEqual(C.compare(text, real, host_name='Foo')['host_name']['status'], 'mismatch')
        self.assertEqual(C.compare(text, real, host_name='Foo Bar')['host_name']['status'], 'match')


class Summary(unittest.TestCase):
    def test_summarize_and_rollup(self):
        good = {'stratum': 'a', 'fields': {f: {'status': 'match'} for f in C.FIELDS}}
        bad = {'stratum': 'a', 'fields': {**good['fields'], 'magazine': {'status': 'mismatch'}}}
        table = C.summarize([good, bad], lambda r: r['stratum'])
        self.assertEqual(table['a']['magazine']['match'], 1)
        self.assertEqual(table['a']['magazine']['mismatch'], 1)
        self.assertEqual(C.rollup(good), {'main_four': True, 'printed_stats': True, 'every_field': True})
        self.assertEqual(C.rollup(bad), {'main_four': False, 'printed_stats': False, 'every_field': False})
        self.assertFalse(C.rollup({'fields': {}})['main_four'])

    def test_stratum_label(self):
        self.assertEqual(C.entry_stratum({'source': 'save'}), 'save')
        self.assertEqual(C.entry_stratum({'source': 'sdk_spawn', 'stratum': 'rarity'}), 'sdk_spawn:rarity')


if __name__ == '__main__':
    unittest.main()
