//! Owner: lane W5-MAPLOAD. Tests of the map load on invented packages (AGENTS.md rule 9).

use super::*;

use std::fmt::Write as _;
use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_balance::types::{
    AwesomeLevel, BalanceSnapshot, ExpLevel, GameStage, LevelCapUpgrades, OpLevel, PlayerCount,
    Playthrough,
};
use ow_core::ParityMode;
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::api::Vm;

use crate::world::BalanceState;

fn snapshot() -> BalanceSnapshot {
    BalanceSnapshot::new(
        Playthrough::NORMAL,
        OpLevel::NONE,
        PlayerCount::SINGLE,
        ExpLevel::new(8),
        GameStage(8),
        AwesomeLevel::new(0),
        LevelCapUpgrades::NONE,
    )
    .unwrap()
}

/// A world over `defs`, with no packs.
pub(super) fn world_over(defs: DefStore, mode: ParityMode) -> World {
    let vm = Vm::new(
        ObjectSpace::new(),
        defs,
        Rc::new(AttributeSystem::new_empty()),
        mode,
    );
    World::new(vm, BalanceState::new(snapshot()), &[]).unwrap()
}

/// Sanctuary on the installed game (`OW_BL2_ROOT`), loaded into a world in Off, then in Reference: placed and refused
/// actors by class, per level. Writes `local/p2/w5-mapload/report_<mode>.md`.
#[test]
#[ignore = "reads the installed game (OW_BL2_ROOT); writes local/p2/w5-mapload/"]
fn sanctuary_as_a_world_on_the_installed_game() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        sanctuary_in(mode);
    }
}

/// One mode of [`sanctuary_as_a_world_on_the_installed_game`].
fn sanctuary_in(mode: ParityMode) {
    let root = std::env::var_os("OW_BL2_ROOT").expect("set OW_BL2_ROOT to the install folder");
    let cooked = std::path::Path::new(&root).join("WillowGame/CookedPCConsole");
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../local/p2/w5-mapload");
    std::fs::create_dir_all(&out).unwrap();
    let started = std::time::Instant::now();
    let cooked = cooked.clone();
    let report = ow_vm::interp::on_vm_stack(move || {
        let defs =
            DefStore::open(cooked, IdentityPolicy::FirstInScanOrder, mode).expect("the store");
        let mut w = world_over(defs, mode);
        load_map(&mut w, &PackageName::parse("Sanctuary_P").unwrap())
            .map(|r| (r, w.actors().borrow().list().len()))
            .map_err(|e| e.to_string())
    });
    let (report, listed) = report.expect("the VM thread").expect("the map");
    let mut r = String::new();
    let _ = writeln!(r, "# Sanctuary as a world ({mode:?}; local, counts only)");
    let _ = writeln!(
        r,
        "load {} ms; world actor list {listed}",
        started.elapsed().as_millis()
    );
    for level in &report.levels {
        let refused: usize = level.refused.values().map(|x| x.count).sum();
        let _ = writeln!(
            r,
            "## {}: listed {} (nulls {}), placed {}, refused {refused}",
            level.name, level.listed, level.nulls, level.placed_total
        );
        for (class, n) in &level.placed {
            let _ = writeln!(r, "  placed {class}: {n}");
        }
        for (class, x) in &level.refused {
            let _ = writeln!(
                r,
                "  refused {class}: {} (not yet {}): {}",
                x.count, x.not_yet, x.first
            );
        }
    }
    let _ = writeln!(r, "missing levels: {:?}", report.missing_levels);
    for p in &report.pending {
        let _ = writeln!(r, "pending: {p}");
    }
    std::fs::write(out.join(format!("report_{mode:?}.md")), &r).unwrap();
    print!("{r}");
}

// ------------------------------------------------------------------------------------------ synthetic packages

use ow_package::testkit::reflect::Declarations;
use ow_package::testkit::{Export, PackageBuilder, ints};

pub(super) fn core() -> Vec<u8> {
    let mut d = Declarations::new(&["Int"]);
    let object = d.cls("Object", 0);
    d.default_object(object, "Object");
    let package = d.cls("Package", object);
    d.default_object(package, "Package");
    d.build()
}

/// `Engine.upk`: `Actor` (a `Health` to tell objects apart), `WorldInfo` with its `StreamingLevels`, `LevelStreaming`
/// with its `PackageName`, `World`, `Level`, and a non-actor `Note`.
///
/// For the bring-up (lane MAP-LOAD-NATIVES): `Actor.bStatic`, a `GameInfo` actor class, `WorldInfo.Game` and
/// `WorldInfo.bBegunPlay`.
pub(super) fn engine() -> Vec<u8> {
    let mut d = Declarations::new(&["Int", "Object", "Array", "Name", "Bool"]);
    let core = d.p.add_import("Core", "Package", 0, "Core");
    let object = d.p.add_import("Core", "Class", core, "Object");
    let actor = d.cls("Actor", object);
    d.prop("Int", actor, "Health", 0);
    d.prop("Bool", actor, "bStatic", 0);
    d.default_object(actor, "Actor");
    let game_info = d.cls("GameInfo", actor);
    d.default_object(game_info, "GameInfo");
    let world_info = d.cls("WorldInfo", actor);
    d.array_of(world_info, "StreamingLevels", "Object", object);
    d.prop("Object", world_info, "Game", game_info);
    d.prop("Bool", world_info, "bBegunPlay", 0);
    d.default_object(world_info, "WorldInfo");
    let streaming = d.cls("LevelStreaming", object);
    d.prop("Name", streaming, "PackageName", 0);
    d.default_object(streaming, "LevelStreaming");
    for name in ["World", "Level", "Note"] {
        let class = d.cls(name, object);
        d.default_object(class, name);
    }
    d.build()
}

/// The prefix bytes, the tags, then `None`.
fn payload(p: &mut PackageBuilder, prefix: usize, tags: &[Vec<u8>]) -> Vec<u8> {
    let mut out = vec![0u8; prefix];
    for t in tags {
        out.extend_from_slice(t);
    }
    out.extend_from_slice(&p.none());
    out
}

fn add(p: &mut PackageBuilder, class: i32, name: &str, outer: i32, payload: Vec<u8>) -> i32 {
    let mut export = Export::new(class, name, payload);
    export.outer = outer;
    p.add_export_with(export)
}

/// A level's tail: the list (owner, count, items), then the URL's protocol string.
fn level_tail(level: i32, items: &[i32]) -> Vec<u8> {
    let mut out = ints(&[level, i32::try_from(items.len()).unwrap()]);
    out.extend(ints(items));
    out.extend(ints(&[7]));
    out.extend_from_slice(b"unreal\0");
    out
}

/// A map file: `TheWorld` (1), `PersistentLevel` (2), its `WorldInfo` (3), then actors with `Health` 1, 2, ... (4
/// on), the level's list naming `listed` of them (export indices) after the WorldInfo and a null entry. With
/// `streaming`, the WorldInfo's `StreamingLevels` names that package through a `LevelStreaming` (exported last). With
/// `note_first`, actor 4 is a `Note`, which is not an actor class.
pub(super) fn map(actors: usize, listed: &[i32], streaming: Option<&str>, note_first: bool) -> Vec<u8> {
    map_with_fault(actors, listed, streaming, note_first, None)
}

#[derive(Clone, Copy)]
enum MapFault {
    MissingLevel,
    WrongLevelClass,
    ActorFirst,
}

fn map_with_fault(
    actors: usize,
    listed: &[i32],
    streaming: Option<&str>,
    note_first: bool,
    fault: Option<MapFault>,
) -> Vec<u8> {
    let mut p = PackageBuilder::new_empty();
    p.names(["None"]);
    let engine = p.add_import("Core", "Package", 0, "Engine");
    let class = |p: &mut PackageBuilder, name: &str| p.add_import("Core", "Class", engine, name);
    let (world, level_class, world_info, actor, streaming_class, note) = (
        class(&mut p, "World"),
        class(&mut p, "Level"),
        class(&mut p, "WorldInfo"),
        class(&mut p, "Actor"),
        class(&mut p, "LevelStreaming"),
        class(&mut p, "Note"),
    );
    let w = payload(&mut p, 4, &[]);
    assert_eq!(add(&mut p, world, "TheWorld", 0, w), 1);
    let mut items = vec![3, 0];
    items.extend_from_slice(listed);
    let mut level = payload(&mut p, 4, &[]);
    level.extend(level_tail(2, &items));
    let level_name = if matches!(fault, Some(MapFault::MissingLevel)) {
        "OtherLevel"
    } else {
        "PersistentLevel"
    };
    let level_class = if matches!(fault, Some(MapFault::WrongLevelClass)) {
        note
    } else {
        level_class
    };
    assert_eq!(add(&mut p, level_class, level_name, 1, level), 2);
    let stream_ref = 4 + i32::try_from(actors).unwrap();
    let wi_tags = match streaming {
        Some(_) => vec![p.array_tag("StreamingLevels", 1, &ints(&[stream_ref]))],
        None => Vec::new(),
    };
    let wi = payload(&mut p, 26, &wi_tags);
    let first_class = if matches!(fault, Some(MapFault::ActorFirst)) {
        actor
    } else {
        world_info
    };
    assert_eq!(add(&mut p, first_class, "WorldInfo_0", 2, wi), 3);
    for k in 0..actors {
        let health = i32::try_from(k).unwrap() + 1;
        let tags = [p.int_tag("Health", health)];
        let body = payload(&mut p, 26, &tags);
        let class = if k == 0 && note_first { note } else { actor };
        add(&mut p, class, &format!("Actor_{k}"), 2, body);
    }
    if let Some(name) = streaming {
        let tags = [p.name_tag("PackageName", name)];
        let body = payload(&mut p, 4, &tags);
        assert_eq!(
            add(&mut p, streaming_class, "LevelStreaming_0", 3, body),
            stream_ref
        );
    }
    p.build()
}

/// A world over a fresh store holding `files` in `mode`, run on the VM's thread.
pub(super) fn with_world<T: Send>(
    name: &str,
    files: Vec<(&'static str, Vec<u8>)>,
    mode: ParityMode,
    f: impl FnOnce(&mut World) -> T + Send,
) -> T {
    let dir = std::env::temp_dir().join(format!(
        "ow-world-mapload-{}-{name}-{mode:?}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let out = ow_vm::interp::on_vm_stack(|| {
        let refs: Vec<(&str, &[u8])> = files.iter().map(|(n, b)| (*n, b.as_slice())).collect();
        let defs =
            DefStore::from_synthetic(&dir, &refs, IdentityPolicy::FirstInScanOrder, mode).unwrap();
        let mut w = world_over(defs, mode);
        f(&mut w)
    })
    .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    out
}

fn health(w: &mut World, o: ObjRef) -> i32 {
    let class = w.vm.space.heap.get(o).unwrap().class;
    let p = w.vm.space.prop(class, "Health").unwrap();
    match w.vm.space.read(o, p.into()).unwrap() {
        Value::Int(h) => *h,
        other => panic!("Health is {other:?}"),
    }
}

#[test]
fn the_map_s_listed_actors_are_placed_in_list_order_with_the_world_info_first() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let files = vec![
            ("Core.upk", core()),
            ("Engine.upk", engine()),
            // Actors 4, 5, 6: the list names 6 then 4; 5 is under the level but unlisted.
            ("Test_P.upk", map(3, &[6, 4], Some("Test_Sub"), false)),
            // The streaming level: its WorldInfo (item 0) is not placed, its one actor is.
            ("Test_Sub.upk", map(1, &[4], None, false)),
        ];
        let (report, healths, first) = with_world("order", files, mode, |w| {
            let report = load_map(w, &PackageName::parse("Test_P").unwrap()).unwrap();
            let list = w.actors().borrow().list().to_vec();
            let first = list[0] == report.world_info
                && w.actors().borrow().world_info() == Some(report.world_info);
            let healths: Vec<i32> = list[1..].iter().map(|&o| health(w, o)).collect();
            (report, healths, first)
        });
        assert!(first, "{mode:?}: the WorldInfo is the list's first actor");
        // Test_P's 3 then 1, then Test_Sub's 1.
        assert_eq!(healths, [3, 1, 1], "{mode:?}");
        let names: Vec<&str> = report.levels.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, ["Test_P", "Test_Sub"]);
        assert_eq!(
            (
                report.levels[0].listed,
                report.levels[0].nulls,
                report.levels[0].placed_total
            ),
            (4, 1, 3)
        );
        assert_eq!(report.levels[1].placed_total, 1);
        assert_eq!(report.levels[0].placed.get("Engine.Actor"), Some(&2));
        assert!(report.missing_levels.is_empty());
        // The native steps the load does not run, in both modes (the C++ has no map load to stand in for).
        assert_eq!(report.pending.len(), 3, "{mode:?}");
        assert!(report.pending.iter().all(|n| n.record == RECORD));
    }
}

#[test]
fn a_listed_object_that_is_not_an_actor_is_refused_and_a_missing_level_is_reported() {
    let files = vec![
        ("Core.upk", core()),
        ("Engine.upk", engine()),
        // Actor 4 is a Note (not an actor); the WorldInfo names a level no file holds.
        ("Test_P.upk", map(2, &[4, 5], Some("Nowhere"), true)),
    ];
    let (report, again) = with_world("refused", files, ParityMode::Off, |w| {
        let map = PackageName::parse("Test_P").unwrap();
        let report = load_map(w, &map).unwrap();
        (
            report,
            load_map(w, &map).map(|_| ()).map_err(|e| e.to_string()),
        )
    });
    let level = &report.levels[0];
    assert_eq!(level.placed_total, 2, "the WorldInfo and actor 5");
    let note = level
        .refused
        .get("Engine.Note")
        .expect("the Note is refused");
    assert_eq!(
        (note.count, note.first.as_str(), note.not_yet),
        (1, "not an actor class", false)
    );
    assert_eq!(report.missing_levels, ["Nowhere"]);
    // A second load into the same world is refused: one map per fresh world.
    assert!(again.unwrap_err().contains("already has actors"));
}

#[test]
fn streaming_file_errors_are_not_missing_levels() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        for case in [
            "absent",
            "corrupt",
            "ambiguous",
            "invalid-name",
            "missing-export",
            "wrong-class",
        ] {
            let stream = if case == "invalid-name" {
                "Bad/Name"
            } else {
                "Test_Sub"
            };
            let mut files = vec![
                ("Core.upk", core()),
                ("Engine.upk", engine()),
                ("Test_P.upk", map(0, &[], Some(stream), false)),
            ];
            match case {
                "corrupt" => files.push(("Test_Sub.upk", vec![0; 32])),
                "ambiguous" => {
                    files.push(("a/Test_Sub.upk", map(0, &[], None, false)));
                    files.push(("b/Test_Sub.upk", map(0, &[], None, false)));
                }
                "missing-export" | "wrong-class" => {
                    let fault = if case == "missing-export" {
                        MapFault::MissingLevel
                    } else {
                        MapFault::WrongLevelClass
                    };
                    files.push((
                        "Test_Sub.upk",
                        map_with_fault(0, &[], None, false, Some(fault)),
                    ));
                }
                _ => {}
            }
            let result = with_world(case, files, mode, |w| {
                load_map(w, &PackageName::parse("Test_P").unwrap())
            });
            match case {
                "absent" => assert_eq!(result.unwrap().missing_levels, ["Test_Sub"]),
                "corrupt" => assert!(
                    matches!(
                        result,
                        Err(MapError::Store {
                            cause: StoreError::Package(_),
                            ..
                        })
                    ),
                    "{mode:?}: {result:?}"
                ),
                "ambiguous" => assert!(
                    matches!(
                        result,
                        Err(MapError::Store {
                            cause: StoreError::AmbiguousName(_),
                            ..
                        })
                    ),
                    "{mode:?}: {result:?}"
                ),
                "invalid-name" => assert!(
                    matches!(
                        result,
                        Err(MapError::InvalidName {
                            cause: IdentError::Separator { .. },
                            ..
                        })
                    ),
                    "{mode:?}: {result:?}"
                ),
                "missing-export" => assert!(
                    matches!(result, Err(MapError::MissingLevel(_))),
                    "{mode:?}: {result:?}"
                ),
                "wrong-class" => assert!(
                    matches!(result, Err(MapError::WrongLevelClass { .. })),
                    "{mode:?}: {result:?}"
                ),
                _ => unreachable!(),
            }
        }
    }
}

#[test]
fn every_levels_first_entry_must_be_a_world_info() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        for streaming in [false, true] {
            let bad = map_with_fault(1, &[4], None, false, Some(MapFault::ActorFirst));
            let files = if streaming {
                vec![
                    ("Core.upk", core()),
                    ("Engine.upk", engine()),
                    ("Test_P.upk", map(0, &[], Some("Test_Sub"), false)),
                    ("Test_Sub.upk", bad),
                ]
            } else {
                vec![
                    ("Core.upk", core()),
                    ("Engine.upk", engine()),
                    ("Test_P.upk", bad),
                ]
            };
            with_world(
                if streaming {
                    "stream-first"
                } else {
                    "persistent-first"
                },
                files,
                mode,
                |w| {
                    let result = load_map(w, &PackageName::parse("Test_P").unwrap());
                    assert!(
                        matches!(result, Err(MapError::WorldInfo(_))),
                        "{mode:?}: {result:?}"
                    );
                    // No actor from the bad level is placed; the persistent WorldInfo stays alone.
                    assert_eq!(w.actors().borrow().list().len(), usize::from(streaming));
                },
            );
        }
    }
}

#[test]
fn a_level_tail_off_the_recovered_layout_is_refused() {
    let level = |tail: Vec<u8>| -> Result<LevelList, String> {
        let mut p = PackageBuilder::new_empty();
        p.names(["None"]);
        let engine = p.add_import("Core", "Package", 0, "Engine");
        let level_class = p.add_import("Core", "Class", engine, "Level");
        let actor = p.add_import("Core", "Class", engine, "Actor");
        let mut body = payload(&mut p, 4, &[]);
        body.extend(tail);
        assert_eq!(add(&mut p, level_class, "PersistentLevel", 0, body), 1);
        let a = payload(&mut p, 26, &[]);
        assert_eq!(add(&mut p, actor, "Actor_0", 1, a), 2);
        let b = payload(&mut p, 26, &[]);
        assert_eq!(add(&mut p, actor, "Other", 0, b), 3);
        let pkg = ow_package::package::Package::from_decoded(
            Default::default(),
            "Test".into(),
            p.build(),
        )
        .unwrap();
        read_level_list(&pkg, 1)
    };
    assert_eq!(level(level_tail(1, &[2, 0])).unwrap().actors, [2, 0]);
    // The owner is not the level; an item outside the level; item 0 null; a count past the tail; no URL after.
    assert!(level(level_tail(2, &[2])).unwrap_err().contains("owner"));
    assert!(
        level(level_tail(1, &[2, 3]))
            .unwrap_err()
            .contains("not an export of the level")
    );
    assert!(level(level_tail(1, &[0, 2])).unwrap_err().contains("null"));
    let mut long = ints(&[1, 9, 2]);
    long.extend(ints(&[7]));
    assert!(level(long).unwrap_err().contains("count"));
    assert!(level(ints(&[1, 1, 2, 0, 0])).unwrap_err().contains("URL"));
}
