"""Weapons parity round 2 live probe (real game): world FOV, muzzle-flash and casing particles, impact spawns.

Load after weapon_firing_probe.py, phaselock.py and phaselock_matched.py (same command namespace; it uses their
helpers: deep, _path, _all_props, _selected_props, _out, _write_json, _get_pawn, _own_functions, _safe, _args, _log,
fp_state, ai_pawn). Loading only defines commands. Everything written is game data under RG_OUT (ignored local/).

  wp2_fov_arm()        hook the HUD's PostRender once; the next frame projects points at known view angles.
  wp2_fov_read()       remove that hook and return what it measured (ClipX/Y, the view FOV, projected points).
  wp2_particles(tag)   the held gun's type fields about muzzle flash, shell casing, tracer and impacts, and every
                       particle system they name (emitters, required module, modules, distributions expanded).
  wp2_explosions()     GD_Weap_Shared_Effects.Default_Elemental_Explosions and the explosion definitions it names.
  wp2_install_hooks()  log emitter-pool spawns, HurtRadius and the weapon's muzzle/shell/impact functions into the
                       current fp run (fp_run_begin / fp3_run_begin), with the game clock.
  wp2_autofire(n, on, off)  press and release fire on the tick, n times (on/off in seconds); wp2_autofire_state().
  wp2_try_level(name, level) try to set an AI pawn's level in memory; returns the level before and after.

Objects are looked up again in every command (see openwillow_realgame/__init__.py).
"""
import json
import math
import re
import time
from pathlib import Path

try:
    import unrealsdk
    from unrealsdk.hooks import Type, add_hook, remove_hook
except ImportError:  # importable outside the game
    unrealsdk = None
    Type = add_hook = remove_hook = None

WP2_ID = "ow_weapon_parity2"
WP2_TICK = "WillowGame.WillowGameViewportClient:Tick"
wp2_state = globals().setdefault("wp2_state", {"fov": None, "fov_hooks": [], "hooks": [], "auto": None})

# View directions (yaw, pitch in degrees, camera frame) projected through the HUD canvas.
FOV_DIRS = [(a, 0.0) for a in (-40, -30, -20, -10, 0, 10, 20, 30, 40)] + [(0.0, b) for b in (-25, -15, -5, 5, 15, 25)]
TYPE_TERMS = ("muzzle", "shell", "casing", "tracer", "impact", "flash", "eject")


def _out_file(folder, tag):
    """<RG_OUT>/<folder>/<tag>.json for a tag of letters, digits and underscores, checked to resolve under RG_OUT."""
    if not re.fullmatch(r"[A-Za-z0-9_]+", str(tag)):
        raise ValueError("tag must match [A-Za-z0-9_]+: %r" % (tag,))
    path = (_out(folder) / (tag + ".json")).resolve()
    root = Path(RG_OUT).resolve()
    if root not in path.parents:
        raise ValueError("output path %s is outside RG_OUT %s" % (path, root))
    return path


# ---------------------------------------------------------------- world FOV

def _rot_axes(pitch_u, yaw_u):
    p = pitch_u * math.pi / 32768.0
    y = yaw_u * math.pi / 32768.0
    fwd = (math.cos(p) * math.cos(y), math.cos(p) * math.sin(y), math.sin(p))
    right = (-math.sin(y), math.cos(y), 0.0)
    up = (fwd[1] * right[2] - fwd[2] * right[1], fwd[2] * right[0] - fwd[0] * right[2], fwd[0] * right[1] - fwd[1] * right[0])
    # UE is left-handed: up = right x fwd would point down; take the sign that has a positive Z at level pitch.
    if up[2] < 0:
        up = (-up[0], -up[1], -up[2])
    return fwd, right, up


def _fov_post_render(obj, args, ret, func):
    if wp2_state["fov"] is not None:
        return
    canvas = getattr(obj, "Canvas", None)
    if canvas is None:
        return
    pc = get_pc()
    pov = pc.PlayerCamera.CameraCache.POV
    loc, rot = pov.Location, pov.Rotation
    fwd, right, up = _rot_axes(rot.Pitch, rot.Yaw)
    rows = []
    for yaw, pitch in FOV_DIRS:
        ty, tp = math.tan(math.radians(yaw)), math.tan(math.radians(pitch))
        d = [fwd[i] + right[i] * ty + up[i] * tp for i in range(3)]
        world = unrealsdk.make_struct("Vector", X=loc.X + 1000 * d[0], Y=loc.Y + 1000 * d[1], Z=loc.Z + 1000 * d[2])
        try:
            s = canvas.Project(world)
            rows.append({"yaw": yaw, "pitch": pitch, "x": s.X, "y": s.Y, "z": s.Z})
        except Exception as error:
            rows.append({"yaw": yaw, "pitch": pitch, "error": repr(error)[:120]})
    wp2_state["fov"] = {"func": func.func._path_name(), "clip": [canvas.ClipX, canvas.ClipY],
                        "size": [getattr(canvas, "SizeX", None), getattr(canvas, "SizeY", None)],
                        "pov_fov": pov.FOV, "pov_loc": [loc.X, loc.Y, loc.Z], "pov_rot": [rot.Pitch, rot.Yaw, rot.Roll],
                        "pc_fov": _safe_call(pc, "GetFOVAngle"), "pc_FOVAngle": getattr(pc, "FOVAngle", None),
                        "default_fov": getattr(pc, "DefaultFOV", None), "rows": rows, "t": time.perf_counter()}


def wp2_fov_arm():
    wp2_state["fov"] = None
    hooked = []
    for path in ("WillowGame.WillowHUD:PostRender", "Engine.HUD:PostRender", "WillowGame.WillowHUDGFxMovie:Tick"):
        try:
            add_hook(path, Type.POST, WP2_ID + "_fov", _safe(_fov_post_render))
            hooked.append(path)
        except Exception:
            pass
    wp2_state["fov_hooks"] = hooked
    return {"hooked": hooked}


def wp2_fov_read():
    for path in wp2_state.get("fov_hooks", []):
        try:
            remove_hook(path, Type.POST, WP2_ID + "_fov")
        except Exception:
            pass
    wp2_state["fov_hooks"] = []
    result = wp2_state["fov"]
    if result:
        _write_json(_out("fov") / ("fov_%d.json" % int(time.time())), result)
    return result


# ---------------------------------------------------------------- particle systems and explosions

def _expand(value, depth=0):
    """deep() with Distribution*, ParticleModule* and ParticleLODLevel objects expanded one level at a time."""
    if _is_uobject(value):
        cls = value.Class.Name
        if depth < 6 and (cls.startswith("Distribution") or cls.startswith("ParticleModule") or cls == "ParticleLODLevel"
                          or cls.startswith("ParticleSpriteEmitter") or cls.endswith("Emitter")):
            out = {"_class": cls, "_path": _path(value)}
            for name in _field_names(value):
                try:
                    v = getattr(value, name)
                except Exception:
                    continue
                if callable(v) or name in ("Outer", "Class", "ObjectArchetype", "ObjectFlags", "Linker", "LinkerIndex",
                                           "HashNext", "HashOuterNext", "StateFrame", "NetIndex", "Name", "Index"):
                    continue
                out[name] = _expand(v, depth + 1)
            return out
        return _path(value)
    if _is_wrapped_struct(value):
        out = {}
        for prop in value._type._properties():
            try:
                out[prop.Name] = _expand(getattr(value, prop.Name), depth + 1)
            except Exception:
                continue
        return out
    if isinstance(value, (list, tuple)) or _is_wrapped_array(value):
        try:
            items = list(value)
        except Exception:
            return repr(value)[:200]
        return [_expand(v, depth + 1) for v in items[:64]]
    return deep(value)


def _particle_system(ps):
    if ps is None:
        return None
    out = {"path": _path(ps), "props": _selected_props(ps, ("SystemUpdateMode", "UpdateTime_FPS", "WarmupTime",
                                                            "bOrientZAxisTowardCamera", "SecondsBeforeInactive",
                                                            "Delay", "FixedRelativeBoundingBox", "LODDistances")),
           "emitters": []}
    for emitter in list(getattr(ps, "Emitters", []) or []):
        if emitter is None:
            continue
        row = {"class": emitter.Class.Name, "name": str(getattr(emitter, "EmitterName", "")),
               "props": _selected_props(emitter, ("EmitterRenderMode", "bUseLegacySpawningBehavior", "InitialAllocationCount",
                                                  "PeakActiveParticles", "SubUVDataOffset", "bCookedOut"))}
        lods = list(getattr(emitter, "LODLevels", []) or [])
        if lods:
            lod = lods[0]
            row["required"] = _expand(lod.RequiredModule)
            row["spawn"] = _expand(getattr(lod, "SpawnModule", None))
            row["type_data"] = _expand(getattr(lod, "TypeDataModule", None))
            row["modules"] = [_expand(m) for m in list(lod.Modules or [])]
        out["emitters"].append(row)
    return out


def _collect_ps(value, found):
    """Every ParticleSystem object reachable from a (deep-read) type field value, by walking the live object."""
    if _is_uobject(value):
        if value.Class.Name == "ParticleSystem":
            found[_path(value)] = value
        elif value.Class.Name.endswith("Definition") or value.Class.Name.endswith("Collection"):
            for name in _field_names(value):
                try:
                    v = getattr(value, name)
                except Exception:
                    continue
                if _is_uobject(v) and v.Class.Name == "ParticleSystem":
                    found[_path(v)] = v
                elif _is_wrapped_struct(v) or _is_wrapped_array(v):
                    _collect_ps(v, found)
        return
    if _is_wrapped_struct(value):
        for prop in value._type._properties():
            try:
                _collect_ps(getattr(value, prop.Name), found)
            except Exception:
                continue
    elif _is_wrapped_array(value) or isinstance(value, (list, tuple)):
        for v in list(value)[:256]:
            _collect_ps(v, found)


def wp2_particles(tag):
    pc, pawn = _get_pawn()
    weapon = pawn.Weapon
    if weapon is None:
        return {"error": "no weapon"}
    data = weapon.DefinitionData
    wtype = data.WeaponTypeDefinition
    fields, found = {}, {}
    for holder_name, holder in (("type", wtype), ("weapon", weapon)):
        for name in _field_names(holder):
            if any(t in name.lower() for t in TYPE_TERMS):
                try:
                    v = getattr(holder, name)
                except Exception:
                    continue
                if callable(v):
                    continue
                fields[holder_name + "." + name] = deep(v)
                _collect_ps(v, found)
    # Parts can carry their own muzzle templates (the barrel part's MuzzleFlashPSTemplates, for instance).
    for slot in ("BodyPartDefinition", "BarrelPartDefinition", "ElementalPartDefinition", "AccessoryPartDefinition",
                 "GripPartDefinition", "SightPartDefinition", "StockPartDefinition", "MaterialPartDefinition"):
        part = getattr(data, slot, None)
        if part is None:
            continue
        for name in _field_names(part):
            if any(t in name.lower() for t in TYPE_TERMS):
                try:
                    v = getattr(part, name)
                except Exception:
                    continue
                if not callable(v):
                    fields[slot + "." + name] = deep(v)
                    _collect_ps(v, found)
    result = {"weapon": _path(weapon), "type": _path(wtype), "name": _safe_call(weapon, "GetShortHumanReadableName"),
              "fields": fields, "systems": {p: _particle_system(ps) for p, ps in found.items()}}
    _write_json(_out_file("particles", tag), result)
    return {"type": result["type"], "fields": len(fields), "systems": sorted(found)}


def wp2_explosions():
    found = {}
    coll = unrealsdk.find_object("ExplosionCollectionDefinition", "GD_Weap_Shared_Effects.Default_Elemental_Explosions")
    result = {"collection": _all_props(coll) if coll is not None else None, "explosions": {}}
    if coll is not None:
        for name in _field_names(coll):
            try:
                v = getattr(coll, name)
            except Exception:
                continue
            if _is_wrapped_array(v):
                for entry in list(v):
                    for prop in entry._type._properties():
                        try:
                            obj = getattr(entry, prop.Name)
                        except Exception:
                            continue
                        if _is_uobject(obj):
                            found[_path(obj)] = obj
    for path, obj in found.items():
        row = _all_props(obj)
        result["explosions"][path] = {"class": obj.Class.Name, "props": row}
        ps = {}
        _collect_ps(obj, ps)
        result["explosions"][path]["systems"] = {p: _particle_system(s) for p, s in ps.items()}
    _write_json(_out("explosions") / "default_elemental_explosions.json", result)
    return {"collection": coll is not None, "explosions": sorted(found)}


# ---------------------------------------------------------------- impact and muzzle hooks

def _wp2_cb(kind):
    def callback(obj, args, ret, func):
        if fp_state.get("run") is None:
            return
        # The hook group goes under "hook": _log's own first argument is the row kind ("wp2"), so a "kind" key here made every
        # call a TypeError, which _safe logged as hook_error (run Z47, 2026-11-09: no wp2 row reached any run file).
        row = {"hook": kind, "func": func.func.Name, "obj": _path(obj), "args": _args(args)}
        if kind == "weapon":
            pawn = get_pc().Pawn
            if pawn is None or pawn.Weapon is None or _path(obj) != _path(pawn.Weapon):
                return
        _log("wp2", **row)
    return callback


def wp2_install_hooks():
    if wp2_state["hooks"]:
        return {"already": len(wp2_state["hooks"])}
    hooked, missing = [], []
    specs = [("EmitterPool", ("SpawnEmitter", "SpawnEmitterCustomLifetime", "SpawnEmitterMeshAttachment"), "emitter"),
             ("Actor", ("HurtRadius",), "hurt"),
             ("WillowExplosion", ("Explode", "PostBeginPlay", "SpawnExplosionParticleSystem"), "explosion")]
    for cls, names, kind in specs:
        try:
            own = _own_functions(cls)
        except Exception:
            own = {}
        for name in names:
            path = own.get(name)
            if path is None:
                missing.append(cls + ":" + name)
                continue
            add_hook(path, Type.PRE, WP2_ID, _safe(_wp2_cb(kind)))
            hooked.append(path)
    try:
        own = _own_functions("WillowWeapon")
    except Exception:
        own = {}
    for name, path in own.items():
        low = name.lower()
        if any(t in low for t in ("muzzle", "shell", "casing", "impacteffect", "tracer")):
            add_hook(path, Type.PRE, WP2_ID, _safe(_wp2_cb("weapon")))
            hooked.append(path)
    wp2_state["hooks"] = hooked
    return {"hooked": hooked, "missing": missing}


def wp2_remove_hooks():
    removed = 0
    for path in wp2_state.get("hooks", []):
        try:
            remove_hook(path, Type.PRE, WP2_ID)
            removed += 1
        except Exception:
            pass
    wp2_state["hooks"] = []
    wp2_stop_autofire()
    return {"removed": removed}


# ---------------------------------------------------------------- auto fire and levels

def _wp2_tick(*_args):
    auto = wp2_state.get("auto")
    if not auto or auto["done"]:
        return
    now = time.perf_counter()
    if now < auto["next"]:
        return
    pc = get_pc()
    if auto["down"]:
        pc.StopFire(0)
        auto["down"] = False
        auto["presses"] += 1
        auto["next"] = now + auto["off"]
        if auto["presses"] >= auto["n"]:
            auto["done"] = True
    else:
        pc.StartFire(0)
        auto["down"] = True
        auto["next"] = now + auto["on"]


def wp2_autofire(n=60, on=0.08, off=0.35):
    wp2_state["auto"] = {"n": int(n), "on": float(on), "off": float(off), "presses": 0, "down": False,
                         "next": time.perf_counter(), "done": False, "t0": time.perf_counter()}
    try:
        remove_hook(WP2_TICK, Type.POST_UNCONDITIONAL, WP2_ID + "_tick")
    except Exception:
        pass
    add_hook(WP2_TICK, Type.POST_UNCONDITIONAL, WP2_ID + "_tick", _wp2_tick)
    return {"armed": int(n)}


def wp2_autofire_state():
    auto = wp2_state.get("auto") or {}
    return {k: auto.get(k) for k in ("n", "presses", "done")}


def wp2_stop_autofire():
    try:
        remove_hook(WP2_TICK, Type.POST_UNCONDITIONAL, WP2_ID + "_tick")
    except Exception:
        pass
    auto = wp2_state.get("auto")
    if auto and auto.get("down"):
        get_pc().StopFire(0)
    wp2_state["auto"] = None
    return {"stopped": True}


def wp2_infinite_ammo(on=True):
    pc, pawn = _get_pawn()
    pawn.InvManager.bInfiniteAmmo = bool(on)
    return {"bInfiniteAmmo": pawn.InvManager.bInfiniteAmmo}


def wp2_try_level(name, level):
    """Memory-only: try the known level setters on an AI pawn. Returns which one changed GetExpLevel."""
    pawn = ai_pawn(name)
    before = _safe_call(pawn, "GetExpLevel")
    tried = {}
    for setter in ("SetExpLevel", "SetGameStage"):
        fn = getattr(pawn, setter, None)
        if fn is None:
            tried[setter] = "missing"
            continue
        try:
            fn(int(level))
            tried[setter] = _safe_call(ai_pawn(name), "GetExpLevel")
        except Exception as error:
            tried[setter] = repr(error)[:100]
        if _safe_call(ai_pawn(name), "GetExpLevel") == int(level):
            break
    return {"before": before, "after": _safe_call(ai_pawn(name), "GetExpLevel"), "tried": tried}


def wp2_status_table():
    g = unrealsdk.find_object("GlobalsDefinition", "GD_Globals.General.Globals")
    return {"StatusEffectChanceScales": deep(getattr(g, "StatusEffectChanceScales", None))} if g else None
