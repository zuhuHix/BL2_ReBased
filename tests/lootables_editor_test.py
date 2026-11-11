"""tools/lootables_editor.py writes only under the repository's ignored local/ (synthetic paths, nothing game-derived)."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import types
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

    @unittest.skipUnless(os.name == 'nt', 'makes the link with a Windows junction (cmd /c mklink /J)')
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


class FakeSlot:
    def __init__(self, material=None):
        self.material = material

    def set_editor_property(self, _name, value):
        self.material = value

    def get_editor_property(self, _name):
        return self.material


class CopyingArray(list):
    """As unreal.Array of a struct: indexing gives a copy, so changing it changes nothing until it is written back."""

    def __getitem__(self, index):
        return FakeSlot(super().__getitem__(index).material)


class FakeMesh:
    def __init__(self, slot_count=1):
        self.slots = [FakeSlot() for _ in range(slot_count)]
        self.modified = False

    def modify(self):
        self.modified = True

    def get_editor_property(self, _name):
        return CopyingArray(FakeSlot(s.material) for s in self.slots)

    def set_editor_property(self, _name, value):
        self.slots = [FakeSlot(s.material) for s in list.__iter__(value)]


class BindSkeletalTest(unittest.TestCase):
    def test_the_slot_keeps_the_instance_through_the_copies(self):
        mesh = FakeMesh()
        self.assertEqual(editor.bind_skeletal(mesh, 'instance'), 'instance')
        self.assertEqual(mesh.slots[0].material, 'instance')
        self.assertTrue(mesh.modified)

    def test_another_slot_count_is_refused(self):
        with self.assertRaisesRegex(RuntimeError, 'one material slot'):
            editor.bind_skeletal(FakeMesh(2), 'instance')


class FakeAsset:
    def __init__(self, path):
        self.path = path
        self.parent = None
        self.params = {}
        self.cleared = 0

    def get_path_name(self):
        name = self.path.rsplit('/', 1)[-1]
        return f'{self.path}.{name}'

    def set_editor_property(self, _name, _value):
        pass


class FakeEditor:
    """The Unreal calls editor_materials makes. No Unreal: it records them, and each save can be told to fail."""

    def __init__(self, out, rows):
        self.assets, self.created, self.fail_saves, self.fail_folders, self.folders, self.fail_params = {}, [], set(), set(), [], set()
        self.parent_parameters = ['Diffuse', 'Normal']
        (out / 'materials_plan.json').write_text(json.dumps({'meshes': rows, 'pickups': []}), encoding='utf-8')
        (out / 'editor_meshes.json').write_text(json.dumps({'mesh_assets': {r['mesh']: f'/Game/{r["mesh"]}' for r in rows}}), encoding='utf-8')
        self.meshes = {f'/Game/{r["mesh"]}': self.mesh(f'/Game/{r["mesh"]}') for r in rows}
        self.out = out

    def mesh(self, path):
        mesh = FakeMesh()
        mesh.get_path_name = lambda: path
        return mesh

    # eal
    def does_asset_exist(self, path):
        return path in self.assets

    def save_loaded_asset(self, asset, only_if_is_dirty=False):
        return asset.get_path_name() not in self.fail_saves

    def save_directory(self, folder, only_if_is_dirty=False, recursive=True):
        self.folders.append(folder)
        return folder not in self.fail_folders

    # tools
    def create_asset(self, name, folder, _cls, _factory):
        asset = FakeAsset(f'{folder}/{name}')
        self.assets[asset.path] = asset
        self.created.append(asset.path)
        return asset

    # unreal
    def load_asset(self, path):
        return self.assets.get(path) or self.meshes.get(path)

    # MaterialEditingLibrary
    def set_material_instance_parent(self, instance, parent):
        instance.parent = parent

    def clear_all_material_instance_parameters(self, instance):
        instance.params.clear()
        instance.cleared += 1

    def get_texture_parameter_names(self, _parent):
        return self.parent_parameters

    def set_material_instance_texture_parameter_value(self, instance, parameter, texture):
        if parameter not in self.fail_params:
            instance.params[parameter] = texture
        return False   # always, as in UE 5.8: MaterialEditingLibrary.cpp never sets the bool it returns

    def get_material_instance_texture_parameter_value(self, instance, parameter):
        return instance.params.get(parameter)


class EditorMaterialsTest(unittest.TestCase):
    def setUp(self):
        folder = tempfile.TemporaryDirectory()
        self.addCleanup(folder.cleanup)
        self.out = Path(folder.name)

    def planned(self, mesh, instance='Mat_1', normal=None):
        return {'mesh': mesh, 'material': 'Mat', 'instance': instance, 'source': 'own export', 'diffuse': f'{instance}_dif.tga',
                'normal': normal, 'reason': ''}

    def run_it(self, rows, prepare=None, fake=None):
        fake = fake or FakeEditor(self.out, rows)
        if prepare:
            prepare(fake)
        saved = (editor.unreal, getattr(editor, 'eal', None), getattr(editor, 'tools', None), editor.lootable_master, editor.import_one)
        self.addCleanup(lambda: (setattr(editor, 'unreal', saved[0]), setattr(editor, 'eal', saved[1]), setattr(editor, 'tools', saved[2]),
                                 setattr(editor, 'lootable_master', saved[3]), setattr(editor, 'import_one', saved[4])))
        editor.unreal = types.SimpleNamespace(
            log=lambda _text: None, load_asset=fake.load_asset, MaterialEditingLibrary=fake, MaterialInstanceConstant=object,
            MaterialInstanceConstantFactoryNew=lambda: None, Texture2D=object,
            TextureCompressionSettings=types.SimpleNamespace(TC_NORMALMAP=1))
        editor.eal, editor.tools = fake, fake
        editor.lootable_master = lambda: 'parent'
        editor.import_one = lambda source, folder, _cls: FakeAsset(f'{folder}/{source}')
        return fake, editor.editor_materials({'out_dir': str(self.out)})

    def test_a_good_bind_records_the_instance(self):
        fake, report = self.run_it([self.planned('A')])
        self.assertEqual(report['bound']['A']['bound'], True)
        self.assertEqual(fake.meshes['/Game/A'].slots[0].material.path, f'{editor.MATERIAL_DEST}/Mat_1/Mat_1')
        self.assertEqual(report['bound']['A']['instance'], f'{editor.MATERIAL_DEST}/Mat_1/Mat_1')

    def test_a_failed_save_is_not_a_bind(self):
        # Each save the bind depends on: the mesh, the instance, a texture.
        for name, path in (('mesh', '/Game/A'), ('instance', f'{editor.MATERIAL_DEST}/Mat_1/Mat_1.Mat_1'),
                           ('texture', f'{editor.MATERIAL_DEST}/Mat_1/Mat_1_dif.tga.Mat_1_dif.tga')):
            with self.subTest(save=name):
                fake, report = self.run_it([self.planned('A')], lambda f, p=path: f.fail_saves.add(p))
                self.assertEqual(report['bound']['A']['bound'], False)
                self.assertIn('could not save', report['bound']['A']['reason'])
                self.assertEqual(editor.unbound_meshes({'meshes': [self.planned('A')]}, report)[0][0], 'A')
                if name != 'mesh':   # what the mesh would point at is not safe yet: it is never touched
                    self.assertIsNone(fake.meshes['/Game/A'].slots[0].material)

    def test_a_failed_folder_save_touches_no_mesh(self):
        rows = [self.planned('A'), self.planned('B', 'Mat_2')]
        fake, report = self.run_it(rows, lambda f: f.fail_folders.add(f'{editor.MATERIAL_DEST}/Mat_2'))
        self.assertEqual({row['bound'] for row in report['bound'].values()}, {False})
        self.assertIn('not changed', report['bound']['A']['reason'])
        self.assertEqual([fake.meshes[f'/Game/{m}'].slots[0].material for m in 'AB'], [None, None])

    def test_only_this_runs_folders_are_saved(self):
        fake, _ = self.run_it([self.planned('A'), self.planned('B', 'Mat_2'), self.planned('C')])
        self.assertEqual(sorted(fake.folders), [f'{editor.MATERIAL_DEST}/Mat_1', f'{editor.MATERIAL_DEST}/Mat_2'])

    def test_the_setters_false_is_not_a_failure(self):
        # What the first real editor run found: the setter returned false and the value was set.
        _, report = self.run_it([self.planned('A', normal='nrm.tga')])
        self.assertEqual(report['bound']['A']['bound'], True)

    def test_a_parent_without_the_parameter_fails_the_row(self):
        fake, report = self.run_it([self.planned('A', normal='nrm.tga')], lambda f: setattr(f, 'parent_parameters', ['Diffuse']))
        self.assertEqual(report['bound']['A']['bound'], False)
        self.assertIn('no Normal parameter', report['bound']['A']['reason'])
        self.assertIsNone(fake.meshes['/Game/A'].slots[0].material)

    def test_a_texture_the_instance_refuses_fails_the_row(self):
        fake, report = self.run_it([self.planned('A', normal='nrm.tga')], lambda f: f.fail_params.add('Normal'))
        self.assertEqual(report['bound']['A']['bound'], False)
        self.assertIn('did not take its Normal', report['bound']['A']['reason'])
        self.assertIsNone(fake.meshes['/Game/A'].slots[0].material)

    def test_one_name_with_different_textures_makes_two_instances(self):
        fake, report = self.run_it([self.planned('A', 'Mat_1'), self.planned('B', 'Mat_2'), self.planned('C', 'Mat_1')])
        self.assertEqual(len(fake.created), 2)
        self.assertEqual(report['bound']['A']['instance'], report['bound']['C']['instance'])
        self.assertNotEqual(report['bound']['A']['instance'], report['bound']['B']['instance'])

    def test_a_rerun_leaves_no_old_normal_on_the_instance(self):
        # The first run has a normal; the plan no longer does.
        fake, _ = self.run_it([self.planned('A', normal='nrm.tga')])
        instance = fake.assets[f'{editor.MATERIAL_DEST}/Mat_1/Mat_1']
        self.assertEqual(set(instance.params), {'Diffuse', 'Normal'})
        (self.out / 'materials_plan.json').write_text(json.dumps({'meshes': [self.planned('A')], 'pickups': []}), encoding='utf-8')
        fake, report = self.run_it([], fake=fake)
        self.assertEqual(len(fake.created), 1)   # updated in place
        self.assertEqual(set(instance.params), {'Diffuse'})
        self.assertEqual(report['bound']['A']['normal'], False)


class MaterialRowsTest(unittest.TestCase):
    PLAN = {'skeletal_meshes': [{'name': 'A'}], 'pickup_meshes': [{'name': 'P'}]}

    def test_no_materials_step_is_a_row_for_every_mesh_not_a_clean_report(self):
        rows = editor.material_rows(self.PLAN, None, {})
        self.assertEqual([(r['mesh'], r['imported']) for r in rows], [('A', False), ('P', False)])
        self.assertIn('has not run', rows[0]['reason'])

    def test_two_meshes_on_one_instance_each_have_a_row(self):
        plan = {'meshes': [{'mesh': 'A', 'instance': 'I', 'material': 'M', 'source': 's', 'reason': ''},
                           {'mesh': 'B', 'instance': 'I', 'material': 'M', 'source': 's', 'reason': ''}], 'pickups': []}
        bound = {'bound': {'A': {'bound': True, 'instance': '/x/I'}, 'B': {'bound': True, 'instance': '/x/I'}}}
        rows = editor.material_rows(self.PLAN, plan, bound)
        self.assertEqual([(r['mesh'], r['asset'], r['imported']) for r in rows], [('A', '/x/I', True), ('B', '/x/I', True)])

    def test_the_import_script_drops_the_old_report_before_any_step(self):
        text = (editor.ROOT / 'tools/import_lootables.ps1').read_text(encoding='utf-8')
        self.assertLess(text.index('Remove-Item -LiteralPath $reportPath'), text.index('$plan = Invoke-Plan'))


class InstanceNameTest(unittest.TestCase):
    def setUp(self):
        folder = tempfile.TemporaryDirectory()
        self.addCleanup(folder.cleanup)
        self.root = Path(folder.name)

    def texture(self, name, content):
        path = self.root / name
        path.write_bytes(content)
        return str(path)

    def test_the_same_textures_in_two_exports_share_a_name(self):
        self.assertEqual(editor.instance_name('M', self.texture('a.tga', b'one'), None), editor.instance_name('M', self.texture('b.tga', b'one'), None))

    def test_one_name_with_other_textures_differs(self):
        first = editor.instance_name('M', self.texture('a.tga', b'one'), None)
        self.assertNotEqual(first, editor.instance_name('M', self.texture('b.tga', b'two'), None))
        self.assertNotEqual(first, editor.instance_name('M', self.texture('c.tga', b'one'), self.texture('n.tga', b'nrm')))
        self.assertTrue(first.startswith('M_'))

    def test_the_diffuse_and_normal_cannot_swap_places(self):
        one, two = self.texture('a.tga', b'one'), self.texture('b.tga', b'two')
        self.assertNotEqual(editor.instance_name('M', one, two), editor.instance_name('M', two, one))


class UnboundMeshesTest(unittest.TestCase):
    PLAN = {'meshes': [{'mesh': 'A', 'reason': ''}, {'mesh': 'B', 'reason': ''}, {'mesh': 'C', 'reason': 'no diffuse'}],
            'pickups': [{'mesh': 'P', 'reason': ''}]}

    def test_all_bound_is_empty(self):
        report = {'bound': {name: {'bound': True} for name in 'ABCP'}}
        self.assertEqual(editor.unbound_meshes(self.PLAN, report), [])

    def test_a_failed_bind_and_a_planned_refusal_are_both_named(self):
        report = {'bound': {'A': {'bound': False, 'reason': 'RuntimeError: slot'}, 'B': {'bound': True},
                            'C': {'bound': False, 'reason': 'no diffuse'}, 'P': {'bound': True}}}
        self.assertEqual(editor.unbound_meshes(self.PLAN, report), [('A', 'RuntimeError: slot'), ('C', 'no diffuse')])

    def test_a_mesh_missing_from_the_report_is_unbound(self):
        rows = editor.unbound_meshes(self.PLAN, {'bound': {'A': {'bound': True}, 'B': {'bound': True}, 'C': {'bound': True}}})
        self.assertEqual(rows, [('P', 'no editor result')])

    def test_no_report_leaves_every_mesh_unbound(self):
        self.assertEqual({mesh for mesh, _ in editor.unbound_meshes(self.PLAN, None)}, set('ABCP'))

    def test_the_import_script_fails_after_writing_the_report(self):
        text = (editor.ROOT / 'tools/import_lootables.ps1').read_text(encoding='utf-8')
        check = text.index('check-materials --plan')
        report = text.index("if (& $run 'report')")
        failure = text.index('if ($materialsFailed) { throw')
        self.assertLess(check, report)
        self.assertLess(report, failure)


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
