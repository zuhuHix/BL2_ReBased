//! Owner: lane IMPL-MISSION-NUMBERS (decision 2026-11-19-93). The two mission numbers of the online game settings
//! refresh, and the mission screen's calls the accept reaches after it. AI-assisted (Claude Code).
//!
//! Coded from NATIVE_MISSION_NUMBERS.md alone (#473 at f586ab9c, its review r2, the final round; analyst lane
//! ANALYST-MISSION-NUMBERS, decision 2026-11-19-86). No
//! native code or analysis output was opened. **Every rule is UNVERIFIED in the running game.** Off only: the caller
//! (`datastore::register`) binds nothing here under `ParityMode::Reference`, so the VM's stub policy answers there.
//!
//! **The conservative form** (the hub's heads-up of 2026-11-19, while #473's last review round runs): the parts of the
//! note that rest on an unresolved read, or on a "no read-back" argument, answer a named `NotYet` in Off. [`Reads`]
//! switches the two unresolved reads; the natives pass [`Reads::OFF`] (nothing taken as read), and the tests also run
//! the rules with them taken as read.
//!
//! **`WillowPlayerController.GetActiveMissionNumber()`** (section 1). It writes nothing, so nothing is preflighted and
//! nothing rolled back.
//! 1. The world's `WorldInfo` (the level's, as `GetWorldInfo` answers), its `GRI` as a `WillowGameReplicationInfo`,
//!    and that object's `MissionTracker`. With no world, a replication info of another class, or no tracker, the
//!    answer is the fallback (step 4).
//!    - **The engine-wide condition** is a global the note did not resolve (open question 1), so Off does not guess
//!      it: with a tracker, both branches are computed, the tracker branch (below) first, then the fallback. Equal
//!      answers give that number; different answers are [`ENGINE_GATE`], `NotYet { NotRead }`. A branch that fails is
//!      the answer, its error unchanged. ([`Reads::engine_gate`] takes the condition as holding: the tracker branch
//!      alone.)
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
//! 1. A negative or omitted index needs the controller's current-playthrough virtual, which was not read and whose
//!    identity is not guessed (open question 2): [`PLAYTHROUGH_QUERY`], `NotYet { NotRead }`. An explicit index of 0
//!    or more does not ask it. (Only the tests take it as read, as `GetCurrentPlaythrough()` through the VM, to run
//!    the rest of the rule.)
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
//! - **`QuestAcceptGFxMovie.UpdateMissionTextList()`:** [`TEXT_LIST`], `NotYet { LayerAbsent }` (record
//!   [`RECORD_GFX`]), whether or not `MissionTextList` is set: the note's read-back check does not cover the 10
//!   undecoded script functions or native readers, so no call here is answered "nothing".
//! - **The calls `SetFocus` makes next** are not reached while the text list refuses. If they are: `GetVariableObject`
//!   is [`MOVIE_QUERY`] and `WillowGFxMovie3D.FocusOn` is [`FOCUS`], each `NotYet { LayerAbsent }`, never None or
//!   nothing.
//! - **The partial-writes mark** (review r1 of #480, item 2): these natives write nothing, so they do not mark their
//!   own `NotYet`. The mark is applied where a write is known: the mission bridge's accept and turn-in mark a `NotYet`
//!   that leaves the controller's script after the mission's status changed (`ow_mission::bridge`). A direct call on
//!   a fresh movie is not marked.
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
/// The `NotYet` of the unresolved engine-wide condition of `GetActiveMissionNumber`, when its two branches differ
/// (section 1, open question 1).
pub const ENGINE_GATE: &str = "the engine-wide condition of GetActiveMissionNumber";
/// The `NotYet` of a `None` entry in the tracker's mission list (section 1, Off step 5).
pub const NONE_ENTRY: &str = "a None entry in the tracker's mission list";
/// The `NotYet` of the controller's unresolved current-playthrough virtual (section 2, open question 2).
pub const PLAYTHROUGH_QUERY: &str = "the controller's current-playthrough virtual";
/// The record of the screen's calls (the note's section 3 names it for the text list).
pub const RECORD_GFX: &str = "docs/verification/NATIVE_GFX_BRIDGE.md";
/// The `NotYet` of the mission screen's text list update (section 3, step 1).
pub const TEXT_LIST: &str = "the mission screen's Flash text list (not hosted)";
/// The `NotYet` of `GetVariableObject` (section 3, step 2).
pub const MOVIE_QUERY: &str = "a Flash movie query (not hosted)";
/// The `NotYet` of `FocusOn` (section 3, step 2: its state is not answered "nothing").
pub const FOCUS: &str = "the 3D movie's Flash focus (not hosted)";
/// The layer the port lacks for [`TEXT_LIST`] and [`FOCUS`].
pub const TEXT_LIST_LAYER: &str = "a hosted Flash movie (the port hosts none)";

/// Which of the note's unresolved reads are taken as read. The natives pass [`Reads::OFF`]; the tests also run the
/// rules behind them with [`Reads::ALL`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reads {
    /// The engine-wide condition of `GetActiveMissionNumber` taken as holding (section 1, step 1): the tracker branch
    /// alone. Off computes both branches instead.
    pub engine_gate: bool,
    /// The controller's current-playthrough virtual, read as `GetCurrentPlaythrough` (section 2, step 1).
    pub playthrough: bool,
}

impl Reads {
    pub const OFF: Reads = Reads { engine_gate: false, playthrough: false };
    pub const ALL: Reads = Reads { engine_gate: true, playthrough: true };
}

/// The mission status numbers the scans grade (the note's status values).
const IN_PROGRESS: [u8; 3] = [1, 2, 3];
const COMPLETE: u8 = 4;

fn bad(unit: &str, message: String) -> VmError {
    VmError::Script { function: unit.to_string(), message }
}

/// What one native call did: whether it loaded a store definition (a write; module documentation).
#[derive(Debug, Default)]
struct Cx {
    wrote: bool,
}

impl Cx {
    /// The native's result: a `NotYet` after a load is marked once, kind and record kept; other errors unchanged.
    fn finish<T>(&self, r: Result<T, VmError>) -> Result<T, VmError> {
        r.map_err(|e| if self.wrote { super::datastore::after_writes(e) } else { e })
    }
}

/// The value of `o.name`, loading an unloaded store definition first (a write, noted on `cx` and on a running settings
/// refresh); a class that does not declare it is bad data.
fn field(vm: &mut Vm, cx: &mut Cx, o: ObjRef, name: &str, unit: &str) -> Result<Value, VmError> {
    if vm.defs.is_unloaded(&vm.space, o) {
        cx.wrote = true;
        super::datastore::mark_written(vm)?;
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

/// `GetActiveMissionNumber` on `controller` (module documentation), with `reads` taken as read.
pub fn active_mission_number(vm: &mut Vm, controller: ObjRef, reads: Reads) -> Result<i32, VmError> {
    let mut cx = Cx::default();
    let r = active_with(vm, &mut cx, controller, reads);
    cx.finish(r)
}

fn active_with(vm: &mut Vm, cx: &mut Cx, controller: ObjRef, reads: Reads) -> Result<i32, VmError> {
    const UNIT: &str = "WillowPlayerController.GetActiveMissionNumber";
    let Some(tracker) = tracker(vm, cx, UNIT)? else {
        return fallback(vm, controller, UNIT);
    };
    let Some(n) = tracker_branch(vm, cx, tracker, UNIT)? else {
        return fallback(vm, controller, UNIT);
    };
    if reads.engine_gate {
        return Ok(n);
    }
    let other = fallback(vm, controller, UNIT)?;
    if n == other { Ok(n) } else { Err(NotYet::not_read(ENGINE_GATE, RECORD).into()) }
}

/// The fallback (section 1, step 4): the controller's `GetActivePlotCriticalMissionNumber(-1)`, dispatched through
/// the VM on its class; the callee's own answer, error included, is the answer.
fn fallback(vm: &mut Vm, controller: ObjRef, unit: &str) -> Result<i32, VmError> {
    let r = call_required(vm, controller, "GetActivePlotCriticalMissionNumber", vec![Value::Int(-1)], unit)?;
    int_of(&r.ret, "GetActivePlotCriticalMissionNumber", unit)
}

/// `GetActivePlotCriticalMissionNumber(index)` on `controller` (module documentation), with `reads` taken as read.
pub fn plot_critical_mission_number(vm: &mut Vm, controller: ObjRef, index: i32, reads: Reads) -> Result<i32, VmError> {
    let mut cx = Cx::default();
    let r = plot_with(vm, &mut cx, controller, index, reads);
    cx.finish(r)
}

fn plot_with(vm: &mut Vm, cx: &mut Cx, controller: ObjRef, index: i32, reads: Reads) -> Result<i32, VmError> {
    const UNIT: &str = "WillowPlayerController.GetActivePlotCriticalMissionNumber";
    let index = if index < 0 {
        if !reads.playthrough {
            return Err(NotYet::not_read(PLAYTHROUGH_QUERY, RECORD).into());
        }
        let Some(answer) = call_method(vm, controller, "GetCurrentPlaythrough", vec![])? else {
            return Err(NotYet::not_read(PLAYTHROUGH_QUERY, RECORD).into());
        };
        int_of(&answer.ret, "GetCurrentPlaythrough", UNIT)?
    } else {
        index
    };
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

/// The reads the bound natives take: nothing in the port ([`Reads::OFF`]). A test may switch them with a
/// [`TestReads`] service, so the rules behind them run through the VM's dispatch too.
fn reads_for(vm: &Vm) -> Reads {
    #[cfg(test)]
    if let Some(r) = vm.service::<TestReads>() {
        return r.borrow().0;
    }
    let _ = vm;
    Reads::OFF
}

/// Tests only: the reads the bound natives take.
#[cfg(test)]
pub(crate) struct TestReads(pub Reads);

pub(crate) fn n_active_mission_number(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let me = c.this.ok_or_else(|| bad(ACTIVE_MISSION_NUMBER, "called with no controller".to_string()))?;
    let reads = reads_for(c.vm);
    Ok(Value::Int(active_mission_number(c.vm, me, reads)?))
}

pub(crate) fn n_plot_critical_number(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let me = c.this.ok_or_else(|| bad(PLOT_CRITICAL_NUMBER, "called with no controller".to_string()))?;
    let index = if c.has(0) { int_of(c.arg(0)?, "PlaythroughIndex", PLOT_CRITICAL_NUMBER)? } else { -1 };
    let reads = reads_for(c.vm);
    Ok(Value::Int(plot_critical_mission_number(c.vm, me, index, reads)?))
}

/// The screen's named `NotYet` of `what`. The native writes nothing, so it is not marked here (module documentation).
fn screen_not_yet(what: &'static str) -> Result<Value, VmError> {
    Err(NotYet::layer_absent(what, TEXT_LIST_LAYER, RECORD_GFX).into())
}

pub(crate) fn n_update_mission_text_list(_c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    screen_not_yet(TEXT_LIST)
}

pub(crate) fn n_movie_query(_c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    screen_not_yet(MOVIE_QUERY)
}

pub(crate) fn n_focus(_c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    screen_not_yet(FOCUS)
}

#[cfg(test)]
#[path = "mission_numbers_tests.rs"]
mod tests;
