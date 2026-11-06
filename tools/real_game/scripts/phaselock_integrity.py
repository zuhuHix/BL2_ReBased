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
  * the target's rotation and its mesh's rotation (the open facing question), its mesh scale, actor draw scale and mesh bounds
    radius (its apparent size);
  * the target's collision flags (actor and cylinder) and Physics;
  * every emitter the skill references and every WillowEmitter whose template names a Phaselock bubble: template, draw
    scale, LifeSpan and its component's InstanceParameters (name, scalar);
  * the PhaselockLight's brightness;
  * every 0.25 s, key bones' world locations and scales ("bones"), to compare the live pose with the converted clips;
  * at the start and when the target is first lifted, its live mesh asset, materials, LOD and attached components ("assets").
- pi_result(): after a cast, which pawns the skill lifted and whether it was the target (the capture retries on that).
- Hooks (marked on the same clock): RunCustomEvent along the skill's class chain (the hand-orb event), the skill's
  state functions (BeginLifting, LockTarget, StartOutro, ReleaseTarget, EndSkill, InterruptPhaseLock, FizzleOut,
  TargetBlocked), and after a RunCustomEvent the first-person particle components attached to the player's meshes.
- pi_flags(): for every live AI pawn, what CanPhaseLockTarget answers (read-only script function).
- pi_injure(): put the player into Fight For Your Life during a lock: a large TakeDamage from the target, repeated on the
  tick every 0.5 s of game time until the player is injured (at most 8), because on 2026-11-06 one hit left her at low
  health and not injured. Refuses when the target is not lifted. Only for the end of a session: the player then bleeds out.
"""
import json
import time
from pathlib import Path

import unrealsdk
from unrealsdk.hooks import Type, add_hook, remove_hook

PI_TICK = "WillowGame.WillowGameViewportClient:Tick"
PI_HOOK = "ow_pi_probe"
pi = {"file": None, "pawn": None, "ended_at": None, "scan_at": -1.0, "seqs": [], "arms_seqs": [], "hooks": [],
      "emitter_fields": [], "light_fields": [], "bubble_scan_at": -1.0, "bubbles": [],
      "seen_lifted": set(), "assets_stage": 0, "bones_at": -1.0, "injure": None}

# Key bones of the bullymong skeleton: the torso, the head, the four arms' hands and the feet.
PI_BONES = ("Hips", "Spine2", "Neck", "Head", "lHand1", "lHand2", "rHand1", "rHand2", "LFoot", "RFoot")

SKILL_FUNCS = ("BeginLifting", "LiftTarget", "LockTarget", "StartOutro", "ReleaseTarget", "EndSkill", "InterruptPhaseLock",
               "FizzleOut", "Fizzled", "TargetBlocked", "TransitionToBubbleFXLoop", "PhaseLockTarget", "SelectTarget",
               "RunCustomEvent", "OnActionSkillEnded")
PLAYER_FUNCS = ("TakeDamage", "PlayInjured", "SetInjuredState", "GoToInjuredAndTargeted", "ServerDeactivateSkill")


def pi_target_rank(ai_class, dist):
    """Sort key for the capture's target pick: the Adult first (it is what the host's target stands for), then the plain
    bullymong, then any other large one, each nearest first. A variant has its own archetype, attachments and material."""
    leaf = ai_class.rsplit(".", 1)[-1]
    return (0 if leaf == "CharClass_PrimalBeastAdult" else 1 if leaf == "CharClass_PrimalBeast" else 2, dist)


def _safe(fn, default=None):
    try:
        return fn()
    except Exception as error:
        return default if default is not None else f"<{type(error).__name__}>"


def _null(fn):
    """fn(), or None (JSON null) when it fails. _safe without a default returns an error-marker string instead."""
    try:
        return fn()
    except Exception:
        return None


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


def _bone_rows(mesh, pawn):
    """World location relative to the pawn, and the bone matrix's axis lengths (its scale), for the key bones."""
    rows = {}
    origin = pawn.Location
    for name in PI_BONES:
        loc = _null(lambda: mesh.GetBoneLocation(name))
        if loc is None:
            rows[name] = None
            continue
        row = {"rel": [_r(loc.X - origin.X), _r(loc.Y - origin.Y), _r(loc.Z - origin.Z)]}
        index = _null(lambda: mesh.MatchRefBone(name))
        matrix = _null(lambda: mesh.GetBoneMatrix(index)) if isinstance(index, int) else None
        if matrix is not None:
            row["scale"] = _null(lambda: [_r((p.X**2 + p.Y**2 + p.Z**2)**0.5) for p in (matrix.XPlane, matrix.YPlane, matrix.ZPlane)])
        rows[name] = row
    return rows


def _component_row(comp):
    row = {"class": _null(lambda: comp.Class.Name), "name": _null(lambda: comp.Name)}
    for field in ("SkeletalMesh", "StaticMesh", "Template"):
        asset = _null(lambda: getattr(comp, field))
        if asset is not None:
            row[field] = _null(lambda: asset._path_name())
    row["hidden"] = _null(lambda: bool(comp.HiddenGame))
    return row


def _asset_row(pawn, mesh):
    """What the live pawn draws: its mesh asset, materials, LOD and every attached or owned component that draws something."""
    row = {"skeletal_mesh": _null(lambda: mesh.SkeletalMesh._path_name()),
           "materials": _null(lambda: [m._path_name() if m else None for m in mesh.Materials]),
           "lod": {f: _null(lambda: int(getattr(mesh, f))) for f in ("PredictedLODLevel", "ForcedLodModel", "MinLodModel")},
           "attachments": [], "components": [],
           "attached_actors": _null(lambda: [f"{a.Class.Name}:{a.Name}" for a in pawn.Attached][:32])}
    for a in (_null(lambda: list(mesh.Attachments)) or []):
        comp = _null(lambda: a.Component)
        if comp is not None:
            row["attachments"].append({"socket": _null(lambda: str(a.SocketName)), "bone": _null(lambda: str(a.BoneName)), **_component_row(comp)})
    for comp in (_null(lambda: list(pawn.Components)) or []):
        if comp is not None:
            row["components"].append(_component_row(comp))
    return row


def _hit_player(me, source):
    zero = unrealsdk.make_struct("Vector", X=0.0, Y=0.0, Z=0.0)
    return plain(_safe(lambda: me.TakeDamage(1000000, source.Controller, me.Location, zero, unrealsdk.find_class("DamageType"))))


def _tick_injure(me, now):
    """Repeat the injure's hit until the player is injured; each attempt goes into the row."""
    job = pi["injure"]
    if job is None or me is None or job["done"] or now < job["next"]:
        return None
    if _safe(lambda: bool(me.bIsInjured)):
        job["done"] = True
        return {"injured_after_hits": job["hits"]}
    if job["hits"] >= 8:
        job["done"] = True
        return {"gave_up_after_hits": job["hits"], "player": _player(me)}
    _hit_player(me, job["source"])
    job["hits"] += 1
    job["next"] = now + 0.5
    return {"hit": job["hits"], "player": _player(me)}


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
               "location": plain(pawn.Location), "rotation": plain(_null(lambda: pawn.Rotation)), "mesh_rotation": plain(_null(lambda: mesh.Rotation)),
               "mesh_translation": plain(mesh.Translation),
               # The target's size, for the held target's apparent scale: the mesh and actor scales and the mesh's bounds sphere.
               "mesh_scale": _r(_null(lambda: float(mesh.Scale))), "mesh_scale3d": plain(_null(lambda: mesh.Scale3D)),
               "draw_scale": _r(_null(lambda: float(pawn.DrawScale))),
               "bounds_radius": _r(_null(lambda: float(mesh.Bounds.SphereRadius))),
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
        if row["lifted"]:
            pi["seen_lifted"].add(row["lifted"])
        if now - pi["bones_at"] > 0.25:
            pi["bones_at"] = now
            row["bones"] = _bone_rows(mesh, pawn)
        stage = 2 if row["lifted"] == pawn.Name else 1
        if pi["assets_stage"] < stage:
            pi["assets_stage"] = stage
            row["assets"] = _asset_row(pawn, mesh)
        injure = _tick_injure(me, now)
        if injure:
            row["injure"] = injure
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
    pi["seen_lifted"], pi["assets_stage"], pi["bones_at"], pi["injure"] = set(), 0, -1.0, None
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


def pi_result():
    """After a cast: which pawns the skill lifted, and whether that included the target."""
    pawn = pi["pawn"]
    lifted = sorted(pi["seen_lifted"])
    return {"target": _safe(lambda: pawn.Name), "lifted": lifted, "target_lifted": _safe(lambda: pawn.Name in lifted),
            "target_has_controller": _safe(lambda: pawn.Controller is not None), "injure": _safe(lambda: {
                k: v for k, v in pi["injure"].items() if k != "source"} if pi["injure"] else None)}


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
    """Large TakeDamage on the player from the target pawn while it is lifted, repeated on the tick (_tick_injure) until she is
    injured; the player is not protected. Refuses when the target is not lifted, so a failed cast does not cost the player."""
    me = get_pc().Pawn
    src = ai_pawn(pawn_name)
    if not any(n == src.Name for n in pi["seen_lifted"]):
        return {"skipped": "the target is not lifted", "seen_lifted": sorted(pi["seen_lifted"])}
    before = _player(me)
    result = _hit_player(me, src)
    now = get_pc().WorldInfo.TimeSeconds
    pi["injure"] = {"source": src, "hits": 1, "next": now + 0.5, "done": False}
    return {"before": before, "after": _player(me), "call": result}
