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
        "NameProperty",
        "FloatProperty",
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
    t.prop("Int", gri, "CurrentPlaythrough", 0, 0);
    // A strict subclass of the replication info (lane IMPL-PLAYTHROUGH-VIRTUAL: its answer is not read).
    t.class("ToySubReplicationInfo", gri);
    t.class("ToyOtherReplicationInfo", object);
    let pc = t.class("WillowPlayerController", object);
    // A strict subclass of the controller (its current-playthrough virtual is not read).
    t.class("ToySubController", pc);
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
    // A controller class whose own GetActivePlotCriticalMissionNumber is a native nothing binds (review r1 of #480).
    let unbound = t.class("ToyUnboundController", object);
    let spec = FunctionSpec::new(NATIVE, "GetActivePlotCriticalMissionNumber");
    let index = [param("Int", "PlaythroughIndex", CPF_OPT, 0)];
    t.function(unbound, "GetActivePlotCriticalMissionNumber", &index, Some(("Int", 0)), &[], &spec, |_| None);
    let movie3d = t.class("WillowGFxMovie3D", object);
    let vector = strukt(&mut t, movie3d, "ToyVector");
    for axis in ["X", "Y", "Z"] {
        t.prop("Float", vector, axis, 0, 0);
    }
    t.prop("Struct", movie3d, "FocusOffset", 0, vector);
    t.prop("Struct", movie3d, "InterpFocusOffset", 0, vector);
    // Declared last, so the layout gives it the slot before `FocusOffset`'s (the slot-missing test cuts between them).
    t.prop("Object", movie3d, "FocusObject", 0, object);
    let spec = FunctionSpec::new(NATIVE, "FocusOn");
    t.function(movie3d, "FocusOn", &[param("Object", "Target", 0, object)], None, &[], &spec, |_| None);
    t.class("ToyOtherMovie", movie3d);
    let movie = t.class("QuestAcceptGFxMovie", movie3d);
    t.prop("Object", movie, "MissionTextList", 0, object);
    array_of(&mut t, movie, "MissionCategories", "Int", 0);
    let spec = FunctionSpec::new(NATIVE, "UpdateMissionTextList");
    t.function(movie, "UpdateMissionTextList", &[], None, &[], &spec, |_| None);
    // A movie class without MissionCategories (bad data for the text list update).
    let bare = t.class("ToyBareQuestMovie", movie3d);
    t.prop("Object", bare, "MissionTextList", 0, object);
    let service = t.class("WillowLeviathanService", object);
    let spec = FunctionSpec::new(NATIVE, "RecordMissionStatusChangedEventForPlayer");
    let params = [param("Object", "ForPlayer", 0, pc), param("Byte", "Status", 0, 0), param("Name", "MissionDefinition", 0, 0)];
    t.function(service, "RecordMissionStatusChangedEventForPlayer", &params, None, &[], &spec, |_| None);
    let spec = FunctionSpec::new(NATIVE, "RecordMissionObjectiveChangedEventForPlayer");
    let params = [
        param("Object", "ForPlayer", 0, pc),
        param("Int", "ObjectiveIndex", 0, 0),
        param("Int", "ObjectiveProgress", 0, 0),
        param("Int", "PreviousObjectiveProgress", 0, 0),
        param("Name", "MissionDefinition", 0, 0),
    ];
    t.function(service, "RecordMissionObjectiveChangedEventForPlayer", &params, None, &[], &spec, |_| None);
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
    toy_with(world, &[])
}

/// [`toy`] with more invented packages after the four (review r1 of #533: a second package with the same native).
fn toy_with(world: bool, extra: &[(&str, &[u8])]) -> Toy {
    let dir = std::env::temp_dir().join(format!("ow-world-mn-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let (e, w, g, d) = (engine(), willow(), gfxui(), definitions());
    let mut files: Vec<(&str, &[u8])> =
        vec![("Engine.upk", &e), ("WillowGame.upk", &w), ("GFxUI.upk", &g), ("GD_ToyMissions.upk", &d)];
    files.extend_from_slice(extra);
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
    Toy { vm, dir, tracker_state: state, pc, tracker, gri, world_info }
}

/// `GFxUI.upk` (invented): `GFxMoviePlayer` and its native `GetVariableObject(string, class)`.
fn gfxui() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "Function", "StrProperty", "ObjectProperty", "ClassProperty"]);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    let player = t.class("GFxMoviePlayer", object);
    let spec = FunctionSpec::new(NATIVE, "GetVariableObject");
    let params = [param("Str", "Path", 0, 0), param("Object", "Type", CPF_OPT, object)];
    t.function(player, "GetVariableObject", &params, Some(("Object", object)), &[], &spec, |_| None);
    t.p.build()
}

/// `GD_ToyMissions.upk` (invented): two mission definitions. `Holder`'s `DlcExpansion` names `Target`, so reading it
/// from the loaded holder gives `Target` as an unloaded store handle. `Target` is plot-critical with number 27.
fn definitions() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class"]);
    let willow = t.p.add_import("Core", "Package", 0, "WillowGame");
    let definition = t.p.add_import("Core", "Class", willow, "MissionDefinition");
    let mut target = ow_package::testkit::ints(&[0]);
    target.extend(t.p.tag_detail("bPlotCritical", "BoolProperty", &[1], &[]));
    target.extend(t.p.tag("MissionNumber", "IntProperty", &ow_package::testkit::ints(&[27])));
    target.extend_from_slice(&t.none);
    let target = t.p.add_export(definition, "Target", target);
    let mut holder = ow_package::testkit::ints(&[0]);
    holder.extend(t.p.tag("DlcExpansion", "ObjectProperty", &ow_package::testkit::ints(&[target])));
    holder.extend_from_slice(&t.none);
    t.p.add_export(definition, "Holder", holder);
    t.p.build()
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

    /// The rule of section 1.
    fn active(&mut self) -> Result<Value, VmError> {
        let pc = self.pc;
        active_mission_number(&mut self.vm, pc).map(Value::Int)
    }

    /// The rule of section 2 (an omitted index is -1).
    fn plot(&mut self, index: Option<i32>) -> Result<Value, VmError> {
        let pc = self.pc;
        plot_critical_mission_number(&mut self.vm, pc, index.unwrap_or(-1)).map(Value::Int)
    }

    /// The world's current playthrough: the replication info's `CurrentPlaythrough`.
    fn set_playthrough(&mut self, n: i32) {
        let gri = self.gri;
        set(&mut self.vm, gri, "CurrentPlaythrough", Value::Int(n));
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
    t.set_playthrough(1);
    assert_eq!(t.plot(None).unwrap(), Value::Int(4), "omitted: the current playthrough");
    assert_eq!(t.plot(Some(-1)).unwrap(), Value::Int(4), "negative: the current playthrough");
    t.set_playthrough(7);
    assert_eq!(t.plot(None).unwrap(), Value::Int(0), "a current playthrough out of range");
}

/// Section 2, step 1: a controller that is not exactly a `WillowPlayerController` asks a current-playthrough virtual
/// that was not read (NATIVE_PLAYTHROUGH_QUERY.md section 4, step 1), whether it is another class or a subclass.
#[test]
fn a_controller_of_another_class_asks_an_unread_virtual() {
    for class in ["WillowGame.ToyBareController", "WillowGame.ToySubController"] {
        let mut t = toy(true);
        let c = t.vm.find_class(class).unwrap();
        let other = t.vm.space.alloc(c, "ToyOther", None, Origin::Runtime).unwrap();
        not_yet_in(
            plot_critical_mission_number(&mut t.vm, other, -1).map(Value::Int),
            false,
            CONTROLLER_OVERRIDE,
            RECORD_PLAYTHROUGH,
        );
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

impl Toy {
    /// `GD_ToyMissions.Target` as the store's unloaded handle, read from the loaded holder.
    fn unloaded_target(&mut self) -> ObjRef {
        let path = ow_core::ObjectPath::parse("GD_ToyMissions.Holder").unwrap();
        let holder = self.vm.defs.find(&mut self.vm.space, &path).unwrap().unwrap();
        let class = self.vm.space.heap.get(holder).unwrap().class;
        let p = self.vm.space.prop(class, "DlcExpansion").unwrap();
        let Value::Object(Some(target)) = self.vm.space.read(holder, PropAddr::scalar(p)).unwrap().clone() else {
            panic!("the holder names its target")
        };
        assert!(self.vm.defs.is_unloaded(&self.vm.space, target), "a genuinely unloaded handle");
        target
    }
}

/// The bound natives (lane IMPL-PLAYTHROUGH-VIRTUAL): a negative or omitted index reads the world's current
/// playthrough through the rule, as a function; `GetActiveMissionNumber` answers its tracker branch alone; an explicit
/// index needs no virtual.
#[test]
fn the_natives_read_the_world_s_current_playthrough() {
    let mut t = toy(true);
    let plot = t.mission(true, false, 12);
    let second = t.mission(true, false, 21);
    t.track(true, &[(plot, 1)]);
    t.records(&[&[(Some(plot), 1)], &[(Some(second), 1)]]);
    let pc = t.pc;
    assert_eq!(t.call(ACTIVE_MISSION_NUMBER, pc, vec![]).unwrap(), Value::Int(12));
    assert_eq!(t.call(PLOT_CRITICAL_NUMBER, pc, vec![]).unwrap(), Value::Int(12), "playthrough 0");
    t.set_playthrough(1);
    assert_eq!(t.call(PLOT_CRITICAL_NUMBER, pc, vec![Value::Int(-1)]).unwrap(), Value::Int(21), "playthrough 1");
    assert_eq!(t.call(PLOT_CRITICAL_NUMBER, pc, vec![Value::Int(0)]).unwrap(), Value::Int(12), "explicit index");
}

/// The current-playthrough rule's steps (NATIVE_PLAYTHROUGH_QUERY.md section 4): no world, no `GRI` and a `GRI` of
/// another class answer 0; a strict subclass of the replication info is `NotRead`; a `CurrentPlaythrough` of the wrong
/// kind is bad data; every exactly-`WillowPlayerController` controller gets the world's answer.
#[test]
fn the_current_playthrough_rule_follows_the_note_s_steps() {
    let t = toy(false);
    let pc = t.pc;
    assert_eq!(current_playthrough(&t.vm, pc).unwrap(), 0, "no world");
    let mut t = toy(true);
    t.set_playthrough(2);
    let pc = t.pc;
    assert_eq!(current_playthrough(&t.vm, pc).unwrap(), 2);
    let c = t.vm.find_class("WillowGame.WillowPlayerController").unwrap();
    let another = t.vm.space.alloc(c, "ToyPC2", None, Origin::Runtime).unwrap();
    assert_eq!(current_playthrough(&t.vm, another).unwrap(), 2, "any such controller: the world's answer");
    let wi = t.world_info;
    set(&mut t.vm, wi, "GRI", Value::Object(None));
    assert_eq!(current_playthrough(&t.vm, pc).unwrap(), 0, "no GRI");
    let other = t.vm.find_class("WillowGame.ToyOtherReplicationInfo").unwrap();
    let other = t.vm.space.alloc(other, "ToyOtherGRI", None, Origin::Runtime).unwrap();
    set(&mut t.vm, wi, "GRI", Value::Object(Some(other)));
    assert_eq!(current_playthrough(&t.vm, pc).unwrap(), 0, "a GRI of another class");
    let sub = t.vm.find_class("WillowGame.ToySubReplicationInfo").unwrap();
    let sub = t.vm.space.alloc(sub, "ToySubGRI", None, Origin::Runtime).unwrap();
    set(&mut t.vm, wi, "GRI", Value::Object(Some(sub)));
    not_yet_in(current_playthrough(&t.vm, pc).map(Value::Int), false, GRI_OVERRIDE, RECORD_PLAYTHROUGH);
    let gri = t.gri;
    set(&mut t.vm, wi, "GRI", Value::Object(Some(gri)));
    let class = t.vm.space.heap.get(gri).unwrap().class;
    let p = t.vm.space.prop(class, "CurrentPlaythrough").unwrap();
    *t.vm.space.slot_mut(gri, PropAddr::scalar(p)).unwrap() = Value::Float(1.0);
    script_error(current_playthrough(&t.vm, pc).map(Value::Int), "CurrentPlaythrough");
}

/// Review r1 of #480, item 3: the fallback is dispatched through the VM on the controller's class, so with no tracker
/// and a plot-critical native nothing binds, the answer is the callee's own `UnboundNative`, not a `NotRead`.
#[test]
fn the_fallback_is_dispatched_and_an_unbound_callee_is_unbound_native() {
    let mut t = toy(false);
    let c = t.vm.find_class("WillowGame.ToyUnboundController").unwrap();
    let bare = t.vm.space.alloc(c, "ToyUnbound", None, Origin::Runtime).unwrap();
    match active_mission_number(&mut t.vm, bare) {
        Err(VmError::UnboundNative(key)) => assert!(key.contains("GetActivePlotCriticalMissionNumber"), "{key}"),
        other => panic!("{other:?}"),
    }
}

/// Review r1 of #480, item 1: a record whose `MissionDef` is a genuinely unloaded store handle is loaded and answered
/// (explicit index). The load is a write: a later `NotYet` from the same native is marked once, kind and record kept;
/// the same `NotYet` with nothing loaded is not marked.
#[test]
fn an_unloaded_definition_is_loaded_and_a_later_not_yet_is_marked() {
    let mut t = toy(true);
    let target = t.unloaded_target();
    t.records(&[&[(Some(target), 1)]]);
    let pc = t.pc;
    assert_eq!(t.call(PLOT_CRITICAL_NUMBER, pc, vec![Value::Int(0)]).unwrap(), Value::Int(27));
    assert!(t.vm.defs.is_loaded(&t.vm.space, target));
    // The tracker's list holds the unloaded definition, then a None entry: the scan loads the definition, then the
    // None entry's NotYet comes after the load and is marked once.
    let mut t = toy(true);
    let target = t.unloaded_target();
    t.track(true, &[]);
    t.tracker_state.borrow_mut().missions = vec![Value::Object(Some(target)), Value::Object(None)];
    let pc = t.pc;
    let marked = format!("{}: {NONE_ENTRY}", crate::world::datastore::PARTIAL_WRITES);
    not_yet(t.call(ACTIVE_MISSION_NUMBER, pc, vec![]), false, &marked);
    // The same NotYet with nothing loaded is not marked.
    let loaded = t.mission(true, false, 27);
    t.tracker_state.borrow_mut().missions = vec![Value::Object(Some(loaded)), Value::Object(None)];
    not_yet(t.call(ACTIVE_MISSION_NUMBER, pc, vec![]), false, NONE_ENTRY);
}

/// Section 1, step 1 (NATIVE_PLAYTHROUGH_QUERY.md section 1): the engine-wide condition holds when the world exists, so
/// the tracker branch alone answers, even when the fallback would differ; data not valid, and no level, take the
/// fallback. (#480's both-branch comparison is retired.)
#[test]
fn the_tracker_branch_alone_answers_when_the_world_exists() {
    let mut t = toy(true);
    let plot = t.mission(true, false, 12);
    let other = t.mission(true, false, 30);
    let pc = t.pc;
    t.track(true, &[(plot, 1)]);
    t.records(&[&[(Some(other), 1)]]);
    assert_eq!(active_mission_number(&mut t.vm, pc).unwrap(), 12, "the tracker branch, not the fallback's 30");
    t.track(false, &[(plot, 1)]);
    assert_eq!(active_mission_number(&mut t.vm, pc).unwrap(), 30, "data not valid: the fallback");
    t.track(true, &[]);
    t.tracker_state.borrow_mut().list_error = Some(NotYet::layer_absent("an invented list", "an invented layer", "an invented record"));
    match active_mission_number(&mut t.vm, pc) {
        Err(VmError::NotYet(n)) => assert_eq!(n.what, "an invented list", "the tracker branch's failure"),
        other => panic!("{other:?}"),
    }
    let mut t = toy(false);
    let lone = t.mission(true, false, 8);
    t.records(&[&[(Some(lone), 2)]]);
    let pc = t.pc;
    assert_eq!(active_mission_number(&mut t.vm, pc).unwrap(), 8, "no level: the fallback");
}

/// The mission screen's calls (NATIVE_MISSION_TEXT_LIST.md section 6, lane IMPL-LINK10-NEXT, the hub's ruling R1 on
/// #504): the text list update returns at once with `MissionTextList` None and is the named `LayerAbsent` with it set;
/// `GetVariableObject` answers None. Neither marks its own `NotYet` (the mark comes from the accept's write boundary in
/// the mission bridge), and nothing on the screen changes. `FocusOn` has its own tests below.
#[test]
fn the_screen_calls_answer_the_notes_defined_cases_only() {
    let mut t = toy(true);
    let movie = t.vm.find_class("WillowGame.QuestAcceptGFxMovie").unwrap();
    let movie = t.vm.space.alloc(movie, "ToyQuestAccept", None, Origin::Runtime).unwrap();
    assert_eq!(t.call(UPDATE_MISSION_TEXT_LIST, movie, vec![]).unwrap(), Value::None, "None list: nothing");
    let wi = t.world_info;
    set(&mut t.vm, movie, "MissionTextList", Value::Object(Some(wi)));
    not_yet_in(t.call(UPDATE_MISSION_TEXT_LIST, movie, vec![]), true, TEXT_LIST, RECORD_GFX);
    let player = t.vm.find_class("GFxUI.GFxMoviePlayer").unwrap();
    let player = t.vm.space.alloc(player, "ToyPlayer", None, Origin::Runtime).unwrap();
    let path = Value::Str("missions.selections.selection1".into());
    assert_eq!(t.call(GET_VARIABLE_OBJECT, player, vec![path]).unwrap(), Value::Object(None));
    let class = t.vm.space.heap.get(movie).unwrap().class;
    let p = t.vm.space.prop(class, "MissionTextList").unwrap();
    assert_eq!(t.vm.space.read(movie, PropAddr::scalar(p)).unwrap(), &Value::Object(Some(wi)), "nothing changed");
}

const FOCUS_PATH: &str = "WillowGame.WillowGFxMovie3D.FocusOn";

/// A `QuestAcceptGFxMovie` with every focus field seeded nonzero: `FocusObject` set, `FocusOffset` (3, -4, 5) and
/// `InterpFocusOffset` (1, 2, 3).
fn seeded_movie(t: &mut Toy) -> ObjRef {
    let movie = t.vm.find_class("WillowGame.QuestAcceptGFxMovie").unwrap();
    let movie = t.vm.space.alloc(movie, "ToyQuestAccept", None, Origin::Runtime).unwrap();
    let wi = t.world_info;
    set(&mut t.vm, movie, "FocusObject", Value::Object(Some(wi)));
    for (name, xyz) in [("FocusOffset", [3.0, -4.0, 5.0]), ("InterpFocusOffset", [1.0, 2.0, 3.0])] {
        let class = t.vm.space.heap.get(movie).unwrap().class;
        let p = t.vm.space.prop(class, name).unwrap();
        let mut v = t.vm.space.read(movie, PropAddr::scalar(p)).unwrap().clone();
        for (axis, x) in ["X", "Y", "Z"].iter().zip(xyz) {
            set_field(&t.vm, &mut v, axis, Value::Float(x));
        }
        set(&mut t.vm, movie, name, v);
    }
    movie
}

/// The heap's text, to show a call wrote nothing.
fn heap_text(t: &Toy) -> String {
    format!("{:?}", t.vm.space.heap)
}

/// `FocusOn(None)` (ruling R2; review r1 of #509): two writes in this order, `FocusObject` = None, then `FocusOffset`
/// = (0, 0, 0), and no other (`InterpFocusOffset` keeps its seed). The plan is checked for the order; the call's end
/// state is the seeded heap with exactly those two writes applied.
#[test]
fn focus_on_none_writes_the_object_then_the_zero_vector() {
    let mut t = toy(true);
    let movie = seeded_movie(&mut t);
    let class = t.vm.space.heap.get(movie).unwrap().class;
    let at = |t: &Toy, n: &str| PropAddr::scalar(t.vm.space.prop(class, n).unwrap());
    let plan = focus_writes(&t.vm, movie).unwrap();
    assert_eq!(plan[0], (at(&t, "FocusObject"), Value::Object(None)), "first: FocusObject");
    assert_eq!(plan[1].0, at(&t, "FocusOffset"), "second: FocusOffset");
    for axis in ["X", "Y", "Z"] {
        assert_eq!(struct_field(&t.vm, &plan[1].1, axis, "test").unwrap(), Value::Float(0.0), "{axis} zeroed");
    }
    let mut expected = t.vm.space.clone();
    for (a, v) in plan.clone() {
        expected.write(movie, a, v).unwrap();
    }
    t.vm.add_service(TestFocusWrites(Vec::new())).unwrap();
    assert_eq!(t.call(FOCUS_PATH, movie, vec![Value::Object(None)]).unwrap(), Value::None);
    let written = t.vm.service::<TestFocusWrites>().unwrap().borrow().0.clone();
    assert_eq!(written, vec![at(&t, "FocusObject"), at(&t, "FocusOffset")], "the native's writes, in order");
    assert_eq!(heap_text(&t), format!("{:?}", expected.heap), "exactly the two planned writes");
    let interp = t.vm.space.read(movie, at(&t, "InterpFocusOffset")).unwrap().clone();
    assert_eq!(struct_field(&t.vm, &interp, "Z", "test").unwrap(), Value::Float(3.0), "InterpFocusOffset not written");
}

/// The refusals write nothing (review r1 of #509): a set `Thing` is the named `LayerAbsent`; a negative bound (a scale
/// above 1) and a NaN bound (a NaN stage height) are `NotYet { NotRead }` with the ruling's text and the note's record.
#[test]
fn focus_on_refusals_write_nothing() {
    let mut t = toy(true);
    let movie = seeded_movie(&mut t);
    let wi = t.world_info;
    t.vm.add_service(TestFocusWrites(Vec::new())).unwrap();
    let before = heap_text(&t);
    not_yet_in(t.call(FOCUS_PATH, movie, vec![Value::Object(Some(wi))]), true, FOCUS, RECORD_GFX);
    assert_eq!(heap_text(&t), before, "set Thing: no write");
    t.vm.add_service(TestFocusStage(FocusStage { height: 720.0, scale: 2.0 })).unwrap();
    not_yet_in(t.call(FOCUS_PATH, movie, vec![Value::Object(None)]), false, FOCUS_BOUND, RECORD_TEXT_LIST);
    assert_eq!(heap_text(&t), before, "negative bound: no write");
    t.vm.service::<TestFocusStage>().unwrap().borrow_mut().0 = FocusStage { height: f32::NAN, scale: 0.5 };
    not_yet_in(t.call(FOCUS_PATH, movie, vec![Value::Object(None)]), false, FOCUS_BOUND, RECORD_TEXT_LIST);
    assert_eq!(heap_text(&t), before, "NaN bound: no write");
    assert!(t.vm.service::<TestFocusWrites>().unwrap().borrow().0.is_empty(), "no write on any refusal");
    // A valid nonzero bound writes as the stand-in does.
    t.vm.service::<TestFocusStage>().unwrap().borrow_mut().0 = FocusStage { height: 720.0, scale: 0.5 };
    assert_eq!(t.call(FOCUS_PATH, movie, vec![Value::Object(None)]).unwrap(), Value::None);
    assert_ne!(heap_text(&t), before);
}

/// A set `Thing` refuses before any field is read (review r1 of #509, item 5): the native called directly on an object
/// whose class declares no focus fields (the world info) is still the typed `LayerAbsent` for a set `Thing`, while
/// `Thing` None there is the field check's script error.
#[test]
fn focus_on_a_set_thing_refuses_before_reading_fields() {
    let mut t = toy(true);
    let wi = t.world_info;
    not_yet_in(t.call(FOCUS_PATH, wi, vec![Value::Object(Some(wi))]), true, FOCUS, RECORD_GFX);
    script_error(t.call(FOCUS_PATH, wi, vec![Value::Object(None)]), "FocusObject");
}

/// A movie whose class declares no `MissionCategories` is bad data for the text list update, even with the list None
/// (the native called directly on it).
#[test]
fn a_movie_without_the_categories_is_a_script_error() {
    let mut t = toy(true);
    let bare = t.vm.find_class("WillowGame.ToyBareQuestMovie").unwrap();
    let bare = t.vm.space.alloc(bare, "ToyBare", None, Origin::Runtime).unwrap();
    script_error(t.call(UPDATE_MISSION_TEXT_LIST, bare, vec![]), "MissionCategories");
}

/// A destination slot missing from the receiver (review r2 of #509): the movie's slots end after `FocusObject`, before
/// `FocusOffset`. The heap's own `SlotMissing` comes back before the first write, so `FocusObject` keeps its seed.
#[test]
fn focus_on_a_missing_offset_slot_refuses_before_any_write() {
    let mut t = toy(true);
    let movie = seeded_movie(&mut t);
    let class = t.vm.space.heap.get(movie).unwrap().class;
    let object = PropAddr::scalar(t.vm.space.prop(class, "FocusObject").unwrap());
    let offset = PropAddr::scalar(t.vm.space.prop(class, "FocusOffset").unwrap());
    let at = t.vm.space.classes.resolve(class, object).unwrap();
    assert!(t.vm.space.classes.resolve(class, offset).unwrap() > at, "the toy declares FocusOffset after FocusObject");
    t.vm.space.heap.get_mut(movie).unwrap().slots.truncate(at + 1);
    t.vm.add_service(TestFocusWrites(Vec::new())).unwrap();
    let e = t.call(FOCUS_PATH, movie, vec![Value::Object(None)]).unwrap_err();
    assert!(matches!(e, VmError::Heap(ow_object::heap::HeapError::SlotMissing { .. })), "{e:?}");
    let wi = t.world_info;
    assert_eq!(t.vm.space.read(movie, object).unwrap(), &Value::Object(Some(wi)), "FocusObject survives");
    assert!(t.vm.service::<TestFocusWrites>().unwrap().borrow().0.is_empty(), "no write");
}

/// The focus clamp's bound (ruling R2): half the margin between the stage height and the stage height times the
/// scale; 0 for the no-player stand-in; a negative or NaN bound is `NotYet { NotRead }`.
#[test]
fn the_focus_bound_is_checked_as_computed() {
    assert_eq!(y_bound(FocusStage::NO_PLAYER).unwrap(), 0.0);
    assert_eq!(y_bound(FocusStage { height: 720.0, scale: 0.5 }).unwrap(), 180.0);
    assert_eq!(y_bound(FocusStage { height: 720.0, scale: 1.0 }).unwrap(), 0.0);
    for bad in [FocusStage { height: 720.0, scale: 2.0 }, FocusStage { height: f32::NAN, scale: 0.5 }] {
        not_yet_in(y_bound(bad).map(Value::Float), false, FOCUS_BOUND, RECORD_TEXT_LIST);
    }
}

/// The status telemetry (ruling R3): nothing, with no write, for a None player and for a player.
#[test]
fn the_status_telemetry_answers_nothing_for_every_player() {
    let mut t = toy(true);
    let service = t.vm.find_class("WillowGame.WillowLeviathanService").unwrap();
    let service = t.vm.space.alloc(service, "ToyService", None, Origin::Runtime).unwrap();
    let name = t.vm.space.names.intern("M_Toy");
    let args = |p: Option<ObjRef>| vec![Value::Object(p), Value::Byte(1), Value::Name(name)];
    assert_eq!(t.call(RECORD_STATUS_CHANGED, service, args(None)).unwrap(), Value::None);
    let pc = t.pc;
    assert_eq!(t.call(RECORD_STATUS_CHANGED, service, args(Some(pc))).unwrap(), Value::None);
}

/// The objective telemetry (ruling R3, NATIVE_RECORD_MISSION_OBJECTIVE_CHANGED.md section 3): nothing, for a None
/// player and for a player, with the heap unchanged (no write, no new object) and nothing logged; a player argument
/// that is not an object is a script error. Its dispatch key is the one the note names. The declared parameters are
/// `ForPlayer, ObjectiveIndex, ObjectiveProgress, PreviousObjectiveProgress, MissionDefinition` (the hub, review of
/// #531); the integers are passed old third, new fourth, as the script does, and nothing reads them.
#[test]
fn the_objective_telemetry_answers_nothing_for_every_player() {
    let mut t = toy(true);
    let f = t.vm.find_function(RECORD_OBJECTIVE_CHANGED).unwrap();
    assert_eq!(
        t.vm.function(f).unwrap().native_key.as_deref(),
        Some("WillowLeviathanService.RecordMissionObjectiveChangedEventForPlayer(object,int,int,int,name)")
    );
    let service = t.vm.find_class("WillowGame.WillowLeviathanService").unwrap();
    let service = t.vm.space.alloc(service, "ToyService", None, Origin::Runtime).unwrap();
    let name = t.vm.space.names.intern("M_Toy");
    let args = |p: Value| vec![p, Value::Int(2), Value::Int(1), Value::Int(3), Value::Name(name)];
    let pc = t.pc;
    // Review r1 of #533, item 2: the whole heap (every object's slots, its load state, every allocation), the RNG
    // streams and the logs are unchanged. TODO(#518): switch to its committed-write, draw and load-transition
    // counters once they are on this base; until then the whole-heap snapshot stands in for them.
    let heap = |t: &Toy| format!("{:?}", t.vm.space.heap);
    let (before_heap, before_rng, before_log) = (heap(&t), t.vm.rng.clone(), t.vm.log.len());
    assert_eq!(t.call(RECORD_OBJECTIVE_CHANGED, service, args(Value::Object(None))).unwrap(), Value::None);
    assert_eq!(t.call(RECORD_OBJECTIVE_CHANGED, service, args(Value::Object(Some(pc)))).unwrap(), Value::None);
    assert_eq!(heap(&t), before_heap, "no write, no allocation and no load, on any object");
    assert!(t.vm.rng == before_rng, "no draw");
    assert_eq!(t.vm.log.len(), before_log, "nothing is logged");
    assert!(t.vm.defs.take_log().is_empty(), "the store logged nothing");
    let e = t.call(RECORD_OBJECTIVE_CHANGED, service, args(Value::Int(7))).unwrap_err();
    assert!(matches!(e, VmError::Script { .. }), "{e:?}");
    // An omitted player argument (no arguments at all) is the VM's ordinary argument error, and writes nothing.
    let before_heap = heap(&t);
    let f = t.vm.find_function(RECORD_OBJECTIVE_CHANGED).unwrap();
    assert!(t.vm.call(f, Some(service), vec![]).is_err(), "an omitted player is refused");
    assert_eq!(heap(&t), before_heap, "the refusal writes nothing");
}

/// `OtherGame.upk` (invented): a `WillowLeviathanService` of another package with the same native and signature, so
/// it shares the dispatch key `WillowLeviathanService.RecordMissionObjectiveChangedEventForPlayer(object,int,int,int,
/// name)` (review r1 of #533, item 1).
fn other_service() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "Function", "IntProperty", "ObjectProperty", "NameProperty"]);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    let service = t.class("WillowLeviathanService", object);
    let spec = FunctionSpec::new(NATIVE, "RecordMissionObjectiveChangedEventForPlayer");
    let params = [
        param("Object", "ForPlayer", 0, object),
        param("Int", "ObjectiveIndex", 0, 0),
        param("Int", "ObjectiveProgress", 0, 0),
        param("Int", "PreviousObjectiveProgress", 0, 0),
        param("Name", "MissionDefinition", 0, 0),
    ];
    t.function(service, "RecordMissionObjectiveChangedEventForPlayer", &params, None, &[], &spec, |_| None);
    t.p.build()
}

/// The cross-package collision (review r1 of #533, item 1): another package's same-key native does not get the
/// objective telemetry's no-op. It is unbound under the VM's stub policy, and nothing is written; the stock
/// declaration still answers nothing. This holds on this base by the body's declaration check, and with #519 by its
/// package-qualified dispatch.
#[test]
fn another_package_s_objective_telemetry_is_not_the_no_op() {
    let other = other_service();
    let mut t = toy_with(true, &[("OtherGame.upk", &other)]);
    let stock = t.vm.find_function(RECORD_OBJECTIVE_CHANGED).unwrap();
    let foreign = t.vm.find_function("OtherGame.WillowLeviathanService.RecordMissionObjectiveChangedEventForPlayer").unwrap();
    assert_ne!(stock, foreign);
    let key = t.vm.function(stock).unwrap().native_key.clone().unwrap();
    assert_eq!(t.vm.function(foreign).unwrap().native_key.as_deref(), Some(key.as_str()), "one dispatch key");
    let name = t.vm.space.names.intern("M_Toy");
    let args = |p: Option<ObjRef>| vec![Value::Object(p), Value::Int(2), Value::Int(1), Value::Int(3), Value::Name(name)];
    let class = t.vm.find_class("OtherGame.WillowLeviathanService").unwrap();
    let service = t.vm.space.alloc(class, "OtherService", None, Origin::Runtime).unwrap();
    let heap = format!("{:?}", t.vm.space.heap);
    let pc = t.pc;
    match t.vm.call(foreign, Some(service), args(Some(pc))) {
        Err(VmError::UnboundNative(k)) => assert_eq!(k, key),
        other => panic!("expected UnboundNative, got {other:?}"),
    }
    assert_eq!(format!("{:?}", t.vm.space.heap), heap, "nothing is written");
    let stock_class = t.vm.find_class("WillowGame.WillowLeviathanService").unwrap();
    let mine = t.vm.space.alloc(stock_class, "ToyService", None, Origin::Runtime).unwrap();
    assert_eq!(t.call(RECORD_OBJECTIVE_CHANGED, mine, args(Some(pc))).unwrap(), Value::None, "the stock one answers");
}

/// Review r2 of #480, L1, after the engine gate's retirement (lane IMPL-PLAYTHROUGH-VIRTUAL): the outer call no longer
/// refuses after the fallback, so no outer mark is left to give. With the world's `WorldInfo`, the tracker branch alone
/// answers (5) and the fallback, with its load of the unloaded definition numbered 27, never runs: the definition stays
/// unloaded. Without it, the VM-dispatched fallback loads the definition and answers 27.
#[test]
fn a_load_inside_the_dispatched_fallback_runs_only_without_the_world() {
    for world in [true, false] {
        let mut t = toy(world);
        let target = t.unloaded_target();
        t.records(&[&[(Some(target), 1)]]);
        if world {
            let tracked = t.mission(true, false, 5);
            t.track(true, &[]);
            let tracker = t.tracker;
            set(&mut t.vm, tracker, "ActiveMission", Value::Object(Some(tracked)));
        }
        let pc = t.pc;
        let got = active_mission_number(&mut t.vm, pc).unwrap();
        assert_eq!(got, if world { 5 } else { 27 }, "world {world}");
        assert_eq!(t.vm.defs.is_unloaded(&t.vm.space, target), world, "world {world}: the fallback's load");
    }
}
