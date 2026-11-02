"""Combat golden-trace recorder for the openwillow_realgame command channel (lane L3, 2026-10-07).

Run through Invoke-GamePyFile (tools/real_game/realgame.ps1); our own code, no game data. Rows go to RG_OUT (the ignored
local/realgame/...). Memory-only: the only writes to the game are the documented script calls the caller makes by hand
(placing the player, aiming the camera, travelling, ExpLevelUp for the cooldown check, switching a mod's hooks off). It
follows tools/real_game/scripts/fire_mission_probe.py: Python hooks see only calls that go through script (ProcessEvent), so a
native-to-native call (the pipeline's shield stage, the OnTakeDamage raise, pool regeneration) is invisible and "not seen"
never means "did not happen".

- cb_start(): hooks (PRE, and POST where it matters) on the victim's TakeDamage chain, the death and drop path, the kill
  experience path, the pickup path, the player's hit, injured ("fight for your life"), second wind, death and respawn
  functions, the action-skill cooldown and the level-up functions, plus every `Behavior_*` class's ApplyBehaviorToContext.
  One row per call: QPC seconds since the start, the game's own clock (WorldInfo.TimeSeconds), a frame counter, the function,
  the object and the arguments; for the key functions also a small state read of the pawn (health, shield, max, regen).
- cb_frames(on): one compact row per rendered frame (player health/shield/regen, injured state and timers, the cooldown pool,
  the focus enemy's health), for recharge delay, bleed-out and cooldown timing.
- cb_snap(label): a labelled state read (player, nearby enemies, pickups, experience, credits, cooldown pool).
- cb_enemies(radius), cb_pickups(radius), cb_place(name, distance, bearing), cb_aim(name): setup helpers (script, memory only).
- cb_mods(): which of the installed SDK mods that change combat or loot are on; cb_mods_off(): switch the loot mods' hooks and
  keybinds off in memory (never through a settings write); cb_mods_state() to read it back.
- cb_arm(when, value, label): run the real WillowPlayerController.ExpLevelUp(False) inside a frame when a condition holds (the
  action skill is `value` seconds old, or the cooldown pool has drained below `value`), so a level-up lands at a known cooldown
  state without command latency (part B). cb_disarm() clears it.
- cb_dump(name) / cb_rows(start) / cb_hooked(): output and which hook targets resolved or did not.

Typical flow: load the file, cb_start(), cb_frames(True), cb_snap('before'), then real inputs with realgame.ps1, cb_snap(...)
between steps, cb_dump(...). The recorder state CB is kept in the shared namespace on purpose: loading this file again keeps
the rows and the hook list.
"""
import json
import math
import time

import unrealsdk
from unrealsdk.hooks import Type, add_hook, remove_hook
from unrealsdk.unreal import UObject, WrappedArray, WrappedStruct

TICK = "WillowGame.WillowGameViewportClient:Tick"
CB = globals().get("CB") or {"on": False, "rows": [], "frames": [], "hooks": [], "t0": None, "frame": 0, "resolved": [],
                             "missing": [], "failed": [], "counts": {}, "sample": False, "focus": None, "armed": []}

# (class, [functions]); every declaration of the function in the class and in the class's states is hooked (a hook fires only
# for the declaration that actually runs, which is the most derived override of the object's class).
FUNCS = [
    # the victim's TakeDamage chain and death
    ("Pawn", ["TakeDamage", "HandleHealthDepleted", "Died", "PlayDying", "NotifyTakeHit", "HealDamage", "SetHealth"]),
    ("WillowPawn", ["TakeDamage", "AdjustDamage", "TookDamageFromEnemy", "TookDamageFromFriendly", "HealDamage",
                    "HandleHealthDepleted", "Died", "PlayDying", "DropLootOnDeath", "ProcessDamageTakenSkillEvents",
                    "ProcessDamageDealtSkillEvents", "NotifyDamageTaken", "OnShieldDepleted", "OnShieldFull", "ShieldTriggered",
                    "ClientShieldTriggered", "CheckGoToDyingState", "AddPoolItems", "AddDefaultInventory"]),
    ("WillowAIPawn", ["TakeDamage", "Died", "PlayDying", "TookDamageFromFriendly", "CheckInjured"]),
    ("WillowPlayerPawn", ["TakeDamage", "AdjustDamage", "HandleHealthDepleted", "KilledEnemy", "ShowRespawnDialog",
                          "ClientShowRespawnDialog", "ResurrectPlayer", "ResurrectAtLocation", "EndInjuredDeadCamera",
                          "EndInjuredCamera", "SetInjuredState", "SetInjuredDeadState", "StartInjuredPreDeathSequence",
                          "StartInjuredDeathSequence", "SetupPlayerInjuredState", "ClearPlayerInjuredState", "CheckLowHealthState"]),
    # the player's down state, second wind, death, respawn (WillowPawn script)
    ("WillowPawn", ["PlayInjured", "GoFromInjuredToHealthy", "GoToInjuredAndTargeted", "GoFromInjuredToMostlyDead",
                    "RejuvenateFromInjured", "ResurrectPlayer", "ResurrectAtLocation", "KilledEnemy", "SetInjuredState",
                    "SetInjuredDeadState", "StartInjuredPreDeathSequence", "StartInjuredDeathSequence", "InjuredRespawn",
                    "RevivePawnHealthAndShield", "InjuredDeathAnimDone", "EndInjuredState", "ShouldDelaySecondWind",
                    "SetSecondWindReason", "ClearSecondWindReason", "SetBeingRevived", "ClientOnRevived", "InjuredSuicide",
                    "SetupPlayerInjuredState", "ClearPlayerInjuredState"]),
    ("WillowPlayerController", ["PawnDied", "SetupInjuredState", "ClearInjuredState", "SetupControllerInjuredState",
                                "ClearControllerInjuredState", "InjuredNoTimeout", "ClientOnRevivedPlayer"]),
    ("WillowGameInfo", ["Killed", "AwardCombatExperience", "ShouldRespawn", "HandlePlayerDeathResurrection", "ResurrectAllPlayers",
                        "TravelToStation", "InitiateTravel"]),
    ("GameInfo", ["Killed", "ReduceDamage"]),
    ("WillowExperiencePipeline", ["CalculateExperiencePointsForKill", "AwardCombatExperienceToParty"]),
    ("WillowPlayerController", ["ExpEarn", "ExpLevelUp", "OnExpLevelChange", "ClientOnExpLevelChange",
                                "RecalculateAttributeInitializedState"]),
    ("WillowPlayerReplicationInfo", ["AddCurrencyOnHand"]),
    ("MissionTracker", ["NotifyPawnDied"]),
    # status effects, AI events that are script-visible (the OnTakeDamage raise itself is native)
    ("StatusEffectsComponent", ["RollChanceForStatusEffect", "ApplyStatusEffect", "OwnerDied"]),
    ("AIClassDefinition", ["OnTakeDamage", "OnVehicleTakeDamage", "OnUsed"]),
    ("BodyClassDeathDefinition", ["OnKilledBy", "OnDeathNonGib", "OnDeathGib", "OnPlayDeathPizazz"]),
    ("AIDefinition", ["OnKilledPawn"]),
    # loot: pool rolls reached through script, pickups, inventory
    ("ItemPool", ["SpawnBalancedInventoryFromPool", "SpawnBalancedInventoryFromInventoryBalanceDefinition"]),
    ("WillowItem", ["InitializeInventory"]),
    ("WillowPickup", ["InitializeFromInventory", "PostBeginPlay", "InitializeDropLifespan", "Landed", "PickupAtRest",
                      "AdjustPickupPhysicsAndCollisionForBeingDropped", "GiveTo", "PickedUpBy", "DenyPickupAttempt",
                      "FailedPickup", "BeginShrinking", "Destroyed", "InventoryAssociated"]),
    ("WillowPlayerController", ["TouchedPickupable", "UnTouchedPickupable", "PickupSomething", "HandlePickup",
                                "ServerAnnounceWeaponPickup", "ServerAnnounceItemPickup", "NotifyUnableToAffordPickupable"]),
    ("WillowInventoryManager", ["AddInventory", "AddInventoryToBackpack", "RemoveFromInventory", "ReadyBackpackInventory"]),
    # action skill cooldown and level-up behaviors (part B)
    ("WillowPlayerController", ["StartActionSkill", "ActionSkillCallback", "StartActiveSkillCooldown", "ResetSkillCooldown",
                                "Behavior_ResetActionSkillCooldown", "ActionSkillCooldownComplete"]),
    ("Behavior_AttributeEffect", ["ApplyBehaviorToContext"]),
    ("Behavior_ActivateSkill", ["ApplyBehaviorToContext"]),
    ("SkillEffectManager", ["ActivateSkill", "DeactivateSkill"]),
    ("LiftActionSkill", ["PhaseLockTarget", "ReleaseTarget", "Fizzled"]),
]
# functions whose rows also carry a pawn state read (health and shield after the call, for a snapshot "after each hit")
STATE_FUNCS = {"TakeDamage", "HandleHealthDepleted", "Died", "PlayInjured", "GoFromInjuredToHealthy", "GoToInjuredAndTargeted",
               "GoFromInjuredToMostlyDead", "ResurrectPlayer", "ResurrectAtLocation", "ShowRespawnDialog", "KilledEnemy",
               "InjuredRespawn", "RevivePawnHealthAndShield", "RejuvenateFromInjured", "SetInjuredState",
               "SetInjuredDeadState", "StartInjuredDeathSequence", "StartInjuredPreDeathSequence", "EndInjuredDeadCamera",
               "OnShieldDepleted", "ShieldTriggered"}
# the installed SDK mods that change combat or loot (legacy mods: object name, how they work)
LOOT_MODS = ("Drop Chance Multiplier", "Loot Collector", "Be Gone Out Of Bounds Loot")


def _p(value, depth=0):
    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    if isinstance(value, UObject):
        return value._path_name()
    if depth >= 2:
        return repr(value)[:120]
    if isinstance(value, WrappedStruct):
        out = {}
        for q in value._type._properties():
            try:
                out[q.Name] = _p(getattr(value, q.Name), depth + 1)
            except Exception:
                pass
        return out
    if isinstance(value, WrappedArray):
        return [_p(v, depth + 1) for v in list(value)[:16]]
    return repr(value)[:120]


def _g(obj, name, default=None):
    try:
        return getattr(obj, name)
    except Exception:
        return default


def _r(x, n=3):
    return round(x, n) if isinstance(x, float) else x


def _args(args, func):
    out = {}
    try:
        for q in func.func._properties():
            if hasattr(args, q.Name):
                out[q.Name] = _p(getattr(args, q.Name))
    except Exception as e:
        out["args_error"] = repr(e)[:100]
    return out


def _game_t():
    try:
        return round(get_pc().WorldInfo.TimeSeconds, 4)
    except Exception:
        return None


def _pool(ref):
    try:
        return ref.Data
    except Exception:
        return None


def _pool_state(pool):
    """The attributes of a resource pool that matter for health, shield and cooldown timing."""
    if pool is None:
        return None
    out = {}
    for key in ("CurrentValue", "MaxValue", "MinValue", "MaxValueBaseValue", "ConsumptionRate", "ActiveRegenerationRate",
                "OnIdleRegenerationRate", "OnIdleRegenerationDelay", "PassiveRegenerationRate", "PoolIdleDelayStartTime",
                "RegenerationDisabled", "IsRegenerating", "RecentImpulseCount"):
        v = _g(pool, key)
        if v is not None:
            out[key] = _r(v, 4)
    return out


def _pawn_state(pawn):
    """Health, shield and their limits of any pawn, through the pawn's own getters and its pools."""
    d = {"name": _g(pawn, "Name")}
    for key, fn in (("hp", "GetHealth"), ("hp_max", "GetMaxHealth"), ("sh", "GetShieldStrength"), ("sh_max", "GetMaxShieldStrength")):
        try:
            d[key] = _r(getattr(pawn, fn)(), 3)
        except Exception:
            pass
    sp = _pool(_g(pawn, "ShieldArmor"))
    if sp is not None:
        d["sh_pool"] = _pool_state(sp)
    for key in ("bIsInjured", "InjuredState", "InjuredDeadState", "bAwaitingInjuredRespawn", "bAwaitingRespawn",
                "bPlayedDeath", "bDeleteMe"):
        v = _g(pawn, key)
        if v is not None:
            d[key] = v
    return d


def _cooldown(pc):
    pool = _pool(_g(pc, "SkillCooldownPool"))
    return _pool_state(pool)


def _weapon_damage(pawn):
    try:
        weapon = pawn.Weapon
        return _r(float(weapon.InstantHitDamage), 3)
    except Exception:
        return None


def _player_state():
    pc = get_pc()
    pawn = pc.Pawn
    d = {}
    if pawn is None:
        d["pawn"] = None
        return d
    d.update(_pawn_state(pawn))
    hp = _pool(_g(pawn, "HealthPool"))
    if hp is not None:
        d["hp_pool"] = _pool_state(hp)
    for key in ("flInjuredTargetedTime", "TotalBleedoutTime", "SuccessiveBleedoutTimePenalty", "NumSuccessiveInjuries",
                "LastInjuryTime", "InjuredStartTime", "bIsInInjuredBonusTime", "bBeingRevived", "LastResurrectReason",
                "InjuredBonusDamageScale", "bInjuredNoTimeout"):
        v = _g(pawn, key)
        if v is not None:
            d[key] = _r(v, 4) if isinstance(v, float) else (v if isinstance(v, (bool, int)) else _p(v))
    try:
        d["pos"] = [round(pawn.Location.X, 1), round(pawn.Location.Y, 1), round(pawn.Location.Z, 1)]
    except Exception:
        pass
    try:
        d["level"] = pawn.GetExpLevel()
        d["exp"] = pc.GetExpPoints()
    except Exception:
        pass
    try:
        d["currency"] = _p(pc.PlayerReplicationInfo.GetCurrencyOnHand(0))
    except Exception as e:
        d["currency_error"] = repr(e)[:80]
    d["cooldown"] = _cooldown(pc)
    d["weapon_damage"] = _weapon_damage(pawn)
    return d


def _mk(path, tag, name):
    with_state = name in STATE_FUNCS

    def hook(obj, args, ret, func):
        if not CB["on"]:
            return
        CB["counts"][path] = CB["counts"].get(path, 0) + 1
        row = {"t": round(time.perf_counter() - CB["t0"], 4), "g": _game_t(), "f": CB["frame"], "tag": tag, "func": path,
               "obj": _p(obj), "args": _args(args, func)}
        if tag == "post":
            try:
                row["ret"] = _p(ret)
            except Exception:
                row["ret"] = "?"
        if with_state:
            try:
                row["state"] = _pawn_state(obj)
            except Exception:
                pass
        CB["rows"].append(row)

    return hook


def _hook(path, typ, ident, fn):
    try:
        add_hook(path, typ, ident, fn)
        CB["hooks"].append((path, typ, ident))
        return True
    except Exception as e:
        CB["failed"].append(f"{path}: {repr(e)[:80]}")
        return False


def _declared_paths(cls_name, func_name):
    """Path names of every declaration of func_name in the class and in its states."""
    cls = unrealsdk.find_class(cls_name)
    paths = []
    for field in cls._fields():
        kind = field.Class.Name
        if kind == "Function" and field.Name == func_name and field.Outer == cls:
            paths.append(field._path_name())
        elif kind == "State":
            try:
                for inner in field._fields():
                    if inner.Class.Name == "Function" and inner.Name == func_name:
                        paths.append(inner._path_name())
            except Exception:
                pass
    return paths


def _behavior_paths():
    seen = {}
    for cls in unrealsdk.find_all("Class"):
        name = cls.Name
        if not name.startswith("Behavior_"):
            continue
        try:
            path = cls._find("ApplyBehaviorToContext")._path_name()
        except Exception:
            continue
        seen[path] = name
    return seen


def cb_start(behaviors=True):
    cb_stop()
    CB.update(rows=[], frames=[], t0=time.perf_counter(), frame=0, resolved=[], missing=[], failed=[], counts={}, on=True)

    def tick(*_a):
        CB["frame"] += 1
        if CB["sample"]:
            _frame_row()
        if CB["armed"]:
            _run_armed()
    _hook(TICK, Type.POST_UNCONDITIONAL, "ow_cb_frame", tick)
    done = set()
    for cls_name, names in FUNCS:
        for name in names:
            try:
                paths = _declared_paths(cls_name, name)
            except Exception:
                paths = []
            if not paths:
                CB["missing"].append(f"{cls_name}.{name}")
                continue
            for path in paths:
                if path in done:
                    continue
                done.add(path)
                CB["resolved"].append(path)
                _hook(path, Type.PRE, "ow_cb_pre_" + path, _mk(path, "pre", name))
                _hook(path, Type.POST, "ow_cb_post_" + path, _mk(path, "post", name))
    if behaviors:
        for path, name in _behavior_paths().items():
            if path in done:
                continue
            done.add(path)
            CB["resolved"].append(path)
            _hook(path, Type.PRE, "ow_cb_pre_" + path, _mk(path, "pre", name))
    return {"resolved": len(CB["resolved"]), "missing": CB["missing"], "failed": CB["failed"]}


def cb_stop():
    CB["on"] = False
    CB["sample"] = False
    for path, typ, ident in CB["hooks"]:
        try:
            remove_hook(path, typ, ident)
        except Exception:
            pass
    CB["hooks"] = []


def cb_hooked():
    return {"resolved": len(CB["resolved"]), "missing": CB["missing"], "failed": CB["failed"],
            "silent": [p for p in CB["resolved"] if p not in CB["counts"] and "Behavior_" not in p],
            "called": {p: n for p, n in CB["counts"].items() if "Behavior_" not in p}}


def cb_rows(start=0):
    return CB["rows"][start:]


def cb_dump(name):
    path = RG_OUT + "/" + name
    with open(path, "w", encoding="utf-8") as handle:
        json.dump({"resolved": CB["resolved"], "missing": CB["missing"], "failed": CB["failed"], "counts": CB["counts"],
                   "rows": CB["rows"], "frames": CB["frames"]}, handle, default=str)
    return {"rows": len(CB["rows"]), "frames": len(CB["frames"])}


def _frame_row():
    """One compact row per rendered frame: player health/shield/regen, injured state, cooldown, focus enemy."""
    try:
        pc = get_pc()
        pawn = pc.Pawn
        row = {"t": round(time.perf_counter() - CB["t0"], 4), "g": round(pc.WorldInfo.TimeSeconds, 4), "f": CB["frame"]}
        if pawn is not None:
            row["hp"] = _r(pawn.GetHealth(), 2)
            row["sh"] = _r(pawn.GetShieldStrength(), 2)
            sp = _pool(_g(pawn, "ShieldArmor"))
            if sp is not None:
                row["sh_act"] = _r(_g(sp, "ActiveRegenerationRate", 0.0), 2)
                row["sh_idle"] = _r(_g(sp, "OnIdleRegenerationRate", 0.0), 2)
                row["sh_dly"] = _r(_g(sp, "OnIdleRegenerationDelay", 0.0), 3)
                row["sh_start"] = _r(_g(sp, "PoolIdleDelayStartTime", 0.0), 3)
            row["inj"] = _g(pawn, "InjuredState")
            row["dead"] = _g(pawn, "InjuredDeadState")
            row["bleed"] = _r(_g(pawn, "flInjuredTargetedTime", 0.0), 3)
            row["bleed_total"] = _r(_g(pawn, "TotalBleedoutTime", 0.0), 3)
            row["wdmg"] = _weapon_damage(pawn)
        cd = _pool(_g(pc, "SkillCooldownPool"))
        if cd is not None:
            row["cd"] = _r(_g(cd, "CurrentValue", 0.0), 4)
            row["cd_max"] = _r(_g(cd, "MaxValue", 0.0), 4)
            row["cd_min"] = _r(_g(cd, "MinValue", 0.0), 4)
            row["cd_rate"] = _r(_g(cd, "ConsumptionRate", 0.0), 4)
            row["cd_regen"] = _r(_g(cd, "ActiveRegenerationRate", 0.0), 4)
        if CB["focus"]:
            enemy = _find_ai(CB["focus"])
            if enemy is not None:
                row["e_hp"] = _r(enemy.GetHealth(), 2)
                row["e_sh"] = _r(enemy.GetShieldStrength(), 2)
        CB["frames"].append(row)
    except Exception:
        pass


def _lift_skill():
    """The Phaselock skill instance the player runs: the one whose SkillStartTime moves."""
    skills = [k for k in unrealsdk.find_all("LiftActionSkill") if "Default__" not in k._path_name()]
    return max(skills, key=lambda k: k.SkillStartTime) if skills else None


def _run_armed():
    pc = get_pc()
    for item in CB["armed"]:
        if item["fired"]:
            continue
        due = False
        if item["when"] == "skill_age":
            skill = _lift_skill()
            now = pc.WorldInfo.TimeSeconds
            due = skill is not None and skill.SkillStartTime > 0 and now - skill.SkillStartTime >= item["value"]
        elif item["when"] == "cd_below":
            cd = _pool(_g(pc, "SkillCooldownPool"))
            due = cd is not None and 0 < cd.CurrentValue <= item["value"] and cd.ConsumptionRate > 0
        if due:
            item["fired"] = True
            before = _cooldown(pc)
            item["g"] = round(pc.WorldInfo.TimeSeconds, 4)
            item["f"] = CB["frame"]
            item["t"] = round(time.perf_counter() - CB["t0"], 4)
            item["cd_before"] = before
            pc.ExpLevelUp(False)
            item["cd_after_call"] = _cooldown(pc)
            CB["rows"].append({"armed_fired": dict(item)})


def cb_arm(when, value, label):
    CB["armed"].append({"when": when, "value": value, "label": label, "fired": False})
    return len(CB["armed"])


def cb_disarm():
    CB["armed"] = []


def cb_frames(on=True, focus=None):
    CB["sample"] = bool(on)
    CB["focus"] = focus
    return CB["sample"]


def _find_ai(name):
    for pawn in unrealsdk.find_all("WillowAIPawn"):
        if pawn.Name == name:
            return pawn
    return None


def cb_enemies(radius=4000.0):
    """The live AI pawns (not the preloaded Pawn_<Type> templates) near the player, nearest first."""
    me = get_pc().Pawn.Location
    rows = []
    for pawn in unrealsdk.find_all("WillowAIPawn"):
        if not pawn.Name.startswith("WillowAIPawn_") or _g(pawn, "bDeleteMe", False):
            continue
        loc = pawn.Location
        dist = math.dist((me.X, me.Y, me.Z), (loc.X, loc.Y, loc.Z))
        if dist > radius:
            continue
        row = _pawn_state(pawn)
        row["dist"] = round(dist)
        row["ai_class"] = _p(_g(pawn, "AIClass"))
        row["balance"] = _p(_g(pawn, "BalanceDefinitionState"))
        row["level"] = _g(pawn, "ExpLevel") if _g(pawn, "ExpLevel") is not None else None
        row["pos"] = [round(loc.X), round(loc.Y), round(loc.Z)]
        row["controller"] = _p(_g(pawn, "Controller"))
        rows.append(row)
    return sorted(rows, key=lambda r: r["dist"])


def cb_pickups(radius=3000.0):
    """The pickups on the ground near the player: class, what they hold, rarity, state."""
    me = get_pc().Pawn.Location
    rows = []
    for pk in unrealsdk.find_all("WillowPickup"):
        if "Default__" in pk._path_name() or _g(pk, "bDeleteMe", False):
            continue
        loc = pk.Location
        dist = math.dist((me.X, me.Y, me.Z), (loc.X, loc.Y, loc.Z))
        if dist > radius:
            continue
        inv = _g(pk, "Inventory")
        row = {"name": pk.Name, "class": pk.Class.Name, "dist": round(dist), "pos": [round(loc.X), round(loc.Y), round(loc.Z)],
               "pickupable": _g(pk, "bPickupable"), "life_span": _r(_g(pk, "LifeSpan", 0.0), 2),
               "rarity": _g(pk, "InventoryRarityLevel"), "mission": _g(pk, "bIsMissionItem")}
        if inv is not None:
            row["inv_class"] = inv.Class.Name
            try:
                row["balance"] = _p(inv.DefinitionData.BalanceDefinition)
            except Exception:
                pass
            row["inv_level"] = _g(inv, "ExpLevel")
            row["inv_rarity"] = _g(inv, "RarityLevel")
            row["inv_game_stage"] = _g(inv, "GameStage")
        rows.append(row)
    return sorted(rows, key=lambda r: r["dist"])


def cb_snap(label, radius=4000.0):
    """A labelled state read, memory only; also appended to the rows."""
    row = {"label": label, "t": round(time.perf_counter() - (CB["t0"] or time.perf_counter()), 4), "g": _game_t(), "f": CB["frame"]}
    try:
        row["player"] = _player_state()
    except Exception as e:
        row["player_error"] = repr(e)[:150]
    try:
        row["enemies"] = cb_enemies(radius)[:8]
    except Exception as e:
        row["enemies_error"] = repr(e)[:150]
    try:
        row["pickups"] = cb_pickups(radius)[:12]
    except Exception as e:
        row["pickups_error"] = repr(e)[:150]
    CB["rows"].append({"snap": row})
    return row


def cb_bearing_to_me(name):
    """The bearing (degrees) from an AI pawn to the player, so cb_place can put the player on the side it already is."""
    t, me = _find_ai(name).Location, get_pc().Pawn.Location
    return math.degrees(math.atan2(me.Y - t.Y, me.X - t.X))


def cb_place(name, distance=900.0, bearing_deg=180.0):
    """Put the player `distance` units from an AI pawn (horizontally), `bearing_deg` around it, a little above the floor, looking
    at its chest. The camera is turned with ClientSetRotation (SetRotation kept the pitch at 0 in this build)."""
    target = _find_ai(name)
    pc = get_pc()
    t = target.Location
    a = math.radians(bearing_deg)
    x, y, z = t.X + math.cos(a) * distance, t.Y + math.sin(a) * distance, t.Z + 60.0
    pc.Pawn.Location = unrealsdk.make_struct("Vector", X=x, Y=y, Z=z)
    pc.Pawn.SetPhysics(2)
    cb_aim(name, x, y, z)
    return [round(x), round(y), round(z)]


def cb_aim(name, x=None, y=None, z=None, up=40.0):
    """Point the camera at an AI pawn's chest (the player's own position unless given)."""
    target = _find_ai(name)
    pc = get_pc()
    me = pc.Pawn.Location
    x, y, z = (me.X if x is None else x), (me.Y if y is None else y), (me.Z if z is None else z)
    t = target.Location
    eye = z + _g(pc.Pawn, "BaseEyeHeight", 64.0)
    pitch = math.degrees(math.atan2(t.Z + up - eye, math.hypot(t.X - x, t.Y - y)))
    yaw = math.degrees(math.atan2(t.Y - y, t.X - x))
    pc.ClientSetRotation(unrealsdk.make_struct("Rotator", Pitch=int(pitch * 65536 / 360) % 65536,
                                               Yaw=int(yaw * 65536 / 360) % 65536, Roll=0))
    return [round(pitch, 2), round(yaw, 2)]


def _loot_mods():
    import mods_base.mod_list as ml
    return [m for m in ml.mod_list if _g(m, "name") in LOOT_MODS or getattr(_g(m, "legacy_mod"), "Name", None) in LOOT_MODS]


def cb_mods():
    """Which loot/combat SDK mods are on, and their option values (reads only)."""
    import mods_base.mod_list as ml
    out = []
    for m in ml.mod_list:
        row = {"name": _g(m, "name"), "enabled": _g(m, "is_enabled")}
        if row["name"] in LOOT_MODS:
            legacy = _g(m, "legacy_mod")
            row["options"] = {getattr(o, "Caption", "?"): getattr(o, "CurrentValue", None) for o in (getattr(legacy, "Options", []) or [])}
            row["keybinds"] = [getattr(k, "name", getattr(k, "Name", "?")) for k in (_g(m, "keybinds") or [])]
        out.append(row)
    return out


def cb_mods_off():
    """Switch the loot mods' hooks and keybinds off in memory only. Nothing here calls save_settings: the drop multiplier is a
    'load on main menu' legacy mod, whose disable path saves its settings, so its hooks are removed directly instead."""
    from legacy_compat.ModMenu import HookManager
    done = []
    for m in _loot_mods():
        legacy = _g(m, "legacy_mod")
        try:
            HookManager.RemoveHooks(legacy)
            for kb in (_g(m, "keybinds") or []):
                try:
                    kb.disable()
                except Exception:
                    pass
            m.is_enabled = False
            done.append(_g(m, "name"))
        except Exception as e:
            done.append(f"{_g(m, 'name')}: {repr(e)[:80]}")
    return done


def cb_mods_state():
    return [{"name": _g(m, "name"), "enabled": _g(m, "is_enabled")} for m in _loot_mods()]


def cb_travel_stations(match=""):
    """Travel station definitions known to the game (names only), for choosing a travel target."""
    rows = []
    for cls in ("FastTravelStationDefinition", "LevelTravelStationDefinition", "TravelStationDefinition"):
        try:
            for o in unrealsdk.find_all(cls):
                path = o._path_name()
                if "Default__" in path or match.lower() not in path.lower():
                    continue
                rows.append([cls, path, _p(_g(o, "StationLevelName")), _p(_g(o, "LevelName")), _p(_g(o, "StationDisplayName"))])
        except Exception:
            pass
    return rows
