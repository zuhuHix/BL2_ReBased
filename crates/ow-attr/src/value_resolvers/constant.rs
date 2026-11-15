//! `GearboxFramework.ConstantAttributeValueResolver`: its `ConstantValue`, ignoring the previous value
//! (NATIVE_ATTRIBUTES.md section 6, high confidence). Status: UNVERIFIED.

use ow_object::ids::ObjRef;

use crate::api::{AttrError, EvalCx, ValueResolver};
use crate::eval::{number, required};
use crate::registry::AttributeSystem;

pub(crate) struct Constant;

impl ValueResolver for Constant {
    fn value(
        &self,
        _sys: &AttributeSystem,
        resolver: ObjRef,
        _context: ObjRef,
        _previous: f32,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        number("ConstantValue", &required(cx, resolver, "ConstantValue")?)
    }
}
