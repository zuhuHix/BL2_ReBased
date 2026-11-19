//! Owner: lane IMPL-MISSION-NUMBERS (decision 2026-11-19-93). The two mission numbers of the online game settings
//! refresh, and the mission screen's text list the accept reaches after it. AI-assisted (Claude Code).
//!
//! Coded from NATIVE_MISSION_NUMBERS.md alone (analyst lane ANALYST-MISSION-NUMBERS, decision 2026-11-19-86). No
//! native code or analysis output was opened. **Every rule is UNVERIFIED in the running game.** Off only: the caller
//! (`datastore::register`) binds nothing here under `ParityMode::Reference`, so the VM's stub policy answers there.
//!
//! **`WillowPlayerController.GetActiveMissionNumber()`** (section 1). It writes nothing, so nothing is preflighted and
//! nothing rolled back.
//! 1. The world's `WorldInfo` (the level's, as `GetWorldInfo` answers), its `GRI` as a `WillowGameReplicationInfo`,
//!    and that object's `MissionTracker`. The tracker's `IsDataValid()` is called through the VM. No world, a
//!    replication info of another class, no tracker, or data not valid: the fallback (step 4). The note's engine-wide
//!    condition (an unresolved global, read as "the game is running") is taken to hold in a running port world
//!    (UNVERIFIED, the note's open question 1).
//! 2. The tracker's `ActiveMission` set: that definition's `MissionNumber`, with no status or plot-critical check.
//! 3. Otherwise the tracker's `GetAllMissions(out)` list, through the VM, in its order. A definition with
//!    `bPlotCritical` and no `DlcExpansion` has its `GetMissionStatus` asked through the VM. In progress (1, 2, 3)
//!    answers its `MissionNumber` at once. Complete (4) keeps the largest. The end answers the largest kept, else 0.
//! 4. The fallback, `GetActivePlotCriticalMissionNumber` for the current playthrough, is taken when step 1 fails or
//!    steps 2 and 3 answer -1.
//!
//! **`WillowPlayerController.GetActivePlotCriticalMissionNumber(optional int PlaythroughIndex)`** (section 2):
//! 1. A negative or omitted index is the controller's `GetCurrentPlaythrough()`, called through the VM. A controller
//!    whose class declares no such function answers [`PLAYTHROUGH_QUERY`] (`NotYet { NotRead }`): the note reads the
//!    virtual as that query, UNVERIFIED.
//! 2. Out of range (negative, or not below the length of `MissionPlaythroughs`): 0.
//! 3. That playthrough's `MissionList`, in order. A record with a `MissionDef` that has `bPlotCritical` and no
//!    `DlcExpansion` is graded by its own `Status` field. In progress answers its `MissionNumber` at once, Complete
//!    keeps the largest, and the end answers the largest kept, else 0.
//!
//! **Calls and data.** A tracker or controller function the VM does not bind is the VM's own answer (its `NotYet` or
//! refusal), propagated unchanged. Inside the settings refresh, the refresh's write mark (decision 2026-11-19-80) turns
//! such a `NotYet` into a terminal stop after partial writes. A definition or record that lacks one of the fields read
//! here, or holds the wrong kind there, is bad data: a `VmError::Script` naming the field. A `None` entry in the
//! tracker's list is bad data too (the note grades definitions only).
//!
//! **`QuestAcceptGFxMovie.UpdateMissionTextList()`** (section 3; the slice's accept reaches it through the
//! controller's `ClientUpdateUIMissionList`, the movie's `UpdateMissionList` and `extPopulateQuestEntries`):
//! - With the movie's `MissionTextList` `None` it returns and changes nothing (the record's first step). That is the
//!   port's case: the mission bridge's screen is a stand-in made from class defaults, and the movie's `Start`, the
//!   only script seen to make the list, is not run. This is a **port Off answer for a headless world with no Flash
//!   movie hosted**, UNVERIFIED (hub ruling of 2026-11-19 on UI-only side effects). It is read on the live object at
//!   the call, so nothing is preflighted.
//! - **The read-back check** (the note's, over every decoded script function of the code packages): the native's
//!   fields are the movie's `MissionCategories` and `MissionTextList`. No script reads `MissionCategories`.
//!   `MissionTextList` is read only by the movie's own script; the one reader links 10 to 12 can reach,
//!   `GetSelectedIndex` (from the turn-in's `ClientSpawnMissionRewardUI`), reads its `HighlightedEntry`, which is None
//!   with the list None whether or not the native ran. So nothing on the slice path reads back what the game would have
//!   changed. Not covered: the 10 script functions the decoder cannot read (the note's open question 5).
//! - With `MissionTextList` set (not the port's case) it answers [`TEXT_LIST`] (`NotYet { LayerAbsent }`), before
//!   the native writes anything. It comes after the accept's own writes, so it is terminal: marked "stopped after
//!   partial writes" ([`super::datastore::PARTIAL_WRITES`]), its kind kept, nothing rolled back (the note's step 3).
//! - Then `SetFocus` asks `GFxMoviePlayer.GetVariableObject` and `WillowGFxMovie3D.FocusOn`: the note's Off answers
//!   for them (None, nothing) are not bound here. They stay Reference-only stubs, so in Off they would be the next
//!   stop once the accept gets this far.

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
/// The `NotYet` of a controller whose class declares no `GetCurrentPlaythrough` (section 2, Off step 1).
pub const PLAYTHROUGH_QUERY: &str = "the controller's current playthrough query";
/// The `NotYet` of a mission screen whose text list is set (section 3, Off step 3).
pub const TEXT_LIST: &str = "the mission screen's Flash text list (not hosted)";
/// The layer the port lacks for [`TEXT_LIST`].
pub const TEXT_LIST_LAYER: &str = "a hosted Flash movie (the port hosts none)";

/// The mission status numbers the scans grade (the note's status values).
const IN_PROGRESS: [u8; 3] = [1, 2, 3];
const COMPLETE: u8 = 4;

fn bad(unit: &str, message: String) -> VmError {
    VmError::Script { function: unit.to_string(), message }
}

/// The value of `o.name`, loading a store definition first; a class that does not declare it is bad data.
fn field(vm: &mut Vm, o: ObjRef, name: &str, unit: &str) -> Result<Value, VmError> {
    if !vm.space.heap.contains(o) {
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
fn plot_mission(vm: &mut Vm, def: ObjRef, unit: &str) -> Result<Option<i32>, VmError> {
    let plot = match field(vm, def, "bPlotCritical", unit)? {
        Value::Bool(b) => b,
        other => return Err(bad(unit, format!("bPlotCritical holds {}, not a bool", other.kind_name()))),
    };
    let dlc = object_of(field(vm, def, "DlcExpansion", unit)?, "DlcExpansion", unit)?;
    let number = int_of(&field(vm, def, "MissionNumber", unit)?, "MissionNumber", unit)?;
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

/// The world's mission tracker with valid data, or `None` for the fallback (section 1, step 1).
fn tracker(vm: &mut Vm, unit: &str) -> Result<Option<ObjRef>, VmError> {
    let world_info = match vm.service::<Actors>() {
        Some(a) => a.try_borrow().map_err(|_| VmError::ServiceBorrowed("Actors"))?.world_info(),
        None => None,
    };
    let Some(world_info) = world_info else { return Ok(None) };
    let Some(gri) = object_of(field(vm, world_info, "GRI", unit)?, "WorldInfo.GRI", unit)? else {
        return Ok(None);
    };
    let Some(gri_class) = vm.space.classes.by_path(GRI_CLASS) else { return Ok(None) };
    if !vm.space.classes.is_child_of(vm.space.heap.get(gri)?.class, gri_class) {
        return Ok(None);
    }
    let Some(tracker) = object_of(field(vm, gri, "MissionTracker", unit)?, "MissionTracker", unit)? else {
        return Ok(None);
    };
    let valid = call_required(vm, tracker, "IsDataValid", vec![], unit)?.ret;
    Ok(valid.truth(&vm.space.names).then_some(tracker))
}

/// Section 1, steps 2 and 3, on a tracker with valid data.
fn from_tracker(vm: &mut Vm, tracker: ObjRef, unit: &str) -> Result<i32, VmError> {
    if let Some(active) = object_of(field(vm, tracker, "ActiveMission", unit)?, "ActiveMission", unit)? {
        return int_of(&field(vm, active, "MissionNumber", unit)?, "MissionNumber", unit);
    }
    let listed = call_required(vm, tracker, "GetAllMissions", vec![Value::array(vec![])], unit)?;
    let list = listed.outs.first().and_then(Value::elements).map(<[Value]>::to_vec).ok_or_else(|| {
        bad(unit, "MissionTracker.GetAllMissions gave no mission array".to_string())
    })?;
    let mut scan = Scan::default();
    for entry in list {
        let Some(def) = object_of(entry, "a mission of the tracker's list", unit)? else {
            return Err(bad(unit, "the tracker's mission list holds None".to_string()));
        };
        let Some(number) = plot_mission(vm, def, unit)? else { continue };
        let status = call_required(vm, tracker, "GetMissionStatus", vec![Value::Object(Some(def))], unit)?.ret;
        if let Some(n) = scan.grade(byte_of(&status, "GetMissionStatus", unit)?, number) {
            return Ok(n);
        }
    }
    Ok(scan.end())
}

/// `GetActiveMissionNumber` on `controller` (module documentation).
pub fn active_mission_number(vm: &mut Vm, controller: ObjRef) -> Result<i32, VmError> {
    const UNIT: &str = "WillowPlayerController.GetActiveMissionNumber";
    if let Some(tracker) = tracker(vm, UNIT)? {
        let n = from_tracker(vm, tracker, UNIT)?;
        if n != -1 {
            return Ok(n);
        }
    }
    plot_critical_mission_number(vm, controller, -1)
}

/// `GetActivePlotCriticalMissionNumber(index)` on `controller` (module documentation).
pub fn plot_critical_mission_number(vm: &mut Vm, controller: ObjRef, index: i32) -> Result<i32, VmError> {
    const UNIT: &str = "WillowPlayerController.GetActivePlotCriticalMissionNumber";
    let index = if index < 0 {
        let Some(answer) = call_method(vm, controller, "GetCurrentPlaythrough", vec![])? else {
            return Err(NotYet::not_read(PLAYTHROUGH_QUERY, RECORD).into());
        };
        int_of(&answer.ret, "GetCurrentPlaythrough", UNIT)?
    } else {
        index
    };
    let all = field(vm, controller, "MissionPlaythroughs", UNIT)?;
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
        let Some(number) = plot_mission(vm, def, UNIT)? else { continue };
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

pub(crate) fn n_update_mission_text_list(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let me = c.this.ok_or_else(|| bad(UPDATE_MISSION_TEXT_LIST, "called with no movie".to_string()))?;
    let list = object_of(field(c.vm, me, "MissionTextList", UPDATE_MISSION_TEXT_LIST)?, "MissionTextList", UPDATE_MISSION_TEXT_LIST)?;
    match list {
        None => Ok(Value::None),
        Some(_) => Err(NotYet::layer_absent(
            format!("{}: {TEXT_LIST}", super::datastore::PARTIAL_WRITES),
            TEXT_LIST_LAYER,
            RECORD,
        )
        .into()),
    }
}

#[cfg(test)]
#[path = "mission_numbers_tests.rs"]
mod tests;
