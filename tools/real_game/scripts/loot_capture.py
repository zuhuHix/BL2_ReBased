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
- lc_ready(mesh, attempt): whether the player's CachedUsableObject is that object, the game's own sign that a press reaches it. If not, one
  nudge for the next read (aim lower, step closer, aim again). lc_next_object(mesh): the next placed object of that type, for a type that never
  offers the prompt.
- lc_opened(mesh): whether its state fields changed since then. Scalars that moved, the bones' world transforms, and the pickups near it (world
  position and the offset in the object's own frame) go in the record.
- lc_view(mesh, which): after an opening, stand at a fixed distance on the side the door turned to ('door') or the other one ('opposite').
  lc_where(mesh): the player's distance from it, for the frame taken from the press stand.
- lc_mods(): the SDK mods loaded, for the record. Reads only; nothing is disabled.
- lc_lower_ammo(): halve the ammo pools, recording them.
- lc_choose_pickup(kind): the nearest reachable pickup of that kind ('ammo' or 'cash'): one dropped onto the floor from an opened container,
  or one loose in the open. One inside a container, or out of reach, is skipped; none left reports "no reachable <kind> pickup".
  lc_over_pickup(): fall onto it. lc_stand_pickup(kind) and
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
LC = globals().get("LC") or {"picked": {}, "side": {}, "before": {}, "door": {}, "pickup": None, "drops": {}}
# Applied in turn after each read of the usable object that is not this one: aim at its lower half, step closer, aim again from where the player
# landed, aim lower again. Host choices; the strong box is small and sits under a shelf.
NUDGES = ("low", "step", "center", "low")
VIEW_DISTANCE = 150.0   # the door-side frame's fixed distance from the mesh's centre, in uu (host choice)
# The fields on WillowInteractiveObject that describe its state. The second real run saw all three move on every opening and none on a failed try.
OPEN_FIELDS = ("SimpleAnimState", "RepSimpleAnimState", "bCanBeUsed")
# Which ammo pool a pickup fills, matched on the names of its definition and of the pool (read live; only the grenade pair has been seen to move).
AMMO_POOLS = (("grenade", "Grenade"), ("assault_rifle", "Combat_Rifle"), ("shotgun", "Combat_Shotgun"), ("repeater_pistol", "Repeater_Pistol"),
              ("smg", "Patrol_SMG"), ("sniper", "Sniper"), ("rocket", "Rocket"))
SCALAR_KINDS = ("BoolProperty", "ByteProperty", "IntProperty", "FloatProperty", "NameProperty", "StrProperty")
# Which pickups a player can reach (host choices, not read from the game). A drop from an opened container counts as on the floor when it
# lies at most this far above the container's origin in the container's own frame: the Z53 cardboard box's clip lay 2.4 uu up, the
# laundry machine's ammo 87 uu up inside it. Whether each type's origin is its floor is UNVERIFIED.
FLOOR_UP = 12.0
# For a loose pickup, the floor is the player's: read from her centre (the pawn's location), it must lie from REACH_BELOW under her to REACH_ABOVE
# over her, and within SEARCH_RADIUS of her. The capsule height is not read, so these bounds are generous and UNVERIFIED. A drop is not
# measured against the player (see lc_choose_pickup).
REACH_BELOW = 130.0
REACH_ABOVE = 10.0
SEARCH_RADIUS = 2000.0   # uu from the player; the nearest reachable pickup inside it is chosen
# A container's frame counts as level when its pitch and roll are each within this many rotator units (65536 is a full turn: 182 is about 1 degree).
TILT_UNITS = 182


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


def _placed(leaf):
    return sorted((a for a in _live("WillowInteractiveObject") if (_row(a)["mesh"] or "").endswith("." + leaf)), key=lambda a: a._path_name())


def _pick(leaf):
    """The placed object with this mesh, looked up afresh: the one this run chose before, else the first by path."""
    rows = _placed(leaf)
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


def _is_lootable(actor):
    """One of the lootable types this capture knows (LC_MESHES), by its skeletal mesh. Any other interactive object is not a container here."""
    mesh = _row(actor)["mesh"] or ""
    return any(mesh.endswith("." + leaf) for leaf in LC_MESHES)


def _center(actor):
    """Where the object really is, and how big: its mesh's bounds. A small box on a shelf is not at its actor's origin plus a guess."""
    component = _component(actor)
    bounds = _field(component, "Bounds") if component is not None else None
    origin, radius = _field(bounds, "Origin"), _field(bounds, "SphereRadius")
    if _is_number(_field(origin, "X")) and _is_number(radius) and radius > 0:
        return (origin.X, origin.Y, origin.Z), radius
    loc = actor.Location
    return (loc.X, loc.Y, loc.Z + 45.0), 60.0


def _facing(actor):
    return (actor.Rotation.Yaw % 65536) * 360.0 / 65536.0


def _stand_at(actor, side, height=70.0, yaw=None, distance=None):
    """A spot round the mesh: eighths of a turn from the object's facing, or an absolute yaw in degrees."""
    center, radius = _center(actor)
    a = math.radians(yaw if yaw is not None else _facing(actor) + 45.0 * side)
    distance = distance if distance is not None else max(170.0, radius + 120.0)
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


def _xyz(v, digits=1):
    return _try(lambda: [round(v.X, digits), round(v.Y, digits), round(v.Z, digits)])


def _transform(actor):
    rot = actor.Rotation
    return {"pos": _xyz(actor.Location), "rot": _try(lambda: [rot.Pitch, rot.Yaw, rot.Roll]), "facing": round(_facing(actor), 1)}


def _tilt(actor):
    """(readable, tilt). tilt is None when pitch and roll are each within TILT_UNITS (65536 units is a full turn, so about 1 degree), else the
    reason. An unreadable rotation is not readable: no drop is tagged from it. _local turns by yaw only, so a tilted container's drop gets an
    approximate use-key frame (VENDORS.md, section 9); the walk-over onto it is refused."""
    rot = actor.Rotation
    pitch, roll = _try(lambda: rot.Pitch), _try(lambda: rot.Roll)
    if not (_is_number(pitch) and _is_number(roll)):
        return False, "rotation unreadable: no drops tagged"
    turn = 65536
    pitch, roll = (pitch + turn // 2) % turn - turn // 2, (roll + turn // 2) % turn - turn // 2   # to a signed angle
    if abs(pitch) > TILT_UNITS or abs(roll) > TILT_UNITS:
        return True, f"tilted: pitch {pitch}, roll {roll} units: tagged for the use key, no walk-over onto it"
    return True, None


def _local(actor, pos):
    """A world position as an offset in the object's own frame (forward, right, up), from its yaw only; pitch and roll are in the record.
    Unlike a distance, this shows a mirrored layout."""
    loc = actor.Location
    dx, dy, dz = pos[0] - loc.X, pos[1] - loc.Y, pos[2] - loc.Z
    a = math.radians(_facing(actor))
    return [round(dx * math.cos(a) + dy * math.sin(a), 1), round(-dx * math.sin(a) + dy * math.cos(a), 1), round(dz, 1)]


def _bones(actor, limit=12):
    """Each bone's world position and rotation, from the mesh component's bone calls (UE3's GetBoneName, GetBoneLocation and
    GetBoneQuaternion, space 0 = world). That this build exposes them is UNVERIFIED; a failed read is recorded, not raised."""
    component = _component(actor)
    out = {}
    for i in range(limit if component is not None else 0):
        name = _try(lambda: component.GetBoneName(i))
        if not isinstance(name, str) or name in ("", "None") or name.startswith("<error"):
            if not out and isinstance(name, str) and name.startswith("<error"):
                out["<read>"] = name
            break
        quat = _try(lambda: component.GetBoneQuaternion(name, 0))
        out[name] = {"pos": _xyz(_try(lambda: component.GetBoneLocation(name, 0)), 2),
                     "quat": _try(lambda: [round(quat.X, 4), round(quat.Y, 4), round(quat.Z, 4), round(quat.W, 4)])}
    return out


def _quat_mul(a, b):
    ax, ay, az, aw = a
    bx, by, bz, bw = b
    return [aw * bx + ax * bw + ay * bz - az * by, aw * by - ax * bz + ay * bw + az * bx,
            aw * bz + ax * by - ay * bx + az * bw, aw * bw - ax * bx - ay * by - az * bz]


def _unit(q):
    length = math.sqrt(sum(c * c for c in q))
    return [c / length for c in q] if length > 0 else None


def _quat_rotate(q, v):
    inverse = [-q[0], -q[1], -q[2], q[3]]
    return _quat_mul(_quat_mul(q, [v[0], v[1], v[2], 0.0]), inverse)[:3]


def _door(before, after, origin, facing):
    """The side the door swung to. Run Z53: the strong box's door bone sits on the hinge, so its position hardly moves (0.02 uu) while its
    rotation turns (about 125 degrees); a position test found nothing. So: take the bone whose rotation turned most (over 10 degrees),
    turn the object's origin about that bone by the same turn, and take the floor direction it moved in. A point behind a door that swings
    out moves outwards, whatever the angle. That direction is snapped to the nearest quarter turn of the object's facing, since the
    sides of a box are. None, so the view falls back to the facing, if no bone turned, none could be read, two bones turned the most
    alike (1 degree apart or less: no single door), or the origin moved 1 uu or less across the floor (no direction to read)."""
    turns = []
    for name, now in after.items():
        then = before.get(name) if isinstance(before, dict) else None
        if not all(isinstance(x, dict) and isinstance(x.get("quat"), list) and isinstance(x.get("pos"), list) for x in (then, now)):
            continue
        q0, q1 = _unit(then["quat"]), _unit(now["quat"])   # the record rounds them to 4 places
        if q0 is None or q1 is None:
            continue
        turn = _quat_mul(q1, [-q0[0], -q0[1], -q0[2], q0[3]])
        turns.append((math.degrees(2.0 * math.acos(min(1.0, abs(turn[3])))), name, turn, then["pos"]))
    turns.sort(key=lambda t: t[0], reverse=True)
    if not turns or turns[0][0] <= 10.0 or (len(turns) > 1 and round(turns[0][0] - turns[1][0], 3) <= 1.0):
        return None
    angle, name, turn, hinge = turns[0]
    arm = [origin[i] - hinge[i] for i in range(3)]
    moved = _quat_rotate(turn, arm)
    dx, dy = moved[0] - arm[0], moved[1] - arm[1]
    if math.hypot(dx, dy) <= 1.0:
        return None
    raw = math.degrees(math.atan2(dy, dx))
    yaw = facing + 90.0 * round(((raw - facing) % 360.0) / 90.0)
    return {"bone": name, "turned": round(angle, 1), "raw_yaw": round(raw % 360.0, 1), "yaw": round(yaw % 360.0, 1)}


def lc_stand(leaf, side=0):
    """Near the object on this side (in eighths of a turn from its front). Its state is kept from the first time, so a later side still compares with the start."""
    actor = _pick(leaf)
    LC["before"].setdefault(leaf, {"state": _open_state(actor), "scalars": _scalars(actor), "bones": _bones(actor)})
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


def lc_ready(leaf, attempt=0):
    """Whether the player's CachedUsableObject is this object. Two real runs: it was the object on all five tries that opened and empty on all
    seven that did not, so the capture presses only when it is. If not, one nudge (NUDGES, in turn) for the next read."""
    actor = _pick(leaf)
    target = _use_target()
    row = {"object": actor._path_name(), "attempt": attempt, "use_target": target, "usable": target == actor._path_name()}
    if row["usable"]:
        # The pickups lying here just before the press. A drop is one that appears after the press: lc_opened tags only those.
        LC.setdefault("pre_press", {})[leaf] = sorted(p._path_name() for p in _live("WillowPickup"))
    if not row["usable"]:
        # Run Z53: the storage locker's field stayed empty on all 20 reads, where run 2 read the locker itself from the same spot. Its own
        # state and the player's distance go in the record, so the next empty read can be told apart from an already opened object.
        me = get_pc().Pawn.Location  # noqa: F821
        row["state"] = _open_state(actor)
        row["distance"] = round(math.dist((me.X, me.Y, me.Z), _center(actor)[0]))
        nudge = NUDGES[attempt % len(NUDGES)]
        center, radius = _center(actor)
        goal = (center[0], center[1], center[2] - 0.5 * radius) if nudge == "low" else center
        if nudge == "step":
            yaw = math.degrees(math.atan2(me.Y - center[1], me.X - center[0]))
            spot, _ = _stand_at(actor, 0, yaw=yaw, distance=max(90.0, radius + 40.0))
            _place(*spot, goal)
        else:
            _aim(goal, me.X, me.Y, me.Z)
        row["next"] = nudge
    return row


def lc_where(leaf):
    """Where the player stands now, and how far that is from the mesh's centre: the current-angle frame is taken from the press stand."""
    actor = _pick(leaf)
    me = get_pc().Pawn.Location  # noqa: F821
    center = _center(actor)[0]
    return {"object": actor._path_name(), "player": _xyz(me), "distance": round(math.dist((me.X, me.Y, me.Z), center)),
            "floor_distance": round(math.hypot(me.X - center[0], me.Y - center[1]))}


def lc_next_object(leaf):
    """Move on to the next placed object of this type by path, with a fresh closed state. None when there is no further one."""
    paths = [a._path_name() for a in _placed(leaf)]
    current = LC["picked"].get(leaf)
    later = [p for p in paths if current is None or p > current]
    if not later:
        return {"object": None, "count": len(paths)}
    LC["picked"][leaf] = later[0]
    LC["before"].pop(leaf, None)
    LC["door"].pop(leaf, None)
    return {"object": later[0], "count": len(paths)}


def lc_view(leaf, which="door"):
    """After an opening: stand VIEW_DISTANCE from the mesh's centre on the side the lid moved to, or opposite it. Without a bone that moved,
    the object's own facing stands in, and the record says so. lc_aim then looks at it from where the player landed."""
    actor = _pick(leaf)
    door = LC["door"].get(leaf)
    yaw = door["yaw"] if door else _facing(actor)
    if which == "opposite":
        yaw += 180.0
    spot, target = _stand_at(actor, 0, yaw=yaw, distance=VIEW_DISTANCE)
    _place(*spot, target)
    return {"object": actor._path_name(), "view": which, "yaw": round(yaw % 360.0, 1), "from": "lid bone" if door else "object facing",
            "transform": _transform(actor)}


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
    bones = _bones(actor)
    door = _door(before.get("bones"), bones, (loc.X, loc.Y, loc.Z), _facing(actor)) if changed else None
    if door:
        LC["door"][leaf] = door
    # The pickups before the press (lc_ready's snapshot, taken once per try); none means no drop is tagged.
    pre = LC.get("pre_press", {}).pop(leaf, None)
    pickups = _near((loc.X, loc.Y, loc.Z), radius, actor)
    readable, tilt = _tilt(actor)
    drops = LC.setdefault("drops", {})
    tagged = []
    # Only loot that appeared after this object's press, and that no earlier container already owns, is its drop. Its frame says where on it the
    # loot lies: yaw only, so a tilted object's drop is approximate and carries its tilt, which refuses the walk-over onto it.
    if changed and pre is not None and readable:
        for row in pickups:
            path = row["path"]
            if path in pre or path in drops or not isinstance(row.get("local"), list):
                continue
            drops[path] = {"container": actor._path_name(), "local": row["local"], "floor_z": round(loc.Z, 1), "tilt": tilt}
            tagged.append(path)
    return {"object": actor._path_name(), "opened": bool(changed), "changed": changed, "state": state, "scalars_moved": moved,
            "transform": _transform(actor), "bones_closed": before.get("bones"), "bones": bones, "door": door,
            "pickups": pickups, "drops_tagged": tagged, "drop_frame": tilt if readable else "rotation unreadable: no drops tagged",
            "use_target": _use_target()}


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
    if "eridium" in text:   # its definition sits under gd_currency too, but it moves no money (Z53c chose one as cash)
        return "eridium", text
    if "money" in text or "cash" in text or "currency" in text:
        return "cash", text
    return "other", text


def _near(origin, radius, frame=None):
    """Pickups within radius: world position and distance, and with a frame object the offset in that object's own frame."""
    rows = []
    for pickup in _live("WillowPickup"):
        loc = pickup.Location
        pos = (loc.X, loc.Y, loc.Z)
        distance = math.dist(pos, origin)
        if distance <= radius:
            kind, definition = _kind(pickup)
            row = {"path": pickup._path_name(), "kind": kind, "definition": definition, "distance": round(distance),
                   "pos": [round(v, 1) for v in pos]}
            if frame is not None:
                row["local"] = _try(_local, frame, pos)
            rows.append(row)
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
    """The nearest pickup of this kind that the player can reach. Two kinds count: one dropped from an opened container that lies on the floor
    in the container's frame (FLOOR_UP), and one loose in the open, outside every lootable's bounds. Loot still inside a container is skipped:
    the Z53 ammo lay 87 uu up inside a laundry machine, where neither a fall nor the use key reached it. A loose pickup must also lie in the band
    a standing player reaches (REACH_BELOW, REACH_ABOVE) and within SEARCH_RADIUS; a drop is judged in its container's frame alone. Inside means within the bounds of a lootable, since a
    line-of-sight trace never answered in this game. Its path and the ammo pool it fills are kept, so that one pickup is judged through both
    tries."""
    me = get_pc().Pawn.Location  # noqa: F821
    here = (me.X, me.Y, me.Z)
    rows = [p for p in _live("WillowPickup") if _kind(p)[0] == kind]
    # The sphere is a conservative approximation of a container's bounds: it may hold a loose pickup near a container out, which fails closed.
    lootables = [_center(actor) for actor in _live("WillowInteractiveObject") if _is_lootable(actor)]
    skipped = {"inside_container": 0, "above_reach": 0, "too_far": 0}
    reachable = []
    for p in rows:
        spot = (p.Location.X, p.Location.Y, p.Location.Z)
        distance = math.dist(spot, here)
        drop = LC.get("drops", {}).get(p._path_name())
        if drop is not None:
            # Judged in its container's frame only: lc_stand_pickup moves the player beside it, so where she stands now says nothing about
            # its floor (run Z54: the cardboard box's clip, 2.4 uu above its box, was refused while she stood on another level).
            if drop["local"][2] > FLOOR_UP:
                skipped["inside_container"] += 1
                continue
            reach, container, local = "dropped on the floor", drop["container"], drop["local"]
        elif any(math.dist(center, spot) < radius for center, radius in lootables):
            skipped["inside_container"] += 1
            continue
        else:
            # Loose: its floor is the player's, so it must lie in her reach and near her.
            if not (-REACH_BELOW <= spot[2] - here[2] <= REACH_ABOVE):
                skipped["above_reach"] += 1
                continue
            if distance > SEARCH_RADIUS:
                skipped["too_far"] += 1
                continue
            reach, container, local = "loose in the open", None, None
        reachable.append((distance, p, reach, container, local))
    if not reachable:
        LC["pickup"] = None
        return {"kind": kind, "pickup": None, "reason": f"no reachable {kind} pickup", "seen": len(rows), "skipped": skipped}
    # The nearest eligible pickup, a drop or not: a far drop must not outrank a nearer loose one.
    _, pickup, reach, container, local = min(reachable, key=lambda row: row[0])
    definition = _kind(pickup)[1]
    pool = _ammo_pool_of(definition) if kind == "ammo" else None
    LC["pickup"] = {"path": pickup._path_name(), "kind": kind, "pool": pool, "baseline": None, "walk_over": None}
    return {"kind": kind, "pickup": pickup._path_name(), "definition": definition, "pool": pool, "reach": reach,
            "pos": [round(v, 1) for v in (pickup.Location.X, pickup.Location.Y, pickup.Location.Z)], "container": container,
            "container_local": local, "pawn_z": round(me.Z, 1), "seen": len(rows), "skipped": skipped}


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
    collects it is the question. Only onto a floor that was read and lies within the pickup's reach: a drop's container origin
    (floor_z, lc_opened). Anything else, a loose pickup included, skips the fall and says why; the use key still runs."""
    pickup = _chosen_pickup()
    if pickup is None:
        return {"pickup": None}
    loc = pickup.Location
    drop = LC.get("drops", {}).get(pickup._path_name())
    floor = drop.get("floor_z") if drop is not None else None
    if drop is None:
        reason = "no floor under the target was read (not a container's drop)"
    elif drop.get("tilt"):
        reason = "the container is tilted, so its floor is not judged: no fall onto its drop"
    elif not _is_number(floor):
        reason = "the container's floor was not read"
    elif not (loc.Z - REACH_BELOW <= floor <= loc.Z + REACH_ABOVE):
        reason = "the floor under the target is outside the pickup's reach"
    else:
        reason = None
    if reason is not None:
        LC["pickup"]["walk_over"] = f"skipped: {reason}"
        return {"pickup": pickup._path_name(), "walk_over": LC["pickup"]["walk_over"]}
    LC["pickup"]["walk_over"] = "fall"
    _place(loc.X, loc.Y, loc.Z + 120.0, (loc.X + 100.0, loc.Y, loc.Z))
    return {"pickup": pickup._path_name(), "walk_over": "fall"}


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
    return {"gone": gone, "changed": changed, "collected": gone and changed, "pools": after["pools"], "money": after["money"],
            "walk_over": chosen.get("walk_over")}
