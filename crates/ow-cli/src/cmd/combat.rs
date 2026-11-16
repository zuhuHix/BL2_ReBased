//! Owner: lane W6-COMBAT. Commands: `--combat-run`, the port of its branch in `src/cli.cpp`. AI-assisted (Claude
//! Code).
//!
//! `<package> --combat-run --cooked <dir> <step>...` spawns pawns from their stock archetypes on a world over the
//! cooked folder, applies the steps and prints the JSON report of `ow_combat::run` (one line), exit 0 when no step
//! failed and 1 when one did. The source package loads first, as the C++ loads it before it reads the mode; the
//! package argument is any package (the archetypes name their own). Errors before the run print `ow-package:
//! <message>` with exit 1; a `NotYet` (a run with a `maya` step, which needs W6-MISSION's bridge) exits 3.
//!
//! **The world.** A definition store over the cooked folder (first holder in scan order), the standard attribute
//! system, the Core natives, and `World::new` with the combat pack (`ow_combat::hit::Pack`) after the world's own
//! packs. Its balance state is the first playthrough, no OP level, one player, level and stage 1 (the C++ run has no
//! playthrough input and reads the first playthrough's rules).
//!
//! **The stub policy** (architecture section 3.8). `ParityMode::Off`: `Refuse` with nothing allowed, so a native the
//! chain reaches that nothing binds stops the step with the VM's `UnboundNative` text. `ParityMode::Reference`:
//! `LogAndZero`, which is what the C++ run does with every unbound native (logged as `UNIMPLEMENTED <key>`, counted in
//! the report's `stubs`, a zero result): the reference stand-in, so that a real-package run compares with the C++.
//!
//! `mode` is the run's parity mode, read once by `main`; it goes to the definition store and the VM.

use std::io::Write as _;
use std::path::Path;
use std::process::ExitCode;
use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_balance::types::{
    AwesomeLevel, BalanceSnapshot, ExpLevel, GameStage, LevelCapUpgrades, OpLevel, PlayerCount,
    Playthrough,
};
use ow_core::ParityMode;
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_package::package::Package;
use ow_vm::api::{NativePack, StubPolicy, Vm};
use ow_vm::error::VmError;
use ow_vm::interp::on_vm_stack;
use ow_world::world::{BalanceState, World};

use crate::args::{Command, USAGE};
use crate::cmd::NOT_PORTED;
use crate::cmd::properties::error_text;

/// Why the command failed before or around the run.
enum Failure {
    Message(String),
    NotYet(String),
}

fn vm_failure(e: VmError) -> Failure {
    match e {
        VmError::NotYet(n) => Failure::NotYet(n.to_string()),
        other => Failure::Message(ow_combat::hit::access::error_text(&other)),
    }
}

pub fn run(_command: &Command, args: &[String], mode: ParityMode) -> ExitCode {
    let args = args.to_vec();
    let outcome = on_vm_stack(move || execute(&args, mode));
    let result = match outcome {
        Ok(r) => r,
        Err(e) => Err(Failure::Message(e.to_string())),
    };
    match result {
        Ok((text, failed)) => {
            // Ignore a failed write (a closed pipe), as the C++ binary does.
            let _ = writeln!(std::io::stdout().lock(), "{text}");
            ExitCode::from(if failed { 1 } else { 0 })
        }
        Err(Failure::Message(message)) => {
            eprintln!("ow-package: {message}");
            ExitCode::from(1)
        }
        Err(Failure::NotYet(message)) => {
            eprintln!("ow-package (Rust): {message}");
            ExitCode::from(NOT_PORTED)
        }
    }
}

/// The world of a run over `cooked` (module documentation).
pub fn world(cooked: &Path, mode: ParityMode) -> Result<World, VmError> {
    let defs = DefStore::open(cooked.to_path_buf(), IdentityPolicy::FirstInScanOrder, mode)
        .map_err(VmError::Def)?;
    let attrs = AttributeSystem::standard().map_err(VmError::Attr)?;
    let policy = match mode {
        ParityMode::Off => StubPolicy::refuse_all(),
        ParityMode::Reference => StubPolicy::LogAndZero,
    };
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(attrs), mode).with_stub_policy(policy);
    ow_vm::natives_core::register(&mut vm)?;
    let snapshot = BalanceSnapshot::new(
        Playthrough::NORMAL,
        OpLevel::new(0).map_err(|e| VmError::Script {
            function: "combat-run".into(),
            message: e.to_string(),
        })?,
        PlayerCount::SINGLE,
        ExpLevel::new(1),
        GameStage(1),
        AwesomeLevel::new(0),
        LevelCapUpgrades::ALL,
    )
    .map_err(|e| VmError::Script {
        function: "combat-run".into(),
        message: e.to_string(),
    })?;
    let pack = ow_combat::hit::Pack;
    let packs: [&dyn NativePack; 1] = [&pack];
    World::new(vm, BalanceState::new(snapshot), &packs).map_err(|e| match e {
        ow_world::world::WorldError::Vm(v) => v,
        other => VmError::Script {
            function: "combat-run".into(),
            message: other.to_string(),
        },
    })
}

fn execute(args: &[String], mode: ParityMode) -> Result<(String, bool), Failure> {
    let source = args
        .first()
        .ok_or_else(|| Failure::Message(USAGE.to_string()))?;
    Package::load(Path::new(source)).map_err(|e| Failure::Message(error_text(&e)))?;
    if args.len() < 4 || args[2] != "--cooked" {
        return Err(Failure::Message(USAGE.to_string()));
    }
    let steps = &args[4..];
    if let Some(refused) = ow_combat::run::needs_bridge(steps) {
        return Err(vm_failure(refused));
    }
    let world = world(Path::new(&args[3]), mode).map_err(vm_failure)?;
    ow_combat::run::combat_run(world, steps).map_err(vm_failure)
}

#[cfg(test)]
mod tests;
