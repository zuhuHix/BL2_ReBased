#!/usr/bin/env python3
"""Prepare a stock Phaselock target (the bullymong) for the host: mesh, idle, death and the four PhaseLock clips.

AI-assisted. Clean-room: no game data lives in this file. It reuses the slice NPC pipeline
(``tools/slice_npc_assets.py``: our reader for identities, UModel for the bounded exports, the fitted MD5-to-UE clip
conversion) with its own output folders, so the slice's Marcus/dummy files are not touched. Everything derived from
the installed game is written under the ignored ``local/phaselock/target`` and ``local/external/umodel/phaselock-target``;
the UE content goes to the ignored ``/Game/OpenWillow/Characters/PhaselockTarget/<Id>``.

Why: ``SpecialMove_PhaseLock`` plays ``PhaseLock_Lift/Loop/Fall/Land`` from the lifted pawn's own AnimSets
(docs/verification/NATIVE_PHASELOCK_PRESENTATION.md section 2). The slice's target dummy has none of them, so the
host's lifted-target animation never ran. The bullymong is the enemy every real-game Phaselock capture was taken on
(``local/realgame/phaselock``), and its ``Anim_PrimalBeast.Unarmed`` set carries all four clips.

Steps (``all`` runs identity, extract, editor-job; ``anims`` needs the editor's reference pose first):

* ``identity``   pawn -> mesh component -> SkeletalMesh / AnimSets / AnimTree / collision cylinder (our reader);
                 checks that the four PhaseLock clips and the chosen idle/death clips exist in the exported set.
* ``extract``    UModel: mesh glTF + MD5, the component's material instances, the AnimSet as MD5 clips.
* ``editor-job`` the job file for ``tools/slice_npc_editor.py`` (modes ``npcs`` then ``anims``), plus
                 ``host_target.json``: the UE asset paths and the pawn numbers the host reads.
* ``anims``      convert the chosen MD5 clips to UE bone tracks (needs ``ref_pose_<Id>.json`` from the editor).
* ``attachments`` add the worn static meshes to ``host_target.json`` (needs the editor's ``attach`` report).

Run the whole thing with ``tools/import_phaselock_target.ps1``.
"""
import argparse
import json
from pathlib import Path
import shutil
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tools'))
import slice_npc_assets as npc  # noqa: E402

OUT = ROOT / 'local/phaselock/target'
UMODEL_OUT = ROOT / 'local/external/umodel/phaselock-target'
TOOL = 'tools/prepare_phaselock_target.py'

# Starting points only; everything else is resolved from the data by identity().
# The roles named Idle / PhaseLock_* matter to the host: AOpenWillowCombatTarget::UseStockPawn finds the PhaseLock clips
# by replacing "Idle" in the idle clip's asset path (Anim_<Id>_Idle -> Anim_<Id>_PhaseLock_Lift).
TARGETS = {
    'Bullymong': {
        'package': 'Ice_Dynamic',   # Three Horns Divide, where the real-game captures were taken; has the pawn and its AnimSet
        # The "Adult Bullymong" of the real-game frames: its balance (PawnBalance_PrimalBeastAdult) names this archetype, whose
        # component material is the tan/brown/rust Mati_BullymongAdult with no scale override (1.0, as the live read) and a
        # 150 x 90 cylinder (as observed). The plain Pawn_PrimalBeast used before carries the near-white Mati_Bullymong
        # (the host's icy white-lilac target) and the 100-unit cylinder and 0.7 scale that conflicted with the live reads.
        'pawn': 'GD_PrimalBeastAdult.Character.Pawn_PrimalBeastAdult',
        'dest': '/Game/OpenWillow/Characters/PhaselockTarget/Bullymong',
        # What the archetype's body composition must wear, as (component, bone); read live on one Adult, 2026-11-06. Any other set
        # stops the tool (check_worn), so a filter or package change cannot quietly drop a piece.
        'attachments': [('StaticMeshComponent_3114', 'lElbow1'), ('StaticMeshComponent_3115', 'lElbow2'),
                        ('StaticMeshComponent_3116', 'rElbow1'), ('StaticMeshComponent_3117', 'rElbow2'),
                        ('StaticMeshComponent_3118', 'Head'), ('StaticMeshComponent_3119', 'Spine2')],
        'roles': [('Idle', 'Idle_var1'), ('Death', 'Death_Fire_var1'),
                  ('PhaseLock_Lift', 'PhaseLock_Lift'), ('PhaseLock_Loop', 'PhaseLock_Loop'),
                  ('PhaseLock_Fall', 'PhaseLock_Fall'), ('PhaseLock_Land', 'PhaseLock_Land')],
        # Collision cylinder observed on the live adult bullymong (local/realgame/phaselock/size_rule/adult68, SDK reads
        # 2026-10-03): 150 x 90 (the archetype says 100 x 90). A live component read on 2026-10-21 recorded Mesh.Scale 1.0,
        # while the archetype data says 0.7. The corrected-FOV Round 20 frame boxes match the live 1.0 read better; use
        # the measured component scale. UE3-to-UE5 rendering remains UNVERIFIED.
        'observed': {'collision_radius': 150.0, 'collision_height': 90.0,
                     'source': 'local/realgame/phaselock/size_rule/adult68 (SDK read)',
                     # UpdateLiftedPawnMeshOffset centres Mesh.Bounds (the stock physics asset, not imported) on the pawn's
                     # Location. Live (2026-10-21, local/realgame/phaselock/target_anim run2): Mesh.Translation.Z went from
                     # -102.1 at rest to -221.5 through the hold (-119.5); the host's bone-box centring moved it -60.8. The
                     # host therefore centres the bones' box this much below the Location (a fit for this pawn, UNVERIFIED).
                     'centre_bias': 58.7,
                     'host_mesh_scale': 1.0,
                     'host_mesh_scale_source': 'local/realgame/phaselock/size_rule/adult68 (2026-10-21 SDK read)'},
    },
    # The range's psycho (tools/prepare_range.py). Its set also has walk, run, melee and flinch clips, which the range's enemy plays
    # (the host finds them by role name next to the idle). No live read of this pawn exists: the cylinder and the scale are the archetype's
    # (host_target's fallback). The set's only clip named as a death is Death_Shield; which death the game plays is UNVERIFIED.
    'Psycho': {
        'package': 'Ice_Dynamic',
        'pawn': 'GD_Psycho.Character.Pawn_Psycho',
        'dest': '/Game/OpenWillow/Characters/PhaselockTarget/Psycho',
        'roles': [('Idle', 'Idle'), ('Death', 'Death_Shield'), ('Walk', 'Patrol_F'), ('Run', 'Run_F_var1'),
                  ('Melee', 'Attack_Melee_var1'), ('Hit', 'HardFlinch_L_var1'),
                  ('PhaseLock_Lift', 'PhaseLock_Lift'), ('PhaseLock_Loop', 'PhaseLock_Loop'),
                  ('PhaseLock_Fall', 'PhaseLock_Fall'), ('PhaseLock_Land', 'PhaseLock_Land')],
        # What the body composition wears by default (read with our reader): a mask and an axe.
        'attachments': [('StaticMeshComponent_14878', 'Head'), ('StaticMeshComponent_14884', 'R_Hand')],
        'observed': {'source': 'archetype collision cylinder (our reader); no live read of this pawn'},
    },
}


def use_own_folders():
    """Point the shared pipeline's module-level folders at this tool's own output."""
    npc.SLICE, npc.UMODEL_OUT = OUT, UMODEL_OUT


def group_of(anim_set):
    """'Anim_PrimalBeast.Unarmed' -> ('Anim_PrimalBeast', 'Unarmed'): UModel -groups writes <package>/<group>/<set>/."""
    group, _, name = anim_set.rpartition('.')
    return group, name


def worn_attachments(composition, read_component):
    """The static meshes a body composition wears by default: rows of its Attachments with switch 0, a StaticMeshComponent and a
    named bone. A switched row (the rocks a bullymong picks up) is off until its switch is set. read_component dumps a component."""
    worn = []
    for row in composition.get('Attachments', []):
        data = row.get('Data') or {}
        part = data.get('ComponentData') or {}
        component = part.get('Component')
        bone = part.get('MeshSocketName')
        if row.get('SwitchIdentifier') != 0 or not component or not bone or bone == 'None' or 'StaticMeshComponent' not in component:
            continue
        props = read_component(component)
        if not props.get('staticmesh'):
            sys.exit(f'{component}: an attached component without a static mesh')
        rotation, translation, scale3d = props.get('rotation', {}), props.get('translation', {}), props.get('scale3d', {})
        worn.append({'component': component, 'bone': bone, 'static_mesh': props['staticmesh'], 'materials': props.get('materials', []),
                     'comp_translation': [float(translation.get(k, 0.0)) for k in 'XYZ'],
                     'comp_rotation': [int(rotation.get(k, 0)) for k in ('Pitch', 'Yaw', 'Roll')],
                     'comp_scale': float(props.get('scale', 1.0)),
                     'comp_scale3d': [float(scale3d.get(k, 1.0)) for k in 'XYZ']})
    return worn


def piece_package(packages, preferred):
    """The package to export a worn piece from: a piece need not be an export of the pawn's package (it can be an import there). The pawn's
    package when it holds it, else another package of the same map, else the first by name. Stops when no package exports it."""
    if not packages:
        sys.exit(f'a worn piece is exported by no package (preferred {preferred}): check the census index')
    if preferred in packages:
        return preferred
    prefix = preferred.split('_')[0] + '_'
    return next((p for p in sorted(packages) if p.startswith(prefix)), sorted(packages)[0])


def locate_pieces(worn, preferred, packages_of):
    """Writes where each piece's mesh and material are exported into its row. packages_of(path) lists the packages exporting a path."""
    for att in worn:
        att['package'] = piece_package(packages_of(att['static_mesh']), preferred)
        att['material_package'] = piece_package(packages_of(att['materials'][0]), preferred) if att['materials'] else None


def check_worn(name, worn, expected):
    """Stop unless worn is exactly the expected (component, bone) pairs, each once."""
    found = [(npc.leaf(att['component']), att['bone']) for att in worn]
    duplicates = sorted({pair for pair in found if found.count(pair) > 1})
    missing = sorted(set(map(tuple, expected)) - set(found))
    extra = sorted(set(found) - set(map(tuple, expected)))
    if duplicates or missing or extra:
        sys.exit(f'{name}: worn attachments differ from the expected set: missing {missing}, unexpected {extra}, duplicated {duplicates}')


def attachment_meshes(worn):
    """Each static mesh once, with the one material its components give it. Two materials on one mesh stop the tool: the import
    sets the material on the asset, so it could not show both."""
    meshes = {}
    for att in worn:
        mesh = npc.leaf(att['static_mesh'])
        material = npc.leaf(att['materials'][0]) if att['materials'] else None
        if meshes.setdefault(mesh, material) != material:
            sys.exit(f'{mesh}: its components name two materials, {meshes[mesh]} and {material}')
    return meshes


def identity_step(args):
    _, cooked, reader_path = npc.settings(args)
    reader = npc.Reader(reader_path, cooked)
    result = {'tool': TOOL, 'generated': npc.now(), 'targets': {}}
    for name, spec in TARGETS.items():
        package = spec['package']
        pawn = reader.dump(spec['pawn'], package)
        component = reader.dump(pawn['mesh'], package)
        cylinder = reader.dump(pawn['cylindercomponent'], package) if pawn.get('cylindercomponent') else {}
        entry = {'dest': spec['dest'], 'package': package, 'pawn': reader.identity(spec['pawn'], package),
                 'mesh_component': reader.identity(pawn['mesh'], package),
                 'mesh_component_properties': {k: component[k] for k in
                                               ('skeletalmesh', 'animsets', 'animtreetemplate', 'materials',
                                                'translation', 'scale', 'scale3d') if k in component},
                 'archetype_cylinder': {k: cylinder[k] for k in ('collisionradius', 'collisionheight') if k in cylinder},
                 'skeletal_mesh': reader.identity(component['skeletalmesh'], package),
                 'anim_sets': [reader.identity(p, package) for p in component.get('animsets', [])],
                 'materials_from_component': [reader.identity(p, package) for p in component.get('materials', [])],
                 'observed': spec['observed']}
        body = reader.dump(pawn['bodyclass'], package)
        entry['attachments'] = worn_attachments(body.get('bodycomposition') or {}, lambda path: reader.dump(path, package))
        check_worn(name, entry['attachments'], spec['attachments'])
        locate_pieces(entry['attachments'], package, lambda path: [row[0] for row in reader.rows(path)])
        if len(entry['anim_sets']) != 1:
            sys.exit(f'{name}: expected one AnimSet on the mesh component, got {component.get("animsets")}')
        result['targets'][name] = entry
        print(f'identity: {name}: mesh {component["skeletalmesh"]}, set {component["animsets"][0]}, '
              f'translation {component.get("translation")}, scale {component.get("scale")}, cylinder {entry["archetype_cylinder"]}, '
              f'{len(entry["attachments"])} attachments')
    npc.write_json(OUT / 'identity.json', result)
    return result


def extract_step(args):
    _, cooked, _ = npc.settings(args)
    umodel = npc.umodel_path(args)
    identity = npc.read_json(OUT / 'identity.json') or identity_step(args)
    records = []

    def go(job_id, group, package, obj, cls, formats, want_group=None):
        record = npc.run_umodel(umodel, cooked, job_id, group, package, obj, cls, formats, want_group)
        print(f'  {job_id}: {"ok" if record["success"] else "FAILED"} exit={record["exit_code"]} '
              f'{record["elapsed_seconds"]}s files={len(record["outputs"])} bytes={record["output_bytes"]}')
        records.append(record)
        return record

    for name, target in identity['targets'].items():
        shutil.rmtree(UMODEL_OUT / name, ignore_errors=True)
        package = target['package']
        mesh = npc.leaf(target['skeletal_mesh']['path'])
        log = go(f'{name}.mesh.gltf', name, package, mesh, 'SkeletalMesh', ['gltf'])
        go(f'{name}.mesh.md5', name, package, mesh, 'SkeletalMesh', ['md5'])
        text = (ROOT / 'local/phaselock/target/logs' / f'{name}.mesh.gltf.log').read_text(encoding='utf-8') if log else ''
        overrides = target.get('materials_from_component', [])
        slots = []
        for index, (default_mic, default_package) in enumerate(npc.mesh_materials_from_log(text)):
            override = overrides[index] if index < len(overrides) and 'path' in overrides[index] else None
            mic = npc.leaf(override['path']) if override else default_mic
            mic_package = override['package'] if override else default_package
            if mic in [s['material'] for s in slots]:
                continue
            record = go(f'{name}.material.{mic}', name, mic_package, mic, 'MaterialInstanceConstant', ['png'])
            props = npc.find_output(name, mic, ['props.txt'])
            info = npc.mic_textures(props) if props else {'textures': []}
            slots.append({'name': default_mic, 'material': mic, 'package': mic_package, 'props': str(props) if props else None,
                          **info, 'export_record': record['id']})
        target['mesh_material_slots'] = slots
        pieces = {npc.leaf(att['static_mesh']): att for att in target['attachments']}
        for mesh_name, material in attachment_meshes(target['attachments']).items():
            go(f'{name}.attachment.{mesh_name}', name, pieces[mesh_name]['package'], mesh_name, 'StaticMesh', ['gltf'])
            # A piece with its own material: exported like the body's, so the zone step makes its instance. A piece that names none keeps what
            # its import makes (phaselock_target_attach_editor.py).
            if material and material not in [s['material'] for s in slots]:
                mic_package = pieces[mesh_name]['material_package']
                record = go(f'{name}.material.{material}', name, mic_package, material, 'MaterialInstanceConstant', ['png'])
                props = npc.find_output(name, material, ['props.txt'])
                info = npc.mic_textures(props) if props else {'textures': []}
                slots.append({'name': material, 'material': material, 'package': mic_package, 'props': str(props) if props else None,
                              **info, 'export_record': record['id']})
        anim_set = target['anim_sets'][0]
        group, set_name = group_of(anim_set['path'])
        go(f'{name}.animset', name, anim_set['package'], set_name, 'AnimSet', ['md5'], want_group=group)
        # Every clip the host needs must be in the exported set.
        missing = [clip for _, clip in TARGETS[name]['roles'] if clip_file(name, anim_set['path'], clip) is None]
        if missing:
            sys.exit(f'{name}: clips not exported from {anim_set["path"]}: {missing}')
    npc.write_json(OUT / 'extract.json', {'tool': TOOL, 'generated': npc.now(), 'umodel': umodel.name, 'records': records})
    npc.write_json(OUT / 'identity.json', identity)
    failed = [r['id'] for r in records if not r['success']]
    print(f'extract: {len(records)} jobs, {len(failed)} failed {failed}')
    if failed:
        sys.exit(1)


def clip_file(name, anim_set, clip):
    group, set_name = group_of(anim_set)
    hits = [h for h in sorted((UMODEL_OUT / name).rglob(f'{clip}.md5anim'))
            if h.parent.name == set_name and h.parent.parent.name == group]
    return hits[0] if hits else None


def host_target(name, target):
    """What the host reads (AOpenWillowCombatTarget::UseCommandLinePawn): UE asset paths and the pawn numbers."""
    dest = target['dest']
    mesh = npc.leaf(target['skeletal_mesh']['path'])
    props = target['mesh_component_properties']
    observed = target['observed']
    # UE3 places the mesh at Location + Translation (the cylinder centre is the pawn's Location); the host target's root
    # is on the floor, so the mesh goes at CollisionHeight + Translation.Z above it.
    # Prefer the live component read when available; otherwise use the archetype's package scale. The rendering decision
    # is UNVERIFIED, but Round 20's corrected-FOV comparison favoured the live 1.0 read over the smaller package value.
    scale = float(observed.get('host_mesh_scale', props.get('scale', 1.0)))
    # A live read of the cylinder wins; without one, the archetype's.
    cylinder = target.get('archetype_cylinder', {})
    radius = float(observed.get('collision_radius', cylinder.get('collisionradius', 0.0)))
    height = float(observed.get('collision_height', cylinder.get('collisionheight', 0.0)))
    # Translation is in the pawn's frame (not scaled by the mesh's own Scale).
    z = height + float(props.get('translation', {}).get('Z', 0.0))
    anim = lambda role: f'{dest}/Animations/Anim_{name}_{role}.Anim_{name}_{role}'  # noqa: E731
    return {'id': name, 'mesh': f'{dest}/Meshes/{mesh}/SkeletalMeshes/{mesh}.{mesh}',
            'idle': anim('Idle'), 'death': anim('Death'), 'mesh_offset_z': z, 'mesh_scale': scale,
            'collision_radius': radius, 'collision_height': height,
            'centre_bias': observed.get('centre_bias', 0.0),
            'source': {'translation': 'archetype mesh component (our reader)',
                       'mesh_scale': observed.get('host_mesh_scale_source', 'archetype mesh component (our reader)'),
                       'cylinder': observed['source']}}


def editor_job_step(args):
    identity = npc.read_json(OUT / 'identity.json')
    job = {'tool': TOOL, 'generated': npc.now(), 'npcs': [], 'slice_dir': str(OUT)}
    hosts = {}
    for name, target in identity['targets'].items():
        mesh = npc.leaf(target['skeletal_mesh']['path'])
        slots = []
        for slot in target.get('mesh_material_slots', []):
            textures = {}
            for texture in slot['textures']:
                file = npc.find_output(name, npc.leaf(texture['object']), ['png', 'tga'])
                if file:
                    textures[texture['parameter']] = str(file)
            slots.append({'slot': slot['name'], 'material': slot['material'], 'textures': textures,
                          'scalars': slot.get('scalars', {}), 'vectors': slot.get('vectors', {})})
        job['npcs'].append({'id': name, 'dest': target['dest'], 'mesh_name': mesh,
                            'gltf': str(npc.find_output(name, mesh, ['gltf'])), 'slots': slots,
                            'attachment_meshes': [{'id': m, 'gltf': str(npc.find_output(name, m, ['gltf'])), 'material': material}
                                                   for m, material in attachment_meshes(target['attachments']).items()],
                            'tracks': str(OUT / f'anim_tracks_{name}.json'), 'roles': [r for r, _ in TARGETS[name]['roles']],
                            'reference_out': str(OUT / f'ref_pose_{name}.json')})
        hosts[name] = host_target(name, target)
    npc.write_json(OUT / 'editor_job.json', job)
    npc.write_json(OUT / 'host_target.json', hosts)
    print(f'editor-job: wrote {OUT / "editor_job.json"} and host_target.json')


def anims_step(args):
    identity = npc.read_json(OUT / 'identity.json')
    report = {'tool': TOOL, 'generated': npc.now(), 'targets': {}}
    for name, target in identity['targets'].items():
        reference = OUT / f'ref_pose_{name}.json'
        if not reference.is_file():
            sys.exit(f'anims: {reference} missing (run the editor npcs mode first)')
        mesh_md5 = npc.find_output(name, npc.leaf(target['skeletal_mesh']['path']), ['md5mesh'])
        clips = {role: clip_file(name, target['anim_sets'][0]['path'], clip) for role, clip in TARGETS[name]['roles']}
        tracks, details = npc.convert_subset(mesh_md5, reference, clips)
        npc.write_json(OUT / f'anim_tracks_{name}.json', tracks)
        report['targets'][name] = details
        print(f'anims: {name}: {len(clips)} clips converted (fit error {details["fit_error"]:.2e}); '
              + ', '.join(f'{r} {d["frames"]}f@{d["rate"]:g}' for r, d in details.items() if r != 'fit_error'))
    npc.write_json(OUT / 'anims.json', report)


def host_attachments(worn, assets):
    """What the host attaches: the asset, the bone and the component's own transform on that bone. The live Adult's bone matrices fit
    the imported bone frames with no flip (2026-11-06, one cast, approximate pose), so the citizens' correction is not used here.
    UNVERIFIED: the composition of a component on its bone (LocalToWorld could not be read) and the glTF axes through the import."""
    import numpy as np
    from prepare_ambient_world import component_transform
    out = []
    for att in worn:
        location, quat, scale = component_transform(np.eye(4), att)
        out.append({'mesh': assets[npc.leaf(att['static_mesh'])], 'bone': att['bone'], 'location': location, 'quat': quat, 'scale': scale})
    return out


def attachments_step(args):
    identity = npc.read_json(OUT / 'identity.json')
    report = npc.read_json(OUT / 'editor_report_attach.json')
    hosts = npc.read_json(OUT / 'host_target.json')
    for name, target in identity['targets'].items():
        worn = target['attachments']
        check_worn(name, worn, TARGETS[name]['attachments'])
        hosts[name]['attachments'] = host_attachments(worn, report[name])
        print(f'attachments: {name}: {len(worn)} pieces')
    npc.write_json(OUT / 'host_target.json', hosts)


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('step', choices=['identity', 'extract', 'editor-job', 'anims', 'attachments', 'all'])
    parser.add_argument('--game', help='Borderlands 2 folder (or OPENWILLOW_BL2)')
    parser.add_argument('--umodel', help='umodel.exe, build 1590 (or OPENWILLOW_UMODEL)')
    parser.add_argument('--reader', default=str(ROOT / 'build/Release/ow-package.exe'))
    args = parser.parse_args()
    use_own_folders()
    steps = {'identity': [identity_step], 'extract': [extract_step], 'editor-job': [editor_job_step],
             'anims': [anims_step], 'attachments': [attachments_step], 'all': [identity_step, extract_step, editor_job_step]}[args.step]
    for step in steps:
        step(args)


if __name__ == '__main__':
    main()
