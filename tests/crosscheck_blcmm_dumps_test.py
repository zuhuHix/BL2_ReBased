"""Synthetic fixtures for the OpenBLCMM dump cross-check.

Every dump-side value here is hand-written in the shape the game's ``obj dump``
prints; no dump text or game data is copied into the repository.
"""
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
from crosscheck_blcmm_dumps import (actor_matrix, bounds_of, compare_actor, compare_bsp,
                                    compare_material_parameters, compare_terrain, compose, dump_matrix,
                                    effective_parameters, rotation_matrix, transform_matrix, transform_point)


def planes(rows, translation):
    """dict(rows, translation) -> the ``_LocalToWorld`` struct shape a dump parses into."""
    names = ('XPlane', 'YPlane', 'ZPlane')
    out = {name: dict(W=0.0, **{axis: rows[i][j] for j, axis in enumerate('XYZ')}) for i, name in enumerate(names)}
    out['WPlane'] = dict(W=1.0, **{axis: translation[j] for j, axis in enumerate('XYZ')})
    return out


def bounds(origin, extent):
    return dict(Origin={axis: origin[i] for i, axis in enumerate('XYZ')},
                BoxExtent={axis: extent[i] for i, axis in enumerate('XYZ')},
                SphereRadius=0.0)


class Stub:
    """Stands in for ``blcmm_dumps.Dumps`` with a fixed name -> properties mapping."""

    def __init__(self, objects):
        self.objects = objects

    def dump(self, name):
        properties = self.objects.get(name)
        return None if properties is None else dict(**{'class': 'Stub'}, name=name, properties=properties)


class MatrixTest(unittest.TestCase):
    def test_rotation_matrix_matches_known_angles(self):
        # A 90 degree yaw sends +X to +Y and +Y to -X.
        rows = rotation_matrix(0.0, 90.0, 0.0)
        self.assertAlmostEqual(rows[0][1], 1.0)
        self.assertAlmostEqual(rows[1][0], -1.0)
        self.assertAlmostEqual(rows[2][2], 1.0)
        self.assertAlmostEqual(max(abs(v) for v in (rows[0][0], rows[0][2], rows[1][1])), 0.0, places=12)

    def test_scale_multiplies_rows_and_translation_is_the_location(self):
        matrix = transform_matrix([10.0, 20.0, 30.0], [0.0, 0.0, 0.0], [2.0, 3.0, 4.0])
        self.assertEqual(matrix['rows'][0][0], 2.0)
        self.assertEqual(matrix['rows'][1][1], 3.0)
        self.assertEqual(matrix['rows'][2][2], 4.0)
        self.assertEqual(matrix['translation'], [10.0, 20.0, 30.0])
        self.assertEqual(transform_point(matrix, (1.0, 1.0, 1.0)), (12.0, 23.0, 34.0))

    def test_compose_applies_the_component_transform_first(self):
        component = transform_matrix([1.0, 0.0, 0.0], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
        actor = transform_matrix([0.0, 0.0, 5.0], [0.0, 90.0, 0.0], [1.0, 1.0, 1.0])
        composed = compose(component, actor)
        # The component offset of +1 along X is rotated to +1 along Y by the actor yaw.
        self.assertAlmostEqual(composed['translation'][0], 0.0)
        self.assertAlmostEqual(composed['translation'][1], 1.0)
        self.assertAlmostEqual(composed['translation'][2], 5.0)

    def test_dump_matrix_reads_the_plane_struct(self):
        rows = [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]]
        self.assertEqual(dump_matrix(planes(rows, [10.0, 11.0, 12.0])),
                         dict(rows=rows, translation=[10.0, 11.0, 12.0]))

    def test_actor_matrix_reads_both_recorded_shapes(self):
        cooked = dict(matrix=[1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 7.0, 8.0, 9.0, 1.0],
                      scale=[0.5, 0.5, 2.0])
        self.assertEqual(actor_matrix(cooked),
                         dict(rows=[[0.5, 0.0, 0.0], [0.0, 0.5, 0.0], [0.0, 0.0, 2.0]], translation=[7.0, 8.0, 9.0]))
        composed = actor_matrix(dict(actor=dict(location=[1.0, 2.0, 3.0], rotation=[0.0, 0.0, 0.0], scale=[1.0, 1.0, 1.0]),
                                     component=dict(location=[0.0, 0.0, 0.0], rotation=[0.0, 0.0, 0.0], scale=[1.0, 1.0, 1.0])))
        self.assertEqual(composed['translation'], [1.0, 2.0, 3.0])

    def test_bounds_of(self):
        origin, extent = bounds_of([(-1.0, 0.0, 2.0), (3.0, 4.0, 2.0)])
        self.assertEqual((origin, extent), ([1.0, 2.0, 2.0], [2.0, 2.0, 0.0]))


class ActorTest(unittest.TestCase):
    ACTOR = dict(level='Level', source='TheWorld.PersistentLevel.StaticMeshActor_1.StaticMeshComponent_2',
                 transform=dict(actor=dict(location=[100.0, 0.0, 0.0], rotation=[0.0, 0.0, 0.0], scale=[1.0, 1.0, 1.0]),
                                component=dict(location=[0.0, 0.0, 0.0], rotation=[0.0, 0.0, 0.0], scale=[1.0, 1.0, 1.0])))
    IDENTITY = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]

    def test_agreement(self):
        report = compare_actor(self.ACTOR, dict(_LocalToWorld=planes(self.IDENTITY, [100.0, 0.0, 0.0])))
        self.assertEqual((report['status'], report['mismatches']), ('agree', []))
        self.assertLess(report['translation_delta'], 1e-9)

    def test_translation_and_rotation_are_reported_separately(self):
        rotated = [[0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]
        report = compare_actor(self.ACTOR, dict(_LocalToWorld=planes(rotated, [100.0, 0.0, 50.0])))
        self.assertEqual(report['status'], 'differ')
        self.assertEqual(report['mismatches'], ['rotation', 'translation'])
        self.assertAlmostEqual(report['translation_delta'], 50.0)

    def test_interp_actors_are_reported_as_movers(self):
        mover = dict(self.ACTOR, source='TheWorld.PersistentLevel.InterpActor_3.StaticMeshComponent_4')
        report = compare_actor(mover, dict(_LocalToWorld=planes(self.IDENTITY, [100.0, 0.0, 500.0])))
        self.assertEqual((report['status'], report['mismatches']), ('mover', ['translation']))


class TerrainTest(unittest.TestCase):
    # A 2x2 cell component whose section starts at (16, 0) in terrain-local space.
    POINTS = [(16.0, 0.0, 0.0), (18.0, 0.0, 0.0), (16.0, 2.0, 1.0), (18.0, 2.0, 1.0)]
    ROWS = [[128.0, 0.0, 0.0], [0.0, 128.0, 0.0], [0.0, 0.0, 256.0]]
    MESH = dict(source='Level:TheWorld.PersistentLevel.Terrain_0.TerrainComponent_1',
                sections=[], terrain=dict(section=[16, 0, 2, 2]))

    def properties(self, origin=(2176.0, 128.0, 128.0), extent=(129.0, 129.0, 129.0), section=(16, 0, 2, 2)):
        return dict(SectionBaseX=section[0], SectionBaseY=section[1], SectionSizeX=section[2], SectionSizeY=section[3],
                    _LocalToWorld=planes(self.ROWS, [2048.0, 0.0, 0.0]), Bounds=bounds(origin, extent))

    def test_agreement_after_the_section_base_is_removed(self):
        # Component-local X runs 0..2 once the base of 16 is subtracted: world 2048..2304.
        report = compare_terrain(self.MESH, self.properties(), self.POINTS)
        self.assertEqual((report['status'], report['mismatches']), ('agree', []))
        self.assertEqual(report['dump_section'], [16, 0, 2, 2])

    def test_section_disagreement(self):
        report = compare_terrain(self.MESH, self.properties(section=(16, 0, 4, 2)), self.POINTS)
        self.assertEqual(report['status'], 'differ')
        self.assertIn('section', report['mismatches'])

    def test_bounds_disagreement(self):
        report = compare_terrain(self.MESH, self.properties(origin=(2500.0, 128.0, 128.0)), self.POINTS)
        self.assertEqual(report['mismatches'], ['bounds_origin'])
        self.assertAlmostEqual(report['origin_delta'][0], 324.0)

    def test_the_one_unit_bounds_expansion_is_accounted_for(self):
        # Without the allowance the extents would be off by exactly one unit each.
        report = compare_terrain(self.MESH, self.properties(), self.POINTS)
        self.assertEqual([round(v, 9) for v in report['extent_delta']], [0.0, 0.0, 0.0])


class BspTest(unittest.TestCase):
    MESH = dict(source='Level:TheWorld.PersistentLevel.ModelComponent_0',
                sections=[dict(slot=0), dict(slot=1)],
                bsp=dict(model='Level:TheWorld.PersistentLevel.Model_3', nodes=[0, 1, 5]))

    def properties(self, nodes=3, elements=2, model='Level.TheWorld:PersistentLevel.Model_3'):
        return dict(Nodes=[None] * nodes, Elements=[None] * elements,
                    Model=dict(**{'class': 'Model'}, path=model), ZoneIndex=1, ComponentIndex=0)

    def test_agreement(self):
        report = compare_bsp(self.MESH, self.properties())
        self.assertEqual((report['status'], report['mismatches']), ('agree', []))
        self.assertEqual((report['nodes'], report['sections']), (3, 2))

    def test_each_kind_of_disagreement(self):
        report = compare_bsp(self.MESH, self.properties(nodes=4, elements=3, model='Level.TheWorld:PersistentLevel.Model_9'))
        self.assertEqual(report['mismatches'], ['nodes', 'elements', 'model'])


class MaterialTest(unittest.TestCase):
    def test_parent_chain_with_child_overrides(self):
        dumps = Stub({
            'Pkg.Materials.Mati_Child': dict(
                TextureParameterValues=[dict(ParameterName='p_Diffuse', ParameterValue=dict(path='Pkg.T.Child_Dif'))],
                Parent=dict(path='Pkg.Materials.Master')),
            'Pkg.Materials.Master': dict(
                TextureParameterValues=[dict(ParameterName='p_Diffuse', ParameterValue=dict(path='Pkg.T.Master_Dif')),
                                        dict(ParameterName='p_Normal', ParameterValue=dict(path='Pkg.T.Master_Nrm'))]),
        })
        self.assertEqual(effective_parameters(dumps, 'Pkg.Materials.Mati_Child'),
                         {'p_Diffuse': 'Pkg.T.Child_Dif', 'p_Normal': 'Pkg.T.Master_Nrm'})

    def test_parent_cycle_terminates(self):
        dumps = Stub({'A': dict(Parent=dict(path='B')), 'B': dict(Parent=dict(path='A'))})
        self.assertEqual(effective_parameters(dumps, 'A'), {})

    def test_missing_dump_yields_no_parameters(self):
        self.assertEqual(effective_parameters(Stub({}), 'Absent'), {})

    def test_every_status(self):
        parameters = {'p_Diffuse': 'Pkg.T.Dif', 'p_Masks': 'Pkg.T.Mask'}
        report = compare_material_parameters(
            dict(diffuse='Pkg.T.Dif', normal='Pkg.T.Nrm', specular='Pkg.T.Mask'), parameters)
        self.assertEqual({k: v['status'] for k, v in report.items()},
                         dict(diffuse='agree', normal='unparameterised', specular='other_parameter'))
        self.assertEqual(compare_material_parameters(dict(diffuse='Pkg.T.Other'), parameters)['diffuse']['status'],
                         'differ')
        self.assertEqual(compare_material_parameters(dict(diffuse='Pkg.T.Dif'), {})['diffuse']['status'],
                         'no_parameters')

    def test_canonical_parameter_name_wins_over_a_variant(self):
        parameters = {'p_DiffuseVertexPaint': 'Pkg.T.Paint', 'p_Diffuse': 'Pkg.T.Dif'}
        report = compare_material_parameters(dict(diffuse='Pkg.T.Dif'), parameters)
        self.assertEqual(report['diffuse']['status'], 'agree')
        # The variant is still part of the set, so choosing it is "other", not a disagreement.
        self.assertEqual(compare_material_parameters(dict(diffuse='Pkg.T.Paint'), parameters)['diffuse']['status'],
                         'other_parameter')


if __name__ == '__main__':
    unittest.main()
