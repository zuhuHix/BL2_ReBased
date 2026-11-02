"""HUD integrity live reads: which HUD movie clips the running game drives, frame by frame, for the slice loop.

Our own code for the openwillow_realgame command channel. tools/real_game/hud_integrity_capture.ps1 runs it through
Invoke-GamePyFile; everything it writes (rows.json, the screenshots) is game data under ignored local/realgame/.

The HUD's drivers (WillowHUDGFxMovie.UpdateInjured, DisplaySecondWind, UpdateInteractionIcon, ...) are native, so a
hook cannot see what they set. Instead a per-frame watcher reads a fixed list of the movie's clip properties with
GetVariableNumber / GetVariableBool / GetVariableString and records each value when it changes, with the QPC clock and
the game's own time. The clip paths come from the HUD movie's own structure (the down-state clip `p1.dying1` with its
labels off / fight / revive / duel, `p1.secondwind`, the interaction icon `p1.tools`, the mission modules, ...).

- hi_start(): install the watcher (a POST hook on the viewport tick). hi_stop() removes it.
- hi_rows(start): the change rows since `start`; hi_census(): every watched value now.
- hi_pawn(): the player's state name, health, the injured timers (read, never written).
- hi_injure(): WillowPlayerPawn.PlayInjured on the player (script; memory only): the game's own down state starts.
- hi_second_wind(): the injured state's Behavior_GiveInjuredPlayerSecondWind(False), i.e. the game's kill-while-down
  path (KilledEnemy with no victim); the second wind follows by the game's own Tick.
- hi_face(name_part, distance): stand in front of the first AI pawn whose class definition path contains `name_part`,
  looking at it (memory only), for the interaction prompt.
"""
import json
import math
import time

import unrealsdk
from unrealsdk.hooks import Type, add_hook, remove_hook
from unrealsdk.unreal import UObject

HI_TICK = "WillowGame.WillowGameViewportClient:Tick"
HI_ID = "openwillow_hud_integrity"
HI = globals().get("HI") or {"on": False, "rows": [], "last": {}, "t0": None, "frame": 0, "errors": 0}

_N, _B, _S = "n", "b", "s"
HI_PATHS = []
for clip in ("dying1", "secondwind", "tools", "toolsSecondary", "contextualPrompt", "levelup", "xp", "challenges",
             "shielddown", "reload", "dmg_u", "dmg_d", "dmg_l", "dmg_r", "ring", "crouch", "boundry", "health",
             "shield", "bullets", "grenades", "crosshairs", "badassToken", "WorldDiscovery", "training",
             "moneyRemoval", "outerequip", "ShieldAbsorb", "activeAbility"):
    HI_PATHS += [(f"p1.{clip}._currentframe", _N), (f"p1.{clip}._visible", _B), (f"p1.{clip}._alpha", _N)]
HI_PATHS += [
    ("p1._visible", _B), ("p1._alpha", _N),
    ("p1.dying1.bkgd._visible", _B), ("p1.dying1.killRevive._currentframe", _N), ("p1.dying1.killRevive._visible", _B),
    ("p1.dying1.killRevive.inner.killReviveText.text", _S), ("p1.dying1.bleedout._currentframe", _N),
    ("p1.dying1.bleedout._visible", _B), ("p1.dying1.fight.text", _S), ("p1.dying1.fight._visible", _B),
    ("p1.dying1.suicide.text", _S), ("p1.dying1.suicide._visible", _B), ("p1.dying1.bar._currentframe", _N),
    ("p1.dying1.bar._visible", _B), ("p1.dying1.duel._visible", _B),
    ("p1.secondwind.inner.text1.text", _S), ("p1.secondwind.inner.text2.text", _S),
    ("p1.tools.desc.text", _S), ("p1.toolsSecondary.desc.text", _S),
    ("p1.contextualPrompt.inner.updateText.text", _S), ("p1.contextualPrompt.inner.glyphClip.glyphString.text", _S),
    ("p1.levelup.inner._currentframe", _N), ("p1.levelup.inner.levelDisp.text.text", _S),
    ("p1.xp.xp.xp.text", _S), ("p1.challenges.inner._currentframe", _N),
    ("p1.crosshairs._visible", _B), ("p1.crosshairs.outer._currentframe", _N),
    ("p1.crosshairs.outer.weapon._currentframe", _N), ("p1.crosshairs.outer.weapon.up._currentframe", _N),
    ("p1.health.health_disp.text", _S), ("p1.shield.shield_disp.text", _S), ("p1.bullets.bullet_c.text", _S),
    ("p1.MissionNotifications._visible", _B),
]
for k in range(1, 9):
    m = f"p1.MissionNotifications.Module{k}"
    HI_PATHS += [(f"{m}._currentframe", _N), (f"{m}._visible", _B), (f"{m}._y", _N), (f"{m}.inner._currentframe", _N),
                 (f"{m}.inner.text.text", _S), (f"{m}.inner.text.htmlText", _S), (f"{m}.inner.healthBar._currentframe", _N)]


def _movie():
    hud = get_pc().myHUD
    return getattr(hud, "HUDMovie", None) if hud is not None else None


def _read(movie, path, kind):
    try:
        if kind == _N:
            v = movie.GetVariableNumber(path)
            return round(v, 3) if isinstance(v, float) else v
        if kind == _B:
            return bool(movie.GetVariableBool(path))
        return movie.GetVariableString(path)
    except Exception as error:
        return f"<{type(error).__name__}>"


def _game_t():
    try:
        return round(get_pc().WorldInfo.TimeSeconds, 4)
    except Exception:
        return None


def hi_pawn():
    pc = get_pc()
    pawn = pc.Pawn
    out = {"t": time.perf_counter(), "game_t": _game_t(), "pawn": pawn._path_name() if pawn is not None else None}
    if pawn is None:
        return out
    for name in ("Health", "HealthMax", "InjuredState", "InjuredDeadState", "flInjuredTargetedTime", "TotalBleedoutTime",
                 "NumSuccessiveInjuries", "SecondWindReason", "bIsInjured"):
        try:
            v = getattr(pawn, name)
            out[name] = v if isinstance(v, (bool, int, float, str)) or v is None else repr(v)[:80]
        except Exception as error:
            out[name] = f"<{type(error).__name__}>"
    try:
        out["state"] = str(pawn.GetStateName())
    except Exception as error:
        out["state"] = f"<{type(error).__name__}>"
    return out


def _tick(*_args):
    if not HI["on"]:
        return
    HI["frame"] += 1
    movie = _movie()
    if movie is None:
        return
    now = time.perf_counter()
    changed = {}
    for path, kind in HI_PATHS:
        v = _read(movie, path, kind)
        if HI["last"].get(path, "<unset>") != v:
            HI["last"][path] = v
            changed[path] = v
    if changed:
        HI["rows"].append({"t": now, "game_t": _game_t(), "frame": HI["frame"], "changed": changed})


def hi_start():
    HI["on"] = True
    HI["t0"] = time.perf_counter()
    add_hook(HI_TICK, Type.POST_UNCONDITIONAL, HI_ID, _tick)
    return len(HI_PATHS)


def hi_stop():
    HI["on"] = False
    remove_hook(HI_TICK, Type.POST_UNCONDITIONAL, HI_ID)
    return len(HI["rows"])


def hi_rows(start=0):
    return HI["rows"][start:]


def hi_census():
    movie = _movie()
    return {path: _read(movie, path, kind) for path, kind in HI_PATHS} if movie is not None else {"error": "no movie"}


def hi_injure():
    pc = get_pc()
    pawn = pc.Pawn
    loc = pawn.Location
    tries = []
    for args in ((pc, None, loc), (None, None, loc), (pc, None), ()):
        try:
            pawn.PlayInjured(*args)
            return {"ok": True, "args": len(args), "pawn": hi_pawn()}
        except Exception as error:
            tries.append(f"{len(args)}: {type(error).__name__}: {str(error)[:120]}")
    return {"ok": False, "tries": tries}


def hi_second_wind():
    try:
        get_pc().Pawn.Behavior_GiveInjuredPlayerSecondWind(False)
        return {"ok": True, "pawn": hi_pawn()}
    except Exception as error:
        return {"ok": False, "error": f"{type(error).__name__}: {str(error)[:160]}"}


def hi_face(name_part, distance=180.0):
    me = get_pc()
    target = None
    for p in unrealsdk.find_all("WillowAIPawn", False):
        if "Default__" in p._path_name():
            continue
        try:
            ai = p.AIClass._path_name() if p.AIClass is not None else ""
        except Exception:
            ai = ""
        if name_part.lower() in ai.lower():
            target = p
            break
    if target is None:
        return {"ok": False, "error": f"no AI pawn with '{name_part}' in its class definition"}
    yaw = target.Rotation.Yaw * 360.0 / 65536.0
    x = target.Location.X + math.cos(math.radians(yaw)) * distance
    y = target.Location.Y + math.sin(math.radians(yaw)) * distance
    z = target.Location.Z + 40.0
    me.Pawn.Location = unrealsdk.make_struct("Vector", X=x, Y=y, Z=z)
    me.Pawn.SetPhysics(2)
    dx, dy = target.Location.X - x, target.Location.Y - y
    me.SetRotation(unrealsdk.make_struct("Rotator", Pitch=0, Yaw=int(math.degrees(math.atan2(dy, dx)) * 65536 / 360), Roll=0))
    return {"ok": True, "target": target._path_name(), "at": [round(x, 1), round(y, 1), round(z, 1)]}


def hi_dump(path):
    with open(path, "w", encoding="utf-8") as handle:
        json.dump({"t0": HI["t0"], "rows": HI["rows"], "paths": [p for p, _ in HI_PATHS]}, handle, default=str)
    return len(HI["rows"])


def hi_out(obj):
    return json.dumps(obj, default=str, ensure_ascii=False)
