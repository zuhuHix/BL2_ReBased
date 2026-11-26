//! Owner: lane SOCKETS-PUBLISH-AND-PHYSX-LEVEL (queue 0af). `maya:8` after `InitializeRBPhysics`, in `ParityMode::Off`:
//! `Engine.GetPhysXLevel` (2.A), `SkeletalMeshComponent.SetHasPhysicsAssetInstance` (2.B) and
//! `NxForceFieldComponent.DoInitRBPhys` (2.C), with the slice worlds' harness setting for the level. AI-assisted
//! (Claude Code).
//!
//! **Clean room.** Implemented from docs/verification/NATIVE_GETPHYSXLEVEL_AND_AFTER.md alone (lane
//! ANALYST-GETPHYSXLEVEL-AND-AFTER, #767, on main); this lane did not read the executable. **Every rule is UNVERIFIED
//! in the running game.** Reference binds nothing (the stub policy answers).
//!
//! **The harness setting** ([`set_physx_level`]): at world set-up the slice worlds' engine object (the
//! [`EngineObject`] service) gets `PhysXLevel` from a harness setting, [`HARNESS_PHYSX_LEVEL`] = 0, the install's
//! shipped default. Only the game's clamped range, 0 to 2, is accepted, and `PhysXLevel` must be declared as an int on
//! the engine object's class; both are checked before the write. One harness line names the value. The game's value
//! is the player's own `[SystemSettings] PhysXLevel`, not observed.
//!
//! **2.A `GetPhysXLevel()`** reads only: the engine object (else [`NO_ENGINE`], `LayerAbsent`), its `PhysXLevel`
//! declared as an int (else `VmError::Script`), returned as stored. At 0, the installed
//! `SetupSkeletalMeshPhysicsForPhysXEffects` returns after it and writes nothing (its only test is "greater than 0").
//!
//! **2.B `SetHasPhysicsAssetInstance(bHasInstance, bUseCurrentPosition)`**, reached only above level 0: the receiver
//! declares `bHasPhysicsAssetInstance` and `bSkelCompFixed` as bools (else `VmError::Script`) and is resident (else
//! [`NOT_LOADED`], `NotRead`). A request that matches the flag writes nothing (the game's rewrite of the same value is
//! not modelled); false to true is [`ASSET_INSTANCE`] (`LayerAbsent`, the physics layer) before any write; true to
//! false is [`TEAR_DOWN`] (`NotRead`).
//!
//! **2.C `DoInitRBPhys()`**, reached only above level 0: [`FORCE_FIELD`] (`LayerAbsent`, the physics layer), nothing
//! written.

use std::rc::Rc;

use ow_core::NotYet;
use ow_object::ids::ObjRef;
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativeFn, Vm};
use ow_vm::error::VmError;

use super::component_setters::{Want, bad, bool_of, schemas};
use super::online::EngineObject;
use super::player_pawn_tail::RIGID_LAYER;

/// The record of the rules.
pub const RECORD: &str = "docs/verification/NATIVE_GETPHYSXLEVEL_AND_AFTER.md";
/// The native keys.
pub const GET_PHYSX_LEVEL: &str = "Engine.GetPhysXLevel()";
pub const SET_HAS_PHYSICS_ASSET_INSTANCE: &str = "SkeletalMeshComponent.SetHasPhysicsAssetInstance(bool,bool)";
pub const DO_INIT_RB_PHYS: &str = "NxForceFieldComponent.DoInitRBPhys()";
/// The slice worlds' harness setting: the shipped default.
pub const HARNESS_PHYSX_LEVEL: i32 = 0;
/// The refusals.
pub const NO_ENGINE: &str = "Engine.GetPhysXLevel: the world has no engine object";
pub const ENGINE_LAYER: &str = "the engine object (the port's world has none)";
pub const ASSET_INSTANCE: &str = "SkeletalMeshComponent.SetHasPhysicsAssetInstance: the physics-asset instance";
pub const TEAR_DOWN: &str = "SkeletalMeshComponent.SetHasPhysicsAssetInstance: the physics-asset instance's tear-down";
pub const FORCE_FIELD: &str = "NxForceFieldComponent.DoInitRBPhys: the force field";
pub const NOT_LOADED: &str =
    "SkeletalMeshComponent.SetHasPhysicsAssetInstance: a component the store has not loaded (a load is a write)";

const WHAT: &str = "Engine.GetPhysXLevel";
const SET_WHAT: &str = "SkeletalMeshComponent.SetHasPhysicsAssetInstance";

/// The world's engine object, if any.
fn engine(vm: &Vm) -> Result<Option<ObjRef>, VmError> {
    match vm.service::<EngineObject>() {
        Some(e) => Ok(Some(e.try_borrow().map_err(|_| VmError::ServiceBorrowed("EngineObject"))?.0)),
        None => Ok(None),
    }
}

/// `engine`'s `PhysXLevel` property, declared as an int, else a data error.
fn level_prop(vm: &Vm, engine: ObjRef, what: &str) -> Result<ow_object::ids::PropId, VmError> {
    let class = vm.space.heap.get(engine)?.class;
    let p = vm.space.prop(class, "PhysXLevel").ok_or_else(|| bad(what, "the engine object declares no PhysXLevel"))?;
    match vm.space.classes.decl(p).map(|d| &d.kind) {
        Some(PropKind::Int) => Ok(p),
        Some(k) => Err(bad(what, format!("PhysXLevel is declared as {}, not an int", k.kind_name()))),
        None => Err(bad(what, "PhysXLevel has no declaration")),
    }
}

/// The harness setting (module documentation): writes `level` into the engine object's `PhysXLevel` and logs it.
/// Off only. A world with no engine object sets nothing and answers `Ok(false)`; a level outside 0 to 2, or an
/// undeclared field, is refused before the write.
pub fn set_physx_level(vm: &mut Vm, level: i32) -> Result<bool, VmError> {
    let refused = |reason: &'static str| VmError::Registration { key: "PhysXLevel".to_string(), reason };
    if vm.parity == ow_core::ParityMode::Reference {
        return Err(refused("the PhysX level setting is an Off answer; Reference keeps none"));
    }
    if !(0..=2).contains(&level) {
        return Err(refused("the PhysX level must be 0, 1 or 2 (the game's clamped range)"));
    }
    let Some(engine) = engine(vm)? else {
        return Ok(false);
    };
    let p = level_prop(vm, engine, "the PhysX level setting")?;
    vm.space.write(engine, ow_object::ids::PropAddr::scalar(p), Value::Int(level))?;
    vm.log.push(format!(
        "harness input: the engine object's PhysXLevel = {level} (a harness setting; UNVERIFIED: the player's setting; \
         shipped default 0)"
    ));
    Ok(true)
}

/// 2.A (module documentation). Reads only.
pub fn get_physx_level(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let vm: &Vm = c.vm;
    let Some(engine) = engine(vm)? else {
        return Err(NotYet::layer_absent(NO_ENGINE, ENGINE_LAYER, RECORD).into());
    };
    let p = level_prop(vm, engine, WHAT)?;
    match vm.space.read(engine, ow_object::ids::PropAddr::scalar(p))? {
        Value::Int(i) => Ok(Value::Int(*i)),
        other => Err(bad(WHAT, format!("PhysXLevel holds {}, not an int", other.kind_name()))),
    }
}

/// 2.B (module documentation). Writes nothing.
pub fn set_has_physics_asset_instance(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let component = c.this.ok_or_else(|| bad(SET_WHAT, "called without a component"))?;
    let want = bool_of(c.arg(0)?, "bHasInstance", SET_WHAT)?;
    let vm: &Vm = c.vm;
    schemas(vm, component, &[("bHasPhysicsAssetInstance", Want::Bool), ("bSkelCompFixed", Want::Bool)], SET_WHAT)?;
    if crate::touch_trace::is_unloaded(vm, component) || vm.defs.is_failed(&vm.space, component) {
        return Err(NotYet::not_read(NOT_LOADED, RECORD).into());
    }
    let current = super::component_setters::Slots::of(component).required(vm, "bHasPhysicsAssetInstance", SET_WHAT)?;
    let current = bool_of(&current, "bHasPhysicsAssetInstance", SET_WHAT)?;
    if current == want {
        Ok(Value::None)
    } else if want {
        Err(NotYet::layer_absent(ASSET_INSTANCE, RIGID_LAYER, RECORD).into())
    } else {
        Err(NotYet::not_read(TEAR_DOWN, RECORD).into())
    }
}

/// 2.C (module documentation). Writes nothing.
pub fn do_init_rb_phys(_c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    Err(NotYet::layer_absent(FORCE_FIELD, RIGID_LAYER, RECORD).into())
}

/// Binds the three keys (Off only; the caller checks the mode). No other pack binds them (checked by grep before this
/// lane; the all-packs wiring test checks the rest), so a duplicate is a registration error.
pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    let natives: [(&str, NativeFn); 3] = [
        (GET_PHYSX_LEVEL, Rc::new(get_physx_level)),
        (SET_HAS_PHYSICS_ASSET_INSTANCE, Rc::new(set_has_physics_asset_instance)),
        (DO_INIT_RB_PHYS, Rc::new(do_init_rb_phys)),
    ];
    for (key, f) in natives {
        vm.register_native(key, f)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "physx_level_tests.rs"]
mod tests;
