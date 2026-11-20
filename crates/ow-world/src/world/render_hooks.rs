//! Owner: lane IMPL-BODYCOMP-RENDER-HOOKS (decision 2026-11-19-NNN, number pending). The three rendering hooks the
//! pawn's `PostInitBodyComposition` runs on each made component, in `ParityMode::Off`, as the body composition's
//! preflight (crate::world::body_composition). The note is NATIVE_BODYCOMP_RENDER_HOOKS.md, section 4. Every rule is
//! **UNVERIFIED** in the running game. AI-assisted (Claude Code).
//!
//! Each hook either does nothing or refuses, read only and before any write. Under today's rules none of them writes:
//! hook 2 would write only where its unidentified pawn test decides, and hook 3 only past its unidentified array check,
//! and both are refused there. So the body composition's writes are unchanged.
//!
//! 1. **The gear-likeness step** ([`gear_likeness`]), before the custom-head step. The made component's class is asked
//!    whether it implements `WillowGame.IGearLikenessConsumer`, read only, in `implements`' own order: a class chain
//!    whose bodies did not all decode answers by the structural rule (synthetic packages), as `implements` does;
//!    otherwise only interface entries already resolved answer. An entry not resolved yet is [`GEAR_UNRESOLVED`]
//!    (`NotRead`): resolving it would load. A class the code index does not hold (hand-declared) answers through its
//!    nearest indexed parent, or no when it has none. The interface not defined at set-up is [`GEAR_INTERFACE`].
//!    No: nothing. Yes: the refresh, not read and render-side, is [`GEAR_REFRESH`] (`LayerAbsent`).
//! 2. **The owner-no-see setting** ([`owner_no_see`]): the gate's known parts first, in the game's order. Not a
//!    `PrimitiveComponent`; a pawn `Controller` that is None, not an `Engine.PlayerController`, or not local by the Off
//!    `IsLocalPlayerController` rule (crate::world::is_local_player): nothing. Then the template must be settled
//!    ([`NOT_SETTLED`]), and `SetOwnerNoSee(true)`'s planner runs on the slots the attach produces. A flag already
//!    true is nothing. Otherwise the unidentified pawn test decides the outcome, so it is [`PAWN_TEST`] (`NotRead`),
//!    ahead of the setter's own deferred-reattach refusal.
//! 3. **The light environment** ([`light_environment`]): the pawn's `LightEnvironment` (declared as an object) None,
//!    or a made class that is not a `MeshComponent`: nothing. Then the template must be settled, and its
//!    `bAcceptsLights` (fitted) clear is nothing; set reaches the unidentified array check, [`ARRAY_CHECK`] (`NotRead`).

use ow_core::NotYet;
use ow_object::ids::{ClassId, ObjRef};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use crate::world::component_setters::{self, Slots};

/// The record every refusal here carries.
pub const RECORD: &str = "docs/verification/NATIVE_BODYCOMP_RENDER_HOOKS.md";
/// The interface hook 1 asks about (by fit, WillowGame's only gear-likeness interface name).
pub const GEAR_LIKENESS: &str = "WillowGame.IGearLikenessConsumer";
pub const GEAR_INTERFACE: &str = "the gear-likeness interface not resolved at set-up";
pub const GEAR_UNRESOLVED: &str =
    "the gear-likeness step: an interface entry of the made component's class not resolved at set-up";
pub const GEAR_REFRESH: &str = "InitializeBodyComposition: the body's rendering hooks (the gear-likeness refresh)";
pub const RENDER_LAYER: &str = "a rendering layer";
pub const NOT_SETTLED: &str = "the rendering hooks on a template not loaded at set-up (its values are not settled)";
pub const PAWN_TEST: &str = "the owner-no-see gate's pawn test (not identified)";
pub const ARRAY_CHECK: &str = "the light-environment step's array check (not identified)";

const WHAT: &str = "InitializeBodyComposition";
const OWNER_NO_SEE: &str = "PrimitiveComponent.SetOwnerNoSee(bool)";
const PRIMITIVE: &str = "Engine.PrimitiveComponent";
const MESH: &str = "Engine.MeshComponent";
const PLAYER_CONTROLLER: &str = "Engine.PlayerController";

fn script(message: String) -> VmError {
    VmError::Script { function: WHAT.to_string(), message }
}

fn is_a(vm: &Vm, class: ClassId, path: &str) -> bool {
    vm.space.classes.by_path(path).is_some_and(|p| vm.space.classes.is_child_of(class, p))
}

fn path_of(vm: &Vm, class: ClassId) -> String {
    vm.space.classes.get_layout(class).map_or_else(|| format!("{class:?}"), |l| l.path().as_str().to_string())
}

/// `name` on `o`, declared as an object reference, else `VmError::Script` naming it.
fn object_field(vm: &Vm, o: ObjRef, name: &str) -> Result<Option<ObjRef>, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| script(format!("{} has no {name}", path_of(vm, class))))?;
    match vm.space.classes.decl(p).map(|d| &d.kind) {
        Some(PropKind::Object { .. }) => {}
        _ => return Err(script(format!("{}.{name} is not declared as an object reference", path_of(vm, class)))),
    }
    match vm.space.read(o, ow_object::ids::PropAddr::scalar(p))? {
        Value::Object(r) => Ok(*r),
        other => Err(script(format!("{}.{name} holds {}, not an object", path_of(vm, class), other.kind_name()))),
    }
}

/// The template's values are settled: it is not a store handle still unloaded (a load is a write).
fn settled(vm: &Vm, template: ObjRef) -> Result<(), VmError> {
    if vm.defs.is_unloaded(&vm.space, template) || vm.defs.is_failed(&vm.space, template) {
        return Err(NotYet::not_read(NOT_SETTLED, RECORD).into());
    }
    Ok(())
}

/// Hook 1 (module documentation) for a made component of `class`.
pub fn gear_likeness(vm: &Vm, class: ClassId) -> Result<(), VmError> {
    let Some(iface) = vm.space.classes.by_path(GEAR_LIKENESS) else {
        return Err(NotYet::not_read(GEAR_INTERFACE, RECORD).into());
    };
    let code = vm.defs.code();
    let Some(ik) = code.key_of(iface) else {
        return Err(NotYet::not_read(GEAR_INTERFACE, RECORD).into());
    };
    // The nearest class of the chain the code index holds (a hand-declared class has its package parent's tables).
    let mut cursor = Some(class);
    let mut key = None;
    while let Some(c) = cursor {
        if let Some(k) = code.key_of(c) {
            key = Some(k);
            break;
        }
        cursor = vm.space.classes.get_layout(c).and_then(|l| l.super_class());
    }
    let Some(ck) = key else {
        return Ok(());
    };
    let decoded = code.class(ik).body_decoded() && code.super_chain(ck).all(|c| code.class(c).body_decoded());
    let implements = if decoded {
        match code.implements_resolved(ck, ik) {
            Some(answer) => answer,
            None => return Err(NotYet::not_read(GEAR_UNRESOLVED, RECORD).into()),
        }
    } else {
        code.implements_structurally(ck, ik)
    };
    if implements {
        return Err(NotYet::layer_absent(GEAR_REFRESH, RENDER_LAYER, RECORD).into());
    }
    Ok(())
}

/// Hook 2 (module documentation) for the component `template` of `class`, made on `pawn`.
pub fn owner_no_see(vm: &Vm, pawn: ObjRef, template: ObjRef, class: ClassId) -> Result<(), VmError> {
    if !is_a(vm, class, PRIMITIVE) {
        return Ok(());
    }
    let Some(controller) = object_field(vm, pawn, "Controller")? else {
        return Ok(());
    };
    if !is_a(vm, vm.space.heap.get(controller)?.class, PLAYER_CONTROLLER) {
        return Ok(());
    }
    if !crate::world::is_local_player(vm, controller)? {
        return Ok(());
    }
    settled(vm, template)?;
    // The slots the attach produces: `Owner` the pawn, the two update flags cleared.
    let overrides =
        [("Owner", Value::Object(Some(pawn))), ("bNeedsReattach", Value::Bool(false)), ("bNeedsUpdateTransform", Value::Bool(false))];
    let slots = Slots { object: template, overrides: &overrides };
    if !component_setters::plan_flag(vm, slots, "bOwnerNoSee", true, OWNER_NO_SEE)? {
        return Ok(());
    }
    Err(NotYet::not_read(PAWN_TEST, RECORD).into())
}

/// Hook 3 (module documentation) for the component `template` of `class`, made on `pawn`.
pub fn light_environment(vm: &Vm, pawn: ObjRef, template: ObjRef, class: ClassId) -> Result<(), VmError> {
    if object_field(vm, pawn, "LightEnvironment")?.is_none() || !is_a(vm, class, MESH) {
        return Ok(());
    }
    settled(vm, template)?;
    let accepts = Slots::of(template).required(vm, "bAcceptsLights", WHAT)?;
    if !component_setters::bool_of(&accepts, "bAcceptsLights", WHAT)? {
        return Ok(());
    }
    Err(NotYet::not_read(ARRAY_CHECK, RECORD).into())
}

