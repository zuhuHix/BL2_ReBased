#!/usr/bin/env python3
"""Host-side half of the ambient-NPC asset pipeline (Sanctuary citizens: the male and the female generic NPC).

AI-assisted (Claude), 2026-10-04. Clean room: no game data lives in this file. Everything derived from the installed
game goes under the ignored ``local/`` directory (``local/slice/ambient`` and ``local/external/umodel/slice-npc``) and the
ignored host Content folder. It reuses the extraction, material and clip-conversion code of ``tools/slice_npc_assets.py``
(Marcus, target dummy, pistol) without touching their outputs: its own content root is
``/Game/OpenWillow/Characters/Ambient/<Id>``, its own job and report files live in ``local/slice/ambient``, and the editor
half is the unchanged ``tools/slice_npc_editor.py`` run with that job file.

Steps (each a sub-command; ``all`` runs identity, extract, editor-job in order):

* ``identity``   pawn -> mesh component -> SkeletalMesh / AnimSets / materials / AnimTree, with our own reader
                 (``ow-package --object-dump`` through slice_npc_assets.Reader); object identities come from the reader.
* ``extract``    UModel (external binary) one bounded object at a time: SkeletalMesh (glTF + MD5), each material slot's
                 MaterialInstanceConstant (props + the textures UModel binds), each AnimSet once (MD5 clips).
* ``anims``      convert the MD5 clips the stock data names (AnimTree leaves, perch special moves) to UE bone tracks with
                 ``slice_npc_assets.convert_subset`` (same fit as Marcus; the mapping is UNVERIFIED beyond its structural checks).
* ``editor-job`` write ``local/slice/ambient/editor_job.json`` for ``tools/slice_npc_editor.py`` (modes npcs, anims, preview).
* ``manifest``   merge identity, extraction and editor reports into ``local/slice/ambient/ambient_assets.json``.

UModel materials are heuristic; this tool reports them as "textures bound by UModel's guess", never as verified
material graphs. The census (``tools/census_ambient_npcs.py``) must have been run: the perch clip names come from it.
"""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tools'))
import slice_npc_assets as S  # noqa: E402

AMBIENT = ROOT / 'local/slice/ambient'
CENSUS = ROOT / 'local/slice/ambient_npcs.json'
TOOL = 'tools/ambient_npc_assets.py'
TOWN = ['Sanctuary_Combat', 'Sanctuary_Dynamic', 'Sanctuary_P', 'Sanctuary_Side']
DEST = '/Game/OpenWillow/Characters/Ambient'

# Starting points only; everything else is resolved from the packages. `pawn` is the archetype the stock pawn balance
# (population) or the placed actor uses.
NPCS = {
    'CitizenMale': {'package': 'Sanctuary_Combat', 'pawn': 'GD_GenericNPCMale.Character.Pawn_GenericNPCMale'},
    'CitizenFemale': {'package': 'Sanctuary_Combat', 'pawn': 'GD_GenericNPCFemale.Character.Pawn_GenericNPCFemale'},
}
# The AnimTree names Idle / Walk_F for these pawns (checked in identity: AnimTree_NPCShared); perch clips are added from the census.
HUMAN_BASE = [('idle', 'Idle'), ('walk', 'Walk_F')]
HUMAN_ANIM_SET = 'Anim_Generic_NPC.Anim_Generic_NPC'


def home_package(reader, path):
    """A town package that holds `path` (preferring the persistent level's own sublevels over SanctuaryAir)."""
    rows = {r[0]: r for r in reader.rows(path)}
    for package in TOWN:
        if package in rows:
            return package
    return next(iter(rows), None)


def perch_clips(census):
    """{clip: (anim_set, perch_def names that use it, phase)} for every perch the town's nodes name."""
    clips = {}
    for perch, info in census['perch_defs'].items():
        for entry in info.get('anim_map', []):
            for phase in ('StartAnim', 'IdleAnim', 'StopAnim'):
                move = entry.get(phase)
                if not move:
                    continue
                stack = [move]
                while stack:
                    m = stack.pop()
                    if m.get('clip'):
                        clips.setdefault(m['clip'], {'anim_set': m.get('anim_set') or move.get('anim_set'), 'perches': set()})['perches'].add(S.leaf(perch))
                    stack.extend(m.get('random', []))
    return clips


def anim_sets_of(identity_npc):
    return [a['path'] for a in identity_npc['anim_sets']]


MIC_SCHEMA = """ScalarParameterValues=StructProperty:ScalarParameterValue
TextureParameterValues=StructProperty:TextureParameterValue
VectorParameterValues=StructProperty:VectorParameterValue
"""


def mic_parameters(reader_path, cooked, package, path):
    """Texture / vector / scalar parameters and the parent of a MaterialInstanceConstant, decoded by our reader."""
    import subprocess
    import tempfile
    reader = S.Reader(reader_path, cooked)
    index = reader.identity(path, package)['export_index']
    with tempfile.TemporaryDirectory() as tmp:
        schema = Path(tmp) / 'mic.schema'
        schema.write_text(MIC_SCHEMA)
        run = subprocess.run([str(reader_path), str(cooked / f'{package}.upk'), '--properties', str(index), '--property-offset', '4',
                              '--array-schema', str(schema)], capture_output=True, encoding='utf-8')
    if run.returncode:
        raise RuntimeError(f'{path}: MIC properties failed: {run.stderr.strip()[:200]}')
    props = {p['name']: p for p in json.loads(run.stdout)['properties']}
    out = {'textures': {}, 'vectors': {}, 'scalars': {}, 'parent': None}

    def fields(entry):
        return {f['name']: f.get('value') for f in entry}

    for entry in props.get('TextureParameterValues', {}).get('value') or []:
        f = fields(entry)
        if isinstance(f.get('ParameterValue'), dict) and f['ParameterValue'].get('path'):
            out['textures'][f['ParameterName']] = f['ParameterValue']['path']
    for entry in props.get('VectorParameterValues', {}).get('value') or []:
        f = fields(entry)
        v = f.get('ParameterValue') or {}
        out['vectors'][f['ParameterName']] = [v.get(k) for k in 'RGBA']
    for entry in props.get('ScalarParameterValues', {}).get('value') or []:
        f = fields(entry)
        out['scalars'][f['ParameterName']] = f.get('ParameterValue')
    parent = props.get('Parent', {}).get('value')
    out['parent'] = parent.get('path') if isinstance(parent, dict) else None
    return out


# ---------------------------------------------------------------------------------------------------- identity
def identity_step(args):
    game, cooked, reader_path = S.settings(args)
    reader = S.Reader(reader_path, cooked)
    census = json.loads(CENSUS.read_text(encoding='utf-8')) if CENSUS.is_file() else None
    result = {'tool': TOOL, 'generated': S.now(), 'npcs': {}}
    for name, spec in NPCS.items():
        package = spec['package']
        pawn = reader.dump(spec['pawn'], package)
        component_path = pawn['mesh']
        component = reader.dump(component_path, package)
        ai = reader.dump(pawn['aiclass'], package)
        mesh_path = component['skeletalmesh']
        mesh_home = home_package(reader, mesh_path)
        entry = {'dest': f'{DEST}/{name}', 'package': package, 'pawn': reader.identity(spec['pawn'], package),
                 'pawn_properties': {k: pawn[k] for k in ('bodyclass', 'aiclass', 'mesh', 'allegiance') if k in pawn},
                 'ai_class_properties': {k: ai.get(k) for k in ('defaultdisplayname', 'groundspeed', 'walkingpct', 'aidef',
                                                                'rotationrate', 'slowdowndist', 'slowdownminpct') if k in ai},
                 'mesh_component': reader.identity(component_path, package),
                 'mesh_component_properties': {k: v for k, v in component.items()
                                               if k in ('skeletalmesh', 'animsets', 'animtreetemplate', 'materials', 'physicsasset',
                                                        'translation', 'scale')},
                 'skeletal_mesh': reader.identity(mesh_path, mesh_home),
                 'anim_sets': [reader.identity(p, home_package(reader, p) or package) for p in component.get('animsets', [])],
                 'materials_from_component': [reader.identity(p, home_package(reader, p) or package) for p in component.get('materials', [])],
                 'anim_tree': reader.identity(component['animtreetemplate'], home_package(reader, component['animtreetemplate']) or package)
                 if component.get('animtreetemplate') else None}
        # The AnimTree's sequence leaves name the walk/idle clips.
        names = []
        if component.get('animtreetemplate'):
            tree_home = home_package(reader, component['animtreetemplate']) or package
            tree = reader.dump(component['animtreetemplate'], tree_home)
            for node_path in tree.get('animtickarray', []):
                try:
                    node = reader.dump(node_path, tree_home)
                except RuntimeError:
                    continue
                if 'animseqname' in node and node['animseqname'] != 'None':
                    names.append(node['animseqname'])
        entry['anim_tree_sequence_names'] = names
        wanted = {c for _, c in HUMAN_BASE}
        if not wanted <= set(names):
            sys.exit(f'{name}: AnimTree does not name {sorted(wanted - set(names))}; has {sorted(names)}')
        if HUMAN_ANIM_SET not in anim_sets_of(entry):
            sys.exit(f'{name}: {HUMAN_ANIM_SET} is not one of its AnimSets: {anim_sets_of(entry)}')
        result['npcs'][name] = entry
    if census:
        result['perch_clips'] = {k: {'anim_set': v['anim_set'], 'perches': sorted(v['perches'])} for k, v in sorted(perch_clips(census).items())}
    S.write_json(AMBIENT / 'identity.json', result)
    print(f'identity: wrote {AMBIENT / "identity.json"}')
    return result


# ---------------------------------------------------------------------------------------------------- extraction
def extract_step(args):
    game, cooked, reader_path = S.settings(args)
    umodel = S.umodel_path(args)
    identity = S.read_json(AMBIENT / 'identity.json') or identity_step(args)
    reader = S.Reader(reader_path, cooked)
    records = []
    only = set(args.only or identity['npcs'])

    def go(job_id, group, package, obj, cls, formats, want_group=None):
        record = S.run_umodel(umodel, cooked, job_id, group, package, obj, cls, formats, want_group)
        state = 'ok' if record['success'] else 'FAILED'
        print(f'  {job_id}: {state} exit={record["exit_code"]} {record["elapsed_seconds"]}s files={len(record["outputs"])} '
              f'bytes={record["output_bytes"]} found={record["objects_found"]}')
        records.append(record)
        return record

    anim_sets = {}
    for name, npc in identity['npcs'].items():
        if name not in only:
            continue
        group = f'Amb_{name}'
        shutil.rmtree(S.UMODEL_OUT / group, ignore_errors=True)
        package = npc['skeletal_mesh']['package']
        mesh = S.leaf(npc['skeletal_mesh']['path'])
        print(f'{name}:')
        r = go(f'{group}.mesh.gltf', group, package, mesh, 'SkeletalMesh', ['gltf'])
        log_text = (ROOT / r['log']).read_text(encoding='utf-8') if r['log'] else ''
        go(f'{group}.mesh.md5', group, package, mesh, 'SkeletalMesh', ['md5'])
        slots, overrides = [], npc.get('materials_from_component', [])
        for index, (default_mic, default_package) in enumerate(S.mesh_materials_from_log(log_text)):
            override = overrides[index] if index < len(overrides) and 'path' in overrides[index] else None
            if override:
                mic_path, mic_package = override['path'], override['package']
            else:
                mic_path = S.Reader(reader_path, cooked).db.execute(
                    "select path from ex where name=? and pkg=? and class like '%MaterialInstanceConstant'", (default_mic, default_package)).fetchone()[0]
                mic_package = default_package
            if mic_path in [s_['path'] for s_ in slots]:
                continue
            info = mic_parameters(reader_path, cooked, mic_package, mic_path)
            # UModel cannot resolve a MIC whose textures live in another package (Sanctuary_P holds the male body
            # textures, the MIC may sit in Sanctuary_Dynamic), so the textures named by our reader's decode of the MIC are
            # exported one by one from a package that holds them.
            textures = []
            for parameter, texture_path in info['textures'].items():
                if parameter not in ('p_Diffuse', 'p_Normal'):
                    textures.append({'parameter': parameter, 'object': texture_path, 'exported': False})
                    continue
                home = home_package(reader, texture_path)
                tr = go(f'{group}.texture.{S.leaf(texture_path)}', group, home, S.leaf(texture_path), 'Texture2D', ['png'])
                textures.append({'parameter': parameter, 'object': texture_path, 'package': home, 'exported': tr['success']})
            slots.append({'name': default_mic, 'material': S.leaf(mic_path), 'material_path': mic_path, 'path': mic_path,
                          'package': mic_package, 'overrides_mesh_default': default_mic if override else None,
                          'textures': [t for t in textures if t['exported'] or t['parameter'] not in ('p_Diffuse', 'p_Normal')],
                          'parent': info['parent'], 'scalars': info['scalars'], 'vectors': info['vectors'],
                          'identity': reader.identity(mic_path, mic_package),
                          'texture_export_failures': [t['object'] for t in textures if t['parameter'] in ('p_Diffuse', 'p_Normal') and not t['exported']]})
        npc['mesh_material_slots'] = slots
        for ident in npc['anim_sets']:
            anim_sets.setdefault(ident['path'], ident)
    # AnimSets: each once, into a shared group (clip lookups search this group by AnimSet name).
    wanted_sets = {HUMAN_ANIM_SET}
    shutil.rmtree(S.UMODEL_OUT / 'Amb_AnimSets', ignore_errors=True) if not args.only else None
    for path in sorted(wanted_sets):
        ident = anim_sets[path]
        go(f'Amb_AnimSets.animset.{path}', 'Amb_AnimSets', ident['package'], S.leaf(path), 'AnimSet', ['md5'],
           want_group=path.split('.')[0])
    S.write_json(AMBIENT / 'extract.json', {'tool': TOOL, 'generated': S.now(), 'umodel': umodel.name, 'records': records})
    S.write_json(AMBIENT / 'identity.json', identity)
    failed = [r['id'] for r in records if not r['success']]
    print(f'extract: {len(records)} jobs, {len(failed)} failed {failed}')


# ---------------------------------------------------------------------------------------------------- animations
def clip_roles(identity):
    """{role: (anim set path, clip)} for the humans: AnimTree leaves plus every perch clip the census names."""
    roles = {role: (HUMAN_ANIM_SET, clip) for role, clip in HUMAN_BASE}
    for clip, info in (identity.get('perch_clips') or {}).items():
        if info['anim_set'] == HUMAN_ANIM_SET:
            roles['perch_' + clip.lower()] = (HUMAN_ANIM_SET, clip)
    return roles


def anims_step(args):
    identity = S.read_json(AMBIENT / 'identity.json')
    roles = clip_roles(identity)
    report = {'tool': TOOL, 'generated': S.now(), 'npcs': {}}
    for name, npc in identity['npcs'].items():
        if args.only and name not in args.only:
            continue
        reference = AMBIENT / f'ref_pose_{name}.json'
        if not reference.is_file():
            print(f'anims: {name}: reference pose {reference.name} missing (run editor mode npcs first)')
            continue
        group = f'Amb_{name}'
        mesh_md5 = S.find_output(group, S.leaf(npc['skeletal_mesh']['path']), ['md5mesh'])
        clips, used = {}, {}
        for role, (anim_set, clip) in roles.items():
            hits = sorted((S.UMODEL_OUT / 'Amb_AnimSets').rglob(f'{clip}.md5anim'))
            hit = next((h for h in hits if f'{h.parent.parent.name}.{h.parent.name}' == anim_set), None)
            if hit is None:
                used[role] = {'anim_set': anim_set, 'clip': clip, 'status': 'not exported'}
                continue
            clips[role] = hit
            used[role] = {'anim_set': anim_set, 'clip': clip}
        tracks, details = S.convert_subset(mesh_md5, reference, clips)
        S.write_json(AMBIENT / f'anim_tracks_{name}.json', tracks)
        for role in clips:
            used[role].update(details[role])
        report['npcs'][name] = {'fit_error': details['fit_error'], 'roles': used, 'tracks_json': f'local/slice/ambient/anim_tracks_{name}.json'}
        print(f'anims: {name}: {len(clips)} clips converted (fit error {details["fit_error"]:.2e})')
    S.write_json(AMBIENT / 'anims.json', report)


# ---------------------------------------------------------------------------------------------------- attachments (hair, hats, heads)
ATTACH_DEST = f'{DEST}/Attachments'


def attachments_step(args):
    """Hair, hats, gear and head variants seen on the live citizens of a real-game capture (tools/real_game/scripts/ambient_npcs.py
    amb_compose): UModel exports of each static mesh and of the textures its material (and each head material) names, resolved with
    our reader; writes local/slice/ambient/attach_job.json for tools/ambient_npc_attach_editor.py."""
    game, cooked, reader_path = S.settings(args)
    umodel = S.umodel_path(args)
    reader = S.Reader(reader_path, cooked)
    compose = json.loads(Path(args.compose).read_text(encoding='utf-8'))
    group = 'Amb_Attach'
    shutil.rmtree(S.UMODEL_OUT / group, ignore_errors=True)
    records = []

    def go(job_id, package, obj, cls, formats):
        record = S.run_umodel(umodel, cooked, job_id, group, package, obj, cls, formats)
        print(f'  {job_id}: {"ok" if record["success"] else "FAILED"} files={len(record["outputs"])}')
        records.append(record)
        return record

    textures_done = {}

    def texture_file(path):
        """Export one Texture2D (png) once; return the file or None."""
        if path in textures_done:
            return textures_done[path]
        home = home_package(reader, path)
        file = None
        if home:
            r = go(f'{group}.texture.{S.leaf(path)}', home, S.leaf(path), 'Texture2D', ['png'])
            hit = S.find_output(group, S.leaf(path), ['png', 'tga'])
            file = str(hit) if hit and r['success'] else None
        textures_done[path] = file
        return file

    def mic_textures(mic_path):
        home = home_package(reader, mic_path)
        if not home:
            return {}
        info = mic_parameters(reader_path, cooked, home, mic_path)
        out = {}
        for parameter, tex in info['textures'].items():
            if parameter in ('p_Diffuse', 'p_Normal', 'p_Masks'):
                f = texture_file(tex)
                if f:
                    out[{'p_Diffuse': 'Diffuse', 'p_Normal': 'Normal', 'p_Masks': 'Masks'}[parameter]] = f
        return out

    def mic_vectors(mic_path):
        home = home_package(reader, mic_path)
        return mic_parameters(reader_path, cooked, home, mic_path)['vectors'] if home else {}

    meshes, materials, heads, bodies = {}, {}, {}, {}
    for pawn in compose:
        kind = {'Skel_GenericMale': 'CitizenMale', 'Skel_GenericFemale': 'CitizenFemale'}.get(pawn['mesh'].rsplit('.', 1)[-1])
        head = pawn['materials'][0] if pawn['materials'] else None
        if kind and head and head.get('textures', {}).get('p_Diffuse'):
            key = (kind, head['textures']['p_Diffuse'])
            if key not in heads:
                heads[key] = {'kind': kind, 'id': f'{kind}_{S.leaf(head["textures"]["p_Diffuse"])}', 'parent': head.get('parent'),
                              'textures': {'Diffuse': texture_file(head['textures']['p_Diffuse'])}}
                if head['textures'].get('p_Normal'):
                    heads[key]['textures']['Normal'] = texture_file(head['textures']['p_Normal'])
                if head['textures'].get('p_Masks'):
                    heads[key]['textures']['Masks'] = texture_file(head['textures']['p_Masks'])
                if head.get('parent'):
                    heads[key]['vectors'] = mic_vectors(head['parent'])
        body = pawn['materials'][1] if len(pawn['materials']) > 1 else None
        if kind and body and body.get('parent') and kind not in bodies:
            bodies[kind] = {'kind': kind, 'id': kind, 'parent': body['parent'], 'textures': mic_textures(body['parent']),
                            'vectors': mic_vectors(body['parent'])}
        for att in pawn['attachments']:
            mesh_path = att.get('static_mesh')
            if not mesh_path or mesh_path in meshes:
                pass
            if mesh_path and mesh_path not in meshes:
                package = home_package(reader, mesh_path)
                if not package:
                    meshes[mesh_path] = {'error': 'not in the town packages'}
                    continue
                r = go(f'{group}.mesh.{S.leaf(mesh_path)}', package, S.leaf(mesh_path), 'StaticMesh', ['gltf'])
                gltf = S.find_output(group, S.leaf(mesh_path), ['gltf'])
                default_mics = S.mesh_materials_from_log((ROOT / r['log']).read_text(encoding='utf-8')) if r['log'] else []
                meshes[mesh_path] = {'id': S.leaf(mesh_path), 'package': package, 'gltf': str(gltf) if gltf and r['success'] else None,
                                     'default_materials': [], 'bones': set()}
                for name, mic_package in default_mics:
                    row = reader.db.execute("select path from ex where name=? and pkg=? and class like '%MaterialInstanceConstant'", (name, mic_package)).fetchone()
                    if row:
                        meshes[mesh_path]['default_materials'].append({'name': name, 'path': row[0], 'textures': mic_textures(row[0]),
                                                                        'vectors': mic_vectors(row[0])})
            if mesh_path in meshes and 'bones' in meshes[mesh_path]:
                meshes[mesh_path]['bones'].add(att['bone'])
            for mat in att.get('materials') or []:
                if mat and mat.get('parent') and mat['parent'] not in materials:
                    materials[mat['parent']] = {'id': 'MI_' + S.leaf(mat['parent']), 'textures': mic_textures(mat['parent']),
                                                'vectors': mic_vectors(mat['parent'])}
    job = {'tool': TOOL, 'generated': S.now(), 'slice_dir': str(AMBIENT), 'dest': ATTACH_DEST,
           'meshes': [{**m, 'bones': sorted(m['bones'])} for m in meshes.values() if m.get('gltf')],
           'parent_materials': list(materials.values()),
           'heads': list(heads.values()), 'bodies': list(bodies.values()),
           'missing_meshes': [k for k, m in meshes.items() if not m.get('gltf')]}
    S.write_json(AMBIENT / 'attach_job.json', job)
    S.write_json(AMBIENT / 'attach_extract.json', {'tool': TOOL, 'generated': S.now(), 'records': records})
    failed = [r['id'] for r in records if not r['success']]
    print(f'attachments: {len(job["meshes"])} meshes, {len(job["parent_materials"])} attachment materials, {len(job["heads"])} head variants, '
          f'{len(records)} UModel jobs, failed {failed}, missing {job["missing_meshes"]}')


# ---------------------------------------------------------------------------------------------------- editor job
def editor_job_step(args):
    identity = S.read_json(AMBIENT / 'identity.json')
    roles = clip_roles(identity)
    job = {'tool': TOOL, 'generated': S.now(), 'npcs': [], 'slice_dir': str(AMBIENT)}
    for name, npc in identity['npcs'].items():
        group = f'Amb_{name}'
        mesh = S.leaf(npc['skeletal_mesh']['path'])
        gltf = S.find_output(group, mesh, ['gltf'])
        slots = []
        for slot in npc.get('mesh_material_slots', []):
            textures = {}
            for tex in slot['textures']:
                file = S.find_output(group, S.leaf(tex['object']), ['png', 'tga'])
                if file:
                    textures[tex['parameter']] = str(file)
            slots.append({'slot': slot['name'], 'material': slot['material'], 'textures': textures,
                          'scalars': slot.get('scalars', {}), 'vectors': slot.get('vectors', {})})
        job['npcs'].append({'id': name, 'dest': npc['dest'], 'mesh_name': mesh, 'gltf': str(gltf), 'slots': slots,
                            'tracks': str(AMBIENT / f'anim_tracks_{name}.json'), 'roles': list(roles),
                            'reference_out': str(AMBIENT / f'ref_pose_{name}.json')})
    S.write_json(AMBIENT / 'editor_job.json', job)
    print(f'editor-job: wrote {AMBIENT / "editor_job.json"}')


# ---------------------------------------------------------------------------------------------------- manifest
def manifest_step(args):
    identity = S.read_json(AMBIENT / 'identity.json')
    anims = S.read_json(AMBIENT / 'anims.json', {})
    editor = {m: S.read_json(AMBIENT / f'editor_report_{m}.json', {}) for m in ('npcs', 'anims', 'preview')}
    manifest = {'schema': 'openwillow.ambient_npc_assets/1', 'generated': S.now(), 'tool': TOOL,
                'status_note': ("Materials are textures bound by UModel's guess, not verified material graphs. UNVERIFIED: the "
                                'MD5-to-UE animation mapping beyond structural checks; no hair/hat/accessory static meshes, '
                                'no FaceFX, no layered body-composition variants (one body + head material per kind).'),
                'npcs': {}, 'use': {}}
    for name, npc in identity['npcs'].items():
        rec = editor['npcs'].get(name, {})
        ar = editor['anims'].get(name, {})
        roles = (anims.get('npcs', {}).get(name, {}) or {}).get('roles', {})
        manifest['npcs'][name] = {
            'source_identity': {k: npc[k] for k in ('pawn', 'skeletal_mesh', 'anim_sets', 'anim_tree', 'mesh_component_properties',
                                                    'ai_class_properties', 'anim_tree_sequence_names')},
            'import_status': rec.get('status', 'not imported'),
            'ue': {k: rec.get(k) for k in ('skeletal_mesh', 'skeleton', 'physics_asset', 'materials', 'textures', 'bounds_extent',
                                           'bone_count', 'material_slots')},
            'anims': {role: {**info, 'ue_asset': (ar.get('assets') or {}).get(role)} for role, info in roles.items()}}
        t = npc['mesh_component_properties'].get('translation') or {}
        manifest['use'][name] = {
            'skeletal_mesh': rec.get('skeletal_mesh'), 'skeleton': rec.get('skeleton'),
            'mesh_offset': [t.get('X', 0), t.get('Y', 0), t.get('Z', 0)],
            'ground_speed': (npc.get('ai_class_properties') or {}).get('groundspeed'),
            'display_name': (npc.get('ai_class_properties') or {}).get('defaultdisplayname'),
            'anims': {role: a['ue_asset'] for role, a in manifest['npcs'][name]['anims'].items() if a['ue_asset']},
            'import_status': manifest['npcs'][name]['import_status']}
    S.write_json(AMBIENT / 'ambient_assets.json', manifest)
    print(f'manifest: wrote {AMBIENT / "ambient_assets.json"}')


STEPS = {'attachments': attachments_step, 'identity': identity_step, 'extract': extract_step, 'anims': anims_step, 'editor-job': editor_job_step,
         'manifest': manifest_step}


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('step', choices=sorted(STEPS) + ['all'])
    parser.add_argument('--reader', default=str(ROOT / 'build/Release/ow-package.exe'))
    parser.add_argument('--game', help='Borderlands 2 folder (default $OPENWILLOW_BL2)')
    parser.add_argument('--umodel', help='umodel.exe (default $OPENWILLOW_UMODEL)')
    parser.add_argument('--only', nargs='*', help='restrict extract/anims to these NPC ids')
    parser.add_argument('--compose', default=str(ROOT / 'local/realgame/ambient/compose_t3.json'), help='real-game amb_compose output (attachments step)')
    args = parser.parse_args()
    if args.step == 'all':
        for step in ('identity', 'extract', 'editor-job'):
            STEPS[step](args)
    else:
        STEPS[args.step](args)


if __name__ == '__main__':
    main()
