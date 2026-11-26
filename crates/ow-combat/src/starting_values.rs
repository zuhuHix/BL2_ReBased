//! Owner: lane IMPL-SPAWN-SETBASEVALUE (decision 2026-11-26-NNN). The combat world's set-up preload of the attribute
//! starting values' definitions, in `ParityMode::Off`. AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from docs/verification/NATIVE_SPAWN_SETBASEVALUE.md section 3 (#811 at 3aedca6) alone;
//! this lane did not read the executable. **UNVERIFIED in the running game**: the game keeps these definitions
//! resident; the port loads them at set-up, a logged harness input, so that `SetBaseValue`'s own preflight (which
//! refuses an unloaded handle, #572) finds them. The native is unchanged. Reference does nothing and logs nothing.
//!
//! **The walk**, for one AI pawn archetype (a copy of its export, as the spawn makes one, freed after):
//! 1. its `AIClass`, then that class's `AttributeParentClass` chain. Each class object is loaded when it is a handle
//!    (logged), after its class's declarations are checked: `AttributeStartingValues` an array of structs,
//!    `AttributeParentClass` an object (else `VmError::Script`, nothing loaded);
//! 2. a class whose `AttributeStartingValuesNew` is declared and non-empty is [`CATEGORIES`] (`NotRead`: the category
//!    entries' layout is not read), decided before any definition loads;
//! 3. the starting values, parents first, then the archetype's own list: each entry's `Attribute` (an object), and its
//!    `BaseValue` read with the evaluator's own data reader (`ow_attr::eval::init_data`): its `BaseValueAttribute` and
//!    `InitializationDefinition`;
//! 4. what `SetBaseValue`'s own preflight walks (`ow_attr::eval::preflight_set_base_value`; the hub's point (b) on
//!    #811): each attribute found (the destination, a base-value attribute, an expression operand), with its
//!    `ContextResolverChain` entries and its `ValueResolverChain` entries up to the first None; and each
//!    initialization definition, followed into the nested data its evaluation reaches: the enabled formula's four
//!    terms, else the enabled conditional's entries (each expression's two attribute operands, its `BaseValueIfTrue`;
//!    the scan stops at an entry with no expressions, else `DefaultBaseValue`), the enabled variance's two bounds and
//!    the enabled range limits, recursively to the preflight's own depth. A part that does not read as the evaluator
//!    reads it is left: the native's preflight then refuses it with its own typed error, so what this walk does not
//!    cover still shows.
//!
//! Each unloaded object is loaded through the store and logged, "harness input: preload <path> (a SetBaseValue
//! definition the game keeps resident; UNVERIFIED)", then its outcome. A failed load is logged and left, so the native's
//! own refusal shows it later; its fields are not read.

use ow_attr::api::{AttrError, EvalCx, InitData, WorldParts};
use ow_core::{NotYet, ParityMode};
use ow_defs::DefStore;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ClassId, ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

/// The record of the rule.
pub const RECORD: &str = "docs/verification/NATIVE_SPAWN_SETBASEVALUE.md (section 3)";
pub const CATEGORIES: &str = "the starting-value categories' layout (AttributeStartingValuesNew)";
const WHAT: &str = "the starting-value preload";

fn bad(message: impl Into<String>) -> VmError {
    VmError::Script { function: WHAT.to_string(), message: message.into() }
}

/// The declared kind of `name` on `class`, None when undeclared.
fn declared(vm: &Vm, class: ClassId, name: &str) -> Option<PropKind> {
    let p = vm.space.prop(class, name)?;
    vm.space.classes.decl(p).map(|d| d.kind.clone())
}

fn want_object(vm: &Vm, class: ClassId, name: &str) -> Result<(), VmError> {
    match declared(vm, class, name) {
        Some(PropKind::Object { .. }) => Ok(()),
        other => Err(bad(format!("{name} is declared as {other:?}, not an object"))),
    }
}

fn want_struct_array(vm: &Vm, class: ClassId, name: &str) -> Result<(), VmError> {
    match declared(vm, class, name) {
        Some(PropKind::Array(inner)) if matches!(*inner, PropKind::Struct(_)) => Ok(()),
        other => Err(bad(format!("{name} is declared as {other:?}, not an array of structs"))),
    }
}

fn read(vm: &Vm, o: ObjRef, name: &str) -> Result<Value, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| bad(format!("no {name}")))?;
    Ok(vm.space.read(o, PropAddr::scalar(p))?.clone())
}

fn object(v: &Value) -> Option<ObjRef> {
    match v {
        Value::Object(o) => *o,
        _ => None,
    }
}

/// Loads `o` when it is an unloaded handle, logging the harness input and its outcome into `lines`; whether `o` is
/// readable after.
fn preload_in(space: &mut ObjectSpace, defs: &mut DefStore, o: ObjRef, lines: &mut Vec<String>, loaded: &mut usize) -> bool {
    if !defs.is_unloaded(space, o) {
        return !defs.is_failed(space, o);
    }
    let path = defs.handle_path(o).unwrap_or_else(|| ow_attr::eval::class_path(space, o).to_string() + " object");
    lines.push(format!("harness input: preload {path} (a SetBaseValue definition the game keeps resident; UNVERIFIED)"));
    let r = defs.ensure_loaded(space, o);
    lines.extend(defs.take_log());
    match r {
        Ok(()) => {
            lines.push(format!("harness input: preload {path}: loaded"));
            *loaded += 1;
            true
        }
        Err(e) => {
            lines.push(format!("harness input: preload {path}: failed ({e})"));
            false
        }
    }
}

/// [`preload_in`] on the VM's store and log.
fn preload(vm: &mut Vm, o: ObjRef, loaded: &mut usize) -> bool {
    let mut lines = Vec::new();
    let ok = preload_in(&mut vm.space, &mut vm.defs, o, &mut lines, loaded);
    vm.log.extend(lines);
    ok
}

/// The starting-value entries of `o`'s `AttributeStartingValues`: (attribute, base value) for each. Reads only.
fn entries(vm: &Vm, o: ObjRef) -> Result<Vec<(Option<ObjRef>, InitData)>, VmError> {
    let list = match read(vm, o, "AttributeStartingValues")? {
        Value::Array(a) => a,
        other => return Err(bad(format!("AttributeStartingValues holds {}, not an array", other.kind_name()))),
    };
    let mut out = Vec::new();
    for entry in list.iter() {
        let attribute = match ow_attr::eval::field(&vm.space, entry, "Attribute").map_err(VmError::from)? {
            Value::Object(a) => *a,
            other => return Err(bad(format!("a starting value's Attribute holds {}, not an object", other.kind_name()))),
        };
        let base = ow_attr::eval::field(&vm.space, entry, "BaseValue").map_err(VmError::from)?.clone();
        let data = ow_attr::eval::init_data(&vm.space, &base).map_err(VmError::from)?;
        out.push((attribute, data));
    }
    Ok(out)
}

/// The preload for the archetype object `archetype` (module documentation); the number of objects loaded. Off only:
/// in Reference nothing is read, loaded or logged.
pub fn preload_from(vm: &mut Vm, archetype: ObjRef) -> Result<usize, VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(0);
    }
    let mut loaded = 0;
    let archetype_class = vm.space.heap.get(archetype)?.class;
    want_object(vm, archetype_class, "AIClass")?;
    // Step 1: the class chain, each class's declarations checked before it loads.
    let mut chain: Vec<ObjRef> = Vec::new();
    let mut cursor = object(&read(vm, archetype, "AIClass")?);
    while let Some(c) = cursor {
        // A class already seen ends the walk (a looping parent chain cannot run forever).
        if chain.contains(&c) {
            break;
        }
        let class = vm.space.heap.get(c)?.class;
        want_struct_array(vm, class, "AttributeStartingValues")?;
        want_object(vm, class, "AttributeParentClass")?;
        if !preload(vm, c, &mut loaded) {
            break;
        }
        chain.push(c);
        cursor = object(&read(vm, c, "AttributeParentClass")?);
    }
    // Step 2: the categories, before any definition loads.
    for &c in &chain {
        let class = vm.space.heap.get(c)?.class;
        if vm.space.prop(class, "AttributeStartingValuesNew").is_some()
            && let Value::Array(items) = read(vm, c, "AttributeStartingValuesNew")?
            && !items.is_empty()
        {
            return Err(NotYet::not_read(CATEGORIES, RECORD).into());
        }
    }
    // Step 3: the entries, parents first, then the archetype's own list.
    let mut found: Vec<(Option<ObjRef>, InitData)> = Vec::new();
    for &c in chain.iter().rev() {
        found.extend(entries(vm, c)?);
    }
    if vm.space.prop(archetype_class, "AttributeStartingValues").is_some() {
        want_struct_array(vm, archetype_class, "AttributeStartingValues")?;
        found.extend(entries(vm, archetype)?);
    }
    // Step 4: what SetBaseValue's preflight walks, for each entry: the preflight's own walk
    // (`AttributeSystem::walk_set_base_value`), loading each unloaded object before its fields are read.
    let attrs = vm.attrs.clone();
    for (attribute, data) in found {
        let mut lines = Vec::new();
        let r = {
            let mut visit = |cx: &mut EvalCx<'_>, o: ObjRef| -> Result<bool, AttrError> {
                let WorldParts { space, defs, .. } = cx.world.parts();
                Ok(match defs {
                    Some(defs) => preload_in(space, defs, o, &mut lines, &mut loaded),
                    None => true,
                })
            };
            attrs.walk_set_base_value(attribute, &data, &mut EvalCx::new(vm), &mut visit)
        };
        vm.log.extend(lines);
        r.map_err(VmError::from)?;
    }
    Ok(loaded)
}

/// The harness line of the set-up's throwaway copy of `archetype` (never placed, freed after).
pub fn copy_line(archetype: &str) -> String {
    format!("harness input: set-up copy of {archetype} to read its starting values; UNVERIFIED")
}

/// The preload for the archetype at `archetype_path` in `package` (a copy of its export, as the spawn makes one,
/// freed after); None when the store holds no such package or the package no such export.
pub fn preload_archetype(vm: &mut Vm, package: &str, archetype_path: &str) -> Result<Option<usize>, VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(None);
    }
    // A package the world's store does not hold (a toy world): nothing to spawn, nothing to preload.
    let Ok(name) = ow_core::PackageName::parse(package) else { return Ok(None) };
    if vm.defs.code_mut().1.load(&name).is_err() {
        return Ok(None);
    }
    let Some(copy) = crate::hit::instantiate_copy(vm, package, archetype_path)? else { return Ok(None) };
    vm.log.push(copy_line(archetype_path));
    let r = preload_from(vm, copy);
    vm.space.free(copy).map_err(VmError::from)?;
    r.map(Some)
}

/// The preload for every AI pawn archetype the run can spawn ([`crate::run::KINDS`] with a pawn balance), at the combat
/// world's set-up; the number of objects loaded.
pub fn preload_kinds(vm: &mut Vm) -> Result<usize, VmError> {
    let mut loaded = 0;
    for (_, package, archetype, balance) in crate::run::KINDS {
        if balance.is_empty() {
            continue;
        }
        loaded += preload_archetype(vm, package, archetype)?.unwrap_or(0);
    }
    Ok(loaded)
}

#[cfg(test)]
#[path = "starting_values_tests.rs"]
mod tests;
