"""Spawn exact-part guns in the running game and read what each one draws (weapon part assets, matched captures).

Loaded once through tools/real_game/weapon_part_session.ps1 -Mode Capture with WPC_RECIPES (local recipe JSON paths) and
RG_OUT set; saves are blocked by the session. Our own code; everything it writes is game data under RG_OUT (ignored
local/realgame/wpa). Spawning follows tools/real_game/scripts/weapon_parity_probe.py (definition data built from the
recipe's parts, AddBackpackWeaponFromDefinitionData, ReadyBackpackInventory into quick slot 1, matching by definition data
because readying creates a new actor).

Commands after loading:
  wpc_equip(recipe_id)  spawn (once) and ready the gun in quick slot 2 or 1 (the one not in hand); returns the slot
  wpc_check(recipe_id)  after the session pressed that slot's key: is the held gun this one; reads its live fragment record
  wpc_cleanup()         remove only the guns this script spawned, and put the original slot-1 weapon back
The live record per gun (live_guns.json): the first-person mesh component's gestalt fields (fragment names and LOD
triangle ranges, discovered by name: any property whose name contains 'Gestalt'), the summed LOD0 triangles, the
material on slot 0 with its parent and vector parameters.
"""
import enum
import json
from pathlib import Path

import unrealsdk

SLOTS = ("Body", "Grip", "Barrel", "Sight", "Stock", "Elemental", "Accessory1", "Accessory2", "Material")
wpc = globals().setdefault("wpc", {"spawned": {}, "recipes": {}, "original": None, "live": {}})


def path(value):
    try:
        return value._path_name() if value is not None else None
    except Exception:
        return None


def deep(value, depth=0):
    if isinstance(value, enum.Enum):
        return value.name
    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    if hasattr(value, "_path_name") and hasattr(value, "Class"):
        return path(value)
    if depth > 7:
        return repr(value)[:120]
    try:
        return {p.Name: deep(getattr(value, p.Name), depth + 1) for p in value._type._properties()}
    except Exception:
        pass
    try:
        return [deep(v, depth + 1) for v in list(value)]
    except Exception:
        return repr(value)[:120]


def fields_like(obj, needle):
    out = {}
    try:
        names = [f.Name for f in obj.Class._fields() if getattr(getattr(f, "Class", None), "Name", "").endswith("Property")]
    except Exception:
        names = []
    for name in names:
        if needle.lower() in name.lower():
            try:
                out[name] = deep(getattr(obj, name))
            except Exception as error:
                out[name] = "<error %r>" % (error,)
    return out


def triangles_in(value):
    """Sum of NumPrimitives-like counts in the first LOD list found in a discovered gestalt record."""
    if isinstance(value, dict):
        for key in ("NumPrimitives", "NumTriangles"):
            if isinstance(value.get(key), int):
                return value[key]
        for key, item in value.items():
            if "lod" in key.lower() and isinstance(item, list) and item:
                first = item[0]
                return sum(triangles_in(x) or 0 for x in (first if isinstance(first, list) else
                                                           first.get("Fragments", first.get("Parts", [first])) if isinstance(first, dict) else []))
        for item in value.values():
            found = triangles_in(item)
            if found:
                return found
    if isinstance(value, list):
        return sum(triangles_in(x) or 0 for x in value) or None
    return None


def definition(recipe):
    fields = {"WeaponTypeDefinition": recipe["weapon_type"], "BalanceDefinition": recipe["balance"],
              "ManufacturerDefinition": recipe["manufacturer"]}
    stage = int(recipe.get("game_stage", recipe.get("level", 8)))
    values = {"GameStage": stage, "ManufacturerGradeIndex": int(recipe.get("manufacturer_grade_index", stage))}
    for slot in SLOTS:
        part = (recipe.get("parts", {}).get(slot) or {})
        part = part.get("part") if isinstance(part, dict) else part
        if part:
            fields[slot + "PartDefinition"] = part
    for key, field in (("prefix", "PrefixPartDefinition"), ("title", "TitlePartDefinition")):
        value = recipe.get(key)
        value = value.get("part") if isinstance(value, dict) else value
        if value:
            fields[field] = value
    for key, value in fields.items():
        obj = unrealsdk.find_object("Object", value)
        if obj is None:
            raise LookupError("could not resolve %s: %s" % (key, value))
        values[key] = obj
    return values


def parts_of(weapon):
    data = weapon.DefinitionData
    return {slot: path(getattr(data, slot + "PartDefinition", None)) for slot in SLOTS}


def matches(weapon, recipe):
    try:
        want = {slot: ((recipe["parts"].get(slot) or {}).get("part") if isinstance(recipe["parts"].get(slot), dict)
                       else recipe["parts"].get(slot)) for slot in SLOTS}
        got = parts_of(weapon)
        return path(weapon.DefinitionData.BalanceDefinition) == recipe["balance"] and all(
            (want[s] or None) == (got[s] or None) for s in SLOTS)
    except Exception:
        return False


def inventory():
    im = get_pc().GetPawnInventoryManager()  # noqa: F821 (driver helper)
    chain, item = [], im.InventoryChain
    while item:
        chain.append(item)
        item = item.Inventory
    weapons = [w for w in chain if w.Class.Name == "WillowWeapon"]
    backpack = [w for w in im.Backpack if w.Class.Name == "WillowWeapon"]
    return im, weapons, backpack


def slot_of(weapon):
    try:
        return int(weapon.QuickSelectSlot)
    except Exception:
        return 0


def find(recipe_id):
    recipe = wpc["recipes"][recipe_id]
    _im, weapons, backpack = inventory()
    return next((w for w in weapons + backpack if matches(w, recipe)), None)


def live_record(weapon):
    mesh = getattr(weapon, "FirstPersonMesh", None) or getattr(weapon, "Mesh", None)
    record = {"weapon": path(weapon), "parts": parts_of(weapon), "mesh_component": path(mesh),
              "mesh_fields": fields_like(weapon, "Mesh")}
    if mesh is not None:
        record["mesh_class"] = mesh.Class.Name
        record["gestalt_fields"] = fields_like(mesh, "Gestalt")
        record["part_fields"] = {**fields_like(mesh, "Part"), **fields_like(mesh, "Fragment")}
        record["skeletal_mesh"] = path(getattr(mesh, "SkeletalMesh", None))
        try:
            material = mesh.GetMaterial(0)
            record["material"] = {"path": path(material), "parent": path(getattr(material, "Parent", None)),
                                  "vectors": deep(getattr(material, "VectorParameterValues", None)),
                                  "scalars": deep(getattr(material, "ScalarParameterValues", None))}
        except Exception as error:
            record["material"] = "<error %r>" % (error,)
        # Every material slot of the gestalt mesh (the pistol gestalt has two; which material the game puts on slot 1 was open).
        slots = []
        count = 0
        for call in ("GetNumElements", "GetNumMaterials"):  # UE3 MeshComponent: GetNumElements()
            try:
                count = int(getattr(mesh, call)())
                break
            except Exception:
                continue
        record["material_overrides"] = deep(getattr(mesh, "Materials", None))
        for slot in range(count):
            try:
                material = mesh.GetMaterial(slot)
                slots.append({"slot": slot, "path": path(material), "parent": path(getattr(material, "Parent", None)),
                              "class": material.Class.Name if material is not None else None})
            except Exception as error:
                slots.append({"slot": slot, "error": repr(error)})
        record["material_slots"] = slots
    record["weapon_gestalt_fields"] = fields_like(weapon, "Gestalt")
    record["triangles"] = triangles_in(record.get("gestalt_fields")) or triangles_in(record.get("weapon_gestalt_fields"))
    return record


def wpc_equip(recipe_id):
    try:
        if recipe_id not in wpc["recipes"]:
            raise LookupError("recipe not loaded: " + recipe_id)
        recipe = wpc["recipes"][recipe_id]
        im, weapons, backpack = inventory()
        if wpc["original"] is None:
            held = get_pc().Pawn.Weapon  # noqa: F821
            wpc["original"] = {"slot": slot_of(held) if held else 0, "parts": parts_of(held) if held else None,
                               "balance": path(held.DefinitionData.BalanceDefinition) if held else None}
        # Keep the backpack small: a gun this script spawned earlier that is back in the backpack is removed now.
        for other in [k for k in wpc["spawned"] if k != recipe_id]:
            old = find(other)
            if old is not None and slot_of(old) == 0:
                get_pc().GetPawnInventoryManager().RemoveInventoryFromBackpack(old)  # noqa: F821
                old.Destroy()
                del wpc["spawned"][other]
        weapon = find(recipe_id)
        if weapon is None:
            im = get_pc().GetPawnInventoryManager()  # noqa: F821
            im.AddBackpackWeaponFromDefinitionData(unrealsdk.make_struct("WeaponDefinitionData", **definition(recipe)))
            weapon = find(recipe_id)
            if weapon is None:
                raise RuntimeError("spawned gun did not appear in the inventory")
            wpc["spawned"][recipe_id] = True
        # Ready into quick slot 1, which must not be in hand (readying into the held slot does not take, as lane E's probe
        # found): the session calls wpc_away() first. Slot 1 is where the inventory's Left lands for Inspect.
        target = 1  # wpc_away() has switched the hand to slot 2 first, so slot 1 is not the held slot
        if slot_of(weapon) != target:
            im = get_pc().GetPawnInventoryManager()  # noqa: F821
            if slot_of(weapon) > 0:
                im.SendSlottedThingToBackpack(slot_of(weapon))
            im.SendSlottedThingToBackpack(target)
            weapon = next((w for w in get_pc().GetPawnInventoryManager().Backpack  # noqa: F821
                           if w.Class.Name == "WillowWeapon" and matches(w, recipe)), None)
            if weapon is None:
                raise RuntimeError("gun not in backpack after clearing slot %d" % target)
            get_pc().GetPawnInventoryManager().ReadyBackpackInventory(weapon, target)  # noqa: F821
            weapon = find(recipe_id)
        record = live_record(weapon)
        wpc["live"][recipe_id] = record
        save_live()
        return json.dumps({"ok": True, "id": recipe_id, "slot": target})
    except Exception as error:
        return json.dumps({"ok": False, "id": recipe_id, "error": repr(error)})


def wpc_clear_backpack():
    """Remove the save's own backpack weapons in memory (saves are blocked; nothing persists), so each spawned gun is the only
    backpack weapon and Inspect can reach it (inventory: Right from the equipped column goes to the backpack's first cell).
    Lane D's 2026-10-21 session removed the save's items the same way."""
    removed = []
    im = get_pc().GetPawnInventoryManager()  # noqa: F821
    for weapon in [w for w in im.Backpack if w.Class.Name == "WillowWeapon"]:
        removed.append(path(weapon))
        im.RemoveInventoryFromBackpack(weapon)
        weapon.Destroy()
    return json.dumps({"removed": len(removed)})


def wpc_spawn(recipe_id):
    """Spawn the gun into the backpack (not readied); wpc_hold() then puts it in hand."""
    try:
        recipe = wpc["recipes"][recipe_id]
        if find(recipe_id) is None:
            get_pc().GetPawnInventoryManager().AddBackpackWeaponFromDefinitionData(  # noqa: F821
                unrealsdk.make_struct("WeaponDefinitionData", **definition(recipe)))
            if find(recipe_id) is None:
                raise RuntimeError("spawned gun did not appear in the inventory")
            wpc["spawned"][recipe_id] = True
        return json.dumps({"ok": True, "id": recipe_id, "slot": 0})
    except Exception as error:
        return json.dumps({"ok": False, "id": recipe_id, "error": repr(error)})


def wpc_hold_slot1():
    """Hold the save's own quick-slot-1 weapon, so no spawned gun is in hand while earlier ones are removed."""
    _im, weapons, _backpack = inventory()
    first = next((w for w in weapons if slot_of(w) == 1), None)
    if first is not None:
        get_pc().Pawn.InvManager.SetCurrentWeapon(first, False)  # noqa: F821
    return json.dumps({"slot1": path(first)})


def wpc_holding(recipe_id):
    """True when this recipe's gun is the weapon in hand."""
    held = get_pc().Pawn.Weapon  # noqa: F821
    return json.dumps({"held": held is not None and matches(held, wpc["recipes"][recipe_id])})


def wpc_held_slot():
    """Quick slot of the weapon in hand (0: unslotted, e.g. a spawned gun), or -1 when nothing is held."""
    held = get_pc().Pawn.Weapon  # noqa: F821
    return json.dumps({"slot": slot_of(held) if held is not None else -1})


def wpc_prune(recipe_id):
    """Remove every other gun this script spawned that is not in hand (keeps the backpack to the current gun)."""
    held = get_pc().Pawn.Weapon  # noqa: F821
    removed, kept = [], []
    for other in [k for k in wpc["spawned"] if k != recipe_id]:
        old = find(other)
        if old is None or slot_of(old) != 0 or (held is not None and path(held) == path(old)):
            kept.append([other, path(old), slot_of(old) if old is not None else None])
        if old is not None and slot_of(old) == 0 and (held is None or path(held) != path(old)):
            # A gun that was held sits in the inventory chain, not in the Backpack array; remove it from whichever list.
            im = get_pc().GetPawnInventoryManager()  # noqa: F821
            if any(path(w) == path(old) for w in im.Backpack):
                im.RemoveInventoryFromBackpack(old)
            else:
                im.RemoveFromInventory(old)
            old.Destroy()
            del wpc["spawned"][other]
            removed.append(other)
    return json.dumps({"removed": removed, "kept": kept, "spawned": sorted(wpc["spawned"])})


def wpc_away():
    """Hold the weapon in quick slot 2 so slot 1 can be readied (a level-8 save has two weapon slots)."""
    _im, weapons, _backpack = inventory()
    other = next((w for w in weapons if slot_of(w) == 2), None)
    held = get_pc().Pawn.Weapon  # noqa: F821
    if other is not None and (held is None or slot_of(held) == 1):
        get_pc().Pawn.InvManager.SetCurrentWeapon(other, False)  # noqa: F821
    return json.dumps({"slot2": path(other), "held_slot": slot_of(held) if held else None})


def save_live():
    """Merge this session's records into RG_OUT/live_guns.json (earlier sessions' guns are kept)."""
    target = Path(RG_OUT, "live_guns.json")  # noqa: F821
    known = json.loads(target.read_text(encoding="utf-8")) if target.exists() else {}
    known.update(wpc["live"])
    target.write_text(json.dumps(known, indent=1), encoding="utf-8")


def wpc_hold(recipe_id):
    """Switch to the readied gun through the inventory manager (the slot key alone did not switch in the 2026-10-23 test)."""
    try:
        weapon = find(recipe_id)
        get_pc().Pawn.InvManager.SetCurrentWeapon(weapon, False)  # noqa: F821
        return json.dumps({"ok": weapon is not None})
    except Exception as error:
        return json.dumps({"ok": False, "error": repr(error)})


def wpc_close_dialogs():
    """Close open dialog boxes (the one-time "Unlocked!" notice waits on a profile write that the save block refuses)."""
    closed = []
    for dialog in unrealsdk.find_all("WillowGFxDialogBox", exact=False):
        if dialog.Name.startswith("Default__"):
            continue
        for call in ("Close", "OnClose"):
            try:
                getattr(dialog, call)()
                closed.append("%s.%s" % (dialog.Name, call))
                break
            except Exception:
                continue
    return json.dumps({"closed": closed})


def wpc_check(recipe_id):
    """After the switch: is the held weapon this recipe's gun? Then read what its first-person mesh draws."""
    try:
        held = get_pc().Pawn.Weapon  # noqa: F821
        ok = held is not None and matches(held, wpc["recipes"][recipe_id])
        record = live_record(held) if ok else {"held": path(held), "held_parts": parts_of(held) if held else None}
        record["held_is_recipe"] = ok
        record["held_slot"] = slot_of(held) if held is not None else None
        wpc["live"][recipe_id] = record
        save_live()
        return json.dumps({"ok": ok, "id": recipe_id, "triangles": record.get("triangles"), "slot": record["held_slot"],
                           "mesh_fields": sorted(record.get("mesh_fields") or [])})
    except Exception as error:
        return json.dumps({"ok": False, "id": recipe_id, "error": repr(error)})


def wpc_cleanup():
    """Put the original weapon back in its slot first (so no spawned gun is held), then remove every spawned gun."""
    restored = None
    original = wpc.get("original") or {}
    if original.get("balance"):
        back = next((w for w in get_pc().GetPawnInventoryManager().Backpack  # noqa: F821
                     if w.Class.Name == "WillowWeapon" and parts_of(w) == original["parts"]), None)
        if back is not None:
            slot = max(1, original["slot"])
            get_pc().GetPawnInventoryManager().SendSlottedThingToBackpack(slot)  # noqa: F821
            back = next((w for w in get_pc().GetPawnInventoryManager().Backpack  # noqa: F821
                         if w.Class.Name == "WillowWeapon" and parts_of(w) == original["parts"]), None)
            get_pc().GetPawnInventoryManager().ReadyBackpackInventory(back, slot)  # noqa: F821
            restored = path(back)
    removed = []
    for recipe_id in list(wpc["spawned"]):
        weapon = find(recipe_id)
        held = get_pc().Pawn.Weapon  # noqa: F821
        if weapon is None or (held is not None and path(held) == path(weapon)):
            continue
        if slot_of(weapon) > 0:
            get_pc().GetPawnInventoryManager().SendSlottedThingToBackpack(slot_of(weapon))  # noqa: F821
            weapon = find(recipe_id)
        if weapon is not None and slot_of(weapon) == 0:
            get_pc().GetPawnInventoryManager().RemoveInventoryFromBackpack(weapon)  # noqa: F821
            weapon.Destroy()
            removed.append(recipe_id)
            del wpc["spawned"][recipe_id]
    return json.dumps({"removed": removed, "restored": restored, "left": sorted(wpc["spawned"])})


for recipe_file in WPC_RECIPES:  # noqa: F821 (set by the session)
    data = json.loads(Path(recipe_file).read_text(encoding="utf-8-sig"))
    wpc["recipes"][Path(recipe_file).stem] = data
print(json.dumps({"loaded": sorted(wpc["recipes"])}))
