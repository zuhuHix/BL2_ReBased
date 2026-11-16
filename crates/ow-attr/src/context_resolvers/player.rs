//! Owner: lane FIX-PHASELOCK-CHAIN (decision 2026-11-16-69). The rule of `PlayerControllerAttributeContextResolver`
//! that `ow-world`'s player-controller query runs ([`player_controller_of`]). UNVERIFIED in the running game.

use ow_core::NotYet;
use ow_object::ids::ObjRef;

use super::implements_interface;
use crate::api::{AttrError, WorldParts};

/// The player-behavior interface, the first test (NATIVE_ITEM_RULES.md; a top-level class export of `WillowGame.upk`,
/// checked locally).
pub const PLAYER_BEHAVIOR_INTERFACE: &str = "WillowGame.IPlayerBehavior";
/// The player-master interface, the second test (its one function is `GetPlayerMasterPlayerController`).
pub const PLAYER_MASTER_INTERFACE: &str = "WillowGame.IPlayerMaster";
/// The record of the rule.
pub const RECORD_PLAYER_CONTROLLER: &str =
    "NATIVE_ITEM_RULES.md (PlayerControllerAttributeContextResolver); NATIVE_ATTRIBUTES.md section 5";

const WHO: &str = "PlayerControllerAttributeContextResolver";

/// The player controller reachable from `source`, the rule NATIVE_ITEM_RULES.md records for
/// `PlayerControllerAttributeContextResolver` (read from the executable by an earlier lane):
///
/// | The source | The answer |
/// |---|---|
/// | implements `IPlayerBehavior` | the source |
/// | else implements `IPlayerMaster` | the controller its `GetPlayerMasterPlayerController` returns: `NotYet { NotPorted }` here (no class of the base code packages reaches this row: the record names only `WillowPlayerController` as an implementer, and it implements both) |
/// | else | None |
///
/// The interface tests are [`implements_interface`]'s (a store is needed; a "no" over a native stand-in ancestry is
/// `NotYet`). The C++ evaluator refuses this resolver, so both parity modes run the rule.
pub fn player_controller_of(
    mut parts: WorldParts<'_>,
    source: ObjRef,
) -> Result<Option<ObjRef>, AttrError> {
    let behavior = WorldParts::new(&mut *parts.space, parts.defs.as_deref_mut());
    if implements_interface(behavior, source, PLAYER_BEHAVIOR_INTERFACE, WHO)? {
        return Ok(Some(source));
    }
    let master = WorldParts::new(&mut *parts.space, parts.defs.as_deref_mut());
    if implements_interface(master, source, PLAYER_MASTER_INTERFACE, WHO)? {
        return Err(NotYet::not_ported(
            "PlayerControllerAttributeContextResolver: a source that is an IPlayerMaster and not an IPlayerBehavior \
             (its GetPlayerMasterPlayerController call is not run here)",
            RECORD_PLAYER_CONTROLLER,
        )
        .into());
    }
    Ok(None)
}
