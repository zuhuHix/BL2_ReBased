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
//!    ([`BASE_TRACE`], [`BASE_TRACE_LAYER`]); a service with geometry, or a world where another live actor has a
//!    `CollisionComponent`, is `NotRead` ([`BASE_TRACE_FLAGS`]: the trace's flags are not decoded). **The stand-in**
//!    (the hub's ruling on #793, and on review of #798, R1): only with the Off-only harness setting
//!    [`NoLevelBaseSearch`] on (the slice's worlds turn it on, as they do `PhysXLevel` and `AtRestBodyPose`;
//!    [`enable_no_level_base_search`]), a service with no geometry ([`crate::placement::NoLevel`]) and no other actor
//!    with collision gives no floor; with `Base` None that changes nothing (logged as a harness input at the commit),
//!    with a `Base` set it is `NotRead` ([`BASE_SETTER`]) until the base setter is read. Without the setting the
//!    `NoLevel` case keeps [`BASE_TRACE_FLAGS`]' `NotRead`. The actor's own `CollisionComponent` is skipped (R2,
//!    UNVERIFIED: the common convention that an engine trace ignores its source). All decided here, before the mode
//!    write. Then into any other mode but `PHYS_Interpolating`, with a `Base` set, is `NotRead`
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
     geometry and no other actor with collision; UNVERIFIED stand-in, the hub's ruling on #793)";
/// The set-up line of [`enable_no_level_base_search`].
pub const BASE_STAND_IN_INPUT: &str = "harness input: the world turns on SetPhysics' no-floor base search for a world      with no level geometry (the hub's ruling on review of #798, R1; UNVERIFIED)";
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

/// The base search's preflight (module documentation, step 3). Reads only.
fn search_base(vm: &Vm, actor: ObjRef) -> Result<(), VmError> {
    let Some(model) = crate::placement::collision_model(vm)? else {
        return Err(NotYet::layer_absent(BASE_TRACE, BASE_TRACE_LAYER, RECORD).into());
    };
    // The stand-in only under its harness setting (the hub's ruling on review of #798, R1).
    if !model.holds_no_geometry() || vm.service::<NoLevelBaseSearch>().is_none() {
        return Err(NotYet::not_read(BASE_TRACE_FLAGS, RECORD).into());
    }
    let actors = crate::actors::actors_of(vm)?;
    let list = actors.try_borrow().map_err(|_| VmError::ServiceBorrowed("Actors"))?.list().to_vec();
    // The actor's own collision is skipped: UNVERIFIED, the common convention that an engine trace ignores its source
    // (the hub's ruling on review of #798, R2).
    for other in list.into_iter().filter(|o| *o != actor) {
        let Ok(x) = vm.space.heap.get(other) else { continue };
        if let Some(p) = vm.space.prop(x.class, "CollisionComponent")
            && matches!(vm.space.read(other, PropAddr::scalar(p))?, Value::Object(Some(_)))
        {
            return Err(NotYet::not_read(BASE_TRACE_FLAGS, RECORD).into());
        }
    }
    if object(vm, actor, "Base")?.is_some() {
        return Err(NotYet::not_read(BASE_SETTER, RECORD).into());
    }
    Ok(())
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
