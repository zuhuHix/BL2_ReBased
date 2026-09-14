"""Coverage accounting must follow placed material overrides, including null slots."""
import sys
from pathlib import Path
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
from audit_scene_materials import audit


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
        self.assertEqual(report['counts']['missing_diffuse'], 2)
        self.assertEqual(report['counts']['opaque_no_channels'], 1)
        self.assertEqual(report['counts']['opaque_missing_diffuse_placed_sections'], 0)
        scene['actors'][0]['materials'][2] = None
        report = audit(scene)
        self.assertEqual(report['counts']['opaque_missing_diffuse_placed_sections'], 1)
        self.assertEqual(report['counts']['opaque_no_channels_placed_sections'], 0)


if __name__ == '__main__':
    unittest.main()
