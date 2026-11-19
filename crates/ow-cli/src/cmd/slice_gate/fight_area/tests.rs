//! The fight-area step's grading on invented positions (AGENTS.md rule 9). AI-assisted (Claude Code).

use super::*;
use ow_assets::movement::Brush;

fn walked() -> Ran {
    Ran { steps: vec!["load".into(), "spawn".into(), "walk".into()], ..Ran::default() }
}

const AREA: FightArea = FightArea { offset: [100.0, -20.0, 5.0], radius: 50.0, floor_above: 160.0, floor_below: 2000.0 };

/// An invented box brush from `lo` to `hi`.
fn cuboid(lo: [f64; 3], hi: [f64; 3]) -> Brush {
    let mut points = Vec::new();
    for i in 0..8 {
        points.push([
            if i & 4 == 0 { lo[0] } else { hi[0] },
            if i & 2 == 0 { lo[1] } else { hi[1] },
            if i & 1 == 0 { lo[2] } else { hi[2] },
        ]);
    }
    Brush::from_hull(&points, &[], 0).expect("a box")
}

/// An invented flat floor whose top is at Z 0, 4000 uu across, and the brushes `more`.
fn floor_and(more: Vec<Brush>) -> CollisionWorld {
    let mut brushes = vec![cuboid([-2000.0, -2000.0, -100.0], [2000.0, 2000.0, 0.0])];
    brushes.extend(more);
    CollisionWorld::new(brushes)
}

/// An invented dummy cylinder.
const DUMMY: Shape = Shape::Cylinder { radius: 30.0, half_height: 50.0 };

fn check_text(f: Failed) -> String {
    match f {
        Failed::Check(t) => t,
        Failed::Vm(e) => panic!("expected a check, got {e}"),
    }
}

#[test]
fn the_centre_is_the_offset_from_the_player_start() {
    assert_eq!(centre(&AREA, [1.0, 2.0, 3.0]), [101.0, -18.0, 8.0]);
    const { assert!(FIGHT_AREA.radius > 0.0, "the fixture has a radius") };
    assert_eq!(horizontal([3.0, 4.0, 100.0], [0.0, 0.0, -100.0]), 5.0, "horizontal only");
}

/// Maya's end and the dummy both inside the area: the link is evident, with no stop, and the harness says it is a
/// port fixture.
#[test]
fn inside_the_area_the_link_is_evident() {
    let mut r = walked();
    graded(&mut r, &AREA, [0.0, 0.0, 0.0], [30.0, 40.0, 70.0], || Ok([0.0, 0.0, 0.0]));
    assert!(r.evident && r.stop.is_none(), "{:?}", r.stop);
    assert_eq!(r.steps.last().map(String::as_str), Some("fight_area"));
    assert!(r.harness.iter().any(|l| l.contains("PORT FIXTURE") && l.contains("ended 50.0 uu")), "{:?}", r.harness);
}

/// Outside the area, either one: an error at `fight_area`, not evident; a NaN position is outside.
#[test]
fn outside_the_area_is_an_error() {
    for (end, dummy, words) in [
        ([50.1, 0.0, 0.0], [0.0, 0.0, 0.0], "Maya's walk ended 50.1 uu"),
        ([0.0, 0.0, 0.0], [0.0, 60.0, 0.0], "the placed dummy stands 60.0 uu"),
        ([f64::NAN, 0.0, 0.0], [0.0, 0.0, 0.0], "Maya's walk ended "),
    ] {
        let mut r = walked();
        graded(&mut r, &AREA, [0.0, 0.0, 0.0], end, || Ok(dummy));
        let (outcome, stop) = r.stop.clone().expect("a stop");
        assert_eq!((outcome, stop.step.as_str()), (Outcome::Error, "fight_area"));
        assert!(stop.text.contains(words), "{}", stop.text);
        assert!(!r.evident);
    }
}

/// A spawn that refused keeps its kind: a `NotYet`'s text grades `notyet`, another text `error`.
#[test]
fn a_spawn_refusal_keeps_its_kind() {
    let not_yet = VmError::from(NotYet::not_read("an invented spawn step", "invented"));
    let mut r = walked();
    graded(&mut r, &AREA, [0.0; 3], [0.0; 3], || Err(Failed::Vm(not_yet)));
    assert_eq!(r.stop.as_ref().map(|(o, s)| (*o, s.step.as_str())), Some((Outcome::NotYet, "fight_area")));
    assert!(!r.evident);
    let mut r = walked();
    graded(&mut r, &AREA, [0.0; 3], [0.0; 3], || Err(Failed::Check("the spawn path refused the dummy (no actor made)".into())));
    assert_eq!(r.stop.as_ref().map(|(o, _)| *o), Some(Outcome::Error));
}

/// Review r1 of #470: a hard error whose text is a `NotYet`'s word for word is still an `error`; the grading goes by
/// the typed variant, never by the text.
#[test]
fn a_hard_error_with_a_not_yet_text_stays_an_error() {
    let not_yet = VmError::from(NotYet::not_read("an invented spawn step", "invented"));
    let same_text = ow_combat::hit::access::error_text(&not_yet);
    for hard in [VmError::Decode(same_text.clone()), VmError::Decode(format!("an invented decode failure: {same_text}"))] {
        let mut r = walked();
        graded(&mut r, &AREA, [0.0; 3], [0.0; 3], || Err(Failed::Vm(hard)));
        assert_eq!(r.stop.as_ref().map(|(o, s)| (*o, s.step.as_str())), Some((Outcome::Error, "fight_area")));
        assert!(!r.evident);
    }
    let mut r = walked();
    graded(&mut r, &AREA, [0.0; 3], [0.0; 3], || Err(Failed::Check(same_text)));
    assert_eq!(r.stop.as_ref().map(|(o, _)| *o), Some(Outcome::Error), "a failed check is an error, whatever its text");
}

/// The floor under the centre: found on an invented flat floor, the dummy standing on it (its centre its half height
/// above the top); none, a start in solid or a slope too steep is an error.
#[test]
fn the_dummy_stands_on_a_walkable_floor_under_the_centre() {
    let maya = MovementParams::maya();
    let spot = floor_spot(&floor_and(vec![]), &AREA, &maya, DUMMY, [100.0, 0.0, 90.0]).expect("a floor");
    assert_eq!((spot[0], spot[1]), (100.0, 0.0));
    assert!((spot[2] - 50.0).abs() < 1.0, "the dummy's centre is its half height above the floor: {}", spot[2]);
    // No floor within reach below the centre.
    let none = floor_spot(&floor_and(vec![]), &AREA, &maya, DUMMY, [100.0, 0.0, 5000.0]).unwrap_err();
    assert!(check_text(none).contains("no floor"));
    // The search starts in solid: a block over the centre.
    let block = floor_and(vec![cuboid([0.0, -100.0, 0.0], [200.0, 100.0, 1000.0])]);
    let solid = floor_spot(&block, &AREA, &maya, DUMMY, [100.0, 0.0, 90.0]).unwrap_err();
    assert!(check_text(solid).contains("in solid"));
    // A steep wedge under the centre: not walkable.
    let points = [
        [0.0, -500.0, 0.0], [0.0, 500.0, 0.0], [200.0, -500.0, 0.0], [200.0, 500.0, 0.0],
        [200.0, -500.0, 400.0], [200.0, 500.0, 400.0],
    ];
    let steep = floor_and(vec![Brush::from_hull(&points, &[], 1).expect("a wedge")]);
    let slope = floor_spot(&steep, &AREA, &maya, DUMMY, [100.0, 0.0, 600.0]).unwrap_err();
    assert!(check_text(slope).contains("not walkable"));
}

/// Maya's reach: on the same flat floor with nothing between, reachable; a wall between, a floor one storey below
/// (a dummy under a deck Maya stands on) or above, an error.
#[test]
fn maya_must_reach_the_dummy_on_the_same_floor() {
    let maya = MovementParams::maya();
    let half = maya.shape.half_height();
    let world = floor_and(vec![]);
    let at = floor_spot(&world, &AREA, &maya, DUMMY, [150.0, 0.0, 90.0]).unwrap();
    reachable(&world, &maya, [0.0, 0.0, half + 1.0], DUMMY, at).expect("reachable");
    // A wall between them.
    let walled = floor_and(vec![cuboid([60.0, -500.0, 0.0], [80.0, 500.0, 1000.0])]);
    let wall = reachable(&walled, &maya, [0.0, 0.0, half + 1.0], DUMMY, at).unwrap_err();
    assert!(check_text(wall).contains("no straight way"));
    // Maya on a deck 300 uu up, the dummy on the floor below.
    let deck = floor_and(vec![cuboid([-500.0, -500.0, 280.0], [500.0, 500.0, 300.0])]);
    let below = reachable(&deck, &maya, [0.0, 0.0, 300.0 + half + 1.0], DUMMY, at).unwrap_err();
    assert!(check_text(below).contains("not the same floor"));
    // Or the dummy 300 uu above her.
    let above = reachable(&world, &maya, [0.0, 0.0, half + 1.0], DUMMY, [150.0, 0.0, at[2] + 300.0]).unwrap_err();
    assert!(check_text(above).contains("not the same floor"));
    // A NaN end is never reachable.
    assert!(reachable(&world, &maya, [f64::NAN, 0.0, half], DUMMY, at).is_err());
}

/// Maya is checked first: outside the area, the dummy is never placed.
#[test]
fn the_dummy_is_placed_only_after_maya_arrives() {
    let mut r = walked();
    let mut asked = false;
    graded(&mut r, &AREA, [0.0; 3], [500.0, 0.0, 0.0], || {
        asked = true;
        Ok([0.0; 3])
    });
    assert!(!asked, "no placement for a walk that missed the area");
    assert_eq!(r.stop.as_ref().map(|(o, s)| (*o, s.step.as_str())), Some((Outcome::Error, "fight_area")));
}
