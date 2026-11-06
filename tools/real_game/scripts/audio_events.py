# Which sound events the running game posts, by labelled window (logging only; runs inside the game through
# openwillow_realgame, after phaselock.py). See tools/real_game/audio_events_capture.ps1.
#
# Hooks every script-visible function that takes a sound event (the list comes from the code packages' exports). A hook only
# records; it never blocks or changes a call. A call made from native code to native code passes no hook, so an empty result
# means "not seen here", not "the game plays nothing". Rows are game data: they stay under ignored local/realgame/.
#
# ae_install() -> which hooks exist         ae_begin(label) ... ae_end(baselines) -> what this window added
import json
import math
from pathlib import Path
import time

import unrealsdk
from unrealsdk.hooks import Type, add_hook, remove_hook

AE_HOOK = "ow_audio_events"
AE_MAX_ROWS = 20000
AE_FUNCTIONS = (
    "Engine.Actor:PostAkEvent",
    "Engine.Actor:PlayAkEvent",
    "Engine.Actor:Behavior_PostAkEvent",
    "Engine.Weapon:WeaponPlayAkEvent",
    "Engine.PlayerController:ClientPlayAkEvent",
    "Engine.WorldSoundManager:StaticHearWorldAkEvent",
    "Engine.WorldSoundManager:StaticPlayUIAkEvent",
    "GearboxFramework.Behavior_PostAkEvent:ApplyBehaviorToContext",
    "WillowGame.Behavior_PostAkEventGetRTPC:ApplyBehaviorToContext",
    "WillowGame.WillowPlayerController:PlayUIAkEvent",
    "WillowGame.WillowPlayerPawn:PlayUIAkEvent",
    "WillowGame.WillowPhysicalMaterialProperty:PlayImpactEffectSound",
    "WillowGame.WillowWeapon:PlayFiringSound",
)
ae = globals().setdefault("ae", {"label": None, "rows": [], "dropped": 0, "errors": 0, "seen": {}, "hooks": []})


def _ae_event(obj, args):
    """The sound event of a call: an argument of class AkEvent, else the behavior object's own Event; None when it has neither."""
    for prop in args._type._properties():
        value = getattr(args, prop.Name, None)
        if getattr(getattr(value, "Class", None), "Name", None) == "AkEvent":
            return value._path_name()
    own = getattr(obj, "Event", None)
    return own._path_name() if getattr(getattr(own, "Class", None), "Name", None) == "AkEvent" else None


def _ae_on_call(obj, args, _ret, func):
    # A failed read must never reach the game's call.
    if ae["label"] is None:
        return
    try:
        if len(ae["rows"]) >= AE_MAX_ROWS:
            ae["dropped"] += 1
            return
        ae["rows"].append({"t": time.perf_counter(), "fn": func.Name, "by": obj.Class.Name, "event": _ae_event(obj, args)})
    except Exception:
        ae["errors"] += 1


def ae_install():
    """Hook what exists and say what does not: a name that is missing is a gap in the result, not an error."""
    found, missing = [], []
    for path in AE_FUNCTIONS:
        try:
            unrealsdk.find_object("Function", path)
        except Exception:
            missing.append(path)
            continue
        add_hook(path, Type.PRE, AE_HOOK, _ae_on_call)
        found.append(path)
    ae["hooks"] = found
    return {"hooked": found, "missing": missing}


def ae_remove():
    for path in ae["hooks"]:
        remove_hook(path, Type.PRE, AE_HOOK)
    ae["hooks"] = []


def ae_begin(label):
    ae.update(label=label, rows=[], dropped=0, errors=0)
    return label


def _ae_key(row):
    return "%s|%s|%s" % (row["fn"], row["by"], row["event"])


def ae_end(baselines=()):
    """Close the window, write its rows and return its counts. `new` holds the events no baseline window saw, keyed by event only,
    so the same sound through another function does not count as new; a call with no event (the firing marker) is left out."""
    rows, label = ae["rows"], ae["label"]
    ae["label"] = None
    counts = {}
    for row in rows:
        counts[_ae_key(row)] = counts.get(_ae_key(row), 0) + 1
    events = {row["event"] for row in rows if row["event"]}
    ae["seen"][label] = events
    earlier = set().union(*(ae["seen"].get(name, set()) for name in baselines)) if baselines else set()
    if label is not None:
        with open(Path(RG_OUT) / ("events_%s.jsonl" % label), "w", encoding="utf-8") as out:
            for row in rows:
                out.write(json.dumps(row) + "\n")
    return {"label": label, "rows": len(rows), "dropped": ae["dropped"], "errors": ae["errors"], "counts": counts,
            "new": sorted(events - earlier), "firing_calls": sum(1 for row in rows if row["fn"] == "PlayFiringSound")}


def ae_health(name):
    """The target's health; 0 once it no longer exists, because killing it counts as a shot landing."""
    try:
        pawn = ai_pawn(name)
    except LookupError:
        return 0.0
    try:
        return float(pawn.GetHealth())
    except Exception:
        return float(pawn.Health)


def ae_weapon():
    """What Maya holds: the fire sound belongs to this gun, whichever it is."""
    try:
        weapon = get_pc().Pawn.Weapon
        if weapon is None:
            return None
        data = weapon.DefinitionData
        return {"name": weapon.GetShortHumanReadableName(), "type": data.WeaponTypeDefinition._path_name() if data.WeaponTypeDefinition else None,
                "balance": data.BalanceDefinition._path_name() if data.BalanceDefinition else None}
    except Exception as error:
        return {"error": str(error)}     # a label, not worth losing the session for


def ae_throw():
    """Drop one backpack item so there is something to pick up. The save it came from is restored after the run."""
    try:
        manager = get_pc().GetPawnInventoryManager()
        items = list(manager.Backpack)
        if not items:
            return {"thrown": None}
        item = items[0]
        label = "%s:%s" % (item.Class.Name, item.GetShortHumanReadableName())
        manager.ThrowBackpackInventory(item)
        return {"thrown": label, "backpack_before": len(items)}
    except Exception as error:
        return {"thrown": None, "error": str(error)}     # the pickup stage then ends incomplete; the others keep their results


def _ae_pickups():
    me = get_pc().Pawn.Location
    rows = []
    for pickup in unrealsdk.find_all("WillowPickup"):
        if "Default__" in pickup._path_name() or pickup.bDeleteMe or pickup.Inventory is None:
            continue
        where = pickup.Location
        rows.append((math.dist((me.X, me.Y, me.Z), (where.X, where.Y, where.Z)), pickup.Name, pickup))
    return sorted(rows, key=lambda row: row[0])


def ae_face_pickup():
    """Stand 120 units from the nearest pickup, looking at it. None when there is none."""
    try:
        rows = _ae_pickups()
        if not rows:
            return None
        _dist, name, pickup = rows[0]
        pc, where = get_pc(), pickup.Location
        pc.Pawn.Location = unrealsdk.make_struct("Vector", X=where.X - 120.0, Y=where.Y, Z=where.Z + 60.0)
        eye = pc.Pawn.Location.Z + pc.Pawn.BaseEyeHeight
        pitch = math.degrees(math.atan2(where.Z - eye, 120.0))
        pc.Rotation = unrealsdk.make_struct("Rotator", Pitch=int(pitch * 65536 / 360) & 0xFFFF, Yaw=0, Roll=0)
        return name
    except Exception:
        return None


def ae_pickup_gone(name):
    return all(row[1] != name for row in _ae_pickups())
