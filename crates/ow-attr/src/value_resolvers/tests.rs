//! The evaluator's tests on synthetic worlds (`testworld.rs`; rule 9). Each fixture of the current evaluators that
//! the architecture's `ow-attr` gate lists is re-expressed here against the one evaluator, with the source named on
//! the test: `tests/mission_script_test.py` (experience curve, reward percentage, skill points, health base),
//! `tests/combat_test.py` (kill experience, the random quantity), `tests/weapon_gen_test.py` (the `Weapon_Is_Acme`
//! terms and the damage stack), `tests/skill_stats_test.py` (a designer attribute as the base), `math.rs`'s record
//! constants (level curve and health base, REALGAME_GROUND_TRUTH.md). Shapes outside the read set return `NotYet`,
//! each with a test.

use std::hint::black_box;

use ow_core::Missing;
use ow_core::num::trunc_i32;
use ow_core::rand::{CrtRand, CrtStream, Rand15};
use ow_defs::DefStore;
use ow_object::heap::Origin;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;

use super::CLASSES;
use super::testworld::{Fx, system};
use crate::api::{
    AttrError, Comparison, EvalCx, ExprData, ExprMode, InitData, NumForms, Operand2Usage, Storage,
};
use crate::math::PowForm;
use crate::registry::AttributeSystem;
use crate::stack::{CombineForm, Combined, Modifier, ModifierId, ModifierType, NumKind, combine};

const FORMS: [PowForm; 2] = [PowForm::Single, PowForm::DoubleThenSingle];
const ADD: u8 = 0;
const SUB: u8 = 1;
const MUL: u8 = 2;
const DIV: u8 = 3;

fn forms(pow: PowForm) -> NumForms {
    NumForms {
        pow,
        combine: CombineForm::default(),
    }
}

fn missing(e: AttrError) -> Missing {
    match e {
        AttrError::NotYet(n) => n.missing,
        other => panic!("expected NotYet, got {other:?}"),
    }
}

fn is_bad_data(e: &AttrError, needle: &str) -> bool {
    matches!(e, AttrError::BadData { reason } if reason.contains(needle))
}

/// `evaluate_init` of `data` on `source` in `fx`'s world.
fn eval(fx: &mut Fx, data: InitData, source: Option<ObjRef>) -> Result<f32, AttrError> {
    eval_in(fx, data, source, None, NumForms::default())
}

fn eval_in(
    fx: &mut Fx,
    data: InitData,
    source: Option<ObjRef>,
    over: Option<ObjRef>,
    forms: NumForms,
) -> Result<f32, AttrError> {
    let sys = system();
    let mut cx = EvalCx::new(&mut fx.w).with_forms(forms);
    sys.evaluate_init(&data, source, over, &mut cx)
}

fn of_def(def: ObjRef) -> InitData {
    InitData {
        initialization_definition: Some(def),
        ..InitData::constant(0.0)
    }
}

fn of_attr(attr: ObjRef) -> InitData {
    InitData {
        base_value_attribute: Some(attr),
        ..InitData::constant(0.0)
    }
}

fn value_of(
    fx: &mut Fx,
    attr: ObjRef,
    source: Option<ObjRef>,
    over: Option<ObjRef>,
) -> Option<(ObjRef, f32)> {
    let sys = system();
    let mut cx = EvalCx::new(&mut fx.w);
    sys.value_of(attr, source, over, &mut cx)
        .unwrap()
        .map(|v| (v.context, v.value))
}

/// The level curve's definition `M * (L ^ P + O)` with the level from global slot 0 through an attribute.
fn curve(fx: &mut Fx, m: f32, p: f32, o: f32) -> ObjRef {
    let (ctx, global) = (fx.no_context(), fx.global(0));
    let level = fx.attr("LevelAttribute", vec![ctx], vec![global]);
    let (m, l, p, o) = (fx.k(m), fx.of_attr(level), fx.k(p), fx.k(o));
    let f = fx.formula(m, l, p, o);
    fx.definition(vec![("ValueFormula", f)])
}

/// `ExperienceCurve::required`: max(0, trunc(f(n)) - trunc(f(1))), the level in global slot 0.
fn required(fx: &mut Fx, def: ObjRef, level: i32, pow: PowForm) -> i64 {
    let mut point = |n: i32| {
        fx.w.slots.set(0, black_box(n) as f32).unwrap();
        i64::from(trunc_i32(
            eval_in(fx, of_def(def), None, None, forms(pow)).unwrap(),
        ))
    };
    (point(level) - point(1)).max(0)
}

#[test]
fn the_invented_experience_curve_of_mission_script_test() {
    // mission_script_test.py: f(n) = 2 x (n^2 + 1), so R(n) = trunc f(n) - 4.
    for pow in FORMS {
        let mut fx = Fx::new();
        let def = curve(&mut fx, 2.0, 2.0, 1.0);
        let got: Vec<i64> = [2, 3, 4, 5, 50]
            .iter()
            .map(|&n| required(&mut fx, def, n, pow))
            .collect();
        assert_eq!(got, [6, 16, 30, 48, 4998], "{pow:?}");
    }
}

#[test]
fn the_live_level_curve_through_a_definition() {
    // REALGAME_GROUND_TRUTH.md (record constants): 60 x (L^2.8 + 7.33); "next level at" 1,241 (2), 28,126 (8),
    // 196,238 (17), 9,155,282 (70), each the curve at L + 1. Both pow forms, the same bits at every level 1..80.
    let mut fx = Fx::new();
    let def = curve(&mut fx, 60.0, 2.8, 7.33);
    for pow in FORMS {
        let got: Vec<i64> = [1, 3, 9, 18, 71]
            .iter()
            .map(|&n| required(&mut fx, def, n, pow))
            .collect();
        assert_eq!(got, [0, 1_241, 28_126, 196_238, 9_155_282], "{pow:?}");
    }
    for level in 1..=80 {
        fx.w.slots.set(0, level as f32).unwrap();
        let single = eval_in(&mut fx, of_def(def), None, None, forms(PowForm::Single)).unwrap();
        let double = eval_in(
            &mut fx,
            of_def(def),
            None,
            None,
            forms(PowForm::DoubleThenSingle),
        )
        .unwrap();
        assert_eq!(single.to_bits(), double.to_bits(), "level {level}");
    }
}

/// The player level as an attribute: a replication info context, the plain int attribute `ExpLevel`.
fn player_level(fx: &mut Fx, level: i32) -> (ObjRef, ObjRef) {
    let pri = fx.obj(fx.c.pri, "PRI", vec![("ExpLevel", Value::Int(level))]);
    (
        fx.property_attr("PlayerExperienceLevel", pri, "ExpLevel"),
        pri,
    )
}

#[test]
fn the_live_health_base_through_a_definition() {
    // REALGAME_GROUND_TRUTH.md: max(20, 80 x 1.13^L), the multiplier and the scaler constant attributes, the power
    // the player level; live 102.152 (L2), 212.6755 (L8), 638.886 (L17), 415,509.44 (L70) to the printed digits.
    let live = [
        (2, 102.152, 1e-3),
        (8, 212.6755, 1e-4),
        (17, 638.886, 1e-3),
        (70, 415_509.44, 1e-2),
    ];
    for (level, shown, unit) in live {
        let mut bits = Vec::new();
        for pow in FORMS {
            let mut fx = Fx::new();
            let (level_attr, _) = player_level(&mut fx, level);
            let m = fx.constant_attr("Att_UniversalBalanceMultiplier_HealthShields", 80.0);
            let l = fx.constant_attr("Att_UniversalBalanceScaler", 1.13);
            let (m, l, p, o) = (
                fx.of_attr(m),
                fx.of_attr(l),
                fx.of_attr(level_attr),
                fx.k(0.0),
            );
            let f = fx.formula(m, l, p, o);
            let r = fx.range(Some(20.0), None);
            let def = fx.definition(vec![("ValueFormula", f), ("RangeRestriction", r)]);
            let v = eval_in(&mut fx, of_def(def), None, None, forms(pow)).unwrap();
            assert!(
                (f64::from(v) - shown).abs() <= unit / 2.0,
                "{pow:?} level {level}: {v} vs {shown}"
            );
            bits.push(v.to_bits());
        }
        assert_eq!(bits[0], bits[1], "level {level}");
    }
    // The same bits under both forms at every level 1..80, through the definition (one world, the level rewritten).
    let mut fx = Fx::new();
    let (level_attr, pri) = player_level(&mut fx, 1);
    let m = fx.constant_attr("Att_UniversalBalanceMultiplier_HealthShields", 80.0);
    let l = fx.constant_attr("Att_UniversalBalanceScaler", 1.13);
    let (m, l, p, o) = (
        fx.of_attr(m),
        fx.of_attr(l),
        fx.of_attr(level_attr),
        fx.k(0.0),
    );
    let f = fx.formula(m, l, p, o);
    let r = fx.range(Some(20.0), None);
    let def = fx.definition(vec![("ValueFormula", f), ("RangeRestriction", r)]);
    let exp_level = PropAddr::scalar(fx.w.space.prop(fx.c.pri, "ExpLevel").unwrap());
    for level in 1..=80 {
        fx.w.space
            .write(pri, exp_level, Value::Int(black_box(level)))
            .unwrap();
        let single = eval_in(&mut fx, of_def(def), None, None, forms(PowForm::Single)).unwrap();
        let double = eval_in(
            &mut fx,
            of_def(def),
            None,
            None,
            forms(PowForm::DoubleThenSingle),
        )
        .unwrap();
        assert_eq!(single.to_bits(), double.to_bits(), "level {level}");
    }
}

#[test]
fn the_invented_health_pool_of_mission_script_test() {
    // mission_script_test.py swap 7: 10 x 1.5 ^ PlayerExperienceLevel: 50.625 (4), 75.9375 (5), 113.90625 (6).
    for pow in FORMS {
        for (level, expected) in [(4, 50.625f32), (5, 75.9375), (6, 113.90625)] {
            let mut fx = Fx::new();
            let (p, _) = player_level(&mut fx, level);
            let (m, l, p, o) = (fx.k(10.0), fx.k(1.5), fx.of_attr(p), fx.k(0.0));
            let f = fx.formula(m, l, p, o);
            let def = fx.definition(vec![("ValueFormula", f)]);
            let got = eval_in(&mut fx, of_def(def), None, None, forms(pow));
            assert_eq!(got, Ok(expected), "{pow:?} level {level}");
        }
    }
}

/// The reward percentage of mission_script_test.py: 0.5, then x a conditional (4 when PlayThroughCount == 2, else
/// 3), on playthrough count `count` (an invented constant attribute standing for the playthrough read).
fn reward_percentage(fx: &mut Fx, count: f32) -> ObjRef {
    let play_through = fx.constant_attr("PlayThroughCount", count);
    let e = fx.expr(
        Some(play_through),
        Comparison::EqualTo,
        Operand2Usage::PreferAttribute,
        None,
        2.0,
    );
    let (four, three) = (fx.k(4.0), fx.k(3.0));
    let cond = fx.conditional(true, vec![(vec![e], four)], three);
    let multiplier = fx.definition(vec![("ConditionalInitialization", cond)]);
    let (ctx, constant) = (fx.no_context(), fx.constant(0.5));
    let arg = fx.of_def(multiplier);
    let math = fx.math(MUL, arg, None);
    fx.attr("XPReward", vec![ctx], vec![constant, math])
}

#[test]
fn the_reward_percentage_chain_and_the_mission_reward() {
    let mut fx = Fx::new();
    let pct = reward_percentage(&mut fx, 1.0);
    let p = eval(&mut fx, of_attr(pct), None).unwrap();
    assert_eq!(p, 1.5);
    // At game stage 4: trunc((R(5) - R(4)) x 1.5) = trunc(18 x 1.5) = 27.
    assert_eq!(trunc_i32((48 - 30) as f32 * p), 27);
    let mut fx = Fx::new();
    let pct = reward_percentage(&mut fx, 2.0);
    assert_eq!(eval(&mut fx, of_attr(pct), None), Ok(2.0));
}

#[test]
fn the_skill_points_per_level_of_mission_script_test() {
    // 1 when PlayerExperienceLevel >= 5, else the default 0.
    for (level, expected) in [(3, 0.0f32), (4, 0.0), (5, 1.0), (6, 1.0)] {
        let mut fx = Fx::new();
        let (p, _) = player_level(&mut fx, level);
        let e = fx.expr(
            Some(p),
            Comparison::GreaterThanOrEqual,
            Operand2Usage::PreferAttribute,
            None,
            5.0,
        );
        let (one, zero) = (fx.k(1.0), fx.k(0.0));
        let cond = fx.conditional(true, vec![(vec![e], one)], zero);
        let def = fx.definition(vec![("ConditionalInitialization", cond)]);
        assert_eq!(
            eval(&mut fx, of_def(def), None),
            Ok(expected),
            "level {level}"
        );
    }
}

#[test]
fn constant_and_scale_rows_of_combat_test() {
    // The kill experience base formula 20 x 1.5 = 30; a thrown rock's ProjectileDamage 10 x 1.5 = 15, x 2.0 = 30.
    let mut fx = Fx::new();
    let base = InitData {
        base_value_scale_constant: 1.5,
        ..InitData::constant(20.0)
    };
    assert_eq!(eval(&mut fx, base, None), Ok(30.0));
    let rock = eval(
        &mut fx,
        InitData {
            base_value_scale_constant: 1.5,
            ..InitData::constant(10.0)
        },
        None,
    )
    .unwrap();
    let cause = InitData {
        base_value_scale_constant: 2.0,
        ..InitData::constant(rock)
    };
    assert_eq!(eval(&mut fx, cause, None), Ok(30.0));
}

#[test]
fn a_random_integer_quantity_reaches_both_ends() {
    // combat_test.py: a random quantity of 2 to 4 (integer) reaches each value; base 0, mode 0, a purely random
    // definition. The draws come from the AttributeVariance stream.
    let mut fx = Fx::new();
    let v = fx.variance(2.0, 4.0, true);
    let def = fx.definition(vec![("RandomVariance", v)]);
    let mut seen = std::collections::BTreeSet::new();
    fx.w.rng = Some(CrtRand::new(6000));
    let mut same = CrtRand::new(6000);
    for roll in 0..40 {
        let q = eval(&mut fx, of_def(def), None).unwrap();
        let draw = f64::from(same.rand15()) / 32768.0;
        assert_eq!(q, (2.0 + (draw * 3.0).floor()) as f32, "roll {roll}");
        seen.insert(trunc_i32(q));
    }
    assert_eq!(seen.into_iter().collect::<Vec<_>>(), [2, 3, 4]);
    assert!(
        fx.w.streams
            .iter()
            .all(|s| *s == CrtStream::AttributeVariance)
    );
}

#[test]
fn a_random_real_amount_is_added_after_the_mode_and_before_the_scale() {
    let mut fx = Fx::new();
    let v = fx.variance(1.0, 3.0, false);
    let (m, l, p, o) = (fx.k(1.0), fx.k(10.0), fx.k(1.0), fx.k(0.0));
    let f = fx.formula(m, l, p, o);
    let def = fx.definition(vec![
        ("ValueFormula", f),
        ("RandomVariance", v),
        ("BaseValueMode", Value::Byte(1)),
    ]);
    fx.w.rng = Some(CrtRand::new(7));
    let data = InitData {
        initialization_definition: Some(def),
        base_value_scale_constant: 2.0,
        ..InitData::constant(5.0)
    };
    let got = eval(&mut fx, data, None).unwrap();
    let draw = f64::from(CrtRand::new(7).rand15()) / 32768.0;
    let amount = 1.0f32 + (draw as f32) * (3.0 - 1.0);
    assert_eq!(got, ((5.0 + 10.0) + amount) * 2.0);
    // An empty integer span gives ceil(low), after the draw.
    let v = fx.variance(2.5, 2.7, true);
    let def = fx.definition(vec![("RandomVariance", v)]);
    assert_eq!(eval(&mut fx, of_def(def), None), Ok(3.0));
    // No generator: the world says so (progression.cpp refuses "no random source").
    fx.w.rng = None;
    assert_eq!(
        missing(eval(&mut fx, of_def(def), None).unwrap_err()),
        Missing::LayerAbsent("ow-world")
    );
}

/// A weapon_gen_test.py-like world: Weapon_Is_Acme is a manufacturer resolver behind a weapon context.
fn maker_attr(fx: &mut Fx, weapon: ObjRef, maker_name: &str) -> ObjRef {
    let c = fx.by_path("WillowGame.ManufacturerAttributeValueResolver");
    let (n, one, zero) = (fx.name(maker_name), fx.k(1.0), fx.k(0.0));
    let ty = fx.c.maker_sel;
    let entry = fx.st(
        ty,
        vec![("AssociatedManufacturerName", n), ("ValueIfMatched", one)],
    );
    let r = fx.obj(
        c,
        "Maker",
        vec![
            ("Manufacturers", Value::array(vec![entry])),
            ("ValueIfNotMatched", zero),
        ],
    );
    let ctx = fx.target(Some(weapon));
    fx.attr("Weapon_Is_Acme", vec![ctx], vec![r])
}

#[test]
fn weapon_is_maker_is_the_manufacturer_resolver() {
    // weapon_gen_test.py init 10: 0.0, attribute Weapon_Is_Acme, scale 3.0: 3 for an Acme gun, 0 for another maker.
    for (maker, expected) in [("Acme", 3.0f32), ("ACME", 3.0), ("Other", 0.0)] {
        let mut fx = Fx::new();
        let m = fx.obj(fx.c.maker, maker, vec![]);
        let gun = fx.weapon(Some(m), None);
        let attr = maker_attr(&mut fx, gun, "Acme");
        let data = InitData {
            base_value_scale_constant: 3.0,
            ..of_attr(attr)
        };
        assert_eq!(eval(&mut fx, data, None), Ok(expected), "{maker}");
        // The name-part form: every expression `Weapon_Is_Acme == 1` must hold.
        let e = ExprData {
            attribute_operand1: Some(attr),
            comparison_operator: Comparison::EqualTo,
            operand2_usage: Operand2Usage::PreferAttribute,
            attribute_operand2: None,
            constant_operand2: 1.0,
        };
        let sys = system();
        let held =
            sys.expressions_hold(&[e], ExprMode::And, None, None, &mut EvalCx::new(&mut fx.w));
        assert_eq!(held, Ok(expected == 3.0));
    }
    // A gun with no manufacturer matches nothing; a context that is not an item is not read.
    let mut fx = Fx::new();
    let gun = fx.weapon(None, None);
    let attr = maker_attr(&mut fx, gun, "Acme");
    assert_eq!(eval(&mut fx, of_attr(attr), None), Ok(0.0));
    let pri = fx.obj(fx.c.pri, "PRI", vec![]);
    let attr = maker_attr(&mut fx, pri, "Acme");
    assert_eq!(
        missing(eval(&mut fx, of_attr(attr), None).unwrap_err()),
        Missing::NotRead
    );
}

#[test]
fn the_weapon_gen_damage_stack_with_the_acme_term() {
    // weapon_gen_test.py: (10 + 2) * (1 + 0.5) / (1 + 0.2) + 3 (Weapon_Is_Acme * 3) = 17.99999996 -> float 18. The
    // four modifier values are evaluated by the one evaluator; the combine is W1-ATTRMATH's (WeaponGen form).
    let mut fx = Fx::new();
    let m = fx.obj(fx.c.maker, "Acme", vec![]);
    let gun = fx.weapon(Some(m), None);
    let acme = maker_attr(&mut fx, gun, "Acme");
    let base = eval(&mut fx, InitData::constant(10.0), None).unwrap();
    let values = [
        (ModifierType::Scale, InitData::constant(-0.2)),
        (ModifierType::Scale, InitData::constant(0.5)),
        (ModifierType::PreAdd, InitData::constant(2.0)),
        (
            ModifierType::PostAdd,
            InitData {
                base_value_scale_constant: 3.0,
                ..of_attr(acme)
            },
        ),
    ];
    let mods: Vec<Modifier> = values
        .iter()
        .enumerate()
        .map(|(i, (kind, d))| Modifier {
            kind: *kind,
            value: eval(&mut fx, *d, None).unwrap(),
            id: ModifierId(i as u64),
        })
        .collect();
    assert_eq!(
        combine(base, &mods, CombineForm::WeaponGen, NumKind::Float),
        Combined::Float(18.0)
    );
}

#[test]
fn a_designer_attribute_as_the_base_of_skill_stats_test() {
    // skill_stats_test.py: the effect value is a designer attribute whose value is 0.25 (shown 25%). Here the
    // designer object is a fixed test context holding a plain float attribute `Value`.
    let mut fx = Fx::new();
    let designer = fx.obj(
        fx.c.designer,
        "Designer",
        vec![("Value", Value::Float(0.25))],
    );
    let attr = fx.property_attr("Designer", designer, "Value");
    assert_eq!(eval(&mut fx, of_attr(attr), None), Ok(0.25));
}

#[test]
fn the_offset_goes_inside_the_multiplier() {
    // NATIVE_PROGRESSION.md section 1: {Multiplier 2, Level 3, Power 2, Offset 1} is 20, not 19.
    let mut fx = Fx::new();
    let (m, l, p, o) = (fx.k(2.0), fx.k(3.0), fx.k(2.0), fx.k(1.0));
    let f = fx.formula(m, l, p, o);
    let def = fx.definition(vec![("ValueFormula", f)]);
    assert_eq!(eval(&mut fx, of_def(def), None), Ok(20.0));
}

#[test]
fn base_value_modes_combine_the_definition_with_the_base() {
    for (mode, expected) in [(0u8, 10.0f32), (1, 14.0), (2, 40.0), (3, 6.0)] {
        let mut fx = Fx::new();
        let (m, l, p, o) = (fx.k(1.0), fx.k(10.0), fx.k(1.0), fx.k(0.0));
        let f = fx.formula(m, l, p, o);
        let def = fx.definition(vec![
            ("ValueFormula", f),
            ("BaseValueMode", Value::Byte(mode)),
        ]);
        let data = InitData {
            initialization_definition: Some(def),
            ..InitData::constant(4.0)
        };
        assert_eq!(eval(&mut fx, data, None), Ok(expected), "mode {mode}");
    }
    let mut fx = Fx::new();
    let (m, l, p, o) = (fx.k(1.0), fx.k(10.0), fx.k(1.0), fx.k(0.0));
    let f = fx.formula(m, l, p, o);
    let def = fx.definition(vec![("ValueFormula", f), ("BaseValueMode", Value::Byte(4))]);
    assert!(is_bad_data(
        &eval(&mut fx, of_def(def), None).unwrap_err(),
        "BaseValueMode 4"
    ));
}

#[test]
fn scale_then_clamp_then_rounding() {
    let mut fx = Fx::new();
    // No definition: the scale applies, nothing clamps.
    assert_eq!(
        eval(
            &mut fx,
            InitData {
                base_value_scale_constant: 2.0,
                ..InitData::constant(3.0)
            },
            None
        ),
        Ok(6.0)
    );
    // A definition with no formula and no conditional leaves the base (weapon_gen.cpp), then scale, clamp, round.
    let r = fx.range(Some(1.0), Some(5.0));
    let def = fx.definition(vec![("RangeRestriction", r)]);
    let data = |c: f32| InitData {
        initialization_definition: Some(def),
        base_value_scale_constant: 2.0,
        ..InitData::constant(c)
    };
    assert_eq!(eval(&mut fx, data(3.0), None), Ok(5.0));
    assert_eq!(eval(&mut fx, data(0.25), None), Ok(1.0));
    // Rounding modes: 1 is floor(x + 0.5) (weapon_gen.cpp Round()), so -2.5 gives -2; 2 floor; 3 ceiling.
    for (mode, x, expected) in [
        (1u8, 2.5f32, 3.0f32),
        (1, -2.5, -2.0),
        (1, 2.4, 2.0),
        (2, 2.7, 2.0),
        (3, 2.1, 3.0),
        (0, 2.25, 2.25),
    ] {
        let def = fx.definition(vec![("RoundingMode", Value::Byte(mode))]);
        let data = InitData {
            initialization_definition: Some(def),
            ..InitData::constant(x)
        };
        assert_eq!(eval(&mut fx, data, None), Ok(expected), "mode {mode} x {x}");
    }
    let def = fx.definition(vec![("RoundingMode", Value::Byte(9))]);
    assert!(is_bad_data(
        &eval(&mut fx, of_def(def), None).unwrap_err(),
        "RoundingMode 9"
    ));
}

#[test]
fn an_attribute_that_does_not_resolve_leaves_the_constant() {
    let mut fx = Fx::new();
    // An empty context chain resolves nothing (it does not pass the source through).
    let r = fx.constant(9.0);
    let empty = fx.attr("Empty", vec![], vec![r]);
    assert_eq!(
        eval(
            &mut fx,
            InitData {
                base_value_attribute: Some(empty),
                ..InitData::constant(4.0)
            },
            None
        ),
        Ok(4.0)
    );
    // A context resolver answering None ends the chain.
    let (none, r) = (fx.target(None), fx.constant(9.0));
    let unresolved = fx.attr("Unresolved", vec![none], vec![r]);
    assert_eq!(
        eval(
            &mut fx,
            InitData {
                base_value_attribute: Some(unresolved),
                ..InitData::constant(4.0)
            },
            None
        ),
        Ok(4.0)
    );
    // A resolved context with a value of 0 gives 0, not the constant.
    let zero = fx.constant_attr("Zero", 0.0);
    assert_eq!(
        eval(
            &mut fx,
            InitData {
                base_value_attribute: Some(zero),
                ..InitData::constant(4.0)
            },
            None
        ),
        Ok(0.0)
    );
}

#[test]
fn context_chains_run_in_order_and_stop_at_the_first_none() {
    let mut fx = Fx::new();
    let pri = fx.obj(fx.c.pri, "PRI", vec![("ExpLevel", Value::Int(7))]);
    let pc = fx.obj(
        fx.c.controller,
        "PC",
        vec![("PlayerReplicationInfo", Value::Object(Some(pri)))],
    );
    // controller -> its PlayerReplicationInfo -> ExpLevel; a None entry is skipped.
    let (hop, value) = (
        fx.hop("PlayerReplicationInfo"),
        fx.property_resolver("ExpLevel"),
    );
    let objs = |v: Vec<Option<ObjRef>>| Value::array(v.into_iter().map(Value::Object).collect());
    let attr = fx.obj(
        fx.c.attribute,
        "Level",
        vec![
            ("ContextResolverChain", objs(vec![None, Some(hop)])),
            ("ValueResolverChain", objs(vec![Some(value)])),
        ],
    );
    assert_eq!(value_of(&mut fx, attr, Some(pc), None), Some((pri, 7.0)));
    // A None source: the hop answers None, so nothing resolves.
    assert_eq!(value_of(&mut fx, attr, None, None), None);
    // Two hops: the second receives the first's answer (the PRI has no PlayerReplicationInfo: None).
    let hop2 = fx.hop("PlayerReplicationInfo");
    let attr2 = fx.attr("Twice", vec![hop, hop2], vec![value]);
    assert_eq!(value_of(&mut fx, attr2, Some(pc), None), None);
    // A chain of None entries only resolves nothing.
    let only_none = fx.obj(
        fx.c.attribute,
        "OnlyNone",
        vec![("ContextResolverChain", objs(vec![None]))],
    );
    let sys = system();
    assert_eq!(
        sys.resolve_context(only_none, Some(pc), &mut EvalCx::new(&mut fx.w)),
        Ok(None)
    );
    assert_eq!(
        sys.resolve_contexts(attr, Some(pc), &mut EvalCx::new(&mut fx.w)),
        Ok(vec![pri])
    );
    assert_eq!(
        sys.resolve_contexts(attr, None, &mut EvalCx::new(&mut fx.w)),
        Ok(vec![])
    );
}

#[test]
fn the_override_source_is_tried_first() {
    let mut fx = Fx::new();
    let pri_a = fx.obj(fx.c.pri, "A", vec![("ExpLevel", Value::Int(3))]);
    let pri_b = fx.obj(fx.c.pri, "B", vec![("ExpLevel", Value::Int(5))]);
    let pc_a = fx.obj(
        fx.c.controller,
        "PCA",
        vec![("PlayerReplicationInfo", Value::Object(Some(pri_a)))],
    );
    let pc_b = fx.obj(
        fx.c.controller,
        "PCB",
        vec![("PlayerReplicationInfo", Value::Object(Some(pri_b)))],
    );
    let bare = fx.obj(fx.c.controller, "Bare", vec![]);
    let (hop, value) = (
        fx.hop("PlayerReplicationInfo"),
        fx.property_resolver("ExpLevel"),
    );
    let attr = fx.attr("Level", vec![hop], vec![value]);
    assert_eq!(
        value_of(&mut fx, attr, Some(pc_a), Some(pc_b)),
        Some((pri_b, 5.0))
    );
    // An override that does not resolve falls back to the source, silently.
    assert_eq!(
        value_of(&mut fx, attr, Some(pc_a), Some(bare)),
        Some((pri_a, 3.0))
    );
    let data = InitData {
        base_value_attribute: Some(attr),
        ..InitData::constant(1.0)
    };
    assert_eq!(
        eval_in(&mut fx, data, Some(pc_a), Some(pc_b), NumForms::default()),
        Ok(5.0)
    );
    // The base variant reads the same context.
    let sys = system();
    let base = sys
        .base_value_of(attr, Some(pc_a), Some(pc_b), &mut EvalCx::new(&mut fx.w))
        .unwrap();
    assert_eq!(base.map(|v| (v.context, v.value)), Some((pri_b, 5.0)));
}

#[test]
fn value_chains_fold_the_previous_value() {
    let mut fx = Fx::new();
    let ctx = fx.no_context();
    let chain = |fx: &mut Fx, op: u8, arg: f32, first: f32| {
        let (k, a) = (fx.constant(first), fx.k(arg));
        let m = fx.math(op, a, None);
        let attr = fx.attr("Math", vec![ctx], vec![k, m]);
        value_of(fx, attr, None, None).map(|v| v.1)
    };
    assert_eq!(chain(&mut fx, ADD, 3.0, 2.0), Some(5.0));
    assert_eq!(chain(&mut fx, SUB, 3.0, 2.0), Some(-1.0));
    assert_eq!(chain(&mut fx, MUL, 3.0, 2.0), Some(6.0));
    assert_eq!(chain(&mut fx, DIV, 4.0, 2.0), Some(0.5));
    assert_eq!(chain(&mut fx, DIV, 1e-9, 2.0), Some(0.0));
    assert_eq!(chain(&mut fx, DIV, -1e-9, 2.0), Some(0.0));
    // Arg1Option FromAttribute: the first operand is the Arg1Attribute data, not the previous value.
    let (k, a, a1) = (fx.constant(100.0), fx.k(3.0), fx.k(10.0));
    let m = fx.math(SUB, a, Some(a1));
    let attr = fx.attr("FromAttribute", vec![ctx], vec![k, m]);
    assert_eq!(value_of(&mut fx, attr, None, None).map(|v| v.1), Some(7.0));
    // An empty value chain is 0; value_from_context of None is 0.
    let empty = fx.attr("NoValues", vec![ctx], vec![]);
    assert_eq!(
        value_of(&mut fx, empty, None, None),
        Some((fx.w.globals, 0.0))
    );
    let sys = system();
    assert_eq!(
        sys.value_from_context(attr, None, &mut EvalCx::new(&mut fx.w)),
        Ok(0.0)
    );
    assert_eq!(
        sys.base_value_from_context(attr, None, &mut EvalCx::new(&mut fx.w)),
        Ok(0.0)
    );
    let globals = fx.w.globals;
    assert_eq!(
        sys.value_from_context(attr, Some(globals), &mut EvalCx::new(&mut fx.w)),
        Ok(7.0)
    );
    // A bad enum byte is bad data.
    let a = fx.k(1.0);
    let m = fx.math(7, a, None);
    let bad = fx.attr("Bad", vec![ctx], vec![m]);
    let sys = system();
    let e = sys
        .value_of(bad, None, None, &mut EvalCx::new(&mut fx.w))
        .unwrap_err();
    assert!(is_bad_data(&e, "Operand 7"));
}

#[test]
fn a_none_entry_in_a_value_chain_is_not_read() {
    let mut fx = Fx::new();
    let ctx = fx.no_context();
    let objs = |v: Vec<Option<ObjRef>>| Value::array(v.into_iter().map(Value::Object).collect());
    let attr = fx.obj(
        fx.c.attribute,
        "Holey",
        vec![
            ("ContextResolverChain", objs(vec![Some(ctx)])),
            ("ValueResolverChain", objs(vec![None])),
        ],
    );
    let sys = system();
    let e = sys
        .value_of(attr, None, None, &mut EvalCx::new(&mut fx.w))
        .unwrap_err();
    assert_eq!(missing(e), Missing::NotRead);
}

#[test]
fn global_slots_feed_the_global_resolver() {
    let mut fx = Fx::new();
    let ctx = fx.no_context();
    fx.w.slots.set(0, 12.0).unwrap();
    fx.w.slots.set(1, 3.0).unwrap();
    for (slot, expected) in [(0u8, 12.0f32), (1, 3.0), (5, 0.0)] {
        let g = fx.global(slot);
        let attr = fx.attr("Global", vec![ctx], vec![g]);
        assert_eq!(
            value_of(&mut fx, attr, None, None).map(|v| v.1),
            Some(expected)
        );
    }
}

#[test]
fn object_property_reads_stack_plain_inherited_and_unflagged_properties() {
    let mut fx = Fx::new();
    let gun = fx.weapon(None, None);
    let w = fx.c.weapon;
    for (n, v) in [
        ("InstantHitDamage", Value::Float(18.0)),
        ("InstantHitDamageBaseValue", Value::Float(10.0)),
        ("Unflagged", Value::Float(4.0)),
        ("ExpLevel", Value::Int(16_777_217)),
        ("ClipSize", Value::Byte(12)),
    ] {
        let p = fx.w.space.prop(w, n).unwrap();
        fx.w.space.write(gun, PropAddr::scalar(p), v).unwrap();
    }
    let sys = system();
    let read = |fx: &mut Fx, property: &str| {
        let attr = fx.property_attr(property, gun, property);
        let sys = system();
        let mut cx = EvalCx::new(&mut fx.w);
        let value = sys
            .value_of(attr, None, None, &mut cx)
            .unwrap()
            .unwrap()
            .value;
        let base = sys
            .base_value_of(attr, None, None, &mut cx)
            .unwrap()
            .unwrap()
            .value;
        (value, base)
    };
    // A stack attribute: the value, and its base companion.
    assert_eq!(read(&mut fx, "InstantHitDamage"), (18.0, 10.0));
    // A plain attribute: itself both ways; an inherited one (ExpLevel is declared on the item class); an int above
    // 2^24 rounds to float as the conversion does; a byte converts.
    assert_eq!(read(&mut fx, "InstantHitDamageBaseValue"), (10.0, 10.0));
    assert_eq!(
        fx.w.space.prop(w, "ExpLevel").unwrap().declared_in,
        fx.c.item
    );
    assert_eq!(read(&mut fx, "ExpLevel"), (16_777_216.0, 16_777_216.0));
    assert_eq!(read(&mut fx, "ClipSize"), (12.0, 12.0));
    // A property that is neither attribute kind reads 0.
    assert_eq!(read(&mut fx, "Unflagged"), (0.0, 0.0));
    // A property the class does not have is not read.
    let attr = fx.property_attr("Missing", gun, "NoSuchProperty");
    let e = sys
        .value_of(attr, None, None, &mut EvalCx::new(&mut fx.w))
        .unwrap_err();
    assert_eq!(missing(e), Missing::NotRead);
}

#[test]
fn object_property_names_its_storage() {
    let mut fx = Fx::new();
    let gun = fx.weapon(None, None);
    let w = fx.c.weapon;
    let sys = system();
    let storage = |fx: &mut Fx, property: &str| {
        let r = fx.property_resolver(property);
        let res = sys.value_resolver(&fx.w.space, r).unwrap();
        res.storage(&sys, r, gun, &mut EvalCx::new(&mut fx.w))
            .unwrap()
    };
    let value = fx.w.space.prop(w, "InstantHitDamage").unwrap();
    let companions = fx.w.space.classes.decl(value).unwrap().attr.unwrap();
    assert_eq!(
        storage(&mut fx, "InstantHitDamage"),
        Some(Storage::Stack {
            owner: gun,
            companions,
            kind: NumKind::Float
        })
    );
    let level = fx.w.space.prop(w, "ExpLevel").unwrap();
    assert_eq!(
        storage(&mut fx, "ExpLevel"),
        Some(Storage::Plain {
            owner: gun,
            prop: level,
            kind: NumKind::Int
        })
    );
    let clip = fx.w.space.prop(w, "ClipSize").unwrap();
    assert_eq!(
        storage(&mut fx, "ClipSize"),
        Some(Storage::Plain {
            owner: gun,
            prop: clip,
            kind: NumKind::Byte
        })
    );
    assert_eq!(storage(&mut fx, "Unflagged"), None);
    // Every other resolver class keeps the class default: not writable.
    let k = fx.constant(1.0);
    let res = sys.value_resolver(&fx.w.space, k).unwrap();
    assert_eq!(
        res.storage(&sys, k, gun, &mut EvalCx::new(&mut fx.w)),
        Ok(None)
    );
}

#[test]
fn weapon_type_matches_the_type_byte() {
    for (ty, expected) in [(4u8, 1.0f32), (2, 0.0)] {
        let mut fx = Fx::new();
        let t = fx.obj(
            fx.c.weapon_type,
            "Type",
            vec![("WeaponType", Value::Byte(ty))],
        );
        let gun = fx.weapon(None, Some(t));
        let c = fx.by_path("WillowGame.WeaponTypeAttributeValueResolver");
        let (one, zero) = (fx.k(1.0), fx.k(0.0));
        let sel = fx.c.type_sel;
        let entry = fx.st(
            sel,
            vec![("WeaponType", Value::Byte(4)), ("ValueIfMatched", one)],
        );
        let r = fx.obj(
            c,
            "Type",
            vec![
                ("WeaponTypes", Value::array(vec![entry])),
                ("ValueIfNotMatched", zero),
            ],
        );
        let ctx = fx.target(Some(gun));
        let attr = fx.attr("Weapon_Is_Assault_Rifle", vec![ctx], vec![r]);
        assert_eq!(
            value_of(&mut fx, attr, None, None).map(|v| v.1),
            Some(expected),
            "type {ty}"
        );
    }
}

#[test]
fn the_conditional_resolver_ignores_its_enabled_flag_and_the_previous_value() {
    let mut fx = Fx::new();
    let five = fx.constant_attr("Five", 5.0);
    let e = fx.expr(
        Some(five),
        Comparison::GreaterThan,
        Operand2Usage::PreferAttribute,
        None,
        4.0,
    );
    let (yes, no) = (fx.k(8.0), fx.k(1.0));
    let cond = fx.conditional(false, vec![(vec![e], yes)], no);
    let c = fx.by_path("GearboxFramework.ConditionalAttributeValueResolver");
    let r = fx.obj(c, "Cond", vec![("ValueExpressions", cond)]);
    let (ctx, k) = (fx.no_context(), fx.constant(100.0));
    let attr = fx.attr("Conditional", vec![ctx], vec![k, r]);
    assert_eq!(value_of(&mut fx, attr, None, None).map(|v| v.1), Some(8.0));
}

fn expr(
    op1: Option<ObjRef>,
    cmp: Comparison,
    usage: Operand2Usage,
    op2: Option<ObjRef>,
    k: f32,
) -> ExprData {
    ExprData {
        attribute_operand1: op1,
        comparison_operator: cmp,
        operand2_usage: usage,
        attribute_operand2: op2,
        constant_operand2: k,
    }
}

#[test]
fn expressions_compare_in_single_precision() {
    let mut fx = Fx::new();
    let three = fx.constant_attr("Three", 3.0);
    let two = fx.constant_attr("Two", 2.0);
    let (none, r) = (fx.target(None), fx.constant(1.0));
    let unresolved = fx.attr("Unresolved", vec![none], vec![r]);
    let sys = system();
    let mut holds = |e: ExprData| {
        sys.expressions_hold(&[e], ExprMode::And, None, None, &mut EvalCx::new(&mut fx.w))
            .unwrap()
    };
    use Comparison as C;
    use Operand2Usage as U;
    let cases = [
        (C::EqualTo, 3.0, true),
        (C::NotEqualTo, 3.0, false),
        (C::LessThan, 3.5, true),
        (C::LessThanOrEqual, 3.0, true),
        (C::GreaterThan, 3.0, false),
        (C::GreaterThanOrEqual, 3.0, true),
    ];
    for (cmp, k, expected) in cases {
        assert_eq!(
            holds(expr(Some(three), cmp, U::PreferAttribute, None, k)),
            expected,
            "{cmp:?} {k}"
        );
    }
    // The right attribute; Multiply scales it by the constant: 3 == 2 x 1.5.
    assert!(!holds(expr(
        Some(three),
        C::EqualTo,
        U::PreferAttribute,
        Some(two),
        1.5
    )));
    assert!(holds(expr(
        Some(three),
        C::EqualTo,
        U::Multiply,
        Some(two),
        1.5
    )));
    assert!(holds(expr(
        Some(three),
        C::GreaterThan,
        U::PreferAttribute,
        Some(two),
        99.0
    )));
    // A None or unresolved left side, or an unresolved right attribute: false.
    assert!(!holds(expr(
        None,
        C::NotEqualTo,
        U::PreferAttribute,
        None,
        0.0
    )));
    assert!(!holds(expr(
        Some(unresolved),
        C::NotEqualTo,
        U::PreferAttribute,
        None,
        0.0
    )));
    assert!(!holds(expr(
        Some(three),
        C::NotEqualTo,
        U::PreferAttribute,
        Some(unresolved),
        0.0
    )));
}

#[test]
fn expression_lists_and_or_and_empty() {
    let mut fx = Fx::new();
    let three = fx.constant_attr("Three", 3.0);
    let t = expr(
        Some(three),
        Comparison::EqualTo,
        Operand2Usage::PreferAttribute,
        None,
        3.0,
    );
    let f = expr(
        Some(three),
        Comparison::EqualTo,
        Operand2Usage::PreferAttribute,
        None,
        4.0,
    );
    let sys = system();
    let mut hold = |list: &[ExprData], mode| {
        sys.expressions_hold(list, mode, None, None, &mut EvalCx::new(&mut fx.w))
            .unwrap()
    };
    assert!(hold(&[], ExprMode::And));
    assert!(hold(&[], ExprMode::Or));
    assert!(hold(&[t, t], ExprMode::And));
    assert!(!hold(&[t, f], ExprMode::And));
    assert!(hold(&[f, t], ExprMode::Or));
    assert!(!hold(&[f, f], ExprMode::Or));
}

#[test]
fn the_conditional_takes_the_first_entry_that_holds() {
    let mut fx = Fx::new();
    let three = fx.constant_attr("Three", 3.0);
    let no = fx.expr(
        Some(three),
        Comparison::LessThan,
        Operand2Usage::PreferAttribute,
        None,
        1.0,
    );
    let yes = fx.expr(
        Some(three),
        Comparison::GreaterThan,
        Operand2Usage::PreferAttribute,
        None,
        1.0,
    );
    let (a, b, c, d) = (fx.k(10.0), fx.k(20.0), fx.k(30.0), fx.k(-1.0));
    // An empty expression list holds, so the third entry would also qualify: the first that holds wins.
    let cond = fx.conditional(
        true,
        vec![
            (vec![yes.clone(), no.clone()], a),
            (vec![yes], b),
            (vec![], c),
        ],
        d.clone(),
    );
    let def = fx.definition(vec![("ConditionalInitialization", cond)]);
    assert_eq!(eval(&mut fx, of_def(def), None), Ok(20.0));
    let ten = fx.k(10.0);
    let cond = fx.conditional(true, vec![(vec![no], ten)], d);
    let def = fx.definition(vec![("ConditionalInitialization", cond)]);
    assert_eq!(eval(&mut fx, of_def(def), None), Ok(-1.0));
}

#[test]
fn resolvers_without_their_layer_or_rule_say_what_is_missing() {
    let mut fx = Fx::new();
    let ctx = fx.no_context();
    let cases: [(&str, Vec<(&str, Value)>, Missing); 3] = [
        (
            "WillowGame.ResourcePoolStateAttributeValueResolver",
            vec![("PoolState", Value::Byte(1))],
            Missing::NotRead,
        ),
        (
            "WillowGame.AttributeSlotEffectAttributeValueResolver",
            vec![("SlotProperty", Value::Byte(1))],
            Missing::NotRead,
        ),
        (
            "WillowGame.BadassAttributeValueResolver",
            vec![],
            Missing::LayerAbsent("ow-save (the player profile)"),
        ),
    ];
    let sys = system();
    for (path, props, expected) in cases {
        let c = fx.by_path(path);
        let r = fx.obj(c, "R", props);
        let attr = fx.attr("A", vec![ctx], vec![r]);
        let e = sys
            .value_of(attr, None, None, &mut EvalCx::new(&mut fx.w))
            .unwrap_err();
        assert_eq!(missing(e), expected, "{path}");
    }
    // Out-of-enum bytes are bad data.
    let c = fx.by_path("WillowGame.ResourcePoolStateAttributeValueResolver");
    let r = fx.obj(c, "R", vec![("PoolState", Value::Byte(3))]);
    let attr = fx.attr("A", vec![ctx], vec![r]);
    assert!(is_bad_data(
        &sys.value_of(attr, None, None, &mut EvalCx::new(&mut fx.w))
            .unwrap_err(),
        "PoolState 3"
    ));
    // A class nobody registered is not read.
    let r = fx.obj(fx.c.unread, "R", vec![]);
    let attr = fx.attr("A", vec![ctx], vec![r]);
    assert_eq!(
        missing(
            sys.value_of(attr, None, None, &mut EvalCx::new(&mut fx.w))
                .unwrap_err()
        ),
        Missing::NotRead
    );
}

#[test]
fn a_multi_context_definition_is_not_read() {
    let mut fx = Fx::new();
    let (ctx, r) = (fx.no_context(), fx.constant(1.0));
    let attr = fx.attr_in(fx.c.multi, "Multi", vec![ctx], vec![r]);
    let sys = system();
    let mut cx = EvalCx::new(&mut fx.w);
    assert_eq!(
        missing(sys.resolve_contexts(attr, None, &mut cx).unwrap_err()),
        Missing::NotRead
    );
    assert_eq!(
        missing(sys.value_of(attr, None, None, &mut cx).unwrap_err()),
        Missing::NotRead
    );
    // ResolveContext itself is AttributeDefinitionBase's, so it runs.
    assert!(sys.resolve_context(attr, None, &mut cx).unwrap().is_some());
}

#[test]
fn a_cycle_in_the_data_is_bad_data() {
    // A definition whose Level term is an attribute whose SimpleMath argument is the same definition.
    let mut fx = Fx::new();
    let def = fx.definition(vec![]);
    let (ctx, arg) = (fx.no_context(), fx.of_def(def));
    let m = fx.math(ADD, arg, None);
    let attr = fx.attr("Loop", vec![ctx], vec![m]);
    let (m, l, p, o) = (fx.k(1.0), fx.of_attr(attr), fx.k(1.0), fx.k(0.0));
    let f = fx.formula(m, l, p, o);
    let p = fx.w.space.prop(fx.c.definition, "ValueFormula").unwrap();
    fx.w.space.write(def, PropAddr::scalar(p), f).unwrap();
    let e = eval(&mut fx, of_def(def), None).unwrap_err();
    assert!(is_bad_data(&e, "cycle"), "{e:?}");
    // The guard unwound: an unrelated evaluation runs after the failure.
    assert_eq!(eval(&mut fx, InitData::constant(2.0), None), Ok(2.0));
}

#[test]
fn nesting_beyond_the_budget_is_bad_data() {
    let mut fx = Fx::new();
    let mut inner: Option<ObjRef> = None;
    for _ in 0..(crate::eval::MAX_DEPTH + 1) {
        let base = match inner {
            Some(d) => fx.of_def(d),
            None => fx.k(1.0),
        };
        let (m, p, o) = (fx.k(1.0), fx.k(1.0), fx.k(0.0));
        let f = fx.formula(m, base, p, o);
        inner = Some(fx.definition(vec![("ValueFormula", f)]));
    }
    let e = eval(&mut fx, of_def(inner.unwrap()), None).unwrap_err();
    assert!(is_bad_data(&e, "deeper"), "{e:?}");
    assert_eq!(eval(&mut fx, InitData::constant(2.0), None), Ok(2.0));
}

#[test]
fn an_unloaded_definition_needs_the_store() {
    let mut fx = Fx::new();
    let handle =
        fx.w.space
            .alloc_stand_in(
                fx.c.definition,
                "Unloaded",
                Origin::Resource {
                    package: 0,
                    index: 1,
                },
            )
            .unwrap();
    // No store in the world: the layer is absent, never a guessed value.
    assert_eq!(
        missing(eval(&mut fx, of_def(handle), None).unwrap_err()),
        Missing::LayerAbsent("ow-defs")
    );
    // With a store, its answer comes through (the contract's body is NotYet until W3-DEFS lands it).
    fx.w.defs = Some(DefStore::new_empty());
    assert!(matches!(
        eval(&mut fx, of_def(handle), None).unwrap_err(),
        AttrError::NotYet(_)
    ));
    // A loaded (runtime) definition needs no store at all.
    let (m, l, p, o) = (fx.k(2.0), fx.k(3.0), fx.k(2.0), fx.k(1.0));
    let f = fx.formula(m, l, p, o);
    let def = fx.definition(vec![("ValueFormula", f)]);
    fx.w.defs = None;
    assert_eq!(eval(&mut fx, of_def(def), None), Ok(20.0));
}

#[test]
fn a_definition_object_of_the_wrong_class_is_bad_data() {
    let mut fx = Fx::new();
    let notdef = fx.obj(fx.c.object, "NotADefinition", vec![]);
    assert!(is_bad_data(
        &eval(&mut fx, of_def(notdef), None).unwrap_err(),
        "has no property ValueFormula"
    ));
    let sys = system();
    let e = sys
        .resolve_context(notdef, None, &mut EvalCx::new(&mut fx.w))
        .unwrap_err();
    assert!(is_bad_data(&e, "has no property ContextResolverChain"));
}

#[test]
fn standard_registers_every_value_resolver_class() {
    let fx = Fx::new();
    let sys = AttributeSystem::standard().unwrap();
    let mut space = fx.w.space;
    for path in CLASSES {
        let c = space.classes.by_path(path).unwrap();
        let o = space.alloc(c, "R", None, Origin::Runtime).unwrap();
        assert!(sys.value_resolver(&space, o).is_ok(), "{path}");
    }
}

/// A PlayerSkill attribute on the player controller `pc` for the skill definition `definition`.
fn player_skill_attr(fx: &mut Fx, pc: ObjRef, definition: Option<ObjRef>) -> ObjRef {
    let c = fx.by_path("WillowGame.PlayerSkillAttributeValueResolver");
    let r = fx.obj(
        c,
        "Skill",
        vec![("AssociatedSkill", Value::Object(definition))],
    );
    let ctx = fx.target(Some(pc));
    fx.attr("SkillGrade", vec![ctx], vec![r])
}

#[test]
fn player_skill_reads_the_grade_through_the_typed_engine_query() {
    let mut fx = Fx::new();
    let pc = fx.obj(fx.c.controller, "PC", vec![]);
    let definition = fx.obj(fx.c.object, "SkillDef", vec![]);
    let other = fx.obj(fx.c.object, "OtherDef", vec![]);
    let skill = fx.obj(fx.c.skill, "Skill", vec![("Grade", Value::Int(3))]);
    let attr = player_skill_attr(&mut fx, pc, Some(definition));
    let sys = system();
    // A world with no engine side: the query's own NotYet, never a guessed 0.
    let e = sys
        .value_of(attr, None, None, &mut EvalCx::new(&mut fx.w))
        .unwrap_err();
    assert_eq!(missing(e), Missing::LayerAbsent("ow-world"));
    // The world answers: the skill's Grade; no such skill in the tree: 0.
    fx.w.skills = Some(vec![(definition, Some(skill)), (other, None)]);
    assert_eq!(value_of(&mut fx, attr, None, None), Some((pc, 3.0)));
    let attr_other = player_skill_attr(&mut fx, pc, Some(other));
    assert_eq!(value_of(&mut fx, attr_other, None, None), Some((pc, 0.0)));
    // A resolver naming no skill is not read.
    let none = player_skill_attr(&mut fx, pc, None);
    let e = sys
        .value_of(none, None, None, &mut EvalCx::new(&mut fx.w))
        .unwrap_err();
    assert_eq!(missing(e), Missing::NotRead);
}

#[test]
fn a_shared_sub_definition_past_the_work_budget_is_bad_data() {
    // Each definition names the previous one in all four formula terms: 4^n evaluations at depth n. Twelve levels
    // (about 5.6e6 steps if nothing stopped them) stay far under the depth cap and far over the work budget.
    let mut fx = Fx::new();
    let mut shared: Option<ObjRef> = None;
    for _ in 0..12 {
        let term = |fx: &mut Fx| match shared {
            Some(d) => fx.of_def(d),
            None => fx.k(1.0),
        };
        let (m, l, p, o) = (term(&mut fx), term(&mut fx), term(&mut fx), term(&mut fx));
        let f = fx.formula(m, l, p, o);
        shared = Some(fx.definition(vec![("ValueFormula", f)]));
    }
    let e = eval(&mut fx, of_def(shared.unwrap()), None).unwrap_err();
    assert!(is_bad_data(&e, "more than 10000 steps"), "{e:?}");
    // The budget is per outermost step: a small shared chain still evaluates, and the count starts again.
    let mut small: Option<ObjRef> = None;
    for _ in 0..3 {
        let term = |fx: &mut Fx| match small {
            Some(d) => fx.of_def(d),
            None => fx.k(1.0),
        };
        let (m, l, p, o) = (term(&mut fx), term(&mut fx), term(&mut fx), term(&mut fx));
        let f = fx.formula(m, l, p, o);
        small = Some(fx.definition(vec![("ValueFormula", f)]));
    }
    // 1 x (1 ^ 1 + 1) = 2, then 2 x (2 ^ 2 + 2) = 12, then 12 x (12 ^ 12 + 12).
    let expected = 12.0f32 * (8_916_100_448_256.0f32 + 12.0);
    assert_eq!(eval(&mut fx, of_def(small.unwrap()), None), Ok(expected));
}

#[test]
fn a_stack_attribute_without_companions_reads_its_value_only() {
    // NATIVE_ATTRIBUTES.md section 3 gives "Modifier stack not found" for add and remove only: the value read takes
    // the stored number; the base read and the storage need the companions.
    let mut fx = Fx::new();
    let loose = fx.obj(fx.c.unlinked, "Loose", vec![("Loose", Value::Float(6.5))]);
    let attr = fx.property_attr("Loose", loose, "Loose");
    let sys = system();
    let mut cx = EvalCx::new(&mut fx.w);
    let v = sys
        .value_of(attr, None, None, &mut cx)
        .unwrap()
        .map(|v| v.value);
    assert_eq!(v, Some(6.5));
    let e = sys.base_value_of(attr, None, None, &mut cx).unwrap_err();
    assert!(
        is_bad_data(&e, "Modifier stack not found for attribute Loose"),
        "{e:?}"
    );
    let r = fx.property_resolver("Loose");
    let res = sys.value_resolver(&fx.w.space, r).unwrap();
    let e = res
        .storage(&sys, r, loose, &mut EvalCx::new(&mut fx.w))
        .unwrap_err();
    assert!(is_bad_data(&e, "Modifier stack not found"), "{e:?}");
}
