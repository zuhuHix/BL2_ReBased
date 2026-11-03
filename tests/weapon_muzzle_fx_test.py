"""Synthetic checks for the muzzle-flash and shell-casing particle reader.

All numbers are invented. The fake package answers props() and classes like
weapon_recipe.Package does for the decoded fields this reader uses.
"""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import weapon_muzzle_fx as fx

HEADER = [9.0, 9.0]  # the two header floats are skipped whatever their values


class FakePackage:
    def __init__(self, objects, classes=None):
        self.objects = objects
        self.classes = classes or {}

    def props(self, path):
        return self.objects[path]


class DistributionTests(unittest.TestCase):
    def test_constant_float_reads_its_value_once(self):
        result = fx.distribution({'LookupTable': HEADER + [0.25, 0.25, 0.25, 0.25]}, 1)
        self.assertEqual(result, {'min': [0.25], 'max': [0.25], 'samples': 1})

    def test_uniform_float_uses_low_and_high_halves(self):
        value = {'Op': 2, 'LookupTableChunkSize': 2, 'LookupTable': HEADER + [0.2, 0.6, 0.2, 0.6]}
        self.assertEqual(fx.distribution(value, 1), {'min': [0.2], 'max': [0.6], 'samples': 1})

    def test_constant_vector_without_chunk_size_uses_component_count(self):
        value = {'LookupTable': HEADER + [0.5, 0.25, 0.75, 0.5, 0.25, 0.75]}
        self.assertEqual(fx.distribution(value, 3),
                         {'min': [0.5, 0.25, 0.75], 'max': [0.5, 0.25, 0.75], 'samples': 1})

    def test_uniform_vector_op2_takes_componentwise_range(self):
        value = {'Op': 2, 'LookupTableChunkSize': 6,
                 'LookupTable': HEADER + [2, 8, 1, 6, 4, 3, 2, 8, 1, 6, 4, 3]}
        self.assertEqual(fx.distribution(value, 3),
                         {'min': [2, 4, 1], 'max': [6, 8, 3], 'samples': 1})

    def test_uniform_vector_op3_reads_the_same_way(self):
        value = {'Op': 3, 'LookupTableChunkSize': 6,
                 'LookupTable': HEADER + [-4, 1, 0, 2, -1, 5]}
        self.assertEqual(fx.distribution(value, 3),
                         {'min': [-4, -1, 0], 'max': [2, 1, 5], 'samples': 1})

    def test_curve_uses_first_sample_and_counts_samples(self):
        value = {'LookupTableChunkSize': 3, 'LookupTableTimeScale': 10.0,
                 'LookupTable': HEADER + [1, 0, 0, 0, 1, 0, 0, 0, 1]}
        self.assertEqual(fx.distribution(value, 3),
                         {'min': [1, 0, 0], 'max': [1, 0, 0], 'samples': 3})

    def test_unresolved_object_distribution_has_no_table(self):
        self.assertIsNone(fx.distribution({'Distribution': 'Synthetic.Dist', 'LookupTable': []}, 1))
        self.assertIsNone(fx.distribution(None, 1))


class LayerTests(unittest.TestCase):
    def modules(self):
        return {
            'ParticleModuleLifetime': {'Lifetime': {'LookupTable': HEADER + [0.1, 0.1]}},
            'ParticleModuleSize': {'StartSize': {'Op': 3, 'LookupTableChunkSize': 6,
                                                 'LookupTable': HEADER + [40, 3, 0, 20, -5, 0]}},
            'ParticleModuleLocation': {'StartLocation': {'LookupTable': HEADER + [12, 0, 0]}},
            'ParticleModuleColor': {'StartColor': {'LookupTable': HEADER + [2, 1, 0.5]}},
            'ParticleModuleOrientationAxisLock': {'LockAxisFlags': 'EPAL_Y'},
        }

    def test_layer_fields_from_modules(self):
        required = {'Material': 'Synthetic.Materials.Mat_Spark', 'ScreenAlignment': 'PSA_Rectangle'}
        spawn = {'BurstList': [{'Count': 1}, {'Count': 2}]}
        result = fx.layer('Synthetic_Side', required, self.modules(), spawn)
        self.assertEqual(result['name'], 'Synthetic_Side')
        self.assertEqual(result['material'], 'Mat_Spark')
        self.assertEqual(result['alignment'], 'PSA_Rectangle')
        self.assertEqual(result['axis'], 'EPAL_Y')
        self.assertEqual(result['life'], [0.1, 0.1])
        self.assertEqual(result['size_x'], [20.0, 40.0])
        self.assertEqual(result['size_y'], [0.0, 5.0])  # signed range -5..3 gives magnitude 0..5
        self.assertEqual(result['offset_x'], 12.0)
        self.assertEqual(result['color'], [2.0, 1.0, 0.5])
        self.assertEqual(result['alpha'], [1.0, 1.0])  # no StartAlpha: engine default 1
        self.assertFalse(result['smoke'])
        self.assertEqual(result['burst'], 3)

    def test_smoke_flag_and_default_alignment(self):
        required = {'Material': 'Synthetic.Materials.Wispy_Smoke_Cloud'}
        result = fx.layer('Synthetic_Smoke', required, {}, {})
        self.assertEqual(result['alignment'], 'PSA_Square')
        self.assertTrue(result['smoke'])
        self.assertIsNone(result['life'])
        self.assertIsNone(result['axis'])
        self.assertEqual(result['burst'], 0)

    def test_colour_falls_back_to_colour_over_life_first_sample(self):
        modules = {'ParticleModuleColorOverLife': {'ColorOverLife': {
            'LookupTable': HEADER + [0.6, 0.3, 0.1, 0.0, 0.0, 0.0]}}}
        result = fx.layer('Synthetic_Glow', {}, modules, {})
        self.assertEqual(result['color'], [0.6, 0.3, 0.1])

    def test_negative_only_size_gives_magnitude_range(self):
        modules = {'ParticleModuleSize': {'StartSize': {'Op': 2, 'LookupTableChunkSize': 6,
                                                        'LookupTable': HEADER + [-30, 0, 0, -20, 0, 0, -30, 0, 0, -20, 0, 0]}}}
        result = fx.layer('Synthetic_Neg', {}, modules, {})
        self.assertEqual(result['size_x'], [20.0, 30.0])


class PackageMappingTests(unittest.TestCase):
    def package(self):
        objects = {
            'Synthetic.Effects.Collection': {'ParticleEffects': [
                {'Expression': 'Synthetic.Eval_Corrosive', 'ParticleEffect': 'Synthetic.Particles.Corrosive'},
                {'Expression': 'Synthetic.Eval_Amp', 'ParticleEffect': 'Synthetic.Particles.Etec'},
                {'Expression': 'Synthetic.Eval_Incendiary', 'ParticleEffect': 'Synthetic.Particles.Fire'},
                {'Expression': None, 'ParticleEffect': 'Synthetic.Particles.Default'},
            ]},
            'Synthetic.Eval_Corrosive': {'Expression': {
                'AttributeOperand1': 'D_Attributes.WeaponDamageType.WeaponDamage_Is_Corrosive', 'ConstantOperand2': 1}},
            'Synthetic.Eval_Amp': {'Expression': {
                'AttributeOperand1': 'D_Attributes.WeaponDamageType.WeaponDamage_Is_Amp', 'ConstantOperand2': 1}},
            'Synthetic.Eval_Incendiary': {'Expression': {
                'AttributeOperand1': 'D_Attributes.WeaponDamageType.WeaponDamage_Is_Incendiary', 'ConstantOperand2': 1}},
            'Synthetic.Particles.Corrosive': {'Emitters': ['Synthetic.Particles.Corrosive.Emit_A',
                                                           'Synthetic.Particles.Corrosive.Emit_Empty']},
            'Synthetic.Particles.Corrosive.Emit_A': {'EmitterName': 'Emit_A',
                                                     'LODLevels': ['Synthetic.Particles.Corrosive.LOD_A']},
            'Synthetic.Particles.Corrosive.Emit_Empty': {'EmitterName': 'Emit_Empty', 'LODLevels': []},
            'Synthetic.Particles.Corrosive.LOD_A': {
                'RequiredModule': 'Synthetic.Particles.Corrosive.Req_A',
                'SpawnModule': 'Synthetic.Particles.Corrosive.Spawn_A',
                'Modules': ['Synthetic.Particles.Corrosive.Life_A']},
            'Synthetic.Particles.Corrosive.Req_A': {'Material': 'Synthetic.Materials.Mat_Acid'},
            'Synthetic.Particles.Corrosive.Spawn_A': {'BurstList': [{'Count': 4}]},
            'Synthetic.Particles.Corrosive.Life_A': {'Lifetime': {'LookupTable': HEADER + [0.05, 0.05]}},
            'Synthetic.Particles.Etec': {'Emitters': []},
            'Synthetic.Particles.Fire': {'Emitters': []},
            'Synthetic.Particles.Default': {'Emitters': []},
        }
        classes = {'Synthetic.Particles.Corrosive.Life_A': 'Engine.ParticleModuleLifetime'}
        return FakePackage(objects, classes)

    def test_collection_maps_damage_expressions_and_default(self):
        result = fx.muzzle_collection(self.package(), 'Synthetic.Effects.Collection')
        self.assertEqual(result, {
            'Corrosive': 'Synthetic.Particles.Corrosive',
            'Slag': 'Synthetic.Particles.Etec',
            'Fire': 'Synthetic.Particles.Fire',
            'Default': 'Synthetic.Particles.Default',
        })

    def test_system_layers_skip_emitters_without_lod_and_read_modules_by_class(self):
        layers = fx.particle_system_layers(self.package(), 'Synthetic.Particles.Corrosive')
        self.assertEqual(len(layers), 1)
        self.assertEqual(layers[0]['name'], 'Emit_A')
        self.assertEqual(layers[0]['material'], 'Mat_Acid')
        self.assertEqual(layers[0]['life'], [0.05, 0.05])
        self.assertEqual(layers[0]['burst'], 4)

    def test_casing_motion_reads_velocity_acceleration_and_spaces(self):
        objects = {
            'Synthetic.Particles.Casing': {'Emitters': ['Synthetic.Particles.Casing.Emit']},
            'Synthetic.Particles.Casing.Emit': {'EmitterName': 'Casing',
                                                'LODLevels': ['Synthetic.Particles.Casing.LOD']},
            'Synthetic.Particles.Casing.LOD': {
                'RequiredModule': 'Synthetic.Particles.Casing.Req',
                'SpawnModule': None,
                'Modules': ['Synthetic.Particles.Casing.Life', 'Synthetic.Particles.Casing.Vel',
                            'Synthetic.Particles.Casing.Acc']},
            'Synthetic.Particles.Casing.Req': {'bUseLocalSpace': True},
            'Synthetic.Particles.Casing.Life': {'Lifetime': {'LookupTable': HEADER + [0.3, 0.3]}},
            'Synthetic.Particles.Casing.Vel': {'StartVelocity': {
                'Op': 2, 'LookupTableChunkSize': 6,
                'LookupTable': HEADER + [30, 10, 5, 20, 2, 1, 30, 10, 5, 20, 2, 1]}},
            'Synthetic.Particles.Casing.Acc': {'Acceleration': {
                'Op': 2, 'LookupTableChunkSize': 6,
                'LookupTable': HEADER + [0, 0, -90, 0, 0, -60, 0, 0, -90, 0, 0, -60]},
                'bAlwaysInWorldSpace': True},
        }
        classes = {
            'Synthetic.Particles.Casing.Life': 'Engine.ParticleModuleLifetime',
            'Synthetic.Particles.Casing.Vel': 'Engine.ParticleModuleVelocity',
            'Synthetic.Particles.Casing.Acc': 'Engine.ParticleModuleAcceleration',
        }
        result = fx.casing_fx(FakePackage(objects, classes), 'Synthetic.Particles.Casing')
        self.assertEqual(result, {
            'system': 'Synthetic.Particles.Casing',
            'life': [0.3, 0.3],
            'start_velocity': {'min': [20, 2, 1], 'max': [30, 10, 5]},
            'acceleration': {'min': [0, 0, -90], 'max': [0, 0, -60]},
            'acceleration_world': True,
            'local_space': True,
        })


if __name__ == '__main__':
    unittest.main()
