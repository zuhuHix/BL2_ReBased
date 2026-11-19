//! Owner: lane H-MATINEE. The curve rules on invented keys (AGENTS.md rule 9).

use super::*;

fn key(time: f32, value: [f32; 3], mode: Mode) -> Key {
    Key { time, value, arrive: [0.0; 3], leave: [0.0; 3], mode }
}

fn curve(keys: Vec<Key>) -> Curve {
    Curve { keys, broken: false }
}

#[test]
fn no_keys_is_zero_and_one_key_holds() {
    assert_eq!(curve(Vec::new()).eval(3.0), [0.0; 3]);
    let one = curve(vec![key(2.0, [1.0, 2.0, 3.0], Mode::Linear)]);
    for t in [-1.0, 2.0, 9.0] {
        assert_eq!(one.eval(t), [1.0, 2.0, 3.0]);
    }
}

#[test]
fn the_ends_hold_without_extrapolation() {
    let c = curve(vec![key(1.0, [10.0, 0.0, 0.0], Mode::Linear), key(3.0, [30.0, 0.0, 0.0], Mode::Linear)]);
    assert_eq!(c.eval(0.0), [10.0, 0.0, 0.0]);
    assert_eq!(c.eval(1.0), [10.0, 0.0, 0.0]);
    assert_eq!(c.eval(3.0), [30.0, 0.0, 0.0]);
    assert_eq!(c.eval(7.0), [30.0, 0.0, 0.0]);
}

#[test]
fn the_previous_key_s_mode_picks_the_segment_rule() {
    let to = key(2.0, [20.0, -4.0, 8.0], Mode::Constant);
    // Linear: halfway is the mean.
    let linear = curve(vec![key(0.0, [0.0; 3], Mode::Linear), to]);
    assert_eq!(linear.eval(1.0), [10.0, -2.0, 4.0]);
    assert_eq!(linear.eval(0.5), [5.0, -1.0, 2.0]);
    // Constant: the previous value until the next key.
    let constant = curve(vec![key(0.0, [1.0; 3], Mode::Constant), to]);
    assert_eq!(constant.eval(1.999), [1.0; 3]);
    assert_eq!(constant.eval(2.0), [20.0, -4.0, 8.0]);
    // A curve with zero tangents is the smoothstep 3s^2 - 2s^3: at s = 0.25, 0.15625.
    let smooth = curve(vec![key(0.0, [0.0; 3], Mode::Curve), key(4.0, [100.0, 0.0, 0.0], Mode::Linear)]);
    assert_eq!(smooth.eval(1.0), [15.625, 0.0, 0.0]);
    assert_eq!(smooth.eval(2.0), [50.0, 0.0, 0.0]);
}

#[test]
fn tangents_scale_by_the_segment_length_unless_broken() {
    // From 0 to 0 over 2 s with leave tangent 1 and arrive tangent 0: at s = 0.5, h10 = 0.125.
    let mut a = key(0.0, [0.0; 3], Mode::Curve);
    a.leave = [1.0, 0.0, 0.0];
    let b = key(2.0, [0.0; 3], Mode::Linear);
    let c = Curve { keys: vec![a, b], broken: false };
    assert_eq!(c.eval(1.0), [0.25, 0.0, 0.0]);
    let broken = Curve { keys: vec![a, b], broken: true };
    assert_eq!(broken.eval(1.0), [0.125, 0.0, 0.0]);
}

#[test]
fn two_keys_at_one_time_the_later_starts_the_next_segment() {
    // Two keys at the same time, then a later one: just after the pair, the second of the pair starts the segment.
    let c = curve(vec![
        key(1.0, [1.0; 3], Mode::Linear),
        key(1.0, [5.0; 3], Mode::Linear),
        key(3.0, [9.0; 3], Mode::Linear),
    ]);
    assert_eq!(c.eval(2.0), [7.0; 3]);
}

#[test]
fn the_key_pose_turns_euler_degrees_into_truncated_rotator_units() {
    let track = MoveTrack {
        position: curve(vec![key(0.0, [1.0, 2.0, 3.0], Mode::Linear)]),
        // X roll 90, Y pitch -45, Z yaw 1 degree (182.04 units, truncated to 182).
        euler: curve(vec![key(0.0, [90.0, -45.0, 1.0], Mode::Linear)]),
        frame: MoveFrame::RelativeToInitial,
        rot_mode: RotMode::Keyframed,
        disable_movement: false,
    };
    assert_eq!(track.key_pose(0.0), ([1.0, 2.0, 3.0], Rotator::new(-8192, 182, 16384)));
    assert_eq!(track.length(), 0.0);
}

#[test]
fn disabled_movement_evaluates_at_time_zero() {
    let moving = curve(vec![key(0.0, [0.0; 3], Mode::Linear), key(1.0, [10.0, 0.0, 0.0], Mode::Linear)]);
    let mut track = MoveTrack {
        position: moving,
        euler: Curve::default(),
        frame: MoveFrame::World,
        rot_mode: RotMode::Ignore,
        disable_movement: false,
    };
    assert_eq!(track.key_pose(0.5).0, [5.0, 0.0, 0.0]);
    track.disable_movement = true;
    assert_eq!(track.key_pose(0.5).0, [0.0; 3]);
    assert_eq!(track.length(), 1.0);
}
