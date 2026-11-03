"""Menus ground-truth probe for the openwillow_realgame command channel (Menus lane rg, 2026-11-02).

Our own code, no game data. Load it with Invoke-GamePyFile (tools/real_game/realgame.ps1); everything it writes goes to
RG_OUT (the ignored local/realgame/menus/). Memory-only: nothing here saves settings or the game.

- mp_mods_off(names): disable third-party SDK mods for this session only (mods_base `disable(dont_update_setting=True)`),
  so menus show stock behaviour (Better UI Controls remaps menu keys, Python Part Notifier adds card text).
- mp_trace_start(name) / mp_trace_mark(label) / mp_trace_stop(): a UI trace like tools/sdk_trace/openwillow_uitrace (every
  function declared on GFxMoviePlayer/GFxObject subclasses, PRE with arguments, POST with the return value), with a larger
  per-function budget and in-trace marks so key presses can be lined up with the calls.
- mp_vars(paths): live GetVariableString/GetVariableNumber reads on the open status menu movie.
- mp_mission(): mission status and GetMissionDescriptionForUI text for the Fire mission (and every mission the tracker lists).
- mp_inv_*: the bounded-recipe inventory set-up, the sorted backpack list as the panel holds it, and per-item tie-break fields.
"""
import json
import re
import time
from collections import Counter
from pathlib import Path

import unrealsdk
from unrealsdk.hooks import Type, add_hook, remove_hook
from unrealsdk.unreal import UObject, WrappedArray, WrappedStruct

MISSION = "GD_Z1_RockPaperGenocide.M_RockPaperGenocide_Fire"
MP = globals().get("MP") or {"file": None, "seq": 0, "t0": 0.0, "hooked": [], "calls": Counter(), "last": {}, "spawned": []}
MP_MAX = 600


def _p(value, depth=0):
    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    if depth >= 3:
        return repr(value)[:200]
    if isinstance(value, UObject):
        return f"{value.Class.Name}'{value._path_name()}'"
    if isinstance(value, WrappedStruct):
        out = {}
        for q in value._type._properties():
            try:
                out[q.Name] = _p(getattr(value, q.Name), depth + 1)
            except Exception:
                pass
        return out
    if isinstance(value, WrappedArray):
        return [_p(v, depth + 1) for v in list(value)[:96]]
    return repr(value)[:200]


def _root():
    """RG_OUT resolved, refused unless it lies inside a repository's ignored local/ (an ancestor named `local` whose parent
    holds `.git`: a folder in the main checkout, a file in a worktree). What this probe writes is game data; realgame.ps1 builds
    RG_OUT from its -OutFolder argument, so a folder like '../../docs' would otherwise send it into tracked files (the same
    guard as scripts/minimap.py)."""
    root = Path(RG_OUT).resolve()
    if not any(a.name == "local" and (a.parent / ".git").exists() for a in root.parents):
        raise ValueError(f"RG_OUT {root} is not inside a repository's local/ folder")
    return root


def _out(name):
    """RG_OUT/<name>: each '/'-separated part of `name` a plain file name (no '..', no drive or absolute path, no backslash),
    and the resolved path still under RG_OUT; otherwise ValueError. Creates the parent folder."""
    parts = name.split("/") if isinstance(name, str) else []
    if not parts or any(not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]*", part) or ".." in part for part in parts):
        raise ValueError(f"output name must be plain file names separated by '/', got {name!r}")
    root = _root()
    path = root.joinpath(*parts).resolve()
    if not path.is_relative_to(root) or path == root:
        raise ValueError(f"{path} is not under {root}")
    path.parent.mkdir(parents=True, exist_ok=True)
    return path


def mp_write_json(name, data):
    path = _out(name)
    with open(path, "w", encoding="utf-8") as handle:
        json.dump(data, handle, indent=1, ensure_ascii=False)
    return str(path)


# ---------------------------------------------------------------- mods
def mp_mods(names=None):
    import mods_base
    rows = []
    for m in mods_base.get_ordered_mod_list():
        if names is None or m.name in names:
            rows.append({"name": m.name, "enabled": bool(getattr(m, "is_enabled", False))})
    return rows


def mp_mods_off(names):
    import mods_base
    done = []
    for m in mods_base.get_ordered_mod_list():
        if m.name in names and getattr(m, "is_enabled", False):
            try:
                m.disable(dont_update_setting=True)
                done.append(m.name)
            except Exception as e:
                done.append(f"{m.name}: {repr(e)[:100]}")
    return done


# ---------------------------------------------------------------- UI trace
def _write(record):
    if MP["file"] is None:
        return
    MP["seq"] += 1
    record = {"seq": MP["seq"], "t": round(time.perf_counter() - MP["t0"], 4), **record}
    MP["file"].write(json.dumps(record, ensure_ascii=False) + "\n")


def _budget_key(path, args):
    try:
        names = {p.Name for p in args._type._properties()}
        for name in ("Member", "Method", "Path", "VarPath"):
            if name in names:
                member = getattr(args, name)
                if isinstance(member, str) and member:
                    return f"{path}#{member}"
    except Exception:
        pass
    return path


def _on_call(obj, args, _ret, func):
    try:
        path = func.func._path_name()
        key = _budget_key(path, args)
        MP["calls"][key] += 1
        MP["last"][path] = MP["calls"][key] <= MP_MAX
        if MP["last"][path]:
            _write({"phase": "call", "func": path, "obj": _p(obj),
                    "args": {p.Name: _p(getattr(args, p.Name)) for p in args._type._properties() if p.Name != "ReturnValue"}})
    except Exception as error:
        _write({"phase": "error", "error": repr(error)[:300]})


def _on_return(obj, args, ret, func):
    try:
        path = func.func._path_name()
        if MP["last"].get(path, True):
            _write({"phase": "return", "func": path, "obj": _p(obj), "ret": _p(ret)})
    except Exception as error:
        _write({"phase": "error", "error": repr(error)[:300]})


def _ui_functions():
    bases = [unrealsdk.find_class(n) for n in ("GFxMoviePlayer", "GFxObject")]
    paths = set()
    for cls in unrealsdk.find_all("Class", exact=True):
        if not any(cls._inherits(b) for b in bases):
            continue
        for field in cls._fields():
            if field.Class.Name == "Function" and field.Outer == cls:
                paths.add(field._path_name())
    return sorted(paths)


def mp_trace_start(name):
    path = _out("traces/" + name + ".jsonl") if isinstance(name, str) else _out(name)   # refuses a bad name before any hook
    mp_trace_stop()
    MP.update(file=open(path, "w", encoding="utf-8"), seq=0, t0=time.perf_counter(),
              calls=Counter(), last={})
    funcs = _ui_functions()
    for path in funcs:
        add_hook(path, Type.PRE, "ow_mp_trace", _on_call)
        add_hook(path, Type.POST, "ow_mp_trace", _on_return)
    MP["hooked"] = funcs
    _write({"phase": "start", "hooked": len(funcs), "qpc": time.perf_counter()})
    return len(funcs)


def mp_trace_mark(label, **data):
    _write({"phase": "mark", "label": label, "qpc": time.perf_counter(), **data})
    if MP["file"] is not None:
        MP["file"].flush()
    return MP["seq"]


def mp_trace_stop():
    for path in MP["hooked"]:
        try:
            remove_hook(path, Type.PRE, "ow_mp_trace")
            remove_hook(path, Type.POST, "ow_mp_trace")
        except Exception:
            pass
    if MP["file"] is not None:
        _write({"phase": "stop", "counts": dict(MP["calls"].most_common())})
        MP["file"].close()
    MP.update(file=None, hooked=[])
    return True


# ---------------------------------------------------------------- live reads
def mp_movie(cls="StatusMenuExGFxMovie"):
    found = [m for m in unrealsdk.find_all(cls, exact=False) if "Default__" not in m.Name]
    return found[-1] if found else None


def mp_vars(paths, cls="StatusMenuExGFxMovie"):
    movie = mp_movie(cls)
    out = {"movie": _p(movie)}
    if movie is None:
        return out
    for path in paths:
        try:
            if path.endswith("._currentframe") or path.endswith("._visible") or path.endswith("._x") or path.endswith("._y"):
                out[path] = movie.GetVariableNumber(path)
            else:
                out[path] = movie.GetVariableString(path)
        except Exception as e:
            out[path] = "ERR " + repr(e)[:100]
    return out


def mp_state():
    pc = get_pc()
    pri = pc.PlayerReplicationInfo
    out = {"map": pc.WorldInfo.GetStreamingPersistentMapName()}
    for key, fn in (("level", lambda: pri.ExpLevel), ("exp", lambda: pc.GetExpPoints()),
                    ("credits", lambda: pri.GetCurrencyOnHand(0)), ("eridium", lambda: pri.GetCurrencyOnHand(1)),
                    ("playthrough", lambda: pc.GetCurrentPlaythrough()),
                    ("pos", lambda: [round(pc.Pawn.Location.X), round(pc.Pawn.Location.Y), round(pc.Pawn.Location.Z)]),
                    ("hp", lambda: pc.Pawn.GetHealth())):
        try:
            out[key] = fn()
        except Exception as e:
            out[key] = "ERR " + repr(e)[:80]
    return out


def mp_mission(all_missions=True):
    pc = get_pc()
    tracker = pc.WorldInfo.GRI.MissionTracker
    out = {"state": mp_state()}
    try:
        out["active"] = _p(tracker.GetActiveMission())
    except Exception as e:
        out["active"] = "ERR " + repr(e)[:80]
    rows = []
    targets = []
    if all_missions:
        try:
            for md in tracker.MissionList:
                targets.append(md.MissionDef)
        except Exception as e:
            out["list_error"] = repr(e)[:120]
    fire = unrealsdk.find_object("MissionDefinition", MISSION)
    if fire not in targets:
        targets.append(fire)
    for m in targets:
        if m is None:
            continue
        row = {"mission": m._path_name()}
        try:
            row["name"] = m.MissionName
        except Exception:
            pass
        try:
            row["status"] = int(tracker.GetMissionStatus(m))
        except Exception as e:
            row["status"] = "ERR " + repr(e)[:80]
        if row.get("status") not in (0,):
            for flags in ((True, False, False), (True, False, True), (False, False, False)):
                try:
                    row["desc_" + "".join("1" if f else "0" for f in flags)] = pc.GetMissionDescriptionForUI(m, *flags)
                except Exception as e:
                    row["desc_err"] = repr(e)[:120]
        rows.append(row)
    out["missions"] = rows
    return out


# ---------------------------------------------------------------- inventory
def _obj_path(o):
    return o._path_name() if o is not None else None


def _weapons():
    im = get_pc().GetPawnInventoryManager()
    chain, item = [], im.InventoryChain
    while item is not None:
        chain.append(item)
        item = item.Inventory
    return chain, list(im.Backpack)


def mp_item(it):
    row = {"path": _obj_path(it), "class": it.Class.Name}
    for key, fn in (("name", lambda: it.GetShortHumanReadableName()), ("address", lambda: hex(it._get_address())),
                    ("manufacturer", lambda: _p(it.GetManufacturer())), ("grade", lambda: it.GetManufacturerGradeIndex()),
                    ("rarity", lambda: it.RarityLevel), ("explevel", lambda: it.ExpLevel), ("value", lambda: it.MonetaryValue),
                    ("category_key", lambda: it.GetCategoryKey()), ("zippy", lambda: it.GetZippyFrame()),
                    ("readied", lambda: bool(it.bReadied)), ("slot", lambda: it.QuickSelectSlot),
                    ("balance", lambda: _p(it.DefinitionData.BalanceDefinition)),
                    ("type", lambda: _p(it.DefinitionData.WeaponTypeDefinition)),
                    ("unique_id", lambda: it.DefinitionData.UniqueId), ("mark", lambda: it.Mark),
                    ("inv_index", lambda: it.InventoryIndex if hasattr(it, "InventoryIndex") else None)):
        try:
            row[key] = fn()
        except Exception as e:
            row[key] = "ERR " + repr(e)[:60]
    return row


def mp_inv_state():
    chain, backpack = _weapons()
    im = get_pc().GetPawnInventoryManager()
    out = {"state": mp_state(), "chain": [mp_item(i) for i in chain], "backpack": [mp_item(i) for i in backpack]}
    for key, fn in (("ready_max", lambda: im.GetWeaponReadyMax()), ("backpack_max", lambda: im.InventorySlotMax_Misc)):
        try:
            out[key] = fn()
        except Exception as e:
            out[key] = "ERR " + repr(e)[:60]
    return out


def mp_inv_clear():
    """Remove every backpack item and every readied item except the weapon in hand (memory only)."""
    pc = get_pc()
    im = pc.GetPawnInventoryManager()
    held = pc.Pawn.Weapon
    removed = []
    for it in list(im.Backpack):
        removed.append(it.GetShortHumanReadableName())
        im.RemoveInventoryFromBackpack(it)
        it.Destroy()
    chain, _ = _weapons()
    for it in chain:
        if it == held:
            continue
        try:
            removed.append(it.GetShortHumanReadableName())
            im.RemoveFromInventory(it)
            it.Destroy()
        except Exception as e:
            removed.append("ERR " + repr(e)[:80])
    return removed


def mp_inv_spawn(defs_path, order=None):
    """Spawn the bounded recipes (defs.json from the d20 session) by full definition, in `order` (list of file names)."""
    im = get_pc().GetPawnInventoryManager()
    defs = json.load(open(defs_path, encoding="utf-8"))
    if order:
        defs = sorted(defs, key=lambda d: order.index(d["file"]))
    made = []
    for d in defs:
        before = {_obj_path(w) for group in _weapons() for w in group}
        values = {}
        for key, value in d["fields"].items():
            values[key] = unrealsdk.find_object("Object", value) if isinstance(value, str) and "." in value else value
        im.AddBackpackWeaponFromDefinitionData(unrealsdk.make_struct("WeaponDefinitionData", **values))
        new = [w for group in _weapons() for w in group if _obj_path(w) not in before]
        MP["spawned"].extend(_obj_path(w) for w in new)
        made.append({"file": d["file"], "new": [mp_item(w) for w in new]})
    return made


def mp_set_level(level):
    pc = get_pc()
    pri = pc.PlayerReplicationInfo
    before = pri.ExpLevel
    pri.ExpLevel = level
    try:
        pc.Pawn.SetExpLevel(level) if hasattr(pc.Pawn, "SetExpLevel") else None
    except Exception:
        pass
    return [before, pri.ExpLevel]


def mp_panel():
    """The backpack panel's rows in display order, read through GetThingByIndex, plus the panel's properties."""
    movie = mp_movie()
    out = {"movie": _p(movie)}
    if movie is None:
        return out
    try:
        inv = movie.InventoryPanel
        bp = inv.BackpackPanel
        out["equipped_view"] = bool(inv.bInEquippedView)
        rows = []
        for i in range(0, 60):
            try:
                thing = bp.GetThingByIndex(i)
            except Exception as e:
                rows.append({"i": i, "err": repr(e)[:80]})
                break
            rows.append({"i": i, "thing": mp_item(thing) if thing is not None else None})
        out["rows"] = rows
        props = {}
        for q in bp.Class._properties():
            try:
                props[q.Name] = _p(getattr(bp, q.Name))
            except Exception:
                pass
        out["backpack_panel_props"] = props
        try:
            out["sort_index"] = get_pc().BackpackSortPreferenceIndex
        except Exception:
            pass
    except Exception as e:
        out["error"] = repr(e)[:200]
    return out


def mp_remove_spawned():
    im = get_pc().GetPawnInventoryManager()
    removed = 0
    chain, backpack = _weapons()
    held = get_pc().Pawn.Weapon
    for it in backpack + chain:
        if _obj_path(it) in MP["spawned"] and it != held:
            try:
                if it in backpack:
                    im.RemoveInventoryFromBackpack(it)
                else:
                    im.RemoveFromInventory(it)
                it.Destroy()
                removed += 1
            except Exception:
                pass
    return removed


def mp_props(obj, match=""):
    out = {}
    for q in obj.Class._properties():
        if match.lower() in q.Name.lower():
            try:
                out[q.Name] = _p(getattr(obj, q.Name))
            except Exception:
                pass
    return out


def mp_funcs(cls_name, words):
    cls = unrealsdk.find_class(cls_name)
    res = []
    while cls is not None:
        for f in cls._fields():
            if f.Class.Name == "Function" and any(w.lower() in f.Name.lower() for w in words):
                res.append(f"{cls.Name}.{f.Name}")
        cls = cls.SuperField
    return res


print("menus_probe loaded")
