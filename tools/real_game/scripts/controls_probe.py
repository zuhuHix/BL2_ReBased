"""Controls lane probe for the openwillow_realgame command channel (2026-11-06): stock use prompts and the melee press.

Run through Invoke-GamePyFile (tools/real_game/controls_capture.ps1) with the Maya save loaded and saves blocked. It runs in the
driver's namespace (get_pc, plain, RG_OUT). Our own code, no game data; everything it writes is game data and goes under RG_OUT
(ignored local/realgame/...). Memory-only: reads, hooks and moving the player. The struct and argument field names of the prompt
were not known when this was written, so each read is guarded and whole records are dumped with plain().

- cp_bindings(): the keyboard bindings the running game holds for the keys this lane cares about.
- cp_targets(): usable things near the plaza, as candidates (vending machines, interactive objects, Marcus).
- cp_prompt(obj, distance): stand in front of obj looking at it, and after cp_prompt_read() report what the controller holds
  (usable object, icons) and what the HUD movie's two prompt clips show.
- cp_prompt_read(): the same read at the current spot.
- cp_melee_hooks() / cp_rows() / cp_unhook(): rows for the melee chain with the game clock.
- cp_melee_facts(): what the pawn's definitions say for the melee: the special moves, the first-person animation and its
  length, the camera anim, the live value of the player melee damage definition.
"""
import json
import math
import os
import time

import unrealsdk
from unrealsdk.hooks import Type, add_hook, remove_hook

CP = {"rows": [], "hooks": [], "t0": None}
KEYS = ("V", "M", "L", "E", "Q", "G", "I", "K", "Tab", "J")
CLIPS = ("p1.tools", "p1.toolsSecondary")
FIELDS = ("desc", "text", "key", "cost", "amount", "button", "icon")
CHAIN = [
    "WillowGame.WillowPlayerController:Behavior_Melee",
    "WillowGame.WillowPlayerController:MeleeAttack",
    "WillowGame.WillowPlayerController:MeleeAttackComplete",
    "WillowGame.WillowPlayerController:PerformSharedWeaponActions",
    "WillowGame.Behavior_MeleeAttack:ApplyBehaviorToContext",
    "WillowGame.MeleeDefinition:MeleeAttack",
    "WillowGame.WillowPlayerController:UpdateInteractionIcon",
    "WillowGame.WillowHUDGFxMovie:UpdateInteractionIcon",
    "WillowGame.WillowHUD:UpdateInteractionIcon",
    "Engine.Actor:TakeDamage",
    "WillowGame.WillowPawn:TakeDamage",
    "WillowGame.WillowAIPawn:TakeDamage",
]


def _try(fn):
    try:
        return fn()
    except Exception as error:
        return f"ERR {repr(error)[:100]}"


def _name(obj):
    return None if obj is None else _try(lambda: f"{obj.Class.Name}'{obj._path_name()}'")


def _pc():
    return get_pc()


def _out(name, data):
    os.makedirs(RG_OUT, exist_ok=True)
    path = os.path.join(RG_OUT, f"controls_{name}.json")
    with open(path, "w", encoding="utf-8") as handle:
        json.dump(data, handle, indent=1, default=str)
    return path


def cp_bindings():
    rows = []
    for binding in list(_pc().PlayerInput.Bindings):
        if str(binding.Name) in KEYS:
            rows.append({"key": str(binding.Name), "command": str(binding.Command), "control": bool(binding.Control),
                         "shift": bool(binding.Shift), "alt": bool(binding.Alt)})
    _out("bindings", rows)
    return rows


def cp_targets():
    pawn = _pc().Pawn
    here = pawn.Location
    rows = []
    for cls in ("WillowVendingMachine", "WillowInteractiveObject", "WillowAIPawn"):
        for obj in unrealsdk.find_all(cls):
            if "Default__" in obj._path_name() or obj.Location is None:
                continue
            distance = math.dist((here.X, here.Y, here.Z), (obj.Location.X, obj.Location.Y, obj.Location.Z))
            rows.append({"class": obj.Class.Name, "path": obj._path_name(), "distance": round(distance),
                         "definition": _name(_try(lambda: obj.InteractiveObjectDefinition))})
    rows.sort(key=lambda row: row["distance"])
    _out("targets", rows)
    return rows


def _icon(record):
    return _try(lambda: plain(record))


def cp_prompt_read(label=""):
    pc = _pc()
    movie = pc.myHUD.HUDMovie
    out = {"label": label, "usable": _name(_try(lambda: pc.CurrentUsableObject)),
           "icons": [_icon(record) for record in list(pc.CurrentInteractionIcon)]}
    for clip in CLIPS:
        row = {}
        for prop in ("_visible", "_alpha", "_x", "_y", "_xscale", "_yscale", "_currentframe"):
            row[prop] = _try(lambda: movie.GetVariableNumber(f"{clip}.{prop}"))
        for field in FIELDS:
            row[field] = _try(lambda: movie.GetVariableString(f"{clip}.{field}.text"))
            row[field + "_html"] = _try(lambda: movie.GetVariableString(f"{clip}.{field}.htmlText"))
        out[clip] = row
    return out


def cp_prompt(path, distance=120.0, up=0.0):
    """Stand `distance` uu from the object on the player's side of it, level with the view, and look at it."""
    pc = _pc()
    pawn = pc.Pawn
    obj = unrealsdk.find_object("Object", path)
    target = obj.Location
    dx, dy = pawn.Location.X - target.X, pawn.Location.Y - target.Y
    length = math.hypot(dx, dy) or 1.0
    pawn.Location.X = target.X + dx / length * distance
    pawn.Location.Y = target.Y + dy / length * distance
    pawn.Location.Z = target.Z + up + 60.0
    _try(lambda: pawn.ForceUpdateComponents(False, False))
    to_units = 65536.0 / 360.0
    pc.Rotation.Yaw = int(math.degrees(math.atan2(-dy, -dx)) * to_units)
    pc.Rotation.Pitch = 0
    return {"placed": True, "at": [pawn.Location.X, pawn.Location.Y, pawn.Location.Z]}


def _row(path, tag):
    def hook(obj, args, ret, _func):
        clock = _try(lambda: obj.WorldInfo.TimeSeconds)
        row = {"t": time.perf_counter() - (CP["t0"] or 0), "game": clock, "fn": path, "tag": tag, "self": _name(obj)}
        if tag == "pre":
            row["args"] = _try(lambda: plain(args))
        CP["rows"].append(row)
        return None
    return hook


def cp_melee_hooks():
    CP["rows"] = []
    CP["t0"] = time.perf_counter()
    done, failed = [], []
    for path in CHAIN:
        for typ, tag in ((Type.PRE, "pre"), (Type.POST, "post")):
            ident = f"ow_cp_{tag}_{path}"
            try:
                add_hook(path, typ, ident, _row(path, tag))
                CP["hooks"].append((path, typ, ident))
                done.append(path)
            except Exception as error:
                failed.append(f"{path}: {repr(error)[:70]}")
    return {"hooked": sorted(set(done)), "failed": failed}


def cp_rows(name=None):
    if name:
        return _out(name, CP["rows"])
    return CP["rows"]


def cp_unhook():
    for path, typ, ident in CP["hooks"]:
        _try(lambda: remove_hook(path, typ, ident))
    CP["hooks"] = []


def cp_melee_facts():
    pc = _pc()
    pawn = pc.Pawn
    body = pawn.BodyClass
    out = {"body": _name(body), "camera_anim": _name(_try(lambda: body.MeleeCameraAnim))}
    third = _try(lambda: pawn.GetBodyWeaponSMD(9))
    first = _try(lambda: pawn.GetFirstPersonBodyWeaponSMD(9))
    for key, move in (("third", third), ("first", first)):
        if isinstance(move, str) or move is None:
            out[key] = move
            continue
        out[key] = {"object": _name(move), "anim": str(move.AnimName), "node": str(move.AnimNodeName), "rate": float(move.PlayRate),
                    "blend_in": float(move.BlendInTime), "blend_out": float(move.BlendOutTime),
                    "length": _try(lambda: float(move.GetAnimLength(pawn.Arms if key == "first" else pawn.Mesh))),
                    "sets": _try(lambda: [_name(s) for s in (pawn.Arms if key == "first" else pawn.Mesh).AnimSets])}
    melee = unrealsdk.find_object("MeleeDefinition", "GD_Siren_Streaming.Melee.BehaviorData_PlayerMeleeAttack_Siren")
    out["definition"] = {"trace": float(melee.TraceDistance), "cone": float(melee.ConeAngle), "use_cone": bool(melee.bUseTargetCone),
                         "use_radius": bool(melee.bUseDamageRadius)}
    damage = unrealsdk.find_object("AttributeInitializationDefinition", "GD_Balance_HealthAndDamage.HealthAndDamage.Init_PlayerMeleeDamage")
    out["damage_value"] = _try(lambda: float(damage.EvaluateInitializationDefinition(pc, 1.0)))
    out["level"] = _try(lambda: int(pc.PlayerReplicationInfo.ExpLevel))
    out["weapon_action_block"] = _try(lambda: bool(pc.CanPerformWeaponAction()))
    _out("melee_facts", out)
    return out
