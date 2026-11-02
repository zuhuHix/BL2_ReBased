"""Import each rolled weapon's filtered gestalt glTF as its own skeletal mesh.

OPENWILLOW_ITEMS is a local directory of tools/weapon_recipe.py recipes
(<id>.json) with a matching tools/filter_gestalt_gltf.py --recipe output
(<id>.gltf). Each becomes /Game/OpenWillow/Weapons/Items/SK_<id>.

Prepared per-recipe paint materials from import_weapon_paint.py are reused
when present. Otherwise the Infinity MIC gets M_OW_InfinityApprox; every other
material gets an explicitly warned neutral grey stand-in, never guessed paint.
"""
import json
import os
from pathlib import Path
import sys
import unreal

ROOT = Path(__file__).resolve().parents[2]
TOOLS = str(ROOT / 'tools')
if TOOLS not in sys.path:
    sys.path.insert(0, TOOLS)
from weapon_item_materials import INFINITY_MIC, choose_item_material, paint_material_path

destination = '/Game/OpenWillow/Weapons/Items'
items = Path(os.environ['OPENWILLOW_ITEMS']).resolve()
reference_dir = Path(os.environ['OPENWILLOW_WEAPON_REFERENCES']).resolve() if os.environ.get('OPENWILLOW_WEAPON_REFERENCES') else None
tools = unreal.AssetToolsHelpers.get_asset_tools()
eal = unreal.EditorAssetLibrary
mel = unreal.MaterialEditingLibrary
recipes = []
for recipe_path in sorted(items.glob('*.json')):
    gltf = recipe_path.with_suffix('.gltf')
    if gltf.is_file():
        recipes.append((recipe_path, gltf, json.loads(recipe_path.read_text(encoding='utf-8'))))


def save_reference_pose(mesh, recipe_id):
    if reference_dir is None:
        return
    reference_dir.mkdir(parents=True, exist_ok=True)
    skeleton = mesh.get_editor_property('skeleton')
    pose = unreal.AnimPoseExtensions.get_reference_pose(skeleton)
    bones = []
    for name in unreal.AnimPoseExtensions.get_bone_names(pose):
        transform = unreal.AnimPoseExtensions.get_bone_pose(pose, name, unreal.AnimPoseSpaces.WORLD)
        bones.append({'name': str(name),
                      'loc': [transform.translation.x, transform.translation.y, transform.translation.z],
                      'quat': [transform.rotation.x, transform.rotation.y, transform.rotation.z, transform.rotation.w]})
    target = reference_dir / f'{recipe_id}.json'
    target.write_text(json.dumps(bones, indent=1), encoding='utf-8')
    unreal.log(f'OW_WEAPON_REFERENCE {recipe_id}: {len(bones)} bones -> {target}')

infinity = unreal.load_asset('/Game/OpenWillow/Weapons/InfinityProxy/M_OW_InfinityApprox')
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
for recipe_path, gltf, recipe in recipes:
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
    paint_path = paint_material_path(recipe_path.stem)
    choice, warning = choose_item_material(recipe_path.stem, recipe.get('material'),
                                           paint_available=eal.does_asset_exist(paint_path),
                                           infinity_available=infinity is not None)
    if choice == 'paint':
        material = unreal.load_asset(paint_path)
        if material is None:
            raise RuntimeError(f'{recipe_path.stem}: prepared paint asset exists but failed to load: {paint_path}')
    elif choice == 'infinity':
        material = infinity
    else:
        material = neutral
        unreal.log_warning(f'OW_ITEM {recipe_path.stem}: {warning}; neutral stand-in')
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
    mesh = unreal.load_asset(target)
    if not eal.save_loaded_asset(mesh, only_if_is_dirty=False):
        raise RuntimeError(f'Could not save {target}')
    save_reference_pose(mesh, recipe_path.stem)
    imported += 1
    unreal.log(f"OW_ITEM {target} name={recipe.get('name')} fragments={len(recipe.get('gestalt_fragments', []))} "
               f"material={material.get_path_name()}")
# The glTF import also creates each mesh's Skeleton and PhysicsAsset under
# <id>/SkeletalMeshes; saving only the mesh left them unsaved, so a fresh
# editor could not load SK_<id>.
if not eal.save_directory(destination, only_if_is_dirty=False, recursive=True):
    raise RuntimeError(f'Could not save {destination}')
unreal.log(f'OW_ITEMS imported={imported} from {items}')
