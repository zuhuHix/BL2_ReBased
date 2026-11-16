//! Owner: lane FIX-DEFS-COLLECTION. A collection actor's components and the untagged entries its tail holds for them
//! ([`DefStore::collection`]).
//!
//! A `StaticMeshCollectionActor` lists its components in its `StaticMeshComponents` array and a
//! `StaticLightCollectionActor` in its `LightComponents` array. After the actor's tag stream, its tail holds one
//! fixed-size entry per array element, in array order. The sizes and the mesh entry's layout are the records', not
//! read from the game's code:
//! - a mesh entry is 84 bytes: 16 floats of an affine rotation and translation matrix as stored, 3 floats of
//!   Scale3D, one float of scale, and one int whose meaning is not assigned (decision 2026-09-10-09,
//!   `tools/prepare_level.py` `collection_transforms`). Every float must be finite and the matrix affine (entries 3, 7
//!   and 11 within 1e-5 of 0, entry 15 within 1e-5 of 1), the tool's checks;
//! - a light entry is 64 bytes and is not decoded: its layout was never recovered (presumed a 4x4 transform,
//!   UNVERIFIED, `docs/verification/SANCTUARY_SCENE_IMPORT_GAPS.md`, "HQ static lights"), so it is handed back raw.
//!
//! The tail must be exactly entries times the entry size, or the actor is a [`DefError::Collection`]: no offset
//! search, and no partial result. A null array element keeps its entry (its bytes checked like the others), as the
//! tool zips the two arrays. A component listed twice, here or by another collection, is the caller's to judge (the
//! tool stops its build there).
//!
//! Status: the layouts are the tools' observations of the Ash and Sanctuary packages, UNVERIFIED in the running game.

use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::reflect::ReflectError;
use ow_object::value::Value;
use ow_package::store::StoreError;

use super::{DefError, DefStore};

/// Bytes per `StaticMeshCollectionActor` tail entry (decision 2026-09-10-09).
pub const MESH_ENTRY_BYTES: usize = 84;
/// Bytes per `StaticLightCollectionActor` tail entry (`SANCTUARY_SCENE_IMPORT_GAPS.md`).
pub const LIGHT_ENTRY_BYTES: usize = 64;
/// The affine check's tolerance (`tools/prepare_level.py` `collection_transforms`).
const AFFINE_TOLERANCE: f32 = 1e-5;

/// Which collection actor class an object is.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectionKind {
    /// `Engine.StaticMeshCollectionActor` (or a class deriving from it).
    StaticMesh,
    /// `Engine.StaticLightCollectionActor` (or a class deriving from it).
    StaticLight,
}

impl CollectionKind {
    /// The class path.
    pub fn class_path(self) -> &'static str {
        match self {
            CollectionKind::StaticMesh => "Engine.StaticMeshCollectionActor",
            CollectionKind::StaticLight => "Engine.StaticLightCollectionActor",
        }
    }

    /// The array property listing the components.
    pub fn array(self) -> &'static str {
        match self {
            CollectionKind::StaticMesh => "StaticMeshComponents",
            CollectionKind::StaticLight => "LightComponents",
        }
    }

    /// Bytes per tail entry.
    pub fn entry_bytes(self) -> usize {
        match self {
            CollectionKind::StaticMesh => MESH_ENTRY_BYTES,
            CollectionKind::StaticLight => LIGHT_ENTRY_BYTES,
        }
    }
}

/// A tail entry (module documentation).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum CollectionPose {
    /// A mesh entry, as stored.
    Mesh {
        /// The rotation and translation matrix, 16 floats in stored order.
        matrix: [f32; 16],
        scale3d: [f32; 3],
        scale: f32,
        /// The last int; its meaning is not assigned.
        unassigned: i32,
    },
    /// A light entry's bytes, not decoded (layout not recovered).
    Light { raw: [u8; LIGHT_ENTRY_BYTES] },
}

/// One element of a collection's component array with its tail entry.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct CollectionEntry {
    /// The store's object for the component (an unloaded handle until loaded); `None` for a null element.
    pub component: Option<ObjRef>,
    pub pose: CollectionPose,
}

/// A loaded collection actor's components, in array order.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Collection {
    pub kind: CollectionKind,
    pub entries: Vec<CollectionEntry>,
}

impl DefStore {
    /// The components of collection actor `obj` with their tail entries (module documentation), `obj` loaded first.
    /// An object of another class is [`DefError::NotADefinition`]; a component array or a tail outside the recorded
    /// layout is [`DefError::Collection`].
    pub fn collection(&mut self, w: &mut ObjectSpace, obj: ObjRef) -> Result<Collection, DefError> {
        self.load(w, obj)?;
        let class = w.heap.get(obj)?.class;
        for kind in [CollectionKind::StaticMesh, CollectionKind::StaticLight] {
            let base = match self.class_id(w, kind.class_path()) {
                Ok(base) => base,
                // No installed package declares the class: nothing is of it.
                Err(DefError::Reflect(ReflectError::ClassNotFound(_)))
                | Err(DefError::Store(StoreError::PackageNotFound(_))) => continue,
                Err(e) => return Err(e),
            };
            if w.classes.is_child_of(class, base) {
                return self.read_collection(w, obj, kind);
            }
        }
        Err(DefError::NotADefinition(format!(
            "{}: not a collection actor",
            self.object_text(w, obj)
        )))
    }

    fn read_collection(
        &mut self,
        w: &mut ObjectSpace,
        obj: ObjRef,
        kind: CollectionKind,
    ) -> Result<Collection, DefError> {
        let object = self.object_text(w, obj);
        let fail = |reason: String| DefError::Collection {
            object: object.clone(),
            reason,
        };
        let prop = self.key(w, kind.class_path(), kind.array())?;
        let components: Vec<Option<ObjRef>> = match w.read(obj, PropAddr::scalar(prop))? {
            Value::Array(items) => items
                .iter()
                .map(|v| match v {
                    Value::Object(o) => Ok(*o),
                    _ => Err(fail(format!("{} holds a non-object", kind.array()))),
                })
                .collect::<Result<_, _>>()?,
            _ => return Err(fail(format!("{} is not an array", kind.array()))),
        };
        let tail = self
            .raw_tail(obj)
            .ok_or_else(|| fail("no tail was kept for it".to_string()))?;
        let bytes = tail.bytes();
        let want = components
            .len()
            .checked_mul(kind.entry_bytes())
            .ok_or_else(|| fail("the tail size overflows".to_string()))?;
        if bytes.len() != want {
            return Err(fail(format!(
                "a tail of {} bytes for {} components of {} bytes",
                bytes.len(),
                components.len(),
                kind.entry_bytes()
            )));
        }
        let mut entries = Vec::with_capacity(components.len());
        for (component, chunk) in components
            .into_iter()
            .zip(bytes.chunks_exact(kind.entry_bytes()))
        {
            let pose = match kind {
                CollectionKind::StaticMesh => mesh_pose(chunk).map_err(fail)?,
                CollectionKind::StaticLight => {
                    let mut raw = [0u8; LIGHT_ENTRY_BYTES];
                    raw.copy_from_slice(chunk);
                    CollectionPose::Light { raw }
                }
            };
            entries.push(CollectionEntry { component, pose });
        }
        Ok(Collection { kind, entries })
    }

    /// The object's name for an error text.
    fn object_text(&self, w: &ObjectSpace, obj: ObjRef) -> String {
        match w.heap.get(obj) {
            Ok(o) => w.names.text(o.name).to_string(),
            Err(_) => format!("{obj:?}"),
        }
    }
}

/// One 84-byte mesh entry, checked as the tool checks it.
fn mesh_pose(chunk: &[u8]) -> Result<CollectionPose, String> {
    let word = |i: usize| -> [u8; 4] {
        let mut b = [0u8; 4];
        b.copy_from_slice(&chunk[i * 4..i * 4 + 4]);
        b
    };
    let mut row = [0f32; 20];
    for (i, v) in row.iter_mut().enumerate() {
        *v = f32::from_le_bytes(word(i));
        if !v.is_finite() {
            return Err(format!("a non-finite value at float {i} of an entry"));
        }
    }
    let affine = [3, 7, 11].iter().all(|&k| row[k].abs() <= AFFINE_TOLERANCE)
        && (row[15] - 1.0).abs() <= AFFINE_TOLERANCE;
    if !affine {
        return Err("a non-affine entry matrix".to_string());
    }
    let mut matrix = [0f32; 16];
    matrix.copy_from_slice(&row[..16]);
    Ok(CollectionPose::Mesh {
        matrix,
        scale3d: [row[16], row[17], row[18]],
        scale: row[19],
        unassigned: i32::from_le_bytes(word(20)),
    })
}

#[cfg(test)]
mod tests;
