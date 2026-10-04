"""Re-import named rolled guns' meshes, touching only those assets (lane C; AI-assisted, 2026-10-04).

OPENWILLOW_ITEMS   folder of tools/weapon_recipe.py recipes with their <id>.gltf (tools/weapon_refresh_fragments.py output)
OPENWILLOW_GUN_IDS comma-separated recipe ids, e.g. infinity_3,slice_pistol,slice_mission_pistol_fire

Why a new script: import_weapon_items.py deletes the whole Weapons/Items folder and import_slice_items.py skips meshes that
exist, and the mission pistol is not imported by it at all. This one replaces exactly the SK_<id> asset and the <id> subfolder
(skeleton, physics asset) of each named id, so other lanes' assets stay. 'infinity_*' ids go to
/Game/OpenWillow/Weapons/Items, every other id to /Game/OpenWillow/Weapons/SliceItems (the folders
UOpenWillowInventory::LoadWeaponMesh looks in). Every mesh gets the neutral stand-in material; run
host/ue5/import_weapon_paint.py afterwards for the paint.
"""
import json
import os
import re
from pathlib import Path
import unreal

items = Path(os.environ['OPENWILLOW_ITEMS']).resolve()
ids = [i for i in os.environ['OPENWILLOW_GUN_IDS'].split(',') if i]
tools = unreal.AssetToolsHelpers.get_asset_tools()
eal = unreal.EditorAssetLibrary

for recipe_id in ids:
    if not re.fullmatch(r'[A-Za-z0-9_]+', recipe_id):
        raise RuntimeError(f'Invalid recipe id {recipe_id}')
    folder = '/Game/OpenWillow/Weapons/' + ('Items' if recipe_id.startswith('infinity_') else 'SliceItems')
    gltf = items / f'{recipe_id}.gltf'
    if not gltf.is_file():
        raise RuntimeError(f'Missing {gltf}')
    recipe = json.loads((items / f'{recipe_id}.json').read_text(encoding='utf-8-sig'))
    neutral = unreal.load_asset(f'{folder}/M_OW_GunNeutral')
    if neutral is None:
        raise RuntimeError(f'{folder}/M_OW_GunNeutral is missing; run the item importer for that folder once')
    target = f'{folder}/SK_{recipe_id}'
    # Import into <folder>/<id>/ (the importer puts the skeleton and physics asset there) and rename the mesh to SK_<id>.
    # The caller (local/orch/C/run_assets.ps1) removes the old SK_<id>.uasset and <id> folder from disk first: deleting inside a
    # commandlet leaves the package file behind and the rename is then refused; importing over the old asset with the
    # skeleton in the shared folder garbled the skinning.
    if eal.does_asset_exist(target):
        raise RuntimeError(f'{target} still exists; remove SK_{recipe_id}.uasset and the {recipe_id} folder first')
    task = unreal.AssetImportTask()
    task.filename = str(gltf)
    task.destination_path = f'{folder}/{recipe_id}'
    task.automated = True
    task.replace_existing = True
    task.save = False
    tools.import_asset_tasks([task])
    meshes = [a for a in task.get_objects() if isinstance(a, unreal.SkeletalMesh)]
    if len(meshes) != 1:
        raise RuntimeError(f'{gltf.name}: expected one skeletal mesh, found {len(meshes)}')
    mesh = meshes[0]
    slots = mesh.get_editor_property('materials')
    for i in range(len(slots)):
        slot = slots[i]
        slot.set_editor_property('material_interface', neutral)
        slots[i] = slot
    mesh.modify()
    mesh.set_editor_property('materials', slots)
    if mesh.get_path_name().split('.')[0] != target and not eal.rename_asset(mesh.get_path_name(), target):
        raise RuntimeError(f'Could not rename {mesh.get_path_name()} to {target}')
    if not eal.save_loaded_asset(unreal.load_asset(target), only_if_is_dirty=False):
        raise RuntimeError(f'Could not save {target}')
    if not eal.save_directory(f'{folder}/{recipe_id}', only_if_is_dirty=False, recursive=True):
        raise RuntimeError(f'Could not save {folder}/{recipe_id}')
    unreal.log(f"OW_GUN_MESH {target} name={recipe.get('name')} fragments={recipe.get('gestalt_fragments')}")
unreal.log(f'OW_GUN_MESHES imported={len(ids)} from {items}')
