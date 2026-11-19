//! Owner: lane IMPL-SETANIMTREETEMPLATE. AI-assisted (Claude Code). `SkeletalMeshComponent.SetAnimTreeTemplate` in
//! Off, coded from NATIVE_SET_ANIM_TREE_TEMPLATE.md alone (analyst lane ANALYST-SETANIMTREETEMPLATE). This lane did not
//! read the native. Every rule is the note's reading of the executable, **UNVERIFIED** in the running game.
//!
//! On the slice path (links 6 and 7, `make`), `WillowWeapon.AttachMeshToPickup` calls it with None on a mesh component
//! it made one statement earlier (the note's section 1).
//!
//! **The preflight** (the note's section 4, step 1), before any write, in the note's order:
//! 1. a template other than None: [`WITH_TEMPLATE`], `NotYet { NotRead }`;
//! 2. a non-empty `AnimTickArray`: [`TICK_NODES`], `NotYet { NotRead }`;
//! 3. a set `Animations`: [`CURRENT_TREE`], `NotYet { NotRead }`;
//! 4. the engine-wide registry walk (section 3, step 2): the port holds no such registry, so [`REGISTRY`],
//!    `NotYet { LayerAbsent }` (the note's default answer). The note's section 5 leaves a "nothing" answer to the
//!    hub; [`Registry::Nothing`] exists for that ruling and for the tests, and [`register`] does not use it.
//!
//! The declarations it reads and writes are checked first (a data error, `VmError::Script`, names the field).
//!
//! **The writes** (section 3, step 4, then section 2, step 3), in order: `bAnimTreeInitialised` false; `Animations`
//! None; `AnimTickArray`, `AnimAlwaysTickArray` and `SkelControlTickArray` emptied; `MorphTargetIndexMap` emptied (a
//! native map, whose value the VM holds as None, as the C++ VM does); `ActiveMorphs` and `ActiveCurveMorphs` emptied;
//! then `AnimTreeTemplate` None. None of them loads a definition.
//!
//! **The owner's event** (section 2, step 4): with `Owner` set, `AnimTreeUpdated(component)` is dispatched as script on
//! the owner's class and state through the VM. A `NotYet` from it comes after the writes: terminal, marked once with
//! "stopped after partial writes: ", its kind and record kept, nothing rolled back. Other errors propagate unchanged.
//! An owner whose class declares no `AnimTreeUpdated` is [`NO_EVENT`], `NotYet { NotRead }`, after the writes too (the
//! note is silent: `Actor` declares it).
//!
//! **Only in `ParityMode::Off`.** The C++ reference binds nothing here, so Reference keeps the VM's stub policy.

use std::rc::Rc;

use ow_core::{NotYet, ParityMode};
use ow_object::ids::ObjRef;
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativeFn, Vm};
use ow_vm::error::VmError;

use crate::equip::{bad, object_of, required, write};

/// The record this native follows.
pub const RECORD: &str = "NATIVE_SET_ANIM_TREE_TEMPLATE.md";

/// The native key [`register`] binds in Off.
pub const SET_ANIM_TREE_TEMPLATE: &str = "SkeletalMeshComponent.SetAnimTreeTemplate(object)";

/// The preflight's named `NotYet`s (the note's section 4, step 1).
pub const WITH_TEMPLATE: &str = "SetAnimTreeTemplate with a template: the tree copy and its initialisation";
pub const TICK_NODES: &str = "the tree teardown's per-node call";
pub const CURRENT_TREE: &str = "the tree teardown's calls on the current tree";
pub const REGISTRY: &str = "the engine's registry of entries that name a skeletal mesh component";
/// The absent layer the registry walk names.
pub const REGISTRY_LAYER: &str = "the engine-wide registry the tree teardown walks";
/// An owner without the event (the note is silent).
pub const NO_EVENT: &str = "SetAnimTreeTemplate's owner event on a class that declares no AnimTreeUpdated";

/// The event sent to the owner.
pub const EVENT: &str = "AnimTreeUpdated";

/// The dynamic arrays the teardown empties, in the note's order around the map.
const TICK_LISTS: [&str; 3] = ["AnimTickArray", "AnimAlwaysTickArray", "SkelControlTickArray"];
const MORPH_MAP: &str = "MorphTargetIndexMap";
const MORPH_LISTS: [&str; 2] = ["ActiveMorphs", "ActiveCurveMorphs"];

/// The answer for the engine-wide registry walk (the note's section 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Registry {
    /// The note's default: `NotYet { LayerAbsent }`, [`REGISTRY`]. The only answer [`register`] binds.
    LayerAbsent,
    /// The walk finds nothing: the note's second option, left to the hub (not bound in Off).
    Nothing,
}

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

/// The declarations the native reads and writes, each of the expected kind, else a data error naming the field.
fn schemas(vm: &Vm, component: ObjRef) -> Result<(), VmError> {
    let want = |name: &str, ok: fn(&PropKind) -> bool| -> Result<(), VmError> {
        let kind = declared(vm, component, name)?;
        if ok(&kind) { Ok(()) } else { Err(bad(WHAT, format!("{name} is declared as {kind:?}"))) }
    };
    want("AnimTreeTemplate", |k| matches!(k, PropKind::Object { .. }))?;
    want("Animations", |k| matches!(k, PropKind::Object { .. }))?;
    want("bAnimTreeInitialised", |k| matches!(k, PropKind::Bool))?;
    want("Owner", |k| matches!(k, PropKind::Object { .. }))?;
    for name in TICK_LISTS.iter().chain(MORPH_LISTS.iter()) {
        want(name, |k| matches!(k, PropKind::Array(_)))?;
    }
    want(MORPH_MAP, |k| matches!(k, PropKind::Map { .. }))
}

fn array_len(vm: &Vm, o: ObjRef, name: &str) -> Result<usize, VmError> {
    match required(vm, o, name, WHAT)? {
        Value::Array(a) => Ok(a.len()),
        other => Err(bad(WHAT, format!("{name} holds {}, not an array", other.kind_name()))),
    }
}

/// The preflight (module documentation), before any write.
fn preflight(vm: &Vm, component: ObjRef, template: Option<ObjRef>, registry: Registry) -> Result<(), VmError> {
    if template.is_some() {
        return Err(NotYet::not_read(WITH_TEMPLATE, RECORD).into());
    }
    schemas(vm, component)?;
    if array_len(vm, component, "AnimTickArray")? != 0 {
        return Err(NotYet::not_read(TICK_NODES, RECORD).into());
    }
    if object_of(&required(vm, component, "Animations", WHAT)?, WHAT)?.is_some() {
        return Err(NotYet::not_read(CURRENT_TREE, RECORD).into());
    }
    match registry {
        Registry::LayerAbsent => Err(NotYet::layer_absent(REGISTRY, REGISTRY_LAYER, RECORD).into()),
        Registry::Nothing => Ok(()),
    }
}

/// The teardown's writes and the cleared template, in the note's order (module documentation).
fn writes(vm: &mut Vm, component: ObjRef) -> Result<(), VmError> {
    write(vm, component, "bAnimTreeInitialised", Value::Bool(false), WHAT)?;
    write(vm, component, "Animations", Value::Object(None), WHAT)?;
    for name in TICK_LISTS {
        write(vm, component, name, Value::array(Vec::new()), WHAT)?;
    }
    // A native map's value is None in this VM (ow_object::layout::PropKind::Map); emptied is that value.
    write(vm, component, MORPH_MAP, Value::None, WHAT)?;
    for name in MORPH_LISTS {
        write(vm, component, name, Value::array(Vec::new()), WHAT)?;
    }
    write(vm, component, "AnimTreeTemplate", Value::Object(None), WHAT)
}

/// `e` after the writes: a `NotYet` marked once, its kind and record kept; any other error unchanged.
fn after_writes(e: VmError) -> VmError {
    match e {
        VmError::NotYet(n) => n.after_partial_writes().into(),
        other => other,
    }
}

/// The owner's event (module documentation), after the writes.
fn notify_owner(vm: &mut Vm, component: ObjRef) -> Result<(), VmError> {
    let Some(owner) = object_of(&required(vm, component, "Owner", WHAT)?, WHAT)? else {
        return Ok(());
    };
    let (class, state) = {
        let x = vm.space.heap.get(owner)?;
        (x.class, x.state)
    };
    let Some(f) = vm.find_method(class, EVENT, state)? else {
        return Err(NotYet::not_read(NO_EVENT, RECORD).into());
    };
    vm.call(f, Some(owner), vec![Value::Object(Some(component))])?;
    Ok(())
}

/// `SetAnimTreeTemplate(NewTemplate)` on `component` with the registry answer `registry` (module documentation).
pub fn set_anim_tree_template(
    vm: &mut Vm,
    component: ObjRef,
    template: Option<ObjRef>,
    registry: Registry,
) -> Result<(), VmError> {
    preflight(vm, component, template, registry)?;
    writes(vm, component).map_err(after_writes)?;
    notify_owner(vm, component).map_err(after_writes)
}

fn native(c: &mut NativeCall<'_>, registry: Registry) -> Result<Value, VmError> {
    let component = c.this.ok_or_else(|| bad(WHAT, "called without a component"))?;
    let template = object_of(c.arg(0)?, WHAT)?;
    set_anim_tree_template(c.vm, component, template, registry)?;
    Ok(Value::None)
}

/// The native with `registry` as its registry answer. [`register`] binds [`Registry::LayerAbsent`] only.
pub fn native_with(registry: Registry) -> NativeFn {
    Rc::new(move |c: &mut NativeCall<'_>| native(c, registry))
}

/// Binds the native in `ParityMode::Off` with the note's default registry answer; Reference binds nothing.
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    vm.register_native(SET_ANIM_TREE_TEMPLATE, native_with(Registry::LayerAbsent))
}

#[cfg(test)]
#[path = "anim_tree_tests.rs"]
mod tests;
