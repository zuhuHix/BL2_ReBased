"""Import each rolled weapon's filtered gestalt glTF as its own skeletal mesh.

OPENWILLOW_ITEMS is a local directory of tools/weapon_recipe.py recipes
(<id>.json) with a matching tools/filter_gestalt_gltf.py --recipe output
(<id>.gltf). Each becomes /Game/OpenWillow/Weapons/Items/SK_<id>.

Only one gun material is reconstructed so far (M_OW_InfinityApprox, from
import_infinity_proxy.py). Recipes using that MIC get it; every other
material gets a neutral grey stand-in and a warning, never a guessed paint.
"""
import json
import os
from pathlib import Path
import unreal

INFINITY_MIC = 'Common_GunMaterials.Materials.Pistol.Mati_VladofLegendaryPistol_Infinity'
destination = '/Game/OpenWillow/Weapons/Items'
items = Path(os.environ['OPENWILLOW_ITEMS']).resolve()
tools = unreal.AssetToolsHelpers.get_asset_tools()
eal = unreal.EditorAssetLibrary
mel = unreal.MaterialEditingLibrary

infinity = unreal.load_asset('/Game/OpenWillow/Weapons/InfinityProxy/M_OW_InfinityApprox')
if infinity is None:
    raise RuntimeError('Run import_infinity_proxy.py first; M_OW_InfinityApprox is missing')
if eal.does_directory_exist(destination):
    eal.delete_directory(destination)
neutral = tools.create_asset('M_OW_GunNeutral', destination, unreal.Material, unreal.MaterialFactoryNew())
grey = mel.create_material_expression(neutral, unreal.MaterialExpressionConstant3Vector, -200, 0)
grey.set_editor_property('constant', unreal.LinearColor(0.25, 0.25, 0.27, 1.0))
mel.connect_material_property(grey, '', unreal.MaterialProperty.MP_BASE_COLOR)
neutral.set_editor_property('used_with_skeletal_mesh', True)
mel.recompile_material(neutral)
eal.save_loaded_asset(neutral, only_if_is_dirty=False)

imported = 0
for recipe_path in sorted(items.glob('*.json')):
    gltf = recipe_path.with_suffix('.gltf')
    if not gltf.is_file():
        continue
    recipe = json.loads(recipe_path.read_text(encoding='utf-8'))
    task = unreal.AssetImportTask()
    task.filename = str(gltf)
    task.destination_path = destination
    task.automated = True
    task.replace_existing = True
    task.save = False
    tools.import_asset_tasks([task])
    meshes = [a for a in task.get_objects() if isinstance(a, unreal.SkeletalMesh)]
    if len(meshes) != 1:
        raise RuntimeError(f'{gltf.name}: expected one skeletal mesh, found {len(meshes)}')
    mesh = meshes[0]
    material = infinity if recipe.get('material') == INFINITY_MIC else neutral
    if material is neutral:
        unreal.log_warning(f"OW_ITEM {recipe_path.stem}: no reconstruction for {recipe.get('material')}; neutral stand-in")
    slots = mesh.get_editor_property('materials')
    for i in range(len(slots)):
        slot = slots[i]
        slot.set_editor_property('material_interface', material)
        slots[i] = slot
    mesh.modify()
    mesh.set_editor_property('materials', slots)
    target = f'{destination}/SK_{recipe_path.stem}'
    if mesh.get_path_name().split('.')[0] != target and not eal.rename_asset(mesh.get_path_name(), target):
        raise RuntimeError(f'Could not rename {mesh.get_path_name()} to {target}')
    if not eal.save_loaded_asset(unreal.load_asset(target), only_if_is_dirty=False):
        raise RuntimeError(f'Could not save {target}')
    imported += 1
    unreal.log(f"OW_ITEM {target} name={recipe.get('name')} fragments={len(recipe.get('gestalt_fragments', []))}")
unreal.log(f'OW_ITEMS imported={imported} from {items}')
