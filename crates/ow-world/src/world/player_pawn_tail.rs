//! Owner: lane IMPL-LINK1-RBPHYSICS. `maya:8` after the collision setters, in `ParityMode::Off`:
//! `WillowPawn.InitializeRBPhysics` (2.A), `WillowPlayerPawn.GetLinearColor` (2.D) and
//! `WillowPlayerPawn.OnNativeApplyBodyClassStartingValuesHook` (2.F). 2.C (`GetFirstPersonWeaponHoldDef`) is in
//! [`super::weapon_hold`]. 2.B (`WillowPawn.InitializeBehaviorProviders`) is not bound here: the kernel pack binds its
//! rule (`ow_behavior::kernel::natives`, NATIVE_INITIALIZE_BEHAVIOR_PROVIDERS.md; #744), and a placeholder here made
//! that binding a duplicate registration (#744's landing check). Nor is 2.E
//! (`MeshComponent.GetInstancedMaterialInstanceConstant`): its rule is `ow_inventory::equip::material_instances`
//! (#744), and the `NotRead` placeholder this module bound kept that rule out wherever both were registered, with no
//! error (lane PLACEHOLDER-SHADOWS). AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from docs/verification/NATIVE_LINK1_RBPHYSICS_AND_AFTER.md section 2 alone (lane
//! ANALYST-LINK1-RBPHYSICS-AND-AFTER, #738 with its r1 fix, on main); this lane did not read the executable. **Every
//! rule is UNVERIFIED in the running game.** Reference keeps the stub policy.
//!
//! **2.A `InitializeRBPhysics`**, write-free and load-free. The pawn declares `bDeleteMe`, `Physics` (a byte),
//! `Components` (an array of object references), `CollisionComponent` and `bPushesRigidBodies`, else
//! `VmError::Script`. With `bDeleteMe` set: nothing. Else the walk over `Components` in order, after the `Physics`
//! filter (with `PHYS_RigidBody`, 10, only the `CollisionComponent`), None entries skipped. **The entry checks run
//! first, in one pass, before any answer** (review of #742): over every entry the walk meets, and the
//! `CollisionComponent` when `bPushesRigidBodies` is set, a store handle not loaded (or a slotless stand-in) is
//! [`NOT_LOADED`] (`NotRead`), and the field the attached test reads must be declared (`bAttached` a bool on a
//! template receiver, `Owner` an object on a live one), else `VmError::Script`. Then, in order, whether an entry is
//! **attached**: on a template receiver (`Origin::Default`), its `bAttached`; on a live one, **the attach record under
//! the pawn only** (`actors::attached_to`; NATIVE_COMPONENT_BOUNDS.md section 7 A, as `bounding_cylinder`'s live path
//! reads it): a live entry's own `bAttached` is not read. Attached is [`RIGID_BODY`] (`LayerAbsent`,
//! [`RIGID_LAYER`]); a live entry not in the record follows refusal L's convention (`bounding_cylinder::REFUSAL_L`
//! and its layer, `LayerAbsent`): attachment is unknown. This takes every such entry, colliding or not, as the
//! conservative side. Then `SetPushesRigidBodies(bPushesRigidBodies)`: clear (the port makes no pushing body) is
//! nothing, and the same-value write is skipped, so the native never writes; set, with a `CollisionComponent`, the
//! same attached test and answers. No entry and no pushing body: nothing.
//!
//! **2.D `GetLinearColor(C)`:** each of R, G, B and A as `byte as f32 * (1.0f32 / 255.0)` (an f32 multiply by the f32
//! constant, not a divide). Nothing written.
//!
//! **2.F `OnNativeApplyBodyClassStartingValuesHook()`:** `ImmuneToFallingDamage` := true (declared as a bool, else
//! `VmError::Script`); nothing else.

use std::rc::Rc;

use ow_core::NotYet;
use ow_object::heap::Origin;
use ow_object::ids::ObjRef;
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

use super::bounding_cylinder::{BOUNDS_LAYER, REFUSAL_L};
use super::component_setters::{Want, bad, bool_of, object_of, schemas, write};

/// The record of the rules.
pub const RECORD: &str = "docs/verification/NATIVE_LINK1_RBPHYSICS_AND_AFTER.md";
/// The native keys.
pub const INITIALIZE_RB_PHYSICS: &str = "WillowPawn.InitializeRBPhysics()";
pub const GET_LINEAR_COLOR: &str = "WillowPlayerPawn.GetLinearColor(color)";
pub const APPLY_HOOK: &str = "WillowPlayerPawn.OnNativeApplyBodyClassStartingValuesHook()";
/// 2.A's refusals.
pub const RIGID_BODY: &str = "WillowPawn.InitializeRBPhysics: a component's rigid body";
pub const RIGID_LAYER: &str = "a physics layer (rigid bodies)";
pub const NOT_LOADED: &str = "WillowPawn.InitializeRBPhysics: a component the store has not loaded (a load is a write)";
/// `PHYS_RigidBody`.
pub const PHYS_RIGID_BODY: u8 = 10;
const RB: &str = "WillowPawn.InitializeRBPhysics";

fn read(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<Value, VmError> {
    super::component_setters::Slots::of(o).required(vm, name, what)
}

fn resident(vm: &Vm, o: ObjRef) -> Result<bool, VmError> {
    let x = vm.space.heap.get(o)?;
    let slotless = vm.space.classes.get_layout(x.class).is_some_and(|l| x.slots.len() < l.slot_count() as usize);
    Ok(!(vm.defs.is_unloaded(&vm.space, o) || vm.defs.is_failed(&vm.space, o) || slotless))
}

/// The entry checks of the first pass (module documentation): resident, and the attached test's field declared.
fn check_entry(vm: &Vm, template: bool, entry: ObjRef) -> Result<(), VmError> {
    if !resident(vm, entry)? {
        return Err(NotYet::not_read(NOT_LOADED, RECORD).into());
    }
    if template {
        schemas(vm, entry, &[("bAttached", Want::Bool)], RB)
    } else {
        schemas(vm, entry, &[("Owner", Want::Object)], RB)
    }
}

/// Whether `entry` (checked) counts as attached (module documentation): `Some(true)` attached, `Some(false)` not
/// attached (a template entry with `bAttached` clear), `None` unknown (a live entry not in the attach record).
fn attached(vm: &Vm, pawn: ObjRef, template: bool, entry: ObjRef) -> Result<Option<bool>, VmError> {
    if template {
        return Ok(Some(bool_of(&read(vm, entry, "bAttached", RB)?, "bAttached", RB)?));
    }
    Ok(if crate::actors::attached_to(vm, entry, pawn)? { Some(true) } else { None })
}

/// The refusal for an entry by its attached state (`Ok` for a template entry not attached).
fn answer(state: Option<bool>) -> Result<(), VmError> {
    match state {
        Some(true) => Err(NotYet::layer_absent(RIGID_BODY, RIGID_LAYER, RECORD).into()),
        Some(false) => Ok(()),
        None => Err(NotYet::layer_absent(REFUSAL_L, BOUNDS_LAYER, RECORD).into()),
    }
}

/// `WillowPawn.InitializeRBPhysics()` in Off (module documentation). Writes nothing.
pub fn initialize_rb_physics(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let pawn = c.this.ok_or_else(|| bad(RB, "called without a pawn"))?;
    let vm: &Vm = c.vm;
    schemas(
        vm,
        pawn,
        &[
            ("bDeleteMe", Want::Bool),
            ("Components", Want::Array),
            ("CollisionComponent", Want::Object),
            ("bPushesRigidBodies", Want::Bool),
        ],
        RB,
    )?;
    if !matches!(super::component_setters::declared(vm, pawn, "Physics", RB)?, PropKind::Byte { .. }) {
        return Err(bad(RB, "Physics is not declared as a byte"));
    }
    let physics = match read(vm, pawn, "Physics", RB)? {
        Value::Byte(b) => b,
        other => return Err(bad(RB, format!("Physics holds {}, not a byte", other.kind_name()))),
    };
    if bool_of(&read(vm, pawn, "bDeleteMe", RB)?, "bDeleteMe", RB)? {
        return Ok(Value::None);
    }
    let template = matches!(vm.space.heap.get(pawn)?.origin, Origin::Default { .. });
    let collision = object_of(&read(vm, pawn, "CollisionComponent", RB)?, RB)?;
    let components = read(vm, pawn, "Components", RB)?;
    let mut walk = Vec::new();
    for item in components.elements().unwrap_or(&[]) {
        let Some(entry) = object_of(item, RB)? else { continue };
        if physics == PHYS_RIGID_BODY && Some(entry) != collision {
            continue;
        }
        walk.push(entry);
    }
    let pushing = if bool_of(&read(vm, pawn, "bPushesRigidBodies", RB)?, "bPushesRigidBodies", RB)? { collision } else { None };
    // The first pass: every entry's checks, before any answer.
    for &entry in walk.iter().chain(pushing.iter()) {
        check_entry(vm, template, entry)?;
    }
    for &entry in walk.iter().chain(pushing.iter()) {
        answer(attached(vm, pawn, template, entry)?)?;
    }
    Ok(Value::None)
}

/// The 2.D conversion of one byte.
pub fn byte_to_linear(b: u8) -> f32 {
    const SCALE: f32 = 1.0 / 255.0;
    f32::from(b) * SCALE
}

/// `WillowPlayerPawn.GetLinearColor(Color C)` (2.D).
pub fn get_linear_color(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "WillowPlayerPawn.GetLinearColor";
    let vm: &Vm = c.vm;
    let color = c.arg(0)?;
    let s = color.as_struct().ok_or_else(|| bad(WHAT, format!("C holds {}, not a struct", color.kind_name())))?;
    let layout = vm.space.classes.struct_layout(s.ty).ok_or_else(|| bad(WHAT, "a Color with no layout"))?;
    let field = |name: &str| -> Result<u8, VmError> {
        let (i, _) = vm.space.names.find(name).and_then(|n| layout.field(n)).ok_or_else(|| bad(WHAT, format!("Color has no {name}")))?;
        match s.fields.get(i as usize) {
            Some(Value::Byte(b)) => Ok(*b),
            other => Err(bad(WHAT, format!("Color.{name} holds {other:?}, not a byte"))),
        }
    };
    let rgba = [field("R")?, field("G")?, field("B")?, field("A")?];
    let ty = match vm.function(c.function).and_then(|d| d.result.as_ref()).map(|r| &r.kind) {
        Some(PropKind::Struct(t)) => *t,
        _ => return Err(bad(WHAT, "the return value is not declared as a struct")),
    };
    let out = vm.space.classes.struct_layout(ty).ok_or_else(|| bad(WHAT, "a LinearColor with no layout"))?;
    let mut fields = match out.zero() {
        Value::Struct(z) => z.fields.clone(),
        _ => return Err(bad(WHAT, "a LinearColor with no zero value")),
    };
    for (name, b) in ["R", "G", "B", "A"].into_iter().zip(rgba) {
        let (i, decl) =
            vm.space.names.find(name).and_then(|n| out.field(n)).ok_or_else(|| bad(WHAT, format!("LinearColor has no {name}")))?;
        if !matches!(decl.kind, PropKind::Float) {
            return Err(bad(WHAT, format!("LinearColor.{name} is not a float")));
        }
        fields[i as usize] = Value::Float(byte_to_linear(b));
    }
    Ok(Value::structure(ty, fields))
}

/// `WillowPlayerPawn.OnNativeApplyBodyClassStartingValuesHook()` (2.F).
pub fn apply_hook(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "WillowPlayerPawn.OnNativeApplyBodyClassStartingValuesHook";
    let pawn = c.this.ok_or_else(|| bad(WHAT, "called without a pawn"))?;
    schemas(c.vm, pawn, &[("ImmuneToFallingDamage", Want::Bool)], WHAT)?;
    write(c.vm, pawn, "ImmuneToFallingDamage", Value::Bool(true), WHAT)?;
    Ok(Value::None)
}

/// Binds the three keys (Off only; the caller checks the mode). A key bound first keeps its binding.
pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    for (key, f) in [
        (INITIALIZE_RB_PHYSICS, initialize_rb_physics as fn(&mut NativeCall<'_>) -> Result<Value, VmError>),
        (GET_LINEAR_COLOR, get_linear_color),
        (APPLY_HOOK, apply_hook),
    ] {
        if !vm.has_native(key) {
            vm.register_native(key, Rc::new(f))?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "player_pawn_tail_tests.rs"]
mod tests;
