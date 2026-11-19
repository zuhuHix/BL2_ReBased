//! Owner: lane H-WORLD. The world thread on invented packages (AGENTS.md rule 9): an `Engine.upk` with `Actor`,
//! `WorldInfo` and `Pawn`, and a map whose level lists a `WorldInfo`, an actor and a pawn.

use std::sync::atomic::{AtomicU32, Ordering};

use ow_object::value::StructValue;
use ow_package::testkit::script::ScriptToy;
use ow_package::testkit::reflect::{toy_array_of, toy_struct};
use ow_package::testkit::{Export, PackageBuilder, ints};

use super::*;

static NEXT: AtomicU32 = AtomicU32::new(0);

/// `Engine.upk`: `Object`, `Vector`, `Rotator`, `Actor` with what the world's tick reads, `WorldInfo` with the time
/// fields, `Pawn`, `World` and `Level`.
fn engine() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&[
        "Class",
        "Function",
        "State",
        "ScriptStruct",
        "Enum",
        "IntProperty",
        "FloatProperty",
        "BoolProperty",
        "NameProperty",
        "ByteProperty",
        "ObjectProperty",
        "ArrayProperty",
        "StructProperty",
    ]);
    let object = t.class("Object", 0);
    let vector = toy_struct(&mut t, object, "Vector", None);
    for f in ["X", "Y", "Z"] {
        t.prop("Float", vector, f, 0, 0);
    }
    let rotator = toy_struct(&mut t, object, "Rotator", None);
    for f in ["Pitch", "Yaw", "Roll"] {
        t.prop("Int", rotator, f, 0, 0);
    }
    let actor = t.class("Actor", object);
    let mut payload = ints(&[0]);
    payload.extend_from_slice(&t.none);
    let values = ["PHYS_None", "PHYS_Walking", "PHYS_Falling", "PHYS_Rotating", "PHYS_Interpolating"];
    payload.extend(ints(&[0, values.len() as i32]));
    for v in values {
        let bytes = t.p.fname(v, 0);
        payload.extend_from_slice(&bytes);
    }
    let mut e = Export::new(t.import("Enum"), "EPhysics", payload);
    e.outer = actor;
    let physics = t.p.add_export_with(e);
    for b in ["bDeleteMe", "bPendingDelete"] {
        t.prop("Bool", actor, b, 0, 0);
    }
    for b in ["Role", "RemoteRole", "TickGroup"] {
        t.prop("Byte", actor, b, 0, 0);
    }
    t.prop("Byte", actor, "Physics", 0, physics);
    t.prop("Float", actor, "LifeSpan", 0, 0);
    t.prop("Object", actor, "Base", 0, actor);
    t.prop("Struct", actor, "Location", 0, vector);
    t.prop("Struct", actor, "Rotation", 0, rotator);
    let world_info = t.class("WorldInfo", actor);
    for f in ["TimeSeconds", "RealTimeSeconds", "TimeDilation", "DeltaSeconds"] {
        t.prop("Float", world_info, f, 0, 0);
    }
    toy_array_of(&mut t, world_info, "StreamingLevels", "Object", object);
    // The Kismet classes the Matinee scan reads (lane H-PHYSICS-MODES review r1): an op, its link struct, the plain
    // object variable, a subclass of it, a named variable and a scalar one.
    let link = toy_struct(&mut t, object, "SeqVarLink", None);
    toy_array_of(&mut t, link, "LinkedVariables", "Object", object);
    let interp = t.class("SeqAct_Interp", object);
    toy_array_of(&mut t, interp, "VariableLinks", "Struct", link);
    let var = t.class("SeqVar_Object", object);
    t.prop("Object", var, "ObjValue", 0, object);
    t.class("SeqVar_ObjectList", var);
    let named = t.class("SeqVar_Named", object);
    t.prop("Name", named, "FindVarName", 0, 0);
    t.class("SeqVar_Float", object);
    let streaming = t.class("LevelStreaming", object);
    t.prop("Name", streaming, "PackageName", 0, 0);
    t.class("Pawn", actor);
    t.class("World", object);
    t.class("Level", object);
    t.p.build()
}

/// A map: `TheWorld` (1), `PersistentLevel` (2), `WorldInfo_0` (3, time dilation 1), `Actor_0` (4), `Pawn_0` (5);
/// the level lists 3, a null, 4 and 5. The pawn's `RemoteRole` is `remote_role`, and its `Physics` is `physics` when
/// given. With `streaming`, the `WorldInfo` names that level through a `LevelStreaming` (6).
fn map(remote_role: u8, physics: Option<&str>, streaming: Option<&str>, kismet: bool) -> Vec<u8> {
    let mut p = PackageBuilder::new_empty();
    p.names(["None"]);
    let engine = p.add_import("Core", "Package", 0, "Engine");
    let class = |p: &mut PackageBuilder, name: &str| p.add_import("Core", "Class", engine, name);
    let (world, level, world_info, actor, pawn, level_streaming) = (
        class(&mut p, "World"),
        class(&mut p, "Level"),
        class(&mut p, "WorldInfo"),
        class(&mut p, "Actor"),
        class(&mut p, "Pawn"),
        class(&mut p, "LevelStreaming"),
    );
    let add = |p: &mut PackageBuilder, class: i32, name: &str, outer: i32, prefix: usize, tags: Vec<Vec<u8>>, tail: Vec<u8>| {
        let mut body = vec![0u8; prefix];
        for t in tags {
            body.extend_from_slice(&t);
        }
        body.extend_from_slice(&p.none());
        body.extend(tail);
        let mut export = Export::new(class, name, body);
        export.outer = outer;
        p.add_export_with(export)
    };
    assert_eq!(add(&mut p, world, "TheWorld", 0, 4, Vec::new(), Vec::new()), 1);
    let mut tail = ints(&[2, 4, 3, 0, 4, 5]);
    tail.extend(ints(&[7]));
    tail.extend_from_slice(b"unreal\0");
    assert_eq!(add(&mut p, level, "PersistentLevel", 1, 4, Vec::new(), tail), 2);
    let role = p.byte_tag("Role", 3);
    let mut wi_tags = vec![role.clone(), p.float_tag("TimeDilation", 1.0)];
    if streaming.is_some() {
        wi_tags.push(p.array_tag("StreamingLevels", 1, &ints(&[6])));
    }
    assert_eq!(add(&mut p, world_info, "WorldInfo_0", 2, 26, wi_tags, Vec::new()), 3);
    assert_eq!(add(&mut p, actor, "Actor_0", 2, 26, vec![role.clone()], Vec::new()), 4);
    let mut tags = vec![role, p.byte_tag("RemoteRole", remote_role)];
    if let Some(mode) = physics {
        tags.push(p.enum_tag("Physics", "EPhysics", mode));
    }
    assert_eq!(add(&mut p, pawn, "Pawn_0", 2, 26, tags, Vec::new()), 5);
    if let Some(name) = streaming {
        let tags = vec![p.name_tag("PackageName", name)];
        assert_eq!(add(&mut p, level_streaming, "LevelStreaming_0", 3, 4, tags, Vec::new()), 6);
    }
    if kismet {
        // The Matinee scan's export walk (review r2 of #420): one Matinee op, two exports of a class that does not
        // resolve (the second from the walk's cache), and one export of a class on NOT_OP_CLASSES (a package object).
        let core = p.add_import("Core", "Package", 0, "Core");
        let package_class = p.add_import("Core", "Class", core, "Package");
        let interp = class(&mut p, "SeqAct_Interp");
        let unknown = class(&mut p, "NoSuchOp");
        add(&mut p, interp, "SeqAct_Interp_0", 0, 4, Vec::new(), Vec::new());
        add(&mut p, unknown, "NoSuchOp_0", 0, 4, Vec::new(), Vec::new());
        add(&mut p, unknown, "NoSuchOp_1", 0, 4, Vec::new(), Vec::new());
        add(&mut p, package_class, "SomeGroup", 0, 4, Vec::new(), Vec::new());
    }
    p.build()
}

/// A builder over a fresh store holding the invented packages, that also moves the pawn to `at` after the load (the
/// invented `Vector` has no tagged form here, so the placement is written, not read).
fn builder(remote_role: u8, at: [f32; 3], yaw: i32) -> Builder {
    builder_with(remote_role, None, false, at, yaw)
}

/// [`builder`] over a map whose `WorldInfo` streams a second level, `Test_Sub`, that places its own `Pawn_0`; the last
/// pawn (the streamed level's) is the one moved to `at`.
fn streaming_builder(at: [f32; 3]) -> Builder {
    builder_files(0, None, false, true, false, at, 0)
}

/// [`builder`] over a map that also holds the Kismet exports of [`map`]'s `kismet`.
fn kismet_builder() -> Builder {
    builder_files(0, None, false, false, true, [0.0; 3], 0)
}

/// [`builder`] with the pawn's `Physics` set to `physics` when given, and the host's physics step installed after
/// the load with Matinee bindings that name the pawn when `bound` (the invented packages hold no Matinee op).
fn builder_with(remote_role: u8, physics: Option<&'static str>, bound: bool, at: [f32; 3], yaw: i32) -> Builder {
    builder_files(remote_role, physics, bound, false, false, at, yaw)
}

/// [`builder_with`], with the streamed level `Test_Sub` when `streamed`.
fn builder_files(
    remote_role: u8,
    physics: Option<&'static str>,
    bound: bool,
    streamed: bool,
    kismet: bool,
    at: [f32; 3],
    yaw: i32,
) -> Builder {
    Box::new(move || {
        let dir = std::env::temp_dir().join(format!(
            "ow-host-world-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let engine = engine();
        let map_p = map(remote_role, physics, streamed.then_some("Test_Sub"), kismet);
        let map_sub = map(remote_role, physics, None, false);
        let mut files: Vec<(&str, &[u8])> = vec![("Engine.upk", &engine), ("Test_P.upk", &map_p)];
        if streamed {
            files.push(("Test_Sub.upk", &map_sub));
        }
        let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, ParityMode::Off)
            .map_err(|e| e.to_string())?;
        let vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
        let mut world = World::new(vm, play_balance()?, &[]).map_err(|e| e.to_string())?;
        let report = load_map(&mut world, &PackageName::parse("Test_P").unwrap()).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_dir_all(&dir);
        let list = world.actors().borrow().list().to_vec();
        let pawn = *list.last().unwrap();
        let mut bindings = matinee_bound(&mut world.vm)?;
        if !kismet {
            assert_eq!(bindings.ops, 0, "the invented packages hold no Matinee op");
        }
        if bound {
            bindings.bound.insert(pawn);
        }
        install_physics(&mut world, bindings)?;
        let space = &mut world.vm.space;
        let names = [("Location", ["X", "Y", "Z"]), ("Rotation", ["Pitch", "Yaw", "Roll"])];
        let values = [at.map(Value::Float), [Value::Int(0), Value::Int(yaw), Value::Int(0)]];
        for ((name, axes), values) in names.into_iter().zip(values) {
            let class = space.heap.get(pawn).unwrap().class;
            let p = space.prop(class, name).unwrap();
            let old = read(space, pawn, name).unwrap();
            let s = old.as_struct().unwrap();
            let layout = space.classes.struct_layout(s.ty).unwrap();
            let mut fields = s.fields.clone();
            for (axis, v) in axes.into_iter().zip(values) {
                let (entry, _) = layout.field(space.names.find(axis).unwrap()).unwrap();
                fields[usize::try_from(entry).unwrap()] = v;
            }
            let ty = s.ty;
            space.write(pawn, PropAddr::scalar(p), Value::Struct(Rc::new(StructValue { ty, fields }))).unwrap();
        }
        let actors = list.len();
        Ok((world, LoadSummary::from_report("Test_P", ParityMode::Off, &report, actors)))
    })
}

#[test]
fn the_loaded_world_ticks_on_its_thread_and_reports_its_pawns() {
    let (mut link, summary) = WorldLink::spawn(builder(0, [10.0, -20.0, 30.5], 16384)).unwrap();
    assert_eq!(summary.actors, 3, "{}", summary.text());
    assert_eq!(summary.levels, vec![("Test_P".to_string(), 4, 3, 0)]);
    assert_eq!(summary.pending.len(), 3, "the map load's three NotYets are carried");
    assert_eq!(summary.pawns.len(), 1);
    let pawn = &summary.pawns[0];
    assert_eq!((pawn.path.as_str(), pawn.class.as_str()), ("Test_P.TheWorld.PersistentLevel.Pawn_0", "Engine.Pawn"));
    assert_eq!(pawn.location, [10.0, -20.0, 30.5]);
    assert_eq!(pawn.rotation.yaw, 16384);
    assert_eq!(pawn.archetype, None);

    let dt = 1.0 / 60.0;
    let mut last = WorldFrame::default();
    for _ in 0..6 {
        last = link.tick(dt);
    }
    assert_eq!(last.halted, None);
    assert_eq!(last.ticks, 6);
    // The WorldInfo's time is the clock's f32 sum of six steps, its delta the last step's.
    let mut sum = 0.0f32;
    for _ in 0..6 {
        sum += dt;
    }
    assert_eq!(last.time_seconds, Some(sum));
    assert_eq!(last.delta_seconds, Some(dt));
    assert_eq!(last.pawns, summary.pawns, "nothing moves the placed pawn");
}

#[test]
fn a_tick_that_fails_halts_the_world_and_names_the_not_yet() {
    // RemoteRole 2 takes a tick variant the record did not read: the world's tick answers NotYet.
    let (mut link, _) = WorldLink::spawn(builder(2, [0.0; 3], 0)).unwrap();
    let first = link.tick(1.0 / 60.0);
    let halt = first.halted.clone().expect("the first tick halts");
    assert_eq!(halt.tick, 1);
    assert!(halt.not_yet, "{}", halt.message);
    assert!(halt.message.contains("RemoteRole"), "{}", halt.message);
    assert_eq!((first.ticks, first.time_seconds), (0, Some(1.0 / 60.0)));
    // A halted world runs no further tick and keeps its answer.
    assert_eq!(link.tick(1.0 / 60.0), first);
}

#[test]
fn a_walking_pawn_halts_the_tick_with_the_walking_step_s_not_yet() {
    let (mut link, _) = WorldLink::spawn(builder_with(0, Some("PHYS_Walking"), false, [0.0; 3], 0)).unwrap();
    let frame = link.tick(0.5);
    let halt = frame.halted.clone().expect("the walking pawn halts the tick");
    assert!(halt.not_yet, "{}", halt.message);
    assert!(halt.message.contains("the PHYS_Walking physics of a placed Engine.Pawn"), "{}", halt.message);
    assert!(halt.message.contains(PHYSICS_RECORD), "{}", halt.message);
    // The clock advanced before the actor pass, and the read-back shows it.
    assert_eq!((frame.ticks, frame.time_seconds, frame.delta_seconds), (0, Some(0.5), Some(0.5)));
}

#[test]
fn an_interpolating_actor_no_matinee_names_ticks_and_stays_put() {
    let at = [5.0, 6.0, 7.0];
    let (mut link, summary) = WorldLink::spawn(builder_with(0, Some("PHYS_Interpolating"), false, at, 0)).unwrap();
    let mut frame = WorldFrame::default();
    for _ in 0..3 {
        frame = link.tick(0.25);
    }
    assert_eq!((frame.halted.clone(), frame.ticks, frame.time_seconds), (None, 3, Some(0.75)));
    assert_eq!(frame.pawns, summary.pawns);
}

#[test]
fn an_interpolating_actor_a_matinee_names_waits_for_the_playback() {
    let (mut link, _) = WorldLink::spawn(builder_with(0, Some("PHYS_Interpolating"), true, [0.0; 3], 0)).unwrap();
    let halt = link.tick(0.25).halted.expect("the bound actor halts the tick");
    assert!(halt.not_yet, "{}", halt.message);
    assert!(halt.message.contains("the Matinee playback that moves a bound interpolating Engine.Pawn"), "{}", halt.message);
    assert!(halt.message.contains(KISMET_RECORD), "{}", halt.message);
}

#[test]
fn each_physics_mode_s_rule() {
    use ow_core::notyet::Missing;
    let c = "Engine.InterpActor";
    let off = ParityMode::Off;
    assert_eq!(physics_rule("PHYS_None", c, &Driver::Bound, off), Ok(()));
    assert_eq!(physics_rule("PHYS_Interpolating", c, &Driver::Unbound, off), Ok(()));
    assert_eq!(physics_rule("PHYS_Interpolating", c, &Driver::NotStarted, off), Ok(()));
    // R3: the no-op is Off's; the C++ reference has no host physics to stand in.
    for driver in [Driver::Unbound, Driver::NotStarted] {
        let reference = physics_rule("PHYS_Interpolating", c, &driver, ParityMode::Reference).unwrap_err();
        assert!(matches!(reference.missing, Missing::NotPorted), "{reference}");
    }
    let missing = |mode: &str, driver: Driver| physics_rule(mode, c, &driver, off).unwrap_err();
    let at_load = missing("PHYS_Interpolating", Driver::AtLoad("Engine.SeqEvent_LevelLoaded".into()));
    assert!(matches!(at_load.missing, Missing::LayerAbsent(_)), "{at_load}");
    assert!(at_load.to_string().contains("the level-load event (Engine.SeqEvent_LevelLoaded)"), "{at_load}");
    assert!(matches!(missing("PHYS_Interpolating", Driver::Bound).missing, Missing::LayerAbsent(_)));
    for (mode, driver, record, words) in [
        ("PHYS_Interpolating", Driver::Unknown, KISMET_RECORD, "whether a Matinee op names an interpolating"),
        (
            "PHYS_Interpolating",
            Driver::Outside("WillowGame.WillowSeqEvent_MissionRemoteEvent".into()),
            KISMET_RECORD,
            "whether WillowGame.WillowSeqEvent_MissionRemoteEvent, fired from outside the Kismet graph",
        ),
        ("PHYS_Walking", Driver::Unbound, PHYSICS_RECORD, "the PHYS_Walking physics of a placed"),
        ("PHYS_Falling", Driver::Unbound, PHYSICS_RECORD, "the PHYS_Falling physics of a placed"),
        ("PHYS_NavMeshWalking", Driver::Unbound, ENGINE_RECORD, "nav-mesh walking physics (a Gearbox mode)"),
        ("PHYS_Rotating", Driver::Unbound, ENGINE_RECORD, "how RotationRate x DeltaTime becomes rotator units"),
        ("PHYS_Spider", Driver::Unbound, ENGINE_RECORD, "the PHYS_Spider physics of a placed"),
    ] {
        let n = missing(mode, driver);
        assert!(matches!(n.missing, Missing::NotRead), "{mode}: {n}");
        assert_eq!(n.record, record, "{mode}");
        assert!(n.to_string().contains(words), "{mode}: {n}");
    }
}

#[test]
fn two_levels_pawns_of_one_name_keep_their_own_paths_and_places() {
    let (mut link, summary) = WorldLink::spawn(streaming_builder([1.0, 2.0, 3.0])).unwrap();
    assert_eq!(summary.levels.len(), 2, "{}", summary.text());
    let paths: Vec<&str> = summary.pawns.iter().map(|p| p.path.as_str()).collect();
    assert_eq!(paths, ["Test_P.TheWorld.PersistentLevel.Pawn_0", "Test_Sub.TheWorld.PersistentLevel.Pawn_0"]);
    assert_eq!(summary.pawns[0].location, [0.0; 3]);
    assert_eq!(summary.pawns[1].location, [1.0, 2.0, 3.0]);
    let frame = link.tick(1.0 / 60.0);
    assert_eq!(frame.pawns, summary.pawns);
    assert!(frame.unplaced.is_empty());
}

#[test]
fn a_transform_that_cannot_be_read_is_refused_not_zeroed() {
    let (mut world, _) = streaming_builder([1.0, 2.0, 3.0])().unwrap();
    let streaming_class = world.vm.find_class("Engine.LevelStreaming").unwrap();
    let space = &world.vm.space;
    let list = world.actors().borrow().list().to_vec();
    let pawn = *list.last().unwrap();
    assert_eq!(transform(space, pawn), Ok(([1.0, 2.0, 3.0], Rotator::new(0, 0, 0))));
    // A rotator is not a vector, nor a vector a rotator.
    let rotation = read(space, pawn, "Rotation").unwrap();
    let location = read(space, pawn, "Location").unwrap();
    assert_eq!(vector_of(space, &rotation), None);
    assert_eq!(rotator_of(space, &location), None);
    // An object with no Location (the persistent level's LevelStreaming) has no transform.
    let streaming = space.live(streaming_class).next();
    assert_eq!(transform(space, streaming.expect("the LevelStreaming is loaded")), Err("no Location"));
}

#[test]
fn a_world_thread_panic_is_reported_with_its_message() {
    let err = WorldLink::spawn(Box::new(|| panic!("an invented failure"))).unwrap_err();
    assert_eq!(err, "the world thread panicked before loading the map: an invented failure");
}

/// The Matinee scan's reading of one op's variables, on objects of the invented Kismet classes (review r1 of #420):
/// a plain object variable binds its actor, an empty one is counted as empty, and a subclass of it or a named
/// variable makes the scan incomplete, so every actor's driver is Unknown.
#[test]
fn the_scan_reads_each_variable_kind_and_flags_what_it_cannot_see() {
    let (mut world, _) = builder(0, [0.0; 3], 0)().unwrap();
    let pawn = *world.actors().borrow().list().last().unwrap();
    let vm = &mut world.vm;
    let link_ty = vm.space.classes.struct_by_path("Engine.Object.SeqVarLink").expect("the invented link struct");
    let object = |vm: &mut Vm, class: &str, name: &str| {
        let c = vm.find_class(class).unwrap();
        vm.space.alloc(c, name, None, ow_object::heap::Origin::Runtime).unwrap()
    };
    let set = |vm: &mut Vm, o: ObjRef, name: &str, v: Value| {
        let class = vm.space.heap.get(o).unwrap().class;
        let p = vm.space.prop(class, name).unwrap();
        vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    };
    // One op linking `vars` through one link.
    let op_over = |vm: &mut Vm, vars: Vec<ObjRef>| {
        let op = object(vm, "Engine.SeqAct_Interp", "Interp");
        let layout = vm.space.classes.struct_layout(link_ty).unwrap();
        let (entry, _) = layout.field(vm.space.names.find("LinkedVariables").unwrap()).unwrap();
        let Value::Struct(zero) = layout.zero().clone() else { panic!("the link is a struct") };
        let mut fields = zero.fields.clone();
        fields[usize::try_from(entry).unwrap()] = Value::array(vars.into_iter().map(|v| Value::Object(Some(v))).collect());
        set(vm, op, "VariableLinks", Value::array(vec![Value::structure(link_ty, fields)]));
        op
    };
    let plain = object(vm, "Engine.SeqVar_Object", "Plain");
    set(vm, plain, "ObjValue", Value::Object(Some(pawn)));
    let empty = object(vm, "Engine.SeqVar_Object", "Empty");
    let scalar = object(vm, "Engine.SeqVar_Float", "Scalar");
    let op = op_over(vm, vec![plain, empty, scalar]);
    let mut out = MatineeBindings::default();
    read_op_variables(vm, op, &mut out).unwrap();
    assert_eq!((out.bound.iter().copied().collect::<Vec<_>>(), out.empty), (vec![pawn], 1));
    assert!(out.complete(), "{out:?}");
    assert_eq!(Driver::of(&out, pawn), Driver::Bound);
    assert_eq!(Driver::of(&out, plain), Driver::Unbound);
    // A subclass of the object variable with no value (an object list, say): incomplete, every driver Unknown.
    let list = object(vm, "Engine.SeqVar_ObjectList", "List");
    let op = op_over(vm, vec![list]);
    let mut out = MatineeBindings::default();
    read_op_variables(vm, op, &mut out).unwrap();
    assert_eq!((out.empty, out.unreadable), (0, 1));
    assert_eq!(Driver::of(&out, pawn), Driver::Unknown);
    // A named variable (no ObjValue, not a class known to hold no actor): incomplete too.
    let named = object(vm, "Engine.SeqVar_Named", "Named");
    let op = op_over(vm, vec![named]);
    let mut out = MatineeBindings::default();
    read_op_variables(vm, op, &mut out).unwrap();
    assert_eq!(out.unreadable, 1);
    assert_eq!(Driver::of(&out, pawn), Driver::Unknown);
    // An op whose VariableLinks is not an array of links is counted, and makes the scan incomplete.
    let bare = object(vm, "Engine.SeqVar_Float", "NotAnOp");
    let mut out = MatineeBindings::default();
    read_op_variables(vm, bare, &mut out).unwrap();
    assert_eq!(out.unreadable_ops, 1);
    assert!(!out.complete());
    // An export class that does not resolve makes it incomplete as well.
    let mut out = MatineeBindings::default();
    out.unresolved_classes.insert("Nowhere.Unknown".into(), 1);
    assert_eq!(Driver::of(&out, pawn), Driver::Unknown);
}

/// The export walk of [`matinee_bound`] over an invented map (review r2 of #420): the Matinee op export is read, the
/// class on `NOT_OP_CLASSES` is skipped, and the class that does not resolve is counted once per export, so the scan
/// is incomplete and the placed pawn's driver is Unknown.
#[test]
fn the_export_walk_reads_ops_skips_listed_classes_and_counts_unresolved_ones() {
    let (mut world, _) = kismet_builder()().unwrap();
    let pawn = *world.actors().borrow().list().last().unwrap();
    let bindings = matinee_bound(&mut world.vm).unwrap();
    assert_eq!(bindings.ops, 1, "{bindings:?}");
    assert_eq!(
        bindings.unresolved_classes.iter().map(|(c, n)| (c.as_str(), *n)).collect::<Vec<_>>(),
        [("Engine.NoSuchOp", 2)],
        "Core.Package is on NOT_OP_CLASSES; the unresolved class is counted for each export"
    );
    assert!(!bindings.complete());
    assert_eq!(Driver::of(&bindings, pawn), Driver::Unknown);
}

/// `n` distinct objects to stand for Kismet ops in a hand-made graph (the invented packages hold no Kismet).
fn op_refs(n: usize) -> Vec<ObjRef> {
    let (mut world, _) = builder(0, [0.0; 3], 0)().unwrap();
    let class = world.vm.find_class("Engine.Actor").unwrap();
    (0..n)
        .map(|i| world.vm.space.alloc(class, &format!("Op_{i}"), None, ow_object::heap::Origin::Runtime).unwrap())
        .collect()
}

/// A graph over `ops` with `classes` (index, class path), `sources` (op, its sources), and the event sets.
fn graph(
    ops: &[ObjRef],
    classes: &[(usize, &str)],
    sources: &[(usize, &[usize])],
    load: &[usize],
    events: &[usize],
    gameplay: &[usize],
) -> matinee::KismetGraph {
    let mut g = matinee::KismetGraph::default();
    for &(i, c) in classes {
        g.ops.insert(ops[i], c.to_string());
    }
    for &(i, from) in sources {
        g.sources.insert(ops[i], from.iter().map(|&f| ops[f]).collect());
    }
    g.load_events = load.iter().map(|&i| ops[i]).collect();
    g.events = events.iter().map(|&i| ops[i]).collect();
    g.gameplay_events = gameplay.iter().map(|&i| ops[i]).collect();
    g
}

#[test]
fn an_op_starts_at_load_only_if_a_level_load_event_reaches_it() {
    use matinee::{Start, start_of};
    let o = op_refs(8);
    let (load, mission, touch, dead, action, op) = (0, 1, 2, 3, 4, 5);
    let classes = [
        (load, "Engine.SeqEvent_LevelLoaded"),
        (mission, "WillowGame.WillowSeqEvent_MissionRemoteEvent"),
        (touch, "Engine.SeqEvent_Touch"),
        (dead, "Engine.SeqAct_Toggle"),
        (action, "Engine.SeqAct_Delay"),
        (op, "Engine.SeqAct_Interp"),
    ];
    let events = [load, mission, touch];
    // Nothing links to the op: it never starts.
    let g = graph(&o, &classes, &[], &[load], &events, &[touch]);
    assert_eq!(start_of(&g, o[op]), Start::NotAtLoad);
    // A load event two links up (through an action): it starts at load, and the load event wins over the others.
    let g = graph(&o, &classes, &[(op, &[action, touch]), (action, &[load, mission])], &[load], &events, &[touch]);
    assert_eq!(start_of(&g, o[op]), Start::AtLoad(vec!["Engine.SeqEvent_LevelLoaded".into()]));
    // A mission remote event (fired from outside the graph): named, not assumed quiet.
    let g = graph(&o, &classes, &[(op, &[action]), (action, &[mission])], &[load], &events, &[touch]);
    assert_eq!(start_of(&g, o[op]), Start::Outside(vec!["WillowGame.WillowSeqEvent_MissionRemoteEvent".into()]));
    // Only a touch and an action no link reaches: not at load.
    let g = graph(&o, &classes, &[(op, &[touch, dead])], &[load], &events, &[touch]);
    assert_eq!(start_of(&g, o[op]), Start::NotAtLoad);
    // A remote event whose action a load event fires (the name edge read_graph adds): at load.
    let (remote, activate) = (6, 7);
    let classes2 = [
        (load, "Engine.SeqEvent_LevelLoaded"),
        (remote, "Engine.SeqEvent_RemoteEvent"),
        (activate, "Engine.SeqAct_ActivateRemoteEvent"),
        (op, "Engine.SeqAct_Interp"),
    ];
    let g = graph(&o, &classes2, &[(op, &[remote]), (remote, &[activate]), (activate, &[load])], &[load], &[load, remote], &[]);
    assert_eq!(start_of(&g, o[op]), Start::AtLoad(vec!["Engine.SeqEvent_LevelLoaded".into()]));
    // The same remote event with no action in the graph: fired from outside.
    let g = graph(&o, &classes2, &[(op, &[remote])], &[load], &[load, remote], &[]);
    assert_eq!(start_of(&g, o[op]), Start::Outside(vec!["Engine.SeqEvent_RemoteEvent".into()]));
    // A cycle with no event in it: nothing starts it.
    let g = graph(&o, &classes, &[(op, &[action]), (action, &[dead]), (dead, &[action])], &[load], &events, &[touch]);
    assert_eq!(start_of(&g, o[op]), Start::NotAtLoad);
}

#[test]
fn an_actor_s_driver_combines_the_ops_that_bind_it() {
    use matinee::Start;
    let o = op_refs(4);
    let (actor, a, b, c) = (o[0], o[1], o[2], o[3]);
    let mut bindings = MatineeBindings::default();
    bindings.bound.insert(actor);
    bindings.by_actor.insert(actor, [a, b].into_iter().collect());
    // A start not worked out: the playback NotYet, as before this lane.
    assert_eq!(Driver::of(&bindings, actor), Driver::Bound);
    bindings.starts.insert(a, Start::NotAtLoad);
    bindings.starts.insert(b, Start::NotAtLoad);
    assert_eq!(Driver::of(&bindings, actor), Driver::NotStarted);
    bindings.starts.insert(b, Start::Outside(vec!["X.Mission".into()]));
    assert_eq!(Driver::of(&bindings, actor), Driver::Outside("X.Mission".into()));
    bindings.by_actor.insert(actor, [a, b, c].into_iter().collect());
    bindings.starts.insert(c, Start::AtLoad(vec!["Engine.SeqEvent_LevelLoaded".into()]));
    assert_eq!(Driver::of(&bindings, actor), Driver::AtLoad("Engine.SeqEvent_LevelLoaded".into()));
    // An actor no op binds, and an unreadable scan.
    assert_eq!(Driver::of(&bindings, a), Driver::Unbound);
    bindings.unreadable = 1;
    assert_eq!(Driver::of(&bindings, actor), Driver::Unknown);
}

#[test]
fn a_builder_error_comes_back_from_spawn() {
    let err = WorldLink::spawn(Box::new(|| Err("no store".to_string()))).unwrap_err();
    assert_eq!(err, "no store");
}

/// Sanctuary on the installed game (`OW_BL2_ROOT`): the map loads into a world on the world thread and is ticked for
/// a second of play steps. Writes `local/p2/h-world/installed.txt`.
#[test]
#[ignore = "reads the installed game (OW_BL2_ROOT); writes local/p2/h-world/"]
fn sanctuary_world_ticks_on_the_installed_game() {
    let root = std::env::var_os("OW_BL2_ROOT").expect("set OW_BL2_ROOT to the install folder");
    let cooked = std::path::Path::new(&root).join("WillowGame/CookedPCConsole");
    let (mut link, summary) =
        WorldLink::spawn(Box::new(move || build_installed(cooked, "Sanctuary_P".into(), ParityMode::Off))).unwrap();
    let mut text = summary.text();
    let mut frame = WorldFrame::default();
    for _ in 0..60 {
        frame = link.tick(1.0 / 60.0);
    }
    text.push_str(&format!(
        "after 60 requests: {} ticks, TimeSeconds {:?}, DeltaSeconds {:?}, halted {:?}\n",
        frame.ticks, frame.time_seconds, frame.delta_seconds, frame.halted
    ));
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../local/p2/h-world");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("installed.txt"), &text).unwrap();
    print!("{text}");
}

/// The physics-mode census of the installed Sanctuary world (lane H-PHYSICS-MODES): for every placed actor whose
/// `Physics` is not `PHYS_None`, its mode by name, class, whether it has a `Base`, and for `PHYS_Interpolating` whether
/// a loaded Matinee op (`SeqAct_Interp`) binds it through an object-variable link; for `PHYS_Rotating`, its
/// `RotationRate` and `bRotateToDesired`. Counts only; writes `local/p2/h-physics-modes/census.txt`.
#[test]
#[ignore = "reads the installed game (OW_BL2_ROOT); writes local/p2/h-physics-modes/"]
fn sanctuary_physics_census() {
    let root = std::env::var_os("OW_BL2_ROOT").expect("set OW_BL2_ROOT to the install folder");
    let cooked = std::path::Path::new(&root).join("WillowGame/CookedPCConsole");
    let text = ow_vm::interp::on_vm_stack(move || {
        let (mut world, _) = build_installed(cooked, "Sanctuary_P".into(), ParityMode::Off).unwrap();
        let list = world.actors().borrow().list().to_vec();
        let bindings = scan_matinee(&mut world.vm).unwrap();
        let bound = bindings.bound.clone();
        let space = &world.vm.space;
        let mut census: std::collections::BTreeMap<String, usize> = Default::default();
        let mut modes: std::collections::BTreeMap<String, usize> = Default::default();
        // What the tick meets per physics actor, in list order: the world's Base NotYet before physics, else the
        // host's rule. The first entry is where tick 1 stops.
        let mut outcomes: std::collections::BTreeMap<String, usize> = Default::default();
        let mut first_stop: Option<String> = None;
        for &o in &list {
            let mode = physics_name(space, o);
            *modes.entry(mode.clone()).or_default() += 1;
            if mode == "PHYS_None" {
                continue;
            }
            let class = ow_attr::eval::class_path(space, o);
            let base = matches!(read(space, o, "Base"), Some(Value::Object(Some(_))));
            let mut extra = String::new();
            if mode == "PHYS_Interpolating" {
                extra = format!(" matinee-bound {}", bound.contains(&o));
            }
            if mode == "PHYS_Rotating" {
                let rate = read(space, o, "RotationRate").map(|v| {
                    ["Pitch", "Yaw", "Roll"].map(|a| match field(space, &v, a) {
                        Some(Value::Int(n)) => *n,
                        _ => i32::MIN,
                    })
                });
                extra = format!(" RotationRate {rate:?} bRotateToDesired {:?}", read(space, o, "bRotateToDesired"));
            }
            *census.entry(format!("{mode} {class} base {base}{extra}")).or_default() += 1;
            let driver = Driver::of(&bindings, o);
            let outcome = if base {
                "the world's Base NotYet".to_string()
            } else {
                match physics_rule(&mode, &class, &driver, ParityMode::Off) {
                    Ok(()) => "passes (no-op)".to_string(),
                    Err(n) => format!("NotYet: {}", n.what),
                }
            };
            if first_stop.is_none() && outcome != "passes (no-op)" {
                first_stop = Some(format!("{class}: {outcome}"));
            }
            *outcomes.entry(outcome).or_default() += 1;
        }
        let mut out = format!(
            "actors {}; {} matinee ops bind {} actors; {} empty object variables; other linked variables {:?}; \
             unreadable variables {}, unreadable ops {}, unresolved export classes {:?}; complete {}\n",
            list.len(),
            bindings.ops,
            bound.len(),
            bindings.empty,
            bindings.unresolved,
            bindings.unreadable,
            bindings.unreadable_ops,
            bindings.unresolved_classes,
            bindings.complete()
        );
        out.push_str(&format!("first stop in list order: {first_stop:?}
"));
        for (k, n) in &outcomes {
            out.push_str(&format!("outcome x{n}: {k}
"));
        }
        for (m, n) in &modes {
            out.push_str(&format!("mode {m}: {n}\n"));
        }
        for (k, n) in &census {
            out.push_str(&format!("{n:5} {k}\n"));
        }
        out
    })
    .unwrap();
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../local/p2/h-physics-modes");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("census.txt"), &text).unwrap();
    print!("{text}");
}

/// H-MATINEE's census of Sanctuary's Matinee ops: how each can start, the op's flags, its tracks by class, and the
/// physics of the actors it binds. Counts only; writes `local/p2/h-matinee/census.txt`.
#[test]
#[ignore = "reads the installed game (OW_BL2_ROOT); writes local/p2/h-matinee/"]
fn sanctuary_matinee_census() {
    use matinee::{Start, read_graph, start_of};
    let root = std::env::var_os("OW_BL2_ROOT").expect("set OW_BL2_ROOT to the install folder");
    let cooked = std::path::Path::new(&root).join("WillowGame/CookedPCConsole");
    let text = ow_vm::interp::on_vm_stack(move || {
        let (mut world, _) = build_installed(cooked, "Sanctuary_P".into(), ParityMode::Off).unwrap();
        let graph = read_graph(&mut world.vm).unwrap();
        let mut out = format!("ops {}, matinee {}, load events {}\n", graph.ops.len(), graph.matinee.len(), graph.load_events.len());
        let mut classes: std::collections::BTreeMap<String, usize> = Default::default();
        for c in graph.ops.values() {
            *classes.entry(c.clone()).or_default() += 1;
        }
        for (c, n) in &classes {
            if c.contains("Event") {
                out.push_str(&format!("event class {c}: {n}\n"));
            }
        }
        // The op class's boolean flags by name.
        if let Some(&op) = graph.matinee.first() {
            let class = world.vm.space.heap.get(op).unwrap().class;
            let mut flags = Vec::new();
            for name in ["bLooping", "bRewindOnPlay", "bForceStartPos", "bIsPlaying", "bClientSideOnly", "bAutoPlay", "bPlayOnLoad"] {
                flags.push(format!("{name}={}", world.vm.space.prop(class, name).is_some()));
            }
            out.push_str(&format!("op class props: {}\n", flags.join(" ")));
        }
        let mut starts: std::collections::BTreeMap<String, usize> = Default::default();
        let mut tracks: std::collections::BTreeMap<String, usize> = Default::default();
        for &op in &graph.matinee.clone() {
            let start = start_of(&graph, op);
            let key = match &start {
                Start::NotAtLoad => "not at load".to_string(),
                Start::AtLoad(e) => format!("at load via {e:?}"),
                Start::Outside(e) => format!("from outside the graph via {e:?}"),
            };
            let playing = read(&world.vm.space, op, "bIsPlaying");
            let looping = read(&world.vm.space, op, "bLooping");
            *starts.entry(format!("{key}; bIsPlaying {playing:?} bLooping {looping:?}")).or_default() += 1;
            if matches!(start, Start::NotAtLoad) {
                let up = matinee::upstream(&graph, op);
                let mut roots: Vec<String> = up
                    .iter()
                    .filter(|o| graph.sources.get(o).is_none_or(|s| s.is_empty()))
                    .map(|o| graph.ops[o].clone())
                    .collect();
                roots.sort();
                roots.dedup();
                *starts.entry(format!("  roots of a not-at-load op: {roots:?}")).or_default() += 1;
            }
            let kind = match &start {
                Start::NotAtLoad => "not-at-load",
                Start::AtLoad(_) => "load-started",
                Start::Outside(_) => "outside-started",
            };
            {
                // Its InterpData: a linked variable of class InterpData; its groups' tracks.
                let Some(Value::Array(links)) = read(&world.vm.space, op, "VariableLinks") else { continue };
                let mut vars = Vec::new();
                for l in links.iter() {
                    if let Some(Value::Array(v)) = field(&world.vm.space, l, "LinkedVariables") {
                        vars.extend(v.iter().filter_map(|x| if let Value::Object(Some(r)) = x { Some(*r) } else { None }));
                    }
                }
                let mut op_tracks: Vec<String> = Vec::new();
                for var in vars {
                    world.vm.defs.ensure_loaded(&mut world.vm.space, var).unwrap();
                    if !ow_attr::eval::class_path(&world.vm.space, var).ends_with("InterpData") {
                        continue;
                    }
                    let length = match read(&world.vm.space, var, "InterpLength") {
                        Some(Value::Float(f)) => f,
                        _ => f32::NAN,
                    };
                    let Some(Value::Array(groups)) = read(&world.vm.space, var, "InterpGroups") else { continue };
                    for g in groups.iter() {
                        let Value::Object(Some(g)) = g else { continue };
                        world.vm.defs.ensure_loaded(&mut world.vm.space, *g).unwrap();
                        let Some(Value::Array(ts)) = read(&world.vm.space, *g, "InterpTracks") else { continue };
                        for t in ts.iter() {
                            let Value::Object(Some(t)) = t else { continue };
                            world.vm.defs.ensure_loaded(&mut world.vm.space, *t).unwrap();
                            let class = ow_attr::eval::class_path(&world.vm.space, *t);
                            *tracks.entry(format!("{kind} {class}")).or_default() += 1;
                            op_tracks.push(class.rsplit('.').next().unwrap_or("").to_string());
                            if class.ends_with(".InterpTrackMove") {
                                // The reader, and the oracle that the keys end within the op's InterpLength.
                                let outcome = match matinee::track::read_move(&world.vm.space, *t) {
                                    Ok(m) => format!(
                                        "read: {:?} {:?}, keys {} + {}, ends within InterpLength {}",
                                        m.frame,
                                        m.rot_mode,
                                        m.position.keys.len(),
                                        m.euler.keys.len(),
                                        m.length() <= length + 1.0e-4
                                    ),
                                    Err(n) => format!("NotYet: {}", n.what),
                                };
                                *tracks.entry(format!("{kind} move track {outcome}")).or_default() += 1;
                            }
                        }
                    }
                }
                op_tracks.sort();
                op_tracks.dedup();
                if kind == "load-started" {
                    *starts.entry(format!("  tracks of a load-started op: {op_tracks:?}")).or_default() += 1;
                }
            }
        }
        for (k, n) in &starts {
            out.push_str(&format!("start x{n}: {k}\n"));
        }
        for (k, n) in &tracks {
            out.push_str(&format!("track {k}: {n}\n"));
        }
        out
    })
    .unwrap();
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../local/p2/h-matinee");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("census.txt"), &text).unwrap();
    print!("{text}");
}
