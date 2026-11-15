//! Owner: lane W1-ATTRMATH. The arithmetic of one `AttributeInitializationDefinition` (architecture section 3.5):
//! the value formula in both [`PowForm`]s, the base-value modes, and the range restriction with its rounding.
//! Pure `f32`; the evaluator that walks the definition and its resolvers is W3-ATTREVAL's (`eval.rs`).
//!
//! The rules are NATIVE_PROGRESSION.md section 1 (read from native code, **UNVERIFIED** as a whole). Two numbers
//! that run through them are confirmed in the game (REALGAME_GROUND_TRUTH.md, 2026-10-02): the level curve
//! `60 x (L^2.8 + 7.33)` for levels 1..59 and the maximum health base `80 x 1.13^L` at levels 2, 8, 17 and 70. The
//! tests below reproduce both under each pow form.
//!
//! # The pow forms (architecture section 5.3)
//!
//! `progression.cpp:248` raises in single precision (`powf`); `weapon_gen.cpp` (and `tools/weapon_recipe.py`) raise in
//! double and round to single after. Both are kept as [`PowForm`] until the game's form is settled (real-game
//! question 21); the default is [`PowForm::DoubleThenSingle`]. On the level curve (levels 1..80) and the health base
//! (levels 1..80) the two forms give the same bits on the reference platform (tested below), so the default passes
//! the verified fixtures and stands by section 5.3's rule. They are not the same function: where the double `pow`
//! lands next to the midpoint of two floats, `powf` can round to the other neighbour (about 1 input in 11,000 in a
//! random search of bases 0.5..100 and powers -2..6; one is pinned in `pow_forms_differ_next_to_a_midpoint`).
//! Every `+` and `*` here is one operation on two floats, so
//! `f32` arithmetic equals the oracles' "double, then round" (architecture 5.1 N4).

use ow_core::num::{RoundingMode, f32r, pow_f32, pow_f64, round_half_away_f32, round_mode};

/// Which reference implementation's power a value formula follows (module doc).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PowForm {
    /// `progression.cpp:248`: `powf(level, power)` in single precision.
    Single,
    /// `weapon_gen.cpp` `FormulaValue`, `weapon_recipe.formula_value`, the VM's `**`: `pow` in double, rounded to
    /// single.
    #[default]
    DoubleThenSingle,
}

/// `ValueFormula`: `Multiplier * (Level ^ Power + Offset)`. The offset is added **before** the multiplier, and the
/// power is skipped when it is exactly 1 (NATIVE_PROGRESSION.md section 1; report F4). Each term is a float the
/// evaluator has already computed.
///
/// Status: UNVERIFIED as a rule; the level curve and the health base that go through it are confirmed live.
pub fn formula(multiplier: f32, level: f32, power: f32, offset: f32, p: PowForm) -> f32 {
    let raised = if power == 1.0 {
        level
    } else {
        match p {
            PowForm::Single => pow_f32(level, power),
            // weapon_gen.cpp FormulaValue: F32(std::pow(level, power)) on doubles holding floats.
            PowForm::DoubleThenSingle => f32r(pow_f64(f64::from(level), f64::from(power))) as f32,
        }
    };
    multiplier * (raised + offset)
}

/// `EBaseValueMode`: how a definition's result `f` combines with the base `b`.
///
/// The names come from `Engine.upk` and `weapon_gen.cpp` pairs each name with its arithmetic; which stored number
/// 1..3 is which name was not checked against the native reading, which gives "1, 2, 3 = b + f, b x f, f - b" in
/// that order (NATIVE_PROGRESSION.md section 1, `progression.cpp:259-263`): UNVERIFIED.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BaseValueMode {
    /// `BASEVALUE_InitializationDefSetsBaseValue` (0, the cooked default): `f`.
    Sets = 0,
    /// `BASEVALUE_InitializationDefAddsToBaseValue`: `b + f`.
    Adds = 1,
    /// `BASEVALUE_InitializationDefScalesBaseValue`: `b * f`.
    Scales = 2,
    /// `BASEVALUE_InitializationDefOffsetByBaseValue`: `f - b`.
    FormulaMinusBase = 3,
}

impl BaseValueMode {
    /// The mode from its stored number; any other number is `None` (the C++ evaluator refuses it as
    /// "unsupported base value mode").
    pub fn from_raw(raw: i64) -> Option<Self> {
        match raw {
            0 => Some(Self::Sets),
            1 => Some(Self::Adds),
            2 => Some(Self::Scales),
            3 => Some(Self::FormulaMinusBase),
            _ => None,
        }
    }
}

/// A definition's result `def_value` combined with the base by `mode` (single precision, one operation).
///
/// Status: UNVERIFIED (NATIVE_PROGRESSION.md section 1).
pub fn combine_base(mode: BaseValueMode, base: f32, def_value: f32) -> f32 {
    match mode {
        BaseValueMode::Sets => def_value,
        BaseValueMode::Adds => base + def_value,
        BaseValueMode::Scales => base * def_value,
        BaseValueMode::FormulaMinusBase => def_value - base,
    }
}

/// `RangeRestriction`: the minimum first, then the maximum, each only when enabled (`None` when disabled). A
/// minimum above the maximum therefore ends at the maximum. A NaN value is left as it is (both oracles compare
/// `bound > value` / `value < bound`, false for NaN).
///
/// Status: UNVERIFIED (NATIVE_PROGRESSION.md section 1 step 4).
pub fn restrict(value: f32, min: Option<f32>, max: Option<f32>) -> f32 {
    let mut v = value;
    // progression.cpp: std::max(value, min) is "value < min ? min : value".
    if let Some(low) = min
        && v < low
    {
        v = low;
    }
    // std::min(value, max) is "max < value ? max : value".
    if let Some(high) = max
        && high < v
    {
        v = high;
    }
    v
}

/// The range restriction, then `RoundingMode` (0 none, 1 nearest, 2 floor, 3 ceiling), as `progression.cpp`
/// `restricted()` applies them when a definition is set (the XP curve and health-base evaluator).
///
/// Mode 1 there is `std::round` on the float: halves away from zero (`ow_core::num::round_half_away_f32`, so `-2.5`
/// gives `-3`). The other C++ evaluator, `weapon_gen.cpp` `AttributeValue`, rounds mode 1 as `floor(x + 0.5)`
/// (`-2.5` gives `-2`): a caller that follows that oracle uses `ow_core::num::round_mode(restrict(..), mode)`
/// directly. Modes 2 and 3 are the same in both (the floor or ceiling of a float is a float). Which form the game
/// uses is UNVERIFIED (architecture 5.3).
///
/// Status: UNVERIFIED (NATIVE_PROGRESSION.md section 1 step 4).
pub fn restrict_and_round(
    value: f32,
    min: Option<f32>,
    max: Option<f32>,
    rounding: RoundingMode,
) -> f32 {
    let v = restrict(value, min, max);
    match rounding {
        // progression.cpp:311: std::round(value).
        RoundingMode::HalfUp => round_half_away_f32(v),
        other => round_mode(v, other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stack::{CombineForm, NumKind, combine};
    use ow_core::fmt::fmt_f;
    use ow_core::num::{card_round, trunc_i32};
    use std::hint::black_box;

    const FORMS: [PowForm; 2] = [PowForm::DoubleThenSingle, PowForm::Single];

    /// The level curve's point `trunc(60 * (L^2.8 + 7.33))` (`progression.cpp` ExperienceCurve::point).
    fn xp_point(level: i32, p: PowForm) -> i64 {
        let level = black_box(level) as f32;
        i64::from(trunc_i32(formula(60.0, level, black_box(2.8), 7.33, p)))
    }

    /// Experience required for a level: `max(0, point(L) - point(1))`.
    fn xp_required(level: i32, p: PowForm) -> i64 {
        (xp_point(level, p) - xp_point(1, p)).max(0)
    }

    #[test]
    fn offset_goes_inside_the_multiplier() {
        // Report F4 / NATIVE_PROGRESSION.md section 1: {Multiplier 2, Level 3, Power 2, Offset 1} is 20, not 19.
        for p in FORMS {
            assert_eq!(formula(2.0, 3.0, 2.0, 1.0, p), 20.0);
        }
    }

    #[test]
    fn power_one_is_skipped() {
        for p in FORMS {
            assert_eq!(formula(3.0, -8.0, 1.0, 0.5, p), -22.5);
            assert_eq!(formula(1.0, 0.0, 1.0, 0.0, p), 0.0);
        }
    }

    #[test]
    fn level_curve_matches_the_live_numbers() {
        // REALGAME_GROUND_TRUTH.md: "Next level at" 1,241 (level 2), 28,126 (8), 196,238 (17), 9,155,282 (70), each
        // the curve's value for L + 1; the curve starts at 0 for level 1.
        for p in FORMS {
            assert_eq!(xp_required(1, p), 0);
            assert_eq!(xp_required(3, p), 1_241);
            assert_eq!(xp_required(9, p), 28_126);
            assert_eq!(xp_required(18, p), 196_238);
            assert_eq!(xp_required(71, p), 9_155_282);
            // NATIVE_PROGRESSION.md section 2: the spans R(L + 1) - R(L) for stages 7..11; 7,918 at stage 8 is the
            // span behind the live Fire mission XP of 395.
            let spans: Vec<i64> = (7..=11)
                .map(|l| xp_required(l + 1, p) - xp_required(l, p))
                .collect();
            assert_eq!(spans, [6_322, 7_918, 9_672, 11_579, 13_639]);
        }
    }

    #[test]
    fn level_curve_is_the_same_in_both_pow_forms() {
        // Architecture 5.3: the verified curve (levels 1..59, single precision) must hold under the default too.
        for level in 1..=80 {
            assert_eq!(
                xp_point(level, PowForm::DoubleThenSingle),
                xp_point(level, PowForm::Single),
                "level {level}"
            );
        }
    }

    #[test]
    fn health_base_matches_the_live_numbers() {
        // REALGAME_GROUND_TRUTH.md: the health pool's MaxValueBaseValue read live, 102.152 (L2), 212.6755 (L8),
        // 638.886 (L17), 415,509.44 (L70): max(20, 80 * 1.13^L) to float precision. Checked to the printed digits.
        let live = [
            (2, 102.152, 1e-3),
            (8, 212.6755, 1e-4),
            (17, 638.886, 1e-3),
            (70, 415_509.44, 1e-2),
        ];
        for p in FORMS {
            for (level, shown, unit) in live {
                let value = restrict(
                    formula(80.0, 1.13, black_box(level) as f32, 0.0, p),
                    Some(20.0),
                    None,
                );
                let value = f64::from(value);
                assert!(
                    (value - shown).abs() <= unit / 2.0,
                    "{p:?} level {level}: {value} vs {shown}"
                );
            }
        }
    }

    #[test]
    fn health_base_is_the_same_in_both_pow_forms() {
        for level in 1..=80 {
            let level = black_box(level) as f32;
            let single = formula(80.0, 1.13, level, 0.0, PowForm::Single);
            let double = formula(80.0, 1.13, level, 0.0, PowForm::DoubleThenSingle);
            assert_eq!(single.to_bits(), double.to_bits(), "level {level}");
        }
    }

    #[test]
    fn base_value_modes() {
        assert_eq!(combine_base(BaseValueMode::Sets, 4.0, 10.0), 10.0);
        assert_eq!(combine_base(BaseValueMode::Adds, 4.0, 10.0), 14.0);
        assert_eq!(combine_base(BaseValueMode::Scales, 4.0, 10.0), 40.0);
        assert_eq!(
            combine_base(BaseValueMode::FormulaMinusBase, 4.0, 10.0),
            6.0
        );
        assert_eq!(BaseValueMode::from_raw(0), Some(BaseValueMode::Sets));
        assert_eq!(
            BaseValueMode::from_raw(3),
            Some(BaseValueMode::FormulaMinusBase)
        );
        assert_eq!(BaseValueMode::from_raw(4), None);
    }

    #[test]
    fn restriction_min_then_max() {
        assert_eq!(restrict(5.0, Some(1.0), Some(10.0)), 5.0);
        assert_eq!(restrict(-3.0, Some(1.0), Some(10.0)), 1.0);
        assert_eq!(restrict(30.0, Some(1.0), Some(10.0)), 10.0);
        assert_eq!(restrict(30.0, None, None), 30.0);
        // A minimum above the maximum: the maximum is applied last and wins.
        assert_eq!(restrict(0.0, Some(8.0), Some(4.0)), 4.0);
        assert!(restrict(f32::NAN, Some(1.0), Some(10.0)).is_nan());
        // weapon_gen_test.py slot grade: base grade -2 clamped to [0, 4] is 0; 5 is 4.
        assert_eq!(restrict(-2.0, Some(0.0), Some(4.0)), 0.0);
        assert_eq!(restrict(5.0, Some(0.0), Some(4.0)), 4.0);
    }

    #[test]
    fn restriction_then_rounding() {
        // Positive cases, where both C++ readings of mode 1 agree.
        assert_eq!(
            restrict_and_round(2.5, None, None, RoundingMode::HalfUp),
            3.0
        );
        assert_eq!(
            restrict_and_round(2.4, None, None, RoundingMode::HalfUp),
            2.0
        );
        assert_eq!(
            restrict_and_round(2.7, None, None, RoundingMode::Floor),
            2.0
        );
        assert_eq!(
            restrict_and_round(2.1, None, None, RoundingMode::Ceiling),
            3.0
        );
        assert_eq!(
            restrict_and_round(2.25, None, None, RoundingMode::Float),
            2.25
        );
        // The clamp comes before the rounding: 0.4 raised to the minimum 1.5, then rounded up to 2.
        assert_eq!(
            restrict_and_round(0.4, Some(1.5), None, RoundingMode::Ceiling),
            2.0
        );
        assert_eq!(
            restrict_and_round(9.6, None, Some(9.5), RoundingMode::Floor),
            9.0
        );
    }

    #[test]
    fn mode_one_rounds_halves_away_from_zero_as_progression_does() {
        // progression.cpp:311 std::round: -2.5 gives -3, where weapon_gen's floor(x + 0.5) gives -2.
        assert_eq!(
            restrict_and_round(-2.5, None, None, RoundingMode::HalfUp),
            -3.0
        );
        assert_eq!(round_mode(-2.5, RoundingMode::HalfUp), -2.0);
        assert_eq!(
            restrict_and_round(-2.4, None, None, RoundingMode::HalfUp),
            -2.0
        );
        // The sign is kept: -0.3 rounds to -0.0.
        let z = restrict_and_round(-0.3, None, None, RoundingMode::HalfUp);
        assert_eq!(z.to_bits(), (-0.0f32).to_bits());
        // Floor and ceiling of a negative value.
        assert_eq!(
            restrict_and_round(-2.5, None, None, RoundingMode::Floor),
            -3.0
        );
        assert_eq!(
            restrict_and_round(-2.5, None, None, RoundingMode::Ceiling),
            -2.0
        );
    }

    #[test]
    fn pow_forms_differ_next_to_a_midpoint() {
        // Invented, found by a random search on the reference platform: level 24.865377 (0x41c6ec4b), power
        // 1.9730978 (0x3ffc8e78). The double pow is 567.0815734975, about 1e-8 above the midpoint 567.0815734863 of
        // the floats 567.08154 (0x440dc538) and 567.0816 (0x440dc539), so rounding it gives the upper one; powf gives
        // the lower. Multiplier 1, offset 0, so the formula returns the raised value itself. These bits are this
        // platform's powf and pow (architecture 5.1 N1).
        let (level, power) = (f32::from_bits(0x41c6_ec4b), f32::from_bits(0x3ffc_8e78));
        let single = formula(
            1.0,
            black_box(level),
            black_box(power),
            0.0,
            PowForm::Single,
        );
        let double = formula(
            1.0,
            black_box(level),
            black_box(power),
            0.0,
            PowForm::DoubleThenSingle,
        );
        assert_eq!(
            (single.to_bits(), double.to_bits()),
            (0x440d_c538, 0x440d_c539)
        );
    }

    #[test]
    fn card_rounding_cases_through_a_stored_value() {
        // tests/card_rounding_cases.json: each value stored as a float attribute (an empty stack under the
        // default form), then presented as weapon_gen.cpp Present does: |v| < 1e-8 is 0, damage ceil, magazine
        // floor, the others half up to one digit.
        let json = include_str!("../../../tests/card_rounding_cases.json");
        let mut seen = 0;
        for line in json.lines().filter(|l| l.contains("\"field\"")) {
            let field = between(line, "\"field\": \"", "\"");
            let value: f64 = between(line, "\"value\": ", ",").parse().expect("value");
            let text = between(line, "\"text\": \"", "\"");
            let stored =
                combine(value as f32, &[], CombineForm::default(), NumKind::Float).to_f32();
            let stored = if f64::from(stored).abs() < 1e-8 {
                0.0
            } else {
                stored
            };
            let shown = match field {
                // The card prints integers with no decimals and the others with one (weapon_gen.cpp's %.1f).
                "damage" => fmt_f(f64::from(round_mode(stored, RoundingMode::Ceiling)), 0),
                "magazine" => fmt_f(f64::from(round_mode(stored, RoundingMode::Floor)), 0),
                _ => fmt_f(f64::from(card_round(stored, 1)), 1),
            };
            assert_eq!(shown, text, "{field} {value}");
            seen += 1;
        }
        assert_eq!(seen, 16);
    }

    fn between<'a>(line: &'a str, start: &str, end: &str) -> &'a str {
        let from = line.find(start).expect("start") + start.len();
        let rest = &line[from..];
        &rest[..rest.find(end).expect("end")]
    }
}
