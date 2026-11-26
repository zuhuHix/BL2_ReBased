//! Owner: lane W6-COMBAT. The stock hit chain on the VM (plan step S-b), the melee swing fixture (S-e), the AI pawn
//! a run spawns (S-a, the half of `src/combat.cpp` W5-POOLS did not take) and the combat pack. Port of
//! `src/combat_hit.cpp` and the pawn half of `src/combat.cpp`. AI-assisted (Claude Code).
//!
//! **What runs where.** [`hit_with_type`] runs the victim's installed script `TakeDamage` with the instigator's
//! controller, so `TakeDamage -> AdjustDamage -> SetHealth -> NotifyTakeHit -> ...` is the game's own script; the
//! natives it reaches are bound by [`Pack`] (this module, `crate::damage`, `crate::death`, `crate::down`,
//! `crate::xp`). Every rule is from NATIVE_DAMAGE_DEATH.md sections 1 to 7 and NATIVE_COMBAT_GROUNDWORK.md sections 3
//! and 6, read from the executable, and is **UNVERIFIED** in the running game unless a line says otherwise; the C++
//! oracle is the reference for every text and number here.
//!
//! **The state** of the natives is one VM service, [`CombatState`]: the pipeline pool, the demigod holds, the event
//! list the reports cut, the kills, the consumer handles and the owner's callbacks. A native borrows it for a short
//! scope and never across a script call (the VM's borrow rule).
//!
//! **Binding** ([`bind`]): as the C++ `bind`, a native is bound under the key the loader derives for its function
//! when the package set declares that function native; a set without it (an invented one) has no such binding. A key
//! another pack bound first is left to that pack: `ResourcePool.GetCurrentValue` (W5-POOLS) and `Actor.Destroy`
//! (W5-WORLD), which the C++ death half also binds, are theirs ([`LEFT_TO_LOWER_PACKS`]).
//!
//! **Parity modes** (architecture R3). Where the C++ reads a stand-in, `ParityMode::Reference` reproduces it and
//! `ParityMode::Off` answers `NotYet` naming what is missing:
//! - an attribute the VM object's class does not declare: Reference reads the stage's identity and notes it once
//!   (`note: <Class>.<name> is not a property of the VM object (read as <x>)`); Off is `NotYet { NotRead }`
//!   ([`attribute`]);
//! - a C++ "not implemented" log line (`UNIMPLEMENTED <what> (not implemented)`): Reference logs it and goes on; Off is
//!   `NotYet { NotPorted }` ([`not_implemented`]);
//! - the "0 with base 0 is 1" reading of a pawn's damage-type resistance and the critical-hit bonus: `crate::damage`;
//! - the AI component's demigod hold (a list of reasons), `GearboxMind.GetAIComponent` (the mind's property) and the
//!   hold behavior's pawn-to-mind context ([`ai_interface_for`]): Reference only. Off has NATIVE_AI_HOLDS.md's hold
//!   rule and the hold behavior's own run ([`crate::holds`], decision 2026-11-16-83).

pub mod access;
pub mod attribute;
pub mod shot;
pub mod pools;
pub mod opinion;

use std::cell::RefCell;
use std::rc::Rc;

use ow_attr::api::EvalCx;
use ow_core::{DetMap, DetSet, NotYet, ParityMode};
use ow_object::ids::{ClassId, ObjRef};
use ow_object::value::Value;
use ow_vm::api::{CallCensus, NativeCall, NativeFn, NativePack, Vm};
use ow_vm::error::VmError;
use ow_vm::interp::instantiate_from_defaults;

use crate::death::BehaviorEvent;
use crate::xp::Kill;
use access::{
    class_name, error, field, g5, get, integer, is_a, loaded, number, object, object_of, path_of,
    required, set, set_field, set_required, truth,
};

/// The records of the hit chain.
pub const RECORD: &str =
    "NATIVE_DAMAGE_DEATH.md sections 1-7; NATIVE_COMBAT_GROUNDWORK.md sections 3 and 6";
/// The record of the AI pawn the run spawns (the stock spawn path is not modelled).
pub const RECORD_SPAWN: &str = "NATIVE_COMBAT_GROUNDWORK.md sections 1.4 and 2 (plan step S-a)";

/// The prefix of a C++ "not implemented" log line (`src/combat.cpp`).
pub const STUB_PREFIX: &str = "UNIMPLEMENTED ";

/// The C++ death half binds these too; here they are the lower packs' (module documentation, "Binding").
pub const LEFT_TO_LOWER_PACKS: [&str; 2] = [
    "Engine.ResourcePool.GetCurrentValue",
    "Engine.Actor.Destroy",
];

/// The damage source classes of NATIVE_DAMAGE_DEATH.md section 1 (stage "damage source"), in the C++ order: which
/// attribute pair a source selects.
pub const SOURCE_CLASSES: [(&str, &str); 6] = [
    ("WillowGame.WillowDmgSource_Bullet", "Bullet"),
    ("WillowGame.WillowDmgSource_Melee", "Melee"),
    ("WillowGame.WillowDmgSource_Grenade", "Grenade"),
    ("WillowGame.WillowDmgSource_Rocket", "Rocket"),
    ("WillowGame.WillowDmgSource_StatusEffect", "StatusEffect"),
    ("WillowGame.WillowDmgSource_Skill", "Skill"),
];

/// What `NotifyTakeHit` hands the owner to raise `OnTakeDamage` with (NATIVE_DAMAGE_DEATH.md section 5).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct TakeDamageEvent {
    pub pawn: ObjRef,
    pub instigator: Option<ObjRef>,
    pub damage_type: Option<ObjRef>,
    pub damage: f64,
    pub shield_damage: f64,
    pub source: Option<ClassId>,
}

/// The owner's raise of `OnTakeDamage`.
pub type TakeDamageFn = Rc<dyn Fn(&mut Vm, &TakeDamageEvent) -> Result<(), VmError>>;
/// The owner's raise of a death-chain behavior event (`crate::death`).
pub type BehaviorEventFn = Rc<dyn Fn(&mut Vm, &BehaviorEvent) -> Result<(), VmError>>;
/// An object the owner supplies (the injured definition of the player's class, `crate::down`).
pub type ObjectFn = Rc<dyn Fn(&mut Vm) -> Result<Option<ObjRef>, VmError>>;

/// The state of the combat natives (module documentation). A VM service.
#[non_exhaustive]
#[derive(Default)]
pub struct CombatState {
    /// The world's `WorldInfo` (its `TimeSeconds`, `Game` and `GRI`).
    pub world: Option<ObjRef>,
    /// `ROLE_Authority`, looked up by its enum name.
    pub authority: u8,
    /// The events of the natives since the start, in order (a hit's report cuts its part).
    pub events: Vec<String>,
    /// Names a stage wanted that the VM object does not have, each noted once.
    missing: DetSet<String>,
    /// The pool of pipeline objects (NATIVE_COMBAT_GROUNDWORK.md section 3.1) and whether each is in use.
    pipelines: Vec<(ObjRef, bool)>,
    /// `GD_Globals.General.Globals`, found on first use.
    globals: Option<ObjRef>,
    /// Stock objects by path (damage types), loaded once.
    by_path: DetMap<String, ObjRef>,
    /// AI component -> the reasons of its demigod holds (the Reference stand-in; Off keeps `DemigodHolds`).
    demigod_holds: DetMap<ObjRef, Vec<String>>,
    /// `WillowMind.SetAIDemigod`.
    mind_demigod: DetMap<ObjRef, bool>,
    /// The controllers `spawn_player` made.
    pub players: Vec<ObjRef>,
    /// What the experience natives did per kill (`crate::xp`).
    pub kills: Vec<Kill>,
    /// Consumer handle `PID` -> pawn (`crate::death`).
    pub(crate) consumers: DetMap<i32, ObjRef>,
    pub(crate) next_consumer: i32,
    /// `enable_deaths` ran.
    pub deaths: bool,
    pub on_take_damage: Option<TakeDamageFn>,
    pub on_behavior_event: Option<BehaviorEventFn>,
    pub injured_definition: Option<ObjectFn>,
    /// Spawned pawn -> the untouched copy of the archetype its spawn copied (lane IMPL-SETDEFAULTPHYSICS-AND-MIND-RECALC;
    /// the port does not model `ObjectArchetype`, so `SetDefaultPhysics` reads this record). Kept for the pawn's life.
    pub(crate) archetypes: DetMap<ObjRef, ObjRef>,
    /// `WillowPawn`'s process-wide can-cloak flag (`StaticCanCloak`), `None` until set: true.
    can_cloak: Option<bool>,
    /// `WillowMind`'s process-wide AI-god flag (`GetAIGod`; lane IMPL-LINK34-AFTER-MIND), initially false.
    ai_god: bool,
}

impl CombatState {
    /// The can-cloak flag (initially true).
    pub fn can_cloak(&self) -> bool {
        self.can_cloak.unwrap_or(true)
    }
    pub fn set_can_cloak(&mut self, on: bool) {
        self.can_cloak = Some(on);
    }
    /// The AI-god flag (initially false).
    pub fn ai_god(&self) -> bool {
        self.ai_god
    }
    pub fn set_ai_god(&mut self, on: bool) {
        self.ai_god = on;
    }
}

/// The archetype record of `pawn` (`CombatState::archetypes`), if its spawn made one.
pub fn archetype_of(vm: &Vm, pawn: ObjRef) -> Result<Option<ObjRef>, VmError> {
    with_state(vm, |s| s.archetypes.get(&pawn).copied())
}

impl std::fmt::Debug for CombatState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CombatState")
            .field("world", &self.world)
            .field("events", &self.events.len())
            .field("kills", &self.kills.len())
            .finish_non_exhaustive()
    }
}

/// The first consumer handle `prepare_for_death` gives out (the C++ bridge's Marcus has 0).
pub const FIRST_CONSUMER: i32 = 100;

/// The VM's combat state, or `NotYet { LayerAbsent }` without [`Pack`].
pub fn state(vm: &Vm) -> Result<Rc<RefCell<CombatState>>, VmError> {
    vm.service::<CombatState>().ok_or_else(|| {
        NotYet::layer_absent("the combat state", "ow-combat (hit::Pack)", RECORD).into()
    })
}

/// Runs `f` on the borrowed state (a short scope: no script call inside).
pub fn with_state<T>(vm: &Vm, f: impl FnOnce(&mut CombatState) -> T) -> Result<T, VmError> {
    let s = state(vm)?;
    let mut s = s
        .try_borrow_mut()
        .map_err(|_| VmError::ServiceBorrowed("CombatState"))?;
    Ok(f(&mut s))
}

/// Appends an event (C++ `logEvent` / `note`).
pub fn note(vm: &Vm, text: impl Into<String>) -> Result<(), VmError> {
    let text = text.into();
    with_state(vm, |s| s.events.push(text))
}

/// Appends `note: <text>` the first time `key` is seen (the C++ `missing_` map).
pub fn note_once(vm: &Vm, key: &str, text: impl Into<String>) -> Result<(), VmError> {
    let text = text.into();
    with_state(vm, |s| {
        if s.missing.insert(key.to_string()) {
            s.events.push(format!("note: {text}"));
        }
    })
}

/// A C++ "not implemented" site (module documentation, "Parity modes").
pub fn not_implemented(vm: &mut Vm, what: &str, record: &'static str) -> Result<(), VmError> {
    match vm.parity {
        ParityMode::Reference => {
            vm.log
                .push(format!("{STUB_PREFIX}{what} (not implemented)"));
            Ok(())
        }
        ParityMode::Off => Err(NotYet::not_ported(what.to_string(), record).into()),
    }
}

/// The attribute `name` of `owner`: the object's plain property of that name (the VM loads attribute defaults with an
/// empty modifier stack, so nothing has modified it). A name the class does not declare: Reference reads `fallback`
/// (the identity of the stage that wanted it) and notes it once; Off is `NotYet { NotRead }` (module documentation).
pub fn attribute(vm: &mut Vm, owner: ObjRef, name: &str, fallback: f32) -> Result<f32, VmError> {
    if let Some(v) = get(vm, owner, name)? {
        return Ok(access::to_f32(number(&v)));
    }
    let key = format!("{}.{name}", class_name(vm, owner));
    match vm.parity {
        ParityMode::Reference => {
            note_once(
                vm,
                &key,
                format!(
                    "{key} is not a property of the VM object (read as {})",
                    g5(f64::from(fallback))
                ),
            )?;
            Ok(fallback)
        }
        ParityMode::Off => Err(NotYet::not_read(
            format!("the attribute {key}: the VM object's class does not declare it"),
            RECORD,
        )
        .into()),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Binding
// ---------------------------------------------------------------------------------------------------------------

/// Binds `f` to the native at `path` (`Package.Class.Function`) when the package set declares it native (module
/// documentation, "Binding"). A function the set lacks, a script function, or a function already bound: nothing.
/// The binding is for `path`'s package (`Vm::register_native_for`), so a same-named native of another package is
/// neither bound nor shadowed by it. `#[track_caller]` (and on [`bind_fn`]): the shadow audit records the file that
/// called this helper, not `hit.rs`, so a shadow between two users of the helper is seen (review of #748).
#[track_caller]
pub fn bind(vm: &mut Vm, path: &str, f: NativeFn) -> Result<(), VmError> {
    let Ok(id) = vm.find_function(path) else {
        return Ok(());
    };
    if !vm.function(id).is_some_and(|d| d.is_native()) || vm.is_bound(id) {
        return Ok(());
    }
    vm.register_native_for(id, f)
}

/// [`bind`] of a closure.
#[track_caller]
pub fn bind_fn(
    vm: &mut Vm,
    path: &str,
    f: impl Fn(&mut NativeCall<'_>) -> Result<Value, VmError> + 'static,
) -> Result<(), VmError> {
    bind(vm, path, Rc::new(f))
}

/// Argument `i` as an object (`None` for a missing argument or a None object).
pub fn arg_object(c: &NativeCall<'_>, i: usize) -> Result<Option<ObjRef>, VmError> {
    if i >= c.arg_count() {
        return Ok(None);
    }
    Ok(object_of(c.arg(i)?))
}

/// Argument `i` as a number (0 for a missing one).
pub fn arg_number(c: &NativeCall<'_>, i: usize) -> Result<f64, VmError> {
    if i >= c.arg_count() {
        return Ok(0.0);
    }
    Ok(number(c.arg(i)?))
}

/// Argument `i` as a name or string's text (`""` for a missing one).
pub fn arg_text(c: &NativeCall<'_>, i: usize) -> Result<String, VmError> {
    if i >= c.arg_count() {
        return Ok(String::new());
    }
    Ok(match c.arg(i)? {
        Value::Name(n) => c.vm.space.names.text(*n).into_owned(),
        Value::Str(s) => s.to_string(),
        _ => String::new(),
    })
}

/// Argument `i`'s truth (false for a missing one).
pub fn arg_truth(c: &NativeCall<'_>, i: usize) -> Result<bool, VmError> {
    if i >= c.arg_count() {
        return Ok(false);
    }
    Ok(truth(c.vm, c.arg(i)?))
}

/// The native's object (`self`), or an error naming the native.
pub fn this(c: &NativeCall<'_>, what: &str) -> Result<ObjRef, VmError> {
    c.this
        .ok_or_else(|| error(format!("combat: {what} called without an object")))
}

/// The combat pack's pawn and pool natives alone (`pools::register`: the class starting values, the mind route's
/// `SetDefaultPhysics`, `GetExpLevel`, `IsStaggered`, `DropCombatPerch` and the pool manager), in Off only and
/// keep-existing, for a world that does not register the whole combat pack: link 9's Phaselock world (lane
/// LINK9-PAWN-NATIVES). Nothing under Reference.
pub fn register_pawn_natives(vm: &mut Vm) -> Result<(), VmError> {
    pools::register(vm)
}

/// The combat natives as a [`NativePack`]: the hit chain (here), `crate::damage`, `crate::death`, `crate::down` and
/// `crate::xp`, and the [`CombatState`] service. Register it after the world's packs (`World::new` does).
#[derive(Debug, Clone, Copy, Default)]
pub struct Pack;

impl NativePack for Pack {
    fn register(&self, vm: &mut Vm) -> Result<(), VmError> {
        if vm.service::<CombatState>().is_none() {
            vm.add_service(CombatState {
                authority: ow_world::actors::ROLE_AUTHORITY,
                next_consumer: FIRST_CONSUMER,
                ..CombatState::default()
            })?;
        }
        // The behavior kernel's natives have one owner (ow-behavior); this pack routes its consumers to it.
        ow_behavior::kernel::natives::Pack.register(vm)?;
        register_hit(vm)?;
        pools::register(vm)?;
        attribute::register(vm)?;
        crate::damage::register(vm)?;
        crate::death::register(vm)?;
        crate::down::register(vm)?;
        crate::loot::exp_level::register(vm)?;
        crate::xp::register(vm)?;
        // Off only (lane IMPL-GO-DOWN-NATIVES): last, so every binding above keeps its key.
        crate::go_down::register(vm)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Lookups the natives share
// ---------------------------------------------------------------------------------------------------------------

/// Calls the method `name` of `o`'s class (virtual lookup in the object's current state) with `args`; `None` when
/// the class has no such function.
pub fn call_method(
    vm: &mut Vm,
    o: ObjRef,
    name: &str,
    args: Vec<Value>,
) -> Result<Option<Value>, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let state = vm.space.heap.get(o)?.state;
    match vm.find_method(class, name, state)? {
        Some(f) => Ok(Some(vm.call(f, Some(o), args)?.ret)),
        None => Ok(None),
    }
}

/// A stock object by path (C++ `stockObject`), loaded once: the definition store's holder of the path.
pub fn stock_object(vm: &mut Vm, path: &str) -> Result<ObjRef, VmError> {
    if let Some(o) = with_state(vm, |s| s.by_path.get(path).copied())? {
        return Ok(o);
    }
    let parsed = ow_core::ObjectPath::parse(path)
        .map_err(|_| error(format!("combat hit: no such stock object {path}")))?;
    let found = vm.defs.find(&mut vm.space, &parsed);
    vm.log.extend(vm.defs.take_log());
    let o = found
        .ok()
        .flatten()
        .ok_or_else(|| error(format!("combat hit: no such stock object {path}")))?;
    with_state(vm, |s| s.by_path.insert(path.to_string(), o))?;
    Ok(o)
}

/// `GD_Globals.General.Globals`: the level-difference tables and the scales of the pipeline.
pub fn globals(vm: &mut Vm) -> Result<ObjRef, VmError> {
    if let Some(g) = with_state(vm, |s| s.globals)? {
        return Ok(g);
    }
    let g = stock_object(vm, "GD_Globals.General.Globals")?;
    with_state(vm, |s| s.globals = Some(g))?;
    Ok(g)
}

/// Whether `o` is a `WillowPlayerController`.
pub fn is_player_controller(vm: &mut Vm, o: Option<ObjRef>) -> Result<bool, VmError> {
    match o {
        Some(o) => is_a(vm, o, "WillowGame.WillowPlayerController"),
        None => Ok(false),
    }
}

/// What a context source is (C++ `describe`): a player controller and its replication info, or an AI pawn and its
/// mind (the mind's pawn, the pawn's `MyWillowMind`).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Described {
    pub controller: Option<ObjRef>,
    pub replication_info: Option<ObjRef>,
    pub pawn: Option<ObjRef>,
    pub mind: Option<ObjRef>,
}

/// [`Described`] of `source`, `None` for any other object.
pub fn describe(vm: &mut Vm, source: Option<ObjRef>) -> Result<Option<Described>, VmError> {
    let Some(source) = source else {
        return Ok(None);
    };
    if is_player_controller(vm, Some(source))? {
        return Ok(Some(Described {
            controller: Some(source),
            replication_info: object(vm, source, "PlayerReplicationInfo")?,
            ..Described::default()
        }));
    }
    if is_a(vm, source, "WillowGame.WillowMind")? {
        return Ok(Some(Described {
            mind: Some(source),
            pawn: object(vm, source, "Pawn")?,
            ..Described::default()
        }));
    }
    if is_a(vm, source, "WillowGame.WillowAIPawn")? {
        return Ok(Some(Described {
            pawn: Some(source),
            mind: object(vm, source, "MyWillowMind")?,
            ..Described::default()
        }));
    }
    Ok(None)
}

/// The experience level of an actor (the balanced-actor interface read as the level fields; UNVERIFIED reading, C++
/// `levelOf`): an AI pawn or its mind answers with the pawn's `ExpLevel`; a player controller or its pawn with the
/// replication info's `ExpLevel`.
pub fn level_of(vm: &mut Vm, actor: Option<ObjRef>) -> Result<i32, VmError> {
    let Some(actor) = actor else { return Ok(0) };
    if !is_player_controller(vm, Some(actor))?
        && let Some(d) = describe(vm, Some(actor))?
        && let Some(pawn) = d.pawn
        && let Some(level) = get(vm, pawn, "ExpLevel")?
    {
        return Ok(level_value(&level));
    }
    let mut current = actor;
    if let Some(controller) = object(vm, current, "Controller")?
        && object(vm, current, "MyWillowMind")?.is_none()
    {
        current = controller;
    }
    if let Some(info) = object(vm, current, "PlayerReplicationInfo")?
        && let Some(level) = get(vm, info, "ExpLevel")?
    {
        return Ok(level_value(&level));
    }
    let class_differs = vm.space.heap.get(current)?.class != vm.space.heap.get(actor)?.class;
    if let Some(pawn) = object(vm, current, "Pawn")?
        && class_differs
    {
        current = pawn;
    }
    Ok(match get(vm, current, "ExpLevel")? {
        Some(level) => level_value(&level),
        None => 0,
    })
}

fn level_value(v: &Value) -> i32 {
    // An ExpLevel is an int; the C++ `int(integer())` keeps the low 32 bits.
    access::low_i32(integer(v))
}

/// An `AttributeInitializationData` value evaluated with `source` as the context source (the one evaluator).
pub fn evaluate_for(vm: &mut Vm, data: &Value, source: Option<ObjRef>) -> Result<f32, VmError> {
    let init = ow_attr::eval::init_data(&vm.space, data)?;
    let attrs = vm.attrs.clone();
    Ok(attrs.evaluate_init(&init, source, None, &mut EvalCx::new(vm))?)
}

// ---------------------------------------------------------------------------------------------------------------
// Allegiance and the hit region
// ---------------------------------------------------------------------------------------------------------------

fn same_allegiance(vm: &Vm, a: ObjRef, b: ObjRef) -> bool {
    if a == b {
        return true;
    }
    let path = path_of(vm, Some(a));
    !path.is_empty() && path == path_of(vm, Some(b))
}

/// `Pawn.GetOpinion(Other)` (NATIVE_COMBAT_GROUNDWORK.md section 6): this pawn's allegiance A and the other's B (the
/// other pawn's, its controller's pawn's, or the other itself if it is an allegiance); no A or no B is Neutral. B is
/// A: A's `SelfOpinion`; else the first `MyOpinions` entry of A for B; else the first `OtherOpinions` entry of B for
/// A; else B's `ForcedOtherOpinion` if it forces all, else A's `DefaultOpinion`. 0 Enemy, 1 Neutral, 2 Friendly.
/// UNVERIFIED.
pub fn opinion(vm: &mut Vm, me: ObjRef, other: Option<ObjRef>) -> Result<i64, VmError> {
    let (Some(mine), Some(other)) = (object(vm, me, "Allegiance")?, other) else {
        return Ok(1);
    };
    let mut actor = other;
    if let Some(pawn) = object(vm, actor, "Pawn")? {
        actor = pawn; // a controller answers through its pawn
    }
    let theirs = match object(vm, actor, "Allegiance")? {
        Some(a) => Some(loaded(vm, a)?),
        None if class_name(vm, actor) == "PawnAllegiance" => Some(actor),
        None => None,
    };
    let Some(theirs) = theirs else { return Ok(1) };
    let own = loaded(vm, mine)?;
    let opinion_of =
        |vm: &Vm, entry: &Value| field(vm, entry, "Opinion").map_or(1, |v| integer(&v));
    if same_allegiance(vm, own, theirs) {
        return Ok(get(vm, own, "SelfOpinion")?.map_or(1, |v| integer(&v)));
    }
    for entry in access::array(vm, own, "MyOpinions")? {
        if let Some(named) = field(vm, &entry, "Allegiance").as_ref().and_then(object_of) {
            let named = loaded(vm, named)?;
            if same_allegiance(vm, named, theirs) {
                return Ok(opinion_of(vm, &entry));
            }
        }
    }
    for entry in access::array(vm, theirs, "OtherOpinions")? {
        if let Some(named) = field(vm, &entry, "Allegiance").as_ref().and_then(object_of) {
            let named = loaded(vm, named)?;
            if same_allegiance(vm, named, own) {
                return Ok(opinion_of(vm, &entry));
            }
        }
    }
    let forced = get(vm, theirs, "bForceAllOtherOpinions")?.is_some_and(|v| truth(vm, &v));
    let fallback = if forced {
        get(vm, theirs, "ForcedOtherOpinion")?
    } else {
        get(vm, own, "DefaultOpinion")?
    };
    Ok(fallback.map_or(1, |v| integer(&v)))
}

/// The hit region of a victim (NATIVE_DAMAGE_DEATH.md section 1, stage "hit region lookup"): the body class's
/// `HitRegionList` entry whose bone names include the hit's bone, else the `DefaultHitRegion`. A hit with no bone
/// takes the default region (noted once).
pub fn hit_region(
    vm: &mut Vm,
    victim: ObjRef,
    hit_info: &Value,
) -> Result<Option<ObjRef>, VmError> {
    let Some(body) = object(vm, victim, "BodyClass")? else {
        return Ok(None);
    };
    let body = loaded(vm, body)?;
    let bone = match field(vm, hit_info, "BoneName") {
        Some(Value::Name(n)) => vm.space.names.text(n).to_ascii_lowercase(),
        Some(Value::Str(s)) => s.to_ascii_lowercase(),
        _ => String::new(),
    };
    if bone.is_empty() || bone == "none" {
        note_once(
            vm,
            "hit region: no bone",
            "hit region: the hit names no bone, so the body class's DefaultHitRegion is used (the bone lookup is not exercised; no head/crit region)",
        )?;
    } else {
        for entry in access::array(vm, body, "HitRegionList")? {
            let Some(region) = object_of(&entry) else {
                continue;
            };
            let region = loaded(vm, region)?;
            for name in access::array(vm, region, "RegionBoneNames")? {
                let text = match name {
                    Value::Name(n) => vm.space.names.text(n).to_ascii_lowercase(),
                    _ => continue,
                };
                if text == bone {
                    return Ok(Some(region));
                }
            }
        }
    }
    match object(vm, body, "DefaultHitRegion")? {
        Some(r) => Ok(Some(loaded(vm, r)?)),
        None => Ok(None),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The pipeline objects (NATIVE_COMBAT_GROUNDWORK.md section 3.1)
// ---------------------------------------------------------------------------------------------------------------

/// The first pool entry that is free, made again from the class defaults (the reset), else a new one; the four
/// fields `GetDamagePipeline` fills are set.
pub fn acquire_pipeline(
    vm: &mut Vm,
    damage_type: Value,
    impact: Value,
    friendly: bool,
    hit_object: Value,
) -> Result<ObjRef, VmError> {
    let class = vm.find_class("WillowGame.WillowDamagePipeline")?;
    let made = instantiate_from_defaults(vm, class)?;
    for (name, v) in [
        ("DamageTypeDef", damage_type),
        ("ImpactDefinition", impact),
        ("bCanHitFriendly", Value::Bool(friendly)),
        ("DirectHitObject", hit_object),
    ] {
        set_required(vm, made, name, v, "combat hit")?;
    }
    with_state(vm, |s| match s.pipelines.iter_mut().find(|p| !p.1) {
        Some(slot) => *slot = (made, true),
        None => s.pipelines.push((made, true)),
    })?;
    Ok(made)
}

/// Marks the pipeline free again.
pub fn release_pipeline(vm: &Vm, pipeline: ObjRef) -> Result<(), VmError> {
    with_state(vm, |s| {
        for p in s.pipelines.iter_mut().filter(|p| p.0 == pipeline) {
            p.1 = false;
        }
    })
}

/// How many pipeline objects the pool holds, and how many are in use (for the tests).
pub fn pipeline_counts(vm: &Vm) -> Result<(usize, usize), VmError> {
    with_state(vm, |s| {
        (
            s.pipelines.len(),
            s.pipelines.iter().filter(|p| p.1).count(),
        )
    })
}

// ---------------------------------------------------------------------------------------------------------------
// The natives of the hit chain
// ---------------------------------------------------------------------------------------------------------------

/// The presentation the hit chain reaches that needs no state in the VM (no animation system, no HUD, no impact
/// effects, no AI targeting): bound no-ops, as the C++ binds them.
pub const PRESENTATION_NO_OPS: [&str; 7] = [
    "WillowGame.WillowPawn.SetupImpactsForPlayHit",
    "WillowGame.WillowPawn.UpdatePlayHitEffectCounters",
    "WillowGame.WillowPawn.UpdatePlayHitInfo",
    "WillowGame.WillowPawn.StartCringe",
    "WillowGame.WillowPawn.InterpRotation",
    "WillowGame.WillowPlayerController.NotifyInstinctSkillAction",
    "WillowGame.WillowPlayerController.NotifyShotStatus",
];

/// The C++ stand-ins for `SpecialMoveComponent.Play` (a no-op), `IsPlayingClass` and `IsPlayingAny` (false: the VM
/// plays no special move, true of this world, not a game rule), bound in `ParityMode::Reference` only (R3; reviews r1
/// and r2 of #437). In Off, `ow-skills`' `action_skill::special_move` binds `Play` and `IsPlayingClass` from
/// NATIVE_SPECIAL_MOVE_PLAY.md; `IsPlayingAny` is not read there, and a world without that pack leaves all three
/// unbound, so the stub policy answers.
pub const SPECIAL_MOVE_STAND_INS: [(&str, bool); 6] = [
    ("WillowGame.SpecialMoveComponent.Play", false),
    ("GearboxFramework.SpecialMoveComponent.Play", false),
    ("WillowGame.SpecialMoveComponent.IsPlayingClass", true),
    ("GearboxFramework.SpecialMoveComponent.IsPlayingClass", true),
    ("WillowGame.SpecialMoveComponent.IsPlayingAny", true),
    ("GearboxFramework.SpecialMoveComponent.IsPlayingAny", true),
];

/// The allegiance natives, the one registration every world uses (the hit pack, and the cast's worlds through
/// `ow-skills`' action skill pack), by path with the first binding of a key winning. UNVERIFIED.
/// - Off: `Pawn.GetOpinion`, `WillowPawn.GetOpinion` and `Pawn.IsFriendly` are NATIVE_IS_FRIENDLY.md's rules
///   ([`opinion::register`], lane IMPL-IS-FRIENDLY, decision 2026-11-19-60), bound first;
/// - Reference: `GetOpinion` and `IsFriendly` are the C++ stand-in, [`opinion`]'s four-step rule of
///   NATIVE_COMBAT_GROUNDWORK.md section 6 (a controller read through its pawn, no charm layer);
/// - both modes: `IsEnemy` and `IsNeutral` compare [`opinion`] with the opinion enum (Enemy 0, Neutral 1, Friendly 2).
pub fn register_allegiance(vm: &mut Vm) -> Result<(), VmError> {
    // Off: `GetOpinion`, `WillowPawn.GetOpinion` and `IsFriendly` are NATIVE_IS_FRIENDLY.md's (`opinion`, lane
    // IMPL-IS-FRIENDLY), bound first, so no world gets the groundwork boolean for them in Off.
    opinion::register(vm)?;
    let mut tests = vec![("Engine.Pawn.IsEnemy", 0), ("Engine.Pawn.IsNeutral", 1)];
    if vm.parity == ParityMode::Reference {
        // The C++ stand-in (Reference only): reads a controller through its pawn and has no charm layer.
        bind_fn(vm, "Engine.Pawn.GetOpinion", |c| {
            let me = this(c, "GetOpinion")?;
            let o = opinion(c.vm, me, arg_object(c, 0)?)?;
            // The opinion enum holds 0..=2 here; a byte keeps the low 8 bits as the C++ makeByte does.
            Ok(Value::Byte(access::low_u8(o)))
        })?;
        tests.push(("Engine.Pawn.IsFriendly", 2));
    }
    // `IsEnemy` and `IsNeutral` keep the groundwork rule in both modes: NATIVE_IS_FRIENDLY.md does not read
    // `WillowAIPawn`'s replacements of them.
    for (path, wanted) in tests {
        bind_fn(vm, path, move |c| {
            let me = this(c, "the opinion tests")?;
            Ok(Value::Bool(opinion(c.vm, me, arg_object(c, 0)?)? == wanted))
        })?;
    }
    Ok(())
}

fn register_hit(vm: &mut Vm) -> Result<(), VmError> {
    // Actor.GetDamagePipeline(InDamageTypeDef, InImpactDef, inCanHitFriendly, InHitObject) and the three small ones
    // (section 3.1). None for a missing argument.
    bind_fn(vm, "Engine.Actor.GetDamagePipeline", |c| {
        let arg = |c: &NativeCall<'_>, i: usize| -> Result<Value, VmError> {
            Ok(if c.has(i) {
                c.arg(i)?.clone()
            } else {
                Value::Object(None)
            })
        };
        let (t, i, h) = (arg(c, 0)?, arg(c, 1)?, arg(c, 3)?);
        let friendly = c.has(2) && arg_truth(c, 2)?;
        let p = acquire_pipeline(c.vm, t, i, friendly, h)?;
        note(c.vm, "GetDamagePipeline")?;
        Ok(Value::Object(Some(p)))
    })?;
    bind_fn(vm, "Engine.Actor.ReleaseDamagePipeline", |c| {
        if let Some(p) = arg_object(c, 0)? {
            release_pipeline(c.vm, p)?;
        }
        note(c.vm, "ReleaseDamagePipeline")?;
        Ok(Value::None)
    })?;
    for (path, field_name) in [
        ("Engine.Actor.GetPipelineDamageType", "DamageTypeDef"),
        (
            "Engine.Actor.GetPipelineImpactDefinition",
            "ImpactDefinition",
        ),
    ] {
        bind_fn(vm, path, move |c| match arg_object(c, 0)? {
            Some(p) => required(c.vm, p, field_name, "combat hit"),
            None => Ok(Value::Object(None)),
        })?;
    }

    // Pawn.PawnCheckTakeDamagePreconditions (DD section 3): true aborts. Damage proceeds only with authority and
    // health above 0 or a pawn that can be injured; the pawn-level virtual the note read as a dead/blocked test was
    // not named and is taken as false (UNVERIFIED, the C++ reading).
    bind_fn(vm, "Engine.Pawn.PawnCheckTakeDamagePreconditions", |c| {
        let me = this(c, "PawnCheckTakeDamagePreconditions")?;
        let health = call_method(c.vm, me, "GetHealth", Vec::new())?.map_or(0.0, |v| number(&v));
        let injurable = get(c.vm, me, "bCanBeInjured")?.is_some_and(|v| truth(c.vm, &v));
        let proceeds = is_authority(c.vm, me)? && (health > 0.0 || injurable);
        Ok(Value::Bool(!proceeds))
    })?;

    // Actor.ActorTakeDamageInner(DamageAmount, EventInstigator, HitLocation, DamageType, Pipeline) (DD section 2):
    // the Kismet TakeDamage events of the actor are not in the VM; MostRecentDamageTaken is stored.
    bind_fn(vm, "Engine.Actor.ActorTakeDamageInner", |c| {
        let me = this(c, "ActorTakeDamageInner")?;
        let damage = arg_number(c, 0)?;
        set(
            c.vm,
            me,
            "MostRecentDamageTaken",
            Value::Float(access::to_f32(damage)),
        )?;
        note(c.vm, format!("ActorTakeDamageInner damage={}", g5(damage)))?;
        Ok(Value::None)
    })?;

    // DamageTypeDefinition.RecordRecentDamage(out RecentDamage, Damage, DamageInstigator, DamageTypeDefinition,
    // bWasShieldDamage) (DD section 6): the damage added to the tracker's total and to its shield or health part.
    bind_fn(vm, "Engine.DamageTypeDefinition.RecordRecentDamage", |c| {
        let mut tracker = c.arg(0)?.clone();
        let damage = access::to_f32(arg_number(c, 1)?);
        let shield = c.has(4) && arg_truth(c, 4)?;
        for name in [
            "TotalDamage",
            if shield {
                "ShieldDamage"
            } else {
                "HealthDamage"
            },
        ] {
            if let Some(v) = field(c.vm, &tracker, name) {
                set_field(
                    c.vm,
                    &mut tracker,
                    name,
                    Value::Float(access::to_f32(number(&v)) + damage),
                );
            }
        }
        c.set_out(0, tracker)?;
        note(
            c.vm,
            format!("RecordRecentDamage damage={}", g5(f64::from(damage))),
        )?;
        Ok(Value::None)
    })?;

    bind_fn(
        vm,
        "WillowGame.WillowDamagePipeline.AdjustDamage",
        crate::damage::adjust_damage,
    )?;

    // Pawn.NotifyTakeHit(InstigatedBy, HitLocation, Damage, DamageType, Momentum, Pipeline) (DD section 5): the pawn's
    // controller raises OnTakeDamage for a pawn with an AI class that has an AI definition. The raise belongs to the
    // owner of the behavior providers ([`CombatState::on_take_damage`]).
    bind_fn(vm, "Engine.Pawn.NotifyTakeHit", |c| {
        let pawn = this(c, "NotifyTakeHit")?;
        let instigator = arg_object(c, 0)?;
        let damage = arg_number(c, 2)?;
        let source = match c.arg(3)? {
            Value::Class(k) => *k,
            _ => None,
        };
        let mut damage_type = None;
        let mut shield = 0.0;
        if c.has(5)
            && let Some(p) = arg_object(c, 5)?
        {
            damage_type = object(c.vm, p, "DamageTypeDef")?;
            if let Some(summary) = get(c.vm, p, "DamageSummary")?
                && let Some(v) = field(c.vm, &summary, "DamageDealtToShields")
            {
                shield = number(&v);
            }
        }
        note(
            c.vm,
            format!("NotifyTakeHit damage={} shield={}", g5(damage), g5(shield)),
        )?;
        let has_definition = match object(c.vm, pawn, "AIClass")? {
            Some(class) => object(c.vm, class, "AIDef")?.is_some(),
            None => false,
        };
        let hook = with_state(c.vm, |s| s.on_take_damage.clone())?;
        if let (Some(hook), true) = (hook, has_definition) {
            let event = TakeDamageEvent {
                pawn,
                instigator,
                damage_type,
                damage,
                shield_damage: shield,
                source,
            };
            hook(c.vm, &event)?;
        }
        Ok(Value::None)
    })?;

    register_allegiance(vm)?;
    // WillowPawn.GetHitRegionForTakenDamage(Instigator, HitInfo): the region lookup of AdjustDamage's stage 6.
    bind_fn(
        vm,
        "WillowGame.WillowPawn.GetHitRegionForTakenDamage",
        |c| {
            let me = this(c, "GetHitRegionForTakenDamage")?;
            let info = if c.arg_count() > 1 {
                c.arg(1)?.clone()
            } else {
                Value::None
            };
            Ok(Value::Object(hit_region(c.vm, me, &info)?))
        },
    )?;
    for path in PRESENTATION_NO_OPS {
        bind_fn(vm, path, |_| Ok(Value::None))?;
    }
    if vm.parity == ParityMode::Reference {
        for (path, query) in SPECIAL_MOVE_STAND_INS {
            if query {
                bind_fn(vm, path, |_| Ok(Value::Bool(false)))?;
            } else {
                bind_fn(vm, path, |_| Ok(Value::None))?;
            }
        }
    }
    // Actor.SetPhysics(newPhysics): the C++ stand-in, the physics mode as the actor's Physics byte, Reference only
    // (R3). Off has NATIVE_ACTOR_SET_PHYSICS.md's rule (`ow_world::world::set_physics`, lane IMPL-ACTOR-SETPHYSICS),
    // which the world binds first; binding the stand-in in Off would win in a world that registers this pack before
    // the world's natives.
    if vm.parity == ParityMode::Reference {
        bind_fn(vm, "Engine.Actor.SetPhysics", |c| {
            let me = this(c, "SetPhysics")?;
            // A byte argument; the C++ makeByte keeps the low 8 bits.
            let mode = access::low_u8(integer(c.arg(0)?));
            set(c.vm, me, "Physics", Value::Byte(mode))?;
            Ok(Value::None)
        })?;
    }

    // Reference only (R3): the C++ stand-ins of the AI component's demigod hold and of GetAIComponent. The installed
    // Behavior_AIHold script calls HoldDemigod / ReleaseDemigod for its type 3; the C++ keeps the hold as a list of
    // reasons and DemigodOnHold answers "any hold" (UNVERIFIED), and GetAIComponent answers the mind's AIComponent
    // property. Off has NATIVE_AI_HOLDS.md's rule instead (`crate::holds`: name and tag pairs on `DemigodHolds` and
    // the other three lists; GetAIComponent the mind's own AIComponent by NATIVE_GEARBOXMIND_GETAICOMPONENT.md, the
    // same value as this stand-in, lane IMPL-GEARBOXMIND-GETAICOMPONENT). Decision 2026-11-16-83.
    if vm.parity == ParityMode::Reference {
        register_hold_stand_ins(vm)?;
    } else {
        crate::holds::register(vm)?;
    }
    // WillowMind.GetAIDemigod / SetAIDemigod: a plain flag.
    bind_fn(vm, "WillowGame.WillowMind.GetAIDemigod", |c| {
        let me = this(c, "GetAIDemigod")?;
        Ok(Value::Bool(with_state(c.vm, |s| {
            s.mind_demigod.get(&me).copied().unwrap_or(false)
        })?))
    })?;
    bind_fn(vm, "WillowGame.WillowMind.SetAIDemigod", |c| {
        let me = this(c, "SetAIDemigod")?;
        let on = c.has(0) && arg_truth(c, 0)?;
        with_state(c.vm, |s| s.mind_demigod.insert(me, on))?;
        Ok(Value::None)
    })
}

/// The C++ stand-ins of the demigod hold and of `GearboxMind.GetAIComponent` (Reference only; see `register_hit`).
fn register_hold_stand_ins(vm: &mut Vm) -> Result<(), VmError> {
    for owner in [
        "GearboxFramework.AIComponent",
        "WillowGame.WillowAIComponent",
    ] {
        bind_fn(vm, &format!("{owner}.HoldDemigod"), |c| {
            let me = this(c, "HoldDemigod")?;
            let reason = arg_text(c, 0)?;
            with_state(c.vm, |s| {
                s.demigod_holds.entry(me).or_default().push(reason.clone())
            })?;
            note(c.vm, format!("HoldDemigod reason={reason}"))?;
            Ok(Value::None)
        })?;
        bind_fn(vm, &format!("{owner}.ReleaseDemigod"), |c| {
            let me = this(c, "ReleaseDemigod")?;
            let reason = arg_text(c, 0)?;
            with_state(c.vm, |s| {
                let holds = s.demigod_holds.entry(me).or_default();
                if let Some(at) = holds.iter().position(|r| *r == reason) {
                    holds.remove(at);
                }
            })?;
            note(c.vm, format!("ReleaseDemigod reason={reason}"))?;
            Ok(Value::None)
        })?;
        bind_fn(vm, &format!("{owner}.DemigodOnHold"), |c| {
            let me = this(c, "DemigodOnHold")?;
            Ok(Value::Bool(with_state(c.vm, |s| {
                s.demigod_holds.get(&me).is_some_and(|h| !h.is_empty())
            })?))
        })?;
    }
    // IAIInterface.GetAIComponent on the mind (GearboxMind): its AIComponent property.
    bind_fn(vm, "GearboxFramework.GearboxMind.GetAIComponent", |c| {
        let me = this(c, "GetAIComponent")?;
        required(c.vm, me, "AIComponent", "combat hit")
    })
}

/// Whether `actor`'s `Role` is `ROLE_Authority`.
pub fn is_authority(vm: &mut Vm, actor: ObjRef) -> Result<bool, VmError> {
    let authority = with_state(vm, |s| s.authority)?;
    Ok(get(vm, actor, "Role")?.is_some_and(|r| integer(&r) == i64::from(authority)))
}

/// The world's `TimeSeconds` (0 without a world).
pub fn world_time(vm: &mut Vm) -> Result<f32, VmError> {
    let Some(world) = with_state(vm, |s| s.world)? else {
        return Ok(0.0);
    };
    Ok(get(vm, world, "TimeSeconds")?.map_or(0.0, |v| access::to_f32(number(&v))))
}

// ---------------------------------------------------------------------------------------------------------------
// The hit
// ---------------------------------------------------------------------------------------------------------------

/// What one hit did, for the reports (C++ `HitReport`).
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HitReport {
    pub health_before: f64,
    pub health_after: f64,
    pub arriving: f64,
    pub applied: f64,
    pub shield_damage: f64,
    pub demigod_before: bool,
    pub demigod_after: bool,
    /// The natives' events of this hit, in order.
    pub events: Vec<String>,
    /// Every function the chain called (script and native), in call order.
    pub order: Vec<String>,
}

/// `Pawn.IsDemigodMode` as the installed script answers it (false for a class without it).
pub fn demigod(vm: &mut Vm, pawn: ObjRef) -> Result<bool, VmError> {
    Ok(call_method(vm, pawn, "IsDemigodMode", Vec::new())?.is_some_and(|v| truth(vm, &v)))
}

/// The census lines of one counter, as the C++ prints them (`name`, ` xN` when more than once, in key order).
fn census_line(counts: &DetMap<String, u64>) -> String {
    counts
        .iter()
        .filter(|(_, n)| **n > 0)
        .map(|(name, n)| {
            if *n > 1 {
                format!("{name} x{n}")
            } else {
                name.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// A hit on `victim` by `instigator` (a controller, or an AI mind for a blow) with the damage type definition
/// `damage_type` and the damage source class `source`: runs the victim's script `TakeDamage` with the arguments by
/// their declared names (no location, momentum, bone or causer: each the zero value of its declared type) and a
/// pipeline carrying the damage type, released after the chain (C++ `hitWithType`).
pub fn hit_with_type(
    vm: &mut Vm,
    victim: ObjRef,
    instigator: Option<ObjRef>,
    damage: f64,
    damage_type: Option<ObjRef>,
    source: Option<ClassId>,
) -> Result<HitReport, VmError> {
    hit_with(
        vm,
        victim,
        instigator,
        damage,
        damage_type,
        source,
        ShotParts::default(),
    )
}

/// What a weapon's shot adds to a hit ([`shot`]; lane FIX-FIRE-TO-HIT, decision 2026-11-19-06): the impact
/// definition the pipeline carries and the damage causer (the weapon). `Default` is a plain hit: neither.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ShotParts {
    pub impact: Option<ObjRef>,
    pub causer: Option<ObjRef>,
}

impl ShotParts {
    pub fn new(impact: Option<ObjRef>, causer: Option<ObjRef>) -> ShotParts {
        ShotParts { impact, causer }
    }
}

/// The `DamageCauser` argument for `causer`: the declared parameter's kind (`IDamageCauser` in the installed class, an
/// object in some fixtures); any other kind gets its zero value, as a plain hit leaves it (review r1 of #393).
pub(crate) fn causer_arg(
    vm: &Vm,
    kind: &ow_object::layout::PropKind,
    causer: Option<ObjRef>,
) -> Value {
    match kind {
        ow_object::layout::PropKind::Interface { .. } => Value::Interface(causer),
        ow_object::layout::PropKind::Object { .. } => Value::Object(causer),
        other => vm.space.classes.zero_value(other),
    }
}

/// [`hit_with_type`] with a shot's parts: the pipeline carries `parts.impact`, and a declared `DamageCauser` gets
/// `parts.causer` (the zero value without one). With [`ShotParts::default`] this is exactly [`hit_with_type`].
/// The kill boundary (review r1 of #481, item 2): an error that leaves the hit after the victim's script state changed
/// (the death began) is past the hit's and the death script's writes. A `NotYet` is then marked once, its kind and
/// record kept; any other error, and any error with the state unchanged, is returned as it is.
pub fn kill_boundary<S: PartialEq>(e: VmError, before: Option<S>, after: Option<S>) -> VmError {
    match e {
        VmError::NotYet(n) if before != after => VmError::NotYet(n.after_partial_writes()),
        other => other,
    }
}

pub fn hit_with(
    vm: &mut Vm,
    victim: ObjRef,
    instigator: Option<ObjRef>,
    damage: f64,
    damage_type: Option<ObjRef>,
    source: Option<ClassId>,
    parts: ShotParts,
) -> Result<HitReport, VmError> {
    let mut report = HitReport::default();
    let mark = with_state(vm, |s| s.events.len())?;
    let type_path = path_of(vm, damage_type);
    let class = vm.space.heap.get(victim)?.class;
    let take = vm.find_method(class, "TakeDamage", None)?;
    let health = vm.find_method(class, "GetHealth", None)?;
    let (Some(take), Some(health)) = (take, health) else {
        return Err(error(
            "combat hit: the victim's class has no TakeDamage / GetHealth",
        ));
    };
    report.health_before = number(&vm.call(health, Some(victim), Vec::new())?.ret);
    report.demigod_before = demigod(vm, victim)?;
    report.arriving = damage;

    let pipeline = acquire_pipeline(
        vm,
        Value::Object(damage_type),
        Value::Object(parts.impact),
        false,
        Value::Object(None),
    )?;
    let params = vm
        .function(take)
        .map(|d| d.params.clone())
        .unwrap_or_default();
    let mut args = Vec::with_capacity(params.len());
    for p in &params {
        let name = vm.space.names.text(p.name).to_ascii_lowercase();
        args.push(match name.as_str() {
            // N5: the step's damage is a finite double; the script's Damage is a float (round to nearest).
            "damage" => Value::Float(access::to_f32(damage)),
            "instigatedby" => Value::Object(instigator),
            "damagetype" => Value::Class(source),
            "pipeline" => Value::Object(Some(pipeline)),
            "damagecauser" if parts.causer.is_some() => causer_arg(vm, &p.kind, parts.causer),
            _ => vm.space.classes.zero_value(&p.kind),
        });
    }
    let source_name = match source {
        Some(k) => vm
            .space
            .classes
            .get_layout(k)
            .map(|l| l.path().name().to_string())
            .unwrap_or_default(),
        None => "none".to_string(),
    };
    note(
        vm,
        format!(
            "TakeDamage damage={} type={type_path} source={source_name}",
            g5(damage)
        ),
    )?;
    if parts != ShotParts::default() {
        let (impact, causer) = (path_of(vm, parts.impact), access::label_of(vm, parts.causer));
        note(vm, format!("TakeDamage shot impact={impact} causer={causer}"))?;
    }

    let outer_census = vm.census.replace(CallCensus::default());
    let outer_trace = vm.trace.replace(Vec::new());
    let state_before = vm.space.heap.get(victim)?.state;
    let ran = vm.call(take, Some(victim), args);
    let census = std::mem::replace(&mut vm.census, outer_census).unwrap_or_default();
    report.order = std::mem::replace(&mut vm.trace, outer_trace).unwrap_or_default();
    // The kill boundary (review r1 of #481, item 2): a NotYet that leaves the hit after the victim's death began (its
    // script state changed, as `Pawn.Died` moves it to `Dying`) comes after the hit's and the death script's writes.
    // It is terminal: marked once, its kind and record kept, carried typed to the caller. Other errors are unchanged.
    let state_after = vm.space.heap.get(victim).map(|o| o.state).ok();
    // Off only (review r2 of #481, M2): Reference keeps the C++ `Run::apply`, which has no such mark.
    let ran = if vm.parity == ParityMode::Off { ran.map_err(|e| kill_boundary(e, Some(state_before), state_after)) } else { ran };
    ran?;
    note(
        vm,
        format!(
            "scripts entered (census): {}",
            census_line(&census.script_calls)
        ),
    )?;
    note(
        vm,
        format!(
            "natives run (census): {}",
            census_line(&census.native_calls)
        ),
    )?;
    note(
        vm,
        format!(
            "calls on None (census): {}",
            census_line(&census.none_contexts)
        ),
    )?;
    if let Some(summary) = get(vm, pipeline, "DamageSummary")? {
        if let Some(v) = field(vm, &summary, "FinalDamage") {
            report.applied = number(&v);
        }
        if let Some(v) = field(vm, &summary, "DamageDealtToShields") {
            report.shield_damage = number(&v);
        }
    }
    release_pipeline(vm, pipeline)?;
    report.health_after = number(&vm.call(health, Some(victim), Vec::new())?.ret);
    report.demigod_after = demigod(vm, victim)?;
    report.events = with_state(vm, |s| s.events[mark..].to_vec())?;
    Ok(report)
}

/// [`hit_with_type`] with the damage type definition given as a stock object path (`""` is None).
pub fn hit(
    vm: &mut Vm,
    victim: ObjRef,
    instigator: Option<ObjRef>,
    damage: f64,
    damage_type_path: &str,
    source: Option<ClassId>,
) -> Result<HitReport, VmError> {
    let damage_type = if damage_type_path.is_empty() {
        None
    } else {
        Some(stock_object(vm, damage_type_path)?)
    };
    hit_with_type(vm, victim, instigator, damage, damage_type, source)
}

// ---------------------------------------------------------------------------------------------------------------
// The player and the AI pawn
// ---------------------------------------------------------------------------------------------------------------

/// The player-controlled attacker of a run: a `WillowPlayerController` with a `PlayerReplicationInfo` at `level` and
/// its `WillowPlayerPawn`.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Player {
    pub controller: ObjRef,
    pub pawn: ObjRef,
}

/// Makes a [`Player`] (C++ `spawnPlayer`). The game's replication info lists the players
/// (`AwardCombatExperienceToParty` walks it): this controller's info, owned by the controller, is appended to the
/// world's `GRI.PRIArray` when the world has one.
pub fn spawn_player(vm: &mut Vm, level: i32) -> Result<Player, VmError> {
    let unit = "combat hit";
    let controller_class = vm.find_class("WillowGame.WillowPlayerController")?;
    let pawn_class = vm.find_class("WillowGame.WillowPlayerPawn")?;
    let info_class = vm.find_class("WillowGame.WillowPlayerReplicationInfo")?;
    let controller = instantiate_from_defaults(vm, controller_class)?;
    let pawn = instantiate_from_defaults(vm, pawn_class)?;
    let info = instantiate_from_defaults(vm, info_class)?;
    set_required(vm, info, "ExpLevel", Value::Int(level), unit)?;
    let (authority, world) = with_state(vm, |s| (s.authority, s.world))?;
    for actor in [controller, pawn] {
        set_required(vm, actor, "Role", Value::Byte(authority), unit)?;
        set_required(vm, actor, "WorldInfo", Value::Object(world), unit)?;
    }
    set_required(
        vm,
        controller,
        "PlayerReplicationInfo",
        Value::Object(Some(info)),
        unit,
    )?;
    set_required(vm, controller, "Pawn", Value::Object(Some(pawn)), unit)?;
    set_required(
        vm,
        pawn,
        "Controller",
        Value::Object(Some(controller)),
        unit,
    )?;
    with_state(vm, |s| s.players.push(controller))?;
    if let Some(world) = world
        && let Some(gri) = object(vm, world, "GRI")?
        && let Some(list) = get(vm, gri, "PRIArray")?
    {
        set_required(vm, info, "Owner", Value::Object(Some(controller)), unit)?;
        let mut items = list.elements().map(<[Value]>::to_vec).unwrap_or_default();
        items.push(Value::Object(Some(info)));
        set(vm, gri, "PRIArray", Value::array(items))?;
    }
    Ok(Player { controller, pawn })
}

/// An AI pawn and its mind.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Enemy {
    pub pawn: ObjRef,
    pub mind: ObjRef,
}

/// A new run-time copy of the definition `archetype` (its class and its loaded values; C++ `instantiateExport`):
/// the archetype itself is never written.
pub fn copy_of(vm: &mut Vm, archetype: ObjRef) -> Result<ObjRef, VmError> {
    let archetype = loaded(vm, archetype)?;
    let class = vm.space.heap.get(archetype)?.class;
    let name = access::name_of(vm, archetype);
    let made = vm.instantiate(class, &name)?;
    let slots = vm.space.heap.get(archetype)?.slots.clone();
    vm.space.heap.get_mut(made)?.slots = slots;
    Ok(made)
}

/// The deepest value nesting [`relink`] follows (a Rust-only bound; archetype values nest a few levels).
pub const RELINK_DEPTH: u32 = 16;

/// `value` with every resource stand-in it holds (an object the decoder left as a reference, `Origin::Resource`)
/// replaced by the definition store's object of the same path; nested structs and arrays are followed to
/// [`RELINK_DEPTH`] levels. A stand-in the store resolves to no definition, and a value nested deeper than the bound:
/// `ParityMode::Reference` keeps them as they are (the C++ keeps its resource stand-ins); `ParityMode::Off` refuses
/// them, naming the path, so nothing reads an unresolved reference as an empty object.
fn relink_value(vm: &mut Vm, value: &Value, depth: u32) -> Result<Value, VmError> {
    if depth > RELINK_DEPTH {
        if vm.parity == ParityMode::Off {
            return Err(error(format!(
                "combat: an archetype value nests deeper than {RELINK_DEPTH} levels (its references are not relinked)"
            )));
        }
        return Ok(value.clone());
    }
    Ok(match value {
        Value::Object(Some(o)) => {
            let stand_in = matches!(
                vm.space.heap.get(*o)?.origin,
                ow_object::heap::Origin::Resource { .. }
            );
            if !stand_in {
                return Ok(value.clone());
            }
            let path = path_of(vm, Some(*o));
            let found = match ow_core::ObjectPath::parse(&path) {
                Ok(parsed) => {
                    let r = vm.defs.find(&mut vm.space, &parsed);
                    vm.log.extend(vm.defs.take_log());
                    r
                }
                Err(e) => Err(ow_defs::store::DefError::NotADefinition(format!(
                    "{path}: {e}"
                ))),
            };
            match (found, vm.parity) {
                (Ok(Some(d)), _) => Value::Object(Some(d)),
                (_, ParityMode::Reference) => value.clone(),
                (Ok(None), ParityMode::Off) => {
                    return Err(error(format!(
                        "combat: the archetype's reference {path} resolves to no definition in the store"
                    )));
                }
                (Err(e), ParityMode::Off) => {
                    return Err(error(format!(
                        "combat: the archetype's reference {path} does not load: {e}"
                    )));
                }
            }
        }
        Value::Struct(s) => {
            let mut fields = Vec::with_capacity(s.fields.len());
            for f in &s.fields {
                fields.push(relink_value(vm, f, depth + 1)?);
            }
            Value::structure(s.ty, fields)
        }
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for v in items.iter() {
                out.push(relink_value(vm, v, depth + 1)?);
            }
            Value::array(out)
        }
        other => other.clone(),
    })
}

/// A new object instantiated from export `archetype_path` of package `package` with the C++ 4-byte property prefix
/// (C++ `instantiateExport(package, index, 4)`): the export's class defaults and its own tags, read from that file,
/// its references then relinked to the definition store's objects by path ([`relink_value`]). `None` when the
/// package does not hold the path.
///
/// Why not the definition store: the store derives a 26-byte prefix for an `Actor` subclass export outside a level
/// and refuses one whose tags do not parse there, which is every pawn archetype of the installed game (checked
/// locally on `Pawn_TargetDummy` and `Pawn_PrimalBeast`; reported in the lane's handoff as a W3-DEFS item). A copy
/// is also what the C++ makes: the archetype itself is never written.
pub fn instantiate_copy(
    vm: &mut Vm,
    package: &str,
    archetype_path: &str,
) -> Result<Option<ObjRef>, VmError> {
    let Ok(name) = ow_core::PackageName::parse(package) else {
        return Ok(None);
    };
    let mode = vm.parity;
    let made = {
        let (code, store) = vm.defs.code_mut();
        let id = store
            .load(&name)
            .map_err(|e| error(format!("combat: {e}")))?;
        let Some(index) = store
            .get(id)
            .find_export(archetype_path)
            .ok()
            .flatten()
            .filter(|i| *i > 0)
        else {
            return Ok(None);
        };
        let mut state = ow_object::decode::DecodeState::new();
        let mut cx = ow_object::decode::ValueCx::new(store, code, &mut vm.space, &mut state, mode);
        ow_object::cdo::instantiate_export(
            &mut cx,
            id,
            index,
            ow_object::prefix::PropertyPrefix::Ordinary,
        )
        .map_err(|e| VmError::Decode(e.to_string()))?
        .object
    };
    let slots = vm.space.heap.get(made)?.slots.clone();
    let mut linked = Vec::with_capacity(slots.len());
    for v in &slots {
        linked.push(relink_value(vm, v, 0)?);
    }
    vm.space.heap.get_mut(made)?.slots = linked;
    Ok(Some(made))
}

/// The stock archetype `archetype_path` of package `package` as an AI pawn at `level` and `game_stage` (C++
/// `spawnEnemy`): a copy of the export ([`instantiate_copy`]) given the rest by [`adopt`].
pub fn spawn_enemy(
    vm: &mut Vm,
    package: &str,
    archetype_path: &str,
    level: i32,
    game_stage: i32,
) -> Result<Enemy, VmError> {
    let Some(pawn) = instantiate_copy(vm, package, archetype_path)? else {
        return Err(error(format!(
            "combat: no such pawn archetype: {archetype_path}"
        )));
    };
    if vm.parity == ParityMode::Off {
        // The archetype record (lane IMPL-SETDEFAULTPHYSICS-AND-MIND-RECALC): a second copy, never written, stands for
        // the archetype object the spawn copied (the store does not load pawn archetypes; `instantiate_copy`).
        if let Some(archetype) = instantiate_copy(vm, package, archetype_path)? {
            with_state(vm, |s| s.archetypes.insert(pawn, archetype))?;
        }
        crate::spawn_balance::pick(vm, pawn, package, archetype_path)?;
    }
    adopt(vm, pawn, level, game_stage)
}

/// Gives a pawn what `spawn_enemy` builds around an archetype (C++ `adopt`): authority, the world, its level and
/// game stage, a `WillowMind` of its AI class with a `WillowAIComponent` (when the mind's class declares
/// `AIComponent`) and the manager, the experience multiplier (`crate::xp`), the death preparation once deaths are on
/// (`crate::death`), and the health pool, made by the pawn's own `SetHealthResourcePoolDefinition` over the class's
/// `HealthPoolDefinition`. Idempotent: a pawn that already has a mind is returned as it is. Nothing else of the stock
/// spawn runs (UNVERIFIED as a model of it; NATIVE_COMBAT_GROUNDWORK.md sections 1.4 and 2).
pub fn adopt(vm: &mut Vm, pawn: ObjRef, level: i32, game_stage: i32) -> Result<Enemy, VmError> {
    let unit = "combat";
    if let Some(mind) = object(vm, pawn, "MyWillowMind")? {
        return Ok(Enemy { pawn, mind });
    }
    let (authority, world, deaths) = with_state(vm, |s| (s.authority, s.world, s.deaths))?;
    set_required(vm, pawn, "Role", Value::Byte(authority), unit)?;
    set_required(vm, pawn, "WorldInfo", Value::Object(world), unit)?;
    set_required(vm, pawn, "ExpLevel", Value::Int(level), unit)?;
    set_required(vm, pawn, "GameStage", Value::Int(game_stage), unit)?;
    let class = required(vm, pawn, "AIClass", unit)?;
    let Some(class) = object_of(&class) else {
        return Err(error("combat: the archetype has no AI class"));
    };
    let class = loaded(vm, class)?;
    let Some(definition) = object(vm, class, "HealthPoolDefinition")? else {
        return Err(error("combat: the AI class has no health pool definition"));
    };

    let mind_class = vm.find_class("WillowGame.WillowMind")?;
    let mind = instantiate_from_defaults(vm, mind_class)?;
    set_required(vm, mind, "Role", Value::Byte(authority), unit)?;
    set_required(vm, mind, "WorldInfo", Value::Object(world), unit)?;
    set_required(vm, mind, "AIClass", Value::Object(Some(class)), unit)?;
    set_required(vm, mind, "Pawn", Value::Object(Some(pawn)), unit)?;
    if get(vm, mind, "AIComponent")?.is_some()
        && let Ok(component_class) = vm.find_class("WillowGame.WillowAIComponent")
    {
        let component = instantiate_from_defaults(vm, component_class)?;
        set(vm, mind, "AIComponent", Value::Object(Some(component)))?;
    }
    set_required(vm, pawn, "Controller", Value::Object(Some(mind)), unit)?;
    set_required(vm, pawn, "MyWillowMind", Value::Object(Some(mind)), unit)?;
    crate::xp::apply_experience_multiplier(vm, pawn, mind)?;
    if deaths {
        crate::death::prepare_for_death(vm, pawn)?;
    }
    if vm.parity == ParityMode::Off {
        // Off: the stock route's class step, the mind's installed script `InitializeCharacterClass` (reached from
        // `WillowMind.Possess`): `ApplyCharacterClassStartingValues`, then `ApplyCharacterClassDefaults(pawn, class)`,
        // which makes the health pool (its `Controller` super call), applies the pawn's attribute starting values
        // (`WillowPawn.ApplyAttributeStartingValues`, NATIVE_NOUN_ATTRIBUTES.md sections 1 and 8, so the mind's nouns
        // such as `Attribute_HealthMultiplier` hold the class values) and recalculates. Read from the script; the rest
        // of `Possess` is not run (decision 2026-11-19-15). A native it reaches that nothing binds stops here.
        set_required(vm, mind, "MyWillowPawn", Value::Object(Some(pawn)), unit)?;
        // The hub's ruling on #796 (NATIVE_LINKS_13_14.md section 4, lane IMPL-LINKS-13-14; UNVERIFIED): `Possess`'s
        // one write the adopt now runs, the mind's `CharacterClass` := the pawn's `AIClass`, before
        // `InitializeCharacterClass`, so the recalculation's class-level part runs from the AI class. This narrows
        // decision 2026-11-19-26 by that write; the rest of `Possess` stays out.
        set_required(vm, mind, "CharacterClass", Value::Object(Some(class)), unit)?;
        vm.log.push(adopt_class_line(vm, class));
        let mind_class = vm.space.heap.get(mind)?.class;
        let Some(initialize) = vm.find_method(mind_class, "InitializeCharacterClass", None)? else {
            return Err(error("combat: the mind class has no InitializeCharacterClass"));
        };
        vm.call(initialize, Some(mind), Vec::new())?;
        return Ok(Enemy { pawn, mind });
    }
    let manager = required(vm, mind, "ResourcePoolManager", unit)?;
    let class_id = vm.space.heap.get(pawn)?.class;
    let Some(set_pool) = vm.find_method(class_id, "SetHealthResourcePoolDefinition", None)? else {
        return Err(error(
            "combat: the pawn class has no SetHealthResourcePoolDefinition",
        ));
    };
    let result = vm.call(
        set_pool,
        Some(pawn),
        vec![manager, Value::Object(Some(definition))],
    )?;
    let written = result
        .outs
        .first()
        .cloned()
        .ok_or_else(|| error("combat: SetHealthResourcePoolDefinition returned no manager"))?;
    set_required(vm, mind, "ResourcePoolManager", written, unit)?;
    Ok(Enemy { pawn, mind })
}

/// The harness line of the adopt's `CharacterClass` write (the hub's ruling on #796).
pub fn adopt_class_line(vm: &Vm, class: ObjRef) -> String {
    let named = vm.defs.handle_path(class).unwrap_or_else(|| crate::hit::access::class_name(vm, class));
    format!(
        "harness input: the AI adopt sets the mind's CharacterClass to the pawn's AIClass {named}, as WillowMind.Possess          does (hub ruling on #796; UNVERIFIED)"
    )
}

/// The object a hold behavior (the installed `Behavior_AIHold` script) is applied to for `context`: the live game
/// applies it to the pawn, and the script asks the context for `IAIInterface`, which the class tables list for the
/// mind and not for `WillowAIPawn`, so an AI pawn is answered by its mind (C++ `aiInterfaceFor`). A stand-in for the
/// table gap, UNVERIFIED; any other object is returned as it is. Reference only, and kept as the C++ has it (R3): Off
/// runs [`crate::holds::apply_hold_behavior`] with `Pawn`'s rule ([`crate::pawn_interface`]).
///
/// NATIVE_PAWN_AI_INTERFACE.md (section 5) agrees that the mind answers a possessed AI pawn, and differs from this
/// stand-in in what Off follows: the rule reads `Controller`, not `MyWillowMind`; the pawn's own class comes first;
/// a declining controller leads to the `Mesh`, not a vehicle; with no controller it asks a driven vehicle's controller,
/// then the `Mesh`, then answers none (the stand-in hands back the pawn); and the rule is every pawn class's, player
/// pawns included.
pub fn ai_interface_for(vm: &mut Vm, context: ObjRef) -> Result<ObjRef, VmError> {
    Ok(object(vm, context, "MyWillowMind")?.unwrap_or(context))
}

// ---------------------------------------------------------------------------------------------------------------
// A melee swing as a fixture (plan step S-e, docs/verification/COMBAT_MELEE.md)
// ---------------------------------------------------------------------------------------------------------------

/// What a definition delivers: the damage data, the damage type definition and the damage source class.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Delivery {
    pub damage: Value,
    pub damage_type: Option<ObjRef>,
    pub source: Option<ClassId>,
    pub what: String,
}

/// What a definition delivers when it hits (NATIVE_AI_MELEE.md, "Hit application"; C++ `deliveryOf`). A
/// `MeleeDefinition` carries all three itself (`Damage`, `DamageTypeDefinition`, `DamageSource`). A thrown projectile
/// (`Behavior_AIThrowProjectileAtTarget`) carries the damage as the `ProjectileDamage` entry of its
/// `ChildProjectileBaseValues`, and the projectile's `Behavior_CauseDamage` `cause_damage` names the type and the
/// source; its formula's scale multiplies the entry's (its constant is not applied, and noted). Read from the data;
/// the natives' bodies are unread, UNVERIFIED.
pub fn delivery_of(
    vm: &mut Vm,
    definition: ObjRef,
    cause_damage: Option<ObjRef>,
) -> Result<Delivery, VmError> {
    let unit = "combat swing";
    let definition = loaded(vm, definition)?;
    let what = path_of(vm, Some(definition));
    let mut delivery = Delivery {
        damage: Value::None,
        damage_type: None,
        source: None,
        what: what.clone(),
    };
    let mut holder = definition;
    if is_a(vm, definition, "WillowGame.MeleeDefinition")? {
        delivery.damage = required(vm, definition, "Damage", unit)?;
        if let Some(Value::Name(n)) = get(vm, definition, "DamageTypeDefinitionOverride")? {
            let named = vm.space.names.text(n).into_owned();
            if !named.is_empty() && named != "None" {
                note(
                    vm,
                    format!(
                        "note: {what} names a damage type override ({named}), which is not read; DamageTypeDefinition is used"
                    ),
                )?;
            }
        }
    } else if is_a(
        vm,
        definition,
        "WillowGame.Behavior_AIThrowProjectileAtTarget",
    )? {
        let Some(cause) = cause_damage else {
            return Err(error(
                "combat swing: a thrown projectile needs the projectile's Behavior_CauseDamage (the second definition)",
            ));
        };
        let cause = loaded(vm, cause)?;
        let entries = required(vm, definition, "ChildProjectileBaseValues", unit)?;
        let mut found = None;
        for entry in entries.elements().unwrap_or(&[]) {
            let attribute = field(vm, entry, "Attribute").as_ref().and_then(object_of);
            if path_of(vm, attribute).ends_with(".ProjectileDamage") {
                found = field(vm, entry, "BaseValue");
                break;
            }
        }
        let Some(mut damage) = found else {
            return Err(error(format!(
                "combat swing: {what} has no ProjectileDamage entry"
            )));
        };
        let formula = required(vm, cause, "DamageFormula", unit)?;
        if field(vm, &formula, "BaseValueConstant").is_some_and(|v| number(&v) != 0.0) {
            let cause_path = path_of(vm, Some(cause));
            note(
                vm,
                format!("note: {cause_path} has a damage formula constant, which is not applied"),
            )?;
        }
        if let Some(scale) = field(vm, &damage, "BaseValueScaleConstant") {
            let by = field(vm, &formula, "BaseValueScaleConstant").map_or(0.0, |v| number(&v));
            set_field(
                vm,
                &mut damage,
                "BaseValueScaleConstant",
                Value::Float(access::to_f32(number(&scale)) * access::to_f32(by)),
            );
        }
        delivery.damage = damage;
        delivery.what = format!("{what} + {}", path_of(vm, Some(cause)));
        holder = cause;
    } else {
        return Err(error(format!(
            "combat swing: {what} is neither a MeleeDefinition nor a Behavior_AIThrowProjectileAtTarget"
        )));
    }
    delivery.source = match get(vm, holder, "DamageSource")? {
        Some(Value::Class(k)) => k,
        _ => None,
    };
    if delivery.source.is_none() {
        return Err(error(format!(
            "combat swing: {} names no damage source class",
            delivery.what
        )));
    }
    let named = required(vm, holder, "DamageTypeDefinition", unit)?;
    let Some(t) = object_of(&named) else {
        return Err(error("combat: unresolved object reference"));
    };
    delivery.damage_type = Some(loaded(vm, t)?);
    Ok(delivery)
}

/// One blow (C++ `Swing`): the arrival, the source's class name, the damage type's path and the hit.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Swing {
    pub arrival: f64,
    pub source: String,
    pub damage_type: String,
    pub hit: HitReport,
}

/// The attacker's blow lands (the fixture: no hit volume, no cone, no allegiance test, no momentum; it always
/// connects): the damage evaluated with `mind` as the context source (NATIVE_AI_MELEE.md, "Damage amount"),
/// delivered to `victim` through its stock `TakeDamage` with the mind as the instigator.
pub fn swing(
    vm: &mut Vm,
    mind: ObjRef,
    victim: ObjRef,
    delivery: &Delivery,
) -> Result<Swing, VmError> {
    let arrival = f64::from(evaluate_for(vm, &delivery.damage, Some(mind))?);
    let source = match delivery.source {
        Some(k) => vm
            .space
            .classes
            .get_layout(k)
            .map(|l| l.path().name().to_string())
            .unwrap_or_default(),
        None => String::new(),
    };
    let damage_type = path_of(vm, delivery.damage_type);
    let mut hit = hit_with_type(
        vm,
        victim,
        Some(mind),
        arrival,
        delivery.damage_type,
        delivery.source,
    )?;
    hit.events.insert(
        0,
        format!(
            "swing {} arrival={} (fixture: always connects, no hit volume)",
            delivery.what,
            g5(arrival)
        ),
    );
    Ok(Swing {
        arrival,
        source,
        damage_type,
        hit,
    })
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod set_physics_pack_tests;
