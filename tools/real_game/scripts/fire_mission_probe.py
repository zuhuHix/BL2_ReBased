"""Fire-mission golden-trace recorder for the openwillow_realgame command channel (lane L2, 2026-10-07).

Run once through Invoke-GamePyFile (tools/real_game/realgame.ps1); our own code, no game data. Rows go to RG_OUT
(the ignored local/realgame/...). Memory-only: the only writes to the game are the documented script calls the
caller makes by hand (standing the player in front of a pawn, a teleport, an AcceptMission call if the key path fails).

- fm_start(): hooks (PRE, and POST for the natives that matter) on the mission tracker, the player controller's
  mission functions, the mission definition's reward natives, the pawn / interactive object mission callbacks, the dialog
  component and manager, the Kismet operations of the mission, the waypoint, the inventory manager and the script
  behaviors (every `Behavior_*` class's ApplyBehaviorToContext). One row per call: seconds on the QPC clock since the
  start, the game's own clock (WorldInfo.TimeSeconds), a frame counter, the function, the object and the arguments.
- fm_rows() / fm_dump(name): the rows; fm_hooked(): which functions resolved and which did not.
- fm_snap(label): mission status, objective progress, active set, tracked mission, XP, level, credits and the inventory
  chain of the player, as one row (state before and after each step).
- fm_stand_in_front(path, distance): put the player in front of a pawn (memory only), looking at it.

Typical flow (docs/verification/FIRE_MISSION_GOLDEN_TRACE.md "How it was driven"): load the probe, call fm_start(), press the
real keys with realgame.ps1 (E at Marcus, Enter on the screens, the mouse button to shoot), call fm_snap(label) between steps,
then fm_dump(name) and fm_stop(). The recorder state FM is kept in the shared namespace on purpose: loading this file again
keeps the rows and the hook list (an earlier version reset them, which silently turned a recording off).
"""
import json
import math
import time

import unrealsdk
from unrealsdk.hooks import Type, add_hook, remove_hook
from unrealsdk.unreal import UObject, WrappedArray, WrappedStruct

TICK = "WillowGame.WillowGameViewportClient:Tick"
MISSION = "GD_Z1_RockPaperGenocide.M_RockPaperGenocide_Fire"
MARCUS_PATH = "Sanctuary_Dynamic.TheWorld:PersistentLevel.WillowAIPawn_13"
FM = globals().get("FM") or {"on": False, "rows": [], "hooks": [], "t0": None, "frame": 0, "resolved": [], "missing": [], "counts": {}}

# (class, function, also POST). Natives called from C++ are not seen by a Python hook; what script calls is.
FUNCS = [
    ("MissionTracker", "ActivateMission", 1), ("MissionTracker", "CompleteMission", 1), ("MissionTracker", "SetMissionStatus", 1),
    ("MissionTracker", "UpdateObjective", 1), ("MissionTracker", "DecrementObjective", 1), ("MissionTracker", "PlayKickoff", 1),
    ("MissionTracker", "PlayKickoffDialogOnly", 1), ("MissionTracker", "PlayTurnIn", 1), ("MissionTracker", "SetKickoffHeard", 1),
    ("MissionTracker", "SetActiveMission", 1), ("MissionTracker", "RegisterWaypoint", 0), ("MissionTracker", "UnregisterWaypoint", 0),
    ("MissionTracker", "RegisterMissionObserver", 0), ("MissionTracker", "TriggerMissionStatusChangedDelegates", 0),
    ("MissionTracker", "TriggerMissionObjectivesChangedDelegates", 0), ("MissionTracker", "TriggerActiveMissionChangedDelegates", 0),
    ("MissionTracker", "GrantMissionWeaponsToClientPlayer", 1), ("MissionTracker", "RemoteUpdateMissionStatus", 0),
    ("MissionTracker", "RemoteUpdateMissionObjective", 0), ("MissionTracker", "RemoteUpdateActiveObjectiveSet", 0),
    ("MissionTracker", "RemoteSubObjectiveSetAdvanced", 0), ("MissionTracker", "NotifyLocalPlayerOfActiveMission", 0),
    ("MissionTracker", "RunMissionCustomEvent", 0), ("MissionTracker", "IsMissionObjectiveActive", 1),
    ("MissionTracker", "CanStartMission", 1), ("MissionTracker", "CanEndMission", 1), ("MissionTracker", "MissionDependenciesMet", 1),
    ("WillowPlayerController", "AcceptMission", 0), ("WillowPlayerController", "ServerAcceptMission", 0),
    ("WillowPlayerController", "ServerCompleteMission", 0), ("WillowPlayerController", "UpdateMissionStatus", 0),
    ("WillowPlayerController", "UpdateMissionObjective", 0), ("WillowPlayerController", "ServerGrantMissionRewards", 0),
    ("WillowPlayerController", "ExpEarn", 1), ("WillowPlayerController", "ExpLevelUp", 0),
    ("WillowPlayerController", "ShowMissionWeaponTraining", 0), ("WillowPlayerController", "IsMissionMoviePlaying", 1),
    ("WillowPlayerController", "ClientShowNoRewardScreen", 0), ("WillowPlayerController", "ClientSpawnMissionRewardUI", 0),
    ("WillowPlayerController", "AcceptOrSaveUnclaimedReward", 0), ("WillowPlayerController", "MissionRewardsReceived", 0),
    ("WillowPlayerController", "GetNumRewardChoices", 1), ("WillowPlayerController", "ReceiveWeaponReward", 0),
    ("WillowPlayerController", "ReceiveItemReward", 0), ("WillowPlayerController", "ClientReceiveMissionStatus", 0),
    ("WillowPlayerController", "IsFastForwardPromptValid", 1), ("WillowPlayerController", "TryPromptForFastForward", 0),
    ("WillowPlayerController", "ShowMissionInterface", 0), ("WillowPlayerController", "AddMission", 0),
    ("MissionDefinition", "GetExperienceReward", 1), ("MissionDefinition", "GetCurrencyReward", 1),
    ("MissionDefinition", "GetCurrencyRewardType", 1), ("MissionDefinition", "GetItemRewardsForPlayer", 1),
    ("MissionDefinition", "ShouldGrantAlternateReward", 1), ("MissionDefinition", "GetOptionalCreditReward", 1),
    ("WillowAIPawn", "OnPlayerAcceptedMission", 0), ("WillowAIPawn", "OnPlayerTurnedInMission", 0), ("WillowAIPawn", "MissionStatusChanged", 0),
    ("WillowAIPawn", "PlayOnUseDialog", 0), ("WillowAIPawn", "PlayMissionTurnedInDialog", 0), ("WillowAIPawn", "PlayDismissalDialog", 0),
    ("WillowAIPawn", "PlayLingeringDialog", 0), ("WillowAIPawn", "OnPlayerOpenedMissionUI", 0), ("WillowAIPawn", "OnPlayerClosedMissionUI", 0),
    ("WillowAIPawn", "FireOnUsedBehaviors", 0), ("WillowAIPawn", "UseObject", 0), ("WillowAIPawn", "BeginUse", 0), ("WillowAIPawn", "EndUse", 0),
    ("WillowInteractiveObject", "OnPlayerAcceptedMission", 0), ("WillowInteractiveObject", "OnPlayerTurnedInMission", 0),
    ("WillowPickup", "OnPlayerAcceptedMission", 0), ("WillowPickup", "OnPlayerTurnedInMission", 0),
    ("AIClassDefinition", "OnUsed", 0),
    ("GearboxDialogComponent", "TriggerEvent", 1), ("GearboxDialogComponent", "Talk", 1), ("GearboxDialogComponent", "TalkReplicated", 0),
    ("GearboxDialogComponent", "StopTalking", 0), ("GearboxDialogManager", "TriggerGroupEvent", 1), ("WillowDialogManager", "PlayEchoDialog", 0),
    ("WillowDialogManager", "PlayPersonalEchoLog", 0), ("WillowDialogManager", "IsMissionKickoffPlaying", 1),
    ("BehaviorKernel", "ActivateBehaviorEventFromScript", 0), ("BehaviorKernel", "IsBehaviorSequenceEnabled", 1),
    ("WillowWaypoint", "Touch", 0), ("WillowWaypoint", "ProcessPlayerTouch", 0), ("WillowWaypoint", "MissionReactionStatusChanged", 0),
    ("WillowWaypoint", "MissionReactionObjectiveSetChanged", 0), ("WillowWaypoint", "MissionReactionObjectiveUpdated", 0),
    ("WillowWaypoint", "MissionReactionObjectiveComplete", 0), ("WillowWaypoint", "MissionReactionLevelLoad", 0),
    ("WillowInventoryManager", "AddInventory", 0), ("WillowInventoryManager", "RemoveFromInventory", 0),
    ("WillowInventoryManager", "RemoveMissionWeapons", 0), ("WillowInventoryManager", "ClientRemoveMissionWeapons", 0),
    ("WillowInventoryManager", "AddInventoryToBackpack", 0), ("WillowInventoryManager", "RemoveInventoryFromBackpack", 0),
]
# Kismet operations the mission reaches (Activated), from the kismet census; recorded in full, with the event name.
KISMET = ["SeqEvent_RemoteEvent", "WillowSeqEvent_MissionRemoteEvent", "SeqAct_ActivateRemoteEvent", "SeqAct_Interp", "GearboxSeqAct_TriggerDialogName",
          "WillowSeqAct_AIScripted", "SeqAct_Toggle", "SeqAct_Destroy", "GearboxSeqAct_ResetPopulationCount", "SeqEvent_PopulatedActor",
          "SeqEvent_PopulatedPoint", "SeqEvent_ArrivedAtMoveNode", "SeqAct_ApplyBehavior", "SeqAct_AttachToActor", "SeqAct_SetPhysics",
          "SeqEvent_Used", "SeqEvent_Touch", "SeqEvent_TakeDamage", "SeqEvent_Death", "SeqAct_ChangeBool", "SeqAct_Gate"]


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


def _mk(path, tag, full, extra):
    def hook(obj, args, ret, func):
        if not FM["on"]:
            return
        FM["counts"][path] = FM["counts"].get(path, 0) + 1
        row = {"t": round(time.perf_counter() - FM["t0"], 4), "g": _game_t(), "f": FM["frame"], "tag": tag, "func": path, "obj": _p(obj)}
        if full:
            row["args"] = _args(args, func)
            if tag == "post":
                try:
                    row["ret"] = _p(ret)
                except Exception:
                    row["ret"] = "?"
        for name in extra:
            try:
                row[name] = _p(getattr(obj, name))
            except Exception:
                pass
        FM["rows"].append(row)

    return hook


def _hook(path, typ, ident, fn):
    add_hook(path, typ, ident, fn)
    FM["hooks"].append((path, typ, ident))


def _func_path(cls_name, func_name):
    cls = unrealsdk.find_class(cls_name)
    return cls._find(func_name)._path_name()


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


def fm_start(behaviors=True):
    fm_stop()
    FM.update(rows=[], t0=time.perf_counter(), frame=0, resolved=[], missing=[], counts={}, on=True)

    def tick(*_a):
        FM["frame"] += 1
    _hook(TICK, Type.POST_UNCONDITIONAL, "ow_fm_frame", tick)
    done = set()
    for cls, fn, post in FUNCS:
        try:
            path = _func_path(cls, fn)
        except Exception:
            FM["missing"].append(f"{cls}.{fn}")
            continue
        if path in done:
            continue
        done.add(path)
        FM["resolved"].append(path)
        _hook(path, Type.PRE, "ow_fm_pre_" + path, _mk(path, "pre", True, ()))
        if post:
            _hook(path, Type.POST, "ow_fm_post_" + path, _mk(path, "post", True, ()))
    for cls in KISMET:
        try:
            path = _func_path(cls, "Activated")
        except Exception:
            FM["missing"].append(f"{cls}.Activated")
            continue
        if path in done:
            continue
        done.add(path)
        FM["resolved"].append(path)
        _hook(path, Type.PRE, "ow_fm_pre_" + path, _mk(path, "pre", False, ("EventName", "Mission")))
    if behaviors:
        for path, name in _behavior_paths().items():
            if path in done:
                continue
            done.add(path)
            FM["resolved"].append(path)
            _hook(path, Type.PRE, "ow_fm_pre_" + path, _mk(path, "pre", True, ()))
    return {"resolved": len(FM["resolved"]), "missing": FM["missing"]}


def fm_stop():
    FM["on"] = False
    for path, typ, ident in FM["hooks"]:
        try:
            remove_hook(path, typ, ident)
        except Exception:
            pass
    FM["hooks"] = []


def fm_rows(start=0):
    return FM["rows"][start:]


def fm_dump(name):
    path = RG_OUT + "/" + name
    with open(path, "w", encoding="utf-8") as handle:
        json.dump({"resolved": FM["resolved"], "missing": FM["missing"], "counts": FM["counts"], "rows": FM["rows"]}, handle)
    return len(FM["rows"])


def _chain(pawn):
    out = []
    try:
        item = pawn.InvManager.InventoryChain
        guard = 0
        while item is not None and guard < 200:
            guard += 1
            row = {"class": item.Class.Name, "path": _p(item)}
            for name in ("bMissionWeapon", "MissionWeaponOwner"):
                try:
                    row[name] = _p(getattr(item, name))
                except Exception:
                    pass
            try:
                row["balance"] = _p(item.DefinitionData.BalanceDefinition)
            except Exception:
                pass
            out.append(row)
            item = item.Inventory
    except Exception as e:
        out.append({"error": repr(e)[:100]})
    try:   # the backpack holds the lent mission weapon in this build
        for item in list(pawn.InvManager.Backpack):
            if item is None:
                continue
            row = {"class": item.Class.Name, "backpack": True}
            try:
                row["balance"] = _p(item.DefinitionData.BalanceDefinition)
            except Exception:
                pass
            out.append(row)
    except Exception as e:
        out.append({"error": repr(e)[:100]})
    return out


def fm_snap(label):
    pc = get_pc()
    row = {"label": label, "t": round(time.perf_counter() - (FM["t0"] or time.perf_counter()), 4), "g": _game_t(), "f": FM["frame"]}
    try:
        tracker = pc.WorldInfo.GRI.MissionTracker
        mission = unrealsdk.find_object("MissionDefinition", MISSION)
        row["status"] = int(tracker.GetMissionStatus(mission))
        try:
            res = tracker.GetObjectivesProgress(mission, [], False)
            row["progress"] = _p(res)
        except Exception as e:
            row["progress_error"] = repr(e)[:100]
        try:
            row["active_set"] = _p(tracker.GetActivePrimaryObjectiveSet(mission))
        except Exception as e:
            row["active_set_error"] = repr(e)[:100]
        try:
            row["tracked"] = _p(tracker.GetActiveMission())
        except Exception as e:
            row["tracked_error"] = repr(e)[:100]
        try:
            row["controller_status"] = None
        except Exception:
            pass
    except Exception as e:
        row["tracker_error"] = repr(e)[:200]
    pawn = pc.Pawn
    for key, fn in (("exp", lambda: pc.GetExpPoints()), ("level", lambda: pawn.GetExpLevel()), ("hp", lambda: pawn.GetHealth()),
                    ("pos", lambda: [round(pawn.Location.X, 1), round(pawn.Location.Y, 1), round(pawn.Location.Z, 1)])):
        try:
            row[key] = fn()
        except Exception:
            pass
    try:
        row["currency"] = _p(pc.GetCurrencyOnHand(0))
    except Exception:
        pass
    row["inventory"] = _chain(pawn)
    FM["rows"].append({"snap": row})
    return row


def fm_stand_in_front(path=MARCUS_PATH, distance=110.0, aim_up=20.0):
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
    pitch = math.degrees(math.atan2(target.Location.Z + aim_up - eye, math.hypot(dx, dy)))   # look at the target's chest, not past it
    yaw = math.degrees(math.atan2(dy, dx))
    # ClientSetRotation: SetRotation kept the pitch at 0 in this build; the unsigned 16-bit form is what the game stores.
    me.ClientSetRotation(unrealsdk.make_struct("Rotator", Pitch=int(pitch * 65536 / 360) % 65536, Yaw=int(yaw * 65536 / 360) % 65536, Roll=0))
    return [round(x, 1), round(y, 1), round(z, 1)]
