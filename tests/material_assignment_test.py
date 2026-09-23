"""Regression tests for the bounded Sanctuary null-section dispositions."""
import importlib.util
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tools'))

prepare_spec = importlib.util.spec_from_file_location(
    'prepare_level', ROOT / 'tools' / 'prepare_level.py')
prepare = importlib.util.module_from_spec(prepare_spec)
prepare_spec.loader.exec_module(prepare)
from audit_scene_materials import audit  # noqa: E402


class MaterialAssignmentTest(unittest.TestCase):
    def test_exact_sanctuary_section_policies_are_narrow_and_idempotent(self):
        def assign(identity, sections):
            prepare.apply_section_material_policies(
                identity, sections,
                material_for_path=lambda path: 'banner_static',
                material_source_for_id=lambda name: {
                    'trim': 'Sanctuary_P:Prop_SancBuildings.Material.Mati_SancBuild1a_04',
                    'banner': 'Sanctuary_P:Prop_RolandsResistance.Materials.Mati_ResistanceBanners_Wind'
                }[name])

        banner = [{'slot': 0, 'material': 'banner'}, {'slot': 1, 'material': None}]
        assign('Sanctuary_P:Prop_RolandsResistance.Mesh.ResistanceBanner_03', banner)
        self.assertEqual(banner[1]['material'], 'banner_static')
        self.assertEqual(banner[1]['material_policy']['method'],
                         'observed_companion_material_v1')
        assign('Sanctuary_P:Prop_RolandsResistance.Mesh.ResistanceBanner_03', banner)
        self.assertEqual(banner[1]['material'], 'banner_static')

        trim = [{'slot': 0, 'material': 'trim'}, {'slot': 1, 'material': None}]
        assign('Sanctuary_Px:Env_Sanctuary.Meshes.SancBuild1_Trim', trim)
        self.assertEqual(trim[1]['material'], 'trim')
        self.assertEqual(trim[1]['material_policy']['method'],
                         'same_mesh_observed_material_v1')

        vending = [{'slot': 0, 'material': None}]
        assign('Sanctuary_P:prop_signs.VendingIcon', vending)
        self.assertIsNone(vending[0]['material'])
        self.assertEqual(vending[0]['material_policy']['method'],
                         'host_neutral_fallback_v1')

        blocking = [{'slot': 0, 'material': None}]
        assign('Sanctuary_P:Common_Meshes.Blocking.Blocking_Cube', blocking)
        self.assertIsNone(blocking[0]['material'])
        self.assertEqual(blocking[0]['material_policy']['method'],
                         'helper_hidden_collision_only_v1')

        untouched = [{'slot': 0, 'material': None}]
        prepare.apply_section_material_policies('Other_P:prop_signs.VendingIcon', untouched)
        self.assertNotIn('material_policy', untouched[0])

    def test_audit_counts_all_fifteen_as_explicit_dispositions(self):
        def section(slot, material, policy=None):
            value = {'slot': slot, 'file': f'{slot}.obj', 'material': material}
            if policy:
                value['material_policy'] = policy
            return value

        banner_policy = {
            'version': prepare.SECTION_MATERIAL_POLICY_VERSION,
            'method': 'observed_companion_material_v1'}
        trim_policy = {
            'version': prepare.SECTION_MATERIAL_POLICY_VERSION,
            'method': 'same_mesh_observed_material_v1'}
        vending_policy = {
            'version': prepare.SECTION_MATERIAL_POLICY_VERSION,
            'method': 'host_neutral_fallback_v1'}
        blocking_policy = {
            'version': prepare.SECTION_MATERIAL_POLICY_VERSION,
            'method': 'helper_hidden_collision_only_v1'}
        meshes = {
            'banner': {'source': 'Sanctuary_P:Prop_RolandsResistance.Mesh.ResistanceBanner_03',
                       'sections': [section(0, 'banner_wind'),
                                    section(1, 'banner_static', banner_policy)]},
            'trim': {'source': 'Sanctuary_Px:Env_Sanctuary.Meshes.SancBuild1_Trim',
                     'sections': [section(0, 'trim'), section(1, 'trim', trim_policy)]},
            'vending': {'source': 'Sanctuary_P:prop_signs.VendingIcon',
                        'sections': [section(0, None, vending_policy)]},
            'blocking': {'source': 'Sanctuary_P:Common_Meshes.Blocking.Blocking_Cube',
                         'sections': [section(0, None, blocking_policy)]}}
        materials = {
            name: {'source': name, 'channels': {'diffuse': name + '.png'}}
            for name in ('banner_wind', 'banner_static', 'trim')}
        actors = []
        for i in range(6):
            actors.append({'source': f'banner-{i}', 'level': 'Sanctuary_P',
                           'mesh': 'banner', 'materials': []})
        for i in range(2):
            actors.append({'source': f'trim-{i}', 'level': 'Sanctuary_Px',
                           'mesh': 'trim', 'materials': []})
        for i in range(3):
            actors.append({'source': f'vending-{i}', 'level': 'Sanctuary_P',
                           'mesh': 'vending', 'materials': []})
        for i in range(4):
            actors.append({'source': f'blocking-{i}', 'level': 'Sanctuary_Dynamic',
                           'mesh': 'blocking', 'materials': [], 'hidden_visual': True})
        report = audit({'schema': 1, 'dynamic_policy': 'frozen', 'map': 'Sanctuary_P',
                        'issues': [], 'materials': materials, 'meshes': meshes,
                        'actors': actors})
        self.assertEqual(report['unassigned_placed_sections'], 0)
        self.assertEqual(report['resolved_policy_sections'], 15)
        self.assertEqual(report['section_policy_counts'], {
            'helper_hidden_collision_only_v1': 4,
            'host_neutral_fallback_v1': 3,
            'observed_companion_material_v1': 6,
            'same_mesh_observed_material_v1': 2})
        causes = [row['cause'] for row in report['section_audit']]
        self.assertEqual(causes.count('intentional_helper_hidden'), 4)
        self.assertEqual(causes.count('explicit_neutral_material_fallback'), 3)
        self.assertEqual(causes.count('explicit_material_fallback'), 8)


if __name__ == '__main__':
    unittest.main()
