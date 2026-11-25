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
//! [`ObjRef`], and its origin becomes `Origin::Export` with the filed id. (Under `ParityMode::Off` its archetype's
//! values stand in for the class default object's: "Archetypes" below.) Because loads do not nest, no load ever
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
//! - an object of an engine-native class (`ClassTable::is_native`) is kept as a stand-in (below, "Engine-native
//!   classes").
//!
//! [`DefStore::ensure_loaded`] returns all three unchanged; [`DefStore::load`] refuses them with
//! [`DefError::NotADefinition`].
//!
//! **Engine-native classes.** An object whose class no installed package exports (`Engine.StaticMesh`, whose objects
//! every `StaticMeshComponent` names) has no layout in the packages. Under `ParityMode::Reference` the decoder makes
//! its C++ stand-in, of a class defined at the class's path with no properties under `Core.Object` and marked native
//! (decision 2026-11-15-08). Under `ParityMode::Off` the decoder refuses such a reference (`NotYet`, the property
//! skipped), so the store makes the handle itself before the decoder sees the reference: a stand-in named after the
//! object, of the same kind of class (defined once at the path the export's class import or the import names, no
//! properties, under `Core.Object`, marked native so no `IsA` answer through it passes for the game's), with
//! `Origin::Resource` of the object. It is the store's object for the object's path like any other (identity by path,
//! the record being `tools/prepare_level.py`, which resolves a component's `StaticMesh` reference by its path), and it
//! stays a stand-in: it never loads, as its tags cannot be typed. A class something else defined at that path (an
//! overlay, a native pack) is left to the decoder. Decision 2026-11-16-55; UNVERIFIED as anything of the game.
//!
//! **Archetypes.** Under `ParityMode::Reference` a loaded definition starts from its class default object, as the C++
//! `instantiateExport` does: the C++ never follows the export table's archetype reference. Under `ParityMode::Off` it
//! starts from its archetype's values instead, then takes its own tags. The archetype is the path the export's
//! archetype reference names, its holder chosen by the identity policy. Its values are, in turn, its own archetype's
//! values (or its class default object's, for none) with its own tags from its derived prefix applied; a class's
//! default object as archetype gives its class default object. The archetype's class must be the export's or one it
//! derives from (its slots are then the first of the export's). An archetype no holder has, of another class, in a
//! cycle or more than 16 deep is a `NotYet` for the load, never class defaults in its place. An archetype inside a
//! class default object (a template, such as `Engine.Default__StaticMeshActor`'s component, the archetype of most
//! placed components) is followed like any other, read from its derived prefix (below, "Subobjects of class default
//! objects"; decision 2026-11-16-70, which replaced the logged skip of decision 2026-11-16-55). Each archetype of a
//! chain is built as its own decode load, so the decoder's resolve budget counts per step, not per chain (a chain has
//! at most 17 steps, each within the budget). The archetype's values are copied as they are: a reference to one of
//! the archetype's own subobjects (a pawn archetype's `DialogComponent`) is shared by every object made from it, not
//! made per object (UNVERIFIED against the game, which instances such subobjects). Decision 2026-11-16-55;
//! UNVERIFIED in the running game.
//!
//! **Subobjects of class default objects.** Under `ParityMode::Off` an object inside a class default object (a path
//! with a `Default__` segment before its last) is a definition like any other: it loads from its holder, from its
//! derived prefix, with its archetype applied. Under `ParityMode::Reference` it stays a stand-in, as before: the C++
//! reads no such object (its class default object applies the `Default__<Class>` export's tags only). The class
//! default object itself is unchanged in both modes. Decision 2026-11-16-70; UNVERIFIED in the running game.
//!
//! **Prefix by class.** Where an export's tags start is derived from the export itself, so the order loads happen in
//! cannot change a decode:
//! - an export whose class derives from `Core.Component` and whose chain of outer exports reaches a class default
//!   object (a template) reads from 16 (`PropertyPrefix::Template`) when it fits the layout decision 2026-11-16-70
//!   recorded from every installed template (a zero word, its own name at 4, its tags from 16 ending at the export's
//!   end or one zero word before it; 599 of 602 do, the three dominant light templates do not), else `NotYet`;
//! - an export whose class path contains `CollectionActor` (`Engine.StaticMeshCollectionActor`,
//!   `Engine.StaticLightCollectionActor`) reads from 4 (`PropertyPrefix::Ordinary`) without a check, wherever it is:
//!   the C++ `--scene-records` rule (`src/cli.cpp`) and decision 2026-09-10-09. Its untagged tail lists its
//!   components' entries ([`DefStore::collection`]). This rule goes by the class's name, as the C++ does, while
//!   [`DefStore::collection`] goes by inheritance from the two engine classes: a class named so that derives from
//!   neither (`Engine.BlockingMeshCollectionActor`) loads from 4 and `collection` refuses it, and a subclass whose
//!   name does not contain `CollectionActor` would take the actor rules below (none is installed);
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
//! its first use (about 6.6 s for the 934 files of the installed `CookedPCConsole`, measured locally; since lane
//! FIX-SCENE-LOAD the store reads them on worker threads, about 1 s for Sanctuary's load). Holder answers are cached
//! per path. A candidate file a lookup reads is kept only as a list of its export paths; only holders' files stay
//! loaded in the package store. Since lane FIX-SCENE-LOAD (decision 2026-11-16-68) a path's candidate list is asked of
//! the package store once per first segment, the first lookup of a segment reads all its candidates' export paths
//! together on worker threads, a holder file read that way is handed to the package store (`PackageStore::adopt`)
//! instead of being decoded again, and a loaded definition's slots move into its handle without a walk over the heap
//! (`ObjectSpace::free_newest`). The answers are unchanged: the same holders, ids and objects.
//!
//! Status: implemented (W3-DEFS, decision 2026-11-15-16). Identity, the holder rules, the prefix rule and the overlay
//! formats are this project's conventions or the tools', UNVERIFIED as rules of the game; the decode is W3-DECODE's.

use std::cell::RefCell;
use std::fmt;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use ow_container::ContainerError;
use ow_core::{FixedHashMap, FixedHashSet, NotYet, ObjectPath, ParityMode};
use ow_object::cdo;
use ow_object::decode::{DecodeError, DecodeState, ReferenceResolver, ValueCx};
use ow_object::heap::{HeapError, ObjectSpace, Origin};
use ow_object::ids::{ClassId, ObjRef, PropAddr, PropId, StructId};
use ow_object::prefix::PropertyPrefix;
use ow_object::reflect::{ClassKey, CodeIndex, ReflectError};
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

/// The record of the collection actors, the engine-native stand-ins in `ParityMode::Off` and the archetypes.
pub const COLLECTION_RECORD: &str =
    "docs/decisions/2026-11/2026-11-16-55-rust-definition-store-collection-actors-engine-native.md";

/// The text the C++ `--scene-records` prefix rule looks for in an export's class path (`src/cli.cpp`).
const COLLECTION_ACTOR: &str = "CollectionActor";

/// The record of the subobjects of class default objects and their 16-byte prefix.
pub const CDO_RECORD: &str = "docs/decisions/2026-11/2026-11-16-70-rust-definition-store-subobjects-of-class-default-objects.md";

/// How many archetypes deep a load follows (module documentation, "Archetypes"); deeper is a `NotYet`.
const MAX_ARCHETYPE_DEPTH: usize = 16;

/// What [`DefStore::load_export_of_file`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileCopy {
    /// The file holds no export at the object's path, or the object is not a definition of this store: nothing done.
    NotInFile,
    /// The object is the file's own export at its path, loaded (now or before).
    Loaded,
}

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
    /// A collection actor whose component array or tail is outside the recorded layout ([`DefStore::collection`]).
    Collection {
        object: String,
        reason: String,
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
            DefError::Collection { object, reason } => {
                write!(f, "collection actor {object}: {reason}")
            }
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
        /// The path the handle was made for, kept for messages (review r2 of #616).
        path: String,
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
    /// How many handles went from unloaded to loaded or to failed ([`DefStore::load_transitions`]).
    transitions: u64,
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
    /// The global index's candidate files by a path's first segment ([`candidate_segment`]): asked of the package
    /// store once per segment (lane FIX-SCENE-LOAD; the store builds a new list on every call).
    candidates: FixedHashMap<String, Rc<[PathBuf]>>,
    /// A holder's file -> its package store id, once loaded (the store keeps every package it loads, so the id holds
    /// for the store's life).
    file_ids: FixedHashMap<PathBuf, PackageId>,
    /// Packages [`Handles::prefetch`] read that hold the path it was asked for, kept until the lookup that asked
    /// hands them to the package store or drops them ([`Handles::release`]).
    held: FixedHashMap<PathBuf, Arc<Package>>,
    /// Paths whose `defs: no holder` line was logged.
    no_holder_logged: FixedHashSet<String>,
}

impl Handles {
    /// The files `PackageStore::global_candidates` gives for `path`, asked once per first segment: the store's list
    /// depends on the segment alone, and its global index stays as built while the tree is not scanned again (the
    /// caches here already assume that).
    fn candidates(&mut self, store: &mut PackageStore, path: &str) -> Rc<[PathBuf]> {
        let segment = candidate_segment(path);
        if let Some(files) = self.candidates.get(&segment) {
            return Rc::clone(files);
        }
        let files: Rc<[PathBuf]> = store.global_candidates(path).into();
        self.candidates.insert(segment, Rc::clone(&files));
        self.prefetch(&files, path);
        files
    }

    /// Reads the export paths of every file of `files` not read yet, on worker threads (at most [`READ_THREADS`]),
    /// as [`Handles::file_index`] would read each one; keeps the package of each that holds `path`. A lookup walks a
    /// segment's candidates in scan order and reads them one by one; reading them together gives each file the same
    /// index (a pure function of its bytes), only sooner, and possibly for a file the walk would not have reached.
    fn prefetch(&mut self, files: &[PathBuf], path: &str) {
        let todo: Vec<&PathBuf> = files
            .iter()
            .filter(|f| !self.files.contains_key(*f))
            .collect();
        if todo.len() < 2 {
            return;
        }
        let read = parallel_map(&todo, |file| match FileIndex::read_with_package(file) {
            Ok((index, package)) => {
                let holds = matches!(index.find(path), Ok(Some(_)));
                (Ok(index), holds.then_some(package))
            }
            Err(e) => (Err(e), None),
        });
        for (file, (index, package)) in todo.into_iter().zip(read) {
            self.files.insert(file.clone(), index.map(Rc::new));
            if let Some(package) = package {
                self.held.insert(file.clone(), package);
            }
        }
    }

    /// Drops the packages [`Handles::prefetch`] kept that no lookup handed to the package store.
    fn release(&mut self) {
        self.held.clear();
    }

    /// The export paths of `file`, read on first use; with the package itself when this call read it, for
    /// [`Handles::file_id`] to keep.
    fn file_index(
        &mut self,
        file: &Path,
    ) -> (Result<Rc<FileIndex>, PackageError>, Option<Arc<Package>>) {
        if let Some(index) = self.files.get(file) {
            return (index.clone(), self.held.remove(file));
        }
        let (index, package) = match FileIndex::read_with_package(file) {
            Ok((index, package)) => (Ok(Rc::new(index)), Some(package)),
            Err(e) => (Err(e), None),
        };
        self.files.insert(file.to_path_buf(), index.clone());
        (index, package)
    }

    /// The package store id of holder file `file`: the store's own load of it (`PackageStore::load_path`), or, when
    /// [`Handles::file_index`] has just read it, that package handed to the store (`PackageStore::adopt`, which keys
    /// it by the same normalised path and gives the id `load_path` would), so that the file is decoded once.
    fn file_id(
        &mut self,
        store: &mut PackageStore,
        file: &Path,
        fresh: Option<Arc<Package>>,
    ) -> Result<PackageId, StoreError> {
        if let Some(&id) = self.file_ids.get(file) {
            return Ok(id);
        }
        let id = match fresh {
            Some(package) => store.adopt(package)?,
            None => store.load_path(file)?,
        };
        self.file_ids.insert(file.to_path_buf(), id);
        Ok(id)
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
                let o = match self.native_stand_in(cx, package, reference)? {
                    Some(o) => o,
                    None => cx
                        .resource(package, reference)?
                        .ok_or(DecodeError::UnsupportedRequest)?,
                };
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
        let o = match self.native_stand_in(cx, target.0, target.1)? {
            Some(o) => o,
            None => cx
                .resource(target.0, target.1)?
                .ok_or(DecodeError::UnsupportedRequest)?,
        };
        let why = if is_class_default(&path) && !loads_inside_class_default(cx.mode, &path) {
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

    /// `ParityMode::Off` only: the store's handle for object `index` of `package` (an export, or an import no holder
    /// has) when its class is one no installed package exports, such as `Engine.StaticMesh` (module documentation,
    /// "Engine-native classes"). `None` in `ParityMode::Reference` (the decoder's C++ stand-in answers there), for an
    /// export whose class is a class object's or in its own package, for a code class (`Function`, `State`,
    /// `Package`, `Enum`, a property class and the rest of `is_code_class`: the decoder handles references to code as
    /// before), and for a class an installed package exports or something else defined (the decoder answers those
    /// too).
    fn native_stand_in(
        &self,
        cx: &mut ValueCx<'_>,
        package: PackageId,
        index: i32,
    ) -> Result<Option<ObjRef>, DecodeError> {
        if cx.mode == ParityMode::Reference {
            return Ok(None);
        }
        let pkg = cx.store.shared(package);
        let entry = pkg.object(index)?.clone();
        // The class path the decoder would fail to find (`ValueCx::object_class`): an export's class import that
        // resolves to no class, or an import's recorded class.
        let class_path = if index > 0 {
            if entry.class >= 0 || cx.code.class_at(cx.store, package, entry.class)?.is_some() {
                return Ok(None);
            }
            pkg.path_of(entry.class)?
        } else {
            let path = format!("{}.{}", entry.class_package, entry.class_name);
            match cx.code.find_class(cx.store, &path) {
                Ok(_) => return Ok(None),
                Err(ReflectError::ClassNotFound(_))
                | Err(ReflectError::Store(StoreError::PackageNotFound(_))) => path,
                Err(e) => return Err(e.into()),
            }
        };
        // Code is reflection's, not an engine-native object: left to the decoder, as before this rule.
        if is_code_class(class_path.rsplit('.').next().unwrap_or_default()) {
            return Ok(None);
        }
        let classes = &mut cx.space.classes;
        let class = match classes.by_path(&class_path) {
            Some(id) if classes.get_layout(id).is_some_and(|l| l.is_defined()) => {
                if !classes.is_native(id) {
                    return Ok(None);
                }
                id
            }
            _ => {
                let object = classes
                    .by_path("Core.Object")
                    .filter(|&o| classes.get_layout(o).is_some_and(|l| l.is_defined()))
                    .ok_or_else(|| DecodeError::NoClass {
                        object: class_path.clone(),
                    })?;
                let id = classes.define(&class_path, Some(object), Vec::new())?;
                classes.mark_native(id)?;
                id
            }
        };
        let origin = Origin::Resource {
            package: cx.code.filed_id(package),
            index,
        };
        Ok(Some(cx.space.alloc_stand_in(class, &entry.name, origin)?))
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
                let files = self.handles.borrow_mut().candidates(cx.store, path);
                for file in files.iter() {
                    // As the store's global fallback: a file that fails to load or whose search fails is no holder.
                    let (index, fresh) = self.handles.borrow_mut().file_index(file);
                    let found = index.and_then(|index| index.find(path));
                    if let Ok(Some(export)) = found {
                        // Only the holder's file is loaded: its definition is decoded from it.
                        let id = self.handles.borrow_mut().file_id(cx.store, file, fresh);
                        self.handles.borrow_mut().release();
                        answer = cx.code.resolve_ref(cx.store, id?, export);
                        break;
                    }
                }
                self.handles.borrow_mut().release();
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

/// The key `PackageStore::global_candidates` files a path under: its first `.`-separated segment, ASCII case folded
/// (the store's `first_segment`). [`Handles::candidates`] caches by it; a test checks the two agree.
fn candidate_segment(path: &str) -> String {
    path.split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
}

/// The most worker threads [`Handles::prefetch`] uses.
const READ_THREADS: usize = 16;

/// `f` of each item, computed on worker threads (at most [`READ_THREADS`]), in the items' order.
fn parallel_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .clamp(1, READ_THREADS);
    let per_thread = items.len().div_ceil(threads).max(1);
    let f = &f;
    std::thread::scope(|scope| {
        let workers: Vec<_> = items
            .chunks(per_thread)
            .map(|chunk| scope.spawn(move || chunk.iter().map(f).collect::<Vec<R>>()))
            .collect();
        workers
            .into_iter()
            .flat_map(|w| w.join().unwrap_or_else(|e| std::panic::resume_unwind(e)))
            .collect()
    })
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
            Some(Entry::Failed { error, .. }) => Err(error),
            Some(Entry::Unloaded { path, target }) => match self.load_unloaded(o, &path, target) {
                Ok(()) => Ok(o),
                Err(error) => {
                    let mut handles = self.linker.handles.borrow_mut();
                    handles.prune(self.cx.space);
                    if self.cx.space.heap.contains(o) {
                        handles.transitions += 1;
                        handles.entries.insert(
                            o,
                            Entry::Failed {
                                path: path.clone(),
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
        let (made, tags_end) = self.instantiate(package, index, prefix)?;
        // `made` is new and nothing older refers to it: its slots move into the handle without a walk over the heap
        // (`ObjectSpace::free_newest`; lane FIX-SCENE-LOAD).
        let built = self.cx.space.free_newest(made)?;
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
            && tags_end <= size
            && start
                .checked_add(size)
                .is_some_and(|end| end <= pkg.data.len())
        {
            self.tails
                .spans
                .insert(o, (package, start + tags_end..start + size));
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
        {
            let mut handles = self.linker.handles.borrow_mut();
            handles.transitions += 1;
            handles.entries.insert(o, Entry::Loaded { class });
        }
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
        // A collection actor reads from 4 wherever it is, without a check: the C++ scene records' rule (`src/cli.cpp`
        // `--scene-records`, a class path naming `CollectionActor`), decision 2026-09-10-09.
        if self.cx.code.class(class).path.contains(COLLECTION_ACTOR) {
            return Ok(PropertyPrefix::Ordinary);
        }
        let chain: Vec<String> = self
            .cx
            .code
            .super_chain(class)
            .map(|k| self.cx.code.class(k).path.to_ascii_lowercase())
            .collect();
        let has = |p: &str| chain.iter().any(|c| c == p);
        // A component inside a class default object (a template): 16 bytes, under the structural check of decision
        // 2026-11-16-70 (`template_fits`). The classes whose templates carry more before their tags (the dominant
        // light components) fail it and are `NotYet`.
        if has("core.component") && self.outer_chain_reaches_class_default(&pkg, entry.outer) {
            return if template_fits(&pkg, &entry) {
                Ok(PropertyPrefix::Template)
            } else {
                Err(DefError::NotYet(NotYet::not_read(
                    format!(
                        "the property prefix of {path}: a component inside a class default object whose bytes do not \
                         fit the recorded 16-byte template layout"
                    ),
                    CDO_RECORD,
                )))
            };
        }
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

    /// A new object made from export `index` of `package`, and where its tags ended (from the payload start): the
    /// decoder's `cdo::instantiate_export` (class defaults, then the export's tags from `prefix`) in
    /// `ParityMode::Reference` and for an export with no archetype to apply; otherwise, in `ParityMode::Off`, the
    /// archetype's values, then the export's tags (module documentation, "Archetypes").
    fn instantiate(
        &mut self,
        package: PackageId,
        index: i32,
        prefix: PropertyPrefix,
    ) -> Result<(ObjRef, usize), DefError> {
        let base = if self.cx.mode == ParityMode::Reference {
            None
        } else {
            self.archetype_slots(package, index, &mut Vec::new())?
        };
        match base {
            None => {
                let made = cdo::instantiate_export(self.cx, package, index, prefix)?;
                Ok((made.object, made.tags_end))
            }
            Some(base) => self.build_over(package, index, prefix, base),
        }
    }

    /// `ParityMode::Off`: the slots of export `index` of `package`'s archetype, with that archetype's own archetype
    /// applied first, and so on; `None` where the class default object is the base (no archetype, or the class
    /// default object of the export's own class). `chain` holds the archetypes already followed, for the cycle guard
    /// and the depth budget. Every other archetype that cannot be followed is a `NotYet`, never class defaults
    /// instead.
    fn archetype_slots(
        &mut self,
        package: PackageId,
        index: i32,
        chain: &mut Vec<(PackageId, i32)>,
    ) -> Result<Option<Vec<Value>>, DefError> {
        let pkg = self.cx.store.shared(package);
        let entry = pkg.object(index).map_err(package_error)?.clone();
        if entry.archetype == 0 {
            return Ok(None);
        }
        let path = full_path(&pkg, entry.archetype).map_err(package_error)?;
        let not_yet = |why: String| {
            DefError::NotYet(NotYet::not_read(
                format!("the archetype {path} of {}: {why}", entry.name),
                COLLECTION_RECORD,
            ))
        };
        let Some(class) = self.cx.code.class_at(self.cx.store, package, entry.class)? else {
            return Err(DecodeError::NullClass.into());
        };
        let key = path_key(&path);
        let Some(target) = self.linker.holder_of(self.cx, &path, &key)? else {
            return Err(not_yet("no package holds it".into()));
        };
        if chain.contains(&target) {
            return Err(not_yet("its archetype chain is a cycle".into()));
        }
        if chain.len() >= MAX_ARCHETYPE_DEPTH {
            return Err(not_yet(format!(
                "more than {MAX_ARCHETYPE_DEPTH} archetypes deep"
            )));
        }
        chain.push(target);
        let arch_pkg = self.cx.store.shared(target.0);
        let arch_entry = arch_pkg.object(target.1).map_err(package_error)?.clone();
        let Some(arch_class) = self
            .cx
            .code
            .class_at(self.cx.store, target.0, arch_entry.class)?
        else {
            return Err(not_yet("its class is not in an installed package".into()));
        };
        // Its slots are a prefix of the export's only when its class is the export's or one it derives from.
        if !self.cx.code.super_chain(class).any(|k| k == arch_class) {
            return Err(not_yet(format!(
                "its class {} is not {} or a class it derives from",
                self.cx.code.class(arch_class).path,
                self.cx.code.class(class).path
            )));
        }
        // A class's default object (a template inside one has another name): the class default object.
        if arch_entry.name.starts_with("Default__") {
            if arch_class == class {
                return Ok(None);
            }
            let defaults = cdo::class_default(self.cx, arch_class)?;
            return Ok(Some(self.cx.space.heap.get(defaults)?.slots.clone()));
        }
        let prefix = self.derive_prefix(target.0, target.1, &path)?;
        let base = match self.archetype_slots(target.0, target.1, chain)? {
            Some(base) => base,
            None => {
                let defaults = cdo::class_default(self.cx, arch_class)?;
                self.cx.space.heap.get(defaults)?.slots.clone()
            }
        };
        let (made, _) = self.build_over(target.0, target.1, prefix, base)?;
        let slots = self.cx.space.free_newest(made)?.slots;
        Ok(Some(slots))
    }

    /// A new object of export `index`'s class: its class default object's slots, `base` over the first of them (an
    /// archetype's slots, of the class or a class it derives from), then the export's tags from `prefix`.
    fn build_over(
        &mut self,
        package: PackageId,
        index: i32,
        prefix: PropertyPrefix,
        base: Vec<Value>,
    ) -> Result<(ObjRef, usize), DefError> {
        let pkg = self.cx.store.shared(package);
        let entry = pkg.object(index).map_err(package_error)?.clone();
        let made = self.cx.load(|cx| {
            let class = cx
                .code
                .class_at(cx.store, package, entry.class)?
                .ok_or(DecodeError::NullClass)?;
            let defaults = cdo::class_default(cx, class)?;
            let id = cx.layout(class)?;
            let mut slots = cx.space.heap.get(defaults)?.slots.clone();
            let Some(head) = slots.get_mut(..base.len()) else {
                return Err(DecodeError::NotYet(NotYet::not_read(
                    format!(
                        "{}: an archetype of {} slots for a class of {}",
                        entry.name,
                        base.len(),
                        slots.len()
                    ),
                    COLLECTION_RECORD,
                )));
            };
            head.clone_from_slice(&base);
            let object =
                cx.space
                    .alloc(id, &entry.name, None, Origin::Export { package, index })?;
            cx.space.heap.get_mut(object)?.slots = slots;
            match cx.apply_tags(object, class, package, index, prefix.bytes()) {
                Ok(end) => Ok((object, end)),
                Err(error) => {
                    // Nothing refers to the new object yet; freeing it only gives its slot back.
                    let _ = cx.space.free(object);
                    Err(error)
                }
            }
        })?;
        Ok(made)
    }

    /// Whether the export chain of outers from `outer` (an export reference of `package`) reaches a class default
    /// object (an export named `Default__...`). Bounded as [`Loader::outer_chain_reaches_level`].
    fn outer_chain_reaches_class_default(&self, pkg: &Package, outer: i32) -> bool {
        let mut at = outer;
        for _ in 0..pkg.exports.len() {
            if at <= 0 {
                return false;
            }
            match pkg.object(at) {
                Ok(e) if e.name.starts_with("Default__") => return true,
                Ok(e) => at = e.outer,
                Err(_) => return false,
            }
        }
        false
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

    /// The class of the export the policy picks for `text`, without loading that export (only the class's code):
    /// `Ok(None)` where no holder exists or the class reference resolves to nothing.
    fn holder_class(&mut self, text: &str) -> Result<Option<ClassKey>, DefError> {
        let key = path_key(text);
        let Some((package, index)) = self.linker.holder_of(self.cx, text, &key)? else {
            return Ok(None);
        };
        let pkg = self.cx.store.shared(package);
        let class_ref = pkg.object(index).map_err(package_error)?.class;
        Ok(self.cx.code.class_at(self.cx.store, package, class_ref)?)
    }

    fn find(&mut self, text: &str) -> Result<Option<ObjRef>, DefError> {
        let key = path_key(text);
        if is_class_default(text) && !loads_inside_class_default(self.cx.mode, text) {
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

/// Whether `path` names an object inside a class default object (a segment before its last starts `Default__`) that
/// is not itself a class default object (its last segment does not).
fn inside_class_default(path: &str) -> bool {
    let dotted_path = dotted(path);
    let segments: Vec<&str> = dotted_path.split('.').collect();
    segments.split_last().is_some_and(|(last, outers)| {
        !last.starts_with("Default__") && outers.iter().any(|s| s.starts_with("Default__"))
    })
}

/// Whether the store loads the object at `path` although it is inside a class default object: in `ParityMode::Off`
/// only (module documentation, "Subobjects of class default objects"); the C++ reads no such object.
fn loads_inside_class_default(mode: ParityMode, path: &str) -> bool {
    mode != ParityMode::Reference && inside_class_default(path)
}

/// Whether export `entry` fits the template layout of decision 2026-11-16-70 (a census of every installed template:
/// 599 of 602 fit, the 3 dominant light templates do not; UNVERIFIED as the engine's format): at least 16 bytes, a
/// zero word at 0, a name at 4 equal to the export's own name, its tag stream from 16 ending at its `None`, and after
/// it either nothing or exactly one 4-byte word of 0 (the 89 templates of six classes that carry a native tail when
/// placed).
fn template_fits(pkg: &Package, entry: &ObjectEntry) -> bool {
    let (Ok(start), Ok(size)) = (usize::try_from(entry.offset), usize::try_from(entry.size)) else {
        return false;
    };
    let Some(payload) = start.checked_add(size).and_then(|end| pkg.data.get(start..end)) else {
        return false;
    };
    if payload.len() < PropertyPrefix::Template.bytes() {
        return false;
    }
    let mut r = Reader::new(payload);
    let header = r.i32().is_ok_and(|w| w == 0)
        && tables::read_name(&mut r, &pkg.names).is_ok_and(|n| n == entry.name);
    if !header {
        return false;
    }
    match tags_end(pkg, entry, PropertyPrefix::Template.bytes()) {
        Some(end) if end == size => true,
        Some(end) => size - end == 4 && payload[end..] == [0, 0, 0, 0],
        None => false,
    }
}

/// Where export `entry`'s tag stream from `prefix` bytes in ends (just past its `None` tag, from the payload start),
/// when it reaches its `None` inside the export's serial data.
fn tags_end(pkg: &Package, entry: &ObjectEntry, prefix: usize) -> Option<usize> {
    walk_tags(pkg, entry, prefix).map(|(_, end)| end)
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
    walk_tags(pkg, entry, prefix).map(|(names, _)| names)
}

/// The tag names and where the stream ends (`tag_names`, `tags_end`).
fn walk_tags(pkg: &Package, entry: &ObjectEntry, prefix: usize) -> Option<(Vec<String>, usize)> {
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
            return Some((names, r.pos()));
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

    /// The class of the export at `path` that [`DefStore::find`] would load (the holder the policy picks), without
    /// loading that export: only the class's code is loaded. `Ok(None)` where no holder exists, or where its class
    /// reference resolves to nothing. Compare it with [`CodeIndex::key_of`] of a class (lane FIX-PLAYER-CLASS-B).
    pub fn holder_class(&mut self, w: &mut ObjectSpace, path: &ObjectPath) -> Result<Option<ClassKey>, DefError> {
        self.run(w, |l| l.holder_class(path.as_str()))
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

    /// How many handles have gone from unloaded to loaded or to failed: a count that only grows (the hub's ruling in
    /// review r2 of #518). A retry of a handle whose load already failed returns the cached error and does not move it.
    pub fn load_transitions(&self) -> u64 {
        self.handles.borrow().transitions
    }

    /// Whether `obj` is a definition of this store that is loaded.
    pub fn is_loaded(&self, w: &ObjectSpace, obj: ObjRef) -> bool {
        w.heap.contains(obj)
            && matches!(
                self.handles.borrow().entries.get(&obj),
                Some(Entry::Loaded { .. })
            )
    }

    /// The object path an unloaded handle (one whose load failed included; review r2 of #616), or a stand-in that stays
    /// one, was made for, read from the store's own record
    /// without loading anything; None for any other object. For messages (review r1 of #615: refusal texts name the
    /// full path, and an unloaded handle has no outer in the object space yet).
    pub fn handle_path(&self, obj: ObjRef) -> Option<String> {
        match self.handles.borrow().entries.get(&obj) {
            Some(Entry::Unloaded { path, .. } | Entry::Failed { path, .. } | Entry::Stays { path, .. }) => {
                Some(path.clone())
            }
            _ => None,
        }
    }

    /// The package and export an unloaded handle will load from (lane LINK-8-STARTUP-MATERIAL-PRELOAD, review of
    /// #712): the stem of the package file that holds its export and the export's index, read from the store's own
    /// record without loading anything. None for any other object, and for a package whose stem is not a valid name.
    pub fn handle_source(&self, obj: ObjRef) -> Option<(String, i32)> {
        match self.handles.borrow().entries.get(&obj) {
            Some(Entry::Unloaded { target: (id, index), .. }) => {
                let name = self.packages.get(*id).package_name().ok()?;
                Some((name.as_str().to_string(), *index))
            }
            _ => None,
        }
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

    /// Whether `obj` is a handle whose load already failed: [`DefStore::ensure_loaded`] returns that error again and
    /// changes nothing (one of the [`DefStore::is_unloaded`] handles; review r2 of #480, L2).
    pub fn is_failed(&self, w: &ObjectSpace, obj: ObjRef) -> bool {
        w.heap.contains(obj) && matches!(self.handles.borrow().entries.get(&obj), Some(Entry::Failed { .. }))
    }

    /// `obj`, the store's object for a path, loaded from **file `file`'s own export at that path** when `file` holds
    /// one, in place of the export the identity policy picked (lane LINK1-HEAD-MESH, the hub's ruling; **UNVERIFIED**
    /// that the game resolves it so). It is for a reference followed out of an object loaded from `file`: a cooked
    /// package holds its own copy of what it references, and the game, which loads that file, has that copy. Only a
    /// handle not loaded yet is re-pointed. One already loaded from another file's copy is a typed
    /// `NotYet { NotRead }` naming both files, an identity conflict. A handle whose load failed returns that error
    /// again. Every other lookup (`find`, `resolve_ref`, `ensure_loaded`, `load`) keeps the policy's holder.
    pub fn load_export_of_file(&mut self, w: &mut ObjectSpace, obj: ObjRef, file: PackageId) -> Result<FileCopy, DefError> {
        let pkg = self.packages.shared(file);
        let entry = self.handles.borrow().entries.get(&obj).cloned();
        let (path, loaded_from) = match (&entry, w.heap.get(obj)?.origin) {
            (Some(Entry::Unloaded { path, .. } | Entry::Failed { path, .. }), _) => (path.clone(), None),
            (Some(Entry::Loaded { .. }), Origin::Export { package, index }) => {
                (full_path(&self.packages.shared(package), index).map_err(package_error)?, Some((package, index)))
            }
            _ => return Ok(FileCopy::NotInFile),
        };
        let key = path_key(&path);
        let mine = (1..=pkg.exports.len())
            .filter_map(|i| i32::try_from(i).ok())
            .find(|&i| full_path(&pkg, i).is_ok_and(|p| path_key(&p) == key));
        let Some(index) = mine else {
            return Ok(FileCopy::NotInFile);
        };
        let same_file = |a: PackageId| a == file || self.packages.get(a).source == pkg.source;
        match (entry, loaded_from) {
            (_, Some((package, i))) if same_file(package) && i == index => Ok(FileCopy::Loaded),
            (_, Some((package, _))) => Err(DefError::NotYet(NotYet::not_read(
                format!(
                    "the definition store: {path} is resident from {}, and {} holds its own copy (an identity conflict)",
                    self.packages.get(package).name,
                    pkg.name
                ),
                RECORD,
            ))),
            (Some(Entry::Unloaded { path, target }), None) => {
                if !(same_file(target.0) && target.1 == index) {
                    let mut handles = self.handles.borrow_mut();
                    handles.by_target.remove(&target);
                    handles.by_target.insert((file, index), obj);
                    handles.entries.insert(obj, Entry::Unloaded { path, target: (file, index) });
                }
                self.ensure_loaded(w, obj)?;
                Ok(FileCopy::Loaded)
            }
            _ => self.ensure_loaded(w, obj).map(|()| FileCopy::Loaded),
        }
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
        if is_class_default(&path) && !loads_inside_class_default(self.mode, &path) {
            return Err(DefError::NotADefinition(format!(
                "{path}: a class default object, or inside one"
            )));
        }
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

    /// A new object made from export `index` of `package` on every call (the C++ `Runtime::instantiateExport`, which
    /// builds a fresh object each time; the mission bridge's director and the slice's dummy pawn are made so): the
    /// export's class defaults (under `ParityMode::Off`, its archetype's values: module documentation, "Archetypes")
    /// and its tags, decoded from the export again, with every reference it decodes resolved
    /// through the store as a definition's are (the store's object for the path, loaded when read; module
    /// documentation, "Lazy loading"). Its origin is `Origin::Export` of `package` and `index`, its outer `None`.
    ///
    /// It is not one of the store's objects: the identity map does not hold it, [`DefStore::find`] never returns it,
    /// two calls give two objects, and writing to it changes no definition. No overlay is applied to it (the C++ has
    /// none). The export is the one given, not the path's holder under the policy (the C++ reads the package it was
    /// handed). `prefix` is checked against the prefix the store derives for the export, as [`DefStore::load_export`]
    /// checks it ([`DefError::PrefixMismatch`], or [`DefError::NotYet`] where none derives). An index that is not
    /// positive is the decoder's "unsupported exported object request" (the C++ text); a code export or a class
    /// default object is [`DefError::NotADefinition`].
    pub fn instance(
        &mut self,
        w: &mut ObjectSpace,
        package: PackageId,
        index: i32,
        prefix: PropertyPrefix,
    ) -> Result<ObjRef, DefError> {
        if index <= 0 {
            return Err(DecodeError::UnsupportedRequest.into());
        }
        let pkg = self.packages.shared(package);
        pkg.object(index).map_err(package_error)?;
        let path = full_path(&pkg, index).map_err(package_error)?;
        self.run(w, |l| {
            let class_name = l.cx.code.class_name(l.cx.store, package, index);
            if (is_class_default(&path) && !loads_inside_class_default(l.cx.mode, &path))
                || is_code_class(&class_name)
            {
                return Err(DefError::NotADefinition(path.clone()));
            }
            let derived = l.derive_prefix(package, index, &path)?;
            if derived != prefix {
                return Err(DefError::PrefixMismatch {
                    object: path.clone(),
                    derived,
                    given: prefix,
                });
            }
            let made = l.instantiate(package, index, prefix);
            if made.is_err() {
                // A failed decode frees what it allocated, handles included: forget those (as a failed load does).
                l.linker.handles.borrow_mut().prune(l.cx.space);
            }
            Ok(made?.0)
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
        let files = self
            .handles
            .borrow_mut()
            .candidates(&mut self.packages, &path);
        for file in files.iter() {
            let file = file.clone();
            let (index, fresh) = self.handles.borrow_mut().file_index(&file);
            let found = index.and_then(|index| index.find(&path));
            let export = match found {
                Err(e) => {
                    out.failures.push(FileFailure::new(file, package_error(e)));
                    continue;
                }
                Ok(Some(export)) => export,
                Ok(None) => continue,
            };
            let id = match self
                .handles
                .borrow_mut()
                .file_id(&mut self.packages, &file, fresh)
            {
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
        self.handles.borrow_mut().release();
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

    /// `read` of the default object of the class at `class_path`, **without publishing it** (lane IMPL-SETTINGS-FILL,
    /// review r1 of #464): a default object already built is read as it is; one not yet built is built for the read
    /// and then freed and dropped from the cache ([`ValueCx::load_discarded`](ow_object::decode::ValueCx::load_discarded)),
    /// so a later [`DefStore::class_default`] builds it again. For a preflight that must leave nothing behind.
    pub fn with_class_default<R>(
        &mut self,
        w: &mut ObjectSpace,
        class_path: &str,
        read: impl FnOnce(&ObjectSpace, ObjRef) -> R,
    ) -> Result<R, DefError> {
        // The resolver allocates stand-ins and records identity entries outside the load's own tracking (review r3 of
        // #464, item 2): the objects not in this snapshot are freed after the read, the identity entries of freed
        // objects pruned, and the no-holder log put back, so the read leaves the store as it was.
        let before: FixedHashSet<ObjRef> = w.heap.handles().into_iter().collect();
        let (logged, log_len) = (self.handles.borrow().no_holder_logged.clone(), self.state.log.len());
        let out = self.run(w, |l| {
            let key = l.cx.code.find_class(l.cx.store, class_path)?;
            Ok(l.cx.load_discarded(|cx| {
                let o = cdo::class_default(cx, key)?;
                Ok(read(cx.space, o))
            })?)
        });
        let fresh: Vec<ObjRef> = w.heap.handles().into_iter().filter(|o| !before.contains(o)).collect();
        if !fresh.is_empty() {
            w.free_many(&fresh).map_err(|e| DefError::from(ow_object::decode::DecodeError::Heap(e)))?;
        }
        let mut handles = self.handles.borrow_mut();
        handles.prune(w);
        handles.no_holder_logged = logged;
        self.state.log.truncate(log_len);
        out
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

mod collection;
pub use collection::{
    Collection, CollectionEntry, CollectionKind, CollectionPose, LIGHT_ENTRY_BYTES,
    MESH_ENTRY_BYTES,
};

#[cfg(test)]
mod tests;
