#!/usr/bin/env python3
"""Checks the Fire mission's host-event order of `ow-package --slice-run` against RULES (script swap 8).

AI-assisted (Claude). The rules below are event kinds and names in an order, plus a few "never here" lists. They come from reading the real game's
Fire mission play-through (docs/verification/FIRE_MISSION_GOLDEN_TRACE.md: where the lent weapon is added and removed, the order inside the hit and the
turn-in, what the dummy does after spawning) and from the script and data the VM runs. No dumped game value is stored here (no amounts, no object
pointers): only names that the docs already use. Passing means the VM's event stream has this order. It does not mean the VM matches the game in anything
the rules do not say, and a rule marked UNVERIFIED reflects a native reading that the live trace did not show.

Needs the installed game (the real packages), like tools/verify_packages.py:

    python tools/verify_fire_event_order.py --reader build/Release/ow-package.exe            # runs --slice-run (about 50 s)
    python tools/verify_fire_event_order.py --from-json run.json                             # checks an existing --slice-run output
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
STEPS = [
    ("stage", "stage:8"), ("player", "player:8:26218"),
    ("use_offer", "use"), ("accept", "accept"),
    ("kickoff", "tick:0.02"), ("t01", "tick:0.1"), ("dialog02", "tick:0.5"), ("t1", "tick:1"),
    ("range", "range"), ("set2", "tick:1"),
    ("spawn", "spawn"), ("after_spawn", "tick:1"),
    ("hit", "hit:fire"), ("dialog05", "tick:0.02"), ("send_back", "tick:2.5"), ("target_back", "tick:1"),
    ("use_turnin", "use"), ("turnin", "turnin"), ("end1", "tick:0.1"), ("end2", "tick:1"),
]

# One rule: a title, the step label (None = the whole run), and one of
#   exact:    the events of that step, in order, are exactly these (kind, a-suffix or None)
#   sequence: these events occur in this order (other events may lie between)
#   absent:   these events do not occur in that step (or anywhere with None)
#   not_in:   (kind, a-suffix) must not occur in the steps with these labels
# The suffix is compared with the end of the event's `a` field (an object path, an event name, a status name); for a sequence_change with "sequence:action".
RULES = [
    ("use before accept: Marcus's offer tag, then the mission screen, then his line", "use_offer", "sequence",
     [("on_use_dialog", "VO_NPC_OnUse_MissionsAvailable"), ("mission_interface", None), ("dialog", "DET_NPC_OnUse_MissionsAvailable")]),
    # Swap 8, observed in the real game: the lent weapon is added inside ActivateMission, after the status routine's script hook (the status event) and
    # before the initial objective set and the kickoff.
    ("accept: status Active, then the lent weapon, nothing else", "accept", "exact", [("status", "Active"), ("mission_weapon_granted", "MW_RockPaper_Fire")]),
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
    ("turn-in: experience, Complete, the weapon is removed, the reward, the turned-in tag", "turnin", "exact",
     [("experience", None), ("status", "Complete"), ("mission_weapon_removed", "MW_RockPaper_Fire"), ("reward", None), ("turn_in_dialog", "VO_NPC_MissionTurnedIn")]),
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


def matches(event, kind, suffix):
    return event["kind"] == kind and (suffix is None or key(event).endswith(suffix))


def steps_of(run):
    """step label -> that step's events, from the records of the run (the labels follow STEPS)."""
    records = run["steps"]
    if len(records) != len(STEPS):
        raise ValueError(f"the run has {len(records)} step records, STEPS has {len(STEPS)}")
    return {label: record.get("events", []) for (label, _), record in zip(STEPS, records)}


def check(run):
    """Returns a list of (title, ok, detail) for every rule, plus the dummy order and the error list."""
    by_step = steps_of(run)
    whole = [event for (label, _) in STEPS for event in by_step[label]]
    results = []
    for title, label, mode, spec in RULES:
        events = whole if label is None else by_step[label]
        ok, detail = True, ""
        if mode == "exact":
            ok = len(events) == len(spec) and all(matches(e, k, s) for e, (k, s) in zip(events, spec))
            detail = "" if ok else "got " + ", ".join(f"{e['kind']}:{e['a'].split('.')[-1]}" for e in events)
        elif mode == "sequence":
            at = 0
            for kind, suffix in spec:
                while at < len(events) and not matches(events[at], kind, suffix): at += 1
                if at == len(events): ok, detail = False, f"missing or out of order: {kind} {suffix}"; break
                at += 1
        elif mode == "absent":
            found = [e for e in events if any(matches(e, k, s) for k, s in spec)]
            ok, detail = not found, "" if not found else "found " + ", ".join(f"{e['kind']}:{e['a'].split('.')[-1]}" for e in found)
        elif mode == "not_in":
            (kind, suffix), labels = spec
            found = [(lab, e) for lab in labels for e in by_step[lab] if matches(e, kind, suffix)]
            ok, detail = not found, "" if not found else "found in " + ", ".join(lab for lab, _ in found)
        results.append((title, ok, detail))
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


def run_slice(reader, cooked):
    command = [str(Path(reader).resolve()), str(Path(cooked) / "Sanctuary_Dynamic.upk"), "--slice-run", MISSION, "--cooked", str(cooked)] + [step for _, step in STEPS]
    done = subprocess.run(command, capture_output=True, text=True, encoding="utf-8")
    if not done.stdout.strip():
        raise SystemExit(f"--slice-run printed nothing (exit {done.returncode}): {done.stderr.strip()}")
    return json.loads(done.stdout)


def synthetic_run(wrong_weapon_step=False):
    """An invented event stream that follows RULES (self-test only). With wrong_weapon_step the weapon is removed at the hit."""
    def e(kind, a="", b="", c=""): return {"kind": kind, "a": a, "b": b, "c": c, "detail": ""}
    events = {label: [] for label, _ in STEPS}
    events["use_offer"] = [e("on_use_dialog", "VO_NPC_OnUse_MissionsAvailable"), e("mission_interface", "Def"), e("dialog", "DET_NPC_OnUse_MissionsAvailable")]
    events["accept"] = [e("status", "Active"), e("mission_weapon_granted", "MW_RockPaper_Fire")]
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
    events["turnin"] = [e("experience", "1"), e("status", "Complete"), e("mission_weapon_removed", "MW_RockPaper_Fire"), e("reward", "X"), e("turn_in_dialog", "VO_NPC_MissionTurnedIn")]
    return {"steps": [{"step": step, "events": events[label]} for label, step in STEPS], "errors": [],
            "dummy_trace": ["OnBehaviorSequenceEnabled -> " + name for name in DUMMY_ORDER]}


def self_test():
    good = check(synthetic_run())
    bad = check(synthetic_run(wrong_weapon_step=True))
    failures = []
    if not all(ok for _, ok, _ in good): failures.append("the invented stream that follows RULES failed: " + str([t for t, ok, _ in good if not ok]))
    failed_bad = [t for t, ok, _ in bad if not ok]
    if not any("weapon is not removed" in t for t in failed_bad): failures.append("a weapon removed at the hit was not caught: " + str(failed_bad))
    shuffled = synthetic_run()
    shuffled["dummy_trace"] = [line for line in shuffled["dummy_trace"] if not line.endswith("Behavior_AIHold_57")]
    if all(ok for _, ok, _ in check(shuffled)): failures.append("a missing AIHold_57 was not caught")
    for failure in failures: print("self-test FAIL: " + failure)
    print("self-test " + ("FAILED" if failures else "passed"))
    return 1 if failures else 0


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--reader", default="build/Release/ow-package.exe")
    parser.add_argument("--cooked", default=os.environ.get("OPENWILLOW_BL2", "") and str(Path(os.environ["OPENWILLOW_BL2"]) / "WillowGame" / "CookedPCConsole"))
    parser.add_argument("--from-json", help="check this --slice-run output (made with the STEPS above) instead of running the reader")
    parser.add_argument("--output", help="write the --slice-run output here (ignored local/ only)")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test: return self_test()
    if args.from_json: run = json.loads(Path(args.from_json).read_text(encoding="utf-8"))
    else:
        if not args.cooked: raise SystemExit("set OPENWILLOW_BL2 or pass --cooked <CookedPCConsole>")
        run = run_slice(args.reader, args.cooked)
        if args.output: Path(args.output).write_text(json.dumps(run), encoding="utf-8")
    results = check(run)
    for title, ok, detail in results:
        print(("PASS  " if ok else "FAIL  ") + title + ("" if ok else "  [" + detail + "]"))
    failed = sum(1 for _, ok, _ in results if not ok)
    print(f"{len(results) - failed} of {len(results)} rules hold")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
