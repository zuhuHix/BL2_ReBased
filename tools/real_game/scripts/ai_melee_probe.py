"""AI targeting and melee probe for the openwillow_realgame command channel (lane L8, 2026-10-09).

Run through Invoke-GamePyFile (tools/real_game/realgame.ps1) AFTER tools/real_game/scripts/firing_probe.py (it uses that file's fp_deep,
fp_props, fp_vec, fp_place, fp_live_ai, fp_find_ai, fp_hook, fp_dump and the FP row store) with the level-8 Maya loaded and saves blocked
from the first frame. Our own code, no game data. Everything it writes is game data and goes under the folder in `RG_OUT` (ignored
local/realgame/...). Memory-only: reads, hooks, and the stand-ins the caller makes by hand (placing the player, blocking the player's
TakeDamage, spawning a gun).

Data:
- am_targeting(path): a TargetingDefinition with its Knowledge and Prioritization iterator objects (class and every property; an
  entry the game holds as None stays None).
- am_aidef(path): an AIDefinition (properties, NodeList as plain data, the Action object of each node with its properties).
- am_comp(pawn): the pawn's WillowAIComponent (pawn.MyWillowMind.AIComponent).
- am_record(rec): every property of a MindTargetInfo record as plain data (the iterator fields included).
- am_snapshot(pawn) / am_summary(name) / am_all(): the component's scalars (UpdateRate, the two update times, CurrentTarget, holds),
  the record of the player (every field, or a compact row of the distances, exposure, threat and priority) for one or all live AI pawns.
- am_adults(): the live adult bullymongs (AIClass name PrimalBeastAdult) near the player, nearest first.

Time series and controlled steps (all hook the viewport Tick):
- am_place_xy(name, distance, bearing_deg, up, hang) / am_release(): put the player at a horizontal distance from an AI pawn.
- am_approach(name, distances, dt, bearing_deg, up, key, watch, full): step the player (hung) through horizontal distances from an
  AI pawn, one step every dt game seconds, logging am_summary (and with full every record field) before each move.
- am_melee_grid(attacker_name, mdef, configs): controlled swings. The player is put at (distance, bearing from the attacker's facing,
  height) configs, then mdef.MeleeAttack(attacker) is called inside a Tick two frames later and the geometry is logged as it is at the
  call; hits are the player-side TakeDamage calls the attacker's mind delivered with that definition (others: foreign_hits).
  am_grid_stop(key) restores the attacker's physics and rotation rate and puts the player back where she was.
- am_track(names): every frame, per named AI pawn, LastTargetUpdateTime, LastStateUpdateTime, CurrentTarget, the record count, the first
  record's Distance2D/Priority and both positions into AM['frames'] with the game clock (capped at AM['cap'] rows: dump and clear it).
- am_hooks(): hooks (PRE and POST) for the attack chain (TriggerBehavior, Behavior_MeleeAttack.ApplyBehaviorToContext,
  MeleeDefinition.MeleeAttack, AIDefinition.OnTargetAcquired/OnTargetLost, WillowActionSequencePawn.ActivateEvent, TakeDamage on the
  player) into AM['rows'], with the object's Outer and, for an Action, the pawn it runs for. Natives that the game calls from C++
  (the two AIDefinition target events were never seen) are invisible to these hooks.
- am_dump(name, data=None), am_clear(), am_unhook().

Traps found while writing it: writing Pawn.Location leaves the actor in its old spatial-hash cell, so melee volume queries and target
searches do not see the new place until something moves it; call ForceUpdateComponents(False, False) after the write (Move and
SetLocation fail on the player here). A hung player (physics none) released inside terrain falls out of the world: restore her place.
"""
import json
import math
import time

import unrealsdk
from unrealsdk.hooks import Type, add_hook, remove_hook

AM = globals().get("AM") or {"rows": [], "frames": [], "hooks": [], "on": False, "tracked": [], "cap": 80000}


def am_comp(pawn):
    mind = getattr(pawn, "MyWillowMind", None)
    return getattr(mind, "AIComponent", None) if mind is not None else None


def am_targeting(path):
    t = unrealsdk.find_object("TargetingDefinition", path)
    out = {"path": path, "props": fp_props(t), "Knowledge": [], "Prioritization": []}
    for key in ("Knowledge", "Prioritization"):
        for it in list(getattr(t, key)):
            if it is None:
                out[key].append(None)
                continue
            out[key].append({"path": it._path_name(), "class": it.Class.Name, "props": fp_props(it)})
    return out


def am_aidef(path):
    d = unrealsdk.find_object("AIDefinition", path) if "." in path else None
    out = {"path": path, "class": d.Class.Name, "props": fp_props(d), "nodes": []}
    for i, node in enumerate(list(d.NodeList)):
        row = fp_deep(node)
        act = getattr(node, "Action", None)
        if act is not None:
            row["_action"] = {"path": act._path_name(), "class": act.Class.Name, "props": fp_props(act)}
        row["_index"] = i
        out["nodes"].append(row)
    return out


def am_record(rec):
    return fp_props(rec)


def _vec(v):
    return [round(v.X, 2), round(v.Y, 2), round(v.Z, 2)]


def am_snapshot(pawn, records=True):
    comp = am_comp(pawn)
    out = {"pawn": pawn.Name, "loc": _vec(pawn.Location), "g": _game_t()}
    if comp is None:
        out["comp"] = None
        return out
    for key in ("UpdateRate", "LastStateUpdateTime", "LastTargetUpdateTime", "TickThrottleTime", "LastTickTime", "bInitialized"):
        try:
            out[key] = fp_deep(getattr(comp, key))
        except Exception as error:
            out[key] = "error " + repr(error)[:60]
    for key in ("CurrentTarget", "ScriptedTarget", "AIDef", "TargetingDef", "MindTargetInfoClass"):
        try:
            out[key] = fp_deep(getattr(comp, key))
        except Exception:
            pass
    for key in ("Holds", "TargetHolds", "MovementHolds"):
        try:
            out[key] = len(list(getattr(comp, key)))
        except Exception:
            pass
    if records:
        out["records"] = [am_record(r) for r in list(comp.Targets) if r is not None]
    else:
        out["n_records"] = len(list(comp.Targets))
    return out


def am_summary(name):
    """One compact row: the pawn's current target, update times and its record of the player (distances, exposure, priority)."""
    pawn = fp_find_ai(name)
    comp = am_comp(pawn)
    me = get_pc().Pawn.Location
    out = {"g": _game_t(), "name": name, "d2": round(math.hypot(pawn.Location.X - me.X, pawn.Location.Y - me.Y), 1),
           "dz": round(me.Z - pawn.Location.Z, 1), "cur": fp_deep(comp.CurrentTarget), "lt": round(comp.LastTargetUpdateTime, 3),
           "ls": round(comp.LastStateUpdateTime, 3), "hp": round(pawn.GetHealth(), 1)}
    try:
        out["irr"] = bool(pawn.MyWillowMind.bDisabledDueToPopulationIrrelevance)
        out["tick"] = round(comp.LastTickTime, 3)
        out["deleted"] = bool(pawn.bDeleteMe)
    except Exception:
        pass
    recs = [r for r in list(comp.Targets) if r is not None]
    out["n"] = len(recs)
    for r in recs[:1]:
        for key in ("Distance", "Distance2D", "DistanceVertical", "DotToTarget", "TargetExposureToMe", "Exposure", "ThreatLevel", "Priority",
                    "OnePriority", "bCanSeeOrHearTarget", "bCanFireAt", "BonusPriority", "TotalDamagePct", "AggroTime", "ForgetTime"):
            out[key] = fp_deep(getattr(r, key))
    try:
        out["state"] = fp_deep(comp.CurrentState)
    except Exception:
        pass
    return out


def am_all(radius=60000.0):
    """am_summary of every live AI pawn that has a component (nearest first)."""
    out = []
    for r in fp_live_ai(radius):
        try:
            out.append(am_summary(r["name"]))
        except Exception:
            pass
    return out


def am_adults(radius=20000.0):
    return [r for r in fp_live_ai(radius) if "PrimalBeastAdult" in str(_ai_class(fp_find_ai(r["name"])))]


def _ai_class(pawn):
    mind = getattr(pawn, "MyWillowMind", None)
    return fp_deep(getattr(mind, "AIClass", None)) if mind is not None else None


def _game_t():
    try:
        return round(get_pc().WorldInfo.TimeSeconds, 4)
    except Exception:
        return None


def am_place_xy(name, distance, bearing_deg=180.0, up=600.0, hang=False):
    """Put the player `distance` units (horizontally) from the AI pawn, `up` above its feet, facing it. Falling afterwards, or hung
    in place (physics none) when hang is set; am_release() lets her fall. Script stand-in, memory only."""
    target = fp_find_ai(name)
    pc = get_pc()
    t = target.Location
    a = math.radians(bearing_deg)
    pc.Pawn.Location = unrealsdk.make_struct("Vector", X=t.X + math.cos(a) * distance, Y=t.Y + math.sin(a) * distance, Z=t.Z + up)
    pc.Pawn.Velocity = unrealsdk.make_struct("Vector", X=0.0, Y=0.0, Z=0.0)
    pc.Pawn.SetPhysics(0 if hang else 2)
    fp_aim_at(name, up=0.0)
    return _vec(pc.Pawn.Location)


def am_release():
    get_pc().Pawn.SetPhysics(2)


def am_approach(name, distances, dt=0.5, bearing_deg=180.0, up=300.0, key="appr", watch=(), full=False):
    """Step the player (hung in place, physics none) through the given horizontal distances from the AI pawn, one every `dt` game
    seconds, and log am_summary(name) (and of every pawn in `watch`; with full, every field of each target record) just before each move into AM[key] (AM[key]['done'] when finished; she is released to falling physics at the end)."""
    st = {"i": 0, "next": 0.0, "done": False, "rows": []}
    AM[key] = st

    def tick(*_a):
        if st["done"] or not AM["on"]:
            return
        now = _game_t()
        if now is None or now < st["next"]:
            return
        pawn = fp_find_ai(name)
        if pawn is None:
            st["done"] = True
            st["error"] = "pawn gone"
            return
        if st["i"] > 0:
            row = {"step": distances[st["i"] - 1], "summary": am_summary(name)}
            if full:
                recs = [r for r in list(am_comp(pawn).Targets) if r is not None]
                row["records"] = [am_record(r) for r in recs]
            for w in watch:
                try:
                    row["watch_" + w] = am_summary(w)
                except Exception:
                    pass
            st["rows"].append(row)
        if st["i"] >= len(distances):
            st["done"] = True
            get_pc().Pawn.SetPhysics(2)
            return
        d = distances[st["i"]]
        a = math.radians(bearing_deg)
        t = pawn.Location
        pc = get_pc()
        pc.Pawn.Location = unrealsdk.make_struct("Vector", X=t.X + math.cos(a) * d, Y=t.Y + math.sin(a) * d, Z=t.Z + up)
        pc.Pawn.Velocity = unrealsdk.make_struct("Vector", X=0.0, Y=0.0, Z=0.0)
        pc.Pawn.SetPhysics(0)
        st["i"] += 1
        st["next"] = now + dt
    add_hook("WillowGame.WillowGameViewportClient:Tick", Type.POST_UNCONDITIONAL, "ow_am_" + key, tick)
    AM["hooks"].append(("WillowGame.WillowGameViewportClient:Tick", Type.POST_UNCONDITIONAL, "ow_am_" + key))
    return st


def am_melee_grid(attacker_name, mdef, configs, key="grid"):
    """Controlled swings (see the module docstring). For each (dist, ang_deg, dz) the player is hung there relative to the attacker's
    facing (positive angle = counter-clockwise seen from above), ForceUpdateComponents re-registers her, and after two frames
    mdef.MeleeAttack(attacker) is called inside the Tick hook. The attacker is held (physics none, rotation rate zero) for the run, but
    the game still turns it, so judge by the measured bearing and distances in each row, not by the commanded ones. Results in
    AM[key]['rows']. Script stand-in for the swing; memory only."""
    att = fp_find_ai(attacker_name)
    mdef_path = mdef._path_name()
    st = {"i": 0, "phase": 0, "rows": [], "done": False, "mark": 0, "attacker": attacker_name,
          "saved": {"physics": att.Physics, "rate": fp_deep(att.RotationRate)}}
    AM[key] = st
    me = get_pc().Pawn.Location
    st["saved"]["me"] = [me.X, me.Y, me.Z]
    att.SetPhysics(0)
    att.RotationRate = unrealsdk.make_struct("Rotator", Pitch=0, Yaw=0, Roll=0)

    def tick(*_a):
        if st["done"] or not AM["on"]:
            return
        pc = get_pc()
        pawn = fp_find_ai(attacker_name)
        if pawn is None:
            st["done"] = True
            st["error"] = "attacker gone"
            return
        if st["i"] >= len(configs):
            am_grid_stop(key)
            return
        dist, ang, dz = configs[st["i"]]
        if st["phase"] == 0:
            yaw = pawn.Rotation.Yaw * 360.0 / 65536.0
            r = math.radians(yaw + ang)
            t = pawn.Location
            pc.Pawn.Location = unrealsdk.make_struct("Vector", X=t.X + math.cos(r) * dist, Y=t.Y + math.sin(r) * dist, Z=t.Z + dz)
            pc.Pawn.Velocity = unrealsdk.make_struct("Vector", X=0.0, Y=0.0, Z=0.0)
            pc.Pawn.SetPhysics(0)
            pc.Pawn.ForceUpdateComponents(False, False)
            st["phase"] = 1
        elif st["phase"] == 1:
            st["phase"] = 2
        elif st["phase"] == 2:
            att_loc, me = pawn.Location, pc.Pawn.Location
            yaw = pawn.Rotation.Yaw * 360.0 / 65536.0
            dx, dy = me.X - att_loc.X, me.Y - att_loc.Y
            bearing = (math.degrees(math.atan2(dy, dx)) - yaw + 180.0) % 360.0 - 180.0
            st["geo"] = {"dist": dist, "ang": ang, "dz": dz, "yaw": round(yaw, 3), "att": _vec(att_loc), "me": _vec(me),
                         "d2": round(math.hypot(dx, dy), 2), "dz_real": round(me.Z - att_loc.Z, 2), "d3": round(math.dist((me.X, me.Y, me.Z), (att_loc.X, att_loc.Y, att_loc.Z)), 2),
                         "bearing": round(bearing, 2), "g": _game_t()}
            st["mark"] = len(AM["rows"])
            AM["rows"].append({"tag": "grid_call", "i": st["i"], "g": _game_t()})
            mdef.MeleeAttack(pawn)
            st["phase"] = 3
        else:
            new = AM["rows"][st["mark"]:]
            allhits = [r for r in new if r.get("func", "").endswith("TakeDamage") and r["tag"] == "pre"]
            mind = pawn.MyWillowMind._path_name()
            hits = [r for r in allhits if r["args"].get("InstigatedBy") == mind and r["args"].get("DamageCauser") == mdef_path]
            row = dict(st["geo"], hit=len(hits), foreign_hits=len(allhits) - len(hits), n_rows=len(new))
            if hits:
                a0 = hits[0]["args"]
                row["damage"] = a0.get("Damage")
                row["momentum"] = a0.get("Momentum")
                row["hit_loc"] = a0.get("HitLocation")
                row["dtype"] = a0.get("DamageType")
                row["causer"] = a0.get("DamageCauser")
            st["rows"].append(row)
            st["i"] += 1
            st["phase"] = 0
    add_hook("WillowGame.WillowGameViewportClient:Tick", Type.POST_UNCONDITIONAL, "ow_am_" + key, tick)
    AM["hooks"].append(("WillowGame.WillowGameViewportClient:Tick", Type.POST_UNCONDITIONAL, "ow_am_" + key))
    return st


def am_grid_stop(key="grid"):
    st = AM.get(key)
    if not st:
        return
    st["done"] = True
    att = fp_find_ai(st.get("attacker", ""))
    if att is not None:
        att.SetPhysics(st["saved"]["physics"])
        rate = st["saved"]["rate"]
        att.RotationRate = unrealsdk.make_struct("Rotator", Pitch=rate["Pitch"], Yaw=rate["Yaw"], Roll=rate["Roll"])
    pc = get_pc()
    x, y, z = st["saved"]["me"]
    pc.Pawn.Location = unrealsdk.make_struct("Vector", X=x, Y=y, Z=z)
    pc.Pawn.Velocity = unrealsdk.make_struct("Vector", X=0.0, Y=0.0, Z=0.0)
    pc.Pawn.SetPhysics(2)
    pc.Pawn.ForceUpdateComponents(False, False)


def am_track(names):
    """Sample the named pawns' components every frame (call once; AM['frames'])."""
    AM["tracked"] = list(names)

    def tick(*_a):
        if not AM["on"] or len(AM["frames"]) >= AM["cap"]:
            return
        now = _game_t()
        for name in AM["tracked"]:
            pawn = fp_find_ai(name)
            comp = am_comp(pawn) if pawn is not None else None
            if comp is None:
                continue
            row = {"g": now, "n": name, "lt": round(comp.LastTargetUpdateTime, 4), "ls": round(comp.LastStateUpdateTime, 4),
                   "tgt": fp_deep(comp.CurrentTarget), "pos": _vec(pawn.Location), "me": _vec(get_pc().Pawn.Location)}
            try:
                recs = [r for r in list(comp.Targets) if r is not None]
                row["nrec"] = len(recs)
                if recs:
                    row["d2"] = round(recs[0].Distance2D, 1)
                    row["pri"] = round(recs[0].Priority, 4)
            except Exception:
                pass
            AM["frames"].append(row)
    add_hook("WillowGame.WillowGameViewportClient:Tick", Type.POST_UNCONDITIONAL, "ow_am_track", tick)
    AM["hooks"].append(("WillowGame.WillowGameViewportClient:Tick", Type.POST_UNCONDITIONAL, "ow_am_track"))


HOOK_TARGETS = [("GearboxFramework.ActionSequence", "TriggerBehavior"), ("WillowGame.Behavior_MeleeAttack", "ApplyBehaviorToContext"),
                ("WillowGame.MeleeDefinition", "MeleeAttack"), ("GearboxFramework.AIDefinition", "OnTargetAcquired"),
                ("GearboxFramework.AIDefinition", "OnTargetLost"), ("WillowGame.WillowActionSequencePawn", "ActivateEvent"),
                ("WillowGame.WillowPlayerPawn", "TakeDamage")]


def _arg_dict(args, func):
    out = {}
    try:
        for q in func.func._properties():
            if hasattr(args, q.Name):
                out[q.Name] = fp_deep(getattr(args, q.Name), 4)
    except Exception as error:
        out["_error"] = repr(error)[:100]
    return out


def _row_hook(path, tag):
    def hook(obj, args, ret, func):
        if not AM["on"] or len(AM["rows"]) >= AM["cap"]:
            return
        row = {"g": _game_t(), "tag": tag, "func": path, "obj": fp_deep(obj, 6), "args": _arg_dict(args, func)}
        try:
            row["outer"] = obj.Outer._path_name()
        except Exception:
            pass
        try:  # an Action carries the pawn it runs for
            row["pawn"] = obj.MyWillowPawn.Name
        except Exception:
            pass
        if tag == "post":
            try:
                row["ret"] = fp_deep(ret, 3)
            except Exception:
                row["ret"] = "?"
        try:
            if path.endswith("Behavior_MeleeAttack:ApplyBehaviorToContext") or path.endswith("MeleeDefinition:MeleeAttack"):
                md = obj if obj.Class.Name == "MeleeDefinition" else getattr(obj, "MeleeDefinition", None)
                row["melee_def"] = fp_deep(md)
        except Exception:
            pass
        AM["rows"].append(row)
    return hook


def am_hooks(targets=None, post=True):
    """Hook each (class path, function) PRE (and POST) with a row: game time, object, arguments. Class paths as
    'Package.Class' for add_hook's 'Package.Class:Function' form. Returns the hooks that were refused."""
    AM["on"] = True
    failed = []
    for cls, fn in (targets or HOOK_TARGETS):
        path = f"{cls}:{fn}"
        for typ, tag in ((Type.PRE, "pre"),) + (((Type.POST, "post"),) if post else ()):
            ident = f"ow_am_{tag}_{path}"
            try:
                add_hook(path, typ, ident, _row_hook(path, tag))
                AM["hooks"].append((path, typ, ident))
            except Exception as error:
                failed.append(f"{path}: {repr(error)[:80]}")
    return failed


def am_unhook():
    AM["on"] = False
    for path, typ, ident in AM["hooks"]:
        try:
            remove_hook(path, typ, ident)
        except Exception:
            pass
    AM["hooks"] = []


def am_clear():
    AM["rows"] = []
    AM["frames"] = []


def am_dump(name, data=None):
    path = RG_OUT + "/" + name
    with open(path, "w", encoding="utf-8") as handle:
        json.dump(AM["rows"] if data is None else data, handle, default=str)
    return {"path": path, "rows": len(AM["rows"]) if data is None else None}
