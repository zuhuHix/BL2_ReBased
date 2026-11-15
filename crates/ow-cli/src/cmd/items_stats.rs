//! Owner: lane W4-STATS. Commands: `--item-stats`, `--item-card` (Rust-only).
//!
//! ```text
//! ow-package --item-card <balance path> --cooked <install folder> --stage <n> --seed <n> [--count <n>]
//! ow-package --item-card --weapons <file> --cooked <install folder> [--seeds <n>]
//! ow-package --item-card --requests <file> --output <file> --cooked <install folder>
//! ow-package --item-card --data <file> --cooked <install folder> [--level <n>] [--overlay <file>]
//! ow-package --item-stats <the same forms>
//! ```
//!
//! Each takes `[--colours <file>]`: the ignored local colours file (`tools/game_colours.py`'s
//! `{"colours": {"card_red_text": "#RRGGBB", ...}}`), whose `card_red_text` recognises a title's red flavour line;
//! without it the neutral grey of that tool is used, as the tool does without the file. The installed `.int` files
//! under `--cooked` override the cooked card strings.
//!
//! **`--item-card`, a weapon** (run with `OW_PARITY=reference`): one JSON line per gun in the shape
//! `tools/weapon_gen_parity.py` compares (`python_gun`: manufacturer, weapon type, gestalt, name, prefix, title, parts
//! with weight and candidate count, gestalt fragments, material, notes and the card), with the request's identity
//! (`balance`, `stage`, `seed`, `spawn_stage`, `stage_cap`; no cap: Reference applies none). The first form rolls
//! `--count` seeds from `--seed`; `--weapons` writes every weapon balance `Startup.upk` exports (sorted) at stages
//! 1, 8, 15, 30, 50 and 80 with seeds 1 to `--seeds` (default 20), the comparison's balance requests;
//! `--requests` reads `balance <path> <stage> <seed>` lines (the comparison's batch format) and writes one line per
//! request to `--output`, in order. A roll that is not rollable is `{"balance":...,"not_rollable":"<reason>"}`. In the
//! batch forms a `NotYet` is `{"balance":...,"not_yet":"<what is missing>"}`, a fifth field (a stage cap, which
//! Reference does not apply) is such a `not_yet` line, a failed evaluation is `{"balance":...,"error":"<why>"}`, and a
//! request line that names no gun is `{"line":<n>,"error":"<why>"}`.
//!
//! **`--data <file>`**: a JSON object naming the item (`kind`, default `weapon`; `balance`; `definition` or `type`;
//! `manufacturer`; `parts` by slot name; `prefix`; `title`; `game_stage`, the level unless `--level` gives it), the
//! shape of a golden card's record. A weapon prints its card; a shield prints its three numbers
//! (`prepare_stock_shield`'s card stats); other kinds print their owner stacks. `--overlay` is a live capture
//! (`tools/real_game/scripts/weapon_dump.py`), applied as `live_overlay.LivePackage` applies it.
//!
//! **`--item-stats`** prints the evaluated attributes instead of the card: `{"attributes": {path: value}, "base":
//! {...}, "unresolved": [...]}` for a weapon. In Off mode (no `OW_PARITY`) `--item-stats --data` instantiates the
//! item through `ow-attr` (`ow_items::stats::instantiate_stats`) and prints its main numbers; the balance forms are
//! Reference only (`NotYet`, exit 3).
//!
//! Output is game data: write it under ignored `local/` only (docs/LEGAL.md). Exit codes: 0 success, 1 a bad command
//! line or a failed evaluation, 3 a layer not there yet (`NotYet`). In the batch forms (`--weapons`, `--requests`) a
//! failed or `NotYet` evaluation is a line of its own and the command still exits 0; only a batch that cannot run at
//! all (an unreadable request file, a store that does not open) exits 1 or 3.

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
use ow_core::{DetMap, DetSet, NotYet, ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_defs::overlay::Overlay;
use ow_defs::store::DefError;
use ow_items::ItemError;
use ow_items::cards::{
    CardText, Localization, NEUTRAL_RED_TEXT, instance_numbers, shield_number_text, weapon_card,
};
use ow_items::data::{DefinitionData, ItemKind};
use ow_items::merge::{RollData, WEAPON_BALANCE_CLASSES};
use ow_items::registry::{
    CensusInput, ITEM_SLOTS, Pv, Registry, WEAPON_SLOTS, build_from_census, python_path_text,
};
use ow_items::roll::{CENSUS_STAGES, RollCx, RollResult, RollWorld, Rolled, roll};
use ow_items::stats::{
    StatsData, Values, WeaponRecipe, assembly, instantiate_stats, item_stats, weapon_attributes,
};
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, PropId};
use ow_object::reflect::ReflectError;
use ow_package::store::StoreError;

use super::NOT_PORTED;
use crate::args::Command;

const USAGE: &str = "usage: ow-package --item-card|--item-stats <balance path> --cooked <install folder> --stage <n> \
--seed <n> [--count <n>] | --weapons <file> --cooked <install folder> [--seeds <n>] | --requests <file> --output \
<file> --cooked <install folder> | --data <file> --cooked <install folder> [--level <n>] [--overlay <file>]; each \
[--colours <file>]";

/// The file the weapon comparison reads its balances from (`weapon_gen_parity.py`).
const WEAPON_PACKAGE: &str = "WillowGame/CookedPCConsole/Startup.upk";

#[derive(Default)]
struct Line {
    balance: Option<String>,
    cooked: Option<String>,
    stage: Option<i32>,
    seed: Option<u32>,
    count: u32,
    weapons: Option<String>,
    seeds: u32,
    requests: Option<String>,
    output: Option<String>,
    data: Option<String>,
    level: Option<i32>,
    overlay: Option<String>,
    colours: Option<String>,
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
        let to_i32 = |n: i64| i32::try_from(n).map_err(|_| format!("{arg}: out of range: {n}"));
        let to_count = |n: i64| {
            u32::try_from(n)
                .ok()
                .filter(|c| *c >= 1)
                .ok_or_else(|| format!("{arg}: must be at least 1: {n}"))
        };
        match arg {
            "--cooked" => line.cooked = Some(value()?),
            "--weapons" => line.weapons = Some(value()?),
            "--requests" => line.requests = Some(value()?),
            "--output" => line.output = Some(value()?),
            "--data" => line.data = Some(value()?),
            "--overlay" => line.overlay = Some(value()?),
            "--colours" => line.colours = Some(value()?),
            "--stage" => line.stage = Some(to_i32(number(value()?)?)?),
            "--level" => line.level = Some(to_i32(number(value()?)?)?),
            "--seed" => {
                let n = number(value()?)?;
                line.seed =
                    Some(u32::try_from(n).map_err(|_| format!("{arg}: out of range: {n}"))?);
            }
            "--count" => line.count = to_count(number(value()?)?)?,
            "--seeds" => line.seeds = to_count(number(value()?)?)?,
            _ => return Err(USAGE.to_string()),
        }
        i += 2;
    }
    Ok(line)
}

pub fn run(command: &Command, args: &[String], mode: ParityMode) -> ExitCode {
    let line = match parse(command, args) {
        Ok(line) => line,
        Err(message) => return fail(&message),
    };
    let Some(cooked) = line.cooked.clone() else {
        return fail(USAGE);
    };
    let forms = usize::from(line.balance.is_some())
        + usize::from(line.weapons.is_some())
        + usize::from(line.requests.is_some())
        + usize::from(line.data.is_some());
    if forms != 1
        || (line.balance.is_some() && (line.stage.is_none() || line.seed.is_none()))
        || (line.requests.is_some() != line.output.is_some())
        || (line.overlay.is_some() && line.data.is_none())
    {
        return fail(USAGE);
    }
    let card = command.flag == "--item-card";
    match execute(&line, Path::new(&cooked), mode, card) {
        Ok(()) => ExitCode::SUCCESS,
        Err(ItemError::NotYet(e)) => {
            eprintln!("ow-package (Rust): {}: {e}", command.flag);
            ExitCode::from(NOT_PORTED)
        }
        Err(e) => fail(&super::properties::error_text(&e)),
    }
}

fn fail(message: &str) -> ExitCode {
    eprintln!("ow-package: {message}");
    ExitCode::from(1)
}

fn bad(message: impl Into<String>) -> ItemError {
    ItemError::BadData(message.into())
}

/// `game_colours.read_file`'s `card_red_text`, or the neutral grey: the file's colour when it is valid
/// `{"colours": {...}}` JSON with a `#RRGGBB` value for the key.
fn red_text(file: Option<&str>) -> Result<[u8; 3], ItemError> {
    let Some(file) = file else {
        return Ok(NEUTRAL_RED_TEXT);
    };
    let text = std::fs::read_to_string(file).map_err(|e| bad(format!("{file}: {e}")))?;
    let Ok(top) = parse_json(&text) else {
        return Ok(NEUTRAL_RED_TEXT);
    };
    let Some(hex) = top.get("colours").and_then(|c| c.text("card_red_text")) else {
        return Ok(NEUTRAL_RED_TEXT);
    };
    let digits = hex
        .strip_prefix('#')
        .filter(|d| d.len() == 6 && d.chars().all(|c| c.is_ascii_hexdigit()));
    let Some(d) = digits else {
        return Ok(NEUTRAL_RED_TEXT);
    };
    let channel = |i: usize| u8::from_str_radix(&d[i..i + 2], 16).map_err(|e| bad(e.to_string()));
    Ok([channel(0)?, channel(2)?, channel(4)?])
}

/// Whether a store error says only that a class (or the package that would hold it) is not in the install.
fn class_missing(e: &DefError) -> bool {
    matches!(
        e,
        DefError::Reflect(ReflectError::ClassNotFound(_))
            | DefError::Store(StoreError::PackageNotFound(_))
    )
}

fn snapshot(stage: i32) -> Result<BalanceSnapshot, ItemError> {
    Ok(BalanceSnapshot::new(
        Playthrough::NORMAL,
        OpLevel::new(0)?,
        PlayerCount::SINGLE,
        ExpLevel::new(stage.clamp(1, 80)),
        GameStage(stage),
        AwesomeLevel::new(0),
        LevelCapUpgrades::ALL,
    )?)
}

/// The world the command evaluates in (module documentation).
enum World {
    Offline(Box<OfflineWorld>),
    Bare(Box<BareWorld>),
}

/// A world with no engine objects, over the store only: used when the offline world's classes are not laid out
/// (an install without the code packages). Every evaluation that needs a world object answers `NotYet`.
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
            ow_items::stats::RECORD,
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
            ow_items::stats::RECORD,
        )
        .into())
    }
    fn rng(&mut self, s: CrtStream) -> Result<&mut dyn Rand15, AttrError> {
        Err(NotYet::layer_absent(
            format!("a random source for {s:?}"),
            "ow-world",
            ow_items::stats::RECORD,
        )
        .into())
    }
}

struct Session {
    reg: Registry,
    weapons: Vec<String>,
    data: RollData,
    sys: AttributeSystem,
    /// The offline world when its classes lay out from the store, else a world over the store alone.
    world: World,
    localization: Localization,
    red: [u8; 3],
    mode: ParityMode,
}

impl Session {
    fn open(cooked: &Path, line: &Line, mode: ParityMode) -> Result<Session, ItemError> {
        let mut defs = DefStore::open(
            PathBuf::from(cooked),
            IdentityPolicy::FirstInScanOrder,
            mode,
        )?;
        let census = defs.census()?;
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
        let data = RollData::new(&input)?;
        let reg = build_from_census(input, &python_path_text(&root.to_string_lossy()));
        let sys = AttributeSystem::standard()?;
        let mut space = ObjectSpace::new();
        // The store-only world is the fallback for an install that lacks one of these classes (or its package);
        // any other store error is passed on, not hidden by the fallback.
        let mut laid_out = true;
        for class in [
            PLAYER_CONTROLLER_CLASS,
            REPLICATION_INFO_CLASS,
            GAME_REPLICATION_INFO_CLASS,
            GLOBALS_CLASS,
        ] {
            match defs.class_id(&mut space, class) {
                Ok(_) => {}
                Err(e) if class_missing(&e) => {
                    laid_out = false;
                    break;
                }
                Err(e) => return Err(e.into()),
            }
        }
        let world = if laid_out {
            World::Offline(Box::new(OfflineWorld::new(
                space,
                Some(defs),
                snapshot(1)?,
            )?))
        } else {
            World::Bare(Box::new(BareWorld {
                space,
                defs,
                slots: GlobalSlots::default(),
            }))
        };
        Ok(Session {
            reg,
            weapons: weapons.into_iter().collect(),
            data,
            sys,
            world,
            localization: Localization::new(cooked),
            red: red_text(line.colours.as_deref())?,
            mode,
        })
    }

    fn roll(&mut self, balance: &str, stage: i32, seed: u32) -> Result<RollResult, ItemError> {
        let snap = snapshot(stage)?;
        let path = ObjectPath::parse(balance).map_err(|e| bad(format!("{balance}: {e}")))?;
        // A Reference roll reads tag streams and needs no world object, so the store-only world serves it too.
        let world: &mut dyn AttrWorld = match &mut self.world {
            World::Offline(w) => w.as_mut(),
            World::Bare(w) => w.as_mut(),
        };
        let mut rng = CrtRand::new(seed);
        let mut rw = RollWorld::new(world, &mut rng, false);
        let mut cx = RollCx::new(
            &snap,
            EvalCx::new(&mut rw),
            &self.sys,
            &self.data,
            self.mode,
        );
        roll(&self.reg, &path, &mut cx)
    }

    /// One request's JSON line (module documentation).
    fn gun_line(
        &mut self,
        balance: &str,
        stage: i32,
        seed: u32,
        card: bool,
    ) -> Result<String, ItemError> {
        if self.mode != ParityMode::Reference {
            return Err(NotYet::not_ported(
                "an Off-mode weapon card from a balance (the Off roll needs the item's instance first; use --data)",
                ow_items::stats::RECORD,
            )
            .into());
        }
        let rolled = match self.roll(balance, stage, seed)? {
            RollResult::Item(r) => r,
            RollResult::NotRollable(n) => {
                return Ok(outcome_line(balance, stage, seed, "not_rollable", n.reason));
            }
            _ => return Err(bad("a roll result this command does not know")),
        };
        if rolled.kind != "weapon" {
            return Err(NotYet::not_ported(
                format!("the {} card of a balance roll (use --data)", rolled.kind),
                ow_items::stats::RECORD,
            )
            .into());
        }
        self.weapon_line(&rolled, stage, seed, card)
    }

    fn weapon_line(
        &self,
        rolled: &Rolled,
        stage: i32,
        seed: u32,
        card: bool,
    ) -> Result<String, ItemError> {
        let stats = StatsData::new(&self.data);
        let recipe = WeaponRecipe::from_rolled(rolled);
        let text = CardText::new(self.red, Some(&self.localization));
        let mut out: DetMap<String, Pv> = DetMap::new();
        let roll = rolled.to_weapon_pv(seed, &self.data);
        for key in [
            "manufacturer",
            "weapon_type",
            "name",
            "prefix",
            "title",
            "notes",
        ] {
            out.insert(key.to_string(), roll.at(key).clone());
        }
        let mut parts = DetMap::new();
        for s in &rolled.slots {
            let mut p = DetMap::new();
            p.insert("part".to_string(), Pv::Str(s.part.clone()));
            p.insert("weight".to_string(), Pv::Float(s.weight));
            p.insert(
                "candidates".to_string(),
                Pv::Int(i64::try_from(s.candidates.len()).unwrap_or(i64::MAX)),
            );
            parts.insert(s.slot.clone(), Pv::Map(p));
        }
        out.insert("parts".to_string(), Pv::Map(parts));
        let a = assembly(&stats, &recipe)?;
        out.insert("gestalt".to_string(), a.gestalt);
        out.insert(
            "gestalt_fragments".to_string(),
            Pv::List(a.fragments.into_iter().map(Pv::Str).collect()),
        );
        out.insert("material".to_string(), a.material);
        out.insert("balance".to_string(), Pv::Str(rolled.balance.clone()));
        out.insert("stage".to_string(), Pv::Int(i64::from(stage)));
        out.insert("seed".to_string(), Pv::Int(i64::from(seed)));
        out.insert("spawn_stage".to_string(), Pv::Null);
        out.insert("stage_cap".to_string(), Pv::Null);
        if card {
            out.insert(
                "card".to_string(),
                weapon_card(&stats, &recipe, rolled.stage, &text)?.to_pv(),
            );
        } else {
            out.insert(
                "stats".to_string(),
                weapon_stats_pv(&stats, &recipe, rolled.stage)?,
            );
        }
        Ok(Pv::Map(out).to_json())
    }
}

fn weapon_stats_pv(
    stats: &StatsData<'_>,
    recipe: &WeaponRecipe,
    level: i64,
) -> Result<Pv, ItemError> {
    let mut v = Values::new(stats, recipe.maker(), level);
    let a = weapon_attributes(&mut v, recipe)?;
    let floats = |m: &DetMap<String, f64>| {
        Pv::Map(m.iter().map(|(k, x)| (k.clone(), Pv::Float(*x))).collect())
    };
    let mut out = DetMap::new();
    out.insert("attributes".to_string(), floats(&a.values));
    out.insert("base".to_string(), floats(&a.base));
    out.insert(
        "unresolved".to_string(),
        Pv::List(v.unresolved.iter().cloned().map(Pv::Str).collect()),
    );
    Ok(Pv::Map(out))
}

/// `{"line":<n>,"error":"<why>"}`: a request line that names no gun.
fn error_line(line: usize, why: &str) -> String {
    let mut m = DetMap::new();
    m.insert(
        "line".to_string(),
        Pv::Int(i64::try_from(line).unwrap_or(i64::MAX)),
    );
    m.insert("error".to_string(), Pv::Str(why.to_string()));
    Pv::Map(m).to_json()
}

fn outcome_line(balance: &str, stage: i32, seed: u32, key: &str, value: &str) -> String {
    let mut m = DetMap::new();
    m.insert("balance".to_string(), Pv::Str(balance.to_string()));
    m.insert("seed".to_string(), Pv::Int(i64::from(seed)));
    m.insert("stage".to_string(), Pv::Int(i64::from(stage)));
    m.insert(key.to_string(), Pv::Str(value.to_string()));
    Pv::Map(m).to_json()
}

/// A `--data` file's item.
struct DataItem {
    kind: ItemKind,
    balance: Option<String>,
    definition: String,
    manufacturer: Option<String>,
    parts: Vec<(String, String)>,
    prefix: Option<String>,
    title: Option<String>,
    level: i32,
}

fn read_data(file: &str, level: Option<i32>) -> Result<DataItem, ItemError> {
    let text = std::fs::read_to_string(file).map_err(|e| bad(format!("{file}: {e}")))?;
    let value = parse_json(&text).map_err(|e| bad(format!("{file}: {e}")))?;
    if !matches!(value, Json::Object(_)) {
        return Err(bad(format!("{file}: not a JSON object")));
    }
    let text_of = |k: &str| value.text(k);
    let kind = match text_of("kind").as_deref().unwrap_or("weapon") {
        "weapon" => ItemKind::Weapon,
        "shield" => ItemKind::Shield,
        "grenade_mod" => ItemKind::GrenadeMod,
        "class_mod" => ItemKind::ClassMod,
        "relic" => ItemKind::Relic,
        other => return Err(bad(format!("{file}: unknown kind {other:?}"))),
    };
    let definition = text_of("definition")
        .or_else(|| text_of("type"))
        .ok_or_else(|| bad(format!("{file}: no definition (or type)")))?;
    let slots: &[&str] = if kind == ItemKind::Weapon {
        &WEAPON_SLOTS
    } else {
        &ITEM_SLOTS
    };
    let mut parts = Vec::new();
    if let Some(Json::Object(p)) = value.get("parts") {
        for slot in slots {
            if let Some((_, Json::Str(path))) = p.iter().find(|(n, _)| n == slot)
                && !path.is_empty()
            {
                parts.push((slot.to_string(), path.clone()));
            }
        }
        for (name, _) in p {
            if !slots.contains(&name.as_str()) {
                return Err(bad(format!(
                    "{file}: slot {name:?} is not a slot of the kind"
                )));
            }
        }
    }
    let level = match level {
        Some(l) => l,
        None => match value.get("game_stage") {
            Some(Json::Int(n)) => {
                i32::try_from(*n).map_err(|_| bad(format!("{file}: game_stage {n}")))?
            }
            _ => return Err(bad(format!("{file}: no integer game_stage (give --level)"))),
        },
    };
    Ok(DataItem {
        kind,
        balance: text_of("balance"),
        definition,
        manufacturer: text_of("manufacturer"),
        parts,
        prefix: text_of("prefix"),
        title: text_of("title"),
        level,
    })
}

fn definition_data(item: &DataItem) -> Result<DefinitionData, ItemError> {
    let path = |p: &str| ObjectPath::parse(p).map_err(|e| bad(format!("{p}: {e}")));
    let opt = |p: &Option<String>| p.as_deref().map(path).transpose();
    let slots: &[&str] = if item.kind == ItemKind::Weapon {
        &WEAPON_SLOTS
    } else {
        &ITEM_SLOTS
    };
    let mut parts: [Option<ObjectPath>; 9] = Default::default();
    for (slot, part) in &item.parts {
        if let Some(i) = slots.iter().position(|s| s == slot) {
            parts[i] = Some(path(part)?);
        }
    }
    Ok(DefinitionData {
        kind: item.kind,
        balance: opt(&item.balance)?,
        definition: path(&item.definition)?,
        manufacturer: opt(&item.manufacturer)?,
        manufacturer_grade: item.level,
        parts,
        prefix: opt(&item.prefix)?,
        title: opt(&item.title)?,
        game_stage: GameStage(item.level),
        unique_id: 0,
    })
}

fn data_line(session: &mut Session, line: &Line, card: bool) -> Result<String, ItemError> {
    let file = line.data.as_deref().unwrap_or_default();
    let item = read_data(file, line.level)?;
    let overlay = match &line.overlay {
        Some(o) => Some(Overlay::read_file(Path::new(o))?),
        None => None,
    };
    if session.mode != ParityMode::Reference {
        return off_data_line(session, &item);
    }
    let mut stats = StatsData::new(&session.data);
    if let Some(o) = &overlay {
        stats = stats.with_overlay(o);
    }
    let level = i64::from(item.level);
    if item.kind == ItemKind::Weapon {
        let mut recipe = WeaponRecipe::new(&item.definition)
            .with_names(item.prefix.as_deref(), item.title.as_deref());
        recipe.balance = item.balance.clone();
        recipe.manufacturer = item.manufacturer.clone();
        for (slot, part) in &item.parts {
            recipe = recipe.with_part(slot, part);
        }
        if !card {
            return Ok(weapon_stats_pv(&stats, &recipe, level)?.to_json());
        }
        let text = CardText::new(session.red, Some(&session.localization));
        let c = weapon_card(&stats, &recipe, level, &text)?;
        let mut pv = c.to_pv();
        if let Pv::Map(m) = &mut pv {
            m.insert("fun_stats_lines".to_string(), c.fun_stats_lines());
            m.insert(
                "unresolved_attributes".to_string(),
                Pv::List(c.unresolved.iter().cloned().map(Pv::Str).collect()),
            );
        }
        return Ok(pv.to_json());
    }
    let parts: Vec<&str> = item.parts.iter().map(|(_, p)| p.as_str()).collect();
    let names: Vec<&str> = [item.prefix.as_deref(), item.title.as_deref()]
        .into_iter()
        .flatten()
        .collect();
    let s = item_stats(&stats, &item.definition, &parts, &names, level)?;
    let mut out = DetMap::new();
    let stacks: DetMap<String, Pv> = s
        .external
        .0
        .iter()
        .map(|(k, b)| {
            let mut m = DetMap::new();
            for (n, x) in [
                ("pre_add", b.pre_add),
                ("post_add", b.post_add),
                ("scale_up", b.scale_up),
                ("scale_down", b.scale_down),
            ] {
                m.insert(n.to_string(), Pv::Float(x));
            }
            (k.clone(), Pv::Map(m))
        })
        .collect();
    out.insert("external".to_string(), Pv::Map(stacks));
    out.insert(
        "slots_reaching_the_owner".to_string(),
        Pv::List(
            s.reached
                .iter()
                .map(|(n, g)| {
                    let mut m = DetMap::new();
                    m.insert("slot".to_string(), Pv::Str(n.clone()));
                    m.insert("grade".to_string(), Pv::Float(*g));
                    Pv::Map(m)
                })
                .collect(),
        ),
    );
    if item.kind == ItemKind::Shield {
        let shown: Vec<Pv> = s
            .shield_card(session.mode)?
            .into_iter()
            .map(|(label, v)| {
                let mut m = DetMap::new();
                m.insert("label".to_string(), Pv::Str(label.to_string()));
                m.insert("value".to_string(), Pv::Str(shield_number_text(v)));
                m.insert("raw".to_string(), Pv::Float(v));
                Pv::Map(m)
            })
            .collect();
        out.insert("stats".to_string(), Pv::List(shown));
    }
    Ok(Pv::Map(out).to_json())
}

/// `--data` in Off mode: the instance through `ow-attr` (`instantiate_stats`).
fn off_data_line(session: &mut Session, item: &DataItem) -> Result<String, ItemError> {
    let data = definition_data(item)?;
    let World::Offline(world) = &mut session.world else {
        return Err(NotYet::layer_absent(
            "the offline world (its classes are not laid out from this install)",
            "ow-object reflection of the WillowGame classes",
            ow_items::stats::RECORD,
        )
        .into());
    };
    let mut cx = EvalCx::new(world.as_mut());
    let instance = instantiate_stats(&data, item.level, &session.sys, &mut cx, &DetMap::new())?;
    let mut out = DetMap::new();
    if item.kind == ItemKind::Weapon {
        let numbers = instance_numbers(instance.object, &session.sys, &mut cx)?;
        for (field, value, shown) in numbers {
            let mut m = DetMap::new();
            m.insert("value".to_string(), value.map_or(Pv::Null, Pv::Float));
            m.insert("display".to_string(), shown.map_or(Pv::Null, |n| n.to_pv()));
            out.insert(field.to_string(), Pv::Map(m));
        }
    } else if item.kind == ItemKind::Shield {
        for (label, v) in instance.external.shield_card(session.mode)? {
            out.insert(label.to_string(), Pv::Str(shield_number_text(v)));
        }
    }
    Ok(Pv::Map(out).to_json())
}

fn execute(line: &Line, cooked: &Path, mode: ParityMode, card: bool) -> Result<(), ItemError> {
    let mut session = Session::open(cooked, line, mode)?;
    let stdout = std::io::stdout();
    if line.data.is_some() {
        let text = data_line(&mut session, line, card)?;
        let mut out = stdout.lock();
        let _ = writeln!(out, "{text}");
        return Ok(());
    }
    if let Some(balance) = &line.balance {
        let (Some(stage), Some(seed)) = (line.stage, line.seed) else {
            return Err(bad(USAGE));
        };
        let mut out = stdout.lock();
        for k in 0..line.count {
            let text = session.gun_line(balance, stage, seed.wrapping_add(k), card)?;
            // A closed pipe is ignored, as the other commands do.
            let _ = writeln!(out, "{text}");
        }
        let _ = out.flush();
        return Ok(());
    }
    // A request is a gun to roll, or the line already known for it (a request line that is not one, or a cap): one
    // output line per request, so one bad request does not stop the batch.
    enum Request {
        Gun(String, i32, u32),
        Known(String),
    }
    let (requests, file): (Vec<Request>, &str) = if let Some(f) = &line.weapons {
        let mut r = Vec::new();
        for b in session.weapons.clone() {
            for stage in CENSUS_STAGES {
                for seed in 1..=line.seeds {
                    r.push(Request::Gun(b.clone(), stage, seed));
                }
            }
        }
        (r, f)
    } else {
        let input = line.requests.as_deref().unwrap_or_default();
        let text = std::fs::read_to_string(input).map_err(|e| bad(format!("{input}: {e}")))?;
        let mut r = Vec::new();
        for (n, l) in text.lines().enumerate() {
            let f: Vec<&str> = l.split_whitespace().collect();
            let parsed = match f.as_slice() {
                [] => continue,
                ["balance", path, stage, seed] => match (stage.parse::<i32>(), seed.parse::<u32>())
                {
                    (Ok(stage), Ok(seed)) => Request::Gun(path.to_string(), stage, seed),
                    _ => Request::Known(error_line(n + 1, "a stage or seed that is not a number")),
                },
                ["balance", path, stage, seed, _] => {
                    match (stage.parse::<i32>(), seed.parse::<u32>()) {
                        (Ok(stage), Ok(seed)) => Request::Known(outcome_line(
                            path,
                            stage,
                            seed,
                            "not_yet",
                            "a stage cap (Reference applies none; W4-ROLL)",
                        )),
                        _ => Request::Known(error_line(
                            n + 1,
                            "a stage or seed that is not a number",
                        )),
                    }
                }
                _ => Request::Known(error_line(
                    n + 1,
                    "not a `balance <path> <stage> <seed>` line",
                )),
            };
            r.push(parsed);
        }
        (r, line.output.as_deref().unwrap_or_default())
    };
    let mut text = String::new();
    for request in requests {
        let gun = match request {
            Request::Known(known) => known,
            Request::Gun(balance, stage, seed) => {
                match session.gun_line(&balance, stage, seed, card) {
                    Ok(gun) => gun,
                    Err(ItemError::NotYet(n)) => {
                        outcome_line(&balance, stage, seed, "not_yet", &n.missing.to_string())
                    }
                    Err(e) => outcome_line(
                        &balance,
                        stage,
                        seed,
                        "error",
                        &super::properties::error_text(&e),
                    ),
                }
            }
        };
        text.push_str(&gun);
        text.push('\n');
    }
    std::fs::write(file, text).map_err(|e| bad(format!("{file}: {e}")))
}

/// A small JSON reader for the command's own inputs (`--data`, `--colours`): objects keep their key order; a number
/// without `.` or an exponent is an integer, as `json.loads` reads it. `Err` with the byte offset of the problem.
fn parse_json(text: &str) -> Result<Json, String> {
    let b = text.trim_start_matches('\u{feff}').as_bytes();
    let mut p = JsonReader { b, i: 0 };
    let v = p.value(0)?;
    p.ws();
    if p.i != b.len() {
        return Err(format!("trailing text at byte {}", p.i));
    }
    Ok(v)
}

#[derive(Debug, Clone, PartialEq)]
enum Json {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    List(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    fn text(&self, key: &str) -> Option<String> {
        match self.get(key) {
            Some(Json::Str(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        }
    }
}

struct JsonReader<'b> {
    b: &'b [u8],
    i: usize,
}

impl JsonReader<'_> {
    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }

    fn err<T>(&self, what: &str) -> Result<T, String> {
        Err(format!("{what} at byte {}", self.i))
    }

    fn value(&mut self, depth: u32) -> Result<Json, String> {
        if depth > 64 {
            return self.err("nesting deeper than 64");
        }
        self.ws();
        match self.b.get(self.i) {
            None => self.err("a value expected"),
            Some(b'{') => {
                self.i += 1;
                let mut out = Vec::new();
                self.ws();
                if self.b.get(self.i) == Some(&b'}') {
                    self.i += 1;
                    return Ok(Json::Object(out));
                }
                loop {
                    self.ws();
                    let Json::Str(k) = self.string()? else {
                        return self.err("a key expected");
                    };
                    self.ws();
                    if self.b.get(self.i) != Some(&b':') {
                        return self.err("':' expected");
                    }
                    self.i += 1;
                    let v = self.value(depth + 1)?;
                    out.push((k, v));
                    self.ws();
                    match self.b.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b'}') => {
                            self.i += 1;
                            return Ok(Json::Object(out));
                        }
                        _ => return self.err("',' or '}' expected"),
                    }
                }
            }
            Some(b'[') => {
                self.i += 1;
                let mut out = Vec::new();
                self.ws();
                if self.b.get(self.i) == Some(&b']') {
                    self.i += 1;
                    return Ok(Json::List(out));
                }
                loop {
                    out.push(self.value(depth + 1)?);
                    self.ws();
                    match self.b.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b']') => {
                            self.i += 1;
                            return Ok(Json::List(out));
                        }
                        _ => return self.err("',' or ']' expected"),
                    }
                }
            }
            Some(b'"') => self.string(),
            Some(b't') if self.b[self.i..].starts_with(b"true") => {
                self.i += 4;
                Ok(Json::Bool(true))
            }
            Some(b'f') if self.b[self.i..].starts_with(b"false") => {
                self.i += 5;
                Ok(Json::Bool(false))
            }
            Some(b'n') if self.b[self.i..].starts_with(b"null") => {
                self.i += 4;
                Ok(Json::Null)
            }
            Some(_) => {
                let start = self.i;
                while self.i < self.b.len()
                    && matches!(
                        self.b[self.i],
                        b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9'
                    )
                {
                    self.i += 1;
                }
                let t = std::str::from_utf8(&self.b[start..self.i]).map_err(|e| e.to_string())?;
                if t.is_empty() {
                    return self.err("a value expected");
                }
                if !t.contains(['.', 'e', 'E'])
                    && let Ok(n) = t.parse::<i64>()
                {
                    return Ok(Json::Int(n));
                }
                t.parse::<f64>()
                    .map(Json::Float)
                    .map_err(|_| format!("bad number {t} at byte {start}"))
            }
        }
    }

    fn string(&mut self) -> Result<Json, String> {
        if self.b.get(self.i) != Some(&b'"') {
            return self.err("a string expected");
        }
        self.i += 1;
        let mut out: Vec<u16> = Vec::new();
        let mut bytes: Vec<u8> = Vec::new();
        let flush = |bytes: &mut Vec<u8>, out: &mut Vec<u16>| -> Result<(), String> {
            let s = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
            out.extend(s.encode_utf16());
            bytes.clear();
            Ok(())
        };
        loop {
            let Some(&c) = self.b.get(self.i) else {
                return self.err("an unterminated string");
            };
            self.i += 1;
            match c {
                b'"' => {
                    flush(&mut bytes, &mut out)?;
                    return String::from_utf16(&out)
                        .map(Json::Str)
                        .map_err(|e| e.to_string());
                }
                b'\\' => {
                    flush(&mut bytes, &mut out)?;
                    let Some(&e) = self.b.get(self.i) else {
                        return self.err("an unterminated escape");
                    };
                    self.i += 1;
                    let unit = match e {
                        b'"' => u16::from(b'"'),
                        b'\\' => u16::from(b'\\'),
                        b'/' => u16::from(b'/'),
                        b'b' => 8,
                        b'f' => 12,
                        b'n' => 10,
                        b'r' => 13,
                        b't' => 9,
                        b'u' => {
                            let hex = self
                                .b
                                .get(self.i..self.i + 4)
                                .ok_or_else(|| "a short unicode escape".to_string())?;
                            let hex = std::str::from_utf8(hex).map_err(|e| e.to_string())?;
                            self.i += 4;
                            u16::from_str_radix(hex, 16).map_err(|e| e.to_string())?
                        }
                        _ => return self.err("an unknown escape"),
                    };
                    out.push(unit);
                }
                other => bytes.push(other),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::{Dispatch, dispatch};

    fn command(flag: &str) -> &'static Command {
        let args = vec![flag.to_string()];
        match dispatch(&args) {
            Dispatch::Run(c) => c,
            Dispatch::Usage => panic!("{flag} is a command"),
        }
    }

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn bad_command_lines_fail_with_code_1() {
        for flag in ["--item-card", "--item-stats"] {
            let c = command(flag);
            for line in [
                &[flag][..],
                &[flag, "X.Y", "--cooked"],
                &[flag, "X.Y", "--cooked", ".", "--stage", "8"],
                &[flag, "X.Y", "--cooked", ".", "--stage", "x", "--seed", "1"],
                &[flag, "X.Y", "--cooked", ".", "--stage", "8", "--seed", "-1"],
                &[flag, "--requests", "r.txt", "--cooked", "."],
                &[
                    flag,
                    "--weapons",
                    "w.jsonl",
                    "--data",
                    "d.json",
                    "--cooked",
                    ".",
                ],
                &[
                    flag,
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
                &[flag, "--bogus"],
            ] {
                assert_eq!(
                    run(c, &args(line), ParityMode::Reference),
                    ExitCode::from(1),
                    "{line:?}"
                );
            }
        }
    }

    #[test]
    fn the_neutral_red_is_used_without_a_valid_colours_file() {
        assert_eq!(red_text(None).unwrap(), NEUTRAL_RED_TEXT);
        let dir =
            std::env::temp_dir().join(format!("ow-cli-items-stats-colours-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("c.json");
        std::fs::write(
            &file,
            r##"{"schemaVersion": 1, "colours": {"card_red_text": "#1A2B3C"}}"##,
        )
        .unwrap();
        assert_eq!(
            red_text(Some(&file.to_string_lossy())).unwrap(),
            [0x1A, 0x2B, 0x3C]
        );
        std::fs::write(&file, r#"{"colours": {"card_red_text": "red"}}"#).unwrap();
        assert_eq!(
            red_text(Some(&file.to_string_lossy())).unwrap(),
            NEUTRAL_RED_TEXT
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn every_request_writes_one_line_and_a_bad_one_does_not_stop_the_batch() {
        let dir = std::env::temp_dir().join(format!("ow-cli-items-stats-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let d = dir.to_string_lossy().to_string();
        let out = dir.join("w.jsonl");
        let o = out.to_string_lossy().to_string();
        let line = args(&["--item-card", "--weapons", &o, "--cooked", &d]);
        assert_eq!(
            run(command("--item-card"), &line, ParityMode::Reference),
            ExitCode::SUCCESS
        );
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "");
        let requests = dir.join("r.txt");
        // A cap is not there yet, a line naming no gun is an error, a balance no package holds is not rollable: one
        // line each, in order, and the batch finishes.
        std::fs::write(
            &requests,
            "balance X.Y 8 1 50\nnonsense\nbalance X.Y eight 1\nbalance X.Y 8 1\n",
        )
        .unwrap();
        let r = requests.to_string_lossy().to_string();
        let line = args(&[
            "--item-card",
            "--requests",
            &r,
            "--output",
            &o,
            "--cooked",
            &d,
        ]);
        assert_eq!(
            run(command("--item-card"), &line, ParityMode::Reference),
            ExitCode::SUCCESS
        );
        let written = std::fs::read_to_string(&out).unwrap();
        let lines: Vec<&str> = written.lines().collect();
        assert_eq!(lines.len(), 4, "{written}");
        assert!(
            lines[0].contains("\"not_yet\":") && lines[0].contains("stage cap"),
            "{}",
            lines[0]
        );
        assert!(
            lines[1].starts_with("{\"error\":") && lines[1].contains("\"line\":2"),
            "{}",
            lines[1]
        );
        assert!(lines[2].contains("\"line\":3"), "{}", lines[2]);
        // An empty install holds no balance: the store-only world rolls it, and it is not rollable.
        assert!(
            lines[3].starts_with("{\"balance\":\"X.Y\"") && lines[3].contains("\"not_rollable\":"),
            "{}",
            lines[3]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
