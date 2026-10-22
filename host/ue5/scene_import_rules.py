"""Pure scene-import policies shared by the UE importer and verifier."""


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
