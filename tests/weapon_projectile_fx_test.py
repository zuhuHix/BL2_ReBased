"""Synthetic tests for tools/weapon_projectile_fx.py: invented objects and numbers, no game data.

The particle reading (weapon_muzzle_fx.particle_system_layers) is replaced by a stub that names the system it was given.
Run: python tests/weapon_projectile_fx_test.py
"""
from pathlib import Path
import sys
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import weapon_projectile_fx as fx  # noqa: E402


class FakePackage:
    def __init__(self, objects):
        self.objects = objects
        self.index = objects

    def props(self, path):
        return self.objects[path]


def stub_layers(package, path):
    return [{'name': 'layer of ' + path}]


def world():
    return FakePackage({
        'Fake.Fx.Tracer': {}, 'Fake.Fx.Trail': {}, 'Fake.Fx.Boom_Small': {}, 'Fake.Fx.Boom_Big': {},
        'Fake.Projectiles.Rocket': {'InFlightEffects': 'Fake.Fx.Trail', 'InFlightEffectsScale': 0.25,
                                    'DefaultExplosionDefinition': 'Fake.Explosions.Default'},
        'Fake.Projectiles.Thrown': {'DefaultExplosionDefinition': 'Fake.Explosions.Default'},
        'Fake.Explosions.Boom': {'ExplosionScale': [
            {'ExplosionPSTemplate': 'Fake.Fx.Boom_Small', 'MinRadius': 0, 'MaxRadius': 100, 'MinParticleScale': 0.1,
             'MaxParticleScale': 0.5, 'ImpactEffect': 'Fake.Impacts.Scorch'},
            {'ExplosionPSTemplate': 'Fake.Fx.Boom_Big', 'MinRadius': 101, 'MaxRadius': 300, 'MinParticleScale': 0.4,
             'MaxParticleScale': 1.0}]},
        'Fake.Explosions.Default': {'ExplosionScale': [
            {'ExplosionPSTemplate': 'Fake.Fx.Boom_Small', 'MinRadius': 0, 'MaxRadius': 500}]},
    })


class ProjectileFxTests(unittest.TestCase):
    def setUp(self):
        self.saved = fx.weapon_muzzle_fx.particle_system_layers
        fx.weapon_muzzle_fx.particle_system_layers = stub_layers

    def tearDown(self):
        fx.weapon_muzzle_fx.particle_system_layers = self.saved

    def test_tracer_from_the_firing_mode_template(self):
        out = fx.build(world(), {'tracer_template': 'Fake.Fx.Tracer'})
        self.assertEqual(out['tracer']['layers'], [{'name': 'layer of Fake.Fx.Tracer'}])
        self.assertIsNone(out['trail'])
        self.assertIsNone(out['explosion'])

    def test_rocket_trail_and_its_splash_explosion_bands(self):
        firing = {'projectile': {'definition': 'Fake.Projectiles.Rocket',
                                 'splash': {'explosion': 'Fake.Explosions.Boom', 'radius': 250}}}
        out = fx.build(world(), firing)
        self.assertEqual(out['trail']['scale'], 0.25)
        self.assertEqual(out['trail']['layers'], [{'name': 'layer of Fake.Fx.Trail'}])
        bands = out['explosion']['bands']
        self.assertEqual([b['template'] for b in bands], ['Fake.Fx.Boom_Small', 'Fake.Fx.Boom_Big'])
        self.assertEqual((bands[1]['min_radius'], bands[1]['max_radius'], bands[1]['min_scale'], bands[1]['max_scale']),
                         (101, 300, 0.4, 1.0))
        self.assertEqual(bands[0]['impact_effect'], 'Fake.Impacts.Scorch')
        self.assertEqual(out['explosion']['radius'], 250)

    def test_impact_explosion_of_a_bullet_mode(self):
        out = fx.build(world(), {'impact_explosion': {'explosion': 'Fake.Explosions.Boom', 'radius': 106}})
        self.assertEqual(out['explosion']['definition'], 'Fake.Explosions.Boom')

    def test_a_splash_without_explosion_takes_the_projectile_default(self):
        firing = {'projectile': {'definition': 'Fake.Projectiles.Thrown', 'splash': {'explosion': None, 'radius': 120}}}
        out = fx.build(world(), firing)
        self.assertEqual(out['explosion']['definition'], 'Fake.Explosions.Default')
        self.assertEqual(out['explosion']['radius'], 120)

    def test_objects_missing_from_the_package_give_no_layers(self):
        out = fx.build(world(), {'tracer_template': 'Fake.Fx.Elsewhere',
                                 'impact_explosion': {'explosion': 'Fake.Explosions.Elsewhere'}})
        self.assertIsNone(out['tracer']['layers'])
        self.assertEqual(out['explosion']['bands'], [])


if __name__ == '__main__':
    unittest.main()
