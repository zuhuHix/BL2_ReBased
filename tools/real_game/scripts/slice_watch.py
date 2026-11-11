"""Read-only recorder for the slice capture sessions, for the openwillow_realgame command channel (lane capture-prep, 2026-11-11).

Run through Invoke-GamePyFile (tools/real_game/realgame.ps1); tools/real_game/slice_watch.ps1 drives it from a capture script's
-WatchSpec switch. Our own code, no game data: the function names it hooks come from a spec file the runner passes (kept under the
ignored local/), and everything it records goes to RG_OUT (the ignored local/realgame/<set>/).

It only reads. Hooks are observers: they never return Block and never change an argument. Reads call getters and read fields;
nothing here sets a field, calls a setter, spawns, damages, moves, travels or sends input. What a capture does to the game stays the
capture's own, already reviewed, steps.

- sw_start(label, spec): spec is a dict (or the path of a JSON file) with
    funcs:       ["Class.Function", ...]  every declaration of Function in Class and in Class's states is hooked, PRE and POST
                 (POST rows carry the return value); a full path "Package.Class:Function" is hooked as written.
    frames:      true to log one row per frame (perf_counter, game time, the tick's DeltaTime argument, the real and game deltas).
    snap_every:  seconds of real time between automatic state snapshots (0 = only the ones the capture asks for).
    pawns_every: seconds between enemy reads (class, pools, GroundSpeed, collision cylinder, draw scale, distance); 0 = off.
    per_func_cap, row_cap, frame_cap: limits (a flood from one function cannot crowd out the rest).
  Every row has t (perf_counter since start), g (WorldInfo.TimeSeconds), f (frame index) and dt (the last frame's DeltaTime).
- sw_snap(label): one state snapshot: player location, rotation, velocity, cylinder, health and shield (current and max, every reader
  that answers), every live resource pool (definition, current, max), cash, level, skill points, the respawn checkpoint, every
  travel station's active flags, the frame-rate settings, and the loaded SDK mods with their enabled flag.
- sw_pawns(label): one read of every live AI pawn.
- sw_counts(), sw_dump(name), sw_stop().
"""
import json
import re
import time
from pathlib import Path

try:
    import unrealsdk
    from unrealsdk.hooks import Type, add_hook, has_hook, remove_hook
    from unrealsdk.unreal import UObject, WrappedArray, WrappedStruct
except ImportError:   # tests/slice_watch_test.py runs the pure parts without a game
    unrealsdk = None
    UObject = WrappedArray = WrappedStruct = ()

SW_TICK = "WillowGame.WillowGameViewportClient:Tick"
SW_ID = "ow_slice_watch"
SW_DEFAULTS = {"funcs": [], "frames": False, "snap_every": 0.0, "pawns_every": 0.0,
               "per_func_cap": 5000, "row_cap": 200000, "frame_cap": 300000}
SW = globals().get("SW") or {"on": False}


def sw_reset(label="", spec=None):
    """The recorder's state, fresh. Kept in the shared namespace so a second load of this file does not silently drop a recording."""
    SW.clear()
    SW.update(on=False, label=label, spec=dict(SW_DEFAULTS, **(spec or {})), t0=time.perf_counter(), frame=0, dt=None,
              last_g=None, last_t=None, rows=[], frames=[], snaps=[], pawns=[], counts={}, dropped={}, resolved=[], missing=[],
              hooks=[], errors=[], next_snap=0.0, next_pawns=0.0)
    return SW


def load_spec(spec):
    """A spec dict, from a dict or a JSON file. Unknown keys are refused (a typo would silently record nothing)."""
    if isinstance(spec, str):
        spec = json.loads(Path(spec).read_text(encoding="utf-8"))
    if not isinstance(spec, dict):
        raise ValueError("spec must be a JSON object")
    known = set(SW_DEFAULTS) | {"name", "notes", "checks"}
    unknown = sorted(set(spec) - known)
    if unknown:
        raise ValueError(f"unknown spec keys: {unknown}")
    funcs = spec.get("funcs", [])
    if not isinstance(funcs, list) or not all(isinstance(f, str) and parse_func(f) for f in funcs):
        raise ValueError("funcs must be a list of 'Class.Function' or 'Package.Class:Function' names")
    return {k: v for k, v in spec.items() if k in SW_DEFAULTS}


def parse_func(name):
    """('path', full) for a full hook path, ('decl', (class, function)) for Class.Function, or None for anything else."""
    if re.fullmatch(r"[A-Za-z_]\w*(\.[A-Za-z_]\w*)+:[A-Za-z_]\w*", name):
        return ("path", name)
    m = re.fullmatch(r"([A-Za-z_]\w*)\.([A-Za-z_]\w*)", name)
    return ("decl", (m.group(1), m.group(2))) if m else None


def _p(value, depth=0):
    """A plain, JSON-safe copy of a value: objects become their path names, structs dicts, arrays lists (16 items at most)."""
    if value is None or isinstance(value, (bool, int, float, str)):
        return round(value, 4) if isinstance(value, float) else value
    if UObject and isinstance(value, UObject):
        try:
            return value._path_name()
        except Exception:
            return repr(value)[:120]
    if depth >= 2:
        return repr(value)[:120]
    if WrappedStruct and isinstance(value, WrappedStruct):
        out = {}
        for q in value._type._properties():
            try:
                out[q.Name] = _p(getattr(value, q.Name), depth + 1)
            except Exception:
                pass
        return out
    if WrappedArray and isinstance(value, WrappedArray):
        return [_p(v, depth + 1) for v in list(value)[:16]]
    return repr(value)[:120]


def _args(args, func):
    out = {}
    try:
        inner = getattr(func, "func", func)
        for q in inner._properties():
            if hasattr(args, q.Name):
                out[q.Name] = _p(getattr(args, q.Name))
    except Exception as e:
        out["args_error"] = repr(e)[:100]
    return out


def _err(where, e):
    if len(SW["errors"]) < 200:
        SW["errors"].append(f"{where}: {repr(e)[:160]}")


def _pc():
    return get_pc()  # noqa: F821 - the driver's namespace provides get_pc


def _game_t():
    try:
        return round(_pc().WorldInfo.TimeSeconds, 4)
    except Exception:
        return None


def _now():
    return round(time.perf_counter() - SW["t0"], 4)


def _stamp():
    return {"t": _now(), "g": _game_t(), "f": SW["frame"], "dt": SW["dt"]}


def add_row(path, tag, obj, args, ret, func):
    """One hook row, within the caps. Pure apart from the reads; the hook wrapper calls it."""
    count = SW["counts"].get(path + "|" + tag, 0) + 1
    SW["counts"][path + "|" + tag] = count
    if count > SW["spec"]["per_func_cap"] or len(SW["rows"]) >= SW["spec"]["row_cap"]:
        SW["dropped"][path] = SW["dropped"].get(path, 0) + 1
        return None
    row = dict(_stamp(), func=path, tag=tag, obj=_p(obj), args=_args(args, func))
    if tag == "post":
        row["ret"] = _p(ret)
    SW["rows"].append(row)
    return row


def _observer(path, tag):
    def hook(obj, args, ret, func):
        if not SW.get("on"):
            return None
        try:
            add_row(path, tag, obj, args, ret, func)
        except Exception as e:
            _err(path, e)
        return None   # an observer: never Block, never a replacement value
    return hook


def frame_row(delta_arg, t_real, g):
    """The per-frame row: the tick's own DeltaTime argument beside the real and game-clock deltas since the last frame."""
    row = {"t": t_real, "g": g, "f": SW["frame"], "dt_arg": delta_arg,
           "dt_real": None if SW["last_t"] is None else round(t_real - SW["last_t"], 5),
           "dt_game": None if (SW["last_g"] is None or g is None) else round(g - SW["last_g"], 5)}
    SW["last_t"], SW["last_g"] = t_real, g
    return row


def _tick(obj, args, ret, func):
    if not SW.get("on"):
        return None
    try:
        SW["frame"] += 1
        delta = getattr(args, "DeltaTime", None)
        SW["dt"] = round(delta, 5) if isinstance(delta, float) else delta
        t_real = _now()
        row = frame_row(SW["dt"], t_real, _game_t())
        if SW["spec"]["frames"] and len(SW["frames"]) < SW["spec"]["frame_cap"]:
            SW["frames"].append(row)
        if SW["spec"]["snap_every"] and t_real >= SW["next_snap"]:
            SW["next_snap"] = t_real + SW["spec"]["snap_every"]
            sw_snap("auto")
        if SW["spec"]["pawns_every"] and t_real >= SW["next_pawns"]:
            SW["next_pawns"] = t_real + SW["spec"]["pawns_every"]
            sw_pawns("auto")
    except Exception as e:
        _err("tick", e)
    return None


def _declared(cls_name, func_name):
    """Path names of every declaration of func_name in the class and in its states (as combat_probe._declared_paths)."""
    cls = unrealsdk.find_class(cls_name)
    paths = []
    for field in cls._fields():
        kind = field.Class.Name
        if kind == "Function" and field.Name == func_name and field.Outer == cls:
            paths.append(field._path_name())
        elif kind == "State":
            try:
                for inner in field._fields():
                    if inner.Class.Name == "Function" and inner.Name == func_name:
                        paths.append(inner._path_name())
            except Exception:
                pass
    return paths


def _hook(path, kind, tag, fn):
    hid = f"{SW_ID}_{tag}"
    if has_hook(path, kind, hid):
        remove_hook(path, kind, hid)
    add_hook(path, kind, hid, fn)
    SW["hooks"].append((path, kind, hid))


def sw_start(label, spec):
    """Hook every function the spec names and start the tick. Returns what resolved and what did not."""
    sw_stop()
    sw_reset(label, load_spec(spec))
    _hook(SW_TICK, Type.POST_UNCONDITIONAL, "tick", _tick)
    for name in SW["spec"]["funcs"]:
        kind, what = parse_func(name)
        try:
            paths = [what] if kind == "path" else _declared(*what)
        except Exception as e:
            paths = []
            _err(name, e)
        if not paths:
            SW["missing"].append(name)
            continue
        for path in paths:
            try:
                _hook(path, Type.PRE, "pre", _observer(path, "pre"))
                _hook(path, Type.POST, "post", _observer(path, "post"))
                SW["resolved"].append(path)
            except Exception as e:
                SW["missing"].append(f"{path} ({repr(e)[:80]})")
    SW["on"] = True
    first = sw_snap("start")
    return {"label": label, "resolved": len(SW["resolved"]), "missing": SW["missing"], "start_snap_keys": sorted(first)}


def _read(obj, names):
    """Each name that answers: a field, or a getter called with no arguments (getters only, by the caller's choice of names)."""
    out = {}
    for name in names:
        try:
            value = getattr(obj, name)
            out[name] = _p(value() if callable(value) else value)
        except Exception:
            pass
    return out


PLAYER_FIELDS = ("Location", "Rotation", "Velocity", "Physics", "GroundSpeed", "DrawScale", "Health", "HealthMax",
                 "GetHealth", "GetMaxHealth", "GetShieldStrength", "GetMaxShieldStrength", "NumSuccessiveInjuries", "bHidden")
PAWN_FIELDS = ("Location", "GroundSpeed", "WalkingPct", "DrawScale", "DrawScale3D", "Health", "HealthMax",
               "GetHealth", "GetMaxHealth", "GetShieldStrength", "GetMaxShieldStrength", "Physics")
STATION_FIELDS = ("bShouldBeActive", "bIsCurrentlyActive", "Location")


def _live(objs):
    return [o for o in objs if "Default__" not in o._path_name()]


def _cylinder(pawn):
    try:
        c = pawn.CylinderComponent
        return {"radius": _p(c.CollisionRadius), "height": _p(c.CollisionHeight)}
    except Exception:
        return None


def _pools():
    rows = []
    for pool in _live(unrealsdk.find_all("ResourcePool", False)):
        row = {"pool": pool._path_name()}
        row.update(_read(pool, ("Definition", "AssociatedProvider", "CurrentValue", "GetMaxValue", "MaxValue")))
        rows.append(row)
    return rows


def _mods():
    try:
        import mods_base
        return [{"name": m.name, "enabled": bool(getattr(m, "is_enabled", False)), "type": _p(getattr(m, "mod_type", None))}
                for m in mods_base.get_ordered_mod_list()]
    except Exception as e:
        return [{"error": repr(e)[:120]}]


def _engine():
    for engine in _live(unrealsdk.find_all("GameEngine", False)):
        return _read(engine, ("bSmoothFrameRate", "MinSmoothedFrameRate", "MaxSmoothedFrameRate"))
    return None


def sw_snap(label):
    """One state snapshot (reads only), kept in the dump and returned."""
    snap = dict(_stamp(), label=label)
    try:
        pc = _pc()
        pawn = pc.Pawn
        pri = pc.PlayerReplicationInfo
        snap["map"] = _p(pc.WorldInfo.GetStreamingPersistentMapName())
        snap["player"] = dict(_read(pawn, PLAYER_FIELDS), cls=pawn.Class.Name, cylinder=_cylinder(pawn)) if pawn else None
        snap["controller"] = _read(pc, ("Rotation", "LastVisitedTeleporter", "FocusCamReturnTime", "CurrentSeenPickupable",
                                        "CurrentTouchedPickupable", "bCinematicMode"))
        snap["pri"] = _read(pri, ("ExpLevel", "GeneralSkillPoints"))
        try:
            snap["cash"] = pri.GetCurrencyOnHand(0)
        except Exception as e:
            snap["cash"] = repr(e)[:80]
        snap["checkpoint"] = _read(pc.WorldInfo.GRI, ("ActiveRespawnCheckpointTeleportActor",))
        snap["stations"] = [dict(_read(s, STATION_FIELDS), name=s._path_name()) for s in _live(unrealsdk.find_all("TravelStation", False))]
        snap["pools"] = _pools()
    except Exception as e:
        _err("snap " + label, e)
    if label in ("start", "end") or label.startswith("mods"):
        snap["mods"] = _mods()
        snap["engine"] = _engine()
    SW["snaps"].append(snap)
    return snap


def sw_pawns(label):
    """One read of every live AI pawn: class, pools, speeds, cylinder, scale, distance to the player."""
    out = dict(_stamp(), label=label, pawns=[])
    try:
        player = _pc().Pawn
        here = player.Location if player else None
        for pawn in _live(unrealsdk.find_all("WillowAIPawn", False)):
            row = dict(_read(pawn, PAWN_FIELDS), name=pawn._path_name(), cls=pawn.Class.Name, cylinder=_cylinder(pawn))
            try:
                row["ai_class"] = _p(pawn.AIClass)
            except Exception:
                pass
            if here is not None:
                try:
                    loc = pawn.Location
                    row["dist"] = round(((loc.X - here.X) ** 2 + (loc.Y - here.Y) ** 2 + (loc.Z - here.Z) ** 2) ** 0.5, 1)
                except Exception:
                    pass
            out["pawns"].append(row)
    except Exception as e:
        _err("pawns " + label, e)
    SW["pawns"].append(out)
    return {"label": label, "pawns": len(out["pawns"])}


def sw_counts():
    """Rows per function and tag, frames and snapshots so far: the in-game usability check reads this."""
    return {"rows": len(SW.get("rows", [])), "frames": len(SW.get("frames", [])), "snaps": len(SW.get("snaps", [])),
            "pawn_reads": len(SW.get("pawns", [])), "counts": dict(SW.get("counts", {})), "dropped": dict(SW.get("dropped", {})),
            "errors": len(SW.get("errors", [])), "missing": list(SW.get("missing", []))}


def sw_root():
    """RG_OUT resolved, refused unless it lies inside a repository's ignored local/ folder (as loot_capture.lc_root)."""
    root = Path(RG_OUT).resolve()  # noqa: F821 - the command channel's prelude sets RG_OUT
    if not any(a.name == "local" and (a.parent / ".git").exists() for a in root.parents):
        raise ValueError(f"RG_OUT {root} is not inside a repository's local/ folder")
    return root


def sw_dump(name):
    """Write everything to RG_OUT/<name>.json; the name must be a plain file name. Returns the counts."""
    if not isinstance(name, str) or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_-]*", name):
        raise ValueError(f"dump name must be a plain file name, got {name!r}")
    root = sw_root()
    root.mkdir(parents=True, exist_ok=True)
    data = {"label": SW.get("label"), "spec": SW.get("spec"), "resolved": SW.get("resolved", []), "missing": SW.get("missing", []),
            "counts": SW.get("counts", {}), "dropped": SW.get("dropped", {}), "errors": SW.get("errors", []),
            "rows": SW.get("rows", []), "frames": SW.get("frames", []), "snaps": SW.get("snaps", []), "pawns": SW.get("pawns", [])}
    (root / f"{name}.json").write_text(json.dumps(data, default=str), encoding="utf-8")
    return sw_counts()


def sw_stop():
    """Remove every hook this recorder added. Safe to call twice."""
    SW["on"] = False
    for path, kind, hid in SW.get("hooks", []):
        try:
            remove_hook(path, kind, hid)
        except Exception:
            pass
    SW["hooks"] = []
    return "stopped"
