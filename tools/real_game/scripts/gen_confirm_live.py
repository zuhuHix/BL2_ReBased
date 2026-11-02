"""Generator confirmation in the running game: spawn the sample's exact guns, and roll pick frequencies.

Run through Invoke-GamePyFile after tools/real_game/scripts/weapon_cards.py (its helpers spawn_definition,
spawn_balance, weapon_record, card_trace_start, remove_spawned and write_json share this namespace). Our own
code; what it writes is game data and goes under RG_OUT (normally the ignored local/realgame/gen_confirm).

- gc_spawn_sample(): reads sample.json (tools/real_game/gen_confirm.py plan) and spawns every gun with exactly
  its generated parts, name parts, manufacturer and stage, one at a time. Writes live.json: per request the
  weapon_record the game holds.
- gc_freq_batch(balance, stage, n): rolls n guns from one balance with spawn_balance, records each
  one's parts, level and names, then removes it from the backpack (destroyed; saves are blocked). Appends the
  batch to frequency_rolls.json (a list of {balance, stage, rolls}).
- gc_remove(paths): removes the weapons this session added, by object path.
- gc_hook_card() / gc_last_card(): the item of the latest SetItemCardEx call, so the capture wrapper can step the
  inventory cursor and see which gun each card belongs to (the card trace has the card text itself).

The cards themselves come from the card trace (card_trace_start) while the inventory UI shows each item; the
capture wrapper drives that walk and gen_confirm.py joins the two.
"""
import json
from pathlib import Path

import unrealsdk


def gc_sample():
    return json.loads((Path(RG_OUT) / "sample.json").read_text(encoding="utf-8"))


def gc_spawn_sample(first=0, count=None):
    """Spawn sample guns [first, first+count). Returns the record list (also written to live.json)."""
    sample = gc_sample()
    chosen = sample[first:first + count] if count else sample[first:]
    path = Path(RG_OUT) / "live.json"
    records = json.loads(path.read_text(encoding="utf-8")) if path.is_file() and first else []
    for row in chosen:
        new = spawn_definition(row["fields"])
        if len(new) != 1:
            raise RuntimeError(f"request {row['index']}: {len(new)} new weapons (expected 1)")
        records.append({"index": row["index"], "record": weapon_record(new[0])})
    path.write_text(json.dumps(records, indent=1, ensure_ascii=False), encoding="utf-8")
    return [r["index"] for r in records]


def _roll_row(w):
    rec = weapon_record(w)
    return {"object": rec["object"], "balance": rec["balance"], "type": rec["type"],
            "manufacturer": rec["manufacturer"], "level": rec["game_stage"], "grade": rec["grade"],
            "parts": rec["parts"], "prefix": rec["prefix"], "title": rec["title"], "name": rec["name"],
            "sale_value": rec["sale_value"]}


def gc_freq_batch(balance, stage, n, out_name="frequency_rolls.json"):
    """Roll n guns from `balance` at `stage`, record and destroy each. Returns the number recorded."""
    path = Path(RG_OUT) / out_name
    data = json.loads(path.read_text(encoding="utf-8")) if path.is_file() else []
    group = next((g for g in data if g["balance"] == balance and g["stage"] == stage), None)
    if group is None:
        group = {"balance": balance, "stage": stage, "rolls": []}
        data.append(group)
    im = get_pc().GetPawnInventoryManager()
    done = 0
    for _ in range(n):
        for w in spawn_balance(balance, stage):
            group["rolls"].append(_roll_row(w))
            im.RemoveInventoryFromBackpack(w)
            w.Destroy()
            done += 1
    path.write_text(json.dumps(data, indent=1, ensure_ascii=False), encoding="utf-8")
    return done


def gc_remove(paths):
    return remove_spawned(set(paths))


CARD_ITEM_HOOK = "gc_card_item"
gc_cards = {"last": "", "items": []}


def _gc_card_item(obj, args, _ret, func):
    try:
        gc_cards["last"] = obj_path(args.InventoryItem) or ""
        gc_cards["items"].append(gc_cards["last"])
    except Exception as error:  # never break the game
        mark("gc_card_item_error", error=repr(error)[:300])


def gc_hook_card():
    """Remember the item of every SetItemCardEx call (the card the inventory shows), for the cursor walk."""
    cls = unrealsdk.find_class("ItemCardGFxObject")
    path = next(f._path_name() for f in cls._fields() if f._path_name().endswith(":SetItemCardEx"))
    add_hook(path, Type.PRE, CARD_ITEM_HOOK, _gc_card_item)
    return path


def gc_last_card():
    return gc_cards["last"]


# ---------------------------------------------------------------- evaluated stats (no card UI)
# The card prints values the game evaluates on the weapon, so they are read from the weapon object once it is
# readied in a quick slot (backpack items may not be initialised). Each gun goes into one free slot, one at a
# time; the slot's original weapon is put back at the end. Names are read dynamically: a field that does not
# exist is recorded as absent, never guessed.
GC_STAT_ATTRS = ("InstantHitDamage", "FireInterval", "ReloadTime", "ClipSize", "Spread", "ProjectilesPerShot",
                 "ShotCost", "StatusEffectDamage", "StatusEffectChance", "StatusChance")
GC_STAT_TERMS = ("status", "chance", "rarity", "level", "exp", "damage", "fire", "reload", "clip", "spread",
                 "shot", "projectile", "monetary", "price")
GC_STAT_METHODS = ("GetRarityLevel", "GetControllerPlayerExpLevelRequiredToUse", "GetMonetaryValue")


def _gc_plain(value):
    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    if hasattr(value, "Value") and hasattr(value, "BaseValue"):  # an attribute struct: its evaluated and base values
        return {"Value": _gc_plain(value.Value), "BaseValue": _gc_plain(value.BaseValue)}
    if hasattr(value, "_path_name"):
        return value._path_name()
    return repr(value)[:120]


def _gc_field_names(obj):
    try:
        return [f.Name for f in obj.Class._fields() if getattr(getattr(f, "Class", None), "Name", "").endswith("Property")]
    except Exception:
        return []


def _gc_stats(w):
    attrs = {}
    names = list(GC_STAT_ATTRS) + [n for n in _gc_field_names(w)
                                   if any(t in n.lower() for t in GC_STAT_TERMS) and n not in GC_STAT_ATTRS]
    for name in names:
        try:
            value = getattr(w, name)
        except Exception:
            attrs[name] = {"absent": True}
            continue
        if not callable(value):
            attrs[name] = _gc_plain(value)
    methods = {}
    for name in GC_STAT_METHODS:
        try:
            methods[name] = _gc_plain(getattr(w, name)())
        except Exception as error:
            methods[name] = {"error": type(error).__name__}
    return {"attrs": attrs, "methods": methods}


def _gc_find(path):
    equipped, backpack = inventory_weapons()
    return next((w for w in equipped + backpack if obj_path(w) == path), None)


def gc_read_stats():
    """Read the evaluated stats of every sample gun in live.json; writes stats.json and returns its row count."""
    live = json.loads((Path(RG_OUT) / "live.json").read_text(encoding="utf-8"))
    im = get_pc().GetPawnInventoryManager()
    held = get_pc().Pawn.Weapon
    held_slot = int(held.QuickSelectSlot) if held is not None else 0
    slot = next(s for s in (1, 2, 3, 4) if s != held_slot)
    occupant = next((w for w in inventory_weapons()[0] if int(w.QuickSelectSlot) == slot), None)
    original = obj_path(occupant) if occupant is not None else None
    rows = []
    for entry in live:
        path = entry["record"]["object"]
        row = {"index": entry["index"], "object": path, "slot": slot}
        w = _gc_find(path)
        if w is None:
            row["error"] = "not in inventory"
            rows.append(row)
            continue
        row["backpack"] = _gc_stats(w) if int(getattr(w, "QuickSelectSlot", 0) or 0) == 0 else None
        current = int(getattr(w, "QuickSelectSlot", 0) or 0)
        if current == held_slot and current:
            row["error"] = "gun is held; not moved"
            rows.append(row)
            continue
        if current:  # equipped in another slot: send it to the backpack first, then ready it into `slot`
            get_pc().GetPawnInventoryManager().SendSlottedThingToBackpack(current)
        im = get_pc().GetPawnInventoryManager()
        im.SendSlottedThingToBackpack(slot)  # clears the slot (the previous gun or the original occupant)
        w = _gc_find(path)
        if w is None:  # gone after the slot was cleared: record it, never guess
            row["error"] = "not in inventory after clearing slot"
            rows.append(row)
            continue
        get_pc().GetPawnInventoryManager().ReadyBackpackInventory(w, slot)
        # The readied gun can come back as a different object (its path changes once it is in a slot; the
        # parity probe looks up by recipe for the same reason), so it is found by its quick slot.
        w = next((x for x in inventory_weapons()[0] if int(x.QuickSelectSlot) == slot), None)
        if w is None:
            row["error"] = "no weapon in quick slot after readying"
            rows.append(row)
            continue
        row["object_after"] = obj_path(w)
        row["equipped"] = _gc_stats(w)
        row["slot_after"] = int(w.QuickSelectSlot)
        rows.append(row)
    if original is not None:  # put the original weapon of the slot back
        get_pc().GetPawnInventoryManager().SendSlottedThingToBackpack(slot)
        w = _gc_find(original)
        if w is not None:
            get_pc().GetPawnInventoryManager().ReadyBackpackInventory(w, slot)
    write_json("stats.json", rows)
    return len(rows)
