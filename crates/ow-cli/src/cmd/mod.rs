//! Owner: lane W0-SCAFFOLD. One module per command family; each owner lane edits only its own file.

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
pub mod mover;
pub mod objects;
pub mod properties;
pub mod resolve;
pub mod script;
pub mod slice;
pub mod tables;
pub mod vm;

use std::process::ExitCode;

use crate::args::{Command, Family};

/// The exit code of a command that is not ported to Rust yet. Distinct from the C++ codes (0 success, 1 error).
pub const NOT_PORTED: u8 = 3;

/// Runs `command`'s family with the whole command line (`args`, without the program name).
pub fn run(command: &'static Command, args: &[String]) -> ExitCode {
    match command.family {
        Family::Tables => tables::run(command, args),
        Family::Properties => properties::run(command, args),
        Family::Resolve => resolve::run(command, args),
        Family::Script => script::run(command, args),
        Family::Classes => classes::run(command, args),
        Family::Assets => assets::run(command, args),
        Family::Objects => objects::run(command, args),
        Family::Defs => defs::run(command, args),
        Family::Vm => vm::run(command, args),
        Family::Census => census::run(command, args),
        Family::Items => items::run(command, args),
        Family::ItemsRoll => items_roll::run(command, args),
        Family::ItemsStats => items_stats::run(command, args),
        Family::ItemsPools => items_pools::run(command, args),
        Family::Behavior => behavior::run(command, args),
        Family::Kismet => kismet::run(command, args),
        Family::Mover => mover::run(command, args),
        Family::Inventory => inventory::run(command, args),
        Family::Mission => mission::run(command, args),
        Family::Slice => slice::run(command, args),
        Family::Combat => combat::run(command, args),
    }
}

/// Says that `command` is not ported yet and returns [`NOT_PORTED`].
pub fn not_ported(command: &Command) -> ExitCode {
    eprintln!(
        "ow-package (Rust): {} is not ported to Rust yet (lane {}); use the C++ build/Release/ow-package.exe",
        command.flag,
        command.family.owner()
    );
    ExitCode::from(NOT_PORTED)
}
