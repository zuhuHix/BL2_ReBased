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

The down rows (plan step S-f, docs/verification/COMBAT_DOWN_RESPAWN.md) are about the stock injured state running on the VM, and are only measured by --vm of the six-injury run
whose step list is in that document: bleed-out total and its successive-injury counter per injury (12 s, 12 s, 8 s for the counter read 0, 1, 2 after the injury began, as
lane L5b saw them), the overshoot of the script's timer past the total (a frame), the length of the death camera (5.76 s), the health share after a second wind (0.25) and after a respawn (1.0), and the respawn fee
(0.07 of the cash, rounded half up, at most the cash; 190, 177, 165 on 2,715, 2,534, 2,357 as lane L3 saw them). --live has no source for them (MISSING).

The loot rows (plan step S-g, docs/verification/COMBAT_LOOT.md) are about the death roll on the VM, measured by --vm of the loot run whose step list is in that document (it may be a
second JSON: --vm takes several files and merges their rows). Loot is random, so most rows are distribution comparisons: the pool chances against the numbers in the data (the constants
below, public values of the game's item pool list for a standard enemy), the empirical rate of a pool being tried over hundreds of seeded kills against that chance (four binomial
standard deviations, plus one for the rare pools), and the live flags the hook saw (a pool tried at item stage 6 that returned false or true, the item stage of a baby and of an adult, a
vehicle skin that is not dropped). The `loot.rule.*` rows are the VM's own readings with no live row (marked UNVERIFIED in the document). A row marked OPEN is a live observation the VM does
not reproduce yet; it is reported, never counted as a pass, and does not fail the run.

VM JSON schema: {"rows": {<row id>: <number>, ...}}. A kill row takes {"award": <number>, "credit": <number>}; a fee row {"cash": <number>, "fee": <number>}. A row that is
absent is reported MISSING and does not fail the run. Only FAIL makes the exit code 1.

--live reads the hits from local/realgame/L6/q7_rows.json when it exists (it carries the Defense Boost count per hit), else from
L5b/player_hits.json with no boost filter. The ratio rows use unboosted hits only; the ratio_boost<N> rows check the factor 1/(1+0.5 N).

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

# The standard enemy pool list's chances for one player at full health, as the data evaluates them (GD_Itempools.ListDefs.StandardEnemyGunsAndGear; the drop odds attributes, one player):
# the values the VM must read from the packages, and against which the empirical rate of "tried" is compared. The ammo pools' 0.2 is the AmmoDrops_PerPlayer share (1.0 x 0.2) and the
# emergency pool's constant; the health pool's 0.03 is its above-threshold weight (the player's health is at or above 40 percent).
LOOT_CHANCE = {"Pool_GunsAndGear": 0.085, "Pool_VehicleSkins_All": 0.05, "Pool_Money_1or2": 0.25, "Pool_Eridium_Bar": 0.0015, "Pool_Eridium_Stick": 0.008,
               "Pool_Health_All": 0.03, "Pool_Ammo_All_NeedOnly": 0.2, "Pool_Ammo_All_Emergency": 0.2}
LOOT_TAGS = ("adult", "baby")                    # the loot batches of the run: an adult at game stage 6 and a baby at stage 6
HEALTH_CHANCE_AT_20_PERCENT = 0.175              # the stand-in's reading of the health weight at 20 percent health: 0.25 + (0.2 / 0.4) x (0.1 - 0.25), UNVERIFIED

# The player's down state as docs/verification/COMBAT_GOLDEN_TRACE.md section D and the L5b question-6 table record it (live values of lanes L3 and L5b).
BLEED_BASE = 12.0            # TotalBleedoutTime of an injury with no successive reduction (Maya, one player)
CAMERA_LENGTH = 5.76         # s from the dead state to the end of the death camera (live: 5.761, 5.765, 5.762)
CAMERA_TOLERANCE = 0.05      # s: one frame of the run plus the live spread
BLEED_OVERSHOOT = 0.05       # s: how far the script's timer may be past the total in the frame that ends the bleed-out (the live bleed-outs were 12.00 to 12.04 s)
SECOND_WIND_HEALTH = 0.25    # share of maximum health after a second wind (136.97 = 0.25 x 547.874)
RESPAWN_HEALTH = 1.0
FEE_RATE = 0.07


def bleed_total(successive):
    """The observed bleed-out for the counter `successive` as read after the injury began (live: 0, 1, 2 gave 12, 12, 8): total x (1 - (N - 1) / 3), no cut below N = 1."""
    return BLEED_BASE * (1 - max(successive - 1, 0) / 3)


def respawn_fee(cash):
    return min(cash, math.floor(FEE_RATE * cash + 0.5))


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
    boosts: int = 0               # ratio: Defense Boost count the hit must carry (0 = unboosted)
    value: float = None           # down rows: the expected number (down_count, down_total, down_share)
    cash: int = None              # down_fee rows: the cash the scenario gives the player before the fee (None: not fixed by the scenario)
    pool: str = None              # loot rows: the item pool; tag: the loot batch; stage: the item game stage of a rollpool step
    tag: str = None
    stage: int = None
    flag: str = None              # loot_flag: "all" (every call returned true) or "none"; loot_destroyed: "none" or "all" dropped
    qty: int = None               # loot_qty: the number of items one call of the money pool made
    open: bool = False            # the live observation is not reproduced yet: a mismatch is OPEN, not FAIL


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
    Row("ratio_boost1.PrimalBeastAdult.L5", "ratio", "PrimalBeastAdult", 5, source=MELEE, boosts=1),
    Row("ratio_boost1.PrimalBeastAdult.L6", "ratio", "PrimalBeastAdult", 6, source=MELEE, boosts=1),
    Row("ratio_boost2.PrimalBeastAdult.L6", "ratio", "PrimalBeastAdult", 6, source=MELEE, boosts=2),
    Row("ratio_boost1.PrimalBeast_Ranged.L5", "ratio", "PrimalBeast_Ranged", 5, source=GRENADE, boosts=1),
    Row("kill.PrimalBeastBaby.L6.p8", "kill", "PrimalBeastBaby", 6, 8, source="L5b"),
    Row("kill.PrimalBeastAdult.L7.p8", "kill", "PrimalBeastAdult", 7, 8, source="L5b"),
    Row("kill.PrimalBeastBaby.L6.p10.a", "kill", "PrimalBeastBaby", 6, 10, source="L5b"),
    Row("kill.PrimalBeastBaby.L6.p10.b", "kill", "PrimalBeastBaby", 6, 10, source="L5b"),
    Row("kill.PrimalBeast.L6.p10", "kill", "PrimalBeast", 6, 10, source="L5b"),
    Row("kill.PrimalBeastAdult.L5.p8", "kill", "PrimalBeastAdult", 5, 8, source="L3"),
    Row("kill.PrimalBeastBaby.L6.p10.L3", "kill", "PrimalBeastBaby", 6, 10, source="L3"),
]

# The six injuries of the S-f run (COMBAT_DOWN_RESPAWN.md): 1 to 3 are first injuries ending in a respawn (cash 2,715, 2,534, 2,357), 4 to 6 follow each other within the 30 s
# window with a second wind between them, so the counter read after each is 0, 1, 2 and the third bleeds out in 8 s.
SUCCESSIVE = {1: 0, 2: 0, 3: 0, 4: 0, 5: 1, 6: 2}
for _k, _n in SUCCESSIVE.items():
    ROWS.append(Row(f"down.successive.{_k}", "down_count", value=_n))
    ROWS.append(Row(f"down.total.{_k}", "down_total", value=bleed_total(_n)))
for _k in (1, 2, 3, 6):
    ROWS.append(Row(f"down.bleed_over.{_k}", "down_over"))
    ROWS.append(Row(f"down.camera.{_k}", "down_camera"))
    ROWS.append(Row(f"down.respawn_health.{_k}", "down_share", value=RESPAWN_HEALTH))
    ROWS.append(Row(f"down.fee.{_k}", "down_fee", cash={1: 2715, 2: 2534, 3: 2357}.get(_k)))
for _k in (4, 5):
    ROWS.append(Row(f"down.second_wind_health.{_k}", "down_share", value=SECOND_WIND_HEALTH))


# The loot rows. Live: COMBAT_GOLDEN_TRACE.md section A step 7 (lane L3) and the bullymong paragraph (lane L5b): the flags of the pools tried at item stage 6 or 7, the item stage of a baby (6)
# and of an adult (4 and 7 were seen), the items made by the money pool, the vehicle skin that got no pickup.
for _pool in LOOT_CHANCE:
    ROWS.append(Row(f"loot.chance.{_pool}", "loot_chance", pool=_pool, tag="adult"))
    for _tag in LOOT_TAGS:
        ROWS.append(Row(f"loot.rate.{_tag}.{_pool}", "loot_rate", pool=_pool, tag=_tag))
for _pool, _stage, _flag in (("Pool_Eridium_Stick", 6, "none"), ("Pool_Ammo_All_Emergency", 6, "none"), ("Pool_Ammo_All_NeedOnly", 6, "none"), ("Pool_Health_All", 6, "all"),
                             ("Pool_Money_1or2", 6, "all"), ("Pool_Money_1or2", 7, "all"), ("Pool_GunsAndGear", 6, "all"), ("Pool_VehicleSkins_All", 6, "all")):
    ROWS.append(Row(f"loot.live.{_pool}.stage{_stage}", "loot_flag", pool=_pool, stage=_stage, flag=_flag))
ROWS.append(Row("loot.rule.Pool_Eridium_Stick.stage7", "loot_flag", pool="Pool_Eridium_Stick", stage=7, flag="all"))        # the minimum stage 7 of the data, no live row
for _pool, _flag in (("Pool_VehicleSkins_All", "none"), ("Pool_Health_All", "all"), ("Pool_Money_1or2", "all")):
    ROWS.append(Row(f"loot.live.drop.{_pool}", "loot_destroyed", pool=_pool, tag="adult", flag=_flag))
ROWS.append(Row("loot.live.stage.baby", "loot_stage", tag="baby"))
ROWS.append(Row("loot.live.stage.adult", "loot_stage", tag="adult"))
ROWS.append(Row("loot.live.level.adult", "loot_level", tag="adult"))
ROWS.append(Row("loot.live.money.baby.qty1", "loot_qty", tag="baby", qty=1))
ROWS.append(Row("loot.live.money.adult.qty3", "loot_qty", tag="adult", qty=3, open=True))     # live: 3 items from Pool_Money_1or2 on an adult; the VM's quantity is 1 or 2
ROWS.append(Row("loot.rule.money.qty2_share", "loot_qty_share", tag="adult"))
ROWS.append(Row("loot.rule.health.chance_at_20pct", "loot_health_low", pool="Pool_Health_All", tag="low"))


def lootkey(tag, *names):
    return ".".join(("loot", "batch", tag) + names)


def measure_loot(rows):
    """The measured value of every loot row whose VM rows are present (the VM JSON keeps flat numbers; the comparisons need two or three of them)."""
    measured = {}

    for row in ROWS:
        if row.kind == "loot_chance":
            value = rows.get(lootkey(row.tag, row.pool, "chance"))
            if value is not None: measured[row.id] = value
        elif row.kind == "loot_rate":
            n, k = rows.get(lootkey(row.tag, "n")), rows.get(lootkey(row.tag, row.pool, "tried"))
            if n is not None and k is not None: measured[row.id] = {"n": n, "k": k}
        elif row.kind == "loot_flag":
            base = f"loot.roll.{row.pool}.stage{row.stage}"
            n, true = rows.get(base + ".n"), rows.get(base + ".true")
            if n is not None and true is not None: measured[row.id] = {"n": n, "true": true}
        elif row.kind == "loot_destroyed":
            items, dropped = rows.get(lootkey(row.tag, row.pool, "items")), rows.get(lootkey(row.tag, row.pool, "dropped"))
            if items is not None and dropped is not None: measured[row.id] = {"items": items, "dropped": dropped}
        elif row.kind == "loot_stage":
            stages = {int(k.rsplit(".", 1)[1]): v for k, v in rows.items() if k.startswith(lootkey(row.tag, "stage") + ".") and k.rsplit(".", 1)[1].isdigit()}
            n = rows.get(lootkey(row.tag, "n"))
            if stages and n is not None: measured[row.id] = {"n": n, "stages": stages}
        elif row.kind == "loot_level":
            mismatch, n = rows.get(lootkey(row.tag, "level_mismatch")), rows.get(lootkey(row.tag, "n"))
            if mismatch is not None and n is not None: measured[row.id] = {"n": n, "mismatch": mismatch}
        elif row.kind in ("loot_qty", "loot_qty_share"):
            counts = {int(k.rsplit(".", 1)[1]): v for k, v in rows.items() if k.startswith(lootkey(row.tag, "Pool_Money_1or2", "qty") + ".")}
            tried = rows.get(lootkey(row.tag, "Pool_Money_1or2", "tried"))
            if tried is not None: measured[row.id] = {"tried": tried, "counts": counts}
        elif row.kind == "loot_health_low":
            value = rows.get(lootkey(row.tag, row.pool, "chance"))
            if value is not None: measured[row.id] = value
    return measured


def loot_expected(row):
    if row.kind in ("loot_chance", "loot_rate"): return LOOT_CHANCE[row.pool]
    return {"loot_flag": f"{row.flag} of the calls return true", "loot_destroyed": f"{row.flag} of the items dropped",
            "loot_stage": "every roll at item stage 6" if row.tag == "baby" else "stages 4 to 7, a quarter each", "loot_level": "every item's level is its roll stage",
            "loot_qty": f"{row.qty} items from one call reachable", "loot_qty_share": "2 items in a quarter of the calls, 1 in the rest",
            "loot_health_low": HEALTH_CHANCE_AT_20_PERCENT}[row.kind]


def binomial_ok(n, k, p):
    """k successes of n at probability p: within four standard deviations plus one (the rare pools have fewer than one expected)."""
    return abs(k - n * p) <= 4 * math.sqrt(n * p * (1 - p)) + 1


def check_loot(row, measured):
    """Returns (status, expected, detail) for a loot row; `measured` is what measure_loot made."""
    if row.kind == "loot_chance":
        exp = LOOT_CHANCE[row.pool]
        return ("PASS" if close(measured, exp) else "FAIL"), exp, ""
    if row.kind == "loot_rate":
        exp = LOOT_CHANCE[row.pool]
        n, k = measured["n"], measured["k"]
        return ("PASS" if binomial_ok(n, k, exp) else "FAIL"), f"{exp:g} x {n:g} = {exp * n:.1f} +- {4 * math.sqrt(n * exp * (1 - exp)) + 1:.1f} tried", f"tried {k:g}"
    if row.kind == "loot_flag":
        n, true = measured["n"], measured["true"]
        ok = n > 0 and true == (n if row.flag == "all" else 0)
        return ("PASS" if ok else "FAIL"), f"{row.flag} of {n:g} calls return true", f"true {true:g}"
    if row.kind == "loot_destroyed":
        items, dropped = measured["items"], measured["dropped"]
        ok = items > 0 and dropped == (items if row.flag == "all" else 0)
        return ("PASS" if ok else "FAIL"), f"{row.flag} of the items dropped", f"items {items:g}, dropped {dropped:g}"
    if row.kind == "loot_stage":
        n, stages = measured["n"], measured["stages"]
        if row.tag == "baby":                       # no offset: the item stage is the game stage (6 in the run)
            return ("PASS" if set(stages) == {6} else "FAIL"), "every roll at item stage 6", f"stages {sorted(stages)}"
        ok = min(stages) == 4 and max(stages) == 7 and all(binomial_ok(n, stages.get(s, 0), 0.25) for s in (4, 5, 6, 7))      # offset -2 to +1 on stage 6, each end seen live
        return ("PASS" if ok else "FAIL"), "stages 4 to 7, a quarter each", f"stages {dict(sorted(stages.items()))}"
    if row.kind == "loot_level":
        return ("PASS" if measured["n"] > 0 and measured["mismatch"] == 0 else "FAIL"), "every item's level is its roll stage", f"mismatches {measured['mismatch']:g}"
    if row.kind == "loot_qty":
        reached = measured["counts"].get(row.qty, 0)
        if reached > 0: return "PASS", f"{row.qty} items from one call reachable", f"{reached:g} of {measured['tried']:g} tried"
        return ("OPEN" if row.open else "FAIL"), f"{row.qty} items from one call reachable", f"quantities seen {sorted(measured['counts'])}"
    if row.kind == "loot_qty_share":
        tried, counts = measured["tried"], measured["counts"]
        return ("PASS" if tried > 0 and binomial_ok(tried, counts.get(2, 0), 0.25) and set(counts) <= {1, 2} else "FAIL"), "2 items in a quarter of the calls, 1 in the rest", f"counts {dict(sorted(counts.items()))}"
    if row.kind == "loot_health_low":
        return ("PASS" if close(measured, HEALTH_CHANCE_AT_20_PERCENT) else "FAIL"), HEALTH_CHANCE_AT_20_PERCENT, ""
    raise ValueError(row.kind)


def expected(row):
    if row.kind == "health":
        return health(HEALTH_MULT[row.cls], row.level)
    if row.kind == "base":
        return base_health(row.level)
    if row.kind == "arrival":
        return arrival(DAMAGE_MULT[row.cls], row.level)
    if row.kind == "ratio":
        return applied_ratio(row.level) / (1 + 0.5 * row.boosts)
    if row.kind.startswith("loot"):
        return loot_expected(row)
    if row.kind in ("down_count", "down_total", "down_share"):
        return row.value
    if row.kind == "down_over":
        return 0.0
    if row.kind == "down_camera":
        return CAMERA_LENGTH
    if row.kind == "down_fee":
        return {"cash": row.cash, "fee": respawn_fee(row.cash)} if row.cash is not None else {"fee": "0.07 x cash, rounded half up, at most the cash"}
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
                full = [h for h in dmg if h.get("boosts", 0) == row.boosts and h["sh0"] == 0 and h["applied"] > 0 and abs(h["hp0"] - h["hp1"] - h["applied"]) < 1e-2]
                if full: measured[row.id] = statistics.median(h["applied"] / h["dmg"] for h in full)
    pool = list(summary["kills"])
    for row in ROWS:
        if row.kind != "kill" or row.source != "L5b": continue
        match = [k for k in pool if k["class"] == row.cls and k["victim_level"] == row.level and k["player_level"] == row.player_level]
        if match:
            pool.remove(match[0])
            measured[row.id] = {"award": match[0]["award"], "credit": match[0]["credited"]}
    return measured


def measure_vm(paths):
    rows = {}
    for path in ([paths] if isinstance(paths, str) else paths):
        data = json.loads(Path(path).read_text(encoding="utf-8"))
        if "rows" not in data:
            raise SystemExit(f"{path}: no 'rows' object (see the schema in this file's docstring)")
        rows.update(data["rows"])
    return {**rows, **measure_loot(rows)}


def close(measured, exp):
    return abs(float(measured) - exp) <= TOLERANCE * abs(exp)


def missing_reason(row):
    if row.kind.startswith("loot"):
        return "a VM row of the S-g loot run; no live dump source"
    if row.kind.startswith("down"):
        return "a VM row of the S-f run; no live dump source"
    if row.source == "L3":
        return "no live source (the L3 dump is not read by this comparator)"
    if row.kind == "ratio":
        return "no unboosted hit in the dump" if row.boosts == 0 else f"no {row.boosts}-boost hit from this source in the dump"
    return "not in the measured values"


def check_row(row, measured):
    """Returns (status, expected, detail): status is PASS, FAIL or MISSING."""
    exp = expected(row)
    if measured is None:
        return "MISSING", exp, missing_reason(row)
    if row.kind.startswith("loot"):
        return check_loot(row, measured)
    if row.kind == "down_fee":
        if not isinstance(measured, dict) or "cash" not in measured or "fee" not in measured:
            return "MISSING", exp, "fee row needs both cash and fee"
        if row.cash is None:
            exp = {"cash": measured["cash"], "fee": respawn_fee(measured["cash"])}
        ok = measured["cash"] == exp["cash"] and measured["fee"] == exp["fee"]
        return ("PASS" if ok else "FAIL"), exp, ""
    if row.kind == "down_over":
        return ("PASS" if -1e-6 <= float(measured) <= BLEED_OVERSHOOT else "FAIL"), f"0 to {BLEED_OVERSHOOT:g} s", ""
    if row.kind == "down_count":
        return ("PASS" if float(measured) == exp else "FAIL"), exp, ""
    if row.kind == "down_camera":
        return ("PASS" if abs(float(measured) - CAMERA_LENGTH) <= CAMERA_TOLERANCE else "FAIL"), f"{CAMERA_LENGTH:g} +- {CAMERA_TOLERANCE:g} s", ""
    if isinstance(exp, dict):
        if not isinstance(measured, dict) or not all(k in measured for k in exp):
            return "MISSING", exp, "kill row needs both award and credit"
        ok = all(close(measured[k], exp[k]) for k in exp)
    else:
        ok = close(measured, exp)
    return ("PASS" if ok else "FAIL"), exp, ""


def fmt(value):
    if isinstance(value, dict) and "fee" in value:
        return (f"cash {value['cash']} " if "cash" in value else "") + f"fee {value['fee']}"
    if isinstance(value, dict):
        return "award " + fmt(value["award"]) + " credit " + fmt(value["credit"])
    if isinstance(value, float):
        return f"{value:.4f}"
    return str(value)


def report(measured):
    counts = {"PASS": 0, "FAIL": 0, "MISSING": 0, "OPEN": 0}
    # The loot rows belong to the loot run (COMBAT_LOOT.md, the rollpool and lootbatch steps): a source without their rows (--live, or the S-a to S-f run) does not list them.
    rows = [row for row in ROWS if not row.kind.startswith("loot") or any(key.startswith(("loot.batch.", "loot.roll.")) for key in measured)]
    print(f"tolerance: relative {TOLERANCE:g} (|measured - expected| <= {TOLERANCE:g} x |expected|)")
    for row in rows:
        status, exp, detail = check_row(row, measured.get(row.id))
        counts[status] += 1
        got = "" if status == "MISSING" or row.kind.startswith("loot") else f"  measured {fmt(measured[row.id])}"        # a loot row says what it measured in its detail
        print(f"{status:<8}{row.id:<34} expected {fmt(exp)}{got}" + (f"  [{detail}]" if detail else ""))
    print(f"{counts['PASS']} PASS, {counts['FAIL']} FAIL, {counts['MISSING']} MISSING of {len(rows)} rows"
          + (f" ({counts['OPEN']} OPEN: a live observation the VM does not reproduce yet, listed above)" if counts["OPEN"] else "")
          + (f"; {len(ROWS) - len(rows)} loot rows not listed (no loot rows in this source, see docs/verification/COMBAT_LOOT.md)" if len(rows) < len(ROWS) else ""))
    return 1 if counts["FAIL"] else 0


def hits_from_q7(records):
    return [{"type": r["type"], "mind": [r["attacker"], r["attacker_level"], None], "dmg": r["arrival"], "applied": r["applied"],
             "sh0": r["sh"][0], "sh1": r["sh"][1], "hp0": r["hp"][0], "hp1": r["hp"][1], "boosts": r["boosts_active"]} for r in records]


def live_hits(directory):
    def load(name):
        return json.loads((directory / name).read_text(encoding="utf-8"))
    q7 = directory / "L6" / "q7_rows.json"
    if q7.exists():
        print("hits: L6/q7_rows.json (ratio rows use unboosted hits only; boost rows use the hit's boost count)")
        return hits_from_q7(load("L6/q7_rows.json"))
    print("note: L6/q7_rows.json not found; ratio rows use all hits with no boost filter, boost rows have no source")
    return load("L5b/player_hits.json")


def live_measured(directory):
    def load(name):
        return json.loads((directory / name).read_text(encoding="utf-8"))
    return measure_live(load("L5/dummy_pre_allprops.json"), load("L5b/bullymong_profiles_pre.json"),
                        load("L5b/bullymong_summary.json"), live_hits(directory))


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

    def boosted_hit(level, boosts, ratio):
        dmg = arrival(5, level)
        applied = dmg * ratio
        return {"type": MELEE, "mind": ["PrimalBeastAdult", level, None], "dmg": dmg, "applied": applied, "boosts": boosts,
                "sh0": 0, "sh1": 0, "hp0": 100.0, "hp1": 100.0 - applied}
    unboosted5 = boosted_hit(5, 0, applied_ratio(5))
    boost1_5 = boosted_hit(5, 1, applied_ratio(5) / 1.5)
    live_boost = measure_live(dummy_ok, profiles, {"kills": []}, [unboosted5, boost1_5])
    boost_status = {row.id: check_row(row, live_boost.get(row.id)) for row in ROWS if row.kind == "ratio"}
    expect("unboosted ratio ignores the boosted hit", boost_status["ratio.PrimalBeastAdult.L5"][0], "PASS")
    expect("boost1 factor PASS", boost_status["ratio_boost1.PrimalBeastAdult.L5"][0], "PASS")
    expect("boost1 expected is ratio / 1.5", round(boost_status["ratio_boost1.PrimalBeastAdult.L5"][1], 9), round(applied_ratio(5) / 1.5, 9))

    only_boosted_6 = measure_live(dummy_ok, profiles, {"kills": []}, [boosted_hit(6, 1, applied_ratio(6) / 1.5)])
    expect("no unboosted L6 hit is MISSING", check_row(next(r for r in ROWS if r.id == "ratio.PrimalBeastAdult.L6"), only_boosted_6.get("ratio.PrimalBeastAdult.L6"))[0], "MISSING")
    expect("no unboosted L6 hit reason", check_row(next(r for r in ROWS if r.id == "ratio.PrimalBeastAdult.L6"), None)[2], "no unboosted hit in the dump")
    expect("boost1 L6 PASS", check_row(next(r for r in ROWS if r.id == "ratio_boost1.PrimalBeastAdult.L6"), only_boosted_6.get("ratio_boost1.PrimalBeastAdult.L6"))[0], "PASS")
    expect("no boost2 hit is MISSING", check_row(next(r for r in ROWS if r.id == "ratio_boost2.PrimalBeastAdult.L6"), None)[0], "MISSING")

    wrong_factor = measure_live(dummy_ok, profiles, {"kills": []}, [boosted_hit(6, 1, applied_ratio(6))])
    expect("boost1 without the 1/1.5 factor FAIL", check_row(next(r for r in ROWS if r.id == "ratio_boost1.PrimalBeastAdult.L6"), wrong_factor.get("ratio_boost1.PrimalBeastAdult.L6"))[0], "FAIL")

    q7_hit = hits_from_q7([{"type": MELEE, "attacker": "PrimalBeastAdult", "attacker_level": 6, "arrival": 83.0, "applied": 40.0,
                            "hp": [100.0, 60.0], "sh": [0.0, 0.0], "boosts_active": 2}])[0]
    expect("q7 hit keeps boost count", q7_hit["boosts"], 2)
    expect("q7 hit keeps arrival and level", (q7_hit["dmg"], q7_hit["mind"][1]), (83.0, 6))
    expect("q7 hit keeps hp pair", (q7_hit["hp0"], q7_hit["hp1"]), (100.0, 60.0))

    vm = {"health.TargetDummy.L8": health(5, 8), "health.PrimalBeastBaby.L6": 1.0, "kill.PrimalBeast.L6.p10": {"award": 75.0, "credit": 11}}
    statuses = {row.id: check_row(row, vm.get(row.id))[0] for row in ROWS}
    expect("vm exact PASS", statuses["health.TargetDummy.L8"], "PASS")
    expect("vm wrong value FAIL", statuses["health.PrimalBeastBaby.L6"], "FAIL")
    expect("vm absent MISSING", statuses["health.PrimalBeastAdult.L7"], "MISSING")
    expect("vm kill both keys PASS", statuses["kill.PrimalBeast.L6.p10"], "PASS")
    expect("vm partial kill MISSING", check_row(next(r for r in ROWS if r.id == "kill.PrimalBeastBaby.L6.p10.b"), {"award": 50.0})[0], "MISSING")
    # The down rows (invented VM values; the live numbers they stand for are in the constants above).
    row_of = {row.id: row for row in ROWS}
    expect("bleed_total N=0,1,2,3", [bleed_total(n) for n in (0, 1, 2, 3)], [12.0, 12.0, 8.0, 4.0])
    expect("respawn_fee rounds half up", [respawn_fee(c) for c in (2715, 2534, 2357, 50, 10)], [190, 177, 165, 4, 1])
    expect("respawn_fee never above the cash", respawn_fee(0), 0)
    expect("down rows listed", sum(1 for r in ROWS if r.kind.startswith("down")), 12 + 4 + 4 + 4 + 4 + 2)
    expect("down total 3rd (counter 2) is 8", expected(row_of["down.total.6"]), 8.0)
    down_vm = {"down.total.1": 12.0, "down.total.5": 8.0, "down.successive.5": 1, "down.successive.6": 1, "down.bleed_over.1": 0.0334,
               "down.bleed_over.2": 0.08, "down.second_wind_health.4": 0.25, "down.respawn_health.1": 0.99,
               "down.fee.1": {"cash": 2715, "fee": 190}, "down.fee.2": {"cash": 2534, "fee": 178}, "down.fee.3": {"cash": 2000, "fee": 165},
               "down.fee.6": {"cash": 2192, "fee": 153}}
    down_status = {rid: check_row(row_of[rid], down_vm.get(rid))[0] for rid in row_of if rid.startswith("down.")}
    expect("down total PASS", down_status["down.total.1"], "PASS")
    expect("down total with the old 12/8/4 reading FAIL", down_status["down.total.5"], "FAIL")
    expect("down counter PASS", down_status["down.successive.5"], "PASS")
    expect("down counter off by one FAIL", down_status["down.successive.6"], "FAIL")
    expect("down overshoot within a frame PASS", down_status["down.bleed_over.1"], "PASS")
    expect("down overshoot of a frame and a half FAIL", down_status["down.bleed_over.2"], "FAIL")
    expect("down camera within the band PASS", check_row(row_of["down.camera.1"], 5.80)[0], "PASS")
    expect("down camera of the old 0.77 s (effect duration unread) FAIL", check_row(row_of["down.camera.2"], 0.7667)[0], "FAIL")
    expect("down second wind share PASS", down_status["down.second_wind_health.4"], "PASS")
    expect("down second wind absent MISSING", down_status["down.second_wind_health.5"], "MISSING")
    expect("down respawn share 0.99 FAIL", down_status["down.respawn_health.1"], "FAIL")
    expect("down fee PASS", down_status["down.fee.1"], "PASS")
    expect("down fee off by one FAIL", down_status["down.fee.2"], "FAIL")
    expect("down fee on the wrong cash FAIL", down_status["down.fee.3"], "FAIL")
    expect("down fee by the rule when the scenario fixes no cash PASS", down_status["down.fee.6"], "PASS")
    expect("down fee half row MISSING", check_row(row_of["down.fee.1"], {"cash": 2715})[0], "MISSING")
    # The loot rows (invented VM numbers).
    expect("loot rows listed", sum(1 for r in ROWS if r.kind.startswith("loot")), 8 + 16 + 9 + 3 + 2 + 2 + 1 + 1 + 1)
    loot_vm = {"loot.batch.adult.n": 500, "loot.batch.baby.n": 500, "loot.batch.low.n": 100}
    for tag in ("adult", "baby"):
        for pool, chance in LOOT_CHANCE.items():
            loot_vm[f"loot.batch.{tag}.{pool}.chance"] = chance
            loot_vm[f"loot.batch.{tag}.{pool}.tried"] = round(500 * chance)
    loot_vm["loot.batch.adult.Pool_Money_1or2.chance"] = 0.3                    # a wrong chance
    loot_vm["loot.batch.baby.Pool_Health_All.tried"] = 40                       # 15 expected, sd 3.8: far outside
    loot_vm["loot.batch.adult.Pool_Eridium_Bar.tried"] = 2                      # a rare pool: 0.75 expected, within the band
    loot_vm["loot.batch.low.Pool_Health_All.chance"] = 0.175
    loot_vm.update({"loot.roll.Pool_Eridium_Stick.stage6.n": 50, "loot.roll.Pool_Eridium_Stick.stage6.true": 0, "loot.roll.Pool_Eridium_Stick.stage7.n": 50, "loot.roll.Pool_Eridium_Stick.stage7.true": 49,
                    "loot.roll.Pool_Health_All.stage6.n": 200, "loot.roll.Pool_Health_All.stage6.true": 200, "loot.roll.Pool_Money_1or2.stage6.n": 5, "loot.roll.Pool_Money_1or2.stage6.true": 4,
                    "loot.roll.Pool_Ammo_All_Emergency.stage6.n": 200, "loot.roll.Pool_Ammo_All_Emergency.stage6.true": 0})
    loot_vm.update({"loot.batch.adult.Pool_VehicleSkins_All.items": 13, "loot.batch.adult.Pool_VehicleSkins_All.dropped": 0, "loot.batch.adult.Pool_Health_All.items": 9, "loot.batch.adult.Pool_Health_All.dropped": 8,
                    "loot.batch.adult.Pool_Money_1or2.items": 100, "loot.batch.adult.Pool_Money_1or2.dropped": 100})
    loot_vm.update({"loot.batch.baby.stage.6": 500, "loot.batch.adult.stage.4": 120, "loot.batch.adult.stage.5": 130, "loot.batch.adult.stage.6": 125, "loot.batch.adult.stage.7": 125,
                    "loot.batch.adult.level_mismatch": 0, "loot.batch.adult.Pool_Money_1or2.tried": 130, "loot.batch.adult.Pool_Money_1or2.qty.1": 97, "loot.batch.adult.Pool_Money_1or2.qty.2": 33,
                    "loot.batch.baby.Pool_Money_1or2.tried": 120, "loot.batch.baby.Pool_Money_1or2.qty.1": 90, "loot.batch.baby.Pool_Money_1or2.qty.2": 30})
    loot_merged = {**loot_vm, **measure_loot(loot_vm)}
    loot_status = {row.id: check_row(row, loot_merged.get(row.id)) for row in ROWS if row.kind.startswith("loot")}
    expect("loot chance PASS", loot_status["loot.chance.Pool_GunsAndGear"][0], "PASS")
    expect("loot chance off FAIL", loot_status["loot.chance.Pool_Money_1or2"][0], "FAIL")
    expect("loot rate PASS", loot_status["loot.rate.adult.Pool_GunsAndGear"][0], "PASS")
    expect("loot rate far above FAIL", loot_status["loot.rate.baby.Pool_Health_All"][0], "FAIL")
    expect("loot rate of a rare pool within the band PASS", loot_status["loot.rate.adult.Pool_Eridium_Bar"][0], "PASS")
    expect("loot flag none PASS", loot_status["loot.live.Pool_Eridium_Stick.stage6"][0], "PASS")
    expect("loot flag all PASS", loot_status["loot.live.Pool_Health_All.stage6"][0], "PASS")
    expect("loot flag all with one false FAIL", loot_status["loot.live.Pool_Money_1or2.stage6"][0], "FAIL")
    expect("loot flag none with a true FAIL", loot_status["loot.rule.Pool_Eridium_Stick.stage7"][0], "FAIL")
    expect("loot flag absent MISSING", loot_status["loot.live.Pool_GunsAndGear.stage6"][0], "MISSING")
    expect("loot destroyed none PASS", loot_status["loot.live.drop.Pool_VehicleSkins_All"][0], "PASS")
    expect("loot dropped all with one destroyed FAIL", loot_status["loot.live.drop.Pool_Health_All"][0], "FAIL")
    expect("loot dropped all PASS", loot_status["loot.live.drop.Pool_Money_1or2"][0], "PASS")
    expect("loot baby stage PASS", loot_status["loot.live.stage.baby"][0], "PASS")
    expect("loot adult stage band PASS", loot_status["loot.live.stage.adult"][0], "PASS")
    expect("loot level PASS", loot_status["loot.live.level.adult"][0], "PASS")
    expect("loot money 1 item reachable PASS", loot_status["loot.live.money.baby.qty1"][0], "PASS")
    expect("loot money 3 items unreachable is OPEN, not FAIL", loot_status["loot.live.money.adult.qty3"][0], "OPEN")
    expect("loot money quantity share PASS", loot_status["loot.rule.money.qty2_share"][0], "PASS")
    expect("loot health at 20 percent PASS", loot_status["loot.rule.health.chance_at_20pct"][0], "PASS")
    loot_wrong = {**loot_merged, "loot.batch.adult.Pool_Money_1or2.qty.3": 4}
    expect("loot money 3 items reachable PASS", check_row(next(r for r in ROWS if r.id == "loot.live.money.adult.qty3"), measure_loot(loot_wrong)["loot.live.money.adult.qty3"])[0], "PASS")
    loot_skewed = {**loot_vm, "loot.batch.adult.stage.4": 10, "loot.batch.adult.stage.5": 240}
    expect("loot adult stage skew FAIL", check_row(next(r for r in ROWS if r.id == "loot.live.stage.adult"), measure_loot(loot_skewed)["loot.live.stage.adult"])[0], "FAIL")
    expect("loot qty1 absent is FAIL (not OPEN)", check_row(next(r for r in ROWS if r.id == "loot.live.money.baby.qty1"), {"tried": 10, "counts": {2: 10}})[0], "FAIL")
    for failure in failures: print("self-test FAIL: " + failure)
    print("self-test " + ("FAILED" if failures else "passed"))
    return 1 if failures else 0


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--live", action="store_true", help="compare with the ignored live dumps under local/realgame/")
    parser.add_argument("--realgame", default=str(REALGAME), help="directory holding the L5 and L5b dumps")
    parser.add_argument("--vm", metavar="FILE.json", nargs="+", help="compare with one or more VM JSONs (schema in the docstring; the rows are merged)")
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
