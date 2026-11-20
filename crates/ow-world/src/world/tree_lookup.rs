//! Owner: lane IMPL-TREE-INIT-OWNER-EVENT (decision 2026-11-19-NNN, number pending). The script natives
//! `SkeletalMeshComponent.FindAnimNode(name)` and `FindSkelControl(name)` in `ParityMode::Off`, read-only, on the
//! component's own (instanced) tree. AI-assisted (Claude Code).
//!
//! **Clean room.** From `docs/verification/NATIVE_ANIM_NODE_LOOKUP.md` section 1 (#543), scoped as
//! NATIVE_TREE_INIT_OWNER_EVENT.md (#612) section 3 binds it; this lane did not read the executable. **Every rule is
//! UNVERIFIED in the running game.** Reference binds neither (the stub policy decides there). The binding is global in
//! Off (every script caller gets it), so it answers only on the path #543 read:
//!
//! - **The component's own tree:** `Animations` is set, its class derives from `Engine.AnimTree`, and its heap outer is
//!   the component (the copy's outer, #561).
//! - **Animation node by name:** None when `Animations` is None. With the own tree and a non-empty `AnimTickArray`
//!   whose first entry is `Animations`, the first node of `AnimTickArray`, in order, whose `NodeName` equals the name
//!   as a whole name (name and number; a None name matches a None `NodeName`). Anything else is the gather path, not
//!   read: [`GATHER`], `NotYet { NotRead }`.
//! - **Skeletal control by name:** None at once for a None name; None when there is no tree or the tree is not an
//!   `AnimTree`; a tree that is not the component's own (its heap outer is not the component) is [`FOREIGN`],
//!   `NotYet { NotRead }` (review r1 of #631, item 2: a shared template's or another component's controls are not
//!   what #543 read). Otherwise `SkelControlLists` in array order, each list's `ControlHead`, then the `NextControl` chain;
//!   the first control whose `ControlName` equals the name (a whole name). A chain that loops is a `VmError::Script`.
//! - An unloaded or failed handle met on the way is #543's refusal ([`crate::world::anim_node_lookup::refusal`]),
//!   raised with nothing loaded. Nothing is written.
//!
//! A tick-list entry whose heap outer is not the component is the gather path too (review r1 of #631, item 2).
//!
//! Both are bound under their package-qualified identity, `Engine` plus the key ([`ow_vm::api::Vm::register_native_in`],
//! review r1 of #631, item 4): a binding of the same key in another package does not stand for Engine's. A key already
//! bound for Engine, or VM-wide, keeps its binding (keep-existing).

use std::rc::Rc;

use ow_core::NotYet;
use ow_object::ids::{NameId, ObjRef};
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

use crate::world::anim_tree_copy::{elements, object_of, read};
use crate::world::body_composition::field;

/// The binding's note (its rules from NATIVE_ANIM_NODE_LOOKUP.md section 1).
pub const RECORD: &str = "docs/verification/NATIVE_TREE_INIT_OWNER_EVENT.md";
pub const FIND_ANIM_NODE: &str = "SkeletalMeshComponent.FindAnimNode(name)";
pub const FIND_SKEL_CONTROL: &str = "SkeletalMeshComponent.FindSkelControl(name)";
/// The node lookup's other path (the tree's gather step), not read.
pub const GATHER: &str = "the animation-node lookup's gather path (no tick list, or not the component's own tree)";
/// The control lookup on a tree that is not the component's own (review r1 of #631, item 2).
pub const FOREIGN: &str = "the skeletal-control lookup on a tree that is not the component's own (not read)";
/// The package both natives are declared in (their package-qualified identity).
pub const PACKAGE: &str = "Engine";

fn script(message: String) -> VmError {
    VmError::Script { function: "SkeletalMeshComponent.FindAnimNode / FindSkelControl".to_string(), message }
}

fn readable(vm: &Vm, o: ObjRef) -> Result<ObjRef, VmError> {
    if vm.defs.is_unloaded(&vm.space, o) || vm.defs.is_failed(&vm.space, o) {
        return Err(crate::world::anim_node_lookup::refusal());
    }
    Ok(o)
}

fn name_in(vm: &Vm, o: ObjRef, prop: &str) -> Result<NameId, VmError> {
    match read(vm, o, prop)? {
        Value::Name(n) => Ok(n),
        other => Err(script(format!("{prop} holds {}, not a name", other.kind_name()))),
    }
}

fn tree_of(vm: &Vm, component: ObjRef) -> Result<Option<ObjRef>, VmError> {
    match object_of(&read(vm, component, "Animations")?) {
        Some(t) => readable(vm, t).map(Some),
        None => Ok(None),
    }
}

fn is_tree(vm: &Vm, o: ObjRef) -> Result<bool, VmError> {
    let class = vm.space.heap.get(o)?.class;
    Ok(vm
        .space
        .classes
        .by_path(crate::world::anim_tree_copy::TREE)
        .is_some_and(|t| vm.space.classes.is_child_of(class, t)))
}

/// `FindAnimNode(name)` on `component` (module documentation).
pub fn find_anim_node(vm: &Vm, component: ObjRef, name: NameId) -> Result<Option<ObjRef>, VmError> {
    let Some(root) = tree_of(vm, component)? else {
        return Ok(None);
    };
    let own = is_tree(vm, root)? && vm.space.heap.get(root)?.outer == Some(component);
    let ticks = elements(&read(vm, component, "AnimTickArray")?, "AnimTickArray")?;
    if !own || ticks.first().and_then(object_of) != Some(root) {
        return Err(NotYet::not_read(GATHER, RECORD).into());
    }
    for t in &ticks {
        let Some(node) = object_of(t) else { continue };
        if vm.space.heap.get(readable(vm, node)?)?.outer != Some(component) {
            return Err(NotYet::not_read(GATHER, RECORD).into());
        }
        if name_in(vm, readable(vm, node)?, "NodeName")? == name {
            return Ok(Some(node));
        }
    }
    Ok(None)
}

/// `FindSkelControl(name)` on `component` (module documentation).
pub fn find_skel_control(vm: &Vm, component: ObjRef, name: NameId) -> Result<Option<ObjRef>, VmError> {
    if name == NameId::NONE {
        return Ok(None);
    }
    let Some(root) = tree_of(vm, component)? else {
        return Ok(None);
    };
    if !is_tree(vm, root)? {
        return Ok(None);
    }
    if vm.space.heap.get(root)?.outer != Some(component) {
        return Err(NotYet::not_read(FOREIGN, RECORD).into());
    }
    for list in elements(&read(vm, root, "SkelControlLists")?, "SkelControlLists")? {
        let mut seen = ow_core::DetSet::new();
        let mut next = object_of(&field(vm, &list, "ControlHead", "SkelControlListHead")?);
        while let Some(c) = next {
            if !seen.insert(c) {
                return Err(script("a skeletal control list that loops back on itself".into()));
            }
            if name_in(vm, readable(vm, c)?, "ControlName")? == name {
                return Ok(Some(c));
            }
            next = object_of(&read(vm, c, "NextControl")?);
        }
    }
    Ok(None)
}

fn args(c: &NativeCall<'_>, what: &str) -> Result<(ObjRef, NameId), VmError> {
    let component = c.this.ok_or_else(|| script(format!("{what} called without an object")))?;
    match c.arg(0)? {
        Value::Name(n) => Ok((component, *n)),
        other => Err(script(format!("{what}'s argument is {}, not a name", other.kind_name()))),
    }
}

fn anim_node_native(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let (component, name) = args(c, FIND_ANIM_NODE)?;
    Ok(Value::Object(find_anim_node(c.vm, component, name)?))
}

fn skel_control_native(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let (component, name) = args(c, FIND_SKEL_CONTROL)?;
    Ok(Value::Object(find_skel_control(c.vm, component, name)?))
}

/// Binds both natives for [`PACKAGE`] (Off only; the caller checks the mode). A key bound first for Engine, or
/// VM-wide, keeps its binding; one bound only in another package does not count.
pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    if !vm.is_bound_in(PACKAGE, FIND_ANIM_NODE) {
        vm.register_native_in(PACKAGE, FIND_ANIM_NODE, Rc::new(anim_node_native))?;
    }
    if !vm.is_bound_in(PACKAGE, FIND_SKEL_CONTROL) {
        vm.register_native_in(PACKAGE, FIND_SKEL_CONTROL, Rc::new(skel_control_native))?;
    }
    Ok(())
}
