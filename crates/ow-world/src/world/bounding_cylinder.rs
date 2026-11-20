//! Owner: lane IMPL-GET-BOUNDING-CYLINDER (decision 2026-11-19-NNN). `Actor.GetBoundingCylinder(out float, out float)`
//! and its `Pawn` and `NavigationPoint` registrations, in Off. AI-assisted (Claude Code).
//!
//! Coded from NATIVE_GET_BOUNDING_CYLINDER.md alone, the note as accepted at 6800f603 (analyst lane
//! ANALYST-GET-BOUNDING-CYLINDER, PR #590; clean room: no native code or analysis output was opened). **Every rule is
//! UNVERIFIED in the running game.** Off only: under `ParityMode::Reference` the C++ binds no `GetBoundingCylinder`, so
//! the VM's stub policy answers (R3).
//!
//! **The answer** ([`answer`]), in the note's section 4 order. It computes both values first; the native writes its two
//! out arguments last, its only effect, so a refusal or a data error writes neither:
//! 1. **The body, by the receiver's exact class:** main's `placement::PAWN_RULE_CLASSES` take the pawn rule,
//!    `Engine.NavigationPoint` the navigation-point rule, `Engine.Actor` and `WillowGame.WillowPickup` the actor default.
//!    Any other class is refusal **C**, main's "the bounding-cylinder rule of <class>" on main's record.
//! 2. **Template or live**, from the receiver's heap `Origin` ([`template`]), asked only where it is used: a class
//!    default is a template, a run-time object is live, and an export or resource stand-in is refusal **T**.
//! 3. **The pawn rule:** a `CylinderComponent` that differs from the `CollisionComponent`, on a template, and set,
//!    answers that component's `CollisionRadius` and `CollisionHeight`. Otherwise the actor default. No class test is
//!    made on the component.
//! 4. **The navigation-point rule:** a set `CylinderComponent` answers its radius and height, live or template.
//!    Otherwise the actor default.
//! 5. **The actor default** ([`actor_default`]), for each `Components` entry in order: None and a non-primitive
//!    (decided from the class the handle carries) are skipped; a lazy handle is refusal **N**, never a load; then
//!    `bAttached`, read from the entry on a template (clear: skipped), refusal **L** on a live receiver (the port does
//!    not track attachment), refusal **T** on an export or resource receiver; then `CollideActors` clear is skipped;
//!    then the skip flag, refusal **S** with main's `trace_filter::SKIP_FLAG` text. `Bounds` would come next; it is not
//!    reached while S refuses, and is refusal L when it is. No entry left gives 0 and 0, through main's
//!    `placement::box_cylinder` of no boxes.
//!
//! **Nothing is loaded.** Classes are looked up only through the class table's `by_path`; a class not in the table
//! counts as "not this class". The cylinder components and the walked primitive entries that the store holds as lazy
//! handles (or as stand-ins with no slots) are refusal N. A receiver that is itself a lazy handle is refusal N too: the
//! note's contract is load-free, and its fields cannot be read without a load (this port's reading of the contract).
//!
//! **Malformed data** (a field the class does not declare, `Components` that is not an array of objects, an object or
//! bool field of another kind) is a `VmError::Script` naming the field, through the world's field reads
//! (`crate::actors`).
//!
//! Every refusal comes before the native's only write, so none carries the "stopped after partial writes: " prefix
//! from inside it; a caller that wrote earlier marks its own.

use std::rc::Rc;

use ow_core::NotYet;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

use crate::actors::{flag, float, object, objects};
use crate::placement::{BOUNDING_CYLINDER_OF, EXTENT_RECORD, PAWN_RULE_CLASSES, box_cylinder};
use crate::trace_filter::SKIP_FLAG;

/// The record of refusals T, N, L and S.
pub const RECORD: &str = "docs/verification/NATIVE_GET_BOUNDING_CYLINDER.md";
/// The three native keys (one exec step registered for `Actor`, `Pawn` and `NavigationPoint`).
pub const KEYS: [&str; 3] = [
    "Actor.GetBoundingCylinder(float,float)",
    "Pawn.GetBoundingCylinder(float,float)",
    "NavigationPoint.GetBoundingCylinder(float,float)",
];
/// The classes that take the actor default (step 1).
pub const ACTOR_DEFAULT_CLASSES: [&str; 2] = ["Engine.Actor", "WillowGame.WillowPickup"];
/// The class that takes the navigation-point rule (step 1).
pub const NAVIGATION_POINT_CLASS: &str = "Engine.NavigationPoint";
/// Refusal T's text.
pub const REFUSAL_T: &str = "Actor.GetBoundingCylinder: whether the actor is a template";
/// Refusal N's text.
pub const REFUSAL_N: &str = "Actor.GetBoundingCylinder: a component the store has not loaded (a load is a write)";
/// Refusal L's text and layer (the pickup targeting's layer text).
pub const REFUSAL_L: &str = "Actor.GetBoundingCylinder: the colliding components' bounds";
pub const BOUNDS_LAYER: &str = "component bounds (the headless world has none)";
/// The native's name in its errors.
const NATIVE: &str = "Actor.GetBoundingCylinder";

/// Binds the three keys (Off only; the caller checks the mode).
pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    for key in KEYS {
        vm.register_native(key, Rc::new(get_bounding_cylinder))?;
    }
    Ok(())
}

/// The native: the whole answer first, then the two out-argument writes (step 6).
fn get_bounding_cylinder(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let this = c.this.ok_or_else(|| VmError::Script {
        function: NATIVE.to_string(),
        message: "called with no actor".to_string(),
    })?;
    let (radius, height) = answer(c.vm, this)?;
    // Both arguments must exist and be declared `out` before either is written, so a malformed declaration cannot
    // leave the radius written and the height not (review r1 of #595). This covers what `set_out` itself refuses (no
    // such argument, not an out parameter). A failure inside the VM's write through a script caller's alias, after
    // the first write, cannot be checked ahead from here: `NativeCall` exposes no dry run of that write.
    let outs = c.vm.function(c.function).map(|f| (0..2).all(|i| f.params.get(i).is_some_and(|p| p.is_out())));
    if c.arg_count() < 2 || outs != Some(true) {
        return Err(VmError::Script {
            function: NATIVE.to_string(),
            message: "the function does not declare two out arguments".to_string(),
        });
    }
    c.set_out(0, Value::Float(radius))?;
    c.set_out(1, Value::Float(height))?;
    Ok(Value::None)
}

/// The three bodies of step 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Body {
    Pawn,
    NavigationPoint,
    ActorDefault,
}

/// (radius, half height) for `this`, by the module documentation's steps 1 to 5. It writes, loads and draws nothing.
pub fn answer(vm: &Vm, this: ObjRef) -> Result<(f32, f32), VmError> {
    let space = &vm.space;
    let class = space.heap.get(this)?.class;
    let exactly = |p: &str| space.classes.by_path(p) == Some(class);
    let body = if PAWN_RULE_CLASSES.iter().any(|p| exactly(p)) {
        Body::Pawn
    } else if exactly(NAVIGATION_POINT_CLASS) {
        Body::NavigationPoint
    } else if ACTOR_DEFAULT_CLASSES.iter().any(|p| exactly(p)) {
        Body::ActorDefault
    } else {
        return Err(NotYet::not_read(format!("{BOUNDING_CYLINDER_OF} {}", class_path(space, this)), EXTENT_RECORD).into());
    };
    resident(vm, this)?;
    match body {
        Body::Pawn => {
            let cylinder = object(space, this, "CylinderComponent")?;
            let collision = object(space, this, "CollisionComponent")?;
            if cylinder != collision
                && template(space, this)?
                && let Some(c) = cylinder
            {
                return cylinder_of(vm, c);
            }
            actor_default(vm, this)
        }
        Body::NavigationPoint => match object(space, this, "CylinderComponent")? {
            Some(c) => cylinder_of(vm, c),
            None => actor_default(vm, this),
        },
        Body::ActorDefault => actor_default(vm, this),
    }
}

/// Step 2: whether `this` is a template, from its heap origin. An export or a resource stand-in could be a placed actor
/// or an archetype (the port keeps no archetype flag): refusal T. The game also walks the outer chain; the port's
/// run-time objects have no template outers (UNVERIFIED).
fn template(space: &ObjectSpace, this: ObjRef) -> Result<bool, VmError> {
    match space.heap.get(this)?.origin {
        Origin::Default { .. } => Ok(true),
        Origin::Runtime => Ok(false),
        _ => Err(NotYet::not_read(REFUSAL_T, RECORD).into()),
    }
}

/// A cylinder component's (CollisionRadius, CollisionHeight), from the resident component (else refusal N).
fn cylinder_of(vm: &Vm, c: ObjRef) -> Result<(f32, f32), VmError> {
    resident(vm, c)?;
    Ok((float(&vm.space, c, "CollisionRadius")?, float(&vm.space, c, "CollisionHeight")?))
}

/// Step 5, the actor default.
fn actor_default(vm: &Vm, this: ObjRef) -> Result<(f32, f32), VmError> {
    let space = &vm.space;
    let primitive = space.classes.by_path("Engine.PrimitiveComponent");
    for c in objects(space, this, "Components")?.into_iter().flatten() {
        let class = space.heap.get(c)?.class;
        if !primitive.is_some_and(|p| space.classes.is_child_of(class, p)) {
            continue;
        }
        resident(vm, c)?;
        if !template(space, this)? {
            // A live receiver: the port does not track attachment (`AttachComponent` leaves `bAttached` as it was).
            return Err(bounds_refusal());
        }
        if !flag(space, c, "bAttached")? || !flag(space, c, "CollideActors")? {
            continue;
        }
        return Err(NotYet::not_read(SKIP_FLAG, RECORD).into());
    }
    let [radius, _, height] = box_cylinder(&[]);
    Ok((radius, height))
}

/// Refusal L: the colliding components' bounds, which the port does not compute. The one place the bounds step
/// refuses: a later reading of attachment or of component bounds changes only its callers here.
fn bounds_refusal() -> VmError {
    NotYet::layer_absent(REFUSAL_L, BOUNDS_LAYER, RECORD).into()
}

/// Refusal N unless `o` is resident: neither a lazy handle the store has not loaded nor a stand-in with no slots. Never
/// a load.
fn resident(vm: &Vm, o: ObjRef) -> Result<(), VmError> {
    let space = &vm.space;
    let obj = space.heap.get(o)?;
    let full = u32::try_from(obj.slots.len()).ok() == space.classes.get_layout(obj.class).map(|l| l.slot_count());
    if vm.defs.is_unloaded(space, o) || !full {
        return Err(NotYet::not_read(REFUSAL_N, RECORD).into());
    }
    Ok(())
}

/// The class path of `o`, for refusal C's text.
fn class_path(space: &ObjectSpace, o: ObjRef) -> String {
    let class = space.heap.get(o).map(|x| x.class);
    match class {
        Ok(c) => space.classes.get_layout(c).map(|l| l.path().as_str().to_string()).unwrap_or_else(|| format!("{c:?}")),
        Err(_) => format!("{o:?}"),
    }
}

#[cfg(test)]
#[path = "bounding_cylinder_tests.rs"]
mod tests;
