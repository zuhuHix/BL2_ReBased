"""Import converted per-recipe reload tracks onto the corresponding weapon skeletons.

OPENWILLOW_WEAPON_RELOAD_ANIMS is a semicolon-separated list of
`recipe_id=local-json-path` pairs written by tools/weapon_reload_anims.py.
With OPENWILLOW_WEAPON_RELOAD_TARGET=library the keys are weapon part library types (Pistol, SMG, ...) and the same
tracks go onto the library gestalt's skeleton, /Game/OpenWillow/Weapons/Library/<Type>/Anims (the per-recipe meshes are
cut from that gestalt, so bone names and bind pose are the same; OpenWillowWalker picks these when the gun is assembled).
"""
import json
import math
import os
from pathlib import Path
import unreal

destination = '/Game/OpenWillow/Weapons/Items'
tools = unreal.AssetToolsHelpers.get_asset_tools()
eal = unreal.EditorAssetLibrary
root = f'{destination}/WeaponAnims'


library = os.environ.get('OPENWILLOW_WEAPON_RELOAD_TARGET') == 'library'
if library:
    root = '/Game/OpenWillow/Weapons/Library'


def import_one(recipe_id, json_path):
    mesh = unreal.load_asset(f'{root}/{recipe_id}/SK_Gestalt_{recipe_id}' if library else f'{destination}/SK_{recipe_id}')
    if mesh is None:
        raise RuntimeError(f'{recipe_id}: weapon mesh is missing from {destination}')
    skeleton = mesh.get_editor_property('skeleton')
    if skeleton is None:
        raise RuntimeError(f'{recipe_id}: weapon mesh has no skeleton')
    animations = json.loads(Path(json_path).read_text(encoding='utf-8'))
    for clip, data in animations.items():
        name = f'Anim_WeaponReload_{clip}'
        folder = f'{root}/{recipe_id}/Anims' if library else f'{root}/{recipe_id}'
        asset_path = f'{folder}/{name}'
        if eal.does_asset_exist(asset_path) and not eal.delete_asset(asset_path):
            raise RuntimeError(f'{recipe_id}: could not replace {asset_path}')
        factory = unreal.AnimSequenceFactory()
        factory.set_editor_property('target_skeleton', skeleton)
        factory.set_editor_property('preview_skeletal_mesh', mesh)
        sequence = tools.create_asset(name, folder, unreal.AnimSequence, factory)
        controller = sequence.controller
        controller.open_bracket(unreal.Text('OpenWillow weapon reload import'))
        source_rate = float(data['rate'])
        rate = int(round(source_rate))
        if abs(source_rate - rate) > 0.001:
            if abs(source_rate - 30.0) > 1.5:
                raise RuntimeError(f'{name}: unsupported noninteger frame rate {source_rate}')
            rate = 30
            unreal.log_warning(f'OW_WEAPON_ANIM {name}: source {source_rate} fps imported at 30 fps')
        if rate != 30:
            controller.set_frame_rate(unreal.FrameRate(math.lcm(30, rate), 1))
        controller.set_frame_rate(unreal.FrameRate(rate, 1))
        controller.set_number_of_frames(unreal.FrameNumber(max(1, data['frames'] - 1)))
        for bone, track in data['tracks'].items():
            controller.add_bone_track(bone)
            positions = [unreal.Vector(*position) for position in track['pos']]
            rotations = [unreal.Quat(*rotation) for rotation in track['rot']]
            scales = [unreal.Vector(1, 1, 1)] * len(positions)
            controller.set_bone_track_keys(bone, positions, rotations, scales)
        controller.close_bracket()
        if not eal.save_loaded_asset(sequence, only_if_is_dirty=False):
            raise RuntimeError(f'{recipe_id}: could not save {asset_path}')
        unreal.log(f'OW_WEAPON_ANIM {asset_path} frames={data["frames"]} rate={rate}')


pairs = filter(None, os.environ.get('OPENWILLOW_WEAPON_RELOAD_ANIMS', '').split(';'))
count = 0
for pair in pairs:
    recipe_id, json_path = pair.split('=', 1)
    import_one(recipe_id, json_path)
    count += 1
if count == 0:
    raise RuntimeError('OPENWILLOW_WEAPON_RELOAD_ANIMS is empty')
if library and os.environ.get('OPENWILLOW_WEAPON_LIBRARY'):
    # Remember what went onto the library skeletons: a meshes re-import (import_weapon_library.py) deletes each type's
    # folder, Anims included, and tools/import_weapon_library.ps1 re-imports these clips afterwards from this record.
    record = Path(os.environ['OPENWILLOW_WEAPON_LIBRARY']) / 'reloads.json'
    known = json.loads(record.read_text(encoding='utf-8')) if record.exists() else {}
    repo = Path(__file__).resolve().parents[2]
    for pair in filter(None, os.environ.get('OPENWILLOW_WEAPON_RELOAD_ANIMS', '').split(';')):
        kind, json_path = pair.split('=', 1)
        # Repository-relative, so the record survives moving or re-provisioning the worktree (resolved again at import).
        try:
            known[kind] = Path(json_path).resolve().relative_to(repo).as_posix()
        except ValueError:
            known[kind] = json_path
    record.write_text(json.dumps(known, indent=1), encoding='utf-8')
if not eal.save_directory(root, only_if_is_dirty=False, recursive=True):
    raise RuntimeError(f'Could not save weapon animation folder {root}')
unreal.log(f'OW_WEAPON_ANIMS imported={count}')
