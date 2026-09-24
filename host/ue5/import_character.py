"""Import Maya's UModel glTF meshes and PNG textures into the UE5 host (editor Python).

OPENWILLOW_CHARACTER points at a local folder holding the UModel exports
(the three .gltf/.bin pairs and the .png textures (UE5 rejects BC-compressed DDS), any layout). Nothing
game-derived is tracked.

Materials are a labeled approximation of Master_Player, whose graph was stripped
at cook time. The _Msk right half (u/2 + 0.5) holds three skin zones in R, G, B
(hair = R on the head); each zone multiplies the diffuse by that zone's Midtone
color. The combine, the factor 2, the Shadow/Hilight colors and the Msk left
half are UNVERIFIED and not reproduced.
"""
import os
from pathlib import Path
import unreal

root = Path(os.environ['OPENWILLOW_CHARACTER']).resolve()
destination = '/Game/OpenWillow/Characters/Maya'
tools = unreal.AssetToolsHelpers.get_asset_tools()
mel = unreal.MaterialEditingLibrary
eal = unreal.EditorAssetLibrary

# mesh file stem -> texture prefix
PARTS = {'Skel_SirenBody': 'SirenBody', 'Skel_Siren000': 'SirenHead', 'Hands_Siren': 'SirenHands'}
# p_AColorMidtone / p_BColorMidtone / p_CColorMidtone of the default skin's
# CD_Skins_Siren_MainGame.Mati_Default_Body and Mati_Default_Head, read with
# ow-package --properties (level-arrays.schema). The hands reusing the body
# colors is an assumption.
BODY = [(0.0270485226, 0.0370314158, 0.0386726074), (0.618761718, 0.618761718, 0.618761718),
        (0.359102786, 0.193826273, 0.0198524334)]
HEAD = [(0.19630821, 0.20426555, 0.602187753), (0.0939191952, 0.0939191952, 0.0939191952),
        (0.977860332, 0.955720723, 1.0)]
ZONES = {'SirenBody': BODY, 'SirenHead': HEAD, 'SirenHands': BODY}


def find(name):
    hits = list(root.rglob(name))
    if len(hits) != 1:
        raise RuntimeError(f'Expected one {name} under {root}: {hits}')
    return hits[0]


def imported(path, subfolder, expected):
    task = unreal.AssetImportTask()
    task.filename = str(path)
    task.destination_path = f'{destination}/{subfolder}'
    task.automated = True
    task.replace_existing = True
    task.save = False
    tools.import_asset_tasks([task])
    objects = [o for o in task.get_objects() if isinstance(o, expected)]
    if len(objects) != 1:
        raise RuntimeError(f'Expected one {expected.__name__} from {path.name}: {task.get_objects()}')
    return objects[0]


def texture(name, kind):
    asset = imported(find(name + '.png'), 'Textures', unreal.Texture2D)
    asset.set_editor_property('srgb', kind == 'color')
    if kind == 'normal':
        asset.set_editor_property('compression_settings', unreal.TextureCompressionSettings.TC_NORMALMAP)
    return asset


def node(material, cls, x, y, **props):
    expression = mel.create_material_expression(material, cls, x, y)
    for key, value in props.items():
        expression.set_editor_property(key, value)
    return expression


def binary(material, cls, a, a_pin, b, b_pin, x, y):
    expression = node(material, cls, x, y)
    mel.connect_material_expressions(a, a_pin, expression, 'A')
    mel.connect_material_expressions(b, b_pin, expression, 'B')
    return expression


def base_material():
    material = tools.create_asset('M_OW_Character', destination, unreal.Material, unreal.MaterialFactoryNew())
    M = unreal.MaterialExpressionMultiply
    # Texture parameters need a default to compile, and skeletal meshes need the
    # usage flag; either one missing renders UE's grey checker fallback instead.
    diffuse = node(material, unreal.MaterialExpressionTextureSampleParameter2D, -900, 0, parameter_name='Diffuse',
                   texture=unreal.load_asset('/Engine/EngineResources/DefaultTexture'))
    normal = node(material, unreal.MaterialExpressionTextureSampleParameter2D, -400, 500, parameter_name='Normal',
                  sampler_type=unreal.MaterialSamplerType.SAMPLERTYPE_NORMAL,
                  texture=unreal.load_asset('/Engine/EngineMaterials/DefaultNormal'))
    # Zone masks live in the right half of _Msk: uv * (0.5, 1) + (0.5, 0).
    uv = node(material, unreal.MaterialExpressionTextureCoordinate, -1400, 300, u_tiling=0.5, v_tiling=1.0)
    shift = node(material, unreal.MaterialExpressionConstant2Vector, -1400, 400, r=0.5, g=0.0)
    mask_uv = binary(material, unreal.MaterialExpressionAdd, uv, '', shift, '', -1200, 300)
    masks = node(material, unreal.MaterialExpressionTextureSampleParameter2D, -900, 300, parameter_name='Masks',
                 sampler_type=unreal.MaterialSamplerType.SAMPLERTYPE_LINEAR_COLOR,
                 texture=unreal.load_asset('/Engine/EngineResources/Black'))
    mel.connect_material_expressions(mask_uv, '', masks, 'UVs')
    zone = None
    for i, (letter, channel) in enumerate((('A', 'R'), ('B', 'G'), ('C', 'B'))):
        color = node(material, unreal.MaterialExpressionVectorParameter, -900, 600 + 150 * i,
                     parameter_name=f'{letter}ColorMidtone')
        term = binary(material, M, masks, channel, color, '', -600, 600 + 150 * i)
        zone = term if zone is None else binary(material, unreal.MaterialExpressionAdd, zone, '', term, '',
                                                 -450, 600 + 150 * i)
    coverage_rg = binary(material, unreal.MaterialExpressionAdd, masks, 'R', masks, 'G', -600, 350)
    coverage = binary(material, unreal.MaterialExpressionAdd, coverage_rg, '', masks, 'B', -450, 350)
    clamped = node(material, unreal.MaterialExpressionSaturate, -300, 350)
    mel.connect_material_expressions(coverage, '', clamped, '')
    two = node(material, unreal.MaterialExpressionConstant, -450, 850, r=2.0)
    zone2 = binary(material, M, zone, '', two, '', -300, 700)
    tinted = binary(material, M, diffuse, 'RGB', zone2, '', -150, 400)
    base = node(material, unreal.MaterialExpressionLinearInterpolate, 0, 0)
    mel.connect_material_expressions(diffuse, 'RGB', base, 'A')
    mel.connect_material_expressions(tinted, '', base, 'B')
    mel.connect_material_expressions(clamped, '', base, 'Alpha')
    mel.connect_material_property(base, '', unreal.MaterialProperty.MP_BASE_COLOR)
    mel.connect_material_property(normal, 'RGB', unreal.MaterialProperty.MP_NORMAL)
    material.set_editor_property('used_with_skeletal_mesh', True)
    material.set_editor_property('two_sided', False)
    mel.recompile_material(material)
    return material


# Everything under the destination is generated by this script; start clean so reruns work.
if eal.does_directory_exist(destination):
    eal.delete_directory(destination)
parent = base_material()
meshes = {}
for stem, prefix in PARTS.items():
    mesh = imported(find(stem + '.gltf'), 'Meshes', unreal.SkeletalMesh)
    instance = tools.create_asset(f'MI_{prefix}', f'{destination}/Materials',
                                  unreal.MaterialInstanceConstant, unreal.MaterialInstanceConstantFactoryNew())
    mel.set_material_instance_parent(instance, parent)
    mel.set_material_instance_texture_parameter_value(instance, 'Diffuse', texture(prefix + '_Dif', 'color'))
    mel.set_material_instance_texture_parameter_value(instance, 'Normal', texture(prefix + '_Nrm', 'normal'))
    mel.set_material_instance_texture_parameter_value(instance, 'Masks', texture(prefix + '_Msk', 'mask'))
    for letter, (r, g, b) in zip('ABC', ZONES[prefix]):
        mel.set_material_instance_vector_parameter_value(instance, f'{letter}ColorMidtone', unreal.LinearColor(r, g, b, 1))
    slots = mesh.get_editor_property('materials')
    if len(slots) != 1:
        raise RuntimeError(f'{stem}: expected one material slot, got {len(slots)}')
    # Indexing an unreal.Array of structs returns a copy; write the copy back.
    slot = slots[0]
    slot.set_editor_property('material_interface', instance)
    slots[0] = slot
    mesh.modify()
    mesh.set_editor_property('materials', slots)
    # save_directory skips packages it doesn't see as dirty; save explicitly.
    if not eal.save_loaded_asset(mesh, only_if_is_dirty=False):
        raise RuntimeError(f'Could not save {stem}')
    meshes[stem] = mesh

# A preview level with body and head at the origin, the arms beside them.
level = f'{destination}/MayaPreview'
if eal.does_asset_exist(level) and not eal.delete_asset(level):
    raise RuntimeError(f'Could not delete old {level}; is it open in an editor?')
if not unreal.EditorLevelLibrary.new_level(level):
    raise RuntimeError(f'Could not create {level}')
for stem, mesh in meshes.items():
    offset = unreal.Vector(0, 120, 0) if stem == 'Hands_Siren' else unreal.Vector(0, 0, 0)
    actor = unreal.EditorLevelLibrary.spawn_actor_from_class(unreal.SkeletalMeshActor, offset)
    actor.skeletal_mesh_component.set_skeletal_mesh_asset(mesh)
    actor.set_actor_label(stem)
light = unreal.EditorLevelLibrary.spawn_actor_from_class(unreal.DirectionalLight, unreal.Vector(0, 0, 300),
                                                         unreal.Rotator(0, -45, 135))
# Fixed exposure and a soft sun keep texture colors readable; the default auto
# exposure with a 10 lux sun washes the diffuse out to white. Inspection aid only.
light.light_component.set_editor_property('intensity', 2.5)
volume = unreal.EditorLevelLibrary.spawn_actor_from_class(unreal.PostProcessVolume, unreal.Vector(0, 0, 0))
volume.set_editor_property('unbound', True)
settings = volume.get_editor_property('settings')
settings.set_editor_property('override_auto_exposure_method', True)
settings.set_editor_property('auto_exposure_method', unreal.AutoExposureMethod.AEM_MANUAL)
settings.set_editor_property('override_auto_exposure_apply_physical_camera_exposure', True)
settings.set_editor_property('auto_exposure_apply_physical_camera_exposure', False)
volume.set_editor_property('settings', settings)
# Keep the default pawn out of shot, behind the preview camera.
unreal.EditorLevelLibrary.spawn_actor_from_class(unreal.PlayerStart, unreal.Vector(700, 60, 100))
# Without a sky there is nothing for the sky light to capture and the shadows go black.
unreal.EditorLevelLibrary.spawn_actor_from_class(unreal.SkyAtmosphere, unreal.Vector(0, 0, 0))
sky = unreal.EditorLevelLibrary.spawn_actor_from_class(unreal.SkyLight, unreal.Vector(0, 0, 400))
sky.light_component.set_editor_property('real_time_capture', True)
# A fixed camera so `-game` runs of the preview show a repeatable front view.
# OPENWILLOW_CHARACTER_VIEW picks the preview camera: x, y, z, pitch, yaw.
VIEWS = {'front': (350, 60, 100, 0, 180), 'face': (70, 0, 158, 0, 180), 'back': (-300, 0, 100, 0, 0)}
x, y, z, pitch, yaw = VIEWS[os.environ.get('OPENWILLOW_CHARACTER_VIEW', 'front')]
camera = unreal.EditorLevelLibrary.spawn_actor_from_class(unreal.CameraActor, unreal.Vector(x, y, z),
                                                          unreal.Rotator(roll=0, pitch=pitch, yaw=yaw))
camera.set_actor_label('OpenWillow_PreviewCamera')
camera.set_editor_property('auto_activate_for_player', unreal.AutoReceiveInput.PLAYER0)
# The project game mode spawns its own pawn and view; the preview only needs the camera.
unreal.EditorLevelLibrary.get_editor_world().get_world_settings().set_editor_property(
    'default_game_mode', unreal.GameModeBase)
unreal.EditorLevelLibrary.save_current_level()
eal.save_directory(destination)
for stem, mesh in meshes.items():
    box = mesh.get_bounds().box_extent
    slot = mesh.get_editor_property('materials')[0].get_editor_property('material_interface')
    unreal.log(f'OW_CHARACTER {stem} extent={box.x:.1f},{box.y:.1f},{box.z:.1f} material={slot.get_name()}')
unreal.log('OW_CHARACTER import complete')
