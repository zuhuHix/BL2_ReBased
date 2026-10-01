"""UE5 editor-Python half of the slice NPC/weapon import (Marcus, target dummy, stock Maliwan pistol).

AI-assisted. Run through tools/seed_slice_npc_assets.ps1 (which owns the editor lock), never directly.
Reads the job file written by ``tools/slice_npc_assets.py editor-job`` (OPENWILLOW_SLICE_JOB) and runs one mode
(OPENWILLOW_SLICE_MODE):

  npcs     import each NPC's UModel glTF skeletal mesh, textures and materials, and dump the skeleton's reference
           pose (input of the MD5 clip converter)
  anims    create AnimSequences from the converted bone tracks (tools/slice_npc_assets.py anims)
  pistol   import the rolled-sample glTF and the one-primitive-per-candidate-fragment glTF of the Maliwan pistol
  preview  load everything back from disk in this fresh session, record bounds/bone counts/anim lengths, and
           build one small preview level per group (the PowerShell runner captures them in -game)

Writes ``local/slice/editor_report_<mode>.json``. Only new content under /Game/OpenWillow/Characters/Marcus,
/Game/OpenWillow/Characters/TargetDummy, /Game/OpenWillow/Characters/Shared and /Game/OpenWillow/Weapons/MaliwanPistol
is created or replaced; nothing else (Maya, the existing weapon items) is touched.

Materials: UModel's material output is heuristic. These materials are the diffuse and normal textures UModel
bound, in a minimal material. Colour zones/masks/patterns of the original master materials are NOT reproduced
(the cooked master graphs are stripped); nothing here is a verified material graph.
"""
import json
import math
import os
from pathlib import Path
import unreal

job = json.loads(Path(os.environ['OPENWILLOW_SLICE_JOB']).read_text(encoding='utf-8'))
mode = os.environ['OPENWILLOW_SLICE_MODE']
slice_dir = Path(job['slice_dir'])
tools = unreal.AssetToolsHelpers.get_asset_tools()
eal = unreal.EditorAssetLibrary
mel = unreal.MaterialEditingLibrary
SHARED = '/Game/OpenWillow/Characters/Shared'
report = {}


def log(message):
    unreal.log(f'OW_SLICE {message}')


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
        raise RuntimeError(f'Expected one {expected.__name__} from {Path(path).name}: {task.get_objects()}')
    return found[0]


def texture(path, folder, parameter):
    asset = import_one(path, folder, unreal.Texture2D)
    kind = parameter.lower()
    asset.set_editor_property('srgb', kind.endswith('diffuse'))
    if kind.endswith('normal'):
        asset.set_editor_property('compression_settings', unreal.TextureCompressionSettings.TC_NORMALMAP)
    eal.save_loaded_asset(asset, only_if_is_dirty=False)
    return asset


def node(material, cls, x, y, **props):
    expression = mel.create_material_expression(material, cls, x, y)
    for key, value in props.items():
        expression.set_editor_property(key, value)
    return expression


def master_material(name, with_normal):
    path = f'{SHARED}/{name}'
    if eal.does_asset_exist(path):
        return unreal.load_asset(path)
    material = tools.create_asset(name, SHARED, unreal.Material, unreal.MaterialFactoryNew())
    diffuse = node(material, unreal.MaterialExpressionTextureSampleParameter2D, -600, 0, parameter_name='Diffuse',
                   texture=unreal.load_asset('/Engine/EngineResources/DefaultTexture'))
    mel.connect_material_property(diffuse, 'RGB', unreal.MaterialProperty.MP_BASE_COLOR)
    if with_normal:
        normal = node(material, unreal.MaterialExpressionTextureSampleParameter2D, -600, 300, parameter_name='Normal',
                      sampler_type=unreal.MaterialSamplerType.SAMPLERTYPE_NORMAL,
                      texture=unreal.load_asset('/Engine/EngineMaterials/DefaultNormal'))
        mel.connect_material_property(normal, 'RGB', unreal.MaterialProperty.MP_NORMAL)
    rough = node(material, unreal.MaterialExpressionConstant, -600, 600, r=0.7)
    mel.connect_material_property(rough, '', unreal.MaterialProperty.MP_ROUGHNESS)
    material.set_editor_property('used_with_skeletal_mesh', True)
    mel.recompile_material(material)
    eal.save_loaded_asset(material, only_if_is_dirty=False)
    return material


def instance(name, folder, parent, textures):
    asset = tools.create_asset(name, folder, unreal.MaterialInstanceConstant, unreal.MaterialInstanceConstantFactoryNew())
    mel.set_material_instance_parent(asset, parent)
    for parameter, tex in textures.items():
        mel.set_material_instance_texture_parameter_value(asset, parameter, tex)
    eal.save_loaded_asset(asset, only_if_is_dirty=False)
    return asset


def reference_pose(skeleton):
    pose = unreal.AnimPoseExtensions.get_reference_pose(skeleton)
    bones = []
    for name in unreal.AnimPoseExtensions.get_bone_names(pose):
        t = unreal.AnimPoseExtensions.get_bone_pose(pose, name, unreal.AnimPoseSpaces.WORLD)
        bones.append({'name': str(name), 'loc': [t.translation.x, t.translation.y, t.translation.z],
                      'quat': [t.rotation.x, t.rotation.y, t.rotation.z, t.rotation.w]})
    return bones


def describe_mesh(mesh):
    box = mesh.get_bounds()
    skeleton = mesh.get_editor_property('skeleton')
    slots = mesh.get_editor_property('materials')
    info = {'skeletal_mesh': mesh.get_path_name().split('.')[0],
            'skeleton': skeleton.get_path_name().split('.')[0] if skeleton else None,
            'physics_asset': (mesh.get_editor_property('physics_asset').get_path_name().split('.')[0]
                              if mesh.get_editor_property('physics_asset') else None),
            'bounds_extent': [round(box.box_extent.x, 2), round(box.box_extent.y, 2), round(box.box_extent.z, 2)],
            'bounds_origin': [round(box.origin.x, 2), round(box.origin.y, 2), round(box.origin.z, 2)],
            'material_slots': [str(s.get_editor_property('material_slot_name')) for s in slots],
            'materials': [s.get_editor_property('material_interface').get_path_name().split('.')[0]
                          if s.get_editor_property('material_interface') else None for s in slots]}
    if skeleton:
        info['bone_count'] = len(reference_pose(skeleton))
    try:
        subsystem = unreal.get_editor_subsystem(unreal.SkeletalMeshEditorSubsystem)
        info['lod0_vertices'] = int(subsystem.get_num_verts(mesh, 0))
        info['lod_count'] = int(subsystem.get_lod_count(mesh))
    except Exception as error:  # the subsystem API differs across engine versions; counts are optional evidence
        info['lod0_vertices'] = None
        info['vertex_count_note'] = f'not available: {error}'
    return info


def assign(mesh, mapping, fallback=None):
    """Set each material slot from {slot name: material}; unmatched slots are an error unless a fallback is given."""
    slots = mesh.get_editor_property('materials')
    unmatched = []
    for i in range(len(slots)):
        slot = slots[i]
        name = str(slot.get_editor_property('material_slot_name'))
        material = mapping.get(name, fallback)
        if material is None:
            unmatched.append(name)
            continue
        slot.set_editor_property('material_interface', material)
        slots[i] = slot
    if unmatched:
        raise RuntimeError(f'{mesh.get_name()}: slots without a material: {unmatched}')
    mesh.modify()
    mesh.set_editor_property('materials', slots)


def clean(folder):
    if eal.does_directory_exist(folder):
        eal.delete_directory(folder)


# ------------------------------------------------------------------------------------------- modes


def mode_npcs():
    parent = master_material('M_OW_NPC', True)
    for npc in job['npcs']:
        dest = npc['dest']
        clean(dest)
        entry = {'status': 'failed'}
        report[npc['id']] = entry
        mesh = import_one(npc['gltf'], f'{dest}/Meshes', unreal.SkeletalMesh)
        slot_names = [str(s.get_editor_property('material_slot_name')) for s in mesh.get_editor_property('materials')]
        log(f'{npc["id"]} imported slots={slot_names}')
        mapping, textures = {}, []
        for slot in npc['slots']:
            loaded = {}
            for parameter, file in slot['textures'].items():
                if parameter in ('p_Diffuse', 'p_Normal'):
                    asset = texture(file, f'{dest}/Textures', parameter)
                    loaded['Diffuse' if parameter == 'p_Diffuse' else 'Normal'] = asset
                    textures.append(asset.get_path_name().split('.')[0])
            mapping[slot['slot']] = instance(f'MI_{slot["material"]}', f'{dest}/Materials', parent, loaded)
        assign(mesh, mapping)
        if not eal.save_loaded_asset(mesh, only_if_is_dirty=False):
            raise RuntimeError(f'Could not save {npc["id"]} mesh')
        eal.save_directory(dest, only_if_is_dirty=False, recursive=True)
        pose = reference_pose(mesh.get_editor_property('skeleton'))
        Path(npc['reference_out']).write_text(json.dumps(pose, indent=1), encoding='utf-8')
        entry.update(describe_mesh(mesh))
        entry.update({'status': 'imported', 'textures': sorted(set(textures)),
                      'material_parameters_not_applied': sorted({k for s in npc['slots'] for k in
                                                                 list(s['scalars']) + list(s['vectors'])}),
                      'textures_not_imported': sorted({p for s in npc['slots'] for p in s['textures']
                                                       if p not in ('p_Diffuse', 'p_Normal')})})
        log(f'OW_NPC {npc["id"]} bones={entry["bone_count"]} extent={entry["bounds_extent"]} slots={slot_names}')


def mode_anims():
    for npc in job['npcs']:
        data = json.loads(Path(npc['tracks']).read_text(encoding='utf-8'))
        mesh = unreal.load_asset(f'{npc["dest"]}/Meshes/{npc["mesh_name"]}/SkeletalMeshes/{npc["mesh_name"]}')
        mesh = mesh or find_mesh(npc)
        skeleton = mesh.get_editor_property('skeleton')
        entry = {'assets': {}, 'details': {}}
        report[npc['id']] = entry
        folder = f'{npc["dest"]}/Animations'
        clean(folder)
        for role, clip in data.items():
            name = f'Anim_{npc["id"]}_{role}'
            factory = unreal.AnimSequenceFactory()
            factory.set_editor_property('target_skeleton', skeleton)
            factory.set_editor_property('preview_skeletal_mesh', mesh)
            sequence = tools.create_asset(name, folder, unreal.AnimSequence, factory)
            controller = sequence.controller
            controller.open_bracket(unreal.Text('OpenWillow slice MD5 import'))
            source_rate = float(clip['rate'])
            rate = int(round(source_rate))
            if abs(source_rate - rate) > 0.001:
                if abs(source_rate - 30.0) > 1.5:
                    raise RuntimeError(f'{name}: unsupported noninteger frame rate {source_rate}')
                rate = 30
                unreal.log_warning(f'OW_SLICE {name}: source {source_rate} fps imported at 30 fps')
            if rate != 30:
                controller.set_frame_rate(unreal.FrameRate(math.lcm(30, rate), 1))
            controller.set_frame_rate(unreal.FrameRate(rate, 1))
            controller.set_number_of_frames(unreal.FrameNumber(max(1, clip['frames'] - 1)))
            for bone, track in clip['tracks'].items():
                controller.add_bone_track(bone)
                positions = [unreal.Vector(*p) for p in track['pos']]
                rotations = [unreal.Quat(*q) for q in track['rot']]
                controller.set_bone_track_keys(bone, positions, rotations, [unreal.Vector(1, 1, 1)] * len(positions))
            controller.close_bracket()
            eal.save_loaded_asset(sequence, only_if_is_dirty=False)
            path = sequence.get_path_name().split('.')[0]
            entry['assets'][role] = path
            entry['details'][role] = {'frames': clip['frames'], 'rate_imported': rate, 'rate_source': source_rate,
                                      'tracks': len(clip['tracks']), 'length_seconds': round(sequence.get_play_length(), 3)}
            log(f'OW_ANIM {path} frames={clip["frames"]} tracks={len(clip["tracks"])}')
        eal.save_directory(npc['dest'], only_if_is_dirty=False, recursive=True)


def find_mesh(npc):
    for asset in eal.list_assets(f'{npc["dest"]}/Meshes', recursive=True, include_folder=False):
        loaded = unreal.load_asset(asset)
        if isinstance(loaded, unreal.SkeletalMesh):
            return loaded
    raise RuntimeError(f'No skeletal mesh under {npc["dest"]}/Meshes')


def mode_pistol():
    p = job.get('pistol')
    if not p:
        raise RuntimeError('No pistol section in the editor job')
    dest = p['dest']
    clean(dest)
    parent = master_material('M_OW_GunComp', False)
    entry = {'status': 'failed', 'meshes': {}}
    report['pistol'] = entry
    textures = {}
    for parameter, file in p['textures'].items():
        textures[parameter] = texture(file, f'{dest}/Textures', parameter)
    entry['textures'] = sorted(t.get_path_name().split('.')[0] for t in textures.values())
    # p_Diffuse is a composite (Comp) texture, used directly as base colour; masks/normal/emissive stay imported only.
    params = {'Diffuse': textures['p_Diffuse']} if 'p_Diffuse' in textures else {}
    material = instance(f'MI_{p["material"]}', f'{dest}/Materials', parent, params)
    entry['material'] = material.get_path_name().split('.')[0]
    for key, gltf, wanted_name in (('sample', p['sample_gltf'], 'SK_Pistol_Maliwan_2_Fire_seed1'),
                                   ('candidates', p['candidates_gltf'], 'SK_Pistol_Maliwan_Candidates')):
        mesh = import_one(gltf, f'{dest}/Meshes', unreal.SkeletalMesh)
        slots = mesh.get_editor_property('materials')
        mapping = {str(s.get_editor_property('material_slot_name')): material for s in slots}
        assign(mesh, mapping)
        target = f'{dest}/{wanted_name}'
        if mesh.get_path_name().split('.')[0] != target and not eal.rename_asset(mesh.get_path_name(), target):
            raise RuntimeError(f'Could not rename {mesh.get_path_name()} to {target}')
        mesh = unreal.load_asset(target)
        if not eal.save_loaded_asset(mesh, only_if_is_dirty=False):
            raise RuntimeError(f'Could not save {target}')
        entry['meshes'][key] = describe_mesh(mesh)
        entry['meshes'][key]['source_gltf'] = Path(gltf).name
    eal.save_directory(dest, only_if_is_dirty=False, recursive=True)
    entry['status'] = 'imported'
    entry['material_note'] = ('Mati_MaliwanUncommon p_Diffuse (a composite texture) used directly as base colour; '
                              'tint/pattern/mask/emissive logic is not reproduced')
    entry['material_parameters_not_applied'] = sorted(list(p['scalars']) + list(p['vectors']))
    log(f'OW_PISTOL imported {list(entry["meshes"])}')


def preview_level(path, actors, extent, origin):
    """A level holding the given SkeletalMeshActors, lights and a fixed front camera (same recipe as MayaPreview)."""
    # A previous run's .umap makes new_level fail even after delete_asset; remove the generated file itself.
    old_file = Path(unreal.Paths.project_content_dir()) / (path.replace('/Game/', '', 1) + '.umap')
    if old_file.is_file():
        old_file.unlink()
    if not unreal.EditorLevelLibrary.new_level(path):
        raise RuntimeError(f'Could not create {path}')
    for mesh, location, anim, label in actors:
        actor = unreal.EditorLevelLibrary.spawn_actor_from_class(unreal.SkeletalMeshActor, location)
        component = actor.skeletal_mesh_component
        component.set_skeletal_mesh_asset(mesh)
        if anim:
            component.set_animation_mode(unreal.AnimationMode.ANIMATION_SINGLE_NODE)
            component.set_animation(anim)
            # Persist 'playing, looping' in the saved component (the level is only ticked later in -game).
            data = component.get_editor_property('animation_data')
            data.set_editor_property('anim_to_play', anim)
            data.set_editor_property('saved_looping', True)
            data.set_editor_property('saved_playing', True)
            component.set_editor_property('animation_data', data)
        actor.set_actor_label(label)
    sun = unreal.EditorLevelLibrary.spawn_actor_from_class(unreal.DirectionalLight, unreal.Vector(0, 0, 300),
                                                           unreal.Rotator(0, -45, 135))
    sun.light_component.set_editor_property('intensity', 2.5)
    volume = unreal.EditorLevelLibrary.spawn_actor_from_class(unreal.PostProcessVolume, unreal.Vector(0, 0, 0))
    volume.set_editor_property('unbound', True)
    settings = volume.get_editor_property('settings')
    settings.set_editor_property('override_auto_exposure_method', True)
    settings.set_editor_property('auto_exposure_method', unreal.AutoExposureMethod.AEM_MANUAL)
    settings.set_editor_property('override_auto_exposure_apply_physical_camera_exposure', True)
    settings.set_editor_property('auto_exposure_apply_physical_camera_exposure', False)
    volume.set_editor_property('settings', settings)
    unreal.EditorLevelLibrary.spawn_actor_from_class(unreal.PlayerStart, unreal.Vector(900, 60, 100))
    unreal.EditorLevelLibrary.spawn_actor_from_class(unreal.SkyAtmosphere, unreal.Vector(0, 0, 0))
    sky = unreal.EditorLevelLibrary.spawn_actor_from_class(unreal.SkyLight, unreal.Vector(0, 0, 400))
    sky.light_component.set_editor_property('real_time_capture', True)
    distance = max(extent) * 3.2 + 60
    camera = unreal.EditorLevelLibrary.spawn_actor_from_class(
        unreal.CameraActor, unreal.Vector(origin[0] + distance, origin[1], origin[2]),
        unreal.Rotator(roll=0, pitch=0, yaw=180))
    camera.set_actor_label('OpenWillow_PreviewCamera')
    camera.set_editor_property('auto_activate_for_player', unreal.AutoReceiveInput.PLAYER0)
    unreal.EditorLevelLibrary.get_editor_world().get_world_settings().set_editor_property(
        'default_game_mode', unreal.GameModeBase)
    unreal.EditorLevelLibrary.save_current_level()


def load_back(path):
    asset = unreal.load_asset(path)
    if asset is None:
        raise RuntimeError(f'Cannot load {path} in a fresh session')
    return asset


def mode_preview():
    """Fresh-session load of every imported asset, then one preview level per group."""
    for npc in job['npcs']:
        saved = json.loads((slice_dir / 'editor_report_npcs.json').read_text(encoding='utf-8'))[npc['id']]
        anims = json.loads((slice_dir / 'editor_report_anims.json').read_text(encoding='utf-8')).get(npc['id'], {})
        mesh = load_back(saved['skeletal_mesh'])
        entry = describe_mesh(mesh)
        entry['loaded_in_fresh_session'] = True
        loaded_anims = {}
        for role, path in (anims.get('assets') or {}).items():
            sequence = load_back(path)
            loaded_anims[role] = {'path': path, 'length_seconds': round(sequence.get_play_length(), 3),
                                  'skeleton_matches_mesh': sequence.get_editor_property('skeleton') == mesh.get_editor_property('skeleton')}
        entry['anims_loaded'] = loaded_anims
        level = f'{npc["dest"]}/Preview_{npc["id"]}'
        idle = unreal.load_asset(anims['assets']['idle']) if (anims.get('assets') or {}).get('idle') else None
        # Dummy/NPC origin is at the feet; lift by half the bounds so the framing is centred.
        preview_level(level, [(mesh, unreal.Vector(0, 0, 0), idle, npc['id'])], entry['bounds_extent'],
                      [0, 0, entry['bounds_origin'][2]])
        entry['level'] = level
        # Per-role previews: the same level path with a different animation, for the captured stills.
        for role in ('walk',):
            if role in loaded_anims:
                lv = f'{npc["dest"]}/Preview_{npc["id"]}_{role}'
                preview_level(lv, [(mesh, unreal.Vector(0, 0, 0), unreal.load_asset(loaded_anims[role]['path']), npc['id'])],
                              entry['bounds_extent'], [0, 0, entry['bounds_origin'][2]])
                entry[f'level_{role}'] = lv
        report[npc['id']] = entry
        log(f'OW_PREVIEW {npc["id"]} bones={entry["bone_count"]} extent={entry["bounds_extent"]} anims={list(loaded_anims)}')
    pistol = job.get('pistol')
    if pistol:
        saved = json.loads((slice_dir / 'editor_report_pistol.json').read_text(encoding='utf-8'))['pistol']
        entry = {'meshes': {}}
        for key, info in saved['meshes'].items():
            mesh = load_back(info['skeletal_mesh'])
            entry['meshes'][key] = describe_mesh(mesh)
            entry['meshes'][key]['loaded_in_fresh_session'] = True
        sample = load_back(saved['meshes']['sample']['skeletal_mesh'])
        extent = entry['meshes']['sample']['bounds_extent']
        level = f'{pistol["dest"]}/Preview_Pistol'
        # The sample pistol and, beside it, the all-candidate mesh (candidate fragments overlap in space by design).
        preview_level(level, [(sample, unreal.Vector(0, 0, 0), None, 'SamplePistol')], extent,
                      entry['meshes']['sample']['bounds_origin'])
        entry['level'] = level
        report['pistol'] = entry
        log(f'OW_PREVIEW pistol {list(entry["meshes"])}')


{'npcs': mode_npcs, 'anims': mode_anims, 'pistol': mode_pistol, 'preview': mode_preview}[mode]()
out = slice_dir / f'editor_report_{mode}.json'
out.write_text(json.dumps(report, indent=1), encoding='utf-8')
log(f'{mode} complete -> {out}')
