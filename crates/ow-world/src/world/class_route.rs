//! Owner: lane FIX-PLAYER-CLASS (decision 2026-11-19-41). AI-assisted (Claude Code). The two natives on the script route
//! of the player's class, implemented from NATIVE_PLAYER_CLASS.md alone, in `ParityMode::Off` only (registered by
//! [`super::pawn_data::Pack`]). Every rule is the note's reading of the executable, UNVERIFIED in the running game.
//!
//! **`GBXDefinition.GetDefinition(DefinitionClass, DefinitionName)`** (section 2):
//! - no class, or an empty name: None;
//! - a name with no `.` (a bare object name): `NotYet { NotRead }`. The note's lookup runs "among the objects already
//!   loaded in any package"; how a bare name resolves there (which of several matches wins) is not read, and the
//!   definition store's own resolution of a bare name is the port's identity policy, not the engine's;
//! - otherwise an object of that class whose full path is that name, among the objects already in memory (the port's
//!   heap: run-time objects and loaded definitions), names compared in any case;
//! - failing that, the object is loaded by that name: in the port, the definition store's load of that path. That a
//!   store load is the engine's "load by name" is this lane's reading (the note leaves it to the implementer),
//!   UNVERIFIED;
//! - failing that, the note's retry (something named from a fixed text is loaded, then the load is tried again) is
//!   `NotYet { NotRead }`: its text is not read.
//!
//! What "an object of that class" means for a subclass (the lookup's class filter) is not read, so an object found
//! at the path whose class is only derived from the requested one is `NotYet`, and so is one of an unrelated class.
//! A name match in memory whose full path the port cannot name is `NotYet` too, before any load.
//!
//! **The class is checked before the load** (review of #431). A load by name first asks the store for the class of
//! the export its identity policy would pick (`DefStore::holder_class`, which loads that class's code but not the
//! export), and compares it with the requested class by class, not by path text. No holder: the retry's `NotYet`. A
//! different class, or a requested class with no package class to compare: `NotYet` (the lookup's class filter is not
//! read), and nothing is loaded. So a refused call leaves no definition live that a later `FindAlreadyLoadedObject`
//! could find, and a duplicate holder the policy does not pick does not refuse the lookup.
//!
//! **`WillowPlayerPawnDataManager.LoadPlayerPawnDataAsync(PawnDataPath, PlayerContext, Payload, Instigator, IntParam,
//! FloatParam, CallbackCustomEventName)`** (section 3):
//! 1. a path with no `.` has no package part. The note makes that a no-op only after the lookup misses, and the
//!    lookup's rule for a partial path is not read (NATIVE_PAWN_DATA.md open question 3), so a miss cannot be
//!    established: `NotYet { NotRead }` (review of #431);
//! 2. the object already in memory at the path is looked up with `FindAlreadyLoadedObject`'s routine
//!    ([`super::pawn_data::lookup`]);
//! 3. found: the report is sent at once. When `PlayerContext` implements `IStreamingDataEvent`, its script
//!    `RunStreamingDataEvent` runs with `CallbackCustomEventName`, `Payload`, the found object, `IntParam` and
//!    `FloatParam` (`Instigator` is not passed). A context that does not implement it gets no report. The VM's
//!    `implements` cannot prove a "no" outside `ow-vm` (decision 2026-11-15-20), so a "no" is `NotYet`;
//! 4. not found (or not known to be found), with a package part: `NotYet { NotRead }`. Whether the object is then
//!    loaded at once (an unidentified engine flag) or queued for an asynchronous load, and when that report comes,
//!    is not read.
//!
//! The report's argument order is the note's (name, payload as the instigator, the found object as the other
//! participant, int, float); it is checked against the declared parameter kinds before the call, and a different
//! shape is `NotYet`.
//!
//! **Reference: unbound.** The C++ reference has no stand-in for either native (architecture R3).

use std::rc::Rc;

use ow_core::{NotYet, ObjectPath};
use ow_object::ids::{ClassId, ObjRef};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;
use ow_vm::path_name::path_name;

/// The record these natives follow.
pub const RECORD: &str = "docs/verification/NATIVE_PLAYER_CLASS.md";
/// The natives this module binds, by registry key.
pub const GET_DEFINITION: &str = "GBXDefinition.GetDefinition(class,string)";
pub const LOAD_ASYNC: &str = "WillowPlayerPawnDataManager.LoadPlayerPawnDataAsync(string,object,object,object,int,float,name)";
/// Where the report goes: the interface as the installed `GearboxFramework` package declares it (with its one
/// function `RunStreamingDataEvent`; read from the package, not from native code).
pub const STREAMING_INTERFACE: &str = "GearboxFramework.IStreamingDataEvent";
const REPORT: &str = "RunStreamingDataEvent";

/// Binds both natives (the caller binds them outside Reference only).
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    vm.register_native(GET_DEFINITION, Rc::new(get_definition))?;
    vm.register_native(LOAD_ASYNC, Rc::new(load_async))
}

fn not_read(what: impl Into<String>) -> VmError {
    NotYet::not_read(what.into(), RECORD).into()
}

fn bad(native: &str, message: impl Into<String>) -> VmError {
    VmError::Script {
        function: native.to_string(),
        message: message.into(),
    }
}

fn text_arg(c: &NativeCall<'_>, i: usize, native: &str) -> Result<String, VmError> {
    match c.arg(i)? {
        Value::Str(s) => Ok(s.to_string()),
        other => Err(bad(
            native,
            format!("argument {i}: expected a string, got {}", other.kind_name()),
        )),
    }
}

fn object_arg(c: &NativeCall<'_>, i: usize) -> Result<Option<ObjRef>, VmError> {
    if !c.has(i) {
        return Ok(None);
    }
    match c.arg(i)? {
        Value::Object(o) | Value::Interface(o) => Ok(*o),
        other => Err(bad(
            LOAD_ASYNC,
            format!(
                "argument {i}: expected an object, got {}",
                other.kind_name()
            ),
        )),
    }
}

// GBXDefinition.GetDefinition(DefinitionClass, DefinitionName) (module documentation).
fn get_definition(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let class = match c.arg(0)? {
        Value::Class(k) => *k,
        other => {
            return Err(bad(
                GET_DEFINITION,
                format!("expected a class, got {}", other.kind_name()),
            ));
        }
    };
    let name = text_arg(c, 1, GET_DEFINITION)?;
    let Some(class) = class else {
        return Ok(Value::Object(None));
    };
    if name.is_empty() {
        return Ok(Value::Object(None));
    }
    if !name.contains('.') {
        return Err(not_read(format!(
            "GBXDefinition.GetDefinition of the bare name {name:?} (how the lookup in any package resolves a name with              no package part)"
        )));
    }
    // In memory: a live object at that full path.
    if let Some(o) = in_memory(c.vm, &name, class)? {
        return checked(c.vm, o, class, &name);
    }
    // Loaded by that name: the definition store's load of the path, once its holders' class settles the filter.
    let loaded = match ObjectPath::parse(&name) {
        Ok(path) => {
            // The holder the store's identity policy picks, by its class's key, without loading the export (review
            // of #431): another holder of a different class is not the one the load would take, and a class named
            // under an outer compares as the class itself, not as a path string.
            let held = c.vm.defs.holder_class(&mut c.vm.space, &path);
            let log = c.vm.defs.take_log();
            c.vm.log.extend(log);
            let held = held.map_err(VmError::Def)?;
            let wanted = c.vm.defs.code_mut().0.key_of(class);
            match (held, wanted) {
                (None, _) => {
                    return Err(not_read(format!(
                        "GBXDefinition.GetDefinition of {name:?}, which neither memory nor a load by name holds (the retry's fixed text; NATIVE_PLAYER_CLASS.md, Not read)"
                    )));
                }
                (Some(_), None) => {
                    return Err(not_read(format!(
                        "GBXDefinition.GetDefinition of {name:?}: the requested class {} has no package class to compare with, so the lookup's class filter cannot be settled",
                        class_name(c.vm, class)
                    )));
                }
                (Some(held), Some(wanted)) if held != wanted => {
                    return Err(not_read(format!(
                        "GBXDefinition.GetDefinition of {name:?}: the export the load would take is of another class than the requested {} (the lookup's class filter is not read; nothing was loaded)",
                        class_name(c.vm, class)
                    )));
                }
                _ => {}
            }
            let found = c.vm.defs.find(&mut c.vm.space, &path);
            let log = c.vm.defs.take_log();
            c.vm.log.extend(log);
            found.map_err(VmError::Def)?
        }
        Err(_) => None,
    };
    match loaded {
        Some(o) => checked(c.vm, o, class, &name),
        None => Err(not_read(format!(
            "GBXDefinition.GetDefinition of {name:?}, which neither memory nor a load by name holds (the retry's fixed \
             text; NATIVE_PLAYER_CLASS.md, Not read)"
        ))),
    }
}

/// `o` when its class is `class`; a derived or unrelated class is `NotYet` (the class filter is not read).
fn checked(vm: &Vm, o: ObjRef, class: ClassId, name: &str) -> Result<Value, VmError> {
    let found = vm.space.heap.get(o)?.class;
    if found == class {
        return Ok(Value::Object(Some(o)));
    }
    Err(not_read(format!(
        "GBXDefinition.GetDefinition of {name:?}: the object there is of class {}, not the requested {} (the lookup's \
         class filter is not read)",
        class_name(vm, found),
        class_name(vm, class),
    )))
}

fn class_name(vm: &Vm, class: ClassId) -> String {
    vm.space
        .classes
        .get_layout(class)
        .map_or_else(|| format!("{class:?}"), |l| l.path().as_str().to_string())
}

/// A live object whose full path is `path`, any case, among the instances of the root of `class`'s chain (every
/// class: the class check comes after). A name match the port cannot give a full path answers `NotYet`, before any load.
fn in_memory(vm: &mut Vm, path: &str, class: ClassId) -> Result<Option<ObjRef>, VmError> {
    let leaf = path.rsplit(['.', ':']).next().unwrap_or_default();
    let mut root = class;
    while let Some(up) = vm.space.classes.layout(root).super_class() {
        root = up;
    }
    let candidates: Vec<ObjRef> = vm
        .space
        .live(root)
        .filter(|o| {
            vm.space
                .heap
                .get(*o)
                .is_ok_and(|x| vm.space.names.text(x.name).eq_ignore_ascii_case(leaf))
        })
        .collect();
    // A name match whose full path the port cannot name (a run-time object with no outer) may be the object at that
    // path, so it answers `NotYet` before any store load (as `pawn_data::lookup` does; review r1 of #426).
    let mut unnamed = false;
    for o in candidates {
        match path_name(vm, o) {
            Ok(full) if full.eq_ignore_ascii_case(path) => return Ok(Some(o)),
            Ok(_) => continue,
            Err(VmError::NotYet(_)) => unnamed = true,
            Err(e) => return Err(e),
        }
    }
    if unnamed {
        let leaf = path.rsplit(['.', ':']).next().unwrap_or_default();
        return Err(not_read(format!(
            "GBXDefinition.GetDefinition of {path}: a run-time object named {leaf} has no full path the port can name, so whether it is the object at that path is not known"
        )));
    }
    Ok(None)
}

// WillowPlayerPawnDataManager.LoadPlayerPawnDataAsync(...) (module documentation).
fn load_async(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let path = text_arg(c, 0, LOAD_ASYNC)?;
    let context = object_arg(c, 1)?;
    let payload = object_arg(c, 2)?;
    // Instigator is checked for its kind like the others, and not passed to the report (section 3 step 3).
    object_arg(c, 3)?;
    let int_param = match c.arg(4)? {
        Value::Int(i) => *i,
        other => {
            return Err(bad(
                LOAD_ASYNC,
                format!("IntParam holds {}", other.kind_name()),
            ));
        }
    };
    let float_param = match c.arg(5)? {
        Value::Float(f) => *f,
        other => {
            return Err(bad(
                LOAD_ASYNC,
                format!("FloatParam holds {}", other.kind_name()),
            ));
        }
    };
    let event = match c.arg(6)? {
        Value::Name(n) => *n,
        other => {
            return Err(bad(
                LOAD_ASYNC,
                format!("CallbackCustomEventName holds {}", other.kind_name()),
            ));
        }
    };
    // 1. No package part: the lookup cannot establish a miss for a partial path (module documentation).
    if !path.contains('.') {
        return Err(not_read(format!(
            "LoadPlayerPawnDataAsync of the bare name {path:?} (the lookup's partial-path rule; NATIVE_PAWN_DATA.md open question 3), so whether it misses is not known"
        )));
    }
    let Some(me) = c.this else {
        return Err(bad(LOAD_ASYNC, "called without a manager"));
    };
    // 2. The object in memory, by FindAlreadyLoadedObject's routine. Its NotYet (a holder not loaded, a name it
    //    cannot place) and its None are both "not found" here, which is step 4.
    let found = match super::pawn_data::lookup(c.vm, me, &path) {
        Ok(found) => found,
        Err(VmError::NotYet(_)) => None,
        Err(e) => return Err(e),
    };
    let Some(found) = found else {
        return Err(not_read(format!(
            "LoadPlayerPawnDataAsync of {path}, not in memory (the engine flag that loads at once, and the queued \
             asynchronous load and its report; NATIVE_PLAYER_CLASS.md section 3 step 4)"
        )));
    };
    // 3. Found: the report at once, to a context that implements IStreamingDataEvent.
    let Some(context) = context else {
        return Ok(Value::None);
    };
    let Ok(iface) = c.vm.find_class(STREAMING_INTERFACE) else {
        return Err(bad(
            LOAD_ASYNC,
            format!("the report's interface {STREAMING_INTERFACE} is not declared"),
        ));
    };
    let class = c.vm.space.heap.get(context)?.class;
    if !c.vm.implements(class, iface) {
        return Err(not_read(
            "LoadPlayerPawnDataAsync's report to a context the VM finds not implementing IStreamingDataEvent (a \"no\" \
             is not proven outside ow-vm, decision 2026-11-15-20)",
        ));
    }
    let state = c.vm.space.heap.get(context)?.state;
    let Some(f) = c.vm.find_method(class, REPORT, state)? else {
        return Err(bad(
            LOAD_ASYNC,
            "the context implements IStreamingDataEvent but has no RunStreamingDataEvent",
        ));
    };
    let kinds: Vec<PropKind> =
        c.vm.function(f)
            .map(|d| d.params.iter().map(|p| p.kind.clone()).collect())
            .unwrap_or_default();
    let shape_ok = kinds.len() >= 5
        && matches!(kinds[0], PropKind::Name)
        && matches!(
            kinds[1],
            PropKind::Object { .. } | PropKind::Interface { .. }
        )
        && matches!(
            kinds[2],
            PropKind::Object { .. } | PropKind::Interface { .. }
        )
        && matches!(kinds[3], PropKind::Int)
        && matches!(kinds[4], PropKind::Float);
    if !shape_ok {
        return Err(not_read(format!(
            "LoadPlayerPawnDataAsync's report: RunStreamingDataEvent declares {kinds:?}, not (name, object, object, int, \
             float) as the note reads the report"
        )));
    }
    let mut args = vec![
        Value::Name(event),
        Value::Object(payload),
        Value::Object(Some(found)),
        Value::Int(int_param),
        Value::Float(float_param),
    ];
    for p in &kinds[5..] {
        args.push(c.vm.space.classes.zero_value(p));
    }
    c.vm.call(f, Some(context), args)?;
    Ok(Value::None)
}

#[cfg(test)]
mod tests;
