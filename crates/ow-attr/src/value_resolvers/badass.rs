//! `WillowGame.BadassAttributeValueResolver` (NATIVE_ATTRIBUTES.md section 6, low confidence): reads the player
//! profile's badass reward rank or token state for `AssociatedBadassReward`; its set-base writes the rank, rounded
//! half up.
//!
//! The profile is the save layer's (`ow-save`), which this crate cannot reach, and the read is not detailed in the
//! record: the value answers `NotYet { LayerAbsent }` after checking the resolver's own field. The set-base has no
//! [`crate::api::Storage`] form (a contract item for W3-ATTREFFECTS, noted in the handoff), so `storage` keeps the
//! class default `None`.

use ow_core::NotYet;
use ow_object::ids::ObjRef;

use super::RECORD;
use crate::api::{AttrError, EvalCx, ValueResolver};
use crate::eval::{object, required};
use crate::registry::AttributeSystem;

pub(crate) struct Badass;

impl ValueResolver for Badass {
    fn value(
        &self,
        _sys: &AttributeSystem,
        resolver: ObjRef,
        _context: ObjRef,
        _previous: f32,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        object(
            "AssociatedBadassReward",
            &required(cx, resolver, "AssociatedBadassReward")?,
        )?;
        Err(NotYet::layer_absent(
            "BadassAttributeValueResolver: the player profile's badass rank",
            "ow-save (the player profile)",
            RECORD,
        )
        .into())
    }
}
