//! Invented geometry only (AGENTS.md rule 9): boxes, wedges and triangles placed by hand.

use super::collide::{Brush, CollisionWorld, SKIN, Shape};
use super::geom::V3;
use super::mover::{MoveInput, MovementParams, Mover, Physics};
use super::walk::{self, Segment};
use ow_core::units::Rotator;

const DT: f64 = 1.0 / 60.0;
const FLOOR_TOP: f64 = 0.0;
/// Where Maya's centre rests on a flat floor: half height and the trace skin above it.
const REST: f64 = FLOOR_TOP + 80.0 + SKIN;

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

fn floor() -> Brush {
    cuboid([-5000.0, -5000.0, FLOOR_TOP - 100.0], [5000.0, 5000.0, FLOOR_TOP], 0)
}

/// A wedge rising along +X from `x0` over `run` uu to `rise` uu, 2000 uu wide.
fn wedge(x0: f64, run: f64, rise: f64) -> Brush {
    let points = [
        [x0, -1000.0, FLOOR_TOP],
        [x0, 1000.0, FLOOR_TOP],
        [x0 + run, -1000.0, FLOOR_TOP],
        [x0 + run, 1000.0, FLOOR_TOP],
        [x0 + run, -1000.0, FLOOR_TOP + rise],
        [x0 + run, 1000.0, FLOOR_TOP + rise],
    ];
    Brush::from_hull(&points, &[], 1).expect("a wedge")
}

fn standing(world: &CollisionWorld, at: [f64; 2], yaw: i32) -> Mover {
    let mut m = Mover::spawn(MovementParams::maya(), [at[0], at[1], REST + 20.0], Rotator::new(0, yaw, 0));
    for _ in 0..120 {
        m.tick(world, &MoveInput::default(), DT);
    }
    assert_eq!(m.physics, Physics::Walking, "settled on the floor");
    m
}

fn hold(world: &CollisionWorld, m: &mut Mover, input: MoveInput, seconds: f64) {
    for _ in 0..(seconds / DT).round() as u32 {
        m.tick(world, &input, DT);
    }
}

const FORWARD: MoveInput = MoveInput { forward: 1.0, strafe: 0.0, jump: false, yaw: 0, pitch: 0 };

fn close(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() <= tolerance
}

#[test]
fn a_flat_floor_holds_the_pawn_and_it_runs_at_ground_speed() {
    let world = CollisionWorld::new(vec![floor()]);
    let mut m = standing(&world, [0.0, 0.0], 0);
    assert!(close(m.location[2], REST, 1e-6), "rests at {}", m.location[2]);
    hold(&world, &mut m, FORWARD, 1.0);
    // AccelRate 2048 with friction 8 reaches 440 in about 0.2 s (section 4).
    assert!(close(m.velocity[0], 440.0, 1e-6), "speed {}", m.velocity[0]);
    assert!(m.location[0] > 380.0 && m.location[0] < 440.0, "x {}", m.location[0]);
    assert!(close(m.location[2], REST, 1e-6));
    assert_eq!(m.physics, Physics::Walking);
}

#[test]
fn releasing_the_input_brakes_to_a_stop() {
    let world = CollisionWorld::new(vec![floor()]);
    let mut m = standing(&world, [0.0, 0.0], 0);
    hold(&world, &mut m, FORWARD, 1.0);
    let x = m.location[0];
    hold(&world, &mut m, MoveInput::default(), 0.5);
    assert_eq!(m.velocity, [0.0; 3], "snapped to 0 below 10 uu/s");
    assert!(m.location[0] - x < 60.0, "braked within {} uu", m.location[0] - x);
}

#[test]
fn a_step_at_the_step_height_is_climbed_and_one_above_it_is_not() {
    for (height, climbs) in [(20.0, true), (35.0, true), (36.0, false), (60.0, false)] {
        let step = cuboid([200.0, -1000.0, FLOOR_TOP], [1000.0, 1000.0, FLOOR_TOP + height], 1);
        let world = CollisionWorld::new(vec![floor(), step]);
        let mut m = standing(&world, [0.0, 0.0], 0);
        hold(&world, &mut m, FORWARD, 1.5);
        if climbs {
            assert!(m.location[0] > 300.0, "{height}: x {}", m.location[0]);
            assert!(close(m.location[2], REST + height, 1e-6), "{height}: z {}", m.location[2]);
        } else {
            assert!(m.location[0] < 200.0 - 42.0 + 1e-6, "{height}: x {}", m.location[0]);
            assert!(close(m.location[2], REST, 1e-6), "{height}: z {}", m.location[2]);
        }
        assert_eq!(m.physics, Physics::Walking);
    }
}

#[test]
fn a_ramp_at_or_below_the_walkable_angle_is_walked_and_a_steeper_one_is_not() {
    // WalkableFloorZ 0.7: the steepest walkable slope is about 45.6 degrees. Rises over a run of 400.
    for (rise, walkable) in [(300.0, true), (400.0, true), (420.0, false), (600.0, false)] {
        let nz = 400.0 / ow_core::num::sqrt_f64(400.0 * 400.0 + rise * rise);
        assert_eq!(nz >= 0.7, walkable, "fixture {rise}");
        let world = CollisionWorld::new(vec![floor(), wedge(200.0, 400.0, rise)]);
        let mut m = standing(&world, [0.0, 0.0], 0);
        hold(&world, &mut m, FORWARD, 1.0);
        if walkable {
            assert!(m.location[2] > REST + 100.0, "{rise}: climbed to {}", m.location[2]);
            assert!(m.location[0] > 300.0, "{rise}: x {}", m.location[0]);
        } else {
            assert!(m.location[2] < REST + 35.0 + 1e-6, "{rise}: z {}", m.location[2]);
            assert!(m.location[0] < 200.0, "{rise}: x {}", m.location[0]);
        }
    }
}

#[test]
fn a_wall_is_slid_along() {
    // A wall along X at y = 150; walking at 45 degrees into it keeps the X part of the move.
    let wall = cuboid([-1000.0, 150.0, FLOOR_TOP], [3000.0, 300.0, FLOOR_TOP + 300.0], 1);
    let world = CollisionWorld::new(vec![floor(), wall]);
    let mut m = standing(&world, [0.0, 0.0], 8192);
    hold(&world, &mut m, FORWARD, 2.0);
    assert!(close(m.location[1], 150.0 - 42.0 - SKIN, 0.01), "y {}", m.location[1]);
    assert!(m.location[0] > 400.0, "slid along x to {}", m.location[0]);
    assert!(close(m.location[2], REST, 1e-6));
}

#[test]
fn a_corner_stops_the_pawn_without_jitter() {
    let wall_y = cuboid([-1000.0, 150.0, FLOOR_TOP], [300.0, 300.0, FLOOR_TOP + 300.0], 1);
    let wall_x = cuboid([150.0, -1000.0, FLOOR_TOP], [300.0, 300.0, FLOOR_TOP + 300.0], 2);
    let world = CollisionWorld::new(vec![floor(), wall_y, wall_x]);
    let mut m = standing(&world, [0.0, 0.0], 8192);
    hold(&world, &mut m, FORWARD, 2.0);
    let a = m.location;
    hold(&world, &mut m, FORWARD, 0.5);
    assert_eq!(a, m.location, "held still in the corner");
    assert!(close(a[0], 150.0 - 42.0 - SKIN, 0.01) && close(a[1], 150.0 - 42.0 - SKIN, 0.01), "{a:?}");
}

#[test]
fn a_fall_lands_and_a_jump_follows_the_recorded_arc() {
    let world = CollisionWorld::new(vec![floor()]);
    let mut m = Mover::spawn(MovementParams::maya(), [0.0, 0.0, REST + 1000.0], Rotator::default());
    let mut landed = None;
    for _ in 0..600 {
        if let Some(v) = m.tick(&world, &MoveInput::default(), DT).landed {
            landed = Some(v);
            break;
        }
    }
    let v = landed.expect("landed");
    // 1000 uu of fall: 1000 uu/s without the drag, about 811 with it.
    assert!(v < -800.0 && v > -820.0, "landing speed {v}");
    assert!(close(m.location[2], REST, 1e-6));

    // A standing jump: JumpZ 630, gravity -500, falling drag 0.3 per second (sections 7 and 13). Without the drag
    // the apex would be 396.9 uu and the flight 2.52 s; with it about 318 and 2.27 s (the record's host note).
    let mut apex = m.location[2];
    let mut air = 0.0;
    let mut events = m.tick(&world, &MoveInput { jump: true, ..MoveInput::default() }, DT);
    assert!(events.left_ground);
    air += DT;
    while events.landed.is_none() {
        events = m.tick(&world, &MoveInput::default(), DT);
        apex = apex.max(m.location[2]);
        air += DT;
        assert!(air < 5.0, "never landed");
    }
    let rise = apex - REST;
    assert!(rise > 310.0 && rise < 325.0, "rise {rise}");
    assert!(air > 2.2 && air < 2.35, "air {air}");
    assert_eq!(m.physics, Physics::Walking);
}

#[test]
fn walking_off_a_ledge_falls_and_lands_below() {
    let high = cuboid([-1000.0, -1000.0, 0.0], [100.0, 1000.0, 300.0], 0);
    let low = cuboid([-1000.0, -1000.0, -100.0], [3000.0, 1000.0, 0.0], 1);
    let world = CollisionWorld::new(vec![high, low]);
    let mut m = Mover::spawn(MovementParams::maya(), [0.0, 0.0, 300.0 + 80.0 + SKIN + 5.0], Rotator::default());
    hold(&world, &mut m, MoveInput::default(), 1.0);
    assert!(close(m.location[2], 380.0 + SKIN, 1e-6));
    hold(&world, &mut m, FORWARD, 2.5);
    assert_eq!(m.physics, Physics::Walking);
    assert!(close(m.location[2], REST, 1e-6), "z {}", m.location[2]);
    assert!(m.location[0] > 200.0);
}

#[test]
fn triangles_hold_the_pawn_too() {
    let a = Brush::from_triangle([-2000.0, -2000.0, 0.0], [2000.0, -2000.0, 0.0], [2000.0, 2000.0, 0.0], 0).unwrap();
    let b = Brush::from_triangle([-2000.0, -2000.0, 0.0], [2000.0, 2000.0, 0.0], [-2000.0, 2000.0, 0.0], 1).unwrap();
    let world = CollisionWorld::new(vec![a, b]);
    let mut m = standing(&world, [-300.0, 200.0], 0);
    assert!(close(m.location[2], REST, 1e-6));
    // Across the shared diagonal.
    hold(&world, &mut m, FORWARD, 1.5);
    assert!(m.location[0] > 250.0);
    assert!(close(m.location[2], REST, 1e-6));
    assert!(Brush::from_triangle([0.0; 3], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0], 0).is_err());
}

#[test]
fn the_cylinder_rests_on_its_rim_on_a_slope_and_a_capsule_lower() {
    // A slope of normal Z 0.8 (Sanctuary's stair hulls: 0.802). Under the centre, a cylinder's feet stand r tan(a)
    // = 31.5 uu above the surface (the live observation: 35-42 uu), a capsule's r (1 / cos(a) - 1) = 10.5 uu.
    let slope = wedge(-2000.0, 4000.0, 3000.0);
    for (shape, expected) in [
        (Shape::Cylinder { radius: 42.0, half_height: 80.0 }, 31.5),
        (Shape::Capsule { radius: 42.0, half_height: 80.0 }, 10.5),
    ] {
        let world = CollisionWorld::new(vec![slope.clone()]);
        let surface = FLOOR_TOP + 0.75 * 2000.0;
        let start = [0.0, 0.0, surface + 300.0];
        let hit = world.trace(&shape, start, [0.0, 0.0, surface - 300.0]);
        assert!(hit.blocked());
        assert!(close(hit.normal[2], 0.8, 1e-9));
        let feet = hit.end[2] - 80.0 - surface;
        assert!(close(feet, expected + SKIN / 0.8, 1e-6), "{shape:?}: feet {feet}");
    }
}

#[test]
fn a_hull_from_its_triangles_has_the_planes_of_a_search() {
    let points = [
        [0.0, 0.0, 0.0],
        [100.0, 0.0, 0.0],
        [0.0, 100.0, 0.0],
        [0.0, 0.0, 100.0],
    ];
    let searched = Brush::from_hull(&points, &[], 0).unwrap();
    let given = Brush::from_hull(&points, &[[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]], 0).unwrap();
    assert_eq!(searched.planes.len(), given.planes.len());
    for p in &searched.planes {
        assert!(given.planes.iter().any(|q| close(q.d, p.d, 1e-9) && (0..3).all(|i| close(q.n[i], p.n[i], 1e-9))));
    }
    // Four faces and the three bevels the faces do not already give (+X, +Y, +Z).
    assert_eq!(searched.planes.len(), 7);
    let flat = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 1.0, 0.0]];
    assert!(Brush::from_hull(&flat, &[], 0).is_err());
}

#[test]
fn the_same_inputs_give_the_same_path() {
    let world = CollisionWorld::new(vec![
        floor(),
        wedge(400.0, 400.0, 300.0),
        cuboid([-600.0, 150.0, FLOOR_TOP], [300.0, 300.0, FLOOR_TOP + 300.0], 2),
        cuboid([-300.0, -400.0, FLOOR_TOP], [-200.0, -300.0, FLOOR_TOP + 30.0], 3),
    ]);
    let script = [
        Segment::new("settle", 0.5, 0.0, 0.0, false, 0),
        Segment::new("forward", 2.0, 1.0, 0.0, false, 0),
        Segment::new("jump", 2.5, 1.0, 0.3, true, 120),
        Segment::new("back", 3.0, -1.0, 1.0, false, -200),
    ];
    let run = || {
        let mut m = Mover::spawn(MovementParams::maya(), [-500.0, -350.0, 200.0], Rotator::new(0, 3000, 0));
        walk::run(&world, &mut m, &script, DT, 1)
    };
    let (a, b) = (run(), run());
    assert_eq!(a, b);
    let bits = |r: &walk::WalkReport| -> Vec<u64> {
        r.samples.iter().flat_map(|s| s.location.iter().map(|v| v.to_bits())).collect()
    };
    assert_eq!(bits(&a), bits(&b));
    assert!(a.first_landing.is_some());
}

#[test]
fn the_walk_checks_catch_a_fall_and_a_stall() {
    // Nothing to land on: fell through.
    let empty = CollisionWorld::new(Vec::new());
    let mut m = Mover::spawn(MovementParams::maya(), [0.0, 0.0, 0.0], Rotator::default());
    let r = walk::run(&empty, &mut m, &[Segment::new("drop", 6.0, 0.0, 0.0, false, 0)], DT, 60);
    assert!(r.fell_through && !r.passed());
    // A wall right ahead: a stall at a wall, named, not a fault; the run as a whole is stuck (it travels too little).
    let wall = cuboid([60.0, -1000.0, FLOOR_TOP], [200.0, 1000.0, FLOOR_TOP + 300.0], 7);
    let world = CollisionWorld::new(vec![floor(), wall]);
    let mut m = Mover::spawn(MovementParams::maya(), [0.0, 0.0, REST + 10.0], Rotator::default());
    let script = [Segment::new("settle", 0.5, 0.0, 0.0, false, 0), Segment::new("into the wall", 1.0, 1.0, 0.0, false, 0)];
    let r = walk::run(&world, &mut m, &script, DT, 60);
    let s = &r.segments[1];
    assert!(s.stalled && !s.stuck, "{s:?}");
    assert_eq!(s.blocker.map(|b| (b.brush, b.normal)), Some((1, [-1.0, 0.0, 0.0])));
    assert!(r.stuck && !r.fell_through && !r.passed());
    // A pawn that cannot move (AccelRate 0) on open ground stalls with nothing in the way: stuck.
    let world = CollisionWorld::new(vec![floor()]);
    let mut params = MovementParams::maya();
    params.accel_rate = 0.0;
    let mut m = Mover::spawn(params, [0.0, 0.0, REST + 10.0], Rotator::default());
    let r = walk::run(&world, &mut m, &script, DT, 60);
    assert!(r.segments[1].stuck && r.segments[1].blocker.is_none());
    // Open ground, enough travel: passes.
    let mut m = Mover::spawn(MovementParams::maya(), [0.0, 0.0, REST + 10.0], Rotator::default());
    let open = [Segment::new("settle", 0.5, 0.0, 0.0, false, 0), Segment::new("run", 3.0, 1.0, 0.0, false, 0)];
    assert!(walk::run(&world, &mut m, &open, DT, 60).passed());
}

#[test]
fn a_pawn_spawned_in_the_floor_is_lifted_out() {
    let world = CollisionWorld::new(vec![floor()]);
    let mut m = Mover::spawn(MovementParams::maya(), [0.0, 0.0, 70.0], Rotator::default());
    assert_eq!(m.lift_out_of_solid(&world), Some(11.0));
    assert!(close(m.location[2], 81.0, 1e-9));
    hold(&world, &mut m, MoveInput::default(), 0.5);
    assert_eq!(m.physics, Physics::Walking);
    // Free already: not moved.
    assert_eq!(m.lift_out_of_solid(&world), Some(0.0));
    // Buried deeper than the search: left alone.
    let mut deep = Mover::spawn(MovementParams::maya(), [0.0, 0.0, -400.0], Rotator::default());
    let deep_world = CollisionWorld::new(vec![cuboid([-500.0, -500.0, -1000.0], [500.0, 500.0, 0.0], 0)]);
    assert_eq!(deep.lift_out_of_solid(&deep_world), None);
    assert_eq!(deep.location[2], -400.0);
}
