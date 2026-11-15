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
    /// The tail's bytes in `data.data` (absolute in the package's decoded bytes); possibly empty. Checked by
    /// [`Tail::new`].
    pub range: Range<usize>,
}

impl Tail {
    /// A view of `range` of `data`'s bytes; `None` when the range is reversed or reaches past the data, so a `Tail`
    /// always names bytes that exist.
    pub fn new(package: PackageId, data: Arc<Package>, range: Range<usize>) -> Option<Self> {
        (range.start <= range.end && range.end <= data.data.len()).then_some(Tail {
            package,
            data,
            range,
        })
    }

    /// The bytes: empty only when the tail is, for every `Tail` the store or [`Tail::new`] made. (The fields are public;
    /// a range a caller moved past the data afterwards gives an empty slice rather than a panic.)
    pub fn bytes(&self) -> &[u8] {
        self.data.data.get(self.range.clone()).unwrap_or(&[])
    }
}

/// Where each loaded definition's tail lies: the object -> (filed package id, absolute byte range).
#[derive(Debug, Default)]

pub(crate) struct Tails {
    pub(crate) spans: FixedHashMap<ObjRef, (PackageId, Range<usize>)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use ow_package::testkit::PackageBuilder;

    #[test]
    fn a_tail_names_only_bytes_that_exist() {
        let bytes = PackageBuilder::new_empty().build();
        let package = Arc::new(Package::from_bytes("T.upk".into(), "T".into(), &bytes).unwrap());
        let len = package.data.len();
        let tail = Tail::new(0, Arc::clone(&package), len - 3..len).unwrap();
        assert_eq!(tail.bytes(), &package.data[len - 3..]);
        assert!(
            Tail::new(0, Arc::clone(&package), len..len)
                .unwrap()
                .bytes()
                .is_empty()
        );
        assert!(Tail::new(0, Arc::clone(&package), len - 1..len + 1).is_none());
        let reversed = Range { start: 2, end: 1 };
        assert!(Tail::new(0, package, reversed).is_none());
    }
}
