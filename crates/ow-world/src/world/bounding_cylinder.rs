//! Owner: lane IMPL-GET-BOUNDING-CYLINDER (decision 2026-11-19-203). `Actor.GetBoundingCylinder(out float, out float)`
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
//! 2. **Template or live**, from the heap `Origin` of the receiver and of every object on its outer chain
//!    ([`template`]), read-only and asked only where it is used. A class default anywhere on the chain wins: a
//!    template. Otherwise an export or resource stand-in anywhere on the chain (the receiver included) is refusal
//!    **T**. Only a chain made wholly of run-time objects is live.
//! 3. **The pawn rule:** a `CylinderComponent` that differs from the `CollisionComponent`, on a template, and set,
//!    answers that component's `CollisionRadius` and `CollisionHeight`. Otherwise the actor default. No class test is
//!    made on the component.
//! 4. **The navigation-point rule:** a set `CylinderComponent` answers its radius and height, live or template.
//!    Otherwise the actor default.
//! 5. **The actor default** ([`actor_default`]), for each `Components` entry in order: None and a non-primitive
//!    (decided from the class the handle carries) are skipped; a lazy handle is refusal **N**, never a load; then
//!    `bAttached`, read from the entry on a template (clear: skipped), refusal **L** on a live receiver (the port does
//!    not track attachment), refusal **T** on an export or resource receiver; then `CollideActors` clear is skipped;
//!    then the skip flag, which was refusal **S** and is now read (lane IMPL-SKIP-FLAG, below). `Bounds` comes next, and
//!    is refusal L. No entry left gives 0 and 0, through main's
//!    `placement::box_cylinder` of no boxes.
//!
//! **Lane IMPL-COMPONENT-BOUNDS** (coded from NATIVE_COMPONENT_BOUNDS.md alone, analyst lane ANALYST-COMPONENT-BOUNDS,
//! PR #639, and the hub's rulings on its review; clean room). On a **live** receiver the actor default now runs, per
//! entry: None and a non-primitive skipped; a lazy handle refusal N; **`CollideActors` clear skipped** (before
//! attachment is asked about); **not attached by the attach record** ([`crate::actors::attached_to`]: recorded with
//! this receiver as owner, still owned by it, neither destroyed) refusal L; then refusal S, which lane IMPL-SKIP-FLAG
//! lifts (below). Past the skip check, an entry's box comes from [`entry_box`], at query time from stored fields
//! (nothing is written to `Bounds`).
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
//! **Lane IMPL-SKIP-FLAG** (coded from NATIVE_COMPONENT_SKIP_FLAG.md section 5 alone, as accepted at 4fd8587d; analyst
//! lane ANALYST-SKIP-FLAG, PR #653, with the hub's rulings; clean room). Refusal S is lifted on both paths:
//! - **the skip check**, where S stood: `BulletListener` (the bool the note reads as bit 10 of the collision word; the
//!   name is a structural fit, UNVERIFIED) set skips the entry, clear goes on;
//! - **the per-component check** ([`component_check`]), asked last, only of an entry every other test would count,
//!   right before its box: an entry whose exact class is one of [`COMPONENT_CHECK_PASSES`] passes (read). Any other
//!   class, the skeletal, static-mesh and particle components among them, is `NotYet { NotRead }`
//!   ([`COMPONENT_CHECK_OF`]);
//! - **the box:** [`entry_box`] on a live receiver, refusal L on a template (#595's bounds step).
//!
//! Every refusal comes before the native's only write, so none carries the "stopped after partial writes: " prefix
//! from inside it; a caller that wrote earlier marks its own.

use std::rc::Rc;

use ow_core::{DetSet, NotYet};
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

use crate::actors::{attached_to, declared_flag, field, flag, float, object, objects};
use crate::placement::{BOUNDING_CYLINDER_OF, EXTENT_RECORD, PAWN_RULE_CLASSES, box_cylinder};

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
/// The exact classes whose per-component check passes (NATIVE_COMPONENT_SKIP_FLAG.md section 3; UNVERIFIED in game).
pub const COMPONENT_CHECK_PASSES: [&str; 3] =
    ["Engine.PrimitiveComponent", "Engine.CylinderComponent", "WillowGame.BulletListenerComponent"];
/// The refusal of an entry whose per-component check was not read (followed by its class path).
pub const COMPONENT_CHECK_OF: &str = "Actor.GetBoundingCylinder: the component check of";
/// The skip check's field (bit 10 of the collision bool word by the note; the name is UNVERIFIED).
pub const SKIP_FIELD: &str = "BulletListener";
/// The record of the per-component check and the skip check.
pub const SKIP_RECORD: &str = "docs/verification/NATIVE_COMPONENT_SKIP_FLAG.md";
/// The refusal of a `Components` entry whose class chain runs through an engine-native stand-in class before it
/// reaches `PrimitiveComponent` (followed by that class's path; lane UNDECLARED-COMPONENT-CLASS).
pub const NATIVE_STAND_IN_CLASS: &str = "a Components entry whose class runs through the engine-native stand-in class \
                                         (decision 2026-11-15-08), so whether it is a primitive is not known:";

/// Whether `class` is `primitive` or derives from it, for a `Components` walk (lane UNDECLARED-COMPONENT-CLASS,
/// decision 2026-11-25-NNN-undeclared-component-class.md; a port policy, UNVERIFIED in game). The chain is walked from
/// `class` up: reaching `primitive` is yes. Reaching an engine-native stand-in class first (a class with no package
/// export, which the definition store defines directly under `Core.Object`, decision 2026-11-15-08) is
/// `NotYet { NotRead }` with `prefix`, the class and `record`: its real super chain is not known, so the game might
/// count the entry, and the VM's casts answer the same (`interp::is_child`). The end of a chain of package classes
/// is no. `primitive` is `None` when the class table has no `Engine.PrimitiveComponent`: no package class can then be
/// a primitive, but a stand-in on the chain is still refused (review r1 of #681). A class that is only declared
/// cannot reach here: the heap makes no object of one, and a class is defined
/// only after its super class. Read only.
pub fn primitive_class(
    space: &ObjectSpace,
    class: ow_object::ids::ClassId,
    primitive: Option<ow_object::ids::ClassId>,
    prefix: &str,
    record: &'static str,
) -> Result<bool, VmError> {
    let mut cursor = Some(class);
    while let Some(c) = cursor {
        if Some(c) == primitive {
            return Ok(true);
        }
        let Some(layout) = space.classes.get_layout(c) else {
            return Ok(false);
        };
        if space.classes.is_native(c) {
            let what = format!("{prefix}: {NATIVE_STAND_IN_CLASS} {}", layout.path().as_str());
            return Err(NotYet::not_read(what, record).into());
        }
        cursor = layout.super_class();
    }
    Ok(false)
}
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
    // Both destinations are checked before either write, so a bad height destination cannot leave the radius written
    // (review r1 of #660, item 1).
    let (radius, height) = (Value::Float(radius), Value::Float(height));
    c.check_out(0, &radius)?;
    c.check_out(1, &height)?;
    c.set_out(0, radius)?;
    c.set_out(1, height)?;
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

/// Step 2: whether `this` is a template. The game tests the receiver and every object on its outer chain for the
/// class-default and archetype flags, and the port reads that chain from heap origins, read-only (heap reads only, no
/// load, no write):
/// - a class default anywhere on the chain: a template;
/// - otherwise an export or a resource stand-in on the chain (the receiver included): refusal T, because it could be
///   a placed object or an archetype (the port keeps no archetype flag);
/// - otherwise (every object on the chain made at run time): live.
///
/// A run-time object is not enough on its own: VM `new` can give it any outer, a template among them (a recheck of
/// #595). UNVERIFIED in the running game.
fn template(space: &ObjectSpace, this: ObjRef) -> Result<bool, VmError> {
    let mut seen: DetSet<ObjRef> = DetSet::new();
    let mut unknown = false;
    let mut cursor = Some(this);
    // `seen` ends the walk on a cycle, which the heap should never hold.
    while let Some(o) = cursor.filter(|&o| seen.insert(o)) {
        let obj = space.heap.get(o)?;
        match obj.origin {
            Origin::Default { .. } => return Ok(true),
            Origin::Runtime => {}
            // An export, a resource stand-in, or any origin the heap adds later.
            _ => unknown = true,
        }
        cursor = obj.outer;
    }
    if unknown {
        return Err(NotYet::not_read(REFUSAL_T, RECORD).into());
    }
    Ok(false)
}

/// A cylinder component's (CollisionRadius, CollisionHeight), from the resident component (else refusal N).
fn cylinder_of(vm: &Vm, c: ObjRef) -> Result<(f32, f32), VmError> {
    resident(vm, c)?;
    Ok((float(&vm.space, c, "CollisionRadius")?, float(&vm.space, c, "CollisionHeight")?))
}

/// Step 5, the actor default (module documentation).
fn actor_default(vm: &Vm, this: ObjRef) -> Result<(f32, f32), VmError> {
    let space = &vm.space;
    let primitive = space.classes.by_path("Engine.PrimitiveComponent");
    let mut boxes = Vec::new();
    for c in objects(space, this, "Components")?.into_iter().flatten() {
        let class = space.heap.get(c)?.class;
        // A chain through an engine-native stand-in is refused, not skipped (lane UNDECLARED-COMPONENT-CLASS), also
        // when the table has no PrimitiveComponent (review r1 of #681).
        if !primitive_class(space, class, primitive, "Actor.GetBoundingCylinder", RECORD)? {
            continue;
        }
        resident(vm, c)?;
        if template(space, this)? {
            // #595's template path: bAttached, CollideActors, the skip check, the component check, then the bounds,
            // which the port does not compute for a template: L.
            if !flag(space, c, "bAttached")? || !flag(space, c, "CollideActors")? || declared_flag(space, c, SKIP_FIELD)? {
                continue;
            }
            component_check(space, c)?;
            return Err(bounds_refusal());
        }
        // A live receiver (lane IMPL-COMPONENT-BOUNDS, note section 7 B): a non-colliding entry is never counted,
        // attached or not.
        if !flag(space, c, "CollideActors")? {
            continue;
        }
        if !attached_to(vm, c, this)? {
            return Err(bounds_refusal());
        }
        // The skip check (lane IMPL-SKIP-FLAG): a bullet listener is left out of the box.
        if declared_flag(space, c, SKIP_FIELD)? {
            continue;
        }
        // The per-component check, right before the box (the hub's ruling on #653).
        component_check(space, c)?;
        boxes.push(entry_box(vm, this, c)?);
    }
    let [radius, _, height] = box_cylinder(&boxes);
    Ok((radius, height))
}

/// The per-component check (NATIVE_COMPONENT_SKIP_FLAG.md section 3): it passes for the exact classes of
/// [`COMPONENT_CHECK_PASSES`] (read: they always pass). Every other class's answer was not read, so it is
/// `NotYet { NotRead }` ([`COMPONENT_CHECK_OF`]), the hub's ruling. Classes are looked up through `by_path` only. The
/// virtual is taken as a pure predicate (UNVERIFIED: only its answers were read), so it is asked last.
pub(crate) fn component_check(space: &ObjectSpace, c: ObjRef) -> Result<(), VmError> {
    let class = space.heap.get(c)?.class;
    if COMPONENT_CHECK_PASSES.iter().any(|p| space.classes.by_path(p) == Some(class)) {
        return Ok(());
    }
    Err(NotYet::not_read(format!("{COMPONENT_CHECK_OF} {} (not read)", class_path(space, c)), SKIP_RECORD).into())
}

/// The cylinder entry's refusal when its world position is not the owner's `Location` by the note's rule.
pub const WORLD_TRANSFORM: &str = "Actor.GetBoundingCylinder: the component's world transform";
/// A skeletal mesh entry: the bounds update's early returns and source choice were not read.
pub const SKELETAL_SOURCE: &str = "Actor.GetBoundingCylinder: a skeletal mesh component's bounds source";
/// A skeletal mesh entry on the branch that uses the current bone transforms, and its layer (#599's).
pub const SKELETAL_BOUNDS: &str = "Actor.GetBoundingCylinder: a skeletal mesh component's bounds";
pub const BONES_LAYER: &str = "a skeletal mesh component's bone transforms";
/// A static mesh entry with a mesh, and its layer.
pub const STATIC_BOUNDS: &str = "Actor.GetBoundingCylinder: a static mesh component's bounds";
pub const STATIC_LAYER: &str = "a static mesh's stored bounds";
/// The record of the entry rules.
pub const BOUNDS_RECORD: &str = "docs/verification/NATIVE_COMPONENT_BOUNDS.md";

/// One counted entry's box (origin, box extent), by its exact class, at query time from stored fields; writes, loads
/// and draws nothing (NATIVE_COMPONENT_BOUNDS.md section 7 B, step 6; UNVERIFIED):
/// - `Engine.CylinderComponent`: extent (R, R, H) from `CollisionRadius` and `CollisionHeight` (floats), origin the
///   owner's `Location`, only when the component's `Translation` is zero, none of `AbsoluteTranslation`,
///   `AbsoluteRotation` and `AbsoluteScale` is set, and its `Owner` is `owner`. Else `NotYet { NotRead }`
///   ([`WORLD_TRANSFORM`]);
/// - `Engine.SkeletalMeshComponent`: `NotYet { NotRead }` ([`SKELETAL_SOURCE`]). The two early returns and the choice
///   between the physics-asset branch (which uses the current bone transforms: `LayerAbsent`, [`BONES_LAYER`]) and
///   the mesh's stored bounds were not read, so no branch can be picked;
/// - `Engine.StaticMeshComponent`: `NotYet { LayerAbsent }` ([`STATIC_LAYER`]);
/// - any other primitive class: `NotYet { NotRead }`, "the bounds update of <class>".
pub fn entry_box(vm: &Vm, owner: ObjRef, c: ObjRef) -> Result<([f32; 3], [f32; 3]), VmError> {
    let space = &vm.space;
    let class = space.heap.get(c)?.class;
    let exactly = |p: &str| space.classes.by_path(p) == Some(class);
    if exactly("Engine.CylinderComponent") {
        let r = float(space, c, "CollisionRadius")?;
        let h = float(space, c, "CollisionHeight")?;
        let translation = vector(space, c, &field(space, c, "Translation")?, "Translation")?;
        let mut absolute = false;
        for name in ["AbsoluteTranslation", "AbsoluteRotation", "AbsoluteScale"] {
            absolute |= flag(space, c, name)?;
        }
        if translation != [0.0; 3] || absolute || object(space, c, "Owner")? != Some(owner) {
            return Err(NotYet::not_read(WORLD_TRANSFORM, BOUNDS_RECORD).into());
        }
        let origin = vector(space, owner, &field(space, owner, "Location")?, "Location")?;
        return Ok((origin, [r, r, h]));
    }
    if exactly("Engine.SkeletalMeshComponent") {
        return Err(NotYet::not_read(SKELETAL_SOURCE, BOUNDS_RECORD).into());
    }
    if exactly("Engine.StaticMeshComponent") {
        return Err(NotYet::layer_absent(STATIC_BOUNDS, STATIC_LAYER, BOUNDS_RECORD).into());
    }
    Err(NotYet::not_read(format!("Actor.GetBoundingCylinder: the bounds update of {}", class_path(space, c)), BOUNDS_RECORD)
        .into())
}

/// The (X, Y, Z) floats of vector field `name` of `o`, read by name lookup only (no interning). Another shape is a
/// `VmError::Script` naming the field.
fn vector(space: &ObjectSpace, o: ObjRef, v: &Value, name: &str) -> Result<[f32; 3], VmError> {
    let bad = |why: String| VmError::Script {
        function: NATIVE.to_string(),
        message: format!("{}.{name} {why}", class_path(space, o)),
    };
    let s = v.as_struct().ok_or_else(|| bad(format!("is a {}, not a vector", v.kind_name())))?;
    let layout = space.classes.struct_layout(s.ty).ok_or_else(|| bad("has no struct layout".to_string()))?;
    let mut out = [0.0; 3];
    for (k, n) in ["X", "Y", "Z"].iter().enumerate() {
        let id = space.names.find(n).ok_or_else(|| bad(format!("has no field {n}")))?;
        let (entry, _) = layout.field(id).ok_or_else(|| bad(format!("has no field {n}")))?;
        let i = usize::try_from(entry).map_err(|_| bad(format!("field {n} is out of range")))?;
        out[k] = match s.fields.get(i) {
            Some(Value::Float(f)) => *f,
            Some(other) => return Err(bad(format!("{n} holds a {}", other.kind_name()))),
            None => return Err(bad(format!("lacks {n}"))),
        };
    }
    Ok(out)
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
