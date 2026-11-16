//! Owner: lane W6-MISSION. The mission tracker: one installed `MissionDefinition` run by its own behavior provider (the
//! port of `src/mission.cpp`, `vm::MissionSystem`). AI-assisted (Claude Code).
//!
//! **What is data and what is native.** `MissionTracker`'s functions and the mission behaviors that matter here
//! (`Behavior_AdvanceObjectiveSet`, `Behavior_MissionRemoteEvent`, `Behavior_UpdateMissionObjective`,
//! `Behavior_ChangeRemoteBehaviorSequenceState`) are native in this build; there is no script to run. What the game
//! stores is data: a `MissionDefinition` with objective sets and objectives, and a `BehaviorProviderDefinition` whose
//! sequences hold named events, behaviors and packed output links. [`MissionSystem`] is the native executor for that
//! data: the objective sets and objectives are read from the package, the provider runs on the behavior kernel of
//! `ow-behavior` (W5-BEHAVIOR), and this file supplies the tracker rules and the four mission behaviors.
//!
//! **The rules** follow docs/verification/NATIVE_MISSION_DISPATCH.md section B, read from native code, and the bridge
//! note NATIVE_MISSION_SCRIPT_BRIDGE.md for the script hooks. All of them are **UNVERIFIED in the running game**,
//! except the lent weapon's place in the status routine, which was observed (FIRE_MISSION_GOLDEN_TRACE.md, lane L2):
//! - B1, objective updates: queued and applied in order (one raised while another is applied waits its turn); refused
//!   unless the mission is Active and the objective is in the active set and below its count; a bit-mask objective
//!   (`bRememberItemsWithinObjective`) ORs the bit in and a zero bit is refused; every accepted update fires the
//!   objective's event with link id 3, and the update that reaches `ObjectiveCount` completes it, runs the set
//!   evaluation and only then fires id 2.
//! - B2, set evaluation: a complete set fires id 5 once, then `bCanCompleteMission` -> ReadyToTurnIn, else
//!   `bAutoEnableNextSet` -> `NextSet`; a `bCanCompleteMission` set whose remaining objectives are all optional makes
//!   the mission ReadyToTurnIn.
//! - B3, set activation: only the active set's `NextSet`, or the `InitialObjectiveSet` while no set is active; refused
//!   when ReadyToTurnIn or Complete; fires the set's event with id 4, then the B2 evaluation when the set can complete
//!   the mission.
//! - B4, status: Active only from NotStarted, ReadyToTurnIn only from Active, Complete only from ReadyToTurnIn; each
//!   change fires `Default` with id 6 + the `EMissionStatus` number; `RunMissionCustomEvent` fires id 0 unless
//!   Complete. Which runs first at acceptance, the status event or the initial set, was not settled: status first.
//! - B6, availability: every dependency complete, and the `ObjectiveDependency` objective Complete (or Active, for an
//!   Active dependency).
//! - B7, link ids: 2 objective completed, 3 objective progress, 4 set activated, 5 set completed, 6 + status, 12
//!   kickoff, 13 kickoff dialog only, 14 turn-in.
//!
//! **Not modelled, as in the C++:** `RequiredObjectivesComplete` and `Failed`, the level-load replay (B5), blocking
//! sets, `ObjectiveCleared`. Four more the C++ also does not model answer `NotYet` in `ParityMode::Off` where the data
//! shows them, decided before anything changes, and keep the C++ behaviour in `ParityMode::Reference` (architecture R3;
//! decision 2026-11-15-34): a collection or branching objective set (refused at construction), the lent weapon's
//! region-stage condition (refused in the Active and Complete status routine), `bRepeatable` on a Complete mission
//! (`can_start`), and a plot-critical activation that finds a pending kickoff record.
//!
//! **The dialog** (lane W5-DIALOG, `crate::dialog`, decision 2026-11-15-32). As the C++ tracker owns its
//! `DialogSystem`, each [`MissionSystem`] owns one `Rc<RefCell<DialogSystem>>` over the mission's package
//! ([`MissionSystem::dialog`]): `dialog::handler` runs `GearboxFramework.Behavior_TriggerDialogEvent` on the mission's
//! own provider, the provider's time hook advances the dialog clock to the kernel's time (the frame's time without a
//! provider), the status routine sets the tracked mission's priority floor (`MissionDialogGroup`, `bPlotCritical`)
//! while Active or ReadyToTurnIn, and every line the dialog chooses is a `Dialog` effect (`a` tag, `b` group, `c` talker,
//! `detail` `act=..;ak=..;talker=echo|pawn;outcome=..;line=<id>`). The script bridge's dialog-component native uses
//! the same `Rc`, so the dialog state is not split in two (decision 2026-11-15-34, addendum).
//!
//! **Re-entrancy.** The C++ tracker is re-entrant: a behavior handler calls back into the tracker, which fires more
//! events on the same provider; a script hook calls natives that read or change the tracker. Here the state is behind
//! a `RefCell` borrowed only for short scopes (never across a provider call, a hook or a script call), and the
//! provider lives in a slot it is taken out of while it runs. A handler lends the running provider back to the slot
//! for as long as it calls out ([`lend`]), so whatever the handler reaches (the tracker, a hook, a native reached
//! from script) finds it there and nests its call on it, depth-first, as the C++ does. [`PROVIDER_BUSY`] is the
//! refusal for a call that finds the slot empty, which no path of this crate produces.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::{Rc, Weak};

use ow_behavior::kernel::{BehaviorId, Provider};
use ow_behavior::provider::{
    BehaviorError, class_path_of, enum_names, object_name, provider_path_leaf, stock_path,
};
use ow_behavior::registry::Handler;
use ow_core::{DetMap, DetSet, NotYet, PackageName, ParityMode};
use ow_defs::store::DefError;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::prefix::PropertyPrefix;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use crate::dialog::{self, DialogSystem, Line};

#[cfg(test)]
mod tests;

/// The record of the tracker rules.
pub const RECORD: &str = "NATIVE_MISSION_DISPATCH.md section B";

/// The record of the bridge's rules the tracker applies (availability, the lent weapon).
pub const BRIDGE_RECORD: &str = "NATIVE_MISSION_SCRIPT_BRIDGE.md";

/// The `NotYet` text of a nested event the Rust tracker cannot reach (module documentation, "Re-entrancy").
pub const PROVIDER_BUSY: &str =
    "a mission provider call while the provider is running and not lent to its slot (the C++ nests it)";

/// The class every mission definition is (exactly, as the C++ compares it).
pub const MISSION_CLASS: &str = "WillowGame.MissionDefinition";

/// The only objective-set class the tracker activates.
pub const OBJECTIVE_SET_CLASS: &str = "WillowGame.MissionObjectiveSetDefinition";

/// The errors are the behavior crate's: the VM, the store, a package read, or a C++ `RuntimeError` text.
pub type MissionError = BehaviorError;

/// Mission event link ids (B7, UNVERIFIED).
pub mod link {
    pub const CUSTOM: i32 = 0;
    pub const OBJECTIVE_COMPLETED: i32 = 2;
    pub const OBJECTIVE_PROGRESS: i32 = 3;
    pub const SET_ACTIVATED: i32 = 4;
    pub const SET_COMPLETED: i32 = 5;
    pub const STATUS_BASE: i32 = 6;
    pub const KICKOFF: i32 = 12;
    pub const KICKOFF_DIALOG_ONLY: i32 = 13;
    pub const TURN_IN: i32 = 14;
}

/// The four tracker states the C++ models (its `int(status)` is [`Status::index`]).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Status {
    NotStarted,
    Active,
    ReadyToTurnIn,
    Complete,
}

impl Status {
    /// The C++ enum value `int(status)` (0..=3), what `--mission-run` prints and the save keeps.
    pub fn index(self) -> u8 {
        match self {
            Status::NotStarted => 0,
            Status::Active => 1,
            Status::ReadyToTurnIn => 2,
            Status::Complete => 3,
        }
    }

    /// The `EMissionStatus` number the script sees (NotStarted 0, Active 1, ReadyToTurnIn 3, Complete 4;
    /// RequiredObjectivesComplete 2 and Failed 5 are not modelled).
    pub fn number(self) -> u8 {
        match self {
            Status::NotStarted => 0,
            Status::Active => 1,
            Status::ReadyToTurnIn => 3,
            Status::Complete => 4,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Status::NotStarted => "NotStarted",
            Status::Active => "Active",
            Status::ReadyToTurnIn => "ReadyToTurnIn",
            Status::Complete => "Complete",
        }
    }

    fn from_index(i: u8) -> Option<Status> {
        Some(match i {
            0 => Status::NotStarted,
            1 => Status::Active,
            2 => Status::ReadyToTurnIn,
            3 => Status::Complete,
            _ => return None,
        })
    }
}

/// What the host must do or show, in order (the C++ `Effect::Kind`). Nothing here has been executed by the tracker.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectKind {
    /// `a` = the event name.
    RemoteEvent,
    /// A line the mission's dialog chose (`Behavior_TriggerDialogEvent` or the director's `TriggerEvent`): `a` tag, `b`
    /// group, `c` talker, `detail` the C++ text.
    Dialog,
    /// `a` = provider path leaf, `b` = sequence, `c` = action (`CHANGE_Enable` / `Disable` / `Toggle`).
    SetSequence,
    /// `a` = the set's name.
    ObjectiveSetActive,
    /// `a` = the objective's name.
    ObjectiveComplete,
    /// `a` = the status name.
    StatusChanged,
    /// `a` = the XP attribute path of the reward.
    Reward,
    /// `a` = the `MissionWeaponBalanceDefinition` path.
    MissionWeaponGranted,
    MissionWeaponRemoved,
    /// `a` = the objective's name, `b` = its new count.
    ObjectiveUpdated,
    /// The tag raised on the director's dialog component by `PlayMissionTurnedInDialog` (`a` tag, `b` speaker, `c`
    /// the other object's class).
    TurnInDialog,
    /// The tag raised when the player's use of the director ends (same fields).
    DismissalDialog,
    /// The director's mission screen closed (`a` = the director's path).
    InterfaceClosed,
    /// The reward page waits for the player (`a` = the mission's path).
    RewardPage,
    /// The player confirmed it (`a` = the mission's path).
    RewardReceived,
}

impl EffectKind {
    /// The `--mission-run` JSON name.
    pub fn json_name(self) -> &'static str {
        match self {
            EffectKind::RemoteEvent => "remote_event",
            EffectKind::Dialog => "dialog",
            EffectKind::SetSequence => "set_sequence",
            EffectKind::ObjectiveSetActive => "objective_set_active",
            EffectKind::ObjectiveComplete => "objective_complete",
            EffectKind::StatusChanged => "status",
            EffectKind::Reward => "reward",
            EffectKind::MissionWeaponGranted => "mission_weapon_granted",
            EffectKind::MissionWeaponRemoved => "mission_weapon_removed",
            EffectKind::ObjectiveUpdated => "objective_updated",
            EffectKind::TurnInDialog => "turn_in_dialog",
            EffectKind::DismissalDialog => "dismissal_dialog",
            EffectKind::InterfaceClosed => "interface_closed",
            EffectKind::RewardPage => "reward_page",
            EffectKind::RewardReceived => "reward_received",
        }
    }
}

/// One effect: its kind, three texts, the tracker's time when it was raised, and a detail text (dialog only).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Effect {
    pub kind: EffectKind,
    pub a: String,
    pub b: String,
    pub c: String,
    pub time: f64,
    pub detail: String,
}

/// The active set's objectives as the HUD's mission tracker lists them (HUD_PARITY.md, "Mission tracker",
/// UNVERIFIED): the objective's name, its `ProgressMessage`, whether it is complete and optional, and its
/// `ObjectiveCount`.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackerLine {
    pub name: String,
    pub message: String,
    pub complete: bool,
    pub optional: bool,
    pub count: i32,
}

/// The tracker's observer notifications (NATIVE_BEHAVIOR_POPULATION.md section A, UNVERIFIED). `LevelLoad` is raised
/// by an observer's owner when it registers; `ObjectiveCleared` is not modelled.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notification {
    LevelLoad,
    StatusChanged,
    ObjectiveSetChanged,
    ObjectiveUpdated,
    ObjectiveCleared,
    ObjectiveComplete,
}

/// Called with the new `EMissionStatus` number inside the status routine (after the state changed, before the
/// observers and the `Default` event). The script bridge runs `UpdateMissionStatus` here.
pub type StatusHook = Rc<dyn Fn(&mut Vm, u8)>;
/// Called when the tracker tick consumes the pending kickoff record, before the kickoff event.
pub type KickoffHook = Rc<dyn Fn(&mut Vm)>;
/// Called with each observer notification, after the tracker changed its own state.
pub type NotifyHook = Rc<dyn Fn(&mut Vm, Notification)>;
/// Called after an applied objective update (after the observers, before the objective's id-3 event) with the
/// objective's path and the bit.
pub type ObjectiveHook = Rc<dyn Fn(&mut Vm, &str, i32)>;

#[derive(Default, Clone)]
struct Hooks {
    status: Option<StatusHook>,
    kickoff: Option<KickoffHook>,
    notify: Option<NotifyHook>,
    objective: Option<ObjectiveHook>,
}

#[derive(Debug, Clone)]
struct ObjectiveSet {
    path: String,
    name: String,
    next: String,
    cls: String,
    can_complete: bool,
    auto_next: bool,
    objectives: Vec<String>,
    objective_paths: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct Objective {
    path: String,
    message: String,
    count: i32,
    mask: bool,
    optional: bool,
}

/// The mission's data, read once.
#[derive(Debug, Clone, Default)]
struct Data {
    path: String,
    name: String,
    giver: String,
    turn_in: String,
    weapon: String,
    description: String,
    xp_attribute: String,
    summary: String,
    turn_in_description: String,
    plot_critical: bool,
    /// `MissionDefinition.bRepeatable`.
    repeatable: bool,
    /// `MissionDefinition.MissionDialogGroup`'s path ("" when none): the tracked mission's dialog floor.
    dialog_group: String,
    dependencies: Vec<String>,
    sets: Vec<ObjectiveSet>,
    /// By objective name, in key order (the C++ `std::map`).
    objectives: DetMap<String, Objective>,
    initial_set: String,
    weapon_objective: String,
    dependency_objective: String,
    activate_initial_set: bool,
    dependency_active: bool,
}

impl Data {
    fn find_set(&self, path: &str) -> Option<&ObjectiveSet> {
        self.sets.iter().find(|s| s.path == path)
    }
}

/// The mission's run state.
#[derive(Debug, Clone, Default)]
struct State {
    status: Option<Status>,
    active_set: String,
    progress: DetMap<String, i32>,
    completed_objectives: DetSet<String>,
    completed_sets: DetSet<String>,
    updates: VecDeque<(String, i32)>,
    draining: bool,
    kickoff_pending: bool,
    kickoff_from_activation: bool,
    heard_kickoff: bool,
    effects: Vec<Effect>,
    now: f64,
    errors: Vec<String>,
}

struct Inner {
    data: Data,
    definition: ObjRef,
    state: RefCell<State>,
    /// The provider, out of its slot while it runs.
    provider: RefCell<Option<Provider>>,
    has_provider: bool,
    /// The provider was lost by a failed lend ([`lend`]): every later call is refused.
    lost: Cell<bool>,
    hooks: RefCell<Hooks>,
    /// The mission's dialog, shared with the script bridge's dialog-component native (module documentation).
    dialog: Rc<RefCell<DialogSystem>>,
}

/// One mission and its provider (module documentation). Cheap to clone: every clone is the same mission.
#[derive(Clone)]
pub struct MissionSystem {
    inner: Rc<Inner>,
}

impl std::fmt::Debug for MissionSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MissionSystem")
            .field("path", &self.inner.data.path)
            .field("status", &self.status())
            .finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Reading the data the C++ way
// ---------------------------------------------------------------------------------------------------------------

/// The value of property `name` of `o` (element 0), or `None` when its class declares no such property (the C++
/// `Runtime::property` returning null).
pub(crate) fn prop(space: &ObjectSpace, o: ObjRef, name: &str) -> Result<Option<Value>, MissionError> {
    let class = space.heap.get(o)?.class;
    match space.prop(class, name) {
        Some(p) => Ok(Some(space.read(o, PropAddr::scalar(p))?.clone())),
        None => Ok(None),
    }
}

/// Field `name` of a struct value, or `None` (the C++ `Value::field`). For a static-array field this is its first
/// entry; [`field_value`] gives the whole array.
pub(crate) fn field<'v>(space: &ObjectSpace, v: &'v Value, name: &str) -> Option<&'v Value> {
    let s = v.as_struct()?;
    let id = space.names.find(name)?;
    let (entry, _) = space.classes.struct_layout(s.ty)?.field(id)?;
    s.fields.get(entry as usize)
}

/// Field `name` of a struct value as the C++ field holds it: a static-array field (`array_dim` above 1) is its
/// entries as an array value, any other field its value.
pub(crate) fn field_value(space: &ObjectSpace, v: &Value, name: &str) -> Option<Value> {
    let s = v.as_struct()?;
    let id = space.names.find(name)?;
    let (entry, decl) = space.classes.struct_layout(s.ty)?.field(id)?;
    let first = entry as usize;
    match usize::try_from(decl.array_dim) {
        Ok(dim) if dim > 1 => Some(Value::array(s.fields.get(first..first.checked_add(dim)?)?.to_vec())),
        _ => s.fields.get(first).cloned(),
    }
}

/// The C++ `Value::integer()`: ints, bytes and bools as they are, floats truncated to 32 bits (N5), else 0.
pub(crate) fn integer(v: &Value) -> i64 {
    match v {
        Value::Int(i) => i64::from(*i),
        Value::Byte(b) => i64::from(*b),
        Value::Bool(b) => i64::from(*b),
        // The C++ casts the float to int64_t; trunc_i64_wrap_i32 is that cast kept to 32 bits, which every field read
        // here (counts, enum bytes, flags) fits.
        Value::Float(f) => i64::from(ow_core::num::trunc_i64_wrap_i32(f64::from(*f))),
        _ => 0,
    }
}

/// [`integer`] as the 32-bit int every value it reads already is (the C++ `int(v.integer())`).
pub(crate) fn integer32(v: &Value) -> i32 {
    match v {
        Value::Int(i) => *i,
        Value::Byte(b) => i32::from(*b),
        Value::Bool(b) => i32::from(*b),
        Value::Float(f) => ow_core::num::trunc_i64_wrap_i32(f64::from(*f)),
        _ => 0,
    }
}

/// The C++ `->s` of a string or name value (anything else: empty).
pub(crate) fn text(space: &ObjectSpace, v: Option<&Value>) -> String {
    match v {
        Some(Value::Name(n)) => space.names.text(*n).into_owned(),
        Some(Value::Str(s)) => s.to_string(),
        _ => String::new(),
    }
}

/// The stock path of an object value (the C++ `refPath`): `""` for None, a non-object value, or an object read from
/// no package.
pub(crate) fn ref_path(vm: &Vm, v: Option<&Value>) -> Result<String, MissionError> {
    match v {
        Some(Value::Object(Some(o))) => stock_path(vm, *o),
        _ => Ok(String::new()),
    }
}

/// The object an object value names, loaded when it is a definition of the store (the C++ `load`, which
/// instantiates an export): `None` for None, a non-object value or an object made at run time.
pub(crate) fn load_ref(vm: &mut Vm, v: Option<&Value>) -> Result<Option<ObjRef>, MissionError> {
    let Some(Value::Object(Some(o))) = v else {
        return Ok(None);
    };
    match vm.space.heap.get(*o)?.origin {
        Origin::Export { .. } | Origin::Resource { .. } => {
            let done = vm.defs.ensure_loaded(&mut vm.space, *o);
            let log = vm.defs.take_log();
            vm.log.extend(log);
            done.map_err(VmError::Def)?;
            Ok(Some(*o))
        }
        _ => Ok(None),
    }
}

/// A flag with its class default; `fallback` only when the class does not declare it.
fn flag(vm: &Vm, o: ObjRef, name: &str, fallback: bool) -> Result<bool, MissionError> {
    Ok(match prop(&vm.space, o, name)? {
        Some(v) => v.truth(&vm.space.names),
        None => fallback,
    })
}

fn text_prop(vm: &Vm, o: ObjRef, name: &str) -> Result<String, MissionError> {
    Ok(text(&vm.space, prop(&vm.space, o, name)?.as_ref()))
}

fn data_error(text: String) -> MissionError {
    BehaviorError::Data(text)
}

/// Export `path` of the store's copy of package `package`, loaded with the ordinary prefix (the C++
/// `instantiateExport(package, findExport(package, path), 4)`); `None` when the package has no such export.
pub fn find_object(vm: &mut Vm, package: &PackageName, path: &str) -> Result<Option<ObjRef>, MissionError> {
    let (_, store) = vm.defs.code_mut();
    let id = store.load(package).map_err(DefError::from)?;
    let Some(index) = store.get(id).find_export(path)?.filter(|&i| i > 0) else {
        return Ok(None);
    };
    let o = vm
        .defs
        .load_export(&mut vm.space, id, index, PropertyPrefix::Ordinary)?;
    let log = vm.defs.take_log();
    vm.log.extend(log);
    Ok(Some(o))
}

/// The number of set bits (a bit-mask objective's progress).
fn bits(mask: i32) -> i32 {
    // count_ones of a u32 is at most 32, which fits.
    mask.count_ones() as i32
}

// ---------------------------------------------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------------------------------------------

impl MissionSystem {
    /// Reads mission `path` of package `package` (C++ `MissionSystem(runtime, package, path)`): its sets, objectives,
    /// dependencies, reward attribute and weapon, and starts its provider with the mission behaviors bound. Errors
    /// carry the C++ texts (`mission not found: <path>`, `not a MissionDefinition: <path>`, `unresolved objective set
    /// in <path>`, `unresolved objective in <set path>`).
    pub fn new(vm: &mut Vm, package: &PackageName, path: &str) -> Result<MissionSystem, MissionError> {
        let mission = find_object(vm, package, path)?
            .ok_or_else(|| data_error(format!("mission not found: {path}")))?;
        if class_path_of(vm, mission)? != MISSION_CLASS {
            return Err(data_error(format!("not a MissionDefinition: {path}")));
        }
        let mut d = Data {
            path: path.to_string(),
            name: text_prop(vm, mission, "MissionName")?,
            description: text_prop(vm, mission, "MissionDescription")?,
            summary: text_prop(vm, mission, "MissionSummary")?,
            turn_in_description: text_prop(vm, mission, "TurnInDescription")?,
            giver: text_prop(vm, mission, "MissionGiver")?,
            turn_in: text_prop(vm, mission, "MissionTurnInLocation")?,
            ..Data::default()
        };
        let weapon = prop(&vm.space, mission, "MissionWeapon")?;
        d.weapon = ref_path(vm, weapon.as_ref())?;
        d.initial_set = ref_path(vm, prop(&vm.space, mission, "InitialObjectiveSet")?.as_ref())?;
        d.activate_initial_set = flag(vm, mission, "bActivateInitialObjectiveSet", true)?;
        d.plot_critical = flag(vm, mission, "bPlotCritical", false)?;
        d.repeatable = flag(vm, mission, "bRepeatable", false)?;
        d.dialog_group = ref_path(vm, prop(&vm.space, mission, "MissionDialogGroup")?.as_ref())?;
        // ObjectiveDependency { Objective, Status: EODS_Complete 0 / EODS_Active 1 } (B6).
        if let Some(dep) = prop(&vm.space, mission, "ObjectiveDependency")?
            && dep.as_struct().is_some()
        {
            d.dependency_objective = ref_path(vm, field(&vm.space, &dep, "Objective"))?;
            d.dependency_active = field(&vm.space, &dep, "Status").is_some_and(|s| integer(s) == 1);
        }
        // The lent mission weapon is tied to one objective by its definition.
        if let Some(definition) = load_ref(vm, weapon.as_ref())? {
            d.weapon_objective = ref_path(vm, prop(&vm.space, definition, "MissionObjective")?.as_ref())?;
        }
        if let Some(Value::Array(list)) = prop(&vm.space, mission, "Dependencies")? {
            for dep in list.iter() {
                d.dependencies.push(ref_path(vm, Some(dep))?);
            }
        }
        if let Some(reward) = prop(&vm.space, mission, "Reward")?
            && let Some(xp) = field(&vm.space, &reward, "ExperienceRewardPercentage")
            && let Some(attribute) = field(&vm.space, xp, "BaseValueAttribute")
        {
            d.xp_attribute = ref_path(vm, Some(attribute))?;
        }
        if let Some(Value::Array(list)) = prop(&vm.space, mission, "ObjectiveSetDefs")? {
            for reference in list.iter() {
                let object = load_ref(vm, Some(reference))?
                    .ok_or_else(|| data_error(format!("unresolved objective set in {path}")))?;
                read_set(vm, &mut d, reference, object)?;
            }
        }
        // A collection or branching objective set is not modelled (module documentation): in Off mode the mission is
        // refused before any state exists; Reference keeps the C++, which reports the set when it is reached.
        if vm.parity == ParityMode::Off
            && let Some(set) = d.sets.iter().find(|s| s.cls != OBJECTIVE_SET_CLASS)
        {
            return Err(NotYet::not_ported(
                format!("an objective set of class {} ({}): collection and branching sets", set.cls, set.name),
                RECORD,
            )
            .into());
        }
        let provider = match prop(&vm.space, mission, "BehaviorProvider")? {
            Some(Value::Object(Some(o))) if is_package_object(&vm.space, o)? => Some(Provider::new(vm, o)?),
            _ => None,
        };
        // Action is an ITargetable.EChangeStatus (CHANGE_Toggle, CHANGE_Enable, CHANGE_Disable).
        let actions = if provider.is_some() {
            enum_names(vm, "Engine", "ITargetable.EChangeStatus")?
        } else {
            Vec::new()
        };
        let has_provider = provider.is_some();
        let package = match vm.space.heap.get(mission)?.origin {
            Origin::Export { package, .. } | Origin::Resource { package, .. } => Some(package),
            _ => None,
        };
        let system = MissionSystem {
            inner: Rc::new(Inner {
                data: d,
                definition: mission,
                state: RefCell::new(State::default()),
                provider: RefCell::new(provider),
                has_provider,
                lost: Cell::new(false),
                hooks: RefCell::new(Hooks::default()),
                dialog: Rc::new(RefCell::new(DialogSystem::new(package))),
            }),
        };
        system.bind_dialog_sink();
        system.bind_behaviors(actions);
        Ok(system)
    }

    /// Every line the dialog chooses is a `Dialog` effect (the C++ `dialog_.onLine`).
    fn bind_dialog_sink(&self) {
        let w = Rc::downgrade(&self.inner);
        self.inner.dialog.borrow_mut().on_line = Some(Box::new(move |line: &Line, outcome: &str| {
            if let Some(inner) = w.upgrade() {
                MissionSystem { inner }.emit_dialog(line, outcome);
            }
        }));
    }

    /// The mission behaviors on the provider (C++ `provider.handle(...)`), the dialog behavior over the mission's own
    /// dialog, and the dialog clock on the kernel's time.
    fn bind_behaviors(&self, actions: Vec<String>) {
        let mut slot = self.inner.provider.borrow_mut();
        let Some(p) = slot.as_mut() else {
            return;
        };
        p.handle(dialog::BEHAVIOR_CLASS, dialog::handler(self.inner.dialog.clone()));
        {
            // The C++ `provider.onTime = dialog_.advanceTo(time)`. The hook cannot fail: a dialog already in use (a
            // time step inside a dialog call, which no path makes) is an error line, not a silent skip.
            let w = Rc::downgrade(&self.inner);
            p.on_time = Some(Rc::new(move |_vm: &mut Vm, time: f64| {
                let Some(inner) = w.upgrade() else {
                    return;
                };
                match inner.dialog.try_borrow_mut() {
                    Ok(mut d) => d.advance_to(time),
                    Err(_) => inner
                        .state
                        .borrow_mut()
                        .errors
                        .push("dialog: the dialog clock was stepped while the dialog system was in use".to_string()),
                };
            }));
        }
        let weak = Rc::downgrade(&self.inner);
        let me = move |w: &Weak<Inner>| {
            w.upgrade().map(|inner| MissionSystem { inner }).ok_or_else(|| {
                data_error("the mission tracker of this provider is gone".to_string())
            })
        };
        {
            let (w, me) = (weak.clone(), me);
            p.handle(
                "WillowGame.Behavior_AdvanceObjectiveSet",
                Handler::run(move |vm, p, id, _| {
                    let o = p.behavior(id).object;
                    let target = ref_path(vm, prop(&vm.space, o, "ObjectiveSetToAdvanceTo")?.as_ref())?;
                    let me = me(&w)?;
                    lend(&me.inner.provider, &me.inner.lost, p, || me.request_advance(vm, &target))?;
                    Ok(Vec::new())
                }),
            );
        }
        {
            let (w, me) = (weak.clone(), me);
            p.handle(
                "WillowGame.Behavior_MissionRemoteEvent",
                Handler::run(move |vm, p, id, _| {
                    let name = text_prop(vm, p.behavior(id).object, "EventName")?;
                    me(&w)?.emit(EffectKind::RemoteEvent, name, String::new(), String::new());
                    Ok(Vec::new())
                }),
            );
        }
        {
            let (w, me) = (weak.clone(), me);
            p.handle(
                "WillowGame.Behavior_UpdateMissionObjective",
                Handler::run(move |vm, p, id, _| {
                    let o = p.behavior(id).object;
                    let path = ref_path(vm, prop(&vm.space, o, "MissionObjective")?.as_ref())?;
                    let me = me(&w)?;
                    lend(&me.inner.provider, &me.inner.lost, p, || me.update_by_path(vm, &path, 0))?;
                    Ok(Vec::new())
                }),
            );
        }
        {
            let (w, me) = (weak, me);
            p.handle(
                "GearboxFramework.Behavior_ChangeRemoteBehaviorSequenceState",
                Handler::run(move |vm, p, id, _| {
                    change_remote(vm, p, id, &actions, &me(&w)?)?;
                    Ok(Vec::new())
                }),
            );
        }
    }
}

/// `Behavior_ChangeRemoteBehaviorSequenceState`: a `SetSequence` effect naming the provider path's leaf, the sequence
/// and the action (class default `CHANGE_Enable`; an index outside the enum gives `""`).
fn change_remote(
    vm: &mut Vm,
    p: &mut Provider,
    id: BehaviorId,
    actions: &[String],
    me: &MissionSystem,
) -> Result<(), MissionError> {
    let o = p.behavior(id).object;
    let path = prop(&vm.space, o, "ProviderDefinitionPathName")?;
    let components = path.as_ref().and_then(|v| field_value(&vm.space, v, "PathComponentNames"));
    let leaf = provider_path_leaf(&vm.space, components.as_ref());
    let sequence = text_prop(vm, o, "SequenceName")?;
    let index = prop(&vm.space, o, "Action")?.map_or(-1, |v| integer(&v));
    let action = usize::try_from(index)
        .ok()
        .and_then(|i| actions.get(i))
        .cloned()
        .unwrap_or_default();
    me.emit(EffectKind::SetSequence, leaf, sequence, action);
    Ok(())
}

/// Whether `o` was read from a package (the C++ `o->resourcePackage` test).
fn is_package_object(space: &ObjectSpace, o: ObjRef) -> Result<bool, MissionError> {
    Ok(matches!(
        space.heap.get(o)?.origin,
        Origin::Export { .. } | Origin::Resource { .. }
    ))
}

/// One objective set and its objectives (the C++ constructor's loop body).
fn read_set(vm: &mut Vm, d: &mut Data, reference: &Value, object: ObjRef) -> Result<(), MissionError> {
    let mut set = ObjectiveSet {
        path: ref_path(vm, Some(reference))?,
        name: text_prop(vm, object, "ObjectiveSetName")?,
        next: ref_path(vm, prop(&vm.space, object, "NextSet")?.as_ref())?,
        cls: class_path_of(vm, object)?,
        can_complete: flag(vm, object, "bCanCompleteMission", true)?,
        auto_next: flag(vm, object, "bAutoEnableNextSet", false)?,
        objectives: Vec::new(),
        objective_paths: Vec::new(),
    };
    if set.name.is_empty() {
        set.name = object_name(vm, object)?;
    }
    if let Some(Value::Array(list)) = prop(&vm.space, object, "ObjectiveDefinitions")? {
        for reference in list.iter() {
            let objective = load_ref(vm, Some(reference))?
                .ok_or_else(|| data_error(format!("unresolved objective in {}", set.path)))?;
            let mut name = text_prop(vm, objective, "ObjectiveName")?;
            if name.is_empty() {
                name = object_name(vm, objective)?;
            }
            let path = ref_path(vm, Some(reference))?;
            set.objectives.push(name.clone());
            set.objective_paths.push(path.clone());
            let count = match prop(&vm.space, objective, "ObjectiveCount")? {
                // The C++ `int(count->integer())`.
                Some(v) => integer32(&v),
                None => 1,
            };
            let parsed = Objective {
                path,
                message: text_prop(vm, objective, "ProgressMessage")?,
                count,
                mask: flag(vm, objective, "bRememberItemsWithinObjective", false)?,
                optional: flag(vm, objective, "bObjectiveIsOptional", false)?,
            };
            d.objectives.insert(name, parsed);
        }
    }
    d.sets.push(set);
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Reading the state
// ---------------------------------------------------------------------------------------------------------------

impl MissionSystem {
    fn data(&self) -> &Data {
        &self.inner.data
    }

    /// The `MissionDefinition` object the script functions take as their `Mission` argument.
    pub fn definition(&self) -> ObjRef {
        self.inner.definition
    }

    pub fn status(&self) -> Status {
        self.inner.state.borrow().status.unwrap_or(Status::NotStarted)
    }

    /// The `EMissionStatus` number the script sees.
    pub fn status_number(&self) -> u8 {
        self.status().number()
    }

    /// The active set's path (`""` when none).
    pub fn active_set(&self) -> String {
        self.inner.state.borrow().active_set.clone()
    }

    pub fn path(&self) -> &str {
        &self.data().path
    }
    pub fn name(&self) -> &str {
        &self.data().name
    }
    pub fn giver(&self) -> &str {
        &self.data().giver
    }
    pub fn turn_in(&self) -> &str {
        &self.data().turn_in
    }
    pub fn weapon_definition(&self) -> &str {
        &self.data().weapon
    }
    pub fn description(&self) -> &str {
        &self.data().description
    }
    /// `MissionSummary` as stored (`""` when absent).
    pub fn summary(&self) -> &str {
        &self.data().summary
    }
    /// `TurnInDescription` as stored (`""` when absent).
    pub fn turn_in_description(&self) -> &str {
        &self.data().turn_in_description
    }
    pub fn dependencies(&self) -> &[String] {
        &self.data().dependencies
    }
    /// `MissionDefinition.bPlotCritical` (story vs optional mission).
    pub fn plot_critical(&self) -> bool {
        self.data().plot_critical
    }
    pub fn has_provider(&self) -> bool {
        self.inner.has_provider
    }

    pub fn has_optional_objective(&self) -> bool {
        self.data().objectives.values().any(|o| o.optional)
    }

    /// The grant condition of the bridge note: a `MissionWeapon` is set and the weapon's own mission objective is one
    /// of this mission's objectives (its region-stage condition is not applied, UNVERIFIED).
    pub fn lends_weapon(&self) -> bool {
        lends(self.data())
    }

    pub fn objective_complete(&self, name: &str) -> bool {
        self.inner.state.borrow().completed_objectives.contains(name)
    }

    /// The count (distinct bits for a bit-mask objective; UNVERIFIED: `TranslateObjectiveCount` of a mask is taken as
    /// its number of set bits).
    pub fn objective_progress(&self, name: &str) -> i32 {
        progress_of(self.data(), &self.inner.state.borrow(), name)
    }

    pub fn kickoff_pending(&self) -> bool {
        self.inner.state.borrow().kickoff_pending
    }

    /// Whether this set (by path) is the active set (collection sets and sub-sets are not modelled).
    pub fn set_active(&self, path: &str) -> bool {
        !path.is_empty() && self.inner.state.borrow().active_set == path
    }

    /// Every dependency is in `completed` and the `ObjectiveDependency`, if any, holds: `objective_states` maps
    /// objective paths of other missions to `"Complete"` / `"Active"` (B6, UNVERIFIED).
    pub fn available(&self, completed: &DetSet<String>, objective_states: &DetMap<String, String>) -> bool {
        let d = self.data();
        if d.dependencies.iter().any(|dep| !completed.contains(dep)) {
            return false;
        }
        if d.dependency_objective.is_empty() {
            return true;
        }
        match objective_states.get(&d.dependency_objective) {
            None => false,
            Some(s) => s == "Complete" || (d.dependency_active && s == "Active"),
        }
    }

    /// `CanStartMission` (NATIVE_MISSION_SCRIPT_BRIDGE.md, "Availability queries", UNVERIFIED): NotStarted and the
    /// dependencies met. Failed and the blocked-mission test are not modelled. A Complete mission with `bRepeatable`
    /// (restarting it is not read): `NotYet` in `ParityMode::Off`; Reference answers the C++ false. The mode is the
    /// VM's (`vm.parity`, the one source, architecture R3).
    pub fn can_start(&self, completed: &DetSet<String>, vm: &Vm) -> Result<bool, MissionError> {
        if vm.parity == ParityMode::Off && self.data().repeatable && self.status() == Status::Complete {
            return Err(NotYet::not_read("CanStartMission of a Complete mission with bRepeatable", BRIDGE_RECORD).into());
        }
        Ok(self.status() == Status::NotStarted && self.available(completed, &DetMap::new()))
    }

    /// `CanEndMission`: ReadyToTurnIn (RequiredObjectivesComplete is not modelled).
    pub fn can_end(&self) -> bool {
        self.status() == Status::ReadyToTurnIn
    }

    /// `GetCompletedBranch` (NATIVE_USE_INTERACTION.md, UNVERIFIED): 0 unless ReadyToTurnIn or Complete; then
    /// `NextSet` is followed from the initial set to the last set; for the objective-set class, 2 when any of its
    /// objectives is not complete, 1 when all are; 0 with no initial set or a last set of another class.
    pub fn completed_branch(&self) -> u8 {
        let status = self.status();
        if status != Status::ReadyToTurnIn && status != Status::Complete {
            return 0;
        }
        let d = self.data();
        let mut last = None;
        let mut seen = DetSet::new();
        let mut path = d.initial_set.clone();
        while !path.is_empty() && seen.insert(path.clone()) {
            let Some(set) = d.find_set(&path) else {
                break;
            };
            last = Some(set);
            path = set.next.clone();
        }
        let Some(last) = last.filter(|s| s.cls == OBJECTIVE_SET_CLASS) else {
            return 0;
        };
        let state = self.inner.state.borrow();
        if last.objectives.iter().any(|o| !state.completed_objectives.contains(o)) {
            2
        } else {
            1
        }
    }

    /// The tracker's own classification of an objective path of this mission (NATIVE_BEHAVIOR_POPULATION.md section
    /// B, UNVERIFIED): `"Complete"` when the mission is Active, ReadyToTurnIn or Complete and the progress equals the
    /// count; else `"Active"` when the mission is Active, the objective is in the active set and below its count;
    /// else `"NotStarted"`. `""` when the path is not one of the mission's objectives.
    pub fn objective_state(&self, path: &str) -> &'static str {
        let d = self.data();
        let state = self.inner.state.borrow();
        let (mut known, mut in_active, mut name) = (false, false, "");
        for set in &d.sets {
            for (i, p) in set.objective_paths.iter().enumerate() {
                if p != path {
                    continue;
                }
                known = true;
                name = &set.objectives[i];
                if set.path == state.active_set {
                    in_active = true;
                }
            }
        }
        if !known {
            return "";
        }
        let Some(objective) = d.objectives.get(name) else {
            return "";
        };
        let progress = progress_of(d, &state, name);
        let status = state.status.unwrap_or(Status::NotStarted);
        if matches!(status, Status::Active | Status::ReadyToTurnIn | Status::Complete) && progress == objective.count {
            return "Complete";
        }
        if status == Status::Active && in_active && progress < objective.count {
            return "Active";
        }
        "NotStarted"
    }

    /// The active set's objectives as the HUD lists them (empty with no active set).
    pub fn tracker_lines(&self) -> Vec<TrackerLine> {
        let d = self.data();
        let state = self.inner.state.borrow();
        let Some(active) = d.find_set(&state.active_set) else {
            return Vec::new();
        };
        active
            .objectives
            .iter()
            .map(|name| {
                let o = d.objectives.get(name);
                TrackerLine {
                    name: name.clone(),
                    message: o.map(|o| o.message.clone()).unwrap_or_default(),
                    complete: state.completed_objectives.contains(name),
                    optional: o.is_some_and(|o| o.optional),
                    count: o.map_or(1, |o| o.count),
                }
            })
            .collect()
    }

    /// The effects raised since the last drain, in order.
    pub fn drain(&self) -> Vec<Effect> {
        std::mem::take(&mut self.inner.state.borrow_mut().effects)
    }

    /// The error lines (the tracker's own and the provider's, each once, in order of first appearance).
    pub fn errors(&self) -> Vec<String> {
        self.inner.state.borrow().errors.clone()
    }

    /// Every `NotYet` the provider recorded for a behavior class it has no handler for, in order (the dialog
    /// behavior, module documentation).
    pub fn not_yet(&self) -> Vec<NotYet> {
        self.inner
            .provider
            .borrow()
            .as_ref()
            .map(|p| p.not_yet.clone())
            .unwrap_or_default()
    }

    // ---- hooks ----------------------------------------------------------------------------------------------------

    pub fn set_status_hook(&self, f: Option<StatusHook>) {
        self.inner.hooks.borrow_mut().status = f;
    }
    pub fn set_kickoff_hook(&self, f: Option<KickoffHook>) {
        self.inner.hooks.borrow_mut().kickoff = f;
    }
    pub fn set_notify_hook(&self, f: Option<NotifyHook>) {
        self.inner.hooks.borrow_mut().notify = f;
    }
    pub fn set_objective_hook(&self, f: Option<ObjectiveHook>) {
        self.inner.hooks.borrow_mut().objective = f;
    }

    /// A named Kismet variable of the mission's provider bound to a world object (`None` clears it); no effect without
    /// a provider. Refused while the provider is running.
    pub fn bind_kismet_variable(&self, name: &str, object: Option<ObjRef>) -> Result<(), MissionError> {
        self.usable()?;
        if !self.inner.has_provider {
            return Ok(());
        }
        let mut slot = self.inner.provider.borrow_mut();
        let p = slot.as_mut().ok_or_else(busy)?;
        p.bind_kismet_variable(name, object);
        Ok(())
    }

    /// The bridge reports these host-side effects so they sit in the effect order.
    pub fn note(&self, kind: EffectKind, a: &str, b: &str, c: &str) {
        self.emit(kind, a.to_string(), b.to_string(), c.to_string());
    }
}

fn lends(d: &Data) -> bool {
    !d.weapon.is_empty() && d.objectives.values().any(|o| o.path == d.weapon_objective)
}

fn progress_of(d: &Data, state: &State, name: &str) -> i32 {
    let Some(&value) = state.progress.get(name) else {
        return 0;
    };
    match d.objectives.get(name) {
        Some(o) if o.mask => bits(value),
        _ => value,
    }
}

/// The `NotYet` text of a call on a mission whose provider a failed [`lend`] lost.
pub const PROVIDER_LOST: &str =
    "a mission whose behavior provider was lost by a failed lend (every later call is refused)";

fn lost_error() -> MissionError {
    BehaviorError::Vm(VmError::NotYet(NotYet::not_ported(PROVIDER_LOST, RECORD)))
}

fn busy() -> MissionError {
    BehaviorError::Vm(VmError::NotYet(NotYet::not_ported(PROVIDER_BUSY, RECORD)))
}

/// Runs `f` with the running provider `p` lent back to its `slot` (module documentation, "Re-entrancy"): `p` is
/// swapped for a copy of its data while `f` runs, so the tracker, a hook or a native `f` reaches finds the provider in
/// its slot and nests its calls on it, as the C++ nests them; afterwards the provider (with what the nested calls did)
/// is swapped back into `p`. The kernel's call that lent `p` is suspended meanwhile, so it never sees the copy. Refused
/// when the slot is not empty (it holds the provider only while it is not running).
pub fn lend<T>(
    slot: &RefCell<Option<Provider>>,
    lost: &Cell<bool>,
    p: &mut Provider,
    f: impl FnOnce() -> Result<T, MissionError>,
) -> Result<T, MissionError> {
    if lost.get() {
        return Err(lost_error());
    }
    if slot.borrow().is_some() {
        return Err(busy());
    }
    let placeholder = Provider::from_data(p.data().clone());
    let running = std::mem::replace(p, placeholder);
    *slot.borrow_mut() = Some(running);
    let result = f();
    match slot.borrow_mut().take() {
        Some(back) => *p = back,
        // f took the provider and did not put it back. The slot's own code always puts it back, so this needs a caller
        // that takes it itself; the provider is gone, so its owner is marked lost and every later call is refused,
        // rather than letting the placeholder copy run.
        None => {
            lost.set(true);
            return Err(lost_error());
        }
    }
    result
}

// ---------------------------------------------------------------------------------------------------------------
// The rules
// ---------------------------------------------------------------------------------------------------------------

impl MissionSystem {
    fn emit(&self, kind: EffectKind, a: String, b: String, c: String) {
        let mut s = self.inner.state.borrow_mut();
        let time = s.now;
        s.effects.push(Effect {
            kind,
            a,
            b,
            c,
            time,
            detail: String::new(),
        });
    }

    fn hooks(&self) -> Hooks {
        self.inner.hooks.borrow().clone()
    }

    fn notify(&self, vm: &mut Vm, n: Notification) {
        if let Some(f) = self.hooks().notify {
            f(vm, n);
        }
    }

    /// Fires `event` with `link_id` on the provider, wherever it is (module documentation, "Re-entrancy").
    fn fire(&self, vm: &mut Vm, event: &str, link_id: i32) -> Result<(), MissionError> {
        self.with_provider(vm, |p, vm| p.fire_event(vm, event, &DetMap::new(), link_id, &[]))
    }

    /// Runs `f` on the provider taken out of its slot, and puts it back whatever `f` returns. Nothing without a
    /// provider; [`PROVIDER_BUSY`] when it is out of its slot.
    fn with_provider(
        &self,
        vm: &mut Vm,
        f: impl FnOnce(&mut Provider, &mut Vm) -> Result<(), MissionError>,
    ) -> Result<(), MissionError> {
        self.usable()?;
        if !self.inner.has_provider {
            return Ok(());
        }
        let taken = self.inner.provider.borrow_mut().take();
        let Some(mut p) = taken else {
            return Err(busy());
        };
        let result = f(&mut p, vm);
        *self.inner.provider.borrow_mut() = Some(p);
        result
    }

    /// Refused once the provider was lost ([`lend`]).
    fn usable(&self) -> Result<(), MissionError> {
        if self.inner.lost.get() { Err(lost_error()) } else { Ok(()) }
    }

    /// The mission's dialog (module documentation): the one the dialog behavior of its provider runs on, which the
    /// script bridge's dialog-component native shares. Two rules for a holder of this `Rc` (review of PR #337):
    /// - do not replace `DialogSystem::on_line`: the tracker's sink lives there, and every chosen line becomes a
    ///   `Dialog` effect through it;
    /// - do not `dialog::register` it on the VM: the mission's provider runs it through its own handler, and a
    ///   registry handler would make every other provider of the VM talk through this mission's dialog.
    pub fn dialog(&self) -> Rc<RefCell<DialogSystem>> {
        self.inner.dialog.clone()
    }

    /// Runs `f` on the mission's dialog; refused when the dialog is already in use (a call from inside one of its own
    /// calls, which no path of this crate makes).
    pub fn with_dialog<T>(&self, f: impl FnOnce(&mut DialogSystem) -> T) -> Result<T, MissionError> {
        let mut d = self
            .inner
            .dialog
            .try_borrow_mut()
            .map_err(|_| data_error("dialog: the dialog system is already in use".to_string()))?;
        Ok(f(&mut d))
    }

    /// The dialog line as a `Dialog` effect (the C++ `onLine`).
    fn emit_dialog(&self, line: &Line, outcome: &str) {
        self.emit(EffectKind::Dialog, line.event_tag.clone(), line.group.clone(), line.talker.clone());
        let detail = format!(
            "act={};ak={};talker={};outcome={outcome};line={}",
            line.talk_act,
            line.ak_event,
            if line.echo { "echo" } else { "pawn" },
            line.id
        );
        if let Some(e) = self.inner.state.borrow_mut().effects.last_mut() {
            e.detail = detail;
        }
    }

    /// `updateTrackedMission`: while Active or ReadyToTurnIn the mission is the tracked one, its `MissionDialogGroup`
    /// the dialog's priority floor. Refused when the dialog is in use (no path does that).
    fn update_tracked_mission(&self) -> Result<(), MissionError> {
        let tracked = matches!(self.status(), Status::Active | Status::ReadyToTurnIn);
        let d = self.data();
        let group = if tracked { d.dialog_group.as_str() } else { "" };
        let mut dialog = self.inner.dialog.try_borrow_mut().map_err(|_| {
            data_error("dialog: the dialog system is already in use (the tracked mission)".to_string())
        })?;
        dialog.set_tracked_mission(group, d.plot_critical);
        Ok(())
    }

    /// Whether a failed lend lost the provider (every later call is then refused).
    pub fn is_lost(&self) -> bool {
        self.inner.lost.get()
    }

    /// The provider's error lines joined to the tracker's, each once (the C++ `collectProviderErrors`).
    fn collect(&self) {
        let lines = match self.inner.provider.borrow().as_ref() {
            Some(p) => p.errors.clone(),
            None => return,
        };
        let mut s = self.inner.state.borrow_mut();
        for line in lines {
            if !s.errors.contains(&line) {
                s.errors.push(line);
            }
        }
    }

    /// B4 (UNVERIFIED): ReadyToTurnIn only from Active, Complete only from ReadyToTurnIn; every accepted change fires
    /// `Default` with id 6 + the status number. Inside the branch, in order: the state, the script hook
    /// (`UpdateMissionStatus`), the pending kickoff record (Active), the status effect, the lent weapon (granted inside
    /// Active, removed inside Complete: observed in the real game, FIRE_MISSION_GOLDEN_TRACE.md lane L2), the
    /// observers, the event.
    fn set_status(&self, vm: &mut Vm, status: Status) -> Result<bool, MissionError> {
        {
            let mut s = self.inner.state.borrow_mut();
            let current = s.status.unwrap_or(Status::NotStarted);
            let allowed = matches!(
                (status, current),
                (Status::Active, Status::NotStarted)
                    | (Status::ReadyToTurnIn, Status::Active)
                    | (Status::Complete, Status::ReadyToTurnIn)
            );
            if !allowed {
                return Ok(false);
            }
            // Rules neither side models, decided before anything changes (Off mode; Reference keeps the C++).
            if vm.parity == ParityMode::Off {
                let d = &self.inner.data;
                if matches!(status, Status::Active | Status::Complete) && lends(d) {
                    return Err(NotYet::not_read("the lent mission weapon's region-stage condition", BRIDGE_RECORD)
                    .into());
                }
                if status == Status::Active && d.plot_critical && s.kickoff_pending {
                    return Err(
                        NotYet::not_read("a plot-critical mission overwriting a pending kickoff record", RECORD).into()
                    );
                }
            }
            s.status = Some(status);
            if status == Status::Complete {
                s.active_set.clear();
            }
        }
        self.update_tracked_mission()?;
        if let Some(f) = self.hooks().status {
            f(vm, status.number());
        }
        {
            // SetActiveMission(mission, fromActivation): a record only when the kickoff was not heard and none is
            // pending.
            let mut s = self.inner.state.borrow_mut();
            if status == Status::Active && !s.heard_kickoff && !s.kickoff_pending {
                s.kickoff_pending = true;
                s.kickoff_from_activation = true;
            }
        }
        self.emit(EffectKind::StatusChanged, status.name().to_string(), String::new(), String::new());
        if self.lends_weapon() {
            let weapon = self.data().weapon.clone();
            match status {
                Status::Active => self.emit(EffectKind::MissionWeaponGranted, weapon, String::new(), String::new()),
                Status::Complete => self.emit(EffectKind::MissionWeaponRemoved, weapon, String::new(), String::new()),
                _ => {}
            }
        }
        self.notify(vm, Notification::StatusChanged);
        self.fire(vm, "Default", link::STATUS_BASE + i32::from(status.number()))?;
        Ok(true)
    }

    /// B3 (UNVERIFIED): only the active set's `NextSet`, or the initial set while no set is active; anything else is
    /// ignored silently.
    fn request_advance(&self, vm: &mut Vm, target: &str) -> Result<(), MissionError> {
        if target.is_empty() {
            return Ok(());
        }
        let active = self.active_set();
        let d = self.data();
        let next_of_active = d.find_set(&active).is_some_and(|s| s.next == target);
        if next_of_active || (active.is_empty() && target == d.initial_set) {
            self.activate_set(vm, target)?;
        }
        Ok(())
    }

    /// B3 (UNVERIFIED): refused when ReadyToTurnIn or Complete; the set's event with id 4; then the B2 evaluation when
    /// the set can complete the mission and is still active.
    fn activate_set(&self, vm: &mut Vm, path: &str) -> Result<bool, MissionError> {
        if matches!(self.status(), Status::ReadyToTurnIn | Status::Complete) {
            return Ok(false);
        }
        let d = self.data();
        let Some(set) = d.find_set(path) else {
            self.inner
                .state
                .borrow_mut()
                .errors
                .push(format!("advance to unknown objective set {path}"));
            return Ok(false);
        };
        // Reached only in Reference mode: Off refuses such a mission at construction.
        if set.cls != OBJECTIVE_SET_CLASS {
            self.inner
                .state
                .borrow_mut()
                .errors
                .push(format!("unsupported objective set class {} ({})", set.cls, set.name));
            return Ok(false);
        }
        self.inner.state.borrow_mut().active_set = path.to_string();
        self.emit(EffectKind::ObjectiveSetActive, set.name.clone(), String::new(), String::new());
        self.notify(vm, Notification::ObjectiveSetChanged);
        self.fire(vm, &set.name, link::SET_ACTIVATED)?;
        if set.can_complete && self.active_set() == path {
            self.evaluate_set(vm)?;
        }
        Ok(true)
    }

    /// B2 (UNVERIFIED): a complete set fires id 5 once, then `bCanCompleteMission` -> ReadyToTurnIn, else
    /// `bAutoEnableNextSet` -> `NextSet`, else nothing (a behavior must advance it). Only optional objectives left in a
    /// `bCanCompleteMission` set: ReadyToTurnIn.
    fn evaluate_set(&self, vm: &mut Vm) -> Result<(), MissionError> {
        let d = self.data();
        let active = self.active_set();
        let Some(set) = d.find_set(&active) else {
            return Ok(());
        };
        let (mut all, mut required) = (true, true);
        {
            let s = self.inner.state.borrow();
            for objective in &set.objectives {
                if s.completed_objectives.contains(objective) {
                    continue;
                }
                all = false;
                let optional = d.objectives.get(objective).is_some_and(|o| o.optional);
                if !set.can_complete || !optional {
                    required = false;
                }
            }
        }
        if all {
            if !self.inner.state.borrow_mut().completed_sets.insert(set.path.clone()) {
                return Ok(());
            }
            self.fire(vm, &set.name, link::SET_COMPLETED)?;
            if set.can_complete {
                self.set_status(vm, Status::ReadyToTurnIn)?;
            } else if set.auto_next && !set.next.is_empty() {
                self.activate_set(vm, &set.next)?;
            }
        } else if required && self.status() == Status::Active {
            self.set_status(vm, Status::ReadyToTurnIn)?;
        }
        Ok(())
    }

    /// `ActivateMission` (B4, UNVERIFIED): NotStarted -> Active (`Default` id 7), then the initial set only when
    /// `bActivateInitialObjectiveSet`. Acceptance also writes the pending kickoff record, which the next
    /// [`MissionSystem::tick`] consumes. False when [`MissionSystem::can_start`] does not hold.
    pub fn accept(&self, vm: &mut Vm, completed: &DetSet<String>) -> Result<bool, MissionError> {
        self.usable()?;
        if !self.can_start(completed, vm)? {
            return Ok(false);
        }
        self.set_status(vm, Status::Active)?;
        let d = self.data();
        if d.activate_initial_set && self.active_set().is_empty() && !d.initial_set.is_empty() {
            self.activate_set(vm, &d.initial_set)?;
        }
        self.collect();
        Ok(true)
    }

    /// `PlayKickoff` / `PlayKickoffDialogOnly`: `Default` with id 12 / 13, played at once (consuming the pending
    /// record). False unless Active.
    pub fn kickoff(&self, vm: &mut Vm, dialog_only: bool) -> Result<bool, MissionError> {
        self.usable()?;
        if self.status() != Status::Active {
            return Ok(false);
        }
        let id = if dialog_only { link::KICKOFF_DIALOG_ONLY } else { link::KICKOFF };
        self.fire(vm, "Default", id)?;
        {
            let mut s = self.inner.state.borrow_mut();
            s.heard_kickoff = true;
            s.kickoff_pending = false;
        }
        self.collect();
        Ok(true)
    }

    /// `PlayTurnIn` with a mission: `Default` with id 14 and nothing else (bridge note).
    pub fn play_turn_in(&self, vm: &mut Vm) -> Result<(), MissionError> {
        self.usable()?;
        self.fire(vm, "Default", link::TURN_IN)?;
        self.collect();
        Ok(())
    }

    /// `MissionTracker.UpdateObjective` by objective name: one queued update (+1, or the bit ORed in for a bit-mask
    /// objective). Returns whether the first queued update (this one, unless one is already being applied) was
    /// applied.
    pub fn update_objective(&self, vm: &mut Vm, name: &str, bit: i32) -> Result<bool, MissionError> {
        self.update(vm, name, bit)
    }

    /// What `Behavior_UpdateMissionObjective` names: the objective by path. A path of no objective of this mission is
    /// an error line and false.
    pub fn update_objective_by_path(&self, vm: &mut Vm, path: &str, bit: i32) -> Result<bool, MissionError> {
        self.update_by_path(vm, path, bit)
    }

    fn update_by_path(&self, vm: &mut Vm, path: &str, bit: i32) -> Result<bool, MissionError> {
        let found = self
            .data()
            .objectives
            .iter()
            .find(|(_, o)| o.path == path)
            .map(|(name, _)| name.clone());
        match found {
            Some(name) => self.update(vm, &name, bit),
            None => {
                self.inner
                    .state
                    .borrow_mut()
                    .errors
                    .push(format!("objective is not part of this mission: {path}"));
                Ok(false)
            }
        }
    }

    /// B1 (UNVERIFIED): updates are queued; one raised while another is applied waits its turn instead of nesting. A
    /// failure empties the queue.
    fn update(&self, vm: &mut Vm, name: &str, bit: i32) -> Result<bool, MissionError> {
        self.usable()?;
        {
            let mut s = self.inner.state.borrow_mut();
            s.updates.push_back((name.to_string(), bit));
            if s.draining {
                return Ok(true);
            }
            s.draining = true;
        }
        let mut first: Option<bool> = None;
        loop {
            let next = self.inner.state.borrow_mut().updates.pop_front();
            let Some((name, value)) = next else {
                break;
            };
            match self.apply_update(vm, &name, value) {
                Ok(applied) => {
                    first.get_or_insert(applied);
                }
                Err(e) => {
                    let mut s = self.inner.state.borrow_mut();
                    s.draining = false;
                    s.updates.clear();
                    return Err(e);
                }
            }
        }
        self.inner.state.borrow_mut().draining = false;
        self.collect();
        Ok(first.unwrap_or(false))
    }

    /// B1 (UNVERIFIED): id 3 on every accepted update; complete when the count equals `ObjectiveCount`; then the set
    /// evaluation, and only then id 2.
    fn apply_update(&self, vm: &mut Vm, name: &str, bit: i32) -> Result<bool, MissionError> {
        if self.status() != Status::Active {
            return Ok(false);
        }
        let d = self.data();
        let in_active = d
            .find_set(&self.active_set())
            .is_some_and(|s| s.objectives.iter().any(|o| o == name));
        if !in_active {
            return Ok(false);
        }
        let Some(objective) = d.objectives.get(name) else {
            return Ok(false);
        };
        let count = {
            let mut s = self.inner.state.borrow_mut();
            if progress_of(d, &s, name) >= objective.count {
                return Ok(false);
            }
            // The zero-bit refusal comes before the progress entry is made, as in the C++ (`mission.cpp`), so a refused
            // update leaves no `progress=` line in the save text.
            if objective.mask && bit == 0 {
                return Ok(false);
            }
            let entry = s.progress.entry(name.to_string()).or_insert(0);
            if objective.mask {
                *entry |= bit;
            } else {
                // Below the count, which is an i32: the increment cannot pass i32::MAX.
                *entry += 1;
            }
            progress_of(d, &s, name)
        };
        self.emit(EffectKind::ObjectiveUpdated, name.to_string(), count.to_string(), String::new());
        self.notify(vm, Notification::ObjectiveUpdated);
        if let Some(f) = self.hooks().objective {
            f(vm, &objective.path, bit);
        }
        self.fire(vm, name, link::OBJECTIVE_PROGRESS)?;
        if count != objective.count {
            return Ok(true);
        }
        self.inner.state.borrow_mut().completed_objectives.insert(name.to_string());
        self.emit(EffectKind::ObjectiveComplete, name.to_string(), String::new(), String::new());
        self.notify(vm, Notification::ObjectiveComplete);
        self.evaluate_set(vm)?;
        self.fire(vm, name, link::OBJECTIVE_COMPLETED)?;
        Ok(true)
    }

    /// `RunMissionCustomEvent`: id 0, unless the mission is Complete (B4, UNVERIFIED).
    pub fn custom_event(&self, vm: &mut Vm, name: &str) -> Result<bool, MissionError> {
        self.usable()?;
        if self.status() == Status::Complete {
            return Ok(false);
        }
        self.fire(vm, name, link::CUSTOM)?;
        self.collect();
        Ok(true)
    }

    /// Turn-in: ReadyToTurnIn -> Complete and a `Reward` effect naming the XP attribute.
    pub fn turn_in_mission(&self, vm: &mut Vm) -> Result<bool, MissionError> {
        self.usable()?;
        if !self.set_status(vm, Status::Complete)? {
            return Ok(false);
        }
        let xp = self.data().xp_attribute.clone();
        self.emit(EffectKind::Reward, xp, String::new(), String::new());
        self.collect();
        Ok(true)
    }

    /// The tracker tick: the pending kickoff record is consumed (the kickoff hook runs, then `PlayKickoff` (id 12) when
    /// the record came from an activation, else `PlayKickoffDialogOnly` (13); the kickoff is heard and the record
    /// cleared; bridge note, UNVERIFIED; the "Loader" level wait, the tracked-mission switch and the dialog request
    /// are not modelled), then the provider's delayed links run.
    pub fn tick(&self, vm: &mut Vm, seconds: f64) -> Result<(), MissionError> {
        self.usable()?;
        let pending = {
            let mut s = self.inner.state.borrow_mut();
            s.now += seconds;
            s.kickoff_pending.then_some(s.kickoff_from_activation)
        };
        if let Some(from_activation) = pending {
            if let Some(f) = self.hooks().kickoff {
                f(vm);
            }
            let id = if from_activation { link::KICKOFF } else { link::KICKOFF_DIALOG_ONLY };
            self.fire(vm, "Default", id)?;
            let mut s = self.inner.state.borrow_mut();
            s.heard_kickoff = true;
            s.kickoff_pending = false;
        }
        // The dialog clock follows the kernel's time (its time hook), or the frame's without a provider (the C++).
        if self.inner.has_provider {
            self.with_provider(vm, |p, vm| p.tick(vm, seconds))?;
        } else {
            self.inner
                .dialog
                .try_borrow_mut()
                .map_err(|_| data_error("dialog: the dialog system is already in use (the tick)".to_string()))?
                .tick(seconds);
        }
        self.collect();
        Ok(())
    }

    // ---- persistence ------------------------------------------------------------------------------------------------

    /// This mission's state as text (the caller owns the file and the completed set).
    pub fn save_state(&self) -> String {
        let s = self.inner.state.borrow();
        let mut out = format!(
            "status={}\nactive={}\n",
            s.status.unwrap_or(Status::NotStarted).index(),
            s.active_set
        );
        for o in &s.completed_objectives {
            out.push_str(&format!("objective={o}\n"));
        }
        for (o, value) in &s.progress {
            if !s.completed_objectives.contains(o) {
                out.push_str(&format!("progress={o}:{value}\n"));
            }
        }
        for set in &s.completed_sets {
            out.push_str(&format!("set={set}\n"));
        }
        out
    }

    /// Restores a [`MissionSystem::save_state`] text; false (and nothing changed) when it does not parse, names no
    /// status, or names an active set this mission does not have. A completed objective's progress is its count (a
    /// bit-mask objective's full mask; one whose count is outside 0..=30 refuses the text, where the C++ shift is
    /// undefined). The kickoff counts as heard once started (an assumption, UNVERIFIED).
    pub fn load_state(&self, text: &str) -> bool {
        if self.is_lost() {
            return false;
        }
        let mut status = None;
        let mut active = String::new();
        let (mut objectives, mut sets) = (DetSet::new(), DetSet::new());
        let mut progress = DetMap::new();
        for line in getlines(text) {
            let Some((key, value)) = line.split_once('=') else {
                return false;
            };
            match key {
                "status" => {
                    let b = value.as_bytes();
                    if b.len() != 1 || !(b'0'..=b'3').contains(&b[0]) {
                        return false;
                    }
                    status = Status::from_index(b[0] - b'0');
                }
                "active" => active = value.to_string(),
                "objective" => {
                    objectives.insert(value.to_string());
                }
                "set" => {
                    sets.insert(value.to_string());
                }
                "progress" => {
                    let Some(colon) = value.rfind(':') else {
                        return false;
                    };
                    let Some(n) = cxx_stoi(&value[colon + 1..]) else {
                        return false;
                    };
                    progress.insert(value[..colon].to_string(), n);
                }
                _ => return false,
            }
        }
        let Some(status) = status else {
            return false;
        };
        let d = self.data();
        if !active.is_empty() && d.find_set(&active).is_none() {
            return false;
        }
        for o in &objectives {
            let full = match d.objectives.get(o) {
                None => 1,
                Some(x) if x.mask => match u32::try_from(x.count).ok().filter(|&c| c <= 30) {
                    Some(c) => (1i32 << c) - 1,
                    None => return false,
                },
                Some(x) => x.count,
            };
            progress.insert(o.clone(), full);
        }
        let mut s = self.inner.state.borrow_mut();
        s.status = Some(status);
        s.active_set = active;
        s.completed_objectives = objectives;
        s.completed_sets = sets;
        s.progress = progress;
        s.kickoff_pending = false;
        s.heard_kickoff = status != Status::NotStarted;
        drop(s);
        // The C++ updateTrackedMission; a dialog in use (no path) leaves the floor as it was.
        let _ = self.update_tracked_mission();
        true
    }
}

/// The lines `std::getline(stream, line)` reads: split at `\n`, without the empty piece after a final `\n`.
fn getlines(text: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = text.split('\n').collect();
    if lines.last() == Some(&"") {
        lines.pop();
    }
    lines
}

/// `std::stoi` of the C++ `loadState`: leading C white space, an optional sign, at least one digit (trailing text
/// ignored); `None` where it throws (no digits, out of range).
fn cxx_stoi(text: &str) -> Option<i32> {
    let b = text.as_bytes();
    let mut at = 0;
    while at < b.len() && matches!(b[at], b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r') {
        at += 1;
    }
    let start = at;
    if at < b.len() && (b[at] == b'+' || b[at] == b'-') {
        at += 1;
    }
    let digits = at;
    while at < b.len() && b[at].is_ascii_digit() {
        at += 1;
    }
    if at == digits {
        return None;
    }
    text[start..at].parse::<i32>().ok()
}
