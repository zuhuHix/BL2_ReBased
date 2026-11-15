//! `WillowGame.WeaponTypeAttributeValueResolver` (NATIVE_ATTRIBUTES.md section 6, medium confidence): as the
//! manufacturer resolver, keyed by the weapon type byte: the `ValueIfMatched` of the first `WeaponTypes` entry whose
//! `WeaponType` is the context weapon's type, else `ValueIfNotMatched`, with the context as the source.
//!
//! **The weapon's type** is read as `DefinitionData.WeaponTypeDefinition`'s `WeaponType` byte (`EWeaponType`), the
//! type definition loaded first. How the native reaches the type is not read; the field path is the item tools'.
//! A context with no `DefinitionData` is `NotYet { NotRead }`; a weapon with no type definition matches no entry.
//! Status: UNVERIFIED.

use ow_object::heap::ObjectSpace;
use ow_object::ids::ObjRef;
use ow_object::value::Value;

use super::manufacturer::{definition_data_object, select};
use crate::api::{AttrError, EvalCx, ValueResolver};
use crate::eval::{byte, field, required};
use crate::registry::AttributeSystem;

pub(crate) struct WeaponType;

impl ValueResolver for WeaponType {
    fn value(
        &self,
        sys: &AttributeSystem,
        resolver: ObjRef,
        context: ObjRef,
        _previous: f32,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        let entries = required(cx, resolver, "WeaponTypes")?;
        let definition = definition_data_object(
            cx,
            "WeaponTypeAttributeValueResolver",
            context,
            "WeaponTypeDefinition",
        )?;
        let weapon_type = match definition {
            Some(d) => Some(byte("WeaponType", &required(cx, d, "WeaponType")?)?),
            None => None,
        };
        let matches = |space: &ObjectSpace, entry: &Value| -> Result<bool, AttrError> {
            Ok(Some(byte("WeaponType", field(space, entry, "WeaponType")?)?) == weapon_type)
        };
        select(sys, resolver, context, &entries, matches, cx)
    }
}
