//! Owner: lane W3-ATTRCTX. Not started. The context resolver classes the notes read (NATIVE_ATTRIBUTES.md section 5),
//! one module file each. `register` was placed by W2-ATTRAPI: [`AttributeSystem::standard`] calls it, and the owner
//! adds a `with_context_resolver` call per class here (an unregistered class stays `NotYet { NotRead }`).

use crate::api::AttrError;
use crate::registry::AttributeSystem;

/// Registers every context resolver class this module implements. None yet.
pub(crate) fn register(sys: AttributeSystem) -> Result<AttributeSystem, AttrError> {
    Ok(sys)
}
