//! Owner: lane W0-SCAFFOLD. One module per command family; each owner lane edits only its own file.

pub mod after_login_census;
pub mod assets;
pub mod behavior;
pub mod census;
pub mod classes;
pub mod combat;
pub mod defs;
pub mod inventory;
pub mod items;
pub mod items_pools;
pub mod items_roll;
pub mod items_stats;
pub mod kismet;
pub mod mission;
pub mod mesh_sockets;
pub mod mover;
pub mod objects;
pub mod properties;
pub mod resolve;
pub mod script;
pub mod slice;
pub mod slice_gate;
pub mod skeletal_source;
pub mod slice_pickup;
pub mod tables;
pub mod vm;

use std::process::ExitCode;

use ow_core::ParityMode;

use crate::args::{Command, Family};

/// The exit code of a command that met a `NotYet` (a rule, layer or native not ported or read yet). Distinct from the
/// C++ codes (0 success, 1 error). Every command is ported (decision 2026-11-16-38).
pub const NOT_PORTED: u8 = 3;

/// Runs `command`'s family with the whole command line (`args`, without the program name) and the parity mode
/// `main` read at start-up.
pub fn run(command: &'static Command, args: &[String], mode: ParityMode) -> ExitCode {
    match command.family {
        Family::Tables => tables::run(command, args, mode),
        Family::Properties => properties::run(command, args, mode),
        Family::Resolve => resolve::run(command, args, mode),
        Family::Script => script::run(command, args, mode),
        Family::Classes => classes::run(command, args, mode),
        Family::Assets => assets::run(command, args, mode),
        Family::Objects => objects::run(command, args, mode),
        Family::Defs => defs::run(command, args, mode),
        Family::Vm => vm::run(command, args, mode),
        Family::Census => census::run(command, args, mode),
        Family::Items => items::run(command, args, mode),
        Family::ItemsRoll => items_roll::run(command, args, mode),
        Family::ItemsStats => items_stats::run(command, args, mode),
        Family::ItemsPools => items_pools::run(command, args, mode),
        Family::Behavior => behavior::run(command, args, mode),
        Family::Kismet => kismet::run(command, args, mode),
        Family::Mover => mover::run(command, args, mode),
        Family::Inventory => inventory::run(command, args, mode),
        Family::Mission => mission::run(command, args, mode),
        Family::Slice => slice::run(command, args, mode),
        Family::Combat => combat::run(command, args, mode),
        Family::SliceGate => slice_gate::run(command, args, mode),
    }
}
