//! `balance_refresh` on invented classes and structs (AGENTS.md rule 9: invented values; the class, struct and field
//! names are the packages'), with a small invented package for unloaded handles. AI-assisted (Claude Code).

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::ParityMode;
use ow_core::notyet::Missing;
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, ObjRef, PropAddr, StructId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_package::testkit::script::ScriptToy;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::*;

static NEXT: AtomicU32 = AtomicU32::new(0);

struct Env {
    vm: Vm,
    dir: PathBuf,
    controller: ClassId,
    sub: ClassId,
    mission: ClassId,
    expansion: ClassId,
    globals: ClassId,
    balance: ClassId,
    region: ClassId,
    stage: StructId,
    playthrough: StructId,
    by_region: StructId,
    adjustment: StructId,
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn decl(vm: &mut Vm, name: &str, kind: PropKind) -> PropertyDecl {
    PropertyDecl::new(vm.space.names.intern(name), kind)
}

/// `Core.upk` (the object class), `ToyLazy.upk` (a holder class with two object fields) and `GD_ToyBalance.upk` (a
/// holder naming two exports the store keeps unloaded: `Balance_Lazy` and `M_Lazy`).
fn packages() -> Vec<(&'static str, Vec<u8>)> {
    let core = {
        let mut t = ScriptToy::new(&[]);
        t.class("Object", 0);
        t.build()
    };
    let lazy = {
        let mut t = ScriptToy::with_imports(&["Class", "ObjectProperty"]);
        let core = t.p.add_import("Core", "Package", 0, "Core");
        let object = t.p.add_import("Core", "Class", core, "Object");
        t.class("ToyThing", object);
        let holder = t.class("ToyHolder", object);
        t.prop("Object", holder, "Balance", 0, object);
        t.prop("Object", holder, "Mission", 0, object);
        t.build()
    };
    let gd = {
        let mut t = ScriptToy::with_imports(&["Class"]);
        let package = t.p.add_import("Core", "Package", 0, "ToyLazy");
        let thing = t.p.add_import("Core", "Class", package, "ToyThing");
        let holder = t.p.add_import("Core", "Class", package, "ToyHolder");
        let mut empty = ow_package::testkit::ints(&[0]);
        empty.extend_from_slice(&t.none);
        let b = t.p.add_export(thing, "Balance_Lazy", empty.clone());
        let m = t.p.add_export(thing, "M_Lazy", empty);
        let mut payload = ow_package::testkit::ints(&[0]);
        payload.extend(t.p.object_tag("Balance", b));
        payload.extend(t.p.object_tag("Mission", m));
        payload.extend_from_slice(&t.none);
        t.p.add_export(holder, "Holder", payload);
        t.build()
    };
    vec![("Core.upk", core), ("ToyLazy.upk", lazy), ("GD_ToyBalance.upk", gd)]
}

fn env() -> Env {
    let dir = std::env::temp_dir().join(format!("ow-mission-balance-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let files = packages();
    let borrowed: Vec<(&str, &[u8])> = files.iter().map(|(n, b)| (*n, b.as_slice())).collect();
    let defs = DefStore::from_synthetic(&dir, &borrowed, IdentityPolicy::FirstInScanOrder, ParityMode::Off).expect("store");
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let object = vm.find_class("Core.Object").unwrap();
    let o = PropKind::Object { class: object };
    let objects = PropKind::Array(Box::new(o.clone()));
    let props = vec![decl(&mut vm, "RegionDef", o.clone()), decl(&mut vm, "GameStage", PropKind::Int), decl(&mut vm, "PlaythroughIdx", PropKind::Int)];
    let stage = vm.space.classes.define_struct("WillowGame.PlayerSaveGame.RegionGameStageData", None, props).unwrap();
    let props = vec![decl(&mut vm, "RegionGameStages", PropKind::Array(Box::new(PropKind::Struct(stage))))];
    let controller = vm.space.classes.define(CONTROLLER_CLASS, Some(object), props).unwrap();
    let sub = vm.space.classes.define("WillowGame.ToySubPlayerController", Some(controller), Vec::new()).unwrap();
    let props = vec![decl(&mut vm, "PlayThroughNumber", PropKind::Int), decl(&mut vm, "BalanceDefinitions", objects)];
    let playthrough = vm.space.classes.define_struct("WillowGame.GlobalsDefinition.PlayThroughData", None, props).unwrap();
    let table = || PropKind::Array(Box::new(PropKind::Struct(playthrough)));
    let props = vec![decl(&mut vm, "RegionBalanceData", table())];
    let expansion = vm.space.classes.define("WillowGame.DownloadableExpansionDefinition", Some(object), props).unwrap();
    let props = vec![decl(&mut vm, "RegionBalanceData", table())];
    let globals = vm.space.classes.define("WillowGame.GlobalsDefinition", Some(object), props).unwrap();
    let props = vec![decl(&mut vm, "DlcExpansion", o.clone())];
    let mission = vm.space.classes.define(MISSION_CLASS, Some(object), props).unwrap();
    let region = vm.space.classes.define("WillowGame.RegionDefinition", Some(object), Vec::new()).unwrap();
    let props = vec![decl(&mut vm, "Mission", o.clone())];
    let adjustment = vm.space.classes.define_struct("WillowGame.GameBalanceDefinition.MissionStateBalanceAdjustment", None, props).unwrap();
    let props = vec![decl(&mut vm, "Region", o), decl(&mut vm, "MissionOverrides", PropKind::Array(Box::new(PropKind::Struct(adjustment))))];
    let by_region = vm.space.classes.define_struct("WillowGame.GameBalanceDefinition.RegionBalanceData", None, props).unwrap();
    let props = vec![decl(&mut vm, "BalanceByRegion", PropKind::Array(Box::new(PropKind::Struct(by_region))))];
    let balance = vm.space.classes.define("WillowGame.GameBalanceDefinition", Some(object), props).unwrap();
    Env { vm, dir, controller, sub, mission, expansion, globals, balance, region, stage, playthrough, by_region, adjustment }
}

fn make(e: &mut Env, class: ClassId, name: &str) -> ObjRef {
    e.vm.space.alloc(class, name, None, Origin::Runtime).unwrap()
}

fn put(e: &mut Env, o: ObjRef, name: &str, v: Value) {
    let class = e.vm.space.heap.get(o).unwrap().class;
    let p = e.vm.space.prop(class, name).unwrap();
    e.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
}

fn get(e: &Env, o: ObjRef, name: &str) -> Value {
    let class = e.vm.space.heap.get(o).unwrap().class;
    let p = e.vm.space.prop(class, name).unwrap();
    e.vm.space.read(o, PropAddr::scalar(p)).unwrap().clone()
}

fn strukt(e: &Env, s: StructId, fields: Vec<(&str, Value)>) -> Value {
    let mut v = e.vm.space.classes.struct_defaults(s).unwrap();
    for (n, x) in fields {
        let id = e.vm.space.names.find(n).unwrap();
        let (i, _) = e.vm.space.classes.struct_layout(s).unwrap().field(id).unwrap();
        v.struct_mut().unwrap().fields[i as usize] = x;
    }
    v
}

/// A balance definition with one region record per `(region, missions named by its overrides)`.
fn balance(e: &mut Env, name: &str, regions: &[(ObjRef, &[ObjRef])]) -> ObjRef {
    let records: Vec<Value> = regions
        .iter()
        .map(|&(r, ms)| {
            let overrides = ms.iter().map(|&m| strukt(e, e.adjustment, vec![("Mission", Value::Object(Some(m)))])).collect();
            strukt(e, e.by_region, vec![("Region", Value::Object(Some(r))), ("MissionOverrides", Value::array(overrides))])
        })
        .collect();
    let b = make(e, e.balance, name);
    put(e, b, "BalanceByRegion", Value::array(records));
    b
}

/// `owner`'s `RegionBalanceData`: one entry per balance-definition list, with `PlayThroughNumber` `number`.
fn table(e: &mut Env, owner: ObjRef, entries: &[(i32, &[Option<ObjRef>])]) {
    let v: Vec<Value> = entries
        .iter()
        .map(|&(n, bs)| {
            let list = bs.iter().map(|&b| Value::Object(b)).collect();
            strukt(e, e.playthrough, vec![("PlayThroughNumber", Value::Int(n)), ("BalanceDefinitions", Value::array(list))])
        })
        .collect();
    put(e, owner, "RegionBalanceData", Value::array(v));
}

/// A controller whose `RegionGameStages` holds `(region, stage, playthrough)` records.
fn controller(e: &mut Env, records: &[(ObjRef, i32, i32)]) -> ObjRef {
    let v: Vec<Value> = records
        .iter()
        .map(|&(r, s, p)| {
            strukt(e, e.stage, vec![("RegionDef", Value::Object(Some(r))), ("GameStage", Value::Int(s)), ("PlaythroughIdx", Value::Int(p))])
        })
        .collect();
    let c = make(e, e.controller, "PC");
    put(e, c, "RegionGameStages", Value::array(v));
    c
}

/// The `(region, stage)` pairs of a `RegionGameStages` value.
fn stages(_e: &Env, v: &Value) -> Vec<(ObjRef, i32)> {
    v.elements()
        .unwrap()
        .iter()
        .map(|r| {
            let s = r.as_struct().unwrap();
            let Value::Object(Some(region)) = s.fields[0] else { panic!() };
            let Value::Int(stage) = s.fields[1] else { panic!() };
            (region, stage)
        })
        .collect()
}

struct World {
    pc: ObjRef,
    mission: ObjRef,
    other: ObjRef,
    definition: ObjRef,
    zone: ObjRef,
    elsewhere: ObjRef,
}

/// The slice's shape: a globals definition whose playthrough entry 0 names one balance definition with two regions;
/// `zone`'s overrides name `other` (not the mission), `elsewhere`'s name nobody.
fn world(e: &mut Env) -> World {
    let mission = make(e, e.mission, "M_Fire");
    let other = make(e, e.mission, "M_Other");
    let (zone, elsewhere) = (make(e, e.region, "Zone"), make(e, e.region, "Elsewhere"));
    let b = balance(e, "Balance_P1", &[(zone, &[other]), (elsewhere, &[])]);
    let definition = make(e, e.globals, "Globals");
    table(e, definition, &[(1, &[Some(b)])]);
    let pc = controller(e, &[(zone, 7, 0), (elsewhere, 3, 0)]);
    World { pc, mission, other, definition, zone, elsewhere }
}

/// The slice's case (note section 4): no override names the mission, so false and no write.
#[test]
fn no_override_naming_the_mission_is_false_with_no_write() {
    let mut e = env();
    let w = world(&mut e);
    let out = plan(&e.vm, w.pc, &Value::Object(Some(w.mission)), Some(w.definition), 0).unwrap();
    assert_eq!(out, Outcome { matched: false, stages: None });
}

/// A matching override: true, and the first record of its region for the current playthrough is removed; a record of
/// that region for another playthrough is passed over; other regions are kept.
#[test]
fn a_matching_override_removes_the_regions_record_for_the_playthrough() {
    let mut e = env();
    let w = world(&mut e);
    let pc = controller(&mut e, &[(w.zone, 9, 1), (w.zone, 7, 0), (w.elsewhere, 3, 0), (w.zone, 5, 0)]);
    let out = plan(&e.vm, pc, &Value::Object(Some(w.other)), Some(w.definition), 0).unwrap();
    assert!(out.matched);
    assert_eq!(stages(&e, out.stages.as_ref().unwrap()), vec![(w.zone, 9), (w.elsewhere, 3), (w.zone, 5)]);
    // No record of the region for the playthrough: true, nothing to write.
    let pc = controller(&mut e, &[(w.zone, 9, 1)]);
    assert_eq!(plan(&e.vm, pc, &Value::Object(Some(w.other)), Some(w.definition), 0).unwrap(), Outcome { matched: true, stages: None });
}

/// One reset per matching override, duplicates included (the hub's heads-up on #559): two overrides naming the mission
/// in the same region remove two records, each against the array the first left.
#[test]
fn every_matching_override_resets_once() {
    let mut e = env();
    let w = world(&mut e);
    let b = balance(&mut e, "Balance_Dup", &[(w.zone, &[w.mission, w.mission])]);
    table(&mut e, w.definition, &[(1, &[Some(b)])]);
    let pc = controller(&mut e, &[(w.zone, 7, 0), (w.zone, 5, 0), (w.zone, 4, 0)]);
    let out = plan(&e.vm, pc, &Value::Object(Some(w.mission)), Some(w.definition), 0).unwrap();
    assert_eq!(stages(&e, out.stages.as_ref().unwrap()), vec![(w.zone, 4)]);
}

/// The table: the expansion's when the mission names one; the entry by position, `PlayThroughNumber` not compared; an
/// index past the table is false, with no clamp to the last entry.
#[test]
fn the_table_and_its_entry_by_position() {
    let mut e = env();
    let w = world(&mut e);
    let b = balance(&mut e, "Balance_Dlc", &[(w.zone, &[w.mission])]);
    let xc = e.expansion;
    let x = make(&mut e, xc, "Dlc");
    table(&mut e, x, &[(42, &[Some(b)])]);
    put(&mut e, w.mission, "DlcExpansion", Value::Object(Some(x)));
    let out = plan(&e.vm, w.pc, &Value::Object(Some(w.mission)), Some(w.definition), 0).unwrap();
    assert!(out.matched, "the expansion's table, its entry 0 whatever its PlayThroughNumber");
    assert_eq!(stages(&e, out.stages.as_ref().unwrap()), vec![(w.elsewhere, 3)]);
    assert_eq!(plan(&e.vm, w.pc, &Value::Object(Some(w.mission)), Some(w.definition), 1).unwrap(), Outcome { matched: false, stages: None });
}

/// The refusals and the plain answers of the preflight.
#[test]
fn the_preflight_checks() {
    let mut e = env();
    let w = world(&mut e);
    let m = Value::Object(Some(w.mission));
    // The receiver.
    assert!(matches!(plan(&e.vm, w.mission, &m, Some(w.definition), 0), Err(VmError::Script { .. })));
    let sub = e.sub;
    let s = make(&mut e, sub, "SubPC");
    put(&mut e, s, "RegionGameStages", Value::array(vec![]));
    match plan(&e.vm, s, &m, Some(w.definition), 0).unwrap_err() {
        VmError::NotYet(n) => assert_eq!((n.missing, n.what.as_ref()), (Missing::NotRead, SUBCLASS)),
        other => panic!("{other:?}"),
    }
    match plan(&e.vm, w.pc, &m, Some(w.definition), -1).unwrap_err() {
        VmError::NotYet(n) => assert_eq!((n.missing, n.what.as_ref()), (Missing::NotRead, NEGATIVE)),
        other => panic!("{other:?}"),
    }
    // The mission.
    assert!(matches!(plan(&e.vm, w.pc, &Value::Object(None), Some(w.definition), 0), Err(VmError::Script { .. })));
    assert!(matches!(plan(&e.vm, w.pc, &Value::Object(Some(w.zone)), Some(w.definition), 0), Err(VmError::Script { .. })));
    assert!(matches!(plan(&e.vm, w.pc, &Value::Int(1), Some(w.definition), 0), Err(VmError::Script { .. })));
    // No globals definition: false.
    assert_eq!(plan(&e.vm, w.pc, &m, None, 0).unwrap(), Outcome { matched: false, stages: None });
    // A None BalanceDefinitions entry.
    table(&mut e, w.definition, &[(1, &[None])]);
    assert!(matches!(plan(&e.vm, w.pc, &m, Some(w.definition), 0), Err(VmError::Script { .. })));
    // A RegionGameStages record of another shape, reached by a reset.
    let b = balance(&mut e, "Balance_M", &[(w.zone, &[w.mission])]);
    table(&mut e, w.definition, &[(1, &[Some(b)])]);
    let bad = strukt(&e, e.adjustment, vec![("Mission", Value::Object(None))]);
    let pc = e.controller;
    let pc = make(&mut e, pc, "BadPC");
    let class = e.vm.space.heap.get(pc).unwrap().class;
    let p = e.vm.space.prop(class, "RegionGameStages").unwrap();
    *e.vm.space.slot_mut(pc, PropAddr::scalar(p)).unwrap() = Value::array(vec![bad]);
    assert!(matches!(plan(&e.vm, pc, &m, Some(w.definition), 0), Err(VmError::Script { .. })));
    let _ = get(&e, w.pc, "RegionGameStages");
}

/// An unloaded balance definition is the named `LayerAbsent` and nothing is loaded; an override naming an unloaded
/// mission handle is compared by identity and never resolved.
#[test]
fn unloaded_handles_load_nothing() {
    let mut e = env();
    let w = world(&mut e);
    let path = ow_core::ObjectPath::parse("GD_ToyBalance.Holder").unwrap();
    let holder = e.vm.defs.find(&mut e.vm.space, &path).unwrap().unwrap();
    let Value::Object(Some(lazy_balance)) = get(&e, holder, "Balance") else { panic!() };
    let Value::Object(Some(lazy_mission)) = get(&e, holder, "Mission") else { panic!() };
    assert!(e.vm.defs.is_unloaded(&e.vm.space, lazy_balance) && e.vm.defs.is_unloaded(&e.vm.space, lazy_mission));
    // An override naming the unloaded mission, beside one naming the completed mission.
    let b = balance(&mut e, "Balance_L", &[(w.zone, &[lazy_mission]), (w.elsewhere, &[w.mission])]);
    table(&mut e, w.definition, &[(1, &[Some(b)])]);
    let heap = format!("{:?}", e.vm.space.heap);
    let out = plan(&e.vm, w.pc, &Value::Object(Some(w.mission)), Some(w.definition), 0).unwrap();
    assert_eq!(stages(&e, out.stages.as_ref().unwrap()), vec![(w.zone, 7)], "only the identity match resets");
    assert!(e.vm.defs.is_unloaded(&e.vm.space, lazy_mission), "never resolved");
    // The unloaded balance definition.
    table(&mut e, w.definition, &[(1, &[Some(b), Some(lazy_balance)])]);
    let heap_before = format!("{:?}", e.vm.space.heap);
    match plan(&e.vm, w.pc, &Value::Object(Some(w.mission)), Some(w.definition), 0).unwrap_err() {
        VmError::NotYet(n) => {
            assert!(matches!(&n.missing, Missing::LayerAbsent(l) if *l == BALANCE_LAYER), "{n}");
            assert_eq!(n.what, unloaded_balance("GD_ToyBalance.Balance_Lazy"));
        }
        other => panic!("{other:?}"),
    }
    assert!(e.vm.defs.is_unloaded(&e.vm.space, lazy_balance), "nothing loaded");
    assert_eq!(format!("{:?}", e.vm.space.heap), heap_before, "the heap unchanged");
    let _ = (heap, w.other);
}
