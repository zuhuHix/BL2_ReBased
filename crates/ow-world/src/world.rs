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
//!    `WorldInfo.TimeSeconds` and `RealTimeSeconds` are written from the clock. UNVERIFIED: where and how the game
//!    advances them was not located (the clock is a placeholder, `ow_core::sim`).
//! 2. The actors are ticked by tick group (`TickingGroup` values 0 to 4 in order), each group over the level list
//!    in order as it stands when the group starts; destroyed actors are skipped. UNVERIFIED: the record reads one
//!    actor's tick, not the order across actors or what an actor spawned during a group gets.
//! 3. One actor's tick is the record's authoritative path (NATIVE_ENGINE_CORE.md, "Actor tick order"): script
//!    `Tick(DeltaTime)`, the state-code step, the timer update pass, `LifeSpan` (destroyed when it falls to 0.0001
//!    or below, and the tick ends), then physics when `Physics` is not `PHYS_None`. An actor with `Role` 0 gets only
//!    a physics step, and only in `PHYS_Falling`, `PHYS_Rotating`, `PHYS_Projectile` or `PHYS_Interpolating`; one
//!    whose `RemoteRole` is 2 takes a variant the record did not read (`NotYet`).
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
use ow_core::NotYet;
use ow_core::sim::{ClockError, Step, TickGroup};
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativePack, Vm};
use ow_vm::error::VmError;
use ow_vm::natives_core::{ActorSource, ActorWalkFn};

use crate::actors::{
    self, Actors, actors_of, arg_object, byte, float, is_destroyed, physics, send_event, set_field,
};
use crate::query::{self, WorldQuery};
use crate::timers::{self, Timers};

/// The record of the tick.
pub const RECORD: &str = "NATIVE_ENGINE_CORE.md (Actor tick order)";

/// The class every actor derives from.
pub const ACTOR_CLASS: &str = "Engine.Actor";

/// `LifeSpan` at or below this destroys the actor (NATIVE_ENGINE_CORE.md, "Actor tick order", step 4).
pub const LIFESPAN_END: f32 = 0.0001;

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
        let step = self.vm.clock.advance(real_dt);
        if let Some(wi) = world_info {
            let (t, r) = (
                self.vm.clock.time_seconds(),
                self.vm.clock.real_time_seconds(),
            );
            set_field(&mut self.vm.space, wi, "TimeSeconds", Value::Float(t))?;
            set_field(&mut self.vm.space, wi, "RealTimeSeconds", Value::Float(r))?;
        }
        let mut index = 0u8;
        while let Some(group) = TickGroup::from_index(index) {
            let list = self.borrow_actors()?.list().to_vec();
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
                if g == group.index() {
                    self.tick_actor(actor, step.dt)?;
                }
            }
            index += 1;
        }
        Ok(step)
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
        if physics(&self.vm.space, actor)? != "PHYS_None" {
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
#[derive(Debug, Clone, Copy, Default)]
pub struct Pack;

impl NativePack for Pack {
    fn register(&self, vm: &mut Vm) -> Result<(), VmError> {
        actors::register(vm)?;
        timers::register(vm)?;
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
        Ok(())
    }
}

#[cfg(test)]
mod tests;
