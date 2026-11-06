"""tools/lootables_editor.py writes only under the repository's ignored local/ (synthetic paths, nothing game-derived)."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import lootables_editor as editor  # noqa: E402


class LocalOutputTest(unittest.TestCase):
    def test_a_path_under_local_is_accepted(self):
        self.assertEqual(editor.local_output('local/lootables/plan.json'),
                         (editor.ROOT / 'local/lootables/plan.json').resolve())

    def test_tracked_and_outside_paths_are_refused(self):
        for value in ('docs/plan.json', 'tools/plan.json', '../plan.json', 'local', 'local/../docs/plan.json',
                      str(Path(tempfile.gettempdir()) / 'local' / 'plan.json')):
            with self.subTest(value=value), self.assertRaisesRegex(ValueError, 'must be inside'):
                editor.local_output(value)

    def test_a_local_folder_that_is_a_link_is_refused(self):
        with tempfile.TemporaryDirectory() as folder:
            repo = Path(folder) / 'repo'
            (repo / 'docs').mkdir(parents=True)
            made = subprocess.run(['cmd', '/c', 'mklink', '/J', str(repo / 'local'), str(repo / 'docs')], capture_output=True)
            if made.returncode:
                self.skipTest('could not make a junction here')
            try:
                with self.assertRaisesRegex(ValueError, 'is a link'):
                    editor.local_output('local/plan.json', repo=repo)
            finally:
                (repo / 'local').rmdir()   # removes the junction, not its target

    def test_a_real_local_folder_is_accepted(self):
        with tempfile.TemporaryDirectory() as folder:
            repo = Path(folder) / 'repo'
            (repo / 'local').mkdir(parents=True)
            self.assertEqual(editor.local_output('local/plan.json', repo=repo), (repo / 'local/plan.json').resolve())

    def test_a_loaded_plan_cannot_point_its_output_at_a_tracked_folder(self):
        with tempfile.TemporaryDirectory() as folder:
            plan = Path(folder) / 'plan.json'
            plan.write_text(json.dumps({'out_dir': str(editor.ROOT / 'docs')}), encoding='utf-8')
            with self.assertRaisesRegex(ValueError, 'must be inside'):
                editor.load_plan(plan)

    def test_a_loaded_plan_inside_local_is_kept(self):
        with tempfile.TemporaryDirectory() as folder:
            plan = Path(folder) / 'plan.json'
            plan.write_text(json.dumps({'out_dir': str(editor.OUT)}), encoding='utf-8-sig')
            self.assertEqual(Path(editor.load_plan(plan)['out_dir']), editor.OUT.resolve())

    def test_the_plan_and_report_commands_refuse_a_tracked_output(self):
        with tempfile.TemporaryDirectory() as folder:
            source = Path(folder) / 'interactives.json'
            source.write_text(json.dumps({'objects': [], 'balances': {}, 'items': {}, 'assets': {}}), encoding='utf-8')
            with self.assertRaises(ValueError):
                editor.main(['plan', '--interactives', str(source), '--out', 'docs/plan.json'])
            self.assertFalse((editor.ROOT / 'docs/plan.json').exists())


class MaterialPlanTest(unittest.TestCase):
    def setUp(self):
        folder = tempfile.TemporaryDirectory()
        self.addCleanup(folder.cleanup)
        self.root = Path(folder.name)

    def export(self, mesh, materials=(), inputs='Diffuse=Tex_Dif\n', textures=('Tex_Dif',)):
        out = self.root / mesh
        out.mkdir(parents=True, exist_ok=True)
        for material in materials:
            (out / f'{material}.mat').write_text(inputs, encoding='utf-8')
        for name in textures:
            (out / f'{name}.tga').write_bytes(b'')
        return out

    def plan(self, own, others=(), slots=('Mat_A',), log=''):
        return editor.plan_material(own.name, own, [own, *others], list(slots), log)

    def test_a_mesh_uses_its_own_export(self):
        row = self.plan(self.export('A', ['Mat_A'], 'Diffuse=Tex_Dif\nNormal=Tex_Nrm\n', ('Tex_Dif', 'Tex_Nrm')))
        self.assertEqual((row['material'], row['source'], row['reason']), ('Mat_A', 'own export', ''))
        self.assertTrue(row['diffuse'].endswith('Tex_Dif.tga'))
        self.assertTrue(row['normal'].endswith('Tex_Nrm.tga'))

    def test_the_slot_name_selects_the_instance_over_its_parent(self):
        row = self.plan(self.export('A', ['Mat_Parent', 'Mat_Inst']), slots=['Mat_Inst'])
        self.assertEqual(row['material'], 'Mat_Inst')

    def test_a_named_slot_without_its_material_is_left_unbound(self):
        row = self.plan(self.export('A', ['Mat_Other']), slots=['Mat_A'])
        self.assertIsNone(row['material'])
        self.assertIn('no exported material', row['reason'])

    def test_emissive_is_out_of_scope(self):
        row = self.plan(self.export('A', ['Mat_A'], 'Diffuse=Tex_Dif\nEmissive=Tex_Dif\n'))
        self.assertNotIn('emissive', row)
        self.assertIsNone(row['normal'])

    def test_a_cooked_elsewhere_material_comes_from_the_import_in_the_log(self):
        other = self.export('Other', ['Mat_Shared'])
        row = self.plan(self.export('A', textures=()), [other], ['dummy_material_0'],
                        'WARNING: Import(Pkg.Mat_Shared) was not found\nWARNING: Import(Common.Master) was not found\n')
        self.assertEqual(row['material'], 'Mat_Shared')
        self.assertIn('Other', row['source'])
        self.assertTrue(row['diffuse'].endswith('Tex_Dif.tga'))

    def test_a_similar_name_alone_does_not_bind_a_material(self):
        other = self.export('Other', ['Mat_A'])
        row = self.plan(self.export('A', textures=()), [other], ['dummy_material_0'], '')
        self.assertIsNone(row['material'])
        self.assertIn('names 0', row['reason'])

    def test_two_named_imports_do_not_bind_a_guess(self):
        others = [self.export('B', ['Mat_B']), self.export('C', ['Mat_C'])]
        log = 'Import(P.Mat_B) was not found\nImport(P.Mat_C) was not found\n'
        row = self.plan(self.export('A', textures=()), others, ['dummy_material_0'], log)
        self.assertIsNone(row['material'])
        self.assertIn('names 2', row['reason'])

    def test_a_mesh_with_another_slot_count_is_left_unbound(self):
        for slots in ([], ['Mat_A', 'Mat_B']):
            with self.subTest(slots=slots):
                row = self.plan(self.export('A', ['Mat_A']), slots=slots)
                self.assertIsNone(row['material'])
                self.assertIn('material slot', row['reason'])

    def test_a_missing_diffuse_leaves_the_mesh_unbound(self):
        row = self.plan(self.export('A', ['Mat_A'], textures=()))
        self.assertIsNone(row['diffuse'])
        self.assertIn('no exported diffuse', row['reason'])

    def test_umodels_utf16_log_is_read(self):
        path = self.root / 'job.log'
        path.write_bytes('Import(P.Mat_A) was not found'.encode('utf-16'))
        self.assertEqual(editor.MISSING_IMPORT.findall(editor.read_umodel_log(path)), ['P.Mat_A'])
        self.assertEqual(editor.read_umodel_log(self.root / 'none.log'), '')
        self.assertEqual(editor.read_umodel_log(None), '')


if __name__ == '__main__':
    unittest.main()
