//! Owner: lane W3-BALANCE. The experience curve: the experience required to reach a level (NATIVE_PROGRESSION.md
//! section 3; the internal routine behind `WillowPlayerController.GetExpPointsRequiredForLevel`, the mission reward
//! and the level-up).
//!
//! ```text
//! f(n) = the curve definition evaluated with n in global slot 0     (60 x (n ^ 2.8 + 7.33) in the game's data)
//! R(n) = max(0, trunc(f(n)) - trunc(f(1)))
//! ```
//!
//! The level reaches the formula through `GlobalAttributeValueResolver` slot 0 (`ExperiencePointTestLevel`), which the
//! routine sets to `n` before it evaluates: here [`AttrWorld::global_slots`](ow_attr::api::AttrWorld::global_slots).
//! The definition is evaluated by the one evaluator (`ow-attr`), as `progression.cpp` `ExperienceCurve` does with its
//! own: a definition on its own is `AttributeInitializationData { constant 0, no attribute, the definition, scale 1 }`
//! (the C++ `evaluateDefinition`, `restricted(combined(definition, 0))`, gives the same steps).
//!
//! **Status: verified** for levels 1..59 (REALGAME_GROUND_TRUTH.md, 2026-10-02: `GetExpPointsRequiredForLevel` called
//! for 1..80 in the game; the single-precision form reproduces every level 1..59). Four levels above the base cap (60,
//! 68, 74, 79) are one point off that form in the game; this port does not reproduce them (the record's guess is the
//! C runtime's float `pow`), so levels above 59 are UNVERIFIED. The game caches `trunc(f(1))` and refreshes it when
//! the globals change; this port evaluates it each time, which gives the same number.

use ow_attr::AttributeSystem;
use ow_attr::api::{EvalCx, InitData};
use ow_object::ids::ObjRef;

use crate::types::{BalanceError, ExpLevel};

/// The record the curve comes from.
pub const RECORD: &str = "NATIVE_PROGRESSION.md section 3";

/// `trunc(f(level))`: the curve definition evaluated with `level` in global slot 0, truncated toward zero.
///
/// The slot is left at `level`, as the game's routine leaves it (it sets the slot and does not restore it; what else
/// reads slot 0 afterwards is not read). A result that is NaN or outside the `i64` range is refused as
/// [`BalanceError::OutOfRange`] (the C++ `int64_t(std::trunc(x))` has no defined answer there); the game's own type
/// is a script `int`, so a caller that hands the number to script must check it fits.
pub fn experience_point(
    sys: &AttributeSystem,
    cx: &mut EvalCx<'_>,
    curve: ObjRef,
    level: ExpLevel,
) -> Result<i64, BalanceError> {
    // The level as the slot's float: int -> float rounds to nearest, as the C++ `float(level)`.
    let slot = level.get() as f32;
    cx.world.global_slots().set(0, slot)?;
    let data = InitData {
        initialization_definition: Some(curve),
        ..InitData::constant(0.0)
    };
    let f = sys.evaluate_init(&data, None, None, cx)?;
    trunc_i64(f)
}

/// The experience required to reach `level` from level 1: `max(0, trunc(f(level)) - trunc(f(1)))`.
///
/// `curve` is the `AttributeInitializationDefinition` the globals name (`GlobalsDefinition.ExpPointsRequiredForLevel`
/// in the game's data). Level 1 is evaluated first, then `level`, so slot 0 ends at `level`.
pub fn experience_required(
    sys: &AttributeSystem,
    cx: &mut EvalCx<'_>,
    curve: ObjRef,
    level: ExpLevel,
) -> Result<i64, BalanceError> {
    let first = experience_point(sys, cx, curve, ExpLevel::new(1))?;
    let point = experience_point(sys, cx, curve, level)?;
    let required = point.checked_sub(first).ok_or(BalanceError::OutOfRange {
        what: "experience required (trunc(f(n)) - trunc(f(1)))",
        value: point,
    })?;
    Ok(required.max(0))
}

/// The experience a level spans: `R(level + 1) - R(level)` (the mission reward's span, NATIVE_PROGRESSION.md section
/// 2).
pub fn experience_span(
    sys: &AttributeSystem,
    cx: &mut EvalCx<'_>,
    curve: ObjRef,
    level: ExpLevel,
) -> Result<i64, BalanceError> {
    let next = level.get().checked_add(1).ok_or(BalanceError::OutOfRange {
        what: "experience span level (level + 1 overflows)",
        value: i64::from(level.get()),
    })?;
    let low = experience_required(sys, cx, curve, level)?;
    let high = experience_required(sys, cx, curve, ExpLevel::new(next))?;
    high.checked_sub(low).ok_or(BalanceError::OutOfRange {
        what: "experience span (R(L + 1) - R(L))",
        value: high,
    })
}

/// `int64_t(std::trunc(x))` for an `x` that has an answer: toward zero; NaN and anything outside the `i64` range are
/// refused, not saturated.
///
/// Why a local form (N5): `ow_core::num` has no float-to-`i64` conversion, and its two forms answer a value for every
/// input (`trunc_i32` gives `i32::MIN` out of range; `trunc_i64_wrap_i32` wraps to 32 bits). The curve's points pass
/// `i32` near level 500 (60 x n^2.8 reaches 2^31 there), and the C++ has no defined answer outside `i64`, so the
/// curve needs a form that **refuses** instead. The one `as` below runs only inside the checked range, where it is
/// exact. A doc follow-up for the hub: move this refusing form into `ow_core::num` (this lane does not edit
/// `ow-core`).
fn trunc_i64(x: f32) -> Result<i64, BalanceError> {
    // 2^63 is exact in f32; NaN is in no range.
    if !(-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0).contains(&x) {
        return Err(BalanceError::OutOfRange {
            what: "experience curve point (NaN or outside the i64 range)",
            value: 0,
        });
    }
    // In range: `as` truncates toward zero exactly (no saturation can happen here).
    Ok(x as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::offline::fixture::Fx;
    use ow_attr::math::PowForm;
    use ow_core::num::pow_f32;
    use std::hint::black_box;

    /// The record's closed form, written independently of the evaluator: `trunc(f32(60 x (n^2.8 + 7.33)))` with a
    /// single-precision pow, add and multiply (REALGAME_GROUND_TRUTH.md, "Level curve").
    fn record_point(n: i32) -> i64 {
        let level = black_box(n) as f32;
        let f = 60.0f32 * (pow_f32(level, black_box(2.8)) + 7.33);
        trunc_i64(f).unwrap()
    }

    #[test]
    fn the_curve_reproduces_the_live_levels_one_to_fifty_nine() {
        // Verified live for levels 1..59 (REALGAME_GROUND_TRUTH.md): the single-precision closed form. Checked under
        // both pow forms (architecture 5.3: the verified curve must hold under the default too).
        for pow in [PowForm::DoubleThenSingle, PowForm::Single] {
            let mut fx = Fx::new();
            let curve = fx.xp_curve();
            let (sys, mut world) = fx.into_world();
            let mut cx = EvalCx::new(&mut world);
            cx.forms.pow = pow;
            for n in 1..=59 {
                let want = (record_point(n) - record_point(1)).max(0);
                let got = experience_required(&sys, &mut cx, curve, ExpLevel::new(n)).unwrap();
                assert_eq!(got, want, "{pow:?} level {n}");
            }
        }
    }

    #[test]
    fn the_curve_gives_the_record_numbers() {
        // NATIVE_PROGRESSION.md section 3: R(2) 358, R(3) 1,241, R(5) 5,376, R(8) 20,208, R(9) 28,126, R(46)
        // 2,715,586; REALGAME_GROUND_TRUTH.md "Next level at" (R(L + 1)): 196,238 at 17, 9,155,282 at 70.
        let mut fx = Fx::new();
        let curve = fx.xp_curve();
        let (sys, mut world) = fx.into_world();
        let mut cx = EvalCx::new(&mut world);
        let mut r = |n| experience_required(&sys, &mut cx, curve, ExpLevel::new(n)).unwrap();
        assert_eq!(r(1), 0);
        assert_eq!(r(2), 358);
        assert_eq!(r(3), 1_241);
        assert_eq!(r(5), 5_376);
        assert_eq!(r(8), 20_208);
        assert_eq!(r(9), 28_126);
        assert_eq!(r(18), 196_238);
        assert_eq!(r(46), 2_715_586);
        assert_eq!(r(71), 9_155_282);
    }

    #[test]
    fn spans_and_the_slot() {
        // NATIVE_PROGRESSION.md section 2: the spans for stages 7..11 (7,918 at stage 8 is behind the live 395).
        let mut fx = Fx::new();
        let curve = fx.xp_curve();
        let (sys, mut world) = fx.into_world();
        let mut cx = EvalCx::new(&mut world);
        let spans: Vec<i64> = (7..=11)
            .map(|l| experience_span(&sys, &mut cx, curve, ExpLevel::new(l)).unwrap())
            .collect();
        assert_eq!(spans, [6_322, 7_918, 9_672, 11_579, 13_639]);
        // The routine leaves slot 0 at the last level it evaluated.
        experience_required(&sys, &mut cx, curve, ExpLevel::new(8)).unwrap();
        assert_eq!(cx.world.global_slots().get(0), 8.0);
    }

    #[test]
    fn a_level_below_one_is_zero_and_an_undefined_point_is_refused() {
        let mut fx = Fx::new();
        let curve = fx.xp_curve();
        let (sys, mut world) = fx.into_world();
        let mut cx = EvalCx::new(&mut world);
        // f(0) = 60 x 7.33 = 439.8 < f(1): the max(0, ..) of the record.
        assert_eq!(
            experience_required(&sys, &mut cx, curve, ExpLevel::new(0)).unwrap(),
            0
        );
        // A negative level to a non-integer power is NaN: refused, never a saturated number.
        assert!(matches!(
            experience_required(&sys, &mut cx, curve, ExpLevel::new(-1)),
            Err(BalanceError::OutOfRange { .. })
        ));
        assert!(trunc_i64(f32::NAN).is_err());
        assert!(trunc_i64(9.3e18).is_err());
        assert_eq!(trunc_i64(-2.9).unwrap(), -2);
    }
}
