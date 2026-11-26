//! Owner: lane W5-WORLD. [`World`]: a [`Vm`] with the world services installed, the world's natives ([`Pack`]) and
//! the tick (architecture section 3.9). AI-assisted (Claude Code).
//!
//! **What `World::new` installs** on the VM, each a VM service so native packs reach it from `&mut Vm`:
//! - [`crate::query::WorldQuery`] behind the VM's `WorldHooks` (globals, game info, engine queries);
//! - [`BalanceState`], the balance inputs and region stages (`ow-balance`, decision 2026-11-15-18), a separate
//!   service because `ow-vm` may not name `ow-balance` types;
//! - [`crate::actors::Actors`] (the level's actor list) and [`crate::timers::Timers`];
//! - an `ow_vm::natives_core::ActorSource` over the actor list, so `AllActors` and `DynamicActors` walk the level;
//! - [`Pack`], then the packs the caller passes, in order.
//!
//! **The tick** ([`World::tick`]), with the label each part has:
//! 1. `WorldInfo.TimeDilation` (the authoritative value) is copied into the clock; a value the clock refuses stops
//!    the tick with [`WorldError::Clock`] (the clock keeps its last good value). Then `SimClock::advance`, and
//!    `WorldInfo.TimeSeconds` and `RealTimeSeconds` are written from the clock, and `DeltaSeconds` with the clock's
//!    dilated delta where the class declares it (lane W5-MAPLOAD). UNVERIFIED: where and how the game advances them
//!    was not located (the clock is a placeholder, `ow_core::sim`).
//! 2. The actors are ticked by tick group (`TickingGroup` values 0 to 4 in order). Each actor's group is read once,
//!    before the clock advances, from the level list in order; an actor gets at most one tick, keeps that group if
//!    its `TickGroup` changes during the tick, and is skipped if destroyed by then. An actor spawned during a tick
//!    is first ticked in the next one: the record sets a spawned actor's `bTicked` from the level's tick state
//!    (spawn step 5), read here that way. UNVERIFIED: that reading, and the order across actors (the record reads
//!    one actor's tick). The unread alternative is spawn step 11's list of newly spawned actors: in public UE3 that
//!    list ticks actors spawned during a tick in the same frame (open item, decision 2026-11-15-26).
//! 3. One actor's tick is the record's authoritative path (NATIVE_ENGINE_CORE.md, "Actor tick order"): script
//!    `Tick(DeltaTime)`, the state-code step, the timer update pass, `LifeSpan` (destroyed when it falls to 0.0001
//!    or below, and the tick ends), then physics when `Physics` is not `PHYS_None` and `Role` is not 2 (simulated
//!    proxy). Physics of an actor with a `Base` is `NotYet`: whether the base is being moved by a rigid-body step is
//!    the record's condition and this world cannot see it. An actor with `Role` 0 gets only a physics step, and only
//!    in `PHYS_Falling`, `PHYS_Rotating`, `PHYS_Projectile` or `PHYS_Interpolating`; one whose `RemoteRole` is 2
//!    takes a variant the record did not read (`NotYet`). Nothing in this world sets `RemoteRole` to none. The
//!    record says the single-player game keeps local actors at remote role none, but where it does so was not read.
//!    Until then, a class whose defaults carry `RemoteRole` 2 stops the tick with this `NotYet`.
//! 4. The state-code step and the physics step are hooks ([`World::set_state_step`], [`World::set_physics_step`]):
//!    lane W5-STATES and the host provide them. Without one, an actor in a state, or one with physics to run, is
//!    `NotYet` naming that layer. The delta every part gets is the clock's dilated delta; an actor's
//!    `CustomTimeDilation` is not applied (UNVERIFIED, NATIVE_ENGINE_CORE.md "Open").

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use ow_attr::api::EvalCx;
use ow_balance::stage::RegionStages;
use ow_balance::types::BalanceSnapshot;
use ow_core::{NotYet, ParityMode};
use ow_core::sim::{ClockError, Step, TickGroup};
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativePack, Vm};
use ow_vm::error::VmError;
use ow_vm::natives_core::{ActorSource, ActorWalkFn};

use crate::actors::{
    self, Actors, actors_of, arg_object, byte, float, is_destroyed, object, physics, send_event,
    set_field,
};
use crate::query::{self, QueryKind, WorldQuery};
use crate::timers::{self, Timers};

/// The record of the tick.
pub const RECORD: &str = "NATIVE_ENGINE_CORE.md (Actor tick order)";

/// The class every actor derives from.
pub const ACTOR_CLASS: &str = "Engine.Actor";

/// `LifeSpan` at or below this destroys the actor (NATIVE_ENGINE_CORE.md, "Actor tick order", step 4).
pub const LIFESPAN_END: f32 = 0.0001;

/// `Role` 2, which the record's physics condition skips (simulated proxy).
const ROLE_SIMULATED_PROXY: u8 = 2;

/// The `RemoteRole` whose tick variant the record did not read (NATIVE_ENGINE_CORE.md: value 2).
const REMOTE_ROLE_VARIANT: u8 = 2;

/// A step a layer above provides for one actor: the actor and this tick's delta.
pub type ActorStep = Rc<dyn Fn(&mut Vm, ObjRef, f32) -> Result<(), VmError>>;

/// The balance inputs of this world and its region stages (decision 2026-11-15-18: the store of stages is the
/// caller's, because the controller field the game keeps them in is not read). A VM service.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct BalanceState {
    pub snapshot: BalanceSnapshot,
    pub region_stages: RegionStages,
}

impl BalanceState {
    pub fn new(snapshot: BalanceSnapshot) -> Self {
        BalanceState {
            snapshot,
            region_stages: RegionStages::new(),
        }
    }
}

/// Why building or ticking the world failed.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum WorldError {
    Vm(VmError),
    /// `WorldInfo.TimeDilation` holds a value the clock refuses.
    Clock(ClockError),
    /// A tick delta that is negative, NaN or infinite.
    BadDelta(f32),
}

impl fmt::Display for WorldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorldError::Vm(e) => write!(f, "world: {e}"),
            WorldError::Clock(ClockError::BadTimeDilation(d)) => write!(
                f,
                "world clock: WorldInfo.TimeDilation {} refused (finite and not negative only)",
                ow_core::fmt::fmt_g(f64::from(*d), 9)
            ),
            WorldError::Clock(other) => write!(f, "world clock: {other}"),
            WorldError::BadDelta(d) => write!(
                f,
                "world tick: the delta must be finite and not negative, got {}",
                ow_core::fmt::fmt_g(f64::from(*d), 9)
            ),
        }
    }
}

impl std::error::Error for WorldError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            WorldError::Vm(e) => Some(e),
            WorldError::Clock(e) => Some(e),
            WorldError::BadDelta(_) => None,
        }
    }
}

impl From<VmError> for WorldError {
    fn from(e: VmError) -> Self {
        WorldError::Vm(e)
    }
}

impl From<NotYet> for WorldError {
    fn from(n: NotYet) -> Self {
        WorldError::Vm(VmError::NotYet(n))
    }
}

/// The running world (module documentation).
pub struct World {
    pub vm: Vm,
    query: Rc<RefCell<WorldQuery>>,
    actors: Rc<RefCell<Actors>>,
    timers: Rc<RefCell<Timers>>,
    balance: Rc<RefCell<BalanceState>>,
    state_step: Option<ActorStep>,
    physics_step: Option<ActorStep>,
}

impl fmt::Debug for World {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("World")
            .field("vm", &self.vm)
            .field("actors", &self.actors)
            .finish_non_exhaustive()
    }
}

impl World {
    /// Installs the world services on `vm` and registers [`Pack`], then `packs` in order (module documentation).
    /// `vm` must know [`ACTOR_CLASS`] (its code packages hold it).
    pub fn new(
        mut vm: Vm,
        balance: BalanceState,
        packs: &[&dyn NativePack],
    ) -> Result<World, WorldError> {
        let actor_class = vm.find_class(ACTOR_CLASS)?;
        let query = query::install(&mut vm)?;
        let actors = vm.add_service(Actors::new(actor_class))?;
        // The world's empty scene layer, in Off (the hub's ruling on NATIVE_SPAWN_COMPONENT_ATTACH.md, item 2): the
        // spawn's component update and Actor.AttachComponent attach to it. An UNVERIFIED harness input, logged.
        if vm.parity == ParityMode::Off && crate::actors::add_empty_scene(&mut vm)? {
            vm.log.push(crate::actors::component_attach::HARNESS_LINE.to_string());
        }
        // The game info falls back to the placed WorldInfo's Game (decision 2026-11-16-49).
        query
            .try_borrow_mut()
            .map_err(|_| VmError::ServiceBorrowed("WorldQuery"))?
            .follow_world_info(actors.clone());
        // The player-controller context query (lane FIX-PHASELOCK-CHAIN, decision 2026-11-16-69).
        query
            .try_borrow_mut()
            .map_err(|_| VmError::ServiceBorrowed("WorldQuery"))?
            .register(
                QueryKind::PlayerController,
                query::player_controller_handler(),
            )?;
        let timers = vm.add_service(Timers::new())?;
        let balance = vm.add_service(balance)?;
        let walk: ActorWalkFn = Rc::new(|vm: &Vm, which| {
            let list = actors_of(vm)?;
            let list = list
                .try_borrow()
                .map_err(|_| VmError::ServiceBorrowed("Actors"))?;
            list.walk(&vm.space, which)
        });
        vm.add_service(ActorSource::new(walk))?;
        Pack.register(&mut vm)?;
        crate::pools::Pack.register(&mut vm)?; // W5-POOLS seam (decision 2026-11-15-29)
        crate::levelup::Pack.register(&mut vm)?;
        for p in packs {
            p.register(&mut vm)?;
        }
        Ok(World {
            vm,
            query,
            actors,
            timers,
            balance,
            state_step: None,
            physics_step: None,
        })
    }

    pub fn query(&self) -> Rc<RefCell<WorldQuery>> {
        self.query.clone()
    }

    pub fn actors(&self) -> Rc<RefCell<Actors>> {
        self.actors.clone()
    }

    pub fn timers(&self) -> Rc<RefCell<Timers>> {
        self.timers.clone()
    }

    pub fn balance(&self) -> Rc<RefCell<BalanceState>> {
        self.balance.clone()
    }

    /// The state-code step of an actor in a state (lane W5-STATES); set once.
    pub fn set_state_step(&mut self, f: ActorStep) -> Result<(), WorldError> {
        set_once(&mut self.state_step, f, "the state-code step")
    }

    /// The physics step of an actor whose `Physics` is not `PHYS_None` (the host); set once.
    pub fn set_physics_step(&mut self, f: ActorStep) -> Result<(), WorldError> {
        set_once(&mut self.physics_step, f, "the physics step")
    }

    /// One world tick of `real_dt` seconds (module documentation). Returns the clock's step.
    pub fn tick(&mut self, real_dt: f32) -> Result<Step, WorldError> {
        if !(real_dt.is_finite() && real_dt >= 0.0) {
            return Err(WorldError::BadDelta(real_dt));
        }
        let world_info = self.borrow_actors()?.world_info();
        if let Some(wi) = world_info {
            let d = float(&self.vm.space, wi, "TimeDilation")?;
            self.vm
                .clock
                .set_time_dilation(d)
                .map_err(WorldError::Clock)?;
        }
        // Each actor's group is decided once, here, before anything moves: an actor that changes its TickGroup
        // during the tick keeps this tick's group, and an actor spawned during the tick is not in the plan, so it is
        // first ticked in the next one. The record sets a spawned actor's bTicked from the level's tick state (spawn
        // step 5); reading that as "waits for the next tick" is UNVERIFIED, as is the order across actors. The unread
        // alternative is spawn step 11's newly-spawned list, which in public UE3 ticks such actors in the same frame.
        let plan = self.tick_plan()?;
        let step = self.vm.clock.advance(real_dt);
        if let Some(wi) = world_info {
            let (t, r) = (
                self.vm.clock.time_seconds(),
                self.vm.clock.real_time_seconds(),
            );
            set_field(&mut self.vm.space, wi, "TimeSeconds", Value::Float(t))?;
            set_field(&mut self.vm.space, wi, "RealTimeSeconds", Value::Float(r))?;
            // Lane W5-MAPLOAD: the frame's dilated delta, which script reads (the AI's actions, decision
            // 2026-11-16-66). A WorldInfo class that declares no DeltaSeconds (a synthetic fixture) is left alone.
            let class = self.vm.space.heap.get(wi).map_err(VmError::from)?.class;
            if self.vm.space.prop(class, "DeltaSeconds").is_some() {
                set_field(
                    &mut self.vm.space,
                    wi,
                    "DeltaSeconds",
                    Value::Float(step.dt),
                )?;
            }
        }
        let mut index = 0u8;
        while let Some(group) = TickGroup::from_index(index) {
            for &(actor, g) in &plan {
                if g == group.index() && !is_destroyed(&self.vm.space, actor)? {
                    self.tick_actor(actor, step.dt)?;
                }
            }
            index += 1;
        }
        Ok(step)
    }

    /// Every listed actor that is not destroyed, with its tick group, in list order.
    fn tick_plan(&self) -> Result<Vec<(ObjRef, u8)>, WorldError> {
        let list = self.borrow_actors()?.list().to_vec();
        let mut plan = Vec::with_capacity(list.len());
        for actor in list {
            if is_destroyed(&self.vm.space, actor)? {
                continue;
            }
            let g = byte(&self.vm.space, actor, "TickGroup")?;
            if TickGroup::from_index(g).is_none() {
                return Err(VmError::Script {
                    function: "World::tick".to_string(),
                    message: format!(
                        "{}.TickGroup is {g}, not a tick group",
                        ow_attr::eval::class_path(&self.vm.space, actor)
                    ),
                }
                .into());
            }
            plan.push((actor, g));
        }
        Ok(plan)
    }

    fn borrow_actors(&self) -> Result<std::cell::Ref<'_, Actors>, WorldError> {
        self.actors
            .try_borrow()
            .map_err(|_| VmError::ServiceBorrowed("Actors").into())
    }

    fn physics_or_not_yet(&mut self, actor: ObjRef, dt: f32) -> Result<(), WorldError> {
        match self.physics_step.clone() {
            Some(f) => Ok(f(&mut self.vm, actor, dt)?),
            None => Err(NotYet::layer_absent(
                "the physics step of an actor whose Physics is not PHYS_None",
                "the host's physics (ow-host)",
                RECORD,
            )
            .into()),
        }
    }

    /// One actor's tick (module documentation, step 3).
    fn tick_actor(&mut self, actor: ObjRef, dt: f32) -> Result<(), WorldError> {
        let space = &self.vm.space;
        if byte(space, actor, "RemoteRole")? == REMOTE_ROLE_VARIANT {
            return Err(NotYet::not_read(
                "the tick variant of an actor whose RemoteRole is 2",
                RECORD,
            )
            .into());
        }
        if byte(space, actor, "Role")? == 0 {
            let mode = physics(space, actor)?;
            if matches!(
                mode.as_str(),
                "PHYS_Falling" | "PHYS_Rotating" | "PHYS_Projectile" | "PHYS_Interpolating"
            ) {
                self.physics_or_not_yet(actor, dt)?;
            }
            return Ok(());
        }
        send_event(&mut self.vm, actor, "Tick", vec![Value::Float(dt)])?;
        if is_destroyed(&self.vm.space, actor)? {
            return Ok(());
        }
        if self
            .vm
            .space
            .heap
            .get(actor)
            .map_err(VmError::from)?
            .state
            .is_some()
        {
            match self.state_step.clone() {
                Some(f) => f(&mut self.vm, actor, dt)?,
                None => {
                    return Err(NotYet::layer_absent(
                        "the state-code step of an actor in a state",
                        "ow-vm states (lane W5-STATES)",
                        RECORD,
                    )
                    .into());
                }
            }
            if is_destroyed(&self.vm.space, actor)? {
                return Ok(());
            }
        }
        timers::update_pass(&mut self.vm, actor, dt)?;
        if is_destroyed(&self.vm.space, actor)? {
            return Ok(());
        }
        let life = float(&self.vm.space, actor, "LifeSpan")?;
        if life != 0.0 {
            let left = life - dt;
            set_field(&mut self.vm.space, actor, "LifeSpan", Value::Float(left))?;
            if left <= LIFESPAN_END {
                actors::destroy(&mut self.vm, actor)?;
                return Ok(());
            }
        }
        let space = &self.vm.space;
        if physics(space, actor)? != "PHYS_None"
            && byte(space, actor, "Role")? != ROLE_SIMULATED_PROXY
        {
            if object(space, actor, "Base")?.is_some() {
                return Err(NotYet::not_read(
                    "the physics condition of an actor with a Base (whether its base is being moved by a rigid-body step)",
                    RECORD,
                )
                .into());
            }
            self.physics_or_not_yet(actor, dt)?;
        }
        Ok(())
    }
}

fn set_once(
    slot: &mut Option<ActorStep>,
    f: ActorStep,
    what: &'static str,
) -> Result<(), WorldError> {
    if slot.is_some() {
        return Err(VmError::Registration {
            key: what.to_string(),
            reason: "already set",
        }
        .into());
    }
    *slot = Some(f);
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// The world's natives
// ---------------------------------------------------------------------------------------------------------------

const EVALUATE: &str = "AttributeInitializationDefinition.EvaluateInitializationData(attributeinitializationdata,object,object)";

/// The world's natives as a [`NativePack`]: `Actor.Spawn`, `Actor.Destroy`, the nine timer natives,
/// `AttributeInitializationDefinition.EvaluateInitializationData`, and the three `GetWorldInfo` natives. They find
/// the world's services at call time, so without [`World::new`] they answer `NotYet`.
/// Off mode also binds `Object.Localize`; its backend/readiness must be supplied explicitly by the caller.
#[derive(Debug, Clone, Copy, Default)]
pub struct Pack;

impl NativePack for Pack {
    fn register(&self, vm: &mut Vm) -> Result<(), VmError> {
        actors::register(vm)?;
        timers::register(vm)?;
        localize::register(vm)?;
        // EvaluateInitializationData(InitializationData, ContextSource, optional OptionalOverrideContextSource), a
        // static native: the evaluator of ow-attr (NATIVE_ATTRIBUTES.md section 1, UNVERIFIED there). The C++ refuses
        // an override context source (`src/combat.cpp`); the evaluator reads it as the record does (tried first for
        // the BaseValueAttribute read).
        vm.register_native(
            EVALUATE,
            Rc::new(|c: &mut NativeCall<'_>| {
                let v = c.arg(0)?.clone();
                let source = arg_object(c, EVALUATE, 1)?;
                let over = arg_object(c, EVALUATE, 2)?;
                let data = ow_attr::eval::init_data(&c.vm.space, &v)?;
                let attrs = c.vm.attrs.clone();
                let x = attrs.evaluate_init(&data, source, over, &mut EvalCx::new(c.vm))?;
                Ok(Value::Float(x))
            }),
        )?;
        // AttributeInitializationDefinition.SetBaseValue, Off only (lane IMPL-SET-BASE-VALUE).
        set_base_value::register(vm)?;
        // WorldInfo.GetWorldInfo: the persistent level's first actor; SequenceObject.GetWorldInfo and
        // BehaviorBase.GetWorldInfo: the world's own world info, ignoring self. Here both are the level's WorldInfo;
        // None when there is no world (NATIVE_ENGINE_CORE.md, "Lookups", UNVERIFIED).
        for key in [
            "WorldInfo.GetWorldInfo()",
            "SequenceObject.GetWorldInfo()",
            "BehaviorBase.GetWorldInfo()",
        ] {
            vm.register_native(
                key,
                Rc::new(|c: &mut NativeCall<'_>| {
                    let wi = match c.vm.service::<Actors>() {
                        Some(a) => a
                            .try_borrow()
                            .map_err(|_| VmError::ServiceBorrowed("Actors"))?
                            .world_info(),
                        None => None,
                    };
                    Ok(Value::Object(wi))
                }),
            )?;
        }
        // WorldInfo.IsPlayInEditor, a stock UE3 native (public UnrealScript semantics): true only inside the editor's
        // Play In Editor session. The shipped game never runs there, so false. Outside Reference only: the C++ does
        // not bind it (R3; decision 2026-11-16-81).
        if vm.parity != ParityMode::Reference {
            vm.register_native("WorldInfo.IsPlayInEditor()", Rc::new(|_c: &mut NativeCall<'_>| Ok(Value::Bool(false))))?;
            // WillowGameInfo.IsEditor (link 1's stop in `FindPlayerStart`; lane IMPL-IS-EDITOR, decision 2026-11-19-120):
            // the hub's ruling, a port answer like the empty online-subsystem slot. The port always runs the game,
            // never the editor, so false: no read, no write, UNVERIFIED in game.
            vm.register_native(IS_EDITOR, Rc::new(|_c: &mut NativeCall<'_>| Ok(Value::Bool(false))))?;
            // WorldInfo.AllNavigationPoints (link 1's next stop in `FindPlayerStart`; lane IMPL-ALL-NAVIGATION-POINTS,
            // decision 2026-11-19-125): the stock iterator over the serialized list, UNVERIFIED in game.
            nav_points::register(vm)?;
            // WillowPawn / WillowPlayerPawn.InitializePawnMeshes (link 1's stop in `maya:8`; lane
            // IMPL-INIT-PAWN-MESHES, NATIVE_INIT_PAWN_MESHES.md): the empty cases do nothing, a non-empty body
            // composition runs the build below (lane IMPL-BODY-COMPOSITION). UNVERIFIED in game.
            pawn_meshes::register(vm)?;
            // InstanceDataHelper.InitializeBodyComposition (lane IMPL-BODY-COMPOSITION,
            // NATIVE_INITIALIZE_BODY_COMPOSITION.md), UNVERIFIED in game.
            body_composition::register(vm)?;
            // SkeletalMeshComponent.FindAnimNode / FindSkelControl (lane IMPL-TREE-INIT-OWNER-EVENT, #543's rules),
            // read-only on the component's own tree; a key bound first keeps its binding. UNVERIFIED in game.
            tree_lookup::register(vm)?;
            vm.register_native(IS_LOCAL_PLAYER_CONTROLLER, Rc::new(is_local_player_controller))?;
            // The engine's data store client, its global stores and the online game settings route
            // (NATIVE_ONLINE_GAME_SETTINGS.md; lane IMPL-ONLINE-GAME-SETTINGS, decision 2026-11-19-65).
            datastore::register(vm)?;
            // GameEngine.GetOnlineSubsystem and OnlineSubsystemSteamworks.GetLoginStatus (NATIVE_LOGIN_STATUS.md; lane
            // IMPL-LOGIN-STATUS, decision 2026-11-19-95): named NotYets, the port's world having no online subsystem.
            online::register(vm)?;
            // WillowGameInfo.TravelPostCountdownInProcess (NATIVE_TRAVEL.md, UNVERIFIED; lane MAP-LOAD-C, decision
            // 2026-11-19-20): whether the game info's TravelPostCountdownTime is above 0; nothing else is read or written.
            // The C++ does not bind it, so Reference keeps the stub (R3).
            vm.register_native(TRAVEL_COUNTDOWN, Rc::new(travel_countdown_in_process))?;
            // Actor/Pawn/NavigationPoint.GetBoundingCylinder (NATIVE_GET_BOUNDING_CYLINDER.md; lane
            // IMPL-GET-BOUNDING-CYLINDER, decision 2026-11-19-203). The C++ binds none, so Reference keeps the stub (R3).
            bounding_cylinder::register(vm)?;
            // SkeletalMeshComponent.SetSkeletalMesh (NATIVE_SET_SKELETAL_MESH.md, UNVERIFIED; lane
            // IMPL-SET-SKELETAL-MESH): the same mesh does nothing; a change is LayerAbsent, before any write.
            skeletal_mesh::register(vm)?;
            // PrimitiveComponent.SetScale (NATIVE_PRIMITIVE_SETSCALE.md, UNVERIFIED; lane IMPL-SETSCALE): the same scale
            // does nothing; a change writes Scale and bNeedsUpdateTransform, or refuses an unread update first.
            primitive_scale::register(vm)?;
            // SkeletalMeshComponent.ForceSkelUpdate and UpdateAnimations (NATIVE_BONE_TRANSFORM_LAYER.md, UNVERIFIED;
            // lane IMPL-BONE-TRANSFORM-LAYER): keep-existing per key, Off only.
            bone_transform::register(vm)?;
            // Actor.SetPhysics (NATIVE_ACTOR_SET_PHYSICS.md, UNVERIFIED; lane IMPL-ACTOR-SETPHYSICS): keep-existing by
            // key, Off only; ow-combat's hit pack keeps its byte-write stand-in for Reference only.
            set_physics::register(vm)?;
            // Actor.SetBase (NATIVE_ACTOR_SETBASE.md, UNVERIFIED; lane IMPL-LINK6-AFTER-TAKE): the seven read classes, Off.
            set_base::register(vm)?;
            // WorldInfo.AllControllers (NATIVE_LINK6_AFTER_TAKE.md 4.1, UNVERIFIED; lane IMPL-LINK6-AFTER-TAKE), Off.
            all_controllers::register(vm)?;
            // BodyClassDefinition.GetWeaponHoldDef (NATIVE_GET_WEAPON_HOLD_DEF.md, UNVERIFIED; lane
            // IMPL-GETWEAPONHOLDDEF): a pure read of the body class's weapon holds; keep-existing by key, Off only.
            weapon_hold::register(vm)?;
            // AnimNodeAimOffset.SetActiveProfileByName / ByIndex (NATIVE_AIMOFFSET_SET_PROFILE.md, UNVERIFIED; lane
            // IMPL-AIMOFFSET-SET-PROFILE): the profile switch; keep-existing by key, Off only.
            aim_offset::register(vm)?;
            // maya:8's tail (NATIVE_LINK1_TAIL_SETTERS.md, UNVERIFIED; lane IMPL-LINK1-TAIL-SETTERS): the blend and
            // collision setters; keep-existing by key, Off only.
            blend_setters::register(vm)?;
            collision_setters::register(vm)?;
            // maya:8 after the collision setters (NATIVE_LINK1_RBPHYSICS_AND_AFTER.md, UNVERIFIED; lane
            // IMPL-LINK1-RBPHYSICS): InitializeRBPhysics and the player pawn's natives; keep-existing by key, Off only.
            player_pawn_tail::register(vm)?;
            // maya:8 after InitializeRBPhysics (NATIVE_GETPHYSXLEVEL_AND_AFTER.md, UNVERIFIED; lane
            // SOCKETS-PUBLISH-AND-PHYSX-LEVEL): GetPhysXLevel, SetHasPhysicsAssetInstance and DoInitRBPhys, Off only.
            physx_level::register(vm)?;
            // Link 6's take (NATIVE_UPDATE_AMMO_COUNTS_AND_TAKE.md, UNVERIFIED; lane IMPL-UPDATEAMMOCOUNTS):
            // ComponentList, MakeNoise, IsUsingVehicle and SetOwner, Off only.
            actor_take::register(vm)?;
            // Pawn.IsPlayerPawn() (NATIVE_BODYCOMP_RENDER_HOOKS.md section 6, UNVERIFIED; lane
            // FIX-IS-PLAYER-PAWN-AND-UNBOUND-KEYS): hook 2's pawn test as the native; keep-existing by key, Off only.
            render_hooks::register(vm)?;
        }
        Ok(())
    }
}

const IS_LOCAL_PLAYER_CONTROLLER: &str = "PlayerController.IsLocalPlayerController()";
/// The native key of `WillowGameInfo.IsEditor()` (lane IMPL-IS-EDITOR).
pub const IS_EDITOR: &str = "WillowGameInfo.IsEditor()";

/// `PlayerController.IsLocalPlayerController()`, a stock UE3 native (public UE3 semantics: true when the controller's
/// `Player` is a local player, not a network connection; decision 2026-11-19-15). Outside Reference only: the C++
/// logs it as a zero-result stub, which Reference keeps (R3).
///
/// The world models local players by its local-player list (`Actors::add_local_player`), not by `Player` objects:
/// - a listed controller that is not destroyed: true;
/// - an unlisted controller whose `Player` is None: false (no player at all, as in UE3);
/// - an unlisted controller whose `Player` is set: `NotYet { NotPorted }`. The world does not model that object, so it
///   cannot tell a local player from a connection;
/// - no world (no `Actors` service): `NotYet { LayerAbsent }`.
///
/// Whether this game's own controller classes change the rule is UNVERIFIED (not read; no live check yet). Two
/// answers are this port's modelling choices, not UE3's rule (which decides by `Player` alone), and are UNVERIFIED
/// too: a listed controller that is destroyed answers false, and a listed controller whose `Player` is None answers
/// true.
fn is_local_player_controller(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let Some(this) = c.this else {
        // The world is still asked first, as before the split (lane IMPL-BODYCOMP-RENDER-HOOKS).
        local_players(c.vm)?;
        return Ok(Value::Bool(false));
    };
    Ok(Value::Bool(is_local_player(c.vm, this)?))
}

/// The local-player list, else the native's `LayerAbsent` (no world).
fn local_players(vm: &Vm) -> Result<Vec<ObjRef>, VmError> {
    match vm.service::<Actors>() {
        Some(a) => Ok(a.try_borrow().map_err(|_| VmError::ServiceBorrowed("Actors"))?.local_players().to_vec()),
        None => Err(NotYet::layer_absent(
            "PlayerController.IsLocalPlayerController: the engine's local player list",
            "ow-world (a World)",
            "docs/verification/NATIVE_ENGINE_CORE.md",
        )
        .into()),
    }
}

/// [`is_local_player_controller`]'s answer for controller `this`, read only, with the same refusals: for a caller
/// that plans on it without a call (the body composition's rendering hooks, lane IMPL-BODYCOMP-RENDER-HOOKS).
pub fn is_local_player(vm: &Vm, this: ObjRef) -> Result<bool, VmError> {
    let players = local_players(vm)?;
    if players.contains(&this) {
        return Ok(!is_destroyed(&vm.space, this)?);
    }
    if object(&vm.space, this, "Player")?.is_some() {
        return Err(NotYet::not_ported(
            "PlayerController.IsLocalPlayerController on a controller outside the world's local-player list whose \
             Player is set (the world does not model Player objects)",
            "docs/verification/NATIVE_ENGINE_CORE.md",
        )
        .into());
    }
    Ok(false)
}

/// The native key of `WillowGameInfo.TravelPostCountdownInProcess()`.
pub const TRAVEL_COUNTDOWN: &str = "WillowGameInfo.TravelPostCountdownInProcess()";

/// `WillowGameInfo.TravelPostCountdownInProcess()` (NATIVE_TRAVEL.md): `TravelPostCountdownTime > 0` on the game info
/// it is called on. A class that declares no int `TravelPostCountdownTime` is a typed error, never a default.
fn travel_countdown_in_process(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let me = c.this.ok_or_else(|| VmError::Script {
        function: TRAVEL_COUNTDOWN.to_string(),
        message: "called with no game info".to_string(),
    })?;
    match actors::field(&c.vm.space, me, "TravelPostCountdownTime")? {
        Value::Int(n) => Ok(Value::Bool(n > 0)),
        other => Err(VmError::Script {
            function: TRAVEL_COUNTDOWN.to_string(),
            message: format!("TravelPostCountdownTime holds {}, not an int", other.kind_name()),
        }),
    }
}

/// A map's placed actors in a world (lane W5-MAPLOAD; here because lanes do not edit the crate's `lib.rs`).
pub mod mapload;

/// A level's navigation, cover and pylon lists joined onto the world's (lane IMPL-MAPLOAD-NAVLIST, NATIVE_MAP_LOAD.md
/// section 5); the bring-up calls it.
pub mod nav_join;

/// The skeletal mesh data layer, read only (lane IMPL-SKELETAL-MESH-LAYER).
pub mod skeletal;

/// The bone-transform layer's gate and `ForceSkelUpdate` / `UpdateAnimations` (lane IMPL-BONE-TRANSFORM-LAYER).
pub mod bone_transform;
// UpdateAnimations' per-node step (NATIVE_UPDATEANIMATIONS_TREE_TICKS.md; lane IMPL-UPDATEANIMATIONS-TREE-TICKS).
pub mod tree_ticks;
// The additive override and the residency test tree_ticks, anim_node_init and aim_offset share (lane
// ADDITIVE-OVERRIDE-HELPER).
pub(crate) mod additive_override;

pub mod localize;

/// The engine's data store client and the online game settings route (lane IMPL-ONLINE-GAME-SETTINGS).
pub mod datastore;

/// The two mission numbers of the settings refresh and the mission screen's text list (lane IMPL-MISSION-NUMBERS).
pub mod mission_numbers;
/// The login status path's natives (lane IMPL-LOGIN-STATUS).
pub mod online;

/// `UpdateURL`, `SaveConfig` and `UpdateInputContext` after the login check (lane IMPL-LINK1-AFTER-LOGIN).
pub mod after_login;
pub mod aim_offset;
pub mod player_pawn_tail;
pub mod actor_take;
pub mod physx_level;
pub mod aim_profiles;
pub mod blend_setters;
pub mod collision_setters;

/// `SkeletalMeshComponent.SetSkeletalMesh` (lane IMPL-SET-SKELETAL-MESH, NATIVE_SET_SKELETAL_MESH.md).
pub mod skeletal_mesh;

/// `PrimitiveComponent.SetScale` (lane IMPL-SETSCALE, NATIVE_PRIMITIVE_SETSCALE.md).
pub mod primitive_scale;

/// The player pawn data manager natives (lane FIX-PAWN-DATA-B, NATIVE_PAWN_DATA.md).
pub mod class_route;
pub mod pawn_data;

/// `WorldInfo.AllNavigationPoints` (lane IMPL-ALL-NAVIGATION-POINTS).
pub mod nav_points;
pub mod anim_node_lookup;
pub mod body_composition;
// Lane IMPL-COPY-ANIM-TREE: the animation tree's copy and initialisation (NATIVE_COPY_ANIM_TREE.md).
pub mod anim_tree_copy;
pub mod tree_lookup;
// Lane IMPL-ANIM-NODE-INIT: the per-node initialisation (NATIVE_ANIM_NODE_INIT.md).
pub mod anim_node_init;
// Lane IMPL-PER-BONE-WEIGHTS: the per-bone weight builders (NATIVE_PER_BONE_WEIGHTS.md).
pub mod per_bone_weights;
/// The tree initialisation's component pass and per-bone control arrays in Off (lane IMPL-COPY-COMPONENT-PASS).
pub mod control_arrays;
pub mod component_setters;
pub mod render_hooks;
pub mod pawn_meshes;

/// `AttributeInitializationDefinition.SetBaseValue` (lane IMPL-SET-BASE-VALUE).
pub mod set_base_value;

/// `Actor.GetBoundingCylinder` and its `Pawn` and `NavigationPoint` keys (lane IMPL-GET-BOUNDING-CYLINDER).
pub mod bounding_cylinder;
pub mod skeletal_bounds;
/// `Actor.Trace` in Off, shared by the Phaselock and go-down binding sites (lane IMPL-ACTOR-TRACE).
pub mod actor_trace;
pub mod set_physics;
pub mod set_base;
pub mod all_controllers;
pub mod weapon_hold;

#[cfg(test)]
mod tests;
