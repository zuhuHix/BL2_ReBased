"""Marcus use-chain recorder for the openwillow_realgame command channel (lane L1, 2026-10-07).

Run once through Invoke-GamePyFile; our own code, no game data. Rows go to RG_OUT.

- mu_start(): hook (PRE and POST) the behavior kernel's sequence check, the script behaviors of the chain, the dialog
  component's TriggerEvent / GetMatchingEvent, WillowAIPawn.PlayOnUseDialog and the class/AI-definition OnUsed; one
  row per call with the arguments, the return value and a short object description. mu_rows() returns them.
- mu_state(): the sequence enabled flags of Marcus's AI provider, his consumer handle and his dialog group list.
- mu_stand_in_front(distance): put the player in front of Marcus, looking at him (memory only).
"""
import json
import math
import time

import unrealsdk
from unrealsdk.hooks import Type, add_hook, remove_hook
from unrealsdk.unreal import UObject, WrappedArray, WrappedStruct

MU = {"on": False, "rows": [], "hooks": [], "t0": None}
MARCUS_PATH = "Sanctuary_Dynamic.TheWorld:PersistentLevel.WillowAIPawn_13"
PROVIDER = "GD_Marcus.Character.AIDef_Marcus.AIBehaviorProviderDefinition_0"

WATCH = [
    "GearboxFramework.BehaviorKernel:IsBehaviorSequenceEnabled",
    "GearboxFramework.Behavior_IsSequenceEnabled:ApplyBehaviorToContext",
    "GearboxFramework.GearboxDialogComponent:TriggerEvent",
    "GearboxFramework.GearboxDialogComponent:GetMatchingEvent",
    "WillowGame.WillowAIPawn:PlayOnUseDialog",
    "WillowGame.AIClassDefinition:OnUsed",
    "WillowGame.Behavior_PlayAIMissionContextDialog:ApplyBehaviorToContext",
    "WillowGame.Behavior_HasMissions:ApplyBehaviorToContext",
    "WillowGame.Behavior_ShowMissionInterface:ApplyBehaviorToContext",
    "GearboxFramework.BehaviorKernel:ActivateBehaviorEventFromScript",
]


def _p(value, depth=0):
    """Short, JSON-safe description of a value."""
    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    if isinstance(value, UObject):
        return value._path_name()
    if depth >= 2:
        return repr(value)[:120]
    if isinstance(value, WrappedStruct):
        out = {}
        for q in value._type._properties():
            try:
                out[q.Name] = _p(getattr(value, q.Name), depth + 1)
            except Exception:
                pass
        return out
    if isinstance(value, WrappedArray):
        return [_p(v, depth + 1) for v in list(value)[:16]]
    return repr(value)[:120]


def _args(args, func):
    out = {}
    try:
        for q in func.func._properties():
            if hasattr(args, q.Name):
                out[q.Name] = _p(getattr(args, q.Name))
    except Exception as e:
        out["args_error"] = repr(e)[:100]
    return out


def _mk(path, tag):
    short = path.split(":")[-1]

    def hook(obj, args, ret, func):
        if not MU["on"]:
            return
        row = {"t": round(time.perf_counter() - MU["t0"], 4), "tag": tag, "func": path, "obj": _p(obj)}
        row["args"] = _args(args, func)
        if short == "ApplyBehaviorToContext" and "IsSequenceEnabled" in path:
            try:
                row["SequenceName"] = str(obj.SequenceName)
            except Exception:
                pass
        if tag == "post":
            try:
                row["ret"] = _p(ret)
            except Exception:
                row["ret"] = "?"
        MU["rows"].append(row)

    return hook


def mu_start():
    mu_stop()
    MU["rows"] = []
    MU["t0"] = time.perf_counter()
    MU["on"] = True
    for path in WATCH:
        for typ, tag in ((Type.PRE, "pre"), (Type.POST, "post")):
            ident = f"ow_mu_{tag}_{path}"
            add_hook(path, typ, ident, _mk(path, tag))
            MU["hooks"].append((path, typ, ident))


def mu_stop():
    MU["on"] = False
    for path, typ, ident in MU["hooks"]:
        try:
            remove_hook(path, typ, ident)
        except Exception:
            pass
    MU["hooks"] = []


def mu_rows():
    return MU["rows"]


def mu_marcus():
    return unrealsdk.find_object("WillowAIPawn", MARCUS_PATH)


def mu_state():
    m = mu_marcus()
    prov = unrealsdk.find_object("BehaviorProviderDefinition", PROVIDER)
    kernel = unrealsdk.find_class("BehaviorKernel").ClassDefaultObject
    handle = m.ConsumerHandle
    seqs = [str(s.BehaviorSequenceName) for s in prov.BehaviorSequences]
    enabled = {n: bool(kernel.IsBehaviorSequenceEnabled(handle, prov, n)[0]) for n in seqs}
    out = {"consumer_handle": _p(handle), "sequences_enabled": enabled}
    try:
        res = m.GetDialogGroups([])  # out array: the SDK returns (..., array)
        groups = res[-1] if isinstance(res, tuple) else res
        out["dialog_groups"] = [_p(g) if g is not None else None for g in list(groups)]
        out["dialog_group_count"] = len(list(groups))
    except Exception as e:
        out["dialog_groups_error"] = repr(e)[:200]
    try:
        out["dialog_name_tag"] = _p(m.GetDialogNameTag())
    except Exception as e:
        out["dialog_name_tag_error"] = repr(e)[:200]
    return out


def mu_stand_in_front(distance=180.0):
    pc = unrealsdk.find_object("WillowAIPawn", MARCUS_PATH)
    me = get_pc()
    yaw = pc.Rotation.Yaw * 360.0 / 65536.0
    x = pc.Location.X + math.cos(math.radians(yaw)) * distance
    y = pc.Location.Y + math.sin(math.radians(yaw)) * distance
    z = pc.Location.Z + 40.0
    me.Pawn.Location = unrealsdk.make_struct("Vector", X=x, Y=y, Z=z)
    me.Pawn.SetPhysics(2)
    dx, dy = pc.Location.X - x, pc.Location.Y - y
    me.SetRotation(unrealsdk.make_struct("Rotator", Pitch=0, Yaw=int(math.degrees(math.atan2(dy, dx)) * 65536 / 360), Roll=0))
    return [round(x, 1), round(y, 1), round(z, 1)]
