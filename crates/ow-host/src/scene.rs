//! Owner: lane H2-SCENE (host track, stage 1). A map's scene as plain, engine-agnostic data in the game's units (uu,
//! rotator units, Z-up): the static mesh placements of the persistent level and every level it streams, the meshes
//! they draw (decoded by `ow-assets`), the materials and textures those meshes name, the player starts, the lights
//! (kinds and counts only), and the terrain and BSP actors (named, not decoded: `ow-assets` has no reader for them).
//! Nothing here draws, converts to an engine frame (that is `frame.rs`, lane H3) or writes a file.
//!
//! **What is read, and from where.** Levels, actors and components go through the definition store
//! (`ow_defs::DefStore`): each export is loaded at the prefix the store derives (8 for a component, 26 for an actor
//! whose outer is a level, 4 for a level-streaming object), with its class default applied. One class cannot go
//! through the store yet: `Engine.StaticMeshCollectionActor`. Its tags start 4 bytes in (decision 2026-09-10-09, and
//! the C++ and Rust `--scene-records` prefix table), but the store's prefix rule (`ow_defs` `Loader::derive_prefix`,
//! decision 2026-11-16-36: an actor whose outer is a level reads from 26, unchecked) fails on it ("invalid FName").
//! Its tags are therefore walked here with `ow_package::props::read_tagged` from 4,
//! typed by `ow_object::decode::ReflectionTyper`, and its untagged tail is read as decision 2026-09-10-09 records it:
//! exactly 84 bytes per component reference, 16 floats of an affine rotation and translation matrix, 3 floats of
//! Scale3D, 1 float of scale and one int whose meaning is not assigned. Every number must be finite and the matrix
//! affine (the same checks as `tools/prepare_level.py`); a collection that fails them is a [`Gap`] and its
//! components are excluded, never placed at a guessed pose. A null entry is checked and dropped (the tool tolerates
//! one per level and stops on a second); a component two entries list is a [`Gap`] and is excluded (the tool stops
//! there).
//!
//! **Which components are placements.** The selection is `tools/prepare_level.py`'s (the Sanctuary manifest the
//! engine benchmark drew), so the two can be compared component by component:
//! - an export of class exactly `Engine.StaticMeshComponent` whose path in its file starts `TheWorld.PersistentLevel.`
//!   (archetype templates in cooked `GD_*` packages are not placements);
//! - with a `StaticMesh` reference (a component with none is [`Exclusion::NoStaticMesh`]);
//! - posed by its collection actor's tail when a collection lists it, else by its owner actor (the component's outer)
//!   when the owner's class is one of [`PLACED_OWNER_CLASSES`]: actor pose and component pose, kept apart;
//! - a mission pickup spawner's or failsafe's `PreviewMesh` component is an editor preview, excluded; any other owner
//!   is [`Exclusion::UnsupportedOwner`];
//! - an interactive object's second component identical to an earlier one (mesh, materials, pose and the seven
//!   flags of [`DUPLICATE_FLAGS`]) is excluded as a duplicate, as the tool does (the five Sanctuary vault symbols).
//!
//! Which of these the game actually draws is UNVERIFIED beyond what the tool's records say: the hidden flags are the
//! serialized `HiddenGame` of the component and `bHidden` of its owner ([`Placement::is_hidden`]), not Kismet,
//! Matinee or mission state, and none of the old UE5 host's helper-mesh rules (`hidden_visual_mesh`) is applied.
//!
//! **Poses are kept as read.** A placement's [`Pose`] holds the serialized values in uu and rotator units, the actor's
//! and the component's separately (or the collection's matrix): composing them into one world transform needs
//! rotator-to-matrix maths and is left to the consumer. Draw scales are not multiplied either.
//!
//! **Meshes, materials and textures.** A placement's mesh reference is read from the component's own tags, not from
//! the store's object: in `ParityMode::Off` the decoder skips a reference to an object of the engine-native class
//! `Engine.StaticMesh` (see `own_tags`). The decoder's own route, defining that class in the `ClassTable` before any
//! decode, would give the store's handle instead, resolved through its identity policy (`FirstInScanOrder`: the
//! first holder in scan order, slowly), not the reference the component names. An export reference is itself; an
//! import is an export of the scene's levels at its path and class (`tools/prepare_level.py` `Scene.resolve`,
//! narrowed; UNVERIFIED as the game's lookup), else what `PackageStore::resolve` gives, the C++ resolver the tool
//! calls. The mesh is decoded with `ow_assets::mesh::read_mesh` from 4 bytes in, as the tool runs
//! `--mesh`. Materials and textures go through the store (its identity policy picks their holder). A material's
//! textures are the `ParameterValue`s of its `TextureParameterValues` and of its parents', and the `Texture` of each
//! expression of the base `Material` (one level: material function calls are not followed). That is every texture
//! the material's tags name, not the one diffuse texture per material the benchmark loaded. Textures are only located
//! here; [`Scene::decode_texture`] decodes one on demand (all of Sanctuary's mips at once would take gigabytes).
//!
//! **Order.** Levels in streaming order (the persistent level, then each `LevelStreaming*` `PackageName` breadth
//! first, as the tool walks them); within a level, export order. Every map here is an [`OrderedMap`] filled in that
//! order, so iteration is deterministic and follows export index.
//!
//! Status: UNVERIFIED as the game's scene. What is verified is how the pieces read on synthetic packages (the tests)
//! and, on the installed game, the comparison with `tools/prepare_level.py` (decision 2026-11-16-46).

use std::collections::VecDeque;
use std::fmt;
use std::path::PathBuf;

use ow_assets::mesh::{MeshAsset, read_mesh};
use ow_assets::texture::{AssetError, TextureAsset, read_texture};
use ow_core::units::Rotator;
use ow_core::{DetMap, DetSet, NotYet, OrderedMap, PackageName};
use ow_defs::DefStore;
use ow_defs::store::DefError;
use ow_object::decode::ReflectionTyper;
use ow_object::heap::{HeapError, ObjectSpace, Origin};
use ow_object::ids::{ClassId, ObjRef, PropAddr};
use ow_object::prefix::PropertyPrefix;
use ow_object::reflect::ReflectError;
use ow_object::value::Value;
use ow_package::PackageId;
use ow_package::package::{Package, PackageError};
use ow_package::props::{self, NoArrayTypes, PropError, PropValue};
use ow_package::store::StoreError;

/// The record a `NotYet` of this module names.
pub const RECORD: &str = "docs/decisions/2026-11/2026-11-16-46-rust-host-scene-loading-stage-1-sanctuary-as-plain-data.md";

/// Owner classes whose `StaticMeshComponent`s are placed like a static mesh actor's (`tools/prepare_level.py`
/// `PLACED_OWNER_CLASSES`). Exact class paths, not subclasses.
pub const PLACED_OWNER_CLASSES: [&str; 4] = [
    "Engine.StaticMeshActor",
    "Engine.InterpActor",
    "GearboxFramework.WiringActor",
    "WillowGame.WillowInteractiveObject",
];

/// Owner classes whose component is the `PreviewMesh` of a mission item: an editor preview, not placed.
pub const PREVIEW_OWNER_CLASSES: [&str; 2] = [
    "WillowGame.WillowMissionPickupSpawner",
    "WillowGame.WillowMissionPickupFailsafe",
];

/// Owner classes whose actors may serialize the same component twice; the second exact copy is not placed.
pub const DEDUPLICATED_OWNER_CLASSES: [&str; 1] = ["WillowGame.WillowInteractiveObject"];

/// The component flags two copies must share to be one (`tools/prepare_level.py` `DUPLICATE_SIGNATURE_PROPERTIES`).
pub const DUPLICATE_FLAGS: [&str; 7] = [
    "HiddenGame",
    "CastShadow",
    "CollideActors",
    "BlockActors",
    "BlockRigidBody",
    "BlockZeroExtent",
    "BlockNonZeroExtent",
];

const STATIC_MESH_COMPONENT: &str = "Engine.StaticMeshComponent";
const MESH_COLLECTION: &str = "Engine.StaticMeshCollectionActor";
const LIGHT_COLLECTION: &str = "Engine.StaticLightCollectionActor";
const LEVEL_PREFIX: &str = "TheWorld.PersistentLevel.";
/// Bytes per component in a collection actor's tail (decision 2026-09-10-09).
const COLLECTION_ENTRY: usize = 84;
/// The tail's affine check tolerance (`tools/prepare_level.py` `collection_transforms`).
const AFFINE_TOLERANCE: f32 = 1e-5;
/// How many parents a material chain may have before it is a [`Gap`] (a cycle or a corrupt chain).
const MAX_MATERIAL_CHAIN: usize = 32;

/// Why a scene could not be loaded at all. A part that cannot be read is not an error: it is a [`Gap`] or an
/// [`Exclusion`] in the scene. This is the scene's error, not the host's: `ow-host` has no crate-wide error type.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum SceneError {
    /// The definition store failed outside one placement (a level-streaming object, a base-class lookup).
    Defs(DefError),
    /// Something the scene needs is not there yet: a store `NotYet` (an unread class layout, say) outside one
    /// placement.
    NotYet(NotYet),
    /// The object space refused a read (a stale handle).
    Heap(HeapError),
    /// A level package could not be loaded (the persistent one, or one a level names).
    Store(StoreError),
    /// A level package's tables could not be read.
    Package(PackageError),
    /// A level-streaming object's `PackageName` is not a package name.
    BadLevelName { level: String, name: String },
    /// A level-streaming object, player start or terrain actor loaded but lacks a property of the expected type.
    Unreadable { object: String, reason: String },
}

impl fmt::Display for SceneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SceneError::Defs(e) => write!(f, "{e}"),
            SceneError::NotYet(n) => write!(f, "{n}"),
            SceneError::Heap(e) => write!(f, "{e}"),
            SceneError::Store(e) => write!(f, "{e}"),
            SceneError::Package(e) => write!(f, "{e}"),
            SceneError::BadLevelName { level, name } => {
                write!(f, "{level} streams {name:?}, which is not a package name")
            }
            SceneError::Unreadable { object, reason } => write!(f, "{object}: {reason}"),
        }
    }
}

impl std::error::Error for SceneError {}

impl From<DefError> for SceneError {
    /// A store `NotYet` or heap error is the scene's own variant; any other store error is `Defs`.
    fn from(e: DefError) -> Self {
        match e {
            DefError::NotYet(n) => SceneError::NotYet(n),
            DefError::Heap(h) => SceneError::Heap(h),
            other => SceneError::Defs(other),
        }
    }
}

impl From<StoreError> for SceneError {
    fn from(e: StoreError) -> Self {
        SceneError::Store(e)
    }
}

impl From<PackageError> for SceneError {
    fn from(e: PackageError) -> Self {
        SceneError::Package(e)
    }
}

impl From<HeapError> for SceneError {
    fn from(e: HeapError) -> Self {
        SceneError::Heap(e)
    }
}

/// Where the asset decoders read from: the folder holding the install's texture file caches (`.tfc`), which is the
/// cooked package folder.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneAssets {
    pub tfc_root: PathBuf,
}

impl SceneAssets {
    pub fn new(tfc_root: impl Into<PathBuf>) -> Self {
        SceneAssets {
            tfc_root: tfc_root.into(),
        }
    }
}

/// An export of one of the scene's levels: the level's place in [`Scene::levels`] and the export's index in its file.
/// Ordered by level, then export index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExportKey {
    pub level: u32,
    pub export: i32,
}

/// One level of the scene.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneLevel {
    /// The level's package name as the streaming object (or the caller) gives it.
    pub name: String,
    pub package: PackageId,
}

/// An actor's serialized pose: `Location`, `Rotation`, `DrawScale`, `DrawScale3D`.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActorPose {
    /// uu.
    pub location: [f32; 3],
    pub rotation: Rotator,
    pub draw_scale: f32,
    pub draw_scale3d: [f32; 3],
}

/// A component's serialized pose relative to its owner: `Translation`, `Rotation`, `Scale`, `Scale3D`.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ComponentPose {
    /// uu.
    pub translation: [f32; 3],
    pub rotation: Rotator,
    pub scale: f32,
    pub scale3d: [f32; 3],
}

/// Which scale a collection placement uses: the component's own `Scale3D` and `Scale` when its tags set either,
/// otherwise the collection tail's copy (decision 2026-09-18-04: the running game applied the property where the two
/// disagreed).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleSource {
    Component,
    CollectionTail,
}

/// Where a placement is, as serialized.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pose {
    /// A component of an actor: the world pose is the actor's pose applied to the component's.
    Attached {
        actor: ActorPose,
        component: ComponentPose,
    },
    /// A component of a static mesh collection: the tail's rotation and translation matrix (16 floats as stored,
    /// translation in uu) and the scale `scale_source` names, as `scale3d` times `scale`.
    Collection {
        matrix: [f32; 16],
        scale3d: [f32; 3],
        scale: f32,
        scale_source: ScaleSource,
    },
}

/// The serialized hidden flags of a placement.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HiddenFlags {
    /// The component's `HiddenGame`.
    pub hidden_game: bool,
    /// The owner actor's (or collection actor's) `bHidden`.
    pub owner_hidden: bool,
}

/// A static mesh placement.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Placement {
    /// The component's path in its level's file.
    pub path: String,
    /// The class path of the actor that owns or poses the component.
    pub owner_class: String,
    /// The mesh's object path: a key of [`Scene::meshes`].
    pub mesh: String,
    pub pose: Pose,
    pub hidden: HiddenFlags,
    /// The component's `Materials`, one per section slot: the material's object path, or `None` where the slot is
    /// empty (the mesh's own section material applies).
    pub materials: Vec<Option<String>>,
}

impl Placement {
    /// Hidden by its serialized flags (component `HiddenGame` or owner `bHidden`). A mover's or interactive object's
    /// flag is only its saved state: Kismet, Matinee or mission state may change it in the game (UNVERIFIED here).
    pub fn is_hidden(&self) -> bool {
        self.hidden.hidden_game || self.hidden.owner_hidden
    }
}

/// Why a component that looked like a placement is not one.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum Exclusion {
    /// The component names no mesh.
    NoStaticMesh,
    /// The component is a mission pickup's editor preview.
    MissionItemPreview,
    /// A second identical component of the same interactive object.
    DuplicateComponent,
    /// The owner's class is not one this module places (its class path).
    UnsupportedOwner(String),
    /// The component's collection actor could not be read (the reason).
    CollectionUnreadable(String),
    /// The component or its owner could not be loaded or read (the error text).
    LoadFailed(String),
}

/// An excluded component, with why.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Excluded {
    pub path: String,
    pub reason: Exclusion,
}

/// Where an asset export lives: the file the store's identity policy chose for its path.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holder {
    pub package: PackageId,
    /// The holder file's stem.
    pub file: String,
    pub export: i32,
}

/// What `ow-assets` made of a mesh.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum MeshStatus {
    Decoded(MeshAsset),
    /// Not decodable yet (no holder; see [`Scene::gaps`]).
    NotYet(NotYet),
    /// The `ow-assets` reader refused the export (one of its checks).
    Refused(AssetError),
}

/// A mesh the scene draws.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct SceneMesh {
    pub holder: Option<Holder>,
    pub status: MeshStatus,
    /// LOD 0's section materials, by slot: object paths (`None` for a section with no material).
    pub section_materials: Vec<Option<String>>,
}

impl SceneMesh {
    /// LOD 0's triangle count, for a decoded mesh.
    pub fn lod0_triangles(&self) -> Option<u64> {
        match &self.status {
            MeshStatus::Decoded(m) => m.lods.first().map(|l| (l.indices.len() / 3) as u64),
            _ => None,
        }
    }
}

/// What the scene read of a material.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum MaterialStatus {
    Read,
    /// The material or one of its parents is of a class this module does not follow, or the chain is too long.
    NotYet(NotYet),
    /// The material or one of its parents could not be loaded (the error text).
    Failed(String),
}

/// A material a placement or a mesh section names.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct SceneMaterial {
    /// The material's class path.
    pub class: String,
    pub parent: Option<String>,
    /// Every texture the material and its parents name (see the module documentation), with the parameter or
    /// expression that named it: the material's own instance parameters first, then each parent's, then the base
    /// material's expressions. A texture named twice the same way is listed once.
    pub textures: Vec<MaterialTexture>,
    /// The base material's `TwoSided`; `None` where the chain reaches no base material whose class declares it.
    pub two_sided: Option<bool>,
    pub status: MaterialStatus,
}

/// Where a material named a texture.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureSource {
    /// A `TextureParameterValues` entry of the material or of a parent instance.
    Instance,
    /// The `Texture` of an expression of the base `Material`.
    Expression,
}

/// One texture a material names, and how.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialTexture {
    /// The texture's object path: a key of [`Scene::textures`].
    pub path: String,
    /// The `ParameterName` of the instance entry, or of the expression where it declares one (a texture sample
    /// parameter); `None` for an expression with no parameter name (or the name `None`).
    pub parameter: Option<String>,
    pub from: TextureSource,
    /// Where in the parent chain the entry was read: 0 for the material itself, 1 for its parent, and so on; an
    /// expression has its base material's depth. A consumer that wants the tool's override order (a child's
    /// parameter replaces its parent's of the same name) reads it from here.
    pub depth: u32,
}

/// A texture a scene material names. Decoded on demand by [`Scene::decode_texture`].
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneTexture {
    /// The texture's class path.
    pub class: String,
    /// `None`: no installed package exports it (a [`Gap`]).
    pub holder: Option<Holder>,
}

/// A player start actor (any subclass of `Engine.PlayerStart`).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerStart {
    pub path: String,
    pub class: String,
    pub pose: ActorPose,
}

/// How many light components of one class the levels hold.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LightCount {
    pub total: u32,
    /// Those held by a `StaticLightCollectionActor`.
    pub in_collections: u32,
}

/// A terrain actor (any subclass of `Engine.Terrain`), named but not decoded.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct TerrainActor {
    pub path: String,
    pub pose: ActorPose,
    /// How many `Engine.TerrainComponent` exports its file holds inside it.
    pub components: u32,
}

/// A level's BSP: its model exports, named but not decoded.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LevelModels {
    /// `Engine.Model` exports under the level.
    pub models: u32,
    /// `Engine.ModelComponent` exports under the level.
    pub components: u32,
}

/// What a [`Gap`] is about.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GapPart {
    Mesh,
    Material,
    Texture,
    Terrain,
    Bsp,
    Collection,
}

/// A part of the scene that is not there yet, with the object it concerns. `not_yet.what` is the same text for every
/// gap of one kind, so gaps can be counted by reason.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gap {
    pub part: GapPart,
    pub object: String,
    pub not_yet: NotYet,
}

/// A map's scene. See the module documentation.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    /// The persistent level's package name as given.
    pub map: String,
    /// Where [`Scene::decode_texture`] reads streamed mips from.
    pub assets: SceneAssets,
    pub levels: Vec<SceneLevel>,
    pub placements: OrderedMap<ExportKey, Placement>,
    pub excluded: OrderedMap<ExportKey, Excluded>,
    /// By object path, first-seen order.
    pub meshes: OrderedMap<String, SceneMesh>,
    pub materials: OrderedMap<String, SceneMaterial>,
    pub textures: OrderedMap<String, SceneTexture>,
    pub player_starts: OrderedMap<ExportKey, PlayerStart>,
    /// By light component class path, first-seen order.
    pub lights: OrderedMap<String, LightCount>,
    pub terrains: OrderedMap<ExportKey, TerrainActor>,
    /// By level index (into [`Scene::levels`]), for each level with any model export.
    pub models: OrderedMap<u32, LevelModels>,
    pub gaps: Vec<Gap>,
}

impl Scene {
    /// A scene with no level and nothing in it.
    pub(crate) fn empty(map: &str, assets: SceneAssets) -> Scene {
        Scene {
            map: map.to_string(),
            assets,
            levels: Vec::new(),
            placements: OrderedMap::new(),
            excluded: OrderedMap::new(),
            meshes: OrderedMap::new(),
            materials: OrderedMap::new(),
            textures: OrderedMap::new(),
            player_starts: OrderedMap::new(),
            lights: OrderedMap::new(),
            terrains: OrderedMap::new(),
            models: OrderedMap::new(),
            gaps: Vec::new(),
        }
    }

    /// Loads map `map` (a persistent level's package name) and every level it streams, through `defs` and `space`
    /// (the same object space on every call to the store). The scene keeps `assets` for [`Scene::decode_texture`];
    /// meshes need none of it. See the module documentation for what is read and how.
    pub fn load(
        defs: &mut DefStore,
        space: &mut ObjectSpace,
        assets: &SceneAssets,
        map: &PackageName,
    ) -> Result<Scene, SceneError> {
        let mut loader = Loader {
            defs,
            space,
            classes: DetMap::new(),
            bases: DetMap::new(),
            scene: Scene::empty(map.as_str(), assets.clone()),
            signatures: DetSet::new(),
            resolved: DetMap::new(),
            level_paths: DetMap::new(),
        };
        loader.levels(map)?;
        for level in 0..loader.scene.levels.len() {
            loader.level(level)?;
        }
        Ok(loader.scene)
    }

    /// Decodes texture `path` (a key of [`Scene::textures`]) with `ow_assets::texture::read_texture`, from 4 bytes in,
    /// its streamed mips from [`Scene::assets`]. A texture with no holder, or of a class outside [`TEXTURE_CLASSES`]
    /// (a `TextureCube`, say), is `NotYet`.
    ///
    /// `defs` must be the store the scene was loaded with: a holder's package id means nothing in another store,
    /// whose `PackageStore::get` panics on an id it did not hand out.
    pub fn decode_texture(
        &self,
        defs: &DefStore,
        path: &str,
    ) -> Result<TextureAsset, TextureError> {
        let texture = self
            .textures
            .get(path)
            .ok_or_else(|| TextureError::NotInScene(path.to_string()))?;
        let holder = texture
            .holder
            .as_ref()
            .ok_or_else(|| TextureError::NotYet(no_holder("Texture")))?;
        if !TEXTURE_CLASSES.contains(&texture.class.as_str()) {
            return Err(TextureError::NotYet(texture_class_not_yet()));
        }
        read_texture(
            defs.packages().get(holder.package),
            holder.export,
            // An ordinary object's prefix, as the tool passes it (`--property-offset 4`).
            PropertyPrefix::Ordinary.bytes(),
            &NoArrayTypes,
            &self.assets.tfc_root,
        )
        .map_err(TextureError::Refused)
    }

    /// The effective material of each LOD 0 section of placement `p`: the override at the section's slot where it
    /// has one, else the mesh's section material. Empty for a mesh that did not decode.
    ///
    /// The tool's rule differs for a mesh with a triangle-less section: `prepare_level.py` drops such sections first
    /// and then indexes `Materials` by the position in the shortened list, so every later section takes the override
    /// of the slot before its own. Here `Materials[i]` always overrides section `i`.
    pub fn effective_materials(&self, p: &Placement) -> Vec<Option<String>> {
        let Some(mesh) = self.meshes.get(p.mesh.as_str()) else {
            return Vec::new();
        };
        mesh.section_materials
            .iter()
            .enumerate()
            .map(|(slot, own)| match p.materials.get(slot) {
                Some(Some(over)) => Some(over.clone()),
                _ => own.clone(),
            })
            .collect()
    }

    /// The counts a report needs, over every placement and over the ones not hidden.
    pub fn summary(&self) -> SceneSummary {
        let mut s = SceneSummary::default();
        let mut meshes_drawn = DetSet::new();
        let mut textures_all = DetSet::new();
        let mut textures_drawn = DetSet::new();
        for p in self.placements.values() {
            s.placements = s.placements.saturating_add(1);
            let hidden = p.is_hidden();
            if hidden {
                s.hidden = s.hidden.saturating_add(1);
            } else {
                meshes_drawn.insert(p.mesh.clone());
            }
            let triangles = self
                .meshes
                .get(p.mesh.as_str())
                .and_then(SceneMesh::lod0_triangles)
                .unwrap_or(0);
            s.triangles_all = s.triangles_all.saturating_add(triangles);
            if !hidden {
                s.triangles_drawn = s.triangles_drawn.saturating_add(triangles);
            }
            for material in self.effective_materials(p).into_iter().flatten() {
                if let Some(m) = self.materials.get(material.as_str()) {
                    for t in &m.textures {
                        textures_all.insert(t.path.clone());
                        if !hidden {
                            textures_drawn.insert(t.path.clone());
                        }
                    }
                }
            }
        }
        s.meshes = self.meshes.len();
        s.meshes_drawn = meshes_drawn.len();
        for m in self.meshes.values() {
            match m.status {
                MeshStatus::Decoded(_) => s.meshes_decoded = s.meshes_decoded.saturating_add(1),
                MeshStatus::NotYet(_) => s.meshes_not_yet = s.meshes_not_yet.saturating_add(1),
                MeshStatus::Refused(_) => s.meshes_refused = s.meshes_refused.saturating_add(1),
            }
        }
        s.textures_named = self.textures.len();
        s.textures_all = textures_all.len();
        s.textures_drawn = textures_drawn.len();
        for g in &self.gaps {
            let n = s
                .gaps
                .entry((g.part, g.not_yet.what.to_string()))
                .or_insert(0);
            *n = n.saturating_add(1);
        }
        s
    }
}

/// The counts of [`Scene::summary`].
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SceneSummary {
    pub placements: usize,
    pub hidden: usize,
    /// Distinct meshes over every placement, and over the ones not hidden.
    pub meshes: usize,
    pub meshes_drawn: usize,
    pub meshes_decoded: usize,
    pub meshes_not_yet: usize,
    pub meshes_refused: usize,
    /// Every texture a scene material names, and those the effective materials of all (or of the not hidden)
    /// placements name.
    pub textures_named: usize,
    pub textures_all: usize,
    pub textures_drawn: usize,
    /// LOD 0 triangles summed over placements (a mesh that did not decode counts 0).
    pub triangles_all: u64,
    pub triangles_drawn: u64,
    /// Gaps by part and reason.
    pub gaps: DetMap<(GapPart, String), usize>,
}

/// Why [`Scene::decode_texture`] gave no texture.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextureError {
    NotInScene(String),
    NotYet(NotYet),
    Refused(AssetError),
}

impl fmt::Display for TextureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TextureError::NotInScene(p) => write!(f, "texture {p} is not in the scene"),
            TextureError::NotYet(n) => write!(f, "{n}"),
            TextureError::Refused(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for TextureError {}

/// The texture classes `ow_assets::texture::read_texture` decodes.
pub const TEXTURE_CLASSES: [&str; 2] = ["Engine.Texture2D", "Engine.TerrainWeightMapTexture"];

fn texture_class_not_yet() -> NotYet {
    NotYet::layer_absent(
        "a texture class ow-assets does not read (it reads Texture2D and TerrainWeightMapTexture)",
        "an ow-assets reader for the class",
        RECORD,
    )
}

fn no_holder(kind: &str) -> NotYet {
    NotYet::not_read(
        format!("{kind} export: no installed package holds its path"),
        RECORD,
    )
}

/// A collection actor's component, as its tail poses it.
#[derive(Debug, Clone, Copy)]
struct CollectionEntry {
    collection: i32,
    matrix: [f32; 16],
    scale3d: [f32; 3],
    scale: f32,
}

/// Why a component is not placed: an exclusion recorded in the scene, or an error that stops the load.
enum Skip {
    Excluded(Exclusion),
    Fatal(SceneError),
}

impl Skip {
    /// The component or its owner could not be loaded or read: an exclusion, not an error of the scene.
    fn load_failed(reason: impl ToString) -> Skip {
        Skip::Excluded(Exclusion::LoadFailed(reason.to_string()))
    }
}

impl<E: Into<SceneError>> From<E> for Skip {
    fn from(e: E) -> Self {
        Skip::Fatal(e.into())
    }
}

/// What a reference resolved to: the export `(package, index)` that defines it, if any.
type Resolved = Result<Option<(PackageId, i32)>, StoreError>;

/// One level's collection actors, read before its components.
#[derive(Debug, Default)]
struct Collections {
    /// Component export -> its tail pose.
    poses: DetMap<i32, CollectionEntry>,
    /// Collection export -> its `bHidden`.
    hidden: DetMap<i32, bool>,
    /// Collection export -> why it could not be read.
    broken: DetMap<i32, String>,
    /// Components two entries list (a gap; never placed).
    listed_twice: DetSet<i32>,
}

/// The load in progress.
struct Loader<'a> {
    defs: &'a mut DefStore,
    space: &'a mut ObjectSpace,
    /// Class path -> its layout, where the store has one.
    classes: DetMap<String, Option<ClassId>>,
    /// Base class path -> its layout (`base_class`).
    bases: DetMap<String, ClassId>,
    scene: Scene,
    /// Duplicate signatures of interactive-object components seen so far.
    signatures: DetSet<String>,
    /// Mesh references already resolved by the package store, whose global fallback reads files on every call.
    resolved: DetMap<(PackageId, i32), Resolved>,
    /// Per level package: folded export path -> the exports at it, in export order (built on first use).
    level_paths: DetMap<PackageId, DetMap<String, Vec<i32>>>,
}

impl Loader<'_> {
    // ------------------------------------------------------------------------------------------------- levels

    /// The persistent level and every level a `LevelStreaming*` object names, breadth first, each once (folded).
    fn levels(&mut self, map: &PackageName) -> Result<(), SceneError> {
        let mut pending = VecDeque::from([map.as_str().to_string()]);
        let mut seen = DetSet::new();
        while let Some(name) = pending.pop_front() {
            if !seen.insert(name.to_ascii_lowercase()) {
                continue;
            }
            let parsed = PackageName::parse(&name).map_err(|_| SceneError::BadLevelName {
                level: self.scene.map.clone(),
                name: name.clone(),
            })?;
            let package = self.defs.code_mut().1.load(&parsed)?;
            self.scene.levels.push(SceneLevel {
                name: name.clone(),
                package,
            });
            // Looked up once a level has loaded, so a missing map is reported as such.
            let streaming = self.base_class("Engine.LevelStreaming")?;
            let count = self.package(package).exports.len();
            for index in 1..=i32::try_from(count).unwrap_or(i32::MAX) {
                let class = self.class_of(package, index)?;
                if !self.is_child(&class, streaming)? {
                    continue;
                }
                let o =
                    self.defs
                        .load_export(self.space, package, index, PropertyPrefix::Ordinary)?;
                let sub = match read(self.space, o, "PackageName") {
                    Ok(Some(Value::Name(n))) => Ok(self.space.names.text(*n).to_string()),
                    Ok(Some(v)) => Err(format!("PackageName is a {}, not a name", v.kind_name())),
                    Ok(None) => Err("the class declares no PackageName".to_string()),
                    Err(reason) => Err(reason),
                };
                let sub = match sub {
                    Ok(sub) => sub,
                    Err(reason) => {
                        let path = self.package(package).path_of(index)?;
                        return Err(bad(&format!("{name}:{path}"), &reason));
                    }
                };
                if sub != "None" {
                    pending.push_back(sub);
                }
            }
        }
        Ok(())
    }

    /// Everything of level `level`, in export order.
    fn level(&mut self, level: usize) -> Result<(), SceneError> {
        let level_key = u32::try_from(level).unwrap_or(u32::MAX);
        let package = self.scene.levels[level].package;
        let count = i32::try_from(self.package(package).exports.len()).unwrap_or(i32::MAX);
        let player_start = self.base_class("Engine.PlayerStart")?;
        let light = self.base_class("Engine.LightComponent")?;
        let terrain = self.base_class("Engine.Terrain")?;

        // Collection tails first: they pose components that come before them in export order.
        let mut collections = Collections::default();
        for index in 1..=count {
            if self.class_of(package, index)? != MESH_COLLECTION {
                continue;
            }
            match self.collection(package, index) {
                Ok((hidden, entries)) => {
                    collections.hidden.insert(index, hidden);
                    for (component, entry) in entries {
                        // A component two entries list has no one pose. The tool stops there
                        // (`prepare_level.py` `Scene.build`, "Duplicate collection component"); here it
                        // is a gap, and `placement` excludes the component.
                        if collections.poses.insert(component, entry).is_some()
                            && collections.listed_twice.insert(component)
                        {
                            let path = self.package(package).path_of(component)?;
                            self.scene.gaps.push(Gap {
                                part: GapPart::Collection,
                                object: format!("{}:{path}", self.scene.levels[level].name),
                                not_yet: NotYet::not_read(
                                    "a component two StaticMeshCollectionActor entries list",
                                    RECORD,
                                ),
                            });
                        }
                    }
                }
                Err(reason) => {
                    let path = self.package(package).path_of(index)?;
                    self.scene.gaps.push(Gap {
                        part: GapPart::Collection,
                        object: format!("{}:{path}", self.scene.levels[level].name),
                        not_yet: NotYet::not_read(
                            "StaticMeshCollectionActor tail outside the recorded layout",
                            RECORD,
                        ),
                    });
                    collections.broken.insert(index, reason);
                }
            }
        }

        for index in 1..=count {
            let key = ExportKey {
                level: level_key,
                export: index,
            };
            let class = self.class_of(package, index)?;
            let path = self.package(package).path_of(index)?;
            if !path.starts_with(LEVEL_PREFIX) {
                continue;
            }
            if class == STATIC_MESH_COMPONENT {
                self.component(level, key, &path, &collections)?;
            } else if self.is_child(&class, light)? {
                let outer = self.package(package).object(index)?.outer;
                let in_collection = outer > 0 && self.class_of(package, outer)? == LIGHT_COLLECTION;
                let mut n = self
                    .scene
                    .lights
                    .get(class.as_str())
                    .copied()
                    .unwrap_or_default();
                n.total = n.total.saturating_add(1);
                n.in_collections = n.in_collections.saturating_add(u32::from(in_collection));
                self.scene.lights.insert(class, n);
            } else if self.is_child(&class, player_start)? {
                let o = self.defs.load_export(
                    self.space,
                    package,
                    index,
                    PropertyPrefix::PlacedActor,
                )?;
                let pose = actor_pose(self.space, o).map_err(|reason| bad(&path, &reason))?;
                self.scene
                    .player_starts
                    .insert(key, PlayerStart { path, class, pose });
            } else if self.is_child(&class, terrain)? {
                self.terrain(level, key, &path)?;
            } else if class == "Engine.Model" || class == "Engine.ModelComponent" {
                let first = !self.scene.models.contains_key(&level_key);
                let mut m = self
                    .scene
                    .models
                    .get(&level_key)
                    .copied()
                    .unwrap_or_default();
                if class == "Engine.Model" {
                    m.models = m.models.saturating_add(1);
                } else {
                    m.components = m.components.saturating_add(1);
                }
                self.scene.models.insert(level_key, m);
                if first {
                    self.scene.gaps.push(Gap {
                        part: GapPart::Bsp,
                        object: self.scene.levels[level].name.clone(),
                        not_yet: NotYet::layer_absent(
                            "BSP model geometry",
                            "an ow-assets Model reader",
                            RECORD,
                        ),
                    });
                }
            }
        }
        Ok(())
    }

    // --------------------------------------------------------------------------------------------- placements

    /// Component `key`: a placement, or an exclusion with its reason.
    fn component(
        &mut self,
        level: usize,
        key: ExportKey,
        path: &str,
        collections: &Collections,
    ) -> Result<(), SceneError> {
        match self.placement(level, key, path, collections) {
            Ok(p) => {
                self.scene.placements.insert(key, p);
            }
            Err(Skip::Excluded(reason)) => {
                let path = path.to_string();
                self.scene.excluded.insert(key, Excluded { path, reason });
            }
            Err(Skip::Fatal(e)) => return Err(e),
        }
        Ok(())
    }

    fn placement(
        &mut self,
        level: usize,
        key: ExportKey,
        path: &str,
        collections: &Collections,
    ) -> Result<Placement, Skip> {
        let package = self.scene.levels[level].package;
        let component = self
            .defs
            .load_export(self.space, package, key.export, PropertyPrefix::Component)
            .map_err(Skip::load_failed)?;
        // The component's own tags: its StaticMesh reference (see `own_tags`) and whether it sets a scale.
        let tags = own_tags(self.package(package), key.export).map_err(Skip::load_failed)?;
        let mesh = match tags.static_mesh {
            Some(r) if r != 0 => r,
            _ => return Err(Skip::Excluded(Exclusion::NoStaticMesh)),
        };
        let outer = self.package(package).object(key.export)?.outer;
        let owner_class = if outer > 0 {
            self.class_of(package, outer)?
        } else {
            String::from("None")
        };
        let component_pose = component_pose(self.space, component).map_err(Skip::load_failed)?;

        let (pose, owner_hidden) = if collections.listed_twice.contains(&key.export) {
            return Err(Skip::Excluded(Exclusion::CollectionUnreadable(
                "listed by two collection entries".to_string(),
            )));
        } else if let Some(entry) = collections.poses.get(&key.export) {
            let (scale3d, scale, scale_source) = if tags.sets_scale {
                (
                    component_pose.scale3d,
                    component_pose.scale,
                    ScaleSource::Component,
                )
            } else {
                (entry.scale3d, entry.scale, ScaleSource::CollectionTail)
            };
            let pose = Pose::Collection {
                matrix: entry.matrix,
                scale3d,
                scale,
                scale_source,
            };
            let hidden = collections
                .hidden
                .get(&entry.collection)
                .copied()
                .unwrap_or(false);
            (pose, hidden)
        } else if let Some(reason) = collections.broken.get(&outer) {
            return Err(Skip::Excluded(Exclusion::CollectionUnreadable(
                reason.clone(),
            )));
        } else if PREVIEW_OWNER_CLASSES.contains(&owner_class.as_str()) {
            let preview = preview_mesh(self.package(package), outer).map_err(Skip::load_failed)?;
            return Err(Skip::Excluded(if preview == Some(key.export) {
                Exclusion::MissionItemPreview
            } else {
                Exclusion::UnsupportedOwner(owner_class)
            }));
        } else if !PLACED_OWNER_CLASSES.contains(&owner_class.as_str()) {
            return Err(Skip::Excluded(Exclusion::UnsupportedOwner(owner_class)));
        } else {
            let owner = self
                .defs
                .load_export(self.space, package, outer, PropertyPrefix::PlacedActor)
                .map_err(Skip::load_failed)?;
            let actor = actor_pose(self.space, owner).map_err(Skip::load_failed)?;
            let hidden = read_bool(self.space, owner, "bHidden").map_err(Skip::load_failed)?;
            let pose = Pose::Attached {
                actor,
                component: component_pose,
            };
            (pose, hidden)
        };

        let hidden_game =
            read_bool(self.space, component, "HiddenGame").map_err(Skip::load_failed)?;
        let overrides =
            read_objects(self.space, component, "Materials").map_err(Skip::load_failed)?;
        let mesh_path = self.mesh(package, mesh)?;
        let mut materials = Vec::with_capacity(overrides.len());
        for o in overrides {
            materials.push(match o {
                Some(o) => Some(self.material(o)?),
                None => None,
            });
        }

        if DEDUPLICATED_OWNER_CLASSES.contains(&owner_class.as_str()) {
            let mut signature = format!("{}|{outer}|{mesh_path}|{materials:?}|", key.level);
            if let Pose::Attached { component: c, .. } = &pose {
                for v in c.translation.iter().chain(&c.scale3d).chain([&c.scale]) {
                    signature.push_str(&v.to_bits().to_string());
                    signature.push(',');
                }
                let r = c.rotation;
                signature.push_str(&format!("{},{},{}|", r.pitch, r.yaw, r.roll));
            }
            for flag in DUPLICATE_FLAGS {
                let set = read_bool(self.space, component, flag).map_err(Skip::load_failed)?;
                signature.push(if set { '1' } else { '0' });
            }
            if !self.signatures.insert(signature) {
                return Err(Skip::Excluded(Exclusion::DuplicateComponent));
            }
        }

        Ok(Placement {
            path: path.to_string(),
            owner_class,
            mesh: mesh_path,
            pose,
            hidden: HiddenFlags {
                hidden_game,
                owner_hidden,
            },
            materials,
        })
    }

    /// A collection actor's `bHidden` and its components with their tail poses. `Err`: why the actor does not read as
    /// decision 2026-09-10-09 records.
    fn collection(
        &mut self,
        package: PackageId,
        index: i32,
    ) -> Result<(bool, Vec<(i32, CollectionEntry)>), String> {
        // The class's layout, and the export's class reference resolved, for the reflection typer's cache.
        self.defs
            .class_id(self.space, MESH_COLLECTION)
            .map_err(|e| e.to_string())?;
        let class_ref = self
            .package(package)
            .object(index)
            .map_err(|e| e.to_string())?
            .class;
        {
            let (code, store) = self.defs.code_mut();
            code.class_at(store, package, class_ref)
                .map_err(|e| e.to_string())?;
        }
        let pkg = self.defs.packages().get(package);
        let typer = ReflectionTyper::new(
            self.defs.packages(),
            self.defs.code(),
            self.space,
            package,
            self.defs.mode(),
        );
        let tagged = props::read_tagged(pkg, index, PropertyPrefix::Ordinary.bytes(), &typer)
            .map_err(|e| prop_text(&e))?;
        let mut hidden = false;
        let mut refs = Vec::new();
        for entry in &tagged.entries {
            match (entry.name.as_str(), &entry.value) {
                ("bHidden", PropValue::Bool(b)) => hidden = *b,
                ("StaticMeshComponents", PropValue::Array(items)) => {
                    for item in items {
                        match item {
                            PropValue::Object(r) => refs.push(*r),
                            _ => return Err("StaticMeshComponents holds a non-object".into()),
                        }
                    }
                }
                ("StaticMeshComponents", _) => {
                    return Err("StaticMeshComponents is not an object array".into());
                }
                _ => {}
            }
        }
        let object = pkg.object(index).map_err(|e| e.to_string())?;
        let offset = usize::try_from(object.offset).map_err(|_| "a negative export offset")?;
        let size = usize::try_from(object.size).map_err(|_| "a negative export size")?;
        let start = offset
            .checked_add(tagged.end)
            .ok_or("tail start overflows")?;
        let end = offset.checked_add(size).ok_or("export end overflows")?;
        let want = refs
            .len()
            .checked_mul(COLLECTION_ENTRY)
            .ok_or("tail size overflows")?;
        if end < start || end - start != want {
            return Err(format!(
                "tail of {} bytes for {} components",
                end.saturating_sub(start),
                refs.len()
            ));
        }
        let mut r = pkg.reader();
        r.set_pos(start);
        r.set_limit(end);
        let mut out = Vec::with_capacity(refs.len());
        for component in refs {
            let mut row = [0f32; 20];
            for v in &mut row {
                *v = f32::from_bits(r.u32().map_err(|e| e.to_string())?);
                if !v.is_finite() {
                    return Err("a non-finite tail value".into());
                }
            }
            r.i32().map_err(|e| e.to_string())?;
            let affine = [3, 7, 11].iter().all(|&k| row[k].abs() <= AFFINE_TOLERANCE)
                && (row[15] - 1.0).abs() <= AFFINE_TOLERANCE;
            if !affine {
                return Err("a non-affine tail matrix".into());
            }
            // A null entry poses nothing: its 84 bytes are checked and dropped. The tool zips the two arrays and
            // keeps the pose under reference 0, which no component has, so it tolerates one null per level and stops
            // on a second ("Duplicate collection component", `prepare_level.py` `Scene.build`); every null is dropped
            // here.
            if component == 0 {
                continue;
            }
            if component < 0 {
                return Err(format!("component reference {component} is not an export"));
            }
            let mut matrix = [0f32; 16];
            matrix.copy_from_slice(&row[..16]);
            out.push((
                component,
                CollectionEntry {
                    collection: index,
                    matrix,
                    scale3d: [row[16], row[17], row[18]],
                    scale: row[19],
                },
            ));
        }
        Ok((hidden, out))
    }

    fn terrain(&mut self, level: usize, key: ExportKey, path: &str) -> Result<(), SceneError> {
        let package = self.scene.levels[level].package;
        let o =
            self.defs
                .load_export(self.space, package, key.export, PropertyPrefix::PlacedActor)?;
        let pose = actor_pose(self.space, o).map_err(|reason| bad(path, &reason))?;
        let pkg = self.package(package);
        let mut components = 0u32;
        for e in &pkg.exports {
            if e.outer == key.export && e.class != 0 {
                let class = pkg.path_of(e.class)?;
                if class == "Engine.TerrainComponent" {
                    components = components.saturating_add(1);
                }
            }
        }
        self.scene.terrains.insert(
            key,
            TerrainActor {
                path: path.to_string(),
                pose,
                components,
            },
        );
        self.scene.gaps.push(Gap {
            part: GapPart::Terrain,
            object: format!("{}:{path}", self.scene.levels[level].name),
            not_yet: NotYet::layer_absent(
                "terrain height and layer data",
                "an ow-assets Terrain reader",
                RECORD,
            ),
        });
        Ok(())
    }

    // ------------------------------------------------------------------------------------ meshes and materials

    /// The mesh reference `reference` of level `package` names, decoded once. An export is itself. An import is an
    /// export of the scene's levels with the import's path and class name (`in_levels`, the tool's `Scene.resolve`
    /// narrowed; UNVERIFIED as the game's lookup), else what the package store resolves it to (C++
    /// `PackageStore::resolve`, whose global fallback reads candidate files in scan order).
    fn mesh(&mut self, package: PackageId, reference: i32) -> Result<String, SceneError> {
        let resolved = match self.resolved.get(&(package, reference)) {
            Some(r) => r.clone(),
            None => {
                let r = match self.in_levels(package, reference)? {
                    Some(found) => Ok(Some(found)),
                    None => self.defs.code_mut().1.resolve(package, reference),
                };
                self.resolved.insert((package, reference), r.clone());
                r
            }
        };
        let (path, holder) = match resolved {
            Ok(Some((p, i))) => {
                let pkg = self.package(p);
                let holder = Holder {
                    package: p,
                    file: pkg.name.clone(),
                    export: i,
                };
                (full_path(pkg, i)?, Some(holder))
            }
            Ok(None) => (String::from("None"), None),
            Err(StoreError::ImportNotFound { .. } | StoreError::PackageNotFound(_)) => {
                (self.package(package).path_of(reference)?, None)
            }
            Err(e) => return Err(e.into()),
        };
        if self.scene.meshes.contains_key(path.as_str()) {
            return Ok(path);
        }
        let Some(holder) = holder else {
            let not_yet = no_holder("StaticMesh");
            self.scene.gaps.push(Gap {
                part: GapPart::Mesh,
                object: path.clone(),
                not_yet: not_yet.clone(),
            });
            self.scene.meshes.insert(
                path.clone(),
                SceneMesh {
                    holder: None,
                    status: MeshStatus::NotYet(not_yet),
                    section_materials: Vec::new(),
                },
            );
            return Ok(path);
        };
        let decoded = read_mesh(
            self.defs.packages().get(holder.package),
            holder.export,
            // An ordinary object's prefix, as the tool passes it (`--property-offset 4`).
            PropertyPrefix::Ordinary.bytes(),
            &NoArrayTypes,
        );
        let mut section_materials = Vec::new();
        let status = match decoded {
            Ok(mesh) => {
                let refs: Vec<i32> = mesh
                    .lods
                    .first()
                    .map(|l| l.sections.iter().map(|s| s.material).collect())
                    .unwrap_or_default();
                for r in refs {
                    section_materials.push(self.section_material(holder.package, r)?);
                }
                MeshStatus::Decoded(mesh)
            }
            Err(e) => MeshStatus::Refused(e),
        };
        self.scene.meshes.insert(
            path.clone(),
            SceneMesh {
                holder: Some(holder),
                status,
                section_materials,
            },
        );
        Ok(path)
    }

    /// A mesh section's material reference (in the mesh's file): its path, read once.
    fn section_material(
        &mut self,
        package: PackageId,
        reference: i32,
    ) -> Result<Option<String>, SceneError> {
        if reference == 0 {
            return Ok(None);
        }
        match self.defs.resolve_ref(self.space, package, reference) {
            Ok(Some(o)) => Ok(Some(self.material(o)?)),
            Ok(None) => Ok(None),
            Err(e) => {
                let path = self.package(package).path_of(reference)?;
                if !self.scene.materials.contains_key(path.as_str()) {
                    self.scene.materials.insert(
                        path.clone(),
                        SceneMaterial {
                            class: String::new(),
                            parent: None,
                            textures: Vec::new(),
                            two_sided: None,
                            status: MaterialStatus::Failed(e.to_string()),
                        },
                    );
                }
                Ok(Some(path))
            }
        }
    }

    /// Material handle `o`: its path, its chain read once (module documentation).
    fn material(&mut self, o: ObjRef) -> Result<String, SceneError> {
        let (path, _) = self.locate(o)?;
        if self.scene.materials.contains_key(path.as_str()) {
            return Ok(path);
        }
        let instance = self.base_class("Engine.MaterialInstance")?;
        let base = self.base_class("Engine.Material")?;
        let mut entry = SceneMaterial {
            class: String::new(),
            parent: None,
            textures: Vec::new(),
            two_sided: None,
            status: MaterialStatus::Read,
        };
        let mut cursor = Some(o);
        let mut depth = 0usize;
        while let Some(m) = cursor.take() {
            if depth == MAX_MATERIAL_CHAIN {
                entry.status = MaterialStatus::NotYet(NotYet::not_read(
                    "a material chain longer than 32 parents",
                    RECORD,
                ));
                break;
            }
            depth += 1;
            if let Err(e) = self.defs.ensure_loaded(self.space, m) {
                entry.status = MaterialStatus::Failed(e.to_string());
                break;
            }
            let class = self.space.heap.get(m)?.class;
            let class_path = self.class_path(class);
            if depth == 1 {
                entry.class = class_path.clone();
            }
            // The depth this material sits at in the chain: 0 for the material asked for.
            let at = u32::try_from(depth - 1).unwrap_or(u32::MAX);
            let read = if self.space.classes.is_child_of(class, instance) {
                self.instance_textures(m, at, &mut entry.textures)
                    .and_then(|()| read_object(self.space, m, "Parent"))
                    .map(|parent| {
                        cursor = parent;
                    })
            } else if self.space.classes.is_child_of(class, base) {
                entry.two_sided = match read(self.space, m, "TwoSided") {
                    Ok(Some(Value::Bool(b))) => Some(*b),
                    _ => None,
                };
                self.expression_textures(m, at, &mut entry.textures)
            } else {
                entry.status = MaterialStatus::NotYet(NotYet::not_read(
                    "a material class outside Engine.Material and Engine.MaterialInstance",
                    RECORD,
                ));
                break;
            };
            if let Err(reason) = read {
                entry.status = MaterialStatus::Failed(reason);
                break;
            }
            if depth == 1 {
                entry.parent = match cursor {
                    Some(p) => Some(self.locate(p)?.0),
                    None => None,
                };
            }
        }
        if let MaterialStatus::NotYet(n) = &entry.status {
            self.scene.gaps.push(Gap {
                part: GapPart::Material,
                object: format!("{path} ({})", entry.class),
                not_yet: n.clone(),
            });
        }
        self.scene.materials.insert(path.clone(), entry);
        Ok(path)
    }

    /// The textures of a material instance's `TextureParameterValues`, with their `ParameterName`s.
    fn instance_textures(
        &mut self,
        m: ObjRef,
        depth: u32,
        out: &mut Vec<MaterialTexture>,
    ) -> Result<(), String> {
        let values = match read(self.space, m, "TextureParameterValues")? {
            Some(Value::Array(items)) => items.clone(),
            Some(_) => return Err("TextureParameterValues is not an array".into()),
            None => return Ok(()),
        };
        for item in values.iter() {
            if let Some(Value::Object(Some(t))) = struct_field(self.space, item, "ParameterValue") {
                let parameter = match struct_field(self.space, item, "ParameterName") {
                    Some(Value::Name(n)) => parameter_name(&self.space.names.text(*n)),
                    _ => None,
                };
                self.texture(*t, parameter, TextureSource::Instance, depth, out)
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    /// The `Texture` of each expression of a base material, with the expression's `ParameterName` where its class
    /// declares one.
    fn expression_textures(
        &mut self,
        m: ObjRef,
        depth: u32,
        out: &mut Vec<MaterialTexture>,
    ) -> Result<(), String> {
        let expressions = read_objects(self.space, m, "Expressions")?;
        for e in expressions.into_iter().flatten() {
            self.defs
                .ensure_loaded(self.space, e)
                .map_err(|e| e.to_string())?;
            if let Some(t) = read_object(self.space, e, "Texture")? {
                let parameter = match read(self.space, e, "ParameterName")? {
                    Some(Value::Name(n)) => parameter_name(&self.space.names.text(*n)),
                    _ => None,
                };
                self.texture(t, parameter, TextureSource::Expression, depth, out)
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    /// Texture handle `t`, named by `parameter` from `from`: located once, and added to `out` unless the same entry is
    /// there already.
    fn texture(
        &mut self,
        t: ObjRef,
        parameter: Option<String>,
        from: TextureSource,
        depth: u32,
        out: &mut Vec<MaterialTexture>,
    ) -> Result<(), SceneError> {
        let (path, holder) = self.locate(t)?;
        if !self.scene.textures.contains_key(path.as_str()) {
            let class = self.space.heap.get(t)?.class;
            let class = self.class_path(class);
            let gap = if holder.is_none() {
                Some(no_holder("Texture"))
            } else if !TEXTURE_CLASSES.contains(&class.as_str()) {
                Some(texture_class_not_yet())
            } else {
                None
            };
            if let Some(not_yet) = gap {
                self.scene.gaps.push(Gap {
                    part: GapPart::Texture,
                    object: format!("{path} ({class})"),
                    not_yet,
                });
            }
            self.scene
                .textures
                .insert(path.clone(), SceneTexture { class, holder });
        }
        let entry = MaterialTexture {
            path,
            parameter,
            from,
            depth,
        };
        if !out.contains(&entry) {
            out.push(entry);
        }
        Ok(())
    }

    /// Import `reference` of `package` as an export of one of the scene's levels: in load order, the first export at
    /// the import's path (folded) whose class has the import's class name. Every export at that path is tried, not
    /// only the first.
    ///
    /// This is the lookup of `tools/prepare_level.py` `Scene.resolve`, narrowed in two ways (decision 2026-11-16-46):
    /// the tool searches every package it has read so far (`Startup`, then the levels, then whatever earlier
    /// `--resolve` calls loaded), this searches the scene's levels only; and the tool accepts an import with no class
    /// name, this does not. What is not found here goes to `PackageStore::resolve`, as in the tool.
    fn in_levels(
        &mut self,
        package: PackageId,
        reference: i32,
    ) -> Result<Option<(PackageId, i32)>, SceneError> {
        if reference >= 0 {
            return Ok(None);
        }
        let from = self.package(package);
        let path = from.path_of(reference)?.to_ascii_lowercase();
        let class = from.object(reference)?.class_name.clone();
        for level in 0..self.scene.levels.len() {
            let id = self.scene.levels[level].package;
            if !self.level_paths.contains_key(&id) {
                let pkg = self.package(id);
                let mut index: DetMap<String, Vec<i32>> = DetMap::new();
                for export in 1..=i32::try_from(pkg.exports.len()).unwrap_or(i32::MAX) {
                    let at = pkg.path_of(export)?.to_ascii_lowercase();
                    index.entry(at).or_default().push(export);
                }
                self.level_paths.insert(id, index);
            }
            let candidates = self
                .level_paths
                .get(&id)
                .and_then(|index| index.get(&path))
                .cloned()
                .unwrap_or_default();
            let pkg = self.package(id);
            for export in candidates {
                let object = pkg.object(export)?;
                let found = if object.class == 0 {
                    "Class"
                } else {
                    pkg.object(object.class)?.name.as_str()
                };
                if found == class {
                    return Ok(Some((id, export)));
                }
            }
        }
        Ok(None)
    }

    // ------------------------------------------------------------------------------------------------ helpers

    fn package(&self, id: PackageId) -> &Package {
        self.defs.packages().get(id)
    }

    /// The object path of store object `o` and, for an export, its holder. The store's handles name the export of
    /// the holder its policy chose (`Origin::Resource` while unloaded, `Origin::Export` once loaded); a reference no
    /// holder has stays a stand-in naming the import (a negative index).
    fn locate(&self, o: ObjRef) -> Result<(String, Option<Holder>), SceneError> {
        let (package, index) = match self.space.heap.get(o)?.origin {
            Origin::Export { package, index } | Origin::Resource { package, index } => {
                (package, index)
            }
            other => {
                return Ok((format!("<object {other:?}>"), None));
            }
        };
        let pkg = self.package(package);
        let path = full_path(pkg, index)?;
        let holder = (index > 0).then(|| Holder {
            package,
            file: pkg.name.clone(),
            export: index,
        });
        Ok((path, holder))
    }

    /// The class path of export or import `index` of `package`, as `--exports` prints it (`Class` for a class).
    fn class_of(&self, package: PackageId, index: i32) -> Result<String, SceneError> {
        let pkg = self.package(package);
        let class = pkg.object(index)?.class;
        Ok(if class == 0 {
            String::from("Class")
        } else {
            pkg.path_of(class)?
        })
    }

    /// The layout of a class the scene needs (`Engine.PlayerStart`, say). A store failure is the scene's error, never
    /// an empty result: without the class, no export could be recognised as one of its kind. Cached: the store's
    /// lookup by path is not, and the material walk asks for two of these per material.
    fn base_class(&mut self, path: &str) -> Result<ClassId, SceneError> {
        if let Some(&c) = self.bases.get(path) {
            return Ok(c);
        }
        let c = self.defs.class_id(self.space, path)?;
        self.bases.insert(path.to_string(), c);
        Ok(c)
    }

    /// The layout of an export's class, cached. `None` only where no installed package exports the class (an
    /// engine-native class such as `Engine.Model`, or a class of a package the install lacks): such an export is then
    /// of none of the base classes. Any other store failure is the scene's error.
    fn export_class(&mut self, path: &str) -> Result<Option<ClassId>, SceneError> {
        if let Some(c) = self.classes.get(path) {
            return Ok(*c);
        }
        let c = match self.defs.class_id(self.space, path) {
            Ok(c) => Some(c),
            Err(DefError::Reflect(ReflectError::ClassNotFound(_)))
            | Err(DefError::Store(StoreError::PackageNotFound(_))) => None,
            Err(e) => return Err(e.into()),
        };
        self.classes.insert(path.to_string(), c);
        Ok(c)
    }

    /// Whether an export of class `class_path` is `base` or a subclass of it.
    fn is_child(&mut self, class_path: &str, base: ClassId) -> Result<bool, SceneError> {
        if class_path == "Class" {
            return Ok(false);
        }
        Ok(match self.export_class(class_path)? {
            Some(c) => self.space.classes.is_child_of(c, base),
            None => false,
        })
    }

    fn class_path(&self, class: ClassId) -> String {
        self.space
            .classes
            .get_layout(class)
            .map(|l| l.path().as_str().to_string())
            .unwrap_or_default()
    }
}

/// A `ParameterName` value: `None` for the name `None`.
pub(crate) fn parameter_name(name: &str) -> Option<String> {
    (name != "None").then(|| name.to_string())
}

/// The object path of `index` in `package` as the game names it: an import's path is full; an export whose outermost
/// export is a `Package` object is named by its path in the file, any other by `<file stem>.<path in the file>` (the
/// definition store's rule, `ow_defs::identity`).
pub(crate) fn full_path(package: &Package, index: i32) -> Result<String, PackageError> {
    let path = package.path_of(index)?;
    if index <= 0 {
        return Ok(path);
    }
    let mut cursor = index;
    // path_of has checked that the chain ends within its bound.
    loop {
        let outer = package.object(cursor)?.outer;
        if outer == 0 {
            break;
        }
        if outer < 0 {
            return Ok(path);
        }
        cursor = outer;
    }
    let outermost = package.object(cursor)?;
    let is_package = outermost.class != 0 && package.object(outermost.class)?.name == "Package";
    Ok(if is_package {
        path
    } else {
        format!("{}.{path}", package.name)
    })
}

/// What a component's own tags say that the store's object cannot: the `StaticMesh` reference as a package
/// reference, and whether the tags set `Scale3D` or `Scale`.
struct OwnTags {
    static_mesh: Option<i32>,
    sets_scale: bool,
}

/// Export `index`'s own tags, walked from 8 bytes in (a component) with no array typing.
///
/// The `StaticMesh` reference is read here, not from the store's object: `Engine.StaticMesh` is an engine-native
/// class no installed package exports, and in `ParityMode::Off` the decoder skips a reference to an object of such
/// a class (`ow_object::decode`, "defaults: skipped StaticMeshComponent.StaticMesh"), so the store's slot is always
/// `None`. A component that does not tag it has none of its own; its archetype's is not read (as the tool).
fn own_tags(package: &Package, index: i32) -> Result<OwnTags, String> {
    let tagged = props::read_tagged(
        package,
        index,
        PropertyPrefix::Component.bytes(),
        &NoArrayTypes,
    )
    .map_err(|e| prop_text(&e))?;
    let mut tags = OwnTags {
        static_mesh: None,
        sets_scale: false,
    };
    for entry in &tagged.entries {
        match (entry.name.as_str(), &entry.value) {
            ("StaticMesh", PropValue::Object(r)) => tags.static_mesh = Some(*r),
            ("StaticMesh", _) => return Err("StaticMesh is not an object reference".into()),
            ("Scale3D" | "Scale", _) => tags.sets_scale = true,
            _ => {}
        }
    }
    Ok(tags)
}

/// A mission pickup spawner's or failsafe's `PreviewMesh` reference, from its own tags (26 bytes in). The cooked
/// class does not declare `PreviewMesh` (an editor-only property), so the store's decoder drops the tag.
fn preview_mesh(package: &Package, owner: i32) -> Result<Option<i32>, String> {
    let tagged = props::read_tagged(
        package,
        owner,
        PropertyPrefix::PlacedActor.bytes(),
        &NoArrayTypes,
    )
    .map_err(|e| prop_text(&e))?;
    Ok(tagged
        .entries
        .iter()
        .find_map(|e| match (e.name.as_str(), &e.value) {
            ("PreviewMesh", PropValue::Object(r)) => Some(*r),
            _ => None,
        }))
}

fn prop_text(e: &PropError) -> String {
    e.to_string()
}

fn bad(object: &str, reason: &str) -> SceneError {
    SceneError::Unreadable {
        object: object.to_string(),
        reason: reason.to_string(),
    }
}

// ---------------------------------------------------------------------------------------------- value readers

/// Property `name` of loaded object `o`, or `None` where its class declares no such property.
pub(crate) fn read<'w>(space: &'w ObjectSpace, o: ObjRef, name: &str) -> Result<Option<&'w Value>, String> {
    let class = space.heap.get(o).map_err(|e| e.to_string())?.class;
    let Some(p) = space.prop(class, name) else {
        return Ok(None);
    };
    space
        .read(o, PropAddr::scalar(p))
        .map(Some)
        .map_err(|e| format!("{name}: {e}"))
}

pub(crate) fn read_object(space: &ObjectSpace, o: ObjRef, name: &str) -> Result<Option<ObjRef>, String> {
    match read(space, o, name)? {
        Some(Value::Object(r)) => Ok(*r),
        None => Ok(None),
        Some(v) => Err(format!("{name} is a {}, not an object", v.kind_name())),
    }
}

pub(crate) fn read_objects(space: &ObjectSpace, o: ObjRef, name: &str) -> Result<Vec<Option<ObjRef>>, String> {
    match read(space, o, name)? {
        Some(Value::Array(items)) => items
            .iter()
            .map(|v| match v {
                Value::Object(r) => Ok(*r),
                v => Err(format!("{name} holds a {}, not an object", v.kind_name())),
            })
            .collect(),
        None => Ok(Vec::new()),
        Some(v) => Err(format!("{name} is a {}, not an array", v.kind_name())),
    }
}

pub(crate) fn read_bool(space: &ObjectSpace, o: ObjRef, name: &str) -> Result<bool, String> {
    match read(space, o, name)? {
        Some(Value::Bool(b)) => Ok(*b),
        Some(v) => Err(format!("{name} is a {}, not a bool", v.kind_name())),
        None => Err(format!("the class declares no {name}")),
    }
}

pub(crate) fn read_float(space: &ObjectSpace, o: ObjRef, name: &str) -> Result<f32, String> {
    match read(space, o, name)? {
        Some(Value::Float(f)) => Ok(*f),
        Some(v) => Err(format!("{name} is a {}, not a float", v.kind_name())),
        None => Err(format!("the class declares no {name}")),
    }
}

/// Field `name` of struct value `v`, or `None`.
pub(crate) fn struct_field<'v>(space: &ObjectSpace, v: &'v Value, name: &str) -> Option<&'v Value> {
    let s = v.as_struct()?;
    let layout = space.classes.struct_layout(s.ty)?;
    let (entry, _) = layout.field(space.names.find(name)?)?;
    s.fields.get(usize::try_from(entry).ok()?)
}

fn read_vector(space: &ObjectSpace, o: ObjRef, name: &str) -> Result<[f32; 3], String> {
    let v = read(space, o, name)?.ok_or_else(|| format!("the class declares no {name}"))?;
    let mut out = [0f32; 3];
    for (slot, field) in out.iter_mut().zip(["X", "Y", "Z"]) {
        *slot = match struct_field(space, v, field) {
            Some(Value::Float(f)) => *f,
            _ => return Err(format!("{name}.{field} is not a float")),
        };
    }
    Ok(out)
}

pub(crate) fn read_rotator(space: &ObjectSpace, o: ObjRef, name: &str) -> Result<Rotator, String> {
    let v = read(space, o, name)?.ok_or_else(|| format!("the class declares no {name}"))?;
    let mut out = [0i32; 3];
    for (slot, field) in out.iter_mut().zip(["Pitch", "Yaw", "Roll"]) {
        *slot = match struct_field(space, v, field) {
            Some(Value::Int(i)) => *i,
            _ => return Err(format!("{name}.{field} is not an int")),
        };
    }
    Ok(Rotator::new(out[0], out[1], out[2]))
}

fn actor_pose(space: &ObjectSpace, o: ObjRef) -> Result<ActorPose, String> {
    Ok(ActorPose {
        location: read_vector(space, o, "Location")?,
        rotation: read_rotator(space, o, "Rotation")?,
        draw_scale: read_float(space, o, "DrawScale")?,
        draw_scale3d: read_vector(space, o, "DrawScale3D")?,
    })
}

fn component_pose(space: &ObjectSpace, o: ObjRef) -> Result<ComponentPose, String> {
    Ok(ComponentPose {
        translation: read_vector(space, o, "Translation")?,
        rotation: read_rotator(space, o, "Rotation")?,
        scale: read_float(space, o, "Scale")?,
        scale3d: read_vector(space, o, "Scale3D")?,
    })
}

#[cfg(test)]
mod tests;
