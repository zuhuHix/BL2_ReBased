"""HUD parity live reads: the player's HUD movie, equipped weapon, ammo and vitals in the running game.

Our own code for the openwillow_realgame command channel. tools/real_game/hud_parity_capture.ps1 runs it through
Invoke-GamePyFile and writes what it returns under the ignored local/realgame/hud_parity/ (values.json, session.json,
save_compare.json and the screenshots). That output is game data: it stays there and is never committed.

Read-only: no property is set, nothing is spawned or equipped and no save is written.

- hp_snapshot(label): one dict with
  - weapon: the pawn's Weapon (class, path, the instance's ammo/clip fields that answer, DefinitionData's properties,
    and the simple properties of its WeaponTypeDefinition and BalanceDefinition);
  - hud: PlayerController.myHUD.HUDMovie read with GetVariableNumber / GetVariableBool / GetVariableString on the p1
    crosshair, vitals, ammo and grenade clips (frame numbers, visibility and the text fields);
  - pools: every ResourcePool whose AssociatedProvider is the player controller, with its Definition path and the
    numeric fields of the pool and of its definition. Current and maximum values are read from these, not assumed.
- hp_quick(label): the weapon's ammo fields and the HUD only, for sampling while moving or firing.
- hp_out(obj): JSON text for the command channel's print.

Property names that do not exist come back as "<Error ...>" strings. Not every name below was verified in this build.
"""
import json
import time

import unrealsdk
from unrealsdk.unreal import UObject, WrappedStruct

HP_NUMBERS = (
    "p1.crosshairs.outer._currentframe", "p1.crosshairs.outer._totalframes",
    "p1.crosshairs.outer.weapon._currentframe", "p1.crosshairs.outer.weapon._totalframes",
    "p1.crosshairs.outer.weapon.up._currentframe",
    "p1.health._currentframe", "p1.shield._currentframe", "p1.bullets._currentframe", "p1.grenades._currentframe",
    "p1.xpbar._currentframe", "p1.bullets.bullet_icon._currentframe", "p1.shield.icon._currentframe",
    "p1.health.health_icon._currentframe",
)
HP_BOOLS = ("p1.crosshairs._visible", "p1.crosshairs.outer._visible", "p1.bullets._visible", "p1.grenades._visible")
HP_STRINGS = ("p1.bullets.bullet_c.text", "p1.grenades.grenadesDisp.text", "p1.health.health_disp.text",
              "p1.shield.shield_disp.text", "p1.levelClassMod.level.text", "p1.levelClassMod.comm.text")
HP_WEAPON_FIELDS = ("AmmoInClip", "CurrentAmmoInClip", "AmmoInClipCount", "CurrentAmmo", "AmmoCount", "ClipSize",
                    "ReloadTime", "FireRate", "Spread", "bIsReloading", "ReplicatedReloadState")
HP_AMMO_FIELDS = ("AmmoInClip", "CurrentAmmoInClip", "AmmoInClipCount", "CurrentAmmo", "AmmoCount")
HP_SIMPLE_TYPES = ("NameProperty", "StrProperty", "ObjectProperty", "EnumProperty", "ByteProperty", "IntProperty",
                   "FloatProperty", "BoolProperty")
HP_NUMBER_TYPES = ("FloatProperty", "IntProperty", "ByteProperty", "BoolProperty")


def _hp_try(fn):
    try:
        return fn()
    except Exception as error:
        return f"<{type(error).__name__}: {str(error)[:80]}>"


def hp_out(obj):
    return json.dumps(obj, default=str, ensure_ascii=False)


def _hp_props(obj):
    """Simple properties of a struct (all of them) or of a UObject (scalar, name and object ones), as plain values."""
    if isinstance(obj, WrappedStruct):
        names = [p.Name for p in obj._type._properties()]
    else:
        names = [f.Name for f in obj.Class._fields() if f.Class.Name in HP_SIMPLE_TYPES]
    return {name: _hp_try(lambda: plain(getattr(obj, name))) for name in names}


def _hp_numbers(obj):
    return {f.Name: _hp_try(lambda: plain(getattr(obj, f.Name))) for f in obj.Class._fields()
            if f.Class.Name in HP_NUMBER_TYPES}


def _hp_movie():
    hud = get_pc().myHUD
    return getattr(hud, "HUDMovie", None) if hud is not None else None


def _hp_weapon_now():
    pawn = get_pc().Pawn
    return getattr(pawn, "Weapon", None) if pawn is not None else None


def hp_weapon(weapon):
    if weapon is None:
        return None
    row = {"class": weapon.Class.Name, "path": weapon._path_name(), "fields": {}}
    for name in HP_WEAPON_FIELDS:
        row["fields"][name] = _hp_try(lambda: plain(getattr(weapon, name)))
    data = _hp_try(lambda: weapon.DefinitionData)
    if isinstance(data, WrappedStruct):
        row["definition"] = _hp_props(data)
        for name in ("WeaponTypeDefinition", "BalanceDefinition"):
            ref = _hp_try(lambda: getattr(data, name))
            if isinstance(ref, UObject):
                row[name] = {"path": ref._path_name(), "props": _hp_props(ref)}
    else:
        row["definition"] = data
    return row


def hp_hud():
    movie = _hp_movie()
    if movie is None:
        return {"error": "no HUD movie"}
    out = {"movie": movie._path_name()}
    for path in HP_NUMBERS:
        out[path] = _hp_try(lambda: movie.GetVariableNumber(path))
    for path in HP_BOOLS:
        out[path] = _hp_try(lambda: movie.GetVariableBool(path))
    for path in HP_STRINGS:
        out[path] = _hp_try(lambda: movie.GetVariableString(path))
    return out


def hp_pools():
    provider = get_pc()._path_name()
    rows = []
    for pool in unrealsdk.find_all("ResourcePool", False):
        if "Default__" in pool._path_name():
            continue
        owner = _hp_try(lambda: pool.AssociatedProvider)
        if not isinstance(owner, UObject) or owner._path_name() != provider:
            continue
        definition = _hp_try(lambda: pool.Definition)
        is_obj = isinstance(definition, UObject)
        rows.append({"pool": pool._path_name(), "definition": plain(definition) if is_obj else definition,
                     "numbers": _hp_numbers(pool),
                     "definition_numbers": _hp_numbers(definition) if is_obj else None})
    return rows


def hp_snapshot(label):
    pc = get_pc()
    return {"label": label, "t": time.perf_counter(),
            "pawn": pc.Pawn._path_name() if pc.Pawn is not None else None,
            "weapon": hp_weapon(_hp_weapon_now()), "hud": hp_hud(), "pools": hp_pools()}


def hp_quick(label):
    weapon = _hp_weapon_now()
    ammo = {}
    if weapon is not None:
        for name in HP_AMMO_FIELDS:
            ammo[name] = _hp_try(lambda: plain(getattr(weapon, name)))
    return {"label": label, "t": time.perf_counter(),
            "weapon": weapon._path_name() if weapon is not None else None, "ammo": ammo, "hud": hp_hud()}
