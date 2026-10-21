"""Phaselock lifted-target animation: per-frame probe for the openwillow_realgame command channel.

Run through Invoke-GamePyFile AFTER scripts/phaselock.py and scripts/phaselock_matched.py (it uses lift_skill(),
ai_pawn() and plain() from their namespace). Our own code; what it writes is game data and goes under RG_OUT
(normally the ignored local/realgame/phaselock/target_anim).

Question it answers (docs/verification/NATIVE_PHASELOCK_PRESENTATION.md section 5, all read from script, UNVERIFIED in
game): does the lifted pawn play PhaseLock_Lift from the cast, PhaseLock_Loop from the lift clip's end, PhaseLock_Fall
time-stretched to DropTime on release and PhaseLock_Land on touch-down; when does its mesh Translation change
(UpdateLiftedPawnMeshOffset); how do its mesh bounds follow the pose.

- ta_start(label, pawn_name) / ta_stop(): a viewport-tick hook that writes one JSON line per rendered frame while the
  skill is active (and for 3 s after it ends) to `<label>_anim.jsonl`: game time, time since SkillStartTime, the
  skill state, the pawn's location and Physics, its mesh Translation and bounds, and every AnimNodeSequence in the
  pawn's AnimTree with a total weight above 0.001 (sequence name, rate, current time, playing/looping flags, weight).
- ta_scale(pawn_name): the mesh component's Scale, Scale3D, the pawn's DrawScale(3D) and the world positions of a few
  bones relative to the pawn, so the rendered size can be checked against the archetype's component Scale (0.7) and
  the Mesh.Scale 1.0 that an earlier read reported.
"""
import json
import time
from pathlib import Path

import unrealsdk
from unrealsdk.hooks import Type, add_hook, remove_hook

TA_TICK = "WillowGame.WillowGameViewportClient:Tick"
TA_HOOK = "ow_ta_probe"
ta = {"file": None, "pawn": None, "ended_at": None}


def _safe(fn, default=None):
    try:
        return fn()
    except Exception as error:
        return default if default is not None else f"<{type(error).__name__}>"


def _nodes(mesh):
    rows = []
    for node in list(getattr(mesh, "AnimTickArray", []) or []):
        if node is None:
            continue
        # Only sequence players (AnimNodeSequence and its subclasses) have AnimSeqName.
        name = _safe(lambda: str(node.AnimSeqName), None) if hasattr(node, "AnimSeqName") else None
        if name is None:
            continue
        weight = _safe(lambda: float(node.NodeTotalWeight), 0.0)
        if not isinstance(weight, float) or weight <= 0.001:
            continue
        rows.append({"node": node.Name, "class": node.Class.Name, "seq": name, "weight": round(weight, 4),
                     "rate": _safe(lambda: round(float(node.Rate), 4)), "time": _safe(lambda: round(float(node.CurrentTime), 4)),
                     "playing": _safe(lambda: bool(node.bPlaying)), "looping": _safe(lambda: bool(node.bLooping))})
    return rows


def _on_tick(*_args):
    try:
        if ta["file"] is None:
            return
        skill = lift_skill()
        pawn = ta["pawn"]
        if skill is None or pawn is None:
            return
        world = get_pc().WorldInfo
        state = str(skill.CurrentState)
        if state == "0" and skill.SkillStartTime > 0:
            ta["ended_at"] = ta["ended_at"] or world.TimeSeconds
            if world.TimeSeconds - ta["ended_at"] > 3.0:
                return
        elif state != "0":
            ta["ended_at"] = None
        if skill.SkillStartTime <= 0:
            return
        mesh = pawn.Mesh
        b = mesh.Bounds
        row = {"t": time.perf_counter(), "game_t": world.TimeSeconds, "since_cast": round(world.TimeSeconds - skill.SkillStartTime, 4),
               "state": state, "location": plain(pawn.Location), "physics": _safe(lambda: str(pawn.Physics)),
               "mesh_translation": plain(mesh.Translation), "sphere_radius": round(b.SphereRadius, 2),
               "bounds_origin": plain(b.Origin), "nodes": _nodes(mesh)}
        ta["file"].write(json.dumps(row) + "\n")
    except Exception as error:  # never break the game
        try:
            ta["file"].write(json.dumps({"event": "error", "error": repr(error)[:300]}) + "\n")
        except Exception:
            pass


def ta_start(label, pawn_name):
    ta_stop()
    folder = Path(RG_OUT)
    (folder / label).parent.mkdir(parents=True, exist_ok=True)
    ta["file"] = (folder / f"{label}_anim.jsonl").open("w", encoding="utf-8")
    ta["pawn"], ta["ended_at"] = ai_pawn(pawn_name), None
    add_hook(TA_TICK, Type.POST_UNCONDITIONAL, TA_HOOK, _on_tick)
    return {"pawn": ta["pawn"].Name, "mesh": ta["pawn"].Mesh.SkeletalMesh._path_name(),
            "anim_tick_nodes": len(list(getattr(ta["pawn"].Mesh, "AnimTickArray", []) or []))}


def ta_scale(pawn_name, bones=("Root", "Hips", "Head", "LClav", "RClav", "lHand1", "rHand1", "LFoot", "RFoot")):
    pawn = ai_pawn(pawn_name)
    mesh = pawn.Mesh
    out = {"pawn": pawn.Name, "location": plain(pawn.Location), "draw_scale": pawn.DrawScale, "draw_scale_3d": plain(pawn.DrawScale3D),
           "mesh_scale": mesh.Scale, "mesh_scale_3d": plain(mesh.Scale3D), "mesh_translation": plain(mesh.Translation),
           "bounds_sphere": mesh.Bounds.SphereRadius, "bones": {}}
    for bone in bones:
        try:
            v = mesh.GetBoneLocation(bone, 0)
            out["bones"][bone] = [round(v.X - pawn.Location.X, 2), round(v.Y - pawn.Location.Y, 2), round(v.Z - pawn.Location.Z, 2)]
        except Exception as error:
            out["bones"][bone] = f"<{type(error).__name__}: {str(error)[:60]}>"
    return out


def ta_stop():
    remove_hook(TA_TICK, Type.POST_UNCONDITIONAL, TA_HOOK)
    if ta["file"] is not None:
        ta["file"].close()
        ta["file"] = None
