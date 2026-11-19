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
//! **The plan's two passes** (review r3 of #435):
//! 1. Every reference in the template's own slots is judged first, from handles and the packages' export tables
//!    alone. Nothing is loaded, so a refusal decided there changes nothing.
//! 2. Each template component is then loaded, checked whole (its slot count must be its class layout's; a stand-in
//!    is refused), and its own slots judged the same way, until nothing new turns up.
//!    - A refusal found inside a component's own fields comes after that component was loaded. That load is of the
//!      template's own definition, read as the archetype itself is read before the spawn's checks.
//!    - The store's loads cannot be staged. Refusing every component not yet loaded would refuse every installed
//!      class-default spawn, whose components are all lazy.
//!
//! Each object is judged on its whole outer chain, so the order objects are met in changes nothing: a sub-object of a
//! component, at any depth, is refused even when it is met before the component.
//!
//! **What is a component of the template.** An object reached from the template's slots, and then from each found
//! component's slots, at any depth: scalars, arrays, structs, interfaces and delegate targets, whatever the field's
//! declared type. It counts when:
//! - its run-time class is `Core.Component` or a subclass, matched by path along the class's own layout chain. No
//!   property flag is read: the flag bits for "instanced" are not identified in this port (rule 4); and
//! - its owner (outer) is the template: by handle, or by canonical export identity (package and export index). A
//!   class default object's sub-objects point at the handle of its `Default__<Class>` export in its class's package,
//!   which is another heap object than the class default, so that export is the template's identity. The export is
//!   chosen as the class-default decoder chooses it (`ow_object::cdo::default_export_in`: the exact name and the class
//!   reference). An object the store has not loaded has no outer yet, so its export's outers in the export table stand
//!   for it. Ownership is never matched by name.
//!
//! **Re-pointed:** every reference to a copied component, in the new actor's slots and in the copies' own slots, at
//! any depth, delegate targets included.
//!
//! **Refused with `NotYet { NotRead }`**, before anything is made:
//! - a reference back to the template itself (or its `Default__` handle): whether it becomes the new actor is not
//!   read;
//! - a sub-object of the template that is not a component: public UE3's `instanced` objects need the unidentified
//!   property flags;
//! - a sub-object of one of the template's components, at any depth (nested instancing);
//! - a template component whose properties are not loaded (a stand-in with no slots);
//! - a component owned by another class default object (inherited from a parent class's default) that the template has
//!   no sub-object of its own for (next paragraph).
//!
//! **An inherited component** (lane INHERITED-COMPONENT-INSTANCING, decision 2026-11-19-91). A template's slot can
//! name a component of a parent class's default object: a field the template's export does not serialize keeps the
//! class default's value, and that value is the default's own component. Public UE3 settles such a reference when it
//! loads the template: the template's own sub-object whose archetype is that component stands for it (the loader's
//! instancing step matches existing sub-objects by archetype). The port does the same from the packages, by export
//! identity, never by name:
//! - the template's own sub-objects are the exports directly outered to its export identity; each one's export
//!   archetype reference is resolved to the export it names (`CodeIndex::resolve_ref`);
//! - a reference to a class default's component whose export is one of those archetypes stands for the template's
//!   sub-object of that archetype. That sub-object is loaded, must be owned by the template (by handle or export
//!   identity), and is copied for the new actor like any other template component; the inherited reference is
//!   re-pointed to the same copy;
//! - where the template has no such sub-object, the reference stays a refusal.
//!
//! The structural oracle is the export table: on the installed `Sanctuary_Dynamic`, the target dummy archetype's own
//! `SpecialMoveComponent0` has `Default__WillowAIPawn.SpecialMoveComponent0` as its export archetype, the very object
//! the inherited slot names. **UNVERIFIED in the running game**; the bl-sdk check that would confirm it: a spawned
//! `WillowAIPawn`'s `SpecialMoveComponent` is a distinct object from its class default's, with the default's values.
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

/// The record of the inherited-component rule.
pub const RECORD_INHERITED: &str =
    "docs/decisions/2026-11/2026-11-19-91-rust-spawn-instances-inherited-components.md";

/// An export by filed package id and export index.
type ExportId = (u32, i32);

/// What a spawn from `template` will copy, worked out before anything is made ([`plan_components`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub template: ObjRef,
    /// The template's components, in discovery order.
    pub originals: Vec<ObjRef>,
    /// Inherited references and the template component that stands for each (module documentation): a parent class
    /// default's component, then the template's own sub-object whose export archetype it is.
    pub inherited: Vec<(ObjRef, ObjRef)>,
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
/// Loads a candidate component the store has not loaded, and resolves an inherited component's stand-in (which can
/// load it). Those lazy loads are writes; the plan makes and changes nothing else, so a spawn runs it before it makes
/// the actor. `NotYet { NotRead }` for what Off cannot copy or re-point correctly (module documentation).
pub fn plan_components(vm: &mut Vm, template: ObjRef) -> Result<Plan, VmError> {
    plan_components_tracked(vm, template, &mut false)
}

/// [`plan_components`], setting `loaded` once a lazy load may have run (review r2 of #478): a pass-2 load of a handle
/// that was not loaded, and every stand-in resolution, since `DefStore::resolve_ref` loads the handle it returns and
/// does not say whether it was loaded before (so it counts as a load, conservatively). The caller prefixes the
/// refusals after it.
pub fn plan_components_tracked(vm: &mut Vm, template: ObjRef, loaded: &mut bool) -> Result<Plan, VmError> {
    let identity = template_export(vm, template);
    let archetypes = own_archetypes(vm, identity);
    let mut originals: Vec<ObjRef> = Vec::new();
    let mut seen: Vec<ObjRef> = Vec::new();
    let mut pending: Vec<(ObjRef, (u32, i32))> = Vec::new();
    let mut inherited: Vec<(ObjRef, ObjRef)> = Vec::new();
    // Pass 1, nothing loaded: every reference the template's own slots hold is judged first (review r3 of #435, item
    // 3), from handles and the packages' export tables alone.
    let mut found = Vec::new();
    let mut judge = Judge { template, identity, archetypes: &archetypes };
    judge.slots(vm, template, &mut seen, &mut found, &mut pending)?;
    *loaded |= !pending.is_empty();
    stand_ins(vm, &judge, &mut pending, &mut seen, &mut found, &mut inherited)?;
    // Pass 2: each template component is loaded (it is the template's definition, read as the archetype itself is),
    // checked whole, and its own slots judged the same way, until nothing new turns up.
    let mut next = 0;
    while let Some(&component) = found.get(next) {
        next += 1;
        if vm.defs.is_unloaded(&vm.space, component) {
            *loaded = true;
            let done = vm.defs.ensure_loaded(&mut vm.space, component);
            vm.log.extend(vm.defs.take_log());
            done?;
        }
        let o = vm.space.heap.get(component)?;
        let want = vm.space.classes.get_layout(o.class).map(|l| l.slot_count());
        if u32::try_from(o.slots.len()).ok() != want {
            return Err(not_yet(&format!(
                "a template component whose properties are not loaded (a stand-in with no slots): {}",
                describe(vm, component)
            )));
        }
        originals.push(component);
        judge = Judge { template, identity, archetypes: &archetypes };
        judge.slots(vm, component, &mut seen, &mut found, &mut pending)?;
        *loaded |= !pending.is_empty();
        stand_ins(vm, &judge, &mut pending, &mut seen, &mut found, &mut inherited)?;
    }
    Ok(Plan { template, originals, inherited })
}

/// The template's own sub-objects that have an export archetype (module documentation, "An inherited component"):
/// `(the archetype's export, the sub-object's export)`, both by filed package id and export index. Reads the export
/// tables only.
fn own_archetypes(vm: &mut Vm, identity: Option<ExportId>) -> Vec<(ExportId, ExportId)> {
    let Some((package, index)) = identity else {
        return Vec::new();
    };
    let children: Vec<(i32, i32)> = {
        let pkg = vm.defs.packages().get(package);
        (1..=pkg.exports.len())
            .filter_map(|i| {
                let i = i32::try_from(i).ok()?;
                let e = pkg.object(i).ok()?;
                (e.outer == index && e.archetype != 0).then_some((i, e.archetype))
            })
            .collect()
    };
    let (code, store) = vm.defs.code_mut();
    let filed = code.filed_id(package);
    children
        .into_iter()
        .filter_map(|(child, archetype)| Some((code.resolve_ref(store, package, archetype)?, (filed, child))))
        .collect()
}

/// Resolves each pending inherited reference to the template's own sub-object (module documentation): loads it, checks
/// that the template owns it, adds it to the template components when it is new, and records the pair. A sub-object
/// another file holds under the identity policy, or one the template does not own, is `NotYet { NotRead }`.
fn stand_ins(
    vm: &mut Vm,
    judge: &Judge<'_>,
    pending: &mut Vec<(ObjRef, (u32, i32))>,
    seen: &mut Vec<ObjRef>,
    found: &mut Vec<ObjRef>,
    inherited: &mut Vec<(ObjRef, ObjRef)>,
) -> Result<(), VmError> {
    for (reference, (package, index)) in std::mem::take(pending) {
        let resolved = vm.defs.resolve_ref(&mut vm.space, package, index);
        vm.log.extend(vm.defs.take_log());
        let own = resolved?;
        // The path the reference resolves by can be held by another file: the object the store returns must be the
        // selected child itself, by canonical export identity (review r1 of #478, item 2; decision 2026-11-19-91).
        let canonical = |o: ObjRef| export_of(vm, o).map(|(p, i)| (vm.defs.code().filed_id(p), i));
        if let Some(o) = own
            && canonical(o) != Some((package, index))
        {
            return Err(not_yet_inherited(&format!(
                "an inherited component whose stand-in resolves to another export than the template's own \
                 sub-object (another file holds that path): {}",
                describe(vm, o)
            )));
        }
        let owned = own.is_some_and(|o| ancestors(vm, o).first().is_some_and(|a| judge.is_template(a)));
        let Some(own) = own.filter(|_| owned) else {
            return Err(not_yet_inherited(&format!(
                "an inherited component whose stand-in, the template's own sub-object of that archetype, is not the \
                 template's (another file holds it, or it has another outer): {}",
                describe(vm, reference)
            )));
        };
        if !is_component(vm, vm.space.heap.get(own)?.class) {
            return Err(not_yet_inherited(&format!(
                "an inherited component whose stand-in is not a component: {}",
                describe(vm, own)
            )));
        }
        if !seen.contains(&own) {
            seen.push(own);
        }
        if !found.contains(&own) {
            found.push(own);
        }
        if !inherited.iter().any(|p| p.0 == reference) {
            inherited.push((reference, own));
        }
    }
    Ok(())
}

/// What the plan judges against: the template, its export identity and its own sub-objects' export archetypes.
struct Judge<'a> {
    template: ObjRef,
    identity: Option<(u32, i32)>,
    archetypes: &'a [(ExportId, ExportId)],
}

impl Judge<'_> {
    /// Whether an outer link (handle, export identity) is the template.
    fn is_template(&self, a: &(Option<ObjRef>, Option<(u32, i32)>)) -> bool {
        a.0 == Some(self.template) || (self.identity.is_some() && a.1 == self.identity)
    }

    /// Judges every object `holder`'s slots refer to (module documentation), at any depth: a template component is
    /// added to `found`; an inherited component with a stand-in goes to `pending`; a refusal is `NotYet { NotRead }`;
    /// anything else stays shared. Loads nothing.
    fn slots(
        &self,
        vm: &Vm,
        holder: ObjRef,
        seen: &mut Vec<ObjRef>,
        found: &mut Vec<ObjRef>,
        pending: &mut Vec<(ObjRef, (u32, i32))>,
    ) -> Result<(), VmError> {
        let (template, identity) = (self.template, self.identity);
        let mut refs = Vec::new();
        for v in &vm.space.heap.get(holder)?.slots {
            collect_refs(v, &mut refs);
        }
        let is_template = |a: &(Option<ObjRef>, Option<(u32, i32)>)| self.is_template(a);
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
            // The whole outer chain, so the judgement does not depend on the order objects are met in (review r3,
            // item 1).
            let chain = ancestors(vm, r);
            let Some(first) = chain.first() else {
                continue;
            };
            if is_template(first) {
                if !component {
                    return Err(not_yet(&format!(
                        "a sub-object of the template that is not a component (which ones are instanced needs the \
                         unidentified property flags): {}",
                        describe(vm, r)
                    )));
                }
                found.push(r);
            } else if chain[1..].iter().any(is_template) {
                return Err(not_yet(&format!(
                    "a sub-object of one of the template's components (nested instancing): {}",
                    describe(vm, r)
                )));
            } else if chain.len() > 1 && !is_default(vm, first) && chain[1..].iter().any(|a| is_default(vm, a)) {
                // An object inside a class default's sub-object (an inherited component's own sub-object): sharing it
                // would point the copy into the parent default's graph (review r1 of #478, item 3).
                return Err(not_yet_inherited(&format!(
                    "a sub-object of another class default object's sub-object (nested inside an inherited \
                     component): {}",
                    describe(vm, r)
                )));
            } else if component && let Some(owner) = default_outer(vm, r) {
                // Decision 2026-11-19-91: the template's own sub-object of that archetype stands for it.
                let key = export_of(vm, r).map(|(p, i)| (vm.defs.code().filed_id(p), i));
                let mut matches = self.archetypes.iter().filter(|(a, _)| key == Some(*a));
                match (matches.next(), matches.next()) {
                    (Some(&(_, own)), None) => pending.push((r, own)),
                    // Two of the template's sub-objects made from that archetype: which stands for it is not read
                    // (review r1 of #478, item 1).
                    (Some(_), Some(_)) => {
                        return Err(not_yet_inherited(&format!(
                            "a component of another class default object with more than one sub-object of the \
                             template made from it: {} of {owner}",
                            describe(vm, r)
                        )));
                    }
                    (None, _) => {
                        return Err(not_yet_inherited(&format!(
                            "a component of another class default object (inherited from a parent class's default), \
                             with no sub-object of the template made from it: {} of {owner}",
                            describe(vm, r)
                        )));
                    }
                }
            }
        }
        Ok(())
    }
}

/// `o`'s outers, innermost first, each as its handle (when the heap has it) and its export identity (when it has one):
/// the heap's outer links, then, past an object the store has not given an outer yet, its export's outers in its
/// package's export table. Bounded at 256 links.
fn ancestors(vm: &Vm, o: ObjRef) -> Vec<(Option<ObjRef>, Option<(u32, i32)>)> {
    let mut chain = Vec::new();
    let mut cursor = o;
    while chain.len() < 256 {
        let Ok(obj) = vm.space.heap.get(cursor) else {
            return chain;
        };
        match obj.outer {
            Some(outer) => {
                chain.push((Some(outer), export_of(vm, outer)));
                cursor = outer;
            }
            None => break,
        }
    }
    let Some((package, mut index)) = export_of(vm, cursor) else {
        return chain;
    };
    let pkg = vm.defs.packages().get(package);
    while chain.len() < 256 {
        let Ok(entry) = pkg.object(index) else {
            break;
        };
        if entry.outer <= 0 {
            break;
        }
        index = entry.outer;
        chain.push((None, Some((package, index))));
    }
    chain
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
    // An inherited reference is re-pointed to its stand-in's copy (decision 2026-11-19-91).
    let mut swaps = pairs.clone();
    for &(reference, own) in &plan.inherited {
        if let Some(&(_, copy)) = pairs.iter().find(|p| p.0 == own) {
            swaps.push((reference, copy));
        }
    }
    // A failure while re-pointing frees the copies too (review r1 of #528, item 2); the caller frees the actor.
    let repointed = (|| -> Result<(), VmError> {
        for o in std::iter::once(actor).chain(pairs.iter().map(|p| p.1)) {
            let slots: Vec<Value> = vm.space.heap.get(o)?.slots.iter().map(|v| repoint(v, &swaps)).collect();
            vm.space.heap.get_mut(o)?.slots = slots;
        }
        Ok(())
    })();
    if let Err(e) = repointed {
        let copies: Vec<ObjRef> = pairs.iter().map(|p| p.1).collect();
        let _ = vm.space.free_many(&copies);
        return Err(e);
    }
    Ok(pairs)
}

fn not_yet(what: &str) -> VmError {
    NotYet::not_read(format!("Spawn: {what}"), RECORD_INSTANCING).into()
}

fn not_yet_inherited(what: &str) -> VmError {
    NotYet::not_read(format!("Spawn: {what}"), RECORD_INHERITED).into()
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
    // The export the class-default decoder applies (review r3 of #435, item 4): the same selection, shared.
    let at = ow_object::cdo::default_export_in(&package.exports, info.export, &wanted)?;
    Some((info.package, at))
}

/// The package export an object stands for (its origin), when it has one.
fn export_of(vm: &Vm, o: ObjRef) -> Option<(u32, i32)> {
    match vm.space.heap.get(o).ok()?.origin {
        Origin::Export { package, index } | Origin::Resource { package, index } if index > 0 => Some((package, index)),
        _ => None,
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

/// Whether outer link `a` is a class default object (`Default__<Class>`), by its handle's name or its export's.
fn is_default(vm: &Vm, a: &(Option<ObjRef>, Option<(u32, i32)>)) -> bool {
    let name = match a {
        (Some(h), _) => vm.space.heap.get(*h).ok().map(|o| vm.space.names.text(o.name).to_string()),
        (None, Some((p, i))) => vm.defs.packages().get(*p).object(*i).ok().map(|e| e.name.clone()),
        (None, None) => None,
    };
    name.is_some_and(|n| n.starts_with("Default__"))
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
