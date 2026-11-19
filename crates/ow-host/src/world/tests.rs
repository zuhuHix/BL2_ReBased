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
    engine_with(true)
}

/// [`engine`], whose `SeqEvent_RemoteEvent` declares no `EventName` unless `remote_named` (delta review of #436,
/// finding 4: a remote event whose name does not read).
fn engine_with(remote_named: bool) -> Vec<u8> {
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
        "StrProperty",
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
    // The Kismet graph's classes (lane H-MATINEE review r1): ops with output links, events, actions, sequences.
    let input_link = toy_struct(&mut t, object, "SeqOpOutputInputLink", None);
    let sequence_op = t.class("SequenceOp", object);
    t.prop("Object", input_link, "LinkedOp", 0, sequence_op);
    let output_link = toy_struct(&mut t, object, "SeqOpOutputLink", None);
    toy_array_of(&mut t, output_link, "Links", "Struct", input_link);
    t.prop_tail("Str", output_link, "LinkDesc", 0, &[]);
    toy_array_of(&mut t, sequence_op, "OutputLinks", "Struct", output_link);
    let event = t.class("SequenceEvent", sequence_op);
    let action = t.class("SequenceAction", sequence_op);
    t.class("Sequence", sequence_op);
    for e in ["SeqEvent_LevelLoaded", "SeqEvent_Touch", "SeqEvent_SequenceActivated", "SeqEvent_Mission"] {
        t.class(e, event);
    }
    let remote = t.class("SeqEvent_RemoteEvent", event);
    if remote_named {
        t.prop("Name", remote, "EventName", 0, 0);
    }
    let activate = t.class("SeqAct_ActivateRemoteEvent", action);
    t.prop("Name", activate, "EventName", 0, 0);
    let finish = t.class("SeqAct_FinishSequence", action);
    t.prop_tail("Str", finish, "OutputLabel", 0, &[]);
    let interp = t.class("SeqAct_Interp", action);
    toy_array_of(&mut t, interp, "VariableLinks", "Struct", link);
    // The movement track's classes: the curve structs, the track and its enums.
    let enum_export = |t: &mut ScriptToy, outer: i32, name: &str, values: &[&str]| {
        let mut payload = ints(&[0]);
        payload.extend_from_slice(&t.none);
        payload.extend(ints(&[0, values.len() as i32]));
        for v in values {
            let bytes = t.p.fname(v, 0);
            payload.extend_from_slice(&bytes);
        }
        let mut e = Export::new(t.import("Enum"), name, payload);
        e.outer = outer;
        t.p.add_export_with(e)
    };
    let curve_mode = enum_export(&mut t, object, "EInterpCurveMode", &[
        "CIM_Linear",
        "CIM_CurveAuto",
        "CIM_Constant",
        "CIM_CurveUser",
        "CIM_CurveBreak",
        "CIM_CurveAutoClamped",
    ]);
    let curve_method = enum_export(&mut t, object, "EInterpCurveMethod", &[
        "IMT_UseFixedTangentEvalAndNewAutoTangents",
        "IMT_UseFixedTangentEval",
        "IMT_UseBrokenTangentEval",
    ]);
    let point = toy_struct(&mut t, object, "InterpCurvePointVector", None);
    t.prop("Float", point, "InVal", 0, 0);
    for v in ["OutVal", "ArriveTangent", "LeaveTangent"] {
        t.prop("Struct", point, v, 0, vector);
    }
    t.prop("Byte", point, "InterpMode", 0, curve_mode);
    let curve = toy_struct(&mut t, object, "InterpCurveVector", None);
    toy_array_of(&mut t, curve, "Points", "Struct", point);
    t.prop("Byte", curve, "InterpMethod", 0, curve_method);
    let track = t.class("InterpTrackMove", object);
    let frame = enum_export(&mut t, track, "EInterpTrackMoveFrame", &["IMF_World", "IMF_RelativeToInitial"]);
    let rot = enum_export(&mut t, track, "EInterpTrackMoveRotMode", &["IMR_Keyframed", "IMR_LookAtGroup", "IMR_Ignore"]);
    for c in ["PosTrack", "EulerTrack"] {
        t.prop("Struct", track, c, 0, curve);
    }
    t.prop("Byte", track, "MoveFrame", 0, frame);
    t.prop("Byte", track, "RotMode", 0, rot);
    for b in ["bUseQuatInterpolation", "bDisableMovement"] {
        t.prop("Bool", track, b, 0, 0);
    }
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

/// What [`map`] holds of Kismet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kismet {
    None,
    /// The Matinee scan's export walk (review r2 of #420).
    Walk,
    /// A remote event `Go` starting a Matinee op whose object variable holds `Pawn_0` (delta review of #436,
    /// finding 5). In [`builder_files`], the streamed level is then [`Kismet::RemoteSource`].
    RemoteTarget,
    /// A level that places no actor (it lists only its `WorldInfo`), with a level-loaded event firing `Go`.
    RemoteSource,
}

/// One output (`desc`) linking to `targets`, as an `OutputLinks` tag.
fn output_links(p: &mut PackageBuilder, desc: &str, targets: &[i32]) -> Vec<u8> {
    let mut links = Vec::new();
    for &t in targets {
        links.extend(p.object_tag("LinkedOp", t));
        links.extend_from_slice(&p.none());
    }
    let mut element = p.array_tag("Links", i32::try_from(targets.len()).unwrap(), &links);
    element.extend(p.str_tag("LinkDesc", desc));
    element.extend_from_slice(&p.none());
    p.array_tag("OutputLinks", 1, &element)
}

/// A map: `TheWorld` (1), `PersistentLevel` (2), `WorldInfo_0` (3, time dilation 1), `Actor_0` (4), `Pawn_0` (5);
/// the level lists 3, a null, 4 and 5 (only 3 for [`Kismet::RemoteSource`]). The pawn's `RemoteRole` is `remote_role`, and its `Physics` is `physics` when
/// given. With `streaming`, the `WorldInfo` names that level through a `LevelStreaming` (6).
fn map(remote_role: u8, physics: Option<&str>, streaming: Option<&str>, kismet: Kismet) -> Vec<u8> {
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
    let mut tail = if kismet == Kismet::RemoteSource { ints(&[2, 1, 3]) } else { ints(&[2, 4, 3, 0, 4, 5]) };
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
    if kismet == Kismet::Walk {
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
    if kismet == Kismet::RemoteTarget {
        let (var_class, interp, remote) =
            (class(&mut p, "SeqVar_Object"), class(&mut p, "SeqAct_Interp"), class(&mut p, "SeqEvent_RemoteEvent"));
        let value = p.object_tag("ObjValue", 5);
        let var = add(&mut p, var_class, "SeqVar_Object_0", 0, 4, vec![value], Vec::new());
        let mut link = p.array_tag("LinkedVariables", 1, &ints(&[var]));
        link.extend_from_slice(&p.none());
        let links = p.array_tag("VariableLinks", 1, &link);
        let op = add(&mut p, interp, "SeqAct_Interp_0", 0, 4, vec![links], Vec::new());
        let name = p.name_tag("EventName", "Go");
        let out = output_links(&mut p, "Out", &[op]);
        add(&mut p, remote, "RemoteEvent_Go", 0, 4, vec![name, out], Vec::new());
    }
    if kismet == Kismet::RemoteSource {
        let (loaded, activate) = (class(&mut p, "SeqEvent_LevelLoaded"), class(&mut p, "SeqAct_ActivateRemoteEvent"));
        let name = p.name_tag("EventName", "Go");
        let action = add(&mut p, activate, "Activate_Go", 0, 4, vec![name], Vec::new());
        let out = output_links(&mut p, "Out", &[action]);
        add(&mut p, loaded, "LevelLoaded_0", 0, 4, vec![out], Vec::new());
    }
    p.build()
}

/// The exports of [`kismet_package_with`], by index.
mod k {
    pub const PARENT: i32 = 1;
    pub const CHILD: i32 = 2;
    pub const LOADED: i32 = 3;
    pub const FINISH: i32 = 4;
    pub const EXIT_OP: i32 = 5;
    pub const PING: i32 = 6;
    pub const PING_ACTION: i32 = 7;
    pub const REMOTE_OP: i32 = 8;
    pub const TOUCH: i32 = 9;
    pub const TOUCH_OP: i32 = 10;
    pub const LONE_OP: i32 = 11;
    pub const TRACK: i32 = 12;
    pub const NAN_TRACK: i32 = 13;
    pub const LOAD_ENTRY: i32 = 14;
    pub const GATED: i32 = 15;
    pub const GATE_TOUCH: i32 = 16;
    pub const GATE_FINISH: i32 = 17;
    pub const GATED_OP: i32 = 18;
}

/// A Vector's native body: three floats.
fn vector_bytes(v: [f32; 3]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// `Test_K.upk` ([`kismet_package_with`]), an invented Kismet package (review r1 of #436), every op read through `read_graph`:
/// - a parent sequence (1) and a child sequence (2) in it; in the child, a level-loaded event (3) linked to a finish op
///   (4) labelled `Out`; the child's output `Out` links to a Matinee op in the parent (5): the exit edge;
/// - a remote event `Ping` (6) linked to an action firing `Ping` (7), which the name edge closes into a cycle, and the
///   event also linked to a Matinee op (8);
/// - a touch event (9) linked to a Matinee op (10), and a Matinee op nothing links to (11);
/// - a movement track (12): position keys at 0 and 2 s with a broken-tangent curve segment, and Euler yaw from 0 to
///   720 degrees (two turns); and a track (13) whose middle key time is NaN;
/// - (delta review of #436, finding 3) a level-loaded event (14) entering a second child sequence (15), in which only a
///   touch (16) reaches the finish op (17) labelled `Done`; the child's output `Done` links to a Matinee op (18).
///
/// The remote event `Ping` (6) has no `EventName` tag unless `remote_named` (for [`engine_with`]`(false)`).
fn kismet_package_with(remote_named: bool) -> Vec<u8> {
    let mut p = PackageBuilder::new_empty();
    p.names(["None"]);
    let engine = p.add_import("Core", "Package", 0, "Engine");
    let class = |p: &mut PackageBuilder, name: &str| p.add_import("Core", "Class", engine, name);
    let classes: Vec<i32> = [
        "Sequence",
        "SeqEvent_LevelLoaded",
        "SeqAct_FinishSequence",
        "SeqAct_Interp",
        "SeqEvent_RemoteEvent",
        "SeqAct_ActivateRemoteEvent",
        "SeqEvent_Touch",
        "InterpTrackMove",
    ]
    .iter()
    .map(|c| class(&mut p, c))
    .collect();
    let (sequence, loaded, finish, interp, remote, activate, touch, track) =
        (classes[0], classes[1], classes[2], classes[3], classes[4], classes[5], classes[6], classes[7]);
    let outputs = output_links;
    let add = |p: &mut PackageBuilder, class: i32, name: &str, outer: i32, tags: Vec<Vec<u8>>| {
        let mut body = vec![0u8; 4];
        for t in tags {
            body.extend_from_slice(&t);
        }
        body.extend_from_slice(&p.none());
        let mut export = Export::new(class, name, body);
        export.outer = outer;
        p.add_export_with(export)
    };
    assert_eq!(add(&mut p, sequence, "Main_Sequence", 0, Vec::new()), k::PARENT);
    let child_out = outputs(&mut p, "Out", &[k::EXIT_OP]);
    assert_eq!(add(&mut p, sequence, "Child", k::PARENT, vec![child_out]), k::CHILD);
    let to_finish = outputs(&mut p, "Out", &[k::FINISH]);
    assert_eq!(add(&mut p, loaded, "LevelLoaded_0", k::CHILD, vec![to_finish]), k::LOADED);
    let label = p.str_tag("OutputLabel", "Out");
    assert_eq!(add(&mut p, finish, "Finish_0", k::CHILD, vec![label]), k::FINISH);
    assert_eq!(add(&mut p, interp, "Interp_Exit", k::PARENT, Vec::new()), k::EXIT_OP);
    let mut ping = if remote_named { vec![p.name_tag("EventName", "Ping")] } else { Vec::new() };
    ping.push(outputs(&mut p, "Out", &[k::PING_ACTION, k::REMOTE_OP]));
    assert_eq!(add(&mut p, remote, "RemoteEvent_0", k::PARENT, ping), k::PING);
    let fire = p.name_tag("EventName", "Ping");
    assert_eq!(add(&mut p, activate, "Activate_0", k::PARENT, vec![fire]), k::PING_ACTION);
    assert_eq!(add(&mut p, interp, "Interp_Remote", k::PARENT, Vec::new()), k::REMOTE_OP);
    let touched = outputs(&mut p, "Touched", &[k::TOUCH_OP]);
    assert_eq!(add(&mut p, touch, "Touch_0", k::PARENT, vec![touched]), k::TOUCH);
    assert_eq!(add(&mut p, interp, "Interp_Touch", k::PARENT, Vec::new()), k::TOUCH_OP);
    assert_eq!(add(&mut p, interp, "Interp_Lone", k::PARENT, Vec::new()), k::LONE_OP);
    // A key: time, value, arrive and leave tangents, mode.
    let key = |p: &mut PackageBuilder, time: f32, value: [f32; 3], leave: [f32; 3], mode: &str| {
        let mut body = p.float_tag("InVal", time);
        body.extend(p.struct_tag("OutVal", "Vector", &vector_bytes(value)));
        body.extend(p.struct_tag("ArriveTangent", "Vector", &vector_bytes([0.0; 3])));
        body.extend(p.struct_tag("LeaveTangent", "Vector", &vector_bytes(leave)));
        body.extend(p.enum_tag("InterpMode", "EInterpCurveMode", mode));
        body.extend_from_slice(&p.none());
        body
    };
    let curve = |p: &mut PackageBuilder, name: &str, keys: Vec<Vec<u8>>, method: &str| {
        let n = i32::try_from(keys.len()).unwrap();
        let mut body = p.array_tag("Points", n, &keys.concat());
        body.extend(p.enum_tag("InterpMethod", "EInterpCurveMethod", method));
        body.extend_from_slice(&p.none());
        p.struct_tag(name, "InterpCurveVector", &body)
    };
    let broken = "IMT_UseBrokenTangentEval";
    let pos_keys = vec![
        key(&mut p, 0.0, [0.0; 3], [1.0, 0.0, 0.0], "CIM_CurveBreak"),
        key(&mut p, 2.0, [0.0; 3], [0.0; 3], "CIM_Linear"),
    ];
    let pos = curve(&mut p, "PosTrack", pos_keys, broken);
    let euler_keys = vec![
        key(&mut p, 0.0, [0.0; 3], [0.0; 3], "CIM_Linear"),
        key(&mut p, 2.0, [0.0, 0.0, 720.0], [0.0; 3], "CIM_Linear"),
    ];
    let euler = curve(&mut p, "EulerTrack", euler_keys, broken);
    let frame = p.enum_tag("MoveFrame", "EInterpTrackMoveFrame", "IMF_RelativeToInitial");
    let rot = p.enum_tag("RotMode", "EInterpTrackMoveRotMode", "IMR_Keyframed");
    assert_eq!(add(&mut p, track, "InterpTrackMove_0", k::PARENT, vec![pos, euler, frame, rot]), k::TRACK);
    let nan_keys = vec![
        key(&mut p, 0.0, [0.0; 3], [0.0; 3], "CIM_Linear"),
        key(&mut p, f32::NAN, [1.0; 3], [0.0; 3], "CIM_Linear"),
        key(&mut p, 2.0, [2.0; 3], [0.0; 3], "CIM_Linear"),
    ];
    let nan_pos = curve(&mut p, "PosTrack", nan_keys, broken);
    let empty = curve(&mut p, "EulerTrack", Vec::new(), broken);
    let frame = p.enum_tag("MoveFrame", "EInterpTrackMoveFrame", "IMF_World");
    let rot = p.enum_tag("RotMode", "EInterpTrackMoveRotMode", "IMR_Keyframed");
    assert_eq!(add(&mut p, track, "InterpTrackMove_NaN", k::PARENT, vec![nan_pos, empty, frame, rot]), k::NAN_TRACK);
    let enter = outputs(&mut p, "Out", &[k::GATED]);
    assert_eq!(add(&mut p, loaded, "LevelLoaded_Entry", k::PARENT, vec![enter]), k::LOAD_ENTRY);
    let done = outputs(&mut p, "Done", &[k::GATED_OP]);
    assert_eq!(add(&mut p, sequence, "Gated", k::PARENT, vec![done]), k::GATED);
    let touched = outputs(&mut p, "Touched", &[k::GATE_FINISH]);
    assert_eq!(add(&mut p, touch, "Touch_Gate", k::GATED, vec![touched]), k::GATE_TOUCH);
    let label = p.str_tag("OutputLabel", "Done");
    assert_eq!(add(&mut p, finish, "Finish_Gate", k::GATED, vec![label]), k::GATE_FINISH);
    assert_eq!(add(&mut p, interp, "Interp_Gated", k::PARENT, Vec::new()), k::GATED_OP);
    p.build()
}

/// A VM over a store of the invented `Engine.upk` and [`kismet_package_with`], whose folder lives as long as the guard
/// (the store opens an object's file again when it loads it).
struct KismetStore {
    vm: Vm,
    package: ow_package::PackageId,
    dir: std::path::PathBuf,
}

impl Drop for KismetStore {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn kismet_store() -> KismetStore {
    kismet_store_with(true)
}

/// [`kismet_store`] over [`engine_with`] and [`kismet_package_with`] of `remote_named`.
fn kismet_store_with(remote_named: bool) -> KismetStore {
    kismet_store_in(remote_named, ParityMode::Off)
}

/// [`kismet_store_with`] with the VM in `parity`.
fn kismet_store_in(remote_named: bool, parity: ParityMode) -> KismetStore {
    let dir = std::env::temp_dir().join(format!(
        "ow-host-kismet-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let (engine, kismet) = (engine_with(remote_named), kismet_package_with(remote_named));
    let files: [(&str, &[u8]); 2] = [("Engine.upk", &engine), ("Test_K.upk", &kismet)];
    let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, parity).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), parity);
    let package = vm.defs.code_mut().1.load(&PackageName::parse("Test_K").unwrap()).unwrap();
    KismetStore { vm, package, dir }
}

/// A builder over a fresh store holding the invented packages, that also moves the pawn to `at` after the load (the
/// invented `Vector` has no tagged form here, so the placement is written, not read).
fn builder(remote_role: u8, at: [f32; 3], yaw: i32) -> Builder {
    builder_with(remote_role, None, false, at, yaw)
}

/// [`builder`] over a map whose `WorldInfo` streams a second level, `Test_Sub`, that places its own `Pawn_0`; the last
/// pawn (the streamed level's) is the one moved to `at`.
fn streaming_builder(at: [f32; 3]) -> Builder {
    builder_files(0, None, false, true, Kismet::None, at, 0)
}

/// [`builder`] over a map that also holds the Kismet exports of [`map`]'s `kismet`.
fn kismet_builder() -> Builder {
    builder_files(0, None, false, false, Kismet::Walk, [0.0; 3], 0)
}

/// [`builder`] with the pawn's `Physics` set to `physics` when given, and the host's physics step installed after
/// the load with Matinee bindings that name the pawn when `bound` (the invented packages hold no Matinee op).
fn builder_with(remote_role: u8, physics: Option<&'static str>, bound: bool, at: [f32; 3], yaw: i32) -> Builder {
    builder_files(remote_role, physics, bound, false, Kismet::None, at, yaw)
}

/// [`builder_with`], with the streamed level `Test_Sub` when `streamed`, and `kismet` in the persistent map (the
/// streamed level holds [`Kismet::RemoteSource`] when it is [`Kismet::RemoteTarget`], else none).
fn builder_files(
    remote_role: u8,
    physics: Option<&'static str>,
    bound: bool,
    streamed: bool,
    kismet: Kismet,
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
        let sub_kismet = if kismet == Kismet::RemoteTarget { Kismet::RemoteSource } else { Kismet::None };
        let map_sub = map(remote_role, physics, None, sub_kismet);
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
        let summary = LoadSummary::from_report("Test_P", ParityMode::Off, &report, list.len());
        let packages = level_packages(&mut world.vm, &summary)?;
        let mut bindings = matinee_bound(&mut world.vm, &packages)?;
        if kismet == Kismet::None {
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
    // R3: the no-op is Off's; the C++ reference has no host physics to stand in.
    let reference = physics_rule("PHYS_Interpolating", c, &Driver::Unbound, ParityMode::Reference).unwrap_err();
    assert!(matches!(reference.missing, Missing::NotPorted), "{reference}");
    // An op the graph does not start at load still answers NotYet (review r1 of #436): the startup paths outside the
    // graph, SetPosition from a script and a touch from a spawn overlap, are not ruled out.
    for touch in [false, true] {
        let n = physics_rule("PHYS_Interpolating", c, &Driver::StartupUnknown { touch }, off).unwrap_err();
        assert!(matches!(n.missing, Missing::NotRead), "{n}");
        assert!(n.to_string().contains("a script calling its SetPosition"), "{n}");
        assert_eq!(n.to_string().contains("a touch from an actor spawned overlapping"), touch, "{n}");
    }
    let incomplete = physics_rule("PHYS_Interpolating", c, &Driver::Incomplete("a reason (x1)".into()), off).unwrap_err();
    assert!(incomplete.to_string().contains("the Kismet graph is incomplete (a reason (x1))"), "{incomplete}");
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
    let (mut world, summary) = kismet_builder()().unwrap();
    let pawn = *world.actors().borrow().list().last().unwrap();
    let packages = level_packages(&mut world.vm, &summary).unwrap();
    let bindings = matinee_bound(&mut world.vm, &packages).unwrap();
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
    // The hand-made graphs' gameplay events are touches.
    g.touch_events = g.gameplay_events.clone();
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
    // Nothing links to the op: the graph does not start it, but a script's SetPosition might (delta review of #436).
    let g = graph(&o, &classes, &[], &[load], &events, &[touch]);
    assert_eq!(start_of(&g, o[op]), Start::StartupUnknown { touch: false });
    // A load event two links up (through an action): it starts at load, and the load event wins over the others.
    let g = graph(&o, &classes, &[(op, &[action, touch]), (action, &[load, mission])], &[load], &events, &[touch]);
    assert_eq!(start_of(&g, o[op]), Start::AtLoad(vec!["Engine.SeqEvent_LevelLoaded".into()]));
    // A mission remote event (fired from outside the graph): named, not assumed quiet.
    let g = graph(&o, &classes, &[(op, &[action]), (action, &[mission])], &[load], &events, &[touch]);
    assert_eq!(start_of(&g, o[op]), Start::Outside(vec!["WillowGame.WillowSeqEvent_MissionRemoteEvent".into()]));
    // Only a touch and an action no link reaches: not at load through the graph, and the start unknown.
    let g = graph(&o, &classes, &[(op, &[touch, dead])], &[load], &events, &[touch]);
    assert_eq!(start_of(&g, o[op]), Start::StartupUnknown { touch: true });
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
    assert_eq!(start_of(&g, o[op]), Start::StartupUnknown { touch: false });
    // A remote event whose only action has no source (a dead sender): still firable from outside (review r1).
    let g = graph(&o, &classes2, &[(op, &[remote]), (remote, &[activate])], &[load], &[load, remote], &[]);
    assert_eq!(start_of(&g, o[op]), Start::Outside(vec!["Engine.SeqEvent_RemoteEvent".into()]));
    // An incomplete graph answers Incomplete for every op, naming its reasons.
    let mut g = graph(&o, &classes, &[], &[load], &events, &[touch]);
    g.incomplete.insert("a link's LinkedOp is missing or not an object".into(), 2);
    assert_eq!(
        start_of(&g, o[op]),
        Start::Incomplete("a link's LinkedOp is missing or not an object (x2)".into())
    );
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
    bindings.starts.insert(a, Start::StartupUnknown { touch: false });
    bindings.starts.insert(b, Start::StartupUnknown { touch: true });
    assert_eq!(Driver::of(&bindings, actor), Driver::StartupUnknown { touch: true });
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

/// The invented Kismet package through `read_graph` (review r1 of #436): the exit out of a subsequence, a remote
/// cycle that stays externally firable, a touch, and an op nothing links to; and an unresolved class elsewhere makes
/// every start Incomplete.
#[test]
fn read_graph_follows_exits_and_keeps_remote_events_external() {
    use matinee::{Start, read_graph, start_of};
    let mut store = kismet_store();
    let package = store.package;
    let vm = &mut store.vm;
    let packages: ow_core::DetSet<_> = [package].into_iter().collect();
    let graph = read_graph(vm, &packages).unwrap();
    assert!(graph.incomplete.is_empty(), "{:?}", graph.incomplete);
    assert_eq!(graph.matinee.len(), 5);
    let op = |vm: &mut Vm, index: i32| vm.defs.resolve_ref(&mut vm.space, package, index).unwrap().unwrap();
    let (exit, remote, touch, lone) = (op(vm, k::EXIT_OP), op(vm, k::REMOTE_OP), op(vm, k::TOUCH_OP), op(vm, k::LONE_OP));
    // The level-loaded event inside the child reaches the parent's op through the finish op and the child's output.
    assert_eq!(start_of(&graph, exit), Start::AtLoad(vec!["Engine.SeqEvent_LevelLoaded".into()]));
    assert!(graph.sources[&exit].contains(&op(vm, k::FINISH)));
    // The remote event has a source in the graph (its own action, a cycle), and stays firable from outside.
    assert!(graph.sources[&op(vm, k::PING)].contains(&op(vm, k::PING_ACTION)));
    assert_eq!(start_of(&graph, remote), Start::Outside(vec!["Engine.SeqEvent_RemoteEvent".into()]));
    assert_eq!(start_of(&graph, touch), Start::StartupUnknown { touch: true });
    assert_eq!(start_of(&graph, lone), Start::StartupUnknown { touch: false });
    // Delta review of #436, finding 3: a load event enters the gated child, but only a touch inside it reaches the
    // finish op, so the op behind the child's output is not started at load. An impulse into a sequence is not one
    // out of it: the sequence is no source of its output's targets, only the finish op is.
    let (gated, gated_op) = (op(vm, k::GATED), op(vm, k::GATED_OP));
    assert_eq!(start_of(&graph, gated_op), Start::StartupUnknown { touch: true });
    assert!(!graph.sources[&gated_op].contains(&gated));
    assert!(graph.sources[&gated_op].contains(&op(vm, k::GATE_FINISH)));
    assert!(graph.sources[&gated].contains(&op(vm, k::LOAD_ENTRY)));
    // Over the map that holds exports of a class that does not resolve, the graph is incomplete.
    let (mut world, summary) = kismet_builder()().unwrap();
    let levels = level_packages(&mut world.vm, &summary).unwrap();
    let graph = read_graph(&mut world.vm, &levels).unwrap();
    assert_eq!(graph.incomplete.get("the export class Engine.NoSuchOp does not resolve"), Some(&2));
    let any = graph.matinee[0];
    assert!(matches!(start_of(&graph, any), Start::Incomplete(_)));
}

/// Delta review of #436, finding 4: a remote event whose `EventName` does not read makes the graph incomplete, by name,
/// as an action's does.
#[test]
fn a_remote_event_whose_name_does_not_read_makes_the_graph_incomplete() {
    use matinee::{Start, read_graph, start_of};
    let mut store = kismet_store_with(false);
    let package = store.package;
    let vm = &mut store.vm;
    let graph = read_graph(vm, &[package].into_iter().collect()).unwrap();
    assert_eq!(graph.incomplete.len(), 1, "{:?}", graph.incomplete);
    assert_eq!(graph.incomplete.get("a remote event with no EventName"), Some(&1));
    let remote = vm.defs.resolve_ref(&mut vm.space, package, k::REMOTE_OP).unwrap().unwrap();
    assert_eq!(start_of(&graph, remote), Start::Incomplete("a remote event with no EventName (x1)".into()));
}

/// Review r3 of #436, finding 1: the delta review's graph rules are Off's. On the gated child (a load event enters
/// it, only a touch reaches its finish op), Off answers the op behind its output `StartupUnknown` (not read), and
/// Reference keeps the pre-delta `AtLoad` (layer absent); an unnamed remote event is a fault in Off only.
#[test]
fn the_delta_graph_rules_are_off_s_and_reference_keeps_the_old_answers() {
    use matinee::{Start, read_graph, start_of};
    use ow_core::notyet::Missing;
    let c = "Engine.InterpActor";
    for parity in [ParityMode::Off, ParityMode::Reference] {
        let mut store = kismet_store_in(true, parity);
        let package = store.package;
        let vm = &mut store.vm;
        let graph = read_graph(vm, &[package].into_iter().collect()).unwrap();
        assert!(graph.incomplete.is_empty(), "{parity:?}: {:?}", graph.incomplete);
        let gated_op = vm.defs.resolve_ref(&mut vm.space, package, k::GATED_OP).unwrap().unwrap();
        let start = start_of(&graph, gated_op);
        let mut bindings = MatineeBindings::default();
        let actor = vm.defs.resolve_ref(&mut vm.space, package, k::TOUCH).unwrap().unwrap();
        bindings.bound.insert(actor);
        bindings.by_actor.insert(actor, [gated_op].into_iter().collect());
        bindings.starts.insert(gated_op, start.clone());
        let refusal = physics_rule("PHYS_Interpolating", c, &Driver::of(&bindings, actor), parity).unwrap_err();
        if parity == ParityMode::Off {
            assert_eq!(start, Start::StartupUnknown { touch: true });
            assert!(matches!(refusal.missing, Missing::NotRead), "{refusal}");
        } else {
            assert_eq!(start, Start::AtLoad(vec!["Engine.SeqEvent_LevelLoaded".into()]));
            assert!(matches!(refusal.missing, Missing::LayerAbsent(_)), "{refusal}");
        }
        let mut unnamed = kismet_store_in(false, parity);
        let package = unnamed.package;
        let graph = read_graph(&mut unnamed.vm, &[package].into_iter().collect()).unwrap();
        assert_eq!(graph.incomplete.is_empty(), parity == ParityMode::Reference, "{parity:?}: {:?}", graph.incomplete);
    }
}

/// Delta review of #436, finding 5: a streamed level that places no actor holds the load event that starts the
/// persistent level's Matinee op (through the remote event `Go`), so the scans must read the loader's levels, not the
/// placed actors' packages.
#[test]
fn a_streamed_level_with_no_placed_actor_changes_the_persistent_level_s_matinee_start() {
    let build = builder_files(0, Some("PHYS_Interpolating"), false, true, Kismet::RemoteTarget, [0.0; 3], 0);
    let (mut world, summary) = build().unwrap();
    assert_eq!(summary.levels.len(), 2, "{}", summary.text());
    assert_eq!((summary.levels[1].0.as_str(), summary.levels[1].2), ("Test_Sub", 0), "the streamed level places nothing");
    let pawn = *world.actors().borrow().list().last().unwrap();
    let levels = level_packages(&mut world.vm, &summary).unwrap();
    assert_eq!(levels.len(), 2);
    let bindings = scan_matinee(&mut world.vm, &levels).unwrap();
    assert!(bindings.complete(), "{bindings:?}");
    assert_eq!(Driver::of(&bindings, pawn), Driver::AtLoad("Engine.SeqEvent_LevelLoaded".into()));
    // Over the persistent level alone (the placed actors' package), the same op reads as fired from outside.
    let persistent = world.vm.defs.code_mut().1.load(&PackageName::parse("Test_P").unwrap()).unwrap();
    let alone = scan_matinee(&mut world.vm, &[persistent].into_iter().collect()).unwrap();
    assert_eq!(Driver::of(&alone, pawn), Driver::Outside("Engine.SeqEvent_RemoteEvent".into()));
}

/// Delta review of #436, finding 2 (the hub's ruling: no rollback): a Matinee refusal mid-tick is terminal. The failed
/// tick's writes stay as they were made (the clock and the `WorldInfo` time fields; the actors' state untouched), the
/// halt says it stopped after partial writes, and the world itself runs no later tick.
#[test]
fn a_matinee_refusal_mid_tick_stops_the_world_after_its_partial_writes() {
    let at = [5.0, 6.0, 7.0];
    let (mut world, _) = builder_with(0, Some("PHYS_Interpolating"), true, at, 0)().unwrap();
    let pawn = *world.actors().borrow().list().last().unwrap();
    let wi = world.actors().borrow().world_info().unwrap();
    let state = |world: &World| {
        let space = &world.vm.space;
        let times = ["TimeSeconds", "RealTimeSeconds", "DeltaSeconds"].map(|f| float(space, wi, f));
        (times, transform(space, pawn).unwrap())
    };
    let mut frame = WorldFrame::default();
    step(&mut world, &mut frame, 0.25);
    let halt = frame.halted.clone().expect("the bound actor's Matinee refusal halts the tick");
    assert!(halt.not_yet && halt.partial_writes, "{halt:?}");
    assert_eq!(halt.tick, 1);
    assert!(halt.message.contains("the Matinee playback that moves a bound interpolating Engine.Pawn"), "{halt:?}");
    // The clock ran and the time fields were written before the actor pass reached the refusal.
    assert_eq!((frame.ticks, frame.time_seconds, frame.delta_seconds), (0, Some(0.25), Some(0.25)));
    let at_stop = state(&world);
    assert_eq!((at_stop.0[0], at_stop.0[2]), (Some(0.25), Some(0.25)));
    assert!(at_stop.0[1].is_some());
    assert_eq!(at_stop.1.0, at, "the refused actor did not move");
    // A later step does nothing: the frame and the world stay as the stop left them.
    let stopped = frame.clone();
    step(&mut world, &mut frame, 0.25);
    assert_eq!(frame, stopped);
    assert_eq!(state(&world), at_stop, "no later tick ran");
}

/// Review r3 of #436, finding 2: a tick refused before it writes (a NaN delta) halts the world for good too, but
/// without the partial-writes mark, and the clock and the time fields are as the load left them.
#[test]
fn a_tick_refused_before_it_writes_halts_without_partial_writes() {
    let at = [5.0, 6.0, 7.0];
    let (mut world, _) = builder_with(0, Some("PHYS_Interpolating"), true, at, 0)().unwrap();
    let pawn = *world.actors().borrow().list().last().unwrap();
    let wi = world.actors().borrow().world_info().unwrap();
    let state = |world: &World| {
        let space = &world.vm.space;
        let times = ["TimeSeconds", "RealTimeSeconds", "DeltaSeconds"].map(|f| float(space, wi, f));
        (world.vm.clock.tick(), times, transform(space, pawn).unwrap())
    };
    let loaded = state(&world);
    let mut frame = WorldFrame::default();
    step(&mut world, &mut frame, f32::NAN);
    let halt = frame.halted.clone().expect("a NaN delta halts the tick");
    assert!(!halt.partial_writes && !halt.not_yet, "{halt:?}");
    assert!(halt.message.contains("the delta must be finite"), "{halt:?}");
    assert_eq!(frame.ticks, 0);
    assert_eq!(state(&world), loaded, "nothing was written");
    // Terminal all the same: a good delta afterwards runs nothing.
    let stopped = frame.clone();
    step(&mut world, &mut frame, 0.25);
    assert_eq!(frame, stopped);
    assert_eq!(state(&world), loaded);
    // Through the link, and in the report: no partial-writes wording.
    let (mut link, _) = WorldLink::spawn(builder(0, [0.0; 3], 0)).unwrap();
    let first = link.tick(f32::NAN);
    let halt = first.halted.clone().expect("the link's first tick halts");
    assert!(!halt.partial_writes, "{halt:?}");
    assert_eq!(link.tick(1.0 / 60.0), first);
    let stats = crate::play::WorldStats { last: first, ..Default::default() };
    let text = crate::play::world_text(&stats);
    assert!(text.contains("play world halted at tick 1 (error): world tick: the delta must be finite"), "{text}");
    assert!(!text.contains("partial writes"), "{text}");
}

/// The invented movement tracks through `read_move` (review r1 of #436): a broken-tangent segment and yaw wound past
/// one turn, and a NaN key time refused while decoding.
#[test]
fn read_move_decodes_the_package_and_refuses_a_nan_key() {
    use matinee::track::{MoveFrame, read_move};
    let mut store = kismet_store();
    let package = store.package;
    let vm = &mut store.vm;
    let track = vm.defs.resolve_ref(&mut vm.space, package, k::TRACK).unwrap().unwrap();
    let m = read_move(&vm.space, track).unwrap();
    assert_eq!(m.frame, MoveFrame::RelativeToInitial);
    assert!(m.position.broken);
    // From 0 to 0 over 2 s with leave tangent 1, broken (not scaled by the 2 s): at 1 s, h10 = 0.125.
    assert_eq!(m.key_pose(1.0).0, [0.125, 0.0, 0.0]);
    // Yaw 720 degrees at 2 s: two turns, 131072 units, not wrapped.
    assert_eq!(m.key_pose(2.0).1.yaw, 131_072);
    assert_eq!(m.key_pose(1.0).1.yaw, 65_536);
    let nan = vm.defs.resolve_ref(&mut vm.space, package, k::NAN_TRACK).unwrap().unwrap();
    let refused = read_move(&vm.space, nan).unwrap_err();
    assert!(refused.to_string().contains("InVal is not finite"), "{refused}");
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
        let (mut world, summary) = build_installed(cooked, "Sanctuary_P".into(), ParityMode::Off).unwrap();
        let list = world.actors().borrow().list().to_vec();
        let packages = level_packages(&mut world.vm, &summary).unwrap();
        let bindings = scan_matinee(&mut world.vm, &packages).unwrap();
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
        let (mut world, summary) = build_installed(cooked, "Sanctuary_P".into(), ParityMode::Off).unwrap();
        let packages = level_packages(&mut world.vm, &summary).unwrap();
        let graph = read_graph(&mut world.vm, &packages).unwrap();
        let mut out = format!("graph incomplete: {:?}\n", graph.incomplete);
        out.push_str(&format!(
            "ops {}, matinee {}, load events {}\n",
            graph.ops.len(),
            graph.matinee.len(),
            graph.load_events.len()
        ));
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
                Start::StartupUnknown { touch } => format!("startup unknown, not at load through the graph (touch {touch})"),
                Start::Incomplete(why) => format!("incomplete: {why}"),
                Start::AtLoad(e) => format!("at load via {e:?}"),
                Start::Outside(e) => format!("from outside the graph via {e:?}"),
            };
            let playing = read(&world.vm.space, op, "bIsPlaying");
            let looping = read(&world.vm.space, op, "bLooping");
            *starts.entry(format!("{key}; bIsPlaying {playing:?} bLooping {looping:?}")).or_default() += 1;
            if matches!(start, Start::StartupUnknown { .. }) {
                let up = matinee::upstream(&graph, op);
                let mut roots: Vec<String> = up
                    .iter()
                    .filter(|o| graph.sources.get(o).is_none_or(|s| s.is_empty()))
                    .map(|o| graph.ops[o].clone())
                    .collect();
                roots.sort();
                roots.dedup();
                *starts.entry(format!("  roots of a startup-unknown op: {roots:?}")).or_default() += 1;
            }
            let kind = match &start {
                Start::StartupUnknown { .. } => "startup-unknown",
                Start::Incomplete(_) => "incomplete",
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
