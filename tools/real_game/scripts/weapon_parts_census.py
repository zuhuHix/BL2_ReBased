"""Live weapon-parts census: every weapon-generation object the running game has loaded, with identity fields.

Run once through Invoke-GamePyFile (tools/real_game/realgame.ps1) with OutFolder 'weapons', so RG_OUT is
local/realgame/weapons. Our own code; it reads the live objects through unrealsdk.find_all and writes
{"state", "counts", "objects": {path: record}} to RG_OUT/live_catalog.json. Read-only: nothing is assigned.

Record fields by class (only what tools/weapon_catalog.py crosscheck compares):
- balances (WeaponBalanceDefinition and subclasses): manufacturer and weapon_type (first value along the
  BaseDefinition chain), runtime_parts {raw slot: [part paths]} from RuntimePartListCollection (every slot
  that holds parts, enabled or not; enabled flags in runtime_enabled).
- part lists: slots {raw slot: [part paths]}.
- parts: part_type (enum name, e.g. WP_Barrel), manufacturer (ManufacturerDefinitionForScopeMovie),
  gestalt_names (GestaltModeSkeletalMeshName then AdditionalGestaltModeSkeletalMeshNames, 'None' dropped).
- pools: children (BalancedItems / ItemPools entries).
- everything else: class only.
"""
import json
from pathlib import Path

import unrealsdk
from unrealsdk.unreal import UObject

CLASSES = ("WeaponBalanceDefinition", "MissionWeaponBalanceDefinition", "WeaponPartDefinition",
           "WeaponTypeDefinition", "ManufacturerDefinition", "WeaponNamePartDefinition",
           "WeaponPartListCollectionDefinition", "WeaponPartListDefinition", "GestaltSkeletalMeshDefinition",
           "ItemPoolDefinition", "KeyedItemPoolDefinition", "ItemPoolListDefinition")
SLOTS = ("Body", "Grip", "Barrel", "Sight", "Stock", "Elemental", "Accessory1", "Accessory2", "Material")
CHAIN_LIMIT = 20


def read(obj, name):
    """Property value, or None when the object or property is missing (never raises)."""
    if obj is None:
        return None
    try:
        return getattr(obj, name)
    except (AttributeError, ValueError, TypeError):
        return None


def path_of(value):
    return value._path_name() if isinstance(value, UObject) else None


def plain(value):
    name = getattr(value, "name", None)  # enum members
    if isinstance(name, str) and not isinstance(value, (bool, str)):
        return name
    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    return path_of(value)


def has_value(value):
    """True for an object, a non-empty string or a non-empty array."""
    if value is None:
        return False
    if isinstance(value, (str, UObject)):
        return bool(value)
    try:
        return len(list(value)) > 0
    except TypeError:
        return True


def chain_value(obj, name):
    """First non-empty value of `name` along the BaseDefinition chain, starting at obj."""
    cursor = obj
    for _ in range(CHAIN_LIMIT):
        if cursor is None:
            return None
        value = read(cursor, name)
        if has_value(value):
            return value
        cursor = read(cursor, "BaseDefinition")
    return None


def slot_parts(collection):
    """{raw slot: [part paths]} for every <Slot>PartData of a WeaponPartListCollectionDefinition, plus enabled flags."""
    parts, enabled = {}, {}
    for slot in SLOTS:
        data = read(collection, slot + "PartData")
        if data is None:
            continue
        enabled[slot] = bool(read(data, "bEnabled"))
        listed = [path_of(read(entry, "Part")) for entry in list(read(data, "WeightedParts") or [])]
        parts[slot] = [p for p in listed if p]
    return parts, enabled


def balance_record(obj):
    makers = chain_value(obj, "Manufacturers")
    maker = None
    if makers is not None and len(list(makers)):
        maker = path_of(read(list(makers)[0], "Manufacturer"))
    runtime, enabled = slot_parts(read(obj, "RuntimePartListCollection"))
    return {"class": obj.Class.Name, "manufacturer": maker,
            "weapon_type": path_of(chain_value(obj, "InventoryDefinition")),
            "runtime_parts": runtime, "runtime_enabled": enabled}


def part_record(obj):
    names = [read(obj, "GestaltModeSkeletalMeshName")]
    names += [str(n) for n in list(read(obj, "AdditionalGestaltModeSkeletalMeshNames") or [])]
    return {"class": obj.Class.Name, "part_type": plain(read(obj, "PartType")),
            "manufacturer": path_of(read(obj, "ManufacturerDefinitionForScopeMovie")),
            "gestalt_names": [str(n) for n in names if n and str(n) != "None"]}


def pool_record(obj):
    children = [path_of(read(e, "ItmPoolDefinition")) or path_of(read(e, "InvBalanceDefinition"))
                for e in list(read(obj, "BalancedItems") or [])]
    children += [path_of(read(e, "ItemPool")) for e in list(read(obj, "ItemPools") or [])]
    return {"class": obj.Class.Name, "children": [c for c in children if c]}


def record(obj):
    cls = obj.Class.Name
    if cls.endswith("WeaponBalanceDefinition"):
        return balance_record(obj)
    if cls == "WeaponPartDefinition":
        return part_record(obj)
    if cls == "WeaponPartListCollectionDefinition":
        parts, enabled = slot_parts(obj)
        return {"class": cls, "slots": parts, "enabled": enabled}
    if "ItemPool" in cls:
        return pool_record(obj)
    return {"class": cls}


def census():
    objects, errors = {}, 0
    for cls_name in CLASSES:
        for obj in unrealsdk.find_all(cls_name, exact=False):
            path = obj._path_name()
            if path in objects:
                continue
            try:
                objects[path] = record(obj)
            except Exception:  # keep going: one odd object must not lose the census
                errors += 1
                objects[path] = {"class": obj.Class.Name, "error": True}
    return objects, errors


def in_game_flag():
    try:
        return len(unrealsdk.find_all("WillowPlayerPawn", exact=False)) > 0
    except Exception:
        return None


objects, errors = census()
counts = {}
for item in objects.values():
    counts[item["class"]] = counts.get(item["class"], 0) + 1
state = {"player_pawn_loaded": in_game_flag(), "record_errors": errors}
target = Path(RG_OUT)
target.mkdir(parents=True, exist_ok=True)
(target / "live_catalog.json").write_text(
    json.dumps({"state": state, "counts": dict(sorted(counts.items())), "objects": objects},
               ensure_ascii=False, sort_keys=True), encoding="utf-8")
print(json.dumps({"state": state, "counts": dict(sorted(counts.items()))}))
