//! `side_missions` on invented classes and structs (AGENTS.md rule 9: invented values; the class, struct, field and enum
//! names are the packages'). The toy `EMissionStatus` puts `MS_Complete` at another position than the installed enum,
//! so a test that passes reads it by name.

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

/// The toy enum: `MS_Complete` is value 1 here (4 in the installed packages).
const STATUSES: [&str; 3] = ["MS_Active", "MS_Complete", "MS_NotStarted"];
const COMPLETE_HERE: u8 = 1;
const ACTIVE_HERE: u8 = 0;

struct Env {
    vm: Vm,
    dir: PathBuf,
    controller: ClassId,
    sub: ClassId,
    expansion: ClassId,
    mission: ClassId,
    entry: StructId,
    playthrough: StructId,
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn decl(vm: &mut Vm, name: &str, kind: PropKind) -> PropertyDecl {
    PropertyDecl::new(vm.space.names.intern(name), kind)
}

fn env() -> Env {
    let dir = std::env::temp_dir().join(format!("ow-mission-side-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let core = {
        let mut t = ScriptToy::new(&[]);
        t.class("Object", 0);
        t.build()
    };
    let files: Vec<(&str, &[u8])> = vec![("Core.upk", core.as_slice())];
    let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, ParityMode::Off).expect("store");
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let object = vm.find_class("Core.Object").unwrap();
    let o = PropKind::Object { class: object };
    let names = STATUSES.iter().map(|n| vm.space.names.intern(n)).collect();
    let statuses = vm.space.classes.define_enum("WillowGame.IMission.EMissionStatus", names).unwrap();
    let props = vec![decl(&mut vm, "MissionDef", o.clone()), decl(&mut vm, "Status", PropKind::Byte { enum_: Some(statuses) })];
    let entry = vm.space.classes.define_struct("WillowGame.IMission.MissionStatusPlayerData", None, props).unwrap();
    let props = vec![decl(&mut vm, "MissionList", PropKind::Array(Box::new(PropKind::Struct(entry))))];
    let playthrough = vm.space.classes.define_struct("WillowGame.WillowPlayerController.MissionPlaythroughData", None, props).unwrap();
    let props = vec![decl(&mut vm, "MissionPlaythroughs", PropKind::Array(Box::new(PropKind::Struct(playthrough))))];
    let controller = vm.space.classes.define(CONTROLLER_CLASS, Some(object), props).unwrap();
    let sub = vm.space.classes.define("WillowGame.ToySubPlayerController", Some(controller), Vec::new()).unwrap();
    let expansion = vm.space.classes.define(EXPANSION_CLASS, Some(object), Vec::new()).unwrap();
    let props = vec![
        decl(&mut vm, "bPlotCritical", PropKind::Bool),
        decl(&mut vm, "bRepeatable", PropKind::Bool),
        decl(&mut vm, "DlcExpansion", o.clone()),
        decl(&mut vm, "MarketingUnlock", o),
    ];
    let mission = vm.space.classes.define("WillowGame.MissionDefinition", Some(object), props).unwrap();
    Env { vm, dir, controller, sub, expansion, mission, entry, playthrough }
}

fn make(e: &mut Env, class: ClassId, name: &str) -> ObjRef {
    e.vm.space.alloc(class, name, None, Origin::Runtime).unwrap()
}

fn put(e: &mut Env, o: ObjRef, name: &str, v: Value) {
    let class = e.vm.space.heap.get(o).unwrap().class;
    let p = e.vm.space.prop(class, name).unwrap();
    e.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
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

/// A mission with the filter's four fields.
fn mission(e: &mut Env, name: &str, plot: bool, repeatable: bool, dlc: Option<ObjRef>, marketing: Option<ObjRef>) -> ObjRef {
    let m = make(e, e.mission, name);
    put(e, m, "bPlotCritical", Value::Bool(plot));
    put(e, m, "bRepeatable", Value::Bool(repeatable));
    put(e, m, "DlcExpansion", Value::Object(dlc));
    put(e, m, "MarketingUnlock", Value::Object(marketing));
    m
}

/// A controller with one playthrough holding `entries` (mission, status).
fn controller(e: &mut Env, entries: &[(ObjRef, u8)]) -> ObjRef {
    let list: Vec<Value> = entries
        .iter()
        .map(|&(m, s)| strukt(e, e.entry, vec![("MissionDef", Value::Object(Some(m))), ("Status", Value::Byte(s))]))
        .collect();
    let playthrough = strukt(e, e.playthrough, vec![("MissionList", Value::array(list))]);
    let c = make(e, e.controller, "PC");
    put(e, c, "MissionPlaythroughs", Value::array(vec![playthrough]));
    c
}

fn answer(e: &Env, c: Option<ObjRef>, x: Option<ObjRef>, index: i32, missions: &[ObjRef]) -> Result<bool, VmError> {
    let list: Vec<Value> = missions.iter().map(|&m| Value::Object(Some(m))).collect();
    match prepare(&e.vm, &Value::Object(c), &Value::Object(x), |_| Ok(index))? {
        Step::Answer(b) => Ok(b),
        Step::Walk(w) => walk(&e.vm, &w, &list),
    }
}

/// Steps 1 and 2: a None controller is false; an argument of another kind, and an expansion of another class, are
/// script errors.
#[test]
fn the_arguments_are_checked_and_a_none_controller_is_false() {
    let mut e = env();
    assert!(!answer(&e, None, None, 0, &[]).unwrap());
    let err = prepare(&e.vm, &Value::Int(3), &Value::Object(None), |_| Ok(0)).unwrap_err();
    assert!(matches!(&err, VmError::Script { function, .. } if function == UNIT), "{err:?}");
    let m = e.mission;
    let not_an_expansion = make(&mut e, m, "NotAnExpansion");
    let err = answer(&e, None, Some(not_an_expansion), 0, &[]).unwrap_err();
    assert!(matches!(&err, VmError::Script { .. }), "{err:?}");
}

/// Step 3: a controller of another class is a script error; a strict subclass is the named `NotRead`.
#[test]
fn the_controller_class_is_checked() {
    let mut e = env();
    let m = e.mission;
    let other = make(&mut e, m, "NotAController");
    assert!(matches!(answer(&e, Some(other), None, 0, &[]).unwrap_err(), VmError::Script { .. }));
    let sub = e.sub;
    let c = make(&mut e, sub, "SubPC");
    match answer(&e, Some(c), None, 0, &[]).unwrap_err() {
        VmError::NotYet(n) => {
            assert_eq!(n.missing, Missing::NotRead);
            assert_eq!(n.what, SUBCLASS);
        }
        other => panic!("{other:?}"),
    }
}

/// Step 4: an index outside `MissionPlaythroughs` is false, without the walk.
#[test]
fn an_index_out_of_range_is_false() {
    let mut e = env();
    let side = mission(&mut e, "Side", false, false, None, None);
    let c = controller(&mut e, &[(side, COMPLETE_HERE)]);
    assert!(!answer(&e, Some(c), None, 1, &[side]).unwrap());
    assert!(!answer(&e, Some(c), None, -1, &[side]).unwrap());
    assert!(answer(&e, Some(c), None, 0, &[side]).unwrap());
}

/// Step 6: the filter skips plot-critical, repeatable, marketing and other-expansion missions; a counted mission
/// needs its first entry complete (by name); none counted is true.
#[test]
fn the_walk_counts_side_missions_of_the_expansion_only() {
    let mut e = env();
    let x = e.expansion;
    let dlc = make(&mut e, x, "Dlc");
    let marketing = make(&mut e, x, "Marketing");
    let plot = mission(&mut e, "Plot", true, false, None, None);
    let repeatable = mission(&mut e, "Repeatable", false, true, None, None);
    let market = mission(&mut e, "Market", false, false, None, Some(marketing));
    let dlc_side = mission(&mut e, "DlcSide", false, false, Some(dlc), None);
    let side = mission(&mut e, "Side", false, false, None, None);
    // The skipped missions have no entry (they would answer false if counted); the base-game side mission is complete.
    let c = controller(&mut e, &[(side, COMPLETE_HERE)]);
    let all = [plot, repeatable, market, dlc_side, side];
    assert!(answer(&e, Some(c), None, 0, &all).unwrap(), "only the base-game side mission counts");
    assert!(!answer(&e, Some(c), Some(dlc), 0, &all).unwrap(), "the expansion's side mission has no entry");
    assert!(answer(&e, Some(c), None, 0, &[]).unwrap(), "none counted");
    assert!(answer(&e, Some(c), None, 0, &[plot]).unwrap(), "none counted");
}

/// Step 6: an entry not complete, or no entry, is false; the first entry of a mission wins; `MS_Complete` is read by
/// name.
#[test]
fn the_entry_must_be_complete_and_the_first_one_wins() {
    let mut e = env();
    let a = mission(&mut e, "A", false, false, None, None);
    let b = mission(&mut e, "B", false, false, None, None);
    let c = controller(&mut e, &[(a, COMPLETE_HERE), (b, ACTIVE_HERE)]);
    assert!(answer(&e, Some(c), None, 0, &[a]).unwrap());
    assert!(!answer(&e, Some(c), None, 0, &[a, b]).unwrap(), "B is active");
    let first_wins = controller(&mut e, &[(a, COMPLETE_HERE), (a, ACTIVE_HERE)]);
    assert!(answer(&e, Some(first_wins), None, 0, &[a]).unwrap(), "the first entry wins");
    let first_active = controller(&mut e, &[(a, ACTIVE_HERE), (a, COMPLETE_HERE)]);
    assert!(!answer(&e, Some(first_active), None, 0, &[a]).unwrap());
    let missing = controller(&mut e, &[(a, COMPLETE_HERE)]);
    assert!(!answer(&e, Some(missing), None, 0, &[a, b]).unwrap(), "B has no entry");
    // The installed number 4 is not this enum's MS_Complete.
    let four = controller(&mut e, &[(a, 4)]);
    assert!(!answer(&e, Some(four), None, 0, &[a]).unwrap(), "read by name, not as 4");
}

/// Step 6: a None mission in the list is skipped.
#[test]
fn a_none_mission_is_skipped() {
    let mut e = env();
    let a = mission(&mut e, "A", false, false, None, None);
    let c = controller(&mut e, &[(a, COMPLETE_HERE)]);
    let step = prepare(&e.vm, &Value::Object(Some(c)), &Value::Object(None), |_| Ok(0)).unwrap();
    let Step::Walk(w) = step else { panic!() };
    assert!(walk(&e.vm, &w, &[Value::Object(None), Value::Object(Some(a))]).unwrap());
}
