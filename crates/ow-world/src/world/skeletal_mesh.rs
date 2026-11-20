//! Owner: lane IMPL-SET-SKELETAL-MESH, then lane IMPL-SKELMESH-MESH-CHANGE (the mesh change).
//! `SkeletalMeshComponent.SetSkeletalMesh(NewMesh, optional bKeepSpaceBases)` in `ParityMode::Off`. It is coded from
//! docs/verification/NATIVE_SET_SKELETAL_MESH.md (#583 at 231bbaed) and, for a different mesh, from
//! docs/verification/NATIVE_SKELMESH_MESH_CHANGE.md section 4 alone. Clean room: lanes ANALYST-SET-SKELETAL-MESH and
//! ANALYST-SKELMESH-MESH-CHANGE read the native, these lanes did not. AI-assisted (Claude Code). **Every rule here is
//! UNVERIFIED in the running game.** Reference binds nothing, so the VM's stub policy answers there.
//!
//! **The answer**, in this order. Every refusal writes nothing, loads nothing, draws nothing and interns no name:
//! 1. **the receiver:** `this` must be set and resident; an unloaded definition handle is `NotYet { NotRead }`;
//! 2. **the arguments:** `NewMesh` is an object (`Object(None)` is null), and `bKeepSpaceBases`, when supplied
//!    (`NativeCall::has`), a bool; an omitted one is false. Anything else is `VmError::Script`;
//! 3. **the current mesh:** `SkeletalMesh`, read straight from the component's slot. A class with no `SkeletalMesh`
//!    object field is `VmError::Script` (review r2 of #599, L2);
//! 4. **identity first,** before either mesh's residency is looked at: equal handles (both None, or the same handle,
//!    loaded or not) are the native's own "nothing to do";
//! 5. **a different mesh** (NATIVE_SKELMESH_MESH_CHANGE.md section 4, its one authoritative Off order, rows 5 to 15
//!    and D) is planned by [`plan_change`] and then written by [`apply_change`]:
//!    - rows 5 and 6: the old mesh, when set, then the new mesh, when set, resident (`NotYet { NotRead }`); a clear
//!      skips row 6;
//!    - rows 7 to 15, each refused before any write in the native's step order: kept transforms over a non-empty
//!      buffer, a physics instance, an attached component, an animation tree, **the transform** (the hub's ruling: a
//!      non-zero origin offset or rotation on the old, then the new mesh is `LayerAbsent`; zero is nothing), a parent
//!      animation component, a physics asset, morph sets or a non-None morph-target map, and `LODInfo` (a new mesh
//!      only: `LayerAbsent` unless #569's layer gives a known LOD count of zero and `LODInfo` is empty);
//!    - row D: the four destinations declared with their kinds, else `VmError::Script`;
//!    - the four writes, in the native's order: `bAnimTreeInitialised` false, `SkeletalMesh`, `SpaceBases` emptied and
//!      `bHasValidBodies` false.

use std::rc::Rc;

use ow_core::NotYet;
use ow_object::ids::{ObjRef, PropAddr, PropId};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

/// The native's registry key.
pub const SET_SKELETAL_MESH: &str = "SkeletalMeshComponent.SetSkeletalMesh(object,bool)";
/// The record of the same-mesh rule.
pub const RECORD: &str = "NATIVE_SET_SKELETAL_MESH.md";
/// The record of the mesh change.
pub const CHANGE_RECORD: &str = "docs/verification/NATIVE_SKELMESH_MESH_CHANGE.md";
/// The layer that keeping the bone transforms over a mesh change needs.
pub const BONES_LAYER: &str = "a skeletal mesh component's bone transforms";
/// The layer for a physics-asset instance.
pub const PHYSICS_INSTANCE_LAYER: &str = "a skeletal physics instance";
/// The layer for the bone-name matches (the parent bone map, the physics bodies flag).
pub const BONE_NAMES_LAYER: &str = "the skeletal mesh data layer's bone names";
/// The layer for the mesh's LOD models.
pub const LOD_LAYER: &str = "the skeletal mesh data layer's LOD models";
/// The layer for the component transform (the hub's ruling on the note's rule 7).
pub const TRANSFORM_LAYER: &str = "a component transform";
const UNIT: &str = "SetSkeletalMesh";

/// Each refusal of a mesh change (NATIVE_SKELMESH_MESH_CHANGE.md section 4).
pub const NEW_MESH_UNLOADED: &str = "SetSkeletalMesh: the new mesh is an unloaded handle (a load is a write)";
pub const OLD_MESH_UNLOADED: &str = "SetSkeletalMesh: the old mesh is an unloaded handle (a load is a write)";
pub const NEW_MESH_STAND_IN: &str = "SetSkeletalMesh: the new mesh is a stand-in with no properties (an object the store cannot load)";
pub const OLD_MESH_STAND_IN: &str = "SetSkeletalMesh: the old mesh is a stand-in with no properties (an object the store cannot load)";
pub const KEEP_BONES: &str = "SetSkeletalMesh: keeping the bone transforms over a mesh change";
pub const PHYSICS_INSTANCE: &str = "SetSkeletalMesh: the physics-asset instance's termination";
pub const TREE_STATE: &str = "SetSkeletalMesh: the animation tree's per-bone state on a mesh change";
pub const REATTACH: &str = "SetSkeletalMesh: an attached component's detach and re-attach (the pose update; see \
                            ANALYST-BODYCOMP-SOCKET-BONE-TABLES)";
pub const PARENT_BONE_MAP: &str = "SetSkeletalMesh: the parent bone map from ParentAnimComponent's mesh";
pub const VALID_BODIES: &str = "SetSkeletalMesh: bHasValidBodies from the physics asset's bodies";
pub const MORPH_MAP: &str = "SetSkeletalMesh: the morph-target index map";
pub const LOD_INFO: &str = "SetSkeletalMesh: LODInfo and its per-LOD usage from the new mesh's LOD models";
pub const TRANSFORM: &str =
    "SetSkeletalMesh: the component transform's origin compensation (a mesh with a non-zero origin)";

/// Binds the native (module documentation) unless the key is already bound (keep-existing); the caller binds it in
/// Off only.
pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.has_native(SET_SKELETAL_MESH) {
        return Ok(());
    }
    vm.register_native(SET_SKELETAL_MESH, Rc::new(set_skeletal_mesh))
}

fn script(message: String) -> VmError {
    VmError::Script { function: UNIT.to_string(), message: format!("{UNIT}: {message}") }
}

fn set_skeletal_mesh(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let component = c.this.ok_or_else(|| script("called with no component".to_string()))?;
    // Rule 1 before the argument shapes (the note's section 4 order; review r1 of #599, item 4).
    resident(c.vm, component)?;
    let new_mesh = match c.arg(0)? {
        Value::Object(o) => *o,
        other => return Err(script(format!("NewMesh holds {}, not an object reference", other.kind_name()))),
    };
    let keep_space_bases = if c.has(1) {
        match c.arg(1)? {
            Value::Bool(b) => *b,
            other => return Err(script(format!("bKeepSpaceBases holds {}, not a bool", other.kind_name()))),
        }
    } else {
        false
    };
    answer(c.vm, component, new_mesh, keep_space_bases)?;
    Ok(Value::None)
}

/// Rule 1: the receiver is resident. An unloaded handle is `NotRead`; one whose load already failed says so
/// (review r1 of #599, item 6).
fn resident(vm: &Vm, component: ObjRef) -> Result<(), VmError> {
    if vm.defs.is_failed(&vm.space, component) {
        return Err(NotYet::not_read(format!("{UNIT}: the component is a handle whose load failed"), RECORD).into());
    }
    if vm.defs.is_unloaded(&vm.space, component) {
        return Err(NotYet::not_read(
            format!("{UNIT}: the component is an unloaded handle (a load is a write)"),
            RECORD,
        )
        .into());
    }
    Ok(())
}

/// The rule on its inputs (module documentation): the preflight, then, for a different mesh, the writes.
pub fn answer(vm: &mut Vm, component: ObjRef, new_mesh: Option<ObjRef>, keep_space_bases: bool) -> Result<(), VmError> {
    let Some(plan) = plan_change(vm, component, new_mesh, keep_space_bases)? else {
        return Ok(());
    };
    apply_change(vm, &plan)
}

/// What a mesh change writes, settled by [`plan_change`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    component: ObjRef,
    new_mesh: Option<ObjRef>,
    tree_initialised: PropId,
    mesh: PropId,
    space_bases: PropId,
    valid_bodies: PropId,
}

/// The preflight (module documentation): `None` when the mesh is already `new_mesh`, else the planned writes.
/// Write-free, load-free, RNG-free and intern-free.
pub fn plan_change(vm: &Vm, component: ObjRef, new_mesh: Option<ObjRef>, keep: bool) -> Result<Option<Change>, VmError> {
    resident(vm, component)?;
    let current = current_mesh(vm, component)?;
    if current == new_mesh {
        return Ok(None);
    }
    // B. Rows 5 and 6: the old mesh, then the new one (a clear skips row 6).
    if let Some(m) = current {
        mesh_resident(vm, m, OLD_MESH_UNLOADED, OLD_MESH_STAND_IN)?;
    }
    if let Some(m) = new_mesh {
        mesh_resident(vm, m, NEW_MESH_UNLOADED, NEW_MESH_STAND_IN)?;
    }
    let layer = |what: &'static str, l: &'static str| -> VmError { NotYet::layer_absent(what, l, CHANGE_RECORD).into() };
    let class = class_of(vm, component)?;
    // C. Rows 7 to 15, in the native's step order.
    let space_bases = vm.space.prop(class, "SpaceBases");
    if keep && current.is_some() {
        let p = decl(vm, component, "SpaceBases", "an array", is_array)?;
        if !elements(vm, component, p, "SpaceBases")?.is_empty() {
            return Err(layer(KEEP_BONES, BONES_LAYER));
        }
    }
    if object_set(vm, component, "PhysicsAssetInstance")? {
        return Err(layer(PHYSICS_INSTANCE, PHYSICS_INSTANCE_LAYER));
    }
    let attached = decl(vm, component, "bAttached", "a bool", is_bool)?;
    if read(vm, component, attached)? == Value::Bool(true) {
        return Err(NotYet::not_read(REATTACH, CHANGE_RECORD).into());
    }
    if object_set(vm, component, "Animations")? {
        return Err(NotYet::not_ported(TREE_STATE, CHANGE_RECORD).into());
    }
    // Row 11, the hub's ruling: the old mesh, then the new one.
    for m in [current, new_mesh].into_iter().flatten() {
        if !origin_zero(vm, m)? {
            return Err(layer(TRANSFORM, TRANSFORM_LAYER));
        }
    }
    if object_set(vm, component, "ParentAnimComponent")? {
        return Err(layer(PARENT_BONE_MAP, BONE_NAMES_LAYER));
    }
    if object_set(vm, component, "PhysicsAsset")? {
        return Err(layer(VALID_BODIES, BONE_NAMES_LAYER));
    }
    let morph_sets = decl(vm, component, "MorphSets", "an array", is_array)?;
    if !elements(vm, component, morph_sets, "MorphSets")?.is_empty() {
        return Err(NotYet::not_ported(MORPH_MAP, CHANGE_RECORD).into());
    }
    // A morph-target map that reads as anything but None (the port's only empty value) is the same refusal.
    if let Some(p) = vm.space.prop(class, "MorphTargetIndexMap") {
        if !matches!(kind(vm, p), Some(PropKind::Map { .. })) {
            return Err(script(format!("{}.MorphTargetIndexMap is not a map", class_path(vm, component)?)));
        }
        if read(vm, component, p)? != Value::None {
            return Err(NotYet::not_ported(MORPH_MAP, CHANGE_RECORD).into());
        }
    }
    // Row 15, a new mesh only: the LOD count and the per-LOD usage come from #569's layer, which gives no count today.
    if let Some(m) = new_mesh {
        let lod_info = decl(vm, component, "LODInfo", "an array", is_array)?;
        let empty = elements(vm, component, lod_info, "LODInfo")?.is_empty();
        if !(empty && crate::world::skeletal::lod_count(vm, m)? == Some(0)) {
            return Err(layer(LOD_INFO, LOD_LAYER));
        }
    }
    // D. The destinations, each declared with its kind (the same-mesh rule already checked `SkeletalMesh`).
    let tree_initialised = decl(vm, component, "bAnimTreeInitialised", "a bool", is_bool)?;
    let mesh = decl(vm, component, "SkeletalMesh", "an object field", |k| matches!(k, PropKind::Object { .. }))?;
    let space_bases = match space_bases {
        Some(p) if matches!(kind(vm, p), Some(k) if is_array(k)) => p,
        _ => decl(vm, component, "SpaceBases", "an array", is_array)?,
    };
    let valid_bodies = decl(vm, component, "bHasValidBodies", "a bool", is_bool)?;
    Ok(Some(Change { component, new_mesh, tree_initialised, mesh, space_bases, valid_bodies }))
}

/// The writes of a planned change, in the native's order. A failure after the first write is marked "stopped after
/// partial writes: " by #518's counters.
pub fn apply_change(vm: &mut Vm, c: &Change) -> Result<(), VmError> {
    let before = crate::pools::Counters::read(vm);
    let writes = [
        (c.tree_initialised, Value::Bool(false)),
        (c.mesh, Value::Object(c.new_mesh)),
        (c.space_bases, Value::array(Vec::new())),
        (c.valid_bodies, Value::Bool(false)),
    ];
    for (p, v) in writes {
        vm.space
            .write(c.component, PropAddr::scalar(p), v)
            .map_err(|e| crate::pools::after_counted_writes(vm, before, VmError::from(e)))?;
    }
    Ok(())
}

fn class_of(vm: &Vm, o: ObjRef) -> Result<ow_object::ids::ClassId, VmError> {
    Ok(vm.space.heap.get(o)?.class)
}

fn class_path(vm: &Vm, o: ObjRef) -> Result<String, VmError> {
    Ok(vm.space.classes.layout(class_of(vm, o)?).path().to_string())
}

fn kind(vm: &Vm, p: PropId) -> Option<&PropKind> {
    vm.space.classes.decl(p).map(|d| &d.kind)
}

fn is_array(k: &PropKind) -> bool {
    matches!(k, PropKind::Array(_))
}

fn is_bool(k: &PropKind) -> bool {
    matches!(k, PropKind::Bool)
}

/// `name` on `o`'s class, declared with a kind `ok` accepts, else a `VmError::Script` naming the field.
fn decl(vm: &Vm, o: ObjRef, name: &str, want: &str, ok: fn(&PropKind) -> bool) -> Result<PropId, VmError> {
    let path = class_path(vm, o)?;
    let p = vm.space.prop(class_of(vm, o)?, name).ok_or_else(|| script(format!("{path} declares no {name}")))?;
    if !matches!(kind(vm, p), Some(k) if ok(k)) {
        return Err(script(format!("{path}.{name} is not {want}")));
    }
    Ok(p)
}

fn read(vm: &Vm, o: ObjRef, p: PropId) -> Result<Value, VmError> {
    Ok(vm.space.read(o, PropAddr::scalar(p))?.clone())
}

fn elements(vm: &Vm, o: ObjRef, p: PropId, what: &str) -> Result<Vec<Value>, VmError> {
    match read(vm, o, p)? {
        Value::Array(a) => Ok(a.as_ref().clone()),
        other => Err(script(format!("{what} holds {}, not an array", other.kind_name()))),
    }
}

/// Whether the object field `name` (declared as an object, else `VmError::Script`) is set.
fn object_set(vm: &Vm, o: ObjRef, name: &str) -> Result<bool, VmError> {
    let p = decl(vm, o, name, "an object field", |k| matches!(k, PropKind::Object { .. }))?;
    match read(vm, o, p)? {
        Value::Object(v) => Ok(v.is_some()),
        other => Err(script(format!("{name} holds {}, not an object reference", other.kind_name()))),
    }
}

/// Rules 4 and 5: `mesh` is neither an unloaded (or failed) store handle nor a stand-in with no slots.
fn mesh_resident(vm: &Vm, mesh: ObjRef, unloaded: &'static str, stand_in: &'static str) -> Result<(), VmError> {
    if vm.defs.is_unloaded(&vm.space, mesh) {
        return Err(NotYet::not_read(unloaded, CHANGE_RECORD).into());
    }
    let obj = vm.space.heap.get(mesh)?;
    let fields = vm.space.classes.get_layout(obj.class).map_or(0, |l| l.slot_count());
    if obj.slots.is_empty() && fields > 0 {
        return Err(NotYet::not_read(stand_in, CHANGE_RECORD).into());
    }
    Ok(())
}

/// Whether `mesh`'s `Origin` (a struct of three floats, by fit `Vector`) and `RotOrigin` (a struct of three ints, by
/// fit `Rotator`) are all zero; `-0.0` counts as zero. Another declaration, or none, is a `VmError::Script` naming the
/// field (row 11).
fn origin_zero(vm: &Vm, mesh: ObjRef) -> Result<bool, VmError> {
    let mut zero = true;
    for (name, want, member) in [("Origin", "a Vector struct", PropKind::Float), ("RotOrigin", "a Rotator struct", PropKind::Int)] {
        let path = class_path(vm, mesh)?;
        let bad = || script(format!("{path}.{name} is not {want}"));
        let p = vm.space.prop(class_of(vm, mesh)?, name).ok_or_else(|| script(format!("{path} declares no {name}")))?;
        let Some(PropKind::Struct(id)) = kind(vm, p) else {
            return Err(bad());
        };
        let fields = vm.space.classes.struct_layout(*id).map(|l| l.fields().map(|(_, f)| f.kind.clone()).collect::<Vec<_>>());
        if fields.as_deref() != Some(&[member.clone(), member.clone(), member][..]) {
            return Err(bad());
        }
        let Value::Struct(v) = read(vm, mesh, p)? else {
            return Err(bad());
        };
        for f in &v.fields {
            zero &= match f {
                Value::Float(x) => *x == 0.0,
                Value::Int(i) => *i == 0,
                _ => return Err(bad()),
            };
        }
    }
    Ok(zero)
}

/// The component's `SkeletalMesh`, read from its slot without a load.
fn current_mesh(vm: &Vm, component: ObjRef) -> Result<Option<ObjRef>, VmError> {
    let class = vm.space.heap.get(component)?.class;
    let path = || vm.space.classes.layout(class).path().to_string();
    let p = vm.space.prop(class, "SkeletalMesh").ok_or_else(|| script(format!("{} declares no SkeletalMesh", path())))?;
    if !matches!(vm.space.classes.decl(p).map(|d| &d.kind), Some(PropKind::Object { .. })) {
        return Err(script(format!("{}.SkeletalMesh is not an object field", path())));
    }
    match vm.space.read(component, PropAddr::scalar(p))? {
        Value::Object(o) => Ok(*o),
        other => Err(script(format!("{}.SkeletalMesh holds {}, not an object reference", path(), other.kind_name()))),
    }
}

#[cfg(test)]
#[path = "skeletal_mesh_tests.rs"]
mod tests;
