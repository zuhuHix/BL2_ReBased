"""Synthetic tests for tools/prepare_phaselock_target.py (no game data).

Covers the parts that do not need the game, UModel or the editor: where UModel -groups puts an AnimSet, which
exported clip file a role resolves to, the host file's paths and mesh offset, and that the roles keep the "Idle"
naming the host relies on (AOpenWillowCombatTarget::UseStockPawn finds Anim_<Id>_PhaseLock_* by replacing "Idle").
Names and numbers are invented. Needs numpy (the shared slice pipeline imports it).
Run: python tests/prepare_phaselock_target_test.py
"""
import json
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


def test_every_target_has_what_identity_requires():
    # identity_step reads these for every target: a real import stops late if one is missing.
    for name, spec in t.TARGETS.items():
        for key in ('package', 'pawn', 'dest', 'roles', 'attachments', 'observed'):
            assert key in spec, f'{name} lacks {key}'
        assert 'source' in spec['observed'], name
        assert {'Idle', 'Death'} <= {role for role, _ in spec['roles']}, name


def test_host_target_takes_the_archetype_cylinder_without_a_live_read():
    target = {'dest': '/Game/X', 'skeletal_mesh': {'path': 'Pkg.Mesh.Made'},
              'mesh_component_properties': {'translation': {'X': 3.0, 'Y': 0.0, 'Z': -10.0}},
              'archetype_cylinder': {'collisionradius': 30.0, 'collisionheight': 50.0},
              'observed': {'source': 'made up: no live read'}}
    host = t.host_target('Beast', target)
    assert (host['collision_radius'], host['collision_height'], host['mesh_offset_z'], host['mesh_scale']) == (30.0, 50.0, 40.0, 1.0)


def test_host_target_uses_component_scale_without_visual_fit():
    target = {'dest': '/Game/X/Beast', 'skeletal_mesh': {'path': 'Char_Fake.Mesh.Skel_Fake'},
              'mesh_component_properties': {'translation': {'Z': 0}, 'scale': 0.8},
              'observed': {'collision_radius': 70.0, 'collision_height': 48.0, 'source': 'made up'}}
    assert t.host_target('Beast', target)['mesh_scale'] == 0.8


def test_host_target_prefers_live_mesh_scale_over_archetype_scale():
    target = {'dest': '/Game/X/Beast', 'skeletal_mesh': {'path': 'Char_Fake.Mesh.Skel_Fake'},
              'mesh_component_properties': {'translation': {'Z': 0}, 'scale': 0.4},
              'observed': {'collision_radius': 70.0, 'collision_height': 48.0, 'source': 'made up',
                           'host_mesh_scale': 1.3, 'host_mesh_scale_source': 'synthetic override'}}
    host = t.host_target('Beast', target)
    assert host['mesh_scale'] == 1.3
    assert host['source']['mesh_scale'] == 'synthetic override'


def test_roles_keep_the_host_naming():
    for spec in t.TARGETS.values():
        roles = [r for r, _ in spec['roles']]
        assert roles[0] == 'Idle' and 'Death' in roles
        assert {'PhaseLock_Lift', 'PhaseLock_Loop', 'PhaseLock_Fall', 'PhaseLock_Land'} <= set(roles)
        assert 'idle' not in spec['dest'].lower()   # a replace of "Idle" must only touch the clip name


def test_job_problems_names_every_missing_input():
    with tempfile.TemporaryDirectory() as tmp:
        good = Path(tmp) / 'Mesh_A.gltf'
        good.write_text('{}', encoding='utf-8')
        empty = Path(tmp) / 'Empty.png'
        empty.write_bytes(b'')
        tex = Path(tmp) / 'Tex_A.png'
        tex.write_bytes(b'x')
        complete = {'npcs': [{'id': 'Fake', 'gltf': str(good), 'attachment_meshes': [{'id': 'Piece', 'gltf': str(good)}],
                              'slots': [{'slot': 'S', 'textures': {'p_Diffuse': str(tex)}, 'missing_textures': []}]}]}
        assert t.job_problems(complete) == []
        # The 2026-11-09 failure: no exports, so every glTF was the string "None" and the textures were dropped.
        broken = {'npcs': [{'id': 'Fake', 'gltf': 'None', 'attachment_meshes': [{'id': 'Piece', 'gltf': ''}],
                            'slots': [{'slot': 'S', 'textures': {'p_Normal': str(empty)}, 'missing_textures': ['p_Masks']}]}]}
        problems = t.job_problems(broken)
        assert len(problems) == 4
        # A texture neither the mesh nor the zone step imports does not block the job, missing or empty (review r2).
        unused = {'npcs': [dict(complete['npcs'][0], slots=[{'slot': 'S', 'textures': {'p_Diffuse': str(tex), 'p_CustomPattern': str(empty)},
                                                              'missing_textures': ['p_PatternMask']}])]}
        assert t.job_problems(unused) == []
        assert any('mesh glTF' in p for p in problems) and any('attachment Piece' in p for p in problems)
        assert any('p_Masks not in the UModel output' in p for p in problems) and any('p_Normal' in p for p in problems)


def run_editor_job(folder, previous, worn=2):
    """editor-job on an invented import in `folder` (identity with `worn` pieces, one glTF export), over a host_target.json holding
    `previous` (None: no file). Returns the host_target.json it wrote and what it printed."""
    import contextlib
    import io
    import json
    out, exports = Path(folder) / 'target', Path(folder) / 'exports'
    out.mkdir()
    exports.mkdir()
    (exports / 'Skel_Fake.gltf').write_text('{}', encoding='utf-8')
    pieces = [{'component': f'P.C_{i}', 'bone': f'B{i}', 'static_mesh': 'Pkg.Mesh.Spike', 'materials': []} for i in range(worn or 0)]
    identity = {'targets': {'Beast': {'dest': '/Game/X/Beast', 'skeletal_mesh': {'path': 'Char_Fake.Mesh.Skel_Fake'},
                                      'mesh_component_properties': {'translation': {'Z': 0}},
                                      'observed': {'collision_radius': 70.0, 'collision_height': 48.0, 'source': 'made up'},
                                      'attachments': pieces}}}
    if worn is None:   # an identity.json from before the import read worn pieces: no key at all
        del identity['targets']['Beast']['attachments']
    (out / 'identity.json').write_text(json.dumps(identity), encoding='utf-8')
    if previous is not None:
        (out / 'host_target.json').write_text(json.dumps(previous), encoding='utf-8')
    saved = t.OUT, t.TARGETS, t.npc.find_output
    t.OUT, t.TARGETS = out, {'Beast': {'master': None, 'roles': [('Idle', 'Idle_A'), ('Death', 'Death_A')]}}
    t.npc.find_output = lambda group, name, extensions: exports / f'{name}.gltf' if (exports / f'{name}.gltf').is_file() else None
    (exports / 'Spike.gltf').write_text('{}', encoding='utf-8')
    printed = io.StringIO()
    try:
        with contextlib.redirect_stdout(printed):
            t.editor_job_step(None)
    finally:
        t.OUT, t.TARGETS, t.npc.find_output = saved
    return json.loads((out / 'host_target.json').read_text(encoding='utf-8')), printed.getvalue()


def test_editor_job_keeps_the_attach_steps_placements():
    # 2026-11-12: -Steps prepare reran editor-job, which rewrote host_target.json from scratch and dropped the placements the attach step had
    # written; the range was then built on enemies without their worn parts.
    placed = [{'mesh': '/Game/X/P0.P0', 'bone': 'B0'}, {'mesh': '/Game/X/P1.P1', 'bone': 'B1'}]
    with tempfile.TemporaryDirectory() as folder:
        hosts, printed = run_editor_job(folder, {'Beast': {'mesh': '/Game/Old.Old', 'attachments': placed}})
        assert hosts['Beast']['attachments'] == placed, hosts
        assert hosts['Beast']['mesh'].endswith('Skel_Fake.Skel_Fake')   # what editor-job owns is rewritten
        assert 'kept 2 attachment placements' in printed, printed


def test_editor_job_names_the_attach_step_when_placements_are_missing_or_stale():
    with tempfile.TemporaryDirectory() as folder:
        hosts, printed = run_editor_job(folder, None)
        assert 'attachments' not in hosts['Beast'] and '-Steps attach' in printed and 'none yet' in printed, printed
    with tempfile.TemporaryDirectory() as folder:   # placements of an earlier identity are not kept
        hosts, printed = run_editor_job(folder, {'Beast': {'attachments': [{'bone': 'B0'}]}})
        assert 'attachments' not in hosts['Beast'] and '1 placements whose bones differ from the 2 worn pieces' in printed, printed
    with tempfile.TemporaryDirectory() as folder:   # review r1: same count, one bone swapped by an edit of the target's list: not kept
        hosts, printed = run_editor_job(folder, {'Beast': {'attachments': [{'bone': 'B0'}, {'bone': 'Other'}]}})
        assert 'attachments' not in hosts['Beast'] and 'bones differ' in printed and 'kept' not in printed, printed
    with tempfile.TemporaryDirectory() as folder:   # the right bones in another order are not the attach step's output either
        hosts, printed = run_editor_job(folder, {'Beast': {'attachments': [{'bone': 'B1'}, {'bone': 'B0'}]}})
        assert 'attachments' not in hosts['Beast'], hosts


def test_editor_job_writes_an_empty_list_for_a_pawn_that_wears_nothing():
    # Neither current target is such a pawn. This guards the step from writing no field at all, which
    # tools/phaselock_attachment_check.py refuses as "never written" (exit 2), while the right answer is an empty list.
    with tempfile.TemporaryDirectory() as folder:
        hosts, printed = run_editor_job(folder, None, worn=0)
        assert hosts['Beast']['attachments'] == [] and 'attach' not in printed, printed
    with tempfile.TemporaryDirectory() as folder:   # stale placements from an earlier identity are not kept on a pawn that now wears nothing
        hosts, printed = run_editor_job(folder, {'Beast': {'attachments': [{'bone': 'B0'}]}}, worn=0)
        assert hosts['Beast']['attachments'] == [], hosts


def test_an_identity_that_never_read_the_worn_pieces_is_not_wearing_nothing():
    # Review r2: an identity entry without the attachments key must not become attachments: [] (the attachment check and the host would
    # take that as "wears nothing"). editor-job stops before writing and names the identity step, then attach.
    placed = [{'mesh': '/Game/X/P0.P0', 'bone': 'B0'}]
    with tempfile.TemporaryDirectory() as folder:
        try:
            run_editor_job(folder, {'Beast': {'mesh': '/Game/Old.Old', 'attachments': placed}}, worn=None)
        except SystemExit as stop:
            assert 'no attachments list in identity.json for Beast' in str(stop) and '-Steps prepare,attach' in str(stop), stop
        else:
            raise AssertionError('editor-job accepted an identity without attachments')
        previous = json.loads((Path(folder) / 'target' / 'host_target.json').read_text(encoding='utf-8'))
        assert previous['Beast']['attachments'] == placed   # nothing was written over the earlier file
    # keep_placements on its own leaves the field out and says what to rerun.
    hosts = {'Beast': {}}
    notes = t.keep_placements(hosts, {'Beast': {'attachments': placed}}, {'targets': {'Beast': {}}})
    assert 'attachments' not in hosts['Beast'] and '-Steps prepare,attach' in notes[0], notes


if __name__ == '__main__':
    for name, fn in list(globals().items()):
        if name.startswith('test_'):
            fn()
            print('ok', name)
