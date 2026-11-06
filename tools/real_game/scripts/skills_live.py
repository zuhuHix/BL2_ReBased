"""Skills live probe (first run 2026-11-06, fixed after it; see NATIVE_SKILL_BEHAVIOURS.md): skill grades, the kill-skill timer, damage taken, the instant-hit
crit bonus and the Phaselock action skill's kill events, for the skills lane (docs/verification/NATIVE_SKILL_BEHAVIOURS.md
section 6, questions 1 to 3 and 5 to 7).

Run through Invoke-GamePyFile AFTER scripts/phaselock.py and scripts/phaselock_matched.py (it uses lift_skill(), ai_pawn() and
protect_player() from their namespace). tools/real_game/skills_live_capture.ps1 runs it. Our own code: it writes no file and no
save or config; what it returns goes into the capture's JSON under the ignored local/realgame/skills_live/.

- sl_find_skills(match): SkillDefinition object paths containing `match` (discovery only).
- sl_set_grades({key: grade}) / sl_restore_grades(): grades in memory only, through the path a save load uses
  (WillowPlayerController.ServerSetSkillSaveGameData: tree grade, the running skill's grade, activation), and read back. The
  value read before the first change of each skill is kept and put back by sl_restore_grades(). A key is a full object path or
  a skill name such as "Helios" or "Mind's Eye" (sl_resolve).
- sl_killskill_watch(seconds): UpdateKillSkills calls and the controller's KillSkillTimeRemaining and
  DurationOfLastKillSkillActivation, sampled every 0.05 s.
- sl_damage_watch(seconds): TakeDamage on every pawn: time, victim, damage, damage type, instigator, victim location.
- sl_killedtarget_watch(seconds): the lift skill's RunCustomEvent (OnKilledTarget and the others), InterruptPhaseLock and
  EndSkill, counted per cast (a cast is keyed by the skill's SkillStartTime).
- sl_crit_bonus(): every controller property whose name holds CriticalHitBonus, modifier stacks as (type, value) rows, and the
  held weapon's name (what else may sit on the stack).
- sl_wound(name) / sl_fire(on) / sl_is_dead(name): leave a pawn at 1 health by one TakeDamage, fire the held gun through the
  controller (the call the fire button makes), and whether it died: a real kill for the kill-skill timer.
- sl_kill(name): one large TakeDamage on an AI pawn from the player's controller; the fallback when the gun missed (the
  capture records which one killed).
- sl_bullymongs(count): adult bullymongs to use; a pawn that cannot be read (a corpse) is left out, not an error.
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
    # WillowPlayerController.PlayerSkillTree (WillowGame.upk; it read back on the first run).
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
    # GetSkillState(SkillDef, out OutSkillState) returns (found, state); SkillTreeSkillStateData.SkillGrade (WillowGame.upk).
    def grade():
        found, state = tree.GetSkillState(defn, unrealsdk.make_struct("SkillTreeSkillStateData"))
        return state.SkillGrade if found else None
    return {"grade": _try(grade)}


def _apply_grade(tree, defn, grade):
    """The save-load path sets the tree grade, the running skill's grade and activation. An integer read back is the proof."""
    get_pc().ServerSetSkillSaveGameData(defn, int(grade))
    return _read_grade(tree, defn)


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
        row["before"] = before
        # No grade is changed unless its original can be put back and checked.
        if path not in SL["originals"] and not isinstance(before.get("grade"), int):
            row["status"] = "original not readable"
            rows.append(row)
            continue
        SL["originals"].setdefault(path, before)
        try:
            row["after"] = _apply_grade(tree, defn, grade)
            # "set" only when the read-back shows the grade: a step does not run on an unconfirmed grade.
            row["status"] = "set" if row["after"].get("grade") == int(grade) else "not confirmed"
        except Exception as error:
            row["status"] = "error"
            row["error"] = repr(error)[:200]
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
            after = _apply_grade(tree, defn, grade)
            confirmed = after.get("grade") == grade
            rows.append({"path": path, "status": "restored" if confirmed else "not restored", "grade": grade, "after": after})
            if not confirmed:
                keep[path] = before
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
    # The argument name is from NATIVE_SKILL_BEHAVIOURS 3.1; the marker says so if the hook cannot read it.
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


def _sample_target_health(name):
    """Target health every 0.05 s, one row per change: damage the TakeDamage hooks may not see (native radius damage)."""
    def sample(watch, now):
        if now - watch["last"] < 0.05:
            return
        watch["last"] = now
        health = _try(lambda: ai_pawn(name).GetHealth())
        if health != watch.get("health"):
            watch["health"] = health
            watch["rows"].append({"t": round(now - watch["t0"], 4), "event": "target_health", "target": name, "health": health})
    return sample


def sl_damage_watch(seconds=8.0, target=None):
    hooks, seen = [], set()
    for cls in PAWN_CLASSES:
        for path in _declared_chain(cls, "TakeDamage"):
            if path not in seen:
                seen.add(path)
                hooks.append((path, Type.PRE, "ow_sl_damage", _damage_hook(path)))
    return _watch_begin("damage", seconds, hooks, sample=_sample_target_health(target) if target else None)


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


def _sample_maya_health(watch, now):
    """Maya's health, one row per change: what Sweet Release's orbs heal."""
    if now - watch["last"] < 0.05:
        return
    watch["last"] = now
    health = _try(lambda: get_pc().Pawn.GetHealth())
    if health != watch.get("maya_health"):
        watch["maya_health"] = health
        watch["rows"].append({"t": round(now - watch["t0"], 4), "event": "maya_health", "health": health})


def sl_maya_health(fraction):
    """Maya's health set to this fraction of her maximum (in memory), so a heal shows; a marker if the call does not exist."""
    pawn = get_pc().Pawn
    before = _try(lambda: pawn.GetHealth())
    maximum = _try(lambda: pawn.GetMaxHealth())
    call = _try(lambda: pawn.SetHealth(maximum * float(fraction)))
    return {"before": before, "max": maximum, "call": call, "after": _try(lambda: pawn.GetHealth())}


def sl_killedtarget_watch(seconds=10.0):
    hooks = []
    for fname in ("RunCustomEvent", "InterruptPhaseLock", "EndSkill"):
        for path in _declared_chain("LiftActionSkill", fname):
            hooks.append((path, Type.PRE, "ow_sl_killedtarget", _killedtarget_hook(path)))
    out = _watch_begin("killedtarget", seconds, hooks, sample=_sample_maya_health)
    SL["watches"]["killedtarget"]["casts"] = {}
    return out


# crit bonus and kills

def _plain_stack(value):
    """A modifier stack as (type, value) rows; anything else as _p gives it."""
    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    try:
        rows = list(value)
    except TypeError:
        return _p(value)
    return [{"type": _try(lambda m=mod: str(m.Type)), "value": _try(lambda m=mod: m.Value)} for mod in rows]


def sl_crit_bonus():
    """Every controller property named *CriticalHitBonus*, stacks included, and the gun in hand."""
    pc = get_pc()
    names, cls = [], pc.Class
    while cls:
        names += [f.Name for f in cls._fields() if "CriticalHitBonus" in f.Name and f.Class.Name != "Function"]
        cls = cls.SuperField
    values = {}
    for name in dict.fromkeys(names):
        try:
            values[name] = _plain_stack(getattr(pc, name))
        except Exception as error:
            values[name] = f"<{type(error).__name__}>"
    return {"game_t": _try(lambda: pc.WorldInfo.TimeSeconds), "values": values,
            "weapon": _objname(lambda: pc.Pawn.Weapon)}


def sl_is_dead(name):
    """True when the pawn is gone, marked for deletion or at no health (a corpse may stop being readable)."""
    try:
        pawn = ai_pawn(name)
        return bool(pawn.bDeleteMe) or pawn.GetHealth() <= 0
    except Exception:
        return True


def sl_wound(name):
    """Leaves the pawn at 1 health, so one real shot kills it."""
    pc = get_pc()
    victim = ai_pawn(name)
    zero = unrealsdk.make_struct("Vector", X=0.0, Y=0.0, Z=0.0)
    before = victim.GetHealth()
    if before > 1:
        victim.TakeDamage(int(before - 1), pc, victim.Location, zero, unrealsdk.find_class("DamageType"))
    return {"victim": name, "before": before, "after": victim.GetHealth()}


def sl_fire(on):
    """Hold or release the fire button's call (scripts/weapon_firing_probe.py fp_fire_start and fp_fire_stop)."""
    pc = get_pc()
    (pc.StartFire if on else pc.StopFire)(0)
    return {"firing": bool(on)}


def sl_bullymongs(count):
    """Adult bullymongs (radius 120 or more), alive with a controller, nearest first. One row's failure (a corpse) drops only it."""
    rows, skipped = [], []
    for dist, name, _cls, dead in list_ai():
        try:
            pawn = ai_pawn(name)
            cls = pawn.AIClass._path_name() if pawn.AIClass else ""
            radius = pawn.CylinderComponent.CollisionRadius if pawn.CylinderComponent else 0
            rows.append({"name": name, "dist": dist, "class": cls.split(".")[-1], "radius": radius, "dead": dead,
                         "ctrl": pawn.Controller is not None, "hp": pawn.GetHealth()})
        except Exception as error:
            skipped.append({"name": name, "error": repr(error)[:120]})
    ok = [r for r in rows if not r["dead"] and r["ctrl"] and r["hp"] > 1 and "PrimalBeast" in r["class"] and r["radius"] >= 120]
    ok.sort(key=lambda r: (not r["class"].endswith("CharClass_PrimalBeast"), r["dist"]))
    return {"picks": [r["name"] for r in ok[:int(count)]], "rows": rows[:30], "skipped": skipped}


def sl_kill(name):
    """One TakeDamage of 1,000,000 on an AI pawn, instigated by the player's controller. The fallback after sl_fire: on the first
    run this kind of kill did not start the kill-skill timer, so a step that used it says so."""
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
