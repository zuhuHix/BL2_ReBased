//! Owner: lane IMPL-MAPLOAD-PLAYER (lane 3 of SLICE_MAPLOAD.md section 4). The local player and its play actor on the
//! map route (NATIVE_MAP_LOAD.md sections 4 and 6, with the hub's corrections while those notes are in review), in
//! `ParityMode::Off`, coded from the notes alone by a lane that did not analyse them (clean room, docs/LANE_BRIEF.md).
//! AI-assisted (Claude Code). Every rule here is **UNVERIFIED** in the running game.
//!
//! **Where it runs.** The engine's local-player list (`GamePlayers`) is filled by script at engine start-up, before
//! any map (section 6.1: `CreateInitialPlayer`, one player, no play actor). This port has no engine start-up, so the
//! caller makes the player ([`make_local_player`], from the class defaults: its `ControllerId` stays 0, the note's
//! port answer for the slice) and passes the list. The map load calls the engine's local-player spawn for each local
//! player after the bring-up (section 6.2), the same routine the script's `LocalPlayer.SpawnPlayActor` reaches.
//! [`spawn_play_actor`] is that one routine, called by the harness: the call site in this port is UNVERIFIED, and the
//! script wrapper is not bound.
//!
//! **The routine** (section 4, server side, in the hub's binding order):
//! 1. **Before any write**, every refusal: not Off (`NotYet { NotPorted }`); no game info in `WorldInfo.Game` (the
//!    bring-up's own `NotYet`); a network client (the client branch is not read); a `GetUniqueNetId` answer the port
//!    does not model ([`NetIdAnswer::Unknown`]: anything but the empty id); no single-player remote role stated (the
//!    caller states it; its value as the engine passes it is not read, UNVERIFIED); a
//!    local player that already has a play actor; a player not in the list given. Each is unmarked. The local
//!    player's index in the list is found here. The game info's `Login` is looked up after the counters' mark, since
//!    the lookup can load the function; a class without one is a data error.
//! 2. The URL text's options are joined (`?` before each) and its portal taken; no `Name=` option is added (the
//!    slice's local player has no name: UNVERIFIED).
//! 3. **`Login(Portal, Options, UniqueId, out ErrorMessage)`** on the game info. The id is [`NetIdAnswer::Empty`]: the
//!    parameter's zero value, standing for no online subsystem (UNVERIFIED). A None answer runs nothing after
//!    `Login` (no attachment, no roles, no `PostLogin`, no store, no local player). With an error text it is the
//!    load's failure, [`PlayActorOutcome::LoginFailed`], the text unchanged (NATIVE_MAP_LOAD.md 6.3: what the map load
//!    then does is not read; by stock convention the load fails). With an empty one it is `NotYet { NotRead }`, "Login
//!    returned None without an error" (the hub's wording).
//! 4. On the returned controller: `NetPlayerIndex` = the index, `Player` = the local player (the attachment), `Role` =
//!    authority, `RemoteRole` = the role passed in. Its fields are read first, so a controller that lacks one is
//!    refused before the first of these writes.
//! 5. **`PostLogin(NewPlayer)`** on the game info (silent when the class defines none).
//! 6. The local player's `Actor` = the controller; then [`crate::actors::Actors::add_local_player`], last and on
//!    success only.
//!
//! **Marking.** From step 3 on, script has run. #518's counters are read before it ([`super::game_info::WriteMark`]):
//! a `NotYet` after either moved is marked "stopped after partial writes", its kind and record kept. The refusals of
//! step 1 are never marked. `ParityMode::Reference` is unchanged: the C++ has no map load.

use ow_core::{NotYet, ParityMode};
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::error::VmError;

use super::RECORD;
use super::bringup::game_info;
use super::game_info::WriteMark;
use crate::actors::{self, NET_MODE_CLIENT, ROLE_AUTHORITY, call_with_zeros, send_event};
use crate::world::World;

/// What `GetUniqueNetId` answers for the local player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetIdAnswer {
    /// The empty id (the parameter's zero value): no online subsystem. A port answer, UNVERIFIED.
    Empty,
    /// The answer is not known: refused before any write.
    Unknown,
}

/// What the caller states for the routine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayActorInput<'a> {
    /// The URL's text (the map URL as a string): `map?option?option#portal`.
    pub url: &'a str,
    /// The engine's local-player list (`GamePlayers`), in order.
    pub game_players: &'a [ObjRef],
    pub net_id: NetIdAnswer,
    /// The remote role the routine passes for a local player; `None` when not stated (refused).
    pub remote_role: Option<u8>,
}

/// The play actor [`spawn_play_actor`] made.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayActor {
    pub controller: ObjRef,
    /// The local player's index in `GamePlayers` (the controller's `NetPlayerIndex`).
    pub index: u8,
}

/// What [`spawn_play_actor`] answered.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayActorOutcome {
    /// `Login` gave a controller, and every step ran.
    Spawned(PlayActor),
    /// `Login` gave None with this error text, unchanged: the load's failure (module documentation, step 3).
    LoginFailed { error: String },
}

/// A local player of `class` (a `LocalPlayer` class path) from its class defaults, as `CreatePlayer` makes it before
/// any map: a harness input (module documentation).
pub fn make_local_player(world: &mut World, class: &str) -> Result<ObjRef, VmError> {
    let c = world.vm.find_class(class)?;
    ow_vm::interp::instantiate_from_defaults(&mut world.vm, c)
}

/// The URL text's options joined with `?` before each, and its portal (module documentation, step 2).
pub fn url_options(url: &str) -> (String, String) {
    let (rest, portal) = url.split_once('#').unwrap_or((url, ""));
    let mut parts = rest.split('?');
    parts.next();
    let options: String = parts.filter(|o| !o.is_empty()).map(|o| format!("?{o}")).collect();
    (options, portal.to_string())
}

struct Plan {
    game: ObjRef,
    index: u8,
    remote_role: u8,
    options: String,
    portal: String,
}

fn not_read(what: impl Into<std::borrow::Cow<'static, str>>) -> VmError {
    NotYet::not_read(what, RECORD).into()
}

fn data_error(message: String) -> VmError {
    VmError::Script { function: "the local-player spawn".into(), message: format!("{message} ({RECORD}, section 4)") }
}

/// Step 1: reads only.
fn preflight(world: &World, player: ObjRef, input: &PlayActorInput<'_>) -> Result<Plan, VmError> {
    if world.vm.parity != ParityMode::Off {
        return Err(NotYet::not_ported(
            "the map load's local-player spawn under ParityMode::Reference (the C++ has no map load)",
            RECORD,
        )
        .into());
    }
    let game = game_info(world)?;
    let info = world.actors().try_borrow().map_err(|_| VmError::ServiceBorrowed("Actors"))?.world_info();
    let info = info.ok_or_else(|| data_error("no WorldInfo".into()))?;
    let space = &world.vm.space;
    if actors::byte(space, info, "NetMode")? == NET_MODE_CLIENT {
        return Err(not_read("the local-player spawn on a network client (the client branch)"));
    }
    if input.net_id == NetIdAnswer::Unknown {
        return Err(not_read("GetUniqueNetId's answer for the local player (no online subsystem answer is modelled)"));
    }
    let Some(remote_role) = input.remote_role else {
        return Err(not_read("the remote role the local-player spawn passes for a local player"));
    };
    if actors::object(space, player, "Actor")?.is_some() {
        return Err(not_read("the local-player spawn for a local player that already has a play actor"));
    }
    let index = input
        .game_players
        .iter()
        .position(|&p| p == player)
        .ok_or_else(|| data_error("the local player is not in GamePlayers".into()))?;
    let index = u8::try_from(index).map_err(|_| data_error(format!("a player index of {index}")))?;
    let (options, portal) = url_options(input.url);
    Ok(Plan { game, index, remote_role, options, portal })
}

/// The local-player spawn (module documentation).
pub fn spawn_play_actor(
    world: &mut World,
    player: ObjRef,
    input: &PlayActorInput<'_>,
) -> Result<PlayActorOutcome, VmError> {
    let plan = preflight(world, player, input)?;
    let mark = WriteMark::now(&world.vm);
    run(world, player, &plan).map_err(|e| mark.mark(&world.vm, e))
}

/// Steps 3 to 6.
fn run(world: &mut World, player: ObjRef, plan: &Plan) -> Result<PlayActorOutcome, VmError> {
    let vm = &mut world.vm;
    // The lookup can load the function (a write), so it runs after the counters' mark.
    let (class, state) = {
        let g = vm.space.heap.get(plan.game)?;
        (g.class, g.state)
    };
    let login = vm.find_method(class, "Login", state)?.ok_or_else(|| {
        data_error(format!("{} defines no Login", ow_attr::eval::class_path(&vm.space, plan.game)))
    })?;
    let args = vec![Value::Str(plan.portal.as_str().into()), Value::Str(plan.options.as_str().into())];
    let result = call_with_zeros(vm, login, Some(plan.game), args)?;
    let error = match result.outs.get(3) {
        Some(Value::Str(s)) => s.to_string(),
        other => return Err(data_error(format!("Login's ErrorMessage is {other:?}, expected a string"))),
    };
    let controller = match result.ret {
        Value::Object(Some(c)) => c,
        Value::Object(None) if error.is_empty() => return Err(not_read("Login returned None without an error")),
        Value::Object(None) => return Ok(PlayActorOutcome::LoginFailed { error }),
        other => return Err(data_error(format!("Login returned {}, expected an object", other.kind_name()))),
    };
    // The controller's four fields read before the first write to it.
    for name in ["NetPlayerIndex", "Role", "RemoteRole"] {
        actors::byte(&vm.space, controller, name)?;
    }
    actors::object(&vm.space, controller, "Player")?;
    actors::set_field(&mut vm.space, controller, "NetPlayerIndex", Value::Byte(plan.index))?;
    actors::set_field(&mut vm.space, controller, "Player", Value::Object(Some(player)))?;
    actors::set_field(&mut vm.space, controller, "Role", Value::Byte(ROLE_AUTHORITY))?;
    actors::set_field(&mut vm.space, controller, "RemoteRole", Value::Byte(plan.remote_role))?;
    send_event(vm, plan.game, "PostLogin", vec![Value::Object(Some(controller))])?;
    actors::set_field(&mut vm.space, player, "Actor", Value::Object(Some(controller)))?;
    world.actors().try_borrow_mut().map_err(|_| VmError::ServiceBorrowed("Actors"))?.add_local_player(controller)?;
    Ok(PlayActorOutcome::Spawned(PlayActor { controller, index: plan.index }))
}

#[cfg(test)]
mod tests;
