//! `GearboxFramework.ConstraintAttributeValueResolver`: the running value of the chain clamped to a range (lane
//! FIX-ATTR-RESOLVERS, decision 2026-11-16-73). The class declares one field, `Constraints` (a `Range`: the
//! enabled-minimum flag and `MinValue`, the enabled-maximum flag and `MaxValue`), and no script function: its get is a
//! native the records had not read (NATIVE_ATTR_RESOLVERS.md now reads it). The emergency ammo pool reaches it in the
//! death roll. **Nothing here is a rule of the game**, and nothing is verified in the running game.
//!
//! **Parity modes** (architecture R3):
//!
//! - `ParityMode::Reference`, the C++ stand-in (`src/progression.cpp`, `ConstraintAttributeValueResolver`): the
//!   running value (the chain's previous result) raised to `MinValue` when the minimum is enabled, then lowered to
//!   `MaxValue` when the maximum is enabled, each bound evaluated on the context only when enabled.
//! - `ParityMode::Off`: `NotYet { NotRead }`. The analyst lane read the native (NATIVE_ATTR_RESOLVERS.md section 5)
//!   and leaves the Off rule to a fresh implementer (docs/LANE_BRIEF.md, "Clean room").
//!
//! **Writes** (add, remove, set-base): `NotYet` in both modes.

use ow_core::{NotYet, ParityMode};
use ow_object::ids::ObjRef;
use ow_object::value::Value;

use crate::api::{AttrError, EvalCx, Storage, ValueResolver};
use crate::eval::{evaluate_init, field, flag, init_data, parity, required};
use crate::registry::AttributeSystem;

pub(crate) const RECORD: &str = "NATIVE_ATTR_RESOLVERS.md (ConstraintAttributeValueResolver)";
const CPP: &str = "src/progression.cpp (ConstraintAttributeValueResolver)";

pub(crate) struct Constraint;

/// A bound of the range, evaluated on the context when its flag is set.
fn bound(
    sys: &AttributeSystem,
    cx: &mut EvalCx<'_>,
    range: &Value,
    on: &str,
    name: &str,
    context: ObjRef,
) -> Result<Option<f32>, AttrError> {
    let space = cx.world.parts().space;
    if !flag(on, field(space, range, on)?)? {
        return Ok(None);
    }
    let data = init_data(space, field(space, range, name)?)?;
    evaluate_init(sys, &data, Some(context), None, cx).map(Some)
}

fn read(
    sys: &AttributeSystem,
    cx: &mut EvalCx<'_>,
    resolver: ObjRef,
    context: ObjRef,
    previous: f32,
) -> Result<f32, AttrError> {
    if parity(cx) != ParityMode::Reference {
        return Err(NotYet::not_read(
            "ConstraintAttributeValueResolver in Off mode: the analyst note's rule awaits a fresh implementer",
            RECORD,
        )
        .into());
    }
    let range = required(cx, resolver, "Constraints")?;
    let mut value = previous;
    if let Some(min) = bound(sys, cx, &range, "bEnableMinValueRestriction", "MinValue", context)? {
        value = value.max(min);
    }
    if let Some(max) = bound(sys, cx, &range, "bEnableMaxValueRestriction", "MaxValue", context)? {
        value = value.min(max);
    }
    Ok(value)
}

impl ValueResolver for Constraint {
    fn value(
        &self,
        sys: &AttributeSystem,
        resolver: ObjRef,
        context: ObjRef,
        previous: f32,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        read(sys, cx, resolver, context, previous)
    }

    fn storage(
        &self,
        _sys: &AttributeSystem,
        _resolver: ObjRef,
        _context: ObjRef,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<Storage>, AttrError> {
        Err(match parity(cx) {
            ParityMode::Reference => NotYet::not_ported(
                "ConstraintAttributeValueResolver add, remove or set-base: the C++ has no write for this class",
                CPP,
            ),
            _ => NotYet::not_read("ConstraintAttributeValueResolver add, remove or set-base", RECORD),
        }
        .into())
    }
}
