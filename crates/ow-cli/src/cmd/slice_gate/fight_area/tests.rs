//! The fight-area step's grading on invented positions (AGENTS.md rule 9). AI-assisted (Claude Code).

use super::*;

fn walked() -> Ran {
    Ran { steps: vec!["load".into(), "spawn".into(), "walk".into()], ..Ran::default() }
}

const AREA: FightArea = FightArea { offset: [100.0, -20.0, 5.0], radius: 50.0 };

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
    let not_yet = ow_vm::error::VmError::from(ow_core::NotYet::not_read("an invented spawn step", "invented"));
    let mut r = walked();
    graded(&mut r, &AREA, [0.0; 3], [0.0; 3], || Err(text(&not_yet)));
    assert_eq!(r.stop.as_ref().map(|(o, s)| (*o, s.step.as_str())), Some((Outcome::NotYet, "fight_area")));
    assert!(!r.evident);
    let mut r = walked();
    graded(&mut r, &AREA, [0.0; 3], [0.0; 3], || Err("the spawn path refused the dummy (no actor made)".into()));
    assert_eq!(r.stop.as_ref().map(|(o, _)| *o), Some(Outcome::Error));
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
