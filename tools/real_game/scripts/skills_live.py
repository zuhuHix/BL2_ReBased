"""Skills live probe (prepared 2026-11-04, not yet run): skill grades, the kill-skill timer, damage taken, the instant-hit
crit bonus and the Phaselock action skill's kill events, for the skills lane (docs/verification/NATIVE_SKILL_BEHAVIOURS.md
section 6, questions 1 to 3 and 5 to 7).

Run through Invoke-GamePyFile AFTER scripts/phaselock.py and scripts/phaselock_matched.py (it uses lift_skill(), ai_pawn() and
protect_player() from their namespace). tools/real_game/skills_live_capture.ps1 runs it. Our own code: it writes no file and no
save or config; what it returns goes into the capture's JSON under the ignored local/realgame/skills_live/.

- sl_find_skills(match): SkillDefinition object paths containing `match` (discovery only).
- sl_set_grades({key: grade}) / sl_restore_grades(): grades in memory only, through the controller's skill tree
  (PlayerSkillTree.SetSkillGrade). The value read before the first change of each skill is kept and put back by
  sl_restore_grades(). A key is a full object path or a skill name such as "Helios" or "Mind's Eye" (sl_resolve).
- sl_killskill_watch(seconds): UpdateKillSkills calls and the controller's KillSkillTimeRemaining and
  DurationOfLastKillSkillActivation, sampled every 0.05 s.
- sl_damage_watch(seconds): TakeDamage on every pawn: time, victim, damage, damage type, instigator, victim location.
- sl_killedtarget_watch(seconds): the lift skill's RunCustomEvent (OnKilledTarget and the others), InterruptPhaseLock and
  EndSkill, counted per cast (a cast is keyed by the skill's SkillStartTime).
- sl_crit_bonus(): CurrentInstantHitCriticalHitBonus and the base-value names, if the build exposes them.
- sl_kill(name): one large TakeDamage on an AI pawn from the player's controller (the pattern scripts/phaselock_integrity.py
  uses on the player).
- sl_watch_result(name) / sl_stop_watches() / sl_shutdown(): read the rows and remove the hooks; shutdown also restores grades.

A watch starts hooks and returns at once: a command runs inside a frame and must not sleep, so the capture waits between
commands. Samples are taken in the game's tick hook; each watch stops collecting after its `seconds`.

Every function looks its objects up again on every call (a reference kept from an earlier command can point at a destroyed
object; see the driver's note).

TODO(live) marks what this file uses but no existing script shows. Until the live run confirms each one, the step that needs
it records "not implemented" or the marker "<AttributeError>" rather than a value.
"""
import re
import time

import unrealsdk
from unrealsdk.hooks import Type, add_hook, remove_hook
from unrealsdk.unreal import UObject

TICK = "WillowGame.WillowGameViewportClient:Tick"
TICK_IDENT = "ow_sl_tick"
PAWN_CLASSES = ("WillowPawn", "WillowAIPawn", "WillowPlayerPawn")
SL = globals().get("SL") or {"watches": {}, "originals": {}, "tick": False}


def _p(value, depth=0):
    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    if isinstance(value, UObject):
        return value._path_name()
    return repr(value)[:120]


def _try(fn):
    """fn() as JSON-able data, or a marker naming the exception (an attribute this build does not expose)."""
    try:
        return _p(fn())
    except Exception as error:
        return f"<{type(error).__name__}>"


def _objname(fn):
    def get():
        value = fn()
        return None if value is None else value.Name
    return _try(get)


def _vec(v):
    return [round(v.X, 2), round(v.Y, 2), round(v.Z, 2)]


def _norm(text):
    return re.sub(r"[^a-z0-9]", "", text.lower())


def _declared_chain(cls_name, func_name):
    """Path names of func_name declared in the class and up its SuperField chain (a hook fires for the declaration that runs)."""
    paths, cls = [], unrealsdk.find_class(cls_name)
    while cls:
        for field in cls._fields():
            if field.Name == func_name and field.Outer == cls and field.Class.Name == "Function":
                paths.append(field._path_name())
        cls = cls.SuperField
    return list(dict.fromkeys(paths))


def _defs():
    return [d for d in unrealsdk.find_all("SkillDefinition", exact=False) if "Default__" not in d._path_name()]


# ---------------------------------------------------------------- grades

def _tree():
    # TODO(live): the controller's property that holds the skill tree. PlayerSkillTree is the class name in NATIVE_INDEX.md
    # (with SetSkillGrade); the property name on WillowPlayerController is not seen in any script yet.
    try:
        return get_pc().PlayerSkillTree
    except Exception:
        return None


def sl_find_skills(match):
    m = _norm(match)
    return [d._path_name() for d in _defs() if m in _norm(d._path_name())][:40]


def sl_resolve(key):
    """(object, how). A full object path first (find_object); else the one SkillDefinition whose object name, with spaces and
    punctuation dropped, is the skill name or "Skill" + the skill name ("Mind's Eye" finds Skill_MindsEye or MindsEye)."""
    if "." in key:
        try:
            obj = unrealsdk.find_object("SkillDefinition", key)
        except Exception:
            obj = None
        if obj is not None:
            return obj, "path"
    last = _norm(key.split(".")[-1])
    hits = [d for d in _defs() if _norm(d.Name) in (last, "skill" + last)]
    if len(hits) == 1:
        return hits[0], "name"
    return None, f"{len(hits)} matches"


def _read_grade(tree, defn):
    # TODO(live): GetSkillState (NATIVE_INDEX.md) and its grade field are not seen in any script yet; the marker says which.
    def grade():
        return tree.GetSkillState(defn).Grade
    return {"grade": _try(grade)}


def sl_set_grades(grades):
    """grades: {key: grade}. Sets each skill's grade in memory and records the grade before (first change only) and after."""
    tree = _tree()
    if tree is None:
        return {"status": "not implemented", "reason": "PlayerSkillTree not readable from the controller (TODO(live))"}
    rows = []
    for key, grade in grades.items():
        defn, how = sl_resolve(key)
        row = {"key": key, "grade": int(grade), "resolved": how}
        if defn is None:
            row["status"] = "not found"
            rows.append(row)
            continue
        path = defn._path_name()
        before = _read_grade(tree, defn)
        SL["originals"].setdefault(path, before)
        row["before"] = before
        try:
            tree.SetSkillGrade(defn, int(grade))  # TODO(live): signature from NATIVE_INDEX only; no script has called it yet
            row["status"] = "set"
        except AttributeError as error:
            row["status"] = "not implemented"
            row["error"] = repr(error)[:200]
        except Exception as error:
            row["status"] = "error"
            row["error"] = repr(error)[:200]
        row["after"] = _read_grade(tree, defn)
        rows.append(row)
    return {"status": "done", "rows": rows}


def sl_restore_grades():
    """Puts back every grade recorded by sl_set_grades (the value read before the first change). A skill whose original
    could not be read is reported as not restorable and kept for the next try."""
    tree = _tree()
    rows, keep = [], {}
    for path, before in list(SL["originals"].items()):
        grade = before.get("grade")
        if tree is None or not isinstance(grade, int):
            rows.append({"path": path, "status": "not restorable", "before": before})
            keep[path] = before
            continue
        defn = unrealsdk.find_object("SkillDefinition", path)
        if defn is None:
            rows.append({"path": path, "status": "not found"})
            keep[path] = before
            continue
        try:
            tree.SetSkillGrade(defn, grade)
            rows.append({"path": path, "status": "restored", "grade": grade, "after": _read_grade(tree, defn)})
        except Exception as error:
            rows.append({"path": path, "status": "error", "error": repr(error)[:200]})
            keep[path] = before
    SL["originals"] = keep
    return {"rows": rows}


# ---------------------------------------------------------------- watches

def _ensure_tick():
    if not SL["tick"]:
        add_hook(TICK, Type.POST_UNCONDITIONAL, TICK_IDENT, _on_tick)
        SL["tick"] = True


def _on_tick(*_args):
    now = time.perf_counter()
    for watch in list(SL["watches"].values()):
        sample = watch.get("sample")
        if sample is None or now > watch["until"]:
            continue
        try:
            sample(watch, now)
        except Exception as error:
            watch["rows"].append({"event": "sample_error", "error": repr(error)[:200]})


def _watch_begin(name, seconds, hooks, sample=None):
    """hooks: (path, hook type, identifier, function). Replaces a watch of the same name."""
    _watch_stop(name)
    now = time.perf_counter()
    watch = {"t0": now, "until": now + float(seconds), "rows": [], "failed": [], "sample": sample, "hooks": [], "last": -1.0}
    SL["watches"][name] = watch
    for path, typ, ident, fn in hooks:
        try:
            add_hook(path, typ, ident, fn)
            watch["hooks"].append((path, typ, ident))
        except Exception as error:
            watch["failed"].append(f"{path}: {repr(error)[:120]}")
    if sample is not None:
        _ensure_tick()
    return {"watch": name, "seconds": float(seconds), "hooked": len(watch["hooks"]), "failed": watch["failed"]}


def _watch_stop(name):
    watch = SL["watches"].pop(name, None)
    if watch is None:
        return
    for path, typ, ident in watch["hooks"]:
        try:
            remove_hook(path, typ, ident)
        except Exception:
            pass


def _result(name):
    watch = SL["watches"].get(name)
    if watch is None:
        return {"watch": name, "status": "no such watch"}
    _watch_stop(name)
    out = {"watch": name, "rows": watch["rows"], "failed": watch["failed"]}
    if "casts" in watch:
        out["casts"] = watch["casts"]
    return out


def sl_watch_result(name):
    return _result(name)


def sl_stop_watches():
    return {name: _result(name) for name in list(SL["watches"])}


# kill-skill timer

def _sample_killskill(watch, now):
    if now - watch["last"] < 0.05:
        return
    watch["last"] = now
    pc = get_pc()
    watch["rows"].append({"t": round(now - watch["t0"], 4),
                          "game_t": _try(lambda: pc.WorldInfo.TimeSeconds),
                          "remaining": _try(lambda: pc.KillSkillTimeRemaining),
                          "last_duration": _try(lambda: pc.DurationOfLastKillSkillActivation)})


def _on_update_kill_skills(obj, args, ret, func):
    watch = SL["watches"].get("killskill")
    if watch is None:
        return
    now = time.perf_counter()
    # TODO(live): the argument name bEnable comes from NATIVE_SKILL_BEHAVIOURS 3.1 (UpdateKillSkills(bEnable)); no hook has read it yet.
    watch["rows"].append({"t": round(now - watch["t0"], 4), "event": "UpdateKillSkills", "bEnable": _try(lambda: args.bEnable)})


def sl_killskill_watch(seconds=8.0):
    hooks = [(p, Type.PRE, "ow_sl_killskill", _on_update_kill_skills)
             for p in _declared_chain("WillowPlayerController", "UpdateKillSkills")]
    return _watch_begin("killskill", seconds, hooks, sample=_sample_killskill)


# damage taken by any pawn

def _damage_hook(path):
    via = path.split(":")[0].split(".")[-1]

    def hook(obj, args, ret, func):
        watch = SL["watches"].get("damage")
        if watch is None:
            return
        now = time.perf_counter()
        watch["rows"].append({
            "t": round(now - watch["t0"], 4), "via": via,
            "game_t": _try(lambda: get_pc().WorldInfo.TimeSeconds),
            "victim": _try(lambda: obj.Name), "victim_class": _try(lambda: obj.Class.Name),
            "victim_loc": _try(lambda: _vec(obj.Location)),
            "damage": _try(lambda: args.Damage),
            "damage_type": _objname(lambda: args.DamageType),
            "instigator": _objname(lambda: args.InstigatedBy),
        })
    return hook


def sl_damage_watch(seconds=8.0):
    hooks, seen = [], set()
    for cls in PAWN_CLASSES:
        for path in _declared_chain(cls, "TakeDamage"):
            if path not in seen:
                seen.add(path)
                hooks.append((path, Type.PRE, "ow_sl_damage", _damage_hook(path)))
    return _watch_begin("damage", seconds, hooks)


# Phaselock action skill events

def _cast_key():
    skill = lift_skill()
    return None if skill is None else str(round(skill.SkillStartTime, 3))


def _killedtarget_hook(path):
    fname = path.split(":")[-1]

    def hook(obj, args, ret, func):
        watch = SL["watches"].get("killedtarget")
        if watch is None:
            return
        try:
            now = time.perf_counter()
            event = _try(lambda: str(args.EventName)) if fname == "RunCustomEvent" else None
            label = event if fname == "RunCustomEvent" else fname
            key = _cast_key()
            counts = watch["casts"].setdefault(str(key), {})
            counts[label] = counts.get(label, 0) + 1
            watch["rows"].append({"t": round(now - watch["t0"], 4), "func": fname, "event": event, "cast": key,
                                  "state": _try(lambda: str(lift_skill().CurrentState)),
                                  "lifted": _objname(lambda: lift_skill().LiftedPawn)})
        except Exception as error:
            watch["rows"].append({"event": "hook_error", "error": repr(error)[:200]})
    return hook


def sl_killedtarget_watch(seconds=10.0):
    hooks = []
    for fname in ("RunCustomEvent", "InterruptPhaseLock", "EndSkill"):
        for path in _declared_chain("LiftActionSkill", fname):
            hooks.append((path, Type.PRE, "ow_sl_killedtarget", _killedtarget_hook(path)))
    out = _watch_begin("killedtarget", seconds, hooks)
    SL["watches"]["killedtarget"]["casts"] = {}
    return out


# crit bonus and kills

def sl_crit_bonus():
    pc = get_pc()
    values = {}
    for suffix in ("", "BaseValue", "_BaseValue", "Base"):
        attr = "CurrentInstantHitCriticalHitBonus" + suffix
        values[attr] = _try(lambda: getattr(pc, attr))
    return {"game_t": _try(lambda: pc.WorldInfo.TimeSeconds), "values": values,
            "note": "a marker means this build does not expose that name (the base value is TODO(live))"}


def sl_kill(name):
    """One TakeDamage of 1,000,000 on an AI pawn, instigated by the player's controller.
    TODO(live): whether a script-level TakeDamage starts the kill-skill timer and fires OnKilledTarget is not confirmed."""
    pc = get_pc()
    victim = ai_pawn(name)
    zero = unrealsdk.make_struct("Vector", X=0.0, Y=0.0, Z=0.0)
    before = _try(lambda: victim.GetHealth())
    call = _try(lambda: victim.TakeDamage(1000000, pc, victim.Location, zero, unrealsdk.find_class("DamageType")))
    return {"victim": name, "health_before": before, "health_after": _try(lambda: victim.GetHealth()), "call": call}


def sl_shutdown():
    """Stops every watch, puts back every grade and removes the sampler. The capture calls it before it quits the game."""
    watches = sl_stop_watches()
    grades = sl_restore_grades()
    if SL["tick"]:
        try:
            remove_hook(TICK, Type.POST_UNCONDITIONAL, TICK_IDENT)
        except Exception:
            pass
        SL["tick"] = False
    return {"watches": watches, "grades": grades}
