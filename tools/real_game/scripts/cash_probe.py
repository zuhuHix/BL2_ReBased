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
    for path in CP["immune_paths"]:
        if not has_hook(path, Type.PRE, CP_IMMUNE_ID):
            add_hook(path, Type.PRE, CP_IMMUNE_ID, hook)
            CP["hooks"].append((path, Type.PRE, CP_IMMUNE_ID))
    return {"immune": cp_immune(), "paths": CP["immune_paths"], "mods_off": cb_mods_off()}


def cp_immune_off():
    """Remove this probe's own immunity hooks (cash_capture.ps1 -DenDown: the pack may reach Maya). Nothing else changes.
    Returns cp_immune() afterwards, which is False once every hook is gone."""
    for path in CP.get("immune_paths") or []:
        if has_hook(path, Type.PRE, CP_IMMUNE_ID):
            remove_hook(path, Type.PRE, CP_IMMUNE_ID)
    CP["hooks"] = [h for h in CP["hooks"] if h[2] != CP_IMMUNE_ID]
    return cp_immune()


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
    return row


def _values(inv):
    """Whatever the item itself says it is worth; a call the class lacks is recorded as such, not guessed around."""
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
    """Stand 500 uu from the enemy, aim at it and leave it one hit point: the caller's real shot then runs the stock death and loot."""
    cb_place(name, 500.0, 180.0)
    _find_ai(name).SetHealth(1.0)
    CP["kills"] += 1
    return name


def cp_collect_start():
    """Stand on the nearest money pickup. Returns the state the matching cp_collect_end compares with, or None without a pickup."""
    items = _money_pickups()
    if not items:
        return None
    me = get_pc().Pawn.Location
    items.sort(key=lambda i: math.dist((me.X, me.Y, me.Z), (i[0].Location.X, i[0].Location.Y, i[0].Location.Z)))
    pk, inv, balance = items[0]
    CP["pending"] = {"before": cp_cash(), "target": _item_row(pk, inv, balance), "present": [i[0].Name for i in items],
                     "levels": {i[0].Name: _g(i[1], "ExpLevel") for i in items}}
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
    summary = {"kills": CP["kills"], "player_level": cp_level(), "real": CP["real"], "items": CP["items"], "sweeps": CP["sweeps"],
               "definition": CP["definition"]}
    with open(RG_OUT + "/" + name, "w", encoding="utf-8") as f:
        json.dump(summary, f, indent=1, default=str)
    single = [r for r in CP["real"] if r["value_per_item"] is not None]
    return {"kills": CP["kills"], "collected": len(CP["real"]), "single_item_rows": len(single),
            "values_by_level": sorted({(r["gone_levels"][0], r["value_per_item"]) for r in single}, key=str)}
