//! Owner: lane W5-MAPLOAD. A map's placed actors in a [`World`]: the persistent level's actor list, read from the
//! map file, each actor loaded through the definition store (its class defaults, its archetype in `ParityMode::Off`,
//! its tags) and placed in the world's level list, the persistent level's `WorldInfo` first; then the actors of the
//! levels its `WorldInfo.StreamingLevels` name. AI-assisted (Claude Code). Decision `2026-11-16-76`.
//!
//! **What is read, and how it was checked.** A level is the export `TheWorld.PersistentLevel` (class `Engine.Level`)
//! of a map file. Its untagged tail starts with the actor list: an owner reference (the level), a count, then the
//! references (NATIVE_MAP_LOAD.md, section 1, recovered from the packages with structural oracles over all ten
//! Sanctuary level files: the owner is the level, item 0 is the level's `WorldInfo`, every other non-null item is an
//! export whose outer is the level, and the level URL's protocol string follows the list). [`read_level_list`]
//! refuses a tail that does not meet those oracles. The list is the order `AllActors` walks.
//!
//! **What the load does.** The persistent level's item 0 is placed with `Actors::place_world_info`, every other
//! non-null item with `Actors::place`, in list order. A streaming level's own `WorldInfo` (its item 0) is not placed:
//! the record walks a streamed level from index 1 (NATIVE_ENGINE_CORE.md, "Iterators"). An actor the definition store
//! refuses to load (a `NotYet` included: in Off, a template archetype the store cannot apply yet) is not placed and is
//! counted by class with its first reason ([`LevelReport::refused`]); the load goes on.
//!
//! **What is not done** (each a `NotYet` in [`MapReport::pending`], in both parity modes: the C++ has no map load, so
//! there is no Reference stand-in to keep):
//! - the begin-play pass over the placed actors (NATIVE_MAP_LOAD.md, section 2): the world has not begun play after a
//!   load, so a spawn before it runs no begin-play events (NATIVE_ENGINE_CORE.md, "Actor.Spawn" edge cases);
//! - the game info, which the engine spawns into `WorldInfo.Game` (NATIVE_MAP_LOAD.md, section 3): the world's game
//!   info queries keep answering `NotYet`;
//! - the local player's controller and pawn, and the engine's local player list (NATIVE_MAP_LOAD.md, section 4).
//!
//! The analyst note left those `NotYet` on purpose (clean room): a fresh implementer codes them from the note.
//! [`bringup`] is that implementer's module (lane MAP-LOAD-NATIVES, decision 2026-11-19-04): the note settles only the
//! game info routine's "already has one" branch, so the three steps stay `NotYet` with their open questions named.
//!
//! **UNVERIFIED.** That every level `StreamingLevels` names is loaded at map load (the game streams levels by their
//! class and Kismet; Sanctuary's are all loaded while it is played, which is not checked here); that the levels'
//! lists concatenated in that order are the order the game's iterators visit; the first-dynamic index, left at 0.

use std::fmt;

use ow_core::{DetMap, IdentError, NotYet, PackageName};
use ow_defs::store::DefError;
use ow_object::ids::ObjRef;
use ow_object::prefix::PropertyPrefix;
use ow_object::value::Value;
use ow_package::PackageId;
use ow_package::package::{Package, PackageError};
use ow_package::props::{self, NoArrayTypes};
use ow_package::store::StoreError;
use ow_vm::error::VmError;

use super::World;

/// The record of the native steps the load leaves `NotYet`.
pub const RECORD: &str = "NATIVE_MAP_LOAD.md";

/// A map file's persistent level.
pub const LEVEL_PATH: &str = "TheWorld.PersistentLevel";

/// The most actors a level list may hold (a bound on the count before it is trusted; Sanctuary's largest is 715).
pub const MAX_LEVEL_ACTORS: usize = 1 << 20;

/// Why a map did not load.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum MapError {
    /// Loading a map file failed; only `PackageNotFound` permits skipping a streaming level.
    Store {
        map: PackageName,
        cause: StoreError,
    },
    InvalidName {
        name: String,
        cause: IdentError,
    },
    Package {
        map: PackageName,
        cause: PackageError,
    },
    MissingLevel(PackageName),
    WrongLevelClass {
        map: PackageName,
        class: String,
    },
    Definition(DefError),
    WorldNotEmpty,
    /// A level's actor list does not meet the recovered layout (NATIVE_MAP_LOAD.md, section 1).
    Layout {
        level: String,
        reason: String,
    },
    /// A level's first actor did not load or is not a `WorldInfo`.
    WorldInfo(String),
    /// The world refused a placement or a read (`Actors::place`, a property read).
    Vm(VmError),
}

impl fmt::Display for MapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MapError::Store { map, cause } => write!(f, "map load: {map}: {cause}"),
            MapError::InvalidName { name, cause } => {
                write!(f, "map load: invalid package name {name:?}: {cause}")
            }
            MapError::Package { map, cause } => write!(f, "map load: {map}: {cause}"),
            MapError::MissingLevel(map) => write!(f, "map load: {map} holds no {LEVEL_PATH}"),
            MapError::WrongLevelClass { map, class } => write!(
                f,
                "map load: {map}'s {LEVEL_PATH} is a {class}, not a Level"
            ),
            MapError::Definition(e) => write!(f, "map load: a StreamingLevels entry: {e}"),
            MapError::WorldNotEmpty => write!(
                f,
                "map load: the world already has actors (one map per fresh world)"
            ),
            MapError::Layout { level, reason } => {
                write!(f, "map load: {level}: {reason} ({RECORD}, section 1)")
            }
            MapError::WorldInfo(e) => write!(f, "map load: the level's WorldInfo: {e}"),
            MapError::Vm(e) => write!(f, "map load: {e}"),
        }
    }
}

impl std::error::Error for MapError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Store { cause, .. } => Some(cause),
            Self::InvalidName { cause, .. } => Some(cause),
            Self::Package { cause, .. } => Some(cause),
            Self::Definition(cause) => Some(cause),
            Self::Vm(cause) => Some(cause),
            _ => None,
        }
    }
}

impl From<VmError> for MapError {
    fn from(e: VmError) -> Self {
        MapError::Vm(e)
    }
}

/// A level's actor list as its map file holds it.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelList {
    /// The level export.
    pub level: i32,
    /// The references in list order; 0 for a null entry.
    pub actors: Vec<i32>,
}

/// Reads level export `level`'s actor list from `pkg` (module documentation), refusing a tail that does not meet the
/// recovered layout's oracles.
pub fn read_level_list(pkg: &Package, level: i32) -> Result<LevelList, String> {
    let object = pkg.object(level).map_err(|e| e.to_string())?;
    let (start, size) = (
        usize::try_from(object.offset).map_err(|_| "a negative export offset")?,
        usize::try_from(object.size).map_err(|_| "a negative export size")?,
    );
    let tagged = props::read_tagged(pkg, level, PropertyPrefix::Ordinary.bytes(), &NoArrayTypes)
        .map_err(|e| e.to_string())?;
    let end = start
        .checked_add(size)
        .ok_or("the export's end overflows")?;
    let tail = pkg
        .data
        .get(start.checked_add(tagged.end).ok_or("the tail overflows")?..end)
        .ok_or("the tail is past the data")?;
    let word = |at: usize| -> Result<i32, String> {
        tail.get(at..at + 4)
            .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .ok_or_else(|| format!("the tail ends before byte {}", at + 4))
    };
    if word(0)? != level {
        return Err(format!("the list's owner is {}, not the level", word(0)?));
    }
    let count = usize::try_from(word(4)?).map_err(|_| "a negative actor count")?;
    if count == 0 || count > MAX_LEVEL_ACTORS || count > (tail.len() - 8) / 4 {
        return Err(format!("an actor count of {count} the tail cannot hold"));
    }
    let mut actors = Vec::with_capacity(count);
    for k in 0..count {
        let r = word(8 + 4 * k)?;
        if r != 0 {
            let entry = pkg.object(r).map_err(|e| format!("item {k}: {e}"))?;
            if r < 0 || entry.outer != level {
                return Err(format!("item {k} ({r}) is not an export of the level"));
            }
        }
        actors.push(r);
    }
    if actors[0] == 0 {
        return Err("item 0 (the level's WorldInfo) is null".into());
    }
    // The level URL follows: its protocol, a string (length word, then that many bytes ending in a NUL).
    let at = 8 + 4 * count;
    let length =
        usize::try_from(word(at)?).map_err(|_| "the URL's protocol is not an ANSI string")?;
    let text = tail
        .get(at + 4..at + 4 + length)
        .filter(|t| {
            length > 0
                && t.last() == Some(&0)
                && t[..length - 1].iter().all(|b| b.is_ascii_graphic())
        })
        .ok_or("the list is not followed by the level URL's protocol string")?;
    let _ = text;
    Ok(LevelList { level, actors })
}

/// Why actors of one class were not placed.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Refusal {
    pub count: usize,
    /// The first reason (the definition store's or the world's error text).
    pub first: String,
    /// The first reason was a `NotYet`.
    pub not_yet: bool,
}

/// One level of a load.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LevelReport {
    /// The map file's name.
    pub name: String,
    /// Entries of the list, null ones included.
    pub listed: usize,
    pub nulls: usize,
    /// Placed actors by class path, and in all.
    pub placed: DetMap<String, usize>,
    pub placed_total: usize,
    /// Refused actors by class path.
    pub refused: DetMap<String, Refusal>,
}

/// What [`load_map`] did.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct MapReport {
    /// The persistent level first, then the streaming levels in `StreamingLevels` order.
    pub levels: Vec<LevelReport>,
    pub world_info: ObjRef,
    /// Streaming level names `StreamingLevels` gives that no file holds (not loaded).
    pub missing_levels: Vec<String>,
    /// The native steps the load does not run (module documentation).
    pub pending: Vec<NotYet>,
}

/// The native steps a load leaves to the implementer of NATIVE_MAP_LOAD.md.
fn pending() -> Vec<NotYet> {
    vec![
        NotYet::not_read(
            "the begin-play pass over a loaded level's actors (NATIVE_MAP_LOAD.md section 2)",
            RECORD,
        ),
        NotYet::not_read(
            "the game info the engine spawns into WorldInfo.Game (NATIVE_MAP_LOAD.md section 3)",
            RECORD,
        ),
        NotYet::not_read(
            "the local player's play actor and the engine's local player list (NATIVE_MAP_LOAD.md section 4)",
            RECORD,
        ),
    ]
}

/// Loads map `map` into `world`, whose actor list must be empty (module documentation).
pub fn load_map(world: &mut World, map: &PackageName) -> Result<MapReport, MapError> {
    let actors = world.actors();
    if !actors
        .try_borrow()
        .map_err(|_| VmError::ServiceBorrowed("Actors"))?
        .list()
        .is_empty()
    {
        return Err(MapError::WorldNotEmpty);
    }
    let vm = &mut world.vm;
    let actor_class = vm.find_class("Engine.Actor")?;
    let world_info_class = vm.find_class("Engine.WorldInfo")?;
    let (package, list) = level_of(vm, map)?;
    let mut levels = Vec::new();

    // The persistent level: its WorldInfo first.
    let mut report = LevelReport {
        name: map.as_str().to_string(),
        listed: list.actors.len(),
        nulls: list.actors.iter().filter(|&&r| r == 0).count(),
        ..LevelReport::default()
    };
    let world_info = load_world_info(vm, package, list.actors[0], world_info_class)?;
    actors
        .try_borrow_mut()
        .map_err(|_| VmError::ServiceBorrowed("Actors"))?
        .place_world_info(world_info)?;
    count(&mut report.placed, class_path(vm, world_info));
    report.placed_total += 1;
    place_rest(
        vm,
        &actors,
        package,
        &list.actors[1..],
        actor_class,
        &mut report,
    )?;
    levels.push(report);

    // The streaming levels the persistent WorldInfo names, in order; each from its index 1.
    let mut missing_levels = Vec::new();
    for name in streaming_levels(vm, world_info)? {
        let parsed = PackageName::parse(&name).map_err(|cause| MapError::InvalidName {
            name: name.clone(),
            cause,
        })?;
        let (package, list) = match level_of(vm, &parsed) {
            Ok(found) => found,
            Err(MapError::Store {
                cause: StoreError::PackageNotFound(_),
                ..
            }) => {
                missing_levels.push(name);
                continue;
            }
            Err(e) => return Err(e),
        };
        // The structural list reader proves ownership and non-nullness, not the actor's class.
        // Validate each streaming level's item 0 before omitting it from placement.
        load_world_info(vm, package, list.actors[0], world_info_class)?;
        let mut report = LevelReport {
            name,
            listed: list.actors.len(),
            nulls: list.actors.iter().filter(|&&r| r == 0).count(),
            ..LevelReport::default()
        };
        place_rest(
            vm,
            &actors,
            package,
            &list.actors[1..],
            actor_class,
            &mut report,
        )?;
        levels.push(report);
    }
    Ok(MapReport {
        levels,
        world_info,
        missing_levels,
        pending: pending(),
    })
}

/// The map file named `map`, loaded into the store, and its persistent level's list.
fn level_of(
    vm: &mut ow_vm::api::Vm,
    map: &PackageName,
) -> Result<(PackageId, LevelList), MapError> {
    let package = vm
        .defs
        .code_mut()
        .1
        .load(map)
        .map_err(|cause| MapError::Store {
            map: map.clone(),
            cause,
        })?;
    let pkg = vm.defs.packages().get(package);
    let level = pkg
        .find_export(LEVEL_PATH)
        .map_err(|cause| MapError::Package {
            map: map.clone(),
            cause,
        })?
        .ok_or_else(|| MapError::MissingLevel(map.clone()))?;
    let class = pkg
        .object(level)
        .map_err(|cause| MapError::Package {
            map: map.clone(),
            cause,
        })?
        .class;
    let class_name = pkg
        .object(class)
        .map_err(|cause| MapError::Package {
            map: map.clone(),
            cause,
        })?
        .name
        .clone();
    if class_name != "Level" {
        return Err(MapError::WrongLevelClass {
            map: map.clone(),
            class: class_name,
        });
    }
    let list = read_level_list(pkg, level).map_err(|reason| MapError::Layout {
        level: map.as_str().to_string(),
        reason,
    })?;
    Ok((package, list))
}

/// Checks the first entry for every level, including levels whose WorldInfo is not placed.
fn load_world_info(
    vm: &mut ow_vm::api::Vm,
    package: PackageId,
    first: i32,
    world_info_class: ow_object::ids::ClassId,
) -> Result<ObjRef, MapError> {
    let object = load_actor(vm, package, first).map_err(|e| MapError::WorldInfo(e.to_string()))?;
    let class = vm.space.heap.get(object).map_err(VmError::from)?.class;
    if !vm.space.classes.is_child_of(class, world_info_class) {
        return Err(MapError::WorldInfo("item 0 is not a WorldInfo".into()));
    }
    Ok(object)
}

/// Places `items` of level file `package` after the ones placed already, counting what was placed and refused.
fn place_rest(
    vm: &mut ow_vm::api::Vm,
    actors: &std::rc::Rc<std::cell::RefCell<crate::actors::Actors>>,
    package: PackageId,
    items: &[i32],
    actor_class: ow_object::ids::ClassId,
    report: &mut LevelReport,
) -> Result<(), MapError> {
    for &r in items {
        if r == 0 {
            continue;
        }
        let o = match load_actor(vm, package, r) {
            Ok(o) => o,
            Err(e) => {
                let class = export_class(vm, package, r);
                refuse(
                    &mut report.refused,
                    class,
                    e.to_string(),
                    matches!(e, DefError::NotYet(_)),
                );
                continue;
            }
        };
        let class = vm.space.heap.get(o).map_err(VmError::from)?.class;
        if !vm.space.classes.is_child_of(class, actor_class) {
            refuse(
                &mut report.refused,
                class_path(vm, o),
                "not an actor class".into(),
                false,
            );
            continue;
        }
        actors
            .try_borrow_mut()
            .map_err(|_| VmError::ServiceBorrowed("Actors"))?
            .place(o)?;
        count(&mut report.placed, class_path(vm, o));
        report.placed_total += 1;
    }
    Ok(())
}

/// Export `r` of level file `package`, loaded through the definition store: the store's object for its path, with
/// the property prefix the store derives for it (a placed actor's 26 bytes, a collection actor's 4: decision
/// 2026-11-16-55) and, in `ParityMode::Off`, its archetype. Strict: anything that does not load as a definition is
/// the store's error.
fn load_actor(vm: &mut ow_vm::api::Vm, package: PackageId, r: i32) -> Result<ObjRef, DefError> {
    let handle = vm
        .defs
        .resolve_ref(&mut vm.space, package, r)?
        .ok_or_else(|| DefError::NotADefinition(format!("reference {r} is null")))?;
    vm.defs.load(&mut vm.space, handle)
}

/// The package names `WorldInfo.StreamingLevels` gives, in order (none when the class declares no such list).
fn streaming_levels(vm: &mut ow_vm::api::Vm, world_info: ObjRef) -> Result<Vec<String>, MapError> {
    let class = vm.space.heap.get(world_info).map_err(VmError::from)?.class;
    let Some(prop) = vm.space.prop(class, "StreamingLevels") else {
        return Ok(Vec::new());
    };
    let list = match vm
        .space
        .read(world_info, prop.into())
        .map_err(VmError::from)?
    {
        Value::Array(items) => items.as_ref().clone(),
        _ => {
            return Err(MapError::WorldInfo(
                "StreamingLevels is not an array".into(),
            ));
        }
    };
    let mut out = Vec::new();
    for item in list {
        let Value::Object(Some(o)) = item else {
            continue;
        };
        vm.defs
            .ensure_loaded(&mut vm.space, o)
            .map_err(MapError::Definition)?;
        let class = vm.space.heap.get(o).map_err(VmError::from)?.class;
        let Some(name) = vm.space.prop(class, "PackageName") else {
            continue;
        };
        if let Value::Name(n) = vm.space.read(o, name.into()).map_err(VmError::from)? {
            let text = vm.space.names.text(*n).to_string();
            if text != "None" {
                out.push(text);
            }
        }
    }
    Ok(out)
}

fn class_path(vm: &ow_vm::api::Vm, o: ObjRef) -> String {
    vm.space
        .heap
        .get(o)
        .ok()
        .and_then(|object| vm.space.classes.get_layout(object.class))
        .map_or_else(|| "?".into(), |l| l.path().as_str().to_string())
}

/// The class path an export names (from its file's tables), for an actor that did not load.
fn export_class(vm: &ow_vm::api::Vm, package: PackageId, r: i32) -> String {
    let pkg = vm.defs.packages().get(package);
    pkg.object(r)
        .ok()
        .and_then(|e| pkg.path_of(e.class).ok())
        .unwrap_or_else(|| "?".into())
}

fn count(map: &mut DetMap<String, usize>, key: String) {
    *map.entry(key).or_default() += 1;
}

fn refuse(map: &mut DetMap<String, Refusal>, class: String, reason: String, not_yet: bool) {
    map.entry(class)
        .and_modify(|r| r.count += 1)
        .or_insert(Refusal {
            count: 1,
            first: reason,
            not_yet,
        });
}

pub mod bringup;
pub mod game_info;
pub mod player;

#[cfg(test)]
mod tests;
