"""HUD minimap ground truth for the openwillow_realgame command channel.

Our own code; what it writes is game data and goes under RG_OUT (normally the ignored local/realgame/minimap).
Run through Invoke-GamePyFile. The minimap's driver (HUDWidget_Minimap.Init/Update/SetMapMovie/UpdateTargetRadius) is
native, so its rules are read from the running movie and from the objects it reads:

- mm_probe(label): one JSON dump: the HUD movie's minimap widget (every property), the level's WillowMapInfo and
  WillowTacticalMapVolume (every property, the brush bounds), the player's view, and one read of every clip below.
- mm_start(label) / mm_stop(): one JSON line per rendered frame: perf_counter, the pawn location and rotation, the view
  rotation, and _x/_y/_rotation/_xscale/_yscale/_visible/_alpha/_currentframe of every clip in MM_CLIPS plus the
  extra paths given to mm_start (icons the probe found).
- mm_children(path, names): which of the given child names exist under a clip, with their reads (a GFxObject cannot
  list its children from script, so the names come from the movie's tree).
- mm_icons(label): every icon slot of the widget (bVisible, MapPos) with its clip's reads and sub-clips.
- mm_sources(label): the live interactive objects with their CompassIcon, definition and location (the icons' sources).
- mm_turn(yaw) / mm_goto(x, y, z, yaw): set the view yaw (writing PlayerController.Rotation) / place the pawn.
"""
import json
import math
import re
import time
from pathlib import Path

import unrealsdk
from unrealsdk.hooks import Type, add_hook, remove_hook

MM_TICK = "WillowGame.WillowGameViewportClient:Tick"
MM_HOOK = "ow_minimap_sample"
MM_ROOT = "p1.minimapEcho"
MM_CLIPS = (MM_ROOT, MM_ROOT + ".minimap", MM_ROOT + ".echo", MM_ROOT + ".minimap.mapmask",
            MM_ROOT + ".minimap.mapPlaceholder", MM_ROOT + ".minimap.DirArrow", MM_ROOT + ".minimap.north",
            MM_ROOT + ".minimap.flash", MM_ROOT + ".minimap.waypointsmask", MM_ROOT + ".minimap.MaskedClip",
            MM_ROOT + ".minimap.recording")
MM_PROPS = ("_x", "_y", "_rotation", "_xscale", "_yscale", "_width", "_height", "_alpha", "_currentframe",
            "_totalframes")
mm = {"file": None, "extra": [], "frames": 0}


def mm_root():
    """RG_OUT resolved, refused unless it lies inside a repository's ignored local/ (an ancestor named `local` whose
    parent holds `.git`, a folder in the main checkout and a file in a worktree). realgame.ps1 builds RG_OUT from its
    -OutFolder argument, so a folder like '../../docs' would otherwise send game data into tracked files. The script
    runs through the command channel's exec, where __file__ is not the script, hence the search from RG_OUT itself."""
    root = Path(RG_OUT).resolve()
    if not any(a.name == "local" and (a.parent / ".git").exists() for a in root.parents):
        raise ValueError(f"RG_OUT {root} is not inside a repository's local/ folder")
    return root


def mm_out(label, suffix=".json"):
    """RG_OUT/<label><suffix>, refused unless RG_OUT is inside the repository's local/ (mm_root), the label is a plain
    file name and the path stays directly under RG_OUT: what the capture writes is game data."""
    if not isinstance(label, str) or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]*", label) or ".." in label:
        raise ValueError(f"capture label must be a plain file name, got {label!r}")
    root = mm_root()
    path = (root / f"{label}{suffix}").resolve()
    if path.parent != root:
        raise ValueError(f"{path} is not directly under {root}")
    root.mkdir(parents=True, exist_ok=True)
    return path


def _try(fn):
    try:
        return fn()
    except Exception as error:
        return f"<{type(error).__name__}: {str(error)[:80]}>"


def _movie():
    hud = get_pc().myHUD
    return getattr(hud, "HUDMovie", None) if hud is not None else None


def _props(obj, depth=0):
    out = {}
    for field in obj.Class._fields():
        if field.Class.Name.endswith("Property"):
            out[field.Name] = _try(lambda: plain(getattr(obj, field.Name)))
    return out


def _clip(movie, path):
    row = {p: _try(lambda: movie.GetVariableNumber(f"{path}.{p}")) for p in MM_PROPS}
    row["_visible"] = _try(lambda: movie.GetVariableBool(f"{path}._visible"))
    row["_name"] = _try(lambda: movie.GetVariableString(f"{path}._name"))
    return row


def _view():
    pc = get_pc()
    pawn = pc.Pawn
    loc = pawn.Location if pawn is not None else None
    rot = pawn.Rotation if pawn is not None else None
    return {"pawn": [loc.X, loc.Y, loc.Z] if loc is not None else None,
            "pawn_yaw": (rot.Yaw * 360.0 / 65536.0) if rot is not None else None,
            "view_yaw": pc.Rotation.Yaw * 360.0 / 65536.0, "view_pitch": pc.Rotation.Pitch * 360.0 / 65536.0}


def mm_probe(label, extra=()):
    movie = _movie()
    if movie is None:
        return {"error": "no HUD movie"}
    out = {"movie": movie._path_name(), "view": _view()}
    widget = _try(lambda: movie.MyMinimapWidget)
    out["widget_class"] = _try(lambda: widget.Class.Name)
    out["widget"] = _props(widget) if hasattr(widget, "Class") else str(widget)
    # The widget's own GFxObject references: read each one's display info and path.
    objs = {}
    if hasattr(widget, "Class"):
        for field in widget.Class._fields():
            if field.Class.Name == "ObjectProperty":
                value = _try(lambda: getattr(widget, field.Name))
                if hasattr(value, "Class") and value.Class.Name.startswith("GFx"):
                    objs[field.Name] = {"class": value.Class.Name,
                                        "display": _try(lambda: plain(value.GetDisplayInfo())),
                                        "_target": _try(lambda: value.GetString("_target")),
                                        "_name": _try(lambda: value.GetString("_name"))}
    out["widget_gfx"] = objs
    wi = get_pc().WorldInfo
    info = _try(lambda: wi.MyMapInfo) if hasattr(wi, "MyMapInfo") else None
    if not hasattr(info, "Class"):
        found = [o for o in unrealsdk.find_all("WillowMapInfo", False) if "Default__" not in o._path_name()]
        info = found[0] if found else None
    out["map_info"] = {"path": info._path_name(), "props": _props(info)} if info is not None else None
    vols = [o for o in unrealsdk.find_all("WillowTacticalMapVolume", False) if "Default__" not in o._path_name()]
    out["volumes"] = []
    for vol in vols:
        row = {"path": vol._path_name(), "props": _props(vol)}
        brush = _try(lambda: vol.BrushComponent)
        if hasattr(brush, "Class"):
            row["brush_bounds"] = _try(lambda: plain(brush.Bounds))
        out["volumes"].append(row)
    defaults = _try(lambda: unrealsdk.find_class("WillowTacticalMapVolume").ClassDefaultObject)
    out["volume_defaults"] = _props(defaults) if hasattr(defaults, "Class") else str(defaults)
    out["clips"] = {path: _clip(movie, path) for path in list(MM_CLIPS) + list(extra)}
    out["discovered"] = _try(lambda: plain(get_pc().DiscoveredWorldAreas))
    out["absolute_minimap"] = _try(lambda: get_pc().bAbsoluteMinimap)
    mm_out(label).write_text(json.dumps(out, indent=1, default=str), encoding="utf-8")
    return {"widget": out["widget_class"], "gfx": list(objs), "volumes": len(out["volumes"]),
            "map_info": out["map_info"]["path"] if out["map_info"] else None}


def mm_children(path, names):
    """Which of `names` exist under `path` (a clip answers _name), with their reads."""
    movie = _movie()
    found = {}
    for name in names:
        full = f"{path}.{name}"
        got = _try(lambda: movie.GetVariableString(full + "._name"))
        if isinstance(got, str) and got and not got.startswith("<"):
            found[name] = _clip(movie, full)
    return found


MM_ICON_FLOATS = ("_x", "_y", "_rotation", "_xscale", "_yscale", "_alpha", "_currentframe", "_totalframes", "_width",
                  "_height")


def _gfx(obj):
    if not hasattr(obj, "GetFloat"):
        return None
    row = {p: _try(lambda: obj.GetFloat(p)) for p in MM_ICON_FLOATS}
    row["_visible"] = _try(lambda: obj.GetBool("_visible"))
    row["_target"] = _try(lambda: obj.GetString("_target"))
    row["_name"] = _try(lambda: obj.GetString("_name"))
    return row


def mm_icons(label=None):
    """Every icon slot of the minimap widget: bVisible, MapPos and the slot clip's reads (plus its sub-clips)."""
    widget = _movie().MyMinimapWidget
    out = {"view": _view(), "MapClip": _gfx(widget.MapClip), "DirArrowClip": _gfx(widget.DirArrowClip),
           "NorthMarkerClip": _gfx(widget.NorthMarkerClip), "CachedPlayerYaw": widget.CachedPlayerYaw,
           "WorldRadius": widget.WorldRadius, "arrays": {}}
    for field in widget.Class._fields():
        if not field.Name.startswith("Icons_"):
            continue
        rows = []
        for i, entry in enumerate(list(getattr(widget, field.Name))):
            row = {"i": i, "bVisible": entry.bVisible, "MapPos": [entry.MapPos.X, entry.MapPos.Y],
                   "Object": _gfx(entry.Object)}
            for sub in ("OptionalClip", "AboveClip", "BelowClip"):
                if hasattr(entry, sub):
                    row[sub] = _gfx(getattr(entry, sub))
            target = row["Object"]["_target"] if row["Object"] else None
            if isinstance(target, str) and target.startswith("/"):
                path = target[1:].replace("/", ".")
                row["children"] = mm_children(path, ("objective", "edgeArrow", "above", "below", "icon", "arrow"))
            rows.append(row)
        out["arrays"][field.Name] = rows
    if label:
        mm_out(label).write_text(json.dumps(out, indent=1, default=str), encoding="utf-8")
    return out


def _sample(*_args):
    try:
        movie = _movie()
        if movie is None or mm["file"] is None:
            return True
        row = {"t": time.perf_counter(), "tw": get_pc().WorldInfo.TimeSeconds, **_view()}
        for path in list(MM_CLIPS) + mm["extra"]:
            row[path] = {p: _try(lambda: movie.GetVariableNumber(f"{path}.{p}"))
                         for p in ("_x", "_y", "_rotation", "_xscale", "_yscale", "_alpha", "_currentframe")}
            row[path]["_visible"] = _try(lambda: movie.GetVariableBool(f"{path}._visible"))
        mm["file"].write(json.dumps(row, default=str) + "\n")
        mm["frames"] += 1
    except Exception as error:
        if mm["file"] is not None:
            mm["file"].write(json.dumps({"error": str(error)[:200]}) + "\n")
    return True


def mm_start(label, extra=()):
    mm_stop()
    mm["file"] = mm_out(label, ".jsonl").open("w", encoding="utf-8")
    mm["extra"] = list(extra)
    mm["frames"] = 0
    add_hook(MM_TICK, Type.POST, MM_HOOK, _sample)
    return label


def mm_stop():
    remove_hook(MM_TICK, Type.POST, MM_HOOK)
    if mm["file"] is not None:
        mm["file"].close()
        mm["file"] = None
    return mm["frames"]


def mm_turn(degrees):
    """Set the view yaw (degrees, game convention) without moving."""
    # SetRotation left the view unchanged here (2026-10-22); writing the property turns it, and the minimap follows.
    pc = get_pc()
    r = pc.Rotation
    pc.Rotation = unrealsdk.make_struct("Rotator", Pitch=r.Pitch, Yaw=int(round(degrees * 65536 / 360)) & 0xFFFF, Roll=0)
    return degrees


def mm_goto(x, y, z, yaw):
    """Put the pawn at (x, y, z) facing `yaw` degrees; it drops to the floor by itself."""
    pc = get_pc()
    pc.Pawn.Location = unrealsdk.make_struct("Vector", X=x, Y=y, Z=z)
    pc.Pawn.SetPhysics(2)
    mm_turn(yaw)
    return [x, y, z, yaw]


def mm_sources(label):
    """The world objects behind the icons: every live interactive object with its CompassIcon, definition, class and
    location; the travel stations; and WillowGlobals' interactive-object list when it answers."""
    rows = []
    for obj in unrealsdk.find_all("WillowInteractiveObject", False):
        if "Default__" in obj._path_name():
            continue
        definition = _try(lambda: obj.InteractiveObjectDefinition)
        loc = obj.Location
        rows.append({"path": obj._path_name(), "class": obj.Class.Name, "icon": _try(lambda: int(obj.CompassIcon)),
                     "def": definition._path_name() if hasattr(definition, "_path_name") else str(definition),
                     "def_icon": _try(lambda: int(definition.CompassIcon)) if hasattr(definition, "Class") else None,
                     "loc": [loc.X, loc.Y, loc.Z], "hidden": _try(lambda: obj.bHidden)})
    stations = []
    for cls in ("FastTravelStation", "LevelTravelStation", "ResurrectTravelStation", "TravelStation"):
        for obj in unrealsdk.find_all(cls, False):
            if "Default__" in obj._path_name():
                continue
            loc = obj.Location
            stations.append({"path": obj._path_name(), "class": obj.Class.Name, "loc": [loc.X, loc.Y, loc.Z],
                             "icon": _try(lambda: int(obj.CompassIcon)) if hasattr(obj, "CompassIcon") else None})
    glob = _try(lambda: unrealsdk.find_all("WillowGlobals", False))
    listed = []
    if isinstance(glob, list):
        for g in glob:
            if "Default__" in g._path_name():
                continue
            for field in g.Class._fields():
                if "Interactive" in field.Name or "Compass" in field.Name or "Minimap" in field.Name:
                    listed.append({"field": field.Name, "value": _try(lambda: plain(getattr(g, field.Name)))})
    out = {"view": _view(), "objects": rows, "stations": stations, "globals": listed}
    mm_out(label).write_text(json.dumps(out, indent=1, default=str), encoding="utf-8")
    return {"objects": len(rows), "with_icon": sum(1 for r in rows if r["icon"] not in (0, None)),
            "stations": len(stations), "globals": len(listed)}
