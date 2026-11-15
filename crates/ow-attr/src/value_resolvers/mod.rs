//! Owner: lane W3-ATTREVAL. Not started. The value resolver classes the notes read (NATIVE_ATTRIBUTES.md section 6),
//! one module file each. `register` was placed by W2-ATTRAPI: [`AttributeSystem::standard`] calls it, and the owner
//! adds a `with_value_resolver` call per class here (an unregistered class stays `NotYet { NotRead }`).

use crate::api::AttrError;
use crate::registry::AttributeSystem;

/// Registers every value resolver class this module implements. None yet.
pub(crate) fn register(sys: AttributeSystem) -> Result<AttributeSystem, AttrError> {
    Ok(sys)
}
