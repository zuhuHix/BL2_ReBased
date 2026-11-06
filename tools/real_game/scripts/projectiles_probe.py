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


# ---------------------------------------------------------------- grenades and status-chance inputs (2026-11-04)
#
#   pj_grenade_mod()       the equipped grenade mod: definition, balance, parts, and the GrenadeDamage /
#                          projectile-related attributes the item exposes (to compare with tools/grenade_mod.py).
#   pj_grenade_start(tag)  per frame: every live WillowProjectile (path, definition, location, velocity), plus each
#                          ThrowGrenade / ClientThrowGrenade / GrenadeThrowComplete call with the game clock, and the
#                          view (location, rotation) at each call: spawn point, launch velocity, release delay, fuse.
#   pj_grenade_stop()      remove the hooks and write <RG_OUT>/projectiles/<tag>.json.
#   pj_status_mods()       the controller's instigated status-chance modifiers (general and per element), read
#                          before a status run; with fp3_no_skills() first, the skills-off values.

PJ_GRENADE_HOOKS = (
    ("WillowGame.WillowPlayerController:ThrowGrenade", "throw"),
    ("WillowGame.WillowPlayerController:ClientThrowGrenade", "client_throw"),
    ("WillowGame.WillowPlayerController:GrenadeThrowComplete", "throw_complete"),
)
PJ_STATUS_MODS = ("InstigatedStatusEffectStatusChanceModifier", "InstigatedIncendiaryStatusChanceModifier",
                  "InstigatedCorrosiveStatusChanceModifier", "InstigatedShockStatusChanceModifier",
                  "InstigatedAmpStatusChanceModifier")
pj_gstate = globals().setdefault("pj_gstate", {"tag": None, "frames": [], "calls": [], "hooks": [], "cap": 100000})


def _pj_view(pc):
    try:
        pov = pc.PlayerCamera.CameraCache.POV
        return {"loc": [pov.Location.X, pov.Location.Y, pov.Location.Z], "rot": _pj_rot(pov.Rotation), "fov": pov.FOV}
    except Exception as error:
        return {"error": repr(error)[:120]}


def _pj_grenade_frame(*_args):
    if pj_gstate["tag"] is None or len(pj_gstate["frames"]) >= pj_gstate["cap"]:
        return
    rows = []
    for proj in unrealsdk.find_all("WillowProjectile", exact=False):
        try:
            if proj.bDeleteMe:
                continue
            loc, vel = proj.Location, proj.Velocity
            rows.append({"path": proj._path_name(), "def": deep(getattr(proj, "Definition", None)),
                         "loc": [loc.X, loc.Y, loc.Z], "vel": [vel.X, vel.Y, vel.Z]})
        except Exception:
            continue
    if rows:
        pj_gstate["frames"].append({"t": _game_t(), "projectiles": rows})


def _pj_grenade_call(kind):
    def callback(obj, args, ret, func):
        if pj_gstate["tag"] is None:
            return
        row = {"kind": kind, "t": _game_t(), "view": _pj_view(get_pc())}
        for prop in args._type._properties():
            try:
                row[prop.Name] = deep(getattr(args, prop.Name))
            except Exception:
                continue
        pj_gstate["calls"].append(row)
    return callback


def pj_grenade_start(tag):
    pj_grenade_stop(write=False)
    pj_gstate.update({"tag": str(tag), "frames": [], "calls": [], "hooks": []})
    add_hook(WP2_TICK, Type.POST, PJ_ID + "_gtick", _safe(lambda o, a, r, f: _pj_grenade_frame()))
    pj_gstate["hooks"].append((WP2_TICK, "_gtick"))
    for path, kind in PJ_GRENADE_HOOKS:
        for when, suffix in ((Type.PRE, "_pre"), (Type.POST, "_post")):
            try:
                add_hook(path, when, PJ_ID + "_g" + kind + suffix, _safe(_pj_grenade_call(kind + suffix)))
                pj_gstate["hooks"].append((path, "_g" + kind + suffix, when))
            except Exception as error:
                pj_gstate["calls"].append({"kind": "hook_error", "path": path, "error": repr(error)[:160]})
    return {"tag": tag, "hooks": len(pj_gstate["hooks"])}


def pj_grenade_stop(write=True):
    for entry in pj_gstate.get("hooks", []):
        path, suffix = entry[0], entry[1]
        when = entry[2] if len(entry) > 2 else Type.POST
        try:
            remove_hook(path, when, PJ_ID + suffix)
        except Exception:
            pass
    pj_gstate["hooks"] = []
    tag, pj_gstate["tag"] = pj_gstate.get("tag"), None
    result = {"tag": tag, "frames": len(pj_gstate["frames"]), "calls": len(pj_gstate["calls"])}
    if write and tag:
        _write_json(_out("projectiles") / (tag + ".json"), {"frames": pj_gstate["frames"], "calls": pj_gstate["calls"]})
    return result


def pj_grenade_mod():
    pc, pawn = _get_pawn()
    # The equipped mod sits on the inventory manager's item chain (Inventory -> NextItem), not in the backpack list.
    inv = pc.GetPawnInventoryManager()
    mod = _safe_call(pc, "GetCurrentGrenadeMod")
    mod = None if isinstance(mod, str) else mod
    item = getattr(inv, "ItemChain", None)
    while item is not None and mod is None:
        if item.Class.Name == "WillowGrenadeMod":
            mod = item
        item = getattr(item, "Inventory", None)
    if mod is None:
        return {"grenade_mod": None}
    out = {"grenade_mod": _path(mod), "definition_data": deep(getattr(mod, "DefinitionData", None)),
           "equipped": deep(getattr(mod, "bEquipped", None))}
    for prop in mod.Class._properties():
        name = prop.Name
        if any(t in name for t in ("Grenade", "Damage", "Radius", "Fuse", "Projectile")) and not name.endswith("ModifierStack"):
            try:
                out[name] = deep(getattr(mod, name))
            except Exception:
                continue
    _write_json(_out("projectiles") / ("grenade_mod_%d.json" % int(time.time())), out)
    return {k: out[k] for k in ("grenade_mod", "equipped")}


def pj_status_mods():
    pc = get_pc()
    out = {}
    for name in PJ_STATUS_MODS:
        try:
            out[name] = deep(getattr(pc, name))
        except Exception as error:
            out[name] = "<%s>" % type(error).__name__
    return out
