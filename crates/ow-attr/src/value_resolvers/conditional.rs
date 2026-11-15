//! `GearboxFramework.ConditionalAttributeValueResolver`: the conditional-initialization result of its
//! `ValueExpressions` (the first entry whose expressions all hold, else the default), evaluated on the context and
//! ignoring the previous value (NATIVE_ATTRIBUTES.md section 6, medium confidence).
//!
//! The struct's `bEnabled` is **not** consulted, as `progression.cpp` reads it: the flag is false in the shipped
//! data that has entries (the gun drop odds by player count), whose editor display shares use the entries. That
//! reading is checked by `tools/loot_pools.py` against those shares, not read from the native. Status: UNVERIFIED.

use ow_object::ids::ObjRef;

use crate::api::{AttrError, EvalCx, ValueResolver};
use crate::eval::{conditional_value, required};
use crate::registry::AttributeSystem;

pub(crate) struct Conditional;

impl ValueResolver for Conditional {
    fn value(
        &self,
        sys: &AttributeSystem,
        resolver: ObjRef,
        context: ObjRef,
        _previous: f32,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        let expressions = required(cx, resolver, "ValueExpressions")?;
        conditional_value(sys, &expressions, Some(context), None, cx)
    }
}
