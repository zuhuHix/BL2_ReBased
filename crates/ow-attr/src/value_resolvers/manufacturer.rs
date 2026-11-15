//! `WillowGame.ManufacturerAttributeValueResolver` (NATIVE_ATTRIBUTES.md section 6, medium confidence): the
//! `ValueIfMatched` data of the first `Manufacturers` entry whose `AssociatedManufacturerName` is the context item's
//! manufacturer, else `ValueIfNotMatched`; evaluated with the context as the source, ignoring the previous value.
//! This replaces the C++ generator's `Weapon_Is_<Maker>` name match (`weapon_gen.cpp` `Evaluator`): the
//! `D_Attributes.WeaponManufacturer.Weapon_Is_<Maker>` attributes are this resolver behind a weapon context.
//!
//! **The item's manufacturer** is read as the context's `DefinitionData.ManufacturerDefinition` (the field of both
//! `WeaponDefinitionData` and `ItemDefinitionData`), and it matches an entry when the manufacturer definition's
//! object name is the entry's name (FName equality, any case). The native asks `WillowInventory.GetManufacturer()`,
//! a native whose body is not read; the definition-data field and the object-name match are what
//! `tools/weapon_recipe.py` and `weapon_gen.cpp` use, which the golden cards (69/69) and the live name-part check of
//! 2026-10-23 (NATIVE_WEAPON_RULES.md section 5) went through. A context with no `DefinitionData` (not an item) is
//! `NotYet { NotRead }`; an item with no manufacturer matches no entry. Status: UNVERIFIED.

use ow_core::NotYet;
use ow_object::heap::ObjectSpace;
use ow_object::ids::ObjRef;
use ow_object::value::Value;

use super::RECORD;
use crate::api::{AttrError, EvalCx, ValueResolver};
use crate::eval::{class_path, elements, evaluate_value, field, name, object, property, required};
use crate::registry::AttributeSystem;

pub(crate) struct Manufacturer;

/// The context item's `DefinitionData` field `name` (an object reference), or `NotYet { NotRead }` for a context
/// that is not an item (no `DefinitionData`).
pub(super) fn definition_data_object(
    cx: &mut EvalCx<'_>,
    resolver_class: &str,
    context: ObjRef,
    name: &str,
) -> Result<Option<ObjRef>, AttrError> {
    let Some(data) = property(cx, context, "DefinitionData")? else {
        let class = class_path(cx.world.parts().space, context);
        return Err(NotYet::not_read(
            format!("{resolver_class} on a {class} context, which has no DefinitionData"),
            RECORD,
        )
        .into());
    };
    let space = cx.world.parts().space;
    object(name, field(space, &data, name)?)
}

/// Evaluates the `ValueIfMatched` of the first entry of `entries` that `matches`, else the resolver's
/// `ValueIfNotMatched`, with the context as the source.
pub(super) fn select(
    sys: &AttributeSystem,
    resolver: ObjRef,
    context: ObjRef,
    entries: &Value,
    matches: impl Fn(&ObjectSpace, &Value) -> Result<bool, AttrError>,
    cx: &mut EvalCx<'_>,
) -> Result<f32, AttrError> {
    for entry in elements("selector entries", entries)? {
        let chosen = {
            let space = cx.world.parts().space;
            if matches(space, entry)? {
                Some(field(space, entry, "ValueIfMatched")?.clone())
            } else {
                None
            }
        };
        if let Some(data) = chosen {
            return evaluate_value(sys, &data, Some(context), None, cx);
        }
    }
    let data = required(cx, resolver, "ValueIfNotMatched")?;
    evaluate_value(sys, &data, Some(context), None, cx)
}

impl ValueResolver for Manufacturer {
    fn value(
        &self,
        sys: &AttributeSystem,
        resolver: ObjRef,
        context: ObjRef,
        _previous: f32,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        let entries = required(cx, resolver, "Manufacturers")?;
        let maker = definition_data_object(
            cx,
            "ManufacturerAttributeValueResolver",
            context,
            "ManufacturerDefinition",
        )?;
        let maker_name = match maker {
            Some(m) => Some(cx.world.parts().space.heap.get(m)?.name),
            None => None,
        };
        let matches = |space: &ObjectSpace, entry: &Value| -> Result<bool, AttrError> {
            let field = field(space, entry, "AssociatedManufacturerName")?;
            Ok(Some(name("AssociatedManufacturerName", field)?) == maker_name)
        };
        select(sys, resolver, context, &entries, matches, cx)
    }
}
