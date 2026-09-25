"""Import a local Infinity pistol assembly and its approximate gun material.

OPENWILLOW_PISTOL_GLTF is a glTF produced by tools/filter_gestalt_gltf.py from
the player's UModel export of Startup.upk's pistol gestalt, keeping only the
chosen part fragments. OPENWILLOW_INFINITY_TEXTURES is UModel's PNG export of
Mati_VladofLegendaryPistol_Infinity's textures.

M_OW_InfinityApprox is NOT Gearbox's cooked Master_Gun shader, whose graph is
stripped. It is an UNVERIFIED approximation: p_Masks R/G/B select the A/B/C
paint regions, and one p_Diffuse channel picks between each region's
shadow/midtone/highlight colors from the MIC. p_Diffuse's channels hold
separate atlases; its name (LauncherShotgunPistol) suggests R/G/B in that
order, so the pistol detail is read from blue. That order is an inference.
M_OW_FxAdditive (tracers, flashes, Phaselock sphere) and M_OW_BulletHole
(impact decal) are host-only effect materials.
Only generated UE assets under /Game/OpenWillow/Weapons/InfinityProxy are touched.
"""
import os
from pathlib import Path
import unreal

source = Path(os.environ['OPENWILLOW_PISTOL_GLTF']).resolve()
if not source.is_file() or source.suffix.lower() != '.gltf':
    raise RuntimeError(f'Expected a local glTF: {source}')
textures = Path(os.environ['OPENWILLOW_INFINITY_TEXTURES']).resolve()
destination = '/Game/OpenWillow/Weapons/InfinityProxy'
tools = unreal.AssetToolsHelpers.get_asset_tools()
eal = unreal.EditorAssetLibrary
mel = unreal.MaterialEditingLibrary
if eal.does_directory_exist(destination):
    eal.delete_directory(destination)


def import_file(path):
    task = unreal.AssetImportTask()
    task.filename = str(path)
    task.destination_path = destination
    task.automated = True
    task.replace_existing = True
    task.save = False
    tools.import_asset_tasks([task])
    return task.get_objects()


def import_texture(name, normal=False):
    path = textures / f'{name}.png'
    if not path.is_file():
        raise RuntimeError(f'Missing UModel texture export: {path}')
    texture = next(a for a in import_file(path) if isinstance(a, unreal.Texture2D))
    # Masks and detail are data, not color; the normal map is two-channel.
    texture.set_editor_property('srgb', False)
    if normal:
        texture.set_editor_property('compression_settings', unreal.TextureCompressionSettings.TC_NORMALMAP)
    eal.save_loaded_asset(texture, only_if_is_dirty=False)
    return texture


meshes = [asset for asset in import_file(source) if isinstance(asset, unreal.SkeletalMesh)]
if len(meshes) != 1:
    raise RuntimeError(f'Expected one pistol skeletal mesh, found {len(meshes)}')
mesh = meshes[0]
masks = import_texture('Weap_Pistols_Comp')
detail = import_texture('Weap_LauncherShotgunPistol_Comp')
normal = import_texture('Weap_Pistols_Nrm', normal=True)

# Mati_VladofLegendaryPistol_Infinity VectorParameterValues (UModel props).
colors = {
    'A': [(0.198099, 0.250253, 0.325955), (0.0177393, 0.0175206, 0.0662959), (0.0880097, 0.0630298, 0.204412)],
    'B': [(0.72747, 0.829674, 1.0), (1.08435, 1.22399, 1.65968), (1.74991, 2.06869, 3.23188)],
    'C': [(0.424271, 0.763497, 1.37119), (0.0169881, 0.0169881, 0.0657539), (0.0869013, 0.0619075, 0.201096)],
}
material = tools.create_asset('M_OW_InfinityApprox', destination, unreal.Material, unreal.MaterialFactoryNew())


def node(kind, x, y):
    return mel.create_material_expression(material, kind, x, y)


def sampler(texture, y, sample_type):
    expression = node(unreal.MaterialExpressionTextureSample, -1400, y)
    expression.set_editor_property('texture', texture)
    expression.set_editor_property('sampler_type', sample_type)
    return expression


def constant3(rgb, x, y):
    expression = node(unreal.MaterialExpressionConstant3Vector, x, y)
    expression.set_editor_property('constant', unreal.LinearColor(*rgb, 1.0))
    return expression


def binary(kind, a, a_pin, b, b_pin, x, y):
    expression = node(kind, x, y)
    mel.connect_material_expressions(a, a_pin, expression, 'A')
    mel.connect_material_expressions(b, b_pin, expression, 'B')
    return expression


def lerp(a, b, alpha, alpha_pin, x, y):
    expression = node(unreal.MaterialExpressionLinearInterpolate, x, y)
    mel.connect_material_expressions(a, '', expression, 'A')
    mel.connect_material_expressions(b, '', expression, 'B')
    mel.connect_material_expressions(alpha, alpha_pin, expression, 'Alpha')
    return expression


mask_sample = sampler(masks, 0, unreal.MaterialSamplerType.SAMPLERTYPE_LINEAR_COLOR)
detail_sample = sampler(detail, 400, unreal.MaterialSamplerType.SAMPLERTYPE_LINEAR_COLOR)
normal_sample = sampler(normal, 800, unreal.MaterialSamplerType.SAMPLERTYPE_NORMAL)
two = node(unreal.MaterialExpressionConstant, -1200, 600)
two.set_editor_property('r', 2.0)
one = node(unreal.MaterialExpressionConstant, -1200, 700)
one.set_editor_property('r', 1.0)
doubled = binary(unreal.MaterialExpressionMultiply, detail_sample, 'B', two, '', -1100, 400)
low = node(unreal.MaterialExpressionSaturate, -1000, 380)
mel.connect_material_expressions(doubled, '', low, '')
high_raw = binary(unreal.MaterialExpressionSubtract, doubled, '', one, '', -1000, 460)
high = node(unreal.MaterialExpressionSaturate, -900, 460)
mel.connect_material_expressions(high_raw, '', high, '')
total = None
for row, (region, pin) in enumerate((('A', 'R'), ('B', 'G'), ('C', 'B'))):
    y = -400 + 250 * row
    shadow, mid, hilight = (constant3(c, -800, y + 60 * i) for i, c in enumerate(colors[region]))
    tone = lerp(lerp(shadow, mid, low, '', -600, y), hilight, high, '', -450, y)
    weighted = binary(unreal.MaterialExpressionMultiply, tone, '', mask_sample, pin, -300, y)
    total = weighted if total is None else binary(unreal.MaterialExpressionAdd, total, '', weighted, '', -150, y)
base = node(unreal.MaterialExpressionSaturate, 0, 0)
mel.connect_material_expressions(total, '', base, '')
mel.connect_material_property(base, '', unreal.MaterialProperty.MP_BASE_COLOR)
mel.connect_material_property(normal_sample, 'RGB', unreal.MaterialProperty.MP_NORMAL)
metallic = node(unreal.MaterialExpressionConstant, 0, 200)
metallic.set_editor_property('r', 0.55)
mel.connect_material_property(metallic, '', unreal.MaterialProperty.MP_METALLIC)
roughness = node(unreal.MaterialExpressionConstant, 0, 280)
roughness.set_editor_property('r', 0.38)
mel.connect_material_property(roughness, '', unreal.MaterialProperty.MP_ROUGHNESS)
material.set_editor_property('used_with_skeletal_mesh', True)
mel.recompile_material(material)

# Host effect material: unlit additive, color/intensity set per instance.
fx = tools.create_asset('M_OW_FxAdditive', destination, unreal.Material, unreal.MaterialFactoryNew())
fx.set_editor_property('blend_mode', unreal.BlendMode.BLEND_ADDITIVE)
fx.set_editor_property('shading_model', unreal.MaterialShadingModel.MSM_UNLIT)
fx.set_editor_property('two_sided', True)
fx_color = mel.create_material_expression(fx, unreal.MaterialExpressionVectorParameter, -600, 0)
fx_color.set_editor_property('parameter_name', 'Color')
fx_color.set_editor_property('default_value', unreal.LinearColor(1.0, 0.7, 0.3, 1.0))
fx_intensity = mel.create_material_expression(fx, unreal.MaterialExpressionScalarParameter, -600, 200)
fx_intensity.set_editor_property('parameter_name', 'Intensity')
fx_intensity.set_editor_property('default_value', 4.0)
# A fresnel rim keeps spheres readable as shells instead of flat discs.
fresnel = mel.create_material_expression(fx, unreal.MaterialExpressionFresnel, -600, 350)
rim = mel.create_material_expression(fx, unreal.MaterialExpressionScalarParameter, -600, 500)
rim.set_editor_property('parameter_name', 'Rim')
rim.set_editor_property('default_value', 0.0)
rim_weight = mel.create_material_expression(fx, unreal.MaterialExpressionLinearInterpolate, -400, 400)
fx_one = mel.create_material_expression(fx, unreal.MaterialExpressionConstant, -600, 600)
fx_one.set_editor_property('r', 1.0)
mel.connect_material_expressions(fx_one, '', rim_weight, 'A')
mel.connect_material_expressions(fresnel, '', rim_weight, 'B')
mel.connect_material_expressions(rim, '', rim_weight, 'Alpha')
scaled = mel.create_material_expression(fx, unreal.MaterialExpressionMultiply, -300, 100)
mel.connect_material_expressions(fx_color, '', scaled, 'A')
mel.connect_material_expressions(fx_intensity, '', scaled, 'B')
emissive = mel.create_material_expression(fx, unreal.MaterialExpressionMultiply, -150, 200)
mel.connect_material_expressions(scaled, '', emissive, 'A')
mel.connect_material_expressions(rim_weight, '', emissive, 'B')
mel.connect_material_property(emissive, '', unreal.MaterialProperty.MP_EMISSIVE_COLOR)
fx.set_editor_property('used_with_skeletal_mesh', True)
mel.recompile_material(fx)

# Host impact decal: a dark, soft-edged round mark.
hole = tools.create_asset('M_OW_BulletHole', destination, unreal.Material, unreal.MaterialFactoryNew())
hole.set_editor_property('material_domain', unreal.MaterialDomain.MD_DEFERRED_DECAL)
hole.set_editor_property('blend_mode', unreal.BlendMode.BLEND_TRANSLUCENT)
uv = mel.create_material_expression(hole, unreal.MaterialExpressionTextureCoordinate, -800, 200)
centre = mel.create_material_expression(hole, unreal.MaterialExpressionConstant2Vector, -800, 300)
centre.set_editor_property('r', 0.5)
centre.set_editor_property('g', 0.5)
distance = mel.create_material_expression(hole, unreal.MaterialExpressionDistance, -600, 250)
mel.connect_material_expressions(uv, '', distance, 'A')
mel.connect_material_expressions(centre, '', distance, 'B')
edge = mel.create_material_expression(hole, unreal.MaterialExpressionConstant, -600, 350)
edge.set_editor_property('r', 2.0)
scaled_distance = mel.create_material_expression(hole, unreal.MaterialExpressionMultiply, -450, 250)
mel.connect_material_expressions(distance, '', scaled_distance, 'A')
mel.connect_material_expressions(edge, '', scaled_distance, 'B')
inverted = mel.create_material_expression(hole, unreal.MaterialExpressionOneMinus, -300, 250)
mel.connect_material_expressions(scaled_distance, '', inverted, '')
clamped = mel.create_material_expression(hole, unreal.MaterialExpressionSaturate, -200, 250)
mel.connect_material_expressions(inverted, '', clamped, '')
falloff = mel.create_material_expression(hole, unreal.MaterialExpressionPower, -100, 250)
mel.connect_material_expressions(clamped, '', falloff, 'Base')
falloff.set_editor_property('const_exponent', 1.5)
mel.connect_material_property(falloff, '', unreal.MaterialProperty.MP_OPACITY)
soot = mel.create_material_expression(hole, unreal.MaterialExpressionConstant3Vector, -200, 0)
soot.set_editor_property('constant', unreal.LinearColor(0.015, 0.013, 0.012, 1.0))
mel.connect_material_property(soot, '', unreal.MaterialProperty.MP_BASE_COLOR)
mel.recompile_material(hole)
eal.save_loaded_asset(hole, only_if_is_dirty=False)

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
eal.save_loaded_asset(fx, only_if_is_dirty=False)
box = mesh.get_bounds().box_extent
origin = mesh.get_bounds().origin
unreal.log(f'OW_INFINITY_PROXY path={mesh.get_path_name()} source={source.name} '
           f'material_slots={len(slots)} origin={origin.x:.1f},{origin.y:.1f},{origin.z:.1f} '
           f'extent={box.x:.1f},{box.y:.1f},{box.z:.1f}')
