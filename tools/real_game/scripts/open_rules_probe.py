"""Open-rule checks for the openwillow_realgame command channel (lane L5, 2026-10-07).

Run through Invoke-GamePyFile (tools/real_game/realgame.ps1) AFTER tools/real_game/scripts/combat_probe.py (hooks, per-frame
sampler, snapshots, placement and aim helpers) has been loaded in the same game session. Do NOT load fire_mission_probe.py in the
same session: both files define `FUNCS`, `_p`, `_mk`, `_hook` and `_behavior_paths` in the shared namespace and the later one breaks the
earlier (seen on 2026-10-07: `cb_start` failed after fire_mission_probe.py was loaded). `or_stand_in_front` below is the one helper
taken over from it. Our own code, no game data; memory-only. Rows go to the combat recorder's list
(CB["rows"]) so they share one timeline (QPC clock, game clock, frame counter) with its hook rows.

It only reads state, except for the one documented call `ExpLevelUp(True)` of `or_arm_cheat`, and the in-memory mod switch of
`or_mod_off` (`Mod.disable(dont_update_setting=True)`, no settings write). Nothing here passes None for a struct or object argument
of a native (that crashed lane L4's game).

- or_props(obj, pattern): every scalar property of obj whose name matches the regex (name -> value), for discovery.
- or_pools(): the controller's ResourcePoolManager slots (index, GUID byte, definition, resource, values) and the pool references
  held by the controller and the pawn (manager, slot, GUID byte, whether the cached pool is set).
- or_crit(): the controller's critical-hit attributes (value and base value) and the neighbouring modifiers.
- or_dummy(): the Fire target dummy pawn (health, max, health pool, level, game stage, flags) and its class defaults.
- or_enemy(name): what the kill-experience formula reads for one AI pawn (class, balance, level, experience multiplier attribute)
  plus the player's level and experience; or_enemies(radius) does it for every live AI pawn near the player.
- or_arm_cheat(when, value, label): run the real WillowPlayerController.ExpLevelUp(True) inside a frame when a condition holds (the
  action skill is `value` seconds old, or the cooldown pool has drained below `value`); or_disarm().
- or_xp_hook(): after cb_start(), one row per kill-experience award (PRE and POST of AwardCombatExperienceToParty) with the award, killer,
  killed actor's class, balance, level and multiplier, and the player's level, experience and pool value.
- or_bully(name): one AI pawn's full profile (class, AI class and balance properties, pools, weapon, flags) for discovery;
  or_dump(obj) and or_fn_params(obj, fn) are the discovery helpers it uses.
- or_mod_off(name): switch one installed SDK mod off in memory only (no settings write).
- or_stand_in_front(path, distance): put the player in front of a pawn, looking at its chest (memory only; as lane L2's helper).
- or_snap(label): a labelled row (player state, dummy, near enemies) appended to the combat rows.
"""
import json
import math
import re
import time

import unrealsdk
from unrealsdk.hooks import Type, add_hook, remove_hook

if "CB" not in globals():
    raise RuntimeError("load combat_probe.py first: open_rules_probe.py reuses its recorder and helpers")

MARCUS_PATH = "Sanctuary_Dynamic.TheWorld:PersistentLevel.WillowAIPawn_13"
OR = globals().get("OR") or {"armed": [], "hooked": False}
SCALARS = ("FloatProperty", "IntProperty", "BoolProperty", "ByteProperty", "NameProperty", "StrProperty")


def or_props(obj, pattern, limit=80):
    """Scalar properties of obj (own and inherited) whose name matches the regex, as name -> value."""
    rx = re.compile(pattern, re.I)
    out = {}
    cls = obj.Class
    while cls is not None and len(out) < limit:
        try:
            fields = list(cls._fields())
        except Exception:
            fields = []
        for field in fields:
            try:
                kind = field.Class.Name
                name = field.Name
            except Exception:
                continue
            if name in out or not kind.endswith("Property") or not rx.search(name):
                continue
            try:
                value = getattr(obj, name)
            except Exception:
                continue
            if kind in SCALARS or isinstance(value, (bool, int, float, str)):
                out[name] = _p(value)
            else:
                out[name] = "<" + kind + ">"
        cls = cls.SuperField if hasattr(cls, "SuperField") else None
    return out


def _ref(ref):
    """A ResourcePoolReference as plain data: manager, slot, generation byte, whether the cached pool is set."""
    if ref is None:
        return None
    out = {}
    for key in ("PoolManager", "PoolIndexInManager", "PoolGUID", "Data"):
        try:
            v = getattr(ref, key)
            out[key] = _p(v) if key in ("PoolManager", "Data") else v
        except Exception as e:
            out[key] = "error " + repr(e)[:60]
    return out


def _pool_row(pool):
    row = {"name": _g(pool, "Name"), "class": pool.Class.Name, "guid": _g(pool, "PoolGUID")}
    d = _g(pool, "Definition")
    row["definition"] = _p(d)
    row["resource"] = _p(_g(d, "Resource")) if d is not None else None
    row["provider"] = _p(_g(pool, "AssociatedProvider"))
    for key in ("CurrentValue", "MaxValue", "MinValue"):
        v = _g(pool, key)
        if v is not None:
            row[key] = _r(v, 4)
    return row


def or_pools():
    """Question 2: the slot order of Maya's controller's pool manager and the references the controller and pawn hold."""
    pc = get_pc()
    out = {"g": _game_t(), "slots": [], "refs": {}}
    mgr = _g(pc, "ResourcePoolManager")
    out["manager"] = _p(mgr)
    if mgr is not None:
        out["next_guid"] = _g(mgr, "NextPoolGUID")
        for i, pool in enumerate(list(mgr.ResourcePools)):
            if pool is None:
                continue
            row = _pool_row(pool)
            row["slot"] = i
            out["slots"].append(row)
    for key in ("ExpPool", "SkillCooldownPool", "MeleeSkillCooldownPool", "AccuracyPool", "OffHandAccuracyPool"):
        out["refs"]["pc." + key] = _ref(_g(pc, key))
    pawn = pc.Pawn
    if pawn is not None:
        out["pawn"] = _p(pawn)
        for key in ("HealthPool", "ShieldArmor"):
            out["refs"]["pawn." + key] = _ref(_g(pawn, key))
    return out


def or_crit():
    """Question 3: the controller's critical hit attributes, value and base value, and the neighbouring modifiers."""
    pc = get_pc()
    out = {"g": _game_t(), "class": pc.Class.Name}
    out["found"] = or_props(pc, r"Critical|ExpScaleForCrit")
    for name in ("CurrentInstantHitCriticalHitBonus", "CurrentMeleeCriticalHitBonus"):
        row = {}
        for suffix in ("", "BaseValue", "_BaseValue", "Base"):
            v = _g(pc, name + suffix)
            if v is not None:
                row[name + suffix] = _p(v)
        out[name] = row
    return out


def _find_dummy():
    for pawn in unrealsdk.find_all("WillowAIPawn"):
        if "TargetDummy" in str(_g(pawn, "AIClass")) and pawn.Name.startswith("WillowAIPawn_") and not _g(pawn, "bDeleteMe", False):
            return pawn
    return None


def or_dummy():
    """Question 1: the Fire target dummy's health, maximum, health pool, level, game stage and the flags that decide whether it can die."""
    pawn = _find_dummy()
    if pawn is None:
        return None
    out = _pawn_state(pawn)
    out["path"] = _p(pawn)
    out["ai_class"] = _p(_g(pawn, "AIClass"))
    out["balance"] = _p(_g(pawn, "BalanceDefinitionState"))
    for key in ("HealthVar", "HealthMaxVar", "ExpLevel", "OverrideGameStage", "GameStage", "bCanBeDamaged", "bAllowDeath",
                "bInvulnerable", "bDamageable", "Health", "HealthMax"):
        v = _g(pawn, key)
        if v is not None:
            out[key] = _p(v)
    for fn in ("GetExpLevel", "GetGameStage", "GetBalanceLevel", "GetLevel"):
        try:
            out[fn] = _p(getattr(pawn, fn)())
        except Exception:
            pass
    out["health_pool_ref"] = _ref(_g(pawn, "HealthPool"))
    pool = _pool(_g(pawn, "HealthPool"))
    if pool is not None:
        out["health_pool"] = _pool_row(pool)
        out["health_pool_state"] = _pool_state(pool)
        out["health_pool_def_props"] = or_props(pool.Definition, r".") if _g(pool, "Definition") is not None else None
    out["flags"] = or_props(pawn, r"Invul|Immortal|Mortal|God|Die|Dead|NoDamage|Damageable|Indestruct|Unkill|MinHealth|Health|Level|Stage|Experience")
    try:
        out["game_stage_world"] = _p(get_pc().WorldInfo.GRI.GameStage) if hasattr(get_pc().WorldInfo.GRI, "GameStage") else None
    except Exception:
        pass
    return out


def or_enemy(name):
    """Question 4: what the kill-experience formula reads for one AI pawn, and the player's level and experience."""
    pawn = _find_ai(name)
    if pawn is None:
        return None
    pc = get_pc()
    out = _pawn_state(pawn)
    out["ai_class"] = _p(_g(pawn, "AIClass"))
    out["balance"] = _p(_g(pawn, "BalanceDefinitionState"))
    out["controller"] = _p(_g(pawn, "Controller"))
    for fn in ("GetExpLevel", "GetGameStage"):
        try:
            out[fn] = _p(getattr(pawn, fn)())
        except Exception:
            pass
    out["exp_props"] = or_props(pawn, r"ExperiencePoint|ExpLevel|XPMult|ExpMult")
    try:
        out["player_level"] = pc.Pawn.GetExpLevel()
        out["player_exp"] = pc.GetExpPoints()
    except Exception:
        pass
    pool = _pool(_g(pc, "ExpPool"))
    if pool is not None:
        out["exp_pool"] = _pool_state(pool)
    return out


def or_enemies(radius=3500.0):
    rows = []
    for row in cb_enemies(radius):
        info = or_enemy(row["name"])
        if info is not None:
            info["dist"] = row["dist"]
            rows.append(info)
    return rows


def or_mod_off(name):
    """Switch one installed SDK mod off in memory: Mod.disable(dont_update_setting=True), as lane L4 did. No settings write."""
    import mods_base.mod_list as ml
    for m in ml.mod_list:
        if _g(m, "name") == name:
            was = _g(m, "is_enabled")
            m.disable(dont_update_setting=True)
            return {"name": name, "was": was, "now": _g(m, "is_enabled")}
    return {"name": name, "found": False}


def or_arm_cheat(when, value, label, flag=True, since=None):
    """Call ExpLevelUp(flag) inside a frame when the condition holds (see combat_probe.cb_arm for the conditions).
    'skill_age' only counts a cast whose SkillStartTime is at or after `since` (default: now), so an older cast does not fire it;
    arm after a cast that is known to have locked a target by passing since = that cast's SkillStartTime minus a little."""
    OR["armed"].append({"when": when, "value": value, "label": label, "flag": flag, "fired": False,
                        "armed_at": _game_t() if since is None else since})
    if not OR["hooked"]:
        add_hook(TICK, Type.POST_UNCONDITIONAL, "ow_or_tick", _or_tick)
        OR["hooked"] = True
    return len(OR["armed"])


def or_xp_hook():
    """Question 4: one row per kill-experience award with everything the formula reads, taken inside
    WillowExperiencePipeline.AwardCombatExperienceToParty (PRE and POST): the award passed in, killer, killed actor (class, balance,
    level, experience multiplier attribute), the player's level and experience before and after."""
    paths = _declared_paths("WillowExperiencePipeline", "AwardCombatExperienceToParty")

    def make(tag):
        def hook(obj, args, ret, func):
            if not CB["on"]:
                return
            pc = get_pc()
            killed, killer = _g(args, "KilledActor"), _g(args, "Killer")
            row = {"t": round(time.perf_counter() - CB["t0"], 4), "g": _game_t(), "f": CB["frame"], "tag": "or_xp_" + tag,
                   "award": _g(args, "ExperiencePoints"), "killer": _p(killer), "killed": _p(killed),
                   "killer_is_player": _p(killer) == _p(pc), "player_level": pc.Pawn.GetExpLevel(),
                   "player_exp": pc.GetExpPoints()}
            pool = _pool(_g(pc, "ExpPool"))
            if pool is not None:
                row["exp_pool_value"] = _r(_g(pool, "CurrentValue"), 4)
            if killed is not None:
                row["killed_class"] = killed.Class.Name
                row["killed_ai_class"] = _p(_g(killed, "AIClass"))
                row["killed_balance"] = _p(_g(killed, "BalanceDefinitionState"))
                row["killed_level"] = _g(killed, "ExpLevel")
                row["killed_exp_props"] = or_props(killed, r"ExperiencePoint|ExpLevel|XPMult|ExpMult")
                try:
                    row["killed_level_fn"] = killed.GetExpLevel()
                except Exception:
                    pass
            CB["rows"].append(row)
        return hook
    done = []
    for path in paths:
        done.append(_hook(path, Type.PRE, "ow_or_xp_pre_" + path, make("pre")))
        done.append(_hook(path, Type.POST, "ow_or_xp_post_" + path, make("post")))
    return {"paths": paths, "installed": done}


def or_fn_params(obj, name):
    """The parameter names of a script function of obj (to check a flag's meaning before calling it)."""
    func = obj.Class._find(name)
    return [(q.Name, q.Class.Name) for q in func._properties()]


def or_dump(obj, limit=300):
    """Every Property of obj (own and inherited) as plain data (objects as path names, arrays up to 16 entries); discovery only."""
    out = {}
    if isinstance(obj, WrappedStruct):
        for q in obj._type._properties():
            try:
                out[q.Name] = _p(getattr(obj, q.Name))
            except Exception as e:
                out[q.Name] = "error " + repr(e)[:60]
        return out
    cls = obj.Class
    while cls is not None and len(out) < limit:
        try:
            fields = list(cls._fields())
        except Exception:
            fields = []
        for field in fields:
            try:
                if not field.Class.Name.endswith("Property"):
                    continue
                name = field.Name
            except Exception:
                continue
            if name in out:
                continue
            try:
                out[name] = _p(getattr(obj, name))
            except Exception as e:
                out[name] = "error " + repr(e)[:60]
        cls = cls.SuperField if hasattr(cls, "SuperField") else None
    return out


def or_bully(name):
    """The bullymong profile: class, AI class and its data, balance definition, level, health, shield, pools, experience multiplier,
    invulnerability flags, weapon and drop related properties. Reads only."""
    pawn = _find_ai(name)
    if pawn is None:
        return None
    out = or_enemy(name)
    out["path"] = _p(pawn)
    out["class"] = pawn.Class.Name
    out["pawn_props"] = or_dump(pawn)
    ai = _g(pawn, "AIClass")
    if ai is not None:
        out["ai_class_props"] = or_dump(ai)
    bal = _g(pawn, "BalanceDefinitionState")
    if bal is not None:
        out["balance_state"] = or_dump(bal)
        bdef = _g(bal, "BalanceDefinition")
        if bdef is not None:
            out["balance_def"] = _p(bdef)
            out["balance_props"] = or_dump(bdef)
    ctrl = _g(pawn, "Controller")
    if ctrl is not None:
        out["controller_class"] = ctrl.Class.Name
        out["controller_props"] = or_props(ctrl, r"Mind|AI|Behavior|Class|State|Enemy|Target", 60)
    out["flags"] = or_props(pawn, r"Invul|Immortal|Mortal|God|Die|Dead|NoDamage|Damageable|Indestruct|Unkill|MinHealth")
    hp = _pool(_g(pawn, "HealthPool"))
    if hp is not None:
        out["health_pool"] = _pool_row(hp)
        out["health_pool_state"] = _pool_state(hp)
    sp = _pool(_g(pawn, "ShieldArmor"))
    if sp is not None:
        out["shield_pool"] = _pool_row(sp)
    for key in ("HealthPool", "ShieldArmor"):
        out["ref." + key] = _ref(_g(pawn, key))
    mgr = _g(pawn, "ResourcePoolManager")
    if mgr is None and ctrl is not None:
        mgr = _g(ctrl, "ResourcePoolManager")
    if mgr is not None:
        slots = []
        for i, pool in enumerate(list(mgr.ResourcePools)):
            if pool is not None:
                row = _pool_row(pool)
                row["slot"] = i
                slots.append(row)
        out["pool_slots"] = slots
    weapon = _g(pawn, "Weapon")
    if weapon is not None:
        out["weapon"] = {"class": weapon.Class.Name, "path": _p(weapon), "props": or_props(weapon, r"Damage|Definition|Type|Melee|Range|Fire", 60)}
    return out


def or_disarm():
    OR["armed"] = []


def _or_tick(*_a):
    if not OR["armed"]:
        return
    try:
        pc = get_pc()
        for item in OR["armed"]:
            if item["fired"]:
                continue
            due = False
            if item["when"] == "skill_age":
                skill = _lift_skill()
                now = pc.WorldInfo.TimeSeconds
                due = (skill is not None and skill.SkillStartTime >= item["armed_at"] and skill.SkillStartTime > 0
                       and now - skill.SkillStartTime >= item["value"])
            elif item["when"] == "cd_below":
                cd = _pool(_g(pc, "SkillCooldownPool"))
                due = cd is not None and 0 < cd.CurrentValue <= item["value"] and cd.ConsumptionRate > 0
            elif item["when"] == "now":
                due = True
            if due:
                item["fired"] = True
                item["g"] = round(pc.WorldInfo.TimeSeconds, 4)
                item["f"] = CB["frame"]
                item["t"] = round(time.perf_counter() - CB["t0"], 4)
                item["cd_before"] = _cooldown(pc)
                item["level_before"] = _g(pc.Pawn, "ExpLevel")
                item["wdmg_before"] = _weapon_damage(pc.Pawn)
                pc.ExpLevelUp(item["flag"])
                item["cd_after_call"] = _cooldown(pc)
                item["level_after"] = _g(pc.Pawn, "ExpLevel")
                CB["rows"].append({"or_cheat_fired": dict(item)})
    except Exception as e:
        CB["rows"].append({"or_tick_error": repr(e)[:200]})


def or_snap(label, radius=3500.0):
    """A labelled row: the combat snapshot (player, enemies, pickups) plus the dummy if there is one."""
    row = cb_snap(label, radius)
    try:
        row["dummy"] = or_dummy()
    except Exception as e:
        row["dummy_error"] = repr(e)[:150]
    return row


def or_stand_in_front(path=MARCUS_PATH, distance=110.0, aim_up=20.0):
    target = unrealsdk.find_object("WillowAIPawn", path)
    me = get_pc()
    yaw = target.Rotation.Yaw * 360.0 / 65536.0
    x = target.Location.X + math.cos(math.radians(yaw)) * distance
    y = target.Location.Y + math.sin(math.radians(yaw)) * distance
    z = target.Location.Z + 40.0
    me.Pawn.Location = unrealsdk.make_struct("Vector", X=x, Y=y, Z=z)
    me.Pawn.SetPhysics(2)
    dx, dy = target.Location.X - x, target.Location.Y - y
    eye = z + getattr(me.Pawn, "BaseEyeHeight", 64.0)
    pitch = math.degrees(math.atan2(target.Location.Z + aim_up - eye, math.hypot(dx, dy)))
    yaw = math.degrees(math.atan2(dy, dx))
    # ClientSetRotation: SetRotation kept the pitch at 0 in this build (lane L2); the unsigned 16-bit form is what the game stores.
    me.ClientSetRotation(unrealsdk.make_struct("Rotator", Pitch=int(pitch * 65536 / 360) % 65536, Yaw=int(yaw * 65536 / 360) % 65536, Roll=0))
    return [round(x, 1), round(y, 1), round(z, 1)]


def or_pending_marks():
    return {"armed": [dict(a) for a in OR["armed"]]}
