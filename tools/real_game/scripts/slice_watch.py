"""Read-only recorder for the slice capture sessions, for the openwillow_realgame command channel (lane capture-prep, 2026-11-11).

Run through Invoke-GamePyFile (tools/real_game/realgame.ps1); tools/real_game/slice_watch.ps1 drives it from a capture script's
-WatchSpec switch. Our own code, no game data: the function names it hooks come from a spec file the runner passes (kept under the
ignored local/), and everything it records goes to RG_OUT (the ignored local/realgame/<set>/).

It only reads. Hooks are observers: they never return Block and never change an argument. Nothing here sets a field, calls a
setter, spawns, damages, moves, travels or sends input.

Safe by construction (after the 2026-11-11 session 2 crashes, an access violation about two seconds after the first pawn read, twice):
- A native getter is called only on the player's own controller and pawn, and on live AI pawns: objects named WillowAIPawn_<n>,
  of that exact class, not bDeleteMe. That is how the tracked probes do it (combat_probe.cb_enemies, den_probe.dp_begin): find_all of
  the class, with the preloaded Pawn_<Type> templates left out. Every other object a scan returns (resource pools, travel stations,
  pickups, the engine) is read by its fields only.
- No scan at all until the player has a pawn AND the map's own clock (WorldInfo.TimeSeconds, which restarts at every map load) has
  passed settle_seconds. A labelled snapshot or pawn read asked for earlier is deferred: the tick takes it once the map has settled,
  and the line carries the time it was asked for.
- Each kind of read is wrapped. Its first failure is counted and switches that kind off for the rest of the run (it is never retried
  each tick); the line names it in 'off'.

The driver runs every command script in ONE shared namespace (openwillow_realgame/__init__.py, `scope`), so every top-level name
here starts with sw_, _sw_, SW or _Sw: nothing a tracked probe defines is replaced, and no probe can replace these
(tests/slice_watch_test.py checks it against each capture's scripts in their real order).

Output: one JSON-lines file, RG_OUT/<name>.jsonl, written as it goes (flushed every flush_every seconds of real time), so a crash
keeps what came before and memory holds only the last few seconds. Line kinds: meta (first), row, frame, snap, pawns, end (last;
absent after a crash). Every stream has a cap, and so has the file's total size; what a cap drops is counted.

Spec (a dict or the path of a JSON file; sw_load_spec refuses unknown keys, wrong types and values outside fixed bounds):
  funcs         ["Class.Function" | "Package.Class:Function"]: every declaration of Function in Class and its states, PRE and POST;
                a full path is looked up first and listed as missing when the game has no such function.
  frames        bool: one row per frame (perf_counter, game time, the tick's DeltaTime, real and game deltas, and 'busy' with the
                milliseconds of any automatic read or flush that ran in that frame, so the analysis can leave those frames out).
  frame_player  bool: the player's location and hidden flag in each frame row.
  pickups       bool: every live pickup in each LABELLED snapshot (item definition, LifeSpan, draw scale, location).
  snap_every    seconds between automatic snapshots (0 = none). Automatic snapshots are light: player, controller, level, cash,
                checkpoint; pools, stations, pickups and mods come only in labelled snapshots (the capture's own sw_snap calls).
  pawns_every   seconds between automatic pawn reads (0 = none).
  pawns_paths   full object paths: an automatic pawn read looks up only these (a name lookup, not a scan of every object); each must
                still be a live WillowAIPawn_<n>.
  settle_seconds  the map clock a scan waits for after every load (default 15).
  pawns_match   substrings: when no paths are given, only pawns whose path, class or AI class holds one are kept.
  caps          per_func_cap, row_cap, frame_cap, snap_cap, pawn_cap, byte_cap; flush_every.
"""
import json as _sw_json
import math as _sw_math
import re as _sw_re
import time as _sw_time
from pathlib import Path as _SwPath

try:
    import unrealsdk as _sw_sdk
    from unrealsdk.hooks import Type as _SwType, add_hook as _sw_add_hook, has_hook as _sw_has_hook, remove_hook as _sw_remove_hook
    from unrealsdk.unreal import UObject as _SwUObject, WrappedArray as _SwArray, WrappedStruct as _SwStruct
except ImportError:   # tests/slice_watch_test.py runs the pure parts without a game
    _sw_sdk = None
    _SwType = _sw_add_hook = _sw_has_hook = _sw_remove_hook = None
    _SwUObject = _SwArray = _SwStruct = ()

SW_TICK = "WillowGame.WillowGameViewportClient:Tick"
SW_ID = "ow_slice_watch"
# name: (type, default, lowest, highest). Numbers must be finite; a cadence of 0 means off, otherwise it must lie in its range.
SW_SPEC_KEYS = {
    "funcs": (list, [], 0, 300), "frames": (bool, False, None, None), "frame_player": (bool, False, None, None),
    "pickups": (bool, False, None, None), "snap_every": (float, 0.0, 0.5, 600.0), "pawns_every": (float, 0.0, 0.1, 600.0),
    "pawns_paths": (list, [], 0, 20), "pawns_match": (list, [], 0, 20),
    "per_func_cap": (int, 5000, 1, 20000), "row_cap": (int, 200000, 1, 400000), "frame_cap": (int, 300000, 1, 400000),
    "snap_cap": (int, 5000, 1, 20000), "pawn_cap": (int, 20000, 1, 50000), "byte_cap": (int, 300_000_000, 1_000_000, 1_000_000_000),
    "flush_every": (float, 2.0, 0.5, 30.0), "settle_seconds": (float, 15.0, 5.0, 120.0),
}
SW_DOC_KEYS = ("name", "notes", "checks")
SW_LABELLED_CAP = 200   # labelled snapshots (the capture's own) are kept beyond snap_cap, up to this many
SW = globals().get("SW") or {"on": False}


def sw_reset(label="", spec=None):
    """The recorder's state, fresh. Kept in the shared namespace so a second load of this file does not silently drop a recording."""
    SW.clear()
    SW.update(on=False, label=label, spec=dict(sw_defaults(), **(spec or {})), t0=_sw_time.perf_counter(), frame=0, dt=None,
              last_g=None, last_t=None, buf=[], fh=None, path=None, bytes=0,
              n={"row": 0, "frame": 0, "snap": 0, "pawns": 0, "labelled": 0}, counts={}, dropped={}, resolved=[], missing=[],
              hooks=[], errors=[], auto_failures=0, next_snap=0.0, next_pawns=0.0, next_flush=0.0, busy=[],
              off=set(), switched_off={}, deferred=[])
    return SW


def sw_defaults():
    return {k: (list(v[1]) if isinstance(v[1], list) else v[1]) for k, v in SW_SPEC_KEYS.items()}


def sw_load_spec(spec):
    """A checked spec dict, from a dict or a JSON file. Refuses unknown keys, wrong types, non-finite numbers and values out of range."""
    if isinstance(spec, str):
        spec = _sw_json.loads(_SwPath(spec).read_text(encoding="utf-8"))
    if not isinstance(spec, dict):
        raise ValueError("spec must be a JSON object")
    unknown = sorted(set(spec) - set(SW_SPEC_KEYS) - set(SW_DOC_KEYS))
    if unknown:
        raise ValueError(f"unknown spec keys: {unknown}")
    out = sw_defaults()
    for key, (kind, _default, lo, hi) in SW_SPEC_KEYS.items():
        if key not in spec:
            continue
        value = spec[key]
        cadence = key in ("snap_every", "pawns_every")
        if kind is bool:
            if not isinstance(value, bool):
                raise ValueError(f"{key} must be true or false")
        elif kind is list:
            if not isinstance(value, list) or not all(isinstance(v, str) and v for v in value) or len(value) > hi:
                raise ValueError(f"{key} must be a list of at most {hi} non-empty strings")
        else:
            if isinstance(value, bool) or not isinstance(value, (int, float)) or not _sw_math.isfinite(value):
                raise ValueError(f"{key} must be a finite number")
            if kind is int and value != int(value):
                raise ValueError(f"{key} must be a whole number")
            if not (cadence and value == 0) and not (lo <= value <= hi):
                raise ValueError(f"{key} must be {'0 or ' if cadence else ''}between {lo} and {hi}")
            value = kind(value)
        out[key] = value
    bad = [f for f in out["funcs"] if not sw_parse_func(f)]
    if bad:
        raise ValueError(f"funcs holds names that are neither 'Class.Function' nor 'Package.Class:Function': {bad[:3]}")
    if "checks" in spec and not isinstance(spec["checks"], dict):
        raise ValueError("checks must be a JSON object")
    return out


def sw_parse_func(name):
    """('path', full) for a full hook path, ('decl', (class, function)) for Class.Function, or None for anything else."""
    if _sw_re.fullmatch(r"[A-Za-z_]\w*(\.[A-Za-z_]\w*)+:[A-Za-z_]\w*", name):
        return ("path", name)
    m = _sw_re.fullmatch(r"([A-Za-z_]\w*)\.([A-Za-z_]\w*)", name)
    return ("decl", (m.group(1), m.group(2))) if m else None


def _sw_p(value, depth=0):
    """A plain, JSON-safe copy of a value: objects become their path names, structs dicts, arrays lists (the first 16 items only)."""
    if value is None or isinstance(value, (bool, int, str)):
        return value
    if isinstance(value, float):
        return round(value, 4) if _sw_math.isfinite(value) else repr(value)
    if _SwUObject and isinstance(value, _SwUObject):
        try:
            return value._path_name()
        except Exception:
            return repr(value)[:120]
    if depth >= 2:
        return repr(value)[:120]
    if _SwStruct and isinstance(value, _SwStruct):
        out = {}
        for q in value._type._properties():
            try:
                out[q.Name] = _sw_p(getattr(value, q.Name), depth + 1)
            except Exception:
                pass
        return out
    if _SwArray and isinstance(value, _SwArray):
        return [_sw_p(value[i], depth + 1) for i in range(min(len(value), 16))]   # by index: no copy of the whole array
    return repr(value)[:120]


def _sw_args(args, func):
    out = {}
    try:
        inner = getattr(func, "func", func)
        for q in inner._properties():
            if hasattr(args, q.Name):
                out[q.Name] = _sw_p(getattr(args, q.Name))
    except Exception as e:
        out["args_error"] = repr(e)[:100]
    return out


def _sw_err(where, e):
    if len(SW["errors"]) < 200:
        SW["errors"].append(f"{where}: {repr(e)[:160]}")


def sw_switch_off(kind, e):
    """A read kind failed: count it, name it, and never try it again in this run."""
    if kind not in SW["off"]:
        SW["off"].add(kind)
        SW["switched_off"][kind] = repr(e)[:160]
    _sw_err("read " + kind, e)


def sw_settled():
    """Whether scans may run: the player has a pawn and the map's own clock has passed settle_seconds since this map loaded."""
    try:
        pc = _sw_pc()
        if pc.Pawn is None:
            return False
        return float(pc.WorldInfo.TimeSeconds) >= SW["spec"]["settle_seconds"]
    except Exception:
        return False


def _sw_pc():
    return get_pc()  # noqa: F821 - the driver's namespace provides get_pc


def _sw_game_t():
    try:
        return round(_sw_pc().WorldInfo.TimeSeconds, 4)
    except Exception:
        return None


def _sw_now():
    return round(_sw_time.perf_counter() - SW["t0"], 4)


def _sw_stamp():
    return {"t": _sw_now(), "g": _sw_game_t(), "f": SW["frame"], "dt": SW["dt"]}


# ---------------------------------------------------------------- the stream
def sw_emit(kind, record, cap_key):
    """Queue one line, within its stream's cap; counts what a cap drops. The tick writes the queue out (sw_flush)."""
    if SW["n"][kind] >= SW["spec"][cap_key]:
        SW["dropped"][kind] = SW["dropped"].get(kind, 0) + 1
        return False
    SW["n"][kind] += 1
    SW["buf"].append(dict(record, k=kind))
    return True


def sw_flush():
    """Write the queued lines and flush the file; stop writing (count as dropped) once byte_cap is reached."""
    fh = SW.get("fh")
    buf, SW["buf"] = SW.get("buf", []), []
    if fh is None:
        return 0
    written = 0
    for record in buf:
        line = _sw_json.dumps(record, default=str) + "\n"
        if SW["bytes"] + len(line) > SW["spec"]["byte_cap"]:
            SW["dropped"]["bytes"] = SW["dropped"].get("bytes", 0) + 1
            continue
        fh.write(line)
        SW["bytes"] += len(line)
        written += 1
    fh.flush()
    return written


def sw_add_row(path, tag, obj, args, ret, func):
    """One hook row, within the per-function and row caps."""
    count = SW["counts"].get(path + "|" + tag, 0) + 1
    SW["counts"][path + "|" + tag] = count
    if count > SW["spec"]["per_func_cap"]:
        SW["dropped"][path] = SW["dropped"].get(path, 0) + 1
        return None
    row = dict(_sw_stamp(), func=path, tag=tag, obj=_sw_p(obj), args=_sw_args(args, func))
    if tag == "post":
        row["ret"] = _sw_p(ret)
    return row if sw_emit("row", row, "row_cap") else None


def _sw_observer(path, tag):
    def hook(obj, args, ret, func):
        if not SW.get("on"):
            return None
        try:
            sw_add_row(path, tag, obj, args, ret, func)
        except Exception as e:
            _sw_err(path, e)
        return None   # an observer: never Block, never a replacement value
    return hook


def sw_frame_row(delta_arg, t_real, g):
    """The per-frame row: the tick's own DeltaTime argument beside the real and game-clock deltas since the last frame."""
    row = {"t": t_real, "g": g, "f": SW["frame"], "dt_arg": delta_arg,
           "dt_real": None if SW["last_t"] is None else round(t_real - SW["last_t"], 5),
           "dt_game": None if (SW["last_g"] is None or g is None) else round(g - SW["last_g"], 5)}
    SW["last_t"], SW["last_g"] = t_real, g
    return row


def _sw_timed(name, fn, *a):
    """Run an automatic read or the flush, and note its milliseconds against this frame."""
    start = _sw_time.perf_counter()
    try:
        return fn(*a)
    finally:
        SW["busy"].append([name, round((_sw_time.perf_counter() - start) * 1000.0, 2)])


def _sw_tick(obj, args, ret, func):
    if not SW.get("on"):
        return None
    try:
        SW["frame"] += 1
        delta = getattr(args, "DeltaTime", None)
        SW["dt"] = round(delta, 5) if isinstance(delta, float) else delta
        t_real = _sw_now()
        row = sw_frame_row(SW["dt"], t_real, _sw_game_t())
        if SW["spec"]["frame_player"] and "frame_player" not in SW["off"]:
            row.update(_sw_player_here())
        SW["busy"] = []
        spec = SW["spec"]
        due_snap = spec["snap_every"] and t_real >= SW["next_snap"]
        due_pawns = spec["pawns_every"] and t_real >= SW["next_pawns"]
        settled = (due_snap or due_pawns or SW["deferred"]) and sw_settled()
        if settled and SW["deferred"]:
            # Labelled reads asked for before the map settled, taken now, oldest first.
            pending, SW["deferred"] = SW["deferred"], []
            for kind, label, asked in pending:
                _sw_timed("deferred", (sw_snap if kind == "snap" else sw_pawns), label, asked)
        if (due_snap or due_pawns) and settled:
            if due_snap:
                SW["next_snap"] = t_real + spec["snap_every"]
                _sw_timed("snap", _sw_auto, sw_snap, "auto")
            if due_pawns:
                SW["next_pawns"] = t_real + spec["pawns_every"]
                _sw_timed("pawns", _sw_auto, sw_pawns, "auto")
        if t_real >= SW["next_flush"]:
            SW["next_flush"] = t_real + spec["flush_every"]
            _sw_timed("flush", sw_flush)
        if SW["busy"]:
            row["busy"] = SW["busy"]
        if spec["frames"]:
            sw_emit("frame", row, "frame_cap")
    except Exception as e:
        _sw_err("tick", e)
    return None


def _sw_auto(fn, label):
    """An automatic read: a failure (a map load, a pawn going away) is counted as an automatic failure, not a hook error."""
    before = len(SW["errors"])
    result = fn(label)
    failed = len(SW["errors"]) - before
    if failed or (isinstance(result, dict) and result.get("failed")):
        SW["auto_failures"] += 1
    del SW["errors"][before:]
    return result


def _sw_player_here():
    """The player's location and hidden flag, for a frame row (fields of the player's own pawn); empty while there is no pawn.
    A failure switches this read off."""
    try:
        pawn = _sw_pc().Pawn
        if pawn is None:
            return {"loc": None}
        loc = pawn.Location
        return {"loc": [round(loc.X, 1), round(loc.Y, 1), round(loc.Z, 1)], "hidden": bool(pawn.bHidden)}
    except Exception as e:
        sw_switch_off("frame_player", e)
        return {"loc": None}


# ---------------------------------------------------------------- hooks
def _sw_declared(cls_name, func_name):
    """Path names of every declaration of func_name in the class and in its states (as combat_probe._declared_paths)."""
    cls = _sw_sdk.find_class(cls_name)
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


def _sw_exists(path):
    """A full function path exists in the running game (looked up first, as the driver's save block does)."""
    try:
        return _sw_sdk.find_object("Function", path) is not None
    except Exception:
        return False


def _sw_hook(path, kind, tag, fn):
    hid = f"{SW_ID}_{tag}"
    if _sw_has_hook(path, kind, hid):
        _sw_remove_hook(path, kind, hid)
    _sw_add_hook(path, kind, hid, fn)
    SW["hooks"].append((path, kind, hid))


def _sw_open(name):
    root = sw_root()
    root.mkdir(parents=True, exist_ok=True)
    path = root / f"{name}.jsonl"
    if path.exists():   # an earlier run's file is kept under a new name, never overwritten
        path.rename(root / f"{name}.{_sw_time.strftime('%Y%m%d-%H%M%S')}.jsonl")
    SW["path"] = path
    SW["fh"] = open(path, "w", encoding="utf-8")


def sw_start(label, spec, name="watch"):
    """Check the spec, open RG_OUT/<name>.jsonl, hook every function the spec names, start the tick. Returns what resolved and what not."""
    if not isinstance(name, str) or not _sw_re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_-]*", name):
        raise ValueError(f"output name must be a plain file name, got {name!r}")
    checked = sw_load_spec(spec)
    sw_stop()
    sw_reset(label, checked)
    _sw_open(name)
    _sw_hook(SW_TICK, _SwType.POST_UNCONDITIONAL, "tick", _sw_tick)
    for fname in SW["spec"]["funcs"]:
        kind, what = sw_parse_func(fname)
        try:
            paths = ([what] if _sw_exists(what) else []) if kind == "path" else _sw_declared(*what)
        except Exception as e:
            paths = []
            _sw_err(fname, e)
        if not paths:
            SW["missing"].append(fname)
            continue
        for path in paths:
            try:
                _sw_hook(path, _SwType.PRE, "pre", _sw_observer(path, "pre"))
                _sw_hook(path, _SwType.POST, "post", _sw_observer(path, "post"))
                SW["resolved"].append(path)
            except Exception as e:
                SW["missing"].append(f"{path} ({repr(e)[:80]})")
    SW["buf"].append({"k": "meta", "label": label, "t0_qpc": SW["t0"], "spec": SW["spec"], "resolved": SW["resolved"],
                      "missing": SW["missing"]})
    SW["on"] = True
    first = sw_snap("start")
    sw_flush()
    return {"label": label, "file": str(SW["path"]), "resolved": len(SW["resolved"]), "missing": SW["missing"],
            "start_failed": first.get("failed", [])}


# ---------------------------------------------------------------- reads
def _sw_read(obj, names):
    """Each name that answers: a field, or a getter called with no arguments (getters only, by the caller's choice of names)."""
    out = {}
    for name in names:
        try:
            value = getattr(obj, name)
            out[name] = _sw_p(value() if callable(value) else value)
        except Exception:
            pass
    return out


SW_PLAYER_FIELDS = ("Location", "Rotation", "Velocity", "Physics", "GroundSpeed", "DrawScale", "Health", "HealthMax",
                    "GetHealth", "GetMaxHealth", "GetShieldStrength", "GetMaxShieldStrength", "NumSuccessiveInjuries", "bHidden")
SW_PAWN_FIELDS = ("Location", "GroundSpeed", "WalkingPct", "DrawScale", "DrawScale3D", "Health", "HealthMax",
                  "GetHealth", "GetMaxHealth", "GetShieldStrength", "GetMaxShieldStrength", "Physics")
SW_STATION_FIELDS = ("bShouldBeActive", "bIsCurrentlyActive", "Location")


def _sw_live(objs):
    """Scanned objects that are not class defaults and not being destroyed. Their fields may be read; no native getter is called on them."""
    out = []
    for o in objs:
        if "Default__" in o._path_name():
            continue
        try:
            if o.bDeleteMe:
                continue
        except Exception:
            pass
        out.append(o)
    return out


def sw_live_pawn(pawn):
    """A live AI pawn, the only scanned object a native getter is called on: named WillowAIPawn_<n> (not a preloaded Pawn_<Type>
    template), not a class default, not bDeleteMe (as combat_probe.cb_enemies and den_probe.dp_begin)."""
    try:
        return (pawn is not None and pawn.Name.startswith("WillowAIPawn_") and "Default__" not in pawn._path_name()
                and not pawn.bDeleteMe)
    except Exception:
        return False


def _sw_cylinder(pawn):
    try:
        c = pawn.CylinderComponent
        return {"radius": _sw_p(c.CollisionRadius), "height": _sw_p(c.CollisionHeight)}
    except Exception:
        return None


SW_POOL_FIELDS = ("Definition", "AssociatedProvider", "CurrentValue", "MaxValue", "MaxValueBaseValue")


def _sw_fields(obj, names):
    """Fields only: a name that is a function is skipped, never called."""
    out = {}
    for name in names:
        try:
            value = getattr(obj, name)
            if not callable(value):
                out[name] = _sw_p(value)
        except Exception:
            pass
    return out


def _sw_pools():
    """Live pools that belong to someone (AssociatedProvider set), by their fields only (as action_skill_hud._find_pool reads them)."""
    rows = []
    for pool in _sw_live(_sw_sdk.find_all("ResourcePool", False)):
        row = dict(_sw_fields(pool, SW_POOL_FIELDS), pool=pool._path_name())
        if row.get("AssociatedProvider"):
            rows.append(row)
    return rows


def _sw_pickups():
    rows = []
    for pk in _sw_live(_sw_sdk.find_all("WillowPickup", False)):
        row = dict(_sw_fields(pk, ("LifeSpan", "DrawScale", "Location")), name=pk._path_name())
        try:
            row["item"] = _sw_p(pk.Inventory.DefinitionData.ItemDefinition)
        except Exception:
            try:
                row["item"] = _sw_p(pk.Inventory.DefinitionData.WeaponTypeDefinition)
            except Exception:
                row["item"] = None
        rows.append(row)
    return rows


def _sw_mods():
    import mods_base as _sw_mods_base
    return [{"name": m.name, "enabled": bool(getattr(m, "is_enabled", False)), "type": _sw_p(getattr(m, "mod_type", None))}
            for m in _sw_mods_base.get_ordered_mod_list()]


def _sw_engine():
    for engine in _sw_live(_sw_sdk.find_all("GameEngine", False)):
        return _sw_fields(engine, ("bSmoothFrameRate", "MinSmoothedFrameRate", "MaxSmoothedFrameRate"))
    return {}


def sw_snap(label, asked=None):
    """One state snapshot (reads only), queued for the file and returned. Each part reads on its own; 'failed' names every part that
    raised, so a usability check can tell a failed read from an empty one. Automatic snapshots (label 'auto') are light; a labelled
    one adds stations, pools, pickups (when the spec asks), the engine's frame-rate settings and the loaded mods."""
    auto = label == "auto"
    if auto and SW["n"]["snap"] >= SW["spec"]["snap_cap"]:
        SW["dropped"]["snap"] = SW["dropped"].get("snap", 0) + 1
        return {}
    if not auto and SW["n"]["labelled"] >= SW_LABELLED_CAP:
        SW["dropped"]["labelled"] = SW["dropped"].get("labelled", 0) + 1
        return {}
    if not auto and asked is None and not sw_settled():
        SW["deferred"].append(("snap", label, _sw_now()))   # taken by the tick once the map has settled
        return {"label": label, "deferred": True, "failed": []}
    failed = []
    snap = dict(_sw_stamp(), label=label)
    if asked is not None:
        snap["asked_t"] = asked
    parts = [("map", lambda pc: _sw_p(pc.WorldInfo.GetStreamingPersistentMapName())),
             ("player", lambda pc: dict(_sw_read(pc.Pawn, SW_PLAYER_FIELDS), cls=pc.Pawn.Class.Name, cylinder=_sw_cylinder(pc.Pawn))),
             ("controller", lambda pc: _sw_read(pc, ("Rotation", "LastVisitedTeleporter", "FocusCamReturnTime", "CurrentSeenPickupable",
                                                    "CurrentTouchedPickupable", "bCinematicMode"))),
             ("pri", lambda pc: _sw_read(pc.PlayerReplicationInfo, ("ExpLevel", "GeneralSkillPoints"))),
             ("cash", lambda pc: int(pc.PlayerReplicationInfo.GetCurrencyOnHand(0))),
             ("checkpoint", lambda pc: _sw_p(pc.WorldInfo.GRI.ActiveRespawnCheckpointTeleportActor))]
    if not auto:
        parts += [("stations", lambda pc: [dict(_sw_fields(s, SW_STATION_FIELDS), name=s._path_name())
                                           for s in _sw_live(_sw_sdk.find_all("TravelStation", False))]),
                  ("pools", lambda pc: _sw_pools()),
                  ("engine", lambda pc: _sw_engine()),
                  ("mods", lambda pc: _sw_mods())]
        if SW["spec"]["pickups"]:
            parts.append(("pickups", lambda pc: _sw_pickups()))
    try:
        pc = _sw_pc()
    except Exception as e:
        pc = None
        _sw_err("snap " + label, e)
    for part, read in parts:
        if part in SW["off"]:
            failed.append(part)
            continue
        try:
            if pc is None:
                raise RuntimeError("no player controller")
            snap[part] = read(pc)
        except Exception as e:
            failed.append(part)
            sw_switch_off(part, e)
    snap["failed"] = failed
    snap["off"] = sorted(SW["off"])
    if auto:
        sw_emit("snap", snap, "snap_cap")
    else:
        SW["n"]["labelled"] += 1
        SW["buf"].append(dict(snap, k="snap"))
    return snap


def sw_pawn_wanted(row, match):
    """No filter: every pawn. Otherwise a pawn whose path, class or AI class holds one of the substrings."""
    return not match or any(m in str(row.get(k, "")) for m in match for k in ("name", "cls", "ai_class"))


def _sw_pawn_row(pawn):
    row = dict(name=pawn._path_name(), cls=pawn.Class.Name)
    try:
        row["ai_class"] = _sw_p(pawn.AIClass)
    except Exception:
        pass
    return row


def _sw_pawn_fill(row, pawn, here):
    row.update(_sw_read(pawn, SW_PAWN_FIELDS), cylinder=_sw_cylinder(pawn))
    if here is not None:
        try:
            loc = pawn.Location
            row["dist"] = round(((loc.X - here.X) ** 2 + (loc.Y - here.Y) ** 2 + (loc.Z - here.Z) ** 2) ** 0.5, 1)
        except Exception:
            pass
    return row


def sw_pawns(label, asked=None):
    """One read of the live AI pawns (sw_live_pawn): by exact path when the spec names paths (a lookup, no scan), else find_all of the
    exact class, filtered. Native getters are called on these only. Deferred until the map has settled; switched off after a failure."""
    auto = label == "auto"
    if auto and SW["n"]["pawns"] >= SW["spec"]["pawn_cap"]:
        SW["dropped"]["pawns"] = SW["dropped"].get("pawns", 0) + 1
        return {"label": label, "pawns": 0, "failed": []}
    if "pawns" in SW["off"]:
        return {"label": label, "pawns": 0, "failed": ["off"]}
    if not auto and asked is None and not sw_settled():
        SW["deferred"].append(("pawns", label, _sw_now()))
        return {"label": label, "deferred": True, "pawns": 0, "failed": []}
    out = dict(_sw_stamp(), label=label, pawns=[])
    if asked is not None:
        out["asked_t"] = asked
    failed = []
    try:
        player = _sw_pc().Pawn
        here = player.Location if player else None
        if SW["spec"]["pawns_paths"]:
            for path in SW["spec"]["pawns_paths"]:
                try:
                    pawn = _sw_sdk.find_object("WillowAIPawn", path)
                except Exception:
                    pawn = None
                if not sw_live_pawn(pawn):
                    failed.append(path)
                    continue
                out["pawns"].append(_sw_pawn_fill(_sw_pawn_row(pawn), pawn, here))
        else:
            for pawn in _sw_sdk.find_all("WillowAIPawn"):   # the exact class, as the tracked probes scan it
                if not sw_live_pawn(pawn):
                    continue
                row = _sw_pawn_row(pawn)
                if sw_pawn_wanted(row, SW["spec"]["pawns_match"]):
                    out["pawns"].append(_sw_pawn_fill(row, pawn, here))
    except Exception as e:
        failed.append("read")
        sw_switch_off("pawns", e)
    out["failed"] = failed
    if auto:
        sw_emit("pawns", out, "pawn_cap")
    elif SW["n"]["labelled"] < SW_LABELLED_CAP:
        SW["n"]["labelled"] += 1
        SW["buf"].append(dict(out, k="pawns"))
    return {"label": label, "pawns": len(out["pawns"]), "failed": failed}


def sw_counts():
    """Lines per stream, hook rows per function, drops and errors so far: the in-game usability check reads this."""
    return {"lines": dict(SW.get("n", {})), "bytes": SW.get("bytes", 0), "counts": dict(SW.get("counts", {})),
            "dropped": dict(SW.get("dropped", {})), "errors": len(SW.get("errors", [])), "auto_failures": SW.get("auto_failures", 0),
            "missing": list(SW.get("missing", [])), "file": str(SW.get("path")), "switched_off": dict(SW.get("switched_off", {})),
            "deferred": len(SW.get("deferred", []))}


def sw_root():
    """RG_OUT resolved, refused unless it lies inside a repository's ignored local/ folder (as loot_capture.lc_root)."""
    root = _SwPath(RG_OUT).resolve()  # noqa: F821 - the command channel's prelude sets RG_OUT
    if not any(a.name == "local" and (a.parent / ".git").exists() for a in root.parents):
        raise ValueError(f"RG_OUT {root} is not inside a repository's local/ folder")
    return root


def _sw_close(stopped):
    if SW.get("fh") is None:
        return
    SW["buf"].append({"k": "end", "stopped": stopped, "counts": SW.get("counts", {}), "dropped": SW.get("dropped", {}),
                      "errors": SW.get("errors", []), "auto_failures": SW.get("auto_failures", 0), "lines": SW.get("n", {}),
                      "switched_off": SW.get("switched_off", {}), "never_taken": [list(d) for d in SW.get("deferred", [])]})
    SW["spec"]["byte_cap"] += 1_000_000   # room for the end line, whatever the cap took
    try:
        sw_flush()
    finally:
        SW["fh"].close()
        SW["fh"] = None


def sw_dump(name="watch"):
    """The end of a recording: the queue written, the end line, the file closed. Returns the counts (the file is RG_OUT/<name>.jsonl)."""
    if SW.get("path") is not None and SW["path"].stem != name:
        raise ValueError(f"this recording writes {SW['path'].name}, not {name}.jsonl")
    SW["on"] = False
    _sw_close(stopped=False)
    return sw_counts()


def sw_stop():
    """Remove every hook this recorder added and close its file (with an end line saying it was stopped). Safe to call twice."""
    SW["on"] = False
    for path, kind, hid in SW.get("hooks", []):
        try:
            _sw_remove_hook(path, kind, hid)
        except Exception:
            pass
    SW["hooks"] = []
    try:
        _sw_close(stopped=True)
    except Exception:
        pass
    return "stopped"
