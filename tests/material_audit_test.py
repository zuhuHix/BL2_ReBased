"""Coverage accounting must follow placed material overrides, including null slots."""
import sys
from pathlib import Path
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
from audit_scene_materials import audit, issue_category


class MaterialAuditTest(unittest.TestCase):
    def test_effective_slots_and_partial_channels(self):
        def material(name, channels, **extra):
            return dict(source=name, channels=channels, **extra)

        scene = dict(schema=1, dynamic_policy='frozen', map='Synthetic_P', issues=[],
                     materials={'base': material('base', {'diffuse': 'base.png'}),
                                'normal': material('normal', {'normal': 'normal.png'}),
                                'neutral': material('neutral', {}),
                                'black': material('black', {}, constant_diffuse=[0, 0, 0])},
                     meshes={'mesh': {'source': 'mesh', 'sections': [
                         {'slot': 0, 'material': 'base'},
                         {'slot': 2, 'material': 'normal'},
                         {'slot': 3, 'material': None}]}},
                     actors=[{'source': 'component', 'level': 'Synthetic_P', 'mesh': 'mesh',
                              'materials': [None, 'neutral', 'black']}])
        report = audit(scene)
        self.assertEqual(report['placed_sections'], 3)
        self.assertEqual(report['unassigned_placed_sections'], 1)
        self.assertEqual(len(report['section_audit']), 3)
        self.assertEqual(report['section_audit'][1]['material'], 'black')
        self.assertEqual(report['section_audit'][2]['cause'], 'unassigned_material')
        self.assertEqual(report['section_audit'][2]['visual_status'], 'UNVERIFIED')
        self.assertEqual(report['counts']['missing_diffuse'], 2)
        self.assertEqual(report['counts']['opaque_no_channels'], 1)
        self.assertEqual(report['counts']['opaque_missing_diffuse_placed_sections'], 0)
        scene['actors'][0]['materials'][2] = None
        report = audit(scene)
        self.assertEqual(report['counts']['opaque_missing_diffuse_placed_sections'], 1)
        self.assertEqual(report['counts']['opaque_no_channels_placed_sections'], 0)
        self.assertEqual(report['gap_status_counts'], {
            'no_supported_channels': 1, 'partial_channels': 1})
        scene['materials']['base']['surface_approximation'] = {'status': 'partial_unverified'}
        report = audit(scene)
        self.assertEqual(report['counts']['partial_surface_approximation_placed_sections'], 1)
        self.assertEqual(report['partial_surfaces'][0]['source'], 'base')
        self.assertEqual(report['gap_status_counts']['partial_surface_approximation'], 1)

    def test_issue_buckets_and_examples_are_stable(self):
        self.assertEqual(issue_category({'object': 'A:Mesh',
                                         'error': 'Unsupported component owner'}),
                         'unsupported_component_owner')
        self.assertEqual(issue_category({'object': 'A:Mesh',
                                         'error': 'invalid color stream'}),
                         'invalid_color_stream')
        self.assertEqual(issue_category({'object': 'A:Mesh:collision',
                                         'error': 'unsupported collision shape'}),
                         'collision')
        self.assertEqual(issue_category({'object': 'A:Material',
                                         'error': 'Approximation: test'}),
                         'approximation')
        scene = dict(schema=1, dynamic_policy='frozen', map='Synthetic_P',
                     issues=[{'object': 'A:Mesh', 'error': 'Unsupported component owner'},
                             {'object': 'A:Mesh2', 'error': 'invalid color stream'}],
                     materials={}, meshes={}, actors=[])
        report = audit(scene)
        self.assertEqual(report['issue_counts'], {
            'invalid_color_stream': 1, 'unsupported_component_owner': 1})
        self.assertEqual(report['issue_examples']['unsupported_component_owner'][0]['object'], 'A:Mesh')


if __name__ == '__main__':
    unittest.main()
