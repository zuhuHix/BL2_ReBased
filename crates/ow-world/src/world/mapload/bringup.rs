//! Owner: lanes MAP-LOAD-NATIVES and MAP-LOAD-C. Bringing a loaded map up for play (NATIVE_MAP_LOAD.md, sections 2 to
//! 4, first pass and the second pass of lane MAP-LOAD-B), coded from the note alone by a lane that did not read the
//! executable (clean room, docs/LANE_BRIEF.md). AI-assisted (Claude Code). Decisions `2026-11-19-04` and
//! `2026-11-19-20`. Every rule here is **UNVERIFIED** in the running game.
//!
//! [`bring_up_for_play`] runs, in the note's order, what the note settles, and names in [`BringUp::pending`] the steps
//! it does not run:
//!
//! 1. **Before anything changes**, two refusals. The game info must already be in `WorldInfo.Game` ([`game_info`]):
//!    the map load runs the game info routine first, and its class choice is still `NotYet` (the receiver of
//!    `SetGameType` in each branch is open). A caller that puts a game info there itself says so (the installed test's
//!    harness set-up does). And the routing's mode argument must be the stock "every actor" reading
//!    ([`RouteMode::EveryActor`]); its value as the bring-up passes it is open in the note, so the other value,
//!    [`RouteMode::SkipStatic`], is `NotYet`. Only static actors tell the two apart.
//! 2. **Time reset**, when the caller's `reset_time` says so: `TimeSeconds`, `RealTimeSeconds` and `AudioTimeSeconds`
//!    become 0. Which value the map load passes is open; on a fresh load the three are 0 either way.
//! 3. **Once**: nothing more when `WorldInfo.bBegunPlay` is already set.
//! 4. **The initialisation pass** (section 2.2) over the world's actor list in order (the persistent level, then the
//!    streaming levels: [`super::load_map`] places them in that order). A network client's pass is `NotYet` (this world
//!    is standalone). An actor whose three `bLoadIfPhysXLevel` flags are not all set is `NotYet`: which physics level
//!    the engine runs at is not read, and only all three set is the same at every level. For an actor that is not
//!    `bDeleteMe`: `LastRenderTime` becomes the most negative float and `Touching` is emptied. Not done: the
//!    unnamed object flag that also exempts an actor (taken as clear), `PhysicsVolume` (the world's default physics
//!    volume; volumes are not modelled, as in `actors::spawn`), and the script-execution set-up virtual (low).
//! 5. `bBegunPlay` and `bStartup` are set; the world's actor list begins play (`Actors::begin_play`: from now on a
//!    spawn runs its begin-play events).
//! 6. **`InitGame(Options, Error)`** on the game info when its `bScriptInitialized` is clear. The options are the URL's
//!    options joined; this load has no URL options, so the text is empty.
//! 7. **The routing** (section 2.1), two passes over the actor list as it was when the routing began (actors spawned by
//!    an event are not visited: whether the engine's loop sees them is not read). An actor already
//!    `bScriptInitialized` gets nothing. An actor *routes* when it is not `bStatic`, or is static with
//!    `bRouteBeginPlayEvenIfStatic`.
//!    - Pass 1: each routing actor gets `PreBeginPlay`.
//!    - Pass 2: each routing actor gets `PostBeginPlay` and, unless `bDeleteMe`, `SetInitialState`; then, when it has
//!      no `Base`, collides with the world, `bShouldBaseAtStartup` and `Physics` is `PHYS_None` or `PHYS_Rotating`, the
//!      base search, which needs collision (`NotYet`, as in `actors::spawn`). A static actor that does not route has
//!      `bScriptInitialized` set and nothing else.
//!    - Not done (each an engine call with no script event, or unread): the physics volume list and the volume
//!      assignment, the component begin-play calls, the "pre" routine's extra virtual and rigid-body tick group, the
//!      `Pawn` / `SkeletalMeshActor` registration, `WillowGameInfo`'s `Hardcore` call.
//!    - The events go through `actors::send_event`, which raises an event the actor's class or state defines (the
//!      note's state probe raises it for an actor outside any state, which is every freshly loaded actor).
//! 8. `bStartup` is cleared.
//!
//! **Failures are counted, not hidden.** An event or a check that fails (a `NotYet`, an unbound native, a script error)
//! is recorded with its step and the actor's class in [`BringUp::failures`], that actor's remaining steps of the same
//! pass are skipped, and the pass goes on to the next actor, so one run measures the whole map. A run with any failure
//! is not the game's bring-up: [`BringUp::complete`] is false.
//!
//! Not run, in [`BringUp::pending`]: the URL options and the setup calls (steps 2 and 3 of the note), the flagged and
//! second engine objects of step 4, the per-level call of step 5, the testing support of step 6, and the local player's
//! play actor (section 4: `SpawnPlayActor`'s call site in the map load, and where the engine fills `GamePlayers`, are
//! open; this project's hook for that list is `Actors::add_local_player`, decision 2026-11-16-81). Step 6 (the
//! `BandwidthLimit=` option passed to the game info, and the automated-testing support) is left out on purpose: a
//! network setting and test support, not behaviour a single-player run depends on.
//!
//! Both parity modes run the same rules: the C++ has no map load, so there is no Reference stand-in (R3).

use ow_core::NotYet;
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::RECORD;
use crate::actors::{self, NET_MODE_CLIENT, is_destroyed, send_event};
use crate::world::World;

/// What the begin-play routing would reach over the world's actor list (NATIVE_MAP_LOAD.md, section 2.1). Null list
/// entries are never placed, so none is counted.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BeginPlayReach {
    /// Actors in the world's list, the `WorldInfo` included.
    pub actors: usize,
    /// Already `bScriptInitialized`: the routing gives them nothing.
    pub script_initialized: usize,
    /// Not script-initialised and routing (not static, or static with `bRouteBeginPlayEvenIfStatic`).
    pub routes: usize,
    /// Not script-initialised, static and not routing: they only get `bScriptInitialized`.
    pub static_unrouted: usize,
    /// The persistent `WorldInfo`'s `bBegunPlay`, `None` when its class declares no such field.
    pub world_info_begun_play: Option<bool>,
}

/// Whether `o` routes (module documentation, step 7).
fn routes(vm: &Vm, o: ObjRef) -> Result<bool, VmError> {
    let space = &vm.space;
    Ok(!actors::flag(space, o, "bStatic")? || actors::flag(space, o, "bRouteBeginPlayEvenIfStatic")?)
}

/// The routing's reach over `world`'s actor list (module documentation).
pub fn begin_play_reach(world: &World) -> Result<BeginPlayReach, VmError> {
    let list = actor_list(world)?;
    let vm = &world.vm;
    let mut reach = BeginPlayReach { actors: list.len(), ..BeginPlayReach::default() };
    for &o in &list {
        if actors::flag(&vm.space, o, "bScriptInitialized")? {
            reach.script_initialized += 1;
        } else if routes(vm, o)? {
            reach.routes += 1;
        } else {
            reach.static_unrouted += 1;
        }
    }
    if let Some(info) = world_info_of(world)? {
        let class = vm.space.heap.get(info)?.class;
        if vm.space.prop(class, "bBegunPlay").is_some() {
            reach.world_info_begun_play = Some(actors::flag(&vm.space, info, "bBegunPlay")?);
        }
    }
    Ok(reach)
}

fn actor_list(world: &World) -> Result<Vec<ObjRef>, VmError> {
    Ok(world.actors().try_borrow().map_err(|_| VmError::ServiceBorrowed("Actors"))?.list().to_vec())
}

fn world_info_of(world: &World) -> Result<Option<ObjRef>, VmError> {
    Ok(world.actors().try_borrow().map_err(|_| VmError::ServiceBorrowed("Actors"))?.world_info())
}

/// The game info routine (NATIVE_MAP_LOAD.md, section 3): the persistent `WorldInfo.Game` when it is set (the routine
/// then does nothing); otherwise `NotYet`, since the class choice is not settled by the note. No `WorldInfo` placed is
/// `NotYet` naming the map load. A class that declares no object property `Game` is a typed error, never a default.
pub fn game_info(world: &World) -> Result<ObjRef, VmError> {
    let Some(info) = world_info_of(world)? else {
        return Err(NotYet::layer_absent(
            "the game info of a world without its WorldInfo",
            "the map load (mapload::load_map)",
            RECORD,
        )
        .into());
    };
    match actors::field(&world.vm.space, info, "Game")? {
        Value::Object(Some(game)) => Ok(game),
        Value::Object(None) => Err(NotYet::not_read(
            "the game info's class choice (NATIVE_MAP_LOAD.md section 3: the receiver of SetGameType in each branch, \
             and the configured DefaultGame, which no reader here reads)",
            RECORD,
        )
        .into()),
        other => Err(VmError::Script {
            function: "Game".into(),
            message: format!("WorldInfo.Game holds {}, expected an object ({RECORD}, section 3)", other.kind_name()),
        }),
    }
}

/// The routing's mode argument (module documentation, step 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteMode {
    /// The stock UE3 reading: every actor goes through the passes (static ones as the routing rule says). UNVERIFIED.
    EveryActor,
    /// Static actors skipped entirely. Not run: `NotYet`.
    SkipStatic,
}

/// What the caller states about the two values the note leaves open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BringUpOptions {
    pub reset_time: bool,
    pub mode: RouteMode,
}

/// What the routing did.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Routed {
    /// The game info's `InitGame` was raised (and returned).
    pub init_game: bool,
    /// Actors whose init-pass resets ran.
    pub init_pass: usize,
    /// Events raised (the actor's class or state defines them).
    pub pre_begin_play: usize,
    pub post_begin_play: usize,
    pub set_initial_state: usize,
    /// Static actors that did not route: `bScriptInitialized` set.
    pub marked_static: usize,
    /// Already script-initialised: nothing.
    pub skipped_initialized: usize,
    /// Destroyed by an event before their later steps.
    pub destroyed: usize,
}

/// One failed step of the bring-up (module documentation, "Failures are counted").
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Failure {
    /// `init_pass`, `InitGame`, `PreBeginPlay`, `PostBeginPlay`, `SetInitialState` or `base search`.
    pub step: &'static str,
    pub actor: ObjRef,
    pub class: String,
    pub text: String,
    pub not_yet: bool,
}

/// What [`bring_up_for_play`] did.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct BringUp {
    pub game: ObjRef,
    /// The reach before the bring-up.
    pub reach: BeginPlayReach,
    pub routed: Routed,
    pub failures: Vec<Failure>,
    /// The steps not run, in the routine's order (module documentation).
    pub pending: Vec<NotYet>,
}

impl BringUp {
    /// Every settled step ran without a failure (the unread steps in `pending` still did not run).
    pub fn complete(&self) -> bool {
        self.failures.is_empty()
    }
}

fn class_of(vm: &Vm, o: ObjRef) -> String {
    ow_attr::eval::class_path(&vm.space, o)
}

fn failure(vm: &Vm, step: &'static str, actor: ObjRef, e: &VmError) -> Failure {
    Failure {
        step,
        actor,
        class: class_of(vm, actor),
        text: e.to_string(),
        not_yet: matches!(e, VmError::NotYet(_)),
    }
}

fn pending() -> Vec<NotYet> {
    [
        "the bring-up's URL options and setup calls (NATIVE_MAP_LOAD.md section 2, steps 2 and 3)",
        "the flagged engine object and the second engine object of begin play (section 2, step 4)",
        "the per-level engine call after begin play (section 2, step 5)",
        "the engine calls of the routing without a script event: physics volumes, component begin play, the pre \
         routine's extra virtual, the Pawn and SkeletalMeshActor registration (section 2.1)",
        "the local player's play actor: where the map load calls SpawnPlayActor and where GamePlayers is filled \
         (section 4)",
    ]
    .into_iter()
    .map(|what| NotYet::not_read(what, RECORD))
    .collect()
}

/// The init pass for one actor (module documentation, step 4).
fn init_actor(vm: &mut Vm, o: ObjRef, client: bool) -> Result<bool, VmError> {
    if client {
        return Err(NotYet::not_read("the initialisation pass on a network client (the role swap and destroys)", RECORD)
            .into());
    }
    for level in ["bLoadIfPhysXLevel0", "bLoadIfPhysXLevel1", "bLoadIfPhysXLevel2"] {
        if !actors::flag(&vm.space, o, level)? {
            return Err(NotYet::not_read(
                "the physics level filter on an actor that does not load at every physics level (which level the \
                 engine runs at)",
                RECORD,
            )
            .into());
        }
    }
    if actors::flag(&vm.space, o, "bDeleteMe")? {
        return Ok(false);
    }
    actors::set_field(&mut vm.space, o, "LastRenderTime", Value::Float(f32::MIN))?;
    actors::set_field(&mut vm.space, o, "Touching", Value::array(Vec::new()))?;
    Ok(true)
}

fn base_search() -> VmError {
    NotYet::layer_absent(
        "the base search after SetInitialState at begin play",
        "collision (the world's collision hash is not modelled)",
        RECORD,
    )
    .into()
}

/// Brings `world`'s loaded map up for play (module documentation).
pub fn bring_up_for_play(world: &mut World, options: BringUpOptions) -> Result<BringUp, VmError> {
    let reach = begin_play_reach(world)?;
    let game = game_info(world)?;
    if options.mode == RouteMode::SkipStatic {
        return Err(NotYet::not_read(
            "the begin-play routing with its mode argument set (static actors skipped): which value the bring-up \
             passes (NATIVE_MAP_LOAD.md, Open)",
            RECORD,
        )
        .into());
    }
    let info = world_info_of(world)?.ok_or_else(|| VmError::Script {
        function: "bring_up_for_play".into(),
        message: "no WorldInfo".into(),
    })?;
    let mut out = BringUp { game, reach, routed: Routed::default(), failures: Vec::new(), pending: pending() };
    let vm = &mut world.vm;
    if options.reset_time {
        for name in ["TimeSeconds", "RealTimeSeconds", "AudioTimeSeconds"] {
            actors::set_field(&mut vm.space, info, name, Value::Float(0.0))?;
        }
    }
    if actors::flag(&vm.space, info, "bBegunPlay")? {
        return Ok(out);
    }
    let list = {
        let a = world.actors();
        let a = a.try_borrow().map_err(|_| VmError::ServiceBorrowed("Actors"))?;
        a.list().to_vec()
    };
    let vm = &mut world.vm;
    let client = actors::byte(&vm.space, info, "NetMode")? == NET_MODE_CLIENT;
    // `stopped` holds the actors a failure or a destroy took out of the routing: an init-pass check that could not be
    // decided (the physics level filter may have removed the actor), and a failed "pre" event.
    let mut stopped: Vec<ObjRef> = Vec::new();
    for &o in &list {
        match init_actor(vm, o, client) {
            Ok(true) => out.routed.init_pass += 1,
            Ok(false) => {}
            Err(e) => {
                out.failures.push(failure(vm, "init_pass", o, &e));
                stopped.push(o);
            }
        }
    }
    actors::set_field(&mut vm.space, info, "bBegunPlay", Value::Bool(true))?;
    actors::set_field(&mut vm.space, info, "bStartup", Value::Bool(true))?;
    world.actors().try_borrow_mut().map_err(|_| VmError::ServiceBorrowed("Actors"))?.begin_play();
    let vm = &mut world.vm;
    if !actors::flag(&vm.space, game, "bScriptInitialized")? {
        match send_event(vm, game, "InitGame", vec![Value::Str("".into())]) {
            Ok(raised) => out.routed.init_game = raised,
            Err(e) => out.failures.push(failure(vm, "InitGame", game, &e)),
        }
    }
    // Pass 1.
    let mut routing: Vec<(ObjRef, bool)> = Vec::with_capacity(list.len());
    for &o in &list {
        let initialized = actors::flag(&vm.space, o, "bScriptInitialized")?;
        let r = routes(vm, o)?;
        routing.push((o, !initialized && r));
        if initialized {
            out.routed.skipped_initialized += 1;
            continue;
        }
        if !r || stopped.contains(&o) || is_destroyed(&vm.space, o)? {
            continue;
        }
        match send_event(vm, o, "PreBeginPlay", Vec::new()) {
            Ok(raised) => out.routed.pre_begin_play += usize::from(raised),
            Err(e) => {
                out.failures.push(failure(vm, "PreBeginPlay", o, &e));
                stopped.push(o);
            }
        }
    }
    // Pass 2.
    for &(o, routed) in &routing {
        if stopped.contains(&o) {
            continue;
        }
        if !routed {
            if !actors::flag(&vm.space, o, "bScriptInitialized")? {
                actors::set_field(&mut vm.space, o, "bScriptInitialized", Value::Bool(true))?;
                out.routed.marked_static += 1;
            }
            continue;
        }
        if is_destroyed(&vm.space, o)? {
            out.routed.destroyed += 1;
            continue;
        }
        match send_event(vm, o, "PostBeginPlay", Vec::new()) {
            Ok(raised) => out.routed.post_begin_play += usize::from(raised),
            Err(e) => {
                out.failures.push(failure(vm, "PostBeginPlay", o, &e));
                continue;
            }
        }
        if is_destroyed(&vm.space, o)? {
            out.routed.destroyed += 1;
            continue;
        }
        if !actors::flag(&vm.space, o, "bDeleteMe")? {
            match send_event(vm, o, "SetInitialState", Vec::new()) {
                Ok(raised) => out.routed.set_initial_state += usize::from(raised),
                Err(e) => {
                    out.failures.push(failure(vm, "SetInitialState", o, &e));
                    continue;
                }
            }
        }
        if !is_destroyed(&vm.space, o)? && actors::needs_base_search(&vm.space, o)? {
            out.failures.push(failure(vm, "base search", o, &base_search()));
        }
    }
    actors::set_field(&mut vm.space, info, "bStartup", Value::Bool(false))?;
    Ok(out)
}

#[cfg(test)]
mod tests;
