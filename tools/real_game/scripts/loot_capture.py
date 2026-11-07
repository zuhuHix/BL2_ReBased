"""Lootable and pickup ground truth for the vendors lane (2026-11-06). Run once through Invoke-GamePyFile, by
tools/real_game/loot_capture.ps1. Our own code; what it writes is game data and stays directly under RG_OUT, inside the
repository's ignored local/. Targets are found in the running world: no position or placed-object id is written here.

It changes memory only, with saves blocked by the capture script: it moves the player, and lowers the ammo pools so that a
pickup can be collected.

- lc_discover(label): every placed interactive object (class, definition, balance, mesh, position), with a count per type.
- lc_materials(label): each lootable mesh's Materials[0], and what each placed object's mesh component renders, GetMaterial(0).
- lc_goto(x, y, z): put the player near a position (from the ignored local data), for a type not found from the start.
- lc_stand(mesh, side): stand near the object on that side, keeping its state from the first time. lc_aim(mesh): look at it from where the
  player landed. lc_side(mesh): the same from a quarter turn further round.
- lc_opened(mesh): whether its state fields changed since then. Scalars that moved and the pickups near it go in the record.
- lc_mods(): the SDK mods loaded, for the record. Reads only; nothing is disabled.
- lc_lower_ammo(): halve the ammo pools, recording them.
- lc_choose_pickup(kind): the nearest pickup of that kind ('ammo' or 'cash'). lc_over_pickup(): fall onto it. lc_stand_pickup(kind) and
  lc_aim_pickup(): stand beside it and look at it, for the use key. lc_pickup_baseline(): right before an attempt, that it is still there and
  what the attempt starts from.
- lc_pickup_state(): whether that pickup is gone and the ammo or money rose. Collected only when both.

Nothing here keeps a game object from one command to the next: the game may have moved or destroyed it, and using such a
reference crashed the game once (openwillow_realgame/__init__.py). LC keeps path names, and each command looks the object up afresh.
"""
import json
import math
from pathlib import Path
import re

import unrealsdk

LC_MESHES = ("Skel_DahlChest_Ammo", "Skel_Dumpster", "Skel_StorageLocker", "Skel_StrongBox", "Skel_CardboardBox",
             "Skel_LaundryMachine")
LC_MESH_PACKAGE = "Prop_Lootables_02.Meshes."   # most live here; the read reports a miss rather than guessing
LC = globals().get("LC") or {"picked": {}, "side": {}, "before": {}, "pickup": None}
# The fields on WillowInteractiveObject that describe its state. The second real run saw all three move on every opening and none on a failed try.
OPEN_FIELDS = ("SimpleAnimState", "RepSimpleAnimState", "bCanBeUsed")
# Which ammo pool a pickup fills, matched on the names of its definition and of the pool (read live; only the grenade pair has been seen to move).
AMMO_POOLS = (("grenade", "Grenade"), ("assault_rifle", "Combat_Rifle"), ("shotgun", "Combat_Shotgun"), ("repeater_pistol", "Repeater_Pistol"),
              ("smg", "Patrol_SMG"), ("sniper", "Sniper"), ("rocket", "Rocket"))
SCALAR_KINDS = ("BoolProperty", "ByteProperty", "IntProperty", "FloatProperty", "NameProperty", "StrProperty")


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
    """The placed object with this mesh, looked up afresh: the one this run chose before, else the first by path."""
    rows = sorted((a for a in _live("WillowInteractiveObject") if (_row(a)["mesh"] or "").endswith("." + leaf)), key=lambda a: a._path_name())
    if not rows:
        raise LookupError(f"no placed object with mesh {leaf}")
    chosen = next((a for a in rows if a._path_name() == LC["picked"].get(leaf)), rows[0])
    LC["picked"][leaf] = chosen._path_name()
    return chosen


def lc_materials(label):
    meshes = {}
    for leaf in LC_MESHES:
        mesh = next((m for m in unrealsdk.find_all("SkeletalMesh", False) if m._path_name().endswith("." + leaf)), None)
        materials = _field(mesh, "Materials") if mesh is not None else None
        meshes[leaf] = {"mesh": _path(mesh), "materials": [_path(m) for m in materials] if materials is not None and not isinstance(materials, str) else materials}
    objects, rendered, problems = [], {}, []
    for actor in _live("WillowInteractiveObject"):
        component = _component(actor)
        mesh = _path(_field(component, "SkeletalMesh")) if component is not None else None
        if not mesh or not any(mesh.endswith("." + leaf) for leaf in LC_MESHES):
            continue
        material = _try(component.GetMaterial, 0)
        path = _path(material) or str(material)
        if not _path(material):
            problems.append(f"{actor._path_name()} renders no material: {path}")
        objects.append({"path": actor._path_name(), "mesh": mesh, "rendered": path})
        key = f"{mesh.rsplit('.', 1)[-1]} -> {path}"
        rendered[key] = rendered.get(key, 0) + 1
    # Every mesh that has a placed object must have read a material from its slots. A type with no placed object is its own stage.
    for leaf, row in meshes.items():
        placed = any(item["mesh"].endswith("." + leaf) for item in objects)
        materials = row["materials"]
        if placed and not (isinstance(materials, list) and materials and all(materials)):
            problems.append(f"{leaf} has no readable material slot: {materials}")
    if not objects:
        problems.append("no placed lootable to read")
    _write(label, {"meshes": meshes, "objects": objects, "rendered_counts": rendered, "problems": problems})
    return {"slots": {k: v["materials"] for k, v in meshes.items()}, "rendered_counts": rendered, "problems": problems}


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


def _center(actor):
    """Where the object really is, and how big: its mesh's bounds. A small box on a shelf is not at its actor's origin plus a guess."""
    component = _component(actor)
    bounds = _field(component, "Bounds") if component is not None else None
    origin, radius = _field(bounds, "Origin"), _field(bounds, "SphereRadius")
    if _is_number(_field(origin, "X")) and _is_number(radius) and radius > 0:
        return (origin.X, origin.Y, origin.Z), radius
    loc = actor.Location
    return (loc.X, loc.Y, loc.Z + 45.0), 60.0


def _stand_at(actor, side, height=70.0):
    center, radius = _center(actor)
    facing = (actor.Rotation.Yaw % 65536) * 360.0 / 65536.0
    a = math.radians(facing + 45.0 * side)
    distance = max(170.0, radius + 120.0)
    return (center[0] + math.cos(a) * distance, center[1] + math.sin(a) * distance, actor.Location.Z + height), center


def _state_value(value):
    """A read as something comparable. A failed read is an error string: it is no value, so it can show no change."""
    if isinstance(value, str) and value.startswith("<error"):
        return None
    return value if isinstance(value, (bool, int, float, str)) else str(value)


def _open_state(actor):
    return {name: _state_value(_field(actor, name)) for name in OPEN_FIELDS}


def _scalars(actor, limit=300):
    """Every scalar property the object has, for the record of what moved."""
    out, cls = {}, _field(actor, "Class")
    while cls is not None and not isinstance(cls, str) and len(out) < limit:
        fields = _try(lambda c=cls: list(c._fields()))
        for field in fields if isinstance(fields, list) else []:
            name = _field(field, "Name")
            kind = _field(_field(field, "Class"), "Name")
            if isinstance(name, str) and kind in SCALAR_KINDS and name not in out:
                out[name] = _state_value(_field(actor, name))
        cls = _field(cls, "SuperField")
    return out


def _use_target():
    return _path(_field(get_pc(), "CachedUsableObject"))  # noqa: F821


def lc_stand(leaf, side=0):
    """Near the object on this side (in eighths of a turn from its front). Its state is kept from the first time, so a later side still compares with the start."""
    actor = _pick(leaf)
    LC["before"].setdefault(leaf, {"state": _open_state(actor), "scalars": _scalars(actor)})
    spot, target = _stand_at(actor, side)
    _place(*spot, target)
    LC["side"][leaf] = side
    return {"object": actor._path_name(), "side": side}


def lc_aim(leaf):
    """Once the player has landed: look at the object from where they really are. The fall changes the angle."""
    actor = _pick(leaf)
    me = get_pc().Pawn.Location  # noqa: F821
    center, radius = _center(actor)
    _aim(center, me.X, me.Y, me.Z)
    return {"object": actor._path_name(), "radius": radius, "use_target": _use_target()}


def lc_side(leaf):
    side = LC["side"].get(leaf, 0) + 2
    return lc_stand(leaf, side)


def lc_opened(leaf, radius=600.0):
    """Whether this object changed since the first stand, in the fields that describe its state. Loot nearby is recorded, but is not
    evidence: a neighbour opened by a stray press drops loot too. Every scalar that moved is recorded as well. No change, or an
    unreadable field, is not opened."""
    actor = _pick(leaf)
    before = LC["before"].get(leaf) or {"state": {}, "scalars": {}}
    state = _open_state(actor)
    changed = {name: [before["state"].get(name), value] for name, value in state.items()
               if before["state"].get(name) is not None and value is not None and before["state"].get(name) != value}
    scalars = _scalars(actor)
    moved = {name: [before["scalars"].get(name), value] for name, value in scalars.items() if before["scalars"].get(name) != value}
    loc = actor.Location
    return {"object": actor._path_name(), "opened": bool(changed), "changed": changed, "state": state, "scalars_moved": moved,
            "pickups": _near((loc.X, loc.Y, loc.Z), radius), "use_target": _use_target()}


def lc_mods():
    """Which SDK mods are loaded and enabled, for the record: a beam or icon on loot could be one of theirs. Reads only."""
    def read():
        from mods_base import get_ordered_mod_list
        return [{"name": _field(mod, "name"), "enabled": _field(mod, "is_enabled")} for mod in get_ordered_mod_list()]
    return _try(read)


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
    value = _try(lambda: get_pc().PlayerReplicationInfo.GetCurrencyOnHand(0))  # noqa: F821
    return value[0] if isinstance(value, tuple) and value else value


def _is_number(value):
    return isinstance(value, (int, float)) and not isinstance(value, bool)


def _ammo_pool_of(definition):
    """The part of a pool's name this ammo pickup fills, or None. An unknown type credits no pool."""
    return next((pool for word, pool in AMMO_POOLS if word in definition), None)


def _value_rose(chosen, before, after):
    """Whether collecting this pickup moved what it should: money, or its own ammo pool. A value that could not be read is an error
    string: that is not a rise. Another pool rising says another pickup was touched."""
    if chosen["kind"] == "cash":
        return _is_number(before["money"]) and _is_number(after["money"]) and after["money"] > before["money"]
    pool = chosen.get("pool")
    if not (pool and isinstance(before["pools"], dict) and isinstance(after["pools"], dict)):
        return False
    return any(pool in name and _is_number(value) and _is_number(before["pools"].get(name)) and value > before["pools"][name]
               for name, value in after["pools"].items())


def lc_lower_ammo():
    before = _pool_values()
    for pool in _ammo_pools():
        _try(pool.SetCurrentValue, pool.CurrentValue * 0.5)
    after = _pool_values()
    return {"before": before, "after": after, "fallen": sorted(name for name, value in after.items() if value < before.get(name, value))}


def _chosen_pickup():
    chosen = LC["pickup"]
    return next((p for p in _live("WillowPickup") if chosen and p._path_name() == chosen["path"]), None)


def lc_choose_pickup(kind):
    """The nearest pickup of this kind that is in the open: loot lies inside some containers, where neither a fall nor a clear look reaches it.
    Inside means within the bounds of a lootable, since a line-of-sight trace never answered in this game. The record says which it got.
    Its path and the ammo pool it fills are kept, so that one pickup is judged through both tries."""
    me = get_pc().Pawn.Location  # noqa: F821
    rows = [p for p in _live("WillowPickup") if _kind(p)[0] == kind]
    if not rows:
        LC["pickup"] = None
        return {"kind": kind, "pickup": None}
    lootables = [_center(actor) for actor in _live("WillowInteractiveObject")]

    def inside(p):
        spot = (p.Location.X, p.Location.Y, p.Location.Z)
        return any(math.dist(center, spot) < radius for center, radius in lootables)

    def rank(p):
        return inside(p), math.dist((p.Location.X, p.Location.Y, p.Location.Z), (me.X, me.Y, me.Z))

    pickup = min(rows, key=rank)
    definition = _kind(pickup)[1]
    pool = _ammo_pool_of(definition) if kind == "ammo" else None
    LC["pickup"] = {"path": pickup._path_name(), "kind": kind, "pool": pool, "baseline": None}
    return {"kind": kind, "pickup": pickup._path_name(), "definition": definition, "pool": pool, "in_the_open": not inside(pickup)}


def lc_stand_pickup(kind, distance=150.0):
    """Beside the chosen pickup, looking at it, so the use key can reach it."""
    if not (LC["pickup"] and LC["pickup"]["kind"] == kind) or _chosen_pickup() is None:
        lc_choose_pickup(kind)
    pickup = _chosen_pickup()
    if pickup is None:
        return {"kind": kind, "pickup": None}
    LC["pickup"]["baseline"] = None   # getting here may touch something: the attempt's baseline comes after
    loc = pickup.Location
    _place(loc.X + distance, loc.Y, loc.Z + 70.0, (loc.X, loc.Y, loc.Z))
    return {"kind": kind, "pickup": pickup._path_name()}


def lc_over_pickup():
    """The walk-over: straight above the chosen pickup, falling onto it, so nothing around it can block the way. Whether touching
    collects it is the question."""
    pickup = _chosen_pickup()
    if pickup is None:
        return {"pickup": None}
    loc = pickup.Location
    _place(loc.X, loc.Y, loc.Z + 120.0, (loc.X + 100.0, loc.Y, loc.Z))
    return {"pickup": pickup._path_name()}


def lc_aim_pickup():
    pickup = _chosen_pickup()
    if pickup is None:
        return {"pickup": None}
    me = get_pc().Pawn.Location  # noqa: F821
    loc = pickup.Location
    _aim((loc.X, loc.Y, loc.Z), me.X, me.Y, me.Z)
    return {"pickup": pickup._path_name(), "use_target": _use_target()}


def lc_pickup_baseline():
    """Right before an attempt: that the chosen pickup is still there, and what the attempt starts from. Nothing that moved on the way here
    can then be credited to the attempt."""
    chosen = LC["pickup"]
    if chosen is None:
        return {"present": False, "reason": "no pickup was chosen"}
    chosen["baseline"] = None
    if _chosen_pickup() is None:
        return {"present": False, "reason": "the chosen pickup went before this attempt"}
    chosen["baseline"] = {"pools": _try(_pool_values), "money": _money()}
    return {"present": True, **chosen["baseline"]}


def lc_pickup_state():
    chosen = LC["pickup"]
    if chosen is None:
        return {"gone": False, "changed": False, "collected": False, "reason": "no pickup was chosen"}
    if chosen.get("baseline") is None:
        return {"gone": False, "changed": False, "collected": False, "reason": "no baseline was taken for this attempt"}
    # A pickup whose state cannot be read stays in the list, so an unreadable one never counts as gone.
    gone = not any(p._path_name() == chosen["path"] for p in _live("WillowPickup"))
    after = {"pools": _try(_pool_values), "money": _money()}
    changed = _value_rose(chosen, chosen["baseline"], after)
    return {"gone": gone, "changed": changed, "collected": gone and changed, "pools": after["pools"], "money": after["money"]}
