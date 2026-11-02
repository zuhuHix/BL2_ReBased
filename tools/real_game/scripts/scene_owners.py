"""Sanctuary scene ground truth: live state of the placed meshes the importer does not place as ordinary props.

Run once through Invoke-GamePyFile (tools/real_game/realgame.ps1). Our own code; what it writes is game data and goes
under RG_OUT (normally the ignored local/realgame/scene). It changes nothing in the world; only so_view and so_hud touch
the player (position, HUD flag).

- so_owners(): every WillowInteractiveObject, WillowMissionPickupSpawner, WillowMissionPickupFailsafe and WiringActor,
  plus the named color-stream placements, with each primitive component's class, mesh, hidden flags, collision flags,
  relative and world transform and last render time.
- so_materials(names): class, parent and texture/scalar/vector parameters of the named live materials.
- so_dump(name): both into <RG_OUT>/<name>.json.
- so_view(x, y, z, lx, ly, lz) / so_hud(show): place the player for a screenshot (z drops to the floor).
- so_camera(): the current view (location, rotation in degrees, FOV) to reuse as a host inspection viewpoint.
"""
import json
import math
from pathlib import Path

import unrealsdk

SO_CLASSES = ("WillowInteractiveObject", "WillowMissionPickupSpawner", "WillowMissionPickupFailsafe", "WiringActor")
# Placements whose meshes fail the importer's color-stream check (by the end of their path).
SO_NAMED = ("StaticMeshCollectionActor_13.StaticMeshActor_SMC_1108", "StaticMeshCollectionActor_6.StaticMeshActor_SMC_1205",
            "StaticMeshCollectionActor_8.StaticMeshActor_SMC_1556", "InterpActor_47.StaticMeshComponent_1217")


def _name(obj):
    try:
        return obj._path_name() if obj is not None else None
    except Exception:
        return repr(obj)


def _get(obj, attr):
    try:
        value = getattr(obj, attr)
    except Exception:
        return "<unreadable>"
    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    if hasattr(value, "X") and hasattr(value, "Y") and hasattr(value, "Z") and not hasattr(value, "W"):
        try:
            return [round(value.X, 3), round(value.Y, 3), round(value.Z, 3)]
        except Exception:
            pass
    if hasattr(value, "Pitch"):
        return [value.Pitch, value.Yaw, value.Roll]
    try:
        return _name(value)
    except Exception:
        return repr(value)[:160]


def _matrix(m):
    try:
        return [[round(getattr(getattr(m, plane), axis), 4) for axis in "XYZW"]
                for plane in ("XPlane", "YPlane", "ZPlane", "WPlane")]
    except Exception:
        return None


def _now():
    try:
        return get_pc().WorldInfo.TimeSeconds
    except Exception:
        return None


def _component(comp):
    row = {"path": _name(comp), "class": comp.Class.Name}
    for attr in ("StaticMesh", "SkeletalMesh", "HiddenGame", "HiddenEditor", "bOwnerNoSee", "bOnlyOwnerSee",
                 "CollideActors", "BlockActors", "BlockRigidBody", "AlwaysCheckCollision", "CastShadow",
                 "Translation", "Rotation", "Scale", "Scale3D", "AbsoluteTranslation", "AbsoluteRotation",
                 "AbsoluteScale", "LastRenderTime", "bAttached", "DepthPriorityGroup", "CullDistance",
                 "CachedMaxDrawDistance", "LDMaxDrawDistance"):
        row[attr] = _get(comp, attr)
    row["LocalToWorld"] = _matrix(getattr(comp, "LocalToWorld", None))
    try:
        row["Materials"] = [_name(m) for m in comp.Materials]
    except Exception:
        pass
    return row


def _components(actor):
    seen, rows = set(), []
    for attr in ("AllComponents", "Components"):
        try:
            items = list(getattr(actor, attr))
        except Exception:
            continue
        for comp in items:
            if comp is None or "PrimitiveComponent" not in [c.Name for c in _supers(comp.Class)]:
                continue
            key = _name(comp)
            if key in seen:
                continue
            seen.add(key)
            row = _component(comp)
            row["listed_in"] = attr
            rows.append(row)
    return rows


def _supers(cls):
    out = []
    while cls is not None:
        out.append(cls)
        cls = cls.SuperField
    return out


def _actor(actor):
    row = {"path": _name(actor), "class": actor.Class.Name}
    for attr in ("bHidden", "bStatic", "bNoDelete", "bMovable", "Physics", "CollisionType", "Location", "Rotation",
                 "DrawScale", "DrawScale3D", "LastRenderTime", "Base", "InteractiveObjectDefinition", "bTearOff"):
        row[attr] = _get(actor, attr)
    row["components"] = _components(actor)
    return row


def so_owners():
    rows = []
    for cls in SO_CLASSES:
        for actor in unrealsdk.find_all(cls):
            if "Default__" in _name(actor):
                continue
            rows.append(_actor(actor))
    for comp in unrealsdk.find_all("StaticMeshComponent"):
        path = _name(comp)
        if any(path.endswith(tail) for tail in SO_NAMED):
            row = _component(comp)
            try:
                row["owner"] = _actor(comp.Owner) if comp.Owner is not None else None
            except Exception:
                row["owner"] = None
            rows.append({"path": path, "class": "named_component", "component": row})
    return {"time_seconds": _now(), "rows": rows}


def so_materials(names):
    rows = []
    for name in names:
        for cls in ("MaterialInstanceConstant", "Material", "MaterialInstanceTimeVarying"):
            try:
                obj = unrealsdk.find_object(cls, name)
            except Exception:
                obj = None
            if obj is None:
                continue
            row = {"name": name, "class": cls, "parent": _get(obj, "Parent")}
            for attr, field in (("TextureParameterValues", "ParameterValue"), ("ScalarParameterValues", "ParameterValue"),
                                ("VectorParameterValues", "ParameterValue")):
                try:
                    row[attr] = {str(e.ParameterName): _get(e, field) for e in getattr(obj, attr)}
                except Exception:
                    pass
            try:
                row["expressions"] = [_name(e) for e in obj.Expressions]
            except Exception:
                pass
            rows.append(row)
            break
        else:
            rows.append({"name": name, "missing": True})
    return rows


def so_dump(name, materials=()):
    out = Path(RG_OUT)
    out.mkdir(parents=True, exist_ok=True)
    data = {"owners": so_owners(), "materials": so_materials(materials)}
    (out / (name + ".json")).write_text(json.dumps(data, indent=1), encoding="utf-8")
    return len(data["owners"]["rows"]), len(data["materials"])


def so_camera():
    """The player's current view: location, rotation (degrees) and FOV, for a matched host viewpoint."""
    pc = get_pc()
    # BL2's controller has no PlayerCamera; GetPlayerViewPoint fills its two out parameters.
    _, location, rot = pc.GetPlayerViewPoint(unrealsdk.make_struct("Vector"), unrealsdk.make_struct("Rotator"))
    rotation = [round(((v + 32768) % 65536 - 32768) * 360.0 / 65536.0, 3) for v in (rot.Pitch, rot.Yaw, rot.Roll)]
    return {"location": [round(location.X, 2), round(location.Y, 2), round(location.Z, 2)],
            "rotation": rotation, "fov": round(pc.GetFOVAngle(), 3)}


def so_hud(show):
    get_pc().myHUD.bShowHUD = bool(show)


def so_view(x, y, z, look_x, look_y, look_z):
    """Put the player pawn at (x, y, z) looking at (look_x, look_y, look_z); the pawn drops to the floor by itself."""
    pc = get_pc()
    pc.Pawn.Location = unrealsdk.make_struct("Vector", X=x, Y=y, Z=z)
    pc.Pawn.SetPhysics(2)
    dx, dy, dz = look_x - x, look_y - y, look_z - (z + 64.0)
    pc.SetRotation(unrealsdk.make_struct("Rotator", Pitch=int(math.degrees(math.atan2(dz, math.hypot(dx, dy))) * 65536 / 360),
                                         Yaw=int(math.degrees(math.atan2(dy, dx)) * 65536 / 360), Roll=0))
