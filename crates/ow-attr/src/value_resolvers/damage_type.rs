//! `WillowGame.DamageTypeAttributeValueResolver` (NATIVE_WEAPON_RULES.md section 5, "Name-part expressions", confirmed
//! live once for the name-part outcome on 2026-10-23): the `ValueIfMatched` data of the first `DamageTypes` entry
//! whose `AssociatedDamageTypeName` is the object name of the context weapon's damage type, else `ValueIfNotMatched`,
//! with the context as the source and ignoring the previous value. Each list entry carries its own `ValueIfMatched`
//! (the installed class layout); the installed lists all hold 1, which is the note's "1 when in the list". Lane
//! IMPL-DAMAGE-TYPE-RESOLVER, for slice link 8 (equip).
//!
//! **Modes:** Off only. Reference keeps the refusal it gave while no class was registered ("value resolver class
//! <path>", NotRead), for the preflight, a value, a base value and the storage alike, as the black-market resolver
//! does (review r1 of #584, item 3).
//!
//! **Read only what is resident** (a load is a write; review r1 of #584, items 1 and 2). The resolver and the context
//! must be resident, and their fields are read without a load. The chosen `AttributeInitializationData` is evaluated
//! only when it is a constant (`BaseValueConstant` times `BaseValueScaleConstant`): one with a `BaseValueAttribute` or
//! an `InitializationDefinition` is `NotYet { NotPorted }` before it is touched, because its evaluation can load or
//! write. The installed lists and fallbacks are constants.
//!
//! **The weapon's damage type** follows NATIVE_STATIC_GET_WEAPON_DAMAGE_TYPE.md (#576): a weapon with no
//! `DefinitionData.WeaponTypeDefinition` has none; otherwise the type's `DefaultDamageTypeDefinition`, replaced by the
//! `DefinitionData.ElementalPartDefinition`'s `CustomDamageTypeDefinition` when that part and its custom type are set.
//! An unloaded resolver, context, weapon type definition or elemental part is `NotYet { NotRead }` naming it, one
//! whose load failed is `BadData`. The damage type handle itself is never loaded: only its object name is compared.
//!
//! **Edge cases,** as the manufacturer resolver settles them: an empty list and a weapon with no damage type match no
//! entry (`ValueIfNotMatched`); a context with no `DefinitionData` (not an item) is `NotYet { NotRead }`. The name
//! comparison is FName equality, as `tools/weapon_recipe.py` and the manufacturer resolver compare names. Every rule is
//! UNVERIFIED in game beyond the name-part outcome above.

use ow_core::{NotYet, ParityMode};
use ow_object::heap::ObjectSpace;
use ow_object::ids::ObjRef;
use ow_object::value::Value;

use super::RECORD;
use crate::api::{AttrError, EvalCx, Storage, ValueResolver, WorldParts};
use crate::eval::{class_path, elements, field, init_data, name, object, parity, resident_field};
use crate::registry::AttributeSystem;

/// The resolver class.
pub(crate) const CLASS: &str = "WillowGame.DamageTypeAttributeValueResolver";

/// The record the weapon's damage type comes from.
pub(crate) const DAMAGE_TYPE_RECORD: &str = "NATIVE_STATIC_GET_WEAPON_DAMAGE_TYPE.md";

const RESOLVER: &str = "DamageTypeAttributeValueResolver";

pub(crate) struct DamageType;

/// Reference's refusal: the one an unregistered class gave (`AttributeSystem::value_resolver`).
fn reference_refusal(space: &ObjectSpace, resolver: ObjRef) -> AttrError {
    NotYet::not_read(format!("value resolver class {}", class_path(space, resolver)), RECORD).into()
}

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

/// The resident context's `DefinitionData` field `name`, read without a load; a context with no `DefinitionData`
/// (not an item) is `NotYet { NotRead }`, as the manufacturer resolver refuses it.
fn definition_data_object(cx: &mut EvalCx<'_>, context: ObjRef, name: &str) -> Result<Option<ObjRef>, AttrError> {
    let space = cx.world.parts().space;
    let class = space.heap.get(context)?.class;
    if space.prop(class, "DefinitionData").is_none() {
        return Err(NotYet::not_read(
            format!("{RESOLVER} on a {} context, which has no DefinitionData", class_path(space, context)),
            RECORD,
        )
        .into());
    }
    let data = resident_field(cx, context, "DefinitionData")?;
    object(name, field(cx.world.parts().space, &data, name)?)
}

/// The context weapon's damage type by #576's rule, reading only resident objects (module documentation).
pub(crate) fn weapon_damage_type(cx: &mut EvalCx<'_>, context: ObjRef) -> Result<Option<ObjRef>, AttrError> {
    loaded(cx, context, "the context")?;
    let Some(weapon_type) = definition_data_object(cx, context, "WeaponTypeDefinition")? else {
        return Ok(None);
    };
    loaded(cx, weapon_type, "the weapon type definition")?;
    let default = object(
        "DefaultDamageTypeDefinition",
        &resident_field(cx, weapon_type, "DefaultDamageTypeDefinition")?,
    )?;
    let Some(part) = definition_data_object(cx, context, "ElementalPartDefinition")? else {
        return Ok(default);
    };
    loaded(cx, part, "the elemental part definition")?;
    let custom = object("CustomDamageTypeDefinition", &resident_field(cx, part, "CustomDamageTypeDefinition")?)?;
    Ok(custom.or(default))
}

/// The constant value of `AttributeInitializationData` `v`, refusing one whose evaluation could load or write.
fn constant(space: &ObjectSpace, v: &Value, what: &str) -> Result<f32, AttrError> {
    let data = init_data(space, v)?;
    if data.base_value_attribute.is_some() || data.initialization_definition.is_some() {
        return Err(NotYet::not_ported(
            format!(
                "{RESOLVER}: a {what} with a BaseValueAttribute or an InitializationDefinition (its evaluation can \
                 load or write; only constants are evaluated read-only)"
            ),
            RECORD,
        )
        .into());
    }
    Ok(data.base_value_constant * data.base_value_scale_constant)
}

/// The Off read (module documentation).
fn read(cx: &mut EvalCx<'_>, resolver: ObjRef, context: ObjRef) -> Result<f32, AttrError> {
    if parity(cx) == ParityMode::Reference {
        return Err(reference_refusal(cx.world.parts().space, resolver));
    }
    loaded(cx, resolver, "the resolver")?;
    let entries = resident_field(cx, resolver, "DamageTypes")?;
    let fallback = resident_field(cx, resolver, "ValueIfNotMatched")?;
    let damage_type = weapon_damage_type(cx, context)?;
    let space = cx.world.parts().space;
    let type_name = match damage_type {
        Some(t) => Some(space.heap.get(t)?.name),
        None => None,
    };
    if type_name.is_some() {
        for entry in elements("DamageTypes", &entries)? {
            let entry_name = name("AssociatedDamageTypeName", field(space, entry, "AssociatedDamageTypeName")?)?;
            if Some(entry_name) == type_name {
                return constant(space, field(space, entry, "ValueIfMatched")?, "ValueIfMatched");
            }
        }
    }
    constant(space, &fallback, "ValueIfNotMatched")
}

impl ValueResolver for DamageType {
    /// Reference refuses at the preflight as the unregistered class did (review r1 of #584, item 3).
    fn availability(&self, space: &ObjectSpace, resolver: ObjRef, mode: ParityMode) -> Result<(), AttrError> {
        match mode {
            ParityMode::Reference => Err(reference_refusal(space, resolver)),
            _ => Ok(()),
        }
    }

    fn value(
        &self,
        _sys: &AttributeSystem,
        resolver: ObjRef,
        context: ObjRef,
        _previous: f32,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        read(cx, resolver, context)
    }

    fn storage(
        &self,
        _sys: &AttributeSystem,
        resolver: ObjRef,
        _context: ObjRef,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<Storage>, AttrError> {
        match parity(cx) {
            ParityMode::Reference => Err(reference_refusal(cx.world.parts().space, resolver)),
            _ => Ok(None),
        }
    }
}
