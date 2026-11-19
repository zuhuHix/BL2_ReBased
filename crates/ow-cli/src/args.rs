//! Owner: lane W0-SCAFFOLD. The command dispatch table of the Rust `ow-package` (a frozen contract, architecture
//! section 7): every command flag of `src/cli.cpp`'s usage and the Rust-only flags of architecture section 2.3, the
//! command family (`cmd/<family>.rs`) each goes to. Options are listed with the commands
//! that read them in `src/cli.cpp` (the Rust-only ones as architecture section 15 lists them). This table is the
//! usage record only: each `cmd/` module parses its own trailing modifiers, and lanes never edit this table or
//! `main.rs`. `tests/crate_edges_test.py` fails when a flag of the C++
//! usage text has no entry or option here.

/// Where the command flag stands on the command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    /// First argument, no package: `ow-package --native <name>`, `ow-package --native-selftest`.
    First,
    /// After the package path: `ow-package <package> --names` (the C++ `argv[2]`).
    AfterPackage,
    /// Either place. Used for the Rust-only flags, whose command lines their owner lanes settle.
    Either,
}

/// A command family: one `cmd/<family>.rs` file, owned by one lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Tables,
    Properties,
    Resolve,
    Script,
    Classes,
    Assets,
    Objects,
    Defs,
    Vm,
    Census,
    Items,
    ItemsRoll,
    ItemsStats,
    ItemsPools,
    Behavior,
    Kismet,
    Mover,
    Inventory,
    Mission,
    Slice,
    Combat,
    SliceGate,
}

/// One dispatch entry.
#[derive(Debug)]
pub struct Command {
    /// The command flag; [`SUMMARY`] uses `<package>`.
    pub flag: &'static str,
    pub position: Position,
    pub family: Family,
    /// The options this command takes after its flag, as in the C++ usage text (the Rust-only ones as architecture
    /// section 15 lists them). The usage record only: each `cmd/` module parses its own trailing arguments.
    #[allow(dead_code)]
    pub options: &'static [&'static str],
}

const fn cmd(
    flag: &'static str,
    position: Position,
    family: Family,
    options: &'static [&'static str],
) -> Command {
    Command {
        flag,
        position,
        family,
        options,
    }
}

use Family as F;
use Position::{AfterPackage as P, Either as E, First};

// The Rust-only commands' modifiers (architecture section 15, item 3). `--op-level` is the OP level; `--op` is a
// Kismet option of `--kismet-run`.
/// `--def-dump <object path> --cooked <dir> [--overlay <file>]`.
const DEF_DUMP: &[&str] = &["--cooked", "--overlay"];
/// `--item-registry --cooked <dir> [--census <file>] [--overlay <file>]`; items family: `<dir>` is the install folder.
const ITEM_REGISTRY: &[&str] = &["--cooked", "--census", "--overlay"];
/// `--item-roll <balance path>` and `--item-pool <pool path>`: `--cooked <dir> --stage <n> --seed <n> [--count <n>]
/// [--playthrough <index>] [--op-level <n>] [--overlay <file>]`; items family: `<dir>` is the install folder.
const ITEM_ROLL: &[&str] = &[
    "--cooked",
    "--stage",
    "--seed",
    "--count",
    "--playthrough",
    "--op-level",
    "--overlay",
];
/// `--item-stats` and `--item-card`: `--cooked <dir>`, then `--data <file>` or the `--item-roll` modifiers,
/// `[--overlay <file>]`; items family: `<dir>` is the install folder.
const ITEM_STATS: &[&str] = &[
    "--cooked",
    "--data",
    "--stage",
    "--seed",
    "--count",
    "--playthrough",
    "--op-level",
    "--overlay",
];

/// Every command. The first block is the C++ usage text in its order; the last block is Rust-only.
pub const COMMANDS: &[Command] = &[
    cmd("--exports", P, F::Tables, &[]),
    cmd("--imports", P, F::Tables, &[]),
    cmd("--names", P, F::Tables, &[]),
    cmd("--census", P, F::Tables, &[]),
    cmd("--scene-records", P, F::Assets, &[]),
    cmd("--terrain-records", P, F::Assets, &[]),
    cmd("--payload", P, F::Tables, &[]),
    cmd("--payload-file", P, F::Tables, &[]),
    cmd("--payloads", P, F::Tables, &[]),
    cmd("--verify-decoded", P, F::Tables, &[]),
    cmd("--resolve", P, F::Resolve, &["--cooked"]),
    cmd(
        "--properties",
        P,
        F::Properties,
        &["--property-offset", "--array-schema"],
    ),
    cmd(
        "--properties-batch",
        P,
        F::Properties,
        &["--property-offset", "--array-schema"],
    ),
    cmd(
        "--mesh",
        P,
        F::Assets,
        &["--property-offset", "--output", "--lod"],
    ),
    cmd(
        "--texture",
        P,
        F::Assets,
        &[
            "--property-offset",
            "--output",
            "--tfc",
            "--mip",
            "--all-mips",
        ],
    ),
    cmd("--script-check", P, F::Script, &["--failures"]),
    cmd("--disasm", P, F::Script, &[]),
    cmd("--class-check", P, F::Classes, &["--cooked", "--failures"]),
    cmd("--interface-compare", P, F::Classes, &["--cooked", "--layouts"]), // --layouts: Rust-only (W2-REFLECT)
    cmd(
        "--vm-sweep",
        P,
        F::Census,
        &["--cooked", "--class", "--limit", "--steps", "--top"],
    ),
    cmd("--run-batch", P, F::Vm, &["--cooked"]),
    cmd(
        "--native-census",
        P,
        F::Census,
        &["--cooked", "--steps", "--no-static"],
    ),
    cmd("--inventory-move", P, F::Inventory, &["--cooked"]),
    cmd("--mover-probe", P, F::Mover, &["--cooked"]),
    cmd(
        "--kismet-run",
        P,
        F::Kismet,
        &[
            "--cooked",
            "--remote",
            "--mission",
            "--op",
            "--originator",
            "--tick",
        ],
    ),
    cmd("--object-dump", P, F::Objects, &["--cooked", "--all"]),
    cmd("--kismet-census", P, F::Kismet, &["--cooked"]),
    cmd("--mission-run", P, F::Mission, &["--cooked"]),
    cmd("--slice-run", P, F::Slice, &["--cooked"]),
    cmd("--combat-run", P, F::Combat, &["--cooked"]),
    cmd("--behavior-dump", P, F::Behavior, &["--cooked"]),
    cmd("--behavior-run", P, F::Behavior, &["--cooked"]),
    cmd("--run", P, F::Vm, &["--cooked", "--self", "--arg"]),
    cmd("--native", First, F::Vm, &["--native-args"]),
    cmd("--native-selftest", First, F::Census, &[]),
    // In src/cli.cpp's dispatch but not in its usage text.
    cmd("--mover-event", P, F::Mover, &["--cooked"]),
    // Rust-only flags (architecture.md section 2.3, CLI command files and flags).
    cmd("--def-dump", E, F::Defs, DEF_DUMP),
    cmd("--item-registry", E, F::Items, ITEM_REGISTRY),
    cmd("--item-roll", E, F::ItemsRoll, ITEM_ROLL),
    cmd("--item-stats", E, F::ItemsStats, ITEM_STATS),
    cmd("--item-card", E, F::ItemsStats, ITEM_STATS),
    cmd("--item-pool", E, F::ItemsPools, ITEM_ROLL),
    // `--slice-gate --cooked <dir>`: the 14 slice-gate links in Off (lane SLICE-RUN-OFF, decision 2026-11-19-08).
    cmd("--slice-gate", First, F::SliceGate, &["--cooked"]),
];

/// `ow-package <package>` with no command flag (the header summary), or with one the table does not know: the C++
/// binary loads the package before it prints the summary or the usage text, so both need the reader first.
pub const SUMMARY: Command = cmd("<package>", P, F::Tables, &[]);

/// The C++ usage text, byte for byte (`src/cli.cpp`, `usage()`); the Rust-only flags are not in it.
pub const USAGE: &str = concat!(
    "usage: ow-package <package> [--exports | --imports | --names | --census | --scene-records <schema> | --terrain-records <schema> | --payload <index> | --payload-file <index> <output> | --payloads <index>... | --verify-decoded <file> | ",
    "--resolve <reference> --cooked <directory> | --properties <index> ",
    "--property-offset <bytes> [--array-schema <file>] | --properties-batch <index-file> ",
    "--property-offset <bytes> [--array-schema <file>] | --mesh <index> ",
    "--property-offset <bytes> --output <obj> [--lod <index>] | --texture <index> ",
    "--property-offset <bytes> --output <png> --tfc <directory> [--mip <index>] ",
    "[--all-mips <directory>]] | --script-check [--failures] | --disasm <index|Class.Function> | ",
    "--class-check --cooked <directory> [--failures] | --interface-compare --cooked <directory> | ",
    "--vm-sweep --cooked <directory> [--class <name>] [--limit <n>] [--steps <n>] [--top <n>] | ",
    "--run-batch <file> --cooked <directory> | ",
    "--native-census <entry-file> --cooked <directory> [--steps <n>] [--no-static] | ",
    "--inventory-move <delta> <start> <count> --cooked <directory> | ",
    "--mover-probe <actor> <action> --cooked <directory> | ",
    "--kismet-run <sequence-path> --cooked <directory> (--remote <name> | --mission <path> <name> | --op <name> | --originator <object-path>) [--tick <s>]... | ",
    "--object-dump <export-index> <prefix> --cooked <directory> [--all] | ",
    "--kismet-census --cooked <directory> | --mission-run <mission-path> --cooked <directory> <step>... | ",
    "--slice-run <mission-path> --cooked <directory> <step>... | --combat-run --cooked <directory> <step>... | --behavior-dump <provider-path> --cooked <directory> | ",
    "--behavior-run <provider-path> --cooked <directory> <step>... | ",
    "--run <Package.Class.Function> --cooked <directory> [--self <Package.Class>] [--arg <type:value>]... | ",
    "--native <name> [--native-args <args>] | --native-selftest",
);

/// What `main` does with a command line.
#[derive(Debug)]
pub enum Dispatch {
    /// No arguments: print [`USAGE`] and exit 1, as the C++ binary does.
    Usage,
    /// Run this command's family.
    Run(&'static Command),
}

fn find(flag: &str, at_first: bool) -> Option<&'static Command> {
    COMMANDS.iter().find(|c| {
        c.flag == flag
            && match c.position {
                Position::First => at_first,
                Position::AfterPackage => !at_first,
                Position::Either => true,
            }
    })
}

/// Picks the command for `args` (the arguments after the program name), with the C++ binary's positions.
pub fn dispatch(args: &[String]) -> Dispatch {
    let Some(first) = args.first() else {
        return Dispatch::Usage;
    };
    if let Some(c) = find(first, true) {
        return Dispatch::Run(c);
    }
    match args.get(1).and_then(|second| find(second, false)) {
        Some(c) => Dispatch::Run(c),
        None => Dispatch::Run(&SUMMARY),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str]) -> &'static str {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        match dispatch(&args) {
            Dispatch::Usage => "usage",
            Dispatch::Run(c) => c.flag,
        }
    }

    #[test]
    fn positions_follow_the_cxx_binary() {
        assert_eq!(run(&[]), "usage");
        assert_eq!(run(&["pkg.upk", "--names"]), "--names");
        assert_eq!(run(&["--native", "Core.Object.Rand"]), "--native");
        assert_eq!(run(&["--native-selftest"]), "--native-selftest");
        assert_eq!(run(&["pkg.upk"]), "<package>");
        // C++ loads "--names" as a package and fails there: the reader comes first, so it is the summary's family.
        assert_eq!(run(&["--names", "x"]), "<package>");
        assert_eq!(run(&["pkg.upk", "--bogus"]), "<package>");
        assert_eq!(run(&["--item-roll", "x"]), "--item-roll");
        assert_eq!(run(&["pkg.upk", "--def-dump"]), "--def-dump");
    }

    #[test]
    fn every_flag_of_the_usage_text_is_a_command_or_an_option() {
        let mut known: Vec<&str> = COMMANDS.iter().map(|c| c.flag).collect();
        known.extend(COMMANDS.iter().flat_map(|c| c.options.iter().copied()));
        let mut rest = USAGE;
        let mut seen = 0;
        while let Some(i) = rest.find("--") {
            rest = &rest[i..];
            let end = rest
                .find(|ch: char| !(ch.is_ascii_lowercase() || ch == '-'))
                .unwrap_or(rest.len());
            let flag = &rest[..end];
            assert!(known.contains(&flag), "{flag} has no entry");
            seen += 1;
            rest = &rest[end..];
        }
        assert!(seen > 60);
    }

    #[test]
    fn flags_are_unique() {
        for (i, a) in COMMANDS.iter().enumerate() {
            assert!(
                COMMANDS[i + 1..].iter().all(|b| b.flag != a.flag),
                "{} twice",
                a.flag
            );
        }
    }
}
