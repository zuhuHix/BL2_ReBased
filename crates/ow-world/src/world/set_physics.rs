//! Owner: lane IMPL-ACTOR-SETPHYSICS. `Actor.SetPhysics(byte)` in `ParityMode::Off`, coded from
//! docs/verification/NATIVE_ACTOR_SET_PHYSICS.md section 3 alone (#688 at f361007, decision 2026-11-25-23). Clean
//! room: the analyst lane read the native, this lane did not. AI-assisted (Claude Code). **Every rule is UNVERIFIED in
//! the running game.** Reference binds nothing here: `ow-combat`'s hit pack keeps its plain `Physics` byte write there
//! (and only there).
//!
//! **The layers by class.** From the actor's class up to its first native class (the fitted native bit, as
//! `ow-combat`'s `pawn_interface` walk takes it): `WillowGame.WillowAIPawn` has the AI, `WillowPawn`, `Pawn` and
//! `Actor` layers; `WillowGame.WillowPawn` the last three; `GearboxFramework.GearboxPawn` and `Engine.Pawn` the `Pawn`
//! and `Actor` layers; `Engine.Actor` the `Actor` layer only; `WillowGame.WillowPlayerPawn` `WillowPawn`'s three (it
//! keeps `WillowPawn`'s body, NATIVE_LINK34_AFTER_MIND.md section 3). Any other first native class is
//! `NotYet { NotRead }`, "Actor.SetPhysics: the physics change of <class>".
//!
//! **Preflight, write-free and load-free** (reads by name lookup on resident objects; each field read must have its
//! declared kind, else `VmError::Script` naming it). The actor itself must be resident. The mode is the argument's low
//! byte. A referenced object (`PhysicsVolume`, `Base`, `CollisionComponent`) is checked for residency only on a path
//! that uses it ([`NOT_LOADED`]). In order:
//! 1. the AI layer: `PHYS_NavMeshWalking` (12) is `NotRead` ([`NAV_MESH`]); `PHYS_RigidBody` (10) with `bTearOff`
//!    clear and `Role` not `ROLE_Authority` is `NotRead` ([`RIGID_EVENT`]); `bHardAttach` set and the mode not
//!    `PHYS_Interpolating` (7) answers at once, with nothing changed;
//! 2. the same mode as now answers at once, with nothing changed;
//! 3. the transitions: into `PHYS_None`, `PHYS_Walking`, `PHYS_Rotating` or `PHYS_Spider` (0, 1, 5, 8) runs the
//!    **base search** (NATIVE_LINK34_AFTER_MIND.md section 2, lane IMPL-LINK34-AFTER-MIND): a floor trace 8 units down,
//!    then the base setter when the floor differs from `Base`. No collision service is `LayerAbsent`
//!    ([`BASE_TRACE`], [`BASE_TRACE_LAYER`]); a service with geometry, or `NoLevel` without the setting below, is
//!    `NotRead` ([`BASE_TRACE_FLAGS`]), decided first. **The stand-in** (the hub's rulings on #793, on review of #798,
//!    R1, and on #809; NATIVE_FLOOR_TRACE_FLAGS.md section 6.1, lane IMPL-FLOOR-TRACE-RULINGS; UNVERIFIED in game):
//!    only with the Off-only harness setting [`NoLevelBaseSearch`] on ([`enable_no_level_base_search`]) and a service
//!    with no geometry ([`crate::placement::NoLevel`]), the level part holds nothing to hit, and the actor part walks
//!    every live actor's primitives in the placement walk's order (`CollisionComponent`, then `Components`), each
//!    only rejecting, in the game walkers' order (the note's section 3):
//!    - a primitive that declares `Owner` and holds None (no owner), or whose owner (`trace_filter::owner_of`) is the
//!      actor itself: passed over;
//!    - the skip flag set, `BlockNonZeroExtent` clear, `CollideActors` clear, or the owner's `bCollideActors` clear:
//!      passed over;
//!    - the primitive's bounds (the bounding-cylinder rule's box) disjoint from the **swept box**: the actor's
//!      collision cylinder (radius, radius, half-height) from `Location` plus its component's `Translation`, swept
//!      down by 8 units, per axis, touching counted as meeting (the hub's ruling 2 on #809). The swept box is read
//!      and checked before the walk: a `CollisionComponent` that is not a cylinder is `NotRead`
//!      ([`BASE_EXTENT`]); a start, extent or bounds that is not finite or not non-negative is `NotRead`
//!      ([`BASE_SWEPT_BOX`]); bounds the rule does not answer keep its own `NotYet`;
//!    - the owner in the actor's `Owner` chain, or the actor in the owner's (`trace_filter::owner_chains`, the hub's
//!      ruling 1 on #809, read in the note's section 4, replacing #798's R2 "common convention"): passed over; a link
//!      the store has not loaded that decides it is `NotRead` ([`BASE_OWNER_LINK`]);
//!    - any primitive left is `NotRead` ([`BASE_CANDIDATE`]: the owner's filter and the primitive's own test are not
//!      ported).
//!
//!    With no candidate left there is no floor: with `Base` None that changes nothing (logged as a harness input at
//!    the commit, [`BASE_STAND_IN`]), with a `Base` set it is `NotRead` ([`BASE_SETTER`]). All decided here, before
//!    the mode write. Then into any other mode but `PHYS_Interpolating`, with a `Base` set, is `NotRead`
//!    ([`BASE_CLEAR`]); into or out of `PHYS_RigidBody` with a `CollisionComponent` set is `LayerAbsent`
//!    ([`RIGID_BODY`], [`RIGID_LAYER`]);
//! 4. every script event the commit will call is resolved now, as Off event dispatch resolves it (the active state,
//!    its parent states, then the class chain; `ow_vm::states::find_loaded_event`) and in loaded code only: with a
//!    `PhysicsVolume`, its `PhysicsChangedFor(actor)` on the volume's class; on the `WillowPawn` layer, each of
//!    `StoppedFalling()`, `BeganFalling(byte)`, `BeganWalking(byte)` and `StoppedWalking()` whose condition will hold
//!    (walking is `PHYS_Walking` or `PHYS_NavMeshWalking`), on the actor. Code not loaded is `NotRead`
//!    ([`code_not_loaded`]); a missing event, or one declared with other parameters, is `VmError::Script`: each
//!    parameter a scalar by value, and the volume event's object parameter of a class that accepts the actor.
//!
//! `Physics` is read after step 1, so the AI layer's answers do not depend on it. The rule is bound for `Engine`'s
//! function only, by its package-qualified identity.
//!
//! With the base search's stand-in, the transition commits are reached: `bForceFloorCheck` set on a transition into
//! `PHYS_Walking` (the `Pawn` layer, before the mode), and `Velocity` and `Acceleration` zeroed into `PHYS_None` or
//! `PHYS_Rotating` (each field of the vector set to 0, after the mode).
//!
//! **Commit, in the game's order** (each handler resolved again, load-free, right before its dispatch, so a callback
//! that changes a receiver's state is followed; review r2 of #696): `Physics` = the mode; with a `PhysicsVolume`, its
//! `PhysicsChangedFor(actor)`; the
//! four `WillowPawn` events, in that order, each when its condition holds (`BeganFalling` and `BeganWalking` get the
//! old mode). A refusal after the write is marked "stopped after partial writes: " when #518's counters moved
//! ([`crate::pools::after_partial_writes`]); `Vm::class_code_loads` (#685) is not on this lane's base.

use std::rc::Rc;

use ow_core::NotYet;
use ow_object::ids::{FuncId, ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

/// The record of the rule.
pub const RECORD: &str = "docs/verification/NATIVE_ACTOR_SET_PHYSICS.md";
/// The native key.
pub const KEY: &str = "Actor.SetPhysics(byte)";
/// A read on an object the store has not loaded.
pub const NOT_LOADED: &str = "Actor.SetPhysics: an object the store has not loaded (a load is a write)";
/// Step 1's nav-mesh check.
pub const NAV_MESH: &str = "Actor.SetPhysics: a WillowAIPawn's nav-mesh walking check";
/// Step 1's rigid-body event.
pub const RIGID_EVENT: &str = "Actor.SetPhysics: a WillowAIPawn's rigid-body event";
/// Step 3's base search (the floor trace and the base setter; NATIVE_LINK34_AFTER_MIND.md section 2).
pub const BASE_TRACE: &str = "Actor.SetPhysics: the base search's floor trace";
pub const BASE_TRACE_LAYER: &str = "collision (the world has no collision service)";
pub const BASE_TRACE_FLAGS: &str = "Actor.SetPhysics: the base search's floor trace (its trace flags are not decoded)";
pub const BASE_SETTER: &str = "Actor.SetPhysics: the base search's base setter";
/// The harness-input line of the stand-in.
pub const BASE_STAND_IN: &str = "harness input: Actor.SetPhysics' base search found no floor (a world with no level \
     geometry and no other actor's primitive the swept box meets; UNVERIFIED stand-in, the hub's rulings on #793, for \
     lane IMPL-COMBAT-WORLD-COLLISION and on #809)";
/// The swept box's extent when the actor's `CollisionComponent` is not a cylinder (NATIVE_FLOOR_TRACE_FLAGS.md section
/// 6.1, item 2).
pub const BASE_EXTENT: &str = "Actor.SetPhysics: the base search's extent (the bounding-cylinder fallback)";
/// A swept box or a candidate's bounds that are not well formed (NATIVE_FLOOR_TRACE_FLAGS.md section 5).
pub const BASE_SWEPT_BOX: &str = "Actor.SetPhysics: the base search's swept box (a malformed extent or bounds)";
/// An `Owner`-chain link the store has not loaded that decides a candidate (the hub's ruling 1 on #809).
pub const BASE_OWNER_LINK: &str =
    "Actor.SetPhysics: the base search's floor trace (a source Owner-chain link the store has not loaded)";
/// A candidate the game's walker would test (NATIVE_FLOOR_TRACE_FLAGS.md section 6.1, item 4).
pub const BASE_CANDIDATE: &str = "Actor.SetPhysics: the base search's floor trace (a candidate the walker tests: the \
     owner filter's blocked-by test and the primitive's swept-box test are not ported)";
/// The set-up line of [`enable_no_level_base_search`].
pub const BASE_STAND_IN_INPUT: &str = "harness input: the world turns on SetPhysics' no-floor base search for a world \
     with no level geometry (the hub's ruling on review of #798, R1; UNVERIFIED)";
/// The registration key of [`NoLevelBaseSearch`].
pub const BASE_STAND_IN_KEY: &str = "NoLevelBaseSearch";

/// The Off-only harness setting of the base search's stand-in (a VM service; its presence turns it on; module
/// documentation, step 3).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoLevelBaseSearch;

/// Turns the base search's stand-in on for `vm`'s world (world set-up), logging [`BASE_STAND_IN_INPUT`]. Off only, else
/// a registration refusal; turning it on twice is a registration refusal too.
pub fn enable_no_level_base_search(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ow_core::ParityMode::Off {
        return Err(VmError::Registration {
            key: BASE_STAND_IN_KEY.to_string(),
            reason: "the base search's stand-in is an Off harness setting; Reference keeps the refusal",
        });
    }
    vm.add_service(NoLevelBaseSearch)?;
    vm.log.push(BASE_STAND_IN_INPUT.to_string());
    Ok(())
}

/// Step 3's base clearing.
pub const BASE_CLEAR: &str = "Actor.SetPhysics: clearing the base (the base setter is not ported)";
/// Step 3's rigid body, and its layer.
pub const RIGID_BODY: &str = "Actor.SetPhysics: the collision component's rigid body";
pub const RIGID_LAYER: &str = "rigid-body physics (the headless world has none)";
const WHAT: &str = "Actor.SetPhysics";

const PHYS_NONE: u8 = 0;
const PHYS_WALKING: u8 = 1;
const PHYS_FALLING: u8 = 2;
const PHYS_ROTATING: u8 = 5;
const PHYS_INTERPOLATING: u8 = 7;
const PHYS_SPIDER: u8 = 8;
const PHYS_RIGID_BODY: u8 = 10;
const PHYS_NAV_MESH_WALKING: u8 = 12;

/// Step 4's text for an event whose code is not loaded.
pub fn code_not_loaded(event: &str, class: &str) -> String {
    format!("Actor.SetPhysics: the code of {event} on {class} is not loaded")
}

/// The layers of a class (module documentation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Layers {
    ai: bool,
    willow: bool,
    pawn: bool,
}

/// The package whose `Actor.SetPhysics` the rule is for.
pub const PACKAGE: &str = "Engine";

/// Binds the rule in Off for `Engine`'s function only (its package-qualified identity, review r1 of #696, item 3):
/// keep-existing there, and another package's same-keyed function is neither bound nor shadowing. Nothing under
/// Reference (the caller binds in Off only).
pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    if !vm.is_bound_in(PACKAGE, KEY) {
        vm.register_native_in(PACKAGE, KEY, Rc::new(set_physics))?;
    }
    Ok(())
}

fn script(message: String) -> VmError {
    VmError::Script { function: WHAT.to_string(), message: format!("{WHAT}: {message}") }
}

/// `NotYet { NotRead }` ([`NOT_LOADED`]) unless `o` is resident: not a store handle still unloaded (or failed), and not
/// a stand-in with fewer slots than its class.
fn resident(vm: &Vm, o: ObjRef) -> Result<(), VmError> {
    let x = vm.space.heap.get(o)?;
    let short = vm.space.classes.get_layout(x.class).is_some_and(|l| x.slots.len() < l.slot_count() as usize);
    if vm.defs.is_unloaded(&vm.space, o) || vm.defs.is_failed(&vm.space, o) || short {
        return Err(NotYet::not_read(NOT_LOADED, RECORD).into());
    }
    Ok(())
}

/// Field `name` of the resident `o`, by name lookup, with its declaration; a class that declares none is a script
/// error naming it.
fn field(vm: &Vm, o: ObjRef, name: &str) -> Result<(PropKind, Value), VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| script(format!("{} declares no {name}", class_path(vm, o))))?;
    let kind = vm.space.classes.decl(p).map(|d| d.kind.clone()).ok_or_else(|| script(format!("{name} has no declaration")))?;
    Ok((kind, vm.space.read(o, PropAddr::scalar(p))?.clone()))
}

fn byte(vm: &Vm, o: ObjRef, name: &str) -> Result<u8, VmError> {
    match field(vm, o, name)? {
        (PropKind::Byte { .. }, Value::Byte(b)) => Ok(b),
        (k, _) => Err(script(format!("{name} is declared as {}, not a byte", k.kind_name()))),
    }
}

fn flag(vm: &Vm, o: ObjRef, name: &str) -> Result<bool, VmError> {
    match field(vm, o, name)? {
        (PropKind::Bool, Value::Bool(b)) => Ok(b),
        (k, _) => Err(script(format!("{name} is declared as {}, not a bool", k.kind_name()))),
    }
}

fn object(vm: &Vm, o: ObjRef, name: &str) -> Result<Option<ObjRef>, VmError> {
    match field(vm, o, name)? {
        (PropKind::Object { .. }, Value::Object(x)) => Ok(x),
        (k, _) => Err(script(format!("{name} is declared as {}, not an object", k.kind_name()))),
    }
}

fn class_path(vm: &Vm, o: ObjRef) -> String {
    vm.space.heap.get(o).map(|x| vm.space.classes.layout(x.class).path().to_string()).unwrap_or_default()
}

/// The layers of `o`'s class, from its first native class.
fn layers(vm: &Vm, o: ObjRef) -> Result<Layers, VmError> {
    let code = vm.defs.code();
    let mut cursor = Some(vm.space.heap.get(o)?.class);
    while let Some(c) = cursor {
        let layout = vm.space.classes.layout(c);
        let native = match code.key_of(c) {
            Some(k) => code.class(k).body.as_ref().map_or(true, |b| b.is_native()),
            None => vm.space.classes.is_native(c),
        };
        if native {
            let path = layout.path().to_string();
            let is = |p: &str| path.eq_ignore_ascii_case(p);
            return match () {
                _ if is("WillowGame.WillowAIPawn") => Ok(Layers { ai: true, willow: true, pawn: true }),
                _ if is("WillowGame.WillowPawn") || is("WillowGame.WillowPlayerPawn") => {
                    Ok(Layers { ai: false, willow: true, pawn: true })
                }
                _ if is("GearboxFramework.GearboxPawn") || is("Engine.Pawn") => {
                    Ok(Layers { ai: false, willow: false, pawn: true })
                }
                _ if is("Engine.Actor") => Ok(Layers { ai: false, willow: false, pawn: false }),
                _ => Err(NotYet::not_read(format!("Actor.SetPhysics: the physics change of {path}"), RECORD).into()),
            };
        }
        cursor = layout.super_class();
    }
    Err(NotYet::not_read(format!("Actor.SetPhysics: the physics change of {}", class_path(vm, o)), RECORD).into())
}

/// Whether `o`'s `Role` is exactly `ROLE_Authority`, by the `Role` property's enum.
fn authority(vm: &Vm, o: ObjRef) -> Result<bool, VmError> {
    let (kind, value) = field(vm, o, "Role")?;
    let (PropKind::Byte { enum_: Some(e) }, Value::Byte(raw)) = (kind, value) else {
        return Err(script("Role is not declared as an enum byte".to_string()));
    };
    let index = vm
        .space
        .names
        .find("ROLE_Authority")
        .and_then(|id| vm.space.classes.enum_layout(e).and_then(|l| l.index_of(id)))
        .ok_or_else(|| script("Role's enum has no ROLE_Authority".to_string()))?;
    Ok(usize::from(raw) == index)
}

/// Which parameters an event must declare.
#[derive(Clone, Copy)]
enum Shape {
    None,
    Byte,
    Object,
}

/// Step 4: the event `name` sent to `o`, in loaded code only, with the parameters `shape`. The lookup is the one Off
/// event dispatch makes (`ow_vm::states::find_loaded_event`): the active state, its parent states, then the class
/// chain (review r1 of #696, item 1). The preflight validates it, and the commit resolves it again right before each
/// dispatch, since an earlier callback may change the receiver's state (review r2 of #696). Each parameter must be a scalar
/// by value (no static array, no `out`), and an object parameter's class must accept `actor` (item 2).
fn event(vm: &Vm, o: ObjRef, name: &str, shape: Shape, actor: ObjRef) -> Result<FuncId, VmError> {
    let class = class_path(vm, o);
    let f = match ow_vm::states::find_loaded_event(vm, o, name)? {
        None => return Err(NotYet::not_read(code_not_loaded(name, &class), RECORD).into()),
        Some(None) => return Err(script(format!("{class} declares no {name}"))),
        Some(Some(f)) => f,
    };
    let params = vm.function(f).map(|d| d.params.clone()).unwrap_or_default();
    let actor_class = vm.space.heap.get(actor)?.class;
    let scalar = |p: &ow_object::layout::PropertyDecl| p.array_dim == 1 && !p.is_out();
    let fits = match (shape, params.as_slice()) {
        (Shape::None, []) => true,
        (Shape::Byte, [p]) => scalar(p) && matches!(p.kind, PropKind::Byte { .. }),
        (Shape::Object, [p]) => {
            scalar(p) && matches!(p.kind, PropKind::Object { class } if vm.space.classes.is_child_of(actor_class, class))
        }
        _ => false,
    };
    if !fits {
        return Err(script(format!("{class}.{name} is declared with other parameters than the rule calls")));
    }
    Ok(f)
}

/// What the commit does, every check passed.
struct Plan {
    actor: ObjRef,
    mode: u8,
    /// The volume whose `PhysicsChangedFor` the commit calls (validated in preflight).
    volume: Option<ObjRef>,
    /// The `WillowPawn` events to fire, in order: name, parameter shape and arguments (validated in preflight; the
    /// handler is resolved again at dispatch, review r2 of #696).
    events: Vec<(&'static str, Shape, Vec<Value>)>,
    /// The base search passed by the stand-in (logged at the commit).
    stand_in: bool,
    /// `bForceFloorCheck` set before the mode (into `PHYS_Walking`, the `Pawn` layer).
    force_floor: bool,
    /// `Velocity` and `Acceleration` zeroed after the mode (into `PHYS_None` or `PHYS_Rotating`).
    zero: bool,
}

/// The preflight (module documentation): `Ok(None)` when nothing changes. Reads only.
fn preflight(vm: &Vm, actor: ObjRef, mode: u8) -> Result<Option<Plan>, VmError> {
    resident(vm, actor)?;
    let layers = layers(vm, actor)?;
    // Step 1, the AI layer, before `Physics` is read (review r1 of #696, item 4).
    if layers.ai {
        if mode == PHYS_NAV_MESH_WALKING {
            return Err(NotYet::not_read(NAV_MESH, RECORD).into());
        }
        if mode == PHYS_RIGID_BODY && !flag(vm, actor, "bTearOff")? && !authority(vm, actor)? {
            return Err(NotYet::not_read(RIGID_EVENT, RECORD).into());
        }
        if flag(vm, actor, "bHardAttach")? && mode != PHYS_INTERPOLATING {
            return Ok(None);
        }
    }
    // Step 2.
    let old = byte(vm, actor, "Physics")?;
    if mode == old {
        return Ok(None);
    }
    // Step 3: the base search, decided here (module documentation).
    let base_search = matches!(mode, PHYS_NONE | PHYS_WALKING | PHYS_ROTATING | PHYS_SPIDER);
    if base_search {
        search_base(vm, actor)?;
    }
    let force_floor = layers.pawn && mode == PHYS_WALKING && old != PHYS_WALKING;
    if force_floor {
        flag(vm, actor, "bForceFloorCheck")?;
    }
    let zero = matches!(mode, PHYS_NONE | PHYS_ROTATING);
    if zero {
        for f in ["Velocity", "Acceleration"] {
            if !matches!(field(vm, actor, f)?.0, PropKind::Struct(_)) {
                return Err(script(format!("{f} is not declared as a struct")));
            }
        }
    }
    if !base_search
        && mode != PHYS_INTERPOLATING
        && let Some(base) = object(vm, actor, "Base")?
    {
        resident(vm, base)?;
        return Err(NotYet::not_read(BASE_CLEAR, RECORD).into());
    }
    if (mode == PHYS_RIGID_BODY || old == PHYS_RIGID_BODY)
        && let Some(component) = object(vm, actor, "CollisionComponent")?
    {
        resident(vm, component)?;
        return Err(NotYet::layer_absent(RIGID_BODY, RIGID_LAYER, RECORD).into());
    }
    // Step 4, the events the commit will call.
    let volume = match object(vm, actor, "PhysicsVolume")? {
        Some(v) => {
            resident(vm, v)?;
            event(vm, v, "PhysicsChangedFor", Shape::Object, actor)?;
            Some(v)
        }
        None => None,
    };
    let mut events = Vec::new();
    if layers.willow {
        let walking = |m: u8| m == PHYS_WALKING || m == PHYS_NAV_MESH_WALKING;
        let old_arg = || vec![Value::Byte(old)];
        let cases: [(&str, &'static str, bool, Shape, Vec<Value>); 4] = [
            ("bNotifyStopFalling", "StoppedFalling", old == PHYS_FALLING && mode != PHYS_FALLING, Shape::None, vec![]),
            ("bNotifyBeginFalling", "BeganFalling", old != PHYS_FALLING && mode == PHYS_FALLING, Shape::Byte, old_arg()),
            ("bNotifyBeginWalking", "BeganWalking", !walking(old) && walking(mode), Shape::Byte, old_arg()),
            ("bNotifyStopWalking", "StoppedWalking", walking(old) && !walking(mode), Shape::None, vec![]),
        ];
        for (notify, name, transition, shape, args) in cases {
            if transition && flag(vm, actor, notify)? {
                event(vm, actor, name, shape, actor)?;
                events.push((name, shape, args));
            }
        }
    }
    Ok(Some(Plan { actor, mode, volume, events, stand_in: base_search, force_floor, zero }))
}

/// The base search's preflight (module documentation, step 3; NATIVE_FLOOR_TRACE_FLAGS.md section 6.1). Reads only.
fn search_base(vm: &Vm, actor: ObjRef) -> Result<(), VmError> {
    // 1. No collision service.
    let Some(model) = crate::placement::collision_model(vm)? else {
        return Err(NotYet::layer_absent(BASE_TRACE, BASE_TRACE_LAYER, RECORD).into());
    };
    // 3. The level part: the stand-in only under its harness setting and only for a service that holds no geometry
    // (the hub's ruling on review of #798, R1; the guard kept by review r1 of #809). Decided first.
    if !model.holds_no_geometry() || vm.service::<NoLevelBaseSearch>().is_none() {
        return Err(NotYet::not_read(BASE_TRACE_FLAGS, RECORD).into());
    }
    // 4. The actor part, over every live actor's primitives in the placement walk's order.
    let space = &vm.space;
    let primitive = space.classes.by_path("Engine.PrimitiveComponent");
    // A handle the store has not loaded, or a slotless stand-in, is an unloaded link (the touch walk's rule; review of
    // #816, item 2), so a chain never reads a missing slot.
    let unloaded = |o: ObjRef| crate::touch_trace::is_unloaded(vm, o);
    let actors = crate::actors::actors_of(vm)?;
    let list = actors.try_borrow().map_err(|_| VmError::ServiceBorrowed("Actors"))?.list().to_vec();
    // 2. The swept box, read and checked before the walk (review of #816, item 1): a source without a cylinder, or a
    // malformed extent, refuses even with no candidate.
    let swept = swept_box(vm, actor)?;
    for walked in list {
        if crate::actors::is_destroyed(space, walked)? {
            continue;
        }
        resident(vm, walked)?;
        let class = space.heap.get(walked)?.class;
        let mut parts: Vec<ObjRef> = Vec::new();
        if space.prop(class, "CollisionComponent").is_some() {
            parts.extend(object(vm, walked, "CollisionComponent")?);
        }
        if space.prop(class, "Components").is_some() {
            for c in crate::actors::objects(space, walked, "Components")?.into_iter().flatten() {
                if !parts.contains(&c) {
                    parts.push(c);
                }
            }
        }
        for part in parts {
            resident(vm, part)?;
            match crate::trace_filter::part_class(space, space.heap.get(part)?.class, primitive) {
                crate::trace_filter::PartClass::Primitive => {}
                crate::trace_filter::PartClass::Other => continue,
                crate::trace_filter::PartClass::Uncertain(path) => {
                    if walked == actor {
                        continue;
                    }
                    return Err(NotYet::not_read(
                        format!("{BASE_TRACE}: {} {path}", crate::trace_filter::STAND_IN_PART),
                        RECORD_FLOOR,
                    )
                    .into());
                }
            }
            // In the walkers' order (NATIVE_FLOOR_TRACE_FLAGS.md section 3): the owner, the skip flag, the extent
            // flag, the per-component test, the bounds, the Owner chains; only rejecting.
            // A primitive with no owner is passed over (the walkers' first step; review of #816, item 3): a component
            // that declares `Owner` and holds None. One that does not declare it is the walked actor's.
            let part_class = space.heap.get(part)?.class;
            if space.prop(part_class, "Owner").is_some() && object(vm, part, "Owner")?.is_none() {
                continue;
            }
            let owner = crate::trace_filter::owner_of(space, part, walked)?;
            if owner == actor {
                continue;
            }
            if crate::actors::flag(space, part, crate::trace_filter::SKIP_FLAG_FIELD)? {
                continue;
            }
            if !crate::actors::flag(space, part, "BlockNonZeroExtent")? {
                continue;
            }
            if !crate::actors::flag(space, part, "CollideActors")? {
                continue;
            }
            resident(vm, owner)?;
            if !crate::actors::flag(space, owner, "bCollideActors")? {
                continue;
            }
            let s = swept;
            let (centre, extent) = crate::world::bounding_cylinder::entry_box(vm, owner, part)?;
            if !(centre.iter().chain(extent.iter()).all(|v| v.is_finite()) && extent.iter().all(|v| *v >= 0.0)) {
                return Err(NotYet::not_read(BASE_SWEPT_BOX, RECORD_FLOOR).into());
            }
            if s.disjoint(centre, extent) {
                continue;
            }
            match crate::trace_filter::owner_chains(space, actor, owner, &unloaded)? {
                crate::trace_filter::Chain::Excluded => continue,
                crate::trace_filter::Chain::Unloaded => {
                    return Err(NotYet::not_read(BASE_OWNER_LINK, RECORD_FLOOR).into());
                }
                crate::trace_filter::Chain::Outside => {}
            }
            return Err(NotYet::not_read(BASE_CANDIDATE, RECORD_FLOOR).into());
        }
    }
    // 5. No candidate left and the level clear: no floor.
    if object(vm, actor, "Base")?.is_some() {
        return Err(NotYet::not_read(BASE_SETTER, RECORD).into());
    }
    Ok(())
}

/// The floor trace's length (NATIVE_LINK34_AFTER_MIND.md section 2): 8 units straight down.
pub const FLOOR_TRACE_LENGTH: f32 = 8.0;
/// The record of the floor trace's flags, candidates and swept box.
pub const RECORD_FLOOR: &str = "docs/verification/NATIVE_FLOOR_TRACE_FLAGS.md";

/// The floor trace's swept box (NATIVE_FLOOR_TRACE_FLAGS.md section 2): from `start` down by [`FLOOR_TRACE_LENGTH`],
/// with the half-extent (`radius`, `radius`, `half_height`) of the actor's collision cylinder.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Swept {
    start: [f32; 3],
    radius: f32,
    half_height: f32,
}

impl Swept {
    /// Whether a box centred on `centre` with half-extent `extent` misses the swept box (NATIVE_FLOOR_TRACE_FLAGS.md
    /// section 5, the hub's ruling 2 on #809): apart on X or Y by more than the two half-widths, or wholly above or
    /// below it. Touching counts as meeting. Both boxes are well formed by now.
    fn disjoint(&self, centre: [f32; 3], extent: [f32; 3]) -> bool {
        let (s, c, e) = (self.start.map(f64::from), centre.map(f64::from), extent.map(f64::from));
        let (rs, hs) = (f64::from(self.radius), f64::from(self.half_height));
        (s[0] - c[0]).abs() > e[0] + rs
            || (s[1] - c[1]).abs() > e[1] + rs
            || s[2] - f64::from(FLOOR_TRACE_LENGTH) - hs > c[2] + e[2]
            || s[2] + hs < c[2] - e[2]
    }
}

/// The actor's swept box (NATIVE_FLOOR_TRACE_FLAGS.md section 6.1, item 2): its `CollisionComponent` a
/// `CylinderComponent` (or a class below it), else [`BASE_EXTENT`]; the start its `Location` plus that component's
/// `Translation` (the note's fit). `CollisionComponent`, `CollisionRadius`, `CollisionHeight`, `Location` and
/// `Translation` declared with their kinds, else `VmError::Script`. A start, radius or half-height that is not finite,
/// or a negative radius or half-height, is [`BASE_SWEPT_BOX`]. Reads only.
fn swept_box(vm: &Vm, actor: ObjRef) -> Result<Swept, VmError> {
    let space = &vm.space;
    let Some(component) = object(vm, actor, "CollisionComponent")? else {
        return Err(NotYet::not_read(BASE_EXTENT, RECORD_FLOOR).into());
    };
    resident(vm, component)?;
    let class = space.heap.get(component)?.class;
    let cylinder = space.classes.by_path("Engine.CylinderComponent").is_some_and(|c| space.classes.is_child_of(class, c));
    if !cylinder {
        return Err(NotYet::not_read(BASE_EXTENT, RECORD_FLOOR).into());
    }
    let vector = |o: ObjRef, name: &str| -> Result<[f32; 3], VmError> {
        let v = crate::actors::field(space, o, name)?;
        crate::world::bounding_cylinder::vector(space, o, &v, name)
    };
    let location = vector(actor, "Location")?;
    let translation = vector(component, "Translation")?;
    let radius = crate::actors::float(space, component, "CollisionRadius")?;
    let half_height = crate::actors::float(space, component, "CollisionHeight")?;
    let start = [0, 1, 2].map(|k| location[k] + translation[k]);
    if !(start.iter().all(|v| v.is_finite()) && radius.is_finite() && half_height.is_finite())
        || radius < 0.0
        || half_height < 0.0
    {
        return Err(NotYet::not_read(BASE_SWEPT_BOX, RECORD_FLOOR).into());
    }
    Ok(Swept { start, radius, half_height })
}

/// `v`, a vector struct, with every float field 0.
fn zeroed(v: &Value) -> Value {
    let mut out = v.clone();
    if let Some(s) = out.struct_mut() {
        for f in s.fields.iter_mut() {
            if matches!(f, Value::Float(_)) {
                *f = Value::Float(0.0);
            }
        }
    }
    out
}

/// The preflight alone (module documentation), for a caller that must decide `SetPhysics`' refusals before its own
/// first write (lane IMPL-SETDEFAULTPHYSICS-AND-MIND-RECALC: `WillowPawn.SetDefaultPhysics`). Reads only; `Ok(())`
/// also when nothing would change.
pub fn preflight_check(vm: &Vm, actor: ObjRef, mode: u8) -> Result<(), VmError> {
    preflight(vm, actor, mode).map(|_| ())
}

/// #518's counters.
fn marks(vm: &Vm) -> (u64, u64, u64) {
    (vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions())
}

/// The native (module documentation).
fn set_physics(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let actor = c.this.ok_or_else(|| script("called with no actor".to_string()))?;
    let mode = match c.arg(0)? {
        Value::Byte(b) => *b,
        Value::Int(i) => (*i & 0xFF) as u8,
        other => return Err(script(format!("the mode is {}, not a byte", other.kind_name()))),
    };
    let Some(plan) = preflight(c.vm, actor, mode)? else {
        return Ok(Value::None);
    };
    let entry = marks(c.vm);
    let result = commit(c, &plan);
    match result {
        Err(e) if marks(c.vm) != entry => Err(crate::pools::after_partial_writes(e)),
        other => other,
    }
}

fn commit(c: &mut NativeCall<'_>, plan: &Plan) -> Result<Value, VmError> {
    let class = c.vm.space.heap.get(plan.actor)?.class;
    if plan.stand_in {
        c.vm.log.push(BASE_STAND_IN.to_string());
    }
    if plan.force_floor {
        let p = c.vm.space.prop(class, "bForceFloorCheck").ok_or_else(|| script("no bForceFloorCheck".to_string()))?;
        c.vm.space.write(plan.actor, PropAddr::scalar(p), Value::Bool(true))?;
    }
    let p = c.vm.space.prop(class, "Physics").ok_or_else(|| script("the class declares no Physics".to_string()))?;
    c.vm.space.write(plan.actor, PropAddr::scalar(p), Value::Byte(plan.mode))?;
    if plan.zero {
        for f in ["Velocity", "Acceleration"] {
            let p = c.vm.space.prop(class, f).ok_or_else(|| script(format!("no {f}")))?;
            let v = zeroed(c.vm.space.read(plan.actor, PropAddr::scalar(p))?);
            c.vm.space.write(plan.actor, PropAddr::scalar(p), v)?;
        }
    }
    // Each handler is resolved again, load-free, right before its dispatch: an earlier callback may have changed the
    // receiver's state (review r2 of #696). A refusal here comes after the Physics write, so it is marked.
    if let Some(volume) = plan.volume {
        let f = event(c.vm, volume, "PhysicsChangedFor", Shape::Object, plan.actor)?;
        c.call_script(f, Some(volume), vec![Value::Object(Some(plan.actor))])?;
    }
    for (name, shape, args) in &plan.events {
        let f = event(c.vm, plan.actor, name, *shape, plan.actor)?;
        c.call_script(f, Some(plan.actor), args.clone())?;
    }
    Ok(Value::None)
}

#[cfg(test)]
#[path = "set_physics_tests.rs"]
mod tests;
