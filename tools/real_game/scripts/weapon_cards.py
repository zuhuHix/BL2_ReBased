"""Weapon golden cards: helpers for the openwillow_realgame command channel.

Run once through Invoke-GamePy (tools/real_game/realgame.ps1); it defines functions in the channel's
shared scope. Our own code; what it writes is game data and goes under the folder in `RG_OUT`
(normally the repository's ignored local/realgame/cards).

- card_trace_start(name) / card_trace_stop(): hook every function declared on ItemCardGFxObject and
  write one JSON line per call in the openwillow_uitrace row format, so tools/weapon_card_audit.py can
  read the file with --trace.
- weapon_record(w): balance, type, manufacturer, grade, game stage, every part by slot, material,
  prefix and title parts, the game's own name strings and sale value.
- spawn_balance(path, stage): one weapon rolled by the game from a balance at a game stage, put in
  the backpack. spawn_definition(dict): a weapon with exactly the given definition data.
- raise_backpack(n): in-memory backpack size (only safe while saving is blocked).
"""
import json
import time
from pathlib import Path

import unrealsdk
from unrealsdk.hooks import Type, add_hook, remove_hook

SLOTS = ("Body", "Grip", "Barrel", "Sight", "Stock", "Elemental", "Accessory1", "Accessory2", "Material")
CARD_HOOK = "ow_card_trace"
card_state = {"file": None, "seq": 0, "paths": []}


def obj_path(value):
    return value._path_name() if value is not None else None


def out_dir():
    folder = Path(RG_OUT)
    folder.mkdir(parents=True, exist_ok=True)
    return folder


def _card_call(obj, args, _ret, func):
    try:
        card_state["seq"] += 1
        row = {"seq": card_state["seq"], "t": time.perf_counter(), "phase": "call",
               "func": func.func._path_name(), "obj": plain(obj),
               "args": {p.Name: plain(getattr(args, p.Name)) for p in args._type._properties()
                        if p.Name != "ReturnValue"}}
        card_state["file"].write(json.dumps(row, ensure_ascii=False) + "\n")
        card_state["file"].flush()
    except Exception as error:  # never break the game
        mark("card_trace_error", error=repr(error)[:300])


def card_trace_start(name):
    card_trace_stop()
    cls = unrealsdk.find_class("ItemCardGFxObject")
    paths = [f._path_name() for f in cls._fields() if f.Class.Name == "Function" and f.Outer == cls]
    card_state.update(file=(out_dir() / name).open("w", encoding="utf-8"), seq=0, paths=paths)
    for path in paths:
        add_hook(path, Type.PRE, CARD_HOOK, _card_call)
    return len(paths)


def card_trace_stop():
    for path in card_state["paths"]:
        remove_hook(path, Type.PRE, CARD_HOOK)
    if card_state["file"] is not None:
        card_state["file"].close()
    card_state.update(file=None, paths=[])


def weapon_record(w):
    d = w.DefinitionData
    return {
        "object": obj_path(w), "class": w.Class.Name,
        "name": w.GetShortHumanReadableName(), "full_name": w.GenerateHumanReadableName(),
        "balance": obj_path(d.BalanceDefinition), "type": obj_path(d.WeaponTypeDefinition),
        "manufacturer": obj_path(d.ManufacturerDefinition), "grade": d.ManufacturerGradeIndex,
        "game_stage": d.GameStage, "unique_id": d.UniqueId,
        "parts": {slot: obj_path(getattr(d, slot + "PartDefinition")) for slot in SLOTS},
        "prefix": obj_path(d.PrefixPartDefinition), "title": obj_path(d.TitlePartDefinition),
        "sale_value": w.GetMonetaryValue(),
    }


def inventory_weapons():
    """Equipped weapons, then backpack weapons, as the inventory manager holds them."""
    im = get_pc().GetPawnInventoryManager()
    chain, item = [], im.InventoryChain
    while item:
        chain.append(item)
        item = item.Inventory
    equipped = [w for w in chain if w.Class.Name == "WillowWeapon"]
    backpack = [w for w in im.Backpack if w.Class.Name == "WillowWeapon"]
    return equipped, backpack


def raise_backpack(slots):
    im = get_pc().GetPawnInventoryManager()
    before = im.InventorySlotMax_Misc
    im.InventorySlotMax_Misc = slots
    return before


def _give(inv):
    im = get_pc().GetPawnInventoryManager()
    im.AddInventoryToBackpack(inv)
    return inv


def spawn_balance(balance_path, stage):
    balance = unrealsdk.find_object("WeaponBalanceDefinition", balance_path)
    pool = unrealsdk.find_class("ItemPool").ClassDefaultObject
    result = pool.SpawnBalancedInventoryFromInventoryBalanceDefinition(balance, 1, stage, 0, get_pc().Pawn, [])
    spawned = list(result[1]) if isinstance(result, tuple) else []
    return [_give(inv) for inv in spawned]


def spawn_definition(fields):
    """fields: WeaponDefinitionData field name -> object path (or int for grade/stage)."""
    # The game readies the new weapon straight into an empty weapon slot when one exists, so look
    # for it among equipped and backpack weapons.
    im = get_pc().GetPawnInventoryManager()
    before = {obj_path(w) for group in inventory_weapons() for w in group}
    values = {}
    for key, value in fields.items():
        if isinstance(value, str):
            values[key] = unrealsdk.find_object("Object", value) if "." in value else value
        else:
            values[key] = value
    data = unrealsdk.make_struct("WeaponDefinitionData", **values)
    im.AddBackpackWeaponFromDefinitionData(data)
    return [w for group in inventory_weapons() for w in group if obj_path(w) not in before]


def backpack_weapon(path):
    for w in get_pc().GetPawnInventoryManager().Backpack:
        if obj_path(w) == path:
            return w
    return None


def equip(path, slot):
    """Swap the backpack weapon at `path` into quick slot `slot` (1-4). The slot must not hold the
    weapon in hand; switch away from it first. Looks everything up fresh (see the driver's note)."""
    im = get_pc().GetPawnInventoryManager()
    held = get_pc().Pawn.Weapon
    if held is not None and held.QuickSelectSlot == slot:
        raise RuntimeError(f"slot {slot} holds the weapon in hand; switch weapons first")
    weapon = backpack_weapon(path)
    if weapon is None:
        raise RuntimeError(f"{path} is not in the backpack")
    im.SendSlottedThingToBackpack(slot)
    im.ReadyBackpackInventory(weapon, slot)
    return weapon.GetShortHumanReadableName()


def remove_spawned(objects):
    """Remove weapons this session added (object paths), leaving the player's own items alone."""
    im = get_pc().GetPawnInventoryManager()
    removed = 0
    for w in list(im.Backpack):
        if obj_path(w) in objects:
            im.RemoveInventoryFromBackpack(w)
            w.Destroy()
            removed += 1
    return removed


def write_json(name, data):
    path = out_dir() / name
    path.write_text(json.dumps(data, indent=1, ensure_ascii=False), encoding="utf-8")
    return str(path)
