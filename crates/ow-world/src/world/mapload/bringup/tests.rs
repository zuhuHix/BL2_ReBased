//! Owners: lanes MAP-LOAD-NATIVES and MAP-LOAD-C. Tests of the bring-up on invented packages (AGENTS.md rule 9), in
//! both parity modes: the reach and the game info over the map-load fixture, the routing over the world fixture (whose
//! `Probe` notes 11 `PreBeginPlay`, 12 `PostBeginPlay`, 13 `SetInitialState` and 17 `InitGame`).

use super::*;

use std::collections::BTreeMap;
use std::fmt::Write as _;

use ow_core::{Missing, PackageName, ParityMode};
use ow_object::ids::PropAddr;

use super::super::load_map;
use super::super::tests::{core, engine, map, with_world};
use crate::world::tests::{Fx, fx_in};

fn files() -> Vec<(&'static str, Vec<u8>)> {
    vec![("Core.upk", core()), ("Engine.upk", engine()), ("Test_P.upk", map(3, &[4, 5, 6], None, false))]
}

fn write(w: &mut World, o: ObjRef, name: &str, v: Value) {
    let class = w.vm.space.heap.get(o).unwrap().class;
    let p = w.vm.space.prop(class, name).unwrap();
    w.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
}

fn not_yet(e: VmError) -> NotYet {
    match e {
        VmError::NotYet(n) => n,
        other => panic!("expected NotYet, got {other:?}"),
    }
}

const EVERY: BringUpOptions = BringUpOptions { reset_time: true, mode: RouteMode::EveryActor };

/// The reach (second pass: `bScriptInitialized`, not pending deletion, is what the routing skips; a static actor
/// routes only with `bRouteBeginPlayEvenIfStatic`), and the game info's `NotYet` before anything changes.
#[test]
fn the_reach_follows_the_second_pass_and_no_game_info_changes_nothing() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let (reach, refused, begun) = with_world("reach", files(), mode, |w| {
            load_map(w, &PackageName::parse("Test_P").unwrap()).unwrap();
            let list = w.actors().borrow().list().to_vec();
            // Actor 1 static, actor 2 static that routes, actor 3 already script-initialised.
            write(w, list[1], "bStatic", Value::Bool(true));
            write(w, list[2], "bStatic", Value::Bool(true));
            write(w, list[2], "bRouteBeginPlayEvenIfStatic", Value::Bool(true));
            write(w, list[3], "bScriptInitialized", Value::Bool(true));
            let reach = begin_play_reach(w).unwrap();
            let refused = bring_up_for_play(w, EVERY).map(|_| ()).map_err(|e| not_yet(e).what.to_string());
            (reach, refused, w.actors().borrow().begun_play())
        });
        assert_eq!(
            reach,
            BeginPlayReach {
                actors: 4,
                script_initialized: 1,
                routes: 2,
                static_unrouted: 1,
                world_info_begun_play: Some(false),
            },
            "{mode:?}"
        );
        assert!(refused.unwrap_err().contains("SetGameType"), "{mode:?}: the class choice");
        assert!(!begun, "{mode:?}: nothing changed");
    }
}

#[test]
fn the_game_info_without_a_world_info_waits_for_the_map_load() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let missing = with_world("empty", files(), mode, |w| game_info(w).map(|_| ()).map_err(|e| not_yet(e).missing));
        assert_eq!(missing, Err(Missing::LayerAbsent("the map load (mapload::load_map)")), "{mode:?}");
    }
}

// ------------------------------------------------------------------------------------------ the routing

/// Every actor class's defaults load at every physics level (the installed defaults do).
fn load_everywhere(f: &mut Fx) {
    for class in ["Probe", "Dud", "WorldInfo"] {
        let cdo = f.w.vm.defs.class_default(&mut f.w.vm.space, &format!("Engine.{class}")).unwrap();
        for level in ["bLoadIfPhysXLevel0", "bLoadIfPhysXLevel1", "bLoadIfPhysXLevel2"] {
            actors::set_field(&mut f.w.vm.space, cdo, level, Value::Bool(true)).unwrap();
        }
    }
    let wi = f.wi;
    for level in ["bLoadIfPhysXLevel0", "bLoadIfPhysXLevel1", "bLoadIfPhysXLevel2"] {
        actors::set_field(&mut f.w.vm.space, wi, level, Value::Bool(true)).unwrap();
    }
}

/// A placed actor of `class` (before play begins).
fn place(f: &mut Fx, class: &str) -> ObjRef {
    let c = f.class(class);
    let o = ow_vm::interp::instantiate_from_defaults(&mut f.w.vm, c).unwrap();
    f.w.actors().borrow_mut().place(o).unwrap();
    o
}

fn set(f: &mut Fx, o: ObjRef, name: &str, v: Value) {
    actors::set_field(&mut f.w.vm.space, o, name, v).unwrap();
}

fn flag_of(f: &Fx, o: ObjRef, name: &str) -> bool {
    actors::flag(&f.w.vm.space, o, name).unwrap()
}

/// The fixture with its `WorldInfo`, a game info `g` (a `Probe`, in `WorldInfo.Game`), and the actors of `classes`
/// placed in order.
fn routed_world(mode: ParityMode, classes: &[&str]) -> (Fx, ObjRef, Vec<ObjRef>) {
    let mut f = fx_in(false, mode);
    load_everywhere(&mut f);
    let placed: Vec<ObjRef> = classes.iter().map(|c| place(&mut f, c)).collect();
    let g = place(&mut f, "Probe");
    let wi = f.wi;
    set(&mut f, wi, "Game", Value::Object(Some(g)));
    (f, g, placed)
}

/// The note's order: the init pass, then `InitGame`, then every routing actor's `PreBeginPlay` in list order, then
/// each one's `PostBeginPlay` and `SetInitialState`; a static actor without the flag only gets `bScriptInitialized`,
/// a script-initialised one nothing; the flags and the time fields as the note says. The same in both modes.
#[test]
fn the_routing_runs_the_events_in_the_note_s_order() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        crate::world::tests::run(move || {
            let (mut f, g, a) = routed_world(mode, &["Probe", "Probe", "Probe", "Probe"]);
            let (moving, unrouted, routed_static, done) = (a[0], a[1], a[2], a[3]);
            set(&mut f, unrouted, "bStatic", Value::Bool(true));
            set(&mut f, routed_static, "bStatic", Value::Bool(true));
            set(&mut f, routed_static, "bRouteBeginPlayEvenIfStatic", Value::Bool(true));
            set(&mut f, done, "bScriptInitialized", Value::Bool(true));
            let wi = f.wi;
            set(&mut f, wi, "TimeSeconds", Value::Float(5.0));
            let up = bring_up_for_play(&mut f.w, EVERY).unwrap();
            assert!(up.complete(), "{mode:?}: {:?}", up.failures);
            assert_eq!(up.game, g);
            let notes = f.all_notes();
            let expected = vec![
                (g, 17),
                (moving, 11),
                (routed_static, 11),
                (g, 11),
                (moving, 12),
                (moving, 13),
                (routed_static, 12),
                (routed_static, 13),
                (g, 12),
                (g, 13),
            ];
            assert_eq!(notes, expected, "{mode:?}");
            assert_eq!(
                (up.routed.pre_begin_play, up.routed.post_begin_play, up.routed.set_initial_state),
                (3, 3, 3),
                "{mode:?}: the WorldInfo defines no event"
            );
            assert_eq!((up.routed.marked_static, up.routed.skipped_initialized), (1, 1), "{mode:?}");
            assert!(up.routed.init_game);
            assert_eq!(up.routed.init_pass, 6, "{mode:?}: every actor, the WorldInfo included");
            assert!(flag_of(&f, unrouted, "bScriptInitialized"), "{mode:?}");
            assert!(!flag_of(&f, moving, "bScriptInitialized"), "{mode:?}: the routing does not set it");
            assert!(flag_of(&f, wi, "bBegunPlay") && !flag_of(&f, wi, "bStartup"), "{mode:?}");
            assert!(f.w.actors().borrow().begun_play(), "{mode:?}");
            assert_eq!(actors::field(&f.w.vm.space, wi, "TimeSeconds").unwrap(), Value::Float(0.0));
            assert_eq!(actors::field(&f.w.vm.space, moving, "LastRenderTime").unwrap(), Value::Float(f32::MIN));
            assert_eq!(up.pending.len(), 9);
            // Once: a second bring-up raises nothing.
            f.notes.borrow_mut().clear();
            let again = bring_up_for_play(&mut f.w, EVERY).unwrap();
            assert!(f.all_notes().is_empty() && again.routed == Routed::default(), "{mode:?}");
        });
    }
}

/// Without `reset_time` the time fields are left; a script-initialised game info gets no `InitGame`.
#[test]
fn no_time_reset_and_no_init_game_for_an_initialised_game() {
    crate::world::tests::run(|| {
        let (mut f, g, _) = routed_world(ParityMode::Off, &[]);
        let wi = f.wi;
        set(&mut f, wi, "TimeSeconds", Value::Float(5.0));
        set(&mut f, g, "bScriptInitialized", Value::Bool(true));
        let up = bring_up_for_play(&mut f.w, BringUpOptions { reset_time: false, mode: RouteMode::EveryActor }).unwrap();
        assert_eq!(actors::field(&f.w.vm.space, wi, "TimeSeconds").unwrap(), Value::Float(5.0));
        assert!(!up.routed.init_game);
        assert!(f.all_notes().is_empty(), "the script-initialised game info gets no event either");
    });
}

/// A network client's world refuses as a whole before anything changes, the time reset included (review r1 of #409).
#[test]
fn a_client_world_refuses_before_any_change() {
    crate::world::tests::run(|| {
        let (mut f, _, _) = routed_world(ParityMode::Off, &["Probe"]);
        let wi = f.wi;
        set(&mut f, wi, "NetMode", Value::Byte(actors::NET_MODE_CLIENT));
        set(&mut f, wi, "TimeSeconds", Value::Float(5.0));
        let e = bring_up_for_play(&mut f.w, EVERY).unwrap_err();
        assert_eq!(not_yet(e).missing, Missing::NotRead);
        assert_eq!(actors::field(&f.w.vm.space, wi, "TimeSeconds").unwrap(), Value::Float(5.0));
        assert!(f.all_notes().is_empty() && !f.w.actors().borrow().begun_play());
        assert!(!flag_of(&f, wi, "bBegunPlay") && !flag_of(&f, wi, "bStartup"));
    });
}

/// The open mode value refuses before anything changes.
#[test]
fn the_skip_static_mode_is_not_read() {
    crate::world::tests::run(|| {
        let (mut f, _, _) = routed_world(ParityMode::Off, &["Probe"]);
        let e = bring_up_for_play(&mut f.w, BringUpOptions { reset_time: true, mode: RouteMode::SkipStatic }).unwrap_err();
        assert_eq!(not_yet(e).missing, Missing::NotRead);
        assert!(f.all_notes().is_empty() && !f.w.actors().borrow().begun_play());
    });
}

/// Failures are counted and the pass goes on: an actor that does not load at every physics level is `NotYet` in the
/// init pass and is not routed; one that destroys itself in `PreBeginPlay` gets no more events; one that needs a base
/// search stops there (`NotYet`, collision) after `SetInitialState`.
#[test]
fn failures_are_counted_and_the_pass_goes_on() {
    crate::world::tests::run(|| {
        let (mut f, _g, a) = routed_world(ParityMode::Off, &["Probe", "Dud", "Probe", "Probe"]);
        let (filtered, dud, based, fine) = (a[0], a[1], a[2], a[3]);
        set(&mut f, filtered, "bLoadIfPhysXLevel1", Value::Bool(false));
        set(&mut f, based, "bCollideWorld", Value::Bool(true));
        set(&mut f, based, "bShouldBaseAtStartup", Value::Bool(true));
        let up = bring_up_for_play(&mut f.w, EVERY).unwrap();
        assert!(!up.complete());
        let steps: Vec<(&str, ObjRef, bool)> = up.failures.iter().map(|x| (x.step, x.actor, x.not_yet)).collect();
        assert_eq!(steps, vec![("init_pass", filtered, true), ("base search", based, true)]);
        assert!(f.all_notes().iter().all(|(o, _)| *o != filtered), "the filtered actor is not routed");
        assert_eq!(up.routed.destroyed, 1, "the dud");
        assert!(!f.all_notes().iter().any(|(o, _)| *o == dud));
        let fine_notes: Vec<i32> = f.all_notes().iter().filter(|(o, _)| *o == fine).map(|(_, k)| *k).collect();
        assert_eq!(fine_notes, vec![11, 12, 13]);
        // Review r1 of #537: the base search's failure comes after the routing wrote (the dud's destroy, the events),
        // so it is marked; the init pass's failure, before the routing, is not.
        let marked: Vec<bool> = up.failures.iter().map(|x| ow_core::notyet::is_partial_writes(&x.text)).collect();
        assert_eq!(marked, vec![false, true]);
    });
}

/// `WillowGameInfo.TravelPostCountdownInProcess` (NATIVE_TRAVEL.md): `TravelPostCountdownTime > 0`, in Off; unbound
/// under Reference (the C++ stubs it).
#[test]
fn the_travel_countdown_answers_from_its_field() {
    crate::world::tests::run(|| {
        let mut f = fx_in(false, ParityMode::Off);
        let c = f.class("WillowGameInfo");
        let g = ow_vm::interp::instantiate_from_defaults(&mut f.w.vm, c).unwrap();
        for (n, want) in [(0, false), (3, true), (-1, false)] {
            set(&mut f, g, "TravelPostCountdownTime", Value::Int(n));
            let got = f.call("WillowGameInfo.TravelPostCountdownInProcess", Some(g), vec![]).unwrap();
            assert_eq!(got, Value::Bool(want), "{n}");
        }
        let r = fx_in(false, ParityMode::Reference);
        assert!(!r.w.vm.has_native(crate::world::TRAVEL_COUNTDOWN));
    });
}

// ------------------------------------------------------------------------------------------ installed

/// Sanctuary on the installed game (`OW_BL2_ROOT`) in Off: the map load, then a **harness set-up** (not the game's
/// class choice, which is `NotYet`): a `WillowGame.WillowGameInfo` spawned from its class defaults into
/// `WorldInfo.Game`; then the bring-up with the stock route mode and the time reset. Writes
/// `local/p2/map-load-c/bringup_Off.md` (counts only).
#[test]
#[ignore = "reads the installed game (OW_BL2_ROOT); writes local/p2/map-load-c/"]
fn sanctuary_brought_up_on_the_installed_game() {
    let root = std::env::var_os("OW_BL2_ROOT").expect("set OW_BL2_ROOT to the install folder");
    let cooked = std::path::Path::new(&root).join("WillowGame/CookedPCConsole");
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../local/p2/map-load-c");
    std::fs::create_dir_all(&out).unwrap();
    let mode = ParityMode::Off;
    let started = std::time::Instant::now();
    let text = ow_vm::interp::on_vm_stack(move || {
        let defs = ow_defs::DefStore::open(cooked, ow_defs::identity::IdentityPolicy::FirstInScanOrder, mode)
            .expect("the store");
        // The VM's core natives (operators, iterators), as the slice and combat runs register them.
        let vm = ow_vm::api::Vm::new(
            ow_object::heap::ObjectSpace::new(),
            defs,
            std::rc::Rc::new(ow_attr::AttributeSystem::standard().map_err(|e| e.to_string())?),
            mode,
        )
        .with_stub_policy(ow_vm::api::StubPolicy::refuse_all());
        let snapshot = ow_balance::types::BalanceSnapshot::new(
            ow_balance::types::Playthrough::NORMAL,
            ow_balance::types::OpLevel::NONE,
            ow_balance::types::PlayerCount::SINGLE,
            ow_balance::types::ExpLevel::new(8),
            ow_balance::types::GameStage(8),
            ow_balance::types::AwesomeLevel::new(0),
            ow_balance::types::LevelCapUpgrades::NONE,
        )
        .map_err(|e| e.to_string())?;
        let mut w = World::new(vm, crate::world::BalanceState::new(snapshot), &[&ow_vm::natives_core::Pack])
            .map_err(|e| e.to_string())?;
        let report = load_map(&mut w, &PackageName::parse("Sanctuary_P").unwrap()).map_err(|e| e.to_string())?;
        let mut r = String::new();
        let placed: usize = report.levels.iter().map(|l| l.placed_total).sum();
        let _ = writeln!(r, "# Sanctuary brought up ({mode:?}; local, counts only)");
        let _ = writeln!(r, "levels {}, placed {placed}", report.levels.len());
        let _ = writeln!(r, "game info before the harness: {:?}", game_info(&w).err().map(|e| e.to_string()));
        let class = w.vm.find_class("WillowGame.WillowGameInfo").map_err(|e| e.to_string())?;
        let info = report.world_info;
        let game = actors::spawn(&mut w.vm, info, &actors::SpawnRequest::new(Some(class)))
            .map_err(|e| e.to_string())?
            .ok_or("the game info spawn returned None")?;
        actors::set_field(&mut w.vm.space, info, "Game", Value::Object(Some(game))).map_err(|e| e.to_string())?;
        let before: Vec<ObjRef> = w.actors().borrow().list().to_vec();
        let up = bring_up_for_play(&mut w, EVERY).map_err(|e| e.to_string())?;
        let _ = writeln!(r, "reach before: {:?}", up.reach);
        let _ = writeln!(r, "routed: {:?}", up.routed);
        let _ = writeln!(r, "failures: {}", up.failures.len());
        let mut by: BTreeMap<(String, String), (usize, String)> = BTreeMap::new();
        for x in &up.failures {
            let key = (x.step.to_string(), x.text.clone());
            let e = by.entry(key).or_insert((0, x.class.clone()));
            e.0 += 1;
        }
        let mut sorted: Vec<_> = by.into_iter().collect();
        sorted.sort_by(|a, b| b.1.0.cmp(&a.1.0));
        for ((step, text), (n, class)) in sorted.iter().take(25) {
            let _ = writeln!(r, "  {n} x {step} (first {class}): {}", text.chars().take(240).collect::<String>());
        }
        let mut by_class: BTreeMap<(String, String), usize> = BTreeMap::new();
        for x in &up.failures {
            *by_class.entry((x.step.to_string(), x.class.clone())).or_default() += 1;
        }
        let mut sorted: Vec<_> = by_class.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        for ((step, class), n) in sorted.iter().take(25) {
            let _ = writeln!(r, "  by class: {n} x {step} {class}");
        }
        let mut gone: BTreeMap<String, usize> = BTreeMap::new();
        let after: Vec<ObjRef> = w.actors().borrow().list().to_vec();
        for o in before {
            if !after.contains(&o) || is_destroyed(&w.vm.space, o).unwrap_or(false) {
                *gone.entry(class_of(&w.vm, o)).or_default() += 1;
            }
        }
        let _ = writeln!(r, "destroyed, by class: {gone:?}");
        let _ = writeln!(r, "begun play {}", w.actors().borrow().begun_play());
        for p in &up.pending {
            let _ = writeln!(r, "pending: {p}");
        }
        Ok::<_, String>(r)
    })
    .expect("the VM thread")
    .expect("the bring-up");
    let text = format!("{text}elapsed {} ms\n", started.elapsed().as_millis());
    std::fs::write(out.join("bringup_Off.md"), &text).unwrap();
    print!("{text}");
}
