"""Import Phaselock's effect textures and meshes from local UModel output and build host materials (editor Python).

AI-assisted (Claude). Run through tools/import_phaselock_fx.ps1 (it holds local/ue_run.lock). Inputs, all under the
ignored local/ tree:
  OPENWILLOW_PHASELOCK_UMODEL   ';'-separated UModel export roots (PNG textures, .mat slot lists, glTF meshes)
  OPENWILLOW_PHASELOCK_EMITTERS research/particle_system.py output (one JSON per template: emitters, materials)
  OPENWILLOW_PHASELOCK_REPORT   where to write the import report (JSON)
Writes only /Game/OpenWillow/Phaselock (recreated on every run). Nothing game-derived is tracked.

What comes from data and what is a host choice (record: docs/verification/PHASELOCK_STOCK_DATA.md, "Host presentation
pass"):
- data: which stock materials the templates use, each material's BlendMode (tagged properties in the emitter JSON;
  a MaterialInstanceConstant takes its parent's), the textures UModel lists for it, the mesh-particle meshes.
- host rules (UNVERIFIED; the cooked material graphs are stripped, so what each material does with its texture is not
  known): the colour texture is the .mat Diffuse slot, except that an emitter with a sub-image layout takes a listed
  texture whose name holds "SubUV"; a Diffuse slot holding a normal map (name ending in _Nrm or _Nrm_Tex) marks a
  screen-distortion material, which the host does not draw; a texture named *_Mirror is a quarter image mirrored to
  the full quad (UV 1 - |2uv - 1|); a modulate material without a texture uses a radial falloff; translucent materials
  whose PNG has no alpha channel use the texture's brightest channel as opacity; a second *_Mirror texture in the slot
  list multiplies colour and opacity as a soft mask over the quad.
- host materials: additive = texture x colour x alpha; translucent = texture x colour with opacity = mask x alpha;
  modulate = lerp(1, colour, mask) with mask = the texture's brightest channel (UE3 modulate ignores opacity);
  dynamic parameter 0 pans U on the mesh-particle material (the energy ribbons) and is unused elsewhere.
"""
import json
import os
from pathlib import Path
import unreal

destination = '/Game/OpenWillow/Phaselock'
roots = [Path(p).resolve() for p in os.environ['OPENWILLOW_PHASELOCK_UMODEL'].split(';') if p]
emitter_dir = Path(os.environ['OPENWILLOW_PHASELOCK_EMITTERS']).resolve()
report_path = Path(os.environ.get('OPENWILLOW_PHASELOCK_REPORT', emitter_dir.parent / 'fx_import.json'))
tools = unreal.AssetToolsHelpers.get_asset_tools()
mel = unreal.MaterialEditingLibrary
eal = unreal.EditorAssetLibrary
report = {'materials': {}, 'textures': {}, 'meshes': {}, 'skipped': {}}


def log(text):
    unreal.log('OW_PLFX ' + text)


def find(name, suffix):
    hits = sorted({p for root in roots for p in root.rglob(name + suffix)})
    return hits[0] if hits else None


def imported(path, folder, expected):
    task = unreal.AssetImportTask()
    task.filename = str(path)
    task.destination_path = f'{destination}/{folder}'
    task.automated = True
    task.replace_existing = True
    task.save = True
    tools.import_asset_tasks([task])
    objects = [o for o in task.get_objects() if isinstance(o, expected)]
    if not objects:
        raise RuntimeError(f'no {expected.__name__} imported from {path}')
    return objects[0]


def png_has_alpha(path):
    # PNG IHDR colour type at byte 25: 4 = grey+alpha, 6 = RGBA.
    with open(path, 'rb') as f:
        header = f.read(26)
    return len(header) == 26 and header[25] in (4, 6)


textures = {}


def texture(name):
    if name in textures:
        return textures[name]
    path = find(name, '.png')
    if not path:
        textures[name] = None
        return None
    asset = imported(path, 'Textures', unreal.Texture2D)
    asset.set_editor_property('srgb', True)
    try:
        asset.set_editor_property('lod_group', unreal.TextureGroup.TEXTUREGROUP_EFFECTS)
    except Exception as error:  # enum spelling differs between engine versions; the group only affects streaming
        log(f'lod group not set on {name}: {error}')
    eal.save_loaded_asset(asset, only_if_is_dirty=False)
    textures[name] = (asset, png_has_alpha(path))
    report['textures'][name] = {'source': str(path), 'alpha': textures[name][1]}
    return textures[name]


# --- host parent materials -------------------------------------------------------------------------------------------

def build(name, blend, domain_fn=None):
    material = tools.create_asset(name, f'{destination}/Materials', unreal.Material, unreal.MaterialFactoryNew())
    material.set_editor_property('blend_mode', blend)
    material.set_editor_property('shading_model', unreal.MaterialShadingModel.MSM_UNLIT)
    material.set_editor_property('two_sided', True)
    nodes = []

    def node(cls, x, y, **props):
        expression = mel.create_material_expression(material, cls, x, y)
        for key, value in props.items():
            expression.set_editor_property(key, value)
        nodes.append(expression)
        return expression

    def op(cls, a, a_pin, b, b_pin, x, y):
        expression = node(cls, x, y)
        mel.connect_material_expressions(a, a_pin, expression, 'A')
        mel.connect_material_expressions(b, b_pin, expression, 'B')
        return expression

    def mask(source, pin, channels, x, y):
        expression = node(unreal.MaterialExpressionComponentMask, x, y, r='R' in channels, g='G' in channels,
                          b='B' in channels, a='A' in channels)
        mel.connect_material_expressions(source, pin, expression, '')
        return expression

    def scalar(param, default, x, y):
        return node(unreal.MaterialExpressionScalarParameter, x, y, parameter_name=param, default_value=default)

    def const(value, x, y):
        return node(unreal.MaterialExpressionConstant, x, y, r=value)

    M, A, S, D = (unreal.MaterialExpressionMultiply, unreal.MaterialExpressionAdd, unreal.MaterialExpressionSubtract,
                  unreal.MaterialExpressionDivide)
    color = node(unreal.MaterialExpressionVectorParameter, -400, -400, parameter_name='Color',
                 default_value=unreal.LinearColor(1, 1, 1, 1))
    color_rgb = mask(color, '', 'RGB', -200, -400)
    color_a = color  # used through its 'A' pin
    if domain_fn == 'screen':
        uv = node(unreal.MaterialExpressionScreenPosition, -2200, 0)
        uv_pin = 'ViewportUV'
    else:
        uv = node(unreal.MaterialExpressionTextureCoordinate, -2200, 0)
        uv_pin = ''
    # Mirror: a quarter image spread over the quad.
    doubled = op(M, uv, uv_pin, const(2.0, -2200, 150), '', -2000, 100)
    centred = op(S, doubled, '', const(1.0, -2000, 250), '', -1850, 100)
    absolute = node(unreal.MaterialExpressionAbs, -1700, 100)
    mel.connect_material_expressions(centred, '', absolute, '')
    mirrored = node(unreal.MaterialExpressionOneMinus, -1550, 100)
    mel.connect_material_expressions(absolute, '', mirrored, '')
    uv1 = node(unreal.MaterialExpressionLinearInterpolate, -1400, 0)
    mel.connect_material_expressions(uv, uv_pin, uv1, 'A')
    mel.connect_material_expressions(mirrored, '', uv1, 'B')
    mel.connect_material_expressions(scalar('Mirror', 0.0, -1550, 250), '', uv1, 'Alpha')
    # Sub-image: (uv + (column, row)) / (columns, rows) with frame index SubUV.B.
    sub = node(unreal.MaterialExpressionVectorParameter, -1550, 400, parameter_name='SubUV',
               default_value=unreal.LinearColor(1, 1, 0, 0))
    column = op(unreal.MaterialExpressionFmod, sub, 'B', sub, 'R', -1250, 450)
    row_f = op(D, sub, 'B', sub, 'R', -1250, 550)
    row = node(unreal.MaterialExpressionFloor, -1100, 550)
    mel.connect_material_expressions(row_f, '', row, '')
    offset = op(unreal.MaterialExpressionAppendVector, column, '', row, '', -950, 450)
    size = op(unreal.MaterialExpressionAppendVector, sub, 'R', sub, 'G', -950, 550)
    shifted = op(A, uv1, '', offset, '', -800, 100)
    uv2 = op(D, shifted, '', size, '', -650, 100)
    # Dynamic parameter 0 x PanScale pans U (mesh ribbons).
    pan = op(M, scalar('DynParam', 0.0, -950, 700), '', scalar('PanScale', 0.0, -950, 780), '', -800, 700)
    pan2 = op(unreal.MaterialExpressionAppendVector, pan, '', const(0.0, -800, 800), '', -650, 700)
    uv3 = op(A, uv2, '', pan2, '', -500, 100)
    tex = node(unreal.MaterialExpressionTextureSampleParameter2D, -350, 100, parameter_name='Tex',
               texture=unreal.load_asset('/Engine/EngineResources/WhiteSquareTexture'))
    mel.connect_material_expressions(uv3, '', tex, 'UVs')
    # Radial falloff for textureless modulate materials.
    distance = op(unreal.MaterialExpressionDistance, uv, uv_pin,
                  node(unreal.MaterialExpressionConstant2Vector, -1400, 900, r=0.5, g=0.5), '', -1250, 900)
    radius = op(M, distance, '', const(2.0, -1250, 1000), '', -1100, 900)
    radial = node(unreal.MaterialExpressionOneMinus, -950, 900)
    mel.connect_material_expressions(radius, '', radial, '')
    radial_c = node(unreal.MaterialExpressionSaturate, -800, 900)
    mel.connect_material_expressions(radial, '', radial_c, '')
    use_radial = scalar('Radial', 0.0, -350, 400)
    rgb = node(unreal.MaterialExpressionLinearInterpolate, -150, 100)
    mel.connect_material_expressions(tex, 'RGB', rgb, 'A')
    mel.connect_material_expressions(radial_c, '', rgb, 'B')
    mel.connect_material_expressions(use_radial, '', rgb, 'Alpha')
    alpha = node(unreal.MaterialExpressionLinearInterpolate, -150, 250)
    mel.connect_material_expressions(tex, 'A', alpha, 'A')
    mel.connect_material_expressions(radial_c, '', alpha, 'B')
    mel.connect_material_expressions(use_radial, '', alpha, 'Alpha')
    # Optional soft mask: a second, mirrored texture the material lists (host reading of the .mat slot list).
    mask_tex = node(unreal.MaterialExpressionTextureSampleParameter2D, -350, 1100, parameter_name='Mask',
                    texture=unreal.load_asset('/Engine/EngineResources/WhiteSquareTexture'))
    mel.connect_material_expressions(mirrored, '', mask_tex, 'UVs')
    mask_weight = node(unreal.MaterialExpressionLinearInterpolate, -150, 1100)
    mel.connect_material_expressions(const(1.0, -350, 1250), '', mask_weight, 'A')
    mel.connect_material_expressions(mask_tex, 'R', mask_weight, 'B')
    mel.connect_material_expressions(scalar('UseMask', 0.0, -350, 1350), '', mask_weight, 'Alpha')
    rgb = op(M, rgb, '', mask_weight, '', 0, 100)
    alpha = op(M, alpha, '', mask_weight, '', 0, 250)
    brightest = op(unreal.MaterialExpressionMax, op(unreal.MaterialExpressionMax, mask(rgb, '', 'R', 0, 300), '',
                                                    mask(rgb, '', 'G', 0, 380), '', 150, 300), '',
                   mask(rgb, '', 'B', 0, 460), '', 300, 300)
    if blend == unreal.BlendMode.BLEND_ADDITIVE:
        emissive = op(M, op(M, rgb, '', color_rgb, '', 100, 0), '', color_a, 'A', 250, 0)
    elif blend == unreal.BlendMode.BLEND_MODULATE:
        emissive = node(unreal.MaterialExpressionLinearInterpolate, 250, 0)
        mel.connect_material_expressions(const(1.0, 100, -100), '', emissive, 'A')
        mel.connect_material_expressions(color_rgb, '', emissive, 'B')
        mel.connect_material_expressions(brightest, '', emissive, 'Alpha')
    else:
        source = brightest if domain_fn == 'screen' else rgb
        emissive = op(M, source, '', color_rgb, '', 250, 0)
        coverage = brightest
        if domain_fn != 'screen':
            coverage = node(unreal.MaterialExpressionLinearInterpolate, 400, 300)
            mel.connect_material_expressions(alpha, '', coverage, 'A')
            mel.connect_material_expressions(brightest, '', coverage, 'B')
            mel.connect_material_expressions(scalar('LumAlpha', 0.0, 250, 400), '', coverage, 'Alpha')
        opacity = node(unreal.MaterialExpressionSaturate, 700, 300)
        mel.connect_material_expressions(op(M, coverage, '', color_a, 'A', 550, 300), '', opacity, '')
        mel.connect_material_property(opacity, '', unreal.MaterialProperty.MP_OPACITY)
    mel.connect_material_property(emissive, '', unreal.MaterialProperty.MP_EMISSIVE_COLOR)
    if domain_fn == 'screen':
        material.set_editor_property('disable_depth_test', True)
    mel.recompile_material(material)
    eal.save_loaded_asset(material, only_if_is_dirty=False)
    return material


def tattoo_material():
    """Additive overlay for Maya's first-person arms: Masks.B on the left half of the mask texture (uv x (0.5, 1)),
    times GlowColor x Enable. The channel/half was chosen by inspecting the exported mask (the tattoo shapes sit in B of
    the left half); Master_Player's graph is stripped, so this is a host reading (UNVERIFIED)."""
    material = tools.create_asset('M_OW_PlTattooGlow', f'{destination}/Materials', unreal.Material,
                                  unreal.MaterialFactoryNew())
    material.set_editor_property('blend_mode', unreal.BlendMode.BLEND_ADDITIVE)
    material.set_editor_property('shading_model', unreal.MaterialShadingModel.MSM_UNLIT)
    material.set_editor_property('used_with_skeletal_mesh', True)
    uv = mel.create_material_expression(material, unreal.MaterialExpressionTextureCoordinate, -900, 0)
    uv.set_editor_property('u_tiling', 0.5)
    uv.set_editor_property('v_tiling', 1.0)
    masks = mel.create_material_expression(material, unreal.MaterialExpressionTextureSampleParameter2D, -700, 0)
    masks.set_editor_property('parameter_name', 'Masks')
    masks.set_editor_property('sampler_type', unreal.MaterialSamplerType.SAMPLERTYPE_LINEAR_COLOR)
    masks.set_editor_property('texture', unreal.load_asset('/Engine/EngineResources/Black'))
    mel.connect_material_expressions(uv, '', masks, 'UVs')
    colour = mel.create_material_expression(material, unreal.MaterialExpressionVectorParameter, -700, 300)
    colour.set_editor_property('parameter_name', 'GlowColor')
    enable = mel.create_material_expression(material, unreal.MaterialExpressionScalarParameter, -700, 450)
    enable.set_editor_property('parameter_name', 'Enable')
    a = mel.create_material_expression(material, unreal.MaterialExpressionMultiply, -400, 100)
    mel.connect_material_expressions(masks, 'B', a, 'A')
    mel.connect_material_expressions(colour, '', a, 'B')
    b = mel.create_material_expression(material, unreal.MaterialExpressionMultiply, -200, 200)
    mel.connect_material_expressions(a, '', b, 'A')
    mel.connect_material_expressions(enable, '', b, 'B')
    rgb = mel.create_material_expression(material, unreal.MaterialExpressionComponentMask, -50, 200)
    for c in 'rgb':
        rgb.set_editor_property(c, True)
    mel.connect_material_expressions(b, '', rgb, '')
    mel.connect_material_property(rgb, '', unreal.MaterialProperty.MP_EMISSIVE_COLOR)
    mel.recompile_material(material)
    eal.save_loaded_asset(material, only_if_is_dirty=False)
    return material


# --- run -------------------------------------------------------------------------------------------------------------

if eal.does_directory_exist(destination):
    eal.delete_directory(destination)
parents = {
    'BLEND_Additive': build('M_OW_PlAdditive', unreal.BlendMode.BLEND_ADDITIVE),
    'BLEND_Translucent': build('M_OW_PlTranslucent', unreal.BlendMode.BLEND_TRANSLUCENT),
    'BLEND_Modulate': build('M_OW_PlModulate', unreal.BlendMode.BLEND_MODULATE),
}
screen_parent = build('M_OW_PlScreen', unreal.BlendMode.BLEND_TRANSLUCENT, 'screen')
tattoo_material()

# Stock materials and meshes named by the decoded templates, with the emitter layouts that use them.
uses, blend, meshes = {}, {}, set()
for template in sorted(emitter_dir.glob('Part_*.json')):
    data = json.loads(template.read_text(encoding='utf-8'))
    materials = data.get('materials') or {}

    def blend_of(path, depth=0):
        entry = materials.get(path) or {}
        props = entry.get('properties') or {}
        if props.get('BlendMode') or depth > 4:
            return props.get('BlendMode')
        return blend_of(props.get('Parent'), depth + 1) if props.get('Parent') else None

    screen_templates = {'Part_PhaseLockScreenEffect'}
    for emitter in data.get('digest') or []:
        req = emitter['required']
        path = req.get('Material')
        if not path:
            continue
        name = path.rsplit('.', 1)[-1]
        layout = uses.setdefault(name, {'subimages': False, 'mesh': False, 'screen': False, 'templates': set()})
        layout['subimages'] |= (req.get('SubImages_Horizontal', 1) or 1) * (req.get('SubImages_Vertical', 1) or 1) > 1
        layout['screen'] |= template.stem in screen_templates
        layout['templates'].add(template.stem)
        mesh = ((emitter.get('type_data') or {}).get('properties') or {}).get('Mesh')
        if mesh:
            layout['mesh'] = True
            meshes.add(mesh.rsplit('.', 1)[-1])
        blend[name] = blend_of(path)

for name in sorted(uses):
    layout = uses[name]
    slots = {}
    mat_file = find(name, '.mat')
    if mat_file:
        for line in mat_file.read_text(encoding='utf-8', errors='replace').splitlines():
            if '=' in line:
                key, value = line.split('=', 1)
                slots[key.strip()] = value.strip()
    chosen = slots.get('Diffuse')
    if layout['subimages']:
        chosen = next((v for v in slots.values() if 'SubUV' in v), chosen)
    mode = blend.get(name)
    entry = {'blend': mode, 'slots': slots, 'templates': sorted(layout['templates']), 'mat': str(mat_file) if mat_file else None}
    if mode not in parents:
        report['skipped'][name] = {**entry, 'reason': f'blend mode {mode} not hosted'}
        continue
    if chosen and (chosen.endswith('_Nrm') or chosen.endswith('_Nrm_Tex')):
        report['skipped'][name] = {**entry, 'reason': 'screen-distortion material (normal map in the Diffuse slot): not drawn'}
        continue
    tex = texture(chosen) if chosen else None
    if chosen and not tex:
        report['skipped'][name] = {**entry, 'reason': f'texture {chosen} not in the UModel output'}
        continue
    if not tex and mode != 'BLEND_Modulate':
        report['skipped'][name] = {**entry, 'reason': 'no texture listed (UModel skips materials without texture parameters)'}
        continue
    parent = screen_parent if layout['screen'] else parents[mode]
    instance = tools.create_asset(f'MI_{name}', f'{destination}/Materials', unreal.MaterialInstanceConstant,
                                  unreal.MaterialInstanceConstantFactoryNew())
    mel.set_material_instance_parent(instance, parent)
    flags = {}
    # A *_Mirror texture listed besides the colour texture is read as a soft mask over the quad (host reading).
    mask_name = next((v for k, v in slots.items() if v != chosen and v.endswith('_Mirror')), None)
    mask = texture(mask_name) if mask_name else None
    if mask:
        mel.set_material_instance_texture_parameter_value(instance, 'Mask', mask[0])
        flags['UseMask'] = 1.0
        entry['mask'] = mask_name
    if tex:
        mel.set_material_instance_texture_parameter_value(instance, 'Tex', tex[0])
        flags['Mirror'] = 1.0 if chosen.endswith('_Mirror') else 0.0
        flags['LumAlpha'] = 0.0 if tex[1] else 1.0
    else:
        flags['Radial'] = 1.0
    if layout['mesh']:
        flags['PanScale'] = 1.0
    for key, value in flags.items():
        mel.set_material_instance_scalar_parameter_value(instance, key, value)
    eal.save_loaded_asset(instance, only_if_is_dirty=False)
    report['materials'][name] = {**entry, 'instance': instance.get_path_name(), 'parent': parent.get_name(),
                                 'texture': chosen, 'flags': flags}
    log(f'{name}: {mode} -> {parent.get_name()} tex={chosen} {flags}')

for name in sorted(meshes):
    path = find(name, '.gltf')
    if not path:
        report['skipped'][name] = {'reason': 'mesh not in the UModel output'}
        continue
    mesh = imported(path, 'Meshes', unreal.StaticMesh)
    # The glTF importer nests the mesh (Meshes/<name>/StaticMeshes/<name>); the host loads Meshes/<name>.
    target = f'{destination}/Meshes/{name}.{name}'
    if mesh.get_path_name() != target:
        if not eal.rename_asset(mesh.get_path_name(), target):
            raise RuntimeError(f'could not move {mesh.get_path_name()} to {target}')
        mesh = unreal.load_asset(target)
    bounds = mesh.get_bounds()
    report['meshes'][name] = {'source': str(path), 'extent': [bounds.box_extent.x, bounds.box_extent.y, bounds.box_extent.z]}
    eal.save_loaded_asset(mesh, only_if_is_dirty=False)
    log(f'mesh {name}: extent {report["meshes"][name]["extent"]}')

for name, entry in report['skipped'].items():
    log(f'skipped {name}: {entry["reason"]}')
report_path.parent.mkdir(parents=True, exist_ok=True)
report_path.write_text(json.dumps(report, indent=1, default=list) + '\n', encoding='utf-8')
log(f'done: {len(report["materials"])} material instances, {len(report["textures"])} textures, '
    f'{len(report["meshes"])} meshes, {len(report["skipped"])} skipped; report {report_path}')
