//! Owner: lane IMPL-SLICE-MAP-WORLD. `MissionScript::adopt`, the bridge on a map world, on the invented packages of
//! the parent module (AGENTS.md rule 9). AI-assisted (Claude Code).
//!
//! The world is built **the map route's way by hand**: the globals natives' owners bound first, the world's globals
//! object, a placed `WorldInfo` whose `GRI` holds a `MissionTracker`, and one local player controller with a PRI.
//! The play actor's possession (the game info's `RestartPlayer`, which ow-world's tests drive through a toy `Login`)
//! stands here as a pawn written into the controller's `Pawn`: a test input, not the game's possession.

use super::*;
use ow_world::world::mapload::game_info::WriteMark;

/// The objects of a hand-built map world.
struct MapWorld {
    world: World,
    mission: MissionSystem,
    dir: PathBuf,
    world_info: ObjRef,
    gri: ObjRef,
    tracker: ObjRef,
    controller: ObjRef,
    pri: ObjRef,
    globals: ObjRef,
}

impl Drop for MapWorld {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// What to leave out of the hand-built world.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Without {
    Nothing,
    Gri,
    Tracker,
    LocalPlayer,
    Pri,
    Globals,
    Owners,
}

fn map_world(mode: ParityMode, without: Without) -> MapWorld {
    let dir = std::env::temp_dir().join(format!(
        "ow-mission-adopt-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let files: Vec<(String, Vec<u8>)> = vec![
        ("Core.upk".into(), core()),
        ("Engine.upk".into(), engine()),
        ("GearboxFramework.upk".into(), gearbox()),
        ("WillowGame.upk".into(), willowgame()),
        ("TestMission.upk".into(), mission_package(false, 0.0, false, false)),
        ("TestDirector.upk".into(), director_package()),
    ];
    let borrowed: Vec<(&str, &[u8])> = files.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect();
    let defs = DefStore::from_synthetic(&dir, &borrowed, IdentityPolicy::FirstInScanOrder, mode).expect("store");
    let vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::standard().unwrap()), mode);
    let mut world = world(vm).expect("world");
    ow_vm::natives_core::register(&mut world.vm).expect("core natives");
    if without != Without::Owners {
        ow_inventory::globals::register(&mut world.vm).expect("the inventory globals owner");
        ow_behavior::kernel::natives::register_kernel_getter(&mut world.vm).expect("the kernel getter owner");
    }
    let vm = &mut world.vm;
    let globals = new_object(vm, "WillowGame.WillowGlobals").unwrap();
    let world_info = new_object(vm, "Engine.WorldInfo").unwrap();
    let gri = new_object(vm, "WillowGame.WillowGameReplicationInfo").unwrap();
    let tracker = new_object(vm, "WillowGame.MissionTracker").unwrap();
    let controller = new_object(vm, "WillowGame.WillowPlayerController").unwrap();
    let pri = new_object(vm, "WillowGame.WillowPlayerReplicationInfo").unwrap();
    if without != Without::Gri {
        set(&mut vm.space, world_info, "GRI", Value::Object(Some(gri))).unwrap();
    }
    if without != Without::Tracker {
        set(&mut vm.space, gri, "MissionTracker", Value::Object(Some(tracker))).unwrap();
    }
    if without != Without::Pri {
        set(&mut vm.space, controller, "PlayerReplicationInfo", Value::Object(Some(pri))).unwrap();
    }
    set(&mut vm.space, controller, "WorldInfo", Value::Object(Some(world_info))).unwrap();
    set(&mut vm.space, controller, "Role", Value::Byte(3)).unwrap();
    world.actors().borrow_mut().place_world_info(world_info).unwrap();
    if without != Without::LocalPlayer {
        world.actors().borrow_mut().add_local_player(controller).unwrap();
    }
    if without != Without::Globals {
        world.query().borrow_mut().set_globals(globals);
    }
    let name = PackageName::parse("TestMission").unwrap();
    let mission = MissionSystem::new(&mut world.vm, &name, "ToyMission").expect("mission");
    MapWorld { world, mission, dir, world_info, gri, tracker, controller, pri, globals }
}

/// The test input standing for the game info's possession: a pawn in the controller's `Pawn`.
fn possess(m: &mut MapWorld) -> ObjRef {
    let vm = &mut m.world.vm;
    let pawn = new_object(vm, "WillowGame.WillowPlayerPawn").unwrap();
    set(&mut vm.space, m.controller, "Pawn", Value::Object(Some(pawn))).unwrap();
    set(&mut vm.space, pawn, "Controller", Value::Object(Some(m.controller))).unwrap();
    pawn
}

fn not_yet(e: MissionError) -> NotYet {
    match e {
        BehaviorError::Vm(VmError::NotYet(n)) => n,
        other => panic!("expected a NotYet, got {other:?}"),
    }
}

/// Adopt makes none of the graph's objects: they are the world's, the validation reply ran on the world's tracker,
/// and the player's pawn is the controller's possessed `Pawn`, the same object on every call, with no pawn made.
#[test]
fn adopt_uses_the_world_s_objects_and_the_possessed_pawn() {
    let mut m = map_world(ParityMode::Off, Without::Nothing);
    let pawn = possess(&mut m);
    let script = MissionScript::adopt(&mut m.world, &m.mission).expect("adopt");
    let o = script.inner.o;
    assert_eq!(
        (o.world_info, o.replication, o.tracker, o.controller, o.pri, o.globals),
        (m.world_info, m.gri, m.tracker, m.controller, m.pri, m.globals)
    );
    assert!(script.inner.adopted);
    let space = &m.world.vm.space;
    assert_eq!(get(space, m.tracker, "bDataValidated").unwrap(), Value::Bool(true), "the validation reply ran");
    assert_eq!(get(space, m.controller, "MissionPlaythroughs").unwrap().elements().map(<[Value]>::len), Some(1));
    assert!(script.errors().is_empty(), "{:?}", script.errors());
    let objects = m.world.vm.space.heap.len();
    assert_eq!(script.player_pawn(&mut m.world.vm).unwrap(), pawn);
    assert_eq!(script.player_pawn(&mut m.world.vm).unwrap(), pawn);
    assert_eq!(m.world.vm.space.heap.len(), objects, "the bridge made no pawn");
    let harness = m.world.vm.log.iter().filter(|l| l.starts_with("harness input: the mission bridge's playthrough")).count();
    assert_eq!(harness, 1, "the playthrough record is a labelled harness input");
}

/// A controller with no possessed pawn: the accessor refuses, typed, and makes nothing.
#[test]
fn without_a_possessed_pawn_the_accessor_refuses() {
    let mut m = map_world(ParityMode::Off, Without::Nothing);
    let script = MissionScript::adopt(&mut m.world, &m.mission).expect("adopt");
    let objects = m.world.vm.space.heap.len();
    let n = not_yet(script.player_pawn(&mut m.world.vm).unwrap_err());
    assert_eq!(n.missing, ow_core::Missing::NotRead);
    assert!(n.what.contains("possessed Pawn"), "{}", n.what);
    assert_eq!(m.world.vm.space.heap.len(), objects, "no pawn from class defaults");
}

/// The one-owner rule for the four globals natives on the map route: each is bound once, by its owner, before the
/// bridge adopts, and the bridge binds none of them (a second binding of a key would be refused, so a successful
/// adopt over the owners' bindings shows it). The owners answer from the world's globals object.
#[test]
fn the_globals_natives_have_one_owner_on_the_map_route() {
    let mut m = map_world(ParityMode::Off, Without::Nothing);
    let _script = MissionScript::adopt(&mut m.world, &m.mission).expect("adopt over the owners' bindings");
    let vm = &mut m.world.vm;
    for path in GLOBALS_NATIVES {
        let f = native_at(vm, path).unwrap();
        assert!(vm.is_bound(f), "{path} is bound");
    }
    let f = vm.find_function("WillowGame.WillowGlobals.GetWillowGlobals").unwrap();
    assert_eq!(vm.call(f, None, Vec::new()).unwrap().ret, Value::Object(Some(m.globals)), "the inventory owner's answer");
    let f = vm.find_function("GearboxFramework.GearboxGlobals.GetBehaviorKernel").unwrap();
    assert_eq!(vm.call(f, Some(m.globals), Vec::new()).unwrap().ret, Value::Object(None), "the kernel is not modelled");
}

/// Every refusal before the first write: typed, unmarked, and nothing written (#518's counters unchanged).
#[test]
fn the_preflight_refuses_before_any_write() {
    let cases = [
        (Without::Gri, "WorldInfo.GRI"),
        (Without::Tracker, "GRI.MissionTracker"),
        (Without::LocalPlayer, "local player controller"),
        (Without::Pri, "PlayerReplicationInfo"),
        (Without::Globals, "globals object"),
        (Without::Owners, "GetWillowGlobals on the map route"),
    ];
    for (without, text) in cases {
        let mut m = map_world(ParityMode::Off, without);
        let mark = WriteMark::now(&m.world.vm);
        let n = not_yet(MissionScript::adopt(&mut m.world, &m.mission).unwrap_err());
        assert!(n.what.contains(text), "{text}: {}", n.what);
        assert!(!ow_core::notyet::is_partial_writes(&n.what), "{text}: unmarked");
        assert!(!mark.moved(&m.world.vm), "{text}: nothing written");
    }
}

/// Reference is unchanged: the C++ has no map load, so adopt is not ported there.
#[test]
fn reference_has_no_adopt() {
    let mut m = map_world(ParityMode::Reference, Without::Nothing);
    let n = not_yet(MissionScript::adopt(&mut m.world, &m.mission).unwrap_err());
    assert_eq!(n.missing, ow_core::Missing::NotPorted);
}

// ---- Review r1 of #556 (lane MAP-ROUTE-FOLLOWUP) ----------------------------------------------------------------

/// Item 1: on the map route the globals definition is the adopted globals' own `TheGlobalsDefinition`, not the
/// package's `GD_Globals` export; unset, it is a typed refusal.
#[test]
fn the_adopted_globals_give_their_own_definition() {
    let mut m = map_world(ParityMode::Off, Without::Nothing);
    let script = MissionScript::adopt(&mut m.world, &m.mission).expect("adopt");
    let n = not_yet(script.player_interaction_distance(&mut m.world.vm).unwrap_err());
    assert_eq!((&*n.what, n.missing), (GLOBALS_UNSET, ow_core::Missing::NotRead));
    let vm = &mut m.world.vm;
    let definition = new_object(vm, "WillowGame.GlobalsDefinition").unwrap();
    set(&mut vm.space, definition, "PlayerInteractionDistance", Value::Float(9.0)).unwrap();
    set(&mut vm.space, m.globals, "TheGlobalsDefinition", Value::Object(Some(definition))).unwrap();
    assert_eq!(script.player_interaction_distance(&mut m.world.vm).unwrap(), 9.0, "the globals' own, not the package's 420");
    // Review r1 of #571, item 2: read on every call. An unloaded definition handle put there later is a typed refusal,
    // with nothing loaded (the handle: an export reference the director's archetype holds, not loaded yet).
    let director = ObjectPath::parse(MARCUS_PAWN).unwrap();
    let vm = &mut m.world.vm;
    let pawn = vm.defs.find(&mut vm.space, &director).unwrap().expect("the director archetype");
    let Value::Object(Some(handle)) = get(&vm.space, pawn, "MissionDirectives").unwrap() else { panic!("no reference") };
    assert!(vm.defs.is_unloaded(&vm.space, handle), "a handle the store has not loaded");
    set(&mut vm.space, m.globals, "TheGlobalsDefinition", Value::Object(Some(handle))).unwrap();
    let loads = vm.defs.load_transitions();
    let n = not_yet(script.player_interaction_distance(vm).unwrap_err());
    assert_eq!(&*n.what, GLOBALS_UNLOADED);
    assert_eq!(vm.defs.load_transitions(), loads, "nothing loaded");
    assert!(vm.defs.is_unloaded(&vm.space, handle));
    // And cleared again: the unset refusal, not the earlier definition.
    set(&mut vm.space, m.globals, "TheGlobalsDefinition", Value::Object(None)).unwrap();
    assert_eq!(&*not_yet(script.player_interaction_distance(vm).unwrap_err()).what, GLOBALS_UNSET);
}

/// Item 2: a `NotYet` after adopt's first write carries the mark, once, its kind kept.
#[test]
fn a_refusal_after_adopt_s_first_write_is_marked() {
    let mut m = map_world(ParityMode::Off, Without::Nothing);
    adopt_fault::set(true);
    let result = MissionScript::adopt(&mut m.world, &m.mission);
    adopt_fault::set(false);
    let Err(e) = result else { panic!("expected the injected refusal") };
    let n = not_yet(e);
    assert!(ow_core::notyet::is_partial_writes(&n.what), "{}", n.what);
    assert_eq!(n.what.matches("partial writes").count(), 1);
    assert_eq!(n.missing, ow_core::Missing::NotRead);
}

/// Item 3: `SetGameStage` on the map route records the stage for the controller's possessed pawn, read at the call.
#[test]
fn set_game_stage_compares_against_the_possessed_pawn() {
    let mut m = map_world(ParityMode::Off, Without::Nothing);
    let pawn = possess(&mut m);
    let script = MissionScript::adopt(&mut m.world, &m.mission).expect("adopt");
    let other = new_object(&mut m.world.vm, "WillowGame.WillowPlayerPawn").unwrap();
    let path = "WillowGame.WillowPawn.SetGameStage";
    let vm = &mut m.world.vm;
    let f = match vm.find_function(path) {
        Ok(f) => f,
        Err(_) => vm.declare_function(FunctionDecl::native(ObjectPath::parse(path).unwrap(), path)).unwrap(),
    };
    let call = |vm: &mut Vm, this: ObjRef, stage: i32| {
        let mut c = NativeCall::new(vm, f, Some(this), vec![Arg::new(Value::Int(stage), false, true)]);
        script.n_set_game_stage(&mut c).unwrap();
    };
    call(vm, other, 4);
    assert_eq!(script.inner.state.borrow().pawn_game_stage, 0, "another pawn's stage is not the player's");
    call(vm, pawn, 7);
    assert_eq!(script.inner.state.borrow().pawn_game_stage, 7, "the possessed pawn's stage");
}

/// Item 4: on the map route the bridge makes no director (a typed refusal, write-free), and takes the map's own placed
/// waypoint when the map load placed it (else a typed refusal); it makes no object either way.
#[test]
fn the_map_route_makes_no_director_and_takes_the_placed_waypoint() {
    let mut m = map_world(ParityMode::Off, Without::Nothing);
    let script = MissionScript::adopt(&mut m.world, &m.mission).expect("adopt");
    let director = PackageName::parse("TestDirector").unwrap();
    let mark = WriteMark::now(&m.world.vm);
    let n = not_yet(script.place_marcus(&mut m.world.vm, &director, MARCUS_PAWN).unwrap_err());
    assert_eq!(&*n.what, director_on_map(MARCUS_PAWN));
    assert!(!mark.moved(&m.world.vm), "write-free");
    // A placed actor of the map package stands for the waypoint (any export: the map route does not make it).
    const PLACED: &str = "GD_Marcus.Character.Directives_Toy";
    let objects = m.world.vm.space.heap.len();
    let loads = m.world.vm.defs.load_transitions();
    // Review r1 of #571, item 1: nothing is loaded first. An uncached package, an unknown one and (below) a missing
    // export are each the typed refusal, not a store error or `false`.
    assert!(!m.world.vm.defs.packages().is_loaded(&director), "TestDirector starts uncached");
    let n = not_yet(script.place_waypoint(&mut m.world.vm, &director, PLACED).unwrap_err());
    assert_eq!(&*n.what, waypoint_on_map(PLACED));
    assert!(!m.world.vm.defs.packages().is_loaded(&director), "still uncached: the lookup loaded no package (review r2 of #571)");
    let unknown = PackageName::parse("NoSuchMap").unwrap();
    let n = not_yet(script.place_waypoint(&mut m.world.vm, &unknown, PLACED).unwrap_err());
    assert_eq!(&*n.what, waypoint_on_map(PLACED));
    assert_eq!(m.world.vm.space.heap.len(), objects, "nothing made");
    assert_eq!(m.world.vm.defs.load_transitions(), loads, "nothing loaded");
    let vm = &mut m.world.vm;
    let (_, store) = vm.defs.code_mut();
    let id = store.load(&director).unwrap();
    let index = store.get(id).find_export(PLACED).unwrap().unwrap();
    let placed = vm.defs.load_export(&mut vm.space, id, index, PropertyPrefix::Ordinary).unwrap();
    m.world.actors().borrow_mut().place(placed).unwrap();
    const MISSING: &str = "GD_Marcus.Character.NoSuchWaypoint";
    let n = not_yet(script.place_waypoint(&mut m.world.vm, &director, MISSING).unwrap_err());
    assert_eq!(&*n.what, waypoint_on_map(MISSING), "a missing export is the refusal, not false");
    let objects = m.world.vm.space.heap.len();
    assert!(script.place_waypoint(&mut m.world.vm, &director, PLACED).unwrap());
    assert!(script.has_waypoint());
    assert_eq!(script.inner.state.borrow().waypoint, Some(placed), "the map's own actor");
    assert_eq!(m.world.vm.space.heap.len(), objects, "nothing made");
}
