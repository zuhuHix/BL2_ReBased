"""Synthetic tests for tools/weapon_gen_firing.py: invented blocks and numbers, no game data."""
import hashlib
import os
from pathlib import Path
import subprocess
import sys
import tempfile
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


class CompleteTest(unittest.TestCase):
    def test_each_shot_kind_needs_its_speed(self):
        self.assertEqual(tool.incomplete({'schema': 1, 'firing_mode': 'M', 'bullet_speed': 10.0}), [])
        self.assertEqual(tool.incomplete({'schema': 1, 'firing_mode': 'M', 'fire_type': 'EWWFT_HitScan'}), [])
        rocket = {'schema': 1, 'firing_mode': 'M', 'fire_type': 'EWWFT_Rocket', 'projectile': {'speed': 10.0}}
        self.assertEqual(tool.incomplete(rocket), [])
        for bad in ({}, {'speed': 0.0}, {'speed': None}, {'speed': float('inf')}, {'speed': float('nan')}):
            self.assertEqual(tool.incomplete(dict(rocket, projectile=bad)), ['projectile.speed'], bad)
        self.assertEqual(tool.incomplete(dict(rocket, projectile=None)), ['projectile.speed'])
        self.assertEqual(tool.incomplete({'schema': 1, 'firing_mode': 'M'}), ['bullet_speed'])

    def test_a_thrown_gun_needs_its_speed(self):
        template = {'schema': 1, 'firing_mode': 'M', 'bullet_speed': 10.0, 'reload_throw': {'projectile': {}}}
        self.assertEqual(tool.incomplete(template), ['reload_throw.projectile.speed'])

    def test_identity_is_required(self):
        self.assertIn('firing_mode', tool.incomplete({'schema': 1, 'bullet_speed': 10.0}))
        self.assertTrue(tool.incomplete({'schema': 2, 'firing_mode': 'M', 'bullet_speed': 10.0}))


class FingerprintTest(unittest.TestCase):
    def test_it_is_the_md5_of_the_bytes(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / 'manifest.txt'
            path.write_bytes(b'invented\r\nmanifest')
            self.assertEqual(tool.fingerprint(path), hashlib.md5(b'invented\r\nmanifest').hexdigest())

    def test_a_missing_manifest_names_its_tool(self):
        with tempfile.TemporaryDirectory() as folder:
            with self.assertRaises(SystemExit) as stop:
                tool.fingerprint(Path(folder) / 'none.txt')
        self.assertIn('weapon_gen_manifest.py', str(stop.exception.code))


class OutputPathTest(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.base = Path(temp.name).resolve()
        self.repo = self.base / 'repo'
        self.repo.mkdir()

    def refused(self, path):
        with self.assertRaises(SystemExit):
            tool.local_output(path, repo=self.repo)

    def test_inside_local_is_accepted(self):
        (self.repo / 'local').mkdir()
        self.assertEqual(tool.local_output(self.repo / 'local' / 'gen' / 'a.json', repo=self.repo), self.repo / 'local' / 'gen' / 'a.json')

    def test_a_tracked_path_is_refused_and_nothing_is_created(self):
        (self.repo / 'local').mkdir()
        self.refused(self.repo / 'tools' / 'gen' / 'a.json')
        self.assertFalse((self.repo / 'tools').exists())
        self.refused(self.repo / 'local' / '..' / 'tools' / 'a.json')
        self.refused(self.repo / 'a.json')
        self.refused(self.repo / 'local')
        self.refused(self.repo / 'localish' / 'a.json')

    @unittest.skipUnless(os.name == 'nt', 'junctions are a Windows feature')
    def test_a_junctioned_local_is_accepted_and_a_link_out_of_it_refused(self):
        store, outside = self.base / 'store', self.base / 'outside'
        store.mkdir()
        outside.mkdir()
        for link, target in ((self.repo / 'local', store), (store / 'escape', outside)):
            subprocess.run(['cmd', '/c', 'mklink', '/J', str(link), str(target)], check=True, capture_output=True)
            self.addCleanup(os.rmdir, link)
        self.assertEqual(tool.local_output(self.repo / 'local' / 'a.json', repo=self.repo), self.repo / 'local' / 'a.json')
        self.refused(self.repo / 'local' / 'escape' / 'a.json')
        self.refused(store / 'a.json')   # the target's own path is not the repo's local/

    @unittest.skipUnless(os.name == 'nt', 'junctions are a Windows feature')
    def test_the_schema_file_is_checked_too(self):
        outside = self.base / 'outside'
        outside.mkdir()
        (self.repo / 'local').mkdir()
        self.assertEqual(tool.output_paths(self.repo / 'local' / 'a.json', repo=self.repo),
                         (self.repo / 'local' / 'a.json', self.repo / 'local' / 'a.schema'))
        # The output is inside local/, its .schema neighbour is a link out of it.
        link = self.repo / 'local' / 'b.schema'
        subprocess.run(['cmd', '/c', 'mklink', '/J', str(link), str(outside)], check=True, capture_output=True)
        self.addCleanup(os.rmdir, link)
        self.assertEqual(tool.local_output(self.repo / 'local' / 'b.json', repo=self.repo), self.repo / 'local' / 'b.json')
        with self.assertRaises(SystemExit):
            tool.output_paths(self.repo / 'local' / 'b.json', repo=self.repo)


if __name__ == '__main__':
    unittest.main()
