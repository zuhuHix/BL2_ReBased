//! Owner: lane W3-ATTRCTX. The actor resolvers: NoContextNeeded, Controller, Pawn, Weapon, OffHandWeapon,
//! PlayerReplicationInfo, PlayerController, GameInfo, and WillowMind (NATIVE_ATTRIBUTES.md section 5; WillowMind from
//! the C++ oracle); Actor from its class script (lane FIX-ATTR-CONTEXTS, decision 2026-11-16-49). Plain property hops
//! over the object space; UNVERIFIED in the running game.

use ow_object::ids::ObjRef;

use super::{is_a, object_prop, required_object_prop};
use crate::api::{AttrError, ContextResolver, EngineQuery, EvalCx};
use crate::registry::AttributeSystem;

const ACTOR: &str = "Engine.Actor";
const PAWN: &str = "Engine.Pawn";
const CONTROLLER: &str = "Engine.Controller";
const WEAPON: &str = "Engine.Weapon";
const REPLICATION_INFO: &str = "Engine.PlayerReplicationInfo";
const MIND: &str = "WillowGame.WillowMind";
const AI_PAWN: &str = "WillowGame.WillowAIPawn";

/// `NoContextNeededAttributeContextResolver`: the given object, or the engine globals object for a None source;
/// never None (confidence high). A world without globals answers `NotYet` ([`crate::api::AttrWorld::globals`]).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoContextNeeded;

impl ContextResolver for NoContextNeeded {
    fn resolve(
        &self,
        _sys: &AttributeSystem,
        _resolver: ObjRef,
        _attr: ObjRef,
        source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<ObjRef>, AttrError> {
        match source {
            Some(s) => Ok(Some(s)),
            None => cx.world.globals().map(Some),
        }
    }
}

/// `ControllerAttributeContextResolver` (confidence medium): a controller -> itself; an actor with an `Instigator`
/// -> the instigator's `Controller`; a pawn without one -> its `DrivenVehicle`'s `Controller` when it drives, else
/// its own `Controller`; anything else -> None.
#[derive(Debug, Clone, Copy, Default)]
pub struct Controller;

impl ContextResolver for Controller {
    fn resolve(
        &self,
        _sys: &AttributeSystem,
        _resolver: ObjRef,
        _attr: ObjRef,
        source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<ObjRef>, AttrError> {
        const WHO: &str = "ControllerAttributeContextResolver";
        let Some(s) = source else { return Ok(None) };
        let parts = cx.world.parts();
        let space = &*parts.space;
        if is_a(space, s, CONTROLLER)? {
            return Ok(Some(s));
        }
        if !is_a(space, s, ACTOR)? {
            return Ok(None);
        }
        if let Some(instigator) = required_object_prop(space, s, "Instigator", WHO)? {
            return required_object_prop(space, instigator, "Controller", WHO);
        }
        if !is_a(space, s, PAWN)? {
            return Ok(None);
        }
        match required_object_prop(space, s, "DrivenVehicle", WHO)? {
            Some(vehicle) => required_object_prop(space, vehicle, "Controller", WHO),
            None => required_object_prop(space, s, "Controller", WHO),
        }
    }
}

/// `PawnAttributeContextResolver` (confidence medium): a pawn -> itself; a controller -> its `Pawn`; another
/// actor -> its `Instigator`; anything else -> None.
#[derive(Debug, Clone, Copy, Default)]
pub struct Pawn;

impl ContextResolver for Pawn {
    fn resolve(
        &self,
        _sys: &AttributeSystem,
        _resolver: ObjRef,
        _attr: ObjRef,
        source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<ObjRef>, AttrError> {
        const WHO: &str = "PawnAttributeContextResolver";
        let Some(s) = source else { return Ok(None) };
        let parts = cx.world.parts();
        let space = &*parts.space;
        if is_a(space, s, PAWN)? {
            Ok(Some(s))
        } else if is_a(space, s, CONTROLLER)? {
            required_object_prop(space, s, "Pawn", WHO)
        } else if is_a(space, s, ACTOR)? {
            required_object_prop(space, s, "Instigator", WHO)
        } else {
            Ok(None)
        }
    }
}

/// The pawn a weapon resolver reads from: a pawn itself, a controller's `Pawn`, else None.
fn pawn_of(
    space: &ow_object::heap::ObjectSpace,
    s: ObjRef,
    who: &'static str,
) -> Result<Option<ObjRef>, AttrError> {
    if is_a(space, s, PAWN)? {
        Ok(Some(s))
    } else if is_a(space, s, CONTROLLER)? {
        required_object_prop(space, s, "Pawn", who)
    } else {
        Ok(None)
    }
}

/// `WeaponAttributeContextResolver` (confidence high): a weapon -> itself; a pawn -> its `Weapon`; a controller ->
/// its pawn's `Weapon`; anything else -> None.
#[derive(Debug, Clone, Copy, Default)]
pub struct Weapon;

impl ContextResolver for Weapon {
    fn resolve(
        &self,
        _sys: &AttributeSystem,
        _resolver: ObjRef,
        _attr: ObjRef,
        source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<ObjRef>, AttrError> {
        const WHO: &str = "WeaponAttributeContextResolver";
        let Some(s) = source else { return Ok(None) };
        let parts = cx.world.parts();
        let space = &*parts.space;
        if is_a(space, s, WEAPON)? {
            return Ok(Some(s));
        }
        match pawn_of(space, s, WHO)? {
            Some(pawn) => required_object_prop(space, pawn, "Weapon", WHO),
            None => Ok(None),
        }
    }
}

/// `OffHandWeaponAttributeContextResolver` (confidence high): a weapon -> itself; a pawn or a controller -> the
/// pawn's `OffHandWeapon`; anything else -> None. A pawn class without `OffHandWeapon` is `NotYet { NotRead }`.
/// Registered as `Engine.OffHandWeaponAttributeContextResolver` ([`super::READ_FROM_PACKAGES`]).
#[derive(Debug, Clone, Copy, Default)]
pub struct OffHandWeapon;

impl ContextResolver for OffHandWeapon {
    fn resolve(
        &self,
        _sys: &AttributeSystem,
        _resolver: ObjRef,
        _attr: ObjRef,
        source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<ObjRef>, AttrError> {
        const WHO: &str = "OffHandWeaponAttributeContextResolver";
        let Some(s) = source else { return Ok(None) };
        let parts = cx.world.parts();
        let space = &*parts.space;
        if is_a(space, s, WEAPON)? {
            return Ok(Some(s));
        }
        match pawn_of(space, s, WHO)? {
            Some(pawn) => required_object_prop(space, pawn, "OffHandWeapon", WHO),
            None => Ok(None),
        }
    }
}

/// `PlayerReplicationInfoAttributeContextResolver` (confidence medium): a controller or a pawn -> its
/// `PlayerReplicationInfo`; a replication info -> itself; anything else -> None.
#[derive(Debug, Clone, Copy, Default)]
pub struct PlayerReplicationInfo;

impl ContextResolver for PlayerReplicationInfo {
    fn resolve(
        &self,
        _sys: &AttributeSystem,
        _resolver: ObjRef,
        _attr: ObjRef,
        source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<ObjRef>, AttrError> {
        const WHO: &str = "PlayerReplicationInfoAttributeContextResolver";
        let Some(s) = source else { return Ok(None) };
        let parts = cx.world.parts();
        let space = &*parts.space;
        if is_a(space, s, REPLICATION_INFO)? {
            Ok(Some(s))
        } else if is_a(space, s, CONTROLLER)? || is_a(space, s, PAWN)? {
            required_object_prop(space, s, "PlayerReplicationInfo", WHO)
        } else {
            Ok(None)
        }
    }
}

/// `PlayerControllerAttributeContextResolver` (confidence medium-low): the player controller reachable from the
/// source, itself or through its owner interface (NATIVE_ITEM_RULES.md: `IPlayerBehavior`, then `IPlayerMaster`).
/// The interface tests are the engine's: [`EngineQuery::PlayerController`]. A None source -> None.
/// Registered as `WillowGame.PlayerControllerAttributeContextResolver` ([`super::READ_FROM_PACKAGES`]).
#[derive(Debug, Clone, Copy, Default)]
pub struct PlayerController;

impl ContextResolver for PlayerController {
    fn resolve(
        &self,
        _sys: &AttributeSystem,
        _resolver: ObjRef,
        _attr: ObjRef,
        source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<ObjRef>, AttrError> {
        match source {
            Some(s) => cx.world.engine_query(EngineQuery::PlayerController, s),
            None => Ok(None),
        }
    }
}

/// `GameInfoAttributeContextResolver` (confidence medium): the game info, whatever the source
/// ([`crate::api::AttrWorld::game_info`]; `None` only in a world that models none).
#[derive(Debug, Clone, Copy, Default)]
pub struct GameInfo;

impl ContextResolver for GameInfo {
    fn resolve(
        &self,
        _sys: &AttributeSystem,
        _resolver: ObjRef,
        _attr: ObjRef,
        _source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<ObjRef>, AttrError> {
        cx.world.game_info()
    }
}

/// `GearboxFramework.ActorAttributeContextResolver` (script; lane FIX-ATTR-CONTEXTS, decision 2026-11-16-49): the
/// source if it is an `Engine.Actor`, else None (a None source included). Its only function,
/// `GetAttributeContext`, is script that returns the source cast to `Actor`, read from the installed
/// `GearboxFramework.upk` with `research/script_disasm.py` (it decodes exactly). The rule is that cast, run here
/// without the VM. The C++ evaluator refuses the class ("unsupported attribute context resolver",
/// `src/progression.cpp`), so there is no Reference reading and both modes answer the same. A class ancestry through
/// an engine-native stand-in is `NotYet { NotRead }` ([`is_a`]). UNVERIFIED in the running game.
#[derive(Debug, Clone, Copy, Default)]
pub struct Actor;

impl ContextResolver for Actor {
    fn resolve(
        &self,
        _sys: &AttributeSystem,
        _resolver: ObjRef,
        _attr: ObjRef,
        source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<ObjRef>, AttrError> {
        let Some(s) = source else { return Ok(None) };
        let parts = cx.world.parts();
        if is_a(parts.space, s, ACTOR)? {
            Ok(Some(s))
        } else {
            Ok(None)
        }
    }
}

/// `WillowMindAttributeContextResolver`: not in the native notes; its rule is the C++ oracle's (`src/combat.cpp`
/// `describe`, `src/progression.cpp`): a `WillowMind` -> itself; a `WillowAIPawn` -> its `MyWillowMind`; anything
/// else -> None. The native body is not read; UNVERIFIED.
#[derive(Debug, Clone, Copy, Default)]
pub struct WillowMind;

impl ContextResolver for WillowMind {
    fn resolve(
        &self,
        _sys: &AttributeSystem,
        _resolver: ObjRef,
        _attr: ObjRef,
        source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<ObjRef>, AttrError> {
        let Some(s) = source else { return Ok(None) };
        let parts = cx.world.parts();
        let space = &*parts.space;
        if is_a(space, s, MIND)? {
            return Ok(Some(s));
        }
        if is_a(space, s, AI_PAWN)? {
            // The C++ reads it as optional: a pawn without the property has no mind.
            return Ok(object_prop(space, s, "MyWillowMind")?.flatten());
        }
        Ok(None)
    }
}
