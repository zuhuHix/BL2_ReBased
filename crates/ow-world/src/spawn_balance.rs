//! Owner: lane IMPL-SETDEFAULTPHYSICS-AND-MIND-RECALC (decision 2026-11-26-NNN). The spawn's `BalanceDefinitionState`
//! step, a logged, UNVERIFIED harness input in `ParityMode::Off` (NATIVE_SET_DEFAULT_PHYSICS_AND_SPAWN_BALANCE.md
//! section 3, the hub's ruling on #788). AI-assisted (Claude Code).
//!
//! In the game the population factory runs `pawn.InitializeBalanceDefinitionState(PawnBalanceDefinition, -1)` after
//! the mind's class step and the attribute starting values, and before the mind's
//! `RecalculateAttributeInitializedState`. The port's spawn has no factory: `ow-combat`'s spawn picks the balance (the
//! harness input, logged there) and records it here; the step runs at the start of the mind's recalculation
//! ([`run_step`], called by [`crate::levelup`]), which is the place the factory's order gives it, since the port's
//! attribute starting values run inside the mind's class-defaults script just before that call.
//!
//! The step: no record, nothing; a recorded refusal (no balance matched, or more than one) is `NotYet { NotRead }`
//! with its text; a balance the store has not loaded is `NotRead` ([`NOT_LOADED`]); otherwise the pawn's installed
//! script `InitializeBalanceDefinitionState(balance, -1)` runs, and the record is spent. Refusals come before the
//! step's first write. Reference records nothing.

use ow_core::{DetMap, NotYet};
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

/// The record of the rule.
pub const RECORD: &str = "docs/verification/NATIVE_SET_DEFAULT_PHYSICS_AND_SPAWN_BALANCE.md (section 3)";
pub const NOT_LOADED: &str = "the spawn's balance step: a balance definition the store has not loaded";

/// Spawned pawn -> the balance its spawn picked, or why none was.
#[derive(Default)]
pub struct PendingBalances {
    map: DetMap<ObjRef, Result<ObjRef, String>>,
}

fn borrowed() -> VmError {
    VmError::ServiceBorrowed("PendingBalances")
}

/// Records `pick` for `pawn` (installs the service on first use).
pub fn record(vm: &mut Vm, pawn: ObjRef, pick: Result<ObjRef, String>) -> Result<(), VmError> {
    let s = match vm.service::<PendingBalances>() {
        Some(s) => s,
        None => vm.add_service(PendingBalances::default())?,
    };
    s.try_borrow_mut().map_err(|_| borrowed())?.map.insert(pawn, pick);
    Ok(())
}

/// What is recorded for `pawn`, if anything.
pub fn pending(vm: &Vm, pawn: ObjRef) -> Result<Option<Result<ObjRef, String>>, VmError> {
    match vm.service::<PendingBalances>() {
        Some(s) => Ok(s.try_borrow().map_err(|_| borrowed())?.map.get(&pawn).cloned()),
        None => Ok(None),
    }
}

/// The step for `mind`'s pawn (module documentation).
pub fn run_step(vm: &mut Vm, mind: ObjRef) -> Result<(), VmError> {
    let Some(pawn) = crate::actors::object(&vm.space, mind, "MyWillowPawn")? else {
        return Ok(());
    };
    let balance = match pending(vm, pawn)? {
        None => return Ok(()),
        Some(Err(why)) => return Err(NotYet::not_read(why, RECORD).into()),
        Some(Ok(b)) => b,
    };
    if vm.defs.is_unloaded(&vm.space, balance) || vm.defs.is_failed(&vm.space, balance) {
        return Err(NotYet::not_read(NOT_LOADED, RECORD).into());
    }
    let class = vm.space.heap.get(pawn)?.class;
    let Some(f) = vm.find_method(class, "InitializeBalanceDefinitionState", None)? else {
        return Err(VmError::Script {
            function: "the spawn's balance step".to_string(),
            message: "the pawn class has no InitializeBalanceDefinitionState".to_string(),
        });
    };
    if let Some(s) = vm.service::<PendingBalances>() {
        s.try_borrow_mut().map_err(|_| borrowed())?.map.remove(&pawn);
    }
    vm.call(f, Some(pawn), vec![Value::Object(Some(balance)), Value::Int(-1)])?;
    Ok(())
}
