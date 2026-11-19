//! Owner: lane W6-FIRE. The Fire mission slice: the stock mission and the target dummy's own behavior provider bound
//! end to end without the engine (the port of `src/slice.cpp`, `vm::FireMissionSlice`). AI-assisted (Claude Code).
//!
//! **What runs.** [`FireMissionSlice`] holds the mission tracker ([`MissionSystem`], read from `Startup` as the C++
//! does), the script bridge ([`MissionScript`]: the controller's script, the waypoint, the director), one dialog
//! system shared by the tracker's provider and the director's dialog component (`MissionSystem::dialog`), and
//! the dummy's provider (`ow_behavior::kernel::Provider` over
//! `GD_TargetDummy.Character.CharClass_TargetDummy.BehaviorProviderDefinition_5` of the given map package). The host
//! supplies the world: it places the player in the range, deals the damage, runs the Kismet world ops; the slice
//! hands back [`HostEvent`]s in order.
//!
//! **The rules**, each with the label its record gives it (porting changes nothing about what is known):
//! - *The dummy's enable conditions* (NATIVE_BEHAVIOR_POPULATION.md section B, read from native code: UNVERIFIED).
//!   Each sequence's `CustomEnableCondition` is a `BehaviorSequenceEnableByMission`. A condition that is not
//!   objective-specific looks up its mission's status bit (`bNotStarted`, `bActive`, `bReadyToTurnIn`,
//!   `bComplete`); an objective-specific one with no linked objective is false, else the objective's state
//!   (`MissionSystem::objective_state`) looks up its bit; then a non-empty `ObjectiveSetRestrictions` keeps a true
//!   verdict only while one listed set is the active set. A mission other than the slice's counts as Complete when it
//!   is in the completed set, else NotStarted, and its objectives as NotStarted. `RequiredObjectivesComplete` and
//!   `Failed` are states the tracker never enters, so their bits never hold.
//! - *What a notification does* to the registered dummy: every condition's verdict is recomputed from the tracker's
//!   current state and applied to its sequence (the kernel fires events only for real transitions). The tracker's
//!   observers run in registration order: the VM's own (the waypoint) first, then the dummy's conditions.
//! - *The registration* (NATIVE_BEHAVIOR_POPULATION.md section C, UNVERIFIED): at the first spawn the provider
//!   registers as a consumer (the `bEnabledOnSpawn` sequences), the conditions deliver their verdicts at once (the
//!   LevelLoad verdict), and only then does `OnSpawned` fire. Before that the provider observes nothing.
//! - *The Kismet variable* `TargetDummy` holds the dummy's pawn from just after its spawn behaviors (observed live,
//!   FIRE_MISSION_GOLDEN_TRACE.md lane L4); a behavior applied to it before that does not run (UNVERIFIED rule).
//! - *Sequence changes* (`Behavior_ChangeRemoteBehaviorSequenceState`, `ITargetable.EChangeStatus`): Enable, Disable,
//!   and Toggle as a flip of the current state (UNVERIFIED); a change naming another provider is an error line.
//! - *The settle bound*: dummy sequence changes made by mission notifications may change mission state again; the
//!   slice drains until quiet, at most 16 rounds, and nested notifications at most 16 deep (the C++ guards, not game
//!   rules).
//! - *Experience*: the bridge's script path (NATIVE_PROGRESSION.md, UNVERIFIED): the turn-in's `ExpEarn`, the pool
//!   update every tick, the level-up's skill points.
//!
//! **Stand-ins and gaps** (architecture R3; each has an Off-mode test and a row in decision 2026-11-16-38):
//! - The player's health pool after a level-up (`MaxHealth`, `Health` events, swap 7b): the C++ bridge builds one
//!   pool by hand on the controller (the class's `HealthPoolDefinition`, a manager, the controller as provider) and
//!   seeds it at `player:` / `health:`. Reproduced here under `ParityMode::Reference` only; in `ParityMode::Off` a
//!   level-up of a class that names a health pool, and the `health:` input, are `NotYet { NotRead }` (the game builds
//!   its pools in the class-defaults routine, not read).
//! - `Behavior_AIHold` on the dummy: the C++ records it as a no-op before the dummy has a mind (it gets its mind and
//!   AI component at its first pawn hit, through the combat chain). Reference: that no-op. Off (decision
//!   2026-11-16-74): the installed script runs on each context through the VM ([`ow_ai::awareness::holds::apply`]).
//!   A pawn context answers `IAIInterface` by `Pawn`'s rule (`ow_combat::pawn_interface`, decision 2026-11-19-62,
//!   which [`FireMissionSlice::new`] registers): a pawn with nothing to ask answers none and the script places no
//!   hold; a pawn whose controller's own answer is not read is `NotYet { NotRead }` naming it, with nothing run.
//!   Without a pawn there is no context and nothing runs. Before `OnTakeDamage` fires, a hold is checked on its
//!   contexts ([`ow_combat::holds::preflight_hold_contexts`]) only when that is cheap and certain: it is the behavior
//!   the firing runs before any other and has no variable links ([`Provider::certain_first_behavior`]), so its
//!   refusal leaves the event state untouched. Any other hold is checked by the firing, after the event's writes, so
//!   its refusal is terminal: its `NotYet` is prefixed "stopped after partial writes", its kind kept (reviews r1 and
//!   r2 of #456).
//! - The player's hit through the stock pawn chain (`damage_dummy` with a damage): `ow-combat` has it (W6-COMBAT), but
//!   it is not wired into the slice yet (follow-up W6-COMBAT-RUN2), so `NotYet { NotPorted }` in both modes. Without
//!   a damage (the host's call so far) `OnTakeDamage` is raised directly, as in the C++.
//! - The world ops listed at the boundary (`Behavior_Transform`, `Behavior_RegisterTargetable`, `Behavior_IntMath`,
//!   `Behavior_ChangeInstanceDataSwitch`, `Behavior_SpecialMove`): not run in either mode (the kernel's boundary
//!   list, with the decoded fields the host needs), as in the C++.
//!
//! **Errors.** A tracker notification reaches the dummy through a hook that returns nothing; an error there is kept
//! and returned by the public call that caused it, after that call's own work (the C++ exception would unwind the
//! tracker's routine at once). Everything else is returned as it happens.
//!
//! One slice per fresh world ([`MissionScript::new`]'s rule): after a failed [`FireMissionSlice::new`] drop the world.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use ow_attr::api::EvalCx;
use ow_attr::eval::init_data;
use ow_behavior::kernel::{BehaviorId, BoundaryCall, Provider};
use ow_behavior::provider::{BehaviorError, class_path_of, enum_names, provider_path_leaf, stock_path};
use ow_behavior::registry::{DescribeFn, Handler};
use ow_core::{DetMap, DetSet, NotYet, PackageName, ParityMode};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::prefix::PropertyPrefix;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;
use ow_world::world::World;

use crate::bridge::{self, Gain, MarcusUse, MissionLists, MissionScript, Toucher, error_text, instantiate_export};
use crate::dialog::DialogSystem;
use crate::tracker::{self, EffectKind, MissionError, MissionSystem, Notification, field, field_value, integer, prop, ref_path, text};

#[cfg(test)]
mod tests;

/// The records of the slice's own rules.
pub const RECORD: &str = "NATIVE_BEHAVIOR_POPULATION.md sections B-C, FIRE_MISSION_GOLDEN_TRACE.md";

/// The record of the health pool stand-in.
pub const HEALTH_RECORD: &str = "NATIVE_LEVEL_UP_ATTRIBUTES.md";

/// The package the mission is read from (the C++ reads it from `Startup` whatever the map package is).
pub const MISSION_PACKAGE: &str = "Startup";

/// The dummy's behavior provider in the map package.
pub const DUMMY_PROVIDER: &str = "GD_TargetDummy.Character.CharClass_TargetDummy.BehaviorProviderDefinition_5";

/// The dummy's stock pawn archetype.
pub const DUMMY_PAWN: &str = "GD_TargetDummy.Character.Pawn_TargetDummy";

/// The stock GoToRange waypoint placed in the map package.
pub const WAYPOINT: &str = "TheWorld.PersistentLevel.WillowWaypoint_9";

/// The named Kismet variable the level's populated-actor event writes the dummy's pawn into.
pub const TARGET_DUMMY_VARIABLE: &str = "TargetDummy";

/// The objective the range wrapper updates without a waypoint.
pub const GO_TO_RANGE: &str = "RockPaper_GoToRange";

/// The stock damage type of the legacy `hit_dummy(true)`.
pub const INCENDIARY_IMPACT: &str = "GD_Incendiary.DamageType.DmgType_Incendiary_Impact";

/// The only enable-condition class the slice evaluates.
pub const CONDITION_CLASS: &str = "WillowGame.BehaviorSequenceEnableByMission";

/// The layer the pawn hit chain needs.
const COMBAT_FOLLOW_UP: &str = "the combat chain wired into the Fire slice (follow-up W6-COMBAT-RUN2)";

/// The settle bound of the drain and the depth bound of nested notifications (the C++ 16).
const SETTLE: usize = 16;

/// The mission status bits a condition looks up, by `Status::index` (the C++ `statusFields`).
const STATUS_FIELDS: [&str; 4] = ["bNotStarted", "bActive", "bReadyToTurnIn", "bComplete"];

/// What the host must do or show, in order (the C++ `HostEvent::Kind`).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostKind {
    /// `a` = the event name (`b` = the mission's path when the mission raised it).
    RemoteEvent,
    /// `a` tag, `b` group, `c` talker; `detail` the dialog's text.
    Dialog,
    /// `a` = the status effect definition.
    StatusEffect,
    MissionWeaponGranted,
    MissionWeaponRemoved,
    /// `a` = the reward's XP attribute.
    Reward,
    /// `a` = the status name.
    Status,
    ObjectiveSet,
    ObjectiveComplete,
    /// `a` = experience the pool gained.
    Experience,
    /// `a` = the level the pool update reached.
    Level,
    /// The mission screen was opened: `a` = movie definition, `b` = the director.
    MissionInterface,
    /// The on-use dialog's tag: `a` tag, `b` speaker, `c` the other object's class.
    OnUseDialog,
    /// `a` = the unspent skill points the level-up awarded (sent with a Level event, 0 included).
    SkillPoints,
    /// `a` = the new maximum health (Reference only, module documentation).
    MaxHealth,
    /// `a` = current health, `b` = maximum health after the level-up (Reference only).
    Health,
    /// `a` = provider, `b` = sequence, `c` = `CHANGE_Enable` / `_Disable` / `_Toggle`.
    SequenceChange,
    TurnInDialog,
    /// Never produced here: the level-up skill effects are lane W6-SKILLS (the bridge answers `NotYet`).
    ActionSkillCooldown,
    /// Never produced here, as [`HostKind::ActionSkillCooldown`].
    WeaponDamageScale,
    DismissalDialog,
    /// `a` = the director's path.
    MissionInterfaceClosed,
    /// `a` = the mission's path.
    RewardPage,
    RewardReceived,
}

impl HostKind {
    /// The `--slice-run` JSON name.
    pub fn json_name(self) -> &'static str {
        match self {
            HostKind::RemoteEvent => "remote_event",
            HostKind::Dialog => "dialog",
            HostKind::StatusEffect => "status_effect",
            HostKind::MissionWeaponGranted => "mission_weapon_granted",
            HostKind::MissionWeaponRemoved => "mission_weapon_removed",
            HostKind::Reward => "reward",
            HostKind::Status => "status",
            HostKind::ObjectiveSet => "objective_set",
            HostKind::ObjectiveComplete => "objective_complete",
            HostKind::Experience => "experience",
            HostKind::Level => "level",
            HostKind::MissionInterface => "mission_interface",
            HostKind::OnUseDialog => "on_use_dialog",
            HostKind::SkillPoints => "skill_points",
            HostKind::MaxHealth => "max_health",
            HostKind::Health => "health",
            HostKind::SequenceChange => "sequence_change",
            HostKind::TurnInDialog => "turn_in_dialog",
            HostKind::ActionSkillCooldown => "action_skill_cooldown",
            HostKind::WeaponDamageScale => "weapon_damage_scale",
            HostKind::DismissalDialog => "dismissal_dialog",
            HostKind::MissionInterfaceClosed => "mission_interface_closed",
            HostKind::RewardPage => "reward_page",
            HostKind::RewardReceived => "reward_received",
        }
    }
}

/// One host event (the C++ `HostEvent`).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostEvent {
    pub kind: HostKind,
    pub a: String,
    pub b: String,
    pub c: String,
    pub detail: String,
}

impl HostEvent {
    fn new(kind: HostKind, a: impl Into<String>, b: impl Into<String>, c: impl Into<String>) -> Self {
        HostEvent {
            kind,
            a: a.into(),
            b: b.into(),
            c: c.into(),
            detail: String::new(),
        }
    }
}

/// The health pool the Reference stand-in builds (module documentation).
#[derive(Debug, Clone, Copy)]
struct HealthPool {
    pool: ObjRef,
}

#[derive(Default)]
struct State {
    events: Vec<HostEvent>,
    /// Experience gains taken from the bridge, as events, until the next drain (the C++ `drainExperience` order).
    gains: Vec<HostEvent>,
    completed: DetSet<String>,
    errors: Vec<String>,
    not_yet: Vec<NotYet>,
    /// The first error a notification hook met, returned by the public call that caused it.
    pending: Option<MissionError>,
    registered: bool,
    notify_depth: usize,
    dummy_pawn: Option<ObjRef>,
    health: Option<HealthPool>,
    health_failed: bool,
}

struct Slice {
    mission: MissionSystem,
    script: MissionScript,
    /// The dummy's provider, out of its slot while it runs (`crate::tracker::lend`).
    dummy: RefCell<Option<Provider>>,
    dummy_lost: Cell<bool>,
    dummy_name: String,
    dummy_package: PackageName,
    mission_package: PackageName,
    world_info: Option<ObjRef>,
    state: RefCell<State>,
}

/// The Fire mission slice (module documentation). Cheap to clone: every clone is the same slice.
#[derive(Clone)]
pub struct FireMissionSlice {
    inner: Rc<Slice>,
}

impl std::fmt::Debug for FireMissionSlice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FireMissionSlice")
            .field("mission", &self.inner.mission)
            .field("dummy", &self.inner.dummy_name)
            .finish_non_exhaustive()
    }
}

fn data(text: String) -> MissionError {
    BehaviorError::Data(text)
}

fn busy() -> MissionError {
    BehaviorError::Vm(VmError::NotYet(NotYet::not_ported(
        "a dummy provider call while the provider is running and not lent to its slot (the C++ nests it)",
        RECORD,
    )))
}

fn lost() -> MissionError {
    BehaviorError::Vm(VmError::NotYet(NotYet::not_ported(
        "a slice whose dummy provider was lost by a failed lend (every later call is refused)",
        RECORD,
    )))
}

/// The C++ `nameOf`: the enum value name at the value's index, else `?<index>`.
fn name_of(names: &[String], value: Option<&Value>) -> String {
    let index = value.map_or(0, integer);
    usize::try_from(index)
        .ok()
        .and_then(|i| names.get(i))
        .cloned()
        .unwrap_or_else(|| format!("?{index}"))
}

fn set_prop(vm: &mut Vm, o: ObjRef, name: &str, v: Value) -> Result<(), MissionError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm
        .space
        .prop(class, name)
        .ok_or_else(|| data(format!("mission script bridge: {} has no property {name}", class_path_of(vm, o).unwrap_or_default())))?;
    vm.space.write(o, PropAddr::scalar(p), v)?;
    Ok(())
}

fn required(vm: &Vm, o: ObjRef, name: &str) -> Result<Value, MissionError> {
    prop(&vm.space, o, name)?.ok_or_else(|| {
        data(format!(
            "mission script bridge: {} has no property {name}",
            class_path_of(vm, o).unwrap_or_default()
        ))
    })
}

fn number(v: &Value) -> f64 {
    match v {
        Value::Float(f) => f64::from(*f),
        other => integer(other) as f64,
    }
}

/// The hold behavior's class, which the slice handles and preflights.
const HOLD_BEHAVIOR: &str = "GearboxFramework.Behavior_AIHold";

/// The canonical write counters (#518's tracker): heap writes, generator draws and definition-store load
/// transitions. Each only grows, so a refusal is after a write exactly when one moved since a recorded start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WriteCounters {
    heap: u64,
    draws: u64,
    loads: u64,
}

impl WriteCounters {
    fn of(vm: &Vm) -> Self {
        WriteCounters { heap: vm.space.heap.committed_writes(), draws: vm.rng.draws(), loads: vm.defs.load_transitions() }
    }

    fn moved(&self, vm: &Vm) -> bool {
        WriteCounters::of(vm) != *self
    }
}

impl FireMissionSlice {
    /// Builds the slice on `world` (a fresh one: [`MissionScript::new`]'s rule): mission `mission_path` of
    /// [`MISSION_PACKAGE`], the bridge with the waypoint and the director of `dummy_package`, the shared dialog, and the
    /// dummy's provider `dummy_path` of `dummy_package` with its behaviors bound. Errors carry the C++ texts (`dummy
    /// behavior provider not found: <path>`).
    pub fn new(
        world: &mut World,
        mission_path: &str,
        dummy_package: &PackageName,
        dummy_path: &str,
    ) -> Result<FireMissionSlice, MissionError> {
        let mission_package =
            PackageName::parse(MISSION_PACKAGE).map_err(|e| data(format!("{MISSION_PACKAGE}: {e}")))?;
        let mission = MissionSystem::new(&mut world.vm, &mission_package, mission_path)?;
        let script = MissionScript::new(world, &mission)?;
        let world_info = world
            .actors()
            .try_borrow()
            .map_err(|_| BehaviorError::Vm(VmError::ServiceBorrowed("Actors")))?
            .world_info();
        let vm = &mut world.vm;
        // Behavior_AIHold's pawn context answers by Pawn's QueryInterface rule in Off (decision 2026-11-19-62).
        ow_combat::pawn_interface::register(vm)?;
        let definition = tracker::find_object(vm, dummy_package, dummy_path)?
            .ok_or_else(|| data(format!("dummy behavior provider not found: {dummy_path}")))?;
        let provider = Provider::new(vm, definition)?;
        let dummy_name = dummy_path.rsplit('.').next().unwrap_or(dummy_path).to_string();
        let slice = FireMissionSlice {
            inner: Rc::new(Slice {
                mission: mission.clone(),
                script: script.clone(),
                dummy: RefCell::new(Some(provider)),
                dummy_lost: Cell::new(false),
                dummy_name,
                dummy_package: dummy_package.clone(),
                mission_package,
                world_info,
                state: RefCell::new(State::default()),
            }),
        };
        // The tracker's observers in registration order: the VM's own (the waypoint) first, then the dummy's conditions.
        let w = Rc::downgrade(&slice.inner);
        mission.set_notify_hook(Some(Rc::new(move |vm: &mut Vm, kind: Notification| {
            if let Some(inner) = w.upgrade() {
                let s = FireMissionSlice { inner };
                // A script stop after partial writes unwinds the tracker at once (review r2 of #439), from the
                // observers or from the dummy's conditions (r3): the hook returns it before the tracker goes on.
                s.inner.script.notify(vm, kind)?;
                let conditions = s.apply_conditions(vm);
                s.settle_conditions(conditions)?;
            }
            Ok(())
        })));
        // The stock GoToRange waypoint and the director, when the map package has them.
        script.place_waypoint(vm, dummy_package, WAYPOINT)?;
        script.place_marcus(vm, dummy_package, bridge::MARCUS_PAWN)?;
        slice.bind_dummy(vm)?;
        Ok(slice)
    }

    // ---- the dummy's behaviors ---------------------------------------------------------------------------------------

    fn weak(&self) -> Weak<Slice> {
        Rc::downgrade(&self.inner)
    }

    fn upgrade(w: &Weak<Slice>) -> Result<FireMissionSlice, BehaviorError> {
        w.upgrade()
            .map(|inner| FireMissionSlice { inner })
            .ok_or_else(|| data("the Fire slice of this provider is gone".to_string()))
    }

    /// The dummy's handlers (the C++ constructor's `d.handle` / `d.reportAtBoundary`).
    fn bind_dummy(&self, vm: &mut Vm) -> Result<(), MissionError> {
        let contexts = enum_names(vm, "Engine", "BehaviorBase.EBehaviorContext")?;
        let transforms = enum_names(vm, "WillowGame", "AIPawnBalanceDefinition.EAITransformed")?;
        let operations = enum_names(vm, "WillowGame", "Behavior_SimpleMath.EBinaryMathOperation")?;
        let actions = enum_names(vm, "Engine", "ITargetable.EChangeStatus")?;
        let mut slot = self.inner.dummy.borrow_mut();
        let d = slot.as_mut().ok_or_else(busy)?;

        // Behavior_UpdateMissionObjective is script (NATIVE_OBJECTIVE_TRIGGERS.md): it runs on the VM and reaches the
        // tracker's UpdateObjective. Its context would be the dummy pawn, which has no VM object here; None casts to no
        // IMissionObjective, as a pawn does, so the bit is 0 either way. The provider is lent back while it runs, so
        // the tracker's notifications reach the dummy's conditions.
        let w = self.weak();
        d.handle(
            "WillowGame.Behavior_UpdateMissionObjective",
            Handler::run(move |vm, p, id, _| {
                let s = Self::upgrade(&w)?;
                let o = p.behavior(id).object;
                tracker::lend(&s.inner.dummy, &s.inner.dummy_lost, p, || {
                    s.inner.script.apply_behavior(vm, o, None)
                })?;
                Ok(Vec::new())
            }),
        );
        // Behavior_CompareObject runs through the world handlers' built-in one (inputs from the variable data).
        let w = self.weak();
        d.handle(
            "WillowGame.Behavior_AttemptStatusEffect",
            Handler::run(move |vm, p, id, _| {
                let s = Self::upgrade(&w)?;
                let o = p.behavior(id).object;
                let effect = ref_path(vm, prop(&vm.space, o, "StatusEffect")?.as_ref())?;
                s.push(HostEvent::new(HostKind::StatusEffect, effect, "", ""));
                Ok(Vec::new())
            }),
        );
        // World ops with no binding yet: reported with the decoded fields the host needs to run them.
        let context = Rc::new(move |vm: &Vm, p: &Provider, id: BehaviorId| -> Result<String, BehaviorError> {
            let v = prop(&vm.space, p.behavior(id).object, "Context")?;
            Ok(name_of(&contexts, v.as_ref().and_then(|v| field(&vm.space, v, "BehaviorContext"))))
        });
        // Installed script: sets WillowAIPawn.TransformType = Transform on the context's IBodyPawn.
        let (cx, names) = (context.clone(), transforms);
        let describe: DescribeFn = Rc::new(move |vm, p, id| {
            let mut f = DetMap::new();
            let t = prop(&vm.space, p.behavior(id).object, "Transform")?;
            f.insert("Transform".to_string(), name_of(&names, t.as_ref()));
            f.insert("Context".to_string(), cx(vm, p, id)?);
            f.insert("Sets".to_string(), "WillowAIPawn.TransformType".to_string());
            Ok(f)
        });
        d.report_at_boundary("WillowGame.Behavior_Transform", Some(describe), Vec::new());
        // Installed script: calls ITargetable.Behavior_RegisterTargetable(bUnregister) on the context object.
        let cx = context.clone();
        let describe: DescribeFn = Rc::new(move |vm, p, id| {
            let mut f = DetMap::new();
            let u = prop(&vm.space, p.behavior(id).object, "bUnregister")?;
            let unregister = u.is_some_and(|v| v.truth(&vm.space.names));
            f.insert("bUnregister".to_string(), if unregister { "true" } else { "false" }.to_string());
            f.insert("Context".to_string(), cx(vm, p, id)?);
            f.insert("Calls".to_string(), "ITargetable.Behavior_RegisterTargetable".to_string());
            Ok(f)
        });
        d.report_at_boundary("WillowGame.Behavior_RegisterTargetable", Some(describe), Vec::new());
        // OnSpawned: native Behavior_IntMath (no script) computes a value that Behavior_ChangeInstanceDataSwitch hands to
        // IBodyCompositionInstance.ChangeInstanceDataSwitch (installed script): a cosmetic body-composition switch, not
        // the damage-sequence choice. Neither is computed here.
        let describe: DescribeFn = Rc::new(move |vm, p, id| {
            let mut f = DetMap::new();
            let (a, _) = p.int_input(vm, id, "A")?;
            let (b, _) = p.int_input(vm, id, "B")?;
            let op = prop(&vm.space, p.behavior(id).object, "Operation")?;
            f.insert("Operation".to_string(), name_of(&operations, op.as_ref()));
            f.insert("A".to_string(), a.map_or_else(|| "?".to_string(), |v| v.to_string()));
            f.insert("B".to_string(), b.map_or_else(|| "?".to_string(), |v| v.to_string()));
            Ok(f)
        });
        d.report_at_boundary("WillowGame.Behavior_IntMath", Some(describe), Vec::new());
        let cx = context;
        let describe: DescribeFn = Rc::new(move |vm, p, id| {
            let mut f = DetMap::new();
            let (value, linked) = p.int_input(vm, id, "NewValue")?;
            let name = prop(&vm.space, p.behavior(id).object, "SwitchName")?;
            f.insert("SwitchName".to_string(), text(&vm.space, name.as_ref()));
            let shown = if linked {
                "variable (written by a behavior not run here)".to_string()
            } else {
                value.map_or_else(|| "?".to_string(), |v| v.to_string())
            };
            f.insert("NewValue".to_string(), shown);
            f.insert("Context".to_string(), cx(vm, p, id)?);
            f.insert("Calls".to_string(), "IBodyCompositionInstance.ChangeInstanceDataSwitch".to_string());
            Ok(f)
        });
        d.report_at_boundary("WillowGame.Behavior_ChangeInstanceDataSwitch", Some(describe), Vec::new());
        // A special-move (animation) request on the pawn: its installed script asks the special-move component to play
        // the move and selects output 0 at once; output 1 follows when the move has ended (no animation system here).
        d.report_at_boundary("GearboxFramework.Behavior_SpecialMove", None, vec![0]);
        // Behavior_AIHold is installed script that reaches the context's AI component through IAIInterface. The C++
        // records it as a no-op before the dummy has a mind (Reference, R3). Off: the script runs on each context
        // through the VM, a pawn context answering by Pawn's rule (module documentation; decisions 2026-11-16-74, 2026-11-19-62).
        let w = self.weak();
        d.handle(
            HOLD_BEHAVIOR,
            Handler::run(move |vm, p, id, _| {
                if vm.parity == ParityMode::Reference {
                    return Ok(Vec::new());
                }
                let s = Self::upgrade(&w)?;
                let pawn = s.inner.state.borrow().dummy_pawn;
                let Some(pawn) = pawn else {
                    return Ok(Vec::new());
                };
                let contexts = p.contexts_for(vm, id, pawn)?;
                // The event that started this behavior has written its state: a refusal here is terminal.
                ow_ai::awareness::holds::apply(vm, p.behavior(id).object, &contexts)
                    .map_err(ow_combat::holds::after_partial_writes)?;
                Ok(Vec::new())
            }),
        );
        let w = self.weak();
        d.handle(
            "Engine.Behavior_RemoteEvent",
            Handler::run(move |vm, p, id, _| {
                let s = Self::upgrade(&w)?;
                let name = prop(&vm.space, p.behavior(id).object, "EventName")?;
                s.push(HostEvent::new(HostKind::RemoteEvent, text(&vm.space, name.as_ref()), "", ""));
                Ok(Vec::new())
            }),
        );
        let w = self.weak();
        d.handle(
            "GearboxFramework.Behavior_ChangeRemoteBehaviorSequenceState",
            Handler::run(move |vm, p, id, _| {
                let s = Self::upgrade(&w)?;
                let o = p.behavior(id).object;
                let path = prop(&vm.space, o, "ProviderDefinitionPathName")?;
                let components = path.as_ref().and_then(|v| field_value(&vm.space, v, "PathComponentNames"));
                let leaf = provider_path_leaf(&vm.space, components.as_ref());
                let sequence = prop(&vm.space, o, "SequenceName")?;
                if components.is_some() && leaf == s.inner.dummy_name && sequence.is_some() {
                    let action = name_of(&actions, prop(&vm.space, o, "Action")?.as_ref());
                    let sequence = text(&vm.space, sequence.as_ref());
                    s.change_sequence(vm, p, &sequence, &action)?;
                } else {
                    p.errors.push(format!("sequence change targets another provider: {leaf}"));
                }
                Ok(Vec::new())
            }),
        );
        Ok(())
    }

    // ---- the slots and the records ---------------------------------------------------------------------------------

    fn push(&self, e: HostEvent) {
        self.inner.state.borrow_mut().events.push(e);
    }

    fn error(&self, line: String) {
        self.inner.state.borrow_mut().errors.push(line);
    }

    /// An error a hook met: its `NotYet` (if any) is listed, and the first one is kept for the public call.
    /// What a notification hook does with its conditions' result (review r3 of #439, item 3): a terminal stop
    /// ("stopped after partial writes") is returned at once, so the tracker stops before its next event; any other
    /// error is kept for the step to report, as before.
    fn settle_conditions(&self, result: Result<(), MissionError>) -> Result<(), MissionError> {
        match result {
            Err(e) if is_terminal(&e) || self.inner.script.partial_stop().is_some() => Err(e),
            Err(e) => {
                self.keep(e);
                Ok(())
            }
            Ok(()) => Ok(()),
        }
    }

    fn keep(&self, e: MissionError) {
        let mut s = self.inner.state.borrow_mut();
        if let Some(n) = e.not_yet() {
            s.not_yet.push(n.clone());
        }
        if s.pending.is_none() {
            s.pending = Some(e);
        }
    }

    /// The kept hook error, if any (module documentation, "Errors").
    fn settle(&self) -> Result<(), MissionError> {
        match self.inner.state.borrow_mut().pending.take() {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// A `NotYet` the slice itself answers: listed, then returned.
    fn refuse(&self, n: NotYet) -> MissionError {
        self.inner.state.borrow_mut().not_yet.push(n.clone());
        n.into()
    }

    /// Runs `f` on the dummy's provider taken out of its slot, and puts it back whatever `f` returns.
    fn with_dummy<T>(
        &self,
        vm: &mut Vm,
        f: impl FnOnce(&mut Provider, &mut Vm) -> Result<T, MissionError>,
    ) -> Result<T, MissionError> {
        if self.inner.dummy_lost.get() {
            return Err(lost());
        }
        let taken = self.inner.dummy.borrow_mut().take();
        let Some(mut p) = taken else {
            return Err(busy());
        };
        let result = f(&mut p, vm);
        *self.inner.dummy.borrow_mut() = Some(p);
        result
    }

    /// Reads the dummy's provider in its slot.
    fn read_dummy<T>(&self, f: impl FnOnce(&Provider) -> T) -> Result<T, MissionError> {
        if self.inner.dummy_lost.get() {
            return Err(lost());
        }
        let slot = self.inner.dummy.borrow();
        slot.as_ref().map(f).ok_or_else(busy)
    }

    // ---- the rules ---------------------------------------------------------------------------------------------------

    /// `Behavior_ChangeRemoteBehaviorSequenceState.Action` on the dummy's own provider `p` (module documentation).
    fn change_sequence(&self, vm: &mut Vm, p: &mut Provider, sequence: &str, action: &str) -> Result<(), MissionError> {
        self.push(HostEvent::new(HostKind::SequenceChange, self.inner.dummy_name.clone(), sequence, action));
        match action {
            "CHANGE_Enable" => {
                p.set_sequence_enabled(vm, sequence, true)?;
            }
            "CHANGE_Disable" => {
                p.set_sequence_enabled(vm, sequence, false)?;
            }
            // UNVERIFIED: that Toggle flips the current state (the script hands the action to a native).
            "CHANGE_Toggle" => {
                let now = p.sequence_enabled(sequence);
                p.set_sequence_enabled(vm, sequence, !now)?;
            }
            other => self.error(format!("unknown sequence change action '{other}' for {sequence}")),
        }
        Ok(())
    }

    /// One `BehaviorSequenceEnableByMission` verdict from the tracker's current state (module documentation).
    fn condition_holds(&self, vm: &mut Vm, condition: ObjRef) -> Result<bool, MissionError> {
        let m = &self.inner.mission;
        let linked = ref_path(vm, prop(&vm.space, condition, "LinkedMission")?.as_ref())?;
        let own = linked == m.path();
        let truth = |vm: &Vm, v: Option<&Value>| v.is_some_and(|v| v.truth(&vm.space.names));
        let specific = truth(vm, prop(&vm.space, condition, "bIsObjectiveSpecific")?.as_ref());
        let mut verdict = if !specific {
            let state = if own {
                STATUS_FIELDS[usize::from(m.status().index())]
            } else if self.inner.state.borrow().completed.contains(&linked) {
                "bComplete"
            } else {
                "bNotStarted"
            };
            let states = prop(&vm.space, condition, "MissionStatesToLinkTo")?;
            truth(vm, states.as_ref().and_then(|v| field(&vm.space, v, state)))
        } else {
            let objective = ref_path(vm, prop(&vm.space, condition, "LinkedObjective")?.as_ref())?;
            if objective.is_empty() {
                return Ok(false);
            }
            let state = if own { m.objective_state(&objective) } else { "NotStarted" };
            if state.is_empty() {
                self.error(format!("enable condition names an objective outside its mission: {objective}"));
                return Ok(false);
            }
            let states = prop(&vm.space, condition, "ObjectiveStatesToLinkTo")?;
            let bit = format!("b{state}");
            truth(vm, states.as_ref().and_then(|v| field(&vm.space, v, &bit)))
        };
        if verdict
            && let Some(Value::Array(list)) = prop(&vm.space, condition, "ObjectiveSetRestrictions")?
            && !list.is_empty()
        {
            verdict = false;
            if own {
                for set in list.iter() {
                    if m.set_active(&ref_path(vm, Some(set))?) {
                        verdict = true;
                        break;
                    }
                }
            }
        }
        Ok(verdict)
    }

    /// What every mission notification does to the registered dummy (module documentation).
    fn apply_conditions(&self, vm: &mut Vm) -> Result<(), MissionError> {
        {
            let mut s = self.inner.state.borrow_mut();
            if !s.registered {
                return Ok(());
            }
            s.notify_depth += 1;
            if s.notify_depth > SETTLE {
                s.errors.push("mission notifications did not settle".to_string());
                s.notify_depth -= 1;
                return Ok(());
            }
        }
        let result = self.apply_each(vm);
        self.inner.state.borrow_mut().notify_depth -= 1;
        result
    }

    fn apply_each(&self, vm: &mut Vm) -> Result<(), MissionError> {
        let names = self.read_dummy(|p| p.sequence_names())?;
        for name in names {
            let Some(condition) = self.read_dummy(|p| p.enable_condition(&name))? else {
                continue;
            };
            let class = class_path_of(vm, condition)?;
            if class != CONDITION_CLASS {
                let line = format!("unsupported enable condition class {class}");
                let mut s = self.inner.state.borrow_mut();
                if !s.errors.contains(&line) {
                    s.errors.push(line);
                }
                continue;
            }
            let verdict = self.condition_holds(vm, condition)?;
            self.with_dummy(vm, |p, vm| p.set_sequence_enabled(vm, &name, verdict).map(|_| ()))?;
        }
        Ok(())
    }

    /// Mission effects routed: sequence changes for the dummy go to its provider, the rest to the host. True when
    /// there was any.
    fn drain_mission(&self, vm: &mut Vm) -> Result<bool, MissionError> {
        let effects = self.inner.mission.drain();
        let any = !effects.is_empty();
        for e in effects {
            let event = |kind| HostEvent::new(kind, e.a.clone(), "", "");
            let triple = |kind| HostEvent::new(kind, e.a.clone(), e.b.clone(), e.c.clone());
            match e.kind {
                EffectKind::SetSequence => {
                    if e.a == self.inner.dummy_name {
                        self.with_dummy(vm, |p, vm| self.change_sequence(vm, p, &e.b, &e.c))?;
                    } else {
                        self.error(format!("mission sequence change targets an unbound provider: {}", e.a));
                    }
                }
                EffectKind::RemoteEvent => {
                    self.push(HostEvent::new(HostKind::RemoteEvent, e.a.clone(), self.inner.mission.path(), ""))
                }
                EffectKind::Dialog => {
                    let mut d = triple(HostKind::Dialog);
                    d.detail = e.detail.clone();
                    self.push(d);
                }
                EffectKind::MissionWeaponGranted => self.push(event(HostKind::MissionWeaponGranted)),
                EffectKind::MissionWeaponRemoved => self.push(event(HostKind::MissionWeaponRemoved)),
                EffectKind::Reward => self.push(event(HostKind::Reward)),
                EffectKind::StatusChanged => self.push(event(HostKind::Status)),
                EffectKind::ObjectiveSetActive => self.push(event(HostKind::ObjectiveSet)),
                EffectKind::ObjectiveComplete => self.push(event(HostKind::ObjectiveComplete)),
                EffectKind::TurnInDialog => self.push(triple(HostKind::TurnInDialog)),
                EffectKind::DismissalDialog => self.push(triple(HostKind::DismissalDialog)),
                EffectKind::InterfaceClosed => self.push(event(HostKind::MissionInterfaceClosed)),
                EffectKind::RewardPage => self.push(event(HostKind::RewardPage)),
                EffectKind::RewardReceived => self.push(event(HostKind::RewardReceived)),
                // Progress only; completion is reported separately.
                EffectKind::ObjectiveUpdated => {}
            }
        }
        Ok(any)
    }

    /// The bridge's experience gains since the last call, as events in order; each level-up's health follows its
    /// skill points (module documentation, the health pool stand-in).
    fn collect_gains(&self, vm: &mut Vm) -> Result<(), MissionError> {
        for gain in self.inner.script.take_gains() {
            let event = match gain {
                Gain::Experience(n) => HostEvent::new(HostKind::Experience, n.to_string(), "", ""),
                Gain::Level(n) => HostEvent::new(HostKind::Level, n.to_string(), "", ""),
                Gain::SkillPoints(n) => HostEvent::new(HostKind::SkillPoints, n.to_string(), "", ""),
            };
            let level_up = event.kind == HostKind::SkillPoints;
            self.inner.state.borrow_mut().gains.push(event);
            if level_up && let Some((maximum, current)) = self.health_after_level_up(vm)? {
                let f = ow_core::fmt::fmt_to_string_f32;
                let mut s = self.inner.state.borrow_mut();
                s.gains.push(HostEvent::new(HostKind::MaxHealth, f(maximum), "", ""));
                s.gains.push(HostEvent::new(HostKind::Health, f(current), f(maximum), ""));
            }
        }
        Ok(())
    }

    /// The C++ `pump`: the experience events, then the mission's effects until quiet (at most 16 rounds).
    fn pump(&self, vm: &mut Vm) -> Result<(), MissionError> {
        self.collect_gains(vm)?;
        {
            let mut s = self.inner.state.borrow_mut();
            let gains = std::mem::take(&mut s.gains);
            s.events.extend(gains);
        }
        for _ in 0..SETTLE {
            if !self.drain_mission(vm)? {
                return self.settle();
            }
        }
        self.error("mission/dummy state did not settle".to_string());
        self.settle()
    }

    // ---- the health pool stand-in (Reference only) ------------------------------------------------------------------

    /// The player's class: the host's Maya, found by its stock path in the mission package.
    fn player_class(&self, vm: &mut Vm) -> Result<Option<ObjRef>, MissionError> {
        tracker::find_object(vm, &self.inner.mission_package, bridge::PLAYER_CLASS_PATH)
    }

    /// Whether the player's class names a health pool (what the C++ builds its pool from).
    fn names_health_pool(&self, vm: &mut Vm) -> Result<bool, MissionError> {
        let Some(class) = self.player_class(vm)? else {
            return Ok(false);
        };
        Ok(matches!(prop(&vm.space, class, "HealthPoolDefinition")?, Some(Value::Object(Some(_)))))
    }

    fn health_unread() -> NotYet {
        NotYet::not_read(
            "the player's health pool (the game builds it in the class-defaults routine; the C++ builds one by hand)",
            HEALTH_RECORD,
        )
    }

    /// The maximum and current health after a level-up: the stand-in pool's (Reference), else `NotYet` when the
    /// class names a health pool, else nothing.
    fn health_after_level_up(&self, vm: &mut Vm) -> Result<Option<(f32, f32)>, MissionError> {
        if vm.parity != ParityMode::Reference {
            if self.names_health_pool(vm)? {
                return Err(self.refuse(Self::health_unread()));
            }
            return Ok(None);
        }
        let Some(h) = self.inner.state.borrow().health else {
            return Ok(None);
        };
        let maximum = number(&required(vm, h.pool, "MaxValue")?);
        let current = number(&required(vm, h.pool, "CurrentValue")?);
        // The pool's fields are floats: the C++ `float(number())`.
        Ok(Some((maximum as f32, current as f32)))
    }

    /// The C++ `ensureHealthPool` (Reference only): the class of the resource's `DefaultResourcePoolClass` over the
    /// class's `HealthPoolDefinition`, in a `ResourcePoolManager` on the controller, the controller as the pool's
    /// provider. False (and no health events) when the packages do not hold that data, as the C++.
    fn ensure_health_pool(&self, vm: &mut Vm) -> bool {
        if self.inner.state.borrow().health.is_some() {
            return true;
        }
        if self.inner.state.borrow().health_failed {
            return false;
        }
        let made = self.build_health_pool(vm);
        let mut s = self.inner.state.borrow_mut();
        match made {
            Ok(Some(pool)) => {
                s.health = Some(pool);
                true
            }
            // The C++ catches its data errors here and has no pool (no health events), as for missing data.
            _ => {
                s.health_failed = true;
                false
            }
        }
    }

    fn build_health_pool(&self, vm: &mut Vm) -> Result<Option<HealthPool>, MissionError> {
        let Some(class) = self.player_class(vm)? else {
            return Ok(None);
        };
        let reference = prop(&vm.space, class, "HealthPoolDefinition")?;
        let Some(definition) = tracker::load_ref(vm, reference.as_ref())? else {
            return Ok(None);
        };
        let resource = prop(&vm.space, definition, "Resource")?;
        let Some(resource) = tracker::load_ref(vm, resource.as_ref())? else {
            return Ok(None);
        };
        // A Class value, or an object stand-in for the class export (a reference into another package).
        let pool_class = match prop(&vm.space, resource, "DefaultResourcePoolClass")? {
            Some(Value::Class(Some(c))) => Some(c),
            Some(Value::Object(Some(o))) => Some(vm.find_class(&stock_path(vm, o)?)?),
            _ => None,
        };
        let Some(pool_class) = pool_class else {
            return Ok(None);
        };
        let controller = self.inner.script.player_controller();
        let manager_class = vm.find_class("Engine.ResourcePoolManager")?;
        let manager = vm.instantiate(manager_class, "")?;
        // As CreateResourcePoolManagerIfNecessary leaves a manager: the owner's authority, and the owner (a package
        // set whose manager class lacks them keeps neither).
        if let Some(role) = prop(&vm.space, controller, "Role")?
            && prop(&vm.space, manager, "Role")?.is_some()
        {
            set_prop(vm, manager, "Role", role)?;
        }
        if prop(&vm.space, manager, "Owner")?.is_some() {
            set_prop(vm, manager, "Owner", Value::Object(Some(controller)))?;
        }
        let pool = vm.instantiate(pool_class, "")?;
        let slots = required(vm, manager, "ResourcePools")?;
        let class = vm.space.heap.get(manager)?.class;
        let id = vm
            .space
            .prop(class, "ResourcePools")
            .ok_or_else(|| data("mission script bridge: the manager has no ResourcePools".to_string()))?;
        match slots {
            Value::Array(list) => {
                let mut list = list.as_ref().clone();
                if list.is_empty() {
                    list.push(Value::Object(None));
                }
                list[0] = Value::Object(Some(pool));
                vm.space.write(manager, PropAddr::scalar(id), Value::array(list))?;
            }
            // A static array: its first slot.
            _ => {
                vm.space.write(manager, PropAddr::scalar(id), Value::Object(Some(pool)))?;
            }
        }
        set_prop(vm, pool, "Definition", Value::Object(Some(definition)))?;
        set_prop(vm, pool, "AssociatedProvider", Value::Object(Some(controller)))?;
        set_prop(vm, pool, "bIsAuthoritative", Value::Bool(true))?;
        set_prop(vm, controller, "ResourcePoolManager", Value::Object(Some(manager)))?;
        // The C++ catches any failure of the link itself and keeps the pool.
        let _ = self.link_player_health(vm, manager, pool);
        Ok(Some(HealthPool { pool }))
    }

    /// The C++ `linkPlayerHealth` (Reference only, with the pool): the player pawn's `HealthPool` reference (section
    /// 1.1 of NATIVE_LEVEL_UP_ATTRIBUTES.md: manager, slot, GUID byte, cached pool) to the built pool, slot 0 and the
    /// pool's own GUID, so the pawn's health reads answer from it. The C++ links at the pawn's creation and at the
    /// pool's, whichever comes second; here the bridge is asked for the player pawn when the pool is built (one pawn
    /// either way, made earlier than the C++ would make it). A pawn class without the reference, or a package set
    /// without the reference struct, keeps no link, as the C++.
    fn link_player_health(&self, vm: &mut Vm, manager: ObjRef, pool: ObjRef) -> Result<(), MissionError> {
        let pawn = self.inner.script.player_pawn(vm)?;
        let class = vm.space.heap.get(pawn)?.class;
        let Some(health) = vm.space.prop(class, "HealthPool") else {
            return Ok(());
        };
        let Some(ty) = vm.space.classes.struct_by_path("Engine.Actor.ResourcePoolReference") else {
            return Ok(());
        };
        // The C++ `makeByte(guid->integer())`: the low byte of the pool's GUID (0 when it has none).
        let guid = prop(&vm.space, pool, "PoolGUID")?.map_or(0, |v| integer(&v));
        let guid = u8::try_from(guid & 0xFF).unwrap_or(0);
        let reference = ow_world::pools::ref_value(
            &vm.space,
            ty,
            ow_world::pools::PoolRef::new(Some(manager), 0, guid, Some(pool)),
        )?;
        vm.space.write(pawn, PropAddr::scalar(health), reference)?;
        Ok(())
    }

    /// The C++ `seedHealthPool` (Reference only): the pool for the player's level, its base maximum from the
    /// definition's `BaseMaxValue` (the controller as the source), an empty modifier stack, the minimum from the
    /// definition, current as given (negative: the maximum; else kept between the minimum and the maximum, the C++
    /// stand-in), then the recorded values. A failure is the C++ script error line.
    fn seed_health_pool(&self, vm: &mut Vm, current: f32) {
        let Some(h) = self.inner.state.borrow().health else {
            return;
        };
        if let Err(e) = self.seed(vm, h.pool, current) {
            self.error(format!("mission script: {}", error_text(&e)));
        }
    }

    fn seed(&self, vm: &mut Vm, pool: ObjRef, current: f32) -> Result<(), MissionError> {
        let definition = match required(vm, pool, "Definition")? {
            Value::Object(Some(d)) => d,
            _ => return Err(data("mission script bridge: the health pool has no definition".to_string())),
        };
        let base = prop(&vm.space, definition, "BaseMaxValue")?
            .ok_or_else(|| data("mission script bridge: the health pool definition has no BaseMaxValue".to_string()))?;
        let d = init_data(&vm.space, &base).map_err(|e| BehaviorError::Vm(VmError::from(e)))?;
        let controller = self.inner.script.player_controller();
        let sys = vm.attrs.clone();
        let maximum = sys
            .evaluate_init(&d, Some(controller), None, &mut EvalCx::new(vm))
            .map_err(|e| BehaviorError::Vm(VmError::from(e)))?;
        let minimum = match prop(&vm.space, definition, "BaseMinValue")? {
            // The C++ `float(number())`.
            Some(v) => number(&v) as f32,
            None => 0.0,
        };
        set_prop(vm, pool, "MaxValueBaseValue", Value::Float(maximum))?;
        set_prop(vm, pool, "MaxValue", Value::Float(maximum))?;
        set_prop(vm, pool, "MinValueBaseValue", Value::Float(minimum))?;
        set_prop(vm, pool, "MinValue", Value::Float(minimum))?;
        let value = if current < 0.0 { maximum } else { minimum.max(current.min(maximum)) };
        set_prop(vm, pool, "CurrentValue", Value::Float(value))?;
        ow_world::pools::update_last_values(&mut vm.space, pool)?;
        Ok(())
    }

    // ---- the host's calls -----------------------------------------------------------------------------------------------

    pub fn mission(&self) -> &MissionSystem {
        &self.inner.mission
    }

    pub fn script(&self) -> &MissionScript {
        &self.inner.script
    }

    /// The dialog the tracker and the director's component share.
    pub fn dialog(&self) -> Rc<RefCell<DialogSystem>> {
        self.inner.mission.dialog()
    }

    /// The use key's press on the director (after the host's use ray chose him): his stock `OnUsed` chain on the VM.
    /// The result says whether it reached the mission interface (a `MissionInterface` event is queued) and lists the
    /// behaviors it ran.
    pub fn use_marcus(&self, vm: &mut Vm, completed: &DetSet<String>) -> Result<MarcusUse, MissionError> {
        self.inner.state.borrow_mut().completed = completed.clone();
        let used = self.inner.script.use_marcus(vm, completed)?;
        if !used.on_use_tag.is_empty() {
            self.push(HostEvent::new(
                HostKind::OnUseDialog,
                used.on_use_tag.clone(),
                used.on_use_speaker.clone(),
                used.on_use_target.clone(),
            ));
        }
        if used.interface_opened {
            self.push(HostEvent::new(HostKind::MissionInterface, used.movie.clone(), used.director.clone(), ""));
        }
        self.pump(vm)?;
        Ok(used)
    }

    /// The mission screen's three lists (the director's own scripts).
    pub fn screen(&self, vm: &mut Vm, completed: &DetSet<String>) -> Result<MissionLists, MissionError> {
        self.inner.state.borrow_mut().completed = completed.clone();
        self.inner.script.mission_lists(vm, completed)
    }

    /// The confirm button on the screen's entry: with the director placed the entry must be one his lists offer; then
    /// `AcceptMission` runs as script with him as the director. The kickoff is the pending record the next tick plays.
    pub fn accept(&self, vm: &mut Vm, completed: &DetSet<String>) -> Result<bool, MissionError> {
        self.inner.state.borrow_mut().completed = completed.clone();
        if self.inner.script.has_marcus() {
            let lists = self.inner.script.mission_lists(vm, completed)?;
            if !lists.eligible.iter().any(|m| m == self.inner.mission.path()) {
                return Ok(false);
            }
        }
        let ok = self.inner.script.accept(vm, completed)?;
        self.pump(vm)?;
        Ok(ok)
    }

    /// Escape on the director's mission screen (the accept closes it by itself).
    pub fn close_screen(&self, vm: &mut Vm) -> Result<bool, MissionError> {
        let ok = self.inner.script.close_screen(vm)?;
        self.pump(vm)?;
        Ok(ok)
    }

    /// Enter on the reward page the open screen showed at the turn-in.
    pub fn confirm_reward(&self, vm: &mut Vm) -> Result<bool, MissionError> {
        let ok = self.inner.script.confirm_reward(vm)?;
        self.pump(vm)?;
        Ok(ok)
    }

    /// The host's overlap of the waypoint's cylinder with the player pawn (or the director): begin or end. The stock
    /// waypoint script decides what it means (NATIVE_OBJECTIVE_TRIGGERS.md, UNVERIFIED).
    pub fn touch_waypoint(&self, vm: &mut Vm, player: bool, begin: bool) -> Result<(), MissionError> {
        let who = if player { Toucher::Player } else { Toucher::Marcus };
        if begin {
            self.inner.script.touch(vm, who)?;
        } else {
            self.inner.script.untouch(vm, who)?;
        }
        self.pump(vm)
    }

    /// The player enters the range: through the waypoint when the package has it, else the objective directly. True
    /// when the objective's progress changed.
    pub fn enter_range(&self, vm: &mut Vm) -> Result<bool, MissionError> {
        let before = self.inner.mission.objective_progress(GO_TO_RANGE);
        if self.inner.script.has_waypoint() {
            self.touch_waypoint(vm, true, true)?;
        } else {
            self.inner.mission.update_objective(vm, GO_TO_RANGE, 0)?;
            self.pump(vm)?;
        }
        Ok(self.inner.mission.objective_progress(GO_TO_RANGE) != before)
    }

    /// `OnSpawned` on the dummy, when the host spawns it (module documentation: the registration first, then the
    /// event, then the Kismet variable).
    pub fn spawn_dummy(&self, vm: &mut Vm) -> Result<bool, MissionError> {
        let first = !self.inner.state.borrow().registered;
        if first {
            self.with_dummy(vm, |p, vm| p.register_consumer(vm))?;
            {
                let mut s = self.inner.state.borrow_mut();
                s.registered = true;
                s.notify_depth = 0;
            }
            // The LevelLoad verdict.
            self.apply_conditions(vm)?;
        }
        self.place_dummy_pawn(vm)?;
        self.with_dummy(vm, |p, vm| p.fire_event(vm, "OnSpawned", &DetMap::new(), -1, &[]))?;
        let pawn = self.inner.state.borrow().dummy_pawn;
        self.inner.mission.bind_kismet_variable(TARGET_DUMMY_VARIABLE, pawn)?;
        self.pump(vm)?;
        Ok(true)
    }

    /// The pawn object the VM holds for the dummy: the stock archetype (a copy), registered as a dialog talker under
    /// its body class's dialog name tag. Made once; data without the archetype gives no pawn.
    fn place_dummy_pawn(&self, vm: &mut Vm) -> Result<(), MissionError> {
        if self.inner.state.borrow().dummy_pawn.is_some() {
            return Ok(());
        }
        let (_, store) = vm.defs.code_mut();
        let id = store
            .load(&self.inner.dummy_package)
            .map_err(|e| BehaviorError::from(ow_defs::store::DefError::from(e)))?;
        let Some(index) = store.get(id).find_export(DUMMY_PAWN)?.filter(|&i| i > 0) else {
            return Ok(());
        };
        if vm.parity != ParityMode::Off {
            // slice.cpp:366, `instantiateExport(dummyPackage_, index, 4)`: a new object, its references the definition
            // store's objects (the body class among them, which the dialog loads). Reference is unchanged.
            let pawn = instantiate_export(vm, id, index, PropertyPrefix::Ordinary)?;
            self.inner.state.borrow_mut().dummy_pawn = Some(pawn);
            return self.dummy_dialog(vm, pawn);
        }
        // Off (lane DUMMY-COMPONENTS, the hub's spawn ruling; review r1 of #526): the dummy gets its own copy of each
        // component its archetype owns, the per-actor plan the world's spawn uses. Every write is tracked by the
        // canonical counters (heap writes, generator draws, load transitions, #518): a refusal is marked "stopped
        // after partial writes: " exactly when one of them moved since the placement began. The dummy is published
        // only once the whole placement, its dialog included, has succeeded, so a retry does the setup again.
        let start = WriteCounters::of(vm);
        let mark = |vm: &Vm, e: MissionError| match e {
            BehaviorError::Vm(v) if start.moved(vm) => BehaviorError::Vm(ow_world::pools::after_partial_writes(v)),
            other => other,
        };
        let placed = (|| -> Result<ObjRef, MissionError> {
            // The pawn first, as the C++ makes it: the plan is then worked out on the very slots the pawn received
            // (the export decoded with no overlay), its identity the archetype's export.
            let pawn = instantiate_export(vm, id, index, PropertyPrefix::Ordinary)?;
            let plan = ow_world::actors::plan_components_tracked(vm, pawn, &mut false)?;
            // Each copy is outered to the pawn and re-pointed in its slots. A copy whose class declares `Owner` gets
            // the pawn there, as the game's attach step sets it (UNVERIFIED in this game; the port does not model that
            // step elsewhere).
            for (_, copy) in ow_world::actors::apply_plan(vm, &plan, pawn)? {
                let class = vm.space.heap.get(copy)?.class;
                if let Some(p) = vm.space.prop(class, "Owner") {
                    vm.space.write(copy, PropAddr::scalar(p), Value::Object(Some(pawn))).map_err(VmError::from)?;
                }
            }
            self.dummy_dialog(vm, pawn)?;
            Ok(pawn)
        })();
        let pawn = placed.map_err(|e| mark(vm, e))?;
        self.inner.state.borrow_mut().dummy_pawn = Some(pawn);
        Ok(())
    }

    /// The dummy's dialog: its body class's talker registered under its name tag.
    fn dummy_dialog(&self, vm: &mut Vm, pawn: ObjRef) -> Result<(), MissionError> {
        let body = prop(&vm.space, pawn, "BodyClass")?.unwrap_or(Value::None);
        self.inner.mission.with_dialog(|d| -> Result<(), MissionError> {
            let dialog = d.pawn_dialog(vm, &body)?;
            if !dialog.name_tag.is_empty() {
                d.register_talker(&dialog.name_tag, Some(pawn));
            }
            Ok(())
        })?
    }

    /// `OnTakeDamage` on the dummy: `damage_type` is the stock path of the damage type the host dealt (`""` for None);
    /// it and `damage_source` are written to the variables the event links as its outputs. With `damage` >= 0 and a
    /// pawn for the dummy the C++ runs the player's hit through the stock pawn chain: not wired here yet (W6-COMBAT-RUN2),
    /// `NotYet`.
    pub fn damage_dummy(
        &self,
        vm: &mut Vm,
        damage_type: &str,
        damage_source: &str,
        damage: f64,
    ) -> Result<bool, MissionError> {
        // The C++ `damage < 0 || !dummyPawn_`: a NaN damage takes the pawn path there too.
        let direct = damage < 0.0 || self.inner.state.borrow().dummy_pawn.is_none();
        if !direct {
            return Err(self.refuse(NotYet::not_ported(
                format!("the player's hit on the dummy pawn through the stock TakeDamage chain ({COMBAT_FOLLOW_UP})"),
                RECORD,
            )));
        }
        let mut outputs = DetMap::new();
        outputs.insert("DamageType".to_string(), damage_type.to_string());
        outputs.insert("DamageSource".to_string(), damage_source.to_string());
        self.preflight_holds(vm, "OnTakeDamage")?;
        self.with_dummy(vm, |p, vm| p.fire_event(vm, "OnTakeDamage", &outputs, -1, &[]))?;
        self.pump(vm)?;
        Ok(true)
    }

    /// Off, with a pawn for the dummy: when the behavior firing `event` runs first is a hold whose context is fixed
    /// ([`Provider::certain_first_behavior`]), it is checked on that context before the firing writes anything (module
    /// documentation). Writes nothing.
    fn preflight_holds(&self, vm: &mut Vm, event: &str) -> Result<(), MissionError> {
        if vm.parity != ParityMode::Off {
            return Ok(());
        }
        let Some(pawn) = self.inner.state.borrow().dummy_pawn else {
            return Ok(());
        };
        self.with_dummy(vm, |p, vm| {
            if let Some(id) = p.certain_first_behavior(event, -1)
                && p.behavior(id).class == HOLD_BEHAVIOR
            {
                let contexts = p.contexts_for(vm, id, pawn)?;
                ow_combat::holds::preflight_hold_contexts(vm, &contexts)?;
            }
            Ok(())
        })
    }

    /// How many times the dummy's `event` has triggered (tests: the event state a refusal leaves).
    #[cfg(test)]
    fn dummy_event_triggers(&self, event: &str) -> i64 {
        self.inner.dummy.borrow().as_ref().map_or(0, |p| p.event_trigger_count(event))
    }

    /// The legacy host call: `true` is the incendiary impact damage type, `false` None.
    pub fn hit_dummy(&self, vm: &mut Vm, fire: bool) -> Result<bool, MissionError> {
        self.damage_dummy(vm, if fire { INCENDIARY_IMPACT } else { "" }, "", -1.0)
    }

    /// `ServerCompleteMission` as script with the director: the entry must be one his lists offer as redeemable (or,
    /// without him, the mission must be able to end).
    pub fn turn_in(&self, vm: &mut Vm) -> Result<bool, MissionError> {
        if self.inner.script.has_marcus() {
            let completed = self.inner.state.borrow().completed.clone();
            let lists = self.inner.script.mission_lists(vm, &completed)?;
            if !lists.redeemable.iter().any(|m| m == self.inner.mission.path()) {
                return Ok(false);
            }
        } else if !self.inner.mission.can_end() {
            return Ok(false);
        }
        let ok = self.inner.script.turn_in(vm)?;
        self.pump(vm)?;
        Ok(ok)
    }

    /// One not-natural level-up (`ExpLevelUp(true)`).
    pub fn cheat_level_up(&self, vm: &mut Vm) -> Result<(), MissionError> {
        self.inner.script.cheat_level_up(vm)?;
        self.collect_gains(vm)?;
        self.pump(vm)
    }

    /// The region's game stage (set before accept; it is locked when the mission becomes Active).
    pub fn set_region_game_stage(&self, stage: i32) {
        self.inner.script.set_region_game_stage(stage);
    }

    /// The player's level and experience; in Reference mode the stand-in health pool is built and seeded full.
    pub fn set_player_experience(&self, vm: &mut Vm, level: i32, experience: i64) -> Result<(), MissionError> {
        self.inner.script.set_player_experience(vm, level, experience)?;
        if vm.parity == ParityMode::Reference && self.ensure_health_pool(vm) {
            self.seed_health_pool(vm, -1.0);
        }
        Ok(())
    }

    /// The host's current health (after `set_player_experience`): the stand-in pool is reseeded with it (Reference);
    /// `NotYet` in Off mode (module documentation).
    pub fn set_player_health(&self, vm: &mut Vm, current: f32) -> Result<(), MissionError> {
        if vm.parity != ParityMode::Reference {
            return Err(self.refuse(Self::health_unread()));
        }
        if self.ensure_health_pool(vm) {
            self.seed_health_pool(vm, current);
        }
        Ok(())
    }

    /// The tick: the tracker (the kickoff record, the provider's delayed links), the game time, the experience pool's
    /// per-frame update, the dummy's delayed links, then the drain (the C++ order).
    pub fn tick(&self, vm: &mut Vm, seconds: f64) -> Result<(), MissionError> {
        self.inner.mission.tick(vm, seconds)?;
        self.advance_time(vm, seconds)?;
        self.inner.script.update_experience_pool(vm)?;
        self.collect_gains(vm)?;
        self.with_dummy(vm, |p, vm| p.tick(vm, seconds))?;
        self.pump(vm)
    }

    /// The C++ `advanceTime`: `WorldInfo.TimeSeconds` grows by `seconds` (read by `OnExpLevelChange`'s 1 s guard). The
    /// world's own tick is not run (it ticks actors, which the C++ slice does not).
    fn advance_time(&self, vm: &mut Vm, seconds: f64) -> Result<(), MissionError> {
        let Some(wi) = self.inner.world_info else {
            return Ok(());
        };
        if let Some(t) = prop(&vm.space, wi, "TimeSeconds")? {
            // The C++ `makeFloat(time->number() + seconds)`: the sum in double, kept as a float.
            set_prop(vm, wi, "TimeSeconds", Value::Float((number(&t) + seconds) as f32))?;
        }
        Ok(())
    }

    /// Persistence: the mission state only (the caller keeps the completed set and rewards).
    pub fn save_state(&self) -> String {
        self.inner.mission.save_state()
    }

    /// Restores a saved mission state; the dummy's sequences follow through their conditions (LevelLoad).
    pub fn load_state(&self, vm: &mut Vm, state: &str) -> Result<bool, MissionError> {
        if !self.inner.mission.load_state(state) {
            return Ok(false);
        }
        self.inner.script.sync_restored(vm)?;
        self.apply_conditions(vm)?;
        self.pump(vm)?;
        Ok(true)
    }

    /// The events since the last call, in order.
    pub fn drain(&self) -> Vec<HostEvent> {
        std::mem::take(&mut self.inner.state.borrow_mut().events)
    }

    /// The error lines: the slice's, the tracker's, the bridge's, the dummy provider's (the C++ order).
    pub fn errors(&self) -> Vec<String> {
        let mut all = self.inner.state.borrow().errors.clone();
        all.extend(self.inner.mission.errors());
        all.extend(self.inner.script.errors());
        if let Ok(lines) = self.read_dummy(|p| p.errors.clone()) {
            all.extend(lines);
        }
        all
    }

    /// Every `NotYet` met: the slice's own, the tracker's, the bridge's script runs', the dummy provider's.
    pub fn not_yet(&self) -> Vec<NotYet> {
        let mut all = self.inner.state.borrow().not_yet.clone();
        all.extend(self.inner.mission.not_yet());
        all.extend(self.inner.script.not_yet());
        if let Ok(list) = self.read_dummy(|p| p.not_yet.clone()) {
            all.extend(list);
        }
        all
    }

    // ---- the dummy's report -----------------------------------------------------------------------------------------

    /// The dummy's boundary behaviors reached (`event -> class:name`).
    pub fn dummy_boundary(&self) -> Result<Vec<String>, MissionError> {
        self.read_dummy(|p| p.boundary.clone())
    }

    pub fn dummy_boundary_calls(&self) -> Result<Vec<BoundaryCall>, MissionError> {
        self.read_dummy(|p| p.boundary_calls.clone())
    }

    /// The dummy's enabled sequences, in provider order.
    pub fn dummy_enabled_sequences(&self) -> Result<Vec<String>, MissionError> {
        self.read_dummy(|p| {
            p.sequence_names()
                .into_iter()
                .filter(|n| p.sequence_enabled(n))
                .collect()
        })
    }

    /// `event -> behavior` of the dummy in execution order.
    pub fn dummy_trace(&self) -> Result<Vec<String>, MissionError> {
        self.read_dummy(|p| p.trace.clone())
    }

    /// The dummy's pawn (made at its first spawn; `None` before, or when the data has no archetype).
    pub fn dummy_pawn(&self) -> Option<ObjRef> {
        self.inner.state.borrow().dummy_pawn
    }
}

/// Whether `e` is a stop after partial writes, by its variant (`bridge::is_partial_stop`; review r1 of #464, item 5).
fn is_terminal(e: &MissionError) -> bool {
    crate::bridge::is_partial_stop(e)
}
