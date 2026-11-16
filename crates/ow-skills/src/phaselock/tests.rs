//! Tests of the Phaselock rules. The numbers are the records' (data constants and the derived timeline of
//! PHASELOCK_STOCK_DATA.md, the score oracle and the magnetism table of NATIVE_PHASELOCK_TARGETING.md section 3), which
//! the docs may hold; everything else is invented (AGENTS.md rule 9).

use super::oracle::{bob_target_z, vinterp_to};
use super::*;

/// Maya's lock duration with Suspension at `grade`, from the record's data (base 5; Suspension PostAdd 0.5 at grade 1,
/// +0.5 per grade: PHASELOCK_STOCK_DATA.md), through the stack rule. A test helper: the game reads both numbers from
/// the installed data, and other modifiers (Thoughtlock and the rest) join the same stack.
fn maya_lock_duration(grade: i32) -> f32 {
    if grade < 1 {
        return lock_duration(5.0, &[]);
    }
    // grade >= 1: the effect's steps are grade - 1 (int -> float, exact for a tree's grades).
    let post = 0.5 + 0.5 * (grade - 1) as f32;
    lock_duration(5.0, &[(ModifierType::PostAdd, post)])
}

/// Phaselock's duration `5 + 0.5 g` for Suspension grades 0 to 5 (the lane's named rule).
#[test]
fn the_lock_duration_is_five_plus_half_the_suspension_grade() {
    let got: Vec<f32> = (0..=5).map(maya_lock_duration).collect();
    assert_eq!(got, vec![5.0, 5.5, 6.0, 6.5, 7.0, 7.5]);
}

/// The lock duration goes through the stack rule: PreAdd before the scale, PostAdd after it.
#[test]
fn the_lock_duration_is_the_attribute_stack_rule() {
    let mods = [
        (ModifierType::PreAdd, 1.0),
        (ModifierType::Scale, 0.5),
        (ModifierType::PostAdd, 0.5),
    ];
    assert_eq!(lock_duration(5.0, &mods), 9.5);
    assert_eq!(lock_duration(5.0, &[]), 5.0);
}

/// The timeline of PHASELOCK_STOCK_DATA.md (data: lift 0.7, fade-out 1.1, release buffer 1, cooldown 13): the base
/// first lock (5.7 / 0.7 / 4.6 / 5.7 / 6.7; the release and the end confirmed live on 2026-11-12) and Suspension's
/// rows (5.7 + 0.5 g and so on). The cooldown is ready 18.7 s after a base cast.
#[test]
fn the_schedule_of_a_cast_matches_the_records_timeline() {
    let base = Schedule::new(0.7, maya_lock_duration(0), 1.0, 1.1, 1.0);
    assert_eq!(base.skill_duration.to_bits(), (0.7f32 + 5.0).to_bits());
    assert_eq!(base.lock_at, 0.7);
    assert!((base.skill_duration - 5.7).abs() < 1e-6);
    assert!((base.outro_at - 4.6).abs() < 1e-6);
    assert!((base.end_at - 6.7).abs() < 1e-6);
    assert!((base.cooldown_ready(13.0) - 18.7).abs() < 1e-5);
    for g in 1..=5 {
        let s = Schedule::new(0.7, maya_lock_duration(g), 1.0, 1.1, 1.0);
        let extra = 0.5 * g as f32;
        assert!((s.release_at - (5.7 + extra)).abs() < 1e-5, "g {g}");
        assert!((s.outro_at - (4.6 + extra)).abs() < 1e-5, "g {g}");
        assert!((s.end_at - (6.7 + extra)).abs() < 1e-5, "g {g}");
    }
}

/// A target locked again within 25 s: the stack rule divides by 1.4 (a finding: the record's row assumed 0.6).
#[test]
fn a_repeat_target_scale_is_the_stack_rules_division() {
    let scale = repeat_target_scale();
    assert!((f64::from(scale) - 1.0 / 1.4).abs() < 1e-6, "{scale}");
    let repeat = Schedule::new(0.7, maya_lock_duration(0), scale, 1.1, 1.0);
    assert!((repeat.release_at - (0.7 + 5.0 / 1.4)).abs() < 1e-5);
    assert!(
        (repeat.release_at - 3.7).abs() > 0.5,
        "not the record's 3.7 s row"
    );
}

fn default_aim() -> AutoAim {
    // GD_Autoaim.Default (data, NATIVE_PHASELOCK_TARGETING.md section 2).
    AutoAim::new(50_000.0, 0.0, 0.8, 0.5, 275.0)
}

/// The magnetism table of section 3 (two decimals).
#[test]
fn the_magnetism_radius_matches_the_records_table() {
    let a = default_aim();
    for (depth, expect) in [
        (300.0, 0.38),
        (500.0, 0.28),
        (1_000.0, 0.23),
        (2_000.0, 0.19),
        (5_000.0, 0.14),
        (10_000.0, 0.11),
    ] {
        let r = magnetism(&a, depth);
        assert!((r - expect).abs() < 0.005, "depth {depth}: {r}");
    }
}

/// The record's synthetic oracle: depth 1,000, the target's own radius 0.05, offset 0.20 gives about 0.55; offset
/// 0.25 is outside the cone and scores 0.
#[test]
fn the_score_matches_the_records_oracle() {
    let a = default_aim();
    // rT = radius x 0.8 / (depth x tan) = 0.05 with depth 1,000 and tan 1: radius 62.5.
    let c = Candidate::new(true, 0.20, 0.0, 1_000.0, 62.5, true);
    let s = score(&a, &c, 1.0);
    assert!((s.score - 0.55).abs() < 0.01, "{}", s.score);
    assert!(!s.within_world_space_radius);
    let off = Candidate { x: 0.25, ..c };
    assert_eq!(score(&a, &off, 1.0).score, 0.0);
    let close = Candidate { x: 0.04, ..c };
    assert!(score(&a, &close, 1.0).within_world_space_radius);
}

#[test]
fn the_score_gates_and_the_pick() {
    let a = AutoAim::new(50_000.0, 100.0, 0.8, 0.5, 275.0);
    let c = Candidate::new(true, 0.1, 0.0, 1_000.0, 62.5, true);
    assert_eq!(
        score(
            &a,
            &Candidate {
                in_front: false,
                ..c
            },
            1.0
        )
        .score,
        0.0,
        "behind the view plane"
    );
    assert_eq!(
        score(
            &a,
            &Candidate {
                line_of_sight: false,
                ..c
            },
            1.0
        )
        .score,
        0.0,
        "blocked"
    );
    assert_eq!(
        score(
            &a,
            &Candidate {
                depth: 50.0,
                x: 0.0,
                ..c
            },
            1.0
        )
        .score,
        0.0,
        "nearer than the minimum"
    );
    assert_eq!(
        score(
            &a,
            &Candidate {
                depth: 60_000.0,
                x: 0.0,
                ..c
            },
            1.0
        )
        .score,
        0.0,
        "beyond the maximum"
    );
    let near_centre = Candidate { x: 0.01, ..c };
    let far = Candidate {
        x: 0.01,
        depth: 5_000.0,
        ..c
    };
    assert_eq!(
        pick_target(&a, &[c, near_centre, far], 1.0),
        Some(1),
        "near the crosshair first, then near the player"
    );
    assert_eq!(
        pick_target(&a, &[c, c], 1.0),
        Some(0),
        "ties keep the first"
    );
    assert_eq!(
        pick_target(
            &a,
            &[Candidate {
                line_of_sight: false,
                ..c
            }],
            1.0
        ),
        None
    );
}

/// The bob oracle: amplitude 30, frequency 0.5 (a 4 s period), from the cast; and `VInterpTo`'s rule.
#[test]
fn the_bob_and_vinterp_to() {
    assert_eq!(bob_target_z(100.0, 30.0, 0.5, 0.0), 100.0);
    assert!(
        (bob_target_z(100.0, 30.0, 0.5, 1.0) - 130.0).abs() < 1e-3,
        "a quarter period"
    );
    assert!((bob_target_z(100.0, 30.0, 0.5, 3.0) - 70.0).abs() < 1e-3);
    let t = [0.0, 0.0, 30.0];
    assert_eq!(vinterp_to([0.0; 3], t, 0.1, 0.0), t, "no speed: the target");
    assert_eq!(
        vinterp_to([0.0, 0.0, 0.0], [0.0, 0.0, 0.009], 0.1, 1.0),
        [0.0, 0.0, f64::from(0.009f32)],
        "close: the target"
    );
    assert_eq!(vinterp_to([0.0; 3], t, 0.1, 1.0), [0.0, 0.0, 3.0]);
    assert_eq!(
        vinterp_to([0.0; 3], t, 5.0, 1.0),
        t,
        "clamped to the whole difference"
    );
}
