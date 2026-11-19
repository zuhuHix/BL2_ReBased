//! Owner: lane MAP-LOAD-NATIVES. Bringing a loaded map up for play (NATIVE_MAP_LOAD.md, sections 2 to 4), coded from
//! the analyst note alone by a lane that did not read the executable (clean room, docs/LANE_BRIEF.md). AI-assisted
//! (Claude Code). Decision `2026-11-19-04`. Every rule here is **UNVERIFIED** in the running game.
//!
//! [`bring_up_for_play`] runs the rules the note states and returns, as `NotYet`, every step whose rule it leaves
//! open. Today it changes nothing in the world:
//!
//! - **The game info** (section 3). The routine does nothing when the persistent level's `WorldInfo.Game` already
//!   holds a game info (high in the note); that is the one branch coded. A network or demo driver, the routine's other
//!   reason to do nothing, never exists in this world (it has no network layer). Otherwise the class choice is
//!   `NotYet`: the note does not name the world info field and the engine check that select the configured
//!   `DefaultGame`, the URL option parse was not read, and neither were the spawn's location, rotation, owner and
//!   instigator.
//! - **The begin-play pass** (section 2, step 4, and 2.1) is `NotYet` as a whole. The note leaves open the identity
//!   of the per-actor virtuals ("by convention `PreBeginPlay` and `PostBeginPlay`, not confirmed": its open items ask
//!   for them to be confirmed before implementing), the second flag that lets a static actor be routed, the fields the
//!   initialisation pass resets and its unidentified engine calls. Setting `bBegunPlay` without the routing would
//!   leave every placed actor without the events the flag says it had, and would switch [`crate::actors::spawn`] to
//!   its after-play rules, so the flag is not set either. [`begin_play_reach`] counts what the routing would reach.
//! - **The rest of the bring-up** (section 2, steps 1, 2, 3 and 5): the time reset's flag and fields, the URL options
//!   and the setup and per-level engine calls are not read (low in the note).
//! - **The local player's play actor** (section 4): the engine routine `LocalPlayer.SpawnPlayActor` calls was not
//!   read, and neither was where the engine fills `GamePlayers` (open, note section 4). This project's hook for that
//!   list is `Actors::add_local_player` (decision 2026-11-16-81), which the caller that makes the player's controller
//!   calls.
//! - **Step 6** (the `BandwidthLimit=` URL option passed to the game info, and the memory-check option) is left out on
//!   purpose: a network setting and debug support, not behaviour a single-player run depends on.
//!
//! Both parity modes run the same rules: the C++ has no map load, so there is no Reference stand-in (R3).

use ow_core::NotYet;
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::error::VmError;

use super::RECORD;
use crate::actors::{self, is_destroyed};
use crate::world::World;

/// What the begin-play routing (NATIVE_MAP_LOAD.md, section 2.1) would reach over the world's actor list, as far as
/// the note settles it. Null list entries are never placed, so none is counted.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BeginPlayReach {
    /// Actors in the world's list, the `WorldInfo` included.
    pub actors: usize,
    /// Pending deletion (read as `actors::is_destroyed`, UNVERIFIED): both passes skip them.
    pub pending_deletion: usize,
    /// Not static: pass 1's first event reaches them.
    pub not_static: usize,
    /// Static: pass 1's event and pass 2's component calls reach them only with the note's unnamed second flag (open);
    /// pass 2's first call reaches them regardless.
    pub static_open: usize,
    /// The persistent `WorldInfo`'s `bBegunPlay` (the routine's once test), `None` when its class declares no such
    /// field.
    pub world_info_begun_play: Option<bool>,
}

/// The routing's reach over `world`'s actor list (module documentation).
pub fn begin_play_reach(world: &World) -> Result<BeginPlayReach, VmError> {
    let actors = world.actors();
    let a = actors
        .try_borrow()
        .map_err(|_| VmError::ServiceBorrowed("Actors"))?;
    let space = &world.vm.space;
    let mut reach = BeginPlayReach {
        actors: a.list().len(),
        ..BeginPlayReach::default()
    };
    for &o in a.list() {
        if is_destroyed(space, o)? {
            reach.pending_deletion += 1;
        } else if actors::flag(space, o, "bStatic")? {
            reach.static_open += 1;
        } else {
            reach.not_static += 1;
        }
    }
    if let Some(info) = a.world_info() {
        let class = space.heap.get(info)?.class;
        if space.prop(class, "bBegunPlay").is_some() {
            reach.world_info_begun_play = Some(actors::flag(space, info, "bBegunPlay")?);
        }
    }
    Ok(reach)
}

/// The game info routine (NATIVE_MAP_LOAD.md, section 3): the persistent `WorldInfo.Game` when it is set (the routine
/// then does nothing); otherwise `NotYet`, since the class choice is not settled by the note. No `WorldInfo` placed is
/// `NotYet` naming the map load. A class that declares no object property `Game` is a typed error, never a default.
pub fn game_info(world: &World) -> Result<ObjRef, VmError> {
    let info = world
        .actors()
        .try_borrow()
        .map_err(|_| VmError::ServiceBorrowed("Actors"))?
        .world_info();
    let Some(info) = info else {
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
            "the game info's class choice (NATIVE_MAP_LOAD.md section 3: which world info field and engine check \
             select the configured DefaultGame, the URL option parse, and the spawn's location, rotation, owner and \
             instigator)",
            RECORD,
        )
        .into()),
        other => Err(VmError::Script {
            function: "Game".into(),
            message: format!(
                "WorldInfo.Game holds {}, expected an object ({RECORD}, section 3)",
                other.kind_name()
            ),
        }),
    }
}

/// What [`bring_up_for_play`] found.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct BringUp {
    /// The world's game info, when the routine had one to keep.
    pub game: Option<ObjRef>,
    pub reach: BeginPlayReach,
    /// The steps not run, in the routine's order (module documentation).
    pub pending: Vec<NotYet>,
}

/// Brings `world`'s loaded map up for play as far as NATIVE_MAP_LOAD.md settles it (module documentation). A
/// `NotYet` from a step goes to [`BringUp::pending`]; any other error is returned.
pub fn bring_up_for_play(world: &World) -> Result<BringUp, VmError> {
    let reach = begin_play_reach(world)?;
    let mut pending = vec![NotYet::not_read(
        "the bring-up's time reset (its flag's source, and the three world info fields checked against the class \
         layout), its URL options and its setup calls (NATIVE_MAP_LOAD.md section 2, steps 1 to 3)",
        RECORD,
    )];
    let game = match game_info(world) {
        Ok(game) => Some(game),
        Err(VmError::NotYet(n)) => {
            pending.push(n);
            None
        }
        Err(e) => return Err(e),
    };
    pending.push(NotYet::not_read(
        "the begin-play pass (NATIVE_MAP_LOAD.md section 2 step 4 and 2.1: the engine call after the begun-play flag \
         (the game info call with two outputs), the engine call on a second object when it exists, the per-actor \
         virtuals' identities, whether pass 2's second event is under the static condition, the \
         second flag that routes a static actor, the initialisation pass's field resets and engine calls)",
        RECORD,
    ));
    pending.push(NotYet::not_read(
        "the per-level engine call after begin play (NATIVE_MAP_LOAD.md section 2, step 5)",
        RECORD,
    ));
    pending.push(NotYet::not_read(
        "the local player's play actor (NATIVE_MAP_LOAD.md section 4: the engine routine SpawnPlayActor calls, and \
         where the engine fills GamePlayers)",
        RECORD,
    ));
    Ok(BringUp {
        game,
        reach,
        pending,
    })
}

#[cfg(test)]
mod tests;
