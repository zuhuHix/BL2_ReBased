//! Owner: lane IMPL-SKELETAL-MESH-LAYER (decision 2026-11-19-NNN, number pending). The skeletal mesh data layer, read
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
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

/// The layer, as the consumers' refusals name it (#529).
pub const LAYER: &str = "a skeletal mesh data reader (sockets and the reference skeleton)";
/// The record of the layer.
pub const RECORD: &str = "docs/decisions/2026-11/2026-11-19-NNN-skeletal-mesh-data-layer.md";
const UNIT: &str = "the skeletal mesh data layer";
const MESH_CLASS: &str = "Engine.SkeletalMesh";
const LOAD_LAYER: &str = "the definition store's load (the skeletal mesh data layer loads nothing)";

/// One bone of a reference skeleton: its name and its parent's index (`None` for the root).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkeletonBone {
    pub name: String,
    pub parent: Option<usize>,
}

/// A mesh's reference skeleton: its bones in order; every bone's parent comes before it.
#[derive(Debug, Clone, PartialEq, Eq)]
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
        self.bones.iter().position(|b| b.name.eq_ignore_ascii_case(name))
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
}

/// The layer's service: the installed source and the cache by mesh identity.
pub struct SkeletalMeshes {
    source: Box<dyn SkeletonSource>,
    cache: DetMap<ObjRef, Rc<ReferenceSkeleton>>,
}

impl SkeletalMeshes {
    pub fn new(source: Box<dyn SkeletonSource>) -> Self {
        SkeletalMeshes { source, cache: DetMap::new() }
    }

    /// Installs the layer in `vm` (a harness input; at most once per VM).
    pub fn install(vm: &mut Vm, source: Box<dyn SkeletonSource>) -> Result<(), VmError> {
        vm.add_service(SkeletalMeshes::new(source)).map(|_| ())
    }
}

fn script(message: String) -> VmError {
    VmError::Script { function: UNIT.to_string(), message: format!("{UNIT}: {message}") }
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
        if let (Ok(path), Ok(entry)) = (ow_defs::identity::full_path(pkg, index), pkg.object(index)) {
            let leaf = entry.name.clone();
            let base = vm.space.classes.by_path(MESH_CLASS);
            for (i, e) in pkg.exports.iter().enumerate() {
                if i64::try_from(i + 1).ok() == Some(i64::from(index)) || !e.name.eq_ignore_ascii_case(&leaf) || e.class == 0 {
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
    Err(NotYet::layer_absent(format!("{UNIT}: {interned}, an object with no package export"), LAYER, RECORD).into())
}

/// Refuses an object the store has not loaded (module documentation, "Write-free").
fn loaded(vm: &Vm, o: ObjRef, what: &str) -> Result<(), VmError> {
    let stand_in = {
        let x = vm.space.heap.get(o)?;
        x.slots.is_empty() && vm.space.classes.get_layout(x.class).is_some_and(|l| l.slot_count() > 0)
    };
    if vm.defs.is_unloaded(&vm.space, o) || stand_in {
        let leaf = vm.space.names.text(vm.space.heap.get(o)?.name).into_owned();
        return Err(NotYet::layer_absent(format!("{UNIT}: the unloaded {what} {leaf}"), LOAD_LAYER, RECORD).into());
    }
    Ok(())
}

fn check_mesh(vm: &Vm, mesh: ObjRef) -> Result<(), VmError> {
    let class = vm.space.heap.get(mesh)?.class;
    let is_mesh = vm.space.classes.by_path(MESH_CLASS).is_some_and(|m| vm.space.classes.is_child_of(class, m));
    if !is_mesh {
        let path = vm.space.classes.get_layout(class).map_or_else(String::new, |l| l.path().as_str().to_string());
        return Err(script(format!("{path} is not a SkeletalMesh")));
    }
    loaded(vm, mesh, "skeletal mesh")
}

/// `mesh`'s reference skeleton (module documentation). Reads only; cached by identity.
pub fn reference_skeleton(vm: &Vm, mesh: ObjRef) -> Result<Rc<ReferenceSkeleton>, VmError> {
    check_mesh(vm, mesh)?;
    let Some(cell) = vm.service::<SkeletalMeshes>() else {
        return Err(NotYet::layer_absent(format!("{UNIT}: no skeleton source is installed"), LAYER, RECORD).into());
    };
    if let Some(hit) = cell.try_borrow().map_err(|_| VmError::ServiceBorrowed("SkeletalMeshes"))?.cache.get(&mesh) {
        return Ok(Rc::clone(hit));
    }
    let (package, path, leaf) = identity(vm, mesh)?;
    let read = cell.try_borrow().map_err(|_| VmError::ServiceBorrowed("SkeletalMeshes"))?.source.skeleton(&package, &path, &leaf);
    let skeleton = match read {
        Ok(Some(s)) => s,
        Ok(None) => {
            return Err(NotYet::layer_absent(format!("{UNIT}: no reference skeleton for {path}"), LAYER, RECORD).into());
        }
        Err(e) => return Err(script(format!("the reference skeleton of {path} failed to parse: {e}"))),
    };
    check_skeleton(&skeleton, &path)?;
    let skeleton = Rc::new(skeleton);
    cell.try_borrow_mut().map_err(|_| VmError::ServiceBorrowed("SkeletalMeshes"))?.cache.insert(mesh, Rc::clone(&skeleton));
    Ok(skeleton)
}

/// A source's answer must be a skeleton: at least one bone, a root first, every other bone's parent before it, and no
/// two bone names equal under ASCII case folding (names compare case-insensitively; review r1 of #569). The source's
/// spelling and order are kept.
fn check_skeleton(s: &ReferenceSkeleton, path: &str) -> Result<(), VmError> {
    if s.bones.is_empty() {
        return Err(script(format!("the reference skeleton of {path} has no bones")));
    }
    let mut seen: ow_core::DetSet<String> = ow_core::DetSet::new();
    for (i, b) in s.bones.iter().enumerate() {
        if !seen.insert(b.name.to_ascii_lowercase()) {
            return Err(script(format!("the reference skeleton of {path}: bone {i}'s name repeats an earlier bone's")));
        }
    }
    for (i, b) in s.bones.iter().enumerate() {
        let ok = match b.parent {
            None => i == 0,
            Some(p) => i > 0 && p < i,
        };
        if !ok {
            return Err(script(format!("the reference skeleton of {path}: bone {i} is out of parent order")));
        }
    }
    Ok(())
}

fn prop(vm: &Vm, o: ObjRef, name: &str) -> Result<Value, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| script(format!("an object without {name}")))?;
    Ok(vm.space.read(o, PropAddr::scalar(p))?.clone())
}

fn field<'a>(vm: &Vm, v: &'a Value, name: &str) -> Result<&'a Value, VmError> {
    let missing = || script(format!("a value without the field {name}"));
    let s = v.as_struct().ok_or_else(missing)?;
    let id = vm.space.names.find(name).ok_or_else(missing)?;
    let (i, _) = vm.space.classes.struct_layout(s.ty).and_then(|l| l.field(id)).ok_or_else(missing)?;
    s.fields.get(usize::try_from(i).map_err(|_| missing())?).ok_or_else(missing)
}

fn name_of(vm: &Vm, v: &Value, what: &str) -> Result<String, VmError> {
    match v {
        Value::Name(n) => Ok(vm.space.names.text(*n).into_owned()),
        other => Err(script(format!("{what} holds {}, not a name", other.kind_name()))),
    }
}

fn floats(vm: &Vm, v: &Value, names: [&str; 3], what: &str) -> Result<[f32; 3], VmError> {
    let mut out = [0.0; 3];
    for (o, n) in out.iter_mut().zip(names) {
        *o = match field(vm, v, n)? {
            Value::Float(f) => *f,
            other => return Err(script(format!("{what}.{n} holds {}, not a float", other.kind_name()))),
        };
    }
    Ok(out)
}

fn ints(vm: &Vm, v: &Value, names: [&str; 3], what: &str) -> Result<[i32; 3], VmError> {
    let mut out = [0; 3];
    for (o, n) in out.iter_mut().zip(names) {
        *o = match field(vm, v, n)? {
            Value::Int(i) => *i,
            other => return Err(script(format!("{what}.{n} holds {}, not an int", other.kind_name()))),
        };
    }
    Ok(out)
}

/// `mesh`'s sockets, in its `Sockets` order (module documentation). Reads only.
pub fn sockets(vm: &Vm, mesh: ObjRef) -> Result<Vec<MeshSocket>, VmError> {
    check_mesh(vm, mesh)?;
    let list = prop(vm, mesh, "Sockets")?;
    let items = list.elements().ok_or_else(|| script("Sockets is not an array".to_string()))?;
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let s = match item {
            Value::Object(Some(s)) => *s,
            Value::Object(None) => {
                return Err(NotYet::not_read(format!("{UNIT}: a None entry in a mesh's Sockets"), RECORD).into());
            }
            other => return Err(script(format!("a Sockets entry holds {}, not an object", other.kind_name()))),
        };
        loaded(vm, s, "socket")?;
        out.push(MeshSocket {
            name: name_of(vm, &prop(vm, s, "SocketName")?, "SocketName")?,
            bone: name_of(vm, &prop(vm, s, "BoneName")?, "BoneName")?,
            location: floats(vm, &prop(vm, s, "RelativeLocation")?, ["X", "Y", "Z"], "RelativeLocation")?,
            rotation: ints(vm, &prop(vm, s, "RelativeRotation")?, ["Pitch", "Yaw", "Roll"], "RelativeRotation")?,
            scale: floats(vm, &prop(vm, s, "RelativeScale")?, ["X", "Y", "Z"], "RelativeScale")?,
        });
    }
    Ok(out)
}

#[cfg(test)]
#[path = "skeletal_tests.rs"]
mod tests;
