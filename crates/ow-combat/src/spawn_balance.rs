//! Owner: lane IMPL-SETDEFAULTPHYSICS-AND-MIND-RECALC (decision 2026-11-26-NNN). The spawn's pawn balance, a logged,
//! UNVERIFIED harness input in `ParityMode::Off` (NATIVE_SET_DEFAULT_PHYSICS_AND_SPAWN_BALANCE.md section 3, the hub's
//! ruling on #788). AI-assisted (Claude Code).
//!
//! In the game the population factory names the balance and the balance points at the archetype (`AIPawnArchetype`).
//! The port's spawn has no factory, so the harness finds the balance by that pointer: among the
//! `AIPawnBalanceDefinition` exports of the archetype's own package file, the one whose `AIPawnArchetype` is the
//! spawned archetype (compared by path). Exactly one must match; none, or more than one, is recorded as the step's
//! refusal naming the archetype. Each candidate is loaded through the store to read its pointer (a harness load at the
//! spawn). The pick is logged ("harness input: …") and recorded for `ow-world`'s step ([`ow_world::spawn_balance`]),
//! which runs it before the mind's recalculation. Which balance the game's own population picks was not observed.

use ow_core::{ObjectPath, PackageName};
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

/// The class the pick scans for.
pub const BALANCE_CLASS: &str = "AIPawnBalanceDefinition";

/// The log line of a pick.
pub fn pick_line(archetype: &str, balance: &str) -> String {
    format!(
        "harness input: the spawn's pawn balance for {archetype}: {balance} (UNVERIFIED: picked by its AIPawnArchetype; \
         in the game the population factory names it)"
    )
}

/// The candidates: the paths of `package`'s exports whose class is named [`BALANCE_CLASS`].
fn candidates(vm: &mut Vm, package: &str) -> Result<Vec<String>, VmError> {
    let Ok(name) = PackageName::parse(package) else { return Ok(Vec::new()) };
    let (_, store) = vm.defs.code_mut();
    let id = store.load(&name).map_err(|e| VmError::Script { function: "the spawn's balance pick".into(), message: e.to_string() })?;
    let pkg = store.get(id);
    let mut out = Vec::new();
    for index in 1..=pkg.exports.len() as i32 {
        let Ok(entry) = pkg.object(index) else { continue };
        let is_balance = entry.class != 0 && pkg.object(entry.class).is_ok_and(|c| c.name.eq_ignore_ascii_case(BALANCE_CLASS));
        if is_balance && let Ok(path) = pkg.path_of(index) {
            out.push(path);
        }
    }
    Ok(out)
}

/// The path of the object `o` names: the store's handle path, else its object path.
fn path_of(vm: &Vm, o: ObjRef) -> String {
    vm.defs.handle_path(o).unwrap_or_else(|| crate::hit::access::path_of(vm, Some(o)))
}

/// Picks and records the balance of the pawn spawned from `archetype` of `package` (module documentation).
pub fn pick(vm: &mut Vm, pawn: ObjRef, package: &str, archetype: &str) -> Result<(), VmError> {
    let mut matched = Vec::new();
    for candidate in candidates(vm, package)? {
        let Ok(path) = ObjectPath::parse(&candidate) else { continue };
        let Ok(Some(balance)) = vm.defs.find(&mut vm.space, &path) else { continue };
        let class = vm.space.heap.get(balance)?.class;
        let Some(p) = vm.space.prop(class, "AIPawnArchetype") else { continue };
        if let Value::Object(Some(a)) = vm.space.read(balance, ow_object::ids::PropAddr::scalar(p))?.clone()
            && path_of(vm, a).eq_ignore_ascii_case(archetype)
        {
            matched.push((balance, candidate));
        }
    }
    let record = match matched.as_slice() {
        [(balance, path)] => {
            vm.log.push(pick_line(archetype, path));
            Ok(*balance)
        }
        [] => Err(format!("the spawn's pawn balance: no {BALANCE_CLASS} of {package} points at {archetype}")),
        many => Err(format!(
            "the spawn's pawn balance: {} {BALANCE_CLASS}s of {package} point at {archetype}",
            many.len()
        )),
    };
    ow_world::spawn_balance::record(vm, pawn, record)
}
