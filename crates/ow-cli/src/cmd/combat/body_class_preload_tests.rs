//! The body-class preload (lane IMPL-BODYCLASS-PRELOAD) on an invented package (AGENTS.md rule 9). The body class lists
//! a default hit region that its `HitRegionList` does not hold, two regions (one with an `OverrideDeathDef`), two
//! stances, a death definition, a player event definition and an unrelated reference; each holder names its own
//! provider definition. The player event definition's sequences name two pass-2 conditions (one twice, one sequence
//! with none); the first links an objective and a mission, and references an unrelated object (lane
//! IMPL-PASS2-CONDITION-PRELOAD). AI-assisted (Claude Code).

use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::{ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::reflect::toy_array_of;
use ow_package::testkit::script::ScriptToy;
use ow_vm::api::Vm;

use super::{body_class_preload_line, preload_body_class_definitions};

static NEXT: AtomicU32 = AtomicU32::new(0);

fn package() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "ObjectProperty", "ArrayProperty", "ScriptStruct", "StructProperty"]);
    let object = t.class("Object", 0);
    let pawn = t.class("ToyPawn", object);
    t.prop("Object", pawn, "BodyClass", 0, object);
    let body = t.class("ToyBodyClass", object);
    t.prop("Object", body, "DefaultHitRegion", 0, object);
    toy_array_of(&mut t, body, "HitRegionList", "Object", object);
    toy_array_of(&mut t, body, "ScrapedStances", "Object", object);
    for name in ["DeathDef", "PlayerEventDef", "Unrelated"] {
        t.prop("Object", body, name, 0, object);
    }
    let holder = t.class("ToyHolder", object);
    t.prop("Object", holder, "BehaviorProviderDefinition", 0, object);
    t.prop("Object", holder, "OverrideDeathDef", 0, object);
    let provider = t.class("ToyProvider", object);
    let sequence = ow_package::testkit::reflect::toy_struct(&mut t, provider, "ToySequence", None);
    t.prop("Object", sequence, "CustomEnableCondition", 0, object);
    toy_array_of(&mut t, provider, "BehaviorSequences", "Struct", sequence);
    let condition = t.class("ToyCondition", object);
    for name in ["LinkedObjective", "LinkedMission", "ToyOther"] {
        t.prop("Object", condition, name, 0, object);
    }
    let list = |t: &mut ScriptToy, name: &str, items: &[i32]| -> Vec<u8> {
        let mut value = i32::try_from(items.len()).unwrap().to_le_bytes().to_vec();
        for i in items {
            value.extend_from_slice(&i.to_le_bytes());
        }
        t.p.tag(name, "ArrayProperty", &value)
    };
    let export = |t: &mut ScriptToy, class: i32, name: &str, tags: &dyn Fn(&mut ScriptToy) -> Vec<u8>| {
        let mut payload = 0i32.to_le_bytes().to_vec();
        payload.extend(tags(t));
        payload.extend_from_slice(&t.none);
        t.p.add_export(class, name, payload)
    };
    let def = |t: &mut ScriptToy, name: &str| export(t, provider, name, &|_| Vec::new());
    let hold = |t: &mut ScriptToy, name: &str, def: i32, death: i32| {
        export(t, holder, name, &move |t| {
            let mut tags = t.p.object_tag("BehaviorProviderDefinition", def);
            if death != 0 {
                tags.extend(t.p.object_tag("OverrideDeathDef", death));
            }
            tags
        })
    };
    // Pass 2's objects: an objective, a mission and an unrelated object (plain objects), and two conditions.
    let plain = |t: &mut ScriptToy, name: &str| export(t, object, name, &|_| Vec::new());
    let (objective, mission, other) = (plain(&mut t, "Objective_1"), plain(&mut t, "Mission_1"), plain(&mut t, "Other_1"));
    let c1 = export(&mut t, condition, "Cond_1", &move |t| {
        let mut tags = t.p.object_tag("LinkedObjective", objective);
        tags.extend(t.p.object_tag("LinkedMission", mission));
        tags.extend(t.p.object_tag("ToyOther", other));
        tags
    });
    let c2 = export(&mut t, condition, "Cond_2", &|_| Vec::new());
    let names = ["P_R1", "P_R2", "P_R3", "P_D2", "P_S1", "P_S2", "P_D1"];
    let mut p: Vec<i32> = names.iter().map(|n| def(&mut t, n)).collect();
    // P_E1's sequences: Cond_1, none, Cond_2, Cond_1 again.
    let events = export(&mut t, provider, "P_E1", &move |t| {
        let mut value = 4i32.to_le_bytes().to_vec();
        for c in [c1, 0, c2, c1] {
            if c != 0 {
                value.extend(t.p.object_tag("CustomEnableCondition", c));
            }
            value.extend_from_slice(&t.none);
        }
        t.p.tag("BehaviorSequences", "ArrayProperty", &value)
    });
    p.push(events);
    let d2 = hold(&mut t, "Death_Override", p[3], 0);
    let r1 = hold(&mut t, "Region_1", p[0], d2);
    let r2 = hold(&mut t, "Region_2", p[1], 0);
    let r3 = hold(&mut t, "Region_Default", p[2], 0);
    let s1 = hold(&mut t, "Stance_Run", p[4], 0);
    let s2 = hold(&mut t, "Stance_Crouch", p[5], 0);
    let d1 = hold(&mut t, "Death_Body", p[6], 0);
    let e1 = hold(&mut t, "Events_Body", p[7], 0);
    let unrelated = export(&mut t, object, "Unrelated_Thing", &|_| Vec::new());
    let body_class = export(&mut t, body, "BodyClass_Toy", &move |t| {
        let mut tags = t.p.object_tag("DefaultHitRegion", r3);
        tags.extend(list(t, "HitRegionList", &[r1, 0, r2]));
        tags.extend(list(t, "ScrapedStances", &[s1, s2]));
        tags.extend(t.p.object_tag("DeathDef", d1));
        tags.extend(t.p.object_tag("PlayerEventDef", e1));
        tags.extend(t.p.object_tag("Unrelated", unrelated));
        tags
    });
    export(&mut t, pawn, "ToyArchetype", &move |t| t.p.object_tag("BodyClass", body_class));
    t.build()
}

struct Fx {
    vm: Vm,
    dir: std::path::PathBuf,
    archetype: ObjRef,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fx() -> Fx {
    let dir = std::env::temp_dir().join(format!("ow-cli-bodyclass-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let bytes = package();
    let defs = DefStore::from_synthetic(&dir, &[("GD_ToyBody.upk", &bytes)], IdentityPolicy::FirstInScanOrder, ParityMode::Off)
        .unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let path = ObjectPath::parse("GD_ToyBody.ToyArchetype").unwrap();
    let archetype = vm.defs.find(&mut vm.space, &path).unwrap().expect("found");
    vm.log.clear();
    Fx { vm, dir, archetype }
}

fn slot(vm: &Vm, o: ObjRef, name: &str) -> Value {
    let class = vm.space.heap.get(o).unwrap().class;
    let p = vm.space.prop(class, name).unwrap();
    vm.space.read(o, PropAddr::scalar(p)).unwrap().clone()
}

fn object(v: &Value) -> ObjRef {
    match v {
        Value::Object(Some(o)) => *o,
        other => panic!("{other:?}"),
    }
}

fn harness(vm: &Vm) -> Vec<String> {
    vm.log.iter().filter(|l| l.starts_with("harness input:")).cloned().collect()
}

fn lines(names: &[&str]) -> Vec<String> {
    names.iter().map(|n| body_class_preload_line(&format!("GD_ToyBody.{n}"))).collect()
}

/// The exact load list, in the preflight's order, each logged; the default region the list does not hold, its
/// definition and the unrelated reference stay unloaded. A second run loads nothing.
#[test]
fn exactly_what_the_preflight_reads_is_loaded_once() {
    let mut x = fx();
    let before = x.vm.defs.load_transitions();
    let n = preload_body_class_definitions(&mut x.vm, x.archetype).unwrap();
    let expected = [
        "BodyClass_Toy",
        "Region_1",
        "P_R1",
        "Death_Override",
        "P_D2",
        "Region_2",
        "P_R2",
        "Stance_Run",
        "P_S1",
        "Stance_Crouch",
        "P_S2",
        "Death_Body",
        "P_D1",
        "Events_Body",
        "P_E1",
    ];
    // Pass 2's objects, after P_E1 (lane IMPL-PASS2-CONDITION-PRELOAD): each condition once, in sequence order, and
    // the first one's LinkedObjective. Not its LinkedMission, not its other reference.
    let pass2 = ["Cond_1", "Objective_1", "Cond_2"];
    assert_eq!(n, expected.len() + pass2.len());
    assert_eq!(x.vm.defs.load_transitions() - before, n as u64, "those objects, nothing else");
    let mut want = lines(&expected);
    want.extend(lines(&pass2));
    want.push(format!("harness input: set-up preload: {} body-class definition objects loaded", expected.len()));
    want.push(format!("harness input: set-up preload: {} pass-2 condition objects loaded", pass2.len()));
    assert_eq!(harness(&x.vm), want);
    let c1 = x.vm.defs.find(&mut x.vm.space, &ObjectPath::parse("GD_ToyBody.Cond_1").unwrap()).unwrap().unwrap();
    for name in ["LinkedMission", "ToyOther"] {
        let o = object(&slot(&x.vm, c1, name));
        assert!(x.vm.defs.is_unloaded(&x.vm.space, o), "{name} stays unloaded");
    }
    let body = object(&slot(&x.vm, x.archetype, "BodyClass"));
    let default = object(&slot(&x.vm, body, "DefaultHitRegion"));
    let unrelated = object(&slot(&x.vm, body, "Unrelated"));
    assert!(x.vm.defs.is_unloaded(&x.vm.space, default), "the default region outside the list");
    assert!(x.vm.defs.is_unloaded(&x.vm.space, unrelated), "an unrelated reference");
    let loads = x.vm.defs.load_transitions();
    x.vm.log.clear();
    assert_eq!(preload_body_class_definitions(&mut x.vm, x.archetype).unwrap(), 0);
    assert_eq!(x.vm.defs.load_transitions(), loads, "a second run loads nothing");
    assert!(harness(&x.vm).is_empty(), "and logs nothing: {:?}", x.vm.log);
}

/// A default hit region the list holds is loaded first (step (a)), and once.
#[test]
fn a_listed_default_region_comes_first() {
    let mut x = fx();
    let body = object(&slot(&x.vm, x.archetype, "BodyClass"));
    x.vm.defs.ensure_loaded(&mut x.vm.space, body).unwrap();
    let region_2 = x.vm.space.prop(x.vm.space.heap.get(body).unwrap().class, "HitRegionList").unwrap();
    let list = x.vm.space.read(body, PropAddr::scalar(region_2)).unwrap().clone();
    let second = list.elements().unwrap()[2].clone();
    let p = x.vm.space.prop(x.vm.space.heap.get(body).unwrap().class, "DefaultHitRegion").unwrap();
    *x.vm.space.slot_mut(body, PropAddr::scalar(p)).unwrap() = second;
    x.vm.log.clear();
    preload_body_class_definitions(&mut x.vm, x.archetype).unwrap();
    let log = harness(&x.vm);
    assert_eq!(&log[..2], &lines(&["Region_2", "P_R2"])[..], "{log:?}");
    assert_eq!(log.iter().filter(|l| l.contains("Region_2")).count(), 1, "once");
}

/// A stance the store keeps as a stand-in (its slots not built) is skipped and logged; its definition is not loaded,
/// and the rest goes on.
#[test]
fn a_stand_in_is_skipped() {
    let mut x = fx();
    let body = object(&slot(&x.vm, x.archetype, "BodyClass"));
    x.vm.defs.ensure_loaded(&mut x.vm.space, body).unwrap();
    let class = x.vm.find_class("GD_ToyBody.ToyHolder").unwrap();
    let stand_in = x.vm.space.alloc_stand_in(class, "Stance_StandIn", Origin::Default { class }).unwrap();
    let p = x.vm.space.prop(x.vm.space.heap.get(body).unwrap().class, "ScrapedStances").unwrap();
    let mut stances = x.vm.space.read(body, PropAddr::scalar(p)).unwrap().clone();
    stances.array_mut().unwrap()[1] = Value::Object(Some(stand_in));
    *x.vm.space.slot_mut(body, PropAddr::scalar(p)).unwrap() = stances;
    x.vm.log.clear();
    preload_body_class_definitions(&mut x.vm, x.archetype).unwrap();
    let log = harness(&x.vm);
    let skip = log.iter().position(|l| l.contains("the stance") && l.ends_with("is not resident; skipped"));
    assert!(skip.is_some(), "{log:?}");
    assert!(!log.iter().any(|l| l.contains("P_S2")), "the stand-in's definition is not loaded: {log:?}");
    assert!(log.iter().any(|l| l.contains("Events_Body")), "the rest goes on: {log:?}");
}
