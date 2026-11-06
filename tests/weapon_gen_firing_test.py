"""Synthetic tests for tools/weapon_gen_firing.py: invented blocks and numbers, no game data."""
from pathlib import Path
import sys
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import weapon_gen_firing as tool


def block(damage=100.0, multiplier=1.0, splash_damage=None, scale=None, pool_min=1.0):
    """An invented resolved firing block for a roll with this card damage and speed multiplier."""
    return {
        'schema': 1, 'firing_mode': 'Fake.Mode', 'kind': 'projectile', 'bullet_speed': 2000.0 * multiplier,
        'projectile': {'speed': 1000.0 * multiplier, 'splash': {'radius': 300.0, 'damage': splash_damage, 'damage_scale': scale}},
        'impact_explosion': None, 'reload_throw': None, 'recoil': {'kick_up': 1.0},
        'accuracy_pool': {'min': pool_min, 'max': 9.0, 'idle_rate': 3.0, 'modifiers': {'a': 1}, 'base': {'min': 2.0, 'max': 8.0, 'idle_rate': 4.0}},
        'damage': damage, 'pellets': 3, 'spread': 0.5, 'burst_count': 0, 'status': None, 'sources': {}, 'unverified': [],
    }


CARD = {'damage': 100.0, 'element': None, 'firing_mode': 'Fake.Mode'}


def add(collector, roll, card=None, multiplier=1.0, weapon_type='Fake.Type'):
    collector.add(roll, dict(CARD, **(card or {})), {tool.SPEED_MULTIPLIER: multiplier}, weapon_type, 'roll')


class TemplateTest(unittest.TestCase):
    def test_roll_fields_stay_out(self):
        template = tool.template_of(block(), 100.0, 1.0)
        for field in ('damage', 'pellets', 'spread', 'burst_count', 'status'):
            self.assertNotIn(field, template)
        self.assertEqual(template['recoil'], {'kick_up': 1.0})

    def test_speeds_are_stored_before_the_multiplier(self):
        template = tool.template_of(block(multiplier=2.0), 100.0, 2.0)
        self.assertEqual(template['bullet_speed'], 2000.0)
        self.assertEqual(template['projectile']['speed'], 1000.0)

    def test_an_evaluated_splash_becomes_a_scale(self):
        splash = tool.template_of(block(splash_damage=50.0), 100.0, 1.0)['projectile']['splash']
        self.assertIsNone(splash['damage'])
        self.assertEqual(splash['damage_scale'], 0.5)

    def test_the_pool_is_the_base(self):
        pool = tool.template_of(block(), 100.0, 1.0)['accuracy_pool']
        self.assertEqual((pool['min'], pool['max'], pool['idle_rate']), (2.0, 8.0, 4.0))
        self.assertNotIn('modifiers', pool)

    def test_apply_roll_scales_speed_and_splash(self):
        merged = tool.apply_roll(tool.template_of(block(splash_damage=50.0), 100.0, 1.0), {'damage': 300.0, 'speed_multiplier': 2.0})
        self.assertEqual(merged['bullet_speed'], 4000.0)
        self.assertEqual(merged['projectile']['speed'], 2000.0)
        self.assertEqual(merged['projectile']['splash']['damage'], 150.0)

    def test_null_scale_means_the_card_damage(self):
        merged = tool.apply_roll(tool.template_of(block(), 100.0, 1.0), {'damage': 300.0})
        self.assertEqual(merged['projectile']['splash']['damage'], 300.0)


class CollectorTest(unittest.TestCase):
    def test_rolls_that_scale_merge_into_one_template(self):
        collector = tool.Collector()
        add(collector, block(splash_damage=50.0))
        add(collector, block(damage=300.0, multiplier=2.0, splash_damage=150.0), {'damage': 300.0}, 2.0)
        self.assertEqual(len(collector.templates), 1)
        self.assertEqual(collector.problems, [])

    def test_the_pool_may_vary(self):
        collector = tool.Collector()
        add(collector, block())
        add(collector, block(pool_min=5.0))
        self.assertEqual(collector.problems, [])

    def test_a_splash_that_does_not_follow_the_card_is_a_problem(self):
        collector = tool.Collector()
        add(collector, block(splash_damage=50.0))
        add(collector, block(damage=300.0, splash_damage=50.0), {'damage': 300.0})
        self.assertTrue(any('projectile.splash.damage' in text for text in collector.problems), collector.problems)

    def test_any_other_difference_is_a_problem(self):
        collector = tool.Collector()
        add(collector, block())
        other = block()
        other['recoil'] = {'kick_up': 2.0}
        add(collector, other)
        self.assertTrue(any('recoil.kick_up' in text for text in collector.problems), collector.problems)

    def test_another_key_is_another_template(self):
        collector = tool.Collector()
        add(collector, block())
        add(collector, block(), {'element': 'Fire'})
        add(collector, block(), weapon_type='Fake.Other')
        self.assertEqual(len(collector.templates), 3)
        self.assertEqual(collector.problems, [])

    def test_the_card_must_name_the_same_firing_mode(self):
        collector = tool.Collector()
        add(collector, block(), {'firing_mode': 'Fake.Other'})
        self.assertTrue(any('firing mode' in text for text in collector.problems))


if __name__ == '__main__':
    unittest.main()
