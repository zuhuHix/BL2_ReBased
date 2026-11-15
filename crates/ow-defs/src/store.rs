//! Owner: lane W3-DEFS. `DefStore`: the definition store (architecture section 3.4). It owns the package store, the
//! reflection index and the decode state, and loads definitions lazily into a caller's [`ObjectSpace`], one heap
//! object per object path, with the overlay layers on top.
//!
//! **One object space.** The store keeps handles into the space it is given ([`ObjRef`]s, class ids, the decode
//! caches), so every call must pass the same [`ObjectSpace`]. A handle from another space is a caller bug the store
//! can only partly catch.
//!
//! **Identity** is by object path, chosen by the [`IdentityPolicy`] (`crate::identity`): each path is one object, the
//! export of the holder the policy picks, whichever way it is reached. Its key is that export as `(filed id, index)`,
//! the id [`CodeIndex::filed_id`] files the package under (decision 2026-11-15-10): a `PackageId` is not file identity
//! after `PackageStore::scan`, so no map of the store is keyed by a raw id.
//!
//! **Lazy loading, and objects still being built.** Naming a definition allocates its one heap object unloaded: a
//! slotless object of the export's class with `Origin::Resource`, as the decoder's stand-ins are (decision
//! 2026-11-15-08). References decoded inside a definition are such handles: loading one definition never loads
//! what it references. [`DefStore::find`], [`DefStore::resolve_ref`] and [`DefStore::load`] return the object
//! **loaded**: its class default object and its own tags are applied in place, so the handle stays the same
//! [`ObjRef`], and its origin becomes `Origin::Export` with the filed id. Because loads do not nest, no load ever
//! reaches an object that is still being built: references among definitions, cyclic ones included, are always
//! handles, and an object is handed out loaded only once it is complete. The decoder's `InstantiationCycle` therefore
//! cannot arise from the store's loads. A caller that reads a referenced definition's properties calls
//! [`DefStore::load`] on it first; reading a slot of an unloaded handle fails in the heap (`SlotMissing`), it never
//! reads a guessed value.
//!
//! **Prefix.** Lazy loads read an export's tags from the ordinary 4-byte prefix (`PropertyPrefix::Ordinary`), the
//! value every C++ definition consumer passes (`behavior.cpp`, `mission.cpp`, `kismet.cpp`, `combat_run.cpp`). A
//! component or a placed actor is loaded with [`DefStore::load_export`] and the caller's prefix.
//!
//! **Code is not a definition.** Class, struct, enum, function, state and property exports are reflection's
//! (`ow_object::reflect`): [`DefStore::find`] refuses them with [`DefError::NotADefinition`]; their layouts are
//! reached through [`DefStore::class_id`] and [`DefStore::key`]. A `Default__<Class>` export is that class's default
//! object, the one [`DefStore::class_default`] gives.
//!
//! **Parity mode.** The store passes its mode to every decode: under `ParityMode::Reference` the decoder keeps its C++
//! stand-ins (decision 2026-11-15-08, architecture section 6.1). The store adds none of its own.
//!
//! Status: contract only (W3-DEFS contract commit). Every method that loads or looks anything up returns
//! [`DefError::NotYet`]; the constructors and accessors work.

use std::fmt;
use std::path::{Path, PathBuf};

use ow_core::{NotYet, ObjectPath, ParityMode};
use ow_object::decode::{DecodeError, DecodeState};
use ow_object::heap::{HeapError, ObjectSpace};
use ow_object::ids::{ClassId, ObjRef, PropAddr, PropId};
use ow_object::prefix::PropertyPrefix;
use ow_object::reflect::{CodeIndex, ReflectError};
use ow_package::PackageId;
use ow_package::store::{PackageStore, StoreError};

use crate::identity::{IdentityPolicy, Owner};
use crate::keys::KeyCache;
use crate::overlay::{Overlay, OverlayId, OverlayStatus, ValueOrigin};
use crate::tail::Tails;

/// The record a `NotYet` of this crate names.
pub const RECORD: &str = "docs/rust-port/ARCHITECTURE.md section 3.4 (lane W3-DEFS)";

/// Why a definition store operation failed. The layer variants are transparent: they print as the inner error, and
/// their `source()` is the inner error's cause.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum DefError {
    Store(StoreError),
    Reflect(ReflectError),
    Decode(DecodeError),
    Heap(HeapError),
    /// An overlay or package file could not be read or written.
    Io {
        path: String,
        reason: String,
    },
    /// An overlay file is not well-formed (1-based line and column).
    OverlaySyntax {
        line: usize,
        column: usize,
        reason: String,
    },
    /// An overlay file is well-formed but not of the expected shape.
    OverlayShape {
        object: String,
        reason: String,
    },
    /// A patch's property path is outside the grammar (`crate::overlay::PropertyPath`).
    BadPropertyPath {
        path: String,
        reason: String,
    },
    /// A patch that does not type against its object (recorded in `OverlayStatus::failed`).
    BadPatch {
        object: String,
        property: String,
        reason: String,
    },
    /// The class at this path declares no such property, nor does any class it derives from.
    NoSuchProperty {
        class: String,
        property: String,
    },
    /// The path or object is not a definition (code, a run-time object, or not this store's).
    NotADefinition(String),
    NotYet(NotYet),
}

impl fmt::Display for DefError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DefError::Store(e) => e.fmt(f),
            DefError::Reflect(e) => e.fmt(f),
            DefError::Decode(e) => e.fmt(f),
            DefError::Heap(e) => e.fmt(f),
            DefError::Io { path, reason } => write!(f, "{path}: {reason}"),
            DefError::OverlaySyntax {
                line,
                column,
                reason,
            } => write!(f, "overlay line {line}, column {column}: {reason}"),
            DefError::OverlayShape { object, reason } => {
                write!(f, "overlay entry {object}: {reason}")
            }
            DefError::BadPropertyPath { path, reason } => {
                write!(f, "bad property path {path:?}: {reason}")
            }
            DefError::BadPatch {
                object,
                property,
                reason,
            } => write!(f, "patch {object} {property}: {reason}"),
            DefError::NoSuchProperty { class, property } => {
                write!(f, "{class} has no property {property}")
            }
            DefError::NotADefinition(what) => write!(f, "not a definition: {what}"),
            DefError::NotYet(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for DefError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DefError::Store(e) => e.source(),
            DefError::Reflect(e) => e.source(),
            DefError::Decode(e) => e.source(),
            DefError::Heap(e) => e.source(),
            _ => None,
        }
    }
}

impl From<StoreError> for DefError {
    fn from(e: StoreError) -> Self {
        DefError::Store(e)
    }
}

impl From<ReflectError> for DefError {
    fn from(e: ReflectError) -> Self {
        DefError::Reflect(e)
    }
}

impl From<DecodeError> for DefError {
    fn from(e: DecodeError) -> Self {
        DefError::Decode(e)
    }
}

impl From<HeapError> for DefError {
    fn from(e: HeapError) -> Self {
        DefError::Heap(e)
    }
}

impl From<NotYet> for DefError {
    fn from(e: NotYet) -> Self {
        DefError::NotYet(e)
    }
}

fn not_yet(what: &'static str) -> DefError {
    DefError::NotYet(NotYet::not_ported(what, RECORD))
}

/// The definition store (module documentation).
#[derive(Debug)]
pub struct DefStore {
    packages: PackageStore,
    code: CodeIndex,
    state: DecodeState,
    policy: IdentityPolicy,
    mode: ParityMode,
    overlays: Vec<(Overlay, OverlayStatus)>,
    keys: KeyCache,
    tails: Tails,
}

impl Default for DefStore {
    fn default() -> Self {
        Self::new_empty()
    }
}

impl DefStore {
    /// A store over `store` (scanned or not; an unscanned store finds nothing by name), choosing holders by `policy`
    /// and decoding in `mode`. No overlay is installed (D1).
    pub fn new(store: PackageStore, policy: IdentityPolicy, mode: ParityMode) -> Self {
        DefStore {
            packages: store,
            code: CodeIndex::new(),
            state: DecodeState::new(),
            policy,
            mode,
            overlays: Vec::new(),
            keys: KeyCache::default(),
            tails: Tails::default(),
        }
    }

    /// An empty store: no packages, [`IdentityPolicy::FirstInScanOrder`], `ParityMode::Off`. Every lookup finds
    /// nothing. Kept from the scaffold for tests that need a store and no data.
    pub fn new_empty() -> Self {
        Self::new(
            PackageStore::new_empty(),
            IdentityPolicy::FirstInScanOrder,
            ParityMode::Off,
        )
    }

    /// [`PackageStore::open`] on `cooked_root`, then [`DefStore::new`].
    pub fn open(
        cooked_root: PathBuf,
        policy: IdentityPolicy,
        mode: ParityMode,
    ) -> Result<Self, DefError> {
        Ok(Self::new(PackageStore::open(cooked_root)?, policy, mode))
    }

    /// Test support: writes each `(relative file name, bytes)` under `dir` (created if missing; sub-folders in the
    /// name are created too), then [`DefStore::open`]s `dir`. Meant for synthetic packages built with
    /// `ow_package::testkit` (AGENTS.md rule 9); the caller owns `dir` and removes it.
    pub fn from_synthetic(
        dir: &Path,
        files: &[(&str, &[u8])],
        policy: IdentityPolicy,
        mode: ParityMode,
    ) -> Result<Self, DefError> {
        let io = |path: &Path, e: std::io::Error| DefError::Io {
            path: path.display().to_string(),
            reason: e.to_string(),
        };
        std::fs::create_dir_all(dir).map_err(|e| io(dir, e))?;
        for (name, bytes) in files {
            let path = dir.join(name);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| io(parent, e))?;
            }
            std::fs::write(&path, bytes).map_err(|e| io(&path, e))?;
        }
        Self::open(dir.to_path_buf(), policy, mode)
    }

    pub fn policy(&self) -> &IdentityPolicy {
        &self.policy
    }

    pub fn mode(&self) -> ParityMode {
        self.mode
    }

    /// The package store, for reading packages by the ids the store's objects carry (`Origin::Export`).
    pub fn packages(&self) -> &PackageStore {
        &self.packages
    }

    /// The reflection index the store decodes with (class keys, filed ids).
    pub fn code(&self) -> &CodeIndex {
        &self.code
    }

    /// The decode log lines (`defaults: ...`, the C++ `Runtime::log` texts) the store's loads wrote since the last
    /// call, in order.
    pub fn take_log(&mut self) -> Vec<String> {
        std::mem::take(&mut self.state.log)
    }

    /// The definition at `path`, loaded, with every overlay applied: the export of the holder the policy picks
    /// (`crate::identity`). `Ok(None)` where no holder exists (under `Loaded`, none in the load set). Code exports
    /// are [`DefError::NotADefinition`]; a `Default__<Class>` export gives that class's default object. A holder
    /// that fails to load is an error, and the object stays unloaded.
    pub fn find(
        &mut self,
        w: &mut ObjectSpace,
        path: &ObjectPath,
    ) -> Result<Option<ObjRef>, DefError> {
        let _ = (w, path);
        Err(not_yet("DefStore::find"))
    }

    /// The object `reference` of package `from` names, loaded: `None` for 0. `from` may be any id the package store
    /// handed out for the file. An export reference is that export's object path, an import its path; either goes
    /// through the identity policy like [`DefStore::find`], so a path is one object however it is reached. An import
    /// whose path no holder has is `Ok(None)` under `Loaded` and the store's `ImportNotFound` otherwise; a reference
    /// outside the package's tables is an error in either mode.
    pub fn resolve_ref(
        &mut self,
        w: &mut ObjectSpace,
        from: PackageId,
        reference: i32,
    ) -> Result<Option<ObjRef>, DefError> {
        let _ = (w, from, reference);
        Err(not_yet("DefStore::resolve_ref"))
    }

    /// Loads `obj` in place if it is one of the store's unloaded handles (module documentation) and returns it; a
    /// loaded definition is returned as it is. An object the store did not make is [`DefError::NotADefinition`].
    pub fn load(&mut self, w: &mut ObjectSpace, obj: ObjRef) -> Result<ObjRef, DefError> {
        let _ = (w, obj);
        Err(not_yet("DefStore::load"))
    }

    /// Whether `obj` is a definition of this store that is loaded.
    pub fn is_loaded(&self, w: &ObjectSpace, obj: ObjRef) -> bool {
        let _ = (w, obj);
        false
    }

    /// Export `index` of `package`, loaded with `prefix` (a component: 8; a placed actor: 26). It is the same
    /// object as the path's (identity by path), so an export another holder owns under the policy is
    /// [`DefError::NotADefinition`] here rather than a second copy. Loading an object already loaded with another
    /// prefix is an error.
    pub fn load_export(
        &mut self,
        w: &mut ObjectSpace,
        package: PackageId,
        index: i32,
        prefix: PropertyPrefix,
    ) -> Result<ObjRef, DefError> {
        let _ = (w, package, index, prefix);
        Err(not_yet("DefStore::load_export"))
    }

    /// Every file holding `path`, in the store's scan order, whatever the policy (for audits: the policy's choice is
    /// the first under `FirstInScanOrder`). Files that fail to load are not holders, as in the store's global
    /// fallback.
    pub fn owners(&mut self, path: &ObjectPath) -> Result<Vec<Owner>, DefError> {
        let _ = path;
        Err(not_yet("DefStore::owners"))
    }

    /// Installs overlay `o` above the earlier ones (`crate::overlay`): every property path is parsed first, and one
    /// that does not parse refuses the whole overlay with nothing installed. Then its patches apply to the loaded
    /// objects at once and to the others when they load; patches that do not type are recorded in its status.
    pub fn push_overlay(&mut self, w: &mut ObjectSpace, o: Overlay) -> Result<OverlayId, DefError> {
        let _ = (w, o);
        Err(not_yet("DefStore::push_overlay"))
    }

    /// What became of overlay `id`'s patches so far; `None` for an id this store did not hand out.
    pub fn overlay_status(&self, id: OverlayId) -> Option<&OverlayStatus> {
        self.overlays.get(id.raw() as usize).map(|(_, s)| s)
    }

    /// Where element `at` of definition `obj` comes from: cooked, or the overlay that wrote it last. A patch inside a
    /// struct or array element marks the whole element `at` addresses.
    pub fn origin(&self, obj: ObjRef, at: PropAddr) -> ValueOrigin {
        let _ = (obj, at);
        ValueOrigin::NotADefinition
    }

    /// The bytes after loaded definition `obj`'s tag stream to the end of its export (`crate::tail`), possibly
    /// empty; `None` for an object that is not a loaded definition of this store.
    pub fn raw_tail(&self, obj: ObjRef) -> Option<&[u8]> {
        let _ = (obj, &self.tails);
        None
    }

    /// Every object path whose export's class is exactly `class_path` (`Package.Class`; subclasses are not included)
    /// over every file of the scan, `Default__` objects left out, each path once at its first holder's place, in scan
    /// order then export order: the tools' census (`tools/weapon_catalog.py` `Census`). Loads no definition.
    pub fn exports_of_class(&mut self, class_path: &str) -> Result<Vec<ObjectPath>, DefError> {
        let _ = class_path;
        Err(not_yet("DefStore::exports_of_class"))
    }

    /// The typed key of property `prop` of the class at `class_path` (`crate::keys`), its layout defined in `w` on
    /// first use and the key cached.
    pub fn key(
        &mut self,
        w: &mut ObjectSpace,
        class_path: &str,
        prop: &str,
    ) -> Result<PropId, DefError> {
        let _ = (w, class_path, prop, &self.keys);
        Err(not_yet("DefStore::key"))
    }

    /// The class at `class_path` (`Package.Class`), its layout defined in `w` on first use.
    pub fn class_id(&mut self, w: &mut ObjectSpace, class_path: &str) -> Result<ClassId, DefError> {
        let _ = (w, class_path);
        Err(not_yet("DefStore::class_id"))
    }

    /// The default object of the class at `class_path`, built on first use (`ow_object::cdo::class_default`), with no
    /// overlay applied (overlays patch definitions, not class defaults).
    pub fn class_default(
        &mut self,
        w: &mut ObjectSpace,
        class_path: &str,
    ) -> Result<ObjRef, DefError> {
        let _ = (w, class_path);
        Err(not_yet("DefStore::class_default"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overlay::OverlayKind;
    use ow_package::testkit::PackageBuilder;

    #[test]
    fn the_empty_store_answers_not_yet_until_the_body_lands() {
        let mut defs = DefStore::new_empty();
        let mut w = ObjectSpace::new();
        let path = ObjectPath::parse("GD_Test.Part").unwrap();
        assert!(matches!(defs.find(&mut w, &path), Err(DefError::NotYet(_))));
        assert!(matches!(
            defs.push_overlay(&mut w, Overlay::new("t", OverlayKind::Test, Vec::new())),
            Err(DefError::NotYet(_))
        ));
        assert_eq!(defs.overlay_status(OverlayId::new(0)), None);
        assert_eq!(defs.mode(), ParityMode::Off);
        assert_eq!(defs.policy(), &IdentityPolicy::FirstInScanOrder);
    }

    #[test]
    fn a_synthetic_store_opens_its_files() {
        let dir = std::env::temp_dir().join(format!("ow-defs-contract-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let bytes = PackageBuilder::new_empty().build();
        let defs = DefStore::from_synthetic(
            &dir,
            &[("Core.upk", &bytes), ("DLC/Extra.upk", &bytes)],
            IdentityPolicy::FirstInScanOrder,
            ParityMode::Reference,
        )
        .unwrap();
        assert_eq!(defs.packages().indexed_package_count(), 2);
        assert_eq!(defs.mode(), ParityMode::Reference);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn errors_print_their_layer_and_keep_its_cause() {
        let e = DefError::from(StoreError::PackageNotFound("X".into()));
        assert_eq!(e.to_string(), "package not found: X");
        assert!(std::error::Error::source(&e).is_none());
    }
}
