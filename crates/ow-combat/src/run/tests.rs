//! Tests of the scripted run and, through it, of the combat natives' rules, on the invented packages of
//! [`super::fixture`] (AGENTS.md rule 9). The expected values are the rules worked by hand (see `tests/combat_test.py`'s
//! docstring for the same data); the C++ comparison of whole reports is in `ow-cli`'s `cmd/combat` tests.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_balance::types::{
    AwesomeLevel, BalanceSnapshot, ExpLevel, GameStage, LevelCapUpgrades, OpLevel, PlayerCount,
    Playthrough,
};
use ow_core::ParityMode;
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_vm::api::{NativePack, StubPolicy, Vm};
use ow_world::world::{BalanceState, World};

use super::fixture;
use super::*;

static NEXT: AtomicU32 = AtomicU32::new(0);

/// A temporary folder holding the fixture, removed on drop.
pub(crate) struct Dir(pub PathBuf);

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The world a run is on (as `ow-cli`'s `--combat-run` builds it) over the fixture, in `mode`.
pub(crate) fn world(mode: ParityMode) -> (World, Dir) {
    let dir = std::env::temp_dir().join(format!(
        "ow-combat-run-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let files = fixture::files();
    let refs: Vec<(&str, &[u8])> = files.iter().map(|(n, b)| (*n, b.as_slice())).collect();
    let defs =
        DefStore::from_synthetic(&dir, &refs, IdentityPolicy::FirstInScanOrder, mode).unwrap();
    let policy = match mode {
        ParityMode::Off => StubPolicy::refuse_all(),
        ParityMode::Reference => StubPolicy::LogAndZero,
    };
    let attrs = AttributeSystem::standard().unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(attrs), mode).with_stub_policy(policy);
    ow_vm::natives_core::register(&mut vm).unwrap();
    let snapshot = BalanceSnapshot::new(
        Playthrough::NORMAL,
        OpLevel::NONE,
        PlayerCount::SINGLE,
        ExpLevel::new(1),
        GameStage(1),
        AwesomeLevel::new(0),
        LevelCapUpgrades::ALL,
    )
    .unwrap();
    let pack = crate::hit::Pack;
    let packs: [&dyn NativePack; 1] = [&pack];
    let w = World::new(vm, BalanceState::new(snapshot), &packs).unwrap();
    (w, Dir(dir))
}

/// A run over the fixture in `mode`, `steps` applied; the report as JSON text and whether a step failed.
pub(crate) fn run_steps(mode: ParityMode, steps: &[&str]) -> (String, bool) {
    let (w, _dir) = world(mode);
    let steps: Vec<String> = steps.iter().map(|s| s.to_string()).collect();
    combat_run(w, &steps).unwrap()
}

/// Writes the fixture's packages to the folder `OW_COMBAT_FIXTURE` names (default `local/p2/w6-combat/rust_fixture`
/// under the workspace), for the C++ binary to run on: the C++ side of the `ow-cli` comparison.
#[test]
#[ignore]
fn write_fixture() {
    let dir = std::env::var("OW_COMBAT_FIXTURE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../local/p2/w6-combat/rust_fixture")
        });
    std::fs::create_dir_all(&dir).unwrap();
    for (name, bytes) in fixture::files() {
        std::fs::write(dir.join(name), bytes).unwrap();
    }
}

#[test]
fn a_spawned_pawn_gets_its_health_pool_from_its_class() {
    let (text, failed) = run_steps(
        ParityMode::Off,
        &["spawn:ToyPawns/Pawn_Toy:4", "sethealth:0:0.5"],
    );
    assert!(!failed, "{text}");
    assert!(text.contains("\"health.Toy.L4\":40.5"), "{text}");
}

/// A run over the fixture in `mode` (nothing applied yet).
pub(crate) fn fresh(mode: ParityMode) -> (Run, Dir) {
    let (w, dir) = world(mode);
    (Run::new(w).unwrap(), dir)
}

/// The definition at `path` in the fixture.
pub(crate) fn def(vm: &mut Vm, path: &str) -> ow_object::ids::ObjRef {
    vm.defs
        .find(&mut vm.space, &ow_core::ObjectPath::parse(path).unwrap())
        .unwrap()
        .unwrap_or_else(|| panic!("no {path}"))
}

/// A new object of class `path` from its class defaults.
pub(crate) fn make(vm: &mut Vm, path: &str) -> ow_object::ids::ObjRef {
    let class = vm.find_class(path).unwrap();
    ow_vm::interp::instantiate_from_defaults(vm, class).unwrap()
}

/// The error texts of a report.
fn errors_of(text: &str) -> &str {
    &text[text.find("\"errors\":").unwrap()..]
}

#[test]
fn the_step_numbers_read_as_the_msvc_library_reads_them() {
    assert_eq!(stoi("4abc").unwrap(), 4);
    assert_eq!(stoi(" -7").unwrap(), -7);
    assert_eq!(error_text(&stoi("x").unwrap_err()), "invalid stoi argument");
    assert_eq!(
        error_text(&stoi("99999999999").unwrap_err()),
        "stoi argument out of range"
    );
    assert_eq!(stoul("3").unwrap(), 3);
    assert_eq!(stoul("-1").unwrap(), usize::try_from(u32::MAX).unwrap());
    assert_eq!(
        error_text(&stoul("").unwrap_err()),
        "invalid stoul argument"
    );
    assert_eq!(stod("29.7").unwrap(), 29.7);
    assert_eq!(stod("-inf").unwrap(), f64::NEG_INFINITY);
    assert_eq!(
        error_text(&stod("abc").unwrap_err()),
        "invalid stod argument"
    );
    assert_eq!(
        error_text(&stod("1e999").unwrap_err()),
        "stod argument out of range"
    );
    assert_eq!(stoll("12x").unwrap(), 12);
}

#[test]
fn the_steps_refuse_with_the_cxx_texts() {
    let (text, failed) = run_steps(
        ParityMode::Off,
        &[
            "spawn:ToyPawns/Nope:4",
            "spawn:nokind:1",
            "spawn:ToyPawns/Pawn_Toy",
            "hit:9:1:x",
            "player:x",
            "sequence:0:a:maybe",
            "tick:1",
            "cash:1",
            "bleed:1",
            "mayahealth:1",
            "nosuchstep",
            "attr:0:Foo:1",
        ],
    );
    assert!(failed);
    let errors = errors_of(&text);
    for e in [
        "spawn:ToyPawns/Nope:4: combat: no such pawn archetype: Nope",
        "spawn:nokind:1: combat-run: unknown pawn kind nokind",
        "spawn:ToyPawns/Pawn_Toy: combat-run: spawn:<kind>:<level>[:<stage>[:<balance path>]]",
        "hit:9:1:x: combat-run: a hit needs a player (the step player:<level> or maya:<level>) first",
        "player:x: invalid stoi argument",
        "sequence:0:a:maybe: combat-run: no pawn 0",
        "tick:1: combat-run: tick needs the mission bridge",
        "cash:1: combat-run: cash needs the mission bridge (the step maya:<level>)",
        "bleed:1: combat-run: bleed needs the mission bridge (the step maya:<level>)",
        "nosuchstep: combat-run: unknown step nosuchstep",
        "attr:0:Foo:1: combat-run: no such player 0",
    ] {
        assert!(errors.contains(e), "{e}: {errors}");
    }
}

#[test]
fn the_loot_steps_and_a_maya_run_wait_for_their_lanes() {
    let (text, _) = run_steps(
        ParityMode::Off,
        &[
            "spawn:ToyPawns/Pawn_Toy:4",
            "player:8",
            "seed:3",
            "rollpool:0:Pool_Coins",
            "lootbatch:dummy:1:1:1",
        ],
    );
    assert_eq!(
        errors_of(&text).matches("needs W6-LOOT").count(),
        3,
        "{text}"
    );
    let steps: Vec<String> = vec!["spawn:ToyPawns/Pawn_Toy:4".into(), "maya:8".into()];
    let (w, _dir) = world(ParityMode::Off);
    match combat_run(w, &steps) {
        Err(VmError::NotYet(n)) => assert!(n.to_string().contains("W6-MISSION"), "{n}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_spawn_with_a_pawn_balance_spawns_the_pawn_and_waits_for_the_loot_setup() {
    let (mut run, _dir) = fresh(ParityMode::Off);
    run.apply("spawn:ToyPawns/Pawn_Toy:4:4:SomeBalance");
    assert_eq!(run.pawns.len(), 1);
    assert!(run.failed());
    assert!(
        run.errors[0]
            .contains("the loot setup of the spawned pawn (SomeBalance; pawn 0 was spawned)")
    );
    assert_eq!(run.rows.get("health.Toy.L4"), Some(&40.5));
}

#[test]
fn a_repeated_row_takes_a_suffix_and_repeated_kills_a_letter() {
    let (text, failed) = run_steps(
        ParityMode::Off,
        &[
            "spawn:ToyPawns/Pawn_Doomed:5",
            "spawn:ToyPawns/Pawn_Doomed:5",
            "player:4",
            "players:1",
            "hit:0:10::Bullet",
            "hit:1:10::Bullet",
        ],
    );
    assert!(!failed, "{text}");
    assert!(text.contains("\"health.Doomed.L5#2\":40.5"), "{text}");
    assert!(
        text.contains("\"kill.Doomed.L5.p4.a\":{\"award\":30}"),
        "{text}"
    );
    assert!(
        text.contains("\"kill.Doomed.L5.p4.b\":{\"award\":30}"),
        "{text}"
    );
    assert!(text.contains("\"hit.Doomed.L5.2.arriving\":10"), "{text}");
}

#[test]
fn the_kill_tag_names_the_kill_row() {
    let (text, _) = run_steps(
        ParityMode::Off,
        &[
            "spawn:ToyPawns/Pawn_Doomed:5",
            "player:4",
            "killtag:first",
            "hit:0:10::Bullet",
        ],
    );
    assert!(
        text.contains("\"kill.Doomed.L5.p4.first\":{\"award\":30}"),
        "{text}"
    );
}

#[test]
fn a_step_naming_a_pawn_reports_its_state_and_the_others_do_not() {
    let (text, _) = run_steps(
        ParityMode::Off,
        &["spawn:ToyPawns/Pawn_Toy:4", "base:0", "sethealth:0:5"],
    );
    let base = &text[text.find("\"step\":\"base:0\"").unwrap()
        ..text.find("\"step\":\"sethealth:0:5\"").unwrap()];
    assert!(!base.contains("\"state\""), "{base}");
    let set = &text[text.find("\"step\":\"sethealth:0:5\"").unwrap()..];
    assert!(
        set.contains("\"state\":{\"demigod\":false,\"health\":5,"),
        "{set}"
    );
}

#[test]
fn a_class_with_no_behavior_provider_is_said_and_sequence_refuses() {
    let (text, failed) = run_steps(
        ParityMode::Off,
        &[
            "spawn:ToyPawns/Pawn_Toy:4",
            "player:8",
            "hit:0:10::Bullet",
            "sequence:0:Any:on",
        ],
    );
    assert!(failed);
    assert!(
        text.contains("the pawn's AI class has no behavior provider"),
        "{text}"
    );
    assert!(
        errors_of(&text).contains(
            "sequence:0:Any:on: combat-run: the pawn's AI class has no behavior provider"
        )
    );
}

// ---------------------------------------------------------------------------------------------------------------
// A behavior event raised on a provider that is running (review r1 of PR #335)
// ---------------------------------------------------------------------------------------------------------------

// The invented provider packages of the `ow-behavior` tests (the same file; it uses only `ow_package::testkit`).
#[allow(dead_code)]
#[path = "../../../ow-behavior/src/kernel/fixtures/packages.rs"]
mod packages;

/// A VM over the `ow-behavior` fixture's install, a provider `ToyDeath` and an invented package `Reentry`: a pawn class
/// with `AIClass` and `MyDeathDef`, and a holder class with `BehaviorProviderDefinition` (the AI class's and the
/// death definition's field). The death sequence: `OnPlayDeathPizazz` -> A, `OnDeathNonGib` -> B, `Custom` -> C;
/// A and B are `Behavior_ToySame`, C `Behavior_ToyDifferent`. The test registers their handlers: on `OnDeathNonGib`,
/// B broadcasts `Custom` through the combat state's raise, as `BroadcastBehaviorEventFromScript` does.
fn reentry_vm() -> (Vm, ow_object::ids::ObjRef, ow_object::ids::ObjRef, Dir) {
    use ow_behavior::registry::{Handler, register};
    use packages::{Seq, Spec, beh, ev, install, provider_package};
    let death = Seq {
        name: "Death",
        enabled_on_spawn: Some(true),
        events: vec![
            ev("OnPlayDeathPizazz", (0, 1)),
            ev("OnDeathNonGib", (1, 1)),
            ev("Custom", (2, 1)),
        ],
        behaviors: vec![
            beh("A", "Behavior_ToySame", (0, 0)),
            beh("B", "Behavior_ToySame", (0, 0)),
            beh("C", "Behavior_ToyDifferent", (0, 0)),
        ],
        links: vec![(0, -1, 0.0), (1, -1, 0.0), (2, -1, 0.0)],
        ..Seq::default()
    };
    let spec = Spec::new("ToyDeath", vec![death], vec![]);
    let mut d = ow_package::testkit::reflect::Declarations::new(&["Object"]);
    let core = d.p.add_import("Core", "Package", 0, "Core");
    let object = d.p.add_import("Core", "Class", core, "Object");
    let pawn_class = d.cls("ToyDeathPawn", object);
    d.prop("Object", pawn_class, "AIClass", object);
    d.prop("Object", pawn_class, "MyDeathDef", object);
    let holder = d.cls("ToyHolder", object);
    d.prop("Object", holder, "BehaviorProviderDefinition", object);
    let mut files = install(&[("ToyDeath", provider_package(&spec))]);
    files.push(("Reentry.upk".into(), d.build()));
    let dir = std::env::temp_dir().join(format!(
        "ow-combat-reentry-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let refs: Vec<(&str, &[u8])> = files
        .iter()
        .map(|(n, b)| (n.as_str(), b.as_slice()))
        .collect();
    let defs = DefStore::from_synthetic(
        &dir,
        &refs,
        IdentityPolicy::FirstInScanOrder,
        ParityMode::Off,
    )
    .unwrap();
    let mut vm = Vm::new(
        ObjectSpace::new(),
        defs,
        Rc::new(AttributeSystem::new_empty()),
        ParityMode::Off,
    );
    ow_vm::natives_core::register(&mut vm).unwrap();
    crate::hit::Pack.register(&mut vm).unwrap();
    vm.add_service(Providers::default()).unwrap();
    crate::hit::with_state(&vm, |s| {
        s.on_behavior_event = Some(Rc::new(raise_behavior_event))
    })
    .unwrap();
    let bpd = vm
        .defs
        .find(
            &mut vm.space,
            &ow_core::ObjectPath::parse("ToyDeath").unwrap(),
        )
        .unwrap()
        .unwrap();
    let make = |vm: &mut Vm, path: &str| make(vm, path);
    let pawn = make(&mut vm, "Reentry.ToyDeathPawn");
    let class_holder = make(&mut vm, "Reentry.ToyHolder");
    let death_holder = make(&mut vm, "Reentry.ToyHolder");
    for h in [class_holder, death_holder] {
        crate::hit::access::set(
            &mut vm,
            h,
            "BehaviorProviderDefinition",
            Value::Object(Some(bpd)),
        )
        .unwrap();
    }
    crate::hit::access::set(&mut vm, pawn, "AIClass", Value::Object(Some(class_holder))).unwrap();
    crate::hit::access::set(
        &mut vm,
        pawn,
        "MyDeathDef",
        Value::Object(Some(death_holder)),
    )
    .unwrap();
    let broadcast = Handler::run(move |vm: &mut Vm, _p, _id, event: &str| {
        if event == "OnDeathNonGib" {
            let hook = crate::hit::with_state(vm, |s| s.on_behavior_event.clone())?.unwrap();
            let raised = BehaviorEvent {
                pawn,
                definition: None,
                killer: None,
                name: "Custom".into(),
                broadcast: true,
            };
            hook(vm, &raised)?;
        }
        Ok(Vec::new())
    });
    register(&mut vm, "WillowGame.Behavior_ToySame", broadcast).unwrap();
    register(
        &mut vm,
        "WillowGame.Behavior_ToyDifferent",
        Handler::run(|_, _, _, _| Ok(Vec::new())),
    )
    .unwrap();
    (vm, pawn, death_holder, Dir(dir))
}

#[test]
fn an_event_raised_on_a_running_provider_runs_after_it_once_and_on_the_same_provider() {
    let (mut vm, pawn, death, _dir) = reentry_vm();
    for name in ["OnPlayDeathPizazz", "OnDeathNonGib"] {
        let event = BehaviorEvent {
            pawn,
            definition: Some(death),
            killer: None,
            name: name.into(),
            broadcast: false,
        };
        raise_behavior_event(&mut vm, &event).unwrap();
    }
    let lines: Vec<String> = crate::hit::with_state(&vm, |s| s.events.clone())
        .unwrap()
        .into_iter()
        .filter(|e| e.starts_with("behavior ") || e.starts_with("provider error"))
        .collect();
    // B's broadcast reaches the pawn's class provider at once (it is not running: made then) and the running death
    // provider after B's event returns.
    assert_eq!(
        lines,
        [
            "behavior OnPlayDeathPizazz -> A",
            "behavior Custom -> C",
            "behavior OnDeathNonGib -> B",
            "behavior Custom -> C",
        ],
        "{lines:?}"
    );
    let service = providers(&vm).unwrap();
    let s = service.borrow();
    assert_eq!(
        s.held.len(),
        2,
        "one class and one death provider for the pawn, no copy"
    );
    assert!(
        s.held.values().all(|h| h.provider.is_some()),
        "every provider is back"
    );
    assert!(s.deferred.is_empty());
    assert_eq!(s.depth, 0);
}

#[test]
fn a_take_damage_raise_on_a_running_provider_is_drained_by_its_own_outermost_raise() {
    let (mut vm, pawn, _death, _dir) = reentry_vm();
    // The class provider is running (taken out by hand): the raise is deferred, then drained by the same outermost
    // raise, which finds it still running and gives up only at the budget, as a runaway.
    {
        let service = providers(&vm).unwrap();
        service.borrow_mut().held.insert(
            (pawn, Which::Class),
            Held {
                provider: None,
                reported: 0,
            },
        );
    }
    let event = TakeDamageEvent {
        pawn,
        instigator: None,
        damage_type: None,
        damage: 1.0,
        shield_damage: 0.0,
        source: None,
    };
    let e = raise_take_damage(&mut vm, &event).unwrap_err();
    assert!(
        error_text(&e).contains("raised on running providers (a runaway)"),
        "{e}"
    );
    let service = providers(&vm).unwrap();
    assert!(
        service.borrow().deferred.is_empty(),
        "a failed raise leaves nothing deferred"
    );
    assert_eq!(service.borrow().depth, 0);
}
