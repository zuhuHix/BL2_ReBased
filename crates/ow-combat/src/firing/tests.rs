//! Owner: lane W1-FIRING. Synthetic checks of `firing` (no game data).
//!
//! Two parts:
//! - `cpp_checks`: every `check` of `tests/weapon_firing_math_test.cpp`, re-expressed in the same order with the same
//!   tolerances, so each C++ case has its Rust twin.
//! - `bit_equal_to_cpp`: the values those cases (plus invented edge cases: odd inputs, NaN, empty and out-of-range
//!   arguments) produce, compared by `to_bits()` against [`CPP_BITS`], the bit patterns the C++ reference prints for the
//!   same calls (`src/weapon_firing.cpp` built with MSVC 14.50 `/O2`, x86_64, zuhu desktop, 2026-11-15; the printing
//!   program is kept under the lane's `local/p2/W1-FIRING/`). Every input is invented.

use super::*;
use ow_core::units::ROTATOR_UNITS_PER_DEGREE;

fn near(a: f64, b: f64) -> bool {
    near_eps(a, b, 1e-6)
}

fn near_eps(a: f64, b: f64, eps: f64) -> bool {
    (a - b).abs() <= eps
}

#[test]
fn cpp_checks() {
    // Spread: diameter in degrees, so the extreme yaw is half the spread; pitch is 0 at the disc's left/right edge.
    let a = (4.0 * ROTATOR_UNITS_PER_DEGREE).trunc();
    let edge = spread_offset(4.0, 0.0, 0.0);
    assert!(
        near(edge.yaw, 0.5 * a) && near(edge.pitch, 0.0),
        "spread: edge yaw is half the diameter, pitch 0"
    );
    let centre = spread_offset(4.0, 0.5, 0.0);
    assert!(
        near(centre.yaw, 0.0) && near(centre.pitch, 0.5 * a),
        "spread: centre column reaches full pitch"
    );
    assert!(
        near(spread_offset(0.0, 0.1, 0.9).yaw, 0.0),
        "spread: zero spread leaves the aim"
    );

    // Pattern: index advances per pellet; t interpolates both components; spread scaling.
    let mut pattern = Pattern {
        lines: vec![
            PatternLine {
                start_pitch: 0,
                start_yaw: -100,
                end_pitch: 0,
                end_yaw: 100,
                start_only: false,
            },
            PatternLine {
                start_pitch: 10,
                start_yaw: 0,
                end_pitch: 10,
                end_yaw: 0,
                start_only: true,
            },
        ],
        ..Pattern::default()
    };
    let mut index = 0;
    let first = pattern_offset(&pattern, 1.0, &mut index, 0.0, 0.25);
    assert!(
        near(first.yaw, -50.0) && index == 1,
        "pattern: line 0 at t=0.25, index -> 1"
    );
    let second = pattern_offset(&pattern, 1.0, &mut index, 0.0, 0.9);
    assert!(
        near(second.pitch, 10.0) && near(second.yaw, 0.0) && index == 0,
        "pattern: start-only line, index wraps"
    );
    pattern.scale_by_spread = true;
    pattern.base_pattern_spread = 2.0;
    pattern.min_pattern_scale = 0.75;
    index = 0;
    assert!(
        near(
            pattern_offset(&pattern, 1.0, &mut index, 0.0, 1.0).yaw,
            75.0
        ),
        "pattern: scale clamps to MinPatternScale"
    );
    pattern.fire_randomly = true;
    index = 0;
    pattern_offset(&pattern, 4.0, &mut index, 0.99, 0.0);
    assert!(
        index == 0,
        "pattern: random pick chose the last line and wrapped"
    );
    let none = Pattern::default();
    index = 3;
    assert!(
        near(pattern_offset(&none, 1.0, &mut index, 0.5, 0.5).yaw, 0.0) && index == 3,
        "pattern: no lines, no change"
    );

    // Fire interval and spin-up.
    assert!(
        near(fire_interval(0.1, true, 3.0, 0.0), 0.3),
        "spin: starts at multiplier x base"
    );
    assert!(
        near(fire_interval(0.1, true, 3.0, 0.5), 0.2),
        "spin: linear in the percent"
    );
    assert!(
        near(fire_interval(0.1, true, 0.5, 0.0), 0.1),
        "spin: a multiplier below 1 never speeds up"
    );
    assert!(
        near(fire_interval(0.1, false, 3.0, 0.0), 0.1),
        "spin: not spinning keeps base"
    );
    assert!(
        near(tick_spin(0.0, 0.25, true, 1.0, 2.0), 0.25)
            && near(tick_spin(0.5, 0.5, false, 1.0, 2.0), 0.25),
        "spin: up at 1/duration, down at 1/spin-down duration"
    );
    assert!(
        near(tick_spin(0.2, 0.1, true, 0.0, 1.0), 1.0),
        "spin: zero duration snaps up"
    );
    assert!(
        near(burst_interval(0.5, 0.05, 0.1), 0.25) && near(burst_interval(0.5, 0.05, 0.0), 0.5),
        "burst interval follows the current fire interval"
    );

    // Bursts: a press owes the whole burst, then waits for a new press; a weapon change resets the state.
    let mut burst = Burst::default();
    burst.press();
    assert!(
        burst.wants_fire(true) && !burst.shot(3) && burst.left == 2,
        "burst: first shot of 3 leaves 2"
    );
    assert!(
        burst.wants_fire(false),
        "burst: released trigger still fires the rest of the burst"
    );
    assert!(
        !burst.shot(3) && burst.shot(3) && burst.done && !burst.wants_fire(true),
        "burst: ends, held trigger waits"
    );
    burst.press();
    assert!(
        burst.wants_fire(true),
        "burst: a new press starts the next burst"
    );
    burst.shot(3);
    burst.reset(); // switched to an automatic weapon mid-burst
    assert!(
        !burst.wants_fire(false) && !burst.active(),
        "burst: weapon change drops the owed shots"
    );
    assert!(
        !burst.shot(0) && burst.wants_fire(true) && !burst.wants_fire(false),
        "burst: count 0 fires only while held"
    );
    // Lifecycle: a reload or a respawn mid-burst drops the owed shots, and a held trigger does not start a new burst.
    let mut reloaded = Burst::default();
    reloaded.press();
    reloaded.shot(3);
    reloaded.interrupt();
    assert!(
        !reloaded.wants_fire(false) && !reloaded.wants_fire(true),
        "burst: interrupted mid-burst fires nothing, held or not"
    );
    reloaded.press();
    assert!(
        reloaded.wants_fire(true),
        "burst: after the interruption a new press fires"
    );
    let mut single = Burst::default();
    single.press();
    assert!(
        !single.shot(1) && single.done && !single.wants_fire(true),
        "burst: count 1 is one shot per press"
    );

    // Per-pellet damage.
    assert!(
        near(
            multi_projectile_damage(10.0, 4.0, 6.0, 1.0, 0.0, 5),
            10.0 * 2.0 + (4.0 * 2.0 + 6.0) / 5.0
        ),
        "pellet damage splits flat adds"
    );

    // Splash falloff with the class defaults.
    let defaults = Falloff::default();
    assert!(
        near(radius_damage_scale(10.0, 100.0, &defaults, false), 1.0),
        "falloff: full inside 20 %"
    );
    assert!(
        near(
            radius_damage_scale(50.0, 100.0, &defaults, false),
            0.2 + 0.8 * 0.5
        ),
        "falloff: linear in the band"
    );
    assert!(
        near(radius_damage_scale(90.0, 100.0, &defaults, false), 0.2),
        "falloff: floor to the edge"
    );
    assert!(
        near(radius_damage_scale(101.0, 100.0, &defaults, false), 0.0),
        "falloff: nothing outside"
    );
    assert!(
        near(radius_damage_scale(90.0, 100.0, &defaults, true), 1.0),
        "falloff: full-damage flag"
    );
    let flat = Falloff {
        max_damage_radius: 0.5,
        min_damage_radius: 0.5,
        min_damage_percent: 0.3,
    };
    assert!(
        near(radius_damage_scale(25.0, 100.0, &flat, false), 0.75),
        "falloff: empty band falls back to 1 - d/R"
    );
    assert!(
        near(explosion_radius(0.0), 256.0) && near(explosion_radius(120.0), 120.0),
        "explosion radius 0 -> 256"
    );

    // Status chance.
    assert!(
        near(status_chance(20.0, 1.0, 0.4, 1.0, 1.0, 1.0), 0.08),
        "status: product x 0.01"
    );
    assert!(
        near(status_chance(80.0, 1.0, 3.0, 1.0, 1.0, 1.0), 1.0),
        "status: clamps to 1"
    );
    assert!(
        near(
            fire_interval_chance_factor(0.05, 0.1, 0.5, 2.0),
            sqrt_f64(0.5)
        ),
        "status: faster gun uses short power"
    );
    assert!(
        near(fire_interval_chance_factor(0.2, 0.1, 0.5, 2.0), 4.0),
        "status: slower gun uses long power"
    );
    assert!(
        near(fire_interval_chance_factor(0.2, 0.0, 0.5, 2.0), 1.0),
        "status: no type rate gives 1"
    );
    assert!(
        near(radius_status_scale(70.0, 20.0, 100.0), 0.5)
            && near(radius_status_scale(10.0, 20.0, 100.0), 1.0),
        "status: radius scale from the cylinder edge"
    );

    // Flight.
    let mut flight = Flight {
        velocity: Vec3 {
            x: 1000.0,
            y: 0.0,
            z: 0.0,
        },
        ..Flight::default()
    };
    let rocket = Motion {
        gravity_z: -100.0,
        ..Motion::default()
    };
    for _ in 0..100 {
        step(&mut flight, &rocket, 0.01);
    }
    assert!(
        near_eps(flight.position.x, 1000.0, 1e-6)
            && flight.position.z < -45.0
            && flight.position.z > -55.0,
        "flight: straight at speed, gravity drop about g t^2 / 2"
    );
    let mut gyro = Flight {
        velocity: Vec3 {
            x: 1000.0,
            y: 0.0,
            z: 0.0,
        },
        ..Flight::default()
    };
    let gyro_motion = Motion {
        air_friction: -1.0,
        max_speed: 1500.0,
        ..Motion::default()
    };
    for _ in 0..100 {
        step(&mut gyro, &gyro_motion, 0.01);
    }
    assert!(
        gyro.velocity.x > 1000.0 && gyro.velocity.x <= 1500.0 + 1e-9,
        "flight: negative friction speeds up, capped"
    );

    // DOT clock: ticks of dps x accumulated time at 0.33 s, overshoot carried, a partial tick at the end.
    let mut clock = 0.0;
    assert!(
        near(dot_advance(&mut clock, 0.2, 30.0, DOT_PERIOD), 0.0) && near(clock, 0.2),
        "dot: no tick before the period"
    );
    assert!(
        near(dot_advance(&mut clock, 0.2, 30.0, DOT_PERIOD), 30.0 * 0.4) && near(clock, 0.4 - 0.33),
        "dot: tick pays the built-up time"
    );
    assert!(
        near(dot_finish(&mut clock, 30.0), 30.0 * (0.4 - 0.33)) && near(clock, 0.0),
        "dot: final partial tick"
    );
    let (mut total, mut steady) = (0.0, 0.0);
    for _ in 0..120 {
        total += dot_advance(&mut steady, 1.0 / 60.0, 10.0, DOT_PERIOD);
    }
    assert!(
        total > 10.0 * 2.0 * 1.0 && total < 10.0 * 2.0 * 1.2,
        "dot: frame overshoot adds a little over dps x time"
    );

    // Momentum (invented numbers): x100 at setup, then momentum / mass on the victim; tiny masses take x100.
    assert!(near(hit_momentum(8.0), 800.0), "momentum: type value x 100");
    let push = momentum_velocity(
        &Vec3 {
            x: 600.0,
            y: -800.0,
            z: 0.0,
        },
        50.0,
    );
    assert!(
        near(push.x, 12.0) && near(push.y, -16.0) && near(push.z, 0.0),
        "momentum: velocity change = momentum / mass"
    );
    assert!(
        near(
            momentum_velocity(
                &Vec3 {
                    x: 3.0,
                    y: 0.0,
                    z: 0.0
                },
                0.005
            )
            .x,
            300.0
        ),
        "momentum: mass <= 0.01 scales by 100"
    );

    // Level scale (invented table): row min(|diff|, rows) - 1, higher or lower column, equal levels 1.
    let rows = level_rows();
    assert!(
        near(status_level_scale(10, 10, &rows), 1.0),
        "level scale: equal levels"
    );
    assert!(
        near(status_level_scale(11, 10, &rows), 1.0)
            && near(status_level_scale(10, 11, &rows), 0.9),
        "level scale: one apart"
    );
    assert!(
        near(status_level_scale(12, 10, &rows), 1.25),
        "level scale: two above"
    );
    assert!(
        near(status_level_scale(54, 7, &rows), 3.0) && near(status_level_scale(7, 54, &rows), 0.1),
        "level scale: clamps to the last row"
    );
    assert!(
        near(status_level_scale(54, 7, &[]), 1.0),
        "level scale: no table"
    );

    // Accuracy pool (invented numbers): impulse clamps and restarts the delay; decay only after the delay.
    let mut pool = AccuracyPool {
        min_value: 1.0,
        max_value: 10.0,
        idle_rate: -4.0,
        idle_delay: 0.5,
        ..AccuracyPool::default()
    };
    pool.reset(false);
    assert!(
        near(pool.value, 1.0),
        "pool: starts at min without StartWithMaxValue"
    );
    pool.impulse(3.0);
    assert!(
        near(pool.value, 4.0) && near(pool.idle_for, 0.0),
        "pool: impulse adds and restarts the delay"
    );
    pool.tick(0.4);
    assert!(near(pool.value, 4.0), "pool: holds during the delay");
    pool.tick(0.35); // 0.25 s past the delay
    assert!(
        near(pool.value, 3.0),
        "pool: decays only for the time past the delay"
    );
    pool.impulse(100.0);
    assert!(near(pool.value, 10.0), "pool: clamps to max");
    pool.tick(10.0);
    assert!(near(pool.value, 1.0), "pool: decays to min, no further");
    pool.impulse(-5.0);
    assert!(
        near(pool.value, 1.0),
        "pool: a negative impulse clamps to min"
    );
    assert!(
        near(accuracy_offset(4.0, 1.0, 0.0, 0.0).yaw, 0.0),
        "pool cone: fully zoomed removes it"
    );
    assert!(
        near(
            accuracy_offset(4.0, 0.5, 0.0, 0.0).yaw,
            0.25 * (4.0 * ROTATOR_UNITS_PER_DEGREE).trunc()
        ),
        "pool cone: half zoom halves it"
    );

    // Kick (invented numbers).
    assert!(
        near(Kick::amount(2.0, 0.0, 1.5), 2.0 * 182.0444)
            && near(Kick::amount(2.0, 1.0, 1.5), 3.0 * 182.0444),
        "kick: impulse x 182.0444 x zoom lerp"
    );
    let mut kd = KickData {
        up: 1.0,
        down: 0.0,
        left: 0.0,
        right: 0.0,
        min_vertical: 0.4,
        min_horizontal: 0.0,
        recovery_time: 1.0,
        speed: 10.0,
        ..KickData::default()
    };
    let mut kick = Kick::default();
    kick.add(100.0, &kd, 1.0, 0.5, 0.0); // v = 0.5 up, h = 0: all vertical
    assert!(
        near(kick.target_pitch, 100.0) && near(kick.target_yaw, 0.0),
        "kick: no horizontal draw puts it all on pitch"
    );
    kd.left = 1.0;
    kd.right = 1.0;
    let mut side = Kick::default();
    side.add(100.0, &kd, 0.5, 0.5, 1.0); // m = 50, v = 0.5, h = +1: vertical 0.4 + 0.6 x 0.5 / 1.5 = 0.6
    assert!(
        near(side.target_pitch, 30.0) && near(side.target_yaw, 20.0),
        "kick: shares and FOV ratio"
    );
    side.tick(0.5, &kd);
    assert!(
        near(side.target_pitch, 15.0) && side.current_pitch > 5.0 && side.current_pitch < 30.0,
        "kick: target decays linearly, current chases"
    );
    side.tick(2.0, &kd);
    assert!(
        near(side.target_pitch, 0.0) && side.current_pitch.abs() < 1e-3,
        "kick: targets reach 0, current follows them home"
    );

    // The chase step is the per-frame linear one (live fit): one short frame moves speed x dt of the gap.
    let chase = KickData {
        speed: 8.0,
        recovery_time: 100.0,
        ..kd
    };
    let mut one = Kick {
        target_pitch: 1000.0,
        recover_left: 100.0,
        ..Kick::default()
    };
    one.tick(0.0085, &chase);
    assert!(
        near_eps(
            one.current_pitch,
            1000.0 * (1.0 - 0.0085 / 100.0) * 8.0 * 0.0085,
            1e-6
        ),
        "kick chase: linear per-frame step at a normal frame"
    );
    // A hitch is substepped: one 0.25 s frame equals eight 1/32 s frames, and does not snap to the target.
    let mut hitch = Kick {
        target_pitch: 1000.0,
        recover_left: 100.0,
        ..Kick::default()
    };
    let mut steps = hitch;
    hitch.tick(0.25, &chase);
    for _ in 0..8 {
        steps.tick(0.25 / 8.0, &chase);
    }
    assert!(
        near_eps(hitch.current_pitch, steps.current_pitch, 1e-9),
        "kick chase: a long frame equals its substeps"
    );
    assert!(
        hitch.current_pitch < hitch.target_pitch * 0.95,
        "kick chase: a 0.25 s hitch does not snap to the target (one unsplit step would)"
    );

    // Signed kick: a negative impulse kicks the other way; non-finite and bad configs are ignored or clamped.
    assert!(
        Kick::amount(-2.0, 0.0, 1.0) < 0.0,
        "kick: amount keeps the impulse's sign"
    );
    let up = KickData {
        up: 1.0,
        down: 0.0,
        left: 0.0,
        right: 0.0,
        min_vertical: 0.0,
        min_horizontal: 0.0,
        ..kd
    };
    let mut neg = Kick::default();
    neg.add(-100.0, &up, 1.0, 0.5, 0.0);
    assert!(
        near(neg.target_pitch, -100.0) && near(neg.target_yaw, 0.0),
        "kick: a negative kick moves the target down"
    );
    let mut nan = Kick::default();
    nan.add(f64::NAN, &up, 1.0, 0.5, 0.0);
    assert!(near(nan.target_pitch, 0.0), "kick: a NaN kick is ignored");
    let backwards = KickData {
        speed: -8.0,
        ..chase
    };
    let mut still = Kick {
        target_pitch: 1000.0,
        recover_left: 100.0,
        ..Kick::default()
    };
    still.tick(0.1, &backwards);
    assert!(
        near(still.current_pitch, 0.0),
        "kick: a negative speed does not move away from the target"
    );
    let mut bad = AccuracyPool {
        min_value: 10.0,
        max_value: 1.0,
        idle_rate: -4.0,
        idle_delay: 0.0,
        ..AccuracyPool::default()
    };
    bad.reset(false);
    bad.impulse(f64::NAN);
    assert!(
        bad.value.is_finite(),
        "pool: a NaN impulse leaves the value finite"
    );
    bad.impulse(100.0);
    assert!(
        near(bad.value, 10.0),
        "pool: min above max is read the other way round"
    );
    bad.impulse(-100.0);
    assert!(
        near(bad.value, 1.0),
        "pool: a negative impulse narrows the cone, clamped to the floor"
    );
    assert!(
        near(accuracy_offset(f64::NAN, 0.0, 0.1, 0.2).yaw, 0.0),
        "pool cone: a NaN pool value gives no offset"
    );
}

fn level_rows() -> Vec<LevelChanceScale> {
    vec![
        LevelChanceScale {
            level_difference: 1,
            higher: 1.0,
            lower: 0.9,
        },
        LevelChanceScale {
            level_difference: 2,
            higher: 1.25,
            lower: 0.5,
        },
        LevelChanceScale {
            level_difference: 3,
            higher: 3.0,
            lower: 0.1,
        },
    ]
}

fn index_value(index: usize) -> f64 {
    f64::from(u32::try_from(index).expect("small index"))
}

/// The same calls, in the same order, as the C++ printing program that produced [`CPP_BITS`].
fn values() -> Vec<(&'static str, f64)> {
    let mut out: Vec<(&'static str, f64)> = Vec::new();
    let mut p = |name: &'static str, v: f64| out.push((name, v));
    let nan = f64::NAN;

    let o = spread_offset(4.0, 0.0, 0.0);
    p("spread edge pitch", o.pitch);
    p("spread edge yaw", o.yaw);
    let o = spread_offset(4.0, 0.5, 0.0);
    p("spread centre pitch", o.pitch);
    p("spread centre yaw", o.yaw);
    p("spread zero yaw", spread_offset(0.0, 0.1, 0.9).yaw);
    let o = spread_offset(3.7, 0.123, 0.987);
    p("spread odd pitch", o.pitch);
    p("spread odd yaw", o.yaw);
    let o = spread_offset(-2.5, 0.31, 0.77);
    p("spread negative pitch", o.pitch);
    p("spread negative yaw", o.yaw);
    let o = spread_offset(1.1, 1.0, 0.2);
    p("spread r1 one pitch", o.pitch);
    p("spread r1 one yaw", o.yaw);
    p("spread nan pitch", spread_offset(nan, 0.3, 0.3).pitch);

    let mut pattern = Pattern {
        lines: vec![
            PatternLine {
                start_pitch: 0,
                start_yaw: -100,
                end_pitch: 0,
                end_yaw: 100,
                start_only: false,
            },
            PatternLine {
                start_pitch: 10,
                start_yaw: 0,
                end_pitch: 10,
                end_yaw: 0,
                start_only: true,
            },
        ],
        ..Pattern::default()
    };
    let mut index = 0;
    let o = pattern_offset(&pattern, 1.0, &mut index, 0.0, 0.25);
    p("pattern first pitch", o.pitch);
    p("pattern first yaw", o.yaw);
    p("pattern first index", index_value(index));
    let o = pattern_offset(&pattern, 1.0, &mut index, 0.0, 0.9);
    p("pattern second pitch", o.pitch);
    p("pattern second yaw", o.yaw);
    p("pattern second index", index_value(index));
    pattern.scale_by_spread = true;
    pattern.base_pattern_spread = 2.0;
    pattern.min_pattern_scale = 0.75;
    index = 0;
    p(
        "pattern scaled yaw",
        pattern_offset(&pattern, 1.0, &mut index, 0.0, 1.0).yaw,
    );
    index = 0;
    let o = pattern_offset(&pattern, 3.3, &mut index, 0.0, 0.37);
    p("pattern spread scale pitch", o.pitch);
    p("pattern spread scale yaw", o.yaw);
    pattern.fire_randomly = true;
    index = 0;
    let o = pattern_offset(&pattern, 4.0, &mut index, 0.99, 0.0);
    p("pattern random index", index_value(index));
    p("pattern random pitch", o.pitch);
    index = 0;
    let o = pattern_offset(&pattern, 4.0, &mut index, nan, 0.0);
    p("pattern random nan index", index_value(index));
    p("pattern random nan yaw", o.yaw);
    let odd = Pattern {
        lines: vec![PatternLine {
            start_pitch: -7,
            start_yaw: 33,
            end_pitch: 91,
            end_yaw: -250,
            start_only: false,
        }],
        ..Pattern::default()
    };
    index = 5;
    let o = pattern_offset(&odd, 1.0, &mut index, 0.0, 0.333);
    p("pattern odd pitch", o.pitch);
    p("pattern odd yaw", o.yaw);
    p("pattern odd index", index_value(index));

    p("spin start", fire_interval(0.1, true, 3.0, 0.0));
    p("spin half", fire_interval(0.1, true, 3.0, 0.5));
    p("spin below one", fire_interval(0.1, true, 0.5, 0.0));
    p("spin off", fire_interval(0.1, false, 3.0, 0.0));
    p("spin odd", fire_interval(0.07, true, 2.3, 0.41));
    p("spin over", fire_interval(0.07, true, 2.3, 1.7));
    p("tick up", tick_spin(0.0, 0.25, true, 1.0, 2.0));
    p("tick down", tick_spin(0.5, 0.5, false, 1.0, 2.0));
    p("tick snap", tick_spin(0.2, 0.1, true, 0.0, 1.0));
    p("tick snap down", tick_spin(0.2, 0.1, false, 1.0, 0.0));
    p("tick odd", tick_spin(0.31, 0.016, true, 0.7, 1.3));
    p("burst follows", burst_interval(0.5, 0.05, 0.1));
    p("burst no base", burst_interval(0.5, 0.05, 0.0));
    p("burst odd", burst_interval(0.3, 0.071, 0.093));

    p(
        "pellet damage",
        multi_projectile_damage(10.0, 4.0, 6.0, 1.0, 0.0, 5),
    );
    p(
        "pellet odd",
        multi_projectile_damage(17.3, 2.1, 0.7, 0.35, 0.12, 7),
    );
    p(
        "pellet zero pellets",
        multi_projectile_damage(17.3, 2.1, 0.7, 0.35, 0.12, 0),
    );

    let defaults = Falloff::default();
    p(
        "falloff inside",
        radius_damage_scale(10.0, 100.0, &defaults, false),
    );
    p(
        "falloff band",
        radius_damage_scale(50.0, 100.0, &defaults, false),
    );
    p(
        "falloff floor",
        radius_damage_scale(90.0, 100.0, &defaults, false),
    );
    p(
        "falloff outside",
        radius_damage_scale(101.0, 100.0, &defaults, false),
    );
    p(
        "falloff full",
        radius_damage_scale(90.0, 100.0, &defaults, true),
    );
    let flat = Falloff {
        max_damage_radius: 0.5,
        min_damage_radius: 0.5,
        min_damage_percent: 0.3,
    };
    p(
        "falloff empty band",
        radius_damage_scale(25.0, 100.0, &flat, false),
    );
    p(
        "falloff odd band",
        radius_damage_scale(37.7, 113.0, &defaults, false),
    );
    p(
        "falloff edge",
        radius_damage_scale(100.0, 100.0, &defaults, false),
    );
    p(
        "falloff zero radius",
        radius_damage_scale(0.0, 0.0, &defaults, false),
    );
    p("explosion zero", explosion_radius(0.0));
    p("explosion kept", explosion_radius(120.0));

    p(
        "status product",
        status_chance(20.0, 1.0, 0.4, 1.0, 1.0, 1.0),
    );
    p("status clamp", status_chance(80.0, 1.0, 3.0, 1.0, 1.0, 1.0));
    p("status odd", status_chance(13.7, 0.9, 1.1, 1.3, 0.83, 0.61));
    p(
        "status negative",
        status_chance(-5.0, 1.0, 1.0, 1.0, 1.0, 1.0),
    );
    p(
        "factor short",
        fire_interval_chance_factor(0.05, 0.1, 0.5, 2.0),
    );
    p(
        "factor long",
        fire_interval_chance_factor(0.2, 0.1, 0.5, 2.0),
    );
    p(
        "factor no type",
        fire_interval_chance_factor(0.2, 0.0, 0.5, 2.0),
    );
    p(
        "factor equal",
        fire_interval_chance_factor(0.1, 0.1, 0.5, 2.0),
    );
    p(
        "factor odd short",
        fire_interval_chance_factor(0.067, 0.11, 0.37, 1.9),
    );
    p(
        "factor odd long",
        fire_interval_chance_factor(0.23, 0.11, 0.37, 1.9),
    );
    p("radius status mid", radius_status_scale(70.0, 20.0, 100.0));
    p(
        "radius status inside",
        radius_status_scale(10.0, 20.0, 100.0),
    );
    p("radius status odd", radius_status_scale(77.7, 23.1, 141.0));
    p("radius status none", radius_status_scale(10.0, 20.0, 0.0));

    let mut flight = Flight {
        velocity: Vec3 {
            x: 1000.0,
            y: 0.0,
            z: 0.0,
        },
        ..Flight::default()
    };
    let rocket = Motion {
        gravity_z: -100.0,
        ..Motion::default()
    };
    for _ in 0..100 {
        step(&mut flight, &rocket, 0.01);
    }
    p("flight x", flight.position.x);
    p("flight z", flight.position.z);
    p("flight vz", flight.velocity.z);
    p("flight age", flight.age);
    let mut gyro = Flight {
        velocity: Vec3 {
            x: 1000.0,
            y: 0.0,
            z: 0.0,
        },
        ..Flight::default()
    };
    let gyro_motion = Motion {
        air_friction: -1.0,
        max_speed: 1500.0,
        ..Motion::default()
    };
    for _ in 0..100 {
        step(&mut gyro, &gyro_motion, 0.01);
    }
    p("gyro vx", gyro.velocity.x);
    p("gyro x", gyro.position.x);
    let mut odd3 = Flight {
        velocity: Vec3 {
            x: 310.5,
            y: -220.25,
            z: 95.0,
        },
        position: Vec3 {
            x: 1.5,
            y: 2.5,
            z: -3.5,
        },
        ..Flight::default()
    };
    let odd_motion = Motion {
        acceleration: Vec3 {
            x: 12.0,
            y: -3.5,
            z: 7.25,
        },
        gravity_z: -386.4,
        air_friction: 0.37,
        max_speed: 350.0,
    };
    for _ in 0..37 {
        step(&mut odd3, &odd_motion, 1.0 / 60.0);
    }
    p("odd flight px", odd3.position.x);
    p("odd flight py", odd3.position.y);
    p("odd flight pz", odd3.position.z);
    p("odd flight vx", odd3.velocity.x);
    p("odd flight vy", odd3.velocity.y);
    p("odd flight vz", odd3.velocity.z);
    p("odd flight age", odd3.age);

    let mut clock = 0.0;
    p("dot first", dot_advance(&mut clock, 0.2, 30.0, DOT_PERIOD));
    p("dot clock first", clock);
    p("dot second", dot_advance(&mut clock, 0.2, 30.0, DOT_PERIOD));
    p("dot clock second", clock);
    p("dot finish", dot_finish(&mut clock, 30.0));
    p("dot clock finish", clock);
    let (mut total, mut steady) = (0.0, 0.0);
    for _ in 0..120 {
        total += dot_advance(&mut steady, 1.0 / 60.0, 10.0, DOT_PERIOD);
    }
    p("dot steady total", total);
    p("dot steady clock", steady);
    let mut neg = 0.1;
    p(
        "dot negative dt",
        dot_advance(&mut neg, -0.5, 10.0, DOT_PERIOD),
    );
    p("dot negative clock", neg);

    p("momentum", hit_momentum(8.0));
    let push = momentum_velocity(
        &Vec3 {
            x: 600.0,
            y: -800.0,
            z: 0.0,
        },
        50.0,
    );
    p("push x", push.x);
    p("push y", push.y);
    p("push z", push.z);
    p(
        "push tiny",
        momentum_velocity(
            &Vec3 {
                x: 3.0,
                y: 0.0,
                z: 0.0,
            },
            0.005,
        )
        .x,
    );
    p(
        "push odd",
        momentum_velocity(
            &Vec3 {
                x: 3.3,
                y: 0.0,
                z: 0.0,
            },
            7.1,
        )
        .x,
    );

    let rows = level_rows();
    p("level equal", status_level_scale(10, 10, &rows));
    p("level one above", status_level_scale(11, 10, &rows));
    p("level one below", status_level_scale(10, 11, &rows));
    p("level two above", status_level_scale(12, 10, &rows));
    p("level clamp above", status_level_scale(54, 7, &rows));
    p("level clamp below", status_level_scale(7, 54, &rows));
    p("level no table", status_level_scale(54, 7, &[]));

    let mut pool = AccuracyPool {
        min_value: 1.0,
        max_value: 10.0,
        idle_rate: -4.0,
        idle_delay: 0.5,
        ..AccuracyPool::default()
    };
    pool.reset(false);
    p("pool reset", pool.value);
    p("pool reset idle", pool.idle_for);
    pool.impulse(3.0);
    p("pool impulse", pool.value);
    p("pool impulse idle", pool.idle_for);
    pool.tick(0.4);
    p("pool delay", pool.value);
    pool.tick(0.35);
    p("pool decay", pool.value);
    p("pool decay idle", pool.idle_for);
    pool.impulse(100.0);
    p("pool max", pool.value);
    pool.tick(10.0);
    p("pool min", pool.value);
    pool.impulse(-5.0);
    p("pool negative", pool.value);
    let mut def = AccuracyPool::default();
    p("pool default value", def.value);
    def.impulse(0.7);
    def.tick(0.13);
    def.tick(0.29);
    def.tick(1.0 / 60.0);
    p("pool odd value", def.value);
    p("pool odd idle", def.idle_for);
    p("cone zoomed yaw", accuracy_offset(4.0, 1.0, 0.0, 0.0).yaw);
    p("cone half yaw", accuracy_offset(4.0, 0.5, 0.0, 0.0).yaw);
    let o = accuracy_offset(7.3, 0.27, 0.19, 0.83);
    p("cone odd pitch", o.pitch);
    p("cone odd yaw", o.yaw);
    p(
        "cone nan zoom yaw",
        accuracy_offset(7.3, nan, 0.19, 0.83).yaw,
    );

    p("kick amount", Kick::amount(2.0, 0.0, 1.5));
    p("kick amount zoomed", Kick::amount(2.0, 1.0, 1.5));
    p("kick amount odd", Kick::amount(1.37, 0.42, 0.65));
    let mut kd = KickData {
        up: 1.0,
        down: 0.0,
        left: 0.0,
        right: 0.0,
        min_vertical: 0.4,
        min_horizontal: 0.0,
        recovery_time: 1.0,
        speed: 10.0,
        ..KickData::default()
    };
    let mut kick = Kick::default();
    kick.add(100.0, &kd, 1.0, 0.5, 0.0);
    p("kick pitch", kick.target_pitch);
    p("kick yaw", kick.target_yaw);
    kd.left = 1.0;
    kd.right = 1.0;
    let mut side = Kick::default();
    side.add(100.0, &kd, 0.5, 0.5, 1.0);
    p("side pitch", side.target_pitch);
    p("side yaw", side.target_yaw);
    side.tick(0.5, &kd);
    p("side tick target", side.target_pitch);
    p("side tick current", side.current_pitch);
    p("side tick current yaw", side.current_yaw);
    side.tick(2.0, &kd);
    p("side home target", side.target_pitch);
    p("side home current", side.current_pitch);
    let chase = KickData {
        speed: 8.0,
        recovery_time: 100.0,
        ..kd
    };
    let mut one = Kick {
        target_pitch: 1000.0,
        recover_left: 100.0,
        ..Kick::default()
    };
    one.tick(0.0085, &chase);
    p("chase one current", one.current_pitch);
    p("chase one target", one.target_pitch);
    p("chase one recover", one.recover_left);
    let mut hitch = Kick {
        target_pitch: 1000.0,
        recover_left: 100.0,
        ..Kick::default()
    };
    let mut steps = hitch;
    hitch.tick(0.25, &chase);
    for _ in 0..8 {
        steps.tick(0.25 / 8.0, &chase);
    }
    p("hitch current", hitch.current_pitch);
    p("steps current", steps.current_pitch);
    p("kick amount negative", Kick::amount(-2.0, 0.0, 1.0));
    let up = KickData {
        up: 1.0,
        down: 0.0,
        left: 0.0,
        right: 0.0,
        min_vertical: 0.0,
        min_horizontal: 0.0,
        ..kd
    };
    let mut negk = Kick::default();
    negk.add(-100.0, &up, 1.0, 0.5, 0.0);
    p("neg pitch", negk.target_pitch);
    p("neg yaw", negk.target_yaw);
    let mut nank = Kick::default();
    nank.add(nan, &up, 1.0, 0.5, 0.0);
    p("nan pitch", nank.target_pitch);
    let backwards = KickData {
        speed: -8.0,
        ..chase
    };
    let mut still = Kick {
        target_pitch: 1000.0,
        recover_left: 100.0,
        ..Kick::default()
    };
    still.tick(0.1, &backwards);
    p("still current", still.current_pitch);
    let oddk = KickData {
        up: 0.8,
        down: 0.3,
        left: 0.45,
        right: 0.6,
        min_vertical: 0.15,
        min_horizontal: 0.2,
        speed: 6.5,
        recovery_time: 0.37,
        ..KickData::default()
    };
    let mut ko = Kick::default();
    ko.add(Kick::amount(1.9, 0.3, 0.7), &oddk, 0.83, 0.217, 0.661);
    ko.add(Kick::amount(1.9, 0.3, 0.7), &oddk, 0.83, 0.905, 0.044);
    p("odd kick target pitch", ko.target_pitch);
    p("odd kick target yaw", ko.target_yaw);
    for i in 0..9 {
        ko.tick(1.0 / 144.0 + f64::from(i) * 0.003, &oddk);
    }
    p("odd kick cur pitch", ko.current_pitch);
    p("odd kick cur yaw", ko.current_yaw);
    p("odd kick tgt pitch", ko.target_pitch);
    p("odd kick recover", ko.recover_left);
    ko.tick(0.7, &oddk);
    p("odd kick long cur pitch", ko.current_pitch);
    let mut bad = AccuracyPool {
        min_value: 10.0,
        max_value: 1.0,
        idle_rate: -4.0,
        idle_delay: 0.0,
        ..AccuracyPool::default()
    };
    bad.reset(false);
    bad.impulse(nan);
    p("bad nan", bad.value);
    bad.impulse(100.0);
    p("bad max", bad.value);
    bad.impulse(-100.0);
    p("bad min", bad.value);
    p("cone nan yaw", accuracy_offset(nan, 0.0, 0.1, 0.2).yaw);
    out
}

#[test]
fn bit_equal_to_cpp() {
    let got = values();
    assert_eq!(
        got.len(),
        CPP_BITS.len(),
        "the Rust and C++ case lists differ in length"
    );
    let mut mismatches = Vec::new();
    for ((name, value), (cpp_name, cpp_bits)) in got.iter().zip(CPP_BITS.iter()) {
        assert_eq!(name, cpp_name, "the case lists are out of step");
        if value.to_bits() != *cpp_bits {
            mismatches.push(format!(
                "{name}: rust {:#018x}, c++ {cpp_bits:#018x}",
                value.to_bits()
            ));
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} of {} values differ from the C++:\n{}",
        mismatches.len(),
        got.len(),
        mismatches.join("\n")
    );
}

/// Bit patterns printed by the C++ reference for the calls in [`values`] (see the module comment).
#[rustfmt::skip]
const CPP_BITS: &[(&str, u64)] = &[
    ("spread edge pitch", 0x0000000000000000),
    ("spread edge yaw", 0x4076c00000000000),
    ("spread centre pitch", 0x4076c00000000000),
    ("spread centre yaw", 0x0000000000000000),
    ("spread zero yaw", 0x0000000000000000),
    ("spread odd pitch", 0xc06ae951e57a41b7),
    ("spread odd yaw", 0x406fb7126e978d50),
    ("spread negative pitch", 0x405c689d1ec1f88b),
    ("spread negative yaw", 0xc0559ccccccccccd),
    ("spread r1 one pitch", 0x0000000000000000),
    ("spread r1 one yaw", 0xc059000000000000),
    ("spread nan pitch", 0x7ff8000000000000),
    ("pattern first pitch", 0x0000000000000000),
    ("pattern first yaw", 0xc049000000000000),
    ("pattern first index", 0x3ff0000000000000),
    ("pattern second pitch", 0x4024000000000000),
    ("pattern second yaw", 0x0000000000000000),
    ("pattern second index", 0x0000000000000000),
    ("pattern scaled yaw", 0x4052c00000000000),
    ("pattern spread scale pitch", 0x0000000000000000),
    ("pattern spread scale yaw", 0xc045733333333333),
    ("pattern random index", 0x0000000000000000),
    ("pattern random pitch", 0x4034000000000000),
    ("pattern random nan index", 0x3ff0000000000000),
    ("pattern random nan yaw", 0xc069000000000000),
    ("pattern odd pitch", 0x4039000000000000),
    ("pattern odd yaw", 0xc04e800000000000),
    ("pattern odd index", 0x0000000000000000),
    ("spin start", 0x3fd3333333333334),
    ("spin half", 0x3fc999999999999a),
    ("spin below one", 0x3fb999999999999a),
    ("spin off", 0x3fb999999999999a),
    ("spin odd", 0x3fbfaa25d8d79d0b),
    ("spin over", 0x3fb1eb851eb851ec),
    ("tick up", 0x3fd0000000000000),
    ("tick down", 0x3fd0000000000000),
    ("tick snap", 0x3ff0000000000000),
    ("tick snap down", 0x0000000000000000),
    ("tick odd", 0x3fd54d880bb3ee72),
    ("burst follows", 0x3fd0000000000000),
    ("burst no base", 0x3fe0000000000000),
    ("burst odd", 0x3fcd50edd50edd4f),
    ("pellet damage", 0x4036cccccccccccd),
    ("pellet odd", 0x403b19999999999a),
    ("pellet zero pellets", 0x403e761bed61bed7),
    ("falloff inside", 0x3ff0000000000000),
    ("falloff band", 0x3fe3333333333334),
    ("falloff floor", 0x3fc999999999999a),
    ("falloff outside", 0x0000000000000000),
    ("falloff full", 0x3ff0000000000000),
    ("falloff empty band", 0x3fe8000000000000),
    ("falloff odd band", 0x3fea4c6c23a4c6c2),
    ("falloff edge", 0x0000000000000000),
    ("falloff zero radius", 0x0000000000000000),
    ("explosion zero", 0x4070000000000000),
    ("explosion kept", 0x405e000000000000),
    ("status product", 0x3fb47ae147ae147b),
    ("status clamp", 0x3ff0000000000000),
    ("status odd", 0x3fb6da6b44aa508d),
    ("status negative", 0x0000000000000000),
    ("factor short", 0x3fe6a09e667f3bcd),
    ("factor long", 0x4010000000000000),
    ("factor no type", 0x3ff0000000000000),
    ("factor equal", 0x3ff0000000000000),
    ("factor odd short", 0x3feaa3069425f9ca),
    ("factor odd long", 0x40103e801780bd9e),
    ("radius status mid", 0x3fe0000000000000),
    ("radius status inside", 0x3ff0000000000000),
    ("radius status odd", 0x3fe39bc75a6ac1e8),
    ("radius status none", 0x0000000000000000),
    ("flight x", 0x408f400000000000),
    ("flight z", 0xc049400000000000),
    ("flight vz", 0xc059000000000000),
    ("flight age", 0x3ff0000000000003),
    ("gyro vx", 0x4097700000000000),
    ("gyro x", 0x4095c7026d8605f0),
    ("odd flight px", 0x4063aa1b677bafd6),
    ("odd flight py", 0xc05ac7bcd2d83c75),
    ("odd flight pz", 0xc038fcac88e4882c),
    ("odd flight vx", 0x406ca13c91d71f32),
    ("odd flight vy", 0xc063f674874e35c9),
    ("odd flight vz", 0xc061a269815de0a7),
    ("odd flight age", 0x3fe3bbbbbbbbbbbe),
    ("dot first", 0x0000000000000000),
    ("dot clock first", 0x3fc999999999999a),
    ("dot second", 0x4028000000000000),
    ("dot clock second", 0x3fb1eb851eb851ec),
    ("dot finish", 0x4000cccccccccccd),
    ("dot clock finish", 0x0000000000000000),
    ("dot steady total", 0x4034555555555553),
    ("dot steady clock", 0x3f947ae147ae1431),
    ("dot negative dt", 0x0000000000000000),
    ("dot negative clock", 0x3fb999999999999a),
    ("momentum", 0x4089000000000000),
    ("push x", 0x4028000000000000),
    ("push y", 0xc030000000000000),
    ("push z", 0x0000000000000000),
    ("push tiny", 0x4072c00000000000),
    ("push odd", 0x3fddbf193d4bb7e3),
    ("level equal", 0x3ff0000000000000),
    ("level one above", 0x3ff0000000000000),
    ("level one below", 0x3feccccccccccccd),
    ("level two above", 0x3ff4000000000000),
    ("level clamp above", 0x4008000000000000),
    ("level clamp below", 0x3fb999999999999a),
    ("level no table", 0x3ff0000000000000),
    ("pool reset", 0x3ff0000000000000),
    ("pool reset idle", 0x3fe0000000000000),
    ("pool impulse", 0x4010000000000000),
    ("pool impulse idle", 0x0000000000000000),
    ("pool delay", 0x4010000000000000),
    ("pool decay", 0x4008000000000000),
    ("pool decay idle", 0x3fe8000000000000),
    ("pool max", 0x4024000000000000),
    ("pool min", 0x3ff0000000000000),
    ("pool negative", 0x3ff0000000000000),
    ("pool default value", 0x4028000000000000),
    ("pool odd value", 0x4024369d0369d037),
    ("pool odd idle", 0x3fdbf258bf258bf2),
    ("cone zoomed yaw", 0x0000000000000000),
    ("cone half yaw", 0x4066c00000000000),
    ("cone odd pitch", 0xc06f6031851abe62),
    ("cone odd yaw", 0x4072c86c226809d5),
    ("cone nan zoom yaw", 0x4079bae147ae147b),
    ("kick amount", 0x4076c16bb98c7e28),
    ("kick amount zoomed", 0x40811110cb295e9e),
    ("kick amount odd", 0x406a97a51eca92d9),
    ("kick pitch", 0x4059000000000000),
    ("kick yaw", 0x0000000000000000),
    ("side pitch", 0x403e000000000000),
    ("side yaw", 0x4034000000000000),
    ("side tick target", 0x402e000000000000),
    ("side tick current", 0x4030ed4ad24e9617),
    ("side tick current yaw", 0x402691b91868c81e),
    ("side home target", 0x0000000000000000),
    ("side home current", 0x3e5992c48a95f963),
    ("chase one current", 0x4050ffa14cec41dd),
    ("chase one target", 0x408f3f51eb851eb8),
    ("chase one recover", 0x4058ff74bc6a7efa),
    ("hitch current", 0x408c11d888800000),
    ("steps current", 0x408c11d888800000),
    ("kick amount negative", 0xc076c16bb98c7e28),
    ("neg pitch", 0xc059000000000000),
    ("neg yaw", 0x0000000000000000),
    ("nan pitch", 0x0000000000000000),
    ("still current", 0x0000000000000000),
    ("odd kick target pitch", 0x40525610d6c3ecf6),
    ("odd kick target yaw", 0x40525610d6c3ecf8),
    ("odd kick cur pitch", 0x4041a59e9198d617),
    ("odd kick cur yaw", 0x4041a59e9198d61a),
    ("odd kick tgt pitch", 0x4043c5faa268bb09),
    ("odd kick recover", 0x3fc989374bc6a7f4),
    ("odd kick long cur pitch", 0x3fdc6c44d45f965b),
    ("bad nan", 0x4024000000000000),
    ("bad max", 0x4024000000000000),
    ("bad min", 0x3ff0000000000000),
    ("cone nan yaw", 0x0000000000000000),
];
