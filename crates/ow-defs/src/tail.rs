//! Owner: lane W3-DEFS. Raw trailing bytes: what follows an export's tag stream in its payload, kept per loaded
//! definition for the readers that decode it themselves (behavior providers, `DefStore::raw_tail`).
//!
//! The tail starts where the stream ended (`ow_object::cdo::Instance::tags_end`, just past the `None` tag, counted
//! from the payload start) and runs to the end of the export's serial data. A [`Tail`] is an owned view: it holds the
//! package (`PackageStore::shared`) and the byte range, so it outlives any borrow of the store. Nothing is copied or
//! interpreted here.

use std::ops::Range;
use std::sync::Arc;

use ow_core::FixedHashMap;
use ow_object::ids::ObjRef;
use ow_package::PackageId;
use ow_package::package::Package;

/// The untagged bytes after a loaded definition's tags (module documentation).
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct Tail {
    /// The filed id of the package the definition was loaded from.
    pub package: PackageId,
    /// That package.
    pub data: Arc<Package>,
    /// The tail's bytes in `data.data` (absolute in the package's decoded bytes); possibly empty.
    pub range: Range<usize>,
}

impl Tail {
    pub fn new(package: PackageId, data: Arc<Package>, range: Range<usize>) -> Self {
        Tail {
            package,
            data,
            range,
        }
    }

    /// The bytes. A range outside the package's data (a caller-built `Tail`) gives an empty slice.
    pub fn bytes(&self) -> &[u8] {
        self.data.data.get(self.range.clone()).unwrap_or(&[])
    }
}

/// Where each loaded definition's tail lies: the object -> (filed package id, absolute byte range).
#[derive(Debug, Default)]
#[allow(dead_code)] // Contract commit: the body (W3-DEFS) fills and reads it.
pub(crate) struct Tails {
    pub(crate) spans: FixedHashMap<ObjRef, (PackageId, Range<usize>)>,
}
