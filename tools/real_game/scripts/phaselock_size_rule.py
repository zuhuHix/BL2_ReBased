"""Phaselock bubble size rule: probe for the openwillow_realgame command channel.

Run through Invoke-GamePyFile AFTER scripts/phaselock.py and scripts/phaselock_matched.py (it uses lift_skill(),
ai_pawn() and plain() from their namespace). Our own code; what it writes is game data and goes under RG_OUT
(normally the ignored local/realgame/phaselock/size_rule).

- stand_still(name): every map AI pawn keeps its controller (a pawn without one is not locked) but cannot
  walk or fly (GroundSpeed and AirSpeed 0); returns the target's location. Call it again right before a cast.
- sr_start(label) / sr_stop(): a viewport-tick hook that writes JSON lines to `<label>_probe.jsonl`:
  every change of the skill's state, and at +0.72, +1.5, +3.0 and +4.5 s after SkillStartTime one dump of
  the lifted pawn (mesh bounds, scales, collision), the skill's timing and bubble fields, and the bubble
  emitters (class, location, DrawScale, DrawScale3D and the ParticleSystemComponent's template, Scale,
  Scale3D, translation). `LockDurationFormula` and the other struct fields are dumped as plain data.
"""
import json
import time
from pathlib import Path

import unrealsdk
from unrealsdk.hooks import Type, add_hook, remove_hook

SR_TICK = "WillowGame.WillowGameViewportClient:Tick"
SR_HOOK = "ow_sr_probe"
sr = {"file": None, "state": None, "done": set(), "skill_start": None}
DUMP_AT = (0.72, 1.5, 3.0, 4.5)
SKILL_FIELDS = ("StateStartTime", "StateDuration", "SkillStartTime", "SkillDuration", "LiftDuration", "LiftStartTime",
                "CollapseStartTime", "CollapseDuration", "MaxCollapseValue", "LockFadeOutTime", "ReleaseBufferTime",
                "BubbleFXScale", "BubbleFXIntroTime", "BubbleFXOutroOverlapTime", "LiftSnapTimePct", "LiftSnapHeightPct",
                "LiftBobAmplitude", "LiftBobFrequency", "LockDurationFormula", "LockDurationScaleFormula",
                "BubbleFXTranslation", "BubbleFXAttachmentName")
EMITTERS = ("BubbleFXEmitter_FadeIn", "BubbleFXEmitter_Loop", "BubbleFXEmitter_FadeOut")


def _get(obj, name):
    try:
        return plain(getattr(obj, name))
    except Exception as error:
        return f"<{type(error).__name__}: {str(error)[:80]}>"


def _emitter(e):
    if e is None:
        return None
    row = {"path": e._path_name(), "class": e.Class.Name}
    for name in ("Location", "DrawScale", "DrawScale3D", "Rotation", "bHidden", "LifeSpan"):
        row[name] = _get(e, name)
    psc = _get(e, "ParticleSystemComponent")
    comp = getattr(e, "ParticleSystemComponent", None)
    if comp is not None:
        row["psc"] = {n: _get(comp, n) for n in ("Scale", "Scale3D", "Translation", "bIsActive")}
        tpl = getattr(comp, "Template", None)
        row["psc"]["template"] = tpl._path_name() if tpl is not None else None
        try:
            row["psc"]["bounds_sphere"] = comp.Bounds.SphereRadius
        except Exception as error:
            row["psc"]["bounds_sphere"] = f"<{type(error).__name__}>"
    else:
        row["psc"] = psc
    return row


def _pawn(p):
    if p is None:
        return None
    m, c = p.Mesh, p.CylinderComponent
    b = m.Bounds
    return {"name": p.Name, "location": plain(p.Location), "draw_scale": p.DrawScale, "draw_scale_3d": plain(p.DrawScale3D),
            "mesh_scale": m.Scale, "mesh_scale_3d": plain(m.Scale3D), "mesh": m.SkeletalMesh._path_name() if m.SkeletalMesh else None,
            "bounds": {"origin": plain(b.Origin), "box_extent": plain(b.BoxExtent), "sphere_radius": b.SphereRadius},
            "cylinder": {"radius": c.CollisionRadius, "height": c.CollisionHeight}}


def _dump(skill, tag, since):
    row = {"t": time.perf_counter(), "event": "dump", "tag": tag, "since_cast": round(since, 4),
           "game_t": get_pc().WorldInfo.TimeSeconds, "skill_path": skill._path_name(),
           "skill": {n: _get(skill, n) for n in SKILL_FIELDS}, "pawn": _pawn(skill.LiftedPawn),
           "emitters": {n: _emitter(getattr(skill, n, None)) for n in EMITTERS}}
    sr["file"].write(json.dumps(row) + "\n")
    sr["file"].flush()


def _on_tick(*_args):
    try:
        skill = lift_skill()
        if skill is None or sr["file"] is None:
            return
        state = str(skill.CurrentState)
        now_game = get_pc().WorldInfo.TimeSeconds
        if state != sr["state"]:
            sr["file"].write(json.dumps({"t": time.perf_counter(), "event": "state", "state": state, "was": sr["state"],
                                         "game_t": now_game, "state_start": skill.StateStartTime,
                                         "state_duration": skill.StateDuration, "skill_start": skill.SkillStartTime,
                                         "skill_duration": skill.SkillDuration}) + "\n")
            sr["state"] = state
        if state == "0" or skill.SkillStartTime <= 0 or skill.LiftedPawn is None:
            return
        if sr["skill_start"] != skill.SkillStartTime:
            sr["skill_start"], sr["done"] = skill.SkillStartTime, set()
        since = now_game - skill.SkillStartTime
        for at in DUMP_AT:
            if since >= at and at not in sr["done"]:
                sr["done"].add(at)
                _dump(skill, f"+{at}", since)
    except Exception as error:  # never break the game
        try:
            sr["file"].write(json.dumps({"event": "error", "error": repr(error)[:300]}) + "\n")
        except Exception:
            pass


def sr_start(label):
    sr_stop()
    folder = Path(RG_OUT)
    folder.mkdir(parents=True, exist_ok=True)
    sr["file"] = (folder / f"{label}_probe.jsonl").open("w", encoding="utf-8")
    sr["state"], sr["done"], sr["skill_start"] = None, set(), None
    add_hook(SR_TICK, Type.POST_UNCONDITIONAL, SR_HOOK, _on_tick)


def sr_stop():
    remove_hook(SR_TICK, Type.POST_UNCONDITIONAL, SR_HOOK)
    if sr["file"] is not None:
        sr["file"].close()
        sr["file"] = None


def stand_still(name):
    target = ai_pawn(name)
    n = 0
    for pawn in list(unrealsdk.find_all("WillowAIPawn")):
        if not pawn.Name.startswith("WillowAIPawn_"):
            continue  # Pawn_<Type> objects are preloaded templates that spawners copy from
        pawn.GroundSpeed = 0.0
        if hasattr(pawn, "AirSpeed"):
            pawn.AirSpeed = 0.0
        n += 1
    return {"target": target.Name, "pawns": n, "target_loc": plain(target.Location)}
