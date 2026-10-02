"""Master_Gun paint reading, shared by the thumbnail renderer, the paint preparer and the tests.

Where it comes from (our own words; nothing here is copied from a listing): the graph of
Master_Gun is stripped from the cooked packages, but the compiled shader cache of the install
(RefShaderCache-PC-D3D-SM3.upk) keeps, for every static permutation of it, the uniform
expression set (which parameter feeds which shader constant and sampler) and the compiled
pixel shaders. Reading the GPU-skinned base-pass pixel shader of the slice permutations with
our own token reader (research/d3d9_bytecode.py) gives the colour model below. The static
parameters (tools/material_static_parameters.py) only pick channels: p_WeapClassSelect picks
the detail atlas channel, p_PatternChannel picks the pattern texture channels, p_DecalChannel
the decal channels; sw_FlipDecalOnRightSide is off on every slice chain.

The model, per pixel (UV0 = u0, v0; UV1 = u1, v1):
- p_Masks holds two maps stacked vertically. The lower half (v0 * 0.5 + 0.5) is the zone mask:
  red, green and blue pick zones A, B and C. The upper half (v0 * 0.5) is a light/dark map:
  its red channel times p_HighlightsIntensity and its green channel times p_ShadowsIntensity,
  each clamped to 0..1, give a highlight and a shadow amount.
- Each zone's colour starts at its Midtone, moves towards Hilight by the highlight amount, then
  towards Shadow by the shadow amount.
- Starting from p_DColor, the zone colours are blended in one after another (A by the mask's
  red, then B by green, then C by blue).
- Pattern: p_Pattern sampled at UV1 * xy + zw of p_PatternScalePosition (only its selected
  channels), times p_PatternColor. Its weight is the squared length of mask * p_PatternChannelScale,
  clamped. The colour is multiplied by a blend from 1 to the pattern colour by that weight; with
  p_ReplacePattern at 1 it is instead replaced by the pattern colour by that weight.
- Decal: UV1 is shifted by zw of p_DecalScalePosition, rotated about (0.5, 0.5) by
  p_DecalRotate * pi, then scaled by xy about (0.5, 0.5). The tint is p_DecalColor, times the
  decal colour when p_UseFullColorDecal is 1. Its amount is the squared length of
  mask * p_DecalChannel, clamped, times the decal alpha (full colour) or times the decal colour
  (single channel). Multiply or replace exactly as for the pattern, by p_ReplaceDecal.
- The result is multiplied by the selected channel of the p_Diffuse detail atlas (UV0).
- Environment term: P_SimpleReflect is sampled at the x and y of the tangent-space reflection
  vector (plus a per-object offset), times p_ReflectColor, times the squared length of
  mask * p_ReflectionChannelScale, clamped. Call that R and the colour so far c; the output is
  c + R blended towards R * c by p_ReflectColorScale (see add_reflection).
- Colour space: vector parameters are linear. Textures follow their installed SRGB flag; the
  class default (Engine.upk Default__Texture) is SRGB on, and only p_Masks and the normal maps
  turn it off, so the detail atlas, patterns, decals and the environment map are sRGB-decoded.
- Not modelled here: emissive, digistruct, the selection tint and all lighting.

Status: UNVERIFIED. The reading is a structural one of compiled data; it has not been checked
against the running game, only against screenshots by eye.
"""
import math

PAINT_READING = ('UNVERIFIED reading of the compiled Master_Gun base-pass shader (see '
                 'tools/weapon_paint_model.py): p_Masks lower half = zone mask, upper half = '
                 'highlight/shadow map; zone tones Midtone->Hilight->Shadow; zones blended over p_DColor; '
                 'pattern and decal multiply (or replace) by squared mask weights; times the detail channel; '
                 'plus the P_SimpleReflect environment term. Textures decoded per their SRGB flag. '
                 'Emissive and lighting are not modelled.')


def clamp(value):
    return 0.0 if value < 0.0 else 1.0 if value > 1.0 else value


def lerp(a, b, t):
    return tuple(x + (y - x) * t for x, y in zip(a, b))


def mask_uvs(u0, v0):
    """(light/dark map UV, zone mask UV) inside the stacked p_Masks texture."""
    return (u0, v0 * 0.5), (u0, v0 * 0.5 + 0.5)


def pattern_uv(u1, v1, scale_position):
    sx, sy, ox, oy = scale_position[:4]
    return u1 * sx + ox, v1 * sy + oy


def decal_uv(u1, v1, scale_position, rotate):
    sx, sy, ox, oy = scale_position[:4]
    angle = rotate * math.pi
    s, c = math.sin(angle), math.cos(angle)
    x, y = u1 + ox - 0.5, v1 + oy - 0.5
    rx, ry = c * x - s * y + 0.5, s * x + c * y + 0.5
    return rx * sx + (1.0 - sx) * 0.5, ry * sy + (1.0 - sy) * 0.5


def squared_weight(mask, scale):
    return clamp(sum((m * s) ** 2 for m, s in zip(mask, scale[:3])))


def select(texel, channels):
    """A static component mask: one channel broadcasts, several keep their places."""
    picked = [texel['RGB'.index(c)] for c in channels if c in 'RGB']
    if len(picked) == 1:
        return (picked[0],) * 3
    return tuple(texel[i] if 'RGB'[i] in channels else 0.0 for i in range(3))


def layer(colour, tint, amount, replace):
    """Multiply by lerp(1, tint, amount), or (replace -> 1) blend to the tint by amount."""
    amounts = amount if isinstance(amount, tuple) else (amount,) * 3
    multiplied = tuple(c * (1.0 + (t - 1.0) * a) for c, t, a in zip(colour, tint, amounts))
    replaced = tuple(c + (t - c) * a for c, t, a in zip(colour, tint, amounts))
    return lerp(multiplied, replaced, replace)


def albedo(light, mask, detail, params, pattern=None, decal=None):
    """Linear HDR colour of one texel before lighting.

    light: (r, g) of the light/dark map; mask: (r, g, b) zone mask; detail: selected detail value;
    pattern: (r, g, b) or None; decal: (r, g, b, a) or None; params: see PAINT_PARAMETERS.
    """
    vec, sca = params['vector'], params['scalar']
    high = clamp(light[0] * sca['p_HighlightsIntensity'])
    low = clamp(light[1] * sca['p_ShadowsIntensity'])
    colour = tuple(vec['p_DColor'][:3])
    for zone, weight in zip('ABC', mask):
        tone = lerp(lerp(vec[f'p_{zone}ColorMidtone'][:3], vec[f'p_{zone}ColorHilight'][:3], high),
                    vec[f'p_{zone}ColorShadow'][:3], low)
        colour = lerp(colour, tone, weight)
    if pattern is not None:
        tint = tuple(p * c for p, c in zip(select(pattern, params['pattern_channels']), vec['p_PatternColor'][:3]))
        colour = layer(colour, tint, squared_weight(mask, vec['p_PatternChannelScale']), sca['p_ReplacePattern'])
    if decal is not None:
        rgb = select(decal[:3], params['decal_channels'])
        full = sca['p_UseFullColorDecal']
        tint = tuple(c + (c * d - c) * full for c, d in zip(vec['p_DecalColor'][:3], rgb))
        weight = squared_weight(mask, vec['p_DecalChannel'])
        amount = tuple(d * weight + (weight * decal[3] - d * weight) * full for d in rgb)
        colour = layer(colour, tint, amount, sca['p_ReplaceDecal'])
    return tuple(c * detail for c in colour)


def add_reflection(colour, environment, mask, params):
    """colour (already times detail) plus the environment term; environment is the linear env texel."""
    vec, sca = params['vector'], params['scalar']
    weight = squared_weight(mask, vec['p_ReflectionChannelScale'])
    reflect = tuple(e * r * weight for e, r in zip(environment, vec['p_ReflectColor'][:3]))
    scale = sca['p_ReflectColorScale']
    return tuple(c + r + (r * c - r) * scale for c, r in zip(colour, reflect))


def srgb_to_linear(value):
    return value / 12.92 if value <= 0.04045 else ((value + 0.055) / 1.055) ** 2.4


PAINT_PARAMETERS = {
    'vector': ['p_DColor'] + [f'p_{zone}Color{tone}' for zone in 'ABC' for tone in ('Shadow', 'Midtone', 'Hilight')],
    'scalar': ['p_HighlightsIntensity', 'p_ShadowsIntensity'],
    'pattern_vector': ['p_PatternColor', 'p_PatternChannelScale', 'p_PatternScalePosition'],
    'pattern_scalar': ['p_ReplacePattern'],
    'decal_vector': ['p_DecalColor', 'p_DecalChannel', 'p_DecalScalePosition'],
    'decal_scalar': ['p_DecalRotate', 'p_UseFullColorDecal', 'p_ReplaceDecal'],
    'reflect_vector': ['p_ReflectColor', 'p_ReflectionChannelScale'],
    'reflect_scalar': ['p_ReflectColorScale'],
}
