"""Firing-mode and projectile probe for the openwillow_realgame command channel (lane L7, 2026-10-08).

Run through Invoke-GamePyFile (tools/real_game/realgame.ps1) with the level-8 Maya loaded (`-Character=Save0008.sav`, saves
blocked from the first frame). Our own code, no game data; everything it writes (rows, dumps) is game data and goes under the
folder in `RG_OUT` (the ignored local/realgame/...). Memory-only: reads, plus the documented in-memory steps the caller makes by
hand (spawning weapons into the backpack while saves are blocked, raising the backpack size, equipping, aiming the camera,
`InventoryManager.bInfiniteAmmo`). It shares the command channel's namespace and uses none of combat_probe.py's or
fire_mission_probe.py's names, so it can be loaded next to weapon_cards.py (spawn and equip helpers) but not next to
fire_mission_probe.py (which redefines `_p` and friends).

Data (what exists):
- fp_deep(value): any property value as plain data (objects as path names, enums by name, structs as dicts, arrays as lists).
- fp_props(obj): every Property of obj (own and inherited, engine bookkeeping skipped), deep, as name -> value;
  fp_class_fields(cls_name) lists the names and property kinds.
- fp_enumerate(cls_name, exact=False): path -> {class, default, props} for every live object of the class.
- fp_behaviors(definition): the behaviours a definition's BehaviorProviderDefinition holds (class, path, events; the full properties of
  Behavior_CauseDamage and Behavior_Explode), e.g. a rocket projectile's damage behaviour.
- fp_func_info(cls_name, fn): whether a function is native (hooks see only calls that go through script) and its parameters.
- fp_type_lists(): per WeaponTypeDefinition the default firing mode and the part-related properties.

Shots:
- fp_hook(targets, extra): hook (class, function) pairs, PRE and POST; one row each with the game clock, a frame counter, the object, the
  arguments (and the return value after the call) and optionally what `extra[function]` returns. fp_shot_hooks() installs SHOT_TARGETS
  (one shot's script-visible functions) with fp_ctx as the extra state read; HIT_TARGETS and fp_hit_extra do the same for a victim's
  TakeDamage. fp_frame_start() counts frames; fp_rows(), fp_dump(name, data), fp_clear(), fp_unhook().
  Trap: a native function's out parameter is not filled in the POST argument struct; read the pattern offset from the next AddSpread
  BaseAim, or from the AddProj ray when the spread is suppressed.
- fp_track_bullets(): every frame, every entry of every light projectile manager's MyProjs (id, start time, location, velocity, stored damage,
  mode, counters) into FP['bul']; fp_track_projectiles(): every live WillowProjectile (location, velocity) into FP['proj'].
- fp_weapon_state(): the held weapon's type, balance, barrel and firing-relevant values; fp_ctx(): the same instant's accuracy pool and view.
- fp_gun_fields(spec, barrel): WeaponDefinitionData fields for weapon_cards.spawn_definition from a plain spec of part paths (no game data here).
- fp_live_ai(radius), fp_find_ai(name), fp_place_at(name, distance, bearing), fp_place_keep_side(name, distance), fp_aim_at(name),
  fp_aim(pitch_deg, yaw_deg), fp_place(x, y, z), fp_player_immune(): script stand-ins for setting a shot up (memory only).
"""
import json
import math
import re
import time

import unrealsdk
from unrealsdk.hooks import Type, add_hook, remove_hook
from unrealsdk.unreal import UObject, WrappedArray, WrappedStruct

FP = globals().get("FP") or {"rows": [], "hooks": [], "t0": time.perf_counter(), "on": False, "frame": 0, "cap": 60000}


def fp_deep(value, depth=0, limit=256):
    if depth > 7:
        return repr(value)[:120]
    if isinstance(value, UObject):  # before the enum test: an object's `name` attribute is its short name
        return value._path_name()
    name = getattr(value, "name", None)  # enum members: IntFlag members are ints too
    if isinstance(name, str) and not isinstance(value, (bool, str)):
        return name
    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    if isinstance(value, WrappedStruct):
        out = {}
        for q in value._type._properties():
            try:
                out[q.Name] = fp_deep(getattr(value, q.Name), depth + 1, limit)
            except Exception as error:
                out[q.Name] = "error " + repr(error)[:60]
        return out
    if isinstance(value, WrappedArray):
        return [fp_deep(v, depth + 1, limit) for v in list(value)[:limit]]
    return repr(value)[:120]


def fp_class_fields(cls_name):
    """(name, property class) of every Property of the class and its supers."""
    cls = unrealsdk.find_class(cls_name)
    out = []
    while cls is not None:
        for field in list(cls._fields()):
            try:
                if field.Class.Name.endswith("Property"):
                    out.append((cls.Name, field.Name, field.Class.Name))
            except Exception:
                pass
        cls = cls.SuperField if hasattr(cls, "SuperField") else None
    return out


ENGINE_NOISE = ("VfTableObject", "HashNext", "ObjectFlags", "HashOuterNext", "StateFrame", "Linker", "LinkerIndex",
                "ObjectInternalInteger", "NetIndex", "ObjectArchetype")


def fp_props(obj, skip=ENGINE_NOISE):
    out = {}
    cls = obj.Class
    while cls is not None:
        for field in list(cls._fields()):
            try:
                if not field.Class.Name.endswith("Property"):
                    continue
                name = field.Name
            except Exception:
                continue
            if name in out or name in skip:
                continue
            try:
                out[name] = fp_deep(getattr(obj, name))
            except Exception as error:
                out[name] = "error " + repr(error)[:60]
        cls = cls.SuperField if hasattr(cls, "SuperField") else None
    return out


def fp_enumerate(cls_name, exact=False):
    result = {}
    for obj in unrealsdk.find_all(cls_name, exact=exact):
        path = obj._path_name()
        result[path] = {"class": obj.Class.Name, "default": "Default__" in path, "props": fp_props(obj)}
    return result


def fp_func_info(cls_name, fn):
    cls = unrealsdk.find_class(cls_name)
    func = cls._find(fn)
    flags = int(func.FunctionFlags)
    return {"path": func._path_name(), "flags": hex(flags), "native": bool(flags & 0x400),
            "params": [(q.Name, q.Class.Name) for q in func._properties()]}


def fp_behaviors(definition, with_props=("Behavior_CauseDamage", "Behavior_Explode")):
    """The behaviors a definition's BehaviorProviderDefinition holds: class and path, and the full properties of the
    damage-causing classes (a rocket's direct damage is a Behavior_CauseDamage)."""
    prov = getattr(definition, "BehaviorProviderDefinition", None)
    out = []
    if prov is None:
        return out
    for seq in list(prov.BehaviorSequences):
        names = [e.UserData.EventName for e in list(seq.EventData2)]
        for item in list(seq.BehaviorData2):
            b = item.Behavior
            if b is None:
                continue
            row = {"sequence": seq.BehaviorSequenceName, "events": names, "class": b.Class.Name, "path": b._path_name()}
            if b.Class.Name in with_props:
                row["props"] = fp_props(b)
            out.append(row)
    return out


def fp_type_lists():
    """Per weapon type: its default firing mode and every object-valued property whose name mentions parts or firing."""
    out = {}
    for obj in unrealsdk.find_all("WeaponTypeDefinition", exact=False):
        row = {}
        for name, value in fp_props(obj).items():
            if re.search(r"FiringMode|Part|Barrel|Elemental", name):
                row[name] = value
        out[obj._path_name()] = row
    return out


def _hook_row(path, tag, name):
    def hook(obj, args, ret, func):
        if not FP["on"] or len(FP["rows"]) >= FP["cap"]:
            return
        row = {"t": round(time.perf_counter() - FP["t0"], 4), "g": _game_t(), "f": FP["frame"], "tag": tag, "func": path,
               "obj": fp_deep(obj, 6)}
        try:
            row["args"] = {q.Name: fp_deep(getattr(args, q.Name), 4) for q in func.func._properties() if hasattr(args, q.Name)}
        except Exception as error:
            row["args_error"] = repr(error)[:100]
        if tag == "post":
            try:
                row["ret"] = fp_deep(ret, 3)
            except Exception:
                row["ret"] = "?"
        if name in FP.get("extra", {}):
            try:
                row["extra"] = FP["extra"][name](obj, args)
            except Exception as error:
                row["extra_error"] = repr(error)[:100]
        FP["rows"].append(row)
    return hook


def _game_t():
    try:
        return round(get_pc().WorldInfo.TimeSeconds, 4)
    except Exception:
        return None


def _declared(cls_name, fn):
    cls = unrealsdk.find_class(cls_name)
    paths = []
    for field in cls._fields():
        kind = field.Class.Name
        if kind == "Function" and field.Name == fn and field.Outer == cls:
            paths.append(field._path_name())
        elif kind == "State":
            try:
                for inner in field._fields():
                    if inner.Class.Name == "Function" and inner.Name == fn:
                        paths.append(inner._path_name())
            except Exception:
                pass
    return paths


def fp_hook(targets, extra=None):
    """targets: [(class name, function name)]. Hooks every declaration PRE and POST; returns which resolved."""
    FP["on"] = True
    FP["extra"] = extra or {}
    resolved, missing, failed = [], [], []
    for cls_name, fn in targets:
        try:
            paths = _declared(cls_name, fn)
        except Exception as error:
            paths = []
            failed.append(f"{cls_name}.{fn}: {repr(error)[:80]}")
        if not paths:
            missing.append(f"{cls_name}.{fn}")
        for path in paths:
            for typ, tag in ((Type.PRE, "pre"), (Type.POST, "post")):
                ident = f"ow_fp_{tag}_{path}"
                try:
                    add_hook(path, typ, ident, _hook_row(path, tag, fn))
                    FP["hooks"].append((path, typ, ident))
                except Exception as error:
                    failed.append(f"{path}: {repr(error)[:80]}")
            resolved.append(path)
    return {"resolved": resolved, "missing": missing, "failed": failed}


def fp_frame_start():
    def tick(*_a):
        FP["frame"] += 1
    add_hook("WillowGame.WillowGameViewportClient:Tick", Type.POST_UNCONDITIONAL, "ow_fp_frame", tick)
    FP["hooks"].append(("WillowGame.WillowGameViewportClient:Tick", Type.POST_UNCONDITIONAL, "ow_fp_frame"))


SHOT_TARGETS = [("WillowLightProjectileManager", "AddProj"), ("WillowWeapon", "GetFiringModeDefinition"),
                ("FiringModeDefinition", "GetFiringPatternAdjustments"), ("FiringModeDefinition", "NotifyFiringPatternWhenShotComplete"),
                ("WillowWeapon", "AddSpread"), ("WillowWeapon", "FiringModeDefinitionFire"), ("WillowWeapon", "ProjectileDefinitionFire"),
                ("WillowWeapon", "GetMultiProjectileDamage"), ("WillowWeapon", "FireAmmunition"), ("WillowWeapon", "InstantFire"),
                ("WillowWeapon", "ProcessInstantHitBullet"), ("WillowWeapon", "GetAdjustedAim"), ("WillowPlayerController", "GetAdjustedAimFor"),
                ("WillowWeapon", "SharedFireAmmunition")]


def fp_ctx(obj=None, args=None):
    """What the shot depends on at this instant: the held weapon's spread-related values, the accuracy pool, the view."""
    pc = get_pc()
    pawn = pc.Pawn
    w = pawn.Weapon if pawn is not None else None
    out = {}
    if w is not None:
        for key in ("Spread", "ProjectilesPerShot", "NextFiringPatternIndex", "ReloadCnt", "ProjectileSpeedMultiplier", "InstantHitDamage"):
            try:
                out[key] = fp_deep(getattr(w, key))
            except Exception:
                pass
    try:
        out["acc_pool"] = round(pc.AccuracyPool.Data.CurrentValue, 4)
    except Exception:
        pass
    try:
        out["view_rot"] = fp_rot(pc.Rotation)
        out["pawn_loc"] = fp_vec(pawn.Location)
    except Exception:
        pass
    return out


def fp_shot_hooks(targets=None):
    names = {fn: fp_ctx for _c, fn in (targets or SHOT_TARGETS)}
    return fp_hook(targets or SHOT_TARGETS, extra=names)


SLOT_KEYS = {"Body": "BodyPartDefinition", "Grip": "GripPartDefinition", "Barrel": "BarrelPartDefinition",
             "Sight": "SightPartDefinition", "Stock": "StockPartDefinition", "Elemental": "ElementalPartDefinition",
             "Accessory1": "Accessory1PartDefinition", "Accessory2": "Accessory2PartDefinition", "Material": "MaterialPartDefinition"}


def fp_gun_fields(spec, barrel=None, grade=0):
    """WeaponDefinitionData fields (for weapon_cards.spawn_definition) from a plain spec: {type, balance, manufacturer, stage,
    parts{slot: path}, title, prefix}; `barrel` replaces the Barrel part. Paths only, no game data in this file."""
    fields = {"WeaponTypeDefinition": spec["type"], "BalanceDefinition": spec["balance"], "ManufacturerDefinition": spec["manufacturer"],
              "ManufacturerGradeIndex": grade, "GameStage": spec["stage"]}
    for slot, path in spec["parts"].items():
        if path:
            fields[SLOT_KEYS[slot]] = path
    if barrel:
        fields["BarrelPartDefinition"] = barrel
    if spec.get("title"):
        fields["TitlePartDefinition"] = spec["title"]
    if spec.get("prefix"):
        fields["PrefixPartDefinition"] = spec["prefix"]
    return fields


def fp_track_projectiles():
    """Sample every WillowProjectile near the player each frame (location, velocity) so a rocket's flight is on the same
    clocks as the shot rows. Call once; rows go to FP['proj']."""
    FP.setdefault("proj", [])

    def tick(*_a):
        if not FP["on"]:
            return
        try:
            pc = get_pc()
            for pr in unrealsdk.find_all("WillowProjectile"):
                if "Default__" in pr._path_name() or pr.bDeleteMe:
                    continue
                FP["proj"].append({"g": round(pc.WorldInfo.TimeSeconds, 4), "f": FP["frame"], "name": pr.Name,
                                   "def": fp_deep(getattr(pr, "ProjectileDefinition", None)), "loc": fp_vec(pr.Location),
                                   "vel": fp_vec(pr.Velocity), "life": round(pr.LifeSpan, 3)})
        except Exception:
            pass
    add_hook("WillowGame.WillowGameViewportClient:Tick", Type.POST_UNCONDITIONAL, "ow_fp_proj", tick)
    FP["hooks"].append(("WillowGame.WillowGameViewportClient:Tick", Type.POST_UNCONDITIONAL, "ow_fp_proj"))


def fp_track_bullets():
    """Sample every live entry of every WillowLightProjectileManager.MyProjs each frame (a bullet's id, start time, location,
    velocity, stored damage, the firing mode it was made from, ricochet and penetration counters). Rows go to FP['bul'].
    A bullet is gone from the array on the frame after its last segment was traced, so the last row bounds its arrival time."""
    FP.setdefault("bul", [])

    def tick(*_a):
        if not FP["on"]:
            return
        try:
            now = round(get_pc().WorldInfo.TimeSeconds, 4)
            for m in unrealsdk.find_all("WillowLightProjectileManager"):
                if "Default__" in m._path_name():
                    continue
                for b in list(m.MyProjs):
                    FP["bul"].append({"g": now, "f": FP["frame"], "mgr": m.Name, "id": b.Id, "start": round(b.StartTime, 4),
                                      "loc": fp_vec(b.Location), "vel": fp_vec(b.Velocity), "dmg": round(b.Damage, 4),
                                      "def": fp_deep(b.FireDef), "ric": b.NumRicochets, "pen": b.PenetrationCount,
                                      "st": fp_vec(b.StartTrace), "et": fp_vec(b.EndTrace), "ost": fp_vec(b.OriginalStartTrace),
                                      "hit": fp_deep(b.LastHitActor), "oc": bool(b.bOvercharged), "pidx": b.FiringPatternIdx})
        except Exception as error:
            FP["bul"].append({"err": repr(error)[:200]})
    add_hook("WillowGame.WillowGameViewportClient:Tick", Type.POST_UNCONDITIONAL, "ow_fp_bul", tick)
    FP["hooks"].append(("WillowGame.WillowGameViewportClient:Tick", Type.POST_UNCONDITIONAL, "ow_fp_bul"))


def fp_live_ai(radius=6000.0):
    """The live AI pawns (not the preloaded Pawn_<Type> templates) near the player, nearest first: name, class, health, position."""
    me = get_pc().Pawn.Location
    rows = []
    for pawn in unrealsdk.find_all("WillowAIPawn"):
        if not pawn.Name.startswith("WillowAIPawn_") or pawn.bDeleteMe:
            continue
        loc = pawn.Location
        dist = math.dist((me.X, me.Y, me.Z), (loc.X, loc.Y, loc.Z))
        if dist <= radius:
            try:
                health = round(pawn.GetHealth(), 2)
            except Exception:
                health = None
            rows.append({"name": pawn.Name, "dist": round(dist), "hp": health, "pos": fp_vec(loc), "class": pawn.Class.Name})
    return sorted(rows, key=lambda r: r["dist"])


def fp_find_ai(name):
    for pawn in unrealsdk.find_all("WillowAIPawn"):
        if pawn.Name == name:
            return pawn
    return None


def fp_place_at(name, distance, bearing_deg=180.0, up=40.0):
    """Put the player `distance` units (horizontally) from an AI pawn on the given bearing around it, a little above the
    floor, and turn the camera to its chest (ClientSetRotation). Script stand-in, memory only."""
    target = fp_find_ai(name)
    pc = get_pc()
    t = target.Location
    a = math.radians(bearing_deg)
    x, y, z = t.X + math.cos(a) * distance, t.Y + math.sin(a) * distance, t.Z + 60.0
    pc.Pawn.Location = unrealsdk.make_struct("Vector", X=x, Y=y, Z=z)
    pc.Pawn.SetPhysics(2)
    fp_aim_at(name)
    return [round(x), round(y), round(z)]


def fp_place_keep_side(name, distance):
    """Like fp_place_at, on the side of the target the player is already on."""
    t, me = fp_find_ai(name).Location, get_pc().Pawn.Location
    return fp_place_at(name, distance, math.degrees(math.atan2(me.Y - t.Y, me.X - t.X)))


def fp_aim_at(name, up=40.0):
    target = fp_find_ai(name)
    pc = get_pc()
    me = pc.Pawn.Location
    t = target.Location
    eye = me.Z + pc.Pawn.BaseEyeHeight
    pitch = math.degrees(math.atan2(t.Z + up - eye, math.hypot(t.X - me.X, t.Y - me.Y)))
    yaw = math.degrees(math.atan2(t.Y - me.Y, t.X - me.X))
    fp_aim(pitch, yaw)
    return [round(pitch, 2), round(yaw, 2)]


def fp_player_immune(on=True):
    """Refuse TakeDamage calls on the player's pawn for the rest of the session (a hook that blocks the call; memory only)."""
    from unrealsdk.hooks import Block

    def hook(obj, args, ret, func):
        try:
            if obj == get_pc().Pawn:
                return Block
        except Exception:
            pass
    add_hook("WillowGame.WillowPawn:TakeDamage", Type.PRE, "ow_fp_immune", hook)
    FP["hooks"].append(("WillowGame.WillowPawn:TakeDamage", Type.PRE, "ow_fp_immune"))
    return True


HIT_TARGETS = [("WillowPawn", "TakeDamage"), ("WillowAIPawn", "TakeDamage")]


def fp_hit_extra(obj, args):
    out = {}
    try:
        out["hp"] = round(obj.GetHealth(), 3)
        out["loc"] = fp_vec(obj.Location)
        out["name"] = obj.Name
    except Exception:
        pass
    try:
        me = get_pc().Pawn.Location
        out["dist_to_player"] = round(math.dist((me.X, me.Y, me.Z), (obj.Location.X, obj.Location.Y, obj.Location.Z)), 1)
    except Exception:
        pass
    return out


def fp_unhook():
    FP["on"] = False
    for path, typ, ident in FP["hooks"]:
        try:
            remove_hook(path, typ, ident)
        except Exception:
            pass
    FP["hooks"] = []


def fp_clear():
    FP["rows"] = []
    FP["t0"] = time.perf_counter()


def fp_rows(start=0):
    return FP["rows"][start:]


def fp_dump(name, data=None):
    path = RG_OUT + "/" + name
    with open(path, "w", encoding="utf-8") as handle:
        json.dump(FP["rows"] if data is None else data, handle, default=str)
    return {"path": path, "rows": len(FP["rows"]) if data is None else None}


def fp_aim(pitch_deg, yaw_deg):
    pc = get_pc()
    pc.ClientSetRotation(unrealsdk.make_struct("Rotator", Pitch=int(pitch_deg * 65536 / 360) % 65536,
                                               Yaw=int(yaw_deg * 65536 / 360) % 65536, Roll=0))


def fp_place(x, y, z):
    pawn = get_pc().Pawn
    pawn.Location = unrealsdk.make_struct("Vector", X=x, Y=y, Z=z)
    pawn.SetPhysics(2)


def fp_vec(v):
    return [round(v.X, 2), round(v.Y, 2), round(v.Z, 2)]


def fp_rot(r):
    return [r.Pitch, r.Yaw, r.Roll]


def fp_weapon_state():
    """The held weapon's firing-relevant live values (reads only)."""
    pc = get_pc()
    w = pc.Pawn.Weapon
    if w is None:
        return None
    out = {"path": w._path_name(), "class": w.Class.Name}
    try:
        d = w.DefinitionData
        out["type"] = fp_deep(d.WeaponTypeDefinition)
        out["balance"] = fp_deep(d.BalanceDefinition)
        out["barrel"] = fp_deep(d.BarrelPartDefinition)
        out["elemental"] = fp_deep(d.ElementalPartDefinition)
        out["manufacturer"] = fp_deep(d.ManufacturerDefinition)
    except Exception:
        pass
    for key in ("ReloadCnt", "ClipSize", "ShotCost", "Spread", "InstantHitDamage", "ProjectilesPerShot", "FireInterval",
                "NextFiringPatternIndex", "ProjectileSpeedMultiplier", "AdditionalRicochets", "ExtraShotChance", "WeaponRange",
                "BarrelSpinUpPercent", "CurrentBurstShotCount", "AutomaticBurstCount"):
        try:
            out[key] = fp_deep(getattr(w, key))
        except Exception:
            pass
    try:
        out["accuracy_pool"] = round(pc.AccuracyPool.Data.CurrentValue, 4)
    except Exception:
        pass
    try:
        out["firing_mode"] = fp_deep(w.GetFiringModeDefinition(0))
    except Exception as error:
        out["firing_mode_error"] = repr(error)[:80]
    return out
