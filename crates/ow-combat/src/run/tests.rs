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
fn the_loot_steps_answer_in_off_mode_and_a_maya_step_needs_the_bridge() {
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
    // Lane W6-LOOT: seed is a step; this pawn has no item game stage; a loot batch is NotYet in Off mode.
    let errors = errors_of(&text);
    assert!(!errors.contains("seed:3"), "{text}");
    assert!(errors.contains("rollpool:0:Pool_Coins: combat-run: WillowAIPawn has no property GameStageForSpawnedInventory"), "{text}");
    assert!(errors.contains("lootbatch:dummy:1:1:1: lootbatch in Off mode"), "{text}");
    // A maya step run without the bridge (the caller builds it, lane W6-COMBAT-RUN2): the C++ text.
    let steps: Vec<String> = vec!["spawn:ToyPawns/Pawn_Toy:4".into(), "maya:8".into()];
    let (w, _dir) = world(ParityMode::Off);
    let (text, failed) = combat_run(w, &steps).unwrap();
    assert!(failed);
    assert!(errors_of(&text).contains("maya:8: combat-run: maya needs the mission bridge"), "{text}");
}

#[test]
fn a_spawn_with_a_pawn_balance_the_package_lacks_fails_before_the_pawn_is_listed() {
    // As the C++ (lane W6-LOOT): the loot setup comes before the rows and the pawn list.
    let (mut run, _dir) = fresh(ParityMode::Off);
    run.apply("spawn:ToyPawns/Pawn_Toy:4:4:SomeBalance");
    assert!(run.pawns.is_empty());
    assert!(run.failed());
    assert!(run.errors[0].contains("combat-run: ToyPawns has no SomeBalance"), "{:?}", run.errors);
    assert_eq!(run.rows.get("health.Toy.L4"), None);
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
// Behavior events raised on providers that are running (reviews r1 and r2 of PR #335)
// ---------------------------------------------------------------------------------------------------------------

// The invented provider packages of the `ow-behavior` tests (the same file; it uses only `ow_package::testkit`).
#[allow(dead_code)]
#[path = "../../../ow-behavior/src/kernel/fixtures/packages.rs"]
mod packages;

/// The pawn, its death definition holder, and the folder of a [`reentry_vm`].
struct Reentry {
    vm: Vm,
    pawn: ow_object::ids::ObjRef,
    death: ow_object::ids::ObjRef,
    _dir: Dir,
}

/// A VM over the `ow-behavior` fixture's install, two providers and an invented package `Reentry`: a pawn class with
/// `AIClass` and `MyDeathDef`, and a holder class with `BehaviorProviderDefinition` (the AI class's and the death
/// definition's field).
///
/// - `ToyDeath` (the death definition's): `OnPlayDeathPizazz` -> A, `OnDeathNonGib` -> B, `Custom` -> C, and `E1`
///   to `E4` -> P1 to P4.
/// - `ToyClass` (the AI class's), sequence `Main`: `Custom` -> K, `OnBehaviorSequenceDisabled` -> S, `Ping` -> Q.
///
/// Every behavior is `Behavior_ToySame`, whose test handler raises by the event it runs for: on `OnDeathNonGib` a
/// broadcast of `Custom` (as `BroadcastBehaviorEventFromScript` does); on `E1` the death events `E2` then `E4`; on
/// `E2` the death event `E3`; on `OnBehaviorSequenceDisabled` a broadcast of `Ping`.
fn reentry_vm() -> Reentry {
    reentry_vm_in(ParityMode::Off)
}

/// [`reentry_vm`] in `mode`.
fn reentry_vm_in(mode: ParityMode) -> Reentry {
    use ow_behavior::registry::{Handler, register};
    use packages::{Seq, Spec, beh, ev, install, provider_package};
    let events = |names: &[&'static str]| -> Vec<packages::Ev> {
        (0..names.len())
            .map(|i| ev(names[i], (i as u32, 1)))
            .collect()
    };
    let behaviors = |names: &[&'static str]| -> Vec<packages::Beh> {
        names
            .iter()
            .map(|n| beh(n, "Behavior_ToySame", (0, 0)))
            .collect()
    };
    let links = |n: u32| -> Vec<(u32, i32, f32)> { (0..n).map(|i| (i, -1, 0.0)).collect() };
    let death_events = [
        "OnPlayDeathPizazz",
        "OnDeathNonGib",
        "Custom",
        "E1",
        "E2",
        "E3",
        "E4",
        "Chain",
    ];
    // Chain (event 7) runs B1 (behavior 7), whose output link 0 runs B2 (behavior 8): a death sequence B1 -> B2.
    let mut death_behaviors = behaviors(&["A", "B", "C", "P1", "P2", "P3", "P4", "B1", "B2"]);
    death_behaviors[7].links = (8, 1);
    let mut death_links = links(8);
    death_links.push((8, 0, 0.0));
    let death = Seq {
        name: "Death",
        enabled_on_spawn: Some(true),
        events: events(&death_events),
        behaviors: death_behaviors,
        links: death_links,
        ..Seq::default()
    };
    let class = Seq {
        name: "Main",
        enabled_on_spawn: Some(true),
        events: events(&["Custom", "OnBehaviorSequenceDisabled", "Ping"]),
        behaviors: behaviors(&["K", "S", "Q"]),
        links: links(3),
        ..Seq::default()
    };
    let mut d = ow_package::testkit::reflect::Declarations::new(&["Object"]);
    let core = d.p.add_import("Core", "Package", 0, "Core");
    let object = d.p.add_import("Core", "Class", core, "Object");
    let pawn_class = d.cls("ToyDeathPawn", object);
    d.prop("Object", pawn_class, "AIClass", object);
    d.prop("Object", pawn_class, "MyDeathDef", object);
    let holder = d.cls("ToyHolder", object);
    d.prop("Object", holder, "BehaviorProviderDefinition", object);
    let mut files = install(&[
        (
            "ToyDeath",
            provider_package(&Spec::new("ToyDeath", vec![death], vec![])),
        ),
        (
            "ToyClass",
            provider_package(&Spec::new("ToyClass", vec![class], vec![])),
        ),
    ]);
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
        mode,
    )
    .unwrap();
    let mut vm = Vm::new(
        ObjectSpace::new(),
        defs,
        Rc::new(AttributeSystem::new_empty()),
        mode,
    );
    ow_vm::natives_core::register(&mut vm).unwrap();
    crate::hit::Pack.register(&mut vm).unwrap();
    vm.add_service(Providers::default()).unwrap();
    crate::hit::with_state(&vm, |s| {
        s.on_behavior_event = Some(Rc::new(raise_behavior_event))
    })
    .unwrap();
    let pawn = make(&mut vm, "Reentry.ToyDeathPawn");
    let mut holders = Vec::new();
    for provider in ["ToyClass", "ToyDeath"] {
        let bpd = def(&mut vm, provider);
        let h = make(&mut vm, "Reentry.ToyHolder");
        crate::hit::access::set(
            &mut vm,
            h,
            "BehaviorProviderDefinition",
            Value::Object(Some(bpd)),
        )
        .unwrap();
        holders.push(h);
    }
    let (class_holder, death_holder) = (holders[0], holders[1]);
    crate::hit::access::set(&mut vm, pawn, "AIClass", Value::Object(Some(class_holder))).unwrap();
    crate::hit::access::set(
        &mut vm,
        pawn,
        "MyDeathDef",
        Value::Object(Some(death_holder)),
    )
    .unwrap();
    let handler = Handler::run(move |vm: &mut Vm, _p, id: BehaviorId, event: &str| {
        let raise = |vm: &mut Vm, name: &str, broadcast: bool| -> Result<(), VmError> {
            let hook = crate::hit::with_state(vm, |s| s.on_behavior_event.clone())?.unwrap();
            let raised = BehaviorEvent {
                pawn,
                definition: if broadcast { None } else { Some(death_holder) },
                killer: None,
                name: name.into(),
                broadcast,
            };
            hook(vm, &raised)
        };
        match event {
            "OnDeathNonGib" => raise(vm, "Custom", true)?,
            "E1" => {
                raise(vm, "E2", false)?;
                raise(vm, "E4", false)?;
            }
            "E2" => raise(vm, "E3", false)?,
            "OnBehaviorSequenceDisabled" => raise(vm, "Ping", true)?,
            // B1 broadcasts Custom, then selects its output link 0 (to B2).
            "Chain" if id.index == 7 => {
                raise(vm, "Custom", true)?;
                return Ok(vec![0]);
            }
            _ => {}
        }
        Ok(Vec::new())
    });
    register(&mut vm, "WillowGame.Behavior_ToySame", handler).unwrap();
    Reentry {
        vm,
        pawn,
        death: death_holder,
        _dir: Dir(dir),
    }
}

/// The `behavior ...` and `provider error: ...` events so far.
fn behavior_lines(vm: &Vm) -> Vec<String> {
    crate::hit::with_state(vm, |s| s.events.clone())
        .unwrap()
        .into_iter()
        .filter(|e| e.starts_with("behavior ") || e.starts_with("provider error"))
        .collect()
}

fn raise_death(r: &mut Reentry, name: &str) {
    let event = BehaviorEvent {
        pawn: r.pawn,
        definition: Some(r.death),
        killer: None,
        name: name.into(),
        broadcast: false,
    };
    raise_behavior_event(&mut r.vm, &event).unwrap();
}

/// After a raise: every provider is back, nothing waits, no raise is open.
fn settled(vm: &Vm, providers_made: usize) {
    let service = providers(vm).unwrap();
    let s = service.borrow();
    assert_eq!(
        s.held.len(),
        providers_made,
        "one provider per pawn and kind, no copy"
    );
    assert!(
        s.held.values().all(|h| h.provider.is_some()),
        "every provider is back"
    );
    assert!(s.deferred.is_empty());
    assert_eq!(s.depth, 0);
}

#[test]
fn a_broadcast_from_a_running_death_provider_runs_on_the_death_provider_then_the_class_one() {
    let mut r = reentry_vm();
    raise_death(&mut r, "OnPlayDeathPizazz");
    raise_death(&mut r, "OnDeathNonGib");
    // The C++ targets [death, class] in that order; the running death provider's share waits for B's event, and the
    // class provider's waits right behind it.
    assert_eq!(
        behavior_lines(&r.vm),
        [
            "behavior OnPlayDeathPizazz -> A",
            "behavior OnDeathNonGib -> B",
            "behavior Custom -> C",
            "behavior Custom -> K",
        ]
    );
    settled(&r.vm, 2);
}

/// The order a death sequence prints when a behavior broadcasts and the sequence goes on after it (item 4 of lane
/// W6-COMBAT-RUN2, decision 2026-11-16-54). The C++ re-enters the running death provider: B1, then Custom on the death
/// provider (C) and on the class provider (K), then B2, so `B1, C, K, B2`. This port defers Custom until the
/// outermost raise's own event has returned, so `B1, B2, C, K`: a named difference (decision 2026-11-15-35, the cost
/// of deferring), pinned here so a change to the deferral shows.
#[test]
fn a_broadcast_in_the_middle_of_a_death_sequence_runs_after_the_sequence_unlike_the_cxx() {
    const CXX_ORDER: [&str; 4] = [
        "behavior Chain -> B1",
        "behavior Custom -> C",
        "behavior Custom -> K",
        "behavior Chain -> B2",
    ];
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let mut r = reentry_vm_in(mode);
        raise_death(&mut r, "Chain");
        let got = behavior_lines(&r.vm);
        assert_eq!(
            got,
            [
                "behavior Chain -> B1",
                "behavior Chain -> B2",
                "behavior Custom -> C",
                "behavior Custom -> K",
            ],
            "{mode:?}"
        );
        assert_ne!(got, CXX_ORDER, "the C++ order is not reproduced: the difference is named");
        let mut sorted_got = got.clone();
        sorted_got.sort();
        let mut sorted_cxx = CXX_ORDER.map(String::from).to_vec();
        sorted_cxx.sort();
        assert_eq!(sorted_got, sorted_cxx, "the same behaviors run, only the order differs");
        settled(&r.vm, 2);
    }
}

#[test]
fn events_deferred_by_a_deferred_event_run_before_the_ones_queued_behind_it() {
    let mut r = reentry_vm();
    raise_death(&mut r, "E1");
    // E1 raises E2 then E4; E2 raises E3. Depth first, as the C++ re-entry runs them: E1, E2, E3, E4.
    assert_eq!(
        behavior_lines(&r.vm),
        [
            "behavior E1 -> P1",
            "behavior E2 -> P2",
            "behavior E3 -> P3",
            "behavior E4 -> P4"
        ]
    );
    settled(&r.vm, 1);
}

#[test]
fn the_sequence_step_is_one_raise_so_its_events_reach_the_provider_after_it() {
    let mut r = reentry_vm_in(ParityMode::Reference);
    assert!(set_sequence(&mut r.vm, r.pawn, "Main", false).unwrap());
    // The tripwire's labelled note (Reference): Ping was deferred to the provider the step held.
    assert_eq!(
        r.vm.log
            .iter()
            .filter(|l| l.starts_with(SEQUENCE_DEFERRAL_NOTE))
            .collect::<Vec<_>>(),
        [&format!("{SEQUENCE_DEFERRAL_NOTE} (Ping)")]
    );
    // S (the disabled event) broadcasts Ping: the death provider has no Ping, and the class provider, held by the
    // step, takes it once the step has put it back. No runaway, no error. By then Main is disabled, so Q does not
    // run: the C++ re-enters and runs Q before the bit is cleared (decision 2026-11-15-35, the deferral's cost).
    assert_eq!(
        behavior_lines(&r.vm),
        ["behavior OnBehaviorSequenceDisabled -> S"]
    );
    let service = providers(&r.vm).unwrap();
    let trace = service.borrow().held[&(r.pawn, Which::Class)]
        .provider
        .as_ref()
        .unwrap()
        .trace
        .clone();
    assert_eq!(trace, ["OnBehaviorSequenceDisabled -> S"]);
    drop(service);
    settled(&r.vm, 2);
    assert!(!set_sequence(&mut r.vm, r.pawn, "NoSuchSequence", true).unwrap());
}

#[test]
fn off_mode_refuses_a_sequence_step_whose_events_raise_on_its_own_class_provider() {
    let mut r = reentry_vm_in(ParityMode::Off);
    let e = set_sequence(&mut r.vm, r.pawn, "Main", false).unwrap_err();
    assert!(matches!(&e, VmError::NotYet(n) if matches!(n.missing, ow_core::Missing::NotPorted)), "{e:?}");
    assert!(error_text(&e).contains("raised Ping on the same class provider"), "{}", error_text(&e));
    // Nothing deferred runs after the refusal (Ping is dropped, so nothing lists S: the provider's trace holds it),
    // nothing is left deferred, every provider is back, and no note.
    assert!(behavior_lines(&r.vm).is_empty());
    let service = providers(&r.vm).unwrap();
    let trace = service.borrow().held[&(r.pawn, Which::Class)]
        .provider
        .as_ref()
        .unwrap()
        .trace
        .clone();
    assert_eq!(trace, ["OnBehaviorSequenceDisabled -> S"]);
    drop(service);
    // The broadcast made the death provider too (it has no Ping), as in Reference.
    settled(&r.vm, 2);
    assert!(!r.vm.log.iter().any(|l| l.starts_with(SEQUENCE_DEFERRAL_NOTE)));
    // A step whose events raise nothing on the provider is not refused.
    assert!(!set_sequence(&mut r.vm, r.pawn, "NoSuchSequence", true).unwrap());
}

#[test]
fn a_raise_on_a_provider_held_outside_every_raise_fails_at_once_naming_the_hold() {
    let mut r = reentry_vm();
    // The class provider is held by hand, outside any raise: the take-damage raise defers to it, and its drain finds
    // it still held.
    {
        let service = providers(&r.vm).unwrap();
        service.borrow_mut().held.insert(
            (r.pawn, Which::Class),
            Held {
                provider: None,
                reported: 0,
            },
        );
    }
    let event = TakeDamageEvent {
        pawn: r.pawn,
        instigator: None,
        damage_type: None,
        damage: 1.0,
        shield_damage: 0.0,
        source: None,
    };
    let e = raise_take_damage(&mut r.vm, &event).unwrap_err();
    assert_eq!(
        error_text(&e),
        "combat-run: OnTakeDamage was raised on the class provider of a pawn while that provider is held outside the raise"
    );
    let service = providers(&r.vm).unwrap();
    assert!(
        service.borrow().deferred.is_empty(),
        "a failed raise leaves nothing deferred"
    );
    assert_eq!(service.borrow().depth, 0);
}

// ---------------------------------------------------------------------------------------------------------------
// AdjustDamage's stages left out, through a real hit (review r2 of PR #335)
// ---------------------------------------------------------------------------------------------------------------

/// A hit with no instigator on a bare copy of `archetype` (no mind, so the source stages read nothing the toy classes
/// lack), with `prepare` applied to the copy first.
fn bare_hit(
    mode: ParityMode,
    archetype: &str,
    damage_type: Option<&str>,
    prepare: impl FnOnce(&mut Run, ow_object::ids::ObjRef),
) -> Result<crate::hit::HitReport, VmError> {
    let (mut run, _dir) = fresh(mode);
    run.apply("spawn:ToyPawns/Pawn_Armored:4");
    run.apply("pool:0:Pool_Ammo");
    let victim = crate::hit::instantiate_copy(&mut run.world().vm, "ToyPawns", archetype)
        .unwrap()
        .unwrap();
    prepare(&mut run, victim);
    let vm = &mut run.world().vm;
    let t = damage_type.map(|p| def(vm, p));
    crate::hit::hit_with_type(vm, victim, None, 100.0, t, None)
}

#[test]
fn a_damage_surface_no_region_names_is_not_yet_in_off_mode_and_flesh_in_reference_mode() {
    // The default region of BodyClass_Toy names no surface.
    match bare_hit(
        ParityMode::Off,
        "Pawn_Bodied",
        Some("DamageType_Toy"),
        |_, _| {},
    ) {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, ow_core::Missing::NotRead);
            assert!(
                n.to_string()
                    .contains("the damage surface of a hit whose region names none"),
                "{n}"
            );
        }
        other => panic!("{other:?}"),
    }
    let r = bare_hit(
        ParityMode::Reference,
        "Pawn_Bodied",
        Some("DamageType_Toy"),
        |_, _| {},
    )
    .unwrap();
    assert!(
        r.events
            .iter()
            .any(|e| e == "AdjustDamage surface=1 modifier=0.5"),
        "{:?}",
        r.events
    );
    assert_eq!(r.applied, 50.0, "100 x 1.0 (the region) x 0.5 (flesh)");
    // Without a damage type the surface selects nothing: Off goes on.
    let r = bare_hit(ParityMode::Off, "Pawn_Bodied", None, |_, _| {});
    assert!(
        matches!(&r, Err(VmError::NotYet(n)) if n.to_string().contains("PlayerAvertDeathChance")),
        "{r:?}"
    );
}

#[test]
fn a_victim_with_a_shield_pool_is_not_yet_in_off_mode_and_noted_in_reference_mode() {
    let shield = |run: &mut Run, victim| {
        let reference = run.pawns[0].pools[0].clone();
        crate::hit::access::set(&mut run.world().vm, victim, "ShieldArmor", reference).unwrap();
    };
    match bare_hit(ParityMode::Off, "Pawn_Armored", None, shield) {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, ow_core::Missing::NotPorted);
            assert!(n.to_string().contains("shield stage"), "{n}");
        }
        other => panic!("{other:?}"),
    }
    let r = bare_hit(ParityMode::Reference, "Pawn_Armored", None, shield).unwrap();
    assert!(
        r.events.iter().any(|e| e == "note: AdjustDamage shield stage for a victim with a shield pool is not implemented"),
        "{:?}",
        r.events
    );
    assert_eq!(
        r.applied, 80.0,
        "the shield is not modelled: 100 / (1 + 0.25) reaches health"
    );
    // An invalid reference is no shield: Off reaches the next refusal, averted death.
    let r = bare_hit(ParityMode::Off, "Pawn_Armored", None, |_, _| {});
    assert!(
        matches!(&r, Err(VmError::NotYet(n)) if n.to_string().contains("PlayerAvertDeathChance")),
        "{r:?}"
    );
}

/// The game's `EffectiveNumPlayers` after `steps` on a fresh run (review r1 of PR #348, decision 2026-11-16-49).
fn effective_players(mode: ParityMode, steps: &[&str]) -> Value {
    let (mut run, _dir) = fresh(mode);
    for step in steps {
        run.apply(step);
    }
    assert!(!run.failed(), "{:?}", run.errors());
    let vm = &mut run.world().vm;
    let world = with_state(vm, |s| s.world).unwrap().unwrap();
    let game = object(vm, world, "Game").unwrap().unwrap();
    get(vm, game, "EffectiveNumPlayers").unwrap().unwrap()
}

/// Scenario setup for one local player: the run's first `player:` step sets the game's count from its default 0 to 1
/// (the C++'s fixed `NumberOfPlayers` of 1); a second player leaves it, a later `players:<n>` wins, and a run with no
/// player keeps the default. Both parity modes.
#[test]
fn the_first_player_makes_the_games_player_count_one() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        assert_eq!(effective_players(mode, &[]), Value::Int(0), "{mode:?}");
        assert_eq!(
            effective_players(mode, &["player:4"]),
            Value::Int(1),
            "{mode:?}"
        );
        assert_eq!(
            effective_players(mode, &["player:4", "player:7"]),
            Value::Int(1),
            "{mode:?}"
        );
        assert_eq!(
            effective_players(mode, &["player:4", "players:3"]),
            Value::Int(3),
            "{mode:?}"
        );
        // A count set before the first player is kept.
        assert_eq!(
            effective_players(mode, &["players:2", "player:4"]),
            Value::Int(2),
            "{mode:?}"
        );
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The mission bridge's player (lane W6-COMBAT-RUN2, decision 2026-11-16-54), through a fake bridge over the toy
// classes: a controller, its replication info listed in the game's PRIArray, a player pawn; ExpEarn adds to the
// fake's pool, as the real bridge's native does.
// ---------------------------------------------------------------------------------------------------------------

/// A [`PlayerBridge`] over invented objects.
struct FakeBridge {
    controller: ObjRef,
    pawn: ObjRef,
    class: Option<ObjRef>,
    level: std::cell::Cell<i32>,
    pool: Rc<std::cell::Cell<f32>>,
    set: std::cell::RefCell<Vec<(i32, i64)>>,
}

impl PlayerBridge for FakeBridge {
    fn player_controller(&self) -> ObjRef {
        self.controller
    }
    fn player_pawn(&self, _vm: &mut Vm) -> Result<ObjRef, VmError> {
        Ok(self.pawn)
    }
    fn set_player_experience(&self, vm: &mut Vm, level: i32, experience: i64) -> Result<(), VmError> {
        let info = object(vm, self.controller, "PlayerReplicationInfo")?.unwrap();
        set(vm, info, "ExpLevel", Value::Int(level))?;
        self.level.set(level);
        // As the bridge: the C++ float(int64_t).
        self.pool.set(experience as f32);
        self.set.borrow_mut().push((level, experience));
        Ok(())
    }
    fn update_experience_pool(&self, _vm: &mut Vm) -> Result<(), VmError> {
        Ok(())
    }
    fn take_gains(&self) -> Vec<BridgeGain> {
        vec![BridgeGain::new("experience", 7, 0.0, 0.0)]
    }
    fn player_level(&self, _vm: &Vm) -> Result<i32, VmError> {
        Ok(self.level.get())
    }
    fn experience_pool(&self) -> f32 {
        self.pool.get()
    }
    fn errors(&self) -> Vec<String> {
        Vec::new()
    }
    fn player_class(&self, _vm: &mut Vm) -> Result<Option<ObjRef>, VmError> {
        Ok(self.class)
    }
}

/// A bridged run over the fixture in `mode`. With `class`, the player's class names the toy `Pool_Health` definition
/// as its health pool (it has no `BaseMaxValue`: the C++ seed's error path).
fn bridged(mode: ParityMode, class: bool) -> (Run, Rc<FakeBridge>, Dir) {
    let (mut w, dir) = world(mode);
    let vm = &mut w.vm;
    let roles = ow_behavior::provider::enum_names(vm, "Engine", "Actor.ENetRole").unwrap();
    let authority = u8::try_from(roles.iter().position(|r| r == "ROLE_Authority").unwrap()).unwrap();
    let info = make(vm, "Engine.WorldInfo");
    set(vm, info, "Role", Value::Byte(authority)).unwrap();
    let gri = make(vm, "WillowGame.WillowGameReplicationInfo");
    set(vm, info, "GRI", Value::Object(Some(gri))).unwrap();
    let controller = make(vm, "WillowGame.WillowPlayerController");
    let pri = make(vm, "WillowGame.WillowPlayerReplicationInfo");
    set(vm, controller, "PlayerReplicationInfo", Value::Object(Some(pri))).unwrap();
    set(vm, controller, "Role", Value::Byte(authority)).unwrap();
    set(vm, pri, "Owner", Value::Object(Some(controller))).unwrap();
    set(vm, gri, "PRIArray", Value::array(vec![Value::Object(Some(pri))])).unwrap();
    let pawn = make(vm, "WillowGame.WillowPlayerPawn");
    set(vm, pawn, "Controller", Value::Object(Some(controller))).unwrap();
    let pool = Rc::new(std::cell::Cell::new(0.0f32));
    let earned = Rc::clone(&pool);
    vm.override_script(
        "WillowGame.WillowPlayerController.ExpEarn",
        Rc::new(move |c: &mut ow_vm::api::NativeCall<'_>| {
            let amount = crate::hit::access::integer(c.arg(0)?);
            earned.set(earned.get() + amount as f32);
            Ok(Value::None)
        }),
    )
    .unwrap();
    let class = if class {
        let definition = def(vm, "ToyPawns.Pool_Health");
        let c = make(vm, "WillowGame.AIClassDefinition");
        set(vm, c, "HealthPoolDefinition", Value::Object(Some(definition))).unwrap();
        Some(c)
    } else {
        None
    };
    w.actors().borrow_mut().place_world_info(info).unwrap();
    let fake = Rc::new(FakeBridge {
        controller,
        pawn,
        class,
        level: std::cell::Cell::new(0),
        pool,
        set: std::cell::RefCell::new(Vec::new()),
    });
    let bridge: Rc<dyn PlayerBridge> = fake.clone();
    (Run::with_bridge(w, Some(bridge)).unwrap(), fake, dir)
}

/// The report without its timing fields (`milliseconds`, `elapsed_ms`).
fn untimed(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find("\"milliseconds\":").or_else(|| rest.find("\"elapsed_ms\":")) {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        let end = tail.find([',', '}']).unwrap_or(tail.len());
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

#[test]
fn the_bridge_steps_without_the_bridge_answer_the_cxx_texts() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let (text, failed) = run_steps(mode, &["tick:1", "cash:1", "mayahealth:1", "bleed:1"]);
        assert!(failed);
        for e in [
            "tick:1: combat-run: tick needs the mission bridge",
            "cash:1: combat-run: cash needs the mission bridge (the step maya:<level>)",
            "mayahealth:1: combat-run: mayahealth needs the mission bridge (the step maya:<level>)",
            "bleed:1: combat-run: bleed needs the mission bridge (the step maya:<level>)",
        ] {
            assert!(errors_of(&text).contains(e), "{mode:?}: {e}: {text}");
        }
        let (mut run, _dir) = fresh(mode);
        run.apply("maya:8");
        assert_eq!(
            run.errors(),
            ["maya:8: combat-run: maya needs the mission bridge (it is built when a maya step is among the steps)"]
        );
    }
}

#[test]
fn maya_sets_the_bridge_player_and_builds_the_cxx_health_pool_only_in_reference_mode() {
    // Reference: the C++ stand-in pool (manager on the controller, the class's definition, the pawn's reference to
    // slot 0); the toy definition has no BaseMaxValue, so the seed is the C++ script error, and the step goes on.
    let (mut run, fake, _dir) = bridged(ParityMode::Reference, true);
    run.apply("maya:4:short3");
    assert_eq!(*fake.set.borrow(), [(4, 0), (4, -3)], "short3 is 3 short of a next level of 0");
    assert_eq!(
        run.errors(),
        [
            "maya:4:short3: mission script: mission script bridge: the health pool definition has no BaseMaxValue",
            "maya:4:short3: mission script: mission script bridge: the health pool definition has no BaseMaxValue",
        ]
    );
    let vm = &mut run.world().vm;
    let manager = object(vm, fake.controller, "ResourcePoolManager").unwrap().expect("a manager");
    let reference = get(vm, fake.pawn, "HealthPool").unwrap().unwrap();
    assert_eq!(field(vm, &reference, "PoolManager").as_ref().and_then(object_of), Some(manager));
    let pool = field(vm, &reference, "Data").as_ref().and_then(object_of).expect("a pool");
    assert_eq!(object(vm, pool, "AssociatedProvider").unwrap(), Some(fake.controller));
    // The run's player count follows the bridge's player (decision 2026-11-16-49's setup, extended).
    let world = with_state(vm, |s| s.world).unwrap().unwrap();
    let game = object(vm, world, "Game").unwrap().unwrap();
    assert_eq!(get(vm, game, "EffectiveNumPlayers").unwrap(), Some(Value::Int(1)));

    // Off: the experience is set, then the stand-in pool is NotYet; nothing is built.
    let (mut run, fake, _dir) = bridged(ParityMode::Off, true);
    run.apply("maya:4");
    assert_eq!(*fake.set.borrow(), [(4, 0)]);
    assert_eq!(run.errors().len(), 1);
    assert!(run.errors()[0].contains("only ParityMode::Reference reproduces"), "{:?}", run.errors());
    let vm = &mut run.world().vm;
    assert_eq!(object(vm, fake.controller, "ResourcePoolManager").unwrap(), None);
}

#[test]
fn a_bridged_kill_credits_what_the_bridge_pool_gained() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let (mut run, fake, _dir) = bridged(mode, false);
        for step in ["spawn:ToyPawns/Pawn_Doomed:5", "maya:4", "players:1", "hit:0:10::Bullet"] {
            run.apply(step);
        }
        // Reference: a player class without a health pool builds none, quietly (the C++ ensureHealthPool). Off: the
        // stand-in pool is NotYet, and the kill goes on.
        let pool_errors: Vec<&String> = run.errors().iter().filter(|e| !e.starts_with("maya:4:")).collect();
        assert!(pool_errors.is_empty(), "{mode:?}: {:?}", run.errors());
        let gained = player::credit(0.0, fake.pool.get()).unwrap();
        assert!(gained > 0, "{mode:?}: the kill credited the bridge's pool");
        let report = run.report(0.0);
        let at = report
            .find("\"kill.Doomed.L5.p4\":{\"award\":")
            .unwrap_or_else(|| panic!("{mode:?}: {report}"));
        let row = &report[at..at + report[at..].find('}').unwrap()];
        assert!(row.ends_with(&format!(",\"credit\":{gained}")), "{mode:?}: {row}");
    }
}

#[test]
fn cash_and_bleed_frames_on_the_bridge() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let (mut run, fake, _dir) = bridged(mode, false);
        run.apply("cash:250");
        let vm = &mut run.world().vm;
        let info = object(vm, fake.controller, "PlayerReplicationInfo").unwrap().unwrap();
        assert_eq!(crate::down::currency_on_hand(vm, info, 0).unwrap(), 250);
        // One second in frames of 0.25 s: four frames, the time a float sum; no injury (the pawn is in no state).
        run.apply("bleed:1:0.25");
        run.apply("bleed:1:0");
        run.apply("bleed:1:2:3");
        let report = run.report(0.0);
        assert!(report.contains("{\"step\":\"bleed:1:0.25\",\"result\":{\"time\":1,\"down\":{\"state\":\"\",\"injury\":0"), "{mode:?}: {report}");
        assert_eq!(
            run.errors(),
            [
                "bleed:1:0: combat-run: bleed: the frame time must be positive",
                "bleed:1:2:3: combat-run: bleed:<seconds>[:<dt>]",
            ],
            "{mode:?}"
        );
    }
    assert_eq!(player::frames(50.0, 1.0 / 60.0).unwrap(), 3000);
    assert_eq!(player::frames(0.5, 1.0).unwrap(), 1, "lround: half away from zero");
    assert!(player::frames(1.0, 1e-300).is_err());
    assert_eq!(player::credit(28116.0, 28151.0).unwrap(), 35);
    assert!(player::credit(0.0, f32::INFINITY).is_err());
}

#[test]
fn tick_lists_the_bridge_gains_and_the_player() {
    let (mut run, _fake, _dir) = bridged(ParityMode::Off, false);
    run.apply("tick:2");
    let report = run.report(0.0);
    assert!(
        report.contains("{\"step\":\"tick:2\",\"result\":{\"gains\":[{\"kind\":\"experience\",\"amount\":7,\"health\":0,\"maximum\":0}],\"player\":{\"level\":0,\"experience\":0,\"next_level_at\":0,"),
        "{report}"
    );
}

#[test]
fn a_bridged_run_replays_to_the_same_report_in_both_modes() {
    let steps = [
        "spawn:ToyPawns/Pawn_Doomed:5",
        "maya:4",
        "players:1",
        "cash:120",
        "hit:0:10::Bullet",
        "bleed:0.5",
        "tick:1",
    ];
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let reports: Vec<String> = (0..2)
            .map(|_| {
                let (mut run, _fake, _dir) = bridged(mode, true);
                for s in steps {
                    run.apply(s);
                }
                untimed(&run.report(0.0))
            })
            .collect();
        assert_eq!(reports[0], reports[1], "{mode:?}");
    }
}
