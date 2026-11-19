//! Owner: lanes MAP-LOAD-NATIVES and MAP-LOAD-C. Bringing a loaded map up for play (NATIVE_MAP_LOAD.md, sections 2 to
//! 4, first pass and the second pass of lane MAP-LOAD-B), coded from the note alone by a lane that did not read the
//! executable (clean room, docs/LANE_BRIEF.md). AI-assisted (Claude Code). Decisions `2026-11-19-04` and
//! `2026-11-19-20`. Every rule here is **UNVERIFIED** in the running game.
//!
//! [`bring_up_for_play`] runs, in the note's order, what the note settles, and names in [`BringUp::pending`] the steps
//! it does not run:
//!
//! 1. **Before anything changes**, three refusals, after every `WorldInfo` field the bring-up uses is read. The game info must already be in `WorldInfo.Game` ([`game_info`]):
//!    the map load runs the game info routine first, and its class choice is still `NotYet` (the receiver of
//!    `SetGameType` in each branch is open). A caller that puts a game info there itself says so (the installed test's
//!    harness set-up does). And the routing's mode argument must be the stock "every actor" reading
//!    ([`RouteMode::EveryActor`]); its value as the bring-up passes it is open in the note, so the other value,
//!    [`RouteMode::SkipStatic`], is `NotYet`. Only static actors tell the two apart. And a network client's world
//!    (`NetMode`) is `NotYet` as a whole: its initialisation pass (the role swap, the destroys) is not modelled.
//! 2. **Time reset**, when the caller's `reset_time` says so: `TimeSeconds`, `RealTimeSeconds` and `AudioTimeSeconds`
//!    become 0. Which value the map load passes is open; on a fresh load the three are 0 either way.
//! 3. **Once**: nothing more when `WorldInfo.bBegunPlay` is already set.
//! 4. **The initialisation pass** (section 2.2) over the world's actor list in order (the persistent level, then the
//!    streaming levels: [`super::load_map`] places them in that order). An actor whose three `bLoadIfPhysXLevel` flags are not all set is `NotYet`: which physics level
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
//!      note's state probe raises it for an actor outside any state, which is every freshly loaded actor). An actor
//!      whose `PreBeginPlay` sent it into a state still gets `PostBeginPlay` and `SetInitialState` whether or not that
//!      state ignores them: the probe mask is not checked here (UNVERIFIED for such an actor).
//!    - Pass 2 reads `bScriptInitialized` and whether the actor routes again, per actor, as the note tests them in
//!      each pass.
//! 8. `bStartup` is cleared, on the error path too.
//!
//! **Failures are counted, not hidden** (a project choice for measuring a map, not the game's rule: UNVERIFIED). An
//! event or a check that fails (a `NotYet`, an unbound native, a script error) is recorded with its step and the
//! actor's class in [`BringUp::failures`], and the pass goes on to the next actor. An actor whose init-pass check or
//! `PreBeginPlay` failed gets no later step; one whose `PostBeginPlay` or `SetInitialState` failed gets no later step
//! of pass 2. A run with any failure is not the game's bring-up: [`BringUp::complete`] is false, and it cannot be
//! redone, because `bBegunPlay` is set and the once test then makes every later call a no-op. In particular an actor
//! the physics level filter could not decide ends neither script-initialised nor hidden nor destroyed, a state the note
//! does not describe.
//!
//! Not run, in [`BringUp::pending`]: the URL options and the setup calls (steps 2 and 3 of the note), the flagged and
//! second engine objects of step 4, the per-level call of step 5, the testing support of step 6, and the local player's
//! play actor (section 4: `SpawnPlayActor`'s call site in the map load, and where the engine fills `GamePlayers`, are
//! open; this project's hook for that list is `Actors::add_local_player`, decision 2026-11-16-81). Step 6 (the
//! `BandwidthLimit=` option passed to the game info, and the automated-testing support) is left out on purpose: a
//! network setting and test support, not behaviour a single-player run depends on.
//!
//! Both parity modes run the same rules: the C++ has no map load, so there is no Reference stand-in (R3).
//!
//! **The map route** ([`bring_up_map_for_play`], lane IMPL-MAPLOAD-GAMEINFO, SLICE_MAPLOAD.md section 3, steps 4 and
//! 5): the game info routine ([`super::game_info::spawn_game_info`], Off only) puts the game info in `WorldInfo.Game`,
//! then [`bring_up_for_play`] runs, so the game info's own script (`InitGame`, its `PreBeginPlay`) makes what it makes.
//! The route mode is checked before the spawn, so that refusal stays unmarked; any `NotYet` the bring-up returns after
//! the routine wrote is marked "stopped after partial writes". The navigation list the design places before the game
//! info (its step 3) is not built: where `WorldInfo.NavigationPointList` and each point's `nextNavigationPoint` come
//! from at load is open (SLICE_MAPLOAD.md section 5, Q1), so it is a `NotYet { NotRead }` in [`MapBringUp::pending`].

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
            "the game info's class choice outside the map route (NATIVE_MAP_LOAD.md section 3: the receiver of \
             SetGameType in each branch; bring_up_map_for_play runs the configured DefaultGame branch)",
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
    /// Every settled step ran without a failure (the unread steps in `pending` still did not run). Counting a failure
    /// and going on is this project's choice for measuring a map, not the game's rule (UNVERIFIED): an incomplete
    /// bring-up has still set `bBegunPlay`, so the once test makes a second call a no-op and it cannot be redone.
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
        "the flag the map load sets on the new game info before the bring-up (NATIVE_MAP_LOAD.md section 2)",
        "the time reset's two engine calls (section 2, step 1)",
        "the bring-up's URL options and setup calls (NATIVE_MAP_LOAD.md section 2, steps 2 and 3)",
        "the flagged engine object and the second engine object of begin play (section 2, step 4)",
        "the initialisation pass's PhysicsVolume (the world's default physics volume), the unnamed object flag that \
         exempts an actor (taken as clear) and the script-execution set-up virtual (section 2.2)",
        "the per-level engine call after begin play (section 2, step 5)",
        "the engine calls of the routing without a script event: physics volumes, component begin play, the pre \
         routine's extra virtual, the Pawn and SkeletalMeshActor registration (section 2.1)",
        "WillowGameInfo's pre override's engine call with the text Hardcore (section 2.1)",
        "the local player's play actor: where the map load calls SpawnPlayActor and where GamePlayers is filled \
         (section 4)",
    ]
    .into_iter()
    .map(|what| NotYet::not_read(what, RECORD))
    .collect()
}

/// The init pass for one actor (module documentation, step 4).
fn init_actor(vm: &mut Vm, o: ObjRef) -> Result<bool, VmError> {
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
        return Err(skip_static());
    }
    let info = world_info_of(world)?.ok_or_else(|| VmError::Script {
        function: "bring_up_for_play".into(),
        message: "no WorldInfo".into(),
    })?;
    // Every WorldInfo field the bring-up reads or writes is checked before anything changes (review r1 of #409).
    let space = &world.vm.space;
    let begun = actors::flag(space, info, "bBegunPlay")?;
    actors::flag(space, info, "bStartup")?;
    let client = actors::byte(space, info, "NetMode")? == NET_MODE_CLIENT;
    if options.reset_time {
        for name in TIME_FIELDS {
            actors::float(space, info, name)?;
        }
    }
    // A network client's initialisation pass (the role swap and the destroys) is not modelled: a whole-world
    // condition, refused before any change rather than once per actor.
    if client && !begun {
        return Err(NotYet::not_read(
            "the bring-up on a network client (the initialisation pass's role swap and destroys)",
            RECORD,
        )
        .into());
    }
    let mut out = BringUp { game, reach, routed: Routed::default(), failures: Vec::new(), pending: pending() };
    let vm = &mut world.vm;
    if options.reset_time {
        for name in TIME_FIELDS {
            actors::set_field(&mut vm.space, info, name, Value::Float(0.0))?;
        }
    }
    if begun {
        return Ok(out);
    }
    let list = {
        let a = world.actors();
        let a = a.try_borrow().map_err(|_| VmError::ServiceBorrowed("Actors"))?;
        a.list().to_vec()
    };
    let vm = &mut world.vm;
    // `stopped` holds the actors a failure or a destroy took out of the routing: an init-pass check that could not be
    // decided (the physics level filter may have removed the actor), and a failed "pre" event.
    let mut stopped: Vec<ObjRef> = Vec::new();
    for &o in &list {
        match init_actor(vm, o) {
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
    let routed = route(vm, game, &list, &mut stopped, &mut out);
    // bStartup is cleared on the error path too: it is set only while begin play is routed.
    actors::set_field(&mut vm.space, info, "bStartup", Value::Bool(false))?;
    routed?;
    Ok(out)
}

/// What [`bring_up_map_for_play`] did.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct MapBringUp {
    /// The game info routine's result (the actor in `WorldInfo.Game`).
    pub game_info: super::game_info::GameInfoSpawned,
    pub bring_up: BringUp,
    /// The map route's own steps not run (module documentation, "The map route").
    pub pending: Vec<NotYet>,
}

/// The map route's steps not run.
pub fn map_route_pending() -> Vec<NotYet> {
    vec![NotYet::not_read(
        "WorldInfo.NavigationPointList and each point's nextNavigationPoint at load (SLICE_MAPLOAD.md section 5, Q1: \
         built at load or read from untagged data, and how the streamed levels' points join)",
        RECORD,
    )]
}

/// The map route (module documentation): the game info routine, then the bring-up.
pub fn bring_up_map_for_play(
    world: &mut World,
    input: &super::game_info::GameInfoInput<'_>,
    options: BringUpOptions,
) -> Result<MapBringUp, VmError> {
    if options.mode == RouteMode::SkipStatic {
        return Err(skip_static());
    }
    let mark = super::game_info::WriteMark::now(&world.vm);
    let game_info = super::game_info::spawn_game_info(world, input)?;
    let bring_up = bring_up_for_play(world, options).map_err(|e| mark.mark(&world.vm, e))?;
    Ok(MapBringUp { game_info, bring_up, pending: map_route_pending() })
}

fn skip_static() -> VmError {
    NotYet::not_read(
        "the begin-play routing with its mode argument set (static actors skipped): which value the bring-up \
         passes (NATIVE_MAP_LOAD.md, Open)",
        RECORD,
    )
    .into()
}

/// The world info's three time fields (step 2).
const TIME_FIELDS: [&str; 3] = ["TimeSeconds", "RealTimeSeconds", "AudioTimeSeconds"];

/// `InitGame` and the two routing passes (module documentation, steps 6 and 7). Pass 2 reads `bScriptInitialized` and
/// whether the actor routes again, per actor, as the note tests them in each pass.
fn route(vm: &mut Vm, game: ObjRef, list: &[ObjRef], stopped: &mut Vec<ObjRef>, out: &mut BringUp) -> Result<(), VmError> {
    if !actors::flag(&vm.space, game, "bScriptInitialized")? {
        match send_event(vm, game, "InitGame", vec![Value::Str("".into())]) {
            Ok(raised) => out.routed.init_game = raised,
            Err(e) => out.failures.push(failure(vm, "InitGame", game, &e)),
        }
    }
    // Pass 1.
    for &o in list {
        if actors::flag(&vm.space, o, "bScriptInitialized")? {
            out.routed.skipped_initialized += 1;
            continue;
        }
        if !routes(vm, o)? || stopped.contains(&o) || is_destroyed(&vm.space, o)? {
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
    for &o in list {
        if stopped.contains(&o) {
            continue;
        }
        if is_destroyed(&vm.space, o)? {
            out.routed.destroyed += 1;
            continue;
        }
        if actors::flag(&vm.space, o, "bScriptInitialized")? {
            continue;
        }
        if !routes(vm, o)? {
            actors::set_field(&mut vm.space, o, "bScriptInitialized", Value::Bool(true))?;
            out.routed.marked_static += 1;
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
    Ok(())
}

#[cfg(test)]
mod tests;
