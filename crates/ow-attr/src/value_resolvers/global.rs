//! `WillowGame.GlobalAttributeValueResolver`: the world's global slot `GlobalAttribute` (0
//! `GATTR_ExperiencePointTestLevel`, 1 `GATTR_BadassTokenTestRank`; any other index reads 0), ignoring the previous
//! value (NATIVE_ATTRIBUTES.md section 6, high confidence). The slots are the world's (`AttrWorld::global_slots`),
//! which the XP curve sets before it evaluates. Status: UNVERIFIED.

use ow_object::ids::ObjRef;

use crate::api::{AttrError, EvalCx, ValueResolver};
use crate::eval::{byte, required};
use crate::registry::AttributeSystem;

pub(crate) struct Global;

impl ValueResolver for Global {
    fn value(
        &self,
        _sys: &AttributeSystem,
        resolver: ObjRef,
        _context: ObjRef,
        _previous: f32,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        let slot = byte(
            "GlobalAttribute",
            &required(cx, resolver, "GlobalAttribute")?,
        )?;
        Ok(cx.world.global_slots().get(slot))
    }
}
