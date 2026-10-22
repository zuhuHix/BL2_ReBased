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


def temporary_sky_policy(scene):
    """Use UE atmosphere/shell only when no accepted dome approximation exists."""
    native = native_sky_approximated(scene)
    return {'native_sky_approximated': native,
            'spawn_sky_fallback': not native,
            'spawn_sky_atmosphere': not native}


def ue3_color_linear(rgb):
    """Linear RGB for a UE3 FColor given as byte/255 values.

    UE3 converts an FColor to FLinearColor with a 2.2 power curve, so a light
    or fog color must be decoded the same way before it is handed to UE5 as a
    linear color. Brightness scales are applied by the caller.
    """
    return [float(c) ** 2.2 for c in rgb]
