//! Owner: lanes IMPL-SET-SKELETAL-MESH and IMPL-SKELMESH-MESH-CHANGE. `SkeletalMeshComponent.SetSkeletalMesh(NewMesh,
//! optional bKeepSpaceBases)` in `ParityMode::Off`, coded from docs/verification/NATIVE_SET_SKELETAL_MESH.md (#583 at
//! 231bbaed) and, for a different mesh, docs/verification/NATIVE_SKELMESH_MESH_CHANGE.md (#635) alone (clean room: the
//! analyst lanes read the native, these lanes did not). AI-assisted (Claude Code). **Every rule here is UNVERIFIED in
//! the running game.** Reference binds nothing, so the VM's stub policy answers there.
//!
//! **The answer** (NATIVE_SKELMESH_MESH_CHANGE.md section 4, the one authoritative Off order). The first row that
//! applies is the answer; every row reads only (no write, no load, no draw, no name interned), and nothing is written
//! until every row has passed.
//!
//! **A. #599's order, unchanged:**
//! 1. the receiver set and resident: an unloaded definition handle, or an object with fewer slots than its class's
//!    layout (a stand-in or a cut object, [`CUT`]), is `NotYet { NotRead }`. That settles every slot the rule reads
//!    or writes on the component before any of them is touched (FIX-645-PREFLIGHT);
//! 2. the arguments: `NewMesh` an object (`Object(None)` is null), and `bKeepSpaceBases`, when supplied
//!    (`NativeCall::has`), a bool; an omitted one is false. Anything else is `VmError::Script`;
//! 3. the current mesh: `SkeletalMesh` from the component's slot. A class with no `SkeletalMesh` object field is
//!    `VmError::Script`;
//! 4. equal handles (both None, or the same handle, loaded or not): the native's own "nothing to do".
//!
//! **B. The meshes:** 5. the old mesh, when set, resident; 6. the new mesh, when set, resident (a clear skips it). An
//! unloaded or a failed handle is `NotYet { NotRead }` ([`OLD_UNLOADED`], [`NEW_UNLOADED`]); a resident one with fewer
//! slots than its class's layout (a stand-in or a cut object) is the same kind with its own text ([`OLD_CUT`],
//! [`NEW_CUT`]).
//!
//! **C. The unported paths, in the native's step order** (the note's step in brackets):
//! 7. [1] `bKeepSpaceBases` with a current mesh and a non-empty `SpaceBases`: `LayerAbsent(`[`BONES_LAYER`]`)`;
//! 8. [2] a set `PhysicsAssetInstance`: `LayerAbsent(`[`PHYSICS_INSTANCE_LAYER`]`)`;
//! 9. [5] `bAttached`: `NotRead`, the detach and re-attach with its pose update ([`ATTACHED`]);
//! 10. [8] a set `Animations`: `NotPorted`, the tree's per-bone state ([`TREE`]);
//! 11. [9] the transform (the hub's ruling, UNVERIFIED): each set mesh's `Origin` (a [`VECTOR`]: `X`, `Y`, `Z` floats)
//!     and `RotOrigin` (a [`ROTATOR`]: `Pitch`, `Yaw`, `Roll` ints), the old mesh's first. Another declaration, or
//!     none, is `VmError::Script` naming the field; so is a struct of another type with the same members, declared or
//!     only stored in the slot (FIX-645-PREFLIGHT and its review r1, as `actor_trace` checks its vectors). Any member non-zero (`-0.0` is zero): `LayerAbsent(`[`TRANSFORM_LAYER`]`)`.
//!     All zero, or no mesh: nothing is answered for the transform;
//! 12. [10] a set `ParentAnimComponent`: `LayerAbsent(`[`BONE_NAMES_LAYER`]`)`, the parent bone map;
//! 13. [11] a set `PhysicsAsset`: the same layer, for `bHasValidBodies`;
//! 14. [15] a non-empty `MorphSets`, or a `MorphTargetIndexMap` that is not None: `NotPorted` ([`MORPH`]);
//! 15. [16] a new mesh only (lane IMPL-ROW15, on the hub's two rulings): the LOD count N
//!     (`skeletal::lod_count`) and each LOD model's trailing count (`skeletal::lod_trailing_counts`, #673). No count,
//!     no trailing list, a list of another length, or any non-zero trailing count: `LayerAbsent(`[`LOD_LAYER`]`)`.
//!     All zero: no LOD has the per-LOD data, so step 16's usage loop does nothing (**UNVERIFIED**: #673's fit that a
//!     zero trailing count stands for "no such data", ruled by the hub). Then, when `LODInfo`'s length differs from
//!     N, a refill is needed (D); when it equals N, nothing.
//!
//! **D. The destinations:** `bAnimTreeInitialised` (a bool), `SkeletalMesh` (an object field taking the new mesh),
//! `SpaceBases` (an array) and `bHasValidBodies` (a bool), each declared on the component's class with that kind, and
//! each planned value accepted by the heap's own check. With a new mesh, `LODInfo` too: a dynamic array of a struct.
//! Anything else is `VmError::Script` naming the field. When row 15 needs a refill, the entry struct must declare
//! exactly [`LOD_ENTRY_MEMBERS`] by name (otherwise `NotYet { NotRead }`, [`LOD_ENTRY`]). Each refilled entry is the
//! natively constructed one, built member by member ([`lod_entry`]; the note's amendment of #687, read): empty
//! `HiddenMaterials`, both bools false, `InstanceWeightUsage` 0 (`IWU_PartialSwap`) and `InstanceWeightIdx` -1. It is
//! never the script default, which holds index 0.
//!
//! **The writes**, only after A to D, in the native's order ([4], [6], [7], [11], [16]): `bAnimTreeInitialised` false,
//! `SkeletalMesh` the new mesh (None on a clear), `SpaceBases` the empty array and `bHasValidBodies` false, each
//! written even when unchanged; then, when row 15 needs a refill, `LODInfo` = N constructed entries (N = 0 empties
//! it). An equal length writes no `LODInfo`.
//! `MorphTargetIndexMap`, the transform, the native-only field of step 12 and step 17's notification are not
//! written. A `NotYet` raised after the first write would be marked "stopped after partial
//! writes: " (none is expected: row 1 settles every slot and D every declaration and value, which is all a heap write
//! checks).

use std::rc::Rc;

use ow_core::NotYet;
use ow_object::ids::{ObjRef, PropAddr, PropId};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

use crate::actors::{flag, object, objects};

/// The native's registry key.
pub const SET_SKELETAL_MESH: &str = "SkeletalMeshComponent.SetSkeletalMesh(object,bool)";
/// The record of #599's rows (A).
pub const RECORD: &str = "NATIVE_SET_SKELETAL_MESH.md";
/// The record of the mesh change's rows (B to D).
pub const CHANGE_RECORD: &str = "docs/verification/NATIVE_SKELMESH_MESH_CHANGE.md";
/// Row 7's layer: the component's bone transforms.
pub const BONES_LAYER: &str = "a skeletal mesh component's bone transforms";
/// Row 8's layer.
pub const PHYSICS_INSTANCE_LAYER: &str = "a skeletal physics instance";
/// Row 11's layer.
pub const TRANSFORM_LAYER: &str = "a component transform";
/// Rows 12 and 13's layer.
pub const BONE_NAMES_LAYER: &str = "the skeletal mesh data layer's bone names";
/// Row 15's layer.
pub const LOD_LAYER: &str = "the skeletal mesh data layer's LOD models";
/// Row 5's text.
pub const OLD_UNLOADED: &str = "SetSkeletalMesh: the old mesh is an unloaded handle (a load is a write)";
/// Row 6's text.
pub const NEW_UNLOADED: &str = "SetSkeletalMesh: the new mesh is an unloaded handle (a load is a write)";
/// Row 5's text for a resident old mesh with fewer slots than its class.
pub const OLD_CUT: &str =
    "SetSkeletalMesh: the old mesh has fewer slots than its class (a stand-in or a cut object; its fields are not read)";
/// Row 6's text for a resident new mesh with fewer slots than its class.
pub const NEW_CUT: &str =
    "SetSkeletalMesh: the new mesh has fewer slots than its class (a stand-in or a cut object; its fields are not read)";
/// Row 7's text.
pub const KEEP: &str = "SetSkeletalMesh: keeping the bone transforms over a mesh change";
/// Row 8's text.
pub const PHYSICS_INSTANCE: &str = "SetSkeletalMesh: the physics-asset instance's termination";
/// Row 9's text.
pub const ATTACHED: &str =
    "SetSkeletalMesh: an attached component's detach and re-attach (the pose update; see ANALYST-BODYCOMP-SOCKET-BONE-TABLES)";
/// Row 10's text.
pub const TREE: &str = "SetSkeletalMesh: the animation tree's per-bone state on a mesh change";
/// Row 11's text.
pub const TRANSFORM: &str =
    "SetSkeletalMesh: the component transform's origin compensation (a mesh with a non-zero origin)";
/// Row 12's text.
pub const PARENT_BONES: &str = "SetSkeletalMesh: the parent bone map (ParentAnimComponent's bone names)";
/// Row 13's text.
pub const VALID_BODIES: &str = "SetSkeletalMesh: bHasValidBodies (the PhysicsAsset's bodies against the new mesh's bones)";
/// Row 14's text.
pub const MORPH: &str = "SetSkeletalMesh: the morph-target index map";
/// Row 15's text.
pub const LOD: &str = "SetSkeletalMesh: the LOD info from the new mesh's LOD models";
/// D's refusal when `LODInfo`'s entry struct is not declared with [`LOD_ENTRY_MEMBERS`].
pub const LOD_ENTRY: &str = "SetSkeletalMesh: the LOD info entry's declaration";
/// `Engine.SkelMeshComponentLODInfo`'s members and kinds, as the runner read the installed packages (2026-11-25).
pub const LOD_ENTRY_MEMBERS: [(&str, &str); 5] = [
    ("HiddenMaterials", "array"),
    ("bNeedsInstanceWeightUpdate", "bool"),
    ("bAlwaysUseInstanceWeights", "bool"),
    ("InstanceWeightUsage", "byte"),
    ("InstanceWeightIdx", "int"),
];
/// Row 1's text for a receiver with fewer slots than its class's layout.
pub const CUT: &str =
    "SetSkeletalMesh: the component has fewer slots than its class (a stand-in or a cut object; its fields are not read)";
/// Row 11's `Origin` struct.
pub const VECTOR: &str = "Core.Object.Vector";
/// Row 11's `RotOrigin` struct.
pub const ROTATOR: &str = "Core.Object.Rotator";
const UNIT: &str = "SetSkeletalMesh";

/// Binds the native (module documentation); the caller binds it in Off only.
pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    vm.register_native(SET_SKELETAL_MESH, Rc::new(set_skeletal_mesh))
}

fn script(message: String) -> VmError {
    VmError::Script { function: UNIT.to_string(), message: format!("{UNIT}: {message}") }
}

fn set_skeletal_mesh(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let component = c.this.ok_or_else(|| script("called with no component".to_string()))?;
    // Row 1 before the argument shapes (review r1 of #599, item 4).
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
    if let Some(plan) = answer(c.vm, component, new_mesh, keep_space_bases)? {
        plan.apply(c.vm)?;
    }
    Ok(Value::None)
}

/// Row 1: the receiver is resident. An unloaded handle is `NotRead`; one whose load already failed says so
/// (review r1 of #599, item 6); one with fewer slots than its class's layout is [`CUT`].
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
    if short(vm, component)? {
        return Err(NotYet::not_read(CUT, RECORD).into());
    }
    Ok(())
}

/// The mesh change's four writes, settled by the preflight (module documentation, D). Private with [`answer`] and
/// [`Plan::apply`], so no caller can change the receiver between the two (review r1 of #664, item 2).
#[derive(Debug)]
struct Plan {
    component: ObjRef,
    writes: Vec<(PropId, Value)>,
}

impl Plan {
    /// The writes, in the native's order. A `NotYet` after the first is marked "stopped after partial writes: ".
    fn apply(self, vm: &mut Vm) -> Result<(), VmError> {
        for (i, (p, v)) in self.writes.into_iter().enumerate() {
            if let Err(e) = vm.space.write(self.component, PropAddr::scalar(p), v) {
                let e = VmError::from(e);
                return Err(if i > 0 { crate::pools::after_partial_writes(e) } else { e });
            }
        }
        Ok(())
    }
}

/// The rule on its inputs (module documentation): `Ok(None)` when the mesh is already `new_mesh`, `Ok(Some(plan))` for
/// a change every row lets through. Reads only.
fn answer(vm: &Vm, component: ObjRef, new_mesh: Option<ObjRef>, keep_space_bases: bool) -> Result<Option<Plan>, VmError> {
    resident(vm, component)?;
    let current = current_mesh(vm, component)?;
    // Row 4.
    if current == new_mesh {
        return Ok(None);
    }
    let space = &vm.space;
    // Rows 5 and 6.
    if let Some(old) = current {
        mesh_resident(vm, old, OLD_UNLOADED, OLD_CUT)?;
    }
    if let Some(new) = new_mesh {
        mesh_resident(vm, new, NEW_UNLOADED, NEW_CUT)?;
    }
    // Row 7.
    if keep_space_bases && current.is_some() && array_len(vm, component, "SpaceBases")? > 0 {
        return Err(NotYet::layer_absent(KEEP, BONES_LAYER, CHANGE_RECORD).into());
    }
    // Row 8.
    if object(space, component, "PhysicsAssetInstance")?.is_some() {
        return Err(NotYet::layer_absent(PHYSICS_INSTANCE, PHYSICS_INSTANCE_LAYER, CHANGE_RECORD).into());
    }
    // Row 9.
    if flag(space, component, "bAttached")? {
        return Err(NotYet::not_read(ATTACHED, CHANGE_RECORD).into());
    }
    // Row 10.
    if object(space, component, "Animations")?.is_some() {
        return Err(NotYet::not_ported(TREE, CHANGE_RECORD).into());
    }
    // Row 11, the old mesh first.
    for mesh in [current, new_mesh].into_iter().flatten() {
        if !origin_zero(vm, mesh)? {
            return Err(NotYet::layer_absent(TRANSFORM, TRANSFORM_LAYER, CHANGE_RECORD).into());
        }
    }
    // Rows 12 and 13.
    if object(space, component, "ParentAnimComponent")?.is_some() {
        return Err(NotYet::layer_absent(PARENT_BONES, BONE_NAMES_LAYER, CHANGE_RECORD).into());
    }
    if object(space, component, "PhysicsAsset")?.is_some() {
        return Err(NotYet::layer_absent(VALID_BODIES, BONE_NAMES_LAYER, CHANGE_RECORD).into());
    }
    // Row 14.
    if !objects(space, component, "MorphSets")?.is_empty() || crate::actors::field(space, component, "MorphTargetIndexMap")? != Value::None
    {
        return Err(NotYet::not_ported(MORPH, CHANGE_RECORD).into());
    }
    // Row 15, a new mesh only: Some(N) when LODInfo is to be refilled with N default entries.
    let resize = match new_mesh {
        Some(new) => lod_resize(vm, component, new)?,
        None => None,
    };
    // D, the destinations.
    let mut writes = vec![
        destination(vm, component, "bAnimTreeInitialised", Value::Bool(false), |k| matches!(k, PropKind::Bool))?,
        destination(vm, component, "SkeletalMesh", Value::Object(new_mesh), |k| matches!(k, PropKind::Object { .. }))?,
        destination(vm, component, "SpaceBases", Value::array(Vec::new()), |k| matches!(k, PropKind::Array(_)))?,
        destination(vm, component, "bHasValidBodies", Value::Bool(false), |k| matches!(k, PropKind::Bool))?,
    ];
    if new_mesh.is_some() {
        let ty = lod_info_struct(vm, component)?;
        if let Some(n) = resize {
            // The natively constructed entry (#687's amendment), never the script default.
            let entry = lod_entry(vm, ty)?;
            let value = Value::array(vec![entry; n]);
            writes.push(destination(vm, component, "LODInfo", value, |k| matches!(k, PropKind::Array(_)))?);
        }
    }
    Ok(Some(Plan { component, writes }))
}

/// Whether `o` holds fewer slots than its class's layout: a stand-in (no slots) or a cut object.
fn short(vm: &Vm, o: ObjRef) -> Result<bool, VmError> {
    let x = vm.space.heap.get(o)?;
    Ok(vm.space.classes.get_layout(x.class).is_some_and(|l| x.slots.len() < l.slot_count() as usize))
}

/// Row 15 on the new mesh (module documentation): `Ok(Some(n))` when `LODInfo` is to be refilled with `n` default
/// entries, `Ok(None)` when its length is already the LOD count. Reads only.
fn lod_resize(vm: &Vm, component: ObjRef, new: ObjRef) -> Result<Option<usize>, VmError> {
    let absent = || -> VmError { NotYet::layer_absent(LOD, LOD_LAYER, CHANGE_RECORD).into() };
    let count = crate::world::skeletal::lod_count(vm, new)?.ok_or_else(absent)?;
    let trailing = crate::world::skeletal::lod_trailing_counts(vm, new)?.ok_or_else(absent)?;
    // The hub's ruling (UNVERIFIED, #673's fit): a zero trailing count stands for "no per-LOD data".
    if trailing.len() != count || trailing.iter().any(|&t| t != 0) {
        return Err(absent());
    }
    Ok((array_len(vm, component, "LODInfo")? != count).then_some(count))
}

/// D for `LODInfo` (module documentation): declared on the component's class as a dynamic array of a struct; its
/// struct. Anything else is `VmError::Script` naming the field.
fn lod_info_struct(vm: &Vm, component: ObjRef) -> Result<ow_object::ids::StructId, VmError> {
    let class = vm.space.heap.get(component)?.class;
    let path = || vm.space.classes.layout(class).path().to_string();
    let p = vm.space.prop(class, "LODInfo").ok_or_else(|| script(format!("{} declares no LODInfo", path())))?;
    match vm.space.classes.decl(p).map(|d| (&d.kind, d.array_dim)) {
        Some((PropKind::Array(inner), 1)) => match **inner {
            PropKind::Struct(ty) => Ok(ty),
            ref k => Err(script(format!("{}.LODInfo holds {}, not a struct", path(), k.kind_name()))),
        },
        _ => Err(script(format!("{}.LODInfo is not declared as a dynamic array", path()))),
    }
}

/// One natively constructed `LODInfo` entry of struct `ty` (NATIVE_SKELMESH_MESH_CHANGE.md, the amendment of #687,
/// read; UNVERIFIED in game): the shape checked by [`lod_entry_shape`], then every member set explicitly, by name:
/// `HiddenMaterials` empty, `bNeedsInstanceWeightUpdate` and `bAlwaysUseInstanceWeights` false, `InstanceWeightUsage`
/// 0 (`IWU_PartialSwap`), `InstanceWeightIdx` -1. The script default supplies only the value's shape; all five members
/// are overwritten, so nothing of it is copied.
fn lod_entry(vm: &Vm, ty: ow_object::ids::StructId) -> Result<Value, VmError> {
    lod_entry_shape(vm, ty)?;
    let refuse = || -> VmError { NotYet::not_read(LOD_ENTRY, CHANGE_RECORD).into() };
    let layout = vm.space.classes.struct_layout(ty).ok_or_else(refuse)?;
    let mut entry = vm.space.classes.struct_defaults(ty).map_err(|_| refuse())?;
    let values = [
        ("HiddenMaterials", Value::array(Vec::new())),
        ("bNeedsInstanceWeightUpdate", Value::Bool(false)),
        ("bAlwaysUseInstanceWeights", Value::Bool(false)),
        ("InstanceWeightUsage", Value::Byte(0)),
        ("InstanceWeightIdx", Value::Int(-1)),
    ];
    for (name, v) in values {
        let id = vm.space.names.find(name).ok_or_else(refuse)?;
        let (i, _) = layout.field(id).ok_or_else(refuse)?;
        let slot = entry.struct_mut().and_then(|s| s.fields.get_mut(i as usize)).ok_or_else(refuse)?;
        *slot = v;
    }
    Ok(entry)
}

/// `LODInfo`'s entry struct, by name (the hub's corrected ruling 2, from the runner's read of the installed
/// `Engine.SkelMeshComponentLODInfo`): exactly the five members of [`LOD_ENTRY_MEMBERS`], each with its kind and
/// `array_dim` 1, in any order. Anything else is `NotYet { NotRead }` ([`LOD_ENTRY`]).
fn lod_entry_shape(vm: &Vm, ty: ow_object::ids::StructId) -> Result<(), VmError> {
    let refuse = || -> VmError { NotYet::not_read(LOD_ENTRY, CHANGE_RECORD).into() };
    let layout = vm.space.classes.struct_layout(ty).ok_or_else(refuse)?;
    if layout.fields().count() != LOD_ENTRY_MEMBERS.len() {
        return Err(refuse());
    }
    for (name, kind) in LOD_ENTRY_MEMBERS {
        let id = vm.space.names.find(name).ok_or_else(refuse)?;
        let (_, decl) = layout.field(id).ok_or_else(refuse)?;
        let fits = decl.array_dim == 1
            && match kind {
                "array" => matches!(decl.kind, PropKind::Array(_)),
                "bool" => matches!(decl.kind, PropKind::Bool),
                "byte" => matches!(decl.kind, PropKind::Byte { .. }),
                _ => matches!(decl.kind, PropKind::Int),
            };
        if !fits {
            return Err(refuse());
        }
    }
    Ok(())
}

/// Rows 5 and 6: `mesh` resident. An unloaded or a failed handle is `NotRead` with `unloaded`; one with fewer slots
/// than its class's layout is `NotRead` with `cut` (review r1 of #664, item 3). Nothing is loaded.
fn mesh_resident(vm: &Vm, mesh: ObjRef, unloaded: &'static str, cut: &'static str) -> Result<(), VmError> {
    if vm.defs.is_unloaded(&vm.space, mesh) {
        return Err(NotYet::not_read(unloaded, CHANGE_RECORD).into());
    }
    if short(vm, mesh)? {
        return Err(NotYet::not_read(cut, CHANGE_RECORD).into());
    }
    Ok(())
}

/// The number of elements of the array field `name` of `o`; a field that is not an array is `VmError::Script`.
fn array_len(vm: &Vm, o: ObjRef, name: &str) -> Result<usize, VmError> {
    let v = crate::actors::field(&vm.space, o, name)?;
    v.elements().map(<[Value]>::len).ok_or_else(|| script(format!("{name} holds {}, not an array", v.kind_name())))
}

/// Row 11 on one mesh: whether its `Origin` and `RotOrigin` are all zero. The declarations are checked by their struct
/// type and their members (module documentation).
fn origin_zero(vm: &Vm, mesh: ObjRef) -> Result<bool, VmError> {
    let floats = members(vm, mesh, "Origin", VECTOR, ["X", "Y", "Z"], |v| match v {
        Value::Float(f) => Some(*f == 0.0),
        _ => None,
    })?;
    let ints = members(vm, mesh, "RotOrigin", ROTATOR, ["Pitch", "Yaw", "Roll"], |v| match v {
        Value::Int(i) => Some(*i == 0),
        _ => None,
    })?;
    Ok(floats && ints)
}

/// Whether every one of the three `names` of the struct field `field` of `mesh` is zero by `zero`; a field that is not
/// declared as the struct `expected` (by identity, not only by shape) with those three members of the expected kind is
/// `VmError::Script` naming it.
fn members(
    vm: &Vm,
    mesh: ObjRef,
    field: &str,
    expected: &str,
    names: [&str; 3],
    zero: impl Fn(&Value) -> Option<bool>,
) -> Result<bool, VmError> {
    let class = vm.space.heap.get(mesh)?.class;
    let path = || vm.space.classes.layout(class).path().to_string();
    let bad = || script(format!("{}.{field} is not declared as a struct of {}", path(), names.join(", ")));
    let p = vm.space.prop(class, field).ok_or_else(|| script(format!("{} declares no {field}", path())))?;
    let Some(PropKind::Struct(ty)) = vm.space.classes.decl(p).map(|d| d.kind.clone()) else {
        return Err(bad());
    };
    if vm.space.classes.struct_by_path(expected) != Some(ty) {
        return Err(script(format!("{}.{field} is a struct of another type, not a {expected}", path())));
    }
    let layout = vm.space.classes.struct_layout(ty).ok_or_else(bad)?;
    let value = vm.space.read(mesh, PropAddr::scalar(p))?;
    let s = value.as_struct().ok_or_else(bad)?;
    // The stored value too, not only the declaration (review r1 of #664, item 1).
    if s.ty != ty {
        return Err(script(format!("{}.{field} holds a struct of another type, not a {expected}", path())));
    }
    let mut all = true;
    for n in names {
        let id = vm.space.names.find(n).ok_or_else(bad)?;
        let (i, _) = layout.field(id).ok_or_else(bad)?;
        let v = s.fields.get(usize::try_from(i).map_err(|_| bad())?).ok_or_else(bad)?;
        all &= zero(v).ok_or_else(bad)?;
    }
    Ok(all)
}

/// D for one write: `name` declared on `component`'s class with a kind `kind_ok` accepts, and `value` accepted by the
/// heap's own check for it. Anything else is `VmError::Script` naming the field.
fn destination(
    vm: &Vm,
    component: ObjRef,
    name: &str,
    value: Value,
    kind_ok: impl Fn(&PropKind) -> bool,
) -> Result<(PropId, Value), VmError> {
    let class = vm.space.heap.get(component)?.class;
    let path = || vm.space.classes.layout(class).path().to_string();
    let p = vm.space.prop(class, name).ok_or_else(|| script(format!("{} declares no {name}", path())))?;
    let kind = vm.space.classes.decl(p).map(|d| d.kind.clone()).ok_or_else(|| script(format!("{name} has no declaration")))?;
    if !kind_ok(&kind) {
        return Err(script(format!("{}.{name} is declared as {}, not the kind the mesh change writes", path(), kind.kind_name())));
    }
    vm.space
        .heap
        .check_value(&vm.space.classes, &kind, &value)
        .map_err(|e| script(format!("{}.{name} cannot take the planned value: {e}", path())))?;
    Ok((p, value))
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

#[cfg(test)]
#[path = "skeletal_mesh_change_tests.rs"]
mod change_tests;
