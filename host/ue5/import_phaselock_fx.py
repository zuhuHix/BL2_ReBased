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
  modulate = lerp(1, colour, mask) with mask = the texture's brightest channel (UE3 modulate ignores opacity), with
  per-material readings of alpha and darkening (MODULATE_READINGS); the darkening ones are drawn as translucent black
  (DARKEN_AS_TRANSLUCENT) so that they darken earlier emitters; the screen particle = a modulate of the view by the
  colour's hue, weighted by alpha and the mask's green streaks; dynamic parameter 0 pans U on the mesh-particle
  material (the energy ribbons) and is unused elsewhere (the bubble's SphereCollapse included); every parent renders
  in the before-DOF translucency pass; additive and translucent layers cap each channel at the LDR clip.
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

# What each stock modulate material is read to do with the particle colour and alpha (host readings, UNVERIFIED; the
# graphs are stripped). Not listed: lerp(1, colour, mask), alpha ignored.
MODULATE_READINGS = {
    # Its emitters author alpha curves (the end template's Brighten ramps from 0), so alpha weights the blend. Its
    # colours ((3, 6, 12) on the cast flashes, (0.4, 16, 30) on the release) are read as a brightness-keeping tint
    # (HueOnly). A plain multiply turns the view white even under UE3's per-channel clip, while the 2026-10-02 game
    # frames show cobalt flashes and a cyan release ring.
    'Mat_SirenGlowMOD': {'UseAlpha': 1.0, 'HueOnly': 1.0},
    # Its emitter in the end template is named BlackSpikeys while its colour is 5: read as darkening by
    # mask x alpha x colour rather than brightening five times.
    'Mat_SirenOrbEnergySpikesMOD': {'UseAlpha': 1.0, 'ColourGain': 1.0, 'Darken': 1.0},
    # Darkening by mask x (1 - alpha), colour unused. This one reading fits its three emitters: the loop and end
    # bubbles (alpha scaled to 0: a dark core) and the hand orb's (alpha 0 at spawn, 1 by 0.2 of its life), which
    # gives the large dark blob the 2026-10-02 game frames show around the raised hand at +0.27 s, gone by +0.45 s.
    # MaxWeight 0.9: in the matched-distance game capture (2026-10-03, an Adult Bullymong at 650 uu) the held target shows
    # at about a third of its brightness in the 8-bit frames, which UE5's linear blending reaches at 0.9 darkness
    # ((1/3)^2.2 is about 0.09). MaxWeightFar 0.96 = 1 - 0.1 / 2.5 for the interior, where the host's additive bubble
    # layers sum to about 2.5 per channel before the black (see build()). Calibrated against that one capture
    # (UNVERIFIED elsewhere).
    # MaxWeightFar 1 (round 5): over the bubble's additive layers the darkness is full and the bubble emitters blend
    # toward a calibrated deep blue-violet instead of black (per-emitter DarkColor in OpenWillowPhaselockFx.cpp).
    'Mat_SirenOrbBlackMOD': {'UseAlpha': 1.0, 'AlphaInvert': 1.0, 'Darken': 1.0, 'MaxWeight': 0.9, 'MaxWeightFar': 1.0},
    'Mat_SirenOrbBlackMOD_NoBias': {'UseAlpha': 1.0, 'AlphaInvert': 1.0, 'Darken': 1.0, 'MaxWeight': 0.9, 'MaxWeightFar': 1.0},
}
# Darkening modulates drawn with the 'darken' parent (translucent black, see build()).
DARKEN_AS_TRANSLUCENT = {'Mat_SirenOrbBlackMOD', 'Mat_SirenOrbBlackMOD_NoBias', 'Mat_SirenOrbEnergySpikesMOD'}
# The black orb has no texture parameter (UModel exports none and the graph is stripped; its EmissiveColor is unconnected,
# so it darkens toward black). Host stand-in: a disc fully dark out to 0.6 of the quad's radius, then fading to its edge.
# Measured against the loop's other sprites at the same draw scale, the core sprite's bright magenta band
# (EnergyOrbCoreColor_Dif_Tex, 0.4-0.6 of its larger quad) falls at 0.46-0.70 of this quad's radius and the bubble
# texture's thin rim at about 0.70. The disc hides the band and leaves the rim at about a quarter of its brightness: a
# thin dim purple edge around a near-black core, as in the 2026-10-02 game frames f034/f066. Round 3 used 2 (dark to 0.5),
# which left a thick bright magenta ring.
RADIAL_SHARPNESS = {'Mat_SirenOrbBlackMOD': 2.5, 'Mat_SirenOrbBlackMOD_NoBias': 2.5}


# Sampler state of the stock textures, read from the Texture2D exports (AddressX/AddressY, SRGB, CompressionSettings; an
# absent address property is TA_Wrap, an absent SRGB is true). Only the textures that differ from the default are listed.
# Nrm_Test and the screen-distortion map are signed normal maps in the game (the host stores them as raw colour and the
# exact materials expand xy to -1..1 themselves).
TEXTURE_OPTIONS = {
    'PhaseLockBubble_Dif_Tex': {'address': 'CLAMP'},
    'EnergyRibbon_Dif_Tex': {'address': 'CLAMP'},
    'EnergySwirl_Dif_Tex': {'address': 'CLAMP'},
    'Square_Mask_Dif': {'address': 'CLAMP'},
    'Tex_Lens_Flare_Wide_Prime': {'address': 'CLAMP'},
    'EnergyOrbCenter_Dif_Mirror': {'address': 'MIRROR'},
    'EnergyOrbSoftMod_Dif_Mirror': {'address': 'MIRROR'},
    'Nrm_Test': {'srgb': False},
    'EnergyOrbScreenUVDistortion_Nrm_Tex': {'srgb': False},
}


def texture(name):
    if name in textures:
        return textures[name]
    path = find(name, '.png')
    if not path:
        textures[name] = None
        return None
    asset = imported(path, 'Textures', unreal.Texture2D)
    options = TEXTURE_OPTIONS.get(name, {})
    asset.set_editor_property('srgb', options.get('srgb', True))
    if options.get('srgb', True) is False:
        # Raw 8-bit data, no normal-map swizzle or block compression.
        asset.set_editor_property('compression_settings', unreal.TextureCompressionSettings.TC_VECTOR_DISPLACEMENTMAP)
    for axis in ('address_x', 'address_y'):
        asset.set_editor_property(axis, getattr(unreal.TextureAddress, 'TA_' + options.get('address', 'WRAP')))
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
    # One translucency pass before depth of field, so that modulate sprites multiply what earlier emitters drew, in
    # emitter order (sort priority), as Cascade draws a system. In UE5's default after-DOF pass a modulate is applied to
    # the scene separately and the additive layer is added over it, which lost the bubble's dark core (host choice).
    material.set_editor_property('translucency_pass', unreal.MaterialTranslucencyPass.MTP_BEFORE_DOF)
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
    # x RadialSharpness (default 1: a linear falloff; > 1: a disc that stays full out to 1 - 1/sharpness of the radius).
    radial_s = op(M, radial, '', scalar('RadialSharpness', 1.0, -950, 1000), '', -900, 950)
    radial_c = node(unreal.MaterialExpressionSaturate, -800, 900)
    mel.connect_material_expressions(radial_s, '', radial_c, '')
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
    # LDR-like clip (host stand-in, UNVERIFIED): UE3 shows each channel of the final colour clipped at 1, so an
    # HDR particle colour such as (0.5, 0.8, 20) reads as saturated blue, while UE5's filmic curve turns it white.
    # Each layer's contribution is therefore capped per channel at what the clip would let through: 1 for an
    # additive layer (exact for one layer over the scene) and 1 / opacity for a translucent one.
    if blend == unreal.BlendMode.BLEND_ADDITIVE:
        emissive = op(unreal.MaterialExpressionMin, op(M, op(M, rgb, '', color_rgb, '', 100, 0), '', color_a, 'A', 250, 0), '',
                      const(1.0, 250, 100), '', 400, 0)
    elif domain_fn == 'screen':
        # Screen particle stand-in (a modulate quad over the view): the scene x the particle colour divided by its
        # luminance (Rec. 709 weights), so that the tint keeps the scene's brightness and pushes it toward the hue (blue
        # x4 for (4, 6, 30)), weighted by alpha and, from 0.6 to 1, by the mask texture's green streaks. Chosen from the
        # texture's name (a UV mask) and the 2026-10-02 game frames (a saturated blue flash, then a world that stays
        # blue and bright), not from the stripped graph (UNVERIFIED). Round 2 divided by the largest channel, which
        # darkened the scene to grey-brown.
        luminance = node(unreal.MaterialExpressionDotProduct, 400, 1800)
        mel.connect_material_expressions(color_rgb, '', luminance, 'A')
        mel.connect_material_expressions(node(unreal.MaterialExpressionConstant3Vector, 250, 1900,
                                              constant=unreal.LinearColor(0.2126, 0.7152, 0.0722, 1.0)), '', luminance, 'B')
        hue = op(D, color_rgb, '', op(unreal.MaterialExpressionMax, luminance, '', const(0.001, 400, 1950), '', 550, 1800), '', 700, 1700)
        streaks = node(unreal.MaterialExpressionLinearInterpolate, 400, 1500)
        mel.connect_material_expressions(const(0.6, 250, 1450), '', streaks, 'A')
        mel.connect_material_expressions(const(1.0, 250, 1520), '', streaks, 'B')
        mel.connect_material_expressions(mask(tex, 'RGB', 'G', 250, 1600), '', streaks, 'Alpha')
        weight = node(unreal.MaterialExpressionSaturate, 700, 1500)
        mel.connect_material_expressions(op(M, streaks, '', color, 'A', 550, 1500), '', weight, '')
        emissive = node(unreal.MaterialExpressionLinearInterpolate, 850, 1500)
        mel.connect_material_expressions(const(1.0, 700, 1400), '', emissive, 'A')
        mel.connect_material_expressions(hue, '', emissive, 'B')
        mel.connect_material_expressions(weight, '', emissive, 'Alpha')
    elif blend == unreal.BlendMode.BLEND_MODULATE or domain_fn == 'darken':
        # lerp(1, target, weight): weight = mask, x the particle alpha (1 - alpha with AlphaInvert) when UseAlpha, x the
        # colour's largest channel when ColourGain; target black instead of the colour when Darken. Which materials use
        # which is set per instance below. The 'darken'
        # parent draws the same blend as translucent black at opacity weight x (1 - the target's largest channel), which
        # equals the modulate for a black target and, unlike UE5's modulate (applied apart from the additive layer even
        # before DOF), darkens what earlier emitters drew.
        inverted = node(unreal.MaterialExpressionOneMinus, -500, 1450)
        mel.connect_material_expressions(color, 'A', inverted, '')
        particle_alpha = node(unreal.MaterialExpressionLinearInterpolate, -350, 1420)
        mel.connect_material_expressions(color, 'A', particle_alpha, 'A')
        mel.connect_material_expressions(inverted, '', particle_alpha, 'B')
        mel.connect_material_expressions(scalar('AlphaInvert', 0.0, -500, 1380), '', particle_alpha, 'Alpha')
        alpha_term = node(unreal.MaterialExpressionLinearInterpolate, -150, 1500)
        mel.connect_material_expressions(const(1.0, -350, 1500), '', alpha_term, 'A')
        mel.connect_material_expressions(particle_alpha, '', alpha_term, 'B')
        mel.connect_material_expressions(scalar('UseAlpha', 0.0, -350, 1580), '', alpha_term, 'Alpha')
        darken = scalar('Darken', 0.0, -350, 1700)
        largest = op(unreal.MaterialExpressionMax, op(unreal.MaterialExpressionMax, mask(color, '', 'R', -500, 1800), '',
                                                      mask(color, '', 'G', -500, 1880), '', -350, 1800), '',
                     mask(color, '', 'B', -500, 1960), '', -200, 1800)
        gain = node(unreal.MaterialExpressionLinearInterpolate, -50, 1800)
        mel.connect_material_expressions(const(1.0, -200, 1950), '', gain, 'A')
        mel.connect_material_expressions(largest, '', gain, 'B')
        mel.connect_material_expressions(scalar('ColourGain', 0.0, -200, 2030), '', gain, 'Alpha')
        # MaxWeight caps the darkening where opaque geometry lies just behind the quad (within 40 uu, fading out by 80 uu:
        # the lifted target under the depth-biased black orb), MaxWeightFar where the scene is further behind (the bubble
        # interior over the background). Both 1 unless set per instance below. Reason: UE3 blended into an 8-bit target
        # that clamps after every blend, so a darkening sprite acted on at most 1; UE5's float target lets the additive
        # layers drawn before it sum above 1, so the same visible darkness needs more weight there.
        depth_gap = op(S, node(unreal.MaterialExpressionSceneDepth, -350, 2200), '', node(unreal.MaterialExpressionPixelDepth, -350, 2300), '',
                       -200, 2250)
        far = node(unreal.MaterialExpressionSaturate, 100, 2250)
        mel.connect_material_expressions(op(D, op(S, depth_gap, '', const(40.0, -200, 2350), '', -50, 2250), '', const(40.0, -50, 2350), '',
                                            0, 2250), '', far, '')
        cap = node(unreal.MaterialExpressionLinearInterpolate, 150, 2150)
        mel.connect_material_expressions(scalar('MaxWeight', 1.0, 0, 2100), '', cap, 'A')
        mel.connect_material_expressions(scalar('MaxWeightFar', 1.0, 0, 2180), '', cap, 'B')
        mel.connect_material_expressions(far, '', cap, 'Alpha')
        weight = op(unreal.MaterialExpressionMin, op(M, op(M, brightest, '', alpha_term, '', 50, 1550), '', gain, '', 150, 1600), '',
                    cap, '', 250, 1600)
        weight_s = node(unreal.MaterialExpressionSaturate, 350, 1600)
        mel.connect_material_expressions(weight, '', weight_s, '')
        weight = weight_s
        # HueOnly: the colour divided by its luminance (Rec. 709), a brightness-keeping tint instead of a plain multiply.
        colour_luminance = node(unreal.MaterialExpressionDotProduct, -50, -350)
        mel.connect_material_expressions(color_rgb, '', colour_luminance, 'A')
        mel.connect_material_expressions(node(unreal.MaterialExpressionConstant3Vector, -200, -300,
                                              constant=unreal.LinearColor(0.2126, 0.7152, 0.0722, 1.0)), '', colour_luminance, 'B')
        colour_hue = op(D, color_rgb, '', op(unreal.MaterialExpressionMax, colour_luminance, '', const(0.001, -50, -250), '', 100, -350),
                        '', 250, -400)
        tint = node(unreal.MaterialExpressionLinearInterpolate, 400, -350)
        mel.connect_material_expressions(color_rgb, '', tint, 'A')
        mel.connect_material_expressions(colour_hue, '', tint, 'B')
        mel.connect_material_expressions(scalar('HueOnly', 0.0, 250, -300), '', tint, 'Alpha')
        target = node(unreal.MaterialExpressionLinearInterpolate, 250, -200)
        mel.connect_material_expressions(tint, '', target, 'A')
        mel.connect_material_expressions(const(0.0, 100, -250), '', target, 'B')
        mel.connect_material_expressions(darken, '', target, 'Alpha')
        if domain_fn == 'darken':
            target_largest = op(unreal.MaterialExpressionMax, op(unreal.MaterialExpressionMax, mask(target, '', 'R', 250, 2000), '',
                                                                 mask(target, '', 'G', 250, 2080), '', 400, 2000), '',
                                mask(target, '', 'B', 250, 2160), '', 550, 2000)
            remaining = node(unreal.MaterialExpressionOneMinus, 700, 2000)
            mel.connect_material_expressions(target_largest, '', remaining, '')
            opacity = node(unreal.MaterialExpressionSaturate, 850, 1800)
            mel.connect_material_expressions(op(M, weight, '', remaining, '', 700, 1800), '', opacity, '')
            mel.connect_material_property(opacity, '', unreal.MaterialProperty.MP_OPACITY)
            # DarkColor: the colour the darkening blends toward (black unless set per emitter by the host).
            emissive = mask(node(unreal.MaterialExpressionVectorParameter, 700, 0, parameter_name='DarkColor',
                                 default_value=unreal.LinearColor(0.0, 0.0, 0.0, 1.0)), '', 'RGB', 850, 0)
        else:
            emissive = node(unreal.MaterialExpressionLinearInterpolate, 400, 0)
            mel.connect_material_expressions(const(1.0, 100, -100), '', emissive, 'A')
            mel.connect_material_expressions(target, '', emissive, 'B')
            mel.connect_material_expressions(weight, '', emissive, 'Alpha')
    else:
        coverage = node(unreal.MaterialExpressionLinearInterpolate, 400, 300)
        mel.connect_material_expressions(alpha, '', coverage, 'A')
        mel.connect_material_expressions(brightest, '', coverage, 'B')
        mel.connect_material_expressions(scalar('LumAlpha', 0.0, 250, 400), '', coverage, 'Alpha')
        opacity = node(unreal.MaterialExpressionSaturate, 700, 300)
        mel.connect_material_expressions(op(M, coverage, '', color_a, 'A', 550, 300), '', opacity, '')
        mel.connect_material_property(opacity, '', unreal.MaterialProperty.MP_OPACITY)
        ceiling = op(D, const(1.0, 700, 450), '', op(unreal.MaterialExpressionMax, opacity, '', const(0.05, 700, 550), '', 850, 450),
                     '', 1000, 400)
        emissive = op(unreal.MaterialExpressionMin, op(M, rgb, '', color_rgb, '', 250, 0), '', ceiling, '', 1100, 0)
    mel.connect_material_property(emissive, '', unreal.MaterialProperty.MP_EMISSIVE_COLOR)
    if domain_fn == 'screen':
        material.set_editor_property('disable_depth_test', True)
    mel.recompile_material(material)
    eal.save_loaded_asset(material, only_if_is_dirty=False)
    return material


def tattoo_material():
    """Additive overlay for Maya's first-person arms, the power-emissive term of Master_Player's compiled pixel shader
    (Round 6; read from the shader cache, UNVERIFIED in game): glow = Enable x PowerEmissiveColor x (1 - f^2) x mask x
    diffuse, where f is the red of a fire-tile texture read at 0.6 x UV plus a slow pan, mask is the B channel of p_Masks at
    (0.5 u, 0.5 v + 0.5) (the tattoo shapes sit in that quadrant) and diffuse is p_Diffuse at the UV. The parameter names
    are the host's (Masks, Diffuse, GlowColor, Enable)."""
    material = tools.create_asset('M_OW_PlTattooGlow', f'{destination}/Materials', unreal.Material,
                                  unreal.MaterialFactoryNew())
    material.set_editor_property('blend_mode', unreal.BlendMode.BLEND_ADDITIVE)
    material.set_editor_property('shading_model', unreal.MaterialShadingModel.MSM_UNLIT)
    material.set_editor_property('used_with_skeletal_mesh', True)

    def node(cls, x, y, **props):
        expression = mel.create_material_expression(material, cls, x, y)
        for key, value in props.items():
            expression.set_editor_property(key, value)
        return expression

    uv = node(unreal.MaterialExpressionTextureCoordinate, -900, 0)
    time = node(unreal.MaterialExpressionTime, -900, 100)
    colour = node(unreal.MaterialExpressionVectorParameter, -900, 200, parameter_name='GlowColor',
                  default_value=unreal.LinearColor(0.0, 14.55, 20.0, 1.0))
    enable = node(unreal.MaterialExpressionScalarParameter, -900, 300, parameter_name='Enable', default_value=0.0)
    masks = node(unreal.MaterialExpressionTextureObjectParameter, -900, 400, parameter_name='Masks',
                 texture=unreal.load_asset('/Engine/EngineResources/Black'), sampler_type=unreal.MaterialSamplerType.SAMPLERTYPE_LINEAR_COLOR)
    diffuse = node(unreal.MaterialExpressionTextureObjectParameter, -900, 500, parameter_name='Diffuse',
                   texture=unreal.load_asset('/Engine/EngineResources/WhiteSquareTexture'))
    fire = texture('Fire_Tile_Dif')
    fire_object = node(unreal.MaterialExpressionTextureObject, -900, 600, texture=fire[0] if fire else unreal.load_asset('/Engine/EngineResources/Black'))
    custom = node(unreal.MaterialExpressionCustom, -400, 0, description='tattoo glow',
                  output_type=unreal.CustomMaterialOutputType.CMOT_FLOAT3, code="""
// Slow drift of the fire-tile read: one tile per 300 s sideways, one per 30 s the other way.
float2 pan = float2(frac(GT / 300.0), frac(-GT / 30.0));
float f = Texture2DSample(Fire, FireSampler, 0.6 * UV + pan).r;
float mask = Texture2DSample(Masks, MasksSampler, float2(0.5 * UV.x, 0.5 * UV.y + 0.5)).b;
float3 base = Texture2DSample(Diffuse, DiffuseSampler, UV).rgb;
return Enable * GlowColor * (1.0 - f * f) * mask * base;
""")
    names = ['UV', 'GT', 'GlowColor', 'Enable', 'Masks', 'Diffuse', 'Fire']
    entries = []
    for name in names:
        entry = unreal.CustomInput()
        entry.set_editor_property('input_name', name)
        entries.append(entry)
    custom.set_editor_property('inputs', entries)
    colour_rgb = node(unreal.MaterialExpressionComponentMask, -700, 200, r=True, g=True, b=True, a=False)
    mel.connect_material_expressions(colour, '', colour_rgb, '')
    for name, source in (('UV', uv), ('GT', time), ('GlowColor', colour_rgb), ('Enable', enable), ('Masks', masks),
                         ('Diffuse', diffuse), ('Fire', fire_object)):
        mel.connect_material_expressions(source, '', custom, name)
    mel.connect_material_property(custom, '', unreal.MaterialProperty.MP_EMISSIVE_COLOR)
    mel.recompile_material(material)
    eal.save_loaded_asset(material, only_if_is_dirty=False)
    return material


# --- exact stock materials -------------------------------------------------------------------------------------------
# Round 6/7: the cooked material graphs of the effect materials are stripped, but the shader cache keeps each material's
# compiled pixel shader and uniform-expression set (docs/verification/PHASELOCK_STOCK_DATA.md, "Round 6"). Each snippet below
# is written from what that shader computes, in our own words; numbers are the shaders' own constants unless a comment says
# "host calibration". Common inputs of every snippet:
#   Col   particle colour (rgb) and alpha (a)         UV   sprite UV, 0..1 across the quad
#   GT    material time in seconds                   SD   scene depth behind the pixel      PD   depth of the sprite plane
#   T0..  the material's textures, in the order the game lists them (sampler state comes from the Texture2D exports)
# "Soft fade" = saturate((SD - PD) / D): 1 where the scene is at least D units behind the sprite plane, 0 where the scene is at
# or in front of it. D is 1 - DepthBias for materials that have a DepthBias/Bias parameter and a fixed number otherwise.
# What stays a host reading (UNVERIFIED): the blend (UE5 draws modulate after the translucent pass, so the darkening
# modulates are translucent black with the darkening as opacity), the tangent-space view vector one material uses for a
# small parallax (taken as zero: the sprites face the camera), fog, and every line marked "host calibration".
# OWSPIN rotates the 2-D vector q by the angle a (radians), counter-clockwise.
SPIN = "#define OWSPIN(q, a) float2(cos(a) * (q).x - sin(a) * (q).y, sin(a) * (q).x + cos(a) * (q).y)\n"

EXACT = {}


def exact(name, blend, textures, code, subuv=False, dyn=False, params=()):
    EXACT[name] = {'blend': blend, 'textures': textures, 'code': SPIN + code, 'subuv': subuv, 'dyn': dyn, 'params': params}


# Mat_SirenEnemyOrb (additive): the bubble's rim sphere. The bubble texture (a thin glowing ring on a dark disc) is read at a
# UV pushed around by animated noise, so the ring wobbles. Noise: two reads of a smoke texture that rotate and drift (one
# gives the horizontal push, one the vertical). The push is zero inside 0.245 of the quad, ramps up by 0.316, and fades to
# nothing at the quad's corner. Direction: two reads of a flat normal map that rotate faster, giving mostly "up" with a small
# sideways part. Result = texture x particle alpha x soft fade over 41 units; the particle colour is not used.
exact('Mat_SirenEnemyOrb', 'add', ['Smoke2_GP_Dif', 'Nrm_Test', 'PhaseLockBubble_Dif_Tex'], r"""
float2 uv = UV;
float t = GT;
float2 centred = uv - 0.5;
float r2 = dot(centred, centred);
// Two drifting, rotating reads of the smoke map: its red and its blue channel are the two push amounts.
float2 smokeA = OWSPIN(2.0 * uv - 0.5, 0.58 * t) + float2(frac(0.2 * t), frac(0.1 * t)) + 0.5;
float2 smokeB = OWSPIN(2.0 * uv, -0.75 * t) + float2(frac(-0.4 * t), frac(0.1 * t));
float pushX = Texture2DSample(T0, T0Sampler, smokeA).r;
float pushY = Texture2DSample(T0, T0Sampler, smokeB).b;
// Where the push applies: nothing inside radius 0.245, full from 0.316, fading to zero at the corner (r2 = 0.5).
float ramp = (r2 <= 0.1) ? (25.0 * r2 - 1.5) : 1.0;
float edgeFade = 1.0 - 2.0 * r2;
float2 push = (ramp >= 0.0 && edgeFade >= 0.0) ? ramp * edgeFade * float2(pushX, pushY) : float2(0.0, 0.0);
// Direction of the push from two rotating normal-map reads (stored signed: the game's format is two signed bytes).
float2 n1 = Texture2DSample(T1, T1Sampler, OWSPIN(centred, 3.0 * t) + 0.5).xy * 2.0 - 1.0;
float2 n2 = Texture2DSample(T1, T1Sampler, OWSPIN(centred, 1.5 * t) + 0.5).xy * 2.0 - 1.0;
float n2z = sqrt(saturate(1.0 - dot(n2, n2)));
float2 direction = float2(n2.y + n1.x, n2z + n1.x);
// Host calibration (UNVERIFIED): the full push frills the rim on every frame, but the game's rim is a clean ring at 1.5 and
// 3.0 s and frayed at 4.5 s. The strength therefore grows with the collapse value (Dyn, 0 at the start, 0.75 at the end).
float strength = lerp(0.15, 0.7, saturate(Dyn / 0.6));
float3 ring = Texture2DSample(T2, T2Sampler, uv + strength * push * direction).rgb;
// Host calibration (UNVERIFIED): the texture's ring is pink-violet; the game frames show a white-blue rim, so red is
// reduced and blue raised (round 8: red 0.7 to 0.55, blue 1.3 to 1.4, because a pink fringe remained on the rim).
ring *= float3(0.55, 0.9, 1.4);
float softFade = saturate((SD - PD) / 41.0);
return float4(min(4.0, Col.a * ring) * softFade, 1.0);
""", dyn=True)

# Mat_EnemyOrbCoreColor (additive): the haze between the dark core and the rim. Colour: the core-colour texture times the
# particle colour, read at a UV nudged along the view direction by a scrolling noise (a small parallax, zero here). Weight:
# the texture's red x particle alpha x a mirrored mask read at twice the UV x the squared distance from the centre, so the
# haze is nil in the middle and strongest toward the edge. Soft fade over 41 units.
exact('Mat_EnemyOrbCoreColor', 'add', ['Tiling_GenericSmoke4_Dif', 'EnergyOrbCoreColor_Dif_Tex', 'EnergyOrbCenter_Dif_Mirror'], r"""
float2 uv = UV;
float t = GT;
float parallax = Texture2DSample(T0, T0Sampler, uv + float2(frac(0.4 * t), frac(0.5 * t))).r - 0.1;
float2 viewXY = float2(0.0, 0.0);
float4 core = Texture2DSample(T1, T1Sampler, uv + parallax * viewXY);
float2 centred = uv - 0.5;
float mask = Texture2DSample(T2, T2Sampler, 2.0 * uv).r;
// Host calibration (UNVERIFIED): x3 on the weight (the shader's own weights give a haze several times fainter than the
// game frames show between core and rim) and a blue-violet tint (the texture is magenta, the game's band is blue-violet).
float weight = 3.0 * core.r * Col.a * mask * dot(centred, centred);
float3 colour = Col.rgb * core.rgb * float3(0.3, 0.5, 1.6);
float softFade = saturate((SD - PD) / 41.0);
return float4(min(4.0, colour) * weight * softFade, 1.0);
""")

# Mat_SirenOrbEnergySpikes (translucent): the blue streaks around the bubble. A horizontally scrolling smoke read (0.25 per
# second) nudges the spike texture's UV by 0.05 x smoke in both axes. Colour = spike texture x particle colour. Alpha = the
# spike's red, cut by a mirrored mask read at (2u, 0.5v + 0.76) (that removes the middle of the sprite) x particle alpha x
# soft fade over 51 units.
exact('Mat_SirenOrbEnergySpikes', 'trans', ['Smoke2_GP_Dif', 'EnergyOrbSpikeys_Dif_Tex', 'EnergyOrbSoftMod_Dif_Mirror'], r"""
float2 uv = UV;
float smoke = Texture2DSample(T0, T0Sampler, uv + float2(frac(0.25 * GT), 0.0)).r;
float3 spike = Texture2DSample(T1, T1Sampler, uv + smoke * 0.05).rgb;
float cut = Texture2DSample(T2, T2Sampler, float2(2.0 * uv.x, 0.5 * uv.y + 0.76)).r;
float alpha = saturate(spike.r * (1.0 - 3.0 * cut)) * Col.a * saturate((SD - PD) / 51.0);
return float4(min(4.0, Col.rgb * spike), alpha);
""")

# Mat_SirenOrbEnergySpikesMOD (modulate): darkens the scene by (fade x particle colour x spike) per channel, a dark streak
# under each blue one. The particle colour is (5, 5, 5), so the darkening is the same in all channels; it is drawn as
# translucent black with that amount as opacity. NOT DRAWN by the host (see NOT_DRAWN): the game frames show no dark streaks.
exact('Mat_SirenOrbEnergySpikesMOD', 'dark', ['Smoke2_GP_Dif', 'EnergyOrbSpikeys_Dif_Tex'], r"""
float2 uv = UV;
float smoke = Texture2DSample(T0, T0Sampler, uv + float2(frac(0.25 * GT), 0.0)).r;
float3 spike = Texture2DSample(T1, T1Sampler, uv + smoke * 0.05).rgb;
float3 weight = Col.rgb * spike;
float softFade = saturate((SD - PD) / 51.0);
return float4(0.0, 0.0, 0.0, saturate(softFade * (weight.r + weight.g + weight.b) / 3.0));
""")

# Mat_SirenOrbBlackMOD (modulate): a round hole in the world. Darkness = (1 - particle alpha) x (1 - A^2) x soft fade over 21
# units (DepthBias -20), where A is 0 out to 0.15 of the quad and 1 from 0.354 of it. The bubble loop scales the particle
# alpha to 0, so the bubble's core is full black in the middle and clear at the rim.
# Host calibration (UNVERIFIED), bubble only (DarkCap < 1 is set per emitter by the host): the game's interior is a
# translucent navy-violet and the target shows at about a third of its brightness, so the darkening is capped, and the
# soft fade has a floor so that the target's near surface is also dimmed (by the shader it would not be). The hand's
# dark blob uses cap 1 and floor 0, which is what the game frames show (pure black at 0.30 s).
exact('Mat_SirenOrbBlackMOD', 'dark', [], r"""
float2 centred = UV - 0.5;
float u = 1.0 - saturate(8.0 * dot(centred, centred));
float A = 1.0 - saturate(1.5 * u * u);
float softFade = max(saturate((SD - PD) / 21.0), FadeFloor);
float coverage = (1.0 - A * A) * softFade;
float keep = Col.a + (1.0 - Col.a) * (1.0 - coverage);
return float4(0.0, 0.0, 0.0, min(DarkCap, saturate(1.0 - keep)));
""", params=(('DarkCap', 1.0), ('FadeFloor', 0.0)))
# The hand fizzle's variant has no depth fade at all.
exact('Mat_SirenOrbBlackMOD_NoBias', 'dark', [], r"""
float2 centred = UV - 0.5;
float u = 1.0 - saturate(8.0 * dot(centred, centred));
float A = 1.0 - saturate(1.5 * u * u);
float keep = Col.a + (1.0 - Col.a) * (A * A);
return float4(0.0, 0.0, 0.0, saturate(1.0 - keep));
""")

# Mat_PowerUpTwirls (translucent, sub-image sprites): twirling wisps. The twirl atlas is read at a UV bent by two slowly
# rotating reads of a scrolling-energy texture (0.02 x). Colour = (sqrt(t) - t) x particle colour x (0.6, 0, 0.8); alpha =
# saturate(4 t.r^2) x particle alpha. The loop template gives it a near-black particle colour, so these are dark wisps.
# Host calibration (UNVERIFIED): alpha x0.5, because the host's wisps read as hard black cracks where the game's are soft
# purple clouds.
exact('Mat_PowerUpTwirls', 'trans', ['Scrolling_Energy', 'LilithPowerUp_D'], r"""
float2 uv = UV;
float t = GT;
float bendX = Texture2DSample(T0, T0Sampler, OWSPIN(0.5 * uv - 0.25, 0.2 * t) + 0.25).r * 0.02;
float bendY = Texture2DSample(T0, T0Sampler, OWSPIN(uv - 0.5, 0.2 * t) + 0.5).b * 0.02;
float3 twirl = saturate(Texture2DSample(T1, T1Sampler, uv + float2(bendX, bendY)).rgb * 10.0);
float alpha = 0.5 * saturate(4.0 * twirl.r * twirl.r) * Col.a;
return float4(min(4.0, (sqrt(twirl) - twirl) * Col.rgb * float3(0.6, 0.0, 0.8)), alpha);
""", subuv=True)

# --- hand and release materials ---------------------------------------------------------------------------------------

# Mat_SirenEnergyRibbons (additive, mesh particles): the ribbons around the hand. The ribbon texture is read at
# (2u - dynamic parameter 0, v), so emitters scroll it along the mesh. Colour = saturate(u - 0.05) x blue^2 x particle alpha
# x particle colour.
exact('Mat_SirenEnergyRibbons', 'add', ['EnergyRibbon_Dif_Tex'], r"""
float2 uv = UV;
float b = Texture2DSample(T0, T0Sampler, float2(2.0 * uv.x - Dyn, uv.y)).b;
float amount = saturate(uv.x - 0.05) * b * (b * Col.a);
return float4(min(4.0, amount * Col.rgb), 1.0);
""", dyn=True)

# Mat_SirenEnergySwirl (translucent): the vortex arcs. Colour = the texture's blue channel x particle colour; alpha = the
# texture's alpha x particle alpha.
# Host calibration (UNVERIFIED): alpha x0.5 and a bluer colour, because the host's swirl at 0.30-0.40 s is a thick white
# vortex where the game's is thin, translucent blue ribbons.
exact('Mat_SirenEnergySwirl', 'trans', ['EnergySwirl_Dif_Tex'], r"""
float4 t = Texture2DSample(T0, T0Sampler, UV);
return float4(min(4.0, t.b * Col.rgb * float3(0.5, 0.8, 1.2)), 0.5 * max(0.0, t.a * Col.a));
""")

# Mat_SirenHandGlow (translucent): a soft glow. A mirrored soft mask read at twice the UV is both the colour weight and the
# alpha weight; soft fade over 21 units.
exact('Mat_SirenHandGlow', 'trans', ['EnergyOrbSoftMod_Dif_Mirror'], r"""
float g = Texture2DSample(T0, T0Sampler, 2.0 * UV).r;
return float4(min(4.0, g * Col.rgb), g * Col.a * saturate((SD - PD) / 21.0));
""")

# Mat_SirenHandGlowShattered (translucent): the star-burst. The star texture (RGBA) times the particle colour; alpha = the
# texture's alpha x particle alpha x soft fade over 21 units.
# Host calibration (UNVERIFIED): alpha x0.2 (0.4 in round 7), because in the host the long rays swamp the hand at 0.55-0.75 s while the game
# keeps a visible palm orb with a few thin white lines.
exact('Mat_SirenHandGlowShattered', 'trans', ['EnergyShatter_Dif_Tex'], r"""
float4 t = Texture2DSample(T0, T0Sampler, UV);
return float4(min(4.0, Col.rgb * t.rgb), 0.2 * t.a * Col.a * saturate((SD - PD) / 21.0));
""")

# Mat_SirenHandInnerOrb (translucent): the blue palm orb. Four reads of a nebula texture in two rotating frames (0.18 rad/s
# about 0.5 and -0.25 rad/s about 0.8, each with its own drift) are combined as (n3 - n4) + (n1 - n2) + 2; that number
# shifts a read m1 of a mirrored centre texture at twice the UV. With the unshifted read m2, a blend weight
# 12 (1 - m2)^6 m2 mixes saturate(2 m2) toward m1, giving k. Colour = lerp(blue, 1.25 x orb texture, k) with the blue
# (0.2508, 0.6524, 0.9323); alpha = saturate(k) x particle alpha. The particle colour is not used.
exact('Mat_SirenHandInnerOrb', 'trans', ['EnergyOrbCenter2_Dif_Tex', 'EnergyOrbCenter_Dif_Mirror', 'Tiling_Nebulous_Dif'], r"""
float2 uv = UV;
float t = GT;
float2 frameP = OWSPIN(uv - 0.5, 0.18 * t) + 0.5;
float2 frameQ = OWSPIN(uv - 0.8, -0.25 * t) + 0.8;
float n3 = Texture2DSample(T2, T2Sampler, frameP + float2(0.0, frac(0.35 * t))).r;
float n1 = Texture2DSample(T2, T2Sampler, frameP + float2(frac(0.04 * t), frac(0.5 * t))).r;
float n4 = Texture2DSample(T2, T2Sampler, frameQ + float2(frac(-0.15 * t), frac(0.15 * t))).r;
float n2 = Texture2DSample(T2, T2Sampler, frameQ + float2(frac(-0.05 * t), frac(0.7 * t))).r;
float shift = (n3 - n4) + (n1 - n2) + 2.0;
float m1 = Texture2DSample(T1, T1Sampler, 2.0 * uv + shift).r;
float m2 = Texture2DSample(T1, T1Sampler, 2.0 * uv).r;
float oneMinus = 1.0 - m2;
float mixWeight = 12.0 * pow(oneMinus, 6.0) * m2;
float base = saturate(2.0 * m2);
float k = base + mixWeight * (m1 - base);
float3 blue = float3(0.2508, 0.6524, 0.9323);
// Host calibration (UNVERIFIED): the orb texture is scaled by 0.6 instead of the shader's 1.25. At 1.25 the orb's centre is
// white-blue; the game's palm orb is a saturated deep blue with a bright highlight and thin arcs.
float3 orb = Texture2DSample(T0, T0Sampler, uv).rgb * 0.6;
return float4(min(4.0, lerp(blue, orb, k)), saturate(k) * Col.a);
""")

# Mat_SirenHandPowerDiffuse (translucent): the smoke after the orb leaves. Colour is just the particle colour. Alpha = 35 x
# a nebula read drifting upward x (1 - (1 - 0.1 x second smoke read)(1 - mask))^2 x particle alpha x soft fade over 16 units.
exact('Mat_SirenHandPowerDiffuse', 'trans', ['Tiling_Nebulous_Dif', 'Tiling_GenericSmoke4_Dif', 'SubUV_2X2_GenericSmoke2_Dif'], r"""
float2 uv = UV;
float t = GT;
float nebula = Texture2DSample(T0, T0Sampler, uv + float2(0.0, frac(0.2 * t))).r * 35.0;
float smoke = Texture2DSample(T1, T1Sampler, uv + float2(frac(0.15 * t), frac(0.2 * t))).r;
float mask = Texture2DSample(T2, T2Sampler, uv).r;
float blend = 1.0 - (1.0 - 0.1 * smoke) * (1.0 - mask);
return float4(min(4.0, Col.rgb), nebula * blend * blend * Col.a * saturate((SD - PD) / 16.0));
""")

# Mati_Wispy_Smoke_Cloud_SubUV (translucent, sub-image sprites; parent Mat_Wispy_Smoke): the sub-image alpha is both the
# colour weight and, with the particle alpha, the opacity; soft fade over 19 units (DepthBias -18). The game blends two
# sub-images by the particle's frame fraction; the host shows one.
exact('Mati_Wispy_Smoke_Cloud_SubUV', 'trans', ['Tex_Wispy_Smoke_SubUV'], r"""
float a = Texture2DSample(T0, T0Sampler, UV).a;
return float4(min(4.0, a * Col.rgb), saturate(a * Col.a) * saturate((SD - PD) / 19.0));
""", subuv=True)

# Mat_SirenGlowMOD (modulate, overbright): the scene is multiplied by 1 + min(3, soft fade over 16 x particle alpha x a
# mirrored soft mask at twice the UV x particle colour). UE5 draws modulate after the translucent layers, not in emitter
# order. Host calibration (UNVERIFIED): the overbright is scaled by 0.4, because at 0.55 s the host's whole street is washed
# out while the game frame keeps its contrast (the game's wash comes at 0.72 s).
exact('Mat_SirenGlowMOD', 'mod', ['EnergyOrbSoftMod_Dif_Mirror'], r"""
float g = Texture2DSample(T0, T0Sampler, 2.0 * UV).r;
float3 boost = min(3.0, saturate((SD - PD) / 16.0) * Col.a * g * Col.rgb);
return float4(1.0 + 0.4 * boost, 1.0);
""")


def exact_material(name):
    spec = EXACT[name]
    blend = {'add': unreal.BlendMode.BLEND_ADDITIVE, 'mod': unreal.BlendMode.BLEND_MODULATE}.get(
        spec['blend'], unreal.BlendMode.BLEND_TRANSLUCENT)
    material = tools.create_asset(f'M_PL_{name}', f'{destination}/Materials', unreal.Material, unreal.MaterialFactoryNew())
    material.set_editor_property('blend_mode', blend)
    material.set_editor_property('shading_model', unreal.MaterialShadingModel.MSM_UNLIT)
    material.set_editor_property('two_sided', True)
    material.set_editor_property('translucency_pass', unreal.MaterialTranslucencyPass.MTP_BEFORE_DOF)

    def node(cls, x, y, **props):
        expression = mel.create_material_expression(material, cls, x, y)
        for key, value in props.items():
            expression.set_editor_property(key, value)
        return expression

    def link(a, a_pin, b, b_pin):
        mel.connect_material_expressions(a, a_pin, b, b_pin)

    color = node(unreal.MaterialExpressionVectorParameter, -900, -300, parameter_name='Color',
                 default_value=unreal.LinearColor(1, 1, 1, 1))
    color_rgb = node(unreal.MaterialExpressionComponentMask, -700, -300, r=True, g=True, b=True, a=False)
    link(color, '', color_rgb, '')
    color4 = node(unreal.MaterialExpressionAppendVector, -500, -300)
    link(color_rgb, '', color4, 'A')
    link(color, 'A', color4, 'B')
    uv = node(unreal.MaterialExpressionTextureCoordinate, -900, 0)
    uv_out = uv
    if spec['subuv']:
        # Sub-image atlas UV: (uv + (column, row)) / (columns, rows), the frame index in SubUV.B (set per particle).
        sub = node(unreal.MaterialExpressionVectorParameter, -900, 150, parameter_name='SubUV',
                   default_value=unreal.LinearColor(1, 1, 0, 0))
        columns = node(unreal.MaterialExpressionComponentMask, -700, 150, r=True, g=False, b=False, a=False)
        rows = node(unreal.MaterialExpressionComponentMask, -700, 220, r=False, g=True, b=False, a=False)
        frame = node(unreal.MaterialExpressionComponentMask, -700, 290, r=False, g=False, b=True, a=False)
        for m in (columns, rows, frame):
            link(sub, '', m, '')
        column = node(unreal.MaterialExpressionFmod, -500, 150)
        link(frame, '', column, 'A')
        link(columns, '', column, 'B')
        row_raw = node(unreal.MaterialExpressionDivide, -500, 250)
        link(frame, '', row_raw, 'A')
        link(columns, '', row_raw, 'B')
        row = node(unreal.MaterialExpressionFloor, -350, 250)
        link(row_raw, '', row, '')
        offset = node(unreal.MaterialExpressionAppendVector, -200, 200)
        link(column, '', offset, 'A')
        link(row, '', offset, 'B')
        size = node(unreal.MaterialExpressionAppendVector, -200, 300)
        link(columns, '', size, 'A')
        link(rows, '', size, 'B')
        shifted = node(unreal.MaterialExpressionAdd, -50, 100)
        link(uv, '', shifted, 'A')
        link(offset, '', shifted, 'B')
        atlas = node(unreal.MaterialExpressionDivide, 100, 100)
        link(shifted, '', atlas, 'A')
        link(size, '', atlas, 'B')
        uv_out = atlas
    time = node(unreal.MaterialExpressionTime, -900, 450)
    scene_depth = node(unreal.MaterialExpressionSceneDepth, -900, 550)
    pixel_depth = node(unreal.MaterialExpressionPixelDepth, -900, 650)
    inputs = [('Col', color4), ('UV', uv_out), ('GT', time), ('SD', scene_depth), ('PD', pixel_depth)]
    for param_name, default in spec['params']:
        inputs.append((param_name, node(unreal.MaterialExpressionScalarParameter, -900, 1200, parameter_name=param_name,
                                        default_value=default)))
    if spec['dyn']:
        dyn = node(unreal.MaterialExpressionScalarParameter, -900, 750, parameter_name='DynParam', default_value=0.0)
        inputs.append(('Dyn', dyn))
    for index, texture_name in enumerate(spec['textures']):
        asset = texture(texture_name)
        if not asset:
            raise RuntimeError(f'{name}: texture {texture_name} not in the UModel output')
        options = TEXTURE_OPTIONS.get(texture_name, {})
        sampler = (unreal.MaterialSamplerType.SAMPLERTYPE_COLOR if options.get('srgb', True)
                   else unreal.MaterialSamplerType.SAMPLERTYPE_LINEAR_COLOR)
        tex = node(unreal.MaterialExpressionTextureObject, -900, 850 + 100 * index, texture=asset[0], sampler_type=sampler)
        inputs.append((f'T{index}', tex))
    custom = node(unreal.MaterialExpressionCustom, 400, 0, code=spec['code'], description=name,
                  output_type=unreal.CustomMaterialOutputType.CMOT_FLOAT4)
    custom_inputs = []
    for input_name, _source in inputs:
        entry = unreal.CustomInput()
        entry.set_editor_property('input_name', input_name)
        custom_inputs.append(entry)
    custom.set_editor_property('inputs', custom_inputs)
    for input_name, source in inputs:
        link(source, '', custom, input_name)
    rgb = node(unreal.MaterialExpressionComponentMask, 650, 0, r=True, g=True, b=True, a=False)
    link(custom, '', rgb, '')
    if blend == unreal.BlendMode.BLEND_MODULATE:
        # The overbright modulate (scene x (1 + c)) is drawn as UE5's modulate with the factor unclamped.
        mel.connect_material_property(rgb, '', unreal.MaterialProperty.MP_EMISSIVE_COLOR)
    else:
        # The shaders write colours up to 4 (min(c, 4)). Host reading (UNVERIFIED): a layer's colour is brought into range
        # by dividing by its largest channel when that exceeds 1, which keeps the hue. A per-channel clip turned the spikes'
        # blue (1.6, 13, 40) x texture into cyan-white, and no clip made them white lines; the game frames show deep blue.
        clamped = node(unreal.MaterialExpressionCustom, 800, 0, description='hue-keeping range', code=
                       'float m = max(1.0, max(C.r, max(C.g, C.b))); return C / m;',
                       output_type=unreal.CustomMaterialOutputType.CMOT_FLOAT3)
        entry = unreal.CustomInput()
        entry.set_editor_property('input_name', 'C')
        clamped.set_editor_property('inputs', [entry])
        link(rgb, '', clamped, 'C')
        mel.connect_material_property(clamped, '', unreal.MaterialProperty.MP_EMISSIVE_COLOR)
    if blend == unreal.BlendMode.BLEND_TRANSLUCENT:
        alpha = node(unreal.MaterialExpressionComponentMask, 650, 150, r=False, g=False, b=False, a=True)
        link(custom, '', alpha, '')
        mel.connect_material_property(alpha, '', unreal.MaterialProperty.MP_OPACITY)
    mel.recompile_material(material)
    eal.save_loaded_asset(material, only_if_is_dirty=False)
    instance = tools.create_asset(f'MI_{name}', f'{destination}/Materials', unreal.MaterialInstanceConstant,
                                  unreal.MaterialInstanceConstantFactoryNew())
    mel.set_material_instance_parent(instance, material)
    eal.save_loaded_asset(instance, only_if_is_dirty=False)
    report['materials'][name] = {'instance': instance.get_path_name(), 'parent': material.get_name(), 'exact': True,
                                 'textures': spec['textures'], 'blend': spec['blend']}
    log(f'{name}: exact {spec["blend"]} textures {spec["textures"]}')


# --- run -------------------------------------------------------------------------------------------------------------

if eal.does_directory_exist(destination):
    eal.delete_directory(destination)
parents = {
    'BLEND_Additive': build('M_OW_PlAdditive', unreal.BlendMode.BLEND_ADDITIVE),
    'BLEND_Translucent': build('M_OW_PlTranslucent', unreal.BlendMode.BLEND_TRANSLUCENT),
    'BLEND_Modulate': build('M_OW_PlModulate', unreal.BlendMode.BLEND_MODULATE),
}
screen_parent = build('M_OW_PlScreen', unreal.BlendMode.BLEND_MODULATE, 'screen')
darken_parent = build('M_OW_PlDarken', unreal.BlendMode.BLEND_TRANSLUCENT, 'darken')
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

# Mat_SirenGlowMOD multiplies the scene by 1 + min(3, c), which an 8-bit target clamps to 1 (UNVERIFIED reading of the
# target format): not drawn. Its emitters are reported as skipped and left out by the host.
NOT_DRAWN = {
    'Mat_SirenOrbEnergySpikesMOD': 'dark streaks under the blue spikes: the shader darkens (1 - fade x 5 x spike) but the game frames show no dark streaks, cause not found; not drawn (UNVERIFIED)',
}
# The slot guess of UModel's .mat names the wrong mask for the screen effect: the shader reads the second referenced
# texture, PhaseLockScreenMask02_Dif_Tex (WillowGame.upk).
TEXTURE_CHOICE = {'Mat_PhaseLockScreenEffect': 'PhaseLockScreenMask02_Dif_Tex'}

for name in sorted(uses):
    if name in NOT_DRAWN:
        report['skipped'][name] = {'reason': NOT_DRAWN[name]}
        continue
    if name in EXACT:
        exact_material(name)
        continue
    layout = uses[name]
    slots = {}
    mat_file = find(name, '.mat')
    if mat_file:
        for line in mat_file.read_text(encoding='utf-8', errors='replace').splitlines():
            if '=' in line:
                key, value = line.split('=', 1)
                slots[key.strip()] = value.strip()
    chosen = TEXTURE_CHOICE.get(name) or slots.get('Diffuse')
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
    parent = screen_parent if layout['screen'] else darken_parent if name in DARKEN_AS_TRANSLUCENT else parents[mode]
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
        if name in RADIAL_SHARPNESS:
            flags['RadialSharpness'] = RADIAL_SHARPNESS[name]
    if layout['mesh']:
        flags['PanScale'] = 1.0
    if mode == 'BLEND_Modulate':
        flags.update(MODULATE_READINGS.get(name, {}))
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
