//! Tests of the two mission numbers and the mission screen's text list (decision 2026-11-19-93) on invented packages
//! (AGENTS.md rule 9). Every value is invented. AI-assisted (Claude Code).

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::{Missing, NotYet, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::script::{CPF_OPT, CPF_OUT, FunctionSpec, Param, ScriptToy};
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

use super::*;
use crate::actors::Actors;

static NEXT: AtomicU32 = AtomicU32::new(0);
const NATIVE: u32 = 0x400 | 0x20000;

fn param(kind: &str, name: &str, flags: u64, type_ref: i32) -> Param {
    Param { flags, type_ref, ..Param::new(kind, name) }
}

/// `Engine.upk`: `Object` and a `WorldInfo` with its `GRI`.
fn engine() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "Function", "ObjectProperty"]);
    let object = t.class("Object", 0);
    let world_info = t.class("WorldInfo", object);
    t.prop("Object", world_info, "GRI", 0, object);
    t.p.build()
}

/// `WillowGame.upk`: the replication info (its `MissionTracker`), a toy `MissionTracker` (`ActiveMission`, and the
/// natives `IsDataValid`, `GetAllMissions(out array)`, `GetMissionStatus`), a `MissionDefinition` with the three fields
/// the scans read, a `WillowPlayerController` with `MissionPlaythroughs` (array of `MissionPlaythroughData`, each a
/// `MissionList` of `MissionStatusData { MissionDef, Status }`), its native `GetCurrentPlaythrough` and the two
/// number natives, `QuestAcceptGFxMovie` with `MissionTextList` and `UpdateMissionTextList`, another replication info
/// class, and a bare controller class that declares no `GetCurrentPlaythrough`.
fn willow() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&[
        "Class",
        "Function",
        "IntProperty",
        "ObjectProperty",
        "BoolProperty",
        "ByteProperty",
        "StructProperty",
        "ArrayProperty",
        "ScriptStruct",
    ]);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    let strukt = |t: &mut ScriptToy, outer: i32, name: &str| {
        let mut e = ow_package::testkit::Export::new(t.import("ScriptStruct"), name, vec![0u8; 16]);
        e.outer = outer;
        t.p.add_export_with(e)
    };
    let array_of = |t: &mut ScriptToy, owner: i32, name: &str, kind: &str, type_ref: i32| {
        let array = t.p.next_export();
        t.prop("Array", owner, name, 0, array + 1);
        t.prop(kind, array, name, 0, type_ref);
    };
    let definition = t.class("MissionDefinition", object);
    t.prop("Bool", definition, "bPlotCritical", 0, 0);
    t.prop("Object", definition, "DlcExpansion", 0, object);
    t.prop("Int", definition, "MissionNumber", 0, 0);
    let tracker = t.class("MissionTracker", object);
    t.prop("Object", tracker, "ActiveMission", 0, definition);
    let spec = FunctionSpec::new(NATIVE, "IsDataValid");
    t.function(tracker, "IsDataValid", &[], Some(("Bool", 0)), &[], &spec, |_| None);
    let inner = t.prop("Object", 0, "ToyMissionsInner", 0, definition);
    let spec = FunctionSpec::new(NATIVE, "GetAllMissions");
    let ids = t.function(tracker, "GetAllMissions", &[param("Array", "Missions", CPF_OUT, inner)], None, &[], &spec, |_| None);
    t.p.set_outer(inner, ids.get("Missions"));
    let spec = FunctionSpec::new(NATIVE, "GetMissionStatus");
    t.function(tracker, "GetMissionStatus", &[param("Object", "Mission", 0, definition)], Some(("Byte", 0)), &[], &spec, |_| None);
    let gri = t.class("WillowGameReplicationInfo", object);
    t.prop("Object", gri, "MissionTracker", 0, tracker);
    t.class("ToyOtherReplicationInfo", object);
    let pc = t.class("WillowPlayerController", object);
    let status = strukt(&mut t, pc, "MissionStatusData");
    t.prop("Object", status, "MissionDef", 0, definition);
    t.prop("Byte", status, "Status", 0, 0);
    let playthrough = strukt(&mut t, pc, "MissionPlaythroughData");
    array_of(&mut t, playthrough, "MissionList", "Struct", status);
    array_of(&mut t, pc, "MissionPlaythroughs", "Struct", playthrough);
    let spec = FunctionSpec::new(NATIVE, "GetCurrentPlaythrough");
    t.function(pc, "GetCurrentPlaythrough", &[], Some(("Int", 0)), &[], &spec, |_| None);
    let spec = FunctionSpec::new(NATIVE, "GetActiveMissionNumber");
    t.function(pc, "GetActiveMissionNumber", &[], Some(("Int", 0)), &[], &spec, |_| None);
    let spec = FunctionSpec::new(NATIVE, "GetActivePlotCriticalMissionNumber");
    let index = [param("Int", "PlaythroughIndex", CPF_OPT, 0)];
    t.function(pc, "GetActivePlotCriticalMissionNumber", &index, Some(("Int", 0)), &[], &spec, |_| None);
    t.class("ToyBareController", object);
    let movie3d = t.class("WillowGFxMovie3D", object);
    let spec = FunctionSpec::new(NATIVE, "FocusOn");
    t.function(movie3d, "FocusOn", &[param("Object", "Target", 0, object)], None, &[], &spec, |_| None);
    t.class("ToyOtherMovie", movie3d);
    let movie = t.class("QuestAcceptGFxMovie", movie3d);
    t.prop("Object", movie, "MissionTextList", 0, object);
    let spec = FunctionSpec::new(NATIVE, "UpdateMissionTextList");
    t.function(movie, "UpdateMissionTextList", &[], None, &[], &spec, |_| None);
    t.p.build()
}

/// What the toy tracker's natives answer.
#[derive(Default)]
struct Tracker {
    valid: bool,
    missions: Vec<Value>,
    /// The status of each definition (NotStarted when absent).
    status: Vec<(ObjRef, u8)>,
    /// What `GetAllMissions` answers instead of the list.
    list_error: Option<NotYet>,
    /// How often `GetMissionStatus` was asked.
    asked: u32,
}

struct Toy {
    vm: Vm,
    dir: PathBuf,
    tracker_state: Rc<RefCell<Tracker>>,
    playthrough: Rc<RefCell<i32>>,
    pc: ObjRef,
    tracker: ObjRef,
    gri: ObjRef,
    world_info: ObjRef,
}

impl Drop for Toy {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn bind_test(vm: &mut Vm, path: &str, f: impl Fn(&mut NativeCall<'_>) -> Result<Value, VmError> + 'static) {
    let id = vm.find_function(path).unwrap();
    let key = vm.function(id).unwrap().native_key.clone().unwrap();
    vm.register_native(&key, Rc::new(f)).unwrap();
}

/// The toy world: a level whose `WorldInfo.GRI` is a `WillowGameReplicationInfo` with a tracker (`world` false: no
/// level at all), the controller, and this module's natives bound as the world binds them in Off.
fn toy(world: bool) -> Toy {
    let dir = std::env::temp_dir().join(format!("ow-world-mn-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let (e, w) = (engine(), willow());
    let files: [(&str, &[u8]); 2] = [("Engine.upk", &e), ("WillowGame.upk", &w)];
    let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, ParityMode::Off).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let state = Rc::new(RefCell::new(Tracker::default()));
    let s = state.clone();
    bind_test(&mut vm, "WillowGame.MissionTracker.IsDataValid", move |_c| Ok(Value::Bool(s.borrow().valid)));
    let s = state.clone();
    bind_test(&mut vm, "WillowGame.MissionTracker.GetAllMissions", move |c| {
        let (missions, error) = {
            let s = s.borrow();
            (s.missions.clone(), s.list_error.clone())
        };
        if let Some(e) = error {
            return Err(e.into());
        }
        c.set_out(0, Value::array(missions))?;
        Ok(Value::None)
    });
    let s = state.clone();
    bind_test(&mut vm, "WillowGame.MissionTracker.GetMissionStatus", move |c| {
        let Value::Object(Some(def)) = c.arg(0)?.clone() else { return Ok(Value::Byte(0)) };
        let mut s = s.borrow_mut();
        s.asked += 1;
        Ok(Value::Byte(s.status.iter().find(|(d, _)| *d == def).map_or(0, |(_, st)| *st)))
    });
    let playthrough = Rc::new(RefCell::new(0));
    let p = playthrough.clone();
    bind_test(&mut vm, "WillowGame.WillowPlayerController.GetCurrentPlaythrough", move |_c| Ok(Value::Int(*p.borrow())));
    crate::world::datastore::register(&mut vm).unwrap();
    let alloc = |vm: &mut Vm, class: &str, name: &str| {
        let c = vm.find_class(class).unwrap();
        vm.space.alloc(c, name, None, Origin::Runtime).unwrap()
    };
    let world_info = alloc(&mut vm, "Engine.WorldInfo", "ToyWorldInfo");
    let gri = alloc(&mut vm, "WillowGame.WillowGameReplicationInfo", "ToyGRI");
    let tracker = alloc(&mut vm, "WillowGame.MissionTracker", "ToyTracker");
    let pc = alloc(&mut vm, "WillowGame.WillowPlayerController", "ToyPC");
    set(&mut vm, world_info, "GRI", Value::Object(Some(gri)));
    set(&mut vm, gri, "MissionTracker", Value::Object(Some(tracker)));
    if world {
        let object = vm.find_class("Engine.Object").unwrap();
        let mut actors = Actors::new(object);
        actors.place_world_info(world_info).unwrap();
        vm.add_service(actors).unwrap();
    }
    Toy { vm, dir, tracker_state: state, playthrough, pc, tracker, gri, world_info }
}

fn set(vm: &mut Vm, o: ObjRef, name: &str, v: Value) {
    let class = vm.space.heap.get(o).unwrap().class;
    let p = vm.space.prop(class, name).unwrap();
    vm.space.write(o, PropAddr::scalar(p), v).unwrap();
}

fn set_field(vm: &Vm, v: &mut Value, name: &str, x: Value) {
    let id = vm.space.names.find(name).unwrap();
    let ty = v.as_struct().unwrap().ty;
    let (index, _) = vm.space.classes.struct_layout(ty).unwrap().field(id).unwrap();
    v.struct_mut().unwrap().fields[usize::try_from(index).unwrap()] = x;
}

impl Toy {
    /// A mission definition: plot-critical or not, with a DLC expansion or not, and its number.
    fn mission(&mut self, plot: bool, dlc: bool, number: i32) -> ObjRef {
        let c = self.vm.find_class("WillowGame.MissionDefinition").unwrap();
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let d = self.vm.space.alloc(c, &format!("ToyMission{n}"), None, Origin::Runtime).unwrap();
        set(&mut self.vm, d, "bPlotCritical", Value::Bool(plot));
        let expansion = if dlc { Some(self.world_info) } else { None };
        set(&mut self.vm, d, "DlcExpansion", Value::Object(expansion));
        set(&mut self.vm, d, "MissionNumber", Value::Int(number));
        d
    }

    /// The tracker's list, each definition with its status.
    fn track(&mut self, valid: bool, list: &[(ObjRef, u8)]) {
        let mut s = self.tracker_state.borrow_mut();
        s.valid = valid;
        s.missions = list.iter().map(|(d, _)| Value::Object(Some(*d))).collect();
        s.status = list.to_vec();
    }

    /// The controller's playthroughs, each a list of (definition, status) records.
    fn records(&mut self, playthroughs: &[&[(Option<ObjRef>, u8)]]) {
        let vm = &mut self.vm;
        let status = vm.space.classes.struct_by_path("WillowGame.WillowPlayerController.MissionStatusData").unwrap();
        let playthrough = vm.space.classes.struct_by_path("WillowGame.WillowPlayerController.MissionPlaythroughData").unwrap();
        let mut all = Vec::new();
        for list in playthroughs {
            let mut records = Vec::new();
            for &(def, st) in *list {
                let mut r = vm.space.classes.struct_defaults(status).unwrap();
                set_field(vm, &mut r, "MissionDef", Value::Object(def));
                set_field(vm, &mut r, "Status", Value::Byte(st));
                records.push(r);
            }
            let mut p = vm.space.classes.struct_defaults(playthrough).unwrap();
            set_field(vm, &mut p, "MissionList", Value::array(records));
            all.push(p);
        }
        let pc = self.pc;
        set(vm, pc, "MissionPlaythroughs", Value::array(all));
    }

    fn call(&mut self, path: &str, this: ObjRef, args: Vec<Value>) -> Result<Value, VmError> {
        let f = self.vm.find_function(path).unwrap();
        Ok(self.vm.call(f, Some(this), args)?.ret)
    }

    /// The rule of section 1 with the note's two unresolved reads taken as read.
    fn active(&mut self) -> Result<Value, VmError> {
        let pc = self.pc;
        active_mission_number(&mut self.vm, pc, Reads::ALL).map(Value::Int)
    }

    /// The rule of section 2 with the current-playthrough virtual taken as read (an omitted index is -1).
    fn plot(&mut self, index: Option<i32>) -> Result<Value, VmError> {
        let pc = self.pc;
        plot_critical_mission_number(&mut self.vm, pc, index.unwrap_or(-1), Reads::ALL).map(Value::Int)
    }
}

fn script_error(r: Result<Value, VmError>, naming: &str) {
    match r {
        Err(VmError::Script { message, .. }) => assert!(message.contains(naming), "{message}"),
        other => panic!("expected a script error naming {naming:?}, got {other:?}"),
    }
}

/// Section 1, step 2: the tracked mission answers its number, with no status or plot-critical check.
#[test]
fn the_tracked_mission_answers_with_no_status_check() {
    let mut t = toy(true);
    let side = t.mission(false, true, 41);
    let plot = t.mission(true, false, 12);
    t.track(true, &[(plot, 1)]);
    let tracker = t.tracker;
    set(&mut t.vm, tracker, "ActiveMission", Value::Object(Some(side)));
    assert_eq!(t.active().unwrap(), Value::Int(41));
    assert_eq!(t.tracker_state.borrow().asked, 0, "no status asked");
}

/// Section 1, step 3: the first plot-critical, non-DLC mission in progress, in list order; side and DLC missions are
/// skipped whatever their status, and their status is not asked.
#[test]
fn the_tracker_scan_answers_the_first_plot_mission_in_progress() {
    let mut t = toy(true);
    let side = t.mission(false, false, 90);
    let done_low = t.mission(true, false, 3);
    let done_high = t.mission(true, false, 9);
    let dlc = t.mission(true, true, 20);
    let first = t.mission(true, false, 5);
    let second = t.mission(true, false, 6);
    t.track(true, &[(side, 1), (done_low, 4), (done_high, 4), (dlc, 2), (first, 3), (second, 1)]);
    assert_eq!(t.active().unwrap(), Value::Int(5));
    assert_eq!(t.tracker_state.borrow().asked, 3, "done_low, done_high, first");
}

/// Section 1, step 3: with no plot mission in progress, the largest Complete number; NotStarted and Failed are not kept.
#[test]
fn the_tracker_scan_keeps_the_largest_complete_number() {
    let mut t = toy(true);
    let a = t.mission(true, false, 7);
    let b = t.mission(true, false, 11);
    let c = t.mission(true, false, 8);
    let failed = t.mission(true, false, 30);
    let not_started = t.mission(true, false, 31);
    t.track(true, &[(a, 4), (b, 4), (c, 4), (failed, 5), (not_started, 0)]);
    assert_eq!(t.active().unwrap(), Value::Int(11));
}

/// An empty scan answers 0, which is not the fallback's -1: the controller's records are not read.
#[test]
fn an_empty_scan_answers_zero_without_the_fallback() {
    let mut t = toy(true);
    let plot = t.mission(true, false, 13);
    t.track(true, &[]);
    t.records(&[&[(Some(plot), 1)]]);
    assert_eq!(t.active().unwrap(), Value::Int(0));
}

/// Section 1, step 4: the fallback is the plot-critical number for the current playthrough, not a tracked side
/// mission: with data not valid, with no level, with a replication info of another class, with no tracker, and when the
/// tracker answers -1.
#[test]
fn the_fallback_is_the_plot_critical_number() {
    let check = |t: &mut Toy| {
        let side = t.mission(false, false, 44);
        let plot = t.mission(true, false, 17);
        t.records(&[&[(Some(side), 1), (Some(plot), 2)]]);
        assert_eq!(t.active().unwrap(), Value::Int(17));
    };
    let mut t = toy(true);
    let tracked = t.mission(true, false, 99);
    t.track(false, &[(tracked, 1)]);
    check(&mut t);
    let mut t = toy(false);
    t.track(true, &[]);
    check(&mut t);
    let mut t = toy(true);
    t.track(true, &[]);
    let other = t.vm.find_class("WillowGame.ToyOtherReplicationInfo").unwrap();
    let other = t.vm.space.alloc(other, "ToyOtherGRI", None, Origin::Runtime).unwrap();
    let wi = t.world_info;
    set(&mut t.vm, wi, "GRI", Value::Object(Some(other)));
    check(&mut t);
    let mut t = toy(true);
    t.track(true, &[]);
    let gri = t.gri;
    set(&mut t.vm, gri, "MissionTracker", Value::Object(None));
    check(&mut t);
    let mut t = toy(true);
    let minus = t.mission(true, false, -1);
    t.track(true, &[]);
    let tracker = t.tracker;
    set(&mut t.vm, tracker, "ActiveMission", Value::Object(Some(minus)));
    check(&mut t);
}

/// Section 2: an explicit index reads that playthrough; out of range answers 0; a record without a definition, a side
/// mission and a DLC mission are skipped; Complete keeps the largest.
#[test]
fn the_plot_critical_number_reads_the_controllers_records() {
    let mut t = toy(true);
    let side = t.mission(false, false, 50);
    let dlc = t.mission(true, true, 60);
    let low = t.mission(true, false, 2);
    let high = t.mission(true, false, 4);
    let active = t.mission(true, false, 3);
    t.records(&[&[(Some(active), 1)], &[(None, 1), (Some(side), 1), (Some(dlc), 3), (Some(high), 4), (Some(low), 4)]]);
    assert_eq!(t.plot(Some(1)).unwrap(), Value::Int(4));
    assert_eq!(t.plot(Some(0)).unwrap(), Value::Int(3));
    assert_eq!(t.plot(Some(2)).unwrap(), Value::Int(0), "out of range");
    assert_eq!(t.plot(Some(5)).unwrap(), Value::Int(0), "out of range");
    *t.playthrough.borrow_mut() = 1;
    assert_eq!(t.plot(None).unwrap(), Value::Int(4), "omitted: the current playthrough");
    assert_eq!(t.plot(Some(-1)).unwrap(), Value::Int(4), "negative: the current playthrough");
    *t.playthrough.borrow_mut() = 7;
    assert_eq!(t.plot(None).unwrap(), Value::Int(0), "a current playthrough out of range");
}

/// Section 2, Off step 1: a controller whose class declares no current-playthrough query is a named `NotYet`.
#[test]
fn a_controller_without_the_playthrough_query_is_not_read() {
    let mut t = toy(true);
    let bare = t.vm.find_class("WillowGame.ToyBareController").unwrap();
    let bare = t.vm.space.alloc(bare, "ToyBare", None, Origin::Runtime).unwrap();
    match plot_critical_mission_number(&mut t.vm, bare, -1, Reads::ALL) {
        Err(VmError::NotYet(n)) => assert!(n.missing == Missing::NotRead && n.what == PLAYTHROUGH_QUERY, "{n}"),
        other => panic!("{other:?}"),
    }
}

/// A tracker native's own `NotYet` is propagated unchanged (kind and text): the port's bridge answers `GetAllMissions`
/// with a `LayerAbsent` today.
#[test]
fn a_tracker_not_yet_is_propagated_unchanged() {
    let mut t = toy(true);
    t.track(true, &[]);
    t.tracker_state.borrow_mut().list_error = Some(NotYet::layer_absent("an invented list", "an invented layer", "an invented record"));
    match t.active() {
        Err(VmError::NotYet(n)) => {
            assert!(matches!(n.missing, Missing::LayerAbsent(_)), "{n}");
            assert_eq!(n.what, "an invented list");
        }
        other => panic!("{other:?}"),
    }
}

/// Bad data is a script error naming what is wrong (an entry of the tracker's list of a class without the fields); a
/// `None` entry is the note's `NotRead`.
#[test]
fn bad_data_is_a_script_error() {
    let mut t = toy(true);
    t.track(true, &[]);
    let wi = t.world_info;
    t.tracker_state.borrow_mut().missions = vec![Value::Object(Some(wi))];
    script_error(t.active(), "bPlotCritical");
    let mut t = toy(true);
    t.track(true, &[]);
    t.tracker_state.borrow_mut().missions = vec![Value::Object(None)];
    not_yet(t.active(), false, NONE_ENTRY);
}

fn not_yet(r: Result<Value, VmError>, layer: bool, what: &str) {
    not_yet_in(r, layer, what, RECORD);
}

fn not_yet_in(r: Result<Value, VmError>, layer: bool, what: &str, record: &str) {
    match r {
        Err(VmError::NotYet(n)) => {
            if layer {
                assert!(matches!(n.missing, Missing::LayerAbsent(_)), "{n}");
            } else {
                assert_eq!(n.missing, Missing::NotRead, "{n}");
            }
            assert_eq!(n.what, what);
            assert_eq!(n.record, record);
        }
        other => panic!("expected the NotYet {what:?}, got {other:?}"),
    }
}

/// The bound natives (nothing taken as read): the fallback's current playthrough is the unread virtual, so a negative
/// or omitted index is `NotRead`, and `GetActiveMissionNumber` meets it through its fallback branch; an explicit,
/// non-negative index needs no virtual and is answered.
#[test]
fn the_natives_answer_the_unread_playthrough_virtual_as_not_read() {
    let mut t = toy(true);
    let plot = t.mission(true, false, 12);
    t.track(true, &[(plot, 1)]);
    t.records(&[&[(Some(plot), 1)]]);
    let pc = t.pc;
    not_yet(t.call(ACTIVE_MISSION_NUMBER, pc, vec![]), false, PLAYTHROUGH_QUERY);
    not_yet(t.call(PLOT_CRITICAL_NUMBER, pc, vec![]), false, PLAYTHROUGH_QUERY);
    not_yet(t.call(PLOT_CRITICAL_NUMBER, pc, vec![Value::Int(-1)]), false, PLAYTHROUGH_QUERY);
    assert_eq!(t.call(PLOT_CRITICAL_NUMBER, pc, vec![Value::Int(0)]).unwrap(), Value::Int(12));
}

/// Section 1, Off step 2: the engine-wide condition is not guessed. With a tracker, both branches are computed; equal
/// answers give the number, different ones are `NotRead`, and a failing branch is the answer (the tracker's first).
/// With no tracker, or data not valid, the fallback alone answers.
#[test]
fn both_branches_answer_when_they_agree_and_not_read_when_they_differ() {
    let off = Reads { engine_gate: false, playthrough: true };
    let mut t = toy(true);
    let plot = t.mission(true, false, 12);
    let other = t.mission(true, false, 30);
    let pc = t.pc;
    t.track(true, &[(plot, 1)]);
    t.records(&[&[(Some(plot), 1)]]);
    assert_eq!(active_mission_number(&mut t.vm, pc, off).unwrap(), 12, "equal");
    t.records(&[&[(Some(other), 1)]]);
    match active_mission_number(&mut t.vm, pc, off) {
        Err(VmError::NotYet(n)) => assert!(n.missing == Missing::NotRead && n.what == ENGINE_GATE, "{n}"),
        other => panic!("{other:?}"),
    }
    t.track(false, &[(plot, 1)]);
    assert_eq!(active_mission_number(&mut t.vm, pc, off).unwrap(), 30, "data not valid: the fallback");
    t.track(true, &[]);
    t.tracker_state.borrow_mut().list_error = Some(NotYet::layer_absent("an invented list", "an invented layer", "an invented record"));
    match active_mission_number(&mut t.vm, pc, off) {
        Err(VmError::NotYet(n)) => assert_eq!(n.what, "an invented list", "the tracker branch's failure first"),
        other => panic!("{other:?}"),
    }
    let mut t = toy(false);
    let lone = t.mission(true, false, 8);
    t.records(&[&[(Some(lone), 2)]]);
    let pc = t.pc;
    assert_eq!(active_mission_number(&mut t.vm, pc, off).unwrap(), 8, "no level: the fallback");
}

/// Section 3 in the conservative form: the text list update is the named `LayerAbsent` with the list None or set, and
/// the focus pair is too; on the mission screen both are marked after the accept's writes, on another movie the focus
/// pair is not marked. Nothing on the screen changes.
#[test]
fn the_screen_calls_are_layer_absent_and_marked_on_the_mission_screen() {
    let mut t = toy(true);
    let marked = |what: &str| format!("{}: {what}", crate::world::datastore::PARTIAL_WRITES);
    let movie = t.vm.find_class("WillowGame.QuestAcceptGFxMovie").unwrap();
    let movie = t.vm.space.alloc(movie, "ToyQuestAccept", None, Origin::Runtime).unwrap();
    not_yet_in(t.call(UPDATE_MISSION_TEXT_LIST, movie, vec![]), true, &marked(TEXT_LIST), RECORD_GFX);
    let wi = t.world_info;
    set(&mut t.vm, movie, "MissionTextList", Value::Object(Some(wi)));
    not_yet_in(t.call(UPDATE_MISSION_TEXT_LIST, movie, vec![]), true, &marked(TEXT_LIST), RECORD_GFX);
    let focus = "WillowGame.WillowGFxMovie3D.FocusOn";
    not_yet_in(t.call(focus, movie, vec![Value::Object(None)]), true, &marked(FOCUS), RECORD_GFX);
    let other = t.vm.find_class("WillowGame.ToyOtherMovie").unwrap();
    let other = t.vm.space.alloc(other, "ToyOther", None, Origin::Runtime).unwrap();
    not_yet_in(t.call(focus, other, vec![Value::Object(None)]), true, FOCUS, RECORD_GFX);
    let class = t.vm.space.heap.get(movie).unwrap().class;
    let p = t.vm.space.prop(class, "MissionTextList").unwrap();
    assert_eq!(t.vm.space.read(movie, PropAddr::scalar(p)).unwrap(), &Value::Object(Some(wi)), "nothing changed");
}
