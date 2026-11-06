#!/usr/bin/env python3
"""Lootable, chest and pickup assets for the vendors lane (PR 3): plan, MD5 clip conversion, report, and the editor half.

AI-assisted. Clean-room: no game data lives in this file. Run through tools/import_lootables.ps1, which owns the UE run
lock and the UModel exports. Everything derived from the installed game stays under the ignored local/lootables/ and the
ignored UE Content folder /Game/OpenWillow/Lootables and /Game/OpenWillow/Pickups.

Plain Python (never launches Unreal or UModel):

  plan     read local/slice/interactives.json (tools/prepare_interactives.py) and write local/lootables/plan.json: the
           supported objects' skeletal meshes, anim sets (Closed_Idle, Open, Opened_Idle), pickup static meshes, the
           UModel export jobs, and the skipped objects.
  tracks   convert each anim set's MD5 clips to UE bone tracks with the fitted converter in tools/slice_npc_assets.py
           (convert_subset). Needs the reference poses written by the editor 'meshes' mode, and numpy.
  report   merge the plan, the UModel job records and the editor reports into local/lootables/import_report.json.

Editor half (run by UnrealEditor-Cmd with OPENWILLOW_LOOT_MODE=meshes|anims and OPENWILLOW_LOOT_PLAN=<plan.json>):

  meshes   import each skeletal mesh (glTF) into /Game/OpenWillow/Lootables/<Mesh>, dump its reference pose, and import
           each pickup static mesh into /Game/OpenWillow/Pickups/<Mesh>. Physics assets are not imported (optional,
           UNVERIFIED need). Materials: the imported meshes keep the importer's default slots (no textures bound).
  anims    create one AnimSequence per clip as /Game/OpenWillow/Lootables/Anims/<AnimSet>_<Clip> from the tracks.

Nothing here is verified in the running game: the rule for how a lootable's clips are played is UNVERIFIED.
"""
import argparse
import datetime
import json
import subprocess
import math
import os
import re
import sys
from pathlib import Path

try:
    import unreal  # only inside the UnrealEditor Python runtime
except ImportError:
    unreal = None

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'local/lootables'
INTERACTIVES = ROOT / 'local/slice/interactives.json'
SKEL_DEST = '/Game/OpenWillow/Lootables'
ANIM_DEST = '/Game/OpenWillow/Lootables/Anims'
PICKUP_DEST = '/Game/OpenWillow/Pickups'
CLIPS = ('Closed_Idle', 'Open', 'Opened_Idle')   # the three sequences every lootable anim set holds (package data)
PLAN_SCHEMA = 'openwillow-lootables-plan/1'


def safe(text):
    return re.sub(r'[^A-Za-z0-9_.-]', '_', text)


def object_name(path):
    """'Prop_Lootables.Meshes.Skel_X' -> 'Skel_X' (the last dotted part is the object name)."""
    return path.rsplit('.', 1)[-1]


def package_name(path):
    """'Prop_Lootables.Meshes.Skel_X' -> 'Prop_Lootables' (the part before the first dot is the package)."""
    return path.split('.')[0]


# ------------------------------------------------------------------------------------------------ plan


def build_plan(data, interactives_path):
    meshes, sets, pickups, skipped = {}, {}, {}, []
    for obj in data['objects']:
        if not obj.get('supported'):
            skipped.append({'id': obj['id'], 'reason': obj.get('unsupported_reason') or 'unsupported'})
            continue
        balance = data['balances'].get(obj['balance'])
        if not balance:
            skipped.append({'id': obj['id'], 'reason': 'balance missing from interactives.json'})
            continue
        components = (balance.get('visual') or {}).get('components') or []
        if not components:
            skipped.append({'id': obj['id'], 'reason': 'no skeletal component in the balance visual'})
            continue
        for comp in components:
            mesh = comp.get('skeletal_mesh')
            if not mesh:
                skipped.append({'id': obj['id'], 'reason': 'component without a skeletal mesh'})
                continue
            entry = meshes.setdefault(mesh, {'path': mesh, 'package': package_name(mesh), 'name': object_name(mesh),
                                             'objects': []})
            entry['objects'].append(obj['id'])
            for set_path in comp.get('anim_sets') or []:
                sequences = (comp.get('sequences') or {}).get(set_path) or []
                sets.setdefault(set_path, {
                    'path': set_path, 'package': package_name(set_path), 'object': object_name(set_path),
                    'id': safe(set_path), 'mesh_name': object_name(mesh), 'mesh_path': mesh,
                    'clips': [c for c in CLIPS if c in sequences],
                    'missing_clips': [c for c in CLIPS if c not in sequences]})
        for config in balance.get('configurations') or []:
            for attachment in config.get('attachments') or []:
                for item in attachment.get('items') or []:
                    leaf = data['items'].get(item['balance'])
                    pickup = leaf.get('pickup') if leaf else None
                    if pickup and pickup.get('mesh'):
                        pickups.setdefault(pickup['mesh'], {'path': pickup['mesh'], 'package': package_name(pickup['mesh']),
                                                            'name': object_name(pickup['mesh'])})
    md5_mesh_dir = OUT / 'umodel/mesh-md5'
    gltf_mesh_dir = OUT / 'umodel/mesh-gltf'
    anim_dir = OUT / 'umodel/anim'
    pickup_dir = OUT / 'umodel/pickup'
    exports = []
    for mesh in meshes.values():
        exports.append({'id': f'mesh.{mesh["name"]}.gltf', 'format': 'gltf', 'class': 'SkeletalMesh',
                        'package': mesh['package'], 'object': mesh['name'], 'out': str(gltf_mesh_dir / safe(mesh['name']))})
        exports.append({'id': f'mesh.{mesh["name"]}.md5', 'format': 'md5', 'class': 'SkeletalMesh',
                        'package': mesh['package'], 'object': mesh['name'], 'out': str(md5_mesh_dir / safe(mesh['name']))})
        mesh['gltf_out'] = str(gltf_mesh_dir / safe(mesh['name']))
        mesh['md5_out'] = str(md5_mesh_dir / safe(mesh['name']))
        mesh['dest'] = f'{SKEL_DEST}/{mesh["name"]}'
    for anim in sets.values():
        exports.append({'id': f'anim.{anim["id"]}', 'format': 'md5', 'class': 'AnimSet', 'package': anim['package'],
                        'object': anim['object'], 'out': str(anim_dir / anim['id'])})
        anim['out'] = str(anim_dir / anim['id'])
    for pickup in pickups.values():
        exports.append({'id': f'pickup.{pickup["name"]}.gltf', 'format': 'gltf', 'class': 'StaticMesh',
                        'package': pickup['package'], 'object': pickup['name'],
                        'out': str(pickup_dir / safe(pickup['name']))})
        pickup['gltf_out'] = str(pickup_dir / safe(pickup['name']))
        pickup['dest'] = f'{PICKUP_DEST}/{pickup["name"]}'
    return {
        'schema': PLAN_SCHEMA,
        'source': str(interactives_path),
        'root': str(ROOT),
        'out_dir': str(OUT),
        'note': 'Package data only. Which clips and assets a lootable uses in game is UNVERIFIED until a live read.',
        'skeletal_meshes': sorted(meshes.values(), key=lambda m: m['name']),
        'anim_sets': sorted(sets.values(), key=lambda s: s['id']),
        'pickup_meshes': sorted(pickups.values(), key=lambda p: p['name']),
        'exports': exports,
        'skipped_objects': skipped,
    }


def resolve_cooked_packages(plan, data, reader, cooked):
    """The .upk file that holds each export job's object, as (job id -> package). The name before an object's first dot
    is its in-package group, not a file: these objects are cooked into the map packages and Startup, and UModel needs
    the file. Looks in the levels the placed objects come from, then Startup. False names the jobs not found."""
    levels = sorted({obj['id'].split(':', 1)[0] for obj in data.get('objects') or []})
    wanted = {(job['object'], job['class']) for job in plan['exports']}
    found = {}
    for level in levels + ['Startup']:
        listing = subprocess.run([reader, str(Path(cooked) / f'{level}.upk'), '--exports'], capture_output=True, text=True,
                                 encoding='utf-8')
        if listing.returncode:
            raise SystemExit(f'ow-package --exports failed for {level}: {listing.stderr.strip()[:200]}')
        for export in json.loads(listing.stdout):
            key = (export['name'], export['class'].rsplit('.', 1)[-1])
            if key in wanted and key not in found:
                found[key] = level
    missing = []
    for job in plan['exports']:
        level = found.get((job['object'], job['class']))
        if level is None:
            missing.append(job['id'])
        else:
            job['package'] = level
    return missing


def cmd_plan(args):
    data = json.loads(Path(args.interactives).read_text(encoding='utf-8-sig'))
    plan = build_plan(data, args.interactives)
    if args.cooked:
        missing = resolve_cooked_packages(plan, data, args.reader, args.cooked)
        if missing:
            raise SystemExit(f'no cooked package holds: {", ".join(missing)}')
    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(plan, indent=1), encoding='utf-8')
    print(f'plan: {len(plan["skeletal_meshes"])} skeletal meshes, {len(plan["anim_sets"])} anim sets, '
          f'{len(plan["pickup_meshes"])} pickup meshes, {len(plan["exports"])} UModel jobs, '
          f'{len(plan["skipped_objects"])} skipped objects')
    print(f'plan written: {out}')


# ------------------------------------------------------------------------------------------------ tracks


def find_one(folder, file_name):
    folder = Path(folder)
    if not folder.is_dir():
        return None
    hits = sorted(folder.rglob(file_name))
    return hits[0] if hits else None


def cmd_tracks(args):
    sys.path.insert(0, str(ROOT / 'tools'))
    import slice_npc_assets as npc   # the fitted MD5 -> UE converter (numpy); imports no Unreal module
    plan = json.loads(Path(args.plan).read_text(encoding='utf-8-sig'))
    meshes = {m['name']: m for m in plan['skeletal_meshes']}
    out = Path(plan['out_dir'])
    report = {}
    for anim in plan['anim_sets']:
        entry = {'status': 'failed', 'reason': '', 'clips': {}}
        report[anim['id']] = entry
        mesh = meshes.get(anim['mesh_name'])
        if mesh is None:
            entry['reason'] = 'skeletal mesh not in the plan'
            continue
        md5mesh = find_one(mesh['md5_out'], f'{mesh["name"]}.md5mesh')
        reference = out / 'ref_pose' / f'{mesh["name"]}.json'
        if md5mesh is None:
            entry['reason'] = 'MD5 mesh not exported (UModel)'
            continue
        if not reference.is_file():
            entry['reason'] = 'reference pose missing (editor meshes step did not import the mesh)'
            continue
        clips = {}
        for clip in anim['clips']:
            found = find_one(anim['out'], f'{clip}.md5anim')
            if found is None:
                entry['clips'][clip] = {'exported': False}
            else:
                clips[clip] = str(found)
                entry['clips'][clip] = {'exported': True}
        if not clips:
            entry['reason'] = 'no MD5 clip exported for this anim set'
            continue
        try:
            tracks, details = npc.convert_subset_rank_deficient(md5mesh, reference, clips)
        except ValueError as error:
            entry['reason'] = f'conversion refused: {error}'
            continue
        target = out / 'tracks' / f'{anim["id"]}.json'
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(json.dumps(tracks, indent=1), encoding='utf-8')
        entry.update(status='converted', fit_error=details['fit_error'], rank=details.get('rank', 4),
                     mirror_tested_directly=details.get('mirror_tested_directly', False),
                     frames={label: details[label]['frames'] for label in clips})
        print(f'tracks: {anim["object"]} {len(clips)} clips, fit error {details["fit_error"]:.2e}')
    (out / 'tracks_report.json').write_text(json.dumps(report, indent=1), encoding='utf-8')
    failed = [k for k, v in report.items() if v['status'] != 'converted']
    print(f'tracks: {len(report) - len(failed)} converted, {len(failed)} not converted')


# ------------------------------------------------------------------------------------------------ report


def load_optional(path):
    path = Path(path)
    return json.loads(path.read_text(encoding='utf-8-sig')) if path.is_file() else None


def export_status(jobs, job_id):
    record = (jobs or {}).get(job_id)
    if record is None:
        return 'not run'
    return 'exported' if record['exit_code'] == 0 and record['files'] > 0 else f'UModel exit {record["exit_code"]}, files {record["files"]}'


def cmd_report(args):
    plan = json.loads(Path(args.plan).read_text(encoding='utf-8-sig'))
    out = Path(plan['out_dir'])
    jobs_list = load_optional(args.umodel_jobs) or []
    if isinstance(jobs_list, dict):   # ConvertTo-Json writes a one-element list as a bare object
        jobs_list = [jobs_list]
    jobs = {job['id']: job for job in jobs_list}
    editor_meshes = load_optional(out / 'editor_meshes.json') or {}
    editor_anims = load_optional(out / 'editor_anims.json') or {}
    tracks = load_optional(out / 'tracks_report.json') or {}
    rows = []

    def row(asset, kind, imported, reason, export):
        rows.append({'asset': asset, 'kind': kind, 'imported': bool(imported), 'reason': reason, 'export': export})

    for mesh in plan['skeletal_meshes']:
        status = (editor_meshes.get('meshes') or {}).get(mesh['name'], {})
        exports = f'gltf {export_status(jobs, "mesh." + mesh["name"] + ".gltf")}; md5 {export_status(jobs, "mesh." + mesh["name"] + ".md5")}'
        row(f'{SKEL_DEST}/{mesh["name"]}', 'skeletal_mesh', status.get('imported'),
            status.get('reason') or ('' if status.get('imported') else 'no editor result'), exports)
    for anim in plan['anim_sets']:
        clip_status = (editor_anims.get('anims') or {}).get(anim['id'], {}).get('clips', {})
        track = tracks.get(anim['id'], {})
        for clip in anim['clips']:
            result = clip_status.get(clip, {})
            reason = result.get('reason') or track.get('reason') or ('' if result.get('imported') else 'no editor result')
            row(f'{ANIM_DEST}/{anim["object"]}_{clip}', 'anim_sequence', result.get('imported'), reason,
                export_status(jobs, f'anim.{anim["id"]}'))
        for clip in anim['missing_clips']:
            row(f'{ANIM_DEST}/{anim["object"]}_{clip}', 'anim_sequence', False,
                'clip not in the package data for this anim set', 'not run')
    for pickup in plan['pickup_meshes']:
        status = (editor_meshes.get('pickups') or {}).get(pickup['name'], {})
        row(f'{PICKUP_DEST}/{pickup["name"]}', 'pickup_mesh', status.get('imported'),
            status.get('reason') or ('' if status.get('imported') else 'no editor result'),
            export_status(jobs, f'pickup.{pickup["name"]}.gltf'))
    counts = {'yes': sum(1 for r in rows if r['imported']), 'no': sum(1 for r in rows if not r['imported'])}
    report = {'schema': 'openwillow-lootables-import/1', 'generated': datetime.datetime.now().isoformat(timespec='seconds'),
              'imported_yes': counts['yes'], 'imported_no': counts['no'], 'assets': rows,
              'skipped_objects': plan['skipped_objects'],
              'verification': 'Import inventory only. Whether the game plays these assets correctly is UNVERIFIED.'}
    target = Path(args.out)
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(json.dumps(report, indent=1), encoding='utf-8')
    print(f'report: {counts["yes"]} imported, {counts["no"]} not imported, {len(plan["skipped_objects"])} skipped objects')
    print(f'report written: {target}')


# ------------------------------------------------------------------------------------------------ editor


def log(message):
    unreal.log(f'OW_LOOT {message}')


def run_editor(mode):
    global tools, eal
    if unreal is None:
        raise SystemExit('OPENWILLOW_LOOT_MODE is set but the unreal module is not available')
    tools = unreal.AssetToolsHelpers.get_asset_tools()
    eal = unreal.EditorAssetLibrary
    plan = json.loads(Path(os.environ['OPENWILLOW_LOOT_PLAN']).read_text(encoding='utf-8-sig'))
    if mode == 'meshes':
        result = editor_meshes(plan)
    elif mode == 'anims':
        result = editor_anims(plan)
    else:
        raise SystemExit(f'unknown OPENWILLOW_LOOT_MODE {mode}')
    target = Path(plan['out_dir']) / {'meshes': 'editor_meshes.json', 'anims': 'editor_anims.json'}[mode]
    target.write_text(json.dumps(result, indent=1), encoding='utf-8')
    log(f'{mode} report {target}')


def clean(folder):
    if eal.does_directory_exist(folder):
        eal.delete_directory(folder)


def import_one(path, folder, expected):
    task = unreal.AssetImportTask()
    task.filename = str(path)
    task.destination_path = folder
    task.automated = True
    task.replace_existing = True
    task.save = False
    tools.import_asset_tasks([task])
    found = [o for o in task.get_objects() if isinstance(o, expected)]
    if len(found) != 1:
        raise RuntimeError(f'expected one {expected.__name__} from {Path(path).name}, got {task.get_objects()}')
    return found[0]


def find_export(folder, file_name):
    hits = sorted(Path(folder).rglob(file_name)) if Path(folder).is_dir() else []
    return hits[0] if hits else None


def reference_pose(skeleton):
    pose = unreal.AnimPoseExtensions.get_reference_pose(skeleton)
    bones = []
    for name in unreal.AnimPoseExtensions.get_bone_names(pose):
        t = unreal.AnimPoseExtensions.get_bone_pose(pose, name, unreal.AnimPoseSpaces.WORLD)
        bones.append({'name': str(name), 'loc': [t.translation.x, t.translation.y, t.translation.z],
                      'quat': [t.rotation.x, t.rotation.y, t.rotation.z, t.rotation.w]})
    return bones


def editor_meshes(plan):
    out = Path(plan['out_dir'])
    report = {'mode': 'meshes', 'meshes': {}, 'pickups': {}, 'mesh_assets': {}}
    for mesh in plan['skeletal_meshes']:
        entry = {'imported': False, 'reason': ''}
        report['meshes'][mesh['name']] = entry
        source = find_export(mesh['gltf_out'], f'{mesh["name"]}.gltf')
        if source is None:
            entry['reason'] = 'glTF not exported (UModel)'
            continue
        try:
            clean(mesh['dest'])
            asset = import_one(source, mesh['dest'], unreal.SkeletalMesh)
            skeleton = asset.get_editor_property('skeleton')
            if skeleton is None:
                raise RuntimeError('imported mesh has no skeleton')
            eal.save_loaded_asset(asset, only_if_is_dirty=False)
            path = asset.get_path_name().split('.')[0]
            pose = reference_pose(skeleton)
            (out / 'ref_pose').mkdir(parents=True, exist_ok=True)
            (out / 'ref_pose' / f'{mesh["name"]}.json').write_text(json.dumps(pose, indent=1), encoding='utf-8')
            report['mesh_assets'][mesh['name']] = path
            entry.update(imported=True, asset=path, bones=len(pose))
            log(f'OW_LOOT mesh {path} bones={len(pose)}')
        except Exception as error:   # recorded per asset; the rest of the list still runs
            entry['reason'] = f'{type(error).__name__}: {error}'
            log(f'OW_LOOT mesh {mesh["name"]} failed: {entry["reason"]}')
    for pickup in plan['pickup_meshes']:
        entry = {'imported': False, 'reason': ''}
        report['pickups'][pickup['name']] = entry
        source = find_export(pickup['gltf_out'], f'{pickup["name"]}.gltf')
        if source is None:
            entry['reason'] = 'glTF not exported (UModel)'
            continue
        try:
            clean(pickup['dest'])
            asset = import_one(source, pickup['dest'], unreal.StaticMesh)
            eal.save_loaded_asset(asset, only_if_is_dirty=False)
            entry.update(imported=True, asset=asset.get_path_name().split('.')[0])
        except Exception as error:
            entry['reason'] = f'{type(error).__name__}: {error}'
    eal.save_directory(SKEL_DEST, only_if_is_dirty=False, recursive=True)
    eal.save_directory(PICKUP_DEST, only_if_is_dirty=False, recursive=True)
    return report


def make_sequence(name, skeleton, mesh, clip):
    factory = unreal.AnimSequenceFactory()
    factory.set_editor_property('target_skeleton', skeleton)
    factory.set_editor_property('preview_skeletal_mesh', mesh)
    sequence = tools.create_asset(name, ANIM_DEST, unreal.AnimSequence, factory)
    controller = sequence.controller
    controller.open_bracket(unreal.Text('OpenWillow lootable MD5 import'))
    source_rate = float(clip['rate'])
    rate = int(round(source_rate))
    if rate != 30:
        # The controller accepts only rates that are factors or multiples of 30 fps: go through their common multiple.
        controller.set_frame_rate(unreal.FrameRate(math.lcm(30, rate), 1))
    controller.set_frame_rate(unreal.FrameRate(rate, 1))
    controller.set_number_of_frames(unreal.FrameNumber(max(1, clip['frames'] - 1)))
    for bone, track in clip['tracks'].items():
        controller.add_bone_track(bone)
        positions = [unreal.Vector(*p) for p in track['pos']]
        rotations = [unreal.Quat(*q) for q in track['rot']]
        controller.set_bone_track_keys(bone, positions, rotations, [unreal.Vector(1, 1, 1)] * len(positions))
    controller.close_bracket()
    return sequence, rate, source_rate


def editor_anims(plan):
    out = Path(plan['out_dir'])
    mesh_assets = (load_optional(out / 'editor_meshes.json') or {}).get('mesh_assets', {})
    report = {'mode': 'anims', 'anims': {}}
    for anim in plan['anim_sets']:
        entry = {'reason': '', 'clips': {}}
        report['anims'][anim['id']] = entry
        tracks_file = out / 'tracks' / f'{anim["id"]}.json'
        if not tracks_file.is_file():
            entry['reason'] = 'no tracks (tracks step did not convert this set)'
            continue
        mesh_path = mesh_assets.get(anim['mesh_name'])
        if not mesh_path:
            entry['reason'] = 'skeletal mesh not imported'
            continue
        mesh = unreal.load_asset(mesh_path)
        skeleton = mesh.get_editor_property('skeleton')
        data = json.loads(tracks_file.read_text(encoding='utf-8-sig'))
        for clip_name, clip in data.items():
            name = f'{anim["object"]}_{clip_name}'
            path = f'{ANIM_DEST}/{name}'
            try:
                if eal.does_asset_exist(path):
                    eal.delete_asset(path)
                sequence, rate, source_rate = make_sequence(name, skeleton, mesh, clip)
                eal.save_loaded_asset(sequence, only_if_is_dirty=False)
                note = '' if rate == source_rate else f'source rate {source_rate} imported as {rate}'
                entry['clips'][clip_name] = {'imported': True, 'asset': path, 'frames': clip['frames'],
                                             'tracks': len(clip['tracks']), 'note': note}
                log(f'OW_LOOT anim {path} frames={clip["frames"]} tracks={len(clip["tracks"])}')
            except Exception as error:
                entry['clips'][clip_name] = {'imported': False, 'reason': f'{type(error).__name__}: {error}'}
                log(f'OW_LOOT anim {name} failed: {error}')
    return report


# ------------------------------------------------------------------------------------------------ main


def main(argv):
    parser = argparse.ArgumentParser(description='Lootable asset plan, track conversion and report (see the module doc).')
    sub = parser.add_subparsers(dest='command', required=True)
    p = sub.add_parser('plan', help='write plan.json from interactives.json (no Unreal, no UModel)')
    p.add_argument('--interactives', default=str(INTERACTIVES))
    p.add_argument('--out', default=str(OUT / 'plan.json'))
    p.add_argument('--cooked', help='CookedPCConsole folder; without it the jobs keep the group name (dry run only)')
    p.add_argument('--reader', default=str(ROOT / 'build/Release/ow-package.exe'))
    p.set_defaults(func=cmd_plan)
    t = sub.add_parser('tracks', help='convert MD5 clips to UE bone tracks')
    t.add_argument('--plan', default=str(OUT / 'plan.json'))
    t.set_defaults(func=cmd_tracks)
    r = sub.add_parser('report', help='write import_report.json')
    r.add_argument('--plan', default=str(OUT / 'plan.json'))
    r.add_argument('--umodel-jobs', default=str(OUT / 'umodel_jobs.json'))
    r.add_argument('--out', default=str(OUT / 'import_report.json'))
    r.set_defaults(func=cmd_report)
    args = parser.parse_args(argv)
    args.func(args)


# Editor first: UnrealEditor-Cmd sets OPENWILLOW_LOOT_MODE; nothing else does.
if os.environ.get('OPENWILLOW_LOOT_MODE'):
    run_editor(os.environ['OPENWILLOW_LOOT_MODE'])
elif __name__ == '__main__':
    main(sys.argv[1:])
