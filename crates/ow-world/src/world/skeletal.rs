//! Owner: lane IMPL-SKELETAL-MESH-LAYER (decision 2026-11-19-184, number pending). The skeletal mesh data layer, read
//! only, for `ParityMode::Off`: a loaded `SkeletalMesh`'s reference skeleton (bone names in order and parent indices)
//! and its sockets. AI-assisted (Claude Code). **UNVERIFIED in the running game**: the layer reports what the data
//! holds, not what the engine does with it.
//!
//! **Who reads what** (the consumers keep their own rules; they switch over in their own lanes):
//! - **#529 (NATIVE_INITIALIZE_BODY_COMPOSITION.md), a socket attach to a skeletal parent:** the parent mesh's
//!   [`sockets`] (each socket's name, bone name, relative location, rotation and scale), matched first, then the
//!   bone names of its [`reference_skeleton`]. Its refusal site names this layer, [`LAYER`], and asks these two.
//!   The `Parent` link's parent-bone map (its hub ruling 3) would read both meshes' bone names.
//! - **#566 (NATIVE_ANIM_NODE_INIT.md), the per-bone weight builds of `AnimNodeBlendPerBone` and
//!   `WillowAnimNodeFeatherBlend`:** the mesh's [`reference_skeleton`]: the bone count, each bone's parent, and the
//!   bone names its branch names are looked up as.
//!
//! **Where the data comes from.** The project's decoders do not read `SkeletalMesh`'s native serialization: its
//! layout has no public spec and none is recovered (AGENTS.md rule 4, ow-assets `skeletal`). So:
//! - **the reference skeleton** comes from a [`SkeletonSource`] the harness installs ([`SkeletalMeshes::new`]). The
//!   world owns the trait; `ow-cli` implements it with ow-assets' ActorX `.psk` reader over UModel's exports of the
//!   player's own install (the crate edges do not let `ow-world` depend on `ow-assets`). With no source installed, or
//!   no export for the mesh, it is `NotYet { LayerAbsent }` with [`LAYER`], the text #529 already names;
//! - **the sockets** are the mesh's tagged `Sockets` property, read from the definition store's objects.
//!
//! **Write-free.** Nothing here writes the heap, draws from the generator or loads through the store: a parse of an
//! export file is not a load. An object the store has not loaded (a handle, or a stand-in with no slots whose class
//! has some) is refused `NotYet { LayerAbsent }` (the store's load), not loaded. The skeleton is cached per mesh
//! object (a `DetMap` by identity), so a second ask parses nothing.
//!
//! **Typed errors:** a receiver that is not a `SkeletalMesh` is `VmError::Script`; an export that fails to parse is
//! `VmError::Script` naming the mesh and the reader's error; a None `Sockets` entry is `NotYet { NotRead }` (what the
//! engine does with one is not read).

use std::rc::Rc;

use ow_core::{DetMap, NotYet};
use ow_object::heap::Origin;
use ow_object::ids::{NameId, ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

/// The layer, as the consumers' refusals name it (#529).
pub const LAYER: &str = "a skeletal mesh data reader (sockets and the reference skeleton)";
/// The record of the layer.
pub const RECORD: &str = "docs/decisions/2026-11/2026-11-19-184-skeletal-mesh-data-layer.md";
const UNIT: &str = "the skeletal mesh data layer";
const MESH_CLASS: &str = "Engine.SkeletalMesh";
const LOAD_LAYER: &str = "the definition store's load (the skeletal mesh data layer loads nothing)";

/// One bone of a reference skeleton: its name and its parent's index (`None` for the root).
#[derive(Debug, Clone, PartialEq)]
pub struct SkeletonBone {
    pub name: String,
    pub parent: Option<usize>,
    /// The bone's bind pose relative to its parent, when the source has it (lane IMPL-BONE-TRANSFORM-LAYER,
    /// NATIVE_BONE_TRANSFORM_LAYER.md section 6.1). `None` is "unknown": the reference pose then refuses. That the
    /// source's bind pose equals the mesh's reference skeleton is **UNVERIFIED**.
    pub bind: Option<BoneBind>,
}

/// A bone's transform relative to its parent: `p_parent = q p q* + t` (the `.psk` reader's convention).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoneBind {
    /// A unit quaternion `(x, y, z, w)`.
    pub rotation: [f32; 4],
    pub translation: [f32; 3],
}

/// One component-space bone transform, as `SpaceBases` holds it: a rotation, a translation and a uniform scale
/// (NATIVE_BONE_TRANSFORM_LAYER.md sections 1 and 6.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpaceBase {
    /// A unit quaternion `(x, y, z, w)`.
    pub rotation: [f32; 4],
    pub translation: [f32; 3],
    pub scale: f32,
}

/// A mesh's reference skeleton: its bones in order; every bone's parent comes before it.
#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceSkeleton {
    pub bones: Vec<SkeletonBone>,
}

impl ReferenceSkeleton {
    /// The bone count.
    pub fn len(&self) -> usize {
        self.bones.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bones.is_empty()
    }

    /// The index of the bone named `name`, compared as names are (case-insensitively).
    pub fn bone_index(&self, name: &str) -> Option<usize> {
        self.bones
            .iter()
            .position(|b| b.name.eq_ignore_ascii_case(name))
    }
}

/// One socket of a mesh, as its `SkeletalMeshSocket` object holds it.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshSocket {
    pub name: String,
    pub bone: String,
    pub location: [f32; 3],
    /// Pitch, yaw, roll in the engine's rotation units.
    pub rotation: [i32; 3],
    pub scale: [f32; 3],
}

/// Where a mesh's reference skeleton is read from. `package` is the file package holding the mesh's export, `path`
/// its object path, `leaf` its object name. `Ok(None)`: no data for that mesh (the layer refuses `LayerAbsent`).
/// `Err(text)`: the data failed to parse.
pub trait SkeletonSource {
    fn skeleton(&self, package: &str, path: &str, leaf: &str) -> Result<Option<ReferenceSkeleton>, String>;

    /// The number of the mesh's LOD models, when the source knows it (lane IMPL-SKELMESH-MESH-CHANGE,
    /// NATIVE_SKELMESH_MESH_CHANGE.md section 4, row 15). The default, and today every source's answer, is `Ok(None)`:
    /// the project reads no LOD models.
    fn lod_count(&self, package: &str, path: &str, leaf: &str) -> Result<Option<usize>, String> {
        let _ = (package, path, leaf);
        Ok(None)
    }

    /// The LOD model count of export `index` of `package`, the definition store's own in-memory package (lane
    /// SKELMESH-LOD-LAYER), when the source reads it from the package itself. Asked before
    /// [`SkeletonSource::lod_count`]; the default, `Ok(None)`, leaves the answer to that method. Reads only.
    fn lod_count_in(&self, package: &ow_package::package::Package, index: i32) -> Result<Option<usize>, String> {
        let _ = (package, index);
        Ok(None)
    }

    /// Each LOD model's trailing count, one entry per LOD, of export `index` of `package`, when the source reads the
    /// LOD models from the package itself (lane SKELMESH-LOD-LAYER). The trailing count is an unidentified LOD-model
    /// field (fitted, UNVERIFIED). The default, `Ok(None)`, is "not read". Reads only.
    fn lod_trailing_in(&self, package: &ow_package::package::Package, index: i32) -> Result<Option<Vec<usize>>, String> {
        let _ = (package, index);
        Ok(None)
    }
}

/// The layer's service: the installed source and the cache by mesh identity.
pub struct SkeletalMeshes {
    source: Box<dyn SkeletonSource>,
    cache: DetMap<ObjRef, Rc<ReferenceSkeleton>>,
    /// The composed reference poses, by mesh identity ([`reference_space_bases`]).
    poses: DetMap<ObjRef, Rc<Vec<SpaceBase>>>,
}

impl SkeletalMeshes {
    pub fn new(source: Box<dyn SkeletonSource>) -> Self {
        SkeletalMeshes {
            source,
            cache: DetMap::new(),
            poses: DetMap::new(),
        }
    }

    /// Installs the layer in `vm` (a harness input; at most once per VM).
    pub fn install(vm: &mut Vm, source: Box<dyn SkeletonSource>) -> Result<(), VmError> {
        vm.add_service(SkeletalMeshes::new(source)).map(|_| ())
    }
}

fn script(message: String) -> VmError {
    VmError::Script {
        function: UNIT.to_string(),
        message: format!("{UNIT}: {message}"),
    }
}

/// The store's file package, object path and object name of `o`, from its export table entry (the entry's own
/// spelling, not the name table's first one: review r1 of #569); nothing is loaded. A mesh whose package holds another
/// mesh export of the same name (in another group) is refused: a flat export layout keyed by package and name cannot
/// tell the two apart (review r1 of #569). "A mesh" is decided by class identity and ancestry (review r2): the other
/// export's class, by its package-qualified path, is looked up in the loaded class table (read only) and must be
/// `SkeletalMesh` or a subclass. A class the table does not hold cannot be settled without a load, so it is refused
/// too, naming the class.
fn identity(vm: &Vm, o: ObjRef) -> Result<(String, String, String), VmError> {
    let x = vm.space.heap.get(o)?;
    let interned = vm.space.names.text(x.name).into_owned();
    if let Origin::Export { package, index } | Origin::Resource { package, index } = x.origin {
        let pkg = vm.defs.packages().get(package);
        if let (Ok(path), Ok(entry)) = (ow_defs::identity::full_path(pkg, index), pkg.object(index))
        {
            let leaf = entry.name.clone();
            let base = vm.space.classes.by_path(MESH_CLASS);
            for (i, e) in pkg.exports.iter().enumerate() {
                if i64::try_from(i + 1).ok() == Some(i64::from(index))
                    || !e.name.eq_ignore_ascii_case(&leaf)
                    || e.class == 0
                {
                    continue;
                }
                let class_path = ow_defs::identity::full_path(pkg, e.class).unwrap_or_default();
                match vm.space.classes.by_path(&class_path) {
                    Some(c) if base.is_some_and(|b| vm.space.classes.is_child_of(c, b)) => {
                        return Err(NotYet::layer_absent(
                            format!(
                                "{UNIT}: {path}, whose package holds another mesh named {leaf} (the flat export layout is \
                                ambiguous)"
                            ),
                            LAYER,
                            RECORD,
                        )
                        .into());
                    }
                    Some(_) => {}
                    None => {
                        return Err(NotYet::layer_absent(
                            format!(
                                "{UNIT}: {path}, whose package holds another export named {leaf} of the class \
                                {class_path}, not in the loaded classes (whether it is a mesh cannot be settled)"
                            ),
                            LAYER,
                            RECORD,
                        )
                        .into());
                    }
                }
            }
            return Ok((pkg.name.clone(), path, leaf));
        }
    }
    Err(NotYet::layer_absent(
        format!("{UNIT}: {interned}, an object with no package export"),
        LAYER,
        RECORD,
    )
    .into())
}

/// Refuses an object the store has not loaded (module documentation, "Write-free").
fn loaded(vm: &Vm, o: ObjRef, what: &str) -> Result<(), VmError> {
    let stand_in = {
        let x = vm.space.heap.get(o)?;
        x.slots.is_empty()
            && vm
                .space
                .classes
                .get_layout(x.class)
                .is_some_and(|l| l.slot_count() > 0)
    };
    if vm.defs.is_unloaded(&vm.space, o) || stand_in {
        let leaf = vm.space.names.text(vm.space.heap.get(o)?.name).into_owned();
        return Err(NotYet::layer_absent(
            format!("{UNIT}: the unloaded {what} {leaf}"),
            LOAD_LAYER,
            RECORD,
        )
        .into());
    }
    Ok(())
}

fn check_mesh(vm: &Vm, mesh: ObjRef) -> Result<(), VmError> {
    let class = vm.space.heap.get(mesh)?.class;
    let is_mesh = vm
        .space
        .classes
        .by_path(MESH_CLASS)
        .is_some_and(|m| vm.space.classes.is_child_of(class, m));
    if !is_mesh {
        let path = vm
            .space
            .classes
            .get_layout(class)
            .map_or_else(String::new, |l| l.path().as_str().to_string());
        return Err(script(format!("{path} is not a SkeletalMesh")));
    }
    loaded(vm, mesh, "skeletal mesh")
}

/// `mesh`'s reference skeleton (module documentation). Reads only; cached by identity.
pub fn reference_skeleton(vm: &Vm, mesh: ObjRef) -> Result<Rc<ReferenceSkeleton>, VmError> {
    check_mesh(vm, mesh)?;
    let Some(cell) = vm.service::<SkeletalMeshes>() else {
        return Err(NotYet::layer_absent(
            format!("{UNIT}: no skeleton source is installed"),
            LAYER,
            RECORD,
        )
        .into());
    };
    if let Some(hit) = cell
        .try_borrow()
        .map_err(|_| VmError::ServiceBorrowed("SkeletalMeshes"))?
        .cache
        .get(&mesh)
    {
        return Ok(Rc::clone(hit));
    }
    let (package, path, leaf) = identity(vm, mesh)?;
    let read = cell
        .try_borrow()
        .map_err(|_| VmError::ServiceBorrowed("SkeletalMeshes"))?
        .source
        .skeleton(&package, &path, &leaf);
    let skeleton = match read {
        Ok(Some(s)) => s,
        Ok(None) => {
            return Err(NotYet::layer_absent(
                format!("{UNIT}: no reference skeleton for {path}"),
                LAYER,
                RECORD,
            )
            .into());
        }
        Err(e) => {
            return Err(script(format!(
                "the reference skeleton of {path} failed to parse: {e}"
            )));
        }
    };
    check_skeleton(&skeleton, &path)?;
    let skeleton = Rc::new(skeleton);
    cell.try_borrow_mut()
        .map_err(|_| VmError::ServiceBorrowed("SkeletalMeshes"))?
        .cache
        .insert(mesh, Rc::clone(&skeleton));
    Ok(skeleton)
}

/// A source's answer must be a skeleton: at least one bone, a root first, every other bone's parent before it, and no
/// two bone names equal under ASCII case folding (names compare case-insensitively; review r1 of #569). The source's
/// spelling and order are kept.
fn check_skeleton(s: &ReferenceSkeleton, path: &str) -> Result<(), VmError> {
    if s.bones.is_empty() {
        return Err(script(format!(
            "the reference skeleton of {path} has no bones"
        )));
    }
    let mut seen: ow_core::DetSet<String> = ow_core::DetSet::new();
    for (i, b) in s.bones.iter().enumerate() {
        if !seen.insert(b.name.to_ascii_lowercase()) {
            return Err(script(format!(
                "the reference skeleton of {path}: bone {i}'s name repeats an earlier bone's"
            )));
        }
    }
    for (i, b) in s.bones.iter().enumerate() {
        let ok = match b.parent {
            None => i == 0,
            Some(p) => i > 0 && p < i,
        };
        if !ok {
            return Err(script(format!(
                "the reference skeleton of {path}: bone {i} is out of parent order"
            )));
        }
    }
    Ok(())
}

fn prop(vm: &Vm, o: ObjRef, name: &str) -> Result<Value, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm
        .space
        .prop(class, name)
        .ok_or_else(|| script(format!("an object without {name}")))?;
    Ok(vm.space.read(o, PropAddr::scalar(p))?.clone())
}

fn field<'a>(vm: &Vm, v: &'a Value, name: &str) -> Result<&'a Value, VmError> {
    let missing = || script(format!("a value without the field {name}"));
    let s = v.as_struct().ok_or_else(missing)?;
    let id = vm.space.names.find(name).ok_or_else(missing)?;
    let (i, _) = vm
        .space
        .classes
        .struct_layout(s.ty)
        .and_then(|l| l.field(id))
        .ok_or_else(missing)?;
    s.fields
        .get(usize::try_from(i).map_err(|_| missing())?)
        .ok_or_else(missing)
}

fn name_of(vm: &Vm, v: &Value, what: &str) -> Result<String, VmError> {
    match v {
        Value::Name(n) => Ok(vm.space.names.text(*n).into_owned()),
        other => Err(script(format!(
            "{what} holds {}, not a name",
            other.kind_name()
        ))),
    }
}

fn floats(vm: &Vm, v: &Value, names: [&str; 3], what: &str) -> Result<[f32; 3], VmError> {
    let mut out = [0.0; 3];
    for (o, n) in out.iter_mut().zip(names) {
        *o = match field(vm, v, n)? {
            Value::Float(f) => *f,
            other => {
                return Err(script(format!(
                    "{what}.{n} holds {}, not a float",
                    other.kind_name()
                )));
            }
        };
    }
    Ok(out)
}

fn ints(vm: &Vm, v: &Value, names: [&str; 3], what: &str) -> Result<[i32; 3], VmError> {
    let mut out = [0; 3];
    for (o, n) in out.iter_mut().zip(names) {
        *o = match field(vm, v, n)? {
            Value::Int(i) => *i,
            other => {
                return Err(script(format!(
                    "{what}.{n} holds {}, not an int",
                    other.kind_name()
                )));
            }
        };
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------------------------
// The bone-transform layer (lane IMPL-BONE-TRANSFORM-LAYER, NATIVE_BONE_TRANSFORM_LAYER.md section 6, #646 at 94ab4339;
// clean room). Two separate answers: the consumers' read ([`stored_space_bases`], the buffer as stored) and the fill
// paths' plan ([`pose_update`], no fill path is ported yet). Every rule is UNVERIFIED in the running game.
// ---------------------------------------------------------------------------------------------------------------

/// The record of the bone-transform layer's refusals.
pub const BONE_RECORD: &str = "docs/verification/NATIVE_BONE_TRANSFORM_LAYER.md";
/// The struct `SpaceBases` holds.
pub const BONE_ATOM: &str = "Core.Object.BoneAtom";
/// `pose_update`'s rule 2.
pub const UNLOADED_MESH: &str =
    "the bone-transform layer: the mesh is an unloaded handle (a load is a write)";
/// `pose_update`'s rule 3: its layer and text.
pub const PARENT_COPY: &str = "the bone-transform layer's parent copy";
pub const PARENT_COPY_TEXT: &str = "the bone-transform layer: the bones of a ParentAnimComponent";
/// `pose_update`'s rule 4: its layer and text.
pub const TREE_POSE: &str = "the animation tree's pose";
pub const TREE_POSE_TEXT: &str = "the bone-transform layer: the animation tree's evaluation";
/// `pose_update`'s rule 5.
pub const ROOT_MOTION: &str = "the bone-transform layer: the discarded root motion";

/// A field's declared kind, as the layer and its natives check it (section 6.3's field kinds).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Object,
    Bool,
    Int,
}

/// Field `name` of `o`: declared on its class with `kind` (by the declaration, not the value), else
/// `VmError::Script` naming it for `function`; then its value, read without a load.
pub fn read_kind(
    vm: &Vm,
    o: ObjRef,
    name: &str,
    kind: Kind,
    function: &str,
) -> Result<Value, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let path = || vm.space.classes.layout(class).path().to_string();
    let bad = || VmError::Script {
        function: function.to_string(),
        message: format!(
            "{function}: {}.{name} is not declared as {}",
            path(),
            kind_name(kind)
        ),
    };
    let p = vm.space.prop(class, name).ok_or_else(bad)?;
    let declared = vm.space.classes.decl(p).map(|d| &d.kind);
    let ok = matches!(
        (kind, declared),
        (Kind::Object, Some(PropKind::Object { .. }))
            | (Kind::Bool, Some(PropKind::Bool))
            | (Kind::Int, Some(PropKind::Int))
    );
    if !ok {
        return Err(bad());
    }
    Ok(vm.space.read(o, PropAddr::scalar(p))?.clone())
}

fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Object => "an object",
        Kind::Bool => "a bool",
        Kind::Int => "an int",
    }
}

/// Whether `o` is resident: not an unloaded or failed handle, and not a stand-in with no slots whose class has some.
pub fn resident(vm: &Vm, o: ObjRef) -> Result<bool, VmError> {
    let x = vm.space.heap.get(o)?;
    let stand_in = x.slots.is_empty()
        && vm
            .space
            .classes
            .get_layout(x.class)
            .is_some_and(|l| l.slot_count() > 0);
    Ok(!vm.defs.is_unloaded(&vm.space, o) && !stand_in)
}

/// The consumers' read (section 6.2): `component`'s `SpaceBases` entries as stored, possibly empty. Its only rule:
/// `SpaceBases` declared as an array of [`BONE_ATOM`], else `VmError::Script` naming it. It never computes a pose.
/// Read-only, load-free, RNG-free and intern-free.
pub fn stored_space_bases(vm: &Vm, component: ObjRef) -> Result<Vec<Value>, VmError> {
    let class = vm.space.heap.get(component)?.class;
    let path = || vm.space.classes.layout(class).path().to_string();
    let bad = || {
        script(format!(
            "{}.SpaceBases is not declared as an array of {BONE_ATOM}",
            path()
        ))
    };
    let p = vm.space.prop(class, "SpaceBases").ok_or_else(bad)?;
    let atoms = match vm.space.classes.decl(p).map(|d| &d.kind) {
        Some(PropKind::Array(inner)) => match inner.as_ref() {
            PropKind::Struct(ty) => vm
                .space
                .classes
                .struct_layout(*ty)
                .is_some_and(|l| l.path().as_str() == BONE_ATOM),
            _ => false,
        },
        _ => false,
    };
    if !atoms {
        return Err(bad());
    }
    Ok(vm
        .space
        .read(component, PropAddr::scalar(p))?
        .elements()
        .map(<[Value]>::to_vec)
        .unwrap_or_default())
}

/// The fill paths' plan (section 6.3).
#[derive(Debug, Clone, PartialEq)]
pub enum PoseUpdate {
    /// Write nothing.
    Skip,
    /// The new `SpaceBases`, one entry per bone of the mesh.
    Write(Rc<Vec<SpaceBase>>),
}

/// The pose update a fill path writes in its own write phase (section 6.3). Write-free, load-free, RNG-free and
/// intern-free. The field kinds are checked first, each by its declaration (else `VmError::Script` naming it):
/// `SkeletalMesh`, `Animations` and `ParentAnimComponent` objects; `bSkipUpdateSkelPose` and
/// `bForceDiscardRootMotion` bools; `bForceRefpose` an int (non-zero is set); `SpaceBases` as [`stored_space_bases`]
/// checks it. Then, the first that applies:
/// 1. no `SkeletalMesh`, or `bSkipUpdateSkelPose` set: [`PoseUpdate::Skip`];
/// 2. the mesh not resident: `NotYet { NotRead }`, [`UNLOADED_MESH`];
/// 3. a set `ParentAnimComponent`: `NotYet { LayerAbsent(`[`PARENT_COPY`]`) }`;
/// 4. a set `Animations` with `bForceRefpose` zero: `NotYet { LayerAbsent(`[`TREE_POSE`]`) }`;
/// 5. `bForceDiscardRootMotion` set: `NotYet { NotPorted }`, [`ROOT_MOTION`];
/// 6. otherwise [`PoseUpdate::Write`] with [`reference_space_bases`]. The update-rate and not-rendered skips are taken
///    as not skipping (UNVERIFIED).
pub fn pose_update(vm: &Vm, component: ObjRef) -> Result<PoseUpdate, VmError> {
    const F: &str = "the bone-transform layer";
    let object = |n: &str| match read_kind(vm, component, n, Kind::Object, F)? {
        Value::Object(o) => Ok(o),
        other => Err(script(format!(
            "{n} holds {}, not an object",
            other.kind_name()
        ))),
    };
    let boolean = |n: &str| match read_kind(vm, component, n, Kind::Bool, F)? {
        Value::Bool(b) => Ok(b),
        other => Err(script(format!(
            "{n} holds {}, not a bool",
            other.kind_name()
        ))),
    };
    let mesh = object("SkeletalMesh")?;
    let animations = object("Animations")?;
    let parent = object("ParentAnimComponent")?;
    let skip = boolean("bSkipUpdateSkelPose")?;
    let discard = boolean("bForceDiscardRootMotion")?;
    let force_refpose = match read_kind(vm, component, "bForceRefpose", Kind::Int, F)? {
        Value::Int(i) => i != 0,
        other => {
            return Err(script(format!(
                "bForceRefpose holds {}, not an int",
                other.kind_name()
            )));
        }
    };
    stored_space_bases(vm, component)?;
    // Rule 1.
    let Some(mesh) = mesh else {
        return Ok(PoseUpdate::Skip);
    };
    if skip {
        return Ok(PoseUpdate::Skip);
    }
    // Rule 2, the residency first: NotRead (a direct call of the layer's functions keeps their own LayerAbsent).
    if !resident(vm, mesh)? {
        return Err(NotYet::not_read(UNLOADED_MESH, BONE_RECORD).into());
    }
    // Rules 3 to 5.
    if parent.is_some() {
        return Err(NotYet::layer_absent(PARENT_COPY_TEXT, PARENT_COPY, BONE_RECORD).into());
    }
    if animations.is_some() && !force_refpose {
        return Err(NotYet::layer_absent(TREE_POSE_TEXT, TREE_POSE, BONE_RECORD).into());
    }
    if discard {
        return Err(NotYet::not_ported(ROOT_MOTION, BONE_RECORD).into());
    }
    // Rule 6.
    Ok(PoseUpdate::Write(reference_space_bases(vm, mesh)?))
}

/// The reference pose's text when a bone of the skeleton has no bind pose.
pub const NO_BIND_POSE: &str =
    "the skeletal mesh data layer: a reference skeleton without bind poses";

/// `mesh`'s reference pose in component space (lane IMPL-BONE-TRANSFORM-LAYER, NATIVE_BONE_TRANSFORM_LAYER.md section
/// 6.1): each bone's bind pose composed onto its parent's composed transform, root first, every scale 1. A child's
/// rotation is its parent's composed rotation times its own; its translation is the parent's translation plus the
/// parent's rotation applied to its own. Every bone counts as required (an UNVERIFIED simplification). A skeleton with
/// any bone's bind pose unknown is `NotYet { LayerAbsent }` with [`LAYER`] and [`NO_BIND_POSE`], and nothing is cached.
/// Reads only; cached by mesh identity, and the skeleton through [`reference_skeleton`]'s cache.
pub fn reference_space_bases(vm: &Vm, mesh: ObjRef) -> Result<Rc<Vec<SpaceBase>>, VmError> {
    let skeleton = reference_skeleton(vm, mesh)?;
    let Some(cell) = vm.service::<SkeletalMeshes>() else {
        return Err(NotYet::layer_absent(
            format!("{UNIT}: no skeleton source is installed"),
            LAYER,
            RECORD,
        )
        .into());
    };
    if let Some(hit) = cell
        .try_borrow()
        .map_err(|_| VmError::ServiceBorrowed("SkeletalMeshes"))?
        .poses
        .get(&mesh)
    {
        return Ok(Rc::clone(hit));
    }
    let mut out: Vec<SpaceBase> = Vec::with_capacity(skeleton.len());
    for bone in &skeleton.bones {
        let Some(bind) = bone.bind else {
            return Err(NotYet::layer_absent(NO_BIND_POSE, LAYER, BONE_RECORD).into());
        };
        let atom = match bone.parent {
            None => SpaceBase {
                rotation: bind.rotation,
                translation: bind.translation,
                scale: 1.0,
            },
            Some(p) => {
                let parent = out[p];
                let r = rotate(parent.rotation, bind.translation);
                SpaceBase {
                    rotation: quat_mul(parent.rotation, bind.rotation),
                    translation: [
                        parent.translation[0] + r[0],
                        parent.translation[1] + r[1],
                        parent.translation[2] + r[2],
                    ],
                    scale: 1.0,
                }
            }
        };
        out.push(atom);
    }
    let out = Rc::new(out);
    cell.try_borrow_mut()
        .map_err(|_| VmError::ServiceBorrowed("SkeletalMeshes"))?
        .poses
        .insert(mesh, Rc::clone(&out));
    Ok(out)
}

/// Whether the composed-pose cache holds `mesh` (tests only: a refusal caches nothing).
#[cfg(test)]
pub(crate) fn has_cached_pose(vm: &Vm, mesh: ObjRef) -> bool {
    vm.service::<SkeletalMeshes>()
        .is_some_and(|c| c.borrow().poses.contains_key(&mesh))
}

/// The Hamilton product `a b` of two `(x, y, z, w)` quaternions, in single precision.
fn quat_mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let [ax, ay, az, aw] = a;
    let [bx, by, bz, bw] = b;
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}

/// `v` rotated by the unit quaternion `q` (`q v q*`), in single precision.
fn rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let [x, y, z, _] = quat_mul(
        quat_mul(q, [v[0], v[1], v[2], 0.0]),
        [-q[0], -q[1], -q[2], q[3]],
    );
    [x, y, z]
}

/// The package export `o` came from, by its validated origin (review r1 of #673, item 4): the store's package index,
/// the export index and the object path. `None` for an object with no export origin, or one whose entry or path the
/// package cannot give. Unlike [`identity`], no flat-name guard: an in-package read is keyed by export index, so two
/// meshes of one name in different groups stay apart.
fn export_of(vm: &Vm, o: ObjRef) -> Result<Option<(u32, i32, String)>, VmError> {
    let (Origin::Export { package, index } | Origin::Resource { package, index }) = vm.space.heap.get(o)?.origin else {
        return Ok(None);
    };
    let pkg = vm.defs.packages().get(package);
    if pkg.object(index).is_err() {
        return Ok(None);
    }
    Ok(ow_defs::identity::full_path(pkg, index).ok().map(|path| (package, index, path)))
}

/// `mesh`'s LOD model count, when the layer can give one (lane IMPL-SKELMESH-MESH-CHANGE): `Ok(None)` with no source
/// installed, for an object the layer cannot place in a package export, or when the source does not know. The
/// in-package answer is asked first, by the mesh's export origin; only the name-based fallback goes through
/// [`identity`] and its flat-name guard. Reads only; a source's parse failure is `VmError::Script`. `mesh` must be a
/// resident `SkeletalMesh` ([`check_mesh`]).
pub fn lod_count(vm: &Vm, mesh: ObjRef) -> Result<Option<usize>, VmError> {
    check_mesh(vm, mesh)?;
    let Some(cell) = vm.service::<SkeletalMeshes>() else {
        return Ok(None);
    };
    let layer = cell.try_borrow().map_err(|_| VmError::ServiceBorrowed("SkeletalMeshes"))?;
    if let Some((at, index, path)) = export_of(vm, mesh)? {
        let pkg = vm.defs.packages().get(at);
        let read = layer.source.lod_count_in(pkg, index);
        if let Some(n) = read.map_err(|e| script(format!("the LOD count of {path} failed to parse: {e}")))? {
            return Ok(Some(n));
        }
    }
    let (package, path, leaf) = match identity(vm, mesh) {
        Ok(id) => id,
        Err(VmError::NotYet(_)) => return Ok(None),
        Err(e) => return Err(e),
    };
    let read = layer.source.lod_count(&package, &path, &leaf);
    read.map_err(|e| script(format!("the LOD count of {path} failed to parse: {e}")))
}

/// `mesh`'s LOD models' trailing counts, one per LOD (lane SKELMESH-LOD-LAYER; the source's
/// [`SkeletonSource::lod_trailing_in`], fitted and UNVERIFIED): `Ok(None)` with no source installed, for an object
/// the layer cannot place in a package export ([`export_of`]), or when the source does not read the LOD models.
/// Reads only; a source's parse failure is `VmError::Script`. `mesh` must be a resident `SkeletalMesh`
/// ([`check_mesh`]).
pub fn lod_trailing_counts(vm: &Vm, mesh: ObjRef) -> Result<Option<Vec<usize>>, VmError> {
    check_mesh(vm, mesh)?;
    let Some(cell) = vm.service::<SkeletalMeshes>() else {
        return Ok(None);
    };
    let Some((at, index, path)) = export_of(vm, mesh)? else {
        return Ok(None);
    };
    let layer = cell.try_borrow().map_err(|_| VmError::ServiceBorrowed("SkeletalMeshes"))?;
    let pkg = vm.defs.packages().get(at);
    layer.source.lod_trailing_in(pkg, index).map_err(|e| script(format!("the LOD models of {path} failed to parse: {e}")))
}

/// A `Sockets` entry as the socket object it names, refused as the module documentation says: a None entry is
/// `NotYet { NotRead }`, an unloaded one `NotYet { LayerAbsent }` (the store's load), another kind `VmError::Script`.
fn socket_object(vm: &Vm, item: &Value) -> Result<ObjRef, VmError> {
    let s = match item {
        Value::Object(Some(s)) => *s,
        Value::Object(None) => {
            return Err(NotYet::not_read(format!("{UNIT}: a None entry in a mesh's Sockets"), RECORD).into());
        }
        other => return Err(script(format!("a Sockets entry holds {}, not an object", other.kind_name()))),
    };
    loaded(vm, s, "socket")?;
    Ok(s)
}

fn name_id(v: &Value, what: &str) -> Result<NameId, VmError> {
    match v {
        Value::Name(n) => Ok(*n),
        other => Err(script(format!("{what} holds {}, not a name", other.kind_name()))),
    }
}

fn read_socket(vm: &Vm, s: ObjRef) -> Result<MeshSocket, VmError> {
    Ok(MeshSocket {
        name: name_of(vm, &prop(vm, s, "SocketName")?, "SocketName")?,
        bone: name_of(vm, &prop(vm, s, "BoneName")?, "BoneName")?,
        location: floats(vm, &prop(vm, s, "RelativeLocation")?, ["X", "Y", "Z"], "RelativeLocation")?,
        rotation: ints(vm, &prop(vm, s, "RelativeRotation")?, ["Pitch", "Yaw", "Roll"], "RelativeRotation")?,
        scale: floats(vm, &prop(vm, s, "RelativeScale")?, ["X", "Y", "Z"], "RelativeScale")?,
    })
}

fn mesh_sockets(vm: &Vm, mesh: ObjRef) -> Result<Vec<Value>, VmError> {
    check_mesh(vm, mesh)?;
    let list = prop(vm, mesh, "Sockets")?;
    Ok(list.elements().ok_or_else(|| script("Sockets is not an array".to_string()))?.to_vec())
}

/// `mesh`'s sockets, in its `Sockets` order (module documentation). Reads only.
pub fn sockets(vm: &Vm, mesh: ObjRef) -> Result<Vec<MeshSocket>, VmError> {
    let items = mesh_sockets(vm, mesh)?;
    let mut out = Vec::with_capacity(items.len());
    for item in &items {
        let s = socket_object(vm, item)?;
        out.push(read_socket(vm, s)?);
    }
    Ok(out)
}

/// A socket [`find_socket`] found, with its `BoneName` as the name table's id.
#[derive(Debug, Clone, PartialEq)]
pub struct FoundSocket {
    pub socket: MeshSocket,
    pub bone: NameId,
}

/// The first of `mesh`'s sockets whose `SocketName` is `name` (lane IMPL-BODYCOMP-SOCKET-BONE-TABLES,
/// NATIVE_BODYCOMP_SOCKET_BONE_TABLES.md section 4, rule 3). It walks `Sockets` in order, refuses each entry it
/// reaches as [`sockets`] does, compares name ids (the table's one id per text: the base case-blind, the instance
/// number exactly), and reads nothing after the first match. Reads only; loads and interns nothing.
pub fn find_socket(vm: &Vm, mesh: ObjRef, name: NameId) -> Result<Option<FoundSocket>, VmError> {
    for item in &mesh_sockets(vm, mesh)? {
        let s = socket_object(vm, item)?;
        if name_id(&prop(vm, s, "SocketName")?, "SocketName")? != name {
            continue;
        }
        let socket = read_socket(vm, s)?;
        let bone = name_id(&prop(vm, s, "BoneName")?, "BoneName")?;
        return Ok(Some(FoundSocket { socket, bone }));
    }
    Ok(None)
}

/// The index of the bone of `mesh`'s reference skeleton named `name` (the note's section 4, rule 4): each bone's text
/// is looked up in the name table without interning (`NameTable::find`, its own split into base and number) and
/// compared by id, so a text the table has never seen is no name. Reads only.
pub fn find_bone(vm: &Vm, mesh: ObjRef, name: NameId) -> Result<Option<usize>, VmError> {
    let skeleton = reference_skeleton(vm, mesh)?;
    Ok(skeleton.bones.iter().position(|b| vm.space.names.find(&b.name) == Some(name)))
}

#[cfg(test)]
#[path = "skeletal_tests.rs"]
mod tests;
