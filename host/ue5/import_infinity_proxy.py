"""Import a local UModel pistol gestalt as a clearly labeled Infinity visual proxy.

OPENWILLOW_PISTOL_GLTF is a glTF exported from the player's Startup.upk.
The shared gestalt contains multiple pistol parts; this script does not claim
to reconstruct the Infinity part list or Gearbox's cooked Master_Gun shader.
Only generated UE assets under /Game/OpenWillow/Weapons/InfinityProxy are touched.
"""
import os
from pathlib import Path
import unreal

source = Path(os.environ['OPENWILLOW_PISTOL_GLTF']).resolve()
if not source.is_file() or source.suffix.lower() != '.gltf':
    raise RuntimeError(f'Expected a local UModel glTF: {source}')
destination = '/Game/OpenWillow/Weapons/InfinityProxy'
tools = unreal.AssetToolsHelpers.get_asset_tools()
eal = unreal.EditorAssetLibrary
mel = unreal.MaterialEditingLibrary
if eal.does_directory_exist(destination):
    eal.delete_directory(destination)

task = unreal.AssetImportTask()
task.filename = str(source)
task.destination_path = destination
task.automated = True
task.replace_existing = True
task.save = False
tools.import_asset_tasks([task])
meshes = [asset for asset in task.get_objects() if isinstance(asset, unreal.SkeletalMesh)]
if len(meshes) != 1:
    raise RuntimeError(f'Expected one UModel pistol skeletal mesh, found {len(meshes)}: {task.get_objects()}')
mesh = meshes[0]

# This deliberate neutral material keeps the proxy distinguishable from a
# verified reconstruction of Mati_VladofLegendaryPistol_Infinity.
material = tools.create_asset('M_OW_PistolProxy', destination, unreal.Material, unreal.MaterialFactoryNew())
color = mel.create_material_expression(material, unreal.MaterialExpressionConstant3Vector, -200, 0)
color.set_editor_property('constant', unreal.LinearColor(0.32, 0.30, 0.42, 1.0))
mel.connect_material_property(color, '', unreal.MaterialProperty.MP_BASE_COLOR)
material.set_editor_property('used_with_skeletal_mesh', True)
mel.recompile_material(material)

slots = mesh.get_editor_property('materials')
for i in range(len(slots)):
    slot = slots[i]
    slot.set_editor_property('material_interface', material)
    slots[i] = slot
mesh.modify()
mesh.set_editor_property('materials', slots)
old_path = mesh.get_path_name()
new_path = f'{destination}/SK_InfinityProxy'
if old_path != new_path and not eal.rename_asset(old_path, new_path):
    raise RuntimeError(f'Could not rename {old_path} to {new_path}')
mesh = unreal.load_asset(new_path)
if not mesh or not eal.save_loaded_asset(mesh, only_if_is_dirty=False):
    raise RuntimeError(f'Could not save {new_path}')
eal.save_loaded_asset(material, only_if_is_dirty=False)
box = mesh.get_bounds().box_extent
unreal.log(f'OW_INFINITY_PROXY path={mesh.get_path_name()} source={source.name} '
           f'material_slots={len(slots)} extent={box.x:.1f},{box.y:.1f},{box.z:.1f}')
