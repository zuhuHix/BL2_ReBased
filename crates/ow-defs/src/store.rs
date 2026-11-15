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
//! cannot arise from the store's loads.
//!
//! **Callers load before they read.** A caller that reads a referenced definition's properties loads it first:
//! [`DefStore::ensure_loaded`] for any object a slot may hold (it leaves every object that is not an unloaded handle
//! as it is), or [`DefStore::load`] where the object must be a definition. Reading a slot of an unloaded handle fails
//! in the heap (`SlotMissing`); it never reads a guessed value.
//!
//! **Stand-ins that stay stand-ins.** Three kinds of slotless object are not unloaded handles and never load:
//! - a decoded reference whose path no holder has (an import into no installed package, or none in the load set)
//!   keeps a stand-in that names the import (`Origin::Resource` with the importing package and the import's
//!   reference), as the C++ `Runtime::resource` does, and the store logs `defs: no holder for <path>` once per path
//!   ([`DefStore::take_log`]). It never decodes as `None`. Its key follows the parity mode: in `ParityMode::Off` it
//!   is keyed by the folded path, so every reference to that path is the same stand-in (the identity rule); under
//!   `ParityMode::Reference` it is keyed by (filed id of the importing package, import index), one stand-in per
//!   import, as the C++ `Runtime::resource` caches it;
//! - under `ParityMode::Reference`, the decoder's `?` stand-in for a reference outside its package's tables (decision
//!   2026-11-15-08) is kept;
//! - a stand-in of an engine-native class (`ClassTable::is_native`, Reference mode only) is kept.
//!
//! [`DefStore::ensure_loaded`] returns all three unchanged; [`DefStore::load`] refuses them with
//! [`DefError::NotADefinition`].
//!
//! **Prefix by class.** Where an export's tags start is derived from the export itself, so the order loads happen in
//! cannot change a decode:
//! - an export whose class derives from `Engine.ActorComponent` reads its tags from 8 bytes in
//!   (`PropertyPrefix::Component`), the only case the C++ loads at 8 (`combat_death.cpp`) and the one
//!   `tools/mission_closure.py` gives 8;
//! - an export whose class derives from `Engine.Actor` and whose outer is a level (an export of class `Engine.Level`)
//!   reads from 26 (`PropertyPrefix::PlacedActor`);
//! - an export of a class deriving from neither `Core.Component` nor `Engine.Actor` reads from 4
//!   (`PropertyPrefix::Ordinary`, the value every C++ definition consumer passes);
//! - any other export (a `Core.Component` subclass that is not an actor component, such as the distribution
//!   subobjects PHASELOCK_STOCK_DATA.md records at 4, 8 or 16; an actor that is not placed in a level, such as an
//!   archetype) is read from its derived prefix only when a structural check holds: the tag stream from that prefix
//!   parses cleanly to its `None` tag within the export's serial size. Otherwise the load is `DefError::NotYet`, never
//!   a guessed prefix.
//!
//! The byte counts are the C++ and the tools' (`ow_object::prefix`); the per-class rule is this store's and UNVERIFIED
//! as a rule of the game, as the prefixes are. It belongs to the implementation and may be refined later (a 16-byte
//! case, `PropertyPrefix` being `#[non_exhaustive]`) without an API change. [`DefStore::load_export`]'s prefix is only
//! checked against the derived one.
//!
//! **A failed load is remembered.** When loading a handle fails, the store keeps the error for that handle: later
//! `find`, `resolve_ref`, `load` and `ensure_loaded` calls on it return the same error without decoding again and
//! without adding log lines, so [`DefStore::take_log`] does not depend on how often a caller retries. The handle
//! stays unloaded (`is_unloaded` stays true). A load that aborted (the decoder's resolve budget) frees what it
//! allocated; its handle is remembered with its error all the same.
//!
//! **Code is not a definition.** Class, struct, enum, function, state and property exports are reflection's
//! (`ow_object::reflect`): [`DefStore::find`] refuses them with [`DefError::NotADefinition`]; their layouts are
//! reached through [`DefStore::class_id`] and [`DefStore::key`]. A `Default__<Class>` export is that class's default
//! object, the one [`DefStore::class_default`] gives.
//!
//! **Code packages.** The store never calls `ow_object::reflect::load_code_packages`: its [`CodeIndex`] starts empty
//! and loads each class lazily, on the first decode or lookup that needs it, which is all a definition needs. A lane
//! that wants every class of the nine code packages laid out up front (the VM's function and native index) builds
//! that index itself and installs it through [`DefStore::code_mut`] **before the store's first decode or lookup**:
//! `let (code, store) = defs.code_mut(); *code = load_code_packages(store, &mut w.names, &mut w.classes)?;`. Later,
//! the store's classes are already in the class table and a second definition of them is refused (`AlreadyDefined`).
//!
//! **Iterating definitions.** Loaded definitions are instances (`Origin::Export`), so they appear in `Heap::live()`, in
//! load order; unloaded handles do not. That order depends on what happened to be loaded, so code that iterates
//! definitions (W4-INTERP2's iterators included) goes through [`DefStore::exports_of_class`] or [`DefStore::census`]
//! and loads what it visits, never through `live()`.
//!
//! **Parity mode.** The store passes its mode to every decode: under `ParityMode::Reference` the decoder keeps its C++
//! stand-ins (decision 2026-11-15-08, architecture section 6.1). The store adds none of its own.
//!
//! Status: contract only (W3-DEFS contract commit, reviews r1 and r2). Every method that loads or looks anything up returns
//! [`DefError::NotYet`]; the constructors and accessors work.

use std::fmt;
use std::path::{Path, PathBuf};

use ow_container::ContainerError;
use ow_core::{NotYet, ObjectPath, ParityMode};
use ow_object::decode::{DecodeError, DecodeState};
use ow_object::heap::{HeapError, ObjectSpace};
use ow_object::ids::{ClassId, ObjRef, PropAddr, PropId};
use ow_object::prefix::PropertyPrefix;
use ow_object::reflect::{CodeIndex, ReflectError};
use ow_package::PackageId;
use ow_package::package::PackageError;
use ow_package::store::{PackageStore, StoreError};

use crate::identity::{Census, Holders, IdentityPolicy};
use crate::keys::KeyCache;
use crate::overlay::{Overlay, OverlayId, OverlayStatus, ValueOrigin};
use crate::tail::{Tail, Tails};

/// The record a `NotYet` of this crate names.
pub const RECORD: &str = "docs/rust-port/ARCHITECTURE.md section 3.4 (lane W3-DEFS)";

/// Why a definition store operation failed. The layer variants are transparent: they print as the inner error, and
/// their `source()` is the inner error's cause. The `From` conversions flatten: a `NotYet` or a heap error from the
/// decode, reflection and store layers is [`DefError::NotYet`] or [`DefError::Heap`] (a container's `NotYet` inside a
/// package error too), and a store or reflection error inside a decode error is [`DefError::Store`] or
/// [`DefError::Reflect`], so a caller matches one variant per cause.
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
    /// The path or object is not a definition (code, a class default, a run-time object, a stand-in that stays one,
    /// or not this store's).
    NotADefinition(String),
    /// [`DefStore::load_export`] was given another prefix than the one the store derives for the export.
    PrefixMismatch {
        object: String,
        derived: PropertyPrefix,
        given: PropertyPrefix,
    },
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
            DefError::PrefixMismatch {
                object,
                derived,
                given,
            } => write!(
                f,
                "{object}: the store derives a {}-byte property prefix, not {}",
                derived.bytes(),
                given.bytes()
            ),
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

/// The `NotYet` a package error holds: the container decoder's.
fn package_not_yet(e: &PackageError) -> Option<NotYet> {
    match e {
        PackageError::Container(ContainerError::NotYet(n)) => Some(n.clone()),
        _ => None,
    }
}

impl From<StoreError> for DefError {
    fn from(e: StoreError) -> Self {
        match e {
            StoreError::NotYet(n) => DefError::NotYet(n),
            StoreError::Package(p) => match package_not_yet(&p) {
                Some(n) => DefError::NotYet(n),
                None => DefError::Store(StoreError::Package(p)),
            },
            other => DefError::Store(other),
        }
    }
}

impl From<ReflectError> for DefError {
    fn from(e: ReflectError) -> Self {
        match e {
            ReflectError::NotYet(n) => DefError::NotYet(n),
            ReflectError::Store(s) => s.into(),
            ReflectError::Package(p) => match package_not_yet(&p) {
                Some(n) => DefError::NotYet(n),
                None => DefError::Reflect(ReflectError::Package(p)),
            },
            other => DefError::Reflect(other),
        }
    }
}

impl From<DecodeError> for DefError {
    fn from(e: DecodeError) -> Self {
        match e {
            DecodeError::NotYet(n) => DefError::NotYet(n),
            DecodeError::Heap(h) => DefError::Heap(h),
            DecodeError::Store(s) => s.into(),
            DecodeError::Reflect(r) => r.into(),
            DecodeError::Package(p) => match package_not_yet(&p) {
                Some(n) => DefError::NotYet(n),
                None => DefError::Decode(DecodeError::Package(p)),
            },
            other => DefError::Decode(other),
        }
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
    /// and decoding in `mode`. No overlay is installed (D1). The reflection index starts empty (module
    /// documentation, "Code packages").
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

    /// The reflection index and the package store, borrowed together, for reflection calls that need both
    /// (`CodeIndex::class_at`, `find_method`, installing `load_code_packages`: module documentation, "Code packages").
    /// A caller may load classes and packages through it; it must not replace the index after the store's first
    /// decode or lookup, and must not `scan()` the package store (the store's identity map holds its ids).
    pub fn code_mut(&mut self) -> (&mut CodeIndex, &mut PackageStore) {
        (&mut self.code, &mut self.packages)
    }

    /// The decode log lines (`defaults: ...`, the C++ `Runtime::log` texts, and the store's own `defs: ...` lines)
    /// the store's loads wrote since the last call, in order.
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
    /// whose path no holder has is the stand-in naming the import (module documentation), not loaded and not an
    /// error; a reference outside the package's tables is an error in either mode.
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
    /// loaded definition is returned as it is. Strict: any object that is not a definition of this store (a run-time
    /// object, a class default object, a stand-in that stays one) is [`DefError::NotADefinition`], and a stale handle
    /// is [`DefError::Heap`]. Use [`DefStore::ensure_loaded`] for an object a slot may hold.
    pub fn load(&mut self, w: &mut ObjectSpace, obj: ObjRef) -> Result<ObjRef, DefError> {
        let _ = (w, obj);
        Err(not_yet("DefStore::load"))
    }

    /// Total loading: loads `obj` in place if it is an unloaded handle, and is `Ok(())` for every other live object
    /// (loaded definitions, run-time objects, class default objects, the stand-ins that stay stand-ins). Fails only
    /// for a stale handle ([`DefError::Heap`]) or a handle whose load fails.
    pub fn ensure_loaded(&mut self, w: &mut ObjectSpace, obj: ObjRef) -> Result<(), DefError> {
        let _ = (w, obj);
        Err(not_yet("DefStore::ensure_loaded"))
    }

    /// Whether `obj` is a definition of this store that is loaded.
    pub fn is_loaded(&self, w: &ObjectSpace, obj: ObjRef) -> bool {
        let _ = (w, obj);
        false
    }

    /// Whether `obj` is one of the store's unloaded handles: [`DefStore::ensure_loaded`] would load it. False for
    /// every other object, the stand-ins that stay stand-ins included.
    pub fn is_unloaded(&self, w: &ObjectSpace, obj: ObjRef) -> bool {
        let _ = (w, obj);
        false
    }

    /// Export `index` of `package`, loaded. Its tags are read from the prefix the store derives for it (module
    /// documentation, "Prefix by class"); `prefix` is the caller's expectation, checked against it
    /// ([`DefError::PrefixMismatch`]). An export whose prefix the store cannot derive is [`DefError::NotYet`].
    /// It is the same object as the path's (identity by path), so an export another holder owns under the policy is
    /// [`DefError::NotADefinition`] here rather than a second copy.
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
    /// the first under `FirstInScanOrder`), each loaded into the package store (so `Owner::id` is set), and the
    /// candidate files that failed to load or search, reported in `Holders::failures`. Only the files the store's
    /// global index lists as candidates for the path are read.
    pub fn owners(&mut self, path: &ObjectPath) -> Result<Holders, DefError> {
        let _ = path;
        Err(not_yet("DefStore::owners"))
    }

    /// Every export path of every file of the scan, with its holders and their classes, the class defaults apart and
    /// the files that failed (`crate::identity::Census`). Reads each file once and keeps none loaded; loads no
    /// definition.
    pub fn census(&mut self) -> Result<Census, DefError> {
        Err(not_yet("DefStore::census"))
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

    /// The bytes after loaded definition `obj`'s tag stream to the end of its export (`crate::tail`), as an owned
    /// view (possibly empty); `None` for an object that is not a loaded definition of this store.
    pub fn raw_tail(&self, obj: ObjRef) -> Option<Tail> {
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
        assert!(matches!(defs.census(), Err(DefError::NotYet(_))));
        assert_eq!(defs.overlay_status(OverlayId::new(0)), None);
        assert_eq!(defs.mode(), ParityMode::Off);
        assert_eq!(defs.policy(), &IdentityPolicy::FirstInScanOrder);
        let (_code, store) = defs.code_mut();
        assert_eq!(store.indexed_package_count(), 0);
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

    #[test]
    fn conversions_flatten_not_yet_heap_store_and_reflection() {
        let n = NotYet::not_read("x", "r");
        assert_eq!(
            DefError::from(DecodeError::NotYet(n.clone())),
            DefError::NotYet(n.clone())
        );
        assert_eq!(
            DefError::from(ReflectError::NotYet(n.clone())),
            DefError::NotYet(n.clone())
        );
        assert_eq!(
            DefError::from(StoreError::NotYet(n.clone())),
            DefError::NotYet(n)
        );
        let s = StoreError::PackageNotFound("P".into());
        assert_eq!(
            DefError::from(DecodeError::Reflect(ReflectError::Store(s.clone()))),
            DefError::Store(s.clone())
        );
        assert_eq!(
            DefError::from(DecodeError::Store(s.clone())),
            DefError::Store(s)
        );
        let n = NotYet::not_ported("lzo", "r");
        let p = PackageError::Container(ContainerError::NotYet(n.clone()));
        assert_eq!(
            DefError::from(StoreError::Package(p.clone())),
            DefError::NotYet(n.clone())
        );
        assert_eq!(DefError::from(DecodeError::Package(p)), DefError::NotYet(n));
        let h = HeapError::InstanceStandIn(ow_object::heap::Origin::Runtime);
        assert_eq!(
            DefError::from(DecodeError::Heap(h.clone())),
            DefError::Heap(h)
        );
    }
}
