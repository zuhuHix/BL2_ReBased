//! Owner: lane IMPL-MISSION-NUMBERS (decision 2026-11-19-93). The two mission numbers of the online game settings
//! refresh, and the mission screen's calls the accept reaches after it. AI-assisted (Claude Code).
//!
//! Coded from NATIVE_MISSION_NUMBERS.md alone (#473 at f586ab9c, its review r2, the final round; analyst lane
//! ANALYST-MISSION-NUMBERS, decision 2026-11-19-86). No
//! native code or analysis output was opened. **Every rule is UNVERIFIED in the running game.** Off only: the caller
//! (`datastore::register`) binds nothing here under `ParityMode::Reference`, so the VM's stub policy answers there.
//!
//! **The two reads NATIVE_MISSION_NUMBERS.md left open** are settled by NATIVE_PLAYTHROUGH_QUERY.md (#486 at 3bb3f6de,
//! analyst lane LINK-10-MISSIONLIST, decision 2026-11-19-99; implemented by lane IMPL-PLAYTHROUGH-VIRTUAL): the
//! engine-wide condition is the port's world existing, and the current-playthrough virtual is
//! [`current_playthrough`]. #480's conservative both-branch form and its `NotYet` are retired.
//!
//! **`WillowPlayerController.GetActiveMissionNumber()`** (section 1). It writes nothing, so nothing is preflighted and
//! nothing rolled back.
//! 1. The world's `WorldInfo` (the level's, as `GetWorldInfo` answers), its `GRI` as a `WillowGameReplicationInfo`,
//!    and that object's `MissionTracker`. With no world, a replication info of another class, or no tracker, the
//!    answer is the fallback (step 4).
//!    - **The engine-wide condition** (NATIVE_PLAYTHROUGH_QUERY.md section 1) is the engine's current world being set,
//!      which in the port is the world's `WorldInfo` existing: with it, the tracker branch alone runs; without it, the
//!      fallback.
//!    - The tracker branch: the tracker's `IsDataValid()` through the VM; false is the fallback. In the installed
//!      slice its data is valid, so the branch reaches the bridge's `GetAllMissions` `LayerAbsent` first.
//! 2. The tracker's `ActiveMission` set: that definition's `MissionNumber`, with no status or plot-critical check.
//! 3. Otherwise the tracker's `GetAllMissions(out)` list, through the VM, in its order. A definition with
//!    `bPlotCritical` and no `DlcExpansion` has its `GetMissionStatus` asked through the VM. In progress (1, 2, 3)
//!    answers its `MissionNumber` at once. Complete (4) keeps the largest. The end answers the largest kept, else 0.
//! 4. The fallback, `GetActivePlotCriticalMissionNumber` for the current playthrough, is taken when step 1 fails or
//!    steps 2 and 3 answer -1.
//!
//! **`WillowPlayerController.GetActivePlotCriticalMissionNumber(optional int PlaythroughIndex)`** (section 2):
//! 1. A negative or omitted index asks the controller's current-playthrough virtual: [`current_playthrough`] as a
//!    function on the controller, not a VM dispatch of `GetCurrentPlaythrough` (NATIVE_PLAYTHROUGH_QUERY.md section 4,
//!    "Direct callers"). An explicit index of 0 or more does not ask it.
//! 2. Out of range (negative, or not below the length of `MissionPlaythroughs`): 0.
//! 3. That playthrough's `MissionList`, in order. A record with a `MissionDef` that has `bPlotCritical` and no
//!    `DlcExpansion` is graded by its own `Status` field. In progress answers its `MissionNumber` at once, Complete
//!    keeps the largest, and the end answers the largest kept, else 0.
//!
//! **Calls and data.** A tracker or controller function the VM does not bind is the VM's own answer (its `NotYet` or
//! refusal), propagated unchanged. Inside the settings refresh, the refresh's write mark (decision 2026-11-19-80) turns
//! such a `NotYet`, and these natives' own, into a terminal stop after partial writes, kind and record kept. A
//! definition or record that lacks one of the fields read here, or holds the wrong kind there, is bad data: a
//! `VmError::Script` naming the field. A `None` entry in the tracker's list is [`NONE_ENTRY`], `NotYet { NotRead }`:
//! the game's loop reads each entry with no `None` test, and what it does with one was not read.
//!
//! **The mission screen** (section 3). The slice's accept reaches it through the controller's
//! `ClientUpdateUIMissionList`, the movie's `UpdateMissionList` and `extPopulateQuestEntries`, after the accept's own
//! writes.
//! - **`QuestAcceptGFxMovie.UpdateMissionTextList()`** (lane IMPL-LINK10-NEXT, from NATIVE_MISSION_TEXT_LIST.md
//!   section 2 and its Off step 1, which settle NATIVE_MISSION_NUMBERS.md open question 3): the movie's class must
//!   declare `MissionTextList` and `MissionCategories`, else a `VmError::Script` naming the field. With
//!   `MissionTextList` None the body reads nothing more and writes nothing, so the native returns at once. With it set,
//!   [`TEXT_LIST`], `NotYet { LayerAbsent }` (record [`RECORD_GFX`]): its steps rebuild a container that only a hosted
//!   movie fills.
//! - **`GFxMoviePlayer.GetVariableObject(Path, Type)`** (NATIVE_MISSION_TEXT_LIST.md section 3): the game asks the
//!   Flash player only when the movie has a native player object with a movie view; with none, no wrapper is made and
//!   the answer is None. The port's movies are stand-ins with no native player, so Off answers None and writes nothing.
//!   UNVERIFIED (the return on this branch was read by fit).
//! - **`WillowGFxMovie3D.FocusOn(Thing)`** (section 4, with the hub's ruling R2 on #504; UNVERIFIED).
//!   - **Any `Thing` but None:** [`FOCUS`], `NotYet { LayerAbsent }`, at once: the chain step's Flash lookups need a
//!     hosted movie. Nothing is read or written first (review r1 of #509).
//!   - **`Thing` None, on a movie with no native player** (every port movie): [`focus_writes`] plans, writing
//!     nothing, `FocusObject` = None, then `FocusOffset` = the zero `Vector` (X, Y and Z all 0), and the native makes
//!     those two writes in that order and no other (`InterpFocusOffset` is not written). Before either write:
//!     - the class must declare `FocusObject` as an object and `FocusOffset` as a struct declaring float `X`, `Y` and
//!       `Z`, else a `VmError::Script` naming the field;
//!     - the clamp's Y bound ([`y_bound`]): half the margin between the stage height and the stage height times the
//!       scale. The port's stand-in for a movie with no native player is a stage of height 0 and scale 0
//!       ([`FocusStage::NO_PLAYER`]), so the bound is 0. A bound that is negative or NaN is [`FOCUS_BOUND`],
//!       `NotYet { NotRead }`, record [`RECORD_TEXT_LIST`] (#504's note, which lands before or with this). The
//!       offset is zero with no `FocusObject`, so the clamp leaves it zero.
//! - **`WillowLeviathanService.RecordMissionStatusChangedEventForPlayer(ForPlayer, Status, MissionDefinition)`**
//!   (section 5, with the hub's ruling R3 on #504): the shared statistics submit routine answers nothing, with no write,
//!   for every player. A port answer, the same as the revived event's (#500); UNVERIFIED. The player argument must be
//!   an object.
//!   - **Known limit, not fixed here (review r1 of #509, item 4):** the binding goes to the native's dispatch key,
//!     `WillowLeviathanService.RecordMissionStatusChangedEventForPlayer(object,byte,name)`, which names the class and
//!     the function but not the package (the VM builds it as the C++ `nativeKey` does). The VM dispatches every
//!     native through one key map, and its per-declaration binding (`Vm::override_script`) takes script functions
//!     only. So a class of the same name with the same native in another package would get this no-op too. Scoping
//!     a native to its resolved declaration is a VM design change, queued by the hub as its own item.
//! - **`WillowLeviathanService.RecordMissionObjectiveChangedEventForPlayer(ForPlayer, ObjectiveIndex,
//!   ObjectiveProgress, PreviousObjectiveProgress, MissionDefinition)`**, no return value (lane
//!   IMPL-R3-MISSION-OBJECTIVE, slice link 11; NATIVE_RECORD_MISSION_OBJECTIVE_CHANGED.md section 3, with the hub's
//!   reading of the declared parameters on #531's review): it ends in the same shared statistics submit routine, so
//!   ruling R3 covers it. Nothing, with no write and no load, for every player, a None one included. Nothing is read
//!   from the three integers or the name: the script's calls pass the old value third and the new value fourth, and
//!   what each means stays UNVERIFIED. A port answer, UNVERIFIED. No telemetry-specific refusal or preflight; the
//!   ordinary argument errors apply, as for the status event (a player argument that is not an object is refused).
//!   **Scoped to its own declaration** (review r1 of #533, item 1): the VM on this base binds by the dispatch key,
//!   which names no package, so a same-named class of another package with the same native reaches this body too.
//!   The body checks the called function's resolved path and answers only for
//!   `WillowGame.WillowLeviathanService.RecordMissionObjectiveChangedEventForPlayer` (case-insensitive). Any other
//!   declaration gets the unbound behaviour: `VmError::UnboundNative` under the VM's stub policy, or, where the policy
//!   allows the key, the legacy `UNIMPLEMENTED <key>` line and no value. #519's package-qualified dispatch makes the
//!   check redundant once it lands; it stays harmless then. The status event above keeps the known limit until #519.
//! - **The partial-writes mark** (review r1 of #480, item 2; r2, H1): these natives write nothing, so they do not mark
//!   their own `NotYet`. The mark is applied where a write is known, in the mission bridge (`ow_mission::bridge`): the
//!   status routine's hook (`UpdateMissionStatus`, which runs only after the status was written, and is the screen's
//!   only script route: the accept, the turn-in and the reward page all reach it) marks a `NotYet` before its own run
//!   records it; the accept's and the turn-in's own frames mark one that leaves after the status changed. A direct
//!   call on a fresh movie is not marked.
//! - **What the accept checks after the screen returns** is the bridge's own code. Nothing here answers for it.
//!
//! **Dispatch and lazy loads** (review r1 of #480, items 1 and 3):
//! - `GetActiveMissionNumber`'s fallback is the controller's `GetActivePlotCriticalMissionNumber(-1)`, dispatched
//!   through the VM on the controller's class, so an unbound or overridden callee answers for itself (an unbound one is
//!   `VmError::UnboundNative`, unchanged).
//! - A field read on a store definition that is not loaded yet (`DefStore::is_unloaded`) loads it first. That load is
//!   a write: it marks a running settings refresh, and a `NotYet` that leaves the native after it is marked "stopped
//!   after partial writes" once, kind and record kept.

use ow_core::NotYet;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{CallResult, NativeCall, Vm};
use ow_vm::error::VmError;

use crate::actors::Actors;

/// The record these rules come from.
pub const RECORD: &str = "docs/verification/NATIVE_MISSION_NUMBERS.md";
/// `WillowPlayerController.GetActiveMissionNumber`.
pub const ACTIVE_MISSION_NUMBER: &str = "WillowGame.WillowPlayerController.GetActiveMissionNumber";
/// `WillowPlayerController.GetActivePlotCriticalMissionNumber`.
pub const PLOT_CRITICAL_NUMBER: &str = "WillowGame.WillowPlayerController.GetActivePlotCriticalMissionNumber";
/// `QuestAcceptGFxMovie.UpdateMissionTextList`.
pub const UPDATE_MISSION_TEXT_LIST: &str = "WillowGame.QuestAcceptGFxMovie.UpdateMissionTextList";
/// The replication info class the tracker hangs on.
pub const GRI_CLASS: &str = "WillowGame.WillowGameReplicationInfo";
/// `GFxMoviePlayer.GetVariableObject` and `WillowGFxMovie3D.FocusOn`, the focus pair of the screen's `SetFocus`.
pub const GET_VARIABLE_OBJECT: &str = "GFxUI.GFxMoviePlayer.GetVariableObject";
pub const FOCUS_ON: &str = "WillowGame.WillowGFxMovie3D.FocusOn";
/// The `NotYet` of a `None` entry in the tracker's mission list (section 1, Off step 5).
pub const NONE_ENTRY: &str = "a None entry in the tracker's mission list";
/// The record of the current-playthrough query.
pub const RECORD_PLAYTHROUGH: &str = "docs/verification/NATIVE_PLAYTHROUGH_QUERY.md";
/// The controller class whose current-playthrough virtual was read.
pub const CONTROLLER_CLASS: &str = "WillowGame.WillowPlayerController";
/// The `NotYet` of a controller whose class is not exactly [`CONTROLLER_CLASS`] (NATIVE_PLAYTHROUGH_QUERY.md section
/// 4, step 1).
pub const CONTROLLER_OVERRIDE: &str = "a subclass override of the controller's current-playthrough query";
/// The `NotYet` of a replication info of a strict subclass of [`GRI_CLASS`] (section 4, step 5).
pub const GRI_OVERRIDE: &str = "a subclass override of the replication info's current-playthrough answer";
/// The record of the screen's calls (the note's section 3 names it for the text list).
pub const RECORD_GFX: &str = "docs/verification/NATIVE_GFX_BRIDGE.md";
/// The `NotYet` of the mission screen's text list update (section 3, step 1).
pub const TEXT_LIST: &str = "the mission screen's Flash text list (not hosted)";
/// The `NotYet` of `FocusOn` (section 3, step 2: its state is not answered "nothing").
pub const FOCUS: &str = "the 3D movie's Flash focus (not hosted)";
/// `WillowLeviathanService.RecordMissionStatusChangedEventForPlayer`.
pub const RECORD_STATUS_CHANGED: &str = "WillowGame.WillowLeviathanService.RecordMissionStatusChangedEventForPlayer";
/// `WillowLeviathanService.RecordMissionObjectiveChangedEventForPlayer`.
pub const RECORD_OBJECTIVE_CHANGED: &str =
    "WillowGame.WillowLeviathanService.RecordMissionObjectiveChangedEventForPlayer";
/// The record of the objective telemetry (ruling R3 applied to it).
pub const RECORD_OBJECTIVE_TELEMETRY: &str = "docs/verification/NATIVE_RECORD_MISSION_OBJECTIVE_CHANGED.md";
/// The record of the text list body, `GetVariableObject`'s no-player answer and the status telemetry.
pub const RECORD_TEXT_LIST: &str = "docs/verification/NATIVE_MISSION_TEXT_LIST.md";
/// The `NotYet` of a focus clamp whose Y bound is negative or NaN (the hub's ruling R2 on #504, its text).
pub const FOCUS_BOUND: &str = "WillowGFxMovie3D.FocusOn: the focus offset's clamp bound";
/// The layer the port lacks for [`TEXT_LIST`] and [`FOCUS`].
pub const TEXT_LIST_LAYER: &str = "a hosted Flash movie (the port hosts none)";

/// `WillowPlayerController.GetCurrentPlaythrough()`, the controller's current-playthrough virtual
/// (NATIVE_PLAYTHROUGH_QUERY.md section 4), in its order; the class tests are exact:
/// 1. a controller whose class is not exactly `WillowPlayerController`: [`CONTROLLER_OVERRIDE`], `NotYet { NotRead }`;
/// 2. and 3. no world, or no `WorldInfo` (the world's actor list holds none): 0;
/// 4. the `WorldInfo`'s `GRI` None, or not a `WillowGameReplicationInfo` (that class or a subclass): 0;
/// 5. a `GRI` of a strict subclass of it: [`GRI_OVERRIDE`], `NotYet { NotRead }`;
/// 6. the `GRI`'s `CurrentPlaythrough`, an int or a byte; any other kind is bad data, a `VmError::Script`.
///
/// It reads nothing of the controller but its class, so every exactly-`WillowPlayerController` controller gets the
/// same answer, and it writes nothing. The objects it reads are run-time objects, never store handles, so it loads
/// nothing. **UNVERIFIED** in the running game.
pub fn current_playthrough(vm: &Vm, controller: ObjRef) -> Result<i32, VmError> {
    const UNIT: &str = "WillowPlayerController.GetCurrentPlaythrough";
    let class = vm.space.heap.get(controller)?.class;
    if vm.space.classes.by_path(CONTROLLER_CLASS) != Some(class) {
        return Err(NotYet::not_read(CONTROLLER_OVERRIDE, RECORD_PLAYTHROUGH).into());
    }
    let world_info = match vm.service::<Actors>() {
        Some(a) => a.try_borrow().map_err(|_| VmError::ServiceBorrowed("Actors"))?.world_info(),
        None => None,
    };
    let Some(world_info) = world_info else { return Ok(0) };
    let Some(gri) = object_of(plain_field(vm, world_info, "GRI", UNIT)?, "WorldInfo.GRI", UNIT)? else {
        return Ok(0);
    };
    let Some(gri_class) = vm.space.classes.by_path(GRI_CLASS) else { return Ok(0) };
    let own = vm.space.heap.get(gri)?.class;
    if !vm.space.classes.is_child_of(own, gri_class) {
        return Ok(0);
    }
    if own != gri_class {
        return Err(NotYet::not_read(GRI_OVERRIDE, RECORD_PLAYTHROUGH).into());
    }
    int_of(&plain_field(vm, gri, "CurrentPlaythrough", UNIT)?, "CurrentPlaythrough", UNIT)
}

/// The value of a run-time object's `name` (no store load: these objects are never handles); a class that does not
/// declare it is bad data.
fn plain_field(vm: &Vm, o: ObjRef, name: &str, unit: &str) -> Result<Value, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm
        .space
        .prop(class, name)
        .ok_or_else(|| bad(unit, format!("{} declares no {name}", ow_attr::eval::class_path(&vm.space, o))))?;
    Ok(vm.space.read(o, PropAddr::scalar(p))?.clone())
}

/// The mission status numbers the scans grade (the note's status values).
const IN_PROGRESS: [u8; 3] = [1, 2, 3];
const COMPLETE: u8 = 4;

fn bad(unit: &str, message: String) -> VmError {
    VmError::Script { function: unit.to_string(), message }
}

/// The VM's count of the definition loads these natives made (review r2 of #480, L1): kept on the VM, so a load inside
/// a VM-dispatched callee (the fallback's own native) counts for the outer call too.
#[derive(Debug, Default)]
pub struct Loads(u64);

fn loads(vm: &Vm) -> u64 {
    vm.service::<Loads>().map_or(0, |l| l.borrow().0)
}

/// One native call: the VM's load count when it began (module documentation).
#[derive(Debug)]
struct Cx {
    start: u64,
}

impl Cx {
    fn new(vm: &Vm) -> Self {
        Cx { start: loads(vm) }
    }

    /// The native's result: a `NotYet` after a load made during the call, its own or a callee's, is marked once, kind
    /// and record kept; other errors unchanged.
    fn finish<T>(&self, vm: &Vm, r: Result<T, VmError>) -> Result<T, VmError> {
        let wrote = loads(vm) > self.start;
        r.map_err(|e| if wrote { super::datastore::after_writes(e) } else { e })
    }
}

/// The value of `o.name`, loading an unloaded store definition first (a write, counted on the VM and marked on a running
/// settings refresh); a class that does not declare it is bad data. A handle whose load already failed is not a new
/// write: its error comes back unchanged (review r2 of #480, L2).
fn field(vm: &mut Vm, _cx: &mut Cx, o: ObjRef, name: &str, unit: &str) -> Result<Value, VmError> {
    if vm.defs.is_unloaded(&vm.space, o) {
        if !vm.defs.is_failed(&vm.space, o) {
            super::datastore::mark_written(vm)?;
            let counter = match vm.service::<Loads>() {
                Some(c) => c,
                None => vm.add_service(Loads::default())?,
            };
            counter.try_borrow_mut().map_err(|_| VmError::ServiceBorrowed("Loads"))?.0 += 1;
        }
        let loaded = vm.defs.ensure_loaded(&mut vm.space, o);
        vm.log.extend(vm.defs.take_log());
        loaded?;
    }
    let class = vm.space.heap.get(o)?.class;
    let p = vm
        .space
        .prop(class, name)
        .ok_or_else(|| bad(unit, format!("{} declares no {name}", ow_attr::eval::class_path(&vm.space, o))))?;
    Ok(vm.space.read(o, PropAddr::scalar(p))?.clone())
}

fn object_of(v: Value, what: &str, unit: &str) -> Result<Option<ObjRef>, VmError> {
    match v {
        Value::Object(o) => Ok(o),
        other => Err(bad(unit, format!("{what} holds {}, not an object", other.kind_name()))),
    }
}

fn int_of(v: &Value, what: &str, unit: &str) -> Result<i32, VmError> {
    match v {
        Value::Int(i) => Ok(*i),
        Value::Byte(b) => Ok(i32::from(*b)),
        other => Err(bad(unit, format!("{what} holds {}, not an int", other.kind_name()))),
    }
}

fn byte_of(v: &Value, what: &str, unit: &str) -> Result<u8, VmError> {
    match v {
        Value::Byte(b) => Ok(*b),
        other => Err(bad(unit, format!("{what} holds {}, not a byte", other.kind_name()))),
    }
}

/// The three fields both scans read of a mission definition: whether it counts (`bPlotCritical` set, no
/// `DlcExpansion`), and its `MissionNumber`.
fn plot_mission(vm: &mut Vm, cx: &mut Cx, def: ObjRef, unit: &str) -> Result<Option<i32>, VmError> {
    let plot = match field(vm, cx, def, "bPlotCritical", unit)? {
        Value::Bool(b) => b,
        other => return Err(bad(unit, format!("bPlotCritical holds {}, not a bool", other.kind_name()))),
    };
    let dlc = object_of(field(vm, cx, def, "DlcExpansion", unit)?, "DlcExpansion", unit)?;
    let number = int_of(&field(vm, cx, def, "MissionNumber", unit)?, "MissionNumber", unit)?;
    Ok((plot && dlc.is_none()).then_some(number))
}

/// The scan both natives share: in progress answers at once, Complete keeps the largest, the end the largest or 0.
#[derive(Default)]
struct Scan {
    largest: Option<i32>,
}

impl Scan {
    /// `Some(number)` when the scan ends here.
    fn grade(&mut self, status: u8, number: i32) -> Option<i32> {
        if IN_PROGRESS.contains(&status) {
            return Some(number);
        }
        if status == COMPLETE && self.largest.is_none_or(|l| number > l) {
            self.largest = Some(number);
        }
        None
    }

    fn end(self) -> i32 {
        self.largest.unwrap_or(0)
    }
}

/// Calls `name` on `o` as its class sees it, or `None` when the class declares no such function.
fn call_method(vm: &mut Vm, o: ObjRef, name: &str, args: Vec<Value>) -> Result<Option<CallResult>, VmError> {
    let (class, state) = {
        let x = vm.space.heap.get(o)?;
        (x.class, x.state)
    };
    match vm.find_method(class, name, state)? {
        Some(f) => Ok(Some(vm.call(f, Some(o), args)?)),
        None => Ok(None),
    }
}

/// `call_method`, where the class must declare the function (a tracker without it is bad data).
fn call_required(vm: &mut Vm, o: ObjRef, name: &str, args: Vec<Value>, unit: &str) -> Result<CallResult, VmError> {
    call_method(vm, o, name, args)?.ok_or_else(|| {
        bad(unit, format!("{} declares no {name}", ow_attr::eval::class_path(&vm.space, o)))
    })
}

/// The world's mission tracker, or `None` when there is no world, replication info of that class or tracker (section
/// 1, step 1).
fn tracker(vm: &mut Vm, cx: &mut Cx, unit: &str) -> Result<Option<ObjRef>, VmError> {
    let world_info = match vm.service::<Actors>() {
        Some(a) => a.try_borrow().map_err(|_| VmError::ServiceBorrowed("Actors"))?.world_info(),
        None => None,
    };
    let Some(world_info) = world_info else { return Ok(None) };
    let Some(gri) = object_of(field(vm, cx, world_info, "GRI", unit)?, "WorldInfo.GRI", unit)? else {
        return Ok(None);
    };
    let Some(gri_class) = vm.space.classes.by_path(GRI_CLASS) else { return Ok(None) };
    if !vm.space.classes.is_child_of(vm.space.heap.get(gri)?.class, gri_class) {
        return Ok(None);
    }
    object_of(field(vm, cx, gri, "MissionTracker", unit)?, "MissionTracker", unit)
}

/// The tracker branch: `None` for the fallback (data not valid, or a -1 answer), else the number of steps 2 and 3.
fn tracker_branch(vm: &mut Vm, cx: &mut Cx, tracker: ObjRef, unit: &str) -> Result<Option<i32>, VmError> {
    let valid = call_required(vm, tracker, "IsDataValid", vec![], unit)?.ret;
    if !valid.truth(&vm.space.names) {
        return Ok(None);
    }
    let n = from_tracker(vm, cx, tracker, unit)?;
    Ok((n != -1).then_some(n))
}

/// Section 1, steps 2 and 3, on a tracker with valid data.
fn from_tracker(vm: &mut Vm, cx: &mut Cx, tracker: ObjRef, unit: &str) -> Result<i32, VmError> {
    if let Some(active) = object_of(field(vm, cx, tracker, "ActiveMission", unit)?, "ActiveMission", unit)? {
        return int_of(&field(vm, cx, active, "MissionNumber", unit)?, "MissionNumber", unit);
    }
    let listed = call_required(vm, tracker, "GetAllMissions", vec![Value::array(vec![])], unit)?;
    let list = listed.outs.first().and_then(Value::elements).map(<[Value]>::to_vec).ok_or_else(|| {
        bad(unit, "MissionTracker.GetAllMissions gave no mission array".to_string())
    })?;
    let mut scan = Scan::default();
    for entry in list {
        let Some(def) = object_of(entry, "a mission of the tracker's list", unit)? else {
            return Err(NotYet::not_read(NONE_ENTRY, RECORD).into());
        };
        let Some(number) = plot_mission(vm, cx, def, unit)? else { continue };
        let status = call_required(vm, tracker, "GetMissionStatus", vec![Value::Object(Some(def))], unit)?.ret;
        if let Some(n) = scan.grade(byte_of(&status, "GetMissionStatus", unit)?, number) {
            return Ok(n);
        }
    }
    Ok(scan.end())
}

/// `GetActiveMissionNumber` on `controller` (module documentation).
pub fn active_mission_number(vm: &mut Vm, controller: ObjRef) -> Result<i32, VmError> {
    let mut cx = Cx::new(vm);
    let r = active_with(vm, &mut cx, controller);
    cx.finish(vm, r)
}

fn active_with(vm: &mut Vm, cx: &mut Cx, controller: ObjRef) -> Result<i32, VmError> {
    const UNIT: &str = "WillowPlayerController.GetActiveMissionNumber";
    let Some(tracker) = tracker(vm, cx, UNIT)? else {
        return fallback(vm, controller, UNIT);
    };
    // The engine-wide condition holds (a world exists, or `tracker` would have answered None): the tracker branch
    // alone (NATIVE_PLAYTHROUGH_QUERY.md section 1).
    match tracker_branch(vm, cx, tracker, UNIT)? {
        Some(n) => Ok(n),
        None => fallback(vm, controller, UNIT),
    }
}

/// The fallback (section 1, step 4): the controller's `GetActivePlotCriticalMissionNumber(-1)`, dispatched through
/// the VM on its class; the callee's own answer, error included, is the answer.
fn fallback(vm: &mut Vm, controller: ObjRef, unit: &str) -> Result<i32, VmError> {
    let r = call_required(vm, controller, "GetActivePlotCriticalMissionNumber", vec![Value::Int(-1)], unit)?;
    int_of(&r.ret, "GetActivePlotCriticalMissionNumber", unit)
}

/// `GetActivePlotCriticalMissionNumber(index)` on `controller` (module documentation).
pub fn plot_critical_mission_number(vm: &mut Vm, controller: ObjRef, index: i32) -> Result<i32, VmError> {
    let mut cx = Cx::new(vm);
    let r = plot_with(vm, &mut cx, controller, index);
    cx.finish(vm, r)
}

fn plot_with(vm: &mut Vm, cx: &mut Cx, controller: ObjRef, index: i32) -> Result<i32, VmError> {
    const UNIT: &str = "WillowPlayerController.GetActivePlotCriticalMissionNumber";
    // A negative or omitted index: the controller's current-playthrough virtual, as a function (not a VM dispatch).
    let index = if index < 0 { current_playthrough(vm, controller)? } else { index };
    let all = field(vm, cx, controller, "MissionPlaythroughs", UNIT)?;
    let all = all.elements().ok_or_else(|| bad(UNIT, "MissionPlaythroughs is not an array".to_string()))?;
    let Some(playthrough) = usize::try_from(index).ok().and_then(|i| all.get(i)).cloned() else {
        return Ok(0);
    };
    let list = struct_field(vm, &playthrough, "MissionList", UNIT)?;
    let list = list.elements().map(<[Value]>::to_vec).ok_or_else(|| bad(UNIT, "MissionList is not an array".to_string()))?;
    let mut scan = Scan::default();
    for record in list {
        let Some(def) = object_of(struct_field(vm, &record, "MissionDef", UNIT)?, "MissionDef", UNIT)? else {
            continue;
        };
        let Some(number) = plot_mission(vm, cx, def, UNIT)? else { continue };
        let status = byte_of(&struct_field(vm, &record, "Status", UNIT)?, "Status", UNIT)?;
        if let Some(n) = scan.grade(status, number) {
            return Ok(n);
        }
    }
    Ok(scan.end())
}

/// Field `name` of the struct value `v`, or bad data.
fn struct_field(vm: &Vm, v: &Value, name: &str, unit: &str) -> Result<Value, VmError> {
    let missing = || bad(unit, format!("a record without the field {name}"));
    let s = v.as_struct().ok_or_else(missing)?;
    let id = vm.space.names.find(name).ok_or_else(missing)?;
    let (index, _) = vm.space.classes.struct_layout(s.ty).and_then(|l| l.field(id)).ok_or_else(missing)?;
    let index = usize::try_from(index).map_err(|_| missing())?;
    s.fields.get(index).cloned().ok_or_else(missing)
}

pub(crate) fn n_active_mission_number(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let me = c.this.ok_or_else(|| bad(ACTIVE_MISSION_NUMBER, "called with no controller".to_string()))?;
    Ok(Value::Int(active_mission_number(c.vm, me)?))
}

pub(crate) fn n_plot_critical_number(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let me = c.this.ok_or_else(|| bad(PLOT_CRITICAL_NUMBER, "called with no controller".to_string()))?;
    let index = if c.has(0) { int_of(c.arg(0)?, "PlaythroughIndex", PLOT_CRITICAL_NUMBER)? } else { -1 };
    Ok(Value::Int(plot_critical_mission_number(c.vm, me, index)?))
}

/// The screen's named `NotYet` of `what`. The native writes nothing, so it is not marked here (module documentation).
fn screen_not_yet(what: &'static str) -> Result<Value, VmError> {
    Err(NotYet::layer_absent(what, TEXT_LIST_LAYER, RECORD_GFX).into())
}

pub(crate) fn n_update_mission_text_list(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const UNIT: &str = UPDATE_MISSION_TEXT_LIST;
    let movie = c.this.ok_or_else(|| bad(UNIT, "called with no movie".to_string()))?;
    let class = c.vm.space.heap.get(movie)?.class;
    for name in ["MissionTextList", "MissionCategories"] {
        if c.vm.space.prop(class, name).is_none() {
            return Err(bad(UNIT, format!("{} declares no {name}", ow_attr::eval::class_path(&c.vm.space, movie))));
        }
    }
    let p = c.vm.space.prop(class, "MissionTextList").expect("checked above");
    let list = object_of(c.vm.space.read(movie, PropAddr::scalar(p))?.clone(), "MissionTextList", UNIT)?;
    match list {
        // The read None branch: nothing more is read, nothing is written (NATIVE_MISSION_TEXT_LIST.md section 2).
        None => Ok(Value::None),
        Some(_) => screen_not_yet(TEXT_LIST),
    }
}

/// `GetVariableObject` on a movie with no native player: None, no write (module documentation).
pub(crate) fn n_movie_query(_c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    Ok(Value::Object(None))
}

/// The status telemetry (module documentation): nothing, with no write, for every player (ruling R3).
pub(crate) fn n_status_changed_telemetry(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    object_of(c.arg(0)?.clone(), "ForPlayer", RECORD_STATUS_CHANGED)?;
    Ok(Value::None)
}

/// The objective telemetry (module documentation): nothing, with no write, for every player (ruling R3, per
/// NATIVE_RECORD_MISSION_OBJECTIVE_CHANGED.md section 3).
pub(crate) fn n_objective_changed_telemetry(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    if !declared_at(c, RECORD_OBJECTIVE_CHANGED) {
        return outside_declaration(c);
    }
    object_of(c.arg(0)?.clone(), "ForPlayer", RECORD_OBJECTIVE_CHANGED)?;
    Ok(Value::None)
}

/// Whether the native being called is the declaration at `path` (compared case-insensitively), not a same-key native
/// of another package (review r1 of #533, item 1).
fn declared_at(c: &NativeCall<'_>, path: &str) -> bool {
    c.vm.function(c.function).is_some_and(|d| d.path.as_str().eq_ignore_ascii_case(path))
}

/// A same-key native of another declaration: the unbound behaviour (module documentation). Nothing is written.
fn outside_declaration(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let key = c.vm.function(c.function).and_then(|d| d.native_key.clone()).unwrap_or_default();
    if !c.vm.stub_policy.allows_stub(&key) {
        return Err(VmError::UnboundNative(key));
    }
    c.vm.log.push(format!("UNIMPLEMENTED {key}"));
    Ok(Value::None)
}

/// The stage the focus clamp reads (NATIVE_MISSION_TEXT_LIST.md section 4, step 3): the native player's view height
/// and the scale. The port's movies have no native player, so the port's stand-in is [`FocusStage::NO_PLAYER`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FocusStage {
    pub height: f32,
    pub scale: f32,
}

impl FocusStage {
    /// The stand-in for a movie with no native player: height 0 and scale 0, so the bound is 0 (ruling R2).
    pub const NO_PLAYER: FocusStage = FocusStage { height: 0.0, scale: 0.0 };
}

/// The focus clamp's Y bound on `stage`: half the margin between the stage height and the stage height times the
/// scale. A negative or NaN bound (a garbage height, or a scale above 1) is [`FOCUS_BOUND`], `NotYet { NotRead }`
/// (ruling R2).
pub fn y_bound(stage: FocusStage) -> Result<f32, VmError> {
    let bound = (stage.height - stage.height * stage.scale) * 0.5;
    if bound.is_nan() || bound < 0.0 {
        return Err(NotYet::not_read(FOCUS_BOUND, RECORD_TEXT_LIST).into());
    }
    Ok(bound)
}

/// The stage `FocusOn` reads: the no-player stand-in, or in a test the [`TestFocusStage`] service's.
fn stage_for(vm: &Vm) -> FocusStage {
    #[cfg(test)]
    if let Some(s) = vm.service::<TestFocusStage>() {
        return s.borrow().0;
    }
    let _ = vm;
    FocusStage::NO_PLAYER
}

/// Tests only: the stage `FocusOn` reads.
#[cfg(test)]
pub(crate) struct TestFocusStage(pub FocusStage);

/// The zero `Vector` of struct `ty`: its defaults with float `X`, `Y` and `Z` each 0; a struct that does not declare
/// all three as floats is a `VmError::Script`.
fn zero_vector(vm: &Vm, ty: ow_object::ids::StructId) -> Result<Value, VmError> {
    const UNIT: &str = FOCUS_ON;
    let mut v = vm.space.classes.struct_defaults(ty).map_err(|e| bad(UNIT, format!("FocusOffset's defaults: {e:?}")))?;
    let layout = vm.space.classes.struct_layout(ty).ok_or_else(|| bad(UNIT, "FocusOffset has no layout".to_string()))?;
    for name in ["X", "Y", "Z"] {
        let field = vm.space.names.find(name).and_then(|id| layout.field(id));
        let index = match field {
            Some((index, d)) if matches!(d.kind, PropKind::Float) => index,
            _ => return Err(bad(UNIT, format!("FocusOffset does not declare {name} as a float"))),
        };
        let index = usize::try_from(index).map_err(|_| bad(UNIT, format!("FocusOffset's {name} is out of range")))?;
        let s = v.struct_mut().ok_or_else(|| bad(UNIT, "FocusOffset's defaults are not a struct".to_string()))?;
        *s.fields.get_mut(index).ok_or_else(|| bad(UNIT, format!("FocusOffset's {name} is out of range")))? =
            Value::Float(0.0);
    }
    Ok(v)
}

/// `FocusOn(None)`'s writes on `movie`, in order, planned with nothing written (module documentation): `FocusObject`
/// = None, then `FocusOffset` = the zero `Vector`, after the declarations, the clamp's bound and both destination
/// slots on the receiver are checked.
pub(crate) fn focus_writes(vm: &Vm, movie: ObjRef) -> Result<[(PropAddr, Value); 2], VmError> {
    const UNIT: &str = FOCUS_ON;
    let class = vm.space.heap.get(movie)?.class;
    let declared = |name: &str| {
        let p = vm
            .space
            .prop(class, name)
            .ok_or_else(|| bad(UNIT, format!("{} declares no {name}", ow_attr::eval::class_path(&vm.space, movie))))?;
        let kind = vm.space.classes.decl(p).map(|d| d.kind.clone());
        Ok::<_, VmError>((p, kind))
    };
    let (object_field, kind) = declared("FocusObject")?;
    if !matches!(kind, Some(PropKind::Object { .. })) {
        return Err(bad(UNIT, "FocusObject is not declared as an object".to_string()));
    }
    let (offset_field, kind) = declared("FocusOffset")?;
    let Some(PropKind::Struct(ty)) = kind else {
        return Err(bad(UNIT, "FocusOffset is not declared as a struct".to_string()));
    };
    let offset = zero_vector(vm, ty)?;
    // The clamp: Y within the bound. With no `FocusObject` the offset is not centred, so Y stays 0 for any valid bound.
    y_bound(stage_for(vm))?;
    let writes = [(PropAddr::scalar(object_field), Value::Object(None)), (PropAddr::scalar(offset_field), offset)];
    // Both destination slots must exist on the receiver (review r2 of #509): a read of each, so a slot missing from
    // the object is the heap's own error before the first write, not after it.
    for (at, _) in &writes {
        vm.space.read(movie, *at)?;
    }
    Ok(writes)
}

/// `FocusOn` (module documentation, ruling R2): a set `Thing` refuses at once; otherwise the planned writes, in order.
pub(crate) fn n_focus(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const UNIT: &str = FOCUS_ON;
    let movie = c.this.ok_or_else(|| bad(UNIT, "called with no movie".to_string()))?;
    if object_of(c.arg(0)?.clone(), "Thing", UNIT)?.is_some() {
        return screen_not_yet(FOCUS);
    }
    for (at, v) in focus_writes(c.vm, movie)? {
        #[cfg(test)]
        if let Some(log) = c.vm.service::<TestFocusWrites>() {
            log.borrow_mut().0.push(at);
        }
        c.vm.space.write(movie, at, v)?;
    }
    Ok(Value::None)
}

/// Tests only: the addresses `FocusOn` writes, in the order it writes them.
#[cfg(test)]
pub(crate) struct TestFocusWrites(pub Vec<PropAddr>);

#[cfg(test)]
#[path = "mission_numbers_tests.rs"]
mod tests;
