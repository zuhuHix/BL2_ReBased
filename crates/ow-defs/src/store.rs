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
//!   `ParityMode::Reference` it is the decoder's stand-in (`ValueCx::resource`), as the C++ `Runtime::resource`
//!   caches it: keyed by the export `CodeIndex::resolve_ref` resolves the import to where it resolves (under `Loaded`,
//!   a holder outside the load set still resolves so, and every import of that export shares the stand-in), and by
//!   (filed id of the importing package, import index), one stand-in per import, only where it does not resolve;
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
//! - any other export is read from its derived prefix only when a structural check holds: the tag stream from that
//!   prefix parses cleanly to its `None` tag within the export's serial size. Otherwise the load is
//!   `DefError::NotYet`, never a guessed prefix. The derived prefix is 8 for a `Core.Component` subclass that is not
//!   an actor component (such as the distribution subobjects PHASELOCK_STOCK_DATA.md records at 4, 8 or 16); 26 for
//!   an actor whose chain of outer exports reaches a level but whose outer is not the level itself; and 4
//!   (`PropertyPrefix::Ordinary`) for an actor whose outer chain reaches no level, such as an archetype (every pawn
//!   archetype, Marcus's), which is the prefix every C++ read of such an actor passes (`combat.cpp` `spawnEnemy`,
//!   `mission_script.cpp` `placeMarcus`; decision 2026-11-16-36).
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
//! **Iterating definitions.** Loaded definitions are instances (`Origin::Export`), so they appear in `Heap::live()`;
//! unloaded handles do not. `live()` yields them in first-sight order (the order their handles were allocated, when a
//! reference or a lookup first named them), not in load order. That order depends on what happened to be named, so
//! code that iterates
//! definitions (W4-INTERP2's iterators included) goes through [`DefStore::exports_of_class`] or [`DefStore::census`]
//! and loads what it visits, never through `live()`.
//!
//! **Parity mode.** The store passes its mode to every decode: under `ParityMode::Reference` the decoder keeps its C++
//! stand-ins (decision 2026-11-15-08, architecture section 6.1). The store adds none of its own.
//!
//! **Cost.** Under [`IdentityPolicy::FirstInScanOrder`] the first holder lookup builds the package store's global index
//! (`PackageStore::global_candidates`), which reads every package of the tree once, as the C++ global fallback does on
//! its first use (about 6.6 s for the 934 files of the installed `CookedPCConsole`, measured locally). Holder answers
//! are cached per path. A candidate file a lookup reads is kept only as a list of its export paths; only holders'
//! files stay loaded in the package store.
//!
//! Status: implemented (W3-DEFS, decision 2026-11-15-16). Identity, the holder rules, the prefix rule and the overlay
//! formats are this project's conventions or the tools', UNVERIFIED as rules of the game; the decode is W3-DECODE's.

use std::cell::RefCell;
use std::fmt;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use ow_container::ContainerError;
use ow_core::{FixedHashMap, FixedHashSet, NotYet, ObjectPath, ParityMode};
use ow_object::cdo;
use ow_object::decode::{DecodeError, DecodeState, ReferenceResolver, ValueCx};
use ow_object::heap::{HeapError, ObjectSpace, Origin};
use ow_object::ids::{ClassId, ObjRef, PropAddr, PropId, StructId};
use ow_object::prefix::PropertyPrefix;
use ow_object::reflect::{CodeIndex, ReflectError};
use ow_object::value::Value;
use ow_package::PackageId;
use ow_package::package::{Package, PackageError};
use ow_package::reader::Reader;
use ow_package::store::{PackageStore, StoreError};
use ow_package::tables::{self, ObjectEntry};

use crate::identity::{
    Census, CensusEntry, FileFailure, FileIndex, Holders, IdentityPolicy, Owner, class_text,
    dotted, full_class, full_path, is_class_default, path_key,
};
use crate::keys::KeyCache;
use crate::overlay::{
    Linking, Overlay, OverlayId, OverlayKind, OverlayStatus, PatchValue, PropertyPath, ValueOrigin,
    write_patch,
};
use crate::tail::{Tail, Tails};

// The record a `NotYet` of this crate names.
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

fn package_error(e: PackageError) -> DefError {
    StoreError::Package(e).into()
}

/// Class names whose exports are code (reflection's), never definitions.
fn is_code_class(name: &str) -> bool {
    matches!(
        name,
        "Class"
            | "ScriptStruct"
            | "Struct"
            | "Enum"
            | "Function"
            | "State"
            | "Const"
            | "Package"
            | "TextBuffer"
            | "MetaData"
            | "ByteProperty"
            | "IntProperty"
            | "BoolProperty"
            | "FloatProperty"
            | "ObjectProperty"
            | "ClassProperty"
            | "ComponentProperty"
            | "InterfaceProperty"
            | "NameProperty"
            | "StrProperty"
            | "StructProperty"
            | "ArrayProperty"
            | "MapProperty"
            | "DelegateProperty"
            | "FloatAttributeProperty"
            | "IntAttributeProperty"
            | "ByteAttributeProperty"
    )
}

/// Why a slotless object of the store stays a stand-in (module documentation, "Stand-ins that stay stand-ins").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stay {
    /// No holder has its path (an import into no installed package, or none in the load set).
    NoHolder,
    /// The decoder's `?` stand-in, or a reference whose path cannot be built.
    OutsideTables,
    /// A stand-in of an engine-native class (`ClassTable::is_native`).
    NativeClass,
    /// A class, struct, enum, function, state, property or package export.
    Code,
    /// A class default object or an object inside one.
    ClassDefault,
}

impl Stay {
    fn reason(self) -> &'static str {
        match self {
            Stay::NoHolder => "no package holds it",
            Stay::OutsideTables => "a reference outside its package's tables",
            Stay::NativeClass => "an object of an engine-native class",
            Stay::Code => "a code export",
            Stay::ClassDefault => "a class default object, or inside one",
        }
    }
}

/// What the store knows of one of its objects.
#[derive(Debug, Clone)]
enum Entry {
    Unloaded {
        path: String,
        target: (PackageId, i32),
    },
    Loaded {
        class: ClassId,
    },
    Failed {
        error: DefError,
    },
    Stays {
        path: String,
        why: Stay,
    },
}

/// The identity map: object paths and holder exports to the store's objects, and what each object is.
#[derive(Debug, Default)]
struct Handles {
    /// Folded object path -> its object (in Off also the no-holder stand-ins).
    by_path: FixedHashMap<String, ObjRef>,
    /// Holder export (filed id, index) -> its object.
    by_target: FixedHashMap<(PackageId, i32), ObjRef>,
    entries: FixedHashMap<ObjRef, Entry>,
    /// Folded object path -> the holder the policy picked (cached; `None`: no holder).
    holders: FixedHashMap<String, Option<(PackageId, i32)>>,
    /// The export paths of every candidate file a holder lookup read, or why it could not be read: read once, and no
    /// package kept for it (only a holder's file is loaded into the package store).
    files: FixedHashMap<PathBuf, Result<Rc<FileIndex>, PackageError>>,
    /// Paths whose `defs: no holder` line was logged.
    no_holder_logged: FixedHashSet<String>,
}

impl Handles {
    /// The export paths of `file`, read on first use.
    fn file_index(&mut self, file: &Path) -> Result<Rc<FileIndex>, PackageError> {
        self.files
            .entry(file.to_path_buf())
            .or_insert_with(|| FileIndex::read(file).map(Rc::new))
            .clone()
    }

    fn prune(&mut self, space: &ObjectSpace) {
        let live = |o: &ObjRef| space.heap.contains(*o);
        self.by_path.retain(|_, o| live(o));
        self.by_target.retain(|_, o| live(o));
        self.entries.retain(|o, _| live(o));
    }
}

/// The store's [`ReferenceResolver`]: every decoded reference becomes the store's object for its path (module
/// documentation), allocated unloaded on first sight.
struct Linker<'h> {
    handles: &'h RefCell<Handles>,
    policy: &'h IdentityPolicy,
}

impl ReferenceResolver for Linker<'_> {
    fn resolve(
        &self,
        cx: &mut ValueCx<'_>,
        package: PackageId,
        reference: i32,
    ) -> Result<Option<ObjRef>, DecodeError> {
        if reference == 0 {
            return Ok(None);
        }
        self.handle_for_ref(cx, package, reference).map(Some)
    }
}

impl Linker<'_> {
    fn live(&self, cx: &ValueCx<'_>, key: &str) -> Option<ObjRef> {
        let handles = self.handles.borrow();
        handles
            .by_path
            .get(key)
            .copied()
            .filter(|&o| cx.space.heap.contains(o))
    }

    /// The object `reference` of `package` names (module documentation).
    fn handle_for_ref(
        &self,
        cx: &mut ValueCx<'_>,
        package: PackageId,
        reference: i32,
    ) -> Result<ObjRef, DecodeError> {
        let pkg = cx.store.shared(package);
        let path = match full_path(&pkg, reference) {
            Ok(path) => path,
            Err(error) => {
                // Outside the tables or a broken outer chain: the decoder's own answer (the `?` stand-in under the
                // reference mode, an error otherwise).
                let o = cx.resource(package, reference)?.ok_or(error)?;
                self.handles
                    .borrow_mut()
                    .entries
                    .entry(o)
                    .or_insert(Entry::Stays {
                        path: "?".to_string(),
                        why: Stay::OutsideTables,
                    });
                return Ok(o);
            }
        };
        let key = path_key(&path);
        if let Some(o) = self.live(cx, &key) {
            return Ok(o);
        }
        match self.holder_of(cx, &path, &key)? {
            Some(target) => self.handle_for_target(cx, path, key, target),
            None => {
                let o = cx
                    .resource(package, reference)?
                    .ok_or(DecodeError::UnsupportedRequest)?;
                let mut handles = self.handles.borrow_mut();
                if handles.no_holder_logged.insert(key.clone()) {
                    cx.state.log.push(format!("defs: no holder for {path}"));
                }
                handles.entries.entry(o).or_insert(Entry::Stays {
                    path,
                    why: Stay::NoHolder,
                });
                if cx.mode != ParityMode::Reference {
                    handles.by_path.insert(key, o);
                }
                Ok(o)
            }
        }
    }

    /// The object of holder export `target` for `path`, allocated unloaded on first sight.
    fn handle_for_target(
        &self,
        cx: &mut ValueCx<'_>,
        path: String,
        key: String,
        target: (PackageId, i32),
    ) -> Result<ObjRef, DecodeError> {
        let existing = self.handles.borrow().by_target.get(&target).copied();
        if let Some(o) = existing.filter(|&o| cx.space.heap.contains(o)) {
            self.handles.borrow_mut().by_path.insert(key, o);
            return Ok(o);
        }
        let class_name = cx.code.class_name(cx.store, target.0, target.1);
        let o = cx
            .resource(target.0, target.1)?
            .ok_or(DecodeError::UnsupportedRequest)?;
        let why = if is_class_default(&path) {
            Some(Stay::ClassDefault)
        } else if is_code_class(&class_name) {
            Some(Stay::Code)
        } else if cx.space.classes.is_native(cx.space.heap.get(o)?.class) {
            Some(Stay::NativeClass)
        } else {
            None
        };
        let entry = match why {
            Some(why) => Entry::Stays { path, why },
            None => Entry::Unloaded { path, target },
        };
        let mut handles = self.handles.borrow_mut();
        handles.entries.entry(o).or_insert(entry);
        handles.by_target.insert(target, o);
        handles.by_path.insert(key, o);
        Ok(o)
    }

    /// The holder of `path` under the policy (module documentation of `crate::identity`), cached by its key.
    fn holder_of(
        &self,
        cx: &mut ValueCx<'_>,
        path: &str,
        key: &str,
    ) -> Result<Option<(PackageId, i32)>, DecodeError> {
        if let Some(&answer) = self.handles.borrow().holders.get(key) {
            return Ok(answer);
        }
        // Searched in the `.` form, as the key is made: `A:B` and `A.B` are one path with one answer.
        let path = &dotted(path);
        let mut answer = None;
        match self.policy {
            IdentityPolicy::FirstInScanOrder => {
                for file in cx.store.global_candidates(path) {
                    // As the store's global fallback: a file that fails to load or whose search fails is no holder.
                    let found = self
                        .handles
                        .borrow_mut()
                        .file_index(&file)
                        .and_then(|index| index.find(path));
                    if let Ok(Some(export)) = found {
                        // Only the holder's file is loaded: its definition is decoded from it.
                        let id = cx.store.load_path(&file)?;
                        answer = cx.code.resolve_ref(cx.store, id, export);
                        break;
                    }
                }
            }
            IdentityPolicy::Loaded(set) => {
                for name in &set.packages {
                    let id = cx.store.load(name)?;
                    let index = cx.code.find_export(cx.store, id, path);
                    if index != 0 {
                        answer = Some((cx.code.filed_id(id), index));
                        break;
                    }
                }
            }
        }
        self.handles
            .borrow_mut()
            .holders
            .insert(key.to_string(), answer);
        Ok(answer)
    }
}

/// One installed overlay.
#[derive(Debug)]
struct Installed {
    overlay: Overlay,
    paths: Vec<PropertyPath>,
    status: OverlayStatus,
}

/// The overlay layers: the installed overlays, the patches waiting for their object to load, and which overlay wrote
/// each element last.
#[derive(Debug, Default)]
struct Overlays {
    installed: Vec<Installed>,
    pending: FixedHashMap<(PackageId, i32), Vec<(usize, usize)>>,
    origins: FixedHashMap<(ObjRef, PropAddr), OverlayId>,
}

/// The census and, per entry, its first holder's `Package.Class` (for `exports_of_class`).
#[derive(Debug)]
struct CensusBuilt {
    census: Census,
    first_class: Vec<String>,
}

/// Each loaded class's properties and their element counts (for `origin`).
type ClassProps = FixedHashMap<ClassId, FixedHashMap<PropId, u32>>;

/// The definition store (module documentation).
#[derive(Debug)]
pub struct DefStore {
    packages: PackageStore,
    code: CodeIndex,
    state: DecodeState,
    policy: IdentityPolicy,
    mode: ParityMode,
    handles: RefCell<Handles>,
    overlays: Overlays,
    keys: KeyCache,
    tails: Tails,
    class_props: ClassProps,
    /// For `exports_of_class`: every census path with its first holder's `Package.Class`, built once (the census
    /// itself is not kept: on the installed game it holds over a million paths).
    classes_of_paths: Option<Vec<(String, String)>>,
}

impl Default for DefStore {
    fn default() -> Self {
        Self::new_empty()
    }
}

/// Everything a load works with, borrowed from the store and the caller's object space at once.
struct Loader<'c, 'a> {
    cx: &'c mut ValueCx<'a>,
    linker: &'a Linker<'a>,
    overlays: &'c mut Overlays,
    tails: &'c mut Tails,
    class_props: &'c mut ClassProps,
}

/// [`crate::overlay::Linking`] over a load's context.
struct PatchLink<'c, 'a> {
    cx: &'c mut ValueCx<'a>,
    linker: &'a Linker<'a>,
}

impl Linking for PatchLink<'_, '_> {
    fn space(&mut self) -> &mut ObjectSpace {
        self.cx.space
    }

    fn object(&mut self, path: &str) -> Result<ObjRef, String> {
        let key = path_key(path);
        if let Some(o) = self.linker.live(self.cx, &key) {
            return Ok(o);
        }
        match self
            .linker
            .holder_of(self.cx, path, &key)
            .map_err(|e| e.to_string())?
        {
            None => Err(format!("no package holds {path}")),
            Some(target) => self
                .linker
                .handle_for_target(self.cx, path.to_string(), key, target)
                .map_err(|e| e.to_string()),
        }
    }

    fn class(&mut self, path: &str) -> Result<ClassId, String> {
        let key = self
            .cx
            .code
            .find_class(self.cx.store, path)
            .map_err(|e| e.to_string())?;
        self.cx.layout(key).map_err(|e| e.to_string())
    }

    fn struct_defaults(&mut self, s: StructId) -> Result<Value, String> {
        self.cx
            .ensure_struct_defaults(s)
            .map_err(|e| e.to_string())?;
        self.cx
            .space
            .classes
            .struct_defaults(s)
            .map_err(|e| e.to_string())
    }
}

impl Loader<'_, '_> {
    fn entry(&self, o: ObjRef) -> Option<Entry> {
        self.linker.handles.borrow().entries.get(&o).cloned()
    }

    /// Loads `o` if it is an unloaded handle. `strict`: anything else that is not a loaded definition is
    /// `NotADefinition`; otherwise it is returned unchanged.
    fn ensure(&mut self, o: ObjRef, strict: bool) -> Result<ObjRef, DefError> {
        self.cx.space.heap.get(o)?;
        match self.entry(o) {
            Some(Entry::Loaded { .. }) => Ok(o),
            Some(Entry::Failed { error }) => Err(error),
            Some(Entry::Unloaded { path, target }) => match self.load_unloaded(o, &path, target) {
                Ok(()) => Ok(o),
                Err(error) => {
                    let mut handles = self.linker.handles.borrow_mut();
                    handles.prune(self.cx.space);
                    if self.cx.space.heap.contains(o) {
                        handles.entries.insert(
                            o,
                            Entry::Failed {
                                error: error.clone(),
                            },
                        );
                    }
                    Err(error)
                }
            },
            Some(Entry::Stays { path, why }) if strict => Err(DefError::NotADefinition(format!(
                "{path}: {}",
                why.reason()
            ))),
            None if strict => Err(DefError::NotADefinition(format!(
                "object {o:?} was not made by this definition store"
            ))),
            _ => Ok(o),
        }
    }

    /// Instantiates holder export `target` and moves the result into handle `o` (module documentation).
    fn load_unloaded(
        &mut self,
        o: ObjRef,
        path: &str,
        target: (PackageId, i32),
    ) -> Result<(), DefError> {
        let (package, index) = target;
        let prefix = self.derive_prefix(package, index, path)?;
        let instance = cdo::instantiate_export(self.cx, package, index, prefix)?;
        let built = self.cx.space.free(instance.object)?;
        let class = built.class;
        {
            let object = self.cx.space.heap.get_mut(o)?;
            object.class = built.class;
            object.name = built.name;
            object.slots = built.slots;
            object.origin = Origin::Export { package, index };
        }
        let pkg = self.cx.store.shared(package);
        let entry = pkg.object(index).map_err(package_error)?.clone();
        // The outer is the store's object for the outer's path, unless the outer is code (a cooked `Package` object
        // above a definition, a class above its default subobjects): code has no object here, so the outer stays
        // `None` (no stand-in of an engine-native class is made for it). An export outer's class is its class
        // reference's name; an import outer's is the class name the import records.
        let outer_is_code = match entry.outer {
            0 => false,
            o if o > 0 => is_code_class(&self.cx.code.class_name(self.cx.store, package, o)),
            o => pkg
                .object(o)
                .is_ok_and(|import| is_code_class(&import.class_name)),
        };
        if entry.outer != 0 && !outer_is_code {
            match self.linker.handle_for_ref(self.cx, package, entry.outer) {
                Ok(outer) => self.cx.space.heap.get_mut(o)?.outer = Some(outer),
                Err(e) => self
                    .cx
                    .state
                    .log
                    .push(format!("defs: outer of {path}: {e}")),
            }
        }
        if let (Ok(start), Ok(size)) = (usize::try_from(entry.offset), usize::try_from(entry.size))
            && instance.tags_end <= size
            && start
                .checked_add(size)
                .is_some_and(|end| end <= pkg.data.len())
        {
            self.tails
                .spans
                .insert(o, (package, start + instance.tags_end..start + size));
        }
        if !self.class_props.contains_key(&class) {
            let props = self
                .cx
                .space
                .classes
                .props(class)
                .map_err(HeapError::from)?
                .into_iter()
                .map(|(p, d)| (p, d.array_dim.max(1) as u32))
                .collect();
            self.class_props.insert(class, props);
        }
        self.linker
            .handles
            .borrow_mut()
            .entries
            .insert(o, Entry::Loaded { class });
        self.apply_pending(target, o);
        Ok(())
    }

    /// The property prefix of export `index` of `package` (module documentation, "Prefix by class").
    fn derive_prefix(
        &mut self,
        package: PackageId,
        index: i32,
        path: &str,
    ) -> Result<PropertyPrefix, DefError> {
        let pkg = self.cx.store.shared(package);
        let entry = pkg.object(index).map_err(package_error)?.clone();
        let Some(class) = self.cx.code.class_at(self.cx.store, package, entry.class)? else {
            return Ok(PropertyPrefix::Ordinary);
        };
        let chain: Vec<String> = self
            .cx
            .code
            .super_chain(class)
            .map(|k| self.cx.code.class(k).path.to_ascii_lowercase())
            .collect();
        let has = |p: &str| chain.iter().any(|c| c == p);
        if has("engine.actorcomponent") {
            return Ok(PropertyPrefix::Component);
        }
        let actor = has("engine.actor");
        let component = has("core.component");
        if actor
            && entry.outer > 0
            && self.cx.code.class_name(self.cx.store, package, entry.outer) == "Level"
        {
            return Ok(PropertyPrefix::PlacedActor);
        }
        if !actor && !component {
            return Ok(PropertyPrefix::Ordinary);
        }
        let derived = if !actor {
            PropertyPrefix::Component
        } else if self.outer_chain_reaches_level(&pkg, package, entry.outer) {
            // Inside a level but not directly: no C++ call reads such an actor, so 26 only under the check.
            PropertyPrefix::PlacedActor
        } else {
            // An actor no level holds (an archetype): the C++ reads every one at 4 (`combat.cpp` spawnEnemy,
            // `mission_script.cpp` placeMarcus, `slice.cpp`; decision 2026-11-16-36).
            PropertyPrefix::Ordinary
        };
        if tags_parse(&pkg, &entry, derived.bytes()) {
            Ok(derived)
        } else {
            Err(DefError::NotYet(NotYet::not_read(
                format!(
                    "the property prefix of {path}: its tags do not parse from the {}-byte prefix its class gives",
                    derived.bytes()
                ),
                RECORD,
            )))
        }
    }

    /// Whether the export chain of outers from `outer` (an export reference of `package`) reaches an export of class
    /// `Level`. The walk follows exports only (an import or 0 ends it) and stops after as many steps as the package
    /// has exports, so a cyclic or broken chain ends as "no level".
    fn outer_chain_reaches_level(&mut self, pkg: &Package, package: PackageId, outer: i32) -> bool {
        let mut at = outer;
        for _ in 0..pkg.exports.len() {
            if at <= 0 {
                return false;
            }
            if self.cx.code.class_name(self.cx.store, package, at) == "Level" {
                return true;
            }
            match pkg.object(at) {
                Ok(e) => at = e.outer,
                Err(_) => return false,
            }
        }
        false
    }

    fn apply_pending(&mut self, target: (PackageId, i32), o: ObjRef) {
        let Some(waiting) = self.overlays.pending.remove(&target) else {
            return;
        };
        for (overlay, patch) in waiting {
            self.overlays.installed[overlay].status.pending -= 1;
            self.apply_one(overlay, patch, o);
        }
    }

    fn apply_one(&mut self, overlay: usize, patch: usize, o: ObjRef) {
        let installed = &self.overlays.installed[overlay];
        let path = installed.paths[patch].clone();
        let p = installed.overlay.patches[patch].clone();
        if installed.overlay.kind == OverlayKind::LiveCapture
            && self.untagged_empty(o, &path, &p.value)
        {
            // live_overlay.py's skip: the property keeps the value it inherits, as the script leaves it.
            self.overlays.installed[overlay].status.applied += 1;
            return;
        }
        let mut link = PatchLink {
            cx: &mut *self.cx,
            linker: self.linker,
        };
        let outcome = write_patch(&mut link, o, &path, &p.value);
        let installed = &mut self.overlays.installed[overlay];
        match outcome {
            Ok(addrs) => {
                installed.status.applied += 1;
                for a in addrs {
                    self.overlays
                        .origins
                        .insert((o, a), OverlayId::new(overlay as u32));
                }
            }
            Err(reason) => installed.status.failed.push((
                patch,
                DefError::BadPatch {
                    object: p.object.as_str().to_string(),
                    property: p.property,
                    reason,
                },
            )),
        }
    }

    /// Whether a live-capture patch is one `tools/real_game/live_overlay.py` skips: a whole property (one segment, no
    /// index) given as `null`, `[]` or `{}` that loaded definition `o`'s own export does not tag. Such a property
    /// keeps the value the class default object gives it; patching it would set `null` where the declaration takes
    /// none (an int, a float, a struct: a failed patch) or overwrite a non-empty inherited value.
    fn untagged_empty(&mut self, o: ObjRef, path: &PropertyPath, value: &PatchValue) -> bool {
        let empty = match value {
            PatchValue::Null => true,
            PatchValue::List(items) => items.is_empty(),
            PatchValue::Fields(fields) => fields.is_empty(),
            _ => false,
        };
        let [segment] = path.segments.as_slice() else {
            return false;
        };
        if !empty || segment.index.is_some() {
            return false;
        }
        let Ok(Origin::Export { package, index }) = self.cx.space.heap.get(o).map(|x| x.origin)
        else {
            return false;
        };
        let pkg = self.cx.store.shared(package);
        let Ok(entry) = pkg.object(index).cloned() else {
            return false;
        };
        let Ok(prefix) = self.derive_prefix(package, index, &segment.name) else {
            return false;
        };
        tag_names(&pkg, &entry, prefix.bytes())
            .is_some_and(|names| !names.iter().any(|n| n.eq_ignore_ascii_case(&segment.name)))
    }

    /// Places every patch of overlay `overlay` (`DefStore::push_overlay`).
    fn place_overlay(&mut self, overlay: usize) {
        let count = self.overlays.installed[overlay].overlay.patches.len();
        for patch in 0..count {
            let installed = &self.overlays.installed[overlay];
            let kind = installed.overlay.kind;
            let object = installed.overlay.patches[patch].object.as_str().to_string();
            if is_class_default(&object) {
                let error = match kind {
                    OverlayKind::TextMod | OverlayKind::Hotfix => {
                        DefError::NotYet(NotYet::not_read(
                            format!("patching the class default {object} ({kind:?} overlay)"),
                            RECORD,
                        ))
                    }
                    _ => DefError::NotADefinition(format!(
                        "{object}: a class default object, or inside one"
                    )),
                };
                self.overlays.installed[overlay]
                    .status
                    .failed
                    .push((patch, error));
                continue;
            }
            let key = path_key(&object);
            match self.linker.holder_of(self.cx, &object, &key) {
                Err(e) => self.overlays.installed[overlay]
                    .status
                    .failed
                    .push((patch, e.into())),
                Ok(None) => self.overlays.installed[overlay].status.pending += 1,
                Ok(Some(target)) => {
                    let loaded = {
                        let handles = self.linker.handles.borrow();
                        handles.by_target.get(&target).copied().filter(|o| {
                            matches!(handles.entries.get(o), Some(Entry::Loaded { .. }))
                        })
                    };
                    match loaded {
                        Some(o) => self.apply_one(overlay, patch, o),
                        None => {
                            self.overlays
                                .pending
                                .entry(target)
                                .or_default()
                                .push((overlay, patch));
                            self.overlays.installed[overlay].status.pending += 1;
                        }
                    }
                }
            }
        }
    }

    fn find(&mut self, text: &str) -> Result<Option<ObjRef>, DefError> {
        let key = path_key(text);
        if is_class_default(text) {
            let own = text
                .rsplit(['.', ':'])
                .next()
                .is_some_and(|last| last.starts_with("Default__"));
            if !own {
                return Err(DefError::NotADefinition(format!(
                    "{text}: inside a class default object"
                )));
            }
            let Some((package, index)) = self.linker.holder_of(self.cx, text, &key)? else {
                return Ok(None);
            };
            let pkg = self.cx.store.shared(package);
            let class_ref = pkg.object(index).map_err(package_error)?.class;
            let class = self
                .cx
                .code
                .class_at(self.cx.store, package, class_ref)?
                .ok_or_else(|| {
                    DefError::NotADefinition(format!("{text}: its class is not loaded"))
                })?;
            return Ok(Some(cdo::class_default(self.cx, class)?));
        }
        let Some(target) = self.linker.holder_of(self.cx, text, &key)? else {
            return Ok(None);
        };
        // Code is refused before a handle is made: a class object's own class is engine-native, which outside the
        // reference mode has no stand-in.
        if is_code_class(&self.cx.code.class_name(self.cx.store, target.0, target.1)) {
            return Err(DefError::NotADefinition(format!(
                "{text}: {}",
                Stay::Code.reason()
            )));
        }
        let o = match self.linker.live(self.cx, &key) {
            Some(o) => o,
            None => self
                .linker
                .handle_for_target(self.cx, text.to_string(), key, target)?,
        };
        self.ensure(o, true).map(Some)
    }
}

/// Whether export `entry`'s tag stream, read from `prefix` bytes into its payload, reaches its `None` tag inside the
/// export's serial data (module documentation, "Prefix by class"): each header readable, each value skipped by its
/// size, nothing past the end.
fn tags_parse(pkg: &Package, entry: &ObjectEntry, prefix: usize) -> bool {
    tag_names(pkg, entry, prefix).is_some()
}

/// The names of the tags in export `entry`'s stream from `prefix` bytes in, in order, when the stream reaches its
/// `None` tag inside the export's serial data; `None` otherwise.
fn tag_names(pkg: &Package, entry: &ObjectEntry, prefix: usize) -> Option<Vec<String>> {
    let start = usize::try_from(entry.offset).ok()?;
    let size = usize::try_from(entry.size).ok()?;
    let payload = start
        .checked_add(size)
        .and_then(|end| pkg.data.get(start..end))?;
    if prefix > payload.len() {
        return None;
    }
    let mut r = Reader::new(payload);
    r.set_pos(prefix);
    let mut names = Vec::new();
    loop {
        let name = tables::read_name(&mut r, &pkg.names).ok()?;
        if name == "None" {
            return Some(names);
        }
        let ty = tables::read_name(&mut r, &pkg.names).ok()?;
        let (value_size, array_index) = (r.i32().ok()?, r.i32().ok()?);
        if value_size < 0 || array_index < 0 {
            return None;
        }
        if ty == "StructProperty" || ty == "ByteProperty" {
            tables::read_name(&mut r, &pkg.names).ok()?;
        }
        if ty == "BoolProperty" {
            r.skip(1).ok()?;
        }
        r.skip(value_size as usize).ok()?;
        names.push(name);
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
            handles: RefCell::new(Handles::default()),
            overlays: Overlays::default(),
            keys: KeyCache::default(),
            tails: Tails::default(),
            class_props: ClassProps::default(),
            classes_of_paths: None,
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

    /// Runs `f` with a load context over this store and `w`.
    fn run<T>(&mut self, w: &mut ObjectSpace, f: impl FnOnce(&mut Loader<'_, '_>) -> T) -> T {
        let DefStore {
            packages,
            code,
            state,
            policy,
            mode,
            handles,
            overlays,
            tails,
            class_props,
            ..
        } = self;
        let linker = Linker {
            handles: &*handles,
            policy: &*policy,
        };
        let mut cx = ValueCx::new(packages, code, w, state, *mode).with_resolver(&linker);
        let mut loader = Loader {
            cx: &mut cx,
            linker: &linker,
            overlays,
            tails,
            class_props,
        };
        f(&mut loader)
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
        self.run(w, |l| l.find(path.as_str()))
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
        if reference == 0 {
            return Ok(None);
        }
        self.packages
            .get(from)
            .object(reference)
            .map_err(package_error)?;
        self.run(w, |l| {
            let o = l.linker.handle_for_ref(l.cx, from, reference)?;
            l.ensure(o, false).map(Some)
        })
    }

    /// Loads `obj` in place if it is one of the store's unloaded handles (module documentation) and returns it; a
    /// loaded definition is returned as it is. Strict: any object that is not a definition of this store (a run-time
    /// object, a class default object, a stand-in that stays one) is [`DefError::NotADefinition`], and a stale handle
    /// is [`DefError::Heap`]. Use [`DefStore::ensure_loaded`] for an object a slot may hold.
    pub fn load(&mut self, w: &mut ObjectSpace, obj: ObjRef) -> Result<ObjRef, DefError> {
        self.run(w, |l| l.ensure(obj, true))
    }

    /// Total loading: loads `obj` in place if it is an unloaded handle, and is `Ok(())` for every other live object
    /// (loaded definitions, run-time objects, class default objects, the stand-ins that stay stand-ins). Fails only
    /// for a stale handle ([`DefError::Heap`]) or a handle whose load fails.
    pub fn ensure_loaded(&mut self, w: &mut ObjectSpace, obj: ObjRef) -> Result<(), DefError> {
        self.run(w, |l| l.ensure(obj, false)).map(|_| ())
    }

    /// Whether `obj` is a definition of this store that is loaded.
    pub fn is_loaded(&self, w: &ObjectSpace, obj: ObjRef) -> bool {
        w.heap.contains(obj)
            && matches!(
                self.handles.borrow().entries.get(&obj),
                Some(Entry::Loaded { .. })
            )
    }

    /// Whether `obj` is one of the store's unloaded handles: [`DefStore::ensure_loaded`] would load it (or, for a
    /// handle whose load failed, return its error again). False for every other object, the stand-ins that stay
    /// stand-ins included.
    pub fn is_unloaded(&self, w: &ObjectSpace, obj: ObjRef) -> bool {
        w.heap.contains(obj)
            && matches!(
                self.handles.borrow().entries.get(&obj),
                Some(Entry::Unloaded { .. } | Entry::Failed { .. })
            )
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
        if index <= 0 {
            return Err(DefError::NotADefinition(format!(
                "reference {index} is not an export"
            )));
        }
        let pkg = self.packages.shared(package);
        pkg.object(index).map_err(package_error)?;
        let path = full_path(&pkg, index).map_err(package_error)?;
        self.run(w, |l| {
            let key = path_key(&path);
            let mine =
                l.cx.code
                    .resolve_ref(l.cx.store, package, index)
                    .ok_or_else(|| DefError::NotADefinition(path.clone()))?;
            let holder = l.linker.holder_of(l.cx, &path, &key)?;
            if holder != Some(mine) {
                return Err(DefError::NotADefinition(format!(
                    "{path}: another file holds this path under the identity policy"
                )));
            }
            let derived = l.derive_prefix(mine.0, mine.1, &path)?;
            if derived != prefix {
                return Err(DefError::PrefixMismatch {
                    object: path.clone(),
                    derived,
                    given: prefix,
                });
            }
            let o = l.linker.handle_for_target(l.cx, path.clone(), key, mine)?;
            l.ensure(o, true)
        })
    }

    /// Every file holding `path`, in the store's scan order, whatever the policy (for audits: the policy's choice is
    /// the first under `FirstInScanOrder`), each holder's file loaded into the package store (so `Owner::id` is set),
    /// and the candidate files that failed to load or search, reported in `Holders::failures`. Only the files the
    /// store's global index lists as candidates for the path are read; a candidate that does not hold the path is read
    /// once for its export paths and not kept loaded.
    pub fn owners(&mut self, path: &ObjectPath) -> Result<Holders, DefError> {
        let mut out = Holders::default();
        let path = dotted(path.as_str());
        for file in self.packages.global_candidates(&path) {
            let found = self
                .handles
                .borrow_mut()
                .file_index(&file)
                .and_then(|index| index.find(&path));
            let export = match found {
                Err(e) => {
                    out.failures.push(FileFailure::new(file, package_error(e)));
                    continue;
                }
                Ok(Some(export)) => export,
                Ok(None) => continue,
            };
            let id = match self.packages.load_path(&file) {
                Ok(id) => id,
                Err(e) => {
                    out.failures.push(FileFailure::new(file, e.into()));
                    continue;
                }
            };
            let pkg = self.packages.shared(id);
            let class = match pkg.object(export).and_then(|e| class_text(&pkg, e.class)) {
                Ok(class) => class,
                Err(e) => {
                    out.failures.push(FileFailure::new(file, package_error(e)));
                    continue;
                }
            };
            // The id the reflection index files the package under.
            let filed = self
                .code
                .resolve_ref(&mut self.packages, id, export)
                .map_or(id, |(filed, _)| filed);
            out.holders.push(Owner::new(
                pkg.name.clone(),
                file,
                export,
                class,
                Some(filed),
            ));
        }
        Ok(out)
    }

    /// Every export path of every file of the scan, with its holders and their classes, the class defaults apart and
    /// the files that failed (`crate::identity::Census`). Reads each file once and keeps none loaded; loads no
    /// definition. Built anew on every call (nothing of it is kept).
    pub fn census(&mut self) -> Result<Census, DefError> {
        Ok(build_census(self.packages.scan_order()).census)
    }

    /// Installs overlay `o` above the earlier ones (`crate::overlay`): every property path is parsed first, and one
    /// that does not parse refuses the whole overlay with nothing installed. Then its patches apply to the loaded
    /// objects at once and to the others when they load; patches that do not type are recorded in its status.
    pub fn push_overlay(&mut self, w: &mut ObjectSpace, o: Overlay) -> Result<OverlayId, DefError> {
        let paths = o
            .patches
            .iter()
            .map(|p| PropertyPath::parse(&p.property))
            .collect::<Result<Vec<_>, _>>()?;
        let index = self.overlays.installed.len();
        let id =
            OverlayId::new(u32::try_from(index).map_err(|_| {
                DefError::NotYet(NotYet::not_read("more than 2^32 overlays", RECORD))
            })?);
        self.overlays.installed.push(Installed {
            overlay: o,
            paths,
            status: OverlayStatus::default(),
        });
        self.run(w, |l| l.place_overlay(index));
        Ok(id)
    }

    /// What became of overlay `id`'s patches so far; `None` for an id this store did not hand out.
    pub fn overlay_status(&self, id: OverlayId) -> Option<&OverlayStatus> {
        self.overlays
            .installed
            .get(id.raw() as usize)
            .map(|i| &i.status)
    }

    /// Where element `at` of definition `obj` comes from: cooked, or the overlay that wrote it last. A patch inside a
    /// struct or array element marks the whole element `at` addresses.
    pub fn origin(&self, obj: ObjRef, at: PropAddr) -> ValueOrigin {
        let handles = self.handles.borrow();
        let Some(Entry::Loaded { class }) = handles.entries.get(&obj) else {
            return ValueOrigin::NotADefinition;
        };
        let valid = self
            .class_props
            .get(class)
            .and_then(|props| props.get(&at.prop))
            .is_some_and(|&dim| at.index < dim);
        if !valid {
            return ValueOrigin::NotADefinition;
        }
        match self.overlays.origins.get(&(obj, at)) {
            Some(&id) => ValueOrigin::Overlay(id),
            None => ValueOrigin::Cooked,
        }
    }

    /// The bytes after loaded definition `obj`'s tag stream to the end of its export (`crate::tail`), as an owned
    /// view (possibly empty); `None` for an object that is not a loaded definition of this store.
    pub fn raw_tail(&self, obj: ObjRef) -> Option<Tail> {
        if !matches!(
            self.handles.borrow().entries.get(&obj),
            Some(Entry::Loaded { .. })
        ) {
            return None;
        }
        let (package, range) = self.tails.spans.get(&obj)?.clone();
        Tail::new(package, self.packages.shared(package), range)
    }

    /// Every object path whose export's class is exactly `class_path` (`Package.Class`; subclasses are not included)
    /// over every file of the scan, `Default__` objects left out, each path once at its first holder's place, in scan
    /// order then export order: the tools' census (`tools/weapon_catalog.py` `Census`). Loads no definition.
    pub fn exports_of_class(&mut self, class_path: &str) -> Result<Vec<ObjectPath>, DefError> {
        if self.classes_of_paths.is_none() {
            let built = build_census(self.packages.scan_order());
            self.classes_of_paths = Some(
                built
                    .census
                    .entries
                    .into_iter()
                    .map(|e| e.path)
                    .zip(built.first_class)
                    .collect(),
            );
        }
        let mut out = Vec::new();
        for (path, class) in self.classes_of_paths.iter().flatten() {
            if class.eq_ignore_ascii_case(class_path) {
                out.push(ObjectPath::parse(path).map_err(|e| {
                    DefError::NotADefinition(format!("{path}: not an object path ({e})"))
                })?);
            }
        }
        Ok(out)
    }

    /// The typed key of property `prop` of the class at `class_path` (`crate::keys`), its layout defined in `w` on
    /// first use and the key cached.
    pub fn key(
        &mut self,
        w: &mut ObjectSpace,
        class_path: &str,
        prop: &str,
    ) -> Result<PropId, DefError> {
        let cache_key = (class_path.to_ascii_lowercase(), prop.to_ascii_lowercase());
        if let Some(&p) = self.keys.keys.get(&cache_key) {
            return Ok(p);
        }
        let class = self.class_id(w, class_path)?;
        let p = w
            .names
            .find(prop)
            .and_then(|n| w.classes.prop(class, n))
            .ok_or_else(|| DefError::NoSuchProperty {
                class: class_path.to_string(),
                property: prop.to_string(),
            })?;
        self.keys.keys.insert(cache_key, p);
        Ok(p)
    }

    /// The class at `class_path` (`Package.Class`), its layout defined in `w` on first use.
    pub fn class_id(&mut self, w: &mut ObjectSpace, class_path: &str) -> Result<ClassId, DefError> {
        self.run(w, |l| {
            let key = l.cx.code.find_class(l.cx.store, class_path)?;
            Ok(l.cx.layout(key)?)
        })
    }

    /// The default object of the class at `class_path`, built on first use (`ow_object::cdo::class_default`), with no
    /// overlay applied (overlays patch definitions, not class defaults).
    pub fn class_default(
        &mut self,
        w: &mut ObjectSpace,
        class_path: &str,
    ) -> Result<ObjRef, DefError> {
        self.run(w, |l| {
            let key = l.cx.code.find_class(l.cx.store, class_path)?;
            Ok(cdo::class_default(l.cx, key)?)
        })
    }
}

/// The census of `files` (`DefStore::census`).
fn build_census(files: &[PathBuf]) -> CensusBuilt {
    let mut census = Census {
        files: files.to_vec(),
        ..Census::default()
    };
    let mut first_class = Vec::new();
    let mut entry_at: FixedHashMap<String, usize> = FixedHashMap::default();
    let mut default_at: FixedHashMap<String, usize> = FixedHashMap::default();
    for file in files {
        let pkg = match Package::load(file) {
            Ok(pkg) => pkg,
            Err(e) => {
                census
                    .failures
                    .push(FileFailure::new(file.clone(), package_error(e)));
                continue;
            }
        };
        // The whole listing first: a file whose listing fails contributes nothing (a failing `--exports`).
        let listing: Result<Vec<(String, String, String)>, PackageError> = (1..=pkg.exports.len()
            as i32)
            .map(|i| {
                let class = pkg.object(i)?.class;
                Ok((
                    pkg.path_of(i)?,
                    class_text(&pkg, class)?,
                    full_class(&pkg, class)?,
                ))
            })
            .collect();
        let listing = match listing {
            Ok(listing) => listing,
            Err(e) => {
                census
                    .failures
                    .push(FileFailure::new(file.clone(), package_error(e)));
                continue;
            }
        };
        for (i, (path, class, full)) in listing.into_iter().enumerate() {
            let owner = Owner::new(pkg.name.clone(), file.clone(), i as i32 + 1, class, None);
            let (entries, at) = if is_class_default(&path) {
                (&mut census.class_defaults, &mut default_at)
            } else {
                (&mut census.entries, &mut entry_at)
            };
            match at.get(&path) {
                Some(&k) => entries[k].holders.push(owner),
                None => {
                    at.insert(path.clone(), entries.len());
                    if !is_class_default(&path) {
                        first_class.push(full);
                    }
                    entries.push(CensusEntry::new(path, vec![owner]));
                }
            }
        }
    }
    CensusBuilt {
        census,
        first_class,
    }
}

#[cfg(test)]
mod tests;
