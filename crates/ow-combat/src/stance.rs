//! Owner: lane IMPL-MAYA9-NATIVES (decision 2026-11-26-NNN). `WillowPawn.SetDefaultStance()` and
//! `WillowPlayerPawn.SetDefaultStance()` in `ParityMode::Off`. AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from docs/verification/NATIVE_MAYA9_AND_AFTER.md section 3.1 alone (lane
//! ANALYST-MAYA9-AND-AFTER, #776) and the hub's rulings; this lane did not read the executable. **Every rule is
//! UNVERIFIED in the running game.** Reference binds nothing (the stub policy answers). `WillowAIPawn`'s own body was
//! not read and keeps go_down's `NotRead`.
//!
//! **`WillowPawn`'s body:** nothing. **`WillowPlayerPawn`'s** (the note's section 3.1):
//! 1. the stance source: `BodyClass`, then `AnimationParentBody` while the source has `bUseParentStances` and a parent.
//!    No `BodyClass`: nothing;
//! 2. the pick: `bIsInjured` the `InjuredStance`; else `bIsCrouched` the `CrouchStance`; else `bIsSprinting` not 0 the
//!    `SprintStance`; else the `RunStance`;
//! 3. the set-stance: nothing when `Mesh` or the stance is None, `Role` is not `ROLE_Authority` or the stance is the
//!    `CurrentStance` already. One of the source's five stances (by identity) is not an override; any other is. With
//!    an override held (`OverrideStance` set), one of the five does nothing. The aim test before the change:
//!    `Physics` not `PHYS_Ladder`, `AimOffset` set, and `CurrentStance` None or with `bCanUseAimOffsets`. Then: an
//!    override is held (`OverrideStance` := it); with a current stance, its provider's `OnExitStance`; `bNetDirty`
//!    set, `CurrentStance` := the stance, its provider's `OnEnterStance`; `bCanCrouch` := the pawn archetype's (the
//!    heap's archetype, else the class default) for one of the five, false for an override; the rotation branch (a
//!    `Controller`, the aim test passing before and failing now) is [`ROTATION`] (`NotRead`); the aim-offset refresh
//!    (the stance's `AimOffsetName`, else the source's `AimOffsetNodeName`; when `AimOffset` is None or its `NodeName`
//!    differs, `Mesh`'s node of that name, kept as a `WillowGame.WillowAnimNodeAimOffset`, else None); the left-hand
//!    IK step (`LeftHandIK` set: wanted is the current stance None or with `bAllowLeftHandIK`, and `LeftHandIKHolds`
//!    empty; active is `StrengthTarget` above 0.5; when they differ, `SetSkelControlActive(wanted)`).
//!
//! **The events** go through the kernel's event rule on the pawn's consumer handle: a kernel-owned handle fires the
//! event on the stance's provider when the consumer registered it, and **nothing when it did not** (the hub's ruling,
//! UNVERIFIED by fit: no consumer to dispatch to); a handle no one owns, nothing; a crate-owned one is [`CRATE_OWNED`]
//! (`NotRead`).
//!
//! **`SetSkelControlActive(bool)`** (stock engine): `StrengthTarget` := 1 or 0; `BlendTimeToGo` := |`StrengthTarget`
//! − `ControlStrength`| × `BlendInTime` (activating) or `BlendOutTime`; not above 0: `BlendTimeToGo` := 0 and
//! `ControlStrength` := `StrengthTarget`; then the same on `NextControl` while the control just set has
//! `bPropagateSetActive`.
//!
//! **The preflight** reads only: the pawn's, the sources', the stances' and the controls' declarations (else
//! `VmError::Script`), every object read resident (else [`NOT_LOADED`]), the rotation branch, the node lookup and the
//! controls' plan, all before the first write. A refusal from a behavior an event runs comes after writes and carries
//! the "stopped after partial writes" prefix. **Fitted, UNVERIFIED:** the two stance fields are named by their use.

use ow_core::{DetMap, NotYet, ParityMode};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

/// The record of the rule.
pub const RECORD: &str = "docs/verification/NATIVE_MAYA9_AND_AFTER.md (section 3.1)";
/// The natives bound here.
pub const PAWN_SET_DEFAULT_STANCE: &str = "WillowGame.WillowPawn.SetDefaultStance";
pub const PLAYER_SET_DEFAULT_STANCE: &str = "WillowGame.WillowPlayerPawn.SetDefaultStance";
pub const ROTATION: &str = "WillowPlayerPawn.SetDefaultStance: the controller's rotation set";
pub const NOT_LOADED: &str = "WillowPlayerPawn.SetDefaultStance: an object the store has not loaded (a load is a write)";
pub const CRATE_OWNED: &str = "WillowPlayerPawn.SetDefaultStance: a stance event on a consumer another crate owns";
/// The stance events.
pub const ENTER: &str = "OnEnterStance";
pub const EXIT: &str = "OnExitStance";
const WHAT: &str = "WillowPlayerPawn.SetDefaultStance";
const FIVE: [&str; 5] = ["RunStance", "PatrolStance", "SprintStance", "InjuredStance", "CrouchStance"];

fn bad(message: impl Into<String>) -> VmError {
    VmError::Script { function: WHAT.to_string(), message: message.into() }
}

fn not_loaded() -> VmError {
    NotYet::not_read(NOT_LOADED, RECORD).into()
}

/// `o`'s `name` with its declared kind, else a script error naming it.
fn read(vm: &Vm, o: ObjRef, name: &str) -> Result<(Value, PropKind), VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| bad(format!("{} declares no {name}", class_path(vm, o))))?;
    let kind = vm.space.classes.decl(p).map(|d| d.kind.clone()).ok_or_else(|| bad(format!("{name} has no declaration")))?;
    Ok((vm.space.read(o, PropAddr::scalar(p))?.clone(), kind))
}

fn class_path(vm: &Vm, o: ObjRef) -> String {
    vm.space.heap.get(o).ok().and_then(|x| vm.space.classes.get_layout(x.class)).map_or_else(String::new, |l| l.path().to_string())
}

fn object(vm: &Vm, o: ObjRef, name: &str) -> Result<Option<ObjRef>, VmError> {
    match read(vm, o, name)? {
        (Value::Object(x), PropKind::Object { .. }) => Ok(x),
        _ => Err(bad(format!("{name} is not declared as an object"))),
    }
}

fn boolean(vm: &Vm, o: ObjRef, name: &str) -> Result<bool, VmError> {
    match read(vm, o, name)? {
        (Value::Bool(b), PropKind::Bool) => Ok(b),
        _ => Err(bad(format!("{name} is not declared as a bool"))),
    }
}

fn float(vm: &Vm, o: ObjRef, name: &str) -> Result<f32, VmError> {
    match read(vm, o, name)? {
        (Value::Float(f), PropKind::Float) => Ok(f),
        _ => Err(bad(format!("{name} is not declared as a float"))),
    }
}

fn name(vm: &Vm, o: ObjRef, field: &str) -> Result<ow_object::ids::NameId, VmError> {
    match read(vm, o, field)? {
        (Value::Name(n), PropKind::Name) => Ok(n),
        _ => Err(bad(format!("{field} is not declared as a name"))),
    }
}

/// Whether the byte `field` of `o` holds its enum's value `wanted` (by its place in the declared enum).
fn is_enum_value(vm: &Vm, o: ObjRef, field: &str, wanted: &str) -> Result<bool, VmError> {
    let (v, kind) = read(vm, o, field)?;
    let PropKind::Byte { enum_: Some(e) } = kind else {
        return Err(bad(format!("{field} is not declared as a byte of an enum")));
    };
    let at = vm
        .space
        .classes
        .enum_layout(e)
        .and_then(|l| l.values().iter().position(|n| vm.space.names.text(*n).eq_ignore_ascii_case(wanted)));
    match (v, at) {
        (Value::Byte(b), Some(at)) => Ok(usize::from(b) == at),
        (Value::Byte(_), None) => Err(bad(format!("{field}'s enum has no {wanted}"))),
        _ => Err(bad(format!("{field} does not hold a byte"))),
    }
}

fn resident(vm: &Vm, o: ObjRef) -> Result<(), VmError> {
    let x = vm.space.heap.get(o)?;
    let short = vm.space.classes.get_layout(x.class).is_some_and(|l| x.slots.len() < l.slot_count() as usize);
    if vm.defs.is_unloaded(&vm.space, o) || vm.defs.is_failed(&vm.space, o) || short {
        return Err(not_loaded());
    }
    Ok(())
}

fn is_a(vm: &Vm, o: ObjRef, path: &str) -> Result<bool, VmError> {
    let mut cursor = Some(vm.space.heap.get(o)?.class);
    let mut steps = 0;
    while let Some(c) = cursor.filter(|_| steps < 256) {
        steps += 1;
        let Some(l) = vm.space.classes.get_layout(c) else { return Ok(false) };
        if l.path().as_str().eq_ignore_ascii_case(path) {
            return Ok(true);
        }
        cursor = l.super_class();
    }
    Ok(false)
}

/// One control write of `SetSkelControlActive`'s plan.
struct ControlWrite {
    control: ObjRef,
    strength_target: f32,
    blend_time: f32,
    strength: Option<f32>,
}

/// `SetSkelControlActive(active)` on `control` and its propagation, planned (reads only).
fn plan_skel_control_active(vm: &Vm, control: ObjRef, active: bool) -> Result<Vec<ControlWrite>, VmError> {
    let mut out = Vec::new();
    let mut cursor = Some(control);
    let mut seen = Vec::new();
    while let Some(c) = cursor {
        if seen.contains(&c) || seen.len() > 256 {
            break;
        }
        seen.push(c);
        resident(vm, c)?;
        let target = if active { 1.0f32 } else { 0.0 };
        let strength = float(vm, c, "ControlStrength")?;
        let blend = if active { float(vm, c, "BlendInTime")? } else { float(vm, c, "BlendOutTime")? };
        let time = (target - strength).abs() * blend;
        let settled = !(time > 0.0);
        out.push(ControlWrite {
            control: c,
            strength_target: target,
            blend_time: if settled { 0.0 } else { time },
            strength: settled.then_some(target),
        });
        let propagate = boolean(vm, c, "bPropagateSetActive")?;
        let next = object(vm, c, "NextControl")?;
        cursor = if propagate { next } else { None };
    }
    Ok(out)
}

/// The aim test (module documentation) with `current` as the current stance.
fn aim_test(vm: &Vm, pawn: ObjRef, current: Option<ObjRef>, aim_offset: Option<ObjRef>) -> Result<bool, VmError> {
    if is_enum_value(vm, pawn, "Physics", "PHYS_Ladder")? || aim_offset.is_none() {
        return Ok(false);
    }
    match current {
        None => Ok(true),
        Some(s) => boolean(vm, s, "bCanUseAimOffsets"),
    }
}

/// The plan of a set-stance that changes something.
struct Plan {
    stance: ObjRef,
    current: Option<ObjRef>,
    is_override: bool,
    can_crouch: bool,
    aim_offset: Option<Option<ObjRef>>,
    controls: Vec<ControlWrite>,
}

/// The preflight (module documentation): `None` for nothing to do. Reads only.
fn plan(vm: &Vm, pawn: ObjRef) -> Result<Option<Plan>, VmError> {
    // The pawn's declarations.
    for f in ["bIsInjured", "bIsCrouched", "bNetDirty", "bCanCrouch"] {
        boolean(vm, pawn, f)?;
    }
    for f in ["BodyClass", "Mesh", "AimOffset", "LeftHandIK", "Controller", "CurrentStance", "OverrideStance"] {
        object(vm, pawn, f)?;
    }
    let sprinting = match read(vm, pawn, "bIsSprinting")? {
        (Value::Int(i), _) => i,
        (other, _) => return Err(bad(format!("bIsSprinting holds {}, not an int", other.kind_name()))),
    };
    if !matches!(read(vm, pawn, "LeftHandIKHolds")?, (_, PropKind::Array(_))) {
        return Err(bad("LeftHandIKHolds is not declared as an array"));
    }
    // 1. The source.
    let Some(body) = object(vm, pawn, "BodyClass")? else {
        return Ok(None);
    };
    resident(vm, body)?;
    let mut source = body;
    for _ in 0..64 {
        for f in FIVE.iter().chain(["AnimationParentBody"].iter()) {
            object(vm, source, f)?;
        }
        name(vm, source, "AimOffsetNodeName")?;
        if !boolean(vm, source, "bUseParentStances")? {
            break;
        }
        let Some(parent) = object(vm, source, "AnimationParentBody")? else { break };
        resident(vm, parent)?;
        source = parent;
    }
    // 2. The pick.
    let field = if boolean(vm, pawn, "bIsInjured")? {
        "InjuredStance"
    } else if boolean(vm, pawn, "bIsCrouched")? {
        "CrouchStance"
    } else if sprinting != 0 {
        "SprintStance"
    } else {
        "RunStance"
    };
    let stance = object(vm, source, field)?;
    plan_set(vm, pawn, source, stance)
}

/// The set-stance's plan (the pawn's virtual, module documentation step 3) for `stance` from `source`. Reads only.
fn plan_set(vm: &Vm, pawn: ObjRef, source: ObjRef, stance: Option<ObjRef>) -> Result<Option<Plan>, VmError> {
    let holds_empty = match read(vm, pawn, "LeftHandIKHolds")? {
        (v, PropKind::Array(_)) => v.elements().is_none_or(|e| e.is_empty()),
        _ => return Err(bad("LeftHandIKHolds is not declared as an array")),
    };
    // 3. The set-stance's first test.
    let mesh = object(vm, pawn, "Mesh")?;
    let current = object(vm, pawn, "CurrentStance")?;
    let (Some(mesh), Some(stance)) = (mesh, stance) else {
        return Ok(None);
    };
    if !is_enum_value(vm, pawn, "Role", "ROLE_Authority")? || Some(stance) == current {
        return Ok(None);
    }
    resident(vm, stance)?;
    if let Some(c) = current {
        resident(vm, c)?;
        for f in ["bCanUseAimOffsets", "bAllowLeftHandIK"] {
            boolean(vm, c, f)?;
        }
        object(vm, c, "BehaviorProviderDefinition")?;
    }
    for f in ["bCanUseAimOffsets", "bAllowLeftHandIK"] {
        boolean(vm, stance, f)?;
    }
    name(vm, stance, "AimOffsetName")?;
    object(vm, stance, "BehaviorProviderDefinition")?;
    let mut five = Vec::new();
    for f in FIVE {
        if let Some(s) = object(vm, source, f)? {
            five.push(s);
        }
    }
    let is_override = !five.contains(&stance);
    if !is_override && object(vm, pawn, "OverrideStance")?.is_some() {
        return Ok(None);
    }
    let aim_offset = object(vm, pawn, "AimOffset")?;
    let before = aim_test(vm, pawn, current, aim_offset)?;
    // bCanCrouch: the archetype's for one of the five.
    let can_crouch = if is_override {
        false
    } else {
        let archetype = match vm.space.heap.get(pawn)?.archetype {
            Some(a) => a,
            None => {
                let class = vm.space.heap.get(pawn)?.class;
                vm.space
                    .heap
                    .handles()
                    .into_iter()
                    .find(|&o| {
                        vm.space.heap.get(o).is_ok_and(|x| {
                            x.class == class && matches!(x.origin, ow_object::heap::Origin::Default { .. })
                        })
                    })
                    .ok_or_else(not_loaded)?
            }
        };
        resident(vm, archetype)?;
        boolean(vm, archetype, "bCanCrouch")?
    };
    // 9. The rotation branch, decided here.
    let after = aim_test(vm, pawn, Some(stance), aim_offset)?;
    if object(vm, pawn, "Controller")?.is_some() && before && !after {
        return Err(NotYet::not_read(ROTATION, RECORD).into());
    }
    // 10. The aim-offset refresh.
    let wanted = match name(vm, stance, "AimOffsetName")? {
        n if n == ow_object::ids::NameId::NONE => name(vm, source, "AimOffsetNodeName")?,
        n => n,
    };
    let current_name = match aim_offset {
        Some(a) => {
            resident(vm, a)?;
            Some(name(vm, a, "NodeName")?)
        }
        None => None,
    };
    let aim = if current_name != Some(wanted) {
        resident(vm, mesh)?;
        let found = ow_world::world::tree_lookup::find_anim_node(vm, mesh, wanted)?;
        let kept = match found {
            Some(n) if is_a(vm, n, "WillowGame.WillowAnimNodeAimOffset")? => Some(n),
            _ => None,
        };
        Some(kept)
    } else {
        None
    };
    // 11. The left-hand IK step, with the current stance then the new one.
    let mut controls = Vec::new();
    if let Some(ik) = object(vm, pawn, "LeftHandIK")? {
        resident(vm, ik)?;
        let wanted = boolean(vm, stance, "bAllowLeftHandIK")? && holds_empty;
        let active = float(vm, ik, "StrengthTarget")? > 0.5;
        if wanted != active {
            controls = plan_skel_control_active(vm, ik, wanted)?;
        }
    }
    Ok(Some(Plan { stance, current, is_override, can_crouch, aim_offset: aim, controls }))
}

fn write(vm: &mut Vm, o: ObjRef, name: &str, v: Value) -> Result<(), VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| bad(format!("no {name}")))?;
    vm.space.write(o, PropAddr::scalar(p), v)?;
    Ok(())
}

/// `event` on `stance`'s provider through the pawn's consumer (module documentation).
fn fire(vm: &mut Vm, pawn: ObjRef, stance: ObjRef, event: &str) -> Result<(), VmError> {
    let Some(provider) = object(vm, stance, "BehaviorProviderDefinition")? else {
        return Ok(());
    };
    let (handle, _) = read(vm, pawn, "ConsumerHandle")?;
    let Some(pid) = ow_behavior::kernel::natives::consumer_pid(vm, &handle) else {
        return Ok(());
    };
    match ow_behavior::kernel::natives::owner_of(vm, pid)? {
        Some(ow_behavior::kernel::natives::Owner::Kernel) => {
            let path = ow_behavior::provider::stock_path(vm, provider).map_err(|e| bad(e.to_string()))?;
            let fired = ow_behavior::kernel::natives::with_provider(vm, pid, &path, |vm, p| {
                p.fire_event(vm, event, &DetMap::new(), -1, &[])
            })?;
            match fired {
                Some(r) => r.map_err(|e| match e {
                    ow_behavior::provider::BehaviorError::Vm(v) => v,
                    other => bad(other.to_string()),
                }),
                None => Ok(()),
            }
        }
        Some(ow_behavior::kernel::natives::Owner::Crate(_)) => Err(NotYet::not_read(CRATE_OWNED, RECORD).into()),
        _ => Ok(()),
    }
}

/// The marks of #518's counters.
fn marks(vm: &Vm) -> (u64, u64, u64) {
    (vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions())
}

/// `WillowPlayerPawn.SetDefaultStance()` (module documentation).
pub fn set_default_stance(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let pawn = c.this.ok_or_else(|| bad("called without a pawn"))?;
    let Some(plan) = plan(c.vm, pawn)? else {
        return Ok(Value::None);
    };
    apply(c.vm, pawn, plan)?;
    Ok(Value::None)
}

/// The set-stance on `stance` from `source` directly (the tests' way to reach an override, which the pick never
/// selects).
#[cfg(test)]
fn set_stance(vm: &mut Vm, pawn: ObjRef, source: ObjRef, stance: ObjRef) -> Result<(), VmError> {
    match plan_set(vm, pawn, source, Some(stance))? {
        Some(plan) => apply(vm, pawn, plan),
        None => Ok(()),
    }
}

/// The plan's writes and events, in order; a refusal after a write carries the partial-writes prefix.
fn apply(vm: &mut Vm, pawn: ObjRef, plan: Plan) -> Result<(), VmError> {
    let before = marks(vm);
    let result = (|| -> Result<(), VmError> {
        if plan.is_override {
            write(vm, pawn, "OverrideStance", Value::Object(Some(plan.stance)))?;
        }
        if let Some(current) = plan.current {
            fire(vm, pawn, current, EXIT)?;
        }
        write(vm, pawn, "bNetDirty", Value::Bool(true))?;
        write(vm, pawn, "CurrentStance", Value::Object(Some(plan.stance)))?;
        fire(vm, pawn, plan.stance, ENTER)?;
        write(vm, pawn, "bCanCrouch", Value::Bool(plan.can_crouch))?;
        if let Some(aim) = plan.aim_offset {
            write(vm, pawn, "AimOffset", Value::Object(aim))?;
        }
        for w in &plan.controls {
            write(vm, w.control, "StrengthTarget", Value::Float(w.strength_target))?;
            write(vm, w.control, "BlendTimeToGo", Value::Float(w.blend_time))?;
            if let Some(s) = w.strength {
                write(vm, w.control, "ControlStrength", Value::Float(s))?;
            }
        }
        Ok(())
    })();
    match result {
        Ok(()) => Ok(()),
        Err(VmError::NotYet(n)) if marks(vm) != before => Err(n.after_partial_writes().into()),
        Err(e) => Err(e),
    }
}

/// `WillowPawn.SetDefaultStance()`: nothing (module documentation).
pub fn pawn_set_default_stance(_c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    Ok(Value::None)
}

/// Binds both bodies in Off (keep-existing; go_down's placeholders give way, `WillowAIPawn`'s stays).
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    crate::hit::bind_fn(vm, PLAYER_SET_DEFAULT_STANCE, set_default_stance)?;
    crate::hit::bind_fn(vm, PAWN_SET_DEFAULT_STANCE, pawn_set_default_stance)?;
    Ok(())
}

#[cfg(test)]
#[path = "stance_tests.rs"]
mod tests;
