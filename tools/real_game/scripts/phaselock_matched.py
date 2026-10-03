"""Matched-distance Phaselock setup for the openwillow_realgame command channel.

Run through Invoke-GamePyFile AFTER scripts/phaselock.py (it uses ai_pawn() and pl_start()
from that file's namespace). Our own code; what it prints is game data and stays under local/.

- list_ai(): the AI pawns on the map with name, class and distance from Maya.
- bounds(name): collision cylinder and mesh bounds of one pawn, plus scale values, so a host stand-in
  can be sized from the pawn's real extents.
- isolate(name): the other map AI pawns stand still (controllers detached), the target cannot walk
  (GroundSpeed 0) but keeps its controller; nothing is destroyed (saving is blocked by block_saves()).
- mark_lock(name): mark every CanPhaseLockTarget result and the PhaseLockTarget call on the burst clock.
- place(name, distance, bearing_deg): put Maya that far from the pawn, looking at it; measure(name) reads
  the resulting distances.
- camera(): the field of view and eye height the capture is taken with.
"""
import math

import unrealsdk
from unrealsdk.hooks import Type, add_hook


state = globals().setdefault("state", {})


def _v(v):
    return [round(v.X, 2), round(v.Y, 2), round(v.Z, 2)]


def list_ai():
    me = get_pc().Pawn.Location
    rows = []
    for pawn in unrealsdk.find_all("WillowAIPawn"):
        if not pawn.Name.startswith("WillowAIPawn_"):
            continue
        loc = pawn.Location
        rows.append((round(math.dist((me.X, me.Y, me.Z), (loc.X, loc.Y, loc.Z))), pawn.Name, pawn.Class.Name,
                     bool(pawn.bDeleteMe)))
    return sorted(rows)


def bounds(name):
    pawn = ai_pawn(name)
    out = {"name": pawn.Name, "class": pawn.Class.Name, "location": _v(pawn.Location),
           "draw_scale": pawn.DrawScale, "draw_scale_3d": _v(pawn.DrawScale3D)}
    cyl = getattr(pawn, "CylinderComponent", None)
    if cyl is not None:
        out["cylinder"] = {"radius": cyl.CollisionRadius, "height": cyl.CollisionHeight}
    mesh = getattr(pawn, "Mesh", None)
    if mesh is not None:
        b = mesh.Bounds
        out["mesh_bounds"] = {"origin": _v(b.Origin), "box_extent": _v(b.BoxExtent), "sphere_radius": b.SphereRadius}
        out["mesh_scale"] = mesh.Scale
        out["mesh_scale_3d"] = _v(mesh.Scale3D)
    return out


def isolate(name):
    """Detach the controller of every map AI pawn except the target (they stand where they are), and set
    the target's GroundSpeed to 0 so it cannot walk while its controller stays attached (it must, to be
    locked like any enemy). Call it again just before the cast: spawners add pawns after Maya moves.
    Findings of 2026-10-03: Destroy() and SetLocation() have no effect on pawns (SetLocation() returned
    False even for Maya; assigning `Location` works); a target with a detached controller was not locked
    (the skill ended at once and the target took damage); CustomTimeDilation = 0 held pawns for a while
    but they moved again within a few seconds."""
    target = ai_pawn(name)
    frozen = []
    for pawn in list(unrealsdk.find_all("WillowAIPawn")):
        if not pawn.Name.startswith("WillowAIPawn_") or pawn == target:
            continue  # Pawn_<Type> objects are preloaded templates that spawners copy from
        if pawn.Controller is not None:
            pawn.DetachFromController(False)
            frozen.append(pawn.Name)
    state["ground_speed"] = state.get("ground_speed", target.GroundSpeed)
    target.GroundSpeed = 0.0
    return {"target": target.Name, "detached": frozen, "target_loc": _v(target.Location),
            "ground_speed_was": state["ground_speed"]}


def mark_lock(name):
    """Mark (same clock as the bursts) every CanPhaseLockTarget result and the PhaseLockTarget call."""
    def on_can(obj, args, ret, func):
        mark("can_lock", result=bool(ret), target=args.NewTarget.Name if args.NewTarget else None)

    def on_lock(obj, args, ret, func):
        mark("phase_lock_target", target=ai_pawn(name).Name)

    add_hook("WillowGame.LiftActionSkill:CanPhaseLockTarget", Type.POST, "ow_release", on_can)
    add_hook("WillowGame.LiftActionSkill:PhaseLockTarget", Type.POST, "ow_release", on_lock)


def unmark_lock():
    from unrealsdk.hooks import remove_hook
    remove_hook("WillowGame.LiftActionSkill:CanPhaseLockTarget", Type.POST, "ow_release")
    remove_hook("WillowGame.LiftActionSkill:PhaseLockTarget", Type.POST, "ow_release")


def place(name, distance=650.0, bearing_deg=180.0):
    """Put Maya `distance` units from the pawn (horizontally), `bearing_deg` around it (180 = west of it),
    a little above the floor so she lands on it, looking straight at it."""
    target = ai_pawn(name)
    pc = get_pc()
    t = target.Location
    a = math.radians(bearing_deg)
    x, y = t.X + math.cos(a) * distance, t.Y + math.sin(a) * distance
    pc.Pawn.Location = unrealsdk.make_struct("Vector", X=x, Y=y, Z=t.Z + 60.0)
    yaw = math.atan2(t.Y - y, t.X - x)
    pc.Rotation = unrealsdk.make_struct("Rotator", Pitch=0, Yaw=int(yaw / math.pi * 32768) & 0xFFFF, Roll=0)
    return [round(x), round(y)]


def measure(name):
    """Horizontal and 3-D distance from Maya's location and from her eye to the pawn."""
    target, me = ai_pawn(name).Location, get_pc().Pawn
    l = me.Location
    return {"xy": round(math.hypot(target.X - l.X, target.Y - l.Y), 1), "dz": round(target.Z - l.Z, 1),
            "d3": round(math.dist((target.X, target.Y, target.Z), (l.X, l.Y, l.Z)), 1), "maya": _v(l),
            "target": _v(target)}


def camera():
    pc = get_pc()
    return {"fov": pc.GetFOVAngle(), "default_fov": pc.DefaultFOV, "eye_height": pc.Pawn.BaseEyeHeight,
            "rotation": [pc.Rotation.Pitch, pc.Rotation.Yaw]}
