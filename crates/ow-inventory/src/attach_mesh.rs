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
//!   - the guards' "nothing": the component None, the actor destroyed on the heap (`Heap::destroy`, the port's
//!     stand-in for the pending-kill object flag, **UNVERIFIED (fitted)**), or the actor's `bDeleteMe` set;
//!   - a world without a [`SceneLayer`]: [`ATTACH`], `NotYet { LayerAbsent }` ([`SCENE_LAYER`]);
//!   - an attached component (`bAttached`): [`ATTACH_DETACH`], `NotYet { NotRead }` (the detach is not read);
//!   - otherwise **the hub's ruling on section 5, a port Off answer, UNVERIFIED**: the world's scene layer is empty,
//!     so only the script-visible writes are made, in the note's order: `bNeedsReattach` and `bNeedsUpdateTransform`
//!     cleared, `Owner` = the actor, then the component appended to the actor's `Components` (no duplicate check).
//!     `Scene` and `bAttached` are left as they are, and nothing is registered: no render, collision or physics.
//!     The transform and the orthonormalized-lighting value are only handed to the registration, so they are not
//!     built.
//!
//! `Actor.AttachComponent` checks a declaration only on the path that reaches it ([`attach_plan`], review r1 of #511).
//! The other natives check every declaration they read or write before their first write (a data error, `VmError::Script`,
//! names the field). A preflight `NotYet` is returned **unmarked**: the native has written nothing, and the caller's
//! write tracking adds "stopped after partial writes: " when earlier steps wrote (the `make` step). An error after a
//! native's first write is marked once, its kind and record kept. None of them loads a definition.
//!
//! **Part 2** (the note's section 6, lane IMPL-ATTACH-MESH-TO-PICKUP part 2, the note at f32d69b, PR #507), the
//! rest of the pickup's `make` step:
//! - `PrimitiveComponent.SetLightEnvironment(object)` (6.2): the argument equal to `LightEnvironment` is nothing; a
//!   component not `bAttached` gets the one write `LightEnvironment` (the native's own rule); an attached one runs
//!   the deferred reattach's preflight, then `PreviousLightEnvironment`, `LightEnvironment` and `bNeedsReattach`. The
//!   two fields are **UNVERIFIED (fitted)**.
//! - `WillowPickup.InitializePickupForRBPhysics()` (6.3): `bUseRBPhysics` false is nothing (the native's own rule);
//!   otherwise the minimal answer of NATIVE_PICKUP_RB_MINIMAL.md in [`pickup_rb`] (lane IMPL-PICKUP-RB), which
//!   replaced 6.6's rigid-body `LayerAbsent`.
//! - `WillowPickup.SpawnPickupParticles()` (6.4): `PickupParticles` None is [`PARTICLES_REMOVAL`],
//!   `NotYet { NotRead }`; otherwise NATIVE_SPAWN_PICKUP_PARTICLES.md's preflight in [`pickup_particles`] (lane
//!   IMPL-SPAWN-PICKUP-PARTICLES), which replaced 6.4's particle-effects `LayerAbsent`.
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

pub mod particle_activation;
pub mod pickup_particles;
pub mod pickup_rb;

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
pub const ATTACH: &str = ow_world::actors::component_attach::ATTACH;
/// The absent layer [`ATTACH`] names, for a world without a [`SceneLayer`].
pub const SCENE_LAYER: &str = ow_world::actors::component_attach::SCENE_LAYER;
pub const ATTACH_DETACH: &str = "Actor.AttachComponent on an attached component: the detach before the attach";

/// Part 2's native keys.
pub const SET_LIGHT_ENVIRONMENT: &str = "PrimitiveComponent.SetLightEnvironment(object)";
pub const INIT_RB_PHYSICS: &str = "WillowPickup.InitializePickupForRBPhysics()";
pub const SPAWN_PICKUP_PARTICLES: &str = "WillowPickup.SpawnPickupParticles()";

/// Part 2's named `NotYet`s, the note's texts.
pub const PARTICLES_REMOVAL: &str = "WillowPickup.SpawnPickupParticles with no particle system: the particles' removal";

/// The fitted fields (UNVERIFIED (fitted), the note's open question 2).
pub const FORCE_REF_POSE: &str = "bForceRefpose";
pub const HIDDEN_GAME: &str = "HiddenGame";
pub const OWNER_NO_SEE: &str = "bOwnerNoSee";
pub const SKELETAL_MESH: &str = "SkeletalMesh";

/// The world's scene that `Actor.AttachComponent` attaches to, and its set-up: moved to `ow-world`
/// (`ow_world::actors::component_attach`, lane IMPL-SPAWN-COMPONENT-ATTACH) so the spawn's component update uses the
/// same layer; re-exported here unchanged. The standard Off world (`ow_world::world::World::new`) installs it.
pub use ow_world::actors::{SceneLayer, add_empty_scene};

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

/// One write of the attach: the object, the field, the value.
pub type AttachWrite = (ObjRef, &'static str, Value);

/// What `AttachComponent(NewComponent)` on `actor` will do in Off, decided before any write (module documentation),
/// in the order of the revised note's section 3.4 (review r1 of #511):
/// 1. the guards, plain returns (`Ok(None)`): no component; the actor destroyed on the heap; the actor's `bDeleteMe`;
/// 2. no [`SceneLayer`]: [`ATTACH`], `NotYet { LayerAbsent }`;
/// 3. the component's `bAttached` set: [`ATTACH_DETACH`], `NotYet { NotRead }`;
/// 4. then, and only then, the declarations of the fields it writes, and the writes in the native's order: the two
///    flag clears, `Owner`, then the `Components` append (no duplicate check).
///
/// Only the fields a step reads are checked before it (`bDeleteMe` for the guard, `bAttached` for the preflight), so
/// an unrelated declaration error cannot mask a guard, the layer refusal or the preflight.
pub fn attach_plan(vm: &Vm, actor: ObjRef, component: Option<ObjRef>) -> Result<Option<Vec<AttachWrite>>, VmError> {
    const WHAT: &str = "Actor.AttachComponent";
    let Some(component) = component else {
        return Ok(None);
    };
    // The pending-kill guard (UNVERIFIED (fitted)): the heap's destroyed state, read with no declaration.
    if vm.space.heap.get(actor)?.destroyed {
        return Ok(None);
    }
    schemas(vm, actor, &[("bDeleteMe", Want::Bool)], WHAT)?;
    if bool_of(&required(vm, actor, "bDeleteMe", WHAT)?, "bDeleteMe", WHAT)? {
        return Ok(None);
    }
    if vm.service::<SceneLayer>().is_none() {
        return Err(NotYet::layer_absent(ATTACH, SCENE_LAYER, RECORD).into());
    }
    schemas(vm, component, &[("bAttached", Want::Bool)], WHAT)?;
    if bool_of(&required(vm, component, "bAttached", WHAT)?, "bAttached", WHAT)? {
        return Err(NotYet::not_read(ATTACH_DETACH, RECORD).into());
    }
    let fields = [("bNeedsReattach", Want::Bool), ("bNeedsUpdateTransform", Want::Bool), ("Owner", Want::Object)];
    schemas(vm, component, &fields, WHAT)?;
    schemas(vm, actor, &[("Components", Want::Array)], WHAT)?;
    let mut list = match required(vm, actor, "Components", WHAT)? {
        Value::Array(a) => a.as_ref().clone(),
        other => return Err(bad(WHAT, format!("Components holds {}, not an array", other.kind_name()))),
    };
    list.push(Value::Object(Some(component)));
    let plan = vec![
        (component, "bNeedsReattach", Value::Bool(false)),
        (component, "bNeedsUpdateTransform", Value::Bool(false)),
        (component, "Owner", Value::Object(Some(actor))),
        (actor, "Components", Value::array(list)),
    ];
    // Each planned write against the heap's own write rules (review r2 of #511): an array's element kind, and an
    // object's class, so no write is refused after an earlier one.
    for (o, name, v) in &plan {
        let kind = declared(vm, *o, name, WHAT)?;
        vm.space
            .heap
            .check_value(&vm.space.classes, &kind, v)
            .map_err(|e| bad(WHAT, format!("{name} cannot take the attach's value: {e}")))?;
    }
    Ok(Some(plan))
}

/// `AttachComponent(NewComponent)` on `actor` in Off: [`attach_plan`], then its writes in order. An error after the
/// first write is marked once, its kind and record kept.
pub fn attach_component(vm: &mut Vm, actor: ObjRef, component: Option<ObjRef>) -> Result<(), VmError> {
    const WHAT: &str = "Actor.AttachComponent";
    let Some(plan) = attach_plan(vm, actor, component)? else {
        return Ok(());
    };
    for (i, (o, field, v)) in plan.into_iter().enumerate() {
        let r = write(vm, o, field, v, WHAT);
        if i == 0 { r? } else { r.map_err(after_writes)? }
    }
    Ok(())
}

/// `SetLightEnvironment(NewLightEnvironment)` on `component` in Off (module documentation, part 2).
pub fn set_light_environment(vm: &mut Vm, component: ObjRef, new: Option<ObjRef>) -> Result<(), VmError> {
    const WHAT: &str = "PrimitiveComponent.SetLightEnvironment";
    // Each check only on the path that reaches it (review r1 of #520): the equal value first, then `bAttached`; on
    // the attached path the deferred reattach's refusals (owner, then scene), and only then the write targets.
    schemas(vm, component, &[("LightEnvironment", Want::Object)], WHAT)?;
    let current = object_of(&required(vm, component, "LightEnvironment", WHAT)?, WHAT)?;
    if current == new {
        return Ok(());
    }
    schemas(vm, component, &[("bAttached", Want::Bool)], WHAT)?;
    let attached = bool_of(&required(vm, component, "bAttached", WHAT)?, "bAttached", WHAT)?;
    let value = Value::Object(new);
    if !attached {
        fits(vm, component, "LightEnvironment", &value, WHAT)?;
        return write(vm, component, "LightEnvironment", value, WHAT);
    }
    schemas(vm, component, &[("Owner", Want::Object)], WHAT)?;
    if object_of(&required(vm, component, "Owner", WHAT)?, WHAT)?.is_some() {
        return Err(NotYet::not_read(REATTACH_OWNER, RECORD).into());
    }
    schemas(vm, component, &[("Scene", Want::Struct)], WHAT)?;
    if !is_zero(&required(vm, component, "Scene", WHAT)?, WHAT)? {
        return Err(NotYet::not_read(REATTACH_SCENE, RECORD).into());
    }
    schemas(vm, component, &[("PreviousLightEnvironment", Want::Object), ("bNeedsReattach", Want::Bool)], WHAT)?;
    fits(vm, component, "PreviousLightEnvironment", &Value::Object(current), WHAT)?;
    fits(vm, component, "LightEnvironment", &value, WHAT)?;
    // Each destination's slot read, immutably, before the first write (review r2 of #520): an object whose slot vector
    // is short of a declared field is refused here, not after two writes.
    for name in ["PreviousLightEnvironment", "LightEnvironment", "bNeedsReattach"] {
        required(vm, component, name, WHAT)?;
    }
    write(vm, component, "PreviousLightEnvironment", Value::Object(current), WHAT)?;
    let rest = (|| {
        write(vm, component, "LightEnvironment", value, WHAT)?;
        write(vm, component, "bNeedsReattach", Value::Bool(true), WHAT)
    })();
    rest.map_err(after_writes)
}

/// `v` against `name`'s declaration on `o` by the heap's own write rules, else a data error naming the field.
fn fits(vm: &Vm, o: ObjRef, name: &str, v: &Value, what: &str) -> Result<(), VmError> {
    let kind = declared(vm, o, name, what)?;
    vm.space
        .heap
        .check_value(&vm.space.classes, &kind, v)
        .map_err(|e| bad(what, format!("{name} cannot take the value: {e}")))
}

fn light_environment_native(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let component = this(c, SET_LIGHT_ENVIRONMENT)?;
    let new = object_of(c.arg(0)?, SET_LIGHT_ENVIRONMENT)?;
    set_light_environment(c.vm, component, new)?;
    Ok(Value::None)
}

fn rb_physics_native(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let pickup = this(c, INIT_RB_PHYSICS)?;
    pickup_rb::initialize(c.vm, pickup)?;
    Ok(Value::None)
}

fn particles_native(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let pickup = this(c, SPAWN_PICKUP_PARTICLES)?;
    pickup_particles::spawn_pickup_particles(c.vm, pickup)?;
    Ok(Value::None)
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

/// The function path the skill worlds bind `Actor.AttachComponent(object)` under.
pub const ATTACH_COMPONENT_PATH: &str = "Engine.Actor.AttachComponent";

/// The record that [`bind_engine_attach`] bound the attach for `Engine` (a VM service, set once).
struct EngineAttachBound;

/// Binds this module's `Actor.AttachComponent` rule to [`ATTACH_COMPONENT_PATH`] alone in `ParityMode::Off`, for the
/// skill worlds (`ow_skills::action_skill::Pack`, lane BIND-ATTACHCOMPONENT-SKILL-WORLDS), which do not register this
/// module. Keep-existing: a function the VM does not declare, one that is not native, one declared under another
/// native key than [`ATTACH_COMPONENT`], or one already bound gets nothing. Reference binds nothing.
///
/// Unlike [`register`], this binds `Engine`'s function only, not the key VM-wide: a same-keyed function of another
/// package stays unbound (review r1 of #623).
pub fn bind_engine_attach(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    let Ok(id) = vm.find_function(ATTACH_COMPONENT_PATH) else {
        return Ok(());
    };
    // Only the signature this rule is for (review r2 of #623): a function declared under another key is left alone,
    // and the reuse record is never set for it.
    let ours = vm.function(id).is_some_and(|d| d.is_native() && d.native_key.as_deref() == Some(ATTACH_COMPONENT));
    if !ours || vm.is_bound(id) {
        return Ok(());
    }
    vm.register_native_for(id, Rc::new(attach_native))?;
    vm.add_service(EngineAttachBound)?;
    Ok(())
}

/// Whether the key's only binding is [`bind_engine_attach`]'s: its record is set, no VM-wide binding exists, and one
/// package binds the key.
fn reuses_engine_attach(vm: &Vm) -> bool {
    vm.service::<EngineAttachBound>().is_some()
        && !vm.native_keys().any(|k| k == ATTACH_COMPONENT)
        && vm.package_bindings(ATTACH_COMPONENT) == 1
}

/// Binds the natives of parts 1 and 2 in `ParityMode::Off`; Reference binds nothing. The scene layer is the world set-up's to add
/// ([`add_empty_scene`]).
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    vm.register_native(SET_FORCE_REF_POSE, Rc::new(force_ref_pose_native))?;
    vm.register_native(SET_HIDDEN, Rc::new(hidden_native))?;
    vm.register_native(SET_OWNER_NO_SEE, Rc::new(owner_no_see_native))?;
    vm.register_native(HIDE_BONE_BY_NAME, Rc::new(hide_bone_native))?;
    // The one reuse allowed (review r1 of #623): [`bind_engine_attach`] bound this rule for `Engine` alone, and nothing
    // else binds the key. Any other binding of the key keeps the duplicate-registration error.
    if !reuses_engine_attach(vm) {
        vm.register_native(ATTACH_COMPONENT, Rc::new(attach_native))?;
    }
    // Part 2's keys, first wins (review r1 of #520): a key an earlier pack bound keeps its handler, and the others are
    // still registered.
    let part2: [(&str, NativeFn); 3] = [
        (SET_LIGHT_ENVIRONMENT, light_environment_native),
        (INIT_RB_PHYSICS, rb_physics_native),
        (SPAWN_PICKUP_PARTICLES, particles_native),
    ];
    for (key, f) in part2 {
        if !vm.has_native(key) {
            vm.register_native(key, Rc::new(f))?;
        }
    }
    Ok(())
}

type NativeFn = fn(&mut NativeCall<'_>) -> Result<Value, VmError>;

#[cfg(test)]
#[path = "attach_mesh_tests.rs"]
mod tests;
