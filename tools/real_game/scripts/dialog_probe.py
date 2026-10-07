"""Dialog-line probe for the openwillow_realgame command channel (lane L4, 2026-10-07).

Run through Invoke-GamePyFile (tools/real_game/realgame.ps1) AFTER tools/real_game/scripts/fire_mission_probe.py has been loaded
in the same game session: it adds rows to that recorder (FM["rows"]) and reuses its helpers, so the dialog rows share one timeline
(QPC clock, game clock, frame counter) with the mission rows. Our own code, no game data; memory-only.

Question it serves (docs/verification/FIRE_MISSION_GOLDEN_TRACE.md, "Dialog lines without the dialog skipper"): why did the live
Fire run never apply `Behavior_TriggerDialogEvent_1184` / `_1183` (the lines 03a and 04 behind `_1186`'s Finished output)? The
installed Dialog Skipper SDK mod hooks the dialog trigger and the talk start; this probe switches that mod off in memory and
watches the dialog state every frame.

- dp_mod_off(): disable the installed "Dialog Skipper" mod in memory (`Mod.disable(dont_update_setting=True)`, which does not write
  its settings file). dp_mod_hooks_on() puts only its hooks back (also no settings write) for a control; dp_mod_state() reads it.
- dp_start(): hooks on script-visible dialog functions (`Behavior_TriggerDialogEvent.TriggerDialogEvent` pre/post,
  `GearboxDialogEventData.IsActive` post, `TalkStarted` / `TalkFinished` of the talk acts) and a per-frame poll that records changes
  of the dialog behaviors' `MyEventData` (live talk act, use count, IsActive), of the dialog components' `IsTalking` and of the
  manager's echo actor. Rows are appended only when something changed.
- dp_snap(label): one row with the same state read once; dp_stop() removes the hooks.
Hooks see only calls that go through script; the poll reads state, so a line that is native-only is still seen by its effects.
"""
import time

import unrealsdk
from unrealsdk.hooks import Type, add_hook, remove_hook

if "FM" not in globals():
    raise RuntimeError("load fire_mission_probe.py first: dialog_probe.py reuses its recorder")

DP = globals().get("DP") or {"on": False, "hooks": [], "last": {}, "comps": {}, "frame_comp": 0, "quiet": False, "errors": 0, "watch": []}
BEHAVIOR_PREFIX = "GD_Z1_RockPaperGenocide.M_RockPaperGenocide_Fire:Behavior_TriggerDialogEvent_"
BEHAVIORS = ["1183", "1184", "1185", "1186", "1187", "1188", "1189"]   # the mission's dialog behaviors (data census)
ACT_CLASSES = ["GearboxDialogAct_Talk", "WillowDialogAct_Talk"]
MARCUS_COMPONENT_OWNER = MARCUS_PATH   # from fire_mission_probe


def _dp_row(kind, **data):
    row = {"t": round(time.perf_counter() - FM["t0"], 4), "g": _game_t(), "f": FM["frame"], "dp": kind}
    row.update(data)
    FM["rows"].append(row)
    return row


def _dp_hook(path, typ, ident, fn):
    add_hook(path, typ, ident, fn)
    DP["hooks"].append((path, typ, ident))


def _act_info(act):
    """The act's talk data as (name tag, audio event) paths, for the line's name in the rows."""
    out = []
    try:
        for entry in list(act.TalkData)[:4]:
            out.append({"name_tag": _p(getattr(entry, "NameTag", None)), "ak_event": _p(getattr(entry, "AkEvent", None))})
    except Exception as e:
        out.append({"error": repr(e)[:80]})
    return out


def _event_data_state(ed):
    """Fields of a GearboxDialogEventData that matter for 'is the line live'. IsActive is read quietly (no row from our own hook)."""
    state = {"path": _p(ed)}
    for name in ("UseCount", "LiveTalkAction", "LiveTalkActionDataID", "LastTalker", "Instigator", "Other"):
        try:
            state[name] = _p(getattr(ed, name))
        except Exception:
            pass
    DP["quiet"] = True
    try:
        state["IsActive"] = bool(ed.IsActive())
    except Exception as e:
        state["IsActive"] = "error " + repr(e)[:60]
    finally:
        DP["quiet"] = False
    return state


def _behavior_state(name):
    # A mission's dialog behavior runs as a transient copy of the data object (found live: "Transient.Behavior_TriggerDialogEvent_N"), and
    # MyEventData lives on that copy, so the copies seen triggering for the mission tracker are polled by path as well.
    path = name if name.startswith("Transient.") else BEHAVIOR_PREFIX + name
    beh = unrealsdk.find_object("Behavior_TriggerDialogEvent", path)
    state = {"use_count": _p(getattr(beh, "MyDataUseCount", None))}
    ed = getattr(beh, "MyEventData", None)
    state["event_data"] = _event_data_state(ed) if ed is not None else None
    return state


def _components():
    """Dialog components worth polling: Marcus, the player's pawn, every echo actor. Refreshed from time to time."""
    found = {}
    try:
        marcus = unrealsdk.find_object("WillowAIPawn", MARCUS_PATH)
        found["marcus"] = marcus.DialogComponent
    except Exception:
        pass
    try:
        found["player"] = get_pc().Pawn.DialogComponent
    except Exception:
        pass
    try:
        for i, actor in enumerate(unrealsdk.find_all("WillowDialogEchoActor")):
            found[f"echo{i}:{actor.Name}"] = actor.DialogComponent
    except Exception:
        pass
    return found


def _manager_state():
    state = {}
    try:
        manager = list(unrealsdk.find_all("WillowDialogManager"))[0]
        state["echo_actor"] = _p(getattr(manager, "EchoActor", None))
        state["kickoff_playing"] = bool(manager.IsMissionKickoffPlaying())
    except Exception as e:
        state["error"] = repr(e)[:80]
    return state


def _poll_state():
    """The whole polled state as plain data; compared with the last one so a row is written only on change."""
    state = {"behaviors": {}, "components": {}, "manager": _manager_state()}
    for name in BEHAVIORS + DP["watch"]:
        try:
            state["behaviors"][name] = _behavior_state(name)
        except Exception as e:
            state["behaviors"][name] = {"error": repr(e)[:80]}
    DP["frame_comp"] += 1
    if DP["frame_comp"] % 60 == 1 or not DP["comps"]:
        DP["comps"] = _components()
    for label, comp in DP["comps"].items():
        try:
            state["components"][label] = bool(comp.IsTalking())
        except Exception:
            state["components"][label] = "gone"
    return state


def _diff(old, new, prefix=""):
    """Leaf-level differences between two nested dicts, as {path: (old, new)}."""
    out = {}
    for key in set(old) | set(new):
        a, b = old.get(key), new.get(key)
        if isinstance(a, dict) and isinstance(b, dict):
            out.update(_diff(a, b, prefix + str(key) + "."))
        elif a != b:
            out[prefix + str(key)] = (a, b)
    return out


def dp_mod_state():
    from mods_base import get_ordered_mod_list
    for mod in get_ordered_mod_list():
        if mod.name == "Dialog Skipper":
            option = {o.identifier: o.value for o in mod.options}
            return {"is_enabled": mod.is_enabled, "options": option, "hooks": len(mod.hooks)}
    return None


def dp_mod_off():
    """Disable the installed Dialog Skipper in memory. dont_update_setting=True: its settings file is not written."""
    from mods_base import get_ordered_mod_list
    for mod in get_ordered_mod_list():
        if mod.name == "Dialog Skipper":
            mod.disable(dont_update_setting=True)
            return dp_mod_state()
    return "Dialog Skipper not installed"


def dp_mod_hooks_on():
    """Control: put only the skipper's hooks back (no Mod.enable, so no settings write)."""
    from mods_base import get_ordered_mod_list
    for mod in get_ordered_mod_list():
        if mod.name == "Dialog Skipper":
            for hook in mod.hooks:
                hook.enable()
            return {"hooks_enabled": len(mod.hooks), "is_enabled_flag": mod.is_enabled}
    return "Dialog Skipper not installed"


def dp_start():
    dp_stop()
    DP.update(on=True, last={}, comps={}, frame_comp=0, errors=0, watch=DP.get("watch", []))

    def trigger_pre(obj, args, ret, func):
        if FM["on"] and DP["on"]:
            _dp_row("TriggerDialogEvent_pre", obj=_p(obj), args=_args(args, func), MyDataUseCount=_p(getattr(obj, "MyDataUseCount", None)))

    def trigger_post(obj, args, ret, func):
        if FM["on"] and DP["on"]:
            try:
                if "MissionTracker" in _p(args.ContextObject) and _p(obj) not in DP["watch"]:
                    DP["watch"].append(_p(obj))
            except Exception:
                pass
            _dp_row("TriggerDialogEvent_post", obj=_p(obj), MyEventData=_p(getattr(obj, "MyEventData", None)),
                    MyDataUseCount=_p(getattr(obj, "MyDataUseCount", None)))

    def is_active_post(obj, args, ret, func):
        if FM["on"] and DP["on"] and not DP["quiet"]:
            _dp_row("IsActive", obj=_p(obj), ret=_p(ret))

    def talk_pre(kind):
        def hook(obj, args, ret, func):
            if FM["on"] and DP["on"]:
                _dp_row(kind, obj=_p(obj), args=_args(args, func), talk_data=_act_info(obj))
        return hook

    resolved, missing = [], []
    for cls, fn, typ, ident, hook in (
            ("Behavior_TriggerDialogEvent", "TriggerDialogEvent", Type.PRE, "ow_dp_trig_pre", trigger_pre),
            ("Behavior_TriggerDialogEvent", "TriggerDialogEvent", Type.POST, "ow_dp_trig_post", trigger_post),
            ("GearboxDialogEventData", "IsActive", Type.POST, "ow_dp_isactive", is_active_post)):
        try:
            path = _func_path(cls, fn)
            _dp_hook(path, typ, ident + path, hook)
            resolved.append(path)
        except Exception as e:
            missing.append(f"{cls}.{fn}: {e!r}"[:100])
    for cls in ACT_CLASSES:
        for fn in ("TalkStarted", "TalkFinished"):
            try:
                path = _func_path(cls, fn)
                _dp_hook(path, Type.PRE, "ow_dp_" + fn + path, talk_pre(fn))
                resolved.append(path)
            except Exception as e:
                missing.append(f"{cls}.{fn}: {e!r}"[:100])

    def poll(*_a):
        if not (FM["on"] and DP["on"]):
            return
        try:
            state = _poll_state()
            changes = _diff(DP["last"], state)
            if changes:
                _dp_row("change", changes={k: list(v) for k, v in changes.items()})
            DP["last"] = state
        except Exception as e:
            DP["errors"] += 1
            if DP["errors"] <= 3:
                _dp_row("poll_error", error=repr(e)[:160])

    _dp_hook("WillowGame.WillowGameViewportClient:Tick", Type.POST_UNCONDITIONAL, "ow_dp_poll", poll)
    return {"resolved": resolved, "missing": missing}


def dp_stop():
    DP["on"] = False
    for path, typ, ident in DP["hooks"]:
        try:
            remove_hook(path, typ, ident)
        except Exception:
            pass
    DP["hooks"] = []


def dp_snap(label):
    state = _poll_state()
    return _dp_row("snap", label=label, state=state)
