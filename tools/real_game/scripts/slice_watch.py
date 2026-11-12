"""Read-only recorder for the slice capture sessions, for the openwillow_realgame command channel (lane capture-prep, 2026-11-11).

Run through Invoke-GamePyFile (tools/real_game/realgame.ps1); tools/real_game/slice_watch.ps1 drives it from a capture script's
-WatchSpec switch. Our own code, no game data: the function names it hooks come from a spec file the runner passes (kept under the
ignored local/), and everything it records goes to RG_OUT (the ignored local/realgame/<set>/).

It only reads. Hooks are observers: they never return Block and never change an argument. Nothing here sets a field, calls a
setter, spawns, damages, moves, travels or sends input.

Safe by construction (after the 2026-11-11 session 2 crashes: an access violation in the same second as the watch's first pawn read,
in two captures; see local/dossier/watch-crash-analysis.md):
- No native function is called on any object a scan returns. Native calls are made only on the player's own objects, and only the
  ones the tracked probes make too (the list is above SW_PLAYER_GETTERS). Everything else, AI pawns included, is read by fields:
  _sw_fields never calls a callable. An AI pawn's health and shield come from the fields of the pools its references point at
  (HealthPool.Data, ShieldArmor.Data), as combat_probe._pool reads them.
- AI pawns are found as the tracked probes find them (combat_probe.cb_enemies, den_probe.dp_begin): find_all of the exact class
  WillowAIPawn, only objects named WillowAIPawn_<n> (not the preloaded Pawn_<Type> templates), not class defaults, not bDeleteMe, and
  only within pawns_radius of the player (default 4000, cb_enemies' radius; at most 6000, dp_begin's). The distance is recorded.
- No scan at all until the player has a pawn AND the map's own clock (WorldInfo.TimeSeconds, which restarts at every map load) has
  passed settle_seconds. Only the 'start' snapshot and the 'arrival' pawn read may be deferred until then (the line carries the time
  it was asked for); any other labelled read asked for before the map settled is NOT taken late: a line records it as not taken,
  because a 'before_down' taken after the immunity is gone, or a 'down' taken at the respawn, would carry the right label at the
  wrong moment.
- Observers read a hooked function's PARAMETERS only, never its locals: for an event the engine sends natively, a local can lie past
  the caller's parameter buffer, and reading an object-typed local would dereference whatever is there. A property counts as a
  parameter only when two independent conditions hold for the whole function: every one of its first NumParams properties carries
  the parameter flag (CPF_Parm, 0x80, the value tests/combat_test.py and tests/mission_script_test.py use) and the property after them,
  if any, does not; and every one of them lies wholly inside ParamsSize. If either fails, nothing of that function is read, the
  misfit counts as an error and the function is listed in the recording's meta line (params_misfit). An out parameter (CPF_Out,
  0x100) is recorded with the rest but named in the row's out_params_at_call: its value is the caller's copy at call time, not a
  result.
  ReturnValue is not read before the call; after it, the hook's own return value is recorded. Object parameters are recorded by path
  name only, and only when not None.
- Hooks can be installed in groups (spec "groups"): the capture calls sw_install_group at the point of its run where that group's
  functions can fire. Before a group is installed, a marker line naming it, its functions and the time is written and flushed, so a
  crash at the first call of a newly hooked function (the mod loader copies a call's parameters before any observer runs; nothing the
  watch reads can prevent that) points at a group. Per function the end line records when it was installed, when it first fired and
  how often: a hook that never fired was installed, not exercised.
- Each kind of read is wrapped: its first Python exception is counted and switches that kind off for the rest of the run. This guard
  does nothing against an access violation in native code, which ends the game before any Python exception exists; the rules above
  are the protection against that class, not this guard.

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
  frame_controller  bool: the player's own controller's bCinematicMode, IgnoreMoveInput and IgnoreLookInput in each frame row ("ctl"),
                fields only.
  pickups       bool: every live pickup in each LABELLED snapshot (item definition, LifeSpan, draw scale, location).
  snap_every    seconds between automatic snapshots (0 = none). Automatic snapshots are light: player, controller, level, cash,
                checkpoint; pools, stations, pickups and mods come only in labelled snapshots (the capture's own sw_snap calls).
  pawns_every   seconds between automatic pawn reads (0 = none).
  pawns_paths   full object paths: an automatic pawn read looks up only these (a name lookup, not a scan of every object); each must
                still be a live WillowAIPawn_<n>.
  settle_seconds  the map clock a scan waits for after every load (default 15).
  pawns_radius  the distance from the player within which AI pawns are read (default 4000, at most 6000).
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
    "frame_controller": (bool, False, None, None),
    "pickups": (bool, False, None, None), "snap_every": (float, 0.0, 0.5, 600.0), "pawns_every": (float, 0.0, 0.1, 600.0),
    "pawns_paths": (list, [], 0, 20), "pawns_match": (list, [], 0, 20),
    "per_func_cap": (int, 5000, 1, 20000), "row_cap": (int, 200000, 1, 400000), "frame_cap": (int, 300000, 1, 400000),
    "snap_cap": (int, 5000, 1, 20000), "pawn_cap": (int, 20000, 1, 50000), "byte_cap": (int, 300_000_000, 1_000_000, 1_000_000_000),
    "flush_every": (float, 2.0, 0.5, 30.0), "settle_seconds": (float, 15.0, 5.0, 120.0),
    "pawns_radius": (float, 4000.0, 500.0, 6000.0),
}
SW_DOC_KEYS = ("name", "notes", "checks", "groups")
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
    out = {k: (list(v[1]) if isinstance(v[1], list) else v[1]) for k, v in SW_SPEC_KEYS.items()}
    out["groups"] = []
    return out


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
    groups = spec.get("groups", [])
    if not isinstance(groups, list) or len(groups) > 12:
        raise ValueError("groups must be a list of at most 12 groups")
    names = set()
    for g in groups:
        if not isinstance(g, dict) or set(g) - {"name", "at", "funcs", "notes"} or not isinstance(g.get("name"), str) \
                or not _sw_re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_-]*", g["name"]) or g["name"] in names:
            raise ValueError("each group needs a unique plain name and only name, at, funcs and notes")
        names.add(g["name"])
        if not isinstance(g.get("at", ""), str) or not isinstance(g.get("funcs"), list) or not g["funcs"] \
                or not all(isinstance(f, str) and sw_parse_func(f) for f in g["funcs"]):
            raise ValueError(f"group {g['name']}: funcs must be a non-empty list of function names, at a string")
    if len(out["funcs"]) + sum(len(g["funcs"]) for g in groups) > 300:
        raise ValueError("at most 300 function names in all")
    out["groups"] = [{"name": g["name"], "at": g.get("at", ""), "funcs": list(g["funcs"])} for g in groups]
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
            return "<object>"
    if depth >= 2:
        return f"<{type(value).__name__}>"   # its type only: printing it would bypass the 16-item bound
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
    return f"<{type(value).__name__}>"   # an unknown kind: its type only, no text is built from it


SW_CPF_PARM = 0x80   # the parameter flag, as tests/combat_test.py and tests/mission_script_test.py define it
SW_CPF_OUT = 0x100   # an out parameter, as the same tests define it


def sw_param_layout(inner):
    """(names, None) for a function whose layout passes both conditions, or ([], why) when it does not. Never raises.
    Condition 1: each of the first NumParams properties carries the parameter flag, and the next property, if any, does not.
    Condition 2: each of the first NumParams properties lies wholly inside ParamsSize."""
    try:
        count, size = int(inner.NumParams), int(inner.ParamsSize)
        props = list(inner._properties())
    except Exception as e:
        return [], f"layout not readable: {repr(e)[:80]}"
    if count < 0 or count > len(props):
        return [], f"NumParams {count} does not fit {len(props)} properties"
    names, outs = [], []
    try:
        for q in props[:count]:
            if not int(q.PropertyFlags) & SW_CPF_PARM:
                return [], f"{q.Name} among the first NumParams lacks the parameter flag"
            if int(q.Offset_Internal) + int(q.ElementSize) * int(q.ArrayDim) > size:
                return [], f"{q.Name} lies outside ParamsSize"
            if q.Name != "ReturnValue":
                names.append(q.Name)
                if int(q.PropertyFlags) & SW_CPF_OUT:
                    outs.append(q.Name)
        if count < len(props) and int(props[count].PropertyFlags) & SW_CPF_PARM:
            return [], f"{props[count].Name} after the first NumParams carries the parameter flag"
    except Exception as e:
        return [], f"flags or offsets not readable: {repr(e)[:80]}"
    SW.setdefault("param_outs", {})[id(inner)] = outs
    return names, None


def sw_param_names(func, path=None):
    """The names of a function's parameters, never its locals (sw_param_layout). Worked out once per function, at hook time when the
    watch starts, else at the first call; a misfit is an error, kept in SW['params_misfit']. ReturnValue is left out (the hook passes
    the return value after the call)."""
    inner = getattr(func, "func", func)
    if path is None:
        try:
            path = inner._path_name()
        except Exception:
            path = None
    cache = SW.setdefault("params", {})
    if path is not None and path in cache:
        return cache[path]
    names, why = sw_param_layout(inner)
    outs = SW.get("param_outs", {}).pop(id(inner), [])
    if path is not None and outs:
        SW.setdefault("outs", {})[path] = outs
    if why is not None:
        SW.setdefault("params_misfit", {})[str(path)] = why
        if "errors" in SW:
            _sw_err("params " + str(path), RuntimeError(why))
    if path is not None:
        cache[path] = names
    return names


def _sw_args(args, func, path=None):
    """The hooked call's parameters (sw_param_names), by value; an object parameter by path name only, None when it is None."""
    out = {}
    try:
        for name in sw_param_names(func, path):
            out[name] = _sw_p(getattr(args, name))
    except Exception as e:
        out["args_error"] = repr(e)[:100]
        seen = SW.setdefault("args_error_seen", set())
        if path not in seen:   # counted once per function, so a repeating one cannot flood the error list
            seen.add(path)
            _sw_err("args " + str(path), e)
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
    row = dict(_sw_stamp(), func=path, tag=tag, obj=_sw_p(obj), args=_sw_args(args, func, path))
    outs = SW.get("outs", {}).get(path) if tag == "pre" else None
    if outs:
        row["out_params_at_call"] = outs   # copies of the caller's values at call time, not results
    if tag == "post":
        row["ret"] = _sw_p(ret)
    return row if sw_emit("row", row, "row_cap") else None


def _sw_fired(path, tag):
    """Installed, first fired, times fired: for each hooked function (PRE calls are counted)."""
    if tag != "pre":
        return
    rec = SW.setdefault("fn", {}).setdefault(path, {"group": None, "installed_t": None, "first_t": None, "n": 0})
    rec["n"] += 1
    if rec["first_t"] is None:
        rec["first_t"] = _sw_now()
        # On disk before the arguments are read (one flush per hooked function): if the loader or the read then faults, the recording
        # still says this function had started to fire.
        SW["buf"].append(dict(_sw_stamp(), k="fired", func=path, group=rec["group"]))
        sw_flush()


def _sw_observer(path, tag):
    def hook(obj, args, ret, func):
        if not SW.get("on"):
            return None
        try:
            _sw_fired(path, tag)
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
        if SW["spec"]["frame_controller"] and "frame_controller" not in SW["off"]:
            row.update(_sw_controller_here())
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


# The player's own controller's state in each frame row (spec frame_controller; session 1 analysis, A2): its cinematic flag and its move and
# look input-ignore counts, as plain fields (no IsMoveInputIgnored or IsLookInputIgnored call is needed).
SW_CONTROLLER_FIELDS = ("bCinematicMode", "IgnoreMoveInput", "IgnoreLookInput")


def _sw_controller_here():
    """[bCinematicMode, IgnoreMoveInput, IgnoreLookInput] of the player's own controller, by fields only; a field this build lacks reads
    None. A failure switches this read off."""
    try:
        fields = _sw_fields(_sw_pc(), SW_CONTROLLER_FIELDS)
        return {"ctl": [fields.get(name) for name in SW_CONTROLLER_FIELDS]}
    except Exception as e:
        sw_switch_off("frame_controller", e)
        return {"ctl": None}


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


def _sw_install(fnames, group):
    """Resolve and hook each name (PRE and POST observers), work out each function's parameter layout, and note when it was
    installed. Returns (resolved paths, missing names)."""
    resolved, missing = [], []
    for fname in fnames:
        kind, what = sw_parse_func(fname)
        try:
            paths = ([what] if _sw_exists(what) else []) if kind == "path" else _sw_declared(*what)
        except Exception as e:
            paths = []
            _sw_err(fname, e)
        if not paths:
            missing.append(fname)
            continue
        for path in paths:
            try:
                fobj = _sw_sdk.find_object("Function", path)
            except Exception:
                fobj = None
            if fobj is not None:
                sw_param_names(fobj, path)
            try:
                _sw_hook(path, _SwType.PRE, "pre", _sw_observer(path, "pre"))
                _sw_hook(path, _SwType.POST, "post", _sw_observer(path, "post"))
                resolved.append(path)
                rec = SW.setdefault("fn", {}).setdefault(path, {"group": None, "installed_t": None, "first_t": None, "n": 0})
                rec["group"], rec["installed_t"] = group, _sw_now()
            except Exception as e:
                missing.append(f"{path} ({repr(e)[:80]})")
    SW["resolved"].extend(resolved)
    SW["missing"].extend(missing)
    return resolved, missing


def sw_install_group(name):
    """Install one of the spec's groups, once. A marker line (the group, its functions, the time) is written and FLUSHED before the
    first hook goes in, and a second line after, so a crash during the install or at a first call points at this group."""
    group = next((g for g in SW["spec"].get("groups", []) if g["name"] == name), None)
    if group is None:
        raise ValueError(f"no group {name!r} in the spec")
    done = SW.setdefault("groups_installed", [])
    if name in done:
        return {"group": name, "already": True}
    SW["buf"].append(dict(_sw_stamp(), k="group", name=name, at=group["at"], funcs=group["funcs"], event="installing"))
    sw_flush()
    resolved, missing = _sw_install(group["funcs"], name)
    done.append(name)
    SW["buf"].append(dict(_sw_stamp(), k="group", name=name, event="installed", resolved=resolved, missing=missing,
                          params_misfit=[p for p in resolved if p in SW.get("params_misfit", {})]))
    sw_flush()
    return {"group": name, "resolved": len(resolved), "missing": missing}


def sw_group_names_at(at):
    """The names of the spec's groups for this point of the capture's run, in spec order (the capture installs them one at a time)."""
    return [g["name"] for g in SW["spec"].get("groups", []) if g["at"] == at]


def sw_install_groups_at(at):
    """Every group of the spec whose 'at' is this point of the capture's run, in spec order."""
    return [sw_install_group(g["name"]) for g in SW["spec"].get("groups", []) if g["at"] == at]


def sw_mark(label):
    """A flushed marker line: a capture's stage boundary, so a crash after it is attributed to that stage."""
    if not isinstance(label, str) or not _sw_re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.:-]*", label):
        raise ValueError(f"mark label must be plain, got {label!r}")
    SW["buf"].append(dict(_sw_stamp(), k="mark", label=label))
    sw_flush()
    return label


def sw_start(label, spec, name="watch"):
    """Check the spec, open RG_OUT/<name>.jsonl, hook every function the spec names, start the tick. Returns what resolved and what not."""
    if not isinstance(name, str) or not _sw_re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_-]*", name):
        raise ValueError(f"output name must be a plain file name, got {name!r}")
    checked = sw_load_spec(spec)
    sw_stop()
    sw_reset(label, checked)
    _sw_open(name)
    SW["buf"].append(dict(_sw_stamp(), k="group", name="base", at="start", funcs=SW["spec"]["funcs"], event="installing"))
    sw_flush()
    _sw_hook(SW_TICK, _SwType.POST_UNCONDITIONAL, "tick", _sw_tick)
    base_resolved, base_missing = _sw_install(SW["spec"]["funcs"], "base")
    SW["buf"].append(dict(_sw_stamp(), k="group", name="base", event="installed", resolved=base_resolved, missing=base_missing))
    SW["buf"].append({"k": "meta", "label": label, "t0_qpc": SW["t0"], "spec": SW["spec"], "resolved": SW["resolved"],
                      "missing": SW["missing"], "params_misfit": dict(SW.get("params_misfit", {}))})
    SW["on"] = True
    first = sw_snap("start")
    sw_flush()
    return {"label": label, "file": str(SW["path"]), "resolved": len(SW["resolved"]), "missing": SW["missing"],
            "start_failed": first.get("failed", [])}


# ---------------------------------------------------------------- reads
# Every native call the watch can make, each on the player's own objects only, and each made by the tracked probes too:
#   the player's pawn (pc.Pawn):         GetHealth, GetMaxHealth, GetShieldStrength, GetMaxShieldStrength
#                                        (den_probe dp_begin/dp_end, cash_probe cp_up, combat_probe _pawn_state);
#   the player's world (pc.WorldInfo):   GetStreamingPersistentMapName (every capture's map check);
#   the player's replication info:       GetCurrencyOnHand(0) (cash_probe cp_cash).
# Each runs inside a wrapped snapshot part, only in a snapshot (never per frame), and only when the player has a pawn. Nothing else is
# called: every other read below goes through _sw_fields, which skips anything callable.
SW_PLAYER_GETTERS = ("GetHealth", "GetMaxHealth", "GetShieldStrength", "GetMaxShieldStrength")
SW_PLAYER_FIELDS = ("Location", "Rotation", "Velocity", "Physics", "GroundSpeed", "DrawScale", "Health", "HealthMax",
                    "NumSuccessiveInjuries", "bHidden")
SW_PAWN_FIELDS = ("Location", "GroundSpeed", "WalkingPct", "DrawScale", "DrawScale3D", "Health", "HealthMax", "Physics")
SW_POOL_STATE = ("CurrentValue", "MaxValue", "MinValue", "MaxValueBaseValue")
SW_STATION_FIELDS = ("bShouldBeActive", "bIsCurrentlyActive", "Location")
SW_POOL_FIELDS = ("Definition", "AssociatedProvider", "CurrentValue", "MaxValue", "MaxValueBaseValue")
SW_DEFERRABLE = ("start", "arrival")   # the only labelled reads that may wait for the map to settle


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


def _sw_player_getters(pawn):
    """The four health and shield getters, on the player's own pawn only (see the list above)."""
    out = {}
    for name in SW_PLAYER_GETTERS:
        try:
            out[name] = _sw_p(getattr(pawn, name)())
        except Exception:
            pass
    return out


def sw_pool_check(manager_ok, ref_index, slot_read, slot_path, ref_guid, slot_guid, slot_def, values_ok):
    """Why a pool reference's values cannot be trusted, or None. Fails closed: every condition must be shown, an unreadable one is a
    reason. The pool is resolved through its manager's slot (the reference's cached Data is only a cache: NATIVE_COMBAT_GROUNDWORK
    section 1): 'no manager'; 'index invalid' (not 0-15); 'slot not read'; 'slot empty'; 'guid unreadable' (either GUID not a number);
    'guid differs'; 'shut down' (no Definition: a deleted pool keeps its object with zeroed values); 'values unreadable'."""
    if not manager_ok:
        return "no manager"
    if not isinstance(ref_index, int) or isinstance(ref_index, bool) or not 0 <= ref_index < 16:
        return "index invalid"
    if not slot_read:
        return "slot not read"
    if slot_path is None:
        return "slot empty"
    if not isinstance(ref_guid, int) or not isinstance(slot_guid, int) or isinstance(ref_guid, bool) or isinstance(slot_guid, bool):
        return "guid unreadable"
    if ref_guid != slot_guid:
        return "guid differs"
    if slot_def is None:
        return "shut down"
    if not values_ok:
        return "values unreadable"
    return None


def _sw_pool_of(ref):
    """The pool a pawn's pool reference resolves to, by fields only, through its manager's slot: the reference's PoolManager,
    PoolIndexInManager and PoolGUID; the slot's pool (manager.ResourcePools[index]) with its PoolGUID, Definition, AssociatedProvider
    and values; the cached Data's path beside it (cache_matches). 'stale' says why the values cannot be trusted (sw_pool_check), or is
    None; a record with stale set is never read as a healthy pool."""
    out = {}
    ref_fields = _sw_fields(ref, ("PoolIndexInManager", "PoolGUID"))
    out["ref_index"], out["ref_guid"] = ref_fields.get("PoolIndexInManager"), ref_fields.get("PoolGUID")
    try:
        manager = ref.PoolManager
        manager = None if callable(manager) else manager
    except Exception:
        manager = None
    out["manager"] = _sw_p(manager)
    try:
        cache = ref.Data
        cache = None if callable(cache) else cache
    except Exception:
        cache = None
    out["cache"] = _sw_p(cache) if cache is not None else None
    slot, slot_read = None, False
    index = out["ref_index"]
    if manager is not None and isinstance(index, int) and not isinstance(index, bool) and 0 <= index < 16:
        try:
            slot = manager.ResourcePools[index]
            slot = None if callable(slot) else slot
            slot_read = True
        except Exception:
            slot_read = False
    out["slot"] = _sw_p(slot) if slot is not None else None
    if slot is not None:
        out.update(_sw_fields(slot, SW_POOL_STATE + ("PoolGUID", "Definition", "AssociatedProvider")))
    out["cache_matches"] = out["cache"] is None or out["cache"] == out["slot"]
    values_ok = all(isinstance(out.get(k), (int, float)) and not isinstance(out.get(k), bool) for k in ("CurrentValue", "MaxValue"))
    out["stale"] = sw_pool_check(manager is not None, index, slot_read, out["slot"], out["ref_guid"], out.get("PoolGUID"),
                                 out.get("Definition"), values_ok)
    return out


def _sw_pool_refs(pawn):
    """Both pool references of a pawn. A pawn without a readable reference gets None (the usability check treats that as missing)."""
    out = {}
    for key, name in (("health_pool", "HealthPool"), ("shield_pool", "ShieldArmor")):
        try:
            ref = getattr(pawn, name)
            out[key] = None if ref is None or callable(ref) else _sw_pool_of(ref)
        except Exception:
            out[key] = None
    return out


def _sw_live(objs):
    """Scanned objects that are not class defaults and not being destroyed. Their fields may be read; nothing is called on them."""
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
    """A live AI pawn: named WillowAIPawn_<n> (not a preloaded Pawn_<Type> template), not a class default, not bDeleteMe
    (as combat_probe.cb_enemies and den_probe.dp_begin). Read by fields only, like every scanned object."""
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
        try:   # little or big money, or another item's balance: a field of the pickup's inventory (session 2 analysis, A3)
            row["balance"] = _sw_p(pk.Inventory.DefinitionData.BalanceDefinition)
        except Exception:
            row["balance"] = None
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


def _sw_not_taken(kind, label):
    """A labelled read asked for before the map settled, other than start and arrival: recorded as not taken, never taken late."""
    line = dict(_sw_stamp(), label=label, not_taken="asked before the map settled", asked_t=_sw_now(), failed=[])
    if SW["n"]["labelled"] < SW_LABELLED_CAP:
        SW["n"]["labelled"] += 1
        SW["buf"].append(dict(line, k=kind))
    else:   # past the cap the line is dropped, and counted, as sw_snap counts its own (review of #245)
        SW["dropped"]["labelled"] = SW["dropped"].get("labelled", 0) + 1
    return {"label": label, "not_taken": True, "failed": [], "pawns": 0}


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
        if label not in SW_DEFERRABLE:
            return _sw_not_taken("snap", label)
        SW["deferred"].append(("snap", label, _sw_now()))   # taken by the tick once the map has settled
        return {"label": label, "deferred": True, "failed": []}
    failed = []
    snap = dict(_sw_stamp(), label=label)
    if asked is not None:
        snap["asked_t"] = asked
    parts = [("map", lambda pc: _sw_p(pc.WorldInfo.GetStreamingPersistentMapName())),
             ("player", lambda pc: dict(_sw_fields(pc.Pawn, SW_PLAYER_FIELDS), **_sw_player_getters(pc.Pawn), **_sw_pool_refs(pc.Pawn),
                                        cls=pc.Pawn.Class.Name, cylinder=_sw_cylinder(pc.Pawn))),
             ("controller", lambda pc: _sw_fields(pc, ("Rotation", "LastVisitedTeleporter", "FocusCamReturnTime", "CurrentSeenPickupable",
                                                      "CurrentTouchedPickupable", "bCinematicMode"))),
             ("pri", lambda pc: _sw_fields(pc.PlayerReplicationInfo, ("ExpLevel", "GeneralSkillPoints"))),
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
            if pc is None or pc.Pawn is None:
                raise RuntimeError("no player controller or pawn")
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
        ai = pawn.AIClass
        row["ai_class"] = None if callable(ai) else _sw_p(ai)
    except Exception:
        pass
    return row


def _sw_dist(pawn, here):
    """Distance from the player, from the Location fields; None when either cannot be read."""
    try:
        loc = pawn.Location
        return round(((loc.X - here.X) ** 2 + (loc.Y - here.Y) ** 2 + (loc.Z - here.Z) ** 2) ** 0.5, 1)
    except Exception:
        return None


def _sw_pawn_fill(row, pawn):
    row.update(_sw_fields(pawn, SW_PAWN_FIELDS), cylinder=_sw_cylinder(pawn))
    row.update(_sw_pool_refs(pawn))
    return row


def sw_pawns(label, asked=None):
    """One read of the live AI pawns within pawns_radius (sw_live_pawn), by fields only: by exact path when the spec names paths (a
    lookup, no scan), else find_all of the exact class. Deferred until the map has settled ('arrival' only; any other label is recorded
    as not taken); switched off after a Python failure."""
    auto = label == "auto"
    if auto and SW["n"]["pawns"] >= SW["spec"]["pawn_cap"]:
        SW["dropped"]["pawns"] = SW["dropped"].get("pawns", 0) + 1
        return {"label": label, "pawns": 0, "failed": []}
    if "pawns" in SW["off"]:
        return {"label": label, "pawns": 0, "failed": ["off"]}
    if not auto and asked is None and not sw_settled():
        if label not in SW_DEFERRABLE:
            return _sw_not_taken("pawns", label)
        SW["deferred"].append(("pawns", label, _sw_now()))
        return {"label": label, "deferred": True, "pawns": 0, "failed": []}
    out = dict(_sw_stamp(), label=label, pawns=[], beyond_radius=0)
    if asked is not None:
        out["asked_t"] = asked
    if not auto:
        try:
            out["map"] = _sw_p(_sw_pc().WorldInfo.GetStreamingPersistentMapName())   # one of the listed calls on the player's own world
        except Exception:
            out["map"] = None
    failed = []
    radius = SW["spec"]["pawns_radius"]
    try:
        player = _sw_pc().Pawn
        here = player.Location if player is not None else None
        if here is None:
            raise RuntimeError("no player pawn")
        if SW["spec"]["pawns_paths"]:
            candidates = []
            for path in SW["spec"]["pawns_paths"]:
                try:
                    pawn = _sw_sdk.find_object("WillowAIPawn", path)
                except Exception:
                    pawn = None
                if not sw_live_pawn(pawn):
                    failed.append(path)
                    continue
                candidates.append(pawn)
        else:
            candidates = [p for p in _sw_sdk.find_all("WillowAIPawn") if sw_live_pawn(p)]   # the exact class, as the probes scan it
        for pawn in candidates:
            dist = _sw_dist(pawn, here)
            if dist is None or dist > radius:
                out["beyond_radius"] += 1
                continue
            row = _sw_pawn_row(pawn)
            if sw_pawn_wanted(row, SW["spec"]["pawns_match"]):
                row["dist"] = dist
                out["pawns"].append(_sw_pawn_fill(row, pawn))
    except Exception as e:
        failed.append("read")
        sw_switch_off("pawns", e)
    out["failed"] = failed
    if auto:
        sw_emit("pawns", out, "pawn_cap")
    elif SW["n"]["labelled"] < SW_LABELLED_CAP:
        SW["n"]["labelled"] += 1
        SW["buf"].append(dict(out, k="pawns"))
    else:   # a labelled read past the cap: dropped and counted (review of #245)
        SW["dropped"]["labelled"] = SW["dropped"].get("labelled", 0) + 1
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
                      "switched_off": SW.get("switched_off", {}), "never_taken": [list(d) for d in SW.get("deferred", [])],
                      "params_misfit": dict(SW.get("params_misfit", {})), "fn": dict(SW.get("fn", {})), "missing": list(SW.get("missing", [])),
                      "groups_installed": list(SW.get("groups_installed", []))})
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
