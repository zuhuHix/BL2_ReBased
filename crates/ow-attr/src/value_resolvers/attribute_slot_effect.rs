//! `WillowGame.AttributeSlotEffectAttributeValueResolver` (NATIVE_ATTRIBUTES.md sections 6 and 9, high confidence
//! for the shape): the context item's slot named `SlotName`, its grade (`SlotProperty` 0,
//! `ATTRSLOTEFFECTPROP_Grade`) or its `ComputedModifierValue` (1), through the item's `IAttributeSlotEffectProvider`
//! interface; 0 without the interface.
//!
//! The interface bodies are not resolved (section 9): which slot answers for a name, and how
//! `ComputedModifierValue` is computed (real-game question 5), are unread. The resolver checks its own fields and
//! answers `NotYet { NotRead }`; it does not pick a slot by a guessed rule.

use ow_core::NotYet;
use ow_object::ids::ObjRef;

use crate::api::{AttrError, EvalCx, ValueResolver};
use crate::eval::{byte, name, required};
use crate::registry::AttributeSystem;

pub(crate) struct AttributeSlotEffect;

impl ValueResolver for AttributeSlotEffect {
    fn value(
        &self,
        _sys: &AttributeSystem,
        resolver: ObjRef,
        _context: ObjRef,
        _previous: f32,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        name("SlotName", &required(cx, resolver, "SlotName")?)?;
        let raw = byte("SlotProperty", &required(cx, resolver, "SlotProperty")?)?;
        let what = match raw {
            0 => "grade",
            1 => "ComputedModifierValue",
            other => {
                return Err(AttrError::BadData {
                    reason: format!(
                        "AttributeSlotEffectAttributeValueResolver SlotProperty {other} is outside the enum"
                    )
                    .into(),
                });
            }
        };
        Err(NotYet::not_read(
            format!(
                "AttributeSlotEffectAttributeValueResolver: the slot {what} the item's IAttributeSlotEffectProvider answers"
            ),
            "NATIVE_ATTRIBUTES.md section 9",
        )
        .into())
    }
}
