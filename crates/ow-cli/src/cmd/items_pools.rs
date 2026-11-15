//! Owner: lane W4-POOLS. Commands: `--item-pool` (Rust-only).
//!
//! ```text
//! ow-package --item-pool <pool path> --cooked <install folder> --stage <n> --seed <n> [--count <n>]
//!     [--playthrough <index>] [--op-level <n>] [--package <file>]
//! ow-package --item-pool --parity <file> --cooked <install folder> [--seeds <n>] [--package <file>]
//! ```
//!
//! `<pool path>` is an `ItemPoolDefinition`, an `ItemPoolListDefinition` or an `AIPawnBalanceDefinition`. The pool
//! data is read from one package, `--package` (relative to the install folder, default
//! `WillowGame/CookedPCConsole/Startup.upk`), as `tools/loot_pools.py` reads one package.
//!
//! The first form prints one JSON line per seed (`--seed`, then the next `--count - 1` seeds), each the roll of
//! `ow_items::pools::roll_pool` from one `CrtRand(seed)`. In `OW_PARITY=reference` it is `loot_pools.py`'s CrtRand
//! path, printed as the C++ batch header prints a pool roll: `{"drops":[<balance>...],"pool":...,"seed":...,
//! "stage":...}`. In Off mode the roll is the death roll of NATIVE_LOOT.md and each drop is an object (balance, chain,
//! the pool-level candidate's manufacturer, definition, stage, level, grade-or-level word and weight, and the
//! drop-on-death flag, `null` while unknown). A roll that answers `NotYet` ends the first form with exit code 3; in
//! the batch form it is the line `{..., "not_yet":"<what is missing>"}` and the batch goes on.
//!
//! `--parity <file>` writes the rolls of `tools/weapon_gen_parity.py`'s pool requests: every pool below
//! `.WeaponPools.` and every `ItemPoolListDefinition` of the package (sorted), at stages 1, 8, 15, 30, 50 and 80,
//! seeds 1 to `--seeds` (default 20), so that lane W4-POOLS's comparison script can compare them with
//! `loot_pools.roll_crt` / `roll_pool_crt`. Run it with `OW_PARITY=reference` for that comparison. Output is game
//! data: write it under ignored `local/` only (docs/LEGAL.md).
//!
//! The player state is one player in the given playthrough (default 0) and OP level (default 0), every level-cap
//! pack licensed. `--overlay` is refused: the roll reads tag streams, which overlays do not patch (as `--item-roll`).
//! Exit codes: 0 success, 1 a bad command line or a failed roll, 3 a layer not there yet (`NotYet`).

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ow_attr::AttributeSystem;
use ow_attr::api::{AttrError, AttrWorld, EvalCx, GlobalSlots, NumForms, WorldParts};
use ow_balance::offline::{
    GAME_REPLICATION_INFO_CLASS, GLOBALS_CLASS, OfflineWorld, PLAYER_CONTROLLER_CLASS,
    REPLICATION_INFO_CLASS,
};
use ow_balance::types::{
    AwesomeLevel, BalanceSnapshot, ExpLevel, GameStage, LevelCapUpgrades, OpLevel, PlayerCount,
    Playthrough,
};
use ow_core::rand::{CrtRand, CrtStream, Rand15};
use ow_core::{NotYet, ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_items::ItemError;
use ow_items::merge::RollData;
use ow_items::pools::{
    PARITY_PACKAGE, PARITY_STAGES, PoolData, PoolRequest, off_drop_json, parity_line, roll_pool,
};
use ow_items::registry::{CensusInput, Pv, Registry, build_from_census, python_path_text};
use ow_items::roll::{RollCx, RollWorld};
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, PropId};

use super::NOT_PORTED;
use crate::args::Command;

const USAGE: &str = "usage: ow-package --item-pool <pool path> --cooked <install folder> --stage <n> --seed <n> \
[--count <n>] [--playthrough <index>] [--op-level <n>] [--package <file>] | --item-pool --parity <file> \
--cooked <install folder> [--seeds <n>] [--package <file>]";

#[derive(Default)]
struct Line {
    pool: Option<String>,
    cooked: Option<String>,
    stage: Option<i32>,
    seed: Option<u32>,
    count: u32,
    playthrough: i32,
    op: i32,
    package: Option<String>,
    parity: Option<String>,
    seeds: u32,
    overlay: Option<String>,
}

fn parse(command: &Command, args: &[String]) -> Result<Line, String> {
    let mut line = Line {
        count: 1,
        seeds: 20,
        ..Line::default()
    };
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        if arg == command.flag {
            // The pool path follows the flag unless another option does.
            if let Some(next) = args.get(i + 1).filter(|a| !a.starts_with("--")) {
                line.pool = Some(next.clone());
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        let value = || {
            args.get(i + 1)
                .cloned()
                .ok_or_else(|| format!("missing value for {arg}"))
        };
        let number = |v: String| -> Result<i64, String> {
            v.parse::<i64>()
                .map_err(|_| format!("{arg}: not a number: {v}"))
        };
        match arg {
            "--cooked" => line.cooked = Some(value()?),
            "--parity" => line.parity = Some(value()?),
            "--package" => line.package = Some(value()?),
            "--stage" => line.stage = Some(to_i32(number(value()?)?, arg)?),
            "--seed" => {
                let n = number(value()?)?;
                line.seed =
                    Some(u32::try_from(n).map_err(|_| format!("{arg}: out of range: {n}"))?);
            }
            "--count" => line.count = to_count(number(value()?)?, arg)?,
            "--seeds" => line.seeds = to_count(number(value()?)?, arg)?,
            "--playthrough" => line.playthrough = to_i32(number(value()?)?, arg)?,
            "--op-level" => line.op = to_i32(number(value()?)?, arg)?,
            "--overlay" => line.overlay = Some(value()?),
            _ => return Err(USAGE.to_string()),
        }
        i += 2;
    }
    Ok(line)
}

fn to_i32(n: i64, what: &str) -> Result<i32, String> {
    i32::try_from(n).map_err(|_| format!("{what}: out of range: {n}"))
}

fn to_count(n: i64, what: &str) -> Result<u32, String> {
    u32::try_from(n)
        .ok()
        .filter(|c| *c >= 1)
        .ok_or_else(|| format!("{what}: must be at least 1: {n}"))
}

pub fn run(command: &Command, args: &[String], mode: ParityMode) -> ExitCode {
    let line = match parse(command, args) {
        Ok(line) => line,
        Err(message) => return fail(&message),
    };
    if line.overlay.is_some() {
        // Architecture section 4: a layer not there yet is NotYet (exit 3), not a bad command line.
        let e = NotYet::not_ported(
            "--overlay on --item-pool: the pool roll reads tag streams, which overlays do not patch",
            ow_items::pools::RECORD,
        );
        eprintln!("ow-package (Rust): --item-pool: {e}");
        return ExitCode::from(NOT_PORTED);
    }
    let Some(cooked) = line.cooked.clone() else {
        return fail(USAGE);
    };
    let single = line.pool.is_some();
    if single == line.parity.is_some() || (single && (line.stage.is_none() || line.seed.is_none()))
    {
        return fail(USAGE);
    }
    match execute(&line, Path::new(&cooked), mode) {
        Ok(()) => ExitCode::SUCCESS,
        Err(ItemError::NotYet(e)) => {
            eprintln!("ow-package (Rust): --item-pool: {e}");
            ExitCode::from(NOT_PORTED)
        }
        Err(e) => fail(&super::properties::error_text(&e)),
    }
}

fn fail(message: &str) -> ExitCode {
    eprintln!("ow-package: {message}");
    ExitCode::from(1)
}

/// A world with no engine objects: the store only (as `--item-roll`'s). Every evaluation that needs a world object
/// answers `NotYet`.
struct BareWorld {
    space: ObjectSpace,
    defs: DefStore,
    slots: GlobalSlots,
}

impl AttrWorld for BareWorld {
    fn parts(&mut self) -> WorldParts<'_> {
        WorldParts::new(&mut self.space, Some(&mut self.defs))
    }
    fn globals(&mut self) -> Result<ObjRef, AttrError> {
        Err(NotYet::layer_absent(
            "the engine globals (the offline world's classes are not laid out in this space)",
            "ow-object reflection of the WillowGame classes",
            ow_items::pools::RECORD,
        )
        .into())
    }
    fn game_info(&mut self) -> Result<Option<ObjRef>, AttrError> {
        Ok(None)
    }
    fn global_slots(&mut self) -> &mut GlobalSlots {
        &mut self.slots
    }
    fn attribute_changed(&mut self, _: ObjRef, _: PropId, _: NumForms) -> Result<(), AttrError> {
        Err(NotYet::layer_absent(
            "a change notification offline",
            "ow-world",
            ow_items::pools::RECORD,
        )
        .into())
    }
    fn rng(&mut self, s: CrtStream) -> Result<&mut dyn Rand15, AttrError> {
        Err(NotYet::layer_absent(
            format!("a random source for {s:?}"),
            "ow-world",
            ow_items::pools::RECORD,
        )
        .into())
    }
}

fn snapshot(line: &Line, stage: i32) -> Result<BalanceSnapshot, ItemError> {
    Ok(BalanceSnapshot::new(
        Playthrough::from_index(line.playthrough)?,
        OpLevel::new(line.op)?,
        PlayerCount::SINGLE,
        ExpLevel::new(stage.clamp(1, 80)),
        GameStage(stage),
        AwesomeLevel::new(0),
        LevelCapUpgrades::ALL,
    )?)
}

/// The offline world when its classes lay out from the store, else a bare one over the store.
fn world(defs: DefStore, base: BalanceSnapshot) -> Result<Box<dyn AttrWorld>, ItemError> {
    let mut defs = defs;
    let mut space = ObjectSpace::new();
    let laid_out = [
        PLAYER_CONTROLLER_CLASS,
        REPLICATION_INFO_CLASS,
        GAME_REPLICATION_INFO_CLASS,
        GLOBALS_CLASS,
    ]
    .iter()
    .all(|c| defs.class_id(&mut space, c).is_ok());
    if laid_out {
        return Ok(Box::new(OfflineWorld::new(space, Some(defs), base)?));
    }
    Ok(Box::new(BareWorld {
        space,
        defs,
        slots: GlobalSlots::default(),
    }))
}

struct Session {
    pools: PoolData,
    /// Off mode only: the registry and the values the balance expansion reads.
    reg: Option<Registry>,
    data: RollData,
    sys: AttributeSystem,
    world: Box<dyn AttrWorld>,
    mode: ParityMode,
}

impl Session {
    fn open(cooked: &Path, line: &Line, mode: ParityMode) -> Result<Session, ItemError> {
        let mut defs = DefStore::open(
            PathBuf::from(cooked),
            IdentityPolicy::FirstInScanOrder,
            mode,
        )
        .map_err(ItemError::from)?;
        let census = defs.census().map_err(ItemError::from)?;
        let root = defs.packages().root().to_path_buf();
        let package = line.package.as_deref().unwrap_or(PARITY_PACKAGE);
        let pools = PoolData::new(PoolData::one_file(&census, &root, package))?;
        let (reg, data) = if mode == ParityMode::Reference {
            // The oracle reads no registry and no balance values.
            (None, RollData::new(&CensusInput::new(root.clone()))?)
        } else {
            let input = CensusInput::from_store(census, &root);
            let data = RollData::new(&input)?;
            let reg = build_from_census(input, &python_path_text(&root.to_string_lossy()));
            (Some(reg), data)
        };
        let sys = AttributeSystem::standard()?;
        let world = world(defs, snapshot(line, 1)?)?;
        Ok(Session {
            pools,
            reg,
            data,
            sys,
            world,
            mode,
        })
    }

    fn line(
        &mut self,
        line: &Line,
        pool: &str,
        stage: i32,
        seed: u32,
    ) -> Result<String, ItemError> {
        let snap = snapshot(line, stage)?;
        let path =
            ObjectPath::parse(pool).map_err(|e| ItemError::BadData(format!("{pool}: {e}")))?;
        let mut rng = CrtRand::new(seed);
        let mut rw = RollWorld::new(self.world.as_mut(), &mut rng, false);
        let mut cx = RollCx::new(
            &snap,
            EvalCx::new(&mut rw),
            &self.sys,
            &self.data,
            self.mode,
        );
        let request = PoolRequest::for_mode(self.mode);
        let drops = roll_pool(&self.pools, self.reg.as_ref(), &path, &request, &mut cx)?;
        if self.mode == ParityMode::Reference {
            return Ok(parity_line(pool, stage, seed, &drops));
        }
        let mut tree = ow_core::DetMap::new();
        tree.insert("pool".to_string(), Pv::Str(pool.to_string()));
        tree.insert("stage".to_string(), Pv::Int(i64::from(stage)));
        tree.insert("seed".to_string(), Pv::Int(i64::from(seed)));
        tree.insert(
            "drops".to_string(),
            Pv::List(drops.iter().map(off_drop_json).collect()),
        );
        Ok(Pv::Map(tree).to_json())
    }
}

/// `{"pool":...,"seed":...,"stage":...,"not_yet":"<missing>"}`.
fn not_yet_line(pool: &str, stage: i32, seed: u32, missing: &str) -> String {
    let mut out = String::from("{\"not_yet\":");
    out.push_str(&ow_core::fmt::json_quote_py(missing));
    out.push_str(",\"pool\":");
    out.push_str(&ow_core::fmt::json_quote_py(pool));
    out.push_str(&format!(",\"seed\":{seed},\"stage\":{stage}}}"));
    out
}

fn execute(line: &Line, cooked: &Path, mode: ParityMode) -> Result<(), ItemError> {
    let mut session = Session::open(cooked, line, mode)?;
    if let Some(pool) = &line.pool {
        let (Some(stage), Some(seed)) = (line.stage, line.seed) else {
            return Err(ItemError::BadData(USAGE.to_string()));
        };
        let stdout = std::io::stdout();
        let mut out = stdout.lock();
        for k in 0..line.count {
            let text = session.line(line, pool, stage, seed.wrapping_add(k))?;
            // Ignore a failed write (a closed pipe), as the other commands do.
            let _ = writeln!(out, "{text}");
        }
        let _ = out.flush();
        return Ok(());
    }
    let Some(file) = &line.parity else {
        return Err(ItemError::BadData(USAGE.to_string()));
    };
    let mut text = String::new();
    for pool in session.pools.parity_pools() {
        for stage in PARITY_STAGES {
            for seed in 1..=line.seeds {
                // A roll that answers NotYet is a line of its own, so one gap does not stop the batch.
                let roll = match session.line(line, &pool, stage, seed) {
                    Err(ItemError::NotYet(n)) => not_yet_line(&pool, stage, seed, &n.to_string()),
                    other => other?,
                };
                text.push_str(&roll);
                text.push('\n');
            }
        }
    }
    std::fs::write(file, text).map_err(|e| ItemError::BadData(format!("{file}: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::{Dispatch, dispatch};

    fn command() -> &'static Command {
        let args = vec!["--item-pool".to_string()];
        match dispatch(&args) {
            Dispatch::Run(c) => c,
            Dispatch::Usage => panic!("--item-pool is a command"),
        }
    }

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn bad_command_lines_fail_with_code_1() {
        let c = command();
        for line in [
            &["--item-pool"][..],
            &["--item-pool", "X.Y", "--cooked"],
            &["--item-pool", "X.Y", "--cooked", ".", "--stage", "8"],
            &[
                "--item-pool",
                "X.Y",
                "--cooked",
                ".",
                "--stage",
                "x",
                "--seed",
                "1",
            ],
            &[
                "--item-pool",
                "X.Y",
                "--cooked",
                ".",
                "--stage",
                "8",
                "--seed",
                "-1",
            ],
            &[
                "--item-pool",
                "X.Y",
                "--cooked",
                ".",
                "--stage",
                "8",
                "--seed",
                "1",
                "--count",
                "0",
            ],
            &[
                "--item-pool",
                "X.Y",
                "--cooked",
                ".",
                "--stage",
                "8",
                "--seed",
                "1",
                "--parity",
                "p.jsonl",
            ],
            &[
                "--item-pool",
                "--parity",
                "p.jsonl",
                "--cooked",
                ".",
                "--seeds",
                "0",
            ],
            &["--item-pool", "--bogus"],
        ] {
            assert_eq!(
                run(c, &args(line), ParityMode::Off),
                ExitCode::from(1),
                "{line:?}"
            );
        }
    }

    #[test]
    fn an_overlay_is_not_there_yet() {
        let line = args(&[
            "--item-pool",
            "X.Y",
            "--cooked",
            ".",
            "--stage",
            "8",
            "--seed",
            "1",
            "--overlay",
            "o.json",
        ]);
        assert_eq!(
            run(command(), &line, ParityMode::Off),
            ExitCode::from(NOT_PORTED)
        );
    }

    #[test]
    fn an_empty_install_has_no_pool() {
        // No package holds the pool: a failed roll (exit 1); the parity batch has no pool and writes no line.
        let dir = std::env::temp_dir().join(format!("ow-cli-items-pools-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let d = dir.to_string_lossy().to_string();
        let single = args(&[
            "--item-pool",
            "X.Y",
            "--cooked",
            &d,
            "--stage",
            "8",
            "--seed",
            "1",
        ]);
        assert_eq!(
            run(command(), &single, ParityMode::Reference),
            ExitCode::from(1)
        );
        let file = dir.join("parity.jsonl");
        let f = file.to_string_lossy().to_string();
        let batch = args(&["--item-pool", "--parity", &f, "--cooked", &d]);
        assert_eq!(
            run(command(), &batch, ParityMode::Reference),
            ExitCode::SUCCESS
        );
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
