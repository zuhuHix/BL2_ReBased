"""AI awareness probe for the openwillow_realgame command channel (lane L9, 2026-10-09).

Run through Invoke-GamePyFile (tools/real_game/realgame.ps1) AFTER firing_probe.py and ai_melee_probe.py (it uses their fp_* and am_*
helpers and the AM row store) with the level-8 Maya loaded in Three Horns Divide and saves blocked from the first frame. Our own code, no
game data; everything it writes is game data and goes under the folder in `RG_OUT` (ignored local/realgame/...). Memory-only: reads, hooks,
and the script stand-ins the caller makes by hand (placing the player with `Pawn.Location`, holding an AI pawn with physics none, a script
`TakeDamage`, clearing one test mob's allegiance, setting `bCloaked`), every one of which is restored by the helper that makes it.

Data (reads):
- aw_den(pawn): the pawn's parent component and den (threat radius, patrol radius, patrol flag, shared dens, members, the den definition's engagement and
  provoked times). Every bullymong sampled in Three Horns Divide has a den parent, and the threat rules read the den, not the pawn.
- aw_cfg(pawn): the values the threat rule reads (SightViewCone, MinAggroTimeToConsiderAwareOfTarget, AICloseThreatRadius, TargetSearchRadius, collision
  radius, den). aw_rec(pawn, root=False/True): the player's record in the pawn's own component or in the den's (where SetThreatLevel writes).
- aw_row(pawn): one state row (geometry to the pawn and to the den, both records, GetThreatLevel, IsAwareOfTarget, the player's targeting counter and one
  row per den member from aw_members).

Time series (all hook the viewport Tick; each returns at once and fills AW['runs'][key]):
- aw_walk(name, bearing, legs, ..., center='pawn'|'den'): move the player along a line (horizontal distance legs, speed in uu/s), log a row per change and a
  compact row per frame. aw_dwell_den(name, configs): put the player at (world bearing, distance from the den, height) configs, resting far between them
  until the den's level for her is below 2 (a natural reset). aw_timeline(name, steps, total): scripted placements on a clock. aw_dwell(name, configs, yaw):
  the same per pawn, with its facing locked.
- aw_melee_over_time(name, mdef_path, ...): call MeleeDefinition.MeleeAttack(pawn) from a Tick hook and log the pawn's MeleeState every frame and the hits
  on the player (needs am_hooks on WillowPlayerPawn.TakeDamage); aw_trace_source(name): GetMeleeTraceSourceLocationAndRotation next to Location/Rotation.
- aw_tree_layout(name) / aw_tree(name, seconds): the pawn's tree as plain data, and every change of a node's run-state fields and of the running path.
- aw_valid_probe(name, other) / aw_allegiance_probe(name): AIComponent.ValidTarget answers (the cloak flag and the allegiance are changed in memory for the
  call and put back). aw_dump(name, data), aw_walk_state(key), aw_run_stop(key), aw_unhook().

Script stand-ins (memory only): placing the player (Pawn.Location, hung with physics none), holding or turning an AI pawn (aw_hold, aw_face), writing a
den-level threat record (done once by hand, outside this file), calling NotifyAttackedBy or MeleeAttack from script, and turning a pawn's yaw.

Traps (from lane L8, kept): writing Pawn.Location does not re-register the actor, so call ForceUpdateComponents(False, False) after every write; a hung
player released inside terrain falls out of the world (call aw_rescue); AI beyond about 10 km despawn.
"""
import json
import math

import unrealsdk
from unrealsdk.hooks import Type, add_hook, remove_hook

AW = globals().get("AW") or {"hooks": [], "runs": {}, "cap": 200000}


def _vec(v):
    return [round(v.X, 2), round(v.Y, 2), round(v.Z, 2)]


def _g():
    try:
        return round(get_pc().WorldInfo.TimeSeconds, 4)
    except Exception:
        return None


def _yaw_deg(pawn):
    return pawn.Rotation.Yaw * 360.0 / 65536.0


def aw_den(pawn):
    """The den above the pawn's AI component, if any: the parent component's owner and what the threat rule reads from it."""
    comp = am_comp(pawn)
    out = {"parent": None, "den": None}
    try:
        par = comp.GetParentComponent()
    except Exception as error:
        out["error"] = repr(error)[:80]
        return out
    if par is None:
        return out
    out["parent"] = par._path_name()
    den = par.Owner
    out["den"] = den._path_name()
    out["loc"] = _vec(den.Location)
    for key in ("bPatrolDen", "PatrolRadius", "ThreatRadius", "SpawnRadius", "bAlwaysSeeInThreatRadius", "MaxActiveActorsIsNormal", "MaxActiveActorsThreatened"):
        try:
            out[key] = fp_deep(getattr(den, key))
        except Exception:
            pass
    out["shared"] = [fp_deep(d) for d in list(den.SharedThreatDens)]
    out["members"] = [fp_deep(d) for d in list(den.SpawnedDenMembers)]
    dd = par.AIDef
    if dd is not None:
        out["def"] = {"class": dd.Class.Name, "MinEngagementTime": getattr(dd, "MinEngagementTime", None),
                      "MinProvokedTime": getattr(dd, "MinProvokedTime", None), "TargetSearchRadius": getattr(dd, "TargetSearchRadius", None)}
    return out


def aw_cfg(pawn):
    mind = pawn.MyWillowMind
    comp = am_comp(pawn)
    g = unrealsdk.find_object("GlobalsDefinition", "GD_Globals.General.Globals")
    out = {"pawn": pawn.Name, "ai_class": _ai_class(pawn), "SightViewCone": mind.SightViewCone,
           "MinAggroTimeToConsiderAwareOfTarget": mind.MinAggroTimeToConsiderAwareOfTarget, "AICloseThreatRadius": g.AICloseThreatRadius,
           "TargetSearchRadius": comp.AIDef.TargetSearchRadius, "bGetTargetsFromParents": comp.AIDef.bGetTargetsFromParents,
           "UpdateRate": comp.UpdateRate, "bProvoked": mind.bProvoked, "ProvokedLocation": _vec(mind.ProvokedLocation),
           "collision_radius": pawn.CollisionComponent.CollisionRadius if pawn.CollisionComponent else None,
           "allegiance": fp_deep(pawn.Allegiance), "den": aw_den(pawn)}
    return out


def aw_rec(pawn, comp=None, root=False):
    """The record of the player in the pawn's own component, or (root) in its parent component (the den's), where SetThreatLevel writes."""
    comp = comp or am_comp(pawn)
    if root:
        comp = comp.GetParentComponent()
        if comp is None:
            return None
    me = get_pc().Pawn
    for r in list(comp.Targets):
        if r is not None and r.Target == me:
            return r
    return None


_REC_KEYS = ("ThreatLevel", "AggroTime", "ForgetTime", "Exposure", "TargetExposureToMe", "bCanSeeOrHearTarget", "bCanFireAt", "Distance", "Distance2D",
             "DistanceVertical", "DotToTarget", "DotFromTarget", "LeftDotToTarget", "DotTargetFacing", "Priority", "BonusPriority", "ExposureChangeTime")


def aw_rec_data(rec):
    return None if rec is None else {k: fp_deep(getattr(rec, k)) for k in _REC_KEYS if hasattr(rec, k)}


def _den_geom(pawn, comp, me):
    """Horizontal distance from the den (and the threat origin the rule would use) to the player."""
    out = {}
    try:
        par = comp.GetParentComponent()
        den = par.Owner if par is not None else None
    except Exception:
        den = None
    mind = pawn.MyWillowMind
    if den is not None and not den.bPatrolDen:
        o = den.Location
        out["origin"] = "den"
        out["radius"] = den.ThreatRadius
        out["patrol_limit"] = den.PatrolRadius
    else:
        o = mind.ProvokedLocation if mind.bProvoked else pawn.Location
        out["origin"] = "pawn"
        out["radius"] = comp.AIDef.TargetSearchRadius
        out["patrol_limit"] = None
    out["d_origin2"] = round(math.hypot(me.X - o.X, me.Y - o.Y), 2)
    return out


def aw_members(pawn):
    """One compact row per member of the pawn's den (itself included): name, class, 3D distance to the player, horizontal distance of the player from the
    den, the angle of the player from the member's facing, the member's own record of the player (threat, exposure, the game's DotToTarget) and whether it
    has the player as its current target."""
    comp = am_comp(pawn)
    par = comp.GetParentComponent()
    if par is None:
        return []
    den = par.Owner
    me = get_pc().Pawn
    ml = me.Location
    out = []
    for m in list(den.SpawnedDenMembers):
        if m is None or m.bDeleteMe:
            continue
        mc = am_comp(m)
        if mc is None:
            continue
        loc = m.Location
        dx, dy, dz = ml.X - loc.X, ml.Y - loc.Y, ml.Z - loc.Z
        ang = (math.degrees(math.atan2(dy, dx)) - _yaw_deg(m) + 180.0) % 360.0 - 180.0
        rec = aw_rec(m, mc)
        out.append([m.Name, str(_ai_class(m)).split(".")[-1][10:], round(math.dist((dx, dy, dz), (0, 0, 0)), 1), round(math.hypot(ml.X - den.Location.X, ml.Y - den.Location.Y), 1),
                    round(ang, 2), fp_deep(rec.ThreatLevel) if rec else None, fp_deep(rec.Exposure) if rec else None, round(rec.DotToTarget, 4) if rec else None,
                    mc.CurrentTarget is not None, round(m.CollisionComponent.CollisionRadius if m.CollisionComponent else 0.0, 1), round(math.hypot(dx, dy), 1)])
    return out


def aw_row(pawn):
    comp = am_comp(pawn)
    mind = pawn.MyWillowMind
    me = get_pc().Pawn
    loc, ml = pawn.Location, me.Location
    yaw = _yaw_deg(pawn)
    dx, dy = ml.X - loc.X, ml.Y - loc.Y
    d2 = math.hypot(dx, dy)
    ang = (math.degrees(math.atan2(dy, dx)) - yaw + 180.0) % 360.0 - 180.0
    row = {"g": _g(), "d2": round(d2, 2), "dz": round(ml.Z - loc.Z, 2), "d3": round(math.dist((dx, dy, ml.Z - loc.Z), (0, 0, 0)), 2), "ang": round(ang, 2),
           "cos": round(math.cos(math.radians(ang)), 5), "yaw": round(yaw, 2), "me": _vec(ml), "pawn": _vec(loc),
           "cur": fp_deep(comp.CurrentTarget) if comp.CurrentTarget is not None else None, "prov": bool(mind.bProvoked), "n": len(list(comp.Targets))}
    row.update(_den_geom(pawn, comp, ml))
    rec = aw_rec(pawn, comp)
    row["rec"] = aw_rec_data(rec)
    try:
        row["root"] = aw_rec_data(aw_rec(pawn, comp, root=True))
    except Exception as error:
        row["root"] = "err " + repr(error)[:50]
    try:
        row["gt"] = int(comp.GetThreatLevel(me))
    except Exception as error:
        row["gt"] = "err " + repr(error)[:50]
    try:
        row["aware"] = bool(mind.IsAwareOfTarget(me))
    except Exception as error:
        row["aware"] = "err " + repr(error)[:50]
    try:
        row["ntarg"] = me.NumAICurrentlyTargeting
    except Exception:
        pass
    try:
        row["mem"] = aw_members(pawn)
    except Exception as error:
        row["mem"] = "err " + repr(error)[:60]
    return row


def _set_yaw(pawn, yaw_deg):
    pawn.Rotation = unrealsdk.make_struct("Rotator", Pitch=0, Yaw=int(yaw_deg * 65536.0 / 360.0) % 65536, Roll=0)


def aw_place(pawn, bearing, dist, dz=40.0, yaw=None):
    """Put the player `dist` horizontally from the pawn, `bearing` degrees from `yaw` (default: the pawn's current facing; positive =
    counter-clockwise from above), hung."""
    pc = get_pc()
    r = math.radians((_yaw_deg(pawn) if yaw is None else yaw) + bearing)
    t = pawn.Location
    pc.Pawn.Location = unrealsdk.make_struct("Vector", X=t.X + math.cos(r) * dist, Y=t.Y + math.sin(r) * dist, Z=t.Z + dz)
    pc.Pawn.Velocity = unrealsdk.make_struct("Vector", X=0.0, Y=0.0, Z=0.0)
    pc.Pawn.SetPhysics(0)
    pc.Pawn.ForceUpdateComponents(False, False)


def aw_far(name, dist=9000.0, bearing=0.0, up=400.0, yaw=None):
    pawn = fp_find_ai(name)
    aw_place(pawn, bearing, dist, up, yaw)
    return _vec(get_pc().Pawn.Location)


def aw_rescue(name, dist=900.0, up=250.0):
    pawn = fp_find_ai(name)
    aw_place(pawn, 0.0, dist, up)
    get_pc().Pawn.SetPhysics(2)
    get_pc().Pawn.ForceUpdateComponents(False, False)


def aw_hold(name, on=True):
    """Hold the pawn in place (physics none, rotation rate zero) or give it back; returns what to restore."""
    pawn = fp_find_ai(name)
    key = "hold_" + name
    if on:
        AW[key] = {"physics": pawn.Physics, "rate": fp_deep(pawn.RotationRate)}
        pawn.SetPhysics(0)
        pawn.RotationRate = unrealsdk.make_struct("Rotator", Pitch=0, Yaw=0, Roll=0)
    else:
        saved = AW.get(key)
        if saved:
            pawn.SetPhysics(saved["physics"])
            rate = saved["rate"]
            pawn.RotationRate = unrealsdk.make_struct("Rotator", Pitch=rate["Pitch"], Yaw=rate["Yaw"], Roll=rate["Roll"])
    return AW.get(key)


def aw_away_yaw(name):
    """World yaw of the direction from the pawn's den to the pawn (continuing it leads away from the den); the den's location is the threat origin."""
    pawn = fp_find_ai(name)
    den = am_comp(pawn).GetParentComponent().Owner.Location
    return round(math.degrees(math.atan2(pawn.Location.Y - den.Y, pawn.Location.X - den.X)), 3)


def aw_face(name, yaw_deg):
    """Turn the (held) pawn to a world yaw: a script stand-in for choosing its facing; memory only."""
    pawn = fp_find_ai(name)
    pawn.Rotation = unrealsdk.make_struct("Rotator", Pitch=0, Yaw=int(yaw_deg * 65536.0 / 360.0) % 65536, Roll=0)
    return round(_yaw_deg(pawn), 2)


def aw_place_den(pawn, world_bearing, dist, dz):
    """Put the player `dist` horizontally from the pawn's den, on the world bearing (degrees), `dz` above the pawn's height, hung."""
    pc = get_pc()
    den = am_comp(pawn).GetParentComponent().Owner.Location
    r = math.radians(world_bearing)
    pc.Pawn.Location = unrealsdk.make_struct("Vector", X=den.X + math.cos(r) * dist, Y=den.Y + math.sin(r) * dist, Z=pawn.Location.Z + dz)
    pc.Pawn.Velocity = unrealsdk.make_struct("Vector", X=0.0, Y=0.0, Z=0.0)
    pc.Pawn.SetPhysics(0)
    pc.Pawn.ForceUpdateComponents(False, False)


def aw_walk(name, bearing, legs, dz=40.0, key="walk", stop_level=2, linger=1.5, frames=True, yaw=None, lock=True, center="pawn"):
    """Move the player along a line towards the pawn (see the module docstring). legs: [(from_dist, to_dist, speed_uu_per_s)], horizontal distances
    from the pawn. The first call places her at legs[0][0]. Rows: AW['runs'][key]['events'] (changes) and ['frames'] (compact, every game frame).
    The run ends `linger` seconds after the first frame in which the threat level (the record's) reached stop_level, or at the end of the last leg."""
    pawn0 = fp_find_ai(name)
    yaw0 = _yaw_deg(pawn0) if yaw is None else yaw
    st = {"key": key, "name": name, "bearing": bearing, "legs": legs, "dz": dz, "leg": 0, "d": float(legs[0][0]), "last_g": None, "events": [], "frames": [],
          "done": False, "t_hit": None, "prev": None, "start_g": _g(), "yaw": yaw0, "lock": lock, "center": center}
    AW["runs"][key] = st

    def tick(*_a):
        if st["done"]:
            return
        now = _g()
        pawn = fp_find_ai(name)
        if pawn is None or pawn.bDeleteMe:
            st["done"], st["error"] = True, "pawn gone"
            return
        if st["last_g"] is not None and st["t_hit"] is None:
            dt = now - st["last_g"]
            frm, to, speed = legs[st["leg"]]
            st["d"] -= speed * dt
            if st["d"] <= to:
                if st["leg"] + 1 < len(legs):
                    st["leg"] += 1
                    st["d"] = min(st["d"], float(legs[st["leg"]][0]))
                else:
                    st["d"] = float(to)
                    st["end_reached"] = now
        st["last_g"] = now
        if lock and center == "pawn":
            _set_yaw(pawn, yaw0)
        if st["t_hit"] is None:
            if center == "den":
                aw_place_den(pawn, bearing, st["d"], dz)
            else:
                aw_place(pawn, bearing, st["d"], dz, yaw0)
        row = aw_row(pawn)
        row["cmd_d"] = round(st["d"], 2)
        rt = row["root"] if isinstance(row["root"], dict) else {}
        sig = (row["rec"]["ThreatLevel"] if row["rec"] else None, row["cur"], row["n"] > 0, row["gt"], row["aware"], row["prov"], rt.get("ThreatLevel"),
               rt.get("AggroTime"), rt.get("ForgetTime"), row["rec"]["Exposure"] if row["rec"] else None)
        if sig != st["prev"]:
            row["event"] = True
            st["events"].append(row)
            st["prev"] = sig
        if frames and len(st["frames"]) < AW["cap"]:
            rec = row["rec"] or {}
            st["frames"].append([row["g"], row["cmd_d"], row["d2"], row["d3"], row["ang"], row.get("d_origin2"), rec.get("ThreatLevel"), rec.get("Exposure"),
                                 rec.get("TargetExposureToMe"), rec.get("bCanSeeOrHearTarget"), row["cur"] is not None, row["gt"], row["aware"],
                                 rt.get("AggroTime"), rt.get("ForgetTime"), row.get("mem")])
        lvl = row["gt"] if isinstance(row["gt"], int) else 0
        if st["t_hit"] is None and lvl >= stop_level:
            st["t_hit"] = now
            st["hit_row"] = row
        if st["t_hit"] is not None and now - st["t_hit"] >= linger:
            st["done"] = True
        if st.get("end_reached") is not None and now - st["end_reached"] >= linger + 1.0:
            st["done"] = True
    add_hook("WillowGame.WillowGameViewportClient:Tick", Type.POST_UNCONDITIONAL, "ow_aw_" + key, tick)
    AW["hooks"].append(("WillowGame.WillowGameViewportClient:Tick", Type.POST_UNCONDITIONAL, "ow_aw_" + key))
    return {"started": st["start_g"]}


def aw_dwell(name, configs, yaw, key="dwell", far=9500.0, far_time=2.6, dwell=2.2, up_far=400.0):
    """For each (bearing, dist, dz) config: put the player far away until the record has dropped (`far_time` s), then at the config (the pawn's facing
    locked to `yaw`) for `dwell` s, and log the compact rows. Result per config in AW['runs'][key]['rows']: the first row of each threat level, the last row, the
    time from arrival to the first level-2 answer. Memory-only stand-ins: the player's place and the held pawn's yaw."""
    st = {"key": key, "name": name, "yaw": yaw, "configs": configs, "i": 0, "phase": "far", "t0": None, "rows": [], "done": False, "cur": None}
    AW["runs"][key] = st

    def tick(*_a):
        if st["done"]:
            return
        pawn = fp_find_ai(name)
        if pawn is None or pawn.bDeleteMe:
            st["done"], st["error"] = True, "pawn gone"
            return
        now = _g()
        _set_yaw(pawn, yaw)
        if st["i"] >= len(configs):
            st["done"] = True
            aw_place(pawn, 0.0, far, up_far, yaw)
            return
        bearing, dist, dz = configs[st["i"]]
        if st["phase"] == "far":
            aw_place(pawn, 0.0, far, up_far, yaw)
            if st["t0"] is None:
                st["t0"] = now
            if now - st["t0"] >= far_time:
                st["phase"], st["t0"] = "near", None
                st["cur"] = {"config": [bearing, dist, dz], "first": {}, "t_arrive": None, "t_2": None, "events": []}
            return
        aw_place(pawn, bearing, dist, dz, yaw)
        if st["t0"] is None:
            st["t0"] = now
            st["cur"]["t_arrive"] = now
        row = aw_row(pawn)
        cur = st["cur"]
        rt = row["root"] if isinstance(row["root"], dict) else {}
        sig = (row["rec"]["ThreatLevel"] if row["rec"] else None, row["cur"] is not None, row["gt"], row["aware"], row["prov"], row["rec"]["Exposure"] if row["rec"] else None)
        if sig != cur.get("sig"):
            cur["sig"] = sig
            cur["events"].append(dict(row, since=round(now - st["t0"], 3)))
        lvl = row["gt"] if isinstance(row["gt"], int) else 0
        if lvl >= 2 and cur["t_2"] is None:
            cur["t_2"] = round(now - st["t0"], 3)
        if now - st["t0"] >= dwell:
            cur["last"] = row
            cur["max_level"] = max([e["gt"] for e in cur["events"] if isinstance(e["gt"], int)] or [0])
            cur.pop("sig", None)
            st["rows"].append(cur)
            st["i"] += 1
            st["phase"], st["t0"] = "far", None
    add_hook("WillowGame.WillowGameViewportClient:Tick", Type.POST_UNCONDITIONAL, "ow_aw_" + key, tick)
    AW["hooks"].append(("WillowGame.WillowGameViewportClient:Tick", Type.POST_UNCONDITIONAL, "ow_aw_" + key))
    return {"n": len(configs)}


def aw_dwell_den(name, configs, dwell=2.4, rest_pos=(0.0, 6200.0, 300.0), up_min=1.5, key="dd"):
    """For each (world_bearing, dist_from_den, dz) config: rest the player at rest_pos (world bearing, distance from the den, dz) until the den's threat
    level for her is below 2 for up_min seconds (a natural reset: the level falls after ForgetTime), then put her at the config for `dwell` s. Per config:
    the change events with each member's record, the time of the first level-2 answer and the members at the last sample before it."""
    st = {"key": key, "name": name, "configs": configs, "i": 0, "phase": "rest", "t0": None, "rows": [], "done": False, "cur": None, "below": None, "prev_mem": None}
    AW["runs"][key] = st

    def tick(*_a):
        if st["done"]:
            return
        pawn = fp_find_ai(name)
        if pawn is None or pawn.bDeleteMe:
            st["done"], st["error"] = True, "pawn gone"
            return
        now = _g()
        if st["i"] >= len(configs):
            st["done"] = True
            aw_place_den(pawn, rest_pos[0], rest_pos[1], rest_pos[2])
            return
        bearing, dist, dz = configs[st["i"]]
        if st["phase"] == "rest":
            aw_place_den(pawn, rest_pos[0], rest_pos[1], rest_pos[2])
            gt = pawn.MyWillowMind.AIComponent.GetThreatLevel(get_pc().Pawn)
            if gt < 2:
                st["below"] = st["below"] if st["below"] is not None else now
            else:
                st["below"] = None
            if st["below"] is not None and now - st["below"] >= up_min:
                st["phase"], st["t0"] = "near", None
                st["cur"] = {"config": [bearing, dist, dz], "events": [], "t_2": None}
            return
        aw_place_den(pawn, bearing, dist, dz)
        if st["t0"] is None:
            st["t0"] = now
        row = aw_row(pawn)
        cur = st["cur"]
        row["since"] = round(now - st["t0"], 3)
        mem = row.get("mem") if isinstance(row.get("mem"), list) else []
        sig = (row["gt"], row["aware"], tuple((m[5], m[6], m[8]) for m in mem))
        if sig != cur.get("sig"):
            cur["events"].append({"prev_mem": st["prev_mem"], "row": row})
            cur["sig"] = sig
        st["prev_mem"] = mem
        if row["gt"] >= 2 and cur["t_2"] is None:
            cur["t_2"] = row["since"]
        if now - st["t0"] >= dwell:
            cur.pop("sig", None)
            st["rows"].append(cur)
            st["i"] += 1
            st["phase"], st["t0"], st["below"] = "rest", None, None
    add_hook("WillowGame.WillowGameViewportClient:Tick", Type.POST_UNCONDITIONAL, "ow_aw_" + key, tick)
    AW["hooks"].append(("WillowGame.WillowGameViewportClient:Tick", Type.POST_UNCONDITIONAL, "ow_aw_" + key))
    return {"n": len(configs)}


def aw_timeline(name, steps, total, key="tl", mem=True):
    """Scripted placements on a clock. steps: [(t_offset_s, "den", world_bearing, dist, dz) or (t_offset_s, "pawn", bearing, dist, dz, yaw)]; each step
    is applied once when the clock passes its offset, and the player stays there until the next one. Rows: AW['runs'][key]['events'] (changes of the
    threat state, with the step index) and ['frames'] (every frame, compact). Ends `total` seconds after the start."""
    st = {"key": key, "name": name, "steps": steps, "total": total, "t0": None, "k": -1, "events": [], "frames": [], "done": False, "prev": None}
    AW["runs"][key] = st

    def tick(*_a):
        if st["done"]:
            return
        pawn = fp_find_ai(name)
        if pawn is None or pawn.bDeleteMe:
            st["done"], st["error"] = True, "pawn gone"
            return
        now = _g()
        if st["t0"] is None:
            st["t0"] = now
        t = now - st["t0"]
        while st["k"] + 1 < len(steps) and t >= steps[st["k"] + 1][0]:
            st["k"] += 1
            sp = steps[st["k"]]
            if sp[1] == "den":
                aw_place_den(pawn, sp[2], sp[3], sp[4])
            else:
                aw_place(pawn, sp[2], sp[3], sp[4], sp[5])
        row = aw_row(pawn)
        row["t"] = round(t, 3)
        row["step"] = st["k"]
        rt = row["root"] if isinstance(row["root"], dict) else {}
        sig = (row["step"], row["rec"]["ThreatLevel"] if row["rec"] else None, row["cur"] is not None, row["gt"], row["aware"], rt.get("ThreatLevel"),
               rt.get("AggroTime"), tuple((m[5], m[8]) for m in row["mem"]) if isinstance(row["mem"], list) else None)
        if sig != st["prev"]:
            st["prev"] = sig
            row.pop("mem", None) if not mem else None
            st["events"].append(row)
        st["frames"].append([row["g"], row["t"], row["step"], row["d2"], row["d3"], row.get("d_origin2"), row["gt"], row["aware"], rt.get("ThreatLevel"),
                             rt.get("AggroTime"), rt.get("ForgetTime"), row["cur"] is not None, row["n"]])
        if t >= total:
            st["done"] = True
    add_hook("WillowGame.WillowGameViewportClient:Tick", Type.POST_UNCONDITIONAL, "ow_aw_" + key, tick)
    AW["hooks"].append(("WillowGame.WillowGameViewportClient:Tick", Type.POST_UNCONDITIONAL, "ow_aw_" + key))
    return {"steps": len(steps)}


def aw_walk_state(key):
    st = AW["runs"].get(key)
    if not st:
        return None
    if "rows" in st:
        return {"done": st["done"], "error": st.get("error"), "i": st["i"], "of": len(st["configs"]), "phase": st["phase"]}
    if "steps" in st:
        return {"done": st["done"], "error": st.get("error"), "k": st["k"], "frames": len(st["frames"])}
    return {"done": st["done"], "error": st.get("error"), "n_frames": len(st["frames"]), "n_events": len(st["events"]), "t_hit": st["t_hit"],
            "d": round(st["d"], 1), "leg": st["leg"], "last_event": st["events"][-1] if st["events"] else None}


def aw_run_stop(key):
    st = AW["runs"].get(key)
    if st:
        st["done"] = True
    try:
        remove_hook("WillowGame.WillowGameViewportClient:Tick", Type.POST_UNCONDITIONAL, "ow_aw_" + key)
    except Exception:
        pass


def aw_unhook():
    for path, typ, ident in AW["hooks"]:
        try:
            remove_hook(path, typ, ident)
        except Exception:
            pass
    AW["hooks"] = []


def aw_dump(name, data=None):
    path = RG_OUT + "/" + name
    with open(path, "w", encoding="utf-8") as handle:
        json.dump(AW["runs"] if data is None else data, handle, default=str)
    return {"path": path}


def aw_valid_probe(name, other=None):
    """ValidTarget answers of the pawn's component. The cloak flag and the allegiance are changed in memory for the call and put back."""
    pawn = fp_find_ai(name)
    comp = am_comp(pawn)
    me = get_pc().Pawn
    out = {"pawn": name}

    def ask(label, actor):
        try:
            out[label] = bool(comp.ValidTarget(actor))
        except Exception as error:
            out[label] = "err " + repr(error)[:80]
    ask("player", me)
    ask("none", None)
    ask("self", pawn)
    if other is not None:
        o = fp_find_ai(other)
        if o is not None:
            ask("other_" + other, o)
            out["other_allegiance"] = fp_deep(o.Allegiance)
    out["bCanTargetFriendlies"] = fp_deep(comp.TargetingDef.bCanTargetFriendlies)
    old = me.bCloaked
    me.bCloaked = True
    ask("player_cloaked", me)
    me.bCloaked = old
    ask("player_uncloaked_again", me)
    return out


def aw_allegiance_probe(name):
    """Clear the pawn's allegiance (memory only), ask ValidTarget and FindTargets-style questions, put it back."""
    pawn = fp_find_ai(name)
    comp = am_comp(pawn)
    me = get_pc().Pawn
    out = {"allegiance_before": fp_deep(pawn.Allegiance)}
    saved = pawn.Allegiance
    try:
        pawn.Allegiance = None
        out["allegiance_cleared"] = fp_deep(pawn.Allegiance)
        try:
            out["valid_player"] = bool(comp.ValidTarget(me))
        except Exception as error:
            out["valid_player"] = "err " + repr(error)[:80]
    finally:
        pawn.Allegiance = saved
    out["allegiance_after"] = fp_deep(pawn.Allegiance)
    try:
        out["valid_player_restored"] = bool(comp.ValidTarget(me))
    except Exception as error:
        out["valid_player_restored"] = "err " + repr(error)[:80]
    return out


def aw_trace_source(name):
    """GetMeleeTraceSourceLocationAndRotation of an AI pawn next to its Location and Rotation (the out parameters come back as a tuple when called with
    two empty structs)."""
    pawn = fp_find_ai(name)
    r = pawn.GetMeleeTraceSourceLocationAndRotation(unrealsdk.make_struct("Vector"), unrealsdk.make_struct("Rotator"))
    loc, rot = r[1], r[2]
    return {"src_loc": _vec(loc), "actor_loc": _vec(pawn.Location), "src_rot": [rot.Pitch, rot.Yaw, rot.Roll],
            "actor_rot": [pawn.Rotation.Pitch, pawn.Rotation.Yaw, pawn.Rotation.Roll]}


def aw_melee_over_time(name, mdef_path, dist, bearing=0.0, dz=0.0, seconds=7.0, key="mot", move=None):
    """Hold the pawn, put the player `dist` from it (bearing from its facing), call MeleeDefinition.MeleeAttack(pawn) from a Tick hook (script stand-in
    for the attack) and log every frame the pawn's MeleeState and the hits on the player (AM rows via am_hooks on WillowPlayerPawn.TakeDamage, filtered
    by the definition). `move` (optional): [(t_offset, bearing, dist)] player placements during the run. Results in AW['runs'][key]."""
    st = {"key": key, "name": name, "t0": None, "frames": [], "done": False, "mark": len(AM["rows"]), "moves": move or [], "mi": 0}
    AW["runs"][key] = st
    mdef = unrealsdk.find_object("MeleeDefinition", mdef_path)
    aw_hold(name, True)

    def tick(*_a):
        if st["done"]:
            return
        pawn = fp_find_ai(name)
        if pawn is None or pawn.bDeleteMe:
            st["done"], st["error"] = True, "pawn gone"
            return
        now = _g()
        if st["t0"] is None:
            st["t0"] = now
            aw_place(pawn, bearing, dist, dz)
            st["phase"] = 1
            return
        t = now - st["t0"]
        if st["phase"] == 1:
            st["phase"] = 2
            return
        if st["phase"] == 2:
            st["call_t"] = round(t, 4)
            st["call_g"] = now
            st["geo"] = aw_row(pawn)
            mdef.MeleeAttack(pawn)
            st["phase"] = 3
        while st["mi"] < len(st["moves"]) and t >= st["moves"][st["mi"]][0]:
            _t, b, d = st["moves"][st["mi"]]
            aw_place(pawn, b, d, dz)
            st["mi"] += 1
        ms = pawn.MeleeState
        st["frames"].append([now, round(t, 4), bool(ms.bIsMeleeActive), fp_deep(ms.MeleeDefinition), round(ms.MeleeStartTime, 4), round(ms.TimeElapsedSinceLastMelee, 4),
                             len(list(ms.HitActors)), _vec(ms.MeleeStartLocation), [ms.MeleeStartRotation.Pitch, ms.MeleeStartRotation.Yaw, ms.MeleeStartRotation.Roll],
                             _vec(pawn.Location), _vec(get_pc().Pawn.Location)])
        if t >= seconds:
            st["done"] = True
            st["hits"] = [{"g": r["g"], "args": {k: r["args"].get(k) for k in ("Damage", "Momentum", "HitLocation")}, "causer": r["args"].get("DamageCauser"),
                           "by": r["args"].get("InstigatedBy")} for r in AM["rows"][st["mark"]:] if r.get("func", "").endswith("TakeDamage") and r["tag"] == "pre"]
    add_hook("WillowGame.WillowGameViewportClient:Tick", Type.POST_UNCONDITIONAL, "ow_aw_" + key, tick)
    AW["hooks"].append(("WillowGame.WillowGameViewportClient:Tick", Type.POST_UNCONDITIONAL, "ow_aw_" + key))
    return {"mdef": mdef._path_name()}


_NODE_KEYS = ("ActiveChild", "NextActiveChild", "LastFailTime", "NextTimedRun", "NextTimedFail", "LoopCount", "LastRandomChild", "NextSequentialChild", "ActionFailName", "FailReason",
              "bHoldingResource")


def aw_tree_layout(name):
    """The pawn's own copy of the tree as plain data (type, children, timed ranges, weights, loop maxima), no run state."""
    pawn = fp_find_ai(name)
    out = []
    for i, node in enumerate(list(am_comp(pawn).AIDef.NodeList)):
        act = getattr(node, "Action", None)
        out.append({"i": i, "name": node.NodeName, "type": fp_deep(node.Type), "children": list(node.Children), "weights": list(node.Weights), "loop_max": list(node.LoopCountMax),
                    "timed_start": fp_deep(node.TimedStart), "timed_reset": fp_deep(node.TimedReset), "timed_fail": fp_deep(node.TimedFail),
                    "stop_if_cannot_run": bool(node.bStopIfCannotRun), "interrupt": bool(node.bInterrupt), "action": act.Class.Name if act is not None else None})
    return out


def aw_tree(name, seconds, key="tree", keep_engaged=True, dist=450.0, every=1.0):
    """Log, for the pawn's tree, every change of a node's run-state fields (rows [g, node, field values]) and of the running leaf (the ActiveChild chain
    from the root), with the player's distance. With keep_engaged the player is put `dist` in front of the pawn every `every` s (hung), so a fight goes on
    (she is made immune by the caller). Results in AW['runs'][key]."""
    st = {"key": key, "name": name, "t0": None, "rows": [], "leaf": [], "prev": {}, "prev_leaf": None, "done": False, "next_place": 0.0, "frames": 0}
    AW["runs"][key] = st

    def leaf_of(nodes):
        """The running path: node indices from the root; a node's ActiveChild is an index into its Children array."""
        i, chain = 0, [0]
        while True:
            n = nodes[i]
            ch = list(n.Children)
            a = n.ActiveChild
            if not ch or a is None or a < 0 or a >= len(ch):
                return chain
            i = ch[a]
            chain.append(i)
            if len(chain) > 12:
                return chain

    def tick(*_a):
        if st["done"]:
            return
        pawn = fp_find_ai(name)
        if pawn is None or pawn.bDeleteMe:
            st["done"], st["error"] = True, "pawn gone"
            return
        now = _g()
        if st["t0"] is None:
            st["t0"] = now
        if keep_engaged and now >= st["next_place"]:
            aw_place(pawn, 0.0, dist, 0.0)
            st["next_place"] = now + every
        nodes = list(am_comp(pawn).AIDef.NodeList)
        st["frames"] += 1
        for i, n in enumerate(nodes):
            vals = tuple(fp_deep(getattr(n, k)) for k in _NODE_KEYS)
            if st["prev"].get(i) != vals:
                st["prev"][i] = vals
                st["rows"].append([now, i] + list(vals))
        chain = leaf_of(nodes)
        if chain != st["prev_leaf"]:
            st["prev_leaf"] = chain
            me, pl = get_pc().Pawn.Location, pawn.Location
            st["leaf"].append([now, chain, round(math.dist((me.X, me.Y, me.Z), (pl.X, pl.Y, pl.Z)), 1), round(pawn.GetHealth(), 1)])
        if now - st["t0"] >= seconds:
            st["done"] = True
    add_hook("WillowGame.WillowGameViewportClient:Tick", Type.POST_UNCONDITIONAL, "ow_aw_" + key, tick)
    AW["hooks"].append(("WillowGame.WillowGameViewportClient:Tick", Type.POST_UNCONDITIONAL, "ow_aw_" + key))
    return {"layout": aw_tree_layout(name)}
