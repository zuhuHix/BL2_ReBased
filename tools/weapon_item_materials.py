"""Material selection for imported weapon meshes.

This pure policy is shared by the UE importer and synthetic tests. A prepared
per-recipe paint material takes precedence; the Infinity approximation is the
only shared fallback, and all other missing paint stays an explicit neutral
stand-in.
"""
import re


INFINITY_MIC = 'Common_GunMaterials.Materials.Pistol.Mati_VladofLegendaryPistol_Infinity'


def paint_material_path(recipe_id):
    if not re.fullmatch(r'[A-Za-z0-9_]+', recipe_id):
        raise ValueError(f'Invalid recipe ID: {recipe_id!r}')
    return f'/Game/OpenWillow/Weapons/Paint/{recipe_id}/M_OW_PaintApprox'


def choose_item_material(recipe_id, material_identity, *, paint_available, infinity_available):
    """Return the material source and an actionable warning for neutral fallback."""
    if paint_available:
        return 'paint', None
    if material_identity == INFINITY_MIC and infinity_available:
        return 'infinity', None
    return 'neutral', f'no prepared paint material at {paint_material_path(recipe_id)} for {material_identity}'
