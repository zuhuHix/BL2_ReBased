//! Owner: lane IMPL-BOUNDING-CYLINDER-AND-AFTER. The swing's tail past `GetBoundingCylinder` (links 3, 4, 13), in
//! `ParityMode::Off`. AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from docs/verification/NATIVE_BOUNDING_CYLINDER_AND_AFTER.md sections 3 to 7 (#799 at its
//! r1 fix fc8d2c42) alone; this lane did not read the executable. **Every rule is
//! UNVERIFIED in the running game.** Reference binds nothing. Each field is read with its declared kind, else
//! `VmError::Script`; each object read must be resident, else `NotRead`.
//!
//! - **`Actor.TraceComponent(...)`** (section 3): no component, or one whose `bAttached` is clear: false, nothing
//!   written. A component the attach record does not know, or one not attached to the receiver (recorded under
//!   another owner, or stale): the bounding cylinder's refusal L (the hub's ruling on review of #804). Attached: [`TRACE_LINE_CHECK`] (`LayerAbsent`); under the Off-only [`TraceComponentMiss`]
//!   harness setting (the hub's ruling on #799), a logged miss instead: false, no out written.
//! - **`WillowPawn.TryFullBodyGib(...)`** (section 4): `bGibbed` set or no `MyDeathDef`: `bGibbed`. Unless
//!   `bDidServerGibPawn`, a live pawn (health above 0) answers `bGibbed`; a dead one is [`GIB_TEST`]. With
//!   `bDidServerGibPawn`, [`THE_GIB`]. Reads only.
//! - **`WillowPawn.GetParent()`**, **`WillowAIPawn.GetParent()`** (section 5): the AI pawn's `SpawnParent`; else its
//!   `Base` when its `BodyClass` has `bOnlyDisplayParentInfoWhileAttached` and the base is a `WillowPawn`; else the pawn
//!   rule: a set `DrivenVehicle` is [`PARENT_VEHICLE`]; an `Owner` that is a `WillowPawn`; else None. Reads only.
//! - **`WillowPawn.GetMaxShieldStrength()`**, **`GetShieldStrength()`** (section 6): through `ShieldArmor`'s valid pool,
//!   its `MaxValue` / `CurrentValue`; else `ShieldMaxVar` / `ShieldVar` as floats. Reads only.
//! - **`WillowPlayerController.ShakeView(Info)`** and the `GearboxPlayerController` key (section 7): on a
//!   `WillowPlayerController`, `CurrentViewShake` := Info, then `ShakeOffsetLength`, `ShakeRotLength` and
//!   `ShakeFOVLength` := Info's `OffsetTime`, `RotTime`, `FOVTime`. Another receiver: [`SHAKE_OTHER`].
//! - **`PlayerController.DoForceFeedbackWaveform(Waveform, Instigator, Scale)`** (section 7): nothing when `PlayerInput`
//!   is set and its `bUsingGamepad` clear, or when `ForceFeedbackManager` or `PlayerReplicationInfo` is None; else
//!   [`FEEDBACK_PLAY`], decided before the `Scale` write. Reads only.
//! - **`WillowGameInfo.GetCombatMusicManager()`** (section 7): `GlobalCombatMusicManager` when set; None is
//!   [`MUSIC_LAZY`]. Since lane IMPL-COMBAT-MUSIC-AND-AFTER the key is bound to `crate::combat_music`'s lazy creation
//!   (registered first); this read stays for callers that must not create.

use ow_core::{NotYet, ParityMode};
use ow_object::ids::ObjRef;
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

use crate::hit::bind_fn;

/// The record of the rules.
pub const RECORD: &str = "docs/verification/NATIVE_BOUNDING_CYLINDER_AND_AFTER.md";
pub const TRACE_LINE_CHECK: &str = "Actor.TraceComponent: the component's line check";
pub const TRACE_LAYER: &str = "collision (a primitive component's own geometry)";
pub const GIB_TEST: &str = "TryFullBodyGib: the death definition's gib test";
pub const THE_GIB: &str = "TryFullBodyGib: the gib";
pub const PARENT_VEHICLE: &str = "GetParent: a driven vehicle's two queries";
pub const SHAKE_OTHER: &str = "ShakeView: the class's own implementation is not resolved";
pub const FEEDBACK_PLAY: &str = "DoForceFeedbackWaveform: the manager's play";
pub const MUSIC_LAZY: &str = "the combat music manager's lazy creation";
/// The harness line of the miss stand-in.
pub const TRACE_MISS_LINE: &str = "harness input: Actor.TraceComponent answered a miss on an attached component (the \
     port holds no per-component collision geometry; UNVERIFIED stand-in, the hub's ruling on #799)";

pub const TRACE_COMPONENT: &str = "Engine.Actor.TraceComponent";
pub const TRY_FULL_BODY_GIB: &str = "WillowGame.WillowPawn.TryFullBodyGib";
pub const GET_PARENT: [&str; 2] = ["WillowGame.WillowPawn.GetParent", "WillowGame.WillowAIPawn.GetParent"];
pub const SHIELD_MAX: &str = "WillowGame.WillowPawn.GetMaxShieldStrength";
pub const SHIELD: &str = "WillowGame.WillowPawn.GetShieldStrength";
pub const SHAKE_VIEW: [&str; 2] =
    ["WillowGame.WillowPlayerController.ShakeView", "GearboxFramework.GearboxPlayerController.ShakeView"];
pub const FORCE_FEEDBACK: &str = "Engine.PlayerController.DoForceFeedbackWaveform";
pub const COMBAT_MUSIC: &str = "WillowGame.WillowGameInfo.GetCombatMusicManager";

/// The hub's ruling on #799: `TraceComponent` on an attached component answers a logged miss (a VM service; its
/// presence turns it on). Off only; the slice's combat set-up turns it on.
#[derive(Debug, Clone, Copy, Default)]
pub struct TraceComponentMiss;

/// Turns [`TraceComponentMiss`] on (Off only, else a registration refusal; twice is a registration refusal too).
pub fn enable_trace_component_miss(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Err(VmError::Registration {
            key: "TraceComponentMiss".to_string(),
            reason: "the TraceComponent miss is an Off harness setting",
        });
    }
    vm.add_service(TraceComponentMiss).map(|_| ())
}

fn bad(what: &str, message: impl Into<String>) -> VmError {
    VmError::Script { function: what.to_string(), message: message.into() }
}

fn read(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<(Value, PropKind), VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| bad(what, format!("{} declares no {name}", path(vm, o))))?;
    let kind = vm.space.classes.decl(p).map(|d| d.kind.clone()).ok_or_else(|| bad(what, format!("{name} undeclared")))?;
    Ok((vm.space.read(o, ow_object::ids::PropAddr::scalar(p))?.clone(), kind))
}

fn path(vm: &Vm, o: ObjRef) -> String {
    vm.space.heap.get(o).ok().and_then(|x| vm.space.classes.get_layout(x.class)).map_or_else(String::new, |l| l.path().to_string())
}

fn object(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<Option<ObjRef>, VmError> {
    match read(vm, o, name, what)? {
        (Value::Object(x), PropKind::Object { .. }) => Ok(x),
        _ => Err(bad(what, format!("{name} is not declared as an object"))),
    }
}

fn boolean(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<bool, VmError> {
    match read(vm, o, name, what)? {
        (Value::Bool(b), PropKind::Bool) => Ok(b),
        _ => Err(bad(what, format!("{name} is not declared as a bool"))),
    }
}

/// A float or int field as a float (the shield's plain fields), else a data error.
fn number(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<f32, VmError> {
    match read(vm, o, name, what)? {
        (Value::Float(f), PropKind::Float | PropKind::FloatAttribute) => Ok(f),
        (Value::Int(i), PropKind::Int | PropKind::IntAttribute) => Ok(i as f32),
        _ => Err(bad(what, format!("{name} is not declared as a float or an int"))),
    }
}

fn resident(vm: &Vm, o: ObjRef) -> Result<(), VmError> {
    let x = vm.space.heap.get(o)?;
    let short = vm.space.classes.get_layout(x.class).is_some_and(|l| x.slots.len() < l.slot_count() as usize);
    if vm.defs.is_unloaded(&vm.space, o) || vm.defs.is_failed(&vm.space, o) || short {
        return Err(NotYet::not_read(format!("{}: an object the store has not loaded (a load is a write)", path(vm, o)), RECORD)
            .into());
    }
    Ok(())
}

fn is_a(vm: &Vm, o: ObjRef, wanted: &str) -> Result<bool, VmError> {
    let mut cursor = Some(vm.space.heap.get(o)?.class);
    let mut steps = 0;
    while let Some(c) = cursor.filter(|_| steps < 256) {
        steps += 1;
        let Some(l) = vm.space.classes.get_layout(c) else { return Ok(false) };
        if l.path().as_str().eq_ignore_ascii_case(wanted) {
            return Ok(true);
        }
        cursor = l.super_class();
    }
    Ok(false)
}

fn this(c: &NativeCall<'_>, what: &str) -> Result<ObjRef, VmError> {
    c.this.ok_or_else(|| bad(what, "called without a receiver"))
}

fn arg_object(c: &NativeCall<'_>, i: usize, what: &str) -> Result<Option<ObjRef>, VmError> {
    if i >= c.arg_count() {
        return Ok(None);
    }
    match c.arg(i)? {
        Value::Object(o) | Value::Interface(o) => Ok(*o),
        Value::None => Ok(None),
        other => Err(bad(what, format!("argument {i} is {}, not an object", other.kind_name()))),
    }
}

// --- TraceComponent -------------------------------------------------------------------------------------------------

/// What `TraceComponent` answers before any line check (module documentation): `Some(false)` for a miss with nothing
/// written, `None` when the component is attached (the line check). Reads only.
pub fn trace_component_gate(vm: &Vm, actor: ObjRef, component: Option<ObjRef>) -> Result<Option<bool>, VmError> {
    const WHAT: &str = "Actor.TraceComponent";
    let Some(component) = component else { return Ok(Some(false)) };
    resident(vm, component)?;
    if !boolean(vm, component, "bAttached", WHAT)? {
        return Ok(Some(false));
    }
    let known = match vm.service::<ow_world::actors::SceneLayer>() {
        Some(layer) => layer.try_borrow().map_err(|_| VmError::ServiceBorrowed("SceneLayer"))?.entry(component).is_some(),
        None => false,
    };
    // A component the record does not know, or one it records under another owner or that is stale, is refusal L,
    // as `GetBoundingCylinder` answers it (the hub's ruling on review of #804, item 1).
    if !known || !ow_world::actors::attached_to(vm, component, actor)? {
        return Err(NotYet::layer_absent(
            ow_world::world::bounding_cylinder::REFUSAL_L,
            ow_world::world::bounding_cylinder::BOUNDS_LAYER,
            ow_world::world::bounding_cylinder::RECORD,
        )
        .into());
    }
    Ok(None)
}

/// `TraceComponent`'s answer (module documentation): the gate, then the miss stand-in or the line check's
/// `LayerAbsent`. No out is written.
pub fn trace_answer(vm: &mut Vm, actor: ObjRef, component: Option<ObjRef>) -> Result<bool, VmError> {
    match trace_component_gate(vm, actor, component)? {
        Some(answer) => Ok(answer),
        None if vm.service::<TraceComponentMiss>().is_some() => {
            vm.log.push(TRACE_MISS_LINE.to_string());
            Ok(false)
        }
        None => Err(NotYet::layer_absent(TRACE_LINE_CHECK, TRACE_LAYER, RECORD).into()),
    }
}

fn trace_component(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "Actor.TraceComponent";
    let actor = this(c, WHAT)?;
    let component = arg_object(c, 2, WHAT)?;
    Ok(Value::Bool(trace_answer(c.vm, actor, component)?))
}

// --- TryFullBodyGib -------------------------------------------------------------------------------------------------

/// `TryFullBodyGib`'s answer before the gib test (module documentation). Reads only.
pub fn try_full_body_gib(vm: &Vm, pawn: ObjRef) -> Result<bool, VmError> {
    const WHAT: &str = "WillowPawn.TryFullBodyGib";
    resident(vm, pawn)?;
    let gibbed = boolean(vm, pawn, "bGibbed", WHAT)?;
    let death = object(vm, pawn, "MyDeathDef", WHAT)?;
    if gibbed || death.is_none() {
        return Ok(gibbed);
    }
    if boolean(vm, pawn, "bDidServerGibPawn", WHAT)? {
        return Err(NotYet::not_read(THE_GIB, RECORD).into());
    }
    if ow_world::pools::get_health(&vm.space, pawn)? > 0.0 {
        return Ok(gibbed);
    }
    Err(NotYet::not_read(GIB_TEST, RECORD).into())
}

// --- GetParent ------------------------------------------------------------------------------------------------------

/// `GetParent()` (module documentation). Reads only.
pub fn get_parent(vm: &Vm, pawn: ObjRef) -> Result<Option<ObjRef>, VmError> {
    const WHAT: &str = "WillowPawn.GetParent";
    resident(vm, pawn)?;
    // Every field the rule reads, declared first (review r1 of #799).
    let ai = is_a(vm, pawn, "WillowGame.WillowAIPawn")?;
    object(vm, pawn, "DrivenVehicle", WHAT)?;
    object(vm, pawn, "Owner", WHAT)?;
    if ai {
        object(vm, pawn, "SpawnParent", WHAT)?;
        object(vm, pawn, "Base", WHAT)?;
        object(vm, pawn, "BodyClass", WHAT)?;
    }
    if ai {
        if let Some(parent) = object(vm, pawn, "SpawnParent", WHAT)? {
            return Ok(Some(parent));
        }
        let base = object(vm, pawn, "Base", WHAT)?;
        if let Some(body) = object(vm, pawn, "BodyClass", WHAT)? {
            resident(vm, body)?;
            if boolean(vm, body, "bOnlyDisplayParentInfoWhileAttached", WHAT)?
                && let Some(base) = base
            {
                resident(vm, base)?;
                if is_a(vm, base, "WillowGame.WillowPawn")? {
                    return Ok(Some(base));
                }
            }
        }
    }
    if object(vm, pawn, "DrivenVehicle", WHAT)?.is_some() {
        return Err(NotYet::not_read(PARENT_VEHICLE, RECORD).into());
    }
    match object(vm, pawn, "Owner", WHAT)? {
        Some(owner) => {
            resident(vm, owner)?;
            Ok(is_a(vm, owner, "WillowGame.WillowPawn")?.then_some(owner))
        }
        None => Ok(None),
    }
}

// --- The shield getters ---------------------------------------------------------------------------------------------

/// The shield's (current, maximum) (module documentation). Reads only.
pub fn shield(vm: &Vm, pawn: ObjRef) -> Result<(f32, f32), VmError> {
    const WHAT: &str = "WillowPawn.GetShieldStrength";
    resident(vm, pawn)?;
    let (reference, kind) = read(vm, pawn, "ShieldArmor", WHAT)?;
    let reference_struct = match kind {
        PropKind::Struct(t) => vm.space.classes.struct_layout(t).map(|l| l.path().as_str().to_string()),
        _ => None,
    };
    if !reference_struct.is_some_and(|p| p.ends_with("ResourcePoolReference")) {
        return Err(bad(WHAT, "ShieldArmor is not declared as a ResourcePoolReference"));
    }
    // The two plain fields declared as ints, checked whatever the reference (review r1 of #799).
    let plain = |name: &str| -> Result<f32, VmError> {
        match read(vm, pawn, name, WHAT)? {
            (Value::Int(i), PropKind::Int) => Ok(i as f32),
            _ => Err(bad(WHAT, format!("{name} is not declared as an int"))),
        }
    };
    let (current, max) = (plain("ShieldVar")?, plain("ShieldMaxVar")?);
    let r = ow_world::pools::read_ref(&vm.space, &reference)?;
    // The manager is read by the resolve, so it must be resident first (review of #804, item 2).
    if let Some(m) = r.manager {
        resident(vm, m)?;
    }
    if let (_, Some(pool)) = ow_world::pools::resolve(&vm.space, r)? {
        resident(vm, pool)?;
        return Ok((number(vm, pool, "CurrentValue", WHAT)?, number(vm, pool, "MaxValue", WHAT)?));
    }
    Ok((current, max))
}

// --- ShakeView ------------------------------------------------------------------------------------------------------

/// The fields `ShakeView` copies: (the controller field, the struct member).
pub const SHAKE_LENGTHS: [(&str, &str); 3] =
    [("ShakeOffsetLength", "OffsetTime"), ("ShakeRotLength", "RotTime"), ("ShakeFOVLength", "FOVTime")];

/// `ShakeView(Info)` on `controller` (module documentation): the four writes, planned first.
pub fn shake_view(vm: &mut Vm, controller: ObjRef, info: &Value) -> Result<(), VmError> {
    const WHAT: &str = "WillowPlayerController.ShakeView";
    if !is_a(vm, controller, "WillowGame.WillowPlayerController")? {
        return Err(NotYet::not_read(SHAKE_OTHER, RECORD).into());
    }
    let s = info.as_struct().ok_or_else(|| bad(WHAT, "Info is not a struct"))?;
    let layout = vm.space.classes.struct_layout(s.ty).ok_or_else(|| bad(WHAT, "Info's struct has no layout"))?;
    if !layout.path().as_str().ends_with("GearboxViewShakeInfo") {
        return Err(bad(WHAT, format!("Info is a {}, not a GearboxViewShakeInfo", layout.path().as_str())));
    }
    let class = vm.space.heap.get(controller)?.class;
    let current = vm.space.prop(class, "CurrentViewShake").ok_or_else(|| bad(WHAT, "no CurrentViewShake"))?;
    if !matches!(vm.space.classes.decl(current).map(|d| &d.kind), Some(PropKind::Struct(t)) if *t == s.ty) {
        return Err(bad(WHAT, "CurrentViewShake is not declared as a GearboxViewShakeInfo"));
    }
    let mut plan = Vec::new();
    for (field, member) in SHAKE_LENGTHS {
        let p = vm.space.prop(class, field).ok_or_else(|| bad(WHAT, format!("no {field}")))?;
        if !matches!(vm.space.classes.decl(p).map(|d| &d.kind), Some(PropKind::Float)) {
            return Err(bad(WHAT, format!("{field} is not declared as a float")));
        }
        let n = vm.space.names.find(member).ok_or_else(|| bad(WHAT, format!("Info declares no {member}")))?;
        let (i, d) = layout.field(n).ok_or_else(|| bad(WHAT, format!("Info declares no {member}")))?;
        if !matches!(d.kind, PropKind::Float) {
            return Err(bad(WHAT, format!("Info's {member} is not declared as a float")));
        }
        plan.push((p, s.fields[i as usize].clone()));
    }
    vm.space.write(controller, ow_object::ids::PropAddr::scalar(current), info.clone())?;
    for (p, v) in plan {
        vm.space.write(controller, ow_object::ids::PropAddr::scalar(p), v).map_err(|e| ow_world::pools::after_partial_writes(e.into()))?;
    }
    Ok(())
}

// --- DoForceFeedbackWaveform ----------------------------------------------------------------------------------------

/// `DoForceFeedbackWaveform`'s gates (module documentation): `Ok` for nothing, else [`FEEDBACK_PLAY`]. Reads only.
pub fn force_feedback(vm: &Vm, controller: ObjRef, waveform: Option<ObjRef>) -> Result<(), VmError> {
    const WHAT: &str = "PlayerController.DoForceFeedbackWaveform";
    resident(vm, controller)?;
    // The declarations first (review r1 of #799): the controller's three fields and the waveform's `Scale`.
    let input = object(vm, controller, "PlayerInput", WHAT)?;
    let manager = object(vm, controller, "ForceFeedbackManager", WHAT)?;
    let info = object(vm, controller, "PlayerReplicationInfo", WHAT)?;
    // The waveform's `Scale` checked on its class, loading nothing; the waveform itself must be resident only past the
    // gates, where `Scale` is written (NATIVE_LINKS_3_4_TAIL.md section 9, row 6; lane IMPL-LINKS-3-4-TAIL).
    if let Some(w) = waveform {
        let class = vm.space.heap.get(w)?.class;
        let kind = vm.space.prop(class, "Scale").and_then(|p| vm.space.classes.decl(p)).map(|d| d.kind.clone());
        if !matches!(kind, Some(PropKind::Float)) {
            return Err(bad(WHAT, "the waveform's Scale is not declared as a float"));
        }
    }
    if let Some(input) = input {
        resident(vm, input)?;
        if !boolean(vm, input, "bUsingGamepad", WHAT)? {
            return Ok(());
        }
    }
    if manager.is_none() || info.is_none() {
        return Ok(());
    }
    if let Some(w) = waveform {
        resident(vm, w)?;
    }
    // Decided before the waveform's `Scale` write, so nothing is written.
    Err(NotYet::not_read(FEEDBACK_PLAY, RECORD).into())
}

// --- GetCombatMusicManager ------------------------------------------------------------------------------------------

/// `GetCombatMusicManager()` (module documentation). Reads only.
pub fn combat_music_manager(vm: &Vm, game: ObjRef) -> Result<ObjRef, VmError> {
    const WHAT: &str = "WillowGameInfo.GetCombatMusicManager";
    resident(vm, game)?;
    object(vm, game, "GlobalCombatMusicManager", WHAT)?.ok_or_else(|| NotYet::not_read(MUSIC_LAZY, RECORD).into())
}

/// Binds the keys in Off (Reference binds nothing); `GetParent`'s two keys leave `go_down`'s unresolved list.
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    bind_fn(vm, TRACE_COMPONENT, trace_component)?;
    bind_fn(vm, TRY_FULL_BODY_GIB, |c| {
        let me = this(c, "WillowPawn.TryFullBodyGib")?;
        Ok(Value::Bool(try_full_body_gib(c.vm, me)?))
    })?;
    for key in GET_PARENT {
        bind_fn(vm, key, |c| {
            let me = this(c, "WillowPawn.GetParent")?;
            Ok(Value::Object(get_parent(c.vm, me)?))
        })?;
    }
    bind_fn(vm, SHIELD_MAX, |c| {
        let me = this(c, "WillowPawn.GetMaxShieldStrength")?;
        Ok(Value::Float(shield(c.vm, me)?.1))
    })?;
    bind_fn(vm, SHIELD, |c| {
        let me = this(c, "WillowPawn.GetShieldStrength")?;
        Ok(Value::Float(shield(c.vm, me)?.0))
    })?;
    for key in SHAKE_VIEW {
        bind_fn(vm, key, |c| {
            let me = this(c, "ShakeView")?;
            let info = c.arg(0)?.clone();
            shake_view(c.vm, me, &info)?;
            Ok(Value::None)
        })?;
    }
    bind_fn(vm, FORCE_FEEDBACK, |c| {
        let me = this(c, "PlayerController.DoForceFeedbackWaveform")?;
        let waveform = arg_object(c, 0, "PlayerController.DoForceFeedbackWaveform")?;
        force_feedback(c.vm, me, waveform)?;
        Ok(Value::None)
    })?;
    bind_fn(vm, COMBAT_MUSIC, |c| {
        let me = this(c, "WillowGameInfo.GetCombatMusicManager")?;
        Ok(Value::Object(Some(combat_music_manager(c.vm, me)?)))
    })?;
    Ok(())
}

#[cfg(test)]
#[path = "bcyl_tail_tests.rs"]
mod tests;
