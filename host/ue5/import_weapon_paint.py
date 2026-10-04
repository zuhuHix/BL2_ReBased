"""Apply one local approximate paint material to an existing rolled gun mesh.

OPENWILLOW_WEAPON_PAINT is prepare_weapon_paint.py's ignored local JSON.
No mesh reimport, no directory deletion. The colour model is the UNVERIFIED
reading in tools/weapon_paint_model.py (data['reading']), recovered from the
compiled Master_Gun shaders: p_Masks holds a light/dark map (upper half) and the
zone mask (lower half); zone tones, zones over p_DColor, pattern and decal
layers, times the detail channel the MIC's static parameters select, plus the
P_SimpleReflect environment term (sampled at the tangent-space reflection
vector, as the compiled shader does). Textures use their installed SRGB flag
(data['srgb']). Emissive and the game's lighting are not reproduced.

Shading inputs (pass 3, UNVERIFIED; reasoning in tools/weapon_paint_model.py, "Lighting inputs"):
the compiled passes multiply the material colour by 0.4 before lighting (DIFFUSE_SCALE), and
their specular is pow(R.L, 15) times the engine's override only, i.e. the material's specular
input compiled to zero. So base colour = 0.4 x colour (hue-preserving clamp to 1), metallic 0,
specular 0, roughness 1: a diffuse-only surface whose shine is the environment term already
inside the colour. The host's Sanctuary lighting is not calibrated to the game's yet, and with those
inputs the guns render 4-7x darker than the reference screenshots (2026-10-02 stills), so the importer
keeps the pass-2 host stand-in (USE_SHADER_SHADING False: scale 1, metallic 0.35, roughness 0.55) until
the scene lighting is calibrated. Set it True to get the shader-grounded inputs.
"""
import json
import os
import re
from pathlib import Path
import unreal

# Pass 4 (2026-10-04, UNVERIFIED): BL2_ANALYTIC_LIGHTING builds an Unlit material that evaluates the original pixel
# shader's own lighting structure, albedo x 0.4 x (ambient + directional terms) + emissive, with constants instead of the
# engine's light environment, so the gun looks the same in every scene (see docs/verification/WEAPON_VISUALS.md). The
# constants are scalar/vector material parameters (OW_*) that OpenWillowGunLook.cpp overrides at run time.
BL2_ANALYTIC_LIGHTING = True
USE_SHADER_SHADING = False  # pass-3 PBR mapping, only used when BL2_ANALYTIC_LIGHTING is False
# Unreal's film tone mapper darkens and desaturates what an Unlit material emits, so the material inverts it: the colour it computes
# is the display colour wanted, and TONE_TABLE maps a display-linear grey (TT) to the scene-linear input (log2, LV) that the
# 1280x720 SceneCapture shows as that grey. Measured 2026-10-04 with the material's own debug ramp (OW_Debug 3) on the shotgun
# side capture; the world camera shows the same curve with the input 4 times smaller (OW_ViewScale 0.25 for the held weapon, 1
# for the capture). Per-channel use of a grey curve is an approximation (the mapper also desaturates highlights).
TONE_TT = [0.00061, 0.00353, 0.00888, 0.01665, 0.02685, 0.03947, 0.05451, 0.07197, 0.09186, 0.11416, 0.13889, 0.16605, 0.19562, 0.22762, 0.26205, 0.29889, 0.33816, 0.37985, 0.42396, 0.47050, 0.51945, 0.57084, 0.62464, 0.68087, 0.73952, 0.80059, 0.86408, 0.93000]
TONE_LV = [-4.1475, -3.4133, -3.0216, -2.7209, -2.4994, -2.2961, -2.1260, -1.9740, -1.8255, -1.6893, -1.5518, -1.4120, -1.2732, -1.1270, -0.9764, -0.8275, -0.6710, -0.5069, -0.3299, -0.1453, 0.0399, 0.2507, 0.4874, 0.7636, 1.0711, 1.4202, 1.8823, 2.4892]
LOOK_DEFAULTS = {'OW_ViewScale': 0.25, 'OW_Clip': 1.0,   # keep in step with OpenWillowGunLook.cpp (calibrated against real-game frames, see WEAPON_VISUALS.md)
    'OW_Diffuse': 0.5, 'OW_Ambient': 1.6, 'OW_Key': 1.2, 'OW_Fill': 0.5, 'OW_Emissive': 1.0,
    'OW_Debug': 0.0, 'OW_KeyDir': (-0.45, 0.55, -0.70), 'OW_FillDir': (0.70, -0.20, -0.40),
}
if USE_SHADER_SHADING:
    DIFFUSE_SCALE = 0.4  # read from the compiled Master_Gun passes (tools/weapon_paint_model.DIFFUSE_SCALE)
    SHADING = {'MP_METALLIC': 0.0, 'MP_SPECULAR': 0.0, 'MP_ROUGHNESS': 1.0}  # no material specular in the original
else:
    DIFFUSE_SCALE = 1.0  # host stand-in, chosen by eye against the wiki screenshots (pass 2)
    SHADING = {'MP_METALLIC': 0.35, 'MP_ROUGHNESS': 0.55}
# Pass-1 colour-space choice, used only when the prepared JSON has no SRGB flag (prepared without --reader).
FALLBACK_SRGB = {'p_Masks': False, 'p_Diffuse': False, 'p_NormalScopesEmissive': False, 'p_Pattern': True,
                 'p_Decal': True, 'P_SimpleReflect': True}


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

    srgb_flags = data.get('srgb') or {}

    def is_srgb(parameter):
        flag = srgb_flags.get(parameter)
        return FALLBACK_SRGB[parameter] if flag is None else bool(flag)

    def import_texture(parameter, normal=False, address=None):
        color = is_srgb(parameter)
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
        else:
            # A re-import keeps the old asset's settings; the packed normal/emissive texture used to be TC_NORMALMAP.
            asset.set_editor_property('compression_settings', unreal.TextureCompressionSettings.TC_DEFAULT)
        # Installed Texture2D AddressX/AddressY (e.g. ['TA_Clamp', 'TA_Wrap']), read by prepare_weapon_paint.py.
        for prop, mode in zip(('address_x', 'address_y'), address or []):
            asset.set_editor_property(prop, getattr(unreal.TextureAddress, mode.upper()))
        eal.save_loaded_asset(asset, only_if_is_dirty=False)
        return asset

    def sample(asset, normal=False):
        color = bool(asset.get_editor_property('srgb'))
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

    def build_analytic_shading(albedo, packed_texture, vector_values):
        """Unlit surface = hue-clamped albedo x diffuse x (ambient + key N.L1 + fill N.L2), plus emissive.

        The structure follows the compiled Master_Gun base pass (colour x 0.4 x lighting terms + an emissive term that is
        added after lighting and clamped to 4). The lighting terms are constants in view space, not the game's light
        environment (UNVERIFIED stand-in, calibrated against real frames). Emissive = p_EmissiveColor x the packed
        texture's blue channel (UNVERIFIED reading of the pixel shader).
        """
        def scalar_param(name):
            return node(unreal.MaterialExpressionScalarParameter, parameter_name=name, default_value=LOOK_DEFAULTS[name])

        def vector_param(name):
            x, y, z = LOOK_DEFAULTS[name]
            return node(unreal.MaterialExpressionVectorParameter, parameter_name=name,
                        default_value=unreal.LinearColor(x, y, z, 1))

        to_view = view_normal
        n_tone = len(TONE_TT)
        tone = (f'float TT[{n_tone}]={{' + ','.join(f'{x:.5f}' for x in TONE_TT) + '};\n'
                f'float LV[{n_tone}]={{' + ','.join(f'{x:.4f}' for x in TONE_LV) + '};\n')
        shade = custom(
            'float3 n=normalize(N);\n'
            'float light=Amb+Key*saturate(dot(n,normalize(KD)))+Fill*saturate(dot(n,normalize(FD)));\n'
            'float3 lit=A*Diff*light;\n'
            'if (Clp>0.5) lit=saturate(lit); else lit/=max(1.0,max(lit.r,max(lit.g,lit.b)));  // OW_Clip 0: HDR zone colours keep their hue; 1: per-channel clip\n'
            'float3 glow=min(Em*EC*EmGain, 4.0);\n'
            'if (Dbg>0.5 && Dbg<1.5) return A/max(1.0,max(A.r,max(A.g,A.b)));\n'
            'if (Dbg>2.5) { float v=exp2(lerp(-5.5,2.8,saturate((SP.x-0.33)/0.42))); return float3(v,v,v); }\n'
            'if (Dbg>1.5) return glow;\n'
            'float3 want=lit+glow;\n'
            + tone +
            'float3 outc=float3(0,0,0);\n'
            'for (int ch=0; ch<3; ch++)\n'
            '{\n'
            '    float t=min(want[ch], TT[' + str(n_tone - 1) + ']);\n'
            '    float v=0;\n'
            '    for (int i=1; i<' + str(n_tone) + '; i++)\n'
            '        if (t>TT[i-1]) v=exp2(lerp(LV[i-1],LV[i],saturate((t-TT[i-1])/(TT[i]-TT[i-1]))));\n'
            '    outc[ch]=v;\n'
            '}\n'
            'return outc*VS;',
            unreal.CustomMaterialOutputType.CMOT_FLOAT3,
            ['A', 'N', 'Diff', 'Amb', 'Key', 'Fill', 'KD', 'FD', 'Em', 'EC', 'EmGain', 'Dbg', 'SP', 'VS', 'Clp'])
        links = {'A': (albedo, ''), 'N': (to_view, ''), 'Diff': (scalar_param('OW_Diffuse'), ''),
                 'Amb': (scalar_param('OW_Ambient'), ''), 'Key': (scalar_param('OW_Key'), ''),
                 'Fill': (scalar_param('OW_Fill'), ''), 'KD': (vector_param('OW_KeyDir'), ''),
                 'FD': (vector_param('OW_FillDir'), ''), 'Em': (packed_texture, 'B'),
                 'EC': (constant3(vector_values.get('p_EmissiveColor', (0, 0, 0))), ''),
                 'EmGain': (scalar_param('OW_Emissive'), ''), 'Dbg': (scalar_param('OW_Debug'), ''),
                 'SP': (node(unreal.MaterialExpressionScreenPosition), ''), 'VS': (scalar_param('OW_ViewScale'), ''), 'Clp': (scalar_param('OW_Clip'), '')}
        for name, (source, output) in links.items():
            mel.connect_material_expressions(source, output, shade, name)
        material.set_editor_property('shading_model', unreal.MaterialShadingModel.MSM_UNLIT)
        mel.connect_material_property(shade, '', unreal.MaterialProperty.MP_EMISSIVE_COLOR)

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
    # One linear texture: red/green are the tangent-space normal, blue the emissive mask (sparse; 0 on most texels).
    # Importing it as TC_NORMALMAP would drop the blue channel, so the normal is rebuilt in the shader code below.
    packed = sample(import_texture('p_NormalScopesEmissive'))
    mel.connect_material_expressions(node(unreal.MaterialExpressionTextureCoordinate, coordinate_index=0), '', packed, 'UVs')
    view_normal = None
    if BL2_ANALYTIC_LIGHTING:
        unpack = custom('float2 xy=P.rg*2-1; return normalize(float3(xy, 0.8*sqrt(saturate(1-dot(xy,xy)))));',
                        unreal.CustomMaterialOutputType.CMOT_FLOAT3, ['P'])
        mel.connect_material_expressions(packed, 'RGB', unpack, 'P')
        view_normal = node(unreal.MaterialExpressionTransform,
                           transform_source_type=unreal.MaterialVectorCoordTransformSource.TRANSFORMSOURCE_TANGENT,
                           transform_type=unreal.MaterialVectorCoordTransform.TRANSFORM_VIEW)
        mel.connect_material_expressions(unpack, '', view_normal, '')
    for zone in 'ABC':
        for tone in ['Shadow', 'Midtone', 'Hilight']:
            inputs[zone + tone] = constant3(vectors[f'p_{zone}Color{tone}'])
    inputs['DColor'] = constant3(vectors['p_DColor'])
    inputs['Tones'] = node(unreal.MaterialExpressionConstant2Vector, r=scalars['p_HighlightsIntensity'],
                           g=scalars['p_ShadowsIntensity'])
    if pattern:
        inputs['Pattern'] = sample(import_texture('p_Pattern'))
        place = vectors['p_PatternScalePosition']
        feed_uv(inputs['Pattern'], 1, place[:2], place[2:4])
        inputs['PatternColor'] = constant3(vectors['p_PatternColor'])
        inputs['PatternWeight'] = constant3(vectors['p_PatternChannelScale'])
        inputs['PatternReplace'] = node(unreal.MaterialExpressionConstant, r=scalars['p_ReplacePattern'])
    if use_decal:
        inputs['Decal'] = sample(import_texture('p_Decal', address=decal['address']))
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

    reflection = data.get('reflection') or {}
    use_reflection = bool(reflection.get('used'))
    if use_reflection:
        inputs['Env'] = sample(import_texture('P_SimpleReflect', address=reflection['address']))
        # Tangent-space reflection xy plus frac(object position xy * 0.001), as the compiled shader samples it.
        to_tangent = node(unreal.MaterialExpressionTransform,
                          transform_source_type=unreal.MaterialVectorCoordTransformSource.TRANSFORMSOURCE_WORLD,
                          transform_type=unreal.MaterialVectorCoordTransform.TRANSFORM_TANGENT)
        mel.connect_material_expressions(node(unreal.MaterialExpressionReflectionVectorWS), '', to_tangent, '')
        if BL2_ANALYTIC_LIGHTING:
            # The Unlit path has no usable reflection vector (it gave a flat purple wash over the Infinity), so the lookup uses the
            # view-space reflection of the camera ray about the shaded normal, without the per-object offset (UNVERIFIED stand-in).
            env_uv = custom('float3 n=normalize(N); float3 v=float3(0,0,-1); float3 r=2*dot(n,v)*n-v; return r.xy;',
                            unreal.CustomMaterialOutputType.CMOT_FLOAT2, ['N'])
            mel.connect_material_expressions(view_normal, '', env_uv, 'N')
        else:
            env_uv = custom('return R.xy + frac(O.xy * 0.001);', unreal.CustomMaterialOutputType.CMOT_FLOAT2, ['R', 'O'])
            mel.connect_material_expressions(to_tangent, '', env_uv, 'R')
            mel.connect_material_expressions(node(unreal.MaterialExpressionObjectPositionWS), '', env_uv, 'O')
        mel.connect_material_expressions(env_uv, '', inputs['Env'], 'UVs')
        inputs['ReflectColor'] = constant3(reflection['p_ReflectColor'])
        inputs['ReflectWeight'] = constant3(reflection['p_ReflectionChannelScale'])
        inputs['ReflectScale'] = node(unreal.MaterialExpressionConstant, r=reflection['p_ReflectColorScale'])

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
    code += f'c*=Detail.{detail};\n'
    if use_reflection:
        # Environment term (tools/weapon_paint_model.add_reflection).
        code += ('float rw=saturate(dot(Mask.rgb*ReflectWeight, Mask.rgb*ReflectWeight));\n'
                 'float3 R=Env.rgb*ReflectColor*rw;\n'
                 'c=c+lerp(R, R*c, ReflectScale);\n')
    if BL2_ANALYTIC_LIGHTING:
        code += 'return c;\n'  # HDR albedo (zone colours reach 3.6); the 0.4 and the light are applied in the shade node
    else:
        code += (f'c*={DIFFUSE_SCALE};\n'
                 '// Keep hue when the HDR colour exceeds 1.\n'
                 'return c/max(1,max(c.r,max(c.g,c.b)));')
    effect = custom(code, unreal.CustomMaterialOutputType.CMOT_FLOAT3, list(inputs) + list(outputs))
    for name, value in inputs.items():
        mel.connect_material_expressions(value, '', effect, name)
    for name, (value, output) in outputs.items():
        mel.connect_material_expressions(value, output, effect, name)
    if BL2_ANALYTIC_LIGHTING:
        build_analytic_shading(effect, packed, vectors)
    else:
        mel.connect_material_property(effect, '', unreal.MaterialProperty.MP_BASE_COLOR)
        normal_tangent = custom('float2 xy=P.rg*2-1; return float3(xy, sqrt(saturate(1-dot(xy,xy))));',
                                unreal.CustomMaterialOutputType.CMOT_FLOAT3, ['P'])
        mel.connect_material_expressions(packed, 'RGB', normal_tangent, 'P')
        mel.connect_material_property(normal_tangent, '', unreal.MaterialProperty.MP_NORMAL)
        for prop, value in [(getattr(unreal.MaterialProperty, name), value) for name, value in SHADING.items()]:
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
               f'decal {"used" if use_decal else "not used"}, reflection {"used" if use_reflection else "not used"}, '
               f'srgb {sorted(k for k in data["textures"] if is_srgb(k))}, shader UNVERIFIED')


data = json.loads(Path(os.environ['OPENWILLOW_WEAPON_PAINT']).read_text())
for entry in data if isinstance(data, list) else [data]:
    apply(entry)
