//! Owner: lane SUBOBJECT-INSTANCING (decision 2026-11-19-44). The per-actor copy of a template's components, made
//! when the world spawns an actor. AI-assisted (Claude Code).
//!
//! **The rule is stock UE3, and UNVERIFIED in this game.** An actor made from a template (its class default object,
//! or an archetype) gets its own copy of each component the template owns. The template's component sub-objects are
//! not shared with it. Public UE3 does this when an object is made from a template (component instancing, the
//! `InstanceSubobjectTemplates` step). This port knows it from public UE3 only: no native of this game was read for
//! it, and no real-game session checked it. Off only: under `ParityMode::Reference` the C++ stand-in shares the
//! template's slots as they are (R3), so the spawn there does not call this.
//!
//! **Which slots count.** A slot counts when its declared class (`PropKind::Object`, or an array of it) is
//! `Core.Component` or a subclass of it. In public UE3 that is a `ComponentProperty`, which the port's reflection reads
//! as an object property of its component class (`ow_object::layout::PropKind`). Only the class is checked, by its
//! name. No property flag is read: the flag bits for "instanced" are not identified in this port (rule 4).
//! - **Copied:** an object in such a slot, of a component class, whose outer is the template. "The template" is
//!   matched by path (the same name and class under the same outers), because the definition store holds a class
//!   default object and the handle of its `Default__<Class>` export, which its sub-objects' outers point at, as two
//!   heap objects. A candidate the store has not loaded is loaded first, since its outer is set on load; a load
//!   that fails stops the spawn with the store's error.
//! - **Left as it is (a plain shared reference):** an object whose outer is anything else, such as another archetype's
//!   component or an asset.
//! - **Found transitively:** the actor's component fields are searched, then each found component's own component
//!   fields, with the same outer test, until nothing new turns up (review r1 of #435). A component reached only
//!   through another one is copied too, so no copy points at a template component.
//! - **Re-pointed:** every reference to a copied component, in the new actor's slots and in the copies' own slots, at
//!   any depth (arrays, structs). This covers `Components` and the named field holding the same object.
//!
//! **UNVERIFIED, left shared.** These are not instanced, because the rule that would cover them is not read:
//! - a component the template holds only inside a struct;
//! - a non-component object outered to the template (public UE3's `instanced` / `editinlinenew` objects, which need
//!   the unidentified flags);
//! - a component's own sub-objects (nested sub-objects, outered to the template's component);
//! - a component inherited from a parent class's default (its outer is that parent's `Default__` object, not the
//!   template);
//! - references to the template itself, or to its `Default__` export handle, held by the actor or a copy: they are not
//!   re-pointed to the new actor.
//!
//! **Lifetime, UNVERIFIED.** A copy refers to its actor (its outer, and often an `Owner` field), and the actor to the
//! copy. `release_unreferenced` does not free such a loop; the port does not model the game's garbage collection.
//!
//! **What a copy is.** The same class and name, outered to the new actor. Its slots are copied from the template's
//! component, with the references re-pointed as above. The copies' archetype link is not kept: the port's heap has
//! no archetype field.

use ow_object::heap::Origin;
use ow_object::ids::{ClassId, ObjRef};
use ow_object::layout::PropKind;
use ow_object::value::{StructValue, Value};
use ow_vm::api::Vm;
use ow_vm::error::VmError;
use std::rc::Rc;

/// The class whose subclasses are components (public UE3 `Core.Component`), found by name.
pub const COMPONENT_CLASS: &str = "Core.Component";

/// Gives `actor`, just made from `template`'s slots, its own copy of each component `template` owns (module
/// documentation). Answers the `(template component, copy)` pairs, in slot order. A class table with no
/// `Core.Component` has no components: nothing is copied.
pub fn instance_components(
    vm: &mut Vm,
    template: ObjRef,
    actor: ObjRef,
) -> Result<Vec<(ObjRef, ObjRef)>, VmError> {
    let Ok(component) = vm.find_class(COMPONENT_CLASS) else {
        return Ok(Vec::new());
    };
    // The actor's component fields first, then each found component's own, until nothing new turns up: a component
    // reached only through another one is copied too (review r1 of #435).
    let mut found: Vec<ObjRef> = Vec::new();
    let mut scan = vec![actor];
    let mut next = 0;
    while let Some(&o) = scan.get(next) {
        next += 1;
        for r in component_refs(vm, o, component)? {
            if found.contains(&r) || !vm.space.classes.is_child_of(vm.space.heap.get(r)?.class, component) {
                continue;
            }
            // An unloaded handle has no outer yet: the store sets it on load.
            if vm.defs.is_unloaded(&vm.space, r) {
                let loaded = vm.defs.ensure_loaded(&mut vm.space, r);
                vm.log.extend(vm.defs.take_log());
                loaded?;
            }
            let outer = vm.space.heap.get(r)?.outer;
            if let Some(outer) = outer
                && same_object(vm, outer, template)?
            {
                found.push(r);
                scan.push(r);
            }
        }
    }
    let mut pairs = Vec::with_capacity(found.len());
    for original in found {
        let (c, name, slots) = {
            let o = vm.space.heap.get(original)?;
            (o.class, vm.space.names.text(o.name).to_string(), o.slots.clone())
        };
        let copy = vm.space.alloc(c, &name, Some(actor), Origin::Runtime)?;
        vm.space.heap.get_mut(copy)?.slots = slots;
        pairs.push((original, copy));
    }
    if pairs.is_empty() {
        return Ok(pairs);
    }
    for o in std::iter::once(actor).chain(pairs.iter().map(|p| p.1)) {
        let slots: Vec<Value> = vm.space.heap.get(o)?.slots.iter().map(|v| repoint(v, &pairs)).collect();
        vm.space.heap.get_mut(o)?.slots = slots;
    }
    Ok(pairs)
}

/// Whether `a` and `b` name the same object: the same heap object, or the same name and class under outers that name
/// the same object (both `None` at the top). The definition store holds a class default object and its
/// `Default__<Class>` export as two heap objects of one path: the class default (`Origin::Default`) and the export's
/// handle, which a sub-object's outer points at.
fn same_object(vm: &Vm, a: ObjRef, b: ObjRef) -> Result<bool, VmError> {
    let (mut a, mut b) = (Some(a), Some(b));
    while let (Some(x), Some(y)) = (a, b) {
        if x == y {
            return Ok(true);
        }
        let (ox, oy) = (vm.space.heap.get(x)?, vm.space.heap.get(y)?);
        if ox.name != oy.name || ox.class != oy.class {
            return Ok(false);
        }
        (a, b) = (ox.outer, oy.outer);
    }
    Ok(a.is_none() && b.is_none())
}

/// The objects `o` holds in its component fields (scalar or array), in slot order. Each field's slots are found from
/// its recorded first slot (`PropId::slot`); a field outside `o`'s slots is an error, never a silent cut.
fn component_refs(vm: &Vm, o: ObjRef, component: ClassId) -> Result<Vec<ObjRef>, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let slots = &vm.space.heap.get(o)?.slots;
    let mut refs = Vec::new();
    for (id, decl) in vm.space.classes.props(class).map_err(ow_object::heap::HeapError::Layout)? {
        if !is_component_kind(vm, &decl.kind, component) {
            continue;
        }
        let dim = u32::try_from(decl.array_dim).unwrap_or(0);
        for k in 0..dim {
            let at = usize::try_from(id.slot + k).unwrap_or(usize::MAX);
            let value = slots.get(at).ok_or_else(|| VmError::Script {
                function: "Spawn".to_string(),
                message: format!(
                    "{}: slot {at} of a component field is outside its {} slots",
                    ow_attr::eval::class_path(&vm.space, o),
                    slots.len()
                ),
            })?;
            match value {
                Value::Object(Some(r)) => refs.push(*r),
                Value::Array(items) => refs.extend(items.iter().filter_map(|v| match v {
                    Value::Object(Some(r)) => Some(*r),
                    _ => None,
                })),
                _ => {}
            }
        }
    }
    Ok(refs)
}

/// An object property of `Core.Component` or a subclass, or an array of one.
fn is_component_kind(vm: &Vm, kind: &PropKind, component: ClassId) -> bool {
    match kind {
        PropKind::Object { class } => vm.space.classes.is_child_of(*class, component),
        PropKind::Array(inner) => matches!(inner.as_ref(), PropKind::Object { class }
            if vm.space.classes.is_child_of(*class, component)),
        _ => false,
    }
}

/// `v` with every reference to a copied component replaced by its copy, at any depth.
fn repoint(v: &Value, pairs: &[(ObjRef, ObjRef)]) -> Value {
    let swap = |r: &Option<ObjRef>| match r {
        Some(x) => Some(pairs.iter().find(|p| p.0 == *x).map_or(*x, |p| p.1)),
        None => None,
    };
    match v {
        Value::Object(r) => Value::Object(swap(r)),
        Value::Interface(r) => Value::Interface(swap(r)),
        Value::Array(items) if items.iter().any(|i| refers(i, pairs)) => {
            Value::Array(Rc::new(items.iter().map(|i| repoint(i, pairs)).collect()))
        }
        Value::Struct(s) if s.fields.iter().any(|f| refers(f, pairs)) => Value::Struct(Rc::new(StructValue {
            ty: s.ty,
            fields: s.fields.iter().map(|f| repoint(f, pairs)).collect(),
        })),
        other => other.clone(),
    }
}

/// Whether `v` holds, at any depth, a reference to a copied component.
fn refers(v: &Value, pairs: &[(ObjRef, ObjRef)]) -> bool {
    match v {
        Value::Object(Some(r)) | Value::Interface(Some(r)) => pairs.iter().any(|p| p.0 == *r),
        Value::Array(items) => items.iter().any(|i| refers(i, pairs)),
        Value::Struct(s) => s.fields.iter().any(|f| refers(f, pairs)),
        _ => false,
    }
}

#[cfg(test)]
#[path = "instancing_tests.rs"]
mod tests;
