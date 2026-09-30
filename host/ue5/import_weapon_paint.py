"""Apply one local approximate paint material to an existing rolled gun mesh.

OPENWILLOW_WEAPON_PAINT is prepare_weapon_paint.py's ignored local JSON.
No mesh reimport, no directory deletion. Master_Gun's missing graph, packed
detail channel, pattern UV1 and lighting response remain UNVERIFIED.
"""
import json
import os
import re
from pathlib import Path
import unreal

def apply(data):
    recipe_id = data['recipe_id']
    if not re.fullmatch(r'[A-Za-z0-9_]+', recipe_id):
        raise RuntimeError('Invalid recipe ID')
    mesh = unreal.load_asset(f'/Game/OpenWillow/Weapons/Items/SK_{recipe_id}')
    if not isinstance(mesh, unreal.SkeletalMesh):
        raise RuntimeError('Import the recipe mesh first')
    tools = unreal.AssetToolsHelpers.get_asset_tools()
    eal = unreal.EditorAssetLibrary
    mel = unreal.MaterialEditingLibrary
    destination = f'/Game/OpenWillow/Weapons/Paint/{recipe_id}'
    path = destination + '/M_OW_PaintApprox'
    material = unreal.load_asset(path) if eal.does_asset_exist(path) else tools.create_asset(
        'M_OW_PaintApprox', destination, unreal.Material, unreal.MaterialFactoryNew())
    mel.delete_all_material_expressions(material)
    material.set_editor_property('used_with_skeletal_mesh', True)


    def node(cls, **properties):
        value = mel.create_material_expression(material, cls, -500, 0)
        for key, prop in properties.items():
            value.set_editor_property(key, prop)
        return value


    def texture(parameter, normal=False, color=False):
        task = unreal.AssetImportTask()
        task.filename = data['textures'][parameter]
        task.destination_path = destination
        task.automated = True
        task.replace_existing = True
        task.save = False
        tools.import_asset_tasks([task])
        textures = [asset for asset in task.get_objects() if isinstance(asset, unreal.Texture2D)]
        if len(textures) != 1:
            raise RuntimeError(f'Expected one texture for {parameter}')
        asset = textures[0]
        asset.set_editor_property('srgb', color)
        if normal:
            asset.set_editor_property('compression_settings', unreal.TextureCompressionSettings.TC_NORMALMAP)
        eal.save_loaded_asset(asset, only_if_is_dirty=False)
        return node(unreal.MaterialExpressionTextureSample, texture=asset,
            sampler_type=unreal.MaterialSamplerType.SAMPLERTYPE_NORMAL if normal else
                unreal.MaterialSamplerType.SAMPLERTYPE_COLOR if color else unreal.MaterialSamplerType.SAMPLERTYPE_LINEAR_COLOR)


    inputs = {'Mask': texture('p_Masks'), 'Detail': texture('p_Diffuse'),
              'Pattern': texture('p_Pattern', color=True)}
    normal = texture('p_NormalScopesEmissive', normal=True)
    vectors = data['params']['vector']
    for zone in 'ABC':
        for tone in ['Shadow', 'Midtone', 'Hilight']:
            inputs[zone+tone] = node(unreal.MaterialExpressionConstant3Vector,
                constant=unreal.LinearColor(*vectors[f'p_{zone}Color{tone}'][:3], 1))
    for name, parameter in [('PatternColor', 'p_PatternColor'), ('PatternWeight', 'p_PatternChannelScale')]:
        inputs[name] = node(unreal.MaterialExpressionConstant3Vector,
            constant=unreal.LinearColor(*vectors[parameter][:3], 1))
    scale = vectors['p_PatternScalePosition']
    uv = node(unreal.MaterialExpressionTextureCoordinate, coordinate_index=1, u_tiling=scale[0], v_tiling=scale[1])
    offset = node(unreal.MaterialExpressionConstant2Vector, r=scale[2], g=scale[3])
    shift = node(unreal.MaterialExpressionAdd)
    mel.connect_material_expressions(uv, '', shift, 'A')
    mel.connect_material_expressions(offset, '', shift, 'B')
    mel.connect_material_expressions(shift, '', inputs['Pattern'], 'UVs')
    channel = 'rgb'[data['detail_channel']]
    code = f'float d=Detail.{channel}; float low=saturate(d*2), high=saturate(d*2-1);\n'
    for zone in 'ABC':
        code += f'float3 {zone}=lerp(lerp({zone}Shadow,{zone}Midtone,low),{zone}Hilight,high);\n'
    code += '''float3 base=A*Mask.r+B*Mask.g+C*Mask.b;
    base += (1-saturate(Mask.r+Mask.g+Mask.b))*float3(.2,.2,.22);
    base=lerp(base,Pattern.rgb*PatternColor*d,saturate(dot(Mask.rgb,PatternWeight)));
    // Compress together to retain HDR palette hue within UE's base-color range.
    return base/(1+max(base.r,max(base.g,base.b)));
    '''
    effect = node(unreal.MaterialExpressionCustom, code=code,
        output_type=unreal.CustomMaterialOutputType.CMOT_FLOAT3)
    custom_inputs = []
    for name in inputs:
        entry = unreal.CustomInput()
        entry.set_editor_property('input_name', name)
        custom_inputs.append(entry)
    effect.set_editor_property('inputs', custom_inputs)
    for name, value in inputs.items():
        mel.connect_material_expressions(value, '', effect, name)
    mel.connect_material_property(effect, '', unreal.MaterialProperty.MP_BASE_COLOR)
    mel.connect_material_property(normal, 'RGB', unreal.MaterialProperty.MP_NORMAL)
    for prop, value in [(unreal.MaterialProperty.MP_METALLIC, .35), (unreal.MaterialProperty.MP_ROUGHNESS, .55)]:
        mel.connect_material_property(node(unreal.MaterialExpressionConstant, r=value), '', prop)
    mel.recompile_material(material)
    eal.save_loaded_asset(material, only_if_is_dirty=False)
    slots = mesh.get_editor_property('materials')
    for index in range(len(slots)):
        slot = slots[index]
        slot.set_editor_property('material_interface', material)
        slots[index] = slot
    mesh.modify()
    mesh.set_editor_property('materials', slots)
    eal.save_loaded_asset(mesh, only_if_is_dirty=False)
    unreal.log(f'OW_PAINT {recipe_id}: {data["material_identity"]}, {len(slots)} slots, shader UNVERIFIED')


data = json.loads(Path(os.environ['OPENWILLOW_WEAPON_PAINT']).read_text())
for entry in data if isinstance(data, list) else [data]:
    apply(entry)
