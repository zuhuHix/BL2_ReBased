"""Create the host-only soft procedural muzzle-flame material in the UE project."""
import unreal

destination = '/Game/OpenWillow/Weapons/InfinityProxy'
asset_name = 'M_OW_MuzzleFlame'
asset_path = f'{destination}/{asset_name}'
asset_tools = unreal.AssetToolsHelpers.get_asset_tools()
eal = unreal.EditorAssetLibrary
mel = unreal.MaterialEditingLibrary

if eal.does_asset_exist(asset_path):
    if not eal.delete_asset(asset_path):
        raise RuntimeError(f'Could not replace generated material: {asset_path}')

material = asset_tools.create_asset(asset_name, destination, unreal.Material, unreal.MaterialFactoryNew())
material.set_editor_property('blend_mode', unreal.BlendMode.BLEND_ADDITIVE)
material.set_editor_property('shading_model', unreal.MaterialShadingModel.MSM_UNLIT)
material.set_editor_property('two_sided', True)


def expression(kind, x, y):
    return mel.create_material_expression(material, kind, x, y)


def connect(source, output, target, input_name):
    mel.connect_material_expressions(source, output, target, input_name)


uv = expression(unreal.MaterialExpressionTextureCoordinate, -1200, 0)
centre = expression(unreal.MaterialExpressionConstant2Vector, -1200, 220)
centre.set_editor_property('r', 0.5)
centre.set_editor_property('g', 0.5)
distance = expression(unreal.MaterialExpressionDistance, -1000, 120)
connect(uv, '', distance, 'A')
connect(centre, '', distance, 'B')

# Low-frequency noise moves the radial boundary so overlapping rotated quads
# form an irregular plume instead of a hard star or perfect disc.
noise = expression(unreal.MaterialExpressionNoise, -1000, 420)
noise.set_editor_property('scale', 7.0)
connect(uv, '', noise, 'Coordinates')
noise_centered = expression(unreal.MaterialExpressionSubtract, -800, 420)
noise_half = expression(unreal.MaterialExpressionConstant, -1000, 620)
noise_half.set_editor_property('r', 0.5)
connect(noise, '', noise_centered, 'A')
connect(noise_half, '', noise_centered, 'B')
noise_amount = expression(unreal.MaterialExpressionMultiply, -600, 420)
noise_amount_scale = expression(unreal.MaterialExpressionConstant, -800, 620)
noise_amount_scale.set_editor_property('r', 0.15)
connect(noise_centered, '', noise_amount, 'A')
connect(noise_amount_scale, '', noise_amount, 'B')
radius = expression(unreal.MaterialExpressionAdd, -400, 350)
radius_base = expression(unreal.MaterialExpressionConstant, -600, 620)
radius_base.set_editor_property('r', 0.40)
connect(radius_base, '', radius, 'A')
connect(noise_amount, '', radius, 'B')
outer_distance = expression(unreal.MaterialExpressionSubtract, -200, 180)
connect(radius, '', outer_distance, 'A')
connect(distance, '', outer_distance, 'B')
outer_soft_edge = expression(unreal.MaterialExpressionConstant, -200, 400)
outer_soft_edge.set_editor_property('r', 0.10)
outer_edge = expression(unreal.MaterialExpressionAdd, 0, 180)
connect(outer_distance, '', outer_edge, 'A')
connect(outer_soft_edge, '', outer_edge, 'B')
outer_scale = expression(unreal.MaterialExpressionMultiply, 200, 180)
outer_scale_value = expression(unreal.MaterialExpressionConstant, 0, 400)
outer_scale_value.set_editor_property('r', 8.0)
connect(outer_edge, '', outer_scale, 'A')
connect(outer_scale_value, '', outer_scale, 'B')
outer_mask = expression(unreal.MaterialExpressionSaturate, 400, 180)
connect(outer_scale, '', outer_mask, '')
outer_falloff = expression(unreal.MaterialExpressionPower, 600, 180)
outer_falloff.set_editor_property('const_exponent', 1.35)
connect(outer_mask, '', outer_falloff, 'Base')

core_radius = expression(unreal.MaterialExpressionConstant, -200, 620)
core_radius.set_editor_property('r', 0.12)
core_distance = expression(unreal.MaterialExpressionSubtract, 0, 620)
connect(core_radius, '', core_distance, 'A')
connect(distance, '', core_distance, 'B')
core_edge = expression(unreal.MaterialExpressionConstant, 200, 620)
core_edge.set_editor_property('r', 0.05)
core_add = expression(unreal.MaterialExpressionAdd, 400, 580)
connect(core_distance, '', core_add, 'A')
connect(core_edge, '', core_add, 'B')
core_scale = expression(unreal.MaterialExpressionMultiply, 600, 580)
core_scale_value = expression(unreal.MaterialExpressionConstant, 400, 780)
core_scale_value.set_editor_property('r', 16.0)
connect(core_add, '', core_scale, 'A')
connect(core_scale_value, '', core_scale, 'B')
core_mask = expression(unreal.MaterialExpressionSaturate, 800, 580)
connect(core_scale, '', core_mask, '')

color = expression(unreal.MaterialExpressionVectorParameter, 800, 0)
color.set_editor_property('parameter_name', 'Color')
color.set_editor_property('default_value', unreal.LinearColor(1.0, 0.22, 0.025, 1.0))
intensity = expression(unreal.MaterialExpressionScalarParameter, 800, 180)
intensity.set_editor_property('parameter_name', 'Intensity')
intensity.set_editor_property('default_value', 7.0)
outer_color = expression(unreal.MaterialExpressionMultiply, 1000, 80)
connect(color, '', outer_color, 'A')
connect(intensity, '', outer_color, 'B')
outer_emission = expression(unreal.MaterialExpressionMultiply, 1200, 100)
connect(outer_color, '', outer_emission, 'A')
connect(outer_falloff, '', outer_emission, 'B')

core_color = expression(unreal.MaterialExpressionVectorParameter, 800, 400)
core_color.set_editor_property('parameter_name', 'CoreColor')
core_color.set_editor_property('default_value', unreal.LinearColor(1.0, 0.68, 0.12, 1.0))
core_strength = expression(unreal.MaterialExpressionScalarParameter, 800, 780)
core_strength.set_editor_property('parameter_name', 'CoreStrength')
core_strength.set_editor_property('default_value', 0.0)
core_color_intensity = expression(unreal.MaterialExpressionMultiply, 1000, 360)
connect(core_color, '', core_color_intensity, 'A')
connect(intensity, '', core_color_intensity, 'B')
core_gain = expression(unreal.MaterialExpressionMultiply, 1200, 380)
connect(core_color_intensity, '', core_gain, 'A')
connect(core_strength, '', core_gain, 'B')
core_emission = expression(unreal.MaterialExpressionMultiply, 1400, 420)
connect(core_gain, '', core_emission, 'A')
connect(core_mask, '', core_emission, 'B')
emission = expression(unreal.MaterialExpressionAdd, 1600, 180)
connect(outer_emission, '', emission, 'A')
connect(core_emission, '', emission, 'B')
mel.connect_material_property(emission, '', unreal.MaterialProperty.MP_EMISSIVE_COLOR)
mel.recompile_material(material)
eal.save_loaded_asset(material, only_if_is_dirty=False)
unreal.log(f'OW_MUZZLE_FLAME {asset_path}')
