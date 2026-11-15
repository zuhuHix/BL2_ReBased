//! Owner: lane W4-ROLL. Commands: `--item-roll` (Rust-only).
//!
//! ```text
//! ow-package --item-roll <balance path> --cooked <install folder> --stage <n> --seed <n> [--count <n>]
//!     [--playthrough <index>] [--op-level <n>]
//! ow-package --item-roll --census <file> --cooked <install folder> [--rolls <n>]
//! ow-package --item-roll --weapons <file> --cooked <install folder> [--seeds <n>]
//! ```
//!
//! The first form prints one JSON line per seed (`--seed`, then the next `--count - 1` seeds), each the roll of
//! `ow_items::roll`: in `OW_PARITY=reference`, a weapon in `weapon_recipe.roll`'s shape and an item in
//! `item_roll.roll`'s; in Off mode, every kind in the item shape. A balance that yields nothing at the stage prints
//! `{"balance":...,"seed":...,"stage":...,"not_rollable":"<reason>"}`. In the two batch forms a roll that answers
//! `NotYet` is the line `{..., "not_yet":"<what is missing>"}` and the batch goes on; in the first form it ends the
//! command with exit code 3.
//!
//! `--census <file>` writes `item_roll.py --census`'s rolls as JSON lines: every shield, grenade mod, class mod and
//! relic balance (sorted), at stages 1, 8, 15, 30, 50 and 80, with `--rolls` (default 20) spread seeds each, so that
//! `item_roll.check` can be run on them (lane W4-ROLL's comparison script). `--weapons <file>` writes the weapon
//! comparison's rolls: every weapon balance `WillowGame/CookedPCConsole/Startup.upk` exports (sorted; its values are
//! the owner's, the first holder in the tool's file order), at the same
//! stages, seeds 1 to `--seeds` (default 20), as `weapon_gen_parity.py` requests them. Both are run with
//! `OW_PARITY=reference` for the comparisons (architecture section 6.1). Output is game data: write it under ignored
//! `local/` only (docs/LEGAL.md).
//!
//! The player state is a snapshot of one player in the given playthrough (default 0) and OP level (default 0), every
//! level-cap pack licensed (the tools' full cap of 80), the player's level taken as the stage (the roll does not read
//! it). `--overlay` is refused: the roll reads tag streams, which overlays do not patch (as `--item-registry`).
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
use ow_core::{DetSet, NotYet, ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_items::merge::{RollData, WEAPON_BALANCE_CLASSES};
use ow_items::registry::{CensusInput, Registry, build_from_census, python_path_text};
use ow_items::roll::{
    CENSUS_STAGES, ITEM_KINDS, RollCx, RollError, RollResult, RollWorld, roll, spread_seeds,
};
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, PropId};

use super::NOT_PORTED;
use crate::args::Command;

const USAGE: &str = "usage: ow-package --item-roll <balance path> --cooked <install folder> --stage <n> --seed <n> \
[--count <n>] [--playthrough <index>] [--op-level <n>] | --item-roll --census <file> --cooked <install folder> \
[--rolls <n>] | --item-roll --weapons <file> --cooked <install folder> [--seeds <n>]";

/// The file the weapon comparison reads its balances from (`weapon_gen_parity.py`).
const WEAPON_PACKAGE: &str = "WillowGame/CookedPCConsole/Startup.upk";

#[derive(Default)]
struct Line {
    balance: Option<String>,
    cooked: Option<String>,
    stage: Option<i32>,
    seed: Option<u32>,
    count: u32,
    playthrough: i32,
    op: i32,
    census: Option<String>,
    weapons: Option<String>,
    rolls: u32,
    seeds: u32,
}

fn parse(command: &Command, args: &[String]) -> Result<Line, String> {
    let mut line = Line {
        count: 1,
        rolls: 20,
        seeds: 20,
        ..Line::default()
    };
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        if arg == command.flag {
            // The balance path follows the flag unless another option does.
            if let Some(next) = args.get(i + 1).filter(|a| !a.starts_with("--")) {
                line.balance = Some(next.clone());
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
            "--census" => line.census = Some(value()?),
            "--weapons" => line.weapons = Some(value()?),
            "--stage" => line.stage = Some(to_i32(number(value()?)?, arg)?),
            "--seed" => {
                let n = number(value()?)?;
                line.seed =
                    Some(u32::try_from(n).map_err(|_| format!("{arg}: out of range: {n}"))?);
            }
            "--count" => line.count = to_count(number(value()?)?, arg)?,
            "--rolls" => line.rolls = to_count(number(value()?)?, arg)?,
            "--seeds" => line.seeds = to_count(number(value()?)?, arg)?,
            "--playthrough" => line.playthrough = to_i32(number(value()?)?, arg)?,
            "--op-level" => line.op = to_i32(number(value()?)?, arg)?,
            "--overlay" => {
                return Err(
                    "--overlay is not supported by --item-roll yet: the roll reads tag streams, which overlays do \
                     not patch"
                        .to_string(),
                );
            }
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
    let Some(cooked) = line.cooked.clone() else {
        return fail(USAGE);
    };
    let single = line.balance.is_some();
    let batches = usize::from(line.census.is_some()) + usize::from(line.weapons.is_some());
    if usize::from(single) + batches != 1
        || (single && (line.stage.is_none() || line.seed.is_none()))
    {
        return fail(USAGE);
    }
    match execute(&line, Path::new(&cooked), mode) {
        Ok(()) => ExitCode::SUCCESS,
        Err(RollError::NotYet(e)) => {
            eprintln!("ow-package (Rust): --item-roll: {e}");
            ExitCode::from(NOT_PORTED)
        }
        Err(e) => fail(&super::properties::error_text(&e)),
    }
}

fn fail(message: &str) -> ExitCode {
    eprintln!("ow-package: {message}");
    ExitCode::from(1)
}

/// A world with no engine objects: the store only. Used when the offline world's classes are not laid out; every
/// evaluation that needs a world object answers `NotYet`.
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
            ow_items::roll::RECORD,
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
            ow_items::roll::RECORD,
        )
        .into())
    }
    fn rng(&mut self, s: CrtStream) -> Result<&mut dyn Rand15, AttrError> {
        Err(NotYet::layer_absent(
            format!("a random source for {s:?}"),
            "ow-world",
            ow_items::roll::RECORD,
        )
        .into())
    }
}

fn snapshot(line: &Line, stage: i32) -> Result<BalanceSnapshot, RollError> {
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

/// The world the rolls evaluate in: the offline world when its classes lay out from the store, else a bare one.
fn world(defs: DefStore, base: BalanceSnapshot) -> Box<dyn AttrWorld> {
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
        // The offline world takes the store; if it refuses the space, the store is gone with it, so build the bare
        // world from a fresh empty store (it then answers NotYet for every lookup).
        match OfflineWorld::new(space, Some(defs), base) {
            Ok(w) => return Box::new(w),
            Err(_) => {
                return Box::new(BareWorld {
                    space: ObjectSpace::new(),
                    defs: DefStore::new_empty(),
                    slots: GlobalSlots::default(),
                });
            }
        }
    }
    Box::new(BareWorld {
        space,
        defs,
        slots: GlobalSlots::default(),
    })
}

struct Session {
    reg: Registry,
    /// The weapon balances the comparison's package holds (whoever owns them), sorted.
    weapons: Vec<String>,
    data: RollData,
    sys: AttributeSystem,
    world: Box<dyn AttrWorld>,
    mode: ParityMode,
}

impl Session {
    fn open(cooked: &Path, line: &Line, mode: ParityMode) -> Result<Session, RollError> {
        let mut defs = DefStore::open(
            PathBuf::from(cooked),
            IdentityPolicy::FirstInScanOrder,
            mode,
        )
        .map_err(ow_items::ItemError::from)?;
        let census = defs.census().map_err(ow_items::ItemError::from)?;
        let root = defs.packages().root().to_path_buf();
        let input = CensusInput::from_store(census, &root);
        let package: PathBuf = WEAPON_PACKAGE.split('/').collect();
        let weapons: DetSet<String> = input
            .paths
            .iter()
            .filter(|e| {
                e.holders.iter().any(|h| {
                    h.file.ends_with(&package)
                        && WEAPON_BALANCE_CLASSES
                            .iter()
                            .any(|c| h.class == format!("WillowGame.{c}"))
                })
            })
            .map(|e| e.path.clone())
            .collect();
        let data = RollData::new(&input);
        let reg = build_from_census(input, &python_path_text(&root.to_string_lossy()));
        let sys = AttributeSystem::standard()?;
        let world = world(defs, snapshot(line, 1)?);
        Ok(Session {
            reg,
            weapons: weapons.into_iter().collect(),
            data,
            sys,
            world,
            mode,
        })
    }

    /// One roll as its JSON line.
    fn line(
        &mut self,
        line: &Line,
        balance: &str,
        stage: i32,
        seed: u32,
    ) -> Result<String, RollError> {
        let snap = snapshot(line, stage)?;
        let path = ObjectPath::parse(balance)
            .map_err(|e| RollError::BadData(format!("{balance}: {e}")))?;
        let mut rng = CrtRand::new(seed);
        let mut rw = RollWorld::new(self.world.as_mut(), &mut rng, false);
        let mut cx = RollCx::new(
            &snap,
            EvalCx::new(&mut rw),
            &self.sys,
            &self.data,
            self.mode,
        );
        let result = roll(&self.reg, &path, &mut cx)?;
        Ok(match result {
            RollResult::Item(r) if r.kind == "weapon" && self.mode == ParityMode::Reference => {
                r.to_weapon_pv(seed, &self.data).to_json()
            }
            RollResult::Item(r) => r.to_item_pv(seed).to_json(),
            RollResult::NotRollable(n) => {
                outcome_line(balance, stage, seed, "not_rollable", n.reason)
            }
            _ => {
                return Err(RollError::BadData(
                    "a roll result this command does not know".to_string(),
                ));
            }
        })
    }

    /// The balances of the four item kinds (the registry's kinds), or the weapon balances of the comparison's
    /// package, sorted.
    fn balances(&self, weapons: bool) -> Vec<String> {
        if weapons {
            return self.weapons.clone();
        }
        let Some(ow_items::registry::Pv::Map(map)) = self.reg.tree().get("balances") else {
            return Vec::new();
        };
        map.iter()
            .filter(|(_, r)| {
                r.at("kind")
                    .as_str()
                    .is_some_and(|k| ITEM_KINDS.contains(&k))
            })
            .map(|(k, _)| k.clone())
            .collect()
    }
}

/// `{"balance":...,"seed":...,"stage":...,"<key>":"<value>"}`.
fn outcome_line(balance: &str, stage: i32, seed: u32, key: &str, value: &str) -> String {
    let mut out = String::from("{\"balance\":");
    out.push_str(&ow_core::fmt::json_quote_py(balance));
    out.push_str(&format!(",\"seed\":{seed},\"stage\":{stage},\"{key}\":"));
    out.push_str(&ow_core::fmt::json_quote_py(value));
    out.push('}');
    out
}

fn execute(line: &Line, cooked: &Path, mode: ParityMode) -> Result<(), RollError> {
    let mut session = Session::open(cooked, line, mode)?;
    if let Some(balance) = &line.balance {
        let (Some(stage), Some(seed)) = (line.stage, line.seed) else {
            return Err(RollError::BadData(USAGE.to_string()));
        };
        let stdout = std::io::stdout();
        let mut out = stdout.lock();
        for k in 0..line.count {
            let text = session.line(line, balance, stage, seed.wrapping_add(k))?;
            // Ignore a failed write (a closed pipe), as the other commands do.
            let _ = writeln!(out, "{text}");
        }
        let _ = out.flush();
        return Ok(());
    }
    let (file, weapons) = match (&line.census, &line.weapons) {
        (Some(f), None) => (f, false),
        (None, Some(f)) => (f, true),
        _ => return Err(RollError::BadData(USAGE.to_string())),
    };
    let seeds: Vec<u32> = if weapons {
        (1..=line.seeds).collect()
    } else {
        spread_seeds(line.rolls)
    };
    let mut text = String::new();
    for balance in session.balances(weapons) {
        for stage in CENSUS_STAGES {
            for seed in &seeds {
                // A roll that answers NotYet (Off mode's class mods, for one) is a line of its own, so one gap does
                // not stop the batch; any other error does.
                let roll = match session.line(line, &balance, stage, *seed) {
                    Err(RollError::NotYet(n)) => {
                        outcome_line(&balance, stage, *seed, "not_yet", &n.missing.to_string())
                    }
                    other => other?,
                };
                text.push_str(&roll);
                text.push('\n');
            }
        }
    }
    std::fs::write(file, text).map_err(|e| RollError::BadData(format!("{file}: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::{Dispatch, dispatch};

    fn command() -> &'static Command {
        let args = vec!["--item-roll".to_string()];
        match dispatch(&args) {
            Dispatch::Run(c) => c,
            Dispatch::Usage => panic!("--item-roll is a command"),
        }
    }

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn bad_command_lines_fail_with_code_1() {
        let c = command();
        for line in [
            &["--item-roll"][..],
            &["--item-roll", "X.Y", "--cooked"],
            &["--item-roll", "X.Y", "--cooked", ".", "--stage", "8"],
            &[
                "--item-roll",
                "X.Y",
                "--cooked",
                ".",
                "--stage",
                "x",
                "--seed",
                "1",
            ],
            &[
                "--item-roll",
                "X.Y",
                "--cooked",
                ".",
                "--stage",
                "8",
                "--seed",
                "-1",
            ],
            &[
                "--item-roll",
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
                "--item-roll",
                "X.Y",
                "--cooked",
                ".",
                "--stage",
                "8",
                "--seed",
                "1",
                "--overlay",
                "o.json",
            ],
            &[
                "--item-roll",
                "X.Y",
                "--cooked",
                ".",
                "--stage",
                "8",
                "--seed",
                "1",
                "--census",
                "c.jsonl",
            ],
            &[
                "--item-roll",
                "--census",
                "c.jsonl",
                "--weapons",
                "w.jsonl",
                "--cooked",
                ".",
            ],
            &["--item-roll", "--bogus"],
        ] {
            assert_eq!(
                run(c, &args(line), ParityMode::Off),
                ExitCode::from(1),
                "{line:?}"
            );
        }
    }

    #[test]
    fn an_empty_install_rolls_nothing() {
        // The store's census has a body (W3-DEFS): an install with no package holds no balance, so the roll is
        // not rollable (exit 0) and the census writes no line.
        let dir = std::env::temp_dir().join(format!("ow-cli-items-roll-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let d = dir.to_string_lossy().to_string();
        let single = args(&[
            "--item-roll",
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
            ExitCode::SUCCESS
        );
        let file = dir.join("census.jsonl");
        let f = file.to_string_lossy().to_string();
        let census = args(&["--item-roll", "--census", &f, "--cooked", &d]);
        assert_eq!(
            run(command(), &census, ParityMode::Reference),
            ExitCode::SUCCESS
        );
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
