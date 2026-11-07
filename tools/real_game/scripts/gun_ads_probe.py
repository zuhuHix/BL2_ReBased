"""Named guns for the ADS frames (gun_ads_capture.ps1). Load after weapon_parity_probe.py, whose commands and state it uses.

Loading defines commands; nothing runs until one is called. Output and reads stay under ignored local/. Objects are looked up again in
every command, since a reference kept from an earlier command can crash the game.

  ads_seed(id, path, level)  make a recipe's exact parts at Maya's level, unless a match exists, as wp_seed does for the slice guns
  ads_held(id)               'exact', 'parts' (the game changed its stage or grade, as seen on some guns) or 'no'
  ads_read()                 the live zoom state, the field that told lane E's frames apart
  ads_character()            the level, and whether Phaselock's skill is loaded (only Maya has it)
  ads_fov_arm() / ads_fov_read()  hook the HUD's PostRender once, project known view directions through its canvas, and fit the view's FOV
  ads_turn(degrees)          the view that many degrees from the one chosen by wp_face_wall, at level pitch, for the turn test
  ads_foreground()           the controller's foreground FOV getter, if script can call it (None otherwise; a getter never tried before)
"""
import json
import math


def ads_character():
    skills = [s for s in unrealsdk.find_all("LiftActionSkill") if "Default__" not in s._path_name()]
    return {"is_maya": bool(skills), "level": get_pc().Pawn.GetExpLevel()}


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


ADS_FOV_DIRECTIONS = [(a, 0.0) for a in (-30, -20, -10, 0, 10, 20, 30)] + [(0.0, b) for b in (-15, -5, 5, 15)]
ads_fov_state = globals().setdefault("ads_fov_state", {"result": None, "hooks": []})


def ads_fit_fov(rows, clip_x):
    """The view's horizontal FOV in degrees from projected points, from the level row, or None. A straight line: screen x against tan(yaw) has slope f in pixels."""
    points = [(math.tan(math.radians(row["yaw"])), row["x"]) for row in rows if row.get("pitch") == 0.0 and "x" in row]
    if len(points) < 3 or not clip_x:
        return None
    mean_t = sum(t for t, _x in points) / len(points)
    mean_x = sum(x for _t, x in points) / len(points)
    spread = sum((t - mean_t) ** 2 for t, _x in points)
    focal = sum((t - mean_t) * (x - mean_x) for t, x in points) / spread if spread else 0.0
    return math.degrees(2.0 * math.atan(clip_x / 2.0 / focal)) if focal > 0.0 else None


def _ads_fov_post_render(obj, _args, _ret, _func):
    """Runs inside rendering: never lets an error out, and measures one frame only."""
    if ads_fov_state["result"] is not None:
        return
    try:
        canvas = getattr(obj, "Canvas", None)
        if canvas is None:
            return
        pov = get_pc().PlayerCamera.CameraCache.POV
        fwd, right, up = _ads_axes(pov.Rotation.Pitch, pov.Rotation.Yaw)
        rows = []
        for yaw, pitch in ADS_FOV_DIRECTIONS:
            ty, tp = math.tan(math.radians(yaw)), math.tan(math.radians(pitch))
            aim = [1000.0 * (fwd[i] + right[i] * ty + up[i] * tp) for i in range(3)]
            screen = canvas.Project(unrealsdk.make_struct("Vector", X=pov.Location.X + aim[0], Y=pov.Location.Y + aim[1], Z=pov.Location.Z + aim[2]))
            rows.append({"yaw": yaw, "pitch": pitch, "x": screen.X, "y": screen.Y})
        ads_fov_state["result"] = {"clip": [canvas.ClipX, canvas.ClipY], "pov_fov": pov.FOV, "rows": rows, "hfov": ads_fit_fov(rows, canvas.ClipX)}
    except Exception as error:
        ads_fov_state["result"] = {"error": repr(error)[:200]}


def _ads_axes(pitch_units, yaw_units):
    pitch, yaw = pitch_units * math.pi / 32768.0, yaw_units * math.pi / 32768.0
    fwd = (math.cos(pitch) * math.cos(yaw), math.cos(pitch) * math.sin(yaw), math.sin(pitch))
    right = (-math.sin(yaw), math.cos(yaw), 0.0)
    return fwd, right, (-math.sin(pitch) * math.cos(yaw), -math.sin(pitch) * math.sin(yaw), math.cos(pitch))


def ads_fov_arm():
    from unrealsdk.hooks import Type, add_hook
    ads_fov_state["result"] = None
    for path in ("WillowGame.WillowHUD:PostRender", "Engine.HUD:PostRender"):
        try:
            add_hook(path, Type.POST, "ow_gun_ads_fov", _ads_fov_post_render)
            ads_fov_state["hooks"].append(path)
        except Exception:
            pass
    return {"hooked": list(ads_fov_state["hooks"])}


def ads_fov_read():
    from unrealsdk.hooks import Type, remove_hook
    for path in ads_fov_state["hooks"]:
        try:
            remove_hook(path, Type.POST, "ow_gun_ads_fov")
        except Exception:
            pass
    ads_fov_state["hooks"] = []
    return ads_fov_state["result"]


def ads_turn(degrees):
    _set_view(wp_state["view"]["yaw"] + float(degrees), 0.0)
    return {"yaw": wp_state["view"]["yaw"] + float(degrees)}


def ads_foreground():
    getter = getattr(get_pc(), "GetFOVAngleForeground", None)
    return {"foreground_fov": getter() if callable(getter) else None, "callable": callable(getter)}
