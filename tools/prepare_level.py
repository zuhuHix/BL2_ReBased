"""Prepare Ash_P and its referenced sublevels for the UE5 editor host.

All game-derived output stays in local/. This is an offline scene loader v1,
not gameplay streaming or a general UE3 material-graph interpreter.
"""
import argparse
import hashlib
import json
import math
import re
from pathlib import Path
import struct
import subprocess
from collision_geometry import hulls as collision_hulls
from installed_content import PACKAGE_SUFFIXES, cache_directory, content_files


def values(tags):
    return {p['name']: p['value'] for p in tags if p.get('status') == 'decoded'}


def props(record):
    if 'error' in record:
        raise ValueError(record['error'])
    return values(record.get('data', {}).get('properties', []))


def vector(value, default=0):
    return [value.get(k, default) for k in ('X', 'Y', 'Z')]


def transform(p, component=False):
    rot = p.get('Rotation', {})
    return {'location': vector(p.get('Translation' if component else 'Location', {})),
            'rotation': [rot.get(k, 0) * 360 / 65536 for k in ('Pitch', 'Yaw', 'Roll')],
            'scale': [v * p.get('Scale' if component else 'DrawScale', 1)
                      for v in vector(p.get('Scale3D' if component else 'DrawScale3D', {}), 1)]}


def collection_transforms(payload, data, count):
    """Observed Ash layout: rotation/translation matrix, Scale3D, scale, int.

    Require the exact tail size and affine finite matrices; no offset scanning.
    The final int is retained without assigning unverified semantics.
    """
    offset = data['property_offset'] + data['consumed_bytes']
    if len(payload) - offset != count * 84:
        raise ValueError('Unsupported collection transform layout')
    result = []
    for i in range(count):
        row = struct.unpack_from('<20fi', payload, offset + i * 84)
        if not all(math.isfinite(v) for v in row[:20]):
            raise ValueError('Nonfinite collection transform')
        if any(abs(row[k]) > 1e-5 for k in (3, 7, 11)) or abs(row[15] - 1) > 1e-5:
            raise ValueError('Non-affine collection matrix')
        result.append({'matrix': list(row[:16]), 'scale': [v * row[19] for v in row[16:19]],
                       'source_flags': row[20]})
    return result


def collection_scale(p, cooked):
    """A collection entry's scale, preferring the component's own Scale3D/Scale.

    The cooked per-entry tail carries a copy of the component's scale, and for
    2036 of the 2037 Sanctuary_P collection components that declare one the two
    agree exactly. The exception is StaticMeshActor_SMC_1802, whose component
    property says Scale3D=(0.97,1,1) while the tail says (1,1,1); the running
    game applies the 0.97 (its object dump reports a _LocalToWorld determinant
    of 0.97), so the property is treated as authoritative where it exists.
    """
    if 'Scale3D' not in p and 'Scale' not in p:
        return cooked
    return [v * p.get('Scale', 1) for v in vector(p.get('Scale3D', {}), 1)]


# UE3 material graphs use a small set of exact names for the ordinary
# channels, but environment masters also use stable aliases. Keep this
# allow-list narrow: treating every mask/noise/cubemap parameter as albedo
# makes sky and FX planes become opaque white geometry in the host.
CHANNELS = {'p_diffuse': 'diffuse', 'diffuse': 'diffuse',
            'p_normal': 'normal', 'normal': 'normal',
            'p_specular': 'specular', 'specular': 'specular',
            'p_emissive': 'emissive', 'emissive': 'emissive'}

# Native sky import is intentionally limited to the observed dome mesh. Other
# sky-named meshes retain ordinary Material v1 handling until their activation
# and material chains are understood.
NATIVE_SKYBOX_MESH = 'Prop_Skybox.Meshes.Sky_Dome'
# The dome's master graph is stripped from the cooked package. Only its named
# inputs survive; the host combines them under a recorded approximation
# (docs/verification/NATIVE_SKY_APPROXIMATION.md). Every mapping here is a
# guess labeled UNVERIFIED, not a decoded graph.
NATIVE_SKY_MASTER = 'Common_Materials.Sky.Mat_SkyTimeOfDay_Master'
SKY_SAMPLERS = {'transition_track': 'transition_track', 'clouds': 'clouds', 'masks': 'masks'}
SKY_SCALARS = {'time_of_day': 'time_of_day', 'sky_brightness': 'sky_brightness',
               'sun_spot_brightness': 'sun_spot_brightness',
               'cloud_cap_opacity': 'cloud_cap_opacity', 'p_couldbrightness': 'cloud_brightness'}
SKY_VECTORS = {'horizion_track_color_multiplier': 'horizon_track_color_multiplier'}
# Column lookup divisor for Time_of_Day on the 256-wide transition strip. The
# alternative reading (degrees, /360) lands in a sun column and produces dusk
# hues; the observed value 170 read as a pixel column gives the blue daytime
# gradient. Neither is decoded from the graph.
SKY_TIME_AXIS = 256.0
SKY_HORIZON_ROW = 0.95
# The `_Outer` city hull and its antennas. Their placed overrides are the
# masked `_Teleported` phase-in variants whose graphs Material v1 cannot
# recover, so they import invisible. Opt-in, the placement falls back to the
# mesh-default materials, which resolve a diffuse. Which of `_Land` and
# `_Outer` the running game shows is Kismet state and is not interpreted.
OUTER_SHELL_MESHES = {'Prop_Skybox.Meshes.SanctuarySky',
                      'FX_ENV_Sanctuary.Meshes.SanctuarySkybox_Antenna1',
                      'FX_ENV_Sanctuary.Meshes.SanctuarySkybox_Antenna2'}
OUTER_SHELL_OVERRIDE_SUFFIX = '_Teleported'
HIDDEN_VISUAL_MESH = 'Common_Meshes.Blocking.Blocking_Cube'
HIDDEN_COLLISION_MESH = 'Common_Meshes.CollisionCube'
HIDDEN_CLOUD_MESH = 'Common_Meshes.Blocking.Blocking_Plane'

# Cheap, opt-in Matinee placement experiment for the Sanctuary hull. This is
# intentionally a source-path match, not a general SeqAct_Interp decoder.
MATINEE_FIRST_KEY_COMPONENT = (
    'TheWorld.PersistentLevel.InterpActor_29.StaticMeshComponent_393')
MATINEE_FIRST_KEY_TRANSLATION = [16551, -171794, -164]
MATINEE_FIRST_KEY_ROTATION = [0, 78.75, 0]


def apply_matinee_first_key_pose(source, pose):
    """Apply the observed first RelativeToInitial key to one hull pose.

    The serialized actor pose remains the source of the base transform. The
    translation and yaw are only an opt-in visual experiment; this function
    does not claim to decode SeqAct_Interp start-position or Kismet state.
    """
    if source != MATINEE_FIRST_KEY_COMPONENT:
        return pose, False
    if 'matrix' in pose or 'actor' not in pose:
        raise ValueError('Matinee first-key target is not an actor/component pose')
    actor = dict(pose['actor'])
    actor['location'] = [a + offset for a, offset in
                         zip(actor['location'], MATINEE_FIRST_KEY_TRANSLATION)]
    actor['rotation'] = list(MATINEE_FIRST_KEY_ROTATION)
    return dict(pose, actor=actor), True
# Both observed cloud-layer instances: `_Light` in Sanctuary_P (four planes)
# and `_01` in Sanctuary_Light (two planes on the horizon, whose sole cooked
# texture is a dust sprite that tiled as yellow/black stripes).
HIDDEN_CLOUD_MATERIALS = {'Sanctuary_P:Env_Ice.Materials.Mat_CloudLayer_Light',
                          'Sanctuary_Light:Env_Ice.Materials.Mat_CloudLayer_01'}
HIDDEN_TRANSITION_MESH = 'Common_Meshes.BasePlane_256x128'
HIDDEN_TRANSITION_MATERIAL = 'Common_Materials.Environment.WorldTransition'
HIDDEN_FOREGROUND_SOURCE = 'TheWorld.PersistentLevel.InterpActor_34.StaticMeshComponent_20'
# This optimized Sanctuary material is an HLS master: its Color and Luminosity
# texture parameters are combined by a stripped static permutation resource.
# Feeding the luminosity atlas directly to Base Color produces the observed
# neon green/magenta walkways.  The ordinary same-package instance has a
# recoverable diffuse atlas with the compatible material family and is a
# bounded visual fallback until that HLS graph is decoded.
REGULAR_DIFFUSE_FALLBACKS = {
    'Sanctuary_P:Prop_SancBuildings.Optimization.Mati_SancBuild4a':
        'Sanctuary_P:Prop_SancBuildings.Material.Mati_SancBuild4a',
}
# Inspected opaque materials whose stripped graphs defeat the generic _Dif
# heuristic: their cooked lists carry several _Dif overlays, so the sole-_Dif
# rule either picks a blend layer or gives up. Each entry names the texture
# that carries the surface's own color, and a native normal (and emissive)
# only when one survives in the cooked list. An entry keyed by a placed
# instance must name the inspected 'base' it was read against; the instance
# is only accepted when it adds no texture parameter of its own, which the
# caller already guarantees. Membership never recovers the layered graph.
INSPECTED_COLOR_FALLBACKS = {
    'Sanctuary_Land:Prop_Glacier.Materials.Mat_FrozenLake': {
        'method': 'frozen_lake_color_fallback_v1', 'label': 'Frozen-lake', 'noun': 'ice',
        'color': 'Sanctuary_Land:Prop_Glacier.Textures.FrozenLake',
        'normal': 'Sanctuary_Land:Prop_Skybox.MoveMe.Ice_Nrm',
        'omitted': ['vertex-painted snow/noise blend', 'reflection',
                    'native normal graph', 'glow', 'UV modulation'],
        'issue': 'Approximation: inspected FrozenLake texture used as ice color; layered graph and UV modulation not reconstructed'},
    'Sanctuary_Land:Env_Sanctuary.Materials.Mat_IceRoadSanctuary': {
        'method': 'ice_road_color_fallback_v1', 'label': 'Ice-road', 'noun': 'road',
        'color': 'Sanctuary_Land:Env_Ice.Textures.BrokenRoad_Dif',
        'normal': None,
        'omitted': ['BrokenRoad_Alpha snow/rock layer blend', 'noise and splatter overlays',
                    'stripped p_Normal texture', 'UV modulation'],
        'issue': 'Approximation: inspected BrokenRoad_Dif texture used as road color; snow/rock layer blend and stripped normal not reconstructed'},
    # The Hyperion station hanging in Sanctuary's sky. The placed instance
    # overrides nothing; its base keeps only tint/fog/rim/emissive-multiplier
    # constants, and the cooked list is the hull's own _Dif/_Nrm/_Emis plus a
    # Tiling_SmokePanner2_Dif overlay, so the sole-_Dif rule gave up and the
    # station rendered as the neutral gray (read as black against the sky).
    'Sanctuary_Light:Prop_MoonBase.Materials.Mati_MoonBase_02a': {
        'method': 'moon_base_color_fallback_v1', 'label': 'Moon-base', 'noun': 'hull',
        'base': 'Sanctuary_Light:Prop_MoonBase.Materials.Mat_MoonBase_02a',
        'color': 'Sanctuary_Light:Prop_MoonBase.Textures.MoonBase02a_Dif',
        'normal': 'Sanctuary_Light:Prop_MoonBase.Textures.MoonBase02a_Nrm',
        'emissive': 'Sanctuary_Light:Prop_MoonBase.Textures.MoonBase02a_Emis',
        'omitted': ['MoonBase_Color tint', 'Emissive_Mult scalar', 'Fog/Fog_Intensity blend',
                    'RimLight_Color', 'Tiling_SmokePanner2_Dif overlay', 'UV modulation'],
        'issue': 'Approximation: inspected MoonBase02a _Dif/_Nrm/_Emis textures used as hull color, normal and emissive; tint, fog, rim light, emissive multiplier and smoke overlay not reconstructed'},
}

# Inspected Unlit materials whose visible color is scaled by a named vector
# constant. The host multiplies the recovered Unlit color by the recorded
# value; everything else in the stripped graph stays omitted. Keyed by the
# placed material; the chain must end at the named base.
INSPECTED_UNLIT_COLOR_MULTIPLIERS = {
    # Elpis. Mat_Moon is additive Unlit with p_moonColor 4.02 gray; without
    # it the additive moon washes out against the dome. The H-shaped
    # MoonBase02_GRP mask, p_moonTimeBaseShadow, Moon_Comp relief and the
    # orange p_Basecolor2 are not placed by anything the package retains.
    'Sanctuary_Light:Prop_MoonBase.Materials.Mati_Moon': {
        'method': 'unlit_color_multiplier_v1',
        'base': 'Sanctuary_Light:Prop_MoonBase.Materials.Mat_Moon',
        'parameter': 'p_moonColor',
        'omitted': ['MoonBase02_GRP station shadow mask', 'p_moonTimeBaseShadow',
                    'Moon_Comp crater relief', 'p_Basecolor2 secondary color',
                    'p_DarkColor', 'Transition_Track time-of-day tint',
                    'p_moonTime/p_moonRotation UV motion']},
}


def native_skybox_mesh(identity):
    """Return whether an object identity is the observed native sky dome."""
    return identity.rsplit(':', 1)[-1] == NATIVE_SKYBOX_MESH


def native_skybox_placement(identity, effective_materials, materials):
    """Accept only the dome with entirely Unlit effective materials."""
    return (native_skybox_mesh(identity) and bool(effective_materials)
            and all(materials.get(name, {}).get('lighting_model') == 'MLM_Unlit'
                    for name in effective_materials))


def outer_shell_mesh(identity):
    """Return whether an object identity is one of the observed outer hull meshes."""
    return identity.rsplit(':', 1)[-1] in OUTER_SHELL_MESHES


def outer_shell_overrides(overrides, sections, materials):
    """Drop only `_Teleported` overrides whose mesh default recovered a diffuse.

    Returns the adjusted override list and a record of every replacement.
    Any other override, and any slot whose default has no diffuse, is kept as
    placed so the policy cannot turn an unrelated material into hull plating.
    """
    kept, replaced = list(overrides), []
    defaults = {section['slot']: section['material'] for section in sections}
    for slot, name in enumerate(overrides):
        if not name:
            continue
        source = materials.get(name, {}).get('source', '')
        if not source.rsplit('.', 1)[-1].endswith(OUTER_SHELL_OVERRIDE_SUFFIX):
            continue
        default = defaults.get(slot)
        if not default or not materials.get(default, {}).get('channels', {}).get('diffuse'):
            continue
        kept[slot] = None
        replaced.append({'slot': slot, 'override': source,
                         'default': materials[default]['source']})
    return kept, replaced


def apply_outer_shell_policy(actor, mesh_identity, sections, materials, enabled):
    """Set an actor's outer-shell fields from its placed overrides.

    `outer_shell_source_materials` always keeps the placed overrides so the
    policy can be re-applied or withdrawn by a later refresh.
    """
    if not outer_shell_mesh(mesh_identity):
        return
    placed = actor.get('outer_shell_source_materials', actor['materials'])
    actor['outer_shell_source_materials'] = list(placed)
    actor['outer_shell'] = bool(enabled)
    if enabled:
        actor['materials'], actor['outer_shell_replaced'] = outer_shell_overrides(
            placed, sections, materials)
    else:
        actor['materials'] = list(placed)
        actor.pop('outer_shell_replaced', None)


def hidden_visual_mesh(identity, effective_materials=None, materials=None, source=None):
    """Return whether an observed helper has no recoverable host-side visual."""
    mesh = identity.rsplit(':', 1)[-1]
    if source == HIDDEN_FOREGROUND_SOURCE:
        # This exact observed BoxLrg placement is a developer blocking volume
        # directly in the Sanctuary start view, despite carrying a prop mesh.
        return True
    if mesh == HIDDEN_COLLISION_MESH:
        # CollisionCube is source collision geometry, not a renderable prop.
        return True
    if mesh == HIDDEN_VISUAL_MESH:
        # Blocking_Cube is an observed placement helper even when an
        # unreliable diffuse override was attached to it.
        return True
    if mesh == HIDDEN_CLOUD_MESH and effective_materials and materials:
        # The cloud material is translucent in UE3 but its opacity graph is not
        # recovered; drawing its diffuse alone produces the observed yellow /
        # black blocking planes. Hide only the exact observed cloud instances.
        return all(materials.get(name, {}).get('source') in HIDDEN_CLOUD_MATERIALS
                   for name in effective_materials)
    if mesh == HIDDEN_TRANSITION_MESH and effective_materials and materials:
        # WorldTransition is a translucent loading/boundary plane. Its graph
        # is not recovered; feeding the undecoded material into a host mesh
        # renders an opaque helper plane. The separate lower IcePlate surfaces
        # are legitimate geometry, not these transition helpers. Hide only
        # this exact helper mesh/material pair and keep its source collision
        # state independent.
        return all((materials.get(name, {}).get('source') or '').rsplit(':', 1)[-1]
                   == HIDDEN_TRANSITION_MATERIAL
                   for name in effective_materials)
    return False
ALIASES = {
    'p_dif': 'diffuse', 'dif': 'diffuse', 'ad_diff': 'diffuse',
    'tex_diff': 'diffuse', 'diff_texture': 'diffuse',
    'p_snowdiffuse': 'diffuse', 'diffuse_a': 'diffuse',
    'diffuse_b': 'diffuse', 'diffuse_c': 'diffuse',
    'p_nrm': 'normal', 'nrm': 'normal', 'normalmap': 'normal',
    'tex_spec': 'specular',
    'emissive_gp': 'emissive', 'emissiveopacityinput': 'emissive',
}


def channel_for_parameter(parameter):
    name = str(parameter).casefold()
    return CHANNELS.get(name) or ALIASES.get(name)


def parameter_priority(parameter):
    """Prefer a direct diffuse override over a shared master default."""
    name = str(parameter).casefold()
    if name in ('p_diffuse', 'diffuse'):
        return 100
    if name in ('diffuse_a', 'diffuse_b', 'diffuse_c', 'p_dif', 'dif', 'ad_diff', 'tex_diff'):
        return 90
    if name == 'p_snowdiffuse':
        return 50
    return 80


def unnamed_diffuse_candidate(parameters, identity):
    """Conservative Material v1 approximation for stripped cooked graphs.

    Sanctuary's Mat_SancBuild_Colorized retains a single unnamed sample pointing
    to SancBuild1a_Dif_04. Require that exact naming pattern and uniqueness;
    never replace a named diffuse channel (including an explicit null override).
    """
    if any(channel_for_parameter(name) == 'diffuse' for name in parameters):
        return None
    candidates = {key for name, key in parameters.items() if key is not None
                  and re.fullmatch(r'materialexpressiontexturesampleparameter2d_\d+', name)
                  and re.search(r'_dif(?:_\d+)?$', identity(key), re.IGNORECASE)}
    return next(iter(candidates)) if len(candidates) == 1 else None


DIFFUSE_SUFFIX = re.compile(r'_diff?(?:_\d+)?$', re.IGNORECASE)
# Cooked-resource texture names that are not plausible diffuse sources: normal
# maps, packed composite/specular/emissive channels, masks, gray noise tiles,
# and detail/roughness/height/opacity-style utility maps that would otherwise
# become a wrong BaseColor with view-dependent shading.
AUXILIARY_TEXTURE = re.compile(
    r'(_n|_nm|_nrm|normal|_gray|_grey|_hs|_spec|_emis|_emissive|_alpha|_mask|_comp|_lm|noise|_cube'
    r'|_detail|_rough(?:ness)?|_height|_bump|_opacity|_illum|_lightmap|_gloss|_metal(?:lic)?'
    r'|_ao|_cavity|_disp(?:lacement)?|_refl(?:ection)?|_env)(?:_\d+)?$',
    re.IGNORECASE)


def cooked_diffuse_candidate(textures, identity, texture_class, blend_mode='BLEND_Opaque'):
    """Pick a diffuse texture from a cooked material's texture list.

    A unique *_Dif/_Diff Texture2D wins. Otherwise, for opaque and masked
    materials only, a unique Texture2D whose name does not mark it as an
    auxiliary channel is used; a translucent material's diffuse alpha would
    become its opacity, and a guessed opacity is worse than the invisible
    fallback. Both are recorded as approximations; the graph, UV mapping and
    tint remain unreconstructed.
    """
    planar = [t for t in textures if texture_class(t) == 'Texture2D']
    named = {t for t in planar if DIFFUSE_SUFFIX.search(identity(t))}
    if len(named) == 1:
        return next(iter(named)), 'sole_cooked_resource_dif_texture'
    if named or blend_mode not in ('BLEND_Opaque', 'BLEND_Masked'):
        return None, None
    plain = {t for t in planar if not AUXILIARY_TEXTURE.search(identity(t))}
    if len(plain) == 1:
        return next(iter(plain)), 'sole_cooked_resource_texture'
    return None, None


def unconnected_diffuse_constant(material_props):
    """Return the constant colour of a Material whose DiffuseColor input was
    never connected, or None when that cannot be established.

    Cooked 832/46 materials strip their expression graphs, so the input's
    Expression reference is always null. The observed distinction is the Mask
    flags: an input that had an expression keeps Mask/MaskR/G/B, an input that
    never had one carries neither. UE3 then evaluates the input as its
    Constant, which defaults to black. Observed on Master_Black; not a general
    format guarantee.
    """
    entries = material_props.get('DiffuseColor')
    if not isinstance(entries, list):
        return None
    fields = {e.get('name'): e.get('value') for e in entries if isinstance(e, dict)}
    expression = fields.get('Expression')
    if isinstance(expression, dict) and expression.get('index'):
        return None
    if any(fields.get(mask) for mask in ('Mask', 'MaskR', 'MaskG', 'MaskB', 'MaskA')):
        return None
    constant = fields.get('Constant')
    if isinstance(constant, dict):
        return [float(constant.get(c, 0)) for c in ('R', 'G', 'B')]
    return [0.0, 0.0, 0.0]


def material_index(records, path):
    matches = [index for index, record in records.items() if record['path'] == path
               and record['class'].rsplit('.', 1)[-1] in
               ('Material', 'MaterialInstanceConstant', 'MaterialInstanceTimeVarying')]
    if len(matches) != 1:
        raise ValueError('Material identity is missing or ambiguous: ' + path)
    return matches[0]


def cooked_texture_references(payload, data):
    """Read the observed 832/46 Material resource prefix, never scan offsets.

    Only empty compile-error/dependency arrays are supported. The remaining
    resource/shader bytes are deliberately opaque. See DECISIONS.md.
    """
    offset = data['property_offset'] + data['consumed_bytes']
    if offset < 0 or offset > len(payload) or len(payload) - offset != data['trailing_bytes']:
        raise ValueError('Cooked material payload/property boundary mismatch')
    tail = memoryview(payload)[offset:]
    if len(tail) < 36:
        raise ValueError('Truncated cooked material resource prefix')
    if struct.unpack_from('<2i', tail) != (0, 0):
        raise ValueError('Unsupported cooked material error/dependency arrays')
    # Two empty arrays, resource int, GUID, resource int, texture-array count.
    count = struct.unpack_from('<i', tail, 32)[0]
    if count < 0 or count > (len(tail) - 36) // 4:
        raise ValueError('Invalid cooked material texture count')
    refs = list(struct.unpack_from(f'<{count}i', tail, 36))
    return refs, len(tail) - 36 - count * 4


class Scene:
    def __init__(self, reader, game, output, include_dlc=False, outer_shell=False,
                 matinee_first_key=False):
        self.reader, self.game, self.output = reader.resolve(), game.resolve(), output.resolve()
        self.cooked = self.game / 'WillowGame/CookedPCConsole'
        self.include_dlc = include_dlc
        self.outer_shell = outer_shell
        self.matinee_first_key = matinee_first_key
        self.package_root = self.game if include_dlc else self.cooked
        self.schema = Path(__file__).with_name('level-arrays.schema')
        self.packages = {}
        self.texture_caches = {}
        for path in content_files(self.game, include_dlc):
            if path.suffix.lower() in PACKAGE_SUFFIXES:
                self.packages.setdefault(path.stem.casefold(), []).append(path)
            elif path.suffix.lower() == '.tfc':
                self.texture_caches.setdefault(path.name.casefold(), []).append(path)
        self.records, self.materials, self.meshes, self.textures = {}, {}, {}, {}
        self.resolved, self.imports = {}, {}
        self.issues = []
        self.output.mkdir(parents=True, exist_ok=True)

    def package(self, name):
        paths = self.packages.get(name.casefold(), [])
        if len(paths) != 1:
            raise ValueError(f'Expected unique package {name}, found {len(paths)}')
        return paths[0]

    def call(self, package, *args):
        run = subprocess.run([str(self.reader), str(self.package(package)), *map(str, args)],
                             capture_output=True, text=True, encoding='utf-8')
        if run.returncode:
            raise ValueError(run.stderr.strip())
        return json.loads(run.stdout)

    def load(self, package):
        if package not in self.records:
            self.records[package] = {r['index']: r for r in self.call(package, '--scene-records', self.schema)}
        return self.records[package]

    def resolve(self, package, ref):
        index = ref.get('index', 0) if isinstance(ref, dict) else ref
        if not index:
            return None
        if index > 0:
            return package, index
        if (package, index) in self.resolved:
            return self.resolved[package, index]
        path = ref.get('path') if isinstance(ref, dict) else None
        if package not in self.imports:
            self.imports[package] = {r['index']: r for r in self.call(package, '--imports')}
        metadata = self.imports[package].get(index, {})
        path = path or metadata.get('path')
        expected_class = metadata.get('class_name')
        if path:
            for loaded, records in self.records.items():
                match = next((r for r in records.values() if r['path'].casefold() == path.casefold()
                              and (not expected_class or r['class'].rsplit('.', 1)[-1] == expected_class)), None)
                if match:
                    self.resolved[package, index] = loaded, match['index']
                    return loaded, match['index']
        # Shared base resources retain the established lookup order in DLC
        # mode. Only an absent target expands the search to the full install.
        try:
            r = self.call(package, '--resolve', index, '--cooked', self.cooked)
        except ValueError as error:
            if not self.include_dlc or 'not found:' not in str(error):
                raise
            r = self.call(package, '--resolve', index, '--cooked', self.package_root)
        self.resolved[package, index] = r['resolved_package'], r['resolved_index']
        return self.resolved[package, index]

    def identity(self, key):
        return key[0] + ':' + self.load(key[0])[key[1]]['path']

    def filename(self, key, suffix):
        return hashlib.sha256(self.identity(key).encode()).hexdigest()[:24] + suffix

    def issue(self, context, error):
        self.issues.append({'object': context, 'error': str(error)})

    def material_parameters(self, key, stack=()):
        if key in stack or len(stack) >= 32:
            raise ValueError('Material parent cycle/depth limit')
        p = props(self.load(key[0])[key[1]])
        parent = self.resolve(key[0], p.get('Parent', 0))
        parameters = self.material_parameters(parent, (*stack, key)) if parent else {}
        # Base-material named texture expressions supply defaults.
        for ref in p.get('Expressions', []):
            expr = self.resolve(key[0], ref)
            if not expr:
                continue
            expression = self.load(expr[0])[expr[1]]
            if 'TextureSampleParameter' not in expression.get('class', ''):
                continue
            e = props(expression)
            name = e.get('ParameterName', '').casefold()
            if (channel_for_parameter(name) or re.fullmatch(r'materialexpressiontexturesampleparameter2d_\d+', name)) and e.get('Texture', {}).get('index'):
                parameters[name] = self.resolve(expr[0], e['Texture'])
        for entry in p.get('TextureParameterValues', []):
            e = values(entry)
            name = e.get('ParameterName', '').casefold()
            if channel_for_parameter(name) or re.fullmatch(r'materialexpressiontexturesampleparameter2d_\d+', name):
                parameters[name] = self.resolve(key[0], e.get('ParameterValue', 0))
        return parameters

    def named_chain_parameters(self, key):
        """Every named sampler/scalar/vector input along a material chain.

        Unlike the Material v1 channel allow-list this keeps all names, with
        instance overrides applied over base defaults. Returns the base
        material key with the three parameter maps (names casefolded).
        """
        chain, current = [], key
        while current is not None:
            if current in chain or len(chain) >= 32:
                raise ValueError('Material parent cycle/depth limit')
            chain.append(current)
            current = self.resolve(current[0], props(self.load(current[0])[current[1]]).get('Parent', 0))
        samplers, scalars, vectors = {}, {}, {}
        for item in reversed(chain):
            p = props(self.load(item[0])[item[1]])
            for ref in p.get('Expressions', []):
                expr = self.resolve(item[0], ref)
                if not expr:
                    continue
                record = self.load(expr[0])[expr[1]]
                cls = record.get('class', '').rsplit('.', 1)[-1]
                if 'Parameter' not in cls or 'error' in record:
                    continue
                e = props(record)
                name = e.get('ParameterName', '').casefold()
                if not name:
                    continue
                if 'TextureSampleParameter' in cls:
                    samplers[name] = self.resolve(expr[0], e.get('Texture', 0))
                elif cls == 'MaterialExpressionScalarParameter':
                    scalars[name] = e.get('DefaultValue')
                elif cls == 'MaterialExpressionVectorParameter':
                    vectors[name] = e.get('DefaultValue')
            for entry in p.get('TextureParameterValues', []):
                e = values(entry)
                samplers[e.get('ParameterName', '').casefold()] = self.resolve(item[0], e.get('ParameterValue', 0))
            for entry in p.get('ScalarParameterValues', []):
                e = values(entry)
                scalars[e.get('ParameterName', '').casefold()] = e.get('ParameterValue')
            for entry in p.get('VectorParameterValues', []):
                e = values(entry)
                vectors[e.get('ParameterName', '').casefold()] = e.get('ParameterValue')
        return chain[-1], samplers, scalars, vectors

    def unlit_color_multiplier(self, key, material):
        """Recorded RGB multiplier for an inspected Unlit chain, or None.

        The value is the named vector constant with instance overrides last.
        It is recorded with its provenance; the host applies it to the
        visible Unlit color. This does not reconstruct the stripped graph.
        """
        entry = INSPECTED_UNLIT_COLOR_MULTIPLIERS.get(self.identity(key))
        if entry is None:
            return None
        if material.get('lighting_model') != 'MLM_Unlit':
            raise ValueError('Unlit color multiplier requires an Unlit chain')
        base, _, _, vectors = self.named_chain_parameters(key)
        if self.identity(base) != entry['base']:
            raise ValueError('Unlit color multiplier requires the inspected base material')
        value = vectors.get(entry['parameter'].casefold())
        if not isinstance(value, dict) or not all(
                isinstance(value.get(c), (int, float)) and not isinstance(value.get(c), bool)
                and math.isfinite(value[c]) and value[c] >= 0 for c in 'RGB'):
            raise ValueError(f"Unlit color multiplier {entry['parameter']} is missing or invalid")
        return {'method': entry['method'], 'status': 'partial_unverified',
                'parameter': entry['parameter'], 'source_material': self.identity(base),
                'rgb': [float(value[c]) for c in 'RGB'], 'omitted': entry['omitted']}

    def native_sky_approximation(self, key):
        """Named inputs of the observed time-of-day sky master, or None.

        The cooked master graph is stripped; only parameter names and the
        instance values survive. The returned record tells the host which
        textures and constants to combine and states the assumptions. It is
        an approximation policy, not a reconstruction of the UE3 sky graph.
        """
        base, samplers, scalars, vectors = self.named_chain_parameters(key)
        if self.identity(base).split(':', 1)[1] != NATIVE_SKY_MASTER:
            return None
        textures = {}
        for parameter, role in SKY_SAMPLERS.items():
            texture = samplers.get(parameter)
            if texture is None:
                raise ValueError(f'Sky master input {parameter} is missing')
            if self.load(texture[0])[texture[1]]['class'].rsplit('.', 1)[-1] != 'Texture2D':
                raise ValueError(f'Sky master input {parameter} is not a Texture2D')
            textures[role] = {'source': self.identity(texture),
                              'file': self.texture(texture, 'sky_' + role)}
        constants = {}
        for parameter, role in SKY_SCALARS.items():
            value = scalars.get(parameter)
            if not isinstance(value, (int, float)) or isinstance(value, bool) or not math.isfinite(value):
                raise ValueError(f'Sky master scalar {parameter} is missing or invalid')
            constants[role] = float(value)
        colors = {}
        for parameter, role in SKY_VECTORS.items():
            value = vectors.get(parameter)
            if not isinstance(value, dict) or not all(
                    isinstance(value.get(c), (int, float)) and math.isfinite(value[c]) for c in 'RGBA'):
                raise ValueError(f'Sky master vector {parameter} is missing or invalid')
            colors[role] = [float(value[c]) for c in 'RGBA']
        if not 0 <= constants['time_of_day'] < SKY_TIME_AXIS:
            raise ValueError('Time_of_Day is outside the transition strip')
        return {'method': 'sky_time_of_day_strip_v1', 'status': 'partial_unverified',
                'master': self.identity(base), 'textures': textures,
                'scalars': constants, 'vectors': colors,
                'time_axis': {'divisor': SKY_TIME_AXIS,
                              'column_u': constants['time_of_day'] / SKY_TIME_AXIS,
                              'note': 'UNVERIFIED: Time_of_Day read as a strip pixel column'},
                'horizon_row_v': SKY_HORIZON_ROW,
                'cloud_channel': 'R',
                'omitted': ['sun spot (Sun_spot_brightness)', 'Masks (stars, cap gradient)',
                            'Horizion_track_color_multiplier', 'cloud channels G/B',
                            'cloud motion', 'time-of-day animation', 'Kismet control']}

    def material_metadata(self, key, stack=()):
        """Carry safe UE3 blend/shading flags without importing full graphs."""
        if key in stack or len(stack) >= 32:
            raise ValueError('Material parent cycle/depth limit')
        p = props(self.load(key[0])[key[1]])
        parent = self.resolve(key[0], p.get('Parent', 0))
        metadata = self.material_metadata(parent, (*stack, key)) if parent else {}
        if p.get('BlendMode'):
            metadata['blend_mode'] = p['BlendMode']
        if p.get('LightingModel'):
            metadata['lighting_model'] = p['LightingModel']
        if 'TwoSided' in p:
            metadata['two_sided'] = bool(p['TwoSided'])
        # A translucent/masked base graph has an opacity input even when its
        # supported texture channels are outside Material v1.
        opacity = p.get('Opacity')
        if isinstance(opacity, list):
            metadata['has_opacity'] = any(
                item.get('name') == 'Expression'
                and isinstance(item.get('value'), dict)
                and item['value'].get('index')
                for item in opacity if isinstance(item, dict))
        return metadata

    def glacier_primary_surface(self, key, base, textures):
        """Explicit primary-layer approximation, never a recovered snow shader.

        Restricted to the two inspected Sanctuary family members and the exact
        resource texture set. UV0 is an approximation: native static permutation
        data and the stripped blend graph are not interpreted by this policy.
        """
        base_path = 'Prop_Glacier.Materials.Mat_Glacier'
        if self.identity(base).split(':', 1)[1] != base_path:
            return None
        if self.identity(key).split(':', 1)[1] not in (
                base_path, 'Prop_Glacier.Materials.Mati_Glacier2x'):
            return None
        expected = {'Prop_Glacier.Textures.GlacierFront_Dif',
                    'Prop_Glacier.Textures.GlacierFront_Nrm',
                    'Prop_Terrain.Textures.Snow_Dif',
                    'Prop_Skybox.MoveMe.R2Tex_SnowTempCubeStaticNegY'}
        by_path = {self.identity(t).split(':', 1)[1]: t for t in textures}
        if len(textures) != 4 or set(by_path) != expected:
            return None
        if any(self.load(t[0])[t[1]]['class'].rsplit('.', 1)[-1] != 'Texture2D'
               for t in textures):
            return None
        name = 'p_texscalar_rgmain_basnow'
        scale = None
        for ref in props(self.load(base[0])[base[1]]).get('Expressions', []):
            expression = self.resolve(base[0], ref)
            if expression is None:
                continue
            row = self.load(expression[0])[expression[1]]
            p = props(row)
            if p.get('ParameterName', '').casefold() == name:
                if row['class'].rsplit('.', 1)[-1] != 'MaterialExpressionVectorParameter' or scale is not None:
                    raise ValueError('Ambiguous glacier primary-layer scale')
                scale = p.get('DefaultValue')
        chain, current = [], key
        while current != base:
            if current in chain or len(chain) >= 32:
                raise ValueError('Glacier parent cycle/depth limit')
            chain.append(current)
            current = self.resolve(current[0], props(self.load(current[0])[current[1]]).get('Parent', 0))
            if current is None:
                raise ValueError('Glacier parent does not reach inspected base')
        for instance in reversed(chain):
            overrides = [values(e).get('ParameterValue') for e in
                         props(self.load(instance[0])[instance[1]]).get('VectorParameterValues', [])
                         if values(e).get('ParameterName', '').casefold() == name]
            if len(overrides) > 1:
                raise ValueError('Duplicate glacier scale override')
            if overrides:
                scale = overrides[0]
        if not isinstance(scale, dict) or not all(
                isinstance(scale.get(c), (int, float)) and math.isfinite(scale[c]) for c in 'RGBA'):
            raise ValueError('Missing or invalid glacier primary-layer scale')
        return {'diffuse': by_path['Prop_Glacier.Textures.GlacierFront_Dif'],
                'normal': by_path['Prop_Glacier.Textures.GlacierFront_Nrm'],
                'scale': [float(scale[c]) for c in 'RGBA']}

    def cooked_material_textures(self, key, stack=()):
        if key in stack or len(stack) >= 32:
            raise ValueError('Material parent cycle/depth limit')
        record = self.load(key[0])[key[1]]
        if record['class'].rsplit('.', 1)[-1] != 'Material':
            parent = self.resolve(key[0], props(record).get('Parent', 0))
            return self.cooked_material_textures(parent, (*stack, key)) if parent else None
        refs, opaque = cooked_texture_references(
            bytes(self.call(key[0], '--payload', key[1])), record['data'])
        textures = []
        for ref in refs:
            texture = self.resolve(key[0], ref)
            if texture is None:
                continue
            target = self.load(texture[0]).get(texture[1])
            if target is None or target['class'].rsplit('.', 1)[-1] not in ('Texture2D', 'TextureCube'):
                raise ValueError('Cooked material reference is not a supported texture')
            textures.append(texture)
        return key, textures, opaque

    def texture(self, key, channel):
        if key is None:
            return None
        cache_key = (*key, channel)
        if cache_key not in self.textures:
            filename = self.filename(key, '_' + channel + '.png')
            tfc_root = self.cooked
            if self.include_dlc:
                cache = props(self.load(key[0])[key[1]]).get('TextureFileCacheName', 'None')
                cache_file = cache if cache.lower().endswith('.tfc') else cache + '.tfc'
                tfc_root = cache_directory(self.texture_caches.get(cache_file.casefold(), []),
                                           self.package(key[0]), self.cooked)
            self.call(key[0], '--texture', key[1], '--property-offset', 4,
                      '--output', self.output / filename, '--tfc', tfc_root)
            self.textures[cache_key] = filename
        return self.textures[cache_key]

    def material(self, key):
        if key is None:
            return None
        name = self.filename(key, '')
        if name not in self.materials:
            material = {'source': self.identity(key), 'channels': {}}
            self.materials[name] = material
            try:
                material.update(self.material_metadata(key))
                parameters = self.material_parameters(key)
                if material.get('lighting_model') == 'MLM_Unlit':
                    # The ordinary diffuse inference below still runs and
                    # stays as the host's fallback when this record is absent.
                    try:
                        sky = self.native_sky_approximation(key)
                    except ValueError as error:
                        sky = None
                        self.issue(material['source'] + ':sky', error)
                    if sky is not None:
                        material['sky_approximation'] = sky
                        self.issue(material['source'],
                                   'Approximation: Time_of_Day column of the transition strip over dome V with a horizon-tinted cloud layer; sky master graph not reconstructed')
                if self.identity(key) in INSPECTED_UNLIT_COLOR_MULTIPLIERS:
                    try:
                        multiplier = self.unlit_color_multiplier(key, material)
                    except ValueError as error:
                        multiplier = None
                        self.issue(material['source'] + ':multiplier', error)
                    if multiplier is not None:
                        material['unlit_color_multiplier'] = multiplier
                        self.issue(material['source'],
                                   f"Approximation: Unlit color scaled by {multiplier['parameter']}; shadow mask, relief and secondary color not reconstructed")
                regular_fallback = REGULAR_DIFFUSE_FALLBACKS.get(material['source'])
                if regular_fallback is not None:
                    parent = self.resolve(key[0], props(self.load(key[0])[key[1]]).get('Parent', 0))
                    if parent is None or self.identity(parent) != 'Sanctuary_P:Prop_SancBuildings.Optimization.Sanc_HLS_Master':
                        raise ValueError('Regular diffuse fallback requires the inspected HLS parent')
                    package, path = regular_fallback.split(':', 1)
                    if package != key[0]:
                        raise ValueError('Regular diffuse fallback package mismatch')
                    regular_index = material_index(self.load(package), path)
                    regular_name = self.material((package, regular_index))
                    regular = self.materials[regular_name]
                    diffuse = regular.get('channels', {}).get('diffuse')
                    if not diffuse:
                        raise ValueError('Regular diffuse fallback has no diffuse channel')
                    candidates = [i for i, row in self.load(key[0]).items()
                                  if row['path'] == 'Prop_SancBuildings.Textures.SancBuild4a_Dif'
                                  and row['class'].rsplit('.', 1)[-1] == 'Texture2D']
                    if len(candidates) != 1:
                        raise ValueError('Regular diffuse fallback requires a unique concrete atlas')
                    expected = (key[0], candidates[0])
                    if diffuse != self.filename(expected, '_diffuse.png'):
                        raise ValueError('Regular diffuse fallback requires the inspected concrete atlas')
                    material['channels']['diffuse'] = diffuse
                    material['diffuse_inference'] = regular['source']
                    material['diffuse_inference_method'] = 'same_package_regular_diffuse_fallback_v1'
                    material['surface_approximation'] = {
                        'method': 'same_package_regular_diffuse_fallback_v1',
                        'status': 'partial_unverified',
                        'source_material': regular['source'],
                        'source_texture': self.identity(expected),
                        'uv_selection': 'UV0 unchanged; regular atlas approximation',
                        'omitted': ['HLS Color/Luminosity combine',
                                    'static permutation', 'modulation']}
                    self.issue(material['source'],
                               'Approximation: HLS Color/Luminosity graph replaced by same-package regular diffuse; static permutation and modulation not reconstructed')
                    # Only replace diffuse; retain any supported explicit channels.
                    parameters = {p: t for p, t in parameters.items()
                                  if channel_for_parameter(p) != 'diffuse'}
                inferred = None if regular_fallback else unnamed_diffuse_candidate(parameters, self.identity)
                if inferred is not None:
                    parameters['p_diffuse'] = inferred
                    material['diffuse_inference'] = self.identity(inferred)
                    self.issue(material['source'], 'Approximation: sole unnamed _Dif texture used as diffuse; cooked graph and tint not reconstructed')
                if not regular_fallback and not any(channel_for_parameter(p) == 'diffuse' for p in parameters):
                    try:
                        cooked = self.cooked_material_textures(key)
                        if cooked is not None:
                            base, textures, opaque = cooked
                            material['cooked_texture_resource'] = {
                                'source': self.identity(base),
                                'textures': [self.identity(t) for t in textures],
                                'opaque_tail_bytes': opaque}
                            inferred, method = cooked_diffuse_candidate(
                                textures, self.identity,
                                lambda t: self.load(t[0])[t[1]]['class'].rsplit('.', 1)[-1],
                                material.get('blend_mode', 'BLEND_Opaque'))
                            fallback = (INSPECTED_COLOR_FALLBACKS.get(self.identity(key))
                                        if material.get('blend_mode', 'BLEND_Opaque') == 'BLEND_Opaque'
                                        else None)
                            if fallback is not None:
                                # A base-keyed entry applies to that base only; an
                                # instance-keyed entry to the inspected parent only.
                                if self.identity(base) != fallback.get('base', self.identity(key)):
                                    raise ValueError(f"{fallback['label']} fallback requires the inspected base material")
                                def inspected(identity, kind):
                                    found = [t for t in textures if self.identity(t) == identity
                                             and self.load(t[0])[t[1]]['class'].rsplit('.', 1)[-1] == 'Texture2D']
                                    if len(found) != 1:
                                        raise ValueError(f"{fallback['label']} fallback requires the inspected {fallback['noun']} {kind}")
                                    return found[0]
                                inferred, method = inspected(fallback['color'], 'texture'), fallback['method']
                                normal = inspected(fallback['normal'], 'normal') if fallback['normal'] else None
                                if normal is not None and not any(channel_for_parameter(p) == 'normal' for p in parameters):
                                    parameters['p_normal'] = normal
                                emissive = inspected(fallback['emissive'], 'emissive') if fallback.get('emissive') else None
                                if emissive is not None and not any(channel_for_parameter(p) == 'emissive' for p in parameters):
                                    parameters['p_emissive'] = emissive
                                material['surface_approximation'] = {
                                    'method': method, 'status': 'partial_unverified',
                                    'source_texture': self.identity(inferred),
                                    'uv_selection': 'UV0 unchanged; native UV modulation unverified',
                                    'omitted': fallback['omitted']}
                                if normal is not None:
                                    material['surface_approximation']['normal_texture'] = self.identity(normal)
                                if emissive is not None:
                                    material['surface_approximation']['emissive_texture'] = self.identity(emissive)
                            glacier = (self.glacier_primary_surface(key, base, textures)
                                       if inferred is None and material.get('blend_mode', 'BLEND_Opaque') == 'BLEND_Opaque'
                                       else None)
                            if glacier is not None:
                                parameters['p_diffuse'] = glacier['diffuse']
                                glacier_channels = ['diffuse']
                                # Preserve any explicit normal parameter, including a null override.
                                if not any(channel_for_parameter(p) == 'normal' for p in parameters):
                                    parameters['p_normal'] = glacier['normal']
                                    glacier_channels.append('normal')
                                material['surface_approximation'] = {
                                    'method': 'glacier_primary_layer_v1',
                                    'status': 'partial_unverified',
                                    'source_scale_parameter': 'P_TexScalar_RGMain_BASnow',
                                    'source_scale': glacier['scale'],
                                    'omitted': ['snow_blend', 'reflection', 'glow'],
                                    'uv_selection': 'UV0 approximation; static permutation not decoded'}
                                material['channel_uv'] = {
                                    channel: {'index': 0, 'scale': glacier['scale'][:2]}
                                    for channel in glacier_channels}
                                self.issue(material['source'], 'Approximation: glacier primary diffuse/normal layer with retained tiling on UV0; snow blend, reflection, glow and static UV selection unverified')
                            if inferred is not None:
                                parameters['p_diffuse'] = inferred
                                material['diffuse_inference'] = self.identity(inferred)
                                material['diffuse_inference_method'] = method
                                self.issue(material['source'], fallback['issue']
                                           if fallback is not None and method == fallback['method'] else
                                           'Approximation: sole cooked resource _Dif texture used as diffuse; graph, UV mapping and tint not reconstructed'
                                           if method == 'sole_cooked_resource_dif_texture' else
                                           'Approximation: sole non-auxiliary cooked resource texture used as diffuse; graph, UV mapping and tint not reconstructed')
                            elif not textures and material.get('blend_mode', 'BLEND_Opaque') == 'BLEND_Opaque':
                                constant = unconnected_diffuse_constant(props(self.load(base[0])[base[1]]))
                                if constant is not None:
                                    material['constant_diffuse'] = constant
                                    self.issue(material['source'], 'Approximation: unconnected DiffuseColor input rendered as its constant; no cooked textures')
                    except ValueError as error:
                        self.issue(material['source'], error)
                for parameter, texture in parameters.items():
                    channel = channel_for_parameter(parameter)
                    if not channel or texture is None:
                        continue
                    try:
                        filename = self.texture(texture, channel)
                        current = material['channels'].get(channel)
                        # Parent defaults often point at StubGray. A concrete
                        # child override wins, while an alias never replaces a
                        # stronger exact parameter that was already selected.
                        priority = parameter_priority(parameter)
                        current_priority = material.setdefault('_channel_priority', {}).get(channel, -1)
                        is_stub = 'common_textures.stub.' in self.identity(texture).casefold()
                        current_is_stub = material.setdefault('_channel_stub', {}).get(channel, False)
                        if (current is None or (current_is_stub and not is_stub)
                                or (current_is_stub == is_stub and priority > current_priority)):
                            material['channels'][channel] = filename
                            material['_channel_priority'][channel] = priority
                            material['_channel_stub'][channel] = is_stub
                    except ValueError as error:
                        # A failed channel decode must not leave inference
                        # metadata claiming a diffuse that never decoded: the
                        # material renders the neutral fallback, so the record
                        # must say so too.
                        if channel == 'diffuse':
                            material.pop('diffuse_inference', None)
                            material.pop('diffuse_inference_method', None)
                            material.pop('surface_approximation', None)
                        self.issue(material['source'] + ':' + channel, error)
                material.pop('_channel_priority', None)
                material.pop('_channel_stub', None)
                if not any(material['channels'].values()) and 'constant_diffuse' not in material:
                    self.issue(material['source'], 'No supported named Material v1 texture parameters; neutral fallback')
            except ValueError as error:
                self.issue(material['source'], error)
        return name

    def mesh(self, key):
        name = self.filename(key, '')
        if name not in self.meshes:
            filename = name + '.obj'
            data = self.call(key[0], '--mesh', key[1], '--property-offset', 4, '--output', self.output / filename)
            # One OBJ per section preserves material slots without relying on FBX
            # importer's OBJ group merging or generated material-name order.
            lines = (self.output / filename).read_text().splitlines(keepends=True)
            header, groups = [], []
            for line in lines:
                if line.startswith('g '):
                    groups.append([])
                elif groups:
                    groups[-1].append(line)
                else:
                    header.append(line)
            if len(groups) != len(data['sections']):
                raise ValueError('Mesh section count differs from OBJ')
            sections = []
            for i, (group, section) in enumerate(zip(groups, data['sections'])):
                if not section['triangles']:
                    continue
                file = f'{name}_s{i}.obj'
                (self.output / file).write_text(''.join(header + group))
                sections.append({'slot': i, 'file': file,
                                 'material': self.material(self.resolve(key[0], section['material_index']))})
            collision = {'status': 'absent', 'hulls': []}
            body = self.resolve(key[0], data.get('body_setup', 0))
            if body:
                try:
                    record = self.load(body[0])[body[1]]
                    if record['class'] != 'Engine.RB_BodySetup' or 'error' in record:
                        raise ValueError('Invalid collision body record')
                    collision = {'status': 'supported', 'source': self.identity(body),
                                 'hulls': collision_hulls(record['data']['properties'])}
                except (ValueError, KeyError) as error:
                    collision = {'status': 'unsupported', 'hulls': [], 'reason': str(error)}
                    self.issue(self.identity(key) + ':collision', error)
            self.meshes[name] = {'source': self.identity(key), 'sections': sections, 'collision': collision}
        return name

    def levels(self, persistent):
        """Persistent level plus every streamed sublevel it names, in load order."""
        levels, pending, seen = [], [persistent], set()
        while pending:
            name = pending.pop(0)
            if name.casefold() in seen:
                continue
            seen.add(name.casefold())
            records = self.load(name)
            levels.append(name)
            for record in records.values():
                if record['class'].startswith('Engine.LevelStreaming'):
                    p = props(record)
                    sublevel = p.get('PackageName')
                    if sublevel and sublevel != 'None':
                        self.package(sublevel)  # missing dependencies are fatal
                        pending.append(sublevel)
        return levels

    def build(self, persistent):
        self.load('Startup')
        levels = self.levels(persistent)
        actors, camera = [], None
        matinee_first_key_applied = 0
        matinee_first_key = getattr(self, 'matinee_first_key', False)
        for level in levels:
            print(f'Preparing {level}', flush=True)
            records = self.load(level)
            placements = {}
            for record in records.values():
                if record['class'] == 'Engine.StaticMeshCollectionActor':
                    p = props(record)
                    refs = p.get('StaticMeshComponents', [])
                    payload = bytes(self.call(level, '--payload', record['index']))
                    transforms = collection_transforms(payload, record['data'], len(refs))
                    for ref, pose in zip(refs, transforms):
                        if ref['index'] in placements:
                            raise ValueError('Duplicate collection component')
                        placements[ref['index']] = pose
                elif 'PlayerStart' in record['class'] and camera is None:
                    camera = transform(props(record))
                    camera['location'][2] += 100
            for record in records.values():
                if record['class'] != 'Engine.StaticMeshComponent' or not record['path'].startswith('TheWorld.PersistentLevel.'):
                    continue
                try:
                    p = props(record)
                    key = self.resolve(level, p.get('StaticMesh', 0))
                    if not key:
                        continue
                    pose = placements.get(record['index'])
                    if pose is not None:
                        pose = dict(pose, scale=collection_scale(p, pose['scale']))
                    if pose is None:
                        owner = records.get(record['outer'])
                        if not owner or owner['class'] not in ('Engine.StaticMeshActor', 'Engine.InterpActor'):
                            self.issue(level + ':' + record['path'], 'Unsupported component owner')
                            continue
                        pose = {'actor': transform(props(owner)), 'component': transform(p, True)}
                    if matinee_first_key:
                        pose, applied = apply_matinee_first_key_pose(record['path'], pose)
                        matinee_first_key_applied += int(applied)
                    overrides = [self.material(self.resolve(level, ref)) for ref in p.get('Materials', [])]
                    mesh_identity = self.identity(key)
                    mesh_name = self.mesh(key)
                    effective_materials = [overrides[i] if i < len(overrides) and overrides[i]
                                           else section['material']
                                           for i, section in enumerate(self.meshes[mesh_name]['sections'])]
                    is_native_skybox = native_skybox_placement(
                        mesh_identity, effective_materials, self.materials)
                    if is_native_skybox:
                        # The observed dome's faces point outward while the
                        # player camera is inside it. Preserve the source mesh
                        # and use the narrow host-side two-sided policy needed
                        # for an interior visual shell.
                        for material_name in effective_materials:
                            if material_name in self.materials:
                                self.materials[material_name]['two_sided'] = True
                    actor = {'source': record['path'], 'level': level, 'mesh': mesh_name,
                             'transform': pose, 'materials': overrides, 'static': True,
                             'collision_enabled': p.get('BlockActors', True) and p.get('CollideActors', True),
                             'native_skybox': is_native_skybox,
                             'native_skybox_source': mesh_identity if is_native_skybox else None,
                             'hidden_visual': hidden_visual_mesh(
                                 mesh_identity, effective_materials, self.materials,
                                 record['path'])}
                    apply_outer_shell_policy(actor, mesh_identity, self.meshes[mesh_name]['sections'],
                                             self.materials, getattr(self, 'outer_shell', False))
                    actors.append(actor)
                except ValueError as error:
                    self.issue(level + ':' + record['path'], error)
        if not actors:
            raise ValueError('No static mesh placements loaded')
        if matinee_first_key and matinee_first_key_applied != 1:
            raise ValueError('Matinee first-key experiment expected exactly one '
                             f'{MATINEE_FIRST_KEY_COMPONENT}, found '
                             f'{matinee_first_key_applied}')
        result = {'schema': 1, 'map': persistent, 'levels': levels, 'actors': actors,
                  'package_scope': 'base_and_dlc' if getattr(self, 'include_dlc', False) else 'base',
                  'meshes': self.meshes, 'materials': self.materials, 'camera': camera,
                  'issues': self.issues, 'dynamic_policy': 'frozen', 'visual_validation': 'pending',
                  'collision_policy': 'observed_convex_and_box_v1',
                  'matinee_first_key_policy': (
                      'relative_to_initial_first_key_experiment_v1'
                      if matinee_first_key else 'serialized_placement'),
                  'matinee_first_key_applied': matinee_first_key_applied,
                  'outer_shell_policy': ('mesh_default_for_teleported_overrides_v1'
                                         if getattr(self, 'outer_shell', False) else 'placed_overrides')}
        temporary = self.output / 'scene.json.tmp'
        temporary.write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
        temporary.replace(self.output / 'scene.json')
        print(json.dumps({'levels': levels, 'placements': len(actors), 'meshes': len(self.meshes),
                          'materials': len(self.materials), 'issues': len(self.issues)}), flush=True)
        return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reader', type=Path, required=True)
    parser.add_argument('--game', type=Path, required=True)
    parser.add_argument('--map', default='Ash_P')
    parser.add_argument('--output', type=Path)
    parser.add_argument('--include-dlc', action='store_true', help='Index installed DLC packages and named texture caches')
    parser.add_argument('--outer-shell', action='store_true',
                        help='Render the observed outer hull meshes with their mesh-default materials '
                             'instead of the unrecoverable masked _Teleported overrides')
    parser.add_argument('--matinee-first-key', action='store_true',
                        help='Experimentally place Sanctuary InterpActor_29 at its observed '
                             'RelativeToInitial first move key')
    args = parser.parse_args()
    if not (args.game / 'Binaries/Win32/Borderlands2.exe').is_file():
        parser.error('An installed Borderlands 2 is required')
    if not args.map.endswith('_P'):
        parser.error('Start with a persistent _P package')
    if args.matinee_first_key and args.map.casefold() != 'sanctuary_p':
        parser.error('--matinee-first-key is only supported for Sanctuary_P')
    if not args.map.replace('_', '').isascii() or not args.map.replace('_', '').isalnum():
        parser.error('Map names must contain only ASCII letters, digits and underscores')
    if args.output is None:
        args.output = Path('local') / args.map[:-2].lower()
    Scene(args.reader, args.game, args.output, include_dlc=args.include_dlc,
          outer_shell=args.outer_shell,
          matinee_first_key=args.matinee_first_key).build(args.map)


if __name__ == '__main__':
    main()
