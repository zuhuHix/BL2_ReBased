//! `GearboxFramework.SimpleMathValueResolver` (NATIVE_ATTRIBUTES.md section 6, high confidence): the first operand
//! is the previous value (`Arg1Option` 0, `MATHRESOLVEARG1_UsePreviousValue`) or the `Arg1Attribute` data (1,
//! `MATHRESOLVEARG1_FromAttribute`); it is combined with the `Argument` data by `Operand`: 0 Add `arg + a1`, 1 Sub
//! `a1 - arg`, 2 Mul `arg * a1`, 3 Div `a1 / arg`, 0 when `|arg|` is below 1e-8. Both data are evaluated with the
//! context as their source (a resolver sees only the context), `Arg1Attribute` first. The enum values are the
//! package enums' positions (`EMathValueResolverOperand`, `EMathValueResolverArg1Option`).
//!
//! Differences from `progression.cpp`, which knows no `Arg1Option` and divides only by an exact 0: this follows the
//! record (the 1e-8 threshold, compared in single precision). The order of the two data matters only when both draw
//! random numbers; it is not read. Status: UNVERIFIED.

use ow_object::ids::ObjRef;

use crate::api::{AttrError, EvalCx, ValueResolver};
use crate::eval::{byte, evaluate_value, required};
use crate::registry::AttributeSystem;

pub(crate) struct SimpleMath;

/// `|arg|` below this divides to 0 (the record).
const DIVIDE_EPSILON: f32 = 1e-8;

impl ValueResolver for SimpleMath {
    fn value(
        &self,
        sys: &AttributeSystem,
        resolver: ObjRef,
        context: ObjRef,
        previous: f32,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        let option = byte("Arg1Option", &required(cx, resolver, "Arg1Option")?)?;
        let operand = byte("Operand", &required(cx, resolver, "Operand")?)?;
        if operand > 3 {
            return Err(outside("Operand", operand));
        }
        let first = match option {
            0 => previous,
            1 => {
                let data = required(cx, resolver, "Arg1Attribute")?;
                evaluate_value(sys, &data, Some(context), None, cx)?
            }
            other => return Err(outside("Arg1Option", other)),
        };
        let data = required(cx, resolver, "Argument")?;
        let arg = evaluate_value(sys, &data, Some(context), None, cx)?;
        Ok(match operand {
            0 => arg + first,
            1 => first - arg,
            2 => arg * first,
            _ if arg.abs() < DIVIDE_EPSILON => 0.0,
            _ => first / arg,
        })
    }
}

fn outside(what: &str, raw: u8) -> AttrError {
    AttrError::BadData {
        reason: format!("SimpleMathValueResolver {what} {raw} is outside the enum").into(),
    }
}
