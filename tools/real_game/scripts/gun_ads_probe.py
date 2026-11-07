"""Named guns for the ADS frames (gun_ads_capture.ps1). Load after weapon_parity_probe.py, whose commands and state it uses.

Loading defines commands; nothing runs until one is called. Output and reads stay under ignored local/. Objects are looked up again in
every command, since a reference kept from an earlier command can crash the game.

  ads_seed(id, path, level)  make a recipe's exact parts at Maya's level, unless a match exists, as wp_seed does for the slice guns
  ads_held(id)               'exact', 'parts' (the game changed its stage or grade, as seen on some guns) or 'no'
  ads_read()                 the live zoom state, the field that told lane E's frames apart
"""
import json


def ads_recipe(recipe, level):
    """The recipe at this level. A gun above Maya's level may not be held; the stage and grade do not choose its parts."""
    return dict(recipe, game_stage=int(level), grade=int(level), manufacturer_grade_index=int(level))


def ads_seed(recipe_id, relative_path, level=8):
    recipe = ads_recipe(json.loads((_repo_root() / relative_path).read_text(encoding="utf-8-sig")), level)
    wp_state["recipes"][recipe_id] = recipe
    if _find_recipe(recipe_id) is not None:
        return {"recipe_id": recipe_id, "created": False}
    values = {}
    for key, value in definition_fields(recipe).items():
        if isinstance(value, str) and key.endswith("Definition"):
            found = unrealsdk.find_object("Object", value)
            if found is None:
                raise LookupError("could not resolve " + key + ": " + value)
            values[key] = found
        else:
            values[key] = value
    get_pc().GetPawnInventoryManager().AddBackpackWeaponFromDefinitionData(unrealsdk.make_struct("WeaponDefinitionData", **values))
    wp_state["spawned"].add(recipe_id)  # owned by this probe from here on, even if the match below fails
    if _find_recipe(recipe_id) is None:
        raise RuntimeError("exact recipe did not appear in inventory: " + recipe_id)
    return {"recipe_id": recipe_id, "created": True}


def ads_held(recipe_id):
    pawn = get_pc().Pawn
    weapon = pawn.Weapon if pawn else None
    if weapon is None:
        return "no"
    recipe = wp_state["recipes"][recipe_id]
    if _matches_recipe(weapon, recipe):
        return "exact"
    return "parts" if _loose_match(weapon, recipe) else "no"


def ads_read():
    pc = get_pc()
    weapon = pc.Pawn.Weapon if pc.Pawn else None
    if weapon is None:
        return {"zoomed": False, "zoom_state": None, "zoomed_rate": None}
    state = str(_deep(weapon.ZoomState)).rsplit(".", 1)[-1]
    return {"zoomed": state == "ZST_Zoomed", "zoom_state": state, "zoomed_rate": _deep(weapon.ZoomedRate),
            "zoomed_fov": _deep(weapon.ZoomedFOV), "camera_fov": _deep(pc.CachedFOVAngle)}
