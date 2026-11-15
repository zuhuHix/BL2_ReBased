//! Owner: lane W3-ATTREVAL. `AttributeExpression` evaluation: the body of [`AttributeSystem::expressions_hold`]
//! (`registry.rs` delegates here in one line; architecture section 3.5), and the reader of
//! `AttributeExpressionData` arrays the conditional forms share.
//!
//! The rules are NATIVE_ATTRIBUTES.md section 11 (read from native code, high confidence) and **UNVERIFIED in the
//! running game**.

use ow_object::heap::ObjectSpace;
use ow_object::ids::ObjRef;

use crate::api::{AttrError, Comparison, EvalCx, ExprData, ExprMode, Operand2Usage};
use crate::eval::{self, field};
use crate::registry::AttributeSystem;

/// [`AttributeSystem::expressions_hold`]: `EvaluateExpressions(Mode, ...)`. And: true when every expression holds,
/// stopping at the first false; Or: true when one holds, stopping at the first true. An empty list holds in both
/// modes. Status: UNVERIFIED (NATIVE_ATTRIBUTES.md section 11).
pub(crate) fn expressions_hold(
    sys: &AttributeSystem,
    exprs: &[ExprData],
    mode: ExprMode,
    source: Option<ObjRef>,
    override_source: Option<ObjRef>,
    cx: &mut EvalCx<'_>,
) -> Result<bool, AttrError> {
    if exprs.is_empty() {
        return Ok(true);
    }
    for e in exprs {
        let holds = expression_holds(sys, e, source, override_source, cx)?;
        match mode {
            ExprMode::And if !holds => return Ok(false),
            ExprMode::Or if holds => return Ok(true),
            _ => {}
        }
    }
    Ok(matches!(mode, ExprMode::And))
}

/// `EvaluateExpression`: the left value is `AttributeOperand1`'s value (`GetValue` with the override tried first); a
/// None attribute or a context that does not resolve makes the expression false. The right value is
/// `ConstantOperand2` without `AttributeOperand2`; otherwise that attribute's value, times the constant when
/// `Operand2Usage` is `Multiply`, and false when it does not resolve. The comparison is in single precision, exact
/// equality for `EqualTo` and `NotEqualTo`. The right operand is not evaluated when the left one already made the
/// expression false. Status: UNVERIFIED (NATIVE_ATTRIBUTES.md section 11).
pub(crate) fn expression_holds(
    sys: &AttributeSystem,
    e: &ExprData,
    source: Option<ObjRef>,
    override_source: Option<ObjRef>,
    cx: &mut EvalCx<'_>,
) -> Result<bool, AttrError> {
    let Some(first) = e.attribute_operand1 else {
        return Ok(false);
    };
    let Some(left) = eval::value_of(sys, first, source, override_source, cx)? else {
        return Ok(false);
    };
    let right = match e.attribute_operand2 {
        None => e.constant_operand2,
        Some(second) => match eval::value_of(sys, second, source, override_source, cx)? {
            None => return Ok(false),
            Some(v) => match e.operand2_usage {
                Operand2Usage::PreferAttribute => v.value,
                Operand2Usage::Multiply => e.constant_operand2 * v.value,
            },
        },
    };
    let l = left.value;
    Ok(match e.comparison_operator {
        Comparison::EqualTo => l == right,
        Comparison::NotEqualTo => l != right,
        Comparison::LessThan => l < right,
        Comparison::LessThanOrEqual => l <= right,
        Comparison::GreaterThan => l > right,
        Comparison::GreaterThanOrEqual => l >= right,
    })
}

/// An array of `AttributeExpressionData` struct values as [`ExprData`]. An enum byte outside its enum is bad data.
pub(crate) fn read_expressions(
    space: &ObjectSpace,
    v: &ow_object::value::Value,
) -> Result<Vec<ExprData>, AttrError> {
    let bad = |what: &str, raw: u8| AttrError::BadData {
        reason: format!("{what} {raw} is outside the enum").into(),
    };
    let mut out = Vec::new();
    for e in eval::elements("Expressions", v)? {
        let op = eval::byte("ComparisonOperator", field(space, e, "ComparisonOperator")?)?;
        let usage = eval::byte("Operand2Usage", field(space, e, "Operand2Usage")?)?;
        out.push(ExprData {
            attribute_operand1: eval::object(
                "AttributeOperand1",
                field(space, e, "AttributeOperand1")?,
            )?,
            comparison_operator: Comparison::from_raw(i64::from(op))
                .ok_or_else(|| bad("ComparisonOperator", op))?,
            operand2_usage: Operand2Usage::from_raw(i64::from(usage))
                .ok_or_else(|| bad("Operand2Usage", usage))?,
            attribute_operand2: eval::object(
                "AttributeOperand2",
                field(space, e, "AttributeOperand2")?,
            )?,
            constant_operand2: eval::number(
                "ConstantOperand2",
                field(space, e, "ConstantOperand2")?,
            )?,
        });
    }
    Ok(out)
}
