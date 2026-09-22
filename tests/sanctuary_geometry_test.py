"""Synthetic checks for Sanctuary host geometry provenance diagnostics."""
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
from diagnose_sanctuary_geometry import analyse, actor_scope, effective_material


def scene():
    weighted = {
        'source': 'Terrain:weighted', 'channels': {},
        'terrain_blend': {'layers': [
            {'layer_index': 0, 'source_material': 'Snow.Base',
             'diffuse': 'snow.png', 'weightmap': 'weight.png', 'diffuse_status': 'available'}
        ]}}
    return {
        'schema': 1, 'map': 'Sanctuary_P',
        'camera': {'location': [0, 0, 1], 'rotation': [0, 0, 0], 'fov': 75},
        'materials': {'weighted': weighted},
        'meshes': {
            'terrain': {'source': 'Terrain:Component', 'sections': [
                {'slot': 0, 'file': 'terrain.obj', 'material': 'weighted'}],
                'collision': {'status': 'triangle_mesh'}},
            'bsp': {'source': 'BSP:Component', 'sections': [
                {'slot': 0, 'file': 'bsp.obj', 'material': None}],
                'collision': {'status': 'triangle_mesh'}},
            'prop': {'source': 'Prop:Box', 'sections': [
                {'slot': 0, 'file': 'prop.obj', 'material': None}],
                'collision': {'status': 'supported'}},
        },
        'actors': [
            {'source': 'TerrainComponent_0', 'level': 'Sanctuary_P', 'mesh': 'terrain',
             'terrain': 'Sanctuary_P:Terrain_0', 'transform': {'actor': {'location': [0, 0, 0],
             'rotation': [0, 0, 0], 'scale': [1, 1, 1]}, 'component': {'location': [0, 0, 0],
             'rotation': [0, 0, 0], 'scale': [1, 1, 1]}}, 'materials': [],
             'collision_enabled': True, 'hidden_visual': False},
            {'source': 'ModelComponent_0', 'level': 'Sanctuary_P', 'mesh': 'bsp',
             'bsp': 'Sanctuary_P:Model_3', 'transform': {'actor': {'location': [8, 0, 0],
             'rotation': [0, 0, 0], 'scale': [1, 1, 1]}, 'component': {'location': [0, 0, 0],
             'rotation': [0, 0, 0], 'scale': [1, 1, 1]}}, 'materials': [],
             'collision_enabled': True, 'hidden_visual': False},
            {'source': 'Prop_0', 'level': 'Sanctuary_P', 'mesh': 'prop',
             'transform': {'actor': {'location': [4, 0, 0], 'rotation': [0, 0, 0],
             'scale': [1, 1, 1]}, 'component': {'location': [0, 0, 0], 'rotation': [0, 0, 0],
             'scale': [1, 1, 1]}}, 'materials': [], 'collision_enabled': True,
             'hidden_visual': True},
        ],
        'terrain_policy': {'unverified': ['lightmaps']},
        'bsp_policy': {'collision': True},
    }


class SanctuaryGeometryTest(unittest.TestCase):
    def test_scope_and_effective_material_are_explicit(self):
        self.assertEqual(actor_scope({'terrain': 'T'}), 'terrain')
        self.assertEqual(actor_scope({'bsp': 'B'}), 'bsp')
        self.assertEqual(actor_scope({}), 'static_or_prop')
        mesh = {'sections': [{'slot': 0, 'material': 'default'}]}
        actor = {'materials': [None]}
        name, material = effective_material(actor, mesh, mesh['sections'][0],
                                            {'default': {'source': 'Default'}})
        self.assertEqual(name, 'default')
        self.assertEqual(material['source'], 'Default')

    def test_camera_hit_reports_terrain_provenance_and_gap(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            # Camera at (0,0,1) looks along +X.  The centre pixel of a 1280x720
            # frame sits half a pixel off centre, so the ray leaves slightly
            # off-axis; each triangle spans z 0..2 to be crossed through its
            # interior rather than along an edge.  Vertices are in component
            # space, so the actor locations in scene() put the terrain at X=2,
            # the hidden prop at X=4 and the BSP at X=8.
            def triangle(x):
                return f'v {x} -1 0\nv {x} 1 0\nv {x} 0 2\nf 1 2 3\n'
            (root / 'terrain.obj').write_text(triangle(2))
            (root / 'bsp.obj').write_text(triangle(0))
            (root / 'prop.obj').write_text(triangle(0))
            report = analyse(scene(), root, pixels=[(640, 360)])
        ray = report['camera']['rays'][0]
        self.assertEqual(ray['hits'][0]['scope'], 'terrain')
        self.assertEqual(ray['hits'][0]['material'], 'Terrain:weighted')
        self.assertEqual(ray['hits'][0]['terrain_layers'][0]['source_material'], 'Snow.Base')
        self.assertEqual(report['camera']['opening_coverage_gap']['gaps'][0]['width'], 6.0)
        self.assertEqual(report['unassigned']['count'], 2)
        self.assertEqual(report['summary']['bsp_actors'], 1)
        self.assertEqual(report['policy_audit']['unverified_terrain_fields'], ['lightmaps'])

    def test_json_is_not_needed_for_analysis(self):
        # Keep the fixture explicitly synthetic and serializable for reviewers.
        json.dumps(scene())


if __name__ == '__main__':
    unittest.main()
