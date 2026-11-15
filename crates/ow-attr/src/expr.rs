//! Owner: lane W3-ATTREVAL. Not started. The stub below was placed by W2-ATTRAPI: it is the body of
//! [`AttributeSystem::expressions_hold`] (`registry.rs` delegates here in one line), returns `NotYet { NotPorted }`
//! and keeps its signature; the owner replaces the body (architecture section 3.5). `AttributeExpression`
//! evaluation (NATIVE_ATTRIBUTES.md section 11).

use ow_object::ids::ObjRef;

use crate::api::{AttrError, EvalCx, ExprData, ExprMode};
use crate::eval::not_ported;
use crate::registry::AttributeSystem;

/// [`AttributeSystem::expressions_hold`].
pub(crate) fn expressions_hold(
    sys: &AttributeSystem,
    exprs: &[ExprData],
    mode: ExprMode,
    source: Option<ObjRef>,
    override_source: Option<ObjRef>,
    cx: &mut EvalCx<'_>,
) -> Result<bool, AttrError> {
    let _ = (sys, exprs, mode, source, override_source, cx);
    not_ported("AttributeSystem::expressions_hold")
}
