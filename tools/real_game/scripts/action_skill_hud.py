"""Action-skill HUD ground truth: per-frame sampler for the openwillow_realgame command channel.

Run through Invoke-GamePyFile AFTER scripts/phaselock.py (it uses lift_skill() and plain() from that namespace). Our own
code; what it writes is game data and goes under RG_OUT (normally the ignored local/realgame/action_skill_hud).

The HUD's action-skill functions are native, so the movie is read live: the clips below through the player's
WillowHUDGFxMovie (GFxMoviePlayer.GetVariableNumber / GetVariableBool / GetVariableString on "<path>.<property>").

- ash_probe(): find the HUD movie, the path prefix and getter calls that answer, and the action-skill cooldown pool;
  returns what worked (also written to <label>.meta.json by ash_start).
- ash_start(label) / ash_stop(): one JSON line per rendered frame to <label>.jsonl: perf_counter (QPC, the burst clock),
  WorldInfo.TimeSeconds, every clip's _currentframe/_visible/_alpha (plus _currentlabel on the labelled clips), the
  lift skill's state and timing, and every float field of the cooldown pool. StartActionSkill is marked.
"""
import json
import time
from pathlib import Path

import unrealsdk
from unrealsdk.hooks import Type, add_hook, remove_hook

ASH_TICK = "WillowGame.WillowGameViewportClient:Tick"
ASH_HOOK = "ow_ash_sample"
ASH_CLIPS = ("p1.activeAbility", "p1.activeAbility.bar", "p1.character", "p1.character.cooldown",
             "p1.character.flash", "p1.character.sp")
ASH_LABELLED = ("p1.activeAbility", "p1.character", "p1.character.cooldown", "p1.character.flash", "p1.character.sp")
ASH_POOL_CALLS = ("GetCurrentValue", "GetMaxValue", "GetMinValue", "GetPercentFull", "GetConsumptionRate",
                  "GetRegenerationRate", "GetCurrentRate")
ash = {"pool_calls": [], "file": None, "movie": None, "prefix": "", "pool": None, "pool_fields": [], "probe": None}


def _try(fn):
    try:
        return fn()
    except Exception as error:
        return f"<{type(error).__name__}: {str(error)[:80]}>"


def _live(objs):
    return [o for o in objs if "Default__" not in o._path_name()]


def _hud_movie():
    hud = get_pc().myHUD
    movie = getattr(hud, "HUDMovie", None) if hud is not None else None
    if movie is not None:
        return movie, "PlayerController.myHUD.HUDMovie"
    movies = _live(unrealsdk.find_all("WillowHUDGFxMovie", False))
    return (movies[0], "find_all(WillowHUDGFxMovie)[0] of %d" % len(movies)) if movies else (None, "none")


def _find_pool():
    """The pool whose Definition is ActiveSkillCooldownPool_* and whose AssociatedProvider is this controller (the
    same manager also holds MeleeSkillCooldownPool_*, health, shield, ammo and experience pools)."""
    rows = []
    for pool in _live(unrealsdk.find_all("ResourcePool", False)):
        refs = {}
        for field in pool.Class._fields():
            if field.Class.Name == "ObjectProperty":
                value = _try(lambda: getattr(pool, field.Name))
                if hasattr(value, "_path_name"):
                    refs[field.Name] = value._path_name()
        rows.append((pool, refs))
    pc = get_pc()._path_name()
    hits = [(p, r) for p, r in rows if "ActiveSkillCooldownPool" in r.get("Definition", "")
            and r.get("AssociatedProvider") == pc]
    listing = [{"pool": p._path_name(), "class": p.Class.Name, "refs": r} for p, r in rows]
    return (hits[0][0] if hits else None), listing


def ash_probe():
    movie, how = _hud_movie()
    out = {"movie": movie._path_name() if movie else None, "movie_class": movie.Class.Name if movie else None,
           "movie_found_by": how, "reads": {}}
    if movie is not None:
        for prefix in ("", "_root."):
            for path in ("p1.character", "p1.activeAbility"):
                key = prefix + path
                out["reads"][key] = {
                    "GetVariableNumber(_currentframe)": _try(lambda: movie.GetVariableNumber(key + "._currentframe")),
                    "GetVariableBool(_visible)": _try(lambda: movie.GetVariableBool(key + "._visible")),
                    "GetVariableString(_currentlabel)": _try(lambda: movie.GetVariableString(key + "._currentlabel")),
                    "GetVariableString(_target)": _try(lambda: movie.GetVariableString(key + "._target"))}
    pool, listing = _find_pool()
    out["pool"] = pool._path_name() if pool else None
    out["pool_floats"] = {f.Name: _try(lambda: getattr(pool, f.Name)) for f in pool.Class._fields()
                          if f.Class.Name == "FloatProperty"} if pool else None
    out["pool_calls"] = {name: _try(lambda: getattr(pool, name)()) for name in ASH_POOL_CALLS} if pool else None
    out["pools"] = listing
    return out


def _clip(movie, path):
    p = ash["prefix"] + path
    row = {"frame": movie.GetVariableNumber(p + "._currentframe"), "visible": movie.GetVariableBool(p + "._visible"),
           "alpha": movie.GetVariableNumber(p + "._alpha")}
    if path in ASH_LABELLED:
        row["label"] = movie.GetVariableString(p + "._currentlabel")
    return row


def _sample(*_args):
    if ash["file"] is None:
        return
    try:
        pc = get_pc()
        row = {"t": time.perf_counter(), "game_t": pc.WorldInfo.TimeSeconds}
        movie = ash["movie"]
        row["hud"] = {path: _clip(movie, path) for path in ASH_CLIPS}
        skill = lift_skill()
        if skill is not None:
            row["lift"] = {"state": int(skill.CurrentState), "state_start": skill.StateStartTime,
                           "state_duration": skill.StateDuration, "skill_start": skill.SkillStartTime,
                           "skill_duration": skill.SkillDuration, "lifted": skill.LiftedPawn.Name if skill.LiftedPawn else None}
        pool = ash["pool"]
        if pool is not None:
            row["pool"] = {name: getattr(pool, name) for name in ash["pool_fields"]}
            row["pool"].update({name + "()": getattr(pool, name)() for name in ash["pool_calls"]})
        ash["file"].write(json.dumps(row) + "\n")
        ash["file"].flush()
    except Exception as error:  # never break the game
        ash["file"].write(json.dumps({"t": time.perf_counter(), "error": repr(error)[:300]}) + "\n")


def _on_start_skill(obj, _args, _ret, _func):
    if ash["file"] is not None:
        ash["file"].write(json.dumps({"t": time.perf_counter(), "event": "StartActionSkill"}) + "\n")


def ash_start(label, prefix=""):
    ash_stop()
    folder = Path(RG_OUT)
    folder.mkdir(parents=True, exist_ok=True)
    probe = ash_probe()
    ash["movie"], _ = _hud_movie()
    ash["prefix"] = prefix
    ash["pool"], _ = _find_pool()
    ash["pool_fields"] = [f.Name for f in ash["pool"].Class._fields()
                          if f.Class.Name in ("FloatProperty", "BoolProperty", "IntProperty")] if ash["pool"] else []
    ash["pool_calls"] = [k for k, v in (probe["pool_calls"] or {}).items() if isinstance(v, (int, float))]
    probe["prefix"], probe["pool_fields"] = prefix, ash["pool_fields"]
    (folder / f"{label}.meta.json").write_text(json.dumps(probe, indent=1, default=str), encoding="utf-8")
    ash["file"] = (folder / f"{label}.jsonl").open("w", encoding="utf-8")
    add_hook(ASH_TICK, Type.POST_UNCONDITIONAL, ASH_HOOK, _sample)
    add_hook("WillowGame.WillowPlayerController:StartActionSkill", Type.PRE, ASH_HOOK, _on_start_skill)
    return {"movie": probe["movie"], "pool": probe["pool"], "fields": len(ash["pool_fields"])}


def ash_stop():
    remove_hook(ASH_TICK, Type.POST_UNCONDITIONAL, ASH_HOOK)
    remove_hook("WillowGame.WillowPlayerController:StartActionSkill", Type.PRE, ASH_HOOK)
    if ash["file"] is not None:
        ash["file"].close()
        ash["file"] = None
