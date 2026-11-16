//! Invented geometry only (AGENTS.md rule 9): a floor and a wall placed by hand.

use super::*;
use crate::movement::collide::Brush;
use crate::movement::mover::{MovementParams, Physics};
use crate::movement::walk::{self, SANCTUARY_SCRIPT};
use winit::keyboard::KeyCode;

fn cuboid(lo: V3, hi: V3, source: u32) -> Brush {
    let mut points = Vec::new();
    for i in 0..8 {
        points.push([
            if i & 4 == 0 { lo[0] } else { hi[0] },
            if i & 2 == 0 { lo[1] } else { hi[1] },
            if i & 1 == 0 { lo[2] } else { hi[2] },
        ]);
    }
    Brush::from_hull(&points, &[], source).expect("a box")
}

/// A floor at z 0, a wall at x 900 and a 30 uu step at y 300..600.
fn world() -> CollisionWorld {
    CollisionWorld::new(vec![
        cuboid([-5000.0, -5000.0, -100.0], [5000.0, 5000.0, 0.0], 0),
        cuboid([900.0, -5000.0, 0.0], [1000.0, 5000.0, 400.0], 1),
        cuboid([-5000.0, 300.0, 0.0], [5000.0, 600.0, 30.0], 2),
    ])
}

fn pawn() -> Mover {
    Mover::spawn(MovementParams::maya(), [0.0, 0.0, 120.0], Rotator::new(0, 2000, 0))
}

fn script() -> Vec<Segment> {
    vec![
        Segment::new("settle", 0.5, 0.0, 0.0, false, 0),
        Segment::new("run", 1.5, 1.0, 0.0, false, 0),
        Segment::new("jump and turn", 2.0, 1.0, 0.4, true, 150),
        Segment::new("into the wall", 2.5, 1.0, -0.5, false, -300),
        Segment::new("stand", 0.5, 0.0, 0.0, false, 0),
    ]
}

const FRAME_60: Duration = Duration::from_micros(16_667);

#[test]
fn a_script_gives_the_walk_s_path_whatever_the_frame_times() {
    let reference = walk::run(&world(), &mut pawn(), &script(), step_seconds(), 1);
    let uneven = run_headless(world(), pawn(), script(), &HEADLESS_FRAMES).unwrap();
    let steady = run_headless(world(), pawn(), script(), &[FRAME_60]).unwrap();
    let tiny = run_headless(world(), pawn(), script(), &[Duration::from_millis(3)]).unwrap();
    for run in [&uneven, &steady, &tiny] {
        assert!(run.finished);
        assert_eq!(run.steps, reference.ticks);
        assert_eq!(run.digest, reference.digest);
        assert_eq!(run.end, reference.end);
        let walk_digests: Vec<_> = reference.segments.iter().map(|s| s.digest).collect();
        let play_digests: Vec<_> = run.segments.iter().map(|s| s.2).collect();
        assert_eq!(play_digests, walk_digests);
    }
    // The uneven pattern really dropped time and took several steps in some frames; the 3 ms one took at most one.
    assert!(uneven.dropped_steps > 0 && uneven.max_steps_in_frame == MAX_STEPS_PER_FRAME);
    assert_eq!(tiny.max_steps_in_frame, 1);
    assert_ne!(uneven.frames, steady.frames);
}

#[test]
fn a_long_frame_takes_at_most_the_step_limit_and_drops_the_rest() {
    let stats = Rc::new(RefCell::new(PlayStats::default()));
    let mut sim = PlayWorld::new(world(), pawn(), None, Rc::clone(&stats));
    sim.step(&[], Duration::from_secs(1)).unwrap();
    let s = stats.borrow();
    assert_eq!(s.steps, u64::from(MAX_STEPS_PER_FRAME));
    // 60 steps' worth of time: 8 taken, 52 dropped (within a rounding step).
    assert!((51..=52).contains(&s.dropped_steps), "{}", s.dropped_steps);
}

#[test]
fn the_view_is_the_eye_between_the_last_two_steps() {
    let stats = Rc::new(RefCell::new(PlayStats::default()));
    let mut sim = PlayWorld::new(world(), pawn(), None, Rc::clone(&stats));
    // Settle onto the floor, then run.
    for _ in 0..60 {
        sim.step(&[], FRAME_60).unwrap();
    }
    let run = PlayInput { forward: true, ..PlayInput::default() };
    for _ in 0..30 {
        sim.step(&[HostCommand::Play(run)], FRAME_60).unwrap();
    }
    // Half a step of time: no step is taken and the view is halfway between the last two eyes.
    let before = sim.prev_eye;
    let after = sim.mover().eye();
    let snap = sim.step(&[HostCommand::Play(run)], Duration::from_secs_f64(step_seconds() * 0.5)).unwrap();
    let view = snap.view.unwrap();
    let alpha = sim.accumulator / step_seconds();
    assert!(alpha > 0.0 && alpha < 1.0, "{alpha}");
    for i in 0..3 {
        let expected = (before[i] + (after[i] - before[i]) * alpha) as f32;
        assert_eq!(view.eye[i], expected);
    }
    assert!(after[0] > before[0], "running forward");
    assert_eq!(sim.mover().physics, Physics::Walking);
    assert!(!view.fly);
}

#[test]
fn the_player_s_input_moves_turns_and_jumps_the_pawn() {
    let stats = Rc::new(RefCell::new(PlayStats::default()));
    let mut sim = PlayWorld::new(world(), pawn(), None, Rc::clone(&stats));
    for _ in 0..60 {
        sim.step(&[], FRAME_60).unwrap();
    }
    let start = sim.mover().location;
    // A turn arrives: the view shows it at once, the pawn takes it on its next step.
    let turn = PlayInput { yaw: 4096, pitch: -1000, ..PlayInput::default() };
    let snap = sim.step(&[HostCommand::Play(turn)], Duration::ZERO).unwrap();
    assert_eq!(snap.view.unwrap().rotation, Rotator::new(-1000, 6096, 0));
    assert_eq!(sim.mover().view.yaw, 2000);
    sim.step(&[], FRAME_60).unwrap();
    assert_eq!((sim.mover().view.yaw, sim.mover().view.pitch), (6096, -1000));
    // Hold W for a second: forward along the new yaw.
    let w = PlayInput { forward: true, ..PlayInput::default() };
    for _ in 0..60 {
        sim.step(&[HostCommand::Play(w)], FRAME_60).unwrap();
    }
    let moved = [sim.mover().location[0] - start[0], sim.mover().location[1] - start[1]];
    assert!(moved[0] > 200.0 && moved[1] > 200.0, "{moved:?}");
    // Space: one press, one jump.
    let jump = PlayInput { jump: true, ..PlayInput::default() };
    sim.step(&[HostCommand::Play(jump)], FRAME_60).unwrap();
    assert_eq!(sim.mover().physics, Physics::Falling);
    assert!(sim.mover().velocity[2] > 600.0);
}

#[test]
fn f1_flies_a_camera_and_leaves_the_pawn_still() {
    let stats = Rc::new(RefCell::new(PlayStats::default()));
    let mut sim = PlayWorld::new(world(), pawn(), None, Rc::clone(&stats));
    for _ in 0..60 {
        sim.step(&[], FRAME_60).unwrap();
    }
    let toggle = PlayInput { toggle_fly: true, ..PlayInput::default() };
    let snap = sim.step(&[HostCommand::Play(toggle)], FRAME_60).unwrap();
    assert!(snap.view.unwrap().fly);
    let pawn_at = sim.mover().location;
    let w = PlayInput { forward: true, up: true, ..PlayInput::default() };
    let mut view = snap.view.unwrap();
    for _ in 0..30 {
        view = sim.step(&[HostCommand::Play(w)], FRAME_60).unwrap().view.unwrap();
    }
    assert_eq!(sim.mover().location, pawn_at, "the pawn stays");
    assert!(view.eye[2] > snap.view.unwrap().eye[2] + 100.0, "the camera rose");
    let back = sim.step(&[HostCommand::Play(toggle)], FRAME_60).unwrap();
    assert!(!back.view.unwrap().fly);
    assert_eq!(stats.borrow().fly_toggles, 2);
}

#[test]
fn the_collector_turns_pixels_into_whole_units_and_keeps_the_rest() {
    let mut c = InputCollector::new(true);
    assert!(c.wants_grab());
    // No grab yet: motion does not turn.
    c.mouse_motion(10.0, 0.0);
    assert_eq!(c.take().yaw, 0);
    c.set_look(true);
    c.mouse_motion(1.03, -0.5);
    let first = c.take();
    // 1.03 x 20 = 20.6 units of yaw: 20 now, 0.6 kept; mouse up 0.5 px is pitch up 10.
    assert_eq!((first.yaw, first.pitch), (20, 10));
    c.mouse_motion(0.03, 0.0);
    assert_eq!(c.take().yaw, 1);
    c.mouse_motion(f64::NAN, 1e30);
    let wild = c.take();
    assert_eq!((wild.yaw, wild.pitch), (0, 0), "a NaN motion is ignored whole");
    c.mouse_motion(1e30, 0.0);
    assert_eq!(c.take().yaw, MAX_TURN_PER_FRAME as i32, "a huge motion is clamped");
}

#[test]
fn the_collector_s_keys() {
    let mut c = InputCollector::new(true);
    c.key(KeyCode::KeyW, true, false);
    c.key(KeyCode::KeyD, true, false);
    c.key(KeyCode::Space, true, false);
    let a = c.take();
    assert!(a.forward && a.right && a.jump && !a.back);
    // The jump is a press: held or repeated, it does not come again.
    c.key(KeyCode::Space, true, true);
    let b = c.take();
    assert!(b.forward && !b.jump);
    c.key(KeyCode::F1, true, false);
    assert!(c.take().toggle_fly);
    c.key(KeyCode::Escape, true, false);
    assert!(!c.wants_grab());
    c.click();
    assert!(c.wants_grab());
    c.focus_lost();
    let lost = c.take();
    assert!(!lost.forward && !lost.right && !c.wants_grab());
    // A scripted run ignores the movement keys but still hears Escape and F1, and never wants the cursor.
    let mut s = InputCollector::new(false);
    assert!(!s.wants_grab());
    s.key(KeyCode::KeyW, true, false);
    s.key(KeyCode::F1, true, false);
    s.click();
    let t = s.take();
    assert!(!t.forward && t.toggle_fly && !s.wants_grab());
}

#[test]
fn the_field_of_view_is_the_record_s_default() {
    let v = vertical_fov_deg();
    assert!((v - 43.0).abs() < 0.1, "{v}");
    let view = ViewPose { eye: [0.0; 3], rotation: Rotator::default(), fly: false };
    let m = view_projection(&view, 1280, 720).unwrap();
    let ahead = m.apply([500.0, 0.0, 0.0, 1.0]);
    assert!(ahead[3] > 0.0 && (ahead[0] / ahead[3]).abs() < 1e-6);
    assert!(view_projection(&view, 0, 720).is_none());
}

#[test]
fn the_sanctuary_script_reads_back_from_its_text() {
    let text = walk::script_text(&SANCTUARY_SCRIPT);
    assert_eq!(walk::parse_script(&text).unwrap(), SANCTUARY_SCRIPT.to_vec());
    assert!(walk::parse_script("1.0 2 0 0 0 too fast").is_err());
    assert!(walk::parse_script("1.0 1 0 yes 0 x").is_err());
    assert!(walk::parse_script("-1 1 0 0 0 x").is_err());
    assert_eq!(walk::parse_script("# a comment\n\n0.5 0 0 0 0\n").unwrap().len(), 1);
}
