//! Owner: lane IMPL-SETANIMTREETEMPLATE. AI-assisted (Claude Code). `SkeletalMeshComponent.SetAnimTreeTemplate` in
//! Off, coded from NATIVE_SET_ANIM_TREE_TEMPLATE.md alone (analyst lane ANALYST-SETANIMTREETEMPLATE, the note at
//! df863aff), with the hub's rulings: the review of #492, and the ruling on the note's section 5 (466350b). This lane did not
//! read the native. Every rule is the note's reading of the executable, **UNVERIFIED** in the running game.
//!
//! On the slice path (links 6 and 7, `make`), `WillowWeapon.AttachMeshToPickup` calls it with None on a mesh component
//! it made one statement earlier (the note's section 1). The port's `new` does not copy the template, so that
//! component holds its class defaults: `Owner` None and empty lists, so no event is sent.
//!
//! **The preflight** (the note's section 4, step 1), before any write, in the note's order:
//! 1. a template other than None: [`WITH_TEMPLATE`], `NotYet { NotRead }`;
//! 2. a non-empty `AnimTickArray`: [`TICK_NODES`], `NotYet { NotRead }`;
//! 3. a set `Animations`: [`CURRENT_TREE`], `NotYet { NotRead }`;
//! 4. the engine-wide registry walk (section 3, step 2), over the world's [`ComponentRegistry`]:
//!    - no registry layer on the world: [`REGISTRY`], `NotYet { LayerAbsent }`;
//!    - an entry naming this component: [`REGISTRY_ENTRY`], `NotYet { NotRead }` (the two handlers were not read);
//!    - otherwise the walk finds nothing (the hub's section 5 ruling: a port Off answer, UNVERIFIED).
//!
//! The declarations it reads and writes are checked before item 2 (a data error, `VmError::Script`, names the field).
//! The preflight's `NotYet`s are returned **unmarked**: the native has written nothing yet, and the caller's write
//! tracking adds "stopped after partial writes: " when earlier steps wrote (the `make` step's `new`).
//!
//! **The writes** (section 4, step 2), in order:
//! 1. `bAnimTreeInitialised` false;
//! 2. `Animations` None;
//! 3. `AnimTickArray`, `AnimAlwaysTickArray`, `SkelControlTickArray`, `ActiveMorphs` and `ActiveCurveMorphs` emptied;
//! 4. `MorphTargetIndexMap`: a no-op, because the port's layout gives a native map no script-visible value (it reads as
//!    None);
//! 5. then `AnimTreeTemplate` None.
//!
//! None of them loads a definition.
//!
//! **The owner's event** (section 4, step 3): only with `Owner` set, `AnimTreeUpdated(component)` is dispatched as
//! script on the owner's class and state. Any `NotYet` after the first write is terminal, marked once with the
//! partial-writes prefix, its kind and record kept; nothing is rolled back. An owner class without the event is
//! [`NO_EVENT`], a data error: `Owner` is declared as an `Actor` and `Actor` declares the event (the note's
//! section 2), so a class without it is an inconsistent class chain, not an unread rule (hub, review of #495).
//!
//! **Only in `ParityMode::Off`.** The C++ reference binds nothing here, so Reference keeps the VM's stub policy.

use std::rc::Rc;

use ow_core::{NotYet, ParityMode};
use ow_object::ids::ObjRef;
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

use crate::equip::{bad, object_of, required, write};

/// The record this native follows.
pub const RECORD: &str = "docs/verification/NATIVE_SET_ANIM_TREE_TEMPLATE.md";

/// The native key [`register`] binds in Off.
pub const SET_ANIM_TREE_TEMPLATE: &str = "SkeletalMeshComponent.SetAnimTreeTemplate(object)";

/// The named `NotYet`s.
pub const WITH_TEMPLATE: &str = "SetAnimTreeTemplate with a template: the tree copy and its initialisation";
pub const TICK_NODES: &str = "the tree teardown's per-node call";
pub const CURRENT_TREE: &str = "the tree teardown's calls on the current tree";
pub const REGISTRY: &str = "the engine's registry of entries that name a skeletal mesh component";
/// The absent layer the registry walk names, for a world without a [`ComponentRegistry`].
pub const REGISTRY_LAYER: &str = "an engine-wide registry of component entries (the port holds none)";
/// A registry entry naming the component: its handlers were not read.
pub const REGISTRY_ENTRY: &str = "the tree teardown's handlers for a registry entry that names this component";
/// An owner whose class chain declares no event: a data error (module documentation).
pub const NO_EVENT: &str = "SetAnimTreeTemplate's owner event on a class that declares no AnimTreeUpdated";

/// The event sent to the owner.
pub const EVENT: &str = "AnimTreeUpdated";

/// The lists the teardown empties, in the note's order.
const LISTS: [&str; 5] = ["AnimTickArray", "AnimAlwaysTickArray", "SkelControlTickArray", "ActiveMorphs", "ActiveCurveMorphs"];
const MORPH_MAP: &str = "MorphTargetIndexMap";

/// The engine-wide registry of component entries that the tree teardown walks (the note's section 3, step 2), as an
/// explicit layer on the world (a VM service). The hub's section 5 ruling: a world that holds it empty answers
/// "nothing" for the walk (a port Off answer, UNVERIFIED in game). A world without it is `NotYet { LayerAbsent }`.
///
/// **A lane that binds a native which adds entries to the game's registry must add them here**; the walk then reads
/// them, and any entry naming the component is `NotYet { NotRead }` until the handlers are read. No native the port
/// binds adds an entry today.
#[derive(Debug, Default)]
pub struct ComponentRegistry {
    /// The components the entries name, in the order they were added.
    pub entries: Vec<ObjRef>,
}

/// Adds an empty [`ComponentRegistry`] to `vm`'s world unless one is there (world set-up).
pub fn add_empty_registry(vm: &mut Vm) -> Result<(), VmError> {
    if vm.service::<ComponentRegistry>().is_none() {
        vm.add_service(ComponentRegistry::default())?;
    }
    Ok(())
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
    let want = |name: &str, ok: fn(&PropKind) -> bool, as_: &str| -> Result<(), VmError> {
        let kind = declared(vm, component, name)?;
        if ok(&kind) { Ok(()) } else { Err(bad(WHAT, format!("{name} is declared as {kind:?}, not {as_}"))) }
    };
    for name in ["AnimTreeTemplate", "Animations", "Owner"] {
        want(name, |k| matches!(k, PropKind::Object { .. }), "an object reference")?;
    }
    want("bAnimTreeInitialised", |k| matches!(k, PropKind::Bool), "a bool")?;
    for name in LISTS {
        want(name, |k| matches!(k, PropKind::Array(_)), "a dynamic array")?;
    }
    want(MORPH_MAP, |k| matches!(k, PropKind::Map { .. }), "a map")
}

/// The preflight (module documentation): nothing is written.
fn preflight(vm: &Vm, component: ObjRef, template: Option<ObjRef>) -> Result<(), VmError> {
    if template.is_some() {
        return Err(NotYet::not_read(WITH_TEMPLATE, RECORD).into());
    }
    schemas(vm, component)?;
    match required(vm, component, "AnimTickArray", WHAT)? {
        Value::Array(a) if a.is_empty() => {}
        Value::Array(_) => return Err(NotYet::not_read(TICK_NODES, RECORD).into()),
        other => return Err(bad(WHAT, format!("AnimTickArray holds {}, not an array", other.kind_name()))),
    }
    if object_of(&required(vm, component, "Animations", WHAT)?, WHAT)?.is_some() {
        return Err(NotYet::not_read(CURRENT_TREE, RECORD).into());
    }
    let Some(registry) = vm.service::<ComponentRegistry>() else {
        return Err(NotYet::layer_absent(REGISTRY, REGISTRY_LAYER, RECORD).into());
    };
    let named = registry
        .try_borrow()
        .map_err(|_| VmError::ServiceBorrowed("ComponentRegistry"))?
        .entries
        .contains(&component);
    if named {
        return Err(NotYet::not_read(REGISTRY_ENTRY, RECORD).into());
    }
    Ok(())
}

/// The writes, in the note's order (module documentation).
fn writes(vm: &mut Vm, component: ObjRef) -> Result<(), VmError> {
    write(vm, component, "bAnimTreeInitialised", Value::Bool(false), WHAT)?;
    write(vm, component, "Animations", Value::Object(None), WHAT)?;
    for name in LISTS {
        write(vm, component, name, Value::array(Vec::new()), WHAT)?;
    }
    // MORPH_MAP: no write. The port's layout gives a native map no script-visible value (module documentation).
    write(vm, component, "AnimTreeTemplate", Value::Object(None), WHAT)
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
        return Err(bad(WHAT, NO_EVENT));
    };
    vm.call(f, Some(owner), vec![Value::Object(Some(component))])?;
    Ok(())
}

/// `e` after the first write: a `NotYet` marked once, its kind and record kept; any other error unchanged.
fn after_writes(e: VmError) -> VmError {
    match e {
        VmError::NotYet(n) => n.after_partial_writes().into(),
        other => other,
    }
}

/// `SetAnimTreeTemplate(NewTemplate)` on `component` in Off (module documentation).
pub fn set_anim_tree_template(vm: &mut Vm, component: ObjRef, template: Option<ObjRef>) -> Result<(), VmError> {
    preflight(vm, component, template)?;
    writes(vm, component).map_err(after_writes)?;
    notify_owner(vm, component).map_err(after_writes)
}

fn native(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let component = c.this.ok_or_else(|| bad(WHAT, "called without a component"))?;
    let template = object_of(c.arg(0)?, WHAT)?;
    set_anim_tree_template(c.vm, component, template)?;
    Ok(Value::None)
}

/// Binds the native in `ParityMode::Off`; Reference binds nothing. The registry layer is the world set-up's to add
/// ([`add_empty_registry`]).
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    vm.register_native(SET_ANIM_TREE_TEMPLATE, Rc::new(native))
}

#[cfg(test)]
#[path = "anim_tree_tests.rs"]
mod tests;
