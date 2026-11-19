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

use ow_core::{FixedHashSet, NotYet, ParityMode};
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ClassId, FuncId, ObjRef, PropAddr, PropId};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{CallResult, NativeCall, Vm};
use ow_vm::error::VmError;
use ow_vm::natives_core::ActorWalk;

mod instancing;
pub use instancing::{COMPONENT_CLASS, Plan, apply_plan, instance_components, plan_components};

/// The record these rules come from.
pub const RECORD: &str = "NATIVE_ENGINE_CORE.md (Spawning and lifetime, Iterators)";
/// The record of the destroy routine's controller and player case (lane FIX-ACTOR-DESTROY, decision 2026-11-16-79).
pub const RECORD_CONTROLLER: &str = "NATIVE_ACTOR_DESTROY.md (the controller and player case)";
/// The record of a controller's and a pawn's destroy, the kill's unlinking (lane IMPL-ACTOR-DESTROY-CONTROLLER,
/// decision 2026-11-19-92).
pub const RECORD_UNLINK: &str = "docs/verification/NATIVE_ACTOR_DESTROY_CONTROLLER.md";
/// The mark of a failure after state was written (the hub's ruling; the same text as the other crates' marks).
pub const PARTIAL_WRITES: &str = "stopped after partial writes";

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
// The world's controller and pawn lists (lane IMPL-ACTOR-DESTROY-CONTROLLER, decision 2026-11-19-92)
// ---------------------------------------------------------------------------------------------------------------

/// One of the world's two linked lists (NATIVE_ACTOR_DESTROY_CONTROLLER.md sections 2 to 4): its head on the
/// `WorldInfo`, and each member's link to the next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldList {
    /// `WorldInfo.ControllerList`, linked through `Controller.NextController`.
    Controllers,
    /// `WorldInfo.PawnList`, linked through `Pawn.NextPawn`.
    Pawns,
}

impl WorldList {
    pub fn head(self) -> &'static str {
        match self {
            WorldList::Controllers => "ControllerList",
            WorldList::Pawns => "PawnList",
        }
    }

    pub fn next(self) -> &'static str {
        match self {
            WorldList::Controllers => "NextController",
            WorldList::Pawns => "NextPawn",
        }
    }

    fn named(self) -> &'static str {
        match self {
            WorldList::Controllers => "the world's controller list",
            WorldList::Pawns => "the world's pawn list",
        }
    }

    /// The list `o` belongs in, by its class: a `Controller`, a `Pawn`, or neither.
    pub fn of(space: &ObjectSpace, o: ObjRef) -> Result<Option<WorldList>, VmError> {
        let class = space.heap.get(o)?.class;
        Ok(if class_named_in_chain(space, class, "Controller") {
            Some(WorldList::Controllers)
        } else if class_named_in_chain(space, class, "Pawn") {
            Some(WorldList::Pawns)
        } else {
            None
        })
    }
}

/// The `WorldInfo` that holds `list`: the world's, when its class declares the list's head. None: the world does not
/// hold that list (the port's lists are kept only on a `WorldInfo` that declares them).
pub fn list_holder(space: &ObjectSpace, actors: &Actors, list: WorldList) -> Result<Option<ObjRef>, VmError> {
    let Some(wi) = actors.world_info else {
        return Ok(None);
    };
    Ok(space.prop(space.heap.get(wi)?.class, list.head()).map(|_| wi))
}

/// The members of `list` in link order, from the holder's head. A link that loops, or that runs longer than the heap,
/// is bad data (a typed error).
pub fn list_members(space: &ObjectSpace, holder: ObjRef, list: WorldList) -> Result<Vec<ObjRef>, VmError> {
    let mut out = Vec::new();
    let mut cursor = object(space, holder, list.head())?;
    while let Some(o) = cursor {
        if out.contains(&o) || out.len() > space.heap.len() {
            return Err(rule_error(list.head(), format!("{} loops through {o:?}", list.head())));
        }
        out.push(o);
        cursor = object(space, o, list.next())?;
    }
    Ok(out)
}

/// Adds `o` at the head of `list` (section 4, "Adding": newest first). A pawn already in the list is first removed.
fn link_head(space: &mut ObjectSpace, holder: ObjRef, list: WorldList, o: ObjRef) -> Result<(), VmError> {
    if list == WorldList::Pawns && list_members(space, holder, list)?.contains(&o) {
        unlink(space, holder, list, o)?;
    }
    let head = object(space, holder, list.head())?;
    set_field(space, o, list.next(), Value::Object(head))?;
    set_field(space, holder, list.head(), Value::Object(Some(o)))
}

/// Unlinks `o` from `list`: the head, or its predecessor's link, takes `o`'s link. A member not in the list is left
/// alone. `o`'s own link is not changed here.
fn unlink(space: &mut ObjectSpace, holder: ObjRef, list: WorldList, o: ObjRef) -> Result<(), VmError> {
    let members = list_members(space, holder, list)?;
    let Some(at) = members.iter().position(|&m| m == o) else {
        return Ok(());
    };
    let next = object(space, o, list.next())?;
    match at.checked_sub(1).map(|i| members[i]) {
        None => set_field(space, holder, list.head(), Value::Object(next)),
        Some(prev) => set_field(space, prev, list.next(), Value::Object(next)),
    }
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
    /// The engine's local players' controllers, in player index order (`Actor.LocalPlayerControllers`).
    local_players: Vec<ObjRef>,
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
            local_players: Vec::new(),
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

    /// Adds `controller` as the next local player's controller (the engine's local player list, in player index
    /// order; a single-player world has one). Who logs a local player in, and when, is the engine's (not read); the
    /// caller that makes the player's controller adds it. A controller already listed is refused.
    pub fn add_local_player(&mut self, controller: ObjRef) -> Result<(), VmError> {
        if self.local_players.contains(&controller) {
            return Err(Self::refuse("the controller is already a local player"));
        }
        self.local_players.push(controller);
        Ok(())
    }

    /// The local players' controllers, in player index order.
    pub fn local_players(&self) -> &[ObjRef] {
        &self.local_players
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
    /// `ActorWalk::LocalPlayers`: the local players' controllers in player index order, destroyed ones skipped.
    pub fn walk(&self, space: &ObjectSpace, which: ActorWalk) -> Result<Vec<ObjRef>, VmError> {
        let (start, dynamic) = match which {
            ActorWalk::All => (0, false),
            ActorWalk::Dynamic => (self.first_dynamic, true),
            ActorWalk::LocalPlayers => {
                let mut out = Vec::new();
                for &o in &self.local_players {
                    if !is_destroyed(space, o)? {
                        out.push(o);
                    }
                }
                return Ok(out);
            }
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
///    base search of step 4. Under `ParityMode::Reference` a template other than the class default object is
///    `NotYet` too (the C++ stand-in does not copy a template's sub-objects). In Off, since decision 2026-11-19-44,
///    the actor is made from it, as public UE3's spawn copies its template's properties (UNVERIFIED in this game);
///    the collision pre-checks still read the class defaults (UNVERIFIED for an
///    archetype whose flags differ). An actor that does not collide with actors is taken to pass the
///    encroachment check (UNVERIFIED: the check's internals are not read). The pre-checks are conservative: they
///    refuse with `NotYet` even when the begin-play events would have made the check unnecessary.
///
///    **In `ParityMode::Off`** (lane IMPL-SPAWN-PLACEMENT, decision 2026-11-19-79, NATIVE_SPAWN_PLACEMENT.md,
///    UNVERIFIED): the spot search reads the template's flags (`crate::placement::search_runs`) and runs through the
///    world's collision service (`crate::placement::spot_search`): without one, `NotYet { LayerAbsent }`; for an
///    extent whose rule is not read (`crate::placement::extent`), and for a query the service does not support,
///    `NotYet { NotRead }`. The extent's component loads run just before the search
///    (`crate::placement::load_extent_components`, NATIVE_SPAWN_EXTENT.md section 5, step 5.3): once one ran, every
///    `NotYet` up to the actor's insertion is marked "stopped after partial writes: ". A failed search returns
///    `None` with nothing made, a successful one gives the location the actor is made at. There is no eager
///    `bCollideActors` refusal in Off: the encroachment check (or the relocation step for a `bNoCollisionFail` spawn)
///    is decided on the live actor after `PreBeginPlay` (`crate::placement::after_pre_begin_play`), terminal and
///    prefixed "stopped after partial writes" (the actor stays made).
/// 3. The actor is made from the class defaults (in `ParityMode::Off`, from the template: the class defaults or an
///    archetype of the class) and appended to the level list. In Off it gets its own copy of each component the
///    template owns ([`instance_components`]: public UE3 component instancing, UNVERIFIED in this game; decision
///    2026-11-19-44). Then `CreationTime` = the world's
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
/// In Off nothing is left behind by a refusal before the actor joins the level list (review r2 of #435): the
/// instancing's refusals and the fields written run before the actor is made, and a failure while making it frees
/// it and its copies. A `NotYet` of the begin-play steps after that leaves the actor made, as step 4 says.
///
/// **The commit boundary** (review r1 of #463, item 1, also review r1 of #478, item 4): in Off the actor's
/// registration in the level list is the spawn's first committed write. Any `NotYet` after it is terminal: its text
/// starts "stopped after partial writes", its kind and record kept ([`crate::pools::after_partial_writes`]).
/// Reference is unchanged.
///
/// **Lazy loads are writes** (review r2 of #478): before the actor is made, an Off spawn can load definitions, the
/// archetype in [`archetype`] and the template's components and stand-ins in the plan
/// ([`instancing::plan_components_tracked`]; the archetype flag came with review r2 of #481). Once one of those loads
/// has run, every `NotYet` the spawn returns is terminal and prefixed the same way, its kind and record kept.
///
/// Not checked: the abstract and "not instantiable" class-flag refusals. The class table holds no class flags and
/// the record does not identify the bits (open item, decision 2026-11-15-26).
pub fn spawn(vm: &mut Vm, spawner: ObjRef, req: &SpawnRequest) -> Result<Option<ObjRef>, VmError> {
    // A lazy archetype load is a write (review r2 of #481, M1): once it happened, every later refusal of the spawn is
    // marked in Off, its kind and record kept.
    let mut loaded = false;
    let made = spawn_from(vm, spawner, req, &mut loaded);
    if loaded && vm.parity == ParityMode::Off { made.map_err(after_writes) } else { made }
}

fn spawn_from(vm: &mut Vm, spawner: ObjRef, req: &SpawnRequest, loaded: &mut bool) -> Result<Option<ObjRef>, VmError> {
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
    let mut source = defaults;
    if let Some(t) = req.template {
        if space.heap.get(t)?.class != class {
            return Ok(None);
        }
        if t != defaults {
            if vm.parity != ParityMode::Off {
                return Err(NotYet::not_read(
                    "Spawn with an ActorTemplate other than the class default object (how the template's instanced \
                     subobjects are copied)",
                    RECORD,
                )
                .into());
            }
            source = archetype(vm, &actors, class, t, loaded)?;
        }
    }
    let space = &vm.space;
    // The pre-checks read the class defaults and, for an archetype, the archetype too: either one asking for the
    // check refuses (review r1 of #435; which of the two the game reads is not read, UNVERIFIED).
    let either = |name: &str| -> Result<bool, VmError> {
        Ok(flag(space, defaults, name)? || (source != defaults && flag(space, source, name)?))
    };
    let client = byte(space, world_info, "NetMode")? == NET_MODE_CLIENT;
    let off = vm.parity == ParityMode::Off;
    if !off && !req.no_collision_fail && (either("bCollideWorld")? || (either("bCollideWhenPlacing")? && !client)) {
        return Err(collision("the spawn placement spot search"));
    }
    if !off && begun && either("bCollideActors")? {
        return Err(collision(
            "the post-spawn encroachment check (or relocation) of an actor that collides with actors",
        ));
    }
    // Reference keeps the base precheck at its original place, before the reads below (review r2 of #463).
    if !off && begun && (needs_base_search(space, defaults)? || (source != defaults && needs_base_search(space, source)?)) {
        return Err(collision("the base search after PostBeginPlay"));
    }
    let time = float(space, world_info, "TimeSeconds")?;
    let requested = match &req.location {
        Some(v) => v.clone(),
        None => field(space, spawner, "Location")?,
    };
    let rotation = match &req.rotation {
        Some(v) => v.clone(),
        None => field(space, spawner, "Rotation")?,
    };
    let instigator = object(space, spawner, "Instigator")?;
    // Off: the spot search on the template alone (NATIVE_SPAWN_PLACEMENT.md, crate::placement), before the actor is
    // made; a failed search makes nothing. The extent's loads run first (NATIVE_SPAWN_EXTENT.md section 5, step 5.3):
    // a load is a store write, so once one ran, every `NotYet` up to the actor's insertion is marked "stopped after
    // partial writes: ", its kind kept, nothing rolled back (after the insertion the rule below marks them all).
    let mut loaded = false;
    let location = if off && crate::placement::search_runs(space, source, req.no_collision_fail, client)? {
        loaded = crate::placement::load_extent_components(vm, source)?;
        let mark = move |e: VmError| if loaded { crate::pools::after_partial_writes(e) } else { e };
        match crate::placement::spot_search(vm, source, &requested).map_err(mark)? {
            Some(l) => l,
            None => return Ok(None),
        }
    } else {
        requested
    };
    let mark = move |e: VmError| if loaded { crate::pools::after_partial_writes(e) } else { e };
    // In Off the base precheck comes after the placement check (review r1 of #463, item 2), both before anything is
    // made, so a template that needs both meets the placement refusal first.
    let space = &vm.space;
    if off
        && begun
        && (needs_base_search(space, defaults).map_err(mark)?
            || (source != defaults && needs_base_search(space, source).map_err(mark)?))
    {
        return Err(mark(collision("the base search after PostBeginPlay")));
    }

    // The fields written below exist before anything is made (review r2 of #435): a class without one fails here,
    // with nothing left behind.
    for name in ["CreationTime", "WorldInfo", "Location", "Rotation", "Instigator", "Owner"] {
        prop_of(&vm.space, source, name).map_err(mark)?;
    }
    // The world list the actor will join, checked before anything is made (review r1 of #481, item 4): its link field
    // declared and the list walkable. A world that does not hold the list keeps none (no refusal at spawn).
    let joins = match (vm.parity, WorldList::of(&vm.space, source)?) {
        (ParityMode::Off, Some(list)) => {
            let holder = list_holder(&vm.space, &*actors.try_borrow().map_err(borrow_err)?, list)?;
            if let Some(holder) = holder {
                prop_of(&vm.space, source, list.next())?;
                list_members(&vm.space, holder, list)?;
                if list == WorldList::Controllers {
                    prop_of(&vm.space, source, "bPendingDelete")?;
                }
            }
            holder.map(|h| (list, h))
        }
        _ => None,
    };
    // The level list is borrowed for the registration before anything is made (review r3 of #435, item 2): a list
    // another caller holds is refused here, with nothing allocated; the borrow ends right after the insertion, before
    // any event runs.
    let mut level = actors.try_borrow_mut().map_err(borrow_mut_err).map_err(mark)?;
    let actor = match vm.parity {
        ParityMode::Reference => ow_vm::interp::instantiate_from_defaults(vm, class)?,
        ParityMode::Off => {
            // Every refusal of the instancing runs before the actor is made; a failure while making it frees it.
            let plan = instancing::plan_components_tracked(vm, source, &mut loaded).map_err(mark)?;
            let actor = vm.instantiate(class, "").map_err(mark)?;
            let made = vm.space.heap.get(source).map(|o| o.slots.clone()).map_err(VmError::from).and_then(|slots| {
                vm.space.heap.get_mut(actor)?.slots = slots;
                apply_plan(vm, &plan, actor)
            });
            if let Err(e) = made {
                let _ = vm.space.free(actor);
                return Err(mark(e));
            }
            actor
        }
    };
    level.list.push(actor);
    drop(level);
    // The actor is registered: in Off, every error from here on goes through the partial-write rule (review r1 of
    // #463, item 1): a `NotYet` is terminal and prefixed, its kind kept; any other error propagates unchanged; the actor
    // stays (no rollback). Reference keeps its errors as they were.
    let rest = after_insertion(vm, actor, begun, off, req, time, world_info, location, rotation, instigator, joins);
    if off {
        rest.map_err(crate::pools::after_partial_writes)
    } else {
        rest
    }
}

/// The spawn's steps after the actor joined the level list (module documentation, steps 3 and 4).
#[allow(clippy::too_many_arguments)]
fn after_insertion(
    vm: &mut Vm,
    actor: ObjRef,
    begun: bool,
    off: bool,
    req: &SpawnRequest,
    time: f32,
    world_info: ObjRef,
    location: Value,
    rotation: Value,
    instigator: Option<ObjRef>,
    joins: Option<(WorldList, ObjRef)>,
) -> Result<Option<ObjRef>, VmError> {
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
        if off {
            // On the live actor, after the destroyed check: terminal and prefixed (crate::placement).
            return Err(crate::placement::after_pre_begin_play(vm, req.no_collision_fail));
        }
        return Err(collision(
            "the post-spawn encroachment check of an actor that PreBeginPlay made collide with actors",
        ));
    }
    // The world's lists (decision 2026-11-19-92; NATIVE_ACTOR_DESTROY_CONTROLLER.md section 4): a native begin-play
    // step adds a controller (unless it is being deleted) or a pawn at the head. Which step, and so where among the
    // begin-play events, was not identified: here, after PreBeginPlay and the encroachment check (review r1 of #481,
    // item 4), so a refused actor is not reachable through a list (UNVERIFIED). Off only; checked before the actor
    // was made.
    if let Some((list, holder)) = joins
        && !(list == WorldList::Controllers && flag(&vm.space, actor, "bPendingDelete")?)
    {
        link_head(&mut vm.space, holder, list, actor)?;
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

/// The archetype `t` of `class` an Off spawn is made from (decision 2026-11-19-44, review r1 of #435), loaded, or
/// `NotYet { NotRead }` before anything is made:
/// - a live actor (one in the level list) as the template: whether its run-time state is copied is not read;
/// - a template whose properties are not loaded (a resource stand-in, its slots not the class layout's);
/// - a destroyed or pending-delete template (`bDeleteMe`, `bPendingDelete`), and one whose `Owner` is set.
fn archetype(vm: &mut Vm, actors: &Rc<RefCell<Actors>>, class: ClassId, t: ObjRef, loaded: &mut bool) -> Result<ObjRef, VmError> {
    if actors.try_borrow().map_err(borrow_err)?.list.contains(&t) {
        return Err(NotYet::not_read(
            "Spawn with a live actor as its ActorTemplate (whether its run-time state is copied)",
            RECORD,
        )
        .into());
    }
    // A destroyed or pending-delete template (review r2 of #435): what the game does with one is not read.
    if vm.space.heap.get(t)?.destroyed {
        return Err(NotYet::not_read("Spawn with a destroyed actor as its ActorTemplate", RECORD).into());
    }
    // A lazy load is a committed write (review r1 of #481, item 4): a refusal after it is marked.
    let lazily_loaded = vm.defs.is_unloaded(&vm.space, t);
    if lazily_loaded {
        *loaded = true;
        let loaded = vm.defs.ensure_loaded(&mut vm.space, t);
        vm.log.extend(vm.defs.take_log());
        loaded?;
    }
    let checked = (|| -> Result<(), VmError> {
    let want = vm
        .space
        .classes
        .get_layout(class)
        .map(|l| l.slot_count())
        .ok_or_else(|| rule_error("Spawn", format!("unknown class {class:?}")))?;
    if u32::try_from(vm.space.heap.get(t)?.slots.len()).ok() != Some(want) {
        return Err(NotYet::not_read(
            "Spawn with an ActorTemplate whose properties are not loaded (a stand-in with no slots)",
            RECORD,
        )
        .into());
    }
    if flag(&vm.space, t, "bDeleteMe")? || flag(&vm.space, t, "bPendingDelete")? {
        return Err(NotYet::not_read("Spawn with a pending-delete actor as its ActorTemplate", RECORD).into());
    }
    // The copied Owner would become the new actor's previous owner for the owner change (a spurious LostChild, a
    // missing GainedChild): until the spawn's owner rules for a template are read, a template with an Owner waits.
    if object(&vm.space, t, "Owner")?.is_some() {
        return Err(NotYet::not_read(
            "Spawn with an ActorTemplate whose Owner is set (the spawn's owner rules for a template are not read)",
            RECORD,
        )
        .into());
    }
        Ok(())
    })();
    match checked {
        Err(e) if lazily_loaded => return Err(after_writes(e)),
        other => other?,
    }
    Ok(t)
}

/// Step 10's base search condition: no `Base`, collides with the world, `bShouldBaseAtStartup`, and `Physics`
/// `PHYS_None` or `PHYS_Rotating`.
pub(crate) fn needs_base_search(space: &ObjectSpace, o: ObjRef) -> Result<bool, VmError> {
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
/// - A `Controller`: in `ParityMode::Reference` the C++ stand-in (`src/combat_death.cpp`, `Actor.Destroy`: the
///   actor is marked `bDeleteMe` and the answer is true; nothing else runs, it stays in the level list), decision
///   2026-11-16-79. In `ParityMode::Off` (lane IMPL-ACTOR-DESTROY-CONTROLLER, decision 2026-11-19-92, from
///   NATIVE_ACTOR_DESTROY.md and NATIVE_ACTOR_DESTROY_CONTROLLER.md): a player controller whose `Player` is a
///   `LocalPlayer` gets `bPendingDestroy` and answers false; one with another `Player` (a network connection) is
///   `NotYet { NotPorted }`; any other controller takes the generic routine below.
/// - **Off, the world-drop step** ([`plan_world_drop`], [`world_drop`]; review r1 of #472): preflighted before any
///   write. A controller answers the note's first refusal, the actor step's unread engine notification (`NotYet {
///   NotRead }`), until it is read. A pawn in a world whose `WorldInfo` does not declare `PawnList` is `NotYet {
///   LayerAbsent }`, and a pawn's drop is its `PawnList` unlink, run right after `Destroyed()`. A `NotYet` after the
///   first write is marked [`PARTIAL_WRITES`] (kind kept); nothing is rolled back. All UNVERIFIED in the running
///   game.
/// - `NotYet`, before anything changes, for the parts this world cannot run: an actor with Kismet events in `GeneratedEvents` (the destroyed-event class is not
///   confirmed: W5-KISMET), an actor in a state (`EndState` and the state frame: W5-STATES), an actor with a `Base`,
///   with attached actors based on it, or touching others (the unbase and untouch routines are physics natives, not
///   read), and a `Destroy` of an actor whose own destroy routine is running.
/// - Otherwise: `bPendingDelete`, script `Destroyed()`, `SetOwner(None)` (`LostChild` to the old owner), then the
///   actor leaves the level list, `bDeleteMe` is set, the heap marks it destroyed and its timer entries are dropped.
///   It is not freed here.
pub fn destroy(vm: &mut Vm, actor: ObjRef) -> Result<bool, VmError> {
    let actors = actors_of(vm)?;
    let off = vm.parity == ParityMode::Off;
    let drop = match destroy_preflight(vm, &actors, actor)? {
        Preflight::Answer(b) => return Ok(b),
        Preflight::ReferenceStandIn => {
            // The C++ stand-in (module documentation): marked, nothing else.
            set_field(&mut vm.space, actor, "bDeleteMe", Value::Bool(true))?;
            return Ok(true);
        }
        Preflight::Run(drop) => drop,
    };
    set_field(&mut vm.space, actor, "bPendingDelete", Value::Bool(true))?;
    actors
        .try_borrow_mut()
        .map_err(borrow_mut_err)?
        .destroying
        .push(actor);
    let events = (|| {
        send_event(vm, actor, "Destroyed", Vec::new())?;
        if let Some(drop) = &drop {
            world_drop(vm, actor, drop)?;
        }
        set_owner(vm, actor, None)
    })();
    // A NotYet after the first write is terminal (the hub's ruling): marked, its kind kept; nothing rolled back.
    let events = if off { events.map_err(after_writes) } else { events };
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

/// A failure after a first write (the hub's ruling): a `NotYet` gets the partial-writes mark once (`ow_core::notyet`),
/// its kind and record kept; any other error is returned as it is.
pub fn after_writes(e: VmError) -> VmError {
    match e {
        VmError::NotYet(n) => n.after_partial_writes().into(),
        other => other,
    }
}

/// What a destroy will do, decided before its first write ([`destroy_preflight`]).
#[derive(Debug)]
pub(crate) enum Preflight {
    /// The routine answers at once, writing nothing.
    Answer(bool),
    /// `ParityMode::Reference`'s C++ stand-in for a controller (it marks `bDeleteMe`).
    ReferenceStandIn,
    /// The generic routine runs; in Off with the class's world-drop step.
    Run(Option<WorldDrop>),
}

/// The destroy's checks, **read only** (review r1 of #481, item 1): every early answer and every refusal the routine
/// makes before its first write, in order. [`destroy`] runs it, and so does the kill's earliest certain point (the
/// `Dying.BeginState` wrapper, [`register`]).
pub(crate) fn destroy_preflight(vm: &Vm, actors: &Rc<RefCell<Actors>>, actor: ObjRef) -> Result<Preflight, VmError> {
    let space = &vm.space;
    if is_destroyed(space, actor)? {
        return Ok(Preflight::Answer(true));
    }
    if flag(space, actor, "bStatic")? || flag(space, actor, "bNoDelete")? {
        return Ok(Preflight::Answer(false));
    }
    if byte(space, actor, "Role")? < ROLE_AUTHORITY && !flag(space, actor, "bNetTemporary")? {
        return Ok(Preflight::Answer(false));
    }
    let a = actors.try_borrow().map_err(borrow_err)?;
    if a.destroying.contains(&actor) {
        return Err(NotYet::not_read("Destroy of an actor whose destroy routine is running", RECORD).into());
    }
    let x = space.heap.get(actor)?;
    let off = vm.parity == ParityMode::Off;
    if class_named_in_chain(space, x.class, "Controller") {
        if !off {
            return Ok(Preflight::ReferenceStandIn);
        }
        // The controller and player case (NATIVE_ACTOR_DESTROY.md section 3; review r1 of #481, item 3). The world's
        // own local-player identity decides: a local player's controller is not destroyed, and the engine is asked to
        // drop the player (the host's viewport). The port has no host effect for it and no read-back check, so it is
        // refused before any write.
        if a.local_players.contains(&actor) {
            return Err(NotYet::layer_absent(
                "the destroy of a local player's controller (the engine drops the player from the viewport)",
                "the host's viewport (ow-host)",
                RECORD_CONTROLLER,
            )
            .into());
        }
        if class_named_in_chain(space, x.class, "PlayerController") && object(space, actor, "Player")?.is_some() {
            return Err(NotYet::not_ported(
                "the destroy of a network player's controller (its child connections)",
                RECORD_CONTROLLER,
            )
            .into());
        }
    }
    drop(a);
    if x.state.is_some() {
        return Err(NotYet::layer_absent(
            "EndState when an actor in a state is destroyed",
            "ow-vm states (lane W5-STATES)",
            RECORD,
        )
        .into());
    }
    if objects(space, actor, "GeneratedEvents")?.iter().any(Option::is_some) {
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
    for b in objects(space, actor, "Attached")?.into_iter().flatten() {
        if !is_destroyed(space, b)? && object(space, b, "Base")? == Some(actor) {
            return Err(NotYet::not_read("unbasing the actors attached to a destroyed actor", RECORD).into());
        }
    }
    for t in objects(space, actor, "Touching")?.into_iter().flatten() {
        if !is_destroyed(space, t)? {
            return Err(NotYet::not_read("the untouch routine for a destroyed actor's neighbours", RECORD).into());
        }
    }
    // The class's world-drop step, preflighted (Off; decision 2026-11-19-92).
    Ok(Preflight::Run(if off { Some(plan_world_drop(vm, actors, actor)?) } else { None }))
}

/// The scripts the kill's earliest certain point wraps (NATIVE_ACTOR_DESTROY_CONTROLLER.md section 5.1): the pawn's
/// `Dying.BeginState`, `Pawn`'s and `WillowPawn`'s (which replaces it and does not call it).
pub const DYING_BEGIN_STATE: [&str; 2] = ["Engine.Pawn.Dying.BeginState", "WillowGame.WillowPawn.Dying.BeginState"];

/// The wrapper of [`DYING_BEGIN_STATE`] (Off; review r1 of #481, item 1). At the entry of the pawn's `Dying.BeginState`,
/// before its first write: when the pawn has a `Controller` whose `Pawn` is this pawn and which is not a player
/// (`bIsPlayer` false), `Controller.PawnDied` will reach `Destroy()` on that controller. So that destroy's read-only
/// checks ([`destroy_preflight`]) run here, and a refusal stops the kill before the body runs, the pawn's dying writes
/// and both possession links untouched. It comes after `Pawn.Died`'s and `PlayDying`'s writes, so a `NotYet` is marked
/// (kind and record kept). Otherwise the body runs as script.
fn dying_begin_state(vm: &mut Vm, this: Option<ObjRef>, body: &mut ow_vm::api::ScriptBody<'_>) -> Result<CallResult, VmError> {
    if vm.parity == ParityMode::Off
        && let Some(pawn) = this
        && let Some(c) = object(&vm.space, pawn, "Controller")?
        && object(&vm.space, c, "Pawn")? == Some(pawn)
        && !flag(&vm.space, c, "bIsPlayer")?
    {
        let actors = actors_of(vm)?;
        destroy_preflight(vm, &actors, c).map_err(after_writes)?;
    }
    body(vm)
}

/// The world-drop step of one destroy, checked before the destroy's first write.
#[derive(Debug)]
pub(crate) struct WorldDrop {
    /// The list the actor leaves, and its holder.
    list: Option<(WorldList, ObjRef)>,
    /// A controller: whether it is the authority (its pool manager is destroyed then).
    authority: bool,
    /// A `WillowMind`: whether its `InteractionProxy` is destroyed.
    proxy: bool,
}

/// The text of the controller destroy's refusals (NATIVE_ACTOR_DESTROY_CONTROLLER.md section 5.2, in table order).
pub const NOTIFICATION_NOT_READ: &str = "the actor step's engine notification at destroy";
pub const LOOKUPS_NOT_READ: &str = "the mind's two engine lookups at destroy";
pub const RULE_ENGINE_NOT_READ: &str = "the rule engine's call at its mind's destroy";

/// The world-drop step's preflight (NATIVE_ACTOR_DESTROY_CONTROLLER.md sections 5.2 and 5.5, review r1 of #472),
/// before any write. For a controller, the table's refusals in its order, the first found being the answer:
/// 1. the actor step's engine notification: `NotYet { NotRead }` (not read, not proven irrelevant). It runs on every
///    controller's destroy, so every Off controller destroy answers it until it is read;
/// 2. a `GearboxMind`'s two engine lookups: `NotYet { NotRead }`;
/// 3. a world that does not hold `ControllerList`: `NotYet { LayerAbsent }`;
/// 4. a `GearboxMind` with `MyRuleEngine` set: `NotYet { NotRead }`.
///
/// For a pawn: a world that does not hold `PawnList` is `NotYet { LayerAbsent }`. A modelled list without the pawn is
/// not a refusal. The actor step's notification also belongs to every other actor's destroy. The generic path omits
/// it for non-controllers, a documented UNVERIFIED gap (the note's open question 6, a hub decision).
///
/// Every field the step reads or writes must be declared (a typed error otherwise), and a list that does not walk is
/// bad data.
pub(crate) fn plan_world_drop(vm: &Vm, actors: &Rc<RefCell<Actors>>, actor: ObjRef) -> Result<WorldDrop, VmError> {
    let space = &vm.space;
    let class = space.heap.get(actor)?.class;
    let Some(list) = WorldList::of(space, actor)? else {
        return Ok(WorldDrop { list: None, authority: false, proxy: false });
    };
    if list == WorldList::Controllers {
        // Row 1 holds for every controller; the declarations are still checked first (bad data is not a NotYet).
        object(space, actor, "ResourcePoolManager")?;
        if class_named_in_chain(space, class, "GearboxMind") {
            object(space, actor, "MyRuleEngine")?;
        }
        if class_named_in_chain(space, class, "WillowMind") {
            object(space, actor, "InteractionProxy")?;
        }
        return Err(NotYet::not_read(NOTIFICATION_NOT_READ, RECORD_UNLINK).into());
    }
    Ok(WorldDrop { list: Some((list, list_ready(space, actors, actor, list)?)), authority: false, proxy: false })
}

/// The rows of section 5.2 after the notification, in order, for a controller whose notification and lookups are read
/// (none are today): the controller list, then the rule engine. Kept so the order holds when the first rows are read;
/// tested directly.
#[cfg_attr(not(test), allow(dead_code))] // reached in tests only until rows 1 and 2 are read
pub(crate) fn plan_controller_rows(vm: &Vm, actors: &Rc<RefCell<Actors>>, actor: ObjRef) -> Result<WorldDrop, VmError> {
    let space = &vm.space;
    let class = space.heap.get(actor)?.class;
    let holder = list_ready(space, actors, actor, WorldList::Controllers)?;
    if class_named_in_chain(space, class, "GearboxMind") && object(space, actor, "MyRuleEngine")?.is_some() {
        return Err(NotYet::not_read(RULE_ENGINE_NOT_READ, RECORD_UNLINK).into());
    }
    Ok(WorldDrop {
        list: Some((WorldList::Controllers, holder)),
        authority: byte(space, actor, "Role")? >= ROLE_AUTHORITY,
        proxy: class_named_in_chain(space, class, "WillowMind") && {
            object(space, actor, "InteractionProxy")?;
            true
        },
    })
}

/// The holder of `list` for `actor`, checked: the world holds the list (else `NotYet { LayerAbsent }`), the actor
/// declares the link, and the list walks.
fn list_ready(space: &ObjectSpace, actors: &Rc<RefCell<Actors>>, actor: ObjRef, list: WorldList) -> Result<ObjRef, VmError> {
    let holder = list_holder(space, &*actors.try_borrow().map_err(borrow_err)?, list)?;
    let Some(holder) = holder else {
        return Err(NotYet::layer_absent(list.named(), "ow-world (the WorldInfo's list)", RECORD_UNLINK).into());
    };
    object(space, actor, list.next())?;
    list_members(space, holder, list)?;
    Ok(holder)
}

/// The class's world-drop step, run right after `Destroyed()` (section 2, step 4; section 3, step 2), in order:
/// - a controller: its `ResourcePoolManager`, when still set on the authority, destroyed and the field None; the
///   `ControllerList` unlink, and its `NextController` None;
/// - a `GearboxMind`: its rule engine (preflighted: none here); its two engine lookups change no game state (not run);
/// - a `WillowMind`: its `InteractionProxy`, when set, destroyed (a nested destroy; the field is not cleared);
/// - a pawn: the `PawnList` unlink; its `NextPawn` is left.
///
/// The actor step's notification and the mind's lookups are refused in the preflight, so a controller reaches this
/// step only through [`plan_controller_rows`] (tests, until they are read).
pub(crate) fn world_drop(vm: &mut Vm, actor: ObjRef, drop: &WorldDrop) -> Result<(), VmError> {
    let Some((list, holder)) = drop.list else {
        return Ok(());
    };
    if list == WorldList::Controllers {
        if drop.authority
            && let Some(pool) = object(&vm.space, actor, "ResourcePoolManager")?
        {
            destroy(vm, pool)?;
            set_field(&mut vm.space, actor, "ResourcePoolManager", Value::Object(None))?;
        }
        unlink(&mut vm.space, holder, list, actor)?;
        set_field(&mut vm.space, actor, list.next(), Value::Object(None))?;
        if drop.proxy
            && let Some(proxy) = object(&vm.space, actor, "InteractionProxy")?
        {
            destroy(vm, proxy)?;
        }
    } else {
        unlink(&mut vm.space, holder, list, actor)?;
    }
    Ok(())
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
    // The kill's earliest certain point (review r1 of #481, item 1); a package set without the script binds nothing.
    for path in DYING_BEGIN_STATE {
        let script = vm.find_function(path).ok().and_then(|f| vm.function(f)).is_some_and(|d| !d.is_native());
        if script {
            vm.wrap_script(path, Rc::new(dying_begin_state))?;
        }
    }
    Ok(())
}
