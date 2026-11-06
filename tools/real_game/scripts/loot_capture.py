"""Lootable and pickup ground truth for the vendors lane (2026-11-06). Run once through Invoke-GamePyFile, by
tools/real_game/loot_capture.ps1. Our own code; what it writes is game data and stays directly under RG_OUT, inside the
repository's ignored local/. Targets are found in the running world: no position or placed-object id is written here.

It changes memory only, with saves blocked by the capture script: it moves the player, and lowers the ammo pools so that a
pickup can be collected.

- lc_discover(label): every placed interactive object (class, definition, balance, mesh, position), with a count per type.
- lc_materials(label): each lootable mesh's Materials[0], and what each placed object's mesh component renders, GetMaterial(0).
- lc_goto(x, y, z): put the player near a position (from the ignored local data), for a type not found from the start.
- lc_stand(mesh): stand near the first object with that mesh, on the first clear side of eight, looking at it.
- lc_side(mesh): the same object from a quarter turn further round.
- lc_after(mesh): the pickups near it and its use state, after the use key.
- lc_lower_ammo(): halve the ammo pools, recording them.
- lc_stand_pickup(kind): stand back from a pickup of that kind ('ammo' or 'cash') on its level, looking at it.
- lc_pickup_state(): whether that pickup still exists, and the ammo pools and money.
"""
import json
import math
from pathlib import Path
import re

import unrealsdk

LC_MESHES = ("Skel_DahlChest_Ammo", "Skel_Dumpster", "Skel_StorageLocker", "Skel_StrongBox", "Skel_CardboardBox",
             "Skel_LaundryMachine")
LC_MESH_PACKAGE = "Prop_Lootables_02.Meshes."   # most live here; the read reports a miss rather than guessing
LC = globals().get("LC") or {"picked": {}, "side": {}, "pickup": None}


def lc_root():
    """RG_OUT resolved, refused unless it lies inside a repository's ignored local/ folder."""
    root = Path(RG_OUT).resolve()  # noqa: F821 - the command channel's prelude sets RG_OUT
    if not any(a.name == "local" and (a.parent / ".git").exists() for a in root.parents):
        raise ValueError(f"RG_OUT {root} is not inside a repository's local/ folder")
    return root


def lc_out(label):
    """RG_OUT/<label>.json; the label must be a plain file name, because what this writes is game data."""
    if not isinstance(label, str) or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_-]*", label):
        raise ValueError(f"capture label must be a plain file name, got {label!r}")
    root = lc_root()
    root.mkdir(parents=True, exist_ok=True)
    return root / f"{label}.json"


def _write(label, data):
    lc_out(label).write_text(json.dumps(data, indent=1), encoding="utf-8")
    return data


def _try(fn, *args):
    try:
        return fn(*args)
    except Exception as e:  # noqa: BLE001 - a probe records failures instead of stopping
        return f"<error {e!r}>"[:200]


def _path(obj):
    return obj._path_name() if hasattr(obj, "_path_name") else None


def _field(obj, name):
    return _try(lambda: getattr(obj, name))


def _live(class_name):
    return [o for o in unrealsdk.find_all(class_name, False) if "Default__" not in o._path_name()
            and _field(o, "bDeleteMe") is not True]


def _component(actor):
    """The object's skeletal mesh component: a named field if it has one, else the first in its component list."""
    for name in ("Mesh", "SkeletalMeshComponent", "SMesh"):
        value = _field(actor, name)
        if hasattr(value, "SkeletalMesh"):
            return value
    components = _field(actor, "AllComponents")
    for value in (components if not isinstance(components, str) else []):
        if value is not None and hasattr(value, "SkeletalMesh"):
            return value
    return None


def _row(actor):
    component = _component(actor)
    mesh = _field(component, "SkeletalMesh") if component is not None else None
    state = _field(actor, "BalanceDefinitionState")
    loc = actor.Location
    return {"path": actor._path_name(), "class": actor.Class.Name, "definition": _path(_field(actor, "InteractiveObjectDefinition")),
            "balance": _path(_field(state, "BalanceDefinition")) if not isinstance(state, str) else state,
            "mesh": _path(mesh), "pos": [round(loc.X, 1), round(loc.Y, 1), round(loc.Z, 1)]}


def lc_discover(label):
    rows = [_row(actor) for actor in _live("WillowInteractiveObject")]
    counts = {leaf: sum(1 for r in rows if (r["mesh"] or "").endswith("." + leaf)) for leaf in LC_MESHES}
    _write(label, {"objects": rows, "counts": counts})
    return counts


def _pick(leaf):
    """The first placed object with this mesh, kept for the run. A kept one is re-checked, since it may have been destroyed."""
    actor = LC["picked"].get(leaf)
    if actor is not None and _field(actor, "bDeleteMe") is False:
        return actor
    rows = [a for a in _live("WillowInteractiveObject") if (_row(a)["mesh"] or "").endswith("." + leaf)]
    if not rows:
        raise LookupError(f"no placed object with mesh {leaf}")
    LC["picked"][leaf] = sorted(rows, key=lambda a: a._path_name())[0]
    return LC["picked"][leaf]


def lc_materials(label):
    meshes = {}
    for leaf in LC_MESHES:
        mesh = next((m for m in unrealsdk.find_all("SkeletalMesh", False) if m._path_name().endswith("." + leaf)), None)
        materials = _field(mesh, "Materials") if mesh is not None else None
        meshes[leaf] = {"mesh": _path(mesh), "materials": [_path(m) for m in materials] if materials is not None and not isinstance(materials, str) else materials}
    objects, rendered = [], {}
    for actor in _live("WillowInteractiveObject"):
        component = _component(actor)
        mesh = _path(_field(component, "SkeletalMesh")) if component is not None else None
        if not mesh or not any(mesh.endswith("." + leaf) for leaf in LC_MESHES):
            continue
        material = _try(component.GetMaterial, 0)
        path = _path(material) or str(material)
        objects.append({"path": actor._path_name(), "mesh": mesh, "rendered": path})
        key = f"{mesh.rsplit('.', 1)[-1]} -> {path}"
        rendered[key] = rendered.get(key, 0) + 1
    _write(label, {"meshes": meshes, "objects": objects, "rendered_counts": rendered})
    return {"slots": {k: v["materials"] for k, v in meshes.items()}, "rendered_counts": rendered}


def _vector(x, y, z):
    return unrealsdk.make_struct("Vector", X=x, Y=y, Z=z)


def _aim(target, x, y, z):
    pc = get_pc()  # noqa: F821 - the openwillow_realgame driver defines get_pc in the command scope
    eye = z + (_field(pc.Pawn, "BaseEyeHeight") if isinstance(_field(pc.Pawn, "BaseEyeHeight"), float) else 64.0)
    pitch = math.degrees(math.atan2(target[2] - eye, math.hypot(target[0] - x, target[1] - y)))
    yaw = math.degrees(math.atan2(target[1] - y, target[0] - x))
    # ClientSetRotation: SetRotation kept the pitch at 0 in this build (tools/real_game/scripts/combat_probe.py).
    pc.ClientSetRotation(unrealsdk.make_struct("Rotator", Pitch=int(pitch * 65536 / 360) % 65536,
                                               Yaw=int(yaw * 65536 / 360) % 65536, Roll=0))


def _place(x, y, z, target):
    pawn = get_pc().Pawn  # noqa: F821
    pawn.Location = _vector(x, y, z)   # writing the property moves the pawn; SetLocation fails here (ambient_npcs.py)
    pawn.SetPhysics(2)   # falling: it drops to the floor by itself
    _aim(target, x, y, z)


def lc_goto(x, y, z):
    _place(x + 200.0, y, z + 100.0, (x, y, z))
    return True


def _clear(start, end):
    """FastTrace: True when nothing blocks start to end. A failed call counts as blocked, and is reported by the caller's side."""
    result = _try(get_pc().FastTrace, _vector(*end), _vector(*start))  # noqa: F821
    return result is True


def _stand_at(actor, side, distance=170.0, height=70.0, look_up=45.0):
    loc = actor.Location
    facing = (actor.Rotation.Yaw % 65536) * 360.0 / 65536.0
    a = math.radians(facing + 45.0 * side)
    x, y, z = loc.X + math.cos(a) * distance, loc.Y + math.sin(a) * distance, loc.Z + height
    return (x, y, z), (loc.X, loc.Y, loc.Z + look_up)


def lc_stand(leaf):
    actor = _pick(leaf)
    chosen, clear = 0, False
    for side in range(8):
        spot, target = _stand_at(actor, side)
        if _clear(spot, target) and _clear(spot, (spot[0], spot[1], spot[2] - 300.0)) is False:
            chosen, clear = side, True   # a clear view, and something under the feet
            break
    spot, target = _stand_at(actor, chosen)
    _place(*spot, target)
    LC["side"][leaf] = chosen
    return {"object": actor._path_name(), "side": chosen, "clear": clear}


def lc_side(leaf):
    actor = _pick(leaf)
    side = LC["side"].get(leaf, 0) + 2
    spot, target = _stand_at(actor, side)
    _place(*spot, target)
    return {"object": actor._path_name(), "side": side, "clear": _clear(spot, target)}


def _kind(pickup):
    inventory = _field(pickup, "Inventory")
    data = _field(inventory, "DefinitionData") if inventory is not None else None
    definition = _field(data, "ItemDefinition") if data is not None and not isinstance(data, str) else None
    text = (_path(definition) or "").lower()
    if "ammo" in text or "ammodump" in text:
        return "ammo", text
    if "money" in text or "cash" in text or "currency" in text:
        return "cash", text
    return "other", text


def _near(origin, radius):
    rows = []
    for pickup in _live("WillowPickup"):
        loc = pickup.Location
        distance = math.dist((loc.X, loc.Y, loc.Z), origin)
        if distance <= radius:
            kind, definition = _kind(pickup)
            rows.append({"path": pickup._path_name(), "kind": kind, "definition": definition, "distance": round(distance)})
    return rows


def lc_after(leaf, radius=600.0):
    actor = _pick(leaf)
    loc = actor.Location
    return {"object": actor._path_name(), "pickups": _near((loc.X, loc.Y, loc.Z), radius),
            "usable": _path(_field(get_pc(), "CurrentUsableObject"))}  # noqa: F821


def _ammo_pools():
    pc = get_pc()._path_name()  # noqa: F821
    rows = []
    for pool in _live("ResourcePool"):
        definition = _path(_field(pool, "Definition")) or ""
        provider = _path(_field(pool, "AssociatedProvider")) or ""
        if "ammo" in definition.lower() and pc in provider:
            rows.append(pool)
    return rows


def _pool_values():
    return {_path(_field(p, "Definition")).rsplit(".", 1)[-1]: round(p.CurrentValue, 2) for p in _ammo_pools()}


def _money():
    return _try(lambda: get_pc().PlayerReplicationInfo.GetCurrencyOnHand(0))  # noqa: F821


def lc_lower_ammo():
    before = _pool_values()
    for pool in _ammo_pools():
        _try(pool.SetCurrentValue, pool.CurrentValue * 0.5)
    return {"before": before, "after": _pool_values()}


def lc_stand_pickup(kind, distance=260.0):
    me = get_pc().Pawn.Location  # noqa: F821
    rows = [p for p in _live("WillowPickup") if _kind(p)[0] == kind]
    if not rows:
        return {"kind": kind, "pickup": None}
    pickup = min(rows, key=lambda p: math.dist((p.Location.X, p.Location.Y, p.Location.Z), (me.X, me.Y, me.Z)))
    loc = pickup.Location
    chosen = None
    for side in range(8):
        a = math.radians(45.0 * side)
        spot = (loc.X + math.cos(a) * distance, loc.Y + math.sin(a) * distance, loc.Z + 70.0)
        if _clear(spot, (loc.X, loc.Y, loc.Z + 10.0)):
            chosen = spot
            break
    spot = chosen or (loc.X + distance, loc.Y, loc.Z + 70.0)
    _place(*spot, (loc.X, loc.Y, loc.Z))
    LC["pickup"] = pickup
    return {"kind": kind, "pickup": pickup._path_name(), "definition": _kind(pickup)[1], "clear": chosen is not None,
            "pools": _pool_values(), "money": _money()}


def lc_pickup_state():
    pickup = LC["pickup"]
    gone = pickup is None or _field(pickup, "bDeleteMe") is not False
    return {"gone": gone, "pools": _pool_values(), "money": _money()}
