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
import re
import sys

# The steps of the driver, in order (OpenWillowRange.cpp): PR1's, then PR2's enemy AI (TickAITest, steps 16-36), then PR3's loot
# on death (TickLootTest, steps 37-44).
EXPECTED = ["teleport", "placement", "body_hit", "crit_hit", "crit_trace", "shield_order", "invulnerable", "passive",
            "patrol", "death", "ragdoll", "respawn", "popup_crit", "dummy_respawn",
            "notice_proximity", "notice_sight", "no_notice_far", "provoked", "approach", "melee_hit", "melee_interval",
            "melee_miss", "forget", "passive_no_melee",
            "loot_near_corpse", "loot_roll", "loot_money_pickup", "loot_weapon_item", "loot_off", "loot_seeded_roll"]
# Steps that may be SKIP when their data is absent: no imported stock mesh / physics asset (ragdoll), and no weapon generator
# manifest (loot_weapon_item: no gun is rolled, the money checks still decide).
MAY_SKIP = {"ragdoll", "loot_weapon_item"}

LINE = re.compile(r"OWRANGETEST step=(\S+) result=(PASS|FAIL|SKIP)(.*)")
SUMMARY = re.compile(r"OWRANGETEST SUMMARY result=(PASS|FAIL)(.*)")


def parse(text):
    steps, summary = {}, None
    for line in text.splitlines():
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
        verdicts = [r for r, _ in results]
        verdict = "FAIL" if "FAIL" in verdicts else ("SKIP" if "SKIP" in verdicts else "PASS")
        detail = next((d for r, d in results if r == verdict), "")
        if verdict == "FAIL" or (verdict == "SKIP" and step not in MAY_SKIP):
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


def main(argv=None):
    parser = argparse.ArgumentParser(description="Grade a gun-range test log")
    parser.add_argument("log")
    parser.add_argument("--steps", help="comma-separated expected steps (default: all of PR1's)")
    args = parser.parse_args(argv)
    with open(args.log, encoding="utf-8", errors="replace") as f:
        text = f.read()
    expected = args.steps.split(",") if args.steps else EXPECTED
    ok, rows = grade(*parse(text), expected=expected)
    for step, verdict, detail in rows:
        print("%-14s %-8s %s" % (step, verdict, detail))
    counts = {v: sum(1 for _, r, _ in rows if r == v) for v in ("PASS", "FAIL", "SKIP", "MISSING")}
    print("range check: %s (pass %d, fail %d, skip %d, missing %d)" % ("PASS" if ok else "FAIL", counts["PASS"],
          counts["FAIL"], counts["SKIP"], counts["MISSING"]))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
