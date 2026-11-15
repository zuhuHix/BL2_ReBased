//! Owner: lane W3-ATTRCTX. The context resolver classes (`AttributeContextResolver.GetAttributeContext`,
//! NATIVE_ATTRIBUTES.md section 5), registered by [`register`], which [`AttributeSystem::standard`] calls.
//!
//! A context resolver turns what the previous step of a definition's `ContextResolverChain` answered (the source,
//! for the first step) into the next object, or `None`, which ends the chain. The chain walk itself (first `None`
//! ends it, an empty chain is `None`, the override source is tried first) is the evaluator's (`eval.rs`, lane
//! W3-ATTREVAL); this module holds one step per class. Which order the stored chains are evaluated in is the stored
//! order, and whether that is the game's order is UNVERIFIED (decision 2026-11-15-08).
//!
//! **Every rule here is UNVERIFIED in the running game.** Each resolver's doc comment gives the confidence the record
//! gives it and what is not read.
//!
//! Plain property hops (a controller's `Pawn`, a pawn's `Weapon`) are read from the object space. What needs the
//! running engine (a pool manager, an inventory manager, an interface call) goes through
//! [`AttrWorld::engine_query`](crate::api::AttrWorld::engine_query); a resolver that needs a query the contract does
//! not have yet answers `NotYet`, never a guess.
//!
//! Class paths: a class is registered only under a path the repository records (the records, `src/`,
//! `research/native_by_class.json`) or that was read from the installed code packages ([`READ_FROM_PACKAGES`],
//! decision 2026-11-15-22). A path is a name of a game object; reading it settles where the class lives, not what
//! its resolver does, so each rule keeps its label.

mod actors;
mod inventory;
mod pending;
mod pools;
#[cfg(test)]
mod tests;

pub use actors::{
    Controller, GameInfo, NoContextNeeded, OffHandWeapon, Pawn, PlayerController,
    PlayerReplicationInfo, Weapon, WillowMind,
};
pub use inventory::{EquippedInventory, Inventory};
pub use pending::{BalancedActor, Designer, Skill, StatusEffectChance, Unread};
pub use pools::{ResourcePool, WeaponResourcePool};

use ow_core::NotYet;
use ow_object::heap::ObjectSpace;
use ow_object::ids::ObjRef;
use ow_object::value::Value;

use crate::api::{AttrError, EvalCx};
use crate::registry::AttributeSystem;

/// The record every resolver of this module names.
pub(crate) const RECORD: &str = "NATIVE_ATTRIBUTES.md section 5";

/// The resolver classes whose package was read from the installed code packages' export tables (each a direct
/// subclass of `Engine.AttributeContextResolver`; decision 2026-11-15-22). No record named their package before.
pub const READ_FROM_PACKAGES: &[&str] = &[
    "Engine.OffHandWeaponAttributeContextResolver",
    "Engine.WeaponResourcePoolAttributeContextResolver",
    "WillowGame.PlayerControllerAttributeContextResolver",
    "WillowGame.EquippedInventoryAttributeContextResolver",
    "WillowGame.SkillAttributeContextResolver",
];

/// Where an unread class is known from: the notes' list of classes not read.
const NOT_READ_YET: &str = "NATIVE_ATTRIBUTES.md \"Not read yet\"";
/// Where an unread class is known from: named in the notes' section 5 table, body not read in detail.
const SECTION_5_UNREAD: &str = "NATIVE_ATTRIBUTES.md section 5 (named, not read in detail)";
/// Where an unread class is known from: the native-function census only, not the notes.
const CENSUS_ONLY: &str = "research/native_by_class.json (not in the notes)";

/// Context resolver classes whose rule is not read, each under the path `research/native_by_class.json` records and
/// with the place it is known from. Each is registered as [`Unread`], so a chain naming one says which class is
/// missing.
///
/// Not listed (an unregistered class answers the registry's `NotYet { NotRead }` all the same): the context resolver
/// classes the code packages export that no note names, `WillowGame.InstanceDataContextResolver` among them (the
/// packages show it is a direct subclass of `Engine.AttributeContextResolver`; decision 2026-11-15-22 lists the rest).
pub const UNREAD: &[(&str, &str)] = &[
    ("Engine.ProjectileAttributeContextResolver", NOT_READ_YET),
    ("Engine.VehicleAttributeContextResolver", NOT_READ_YET),
    (
        "GearboxFramework.ActorAttributeContextResolver",
        NOT_READ_YET,
    ),
    (
        "GearboxFramework.PopulationMasterAttributeContextResolver",
        NOT_READ_YET,
    ),
    (
        "WillowGame.WillowInventoryManagerAttributeContextResolver",
        NOT_READ_YET,
    ),
    (
        "WillowGame.WillowInteractiveObjectAttributeContextResolver",
        NOT_READ_YET,
    ),
    (
        "WillowGame.CurrentProficiencySkillAttributeContextResolver",
        SECTION_5_UNREAD,
    ),
    ("Engine.OwnerAttributeContextResolver", CENSUS_ONLY),
    (
        "Engine.GameReplicationInfoAttributeContextResolver",
        CENSUS_ONLY,
    ),
    (
        "GearboxFramework.AIComponentAttributeContextResolver",
        CENSUS_ONLY,
    ),
    (
        "WillowGame.InventoryDefinitionAttributeContextResolver",
        CENSUS_ONLY,
    ),
];

/// Registers every context resolver class this module implements under its recorded path ([`READ_FROM_PACKAGES`]
/// included), `NoContextNeeded` included, and the [`UNREAD`] classes as [`Unread`].
pub(crate) fn register(sys: AttributeSystem) -> Result<AttributeSystem, AttrError> {
    let mut sys = sys
        .with_context_resolver(
            "GearboxFramework.NoContextNeededAttributeContextResolver",
            NoContextNeeded,
        )?
        .with_context_resolver("Engine.ControllerAttributeContextResolver", Controller)?
        .with_context_resolver("Engine.PawnAttributeContextResolver", Pawn)?
        .with_context_resolver("Engine.WeaponAttributeContextResolver", Weapon)?
        .with_context_resolver(
            "Engine.OffHandWeaponAttributeContextResolver",
            OffHandWeapon,
        )?
        .with_context_resolver(
            "Engine.WeaponResourcePoolAttributeContextResolver",
            WeaponResourcePool,
        )?
        .with_context_resolver(
            "WillowGame.PlayerControllerAttributeContextResolver",
            PlayerController,
        )?
        .with_context_resolver(
            "WillowGame.EquippedInventoryAttributeContextResolver",
            EquippedInventory,
        )?
        .with_context_resolver("WillowGame.SkillAttributeContextResolver", Skill)?
        .with_context_resolver("Engine.ResourcePoolAttributeContextResolver", ResourcePool)?
        .with_context_resolver("Engine.GameInfoAttributeContextResolver", GameInfo)?
        .with_context_resolver(
            "Engine.BalancedActorAttributeContextResolver",
            BalancedActor,
        )?
        .with_context_resolver(
            "WillowGame.PlayerReplicationInfoAttributeContextResolver",
            PlayerReplicationInfo,
        )?
        .with_context_resolver("WillowGame.WillowMindAttributeContextResolver", WillowMind)?
        .with_context_resolver("WillowGame.InventoryAttributeContextResolver", Inventory)?
        .with_context_resolver(
            "WillowGame.StatusEffectChanceModifierResolver",
            StatusEffectChance,
        )?
        .with_context_resolver(
            "WillowGame.DesignerAttributeContextResolver",
            Designer { by_name: false },
        )?
        .with_context_resolver(
            "WillowGame.DesignerAttributeContextResolverByName",
            Designer { by_name: true },
        )?;
    for &(class, record) in UNREAD {
        sys = sys.with_context_resolver(class, Unread::new(record))?;
    }
    Ok(sys)
}

// ---------------------------------------------------------------------------------------------------------------
// Helpers shared by the resolver files
// ---------------------------------------------------------------------------------------------------------------

/// Whether `o` is an object of the class at `base_path` or a subclass. A space with no such class has no such
/// object (`false`). An ancestry that ends at an engine-native stand-in (a class defined only by its import path, with
/// no super class) cannot answer, so that is `NotYet { NotRead }` rather than a `false` that may be wrong.
pub(crate) fn is_a(space: &ObjectSpace, o: ObjRef, base_path: &str) -> Result<bool, AttrError> {
    let Some(base) = space.classes.by_path(base_path) else {
        return Ok(false);
    };
    let class = space.heap.get(o)?.class;
    if space.classes.is_child_of(class, base) {
        return Ok(true);
    }
    let mut cursor = Some(class);
    while let Some(c) = cursor {
        if space.classes.is_native(c) {
            let path = space
                .classes
                .get_layout(c)
                .map(|l| l.path().as_str().to_string())
                .unwrap_or_default();
            return Err(NotYet::not_read(
                format!("whether an object of the native stand-in class {path} is a {base_path}"),
                RECORD,
            )
            .into());
        }
        cursor = space.classes.get_layout(c).and_then(|l| l.super_class());
    }
    Ok(false)
}

/// The object reference held by property `name` of `o`: `Ok(None)` when the class declares no such property,
/// `Ok(Some(None))` for a None reference. An object or interface property; another kind is bad data.
pub(crate) fn object_prop(
    space: &ObjectSpace,
    o: ObjRef,
    name: &str,
) -> Result<Option<Option<ObjRef>>, AttrError> {
    let class = space.heap.get(o)?.class;
    let Some(prop) = space.prop(class, name) else {
        return Ok(None);
    };
    match space.read(o, prop.into())? {
        Value::Object(r) | Value::Interface(r) => Ok(Some(*r)),
        other => Err(AttrError::BadData {
            reason: format!(
                "property {name} is read as an object reference, it holds {}",
                other.kind_name()
            )
            .into(),
        }),
    }
}

/// [`object_prop`] where the class must declare the property: a class without it is `NotYet { NotRead }` (what the
/// native does for such a class is not read).
pub(crate) fn required_object_prop(
    space: &ObjectSpace,
    o: ObjRef,
    name: &str,
    resolver: &'static str,
) -> Result<Option<ObjRef>, AttrError> {
    match object_prop(space, o, name)? {
        Some(r) => Ok(r),
        None => Err(not_declared(space, o, name, resolver)),
    }
}

/// `NotYet { NotRead }`: `resolver` read property `name` on an object whose class does not declare it.
pub(crate) fn not_declared(
    space: &ObjectSpace,
    o: ObjRef,
    name: &str,
    resolver: &'static str,
) -> AttrError {
    let class = space
        .heap
        .get(o)
        .ok()
        .and_then(|x| space.classes.get_layout(x.class))
        .map(|l| l.path().as_str().to_string())
        .unwrap_or_default();
    NotYet::not_read(
        format!("{resolver} on an object of class {class}, which declares no {name}"),
        RECORD,
    )
    .into()
}

/// Makes sure the resolver object `o` has its slots before its fields are read. A definition reached through a
/// chain may be one of the store's unloaded handles (slotless, architecture section 3.4): it is loaded in place
/// ([`DefStore::is_unloaded`](ow_defs::DefStore::is_unloaded), then
/// [`DefStore::ensure_loaded`](ow_defs::DefStore::ensure_loaded)). Any other object is read as it is; a slotless one
/// then fails in the heap (`SlotMissing`), never with a guessed value. A world without a store that hands over a
/// slotless object answers `NotYet { LayerAbsent("ow-defs") }`.
pub(crate) fn ensure_loaded(cx: &mut EvalCx<'_>, o: ObjRef) -> Result<(), AttrError> {
    let parts = cx.world.parts();
    let space = &mut *parts.space;
    match parts.defs {
        Some(defs) => {
            if defs.is_unloaded(space, o) {
                defs.ensure_loaded(space, o)?;
            }
            Ok(())
        }
        None => {
            let object = space.heap.get(o)?;
            let needed = space
                .classes
                .get_layout(object.class)
                .map(|l| l.slot_count() as usize)
                .unwrap_or(0);
            if object.slots.len() >= needed {
                return Ok(());
            }
            Err(NotYet::layer_absent(
                "loading an unloaded resolver definition (this world has no definition store)",
                "ow-defs",
                RECORD,
            )
            .into())
        }
    }
}
