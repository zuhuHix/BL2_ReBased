//! Owner: lane W3-ATTREVAL. The value resolver classes the notes read (NATIVE_ATTRIBUTES.md section 6), one module
//! file each. [`register`] is what [`AttributeSystem::standard`] calls (placed by W2-ATTRAPI); a class not
//! registered here stays `NotYet { NotRead }` (the registry's answer).
//!
//! Each resolver reads its own fields through [`crate::eval::property`], so a resolver object that is still an
//! unloaded definition handle is loaded first. Every rule is **UNVERIFIED in the running game**; the per-class
//! confidence is the record's. A class whose rule needs a layer this crate cannot reach (the skill tree, the player
//! profile) or a step the record does not read (the pool-state test, the slot provider) is registered and answers
//! the precise `NotYet`, so the message names what is missing instead of the generic "class not read".
//!
//! Class default of every resolver here (section 6): the base value is the value, and add, remove and set-base
//! answer false (`ValueResolver::storage` is `None`); only `ObjectPropertyAttributeValueResolver` is writable.

use crate::api::AttrError;
use crate::registry::AttributeSystem;

mod attribute_slot_effect;
mod badass;
mod conditional;
mod constant;
mod global;
mod manufacturer;
mod object_property;
mod player_skill;
mod resource_pool_state;
mod simple_math;
mod weapon_type;

#[cfg(test)]
mod tests;
#[cfg(test)]
pub(crate) mod testworld;

/// The record the value resolvers come from.
pub(crate) const RECORD: &str = "NATIVE_ATTRIBUTES.md section 6";

/// Every value resolver class this module implements, by its class path in the packages.
pub(crate) const CLASSES: [&str; 11] = [
    "Engine.ObjectPropertyAttributeValueResolver",
    "GearboxFramework.ConstantAttributeValueResolver",
    "GearboxFramework.SimpleMathValueResolver",
    "GearboxFramework.ConditionalAttributeValueResolver",
    "WillowGame.GlobalAttributeValueResolver",
    "WillowGame.PlayerSkillAttributeValueResolver",
    "WillowGame.ResourcePoolStateAttributeValueResolver",
    "WillowGame.AttributeSlotEffectAttributeValueResolver",
    "WillowGame.BadassAttributeValueResolver",
    "WillowGame.ManufacturerAttributeValueResolver",
    "WillowGame.WeaponTypeAttributeValueResolver",
];

/// Registers every value resolver class this module implements ([`CLASSES`], in that order).
pub(crate) fn register(sys: AttributeSystem) -> Result<AttributeSystem, AttrError> {
    sys.with_value_resolver(CLASSES[0], object_property::ObjectProperty)?
        .with_value_resolver(CLASSES[1], constant::Constant)?
        .with_value_resolver(CLASSES[2], simple_math::SimpleMath)?
        .with_value_resolver(CLASSES[3], conditional::Conditional)?
        .with_value_resolver(CLASSES[4], global::Global)?
        .with_value_resolver(CLASSES[5], player_skill::PlayerSkill)?
        .with_value_resolver(CLASSES[6], resource_pool_state::ResourcePoolState)?
        .with_value_resolver(CLASSES[7], attribute_slot_effect::AttributeSlotEffect)?
        .with_value_resolver(CLASSES[8], badass::Badass)?
        .with_value_resolver(CLASSES[9], manufacturer::Manufacturer)?
        .with_value_resolver(CLASSES[10], weapon_type::WeaponType)
}
