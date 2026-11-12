"""How a real den engages a player who stands still, for the openwillow_realgame command channel (range lane, 2026-11).

Run through Invoke-GamePyFile (tools/real_game/realgame.ps1) after scripts/combat_probe.py and scripts/cash_probe.py, whose helpers it uses
(get_pc, cb_enemies, cb_place, _g, _p, cp_start for the player's immunity). Our own code, no game data. Rows go to RG_OUT (the ignored
local/realgame/cash). tools/real_game/cash_capture.ps1 drives it as its own stage, after the cash read. Nothing is saved, but it is not read-only: it moves
the player, changes her physics and aims the camera, with her immunity on, in a save-blocked session whose files are restored afterwards.

The host's range (OpenWillowEnemyAI.cpp) has three rules no live read has settled:
- how far apart the first blows of a den's members land (the host's four landed on one tick);
- how fast a member really closes on the player (the host's run_speed is a stand-in; the stock GroundSpeed field is not proven to be it);
- how far from the player a swing really starts (the host now takes the stock attack action's range).

So: the player stands 1,600 uu from an enemy, immune and without input, and every melee call, every blow and the members' positions are
recorded. The rules a trial's numbers can speak for are this map's dens and this player's level: anything else stays UNVERIFIED.
"""
import json
import math
import statistics

try:
    import unrealsdk
    from unrealsdk.hooks import Type, add_hook, remove_hook
except ImportError:   # tests/den_probe_test.py exercises the arithmetic without a game
    unrealsdk = None

DP = globals().get("DP") or {"hooks": [], "trials": [], "trial": None, "last_sample": 0.0, "hook_errors": []}
DP_TICK = "WillowGame.WillowGameViewportClient:Tick"
DP_MELEE = "WillowGame.MeleeDefinition:MeleeAttack"
SAMPLE_SECONDS = 0.05
SWING_TO_DAMAGE = 0.41      # AI_MELEE_TARGETING_LIVE 4: the melee call comes this long after the swing's Attack trigger
MOVING_SPEED = 200.0        # uu/s: slower than this is not the chase (a patrol walk or standing)


def flat(a, b):
    return math.hypot(a[0] - b[0], a[1] - b[1])


def summarise(trial):
    """The three answers of one trial, from its rows only. An answer the rows cannot give is None, never a guess."""
    samples = trial.get("samples") or []
    calls = trial.get("calls") or []
    blows = trial.get("blows") or []
    first_blow = {}
    for blow in blows:
        if blow.get("attacker") and blow["attacker"] not in first_blow:
            first_blow[blow["attacker"]] = blow["t"]
    first_call = {}
    for call in calls:
        if call.get("attacker") and call["attacker"] not in first_call:
            first_call[call["attacker"]] = call["t"]
    by_pawn = {}
    for row in samples:
        by_pawn.setdefault(row["name"], []).append(row)

    speeds = {}
    for name, rows in by_pawn.items():
        moving = [r["speed"] for r in rows if r["speed"] >= MOVING_SPEED]
        if moving:
            speeds[name] = round(statistics.median(moving), 1)

    def position_at(name, t):
        rows = by_pawn.get(name) or []
        return min(rows, key=lambda r: abs(r["t"] - t)) if rows else None

    starts = []
    for call in calls:
        row = position_at(call.get("attacker"), call["t"] - SWING_TO_DAMAGE)
        # Only a sample close to the swing's start says where it started.
        if row and abs(row["t"] - (call["t"] - SWING_TO_DAMAGE)) <= 0.2:
            starts.append({"attacker": call["attacker"], "definition": call.get("definition"), "distance_at_start": round(row["to_player"]),
                           "distance_at_call": call.get("to_player")})
    spread = lambda first: round(max(first.values()) - min(first.values()), 3) if len(first) > 1 else None
    return {"members_seen": sorted(by_pawn), "kinds": trial.get("kinds"), "attackers": len(first_blow), "first_blow": first_blow, "first_blow_spread": spread(first_blow),
            "melee_callers": len(first_call), "first_call": first_call, "first_call_spread": spread(first_call), "chase_speed_median": speeds,
            "swing_starts": starts, "calls": len(calls), "blows": len(blows), "samples": len(samples),
            "immunity_held": trial.get("hp_end", 0) >= trial.get("hp_start", 1), "hook_errors": trial.get("hook_errors", 0)}


def trial_counts(summary):
    """A trial counts when it saw a melee call and a member move, with Maya kept whole and every hook call read: otherwise its numbers can't be trusted."""
    return bool(summary["calls"] and summary["chase_speed_median"] and summary["immunity_held"] and not summary["hook_errors"])


def den_answers(summaries):
    """What the stage still lacks, of what it must have seen: empty when, in trials that count, the three questions of this file's header each have an answer."""
    counted = [s for s in summaries if s.get("counts")]
    missing = []
    if not any(s["first_blow_spread"] is not None for s in counted):
        missing.append("first-blow spread: no counted trial had blows from two attackers")
    if not any(s["chase_speed_median"] for s in counted):
        missing.append("chase speed: no counted trial saw a member move")
    if not any(s["swing_starts"] for s in counted):
        missing.append("swing start: no counted trial had a position at a swing's start")
    return missing


def _now():
    return get_pc().WorldInfo.TimeSeconds


def _attacker_of_call(args, func):
    """The melee call's first object argument is the attacking pawn (ai_awareness_probe.py calls it that way)."""
    for prop in func.func._properties():
        value = getattr(args, prop.Name, None)
        if value is not None and hasattr(value, "Location"):
            return value
    return None


def _guarded(hook):
    """An error inside a hook is only printed by the SDK, and the event is lost: so it is counted, and a trial with any does not count."""
    def run(obj, args, ret, func):
        try:
            return hook(obj, args, ret, func)
        except Exception as error:
            DP["hook_errors"].append(hook.__name__ + ": " + repr(error)[:120])
            return None
    return run


def _on_tick(obj, args, ret, func):
    trial = DP["trial"]
    if not trial:
        return
    now = _now()
    if now - DP["last_sample"] < SAMPLE_SECONDS:
        return
    DP["last_sample"] = now
    me = get_pc().Pawn.Location
    for pawn in trial["pawns"]:
        if _g(pawn, "bDeleteMe", False):
            continue
        loc, vel = pawn.Location, pawn.Velocity
        trial["samples"].append({"t": round(now - trial["t0"], 3), "name": pawn.Name, "to_player": round(flat((loc.X, loc.Y), (me.X, me.Y)), 1),
                                 "speed": round(math.hypot(vel.X, vel.Y), 1), "target": _g(_g(_g(pawn, "MyWillowMind"), "AIComponent"), "CurrentTarget") is not None})


def _on_melee(obj, args, ret, func):
    trial = DP["trial"]
    if not trial:
        return
    pawn = _attacker_of_call(args, func)
    me = get_pc().Pawn.Location
    row = {"t": round(_now() - trial["t0"], 3), "definition": obj.Name, "attacker": pawn.Name if pawn else None}
    if pawn:
        row["to_player"] = round(flat((pawn.Location.X, pawn.Location.Y), (me.X, me.Y)), 1)
    trial["calls"].append(row)


def _on_damage(obj, args, ret, func):
    trial = DP["trial"]
    if not trial or obj != get_pc().Pawn:
        return
    instigator = _g(args, "InstigatedBy")
    pawn = _g(instigator, "Pawn") if instigator else None
    trial["blows"].append({"t": round(_now() - trial["t0"], 3), "damage": _g(args, "Damage"), "attacker": pawn.Name if pawn else None})


def dp_start():
    """The recording hooks; a hook this build refuses is reported, and the stage fails on it rather than run half blind."""
    # add_hook takes any name, so a wrong path would only ever record nothing: each must be a real function, and the player's damage has a real declaration.
    damage = immune_paths(_declared_paths)
    if not damage:
        raise RuntimeError("no declaration of the player's TakeDamage")
    wanted = [(DP_TICK, "ow_dp_tick", _on_tick), (DP_MELEE, "ow_dp_melee", _on_melee)] + [(path, "ow_dp_damage", _on_damage) for path in damage]
    for path, ident, hook in wanted:
        unrealsdk.find_object("Function", path)
        if (path, Type.PRE, ident) in DP["hooks"]:
            continue
        add_hook(path, Type.PRE, ident, _guarded(hook))
        DP["hooks"].append((path, Type.PRE, ident))
    return [path for path, _, _ in wanted]


def dp_begin(distance=1600.0, radius=6000.0, near=3500.0):
    """The player `distance` from the nearest living enemy with a controller; recording starts. The name, or None without one."""
    rows = [r for r in cb_enemies(radius) if r.get("controller") and not r.get("bPlayedDeath") and (r.get("hp") or 0) > 0]
    if not rows:
        return None
    cb_place(rows[0]["name"], distance, 180.0)
    centre = _find_ai(rows[0]["name"]).Location
    pawns = []
    kinds = {}
    for pawn in unrealsdk.find_all("WillowAIPawn"):
        if not pawn.Name.startswith("WillowAIPawn_") or _g(pawn, "bDeleteMe", False):
            continue
        loc = pawn.Location
        if math.dist((loc.X, loc.Y, loc.Z), (centre.X, centre.Y, centre.Z)) <= near:
            pawns.append(pawn)
            kinds[pawn.Name] = str(_p(_g(pawn, "AIClass")))
    DP["hook_errors"] = []
    DP["trial"] = {"t0": _now(), "hp_start": get_pc().Pawn.GetHealth(), "pawns": pawns, "kinds": kinds, "first": rows[0]["name"], "distance": distance, "samples": [], "calls": [], "blows": []}
    DP["last_sample"] = 0.0
    return rows[0]["name"]


def dp_replace(distance=1600.0, radius=6000.0):
    """The den down's re-placement (cash_capture.ps1), as ONE call, so nothing runs in the game between the check and the move: only while
    Maya is still up, she is put `distance` from a living enemy within `radius` of where she stands. The enemy is picked by the watch's
    fields-only pawn read (slice_watch.py: sw_live_pawn, _sw_dist, _sw_pool_refs, _sw_fields, which ran in S0 of 2026-11-12), so nothing
    is called on a scanned pawn, unlike dp_begin's pick through cb_enemies, which calls the health and shield getters on each. Living means
    a Controller field set and a health pool resolved through its manager (not stale) above 0.
    Where: the first placement's enemy (dp_begin's trial 'first') while it is still living and in range, at dp_begin's bearing 180, the
    one side proven open ground (result 'first'); otherwise the nearest living enemy, on the side she is on now, the bearing from the
    two Location fields (result 'nearest_my_side'), since another pawn's bearing 180 may be rock, a ledge or the far side of the pack.
    The open den trial is left as dp_begin opened it. Never raises: the reply is always a result word for the log, and 'not_up' leaves
    the down to the next poll."""
    try:
        state = cp_down_state().get("state")   # GetHealth and GetShieldStrength on the player's own pawn only
        if state != "up":
            return {"result": "not_up", "state": state}
        if "sw_live_pawn" not in globals():
            return {"result": "no_watch"}
        here = get_pc().Pawn.Location
        first = (DP.get("trial") or {}).get("first")
        best, chosen = None, None
        for pawn in unrealsdk.find_all("WillowAIPawn"):
            if not sw_live_pawn(pawn):
                continue
            dist = _sw_dist(pawn, here)
            if dist is None or dist > radius or _sw_fields(pawn, ("Controller",)).get("Controller") is None:
                continue
            health = _sw_pool_refs(pawn).get("health_pool") or {}
            value = health.get("CurrentValue")
            if health.get("stale") is not None or not isinstance(value, (int, float)) or value <= 0:
                continue
            if pawn.Name == first:
                chosen = (dist, pawn.Name, None)
            if best is None or dist < best[0]:
                best = (dist, pawn.Name, pawn.Location)
        if chosen is None and best is None:
            return {"result": "no_enemy"}
        if chosen is not None:
            cb_place(chosen[1], distance, 180.0)
            return {"result": "placed", "rule": "first", "enemy": chosen[1], "dist": chosen[0]}
        t = best[2]
        bearing = math.degrees(math.atan2(here.Y - t.Y, here.X - t.X))
        cb_place(best[1], distance, bearing)
        return {"result": "placed", "rule": "nearest_my_side", "enemy": best[1], "dist": best[0], "bearing": round(bearing, 1)}
    except Exception as error:
        return {"result": "error", "why": repr(error)[:160]}


def dp_end():
    """Stops recording. The trial's answers, and whether they count."""
    trial = DP["trial"]
    DP["trial"] = None
    if not trial:
        return None
    trial.pop("pawns", None)
    trial["hp_end"] = get_pc().Pawn.GetHealth()
    trial["hook_errors"] = len(DP["hook_errors"])
    summary = summarise(trial)
    summary["hook_error_samples"] = DP["hook_errors"][:3]
    summary["counts"] = trial_counts(summary)
    DP["trials"].append({"summary": summary, "rows": trial})
    return summary


def dp_stop():
    for path, typ, ident in DP["hooks"]:
        remove_hook(path, typ, ident)
    DP["hooks"] = []
    return True


def dp_dump(name="den.json"):
    """Writes every trial's rows; returns the summaries, and what the stage must have seen and did not."""
    summaries = [t["summary"] for t in DP["trials"]]
    missing = den_answers(summaries)
    with open(RG_OUT + "/" + name, "w", encoding="utf-8") as f:
        json.dump({"limits": "one map's dens at one player level; a member's first blow is not proof of the den's order elsewhere", "missing": missing, "trials": DP["trials"]}, f, indent=1, default=str)
    return {"summaries": summaries, "missing": missing}
