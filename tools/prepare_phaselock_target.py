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
        'pawn': 'GD_PrimalBeast.Character.Pawn_PrimalBeast',
        'dest': '/Game/OpenWillow/Characters/PhaselockTarget/Bullymong',
        'roles': [('Idle', 'Idle_var1'), ('Death', 'Death_Fire_var1'),
                  ('PhaseLock_Lift', 'PhaseLock_Lift'), ('PhaseLock_Loop', 'PhaseLock_Loop'),
                  ('PhaseLock_Fall', 'PhaseLock_Fall'), ('PhaseLock_Land', 'PhaseLock_Land')],
        # Collision cylinder observed on the live adult bullymong (local/realgame/phaselock/size_rule/adult68, SDK reads
        # 2026-10-03): 150 x 90 (the archetype says 100 x 90). The same read gave Mesh.Scale 1.0, but at that scale the host's
        # bullymong is about 1.45x too large against the matched_650 frames (same distance and field of view, curled loop pose
        # and idle), while the archetype's component Scale 0.7 matches them; the host uses 0.7 and the conflict stays UNVERIFIED.
        'observed': {'collision_radius': 150.0, 'collision_height': 90.0,
                     'source': 'local/realgame/phaselock/size_rule/adult68 (SDK read)',
                     # UpdateLiftedPawnMeshOffset centres Mesh.Bounds (the stock physics asset, not imported) on the pawn's
                     # Location. Live (2026-10-21, local/realgame/phaselock/target_anim run2): Mesh.Translation.Z went from
                     # -102.1 at rest to -221.5 through the hold (-119.5); the host's bone-box centring moved it -60.8. The
                     # host therefore centres the bones' box this much below the Location (a fit for this pawn, UNVERIFIED).
                     'centre_bias': 58.7},
    },
}


def use_own_folders():
    """Point the shared pipeline's module-level folders at this tool's own output."""
    npc.SLICE, npc.UMODEL_OUT = OUT, UMODEL_OUT


def group_of(anim_set):
    """'Anim_PrimalBeast.Unarmed' -> ('Anim_PrimalBeast', 'Unarmed'): UModel -groups writes <package>/<group>/<set>/."""
    group, _, name = anim_set.rpartition('.')
    return group, name


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
        if len(entry['anim_sets']) != 1:
            sys.exit(f'{name}: expected one AnimSet on the mesh component, got {component.get("animsets")}')
        result['targets'][name] = entry
        print(f'identity: {name}: mesh {component["skeletalmesh"]}, set {component["animsets"][0]}, '
              f'translation {component.get("translation")}, scale {component.get("scale")}, cylinder {entry["archetype_cylinder"]}')
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
    scale = float(props.get('scale', 1.0))
    # Translation is in the pawn's frame (not scaled by the mesh's own Scale).
    z = observed['collision_height'] + float(props.get('translation', {}).get('Z', 0.0))
    anim = lambda role: f'{dest}/Animations/Anim_{name}_{role}.Anim_{name}_{role}'  # noqa: E731
    return {'id': name, 'mesh': f'{dest}/Meshes/{mesh}/SkeletalMeshes/{mesh}.{mesh}',
            'idle': anim('Idle'), 'death': anim('Death'), 'mesh_offset_z': z, 'mesh_scale': scale,
            'collision_radius': observed['collision_radius'], 'collision_height': observed['collision_height'],
            'centre_bias': observed.get('centre_bias', 0.0),
            'source': {'translation_and_scale': 'archetype mesh component (our reader)', 'cylinder': observed['source']}}


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


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('step', choices=['identity', 'extract', 'editor-job', 'anims', 'all'])
    parser.add_argument('--game', help='Borderlands 2 folder (or OPENWILLOW_BL2)')
    parser.add_argument('--umodel', help='umodel.exe, build 1590 (or OPENWILLOW_UMODEL)')
    parser.add_argument('--reader', default=str(ROOT / 'build/Release/ow-package.exe'))
    args = parser.parse_args()
    use_own_folders()
    steps = {'identity': [identity_step], 'extract': [extract_step], 'editor-job': [editor_job_step],
             'anims': [anims_step], 'all': [identity_step, extract_step, editor_job_step]}[args.step]
    for step in steps:
        step(args)


if __name__ == '__main__':
    main()
