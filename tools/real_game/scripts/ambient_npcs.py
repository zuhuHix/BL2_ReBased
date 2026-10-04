"""Ambient-NPC ground truth: which town NPCs exist in the running game, what they do, and where they walk.

Run once through Invoke-GamePyFile (tools/real_game/realgame.ps1). Our own code; what it writes is game data and goes
under RG_OUT (normally the ignored local/realgame/ambient). It changes no pawn, den or save; only amb_goto and amb_hud touch the player (position, HUD flag).

- amb_pawns(): one row per live WillowAIPawn (class, display name, mesh, location, rotation, speed, current animation
  when it can be read).
- amb_dens(): every PopulationOpportunityDen / WillowPopulationEncounter with its enabled flag and the pawns it owns.
- amb_sample(name, seconds, every): per-frame sampler (ticks of the viewport client) appending rows
  {t, game_t, pawns: {path: [x, y, z, yaw]}} to <RG_OUT>/<name>.jsonl until the time is up.
- amb_stop(): stop the sampler.
- amb_dump(name): amb_pawns() + amb_dens() into <RG_OUT>/<name>.json.
- amb_goto(x, y, z, lx, ly, lz) / amb_hud(show) / amb_live(name): place the player, hide the HUD, list live pawns (screenshots).
"""
import json
import time
from pathlib import Path

import unrealsdk
from unrealsdk.hooks import Type, add_hook, remove_hook

TICK = "WillowGame.WillowGameViewportClient:Tick"
HOOK = "ow_amb_sample"
amb_state = {"file": None, "until": 0.0, "last": 0.0, "every": 0.5}


def _vec(v):
    return [round(v.X, 2), round(v.Y, 2), round(v.Z, 2)] if v is not None else None


def _name(obj):
    try:
        return obj._path_name()
    except Exception:
        return repr(obj)


def _display(pawn):
    try:
        return str(pawn.AIClass.DefaultDisplayName)
    except Exception:
        return None


def _anim(pawn):
    """Names of the animation sequence nodes the pawn's mesh is playing, when the SDK exposes them."""
    names = []
    try:
        for node in pawn.Mesh.AnimTickArray:
            if node is not None and "AnimNodeSequence" in node.Class.Name and node.bPlaying:
                names.append(str(node.AnimSeqName))
    except Exception:
        pass
    return names


def amb_pawns():
    rows = []
    for pawn in unrealsdk.find_all("WillowAIPawn"):
        path = _name(pawn)
        if "Default__" in path:
            continue
        row = {"path": path, "name": _display(pawn), "loc": _vec(pawn.Location), "yaw": getattr(pawn.Rotation, "Yaw", None)}
        try:
            row["mesh"] = str(pawn.Mesh.SkeletalMesh.Name)
        except Exception:
            row["mesh"] = None
        try:
            row["speed"] = round(pawn.Velocity.X ** 2 + pawn.Velocity.Y ** 2, 1) ** 0.5
        except Exception:
            row["speed"] = None
        row["anim"] = _anim(pawn)
        rows.append(row)
    return rows


def amb_dens():
    rows = []
    for cls in ("PopulationOpportunityDen", "WillowPopulationEncounter"):
        for den in unrealsdk.find_all(cls):
            path = _name(den)
            if "Default__" in path:
                continue
            row = {"class": cls, "path": path, "enabled": bool(getattr(den, "IsEnabled", None))}
            for field in ("MaxActiveActorsIsNormal", "MaxActiveActorsThreatened"):
                try:
                    row[field] = int(getattr(den, field))
                except Exception:
                    pass
            try:
                row["population"] = _name(den.PopulationDef)
            except Exception:
                pass
            rows.append(row)
    return rows


def _sample(*_args):
    now = time.perf_counter()
    if amb_state["file"] is None:
        return
    if now >= amb_state["until"]:
        amb_stop()
        return
    if now - amb_state["last"] < amb_state["every"]:
        return
    amb_state["last"] = now
    row = {"t": now, "pawns": {}}
    for pawn in unrealsdk.find_all("WillowAIPawn"):
        path = _name(pawn)
        if "Default__" in path:
            continue
        loc = pawn.Location
        row["pawns"][path] = [round(loc.X, 1), round(loc.Y, 1), round(loc.Z, 1), getattr(pawn.Rotation, "Yaw", 0)]
    with Path(amb_state["file"]).open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(row) + "\n")


def amb_sample(name, seconds=60.0, every=0.5):
    out = Path(RG_OUT)
    out.mkdir(parents=True, exist_ok=True)
    amb_state.update(file=str(out / (name + ".jsonl")), until=time.perf_counter() + seconds, last=0.0, every=every)
    Path(amb_state["file"]).write_text("", encoding="utf-8")
    add_hook(TICK, Type.POST_UNCONDITIONAL, HOOK, _sample)
    return amb_state["file"]


def amb_stop():
    amb_state["file"] = None
    try:
        remove_hook(TICK, Type.POST_UNCONDITIONAL, HOOK)
    except Exception:
        pass


def amb_dump(name):
    out = Path(RG_OUT)
    out.mkdir(parents=True, exist_ok=True)
    data = {"pawns": amb_pawns(), "dens": amb_dens()}
    (out / (name + ".json")).write_text(json.dumps(data, indent=1), encoding="utf-8")
    return {"pawns": len(data["pawns"]), "dens": len(data["dens"]), "enabled_dens": sum(1 for d in data["dens"] if d["enabled"])}


def amb_hud(show):
    """Show or hide the HUD (screenshots only)."""
    get_pc().myHUD.bShowHUD = bool(show)


def amb_goto(x, y, z, look_x, look_y, look_z):
    """Put the player pawn at (x, y, z) looking at (look_x, look_y, look_z); the pawn drops to the floor by itself."""
    import math
    pc = get_pc()
    # SetLocation returned False here (2026-10-04); writing the property moves the pawn, and falling physics drops it to the floor.
    pc.Pawn.Location = unrealsdk.make_struct("Vector", X=x, Y=y, Z=z)
    pc.Pawn.SetPhysics(2)
    dx, dy, dz = look_x - x, look_y - y, look_z - (z + 64.0)
    yaw = math.degrees(math.atan2(dy, dx))
    pitch = math.degrees(math.atan2(dz, math.hypot(dx, dy)))
    pc.SetRotation(unrealsdk.make_struct("Rotator", Pitch=int(pitch * 65536 / 360), Yaw=int(yaw * 65536 / 360), Roll=0))


def amb_live(name="Sanctuary Citizen"):
    """Live pawns with this display name that have a real location: [(path, [x, y, z], yaw, speed)]."""
    return [(r["path"], r["loc"], r["yaw"], r["speed"]) for r in amb_pawns() if r["name"] == name and r["loc"] != [0.0, 0.0, 0.0]]


amb_target = {"path": None}


def _find(path):
    for pawn in unrealsdk.find_all("WillowAIPawn"):
        if _name(pawn) == path:
            return pawn
    return None


def amb_reaim():
    """Turn the player toward the pawn picked by the last amb_view_* call (look objects up again each time)."""
    import math
    pawn = _find(amb_target["path"]) if amb_target["path"] else None
    if pawn is None:
        return None
    pc = get_pc()
    me = pc.Pawn.Location
    dx, dy, dz = pawn.Location.X - me.X, pawn.Location.Y - me.Y, pawn.Location.Z - (me.Z + 64.0) - 20.0
    yaw = math.degrees(math.atan2(dy, dx))
    pitch = math.degrees(math.atan2(dz, math.hypot(dx, dy)))
    pc.SetRotation(unrealsdk.make_struct("Rotator", Pitch=int(pitch * 65536 / 360), Yaw=int(yaw * 65536 / 360), Roll=0))
    return [round(pawn.Location.X), round(pawn.Location.Y), round(pawn.Location.Z)]


def amb_view_idle(cx, cy, cz, ax, ay, radius=1500.0):
    """Camera at (cx, cy, cz) (z is raised so the pawn falls to the floor), aim at the live citizen nearest to (ax, ay)."""
    import math
    best = None
    for path, loc, yaw, speed in amb_live():
        d = math.hypot(loc[0] - ax, loc[1] - ay)
        if d < radius and (best is None or d < best[0]):
            best = (d, path, loc)
    amb_target["path"] = best[1] if best else None
    amb_goto(cx, cy, cz + 60.0, ax, ay, cz)
    return best


def amb_view_walk(path):
    """Camera on the right of the live pawn `path` and a little ahead of it, aimed at it (pick a walker from a sample file:
    the Velocity property reads 0 for most scripted walkers, so moving pawns are found by displacement, not by speed)."""
    import math
    pawn = _find(path)
    if pawn is None:
        amb_target["path"] = None
        return None
    amb_target["path"] = path
    loc, yaw = pawn.Location, pawn.Rotation.Yaw
    a = math.radians(yaw * 360.0 / 65536.0)
    cx = loc.X + (-math.sin(a)) * 380.0 + math.cos(a) * 420.0
    cy = loc.Y + math.cos(a) * 380.0 + math.sin(a) * 420.0
    amb_goto(cx, cy, loc.Z + 40.0 + 60.0, loc.X, loc.Y, loc.Z + 40.0)
    return [round(loc.X), round(loc.Y), round(loc.Z), round(yaw * 360.0 / 65536.0)]


def amb_view_front(path, distance=240.0, turn=0.0):
    """Camera `distance` from the live pawn `path` at `turn` degrees round its facing (0 = in front), aimed at its chest.
    Perches face walls, so callers try several turns and keep the frame that shows the pawn (FastTrace read False in every
    direction when tried, so it cannot pick the free side)."""
    import math
    pawn = _find(path)
    if pawn is None:
        amb_target["path"] = None
        return None
    amb_target["path"] = path
    loc, yaw = pawn.Location, pawn.Rotation.Yaw
    base = yaw * 360.0 / 65536.0
    a = math.radians(base + turn)
    amb_goto(loc.X + math.cos(a) * distance, loc.Y + math.sin(a) * distance, loc.Z + 60.0, loc.X, loc.Y, loc.Z)
    return [round(loc.X), round(loc.Y), round(loc.Z), round(base), turn]


def amb_los():
    """True when the player's controller has a line of sight to the pawn picked by the last amb_view_* call (call it a moment after
    amb_goto: the pawn's new position is only used after a physics tick)."""
    pawn = _find(amb_target["path"]) if amb_target["path"] else None
    return bool(pawn is not None and get_pc().LineOfSightTo(pawn))


def amb_cam(cx, cy, cz, px, py, pz):
    """Player at the camera spot (cx, cy, cz) aimed at (px, py, pz + 25): a spot chosen in the host by a collision sweep."""
    amb_goto(cx, cy, cz, px, py, pz + 25.0)


def amb_aim_at(px, py, pz):
    """Turn the player toward a point from where the pawn has settled (call a moment after amb_cam: the pawn falls to the floor)."""
    import math
    pc = get_pc()
    me = pc.Pawn.Location
    dx, dy, dz = px - me.X, py - me.Y, (pz + 25.0) - (me.Z + 64.0)
    pc.SetRotation(unrealsdk.make_struct("Rotator", Pitch=int(math.degrees(math.atan2(dz, math.hypot(dx, dy))) * 65536 / 360),
                                         Yaw=int(math.degrees(math.atan2(dy, dx)) * 65536 / 360), Roll=0))
