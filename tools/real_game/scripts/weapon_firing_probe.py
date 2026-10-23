"""Live firing-definition dump and observed-shot probe (firing lane, real game).

Run through Invoke-GamePyFile after the driver is up and saves are blocked (block_saves()). Loading the
file only defines commands; nothing runs until one is called. Game-derived output goes under RG_OUT
(normally local/realgame/firing/, ignored). Nothing here writes into a tracked file.

Definitions (Task A; works at the main menu):
  fp_defs()             dump the firing definition classes to <RG_OUT>/defs/<Class>.json, plus counts.
  fp_discover()         firing-vocabulary class census (names only).
  fp_props(path)        every readable property of one object.

Observed shots (Task B/C; needs a loaded level with the player pawn):
  fp_block()            call block_saves() if the driver has not already done it this session.
  fp_spawn(recipe_id)   create the exact part list from a recipe in the backpack (once; reuses a match).
  fp_ready(recipe_id, slot)   hold it in quick slot 1..4 (moves the slot's old gun to the backpack).
  fp_face_wall()        turn to the flattest wall 2.5-5.5 m ahead and write its pose; fp_reface() re-applies it.
  fp_install_hooks()    add the fire / projectile / damage / status hooks once per session.
  fp_run_begin(name)    start a run folder under <RG_OUT>/shots/<name>/ and log the view.
  fp_run_end()          stop logging; returns the run's event counts.
  fp_cleanup()          remove only guns this probe spawned (never one that is held).

Every observed value is logged with two clocks: perf_counter (QueryPerformanceCounter, the Stopwatch clock
used by realgame.ps1) and the game's own WorldInfo.TimeSeconds (game_t). The PowerShell side drives the
keys and clicks; the Python side only reads and logs.

Objects are looked up again in every command. A reference kept from an earlier command can point at a
destroyed object and crash the game (see openwillow_realgame/__init__.py).
"""
import enum
import json
import math
import sys
import time
from pathlib import Path

try:
    import unrealsdk
    from unrealsdk.hooks import Type, add_hook, remove_hook
except ImportError:  # keeps the module importable outside the game
    unrealsdk = None
    Type = add_hook = remove_hook = None


DEF_CLASSES = (
    "FiringModeDefinition",
    "ProjectileDefinition",
    "ExplosionDefinition",
    "ExplosionCollectionDefinition",
    "WillowExplosionImpactDefinition",
    "WillowImpactDefinition",
    "StatusEffectDefinition",
)
FIRING_TERMS = ("firing", "projectile", "explosion", "impact", "statuseffect", "behavior_", "burst", "spin",
                "tracer", "muzzle", "damagetype", "dot", "spread", "recoil", "kick")
BEHAVIOR_PREFIXES = ("Behavior_SpawnProjectile", "ProjectileBehavior_")
ATTR_TERMS = ("projectile", "spread", "burst", "firerate", "fire_rate", "fireinterval", "status", "damage",
              "explosion", "kick", "recoil", "spin", "accuracy", "chance", "radius")
MAX_DEPTH = 8
MAX_ARRAY = 512
PART_EXTRA_FIELDS = ("CustomFiringModeDefinition", "CustomDamageTypeDefinition", "WeaponAttributeEffects",
                     "ZoomWeaponAttributeEffects", "ExternalAttributeEffects", "AttributeSlotEffects",
                     "AttributeSlotUpgrades", "MuzzleFlashPSTemplates", "TracerTemplate", "BehaviorProviderDefinition")
WEAPON_LEVEL_FIELDS = ("FireRate", "FireInterval", "Spread", "ProjectilesPerShot", "ClipSize", "ReloadTime",
                       "InstantHitDamage", "StatusEffectDamage", "BaseStatusEffectChanceModifier")
SLOTS = ("Body", "Grip", "Barrel", "Sight", "Stock", "Elemental", "Accessory1", "Accessory2", "Material")

# Hook specs: (owning class, own function names, log kind, PRE or POST).
HOOK_SPECS = (
    ("WillowWeapon", ("FireAmmunition", "ExtraFireAmmunition", "BurstDelayComplete",
                      "FireOnRelease", "Behavior_Fire"), "fire", "PRE"),
    # POST: the trace and impact list are filled in only after the firing mode has run.
    ("WillowWeapon", ("FiringModeDefinitionFire",), "fire_post", "POST"),
    ("WillowProjectile", ("Init", "InitWithVelocity", "InitializeFromDefinition", "InitializeExplosion", "Explode",
                          "DetonateProjectile", "HitWall", "DoHitWall", "HandleTouch", "OnDamagedEnemy"),
     "projectile", "PRE"),
    ("WillowPawn", ("TakeDamage", "TakeRadiusDamage", "ReactToPrimaryStatusEffect"), "pawn", "PRE"),
)
HOOK_ID = "ow_weapon_firing_probe"
TICK_PATH = "WillowGame.WillowGameViewportClient:Tick"

fp_state = globals().setdefault("fp_state", {
    "recipes": {}, "spawned": set(), "run": None, "hooked": [], "wall": None,
    "proj_seen": {}, "proj_ids": {}, "hooks_on": False,
})


# ---------------------------------------------------------------- shared helpers

def _path(value):
    if value is None:
        return None
    try:
        return value._path_name()
    except Exception:
        return value if isinstance(value, str) else None


def _is_uobject(value):
    return value is not None and hasattr(value, "_path_name") and hasattr(value, "Class")


def _is_wrapped_struct(value):
    try:
        return callable(value._type._properties)
    except Exception:
        return False


def _is_wrapped_array(value):
    return type(value).__name__.lower().startswith("wrappedarray")


def deep(value, depth=0):
    """Deep, JSON-safe value. Enums by name only; SDK attribute lookup is case-insensitive, so getattr on
    a UObject or struct for "name" would return its Name property (weapon_parity_probe.py notes this)."""
    if isinstance(value, enum.Enum):
        return value.name
    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    if _is_uobject(value):
        return _path(value)
    if depth >= MAX_DEPTH:
        return repr(value)[:200]
    if isinstance(value, dict):
        return {str(k): deep(v, depth + 1) for k, v in value.items()}
    if _is_wrapped_struct(value):
        result = {}
        for prop in value._type._properties():
            try:
                result[prop.Name] = deep(getattr(value, prop.Name), depth + 1)
            except Exception:
                continue
        return result
    if isinstance(value, (list, tuple)) or _is_wrapped_array(value):
        try:
            items = list(value)
        except Exception:
            return repr(value)[:200]
        out = [deep(item, depth + 1) for item in items[:MAX_ARRAY]]
        if len(items) > MAX_ARRAY:
            out.append({"_truncated_at": MAX_ARRAY, "_length": len(items)})
        return out
    return repr(value)[:200]


_FIELD_CACHE = {}


def _field_names(obj):
    """Property names of an object's class and its superclasses (UObjects expose fields via .Class)."""
    try:
        cls = obj.Class
        key = cls._path_name()
    except Exception:
        return []
    if key not in _FIELD_CACHE:
        names = []
        walk = cls
        while walk:
            try:
                for field in walk._fields():
                    if getattr(getattr(field, "Class", None), "Name", "").endswith("Property") and field.Name not in names:
                        names.append(field.Name)
            except Exception:
                pass
            walk = getattr(walk, "SuperField", None)
        _FIELD_CACHE[key] = names
    return _FIELD_CACHE[key]


def _all_props(obj):
    """Every readable property of a UObject (inherited ones included); unreadable ones are skipped."""
    result = {}
    for name in _field_names(obj):
        try:
            value = getattr(obj, name)
        except Exception:
            continue
        if callable(value):
            continue
        result[name] = deep(value)
    return result


def _selected_props(obj, names):
    result = {}
    for name in names:
        try:
            value = getattr(obj, name)
        except Exception:
            continue
        if callable(value):
            continue
        result[name] = deep(value)
    return result


def _out(*parts):
    folder = Path(RG_OUT).joinpath(*parts)
    folder.mkdir(parents=True, exist_ok=True)
    return folder


def _write_json(path, data):
    path.write_text(json.dumps(data, indent=1, ensure_ascii=False, sort_keys=True), encoding="utf-8")


def _repo_root():
    # RG_OUT may be local/realgame/firing or local/realgame/firing/r2: walk up to the repository root.
    for parent in Path(RG_OUT).resolve().parents:
        if (parent / "tools" / "real_game").is_dir():
            return parent
    raise RuntimeError("repository root not found above " + str(RG_OUT))


def _get_pawn():
    pc = get_pc()
    if pc is None or pc.Pawn is None:
        raise RuntimeError("no player pawn; load a level first")
    return pc, pc.Pawn


def _game_t():
    pc = get_pc()
    try:
        return float(pc.WorldInfo.TimeSeconds)
    except Exception:
        return None


# ---------------------------------------------------------------- Task A: definitions

def _class_census():
    """Every class name in the game that matches the firing vocabulary (no instances read)."""
    names = {}
    for cls in unrealsdk.find_all("Class", exact=False):
        name = cls.Name
        if any(term in name.lower() for term in FIRING_TERMS):
            names[name] = cls._path_name()
    return dict(sorted(names.items()))


def _behavior_classes():
    return {name: path for name, path in _class_census().items()
            if name.startswith(BEHAVIOR_PREFIXES) or name == "Behavior_SpawnProjectile"}


def _dump_instances(class_name, row_fn, exact=False):
    rows = {}
    for obj in unrealsdk.find_all(class_name, exact=exact):
        try:
            rows[_path(obj)] = {"class": obj.Class.Name, "properties": row_fn(obj)}
        except Exception as error:
            rows[_path(obj) or repr(obj)[:80]] = {"class": class_name, "error": repr(error)[:200]}
    return rows


def _part_row(obj):
    """Weapon part: the firing-relevant fields in full, and attribute effects filtered by term."""
    row = _selected_props(obj, PART_EXTRA_FIELDS)
    effect_fields = ("WeaponAttributeEffects", "ZoomWeaponAttributeEffects", "ExternalAttributeEffects")
    kept = {}
    for field in effect_fields:
        if not isinstance(row.get(field), list):
            continue
        entries = [e for e in row[field] if any(t in json.dumps(e, ensure_ascii=False).lower() for t in ATTR_TERMS)]
        if entries:
            kept[field] = entries
    out = {k: v for k, v in row.items() if k not in effect_fields}
    out.update(kept)
    return out


def fp_discover():
    census = _class_census()
    return {"count": len(census), "classes": census}


def fp_defs():
    """Task A: dump the firing definitions. Writes <RG_OUT>/defs/<Class>.json and defs/_counts.json."""
    folder = _out("defs")
    counts = {}
    for class_name in DEF_CLASSES:
        rows = _dump_instances(class_name, _all_props)
        _write_json(folder / (class_name + ".json"), rows)
        counts[class_name] = len(rows)

    behaviors = _behavior_classes()
    for class_name in sorted(behaviors):
        rows = _dump_instances(class_name, _all_props)
        if rows:
            _write_json(folder / (class_name + ".json"), rows)
            counts[class_name] = len(rows)

    rows = _dump_instances("WeaponTypeDefinition", _all_props)
    _write_json(folder / "WeaponTypeDefinition.json", rows)
    counts["WeaponTypeDefinition"] = len(rows)

    rows = _dump_instances("WeaponPartDefinition", _part_row)
    _write_json(folder / "WeaponPartDefinition_firing.json", rows)
    counts["WeaponPartDefinition"] = len(rows)
    custom_fire = sum(1 for r in rows.values() if r.get("properties", {}).get("CustomFiringModeDefinition"))
    custom_dmg = sum(1 for r in rows.values() if r.get("properties", {}).get("CustomDamageTypeDefinition"))

    census = _class_census()
    _write_json(folder / "_class_census.json", census)
    summary = {"counts": counts, "behavior_classes": sorted(behaviors),
               "parts_with_custom_firing_mode": custom_fire, "parts_with_custom_damage_type": custom_dmg,
               "census_classes": len(census)}
    _write_json(folder / "_counts.json", summary)
    return summary


def fp_props(path):
    """All readable properties of one object (spot check)."""
    obj = unrealsdk.find_object("Object", path)
    if obj is None:
        return {"error": "not found", "path": path}
    return {"class": obj.Class.Name, "properties": _all_props(obj)}


# ---------------------------------------------------------------- Task B/C: guns in hand

def fp_block():
    """Refuse save writes for this session (the driver's block_saves). Idempotent for hook setup."""
    blocker = globals().get("block_saves")
    if not callable(blocker):
        raise RuntimeError("driver block_saves() missing; the driver must be installed")
    return blocker()


def _recipe_file(recipe_id):
    root = _repo_root() / "local" / "items"
    for candidate in (root / "slice" / (recipe_id + ".json"), root / (recipe_id + ".json"),
                      root / "firing" / "coverage" / (recipe_id + ".json")):
        if candidate.is_file():
            return candidate
    raise FileNotFoundError("no local recipe for " + recipe_id)


def _recipe(recipe_id):
    if recipe_id not in fp_state["recipes"]:
        fp_state["recipes"][recipe_id] = json.loads(_recipe_file(recipe_id).read_text(encoding="utf-8"))
    return fp_state["recipes"][recipe_id]


def _definition_fields(recipe):
    """WeaponDefinitionData fields from one local recipe (same rule as weapon_parity_probe.definition_fields)."""
    required = {
        "WeaponTypeDefinition": recipe.get("weapon_type"),
        "BalanceDefinition": recipe.get("balance"),
        "ManufacturerDefinition": recipe.get("manufacturer"),
    }
    missing = [key for key, value in required.items() if not value]
    if missing:
        raise ValueError("recipe is missing: " + ", ".join(missing))
    stage = recipe.get("game_stage", recipe.get("level", 8))
    fields = dict(required)
    fields["GameStage"] = int(stage)
    fields["ManufacturerGradeIndex"] = int(recipe.get("manufacturer_grade_index", recipe.get("grade", stage)))
    for slot in SLOTS:
        value = recipe.get("parts", {}).get(slot)
        if isinstance(value, dict):
            value = value.get("part")
        if value:
            fields[slot + "PartDefinition"] = value
    for recipe_key, field in (("prefix", "PrefixPartDefinition"), ("title", "TitlePartDefinition")):
        value = recipe.get(recipe_key)
        if isinstance(value, dict):
            value = value.get("part")
        if value:
            fields[field] = value
    return fields


def _inventory():
    im = get_pc().GetPawnInventoryManager()
    chain, item = [], im.InventoryChain
    while item:
        chain.append(item)
        item = item.Inventory
    equipped = [w for w in chain if w.Class.Name == "WillowWeapon"]
    backpack = [w for w in im.Backpack if w.Class.Name == "WillowWeapon"]
    return im, equipped, backpack


def _matches(weapon, recipe):
    try:
        data = weapon.DefinitionData
    except Exception:
        return False
    for key, wanted in _definition_fields(recipe).items():
        try:
            got = getattr(data, key)
        except Exception:
            return False
        if key.endswith("Definition"):
            if _path(got) != wanted:
                return False
        elif got != wanted:
            return False
    return True


def _loose(weapon, recipe):
    """Balance, type, manufacturer and slot parts only. Used when the exact check fails after the game has
    changed the stage or grade of a gun on spawn or readying (seen for the mission pistol and Infinity)."""
    try:
        data = weapon.DefinitionData
    except Exception:
        return False
    for key, wanted in _definition_fields(recipe).items():
        # The game re-rolls the name prefix and title from the parts; they do not change stats.
        if not key.endswith("Definition") or key in ("PrefixPartDefinition", "TitlePartDefinition"):
            continue
        try:
            if _path(getattr(data, key)) != wanted:
                return False
        except Exception:
            return False
    return True


def _slot(weapon):
    try:
        return int(weapon.QuickSelectSlot)
    except Exception:
        return 0


def _find_recipe(recipe_id, loose_ok=True):
    recipe = _recipe(recipe_id)
    _im, equipped, backpack = _inventory()
    found = next((w for w in equipped + backpack if _matches(w, recipe)), None)
    if found is None and loose_ok:
        found = next((w for w in equipped + backpack if _loose(w, recipe)), None)
    return found


def fp_spawn(recipe_id):
    """Create the recipe's exact gun in the backpack unless one already matches. Returns what it found."""
    recipe = _recipe(recipe_id)
    found = _find_recipe(recipe_id)
    if found is not None:
        return {"recipe": recipe_id, "created": False, "weapon": _path(found)}
    im, _eq, backpack = _inventory()
    before = {_path(w) for w in backpack}
    values = {}
    for key, value in _definition_fields(recipe).items():
        if isinstance(value, str) and key.endswith("Definition"):
            obj = unrealsdk.find_object("Object", value)
            if obj is None:
                raise LookupError("could not resolve " + key + ": " + value)
            values[key] = obj
        else:
            values[key] = value
    im.AddBackpackWeaponFromDefinitionData(unrealsdk.make_struct("WeaponDefinitionData", **values))
    fp_state["spawned"].add(recipe_id)  # owned by this probe from here on, even if the match below fails
    found = _find_recipe(recipe_id)
    if found is None:
        new = [_path(w) for w in _inventory()[2] if _path(w) not in before]
        raise RuntimeError("exact recipe did not appear: " + recipe_id + " new=" + repr(new))
    return {"recipe": recipe_id, "created": True, "weapon": _path(found)}


def fp_ready(recipe_id, slot=1):
    """Hold the recipe's gun in quick slot 1..4 (its equip animation takes about a second)."""
    slot = int(slot)
    if slot < 1 or slot > 4:
        raise ValueError("slot must be 1..4")
    weapon = _find_recipe(recipe_id)
    if weapon is None:
        raise LookupError("spawn it first: " + recipe_id)
    held = get_pc().Pawn.Weapon
    if held is not None and _path(held) == _path(weapon):
        return {"recipe": recipe_id, "held": True, "already": True}
    if held is not None and _slot(held) == slot:
        raise RuntimeError("switch away from slot %d before replacing it" % slot)
    if _slot(weapon) > 0:
        get_pc().GetPawnInventoryManager().SendSlottedThingToBackpack(_slot(weapon))
    get_pc().GetPawnInventoryManager().SendSlottedThingToBackpack(slot)
    weapon = next((w for w in get_pc().GetPawnInventoryManager().Backpack
                   if w.Class.Name == "WillowWeapon" and (_matches(w, _recipe(recipe_id))
                                                          or _loose(w, _recipe(recipe_id)))), None)
    if weapon is None:
        raise LookupError("gun not in backpack after clearing slot %d" % slot)
    get_pc().GetPawnInventoryManager().ReadyBackpackInventory(weapon, slot)
    ready = _find_recipe(recipe_id)
    return {"recipe": recipe_id, "slot": slot, "weapon": _path(ready) if ready else None,
            "now_in_slot": _slot(ready) if ready else None}


def fp_cleanup():
    """Remove only guns this probe spawned. A held one is switched away from first (the caller's job)."""
    removed = []
    for recipe_id in sorted(fp_state["spawned"]):
        weapon = _find_recipe(recipe_id)
        if weapon is None:
            fp_state["spawned"].discard(recipe_id)
            continue
        held = get_pc().Pawn.Weapon
        if held is not None and _path(held) == _path(weapon):
            raise RuntimeError("switch away from this probe weapon before cleanup: " + recipe_id)
        if _slot(weapon) > 0:
            get_pc().GetPawnInventoryManager().SendSlottedThingToBackpack(_slot(weapon))
        im, _eq, backpack = _inventory()
        weapon = next((w for w in backpack if _matches(w, _recipe(recipe_id)) or _loose(w, _recipe(recipe_id))), None)
        if weapon is None:
            raise RuntimeError("probe weapon not in backpack for cleanup: " + recipe_id)
        path = _path(weapon)
        im.RemoveInventoryFromBackpack(weapon)
        weapon.Destroy()
        removed.append(recipe_id + " " + path)
        fp_state["spawned"].discard(recipe_id)
    return {"removed": removed, "remaining": sorted(fp_state["spawned"])}


def _vec(x, y, z):
    return unrealsdk.make_struct("Vector", X=float(x), Y=float(y), Z=float(z))


def _hit_distance(pawn, start, yaw, pitch=0.0, max_dist=1500.0):
    """Distance along (yaw, pitch) degrees to world geometry by bisecting Actor.FastTrace; None if clear."""
    dx = math.cos(math.radians(pitch)) * math.cos(math.radians(yaw))
    dy = math.cos(math.radians(pitch)) * math.sin(math.radians(yaw))
    dz = math.sin(math.radians(pitch))
    begin = _vec(*start)

    def clear(dist):
        return bool(pawn.FastTrace(_vec(start[0] + dx * dist, start[1] + dy * dist, start[2] + dz * dist), begin))

    if clear(max_dist):
        return None
    low, high = 0.0, max_dist
    for _ in range(14):
        mid = (low + high) / 2
        if clear(mid):
            low = mid
        else:
            high = mid
    return high


def _set_view(yaw, pitch=0.0):
    get_pc().SetRotation(unrealsdk.make_struct("Rotator", Pitch=int(pitch * 65536 / 360) & 0xFFFF,
                                               Yaw=int(yaw * 65536 / 360) & 0xFFFF, Roll=0))


def fp_face_wall(min_dist=250.0, max_dist=550.0, step=5.0, max_residual=4.0, flat_frac=0.05):
    """Face the flattest wall whose perpendicular distance from the eye is in [min_dist, max_dist] uu.

    Same method as weapon_parity_probe.wp_face_wall: sweep yaw at eye height with FastTrace, fit a line through
    three hits and turn to its normal. max_residual (uu) and flat_frac relax the flatness test for rough
    surfaces; the pose records the residual actually measured. Records the pose (eye, yaw, the aim hit point
    and the wall distance along the aim ray) in fp_state and writes <RG_OUT>/shots/pose.json.
    """
    pc, pawn = _get_pawn()
    loc = pawn.Location
    start = (loc.X, loc.Y, loc.Z + float(getattr(pawn, "BaseEyeHeight", 60.0)))
    best = None
    yaw = 0.0
    while yaw < 360.0:
        points = []
        for offset in (-8.0, 0.0, 8.0):
            dist = _hit_distance(pawn, start, yaw + offset)
            if dist is None:
                break
            angle = math.radians(yaw + offset)
            points.append((start[0] + math.cos(angle) * dist, start[1] + math.sin(angle) * dist))
        if len(points) == 3:
            (ax, ay), (bx, by), (cx, cy) = points
            length = math.hypot(cx - ax, cy - ay)
            if length > 1.0:
                residual = abs((cx - ax) * (ay - by) - (ax - bx) * (cy - ay)) / length
                perp = abs((cx - ax) * (ay - start[1]) - (ax - start[0]) * (cy - ay)) / length
                normal_yaw = math.degrees(math.atan2(cx - ax, -(cy - ay)))
                facing = normal_yaw if math.cos(math.radians(normal_yaw - yaw)) > 0 else normal_yaw + 180.0
                vertical = [_hit_distance(pawn, start, facing, pitch) for pitch in (-12.0, 12.0)]
                flat = all(v is not None for v in vertical) and abs(vertical[0] - vertical[1]) < flat_frac * perp + 5
                if min_dist <= perp <= max_dist and residual < max_residual and flat:
                    score = residual + abs(perp - (min_dist + max_dist) / 2) / 50.0
                    if best is None or score < best["score"]:
                        best = {"yaw": facing % 360.0, "perp": perp, "residual": residual, "score": score}
        yaw += step
    if best is None:
        return {"found": False, "eye": list(start)}
    _set_view(best["yaw"], 0.0)
    aim = _hit_distance(pawn, start, best["yaw"], 0.0)
    ang = math.radians(best["yaw"])
    pose = {"found": True, "eye": list(start), "yaw_deg": best["yaw"], "pitch_deg": 0.0,
            "perp_uu": round(best["perp"], 2), "residual_uu": round(best["residual"], 3),
            "aim_distance_uu": None if aim is None else round(aim, 2),
            "aim_hit": None if aim is None else [start[0] + math.cos(ang) * aim, start[1] + math.sin(ang) * aim,
                                                 start[2]],
            "pawn_location": [loc.X, loc.Y, loc.Z], "map": str(get_pc().WorldInfo.GetMapName(True))}
    fp_state["wall"] = pose
    _write_json(_out("shots") / "pose.json", pose)
    return pose


def fp_reface():
    """Re-apply the recorded view (recoil moves the camera between shots)."""
    pose = fp_state.get("wall")
    if pose is None or not pose.get("found"):
        return None
    _set_view(pose["yaw_deg"], pose.get("pitch_deg", 0.0))
    return {"yaw_deg": pose["yaw_deg"], "pitch_deg": pose.get("pitch_deg", 0.0)}


def fp_use_view(label="current_view"):
    """Record the camera as it is now as the fixed pose (the aim is set by hand, the wall distance comes
    from the logged impact). Writes <RG_OUT>/shots/pose.json."""
    pc, pawn = _get_pawn()
    rot = pc.Rotation
    yaw = (rot.Yaw & 0xFFFF) * 360.0 / 65536
    pitch = ((rot.Pitch & 0xFFFF) - (65536 if (rot.Pitch & 0xFFFF) > 32767 else 0)) * 360.0 / 65536
    loc = pawn.Location
    pose = {"found": True, "mode": str(label), "yaw_deg": yaw, "pitch_deg": pitch,
            "eye": [loc.X, loc.Y, loc.Z + float(getattr(pawn, "BaseEyeHeight", 60.0))],
            "pawn_location": [loc.X, loc.Y, loc.Z], "map": str(pc.WorldInfo.GetMapName(True))}
    fp_state["wall"] = pose
    _write_json(_out("shots") / "pose.json", pose)
    return pose


def fp_remove_hooks():
    """Remove every hook this probe added (both PRE and POST), so a changed HOOK_SPECS takes effect."""
    removed = 0
    for entry in fp_state.get("hooked", []):
        if not isinstance(entry, str):
            continue
        for when in (Type.PRE, Type.POST):
            try:
                remove_hook(entry, when, HOOK_ID)
                removed += 1
            except Exception:
                continue
    try:
        remove_hook(TICK_PATH, Type.POST_UNCONDITIONAL, HOOK_ID + "_tick")
    except Exception:
        pass
    fp_state["hooks_on"] = False
    fp_state["hooked"] = []
    return {"removed": removed}


def _args(args):
    """Hook arguments as JSON-safe values; the SDK exposes the function parameters on the args object."""
    out = {}
    try:
        for prop in args._type._properties():
            try:
                out[prop.Name] = deep(getattr(args, prop.Name))
            except Exception:
                continue
    except Exception as error:
        out["_error"] = repr(error)[:120]
    return out


def _log(kind, **data):
    run = fp_state.get("run")
    if run is None:
        return
    row = {"kind": kind, "t": time.perf_counter(), "game_t": _game_t()}
    row.update({k: deep(v) for k, v in data.items()})
    run["file"].write(json.dumps(row, ensure_ascii=False) + "\n")
    run["counts"][kind] = run["counts"].get(kind, 0) + 1


def _safe(callback):
    def wrapper(obj, args, ret, func):
        try:
            callback(obj, args, ret, func)
        except Exception as error:
            _log("hook_error", func=func.func.Name, error=repr(error)[:240])
    return wrapper


def _on_weapon(obj, args, ret, func):
    pc = get_pc()
    _log("fire_call", func=func.func.Name, weapon=_path(obj), name=_safe_call(obj, "GetShortHumanReadableName"),
         args=_args(args), rotation=[pc.Rotation.Pitch, pc.Rotation.Yaw, pc.Rotation.Roll],
         fire_interval=_deep_attr(obj, "FireInterval"), fire_rate=_deep_attr(obj, "FireRate"))


def _on_weapon_post(obj, args, ret, func):
    _log("fire_post", func=func.func.Name, weapon=_path(obj), args=_args(args))


def _on_projectile(obj, args, ret, func):
    _log("proj_event", func=func.func.Name, projectile=_path(obj), args=_args(args),
         location=_vec_list(obj, "Location"), velocity=_vec_list(obj, "Velocity"))


def _on_pawn(obj, args, ret, func):
    # Pawn location and vitals at the event, so splash distance and per-hit health loss can be fitted later.
    _log("pawn_event", func=func.func.Name, pawn=_path(obj), args=_args(args),
         pawn_location=_vec_list(obj, "Location"), vitals=fp2_vitals(obj))


def _vec_list(obj, name):
    try:
        v = getattr(obj, name)
        return [v.X, v.Y, v.Z]
    except Exception:
        return None


def _deep_attr(obj, name):
    try:
        return deep(getattr(obj, name))
    except Exception:
        return None


def _safe_call(obj, name):
    try:
        method = getattr(obj, name)
        return deep(method()) if callable(method) else None
    except Exception:
        return None


def _near_eye(obj, radius):
    """True when an actor is within radius uu of the player's eye (where our own shots start)."""
    pc, pawn = _get_pawn()
    loc = pawn.Location
    eye = (loc.X, loc.Y, loc.Z + float(getattr(pawn, "BaseEyeHeight", 60.0)))
    p = obj.Location
    return math.dist(eye, (p.X, p.Y, p.Z)) <= radius


def fp_fire_start(mode=0):
    """Hold fire through the controller (the same call the fire button makes), for a timed hold."""
    get_pc().StartFire(int(mode))
    return {"started": int(mode)}


def fp_fire_stop(mode=0):
    get_pc().StopFire(int(mode))
    return {"stopped": int(mode)}


def _tick(*_args):
    run = fp_state.get("run")
    if run is None:
        return
    try:
        now = {}
        baseline = fp_state.setdefault("proj_baseline", set())
        for proj in list(unrealsdk.find_all("WillowProjectile", exact=False)):
            path = _path(proj)
            if not path or "Default__" in path or path in baseline:
                continue  # projectile actors that already existed when the run began are not ours
            loc = _vec_list(proj, "Location")
            vel = _vec_list(proj, "Velocity")
            if not vel or not any(vel):
                continue  # parked actors: nothing to measure until they move
            if path not in fp_state["proj_ids"]:
                # A projectile we did not fire (NPC darts across the hall) is first seen far from the muzzle.
                if not _near_eye(proj, 600.0):
                    baseline.add(path)
                    continue
                fp_state["proj_ids"][path] = len(fp_state["proj_ids"]) + 1
            pid = fp_state["proj_ids"][path]
            now[path] = {"id": pid, "loc": loc, "vel": vel}
            _log("proj", id=pid, loc=loc, vel=vel)
        for path, last in fp_state["proj_seen"].items():
            if path not in now:
                _log("proj_gone", id=last["id"], last_loc=last["loc"], last_vel=last["vel"])
        fp_state["proj_seen"] = now
    except Exception as error:
        _log("tick_error", error=repr(error)[:240])


def fp_install_hooks():
    """Add the fire / projectile / damage / status hooks once per session; they log only inside a run."""
    if fp_state["hooks_on"]:
        return {"already": True, "hooked": fp_state["hooked"]}
    cb = {"fire": _on_weapon, "fire_post": _on_weapon_post, "projectile": _on_projectile, "pawn": _on_pawn}
    hooked = []
    for class_name, names, kind, when in HOOK_SPECS:
        cls = unrealsdk.find_class(class_name)
        own = {}
        for field in cls._fields():
            if field.Class.Name == "Function" and field.Outer == cls and field.Name in names:
                own[field.Name] = field._path_name()
        for name in names:
            path = own.get(name)
            if path is None:
                hooked.append({"missing": class_name + ":" + name})
                continue
            add_hook(path, Type.PRE if when == "PRE" else Type.POST, HOOK_ID, _safe(cb[kind]))
            hooked.append(path)
    add_hook(TICK_PATH, Type.POST_UNCONDITIONAL, HOOK_ID + "_tick", _tick)
    hooked.append(TICK_PATH)
    fp_state["hooks_on"] = True
    fp_state["hooked"] = hooked
    return {"hooked": hooked}


def fp_run_begin(name):
    """Open <RG_OUT>/shots/<name>/events.jsonl and log the view and the held gun at the start."""
    _pc, pawn = _get_pawn()
    if fp_state.get("run") is not None:
        fp_run_end()
    folder = _out("shots", name)
    stream = (folder / "events.jsonl").open("w", encoding="utf-8")
    fp_state["run"] = {"name": name, "file": stream, "counts": {}}
    fp_state["proj_seen"] = {}
    fp_state["proj_baseline"] = {_path(p) for p in unrealsdk.find_all("WillowProjectile", exact=False)
                                 if _path(p) and "Default__" not in _path(p)}
    _log("run_begin", name=name, held=_path(pawn.Weapon) if pawn.Weapon else None,
         held_name=_safe_call(pawn.Weapon, "GetShortHumanReadableName") if pawn.Weapon else None,
         wall=fp_state.get("wall"))
    return {"run": name, "folder": str(folder)}


def fp_run_end():
    run = fp_state.get("run")
    if run is None:
        return None
    _log("run_end")
    run["file"].close()
    fp_state["run"] = None
    return {"name": run["name"], "counts": run["counts"]}


def fp_held():
    _pc, pawn = _get_pawn()
    weapon = pawn.Weapon
    if weapon is None:
        return None
    return {"path": _path(weapon), "name": _safe_call(weapon, "GetShortHumanReadableName"),
            "slot": _slot(weapon), "fire_interval": _deep_attr(weapon, "FireInterval"),
            "fire_rate": _deep_attr(weapon, "FireRate"), "spread": _deep_attr(weapon, "Spread"),
            "projectiles_per_shot": _deep_attr(weapon, "ProjectilesPerShot")}


def fp_pawns_near(radius=3500.0):
    """Non-default pawns within radius uu of the player: a safety check before firing."""
    _pc, pawn = _get_pawn()
    loc = pawn.Location
    rows = []
    for p in unrealsdk.find_all("WillowPawn", exact=False):
        path = _path(p)
        if not path or "Default__" in path or p == pawn:
            continue
        try:
            d = math.dist((loc.X, loc.Y, loc.Z), (p.Location.X, p.Location.Y, p.Location.Z))
        except Exception:
            continue
        if d <= radius:
            rows.append({"path": path, "distance_uu": round(d, 1), "class": p.Class.Name})
    return sorted(rows, key=lambda r: r["distance_uu"])


# ---------------------------------------------------------------- round 2: save guard, target, coverage guns

SAVE_CLASSES = ("WillowSaveGameManager", "WillowGameInfo", "WillowPlayerController",
                "WillowGameViewportClient", "WillowGameEngine")
SAVE_WORDS = ("save", "backup")
SAVE_SKIP_WORDS = ("load", "get", "is", "has", "can", "check")
VITAL_NAMES = ("HealthVar", "HealthMaxVar", "ShieldVar", "ShieldMaxVar", "IntrinsicArmor", "bIsDead", "bHasShieldVar")
FN_WORDS = ("impact", "trace", "instant", "hit", "status", "dot", "burn", "shock", "corro", "slag", "damage",
            "explo", "throw", "reload", "decal", "particle", "effect", "fire", "projectile")
R2_CLASSES = ("WillowWeapon", "WillowPawn", "WillowProjectile", "WillowAIPawn")
SAVE_GUARD_ID = "ow_r2_save_guard"


def _own_functions(class_name):
    """Own function names of a class (not inherited) mapped to their object paths."""
    cls = unrealsdk.find_class(class_name)
    own = {}
    for field in cls._fields():
        if field.Class.Name == "Function" and field.Outer == cls:
            own[field.Name] = field._path_name()
    return own


def _append_row(path, row):
    with path.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(row, ensure_ascii=False) + "\n")


def _is_save_write(name):
    """Save-write entry points only: names that start with a save/backup verb and are not completion callbacks.
    Apply*, Notify*, On*, Read*, Client*, Server* and Get*/Is* stay unhooked (they load or report, not write)."""
    low = name.lower()
    if "complete" in low or "started" in low:
        return False
    return low.startswith(("save", "quicksave", "backup", "deletesave", "writelastsavedid", "preprofilewrite"))


def fp2_unguard():
    """Remove the save-guard hooks (the previous, broader guard hooked load and callback functions)."""
    removed = 0
    for class_name in SAVE_CLASSES:
        try:
            own = _own_functions(class_name)
        except Exception:
            continue
        for _name, path in own.items():
            try:
                remove_hook(path, Type.PRE, SAVE_GUARD_ID)
                removed += 1
            except Exception:
                continue
    return {"removed": removed}


def fp2_guard_saves():
    """Block save/backup functions on the save-related classes (in addition to block_saves()) and log every
    refused call to <RG_OUT>/saves.jsonl. The .bak rewrite of round 1 happened at game start, before this
    could run; the read-only attribute on the backups (set by the PowerShell side) covers that window."""
    from unrealsdk.hooks import Block

    def guard(obj, args, ret, func):
        _append_row(_out() / "saves.jsonl", {"kind": "save_blocked", "t": time.perf_counter(),
                                             "game_t": _game_t(), "func": func.func.Name,
                                             "args": _args(args)})
        return Block

    hooked, skipped = [], []
    for class_name in SAVE_CLASSES:
        try:
            own = _own_functions(class_name)
        except Exception as error:
            skipped.append({"class": class_name, "error": repr(error)[:120]})
            continue
        for name, path in sorted(own.items()):
            if _is_save_write(name):
                add_hook(path, Type.PRE, SAVE_GUARD_ID, guard)
                hooked.append(path)
    result = {"hooked": hooked, "skipped_classes": skipped}
    _write_json(_out() / "saves_guard.json", result)
    return result


def fp2_discover():
    """Function names of the weapon, pawn and projectile classes that look like impact, status, damage or fire
    paths. Writes <RG_OUT>/fn_discovery.json. Hook the ones that matter with fp2_hook()."""
    rows = {}
    for class_name in R2_CLASSES:
        try:
            own = _own_functions(class_name)
        except Exception as error:
            rows[class_name] = {"error": repr(error)[:120]}
            continue
        rows[class_name] = sorted(n for n in own if any(w in n.lower() for w in FN_WORDS))
        if class_name == "WillowWeapon":
            rows["WillowWeapon_all"] = sorted(own)
    _write_json(_out() / "fn_discovery.json", rows)
    return {k: len(v) for k, v in rows.items()}


def _fn_callback(tag):
    def callback(obj, args, ret, func):
        _log("fn", when=tag, func=func.func.Name, obj=_path(obj), args=_args(args), ret=deep(ret))
    return callback


def fp2_hook(class_name, names):
    """Log the args (PRE) and return value (POST) of the named own functions of one class, inside a run."""
    own = _own_functions(class_name)
    hooked, missing = [], []
    for name in names:
        path = own.get(name)
        if path is None:
            missing.append(name)
            continue
        add_hook(path, Type.PRE, HOOK_ID, _safe(_fn_callback("pre")))
        add_hook(path, Type.POST, HOOK_ID, _safe(_fn_callback("post")))
        hooked.append(path)
    fp_state.setdefault("hooked", []).extend(hooked)
    return {"hooked": hooked, "missing": missing}


def fp2_vitals(pawn):
    """Every readable health / shield / armor / dead property of a pawn (names as the game has them)."""
    out = {}
    try:
        names = _field_names(pawn)
    except Exception:
        return out
    for name in VITAL_NAMES:
        if name in names:
            out[name] = _deep_attr(pawn, name)
    return out


def fp2_target(name, distance=600.0, bearing=180.0):
    """Isolate the AI pawn `name`, put Maya `distance` uu away on `bearing`, and aim at it. Needs
    scripts/phaselock.py and scripts/phaselock_matched.py loaded first (same namespace)."""
    for helper in ("isolate", "place", "aim", "measure", "ai_pawn"):
        if not callable(globals().get(helper)):
            raise RuntimeError("load phaselock.py and phaselock_matched.py first (missing " + helper + ")")
    iso = isolate(name)
    # The target keeps no controller: a bullymong with one melees Maya (83 per hit, large momentum) and pushes her off the mark.
    ai_pawn(name).DetachFromController(False)
    placed = place(name, float(distance), float(bearing))
    pitch = aim(name)
    fp_state["target"] = name
    return {"target": name, "isolate": iso, "placed": placed, "aim_pitch": pitch,
            "measure": measure(name), "vitals": fp2_vitals(ai_pawn(name))}


def fp2_target_state():
    """Health and shield of the current target, looked up again now (None when no target is set)."""
    name = fp_state.get("target")
    if not name:
        return None
    try:
        pawn = ai_pawn(name)
    except LookupError:
        return {"target": name, "gone": True}
    return {"target": name, "vitals": fp2_vitals(pawn), "measure": measure(name)}


def fp2_held_check(recipe_id):
    """Whether the gun in hand is the recipe's gun (balance, type, manufacturer and parts match)."""
    _pc, pawn = _get_pawn()
    held = pawn.Weapon
    if held is None:
        return {"held": None, "matches": False}
    recipe = _recipe(recipe_id)
    return {"held": _path(held), "name": _safe_call(held, "GetShortHumanReadableName"),
            "matches": bool(_matches(held, recipe) or _loose(held, recipe))}


def fp2_probe_guns_left():
    """Inventory weapons that this probe spawned (quick slots and backpack). Empty when clean."""
    _im, equipped, backpack = _inventory()
    out = []
    for recipe_id in sorted(fp_state.get("spawned", set())):
        recipe = _recipe(recipe_id)
        for weapon in equipped + backpack:
            if _matches(weapon, recipe) or _loose(weapon, recipe):
                out.append({"recipe": recipe_id, "path": _path(weapon), "slot": _slot(weapon)})
    return out


# ---------------------------------------------------------------- offline analysis (python -I, not in game)

def analyze_run(events_path):
    """Per projectile: first and last speed, speed change per second, vertical-velocity slope (gravity, uu/s^2),
    lifetime, and where it was last seen. Pawn damage events with their location and vitals. Pure Python over
    events.jsonl; writes analysis.json next to it. Values are raw fits, not verified models."""
    rows = [json.loads(line) for line in Path(events_path).read_text(encoding="utf-8").splitlines() if line.strip()]
    by_id = {}
    for row in rows:
        if row.get("kind") == "proj":
            by_id.setdefault(row["id"], []).append(row)
    gone = {row["id"]: row for row in rows if row.get("kind") == "proj_gone"}
    projectiles = {}
    for pid, samples in sorted(by_id.items()):
        samples.sort(key=lambda r: r["t"])
        t0 = samples[0]["t"]
        pts = [(s["t"] - t0, s["vel"]) for s in samples if s.get("vel")]
        speeds = [math.dist(v, (0, 0, 0)) for _, v in pts]
        slope = None
        if len(pts) >= 3:
            xs = [p[0] for p in pts]
            ys = [p[1][2] for p in pts]
            mx, my = sum(xs) / len(xs), sum(ys) / len(ys)
            var = sum((x - mx) ** 2 for x in xs)
            slope = sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / var if var else None
        span = pts[-1][0] - pts[0][0] if pts else 0.0
        projectiles[str(pid)] = {
            "samples": len(samples), "first_speed": speeds[0] if speeds else None,
            "last_speed": speeds[-1] if speeds else None,
            "speed_change_per_s": (speeds[-1] - speeds[0]) / span if span > 0 else None,
            "vz_slope_uu_s2": slope, "lifetime_s": span,
            "last_loc": gone.get(pid, {}).get("last_loc"),
        }
    pawn_rows = [{"t": r["t"], "func": r["func"], "pawn_location": r.get("pawn_location"),
                  "vitals": r.get("vitals"), "args": r.get("args")}
                 for r in rows if r.get("kind") == "pawn_event"]
    result = {"projectiles": projectiles, "pawn_events": pawn_rows,
              "fire_calls": sum(1 for r in rows if r.get("kind") == "fire_call")}
    Path(events_path).with_name("analysis.json").write_text(json.dumps(result, indent=1, ensure_ascii=False),
                                                            encoding="utf-8")
    return result




def fp2_pick_target(bearing=0.0, distance=600.0):
    """Nearest live bullymong (PrimalBeast skeleton) to Maya; sets it up with fp2_target. Returns None if none."""
    _pc, me = _get_pawn()
    best = None
    for pawn in unrealsdk.find_all("WillowAIPawn"):
        if not pawn.Name.startswith("WillowAIPawn_") or _path(pawn) == _path(me):
            continue
        mesh = getattr(pawn, "Mesh", None)
        sm = getattr(mesh, "SkeletalMesh", None) if mesh is not None else None
        if sm is None or "PrimalBeast" not in sm._path_name():
            continue
        if fp2_vitals(pawn).get("bIsDead") or (fp2_vitals(pawn).get("HealthVar") or 0) <= 0:
            continue
        d = math.dist((me.Location.X, me.Location.Y, me.Location.Z), (pawn.Location.X, pawn.Location.Y, pawn.Location.Z))
        if best is None or d < best[0]:
            best = (d, pawn.Name)
    if best is None:
        return None
    return fp2_target(best[1], distance, bearing)


# ---------------------------------------------------------------- round 3: damage factor, own projectiles, splash, status

# Round 2's ~47,000 per hit was the player's own gear, not the gun: the held weapon's InstantHitDamage modifier stack
# carried skill modifiers (an equipped shield's flat PostAdd and several percentage scales). fp3_damage() shows the stack
# with the skill behind each modifier; fp3_no_skills() deactivates every active skill of the player in memory (skill
# grades, inventory and saves are not touched) so the gun's own value is measured. Call it again after every readying:
# equipping can bring skills back.
FP3_ID = "ow_fp3"
# (class, own function, when) logged inside a run; filters are in _fp3_keep().
FP3_HOOKS = (
    ("WillowWeapon", "FireAmmunition", "PRE"),
    ("WillowWeapon", "ProjectileDefinitionFire", "PRE"),
    ("WillowWeapon", "ProcessInstantHitBullet", "PRE"),
    ("WillowWeapon", "PlayImpactEffects", "PRE"),
    ("WillowWeapon", "BeginReload", "PRE"),
    ("WillowWeapon", "ServerStartReload", "PRE"),
    ("WillowLightProjectileManager", "AddProj", "PRE"),
    ("WillowLightProjectileManager", "GetReflectionDestination", "PRE"),
    ("WillowProjectile", "InitializeFromDefinition", "POST"),
    ("WillowProjectile", "HitWall", "PRE"),
    ("WillowProjectile", "Explode", "PRE"),
    ("WillowProjectile", "DetonateProjectile", "PRE"),
    ("WillowPawn", "TakeDamage", "BOTH"),
    ("WillowPawn", "TakeRadiusDamage", "BOTH"),
    ("WillowDamageTypeDefinition", "CalcRadiusDamageScale", "POST"),
    ("StatusEffectsComponent", "RollChanceForStatusEffect", "BOTH"),
    ("StatusEffectsComponent", "ApplyStatusEffect", "BOTH"),
    ("StatusEffectsComponent", "OnStatusEffectEnd", "PRE"),
    ("DamageOverTimeManager", "AddSourceObject", "PRE"),
    ("DamageOverTimeManager", "DoTakeRadiusDamage", "PRE"),
)
PROJ_FIELDS = ("MaxSpeed", "Speed", "AccelRate", "SpeedMultiplier", "CustomGravityScaling", "Damage", "DamageRadius",
               "FuseTime", "LifeSpan", "Physics", "Definition", "Owner", "Instigator", "bOvercharged")
fp3_state = globals().setdefault("fp3_state", {"hooks": [], "aim": None, "own": {}, "seen_other": set(), "live": {}})


def _skill_of(modifier):
    try:
        return _path(modifier.Outer.Definition)
    except Exception:
        return None


def _stack(stack):
    return [{"type": str(m.Type), "value": m.Value, "skill": _skill_of(m)} for m in stack]


def fp3_damage(weapon=None):
    """The held (or given) gun's damage values and the InstantHitDamage modifier stack with the skill behind each
    modifier; plus the player's level and the playthrough."""
    pc, pawn = _get_pawn()
    weapon = weapon or pawn.Weapon
    if weapon is None:
        return None
    data = _deep_attr(weapon, "DefinitionData") or {}
    return {"weapon": _path(weapon), "name": _safe_call(weapon, "GetShortHumanReadableName"),
            "InstantHitDamage": weapon.InstantHitDamage, "InstantHitDamageBaseValue": weapon.InstantHitDamageBaseValue,
            "stack": _stack(weapon.InstantHitDamageModifierStack), "ProjectilesPerShot": weapon.ProjectilesPerShot,
            "GetMultiProjectileDamage": _safe_call(weapon, "GetMultiProjectileDamage"),
            "StatusEffectDamage": weapon.StatusEffectDamage,
            "status_stack": _stack(weapon.StatusEffectDamageModifierStack),
            "weapon_game_stage": data.get("GameStage"), "player_level": _safe_call(pawn, "GetExpLevel"),
            "playthrough": _safe_call(pc, "GetCurrentPlaythrough")}


def _active_skills():
    pc = get_pc()
    rows = []
    for skill in unrealsdk.find_all("Skill", exact=False):
        path = _path(skill)
        if not path or "Default__" in path:
            continue
        try:
            if "Active" in str(skill.SkillState) and _path(skill.SkillInstigator) == _path(pc):
                rows.append(_path(skill.Definition))
        except Exception:
            continue
    return rows


def fp3_no_skills():
    """Deactivate every active skill of the player (SkillEffectManager.DeactivateAllSkillsForInstigator): skill-tree
    skills, Badass rank, shield and class-mod skills. In memory only; returns what was active and the gun's damage."""
    pc = get_pc()
    before = _active_skills()
    pc.GetSkillManager().DeactivateAllSkillsForInstigator(pc)
    return {"active_before": before, "active_after": _active_skills(), "damage": fp3_damage()}


def fp3_levels(name=None):
    pc, pawn = _get_pawn()
    out = {"player_level": _safe_call(pawn, "GetExpLevel"), "playthrough": _safe_call(pc, "GetCurrentPlaythrough")}
    if name:
        target = ai_pawn(name)
        out.update({"target_level": _safe_call(target, "GetExpLevel"), "target_health": _safe_call(target, "GetHealth"),
                    "target_max_health": _safe_call(target, "GetMaxHealth"),
                    "target_class": _path(getattr(target, "AIClass", None))})
    return out


def fp3_target(name, distance=400.0, bearing=0.0, max_health=1.0e6):
    """fp2_target (isolate, detach, place, aim) plus a large health pool, so one target takes every hit of a series.
    Damage, status chance and DOT are not expected to read the target's health (UNVERIFIED)."""
    result = fp2_target(name, distance, bearing)
    if max_health:
        ai_pawn(name).SetMaxHealth(float(max_health))
        ai_pawn(name).SetHealth(float(max_health))
    result["health_after_setup"] = [_safe_call(ai_pawn(name), "GetHealth"), _safe_call(ai_pawn(name), "GetMaxHealth")]
    result["levels"] = fp3_levels(name)
    return result


def fp3_refill(max_health=1.0e6):
    ai_pawn(fp_state.get("target")).SetHealth(float(max_health))
    return _safe_call(ai_pawn(fp_state.get("target")), "GetHealth")


def _eye():
    _pc, pawn = _get_pawn()
    loc = pawn.Location
    return (loc.X, loc.Y, loc.Z + float(getattr(pawn, "BaseEyeHeight", 60.0)))


def fp3_aim_point(x, y, z, lock=True):
    """Aim the view at a world point; with lock the view is put back there every frame (recoil removed)."""
    ex, ey, ez = _eye()
    dx, dy, dz = float(x) - ex, float(y) - ey, float(z) - ez
    yaw = math.degrees(math.atan2(dy, dx))
    pitch = math.degrees(math.atan2(dz, math.hypot(dx, dy)))
    rot = (int(round(pitch * 65536 / 360)) & 0xFFFF, int(round(yaw * 65536 / 360)) & 0xFFFF)
    get_pc().Rotation = unrealsdk.make_struct("Rotator", Pitch=rot[0], Yaw=rot[1], Roll=0)
    fp3_state["aim"] = rot if lock else None
    return {"pitch_deg": round(pitch, 3), "yaw_deg": round(yaw, 3), "point": [float(x), float(y), float(z)],
            "eye": [ex, ey, ez], "range": round(math.dist((ex, ey, ez), (float(x), float(y), float(z))), 1)}


def fp3_aim_target(dz=0.0, lock=True):
    t = ai_pawn(fp_state.get("target")).Location
    return fp3_aim_point(t.X, t.Y, t.Z + float(dz), lock)


def fp3_floor_z():
    """Floor height under the target: its origin minus its collision half-height."""
    target = ai_pawn(fp_state.get("target"))
    return target.Location.Z - float(target.CylinderComponent.CollisionHeight)


def fp3_aim_ground(offset, lock=True):
    """Aim at the floor `offset` uu in front of the target, on the line from the target towards the player."""
    t = ai_pawn(fp_state.get("target")).Location
    me = get_pc().Pawn.Location
    d = math.hypot(me.X - t.X, me.Y - t.Y) or 1.0
    x, y = t.X + (me.X - t.X) / d * float(offset), t.Y + (me.Y - t.Y) / d * float(offset)
    out = fp3_aim_point(x, y, fp3_floor_z(), lock)
    out["offset"] = float(offset)
    return out


def fp3_unlock_aim():
    fp3_state["aim"] = None
    return True


def _fp3_is_own(proj, pawn):
    try:
        return pawn is not None and _path(proj.Instigator) == _path(pawn)
    except Exception:
        return False


def _fp3_tick(*_args):
    aim = fp3_state.get("aim")
    if aim is not None:
        try:
            get_pc().Rotation = unrealsdk.make_struct("Rotator", Pitch=aim[0], Yaw=aim[1], Roll=0)
        except Exception:
            pass
    if fp_state.get("run") is None:
        return
    try:
        pawn = get_pc().Pawn
        held = _path(pawn.Weapon) if pawn.Weapon else None
        now = {}
        for proj in list(unrealsdk.find_all("WillowProjectile", exact=False)):
            path = _path(proj)
            if not path or "Default__" in path or path in fp3_state["seen_other"]:
                continue
            if path not in fp3_state["own"]:
                if not _fp3_is_own(proj, pawn):
                    fp3_state["seen_other"].add(path)  # gore, den explosions, NPC shots: never logged
                    continue
                fp3_state["own"][path] = len(fp3_state["own"]) + 1
                meta = _selected_props(proj, PROJ_FIELDS)
                meta["held_at_spawn"] = held
                _log("p3_new", id=fp3_state["own"][path], path=path, meta=meta)
            pid = fp3_state["own"][path]
            loc, vel = _vec_list(proj, "Location"), _vec_list(proj, "Velocity")
            now[path] = {"id": pid, "loc": loc, "vel": vel}
            _log("p3", id=pid, loc=loc, vel=vel, acc=_vec_list(proj, "Acceleration"))
        for path, last in fp3_state.get("live", {}).items():
            if path not in now:
                _log("p3_gone", id=last["id"], last_loc=last["loc"], last_vel=last["vel"])
        fp3_state["live"] = now
        _fp3_light(pawn)
    except Exception as error:
        _log("tick_error", error=repr(error)[:240])


def _fp3_light(pawn):
    """Light bullets (Bullet fire type: most guns, the gyrojet and the Torgue shotgun) are entries of the player's
    WillowLightProjectileManager.MyProjs, not actors; log every live entry each frame."""
    for manager in unrealsdk.find_all("WillowLightProjectileManager", exact=False):
        if _path(getattr(manager, "PawnOwner", None)) != _path(pawn):
            continue
        for entry in manager.MyProjs:
            _log("lp", id=entry.Id, start=entry.StartTime, loc=[entry.Location.X, entry.Location.Y, entry.Location.Z],
                 vel=[entry.Velocity.X, entry.Velocity.Y, entry.Velocity.Z], ric=entry.NumRicochets,
                 dmg=entry.Damage, on=bool(entry.bEnabled), fire_def=_path(entry.FireDef),
                 start_trace=[entry.StartTrace.X, entry.StartTrace.Y, entry.StartTrace.Z],
                 end_trace=[entry.EndTrace.X, entry.EndTrace.Y, entry.EndTrace.Z])


def fp3_aim_dir(pitch_deg, yaw_deg, lock=True):
    """Aim along a fixed direction (degrees); with lock the view is held there every frame."""
    rot = (int(round(float(pitch_deg) * 65536 / 360)) & 0xFFFF, int(round(float(yaw_deg) * 65536 / 360)) & 0xFFFF)
    get_pc().Rotation = unrealsdk.make_struct("Rotator", Pitch=rot[0], Yaw=rot[1], Roll=0)
    fp3_state["aim"] = rot if lock else None
    return {"pitch_deg": float(pitch_deg), "yaw_deg": float(yaw_deg), "eye": list(_eye())}


def _is_target(path):
    target = fp_state.get("target")
    return bool(target and path and (path.endswith("." + target) or ("." + target + ".") in path))


def _fp3_keep(cls, obj, args):
    """Only events that belong to the player's shots or to the target."""
    pc = get_pc()
    pawn = pc.Pawn
    if cls == "WillowWeapon":
        return pawn is not None and pawn.Weapon is not None and _path(obj) == _path(pawn.Weapon)
    if cls == "WillowProjectile":
        return _fp3_is_own(obj, pawn)
    if cls == "WillowPawn":
        return _is_target(_path(obj)) or _path(getattr(args, "InstigatedBy", None)) == _path(pc)
    if cls == "StatusEffectsComponent":
        return _is_target(_path(obj)) or _is_target(_path(getattr(obj, "Owner", None)))
    return True  # light-projectile manager, damage types, DOT manager: filtered offline by their args


def _fp3_callback(cls, when):
    def callback(obj, args, ret, func):
        if fp_state.get("run") is None or not _fp3_keep(cls, obj, args):
            return
        row = {"cls": cls, "func": func.func.Name, "when": when, "obj": _path(obj), "args": _args(args)}
        if when == "post":
            row["ret"] = deep(ret)
        if cls == "WillowPawn":
            row["health"] = _safe_call(obj, "GetHealth")
            row["loc"] = _vec_list(obj, "Location")
            pipeline = getattr(args, "Pipeline", None)
            if when == "post" and pipeline is not None:
                row["summary"] = _deep_attr(pipeline, "DamageSummary")
            if "StatusEffect" in str(_path(getattr(args, "DamageType", None))):
                # DOT tick: the effect's own clock (AccumulatedTime, TotalElapsedTime) around the damage call
                component = obj.GetStatusEffectsComponent()
                row["ase"] = _deep_attr(component, "ActiveStatusEffects") if component is not None else None
        if cls == "WillowProjectile":
            row["loc"] = _vec_list(obj, "Location")
            row["vel"] = _vec_list(obj, "Velocity")
            row["id"] = fp3_state["own"].get(_path(obj))
            if func.func.Name == "InitializeFromDefinition":
                row["meta"] = _selected_props(obj, PROJ_FIELDS)
        if cls == "StatusEffectsComponent":
            row["owner"] = _path(getattr(obj, "Owner", None))
            if when == "post" and func.func.Name == "ApplyStatusEffect":
                row["active"] = _deep_attr(obj, "ActiveStatusEffects")  # duration, DPS, elapsed of each effect
        _log("e3", **row)
    return callback


def fp3_install_hooks():
    """Install the round-3 hooks and the per-frame sampler / aim lock (idempotent)."""
    if fp3_state["hooks"]:
        return {"already": len(fp3_state["hooks"])}
    hooked, missing = [], []
    for cls, name, when in FP3_HOOKS:
        try:
            path = _own_functions(cls).get(name)
        except Exception:
            path = None
        if path is None:
            missing.append(cls + ":" + name)
            continue
        if when in ("PRE", "BOTH"):
            add_hook(path, Type.PRE, FP3_ID, _safe(_fp3_callback(cls, "pre")))
        if when in ("POST", "BOTH"):
            add_hook(path, Type.POST, FP3_ID, _safe(_fp3_callback(cls, "post")))
        hooked.append(path)
    add_hook(TICK_PATH, Type.POST_UNCONDITIONAL, FP3_ID + "_tick", _fp3_tick)
    fp3_state["hooks"] = hooked
    return {"hooked": hooked, "missing": missing}


def fp3_remove_hooks():
    removed = 0
    for path in fp3_state.get("hooks", []):
        for when in (Type.PRE, Type.POST):
            try:
                remove_hook(path, when, FP3_ID)
                removed += 1
            except Exception:
                pass
    try:
        remove_hook(TICK_PATH, Type.POST_UNCONDITIONAL, FP3_ID + "_tick")
    except Exception:
        pass
    fp3_state["hooks"] = []
    fp3_state["aim"] = None
    return {"removed": removed}


def fp3_camera():
    """The exact view: GetPlayerViewPoint location and rotation, FOV, pawn location and eye height."""
    pc = get_pc()
    view = {}
    try:
        loc = unrealsdk.make_struct("Vector")
        rot = unrealsdk.make_struct("Rotator")
        res = pc.GetPlayerViewPoint(loc, rot)
        loc, rot = res[-2:] if isinstance(res, tuple) else (loc, rot)  # (return value, out location, out rotation)
        view = {"view_loc": [loc.X, loc.Y, loc.Z], "view_rot": [rot.Pitch, rot.Yaw, rot.Roll]}
    except Exception as error:
        view = {"view_error": repr(error)[:120]}
    view.update({"fov": _safe_call(pc, "GetFOVAngle"), "default_fov": _deep_attr(pc, "DefaultFOV"),
                 "pawn": _vec_list(pc.Pawn, "Location"), "eye_height": _deep_attr(pc.Pawn, "BaseEyeHeight"),
                 "rotation": [pc.Rotation.Pitch, pc.Rotation.Yaw, pc.Rotation.Roll]})
    return view


def fp3_run_begin(name, extra=None):
    """fp_run_begin plus the gun's damage breakdown, the levels, the target state and the camera at the start."""
    fp3_state["own"] = {}
    fp3_state["live"] = {}
    fp3_state["seen_other"] = {_path(p) for p in unrealsdk.find_all("WillowProjectile", exact=False) if _path(p)}
    out = fp_run_begin(name)
    _log("r3_begin", damage=fp3_damage(), levels=fp3_levels(fp_state.get("target")),
         target=fp2_target_state(), camera=fp3_camera(), extra=extra or {})
    return out


def fp3_run_end():
    if fp_state.get("run") is not None:
        _log("r3_end", target=fp2_target_state(), damage=fp3_damage())
    return fp_run_end()


# ---------------------------------------------------------------- round 3 offline fits (python -I, not in game)

def _linfit(xs, ys):
    n = len(xs)
    if n < 2:
        return None, None
    mx, my = sum(xs) / n, sum(ys) / n
    var = sum((x - mx) ** 2 for x in xs)
    if not var:
        return None, None
    slope = sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / var
    return slope, my - slope * mx


def _pairs(e3, func):
    """PRE and POST rows of one pawn function joined per pawn (calls do not nest on one pawn)."""
    out, pre = [], {}
    for r in e3:
        if r["func"] != func:
            continue
        if r["when"] == "pre":
            pre[r["obj"]] = r
        elif r["obj"] in pre:
            out.append((pre.pop(r["obj"]), r))
    return out


def analyze3(events_path):
    """Round-3 run summary over events.jsonl (game time): per own projectile the speed series, speed slope,
    log-speed slope (growth proportional to speed), vertical-velocity slope; radius-damage pairs; damage on the
    target; status and DOT events. Writes analysis3.json next to it. Raw fits, not verified models."""
    rows = [json.loads(line) for line in Path(events_path).read_text(encoding="utf-8").splitlines() if line.strip()]
    meta = {r["id"]: r for r in rows if r.get("kind") == "p3_new"}
    samples = {}
    for r in rows:
        if r.get("kind") == "p3" and r.get("vel"):
            samples.setdefault(r["id"], []).append(r)
    gone = {r["id"]: r for r in rows if r.get("kind") == "p3_gone"}
    projectiles = {}
    for pid, ss in sorted(samples.items()):
        ss.sort(key=lambda r: r["game_t"])
        dedup = []
        for s in ss:  # several viewport ticks can share one game time
            if math.dist(s["vel"], (0, 0, 0)) < 1.0 and dedup:
                break  # exploded projectiles stay parked (speed 0) and are not removed: the flight ends here
            if not dedup or s["game_t"] > dedup[-1]["game_t"] + 1e-6:
                dedup.append(s)
        t0 = dedup[0]["game_t"]
        ts = [s["game_t"] - t0 for s in dedup]
        speeds = [math.dist(s["vel"], (0, 0, 0)) for s in dedup]
        hspeeds = [math.hypot(s["vel"][0], s["vel"][1]) for s in dedup]
        m = meta.get(pid, {}).get("meta", {})
        projectiles[str(pid)] = {
            "definition": m.get("Definition"), "owner": m.get("Owner"), "held_at_spawn": m.get("held_at_spawn"),
            "MaxSpeed": m.get("MaxSpeed"), "AccelRate": m.get("AccelRate"), "SpeedMultiplier": m.get("SpeedMultiplier"),
            "CustomGravityScaling": m.get("CustomGravityScaling"), "LifeSpan": m.get("LifeSpan"),
            "Damage": m.get("Damage"), "DamageRadius": m.get("DamageRadius"), "Physics": m.get("Physics"),
            "samples": len(dedup), "flight_s": ts[-1], "first_loc": dedup[0]["loc"], "first_vel": dedup[0]["vel"],
            "first_acc": dedup[0].get("acc"), "last_loc": gone.get(pid, {}).get("last_loc") or dedup[-1]["loc"],
            "t": [round(t, 4) for t in ts], "speed": [round(s, 2) for s in speeds],
            "hspeed": [round(s, 2) for s in hspeeds], "vz": [round(s["vel"][2], 2) for s in dedup],
            "speed_slope": _linfit(ts, speeds)[0], "hspeed_slope": _linfit(ts, hspeeds)[0],
            "vz_slope": _linfit(ts, [s["vel"][2] for s in dedup])[0],
            "log_speed_slope": _linfit(ts, [math.log(s) for s in speeds])[0] if min(speeds) > 0 else None,
        }
    lights = {}
    for r in rows:
        if r.get("kind") == "lp":
            lights.setdefault("%d@%.4f" % (r["id"], r["start"]), []).append(r)
    light_fits = {}
    for key, ss in lights.items():
        dedup = []
        for s in sorted(ss, key=lambda r: r["game_t"]):
            if not dedup or s["game_t"] > dedup[-1]["game_t"] + 1e-6:
                dedup.append(s)
        ts = [s["game_t"] - dedup[0]["start"] for s in dedup]
        speeds = [math.dist(s["vel"], (0, 0, 0)) for s in dedup]
        light_fits[key] = {
            "fire_def": dedup[0]["fire_def"], "samples": len(dedup), "start_trace": dedup[0]["start_trace"],
            "end_trace": dedup[0]["end_trace"], "t_since_start": [round(t, 4) for t in ts],
            "loc": [[round(c, 1) for c in s["loc"]] for s in dedup], "speed": [round(v, 2) for v in speeds],
            "vz": [round(s["vel"][2], 2) for s in dedup], "ric": [s["ric"] for s in dedup], "dmg": dedup[0]["dmg"],
            "speed_slope": _linfit(ts, speeds)[0],
            "log_speed_slope": _linfit(ts, [math.log(v) for v in speeds])[0] if speeds and min(speeds) > 0 else None,
            "vz_slope": _linfit(ts, [s["vel"][2] for s in dedup])[0],
            "dist_from_start": [round(math.dist(s["loc"], dedup[0]["start_trace"]), 1) for s in dedup]}
    e3 = [r for r in rows if r.get("kind") == "e3"]
    radius = []
    for p, r in _pairs(e3, "TakeRadiusDamage"):
        origin = p["args"].get("HurtOrigin") or {}
        o = [origin.get("X", 0.0), origin.get("Y", 0.0), origin.get("Z", 0.0)]
        loc = p.get("loc") or [0.0, 0.0, 0.0]
        radius.append({"game_t": p["game_t"], "pawn": p["obj"], "base_damage": p["args"].get("BaseDamage"),
                       "radius": p["args"].get("DamageRadius"), "full": p["args"].get("bFullDamage"),
                       "falloff_exp": p["args"].get("DamageFalloffExponent"), "causer": p["args"].get("DamageCauser"),
                       "type": p["args"].get("DamageType"), "origin": o, "pawn_loc": loc,
                       "dist3": round(math.dist(o, loc), 2), "dist_xy": round(math.hypot(o[0] - loc[0], o[1] - loc[1]), 2),
                       "health_before": p.get("health"), "health_after": r.get("health"),
                       "loss": None if p.get("health") is None or r.get("health") is None else p["health"] - r["health"]})
    hits = []
    for p, r in _pairs(e3, "TakeDamage"):
        s = r.get("summary") or {}
        hits.append({"game_t": p["game_t"], "pawn": p["obj"], "damage_arg": p["args"].get("Damage"),
                     "type": p["args"].get("DamageType"), "causer": p["args"].get("DamageCauser"),
                     "bone": (p["args"].get("HitInfo") or {}).get("BoneName"),
                     "health_before": p.get("health"), "health_after": r.get("health"),
                     "loss": None if p.get("health") is None or r.get("health") is None else p["health"] - r["health"],
                     "summary": {k: s.get(k) for k in ("InitialDamage", "FinalDamage", "bWasCrit",
                                                       "ExpLevelDifferenceReduction", "DamageSurfaceReduction",
                                                       "HitRegionReduction", "AIDamageScaleReduction",
                                                       "RecipientDamageTypeReduction", "DamageSourceReduction")}})
    status = [{"game_t": r["game_t"], "func": r["func"], "when": r["when"], "obj": r["obj"], "args": r["args"],
               "ret": r.get("ret")} for r in e3 if r["cls"] in ("StatusEffectsComponent", "DamageOverTimeManager")]
    calc = [{"game_t": r["game_t"], "obj": r["obj"], "args": r["args"], "ret": r.get("ret")}
            for r in e3 if r["func"] == "CalcRadiusDamageScale"]
    result = {"begin": next((r for r in rows if r.get("kind") == "r3_begin"), None),
              "counts": {k: sum(1 for r in e3 if r["func"] == k) for k in sorted({r["func"] for r in e3})},
              "fire_game_t": [r["game_t"] for r in e3 if r["func"] == "FireAmmunition"],
              "projectiles": projectiles, "lights": light_fits, "radius": radius, "calc_radius_scale": calc, "hits": hits, "status": status}
    Path(events_path).with_name("analysis3.json").write_text(json.dumps(result, indent=1, ensure_ascii=False),
                                                             encoding="utf-8")
    return result


if __name__ == "__main__" and len(sys.argv) == 3 and sys.argv[1] in ("analyze", "analyze3"):
    print(json.dumps((analyze_run if sys.argv[1] == "analyze" else analyze3)(sys.argv[2]), indent=1)[:6000])