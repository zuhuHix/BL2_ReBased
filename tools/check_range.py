#!/usr/bin/env python3
# AI-assisted (Claude). Grades a host gun-range test log (-owrangetest, tools/test_range.ps1).
#
# The host logs one line per step:  OWRANGETEST step=<name> result=PASS|FAIL|SKIP [key=value ...]
# and a closing                       OWRANGETEST SUMMARY result=PASS|FAIL pass=N fail=M skip=K
# This checker does not trust the summary alone: every expected step must appear, a FAIL anywhere fails the run, and a SKIP
# passes only for the steps that may be skipped when local data is missing (they are listed apart, so a report can say
# "passed except for missing data" honestly). Host behaviour only: the range rules are UNVERIFIED in game.
#
# Usage: python tools/check_range.py <log> [--steps a,b,c]
import argparse
import json
import math
import os
import re
import sys
from collections import Counter

# The steps of the driver, in order (OpenWillowRange.cpp): PR1's, then PR2's enemy AI (TickAITest, steps 16-36), then PR3's loot
# on death (TickLootTest, steps 37-44).
EXPECTED = ["teleport", "placement", "body_hit", "crit_hit", "popup_crit", "crit_minds_eye", "crit_trace", "shield_order", "invulnerable",
            "passive", "patrol", "death", "ragdoll", "respawn", "dummy_respawn",
            "notice_proximity", "notice_sight", "no_notice_far", "provoked", "approach", "melee_hit", "melee_interval",
            "melee_miss", "forget", "passive_no_melee", "swing_range", "corpse_limp", "corpse_cap", "psycho_melee", "kind_melee",
            "loot_near_corpse", "loot_roll", "loot_money_pickup", "loot_weapon_item", "loot_off", "loot_seeded_roll", "refusal_no_spot"]
# Steps that may be SKIP when their data is absent: no imported stock mesh / physics asset (ragdoll), and no weapon generator
# manifest (loot_weapon_item: no gun is rolled, the money checks still decide), no imported psycho (psycho_melee: it isn't spawned then),
# and no local skill effects file (crit_minds_eye: tools/prepare_skill_effects.py).
MAY_SKIP = {"ragdoll", "corpse_limp", "corpse_cap", "loot_weapon_item", "psycho_melee", "crit_minds_eye"}
# A step here may SKIP only for this reason; any other SKIP of it is a failure.
NO_STOCK_MESH = "reason=the manifest names no stock mesh"
NO_PSYCHO = "reason=no psycho on this range (its mesh is not imported)"
SKIP_ONLY_FOR = {"crit_minds_eye": "reason=no local skill effects file", "ragdoll": NO_STOCK_MESH, "corpse_limp": NO_STOCK_MESH, "corpse_cap": NO_STOCK_MESH,
                 "psycho_melee": NO_PSYCHO}

# The acceptance table of docs/verification/COMBAT_RANGE.md ("Acceptance table: melee per kind"): per range kind with a live read, the
# swing cycle (s, one swing start to the next) and the distance it stands at to swing once it has closed (uu, horizontal, enemy origin
# to Maya's). "value" is what the host uses (OpenWillowEnemyAI.cpp, KindMeleeRows; tests/range_kind_melee_test.py holds the two to
# each other); a kind_melee result must land in the band. The bands are the measured spreads; the one host allowance is on the cycle's
# upper edge, raised by the run's longest frame (max_frame, logged by the host), because a swing starts on the first frame after it is due.
# The stand distance needs none: the host clamps the last step of the run to it. This checker is the only place the numbers are graded.
# Sources and how each band was chosen are in the record. Kinds not listed have no live read and keep the host stand-ins. Every row is
# one session's measurement, UNVERIFIED beyond it.
ACCEPTANCE = {
    # cycle: AI_MELEE_TARGETING_LIVE 4 (one adult, 34 gaps). The den read of 2026-11-07 disagrees (1.216 to 1.217 s, 9 gaps): open.
    # stand: the den read (one adult, 181 samples, 190 to 225).
    "bullymong_adult": {"cycle": 1.25, "cycle_band": (1.221, 1.386), "stand": 201.0, "stand_band": (190.0, 225.0)},
    # both from the den read (grab cycle: two babies, 31 gaps, 1.133 to 1.134, widened 0.005 each way for the logs' millisecond rounding;
    # stand: one baby, 347 samples, 106 to 118).
    "bullymong_baby": {"cycle": 1.133, "cycle_band": (1.128, 1.139), "stand": 111.0, "stand_band": (106.0, 118.0)},
}
# A frame longer than this is a stall, not a frame: the cycle allowance stops there (the host's state update is 0.2 s). It counts only when
# a cycle is over its plain band; then the line asks for a rerun.
MAX_FRAME = 0.2
KEY_VALUE = re.compile(r"(\w+)=(\S+)")


# The fields a kind_melee result must carry, each exactly once. The host needs four swings: the first on the way in, three where it stands.
KIND_MELEE_FIELDS = ("kind", "enemy", "cycle_min", "cycle_max", "stand_min", "stand_max", "first_swing", "start_range", "max_frame", "swings")
KIND_MELEE_SWINGS = 4
# One per range enemy at its setup (AOpenWillowEnemy::SetupKind): which kind it was spawned as and where its melee comes from.
SPAWN_LINE = re.compile(r"OWRANGE enemy (\S+) kind_melee kind=(\S+) swing_interval=\S+ stand=\S+ source=(\S+)")


def kind_melee_problem(detail):
    """Why a kind_melee PASS line does not land in the acceptance table, or "". The host judges its own run too; this re-checks the logged
    numbers against the table above, so a host that carries other numbers cannot pass. Incomplete or impossible evidence fails."""
    pairs = KEY_VALUE.findall(detail)
    counts = Counter(key for key, _ in pairs)
    repeated = sorted(key for key, n in counts.items() if n > 1)
    if repeated:
        return "the line repeats %s (each field must appear once)" % ", ".join(repeated)
    missing = [key for key in KIND_MELEE_FIELDS if key not in counts]
    if missing:
        return "the line lacks %s" % ", ".join(missing)
    fields = dict(pairs)
    row = ACCEPTANCE.get(fields["kind"])
    if row is None:
        return "kind %s has no row in the acceptance table" % fields["kind"]
    numbers = {}
    for key in ("cycle_min", "cycle_max", "stand_min", "stand_max", "first_swing", "start_range", "max_frame"):
        try:
            numbers[key] = float(fields[key])
        except ValueError:
            return "%s=%s is not a number" % (key, fields[key])
        if not math.isfinite(numbers[key]):
            return "%s=%s is not a finite number" % (key, fields[key])
    if not re.fullmatch(r"\d+", fields["swings"]) or int(fields["swings"]) < KIND_MELEE_SWINGS:
        return "swings=%s: the step needs at least %d swing starts" % (fields["swings"], KIND_MELEE_SWINGS)
    cycle = (numbers["cycle_min"], numbers["cycle_max"])
    stand = (numbers["stand_min"], numbers["stand_max"])
    first, start_range, max_frame = numbers["first_swing"], numbers["start_range"], numbers["max_frame"]
    if not max_frame > 0:
        return "max_frame %.3f s is not a frame time (it must be above 0)" % max_frame
    if cycle[0] > cycle[1] or stand[0] > stand[1]:
        return "a minimum is above its maximum (cycle %.3f to %.3f, stand %.0f to %.0f)" % (cycle[0], cycle[1], stand[0], stand[1])
    low, high = row["cycle_band"]
    # The allowance is needed only above the plain band; a long frame elsewhere in the step does not matter then.
    if not (low <= cycle[0] and cycle[1] <= high):
        if cycle[0] < low:
            return "cycle %.3f to %.3f s is outside the acceptance band %.3f to %.3f" % (cycle[0], cycle[1], low, high)
        if max_frame > MAX_FRAME:
            return ("stalled run, repeat it: the cycle %.3f s is over the band's %.3f s and the step had a %.3f s frame, longer than the %.2f s "
                    "the allowance covers" % (cycle[1], high, max_frame, MAX_FRAME))
        if cycle[1] > high + max_frame:
            return "cycle %.3f to %.3f s is outside the acceptance band %.3f to %.3f (+ max_frame %.3f)" % (cycle[0], cycle[1], low, high, max_frame)
    low, high = row["stand_band"]
    if not (low <= stand[0] and stand[1] <= high):
        return "stand %.0f to %.0f uu is outside the acceptance band %.0f to %.0f" % (stand[0], stand[1], low, high)
    if not (0 < first <= start_range):
        return "the first swing started %.0f uu out, not inside the stock range %.0f" % (first, start_range)
    return ""


def kind_melee_graded(results, spawns):
    """The kind_melee results with the checker's verdicts: each PASS needs good numbers (kind_melee_problem) from an enemy spawned as that kind
    with a live row, and every kind spawned with a live row needs exactly one such result. spawns: enemy name -> (kind, source)."""
    expected = sorted({kind for kind, source in spawns.values() if source == "live_read"})
    graded, passed = [], Counter()
    for verdict, detail in results:
        if verdict != "PASS":
            graded.append((verdict, detail))
            continue
        fields = dict(KEY_VALUE.findall(detail))
        problem = kind_melee_problem(detail)
        if not problem:
            spawned = spawns.get(fields.get("enemy", ""))
            if spawned is None or spawned[0] != fields["kind"] or spawned[1] != "live_read":
                problem = "enemy %s was not spawned as %s with a live melee row (its spawn record: %s)" % (fields.get("enemy"), fields["kind"], spawned)
        if problem:
            graded.append(("FAIL", detail + " (checker: " + problem + ")"))
            continue
        passed[fields["kind"]] += 1
        graded.append(("PASS", detail))
    if not expected:
        graded.append(("FAIL", "(checker: no enemy was spawned with a live melee row: no 'OWRANGE enemy ... kind_melee ... source=live_read' line)"))
    for kind in expected:
        if passed[kind] != 1:
            graded.append(("FAIL", "(checker: %d passing results for spawned kind %s, exactly 1 wanted)" % (passed[kind], kind)))
    return graded


class ParsedSteps(dict):
    """Step name -> [(verdict, detail)], plus .spawns: enemy name -> (kind, melee source) from the spawn lines of the same log."""
    spawns = None


# The status ends at whitespace or the end of the line: result=SKIP_typo is no status, so its step counts as missing.
SURVEY_DONE = re.compile(r"OWRANGESURVEY done candidates=\d+ accepted=([1-9]\d*)(?:\s|$)", re.M)
KILL_RESULT = re.compile(r"OWRANGEKILL gun=(\d+) result=(PASS|FAIL) .*?: (\d+) shots", re.M)
KILL_GUNS = 2


def survey_ok(text):
    """A survey counts when it finished and built a range on at least one candidate."""
    return bool(SURVEY_DONE.search(text))


def kills_ok(text):
    """(ok, why) for tools/play_range.ps1 -KillRow: every gun has its own PASS line with shots fired, and the row finished with no failure."""
    if "OWRANGEKILL done guns=%d fail=0" % KILL_GUNS not in text:
        return False, "no \"OWRANGEKILL done guns=%d fail=0\" line" % KILL_GUNS
    found = {int(gun): (result, int(shots)) for gun, result, shots in KILL_RESULT.findall(text)}
    for gun in range(KILL_GUNS):
        if found.get(gun, ("", 0))[0] != "PASS" or found[gun][1] <= 0:
            return False, "gun %d: %s" % (gun, found.get(gun, "no result line"))
    return True, ""


LINE = re.compile(r"OWRANGETEST step=(\S+) result=(PASS|FAIL|SKIP)(?=\s|$)(.*)")
SUMMARY = re.compile(r"OWRANGETEST SUMMARY result=(PASS|FAIL)(?=\s|$)(.*)")


def parse(text):
    steps, summary = ParsedSteps(), None
    steps.spawns = {}
    for line in text.splitlines():
        m = SPAWN_LINE.search(line)
        if m:
            steps.spawns[m.group(1)] = (m.group(2), m.group(3))
            continue
        m = LINE.search(line)
        if m:
            steps.setdefault(m.group(1), []).append((m.group(2), m.group(3).strip()))
            continue
        m = SUMMARY.search(line)
        if m:
            summary = (m.group(1), m.group(2).strip())
    return steps, summary


def grade(steps, summary, expected=EXPECTED):
    """(ok, rows): rows are (step, verdict, detail) with verdict PASS, FAIL, SKIP or MISSING."""
    rows, ok = [], True
    for step in expected:
        results = steps.get(step)
        if not results:
            rows.append((step, "MISSING", ""))
            ok = False
            continue
        if step == "kind_melee":
            # Each PASS line must carry numbers inside the acceptance bands and come from an enemy spawned as that kind, one per spawned
            # kind with a live row (the host's own verdict is not enough). Steps not read from a log carry no spawns: that fails here.
            results = kind_melee_graded(results, getattr(steps, "spawns", None) or {})
        verdicts = [r for r, _ in results]
        verdict = "FAIL" if "FAIL" in verdicts else ("SKIP" if "SKIP" in verdicts else "PASS")
        detail = next((d for r, d in results if r == verdict), "")
        if verdict == "FAIL" or (verdict == "SKIP" and step not in MAY_SKIP):
            ok = False
        if verdict == "SKIP" and step in SKIP_ONLY_FOR and SKIP_ONLY_FOR[step] != detail:
            ok = False
        rows.append((step, verdict, detail))
    for step in sorted(set(steps) - set(expected)):
        verdicts = [r for r, _ in steps[step]]
        rows.append((step, "FAIL" if "FAIL" in verdicts else verdicts[0], "(not an expected step)"))
        if "FAIL" in verdicts:
            ok = False
    if summary is None:
        rows.append(("SUMMARY", "MISSING", "the host never wrote its summary (crash or timeout?)"))
        ok = False
    elif summary[0] != "PASS" and ok:
        rows.append(("SUMMARY", "FAIL", "host summary says FAIL although every step passed: " + summary[1]))
        ok = False
    return ok, rows


def spot_json_ok(path):
    """True when local/range/range.json holds a spot the host can use (x, y and z numbers)."""
    if not path or not os.path.isfile(path):
        return False
    try:
        with open(path, encoding="utf-8") as f:
            spot = json.load(f).get("spot")
        return all(isinstance(spot.get(key), (int, float)) for key in ("x", "y", "z"))
    except (ValueError, AttributeError):
        return False


def preflight(manifest_path, spot_json=None, has_at=False, survey=False):
    """Why a test run must not start on this manifest ("" when it may). A failed or old tools/prepare_range.py leaves a manifest without
    the default spot: the host would then quietly fall through to another candidate and the run could go green at the wrong place."""
    try:
        with open(manifest_path, encoding="utf-8") as f:
            manifest = json.load(f)
    except (OSError, ValueError) as error:
        return "cannot read the range manifest %s: %s" % (manifest_path, error)
    candidates = manifest.get("spot_candidates") or []
    # A survey builds a range on each candidate too, so the same rule applies.
    spawned = {spawn.get("kind") for spawn in manifest.get("spawns") or []}
    # Every range kind melees: without these its enemies approach and never swing (a --no-vm manifest), or swing at the full reach (an old
    # one). A spawn with no kind is refused too. The host checks the same (UOpenWillowRangeSubsystem::ManifestProblem).
    kinds = manifest.get("kinds") or {}
    stale = sorted(str(name) for name in spawned if not (kinds.get(name, {}).get("melee_arrival_by_level") and kinds.get(name, {}).get("melee_ratio_by_level")
                                                         and (kinds.get(name, {}).get("melee_start_range") or 0) > 0))
    if stale:
        return ("the manifest %s lacks melee_arrival_by_level, melee_ratio_by_level or a positive melee_start_range for %s: it is old, written "
                "with --no-vm, or tools/prepare_range.py could not read them. Re-run it without --no-vm and read its message"
                % (manifest_path, ", ".join(stale)))
    if survey:
        return "" if candidates else "the manifest %s has no spot_candidates to survey" % manifest_path
    if has_at or spot_json_ok(spot_json):
        return ""
    index = manifest.get("default_spot_candidate")
    if isinstance(index, int) and 0 <= index < len(candidates) and manifest.get("scout_centre"):
        return ""
    return ("the manifest %s has no default_spot_candidate and scout_centre (default_spot_candidate=%s, %d candidates): it is old, or "
            "tools/prepare_range.py failed. Re-run it and read its message, or pass -At or a local/range/range.json"
            % (manifest_path, index, len(candidates)))


def main(argv=None):
    parser = argparse.ArgumentParser(description="Grade a gun-range test log, or check a manifest before a run (--preflight)")
    parser.add_argument("log", nargs="?")
    parser.add_argument("--survey-log", action="store_true", help="the log is a -Survey run: pass when it built a range on at least one candidate")
    parser.add_argument("--kill-log", action="store_true", help="the log is a tools/play_range.ps1 -KillRow run: pass when each gun fired, killed and its corpse was measured")
    parser.add_argument("--require-psycho", action="store_true", help="a run that exercised no psycho fails (a batch that means to test it)")
    parser.add_argument("--steps", help="comma-separated expected steps (default: all of PR1's)")
    parser.add_argument("--preflight", metavar="MANIFEST", help="check the range manifest before a run instead of grading a log")
    parser.add_argument("--spot-json", help="with --preflight: local/range/range.json (a usable spot there is an explicit choice)")
    parser.add_argument("--has-at", action="store_true", help="with --preflight: the run passes -At")
    parser.add_argument("--survey", action="store_true", help="with --preflight: the run is a -Survey")
    args = parser.parse_args(argv)
    if args.preflight:
        problem = preflight(args.preflight, args.spot_json, args.has_at, args.survey)
        if problem:
            print("range preflight: FAIL: " + problem)
            return 2
        print("range preflight: ok")
        return 0
    if not args.log:
        parser.error("a log to grade, or --preflight MANIFEST")
    with open(args.log, encoding="utf-8", errors="replace") as f:
        text = f.read()
    if args.survey_log:
        print("range survey: %s" % ("PASS" if survey_ok(text) else "FAIL, no candidate was accepted (or it did not finish)"))
        return 0 if survey_ok(text) else 2
    if args.kill_log:
        ok, why = kills_ok(text)
        print("kill row: %s" % ("PASS" if ok else "FAIL, " + why))
        return 0 if ok else 1
    expected = args.steps.split(",") if args.steps else EXPECTED
    ok, rows = grade(*parse(text), expected=expected)
    no_psycho = any(step == "psycho_melee" and verdict == "SKIP" for step, verdict, _ in rows)
    if no_psycho and args.require_psycho:
        ok = False
    for step, verdict, detail in rows:
        print("%-14s %-8s %s" % (step, verdict, detail))
    counts = {v: sum(1 for _, r, _ in rows if r == v) for v in ("PASS", "FAIL", "SKIP", "MISSING")}
    print("range check: %s%s (pass %d, fail %d, skip %d, missing %d)" % ("PASS" if ok else "FAIL", ", no Psycho exercised" if no_psycho else "", counts["PASS"],
          counts["FAIL"], counts["SKIP"], counts["MISSING"]))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
