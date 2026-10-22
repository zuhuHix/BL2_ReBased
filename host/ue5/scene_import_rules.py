"""Pure scene-import policies shared by the UE importer and verifier."""
import math


# UNVERIFIED host calibration against lane A's matched Sanctuary captures.
# Round-2 camera 02/04/05/06/07 showed clipped exteriors while 01/03/08 were
# too dark and blue. Lower the direct light and use a brighter, less-blue fill;
# these values tune the screenshots, not UE3-to-UE5 units.
SANCTUARY_DIRECTIONAL_LUX_PER_SOURCE_BRIGHTNESS = 5.0
SANCTUARY_SKY_INTENSITY_CALIBRATION = 1.5
SANCTUARY_SKY_LIGHT_COLOR_CALIBRATION = (3.0, 2.1, 1.0)
SANCTUARY_AUTO_EXPOSURE_CALIBRATION_BIAS = -1.5
# Round-4 host grade, UNVERIFIED and tuned only against the matched Sanctuary
# captures (outdoor 02/05/06 washed and neutral, interior 01/03/08 too dark).
# The game is warm (sand, rust facades, warm highlights) with firmer contrast.
# UE white temp above 6500 warms the image; these are not UE3 units.
SANCTUARY_HOST_WHITE_TEMP = 8200.0
SANCTUARY_HOST_WHITE_TINT = 0.0
SANCTUARY_HOST_COLOR_SATURATION = (1.12, 1.02, 0.92, 1.0)
SANCTUARY_HOST_COLOR_CONTRAST = (1.10, 1.10, 1.10, 1.0)
# Added to the source color offset (or zero): lifts interiors that were too dark.
SANCTUARY_HOST_SHADOW_LIFT = (0.03, 0.03, 0.035)
# Round-4 sky calibration, UNVERIFIED: the recorded cloud_cap_opacity drives
# broad bright streaks that the game sky does not show at this strength. The
# cloud band is scaled down; the recorded source value stays in scene.json.
SANCTUARY_SKY_CLOUD_OPACITY_CALIBRATION = 0.14

# Sanctuary-only host choices. Light units and these thresholds are screenshot
# calibration, not a UE3-to-UE5 unit conversion.
# Round-3 HQ captures showed clipped source-light pools at 300 cd per source
# brightness unit. This lower value is a named, UNVERIFIED visual calibration.
SANCTUARY_POINT_LIGHT_CANDELA_PER_SOURCE_BRIGHTNESS = 25.0
SANCTUARY_SPOT_LIGHT_CANDELA_PER_SOURCE_BRIGHTNESS = 25.0
# High source values still produced an orange pool in the matched HQ capture
# after the first brightness clamp. Clamp source brightness before applying
# the named scale; smaller lights retain their source value. This remains an
# UNVERIFIED host calibration, with radius and source color still preserved.
SANCTUARY_LIGHT_BRIGHTNESS_CEILING = 3.0
# A saturated orange static point light still created an artificial HQ ring
# after the general ceiling. This narrowly scoped, screenshot-derived scale is
# an UNVERIFIED intensity calibration; it does not change source color/radius.
SANCTUARY_ORANGE_POINT_LIGHT_INTENSITY_SCALE = 0.01
SANCTUARY_LIGHT_MIN_RADIUS = 128.0
SANCTUARY_LIGHT_MIN_BRIGHTNESS = 0.1
SANCTUARY_LIGHT_CELL_SIZE = 1500.0
SANCTUARY_LIGHT_MAX_PER_CELL = 2
SANCTUARY_LIGHT_MAX_IMPORTED = 128

SANCTUARY_GRAFITTO_INK_LUMINANCE_CEILING = 0.55
SANCTUARY_GRAFITTO_INK_OPACITY_GAIN = 6.0
SANCTUARY_GRAFITTO_INK_COLOR = (0.001, 0.003, 0.008)
# The kept graffiti actors sit outside their receiver surface. Their local
# volume reaches inward by a bounded amount; the cap avoids long UE3 far planes
# reaching unrelated receivers. This is an UNVERIFIED host calibration.
SANCTUARY_GRAFITTO_MAX_DEPTH = 128.0

SANCTUARY_WORLD_MAP_PALETTE = {
    'background': (0.11, 0.38, 0.50),
    'land': (0.30, 0.72, 0.82),
    'grid': (0.004, 0.020, 0.028),
    'markers': (0.006, 0.030, 0.045),
}


def ue3_directional_brightness_to_ue5(brightness):
    """Apply the named, screenshot-calibrated UE3 brightness-to-lux scale."""
    if (not isinstance(brightness, (int, float)) or isinstance(brightness, bool)
            or not math.isfinite(brightness) or brightness < 0):
        raise ValueError('Directional brightness must be finite and non-negative')
    return float(brightness) * SANCTUARY_DIRECTIONAL_LUX_PER_SOURCE_BRIGHTNESS


def ue3_sky_light_color_to_ue5(rgb):
    """Apply the screenshot-calibrated green-cyan fill tint to the source FColor."""
    if (not isinstance(rgb, (list, tuple)) or len(rgb) != 3
            or any(not isinstance(value, (int, float)) or isinstance(value, bool)
                   or not math.isfinite(value) or value < 0 or value > 1 for value in rgb)):
        raise ValueError('Sky light color must contain three normalized finite channels')
    linear = ue3_color_linear(rgb)
    calibrated = [value * tint for value, tint in zip(linear, SANCTUARY_SKY_LIGHT_COLOR_CALIBRATION)]
    if any(not math.isfinite(value) or value < 0 or value > 1 for value in calibrated):
        raise ValueError('Calibrated sky light color must stay within linear RGB range')
    return calibrated


def ue3_world_post_process_to_ue5(settings):
    """Map the decoded WorldInfo defaults to UE5 exposure and color grading.

    Tone scale is mapped to stops with log2, Scene_MidTones to Color Gain,
    Scene_Shadows to Color Offset, and Bloom_Scale to Bloom Intensity. These
    relationships are an UNVERIFIED host approximation of the UE3 tone curve.
    """
    settings = settings if isinstance(settings, dict) else {}
    scale = settings.get('Scene_TonemapperScale', 1.0)
    if (not isinstance(scale, (int, float)) or isinstance(scale, bool)
            or not math.isfinite(scale) or scale <= 0):
        raise ValueError('WorldInfo tonemapper scale must be finite and positive')
    midtones = settings.get('Scene_MidTones', {'X': 1.0, 'Y': 1.0, 'Z': 1.0})
    shadows = settings.get('Scene_Shadows', {'X': 0.0, 'Y': 0.0, 'Z': 0.0})
    axes = ('X', 'Y', 'Z')
    if not isinstance(midtones, dict) or not isinstance(shadows, dict):
        raise ValueError('WorldInfo color grading vectors must be decoded structs')
    gain = [midtones.get(axis, 1.0) for axis in axes]
    offset = [shadows.get(axis, 0.0) for axis in axes]
    if any(not isinstance(value, (int, float)) or isinstance(value, bool)
           or not math.isfinite(value) for value in gain + offset):
        raise ValueError('WorldInfo color grading vectors must be finite')
    bloom = settings.get('Bloom_Scale', 0.5)
    if (not isinstance(bloom, (int, float)) or isinstance(bloom, bool)
            or not math.isfinite(bloom) or bloom < 0):
        raise ValueError('WorldInfo bloom scale must be finite and non-negative')
    return {'auto_exposure_bias': math.log2(float(scale)) + SANCTUARY_AUTO_EXPOSURE_CALIBRATION_BIAS,
            'color_gain': [float(value) for value in gain],
            'color_offset': [float(value) for value in offset],
            'bloom_intensity': float(bloom)}


def ue3_decal_dimensions_to_ue5(dimensions):
    """Map inspected UE3 decal extents to UE5 local projection half-extents.

    UE3 near/far clipping is reduced to a single UE5 local X depth. The axis
    and clipping conversion is an UNVERIFIED host approximation.
    """
    names = ('width', 'height', 'thickness', 'near_plane', 'far_plane')
    values = [dimensions.get(name) for name in names] if isinstance(dimensions, dict) else []
    if (len(values) != len(names)
            or any(not isinstance(value, (int, float)) or isinstance(value, bool)
                   or not math.isfinite(value) or value < 0 for value in values)):
        raise ValueError('Decal dimensions must be finite, non-negative numbers')
    width, height, thickness, near_plane, far_plane = map(float, values)
    if far_plane < near_plane:
        raise ValueError('Decal far plane must not precede near plane')
    depth = max(thickness, far_plane - near_plane)
    return [depth * 0.5, width * 0.5, height * 0.5]


def ue3_grafitto_dimensions_to_ue5(dimensions):
    """Bound the kept graffiti's UE5 projector using UE3 near/far depth.

    UE5 DecalSize.X is a centered half-depth along local X. The actor origin
    is moved inward separately by ue3_grafitto_projection_to_ue5 so an
    off-surface UE3 hit point still intersects its receiver. The depth cap is
    an UNVERIFIED visual safety calibration; width and height retain their
    UE3 world-space half-extents.
    """
    full = ue3_decal_dimensions_to_ue5(dimensions)
    depth = min(full[0] * 2.0, SANCTUARY_GRAFITTO_MAX_DEPTH)
    return [depth * 0.5, full[1], full[2]]


def ue3_grafitto_projection_to_ue5(location, hit_normal, dimensions):
    """Return the receiver-overlapping UE5 center and local-X direction.

    UE3 HitNormal points out of the receiver. Moving the centered UE5 volume
    opposite that normal keeps the source hit point at its outer boundary;
    its half-depth then reaches the receiver without projecting the full
    native far plane through nearby geometry. The mapping is UNVERIFIED.
    """
    if (not isinstance(location, (list, tuple)) or len(location) != 3
            or any(not isinstance(value, (int, float)) or isinstance(value, bool)
                   or not math.isfinite(value) for value in location)):
        raise ValueError('Graffito location must contain three finite coordinates')
    if (not isinstance(hit_normal, (list, tuple)) or len(hit_normal) != 3
            or any(not isinstance(value, (int, float)) or isinstance(value, bool)
                   or not math.isfinite(value) for value in hit_normal)):
        raise ValueError('Graffito HitNormal must contain three finite coordinates')
    magnitude = math.sqrt(sum(float(value) ** 2 for value in hit_normal))
    if magnitude <= 1e-5:
        raise ValueError('Graffito HitNormal must be non-zero')
    size = ue3_grafitto_dimensions_to_ue5(dimensions)
    normal = [float(value) / magnitude for value in hit_normal]
    near_plane = float(dimensions['near_plane'])
    offset = near_plane + size[0]
    center = [float(value) - axis * offset for value, axis in zip(location, normal)]
    return center, normal


def sanctuary_grafitto_opacity(rgb):
    """Return the calibrated dark-ink mask for the opaque white-backed atlas."""
    if (not isinstance(rgb, (list, tuple)) or len(rgb) != 3
            or any(not isinstance(value, (int, float)) or isinstance(value, bool)
                   or not math.isfinite(value) or value < 0 or value > 1 for value in rgb)):
        raise ValueError('Graffitto RGB must contain three normalized finite channels')
    luminance = sum(float(value) * weight for value, weight in zip(rgb, (0.2126, 0.7152, 0.0722)))
    return min(1.0, max(0.0,
                        (SANCTUARY_GRAFITTO_INK_LUMINANCE_CEILING - luminance)
                        * SANCTUARY_GRAFITTO_INK_OPACITY_GAIN))


def sanctuary_world_map_color(rgb_masks):
    """Calibrated R=land, G=grid, B=markers palette for WorldMap_Comp."""
    if (not isinstance(rgb_masks, (list, tuple)) or len(rgb_masks) != 3
            or any(not isinstance(value, (int, float)) or isinstance(value, bool)
                   or not math.isfinite(value) or value < 0 or value > 1 for value in rgb_masks)):
        raise ValueError('World map masks must contain three normalized finite channels')
    r, g, b = (float(value) for value in rgb_masks)
    palette = SANCTUARY_WORLD_MAP_PALETTE
    land = [background + (value - background) * r
            for background, value in zip(palette['background'], palette['land'])]
    grid = [value + (dark - value) * g
            for value, dark in zip(land, palette['grid'])]
    return [value + (dark - value) * b
            for value, dark in zip(grid, palette['markers'])]


def sanctuary_light_calibration(kind, brightness, color_rgb=None):
    """Map an installed point/spot light's source brightness to UE5 candelas.

    The multiplier, source-brightness ceiling and saturated-orange point-light
    scale are UNVERIFIED Sanctuary capture calibrations. Radius stays in world
    units; source FColor channels are separately converted to linear.
    """
    if (kind not in ('Point', 'Spot') or not isinstance(brightness, (int, float))
            or isinstance(brightness, bool) or not math.isfinite(brightness) or brightness < 0):
        raise ValueError('Light kind or brightness is invalid')
    scale = (SANCTUARY_POINT_LIGHT_CANDELA_PER_SOURCE_BRIGHTNESS if kind == 'Point'
             else SANCTUARY_SPOT_LIGHT_CANDELA_PER_SOURCE_BRIGHTNESS)
    intensity = min(float(brightness), SANCTUARY_LIGHT_BRIGHTNESS_CEILING) * scale
    if color_rgb is not None:
        if (not isinstance(color_rgb, (list, tuple)) or len(color_rgb) != 3
                or any(not isinstance(value, (int, float)) or isinstance(value, bool)
                       or not math.isfinite(value) or value < 0 or value > 1 for value in color_rgb)):
            raise ValueError('Light color must contain three normalized finite channels')
        red, green, blue = (float(value) for value in color_rgb)
        if kind == 'Point' and red >= 0.8 and 0.25 <= green <= 0.6 and blue <= 0.1:
            intensity *= SANCTUARY_ORANGE_POINT_LIGHT_INTENSITY_SCALE
    return intensity


def native_sky_approximated(scene):
    """Whether an accepted native dome already supplies the visible sky."""
    for instance in scene['actors']:
        if not instance.get('native_skybox'):
            continue
        materials = instance['materials'] or [
            section['material'] for section in scene['meshes'][instance['mesh']]['sections']]
        if any(material and scene['materials'].get(material, {}).get('sky_approximation')
               for material in materials):
            return True
    return False


def sanctuary_sky_cloud_opacity(source_opacity):
    """Host cloud-band opacity: recorded source value times the round-4 calibration."""
    if (not isinstance(source_opacity, (int, float)) or isinstance(source_opacity, bool)
            or not math.isfinite(source_opacity) or source_opacity < 0):
        raise ValueError('Sky cloud opacity must be a finite non-negative number')
    return min(float(source_opacity) * SANCTUARY_SKY_CLOUD_OPACITY_CALIBRATION, 1.0)


def temporary_sky_policy(scene):
    """Use UE atmosphere/shell only when no accepted dome approximation exists."""
    native = native_sky_approximated(scene)
    return {'native_sky_approximated': native,
            'spawn_sky_fallback': not native,
            'spawn_sky_atmosphere': not native}


# Sanctuary host profile. Only the Sanctuary_P map gets the Sanctuary
# calibrations, source-derived lighting, decals and static lights. Every other
# map keeps the pre-round-1 inspection rig. Round-4 host pieces are additionally
# opt-in through this environment variable (comma-separated, case-insensitive).
SANCTUARY_MAP = 'Sanctuary_P'
SANCTUARY_OPT_IN_ENV = 'OPENWILLOW_SANCTUARY_OPT_IN'
SANCTUARY_OPT_IN_NAMES = frozenset(('grade', 'diffuse_tint', 'xmas_bulbs', 'static_lights',
                                    'graffiti_decals'))
SANCTUARY_XMAS_BULB_METHOD = 'xmas_bulb_emissive_v1'


def sanctuary_profile(scene):
    """True only for Sanctuary_P; other maps never receive the Sanctuary profile."""
    return scene.get('map') == SANCTUARY_MAP


def sanctuary_opt_ins(environ_value, scene):
    """Opt-in names that take effect for this scene.

    Unknown names raise ValueError even on other maps, so a typo is never silent.
    Valid names are ignored (empty set) unless the scene is Sanctuary_P.
    """
    requested = set()
    for token in (environ_value or '').split(','):
        name = token.strip().lower()
        if not name:
            continue
        if name not in SANCTUARY_OPT_IN_NAMES:
            raise ValueError('Unknown %s name %r; expected one of: %s' % (
                SANCTUARY_OPT_IN_ENV, token.strip(), ', '.join(sorted(SANCTUARY_OPT_IN_NAMES))))
        requested.add(name)
    if not sanctuary_profile(scene):
        return frozenset()
    return frozenset(requested)


def sanctuary_source_lighting(scene):
    """Decoded source lighting records; empty for non-Sanctuary maps (pre-round-1 rig)."""
    return scene.get('lighting', {}) if sanctuary_profile(scene) else {}


def sanctuary_verifier_checks(scene):
    """True when verify_level.py runs the Sanctuary-only checks (same gate as the importer).

    Other maps get the generic inspection-rig checks instead. A Sanctuary_P scene
    without a 'lighting' record still runs these checks, but its source-lighting
    records are empty, so the verifier skips the selection-policy asserts.
    """
    return sanctuary_profile(scene)


def host_sky_cloud_opacity(scene, source_opacity):
    """Sanctuary cloud-band calibration only on Sanctuary_P; the source cap otherwise."""
    if sanctuary_profile(scene):
        return sanctuary_sky_cloud_opacity(source_opacity)
    return float(source_opacity)


def effective_scene_recipe(definition, opt_ins):
    """The scene_recipe a material gets this run; the bulb recipe needs its opt-in."""
    recipe = definition.get('scene_recipe', {})
    if recipe.get('method') == SANCTUARY_XMAS_BULB_METHOD and 'xmas_bulbs' not in opt_ins:
        return {}
    return recipe


def ue3_color_linear(rgb):
    """Linear RGB for a UE3 FColor given as byte/255 values.

    UE3 converts an FColor to FLinearColor with a 2.2 power curve, so a light
    or fog color must be decoded the same way before it is handed to UE5 as a
    linear color. Brightness scales are applied by the caller.
    """
    return [float(c) ** 2.2 for c in rgb]
