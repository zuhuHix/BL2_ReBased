"""Import the slice gear's pool-rolled guns as their own skeletal meshes, without deleting anything.

OPENWILLOW_ITEMS is tools/weapon_slice_gear.py's local directory (local/items/slice): <id>.json recipes with
a matching <id>.gltf. Each pool-rolled recipe becomes /Game/OpenWillow/Weapons/SliceItems/SK_<id> (re-running
skips items whose SK_<id> already exists, so it only ever adds). The mission weapon is skipped: the host shows it with the imported Maliwan
pistol (Weapons/MaliwanPistol, tools/seed_slice_npc_assets.ps1). Unlike import_weapon_items.py this never
deletes anything, so Weapons/Items is untouched; to reimport an item, delete its SK_<id> in the editor first.

Every mesh gets the neutral grey stand-in M_OW_GunNeutral (created here if missing); paint is a separate,
optional pass (prepare_weapon_paint.py + import_weapon_paint.py) for MICs that have a local export.
"""
import json
import os
from pathlib import Path
import unreal

destination = '/Game/OpenWillow/Weapons/SliceItems'
items = Path(os.environ['OPENWILLOW_ITEMS']).resolve()
tools = unreal.AssetToolsHelpers.get_asset_tools()
eal = unreal.EditorAssetLibrary
mel = unreal.MaterialEditingLibrary

neutral_path = f'{destination}/M_OW_GunNeutral'
if eal.does_asset_exist(neutral_path):
    neutral = unreal.load_asset(neutral_path)
else:
    neutral = tools.create_asset('M_OW_GunNeutral', destination, unreal.Material, unreal.MaterialFactoryNew())
    grey = mel.create_material_expression(neutral, unreal.MaterialExpressionConstant3Vector, -200, 0)
    grey.set_editor_property('constant', unreal.LinearColor(0.25, 0.25, 0.27, 1.0))
    mel.connect_material_property(grey, '', unreal.MaterialProperty.MP_BASE_COLOR)
    neutral.set_editor_property('used_with_skeletal_mesh', True)
    mel.recompile_material(neutral)
    eal.save_loaded_asset(neutral, only_if_is_dirty=False)

imported = skipped = 0
for recipe_path in sorted(items.glob('*.json')):
    gltf = recipe_path.with_suffix('.gltf')
    recipe = json.loads(recipe_path.read_text(encoding='utf-8'))
    if not gltf.is_file() or 'stats' not in recipe:
        continue
    if (recipe.get('provenance') or {}).get('kind') == 'mission_weapon':
        skipped += 1
        unreal.log(f'OW_SLICE_ITEM {recipe_path.stem}: mission weapon, not imported here')
        continue
    target = f'{destination}/SK_{recipe_path.stem}'
    if eal.does_asset_exist(target):
        skipped += 1
        unreal.log(f'OW_SLICE_ITEM {target}: already imported, left as is')
        continue
    task = unreal.AssetImportTask()
    task.filename = str(gltf)
    task.destination_path = f'{destination}/{recipe_path.stem}'
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
    imported += 1
    unreal.log(f"OW_SLICE_ITEM {target} name={recipe.get('name')} material={recipe.get('material')} (neutral stand-in)")
# Skeletons and physics assets created next to each mesh must be saved too, or a fresh editor cannot load SK_<id>.
if not eal.save_directory(destination, only_if_is_dirty=False, recursive=True):
    raise RuntimeError(f'Could not save {destination}')
unreal.log(f'OW_SLICE_ITEMS imported={imported} skipped={skipped} from {items}')
