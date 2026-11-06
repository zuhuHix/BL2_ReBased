"""Mission screen recorder for the openwillow_realgame command channel (mission UI lane, 2026-11-06).

Run through Invoke-GamePyFile after fire_mission_probe.py (it uses fm_stand_in_front). Our own code, no game data; rows go to RG_OUT (the
ignored local/realgame/...). It answers what the host's mission page must do: which keys Marcus's screen takes, what closes it, whether
the world keeps running behind it, what the reward page shows, and what the respawn dialog does.

- ms_start() / ms_stop(): hooks on the screen's script functions and on what it writes to its movie. ms_dump(name) writes the rows.
- ms_read(label): one row with the screen's state: open or not, the movie's text fields, and the player's position, view, clock and money,
  so that two reads show what a key changed. ms_settings(): the movie's and its definition's plain settings.
- ms_advance(): completes whichever of the mission's objectives is active, through MissionTracker.UpdateObjective; ms_force_ready() is the
  fallback (SetMissionStatus). These are a shortcut to reach the turn-in: the objective path is not what this run records.
- ms_down(): a large hit on the player, repeated on the tick until she is down.
"""
import json
import time

import unrealsdk
from unrealsdk.hooks import Type, add_hook, remove_hook

MS = globals().get("MS") or {"on": False, "rows": [], "hooks": [], "t0": None, "movie": None, "down": None, "missing": []}
MS_MISSION = "GD_Z1_RockPaperGenocide.M_RockPaperGenocide_Fire"
MS_TICK = "WillowGame.WillowGameViewportClient:Tick"
MS_MAX_HITS = 8

# Functions of the screen and of the controller and pawn around it: (class, function, also POST).
MS_FUNCS = [("WillowGame.QuestAcceptGFxMovie", name, post) for name, post in (
    ("Start", True), ("HandleInputKey", True), ("HandleRewardInputKey", True), ("extChoiceConfirmed", False), ("extChoiceCancelled", False),
    ("extAcceptConfirmed", False), ("extCompleteConfirmed", False), ("extGenericButtonClicked", False), ("extOnClickedChoice", False),
    ("GotoCorrectPartOfMovie", False), ("HideTopPanel", False), ("InitForPC", False), ("InitForConsole", False), ("DisplayRewardsPage", False),
    ("ShowNoRewardScreen", False), ("AcceptReward", False), ("UpdateMissionList", False), ("OnClose", True), ("OnClosedOnDeath", False),
    ("SetQuestTitle", False), ("SetPlayerXP", False), ("SetPlayerTotalCredits", False), ("SetRewardsXP", False), ("SetRewardsTotalCredits", False),
    ("SetRewardsItem", False), ("SetLevelAndDifficulty", False), ("SetScrollArrows", False), ("SetFocus", False))] + [
    ("WillowGame.MissionRewardGFxObject", "SetNoReward", False), ("WillowGame.MissionRewardGFxObject", "SetPendingMissionRewardData", False),
    ("WillowGame.MissionRewardGFxObject", "SetUpRewardsPage", False), ("WillowGame.MissionRewardGFxObject", "AcceptReward", False),
    ("WillowGame.MissionRewardGFxObject", "SetTooltips", False), ("WillowGame.MissionRewardGFxObject", "PanelOnInputKey", True),
    ("WillowGame.WillowPlayerController", "AcceptMission", False), ("WillowGame.WillowPlayerController", "ServerCompleteMission", False),
    ("WillowGame.WillowPlayerController", "ClientSpawnMissionRewardUI", False), ("WillowGame.WillowPlayerController", "ClientShowNoRewardScreen", False),
    ("WillowGame.WillowPlayerController", "MissionRewardsReceived", False), ("WillowGame.WillowPlayerController", "AcceptOrSaveUnclaimedReward", False),
    ("WillowGame.WillowPlayerPawn", "ShowRespawnDialog", False), ("WillowGame.WillowPlayerPawn", "ClientShowRespawnDialog", False),
    ("WillowGame.WillowPlayerPawn", "AwaitingRespawnDisplayRespawnCost", False), ("WillowGame.WillowPlayerPawn", "InjuredRespawn", False),
    ("WillowGame.WillowPlayerPawn", "ResurrectAtLocation", False), ("WillowGame.WillowPlayerPawn", "GoFromInjuredToHealthy", False)]
# What the screen writes to its own movie (the host page has to write the same paths). Filtered to this movie: the HUD uses them too.
MS_WRITES = ["SetVariableString", "SetVariableBool", "SetVariableNumber"]
# Text fields of the two pages, read back while the screen is open. A path the current frame does not have reads empty.
MS_TEXT = ["missions.header.description.text", "missions.tooltips.text", "missions.tooltips.htmlText", "missions.description.htmlText",
           "missions.OptionalRequired.textField.htmlText", "reward.rewardsHeader.text", "reward.missionTitle.text", "reward.description.text",
           "reward.rewards.text", "reward.norewardtext.text", "reward.expHeader.htmlText", "reward.cashHeader.htmlText",
           "reward.gearHeader.htmlText", "reward.xpReward.text", "reward.tooltips.htmlText", "reward.tooltips.text"]


def ms_pick_text(read_text):
    """The fields that have a value; read_text(path) gives a field's text and may raise. Tested without the game."""
    out = {}
    for path in MS_TEXT:
        try:
            value = read_text(path)
        except Exception as e:
            out[path] = "error " + repr(e)[:60]
            continue
        if value:
            out[path] = value
    return out


def ms_turnin_ready(status):
    """The tracker's status for ready to turn in, as read in the golden trace (3)."""
    return status == 3


def _ms_now():
    return round(time.perf_counter() - MS["t0"], 4)


def _ms_p(value):
    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    try:
        return value._path_name()
    except Exception:
        return repr(value)[:160]


def _ms_args(args, func):
    out = {}
    try:
        for q in func.func._properties():
            if hasattr(args, q.Name):
                out[q.Name] = _ms_p(getattr(args, q.Name))
    except Exception as e:
        out["args_error"] = repr(e)[:100]
    return out


def _ms_scalars(obj):
    """The object's plain settings (flags, numbers, names), for the record of how the screen is set up."""
    out = {}
    try:
        for q in obj.Class._properties():
            try:
                value = getattr(obj, q.Name)
            except Exception:
                continue
            if isinstance(value, (bool, int, float, str)):
                out[q.Name] = value
    except Exception as e:
        out["error"] = repr(e)[:100]
    return out


def _ms_mk(path, tag):
    is_start, is_close = path.endswith(":Start"), path.endswith(":OnClose")

    def hook(obj, args, ret, func):
        if not MS["on"]:
            return
        row = {"t": _ms_now(), "tag": tag, "func": path.split(".")[-1], "args": _ms_args(args, func)}
        if tag == "post":
            row["ret"] = _ms_p(ret)
        MS["rows"].append(row)
        if tag == "post" and is_start:
            MS["movie"] = obj
        if tag == "post" and is_close:
            MS["movie"] = None

    return hook


def _ms_write_hook(name):
    def hook(obj, args, ret, func):
        if MS["on"] and obj.Class.Name == "QuestAcceptGFxMovie":
            MS["rows"].append({"t": _ms_now(), "tag": "write", "func": name, "args": _ms_args(args, func)})

    return hook


def _ms_tick(obj, args, ret, func):
    job = MS["down"]
    if not job or job["done"]:
        return
    pc = get_pc()
    pawn = pc.Pawn
    if pawn is None:
        return
    if bool(pawn.bIsInjured):
        job["done"] = job["injured"] = True
        MS["rows"].append({"t": _ms_now(), "tag": "mark", "func": "down", "args": {"hits": job["hits"]}})
        return
    now = pc.WorldInfo.TimeSeconds
    if now < job["next"]:
        return
    if job["hits"] >= MS_MAX_HITS:
        job["done"] = True
        MS["rows"].append({"t": _ms_now(), "tag": "mark", "func": "down_gave_up", "args": {"hits": job["hits"]}})
        return
    job["next"] = now + 0.5
    job["hits"] += 1
    zero = unrealsdk.make_struct("Vector", X=0.0, Y=0.0, Z=0.0)
    pawn.TakeDamage(1000000, pc, pawn.Location, zero, unrealsdk.find_class("DamageType"))


def ms_stop():
    MS["on"] = False
    for path, typ, ident in MS["hooks"]:
        try:
            remove_hook(path, typ, ident)
        except Exception:
            pass
    MS["hooks"] = []
    return "stopped"


def ms_start():
    ms_stop()
    MS.update(rows=[], t0=time.perf_counter(), on=True, movie=None, down=None, missing=[])
    for cls, name, post in MS_FUNCS:
        path = f"{cls}:{name}"
        for typ, tag in [(Type.PRE, "pre")] + ([(Type.POST, "post")] if post else []):
            ident = f"ow_ms_{tag}_{path}"
            try:
                add_hook(path, typ, ident, _ms_mk(path, tag))
                MS["hooks"].append((path, typ, ident))
            except Exception as e:
                MS["missing"].append(f"{path}: {e!r}"[:160])
    for name in MS_WRITES:
        path = f"GFxUI.GFxMoviePlayer:{name}"
        add_hook(path, Type.PRE, f"ow_ms_write_{name}", _ms_write_hook(name))
        MS["hooks"].append((path, Type.PRE, f"ow_ms_write_{name}"))
    add_hook(MS_TICK, Type.PRE, "ow_ms_tick", _ms_tick)
    MS["hooks"].append((MS_TICK, Type.PRE, "ow_ms_tick"))
    return {"hooks": len(MS["hooks"]), "missing": MS["missing"]}


def ms_read(label):
    pc = get_pc()
    world = pc.WorldInfo
    row = {"label": label, "t": _ms_now(), "open": MS["movie"] is not None, "game_time": world.TimeSeconds, "real_time": world.RealTimeSeconds,
           "paused": _ms_p(world.Pauser), "view": [pc.Rotation.Pitch, pc.Rotation.Yaw]}
    pawn = pc.Pawn
    if pawn is not None:
        row["pos"] = [round(pawn.Location.X, 1), round(pawn.Location.Y, 1), round(pawn.Location.Z, 1)]
        row["health"] = pawn.GetHealth()
        row["state"] = str(pawn.GetStateName())
        row["injured"] = bool(pawn.bIsInjured)
    for key, fn in (("ignore_move", lambda: pc.IsMoveInputIgnored()), ("ignore_look", lambda: pc.IsLookInputIgnored()), ("exp", lambda: pc.GetExpPoints()),
                    ("cash", lambda: pc.PlayerReplicationInfo.GetCurrencyOnHand(0)), ("movie_playing", lambda: pc.IsMissionMoviePlaying()),
                    ("status", lambda: int(world.GRI.MissionTracker.GetMissionStatus(unrealsdk.find_object("MissionDefinition", MS_MISSION))))):
        try:
            row[key] = fn()
        except Exception as e:
            row[key + "_error"] = repr(e)[:100]
    movie = MS["movie"]
    if movie is not None:
        row["text"] = ms_pick_text(movie.GetVariableString)
        for key, fn in (("entries", lambda: len(movie.MissionList)), ("single", lambda: movie.bSingleMissionDisplay), ("redeemable", lambda: movie.RedeemableCount),
                        ("eligible", lambda: movie.EligibleCount), ("list", lambda: [_ms_p(m.MissionDef) for m in movie.MissionList]),
                        ("reward_object", lambda: _ms_p(movie.RewardObject))):
            try:
                row[key] = fn()
            except Exception as e:
                row[key + "_error"] = repr(e)[:100]
    MS["rows"].append({"read": row})
    return row


def ms_settings():
    movie = MS["movie"]
    if movie is None:
        return {"error": "the screen is not open"}
    return {"movie": _ms_scalars(movie), "definition": _ms_scalars(movie.MissionUIDef) if movie.MissionUIDef else None}


def _ms_objectives():
    return [o for o in unrealsdk.find_all("MissionObjectiveDefinition", False) if MS_MISSION in o._path_name()]


def ms_advance():
    tracker = get_pc().WorldInfo.GRI.MissionTracker
    updated = []
    for objective in _ms_objectives():
        if tracker.IsMissionObjectiveActive(objective) and not tracker.IsMissionObjectiveComplete(objective):
            tracker.UpdateObjective(objective, 0)
            updated.append(objective.Name)
    status = int(tracker.GetMissionStatus(unrealsdk.find_object("MissionDefinition", MS_MISSION)))
    MS["rows"].append({"t": _ms_now(), "tag": "mark", "func": "advance", "args": {"updated": updated, "status": status}})
    return {"updated": updated, "status": status, "ready": ms_turnin_ready(status)}


def ms_force_ready():
    pc = get_pc()
    tracker = pc.WorldInfo.GRI.MissionTracker
    mission = unrealsdk.find_object("MissionDefinition", MS_MISSION)
    tracker.SetMissionStatus(mission, 3, pc)
    status = int(tracker.GetMissionStatus(mission))
    MS["rows"].append({"t": _ms_now(), "tag": "mark", "func": "forced_ready", "args": {"status": status}})
    return {"status": status, "ready": ms_turnin_ready(status)}


def ms_down():
    MS["down"] = {"hits": 0, "next": 0.0, "done": False, "injured": False}
    return ms_read("down_requested")


def ms_dump(name):
    with open(f"{RG_OUT}/{name}.json", "w", encoding="utf-8") as f:
        json.dump({"rows": MS["rows"], "missing": MS["missing"]}, f, indent=1, default=str)
    return len(MS["rows"])


print("mission_screen_probe loaded")
