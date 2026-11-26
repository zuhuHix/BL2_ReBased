//! Owner: lane IMPL-LINK6-AFTER-TAKE (decision 2026-11-26-57). `Actor.SetBase(NewBase, optional NewFloor, optional
//! SkelComp, optional AttachName)` in `ParityMode::Off`. AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from docs/verification/NATIVE_ACTOR_SETBASE.md section 3 (#795 as landed) alone; this
//! lane did not read the executable. **Every rule is UNVERIFIED in the running game.** Reference binds nothing.
//!
//! **Which receivers.** The rule applies to a receiver whose first native class (the class table only) is one of the
//! seven tables the note read, plus `PawnInteractionProxy` (NATIVE_PAWN_INTERACTION_PROXY.md section 5;
//! [`READ_CLASSES`]). Any other receiver is [`NOT_RESOLVED`] (`NotRead`), decided first.
//! The native callers (`SetPhysics`' base search, the base-clearing transition) are not read: `SetPhysics`' base-setter
//! `NotRead` stands, and this binding does not lift it.
//!
//! **The preflight** (write-free and load-free; each field read with its declared kind, else `VmError::Script`; each
//! base, old base and actor the loop walk reaches resident, else [`NOT_LOADED`]). First the receiver's declarations
//! the commit reads, as the note lists them (`Role` an enum byte, `bHardAttach`, `RelativeLocation`,
//! `RelativeRotation`, `BaseSkelComponent`, `BaseBoneName`; review of #800, item 7), then steps 1 to 5 in order, then
//! step 8's math cases (a base `Rotation` part missing or not an int is `VmError::Script`, item 6):
//! 1. a `NewBase` of `StaticMeshCollectionActor` / `BlockingMeshCollectionActor` (by class names already in the name
//!    table, along its class chain): [`COLLECTION`] (`NotRead`; its replacement actor's lookup is not read);
//! 2. `NewBase`'s `Base` chain reaching the receiver: nothing (a loop that never reaches it is `VmError::Script`);
//! 3. a receiver `bStatic` or not `bMovable`: a moving `NewBase` (not `bStatic`, `bMovable`), or one with `bDeleteMe`,
//!    nothing;
//! 4. a `SkelComp` whose `Owner` is not `NewBase`: nothing; one that passes: [`BONE_PLACEMENT`] (`NotRead`);
//! 5. `NewBase`, `SkelComp` and `AttachName` all as they are: nothing (no event, no `BaseChange`);
//! 8. with a `NewBase` that is not `bWorldGeometry`: a hard-attached receiver that is not a simulated proxy needs the
//!    full inverse ([`HARD_ATTACH`]), except in NATIVE_PAWN_INTERACTION_PROXY.md section 5's identity case (the
//!    receiver exactly on an unturned base with `DrawScale` 1, `DrawScale3D` (1, 1, 1) and `PrePivot` zero, those three
//!    declared on the base), which gets a zero relative pose (lane IMPL-PAWN-INTERACTION-PROXY); a base with a non-zero `Rotation`, or a receiver rotation not normalised (each
//!    part within the 16-bit signed range, pitch within a quarter turn), needs the rotation conversion ([`TURNED`]).
//!
//! **The commit**, steps 6 to 10: the old base (not `bWorldGeometry`) loses every copy of the receiver from `Attached`,
//! then gets `Detach(receiver)`; then the preflight's checks run again on the state that left (a refusal there is
//! prefixed); `Base` := `NewBase`, `BaseSkelComponent` and `BaseBoneName` := None; with a `NewBase` not
//! `bWorldGeometry`: the relative pose when the receiver is not hard-attached or is the authority (`RelativeLocation` :=
//! `Location` − `NewBase.Location`, `RelativeRotation` := `Rotation`), the receiver appended to `NewBase.Attached`, then
//! `Attach(receiver)`; then `BaseChange()`. A refusal after the first write is prefixed. **Gap:** the port's event call
//! has no probe masks (the game skips an event the state ignores).

use ow_core::NotYet;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

/// The record of the rule.
pub const RECORD: &str = "docs/verification/NATIVE_ACTOR_SETBASE.md (section 3)";
/// The script native's key and package.
pub const KEY: &str = "Actor.SetBase(object,vector,object,name)";
const PACKAGE: &str = "Engine";
const WHAT: &str = "Actor.SetBase";
/// The first native classes whose tables the note read.
pub const READ_CLASSES: [&str; 8] = [
    "Engine.Actor",
    "Engine.Inventory",
    "Engine.Weapon",
    "WillowGame.WillowWeapon",
    "Engine.WillowInventory",
    "Engine.DroppedPickup",
    "WillowGame.WillowPickup",
    // NATIVE_PAWN_INTERACTION_PROXY.md section 5 (lane IMPL-PAWN-INTERACTION-PROXY): the proxy's table holds Actor's body.
    "WillowGame.PawnInteractionProxy",
];
pub const NOT_RESOLVED: &str = "SetBase: the class's own implementation is not resolved";
pub const NOT_LOADED: &str = "SetBase: an object the store has not loaded (a load is a write)";
pub const COLLECTION: &str = "SetBase: a collection actor as the base";
pub const BONE_PLACEMENT: &str = "SetBase: the bone and socket placement";
pub const TURNED: &str = "SetBase: the relative rotation to a turned base";
pub const HARD_ATTACH: &str = "SetBase: the hard-attach relative transform";
const COLLECTION_CLASSES: [&str; 2] = ["StaticMeshCollectionActor", "BlockingMeshCollectionActor"];

fn script(message: String) -> VmError {
    VmError::Script { function: WHAT.to_string(), message: format!("{WHAT}: {message}") }
}

fn resident(vm: &Vm, o: ObjRef) -> Result<(), VmError> {
    let x = vm.space.heap.get(o)?;
    let short = vm.space.classes.get_layout(x.class).is_some_and(|l| x.slots.len() < l.slot_count() as usize);
    if vm.defs.is_unloaded(&vm.space, o) || vm.defs.is_failed(&vm.space, o) || short {
        return Err(NotYet::not_read(NOT_LOADED, RECORD).into());
    }
    Ok(())
}

fn field(vm: &Vm, o: ObjRef, name: &str) -> Result<(PropKind, Value), VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| script(format!("{} declares no {name}", class_path(vm, o))))?;
    let kind = vm.space.classes.decl(p).map(|d| d.kind.clone()).ok_or_else(|| script(format!("{name} undeclared")))?;
    Ok((kind, vm.space.read(o, PropAddr::scalar(p))?.clone()))
}

fn flag(vm: &Vm, o: ObjRef, name: &str) -> Result<bool, VmError> {
    match field(vm, o, name)? {
        (PropKind::Bool, Value::Bool(b)) => Ok(b),
        (k, _) => Err(script(format!("{name} is declared as {}, not a bool", k.kind_name()))),
    }
}

fn object(vm: &Vm, o: ObjRef, name: &str) -> Result<Option<ObjRef>, VmError> {
    match field(vm, o, name)? {
        (PropKind::Object { .. }, Value::Object(x)) => Ok(x),
        (k, _) => Err(script(format!("{name} is declared as {}, not an object", k.kind_name()))),
    }
}

fn name_of(vm: &Vm, o: ObjRef, name: &str) -> Result<Value, VmError> {
    match field(vm, o, name)? {
        (PropKind::Name, v @ Value::Name(_)) => Ok(v),
        (k, _) => Err(script(format!("{name} is declared as {}, not a name", k.kind_name()))),
    }
}

fn structure(vm: &Vm, o: ObjRef, name: &str) -> Result<Value, VmError> {
    match field(vm, o, name)? {
        (PropKind::Struct(_), v) => Ok(v),
        (k, _) => Err(script(format!("{name} is declared as {}, not a struct", k.kind_name()))),
    }
}

fn objects(vm: &Vm, o: ObjRef, name: &str) -> Result<Vec<Value>, VmError> {
    match field(vm, o, name)? {
        (PropKind::Array(_), Value::Array(items)) => Ok((*items).clone()),
        (k, _) => Err(script(format!("{name} is declared as {}, not an array", k.kind_name()))),
    }
}

fn class_path(vm: &Vm, o: ObjRef) -> String {
    vm.space.heap.get(o).ok().and_then(|x| vm.space.classes.get_layout(x.class)).map_or_else(String::new, |l| l.path().to_string())
}

/// The path of `o`'s first native class (the class table only, as `Actor.SetPhysics`' layers take it).
pub(crate) fn first_native(vm: &Vm, o: ObjRef) -> Result<Option<String>, VmError> {
    let code = vm.defs.code();
    let mut cursor = Some(vm.space.heap.get(o)?.class);
    while let Some(c) = cursor {
        let Some(layout) = vm.space.classes.get_layout(c) else { return Ok(None) };
        let native = match code.key_of(c) {
            Some(k) => code.class(k).body.as_ref().map_or(true, |b| b.is_native()),
            None => vm.space.classes.is_native(c),
        };
        if native {
            return Ok(Some(layout.path().to_string()));
        }
        cursor = layout.super_class();
    }
    Ok(None)
}

/// Whether any class on `o`'s chain is named one of `names` (names already in the table only; nothing interned).
fn chain_named(vm: &Vm, o: ObjRef, names: &[&str]) -> Result<bool, VmError> {
    let ids: Vec<_> = names.iter().filter_map(|n| vm.space.names.find(n)).collect();
    if ids.is_empty() {
        return Ok(false);
    }
    let mut cursor = Some(vm.space.heap.get(o)?.class);
    while let Some(c) = cursor {
        let Some(layout) = vm.space.classes.get_layout(c) else { break };
        let leaf = layout.path().as_str().rsplit('.').next().unwrap_or_default().to_string();
        if names.iter().any(|n| n.eq_ignore_ascii_case(&leaf)) {
            return Ok(true);
        }
        cursor = layout.super_class();
    }
    Ok(false)
}

fn struct_field(vm: &Vm, v: &Value, name: &str) -> Option<Value> {
    let s = v.as_struct()?;
    let id = vm.space.names.find(name)?;
    let (i, _) = vm.space.classes.struct_layout(s.ty)?.field(id)?;
    s.fields.get(usize::try_from(i).ok()?).cloned()
}

/// Whether the byte `Role` of `o` is its enum's `wanted`.
fn role_is(vm: &Vm, o: ObjRef, wanted: &str) -> Result<bool, VmError> {
    let (kind, value) = field(vm, o, "Role")?;
    let (PropKind::Byte { enum_: Some(e) }, Value::Byte(raw)) = (kind, value) else {
        return Err(script("Role is not declared as an enum byte".to_string()));
    };
    let at = vm
        .space
        .names
        .find(wanted)
        .and_then(|id| vm.space.classes.enum_layout(e).and_then(|l| l.index_of(id)))
        .ok_or_else(|| script(format!("Role's enum has no {wanted}")))?;
    Ok(usize::from(raw) == at)
}

/// Whether the rotator `r` is zero: each of `Pitch`, `Yaw` and `Roll` an int (review of #800, item 6: a missing or
/// non-int part is `VmError::Script`, not 0).
fn is_zero_rotation(vm: &Vm, r: &Value) -> Result<bool, VmError> {
    let mut zero = true;
    for p in ["Pitch", "Yaw", "Roll"] {
        match struct_field(vm, r, p) {
            Some(Value::Int(i)) => zero &= i == 0,
            other => return Err(script(format!("the base's Rotation.{p} is {other:?}, not an int"))),
        }
    }
    Ok(zero)
}

fn is_normalised(vm: &Vm, r: &Value) -> bool {
    let part = |p: &str| match struct_field(vm, r, p) {
        Some(Value::Int(i)) => Some(i),
        _ => None,
    };
    let in_short = |i: i32| (-32768..=32767).contains(&i);
    match (part("Pitch"), part("Yaw"), part("Roll")) {
        (Some(p), Some(y), Some(r)) => in_short(p) && in_short(y) && in_short(r) && (-16384..=16384).contains(&p),
        _ => false,
    }
}

/// The declarations an actor in the call must have (module documentation).
fn check_actor(vm: &Vm, o: ObjRef) -> Result<(), VmError> {
    resident(vm, o)?;
    object(vm, o, "Base")?;
    for f in ["bStatic", "bMovable", "bDeleteMe", "bWorldGeometry"] {
        flag(vm, o, f)?;
    }
    for f in ["Location", "Rotation"] {
        structure(vm, o, f)?;
    }
    objects(vm, o, "Attached")?;
    Ok(())
}

/// What the commit does.
struct Plan {
    old: Option<ObjRef>,
    new: Option<ObjRef>,
    pose: bool,
}

/// The receiver's declarations the commit reads (`Role` an enum byte, `bHardAttach`, `BaseSkelComponent`,
/// `BaseBoneName`, `RelativeLocation`, `RelativeRotation`), checked up front as the note lists them (review of #800,
/// item 7), before any step can answer.
fn check_receiver(vm: &Vm, receiver: ObjRef) -> Result<(), VmError> {
    check_actor(vm, receiver)?;
    object(vm, receiver, "BaseSkelComponent")?;
    name_of(vm, receiver, "BaseBoneName")?;
    flag(vm, receiver, "bHardAttach")?;
    for f in ["RelativeLocation", "RelativeRotation"] {
        structure(vm, receiver, f)?;
    }
    role_is(vm, receiver, "ROLE_Authority")?;
    Ok(())
}

/// The checks of the receiver and the new base that step 8 needs (also run again after `Detach`).
fn check_pose(vm: &Vm, receiver: ObjRef, new: Option<ObjRef>) -> Result<bool, VmError> {
    check_receiver(vm, receiver)?;
    let Some(new) = new else { return Ok(false) };
    check_actor(vm, new)?;
    if flag(vm, new, "bWorldGeometry")? {
        return Ok(false);
    }
    let hard = flag(vm, receiver, "bHardAttach")?;
    let simulated = role_is(vm, receiver, "ROLE_SimulatedProxy")?;
    // The identity case (NATIVE_PAWN_INTERACTION_PROXY.md section 5): a hard-attached receiver that is not a simulated
    // proxy, sitting exactly on an unturned, unscaled, unpivoted base, gets a zero relative pose; any other is the full
    // inverse's `NotRead`.
    if hard && !simulated && !identity(vm, receiver, new)? {
        return Err(NotYet::not_read(HARD_ATTACH, RECORD).into());
    }
    let pose = !hard || !simulated;
    if pose {
        let base_rotation = structure(vm, new, "Rotation")?;
        let rotation = structure(vm, receiver, "Rotation")?;
        if !is_zero_rotation(vm, &base_rotation)? || !is_normalised(vm, &rotation) {
            return Err(NotYet::not_read(TURNED, RECORD).into());
        }
    }
    Ok(pose)
}

/// Whether every float field of the vector struct `v` is `want` (a missing or non-float field is a data error).
fn vector_is(vm: &Vm, v: &Value, want: f32, what: &str) -> Result<bool, VmError> {
    let mut all = true;
    for f in ["X", "Y", "Z"] {
        match struct_field(vm, v, f) {
            Some(Value::Float(x)) => all &= x == want,
            other => return Err(script(format!("{what}.{f} is {other:?}, not a float"))),
        }
    }
    Ok(all)
}

/// Section 5's identity case (module documentation): the receiver's `Location` and `Rotation` equal the base's, the
/// base's `Rotation` zero, `DrawScale` 1, `DrawScale3D` (1, 1, 1) and `PrePivot` zero. The base's three scale fields
/// must be declared (a float and two structs), else `VmError::Script`. Reads only.
fn identity(vm: &Vm, receiver: ObjRef, new: ObjRef) -> Result<bool, VmError> {
    let scale = match field(vm, new, "DrawScale")? {
        (PropKind::Float, Value::Float(f)) => f,
        (k, _) => return Err(script(format!("DrawScale is declared as {}, not a float", k.kind_name()))),
    };
    let scale_3d = structure(vm, new, "DrawScale3D")?;
    let pivot = structure(vm, new, "PrePivot")?;
    let (base_location, base_rotation) = (structure(vm, new, "Location")?, structure(vm, new, "Rotation")?);
    let (location, rotation) = (structure(vm, receiver, "Location")?, structure(vm, receiver, "Rotation")?);
    Ok(location == base_location
        && rotation == base_rotation
        && is_zero_rotation(vm, &base_rotation)?
        && scale == 1.0
        && vector_is(vm, &scale_3d, 1.0, "DrawScale3D")?
        && vector_is(vm, &pivot, 0.0, "PrePivot")?)
}

/// `SetBase(new, , skel, attach)`'s preflight alone, for a caller that must decide its refusals before its own writes
/// (lane IMPL-PAWN-INTERACTION-PROXY: `SetHardAttach`'s first `SetBase(None)`). Reads only.
pub(crate) fn preflight(vm: &Vm, receiver: ObjRef, new: Option<ObjRef>, skel: Option<ObjRef>, attach: &Value) -> Result<(), VmError> {
    plan(vm, receiver, new, skel, attach).map(|_| ())
}

/// `SetBase(new, , skel, attach)` as the native runs it: the preflight, then the commit, a refusal after its first
/// write prefixed. For `SetHardAttach`'s two inner calls.
pub(crate) fn run(vm: &mut Vm, receiver: ObjRef, new: Option<ObjRef>, skel: Option<ObjRef>, attach: &Value) -> Result<(), VmError> {
    let Some(plan) = plan(vm, receiver, new, skel, attach)? else {
        return Ok(());
    };
    let entry = marks(vm);
    match commit(vm, receiver, &plan) {
        Ok(()) => Ok(()),
        Err(e) if marks(vm) != entry => Err(crate::pools::after_partial_writes(e)),
        Err(e) => Err(e),
    }
}

/// The preflight (module documentation): `None` for nothing to do. Reads only.
fn plan(vm: &Vm, receiver: ObjRef, new: Option<ObjRef>, skel: Option<ObjRef>, attach: &Value) -> Result<Option<Plan>, VmError> {
    let native = first_native(vm, receiver)?;
    if !native.as_deref().is_some_and(|n| READ_CLASSES.iter().any(|c| c.eq_ignore_ascii_case(n))) {
        return Err(NotYet::not_read(NOT_RESOLVED, RECORD).into());
    }
    check_receiver(vm, receiver)?;
    let old = object(vm, receiver, "Base")?;
    if let Some(o) = old {
        check_actor(vm, o)?;
    }
    if let Some(n) = new {
        check_actor(vm, n)?;
        // Step 1.
        if chain_named(vm, n, &COLLECTION_CLASSES)? {
            return Err(NotYet::not_read(COLLECTION, RECORD).into());
        }
        // Step 2: the loop walk.
        let mut seen = Vec::new();
        let mut cursor = Some(n);
        while let Some(c) = cursor {
            if c == receiver {
                return Ok(None);
            }
            if seen.contains(&c) || seen.len() > 1024 {
                return Err(script("the new base's Base chain loops without reaching the receiver".to_string()));
            }
            seen.push(c);
            resident(vm, c)?;
            cursor = object(vm, c, "Base")?;
        }
        // Step 3.
        if flag(vm, receiver, "bStatic")? || !flag(vm, receiver, "bMovable")? {
            if (!flag(vm, n, "bStatic")? && flag(vm, n, "bMovable")?) || flag(vm, n, "bDeleteMe")? {
                return Ok(None);
            }
        }
    }
    // Step 4.
    if let Some(s) = skel {
        resident(vm, s)?;
        if object(vm, s, "Owner")? != new {
            return Ok(None);
        }
        return Err(NotYet::not_read(BONE_PLACEMENT, RECORD).into());
    }
    // Step 5.
    let same_skel = object(vm, receiver, "BaseSkelComponent")?.is_none();
    if new == old && same_skel && name_of(vm, receiver, "BaseBoneName")? == *attach {
        return Ok(None);
    }
    let pose = check_pose(vm, receiver, new)?;
    Ok(Some(Plan { old, new, pose }))
}

fn write(vm: &mut Vm, o: ObjRef, name: &str, v: Value) -> Result<(), VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| script(format!("no {name}")))?;
    vm.space.write(o, PropAddr::scalar(p), v)?;
    Ok(())
}

/// `a - b` for two vector structs, field by field (floats).
fn minus(vm: &Vm, a: &Value, b: &Value) -> Value {
    let mut out = a.clone();
    if let Some(s) = out.struct_mut() {
        for f in ["X", "Y", "Z"] {
            let Some(id) = vm.space.names.find(f) else { continue };
            let Some((i, _)) = vm.space.classes.struct_layout(s.ty).and_then(|l| l.field(id)) else { continue };
            let i = i as usize;
            if let (Some(Value::Float(x)), Some(Value::Float(y))) = (s.fields.get(i).cloned(), struct_field(vm, b, f)) {
                s.fields[i] = Value::Float(x - y);
            }
        }
    }
    out
}

fn commit(vm: &mut Vm, receiver: ObjRef, plan: &Plan) -> Result<(), VmError> {
    // Step 6.
    if let Some(old) = plan.old
        && !flag(vm, old, "bWorldGeometry")?
    {
        let kept: Vec<Value> =
            objects(vm, old, "Attached")?.into_iter().filter(|v| *v != Value::Object(Some(receiver))).collect();
        write(vm, old, "Attached", Value::array(kept))?;
        crate::actors::send_event(vm, old, "Detach", vec![Value::Object(Some(receiver))])?;
    }
    // The checks again, on the state `Detach` left.
    let pose = if plan.old.is_some() { check_pose(vm, receiver, plan.new)? } else { plan.pose };
    // Step 7.
    write(vm, receiver, "Base", Value::Object(plan.new))?;
    write(vm, receiver, "BaseSkelComponent", Value::Object(None))?;
    write(vm, receiver, "BaseBoneName", Value::Name(ow_object::ids::NameId::NONE))?;
    if let Some(new) = plan.new
        && !flag(vm, new, "bWorldGeometry")?
    {
        // Step 8.
        if pose {
            let relative = minus(vm, &structure(vm, receiver, "Location")?, &structure(vm, new, "Location")?);
            write(vm, receiver, "RelativeLocation", relative)?;
            let rotation = structure(vm, receiver, "Rotation")?;
            write(vm, receiver, "RelativeRotation", rotation)?;
        }
        // Step 9.
        let mut attached = objects(vm, new, "Attached")?;
        attached.push(Value::Object(Some(receiver)));
        write(vm, new, "Attached", Value::array(attached))?;
        crate::actors::send_event(vm, new, "Attach", vec![Value::Object(Some(receiver))])?;
    }
    // Step 10.
    crate::actors::send_event(vm, receiver, "BaseChange", vec![])?;
    Ok(())
}

fn marks(vm: &Vm) -> (u64, u64, u64) {
    (vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions())
}

fn set_base(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let receiver = c.this.ok_or_else(|| script("called with no actor".to_string()))?;
    let object_arg = |c: &NativeCall<'_>, i: usize| -> Result<Option<ObjRef>, VmError> {
        if i >= c.arg_count() || !c.has(i) {
            return Ok(None);
        }
        match c.arg(i)? {
            Value::Object(o) => Ok(*o),
            Value::None => Ok(None),
            other => Err(script(format!("argument {i} is {}, not an object", other.kind_name()))),
        }
    };
    let new = object_arg(c, 0)?;
    let skel = object_arg(c, 2)?;
    let attach = if c.arg_count() > 3 && c.has(3) {
        match c.arg(3)? {
            v @ Value::Name(_) => v.clone(),
            Value::None => Value::Name(ow_object::ids::NameId::NONE),
            other => return Err(script(format!("AttachName is {}, not a name", other.kind_name()))),
        }
    } else {
        Value::Name(ow_object::ids::NameId::NONE)
    };
    let Some(plan) = plan(c.vm, receiver, new, skel, &attach)? else {
        return Ok(Value::None);
    };
    let entry = marks(c.vm);
    match commit(c.vm, receiver, &plan) {
        Ok(()) => Ok(Value::None),
        Err(e) if marks(c.vm) != entry => Err(crate::pools::after_partial_writes(e)),
        Err(e) => Err(e),
    }
}

/// Binds the key in Off (the caller binds in Off only; keep-existing).
pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    if !vm.is_bound_in(PACKAGE, KEY) {
        vm.register_native_in(PACKAGE, KEY, std::rc::Rc::new(set_base))?;
    }
    Ok(())
}

// Tests: `world/tests.rs`, module `link6` (they need the world fixture).
