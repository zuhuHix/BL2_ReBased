"""Synthetic tests for the worn attachments of tools/prepare_phaselock_target.py (no game data, no editor; the rows are invented).

Run: python tests/phaselock_target_attachments_test.py (numpy needed for the placement test).
"""
import math
import sys
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


def test_a_mesh_with_two_materials_stops():
    assert t.attachment_meshes([{'static_mesh': 'P.A', 'materials': ['P.M']}, {'static_mesh': 'P.A', 'materials': ['P.M']}]) == {'A': 'M'}
    try:
        t.attachment_meshes([{'static_mesh': 'P.A', 'materials': ['P.M']}, {'static_mesh': 'P.A', 'materials': ['P.N']}])
    except SystemExit as stop:
        assert 'two materials' in str(stop)
        return
    raise AssertionError('two materials were accepted')


def test_host_rows_keep_the_transform_and_a_reflection():
    import numpy as np
    worn = t.worn_attachments({'Attachments': [row('P.StaticMeshComponent_1', 'Head')]}, component)
    rows = t.host_attachments(worn, {'Spike': '/Game/X/Spike.Spike'}, {'Head': np.eye(4)})
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
