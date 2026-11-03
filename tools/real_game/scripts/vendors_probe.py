"""Vending machine ground truth for the vendors lane (2026-11-03).

Run once through Invoke-GamePyFile (tools/real_game/realgame.ps1). Our own code; what it writes is game data and goes
under RG_OUT (normally the ignored local/realgame/vendors). It reads only: no purchase, sale or reset is made.

- vp_dump(name): every WillowVendingMachine (and the Black Market) in the loaded world with its class, shop type,
  location, markup values, last reset time, and for each stock item and the featured item: class, balance, level,
  the item's monetary value and the shop's selling price for it. Also the player's backpack items with their value
  and the price each shop quotes for them, the player's currency, the world time and the game's last shop reset time,
  and the live values of the two markup attributes. Written to <RG_OUT>/<name>.json and returned.
Every read is wrapped: a failing call is recorded as "<error ...>" instead of aborting the dump.
"""
import json
import os
from pathlib import Path

import unrealsdk

MARKUP_ATTRIBUTES = ("GD_Economy.VendingMachine.Att_SellExponentializer",
                     "GD_Economy.VendingMachine.Att_BaseVendingMarkupIncrease",
                     "D_Attributes.Inventory.UnownedItemLevel")


def _try(fn, *args):
    try:
        return fn(*args)
    except Exception as e:  # noqa: BLE001 - a probe records failures instead of stopping
        return f"<error {e!r}>"[:200]


def _name(obj):
    return obj._path_name() if obj is not None else None


def _first(value):
    return value[0] if isinstance(value, tuple) else value


def _pc():
    return get_pc()  # noqa: F821 - the openwillow_realgame driver defines get_pc in the command scope


def _item(item, machine, pc):
    row = {"object": _try(_name, item), "class": _try(lambda: item.Class.Name)}
    row["balance"] = _try(lambda: _name(item.DefinitionData.BalanceDefinition))
    row["level"] = _try(lambda: item.DefinitionData.ManufacturerGradeIndex)
    row["game_stage"] = _try(lambda: item.DefinitionData.GameStage)
    row["exp_level"] = _try(lambda: item.ExpLevel)
    row["rarity"] = _try(lambda: _first(item.GetRarityLevel()))
    row["monetary_value"] = _try(lambda: _first(item.GetMonetaryValue()))
    row["name"] = _try(lambda: _first(item.GetShortHumanReadableName()))
    if machine is not None:
        row["selling_price"] = _try(lambda: _first(machine.GetSellingPriceForInventory(item, pc, 1)))
    return row


def _attribute(path, context):
    attribute = _try(unrealsdk.find_object, "AttributeDefinition", path)
    if isinstance(attribute, str) or attribute is None:
        return {"path": path, "value": attribute}
    return {"path": path, "value": _try(lambda: list(attribute.GetValue(context)) if isinstance(
        attribute.GetValue(context), tuple) else attribute.GetValue(context))}


def _machine(machine, pc):
    row = {"object": _try(_name, machine), "class": _try(lambda: machine.Class.Name),
           "definition": _try(lambda: _name(machine.InteractiveObjectDefinition)),
           "shop_type": _try(lambda: int(machine.ShopType)),
           "location": _try(lambda: [machine.Location.X, machine.Location.Y, machine.Location.Z]),
           "commerce_markup": _try(lambda: machine.CommerceMarkup.Value),
           "commerce_markup_base": _try(lambda: machine.CommerceMarkup.BaseValue),
           "featured_markup": _try(lambda: machine.FeaturedItemCommerceMarkup.Value),
           "last_reset": _try(lambda: machine.LastInventoryResetTime),
           "fixed_item_cost": _try(lambda: machine.FixedItemCost),
           "fixed_featured_cost": _try(lambda: machine.FixedFeaturedItemCost),
           "reset_cost": _try(lambda: _first(machine.GetResetCost()))}
    stock = _try(lambda: list(machine.ShopInventory) if machine.ShopInventory is not None else [])
    row["stock"] = [_item(i, machine, pc) for i in stock] if isinstance(stock, list) else stock
    featured = _try(lambda: machine.FeaturedItem)
    row["featured"] = _item(featured, machine, pc) if featured is not None and not isinstance(featured, str) else featured
    row["attributes"] = [_attribute(p, machine) for p in MARKUP_ATTRIBUTES]
    return row


def vp_dump(name="vendors"):
    pc = _pc()
    world = pc.WorldInfo
    out = {"time_seconds": _try(lambda: world.TimeSeconds),
           "last_shop_reset": _try(lambda: world.Game.LastShopResetTime),
           "minutes_between_resets": _try(lambda: unrealsdk.find_object(
               "GlobalsDefinition", "GD_Globals.General.Globals").MinutesBetweenShopResets),
           "player_level": _try(lambda: pc.Pawn.GetExpLevel()),
           "currency": _try(lambda: [pc.PlayerReplicationInfo.GetCurrencyOnHand(i) for i in range(3)]),
           "player_markup": _try(lambda: pc.Pawn.CommerceMarkup)}
    machines = [m for m in unrealsdk.find_all("WillowVendingMachineBase", exact=False)
                if "Default__" not in _name(m)]
    out["machines"] = [_machine(m, pc) for m in machines]
    backpack = _try(lambda: list(pc.GetPawnInventoryManager().Backpack))
    out["backpack"] = ([dict(_item(i, None, pc), quotes=[_try(lambda m=m: _first(
        m.GetSellingPriceForInventory(i, pc, 1))) for m in machines]) for i in backpack]
        if isinstance(backpack, list) else backpack)
    folder = Path(globals().get("RG_OUT") or os.environ.get("RG_OUT", "."))  # Invoke-GamePyFile sets RG_OUT
    folder.mkdir(parents=True, exist_ok=True)
    (folder / f"{name}.json").write_text(json.dumps(out, indent=1, default=str), encoding="utf-8")
    return out
