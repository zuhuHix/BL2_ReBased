"""Read-side live weapon parity probe for the Sanctuary slice.

Run through Invoke-GamePyFile after Maya is in Sanctuary, with saves blocked.
It reads the four level-8 slice recipes from local/items/slice, spawns exact
WeaponDefinitionData part references into memory, and writes all game-derived
output under local/realgame/laneE/. It never launches the game or writes saves.

After the initial setup, the command namespace exposes:
  wp_equip(recipe_id, slot_1_to_4)
  wp_sample(label)                    one state snapshot
  wp_watch_start(name) / wp_watch_stop()  per-tick state + weapon-call markers
  wp_face_wall() / wp_reface()        turn to a flat wall a few metres away / re-apply that view
  wp_restore_slot(slot_1_to_4)        put the pre-probe weapon back in its quick slot
  wp_cleanup()                        remove only weapons this probe spawned

Property reads follow weapon_dump.py; exact item construction and inventory
lookups follow weapon_cards.py. Object references are re-looked-up after every
command or inventory mutation because stale SDK wrappers can crash the game.
"""
import enum
import json
import math
import time
from pathlib import Path

try:
    import unrealsdk
    from unrealsdk.hooks import Type, add_hook, remove_hook
except ImportError:  # allows the pure recipe helper to be tested outside BL2
    unrealsdk = None
    Type = add_hook = remove_hook = None


RECIPE_IDS = (
    "slice_pistol",
    "slice_mission_pistol_fire",
    "slice_assault_rifle",
    "slice_smg",
)
SLOTS = ("Body", "Grip", "Barrel", "Sight", "Stock", "Elemental", "Accessory1", "Accessory2", "Material")
TYPE_FIELDS = (
    "FireRate", "ReloadTime", "ClipSize", "Spread", "PlayerViewOffset", "FirstPersonMeshFOV",
    "ZoomFOV", "ZoomedFOV", "ZoomWeaponAttributeEffects", "WeaponAttributeEffects",
    "WeaponEquipAnimations", "WeaponFireAnimations", "WeaponReloadAnimations", "MuzzleFlashSocket",
    "MuzzleFlashDuration", "MuzzleFlashPSTemplates", "MuzzleFlashLightTemplate", "TracerTemplate",
    "TracerColor", "TracerColorParameterName", "FireSound", "FiringSound", "WeaponFireSound",
    "ReloadSound", "WeaponReloadSound", "FireShake", "ViewModelLeadPivotName",
    "ViewModelRotationOriginOffset", "ViewModelRotationAmt", "ViewModelTranslationAmt", "LeadingSpeed",
    "MaxPitchLead", "MaxYawLead", "BobDamping", "ShellCasingPSCTemplate", "ShellCasingSocket",
    "AttachmentData", "MaterialClass", "BarrelSpinMode", "MinFireAnimDuration",
)
TYPE_TERMS = (
    "fire", "reload", "zoom", "aim", "ads", "offset", "recoil", "kick", "sway", "lead", "bob",
    "muzzle", "tracer", "impact", "hit", "effect", "particle", "template", "sound", "anim", "shell",
    "casing", "spin", "shake", "spread", "fov",
)
PART_FIELDS = (
    "NongestaltSkeletalMesh", "GestaltModeSkeletalMeshName", "AdditionalGestaltModeSkeletalMeshNames",
    "Material", "MaterialVectorParameterValues", "CustomFiringModeDefinition", "CustomDamageTypeDefinition",
    "WeaponAttributeEffects", "AttributeSlotUpgrades", "ZoomWeaponAttributeEffects", "AltMuzzleFlashSockets",
    "ShellCasingPSCTemplate", "ShellCasingSocket", "bOverrideShellCasing", "MuzzleFlashPSTemplates",
)
PART_TERMS = ("mesh", "material", "fire", "reload", "zoom", "aim", "ads", "offset", "muzzle", "tracer",
              "impact", "hit", "effect", "particle", "template", "sound", "shell", "casing", "recoil", "kick", "anim")
WEAPON_FIELDS = (
    "FireInterval", "FireRate", "ReloadTime", "ClipSize", "Spread", "ProjectilesPerShot", "ShotCost",
    "InstantHitDamage", "StatusEffectDamage", "CurrentAmmo", "CurrentAmmoInClip", "AmmoInClip",
    "AmmoCount", "ReplicatedReloadState", "bIsZoomed", "bZoomed", "bZooming", "bIsReloading",
)
WEAPON_TERMS = ("fireinterval", "reloadtime", "clipsize", "projectile", "shotcost", "spread", "ammo", "zoom", "aim",
                "ads", "muzzle", "tracer", "effect", "hit", "sound")
TICK_TERMS = ("ammo", "clip", "reload", "zoom")
PC_FIELDS = ("FOVAngle", "DefaultFOV", "ForegroundFOV", "bForegroundFOV", "Rotation")
PC_TERMS = ("fov", "zoom", "aim", "ads", "foreground")
COMPONENT_FIELDS = (
    "SkeletalMesh", "StaticMesh", "Template", "AnimSet", "AnimTreeTemplate", "RelativeLocation",
    "RelativeRotation", "RelativeScale3D", "Location", "Rotation", "Scale", "Scale3D", "AttachSocketName",
)
COMPONENT_TOKENS = ("arms", "mesh", "muzzle", "flash", "tracer", "shell", "weapon")
EVENT_METHODS = frozenset((
    "BeginReload", "StartReload", "ReloadDone", "StopReloading", "OnAbortReload", "TryPutDown",
    "PutDownWeapon", "WeaponPuttingDown", "Fire", "FireWeapon", "StartFire", "StopFire",
    "PlayFireEffects", "PlayFiringEffects", "FireEffects", "ZoomIn", "ZoomOut", "StartZoom", "StopZoom",
    "ApplyZoom", "FireAmmunition", "ConsumeAmmo", "InstantFire", "ProjectileFire", "CustomFire",
    "WeaponFired", "IncrementFlashCount", "BeginFire", "EndFire", "SendToFiringState", "RefireCheckTimer",
    "ShouldRefire", "PlayReloadAnimation", "ReloadComplete", "OnReloadComplete", "AddAmmo",
))
TICK_PATH = "WillowGame.WillowGameViewportClient:Tick"
HOOK_ID = "ow_weapon_parity_probe"
MAX_DEPTH = 6
MAX_ARRAY = 96

wp_state = globals().setdefault("wp_state", {"paths": {}, "spawned": set(), "watch": None})


def definition_fields(recipe):
    """Build a synthetic-testable WeaponDefinitionData field dict from one local recipe."""
    if not isinstance(recipe, dict):
        raise TypeError("recipe must be a dictionary")
    required = {
        "WeaponTypeDefinition": recipe.get("weapon_type"),
        "BalanceDefinition": recipe.get("balance"),
        "ManufacturerDefinition": recipe.get("manufacturer"),
    }
    missing = [key for key, value in required.items() if not value]
    if missing:
        raise ValueError("recipe is missing: " + ", ".join(missing))

    stage = recipe.get("game_stage", recipe.get("level", 8))
    fields = dict(required)
    fields["GameStage"] = int(stage)
    # Slice recipes are prepared at stage 8 and the existing live definitions
    # use ManufacturerGradeIndex 8. Preserve explicit values if a recipe adds one.
    fields["ManufacturerGradeIndex"] = int(recipe.get("manufacturer_grade_index", recipe.get("grade", stage)))
    for slot in SLOTS:
        value = recipe.get("parts", {}).get(slot)
        if isinstance(value, dict):
            value = value.get("part")
        if value:
            fields[slot + "PartDefinition"] = value
    for recipe_key, field in (("prefix", "PrefixPartDefinition"), ("title", "TitlePartDefinition")):
        value = recipe.get(recipe_key)
        if isinstance(value, dict):
            value = value.get("part")
        if value:
            fields[field] = value
    return fields


def _path(value):
    if value is None:
        return None
    try:
        return value._path_name()
    except Exception:
        return value if isinstance(value, str) else None


def _is_uobject(value):
    return value is not None and hasattr(value, "_path_name") and hasattr(value, "Class")


def _is_wrapped_struct(value):
    try:
        return callable(value._type._properties)
    except Exception:
        return False


def _is_wrapped_array(value):
    return type(value).__name__.lower().startswith("wrappedarray")


def _deep(value, depth=0):
    # Enums only: SDK attribute lookup is case-insensitive, so getattr(obj, "name") on a UObject or
    # struct returns its Name property and would collapse object paths to short names.
    if isinstance(value, enum.Enum):
        return value.name
    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    if _is_uobject(value):
        return _path(value)
    if depth >= MAX_DEPTH:
        return repr(value)[:180]
    if _is_wrapped_struct(value):
        result = {}
        for prop in value._type._properties():
            name = prop.Name
            try:
                result[name] = _deep(getattr(value, name), depth + 1)
            except Exception:
                continue
        return result
    if isinstance(value, (list, tuple)) or _is_wrapped_array(value):
        try:
            values = list(value)
        except Exception:
            return repr(value)[:180]
        return [_deep(item, depth + 1) for item in values[:MAX_ARRAY]]
    return repr(value)[:180]


_FIELD_CACHE = {}


def _field_names(obj):
    """Property names of an object's class, inherited ones included (UObjects expose fields via .Class)."""
    try:
        cls = obj.Class
        key = cls._path_name()
    except Exception:
        return []
    if key not in _FIELD_CACHE:
        try:
            _FIELD_CACHE[key] = [field.Name for field in cls._fields()
                                 if getattr(getattr(field, "Class", None), "Name", "").endswith("Property")]
        except Exception:
            _FIELD_CACHE[key] = []
    return _FIELD_CACHE[key]


def _snapshot(obj, explicit=(), terms=()):
    if obj is None:
        return {}
    names = list(explicit)
    lower_terms = tuple(term.lower() for term in terms)
    for name in _field_names(obj):
        low = name.lower()
        if any(term in low for term in lower_terms) and name not in names:
            names.append(name)
    result = {}
    for name in names:
        try:
            value = getattr(obj, name)
        except Exception:
            continue
        if callable(value):
            continue
        result[name] = _deep(value)
    return result


def _component_snapshot(component, include_sockets=False):
    if not _is_uobject(component):
        return None
    record = {
        "object": _path(component),
        "class": component.Class.Name,
        "properties": _snapshot(component, COMPONENT_FIELDS, ()),
    }
    if include_sockets:
        getter = getattr(component, "GetSocketLocation", None)
        if callable(getter):
            sockets = {}
            for name in ("Muzzle", "MuzzleFlashSocket", "EjectPort", "Weapon"):
                try:
                    sockets[name] = _deep(getter(name))
                except Exception:
                    continue
            if sockets:
                record["socket_world_locations"] = sockets
    return record


def _component_refs(obj, include_sockets=False):
    result = {}
    if obj is None:
        return result
    for name in _field_names(obj):
        if not any(token in name.lower() for token in COMPONENT_TOKENS):
            continue
        try:
            value = getattr(obj, name)
        except Exception:
            continue
        if _is_uobject(value) and "component" in value.Class.Name.lower():
            result[name] = _component_snapshot(value, include_sockets)
    return result


def _safe_call(obj, name):
    try:
        method = getattr(obj, name)
        if callable(method):
            return _deep(method())
    except Exception:
        return None
    return None


def _slot_parts(definition):
    parts = {}
    for slot in SLOTS:
        field = slot + "PartDefinition"
        try:
            value = getattr(definition, field)
        except Exception:
            value = None
        if value is not None:
            parts[slot] = _path(value)
    return parts


def _inventory_weapons():
    im = get_pc().GetPawnInventoryManager()
    chain, item = [], im.InventoryChain
    while item:
        chain.append(item)
        item = item.Inventory
    equipped = [weapon for weapon in chain if weapon.Class.Name == "WillowWeapon"]
    backpack = [weapon for weapon in im.Backpack if weapon.Class.Name == "WillowWeapon"]
    return im, equipped, backpack


def _matches_recipe(weapon, recipe):
    expected = definition_fields(recipe)
    try:
        data = weapon.DefinitionData
    except Exception:
        return False
    for key, wanted in expected.items():
        try:
            got = getattr(data, key)
        except Exception:
            return False
        if key.endswith("Definition"):
            if _path(got) != wanted:
                return False
        elif got != wanted:
            return False
    return True


def _weapon_record(weapon):
    data = weapon.DefinitionData
    return {
        "object": _path(weapon),
        "class": weapon.Class.Name,
        "name": _safe_call(weapon, "GetShortHumanReadableName"),
        "full_name": _safe_call(weapon, "GenerateHumanReadableName"),
        "balance": _path(data.BalanceDefinition),
        "type": _path(data.WeaponTypeDefinition),
        "manufacturer": _path(data.ManufacturerDefinition),
        "grade": _deep(data.ManufacturerGradeIndex),
        "game_stage": _deep(data.GameStage),
        "parts": _slot_parts(data),
        "prefix": _path(getattr(data, "PrefixPartDefinition", None)),
        "title": _path(getattr(data, "TitlePartDefinition", None)),
        "quick_select_slot": _deep(getattr(weapon, "QuickSelectSlot", None)),
        "sale_value": _safe_call(weapon, "GetMonetaryValue"),
    }


def _capture_weapon(weapon, pc=None):
    pc = pc or get_pc()
    pawn = pc.Pawn
    definition = weapon.DefinitionData
    type_definition = definition.WeaponTypeDefinition
    parts = {}
    for slot, path in _slot_parts(definition).items():
        part = unrealsdk.find_object("Object", path)
        parts[slot] = {"path": path, "properties": _snapshot(part, PART_FIELDS, PART_TERMS)}
    return {
        "record": _weapon_record(weapon),
        "live_weapon": _snapshot(weapon, WEAPON_FIELDS, WEAPON_TERMS),
        "weapon_state": _weapon_state(weapon),
        "weapon_type": {
            "path": _path(type_definition),
            "properties": _snapshot(type_definition, TYPE_FIELDS, TYPE_TERMS),
        },
        "parts": parts,
        "controller": _snapshot(pc, PC_FIELDS, PC_TERMS),
        "pawn_components": _component_refs(pawn, include_sockets=True),
        "weapon_components": _component_refs(weapon, include_sockets=True),
    }


def _weapon_state(weapon):
    if weapon is None:
        return None
    states = ("WeaponReloading", "WeaponPuttingDown", "WeaponEquipping", "WeaponFiring", "Active", "Inactive")
    active = []
    for state in states:
        try:
            if weapon.IsInState(state):
                active.append(state)
        except Exception:
            continue
    return {
        "active_states": active,
        "reload_state": _deep(getattr(weapon, "ReplicatedReloadState", None)),
        "putting_down": bool(_safe_call(weapon, "IsPuttingDown")),
        "equipping": bool(_safe_call(weapon, "IsEquipping")),
        "zoom_fields": _snapshot(weapon, ("bIsZoomed", "bZoomed", "bZooming", "bAiming", "bADS"), ()),
    }


def _repo_root():
    try:
        return Path(RG_OUT).resolve().parents[2]
    except Exception as error:
        raise RuntimeError("RG_OUT is missing; invoke this with Invoke-GamePyFile") from error


def _out_dir():
    folder = Path(RG_OUT)
    folder.mkdir(parents=True, exist_ok=True)
    return folder


def _load_recipes():
    folder = _repo_root() / "local" / "items" / "slice"
    recipes = {}
    for recipe_id in RECIPE_IDS:
        path = folder / (recipe_id + ".json")
        if not path.is_file():
            raise FileNotFoundError("missing local slice recipe: " + str(path))
        recipes[recipe_id] = json.loads(path.read_text(encoding="utf-8"))
    return folder, recipes


def wp_seed():
    """Block saves and create/find the four exact-part slice weapons in memory."""
    if unrealsdk is None:
        raise RuntimeError("unrealsdk is available only inside the running game")
    blocker = globals().get("block_saves")
    if not callable(blocker):
        raise RuntimeError("real-game driver block_saves() helper is missing")
    blocker()
    if get_pc() is None or get_pc().Pawn is None:
        raise RuntimeError("load Maya into Sanctuary before running this probe")

    folder, recipes = _load_recipes()
    im, equipped, backpack = _inventory_weapons()
    if "original_slots" not in wp_state:
        wp_state["original_slots"] = {_slot(weapon): _path(weapon) for weapon in equipped if _slot(weapon) > 0}
        wp_state["original_prints"] = {_slot(weapon): _fingerprint(weapon) for weapon in equipped if _slot(weapon) > 0}
    all_weapons = equipped + backpack
    report = {
        "warning": "Game-derived live values are local output; grade fallback and native semantics remain UNVERIFIED.",
        "recipes_root": str(folder),
        "created_at_qpc": time.perf_counter(),
        "weapons": [],
    }
    for recipe_id, recipe in recipes.items():
        matches = [weapon for weapon in all_weapons if _matches_recipe(weapon, recipe)]
        created = False
        if not matches:
            before = {_path(weapon) for weapon in all_weapons}
            fields = definition_fields(recipe)
            values = {}
            for key, value in fields.items():
                if isinstance(value, str) and key.endswith("Definition"):
                    obj = unrealsdk.find_object("Object", value)
                    if obj is None:
                        raise LookupError("could not resolve " + key + ": " + value)
                    values[key] = obj
                else:
                    values[key] = value
            im.AddBackpackWeaponFromDefinitionData(unrealsdk.make_struct("WeaponDefinitionData", **values))
            # Re-read the chain and backpack after the mutation; SDK wrappers can go stale across inventory changes.
            im, equipped, backpack = _inventory_weapons()
            all_weapons = equipped + backpack
            matches = [weapon for weapon in all_weapons if _matches_recipe(weapon, recipe)]
            if not matches:
                new_paths = [_path(weapon) for weapon in all_weapons if _path(weapon) not in before]
                raise RuntimeError("exact recipe did not appear in inventory: " + recipe_id + " new=" + repr(new_paths))
            created = True
        weapon = matches[0]
        path = _path(weapon)
        wp_state["paths"][recipe_id] = path
        wp_state.setdefault("recipes", {})[recipe_id] = recipe
        if created:
            wp_state["spawned"].add(recipe_id)
        report["weapons"].append({
            "recipe_id": recipe_id,
            "recipe_path": "local/items/slice/" + recipe_id + ".json",
            "newly_spawned_by_probe": created,
            "source_card": recipe.get("stats", {}).get("card", {}),
            "capture": _capture_weapon(weapon),
        })

    out_path = _out_dir() / "weapon_parity_setup.json"
    out_path.write_text(json.dumps(report, indent=1, ensure_ascii=False), encoding="utf-8")
    return {"path": str(out_path), "weapons": len(report["weapons"]),
            "spawned": sorted(wp_state["spawned"]), "original_slots": wp_state["original_slots"]}


def _slot(weapon):
    """Quick slot 1..4 of a weapon; 0 when unslotted. QuickSelectSlot is an enum whose values are the slot numbers."""
    try:
        return int(weapon.QuickSelectSlot)
    except Exception:
        return 0


def _fingerprint(weapon):
    """Identity of a weapon that survives readying (which creates a new actor): its definition data."""
    data = weapon.DefinitionData
    return json.dumps([_path(data.BalanceDefinition), _path(data.WeaponTypeDefinition), _slot_parts(data),
                       _deep(data.ManufacturerGradeIndex), _deep(data.GameStage)], sort_keys=True)


def _loose_match(weapon, recipe):
    """Balance and slot parts only; used to report a recipe weapon whose grade/stage changed on readying."""
    expected = definition_fields(recipe)
    try:
        data = weapon.DefinitionData
        return all(_path(getattr(data, key)) == value for key, value in expected.items()
                   if key.endswith("Definition") and key != "ManufacturerDefinition")
    except Exception:
        return False


def _find_recipe(recipe_id):
    """The live weapon built from this recipe. Readying a backpack weapon creates a new actor, so match by
    definition data rather than by a remembered object path. Held/slotted copies come first."""
    recipe = wp_state.get("recipes", {}).get(recipe_id)
    if recipe is None:
        raise LookupError("unknown recipe id; run wp_seed first: " + recipe_id)
    _im, equipped, backpack = _inventory_weapons()
    return next((weapon for weapon in equipped + backpack if _matches_recipe(weapon, recipe)), None)


def wp_equip(recipe_id, slot):
    """Ready a probed recipe in quick slot 1..4; the slot must not hold the weapon in hand."""
    slot = int(slot)
    if slot < 1 or slot > 4:
        raise ValueError("slot must be 1..4")
    weapon = _find_recipe(recipe_id)
    if weapon is None:
        raise LookupError("probed weapon is no longer in inventory: " + recipe_id)
    held = get_pc().Pawn.Weapon
    if _slot(weapon) == slot:
        return {"recipe_id": recipe_id, "slot": slot, "weapon": _path(weapon), "already_slotted": True}
    if held is not None and _slot(held) == slot:
        raise RuntimeError("switch away from slot %d before replacing it" % slot)
    if held is not None and _path(held) == _path(weapon):
        raise RuntimeError("switch away from the held probe weapon before moving it")
    im = get_pc().GetPawnInventoryManager()
    if _slot(weapon) > 0:
        im.SendSlottedThingToBackpack(_slot(weapon))
    im = get_pc().GetPawnInventoryManager()
    im.SendSlottedThingToBackpack(slot)
    weapon = next((item for item in get_pc().GetPawnInventoryManager().Backpack
                   if item.Class.Name == "WillowWeapon" and _matches_recipe(item, wp_state["recipes"][recipe_id])), None)
    if weapon is None:
        raise LookupError("probe weapon not in backpack after clearing destination slot")
    get_pc().GetPawnInventoryManager().ReadyBackpackInventory(weapon, slot)
    ready = _find_recipe(recipe_id)
    _im, equipped, backpack = _inventory_weapons()
    result = {"recipe_id": recipe_id, "slot": slot, "weapon": _path(ready), "now_in_slot": _slot(ready) if ready else None,
              "equipped": [(_slot(w), _safe_call(w, "GetShortHumanReadableName"), _path(w)) for w in equipped]}
    if ready is None:
        loose = [w for w in equipped + backpack if _loose_match(w, wp_state["recipes"][recipe_id])]
        result["loose_matches"] = [(_path(w), _slot(w), _deep(w.DefinitionData.GameStage),
                                    _deep(w.DefinitionData.ManufacturerGradeIndex)) for w in loose]
        result["backpack_count"] = len(backpack)
    else:
        wp_state["paths"][recipe_id] = _path(ready)
    return result


def wp_held_is(recipe_id):
    weapon = get_pc().Pawn.Weapon
    return weapon is not None and _matches_recipe(weapon, wp_state["recipes"][recipe_id])


def wp_restore_slot(slot):
    """Put the weapon that was in quick slot 1..4 before wp_seed back there (it must not be the held slot)."""
    slot = int(slot)
    wanted = wp_state.get("original_prints", {}).get(slot)
    if wanted is None:
        return None
    held = get_pc().Pawn.Weapon
    if held is not None and _fingerprint(held) == wanted:
        return {"slot": slot, "already_held": True}
    if held is not None and _slot(held) == slot:
        raise RuntimeError("switch away from slot %d before restoring it" % slot)
    im, equipped, _backpack = _inventory_weapons()
    if any(_slot(weapon) == slot and _fingerprint(weapon) == wanted for weapon in equipped):
        return {"slot": slot, "already_slotted": True}
    im.SendSlottedThingToBackpack(slot)
    weapon = next((item for item in get_pc().GetPawnInventoryManager().Backpack
                   if item.Class.Name == "WillowWeapon" and _fingerprint(item) == wanted), None)
    if weapon is None:
        raise LookupError("original weapon for slot %d not in backpack" % slot)
    get_pc().GetPawnInventoryManager().ReadyBackpackInventory(weapon, slot)
    return {"slot": slot, "restored": _safe_call(weapon, "GetShortHumanReadableName")}


def _vec(x, y, z):
    return unrealsdk.make_struct("Vector", X=float(x), Y=float(y), Z=float(z))


def _hit_distance(pawn, start, yaw, pitch=0.0, max_dist=1500.0):
    """Distance along (yaw, pitch) degrees to world geometry by bisecting Actor.FastTrace; None if clear."""
    dx = math.cos(math.radians(pitch)) * math.cos(math.radians(yaw))
    dy = math.cos(math.radians(pitch)) * math.sin(math.radians(yaw))
    dz = math.sin(math.radians(pitch))
    begin = _vec(*start)

    def clear(dist):
        return bool(pawn.FastTrace(_vec(start[0] + dx * dist, start[1] + dy * dist, start[2] + dz * dist), begin))

    if clear(max_dist):
        return None
    low, high = 0.0, max_dist
    for _ in range(14):
        mid = (low + high) / 2
        if clear(mid):
            low = mid
        else:
            high = mid
    return high


def _set_view(yaw, pitch=0.0):
    pc = get_pc()
    pc.SetRotation(unrealsdk.make_struct("Rotator", Pitch=int(pitch * 65536 / 360) & 0xFFFF,
                                         Yaw=int(yaw * 65536 / 360) & 0xFFFF, Roll=0))


def wp_face_wall(min_dist=250.0, max_dist=550.0, step=5.0):
    """Face the flattest wall whose perpendicular distance from the eye is in [min_dist, max_dist] cm.

    Sweeps yaw at eye height with Actor.FastTrace, fits a line through hits at yaw-8/yaw/yaw+8 and
    turns to the wall normal. Writes nothing; returns the chosen view so later calls can re-apply it.
    """
    pawn = get_pc().Pawn
    loc = pawn.Location
    start = (loc.X, loc.Y, loc.Z + float(getattr(pawn, "BaseEyeHeight", 60.0)))
    best = None
    yaw = 0.0
    while yaw < 360.0:
        points = []
        for offset in (-8.0, 0.0, 8.0):
            dist = _hit_distance(pawn, start, yaw + offset)
            if dist is None:
                break
            angle = math.radians(yaw + offset)
            points.append((start[0] + math.cos(angle) * dist, start[1] + math.sin(angle) * dist))
        if len(points) == 3:
            (ax, ay), (bx, by), (cx, cy) = points
            length = math.hypot(cx - ax, cy - ay)
            if length > 1.0:
                # Residual of the middle hit from the outer chord, and the eye's perpendicular distance.
                residual = abs((cx - ax) * (ay - by) - (ax - bx) * (cy - ay)) / length
                perp = abs((cx - ax) * (ay - start[1]) - (ax - start[0]) * (cy - ay)) / length
                normal_yaw = math.degrees(math.atan2(cx - ax, -(cy - ay)))
                facing = normal_yaw if math.cos(math.radians(normal_yaw - yaw)) > 0 else normal_yaw + 180.0
                vertical = [_hit_distance(pawn, start, facing, pitch) for pitch in (-12.0, 12.0)]
                flat_vertical = all(v is not None for v in vertical) and abs(vertical[0] - vertical[1]) < 0.05 * perp + 5
                if min_dist <= perp <= max_dist and residual < 4.0 and flat_vertical:
                    score = residual + abs(perp - (min_dist + max_dist) / 2) / 50.0
                    if best is None or score < best["score"]:
                        best = {"yaw": facing % 360.0, "distance": perp, "residual": residual, "score": score}
        yaw += step
    if best is None:
        # Keep the current view as the fixed one so wp_reface still undoes recoil drift.
        rotation = get_pc().Rotation
        wp_state["view"] = {"yaw": (rotation.Yaw & 0xFFFF) * 360.0 / 65536, "kept_current_view": True}
        return {"found": False, "location": list(start), "view": wp_state["view"]}
    _set_view(best["yaw"], 0.0)
    wp_state["view"] = best
    best = dict(best, found=True, location=list(start))
    return best


def wp_reface():
    """Re-apply the view chosen by wp_face_wall (recoil moves the camera between captures)."""
    view = wp_state.get("view")
    if view is None:
        return None
    _set_view(view["yaw"], 0.0)
    return view


def wp_sample(label):
    """Write one live state snapshot; use the same label for its matching saved screenshot."""
    pc = get_pc()
    pawn = pc.Pawn if pc else None
    weapon = pawn.Weapon if pawn else None
    row = {"label": str(label), "t": time.perf_counter(),
           "game_t": _deep(pc.WorldInfo.TimeSeconds) if pc else None,
           "held_weapon": _path(weapon) if weapon else None}
    if weapon:
        row["capture"] = _capture_weapon(weapon, pc)
    path = _out_dir() / "weapon_parity_samples.jsonl"
    with path.open("a", encoding="utf-8") as stream:
        stream.write(json.dumps(row, ensure_ascii=False) + "\n")
    return {"path": str(path), "label": str(label), "weapon": row["held_weapon"]}


def _event_paths():
    cls = unrealsdk.find_class("WillowWeapon")
    paths = []
    while cls:
        for field in cls._fields():
            if field.Class.Name == "Function" and field.Name in EVENT_METHODS and field.Outer == cls:
                paths.append(field._path_name())
        cls = cls.SuperField
    return paths


def _watch_write(row):
    watch = wp_state.get("watch")
    if watch is None:
        return
    try:
        watch["file"].write(json.dumps(row, ensure_ascii=False) + "\n")
        watch["file"].flush()
    except Exception:
        pass


def _watch_tick(*_args):
    try:
        pc = get_pc()
        pawn = pc.Pawn if pc else None
        weapon = pawn.Weapon if pawn else None
        row = {"kind": "tick", "t": time.perf_counter(),
               "game_t": _deep(pc.WorldInfo.TimeSeconds) if pc else None,
               "weapon": _path(weapon) if weapon else None}
        if weapon:
            row["name"] = _safe_call(weapon, "GetShortHumanReadableName")
            row["state"] = _weapon_state(weapon)
            row["stats"] = _snapshot(weapon, WEAPON_FIELDS, TICK_TERMS)
        if pc:
            row["controller"] = _snapshot(pc, PC_FIELDS, ())
            row["pawn_fov"] = _snapshot(pawn, (), ("fov", "zoom")) if pawn else None
        _watch_write(row)
    except Exception as error:
        _watch_write({"kind": "sample_error", "t": time.perf_counter(), "error": repr(error)[:240]})


def _watch_event(obj, _args, _ret, func):
    try:
        pc = get_pc()
        weapon = pc.Pawn.Weapon if pc and pc.Pawn else None
        _watch_write({"kind": "weapon_call", "t": time.perf_counter(),
                      "game_t": _deep(pc.WorldInfo.TimeSeconds) if pc else None,
                      "func": func.func.Name, "object": _path(obj),
                      "name": _safe_call(obj, "GetShortHumanReadableName"),
                      "held": _path(weapon) if weapon else None,
                      "state": _weapon_state(obj)})
    except Exception as error:
        _watch_write({"kind": "event_error", "t": time.perf_counter(), "error": repr(error)[:240]})


def wp_watch_start(name="weapon_parity_events.jsonl"):
    """Sample the held weapon every game tick and mark reload/fire/zoom method calls."""
    wp_watch_stop()
    if unrealsdk is None:
        raise RuntimeError("unrealsdk is available only inside the running game")
    file_name = Path(str(name)).name
    if not file_name.endswith(".jsonl"):
        file_name += ".jsonl"
    path = _out_dir() / file_name
    stream = path.open("w", encoding="utf-8")
    paths = _event_paths()
    wp_state["watch"] = {"file": stream, "path": str(path), "event_paths": paths}
    add_hook(TICK_PATH, Type.POST_UNCONDITIONAL, HOOK_ID, _watch_tick)
    for event_path in paths:
        add_hook(event_path, Type.PRE, HOOK_ID, _watch_event)
    return {"path": str(path), "hooked_functions": paths}


def wp_watch_stop():
    watch = wp_state.get("watch")
    if watch is None:
        return None
    try:
        remove_hook(TICK_PATH, Type.POST_UNCONDITIONAL, HOOK_ID)
        for event_path in watch["event_paths"]:
            remove_hook(event_path, Type.PRE, HOOK_ID)
    finally:
        watch["file"].close()
        wp_state["watch"] = None
    return {"path": watch["path"]}


def wp_cleanup():
    """Remove only probe-created guns (one per recipe it spawned). Switch away from one if it is held."""
    if wp_state.get("watch") is not None:
        wp_watch_stop()
    removed = []
    for recipe_id in sorted(wp_state["spawned"]):
        weapon = _find_recipe(recipe_id)
        if weapon is None:
            wp_state["spawned"].discard(recipe_id)
            continue
        held = get_pc().Pawn.Weapon
        if held is not None and _path(held) == _path(weapon):
            raise RuntimeError("switch away from this probe-created weapon before cleanup: " + recipe_id)
        if _slot(weapon) > 0:
            get_pc().GetPawnInventoryManager().SendSlottedThingToBackpack(_slot(weapon))
        im, _equipped, backpack = _inventory_weapons()
        weapon = next((item for item in backpack if _matches_recipe(item, wp_state["recipes"][recipe_id])), None)
        if weapon is None:
            raise RuntimeError("could not find probe weapon in backpack for cleanup: " + recipe_id)
        path = _path(weapon)
        im.RemoveInventoryFromBackpack(weapon)
        weapon.Destroy()
        removed.append(recipe_id + " " + path)
        wp_state["spawned"].discard(recipe_id)
    return {"removed": removed, "remaining": sorted(wp_state["spawned"])}


if "RG_OUT" in globals():
    print(json.dumps(wp_seed(), ensure_ascii=False))
