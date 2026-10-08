#!/usr/bin/env python3
"""Compares the bullymong combat numbers with their closed forms (plan S2 section 0, step S-a/S-d/S-e oracles).

AI-assisted (Claude). The rows are the values docs/verification/COMBAT_GOLDEN_TRACE.md and NATIVE_COMBAT_GROUNDWORK.md 5.1-5.2
record: pawn health per class and level, the per-level base, the melee and grenade arrival values, the applied ratio, and kill
experience (award and credit). The formulas and the class constants below are the public ones from those docs. No measured value
is stored here: the live values are read from the ignored dumps under local/realgame/ (--live), or from a VM JSON (--vm).

    python tools/verify_combat_pawn.py --live                    # formula vs the live dumps (ignored local data)
    python tools/verify_combat_pawn.py --vm run.json             # formula vs a --combat-run output
    python tools/verify_combat_pawn.py --list                    # row ids and expected values
    python tools/verify_combat_pawn.py --self-test               # formulas and PASS/FAIL/MISSING logic on invented inputs

VM JSON schema: {"rows": {<row id>: <number>, ...}}. A kill row takes {"award": <number>, "credit": <number>}. A row that is
absent is reported MISSING and does not fail the run. Only FAIL makes the exit code 1.

Tolerance: relative 1e-3 (|measured - expected| <= 1e-3 x |expected|). Kill credits are integers and get the same rule.
A FAIL means the closed form does not reproduce the measured value; do not widen the tolerance to pass it.
"""
import argparse
import json
import math
import statistics
import sys
from dataclasses import dataclass
from pathlib import Path

REALGAME = Path(__file__).resolve().parents[1] / "local" / "realgame"
TOLERANCE = 1e-3

# Class constants as recorded in docs/verification/COMBAT_GOLDEN_TRACE.md and NATIVE_COMBAT_GROUNDWORK.md (5.1, 5.2).
HEALTH_MULT = {"TargetDummy": 5, "PrimalBeastAdult": 6, "PrimalBeast": 2, "PrimalBeast_Ranged": 4, "PrimalBeastBaby": 0.5}
DAMAGE_MULT = {"PrimalBeastAdult": 5, "PrimalBeast": 1.5, "PrimalBeastBaby": 0.75, "PrimalBeast_Ranged": 3}
XP_MULT = {"TargetDummy": 0.5, "PrimalBeastAdult": 1.25, "PrimalBeast": 0.75, "PrimalBeastBaby": 0.5, "PrimalBeast_Ranged": 1.0}
LOWER_LEVEL_EXP_SCALE = [0.9, 0.7, 0.4, 0.15, 0.05, 0.01]  # index |d| - 1; the last entry also covers larger differences

MELEE, GRENADE = "WillowDmgSource_Melee", "WillowDmgSource_Grenade"


def base_health(level):
    return 50.5 * (1 + 0.02 * level) * 1.13 ** level


def health(mult, level):
    return mult * base_health(level)


def arrival(damage_mult, level):
    return 8 * 1.13 ** level * damage_mult


def applied_ratio(attacker_level):
    return 0.65 * (1 + 0.02 * attacker_level)


def kill_award(xp_mult, victim_level):
    return max(10.0, xp_mult * 10 * (victim_level + 4))


def kill_credit(award, victim_level, player_level):
    d = player_level - victim_level
    if d < 1:
        raise ValueError("only lower-level victims have a table entry in this comparator")
    return math.trunc(round(award * LOWER_LEVEL_EXP_SCALE[min(d, len(LOWER_LEVEL_EXP_SCALE)) - 1], 9))


@dataclass
class Row:
    id: str
    kind: str                      # health, base, arrival, ratio, kill
    cls: str = None
    level: int = None
    player_level: int = None
    source: str = None             # arrival: melee or grenade; kill: "L5b" (live dumps) or "L3" (not read here)


ROWS = [
    Row("health.TargetDummy.L8", "health", "TargetDummy", 8),
    Row("health.PrimalBeastAdult.L7", "health", "PrimalBeastAdult", 7),
    Row("health.PrimalBeast_Ranged.L6", "health", "PrimalBeast_Ranged", 6),
    Row("health.PrimalBeastBaby.L6", "health", "PrimalBeastBaby", 6),
    Row("base.L5", "base", level=5),
    Row("base.L6", "base", level=6),
    Row("base.L7", "base", level=7),
    Row("arrival.PrimalBeastAdult.L5", "arrival", "PrimalBeastAdult", 5, source=MELEE),
    Row("arrival.PrimalBeastAdult.L6", "arrival", "PrimalBeastAdult", 6, source=MELEE),
    Row("arrival.PrimalBeastAdult.L7", "arrival", "PrimalBeastAdult", 7, source=MELEE),
    Row("arrival.PrimalBeast.L6", "arrival", "PrimalBeast", 6, source=MELEE),
    Row("arrival.PrimalBeastBaby.L6", "arrival", "PrimalBeastBaby", 6, source=MELEE),
    Row("arrival.PrimalBeast_Ranged.L5", "arrival", "PrimalBeast_Ranged", 5, source=GRENADE),
    Row("ratio.PrimalBeastAdult.L5", "ratio", "PrimalBeastAdult", 5, source=MELEE),
    Row("ratio.PrimalBeastAdult.L6", "ratio", "PrimalBeastAdult", 6, source=MELEE),
    Row("ratio.PrimalBeastAdult.L7", "ratio", "PrimalBeastAdult", 7, source=MELEE),
    Row("ratio.PrimalBeast.L6", "ratio", "PrimalBeast", 6, source=MELEE),
    Row("ratio.PrimalBeastBaby.L6", "ratio", "PrimalBeastBaby", 6, source=MELEE),
    Row("kill.PrimalBeastBaby.L6.p8", "kill", "PrimalBeastBaby", 6, 8, source="L5b"),
    Row("kill.PrimalBeastAdult.L7.p8", "kill", "PrimalBeastAdult", 7, 8, source="L5b"),
    Row("kill.PrimalBeastBaby.L6.p10.a", "kill", "PrimalBeastBaby", 6, 10, source="L5b"),
    Row("kill.PrimalBeastBaby.L6.p10.b", "kill", "PrimalBeastBaby", 6, 10, source="L5b"),
    Row("kill.PrimalBeast.L6.p10", "kill", "PrimalBeast", 6, 10, source="L5b"),
    Row("kill.PrimalBeastAdult.L5.p8", "kill", "PrimalBeastAdult", 5, 8, source="L3"),
    Row("kill.PrimalBeastBaby.L6.p10.L3", "kill", "PrimalBeastBaby", 6, 10, source="L3"),
]


def expected(row):
    if row.kind == "health":
        return health(HEALTH_MULT[row.cls], row.level)
    if row.kind == "base":
        return base_health(row.level)
    if row.kind == "arrival":
        return arrival(DAMAGE_MULT[row.cls], row.level)
    if row.kind == "ratio":
        return applied_ratio(row.level)
    award = kill_award(XP_MULT[row.cls], row.level)
    return {"award": award, "credit": kill_credit(award, row.level, row.player_level)}


def pawn_class(ai_class):
    marker = "CharClass_"
    return ai_class[ai_class.index(marker) + len(marker):] if marker in ai_class else None


def measure_live(dummy, profiles, summary, hits):
    """Measured values keyed by row id, from the parsed dumps; rows with no live source are absent."""
    pawns = [(pawn_class(dummy["dummy"]["ai_class"]), dummy["allprops"]["ExpLevel"], dummy["dummy"]["hp_max"])]
    pawns += [(pawn_class(p["ai_class"]), p["GetExpLevel"], p["hp_max"]) for p in profiles.values()]
    measured = {}
    for row in ROWS:
        if row.kind == "health":
            hit = [hp for c, lv, hp in pawns if c == row.cls and lv == row.level]
            if hit: measured[row.id] = hit[0]
        elif row.kind == "base":
            hit = [hp / HEALTH_MULT[c] for c, lv, hp in pawns if lv == row.level and c in HEALTH_MULT]
            if hit: measured[row.id] = hit[0]
        elif row.kind in ("arrival", "ratio"):
            dmg = [h for h in hits if h["type"] == row.source and h["mind"] and h["mind"][0] == row.cls and h["mind"][1] == row.level and h["dmg"] > 0]
            if row.kind == "arrival" and dmg:
                measured[row.id] = statistics.median(h["dmg"] for h in dmg)
            if row.kind == "ratio":
                full = [h for h in dmg if h["sh0"] == 0 and h["applied"] > 0 and abs(h["hp0"] - h["hp1"] - h["applied"]) < 1e-2]
                if full: measured[row.id] = statistics.median(h["applied"] / h["dmg"] for h in full)
    pool = list(summary["kills"])
    for row in ROWS:
        if row.kind != "kill" or row.source != "L5b": continue
        match = [k for k in pool if k["class"] == row.cls and k["victim_level"] == row.level and k["player_level"] == row.player_level]
        if match:
            pool.remove(match[0])
            measured[row.id] = {"award": match[0]["award"], "credit": match[0]["credited"]}
    return measured


def measure_vm(path):
    data = json.loads(Path(path).read_text(encoding="utf-8"))
    if "rows" not in data:
        raise SystemExit(f"{path}: no 'rows' object (see the schema in this file's docstring)")
    return data["rows"]


def close(measured, exp):
    return abs(float(measured) - exp) <= TOLERANCE * abs(exp)


def check_row(row, measured):
    """Returns (status, expected, detail): status is PASS, FAIL or MISSING."""
    exp = expected(row)
    if measured is None:
        return "MISSING", exp, ("no live source (the L3 dump is not read by this comparator)" if row.source == "L3" else "not in the measured values")
    if isinstance(exp, dict):
        if not isinstance(measured, dict) or not all(k in measured for k in exp):
            return "MISSING", exp, "kill row needs both award and credit"
        ok = all(close(measured[k], exp[k]) for k in exp)
    else:
        ok = close(measured, exp)
    return ("PASS" if ok else "FAIL"), exp, ""


def fmt(value):
    if isinstance(value, dict):
        return "award " + fmt(value["award"]) + " credit " + fmt(value["credit"])
    if isinstance(value, float):
        return f"{value:.4f}"
    return str(value)


def report(measured):
    counts = {"PASS": 0, "FAIL": 0, "MISSING": 0}
    print(f"tolerance: relative {TOLERANCE:g} (|measured - expected| <= {TOLERANCE:g} x |expected|)")
    for row in ROWS:
        status, exp, detail = check_row(row, measured.get(row.id))
        counts[status] += 1
        got = "" if status == "MISSING" else f"  measured {fmt(measured[row.id])}"
        print(f"{status:<8}{row.id:<34} expected {fmt(exp)}{got}" + (f"  [{detail}]" if detail else ""))
    print(f"{counts['PASS']} PASS, {counts['FAIL']} FAIL, {counts['MISSING']} MISSING of {len(ROWS)} rows")
    return 1 if counts["FAIL"] else 0


def live_measured(directory):
    def load(name):
        return json.loads((directory / name).read_text(encoding="utf-8"))
    return measure_live(load("L5/dummy_pre_allprops.json"), load("L5b/bullymong_profiles_pre.json"),
                        load("L5b/bullymong_summary.json"), load("L5b/player_hits.json"))


def self_test():
    failures = []

    def expect(name, got, want):
        if got != want: failures.append(f"{name}: got {got!r}, want {want!r}")

    expect("base_health(0)", base_health(0), 50.5)
    expect("health(2, 0)", health(2.0, 0), 101.0)
    expect("arrival(2, 1)", round(arrival(2.0, 1), 9), round(16 * 1.13, 9))
    expect("applied_ratio(0)", applied_ratio(0), 0.65)
    expect("applied_ratio(10)", round(applied_ratio(10), 9), round(0.65 * 1.2, 9))
    expect("kill_award floor", kill_award(0.1, 0), 10.0)
    expect("kill_award(1, 0)", kill_award(1.0, 0), 40.0)
    expect("kill_credit d=1", kill_credit(100.0, 3, 4), 90)
    expect("kill_credit d=2 exact", kill_credit(50.0, 6, 8), 35)
    expect("kill_credit d>=6 uses the last entry", kill_credit(100.0, 0, 6), 1)

    dummy_ok = {"dummy": {"ai_class": "GD.Character.CharClass_TargetDummy", "hp_max": health(5, 8)}, "allprops": {"ExpLevel": 8}}
    profiles = {
        "good": {"ai_class": "GD.CharClass_PrimalBeastBaby", "GetExpLevel": 6, "hp_max": health(0.5, 6)},
        "off": {"ai_class": "GD.CharClass_PrimalBeastAdult", "GetExpLevel": 7, "hp_max": health(6, 7) * 1.01},
    }
    kills = [{"class": "PrimalBeastBaby", "victim_level": 6, "player_level": 8, "award": kill_award(0.5, 6), "credited": 35},
             {"class": "PrimalBeastAdult", "victim_level": 7, "player_level": 8, "award": kill_award(1.25, 7), "credited": kill_credit(kill_award(1.25, 7), 7, 8) + 1}]
    hits = [{"type": MELEE, "mind": ["PrimalBeastAdult", 6, "p"], "dmg": arrival(5, 6), "applied": arrival(5, 6) * applied_ratio(6),
             "sh0": 0, "sh1": 0, "hp0": 100.0, "hp1": 100.0 - arrival(5, 6) * applied_ratio(6)},
            {"type": MELEE, "mind": ["PrimalBeastAdult", 6, "p"], "dmg": arrival(5, 6), "applied": 1.0, "sh0": 9.0, "sh1": 8.0, "hp0": 100.0, "hp1": 100.0}]
    live = measure_live(dummy_ok, profiles, {"kills": kills}, hits)
    status = {row.id: check_row(row, live.get(row.id))[0] for row in ROWS}
    expect("live health PASS", status["health.TargetDummy.L8"], "PASS")
    expect("live health baby PASS", status["health.PrimalBeastBaby.L6"], "PASS")
    expect("live health 1% off FAIL", status["health.PrimalBeastAdult.L7"], "FAIL")
    expect("live base.L5 absent MISSING", status["base.L5"], "MISSING")
    expect("live arrival PASS", status["arrival.PrimalBeastAdult.L6"], "PASS")
    expect("live ratio ignores shielded hit", status["ratio.PrimalBeastAdult.L6"], "PASS")
    expect("live kill award+credit PASS", status["kill.PrimalBeastBaby.L6.p8"], "PASS")
    expect("live kill credit off by one FAIL", status["kill.PrimalBeastAdult.L7.p8"], "FAIL")
    expect("L3 kill is MISSING", status["kill.PrimalBeastAdult.L5.p8"], "MISSING")

    vm = {"health.TargetDummy.L8": health(5, 8), "health.PrimalBeastBaby.L6": 1.0, "kill.PrimalBeast.L6.p10": {"award": 75.0, "credit": 11}}
    statuses = {row.id: check_row(row, vm.get(row.id))[0] for row in ROWS}
    expect("vm exact PASS", statuses["health.TargetDummy.L8"], "PASS")
    expect("vm wrong value FAIL", statuses["health.PrimalBeastBaby.L6"], "FAIL")
    expect("vm absent MISSING", statuses["health.PrimalBeastAdult.L7"], "MISSING")
    expect("vm kill both keys PASS", statuses["kill.PrimalBeast.L6.p10"], "PASS")
    expect("vm partial kill MISSING", check_row(next(r for r in ROWS if r.id == "kill.PrimalBeastBaby.L6.p10.b"), {"award": 50.0})[0], "MISSING")
    for failure in failures: print("self-test FAIL: " + failure)
    print("self-test " + ("FAILED" if failures else "passed"))
    return 1 if failures else 0


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--live", action="store_true", help="compare with the ignored live dumps under local/realgame/")
    parser.add_argument("--realgame", default=str(REALGAME), help="directory holding the L5 and L5b dumps")
    parser.add_argument("--vm", metavar="FILE.json", help="compare with a VM JSON (schema in the docstring)")
    parser.add_argument("--list", action="store_true", help="print the row ids and expected values")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test: return self_test()
    if args.list:
        for row in ROWS: print(f"{row.id:<34} {fmt(expected(row))}")
        return 0
    if args.live == bool(args.vm): raise SystemExit("choose exactly one of --live or --vm FILE.json (or --list, --self-test)")
    measured = live_measured(Path(args.realgame)) if args.live else measure_vm(args.vm)
    return report(measured)


if __name__ == "__main__":
    sys.exit(main())
