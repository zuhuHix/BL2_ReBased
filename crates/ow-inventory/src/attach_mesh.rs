//! Owner: lane IMPL-ATTACH-MESH-TO-PICKUP (part 1). AI-assisted (Claude Code). The natives that
//! `WillowWeapon.AttachMeshToPickup` reaches after `SetAnimTreeTemplate`, in Off, coded from
//! NATIVE_ATTACH_MESH_TO_PICKUP.md part 1 alone (analyst lane ANALYST-ATTACH-MESH-TO-PICKUP, the note at fefadd6), with
//! the hub's ruling on the note's section 5. This lane did not read the natives. Every rule is the note's reading of
//! the executable, **UNVERIFIED** in the running game.
//!
//! **The deferred reattach** (the note's section 2), shared by `SetForceRefPose`, `SetHidden` and `SetOwnerNoSee`.
//! Its preflight runs before the calling native writes anything:
//! - `Owner` set: [`REATTACH_OWNER`], `NotYet { NotRead }`;
//! - no `Owner`, and `Scene` not zero: [`REATTACH_SCENE`], `NotYet { NotRead }`;
//! - otherwise (the slice's case) its only write is `bNeedsReattach` = true, after the native's own write.
//!
//! **The natives** (section 3):
//! - `SkeletalMeshComponent.SetForceRefPose(bool)`: the deferred reattach's preflight, then `bForceRefpose` = 1 or 0
//!   (an int field, **UNVERIFIED (fitted)** against `bOldForceRefPose`), then `bNeedsReattach`. It always writes: there
//!   is no "unchanged" short cut.
//! - `PrimitiveComponent.SetHidden(bool)` and `PrimitiveComponent.SetOwnerNoSee(bool)`: when the flag already equals
//!   the argument, nothing. Otherwise the preflight, then the flag, then `bNeedsReattach`. The flags are
//!   `HiddenGame` and `bOwnerNoSee`, **UNVERIFIED (fitted)**.
//! - `SkeletalMeshComponent.HideBoneByName(name,byte)`: a None name, or a component whose `SkeletalMesh` is None
//!   (**UNVERIFIED (fitted)**), is "no bone": nothing. Otherwise [`HIDE_BONE`], `NotYet { NotRead }`, before any write.
//! - `Actor.AttachComponent(object)`:
//!   - the guards' "nothing": the component None, the actor's `bDeleteMe` set, or the actor destroyed (the port's
//!     stand-in for the pending-kill object flag, **UNVERIFIED (fitted)**);
//!   - a world without a [`SceneLayer`]: [`ATTACH`], `NotYet { LayerAbsent }` ([`SCENE_LAYER`]);
//!   - an attached component (`bAttached`): [`ATTACH_DETACH`], `NotYet { NotRead }` (the detach is not read);
//!   - otherwise **the hub's ruling on section 5, a port Off answer, UNVERIFIED**: the world's scene layer is empty,
//!     so only the script-visible writes are made, in the note's order: `bNeedsReattach` and `bNeedsUpdateTransform`
//!     cleared, `Owner` = the actor, then the component appended to the actor's `Components` (no duplicate check).
//!     `Scene` and `bAttached` are left as they are, and nothing is registered: no render, collision or physics.
//!     The transform and the orthonormalized-lighting value are only handed to the registration, so they are not
//!     built.
//!
//! Every declaration a native reads or writes is checked before its first write (a data error, `VmError::Script`,
//! names the field). A preflight `NotYet` is returned **unmarked**: the native has written nothing, and the caller's
//! write tracking adds "stopped after partial writes: " when earlier steps wrote (the `make` step). An error after a
//! native's first write is marked once, its kind and record kept. None of them loads a definition.
//!
//! **Only in `ParityMode::Off`.** The C++ reference binds none of them, so Reference keeps the VM's stub policy
//! (`Actor.AttachComponent` is a Reference stub of the slice runs).

use std::rc::Rc;

use ow_core::{NotYet, ParityMode};
use ow_object::ids::{NameId, ObjRef};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

use crate::equip::{bad, object_of, required, write};

/// The record these natives follow.
pub const RECORD: &str = "docs/verification/NATIVE_ATTACH_MESH_TO_PICKUP.md";

/// The native keys [`register`] binds in Off.
pub const SET_FORCE_REF_POSE: &str = "SkeletalMeshComponent.SetForceRefPose(bool)";
pub const SET_HIDDEN: &str = "PrimitiveComponent.SetHidden(bool)";
pub const SET_OWNER_NO_SEE: &str = "PrimitiveComponent.SetOwnerNoSee(bool)";
pub const HIDE_BONE_BY_NAME: &str = "SkeletalMeshComponent.HideBoneByName(name,byte)";
pub const ATTACH_COMPONENT: &str = "Actor.AttachComponent(object)";

/// The named `NotYet`s.
pub const REATTACH_OWNER: &str = "the deferred reattach: the owner's component update";
pub const REATTACH_SCENE: &str = "the deferred reattach: the component's detach and reattach";
pub const HIDE_BONE: &str =
    "SkeletalMeshComponent.HideBoneByName on a component with a skeletal mesh: the bone lookup and the visibility rebuild";
pub const ATTACH: &str = "Actor.AttachComponent: the component's attach to the world's scene";
/// The absent layer [`ATTACH`] names, for a world without a [`SceneLayer`].
pub const SCENE_LAYER: &str = "a scene that components attach to: render, collision and physics registration (the port has none)";
pub const ATTACH_DETACH: &str = "Actor.AttachComponent on an attached component: its detach first";

/// The fitted fields (UNVERIFIED (fitted), the note's open question 2).
pub const FORCE_REF_POSE: &str = "bForceRefpose";
pub const HIDDEN_GAME: &str = "HiddenGame";
pub const OWNER_NO_SEE: &str = "bOwnerNoSee";
pub const SKELETAL_MESH: &str = "SkeletalMesh";

/// The world's scene that `Actor.AttachComponent` attaches to, as an explicit layer on the world (a VM service). The
/// hub's ruling on the note's section 5: a world that holds it, empty, gets the attach's script-visible writes and no
/// registration (a port Off answer, UNVERIFIED in game). A world without it is `NotYet { LayerAbsent }`.
///
/// **A lane that adds a real scene feeds this layer**; the attach then registers there. It holds nothing today.
#[derive(Debug, Default)]
pub struct SceneLayer {}

/// Adds an empty [`SceneLayer`] to `vm`'s world unless one is there (world set-up).
pub fn add_empty_scene(vm: &mut Vm) -> Result<(), VmError> {
    if vm.service::<SceneLayer>().is_none() {
        vm.add_service(SceneLayer::default())?;
    }
    Ok(())
}

fn declared(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<PropKind, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm
        .space
        .prop(class, name)
        .ok_or_else(|| bad(what, format!("{} declares no {name}", ow_attr::eval::class_path(&vm.space, o))))?;
    vm.space.classes.decl(p).map(|d| d.kind.clone()).ok_or_else(|| bad(what, format!("{name} has no declaration")))
}

#[derive(Clone, Copy)]
enum Want {
    Object,
    Bool,
    Int,
    Struct,
    Array,
}

/// Each `(name, kind)` declared on `o` with that kind, else a data error naming the field.
fn schemas(vm: &Vm, o: ObjRef, fields: &[(&str, Want)], what: &str) -> Result<(), VmError> {
    for &(name, want) in fields {
        let kind = declared(vm, o, name, what)?;
        let ok = match want {
            Want::Object => matches!(kind, PropKind::Object { .. }),
            Want::Bool => matches!(kind, PropKind::Bool),
            Want::Int => matches!(kind, PropKind::Int),
            Want::Struct => matches!(kind, PropKind::Struct(_)),
            Want::Array => matches!(kind, PropKind::Array(_)),
        };
        if !ok {
            return Err(bad(what, format!("{name} is declared as {kind:?}, not the kind the note reads")));
        }
    }
    Ok(())
}

/// The component fields the deferred reattach reads and writes.
const REATTACH_FIELDS: [(&str, Want); 3] = [("Owner", Want::Object), ("Scene", Want::Struct), ("bNeedsReattach", Want::Bool)];

fn bool_of(v: &Value, name: &str, what: &str) -> Result<bool, VmError> {
    match v {
        Value::Bool(b) => Ok(*b),
        other => Err(bad(what, format!("{name} holds {}, not a bool", other.kind_name()))),
    }
}

/// Whether `v`, the `Scene` pointer struct, is zero: every field an int 0, at any depth.
fn is_zero(v: &Value, what: &str) -> Result<bool, VmError> {
    match v {
        Value::Int(i) => Ok(*i == 0),
        Value::Struct(s) => {
            for f in &s.fields {
                if !is_zero(f, what)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        other => Err(bad(what, format!("Scene holds {}, not a pointer struct of ints", other.kind_name()))),
    }
}

/// The deferred reattach's preflight (module documentation): nothing is written.
fn reattach_preflight(vm: &Vm, component: ObjRef, what: &str) -> Result<(), VmError> {
    if object_of(&required(vm, component, "Owner", what)?, what)?.is_some() {
        return Err(NotYet::not_read(REATTACH_OWNER, RECORD).into());
    }
    if !is_zero(&required(vm, component, "Scene", what)?, what)? {
        return Err(NotYet::not_read(REATTACH_SCENE, RECORD).into());
    }
    Ok(())
}

/// `e` after a native's first write: a `NotYet` marked once, its kind and record kept; any other error unchanged.
fn after_writes(e: VmError) -> VmError {
    match e {
        VmError::NotYet(n) => n.after_partial_writes().into(),
        other => other,
    }
}

/// `SetForceRefPose(bNewForceRefPose)` on `component` in Off (module documentation).
pub fn set_force_ref_pose(vm: &mut Vm, component: ObjRef, value: bool) -> Result<(), VmError> {
    const WHAT: &str = "SkeletalMeshComponent.SetForceRefPose";
    schemas(vm, component, &REATTACH_FIELDS, WHAT)?;
    schemas(vm, component, &[(FORCE_REF_POSE, Want::Int)], WHAT)?;
    reattach_preflight(vm, component, WHAT)?;
    write(vm, component, FORCE_REF_POSE, Value::Int(i32::from(value)), WHAT)?;
    write(vm, component, "bNeedsReattach", Value::Bool(true), WHAT).map_err(after_writes)
}

/// `SetHidden` or `SetOwnerNoSee` on `component` in Off: `flag` set to `value` (module documentation).
pub fn set_flag(vm: &mut Vm, component: ObjRef, flag: &str, value: bool, what: &str) -> Result<(), VmError> {
    schemas(vm, component, &REATTACH_FIELDS, what)?;
    schemas(vm, component, &[(flag, Want::Bool)], what)?;
    if bool_of(&required(vm, component, flag, what)?, flag, what)? == value {
        return Ok(());
    }
    reattach_preflight(vm, component, what)?;
    write(vm, component, flag, Value::Bool(value), what)?;
    write(vm, component, "bNeedsReattach", Value::Bool(true), what).map_err(after_writes)
}

/// `HideBoneByName(BoneName, PhysBodyOption)` on `component` in Off (module documentation). Writes nothing.
pub fn hide_bone_by_name(vm: &Vm, component: ObjRef, bone: NameId) -> Result<(), VmError> {
    const WHAT: &str = "SkeletalMeshComponent.HideBoneByName";
    schemas(vm, component, &[(SKELETAL_MESH, Want::Object)], WHAT)?;
    if bone == NameId::NONE || object_of(&required(vm, component, SKELETAL_MESH, WHAT)?, WHAT)?.is_none() {
        return Ok(());
    }
    Err(NotYet::not_read(HIDE_BONE, RECORD).into())
}

/// `AttachComponent(NewComponent)` on `actor` in Off (module documentation).
pub fn attach_component(vm: &mut Vm, actor: ObjRef, component: Option<ObjRef>) -> Result<(), VmError> {
    const WHAT: &str = "Actor.AttachComponent";
    let Some(component) = component else {
        return Ok(());
    };
    schemas(vm, actor, &[("Components", Want::Array), ("bDeleteMe", Want::Bool)], WHAT)?;
    if bool_of(&required(vm, actor, "bDeleteMe", WHAT)?, "bDeleteMe", WHAT)? || vm.space.heap.get(actor)?.destroyed {
        return Ok(());
    }
    if vm.service::<SceneLayer>().is_none() {
        return Err(NotYet::layer_absent(ATTACH, SCENE_LAYER, RECORD).into());
    }
    let fields = [
        ("Owner", Want::Object),
        ("bAttached", Want::Bool),
        ("bNeedsReattach", Want::Bool),
        ("bNeedsUpdateTransform", Want::Bool),
    ];
    schemas(vm, component, &fields, WHAT)?;
    if bool_of(&required(vm, component, "bAttached", WHAT)?, "bAttached", WHAT)? {
        return Err(NotYet::not_read(ATTACH_DETACH, RECORD).into());
    }
    let mut list = match required(vm, actor, "Components", WHAT)? {
        Value::Array(a) => a.as_ref().clone(),
        other => return Err(bad(WHAT, format!("Components holds {}, not an array", other.kind_name()))),
    };
    list.push(Value::Object(Some(component)));
    write(vm, component, "bNeedsReattach", Value::Bool(false), WHAT)?;
    let rest = (|| {
        write(vm, component, "bNeedsUpdateTransform", Value::Bool(false), WHAT)?;
        write(vm, component, "Owner", Value::Object(Some(actor)), WHAT)?;
        write(vm, actor, "Components", Value::array(list), WHAT)
    })();
    rest.map_err(after_writes)
}

fn this(c: &NativeCall<'_>, what: &str) -> Result<ObjRef, VmError> {
    c.this.ok_or_else(|| bad(what, "called without an object"))
}

fn arg_bool(c: &NativeCall<'_>, what: &str) -> Result<bool, VmError> {
    bool_of(c.arg(0)?, "the argument", what)
}

fn force_ref_pose_native(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let (component, value) = (this(c, SET_FORCE_REF_POSE)?, arg_bool(c, SET_FORCE_REF_POSE)?);
    set_force_ref_pose(c.vm, component, value)?;
    Ok(Value::None)
}

fn hidden_native(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let (component, value) = (this(c, SET_HIDDEN)?, arg_bool(c, SET_HIDDEN)?);
    set_flag(c.vm, component, HIDDEN_GAME, value, "PrimitiveComponent.SetHidden")?;
    Ok(Value::None)
}

fn owner_no_see_native(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let (component, value) = (this(c, SET_OWNER_NO_SEE)?, arg_bool(c, SET_OWNER_NO_SEE)?);
    set_flag(c.vm, component, OWNER_NO_SEE, value, "PrimitiveComponent.SetOwnerNoSee")?;
    Ok(Value::None)
}

fn hide_bone_native(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let component = this(c, HIDE_BONE_BY_NAME)?;
    let bone = match c.arg(0)? {
        Value::Name(n) => *n,
        other => return Err(bad(HIDE_BONE_BY_NAME, format!("BoneName holds {}, not a name", other.kind_name()))),
    };
    hide_bone_by_name(c.vm, component, bone)?;
    Ok(Value::None)
}

fn attach_native(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let actor = this(c, ATTACH_COMPONENT)?;
    let component = object_of(c.arg(0)?, ATTACH_COMPONENT)?;
    attach_component(c.vm, actor, component)?;
    Ok(Value::None)
}

/// Binds the five natives in `ParityMode::Off`; Reference binds nothing. The scene layer is the world set-up's to add
/// ([`add_empty_scene`]).
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    vm.register_native(SET_FORCE_REF_POSE, Rc::new(force_ref_pose_native))?;
    vm.register_native(SET_HIDDEN, Rc::new(hidden_native))?;
    vm.register_native(SET_OWNER_NO_SEE, Rc::new(owner_no_see_native))?;
    vm.register_native(HIDE_BONE_BY_NAME, Rc::new(hide_bone_native))?;
    vm.register_native(ATTACH_COMPONENT, Rc::new(attach_native))
}

#[cfg(test)]
#[path = "attach_mesh_tests.rs"]
mod tests;
