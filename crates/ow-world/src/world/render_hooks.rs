//! Owner: lane IMPL-BODYCOMP-RENDER-HOOKS (decision 2026-11-19-268, number pending). The three rendering hooks the
//! pawn's `PostInitBodyComposition` runs on each made component, in `ParityMode::Off`, as the body composition's
//! preflight (crate::world::body_composition). The note is NATIVE_BODYCOMP_RENDER_HOOKS.md, section 4. Every rule is
//! **UNVERIFIED** in the running game. AI-assisted (Claude Code).
//!
//! Each hook's preflight is read only and comes before any write. Hook 2 writes in the write phase once its plan
//! passes (lane IMPL-DEFERRED-REATTACH-OWNER); hook 3 writes only past its unidentified array check, which refuses.
//!
//! 1. **The gear-likeness step** ([`gear_likeness`]), before the custom-head step. The made component's class is asked
//!    whether it implements `WillowGame.IGearLikenessConsumer`, read only, in `implements`' own order: only interface
//!    entries already resolved answer, and an entry not resolved yet is [`GEAR_UNRESOLVED`] (`NotRead`): resolving it
//!    would load. Two answers are **UNVERIFIED stand-ins** for data the game's lists do not cover (review of #657,
//!    LOW 1): a class the code index does not hold (hand-declared) answers through its nearest indexed parent, or no
//!    when it has none; and a class chain whose bodies did not all decode (synthetic packages) answers by the
//!    structural fallback, as `implements` does. The interface not defined at set-up is [`GEAR_INTERFACE`].
//!    No: nothing. Yes: the refresh, not read and render-side, is [`GEAR_REFRESH`] (`LayerAbsent`).
//! 2. **The owner-no-see setting** ([`owner_no_see`], section 6 of the note, lane IMPL-BODYCOMP-HOOK2-PAWN-TEST): the
//!    gate in the game's order. Not a `PrimitiveComponent`: nothing. The pawn test (`Pawn.IsPlayerPawn`, by the pawn's
//!    first native class): `WillowPlayerPawn` passes; `Pawn`, `GearboxPawn`, `WillowPawn` and `WillowAIPawn` pass when
//!    `Controller` is set and its `bIsPlayer` is set; any other class is undecided ([`PAWN_TEST`]). The pawn's
//!    `Controller` None or not an `Engine.PlayerController`: nothing. The controller test
//!    (`PlayerController.UsingFirstPersonCamera`, by its first native class): `WillowPlayerController` passes when
//!    `bBehindView` and `bViewingThirdPersonMenu` are both clear; any other class is undecided ([`CAMERA_TEST`]). The
//!    cast is judged by the class the handle carries, so a lazy `Controller` is judged too; one the store has not loaded
//!    is undecided ([`NOT_LOADED`]) only where a test must read it (a `Pawn`-body pawn test, or a
//!    `WillowPlayerController`'s controller test). Then the template must be settled
//!    ([`NOT_SETTLED`]), and `SetOwnerNoSee(true)`'s planner runs on the slots the attach produces: a flag already true
//!    is nothing. **A decided failing test answers nothing, and an undecided one is raised (`NotRead`, the first in
//!    gate order) only when every other part passes and the setter would write.** Where everything passes, the setter's
//!    deferred reattach applies the owner rule to the attach's `Owner`, the pawn (NATIVE_ATTACH_MESH_TO_PICKUP.md
//!    section 2.1.1, lane IMPL-DEFERRED-REATTACH-OWNER): a static or tick-disabled pawn is `REATTACH_OWNER`; an
//!    ordinary one passes, and hook 2's plan (`bOwnerNoSee`, then `bNeedsReattach`) is applied in the body
//!    composition's write phase, after the custom head and before hook 3.
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
/// Hook 2's pawn test on a pawn of an unlisted first native class (followed by " on <class>").
pub const PAWN_TEST: &str = "the owner-no-see gate's pawn test (IsPlayerPawn)";
/// Hook 2's controller test on a controller whose first native class is not `WillowPlayerController` (followed by
/// "<class>)"; `PlayerController` for `Engine.PlayerController` and `GearboxFramework.GearboxPlayerController`).
pub const CAMERA_TEST: &str = "the owner-no-see gate's camera test (UsingFirstPersonCamera of ";
/// Hook 2 reaching a `Controller` that is a store handle not loaded yet.
pub const NOT_LOADED: &str = "the rendering hooks: an object the store has not loaded (a load is a write)";
pub const ARRAY_CHECK: &str = "the light-environment step's array check (not identified)";

const WHAT: &str = "InitializeBodyComposition";
pub(crate) const OWNER_NO_SEE: &str = "PrimitiveComponent.SetOwnerNoSee(bool)";
const PRIMITIVE: &str = "Engine.PrimitiveComponent";
const MESH: &str = "Engine.MeshComponent";
const PLAYER_CONTROLLER: &str = "Engine.PlayerController";
const WILLOW_PLAYER_PAWN: &str = "WillowGame.WillowPlayerPawn";
/// The first native classes whose pawn test reads `Controller.bIsPlayer` (`Engine.Pawn`'s body, kept by these).
const PAWN_BODY: [&str; 4] = ["Engine.Pawn", "GearboxFramework.GearboxPawn", "WillowGame.WillowPawn", "WillowGame.WillowAIPawn"];
const WILLOW_PLAYER_CONTROLLER: &str = "WillowGame.WillowPlayerController";
/// The first native classes whose controller test is the engine's camera test (not read in detail).
const CAMERA_BODY: [&str; 2] = ["Engine.PlayerController", "GearboxFramework.GearboxPlayerController"];

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

/// The template's values are settled: it is not a store handle still unloaded (a load is a write). `is_unloaded`
/// also covers a handle whose load failed (`DefStore::is_unloaded` matches the failed entry too), so no separate
/// failed check is needed (review r1 of #665, LOW 2). That case is not reached from the body composition, whose walk
/// refuses a failed template first (its `failed_template` refusal); it is tested directly (review of #657, LOW 2).
fn settled(vm: &Vm, template: ObjRef) -> Result<(), VmError> {
    if vm.defs.is_unloaded(&vm.space, template) {
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
    // UNVERIFIED stand-in (module documentation): the nearest class of the chain the code index holds (a hand-declared
    // class has its package parent's tables), or no when there is none.
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
        // UNVERIFIED stand-in (module documentation): the structural fallback for bodies that did not decode.
        code.implements_structurally(ck, ik)
    };
    if implements {
        return Err(NotYet::layer_absent(GEAR_REFRESH, RENDER_LAYER, RECORD).into());
    }
    Ok(())
}

/// The path of `class`'s first native class, from `class` up: the fitted native bit of the class body where the code
/// index holds the class (a body that did not decode counts as native), else the class table's mark, as
/// `set_physics` walks it. `None` when no class of the chain is native. Reads only: no class code is loaded.
fn first_native(vm: &Vm, class: ClassId) -> Option<String> {
    let code = vm.defs.code();
    let mut cursor = Some(class);
    while let Some(c) = cursor {
        let layout = vm.space.classes.get_layout(c)?;
        let native = match code.key_of(c) {
            Some(k) => code.class(k).body.as_ref().map_or(true, |b| b.is_native()),
            None => vm.space.classes.is_native(c),
        };
        if native {
            return Some(layout.path().as_str().to_string());
        }
        cursor = layout.super_class();
    }
    None
}

fn is_one_of(path: &str, list: &[&str]) -> bool {
    list.iter().any(|p| path.eq_ignore_ascii_case(p))
}

/// `name` on `o`, declared as a bool, else `VmError::Script` naming it.
fn bool_field(vm: &Vm, o: ObjRef, name: &str) -> Result<bool, VmError> {
    component_setters::schemas(vm, o, &[(name, component_setters::Want::Bool)], WHAT)?;
    component_setters::bool_of(&Slots::of(o).required(vm, name, WHAT)?, name, WHAT)
}

/// One of hook 2's tests: decided either way, or undecided with the refusal it raises when everything else passes.
enum Test {
    Pass,
    Fail,
    Undecided(VmError),
}

/// The pawn's `Controller`: None, a store handle not loaded yet, or a resident object.
enum Controller {
    None,
    NotLoaded,
    Resident(ObjRef),
}

fn controller(vm: &Vm, pawn: ObjRef) -> Result<Controller, VmError> {
    Ok(match object_field(vm, pawn, "Controller")? {
        None => Controller::None,
        Some(c) if vm.defs.is_unloaded(&vm.space, c) => Controller::NotLoaded,
        Some(c) => Controller::Resident(c),
    })
}

fn not_loaded() -> VmError {
    NotYet::not_read(NOT_LOADED, RECORD).into()
}

/// The pawn test, `Pawn.IsPlayerPawn` (module documentation).
fn pawn_test(vm: &Vm, pawn: ObjRef) -> Result<Test, VmError> {
    let class = vm.space.heap.get(pawn)?.class;
    let native = first_native(vm, class);
    match native.as_deref() {
        Some(p) if p.eq_ignore_ascii_case(WILLOW_PLAYER_PAWN) => Ok(Test::Pass),
        Some(p) if is_one_of(p, &PAWN_BODY) => Ok(match controller(vm, pawn)? {
            Controller::None => Test::Fail,
            Controller::NotLoaded => Test::Undecided(not_loaded()),
            Controller::Resident(c) if bool_field(vm, c, "bIsPlayer")? => Test::Pass,
            Controller::Resident(_) => Test::Fail,
        }),
        other => {
            let named = other.map_or_else(|| path_of(vm, class), str::to_string);
            Ok(Test::Undecided(NotYet::not_read(format!("{PAWN_TEST} on {named}"), RECORD).into()))
        }
    }
}

/// The controller test, `PlayerController.UsingFirstPersonCamera` (module documentation), on a `PlayerController`
/// (resident or a lazy handle: only the `WillowPlayerController` rule reads it).
fn camera_test(vm: &Vm, pc: ObjRef) -> Result<Test, VmError> {
    let class = vm.space.heap.get(pc)?.class;
    let native = first_native(vm, class);
    let named = match native.as_deref() {
        Some(p) if p.eq_ignore_ascii_case(WILLOW_PLAYER_CONTROLLER) => {
            if vm.defs.is_unloaded(&vm.space, pc) {
                return Ok(Test::Undecided(not_loaded()));
            }
            let behind = bool_field(vm, pc, "bBehindView")?;
            let menu = bool_field(vm, pc, "bViewingThirdPersonMenu")?;
            return Ok(if behind || menu { Test::Fail } else { Test::Pass });
        }
        Some(p) if is_one_of(p, &CAMERA_BODY) => "PlayerController".to_string(),
        Some(p) => p.to_string(),
        None => path_of(vm, class),
    };
    Ok(Test::Undecided(NotYet::not_read(format!("{CAMERA_TEST}{named})"), RECORD).into()))
}

/// Hook 2 (module documentation) for the component `template` of `class`, made on `pawn`: `true` when its plan is to
/// be applied in the body composition's write phase (`bOwnerNoSee` = true, then `bNeedsReattach` = true on the made
/// component), `false` for nothing.
pub fn owner_no_see(vm: &Vm, pawn: ObjRef, template: ObjRef, class: ClassId) -> Result<bool, VmError> {
    if !is_a(vm, class, PRIMITIVE) {
        return Ok(false);
    }
    // The first undecided test, raised only once every other part has passed and the setter would write.
    let mut undecided = None;
    match pawn_test(vm, pawn)? {
        Test::Fail => return Ok(false),
        Test::Undecided(e) => undecided = Some(e),
        Test::Pass => {}
    }
    // The `PlayerController` cast, judged by the class the handle carries, so a lazy handle is judged too.
    let Some(c) = object_field(vm, pawn, "Controller")? else {
        return Ok(false);
    };
    if !is_a(vm, vm.space.heap.get(c)?.class, PLAYER_CONTROLLER) {
        return Ok(false);
    }
    match camera_test(vm, c)? {
        Test::Fail => return Ok(false),
        Test::Undecided(e) => {
            undecided.get_or_insert(e);
        }
        Test::Pass => {}
    }
    settled(vm, template)?;
    // The slots the attach produces: `Owner` the pawn, the two update flags cleared.
    let overrides =
        [("Owner", Value::Object(Some(pawn))), ("bNeedsReattach", Value::Bool(false)), ("bNeedsUpdateTransform", Value::Bool(false))];
    let slots = Slots { object: template, overrides: &overrides };
    if !component_setters::plan_flag(vm, slots, "bOwnerNoSee", true, OWNER_NO_SEE)? {
        return Ok(false);
    }
    if let Some(e) = undecided {
        return Err(e);
    }
    // The setter's deferred reattach, with the attach's `Owner` (the pawn): the owner rule (NATIVE_ATTACH_MESH_TO_PICKUP.md
    // section 2.1.1, lane IMPL-DEFERRED-REATTACH-OWNER). An ordinary pawn passes, and the plan is applied in the write
    // phase; the unconditional fallback refusal is gone.
    component_setters::reattach_check(vm, slots, OWNER_NO_SEE)?;
    Ok(true)
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

