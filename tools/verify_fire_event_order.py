#!/usr/bin/env python3
"""Checks the Fire mission's host-event order of `ow-package --slice-run` against RULES (script swap 8).

AI-assisted (Claude). The rules below are event kinds and names in an order, plus a few "never here" lists. They come from reading the real game's
Fire mission play-through (docs/verification/FIRE_MISSION_GOLDEN_TRACE.md: where the lent weapon is added and removed, the order inside the hit and the
turn-in, what the dummy does after spawning) and from the script and data the VM runs. No dumped game value is stored here (no amounts, no object
pointers): only names that the docs already use. Passing means the VM's event stream has this order. It does not mean the VM matches the game in anything
the rules do not say, and a rule marked UNVERIFIED reflects a native reading that the live trace did not show.

Two runs: the default one (no line player, the host's own case; LANE L2's step list) and a LINES run (Marcus registered as a talker, every dialog line 2 s,
longer gaps between the steps), which follows the unmodded dialog order of lane L4: 03b long, the dummy spawns during it and is bound to the Kismet
variable TargetDummy, then 03a spoken by the dummy, then 04. In the default run every line takes no time, so 03b has ended before the dummy exists,
TargetDummy is empty when 03a's behavior is applied and the application is dropped (the live skipper-on result; the empty-variable rule is UNVERIFIED).

Needs the installed game (the real packages), like tools/verify_packages.py:

    python tools/verify_fire_event_order.py --reader build/Release/ow-package.exe            # runs --slice-run twice (about 100 s)
    python tools/verify_fire_event_order.py --from-json run.json --lines-json lines.json     # checks existing --slice-run outputs
    python tools/verify_fire_event_order.py --self-test                                      # the checker against an invented event stream
"""
import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

MISSION = "GD_Z1_RockPaperGenocide.M_RockPaperGenocide_Fire"

# (label, --slice-run step). The labels are what RULES refer to; the step list is the one lane L2 compared with the real game.
# The experience in player:8:<total> is invented (21,000: inside level 8, which runs from 20,208 to 28,126 by the curve in NATIVE_PROGRESSION.md,
# and still inside it after the turn-in's 395), not the traced save's.
STEPS = [
    ("stage", "stage:8"), ("player", "player:8:21000"),
    ("use_offer", "use"), ("accept", "accept"),
    ("kickoff", "tick:0.02"), ("t01", "tick:0.1"), ("dialog02", "tick:0.5"), ("t1", "tick:1"),
    ("range", "range"), ("set2", "tick:1"),
    ("spawn", "spawn"), ("after_spawn", "tick:1"),
    ("hit", "hit:fire"), ("dialog05", "tick:0.02"), ("send_back", "tick:2.5"), ("target_back", "tick:1"),
    ("use_turnin", "use"), ("turnin", "turnin"), ("end1", "tick:0.1"), ("end2", "tick:1"),
    # Swap 11: Enter on the reward page the turn-in showed, then Escape (the screen's close).
    ("confirmreward", "confirmreward"), ("close", "close"),
]

# The LINES run: the same story with lines that take time (every line 2 s, Marcus a registered pawn talker), gaps long enough for each line.
STEPS_LINES = [
    ("stage", "stage:8"), ("talker", "talker:GD_Dialog_NPC.Names.DialogName_Marcus"), ("lines", "lines:2"), ("player", "player:8:21000"),
    ("use_offer", "use"), ("accept", "accept"), ("kickoff", "tick:0.02"), ("t01", "tick:3"), ("t02", "tick:3"),
    ("range", "range"), ("set2", "tick:1"), ("spawn", "spawn"), ("after_spawn", "tick:1.5"), ("chain", "tick:2.5"), ("chain_end", "tick:2.5"),
    ("hit", "hit:fire"), ("dialog05", "tick:0.02"),
]

# One rule: a title, the step label (None = the whole run), and one of
#   exact:    the events of that step, in order, are exactly these (kind, a-suffix or None)
#   sequence: these events occur in this order (other events may lie between)
#   absent:   these events do not occur in that step (or anywhere with None)
#   not_in:   (kind, a-suffix) must not occur in the steps with these labels
# The suffix is compared with the end of the event's `a` field (an object path, an event name, a status name); for a sequence_change with "sequence:action".
# An optional third element of a (kind, suffix, text) item must occur in the event's `detail` or in ";name_tag=<c>" (the talker's name tag).
RULES = [
    ("use before accept: Marcus's offer tag, then the mission screen, then his line", "use_offer", "sequence",
     [("on_use_dialog", "VO_NPC_OnUse_MissionsAvailable"), ("mission_interface", None), ("dialog", "DET_NPC_OnUse_MissionsAvailable")]),
    # Swap 8, observed in the real game: the lent weapon is added inside ActivateMission, after the status routine's script hook (the status event) and
    # before the initial objective set and the kickoff.
    # Swap 11 (the mission screen is open at the accept and closes inside it): the lent weapon still follows the status event, which now follows the close.
    ("accept: status Active, then the lent weapon", "accept", "sequence", [("status", "Active"), ("mission_weapon_granted", "MW_RockPaper_Fire")]),
    ("accept closes Marcus's screen: the dismissal tag, then the interface closed, then status Active, then the weapon", "accept", "sequence",
     [("dismissal_dialog", "VO_NPC_GenericDismissal"), ("mission_interface_closed", "Pawn_Marcus"), ("status", "Active"), ("mission_weapon_granted", "MW_RockPaper_Fire")]),
    ("the screen closes once at the accept and not before it", None, "not_in",
     [("mission_interface_closed", None), ["stage", "player", "use_offer", "kickoff", "t01", "dialog02", "t1", "range", "set2", "spawn", "after_spawn", "hit",
                                           "dialog05", "send_back", "target_back", "use_turnin", "turnin", "end1", "end2", "confirmreward"]]),
    ("kickoff (next tick): dialog 01, the first set, Marcus's walk", "kickoff", "sequence",
     [("dialog", "VOSQ_RockPaperGeno_01_EchoX_Marcus"), ("objective_set", "GoToRange_ObjSet"), ("remote_event", "RocksPaper_MoveMarcusToRange")]),
    ("dialog 02 comes after the walk starts", "dialog02", "sequence", [("dialog", "VOSQ_RockPaperGeno_02_EchoX_Marcus")]),
    ("range: the objective completes, then (0.5 s later) the second set; the weapon is not granted again", None, "sequence",
     [("objective_complete", "RockPaper_GoToRange"), ("objective_set", "RocksPaper_FinalObj")]),
    ("the second set grants nothing", "set2", "absent", [("mission_weapon_granted", None)]),
    ("spawn: the target moves forward", "spawn", "sequence", [("remote_event", "RocksPaper_MoveTargetForward")]),
    # The 1 s delay is a link of the mission's AdvanceObjectiveSet_96 (default output, ActivateDelay 1.0) to ChangeRemoteBehaviorSequenceState_171; the
    # dummy's Targetable sequence is enabled by it, not at spawn.
    ("Targetable is enabled by the mission's delayed behavior, not at spawn", "spawn", "absent", [("sequence_change", "Targetable:CHANGE_Enable")]),
    ("Targetable is enabled after the spawn", "after_spawn", "exact", [("sequence_change", "Targetable:CHANGE_Enable")]),
    # Observed in the real game: the lent weapon stays in the inventory after the objective completes.
    ("hit: the dummy's own sequence change, the objective, ReadyToTurnIn, FireCompleted", "hit", "sequence",
     [("sequence_change", "ObjectiveComplete:CHANGE_Enable"), ("objective_complete", "Fire"), ("status", "ReadyToTurnIn"), ("remote_event", "RocksPaper_FireCompleted")]),
    ("the weapon is not removed at the hit or before the turn-in", None, "not_in",
     [("mission_weapon_removed", None), ["use_offer", "accept", "kickoff", "t01", "dialog02", "t1", "range", "set2", "spawn", "after_spawn", "hit", "dialog05",
                                         "send_back", "target_back", "use_turnin"]]),
    ("after the hit the target is sent back and returns", None, "sequence",
     [("remote_event", "RocksPaper_SendTargetBack"), ("remote_event", "RocksPaper_TargetBack"), ("remote_event", "RocksPaper_TargetBack")]),
    ("the target's sequence is disabled again when it is sent back", "send_back", "sequence", [("sequence_change", "ObjectiveComplete:CHANGE_Disable")]),
    ("turn-in press: the mission-complete tag", "use_turnin", "sequence", [("on_use_dialog", "VO_NPC_OnUse_MissionComplete"), ("mission_interface", None)]),
    # Observed in the real game: the weapon is removed inside CompleteMission, after the status hook (experience) and before the reward; then the director's
    # PlayMissionTurnedInDialog raises its tag.
    # Swap 11: with the screen open the controller shows the reward page (ClientSpawnMissionRewardUI) between the experience and the status; the reward itself
    # waits for the player's confirmation.
    ("turn-in: experience, the reward page, Complete, the weapon is removed, the reward, the turned-in tag", "turnin", "exact",
     [("experience", None), ("reward_page", "M_RockPaperGenocide_Fire"), ("status", "Complete"), ("mission_weapon_removed", "MW_RockPaper_Fire"), ("reward", None),
      ("turn_in_dialog", "VO_NPC_MissionTurnedIn")]),
    ("the reward is not received before the player confirms the page", None, "not_in",
     [("reward_received", None), ["use_offer", "accept", "kickoff", "t01", "dialog02", "t1", "range", "set2", "spawn", "after_spawn", "hit", "dialog05",
                                  "send_back", "target_back", "use_turnin", "turnin", "end1", "end2"]]),
    ("confirming the page gives exactly reward_received", "confirmreward", "exact", [("reward_received", "M_RockPaperGenocide_Fire")]),
    ("Escape closes the screen: the dismissal tag, then the interface closed", "close", "sequence",
     [("dismissal_dialog", "VO_NPC_GenericDismissal"), ("mission_interface_closed", "Pawn_Marcus")]),
    ("closing the screen does not receive the reward again", "close", "absent", [("reward_received", None)]),
    # Lane L4 (skipper on, the same race): with no line player 03b ends at once, before the dummy exists; 03a's behavior is applied to the empty Kismet variable
    # TargetDummy and dropped, and 04 (behind it) never starts. UNVERIFIED rule (an empty context list drops the application), observed order.
    ("no line player: 03a and 04 do not run (TargetDummy is still empty when 03a is applied)", None, "absent",
     [("dialog", "VOSQ_RockPaperGeno_03a_LiveHypEng"), ("dialog", "VOSQ_RockPaperGeno_04_EchoX_Marcus")]),
    # The engine raises a level in the experience pool's per-frame update, so a level-up lands in a tick step after the turn-in, where no rule above looks.
    # The invented total (player:8:21000, plus the turn-in's experience) stays inside level 8; a level event anywhere means the total or the reward changed.
    ("no level-up anywhere in the run (the invented experience keeps the player at level 8)", None, "absent", [("level", None)]),
]

# Rules of the LINES run (labels of STEPS_LINES). Lane L4, unmodded game: 03b is spoken by Marcus, 03a (applied after 03b ends, long after the dummy spawned) by the
# target dummy pawn, then 04 by Marcus. The dummy's name tag, the instigator rule and the fallback that makes Marcus speak 03b are UNVERIFIED native readings.
RULES_LINES = [
    ("03b is a live line spoken by Marcus's pawn", "set2", "sequence", [("dialog", "VOSQ_RockPaperGeno_03b_echoX_Marcus", "outcome=started")]),
    ("03a does not start before the dummy spawned (it waits for 03b)", "set2", "absent", [("dialog", "VOSQ_RockPaperGeno_03a_LiveHypEng")]),
    ("03a does not start at the spawn", "spawn", "absent", [("dialog", "VOSQ_RockPaperGeno_03a_LiveHypEng")]),
    ("03a, then 04: 03a is spoken by the dummy (its name tag, a started line), 04 by Marcus", "chain", "sequence",
     [("dialog", "VOSQ_RockPaperGeno_03a_LiveHypEng", "outcome=started"), ("dialog", "VOSQ_RockPaperGeno_04_EchoX_Marcus", "outcome=started")]),
    ("03a's talker is the HypEngineer name tag", "chain", "sequence", [("dialog", "VOSQ_RockPaperGeno_03a_LiveHypEng", ";name_tag=GD_Dialog_NPC.Names.DialogName_HypEngineer")]),
    ("the spawn comes before 03a: the dummy moves forward first", None, "sequence",
     [("remote_event", "RocksPaper_MoveTargetForward"), ("dialog", "VOSQ_RockPaperGeno_03a_LiveHypEng")]),
]

# The dummy provider's behaviors in the order they ran (BehaviorProvider::trace, "event -> behavior"): as many as the real game's hooks saw, in its order.
DUMMY_ORDER = [
    "Behavior_SpecialMove_58", "Behavior_AIHold_57", "Behavior_Transform_12", "Behavior_IntMath_2", "Behavior_ChangeInstanceDataSwitch_15",
    "Behavior_IntMath_3", "Behavior_ChangeInstanceDataSwitch_14", "Behavior_RemoteEvent_60", "Behavior_RegisterTargetable_36",
    "Behavior_CompareObject_13", "Behavior_AIHold_59", "Behavior_ChangeRemoteBehaviorSequenceState_84", "Behavior_UpdateMissionObjective_13",
    "Behavior_RemoteEvent_44", "Behavior_ChangeRemoteBehaviorSequenceState_64",
]


def key(event):
    """What a rule's suffix is compared with: the event's `a` field; for a sequence change `sequence:action` (its `a` is the provider)."""
    return event["b"] + ":" + event["c"] if event["kind"] == "sequence_change" else event["a"]


def matches(event, kind, suffix, text=None):
    return (event["kind"] == kind and (suffix is None or key(event).endswith(suffix))
            and (text is None or text in event.get("detail", "") + ";name_tag=" + event["c"]))


def steps_of(run, steps=None):
    """step label -> that step's events, from the records of the run (the labels follow STEPS, or `steps`)."""
    steps = steps or STEPS
    records = run["steps"]
    if len(records) != len(steps):
        raise ValueError(f"the run has {len(records)} step records, the step list has {len(steps)}")
    return {label: record.get("events", []) for (label, _), record in zip(steps, records)}


def apply_rules(run, steps, rules):
    """Returns a list of (title, ok, detail) for the rules, over the run's step records (labels from `steps`)."""
    by_step = steps_of(run, steps)
    whole = [event for (label, _) in steps for event in by_step[label]]
    results = []
    for title, label, mode, spec in rules:
        events = whole if label is None else by_step[label]
        ok, detail = True, ""
        if mode == "exact":
            ok = len(events) == len(spec) and all(matches(e, *item) for e, item in zip(events, spec))
            detail = "" if ok else "got " + ", ".join(f"{e['kind']}:{e['a'].split('.')[-1]}" for e in events)
        elif mode == "sequence":
            at = 0
            for item in spec:
                while at < len(events) and not matches(events[at], *item): at += 1
                if at == len(events): ok, detail = False, f"missing or out of order: {item[0]} {item[1]}"; break
                at += 1
        elif mode == "absent":
            found = [e for e in events if any(matches(e, *item) for item in spec)]
            ok, detail = not found, "" if not found else "found " + ", ".join(f"{e['kind']}:{e['a'].split('.')[-1]}" for e in found)
        elif mode == "not_in":
            (kind, suffix), labels = spec
            found = [(lab, e) for lab in labels for e in by_step[lab] if matches(e, kind, suffix)]
            ok, detail = not found, "" if not found else "found in " + ", ".join(lab for lab, _ in found)
        results.append((title, ok, detail))
    return results


def check_lines(run):
    """The LINES run: its rules, and no errors."""
    return apply_rules(run, STEPS_LINES, RULES_LINES) + [("the lines run has no errors", run.get("errors") == [], str(run.get("errors")))]


def check(run):
    """Returns a list of (title, ok, detail) for every rule, plus the dummy order and the error list."""
    results = apply_rules(run, STEPS, RULES)
    trace = [line.split(" -> ")[-1] for line in run.get("dummy_trace", [])]
    positions, at = [], 0
    for name in DUMMY_ORDER:
        while at < len(trace) and trace[at] != name: at += 1
        positions.append(at)
        if at == len(trace): break
        at += 1
    ok = positions and positions[-1] < len(trace) and len(positions) == len(DUMMY_ORDER)
    results.append(("the dummy's behaviors ran in the observed order (AIHold_57 after SpecialMove_58)", bool(ok), "" if ok else f"trace: {trace}"))
    adjacent = any(a == "Behavior_SpecialMove_58" and b == "Behavior_AIHold_57" for a, b in zip(trace, trace[1:]))
    results.append(("AIHold_57 runs right after SpecialMove_58 (the move's output 0)", adjacent, "" if adjacent else f"trace: {trace}"))
    results.append(("the run has no errors", run.get("errors") == [], str(run.get("errors"))))
    return results


def run_slice(reader, cooked, steps=None):
    command = [str(Path(reader).resolve()), str(Path(cooked) / "Sanctuary_Dynamic.upk"), "--slice-run", MISSION, "--cooked", str(cooked)] + [step for _, step in (steps or STEPS)]
    done = subprocess.run(command, capture_output=True, text=True, encoding="utf-8")
    if not done.stdout.strip():
        raise SystemExit(f"--slice-run printed nothing (exit {done.returncode}): {done.stderr.strip()}")
    return json.loads(done.stdout)


def synthetic_run(wrong_weapon_step=False, early_reward=False, late_close=False, level_up=False):
    """An invented event stream that follows RULES (self-test only). With wrong_weapon_step the weapon is removed at the hit; with early_reward the reward is
    received inside the turn-in; with late_close the accept's close comes after the status event; with level_up the tick after the turn-in raises a level."""
    def e(kind, a="", b="", c=""): return {"kind": kind, "a": a, "b": b, "c": c, "detail": ""}
    events = {label: [] for label, _ in STEPS}
    events["use_offer"] = [e("on_use_dialog", "VO_NPC_OnUse_MissionsAvailable"), e("mission_interface", "Def"), e("dialog", "DET_NPC_OnUse_MissionsAvailable")]
    events["accept"] = [e("dismissal_dialog", "VO_NPC_GenericDismissal"), e("dialog", "DET_NPC_GenericDismissal"), e("mission_interface_closed", "Pawn_Marcus"),
                        e("status", "Active"), e("mission_weapon_granted", "MW_RockPaper_Fire")]
    events["kickoff"] = [e("dialog", "VOSQ_RockPaperGeno_01_EchoX_Marcus"), e("objective_set", "GoToRange_ObjSet"), e("remote_event", "RocksPaper_MoveMarcusToRange")]
    events["dialog02"] = [e("dialog", "VOSQ_RockPaperGeno_02_EchoX_Marcus")]
    events["range"] = [e("objective_complete", "RockPaper_GoToRange")]
    events["set2"] = [e("objective_set", "RocksPaper_FinalObj")]
    events["spawn"] = [e("remote_event", "RocksPaper_MoveTargetForward")]
    events["after_spawn"] = [e("sequence_change", "Dummy", "Targetable", "CHANGE_Enable")]
    events["hit"] = [e("sequence_change", "Dummy", "ObjectiveComplete", "CHANGE_Enable"), e("objective_complete", "Fire"), e("status", "ReadyToTurnIn"), e("remote_event", "RocksPaper_FireCompleted")]
    if wrong_weapon_step: events["hit"].append(e("mission_weapon_removed", "MW_RockPaper_Fire"))
    events["send_back"] = [e("remote_event", "RocksPaper_SendTargetBack"), e("sequence_change", "Dummy", "ObjectiveComplete", "CHANGE_Disable")]
    events["target_back"] = [e("remote_event", "RocksPaper_TargetBack"), e("remote_event", "RocksPaper_TargetBack")]
    events["use_turnin"] = [e("on_use_dialog", "VO_NPC_OnUse_MissionComplete"), e("mission_interface", "Def")]
    events["turnin"] = [e("experience", "1"), e("reward_page", "M_RockPaperGenocide_Fire"), e("status", "Complete"), e("mission_weapon_removed", "MW_RockPaper_Fire"),
                        e("reward", "X"), e("turn_in_dialog", "VO_NPC_MissionTurnedIn")]
    events["confirmreward"] = [e("reward_received", "M_RockPaperGenocide_Fire")]
    events["close"] = [e("dismissal_dialog", "VO_NPC_GenericDismissal"), e("dialog", "DET_NPC_GenericDismissal"), e("mission_interface_closed", "Pawn_Marcus")]
    if early_reward: events["turnin"].append(e("reward_received", "M_RockPaperGenocide_Fire"))
    if late_close: events["accept"].insert(3, events["accept"].pop(2))      # mission_interface_closed after the status event
    if level_up: events["end1"].append(e("level", "9"))      # the pool's per-frame update, one tick after the turn-in
    return {"steps": [{"step": step, "events": events[label]} for label, step in STEPS], "errors": [],
            "dummy_trace": ["OnBehaviorSequenceEnabled -> " + name for name in DUMMY_ORDER]}


def synthetic_lines_run(dummy_too_early=False):
    """An invented LINES event stream that follows RULES_LINES. With dummy_too_early 03a starts at the spawn (before 03b would have ended)."""
    def e(kind, a="", b="", c="", detail=""): return {"kind": kind, "a": a, "b": b, "c": c, "detail": detail}
    line_03b = e("dialog", "VOSQ_RockPaperGeno_03b_echoX_Marcus", c="Names.DialogName_Marcus", detail="talker=pawn;outcome=started")
    line_03a = e("dialog", "VOSQ_RockPaperGeno_03a_LiveHypEng", c="GD_Dialog_NPC.Names.DialogName_HypEngineer", detail="talker=pawn;outcome=started")
    line_04 = e("dialog", "VOSQ_RockPaperGeno_04_EchoX_Marcus", c="Names.DialogName_Marcus", detail="talker=pawn;outcome=started")
    events = {label: [] for label, _ in STEPS_LINES}
    events["set2"] = [e("objective_set", "RocksPaper_FinalObj"), line_03b]
    events["spawn"] = [e("remote_event", "RocksPaper_MoveTargetForward")] + ([line_03a] if dummy_too_early else [])
    events["chain"] = [] if dummy_too_early else [line_03a, line_04]
    return {"steps": [{"step": step, "events": events[label]} for label, step in STEPS_LINES], "errors": []}


def self_test():
    lines_good = check_lines(synthetic_lines_run())
    lines_bad = check_lines(synthetic_lines_run(dummy_too_early=True))
    lines_failures = []
    if not all(ok for _, ok, _ in lines_good): lines_failures.append("the invented lines stream that follows RULES_LINES failed: " + str([t for t, ok, _ in lines_good if not ok]))
    if not any("does not start at the spawn" in t for t, ok, _ in lines_bad if not ok): lines_failures.append("03a at the spawn was not caught")
    good = check(synthetic_run())
    bad = check(synthetic_run(wrong_weapon_step=True))
    failures = []
    if not all(ok for _, ok, _ in good): failures.append("the invented stream that follows RULES failed: " + str([t for t, ok, _ in good if not ok]))
    failed_bad = [t for t, ok, _ in bad if not ok]
    if not any("weapon is not removed" in t for t in failed_bad): failures.append("a weapon removed at the hit was not caught: " + str(failed_bad))
    if not any("reward is not received before" in t for t, ok, _ in check(synthetic_run(early_reward=True)) if not ok): failures.append("a reward received before the confirmation was not caught")
    if not any("accept closes Marcus" in t for t, ok, _ in check(synthetic_run(late_close=True)) if not ok): failures.append("a close after the status event was not caught")
    if not any("no level-up" in t for t, ok, _ in check(synthetic_run(level_up=True)) if not ok): failures.append("a level-up in the tick after the turn-in was not caught")
    shuffled = synthetic_run()
    shuffled["dummy_trace"] = [line for line in shuffled["dummy_trace"] if not line.endswith("Behavior_AIHold_57")]
    if all(ok for _, ok, _ in check(shuffled)): failures.append("a missing AIHold_57 was not caught")
    with_03a = synthetic_run()
    with_03a["steps"][STEPS.index(("range", "range"))]["events"].append({"kind": "dialog", "a": "VOSQ_RockPaperGeno_03a_LiveHypEng", "b": "", "c": "", "detail": ""})
    if all(ok for _, ok, _ in check(with_03a)): failures.append("03a in the no-line-player run was not caught")
    failures += lines_failures
    for failure in failures: print("self-test FAIL: " + failure)
    print("self-test " + ("FAILED" if failures else "passed"))
    return 1 if failures else 0


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--reader", default="build/Release/ow-package.exe")
    parser.add_argument("--cooked", default=os.environ.get("OPENWILLOW_BL2", "") and str(Path(os.environ["OPENWILLOW_BL2"]) / "WillowGame" / "CookedPCConsole"))
    parser.add_argument("--from-json", help="check this --slice-run output (made with the STEPS above) instead of running the reader")
    parser.add_argument("--lines-json", help="with --from-json: the LINES run's --slice-run output (made with STEPS_LINES); without it the LINES rules are skipped")
    parser.add_argument("--output", help="write the default run's --slice-run output here (ignored local/ only)")
    parser.add_argument("--lines-output", help="write the LINES run's --slice-run output here (ignored local/ only)")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test: return self_test()
    lines_run = None
    if args.from_json:
        run = json.loads(Path(args.from_json).read_text(encoding="utf-8"))
        if args.lines_json: lines_run = json.loads(Path(args.lines_json).read_text(encoding="utf-8"))
    else:
        if not args.cooked: raise SystemExit("set OPENWILLOW_BL2 or pass --cooked <CookedPCConsole>")
        run = run_slice(args.reader, args.cooked)
        if args.output: Path(args.output).write_text(json.dumps(run), encoding="utf-8")
        lines_run = run_slice(args.reader, args.cooked, STEPS_LINES)
        if args.lines_output: Path(args.lines_output).write_text(json.dumps(lines_run), encoding="utf-8")
    results = check(run) + (check_lines(lines_run) if lines_run else [])
    for title, ok, detail in results:
        print(("PASS  " if ok else "FAIL  ") + title + ("" if ok else "  [" + detail + "]"))
    failed = sum(1 for _, ok, _ in results if not ok)
    print(f"{len(results) - failed} of {len(results)} rules hold")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
