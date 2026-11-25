//! Owner: lane IMPL-MAPLOAD-GAMEINFO (lane 2 of SLICE_MAPLOAD.md section 4). The game info routine of the map load
//! (NATIVE_MAP_LOAD.md, section 3), in `ParityMode::Off`, coded from the note and the design alone by a lane that did
//! not read the executable (clean room, docs/LANE_BRIEF.md). AI-assisted (Claude Code). Every rule here is
//! **UNVERIFIED** in the running game.
//!
//! [`spawn_game_info`] runs the note's routine for the branch Sanctuary takes (SLICE_MAPLOAD.md section 3, step 4):
//!
//! 1. **Already one**: when `WorldInfo.Game` holds a game info, nothing is done ([`GameInfoBranch::AlreadySet`]).
//! 2. **Refused before anything changes**, each a typed `NotYet { NotRead }`, unmarked:
//!    - a world whose `NetMode` is not standalone (0): the note skips the routine when a network or demo driver exists
//!      (medium), and whether one does is not modelled here;
//!    - a world that has begun play: the routine runs at load, before the bring-up, so its spawn raises no begin-play
//!      events (NATIVE_ENGINE_CORE.md, "Actor.Spawn"); a spawn after that would, which the note does not describe;
//!    - a set `WorldInfo.DefaultGameType`, and a `GAME=` option in the URL options: both reach the script event
//!      `SetGameType`, whose receiver in each branch is open (the note's Open list). Telling a `GAME=` class that loads
//!      from one that does not would need a load, which is a write, so any `GAME=` option is refused;
//!    - no configured default game (the game ini's `[Engine.GameInfo] DefaultGame`): the caller passes it, read from
//!      the installed `DefaultGame.ini` by [`default_game_from_ini`]. Whether the user's `WillowGame.ini` overrides it
//!      is open (SLICE_MAPLOAD.md section 5, Q2).
//! 3. **The configured default** ([`GameInfoBranch::ConfiguredDefault`]): the class the configured path names is
//!    looked up (which may load a package), checked against `WorldInfo.Game`'s declared class, spawned through the
//!    world spawn ([`crate::actors::spawn`]) with the `WorldInfo` as the spawner (UNVERIFIED: the engine's spawn has
//!    no spawning actor; the `WorldInfo` gives the location, rotation and an empty instigator), and stored in
//!    `WorldInfo.Game`. A class that is not found, and a spawn that returns nothing, give the note's not-found text
//!    with the field left empty. The note has the engine log it and carry on; **this port stops the route there**
//!    (a data error, the port's choice: a map without its game info is not a world the slice can grade). The "loaded
//!    once and cached" of the note is the class table's own caching.
//!
//! **Writes and marking** (the hub's rules): steps 1 and 2 read only. From step 3 on, a lazy load counts as a write.
//! #518's three counters (the heap's committed writes, the definition store's load transitions and the game RNG's
//! draws, [`WriteMark`]) and the VM's code loads (lane GAMEINFO-COLLIDER-CODE-LOAD: the class lookup loads the class's
//! code, which #518's counters do not see) are read before step 3; any `NotYet` returned after one moved is marked
//! "stopped after partial writes", its kind and record kept ([`crate::actors::after_writes`]). The not-found stop is
//! marked with the same prefix when a lazy load already ran (review r1 of #537); it is a `VmError::Script`, so the mark
//! sits at the start of its message, after the function name ("the game info routine"), not at the start of the error's
//! text. Other errors are returned as they are.
//!
//! **The ini reader refuses instead of guessing** (review r1 of #537): a quoted value, a prefixed key (`+`, `-`, `.`,
//! `!`, the ini's array and removal operators) and a second `DefaultGame` in the section are `NotYet { NotRead }`. A
//! UTF-8 byte-order mark is skipped, and the installed file is read only up to [`MAX_INI_BYTES`].
//!
//! `ParityMode::Reference` is unchanged: the C++ has no map load, so the routine is `NotYet { NotPorted }` there.

use std::path::Path;

use ow_core::{NotYet, ParityMode};
use ow_object::ids::{ClassId, ObjRef};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::RECORD;
use crate::actors::{self, SpawnRequest, after_writes};
use crate::world::World;

/// The game ini the configured default game is read from, relative to the install folder.
pub const DEFAULT_GAME_INI: &str = "WillowGame/Config/DefaultGame.ini";

/// The most bytes [`installed_default_game`] reads; a larger file is refused (review r1 of #537).
pub const MAX_INI_BYTES: u64 = 1 << 20;

/// The `NetMode` of a standalone world (no network driver).
pub const NET_MODE_STANDALONE: u8 = 0;

/// The note's text for a game info class that is not there (NATIVE_MAP_LOAD.md section 3).
pub fn not_found_text(class: &str) -> String {
    format!("GameInfo class '{class}' not found (or could not be loaded)")
}

/// What the caller states for the routine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GameInfoInput<'a> {
    /// The URL's options, joined with `?` before each (the slice's load has none: empty).
    pub options: &'a str,
    /// The configured default game's class path (`[Engine.GameInfo] DefaultGame`), `None` when the ini has no key.
    pub default_game: Option<&'a str>,
}

/// Which branch of the routine ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameInfoBranch {
    /// `WorldInfo.Game` already held a game info: nothing done.
    AlreadySet,
    /// `DefaultGameType` None and no `GAME=` option: the configured default was spawned.
    ConfiguredDefault,
}

/// What [`spawn_game_info`] did: the game info in `WorldInfo.Game`, its class path and the branch. This is the entry
/// point the local player's lane starts from (its `Login` and `PostLogin` are this actor's script events).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameInfoSpawned {
    pub game: ObjRef,
    pub class: String,
    pub branch: GameInfoBranch,
}

/// The configured default game in a game ini's text: the value of `DefaultGame` in section `[Engine.GameInfo]`
/// (section and key compared without case, as ini keys are; blank lines and `;` comments skipped; a leading UTF-8
/// byte-order mark skipped). `Ok(None)` when the section or the key is absent or the value is empty. Refused, as
/// `NotYet { NotRead }` (module documentation): a quoted value, a prefixed `DefaultGame` key, a second `DefaultGame`.
pub fn default_game_from_ini(text: &str) -> Result<Option<String>, NotYet> {
    let refuse = |what: &'static str| NotYet::not_read(what, RECORD);
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut in_section = false;
    let mut found: Option<String> = None;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }
        if let Some(section) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            in_section = section.trim().eq_ignore_ascii_case("Engine.GameInfo");
            continue;
        }
        if !in_section {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let bare = key.trim_start_matches(['+', '-', '.', '!']);
        if !bare.eq_ignore_ascii_case("DefaultGame") {
            continue;
        }
        if bare.len() != key.len() {
            return Err(refuse("a prefixed DefaultGame key in the game ini (the ini's array and removal operators)"));
        }
        if found.is_some() {
            return Err(refuse("a second DefaultGame in the game ini's [Engine.GameInfo] (which one the engine takes)"));
        }
        let value = value.trim();
        if value.starts_with('"') || value.ends_with('"') {
            return Err(refuse("a quoted DefaultGame value in the game ini (how the engine reads the quotes)"));
        }
        found = Some(value.to_string());
    }
    Ok(found.filter(|v| !v.is_empty()))
}

/// [`default_game_from_ini`] over the installed `DefaultGame.ini` under `root` (the install folder). A file larger
/// than [`MAX_INI_BYTES`], and a read error, are data errors; the reader's refusals are its `NotYet`.
pub fn installed_default_game(root: &Path) -> Result<Option<String>, VmError> {
    use std::io::Read as _;
    let path = root.join(DEFAULT_GAME_INI);
    let io = |e: std::io::Error| data_error(format!("reading {}: {e}", path.display()));
    let mut bytes = Vec::new();
    std::fs::File::open(&path).map_err(io)?.take(MAX_INI_BYTES + 1).read_to_end(&mut bytes).map_err(io)?;
    if bytes.len() as u64 > MAX_INI_BYTES {
        return Err(data_error(format!("{} is larger than {MAX_INI_BYTES} bytes", path.display())));
    }
    Ok(default_game_from_ini(&String::from_utf8_lossy(&bytes))?)
}

/// The value of the first `GAME=` option in `options` (key compared without case), if any.
pub fn game_option(options: &str) -> Option<&str> {
    options.split('?').find_map(|o| {
        let (key, value) = o.split_once('=')?;
        key.trim().eq_ignore_ascii_case("GAME").then_some(value)
    })
}

/// The three counters a write moves (#518): the heap's committed writes, the store's load transitions and the game
/// RNG's draws (a draw is a write, review r1 of #537).
///
/// It also keeps the VM's code loads (lane COUNTERS-MISS-CLASS-CODE-LOADS) for [`WriteMark::still`], the tests'
/// "nothing moved" check; [`WriteMark::moved`], which marks a runtime refusal, still reads only #518's three.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WriteMark {
    heap: u64,
    loads: u64,
    draws: u64,
    code_loads: u64,
}

impl WriteMark {
    /// The counters now.
    pub fn now(vm: &Vm) -> Self {
        WriteMark {
            heap: vm.space.heap.committed_writes(),
            loads: vm.defs.load_transitions(),
            draws: vm.rng.draws(),
            code_loads: vm.class_code_loads(),
        }
    }

    /// Whether one of #518's three counters moved since this mark (not the code loads: see the type).
    pub fn moved(&self, vm: &Vm) -> bool {
        let now = WriteMark::now(vm);
        (now.heap, now.loads, now.draws) != (self.heap, self.loads, self.draws)
    }

    /// Whether nothing moved since this mark, the code loads included: the tests' write-free check.
    pub fn still(&self, vm: &Vm) -> bool {
        *self == WriteMark::now(vm)
    }

    /// `e`, marked "stopped after partial writes" when something was written since this mark (a `NotYet` only).
    pub fn mark(&self, vm: &Vm, e: VmError) -> VmError {
        if self.moved(vm) { after_writes(e) } else { e }
    }
}

fn not_read(what: &'static str) -> VmError {
    NotYet::not_read(what, RECORD).into()
}

fn world_info(world: &World) -> Result<ObjRef, VmError> {
    let info = world.actors().try_borrow().map_err(|_| VmError::ServiceBorrowed("Actors"))?.world_info();
    info.ok_or_else(|| {
        NotYet::layer_absent(
            "the game info of a world without its WorldInfo",
            "the map load (mapload::load_map)",
            RECORD,
        )
        .into()
    })
}

fn data_error(message: String) -> VmError {
    VmError::Script { function: "the game info routine".into(), message: format!("{message} ({RECORD}, section 3)") }
}

/// What the write-free part of the routine decided.
enum Choice {
    Already(ObjRef),
    Configured { info: ObjRef, path: String, game_class: ClassId },
}

/// Steps 1 and 2 of the module documentation: reads only.
fn choose(world: &World, input: &GameInfoInput<'_>) -> Result<Choice, VmError> {
    let vm = &world.vm;
    if vm.parity != ParityMode::Off {
        return Err(NotYet::not_ported(
            "the map load's game info routine under ParityMode::Reference (the C++ has no map load)",
            RECORD,
        )
        .into());
    }
    let info = world_info(world)?;
    let space = &vm.space;
    let game_prop = actors::prop_of(space, info, "Game")?;
    let game_class = match space.classes.decl(game_prop).map(|d| &d.kind) {
        Some(PropKind::Object { class }) => *class,
        _ => return Err(data_error("WorldInfo.Game is not declared as an object reference".into())),
    };
    match actors::field(space, info, "Game")? {
        Value::Object(Some(game)) => return Ok(Choice::Already(game)),
        Value::Object(None) => {}
        other => return Err(data_error(format!("WorldInfo.Game holds {}, expected an object", other.kind_name()))),
    }
    if actors::byte(space, info, "NetMode")? != NET_MODE_STANDALONE {
        return Err(not_read(
            "the game info routine on a world that is not standalone (whether a network or demo driver exists, which \
             makes the routine do nothing)",
        ));
    }
    if world.actors().try_borrow().map_err(|_| VmError::ServiceBorrowed("Actors"))?.begun_play() {
        return Err(not_read("the game info routine after play began (the routine runs at load, before the bring-up)"));
    }
    match actors::field(space, info, "DefaultGameType")? {
        Value::Class(None) => {}
        Value::Class(Some(_)) => {
            return Err(not_read(
                "the game info class choice with WorldInfo.DefaultGameType set (the receiver of SetGameType)",
            ));
        }
        other => {
            return Err(data_error(format!("WorldInfo.DefaultGameType holds {}, expected a class", other.kind_name())));
        }
    }
    if game_option(input.options).is_some() {
        return Err(not_read(
            "the game info class choice with a GAME= option (whether its class loads, and the receiver of \
             SetGameType)",
        ));
    }
    let Some(path) = input.default_game.filter(|p| !p.trim().is_empty()) else {
        return Err(not_read(
            "the configured default game: the game ini given holds no [Engine.GameInfo] DefaultGame (whether the \
             user's WillowGame.ini overrides DefaultGame.ini is open)",
        ));
    };
    Ok(Choice::Configured { info, path: path.trim().to_string(), game_class })
}

/// The game info routine (module documentation).
pub fn spawn_game_info(world: &mut World, input: &GameInfoInput<'_>) -> Result<GameInfoSpawned, VmError> {
    let (info, path, game_class) = match choose(world, input)? {
        Choice::Already(game) => {
            let class = ow_attr::eval::class_path(&world.vm.space, game);
            return Ok(GameInfoSpawned { game, class, branch: GameInfoBranch::AlreadySet });
        }
        Choice::Configured { info, path, game_class } => (info, path, game_class),
    };
    let vm = &mut world.vm;
    let mark = WriteMark::now(vm);
    configured(vm, &mark, info, &path, game_class)
        .map_err(|e| if wrote(vm, &mark) { after_writes(e) } else { e })
}

/// Whether step 3 has written or loaded anything since `mark`: #518's three counters, and the VM's code loads (lane
/// GAMEINFO-COLLIDER-CODE-LOAD). NATIVE_MAP_LOAD.md section 3 has the routine load and cache the configured class
/// before spawning it. The port's class lookup also loads that class's code into the VM, a side effect of the host's
/// lookup (not something the note says the game does), and the module's step-3 rule counts it: a lazy load from step 3
/// on is a write, so a refusal after it is marked. UNVERIFIED in the running game.
fn wrote(vm: &Vm, mark: &WriteMark) -> bool {
    !mark.still(vm)
}

/// The not-found stop (module documentation, step 3), marked when something was written or loaded since `mark`.
fn not_found(vm: &Vm, mark: &WriteMark, path: &str) -> VmError {
    let text = not_found_text(path);
    data_error(if wrote(vm, mark) { ow_core::notyet::mark_partial_writes(&text) } else { text })
}

/// Step 3: the lookup, the spawn and the store.
fn configured(
    vm: &mut Vm,
    mark: &WriteMark,
    info: ObjRef,
    path: &str,
    game_class: ClassId,
) -> Result<GameInfoSpawned, VmError> {
    let Some(class) = vm.find_class_if_present(path)? else {
        return Err(not_found(vm, mark, path));
    };
    if !vm.space.classes.is_child_of(class, game_class) {
        return Err(data_error(format!("{path} is not a class WorldInfo.Game can hold")));
    }
    let Some(game) = actors::spawn(vm, info, &SpawnRequest::new(Some(class)))? else {
        return Err(not_found(vm, mark, path));
    };
    actors::set_field(&mut vm.space, info, "Game", Value::Object(Some(game)))?;
    Ok(GameInfoSpawned { game, class: path.to_string(), branch: GameInfoBranch::ConfiguredDefault })
}

#[cfg(test)]
mod tests;
