"""Phaselock integrity probe (2026-11-02): per-frame reads for the open statements of
docs/verification/NATIVE_PHASELOCK_PRESENTATION.md section 5 and the lock-vs-down rule.

Run through Invoke-GamePyFile AFTER scripts/phaselock.py and scripts/phaselock_matched.py (it uses lift_skill(),
ai_pawn(), plain() and mark() from their namespace). Our own code; everything it writes is game data and goes under
RG_OUT (normally the ignored local/realgame/phaselock_integrity).

- pi_discover(): the lift skill's object, light and emitter fields and the player pawn's skeletal mesh components (names
  only), so the per-frame reads below do not depend on remembered field names.
- pi_start(label, pawn_name) / pi_stop(): one JSON line per rendered frame while the skill runs (and 3 s after) to
  `<label>.jsonl`:
  * the skill state and times, the cooldown pool;
  * every AnimNodeSequence that drives the target's mesh with weight above 0.001 (found by walking the AnimTree from
    Mesh.Animations and, every 0.25 s, by a scan of all AnimNodeSequence objects whose SkelComponent is that mesh, since
    the special-move node's players were not in AnimTickArray on 2026-10-21): sequence name, rate, time, length,
    playing / looping, weight; the same for the first-person arms;
  * the target's collision flags (actor and cylinder) and Physics;
  * every emitter the skill references and every WillowEmitter whose template names a Phaselock bubble: template, draw
    scale, LifeSpan and its component's InstanceParameters (name, scalar);
  * the PhaselockLight's brightness.
- Hooks (marked on the same clock): RunCustomEvent along the skill's class chain (the hand-orb event), the skill's
  state functions (BeginLifting, LockTarget, StartOutro, ReleaseTarget, EndSkill, InterruptPhaseLock, FizzleOut,
  TargetBlocked), and after a RunCustomEvent the first-person particle components attached to the player's meshes.
- pi_flags(): for every live AI pawn, what CanPhaseLockTarget answers (read-only script function).
- pi_injure(): put the player into Fight For Your Life during a lock (one large TakeDamage from the target), returning
  what the pawn's injured fields read after it. Only for the end of a session: the player then bleeds out.
"""
import json
import time
from pathlib import Path

import unrealsdk
from unrealsdk.hooks import Type, add_hook, remove_hook

PI_TICK = "WillowGame.WillowGameViewportClient:Tick"
PI_HOOK = "ow_pi_probe"
pi = {"file": None, "pawn": None, "ended_at": None, "scan_at": -1.0, "seqs": [], "arms_seqs": [], "hooks": [],
      "emitter_fields": [], "light_fields": [], "bubble_scan_at": -1.0, "bubbles": []}

SKILL_FUNCS = ("BeginLifting", "LiftTarget", "LockTarget", "StartOutro", "ReleaseTarget", "EndSkill", "InterruptPhaseLock",
               "FizzleOut", "Fizzled", "TargetBlocked", "TransitionToBubbleFXLoop", "PhaseLockTarget", "SelectTarget",
               "RunCustomEvent", "OnActionSkillEnded")
PLAYER_FUNCS = ("TakeDamage", "PlayInjured", "SetInjuredState", "GoToInjuredAndTargeted", "ServerDeactivateSkill")


def _safe(fn, default=None):
    try:
        return fn()
    except Exception as error:
        return default if default is not None else f"<{type(error).__name__}>"


def _r(x, n=4):
    return round(x, n) if isinstance(x, float) else x


def _chain(cls):
    while cls:
        yield cls
        cls = cls.SuperField


def _object_fields(obj, wanted):
    """Names of object properties along obj's class chain whose name contains one of `wanted`."""
    out = []
    for cls in _chain(obj.Class):
        for prop in cls._properties():
            if prop.Outer != cls or prop.Class.Name not in ("ObjectProperty", "ComponentProperty"):
                continue
            if any(w.lower() in prop.Name.lower() for w in wanted):
                out.append(prop.Name)
    return out


def _declared(cls, names):
    paths = []
    for c in _chain(cls):
        for f in c._fields():
            if f.Class.Name == "Function" and f.Outer == c and f.Name in names:
                paths.append(f._path_name())
    return paths


def _seq_row(node):
    seq = _safe(lambda: node.AnimSeq, None)
    return {"node": node.Name, "class": node.Class.Name, "seq": _safe(lambda: str(node.AnimSeqName)),
            "weight": _r(_safe(lambda: float(node.NodeTotalWeight), 0.0)), "rate": _r(_safe(lambda: float(node.Rate))),
            "time": _r(_safe(lambda: float(node.CurrentTime))), "playing": _safe(lambda: bool(node.bPlaying)),
            "looping": _safe(lambda: bool(node.bLooping)),
            "length": _r(_safe(lambda: float(seq.SequenceLength))) if seq else None,
            "rate_scale": _r(_safe(lambda: float(seq.RateScale))) if seq else None,
            "owner": _safe(lambda: node.ParentNodes[0].Name if node.ParentNodes else None)}


def _walk(node, out, depth=0):
    if node is None or depth > 40:
        return
    if hasattr(node, "AnimSeqName"):
        out.append(node)
    children = _safe(lambda: list(node.Children), [])
    if isinstance(children, list):
        for child in children:
            _walk(_safe(lambda: child.Anim, None), out, depth + 1)


def _blend_rows(mesh):
    """Special-move and slot nodes in the tree: class, name, weight, active child."""
    rows, stack = [], [_safe(lambda: mesh.Animations, None)]
    while stack:
        node = stack.pop()
        if node is None or not hasattr(node, "Children"):
            continue
        cname = node.Class.Name
        if "Special" in cname or "Slot" in cname or "PhaseLock" in cname or "Phaselock" in cname:
            rows.append({"node": node.Name, "class": cname, "weight": _r(_safe(lambda: float(node.NodeTotalWeight), 0.0)),
                         "active": _safe(lambda: int(node.ActiveChildIndex)) if hasattr(node, "ActiveChildIndex") else None,
                         "children": [_r(_safe(lambda: float(c.Weight), 0.0)) for c in _safe(lambda: list(node.Children), [])]})
        for c in _safe(lambda: list(node.Children), []):
            stack.append(_safe(lambda: c.Anim, None))
    return rows


def _scan_sequences(mesh):
    """Every AnimNodeSequence (and subclass) bound to this mesh, tree or not."""
    found = []
    tree = []
    _walk(_safe(lambda: mesh.Animations, None), tree)
    seen = {n.Name for n in tree}
    found += tree
    for node in unrealsdk.find_all("AnimNodeSequence", exact=False):
        try:
            if node.SkelComponent == mesh and node.Name not in seen:
                found.append(node)
                seen.add(node.Name)
        except Exception:
            pass
    return found


def _arms(pawn):
    for name in ("Arms", "Mesh1P", "FirstPersonMesh", "ArmsMesh"):
        mesh = _safe(lambda: getattr(pawn, name), None)
        if mesh is not None and not isinstance(mesh, str):
            return mesh
    return None


def _collision(pawn):
    out = {"physics": _safe(lambda: str(pawn.Physics))}
    for key in ("bCollideActors", "bBlockActors", "bCollideWorld", "bProjTarget", "bCanBeDamaged"):
        out[key] = _safe(lambda: bool(getattr(pawn, key)))
    cyl = _safe(lambda: pawn.CylinderComponent, None)
    if cyl is not None and not isinstance(cyl, str):
        out["cyl"] = {k: _safe(lambda: bool(getattr(cyl, k))) for k in
                      ("CollideActors", "BlockActors", "BlockZeroExtent", "BlockNonZeroExtent", "BlockRigidBody")}
    return out


def _emitter_row(em):
    psc = _safe(lambda: em.ParticleSystemComponent, None)
    row = {"name": _safe(lambda: em.Name), "class": _safe(lambda: em.Class.Name),
           "draw_scale": _r(_safe(lambda: float(em.DrawScale))), "life_span": _r(_safe(lambda: float(em.LifeSpan))),
           "deleted": _safe(lambda: bool(em.bDeleteMe))}
    if psc is not None and not isinstance(psc, str):
        row["template"] = _safe(lambda: psc.Template._path_name() if psc.Template else None)
        row["active"] = _safe(lambda: bool(psc.bIsActive))
        row["params"] = [{"name": _safe(lambda: str(p.Name)), "type": _safe(lambda: str(p.ParamType)),
                          "scalar": _r(_safe(lambda: float(p.Scalar)))} for p in _safe(lambda: list(psc.InstanceParameters), [])]
    return row


def _pool_state(pc):
    pool = _safe(lambda: pc.SkillCooldownPool.Data, None)
    if pool is None or isinstance(pool, str):
        return None
    return {k: _r(_safe(lambda: float(getattr(pool, k)))) for k in ("CurrentValue", "MaxValue", "ConsumptionRate")}


def _player(pawn):
    out = {"hp": _r(_safe(lambda: float(pawn.GetHealth())))}
    for key in ("bIsInjured", "InjuredState", "InjuredDeadState"):
        out[key] = _safe(lambda: plain(getattr(pawn, key)))
    return out


def _on_tick(*_args):
    try:
        if pi["file"] is None:
            return
        skill = lift_skill()
        pawn = pi["pawn"]
        pc = get_pc()
        if skill is None or pawn is None:
            return
        world = pc.WorldInfo
        now = world.TimeSeconds
        state = str(skill.CurrentState)
        if state == "0" and skill.SkillStartTime > 0:
            pi["ended_at"] = pi["ended_at"] or now
            if now - pi["ended_at"] > 3.0:
                return
        elif state != "0":
            pi["ended_at"] = None
        if skill.SkillStartTime <= 0:
            return
        mesh = pawn.Mesh
        me = pc.Pawn
        arms = _arms(me) if me else None
        if now - pi["scan_at"] > 0.25:
            pi["scan_at"] = now
            pi["seqs"] = _scan_sequences(mesh)
            pi["arms_seqs"] = _scan_sequences(arms) if arms is not None else []
        if now - pi["bubble_scan_at"] > 0.1:
            pi["bubble_scan_at"] = now
            pi["bubbles"] = [e for e in unrealsdk.find_all("WillowEmitter", exact=False)
                             if not _safe(lambda: e.bDeleteMe, True) and "Default__" not in e._path_name()
                             and any(k in str(_safe(lambda: e.ParticleSystemComponent.Template._path_name(), ""))
                                     for k in ("Bubble", "PhaseLock", "Phaselock", "Siren"))]
        row = {"t": time.perf_counter(), "game_t": now, "since_cast": round(now - skill.SkillStartTime, 4), "state": state,
               "skill_duration": _r(float(skill.SkillDuration)), "state_start": _r(float(skill.StateStartTime)),
               "state_duration": _r(float(skill.StateDuration)), "collapse_start": _r(float(skill.CollapseStartTime)),
               "lifted": _safe(lambda: skill.LiftedPawn.Name if skill.LiftedPawn else None),
               "location": plain(pawn.Location), "rotation": plain(pawn.Rotation), "mesh_rotation": plain(_safe(lambda: mesh.Rotation)),
               "mesh_translation": plain(mesh.Translation),
               "collision": _collision(pawn), "pool": _pool_state(pc), "player": _player(me) if me else None,
               "seqs": [r for r in (_seq_row(n) for n in pi["seqs"]) if isinstance(r["weight"], float) and r["weight"] > 0.001],
               "arms": [r for r in (_seq_row(n) for n in pi["arms_seqs"]) if isinstance(r["weight"], float) and r["weight"] > 0.001],
               "blends": _blend_rows(mesh)}
        emitters = {}
        for name in pi["emitter_fields"]:
            em = _safe(lambda: getattr(skill, name), None)
            if em is not None and not isinstance(em, str) and hasattr(em, "ParticleSystemComponent"):
                emitters[name] = _emitter_row(em)
        row["skill_emitters"] = emitters
        row["bubbles"] = [_emitter_row(e) for e in pi["bubbles"]]
        lights = {}
        for name in pi["light_fields"]:
            light = _safe(lambda: getattr(skill, name), None)
            if light is not None and not isinstance(light, str):
                lights[name] = {"brightness": _r(_safe(lambda: float(light.Brightness))),
                                "radius": _r(_safe(lambda: float(light.Radius))), "enabled": _safe(lambda: bool(light.bEnabled))}
        row["lights"] = lights
        pi["file"].write(json.dumps(row) + "\n")
    except Exception as error:  # never break the game
        try:
            pi["file"].write(json.dumps({"event": "error", "error": repr(error)[:300]}) + "\n")
        except Exception:
            pass


def _attachments(pawn):
    """First-person particle components attached to the player's skeletal meshes (the hand orb)."""
    out = []
    for name in _object_fields(pawn, ("Mesh", "Arms")):
        mesh = _safe(lambda: getattr(pawn, name), None)
        if mesh is None or isinstance(mesh, str) or not hasattr(mesh, "Attachments"):
            continue
        for a in _safe(lambda: list(mesh.Attachments), []):
            comp = _safe(lambda: a.Component, None)
            if comp is None or isinstance(comp, str) or not hasattr(comp, "Template"):
                continue
            out.append({"mesh": name, "socket": _safe(lambda: str(a.SocketName)), "bone": _safe(lambda: str(a.BoneName)),
                        "template": _safe(lambda: comp.Template._path_name() if comp.Template else None),
                        "active": _safe(lambda: bool(comp.bIsActive)), "rel_loc": plain(_safe(lambda: a.RelativeLocation)),
                        "rel_scale": plain(_safe(lambda: a.RelativeScale)), "comp_translation": plain(_safe(lambda: comp.Translation)),
                        "comp_scale": _r(_safe(lambda: float(comp.Scale)))})
    return out


def _mk(tag):
    def hook(obj, args, _ret, func):
        try:
            skill = lift_skill()
            now = get_pc().WorldInfo.TimeSeconds
            data = {"func": func.func.Name, "tag": tag, "game_t": now,
                    "since_cast": round(now - skill.SkillStartTime, 4) if skill else None,
                    "state": str(skill.CurrentState) if skill else None}
            if func.func.Name == "RunCustomEvent":
                data["event"] = _safe(lambda: str(args.EventName))
                if tag == "post":
                    data["attachments"] = _attachments(get_pc().Pawn)
            if func.func.Name == "TakeDamage":
                data["damage"] = _safe(lambda: int(args.Damage))
            if pi["file"] is not None:
                pi["file"].write(json.dumps({"event": "call", **data}) + "\n")
            mark("pi_call", **data)
        except Exception as error:
            mark("pi_hook_error", error=repr(error)[:200])
    return hook


def pi_discover():
    skill = lift_skill()
    me = get_pc().Pawn
    pi["emitter_fields"] = _object_fields(skill, ("Emitter", "FX", "Bubble", "Particle"))
    pi["light_fields"] = _object_fields(skill, ("Light",))
    return {"skill": skill._path_name(), "skill_class": skill.Class.Name, "emitter_fields": pi["emitter_fields"],
            "light_fields": pi["light_fields"], "player_meshes": _object_fields(me, ("Mesh", "Arms")),
            "arms": _safe(lambda: _arms(me)._path_name()),
            "skill_hooks": _declared(skill.Class, SKILL_FUNCS), "player_hooks": _declared(me.Class, PLAYER_FUNCS)}


def pi_start(label, pawn_name):
    pi_stop()
    info = pi_discover()
    folder = Path(RG_OUT)
    folder.mkdir(parents=True, exist_ok=True)
    pi["file"] = (folder / f"{label}.jsonl").open("w", encoding="utf-8")
    pi["pawn"], pi["ended_at"], pi["scan_at"], pi["bubble_scan_at"] = ai_pawn(pawn_name), None, -1.0, -1.0
    add_hook(PI_TICK, Type.POST_UNCONDITIONAL, PI_HOOK, _on_tick)
    for path in info["skill_hooks"] + info["player_hooks"]:
        add_hook(path, Type.PRE, PI_HOOK, _mk("pre"))
        add_hook(path, Type.POST, PI_HOOK + "_post", _mk("post"))
        pi["hooks"].append(path)
    return info


def pi_stop():
    remove_hook(PI_TICK, Type.POST_UNCONDITIONAL, PI_HOOK)
    for path in pi["hooks"]:
        remove_hook(path, Type.PRE, PI_HOOK)
        remove_hook(path, Type.POST, PI_HOOK + "_post")
    pi["hooks"] = []
    if pi["file"] is not None:
        pi["file"].close()
        pi["file"] = None


def pi_flags():
    """Do not use in a session: on 2026-11-02 this call got no reply and the session was lost.
    CanPhaseLockTarget for every live AI pawn (a read-only script function), with its class and allegiance."""
    skill = lift_skill()
    rows = []
    for pawn in unrealsdk.find_all("WillowAIPawn"):
        if not pawn.Name.startswith("WillowAIPawn_") or pawn.bDeleteMe:
            continue
        rows.append({"name": pawn.Name, "ai_class": _safe(lambda: pawn.AIClass._path_name().split(".")[-1]),
                     "can_lock": _safe(lambda: bool(skill.CanPhaseLockTarget(pawn))),
                     "alive_and_well": _safe(lambda: bool(pawn.IsAliveAndWell()))})
    return rows


def pi_injure(pawn_name):
    """One large TakeDamage on the player from the target pawn (it must be locked); the player is not protected."""
    me = get_pc().Pawn
    src = ai_pawn(pawn_name)
    before = _player(me)
    zero = unrealsdk.make_struct("Vector", X=0.0, Y=0.0, Z=0.0)
    result = _safe(lambda: me.TakeDamage(1000000, src.Controller, me.Location, zero, unrealsdk.find_class("DamageType")))
    return {"before": before, "after": _player(me), "call": plain(result)}
