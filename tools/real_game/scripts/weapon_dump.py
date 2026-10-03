"""Weapon data dump: every property the stat evaluator and the name rule read, straight from the running game.

Run once through Invoke-GamePyFile (tools/real_game/realgame.ps1). Our own code; the output is game data
and goes under the folder in `RG_OUT` (normally the repository's ignored local/realgame/cards), never into
a tracked file. It reads the live objects of WeaponPartDefinition, WeaponTypeDefinition and
WeaponNamePartDefinition and writes {path: {"class": ..., "properties": {...}}} with the keys that
tools/weapon_card_audit.py lists in OVERLAY_KEYS, in the shape weapon_recipe's decoder produces
(object references as plain object paths, enums by name, structs as dicts, arrays as lists).
"""
import json
from pathlib import Path

import unrealsdk
from unrealsdk.unreal import UObject, WrappedArray, WrappedStruct

DUMP_CLASSES = ("WeaponPartDefinition", "WeaponTypeDefinition", "WeaponNamePartDefinition")
DUMP_KEYS = ("WeaponAttributeEffects", "ExternalAttributeEffects", "ZoomWeaponAttributeEffects",
             "AttributeSlotEffects", "AttributeSlotUpgrades", "ClipSize", "ReloadTime", "FireRate", "Spread",
             "InstantHitDamage", "StatusEffectDamage", "BaseStatusEffectChanceModifier", "ProjectilesPerShot",
             "MonetaryValueMod", "Rarity", "Priority", "TitleList", "PrefixList", "CustomDamageTypeDefinition",
             "CustomFiringModeDefinition", "MinExpLevelRequirement", "MaxExpLevelRequirement")


def deep(value, depth=0):
    if depth > 8:
        return repr(value)[:120]
    name = getattr(value, "name", None)  # enum members first: IntFlag members are ints too
    if isinstance(name, str) and not isinstance(value, (bool, str)):
        return name
    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    if isinstance(value, UObject):
        return value._path_name()
    if isinstance(value, WrappedStruct):
        return {p.Name: deep(getattr(value, p.Name), depth + 1) for p in value._type._properties()}
    if isinstance(value, WrappedArray):
        return [deep(v, depth + 1) for v in list(value)]
    return repr(value)[:120]


def weapon_dump():
    result = {}
    for cls_name in DUMP_CLASSES:
        for obj in unrealsdk.find_all(cls_name, exact=False):
            record = {}
            for key in DUMP_KEYS:
                try:
                    record[key] = deep(getattr(obj, key))
                except (AttributeError, ValueError):
                    pass
            result[obj._path_name()] = {"class": obj.Class.Name, "properties": record}
    return result


data = weapon_dump()
target = Path(RG_OUT)
target.mkdir(parents=True, exist_ok=True)
(target / "live_weapon_data.json").write_text(json.dumps(data, ensure_ascii=False, sort_keys=True), encoding="utf-8")
by_class = {}
for row in data.values():
    by_class[row["class"]] = by_class.get(row["class"], 0) + 1
print(json.dumps(by_class))
