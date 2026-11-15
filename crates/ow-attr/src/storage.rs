//! Owner: lane W3-ATTREFFECTS. Not started. The stubs below were placed by W2-ATTRAPI: each is the body of one public
//! entry point (`registry.rs` delegates here in one line), returns `NotYet { NotPorted }` and keeps its signature; the
//! owner replaces the bodies (architecture section 3.5). The property-level rules (NATIVE_ATTRIBUTES.md section 3):
//! what add, remove and set-base do to a stack or plain attribute's stored numbers, the by-name `Object` natives, and
//! the script VM's typed let.

use ow_object::heap::ObjectSpace;
use ow_object::ids::{NameId, ObjRef};
use ow_object::layout::AttrCompanions;
use ow_object::value::Value;

use crate::api::not_ported;
use crate::api::{AddModifierOutcome, AttrError, EvalCx, RemoveModifierOutcome};
use crate::registry::AttributeSystem;
use crate::stack::CombineForm;

/// [`AttributeSystem::add_modifier_by_name`].
pub(crate) fn add_modifier_by_name(
    sys: &AttributeSystem,
    object: ObjRef,
    modifier: ObjRef,
    attribute_name: NameId,
    suppress_notify: bool,
    cx: &mut EvalCx<'_>,
) -> Result<AddModifierOutcome, AttrError> {
    let _ = (sys, object, modifier, attribute_name, suppress_notify, cx);
    not_ported("AttributeSystem::add_modifier_by_name")
}

/// [`AttributeSystem::remove_modifier_by_name`].
pub(crate) fn remove_modifier_by_name(
    sys: &AttributeSystem,
    object: ObjRef,
    modifier: ObjRef,
    attribute_name: NameId,
    suppress_notify: bool,
    cx: &mut EvalCx<'_>,
) -> Result<RemoveModifierOutcome, AttrError> {
    let _ = (sys, object, modifier, attribute_name, suppress_notify, cx);
    not_ported("AttributeSystem::remove_modifier_by_name")
}

/// [`AttributeSystem::attribute_value_by_name`].
pub(crate) fn attribute_value_by_name(
    sys: &AttributeSystem,
    object: ObjRef,
    attribute_name: NameId,
    cx: &mut EvalCx<'_>,
) -> Result<f32, AttrError> {
    let _ = (sys, object, attribute_name, cx);
    not_ported("AttributeSystem::attribute_value_by_name")
}

/// [`crate::registry::let_base`]: refuses an `rhs` whose kind differs from the base companion's
/// (`AttrError::BadData`), converts nothing, keeps an integer exact.
pub(crate) fn let_base(
    space: &mut ObjectSpace,
    owner: ObjRef,
    companions: AttrCompanions,
    rhs: Value,
    form: CombineForm,
) -> Result<(), AttrError> {
    let _ = (space, owner, companions, rhs, form);
    not_ported("ow_attr::registry::let_base")
}
