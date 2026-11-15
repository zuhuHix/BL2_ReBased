//! Owner: lane W5-WORLD. Actors: the level's actor list ([`Actors`], a VM service), `Actor.Spawn`, `Actor.Destroy`,
//! the engine's `SetOwner`, script events, and the walks behind `Actor.AllActors` and `Actor.DynamicActors`.
//! AI-assisted (Claude Code).
//!
//! Every rule follows NATIVE_ENGINE_CORE.md ("Spawning and lifetime", "Iterators"), read from the executable and
//! **UNVERIFIED in the running game**. Where the record leaves a step unread, or the step needs a layer this world
//! does not have (collision, physics volumes, Kismet, the state frame), the call is `NotYet` naming it, checked
//! before anything is changed where that can be known in advance.
//!
//! **One level.** The world holds the persistent level only: [`Actors::place_world_info`] puts the `WorldInfo` at
//! index 0, [`Actors::place`] adds map actors before play begins, [`spawn`] appends. Streaming levels (whose walk
//! starts at index 1, and which can be flagged unusable) are not modelled.
//!
//! **Script events** ([`send_event`]) call the function the object sees by name in its state, then its class chain
//! (`Vm::find_method`), with the engine's zeroed parameters filled in. The probe mask (`Disable('Tick')` and the
//! like) belongs to the state frame of lane W5-STATES; until it exists every probe event is sent when a function is
//! found, which is the record's rule for an object whose class or state defines the function and nothing disabled it.
//!
//! **Fields.** The rules read and write the `Engine.Actor` fields the record names (`bStatic`, `Role`, `Owner`, ...)
//! by name. An actor class that does not declare one is a typed error, never a default.

use std::cell::RefCell;
use std::rc::Rc;

use ow_core::{FixedHashSet, NotYet};
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ClassId, FuncId, ObjRef, PropAddr, PropId};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{CallResult, NativeCall, Vm};
use ow_vm::error::VmError;
use ow_vm::natives_core::ActorWalk;

/// The record these rules come from.
pub const RECORD: &str = "NATIVE_ENGINE_CORE.md (Spawning and lifetime, Iterators)";

/// `ROLE_Authority` (NATIVE_ENGINE_CORE.md, "Conventions": the value is 3).
pub const ROLE_AUTHORITY: u8 = 3;
/// `WorldInfo.NetMode` of a network client (NATIVE_ENGINE_CORE.md, "World time fields": the value is 3).
pub const NET_MODE_CLIENT: u8 = 3;

// ---------------------------------------------------------------------------------------------------------------
// Fields
// ---------------------------------------------------------------------------------------------------------------

fn rule_error(rule: &str, message: String) -> VmError {
    VmError::Script {
        function: rule.to_string(),
        message,
    }
}

/// The property `name` of `o`'s class, or a typed error naming the class.
pub(crate) fn prop_of(space: &ObjectSpace, o: ObjRef, name: &str) -> Result<PropId, VmError> {
    let class = space.heap.get(o)?.class;
    space.prop(class, name).ok_or_else(|| {
        rule_error(
            name,
            format!(
                "{} declares no {name} ({RECORD})",
                ow_attr::eval::class_path(space, o)
            ),
        )
    })
}

/// The value of property `name` of `o` (element 0).
pub(crate) fn field(space: &ObjectSpace, o: ObjRef, name: &str) -> Result<Value, VmError> {
    let p = prop_of(space, o, name)?;
    Ok(space.read(o, PropAddr::scalar(p))?.clone())
}

fn wrong_kind(space: &ObjectSpace, o: ObjRef, name: &str, wanted: &str, v: &Value) -> VmError {
    rule_error(
        name,
        format!(
            "{}.{name} holds {}, expected {wanted}",
            ow_attr::eval::class_path(space, o),
            v.kind_name()
        ),
    )
}

pub(crate) fn flag(space: &ObjectSpace, o: ObjRef, name: &str) -> Result<bool, VmError> {
    match field(space, o, name)? {
        Value::Bool(b) => Ok(b),
        other => Err(wrong_kind(space, o, name, "a bool", &other)),
    }
}

pub(crate) fn byte(space: &ObjectSpace, o: ObjRef, name: &str) -> Result<u8, VmError> {
    match field(space, o, name)? {
        Value::Byte(b) => Ok(b),
        other => Err(wrong_kind(space, o, name, "a byte", &other)),
    }
}

pub(crate) fn float(space: &ObjectSpace, o: ObjRef, name: &str) -> Result<f32, VmError> {
    match field(space, o, name)? {
        Value::Float(f) => Ok(f),
        other => Err(wrong_kind(space, o, name, "a float", &other)),
    }
}

pub(crate) fn object(
    space: &ObjectSpace,
    o: ObjRef,
    name: &str,
) -> Result<Option<ObjRef>, VmError> {
    match field(space, o, name)? {
        Value::Object(r) => Ok(r),
        other => Err(wrong_kind(space, o, name, "an object", &other)),
    }
}

pub(crate) fn objects(
    space: &ObjectSpace,
    o: ObjRef,
    name: &str,
) -> Result<Vec<Option<ObjRef>>, VmError> {
    let v = field(space, o, name)?;
    let Some(items) = v.elements() else {
        return Err(wrong_kind(space, o, name, "an array", &v));
    };
    items
        .iter()
        .map(|x| match x {
            Value::Object(r) => Ok(*r),
            other => Err(wrong_kind(space, o, name, "an array of objects", other)),
        })
        .collect()
}

pub(crate) fn set_field(
    space: &mut ObjectSpace,
    o: ObjRef,
    name: &str,
    v: Value,
) -> Result<(), VmError> {
    let p = prop_of(space, o, name)?;
    space.write(o, PropAddr::scalar(p), v)?;
    Ok(())
}

/// The name of `o`'s `Physics` value in its enum (`PHYS_None`, `PHYS_Rotating`, ...), read from the property's
/// enum so no enum value is assumed.
pub(crate) fn physics(space: &ObjectSpace, o: ObjRef) -> Result<String, VmError> {
    let p = prop_of(space, o, "Physics")?;
    let b = byte(space, o, "Physics")?;
    let e = match space.classes.decl(p).map(|d| &d.kind) {
        Some(PropKind::Byte { enum_: Some(e) }) => *e,
        _ => {
            return Err(rule_error(
                "Physics",
                format!(
                    "{}.Physics names no enum",
                    ow_attr::eval::class_path(space, o)
                ),
            ));
        }
    };
    let values = space
        .classes
        .enum_layout(e)
        .map(|l| l.values())
        .unwrap_or(&[]);
    match values.get(usize::from(b)) {
        Some(n) => Ok(space.names.text(*n).into_owned()),
        None => Err(rule_error(
            "Physics",
            format!(
                "{}.Physics is {b}, outside its enum",
                ow_attr::eval::class_path(space, o)
            ),
        )),
    }
}

/// Whether `o` is destroyed: freed, marked destroyed in the heap (pending kill), or `bDeleteMe` set on a class that
/// declares it (NATIVE_ENGINE_CORE.md, "Conventions": pending kill).
pub fn is_destroyed(space: &ObjectSpace, o: ObjRef) -> Result<bool, VmError> {
    if !space.heap.contains(o) {
        return Ok(true);
    }
    let x = space.heap.get(o)?;
    if x.destroyed {
        return Ok(true);
    }
    match space.prop(x.class, "bDeleteMe") {
        Some(_) => flag(space, o, "bDeleteMe"),
        None => Ok(false),
    }
}

/// Whether a class named `name` is `class` or one of its super classes.
fn class_named_in_chain(space: &ObjectSpace, class: ClassId, name: &str) -> bool {
    let mut cursor = Some(class);
    let mut steps = 0usize;
    while let Some(c) = cursor {
        let Some(layout) = space.classes.get_layout(c) else {
            return false;
        };
        if layout.path().name().eq_ignore_ascii_case(name) {
            return true;
        }
        steps += 1;
        if steps > space.classes.len() {
            return false; // a chain longer than the table loops; the class table refuses one, so this is a guard
        }
        cursor = layout.super_class();
    }
    false
}

// ---------------------------------------------------------------------------------------------------------------
// The level's actor list
// ---------------------------------------------------------------------------------------------------------------

/// The persistent level's actor list and play state (module documentation). A VM service.
#[derive(Debug)]
pub struct Actors {
    actor_class: ClassId,
    list: Vec<ObjRef>,
    first_dynamic: usize,
    begun_play: bool,
    world_info: Option<ObjRef>,
    destroying: Vec<ObjRef>,
}

impl Actors {
    /// An empty level whose actors are instances of `actor_class` (`Engine.Actor`).
    pub fn new(actor_class: ClassId) -> Self {
        Actors {
            actor_class,
            list: Vec::new(),
            first_dynamic: 0,
            begun_play: false,
            world_info: None,
            destroying: Vec::new(),
        }
    }

    pub fn actor_class(&self) -> ClassId {
        self.actor_class
    }

    pub fn world_info(&self) -> Option<ObjRef> {
        self.world_info
    }

    pub fn begun_play(&self) -> bool {
        self.begun_play
    }

    /// The level's actor list in order: index 0 is the `WorldInfo`, then the map's actors, then spawned ones.
    pub fn list(&self) -> &[ObjRef] {
        &self.list
    }

    pub fn first_dynamic(&self) -> usize {
        self.first_dynamic
    }

    fn refuse(what: &str) -> VmError {
        rule_error("Actors", what.to_string())
    }

    /// Puts the level's `WorldInfo` at index 0 (the map load does this first).
    pub fn place_world_info(&mut self, world_info: ObjRef) -> Result<(), VmError> {
        if !self.list.is_empty() {
            return Err(Self::refuse(
                "the WorldInfo must be the level's first actor",
            ));
        }
        self.list.push(world_info);
        self.world_info = Some(world_info);
        Ok(())
    }

    /// Adds a map actor (the map load, before play begins; after that actors come from [`spawn`]).
    pub fn place(&mut self, actor: ObjRef) -> Result<(), VmError> {
        if self.world_info.is_none() {
            return Err(Self::refuse("place the WorldInfo first"));
        }
        if self.begun_play {
            return Err(Self::refuse("map actors are placed before play begins"));
        }
        self.list.push(actor);
        Ok(())
    }

    /// The level's first-dynamic-actor index, where `DynamicActors` starts. The map load states it: how the game
    /// orders a level's list and computes the index is not in the record (UNVERIFIED); 0 until set.
    pub fn set_first_dynamic(&mut self, index: usize) -> Result<(), VmError> {
        if index > self.list.len() {
            return Err(Self::refuse(
                "the first dynamic index is past the actor list",
            ));
        }
        self.first_dynamic = index;
        Ok(())
    }

    /// Play begins: from now on [`spawn`] runs the begin-play events and refuses static classes. The begin-play pass
    /// the game runs over actors that existed before is not read (NATIVE_ENGINE_CORE.md, "Not read yet"); none is run.
    pub fn begin_play(&mut self) {
        self.begun_play = true;
    }

    /// The candidates of `AllActors` (`ActorWalk::All`: from index 0) or `DynamicActors` (from the first dynamic
    /// index, skipping `bStatic` actors), in list order, destroyed ones skipped. Hidden actors are not skipped.
    pub fn walk(&self, space: &ObjectSpace, which: ActorWalk) -> Result<Vec<ObjRef>, VmError> {
        let (start, dynamic) = match which {
            ActorWalk::All => (0, false),
            ActorWalk::Dynamic => (self.first_dynamic, true),
            other => {
                return Err(NotYet::not_ported(format!("the actor walk {other:?}"), RECORD).into());
            }
        };
        let mut out = Vec::new();
        for &o in self.list.get(start..).unwrap_or(&[]) {
            if is_destroyed(space, o)? || (dynamic && flag(space, o, "bStatic")?) {
                continue;
            }
            out.push(o);
        }
        Ok(out)
    }

    fn remove(&mut self, actor: ObjRef) {
        if let Some(i) = self.list.iter().position(|&o| o == actor) {
            self.list.remove(i);
            if i < self.first_dynamic {
                self.first_dynamic -= 1;
            }
        }
    }
}

/// The world's [`Actors`] service, or `NotYet` when this VM has no world.
pub fn actors_of(vm: &Vm) -> Result<Rc<RefCell<Actors>>, VmError> {
    vm.service::<Actors>().ok_or_else(|| {
        NotYet::layer_absent("the level's actor list", "ow-world (World::new)", RECORD).into()
    })
}

fn borrow_err(_: std::cell::BorrowError) -> VmError {
    VmError::ServiceBorrowed("Actors")
}

fn borrow_mut_err(_: std::cell::BorrowMutError) -> VmError {
    VmError::ServiceBorrowed("Actors")
}

// ---------------------------------------------------------------------------------------------------------------
// Script events
// ---------------------------------------------------------------------------------------------------------------

/// Calls `f` on `this` with `given` first and every further parameter at its kind's zero, all supplied (the engine
/// passes a zeroed parameter block, so optional defaults do not run).
pub(crate) fn call_with_zeros(
    vm: &mut Vm,
    f: FuncId,
    this: Option<ObjRef>,
    mut given: Vec<Value>,
) -> Result<CallResult, VmError> {
    let kinds: Vec<PropKind> = vm
        .function(f)
        .map(|d| d.params.iter().map(|p| p.kind.clone()).collect())
        .unwrap_or_default();
    for k in kinds.iter().skip(given.len()) {
        given.push(vm.space.classes.zero_value(k));
    }
    vm.call(f, this, given)
}

/// Sends the script event `name` to `o` (module documentation): `Ok(false)` when `o` sees no function of that
/// name, which is silent.
pub fn send_event(vm: &mut Vm, o: ObjRef, name: &str, args: Vec<Value>) -> Result<bool, VmError> {
    let (class, state) = {
        let x = vm.space.heap.get(o)?;
        (x.class, x.state)
    };
    let Some(f) = vm.find_method(class, name, state)? else {
        return Ok(false);
    };
    call_with_zeros(vm, f, Some(o), args)?;
    Ok(true)
}

// ---------------------------------------------------------------------------------------------------------------
// SetOwner
// ---------------------------------------------------------------------------------------------------------------

/// The engine's `SetOwner` (NATIVE_ENGINE_CORE.md, "SetOwner", inferred there): nothing when the owner does not
/// change or `actor` is destroyed; an owner whose own owner chain reaches `actor` is refused (nothing changes);
/// otherwise `LostChild(actor)` to the old owner (stopping if script changed the owner meanwhile), the field set,
/// then `GainedChild(actor)` to the new owner.
pub fn set_owner(vm: &mut Vm, actor: ObjRef, owner: Option<ObjRef>) -> Result<(), VmError> {
    let old = object(&vm.space, actor, "Owner")?;
    if old == owner || is_destroyed(&vm.space, actor)? {
        return Ok(());
    }
    if let Some(new) = owner {
        let mut seen = FixedHashSet::default();
        let mut cursor = Some(new);
        while let Some(c) = cursor {
            if c == actor {
                return Ok(()); // an ownership loop: refused
            }
            if !seen.insert(c) {
                return Err(rule_error(
                    "SetOwner",
                    "the new owner's owner chain loops without reaching the actor".to_string(),
                ));
            }
            cursor = object(&vm.space, c, "Owner")?;
        }
    }
    if let Some(o) = old {
        send_event(vm, o, "LostChild", vec![Value::Object(Some(actor))])?;
        if object(&vm.space, actor, "Owner")? != old {
            return Ok(());
        }
    }
    set_field(&mut vm.space, actor, "Owner", Value::Object(owner))?;
    if let Some(n) = owner {
        send_event(vm, n, "GainedChild", vec![Value::Object(Some(actor))])?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Spawn
// ---------------------------------------------------------------------------------------------------------------

/// The arguments of `Actor.Spawn` that the rule uses (`SpawnTag` is read by the native and then not used:
/// NATIVE_ENGINE_CORE.md, UNVERIFIED).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct SpawnRequest {
    pub class: Option<ClassId>,
    pub owner: Option<ObjRef>,
    /// `None`: the spawner's `Location`.
    pub location: Option<Value>,
    /// `None`: the spawner's `Rotation`.
    pub rotation: Option<Value>,
    pub template: Option<ObjRef>,
    pub no_collision_fail: bool,
}

impl SpawnRequest {
    pub fn new(class: Option<ClassId>) -> Self {
        SpawnRequest {
            class,
            owner: None,
            location: None,
            rotation: None,
            template: None,
            no_collision_fail: false,
        }
    }

    pub fn with_owner(mut self, owner: Option<ObjRef>) -> Self {
        self.owner = owner;
        self
    }

    pub fn with_location(mut self, v: Value) -> Self {
        self.location = Some(v);
        self
    }

    pub fn with_rotation(mut self, v: Value) -> Self {
        self.rotation = Some(v);
        self
    }

    pub fn with_template(mut self, t: Option<ObjRef>) -> Self {
        self.template = t;
        self
    }

    pub fn with_no_collision_fail(mut self, b: bool) -> Self {
        self.no_collision_fail = b;
        self
    }
}

fn class_default(vm: &mut Vm, class: ClassId) -> Result<ObjRef, VmError> {
    let path = vm
        .space
        .classes
        .get_layout(class)
        .map(|l| l.path().as_str().to_string())
        .ok_or_else(|| rule_error("Spawn", format!("unknown class {class:?}")))?;
    let made = vm.defs.class_default(&mut vm.space, &path);
    vm.log.extend(vm.defs.take_log());
    Ok(made?)
}

fn collision(what: &'static str) -> VmError {
    NotYet::layer_absent(
        what,
        "collision (the world's collision hash is not modelled)",
        RECORD,
    )
    .into()
}

/// `Actor.Spawn` by `spawner` (NATIVE_ENGINE_CORE.md, "Actor.Spawn" and the spawn routine), in the record's order:
///
/// 1. `None` for: no class; a class that is not an actor class; after play began, a class whose default object is
///    `bStatic` or `bNoDelete`; a template of another class.
/// 2. `NotYet`, before anything is made, for what needs collision, judged on the class defaults: the placement spot
///    search (`bCollideWorld`, or `bCollideWhenPlacing` outside a client, unless `bNoCollisionFail`); after play
///    began, the encroachment check or relocation of an actor that collides with actors (`bCollideActors`), and the
///    base search of step 4. A template other than the class default object is `NotYet` too (how its instanced
///    subobjects are copied is not read). An actor that does not collide with actors is taken to pass the
///    encroachment check (UNVERIFIED: the check's internals are not read). The pre-checks are conservative: they
///    refuse with `NotYet` even when the begin-play events would have made the check unnecessary.
/// 3. The actor is made from the class defaults and appended to the level list; `CreationTime` = the world's
///    `TimeSeconds`, `WorldInfo`, `Location` and `Rotation` (the spawner's when omitted), the owner through
///    [`set_owner`] (`GainedChild`), then `Instigator` = **the spawner's** `Instigator`.
/// 4. After play began: `PreBeginPlay`, `PostBeginPlay`, then `SetInitialState`; a base search when the actor has
///    no `Base`, collides with the world, `bShouldBaseAtStartup` and `Physics` is `PHYS_None` or `PHYS_Rotating`
///    (`NotYet`: collision). Before play began none of these run (the game's later begin-play pass is not read).
///
/// An event that destroys the actor makes the spawn return `None`. The begin-play events can change the fields the
/// collision refusals read, so `bCollideActors` is checked again after `PreBeginPlay` and the base search after
/// `SetInitialState`. A `NotYet` from one of those post-event checks leaves the actor made, in the level list and
/// past the events that ran; the run is meant to stop there, as at any `NotYet`.
///
/// `bTicked` (step 5, set from the level's tick state) is not written; its effect is the tick's rule that an actor
/// spawned during a tick is first ticked in the next one ([`crate::world::World::tick`], UNVERIFIED). The unread
/// alternative is step 11's list of newly spawned actors, which in public UE3 ticks them in the same frame.
///
/// Not modelled, each a step the record leaves unread or a layer this world lacks: `PhysicsVolume`, the
/// physics-body and state-frame set-up of step 6, the zone and tick-group changes after `PreBeginPlay`, the
/// per-component call of step 8.
///
/// Not checked: the abstract and "not instantiable" class-flag refusals. The class table holds no class flags and
/// the record does not identify the bits (open item, decision 2026-11-15-26).
pub fn spawn(vm: &mut Vm, spawner: ObjRef, req: &SpawnRequest) -> Result<Option<ObjRef>, VmError> {
    let Some(class) = req.class else {
        return Ok(None);
    };
    let actors = actors_of(vm)?;
    let (actor_class, begun, world_info) = {
        let a = actors.try_borrow().map_err(borrow_err)?;
        (a.actor_class, a.begun_play, a.world_info)
    };
    if !vm.space.classes.is_child_of(class, actor_class) {
        return Ok(None);
    }
    let Some(world_info) = world_info else {
        return Err(NotYet::layer_absent(
            "Spawn in a level without its WorldInfo actor",
            "the map load (Actors::place_world_info)",
            RECORD,
        )
        .into());
    };
    let defaults = class_default(vm, class)?;
    let space = &vm.space;
    if begun && (flag(space, defaults, "bStatic")? || flag(space, defaults, "bNoDelete")?) {
        return Ok(None);
    }
    if let Some(t) = req.template {
        if space.heap.get(t)?.class != class {
            return Ok(None);
        }
        if t != defaults {
            return Err(NotYet::not_read(
                "Spawn with an ActorTemplate other than the class default object (how the template's instanced \
                 subobjects are copied)",
                RECORD,
            )
            .into());
        }
    }
    let client = byte(space, world_info, "NetMode")? == NET_MODE_CLIENT;
    if !req.no_collision_fail
        && (flag(space, defaults, "bCollideWorld")?
            || (flag(space, defaults, "bCollideWhenPlacing")? && !client))
    {
        return Err(collision("the spawn placement spot search"));
    }
    if begun && flag(space, defaults, "bCollideActors")? {
        return Err(collision(
            "the post-spawn encroachment check (or relocation) of an actor that collides with actors",
        ));
    }
    if begun && needs_base_search(space, defaults)? {
        return Err(collision("the base search after PostBeginPlay"));
    }
    let time = float(space, world_info, "TimeSeconds")?;
    let location = match &req.location {
        Some(v) => v.clone(),
        None => field(space, spawner, "Location")?,
    };
    let rotation = match &req.rotation {
        Some(v) => v.clone(),
        None => field(space, spawner, "Rotation")?,
    };
    let instigator = object(space, spawner, "Instigator")?;

    let actor = ow_vm::interp::instantiate_from_defaults(vm, class)?;
    actors
        .try_borrow_mut()
        .map_err(borrow_mut_err)?
        .list
        .push(actor);
    set_field(&mut vm.space, actor, "CreationTime", Value::Float(time))?;
    set_field(
        &mut vm.space,
        actor,
        "WorldInfo",
        Value::Object(Some(world_info)),
    )?;
    set_field(&mut vm.space, actor, "Location", location)?;
    set_field(&mut vm.space, actor, "Rotation", rotation)?;
    set_owner(vm, actor, req.owner)?;
    if is_destroyed(&vm.space, actor)? {
        return Ok(None);
    }
    set_field(
        &mut vm.space,
        actor,
        "Instigator",
        Value::Object(instigator),
    )?;
    if !begun {
        return Ok(Some(actor));
    }
    send_event(vm, actor, "PreBeginPlay", Vec::new())?;
    if is_destroyed(&vm.space, actor)? {
        return Ok(None);
    }
    if flag(&vm.space, actor, "bCollideActors")? {
        return Err(collision(
            "the post-spawn encroachment check of an actor that PreBeginPlay made collide with actors",
        ));
    }
    send_event(vm, actor, "PostBeginPlay", Vec::new())?;
    if is_destroyed(&vm.space, actor)? {
        return Ok(None);
    }
    send_event(vm, actor, "SetInitialState", Vec::new())?;
    if is_destroyed(&vm.space, actor)? {
        return Ok(None);
    }
    if needs_base_search(&vm.space, actor)? {
        return Err(collision(
            "the base search of an actor that its begin-play events made need one",
        ));
    }
    Ok(Some(actor))
}

/// Step 10's base search condition: no `Base`, collides with the world, `bShouldBaseAtStartup`, and `Physics`
/// `PHYS_None` or `PHYS_Rotating`.
fn needs_base_search(space: &ObjectSpace, o: ObjRef) -> Result<bool, VmError> {
    Ok(object(space, o, "Base")?.is_none()
        && flag(space, o, "bCollideWorld")?
        && flag(space, o, "bShouldBaseAtStartup")?
        && matches!(physics(space, o)?.as_str(), "PHYS_None" | "PHYS_Rotating"))
}

// ---------------------------------------------------------------------------------------------------------------
// Destroy
// ---------------------------------------------------------------------------------------------------------------

/// `Actor.Destroy` (NATIVE_ENGINE_CORE.md, "Actor.Destroy" and the destroy routine):
///
/// - `true` for an actor already destroyed; `false` for a `bStatic` or `bNoDelete` actor, and for one whose `Role`
///   is below authority and that is not `bNetTemporary`.
/// - `NotYet`, before anything changes, for the parts this world cannot run: a `Controller` (the controller and
///   player case, not read), an actor with Kismet events in `GeneratedEvents` (the destroyed-event class is not
///   confirmed: W5-KISMET), an actor in a state (`EndState` and the state frame: W5-STATES), an actor with a `Base`,
///   with attached actors based on it, or touching others (the unbase and untouch routines are physics natives, not
///   read), and a `Destroy` of an actor whose own destroy routine is running.
/// - Otherwise: `bPendingDelete`, script `Destroyed()`, `SetOwner(None)` (`LostChild` to the old owner), then the
///   actor leaves the level list, `bDeleteMe` is set, the heap marks it destroyed and its timer entries are dropped.
///   It is not freed here.
pub fn destroy(vm: &mut Vm, actor: ObjRef) -> Result<bool, VmError> {
    let space = &vm.space;
    if is_destroyed(space, actor)? {
        return Ok(true);
    }
    if flag(space, actor, "bStatic")? || flag(space, actor, "bNoDelete")? {
        return Ok(false);
    }
    if byte(space, actor, "Role")? < ROLE_AUTHORITY && !flag(space, actor, "bNetTemporary")? {
        return Ok(false);
    }
    let actors = actors_of(vm)?;
    if actors
        .try_borrow()
        .map_err(borrow_err)?
        .destroying
        .contains(&actor)
    {
        return Err(NotYet::not_read(
            "Destroy of an actor whose destroy routine is running",
            RECORD,
        )
        .into());
    }
    let x = space.heap.get(actor)?;
    if class_named_in_chain(space, x.class, "Controller") {
        return Err(
            NotYet::not_read("the destroy routine's controller and player case", RECORD).into(),
        );
    }
    if x.state.is_some() {
        return Err(NotYet::layer_absent(
            "EndState when an actor in a state is destroyed",
            "ow-vm states (lane W5-STATES)",
            RECORD,
        )
        .into());
    }
    if objects(space, actor, "GeneratedEvents")?
        .iter()
        .any(Option::is_some)
    {
        return Err(NotYet::layer_absent(
            "the Kismet destroyed events of a destroyed actor (event class not confirmed)",
            "ow-mission kismet (lane W5-KISMET)",
            RECORD,
        )
        .into());
    }
    if object(space, actor, "Base")?.is_some() {
        return Err(NotYet::not_read("unbasing a destroyed actor (SetBase)", RECORD).into());
    }
    for a in objects(space, actor, "Attached")?.into_iter().flatten() {
        if !is_destroyed(space, a)? && object(space, a, "Base")? == Some(actor) {
            return Err(NotYet::not_read(
                "unbasing the actors attached to a destroyed actor",
                RECORD,
            )
            .into());
        }
    }
    for t in objects(space, actor, "Touching")?.into_iter().flatten() {
        if !is_destroyed(space, t)? {
            return Err(NotYet::not_read(
                "the untouch routine for a destroyed actor's neighbours",
                RECORD,
            )
            .into());
        }
    }

    set_field(&mut vm.space, actor, "bPendingDelete", Value::Bool(true))?;
    actors
        .try_borrow_mut()
        .map_err(borrow_mut_err)?
        .destroying
        .push(actor);
    let events = (|| {
        send_event(vm, actor, "Destroyed", Vec::new())?;
        set_owner(vm, actor, None)
    })();
    actors
        .try_borrow_mut()
        .map_err(borrow_mut_err)?
        .destroying
        .retain(|&o| o != actor);
    events?;
    actors
        .try_borrow_mut()
        .map_err(borrow_mut_err)?
        .remove(actor);
    set_field(&mut vm.space, actor, "bDeleteMe", Value::Bool(true))?;
    vm.space.heap.destroy(actor)?;
    // The destroyed actor's timer entries go with it (its update pass never runs again).
    if let Some(timers) = vm.service::<crate::timers::Timers>() {
        timers
            .try_borrow_mut()
            .map_err(|_| VmError::ServiceBorrowed("Timers"))?
            .forget(actor);
    }
    Ok(true)
}

// ---------------------------------------------------------------------------------------------------------------
// Natives
// ---------------------------------------------------------------------------------------------------------------

fn this(c: &NativeCall<'_>, native: &str) -> Result<ObjRef, VmError> {
    c.this
        .ok_or_else(|| rule_error(native, "called without an actor".to_string()))
}

fn bad(native: &str, index: usize, reason: &'static str) -> VmError {
    VmError::BadArgument {
        function: native.to_string(),
        index,
        reason,
    }
}

/// An object argument; omitted or `Value::None` is None.
pub(crate) fn arg_object(
    c: &NativeCall<'_>,
    native: &str,
    i: usize,
) -> Result<Option<ObjRef>, VmError> {
    if !c.has(i) {
        return Ok(None);
    }
    match c.arg(i)? {
        Value::Object(o) => Ok(*o),
        Value::None => Ok(None),
        _ => Err(bad(native, i, "expected an object")),
    }
}

/// A bool argument; omitted or `Value::None` is false.
pub(crate) fn arg_bool(c: &NativeCall<'_>, native: &str, i: usize) -> Result<bool, VmError> {
    if !c.has(i) {
        return Ok(false);
    }
    match c.arg(i)? {
        Value::Bool(b) => Ok(*b),
        Value::None => Ok(false),
        _ => Err(bad(native, i, "expected a bool")),
    }
}

const SPAWN: &str = "Actor.Spawn(class,object,name,vector,rotator,object,bool)";
const DESTROY: &str = "Actor.Destroy()";

/// Registers `Actor.Spawn` and `Actor.Destroy`.
pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    vm.register_native(
        SPAWN,
        Rc::new(|c: &mut NativeCall<'_>| {
            let spawner = this(c, SPAWN)?;
            let class = if c.has(0) {
                match c.arg(0)? {
                    Value::Class(k) => *k,
                    Value::None => None,
                    _ => return Err(bad(SPAWN, 0, "expected a class")),
                }
            } else {
                None
            };
            let mut req = SpawnRequest::new(class)
                .with_owner(arg_object(c, SPAWN, 1)?)
                .with_template(arg_object(c, SPAWN, 5)?)
                .with_no_collision_fail(arg_bool(c, SPAWN, 6)?);
            if c.has(3) {
                req = req.with_location(c.arg(3)?.clone());
            }
            if c.has(4) {
                req = req.with_rotation(c.arg(4)?.clone());
            }
            Ok(Value::Object(spawn(c.vm, spawner, &req)?))
        }),
    )?;
    vm.register_native(
        DESTROY,
        Rc::new(|c: &mut NativeCall<'_>| {
            let actor = this(c, DESTROY)?;
            Ok(Value::Bool(destroy(c.vm, actor)?))
        }),
    )?;
    Ok(())
}
