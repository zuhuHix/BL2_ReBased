//! Owner: lane W3-ATTREFFECTS. Not started. The stubs below were placed by W2-ATTRAPI: each is the body of one
//! [`AttributeSystem`] method (`registry.rs` delegates here in one line), returns `NotYet { NotPorted }` and keeps its
//! signature; the owner replaces the bodies (architecture section 3.5). Modifiers through a definition's value
//! chain, set-base, and `AttributeEffect.ApplyAttributeEffects` / `RemoveAttributeEffects` (NATIVE_ATTRIBUTES.md
//! sections 1 and 7).

use ow_object::ids::ObjRef;

use crate::api::not_ported;
use crate::api::{
    AddModifierOutcome, Applied, AttrError, EffectData, EvalCx, RemoveModifierOutcome,
};
use crate::registry::AttributeSystem;

/// [`AttributeSystem::add_modifier`].
pub(crate) fn add_modifier(
    sys: &AttributeSystem,
    attr: ObjRef,
    context: Option<ObjRef>,
    modifier: ObjRef,
    suppress_notify: bool,
    cx: &mut EvalCx<'_>,
) -> Result<AddModifierOutcome, AttrError> {
    let _ = (sys, attr, context, modifier, suppress_notify, cx);
    not_ported("AttributeSystem::add_modifier")
}

/// [`AttributeSystem::remove_modifier`].
pub(crate) fn remove_modifier(
    sys: &AttributeSystem,
    attr: ObjRef,
    context: Option<ObjRef>,
    modifier: ObjRef,
    suppress_notify: bool,
    cx: &mut EvalCx<'_>,
) -> Result<RemoveModifierOutcome, AttrError> {
    let _ = (sys, attr, context, modifier, suppress_notify, cx);
    not_ported("AttributeSystem::remove_modifier")
}

/// [`AttributeSystem::set_base`].
pub(crate) fn set_base(
    sys: &AttributeSystem,
    attr: ObjRef,
    source: Option<ObjRef>,
    value: f32,
    suppress_notify: bool,
    cx: &mut EvalCx<'_>,
) -> Result<bool, AttrError> {
    let _ = (sys, attr, source, value, suppress_notify, cx);
    not_ported("AttributeSystem::set_base")
}

/// [`AttributeSystem::apply_effects`].
pub(crate) fn apply_effects(
    sys: &AttributeSystem,
    effects: &[EffectData],
    source: Option<ObjRef>,
    override_source: Option<ObjRef>,
    cx: &mut EvalCx<'_>,
) -> Result<Applied, AttrError> {
    let _ = (sys, effects, source, override_source, cx);
    not_ported("AttributeSystem::apply_effects")
}

/// [`AttributeSystem::remove_effects`].
pub(crate) fn remove_effects(
    sys: &AttributeSystem,
    applied: Applied,
    cx: &mut EvalCx<'_>,
) -> Result<(), AttrError> {
    let _ = (sys, applied, cx);
    not_ported("AttributeSystem::remove_effects")
}
