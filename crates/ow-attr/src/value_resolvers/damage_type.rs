//! `WillowGame.DamageTypeAttributeValueResolver` (NATIVE_WEAPON_RULES.md section 5, "Name-part expressions", confirmed
//! live once for the name-part outcome on 2026-10-23): the `ValueIfMatched` data of the first `DamageTypes` entry
//! whose `AssociatedDamageTypeName` is the object name of the context weapon's damage type, else `ValueIfNotMatched`,
//! evaluated with the context as the source and ignoring the previous value, as the manufacturer resolver does.
//! Lane IMPL-DAMAGE-TYPE-RESOLVER, for slice link 8 (equip).
//!
//! **The weapon's damage type** follows NATIVE_STATIC_GET_WEAPON_DAMAGE_TYPE.md (#576): a weapon with no
//! `DefinitionData.WeaponTypeDefinition` has none; otherwise the type's `DefaultDamageTypeDefinition`, replaced by the
//! `DefinitionData.ElementalPartDefinition`'s `CustomDamageTypeDefinition` when that part and its custom type are set.
//! Only what is already resident is read (a load is a write): an unloaded weapon type definition or elemental part is
//! `NotYet { NotRead }` naming it, one whose load failed is `BadData`. The damage type handle itself is never loaded:
//! only its object name is compared.
//!
//! **Edge cases,** as the manufacturer resolver settles them: an empty list and a weapon with no damage type match no
//! entry (`ValueIfNotMatched`); a context with no `DefinitionData` (not an item) is `NotYet { NotRead }`. The name
//! comparison is FName equality, as `tools/weapon_recipe.py` and the manufacturer resolver compare names. Every rule is
//! UNVERIFIED in game beyond the name-part outcome above.

use ow_core::NotYet;
use ow_object::heap::ObjectSpace;
use ow_object::ids::ObjRef;
use ow_object::value::Value;

use super::manufacturer::{definition_data_object, select};
use crate::api::{AttrError, EvalCx, ValueResolver, WorldParts};
use crate::eval::{class_path, field, name, object, required, resident_field};
use crate::registry::AttributeSystem;

/// The resolver class.
pub(crate) const CLASS: &str = "WillowGame.DamageTypeAttributeValueResolver";

/// The record the weapon's damage type comes from.
pub(crate) const DAMAGE_TYPE_RECORD: &str = "NATIVE_STATIC_GET_WEAPON_DAMAGE_TYPE.md";

const RESOLVER: &str = "DamageTypeAttributeValueResolver";

pub(crate) struct DamageType;

/// `obj` must be resident: an unloaded store handle is `NotYet { NotRead }` naming `what`, a failed one `BadData`.
/// Without a store every object is resident.
fn loaded(cx: &mut EvalCx<'_>, obj: ObjRef, what: &str) -> Result<(), AttrError> {
    let WorldParts { space, defs, .. } = cx.world.parts();
    let Some(defs) = defs else { return Ok(()) };
    let named = || {
        let n = space.heap.get(obj).map_or_else(|_| "?".into(), |o| space.names.text(o.name).into_owned());
        format!("{} {n}", class_path(space, obj))
    };
    if defs.is_failed(space, obj) {
        return Err(AttrError::BadData { reason: format!("{RESOLVER}: {what} ({}) failed to load", named()).into() });
    }
    if defs.is_unloaded(space, obj) {
        return Err(NotYet::not_read(
            format!("{RESOLVER}: {what} ({}) is not loaded (a load is a write)", named()),
            DAMAGE_TYPE_RECORD,
        )
        .into());
    }
    Ok(())
}

/// The context weapon's damage type by #576's rule, reading only resident objects (module documentation).
pub(crate) fn weapon_damage_type(cx: &mut EvalCx<'_>, context: ObjRef) -> Result<Option<ObjRef>, AttrError> {
    loaded(cx, context, "the context")?;
    let Some(weapon_type) = definition_data_object(cx, RESOLVER, context, "WeaponTypeDefinition")? else {
        return Ok(None);
    };
    loaded(cx, weapon_type, "the weapon type definition")?;
    let default = object(
        "DefaultDamageTypeDefinition",
        &resident_field(cx, weapon_type, "DefaultDamageTypeDefinition")?,
    )?;
    let Some(part) = definition_data_object(cx, RESOLVER, context, "ElementalPartDefinition")? else {
        return Ok(default);
    };
    loaded(cx, part, "the elemental part definition")?;
    let custom = object("CustomDamageTypeDefinition", &resident_field(cx, part, "CustomDamageTypeDefinition")?)?;
    Ok(custom.or(default))
}

impl ValueResolver for DamageType {
    fn value(
        &self,
        sys: &AttributeSystem,
        resolver: ObjRef,
        context: ObjRef,
        _previous: f32,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        let entries = required(cx, resolver, "DamageTypes")?;
        let damage_type = weapon_damage_type(cx, context)?;
        let type_name = match damage_type {
            Some(t) => Some(cx.world.parts().space.heap.get(t)?.name),
            None => None,
        };
        let matches = |space: &ObjectSpace, entry: &Value| -> Result<bool, AttrError> {
            let field = field(space, entry, "AssociatedDamageTypeName")?;
            Ok(type_name.is_some() && Some(name("AssociatedDamageTypeName", field)?) == type_name)
        };
        select(sys, resolver, context, &entries, matches, cx)
    }
}
