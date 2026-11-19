//! Owner: lane W6-MISSION. The mission-script bridge: the script half of accepting and turning in a mission, and the
//! controller's mission records (the port of `src/mission_script.cpp`, `vm::MissionScript`). AI-assisted (Claude
//! Code).
//!
//! In this build the controller functions `AcceptMission`, `ServerCompleteMission`, `UpdateMissionStatus` and
//! `ServerGrantMissionRewards` are script; they call a few natives on the `MissionTracker`. [`MissionScript`] builds
//! the small object graph that script reads (a local controller with authority, the world's `WorldInfo` whose `GRI`
//! is a `WillowGameReplicationInfo` whose `MissionTracker` is a VM tracker object, the installed `MissionDefinition`)
//! and binds those natives onto the [`MissionSystem`], which stays the single owner of the mission's state. The
//! installed script runs unchanged (architecture R2). The rules are NATIVE_MISSION_SCRIPT_BRIDGE.md,
//! NATIVE_PROGRESSION.md sections 2 and 3, NATIVE_CONTROLLER_HELPERS.md, NATIVE_OBJECTIVE_TRIGGERS.md,
//! NATIVE_USE_INTERACTION.md, NATIVE_MARCUS_USE_CHAIN.md and NATIVE_BEHAVIOR_CONTEXT.md, read from native code: all
//! **UNVERIFIED in the running game**, except where a line says it was observed.
//!
//! **What runs where.** The bridge needs the world's natives (`ow-world`, lanes W5-WORLD and W5-POOLS): the level's
//! `WorldInfo` (`BehaviorBase.GetWorldInfo`), `EvaluateInitializationData` (the one evaluator of `ow-attr`), the pool
//! natives and `RecalculateAttributeInitializedState` (the level-up rebase, decision 2026-11-15-29). Those are not
//! bound here again (the VM refuses a second binding): the bridge places its `WorldInfo` as the world's and lets the
//! world's natives answer. The experience curve and the mission reward are `ow-balance`'s (decision 2026-11-15-18).
//!
//! **Natives bound here**, each scoped to the bridge's own objects like the C++ (a call on another object is the
//! unbound behaviour: under the VM's stub policy, `VmError::UnboundNative` for a gameplay command; in
//! `ParityMode::Reference` the C++ `UNIMPLEMENTED <key> (outside the mission bridge binding)` line and the zero result):
//! - the tracker: `ActivateMission`, `CompleteMission`, `PlayTurnIn`, `GetMissionStatus`, `CanStartMission`,
//!   `CanEndMission`, `GetCompletedBranch`, `IsDataValid`, `ValidateData`, `IsMissionObjectiveActive`,
//!   `IsMissionObjectiveComplete`, `IsObjectiveSetActive`, `UpdateObjective`, `RegisterMissionObserver`, and outside
//!   `ParityMode::Reference` `GetObjectivesProgress` (NATIVE_SLICE_OFF_STOPS.md; the C++ stubs it, decision
//!   2026-11-19-05);
//! - the controller: `NativeGetMissionIndex`, `GetCurrentPlaythrough`, `ExpEarn`, `GetMaxExpLevel`,
//!   `GetExpPointsRequiredForLevel`, `GetHUDMovie` (None), `UpdateLcdMissionStatus` and `PlayUIAkEvent`
//!   (presentation: no-ops), `PlayerController.IsPrimaryPlayer`;
//! - the mission: `GetExperienceReward`, `GetGameStage`, `GetCurrencyRewardType`, `GetCurrencyReward`,
//!   `ShouldGrantAlternateReward`, `GetItemRewardsForPlayer` (empty rewards only);
//! - the globals: `GetWillowGlobals`, `GetGearboxGlobals`, `GetBehaviorKernel`, `GetGlobalsDefinition`;
//!   `WorldInfo.IsMenuLevel`, `Actor.IsPlayerOwned`; the pawn's `GetAWillowPawn` and `SetGameStage`;
//! - with the director placed, his use chain (`ResolveBehaviorProviderDefinitionReference`, `GetBehaviorContext`,
//!   the `ClientGFxPlayMovie` override) and the mission screen's stand-in. The behavior kernel's natives
//!   (`ActivateBehaviorOutputLink`, `IsBehaviorSequenceEnabled`, `ActivateBehaviorEventFromScript`,
//!   `GetBehaviorConsumerHandle`) are the kernel pack's (`ow_behavior::kernel::natives`): the bridge claims the
//!   director's handle there and gives its bodies for it, and in Reference also for a handle nobody owns.
//!
//! **Script runs** ([`MissionScript`] keeps the C++ `run`): the outermost run collects what the VM logged while it ran
//! (`UNIMPLEMENTED ...` lines become the stub list, other new lines the notes), and a script that fails is recorded
//! (`mission script: <error>`), never propagated, so the native state the caller already changed stays as it is. A
//! failure that is a `NotYet` is also kept in [`MissionScript::not_yet`]. `--mission-run` is a gameplay command, so its
//! VM refuses an unbound native (`StubPolicy::Refuse`): a native the C++ logs as a stub is an error line here. The
//! bridge's own C++ stubs (the "not implemented" paths of the reward natives, `GetBehaviorContext`'s instance-data
//! path, `RunBehaviors` outside the level-up) keep the C++ stub line only in `ParityMode::Reference` and answer
//! `NotYet` in Off mode (architecture R3; the table of decision 2026-11-15-34).
//!
//! **Not ported, by design or for another lane:**
//! - The pawn health and hit chain (`CombatSystem`, src/combat*.cpp): lane W6-COMBAT. The bridge exposes the
//!   controller and the player's VM pawn ([`MissionScript::player_controller`], [`MissionScript::player_pawn`]).
//! - The level-up's health pool (swap 7b). The C++ builds one pool by hand (a stand-in for the game's class-defaults
//!   routine) and seeds it through its evaluator; the record's way is `CreatePool` (W5-POOLS), which Off mode refuses
//!   on the installed data until `ResetIdleTimer` is read. So no pool is built and no `MaxHealth` / `Health` gains are
//!   reported; [`MissionScript::set_player_health`] is `NotYet`. `--mission-run` prints no health, so its output does
//!   not show this.
//! - `BehaviorBase.RunBehaviors` inside the level-up, the C++ stand-in that applies the OnLevelUp skills' effects by
//!   their data (swaps 7b, 9, 10): `NotYet { NotPorted }` (skills are lane W6-SKILLS). Outside the level-up it is the
//!   C++ log line and nothing.
//! - `GetExperienceReward` for a mission with optional objectives: the C++ logs "not implemented" and leaves their
//!   terms out (kept in Reference mode); in Off mode it is `NotYet { NotPorted }` (`ow_balance::xp::mission_xp` takes
//!   the terms; the bridge does not collect the completed objectives' percentages yet).
//!
//! **The dialog** is the mission's (`MissionSystem::dialog`, lane W5-DIALOG): the director's
//! `GearboxDialogComponent.TriggerEvent` runs the tag on that same `Rc<RefCell<DialogSystem>>` the tracker's dialog
//! behavior runs on, so one dialog state serves both, as the C++ bridge calls `mission_.dialog()` (decision
//! 2026-11-15-34, addendum). `WillowDialogGlobalsDefinition.Get` returns the dialog globals object.
//!
//! **The director is a copy of his archetype**, as in the C++: `GD_Marcus.Character.Pawn_Marcus`, his
//! `MissionDirectives` table, his `AIClass` and his `DialogComponent` are each a new object made from their export on
//! every call, with the C++ prefix (4 bytes; 8 for the component), by `DefStore::instance` (the C++
//! `Runtime::instantiateExport`, `src/mission_script.cpp` `placeMarcus` and `loadRef`). The references decoded inside
//! them are the definition store's objects for their paths, which load when read, as any definition's references do
//! (lane FIX-DIRECTOR-STORE, decision 2026-11-16-48; before it they were decoded outside the store, as stand-ins that
//! could not be read). The director's provider (not an actor) is the store's definition.
//!
//! Every other reference of the director (his `BodyClass` among them) is therefore the store's **shared** definition
//! for its path, not a copy: a write through one would change what every later `DefStore::find` of that path returns
//! (the C++ writes to a stand-in of its own). Nothing here writes to them, and the slice only reads them; code that
//! needs to write to one makes its own object first (`DefStore::instance`; decision 2026-11-16-48, open items).

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use ow_attr::api::EvalCx;
use ow_attr::eval::init_data;
use ow_balance::curve::experience_required;
use ow_balance::types::{
    AwesomeLevel, BalanceError, BalanceSnapshot, ExpLevel, GameStage, LevelCapUpgrades, OpLevel, PlayerCount,
    Playthrough,
};
use ow_balance::xp::{MissionXp, mission_xp};
use ow_behavior::kernel::Provider;
use ow_behavior::kernel::natives;
use ow_behavior::provider::{BehaviorError, enum_names, stock_path};
use ow_behavior::registry::Handler;
use ow_core::{DetMap, DetSet, NotYet, PackageName, ParityMode};
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{FuncId, ObjRef, PropAddr, PropId};
use ow_object::layout::PropKind;
use ow_object::prefix::PropertyPrefix;
use ow_object::value::Value;
use ow_package::PackageId;
use ow_vm::api::{CallResult, NativeCall, NativeFn, Vm};
use ow_vm::error::VmError;
use ow_world::world::{BalanceState, World, WorldError};

use crate::dialog::DialogSystem;
use crate::tracker::{
    self, EffectKind, MissionError, MissionSystem, Notification, field, field_value, integer, integer32, lend, load_ref, prop,
    text,
};

/// The record of the bridge.
pub const RECORD: &str = "NATIVE_MISSION_SCRIPT_BRIDGE.md";

/// The director's stock pawn archetype and his AI-definition provider.
pub const MARCUS_PAWN: &str = "GD_Marcus.Character.Pawn_Marcus";
pub const MARCUS_PROVIDER: &str = "GD_Marcus.Character.AIDef_Marcus.AIBehaviorProviderDefinition_0";
/// The experience curve definition, found by its stock path in the mission's package (as the C++).
pub const CURVE_PATH: &str = "GD_Balance_Experience.Formulas.Init_ExperienceRequiredForLevel";
/// The configured globals data object (UNVERIFIED: its name comes from the notes' prose).
pub const GLOBALS_PATH: &str = "GD_Globals.General.Globals";
/// The player's class (the host's Maya), found by its stock path in the mission's package.
pub const PLAYER_CLASS_PATH: &str = "GD_Siren.Character.CharClass_Siren";
/// The dialog globals the director's on-use script reads.
pub const DIALOG_GLOBALS_PATH: &str = "GD_Globals.Dialog.DialogGlobals";

/// The level cap without DLC (NATIVE_PROGRESSION.md section 3).
pub const MAX_LEVEL: i32 = 50;

/// Where the bridge asks the kernel pack's consumer table for the director's handle: the lowest free `PID` from here
/// (UNVERIFIED: the game's allocator base). The C++ gives him this one; -1 is invalid.
const MARCUS_PID: i32 = 0;

/// This crate's name as the owner of the director's consumer handle (`ow_behavior::kernel::natives`).
pub const CONSUMER_OWNER: &str = "ow-mission";

/// A level-up loop guard (the C++ 200).
const LEVEL_UP_GUARD: usize = 200;

/// The `NextSet` walk of `ShouldGrantAlternateReward` (the note has no loop guard; the C++ stops after 64).
const NEXT_SET_GUARD: usize = 64;

const STUB_PREFIX: &str = "UNIMPLEMENTED ";

/// The layer the mission's reward items need.
const LOOT_LAYER: &str = "ow-combat loot (lane W6-LOOT)";

/// One `ExpEarn` call as the script made it: amount, source, and the type (-1: the optional argument was omitted).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpEarn {
    pub amount: i32,
    pub source: i32,
    pub exp_type: i32,
}

/// What the script's level-up path produced, in order, for the host (the C++ `Gain`; health gains are not produced
/// here, module documentation).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Gain {
    /// What `ExpEarn` added to the pool.
    Experience(i32),
    /// The level the pool update raised the player to.
    Level(i32),
    /// The unspent skill points that level-up awarded (`GeneralSkillPoints` after minus before, sent even when 0).
    SkillPoints(i32),
}

/// The use key's press on the director (the C++ `MarcusUse`).
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MarcusUse {
    pub interface_opened: bool,
    pub movie: String,
    pub director: String,
    /// Every script behavior of the run: `name(sequence) -> selected output ids`.
    pub cascade: Vec<String>,
    /// The on-use dialog's tag (empty: the director's `TriggerEvent` was not called, or it was called with no tag).
    pub on_use_tag: String,
    pub on_use_speaker: String,
    pub on_use_target: String,
    pub on_use_group_count: usize,
    pub on_use_first_group: String,
    pub on_use_last_group: String,
}

/// The mission screen's lists (mission paths, the script's own order).
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MissionLists {
    pub eligible: Vec<String>,
    pub in_progress: Vec<String>,
    pub redeemable: Vec<String>,
}

/// The objects the bridge builds once.
#[derive(Debug, Clone, Copy)]
struct Objects {
    tracker: ObjRef,
    replication: ObjRef,
    world_info: ObjRef,
    controller: ObjRef,
    globals: ObjRef,
    pri: ObjRef,
    authority: u8,
}

#[derive(Debug, Default)]
struct BState {
    completed: DetSet<String>,
    stubs: DetMap<String, usize>,
    notes: Vec<String>,
    errors: Vec<String>,
    not_yet: Vec<NotYet>,
    exp_earned: Vec<ExpEarn>,
    gains: Vec<Gain>,
    pool: f32,
    region_stage: i32,
    locked_stage: i32,
    depth: u32,
    objective_updates: i32,
    observers: Vec<ObjRef>,
    player_pawn: Option<ObjRef>,
    marcus_pawn: Option<ObjRef>,
    director: Option<ObjRef>,
    dialog_component: Option<ObjRef>,
    dialog_globals: Option<ObjRef>,
    globals_definition: Option<Option<ObjRef>>,
    player_class: Option<Option<ObjRef>>,
    curve: Option<ObjRef>,
    in_level_up: bool,
    pawn_game_stage: i64,
    use_: MarcusUse,
    screen_open: bool,
    ui_manager: Option<ObjRef>,
    screen_movie: Option<ObjRef>,
    closing: bool,
    turning_in: bool,
    reward_mission: String,
    waypoint: Option<ObjRef>,
}

struct Bridge {
    mission: MissionSystem,
    package: PackageName,
    o: Objects,
    state: RefCell<BState>,
    /// The director's AI-definition provider, out of its slot while it runs (`crate::tracker::lend`).
    marcus: RefCell<Option<Provider>>,
    has_marcus: Cell<bool>,
    /// The director's consumer handle `PID`, allocated when his use chain is bound.
    marcus_pid: Cell<Option<i32>>,
    /// The director's provider was lost by a failed lend: every later call on it is refused.
    marcus_lost: Cell<bool>,
}

/// The bridge (module documentation). Cheap to clone: every clone is the same bridge.
#[derive(Clone)]
pub struct MissionScript {
    inner: Rc<Bridge>,
}

impl std::fmt::Debug for MissionScript {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MissionScript")
            .field("mission", &self.inner.mission)
            .finish_non_exhaustive()
    }
}

/// Who touches the waypoint.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toucher {
    Player,
    Marcus,
}

// ---------------------------------------------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------------------------------------------

fn data(text: String) -> MissionError {
    BehaviorError::Data(text)
}

/// A world for the bridge: `vm` with the world's services and natives (`ow_world::World::new`). The balance inputs
/// are placeholders the bridge does not read (it keeps its own region stage and playthrough; the region stages of
/// decision 2026-11-15-18 are the host's): normal mode, one player, level 1, stage 1.
pub fn world(vm: Vm) -> Result<World, MissionError> {
    let snapshot = BalanceSnapshot::new(
        Playthrough::NORMAL,
        OpLevel::NONE,
        PlayerCount::SINGLE,
        ExpLevel::new(1),
        GameStage(1),
        AwesomeLevel::new(0),
        LevelCapUpgrades::NONE,
    )
    .map_err(balance)?;
    World::new(vm, BalanceState::new(snapshot), &[]).map_err(world_error)
}

fn world_error(e: WorldError) -> MissionError {
    match e {
        WorldError::Vm(v) => BehaviorError::Vm(v),
        other => data(other.to_string()),
    }
}

fn balance(e: BalanceError) -> MissionError {
    match e {
        BalanceError::NotYet(n) => BehaviorError::Vm(VmError::NotYet(n)),
        other => data(error_text(&other)),
    }
}

/// An error's text with its causes (the C++ `what()` of a rethrown exception).
pub(crate) fn error_text(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut cause = error.source();
    while let Some(c) = cause {
        text.push_str(": ");
        text.push_str(&c.to_string());
        cause = c.source();
    }
    text
}

/// A bridge error inside a native: the VM's own error, or a script error naming the native.
fn to_vm(e: MissionError, function: &str) -> VmError {
    match e {
        BehaviorError::Vm(v) => v,
        other => VmError::Script {
            function: function.to_string(),
            message: error_text(&other),
        },
    }
}

/// Sets the object or interface property `name` of `o` to `target`, as the property's declared kind holds it (the
/// installed movie class declares some of these as interfaces; seam of lane W6-FIRE, decision 2026-11-16-38).
fn set_reference(space: &mut ObjectSpace, o: ObjRef, name: &str, target: ObjRef) -> Result<(), MissionError> {
    let p = required_prop(space, o, name)?;
    let interface = matches!(space.classes.decl(p).map(|d| &d.kind), Some(PropKind::Interface { .. }));
    let v = if interface { Value::Interface(Some(target)) } else { Value::Object(Some(target)) };
    space.write(o, PropAddr::scalar(p), v)?;
    Ok(())
}

/// The class's short name (the C++ `cls->name`).
fn class_name(space: &ObjectSpace, o: ObjRef) -> Result<String, MissionError> {
    let class = space.heap.get(o)?.class;
    Ok(space
        .classes
        .get_layout(class)
        .map(|l| l.path().name().to_string())
        .unwrap_or_default())
}

/// The property `name` of `o`, or the C++ `required` text.
fn required_prop(space: &ObjectSpace, o: ObjRef, name: &str) -> Result<PropId, MissionError> {
    let class = space.heap.get(o)?.class;
    match space.prop(class, name) {
        Some(p) => Ok(p),
        None => Err(data(format!(
            "mission script bridge: {} has no property {name}",
            class_name(space, o)?
        ))),
    }
}

fn get(space: &ObjectSpace, o: ObjRef, name: &str) -> Result<Value, MissionError> {
    let p = required_prop(space, o, name)?;
    Ok(space.read(o, PropAddr::scalar(p))?.clone())
}

fn set(space: &mut ObjectSpace, o: ObjRef, name: &str, v: Value) -> Result<(), MissionError> {
    let p = required_prop(space, o, name)?;
    space.write(o, PropAddr::scalar(p), v)?;
    Ok(())
}

/// `v` with field `name` set to `x` (a struct value), or `None` when `v` is not a struct with that field.
fn with_field(space: &ObjectSpace, v: &Value, name: &str, x: Value) -> Option<Value> {
    let s = v.as_struct()?;
    let id = space.names.find(name)?;
    let (entry, _) = space.classes.struct_layout(s.ty)?.field(id)?;
    let mut out = v.clone();
    let slot = out.struct_mut()?.fields.get_mut(entry as usize)?;
    *slot = x;
    Some(out)
}

/// The class at `path`; a path no package holds is the C++ text (`class not found: <path>`), which the screen's note
/// prints.
fn find_class(vm: &mut Vm, path: &str) -> Result<ow_object::ids::ClassId, MissionError> {
    vm.find_class(path).map_err(|e| {
        let wanted = format!("class not found: {path}");
        if error_text(&e).ends_with(&wanted) {
            data(wanted)
        } else {
            BehaviorError::Vm(e)
        }
    })
}

/// A new object of `class_path` starting from its class default object's values, as the C++ `Runtime::instantiate`
/// copies `defaults->props` (lane W6-COMBAT-RUN2, decision 2026-11-16-54: the controller's damage modifiers, the
/// pawn's and the replication info's defaults were zero before).
fn new_object(vm: &mut Vm, class_path: &str) -> Result<ObjRef, MissionError> {
    let c = vm.find_class(class_path)?;
    Ok(ow_vm::interp::instantiate_from_defaults(vm, c)?)
}

/// A new object made from export `index` of `package` on every call, its references the definition store's objects
/// (the C++ `Runtime::instantiateExport`; `DefStore::instance`). The store's log lines go to the VM's log.
pub(crate) fn instantiate_export(
    vm: &mut Vm,
    package: PackageId,
    index: i32,
    prefix: PropertyPrefix,
) -> Result<ObjRef, MissionError> {
    let made = vm.defs.instance(&mut vm.space, package, index, prefix);
    vm.log.extend(vm.defs.take_log());
    Ok(made?)
}

/// The export an object value names (the C++ `resourcePackage` / `resourceIndex`): a definition store object, loaded
/// or not, or a stand-in; `None` for None, a non-object value, or an object read from no package.
fn export_of(vm: &Vm, v: &Value) -> Result<Option<(PackageId, i32)>, MissionError> {
    let Value::Object(Some(o)) = v else {
        return Ok(None);
    };
    Ok(match vm.space.heap.get(*o)?.origin {
        Origin::Export { package, index } | Origin::Resource { package, index } => Some((package, index)),
        _ => None,
    })
}

/// Whether `o`'s class is `ancestor` or derives from it.
fn is_child_of(space: &ObjectSpace, o: ObjRef, ancestor: ow_object::ids::ClassId) -> Result<bool, MissionError> {
    let mut cursor = Some(space.heap.get(o)?.class);
    let mut steps = 0;
    while let Some(c) = cursor {
        if c == ancestor {
            return Ok(true);
        }
        steps += 1;
        if steps > 256 {
            break;
        }
        cursor = space.classes.get_layout(c).and_then(|l| l.super_class());
    }
    Ok(false)
}

/// `self.<name>(args)` found as the object's class and state see it (the C++ `callByName`).
fn call_by_name(vm: &mut Vm, o: ObjRef, name: &str, args: Vec<Value>) -> Result<CallResult, MissionError> {
    let (class, state) = {
        let x = vm.space.heap.get(o)?;
        (x.class, x.state)
    };
    let f = vm
        .find_method(class, name, state)?
        .ok_or_else(|| data(format!("no function {name} on {}", class_name(&vm.space, o).unwrap_or_default())))?;
    Ok(vm.call(f, Some(o), args)?)
}

/// The registry key of the native at `path`, or the C++ refusal for a script function.
fn native_key(vm: &mut Vm, path: &str) -> Result<String, MissionError> {
    let f = vm.find_function(path)?;
    vm.function(f)
        .and_then(|d| d.native_key.clone())
        .ok_or_else(|| data(format!("mission script bridge: expected a native: {path}")))
}

fn bind(vm: &mut Vm, path: &str, f: NativeFn) -> Result<(), MissionError> {
    let key = native_key(vm, path)?;
    vm.register_native(&key, f)?;
    Ok(())
}

/// A native the real packages have and a synthetic package set may lack: no binding when it is not declared.
fn bind_optional(vm: &mut Vm, path: &str, f: NativeFn) -> Result<(), MissionError> {
    match native_key(vm, path) {
        Ok(key) => {
            vm.register_native(&key, f)?;
            Ok(())
        }
        Err(_) => Ok(()),
    }
}

fn key_of(vm: &Vm, f: FuncId) -> String {
    vm.function(f).map(|d| d.display_key().to_string()).unwrap_or_default()
}

/// A native of the bound class called on another object: the unbound behaviour. In `ParityMode::Reference`, or when
/// the VM's stub policy allows the key, the C++ stub (a log line and the zero result); otherwise the policy's refusal
/// (`VmError::UnboundNative`), as for any unbound native of a gameplay command.
fn outside_binding(c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
    let key = key_of(c.vm, c.function);
    if c.vm.parity != ParityMode::Reference && !c.vm.stub_policy.allows_stub(&key) {
        return Err(BehaviorError::Vm(VmError::UnboundNative(key)));
    }
    c.vm
        .log
        .push(format!("{STUB_PREFIX}{key} (outside the mission bridge binding)"));
    Ok(zero_result(c))
}

fn zero_result(c: &NativeCall<'_>) -> Value {
    match c.vm.function(c.function).and_then(|d| d.result.as_ref()) {
        Some(r) => c.vm.space.classes.zero_value(&r.kind),
        None => Value::None,
    }
}

fn arg_object(c: &NativeCall<'_>, i: usize) -> Result<Option<ObjRef>, VmError> {
    Ok(match c.arg(i)? {
        Value::Object(o) | Value::Interface(o) => *o,
        _ => None,
    })
}

fn arg_int(c: &NativeCall<'_>, i: usize) -> Result<i32, VmError> {
    Ok(integer32(c.arg(i)?))
}

fn arg_truth(c: &NativeCall<'_>, i: usize) -> Result<bool, VmError> {
    let v = c.arg(i)?.clone();
    Ok(v.truth(&c.vm.space.names))
}

/// The stock path of an object argument (`""`: None or an object read from no package).
fn path_of(vm: &Vm, v: &Value) -> Result<String, MissionError> {
    match v {
        Value::Object(Some(o)) => stock_path(vm, *o),
        _ => Ok(String::new()),
    }
}

fn number(v: &Value) -> f64 {
    match v {
        Value::Float(f) => f64::from(*f),
        other => integer(other) as f64,
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------------------------------------------

type Body = fn(&MissionScript, &mut NativeCall<'_>) -> Result<Value, MissionError>;

impl MissionScript {
    /// Builds the graph on `world` for `mission`, binds the natives, delivers the mission data validation reply
    /// (the installed `ClientValidateMissionData` runs on the controller: a shortcut for the trigger, which was not
    /// read; the effect is the game's) and installs the tracker's hooks.
    ///
    /// **One bridge per fresh world.** `new` refuses a world that already has actors (it places its own `WorldInfo`
    /// as the level's first actor). It is not transactional: after an error the objects it made stay in the world's
    /// VM, its `WorldInfo` may already be placed and some natives may be bound, so **the world cannot be used after a
    /// failed `new`** (drop it and build another). The CLI builds the bridge once and fails as a whole; a host or the
    /// slice (W6-FIRE) must do the same.
    pub fn new(world: &mut World, mission: &MissionSystem) -> Result<MissionScript, MissionError> {
        let actors = world.actors();
        if !actors
            .try_borrow()
            .map_err(|_| BehaviorError::Vm(VmError::ServiceBorrowed("Actors")))?
            .list()
            .is_empty()
        {
            return Err(data("mission script bridge: the world already has actors (one bridge per fresh world)".to_string()));
        }
        let world_query = world.query();
        let vm = &mut world.vm;
        let package = match vm.space.heap.get(mission.definition())?.origin {
            Origin::Export { package, .. } | Origin::Resource { package, .. } => {
                let name = vm.defs.packages().get(package).name.clone();
                PackageName::parse(&name).map_err(|e| data(format!("{name}: {e}")))?
            }
            _ => return Err(data("mission script bridge: the mission is not read from a package".to_string())),
        };
        let tracker = new_object(vm, "WillowGame.MissionTracker")?;
        let replication = new_object(vm, "WillowGame.WillowGameReplicationInfo")?;
        set(&mut vm.space, replication, "MissionTracker", Value::Object(Some(tracker)))?;
        let world_info = new_object(vm, "Engine.WorldInfo")?;
        set(&mut vm.space, world_info, "GRI", Value::Object(Some(replication)))?;
        // The level's WorldInfo, which the world's GetWorldInfo natives answer with.
        actors
            .try_borrow_mut()
            .map_err(|_| BehaviorError::Vm(VmError::ServiceBorrowed("Actors")))?
            .place_world_info(world_info)?;
        let controller = new_object(vm, "WillowGame.WillowPlayerController")?;
        set(&mut vm.space, controller, "WorldInfo", Value::Object(Some(world_info)))?;
        // The one local player (`Actor.LocalPlayerControllers`, decision 2026-11-16-81).
        actors
            .try_borrow_mut()
            .map_err(|_| BehaviorError::Vm(VmError::ServiceBorrowed("Actors")))?
            .add_local_player(controller)?;
        // The one globals instance every GetWillowGlobals / GetGearboxGlobals call returns (NATIVE_CONTROLLER_HELPERS.md).
        let globals = new_object(vm, "WillowGame.WillowGlobals")?;
        // ... which is the world's globals object too (what the evaluator's no-context resolver answers with).
        world_query
            .try_borrow_mut()
            .map_err(|_| BehaviorError::Vm(VmError::ServiceBorrowed("WorldQuery")))?
            .set_globals(globals);
        let pri = new_object(vm, "WillowGame.WillowPlayerReplicationInfo")?;
        set(&mut vm.space, controller, "PlayerReplicationInfo", Value::Object(Some(pri)))?;
        // The game's replication info lists the one player (kill experience walks the list); a synthetic package set
        // without these properties keeps no list.
        let pri_class = vm.space.heap.get(pri)?.class;
        let rep_class = vm.space.heap.get(replication)?.class;
        if vm.space.prop(pri_class, "Owner").is_some() && vm.space.prop(rep_class, "PRIArray").is_some() {
            set(&mut vm.space, pri, "Owner", Value::Object(Some(controller)))?;
            let mut list = match get(&vm.space, replication, "PRIArray")? {
                Value::Array(a) => a.as_ref().clone(),
                _ => Vec::new(),
            };
            list.push(Value::Object(Some(pri)));
            set(&mut vm.space, replication, "PRIArray", Value::array(list))?;
        }
        // A local controller with authority (single player); the role is looked up by its enum name, not assumed.
        let roles = enum_names(vm, "Engine", "Actor.ENetRole")?;
        let authority = roles
            .iter()
            .position(|r| r == "ROLE_Authority")
            .ok_or_else(|| data("mission script bridge: ENetRole has no ROLE_Authority".to_string()))?;
        let authority = u8::try_from(authority)
            .map_err(|_| data("mission script bridge: ROLE_Authority is past the byte range".to_string()))?;
        set(&mut vm.space, controller, "Role", Value::Byte(authority))?;
        // The world runs as the authority too (behavior scripts test the world's role).
        set(&mut vm.space, world_info, "Role", Value::Byte(authority))?;
        // One playthrough with an empty mission list (the script indexes MissionPlaythroughs[GetCurrentPlaythrough()]).
        let p = required_prop(&vm.space, controller, "MissionPlaythroughs")?;
        let element = match vm.space.classes.decl(p).map(|d| &d.kind) {
            Some(PropKind::Array(inner)) => match inner.as_ref() {
                PropKind::Struct(s) => *s,
                _ => return Err(data("mission script bridge: MissionPlaythroughs is not an array of structs".to_string())),
            },
            _ => return Err(data("mission script bridge: MissionPlaythroughs is not an array".to_string())),
        };
        let playthrough = vm
            .space
            .classes
            .struct_defaults(element)
            .map_err(|e| BehaviorError::Vm(VmError::from(ow_object::heap::HeapError::Layout(e))))?;
        set(&mut vm.space, controller, "MissionPlaythroughs", Value::array(vec![playthrough]))?;
        let script = MissionScript {
            inner: Rc::new(Bridge {
                mission: mission.clone(),
                package,
                o: Objects {
                    tracker,
                    replication,
                    world_info,
                    controller,
                    globals,
                    pri,
                    authority,
                },
                state: RefCell::new(BState::default()),
                marcus: RefCell::new(None),
                has_marcus: Cell::new(false),
                marcus_pid: Cell::new(None),
                marcus_lost: Cell::new(false),
            }),
        };
        script.bind_natives(vm)?;
        let c = controller;
        script.run(vm, |vm| call_by_name(vm, c, "ClientValidateMissionData", Vec::new()).map(|_| ()));
        script.install_hooks();
        Ok(script)
    }

    fn weak(&self) -> Weak<Bridge> {
        Rc::downgrade(&self.inner)
    }

    /// A native over `body` that finds this bridge at call time.
    fn native(&self, path: &'static str, body: Body) -> NativeFn {
        let w = self.weak();
        Rc::new(move |c: &mut NativeCall<'_>| {
            let inner = w
                .upgrade()
                .ok_or_else(|| VmError::Script {
                    function: path.to_string(),
                    message: "the mission script bridge is gone".to_string(),
                })?;
            body(&MissionScript { inner }, c).map_err(|e| to_vm(e, path))
        })
    }

    fn bind_natives(&self, vm: &mut Vm) -> Result<(), MissionError> {
        const BOUND: [(&str, Body); 33] = [
            ("WillowGame.MissionTracker.ActivateMission", MissionScript::n_activate),
            ("WillowGame.MissionTracker.CompleteMission", MissionScript::n_complete),
            ("WillowGame.MissionTracker.PlayTurnIn", MissionScript::n_play_turn_in),
            ("WillowGame.MissionTracker.GetMissionStatus", MissionScript::n_status),
            ("WillowGame.MissionTracker.CanStartMission", MissionScript::n_can_start),
            ("WillowGame.MissionTracker.CanEndMission", MissionScript::n_can_end),
            ("WillowGame.MissionTracker.GetCompletedBranch", MissionScript::n_branch),
            ("WillowGame.WillowPlayerController.NativeGetMissionIndex", MissionScript::n_mission_index),
            ("WillowGame.WillowPlayerController.GetCurrentPlaythrough", MissionScript::n_playthrough),
            ("Engine.PlayerController.IsPrimaryPlayer", MissionScript::n_primary),
            ("WillowGame.WillowPlayerController.GetHUDMovie", MissionScript::n_hud_movie),
            ("WillowGame.WillowPlayerController.UpdateLcdMissionStatus", MissionScript::n_controller_noop),
            ("WillowGame.WillowPlayerController.PlayUIAkEvent", MissionScript::n_controller_noop),
            ("Engine.WorldInfo.IsMenuLevel", MissionScript::n_menu_level),
            ("WillowGame.WillowGlobals.GetWillowGlobals", MissionScript::n_globals),
            ("GearboxFramework.GearboxGlobals.GetGearboxGlobals", MissionScript::n_globals),
            ("GearboxFramework.GearboxGlobals.GetBehaviorKernel", MissionScript::n_kernel),
            ("WillowGame.WillowGlobals.GetGlobalsDefinition", MissionScript::n_globals_definition),
            ("WillowGame.MissionTracker.IsDataValid", MissionScript::n_data_valid),
            ("WillowGame.MissionTracker.ValidateData", MissionScript::n_validate),
            ("WillowGame.WillowPlayerController.ExpEarn", MissionScript::n_exp_earn),
            ("WillowGame.WillowPlayerController.GetMaxExpLevel", MissionScript::n_max_level),
            ("WillowGame.WillowPlayerController.GetExpPointsRequiredForLevel", MissionScript::n_required),
            ("WillowGame.MissionDefinition.GetExperienceReward", MissionScript::n_xp_reward),
            ("WillowGame.MissionDefinition.GetGameStage", MissionScript::n_game_stage),
            ("WillowGame.MissionDefinition.GetCurrencyRewardType", MissionScript::n_currency_type),
            ("WillowGame.MissionDefinition.GetCurrencyReward", MissionScript::n_currency),
            ("WillowGame.MissionDefinition.ShouldGrantAlternateReward", MissionScript::n_alternate),
            ("WillowGame.MissionDefinition.GetItemRewardsForPlayer", MissionScript::n_item_rewards),
            ("WillowGame.MissionTracker.IsMissionObjectiveActive", MissionScript::n_objective_active),
            ("WillowGame.MissionTracker.IsMissionObjectiveComplete", MissionScript::n_objective_complete),
            ("WillowGame.MissionTracker.IsObjectiveSetActive", MissionScript::n_set_active),
            ("WillowGame.MissionTracker.UpdateObjective", MissionScript::n_update_objective),
        ];
        for (path, body) in BOUND {
            bind(vm, path, self.native(path, body))?;
        }
        const OPTIONAL: [(&str, Body); 3] = [
            ("WillowGame.WillowPawn.GetAWillowPawn", MissionScript::n_a_willow_pawn),
            ("WillowGame.WillowPawn.SetGameStage", MissionScript::n_set_game_stage),
            ("Engine.BehaviorBase.RunBehaviors", MissionScript::n_run_behaviors),
        ];
        for (path, body) in OPTIONAL {
            bind_optional(vm, path, self.native(path, body))?;
        }
        // Off only: the C++ stubs it, and Reference keeps that stub (R3; decision 2026-11-19-05).
        if vm.parity != ParityMode::Reference {
            let path = "WillowGame.MissionTracker.GetObjectivesProgress";
            bind_optional(vm, path, self.native(path, MissionScript::n_objectives_progress))?;
        }
        bind(
            vm,
            "WillowGame.MissionTracker.RegisterMissionObserver",
            self.native("WillowGame.MissionTracker.RegisterMissionObserver", MissionScript::n_register_observer),
        )?;
        bind(
            vm,
            "Engine.Actor.IsPlayerOwned",
            self.native("Engine.Actor.IsPlayerOwned", MissionScript::n_player_owned),
        )?;
        Ok(())
    }

    fn install_hooks(&self) {
        let w = self.weak();
        self.inner.mission.set_status_hook(Some(Rc::new(move |vm: &mut Vm, status: u8| {
            if let Some(inner) = w.upgrade() {
                MissionScript { inner }.update_mission_status(vm, status);
            }
        })));
        let w = self.weak();
        self.inner
            .mission
            .set_objective_hook(Some(Rc::new(move |vm: &mut Vm, path: &str, bit: i32| {
                if let Some(inner) = w.upgrade() {
                    MissionScript { inner }.objective_updated(vm, path, bit);
                }
            })));
        let w = self.weak();
        self.inner.mission.set_kickoff_hook(Some(Rc::new(move |vm: &mut Vm| {
            if let Some(inner) = w.upgrade() {
                let s = MissionScript { inner };
                let c = s.inner.o.controller;
                s.run(vm, |vm| call_by_name(vm, c, "IsMissionMoviePlaying", Vec::new()).map(|_| ()));
            }
        })));
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Running script
// ---------------------------------------------------------------------------------------------------------------

impl MissionScript {
    /// The outermost script run collects what the VM logged while it ran; a script that fails is recorded, never
    /// propagated (module documentation).
    fn run(&self, vm: &mut Vm, script: impl FnOnce(&mut Vm) -> Result<(), MissionError>) {
        let before = vm.log.len();
        self.inner.state.borrow_mut().depth += 1;
        let result = script(vm);
        let mut s = self.inner.state.borrow_mut();
        s.depth -= 1;
        if let Err(e) = result {
            if let Some(n) = e.not_yet() {
                s.not_yet.push(n.clone());
            }
            s.errors.push(format!("mission script: {}", error_text(&e)));
        }
        if s.depth != 0 {
            return;
        }
        for line in vm.log.iter().skip(before) {
            if let Some(stub) = line.strip_prefix(STUB_PREFIX) {
                *s.stubs.entry(stub.to_string()).or_insert(0) += 1;
            } else if !s.notes.contains(line) {
                s.notes.push(line.clone());
            }
        }
    }

    /// A path the C++ leaves out with a "not implemented" stub line (architecture R3): in `ParityMode::Reference` that
    /// line, and the caller goes on as the C++ does; otherwise `unread`, the honest refusal.
    fn not_implemented(vm: &mut Vm, what: &str, unread: NotYet) -> Result<(), MissionError> {
        if vm.parity == ParityMode::Reference {
            vm.log.push(format!("{STUB_PREFIX}{what} (not implemented)"));
            return Ok(());
        }
        Err(unread.into())
    }

    fn completed(&self) -> DetSet<String> {
        self.inner.state.borrow().completed.clone()
    }

    fn director(&self) -> Option<ObjRef> {
        self.inner.state.borrow().director
    }

    /// The mission object, or a reference to the same export (a director table names missions by reference).
    fn is_mission(&self, vm: &Vm, v: &Value) -> Result<bool, MissionError> {
        let Value::Object(Some(o)) = v else {
            return Ok(false);
        };
        if *o == self.inner.mission.definition() {
            return Ok(true);
        }
        Ok(stock_path(vm, *o)? == self.inner.mission.path())
    }

    /// The status routine's script hook: the Active branch locks the mission game stage (the region's) before the
    /// hook runs; then `UpdateMissionStatus` on the one local controller and the tracker's status-changed delegates.
    fn update_mission_status(&self, vm: &mut Vm, status: u8) {
        if status == 1 {
            let mut s = self.inner.state.borrow_mut();
            s.locked_stage = s.region_stage;
        }
        let (c, t, def) = (self.inner.o.controller, self.inner.o.tracker, self.inner.mission.definition());
        self.run(vm, |vm| {
            call_by_name(vm, c, "UpdateMissionStatus", vec![Value::Object(Some(def)), Value::Byte(status)])?;
            call_by_name(vm, t, "TriggerMissionStatusChangedDelegates", Vec::new())?;
            Ok(())
        });
    }

    /// After an applied objective update (NATIVE_OBJECTIVE_TRIGGERS.md): the controller hears
    /// `UpdateMissionObjective(objective, bit)` and the tracker its objectives-changed delegates.
    fn objective_updated(&self, vm: &mut Vm, path: &str, bit: i32) {
        self.inner.state.borrow_mut().objective_updates += 1;
        let (c, t, def) = (self.inner.o.controller, self.inner.o.tracker, self.inner.mission.definition());
        let package = self.inner.package.clone();
        self.run(vm, |vm| {
            // The objective as the script sees it: the same object the mission data's own references load to.
            let objective = tracker::find_object(vm, &package, path)?;
            call_by_name(vm, c, "UpdateMissionObjective", vec![Value::Object(objective), Value::Int(bit)])?;
            call_by_name(vm, t, "TriggerMissionObjectivesChangedDelegates", vec![Value::Object(Some(def))])?;
            Ok(())
        });
    }

    /// `AcceptMission(Mission, MissionDirector)` on the controller; `completed` is what `MissionDependenciesMet`
    /// sees. True when the mission's status changed through it.
    pub fn accept(&self, vm: &mut Vm, completed: &DetSet<String>) -> Result<bool, MissionError> {
        self.inner.state.borrow_mut().completed = completed.clone();
        let before = self.inner.mission.status();
        let (c, def, director) = (self.inner.o.controller, self.inner.mission.definition(), self.director());
        self.run(vm, |vm| {
            call_by_name(vm, c, "AcceptMission", vec![Value::Object(Some(def)), Value::Object(director)]).map(|_| ())
        });
        Ok(self.inner.mission.status() != before)
    }

    /// `ServerCompleteMission(Mission, MissionDirector)`. True when the mission became Complete through it.
    pub fn turn_in(&self, vm: &mut Vm) -> Result<bool, MissionError> {
        self.inner.state.borrow_mut().turning_in = true;
        let (c, def, director) = (self.inner.o.controller, self.inner.mission.definition(), self.director());
        self.run(vm, |vm| {
            call_by_name(vm, c, "ServerCompleteMission", vec![Value::Object(Some(def)), Value::Object(director)])
                .map(|_| ())
        });
        self.inner.state.borrow_mut().turning_in = false;
        Ok(self.inner.mission.status() == tracker::Status::Complete)
    }

    /// After `MissionSystem::load_state`: the controller's own record follows the restored status, as the level-load
    /// replay would leave it.
    pub fn sync_restored(&self, vm: &mut Vm) -> Result<(), MissionError> {
        let Some((index, mut playthrough)) = self.playthrough_record(vm)? else {
            return Ok(());
        };
        let mut list = Vec::new();
        if self.inner.mission.status() != tracker::Status::NotStarted {
            let ty = vm
                .space
                .classes
                .struct_by_path("WillowGame.IMission.MissionStatusPlayerData")
                .ok_or_else(|| data("struct not found: WillowGame.IMission.MissionStatusPlayerData".to_string()))?;
            let record = vm
                .space
                .classes
                .struct_defaults(ty)
                .map_err(|e| BehaviorError::Vm(VmError::from(ow_object::heap::HeapError::Layout(e))))?;
            let record = with_field(&vm.space, &record, "MissionDef", Value::Object(Some(self.inner.mission.definition())))
                .and_then(|r| with_field(&vm.space, &r, "Status", Value::Byte(self.inner.mission.status_number())))
                .ok_or_else(|| data("mission script bridge: MissionStatusPlayerData lacks MissionDef or Status".to_string()))?;
            list.push(record);
        }
        playthrough = with_field(&vm.space, &playthrough, "MissionList", Value::array(list))
            .ok_or_else(|| data("mission script bridge: MissionPlaythroughData has no MissionList".to_string()))?;
        let all = get(&vm.space, self.inner.o.controller, "MissionPlaythroughs")?;
        let mut items = all.elements().map(<[Value]>::to_vec).unwrap_or_default();
        if let Some(slot) = items.get_mut(index) {
            *slot = playthrough;
        }
        set(&mut vm.space, self.inner.o.controller, "MissionPlaythroughs", Value::array(items))
    }

    fn current_playthrough(&self, vm: &Vm) -> Result<i32, MissionError> {
        Ok(prop(&vm.space, self.inner.o.replication, "CurrentPlaythrough")?.map_or(0, |v| integer32(&v)))
    }

    /// The current playthrough's `MissionPlaythroughData` and its index, `None` when the index is out of range.
    fn playthrough_record(&self, vm: &Vm) -> Result<Option<(usize, Value)>, MissionError> {
        let Some(all) = prop(&vm.space, self.inner.o.controller, "MissionPlaythroughs")? else {
            return Ok(None);
        };
        let index = self.current_playthrough(vm)?;
        let Ok(i) = usize::try_from(index) else {
            return Ok(None);
        };
        Ok(all.elements().and_then(|e| e.get(i)).map(|v| (i, v.clone())))
    }

    /// The current playthrough's `MissionList` records.
    fn mission_list(&self, vm: &Vm) -> Result<Vec<Value>, MissionError> {
        let Some((_, record)) = self.playthrough_record(vm)? else {
            return Ok(Vec::new());
        };
        Ok(field(&vm.space, &record, "MissionList")
            .and_then(|l| l.elements())
            .map(<[Value]>::to_vec)
            .unwrap_or_default())
    }

    /// The controller's record of the mission (`MissionPlaythroughs[current].MissionList[..].Status`), -1 when it has
    /// none.
    pub fn controller_status(&self, vm: &Vm) -> Result<i32, MissionError> {
        for record in self.mission_list(vm)? {
            let def = field(&vm.space, &record, "MissionDef");
            let status = field(&vm.space, &record, "Status");
            if let (Some(Value::Object(Some(d))), Some(s)) = (def, status)
                && *d == self.inner.mission.definition()
            {
                return Ok(integer32(s));
            }
        }
        Ok(-1)
    }

    pub fn controller_needs_rewards(&self, vm: &Vm) -> Result<bool, MissionError> {
        for record in self.mission_list(vm)? {
            let def = field(&vm.space, &record, "MissionDef");
            let needs = field(&vm.space, &record, "bNeedsRewards");
            if let (Some(Value::Object(Some(d))), Some(n)) = (def, needs)
                && *d == self.inner.mission.definition()
            {
                return Ok(n.truth(&vm.space.names));
            }
        }
        Ok(false)
    }

    // ---- inputs the host owns ----------------------------------------------------------------------------------

    /// The region's game stage (what `WillowRegionDefinition.GetRegionGameStage` answers now); the Active status locks
    /// it into the mission.
    pub fn set_region_game_stage(&self, stage: i32) {
        self.inner.state.borrow_mut().region_stage = stage;
    }

    /// The player's experience level and pool value, read before the pool is updated.
    pub fn set_player_experience(&self, vm: &mut Vm, level: i32, experience: i64) -> Result<(), MissionError> {
        let next = level
            .checked_add(1)
            .ok_or_else(|| data(format!("mission script bridge: level {level} has no next level")))?;
        let required = self.required(vm, next)?;
        let pri = self.inner.o.pri;
        set(&mut vm.space, pri, "ExpLevel", Value::Int(level))?;
        set(&mut vm.space, pri, "ExpPointsNextLevelAt", Value::Int(script_int(required)?))?;
        // The C++ float(int64_t): rounds to the nearest float, as `as` does for an integer.
        self.inner.state.borrow_mut().pool = experience as f32;
        Ok(())
    }

    /// The player's current health (the swap 7b pool): not built here (module documentation).
    pub fn set_player_health(&self, current: f32) -> Result<(), MissionError> {
        let _ = current;
        Err(NotYet::not_ported(
            "the level-up's health pool of the mission bridge (the C++ hand-built pool; CreatePool is the record's way)",
            RECORD,
        )
        .into())
    }

    /// `ApplyExpPointsToExpLevel(false)`, run from the pool's per-frame update: while the pool has reached the next
    /// level's requirement and the level is below the cap, the script `ExpLevelUp(bCheated = false)` runs.
    pub fn update_experience_pool(&self, vm: &mut Vm) -> Result<(), MissionError> {
        self.run_level_ups(vm, false)
    }

    /// One `ExpLevelUp(bCheated = true)`: the not-natural level-up.
    pub fn cheat_level_up(&self, vm: &mut Vm) -> Result<(), MissionError> {
        self.run_level_ups(vm, true)
    }

    pub fn player_level(&self, vm: &Vm) -> Result<i32, MissionError> {
        Ok(integer32(&get(&vm.space, self.inner.o.pri, "ExpLevel")?))
    }

    pub fn experience_pool(&self) -> f32 {
        self.inner.state.borrow().pool
    }

    pub fn exp_earned(&self) -> Vec<ExpEarn> {
        self.inner.state.borrow().exp_earned.clone()
    }

    pub fn take_gains(&self) -> Vec<Gain> {
        std::mem::take(&mut self.inner.state.borrow_mut().gains)
    }

    pub fn errors(&self) -> Vec<String> {
        self.inner.state.borrow().errors.clone()
    }

    /// The `NotYet` of each script run that stopped on one, in order.
    pub fn not_yet(&self) -> Vec<NotYet> {
        self.inner.state.borrow().not_yet.clone()
    }

    /// Natives without an implementation that the script reached, `Class.Name(types) xN`, in key order.
    pub fn stubs(&self) -> Vec<String> {
        self.inner
            .state
            .borrow()
            .stubs
            .iter()
            .map(|(name, n)| format!("{name} x{n}"))
            .collect()
    }

    pub fn notes(&self) -> Vec<String> {
        self.inner.state.borrow().notes.clone()
    }

    pub fn objective_updates(&self) -> i32 {
        self.inner.state.borrow().objective_updates
    }

    /// The VM controller (the player, the context source of the evaluator and the owner of the experience pool).
    pub fn player_controller(&self) -> ObjRef {
        self.inner.o.controller
    }

    /// The player's VM pawn (made on first use).
    pub fn player_pawn(&self, vm: &mut Vm) -> Result<ObjRef, MissionError> {
        self.pawn_for(vm, Toucher::Player)
    }

    /// The mission game stage: the one locked when it became Active, else the region's (0 without one).
    pub fn current_game_stage(&self) -> i32 {
        let s = self.inner.state.borrow();
        if s.locked_stage != 0 { s.locked_stage } else { s.region_stage }
    }

    fn pawn_for(&self, vm: &mut Vm, who: Toucher) -> Result<ObjRef, MissionError> {
        let existing = {
            let s = self.inner.state.borrow();
            match who {
                Toucher::Player => s.player_pawn,
                Toucher::Marcus => s.marcus_pawn,
            }
        };
        if let Some(p) = existing {
            return Ok(p);
        }
        let pawn = match who {
            Toucher::Player => {
                let p = new_object(vm, "WillowGame.WillowPlayerPawn")?;
                // The player's pawn is controlled by the player's controller; Marcus has no player controller.
                set(&mut vm.space, p, "Controller", Value::Object(Some(self.inner.o.controller)))?;
                p
            }
            Toucher::Marcus => new_object(vm, "WillowGame.WillowAIPawn")?,
        };
        let mut s = self.inner.state.borrow_mut();
        match who {
            Toucher::Player => s.player_pawn = Some(pawn),
            Toucher::Marcus => s.marcus_pawn = Some(pawn),
        }
        Ok(pawn)
    }

    /// An export of the mission's package by stock path, loaded once and cached in `slot`.
    fn mission_object(&self, vm: &mut Vm, path: &str) -> Result<Option<ObjRef>, MissionError> {
        tracker::find_object(vm, &self.inner.package, path)
    }

    fn globals_definition(&self, vm: &mut Vm) -> Result<Option<ObjRef>, MissionError> {
        if let Some(cached) = self.inner.state.borrow().globals_definition {
            return Ok(cached);
        }
        let found = self.mission_object(vm, GLOBALS_PATH)?;
        self.inner.state.borrow_mut().globals_definition = Some(found);
        Ok(found)
    }

    fn player_class(&self, vm: &mut Vm) -> Result<Option<ObjRef>, MissionError> {
        if let Some(cached) = self.inner.state.borrow().player_class {
            return Ok(cached);
        }
        let found = self.mission_object(vm, PLAYER_CLASS_PATH)?;
        self.inner.state.borrow_mut().player_class = Some(found);
        Ok(found)
    }

    /// `GlobalsDefinition.PlayerInteractionDistance` (NATIVE_USE_INTERACTION.md, UNVERIFIED): 0 when the data has no
    /// globals definition or the property is absent.
    pub fn player_interaction_distance(&self, vm: &mut Vm) -> Result<f32, MissionError> {
        let Some(g) = self.globals_definition(vm)? else {
            return Ok(0.0);
        };
        Ok(match prop(&vm.space, g, "PlayerInteractionDistance")? {
            Some(Value::Float(f)) => f,
            // The C++ static_cast<float>(number()): an integer value converted to the nearest float.
            Some(other) => integer(&other) as f32,
            None => 0.0,
        })
    }

    // ---- experience ----------------------------------------------------------------------------------------------

    fn curve(&self, vm: &mut Vm) -> Result<ObjRef, MissionError> {
        if let Some(c) = self.inner.state.borrow().curve {
            return Ok(c);
        }
        let c = self.mission_object(vm, CURVE_PATH)?.ok_or_else(|| {
            data("mission script bridge: the experience curve definition is not in the mission package".to_string())
        })?;
        self.inner.state.borrow_mut().curve = Some(c);
        Ok(c)
    }

    /// `R(level)` of the installed curve (`ow_balance::curve`, verified live for levels 1..59).
    fn required(&self, vm: &mut Vm, level: i32) -> Result<i64, MissionError> {
        let curve = self.curve(vm)?;
        let sys = vm.attrs.clone();
        experience_required(&sys, &mut EvalCx::new(vm), curve, ExpLevel::new(level)).map_err(balance)
    }

    /// `ExpEarn` (bridge note): `Exp x the type's scale x ExpAllPointsScale` added to the pool, never below 0, never
    /// above the experience the maximum level needs, and only when it raises the pool. The scales are attributes of
    /// the pool whose base values were not decoded: taken as 1 (UNVERIFIED). It does not level up (the pool update
    /// does). The call is also recorded.
    fn exp_earn(&self, vm: &mut Vm, amount: i32, source: i32, exp_type: i32) -> Result<(), MissionError> {
        self.inner.state.borrow_mut().exp_earned.push(ExpEarn {
            amount,
            source,
            exp_type,
        });
        // int64 -> float, the nearest float, as the C++ float(int64_t).
        let cap = self.required(vm, MAX_LEVEL)? as f32;
        let mut s = self.inner.state.borrow_mut();
        if s.pool >= cap {
            return Ok(());
        }
        let scale = 1.0f32;
        // The record's bounds (never below 0, never above the cap), the C++ std::clamp.
        let next = (s.pool + amount as f32 * scale).clamp(0.0, cap);
        if next > s.pool {
            let gained = ow_core::num::trunc_i32(next - s.pool);
            s.gains.push(Gain::Experience(gained));
            s.pool = next;
        }
        Ok(())
    }

    /// `MissionDefinition.GetExperienceReward` (NATIVE_PROGRESSION.md section 2; 395 at stage 8 confirmed once in
    /// game): `ow_balance::xp::mission_xp` at the mission game stage, for the current playthrough and the player's
    /// level, the percentage of `Reward` (or `AlternativeReward`) evaluated with the controller as the source.
    fn experience_reward(&self, vm: &mut Vm, alternate: bool) -> Result<i32, MissionError> {
        if self.inner.mission.has_optional_objective() {
            Self::not_implemented(
                vm,
                "MissionDefinition.GetExperienceReward: optional objective experience",
                NotYet::not_ported(
                    "GetExperienceReward of a mission with optional objectives (the completed optional objectives' \
                     OptionalExperienceRewardPercentage terms)",
                    RECORD,
                ),
            )?;
        }
        let reward = self.reward_data(vm, alternate)?;
        let pct = field(&vm.space, &reward, "ExperienceRewardPercentage")
            .ok_or_else(|| data("mission script bridge: the reward has no ExperienceRewardPercentage".to_string()))?;
        let pct = init_data(&vm.space, pct).map_err(|e| BehaviorError::Vm(VmError::from(e)))?;
        let playthrough = Playthrough::from_index(self.current_playthrough(vm)?).map_err(balance)?;
        let level = self.player_level(vm)?;
        let curve = self.curve(vm)?;
        let x = MissionXp::new(
            GameStage(self.current_game_stage()),
            pct,
            Some(self.inner.o.controller),
            playthrough,
            ExpLevel::new(level),
        );
        let sys = vm.attrs.clone();
        mission_xp(&sys, &mut EvalCx::new(vm), curve, &x).map_err(balance)
    }

    /// `Reward` or `AlternativeReward` of the mission (a `MissionRewardData` struct).
    fn reward_data(&self, vm: &Vm, alternate: bool) -> Result<Value, MissionError> {
        let name = if alternate { "AlternativeReward" } else { "Reward" };
        match prop(&vm.space, self.inner.mission.definition(), name)? {
            Some(v) if v.as_struct().is_some() => Ok(v),
            _ => Err(data("mission script bridge: the mission has no reward data".to_string())),
        }
    }

    /// One evaluation with no context source (the C++ empty `AttributeContext`).
    fn evaluate(&self, vm: &mut Vm, v: &Value) -> Result<f32, MissionError> {
        let d = init_data(&vm.space, v).map_err(|e| BehaviorError::Vm(VmError::from(e)))?;
        let sys = vm.attrs.clone();
        sys.evaluate_init(&d, None, None, &mut EvalCx::new(vm))
            .map_err(|e| BehaviorError::Vm(VmError::from(e)))
    }

    /// The level-up loop (`cheated`: one `ExpLevelUp(true)` whatever the pool holds). The controller's `Pawn` and
    /// `PlayerClass` are given for the level-up only (OnExpLevelChange's pawn branch needs both), so the other scripts
    /// see the controller as before.
    fn run_level_ups(&self, vm: &mut Vm, cheated: bool) -> Result<(), MissionError> {
        let (c, pri) = (self.inner.o.controller, self.inner.o.pri);
        let before = self.player_level(vm)?;
        let points_before = integer32(&get(&vm.space, pri, "GeneralSkillPoints")?);
        let saved_pawn = prop(&vm.space, c, "Pawn")?;
        let saved_class = prop(&vm.space, c, "PlayerClass")?;
        let class = if saved_class.is_some() { self.player_class(vm)? } else { None };
        if saved_pawn.is_some() {
            let pawn = self.pawn_for(vm, Toucher::Player)?;
            set(&mut vm.space, c, "Pawn", Value::Object(Some(pawn)))?;
        }
        if let (true, Some(cls)) = (saved_class.is_some(), class) {
            set(&mut vm.space, c, "PlayerClass", Value::Object(Some(cls)))?;
        }
        self.inner.state.borrow_mut().in_level_up = true;
        for _ in 0..LEVEL_UP_GUARD {
            let next = number(&get(&vm.space, pri, "ExpPointsNextLevelAt")?);
            let level = self.player_level(vm)?;
            let pool = f64::from(self.inner.state.borrow().pool);
            // The C++ condition as written: its negation is not `next <= 0.0 || ..` for a NaN requirement.
            #[allow(clippy::nonminimal_bool)]
            let stop = !cheated && !(next > 0.0 && pool >= next && MAX_LEVEL > level);
            if stop {
                break;
            }
            self.run(vm, |vm| call_by_name(vm, c, "ExpLevelUp", vec![Value::Bool(cheated)]).map(|_| ()));
            if self.player_level(vm)? == level {
                self.inner
                    .state
                    .borrow_mut()
                    .notes
                    .push("ExpLevelUp did not raise ExpLevel".to_string());
                break;
            }
            if cheated {
                break;
            }
        }
        self.inner.state.borrow_mut().in_level_up = false;
        if let Some(v) = saved_pawn {
            set(&mut vm.space, c, "Pawn", v)?;
        }
        if let Some(v) = saved_class {
            set(&mut vm.space, c, "PlayerClass", v)?;
        }
        let after = self.player_level(vm)?;
        if after == before {
            return Ok(());
        }
        let points = integer32(&get(&vm.space, pri, "GeneralSkillPoints")?);
        let mut s = self.inner.state.borrow_mut();
        s.gains.push(Gain::Level(after));
        s.gains.push(Gain::SkillPoints(points.wrapping_sub(points_before)));
        Ok(())
    }
}

/// A 64-bit count as a script int; one that does not fit is refused (the C++ `makeInt` would wrap it).
fn script_int(v: i64) -> Result<i32, MissionError> {
    i32::try_from(v).map_err(|_| data(format!("mission script bridge: {v} does not fit a script int")))
}

// ---------------------------------------------------------------------------------------------------------------
// The natives
// ---------------------------------------------------------------------------------------------------------------

impl MissionScript {
    fn on(&self, c: &NativeCall<'_>, o: ObjRef) -> bool {
        c.this == Some(o)
    }

    // ActivateMission / CompleteMission (bridge note): thin front ends of the status routine. The native refuses
    // silently (no record, wrong status, dependencies not met) and ignores the role. Not modelled: CompleteMission's
    // chain to NextMissionInChain, the untracking, the unlock queue and the fast-forward prompt.
    fn n_activate(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        if !self.on(c, self.inner.o.tracker) {
            return outside_binding(c);
        }
        if self.is_mission(c.vm, &c.arg(0)?.clone())? {
            let completed = self.completed();
            self.inner.mission.accept(c.vm, &completed)?;
        }
        Ok(Value::None)
    }

    fn n_complete(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        if !self.on(c, self.inner.o.tracker) {
            return outside_binding(c);
        }
        if self.is_mission(c.vm, &c.arg(0)?.clone())? {
            self.inner.mission.turn_in_mission(c.vm)?;
        }
        Ok(Value::None)
    }

    // PlayTurnIn with a mission fires "Default" with link id 14 and does nothing else.
    fn n_play_turn_in(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        if !self.on(c, self.inner.o.tracker) {
            return outside_binding(c);
        }
        if self.is_mission(c.vm, &c.arg(0)?.clone())? {
            self.inner.mission.play_turn_in(c.vm)?;
        }
        Ok(Value::None)
    }

    fn n_status(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        if !self.on(c, self.inner.o.tracker) {
            return outside_binding(c);
        }
        // No record: NotStarted.
        let mine = self.is_mission(c.vm, &c.arg(0)?.clone())?;
        Ok(Value::Byte(if mine { self.inner.mission.status_number() } else { 0 }))
    }

    // The availability queries (bridge note, "Availability queries"): a mission without a record (any other mission of
    // the director's table) is NotStarted and is not offered here (the tracker holds one mission), UNVERIFIED.
    fn n_can_start(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        if !self.on(c, self.inner.o.tracker) {
            return outside_binding(c);
        }
        let mine = self.is_mission(c.vm, &c.arg(0)?.clone())?;
        let start = mine && self.inner.mission.can_start(&self.completed(), c.vm)?;
        Ok(Value::Bool(start))
    }

    fn n_can_end(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        if !self.on(c, self.inner.o.tracker) {
            return outside_binding(c);
        }
        let mine = self.is_mission(c.vm, &c.arg(0)?.clone())?;
        Ok(Value::Bool(mine && self.inner.mission.can_end()))
    }

    fn n_branch(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        if !self.on(c, self.inner.o.tracker) {
            return outside_binding(c);
        }
        let mine = self.is_mission(c.vm, &c.arg(0)?.clone())?;
        Ok(Value::Byte(if mine { self.inner.mission.completed_branch() } else { 0 }))
    }

    // The controller's own list lookup (NATIVE_CONTROLLER_HELPERS.md): the index of the first record of the current
    // playthrough's list whose MissionDef is that object; -1 when the playthrough index is out of range or none is.
    fn n_mission_index(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        if !self.on(c, self.inner.o.controller) {
            return outside_binding(c);
        }
        let wanted = arg_object(c, 0)?;
        for (i, record) in self.mission_list(c.vm)?.iter().enumerate() {
            if let Some(Value::Object(d)) = field(&c.vm.space, record, "MissionDef")
                && *d == wanted
            {
                return Ok(Value::Int(script_int(i as i64)?));
            }
        }
        Ok(Value::Int(-1))
    }

    // GetCurrentPlaythrough: CurrentPlaythrough of the world's replication info (0 when absent); no side effects.
    fn n_playthrough(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        if !self.on(c, self.inner.o.controller) {
            return outside_binding(c);
        }
        Ok(Value::Int(self.current_playthrough(c.vm)?))
    }

    // IsPrimaryPlayer: true for the sole local controller of a standalone game.
    fn n_primary(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        if !self.on(c, self.inner.o.controller) {
            return outside_binding(c);
        }
        Ok(Value::Bool(true))
    }

    // GetHUDMovie: None without a WillowHUD (the VM graph has none, so the script skips its HUD branches).
    fn n_hud_movie(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        if !self.on(c, self.inner.o.controller) {
            return outside_binding(c);
        }
        Ok(Value::Object(None))
    }

    // Presentation only (hardware LCD text, UI sound): documented no-ops.
    fn n_controller_noop(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        if !self.on(c, self.inner.o.controller) {
            return outside_binding(c);
        }
        Ok(Value::None)
    }

    // IsMenuLevel (no-argument form): the world's bIsMenuLevel bit, false when absent.
    fn n_menu_level(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        let menu = prop(&c.vm.space, self.inner.o.world_info, "bIsMenuLevel")?;
        Ok(Value::Bool(menu.is_some_and(|v| v.truth(&c.vm.space.names))))
    }

    fn n_globals(&self, _c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        Ok(Value::Object(Some(self.inner.o.globals)))
    }

    // GetBehaviorKernel reads the globals' TheBehaviorKernel field (the kernel object itself is not modelled: None).
    fn n_kernel(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        Ok(prop(&c.vm.space, self.inner.o.globals, "TheBehaviorKernel")?.unwrap_or(Value::Object(None)))
    }

    fn n_globals_definition(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        Ok(Value::Object(self.globals_definition(c.vm)?))
    }

    // IsDataValid reports the tracker's bDataValidated flag; ValidateData, the only writer, sets it.
    fn n_data_valid(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        if !self.on(c, self.inner.o.tracker) {
            return outside_binding(c);
        }
        let v = get(&c.vm.space, self.inner.o.tracker, "bDataValidated")?;
        Ok(Value::Bool(v.truth(&c.vm.space.names)))
    }

    fn n_validate(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        if !self.on(c, self.inner.o.tracker) {
            return outside_binding(c);
        }
        set(&mut c.vm.space, self.inner.o.tracker, "bDataValidated", Value::Bool(true))?;
        Ok(Value::None)
    }

    fn n_exp_earn(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        if !self.on(c, self.inner.o.controller) {
            return outside_binding(c);
        }
        let amount = arg_int(c, 0)?;
        let source = arg_int(c, 1)?;
        let exp_type = if c.has(2) { arg_int(c, 2)? } else { -1 };
        self.exp_earn(c.vm, amount, source, exp_type)?;
        Ok(Value::None)
    }

    fn n_max_level(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        if !self.on(c, self.inner.o.controller) {
            return outside_binding(c);
        }
        Ok(Value::Int(MAX_LEVEL))
    }

    fn n_required(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        if !self.on(c, self.inner.o.controller) {
            return outside_binding(c);
        }
        let level = arg_int(c, 0)?;
        Ok(Value::Int(script_int(self.required(c.vm, level)?)?))
    }

    fn this_is_mission(&self, c: &NativeCall<'_>) -> Result<bool, MissionError> {
        self.is_mission(c.vm, &Value::Object(c.this))
    }

    fn n_xp_reward(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        if !self.this_is_mission(c)? {
            return outside_binding(c);
        }
        let alternate = arg_truth(c, 1)?;
        Ok(Value::Int(self.experience_reward(c.vm, alternate)?))
    }

    fn n_game_stage(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        if !self.this_is_mission(c)? {
            return outside_binding(c);
        }
        Ok(Value::Int(self.current_game_stage()))
    }

    fn n_currency_type(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        if !self.this_is_mission(c)? {
            return outside_binding(c);
        }
        let reward = self.reward_data(c.vm, arg_truth(c, 0)?)?;
        let t = field(&c.vm.space, &reward, "CurrencyRewardType").map_or(0, integer);
        Ok(Value::Byte(u8::try_from(t).unwrap_or(0)))
    }

    // Credits: GlobalsDefinition.MissionCreditRewardFormula x CreditRewardMultiplier, truncated. The formula's attribute
    // values were not decoded in the note, so only a multiplier of exactly 0 (the Fire mission) is answered; any other
    // is logged as not implemented, as are optional objectives' currency rewards. Other currencies:
    // OtherCurrencyReward, truncated.
    fn n_currency(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        if !self.this_is_mission(c)? {
            return outside_binding(c);
        }
        let reward = self.reward_data(c.vm, arg_truth(c, 1)?)?;
        let currency = field(&c.vm.space, &reward, "CurrencyRewardType").map_or(0, integer);
        if self.inner.mission.has_optional_objective() {
            Self::not_implemented(
                c.vm,
                "MissionDefinition.GetCurrencyReward: optional objective currency rewards",
                NotYet::not_ported("GetCurrencyReward: the optional objectives' currency rewards", RECORD),
            )?;
        }
        let need = |name: &str| {
            field(&c.vm.space, &reward, name)
                .cloned()
                .ok_or_else(|| data(format!("mission script bridge: the reward has no {name}")))
        };
        if currency != 0 {
            let other = need("OtherCurrencyReward")?;
            let x = self.evaluate(c.vm, &other)?;
            // The C++ int64_t(std::trunc(x)) made an int.
            return Ok(Value::Int(ow_core::num::trunc_i64_wrap_i32(f64::from(x))));
        }
        let multiplier = need("CreditRewardMultiplier")?;
        if self.evaluate(c.vm, &multiplier)? != 0.0 {
            Self::not_implemented(
                c.vm,
                "MissionDefinition.GetCurrencyReward: credits need MissionCreditRewardFormula attribute values",
                NotYet::not_read(
                    "GetCurrencyReward's credits: GlobalsDefinition.MissionCreditRewardFormula x CreditRewardMultiplier \
                     (the formula's attribute values are not decoded in the record)",
                    RECORD,
                ),
            )?;
        }
        Ok(Value::Int(0))
    }

    // The alternate reward when the last objective set of the NextSet chain was not fully progressed (note: low-medium
    // confidence). Required progress is the count, or for an item-remembering objective its full bit mask
    // (UNVERIFIED).
    fn n_alternate(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        if !self.this_is_mission(c)? {
            return outside_binding(c);
        }
        let def = self.inner.mission.definition();
        let initial = prop(&c.vm.space, def, "InitialObjectiveSet")?;
        let defs = prop(&c.vm.space, def, "ObjectiveDefs")?;
        let (Some(initial @ Value::Object(Some(_))), Some(defs)) = (initial, defs) else {
            return Ok(Value::Bool(false));
        };
        let mut set_obj = self.load_ref(c.vm, &initial)?;
        for _ in 0..NEXT_SET_GUARD {
            match prop(&c.vm.space, set_obj, "NextSet")? {
                Some(next @ Value::Object(Some(_))) => set_obj = self.load_ref(c.vm, &next)?,
                _ => break,
            }
        }
        let members = prop(&c.vm.space, set_obj, "ObjectiveDefinitions")?;
        let members: Vec<Value> = members
            .as_ref()
            .and_then(|m| m.elements())
            .map(<[Value]>::to_vec)
            .unwrap_or_default();
        let progress: Vec<Value> = c.arg(0)?.elements().map(<[Value]>::to_vec).unwrap_or_default();
        let defs: Vec<Value> = defs.elements().map(<[Value]>::to_vec).unwrap_or_default();
        for member in members {
            let target = match &member {
                Value::Object(o) => *o,
                _ => None,
            };
            let Some(index) = defs.iter().position(|d| matches!(d, Value::Object(o) if *o == target)) else {
                // Not one of the mission's objectives.
                return Ok(Value::Bool(false));
            };
            let Some(done) = progress.get(index) else {
                continue;
            };
            let objective = self.load_ref(c.vm, &member)?;
            let wanted = prop(&c.vm.space, objective, "ObjectiveCount")?.map_or(1, |v| integer(&v));
            let mask = prop(&c.vm.space, objective, "bRememberItemsWithinObjective")?
                .is_some_and(|v| v.truth(&c.vm.space.names));
            let full = if mask {
                let bits = u32::try_from(wanted)
                    .ok()
                    .filter(|&b| b < 63)
                    .ok_or_else(|| data(format!("mission script bridge: an objective count of {wanted} as a bit mask")))?;
                (1i64 << bits) - 1
            } else {
                wanted
            };
            if integer(done) < full {
                return Ok(Value::Bool(true));
            }
        }
        Ok(Value::Bool(false))
    }

    // Reward items: only the empty case (the Fire mission). Item generation and pool rolls are not done here
    // (NATIVE_LOOT.md, lane W6-LOOT).
    fn n_item_rewards(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        if !self.this_is_mission(c)? {
            return outside_binding(c);
        }
        let alternate = field(&c.vm.space, c.arg(1)?, "bGrantAltReward").is_some_and(|v| v.truth(&c.vm.space.names));
        let reward = self.reward_data(c.vm, alternate)?;
        let any = |name: &str| {
            field(&c.vm.space, &reward, name)
                .and_then(|v| v.elements())
                .is_some_and(|e| !e.is_empty())
        };
        if any("RewardItems") || any("RewardItemPools") {
            Self::not_implemented(
                c.vm,
                "MissionDefinition.GetItemRewardsForPlayer: item rewards need balance generation and pool rolls (NATIVE_LOOT.md)",
                NotYet::layer_absent(
                    "GetItemRewardsForPlayer's reward items (balance generation and pool rolls)",
                    LOOT_LAYER,
                    RECORD,
                ),
            )?;
        }
        Ok(Value::None)
    }

    // Objective progress through the tracker (NATIVE_OBJECTIVE_TRIGGERS.md, UNVERIFIED): IsMissionObjectiveActive is
    // "can this objective be updated now", the tracker's Active classification; UpdateObjective only queues.
    fn n_objective_active(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        if !self.on(c, self.inner.o.tracker) {
            return outside_binding(c);
        }
        let path = path_of(c.vm, c.arg(0)?)?;
        Ok(Value::Bool(self.inner.mission.objective_state(&path) == "Active"))
    }

    fn n_objective_complete(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        if !self.on(c, self.inner.o.tracker) {
            return outside_binding(c);
        }
        let path = path_of(c.vm, c.arg(0)?)?;
        Ok(Value::Bool(self.inner.mission.objective_state(&path) == "Complete"))
    }

    fn n_set_active(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        if !self.on(c, self.inner.o.tracker) {
            return outside_binding(c);
        }
        let path = path_of(c.vm, c.arg(0)?)?;
        Ok(Value::Bool(self.inner.mission.set_active(&path)))
    }

    fn n_update_objective(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        if !self.on(c, self.inner.o.tracker) {
            return outside_binding(c);
        }
        let path = path_of(c.vm, c.arg(0)?)?;
        if !path.is_empty() {
            let bit = if c.has(1) { arg_int(c, 1)? } else { 0 };
            self.inner.mission.update_objective_by_path(c.vm, &path, bit)?;
        }
        Ok(Value::None)
    }

    // GetObjectivesProgress(Mission, out Progress, Flag), Off mode only (NATIVE_SLICE_OFF_STOPS.md, UNVERIFIED; the C++
    // stubs it, so Reference keeps the stub). No record (another mission): the array is left as passed. Otherwise it
    // becomes the record's progress, one value per entry of the mission's `ObjectiveDefs`. With the flag set, an
    // all-or-nothing objective that fails the objective's "is complete" test reads 0; that test is not read, so a
    // `bAllOrNothing` entry that is not already 0 is `NotYet { NotRead }` (an entry that is 0 stays 0 either way).
    fn n_objectives_progress(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        if !self.on(c, self.inner.o.tracker) {
            return outside_binding(c);
        }
        if !self.is_mission(c.vm, &c.arg(0)?.clone())? {
            return Ok(Value::None);
        }
        let flag = c.arg(2)?.truth(&c.vm.space.names);
        let def = self.inner.mission.definition();
        let defs = prop(&c.vm.space, def, "ObjectiveDefs")?;
        let defs: Vec<Value> = defs.as_ref().and_then(|d| d.elements()).map(<[Value]>::to_vec).unwrap_or_default();
        let mut progress = Vec::with_capacity(defs.len());
        for d in &defs {
            let value = self.inner.mission.stored_progress(&path_of(c.vm, d)?);
            if flag && value != 0 && matches!(d, Value::Object(Some(_))) {
                let objective = self.load_ref(c.vm, d)?;
                if prop(&c.vm.space, objective, "bAllOrNothing")?.is_some_and(|v| v.truth(&c.vm.space.names)) {
                    return Err(NotYet::not_read(
                        "MissionTracker.GetObjectivesProgress with its flag set, on an all-or-nothing objective with \
                         progress (the objective's \"is complete\" test)",
                        "NATIVE_SLICE_OFF_STOPS.md",
                    )
                    .into());
                }
            }
            progress.push(Value::Int(value));
        }
        c.set_out(1, Value::Array(Rc::new(progress)))?;
        Ok(Value::None)
    }

    // RegisterMissionObserver: the observer is told "level load" at once and then every notification the tracker
    // raises (MissionScript::notify).
    fn n_register_observer(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        if !self.on(c, self.inner.o.tracker) {
            return outside_binding(c);
        }
        if let Some(o) = arg_object(c, 0)? {
            self.inner.state.borrow_mut().observers.push(o);
            let args = vec![
                Value::Object(Some(self.inner.o.tracker)),
                Value::Object(Some(self.inner.mission.definition())),
            ];
            call_by_name(c.vm, o, "MissionReactionLevelLoad", args)?;
        }
        Ok(Value::None)
    }

    // IsPlayerOwned: the owner chain's root, its controller (a pawn's Controller, a controller itself), and whether that
    // controller is the player's.
    fn n_player_owned(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        let mut root = c.this;
        for _ in 0..16 {
            let Some(r) = root else { break };
            match prop(&c.vm.space, r, "Owner")? {
                Some(Value::Object(Some(owner))) => root = Some(owner),
                _ => break,
            }
        }
        let controller = self.inner.o.controller;
        let owner = match root {
            Some(r) if r != controller => match prop(&c.vm.space, r, "Controller")? {
                Some(Value::Object(o)) => o,
                _ => None,
            },
            other => other,
        };
        Ok(Value::Bool(owner == Some(controller)))
    }

    // The pawn branch of OnExpLevelChange: the interface getter answers the pawn itself, the game stage is recorded only
    // (UNVERIFIED stand-ins, as the C++).
    fn n_a_willow_pawn(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        Ok(Value::Object(c.this))
    }

    fn n_set_game_stage(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        let stage = integer(c.arg(0)?);
        let mut s = self.inner.state.borrow_mut();
        if c.this.is_some() && c.this == s.player_pawn {
            s.pawn_game_stage = stage;
        }
        Ok(Value::None)
    }

    fn n_run_behaviors(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        if !self.inner.state.borrow().in_level_up {
            // The C++ runs RunBehaviors only inside its level-up stand-in and logs a stub elsewhere (R3: Reference only).
            if c.vm.parity != ParityMode::Reference {
                return Err(NotYet::not_ported(
                    "BehaviorBase.RunBehaviors outside the level-up (the behavior collections it runs)",
                    RECORD,
                )
                .into());
            }
            let key = key_of(c.vm, c.function);
            c.vm
                .log
                .push(format!("{STUB_PREFIX}{key} (outside the level-up stand-in)"));
            return Ok(Value::None);
        }
        Err(NotYet::not_ported(
            "BehaviorBase.RunBehaviors during the level-up (the C++ stand-in that applies the OnLevelUp skills' \
             effects by their data; skills are lane W6-SKILLS)",
            RECORD,
        )
        .into())
    }

    fn load_ref(&self, vm: &mut Vm, v: &Value) -> Result<ObjRef, MissionError> {
        load_ref(vm, Some(v))?.ok_or_else(|| data("mission script bridge: unresolved reference".to_string()))
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The director: his table, his use chain and the mission screen
// ---------------------------------------------------------------------------------------------------------------

/// The script behavior classes of the director's chain: each runs its own `ApplyBehaviorToContext` on the VM.
const SCRIPT_BEHAVIORS: [&str; 5] = [
    "GearboxFramework.Behavior_IsSequenceEnabled",
    "WillowGame.Behavior_RemoteCustomEvent",
    "WillowGame.Behavior_PlayAIMissionContextDialog",
    "WillowGame.Behavior_HasMissions",
    "WillowGame.Behavior_ShowMissionInterface",
];

/// World operations of his spawn-time sequences that nothing here runs: listed at the boundary.
const BOUNDARY_BEHAVIORS: [&str; 4] = [
    "GearboxFramework.Behavior_AIHold",
    "WillowGame.Behavior_SetPawnThrottleData",
    "WillowGame.Behavior_ChangeUsability",
    "WillowGame.Behavior_SetUsableIcon",
];

impl MissionScript {
    /// The director (NATIVE_USE_INTERACTION.md, UNVERIFIED): the stock archetype `path` of `package` (a
    /// `WillowAIPawn` whose `MissionDirectivesDefinition` table is the data's), with authority in the VM world. His
    /// list scripts build the screen's lists over that table and the tracker's availability queries; `AcceptMission`
    /// and `ServerCompleteMission` then get him as their director. With his AI-definition provider at
    /// [`MARCUS_PROVIDER`], his use chain is bound and the provider registered on his consumer. False when the package
    /// has no such pawn.
    pub fn place_marcus(&self, vm: &mut Vm, package: &PackageName, path: &str) -> Result<bool, MissionError> {
        let (_, store) = vm.defs.code_mut();
        let id = store.load(package).map_err(ow_defs::store::DefError::from)?;
        let Some(index) = store.get(id).find_export(path)?.filter(|&i| i > 0) else {
            return Ok(false);
        };
        // mission_script.cpp:487, `instantiateExport(package, index, 4)`.
        let director = instantiate_export(vm, id, index, PropertyPrefix::Ordinary)?;
        self.inner.state.borrow_mut().director = Some(director);
        set(&mut vm.space, director, "Role", Value::Byte(self.inner.o.authority))?;
        set(&mut vm.space, director, "WorldInfo", Value::Object(Some(self.inner.o.world_info)))?;
        // The directive table is archetype data: a reference to a MissionDirectivesDefinition export, made into its
        // own object (mission_script.cpp:491-492, `loadRef`: `instantiateExport` at 4 of the reference's export).
        let directives = get(&vm.space, director, "MissionDirectives")?;
        if let Some((package, index)) = export_of(vm, &directives)? {
            let o = instantiate_export(vm, package, index, PropertyPrefix::Ordinary)?;
            set(&mut vm.space, director, "MissionDirectives", Value::Object(Some(o)))?;
        }
        // The constructor leaves the ConsumerHandle at -1 (invalid); the consumer registration assigns it once (below).
        self.set_pid(vm, director, -1)?;
        // What PlayOnUseDialog's guard needs: a mind whose AIClass is the archetype's AI class, and his dialog
        // component, both made from the archetype's references (the mind is the bare WillowMind object;
        // mission_script.cpp:498-511: only an export reference, `resourceIndex > 0`, is made, at 4 and at 8).
        if let Some(ai_class) = prop(&vm.space, director, "AIClass")?
            && let Some((package, index)) = export_of(vm, &ai_class)?.filter(|&(_, i)| i > 0)
        {
            let loaded = instantiate_export(vm, package, index, PropertyPrefix::Ordinary)?;
            set(&mut vm.space, director, "AIClass", Value::Object(Some(loaded)))?;
            let director_class = vm.space.heap.get(director)?.class;
            if vm.space.prop(director_class, "MyWillowMind").is_some() {
                let mind = new_object(vm, "WillowGame.WillowMind")?;
                set(&mut vm.space, mind, "AIClass", Value::Object(Some(loaded)))?;
                set(&mut vm.space, director, "MyWillowMind", Value::Object(Some(mind)))?;
            }
        }
        if let Some(component) = prop(&vm.space, director, "DialogComponent")?
            && let Some((package, index)) = export_of(vm, &component)?.filter(|&(_, i)| i > 0)
        {
            let loaded = instantiate_export(vm, package, index, PropertyPrefix::Component)?;
            self.inner.state.borrow_mut().dialog_component = Some(loaded);
            set(&mut vm.space, director, "DialogComponent", Value::Object(Some(loaded)))?;
        }
        // The same pawn is the waypoint's Marcus toucher.
        self.inner.state.borrow_mut().marcus_pawn = Some(director);
        let Some(definition) = tracker::find_object(vm, package, MARCUS_PROVIDER)? else {
            return Ok(true);
        };
        self.bind_use(vm)?;
        self.bind_screen(vm, director);
        let mut provider = Provider::new(vm, definition)?;
        for class in SCRIPT_BEHAVIORS {
            let w = self.weak();
            provider.handle(
                class,
                Handler::run(move |vm, p, id, _event| {
                    let inner = w
                        .upgrade()
                        .ok_or_else(|| data("the mission script bridge is gone".to_string()))?;
                    MissionScript { inner }.script_behavior(vm, p, id)
                }),
            );
        }
        for class in BOUNDARY_BEHAVIORS {
            provider.report_at_boundary(class, None, Vec::new());
        }
        // InitializeBehaviorProviders: while the handle is -1 the pawn registers as a consumer and keeps the answer.
        if self.pid(vm, director)? == Some(-1)
            && let Some(pid) = self.inner.marcus_pid.get()
        {
            self.set_pid(vm, director, pid)?;
        }
        *self.inner.marcus.borrow_mut() = Some(provider);
        self.inner.has_marcus.set(true);
        self.run(vm, |vm| self.with_marcus(vm, |p, vm| p.register_consumer(vm)));
        let mut slot = self.inner.marcus.borrow_mut();
        if let Some(p) = slot.as_mut() {
            let mut s = self.inner.state.borrow_mut();
            for line in &p.boundary {
                s.notes
                    .push(format!("Marcus provider, reached at registration but not run: {line}"));
            }
            for error in p.errors.drain(..) {
                s.errors.push(format!("Marcus provider: {error}"));
            }
        }
        Ok(true)
    }

    pub fn has_marcus(&self) -> bool {
        self.inner.state.borrow().director.is_some()
    }

    fn pid(&self, vm: &Vm, pawn: ObjRef) -> Result<Option<i32>, MissionError> {
        Ok(prop(&vm.space, pawn, "ConsumerHandle")?
            .and_then(|h| field(&vm.space, &h, "PID").map(integer32)))
    }

    fn set_pid(&self, vm: &mut Vm, pawn: ObjRef, pid: i32) -> Result<(), MissionError> {
        if let Some(handle) = prop(&vm.space, pawn, "ConsumerHandle")?
            && let Some(v) = with_field(&vm.space, &handle, "PID", Value::Int(pid))
        {
            set(&mut vm.space, pawn, "ConsumerHandle", v)?;
        }
        Ok(())
    }

    /// Runs `f` on the director's provider taken out of its slot (nothing without one).
    fn with_marcus(
        &self,
        vm: &mut Vm,
        f: impl FnOnce(&mut Provider, &mut Vm) -> Result<(), MissionError>,
    ) -> Result<(), MissionError> {
        if !self.inner.has_marcus.get() {
            return Ok(());
        }
        self.marcus_usable()?;
        let taken = self.inner.marcus.borrow_mut().take();
        let Some(mut p) = taken else {
            return Err(NotYet::not_ported(tracker::PROVIDER_BUSY, RECORD).into());
        };
        let result = f(&mut p, vm);
        *self.inner.marcus.borrow_mut() = Some(p);
        result
    }

    /// One script behavior of the director's chain: its `ApplyBehaviorToContext` runs once per context object the
    /// kernel resolves for it, with the provider lent back to its slot (the natives it calls read and fire on it).
    /// An empty context list: the behavior does not run for this pass (no line, no output); the provider still takes
    /// its default link when the behavior supports one (NATIVE_BEHAVIOR_CONTEXT.md, UNVERIFIED).
    fn script_behavior(
        &self,
        vm: &mut Vm,
        p: &mut Provider,
        id: ow_behavior::kernel::BehaviorId,
    ) -> Result<Vec<i32>, MissionError> {
        let behavior = p.behavior(id).clone();
        let director = self
            .director()
            .ok_or_else(|| data("mission script bridge: the director's chain ran without a director".to_string()))?;
        let contexts = p.contexts_for(vm, id, director)?;
        let (class, state) = {
            let x = vm.space.heap.get(behavior.object)?;
            (x.class, x.state)
        };
        let apply = vm.find_method(class, "ApplyBehaviorToContext", state)?;
        // The output links go to the behavior kernel pack's shared collector.
        let (ran, ids) = natives::collect(vm, |vm| {
            lend(&self.inner.marcus, &self.inner.marcus_lost, p, || {
                for &context in &contexts {
                    let Some(f) = apply else {
                        self.inner
                            .state
                            .borrow_mut()
                            .errors
                            .push(format!("no ApplyBehaviorToContext on {}", behavior.class));
                        break;
                    };
                    let params = vm.function(f).map(|d| d.params.clone()).unwrap_or_default();
                    let args = params
                        .iter()
                        .map(|param| {
                            let name = vm.space.names.text(param.name);
                            if name.eq_ignore_ascii_case("ContextObject") {
                                Value::Object(Some(context))
                            } else if name.eq_ignore_ascii_case("SelfObject") {
                                Value::Object(Some(director))
                            } else {
                                vm.space.classes.zero_value(&param.kind)
                            }
                        })
                        .collect::<Vec<_>>();
                    let object = behavior.object;
                    self.run(vm, |vm| vm.call(f, Some(object), args).map(|_| ()).map_err(BehaviorError::Vm));
                }
                Ok(())
            })
        })?;
        ran?;
        if contexts.is_empty() {
            return Ok(ids);
        }
        let sequence = text(&vm.space, prop(&vm.space, behavior.object, "SequenceName")?.as_ref());
        let mut line = behavior.name.clone();
        if !sequence.is_empty() {
            line.push_str(&format!("({sequence})"));
        }
        line.push_str(" ->");
        for i in &ids {
            line.push_str(&format!(" {i}"));
        }
        self.inner.state.borrow_mut().use_.cascade.push(line);
        Ok(ids)
    }

    /// Refused once a failed lend lost the director's provider: every reader and caller of it (module documentation of
    /// `crate::tracker`, "Re-entrancy").
    fn marcus_usable(&self) -> Result<(), MissionError> {
        if self.inner.marcus_lost.get() {
            return Err(NotYet::not_ported(tracker::PROVIDER_LOST, RECORD).into());
        }
        Ok(())
    }

    /// The consumer's registered provider for a handle and a provider object (NATIVE_MARCUS_USE_CHAIN.md result rule).
    /// Refused when the director's provider was lost.
    fn registered(&self, vm: &Vm, handle: &Value, provider: &Value) -> Result<bool, MissionError> {
        self.marcus_usable()?;
        let Value::Object(Some(o)) = provider else {
            return Ok(false);
        };
        let slot = self.inner.marcus.borrow();
        let Some(p) = slot.as_ref() else {
            return Ok(false);
        };
        if !p.registered() {
            return Ok(false);
        }
        let pid = field(&vm.space, handle, "PID").map(integer32);
        if pid.is_none() || pid != self.inner.marcus_pid.get() {
            return Ok(false);
        }
        Ok(stock_path(vm, *o)? == p.path())
    }

    /// The stock use chain's natives (NATIVE_MARCUS_USE_CHAIN.md, NATIVE_MISSION_DISPATCH.md A2,
    /// NATIVE_CONTROLLER_HELPERS.md; all UNVERIFIED), bound when the director is placed. The behavior kernel's
    /// natives are the kernel pack's (`ow_behavior::kernel::natives`: `ActivateBehaviorOutputLink`,
    /// `WillowPawn.GetBehaviorConsumerHandle`). The director's handle is allocated there (the lowest free `PID` from
    /// [`MARCUS_PID`], so a consumer the kernel registered first keeps its own), and the bodies below run for his
    /// handle. They are also the Reference bodies for a handle nobody owns, as the C++ bridge binds these natives.
    fn bind_use(&self, vm: &mut Vm) -> Result<(), MissionError> {
        let pid = natives::allocate(vm, CONSUMER_OWNER, MARCUS_PID)?;
        self.inner.marcus_pid.set(Some(pid));
        const ROUTED: [(&str, Body); 3] = [
            (natives::IS_SEQUENCE_ENABLED, MissionScript::n_sequence_enabled),
            (natives::EVENT_FROM_SCRIPT, MissionScript::n_event_from_script),
            (natives::CONSUMER_HANDLE, MissionScript::n_consumer_handle),
        ];
        for (path, body) in ROUTED {
            let f = self.native(path, body);
            natives::route(vm, CONSUMER_OWNER, path, f.clone())?;
            natives::fallback(vm, CONSUMER_OWNER, path, f)?;
        }
        const BOUND: [(&str, Body); 2] = [
            (
                "GearboxFramework.BehaviorHelpers.ResolveBehaviorProviderDefinitionReference",
                MissionScript::n_resolve_provider,
            ),
            ("Engine.BehaviorBase.GetBehaviorContext", MissionScript::n_behavior_context),
        ];
        for (path, body) in BOUND {
            bind(vm, path, self.native(path, body))?;
        }
        if self.inner.state.borrow().dialog_component.is_some() {
            const DIALOG: [(&str, Body); 2] = [
                ("WillowGame.WillowDialogGlobalsDefinition.Get", MissionScript::n_dialog_globals),
                ("GearboxFramework.GearboxDialogComponent.TriggerEvent", MissionScript::n_trigger_event),
            ];
            for (path, body) in DIALOG {
                bind(vm, path, self.native(path, body))?;
            }
        }
        // The client RPC that opens a movie is presentation: not run, reported (the host answers it).
        let path = "WillowGame.WillowPlayerController.ClientGFxPlayMovie";
        vm.override_script(path, self.native(path, MissionScript::n_play_movie))?;
        Ok(())
    }

    // IsBehaviorSequenceEnabled(ConsumerHandle, ProviderDefinition, SequenceName): true only when the consumer has
    // that provider registered, the provider has the sequence and its enabled bit is set; state is only read.
    fn n_sequence_enabled(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        let (handle, provider) = (c.arg(0)?.clone(), c.arg(1)?.clone());
        if !self.registered(c.vm, &handle, &provider)? {
            return Ok(Value::Bool(false));
        }
        let name = text(&c.vm.space, Some(c.arg(2)?));
        let slot = self.inner.marcus.borrow();
        Ok(Value::Bool(slot.as_ref().is_some_and(|p| p.sequence_enabled(&name))))
    }

    // ActivateBehaviorEventFromScript(ConsumerHandle, ProviderDefinition, EventName, optional EventOutputToActivate,
    // optional Parameters): a None provider fires nothing; an omitted filter is -1 (every link); otherwise the event
    // is fired on that provider's enabled sequences.
    fn n_event_from_script(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        let provider = c.arg(1)?.clone();
        if !matches!(provider, Value::Object(Some(_))) {
            return Ok(Value::None);
        }
        let filter = if c.has(3) { arg_int(c, 3)? } else { -1 };
        let handle = c.arg(0)?.clone();
        let name = text(&c.vm.space, Some(c.arg(2)?));
        if self.registered(c.vm, &handle, &provider)? {
            self.with_marcus(c.vm, |p, vm| p.fire_event(vm, &name, &DetMap::new(), filter, &[]))?;
        }
        Ok(Value::None)
    }

    // ResolveBehaviorProviderDefinitionReference(SourceBehavior, ProviderReference, PathName): a non-empty path wins
    // (its set name slots joined into one object path; only the director's provider is found, and the subobject
    // separator is written as a dot), else the reference, else the Outer of the source behavior when it is a provider
    // definition, else None. The reference branch is read only coarsely (the object itself is returned).
    fn n_resolve_provider(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        let mut full = String::new();
        if let Some(slots) = field_value(&c.vm.space, c.arg(2)?, "PathComponentNames") {
            let names: Vec<Value> = match slots.elements() {
                Some(e) => e.to_vec(),
                None => vec![slots.clone()],
            };
            for slot in names {
                let t = text(&c.vm.space, Some(&slot));
                if !t.is_empty() && t != "None" {
                    if !full.is_empty() {
                        full.push('.');
                    }
                    full.push_str(&t);
                }
            }
        }
        if !full.is_empty() {
            self.marcus_usable()?;
            let slot = self.inner.marcus.borrow();
            let found = slot
                .as_ref()
                .filter(|p| full == p.path())
                .map(|p| p.definition());
            return Ok(Value::Object(found));
        }
        if let Value::Object(Some(o)) = c.arg(1)? {
            return Ok(Value::Object(Some(*o)));
        }
        if let Some(source) = arg_object(c, 0)?
            && let Some(outer) = c.vm.space.heap.get(source)?.outer
        {
            let base = c.vm.find_class("GearboxFramework.BehaviorProviderDefinition")?;
            if is_child_of(&c.vm.space, outer, base)? {
                return Ok(Value::Object(Some(outer)));
            }
        }
        Ok(Value::Object(None))
    }

    // GetBehaviorContext (NATIVE_BEHAVIOR_CONTEXT.md, UNVERIFIED): a pure resolver over its arguments. The selector
    // picks the base object: 0 SelfObject, 1 MyInstigatorObject, 2 OtherEventParticipantObject, 4 the struct's own
    // ContextObject; 3 (EventData) and anything else give None. A non-empty InstancedDataContextName asks the
    // object's instance data (not implemented: listed with the stubs).
    fn n_behavior_context(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        let d = c.arg(0)?.clone();
        let selector = field(&c.vm.space, &d, "BehaviorContext").map_or(0, integer);
        let name = text(&c.vm.space, field(&c.vm.space, &d, "InstancedDataContextName"));
        let base = match selector {
            0 => arg_object(c, 1)?,
            1 => arg_object(c, 2)?,
            2 => arg_object(c, 3)?,
            4 => match field(&c.vm.space, &d, "ContextObject") {
                Some(Value::Object(o)) => *o,
                _ => None,
            },
            _ => None,
        };
        let named = !name.is_empty() && name != "None";
        if base.is_some() && named {
            Self::not_implemented(
                c.vm,
                "BehaviorBase.GetBehaviorContext instance-data path",
                NotYet::not_ported("GetBehaviorContext with an InstancedDataContextName (the instance-data path)", RECORD),
            )?;
        }
        Ok(Value::Object(if named { None } else { base }))
    }

    // GetBehaviorConsumerHandle (IBehaviorConsumer), as the C++ bridge binds it: the director's own ConsumerHandle; any
    // other pawn is outside the binding. The kernel pack runs it in Reference only (Off reads every pawn's field).
    fn n_consumer_handle(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        let Some(director) = self.director().filter(|d| c.this == Some(*d)) else {
            return outside_binding(c);
        };
        get(&c.vm.space, director, "ConsumerHandle")
    }

    fn n_dialog_globals(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        if let Some(g) = self.inner.state.borrow().dialog_globals {
            return Ok(Value::Object(Some(g)));
        }
        let g = self.mission_object(c.vm, DIALOG_GLOBALS_PATH)?;
        self.inner.state.borrow_mut().dialog_globals = g;
        Ok(Value::Object(g))
    }

    // GearboxDialogComponent.TriggerEvent(EventTag, Other, ObjectParameter, optional EventData) on the director's
    // component (NATIVE_DIALOG.md "Component TriggerEvent", NATIVE_DIALOG_GROUPS.md; UNVERIFIED): the speaker is the
    // director, whose body class gives the name tag and the groups (`DialogSystem::pawn_dialog`); the tag runs through
    // the mission's dialog (`DialogSystem::trigger_on_component`), the same one the tracker's dialog behavior uses, and
    // a line that starts reaches the host as the tracker's `Dialog` effect. The tag, the speaker and the other object's
    // class are recorded too: inside ServerCompleteMission as the turned-in dialog, while the screen closes as the
    // dismissal dialog, otherwise as the on-use dialog (with the speaker's group count, first and last). Returns the
    // zero event data.
    fn n_trigger_event(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        let (component, turning_in, closing) = {
            let s = self.inner.state.borrow();
            (s.dialog_component, s.turning_in, s.closing)
        };
        if c.this.is_none() || c.this != component {
            return outside_binding(c);
        }
        let director = self
            .director()
            .ok_or_else(|| data("mission script bridge: the director's component has no director".to_string()))?;
        let tag = c.arg(0)?.clone();
        let other = match arg_object(c, 1)? {
            Some(o) => class_name(&c.vm.space, o)?,
            None => String::new(),
        };
        // The body class is the definition store's object for its path (the director's references are; module
        // documentation), which the dialog loads.
        let body = prop(&c.vm.space, director, "BodyClass")?.unwrap_or(Value::None);
        let dialog = self.inner.mission.dialog();
        let busy = || data("dialog: the dialog system is already in use".to_string());
        let pawn = dialog.try_borrow_mut().map_err(|_| busy())?.pawn_dialog(c.vm, &body)?;
        let tag_path = path_of(c.vm, &tag)?;
        let speaker = stock_path(c.vm, director)?;
        if turning_in {
            self.inner.mission.note(EffectKind::TurnInDialog, &tag_path, &speaker, &other);
        } else if closing {
            self.inner.mission.note(EffectKind::DismissalDialog, &tag_path, &speaker, &other);
        } else {
            let first = match pawn.groups.first() {
                Some(g) => DialogSystem::object_path(c.vm, g)?,
                None => String::new(),
            };
            let last = match pawn.groups.last() {
                Some(g) => DialogSystem::object_path(c.vm, g)?,
                None => String::new(),
            };
            let mut s = self.inner.state.borrow_mut();
            s.use_.on_use_tag = tag_path;
            s.use_.on_use_speaker = speaker;
            s.use_.on_use_target = other;
            s.use_.on_use_group_count = pawn.groups.len();
            s.use_.on_use_first_group = first;
            s.use_.on_use_last_group = last;
        }
        if !pawn.name_tag.is_empty() && matches!(tag, Value::Object(Some(_))) {
            dialog
                .try_borrow_mut()
                .map_err(|_| busy())?
                .trigger_on_component(c.vm, &pawn.name_tag, &pawn.groups, &tag)?;
        }
        Ok(zero_result(c))
    }

    fn n_play_movie(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        if !self.on(c, self.inner.o.controller) {
            return outside_binding(c);
        }
        let movie = path_of(c.vm, c.arg(0)?)?;
        let director = path_of(c.vm, c.arg(1)?)?;
        let mut s = self.inner.state.borrow_mut();
        s.use_.interface_opened = true;
        s.use_.movie = movie;
        s.use_.director = director;
        Ok(Value::None)
    }

    /// The screen's lists (NATIVE_USE_INTERACTION.md, UNVERIFIED): the director's own `GetEligibleMissions`,
    /// `GetInProgressMissions` and `GetRedeemableMissions` scripts. Empty without a director.
    pub fn mission_lists(&self, vm: &mut Vm, completed: &DetSet<String>) -> Result<MissionLists, MissionError> {
        let mut lists = MissionLists::default();
        let Some(director) = self.director() else {
            return Ok(lists);
        };
        self.inner.state.borrow_mut().completed = completed.clone();
        self.run(vm, |vm| {
            for (name, into) in [
                ("GetEligibleMissions", &mut lists.eligible),
                ("GetInProgressMissions", &mut lists.in_progress),
                ("GetRedeemableMissions", &mut lists.redeemable),
            ] {
                for m in director_list(vm, director, name)? {
                    if let Value::Object(Some(o)) = m
                        && matches!(vm.space.heap.get(o)?.origin, Origin::Export { .. } | Origin::Resource { .. })
                    {
                        into.push(stock_path(vm, o)?);
                    }
                }
            }
            Ok(())
        });
        Ok(lists)
    }

    /// The lists as `--mission-run` prints them (`script:screen`).
    pub fn mission_lists_json(&self, vm: &mut Vm, completed: &DetSet<String>) -> Result<String, MissionError> {
        let l = self.mission_lists(vm, completed)?;
        let one = |paths: &[String]| {
            paths
                .iter()
                .map(|p| ow_core::fmt::json_quote(p))
                .collect::<Vec<_>>()
                .join(",")
        };
        Ok(format!(
            "{{\"eligible\":[{}],\"in_progress\":[{}],\"redeemable\":[{}]}}",
            one(&l.eligible),
            one(&l.in_progress),
            one(&l.redeemable)
        ))
    }

    /// The use key's press on the director, after the host's use ray chose him (NATIVE_MARCUS_USE_CHAIN.md,
    /// UNVERIFIED): `AIDefinition.OnUsed` raises "OnUsed" with link filter 2 on his provider, the payload being the
    /// player's VM pawn and no component. The `ClientGFxPlayMovie` the chain ends in is reported, not run. A movie
    /// opens once: the screen's open runs only while it is closed.
    pub fn use_marcus(&self, vm: &mut Vm, completed: &DetSet<String>) -> Result<MarcusUse, MissionError> {
        self.inner.state.borrow_mut().use_ = MarcusUse::default();
        if !self.inner.has_marcus.get() {
            return Ok(MarcusUse::default());
        }
        self.inner.state.borrow_mut().completed = completed.clone();
        let pawn = self.pawn_for(vm, Toucher::Player)?;
        self.run(vm, |vm| {
            self.with_marcus(vm, |p, vm| p.fire_event(vm, "OnUsed", &DetMap::new(), 2, &[Some(pawn), None]))
        });
        let (opened, open) = {
            let s = self.inner.state.borrow();
            (s.use_.interface_opened, s.screen_open)
        };
        if opened && !open {
            self.open_screen(vm)?;
        }
        if let Some(p) = self.inner.marcus.borrow_mut().as_mut() {
            let mut s = self.inner.state.borrow_mut();
            for error in p.errors.drain(..) {
                s.errors.push(format!("Marcus provider: {error}"));
            }
        }
        Ok(self.inner.state.borrow().use_.clone())
    }

    /// [`MissionScript::use_marcus`] and its JSON (`script:use`): whether the interface opened, and the record.
    pub fn use_marcus_json(&self, vm: &mut Vm, completed: &DetSet<String>) -> Result<(bool, String), MissionError> {
        let u = self.use_marcus(vm, completed)?;
        let q = ow_core::fmt::json_quote;
        let cascade = u.cascade.iter().map(|l| q(l)).collect::<Vec<_>>().join(",");
        Ok((
            u.interface_opened,
            format!(
                "{{\"interface_opened\":{},\"movie\":{},\"director\":{},\"on_use_tag\":{},\"on_use_speaker\":{},\
                 \"on_use_target\":{},\"on_use_groups\":{{\"count\":{},\"first\":{},\"last\":{}}},\"cascade\":[{}]}}",
                u.interface_opened,
                q(&u.movie),
                q(&u.director),
                q(&u.on_use_tag),
                q(&u.on_use_speaker),
                q(&u.on_use_target),
                u.on_use_group_count,
                q(&u.on_use_first_group),
                q(&u.on_use_last_group),
                cascade
            ),
        ))
    }

    // ---- the mission screen ----------------------------------------------------------------------------------------

    /// The mission screen's stand-in (FIRE_MISSION_GOLDEN_TRACE.md, "The mission screen's close and the reward page";
    /// NATIVE_GFX_BRIDGE.md; UNVERIFIED beyond the observed order). The movie is not hosted: a VM object of the
    /// installed class `QuestAcceptGFxMovie` answers `WillowGFxUIManager.GetPlayingMovie` while the screen is open, so
    /// the controller's own script finds a movie and the movie's own entry-list script decides. The host answers
    /// `GetPlayingMovie`, `Close` (its script event `OnClose`, emulated), `DisplayRewardsPage` (the page waits) and
    /// `PlayUISound` (nothing). A package set without those classes has no screen: a note says why.
    fn bind_screen(&self, vm: &mut Vm, director: ObjRef) {
        if let Err(e) = self.try_bind_screen(vm, director) {
            let mut s = self.inner.state.borrow_mut();
            s.ui_manager = None;
            s.screen_movie = None;
            s.notes.push(format!("mission screen not hosted: {}", error_text(&e)));
        }
    }

    fn try_bind_screen(&self, vm: &mut Vm, director: ObjRef) -> Result<(), MissionError> {
        let movie_class = find_class(vm, "WillowGame.QuestAcceptGFxMovie")?;
        let manager_class = find_class(vm, "WillowGame.WillowGFxUIManager")?;
        const BOUND: [(&str, Body); 3] = [
            ("WillowGame.WillowGFxUIManager.GetPlayingMovie", MissionScript::n_playing_movie),
            ("GFxUI.GFxMoviePlayer.Close", MissionScript::n_close),
            ("GearboxFramework.GearboxGFxMovie.PlayUISound", MissionScript::n_ui_sound),
        ];
        for (path, body) in BOUND {
            bind(vm, path, self.native(path, body))?;
        }
        const OVERRIDES: [(&str, Body); 2] = [
            ("WillowGame.QuestAcceptGFxMovie.DisplayRewardsPage", MissionScript::n_rewards_page),
            ("WillowGame.QuestAcceptGFxMovie.DetermineQuestEntries", MissionScript::n_quest_entries),
        ];
        for (path, body) in OVERRIDES {
            vm.override_script(path, self.native(path, body))?;
        }
        let ui = vm.instantiate(manager_class, "")?;
        set_reference(&mut vm.space, self.inner.o.controller, "GFxUIManager", ui)?;
        let movie = vm.instantiate(movie_class, "")?;
        for name in ["MissionDirector", "FocusSubject", "ContextNPC"] {
            set_reference(&mut vm.space, movie, name, director)?;
        }
        let mut s = self.inner.state.borrow_mut();
        s.ui_manager = Some(ui);
        s.screen_movie = Some(movie);
        Ok(())
    }

    fn movie(&self) -> Option<ObjRef> {
        self.inner.state.borrow().screen_movie
    }

    fn n_playing_movie(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        let (ui, open, movie) = {
            let s = self.inner.state.borrow();
            (s.ui_manager, s.screen_open, s.screen_movie)
        };
        if c.this.is_none() || c.this != ui {
            return outside_binding(c);
        }
        Ok(Value::Object(if open { movie } else { None }))
    }

    // GFxMoviePlayer.Close(Unload): the script event OnClose is raised, then the movie stops. Only the script half is
    // modelled.
    fn n_close(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        if c.this.is_none() || c.this != self.movie() {
            return outside_binding(c);
        }
        self.close_movie(c.vm)?;
        Ok(Value::None)
    }

    fn n_ui_sound(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        if c.this.is_none() || c.this != self.movie() {
            return outside_binding(c);
        }
        Ok(Value::None)
    }

    // DisplayRewardsPage(MissionReward): the page is a flag that waits for confirm_reward; nothing is drawn.
    fn n_rewards_page(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        if c.this.is_none() || c.this != self.movie() {
            return outside_binding(c);
        }
        let mission = match field(&c.vm.space, c.arg(0)?, "Mission") {
            Some(v) => path_of(c.vm, v)?,
            None => String::new(),
        };
        self.inner.state.borrow_mut().reward_mission = mission.clone();
        self.inner.mission.note(EffectKind::RewardPage, &mission, "", "");
        Ok(Value::None)
    }

    // DetermineQuestEntries: the rows the director's lists give, redeemable first (status 3), then eligible (0); in
    // progress (1) apart. Its script grows arrays of structs from empty, which the C++ VM could not run, so the C++
    // builds the rows with the same rule; so does this port. The director's table names the mission by reference;
    // the row holds the mission object.
    fn n_quest_entries(&self, c: &mut NativeCall<'_>) -> Result<Value, MissionError> {
        let Some(movie) = self.movie().filter(|m| c.this == Some(*m)) else {
            return outside_binding(c);
        };
        let director = self
            .director()
            .ok_or_else(|| data("mission screen: no director".to_string()))?;
        let def = self.inner.mission.definition();
        let collect = |vm: &mut Vm, name: &str| -> Result<Vec<Value>, MissionError> {
            let list = director_list(vm, director, name)?;
            list.into_iter()
                .map(|m| Ok(if self.is_mission(vm, &m)? { Value::Object(Some(def)) } else { m }))
                .collect()
        };
        let redeemable = collect(c.vm, "GetRedeemableMissions")?;
        let eligible = collect(c.vm, "GetEligibleMissions")?;
        let in_progress = collect(c.vm, "GetInProgressMissions")?;
        let ty = c
            .vm
            .space
            .classes
            .struct_by_path("WillowGame.WillowPlayerController.StatusMenuMissionEligibilityData")
            .ok_or_else(|| data("struct not found: WillowGame.WillowPlayerController.StatusMenuMissionEligibilityData".to_string()))?;
        let row_default = c
            .vm
            .space
            .classes
            .struct_defaults(ty)
            .map_err(|e| BehaviorError::Vm(VmError::from(ow_object::heap::HeapError::Layout(e))))?;
        let rows = |space: &ObjectSpace, missions: &[Value], status: u8| -> Result<Vec<Value>, MissionError> {
            missions
                .iter()
                .map(|m| {
                    with_field(space, &row_default, "MissionDef", m.clone())
                        .and_then(|r| with_field(space, &r, "MissionStatus", Value::Byte(status)))
                        .ok_or_else(|| data("mission screen: the row struct lacks MissionDef or MissionStatus".to_string()))
                })
                .collect()
        };
        let mut list = rows(&c.vm.space, &redeemable, 3)?;
        list.extend(rows(&c.vm.space, &eligible, 0)?);
        let progress = rows(&c.vm.space, &in_progress, 1)?;
        let counts = [
            ("RedeemableCount", redeemable.len()),
            ("EligibleCount", eligible.len()),
            ("InProgressCount", in_progress.len()),
        ];
        set(&mut c.vm.space, movie, "MissionList", Value::array(list))?;
        set(&mut c.vm.space, movie, "InProgressMissionList", Value::array(progress))?;
        for (name, n) in counts {
            set(&mut c.vm.space, movie, name, Value::Int(script_int(n as i64)?))?;
        }
        Ok(Value::None)
    }

    /// `QuestAcceptGFxMovie.Start`'s part that reaches script (observed order: the director hears
    /// `OnPlayerOpenedMissionUI`, the entry list is built through his list functions, then the player's pawn begins
    /// using him).
    fn open_screen(&self, vm: &mut Vm) -> Result<(), MissionError> {
        self.inner.state.borrow_mut().screen_open = true;
        let director = self
            .director()
            .ok_or_else(|| data("mission screen: no director".to_string()))?;
        let (c, movie) = (self.inner.o.controller, self.movie());
        let pawn = self.pawn_for(vm, Toucher::Player)?;
        self.run(vm, |vm| {
            call_by_name(vm, director, "OnPlayerOpenedMissionUI", vec![Value::Object(Some(c))])?;
            if let Some(m) = movie {
                call_by_name(vm, m, "DetermineQuestEntries", Vec::new())?;
            }
            call_by_name(vm, director, "BeginUse", vec![Value::Object(Some(pawn))])?;
            Ok(())
        });
        Ok(())
    }

    /// The movie's `OnClose`, as far as it reaches script and was observed: the player stops using the director, then
    /// he hears `OnPlayerClosedMissionUI`.
    fn close_movie(&self, vm: &mut Vm) -> Result<(), MissionError> {
        if !self.inner.state.borrow().screen_open {
            return Ok(());
        }
        let director = self
            .director()
            .ok_or_else(|| data("mission screen: no director".to_string()))?;
        {
            let mut s = self.inner.state.borrow_mut();
            s.screen_open = false;
            s.closing = true;
        }
        let pawn = self.pawn_for(vm, Toucher::Player)?;
        let c = self.inner.o.controller;
        let result = call_by_name(vm, pawn, "ServerPlayerEndUseNPC", vec![Value::Object(Some(director))])
            .and_then(|_| call_by_name(vm, director, "OnPlayerClosedMissionUI", vec![Value::Object(Some(c))]));
        self.inner.state.borrow_mut().closing = false;
        result?;
        let path = stock_path(vm, director)?;
        self.inner.mission.note(EffectKind::InterfaceClosed, &path, "", "");
        Ok(())
    }

    pub fn screen_open(&self) -> bool {
        self.inner.state.borrow().screen_open
    }

    pub fn reward_pending(&self) -> bool {
        !self.inner.state.borrow().reward_mission.is_empty()
    }

    /// The player leaves the screen (Escape): the movie's `extChoiceCancelled`. Refused while the reward page waits.
    pub fn close_screen(&self, vm: &mut Vm) -> Result<bool, MissionError> {
        let Some(movie) = self.movie() else {
            return Ok(false);
        };
        if !self.screen_open() || self.reward_pending() {
            return Ok(false);
        }
        self.run(vm, |vm| call_by_name(vm, movie, "extChoiceCancelled", Vec::new()).map(|_| ()));
        Ok(!self.screen_open())
    }

    /// The player confirms the reward page: the controller's `MissionRewardsReceived` with the mission (observed).
    pub fn confirm_reward(&self, vm: &mut Vm) -> Result<bool, MissionError> {
        let mission = self.inner.state.borrow().reward_mission.clone();
        if mission.is_empty() {
            return Ok(false);
        }
        let before = self.controller_needs_rewards(vm)?;
        let (c, def) = (self.inner.o.controller, self.inner.mission.definition());
        self.run(vm, |vm| {
            call_by_name(vm, c, "MissionRewardsReceived", vec![Value::Object(Some(def))]).map(|_| ())
        });
        self.inner.mission.note(EffectKind::RewardReceived, &mission, "", "");
        self.inner.state.borrow_mut().reward_mission.clear();
        Ok(before && !self.controller_needs_rewards(vm)?)
    }

    // ---- the waypoint and the observers (the slice's) --------------------------------------------------------------

    /// The stock waypoint actor of the GoToRange objective (NATIVE_OBJECTIVE_TRIGGERS.md, UNVERIFIED): the placed
    /// `WillowWaypoint` of `package` at `path` runs its own script (`PostBeginPlay` at placement registers it as an
    /// observer). False when the package has no such actor.
    pub fn place_waypoint(&self, vm: &mut Vm, package: &PackageName, path: &str) -> Result<bool, MissionError> {
        let (_, store) = vm.defs.code_mut();
        let id = store.load(package).map_err(ow_defs::store::DefError::from)?;
        let Some(index) = store.get(id).find_export(path)?.filter(|&i| i > 0) else {
            return Ok(false);
        };
        let waypoint = vm
            .defs
            .load_export(&mut vm.space, id, index, PropertyPrefix::PlacedActor)?;
        self.inner.state.borrow_mut().waypoint = Some(waypoint);
        set(&mut vm.space, waypoint, "Role", Value::Byte(self.inner.o.authority))?;
        set(&mut vm.space, waypoint, "WorldInfo", Value::Object(Some(self.inner.o.world_info)))?;
        self.run(vm, |vm| call_by_name(vm, waypoint, "PostBeginPlay", Vec::new()).map(|_| ()));
        Ok(true)
    }

    pub fn has_waypoint(&self) -> bool {
        self.inner.state.borrow().waypoint.is_some()
    }

    /// Touch: the engine keeps the actor's `Touching` list and raises the script event once per overlapping pair (the
    /// host reports the overlap).
    pub fn touch(&self, vm: &mut Vm, who: Toucher) -> Result<(), MissionError> {
        let Some(waypoint) = self.inner.state.borrow().waypoint else {
            return Ok(());
        };
        let actor = self.pawn_for(vm, who)?;
        let mut touching = match get(&vm.space, waypoint, "Touching")? {
            Value::Array(a) => a.as_ref().clone(),
            _ => Vec::new(),
        };
        if touching.contains(&Value::Object(Some(actor))) {
            return Ok(());
        }
        touching.push(Value::Object(Some(actor)));
        set(&mut vm.space, waypoint, "Touching", Value::array(touching))?;
        self.run(vm, |vm| call_by_name(vm, waypoint, "Touch", vec![Value::Object(Some(actor))]).map(|_| ()));
        Ok(())
    }

    pub fn untouch(&self, vm: &mut Vm, who: Toucher) -> Result<(), MissionError> {
        let Some(waypoint) = self.inner.state.borrow().waypoint else {
            return Ok(());
        };
        let actor = self.pawn_for(vm, who)?;
        let Value::Array(items) = get(&vm.space, waypoint, "Touching")? else {
            return Ok(());
        };
        let mut items = items.as_ref().clone();
        let Some(at) = items.iter().position(|v| *v == Value::Object(Some(actor))) else {
            return Ok(());
        };
        items.remove(at);
        set(&mut vm.space, waypoint, "Touching", Value::array(items))?;
        let (class, state) = {
            let x = vm.space.heap.get(waypoint)?;
            (x.class, x.state)
        };
        if vm.find_method(class, "UnTouch", state)?.is_some() {
            self.run(vm, |vm| call_by_name(vm, waypoint, "UnTouch", vec![Value::Object(Some(actor))]).map(|_| ()));
        }
        Ok(())
    }

    /// The tracker's observer notifications reach the VM observers (a registered waypoint) as their
    /// `MissionReaction*` script events (arguments beyond the tracker are not passed).
    pub fn notify(&self, vm: &mut Vm, kind: Notification) {
        let name = match kind {
            Notification::LevelLoad => "MissionReactionLevelLoad",
            Notification::StatusChanged => "MissionReactionStatusChanged",
            Notification::ObjectiveSetChanged => "MissionReactionObjectiveSetChanged",
            Notification::ObjectiveUpdated => "MissionReactionObjectiveUpdated",
            Notification::ObjectiveCleared => "MissionReactionObjectiveCleared",
            Notification::ObjectiveComplete => "MissionReactionObjectiveComplete",
        };
        let observers = self.inner.state.borrow().observers.clone();
        let tracker = self.inner.o.tracker;
        for observer in observers {
            self.run(vm, |vm| call_by_name(vm, observer, name, vec![Value::Object(Some(tracker))]).map(|_| ()));
        }
    }

    /// Runs a script behavior's `ApplyBehaviorToContext` on the VM with `context` as its `ContextObject` (`None`: the
    /// C++ null context, which the Fire slice passes for the dummy's objective behavior; seam of lane W6-FIRE, decision
    /// 2026-11-16-38).
    pub fn apply_behavior(&self, vm: &mut Vm, behavior: ObjRef, context: Option<ObjRef>) {
        self.run(vm, |vm| {
            call_by_name(vm, behavior, "ApplyBehaviorToContext", vec![Value::Object(context)]).map(|_| ())
        });
    }

    // ---- the report ----------------------------------------------------------------------------------------------

    /// The bridge's report as `--mission-run` prints it: the controller's own record of the mission, every `ExpEarn`
    /// call, the stubs the script reached and the VM notes.
    pub fn report_json(&self, vm: &mut Vm) -> Result<String, MissionError> {
        let q = ow_core::fmt::json_quote;
        let earned = self
            .exp_earned()
            .iter()
            .map(|e| format!("{{\"amount\":{},\"source\":{},\"type\":{}}}", e.amount, e.source, e.exp_type))
            .collect::<Vec<_>>()
            .join(",");
        let stubs = self.stubs().iter().map(|l| q(l)).collect::<Vec<_>>().join(",");
        let notes = self.notes().iter().map(|l| q(l)).collect::<Vec<_>>().join(",");
        Ok(format!(
            "{{\"player_status\":{},\"needs_rewards\":{},\"player_level\":{},\"experience_pool\":{},\
             \"interaction_distance\":{},\"exp_earned\":[{}],\"stubs\":[{}],\"notes\":[{}]}}",
            self.controller_status(vm)?,
            self.controller_needs_rewards(vm)?,
            self.player_level(vm)?,
            ow_core::fmt::fmt_ostream(f64::from(self.experience_pool())),
            ow_core::fmt::fmt_ostream(f64::from(self.player_interaction_distance(vm)?)),
            earned,
            stubs,
            notes
        ))
    }
}

/// `director.<name>(out Missions)`: the missions the list script put in its out array.
fn director_list(vm: &mut Vm, director: ObjRef, name: &str) -> Result<Vec<Value>, MissionError> {
    let (class, state) = {
        let x = vm.space.heap.get(director)?;
        (x.class, x.state)
    };
    let f = vm
        .find_method(class, name, state)?
        .ok_or_else(|| data(format!("mission script bridge: Marcus has no {name}")))?;
    let result = vm.call(f, Some(director), vec![Value::array(Vec::new())])?;
    Ok(result
        .outs
        .first()
        .and_then(|v| v.elements())
        .map(<[Value]>::to_vec)
        .unwrap_or_default())
}

#[cfg(test)]
mod tests;
