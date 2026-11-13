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

# The live-read tables live in the prep tool only (OBSERVED_SPEED, OBSERVED_COLLISION); the preflight reads them from there. The folder is
# added by hand because python -I does not put the script's folder on the path.
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import prepare_range  # noqa: E402

# The steps of the driver, in order (OpenWillowRange.cpp): PR1's, then PR2's enemy AI (TickAITest, steps 16-36), then PR3's loot
# on death (TickLootTest, steps 37-44).
EXPECTED = ["teleport", "placement", "body_hit", "crit_hit", "popup_crit", "crit_minds_eye", "crit_trace", "shield_order", "invulnerable",
            "passive", "patrol", "death", "ragdoll", "respawn", "dummy_respawn",
            "notice_proximity", "notice_sight", "no_notice_far", "provoked", "approach", "melee_hit", "melee_interval",
            "melee_miss", "forget", "passive_no_melee", "swing_range", "corpse_limp", "corpse_cap", "psycho_melee", "kind_melee", "leap", "throw",
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
    # cycle: session 2 of 2026-11-12 (medians 1.220 to 1.223 s, 407 gaps, four recordings) and the den read of 2026-11-07 (1.216 to
    # 1.217 s, 9 gaps), widened 0.005 each way like the baby's; AI_MELEE_TARGETING_LIVE 4's median 1.249 (34 gaps) is outside it.
    # stand: the den read (one adult, 181 samples, 190 to 225).
    "bullymong_adult": {"cycle": 1.222, "cycle_band": (1.211, 1.228), "stand": 201.0, "stand_band": (190.0, 225.0)},
    # both from the den read (grab cycle: two babies, 31 gaps, 1.133 to 1.134, widened 0.005 each way for the logs' millisecond rounding;
    # stand: one baby, 347 samples, 106 to 118).
    "bullymong_baby": {"cycle": 1.133, "cycle_band": (1.128, 1.139), "stand": 111.0, "stand_band": (106.0, 118.0)},
    # The cycle only (2026-11-12): stand None means no live stand distance, so the host keeps its stand-in stop and no stand band is graded.
    # Regular: session 2 of 2026-11-12, medians 1.2200 to 1.2227 s per recording, 96 gaps over three, widened 0.005 each way like the adult's
    # (each edge rounded outward to the millisecond, as for all three bullymong cycle bands).
    # Its median call distance sat 70 to 85 uu beyond contact, unexplained, and no stand distance was sampled: the stand stays open.
    "bullymong": {"cycle": 1.222, "cycle_band": (1.215, 1.228), "stand": None, "stand_band": None},
    # Slinger's melee: the same session, median 1.2224 s, 28 gaps, one recording, widened 0.005 each way (1.2174 to 1.2274) and rounded
    # outward to the millisecond. No stand distance sampled.
    "bullymong_slinger": {"cycle": 1.222, "cycle_band": (1.217, 1.228), "stand": None, "stand_band": None},
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
    # A row with no live stand distance (stand_band None) grades the cycle only; the stand numbers are still required and checked above.
    low, high = row["stand_band"] or (-math.inf, math.inf)
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


# ---- Leaps and thrown rocks (2026-11-13) ---------------------------------------------------------------------------------------------
# The note docs/verification/NATIVE_AI_LEAP_THROW.md, one row per kind, in the order of the host's rows (OpenWillowLeapThrow.h,
# OpenWillowFindKindAttacks; tests/range_tools_test.py holds the two to each other number by number). Read from data, script and native
# analysis, UNVERIFIED in the running game. Bands are horizontal distances (uu); "clips" are (clip seconds, release moments) at the play rate.
ATTACKS = {
    # Regular: the random node 1.0 : 0.5 (melee, a leap with no roar and no resource); the baby's leap clips. No throw in its node.
    "bullymong": {"close": 320.0, "weights": (1.0, 0.5, 0.0), "leap": (1024.0, 2048.0, 2000.0, 0.4, 0.0, False, 0.833, 0.8),
                  "throw": None},
    # Adult: 1.0 : 0.25 : 0.25, the leap holds the leap resource (its roar skipped, the hub's decision 4), the throw needs Maya in front.
    "bullymong_adult": {"close": 384.0, "weights": (1.0, 0.25, 0.25), "leap": (1024.0, 2048.0, 2500.0, 0.4, 92.0, True, 0.833, 1.233),
                        "throw": (512.0, 3072.0, 2000.0, 0.3, 480.0, True, True, ((1.733, (1.064,)),))},
    # Baby: no random node; the grab inside 480, else the leap (its backup attack).
    "bullymong_baby": {"close": 480.0, "weights": (0.0, 1.0, 0.0), "leap": (480.0, 2048.0, 2500.0, 0.45, 128.0, False, 0.833, 0.8),
                       "throw": None},
    # Slinger: no leap; the close melee inside 384, else the throw (two one-rock clips and one two-rock clip, equal weights).
    "bullymong_slinger": {"close": 384.0, "weights": (0.0, 0.0, 1.0), "leap": None,
                          "throw": (1536.0, 3072.0, 2000.0, 0.3, 512.0, True, True,
                                    ((1.733, (1.064,)), (1.733, (1.064,)), (2.453, (0.68, 1.66))))},
}
LEAP_MOMENTUM, ROCK_MOMENTUM, KNOCKBACK_MASS = 100000.0, 50000.0, 100.0   # note 3.3 and 4.3; the mass is the host's stand-in
# L-4 (AI_MELEE_TARGETING_LIVE section 4): Attack to Landed 1.08 to 3.48 s over 157 leaps, kinds mixed. The host's Attack moment is the start
# of the launch clip (the hub's decision 7), so its launch clip plus flight must fall inside.
LIVE_ATTACK_TO_LANDED = (1.08, 3.48)
# Maya's capsule radius (standing) plus the rock's touch radius, the host's: a rock comes down within this of its aim point when it touches
# a Maya standing on it; 8 uu more for the frame's step.
ROCK_TOUCH = 42.0 + 20.0
ROCK_MISS_ALLOWANCE = ROCK_TOUCH + 8.0
# A rock touches her capsule before its arc reaches her centre: at most this much earlier (s), for a Maya standing on the aim point.
ROCK_EARLY = 0.1
LEAP_FIELDS = ("kind", "enemy", "forced", "start_distance", "band_min", "band_max", "windup", "launch_clip", "d0", "dz", "d1", "lead", "speed",
               "angle_pct", "lead_cap", "flight_predicted", "flight_measured", "attack_to_landed", "land_error", "hit", "expected", "applied",
               "pools_lost", "knock", "max_frame")
THROW_FIELDS = ("kind", "enemy", "forced", "start_distance", "band_min", "band_max", "release", "release_due", "d0", "dz", "d1", "lead", "speed",
                "angle_pct", "flight_predicted", "flight_measured", "what", "miss", "hit", "expected", "applied", "pools_lost", "knock", "max_frame")
# One per range enemy at its setup (AOpenWillowEnemy::SetupKind): the leap and throw row it runs ("source=none": the plain melee only).
ATTACK_SPAWN_LINE = re.compile(r"OWRANGE enemy (\S+) attacks kind=(\S+)(.*?) source=(\S+)")


def solve_arc(d0, dz, d1, speed, angle_pct):
    """(angle in degrees, horizontal speed, flight time) of the note's solver (section 3.2): the launch angle is the elevation of the line to
    the aim point plus angle_pct of what is left to straight up, the speed is fixed, and the flight is the distance to the final (led) aim
    point over the horizontal speed."""
    elevation = math.atan2(dz, d0)
    angle = elevation + angle_pct * (math.pi / 2 - elevation)
    horizontal = speed * math.cos(angle)
    return math.degrees(angle), horizontal, d1 / horizontal


def _fields(detail, wanted):
    """(fields, numbers, problem): each wanted field exactly once, every one but the text ones a finite number."""
    pairs = KEY_VALUE.findall(detail)
    counts = Counter(key for key, _ in pairs)
    repeated = sorted(key for key, n in counts.items() if n > 1)
    if repeated:
        return None, None, "the line repeats %s (each field must appear once)" % ", ".join(repeated)
    missing = [key for key in wanted if key not in counts]
    if missing:
        return None, None, "the line lacks %s" % ", ".join(missing)
    fields, numbers = dict(pairs), {}
    for key in wanted:
        if key in ("kind", "enemy", "what"):
            continue
        try:
            numbers[key] = float(fields[key])
        except ValueError:
            return None, None, "%s=%s is not a number" % (key, fields[key])
        if not math.isfinite(numbers[key]):
            return None, None, "%s=%s is not a finite number" % (key, fields[key])
    return fields, numbers, ""


def _frame_problem(n):
    if not n["max_frame"] > 0:
        return "max_frame %.3f s is not a frame time (it must be above 0)" % n["max_frame"]
    if n["max_frame"] > MAX_FRAME:
        return "stalled run, repeat it: the step had a %.3f s frame, longer than the %.2f s the timing allowance covers" % (n["max_frame"], MAX_FRAME)
    return ""


def _close(a, b, tolerance):
    return abs(a - b) <= tolerance


def leap_problem(detail):
    """Why a leap PASS line does not follow the note's rule, or "": the row is the note's, it started inside its band, the launch clip and
    the flight lasted what the rule says (each up to one frame late, as the host acts on the first frame after a moment is due), the arc is
    the note's solver, it came down on its aim point, and the landing hit delivered the melee numbers with the 100,000 knock-back."""
    fields, n, problem = _fields(detail, LEAP_FIELDS)
    if problem:
        return problem
    row = ATTACKS.get(fields["kind"])
    if row is None or row["leap"] is None:
        return "kind %s has no leap in the note's rows" % fields["kind"]
    low, high, speed, angle_pct, lead_cap, _, launch, _ = row["leap"]
    for key, want in (("band_min", low), ("band_max", high), ("speed", speed), ("angle_pct", angle_pct), ("lead_cap", lead_cap), ("launch_clip", launch)):
        if not _close(n[key], want, 1e-3):
            return "%s=%g is not the note's %g for %s" % (key, n[key], want, fields["kind"])
    problem = _frame_problem(n)
    if problem:
        return problem
    frame = n["max_frame"] + 0.001
    if n["forced"] != 1:
        return "forced=%g: the step asks for the leap (forced=1)" % n["forced"]
    if not low <= n["start_distance"] <= high:
        return "the leap started %.0f uu from Maya, outside its band %.0f to %.0f" % (n["start_distance"], low, high)
    if not launch - 0.001 <= n["windup"] <= launch + frame:
        return "the launch clip lasted %.3f s, not %.3f s (+ max_frame %.3f)" % (n["windup"], launch, n["max_frame"])
    angle, horizontal, flight = solve_arc(n["d0"], n["dz"], n["d1"], speed, angle_pct)
    if not _close(flight, n["flight_predicted"], 0.002):
        return "flight_predicted %.4f s is not the note's solver's %.4f s (d0 %.1f, dz %.1f, d1 %.1f)" % (n["flight_predicted"], flight, n["d0"], n["dz"], n["d1"])
    if n["lead"] > lead_cap + 0.5 or (lead_cap == 0 and not _close(n["d1"], n["d0"], 0.5)):
        return "lead %.1f uu (d0 %.1f, d1 %.1f) is over the cap %.0f" % (n["lead"], n["d0"], n["d1"], lead_cap)
    if not n["flight_predicted"] - 0.001 <= n["flight_measured"] <= n["flight_predicted"] + frame:
        return "the flight lasted %.4f s, not %.4f s (+ max_frame %.3f)" % (n["flight_measured"], n["flight_predicted"], n["max_frame"])
    if not _close(n["attack_to_landed"], n["windup"] + n["flight_measured"], 0.002):
        return "attack_to_landed %.3f s is not the launch clip plus the flight (%.3f s)" % (n["attack_to_landed"], n["windup"] + n["flight_measured"])
    if not LIVE_ATTACK_TO_LANDED[0] <= n["attack_to_landed"] <= LIVE_ATTACK_TO_LANDED[1]:
        return "attack_to_landed %.3f s is outside L-4's %.2f to %.2f s" % ((n["attack_to_landed"],) + LIVE_ATTACK_TO_LANDED)
    if n["land_error"] > 1.0:
        return "it came down %.1f uu from its aim point (1 uu allowed)" % n["land_error"]
    if n["hit"] != 1 or not n["expected"] > 0:
        return "the landing did not hit Maya standing on its aim point (hit=%g, expected %.2f)" % (n["hit"], n["expected"])
    if not (_close(n["applied"], n["expected"], 0.01) and _close(n["pools_lost"], n["applied"], 0.01)):
        return "applied %.2f and pools_lost %.2f are not the melee numbers' %.2f" % (n["applied"], n["pools_lost"], n["expected"])
    if not _close(n["knock"], LEAP_MOMENTUM / KNOCKBACK_MASS, 1.0):
        return "knock %.0f uu/s is not the momentum %.0f over the mass %.0f" % (n["knock"], LEAP_MOMENTUM, KNOCKBACK_MASS)
    return ""


def throw_problem(detail):
    """Why a throw PASS line does not follow the note's rule, or "": the row is the note's, it started inside its band, the rock left at one
    of the clip's release moments (up to a frame late), the arc is the note's solver, and the rock touched a Maya standing on its aim point
    when its flight time ran out (up to ROCK_EARLY before: her capsule is wider than her centre), with the melee numbers and the 50,000 push."""
    fields, n, problem = _fields(detail, THROW_FIELDS)
    if problem:
        return problem
    row = ATTACKS.get(fields["kind"])
    if row is None or row["throw"] is None:
        return "kind %s has no throw in the note's rows" % fields["kind"]
    low, high, speed, angle_pct, lead_cap, _, _, clips = row["throw"]
    for key, want in (("band_min", low), ("band_max", high), ("speed", speed), ("angle_pct", angle_pct)):
        if not _close(n[key], want, 1e-3):
            return "%s=%g is not the note's %g for %s" % (key, n[key], want, fields["kind"])
    problem = _frame_problem(n)
    if problem:
        return problem
    frame = n["max_frame"] + 0.001
    if n["forced"] != 1:
        return "forced=%g: the step asks for the throw (forced=1)" % n["forced"]
    if not low <= n["start_distance"] <= high:
        return "the throw started %.0f uu from Maya, outside its band %.0f to %.0f" % (n["start_distance"], low, high)
    if not any(_close(n["release_due"], moment, 1e-3) for _, moments in clips for moment in moments):
        return "release_due %.3f s is no release moment of the note's clips" % n["release_due"]
    if not n["release_due"] - 0.001 <= n["release"] <= n["release_due"] + frame:
        return "the rock left %.3f s into the clip, not at %.3f s (+ max_frame %.3f)" % (n["release"], n["release_due"], n["max_frame"])
    angle, horizontal, flight = solve_arc(n["d0"], n["dz"], n["d1"], speed, angle_pct)
    if not _close(flight, n["flight_predicted"], 0.002):
        return "flight_predicted %.4f s is not the note's solver's %.4f s (d0 %.1f, dz %.1f, d1 %.1f)" % (n["flight_predicted"], flight, n["d0"], n["dz"], n["d1"])
    if n["lead"] > lead_cap + 0.5:
        return "lead %.1f uu is over the cap %.0f" % (n["lead"], lead_cap)
    if fields["what"] != "maya" or n["hit"] != 1:
        return "the rock came down on %s (hit=%g), not on Maya standing on its aim point" % (fields["what"], n["hit"])
    if not n["flight_predicted"] - ROCK_EARLY <= n["flight_measured"] <= n["flight_predicted"] + frame:
        return "the rock flew %.4f s, not %.4f s (from %.1f s before to max_frame %.3f after)" % (n["flight_measured"], n["flight_predicted"], ROCK_EARLY, n["max_frame"])
    if n["miss"] > ROCK_MISS_ALLOWANCE:
        return "the rock touched %.1f uu from its aim point (%.0f allowed: Maya's radius and the rock's, plus 8)" % (n["miss"], ROCK_MISS_ALLOWANCE)
    if not n["expected"] > 0 or not (_close(n["applied"], n["expected"], 0.01) and _close(n["pools_lost"], n["applied"], 0.01)):
        return "applied %.2f and pools_lost %.2f are not the melee numbers' %.2f" % (n["applied"], n["pools_lost"], n["expected"])
    if not _close(n["knock"], ROCK_MOMENTUM / KNOCKBACK_MASS, 1.0):
        return "knock %.0f uu/s is not the momentum %.0f over the mass %.0f" % (n["knock"], ROCK_MOMENTUM, KNOCKBACK_MASS)
    return ""


def attack_row_problem(kind, logged):
    """Why an enemy's logged leap and throw row (its spawn line, without kind= and source=) is not the note's, or ""."""
    row = ATTACKS.get(kind)
    if row is None:
        return "kind %s runs leap or throw rows the note does not give" % kind
    fields = dict(KEY_VALUE.findall(logged))
    want = {"close": "%.0f" % row["close"], "weights": "%.2f:%.2f:%.2f" % row["weights"]}
    leap = row["leap"] or (0.0, 0.0, 0.0, 0.0, 0.0, False, 0.0, 0.0)
    want.update({"leap": "%.0f-%.0f" % leap[:2], "leap_speed": "%.0f" % leap[2], "leap_angle_pct": "%.2f" % leap[3], "leap_lead_cap": "%.0f" % leap[4],
                 "leap_resource": "%d" % leap[5], "leap_clips": "%.3f:%.3f" % leap[6:8]})
    throw = row["throw"] or (0.0, 0.0, 0.0, 0.0, 0.0, False, False, ())
    want.update({"throw": "%.0f-%.0f" % throw[:2], "throw_speed": "%.0f" % throw[2], "throw_angle_pct": "%.2f" % throw[3],
                 "throw_lead_cap": "%.0f" % throw[4], "throw_front": "%d" % throw[5], "throw_resource": "%d" % throw[6], "throw_clips": "%d" % len(throw[7])})
    wrong = ["%s=%s (note %s)" % (key, fields.get(key), value) for key, value in want.items() if fields.get(key) != value]
    return ("enemy of kind %s runs %s" % (kind, ", ".join(wrong))) if wrong else ""


def ranged_graded(step, results, spawns, attack_rows):
    """The leap or throw results with the checker's verdicts: each PASS must follow the rule (leap_problem / throw_problem) and come from an
    enemy spawned as that kind with the note's row; every kind spawned with that attack needs exactly one such result. spawns: enemy name ->
    (kind, melee source); attack_rows: enemy name -> (kind, logged row, source)."""
    has = "leap" if step == "leap" else "throw"
    expected = sorted({kind for kind, _, source in attack_rows.values() if source == "note" and (ATTACKS.get(kind) or {}).get(has)})
    graded, passed = [], Counter()
    for kind, logged, source in sorted(set(attack_rows.values())):
        if source == "note":
            problem = attack_row_problem(kind, logged)
            if problem:
                graded.append(("FAIL", "(checker: %s)" % problem))
    for verdict, detail in results:
        if verdict != "PASS":
            graded.append((verdict, detail))
            continue
        fields = dict(KEY_VALUE.findall(detail))
        problem = leap_problem(detail) if step == "leap" else throw_problem(detail)
        if not problem:
            spawned = attack_rows.get(fields.get("enemy", ""))
            if spawned is None or spawned[0] != fields["kind"] or spawned[2] != "note":
                problem = "enemy %s was not spawned as %s with the note's row (its spawn record: %s)" % (fields.get("enemy"), fields["kind"], spawned)
        if problem:
            graded.append(("FAIL", detail + " (checker: " + problem + ")"))
            continue
        passed[fields["kind"]] += 1
        graded.append(("PASS", detail))
    if not expected:
        graded.append(("FAIL", "(checker: no enemy was spawned with a %s: no 'OWRANGE enemy ... attacks ... source=note' line for such a kind)" % has))
    for kind in expected:
        if passed[kind] != 1:
            graded.append(("FAIL", "(checker: %d passing results for spawned kind %s, exactly 1 wanted)" % (passed[kind], kind)))
    return graded


def fight_counts(text):
    """What the run's free fights did, for the report (not graded): leaps started and landed, landings that hit, rocks released and those that
    hit Maya, and the attack picks by kind."""
    picks = Counter(re.findall(r"OWRANGE attack_pick enemy=\S+ kind=(\S+) pick=(\w+)", text))
    return {"leap_starts": len(re.findall(r"OWRANGE leap_start ", text)), "leap_lands": len(re.findall(r"OWRANGE leap_land ", text)),
            "leap_hits": len(re.findall(r"OWRANGE leap_land .*? hit=1 ", text)), "rocks": len(re.findall(r"OWRANGE rock_release .*? release=", text)),
            "rock_hits": len(re.findall(r"OWRANGE rock_impact .*? what=maya .*? hit=1 ", text)), "picks": dict(picks)}


class ParsedSteps(dict):
    """Step name -> [(verdict, detail)], plus .spawns: enemy name -> (kind, melee source) and .attacks: enemy name -> (kind, leap and throw
    row, source) from the spawn lines of the same log, and .bare: how many stock pawns logged MISSING_ATTACHMENTS."""
    spawns = None
    attacks = None
    bare = 0


# The host's line for a stock pawn entry with no attachments list at all (AOpenWillowCombatTarget::AttachWornMeshes): old or broken
# prep data, so the pawn runs bare. Any such line fails the run, whatever the steps say. An empty list is a valid answer and logs
# "0 of 0 attachments" instead, which passes. tools/check_slice_gate.py reads the same text.
MISSING_ATTACHMENTS = "OpenWillow target: no attachments list on the stock pawn entry"
MISSING_ATTACHMENTS_WHY = ("%d stock pawn(s) logged no attachments list (old or broken prep data): re-run " + prepare_range.ATTACH_STEP)


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
    steps.attacks = {}
    for line in text.splitlines():
        if MISSING_ATTACHMENTS in line:
            steps.bare += 1
            continue
        m = SPAWN_LINE.search(line)
        if m:
            steps.spawns[m.group(1)] = (m.group(2), m.group(3))
            continue
        m = ATTACK_SPAWN_LINE.search(line)
        if m:
            steps.attacks[m.group(1)] = (m.group(2), m.group(3), m.group(4))
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
        if step in ("leap", "throw"):
            # Each PASS line must follow the note's rule and come from an enemy spawned as that kind with the note's row, one per spawned
            # kind with that attack; the logged rows themselves must be the note's.
            results = ranged_graded(step, results, getattr(steps, "spawns", None) or {}, getattr(steps, "attacks", None) or {})
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
    if getattr(steps, "bare", 0):
        rows.append(("attachments", "FAIL", MISSING_ATTACHMENTS_WHY % steps.bare))
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


def live_read_problem(manifest_path, kinds):
    """Why a manifest predates the live-read constants of 2026-11-12, or "". A kind of prepare_range.OBSERVED_SPEED needs its speed_source
    and those numbers, and one of OBSERVED_COLLISION (when the manifest has it) its collision_source and that cylinder; else the run would
    test the old stand-ins with no warning."""
    stale = []
    for name, (run, walk) in sorted(prepare_range.OBSERVED_SPEED.items()):
        kind = kinds.get(name)
        if kind is not None and not (kind.get("speed_source") and kind.get("run_speed") == run and kind.get("walk_speed") == walk):
            stale.append("%s speed" % name)
    for name, (radius, height) in sorted(prepare_range.OBSERVED_COLLISION.items()):
        kind = kinds.get(name)
        if kind is not None and not (kind.get("collision_source") and kind.get("collision_radius") == radius
                                     and kind.get("collision_height") == height):
            stale.append("%s collision" % name)
    if not stale:
        return ""
    return ("the manifest %s predates the live-read constants of 2026-11-12 (stale: %s): re-run python tools/prepare_range.py"
            % (manifest_path, ", ".join(stale)))


def attachments_problem(manifest_path, kinds, spawned):
    """Why a spawned kind's enemies would run without their worn pieces, or "". prepare_range writes attachments_expected per kind, from
    the import's own read of the pawn (identity.json); a kind on engine shapes (no mesh) wears nothing, and a pawn that wears nothing has 0,
    so neither is refused. A manifest without the field predates this check (2026-11-12)."""
    old, short = [], []
    for name in sorted(str(n) for n in spawned):
        kind = kinds.get(name) or {}
        if not kind.get("mesh"):
            continue
        expected = kind.get("attachments_expected")
        if isinstance(expected, bool) or not isinstance(expected, int) or expected < 0:
            old.append(name)
            continue
        have = len(kind.get("attachments") or [])
        if have != expected:
            short.append("%s %d of %d" % (name, have, expected))
    if old:
        return ("the manifest %s predates the worn-attachment check (no attachments_expected for %s): re-run python tools/prepare_range.py"
                % (manifest_path, ", ".join(old)))
    if short:
        return ("the manifest %s has enemies without their worn attachments (%s pieces): local/phaselock/target/host_target.json lost its "
                "placements. Re-run %s" % (manifest_path, ", ".join(short), prepare_range.ATTACH_STEP))
    return ""


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
    problem = live_read_problem(manifest_path, kinds) or attachments_problem(manifest_path, kinds, spawned)
    if problem:
        return problem
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
    bare = text.count(MISSING_ATTACHMENTS)
    if args.survey_log:
        if bare:
            print("range survey: FAIL, " + MISSING_ATTACHMENTS_WHY % bare)
            return 2
        print("range survey: %s" % ("PASS" if survey_ok(text) else "FAIL, no candidate was accepted (or it did not finish)"))
        return 0 if survey_ok(text) else 2
    if args.kill_log:
        ok, why = kills_ok(text)
        if bare:
            ok, why = False, MISSING_ATTACHMENTS_WHY % bare
        print("kill row: %s" % ("PASS" if ok else "FAIL, " + why))
        return 0 if ok else 1
    expected = args.steps.split(",") if args.steps else EXPECTED
    ok, rows = grade(*parse(text), expected=expected)
    no_psycho = any(step == "psycho_melee" and verdict == "SKIP" for step, verdict, _ in rows)
    if no_psycho and args.require_psycho:
        ok = False
    for step, verdict, detail in rows:
        print("%-14s %-8s %s" % (step, verdict, detail))
    counts = fight_counts(text)
    print("leaps and rocks in this log (not graded): %d leaps started, %d landed, %d landing hits; %d rocks released, %d hit Maya; picks %s"
          % (counts["leap_starts"], counts["leap_lands"], counts["leap_hits"], counts["rocks"], counts["rock_hits"],
             ", ".join("%s %s %d" % (kind, pick, n) for (kind, pick), n in sorted(counts["picks"].items())) or "none"))
    counts = {v: sum(1 for _, r, _ in rows if r == v) for v in ("PASS", "FAIL", "SKIP", "MISSING")}
    print("range check: %s%s (pass %d, fail %d, skip %d, missing %d)" % ("PASS" if ok else "FAIL", ", no Psycho exercised" if no_psycho else "", counts["PASS"],
          counts["FAIL"], counts["SKIP"], counts["MISSING"]))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
