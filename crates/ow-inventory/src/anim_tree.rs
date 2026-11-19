//! Owner: lane IMPL-SETANIMTREETEMPLATE. AI-assisted (Claude Code). `SkeletalMeshComponent.SetAnimTreeTemplate` in
//! Off, coded from NATIVE_SET_ANIM_TREE_TEMPLATE.md alone (analyst lane ANALYST-SETANIMTREETEMPLATE), with the hub's
//! rulings from the review of #492 (the note at df863aff). This lane did not read the native. Every rule is the note's reading of the
//! executable, **UNVERIFIED** in the running game.
//!
//! On the slice path (links 6 and 7, `make`), `WillowWeapon.AttachMeshToPickup` calls it with None on a mesh component
//! it made one statement earlier (the note's section 1). The port's `new` does not copy the template, so that
//! component holds its class defaults (`Owner` None, empty lists).
//!
//! **What is bound** (the note's section 4, step 1), in the note's order, before any write of its own:
//! 1. a template other than None: [`WITH_TEMPLATE`], `NotYet { NotRead }`;
//! 2. a non-empty `AnimTickArray`: [`TICK_NODES`], `NotYet { NotRead }`;
//! 3. a set `Animations`: [`CURRENT_TREE`], `NotYet { NotRead }`;
//! 4. the engine-wide registry walk (section 3, step 2): the port holds no such registry, so [`REGISTRY`],
//!    `NotYet { LayerAbsent }`. In Off the native always ends here.
//!
//! The two fields it reads (`AnimTickArray`, `Animations`) are checked against their declarations first. A mismatch is
//! a data error (`VmError::Script`) that names the field.
//!
//! The `NotYet` is returned **unmarked**. The caller's write tracking adds "stopped after partial writes: " when
//! earlier steps already wrote (the `make` step's `new`). The teardown's writes and the owner's `AnimTreeUpdated` event
//! (the note's sections 2 and 3) are **not coded**: they stay unreachable until the hub rules on the note's section 5.
//!
//! **Only in `ParityMode::Off`.** The C++ reference binds nothing here, so Reference keeps the VM's stub policy.

use std::rc::Rc;

use ow_core::{NotYet, ParityMode};
use ow_object::ids::ObjRef;
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

use crate::equip::{bad, object_of, required};

/// The record this native follows.
pub const RECORD: &str = "docs/verification/NATIVE_SET_ANIM_TREE_TEMPLATE.md";

/// The native key [`register`] binds in Off.
pub const SET_ANIM_TREE_TEMPLATE: &str = "SkeletalMeshComponent.SetAnimTreeTemplate(object)";

/// The named `NotYet`s (the note's section 4, step 1).
pub const WITH_TEMPLATE: &str = "SetAnimTreeTemplate with a template: the tree copy and its initialisation";
pub const TICK_NODES: &str = "the tree teardown's per-node call";
pub const CURRENT_TREE: &str = "the tree teardown's calls on the current tree";
pub const REGISTRY: &str = "the engine's registry of entries that name a skeletal mesh component";
/// The absent layer the registry walk names.
pub const REGISTRY_LAYER: &str = "an engine-wide registry of component entries (the port holds none)";

const WHAT: &str = "SkeletalMeshComponent.SetAnimTreeTemplate";

fn declared(vm: &Vm, o: ObjRef, name: &str) -> Result<PropKind, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm
        .space
        .prop(class, name)
        .ok_or_else(|| bad(WHAT, format!("{} declares no {name}", ow_attr::eval::class_path(&vm.space, o))))?;
    vm.space
        .classes
        .decl(p)
        .map(|d| d.kind.clone())
        .ok_or_else(|| bad(WHAT, format!("{name} has no declaration")))
}

/// `SetAnimTreeTemplate(NewTemplate)` on `component` in Off (module documentation). It writes nothing, and it always
/// ends in a `NotYet` or a data error.
pub fn set_anim_tree_template(vm: &Vm, component: ObjRef, template: Option<ObjRef>) -> Result<(), VmError> {
    if template.is_some() {
        return Err(NotYet::not_read(WITH_TEMPLATE, RECORD).into());
    }
    let ticks = declared(vm, component, "AnimTickArray")?;
    if !matches!(ticks, PropKind::Array(_)) {
        return Err(bad(WHAT, format!("AnimTickArray is declared as {ticks:?}, not a dynamic array")));
    }
    let tree = declared(vm, component, "Animations")?;
    if !matches!(tree, PropKind::Object { .. }) {
        return Err(bad(WHAT, format!("Animations is declared as {tree:?}, not an object reference")));
    }
    match required(vm, component, "AnimTickArray", WHAT)? {
        Value::Array(a) if a.is_empty() => {}
        Value::Array(_) => return Err(NotYet::not_read(TICK_NODES, RECORD).into()),
        other => return Err(bad(WHAT, format!("AnimTickArray holds {}, not an array", other.kind_name()))),
    }
    if object_of(&required(vm, component, "Animations", WHAT)?, WHAT)?.is_some() {
        return Err(NotYet::not_read(CURRENT_TREE, RECORD).into());
    }
    Err(NotYet::layer_absent(REGISTRY, REGISTRY_LAYER, RECORD).into())
}

fn native(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let component = c.this.ok_or_else(|| bad(WHAT, "called without a component"))?;
    let template = object_of(c.arg(0)?, WHAT)?;
    set_anim_tree_template(c.vm, component, template)?;
    Ok(Value::None)
}

/// Binds the native in `ParityMode::Off`; Reference binds nothing.
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    vm.register_native(SET_ANIM_TREE_TEMPLATE, Rc::new(native))
}

#[cfg(test)]
#[path = "anim_tree_tests.rs"]
mod tests;
