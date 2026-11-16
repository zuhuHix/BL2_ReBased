//! Owner: lane H7-ANIMTREE. Synthetic trees on invented values (AGENTS.md rule 9), and one ignored test that writes
//! the installed-game report under `local/`.

use ow_assets::animation::BoundClip;
use ow_assets::skeletal::{Bone, BoneTransform, SkeletalMesh};
use ow_core::OrderedMap;

use super::data::{Child, Kind, Node, NodeId, Selector, SequenceParams, TreeDef};
use super::eval::{
    ClipInfo, ClipLibrary, Driver, Ending, EvalError, Inputs, Notify, Play, Unsupported,
    directional,
};
use super::graph::{GraphError, MAX_DEPTH, check};
use super::load::{TreeError, same_file};
use crate::anim::pose::{Local, Skeleton, Wrap};

const ID: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
const S: f32 = std::f32::consts::FRAC_1_SQRT_2;

/// Two bones: Root at 0, B1 at (10, 0, 0).
fn skeleton() -> Skeleton {
    let bones = vec![
        Bone::new("Root", None, BoneTransform::new(ID, [0.0; 3])),
        Bone::new("B1", Some(0), BoneTransform::new(ID, [10.0, 0.0, 0.0])),
    ];
    Skeleton::new(&SkeletalMesh::new(
        bones,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    ))
    .unwrap()
}

/// A clip that holds B1 at `rotation` and `translation` for `frames` frames at 10 fps; Root stays at the bind.
fn hold(name: &str, frames: usize, rotation: [f32; 4], translation: [f32; 3]) -> ClipInfo {
    let root = vec![BoneTransform::new(ID, [0.0; 3]); frames];
    let b1 = vec![BoneTransform::new(rotation, translation); frames];
    ClipInfo::new(
        BoundClip::new(name, 10.0, frames, vec![Some(root), Some(b1)]),
        1.0,
        Vec::new(),
    )
}

/// B1's translation moves from x = 10 to x = 10 + frames - 1, one uu a frame, at 10 fps.
fn slide(name: &str, frames: usize) -> ClipInfo {
    let root = vec![BoneTransform::new(ID, [0.0; 3]); frames];
    let b1 = (0..frames)
        .map(|k| BoneTransform::new(ID, [10.0 + f32::from(u16::try_from(k).unwrap()), 0.0, 0.0]))
        .collect();
    ClipInfo::new(
        BoundClip::new(name, 10.0, frames, vec![Some(root), Some(b1)]),
        1.0,
        Vec::new(),
    )
}

fn seq(clip: &str, looping: bool) -> Kind {
    Kind::Sequence(SequenceParams::new(
        Some(clip.to_string()),
        1.0,
        looping,
        true,
    ))
}

fn node(path: &str, kind: Kind, children: &[(&str, Option<NodeId>)]) -> Node {
    let children = children.iter().map(|(n, c)| Child::new(*n, *c)).collect();
    Node::new(path, "Test.Node", kind, children)
}

fn tree(nodes: Vec<(NodeId, Node)>) -> TreeDef {
    let map: OrderedMap<NodeId, Node> = nodes.into_iter().collect();
    TreeDef::new("Test.Tree", 1, map)
}

fn library(clips: Vec<ClipInfo>) -> ClipLibrary {
    let mut lib = ClipLibrary::new();
    lib.add_set("Test", clips);
    lib
}

fn b1(locals: &[Local]) -> Local {
    locals[1]
}

fn close(a: f64, b: f64, eps: f64) -> bool {
    (a - b).abs() < eps
}

/// Root -> a list on the posture input over three held clips: A (B1 at x 10), B (x 20), C (x 40).
fn list_tree(blend_time: f32) -> TreeDef {
    tree(vec![
        (1, node("T.Root", Kind::Root, &[("Child", Some(2))])),
        (
            2,
            node(
                "T.Posture",
                Kind::List {
                    selector: Selector::Posture,
                    blend_time: Some(blend_time),
                },
                &[
                    ("Standing", Some(3)),
                    ("Crouched", Some(4)),
                    ("Sprinting", Some(5)),
                ],
            ),
        ),
        (3, node("T.A", seq("A", true), &[])),
        (4, node("T.B", seq("B", true), &[])),
        (5, node("T.C", seq("C", true), &[])),
    ])
}

fn list_library() -> ClipLibrary {
    library(vec![
        hold("A", 4, ID, [10.0, 0.0, 0.0]),
        hold("B", 4, ID, [20.0, 0.0, 0.0]),
        hold("C", 4, ID, [40.0, 0.0, 0.0]),
    ])
}

#[test]
fn blend_weights_sum_to_one_and_ramp_linearly() {
    let (t, lib, sk) = (list_tree(0.2), list_library(), skeleton());
    let mut d = Driver::new(&t, &lib, &sk, Inputs::default()).unwrap();
    assert!(close(b1(&d.locals().unwrap()).translation[0], 10.0, 1e-9));
    d.inputs.select.insert("posture".into(), 2);
    for step in 1..=4 {
        d.tick(0.05).unwrap();
        let w = d.effective_weights();
        let leaves: f64 = [3, 4, 5].iter().filter_map(|i| w.get(i)).sum();
        assert!(close(leaves, 1.0, 1e-12), "{leaves}");
        // A falls and C rises linearly over 0.2 s: x = 10 + 30 * fraction.
        let fraction = f64::from(step) * 0.25;
        let x = b1(&d.locals().unwrap()).translation[0];
        // 1e-5: the blend time is an f32 0.2, which is not exactly 0.2 in f64.
        assert!(close(x, 10.0 + 30.0 * fraction, 1e-5), "{step}: {x}");
    }
    // A three-way switch while blending: the weights still sum to 1 after normalisation.
    d.inputs.select.insert("posture".into(), 1);
    d.tick(0.05).unwrap();
    let w = d.effective_weights();
    let leaves: f64 = [3, 4, 5].iter().filter_map(|i| w.get(i)).sum();
    assert!(close(leaves, 1.0, 1e-12));
}

#[test]
fn the_additive_delta_on_the_identity_base_is_the_delta() {
    // Base: B1 unrotated at x 0 (the identity local). Add: B1 turned 90 degrees about Z, moved by (1, 2, 3).
    let t = tree(vec![
        (1, node("T.Root", Kind::Root, &[("Child", Some(2))])),
        (
            2,
            node(
                "T.Add",
                Kind::Additive { child2_weight: 1.0 },
                &[("Base", Some(3)), ("Add", Some(4))],
            ),
        ),
        (3, node("T.Base", seq("Base", true), &[])),
        (4, node("T.Delta", seq("ADD_Delta", true), &[])),
    ]);
    let lib = library(vec![
        hold("Base", 2, ID, [0.0; 3]),
        hold("ADD_Delta", 2, [0.0, 0.0, S, S], [1.0, 2.0, 3.0]),
    ]);
    let sk = skeleton();
    let mut d = Driver::new(&t, &lib, &sk, Inputs::default()).unwrap();
    let out = b1(&d.locals().unwrap());
    assert!(close(out.translation[0], 1.0, 1e-9) && close(out.translation[2], 3.0, 1e-9));
    let s = f64::from(S);
    for (k, want) in [0.0, 0.0, s, s].iter().enumerate() {
        assert!(close(out.rotation[k], *want, 1e-6), "{k}");
    }
    // At weight 0.5: half the translation, half the angle (45 degrees).
    let mut half = t.clone();
    if let Some(n) = half.nodes.get_mut(&2) {
        n.kind = Kind::Additive { child2_weight: 0.5 };
    }
    let mut d = Driver::new(&half, &lib, &sk, Inputs::default()).unwrap();
    let out = b1(&d.locals().unwrap());
    assert!(close(out.translation[1], 1.0, 1e-9));
    let angle = 2.0 * ow_core::num::acos_f64(out.rotation[3]);
    assert!(close(angle, std::f64::consts::FRAC_PI_4, 1e-6), "{angle}");
    // A missing additive clip adds nothing (not the bind pose).
    let lib = library(vec![hold("Base", 2, ID, [5.0, 0.0, 0.0])]);
    let mut d = Driver::new(&t, &lib, &sk, Inputs::default()).unwrap();
    assert!(close(b1(&d.locals().unwrap()).translation[0], 5.0, 1e-9));
    assert!(d.log.missing_clips.contains("ADD_Delta"));
}

/// Root -> slot (source: a held idle; one free channel).
fn slot_tree() -> TreeDef {
    tree(vec![
        (1, node("T.Root", Kind::Root, &[("Child", Some(2))])),
        (
            2,
            node(
                "T.Slot",
                Kind::Slot,
                &[("Source", Some(3)), ("Channel 0", None)],
            ),
        ),
        (3, node("T.Idle", seq("Idle", true), &[])),
    ])
}

#[test]
fn a_slot_plays_once_then_blends_back() {
    // Idle holds B1 at x 10; the one-shot holds it at x 30 for 0.4 s (4 frames at 10 fps), blends 0.1 in and out.
    let lib = library(vec![
        hold("Idle", 4, ID, [10.0, 0.0, 0.0]),
        hold("Shot", 4, ID, [30.0, 0.0, 0.0]),
    ]);
    let (t, sk) = (slot_tree(), skeleton());
    let mut d = Driver::new(&t, &lib, &sk, Inputs::default()).unwrap();
    d.play("Slot", Play::once("Shot", 0.1, 0.1)).unwrap();
    let x = |d: &mut Driver<'_>| b1(&d.locals().unwrap()).translation[0];
    assert!(close(x(&mut d), 10.0, 1e-9), "starts on the source");
    d.tick(0.05).unwrap();
    assert!(close(x(&mut d), 20.0, 1e-5), "half way in");
    d.tick(0.15).unwrap();
    assert!(close(x(&mut d), 30.0, 1e-5), "fully in");
    d.tick(0.15).unwrap();
    assert!(
        close(x(&mut d), 20.0, 1e-5),
        "half way out, 0.05 s before the end"
    );
    assert!(d.is_playing("Slot"));
    // 0.06, not 0.05: the steps so far sum to just under 0.35 in f64.
    d.tick(0.06).unwrap();
    assert!(!d.is_playing("Slot"), "done at its end");
    assert!(close(x(&mut d), 10.0, 1e-9), "back on the source");
    assert_eq!(
        d.play("Nope", Play::once("Shot", 0.1, 0.1)),
        Err(EvalError::NoSuchNode("Nope".into()))
    );
    assert_eq!(
        d.play("Slot", Play::once("Nope", 0.1, 0.1)),
        Err(EvalError::NoSuchClip("Nope".into()))
    );
}

#[test]
fn special_moves_take_the_recorded_blend_rules() {
    let t = tree(vec![
        (1, node("T.Root", Kind::Root, &[("Child", Some(2))])),
        (
            2,
            node(
                "T.Move",
                Kind::SpecialMove,
                &[("Normal", Some(3)), ("Child1", None)],
            )
            .with_name("WeaponAction"),
        ),
        (3, node("T.Idle", seq("Idle", true), &[])),
    ]);
    let lib = library(vec![
        hold("Idle", 4, ID, [10.0, 0.0, 0.0]),
        hold("Move", 4, ID, [30.0, 0.0, 0.0]),
    ]);
    let sk = skeleton();
    let x = |d: &mut Driver<'_>| b1(&d.locals().unwrap()).translation[0];
    // OnAnimEnd drops the blend-out: fully on the move until its last moment.
    let mut d = Driver::new(&t, &lib, &sk, Inputs::default()).unwrap();
    d.play(
        "WeaponAction",
        Play::once("Move", 0.1, 0.2).with_ending(Ending::OnAnimEnd),
    )
    .unwrap();
    d.tick(0.35).unwrap();
    assert!(close(x(&mut d), 30.0, 1e-9));
    // A clip (0.4 s) shorter than blend-in plus blend-out (0.3 + 0.3) plays with no blend at all.
    let mut d = Driver::new(&t, &lib, &sk, Inputs::default()).unwrap();
    d.play("WeaponAction", Play::once("Move", 0.3, 0.3))
        .unwrap();
    assert!(close(x(&mut d), 30.0, 1e-9));
    // Loop never ends.
    let mut d = Driver::new(&t, &lib, &sk, Inputs::default()).unwrap();
    d.play(
        "WeaponAction",
        Play::once("Move", 0.0, 0.0).with_ending(Ending::Loop),
    )
    .unwrap();
    d.tick(5.0).unwrap();
    assert!(d.is_playing("WeaponAction"));
}

#[test]
fn a_cycle_a_dangling_link_and_a_deep_chain_are_refused() {
    let mut t = list_tree(0.2);
    if let Some(n) = t.nodes.get_mut(&3) {
        n.children.push(Child::new("Back", Some(2)));
    }
    assert_eq!(check(&t), Err(GraphError::Cycle { at: 2 }));
    let mut t = list_tree(0.2);
    if let Some(n) = t.nodes.get_mut(&3) {
        n.children.push(Child::new("Lost", Some(99)));
    }
    assert_eq!(check(&t), Err(GraphError::Dangling { from: 3, to: 99 }));
    // A chain of pass-through nodes one deeper than the budget.
    let depth = i32::try_from(MAX_DEPTH).unwrap() + 1;
    let nodes: Vec<(NodeId, Node)> = (1..=depth)
        .map(|i| {
            let next = if i < depth { Some(i + 1) } else { None };
            (
                i,
                node(
                    "T.N",
                    Kind::ScaleRate { base_speed: 1.0 },
                    &[("Input", next)],
                ),
            )
        })
        .collect();
    assert!(matches!(
        check(&tree(nodes)),
        Err(GraphError::TooDeep { .. })
    ));
    // The shared child of a diamond is fine.
    let mut t = list_tree(0.2);
    if let Some(n) = t.nodes.get_mut(&2) {
        n.children[1].node = Some(3);
    }
    assert!(check(&t).is_ok());
}

#[test]
fn a_single_clip_path_equals_the_sampler() {
    let t = slot_tree();
    let lib = library(vec![slide("Idle", 5)]);
    let sk = skeleton();
    let mut d = Driver::new(&t, &lib, &sk, Inputs::default()).unwrap();
    d.tick(0.137).unwrap();
    let time = d.node_time(3).unwrap();
    let tree_pose = sk.pose_from_locals(d.locals().unwrap()).unwrap();
    let direct = sk
        .sample(&lib.find("Idle").unwrap().1.clip, time, Wrap::Loop)
        .unwrap();
    for (a, b) in tree_pose.model.iter().zip(&direct.model) {
        for k in 0..3 {
            assert!(close(a.translation[k], b.translation[k], 1e-12));
        }
    }
    assert!(close(time, 0.137, 1e-12));
}

#[test]
fn ticks_are_deterministic_over_dt() {
    let lib = list_library();
    let (t, sk) = (list_tree(0.2), skeleton());
    let mut coarse = Driver::new(&t, &lib, &sk, Inputs::default()).unwrap();
    let mut fine = Driver::new(&t, &lib, &sk, Inputs::default()).unwrap();
    coarse.inputs.select.insert("posture".into(), 1);
    fine.inputs.select.insert("posture".into(), 1);
    coarse.tick(0.1).unwrap();
    for _ in 0..10 {
        fine.tick(0.01).unwrap();
    }
    let (a, b) = (b1(&coarse.locals().unwrap()), b1(&fine.locals().unwrap()));
    assert!(
        close(a.translation[0], b.translation[0], 1e-9),
        "{} {}",
        a.translation[0],
        b.translation[0]
    );
    // The same steps give the same bits.
    let mut again = Driver::new(&t, &lib, &sk, Inputs::default()).unwrap();
    again.inputs.select.insert("posture".into(), 1);
    again.tick(0.1).unwrap();
    assert_eq!(b1(&again.locals().unwrap()), a);
    assert_eq!(again.tick(-1.0), Err(EvalError::BadStep));
    assert_eq!(again.tick(f64::NAN), Err(EvalError::BadStep));
}

#[test]
fn not_yet_nodes_are_refused_or_passed_through_and_counted() {
    let t = tree(vec![
        (1, node("T.Root", Kind::Root, &[("Child", Some(2))])),
        (
            2,
            node(
                "T.Camera",
                Kind::NotYet { reason: "unread" },
                &[("Source", Some(3)), ("Add", Some(3))],
            ),
        ),
        (3, node("T.Idle", seq("Idle", true), &[])),
    ]);
    let lib = library(vec![hold("Idle", 2, ID, [10.0, 0.0, 0.0])]);
    let sk = skeleton();
    let mut d = Driver::new(&t, &lib, &sk, Inputs::default()).unwrap();
    match d.locals() {
        Err(EvalError::NotYet(n)) => {
            assert!(n.what.contains("Test.Node") && n.what.contains("T.Camera"))
        }
        other => panic!("{other:?}"),
    }
    d.unsupported = Unsupported::PassFirstChild;
    assert!(close(b1(&d.locals().unwrap()).translation[0], 10.0, 1e-9));
    assert_eq!(d.log.passed_through.get(&2), Some(&1));
}

#[test]
fn directional_weights_fit_the_stored_ones() {
    // The installed hands node stores DirAngle -0.1396 rad (8 degrees left) with Forward 0.9111, Left 0.0889.
    let w = directional(-0.139626339, 4);
    assert!(
        close(w[0], 0.911111, 1e-6) && close(w[2], 0.088889, 1e-6) && w[1] == 0.0 && w[3] == 0.0
    );
    let w = directional(0.056435965, 4);
    assert!(close(w[3], 0.035928, 1e-6));
    let w = directional(std::f64::consts::PI, 4);
    assert!(close(w[1], 1.0, 1e-12));
    for a in [-3.0, -1.0, 0.0, 0.5, 2.0] {
        assert!(close(directional(a, 4).iter().sum(), 1.0, 1e-12));
    }
}

#[test]
fn speed_bands_scale_rates_and_report_notifies() {
    // Speed bands [0, 10, 250): idle below 10 uu/s, walk above; the walk sits under a ScaleRate of base 100.
    let t = tree(vec![
        (1, node("T.Root", Kind::Root, &[("Child", Some(2))])),
        (
            2,
            node(
                "T.Speed",
                Kind::Speed {
                    constraints: vec![0.0, 10.0, 250.0],
                    blend_up: 0.0,
                    blend_down: 0.0,
                },
                &[("Standing", Some(3)), ("Walking", Some(4))],
            ),
        ),
        (3, node("T.Idle", seq("Idle", true), &[])),
        (
            4,
            node(
                "T.Scale",
                Kind::ScaleRate { base_speed: 100.0 },
                &[("Input", Some(5))],
            ),
        ),
        (5, node("T.Walk", seq("Walk", true), &[])),
    ]);
    let mut walk = slide("Walk", 10);
    walk.notifies = vec![Notify::new(0.5, "Test.Footstep")];
    let mut idle = hold("Idle", 10, ID, [10.0, 0.0, 0.0]);
    idle.notifies = vec![Notify::new(0.5, "Test.IdleNotify")];
    let lib = library(vec![idle, walk]);
    let sk = skeleton();
    let mut d = Driver::new(&t, &lib, &sk, Inputs::default()).unwrap();
    d.inputs.speed = 200.0;
    // At 200 uu/s over base 100 the walk plays at twice its rate: 0.3 s of ticks is 0.6 s of clip.
    d.tick(0.3).unwrap();
    assert!(close(d.node_time(5).unwrap(), 0.6, 1e-12));
    assert!(close(b1(&d.locals().unwrap()).translation[0], 16.0, 1e-9));
    // The walk's footstep at 0.5 s was crossed while the walk had weight; the idle's notify was not reported
    // (the idle ticks too, but had no weight).
    let names: Vec<&str> = d
        .log
        .notifies
        .iter()
        .map(|e| e.notify.what.as_str())
        .collect();
    assert_eq!(names, vec!["Test.Footstep"]);
}

/// The installed-game report: `OW_H7_COOKED` is the install's `CookedPCConsole`, `OW_H7_EXPORTS` a `;`-separated
/// list of UModel export folders for `GD_Siren_Streaming_SF`, `OW_H7_OUT` the JSON file to write (under `local/`).
/// It fails if the report does (a non-finite pose, or the single-clip check above 1e-4 uu).
#[test]
#[ignore = "needs the installed game and UModel exports under local/"]
fn installed_report() {
    let cooked = std::env::var_os("OW_H7_COOKED").expect("set OW_H7_COOKED");
    let roots: Vec<std::path::PathBuf> = std::env::var("OW_H7_EXPORTS")
        .expect("set OW_H7_EXPORTS")
        .split(';')
        .map(std::path::PathBuf::from)
        .collect();
    let out = std::env::var_os("OW_H7_OUT").expect("set OW_H7_OUT");
    let text = super::report::run(std::path::Path::new(&cooked), &roots).unwrap();
    std::fs::write(out, text).unwrap();
}

// ---------------------------------------------------------------------------------------------- review r1 of PR #354

/// A clip for the two-bone skeleton with one transform per bone held for `frames` frames at 10 fps; `None` leaves
/// the bone untracked.
fn clip2(
    name: &str,
    frames: usize,
    root: Option<([f32; 4], [f32; 3])>,
    b1: Option<([f32; 4], [f32; 3])>,
) -> ClipInfo {
    let track =
        |t: Option<([f32; 4], [f32; 3])>| t.map(|(r, p)| vec![BoneTransform::new(r, p); frames]);
    ClipInfo::new(
        BoundClip::new(name, 10.0, frames, vec![track(root), track(b1)]),
        1.0,
        Vec::new(),
    )
}

/// Same up to sign, entry by entry within `eps`.
fn same_rotation(a: [f64; 4], b: [f64; 4], eps: f64) -> bool {
    let plus = (0..4).all(|k| close(a[k], b[k], eps));
    let minus = (0..4).all(|k| close(a[k], -b[k], eps));
    plus || minus
}

/// Base: Root turned 90 degrees about Z, B1 turned 90 degrees about Z at (5, 0, 0). Delta: both bones turned 90
/// degrees about X, B1 moved by (1, 2, 3).
fn rotated_base_and_delta() -> ClipLibrary {
    let x90 = [S, 0.0, 0.0, S];
    let z90 = [0.0, 0.0, S, S];
    library(vec![
        clip2(
            "Base",
            2,
            Some((z90, [0.0; 3])),
            Some((z90, [5.0, 0.0, 0.0])),
        ),
        clip2(
            "ADD_Delta",
            2,
            Some((x90, [0.0; 3])),
            Some((x90, [1.0, 2.0, 3.0])),
        ),
    ])
}

/// What the additive rule gives on that base: for B1 (a child) the delta after the base, Z90 * X90 =
/// (0.5, 0.5, 0.5, 0.5), and (5, 0, 0) + (1, 2, 3); for the root the delta before the base, X90 * Z90 =
/// (0.5, -0.5, 0.5, 0.5). A replace rule would give X90 and (1, 2, 3); a blend rule something between.
fn assert_rotated_additive(out: &[Local]) {
    assert!(
        same_rotation(out[1].rotation, [0.5, 0.5, 0.5, 0.5], 1e-6),
        "{:?}",
        out[1].rotation
    );
    assert!(close(out[1].translation[0], 6.0, 1e-6));
    assert!(close(out[1].translation[1], 2.0, 1e-6));
    assert!(close(out[1].translation[2], 3.0, 1e-6));
    assert!(
        same_rotation(out[0].rotation, [0.5, -0.5, 0.5, 0.5], 1e-6),
        "{:?}",
        out[0].rotation
    );
}

#[test]
fn the_additive_delta_on_a_rotated_and_moved_base() {
    let t = tree(vec![
        (1, node("T.Root", Kind::Root, &[("Child", Some(2))])),
        (
            2,
            node(
                "T.Add",
                Kind::Additive { child2_weight: 1.0 },
                &[("Base", Some(3)), ("Add", Some(4))],
            ),
        ),
        (3, node("T.Base", seq("Base", true), &[])),
        (4, node("T.Delta", seq("ADD_Delta", true), &[])),
    ]);
    let (lib, sk) = (rotated_base_and_delta(), skeleton());
    let mut d = Driver::new(&t, &lib, &sk, Inputs::default()).unwrap();
    assert_rotated_additive(&d.locals().unwrap());
}

#[test]
fn the_sequence_add_clip_path_applies_the_same_delta() {
    // WillowAnimNodeSequenceAdditiveBlend: one node playing Base with ADD_Delta on top.
    let mut p = SequenceParams::new(Some("Base".into()), 1.0, true, true);
    p.add_clip = Some("ADD_Delta".into());
    let t = tree(vec![
        (1, node("T.Root", Kind::Root, &[("Child", Some(2))])),
        (2, node("T.SeqAdd", Kind::Sequence(p), &[])),
    ]);
    let (lib, sk) = (rotated_base_and_delta(), skeleton());
    let mut d = Driver::new(&t, &lib, &sk, Inputs::default()).unwrap();
    assert_rotated_additive(&d.locals().unwrap());
}

#[test]
fn an_untracked_bone_of_an_additive_clip_adds_nothing() {
    // B1's bind translation is (10, 0, 0). The additive clip tracks only the root, so B1 must stay at the base's
    // (10, 0, 0), not (20, 0, 0) from the bind pose added as a delta.
    let t = tree(vec![
        (1, node("T.Root", Kind::Root, &[("Child", Some(2))])),
        (
            2,
            node(
                "T.Add",
                Kind::Additive { child2_weight: 1.0 },
                &[("Base", Some(3)), ("Add", Some(4))],
            ),
        ),
        (3, node("T.Base", seq("Base", true), &[])),
        (4, node("T.Delta", seq("ADD_RootOnly", true), &[])),
    ]);
    let lib = library(vec![
        clip2(
            "Base",
            2,
            Some((ID, [0.0; 3])),
            Some((ID, [10.0, 0.0, 0.0])),
        ),
        clip2("ADD_RootOnly", 2, Some((ID, [0.0, 0.0, 1.0])), None),
    ]);
    let sk = skeleton();
    let mut d = Driver::new(&t, &lib, &sk, Inputs::default()).unwrap();
    let out = d.locals().unwrap();
    assert!(
        close(out[1].translation[0], 10.0, 1e-9),
        "{:?}",
        out[1].translation
    );
    assert!(close(out[0].translation[2], 1.0, 1e-9));
}

/// Three bones in a chain: Root, B1 under it, B2 under B1.
fn skeleton3() -> Skeleton {
    let bones = vec![
        Bone::new("Root", None, BoneTransform::new(ID, [0.0; 3])),
        Bone::new("B1", Some(0), BoneTransform::new(ID, [10.0, 0.0, 0.0])),
        Bone::new("B2", Some(1), BoneTransform::new(ID, [10.0, 0.0, 0.0])),
    ];
    Skeleton::new(&SkeletalMesh::new(
        bones,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    ))
    .unwrap()
}

/// Every bone of the three-bone skeleton held at (`x`, 0, 0).
fn all_at(name: &str, x: f32) -> ClipInfo {
    let track = Some(vec![BoneTransform::new(ID, [x, 0.0, 0.0]); 2]);
    ClipInfo::new(
        BoundClip::new(name, 10.0, 2, vec![track.clone(), track.clone(), track]),
        1.0,
        Vec::new(),
    )
}

#[test]
fn per_bone_blends_mask_the_branch_and_log_a_missing_bone() {
    let per_bone = |branches: &[&str], w: f32| {
        tree(vec![
            (1, node("T.Root", Kind::Root, &[("Child", Some(2))])),
            (
                2,
                node(
                    "T.Split",
                    Kind::PerBone {
                        branches: branches.iter().map(|b| b.to_string()).collect(),
                        child2_weight: w,
                    },
                    &[("Source", Some(3)), ("Target", Some(4))],
                ),
            ),
            (3, node("T.Source", seq("Source", true), &[])),
            (4, node("T.Target", seq("Target", true), &[])),
        ])
    };
    let lib = library(vec![all_at("Source", 1.0), all_at("Target", 3.0)]);
    let sk = skeleton3();
    let xs = |t: &TreeDef| {
        let mut d = Driver::new(t, &lib, &sk, Inputs::default()).unwrap();
        let out = d.locals().unwrap();
        (
            out.iter().map(|l| l.translation[0]).collect::<Vec<_>>(),
            d.log.missing_bones.clone(),
        )
    };
    // The branch from B1 takes the target: B1 and B2 (below it), not the root.
    let (x, missing) = xs(&per_bone(&["B1"], 1.0));
    assert!(
        close(x[0], 1.0, 1e-9) && close(x[1], 3.0, 1e-9) && close(x[2], 3.0, 1e-9),
        "{x:?}"
    );
    assert!(missing.is_empty());
    // At weight 0.5 the branch is half way.
    let (x, _) = xs(&per_bone(&["B1"], 0.5));
    assert!(
        close(x[0], 1.0, 1e-9) && close(x[1], 2.0, 1e-9) && close(x[2], 2.0, 1e-9),
        "{x:?}"
    );
    // A branch bone the skeleton lacks is logged and masks nothing; the other branch still applies.
    let (x, missing) = xs(&per_bone(&["Nope", "B2"], 1.0));
    assert!(
        close(x[0], 1.0, 1e-9) && close(x[1], 1.0, 1e-9) && close(x[2], 3.0, 1e-9),
        "{x:?}"
    );
    assert!(missing.contains("Nope"));
}

#[test]
fn the_back_half_of_directional_mirrors_the_front() {
    let quarter = std::f64::consts::FRAC_PI_2;
    // 135 degrees right: half backward, half right. 135 degrees left: half backward, half left.
    let w = directional(1.5 * quarter, 4);
    assert!(
        close(w[1], 0.5, 1e-12) && close(w[3], 0.5, 1e-12) && w[0] == 0.0 && w[2] == 0.0,
        "{w:?}"
    );
    let w = directional(-1.5 * quarter, 4);
    assert!(close(w[1], 0.5, 1e-12) && close(w[2], 0.5, 1e-12), "{w:?}");
    // Straight back from either side, and exactly sideways.
    assert!(close(directional(-std::f64::consts::PI, 4)[1], 1.0, 1e-12));
    let w = directional(quarter, 4);
    assert!(close(w[3], 1.0, 1e-12) && close(w[0], 0.0, 1e-12) && close(w[1], 0.0, 1e-12));
}

#[test]
fn stop_on_last_frame_drops_the_blend_out() {
    let t = tree(vec![
        (1, node("T.Root", Kind::Root, &[("Child", Some(2))])),
        (
            2,
            node(
                "T.Move",
                Kind::SpecialMove,
                &[("Normal", Some(3)), ("Child1", None)],
            )
            .with_name("WeaponAction"),
        ),
        (3, node("T.Idle", seq("Idle", true), &[])),
    ]);
    // A 1 s move (10 frames at 10 fps).
    let lib = library(vec![
        hold("Idle", 4, ID, [10.0, 0.0, 0.0]),
        hold("Move", 10, ID, [30.0, 0.0, 0.0]),
    ]);
    let sk = skeleton();
    let x = |d: &mut Driver<'_>| b1(&d.locals().unwrap()).translation[0];
    let mut d = Driver::new(&t, &lib, &sk, Inputs::default()).unwrap();
    d.play(
        "WeaponAction",
        Play::once("Move", 0.1, 0.4).with_ending(Ending::StopOnLastFrame),
    )
    .unwrap();
    d.tick(0.9).unwrap();
    // 0.1 s before the end: with a 0.4 s blend-out it would be 3/4 of the way back; StopOnLastFrame keeps it full.
    assert!(close(x(&mut d), 30.0, 1e-9));
    d.tick(0.2).unwrap();
    assert!(!d.is_playing("WeaponAction"));
    assert!(close(x(&mut d), 10.0, 1e-9));
}

#[test]
fn rate_scale_speeds_up_sequences_and_one_shots() {
    // A 10-frame slide at 10 fps whose RateScale is 2: the export's rate already holds it (10 fps), so the clip's own
    // length is 2 s, and it plays 2 clip seconds per real second.
    let mut fast = slide("Fast", 10);
    fast.rate_scale = 2.0;
    let t = tree(vec![
        (1, node("T.Root", Kind::Root, &[("Child", Some(2))])),
        (
            2,
            node(
                "T.Slot",
                Kind::Slot,
                &[("Source", Some(3)), ("Channel 0", None)],
            ),
        ),
        (3, node("T.Fast", seq("Fast", true), &[])),
    ]);
    let lib = library(vec![fast, hold("Shot", 10, ID, [30.0, 0.0, 0.0])]);
    let sk = skeleton();
    let mut d = Driver::new(&t, &lib, &sk, Inputs::default()).unwrap();
    d.tick(0.1).unwrap();
    // 0.1 real seconds: 0.2 clip seconds, sampled at 0.1 s of the export: frame 1, x = 11.
    assert!(close(d.node_time(3).unwrap(), 0.2, 1e-12));
    assert!(close(b1(&d.locals().unwrap()).translation[0], 11.0, 1e-9));
    // A one-shot of a RateScale-2 clip ends after half its clip length in real time; its blend times stay real
    // seconds (0.1 s in: half way at 0.05 s).
    let mut shot = hold("Shot2", 10, ID, [30.0, 0.0, 0.0]);
    shot.rate_scale = 2.0;
    let lib2 = library(vec![hold("Idle", 4, ID, [10.0, 0.0, 0.0]), shot]);
    let t2 = slot_tree();
    let mut d = Driver::new(&t2, &lib2, &sk, Inputs::default()).unwrap();
    d.play("Slot", Play::once("Shot2", 0.1, 0.0)).unwrap();
    d.tick(0.05).unwrap();
    assert!(close(b1(&d.locals().unwrap()).translation[0], 20.0, 1e-5));
    d.tick(0.9).unwrap();
    assert!(d.is_playing("Slot"));
    d.tick(0.1).unwrap();
    assert!(
        !d.is_playing("Slot"),
        "a 2 s clip at RateScale 2 ends after 1 s"
    );
}

#[test]
fn bad_requests_and_inputs_are_refused_and_loops_can_stop() {
    let lib = library(vec![
        hold("Idle", 4, ID, [10.0, 0.0, 0.0]),
        hold("Shot", 4, ID, [30.0, 0.0, 0.0]),
    ]);
    let (t, sk) = (slot_tree(), skeleton());
    let mut d = Driver::new(&t, &lib, &sk, Inputs::default()).unwrap();
    let mut nan = Play::once("Shot", 0.1, 0.1);
    nan.rate = f32::NAN;
    assert!(matches!(d.play("Slot", nan), Err(EvalError::BadPlay(_))));
    let mut zero = Play::once("Shot", 0.1, 0.1);
    zero.rate = 0.0;
    assert!(matches!(d.play("Slot", zero), Err(EvalError::BadPlay(_))));
    assert!(matches!(
        d.play("Slot", Play::once("Shot", f32::INFINITY, 0.1)),
        Err(EvalError::BadPlay(_))
    ));
    // A loop runs until stopped.
    d.play(
        "Slot",
        Play::once("Shot", 0.0, 0.0).with_ending(Ending::Loop),
    )
    .unwrap();
    d.tick(10.0).unwrap();
    assert!(d.is_playing("Slot"));
    assert_eq!(d.stop("Slot"), Ok(true));
    assert!(!d.is_playing("Slot"));
    assert_eq!(d.stop("Slot"), Ok(false));
    assert_eq!(d.stop("Nope"), Err(EvalError::NoSuchNode("Nope".into())));
    // Inputs that are not finite.
    d.inputs.speed = f32::NAN;
    assert_eq!(d.tick(0.1), Err(EvalError::BadInput("speed")));
    d.inputs.speed = 0.0;
    d.inputs.direction = f32::INFINITY;
    assert_eq!(d.tick(0.1), Err(EvalError::BadInput("direction")));
    let inputs = Inputs {
        direction: f32::NAN,
        ..Inputs::default()
    };
    assert_eq!(
        Driver::new(&t, &lib, &sk, inputs).err(),
        Some(EvalError::BadInput("direction"))
    );
}

#[test]
fn a_node_from_another_file_is_refused() {
    assert!(same_file("GD_A_SF", "gd_a_sf", "X.Node").is_ok());
    assert_eq!(
        same_file("GD_A_SF", "GD_B_SF", "X.Node"),
        Err(TreeError::OtherFile {
            node: "X.Node".into(),
            file: "GD_B_SF".into(),
            tree_file: "GD_A_SF".into()
        })
    );
}

#[test]
fn a_resumed_driver_continues_where_it_stopped() {
    // Saving the state and resuming it each tick (as the play window's arms do) equals one driver ticked throughout.
    let lib = list_library();
    let (t, sk) = (list_tree(0.2), skeleton());
    let mut inputs = Inputs::default();
    inputs.select.insert("posture".into(), 2);
    let mut whole = Driver::new(&t, &lib, &sk, inputs.clone()).unwrap();
    let mut saved = Driver::new(&t, &lib, &sk, inputs.clone()).unwrap().into_state();
    for _ in 0..7 {
        whole.tick(0.03).unwrap();
        let mut d = Driver::resume(&t, &lib, &sk, inputs.clone(), saved).unwrap();
        d.tick(0.03).unwrap();
        assert_eq!(b1(&d.locals().unwrap()), b1(&whole.locals().unwrap()));
        assert_eq!(d.elapsed(), whole.elapsed());
        saved = d.into_state();
    }
    let mut bad = inputs;
    bad.speed = f32::NAN;
    assert_eq!(Driver::resume(&t, &lib, &sk, bad, saved).err(), Some(EvalError::BadInput("speed")));
}
