"""Synthetic tests for tools/prepare_phaselock_target.py (no game data).

Covers the parts that do not need the game, UModel or the editor: where UModel -groups puts an AnimSet, which
exported clip file a role resolves to, the host file's paths and mesh offset, and that the roles keep the "Idle"
naming the host relies on (AOpenWillowCombatTarget::UseStockPawn finds Anim_<Id>_PhaseLock_* by replacing "Idle").
Names and numbers are invented. Needs numpy (the shared slice pipeline imports it).
Run: python tests/prepare_phaselock_target_test.py
"""
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
import prepare_phaselock_target as t  # noqa: E402


def test_group_of():
    assert t.group_of('Anim_Fake.Unarmed') == ('Anim_Fake', 'Unarmed')


def test_clip_file_picks_the_named_set():
    with tempfile.TemporaryDirectory() as folder:
        t.UMODEL_OUT = Path(folder)
        for group, name in (('Anim_Fake', 'Unarmed'), ('Anim_Other', 'Unarmed'), ('Anim_Fake', 'Armed')):
            (Path(folder) / 'Beast/Pkg' / group / name).mkdir(parents=True)
            (Path(folder) / 'Beast/Pkg' / group / name / 'PhaseLock_Lift.md5anim').write_text('x')
        hit = t.clip_file('Beast', 'Anim_Fake.Unarmed', 'PhaseLock_Lift')
        assert hit.parent.name == 'Unarmed' and hit.parent.parent.name == 'Anim_Fake', hit
        assert t.clip_file('Beast', 'Anim_Fake.Unarmed', 'PhaseLock_Land') is None


def test_host_target():
    target = {'dest': '/Game/X/Beast', 'skeletal_mesh': {'path': 'Char_Fake.Mesh.Skel_Fake'},
              'mesh_component_properties': {'translation': {'X': 0, 'Y': 0, 'Z': -50}, 'scale': 0.8},
              'observed': {'collision_radius': 70.0, 'collision_height': 48.0, 'source': 'made up'}}
    host = t.host_target('Beast', target)
    assert host['mesh'] == '/Game/X/Beast/Meshes/Skel_Fake/SkeletalMeshes/Skel_Fake.Skel_Fake'
    assert host['idle'] == '/Game/X/Beast/Animations/Anim_Beast_Idle.Anim_Beast_Idle'
    # Floor-based origin: the cylinder centre is CollisionHeight above the floor, the mesh hangs by Translation.Z.
    assert host['mesh_offset_z'] == -2.0
    assert (round(host['mesh_scale'], 2), host['collision_radius'], host['collision_height']) == (0.8, 70.0, 48.0)
    # The host derives the clip paths from the idle path by replacing "Idle".
    assert host['idle'].replace('Idle', 'PhaseLock_Lift').endswith('Anim_Beast_PhaseLock_Lift.Anim_Beast_PhaseLock_Lift')


def test_host_target_uses_component_scale_without_visual_fit():
    target = {'dest': '/Game/X/Beast', 'skeletal_mesh': {'path': 'Char_Fake.Mesh.Skel_Fake'},
              'mesh_component_properties': {'translation': {'Z': 0}, 'scale': 0.8},
              'observed': {'collision_radius': 70.0, 'collision_height': 48.0, 'source': 'made up'}}
    assert t.host_target('Beast', target)['mesh_scale'] == 0.8


def test_host_target_prefers_live_mesh_scale_over_archetype_scale():
    target = {'dest': '/Game/X/Beast', 'skeletal_mesh': {'path': 'Char_Fake.Mesh.Skel_Fake'},
              'mesh_component_properties': {'translation': {'Z': 0}, 'scale': 0.7},
              'observed': {'collision_radius': 70.0, 'collision_height': 48.0, 'source': 'made up',
                           'host_mesh_scale': 1.0, 'host_mesh_scale_source': 'measured live'}}
    host = t.host_target('Beast', target)
    assert host['mesh_scale'] == 1.0
    assert host['source']['mesh_scale'] == 'measured live'


def test_roles_keep_the_host_naming():
    for spec in t.TARGETS.values():
        roles = [r for r, _ in spec['roles']]
        assert roles[0] == 'Idle' and 'Death' in roles
        assert {'PhaseLock_Lift', 'PhaseLock_Loop', 'PhaseLock_Fall', 'PhaseLock_Land'} <= set(roles)
        assert 'idle' not in spec['dest'].lower()   # a replace of "Idle" must only touch the clip name


if __name__ == '__main__':
    for name, fn in list(globals().items()):
        if name.startswith('test_'):
            fn()
            print('ok', name)
