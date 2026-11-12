"""What one money pickup is worth, for the openwillow_realgame command channel (range lane, 2026-11).

Run through Invoke-GamePyFile (tools/real_game/realgame.ps1) after scripts/combat_probe.py, whose helpers it uses (get_pc, cb_enemies,
cb_place, cb_aim, cb_mods_off, _g, _p); our own code, no game data. Rows go to RG_OUT (the ignored local/realgame/cash). tools/real_game/
cash_capture.ps1 drives it. Memory only: saves are blocked by the driver and the session's cash change is never written.

Why: the range's money pickups use a stand-in value. COMBAT_GOLDEN_TRACE.md item 8 has one live number (three Money_Little pickups, +3
each, at item level 7); this probe records more, with their levels, so the host can use a rule instead of a guess.

The two kinds of row are kept apart on purpose:
- cp_collect_*: a REAL pickup. The cash before and after, and which pickups went. A value per item is reported only when exactly one
  pickup went, because neighbours can be collected by the same touch.
- cp_sweep: the item's own value call at other levels (ExpLevel set in memory on a pickup lying on the ground, restored at once). That is
  an evaluation, not a payment: compare it with the real rows at their level before trusting it.
"""
import json
import math

try:
    import unrealsdk
    from unrealsdk.hooks import Block, Type, add_hook, has_hook, remove_hook
except ImportError:   # tests/den_probe_test.py exercises the arithmetic without a game
    unrealsdk = None

CP = globals().get("CP") or {"hooks": [], "real": [], "sweeps": [], "items": [], "definition": None, "kills": 0}
# The readings loop (session 2 analysis, A2): a pickup that gave no reading is skipped from then on, not retried; each try notes whether the
# touch event fired and the pickup's location and physics fields; deaths are counted apart from kill attempts.
CP.setdefault("skip", [])
CP.setdefault("touches", 0)
CP.setdefault("deaths", 0)
CP_TOUCH = "WillowGame.WillowPlayerController:TouchedPickupable"
CP_TOUCH_ID = "ow_cp_touch"
# The pickup's own fields recorded per try: where it is and whether it can be touched. Fields only: a name that is a function is skipped.
CP_PICKUP_FIELDS = ("Physics", "bCollideActors", "bBlockActors", "bHidden", "bDeleteMe")
# Names to try for the item's own value: which of these the class has is itself a result, so all are recorded.
VALUE_CALLS = ("GetMonetaryValue", "GetMonetaryValueAfterModifiers")
VALUE_FIELDS = ("MonetaryValue",)
# The player's own class declares TakeDamage too (ai_melee_probe.py hooks it there), and a hook is on one declaration only. So every declaration on the way down
# is hooked: the first one reached refuses the call.
CP_IMMUNE_CLASSES = ("WillowPlayerPawn", "WillowPawn", "Pawn")
CP_IMMUNE_ID = "ow_cp_immune"


def immune_paths(declared):
    """Every declaration of TakeDamage to hook, given declared(class name, function name). Empty unless the player's own class or its parent declares it."""
    paths = {name: list(declared(name, "TakeDamage")) for name in CP_IMMUNE_CLASSES}
    if not paths["WillowPlayerPawn"] and not paths["WillowPawn"]:
        return []
    return [path for name in CP_IMMUNE_CLASSES for path in paths[name]]


def value_per_item(delta, gone, target):
    """The value of one pickup, or None when the row cannot say: exactly the pickup stood on went, and the cash rose. A neighbour going
    instead, or nothing paid, is not a reading."""
    return delta if gone == [target] and delta > 0 else None


def cp_start():
    """The player takes no damage for the session (a hook that refuses the call, memory only); the loot mods' hooks are off."""
    def hook(obj, args, ret, func):
        try:
            if obj == get_pc().Pawn:
                return Block
        except Exception:
            pass
    CP["immune_paths"] = immune_paths(_declared_paths)
    # The touch counter: an observer that counts the calls and reads nothing, never blocks. A path this build lacks is reported, not fatal.
    try:
        unrealsdk.find_object("Function", CP_TOUCH)
        if not has_hook(CP_TOUCH, Type.PRE, CP_TOUCH_ID):
            add_hook(CP_TOUCH, Type.PRE, CP_TOUCH_ID, _count_touch)
            CP["hooks"].append((CP_TOUCH, Type.PRE, CP_TOUCH_ID))
        CP["touch_hook"] = "installed"
    except Exception as error:
        CP["touch_hook"] = "not installed: " + repr(error)[:80]
    for path in CP["immune_paths"]:
        if not has_hook(path, Type.PRE, CP_IMMUNE_ID):
            add_hook(path, Type.PRE, CP_IMMUNE_ID, hook)
            CP["hooks"].append((path, Type.PRE, CP_IMMUNE_ID))
    return {"immune": cp_immune(), "paths": CP["immune_paths"], "mods_off": cb_mods_off()}


def _count_touch(obj, args, ret, func):
    CP["touches"] = CP.get("touches", 0) + 1


def _fields(obj, names):
    """Plain fields of an object; a name that is a function is skipped, never called."""
    out = {}
    for name in names:
        try:
            value = getattr(obj, name)
        except Exception:
            continue
        if not callable(value):
            out[name] = value if isinstance(value, (bool, int, float, str)) or value is None else str(value)
    return out


def _where(pk):
    """A pickup's location and its touch-related fields, by fields only."""
    try:
        loc = pk.Location
        at = [round(loc.X), round(loc.Y), round(loc.Z)]
    except Exception:
        at = None
    return dict(_fields(pk, CP_PICKUP_FIELDS), location=at)


def cp_immune_off():
    """Remove this probe's own immunity hooks (cash_capture.ps1 -DenDown: the pack may reach Maya). Nothing else changes.
    Returns {"remaining": [...]}: every path that still has this probe's hook, asked of the SDK path by path. Empty means none is left."""
    for path in CP.get("immune_paths") or []:
        if has_hook(path, Type.PRE, CP_IMMUNE_ID):
            remove_hook(path, Type.PRE, CP_IMMUNE_ID)
    CP["hooks"] = [h for h in CP["hooks"] if h[2] != CP_IMMUNE_ID]
    return {"remaining": immune_remaining(CP.get("immune_paths") or [], lambda path: has_hook(path, Type.PRE, CP_IMMUNE_ID))}


def immune_remaining(paths, hooked):
    """The paths among `paths` that still carry the immunity hook, by hooked(path)."""
    return [path for path in paths if hooked(path)]


def down_state(pawn, health, injured):
    """Why Maya counts as down or up, from what could be read: 'no_pawn', 'injured_flag', 'no_health' or 'up'. The injured flag is tested
    before the health: in the four downs of session 2 (2026-11-12) the health had already read 0 at the poll that saw the down, so testing
    the health first labelled every down 'no_health' and the flag was never reported. `injured` is None when the pawn has no readable
    bIsInjured field; then a down shows only as 'no_health' or 'no_pawn'."""
    if pawn is None:
        return "no_pawn"
    if injured:
        return "injured_flag"
    if health is None or health <= 0:
        return "no_health"
    return "up"


def cp_down_state():
    """Maya's state for the den down, reads only: the condition, whether the injured field could be read, and health plus shield ('pool',
    or None when the shield cannot be read), which cash_capture.ps1 compares from poll to poll: a drop means a hit reached her. Native
    calls: GetHealth and GetShieldStrength on the player's own pawn only (two of the calls the watch lists)."""
    pawn = get_pc().Pawn
    if pawn is None:
        return {"state": down_state(None, None, None), "injured_field": None, "pool": None}
    health = pawn.GetHealth()
    try:
        injured = bool(pawn.bIsInjured)
    except Exception:
        injured = None
    try:
        pool = round(float(health) + float(pawn.GetShieldStrength()), 3)
    except Exception:
        pool = None
    return {"state": down_state(pawn, health, injured), "injured_field": injured is not None, "pool": pool}


def cp_immune():
    """Whether the immunity hooks are really installed: asked of the SDK, not of our own list."""
    paths = CP.get("immune_paths") or []
    return bool(paths) and all(has_hook(path, Type.PRE, CP_IMMUNE_ID) for path in paths)


def cp_up():
    """Maya stands, with health: what the immunity is for. A hook in place does not prove it held."""
    pawn = get_pc().Pawn
    return pawn is not None and pawn.GetHealth() > 0 and not _g(pawn, "bIsInjured", False)


def cp_cash():
    return get_pc().PlayerReplicationInfo.GetCurrencyOnHand(0)


def cp_level():
    return get_pc().Pawn.GetExpLevel()


def _money_pickups():
    rows = []
    for pk in unrealsdk.find_all("WillowPickup"):
        if "Default__" in pk._path_name() or _g(pk, "bDeleteMe", False):
            continue
        inv = _g(pk, "Inventory")
        if inv is None:
            continue
        try:
            balance = _p(inv.DefinitionData.BalanceDefinition)
        except Exception:
            continue
        if "Currency_Money" in str(balance):
            rows.append((pk, inv, str(balance)))
    return rows


def _item_row(pk, inv, balance):
    me = get_pc().Pawn.Location
    loc = pk.Location
    row = {"name": pk.Name, "balance": balance.rsplit(".", 1)[-1], "exp_level": _g(inv, "ExpLevel"), "game_stage": _g(inv, "GameStage"),
           "dist": round(math.dist((me.X, me.Y, me.Z), (loc.X, loc.Y, loc.Z))), "values": _values(inv)}
    # The item's part definitions (session 2 analysis, A4): its DefinitionData struct, each part by name. Fields only.
    try:
        row["parts"] = _p(inv.DefinitionData, 0)
    except Exception as error:
        row["parts"] = "unavailable: " + repr(error)[:80]
    return row


def _values(inv):
    """Whatever the item itself says it is worth; a call the class lacks is recorded as such, not guessed around. Not fields only: the
    item's value functions are called (inherited since e7b5fa3; docs/TOOLING.md, "What the fields-only rule binds")."""
    out = {}
    for call in VALUE_CALLS:
        try:
            out[call] = getattr(inv, call)()
        except Exception as error:
            out[call] = "unavailable: " + repr(error)[:80]
    for field in VALUE_FIELDS:
        out[field] = _g(inv, field, "unavailable")
    return out


def cp_money():
    """The money pickups on the ground, nearest first."""
    rows = [_item_row(*item) for item in _money_pickups()]
    rows.sort(key=lambda r: r["dist"])
    for row in rows:
        if row["name"] not in {i["name"] for i in CP["items"]}:
            CP["items"].append(row)
    return rows


def cp_target(radius=6000.0):
    """The nearest living enemy with a controller, or None."""
    for row in cb_enemies(radius):
        if row.get("controller") and not row.get("bPlayedDeath") and (row.get("hp") or 0) > 0:
            return row["name"]
    return None


def cp_ready_kill(name):
    """Stand 500 uu from the enemy, aim at it and leave it one hit point: the caller's real shot then runs the stock death and loot.
    CP["kills"] counts these attempts; cp_died says whether the attempt killed. Not fields only: SetHealth is called on the kill target,
    and cp_target's cb_enemies calls the four health and shield getters on each pawn in its radius (inherited since e7b5fa3; ran in the
    sessions of 2026-11-12; docs/TOOLING.md, "What the fields-only rule binds")."""
    cb_place(name, 500.0, 180.0)
    _find_ai(name).SetHealth(1.0)
    CP["kills"] += 1
    return name


def cp_died(name):
    """Whether the enemy of the last kill attempt died: gone, being destroyed or marked dead, by its fields only. Counted in CP["deaths"]."""
    pawn = _find_ai(name)
    died = pawn is None or bool(_fields(pawn, ("bDeleteMe",)).get("bDeleteMe")) or bool(_fields(pawn, ("bPlayedDeath",)).get("bPlayedDeath"))
    if died:
        CP["deaths"] = CP.get("deaths", 0) + 1
    return died


def cp_collect_start():
    """Stand on the nearest money pickup not yet skipped. Returns the state the matching cp_collect_end compares with, or None without one.
    The target is chosen from the pickups not skipped, but what was present is recorded from ALL money pickups: a skipped one that goes in
    the same walk then shows as gone, and the try is no reading (review of #251/#255: its cash was read as the target's value)."""
    every = _money_pickups()
    items = [i for i in every if i[0].Name not in CP["skip"]]
    if not items:
        return None
    me = get_pc().Pawn.Location
    items.sort(key=lambda i: math.dist((me.X, me.Y, me.Z), (i[0].Location.X, i[0].Location.Y, i[0].Location.Z)))
    pk, inv, balance = items[0]
    # Skipped from now on, before anything else is read or moved: a try that raises half way (its end never comes) cannot bring the same
    # pickup back, so the loop's "distinct pickups" stays true. A pickup that gave a reading is gone, so skipping it costs nothing.
    if pk.Name not in CP["skip"]:
        CP["skip"].append(pk.Name)
    CP["pending"] = {"before": cp_cash(), "target": _item_row(pk, inv, balance), "present": [i[0].Name for i in every],
                     "levels": {i[0].Name: _g(i[1], "ExpLevel") for i in every}, "touches_before": CP.get("touches", 0),
                     "where_before": _where(pk)}
    loc = pk.Location
    get_pc().Pawn.Location = unrealsdk.make_struct("Vector", X=loc.X, Y=loc.Y, Z=loc.Z + 60.0)
    return CP["pending"]["target"]["name"]


def cp_collect_end():
    """The result of the last cp_collect_start: the cash change, which pickups went, and a value per item when exactly one did."""
    pending = CP.get("pending")
    if not pending:
        return None
    still = {i[0].Name for i in _money_pickups()}
    gone = [n for n in pending["present"] if n not in still]
    after = cp_cash()
    row = {"cash_before": pending["before"], "cash_after": after, "delta": after - pending["before"], "gone": gone,
           "gone_levels": [pending["levels"].get(n) for n in gone], "target": pending["target"], "player_level": cp_level()}
    row["value_per_item"] = value_per_item(row["delta"], gone, pending["target"]["name"])
    # Why a try gave no reading: whether the touch event fired, and where the pickup was before and is now (fields only).
    target = pending["target"]["name"]
    row["touches"] = CP.get("touches", 0) - pending.get("touches_before", 0)
    row["where_before"] = pending.get("where_before")
    left = [i[0] for i in _money_pickups() if i[0].Name == target]
    row["where_after"] = _where(left[0]) if left else None
    if row["value_per_item"] is None:
        row["skipped_from_now"] = True   # not retried (recorded at the start of the try): the next try takes another pickup
    CP["real"].append(row)
    CP["pending"] = None
    return row


def cp_sweep(levels=(1, 2, 5, 8, 10, 15, 20, 30, 40, 50)):
    """The value call of one pickup on the ground at other levels (ExpLevel in memory, restored at once). Evaluation only, see the header."""
    items = _money_pickups()
    if not items:
        return None
    pk, inv, balance = items[0]
    original = _g(inv, "ExpLevel")
    rows = {"balance": balance.rsplit(".", 1)[-1], "original_level": original, "at_original": _values(inv), "by_level": {}}
    try:
        for level in levels:
            inv.ExpLevel = level
            rows["by_level"][str(level)] = _values(inv)
    finally:
        inv.ExpLevel = original
    rows["restored"] = _g(inv, "ExpLevel") == original
    CP["sweeps"].append(rows)
    return rows


def cp_definition():
    """The money item's definition, as names and plain values, for reading the value rule in the data afterwards."""
    items = _money_pickups()
    if not items:
        return None
    inv = items[0][1]
    row = {}
    for field in ("DefinitionData", "InventoryDefinition", "Definition"):
        try:
            row[field] = _p(getattr(inv, field), 0)
        except Exception as error:
            row[field] = "unavailable: " + repr(error)[:80]
    CP["definition"] = row
    return row


def cp_dump(name="cash.json"):
    summary = {"kills": CP["kills"], "deaths": CP.get("deaths", 0), "player_level": cp_level(), "real": CP["real"], "items": CP["items"],
               "sweeps": CP["sweeps"], "definition": CP["definition"], "skipped": CP["skip"], "touch_hook": CP.get("touch_hook")}
    with open(RG_OUT + "/" + name, "w", encoding="utf-8") as f:
        json.dump(summary, f, indent=1, default=str)
    single = [r for r in CP["real"] if r["value_per_item"] is not None]
    return {"kills": CP["kills"], "deaths": CP.get("deaths", 0), "collected": len(CP["real"]), "single_item_rows": len(single),
            "skipped": len(CP["skip"]), "touch_hook": CP.get("touch_hook"),
            "values_by_level": sorted({(r["gone_levels"][0], r["value_per_item"]) for r in single}, key=str)}
