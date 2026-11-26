//! Owner: lane IMPL-LINK1-RBPHYSICS. `maya:8` after the collision setters, in `ParityMode::Off`:
//! `WillowPawn.InitializeRBPhysics` (2.A), `WillowPlayerPawn.GetLinearColor` (2.D) and
//! `WillowPlayerPawn.OnNativeApplyBodyClassStartingValuesHook` (2.F). 2.C (`GetFirstPersonWeaponHoldDef`) is in
//! [`super::weapon_hold`]. 2.B (`WillowPawn.InitializeBehaviorProviders`) is not bound here: the kernel pack binds its
//! rule (`ow_behavior::kernel::natives`, NATIVE_INITIALIZE_BEHAVIOR_PROVIDERS.md; #744), and a placeholder here made
//! that binding a duplicate registration (#744's landing check). Nor is 2.E
//! (`MeshComponent.GetInstancedMaterialInstanceConstant`): its rule is `ow_inventory::equip::material_instances`
//! (#744), and the `NotRead` placeholder this module bound kept that rule out wherever both were registered, with no
//! error (lane PLACEHOLDER-SHADOWS). AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from docs/verification/NATIVE_LINK1_RBPHYSICS_AND_AFTER.md section 2 alone (lane
//! ANALYST-LINK1-RBPHYSICS-AND-AFTER, #738 with its r1 fix, on main); this lane did not read the executable. **Every
//! rule is UNVERIFIED in the running game.** Reference keeps the stub policy.
//!
//! **2.A `InitializeRBPhysics`**, load-free. The pawn declares `bDeleteMe`, `Physics` (a byte),
//! `Components` (an array of object references), `CollisionComponent` and `bPushesRigidBodies`, else
//! `VmError::Script`. With `bDeleteMe` set: nothing. Else the walk over `Components` in order, after the `Physics`
//! filter (with `PHYS_RigidBody`, 10, only the `CollisionComponent`), None entries skipped. **The entry checks run
//! first, in one pass, before any answer** (review of #742): over every entry the walk meets, and the
//! `CollisionComponent` when `bPushesRigidBodies` is set, a store handle not loaded (or a slotless stand-in) is
//! [`NOT_LOADED`] (`NotRead`), and the field the attached test reads must be declared (`bAttached` a bool on a
//! template receiver, `Owner` an object on a live one), else `VmError::Script`. Then, in order, whether an entry is
//! **attached**: on a template receiver (`Origin::Default`), its `bAttached`; on a live one, **the attach record under
//! the pawn only** (`actors::attached_to`; NATIVE_COMPONENT_BOUNDS.md section 7 A, as `bounding_cylinder`'s live path
//! reads it): a live entry's own `bAttached` is not read. A live entry not in the record follows refusal L's
//! convention (`bounding_cylinder::REFUSAL_L` and its layer, `LayerAbsent`): attachment is unknown.
//!
//! **A template receiver** (review r1 of #763, the hub's ruling): an attached entry is [`RIGID_BODY`] (`LayerAbsent`,
//! [`RIGID_LAYER`]), as before, and a template is never written.
//!
//! **A live receiver: the class table** (lane IMPL-LINK1-RIGID-BODY-CLASS-TABLE, decision 2026-11-26-25), coded from
//! docs/verification/NATIVE_LINK1_RIGID_BODY_LAYER.md section 3.1 alone (lane ANALYST-LINK1-RIGID-BODY-LAYER, #763
//! with its r1 fix; clean room: this lane did not read the executable). It replaces the attached entry's
//! `LayerAbsent`: the game makes no body for these cases. The visit is **fixed** unless the pawn's `Physics` is
//! `PHYS_RigidBody` (10). Each attached entry is answered by its **first native class**
//! (`render_hooks::first_native`):
//! - [`SET_UP_NOTHING`] (the non-primitive components Maya holds, and `PrimitiveComponent` and `CylinderComponent`,
//!   which have no shape): nothing;
//! - [`SET_UP_SKELETAL`] (`SkeletalMeshComponent` and three subclasses): the skeletal rule below;
//! - any other: [`UNREAD_CLASS`] (`NotRead`), naming the class.
//!
//! **The skeletal rule.** With `bDisableAllRigidBody` set: nothing (its attachments are not visited). Else, in order:
//! `bUseSingleBodyPhysics` not 0 is [`RIGID_BODY`] (one body from the asset); `bHasPhysicsAssetInstance` set with a
//! `SkeletalMesh` is [`RIGID_BODY`] (the asset's bodies); `ClothSim` zero with the mesh's cloth bool set is [`CLOTH`]
//! (`NotRead`; the bool is [`CLOTH_BOOL`] by place, **fitted, UNVERIFIED**, read only when `ClothSim` is zero and there
//! is a mesh); `bEnableSoftBodySimulation` set is [`RIGID_BODY`] (the soft body). Otherwise the writes:
//! `bSkelCompFixed` := fixed, and `ClothMeshIndexData` := empty when `ClothSim` is zero. Then each `Attachments`
//! entry's `Component` in the attach record under the same owner gets the same table, recursively, in array order;
//! one not in the record is not visited.
//!
//! **The preflight.** Every refusal comes **before the first write**, over the whole walk and its recursion: the
//! declarations (each flag and field with its kind, else `VmError::Script`); every entry, attachment component and
//! mesh resident (else [`NOT_LOADED`], `NotRead`); refusal L; the rows above; and `SetPushesRigidBodies`' own
//! refusal, folded in (set, with a `CollisionComponent`: the same attached test, [`RIGID_BODY`] or refusal L). So no
//! refusal follows a write, and none needs the partial-writes prefix (the game writes component by component; the
//! ordering is the port's). The physics scene is treated as present, as the note rules. Then
//! `SetPushesRigidBodies(bPushesRigidBodies)`: clear (the port makes no pushing body) is nothing, and the same-value
//! write is skipped. No entry and no pushing body: nothing. **UNVERIFIED** in the running game.
//!
//! **2.D `GetLinearColor(C)`:** each of R, G, B and A as `byte as f32 * (1.0f32 / 255.0)` (an f32 multiply by the f32
//! constant, not a divide). Nothing written.
//!
//! **2.F `OnNativeApplyBodyClassStartingValuesHook()`:** `ImmuneToFallingDamage` := true (declared as a bool, else
//! `VmError::Script`); nothing else.

use std::rc::Rc;

use ow_core::NotYet;
use ow_object::heap::Origin;
use ow_object::ids::ObjRef;
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

use super::bounding_cylinder::{BOUNDS_LAYER, REFUSAL_L};
use super::component_setters::{Want, bad, bool_of, object_of, schemas, write};

/// The record of the rules.
pub const RECORD: &str = "docs/verification/NATIVE_LINK1_RBPHYSICS_AND_AFTER.md";
/// The native keys.
pub const INITIALIZE_RB_PHYSICS: &str = "WillowPawn.InitializeRBPhysics()";
pub const GET_LINEAR_COLOR: &str = "WillowPlayerPawn.GetLinearColor(color)";
pub const APPLY_HOOK: &str = "WillowPlayerPawn.OnNativeApplyBodyClassStartingValuesHook()";
/// 2.A's refusals.
pub const RIGID_BODY: &str = "WillowPawn.InitializeRBPhysics: a component's rigid body";
pub const RIGID_LAYER: &str = "a physics layer (rigid bodies)";
pub const NOT_LOADED: &str = "WillowPawn.InitializeRBPhysics: a component the store has not loaded (a load is a write)";
/// The class table's record.
pub const RECORD_LAYER: &str = "docs/verification/NATIVE_LINK1_RIGID_BODY_LAYER.md";
/// First native classes whose component set-up does nothing (section 3.1's table, its first two rows).
pub const SET_UP_NOTHING: [&str; 7] = [
    "Engine.ActorComponent",
    "Engine.DynamicLightEnvironmentComponent",
    "GearboxFramework.SpecialMoveComponent",
    "GearboxFramework.GearboxDialogComponent",
    "WillowGame.StatusEffectsComponent",
    "Engine.PrimitiveComponent",
    "Engine.CylinderComponent",
];
/// First native classes that take the skeletal rule (section 3.1's table, its third row).
pub const SET_UP_SKELETAL: [&str; 4] = [
    "Engine.SkeletalMeshComponent",
    "GearboxFramework.GearboxSkeletalMeshComponent",
    "WillowGame.CustomSkeletalMeshComponent",
    "WillowGame.GearLikenessMeshComponent",
];
/// Any other first native class: `"<UNREAD_CLASS> <class>"` (`NotRead`).
pub const UNREAD_CLASS: &str = "WillowPawn.InitializeRBPhysics: the rigid-body set-up of";
/// The cloth step (`NotRead`).
pub const CLOTH: &str = "InitializeRBPhysics: the cloth set-up";
/// The mesh's cloth bool, by place (fitted, UNVERIFIED).
pub const CLOTH_BOOL: &str = "bClothMetal";
/// The skeletal fields the rule reads, with their kinds (the two arrays' element kinds are checked apart).
const SKELETAL_FIELDS: [(&str, Want); 9] = [
    ("bDisableAllRigidBody", Want::Bool),
    ("bHasPhysicsAssetInstance", Want::Bool),
    ("bSkelCompFixed", Want::Bool),
    ("bEnableSoftBodySimulation", Want::Bool),
    ("bUseSingleBodyPhysics", Want::Int),
    ("ClothSim", Want::Struct),
    ("ClothMeshIndexData", Want::Array),
    ("SkeletalMesh", Want::Object),
    ("Attachments", Want::Array),
];

/// [`NOT_LOADED`] naming the entry: `"<NOT_LOADED>: <class> <path>"`, the path the store's handle records (else the
/// object's path), as `tree_ticks::not_loaded` names its object.
pub fn not_loaded(vm: &Vm, entry: ObjRef) -> String {
    let class = vm
        .space
        .heap
        .get(entry)
        .ok()
        .and_then(|x| vm.space.classes.get_layout(x.class))
        .map_or_else(String::new, |l| l.path().as_str().to_string());
    let path = vm.defs.handle_path(entry).unwrap_or_else(|| crate::pools::object_path(&vm.space, entry));
    format!("{NOT_LOADED}: {class} {path}")
}
/// `PHYS_RigidBody`.
pub const PHYS_RIGID_BODY: u8 = 10;
const RB: &str = "WillowPawn.InitializeRBPhysics";

fn read(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<Value, VmError> {
    super::component_setters::Slots::of(o).required(vm, name, what)
}

fn resident(vm: &Vm, o: ObjRef) -> Result<bool, VmError> {
    let x = vm.space.heap.get(o)?;
    let slotless = vm.space.classes.get_layout(x.class).is_some_and(|l| x.slots.len() < l.slot_count() as usize);
    Ok(!(vm.defs.is_unloaded(&vm.space, o) || vm.defs.is_failed(&vm.space, o) || slotless))
}

/// The entry checks of the first pass (module documentation): resident, and the attached test's field declared.
fn check_entry(vm: &Vm, template: bool, entry: ObjRef) -> Result<(), VmError> {
    if !resident(vm, entry)? {
        return Err(NotYet::not_read(not_loaded(vm, entry), RECORD).into());
    }
    if template {
        schemas(vm, entry, &[("bAttached", Want::Bool)], RB)
    } else {
        schemas(vm, entry, &[("Owner", Want::Object)], RB)
    }
}

/// Whether `entry` (checked) counts as attached (module documentation): `Some(true)` attached, `Some(false)` not
/// attached (a template entry with `bAttached` clear), `None` unknown (a live entry not in the attach record).
fn attached(vm: &Vm, pawn: ObjRef, template: bool, entry: ObjRef) -> Result<Option<bool>, VmError> {
    if template {
        return Ok(Some(bool_of(&read(vm, entry, "bAttached", RB)?, "bAttached", RB)?));
    }
    Ok(if crate::actors::attached_to(vm, entry, pawn)? { Some(true) } else { None })
}

/// The refusal for an entry by its attached state (`Ok` for a template entry not attached).
fn answer(state: Option<bool>) -> Result<(), VmError> {
    match state {
        Some(true) => Err(NotYet::layer_absent(RIGID_BODY, RIGID_LAYER, RECORD).into()),
        Some(false) => Ok(()),
        None => Err(NotYet::layer_absent(REFUSAL_L, BOUNDS_LAYER, RECORD).into()),
    }
}

/// One skeletal entry's writes (module documentation): `bSkelCompFixed`, and `ClothMeshIndexData` emptied when
/// `ClothSim` is zero.
struct SkeletalWrite {
    component: ObjRef,
    empty_cloth: bool,
}

/// `o` resident, else [`NOT_LOADED`] naming it.
fn require_resident(vm: &Vm, o: ObjRef) -> Result<(), VmError> {
    if resident(vm, o)? { Ok(()) } else { Err(NotYet::not_read(not_loaded(vm, o), RECORD).into()) }
}

/// The element kind of the array `name` on `o` (declared as an array, else a data error).
fn element_kind(vm: &Vm, o: ObjRef, name: &str) -> Result<PropKind, VmError> {
    match super::component_setters::declared(vm, o, name, RB)? {
        PropKind::Array(inner) => Ok(*inner),
        other => Err(bad(RB, format!("{name} is declared as {}, not an array", other.kind_name()))),
    }
}

/// Whether the pointer struct `v` is zero: every field an int 0, at any depth.
fn pointer_zero(v: &Value, name: &str) -> Result<bool, VmError> {
    match v {
        Value::Int(i) => Ok(*i == 0),
        Value::Struct(s) => {
            for f in &s.fields {
                if !pointer_zero(f, name)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        other => Err(bad(RB, format!("{name} holds {}, not a pointer struct of ints", other.kind_name()))),
    }
}

fn flag(vm: &Vm, o: ObjRef, name: &str) -> Result<bool, VmError> {
    bool_of(&read(vm, o, name, RB)?, name, RB)
}

/// The class table on the attached, resident `entry` (module documentation): reads only. A skeletal entry that
/// passes adds its writes to `writes`, then its attachments in the record under `owner` are planned the same way.
/// `seen` stops a component met twice: a port guard (a data error here); the note does not rule a double visit, and the
/// attach record maps a component to its owner actor, not to an attachment parent (review of #768).
fn plan(
    vm: &Vm,
    owner: ObjRef,
    entry: ObjRef,
    seen: &mut Vec<ObjRef>,
    writes: &mut Vec<SkeletalWrite>,
) -> Result<(), VmError> {
    if seen.contains(&entry) {
        return Err(bad(RB, "a component met twice in the walk"));
    }
    seen.push(entry);
    let class = vm.space.heap.get(entry)?.class;
    let native = super::render_hooks::first_native(vm, class);
    let is = |list: &[&str]| native.as_deref().is_some_and(|n| list.iter().any(|p| n.eq_ignore_ascii_case(p)));
    if is(&SET_UP_NOTHING) {
        return Ok(());
    }
    if !is(&SET_UP_SKELETAL) {
        let named = native.unwrap_or_else(|| ow_attr::eval::class_path(&vm.space, entry));
        return Err(NotYet::not_read(format!("{UNREAD_CLASS} {named}"), RECORD_LAYER).into());
    }
    schemas(vm, entry, &SKELETAL_FIELDS, RB)?;
    if !matches!(element_kind(vm, entry, "ClothMeshIndexData")?, PropKind::Int) {
        return Err(bad(RB, "ClothMeshIndexData is not declared as an array of ints"));
    }
    let PropKind::Struct(attachment) = element_kind(vm, entry, "Attachments")? else {
        return Err(bad(RB, "Attachments is not declared as an array of structs"));
    };
    let member = vm.space.names.find("Component").and_then(|n| vm.space.classes.struct_layout(attachment)?.field(n));
    if !matches!(member, Some((_, d)) if matches!(d.kind, PropKind::Object { .. })) {
        return Err(bad(RB, "an Attachments entry declares no Component object"));
    }
    if flag(vm, entry, "bDisableAllRigidBody")? {
        return Ok(());
    }
    let single = match read(vm, entry, "bUseSingleBodyPhysics", RB)? {
        Value::Int(i) => i,
        other => return Err(bad(RB, format!("bUseSingleBodyPhysics holds {}, not an int", other.kind_name()))),
    };
    if single != 0 {
        return Err(NotYet::layer_absent(RIGID_BODY, RIGID_LAYER, RECORD).into());
    }
    let mesh = object_of(&read(vm, entry, "SkeletalMesh", RB)?, RB)?;
    if flag(vm, entry, "bHasPhysicsAssetInstance")? && mesh.is_some() {
        return Err(NotYet::layer_absent(RIGID_BODY, RIGID_LAYER, RECORD).into());
    }
    let empty_cloth = pointer_zero(&read(vm, entry, "ClothSim", RB)?, "ClothSim")?;
    if empty_cloth && let Some(mesh) = mesh {
        require_resident(vm, mesh)?;
        schemas(vm, mesh, &[(CLOTH_BOOL, Want::Bool)], RB)?;
        if flag(vm, mesh, CLOTH_BOOL)? {
            return Err(NotYet::not_read(CLOTH, RECORD_LAYER).into());
        }
    }
    if flag(vm, entry, "bEnableSoftBodySimulation")? {
        return Err(NotYet::layer_absent(RIGID_BODY, RIGID_LAYER, RECORD).into());
    }
    writes.push(SkeletalWrite { component: entry, empty_cloth });
    let list = read(vm, entry, "Attachments", RB)?;
    for item in list.elements().unwrap_or(&[]) {
        let component = super::body_composition::field(vm, item, "Component", "an Attachments entry")?;
        let Some(component) = object_of(&component, RB)? else { continue };
        require_resident(vm, component)?;
        if crate::actors::attached_to(vm, component, owner)? {
            plan(vm, owner, component, seen, writes)?;
        }
    }
    Ok(())
}

/// `WillowPawn.InitializeRBPhysics()` in Off (module documentation). Every refusal comes before the first write.
pub fn initialize_rb_physics(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let pawn = c.this.ok_or_else(|| bad(RB, "called without a pawn"))?;
    let vm: &Vm = c.vm;
    schemas(
        vm,
        pawn,
        &[
            ("bDeleteMe", Want::Bool),
            ("Components", Want::Array),
            ("CollisionComponent", Want::Object),
            ("bPushesRigidBodies", Want::Bool),
        ],
        RB,
    )?;
    if !matches!(super::component_setters::declared(vm, pawn, "Physics", RB)?, PropKind::Byte { .. }) {
        return Err(bad(RB, "Physics is not declared as a byte"));
    }
    let physics = match read(vm, pawn, "Physics", RB)? {
        Value::Byte(b) => b,
        other => return Err(bad(RB, format!("Physics holds {}, not a byte", other.kind_name()))),
    };
    if bool_of(&read(vm, pawn, "bDeleteMe", RB)?, "bDeleteMe", RB)? {
        return Ok(Value::None);
    }
    let template = matches!(vm.space.heap.get(pawn)?.origin, Origin::Default { .. });
    let collision = object_of(&read(vm, pawn, "CollisionComponent", RB)?, RB)?;
    let components = read(vm, pawn, "Components", RB)?;
    let mut walk = Vec::new();
    for item in components.elements().unwrap_or(&[]) {
        let Some(entry) = object_of(item, RB)? else { continue };
        if physics == PHYS_RIGID_BODY && Some(entry) != collision {
            continue;
        }
        walk.push(entry);
    }
    let pushing = if bool_of(&read(vm, pawn, "bPushesRigidBodies", RB)?, "bPushesRigidBodies", RB)? { collision } else { None };
    // The first pass: every entry's checks, before any answer.
    for &entry in walk.iter().chain(pushing.iter()) {
        check_entry(vm, template, entry)?;
    }
    if template {
        // Review r1 of #763: a template receiver keeps the physics `LayerAbsent` and is never written.
        for &entry in walk.iter().chain(pushing.iter()) {
            answer(attached(vm, pawn, template, entry)?)?;
        }
        return Ok(Value::None);
    }
    // A live receiver: the class table, every refusal before the first write.
    let fixed = physics != PHYS_RIGID_BODY;
    let (mut seen, mut writes) = (Vec::new(), Vec::new());
    for &entry in &walk {
        match attached(vm, pawn, false, entry)? {
            Some(_) => plan(vm, pawn, entry, &mut seen, &mut writes)?,
            None => answer(None)?,
        }
    }
    // `SetPushesRigidBodies`' own refusal, folded into the preflight (the hub's ruling).
    if let Some(entry) = pushing {
        answer(attached(vm, pawn, false, entry)?)?;
    }
    for w in writes {
        write(c.vm, w.component, "bSkelCompFixed", Value::Bool(fixed), RB)?;
        if w.empty_cloth {
            write(c.vm, w.component, "ClothMeshIndexData", Value::array(Vec::new()), RB)?;
        }
    }
    Ok(Value::None)
}

/// The 2.D conversion of one byte.
pub fn byte_to_linear(b: u8) -> f32 {
    const SCALE: f32 = 1.0 / 255.0;
    f32::from(b) * SCALE
}

/// `WillowPlayerPawn.GetLinearColor(Color C)` (2.D).
pub fn get_linear_color(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "WillowPlayerPawn.GetLinearColor";
    let vm: &Vm = c.vm;
    let color = c.arg(0)?;
    let s = color.as_struct().ok_or_else(|| bad(WHAT, format!("C holds {}, not a struct", color.kind_name())))?;
    let layout = vm.space.classes.struct_layout(s.ty).ok_or_else(|| bad(WHAT, "a Color with no layout"))?;
    let field = |name: &str| -> Result<u8, VmError> {
        let (i, _) = vm.space.names.find(name).and_then(|n| layout.field(n)).ok_or_else(|| bad(WHAT, format!("Color has no {name}")))?;
        match s.fields.get(i as usize) {
            Some(Value::Byte(b)) => Ok(*b),
            other => Err(bad(WHAT, format!("Color.{name} holds {other:?}, not a byte"))),
        }
    };
    let rgba = [field("R")?, field("G")?, field("B")?, field("A")?];
    let ty = match vm.function(c.function).and_then(|d| d.result.as_ref()).map(|r| &r.kind) {
        Some(PropKind::Struct(t)) => *t,
        _ => return Err(bad(WHAT, "the return value is not declared as a struct")),
    };
    let out = vm.space.classes.struct_layout(ty).ok_or_else(|| bad(WHAT, "a LinearColor with no layout"))?;
    let mut fields = match out.zero() {
        Value::Struct(z) => z.fields.clone(),
        _ => return Err(bad(WHAT, "a LinearColor with no zero value")),
    };
    for (name, b) in ["R", "G", "B", "A"].into_iter().zip(rgba) {
        let (i, decl) =
            vm.space.names.find(name).and_then(|n| out.field(n)).ok_or_else(|| bad(WHAT, format!("LinearColor has no {name}")))?;
        if !matches!(decl.kind, PropKind::Float) {
            return Err(bad(WHAT, format!("LinearColor.{name} is not a float")));
        }
        fields[i as usize] = Value::Float(byte_to_linear(b));
    }
    Ok(Value::structure(ty, fields))
}

/// `WillowPlayerPawn.OnNativeApplyBodyClassStartingValuesHook()` (2.F).
pub fn apply_hook(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "WillowPlayerPawn.OnNativeApplyBodyClassStartingValuesHook";
    let pawn = c.this.ok_or_else(|| bad(WHAT, "called without a pawn"))?;
    schemas(c.vm, pawn, &[("ImmuneToFallingDamage", Want::Bool)], WHAT)?;
    write(c.vm, pawn, "ImmuneToFallingDamage", Value::Bool(true), WHAT)?;
    Ok(Value::None)
}

/// Binds the three keys (Off only; the caller checks the mode). A key bound first keeps its binding.
pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    for (key, f) in [
        (INITIALIZE_RB_PHYSICS, initialize_rb_physics as fn(&mut NativeCall<'_>) -> Result<Value, VmError>),
        (GET_LINEAR_COLOR, get_linear_color),
        (APPLY_HOOK, apply_hook),
    ] {
        if !vm.has_native(key) {
            vm.register_native(key, Rc::new(f))?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "player_pawn_tail_tests.rs"]
mod tests;
