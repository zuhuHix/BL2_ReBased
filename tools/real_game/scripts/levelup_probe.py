"""Level-up observation helpers for the openwillow_realgame command channel (lane L1, 2026-10-07).

Run once through Invoke-GamePyFile (tools/real_game/realgame.ps1); our own code, no game data. Everything it
records goes under RG_OUT (the ignored local/realgame/...). Memory-only: the only writes to the game are the
documented script calls (SetCurrentValue on the health and shield pools to set up a half-health start, ExpEarn).

- lu_snap(): level, experience, skill points, health and shield (current / max / base) of the player.
- lu_attrs(): every numeric attribute of the controller, pawn, replication info and the resource pools as
  {"<object>.<name>": value, "<object>.<name>BaseValue": base}; lu_diff(a, b) lists what changed.
- lu_start() / lu_stop(): hook the level-up functions (PRE and POST) and the pools' SetCurrentValue /
  AddCurrentValueImpulse while a window is open, and record one row per call plus a sample every frame.
"""
import json
import time

import unrealsdk
from unrealsdk.hooks import Type, add_hook, remove_hook
from unrealsdk.unreal import UObject, WrappedArray, WrappedStruct

TICK = "WillowGame.WillowGameViewportClient:Tick"
lu = {"pool_paths": set(), "on": False, "rows": [], "hooks": [], "t0": None, "sample": False, "last_attrs": None}


def _num(v):
    return isinstance(v, (bool, int, float)) and not isinstance(v, type(None))


def _allprops(obj):
    out = []
    k = obj.Class
    while k is not None:
        out += list(k._properties())
        k = k.SuperField
    return out


def _pools(pawn):
    """The pawn's health and shield pool objects (looked up again every call)."""
    hp = pawn.HealthPool.Data if pawn is not None else None
    sp = pawn.ShieldArmor.Data if pawn is not None and pawn.ShieldArmor.Data is not None else None
    return hp, sp


def lu_snap():
    pc = get_pc()
    pawn = pc.Pawn
    pri = pc.PlayerReplicationInfo
    hp, sp = _pools(pawn)
    row = {
        "t": round(time.perf_counter() - lu["t0"], 4) if lu["t0"] else None,
        "game_t": round(pc.WorldInfo.TimeSeconds, 4),
        "level": pawn.GetExpLevel(),
        "pri_level": pri.ExpLevel,
        "exp": pc.GetExpPoints(),
        "next_at": pri.ExpPointsNextLevelAt,
        "skill_pts": pri.GeneralSkillPoints,
        "hp": pawn.GetHealth(),
        "hp_max": pawn.GetMaxHealth(),
        "shield": pawn.GetShieldStrength(),
        "shield_max": pawn.GetMaxShieldStrength(),
    }
    if hp is not None:
        row.update(hp_pool_cur=hp.CurrentValue, hp_pool_max=hp.MaxValue, hp_pool_base=hp.MaxValueBaseValue,
                   hp_pool_min=hp.MinValue)
    if sp is not None:
        row.update(sh_pool_cur=sp.CurrentValue, sh_pool_max=sp.MaxValue, sh_pool_base=sp.MaxValueBaseValue)
    try:
        row["exp_pool"] = pc.ExpPool.Data.CurrentValue
    except Exception:
        pass
    return row


def lu_attrs():
    pc = get_pc()
    pawn = pc.Pawn
    hp, sp = _pools(pawn)
    objs = {"pc": pc, "pawn": pawn, "pri": pc.PlayerReplicationInfo, "hp": hp, "sp": sp}
    try:
        objs["exp"] = pc.ExpPool.Data
    except Exception:
        pass
    out = {}
    for tag, obj in objs.items():
        if obj is None:
            continue
        for q in _allprops(obj):
            name = q.Name
            try:
                v = getattr(obj, name)
            except Exception:
                continue
            if _num(v):
                out[f"{tag}.{name}"] = v
    return out


def lu_diff(a, b, eps=1e-6):
    d = {}
    for k in sorted(set(a) | set(b)):
        x, y = a.get(k), b.get(k)
        if x is None or y is None or (abs(x - y) > eps if _num(x) and _num(y) else x != y):
            d[k] = [x, y]
    return d


def _row(kind, func, extra=None):
    r = {"kind": kind, "func": func}
    try:
        r.update(lu_snap())
    except Exception as e:  # keep the hook alive
        r["snap_error"] = repr(e)[:200]
    if extra:
        r.update(extra)
    lu["rows"].append(r)


def _mk(name, kind, deep, pool_filter=False):
    def hook(obj, args, ret, func):
        if not lu["on"]:
            return
        if pool_filter and obj._path_name() not in lu["pool_paths"]:
            return
        extra = None
        if pool_filter:
            extra = {"pool": obj._path_name().split(".")[-1]}
        try:
            extra = {**(extra or {}), "args": {p.Name: (getattr(args, p.Name) if _num(getattr(args, p.Name)) else repr(getattr(args, p.Name))[:80])
                              for p in func.func._properties() if hasattr(args, p.Name)}}
        except Exception:
            pass
        if deep:
            try:
                a = lu_attrs()
                last = lu["last_attrs"]
                extra = (extra or {})
                if last is not None:
                    extra["attr_diff_since_last_deep"] = lu_diff(last, a)
                lu["last_attrs"] = a
            except Exception as e:
                extra = (extra or {}); extra["attr_error"] = repr(e)[:200]
        _row(kind, name, extra)
    return hook


def _sample(*_a):
    if not lu["on"] or not lu["sample"]:
        return
    try:
        _row("frame", "tick")
    except Exception:
        pass


# (function path, deep attribute snapshot?)
WATCH = [
    ("WillowGame.WillowPlayerController:ExpEarn", False),
    ("WillowGame.WillowPlayerController:ExpLevelUp", True),
    ("WillowGame.WillowPlayerController:OnExpLevelChange", True),
    ("WillowGame.WillowPlayerController:RecalculateAttributeInitializedState", True),
    ("WillowGame.WillowPlayerController:ClientOnExpLevelChange", False),
    ("WillowGame.ExperienceResourcePool:ApplyExpPointsToExpLevel", False),
]


def lu_start(extra_funcs=()):
    lu_stop()
    lu["rows"] = []
    lu["t0"] = time.perf_counter()
    lu["last_attrs"] = None
    lu["on"] = True
    pawn = get_pc().Pawn
    hp, sp = _pools(pawn)
    funcs = list(WATCH) + [(f, False) for f in extra_funcs]
    pools = [o for o in (hp, sp, get_pc().ExpPool.Data) if o is not None]
    lu["pool_paths"] = {o._path_name() for o in pools}
    pool_funcs = set()
    for fname in ("SetCurrentValue", "AddCurrentValueImpulse"):
        pool_funcs.add(getattr(hp, fname).func._path_name())  # "Engine.ResourcePool:SetCurrentValue"
    seen = set()
    for path, deep in funcs + [(f, False) for f in sorted(pool_funcs)]:
        if path in seen:
            continue
        seen.add(path)
        for typ, tag in ((Type.PRE, "pre"), (Type.POST, "post")):
            ident = f"ow_lu_{tag}_{path}"
            add_hook(path, typ, ident, _mk(path.split(":")[-1], tag, deep, path in pool_funcs))
            lu["hooks"].append((path, typ, ident))
    add_hook(TICK, Type.POST_UNCONDITIONAL, "ow_lu_tick", _sample)
    lu["hooks"].append((TICK, Type.POST_UNCONDITIONAL, "ow_lu_tick"))
    return [h[0] for h in lu["hooks"]]


def lu_stop():
    lu["on"] = False
    for path, typ, ident in lu["hooks"]:
        try:
            remove_hook(path, typ, ident)
        except Exception:
            pass
    lu["hooks"] = []


def lu_save(path):
    with open(path, "w", encoding="utf-8") as f:
        json.dump(lu["rows"], f, indent=1, default=str)
    return len(lu["rows"])
