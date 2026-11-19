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
//! - **`WillowGFxMovie3D.FocusOn(Thing)`** (section 4, with the hub's ruling R2 on #504; UNVERIFIED). The movie's
//!   class must declare `FocusObject` (an object) and `FocusOffset` (a struct with float `X` and `Y`), else a
//!   `VmError::Script` naming the field; this is checked before any write.
//!   - **`Thing` None, on a movie with no native player** (every port movie): `FocusObject` = None, then
//!     `FocusOffset` = (0, 0), and no other write. The game's clamp limits Y to plus or minus a bound made from the
//!     stage height; the port's movie has no view, so its stage height is 0 and the bound is 0. A bound that came out
//!     negative or NaN would be [`FOCUS_BOUND`], `NotYet { NotRead }`, before any write ([`y_bound`]).
//!   - **Any other `Thing`:** [`FOCUS`], `NotYet { LayerAbsent }`: the chain step's Flash lookups need a hosted movie.
//! - **`WillowLeviathanService.RecordMissionStatusChangedEventForPlayer(ForPlayer, Status, MissionDefinition)`**
//!   (section 5, with the hub's ruling R3 on #504): the shared statistics submit routine answers nothing, with no write,
//!   for every player. A port answer, the same as the revived event's (#500); UNVERIFIED. The player argument must be
//!   an object.
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
/// The record of the text list body, `GetVariableObject`'s no-player answer and the status telemetry.
pub const RECORD_TEXT_LIST: &str = "docs/verification/NATIVE_MISSION_TEXT_LIST.md";
/// The `NotYet` of a focus clamp whose Y bound is negative or NaN (the hub's ruling R2 on #504).
pub const FOCUS_BOUND: &str = "WillowGFxMovie3D.FocusOn: a negative or NaN focus clamp bound";
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

/// The focus clamp's Y bound for a movie with no native player: half the margin between the stage height and the
/// stage height times a scale. The port's movie has no view, so the stage height is 0 and the bound is 0, whatever the
/// (unread) scale. A negative or NaN bound is [`FOCUS_BOUND`] (ruling R2).
pub fn y_bound(stage_height: f32) -> Result<f32, VmError> {
    if stage_height != 0.0 {
        // The scale's source was not read (NATIVE_MISSION_TEXT_LIST.md open question 3); a port movie never gets here.
        return Err(NotYet::not_read(FOCUS_BOUND, RECORD_TEXT_LIST).into());
    }
    let bound = 0.0_f32;
    if bound.is_nan() || bound < 0.0 {
        return Err(NotYet::not_read(FOCUS_BOUND, RECORD_TEXT_LIST).into());
    }
    Ok(bound)
}

/// `FocusOffset`'s current value with `X` and `Y` set to `x` and `y`, checked as a struct with float `X` and `Y`.
fn focus_offset(vm: &Vm, current: &Value, x: f32, y: f32) -> Result<Value, VmError> {
    const UNIT: &str = FOCUS_ON;
    let mut v = current.clone();
    let s = v.as_struct().ok_or_else(|| bad(UNIT, format!("FocusOffset holds {}, not a struct", current.kind_name())))?;
    let layout = vm.space.classes.struct_layout(s.ty).ok_or_else(|| bad(UNIT, "FocusOffset has no layout".to_string()))?;
    let mut at = Vec::new();
    for name in ["X", "Y"] {
        let id = vm.space.names.find(name).ok_or_else(|| bad(UNIT, format!("FocusOffset has no {name}")))?;
        let (index, _) = layout.field(id).ok_or_else(|| bad(UNIT, format!("FocusOffset has no {name}")))?;
        let index = usize::try_from(index).map_err(|_| bad(UNIT, format!("FocusOffset's {name} is out of range")))?;
        match s.fields.get(index) {
            Some(Value::Float(_)) => at.push(index),
            other => return Err(bad(UNIT, format!("FocusOffset's {name} holds {other:?}, not a float"))),
        }
    }
    let s = v.struct_mut().expect("checked above");
    s.fields[at[0]] = Value::Float(x);
    s.fields[at[1]] = Value::Float(y);
    Ok(v)
}

/// `FocusOn` (module documentation, ruling R2): the declarations, then the case; the two writes come last.
pub(crate) fn n_focus(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const UNIT: &str = FOCUS_ON;
    let movie = c.this.ok_or_else(|| bad(UNIT, "called with no movie".to_string()))?;
    let thing = object_of(c.arg(0)?.clone(), "Thing", UNIT)?;
    let class = c.vm.space.heap.get(movie)?.class;
    let declared = |name: &str| {
        c.vm.space
            .prop(class, name)
            .ok_or_else(|| bad(UNIT, format!("{} declares no {name}", ow_attr::eval::class_path(&c.vm.space, movie))))
    };
    let object_field = declared("FocusObject")?;
    let offset_field = declared("FocusOffset")?;
    if !matches!(c.vm.space.read(movie, PropAddr::scalar(object_field))?, Value::Object(_)) {
        return Err(bad(UNIT, "FocusObject is not an object field".to_string()));
    }
    let current = c.vm.space.read(movie, PropAddr::scalar(offset_field))?.clone();
    if thing.is_some() {
        return screen_not_yet(FOCUS);
    }
    let bound = y_bound(0.0)?;
    let offset = focus_offset(c.vm, &current, 0.0, 0.0_f32.clamp(-bound, bound))?;
    c.vm.space.write(movie, PropAddr::scalar(object_field), Value::Object(None))?;
    c.vm.space.write(movie, PropAddr::scalar(offset_field), offset)?;
    Ok(Value::None)
}

#[cfg(test)]
#[path = "mission_numbers_tests.rs"]
mod tests;
