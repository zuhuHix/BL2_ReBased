"""Give Maya's imported materials a flatter, inked look (editor Python).

Run after import_character.py. Idempotent:
- M_OW_CharacterOutline: an inverted-hull ink line. Unlit near-black, drawn on
  back faces only (two-sided, masked by TwoSidedSign) and pushed out along the
  vertex normal by the `ThicknessCm` parameter. The inventory display copies of
  Maya's meshes use it (AOpenWillowInventoryMayaDisplay).
- M_OW_Character: Specular and Roughness constants, so the default skin loses
  the glossy sheen of UE's default lit surface.
- M_OW_MenuBackdrop: world-only dimming/desaturation/vignette after tonemapping;
  visible Maya and outline pixels use reserved custom stencil 247.

BL2 draws its ink lines with its own shaders; `Master_Player`'s graph is
stripped in the cooked data, so both are art-direction approximations
(UNVERIFIED), not reconstructions.
"""
import unreal

destination = '/Game/OpenWillow/Characters/Maya'
tools = unreal.AssetToolsHelpers.get_asset_tools()
eal = unreal.EditorAssetLibrary
mel = unreal.MaterialEditingLibrary
MP = unreal.MaterialProperty


def node(material, cls, x, y, **props):
    expression = mel.create_material_expression(material, cls, x, y)
    for key, value in props.items():
        expression.set_editor_property(key, value)
    return expression


def outline_material():
    path = f'{destination}/M_OW_CharacterOutline'
    if eal.does_asset_exist(path):
        eal.delete_asset(path)
    material = tools.create_asset('M_OW_CharacterOutline', destination, unreal.Material, unreal.MaterialFactoryNew())
    material.set_editor_property('shading_model', unreal.MaterialShadingModel.MSM_UNLIT)
    material.set_editor_property('blend_mode', unreal.BlendMode.BLEND_MASKED)
    material.set_editor_property('two_sided', True)
    material.set_editor_property('used_with_skeletal_mesh', True)
    ink = node(material, unreal.MaterialExpressionConstant3Vector, -400, -200,
               constant=unreal.LinearColor(0.015, 0.015, 0.02, 1.0))
    mel.connect_material_property(ink, '', MP.MP_EMISSIVE_COLOR)
    # Back faces have TwoSidedSign -1: (1 - sign) / 2 is 1 there and 0 on front faces.
    sign = node(material, unreal.MaterialExpressionTwoSidedSign, -600, 0)
    flip = node(material, unreal.MaterialExpressionOneMinus, -450, 0)
    mel.connect_material_expressions(sign, '', flip, '')
    half = node(material, unreal.MaterialExpressionMultiply, -300, 0, const_b=0.5)
    mel.connect_material_expressions(flip, '', half, 'A')
    mel.connect_material_property(half, '', MP.MP_OPACITY_MASK)
    normal = node(material, unreal.MaterialExpressionVertexNormalWS, -600, 200)
    thickness = node(material, unreal.MaterialExpressionScalarParameter, -600, 320,
                     parameter_name='ThicknessCm', default_value=0.35)
    offset = node(material, unreal.MaterialExpressionMultiply, -300, 240)
    mel.connect_material_expressions(normal, '', offset, 'A')
    mel.connect_material_expressions(thickness, '', offset, 'B')
    mel.connect_material_property(offset, '', MP.MP_WORLD_POSITION_OFFSET)
    mel.recompile_material(material)
    eal.save_loaded_asset(material, only_if_is_dirty=False)
    unreal.log(f'OW_LOOK {path}')


def matte_character():
    material = unreal.load_asset(f'{destination}/M_OW_Character')
    if material is None:
        raise RuntimeError('Import Maya with import_character.py first')
    for prop, value, y in ((MP.MP_SPECULAR, 0.15, 400), (MP.MP_ROUGHNESS, 0.85, 480)):
        if mel.get_material_property_input_node(material, prop) is None:
            constant = node(material, unreal.MaterialExpressionConstant, -300, y, r=value)
            mel.connect_material_property(constant, '', prop)
    mel.recompile_material(material)
    eal.save_loaded_asset(material, only_if_is_dirty=False)
    unreal.log('OW_LOOK M_OW_Character specular 0.15 roughness 0.85')


def backdrop_material():
    """After-tonemap world grading, preserving visible stencil-247 Maya pixels."""
    path = f'{destination}/M_OW_MenuBackdrop'
    material = unreal.load_asset(path)
    if material is None:
        material = tools.create_asset('M_OW_MenuBackdrop', destination, unreal.Material, unreal.MaterialFactoryNew())
    mel.delete_all_material_expressions(material)
    material.set_editor_property('material_domain', unreal.MaterialDomain.MD_POST_PROCESS)
    material.set_editor_property('blendable_location', unreal.BlendableLocation.BL_SCENE_COLOR_AFTER_TONEMAPPING)
    inputs = {}
    for name, texture in (('Color', unreal.SceneTextureId.PPI_POST_PROCESS_INPUT0),
                          ('Stencil', unreal.SceneTextureId.PPI_CUSTOM_STENCIL),
                          ('CustomDepth', unreal.SceneTextureId.PPI_CUSTOM_DEPTH),
                          ('Depth', unreal.SceneTextureId.PPI_SCENE_DEPTH)):
        inputs[name] = node(material, unreal.MaterialExpressionSceneTexture, -600, len(inputs) * 100,
                            scene_texture_id=texture)
    inputs['UV'] = node(material, unreal.MaterialExpressionScreenPosition, -600, 400)
    inputs['Gain'] = node(material, unreal.MaterialExpressionVectorParameter, -600, 500,
                          parameter_name='BackdropGain', default_value=unreal.LinearColor(0.42, 0.44, 0.5, 1))
    inputs['Saturation'] = node(material, unreal.MaterialExpressionScalarParameter, -600, 600,
                                parameter_name='BackdropSaturation', default_value=0.55)
    inputs['Vignette'] = node(material, unreal.MaterialExpressionScalarParameter, -600, 700,
                              parameter_name='VignetteIntensity', default_value=1.1)
    effect = node(material, unreal.MaterialExpressionCustom, 0, 0,
                  output_type=unreal.CustomMaterialOutputType.CMOT_FLOAT3,
                  code='''
// Depth comparison prevents a hidden Maya silhouette protecting world pixels.
if (abs(Stencil.r - 247.0) < 0.5 && CustomDepth.r <= Depth.r + 1.0)
    return Color.rgb;
float grey = dot(Color.rgb, float3(0.2126, 0.7152, 0.0722));
float2 p = UV.xy * 2.0 - 1.0;
float shade = 1.0 - saturate(dot(p, p) * Vignette * 0.25);
return lerp(grey.xxx, Color.rgb, Saturation) * Gain.rgb * shade;
''')
    custom_inputs = []
    for name in inputs:
        custom_input = unreal.CustomInput()
        custom_input.set_editor_property('input_name', name)
        custom_inputs.append(custom_input)
    effect.set_editor_property('inputs', custom_inputs)
    for name, expression in inputs.items():
        mel.connect_material_expressions(expression, 'ViewportUV' if name == 'UV' else '', effect, name)
    mel.connect_material_property(effect, '', MP.MP_EMISSIVE_COLOR)
    mel.recompile_material(material)
    eal.save_loaded_asset(material, only_if_is_dirty=False)
    unreal.log(f'OW_LOOK {path}')


outline_material()
matte_character()
backdrop_material()
