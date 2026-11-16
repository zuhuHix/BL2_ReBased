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
    ];
    let death = Seq {
        name: "Death",
        enabled_on_spawn: Some(true),
        events: events(&death_events),
        behaviors: behaviors(&["A", "B", "C", "P1", "P2", "P3", "P4"]),
        links: links(7),
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
    let handler = Handler::run(move |vm: &mut Vm, _p, _id, event: &str| {
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
    let mut r = reentry_vm();
    assert!(set_sequence(&mut r.vm, r.pawn, "Main", false).unwrap());
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
