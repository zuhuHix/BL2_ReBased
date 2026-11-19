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
//! **The rule for Off: copy and re-point correctly, or refuse with a named `NotYet`; never share a reference
//! silently** (review r2 of #435).
//!
//! **Two steps.** [`plan_components`] works out what to copy and runs every refusal before anything is made, so a
//! spawn calls it before it makes the actor. [`apply_plan`] then makes the copies and re-points; if a copy cannot be
//! made, the copies made so far are freed and the caller frees the actor.
//!
//! **What is a component of the template.** An object reached from the template's slots, and then from each found
//! component's slots, at any depth: scalars, arrays, structs, interfaces and delegate targets, whatever the field's
//! declared type. It counts when:
//! - its run-time class is `Core.Component` or a subclass, matched by path along the class's own layout chain. No
//!   property flag is read: the flag bits for "instanced" are not identified in this port (rule 4); and
//! - its owner (outer) is the template: by handle, or by canonical export identity (package and export index). A
//!   class default object's sub-objects point at the handle of its `Default__<Class>` export in its class's package,
//!   which is another heap object than the class default, so that export is the template's identity. A component the
//!   store has not loaded is loaded first, since the store sets its outer on load, and a load that fails stops the
//!   spawn. Ownership is never matched by name.
//!
//! **Re-pointed:** every reference to a copied component, in the new actor's slots and in the copies' own slots, at
//! any depth, delegate targets included.
//!
//! **Refused with `NotYet { NotRead }`**, before anything is made:
//! - a reference back to the template itself (or its `Default__` handle): whether it becomes the new actor is not
//!   read;
//! - a sub-object of the template that is not a component: public UE3's `instanced` objects need the unidentified
//!   property flags;
//! - a sub-object of one of the template's components (nested instancing);
//! - a component owned by another class default object (inherited from a parent class's default).
//!
//! **Left as it is (a plain shared reference):** an object owned by anything else, such as another archetype's
//! component or an asset. That is a reference the template does not own, so sharing it is the rule, not a fallback.
//!
//! **What a copy is.** The same class and name, outered to the new actor, its slots copied from the template's
//! component and re-pointed. The copies' archetype link is not kept: the port's heap has no archetype field.
//!
//! **Lifetime, UNVERIFIED.** A copy refers to its actor (its outer, and often an `Owner` field), and the actor to the
//! copy. `release_unreferenced` does not free such a loop; the port does not model the game's garbage collection.

use ow_core::NotYet;
use ow_object::heap::Origin;
use ow_object::ids::{ClassId, ObjRef};
use ow_object::value::{DelegateValue, StructValue, Value};
use ow_vm::api::Vm;
use ow_vm::error::VmError;
use std::rc::Rc;

/// The class whose subclasses are components (public UE3 `Core.Component`), matched by path along a class's own
/// layout chain (no lookup that could fail).
pub const COMPONENT_CLASS: &str = "Core.Component";

/// The record of this rule.
pub const RECORD_INSTANCING: &str = "docs/decisions/2026-11/2026-11-19-44-rust-spawn-gives-each-actor-its-own-template-components.md";

/// What a spawn from `template` will copy, worked out before anything is made ([`plan_components`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub template: ObjRef,
    /// The template's components, in discovery order.
    pub originals: Vec<ObjRef>,
}

/// Gives `actor`, just made from `template`'s slots, its own copy of each component `template` owns (module
/// documentation): [`plan_components`], then [`apply_plan`]. Answers the `(template component, copy)` pairs.
pub fn instance_components(
    vm: &mut Vm,
    template: ObjRef,
    actor: ObjRef,
) -> Result<Vec<(ObjRef, ObjRef)>, VmError> {
    let plan = plan_components(vm, template)?;
    apply_plan(vm, &plan, actor)
}

/// Finds the components `template` owns, through its slots and then each found component's own, at any depth
/// (scalars, arrays, structs, delegates), by each object's run-time class and owner, not the declared field type.
/// Loads a candidate component the store has not loaded. Changes nothing else, so a spawn can run it before it makes
/// the actor. `NotYet { NotRead }` for what Off cannot copy or re-point correctly (module documentation).
pub fn plan_components(vm: &mut Vm, template: ObjRef) -> Result<Plan, VmError> {
    let identity = template_export(vm, template);
    let mut originals: Vec<ObjRef> = Vec::new();
    let mut seen: Vec<ObjRef> = Vec::new();
    let mut scan = vec![template];
    let mut next = 0;
    while let Some(&holder) = scan.get(next) {
        next += 1;
        let mut refs = Vec::new();
        for v in &vm.space.heap.get(holder)?.slots {
            collect_refs(v, &mut refs);
        }
        for r in refs {
            if r == template || (identity.is_some() && export_of(vm, r) == identity) {
                return Err(not_yet(
                    "a reference back to the template itself (whether it becomes the new actor is not read)",
                ));
            }
            if seen.contains(&r) {
                continue;
            }
            seen.push(r);
            let component = is_component(vm, vm.space.heap.get(r)?.class);
            // An unloaded component handle has no outer yet: the store sets it on load.
            if component && vm.defs.is_unloaded(&vm.space, r) {
                let loaded = vm.defs.ensure_loaded(&mut vm.space, r);
                vm.log.extend(vm.defs.take_log());
                loaded?;
            }
            if owned_by(vm, r, template, identity) {
                if !component {
                    return Err(not_yet(&format!(
                        "a sub-object of the template that is not a component (which ones are instanced needs the \
                         unidentified property flags): {}",
                        describe(vm, r)
                    )));
                }
                originals.push(r);
                scan.push(r);
            } else if originals.iter().any(|&c| owned_by(vm, r, c, export_of(vm, c))) {
                return Err(not_yet(&format!(
                    "a sub-object of one of the template's components (nested instancing): {}",
                    describe(vm, r)
                )));
            } else if component && let Some(owner) = default_outer(vm, r) {
                return Err(not_yet(&format!(
                    "a component of another class default object (inherited from a parent class's default): {} of \
                     {owner}",
                    describe(vm, r)
                )));
            }
        }
    }
    Ok(Plan { template, originals })
}

/// Makes the copies of `plan` for `actor` and re-points the actor's and the copies' references, delegates included.
/// A copy that cannot be made frees the copies made so far and fails; the caller frees the actor.
pub fn apply_plan(vm: &mut Vm, plan: &Plan, actor: ObjRef) -> Result<Vec<(ObjRef, ObjRef)>, VmError> {
    let mut pairs: Vec<(ObjRef, ObjRef)> = Vec::with_capacity(plan.originals.len());
    for &original in &plan.originals {
        let (c, name, slots) = {
            let o = vm.space.heap.get(original)?;
            (o.class, vm.space.names.text(o.name).to_string(), o.slots.clone())
        };
        let made = vm.space.alloc(c, &name, Some(actor), Origin::Runtime).map_err(VmError::from).and_then(|copy| {
            vm.space.heap.get_mut(copy)?.slots = slots;
            Ok(copy)
        });
        match made {
            Ok(copy) => pairs.push((original, copy)),
            Err(e) => {
                let copies: Vec<ObjRef> = pairs.iter().map(|p| p.1).collect();
                let _ = vm.space.free_many(&copies);
                return Err(e);
            }
        }
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

fn not_yet(what: &str) -> VmError {
    NotYet::not_read(format!("Spawn: {what}"), RECORD_INSTANCING).into()
}

/// Whether `class` is `Core.Component` or a subclass, by the paths of its own layout chain.
fn is_component(vm: &Vm, class: ClassId) -> bool {
    let mut cursor = Some(class);
    let mut steps = 0;
    while let Some(c) = cursor {
        let Some(layout) = vm.space.classes.get_layout(c) else {
            return false;
        };
        if layout.path().as_str().eq_ignore_ascii_case(COMPONENT_CLASS) {
            return true;
        }
        steps += 1;
        if steps > 256 {
            return false;
        }
        cursor = layout.super_class();
    }
    false
}

/// The template's canonical export identity: the export it was loaded from, or, for a class default object, the
/// `Default__<Class>` export of its class's own package (its sub-objects' outers point at that export's handle,
/// another heap object than the class default). Compared by package and export index, never by name.
fn template_export(vm: &Vm, template: ObjRef) -> Option<(u32, i32)> {
    if let Some(id) = export_of(vm, template) {
        return Some(id);
    }
    let Origin::Default { class } = vm.space.heap.get(template).ok()?.origin else {
        return None;
    };
    let code = vm.defs.code();
    let info = code.class(code.key_of(class)?);
    let package = vm.defs.packages().get(info.package);
    let wanted = format!("Default__{}", info.name);
    let at = package.exports.iter().position(|e| e.outer == 0 && e.name.eq_ignore_ascii_case(&wanted))?;
    Some((info.package, i32::try_from(at).ok()? + 1))
}

/// The package export an object stands for (its origin), when it has one.
fn export_of(vm: &Vm, o: ObjRef) -> Option<(u32, i32)> {
    match vm.space.heap.get(o).ok()?.origin {
        Origin::Export { package, index } | Origin::Resource { package, index } if index > 0 => Some((package, index)),
        _ => None,
    }
}

/// Whether `owner` is `o`'s outer: by handle, or by the outer's canonical export identity (`identity`, the owner's
/// export), which covers an outer that is another handle of the same export and an object the store has not given an
/// outer yet (its export's outer in the same package). Never by name.
fn owned_by(vm: &Vm, o: ObjRef, owner: ObjRef, identity: Option<(u32, i32)>) -> bool {
    let Ok(obj) = vm.space.heap.get(o) else {
        return false;
    };
    if let Some(outer) = obj.outer {
        return outer == owner || (identity.is_some() && export_of(vm, outer) == identity);
    }
    let (Some(identity), Some((package, index))) = (identity, export_of(vm, o)) else {
        return false;
    };
    match vm.defs.packages().get(package).object(index) {
        Ok(entry) => entry.outer > 0 && (package, entry.outer) == identity,
        Err(_) => false,
    }
}

/// `o`'s name and class, for a refusal's text.
fn describe(vm: &Vm, o: ObjRef) -> String {
    match vm.space.heap.get(o) {
        Ok(x) => format!("{} ({})", vm.space.names.text(x.name), ow_attr::eval::class_path(&vm.space, o)),
        Err(_) => format!("{o:?}"),
    }
}

/// The name of `o`'s outer when that outer is a class default object (`Default__...`), by the outer's own name.
fn default_outer(vm: &Vm, o: ObjRef) -> Option<String> {
    let obj = vm.space.heap.get(o).ok()?;
    let name = match obj.outer {
        Some(outer) => vm.space.names.text(vm.space.heap.get(outer).ok()?.name).to_string(),
        None => export_of(vm, o).and_then(|(p, i)| {
            let pkg = vm.defs.packages().get(p);
            let outer = pkg.object(i).ok()?.outer;
            (outer > 0).then(|| pkg.object(outer).ok().map(|e| e.name.clone())).flatten()
        })?,
    };
    name.starts_with("Default__").then_some(name)
}

/// Every object `v` refers to, at any depth (objects, interfaces, delegate targets, arrays, structs).
fn collect_refs(v: &Value, out: &mut Vec<ObjRef>) {
    match v {
        Value::Object(Some(r)) | Value::Interface(Some(r)) => out.push(*r),
        Value::Delegate(Some(DelegateValue { object: Some(r), .. })) => out.push(*r),
        Value::Array(items) => items.iter().for_each(|i| collect_refs(i, out)),
        Value::Struct(s) => s.fields.iter().for_each(|f| collect_refs(f, out)),
        _ => {}
    }
}

/// `v` with every reference to a copied component replaced by its copy, at any depth (delegate targets too).
fn repoint(v: &Value, pairs: &[(ObjRef, ObjRef)]) -> Value {
    let swap = |r: &Option<ObjRef>| match r {
        Some(x) => Some(pairs.iter().find(|p| p.0 == *x).map_or(*x, |p| p.1)),
        None => None,
    };
    match v {
        Value::Object(r) => Value::Object(swap(r)),
        Value::Interface(r) => Value::Interface(swap(r)),
        Value::Delegate(Some(d)) => Value::Delegate(Some(DelegateValue { object: swap(&d.object), function: d.function })),
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

/// Whether `v` holds, at any depth, a reference to a copied component (delegate targets too).
fn refers(v: &Value, pairs: &[(ObjRef, ObjRef)]) -> bool {
    let mut refs = Vec::new();
    collect_refs(v, &mut refs);
    refs.iter().any(|r| pairs.iter().any(|p| p.0 == *r))
}

#[cfg(test)]
#[path = "instancing_tests.rs"]
mod tests;
