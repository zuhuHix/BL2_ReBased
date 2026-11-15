//! Owner: lane W3-DEFS. Typed property keys: `DefStore::key` turns a class path and a property name into the
//! [`PropId`] of the declaring class's layout, once, and caches it.
//!
//! A key is the property's first slot in the class that declares it, valid in that class and every subclass
//! (decision D14), so a key asked for on a subclass is the one its declaring class gives. Names match as FNames do
//! (ASCII case folded). A static array's elements are addressed with `PropAddr { prop: key, index }`.

use ow_core::DetMap;
use ow_object::ids::PropId;

/// The cache of typed keys: (folded class path, folded property name) -> key.
#[derive(Debug, Default)]

pub(crate) struct KeyCache {
    pub(crate) keys: DetMap<(String, String), PropId>,
}
