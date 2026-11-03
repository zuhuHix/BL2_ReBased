"""Slice projectiles lane live probe (real game): accuracy pool bloom and recovery, view kick, spread draws.

Load after weapon_firing_probe.py and weapon_parity2_probe.py (same command namespace; it uses their helpers: deep, _out,
_write_json, _get_pawn, _safe, _game_t, _safe_call, get_pc, WP2_TICK, wp2_autofire, wp2_try_level). Loading only defines
commands. Everything written is game data under RG_OUT (ignored local/). Memory-only reads plus hooks; no game state is
changed by this file.

  pj_start(tag)     sample every frame (viewport tick): game clock, the player's AccuracyPool (current, min, max), the
                    controller's kick fields, its view rotation, the held weapon's zoom effect, spread and clip; and log
                    every AddWeaponKick (its arguments), AddSpread (base aim in, aim out, spread) and GetAdjustedAimFor
                    (aim out) call with the clock.
  pj_stop()         remove the hooks and write <RG_OUT>/projectiles/<tag>.json; returns the row counts.
  pj_fields()       the controller's and weapon type's field names that mention kick, accuracy, wander or recoil, with
                    their values now (to settle the names the sampler reads).

The questions (WEAPON_PROJECTILES.md section 10): the pool's hold time and decay rate after a shot, its minimum and maximum
with each slice gun, the impulse per shot; whether the kick returns the view (an offset) or moves it for good (an added
rate); the AddSpread offsets' density (uniform yaw, chord pitch); the cone under zoom.
"""
import json
import time

try:
    import unrealsdk
    from unrealsdk.hooks import Type, add_hook, remove_hook
except ImportError:  # importable outside the game
    unrealsdk = None
    Type = add_hook = remove_hook = None

PJ_ID = "ow_projectiles"
PJ_KICK_FIELDS = ("TargetVerticalKickAmt", "TargetHorizontalKickAmt", "CurrentVerticalKickAmt", "CurrentHorizontalKickAmt",
                  "LastWeaponKickTime", "WanderingSmoothInDuration")
PJ_HOOKS = (
    ("WillowGame.WillowPlayerController:AddWeaponKick", "kick"),
    ("WillowGame.WillowWeapon:AddSpread", "spread"),
    ("WillowGame.WillowPlayerController:GetAdjustedAimFor", "aim"),
    ("WillowGame.WillowWeapon:FireAmmunition", "fire"),
)
pj_state = globals().setdefault("pj_state", {"tag": None, "frames": [], "calls": [], "hooks": [], "cap": 200000})


def _pj_rot(r):
    return None if r is None else [int(r.Pitch), int(r.Yaw), int(r.Roll)]


def _pj_pool(pc):
    pool = getattr(pc, "AccuracyPool", None)
    if pool is None:
        return None
    data = getattr(pool, "Data", None)
    row = {"current": getattr(data, "CurrentValue", None) if data is not None else None}
    for call in ("GetMinValue", "GetMaxValue", "GetTotalRegenRate"):
        row[call] = _safe_call(pool, call)
    return row


def _pj_frame(*_args):
    if pj_state["tag"] is None or len(pj_state["frames"]) >= pj_state["cap"]:
        return
    pc = get_pc()
    if pc is None or pc.Pawn is None:
        return
    weapon = pc.Pawn.Weapon
    row = {"t": _game_t(), "pool": _pj_pool(pc), "view": _pj_rot(getattr(pc, "Rotation", None))}
    for name in PJ_KICK_FIELDS:
        value = getattr(pc, name, None)
        row[name] = value if isinstance(value, (int, float)) or value is None else deep(value)
    if weapon is not None:
        row["zoom"] = _safe_call(weapon, "GetZoomEffect")
        row["spread"] = getattr(weapon, "Spread", None)
        row["clip"] = getattr(weapon, "ReloadCnt", None)
    pj_state["frames"].append(row)


def _pj_call(kind):
    def callback(obj, args, ret, func):
        if pj_state["tag"] is None or len(pj_state["calls"]) >= pj_state["cap"]:
            return
        row = {"kind": kind, "t": _game_t()}
        for prop in args._type._properties():
            try:
                row[prop.Name] = deep(getattr(args, prop.Name))
            except Exception:
                continue
        if ret is not None:
            row["ret"] = deep(ret)
        pj_state["calls"].append(row)
    return callback


def pj_start(tag):
    pj_stop(write=False)
    pj_state.update({"tag": str(tag), "frames": [], "calls": [], "hooks": []})
    add_hook(WP2_TICK, Type.POST, PJ_ID + "_tick", _safe(lambda o, a, r, f: _pj_frame()))
    pj_state["hooks"].append(WP2_TICK)
    for path, kind in PJ_HOOKS:
        try:
            add_hook(path, Type.POST, PJ_ID + "_" + kind, _safe(_pj_call(kind)))
            pj_state["hooks"].append(path)
        except Exception as error:
            pj_state["calls"].append({"kind": "hook_error", "path": path, "error": repr(error)[:160]})
    return {"tag": tag, "hooks": list(pj_state["hooks"])}


def pj_stop(write=True):
    for path in pj_state.get("hooks", []):
        for kind in ["tick"] + [k for _, k in PJ_HOOKS]:
            try:
                remove_hook(path, Type.POST, PJ_ID + "_" + kind)
            except Exception:
                pass
    pj_state["hooks"] = []
    tag, pj_state["tag"] = pj_state.get("tag"), None
    result = {"tag": tag, "frames": len(pj_state["frames"]), "calls": len(pj_state["calls"])}
    if write and tag:
        _write_json(_out("projectiles") / (tag + ".json"), {"frames": pj_state["frames"], "calls": pj_state["calls"]})
    return result


def pj_fields():
    pc, pawn = _get_pawn()
    terms = ("kick", "accuracy", "wander", "recoil", "spread")
    out = {}
    holders = [("pc", pc)]
    weapon = pawn.Weapon
    if weapon is not None:
        holders += [("weapon", weapon), ("type", weapon.DefinitionData.WeaponTypeDefinition)]
    for label, holder in holders:
        for prop in holder.Class._properties():
            if any(t in prop.Name.lower() for t in terms):
                try:
                    out[label + "." + prop.Name] = deep(getattr(holder, prop.Name))
                except Exception as error:
                    out[label + "." + prop.Name] = "<%s>" % type(error).__name__
    _write_json(_out("projectiles") / ("fields_%d.json" % int(time.time())), out)
    return out
