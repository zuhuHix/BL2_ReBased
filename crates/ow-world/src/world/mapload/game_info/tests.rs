//! Owner: lane IMPL-MAPLOAD-GAMEINFO. Tests of the game info routine and the map route on the world fixture (invented
//! packages, AGENTS.md rule 9), whose `Probe` notes 11 `PreBeginPlay`, 12 `PostBeginPlay`, 13 `SetInitialState` and
//! 17 `InitGame`. The fixture's `WorldInfo.Game` holds an `Actor`, so `Engine.Probe` stands for the configured game.

use super::*;

use ow_core::Missing;
use ow_core::notyet::is_partial_writes;

use crate::world::mapload::bringup::{BringUpOptions, RouteMode, bring_up_map_for_play};
use crate::world::tests::{Fx, fx_in, run};

const PROBE: GameInfoInput<'static> = GameInfoInput { options: "", default_game: Some("Engine.Probe") };
const EVERY: BringUpOptions = BringUpOptions { reset_time: true, mode: RouteMode::EveryActor };

fn set(f: &mut Fx, o: ObjRef, name: &str, v: Value) {
    actors::set_field(&mut f.w.vm.space, o, name, v).unwrap();
}

fn game_of(f: &Fx) -> Value {
    actors::field(&f.w.vm.space, f.wi, "Game").unwrap()
}

fn listed(f: &Fx) -> usize {
    f.w.actors().borrow().list().len()
}

/// A refusal before anything changed: a `NotYet` of `missing`, unmarked.
fn unmarked(e: VmError, missing: Missing) -> String {
    let VmError::NotYet(n) = e else { panic!("expected a NotYet, got {e:?}") };
    assert_eq!(n.missing, missing, "{}", n.what);
    assert!(!is_partial_writes(&n.what), "unmarked: {}", n.what);
    n.what.to_string()
}

/// Every class the bring-up visits loads at every physics level (the installed defaults do).
fn load_everywhere(f: &mut Fx) {
    for class in ["Probe", "WorldInfo"] {
        let cdo = f.w.vm.defs.class_default(&mut f.w.vm.space, &format!("Engine.{class}")).unwrap();
        for level in ["bLoadIfPhysXLevel0", "bLoadIfPhysXLevel1", "bLoadIfPhysXLevel2"] {
            actors::set_field(&mut f.w.vm.space, cdo, level, Value::Bool(true)).unwrap();
        }
    }
    let wi = f.wi;
    for level in ["bLoadIfPhysXLevel0", "bLoadIfPhysXLevel1", "bLoadIfPhysXLevel2"] {
        set(f, wi, level, Value::Bool(true));
    }
}

#[test]
fn the_ini_reader_takes_default_game_from_its_section_only() {
    let read = |t: &str| default_game_from_ini(t).unwrap();
    let ini = "; a comment\n[Engine.GameReplicationInfo]\nDefaultGame=Wrong.Section\n\n[engine.gameinfo]\n\
               ; DefaultGame=Commented.Out\n  defaultgame = Toy.ToyGame \nDefaultServerGame=Toy.Server\n";
    assert_eq!(read(ini).as_deref(), Some("Toy.ToyGame"), "any case, trimmed, its section only");
    assert_eq!(read("[Engine.GameInfo]\nDefaultServerGame=Toy.Server\n"), None, "no key");
    assert_eq!(read("[Engine.GameInfo]\nDefaultGame=\n"), None, "an empty value");
    assert_eq!(read("DefaultGame=Toy.NoSection\n"), None, "outside any section");
    assert_eq!(read("[Engine.GameInfo]\r\nDefaultGame=Toy.Crlf\r\n").as_deref(), Some("Toy.Crlf"));
    assert_eq!(read("\u{feff}[Engine.GameInfo]\nDefaultGame=Toy.Bom\n").as_deref(), Some("Toy.Bom"), "a BOM skipped");
}

/// The reader refuses instead of guessing (review r1 of #537): a second `DefaultGame` in the section (also in a second
/// copy of the section), a prefixed key, a quoted value. Each `NotYet { NotRead }`.
#[test]
fn the_ini_reader_refuses_what_it_would_have_to_guess() {
    for ini in [
        "[Engine.GameInfo]\nDefaultGame=Toy.First\nDefaultGame=Toy.Second\n",
        "[Engine.GameInfo]\nDefaultGame=Toy.First\n[Engine.Other]\n[Engine.GameInfo]\ndefaultgame=Toy.Second\n",
        "[Engine.GameInfo]\n+DefaultGame=Toy.Added\n",
        "[Engine.GameInfo]\n-DefaultGame=Toy.Removed\n",
        "[Engine.GameInfo]\n.DefaultGame=Toy.Dotted\n",
        "[Engine.GameInfo]\n!DefaultGame=Toy.Cleared\n",
        "[Engine.GameInfo]\nDefaultGame=\"Toy.Quoted\"\n",
    ] {
        let n = default_game_from_ini(ini).unwrap_err();
        assert_eq!(n.missing, Missing::NotRead, "{ini:?}");
    }
}

/// The installed reader caps the file (review r1 of #537): a file over the cap is refused, one at the cap is read.
#[test]
fn the_installed_ini_read_is_capped() {
    let root = std::env::temp_dir().join(format!("ow-world-ini-cap-{}", std::process::id()));
    let config = root.join("WillowGame/Config");
    std::fs::create_dir_all(&config).unwrap();
    let head = b"[Engine.GameInfo]\nDefaultGame=Toy.Capped\n;";
    let mut at_cap = head.to_vec();
    at_cap.resize(usize::try_from(MAX_INI_BYTES).unwrap(), b' ');
    std::fs::write(config.join("DefaultGame.ini"), &at_cap).unwrap();
    assert_eq!(installed_default_game(&root).unwrap().as_deref(), Some("Toy.Capped"));
    at_cap.push(b' ');
    std::fs::write(config.join("DefaultGame.ini"), &at_cap).unwrap();
    let e = installed_default_game(&root).unwrap_err();
    assert!(matches!(&e, VmError::Script { message, .. } if message.contains("larger than")), "{e:?}");
    let _ = std::fs::remove_dir_all(&root);
}

/// The mark follows the game RNG's draws too (review r1 of #537): a draw alone moves it.
#[test]
fn a_rng_draw_moves_the_write_mark() {
    run(|| {
        let mut f = fx_in(false, ParityMode::Off);
        let mark = WriteMark::now(&f.w.vm);
        f.w.vm.rng.script(ow_core::rand::ScriptStream::ALL[0]).frand();
        assert!(mark.moved(&f.w.vm));
    });
}

#[test]
fn the_game_option_is_found_in_the_joined_options() {
    assert_eq!(game_option(""), None);
    assert_eq!(game_option("?Name=Toy?Listen"), None);
    assert_eq!(game_option("?Name=Toy?game=Toy.ToyGame"), Some("Toy.ToyGame"));
    assert_eq!(game_option("?GAME=Toy.A?GAME=Toy.B"), Some("Toy.A"), "the first");
}

/// The configured default (`DefaultGameType` None, no `GAME=`): the class is spawned before play begins (no event
/// raised), appended to the actor list and stored in `WorldInfo.Game`.
#[test]
fn the_configured_default_is_spawned_into_world_info_game() {
    run(|| {
        let mut f = fx_in(false, ParityMode::Off);
        let before = listed(&f);
        let spawned = spawn_game_info(&mut f.w, &PROBE).unwrap();
        assert_eq!(spawned.branch, GameInfoBranch::ConfiguredDefault);
        assert_eq!(spawned.class, "Engine.Probe");
        assert_eq!(game_of(&f), Value::Object(Some(spawned.game)));
        assert_eq!(ow_attr::eval::class_path(&f.w.vm.space, spawned.game), "Engine.Probe");
        assert_eq!(listed(&f), before + 1);
        assert_eq!(f.w.actors().borrow().list().last(), Some(&spawned.game), "appended last");
        assert!(f.all_notes().is_empty(), "the world has not begun play: no begin-play events");
        // A second call finds it there and does nothing.
        let mark = WriteMark::now(&f.w.vm);
        let again = spawn_game_info(&mut f.w, &PROBE).unwrap();
        assert_eq!((again.game, again.branch), (spawned.game, GameInfoBranch::AlreadySet));
        assert!(!mark.moved(&f.w.vm), "nothing written");
    });
}

/// The branches the note leaves open, and the worlds the routine does not run on: each a `NotYet { NotRead }`,
/// unmarked, with nothing written, loaded or placed.
#[test]
fn the_open_branches_refuse_before_anything_changes() {
    type Setup = fn(&mut Fx) -> GameInfoInput<'static>;
    let cases: [(&str, bool, Setup); 6] = [
        ("DefaultGameType", false, |f| {
            let c = f.class("Probe");
            let wi = f.wi;
            set(f, wi, "DefaultGameType", Value::Class(Some(c)));
            PROBE
        }),
        ("GAME= option", false, |_| GameInfoInput { options: "?GAME=Engine.Probe", ..PROBE }),
        ("game= among options", false, |_| GameInfoInput { options: "?Name=Toy?game=Engine.Probe", ..PROBE }),
        ("configured default game", false, |_| GameInfoInput { default_game: None, ..PROBE }),
        ("not standalone", false, |f| {
            let wi = f.wi;
            set(f, wi, "NetMode", Value::Byte(1));
            PROBE
        }),
        ("after play began", true, |_| PROBE),
    ];
    for (name, begun, setup) in cases {
        run(move || {
            let mut f = fx_in(begun, ParityMode::Off);
            let input = setup(&mut f);
            let (mark, before) = (WriteMark::now(&f.w.vm), listed(&f));
            let what = unmarked(spawn_game_info(&mut f.w, &input).unwrap_err(), Missing::NotRead);
            assert!(!mark.moved(&f.w.vm), "{name}: nothing written or loaded ({what})");
            assert_eq!((game_of(&f), listed(&f)), (Value::Object(None), before), "{name}");
        });
    }
}

/// Reference is unchanged: the C++ has no map load, so the routine is not ported there.
#[test]
fn reference_has_no_game_info_routine() {
    run(|| {
        let mut f = fx_in(false, ParityMode::Reference);
        let mark = WriteMark::now(&f.w.vm);
        unmarked(spawn_game_info(&mut f.w, &PROBE).unwrap_err(), Missing::NotPorted);
        assert!(!mark.moved(&f.w.vm) && game_of(&f) == Value::Object(None));
    });
}

/// A class that is not there is the note's not-found text with the field left empty; a class `WorldInfo.Game` cannot
/// hold is a data error before the spawn; a spawn refused for collision (a placement search with no collision service)
/// is that `NotYet`, unmarked, since nothing was written before it.
#[test]
fn a_missing_or_unfit_class_and_a_refused_spawn_leave_the_field_empty() {
    run(|| {
        let mut f = fx_in(false, ParityMode::Off);
        let before = listed(&f);
        let e = spawn_game_info(&mut f.w, &GameInfoInput { default_game: Some("Engine.NoSuchGame"), ..PROBE });
        let e = e.unwrap_err();
        assert!(
            matches!(&e, VmError::Script { message, .. } if message.starts_with(&not_found_text("Engine.NoSuchGame"))),
            "no lazy load ran: unmarked ({e:?})"
        );
        let e = spawn_game_info(&mut f.w, &GameInfoInput { default_game: Some("Engine.Thing"), ..PROBE }).unwrap_err();
        assert!(matches!(&e, VmError::Script { message, .. } if message.contains("is not a class WorldInfo.Game can hold")), "{e:?}");
        let cdo = f.w.vm.defs.class_default(&mut f.w.vm.space, "Engine.Collider").unwrap();
        actors::set_field(&mut f.w.vm.space, cdo, "bCollideWorld", Value::Bool(true)).unwrap();
        let mark = WriteMark::now(&f.w.vm);
        let e = spawn_game_info(&mut f.w, &GameInfoInput { default_game: Some("Engine.Collider"), ..PROBE });
        let what = unmarked(e.unwrap_err(), Missing::LayerAbsent("collision (the world's collision service, a SpawnCollision)"));
        assert!(!mark.moved(&f.w.vm), "{what}");
        assert_eq!((game_of(&f), listed(&f)), (Value::Object(None), before));
    });
}

/// The marking rule on #518's counters: a `NotYet` after a committed write is marked once, its kind and record kept;
/// one with no write since the mark is not; another error is returned as it is.
#[test]
fn a_not_yet_after_a_write_is_marked_once() {
    run(|| {
        let mut f = fx_in(false, ParityMode::Off);
        let mark = WriteMark::now(&f.w.vm);
        let refusal = || -> VmError { NotYet::not_read("an invented refusal", "docs/invented.md").into() };
        unmarked(mark.mark(&f.w.vm, refusal()), Missing::NotRead);
        f.w.vm.space.heap.note_write();
        let VmError::NotYet(n) = mark.mark(&f.w.vm, refusal()) else { panic!("a NotYet") };
        assert!(is_partial_writes(&n.what) && n.what.ends_with("an invented refusal"), "{}", n.what);
        assert_eq!((n.missing, n.record), (Missing::NotRead, "docs/invented.md"));
        let again = mark.mark(&f.w.vm, n.into());
        assert_eq!(again.to_string().matches("stopped after partial writes").count(), 1, "once");
        let script = VmError::Script { function: "f".into(), message: "m".into() };
        assert_eq!(mark.mark(&f.w.vm, script.clone()).to_string(), script.to_string());
    });
}

/// The map route: the game info is in `WorldInfo.Game` before the bring-up, so the bring-up raises its `InitGame`
/// first, then routes it with the other actors (it was appended last). The navigation list is the route's pending
/// `NotYet { NotRead }`. The route mode the note leaves open refuses before the spawn, unmarked.
#[test]
fn the_map_route_spawns_the_game_info_then_brings_the_map_up() {
    run(|| {
        let mut f = fx_in(false, ParityMode::Off);
        load_everywhere(&mut f);
        let c = f.class("Probe");
        let placed = ow_vm::interp::instantiate_from_defaults(&mut f.w.vm, c).unwrap();
        f.w.actors().borrow_mut().place(placed).unwrap();

        let mark = WriteMark::now(&f.w.vm);
        let skip = BringUpOptions { reset_time: true, mode: RouteMode::SkipStatic };
        unmarked(bring_up_map_for_play(&mut f.w, &PROBE, skip).unwrap_err(), Missing::NotRead);
        assert!(!mark.moved(&f.w.vm) && game_of(&f) == Value::Object(None), "refused before the spawn");

        let up = bring_up_map_for_play(&mut f.w, &PROBE, EVERY).unwrap();
        let g = up.game_info.game;
        assert_eq!(up.bring_up.game, g, "the bring-up took the routine's game info");
        assert!(up.bring_up.complete(), "{:?}", up.bring_up.failures);
        assert!(up.bring_up.routed.init_game);
        let expected = vec![(g, 17), (placed, 11), (g, 11), (placed, 12), (placed, 13), (g, 12), (g, 13)];
        assert_eq!(f.all_notes(), expected);
        assert_eq!(up.pending.len(), 1);
        assert_eq!(up.pending[0].missing, Missing::NotRead);
        assert!(up.pending[0].what.contains("NavigationPointList"));
    });
}

// ------------------------------------------------------------------------------------------ installed

/// Sanctuary on the installed game (`OW_BL2_ROOT`) in Off through the map route: the map load, the game info routine
/// with the installed `DefaultGame.ini` and no URL options, then the bring-up. Writes
/// `local/p2/mapload-gameinfo/map_route_Off.md` (class paths and counts only): the class chosen, whether the script
/// filled `WorldInfo.GRI` and the GRI's `MissionTracker`, the failures on the game info itself (its first stop), and
/// the most common failures over the map.
#[test]
#[ignore = "reads the installed game (OW_BL2_ROOT); writes local/p2/mapload-gameinfo/"]
fn sanctuary_through_the_map_route_on_the_installed_game() {
    use std::collections::BTreeMap;
    use std::fmt::Write as _;

    let root = std::env::var_os("OW_BL2_ROOT").expect("set OW_BL2_ROOT to the install folder");
    let root = std::path::PathBuf::from(root);
    let cooked = root.join("WillowGame/CookedPCConsole");
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../local/p2/mapload-gameinfo");
    std::fs::create_dir_all(&out).unwrap();
    let mode = ParityMode::Off;
    let started = std::time::Instant::now();
    let text = ow_vm::interp::on_vm_stack(move || {
        let default_game = installed_default_game(&root).map_err(|e| e.to_string())?;
        let defs = ow_defs::DefStore::open(cooked, ow_defs::identity::IdentityPolicy::FirstInScanOrder, mode)
            .expect("the store");
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
        let report = super::super::load_map(&mut w, &ow_core::PackageName::parse("Sanctuary_P").unwrap())
            .map_err(|e| e.to_string())?;
        let mut r = String::new();
        let placed: usize = report.levels.iter().map(|l| l.placed_total).sum();
        let _ = writeln!(r, "# Sanctuary through the map route ({mode:?}; local, class paths and counts only)");
        let _ = writeln!(r, "levels {}, placed {placed}", report.levels.len());
        let _ = writeln!(r, "configured DefaultGame: {default_game:?}");
        let input = GameInfoInput { options: "", default_game: default_game.as_deref() };
        let up = match bring_up_map_for_play(&mut w, &input, EVERY) {
            Ok(up) => up,
            Err(e) => {
                let _ = writeln!(r, "map route refused: {e}");
                return Ok::<_, String>(r);
            }
        };
        let g = up.game_info.game;
        let _ = writeln!(r, "game info: {} ({:?})", up.game_info.class, up.game_info.branch);
        let info = report.world_info;
        let space = &w.vm.space;
        // What the test asserts (the hub's scope, review r1 of #530): the class chosen, the spawn, WorldInfo.Game, and
        // the typed stop at the auto state on the game info's InitGame and PreBeginPlay. What the script made past
        // them (the GRI, the tracker) is reported, not asserted: the auto state is its own lane.
        assert_eq!(up.game_info.class, "WillowGame.WillowCoopGameInfo", "the configured DefaultGame");
        assert_eq!(up.game_info.branch, GameInfoBranch::ConfiguredDefault);
        assert_eq!(ow_attr::eval::class_path(space, g), "WillowGame.WillowCoopGameInfo", "the spawned actor");
        assert_eq!(actors::field(space, info, "Game").map_err(|e| e.to_string())?, Value::Object(Some(g)));
        assert!(w.actors().borrow().list().contains(&g), "the game info is in the actor list");
        let on_game: Vec<(&str, bool, bool, bool)> = up
            .bring_up
            .failures
            .iter()
            .filter(|x| x.actor == g)
            .map(|x| (x.step, x.not_yet, x.text.contains("GotoState('Auto')"), is_partial_writes(&x.text)))
            .collect();
        assert_eq!(
            on_game,
            vec![("InitGame", true, true, true), ("PreBeginPlay", true, true, true)],
            "the typed auto-state stop on the game info's own path, marked (review r1 of #537)"
        );
        let gri = actors::field(space, info, "GRI").map_err(|e| e.to_string())?;
        let _ = writeln!(r, "WorldInfo.GRI: {}", describe(space, &gri));
        if let Value::Object(Some(gri)) = gri {
            let tracker = actors::field(space, gri, "MissionTracker").map(|v| describe(space, &v));
            let _ = writeln!(r, "GRI.MissionTracker: {}", tracker.unwrap_or_else(|e| e.to_string()));
        }
        let _ = writeln!(r, "routed: {:?}", up.bring_up.routed);
        let _ = writeln!(r, "failures: {}", up.bring_up.failures.len());
        for x in up.bring_up.failures.iter().filter(|x| x.actor == g) {
            let _ = writeln!(r, "  on the game info: {}: {}", x.step, x.text.chars().take(300).collect::<String>());
        }
        let mut by: BTreeMap<(String, String), (usize, String)> = BTreeMap::new();
        for x in &up.bring_up.failures {
            let e = by.entry((x.step.to_string(), x.text.clone())).or_insert((0, x.class.clone()));
            e.0 += 1;
        }
        let mut sorted: Vec<_> = by.into_iter().collect();
        sorted.sort_by_key(|x| std::cmp::Reverse(x.1.0));
        for ((step, text), (n, class)) in sorted.iter().take(12) {
            let _ = writeln!(r, "  {n} x {step} (first {class}): {}", text.chars().take(200).collect::<String>());
        }
        for p in &up.pending {
            let _ = writeln!(r, "pending (map route): {p}");
        }
        Ok(r)
    })
    .expect("the VM thread")
    .expect("the map route");
    let text = format!("{text}elapsed {} ms\n", started.elapsed().as_millis());
    std::fs::write(out.join("map_route_Off.md"), &text).unwrap();
    print!("{text}");
}

fn describe(space: &ow_object::heap::ObjectSpace, v: &Value) -> String {
    match v {
        Value::Object(Some(o)) => ow_attr::eval::class_path(space, *o),
        Value::Object(None) => "None".into(),
        other => other.kind_name().to_string(),
    }
}

