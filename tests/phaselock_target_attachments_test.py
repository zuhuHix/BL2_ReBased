"""Synthetic tests for the worn attachments of tools/prepare_phaselock_target.py (no game data, no editor; the rows are invented).

Run: python tests/phaselock_target_attachments_test.py (numpy needed for the placement test).
"""
import json
import math
import os
import runpy
import sys
import tempfile
import types
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import prepare_phaselock_target as t  # noqa: E402


def row(component, bone, switch=0):
    return {'SwitchIdentifier': switch, 'Data': {'ComponentData': {'Component': component, 'MeshSocketName': bone}}}


def component(path):
    return {'P.StaticMeshComponent_1': {'staticmesh': 'Pkg.Mesh.Spike', 'materials': ['Pkg.Materials.Fur'], 'scale': 2.0,
                                        'scale3d': {'Y': -1.0}, 'translation': {'X': 1.0}, 'rotation': {'Yaw': 16384}},
            'P.StaticMeshComponent_2': {'staticmesh': 'Pkg.Mesh.Spike', 'materials': ['Pkg.Materials.Fur']}}[path]


def test_only_default_on_static_meshes_on_a_bone_are_worn():
    composition = {'Attachments': [row('P.SkeletalMeshComponent_9', 'None'), row(None, 'Rock'), row('P.StaticMeshComponent_8', 'Rock', 1),
                                   row('P.StaticMeshComponent_8', 'None'), row('P.StaticMeshComponent_1', 'Head'),
                                   row('P.StaticMeshComponent_2', 'Spine')]}
    worn = t.worn_attachments(composition, component)
    assert [(w['component'], w['bone']) for w in worn] == [('P.StaticMeshComponent_1', 'Head'), ('P.StaticMeshComponent_2', 'Spine')]
    assert worn[0]['comp_translation'] == [1.0, 0.0, 0.0] and worn[0]['comp_rotation'] == [0, 16384, 0]
    assert worn[0]['comp_scale'] == 2.0 and worn[0]['comp_scale3d'] == [1.0, -1.0, 1.0]
    assert worn[1]['comp_scale'] == 1.0 and worn[1]['comp_scale3d'] == [1.0, 1.0, 1.0]    # defaults when the archetype omits them


def expect_stop(text, *args):
    try:
        t.check_worn(*args)
    except SystemExit as stop:
        assert text in str(stop), str(stop)
        return
    raise AssertionError(f'expected a stop about {text}')


def test_any_set_but_the_expected_one_stops():
    expected = [('C_1', 'Head'), ('C_2', 'Spine')]
    head, spine = {'component': 'P.C_1', 'bone': 'Head'}, {'component': 'P.C_2', 'bone': 'Spine'}
    t.check_worn('T', [spine, head], expected)
    expect_stop("missing [('C_2', 'Spine')]", 'T', [head], expected)
    expect_stop("unexpected [('C_3', 'Arm')]", 'T', [head, spine, {'component': 'P.C_3', 'bone': 'Arm'}], expected)
    # A duplicate standing in for a missing piece keeps the count right and still stops.
    expect_stop("duplicated [('C_1', 'Head')]", 'T', [head, head], expected)
    expect_stop('unexpected', 'T', [head, {'component': 'P.C_2', 'bone': 'Neck'}], expected)    # right component, wrong bone


class Named:
    def __init__(self, path):
        self.path = path

    def get_path_name(self):
        return self.path


class Mesh(Named):
    def __init__(self, keeps=True, slots=1):
        super().__init__('/Game/X/Attachments/Spike.Spike')
        self.keeps, self.slots, self.set = keeps, slots, {}

    def get_editor_property(self, name):
        assert name == 'static_materials'
        return [object()] * self.slots

    def set_material(self, slot, material):
        if self.keeps:
            self.set[slot] = material

    def get_material(self, slot):
        return self.set.get(slot)


def run_editor(mesh, loads=True, exists=True, saves=True, material='Fur'):
    """Run the editor script against a stub unreal module. Returns the report path, and the error if it stopped."""
    stub = types.ModuleType('unreal')

    class Task:
        def get_objects(self):
            return [mesh]

    stub.AssetImportTask = Task
    stub.StaticMesh = Mesh
    stub.log = lambda message: None
    stub.AssetToolsHelpers = types.SimpleNamespace(get_asset_tools=lambda: types.SimpleNamespace(import_asset_tasks=lambda tasks: None))
    stub.EditorAssetLibrary = types.SimpleNamespace(does_asset_exist=lambda path: exists,
                                                    save_loaded_asset=lambda asset, only_if_is_dirty=False: saves)
    stub.load_asset = lambda path: Named(path) if loads else None
    with tempfile.TemporaryDirectory() as tmp:
        job = Path(tmp) / 'job.json'
        job.write_text(json.dumps({'slice_dir': tmp, 'npcs': [{'id': 'T', 'dest': '/Game/X', 'attachment_meshes': [
            {'id': 'Spike', 'gltf': 'Spike.gltf', 'material': material}]}]}), encoding='utf-8')
        os.environ['OPENWILLOW_SLICE_JOB'] = str(job)
        sys.modules['unreal'] = stub
        error = None
        try:
            runpy.run_path(str(Path(t.ROOT) / 'tools' / 'phaselock_target_attach_editor.py'))
        except RuntimeError as stopped:
            error = str(stopped)
        finally:
            del sys.modules['unreal'], os.environ['OPENWILLOW_SLICE_JOB']
        report = Path(tmp) / 'editor_report_attach.json'
        return (json.loads(report.read_text(encoding='utf-8')) if report.exists() else None), error


def test_the_editor_step_reports_only_after_every_check_passed():
    report, error = run_editor(Mesh())
    assert error is None and report == {'T': {'Spike': '/Game/X/Attachments/Spike.Spike'}}
    for text, mesh, options in (('is missing', Mesh(), {'exists': False}), ('did not load', Mesh(), {'loads': False}),
                                ('no material slots', Mesh(slots=0), {}), ('did not keep', Mesh(keeps=False), {}),
                                ('failed', Mesh(), {'saves': False})):
        report, error = run_editor(mesh, **options)
        assert report is None and error and text in error, (text, report, error)


def test_a_piece_with_no_material_keeps_what_its_import_made():
    mesh = Mesh()
    report, error = run_editor(mesh, exists=False, material=None)
    assert error is None and report == {'T': {'Spike': '/Game/X/Attachments/Spike.Spike'}} and mesh.set == {}


def test_a_piece_comes_from_a_package_that_exports_it():
    assert t.piece_package(['Ash_P', 'Ice_Dynamic', 'Ice_P'], 'Ice_Dynamic') == 'Ice_Dynamic'
    assert t.piece_package(['Ash_P', 'IceCanyon_P', 'Ice_P'], 'Ice_Dynamic') == 'Ice_P'    # the same map, not a similar name
    assert t.piece_package(['Cove_P', 'Ash_P'], 'Ice_Dynamic') == 'Ash_P'
    try:
        t.piece_package([], 'Ice_Dynamic')
    except SystemExit as stop:
        assert 'no package' in str(stop)
    else:
        raise AssertionError('a piece no package exports was accepted')
    worn = [{'static_mesh': 'A.Mask', 'materials': ['A.Head']}, {'static_mesh': 'B.Axe', 'materials': []}]
    t.locate_pieces(worn, 'Ice_Dynamic', {'A.Mask': ['Ice_P'], 'A.Head': ['Ash_P', 'Ice_P'], 'B.Axe': ['Ice_Dynamic']}.__getitem__)
    assert (worn[0]['package'], worn[0]['material_package']) == ('Ice_P', 'Ice_P')
    assert (worn[1]['package'], worn[1]['material_package']) == ('Ice_Dynamic', None)


def test_a_mesh_with_two_materials_stops():
    assert t.attachment_meshes([{'static_mesh': 'P.A', 'materials': ['P.M']}, {'static_mesh': 'P.A', 'materials': ['P.M']}]) == {'A': 'M'}
    try:
        t.attachment_meshes([{'static_mesh': 'P.A', 'materials': ['P.M']}, {'static_mesh': 'P.A', 'materials': ['P.N']}])
    except SystemExit as stop:
        assert 'two materials' in str(stop)
        return
    raise AssertionError('two materials were accepted')


def test_host_rows_keep_the_transform_and_a_reflection():
    worn = t.worn_attachments({'Attachments': [row('P.StaticMeshComponent_1', 'Head')]}, component)
    rows = t.host_attachments(worn, {'Spike': '/Game/X/Spike.Spike'})
    assert rows[0]['mesh'] == '/Game/X/Spike.Spike' and rows[0]['bone'] == 'Head' and rows[0]['location'] == [1.0, 0.0, 0.0]
    # A negative Scale3D is a reflection: exactly one size comes back negative, and the sizes keep the magnitudes.
    scale = rows[0]['scale']
    assert sum(v < 0 for v in scale) == 1 and all(math.isclose(abs(v), 2.0, abs_tol=1e-4) for v in scale)
    assert math.isclose(sum(v * v for v in rows[0]['quat']), 1.0, abs_tol=1e-4)


if __name__ == '__main__':
    for name, test in list(globals().items()):
        if name.startswith('test_'):
            test()
    print('ok')
