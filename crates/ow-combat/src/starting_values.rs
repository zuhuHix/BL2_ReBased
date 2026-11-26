//! Owner: lane IMPL-SPAWN-SETBASEVALUE (decision 2026-11-26-72). The combat world's set-up preload of the attribute
//! starting values' definitions, in `ParityMode::Off`. AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from docs/verification/NATIVE_SPAWN_SETBASEVALUE.md section 3 (#811, fixed at 0b0a1e6) alone;
//! this lane did not read the executable. **UNVERIFIED in the running game**: the game keeps these definitions
//! resident; the port loads them at set-up, a logged harness input, so that `SetBaseValue`'s own preflight (which
//! refuses an unloaded handle, #572) finds them. The native is unchanged. Reference does nothing and logs nothing.
//!
//! **The walk**, for one AI pawn archetype (the store's own load of its export, logged ([`archetype_line`]); its
//! references stay handles, so nothing loads before the walk logs it, and nothing is copied; review of #815, items 2
//! and 3):
//! 1. its `AIClass`, then that class's `AttributeParentClass` chain. Each class object is loaded when it is a handle
//!    (logged), after its class's declarations are checked: `AttributeStartingValues` an array of structs,
//!    `AttributeParentClass` an object (else `VmError::Script`, nothing loaded). The chain ends at a None parent or at
//!    a class already seen, so a loop cannot run forever;
//! 2. a class whose `AttributeStartingValuesNew` is declared and non-empty is [`CATEGORIES`] (`NotRead`: the category
//!    entries' layout is not read), decided before any definition loads;
//! 3. the starting values, parents first, then the archetype's own list: each entry's `Attribute` (an object), and its
//!    `BaseValue` read with the evaluator's own data reader (`ow_attr::eval::init_data`): its `BaseValueAttribute` and
//!    `InitializationDefinition`;
//! 4. the closure `SetBaseValue`'s preflight walks, by that preflight's own walk (`AttributeSystem::walk_set_base_value`
//!    in `ow-attr`, the note's step 5, so the two cannot drift): the destination with its context resolvers and its
//!    value resolvers up to the first None; the base value's attribute and definition, followed into the nested data
//!    the evaluation reaches (the enabled formula's four terms, else the enabled conditional's reached entries and
//!    operands, the enabled variance bounds and range limits), to the preflight's depth. Each object is loaded before
//!    its fields are read. A field the preflight requires that is missing or of another kind is the preflight's own
//!    error here too (`VmError::Script` at set-up); the preflight's class checks are not made, so what they would
//!    refuse is left to the native.
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

/// An attribute-system error of a data read as a data error (`VmError::Script`, review of #815, item 4); a `NotYet`
/// keeps its kind.
fn script(e: ow_attr::api::AttrError) -> VmError {
    match VmError::from(e) {
        VmError::Attr(a) => bad(a.to_string()),
        other => other,
    }
}

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
    let path = defs.handle_path(o).unwrap_or_else(|| ow_attr::eval::class_path(space, o).to_string() + " object");
    if !defs.is_unloaded(space, o) {
        if defs.is_failed(space, o) {
            return false;
        }
        // A stand-in short of its class's slots (a missing import's) is not readable either (review r2 of #815, item
        // 3): skipped, logged, so the native's own refusal shows it.
        let short = space.heap.get(o).is_ok_and(|x| {
            space.classes.get_layout(x.class).is_some_and(|l| x.slots.len() < l.slot_count() as usize)
        });
        if short {
            lines.push(format!("harness input: preload {path}: skipped (a stand-in without its slots; the native refuses it)"));
            return false;
        }
        return true;
    }
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
        let attribute = match ow_attr::eval::field(&vm.space, entry, "Attribute").map_err(script)? {
            Value::Object(a) => *a,
            other => return Err(bad(format!("a starting value's Attribute holds {}, not an object", other.kind_name()))),
        };
        let base = ow_attr::eval::field(&vm.space, entry, "BaseValue").map_err(script)?.clone();
        let data = ow_attr::eval::init_data(&vm.space, &base).map_err(script)?;
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
        if let Some(p) = vm.space.prop(class, "AttributeStartingValuesNew") {
            // A tag the decoder skipped left the default in the slot: malformed, not an empty list (review r2 of #815,
            // item 4).
            if vm.defs.is_skipped(&vm.space, c, p) {
                return Err(bad("AttributeStartingValuesNew's tag did not decode (the decoder skipped it)"));
            }
            // Declared: an array, else a data error, before any definition loads (review of #815, item 4).
            match (declared(vm, class, "AttributeStartingValuesNew"), read(vm, c, "AttributeStartingValuesNew")?) {
                (Some(PropKind::Array(_)), Value::Array(items)) => {
                    if !items.is_empty() {
                        return Err(NotYet::not_read(CATEGORIES, RECORD).into());
                    }
                }
                (kind, _) => return Err(bad(format!("AttributeStartingValuesNew is declared as {kind:?}, not an array"))),
            }
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
        r.map_err(script)?;
    }
    Ok(loaded)
}

/// The harness line of the set-up's read of `archetype` (review of #815, items 2 and 3: the store's own load of the
/// export, no copy).
pub fn archetype_line(archetype: &str) -> String {
    format!("harness input: preload {archetype} (the spawn kind's archetype, read for its starting values; UNVERIFIED)")
}

/// The preload for the archetype at `archetype_path` (a full object path) of the spawn kind's `package` (the store's
/// load of the export, logged); None when the store holds no such package, or the archetype is not found or fails to
/// load (logged).
pub fn preload_archetype(vm: &mut Vm, package: &str, archetype_path: &str) -> Result<Option<usize>, VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(None);
    }
    // A package the world's store does not hold (a toy world): nothing to spawn, nothing to preload.
    let Ok(name) = ow_core::PackageName::parse(package) else { return Ok(None) };
    if vm.defs.code_mut().1.load(&name).is_err() {
        return Ok(None);
    }
    // The archetype as the store loads it (review of #815, items 2 and 3): its references stay handles, so the walk
    // below loads and logs each of them; nothing is copied, so nothing is left to free, and a second discovery finds it
    // loaded. A failed load is logged and left: the spawn's own copy and the native's refusal show it.
    // The export in the named package, as the spawn finds it (review r2 of #815, item 2), not the path's holder under
    // the identity policy: another package's copy of the path may name another AIClass. The store gives that export's
    // object only when it is the path's object; otherwise its refusal is logged and left.
    let found = {
        let (_, store) = vm.defs.code_mut();
        store.load(&name).ok().and_then(|id| store.get(id).find_export(archetype_path).ok().flatten().filter(|i| *i > 0).map(|i| (id, i)))
    };
    let Some((id, index)) = found else {
        vm.log.push(format!("harness input: preload {archetype_path}: no such export in {package}"));
        return Ok(None);
    };
    vm.log.push(archetype_line(archetype_path));
    let loaded = vm.defs.load_export(&mut vm.space, id, index, ow_object::prefix::PropertyPrefix::Ordinary);
    vm.log.extend(vm.defs.take_log());
    let archetype = match loaded {
        Ok(o) => o,
        Err(e) => {
            vm.log.push(format!("harness input: preload {archetype_path}: failed ({e})"));
            return Ok(None);
        }
    };
    preload_from(vm, archetype).map(Some)
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
