"""Walking ground truth: the player's movement settings, blocking-volume state and steered walks.

Run once through Invoke-GamePyFile (tools/real_game/realgame.ps1). Our own code; what it writes is game data
and goes under RG_OUT (normally the ignored local/realgame/walking). Only walk_goto and walk_route touch the
player (position and view yaw); nothing changes a save, a volume or the movement settings.

- walk_info(): the player pawn's cylinder, MaxStepHeight, WalkableFloorZ, speeds, JumpZ, gravity, physics.
- walk_volumes(): every Volume subclass with its collision flags, component flags and world bounds.
- walk_goto(x, y, z, yaw_degrees): place the player; falling physics drops it to the floor.
- walk_route(name, points, seconds): steer toward each waypoint in turn (view yaw set every viewport tick)
  while the caller holds the forward key; one row per tick goes to <RG_OUT>/<name>.jsonl. Stops at the last
  waypoint, at the time limit or when the pawn makes no progress for two seconds. walk_status() reports it.
- walk_record(name, seconds): the same rows without steering, for a fixed time (a jump).
"""
import json
import math
import time
from pathlib import Path

import unrealsdk
from unrealsdk.hooks import Type, add_hook, remove_hook

TICK = "WillowGame.WillowGameViewportClient:Tick"
WALK_HOOK = "ow_walk_route"
REACH = 40.0
walk_state = {"file": None}


def _vec(v):
    return [round(v.X, 2), round(v.Y, 2), round(v.Z, 2)] if v is not None else None


def _get(obj, name):
    try:
        value = getattr(obj, name)
    except Exception:
        return None
    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    if hasattr(value, "X") and hasattr(value, "Y") and hasattr(value, "Z"):
        return _vec(value)
    try:
        return value._path_name()
    except Exception:
        return str(value)


def _physics(pawn):
    value = _get(pawn, "Physics")
    return str(value) if value is not None else None


def walk_info():
    pc = unrealsdk.find_all("WillowPlayerController")
    pc = [p for p in pc if p.Pawn is not None][0]
    pawn = pc.Pawn
    cyl = pawn.CylinderComponent
    info = {"pawn": pawn._path_name(), "class": pawn.Class.Name, "location": _vec(pawn.Location),
            "physics": _physics(pawn), "base": _get(pawn, "Base"),
            "cylinder": {"radius": cyl.CollisionRadius, "height": cyl.CollisionHeight}}
    for name in ("MaxStepHeight", "MaxJumpHeight", "WalkableFloorZ", "LedgeCheckThreshold", "GroundSpeed",
                 "AirSpeed", "AccelRate", "JumpZ", "AirControl", "MaxFallSpeed", "WalkingPct", "CrouchedPct",
                 "SprintingPct", "MovementSpeedModifier", "BaseEyeHeight", "EyeHeight", "bCanWalkOffLedges",
                 "bAllowLedgeOverhang", "bCanJump", "CrouchHeight", "CrouchRadius", "Mass", "bIsCrouched"):
        info[name] = _get(pawn, name)
    for name in ("SprintSpeed", "SprintingPct", "GroundSpeedBaseValue", "JumpZBaseValue", "AccelRateBaseValue"):
        info.setdefault(name, _get(pawn, name))
    world = pawn.WorldInfo
    info["gravity"] = {"DefaultGravityZ": _get(world, "DefaultGravityZ"), "WorldGravityZ": _get(world, "WorldGravityZ"),
                       "GlobalGravityZ": _get(world, "GlobalGravityZ")}
    try:
        info["gravity"]["GetGravityZ"] = world.GetGravityZ()
    except Exception:
        pass
    return info


def walk_volumes():
    rows = []
    for volume in unrealsdk.find_all("Volume"):
        path = volume._path_name()
        if "PersistentLevel" not in path:
            continue  # class defaults and archetypes
        row = {"path": path, "class": volume.Class.Name, "location": _vec(volume.Location)}
        for name in ("bCollideActors", "bBlockActors", "bHidden", "bWorldGeometry", "bEnabled", "bBlockCamera"):
            row[name] = _get(volume, name)
        component = volume.CollisionComponent
        if component is not None:
            row["component"] = {n: _get(component, n) for n in ("CollideActors", "BlockActors", "BlockZeroExtent",
                                                                "BlockNonZeroExtent", "BlockRigidBody")}
            try:
                bounds = component.Bounds
                row["bounds"] = {"origin": _vec(bounds.Origin), "extent": _vec(bounds.BoxExtent)}
            except Exception:
                pass
        rows.append(row)
    return rows


def walk_goto(x, y, z, yaw_degrees):
    pc = unrealsdk.find_all("WillowPlayerController")
    pc = [p for p in pc if p.Pawn is not None][0]
    pc.Pawn.Velocity = unrealsdk.make_struct("Vector", X=0.0, Y=0.0, Z=0.0)
    pc.Pawn.Location = unrealsdk.make_struct("Vector", X=x, Y=y, Z=z)
    pc.Pawn.SetPhysics(2)  # PHYS_Falling, as amb_goto: it lands by itself
    pc.SetRotation(unrealsdk.make_struct("Rotator", Pitch=0, Yaw=int(yaw_degrees * 65536 / 360), Roll=0))


def _tick(*_args):
    state = walk_state
    if not state["file"]:
        return
    try:
        pc = [p for p in unrealsdk.find_all("WillowPlayerController") if p.Pawn is not None][0]
        pawn = pc.Pawn
        now = time.perf_counter()
        loc = pawn.Location
        points, wp = state["points"], state["wp"]
        if not points:  # walk_record: no steering, fixed duration
            row = {"t": round(now - state["t0"], 4), "p": _vec(loc), "v": _vec(pawn.Velocity),
                   "feet": round(loc.Z - pawn.CylinderComponent.CollisionHeight, 2), "phys": _physics(pawn)}
            with open(state["file"], "a", encoding="utf-8") as handle:
                handle.write(json.dumps(row) + "\n")
            if now - state["t0"] > state["seconds"]:
                state.update(file=None, result="recorded", ended=now)
            return
        while wp < len(points) - 1 and math.hypot(points[wp][0] - loc.X, points[wp][1] - loc.Y) <= REACH:
            wp += 1
        state["wp"] = wp
        target = points[wp]
        yaw = math.degrees(math.atan2(target[1] - loc.Y, target[0] - loc.X))
        pc.Rotation = unrealsdk.make_struct("Rotator", Pitch=0, Yaw=int(yaw * 65536 / 360), Roll=0)
        row = {"t": round(now - state["t0"], 4), "p": _vec(loc), "v": _vec(pawn.Velocity),
               "feet": round(loc.Z - pawn.CylinderComponent.CollisionHeight, 2), "phys": _physics(pawn),
               "base": _get(pawn, "Base"), "floor": _vec(pawn.Floor), "wp": wp}
        with open(state["file"], "a", encoding="utf-8") as handle:
            handle.write(json.dumps(row) + "\n")
        if math.hypot(loc.X - state["best"][0], loc.Y - state["best"][1]) > 10:
            state["best"], state["best_t"] = [loc.X, loc.Y], now
        done = None
        if wp == len(points) - 1 and math.hypot(target[0] - loc.X, target[1] - loc.Y) <= REACH:
            done = "reached"
        elif now - state["t0"] > state["seconds"]:
            done = "timeout"
        elif now - state["best_t"] > 2.0:
            done = "stuck"
        if done:
            state.update(file=None, result=done, ended=now)
    except Exception as error:  # never break the game
        walk_state.update(file=None, result="error: " + str(error)[:200])


def walk_route(name, points, seconds=20.0):
    out = Path(RG_OUT)  # noqa: F821 (set by Invoke-GamePyFile)
    out.mkdir(parents=True, exist_ok=True)
    path = out / (name + ".jsonl")
    path.write_text("", encoding="utf-8")
    now = time.perf_counter()
    walk_state.update(file=str(path), points=[list(p) for p in points], wp=1 if len(points) > 1 else 0,
                      t0=now, seconds=seconds, best=[1e30, 1e30], best_t=now, result=None, name=name)
    add_hook(TICK, Type.POST_UNCONDITIONAL, WALK_HOOK, _tick)
    return str(path)


def walk_record(name, seconds=3.0):
    """Record the pawn every tick without steering (e.g. around a jump key press)."""
    return walk_route(name, [], seconds)


def walk_status():
    return {"name": walk_state.get("name"), "running": bool(walk_state["file"]), "result": walk_state.get("result")}


def walk_stop():
    walk_state["file"] = None
    try:
        remove_hook(TICK, Type.POST_UNCONDITIONAL, WALK_HOOK)
    except Exception:
        pass


def walk_dump(name):
    out = Path(RG_OUT)  # noqa: F821
    out.mkdir(parents=True, exist_ok=True)
    data = {"info": walk_info(), "volumes": walk_volumes()}
    (out / (name + ".json")).write_text(json.dumps(data, indent=1), encoding="utf-8")
    return {"volumes": len(data["volumes"]), "pawn": data["info"]["class"]}
