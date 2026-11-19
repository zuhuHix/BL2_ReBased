//! Tests of the tracker's list build (lane IMPL-MISSIONLIST-BUILD) on invented packages (AGENTS.md rule 9): a toy
//! `WillowGame` (an `Object`, `MissionDefinition` with the fields the build reads, `DownloadableExpansionDefinition`, a
//! `MissionTracker` with its `MissionList`) and a toy `GD_ToyMissions` holding the missions. Every value is invented.
//! AI-assisted (Claude Code).

use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::registry::AttributeSystem;
use ow_core::{Missing, ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::ints;
use ow_package::testkit::reflect::{toy_array_of, toy_struct};
use ow_package::testkit::script::ScriptToy;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::*;

static NEXT: AtomicU32 = AtomicU32::new(0);

fn willow() -> Vec<u8> {
    let mut t = ScriptToy::new(&["Int", "Byte", "Bool", "Object", "Array", "Struct"]);
    let object = t.class("Object", 0);
    let mission = t.class("MissionDefinition", object);
    for name in ["bPlotCritical", "bGameStageLocked"] {
        t.prop("Bool", mission, name, 0, 0);
    }
    for name in ["MissionNumber", "GameStage"] {
        t.prop("Int", mission, name, 0, 0);
    }
    t.prop("Object", mission, "NextMissionInChain", 0, mission);
    t.prop("Object", mission, "DlcExpansion", 0, object);
    toy_array_of(&mut t, mission, "Dependencies", "Object", mission);
    let expansion = t.class("DownloadableExpansionDefinition", object);
    t.prop("Int", expansion, "MissionNumberBase", 0, 0);
    let tracker = t.class("MissionTracker", object);
    let data = toy_struct(&mut t, tracker, "MissionData", None);
    t.prop("Object", data, "MissionDef", 0, mission);
    t.prop("Byte", data, "Status", 0, 0);
    toy_array_of(&mut t, tracker, "MissionList", "Struct", data);
    t.p.build()
}

/// One invented mission: its name, whether it is plot-critical, its dependencies and next chain member (by name, in
/// this package), its expansion (by name) and whether it starts with non-zero values in the reset fields.
struct M {
    name: &'static str,
    plot: bool,
    deps: &'static [&'static str],
    next: Option<&'static str>,
    dlc: Option<&'static str>,
}

const fn m(name: &'static str, plot: bool, deps: &'static [&'static str], next: Option<&'static str>) -> M {
    M { name, plot, deps, next, dlc: None }
}

/// `GD_ToyMissions.upk`: the missions in the given order (exports 1..), then the expansions named by any mission,
/// each with `MissionNumberBase` 100. Every mission starts locked, numbered 7 and at game stage 9 (the resets' check).
fn mission_package(list: &[M]) -> Vec<u8> {
    let mut t = ScriptToy::new(&["Int", "Bool", "Object", "Array"]);
    let willow = t.p.add_import("Core", "Package", 0, "WillowGame");
    let class = t.p.add_import("Core", "Class", willow, "MissionDefinition");
    let expansion_class = t.p.add_import("Core", "Class", willow, "DownloadableExpansionDefinition");
    let mut expansions: Vec<&str> = Vec::new();
    for x in list {
        if let Some(e) = x.dlc
            && !expansions.contains(&e)
        {
            expansions.push(e);
        }
    }
    let index = |name: &str| -> i32 {
        if let Some(i) = list.iter().position(|x| x.name == name) {
            return i32::try_from(i).unwrap() + 1;
        }
        let e = expansions.iter().position(|e| *e == name).expect("a named object");
        i32::try_from(list.len() + e).unwrap() + 1
    };
    for x in list {
        let mut payload = ints(&[0]);
        payload.extend(t.p.tag_detail("bPlotCritical", "BoolProperty", &[u8::from(x.plot)], &[]));
        payload.extend(t.p.tag_detail("bGameStageLocked", "BoolProperty", &[1], &[]));
        payload.extend(t.p.tag("MissionNumber", "IntProperty", &ints(&[7])));
        payload.extend(t.p.tag("GameStage", "IntProperty", &ints(&[9])));
        if !x.deps.is_empty() {
            let mut refs = vec![i32::try_from(x.deps.len()).unwrap()];
            refs.extend(x.deps.iter().map(|d| index(d)));
            payload.extend(t.p.tag("Dependencies", "ArrayProperty", &ints(&refs)));
        }
        if let Some(n) = x.next {
            payload.extend(t.p.tag("NextMissionInChain", "ObjectProperty", &ints(&[index(n)])));
        }
        if let Some(e) = x.dlc {
            payload.extend(t.p.tag("DlcExpansion", "ObjectProperty", &ints(&[index(e)])));
        }
        payload.extend_from_slice(&t.none);
        t.p.add_export(class, x.name, payload);
    }
    for e in &expansions {
        let mut payload = ints(&[0]);
        payload.extend(t.p.tag("MissionNumberBase", "IntProperty", &ints(&[100])));
        payload.extend_from_slice(&t.none);
        t.p.add_export(expansion_class, e, payload);
    }
    t.p.build()
}

struct Toy {
    vm: Vm,
    tracker: ObjRef,
    dir: std::path::PathBuf,
}

impl Drop for Toy {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// The world: both packages, the missions' package loaded (as the set-up's loads would leave it), and a tracker.
fn toy(list: &[M]) -> Toy {
    let dir = std::env::temp_dir().join(format!("ow-mission-list-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let (w, g) = (willow(), mission_package(list));
    let files: [(&str, &[u8]); 2] = [("WillowGame.upk", &w), ("GD_ToyMissions.upk", &g)];
    let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, ParityMode::Off).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let c = vm.find_class("WillowGame.MissionTracker").unwrap();
    let tracker = ow_vm::interp::instantiate_from_defaults(&mut vm, c).unwrap();
    // Load the missions' package by asking for its first export.
    let first = ObjectPath::parse(&format!("GD_ToyMissions.{}", list[0].name)).unwrap();
    vm.defs.find(&mut vm.space, &first).unwrap().unwrap();
    Toy { vm, tracker, dir }
}

impl Toy {
    fn get(&mut self, name: &str) -> ObjRef {
        let p = ObjectPath::parse(&format!("GD_ToyMissions.{name}")).unwrap();
        self.vm.defs.find(&mut self.vm.space, &p).unwrap().unwrap()
    }

    fn read(&self, o: ObjRef, field: &str) -> Value {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, field).unwrap();
        self.vm.space.read(o, PropAddr::scalar(p)).unwrap().clone()
    }

    fn number(&mut self, name: &str) -> i32 {
        let o = self.get(name);
        match self.read(o, "MissionNumber") {
            Value::Int(i) => i,
            other => panic!("{other:?}"),
        }
    }

    fn build(&mut self) -> Result<(), VmError> {
        let mut built = false;
        build(&mut self.vm, self.tracker, &mut built)?;
        assert!(built);
        Ok(())
    }

    fn listed(&self) -> Vec<Value> {
        missions(&self.vm, self.tracker).unwrap()
    }
}

/// The main chain is numbered from 0 along `NextMissionInChain`; side missions follow, by the bucket of their anchor
/// (bucket 0 first) and in placement order inside one; a plot-critical mission off the chain and a side mission with
/// no plot-critical mission behind it stay -1.
#[test]
fn the_main_chain_then_the_side_missions_by_bucket() {
    let list = [
        m("Story0", true, &[], Some("Story1")),
        m("Story1", true, &["Story0"], Some("Story2")),
        m("Story2", true, &["Story1"], None),
        m("OffChain", true, &["Story2"], None),
        m("SideA", false, &["Story1"], None),
        m("SideB", false, &["SideA"], None),
        m("SideEarly", false, &["Story0"], None),
        m("Loose", false, &[], None),
    ];
    let mut t = toy(&list);
    t.build().unwrap();
    for (name, n) in [("Story0", 0), ("Story1", 1), ("Story2", 2), ("SideEarly", 3), ("SideA", 4), ("SideB", 5), ("OffChain", -1), ("Loose", -1)] {
        assert_eq!(t.number(name), n, "{name}");
    }
}

/// The resets: every definition of the set starts unlocked at game stage 0; the record of each is zero with its
/// `MissionDef`; the list holds each definition once, in first-insertion name order (hub ruling 3).
#[test]
fn the_resets_the_records_and_the_order() {
    let list = [m("Zed", false, &[], None), m("Alpha", false, &[], None), m("Mid", false, &[], None)];
    let mut t = toy(&list);
    t.build().unwrap();
    for x in &list {
        let o = t.get(x.name);
        assert_eq!(t.read(o, "bGameStageLocked"), Value::Bool(false), "{}", x.name);
        assert_eq!(t.read(o, "GameStage"), Value::Int(0), "{}", x.name);
        assert_eq!(t.read(o, "MissionNumber"), Value::Int(-1), "{}: unanchored", x.name);
    }
    // The expected order: the names' first appearance in the loaded packages' name tables, in load order.
    let store = t.vm.defs.packages();
    let mut seen: Vec<String> = Vec::new();
    for id in 0..store.loaded_count() {
        for n in &store.get(u32::try_from(id).unwrap()).names {
            if !seen.iter().any(|s| s.eq_ignore_ascii_case(n)) {
                seen.push(n.clone());
            }
        }
    }
    let mut want: Vec<&str> = list.iter().map(|x| x.name).collect();
    want.sort_by_key(|n| seen.iter().position(|s| s == n).unwrap());
    let want: Vec<Value> = want.iter().map(|n| Value::Object(Some(t.get(n)))).collect();
    assert_eq!(t.listed(), want);
    let records = t.read(t.tracker, "MissionList");
    for r in records.elements().unwrap() {
        let s = r.as_struct().unwrap();
        assert!(s.fields.contains(&Value::Byte(0)), "status 0");
    }
}

/// An expansion's group counts from its `MissionNumberBase`, apart from the base game's, when set-up has loaded the
/// expansion. Not loaded (hub ruling 2 loads mission definitions only), it is the named `NotRead`, marked since it comes
/// after the set's loads (which count as writes), and before any field write.
#[test]
fn an_expansion_group_counts_from_its_base() {
    let list = [
        m("Story0", true, &[], None),
        M { name: "DlcStory", plot: true, deps: &[], next: None, dlc: Some("Pack") },
        M { name: "DlcSide", plot: false, deps: &["DlcStory"], next: None, dlc: Some("Pack") },
    ];
    let mut t = toy(&list);
    match t.build() {
        Err(VmError::NotYet(n)) => {
            assert!(n.missing == Missing::NotRead && n.what == format!("stopped after partial writes: {NEEDS_LOAD}"), "{n}")
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(t.number("Story0"), 7, "nothing written");
    let mut t = toy(&list);
    t.get("Pack");
    t.build().unwrap();
    assert_eq!(t.number("Story0"), 0);
    assert_eq!(t.number("DlcStory"), 100);
    assert_eq!(t.number("DlcSide"), 101);
}

/// Bad data before any write: a cycle among side missions, a chain loop. Nothing is written.
#[test]
fn cycles_are_bad_data_found_before_any_write() {
    for list in [
        vec![m("Story0", true, &[], None), m("SideX", false, &["SideY"], None), m("SideY", false, &["SideX"], None)],
        vec![m("Story0", true, &[], Some("Story1")), m("Story1", true, &["Story0"], Some("Story0"))],
    ] {
        let mut t = toy(&list);
        match t.build() {
            Err(VmError::Script { message, .. }) => assert!(message.contains("loop") || message.contains("cycle"), "{message}"),
            other => panic!("{other:?}"),
        }
        let o = t.get(list[0].name);
        assert_eq!(t.read(o, "MissionNumber"), Value::Int(7), "nothing written");
        assert_eq!(t.read(t.tracker, "MissionList").elements().map(<[Value]>::len), Some(0));
    }
}

/// A second build in one world is `NotYet { NotRead }` (hub ruling 1).
#[test]
fn a_second_build_is_not_read() {
    let mut t = toy(&[m("Story0", true, &[], None)]);
    let mut built = true;
    match build(&mut t.vm, t.tracker, &mut built) {
        Err(VmError::NotYet(n)) => assert!(n.missing == Missing::NotRead && n.what == SECOND_BUILD && n.record == RECORD, "{n}"),
        other => panic!("{other:?}"),
    }
}

/// The second pass (section 3, step 6): a mission with two dependencies gets those that no other mission of the bucket
/// depends on moved to stand just before it, in their old order. X, Y, Z join bucket 0 in that order and W (after X
/// and Z) after Z; the pass moves X and Z before W: Y, X, Z, W.
#[test]
fn the_second_pass_moves_a_missions_own_dependencies_before_it() {
    let list = [
        m("Story0", true, &[], None),
        m("SideX", false, &["Story0"], None),
        m("SideY", false, &["Story0"], None),
        m("SideZ", false, &["Story0"], None),
        m("SideW", false, &["SideX", "SideZ"], None),
    ];
    let mut t = toy(&list);
    t.build().unwrap();
    for (name, n) in [("Story0", 0), ("SideY", 1), ("SideX", 2), ("SideZ", 3), ("SideW", 4)] {
        assert_eq!(t.number(name), n, "{name}");
    }
}

/// The hub's steer: two definitions with the same object name and number (here in two loaded packages) are
/// `NotYet { NotRead }`, "the visit order of equal mission names", before any write.
#[test]
fn equal_mission_names_are_not_read() {
    let dir = std::env::temp_dir().join(format!("ow-mission-list-tie-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let list = [m("Twin", false, &[], None)];
    let (w, a, b) = (willow(), mission_package(&list), mission_package(&list));
    let files: [(&str, &[u8]); 3] = [("WillowGame.upk", &w), ("GD_ToyMissions.upk", &a), ("GD_OtherMissions.upk", &b)];
    let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, ParityMode::Off).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let c = vm.find_class("WillowGame.MissionTracker").unwrap();
    let tracker = ow_vm::interp::instantiate_from_defaults(&mut vm, c).unwrap();
    for p in ["GD_ToyMissions.Twin", "GD_OtherMissions.Twin"] {
        vm.defs.find(&mut vm.space, &ObjectPath::parse(p).unwrap()).unwrap().unwrap();
    }
    let mut built = false;
    match build(&mut vm, tracker, &mut built) {
        Err(VmError::NotYet(n)) => assert!(n.missing == Missing::NotRead && n.what == TIE, "{n}"),
        other => panic!("{other:?}"),
    }
    assert!(built, "the build ran (and refused): a later request is the second-build refusal");
    match build(&mut vm, tracker, &mut built) {
        Err(VmError::NotYet(n)) => assert_eq!(n.what, SECOND_BUILD),
        other => panic!("{other:?}"),
    }
    let class = vm.space.heap.get(tracker).unwrap().class;
    let p = vm.space.prop(class, "MissionList").unwrap();
    assert_eq!(vm.space.read(tracker, PropAddr::scalar(p)).unwrap().elements().map(<[Value]>::len), Some(0));
    let _ = std::fs::remove_dir_all(&dir);
}

/// The second pass's edge (section 3, step 6): SideK is SideM's dependency and stands after it in the bucket, so the
/// dry run refuses with `NotYet { NotRead }` before any field write. Bucket 0 fills A, M, Z, then K after Z.
#[test]
fn a_dependency_after_its_mission_is_not_read_before_any_write() {
    let list = [
        m("Story0", true, &[], None),
        m("SideA", false, &["Story0"], None),
        m("SideM", false, &["SideA", "SideK"], None),
        m("SideZ", false, &["Story0"], None),
        m("SideK", false, &["SideZ"], None),
    ];
    let mut t = toy(&list);
    match t.build() {
        Err(VmError::NotYet(n)) => {
            assert!(n.missing == Missing::NotRead && n.what == format!("stopped after partial writes: {EDGE}"), "{n}")
        }
        other => panic!("{other:?}"),
    }
    for x in &list {
        assert_eq!(t.number(x.name), 7, "{}: nothing written", x.name);
    }
    assert_eq!(t.read(t.tracker, "MissionList").elements().map(<[Value]>::len), Some(0));
}

/// Review r1 of #496, item 2: a loaded `Dependencies` target that is not a `MissionDefinition` is bad data naming the
/// field, before any field write.
#[test]
fn a_dependency_that_is_not_a_mission_is_bad_data() {
    let list = [
        m("Story0", true, &[], None),
        m("SideOdd", false, &[], None),
        M { name: "DlcStory", plot: true, deps: &[], next: None, dlc: Some("Pack") },
    ];
    let mut t = toy(&list);
    // The store's decoder drops a reference of the wrong class, so the slot is set past the class check (an invented
    // inconsistency the check guards against).
    let pack = t.get("Pack");
    let odd = t.get("SideOdd");
    let class = t.vm.space.heap.get(odd).unwrap().class;
    let p = t.vm.space.prop(class, "Dependencies").unwrap();
    *t.vm.space.slot_mut(odd, PropAddr::scalar(p)).unwrap() = Value::array(vec![Value::Object(Some(pack))]);
    match t.build() {
        Err(VmError::Script { message, .. }) => assert!(message.contains("Dependencies"), "{message}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(t.number("Story0"), 7, "nothing written");
}

/// Review r2 of #496, item 3: the same object path listed twice (one package listed twice), folded for case, is one
/// candidate, so it is not a tie; two different paths with the same name stay two (and tie).
#[test]
fn a_path_listed_twice_is_one_candidate() {
    let c = |path: &str, key: (u64, i32)| Candidate { path: path.to_string(), key };
    let out = dedupe(vec![c("GD_Toy.Story0", (3, 0)), c("gd_toy.story0", (3, 0)), c("GD_Other.Story0", (3, 0))]);
    let paths: Vec<&str> = out.iter().map(|c| c.path.as_str()).collect();
    assert_eq!(paths, ["GD_Toy.Story0", "GD_Other.Story0"]);
}
