//! Owner: lane W3-DEFS. Raw trailing bytes: what follows an export's tag stream in its payload, kept per loaded
//! definition for the readers that decode it themselves (behavior providers, `DefStore::raw_tail`).
//!
//! The tail starts where the stream ended (`ow_object::cdo::Instance::tags_end`, just past the `None` tag, counted
//! from the payload start) and runs to the end of the export's serial data. It is a view of the package's decoded
//! bytes: nothing is copied or interpreted here.

use ow_core::FixedHashMap;
use ow_object::ids::ObjRef;
use ow_package::PackageId;

/// Where each loaded definition's tail lies: the object -> (package, first byte, end), absolute in the package's
/// decoded data.
#[derive(Debug, Default)]
#[allow(dead_code)] // Contract commit: the body (W3-DEFS) fills and reads it.
pub(crate) struct Tails {
    pub(crate) spans: FixedHashMap<ObjRef, (PackageId, usize, usize)>,
}
