//! Owner: lane W3-ATTREVAL. Not started. The stubs below were placed by W2-ATTRAPI: each is the body of one
//! [`AttributeSystem`] method (`registry.rs` delegates here in one line), returns `NotYet { NotPorted }` and keeps its
//! signature; the owner replaces the bodies (architecture section 3.5). Chain walks and the
//! `AttributeInitializationData` evaluator (NATIVE_ATTRIBUTES.md section 1, NATIVE_PROGRESSION.md section 1).

use ow_object::ids::ObjRef;

use crate::api::{AttrError, AttrValue, EvalCx, InitData, not_ported};
use crate::registry::AttributeSystem;

/// [`AttributeSystem::resolve_context`].
pub(crate) fn resolve_context(
    sys: &AttributeSystem,
    attr: ObjRef,
    source: Option<ObjRef>,
    cx: &mut EvalCx<'_>,
) -> Result<Option<ObjRef>, AttrError> {
    let _ = (sys, attr, source, cx);
    not_ported("AttributeSystem::resolve_context")
}

/// [`AttributeSystem::resolve_contexts`].
pub(crate) fn resolve_contexts(
    sys: &AttributeSystem,
    attr: ObjRef,
    source: Option<ObjRef>,
    cx: &mut EvalCx<'_>,
) -> Result<Vec<ObjRef>, AttrError> {
    let _ = (sys, attr, source, cx);
    not_ported("AttributeSystem::resolve_contexts")
}

/// [`AttributeSystem::value_of`].
pub(crate) fn value_of(
    sys: &AttributeSystem,
    attr: ObjRef,
    source: Option<ObjRef>,
    override_source: Option<ObjRef>,
    cx: &mut EvalCx<'_>,
) -> Result<Option<AttrValue>, AttrError> {
    let _ = (sys, attr, source, override_source, cx);
    not_ported("AttributeSystem::value_of")
}

/// [`AttributeSystem::base_value_of`].
pub(crate) fn base_value_of(
    sys: &AttributeSystem,
    attr: ObjRef,
    source: Option<ObjRef>,
    override_source: Option<ObjRef>,
    cx: &mut EvalCx<'_>,
) -> Result<Option<AttrValue>, AttrError> {
    let _ = (sys, attr, source, override_source, cx);
    not_ported("AttributeSystem::base_value_of")
}

/// [`AttributeSystem::value_from_context`].
pub(crate) fn value_from_context(
    sys: &AttributeSystem,
    attr: ObjRef,
    context: Option<ObjRef>,
    cx: &mut EvalCx<'_>,
) -> Result<f32, AttrError> {
    let _ = (sys, attr, context, cx);
    not_ported("AttributeSystem::value_from_context")
}

/// [`AttributeSystem::base_value_from_context`].
pub(crate) fn base_value_from_context(
    sys: &AttributeSystem,
    attr: ObjRef,
    context: Option<ObjRef>,
    cx: &mut EvalCx<'_>,
) -> Result<f32, AttrError> {
    let _ = (sys, attr, context, cx);
    not_ported("AttributeSystem::base_value_from_context")
}

/// [`AttributeSystem::evaluate_init`].
pub(crate) fn evaluate_init(
    sys: &AttributeSystem,
    data: &InitData,
    source: Option<ObjRef>,
    override_source: Option<ObjRef>,
    cx: &mut EvalCx<'_>,
) -> Result<f32, AttrError> {
    let _ = (sys, data, source, override_source, cx);
    not_ported("AttributeSystem::evaluate_init")
}
