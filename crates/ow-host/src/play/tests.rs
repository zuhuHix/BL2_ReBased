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
        let play_digests: Vec<_> = run.segments.iter().map(|s| s.3).collect();
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
    assert_eq!(c.take().yaw, ow_core::num::trunc_i32_f64(MAX_TURN_PER_FRAME), "a huge motion is clamped");
}

#[test]
fn the_collector_s_keys() {
    let mut c = InputCollector::new(true);
    c.key(KeyCode::KeyW, true, false, false);
    c.key(KeyCode::KeyD, true, false, false);
    c.key(KeyCode::Space, true, false, false);
    let a = c.take();
    assert!(a.forward && a.right && a.jump && !a.back);
    // The jump is a press: held or repeated, it does not come again.
    c.key(KeyCode::Space, true, true, false);
    let b = c.take();
    assert!(b.forward && !b.jump);
    c.key(KeyCode::F1, true, false, false);
    assert!(c.take().toggle_fly);
    c.key(KeyCode::Escape, true, false, false);
    assert!(!c.wants_grab());
    c.click();
    assert!(c.wants_grab());
    c.focus_lost();
    let lost = c.take();
    assert!(!lost.forward && !lost.right && !c.wants_grab());
    // A scripted run ignores the movement keys but still hears Escape and F1, and never wants the cursor.
    let mut s = InputCollector::new(false);
    assert!(!s.wants_grab());
    s.key(KeyCode::KeyW, true, false, false);
    s.key(KeyCode::F1, true, false, false);
    s.click();
    let t = s.take();
    assert!(!t.forward && t.toggle_fly && !s.wants_grab());
}

#[test]
fn the_field_of_view_is_the_record_s_default() {
    let v = vertical_fov_deg();
    // 2 atan(tan(35 deg) x 0.5625) = 42.996 degrees, 43.00 to two places (review r1 of PR #355 asked).
    assert!((v - 42.995_661).abs() < 1e-5, "{v}");
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

/// Review r1 of PR #355, finding 1: segments of no tick (at the start, the middle and the end) keep their script
/// index, so each play digest lines up with `walk::run`'s for the same segment.
#[test]
fn segments_of_no_tick_keep_their_place() {
    let script = vec![
        Segment::new("nothing first", 0.0, 0.0, 0.0, false, 0),
        Segment::new("run", 1.0, 1.0, 0.0, false, 0),
        // 0.004 s rounds to no tick at 1/60 s.
        Segment::new("a blink", 0.004, 1.0, 0.0, true, 500),
        Segment::new("jump and turn", 1.0, 1.0, 0.0, true, 200),
        Segment::new("nothing last", 0.0, 0.0, 0.0, false, 0),
    ];
    let reference = walk::run(&world(), &mut pawn(), &script, step_seconds(), 1);
    assert_eq!(walk::segment_ticks(&script[2], step_seconds()), 0);
    for frames in [&HEADLESS_FRAMES[..], &[FRAME_60][..]] {
        let run = run_headless(world(), pawn(), script.clone(), frames).unwrap();
        let indices: Vec<usize> = run.segments.iter().map(|s| s.0).collect();
        assert_eq!(indices, vec![0, 1, 2, 3, 4]);
        for ((_, name, _, digest), walked) in run.segments.iter().zip(&reference.segments) {
            assert_eq!(*name, walked.name);
            assert_eq!(*digest, walked.digest, "segment {name}");
        }
        assert_eq!(run.digest, reference.digest);
        // The text numbers each line by its script index.
        let text = stats_text(&run);
        assert!(text.contains("play segment 2 a blink:") && text.contains("play segment 4 nothing last:"), "{text}");
    }
}

/// Finding 3: a press winit makes up when the window regains focus does not walk or jump; a made-up release
/// releases.
#[test]
fn made_up_presses_on_focus_are_ignored() {
    let mut c = InputCollector::new(true);
    c.key(KeyCode::KeyW, true, false, true);
    c.key(KeyCode::Space, true, false, true);
    let a = c.take();
    assert!(!a.forward && !a.jump);
    c.key(KeyCode::KeyW, true, false, false);
    assert!(c.take().forward);
    c.key(KeyCode::KeyW, false, false, true);
    assert!(!c.take().forward);
}

/// Finding 2: the start screenshot is due at its frame, or on a finishing frame before it.
#[test]
fn the_start_shot_is_taken_even_for_a_short_script() {
    assert!(!start_shot_due(30, 29, false));
    assert!(start_shot_due(30, 30, false));
    assert!(start_shot_due(30, 3, true));
}

/// Lane H4-WEAPON: a script that fires gives the same shots (and the same path as `walk::run`, which ignores the
/// trigger) at every frame-time pattern.
#[test]
fn a_firing_script_shoots_alike_whatever_the_frame_times() {
    let firing = vec![
        Segment::new("settle", 0.5, 0.0, 0.0, false, 0),
        Segment::new("fire at the wall", 1.0, 0.0, 0.0, false, 0).firing(),
        Segment::new("walk and fire", 1.0, 1.0, 0.0, false, 100).firing(),
        Segment::new("stand", 0.5, 0.0, 0.0, false, 0),
    ];
    let gun = || crate::weapon::Gun::new(Ok(crate::weapon::WeaponStats::new(20.0, 0.1, Some(1.0), Some(3000.0))));
    let reference = walk::run(&world(), &mut pawn(), &firing, step_seconds(), 1);
    let runs: Vec<PlayStats> = [&HEADLESS_FRAMES[..], &[FRAME_60][..], &[Duration::from_millis(3)][..]]
        .into_iter()
        .map(|frames| run_headless_with(world(), pawn(), firing.clone(), frames, gun(), Vec::new()).unwrap())
        .collect();
    for run in &runs {
        assert_eq!(run.digest, reference.digest, "firing moves nothing");
        assert_eq!(run.shot_digest, runs[0].shot_digest);
        assert_eq!(run.shots.len(), runs[0].shots.len());
    }
    // 2 s of trigger at a 0.1 s interval: 20 shots, all from the trigger segments' steps (31 to 150).
    assert_eq!(runs[0].shots.len(), 20);
    assert!(runs[0].shots.iter().all(|(s, _)| (31..=150).contains(&s.step)));
    // The pawn faces yaw 2000, toward the wall at x 900: the first shot hits it.
    let first = &runs[0].shots[0].0;
    let hit = first.hit.expect("the wall");
    assert!((hit.point[0] - 900.0).abs() < 0.1, "{hit:?}");
    assert!(stats_text(&runs[0]).contains("play shot 0 step 31"));
    // The script text keeps the trigger.
    let text = walk::script_text(&firing);
    assert_eq!(walk::parse_script(&text).unwrap(), firing);
    assert!(text.lines().nth(1).unwrap().contains(" f "));
    assert_eq!(walk::parse_script("1 0 0 jf 0 both").unwrap()[0].fire, true);
    assert!(walk::parse_script("1 0 0 x 0 bad").is_err());
}

/// The player's trigger: the left button fires only while the cursor is held, and never while flying.
#[test]
fn the_left_button_fires_once_the_cursor_is_held() {
    let mut c = InputCollector::new(true);
    c.left_button(true);
    assert!(!c.take().fire, "the click that takes the cursor does not fire");
    c.set_look(true);
    c.left_button(true);
    assert!(c.take().fire);
    c.left_button(false);
    assert!(!c.take().fire);
    // L2: a click shorter than a frame still reaches the message, once.
    c.left_button(true);
    c.left_button(false);
    assert!(c.take().fire);
    assert!(!c.take().fire);
    // L1: letting go of the cursor (Escape) lets go of a held trigger.
    c.left_button(true);
    c.set_look(false);
    assert!(!c.take().fire);
    let mut scripted = InputCollector::new(false);
    scripted.set_look(true);
    scripted.left_button(true);
    assert!(!scripted.take().fire);

    let stats = Rc::new(RefCell::new(PlayStats::default()));
    let gun = crate::weapon::Gun::new(Ok(crate::weapon::WeaponStats::new(5.0, 0.5, None, None)));
    let mut sim = PlayWorld::new(world(), pawn(), None, Rc::clone(&stats)).with_gun(gun, Vec::new(), false);
    for _ in 0..30 {
        sim.step(&[], FRAME_60).unwrap();
    }
    let fire = PlayInput { fire: true, ..PlayInput::default() };
    let snap = sim.step(&[HostCommand::Play(fire)], FRAME_60).unwrap();
    let shot = snap.shot.expect("a shot");
    assert!(shot.this_frame && shot.damage_known && shot.impact.is_some());
    // Held for a second at 0.5 s: shots on the first step and 30 steps later.
    for _ in 0..59 {
        sim.step(&[HostCommand::Play(fire)], FRAME_60).unwrap();
    }
    assert_eq!(stats.borrow().shots.len(), 2);
    // While flying the trigger does nothing.
    let toggle = PlayInput { toggle_fly: true, fire: true, ..PlayInput::default() };
    for _ in 0..60 {
        sim.step(&[HostCommand::Play(PlayInput { toggle_fly: false, ..toggle })], FRAME_60).unwrap();
    }
    let before = stats.borrow().shots.len();
    sim.step(&[HostCommand::Play(toggle)], FRAME_60).unwrap();
    for _ in 0..60 {
        sim.step(&[HostCommand::Play(fire)], FRAME_60).unwrap();
    }
    assert_eq!(stats.borrow().shots.len(), before);
}

/// Review r1 of PR #366, L2: a trigger message that arrives on a frame too short for a step, and is let go by the next
/// message, still fires one shot on the next step.
#[test]
fn a_press_shorter_than_a_step_fires_once() {
    let stats = Rc::new(RefCell::new(PlayStats::default()));
    let gun = crate::weapon::Gun::new(Ok(crate::weapon::WeaponStats::new(5.0, 0.5, None, None)));
    let mut sim = PlayWorld::new(world(), pawn(), None, Rc::clone(&stats)).with_gun(gun, Vec::new(), false);
    for _ in 0..30 {
        sim.step(&[], FRAME_60).unwrap();
    }
    let fire = PlayInput { fire: true, ..PlayInput::default() };
    // A 1 ms frame: no step runs, the trigger is latched.
    sim.step(&[HostCommand::Play(fire)], Duration::from_millis(1)).unwrap();
    assert_eq!(stats.borrow().shots.len(), 0);
    // Let go in the next message, with a whole step of time: the latched press fires once.
    sim.step(&[HostCommand::Play(PlayInput::default())], FRAME_60).unwrap();
    assert_eq!(stats.borrow().shots.len(), 1);
    for _ in 0..60 {
        sim.step(&[HostCommand::Play(PlayInput::default())], FRAME_60).unwrap();
    }
    assert_eq!(stats.borrow().shots.len(), 1, "the latch is used once");
}

/// An invented world (lane H-WORLD): it records every delta it is ticked by, keeps an f32 clock, walks its one pawn
/// 1 uu along X per tick, and from tick `halt_at` on answers a halt as the real link does.
struct FakeWorld {
    deltas: Rc<RefCell<Vec<f32>>>,
    frame: crate::world::WorldFrame,
    halt_at: u64,
}

impl crate::world::WorldTick for FakeWorld {
    fn tick(&mut self, dt: f32) -> crate::world::WorldFrame {
        if self.frame.halted.is_some() {
            return self.frame.clone();
        }
        self.deltas.borrow_mut().push(dt);
        let tick = self.frame.ticks + 1;
        if tick >= self.halt_at {
            self.frame.halted =
                Some(crate::world::Halt {
                    tick,
                    message: "an invented NotYet".into(),
                    not_yet: true,
                    partial_writes: true,
                });
            return self.frame.clone();
        }
        self.frame.ticks = tick;
        self.frame.time_seconds = Some(self.frame.time_seconds.unwrap_or(0.0) + dt);
        self.frame.delta_seconds = Some(dt);
        for p in &mut self.frame.pawns {
            p.location[0] += 1.0;
        }
        self.frame.clone()
    }
}

fn placed_pawn() -> crate::world::PawnActor {
    crate::world::PawnActor {
        path: "TheWorld.PersistentLevel.Pawn_0".into(),
        class: "Engine.Pawn".into(),
        archetype: None,
        location: [100.0, 0.0, 50.0],
        rotation: Rotator::new(0, 16384, 0),
    }
}

fn on_fake_world(frames: &[Duration], halt_at: u64) -> (PlayStats, Vec<f32>) {
    let deltas = Rc::new(RefCell::new(Vec::new()));
    let fake = FakeWorld {
        deltas: Rc::clone(&deltas),
        frame: crate::world::WorldFrame { pawns: vec![placed_pawn()], ..Default::default() },
        halt_at,
    };
    let gun = Gun::new(Err(crate::weapon::equipped_weapon()));
    let loaded: LoadedWorld = (Box::new(fake), vec![placed_pawn()]);
    let stats = run_headless_on(world(), pawn(), script(), frames, gun, Vec::new(), Some(loaded)).unwrap();
    let deltas = deltas.borrow().clone();
    (stats, deltas)
}

#[test]
fn the_world_is_ticked_once_per_step_by_the_step_whatever_the_frame_times() {
    let plain = run_headless(world(), pawn(), script(), &HEADLESS_FRAMES).unwrap();
    let (uneven, uneven_deltas) = on_fake_world(&HEADLESS_FRAMES, u64::MAX);
    let (steady, _) = on_fake_world(&[FRAME_60], u64::MAX);
    // One tick per step, each by the step's length, never by a frame time.
    assert_eq!(uneven_deltas.len() as u64, uneven.steps);
    assert!(uneven_deltas.iter().all(|&d| d == DEFAULT_STEP_SECONDS));
    let (u, s) = (uneven.world.as_ref().unwrap(), steady.world.as_ref().unwrap());
    assert_eq!((u.requested, u.last.ticks), (uneven.steps, uneven.steps));
    // The world's digest is a function of the script, as the path's is; the world does not change the path.
    assert_eq!(u.digest, s.digest);
    assert_eq!(u.trace, s.trace);
    assert_eq!((uneven.digest, uneven.segments.clone()), (plain.digest, plain.segments.clone()));
    assert_eq!(u.last.pawns[0].location[0], 100.0 + uneven.steps as f32);
    let text = stats_text(&uneven);
    assert!(text.contains("play world halted: no"), "{text}");
    assert!(plain.world.is_none() && !stats_text(&plain).contains("play world"));
}

#[test]
fn a_halted_world_stops_ticking_and_the_walk_goes_on() {
    let plain = run_headless(world(), pawn(), script(), &HEADLESS_FRAMES).unwrap();
    let (run, deltas) = on_fake_world(&HEADLESS_FRAMES, 10);
    assert!(run.finished);
    assert_eq!(run.digest, plain.digest);
    let w = run.world.as_ref().unwrap();
    assert_eq!(w.last.ticks, 9);
    assert_eq!(deltas.len(), 10, "the halting tick is the last one the world sees");
    assert_eq!(w.requested, run.steps);
    let text = stats_text(&run);
    assert!(text.contains("play world halted at tick 10 (NotYet, stopped after partial writes): an invented NotYet"), "{text}");
}

#[test]
fn the_snapshot_carries_the_world_s_pawns() {
    let stats = Rc::new(RefCell::new(PlayStats::default()));
    let fake = FakeWorld {
        deltas: Rc::new(RefCell::new(Vec::new())),
        frame: crate::world::WorldFrame { pawns: vec![placed_pawn()], ..Default::default() },
        halt_at: u64::MAX,
    };
    let mut sim = PlayWorld::new(world(), pawn(), Some(script()), Rc::clone(&stats)).with_world(Box::new(fake), vec![placed_pawn()]);
    // Before any step: the pawns as loaded.
    let first = sim.step(&[], Duration::ZERO).unwrap();
    assert_eq!(first.pawns.as_ref().unwrap()[0].location, [100.0, 0.0, 50.0]);
    let second = sim.step(&[], FRAME_60 * 2).unwrap();
    assert_eq!(second.step, 2);
    assert_eq!(second.pawns.as_ref().unwrap()[0].location, [102.0, 0.0, 50.0]);
    // Without a world, no pawns go out.
    let mut bare = PlayWorld::new(world(), pawn(), Some(script()), Rc::new(RefCell::new(PlayStats::default())));
    assert_eq!(bare.step(&[], FRAME_60).unwrap().pawns, None);
}
