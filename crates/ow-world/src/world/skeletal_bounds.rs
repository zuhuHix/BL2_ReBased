//! Owner: lane IMPL-SKELMESH-BOUNDS-SOURCE (decision 2026-11-25-41). The bounds of a live
//! `Engine.SkeletalMeshComponent` entry for `Actor.GetBoundingCylinder`, in Off. AI-assisted (Claude Code).
//!
//! Coded from NATIVE_COMPONENT_BOUNDS.md section 9 alone, as landed on main 6dad61f (#694, analyst lane
//! ANALYST-SKELMESH-BOUNDS-SOURCE; clean room: no native code or analysis output was opened). **Every rule is
//! UNVERIFIED in the running game**, and the field names are the note's by-fit names.
//!
//! The box routine reads a component's stored `Bounds`, which the skeletal bounds update writes; the port has no bounds
//! update, so [`skeletal_box`] recomputes at the query what that update would write (note section 9.2), in the note's
//! section 9.6 order. Reads are resident and by name only: nothing is written, loaded or drawn, and no name is
//! interned. A required object that is not resident is the bounding cylinder's refusal N. A field of the wrong kind is
//! a `VmError::Script` naming it (the world's field reads).
//!
//! 1. **A sleeping rigid body** (`Owner` set, its `Physics` the `EPhysics` value `PHYS_RigidBody`, and the
//!    component's `FramesPhysicsAsleep` above 5): `NotYet { NotRead }` ([`SLEEPING`]); the owner's further physics
//!    test is not read.
//! 2. **`bSkipUpdateSkelPose`** set: `NotYet { NotRead }` ([`SKIP_POSE`]); the port keeps no previous `Bounds`.
//! 3. **`DetailMode`** above 0: `NotYet { NotRead }` ([`DETAIL_MODE`]); the engine's detail mode is not in the port.
//!    Otherwise the component is visible when it is shown (`HiddenGame` clear, and no `Owner`, the owner's `bHidden`
//!    clear, or `bIgnoreOwnerHidden` set) or `bCastHiddenShadow` is set.
//! 4. **Source a** (not visible, or `bComponentUseFixedSkelBounds`): no `SkeletalMesh` is [`HIDDEN_NO_MESH`]'s
//!    `NotRead`; otherwise the mesh's stored bounds ([`stored_box`]).
//! 5. **Source b** (a `ParentAnimComponent` with a `SkeletalMesh` and its own `bComponentUseFixedSkelBounds`): the
//!    parent mesh's stored bounds through the accessor (the parent's flag is read before its mesh is required
//!    resident). While it answers none, the stored-bounds `LayerAbsent`. Once it answers, the world transform's
//!    `NotRead` unless the transform is the simplest one (below), and then [`PARENT_ROOT`]'s `NotRead`: the note did
//!    not check whether the root offset is added on this source.
//! 6. **Sources c and d**, each asked only when its object is there:
//!    - **c**, the component's own `PhysicsAsset` with `bHasValidBodies` set and "can use the bones": the bodies'
//!      `LayerAbsent` ([`BODIES_LAYER`]), its text naming the bone transforms' layer too;
//!    - **d**, a parent with a `PhysicsAsset` and "can use the bones": `bUseBoundsFromParentAnimComponent` set is the
//!      bounding cylinder's `BOUNDS_LAYER` (the parent's stored `Bounds`); otherwise c's refusal.
//!
//!    "Can use the bones" ([`can_use_bones`]): a uniform combined scale (`Scale` × `Scale3D`, times the owner's
//!    `DrawScale` × `DrawScale3D`, the three axes within 0.0001 of each other), a `SkeletalMesh`, and either
//!    `SpaceBases` as long as the mesh's bone count or a parent with a `PhysicsAsset`. With `SpaceBases` empty the
//!    count half is false. With it not empty, the bone count comes from the accessor; while that answers none, the
//!    stored-bounds `LayerAbsent`.
//! 7. **Source e** (no `SkeletalMesh`): the game's base rule (origin 0, a half extent of 262144) is **not answered**:
//!    `NotYet { NotRead }` ([`TEMPLATE`]), by the hub's revised ruling on the template gap (below).
//! 8. **Source f**: the mesh's stored bounds ([`stored_box`]).
//!
//! **The stored box** ([`stored_box`], sources a and f): the accessor ([`crate::world::skeletal::mesh_bounds`]); while
//! it answers none, `NotYet { LayerAbsent }` ([`SKELETAL_BOUNDS`], [`STORED_LAYER`]). Once it answers, only the
//! simplest world transform is answered: the cylinder rule's conditions (`Translation` zero, none of the three
//! Absolute flags, `Owner` the receiver), a zero component and owner `Rotation`, a zero owner `PrePivot` (review of
//! #764, item 4: the port's reading that a pre-pivot moves the owner's frame, **UNVERIFIED**), and a **uniform,
//! positive** combined
//! scale `s` (`Scale` × `Scale3D` × the owner's `DrawScale` × `DrawScale3D`, the three axes exactly equal, finite and
//! above 0; the hub's ruling, lane SCALED-BOUNDS-TRANSFORM). Then the box is (`Location` + `s` × the mesh origin + `s`
//! × `RootBoneTranslation`, the mesh extent × `s` × `BoundsScale`): the standard UE3 bounds transform with no rotation
//! (the root-adjusted local box scaled, then moved to the owner), **UNVERIFIED**: the port's reading of the engine
//! maths, not read from the native. At `s` = 1 it is the earlier unit-scale rule, term for term. Any other case (a
//! non-uniform, zero, negative or non-finite scale among them) is the bounding cylinder's `WORLD_TRANSFORM`
//! `NotRead`. **A translated component** (lane IMPL-BOUNDING-CYLINDER-AND-AFTER, NATIVE_BOUNDING_CYLINDER_AND_AFTER.md
//! section 2, UNVERIFIED): a non-zero `Translation` is accepted only when both rotations are zero and the combined
//! scale is exactly 1 on every axis ([`translated_unit_transform`]): each matrix is then a pure translation, so the
//! origin is `Location` + `Translation` + the mesh origin + `RootBoneTranslation`, the extent unchanged; a scaled or
//! rotated translated component stays the `NotRead`. A non-empty `SpaceBases` with a mesh that lists
//! per-poly collision bones (`PerPolyCollisionBones`) is [`PER_BONE`]'s `NotRead`: the per-bone step is not read.
//!
//! **The template gap** (note section 9.5): the port's `new` ignores a component's template, so the slice's pickup
//! component holds class defaults and no mesh. The note recommends refusing a component made from a template that
//! holds a mesh (option B), which needs the port to record a `new` component's template; it does not. The hub's
//! revised ruling (review r1 of #707): a component with no mesh is not answered at all, since in the port that is
//! always the class-default input the game never has. Source e is [`TEMPLATE`]'s `NotRead`.

use ow_core::NotYet;
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use crate::actors::{byte, field, flag, float, object, physics};
use crate::world::bounding_cylinder::{BOUNDS_LAYER, BOUNDS_RECORD, WORLD_TRANSFORM, resident, vector};

/// Step 1: a sleeping rigid body keeps its bounds, behind an owner physics test that is not read.
pub const SLEEPING: &str =
    "Actor.GetBoundingCylinder: a sleeping rigid body's kept bounds (the owner's physics test is not read)";
/// Step 2: `bSkipUpdateSkelPose` keeps the previous bounds, which the port does not store.
pub const SKIP_POSE: &str = "Actor.GetBoundingCylinder: a skeletal mesh component's kept bounds (bSkipUpdateSkelPose)";
/// Step 3: a `DetailMode` above `DM_Low` is compared with the engine's detail mode, which the port does not hold.
pub const DETAIL_MODE: &str = "Actor.GetBoundingCylinder: the engine's detail mode";
/// Source a with no mesh: the game reads the mesh unchecked.
pub const HIDDEN_NO_MESH: &str =
    "Actor.GetBoundingCylinder: a hidden or fixed-bounds skeletal component with no mesh (the game reads it unchecked)";
/// A skeletal entry whose box needs the mesh's stored bounds, and the layer that would hold them.
pub const SKELETAL_BOUNDS: &str = "Actor.GetBoundingCylinder: a skeletal mesh component's bounds";
pub const STORED_LAYER: &str = "a skeletal mesh's stored bounds";
/// Source b once the parent mesh's bounds are known: whether the root offset is added was not checked.
pub const PARENT_ROOT: &str =
    "Actor.GetBoundingCylinder: a parent anim component's fixed bounds (whether the root offset is added is not read)";
/// Sources c and d: the physics-asset bodies' box under the current bone transforms.
pub const BODIES: &str = "Actor.GetBoundingCylinder: a skeletal mesh component's physics-asset box (it needs a \
                          skeletal mesh component's bone transforms too)";
pub const BODIES_LAYER: &str = "a physics asset's bodies";
/// Source d with `bUseBoundsFromParentAnimComponent`: a copy of the parent's stored `Bounds`.
pub const PARENT_BOUNDS: &str = "Actor.GetBoundingCylinder: a parent anim component's stored bounds";
/// The per-bone collision boxes of a mesh that lists per-poly collision bones, with a non-empty `SpaceBases`.
pub const PER_BONE: &str = "Actor.GetBoundingCylinder: a skeletal mesh's per-bone collision boxes";
/// Source e, a component with no mesh: the game's base rule would apply, but in the port such a component holds class
/// defaults only (its template is ignored by `new`), an input the game never has. Refused (the hub's revised ruling
/// on note section 9.5, review r1 of #707).
pub const TEMPLATE: &str = "Actor.GetBoundingCylinder: the component's template (option B) is not read";
/// The largest difference between two axes of a combined scale that still counts as uniform.
const UNIFORM: f32 = 0.0001;

/// The box (origin, box extent) of live, recorded, colliding entry `c` of exact class `Engine.SkeletalMeshComponent`
/// on receiver `owner` (module documentation, steps 1 to 8). Writes, loads and draws nothing.
pub fn skeletal_box(vm: &Vm, owner: ObjRef, c: ObjRef) -> Result<([f32; 3], [f32; 3]), VmError> {
    let space = &vm.space;
    let component_owner = object(space, c, "Owner")?;
    // 1. A sleeping rigid body.
    if let Some(o) = component_owner {
        resident(vm, o)?;
        if physics(space, o)? == "PHYS_RigidBody" && int(vm, c, "FramesPhysicsAsleep")? > 5 {
            return Err(NotYet::not_read(SLEEPING, BOUNDS_RECORD).into());
        }
    }
    // 2. A skipped pose update.
    if flag(space, c, "bSkipUpdateSkelPose")? {
        return Err(NotYet::not_read(SKIP_POSE, BOUNDS_RECORD).into());
    }
    // 3. Visible or not.
    if byte(space, c, "DetailMode")? > 0 {
        return Err(NotYet::not_read(DETAIL_MODE, BOUNDS_RECORD).into());
    }
    let owner_shown = match component_owner {
        None => true,
        Some(o) => !flag(space, o, "bHidden")? || flag(space, c, "bIgnoreOwnerHidden")?,
    };
    let shown = !flag(space, c, "HiddenGame")? && owner_shown;
    let visible = shown || flag(space, c, "bCastHiddenShadow")?;
    let mesh = resident_object(vm, c, "SkeletalMesh")?;
    // 4. Source a.
    if !visible || flag(space, c, "bComponentUseFixedSkelBounds")? {
        let Some(mesh) = mesh else {
            return Err(NotYet::not_read(HIDDEN_NO_MESH, BOUNDS_RECORD).into());
        };
        return stored_box(vm, owner, c, mesh);
    }
    // 5. Source b: the parent's flag first, so the parent's mesh must be resident only when this source applies
    // (review r1 of #707).
    let parent = resident_object(vm, c, "ParentAnimComponent")?;
    if let Some(p) = parent
        && flag(space, p, "bComponentUseFixedSkelBounds")?
        && let Some(parent_mesh) = resident_object(vm, p, "SkeletalMesh")?
    {
        if crate::world::skeletal::mesh_bounds(vm, parent_mesh)?.is_none() {
            return Err(NotYet::layer_absent(SKELETAL_BOUNDS, STORED_LAYER, BOUNDS_RECORD).into());
        }
        // The root offset question arises only where the box would be answered: in the simplest world transform.
        if simple_transform(vm, owner, c)?.is_none() {
            return Err(NotYet::not_read(WORLD_TRANSFORM, BOUNDS_RECORD).into());
        }
        return Err(NotYet::not_read(PARENT_ROOT, BOUNDS_RECORD).into());
    }
    // 6. Sources c and d.
    let parent_asset = match parent {
        Some(p) => object(space, p, "PhysicsAsset")?.is_some(),
        None => false,
    };
    if object(space, c, "PhysicsAsset")?.is_some()
        && flag(space, c, "bHasValidBodies")?
        && can_use_bones(vm, c, component_owner, mesh, parent_asset)?
    {
        return Err(NotYet::layer_absent(BODIES, BODIES_LAYER, BOUNDS_RECORD).into());
    }
    if parent_asset && can_use_bones(vm, c, component_owner, mesh, parent_asset)? {
        if flag(space, c, "bUseBoundsFromParentAnimComponent")? {
            return Err(NotYet::layer_absent(PARENT_BOUNDS, BOUNDS_LAYER, BOUNDS_RECORD).into());
        }
        return Err(NotYet::layer_absent(BODIES, BODIES_LAYER, BOUNDS_RECORD).into());
    }
    // 7. Source e: refused (the hub's revised ruling on 9.5, review r1 of #707), and 8. source f.
    match mesh {
        None => Err(NotYet::not_read(TEMPLATE, BOUNDS_RECORD).into()),
        Some(mesh) => stored_box(vm, owner, c, mesh),
    }
}

/// The simplest world transform the stored box answers (module documentation, "The stored box"): zero `Translation`,
/// none of the three Absolute flags, `owner` as the component's `Owner`, zero component and owner `Rotation`, a zero
/// owner `PrePivot` (UNVERIFIED), and a uniform, positive combined scale, which is the answer; `None` for any other
/// transform.
fn simple_transform(vm: &Vm, owner: ObjRef, c: ObjRef) -> Result<Option<(f32, [f32; 3])>, VmError> {
    let space = &vm.space;
    let component_owner = object(space, c, "Owner")?;
    let translation = vector(space, c, &field(space, c, "Translation")?, "Translation")?;
    let mut absolute = false;
    for name in ["AbsoluteTranslation", "AbsoluteRotation", "AbsoluteScale"] {
        absolute |= flag(space, c, name)?;
    }
    if absolute
        || component_owner != Some(owner)
        || !rotation_zero(vm, c)?
        || !rotation_zero(vm, owner)?
        || vector(space, owner, &field(space, owner, "PrePivot")?, "PrePivot")? != [0.0; 3]
    {
        return Ok(None);
    }
    let Some(s) = uniform_scale(combined_scale(vm, c, component_owner)?) else { return Ok(None) };
    // A translated component only under unit scale (the note's section 2; the composition order is not settled for
    // a scaled one).
    if translation != [0.0; 3] && s != 1.0 {
        return Ok(None);
    }
    Ok(Some((s, translation)))
}

/// The translated case of the simplest world transform (module documentation), for the cylinder rule: the
/// component's `Translation` when the three Absolute flags are clear, `owner` is its `Owner`, both rotations and the
/// owner's `PrePivot` are zero and the combined scale is exactly 1 on every axis; `None` otherwise. Reads only.
pub(crate) fn translated_unit_transform(vm: &Vm, owner: ObjRef, c: ObjRef) -> Result<Option<[f32; 3]>, VmError> {
    Ok(match simple_transform(vm, owner, c)? {
        Some((s, t)) if s == 1.0 => Some(t),
        _ => None,
    })
}

/// The one factor of a uniform, positive scale: the three axes exactly equal, finite and above 0 (the hub's ruling;
/// strict, unlike step 6's `UNIFORM` tolerance, so the factor is never a choice between axes). `None` otherwise.
fn uniform_scale(scale: [f32; 3]) -> Option<f32> {
    let s = scale[0];
    (scale[1] == s && scale[2] == s && s.is_finite() && s > 0.0).then_some(s)
}

/// Step 6's "can use the bones" (module documentation).
fn can_use_bones(
    vm: &Vm,
    c: ObjRef,
    component_owner: Option<ObjRef>,
    mesh: Option<ObjRef>,
    parent_asset: bool,
) -> Result<bool, VmError> {
    let scale = combined_scale(vm, c, component_owner)?;
    let uniform = (scale[0] - scale[1]).abs() <= UNIFORM
        && (scale[1] - scale[2]).abs() <= UNIFORM
        && (scale[0] - scale[2]).abs() <= UNIFORM;
    let Some(mesh) = mesh.filter(|_| uniform) else {
        return Ok(false);
    };
    if parent_asset {
        return Ok(true);
    }
    let bases = space_bases(vm, c)?;
    if bases == 0 {
        return Ok(false);
    }
    match crate::world::skeletal::mesh_bounds(vm, mesh)? {
        Some(b) => Ok(bases == b.bones),
        None => Err(NotYet::layer_absent(SKELETAL_BOUNDS, STORED_LAYER, BOUNDS_RECORD).into()),
    }
}

/// Sources a and f: the mesh's stored bounds through the simplest world transform (module documentation, "The
/// stored box").
fn stored_box(vm: &Vm, owner: ObjRef, c: ObjRef, mesh: ObjRef) -> Result<([f32; 3], [f32; 3]), VmError> {
    let space = &vm.space;
    let Some(bounds) = crate::world::skeletal::mesh_bounds(vm, mesh)? else {
        return Err(NotYet::layer_absent(SKELETAL_BOUNDS, STORED_LAYER, BOUNDS_RECORD).into());
    };
    let Some((s, translation)) = simple_transform(vm, owner, c)? else {
        return Err(NotYet::not_read(WORLD_TRANSFORM, BOUNDS_RECORD).into());
    };
    if space_bases(vm, c)? > 0 && list_len(vm, mesh, "PerPolyCollisionBones")? > 0 {
        return Err(NotYet::not_read(PER_BONE, BOUNDS_RECORD).into());
    }
    let location = vector(space, owner, &field(space, owner, "Location")?, "Location")?;
    let root = vector(space, c, &field(space, c, "RootBoneTranslation")?, "RootBoneTranslation")?;
    let scale = float(space, c, "BoundsScale")?;
    // UNVERIFIED: the port's reading of the UE3 bounds transform (module documentation, "The stored box").
    // The translation is zero unless the scale is exactly 1 (`simple_transform`).
    let origin = [0, 1, 2].map(|k| location[k] + translation[k] + s * bounds.origin[k] + s * root[k]);
    let extent = bounds.extent.map(|e| e * s * scale);
    Ok((origin, extent))
}

/// `Scale` × `Scale3D`, times the owner's `DrawScale` × `DrawScale3D` when there is an owner.
fn combined_scale(vm: &Vm, c: ObjRef, component_owner: Option<ObjRef>) -> Result<[f32; 3], VmError> {
    let space = &vm.space;
    let own = float(space, c, "Scale")?;
    let own3 = vector(space, c, &field(space, c, "Scale3D")?, "Scale3D")?;
    let mut out = own3.map(|v| v * own);
    if let Some(o) = component_owner {
        let draw = float(space, o, "DrawScale")?;
        let draw3 = vector(space, o, &field(space, o, "DrawScale3D")?, "DrawScale3D")?;
        for k in 0..3 {
            out[k] *= draw * draw3[k];
        }
    }
    Ok(out)
}

/// The length of `c`'s `SpaceBases` (a script array).
fn space_bases(vm: &Vm, c: ObjRef) -> Result<usize, VmError> {
    list_len(vm, c, "SpaceBases")
}

/// The length of array field `name` of `o`; another kind is a `VmError::Script` naming it.
fn list_len(vm: &Vm, o: ObjRef, name: &str) -> Result<usize, VmError> {
    let v = field(&vm.space, o, name)?;
    v.elements().map(<[Value]>::len).ok_or_else(|| wrong(vm, o, name, "an array", &v))
}

/// Int field `name` of `o`; another kind is a `VmError::Script` naming it.
fn int(vm: &Vm, o: ObjRef, name: &str) -> Result<i32, VmError> {
    match field(&vm.space, o, name)? {
        Value::Int(i) => Ok(i),
        other => Err(wrong(vm, o, name, "an int", &other)),
    }
}

/// Whether rotator field `Rotation` of `o` is zero in its three members, read by name lookup only.
fn rotation_zero(vm: &Vm, o: ObjRef) -> Result<bool, VmError> {
    let space = &vm.space;
    let v = field(space, o, "Rotation")?;
    let s = v.as_struct().ok_or_else(|| wrong(vm, o, "Rotation", "a rotator", &v))?;
    let layout = space.classes.struct_layout(s.ty).ok_or_else(|| wrong(vm, o, "Rotation", "a rotator", &v))?;
    for member in ["Pitch", "Yaw", "Roll"] {
        let slot = space
            .names
            .find(member)
            .and_then(|id| layout.field(id))
            .and_then(|(entry, _)| usize::try_from(entry).ok())
            .and_then(|i| s.fields.get(i));
        match slot {
            Some(Value::Int(0)) => {}
            Some(Value::Int(_)) => return Ok(false),
            _ => return Err(wrong(vm, o, "Rotation", "a rotator", &v)),
        }
    }
    Ok(true)
}

/// Object field `name` of `o`, which must be resident when set (else refusal N).
fn resident_object(vm: &Vm, o: ObjRef, name: &str) -> Result<Option<ObjRef>, VmError> {
    let found = object(&vm.space, o, name)?;
    if let Some(x) = found {
        resident(vm, x)?;
    }
    Ok(found)
}

/// A `VmError::Script` naming field `name` of `o` and the kind it held.
fn wrong(vm: &Vm, o: ObjRef, name: &str, wanted: &str, v: &Value) -> VmError {
    VmError::Script {
        function: "Actor.GetBoundingCylinder".to_string(),
        message: format!("{}.{name} holds {}, expected {wanted}", ow_attr::eval::class_path(&vm.space, o), v.kind_name()),
    }
}

// The tests are in `bounding_cylinder_tests.rs`, beside the bounding cylinder's fixture.
