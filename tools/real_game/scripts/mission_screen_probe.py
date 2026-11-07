"""Mission screen recorder for the openwillow_realgame command channel (mission UI lane, 2026-11-06).

Run through Invoke-GamePyFile after fire_mission_probe.py (it uses fm_stand_in_front). Our own code, no game data; rows go to RG_OUT (the
ignored local/realgame/...). It answers what the host's mission page must do: which keys Marcus's screen takes, what closes it, whether
the world keeps running behind it, what the reward page shows, and what the respawn dialog does.

- ms_start() / ms_stop(): hooks on the screen's script functions and on what it writes to its movie. ms_dump(name) writes the rows.
- ms_read(label): one row with the screen's state: open or not, the movie's text fields, and the player's position, view, clock and money,
  so that two reads show what a key changed. ms_settings(): the movie's and its definition's plain settings.
- The movie is kept as a path and found again for each command, never as an object held across commands: it can be gone by then, and using a dead object
  can take the game down. Both of the screen's close paths clear it.
- ms_mark(name) puts a mark in the rows; the verdicts (ms_reward_shown, ms_reward_taken, ms_respawn) only count what came after their mark, and check the outcome.
- ms_advance(): completes whichever of the mission's objectives is active, through MissionTracker.UpdateObjective; ms_force_ready() is the
  fallback (SetMissionStatus). These are a shortcut to reach the turn-in: the objective path is not what this run records.
- ms_down(): a large hit on the player, repeated on the tick until she is down. Refused while the screen is open.
"""
import json
import time

import unrealsdk
from unrealsdk.hooks import Type, add_hook, remove_hook

MS = globals().get("MS") or {"on": False, "rows": [], "hooks": [], "t0": None, "movie_path": None, "down": None, "missing": [], "hook_errors": 0, "hook_error_samples": []}
MS_MISSION = "GD_Z1_RockPaperGenocide.M_RockPaperGenocide_Fire"
MS_TICK = "WillowGame.WillowGameViewportClient:Tick"
MS_MAX_HITS = 8

# Functions of the screen and of the controller and pawn around it: (class, function, also POST).
MS_FUNCS = [("WillowGame.QuestAcceptGFxMovie", name, post) for name, post in (
    ("Start", True), ("HandleInputKey", True), ("HandleRewardInputKey", True), ("extChoiceConfirmed", False), ("extChoiceCancelled", False),
    ("extAcceptConfirmed", False), ("extCompleteConfirmed", False), ("extGenericButtonClicked", False), ("extOnClickedChoice", False),
    ("GotoCorrectPartOfMovie", False), ("HideTopPanel", False), ("InitForPC", False), ("InitForConsole", False), ("DisplayRewardsPage", False),
    ("ShowNoRewardScreen", False), ("AcceptReward", False), ("UpdateMissionList", False), ("OnClose", True), ("OnClosedOnDeath", True),
    ("SetQuestTitle", False), ("SetPlayerXP", False), ("SetPlayerTotalCredits", False), ("SetRewardsXP", False), ("SetRewardsTotalCredits", False),
    ("SetRewardsItem", False), ("SetLevelAndDifficulty", False), ("SetScrollArrows", False), ("SetFocus", False))] + [
    ("WillowGame.MissionRewardGFxObject", "SetNoReward", False), ("WillowGame.MissionRewardGFxObject", "SetPendingMissionRewardData", False),
    ("WillowGame.MissionRewardGFxObject", "SetUpRewardsPage", False), ("WillowGame.MissionRewardGFxObject", "AcceptReward", False),
    ("WillowGame.MissionRewardGFxObject", "SetTooltips", False), ("WillowGame.MissionRewardGFxObject", "PanelOnInputKey", True),
    ("WillowGame.WillowPlayerController", "AcceptMission", False), ("WillowGame.WillowPlayerController", "ServerCompleteMission", False),
    ("WillowGame.WillowPlayerController", "ClientSpawnMissionRewardUI", False), ("WillowGame.WillowPlayerController", "ClientShowNoRewardScreen", False),
    ("WillowGame.WillowPlayerController", "MissionRewardsReceived", False), ("WillowGame.WillowPlayerController", "AcceptOrSaveUnclaimedReward", False),
    ("WillowGame.WillowPlayerPawn", "ShowRespawnDialog", False), ("WillowGame.WillowPlayerPawn", "ClientShowRespawnDialog", False),
    ("WillowGame.WillowPlayerPawn", "AwaitingRespawnDisplayRespawnCost", False), ("WillowGame.WillowPawn", "InjuredRespawn", False),
    ("WillowGame.WillowPlayerPawn", "ResurrectAtLocation", False), ("WillowGame.WillowPawn", "GoFromInjuredToHealthy", False),
    ("WillowGame.WillowHUD", "ShowRespawnDialog", False)]
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


def _ms_truthy(ret):
    """Start returns false when the screen refuses to open. Anything else counts as open, and the next read's find confirms it."""
    return ret is not False


def _ms_movie():
    """The open screen's movie, found again now; None when it is closed or no longer there (and then it stays closed)."""
    path = MS["movie_path"]
    if not path:
        return None
    try:
        movie = unrealsdk.find_object("QuestAcceptGFxMovie", path)
    except Exception:
        movie = None
    if movie is None:
        MS["movie_path"] = None
    return movie


def _ms_after(name):
    """The rows after the last mark called name, and whether that mark exists."""
    rows = MS["rows"]
    for i in range(len(rows) - 1, -1, -1):
        if rows[i].get("tag") == "mark" and rows[i].get("func") == name:
            return rows[i + 1:], True
    return [], False


def _ms_called(rows, name):
    return [r for r in rows if r.get("tag") == "pre" and r.get("func", "").endswith(":" + name)]


def _ms_reads(rows):
    return [r["read"] for r in rows if "read" in r]


def ms_mark(name):
    MS["rows"].append({"t": _ms_now(), "tag": "mark", "func": name, "args": {}})
    return name


def ms_reward_shown():
    """After the Enter on the turn-in (mark turnin_enter): the mission complete, the page drawn, the screen open with the page's text, the reward not yet taken."""
    rows, marked = _ms_after("turnin_enter")
    reads = _ms_reads(rows)
    last = reads[-1] if reads else {}
    text = last.get("text") or {}
    out = {"marked": marked, "page_called": bool(_ms_called(rows, "DisplayRewardsPage") or _ms_called(rows, "ShowNoRewardScreen")),
           "open": last.get("open") is True, "status": last.get("status"), "header": text.get("reward.rewardsHeader.text", ""),
           "title": text.get("reward.missionTitle.text", ""), "taken_already": bool(_ms_called(rows, "MissionRewardsReceived"))}
    out["ok"] = (marked and out["page_called"] and out["open"] and out["status"] == 4 and bool(out["header"]) and bool(out["title"])
                 and not out["taken_already"])
    return out


def ms_reward_taken():
    """After the page was shown (mark reward_shown): exactly one MissionRewardsReceived, which press led to it (the last reward_press_* mark before it), and a later read
    that no longer shows the reward page."""
    rows, marked = _ms_after("reward_shown")
    press, first, count = None, None, 0
    for i, row in enumerate(rows):
        if row.get("tag") == "mark" and row.get("func", "").startswith("reward_press_") and first is None:
            press = row["func"][len("reward_press_"):]
        if row.get("tag") == "pre" and row.get("func", "").endswith(":MissionRewardsReceived"):
            count += 1
            first = i if first is None else first
    later = _ms_reads(rows[first + 1:]) if first is not None else []
    gone = bool(later) and (later[-1].get("open") is False or not (later[-1].get("text") or {}).get("reward.rewardsHeader.text"))
    out = {"marked": marked, "count": count, "press": press, "read_after": bool(later), "page_gone": gone}
    out["ok"] = marked and count == 1 and press is not None and gone
    return out


def ms_respawn():
    """After the down request (mark down_requested): she went down, was brought back, and only then the dialog came; the money changed by what the dialog was told,
    and a read after that shows her alive. Seeing the dialog function alone proves none of this."""
    rows, marked = _ms_after("down_requested")
    out = {"marked": marked, "down": False, "respawned": False, "dialog": False, "charged": None, "cash_before": MS.get("cash_before"), "cash_after": None, "alive_after": False}
    respawn_at = dialog_at = None
    for i, row in enumerate(rows):
        func = row.get("func", "")
        if row.get("tag") == "mark" and func == "down":
            out["down"] = True
        if row.get("tag") == "pre" and func.endswith(":ResurrectAtLocation") and out["down"] and respawn_at is None:
            respawn_at = i
        if row.get("tag") == "pre" and func.endswith(":ClientShowRespawnDialog") and respawn_at is not None and dialog_at is None:
            dialog_at = i
            out["charged"] = row["args"].get("AmountCharged")
    out["respawned"], out["dialog"] = respawn_at is not None, dialog_at is not None
    reads = _ms_reads(rows[dialog_at + 1:]) if dialog_at is not None else []
    if reads:
        out["cash_after"] = reads[-1].get("cash")
        out["alive_after"] = (reads[-1].get("health") or 0) > 0 and reads[-1].get("injured") is False
    out["ok"] = (marked and out["down"] and out["respawned"] and out["dialog"] and out["alive_after"] and isinstance(out["charged"], int)
                 and isinstance(out["cash_before"], int) and out["cash_after"] == out["cash_before"] - out["charged"])
    return out


def _ms_now():
    return round(time.perf_counter() - MS["t0"], 4)


def _ms_p(value):
    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    try:
        return value._path_name()
    except Exception:
        return str(value)[:160]   # an FName is not a str: its text is what the key rows compare


def _ms_guard(hook):
    """An exception in a hook is counted, never lost: a recording with errors fails its hook_errors stage."""

    def guarded(obj, args, ret, func):
        try:
            hook(obj, args, ret, func)
        except Exception as e:
            MS["hook_errors"] += 1
            if len(MS["hook_error_samples"]) < 5:
                MS["hook_error_samples"].append(repr(e)[:200])

    return guarded


def ms_errors():
    return {"hook_errors": MS["hook_errors"], "samples": MS["hook_error_samples"]}


# What the game's own handler calls each key. The capture's names: Esc, Space.
MS_KEY_NAMES = {"Esc": "Escape", "Space": "SpaceBar"}


def ms_key_seen(mark):
    """Did the screen's own key handler get this key after the mark key_<stage>_<key>? A key that never arrived would read as "this key does nothing"."""
    rows, marked = _ms_after(mark)
    key = mark.rsplit("_", 1)[-1]
    wanted = MS_KEY_NAMES.get(key, key).lower()
    seen = [str(r["args"].get("ukey")) for r in rows if r.get("tag") == "pre" and r.get("func", "").endswith(("HandleInputKey", "HandleRewardInputKey"))]
    return {"marked": marked, "seen": seen[:8], "delivered": any(name.lower() == wanted for name in seen)}


def _ms_args(args, func):
    out = {}
    try:
        for q in func.func._properties():
            if hasattr(args, q.Name):
                out[q.Name] = _ms_p(getattr(args, q.Name))
    except Exception as e:
        out["args_error"] = repr(e)[:100]
    return out


def _ms_named(obj, names):
    """The named plain settings; a name this class lacks reads as absent. Only these are touched: reading every property of a live movie is a risk this run does not need."""
    out = {}
    for name in names:
        try:
            value = getattr(obj, name)
        except Exception:
            continue
        out[name] = value if isinstance(value, (bool, int, float, str)) else str(value)[:80]
    return out


def _ms_mk(path, tag):
    is_start, is_close = path.endswith(":Start"), path.endswith((":OnClose", ":OnClosedOnDeath"))

    def hook(obj, args, ret, func):
        if not MS["on"]:
            return
        row = {"t": _ms_now(), "tag": tag, "func": path.split(".")[-1], "args": _ms_args(args, func)}
        if tag == "post":
            row["ret"] = _ms_p(ret)
        MS["rows"].append(row)
        if tag == "post" and is_start and _ms_truthy(ret):
            MS["movie_path"] = obj._path_name()
        if is_close:   # before the close runs too: nothing may use the movie from here on
            MS["movie_path"] = None

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
    MS.update(rows=[], t0=time.perf_counter(), on=True, movie_path=None, down=None, missing=[], cash_before=None, hook_errors=0, hook_error_samples=[])
    for cls, name, post in MS_FUNCS:
        path = f"{cls}:{name}"
        # add_hook takes any string and such a hook never fires, so the name is looked up first.
        try:
            unrealsdk.find_object("Function", path)
        except Exception as e:
            MS["missing"].append(f"{path}: {e!r}"[:160])
            continue
        for typ, tag in [(Type.PRE, "pre")] + ([(Type.POST, "post")] if post else []):
            ident = f"ow_ms_{tag}_{path}"
            add_hook(path, typ, ident, _ms_guard(_ms_mk(path, tag)))
            MS["hooks"].append((path, typ, ident))
    for name in MS_WRITES:
        path = f"GFxUI.GFxMoviePlayer:{name}"
        add_hook(path, Type.PRE, f"ow_ms_write_{name}", _ms_guard(_ms_write_hook(name)))
        MS["hooks"].append((path, Type.PRE, f"ow_ms_write_{name}"))
    add_hook(MS_TICK, Type.PRE, "ow_ms_tick", _ms_guard(_ms_tick))
    MS["hooks"].append((MS_TICK, Type.PRE, "ow_ms_tick"))
    return {"hooks": len(MS["hooks"]), "missing": MS["missing"]}


def ms_read(label):
    pc = get_pc()
    world = pc.WorldInfo
    movie = _ms_movie()
    row = {"label": label, "t": _ms_now(), "open": movie is not None}
    pawn = pc.Pawn
    # One field that this build answers differently must not cost the read: its error goes in the row.
    for key, fn in (("game_time", lambda: world.TimeSeconds), ("real_time", lambda: world.RealTimeSeconds), ("paused", lambda: _ms_p(world.Pauser)),
                    ("view", lambda: [pc.Rotation.Pitch, pc.Rotation.Yaw]), ("pos", lambda: [round(pawn.Location.X, 1), round(pawn.Location.Y, 1), round(pawn.Location.Z, 1)]),
                    ("health", lambda: pawn.GetHealth()), ("state", lambda: str(pawn.GetStateName())), ("injured", lambda: bool(pawn.bIsInjured)),
                    ("ignore_move", lambda: pc.IsMoveInputIgnored()), ("ignore_look", lambda: pc.IsLookInputIgnored()), ("exp", lambda: pc.GetExpPoints()),
                    ("cash", lambda: pc.PlayerReplicationInfo.GetCurrencyOnHand(0)), ("movie_playing", lambda: pc.IsMissionMoviePlaying()),
                    ("status", lambda: int(world.GRI.MissionTracker.GetMissionStatus(unrealsdk.find_object("MissionDefinition", MS_MISSION))))):
        try:
            row[key] = fn()
        except Exception as e:
            row[key + "_error"] = repr(e)[:100]
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
    # Written at every read, so a stage that throws does not take the rows with it.
    try:
        ms_dump("screen")
    except Exception as e:
        row["dump_error"] = repr(e)[:100]
    return row


def ms_settings():
    movie = _ms_movie()
    if movie is None:
        return {"error": "the screen is not open"}
    return {"movie": _ms_named(movie, ("bPauseGameWhileActive", "bCaptureInput", "bIgnoreMouseInput", "bBlurLesserMovies", "bDisplayWithHudOff", "bCloseOnLevelChange", "TickRateSeconds")),
            "definition": _ms_named(movie.MissionUIDef, ("bUsePPOverride",)) if movie.MissionUIDef else None}


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
    read = ms_read("before_down")
    if read["open"] or read.get("movie_playing") is True:
        return {"refused": "the screen is still open", "read": read}
    MS["cash_before"] = read.get("cash")
    ms_mark("down_requested")
    MS["down"] = {"hits": 0, "next": 0.0, "done": False, "injured": False}
    return {"refused": None, "read": read}


def ms_dump(name):
    with open(f"{RG_OUT}/{name}.json", "w", encoding="utf-8") as f:
        json.dump({"rows": MS["rows"], "missing": MS["missing"], "errors": ms_errors()}, f, indent=1, default=str)
    return len(MS["rows"])


print("mission_screen_probe loaded")
