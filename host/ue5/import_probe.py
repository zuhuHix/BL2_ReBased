"""Run inside the UE5 editor with -ExecutePythonScript; generated assets stay local.

This is a Phase 0 editor import spike, not the eventual runtime UPK loader.
"""
import json
import os
from pathlib import Path
import unreal

root = Path(os.environ['OPENWILLOW_PROBE']).resolve()
game = Path(os.environ['OPENWILLOW_BL2']).resolve()
if not (game / 'Binaries/Win32/Borderlands2.exe').is_file():
    raise RuntimeError('An installed Borderlands 2 is required')
manifest = json.loads((root / 'probe.json').read_text(encoding='utf-8'))
tools = unreal.AssetToolsHelpers.get_asset_tools()

def imported(filename, expected):
    task = unreal.AssetImportTask()
    task.set_editor_property('filename', str(root / filename))
    task.set_editor_property('destination_path', '/Game/Phase0')
    task.set_editor_property('automated', True)
    task.set_editor_property('replace_existing', True)
    task.set_editor_property('save', True)
    if filename.endswith('.obj'):
        options = unreal.FbxImportUI()
        options.set_editor_property('import_mesh', True)
        options.set_editor_property('import_materials', False)
        options.set_editor_property('import_textures', False)
        options.set_editor_property('import_as_skeletal', False)
        options.set_editor_property('mesh_type_to_import', unreal.FBXImportType.FBXIT_STATIC_MESH)
        task.set_editor_property('options', options)
    tools.import_asset_tasks([task])
    objects = [o for o in task.get_objects() if isinstance(o, expected)]
    if len(objects) != 1:
        raise RuntimeError(f'Expected one {expected} from {filename}, got {objects}')
    return objects[0]

mesh = imported('mesh.obj', unreal.StaticMesh)
texture = imported('texture.png', unreal.Texture2D)
material = unreal.load_asset('/Game/Phase0/ProbeMaterial')
if material is None:
    material = tools.create_asset('ProbeMaterial', '/Game/Phase0', unreal.Material, unreal.MaterialFactoryNew())
unreal.MaterialEditingLibrary.delete_all_material_expressions(material)
sample = unreal.MaterialEditingLibrary.create_material_expression(material, unreal.MaterialExpressionTextureSample)
sample.set_editor_property('texture', texture)
unreal.MaterialEditingLibrary.connect_material_property(sample, 'RGB', unreal.MaterialProperty.MP_BASE_COLOR)
unreal.MaterialEditingLibrary.connect_material_property(sample, 'RGB', unreal.MaterialProperty.MP_EMISSIVE_COLOR)
material.set_editor_property('two_sided', True)
material.set_editor_property('blend_mode', unreal.BlendMode.BLEND_OPAQUE)
material.set_editor_property('shading_model', unreal.MaterialShadingModel.MSM_UNLIT)
unreal.MaterialEditingLibrary.recompile_material(material)
mesh.set_material(0, material)
unreal.EditorAssetLibrary.save_loaded_asset(material)
unreal.EditorAssetLibrary.save_loaded_asset(mesh)

level = unreal.get_editor_subsystem(unreal.LevelEditorSubsystem)
actors = unreal.get_editor_subsystem(unreal.EditorActorSubsystem)
if not level.new_level('/Game/Phase0/Phase0'):
    if not level.load_level('/Game/Phase0/Phase0'):
        raise RuntimeError('Cannot create/load probe level')
    # Only replace actors previously created by this script.
    for actor in actors.get_all_level_actors():
        if actor.get_actor_label().startswith('OpenWillow_'):
            actors.destroy_actor(actor)
actor = actors.spawn_actor_from_class(unreal.StaticMeshActor, unreal.Vector())
actor.set_actor_label('OpenWillow_Mesh')
actor.static_mesh_component.set_static_mesh(mesh)
actor.static_mesh_component.set_material(0, material)
center, extent = actor.get_actor_bounds(False)
distance = max(extent.x, extent.y, extent.z, 100) * 2.4
camera_pos = center + unreal.Vector(distance, -distance, distance)
rotation = unreal.MathLibrary.find_look_at_rotation(camera_pos, center)
unreal.EditorLevelLibrary.set_level_viewport_camera_info(camera_pos, rotation)
camera = actors.spawn_actor_from_class(unreal.CameraActor, camera_pos, rotation)
camera.set_actor_label('OpenWillow_Camera')
camera.set_editor_property('auto_activate_for_player', unreal.AutoReceiveInput.PLAYER0)
sun = actors.spawn_actor_from_class(unreal.DirectionalLight, center + unreal.Vector(0,0,distance), unreal.Rotator(-50,-30,0))
sun.set_actor_label('OpenWillow_Sun')
sun.get_component_by_class(unreal.DirectionalLightComponent).set_editor_property('mobility', unreal.ComponentMobility.MOVABLE)
sky = actors.spawn_actor_from_class(unreal.SkyLight, center)
sky.set_actor_label('OpenWillow_Sky')
sky.get_component_by_class(unreal.SkyLightComponent).set_editor_property('mobility', unreal.ComponentMobility.MOVABLE)
level.save_current_level()
unreal.log('OpenWillow probe imported. Inspect winding, UVs and material, then capture the Phase 0 screenshot.')
# Import success is deliberately distinct from visual/render acceptance.
(root/'ue-import.json').write_text(json.dumps({'imported': True, 'mesh': mesh.get_path_name(),
    'texture': texture.get_path_name(), 'visual_gate': 'pending inspection', 'probe': manifest}, indent=2), encoding='utf-8')
