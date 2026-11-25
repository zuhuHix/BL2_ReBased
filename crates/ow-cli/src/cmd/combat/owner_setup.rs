//! Link 1's set-up for the special-move blend's owner test (lane ROUTE-A; NATIVE_SPECIALMOVE_OWNER.md section 4.1,
//! route (a)). A **harness input**, not a game rule, run inside the `maya` step so the link's harness list shows it:
//!
//! 1. `GearboxFramework.SpecialMoveInterface` is loaded ([`crate::cmd::slice_gate::preload_class`]: an UNVERIFIED
//!    stand-in for the game having it loaded);
//! 2. the interface entries on the class chain of Maya's pawn class (the pawn archetype's class) are resolved
//!    ([`owner_tables`]), so the owner test (`ow_world::world::anim_node_init::owner_implements`, unchanged) answers
//!    from resolved entries instead of O5.
//!
//! The owner rule itself still loads and resolves nothing. AI-assisted (Claude Code).

use ow_core::NotYet;
use ow_object::ids::ClassId;
use ow_vm::api::Vm;
use ow_vm::error::VmError;
use ow_world::world::anim_node_init::{OWNER_RECORD, SPECIAL_MOVE_INTERFACE};

/// The text that starts [`owner_tables`]'s line.
pub const OWNER_TABLES_INPUT: &str = "harness input: set-up interface tables";

/// The owner test's set-up (module documentation): both lines are pushed onto the log, whatever the outcome; the
/// first failure stops it. `pawn` is the pawn archetype's class, `None` when no archetype was loaded.
pub fn special_move_owner_setup(vm: &mut Vm, pawn: Option<ClassId>) -> Result<(), VmError> {
    let (line, outcome) = crate::cmd::slice_gate::preload_class(vm, SPECIAL_MOVE_INTERFACE);
    vm.log.push(line);
    outcome?;
    let Some(pawn) = pawn else {
        vm.log.push(format!("{OWNER_TABLES_INPUT} skipped (no pawn archetype loaded)"));
        return Ok(());
    };
    let (line, outcome) = owner_tables(vm, pawn);
    vm.log.push(line);
    outcome
}

/// Resolves every interface entry on `pawn`'s class chain ([`Vm::resolve_interface_tables`], a load) and says where
/// the owner test's answer will come from:
/// - **interface not loaded, or a class without a code key, or a chain whose bodies did not all decode:** nothing is
///   resolved, and the owner test then answers that case itself (O1, O3 or O3', O4). No structural fallback is taken
///   here, as the owner test takes none;
/// - **read:** the answer (yes or no) is the packages' interface lists, each entry resolved to its class.
///   Resolving them at set-up rather than when the game first asks is the harness input (UNVERIFIED stand-in);
/// - **unresolved:** `Vm::resolve_interface_tables` keeps an entry whose lookup failed as "resolved to nothing", which
///   the owner test would read as a settled answer (O7). So the answer is checked against the strict
///   `CodeIndex::implements_declared` (as review r1 of #665 asked of its own set-up): when that has no answer (an entry
///   resolves to no class, an incomplete chain), fails, or disagrees with the resolved entries, the line says so and
///   the set-up answers `NotYet` (`NotRead`).
///
/// Every load the step makes (the resolve, the strict read) is a write, which the line names.
pub fn owner_tables(vm: &mut Vm, pawn: ClassId) -> (String, Result<(), VmError>) {
    let class = vm.space.classes.get_layout(pawn).map_or_else(|| format!("{pawn:?}"), |l| l.path().to_string());
    let head = format!("{OWNER_TABLES_INPUT} of {class} for {SPECIAL_MOVE_INTERFACE}");
    let Some(iface) = vm.space.classes.by_path(SPECIAL_MOVE_INTERFACE) else {
        return (format!("{head}: the interface is not loaded, nothing resolved"), Ok(()));
    };
    let (ck, ik, decoded) = {
        let code = vm.defs.code();
        let (Some(ck), Some(ik)) = (code.key_of(pawn), code.key_of(iface)) else {
            return (format!("{head}: no code key, nothing resolved"), Ok(()));
        };
        (ck, ik, code.class(ik).body_decoded() && code.super_chain(ck).all(|k| code.class(k).body_decoded()))
    };
    if !decoded {
        return (
            format!("{head}: the class bodies did not all decode, nothing resolved (no structural fallback is taken)"),
            Ok(()),
        );
    }
    let loads = vm.defs.load_transitions();
    let resolved = vm.resolve_interface_tables(pawn);
    let strict = {
        let (code, store) = vm.defs.code_mut();
        code.implements_declared(store, ck, ik)
    };
    let read = vm.defs.code().implements_resolved_chain(ck, ik);
    let loaded = if vm.defs.load_transitions() != loads { " (the lookups loaded code: a write)" } else { "" };
    let entries = match resolved {
        0 => "every entry already resolved".to_string(),
        1 => "1 entry resolved at set-up (a load; UNVERIFIED stand-in for the game having it resolved)".to_string(),
        n => format!("{n} entries resolved at set-up (a load; UNVERIFIED stand-in for the game having them resolved)"),
    };
    match (strict, read) {
        (Ok(Some(answer)), Ok(read)) if answer == read => {
            let yes_no = if answer { "yes" } else { "no" };
            (format!("{head}{loaded}: {yes_no}, read from the packages' interface lists; {entries}"), Ok(()))
        }
        (strict, read) => {
            let why = match (&strict, &read) {
                (Err(e), _) => format!("an entry's lookup failed ({e})"),
                (Ok(None), _) => "an entry did not resolve to a class, or the chain is incomplete".to_string(),
                (_, Err(_)) => "an entry is still unresolved".to_string(),
                (Ok(Some(_)), Ok(_)) => "the strict reading disagrees with the resolved entries".to_string(),
            };
            let line = format!("{head}{loaded}: unresolved, {why}; no answer from the packages' interface lists");
            let what = format!("the set-up interface tables of {class} for {SPECIAL_MOVE_INTERFACE}: {why}");
            (line, Err(NotYet::not_read(what, OWNER_RECORD).into()))
        }
    }
}

#[cfg(test)]
#[path = "owner_setup_tests.rs"]
mod tests;
