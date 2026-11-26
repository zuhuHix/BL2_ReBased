//! Owner: lane IMPL-BONE-TRANSFORM-LAYER. The script natives `SkeletalMeshComponent.ForceSkelUpdate()` and
//! `SkeletalMeshComponent.UpdateAnimations()` in `ParityMode::Off`, coded from docs/verification/
//! NATIVE_BONE_TRANSFORM_LAYER.md section 6.4 alone (#646 at 94ab4339; clean room: lane ANALYST-BONE-TRANSFORM-LAYER
//! read the native, this lane did not). The layer's read and pose update (sections 6.1 to 6.3) are in
//! `world::skeletal`. AI-assisted (Claude Code). **Every rule here is UNVERIFIED in the running game.** Reference
//! binds nothing.
//!
//! Both are bound keep-existing per key in Off ([`register`]). Each checks, in order, and reads only (no write, no
//! load, no draw, no name interned):
//! 1. its receiver set (else `VmError::Script`) and resident: an unloaded or failed handle, or a stand-in with no slots,
//!    is `NotYet { NotRead }` (the port's check, as `SetSkeletalMesh`'s);
//! 2. the field it reads declared with its kind, else `VmError::Script` naming it: `bAttached` a bool, `Animations` an
//!    object;
//! 3. `ForceSkelUpdate()`: `bAttached` clear is nothing; set is `NotYet { NotRead }`, [`FORCE_ATTACHED`];
//!    `UpdateAnimations()`: `Animations` None is nothing; set runs the per-node step of
//!    NATIVE_UPDATEANIMATIONS_TREE_TICKS.md ([`super::tree_ticks`], lane IMPL-UPDATEANIMATIONS-TREE-TICKS): its
//!    preflight first, then the tag and the sequence nodes' writes. It replaces the `NotYet { NotPorted }`
//!    [`TREE_TICKS`], which no call raises any more (the constant stays for the records that name it).

use std::rc::Rc;

use ow_core::NotYet;
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

use crate::world::skeletal::{self, Kind};

/// The record of every refusal here.
pub const RECORD: &str = "docs/verification/NATIVE_BONE_TRANSFORM_LAYER.md";
/// `ForceSkelUpdate`'s refusal on an attached component.
pub const FORCE_ATTACHED: &str = "ForceSkelUpdate on an attached component (the tree tick, the LOD choice, the pose update and the transform update)";
/// `UpdateAnimations`' former refusal with a tree (replaced by `super::tree_ticks`).
pub const TREE_TICKS: &str = "UpdateAnimations: the tree nodes' ticks";
/// The natives' keys.
pub const FORCE_SKEL_UPDATE: &str = "SkeletalMeshComponent.ForceSkelUpdate()";
pub const UPDATE_ANIMATIONS: &str = "SkeletalMeshComponent.UpdateAnimations()";

fn script(function: &str, message: String) -> VmError {
    VmError::Script {
        function: function.to_string(),
        message: format!("{function}: {message}"),
    }
}

/// Rule 1: the receiver set and resident.
fn receiver(c: &NativeCall<'_>, function: &str) -> Result<ObjRef, VmError> {
    let o = c
        .this
        .ok_or_else(|| script(function, "called with no component".to_string()))?;
    if !skeletal::resident(c.vm, o)? {
        return Err(NotYet::not_read(
            format!("{function}: the component is an unloaded handle (a load is a write)"),
            RECORD,
        )
        .into());
    }
    Ok(o)
}

fn force_skel_update(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let o = receiver(c, "ForceSkelUpdate")?;
    if skeletal::read_kind(c.vm, o, "bAttached", Kind::Bool, "ForceSkelUpdate")?
        == Value::Bool(true)
    {
        return Err(NotYet::not_read(FORCE_ATTACHED, RECORD).into());
    }
    Ok(Value::None)
}

fn update_animations(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let o = receiver(c, "UpdateAnimations")?;
    if skeletal::read_kind(c.vm, o, "Animations", Kind::Object, "UpdateAnimations")?
        == Value::Object(None)
    {
        return Ok(Value::None);
    }
    if let Some(plan) = super::tree_ticks::plan(c.vm, o)? {
        super::tree_ticks::commit(c.vm, o, plan)?;
    }
    Ok(Value::None)
}

/// Binds both natives, keep-existing per key, in Off only; Reference binds nothing.
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ow_core::ParityMode::Off {
        return Ok(());
    }
    if !vm.has_native(FORCE_SKEL_UPDATE) {
        vm.register_native(FORCE_SKEL_UPDATE, Rc::new(force_skel_update))?;
    }
    if !vm.has_native(UPDATE_ANIMATIONS) {
        vm.register_native(UPDATE_ANIMATIONS, Rc::new(update_animations))?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "bone_transform_tests.rs"]
mod tests;
