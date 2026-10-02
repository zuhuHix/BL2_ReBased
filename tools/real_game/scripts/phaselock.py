"""Phaselock ground truth: per-frame sampler for the openwillow_realgame command channel.

Run once through Invoke-GamePyFile (tools/real_game/realgame.ps1). Our own code; what it writes is
game data and goes under RG_OUT (normally the ignored local/realgame/phaselock).

- pl_start(name) / pl_stop(): every rendered frame, append one JSON line with time.perf_counter()
  (the same clock as the burst capture's QPC stamps), WorldInfo.TimeSeconds, the lift skill's state
  and timing fields, the lifted pawn's location and Maya's weapon state. StartActionSkill calls are
  marked with their outcome (the skill state after the call).
- protect_player(on): refuse damage to the player pawn while capturing (so a test is not ended by
  Maya going down); it changes nothing else.
- face(pawn_path, distance): put Maya `distance` units from an AI pawn, looking at it.
"""
import json
import math
import time
from pathlib import Path

import unrealsdk
from unrealsdk.hooks import Block, Type, add_hook, remove_hook

TICK = "WillowGame.WillowGameViewportClient:Tick"
SAMPLE_HOOK = "ow_pl_sample"
pl_state = {"file": None, "skill": None}


def _vec(v):
    return [round(v.X, 3), round(v.Y, 3), round(v.Z, 3)] if v is not None else None


def lift_skill():
    skills = [s for s in unrealsdk.find_all("LiftActionSkill") if "Default__" not in s._path_name()]
    return skills[-1] if skills else None


# GetStateName() reads None through the SDK on weapons, so ask IsInState for the states that matter.
WEAPON_STATES = ("WeaponReloading", "WeaponPuttingDown", "WeaponEquipping", "WeaponFiring", "Active", "Inactive")


def weapon_state(weapon):
    if weapon is None:
        return {"weapon_state": None}
    return {"weapon_state": next((s for s in WEAPON_STATES if weapon.IsInState(s)), "other"),
            "reload_state": int(weapon.ReplicatedReloadState), "putting_down": bool(weapon.IsPuttingDown()),
            "equipping": bool(weapon.IsEquipping())}


def _sample(*_args):
    try:
        pc = get_pc()
        pawn = pc.Pawn if pc else None
        skill = lift_skill()
        row = {"t": time.perf_counter(), "game_t": pc.WorldInfo.TimeSeconds if pc else None}
        if pawn is not None:
            weapon = pawn.Weapon
            row["weapon"] = weapon.GetShortHumanReadableName() if weapon else None
            row.update(weapon_state(weapon))
        if skill is not None:
            lifted = skill.LiftedPawn
            row.update({
                "state": str(skill.CurrentState), "state_start": skill.StateStartTime,
                "state_duration": skill.StateDuration, "skill_start": skill.SkillStartTime,
                "skill_duration": skill.SkillDuration, "lift_start": skill.LiftStartTime,
                "lift_duration": skill.LiftDuration, "collapse_start": skill.CollapseStartTime,
                "bob_amplitude": skill.LiftBobAmplitude, "bob_frequency": skill.LiftBobFrequency,
                "lift_start_loc": _vec(skill.LiftStartLocation), "lift_end_loc": _vec(skill.LiftEndLocation),
                "prev_bob": _vec(skill.PrevBobLocation),
                "lifted": lifted._path_name().split(".")[-1] if lifted else None,
                "lifted_loc": _vec(lifted.Location) if lifted else None,
            })
        pl_state["file"].write(json.dumps(row) + "\n")
    except Exception as error:  # never break the game
        mark("pl_sample_error", error=repr(error)[:300])


def _on_start_skill(obj, _args, _ret, func):
    skill = lift_skill()
    mark("start_action_skill", phase="post", state=str(skill.CurrentState) if skill else None)


def _on_start_skill_pre(obj, _args, _ret, func):
    pawn = get_pc().Pawn
    mark("start_action_skill", phase="pre", **weapon_state(pawn.Weapon if pawn else None))


# Weapon calls marked on the same clock, to show what the weapon was doing when the key went down.
WEAPON_MARKS = ("BeginReload", "StartReload", "ReloadDone", "StopReloading", "OnAbortReload", "TryPutDown",
                "PutDownWeapon", "WeaponPuttingDown")


def _on_weapon_call(obj, _args, _ret, func):
    mark("weapon_call", func=func.func.Name, weapon=obj.GetShortHumanReadableName())


def _weapon_mark_paths():
    cls, paths = unrealsdk.find_class("WillowWeapon"), []
    while cls:
        paths += [f._path_name() for f in cls._fields()
                  if f.Class.Name == "Function" and f.Name in WEAPON_MARKS and f.Outer == cls]
        cls = cls.SuperField
    return paths


def pl_start(name):
    pl_stop()
    folder = Path(RG_OUT)
    folder.mkdir(parents=True, exist_ok=True)
    pl_state["file"] = (folder / name).open("w", encoding="utf-8")
    add_hook(TICK, Type.POST_UNCONDITIONAL, SAMPLE_HOOK, _sample)
    add_hook("WillowGame.WillowPlayerController:StartActionSkill", Type.PRE, SAMPLE_HOOK, _on_start_skill_pre)
    add_hook("WillowGame.WillowPlayerController:StartActionSkill", Type.POST, SAMPLE_HOOK, _on_start_skill)
    for path in _weapon_mark_paths():
        add_hook(path, Type.PRE, SAMPLE_HOOK, _on_weapon_call)


def pl_stop():
    remove_hook(TICK, Type.POST_UNCONDITIONAL, SAMPLE_HOOK)
    remove_hook("WillowGame.WillowPlayerController:StartActionSkill", Type.PRE, SAMPLE_HOOK)
    remove_hook("WillowGame.WillowPlayerController:StartActionSkill", Type.POST, SAMPLE_HOOK)
    for path in _weapon_mark_paths():
        remove_hook(path, Type.PRE, SAMPLE_HOOK)
    if pl_state["file"] is not None:
        pl_state["file"].close()
        pl_state["file"] = None


def _refuse_damage(obj, _args, _ret, _func):
    if obj == get_pc().Pawn:
        return Block
    return None


def _take_damage_paths():
    """Every TakeDamage declared along the player pawn's class chain (a hook fires only for the
    function that actually runs, which is the most derived override)."""
    paths, cls = [], get_pc().Pawn.Class
    while cls:
        for field in cls._fields():
            if field.Name == "TakeDamage" and field.Outer == cls:
                paths.append(field._path_name())
        cls = cls.SuperField
    return paths


def protect_player(on=True):
    paths = _take_damage_paths()
    for path in paths:
        if on:
            add_hook(path, Type.PRE, "ow_protect", _refuse_damage)
        else:
            remove_hook(path, Type.PRE, "ow_protect")
    return paths


def ai_pawn(name):
    """An AI pawn by its object name (e.g. WillowAIPawn_45); paths differ per map and sublevel."""
    for pawn in unrealsdk.find_all("WillowAIPawn"):
        if pawn.Name == name:
            return pawn
    raise LookupError(name)


def face(name, distance=1200.0):
    target = ai_pawn(name)
    pc = get_pc()
    me = pc.Pawn
    dx, dy = me.Location.X - target.Location.X, me.Location.Y - target.Location.Y
    length = math.hypot(dx, dy) or 1.0
    x = target.Location.X + dx / length * distance
    y = target.Location.Y + dy / length * distance
    me.SetLocation(unrealsdk.make_struct("Vector", X=x, Y=y, Z=target.Location.Z + 60))
    yaw = math.atan2(target.Location.Y - y, target.Location.X - x)
    pc.Rotation = unrealsdk.make_struct("Rotator", Pitch=0, Yaw=int(yaw / math.pi * 32768) & 0xFFFF, Roll=0)
    return [round(x), round(y)]
