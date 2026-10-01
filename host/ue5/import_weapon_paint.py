"""Apply one local approximate paint material to an existing rolled gun mesh.

OPENWILLOW_WEAPON_PAINT is prepare_weapon_paint.py's ignored local JSON.
No mesh reimport, no directory deletion. The colour model is the UNVERIFIED
reading in tools/weapon_paint_model.py (data['reading']), recovered from the
compiled Master_Gun shaders: p_Masks holds a light/dark map (upper half) and the
zone mask (lower half); zone tones, zones over p_DColor, pattern and decal
layers, times the detail channel the MIC's static parameters select. The
environment reflection, emissive and the game's lighting are not reproduced;
DISPLAY_SCALE maps the HDR result into UE's base-colour range and is chosen by
eye (UNVERIFIED).
"""
import json
import os
import re
from pathlib import Path
import unreal

DISPLAY_SCALE = 0.4  # HDR paint colour -> UE base colour; chosen by eye against screenshots, UNVERIFIED


def channel_code(texel, channels):
    """HLSL for a static component mask: one channel broadcasts, several keep their places."""
    picked = [c for c in 'rgb' if c.upper() in channels]
    if len(picked) == 1:
        return f'{texel}.{picked[0]}.xxx'
    return f'({texel}.rgb*float3({",".join("1" if c.upper() in channels else "0" for c in "rgb")}))'


def apply(data):
    recipe_id = data['recipe_id']
    if not re.fullmatch(r'[A-Za-z0-9_]+', recipe_id):
        raise RuntimeError('Invalid recipe ID')
    mesh_path = data.get('mesh') or f'/Game/OpenWillow/Weapons/Items/SK_{recipe_id}'
    if not re.fullmatch(r'/Game/OpenWillow/Weapons/[A-Za-z0-9_/]+', mesh_path):
        raise RuntimeError('Invalid mesh path')
    mesh = unreal.load_asset(mesh_path)
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

    def import_texture(parameter, normal=False, color=False, address=None):
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
            # TC_NORMALMAP keeps only red/green and rebuilds z, as the game's shader does (blue is not normal data).
            asset.set_editor_property('compression_settings', unreal.TextureCompressionSettings.TC_NORMALMAP)
        # Installed Texture2D AddressX/AddressY (e.g. ['TA_Clamp', 'TA_Wrap']), read by prepare_weapon_paint.py.
        for prop, mode in zip(('address_x', 'address_y'), address or []):
            asset.set_editor_property(prop, getattr(unreal.TextureAddress, mode.upper()))
        eal.save_loaded_asset(asset, only_if_is_dirty=False)
        return asset

    def sample(asset, normal=False, color=False):
        return node(unreal.MaterialExpressionTextureSample, texture=asset,
                    sampler_type=unreal.MaterialSamplerType.SAMPLERTYPE_NORMAL if normal else
                    unreal.MaterialSamplerType.SAMPLERTYPE_COLOR if color else
                    unreal.MaterialSamplerType.SAMPLERTYPE_LINEAR_COLOR)

    def custom(code, output, names):
        effect = node(unreal.MaterialExpressionCustom, code=code, output_type=output)
        entries = []
        for name in names:
            entry = unreal.CustomInput()
            entry.set_editor_property('input_name', name)
            entries.append(entry)
        effect.set_editor_property('inputs', entries)
        return effect

    def feed_uv(target, coordinate, scale=(1.0, 1.0), offset=(0.0, 0.0)):
        uv = node(unreal.MaterialExpressionTextureCoordinate, coordinate_index=coordinate,
                  u_tiling=scale[0], v_tiling=scale[1])
        shift = node(unreal.MaterialExpressionAdd)
        mel.connect_material_expressions(uv, '', shift, 'A')
        mel.connect_material_expressions(node(unreal.MaterialExpressionConstant2Vector, r=offset[0], g=offset[1]),
                                         '', shift, 'B')
        mel.connect_material_expressions(shift, '', target, 'UVs')

    def constant3(values):
        return node(unreal.MaterialExpressionConstant3Vector, constant=unreal.LinearColor(*values[:3], 1))

    vectors, scalars = data['params']['vector'], data['params']['scalar']
    pattern = data.get('pattern_used', False)
    decal = data.get('decal') or {}
    use_decal = bool(decal.get('used'))
    inputs, outputs = {}, {}  # custom input -> expression, or (expression, output name)

    masks = import_texture('p_Masks')
    inputs['Light'] = sample(masks)
    feed_uv(inputs['Light'], 0, (1.0, 0.5))            # upper half: light/dark map
    inputs['Mask'] = sample(masks)
    feed_uv(inputs['Mask'], 0, (1.0, 0.5), (0.0, 0.5))  # lower half: zone mask
    inputs['Detail'] = sample(import_texture('p_Diffuse'))
    normal = sample(import_texture('p_NormalScopesEmissive', normal=True), normal=True)
    for zone in 'ABC':
        for tone in ['Shadow', 'Midtone', 'Hilight']:
            inputs[zone + tone] = constant3(vectors[f'p_{zone}Color{tone}'])
    inputs['DColor'] = constant3(vectors['p_DColor'])
    inputs['Tones'] = node(unreal.MaterialExpressionConstant2Vector, r=scalars['p_HighlightsIntensity'],
                           g=scalars['p_ShadowsIntensity'])
    if pattern:
        inputs['Pattern'] = sample(import_texture('p_Pattern', color=True), color=True)
        place = vectors['p_PatternScalePosition']
        feed_uv(inputs['Pattern'], 1, place[:2], place[2:4])
        inputs['PatternColor'] = constant3(vectors['p_PatternColor'])
        inputs['PatternWeight'] = constant3(vectors['p_PatternChannelScale'])
        inputs['PatternReplace'] = node(unreal.MaterialExpressionConstant, r=scalars['p_ReplacePattern'])
    if use_decal:
        inputs['Decal'] = sample(import_texture('p_Decal', color=True, address=decal['address']), color=True)
        outputs['DecalAlpha'] = (inputs['Decal'], 'A')
        place, angle = decal['scale_position'], decal['rotate'] * 3.14159265
        # Shift by zw, rotate about the centre, scale xy about the centre (tools/weapon_paint_model.decal_uv).
        uv_code = (f'float2 t=UV+float2({place[2]},{place[3]})-0.5; float s=sin({angle}), c=cos({angle});\n'
                   f'float2 r=float2(c*t.x-s*t.y, s*t.x+c*t.y)+0.5;\n'
                   f'return r*float2({place[0]},{place[1]})+(1-float2({place[0]},{place[1]}))*0.5;')
        uv_node = custom(uv_code, unreal.CustomMaterialOutputType.CMOT_FLOAT2, ['UV'])
        mel.connect_material_expressions(node(unreal.MaterialExpressionTextureCoordinate, coordinate_index=1),
                                         '', uv_node, 'UV')
        mel.connect_material_expressions(uv_node, '', inputs['Decal'], 'UVs')
        inputs['DecalColor'] = constant3(decal['color'])
        inputs['DecalWeight'] = constant3(decal['channel'])
        inputs['DecalModes'] = node(unreal.MaterialExpressionConstant2Vector, r=decal['full_color'], g=decal['replace'])

    detail = 'rgb'[data['detail_channel']]
    code = (f'float high=saturate(Light.r*Tones.x), low=saturate(Light.g*Tones.y);\n'
            f'float3 c=DColor;\n')
    for zone, channel in zip('ABC', 'rgb'):
        code += f'c=lerp(c, lerp(lerp({zone}Midtone,{zone}Hilight,high),{zone}Shadow,low), Mask.{channel});\n'
    if pattern:
        code += (f'float3 p={channel_code("Pattern", data.get("pattern_channels", "RGB"))}*PatternColor;\n'
                 f'float pw=saturate(dot(Mask.rgb*PatternWeight, Mask.rgb*PatternWeight));\n'
                 f'c=lerp(c*lerp(float3(1,1,1),p,pw), lerp(c,p,pw), PatternReplace);\n')
    if use_decal:
        code += (f'float3 d={channel_code("Decal", data.get("decal_channels", "RGB"))};\n'
                 f'float3 dt=lerp(DecalColor, DecalColor*d, DecalModes.x);\n'
                 f'float dw=saturate(dot(Mask.rgb*DecalWeight, Mask.rgb*DecalWeight));\n'
                 f'float3 da=lerp(d*dw, dw*DecalAlpha, DecalModes.x);\n'
                 f'c=lerp(c*lerp(float3(1,1,1),dt,da), lerp(c,dt,da), DecalModes.y);\n')
    code += (f'c*=Detail.{detail}*{DISPLAY_SCALE};\n'
             '// Keep hue when the scaled HDR colour still exceeds 1.\n'
             'return c/max(1,max(c.r,max(c.g,c.b)));')
    effect = custom(code, unreal.CustomMaterialOutputType.CMOT_FLOAT3, list(inputs) + list(outputs))
    for name, value in inputs.items():
        mel.connect_material_expressions(value, '', effect, name)
    for name, (value, output) in outputs.items():
        mel.connect_material_expressions(value, output, effect, name)
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
    unreal.log(f'OW_PAINT {recipe_id} -> {mesh_path}: {data["material_identity"]}, {len(slots)} slots, '
               f'detail {detail}, pattern {"used" if pattern else "not used"}, '
               f'decal {"used" if use_decal else "not used"}, shader UNVERIFIED')


data = json.loads(Path(os.environ['OPENWILLOW_WEAPON_PAINT']).read_text())
for entry in data if isinstance(data, list) else [data]:
    apply(entry)
