//! Owner: lane MAP-LOAD-NATIVES. Tests of the bring-up on invented packages (AGENTS.md rule 9), in both parity modes.

use super::*;

use std::fmt::Write as _;

use ow_core::{Missing, PackageName, ParityMode};
use ow_object::ids::PropAddr;

use super::super::load_map;
use super::super::tests::{core, engine, map, with_world};

fn files() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("Core.upk", core()),
        ("Engine.upk", engine()),
        ("Test_P.upk", map(3, &[4, 5, 6], None, false)),
    ]
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

#[test]
fn the_bring_up_counts_the_routing_reach_and_runs_no_unread_step() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let (up, list_before, list_after, begun) = with_world("reach", files(), mode, |w| {
            load_map(w, &PackageName::parse("Test_P").unwrap()).unwrap();
            let list = w.actors().borrow().list().to_vec();
            // Actor 2 static, actor 3 pending deletion; actor 1 and the WorldInfo neither.
            write(w, list[2], "bStatic", Value::Bool(true));
            w.vm.space.heap.get_mut(list[3]).unwrap().destroyed = true;
            let up = bring_up_for_play(w).unwrap();
            let after = w.actors().borrow().list().to_vec();
            (up, list, after, w.actors().borrow().begun_play())
        });
        assert_eq!(
            up.reach,
            BeginPlayReach {
                actors: 4,
                pending_deletion: 1,
                not_static: 2,
                static_open: 1,
                world_info_begun_play: Some(false),
            },
            "{mode:?}"
        );
        assert_eq!(up.game, None, "{mode:?}");
        // Nothing changed: no actor added or removed, play not begun.
        assert_eq!(list_before, list_after, "{mode:?}");
        assert!(!begun, "{mode:?}");
        // The time reset, URL and setup calls; the game info's class choice; begin play; the per-level call; the local
        // player's play actor. All named for the note, all NotRead.
        assert_eq!(up.pending.len(), 5, "{mode:?}");
        assert!(
            up.pending
                .iter()
                .all(|n| n.record == RECORD && n.missing == Missing::NotRead),
            "{mode:?}"
        );
        assert!(up.pending[1].what.contains("class choice"), "{mode:?}");
        assert!(up.pending[2].what.contains("begin-play pass"), "{mode:?}");
    }
}

#[test]
fn a_game_info_the_world_info_already_holds_is_kept() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let (game, up, held) = with_world("kept", files(), mode, |w| {
            let report = load_map(w, &PackageName::parse("Test_P").unwrap()).unwrap();
            let class = w.vm.find_class("Engine.GameInfo").unwrap();
            let game = ow_vm::interp::instantiate_from_defaults(&mut w.vm, class).unwrap();
            write(w, report.world_info, "Game", Value::Object(Some(game)));
            let up = bring_up_for_play(w).unwrap();
            let held = match actors::field(&w.vm.space, report.world_info, "Game").unwrap() {
                Value::Object(r) => r,
                other => panic!("Game holds {other:?}"),
            };
            (game, up, held)
        });
        assert_eq!(up.game, Some(game), "{mode:?}");
        assert_eq!(held, Some(game), "{mode:?}: the field is unchanged");
        assert_eq!(up.pending.len(), 4, "{mode:?}: no class choice pending");
        assert!(
            up.pending.iter().all(|n| !n.what.contains("class choice")),
            "{mode:?}"
        );
    }
}

#[test]
fn the_game_info_without_a_world_info_waits_for_the_map_load() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let missing = with_world("empty", files(), mode, |w| {
            game_info(w).map(|_| ()).map_err(|e| not_yet(e).missing)
        });
        assert_eq!(
            missing,
            Err(Missing::LayerAbsent("the map load (mapload::load_map)")),
            "{mode:?}"
        );
    }
}

/// Sanctuary on the installed game (`OW_BL2_ROOT`) in Off: the map load, then the bring-up's reach, game info and
/// pending steps, and the persistent `WorldInfo`'s play and time fields. Writes
/// `local/p2/map-load-natives/bringup_Off.md` (counts only).
#[test]
#[ignore = "reads the installed game (OW_BL2_ROOT); writes local/p2/map-load-natives/"]
fn sanctuary_brought_up_on_the_installed_game() {
    let root = std::env::var_os("OW_BL2_ROOT").expect("set OW_BL2_ROOT to the install folder");
    let cooked = std::path::Path::new(&root).join("WillowGame/CookedPCConsole");
    let out =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../local/p2/map-load-natives");
    std::fs::create_dir_all(&out).unwrap();
    let mode = ParityMode::Off;
    let started = std::time::Instant::now();
    let text = ow_vm::interp::on_vm_stack(move || {
        let defs = ow_defs::DefStore::open(
            cooked,
            ow_defs::identity::IdentityPolicy::FirstInScanOrder,
            mode,
        )
        .expect("the store");
        let mut w = super::super::tests::world_over(defs, mode);
        let report = load_map(&mut w, &PackageName::parse("Sanctuary_P").unwrap())
            .map_err(|e| e.to_string())?;
        let up = bring_up_for_play(&w).map_err(|e| e.to_string())?;
        let mut r = String::new();
        let placed: usize = report.levels.iter().map(|l| l.placed_total).sum();
        let refused: usize = report
            .levels
            .iter()
            .flat_map(|l| l.refused.values())
            .map(|x| x.count)
            .sum();
        let _ = writeln!(r, "# Sanctuary brought up ({mode:?}; local, counts only)");
        let _ = writeln!(
            r,
            "levels {}, placed {placed}, refused {refused}",
            report.levels.len()
        );
        let _ = writeln!(r, "reach: {:?}", up.reach);
        let _ = writeln!(r, "game info: {:?}", up.game);
        let info = report.world_info;
        for name in [
            "Game",
            "bBegunPlay",
            "bStartup",
            "TimeSeconds",
            "RealTimeSeconds",
            "AudioTimeSeconds",
            "NetMode",
        ] {
            let v = actors::field(&w.vm.space, info, name).map(|v| format!("{v:?}"));
            let _ = writeln!(r, "WorldInfo.{name}: {}", v.unwrap_or_else(|e| e.to_string()));
        }
        let _ = writeln!(
            r,
            "Actors: begun play {}, local players {}",
            w.actors().borrow().begun_play(),
            w.actors().borrow().local_players().len()
        );
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
