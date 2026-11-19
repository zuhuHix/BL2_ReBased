//! Owner: lane FIX-PAWN-DATA-B. AI-assisted (Claude Code). The player pawn data manager:
//! `WillowGlobals.GetPlayerPawnDataManager` and `WillowPlayerPawnDataManager.FindAlreadyLoadedObject`, implemented
//! from NATIVE_PAWN_DATA.md alone (decision 2026-11-19-24). Every rule is the record's reading of the executable,
//! UNVERIFIED in the running game.
//!
//! **GetPlayerPawnDataManager.** A plain read of the globals' `ThePlayerPawnDataManager`: it creates nothing. The game
//! fills that field in a globals set-up routine whose call site was not found (the note's open question 1). It is
//! expected to run before any controller script does. So a field that is still None here means the port did not run
//! that set-up, not a game state: `NotYet { NotRead }`, naming the question.
//!
//! **The set-up step** ([`set_up`]). This is what the note reads of that routine, for this field only. When the field is
//! None:
//! 1. an existing `WillowPlayerPawnDataManager` that an engine check accepts would be taken. The check is not read
//!    (open question 2), so an existing one is `NotYet { NotRead }`;
//! 2. otherwise a new one is made from its class defaults, with no name, and stored. The record reads its outer as
//!    the engine's transient package (UNVERIFIED, open question 2). The port has no such package, so it has no outer.
//!    Nothing frees run-time objects here, so the root set needs no counterpart.
//!
//! The port calls it where it makes its globals object (`--combat-run`'s bridge), before the player's script runs.
//!
//! **FindAlreadyLoadedObject(Path).** The object already in memory at the full path `Path`, any class, or None. It never
//! loads. In the port, "in memory" is an instance live in the heap (a run-time object or a loaded definition) whose
//! full path name equals `Path`, in any case, as names compare. A name match whose full path the port cannot name (a
//! run-time object with no outer) is skipped. Then:
//! - a match: it;
//! - no match, and an installed package holds the path (asked of the store's export tables, loading nothing): a store
//!   handle not loaded, or a path nothing has named yet. That is `NotYet { NotRead }`, as is a candidate file the
//!   store could not read. Whether the game has that object in memory depends on the `LoadPlayerPawnData` family,
//!   which is not read; the note leaves this to the port and warns that None would run `BodySelectClass(None)`, which
//!   the game is not expected to do;
//! - no match and no holder anywhere (or a string that is no object path): None;
//! - a path with no `.` (not a full path): `NotYet { NotRead }` (open question 3);
//! - an empty string: `NotYet { NotRead }`. The note reads it as a lookup of an empty name, without saying what that
//!   finds. On the maya path an empty path means the controller's `PlayerClass` is None, so the script read
//!   `PlayerClass.PawnArchetypePath` through None, which the game, with a player class set, does not do.
//!
//! **Only in `ParityMode::Off`.** The C++ reference has no stand-in for either native, so under Reference both stay
//! unbound and the stub policy answers them (architecture R3).

use std::rc::Rc;

use ow_core::{NotYet, ParityMode};
use ow_object::ids::ObjRef;
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativePack, Vm};
use ow_vm::error::VmError;
use ow_vm::interp::instantiate_from_defaults;
use ow_vm::path_name::path_name;

use crate::actors::{object, set_field};

/// The record these natives follow.
pub const RECORD: &str = "docs/verification/NATIVE_PAWN_DATA.md";
/// The natives this pack binds, by registry key.
pub const GET_MANAGER: &str = "WillowGlobals.GetPlayerPawnDataManager()";
pub const FIND_LOADED: &str = "WillowPlayerPawnDataManager.FindAlreadyLoadedObject(string)";
/// The globals' field (declared `WillowPlayerPawnDataManager` in the installed package; the manager's class is read
/// from that declaration).
const FIELD: &str = "ThePlayerPawnDataManager";

/// Binds the two natives, outside Reference only (module documentation).
#[derive(Debug, Clone, Copy, Default)]
pub struct Pack;

impl NativePack for Pack {
    fn register(&self, vm: &mut Vm) -> Result<(), VmError> {
        if vm.parity != ParityMode::Reference {
            vm.register_native(GET_MANAGER, Rc::new(get_manager))?;
            vm.register_native(FIND_LOADED, Rc::new(find_loaded))?;
        }
        Ok(())
    }
}

/// The globals set-up step for `ThePlayerPawnDataManager` on `globals` (module documentation). A field already set is
/// kept. Returns the manager the field holds afterwards.
pub fn set_up(vm: &mut Vm, globals: ObjRef) -> Result<ObjRef, VmError> {
    if let Some(m) = object(&vm.space, globals, FIELD)? {
        return Ok(m);
    }
    let class = manager_class(vm, globals)?;
    if vm.space.live(class).next().is_some() {
        return Err(NotYet::not_read(
            "the globals set-up of ThePlayerPawnDataManager with a WillowPlayerPawnDataManager already alive (the \
             engine check that picks it; NATIVE_PAWN_DATA.md open question 2)",
            RECORD,
        )
        .into());
    }
    let manager = instantiate_from_defaults(vm, class)?;
    set_field(&mut vm.space, globals, FIELD, Value::Object(Some(manager)))?;
    Ok(manager)
}

// WillowGlobals.GetPlayerPawnDataManager(): the field as it is; None is the set-up not run (module documentation).
fn get_manager(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let Some(globals) = c.this else {
        return Ok(Value::Object(None));
    };
    match object(&c.vm.space, globals, FIELD)? {
        Some(m) => Ok(Value::Object(Some(m))),
        None => Err(NotYet::not_read(
            "WillowGlobals.GetPlayerPawnDataManager before the globals set-up filled ThePlayerPawnDataManager (when \
             that set-up runs; NATIVE_PAWN_DATA.md open question 1)",
            RECORD,
        )
        .into()),
    }
}

// WillowPlayerPawnDataManager.FindAlreadyLoadedObject(Path) (module documentation).
fn find_loaded(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let path = match c.arg(0)? {
        Value::Str(s) => s.to_string(),
        other => {
            return Err(VmError::Script {
                function: FIND_LOADED.to_string(),
                message: format!("expected a string path, got {}", other.kind_name()),
            });
        }
    };
    if path.is_empty() {
        return Err(NotYet::not_read(
            "FindAlreadyLoadedObject with an empty path (what a lookup of an empty name finds is not read; on the \
             maya path it means the controller has no PlayerClass)",
            RECORD,
        )
        .into());
    }
    if !path.contains('.') {
        return Err(NotYet::not_read(
            format!(
                "FindAlreadyLoadedObject with a partial path {path:?} (NATIVE_PAWN_DATA.md open question 3)"
            ),
            RECORD,
        )
        .into());
    }
    // Every class: the root of the manager's own class chain (`Core.Object` in the installed packages).
    let Some(me) = c.this else {
        return Ok(Value::Object(None));
    };
    let mut base = c.vm.space.heap.get(me)?.class;
    while let Some(up) = c.vm.space.classes.layout(base).super_class() {
        base = up;
    }
    // Only instances (run-time objects and loaded definitions) are live; the store's unloaded handles are not. Only
    // objects whose own name is the path's last part can match, and their full path is built only then. A name match
    // whose full path the port cannot name (a run-time object with no outer) is skipped, but remembered: the game may
    // hold it at a path the port cannot build, so it turns a final None into `NotYet` (review r2 of #411).
    let leaf = path.rsplit(['.', ':']).next().unwrap_or_default();
    let candidates: Vec<ObjRef> = c
        .vm
        .space
        .live(base)
        .filter(|o| {
            c.vm.space.heap.get(*o).is_ok_and(|x| c.vm.space.names.text(x.name).eq_ignore_ascii_case(leaf))
        })
        .collect();
    let mut unnamed = false;
    for o in candidates {
        match path_name(c.vm, o) {
            Ok(full) if full.eq_ignore_ascii_case(&path) => return Ok(Value::Object(Some(o))),
            Ok(_) => continue,
            Err(VmError::NotYet(_)) => {
                unnamed = true;
                continue;
            }
            Err(e) => return Err(e),
        }
    }
    // Nothing in memory at that path. Whether an installed package holds it is asked of the store's export tables,
    // which loads no definition: a holder (a handle not loaded, or a path nothing has named yet) is not proof that the
    // game lacks the object, so that answer is `NotYet`. So is a candidate file the store could not read.
    let holders = match ow_core::ObjectPath::parse(&path) {
        Ok(parsed) => Some(c.vm.defs.owners(&parsed).map_err(VmError::Def)?),
        // A string that is no object path names nothing any package holds.
        Err(_) => None,
    };
    if let Some(h) = holders
        && (!h.holders.is_empty() || !h.failures.is_empty())
    {
        return Err(NotYet::not_read(
            format!(
                "FindAlreadyLoadedObject of {path}, which an installed package holds but nothing has loaded (whether \
                 the game holds it in memory: the LoadPlayerPawnData family, not read)"
            ),
            RECORD,
        )
        .into());
    }
    if unnamed {
        return Err(NotYet::not_read(
            format!(
                "FindAlreadyLoadedObject of {path}: a run-time object named {leaf} has no full path the port can \
                 name, so whether it is the object at that path is not known"
            ),
            RECORD,
        )
        .into());
    }
    Ok(Value::Object(None))
}

/// The class `globals`' `ThePlayerPawnDataManager` is declared with.
fn manager_class(vm: &Vm, globals: ObjRef) -> Result<ow_object::ids::ClassId, VmError> {
    let class = vm.space.heap.get(globals)?.class;
    let kind = vm.space.prop(class, FIELD).and_then(|p| vm.space.classes.decl(p)).map(|d| d.kind.clone());
    match kind {
        Some(PropKind::Object { class }) => Ok(class),
        _ => Err(VmError::Script {
            function: GET_MANAGER.to_string(),
            message: format!("the globals object has no object property {FIELD}"),
        }),
    }
}
