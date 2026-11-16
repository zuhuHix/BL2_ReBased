//! The cases of `tests/skill_stats_test.py` and `tests/skill_effects_test.py`, re-expressed on the synthetic world of
//! `fixture.rs` (invented paths and values, the Python tests' own), plus a test of every other rule of `skills.rs`.
//! Each re-expressed test names its Python source; the expected answer is the Python test's.

use ow_attr::stack::ModifierType;
use ow_object::value::Value;

use super::fixture::Fx;
use super::*;

// ---------------------------------------------------------------------------------------------------------------
// tests/skill_stats_test.py
// ---------------------------------------------------------------------------------------------------------------

fn flags() -> NumberFlags {
    NumberFlags::default()
}

/// `skill_stats_test.py` `test_number_format`, all seven asserts.
#[test]
fn number_format_as_skill_stats_test() {
    assert_eq!(format_number(0.05, flags()), "+5%");
    assert_eq!(
        format_number(
            -0.08,
            NumberFlags {
                sign: SignStyle::Negative,
                ..flags()
            }
        ),
        "-8%"
    );
    assert_eq!(
        format_number(
            -0.1,
            NumberFlags {
                sign: SignStyle::Positive,
                ..flags()
            }
        ),
        "+10%"
    );
    assert_eq!(
        format_number(
            0.1,
            NumberFlags {
                no_plus_sign: true,
                ..flags()
            }
        ),
        "10%"
    );
    assert_eq!(
        format_number(
            0.012,
            NumberFlags {
                percent_as_float: true,
                ..flags()
            }
        ),
        "+1.2%"
    );
    assert_eq!(
        format_number(
            0.5,
            NumberFlags {
                as_percentage: false,
                rounding_float: true,
                ..flags()
            }
        ),
        "+0.5"
    );
    assert_eq!(
        format_number(
            13.0,
            NumberFlags {
                as_percentage: false,
                no_plus_sign: true,
                ..flags()
            }
        ),
        "13"
    );
}

/// Python's `round` and `.1f` round half to even on the exact value; so does `format_number` (pins the tool's
/// rounding where a half falls exactly).
#[test]
fn number_format_rounds_half_to_even_as_python() {
    let n = NumberFlags {
        as_percentage: false,
        ..flags()
    };
    assert_eq!(format_number(12.5, n), "+12");
    assert_eq!(format_number(13.5, n), "+14");
    let f = NumberFlags {
        as_percentage: false,
        rounding_float: true,
        ..flags()
    };
    assert_eq!(format_number(0.25, f), "+0.2");
    assert_eq!(
        format_number(2.0, f),
        "+2",
        "trailing zeros and the point go"
    );
    assert_eq!(
        format_number(-0.0, flags()),
        "+0%",
        "negative zero is not below zero"
    );
}

/// `skill_stats_test.py` `test_localization_key`.
#[test]
fn localization_key_as_skill_stats_test() {
    let classes = |p: &str| -> Option<String> {
        match p {
            "Pkg" | "Pkg.Group" => Some("Core.Package".to_string()),
            "Pkg.Group.Skill" => Some("WillowGame.SkillDefinition".to_string()),
            _ => None,
        }
    };
    assert_eq!(localization_key("Pkg.Group.Skill", classes), "Group.Skill");
    assert_eq!(
        localization_key("Pkg.Group.Skill.Presentation_1", classes),
        "Group.Skill:Presentation_1"
    );
    // A deeper object joins with ':' once, then '.'.
    let deeper = |p: &str| -> Option<String> {
        match p {
            "Pkg.A" | "Pkg.A.B" => Some("Invented.Thing".to_string()),
            _ => None,
        }
    };
    assert_eq!(localization_key("Pkg.A.B.C", deeper), "A:B.C");
}

fn effect_of(fx: &Fx, start: i32, interval: i32) -> SkillEffect {
    let v = fx.effect(
        None,
        "TARGET_Self",
        "MT_Scale",
        fx.init_default(),
        fx.init_default(),
        start,
        interval,
        &[],
    );
    super::read_effect(&fx.w.space, &v).unwrap()
}

/// `skill_stats_test.py` `test_effect_value`. The Python adds float64 inputs and checks `0.3` to 1e-9; the rule is
/// single precision (NATIVE_SKILLS.md 4.3), so grade 3 gives the float `0.1 + 0.1 x 2`, which is 0.3 to 1e-7.
#[test]
fn effect_value_as_skill_stats_test() {
    let fx = Fx::new();
    let e = effect_of(&fx, 1, 1);
    assert_eq!(shown_value(&e, 0, 0.1, 0.1), None);
    let g3 = shown_value(&e, 3, 0.1, 0.1).unwrap();
    assert_eq!(g3.to_bits(), (0.1f32 + 0.1f32 * 2.0).to_bits());
    assert!((f64::from(g3) - 0.3).abs() < 1e-7);
    let every_other = effect_of(&fx, 1, 2);
    assert_eq!(shown_value(&every_other, 4, 1.0, 1.0), Some(2.0));
}

/// The fixture of `skill_stats_test.py` `test_skill_lines`.
fn lines_fixture(fx: &mut Fx) -> ObjRef {
    let speed = fx.runtime_attribute("Attr.Speed");
    let quote = fx.runtime_attribute("Attr.Quote");
    let chance = fx.runtime_attribute("Attr.Chance");
    let skill = fx.skill("Pkg.Group.Skill", "Invented", "SKILL_TYPE_Passive", 5);
    let a = fx.presentation("Pkg.Group.Skill.PresA", Some(speed), "Speed: $NUMBER$");
    fx.set(a, "bUseCustomNumberPlacement", Value::Bool(true));
    let b = fx.presentation("Pkg.Group.Skill.PresB", Some(quote), "Quoted text");
    fx.set(b, "bDontDisplayNumber", Value::Bool(true));
    let designer = fx.obj(
        "WillowGame.DesignerAttributeDefinition",
        "Pkg.Misc.Designer",
    );
    let base = fx.init(0.25, None, None, None);
    fx.set(designer, "BaseValue", base);
    let c = fx.presentation("Pkg.Group.Skill.PresC", Some(chance), "$NUMBER$ chance.");
    fx.set(c, "bUseCustomNumberPlacement", Value::Bool(true));
    fx.set(c, "bDontDisplayPlusSign", Value::Bool(true));
    let effects = vec![
        fx.effect(
            Some(speed),
            "TARGET_Self",
            "MT_Scale",
            fx.init(0.1, None, None, None),
            fx.init(0.1, None, None, None),
            1,
            1,
            &[],
        ),
        fx.effect(
            Some(quote),
            "TARGET_Self",
            "MT_Scale",
            fx.init(1.0, None, None, None),
            fx.init(0.0, None, None, None),
            1,
            1,
            &[],
        ),
        fx.effect(
            Some(chance),
            "TARGET_Self",
            "MT_Scale",
            fx.init(0.0, Some(designer), None, None),
            fx.init(0.0, None, None, None),
            1,
            1,
            &[],
        ),
    ];
    let effects = fx.array(effects);
    fx.set(skill, "SkillEffectDefinitions", effects);
    let pres = fx.objects(&[a, b, c]);
    fx.set(skill, "SkillEffectPresentations", pres);
    skill
}

/// `skill_stats_test.py` `test_skill_lines`: no notes, nothing at grade 0, and the three traced line shapes at grade 2
/// (custom placement with a localized text, a hidden number, a designer attribute's value without a plus sign).
#[test]
fn skill_lines_as_skill_stats_test() {
    let mut fx = Fx::new();
    let skill = lines_fixture(&mut fx);
    let mut loc = ow_core::DetMap::new();
    loc.insert(
        ("Group.Skill:PresA".to_string(), "Description".to_string()),
        "Localized speed: $NUMBER$".to_string(),
    );
    let sys = AttributeSystem::standard().unwrap();
    let mut cx = fx.cx();
    let def = read_skill(&mut cx, skill).unwrap();
    let mut reader = Reader::new(&sys);
    let (lines, notes) = skill_lines(&mut cx, &mut reader, &def, &loc, 2).unwrap();
    assert!(notes.is_empty(), "{notes:?}");
    assert!(lines[0].is_empty());
    let s = |a: &str, b: &str, c: &str| (a.to_string(), b.to_string(), c.to_string());
    assert_eq!(
        lines[2],
        vec![
            s("Localized speed: ", "+20%", ""),
            s("", "", " Quoted text"),
            s("", "25%", " chance.")
        ]
    );
}

/// The notes of `skill_lines`: a presentation with no matching effect, and an effect whose value does not reduce.
#[test]
fn skill_lines_notes_an_unmatched_presentation_and_an_unresolved_value() {
    let mut fx = Fx::new();
    let speed = fx.runtime_attribute("Attr.Speed");
    let other = fx.runtime_attribute("Attr.Other");
    let runtime = fx.runtime_attribute("Attr.Runtime");
    let skill = fx.skill("Pkg.S", "S", "SKILL_TYPE_Passive", 1);
    let a = fx.presentation("Pkg.S.A", Some(other), "x");
    let b = fx.presentation("Pkg.S.B", Some(speed), "y");
    let e = fx.effect(
        Some(speed),
        "TARGET_Self",
        "MT_Scale",
        fx.init(0.0, Some(runtime), None, None),
        fx.init_default(),
        1,
        1,
        &[],
    );
    let effects = fx.array(vec![e]);
    fx.set(skill, "SkillEffectDefinitions", effects);
    let pres = fx.objects(&[a, b]);
    fx.set(skill, "SkillEffectPresentations", pres);
    let sys = AttributeSystem::standard().unwrap();
    let mut cx = fx.cx();
    let def = read_skill(&mut cx, skill).unwrap();
    let mut reader = Reader::new(&sys);
    let (lines, notes) =
        skill_lines(&mut cx, &mut reader, &def, &ow_core::DetMap::new(), 1).unwrap();
    assert_eq!(
        notes,
        vec![
            "Pkg.S.A: no effect on Attr.Other".to_string(),
            "Pkg.S.B: unresolved value".to_string()
        ]
    );
    assert!(
        lines[1].is_empty(),
        "an unresolved value's line is left out"
    );
    assert_eq!(reader.unresolved, vec![runtime]);
}

/// `skill_stats_test.py` `test_effect_rows`. The Python's untagged `EffectTarget` reads as `None`; here it is the
/// enum's value 0, `TARGET_None` (the decoded struct has no "absent" field).
#[test]
fn effect_rows_as_skill_stats_test() {
    let mut fx = Fx::new();
    let time = fx.runtime_attribute("Attr.Time");
    let rate = fx.runtime_attribute("Attr.Rate");
    let skill = fx.skill("Pkg.S", "S", "SKILL_TYPE_Passive", 3);
    let effects = vec![
        fx.effect(
            Some(time),
            "TARGET_Self",
            "MT_PostAdd",
            fx.init(0.25, None, None, None),
            fx.init(0.25, None, None, None),
            1,
            1,
            &[],
        ),
        fx.effect(
            Some(rate),
            "TARGET_None",
            "MT_PreAdd",
            fx.init(-1.0, None, None, None),
            fx.init(0.0, None, None, None),
            0,
            1,
            &[],
        ),
    ];
    let effects = fx.array(effects);
    fx.set(skill, "SkillEffectDefinitions", effects);
    let sys = AttributeSystem::standard().unwrap();
    let mut cx = fx.cx();
    let def = read_skill(&mut cx, skill).unwrap();
    let rows = effect_rows(&mut cx, &mut Reader::new(&sys), &def, 3).unwrap();
    assert_eq!(rows[0].attribute, Some(time));
    assert_eq!(rows[0].modifier_type, ModifierType::PostAdd);
    assert_eq!(rows[0].target, EffectTarget::Self_);
    assert_eq!(rows[0].start_grade, 1);
    assert_eq!(
        rows[0].values,
        vec![None, Some(0.25), Some(0.5), Some(0.75)]
    );
    assert_eq!(rows[1].values, vec![Some(-1.0); 4]);
    assert_eq!(rows[1].target, EffectTarget::None);
}

// ---------------------------------------------------------------------------------------------------------------
// tests/skill_effects_test.py
// ---------------------------------------------------------------------------------------------------------------

/// The `OBJECTS` / `CLASSES` fixture of `skill_effects_test.py`.
fn effects_fixture(fx: &mut Fx) -> ObjRef {
    let attr_a = fx.runtime_attribute("Pkg.AttrA");
    let runtime = fx.runtime_attribute("Pkg.Runtime");
    let attr_c = fx.runtime_attribute("Pkg.AttrC");
    let attr_b = fx.runtime_attribute("Pkg.AttrB");
    let constant = fx.constant_attribute("Pkg.Constant", 3.0);
    let formula = fx.obj("Engine.AttributeInitializationDefinition", "Pkg.Formula");
    let cond = fx.make(fx.s.conditional, &[("bEnabled", Value::Bool(true))]);
    fx.set(formula, "ConditionalInitialization", cond);
    let skill = fx.skill("Pkg.Skill", "Invented", "SKILL_TYPE_Kill", 5);
    fx.set(skill, "bSubjectToGradeRules", Value::Bool(true));
    let effects = vec![
        fx.effect(
            Some(attr_a),
            "TARGET_Self",
            "MT_Scale",
            fx.init(0.25, None, None, None),
            fx.init(0.25, None, None, None),
            1,
            1,
            &[(3, 0.5)],
        ),
        // Untagged start grade and interval: the struct default 0.
        fx.effect(
            Some(attr_b),
            "TARGET_Self",
            "MT_PostAdd",
            fx.init(1.0, Some(runtime), None, Some(0.5)),
            fx.init(1.0, Some(constant), None, Some(2.0)),
            0,
            0,
            &[],
        ),
        fx.effect(
            None,
            "TARGET_None",
            "MT_Scale",
            fx.init(0.0, None, Some(formula), None),
            fx.init_default(),
            0,
            0,
            &[],
        ),
    ];
    let effects = fx.array(effects);
    fx.set(skill, "SkillEffectDefinitions", effects);
    // Constraints: an Or of an attribute test and the on-foot test, then a definition holding the healthy test.
    let compound = fx.obj(
        "GearboxFramework.CompoundExpressionEvaluator",
        "Pkg.Skill.Compound",
    );
    let left = fx.obj(
        "GearboxFramework.AttributeExpressionEvaluator",
        "Pkg.Skill.Left",
    );
    let right = fx.obj(
        "WillowGame.VehiclePassengerExpressionEvaluator",
        "Pkg.Skill.Right",
    );
    fx.set(right, "bNotInVehicle", Value::Bool(true));
    let expr = fx.make(
        fx.s.expr,
        &[
            ("AttributeOperand1", Value::Object(Some(attr_c))),
            ("ComparisonOperator", fx.byte("OPERATOR_LessThan")),
            ("ConstantOperand2", Value::Float(1.0)),
        ],
    );
    fx.set(left, "Expression", expr);
    fx.set(compound, "Expression1", Value::Object(Some(left)));
    let or = fx.byte("CEOP_Or");
    fx.set(compound, "Operator", or);
    fx.set(compound, "Expression2", Value::Object(Some(right)));
    let def = fx.obj("WillowGame.SkillExpressionEvaluatorDefinition", "Pkg.Def");
    let health = fx.obj(
        "WillowGame.HealthStateExpressionEvaluator",
        "Pkg.Def.Health",
    );
    fx.set(health, "bHealthy", Value::Bool(true));
    fx.set(def, "Evaluator", Value::Object(Some(health)));
    let c1 = fx.make(
        fx.s.constraint,
        &[
            ("Evaluator", Value::Object(Some(compound))),
            ("bApplyConstraintWhileActive", Value::Bool(true)),
            ("OnFailure", fx.byte("SKILL_Paused")),
        ],
    );
    let c2 = fx.make(
        fx.s.constraint,
        &[
            ("EvaluatorDefinitions", fx.objects(&[def])),
            ("bApplyConstraintOnActivatation", Value::Bool(true)),
        ],
    );
    let constraints = fx.array(vec![c1, c2]);
    fx.set(skill, "SkillConstraints", constraints);
    skill
}

fn entry(fx: &mut Fx) -> (SkillEntry, Vec<ObjRef>) {
    let skill = effects_fixture(fx);
    let sys = AttributeSystem::standard().unwrap();
    let mut cx = fx.cx();
    let mut reader = Reader::new(&sys);
    let e = skill_entry(&mut cx, &mut reader, skill).unwrap();
    (e, reader.unresolved)
}

/// `skill_effects_test.py` `test_skill_fields`.
#[test]
fn skill_fields_as_skill_effects_test() {
    let mut fx = Fx::new();
    let (e, _) = entry(&mut fx);
    assert_eq!(e.def.name, "Invented");
    assert_eq!(e.def.skill_type.label(), "Kill");
    assert_eq!(e.def.max_grade, 5);
    assert!(e.def.subject_to_grade_rules);
}

/// `skill_effects_test.py` `test_effect_values`.
#[test]
fn effect_values_as_skill_effects_test() {
    let mut fx = Fx::new();
    let (e, unresolved) = entry(&mut fx);
    let [a, b, c] = e.effects.as_slice() else {
        panic!("three rows")
    };
    assert_eq!(modifier_type_label(a.modifier_type), "Scale");
    assert_eq!(a.target.label(), "Self");
    assert_eq!(a.base, InitValue::Constant(0.25));
    assert_eq!(
        a.bonus,
        vec![Bonus {
            grade: 3,
            value: 0.5
        }]
    );
    assert_eq!(
        b.base,
        InitValue::Attribute {
            attribute: fx.at("Pkg.Runtime"),
            scale: 0.5
        }
    );
    assert_eq!(
        b.per_grade,
        InitValue::Constant(6.0),
        "a constant attribute resolves, times its scale"
    );
    assert_eq!(
        (b.start_grade, b.interval),
        (0, 0),
        "untagged start grade is the struct default 0"
    );
    assert_eq!(c.attribute, None);
    assert_eq!(c.target.label(), "None");
    assert_eq!(c.base, InitValue::Unresolved(fx.at("Pkg.Formula")));
    assert_eq!(unresolved, vec![fx.at("Pkg.Runtime"), fx.at("Pkg.Formula")]);
}

/// `skill_effects_test.py` `test_conditions`.
#[test]
fn conditions_as_skill_effects_test() {
    let mut fx = Fx::new();
    let (e, _) = entry(&mut fx);
    let [first, second] = e.conditions.as_slice() else {
        panic!("two rows")
    };
    assert_eq!(first.on_failure.label(), "Paused");
    assert!(first.while_active);
    let Test::Compound { op, left, right } = &first.test else {
        panic!("{:?}", first.test)
    };
    assert_eq!(op, "or");
    assert_eq!(right.kind(), "onFoot");
    assert_eq!(
        **left,
        Test::Attribute {
            attribute: Some(fx.at("Pkg.AttrC")),
            op: "LessThan".to_string(),
            attribute2: None,
            usage: "PreferAttribute".to_string(),
            constant: 1.0,
        }
    );
    assert_eq!(second.test.kind(), "healthState");
    assert!(second.on_activation);
    assert!(!second.while_active);
    assert_eq!(second.on_failure.label(), "Deactivated");
}

/// `skill_effects_test.py` `test_output_must_be_inside_local`. The Python resolves symbolic links (`Path.resolve`);
/// this resolves `..` lexically.
#[test]
fn output_must_be_inside_local_as_skill_effects_test() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let good = repo.join("local").join("ui").join("run");
    // The expected path, normalized here on its own: the crate folder's two parents, then local/ui/run.
    let mut parts: Vec<std::ffi::OsString> = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .components()
        .map(|c| c.as_os_str().to_os_string())
        .collect();
    parts.truncate(parts.len() - 2);
    let expect = parts
        .iter()
        .collect::<PathBuf>()
        .join("local")
        .join("ui")
        .join("run");
    assert_eq!(
        local_output(&good, &repo).unwrap(),
        expect,
        "the normalized path, not a suffix"
    );
    for bad in [
        repo.join("docs").join("verification"),
        repo.clone(),
        repo.join("local"),
        repo.join("local").join("..").join("docs"),
        repo.join("localx"),
    ] {
        assert!(
            local_output(&bad, &repo).is_err(),
            "{} was accepted",
            bad.display()
        );
    }
}

/// `skill_effects_test.py` `test_formula_value`: a definition whose only rule is a formula with a run-time term
/// stays a formula, each term itself a value. The Python's `mode == 0` is the enum's value 0, named here.
#[test]
fn formula_value_as_skill_effects_test() {
    let mut fx = Fx::new();
    let runtime = fx.runtime_attribute("Pkg.Runtime");
    let other = fx.runtime_attribute("Pkg.Other");
    let curve = fx.obj("Engine.AttributeInitializationDefinition", "Pkg.Curve");
    let formula = fx.make(
        fx.s.formula,
        &[
            ("bEnabled", Value::Bool(true)),
            ("Multiplier", fx.init(2.0, None, None, None)),
            ("Level", fx.init(0.0, Some(runtime), None, None)),
            ("Power", fx.init(1.0, None, None, None)),
            ("Offset", fx.init(0.0, Some(other), None, Some(-1.0))),
        ],
    );
    fx.set(curve, "ValueFormula", formula);
    let data = init_data(&fx.w.space, &fx.init(0.0, None, Some(curve), Some(0.5))).unwrap();
    let sys = AttributeSystem::standard().unwrap();
    let mut cx = fx.cx();
    let v = Reader::new(&sys).classify(&mut cx, &data).unwrap();
    let InitValue::Formula {
        terms,
        base,
        mode,
        scale,
    } = v
    else {
        panic!("{v:?}")
    };
    assert_eq!(scale, 0.5);
    assert_eq!(mode, "BASEVALUE_InitializationDefSetsBaseValue");
    assert_eq!(*base, InitValue::Constant(0.0));
    assert_eq!(terms.multiplier, InitValue::Constant(2.0));
    assert_eq!(terms.power, InitValue::Constant(1.0));
    assert_eq!(
        terms.level,
        InitValue::Attribute {
            attribute: runtime,
            scale: 1.0
        }
    );
    assert_eq!(
        terms.offset,
        InitValue::Attribute {
            attribute: other,
            scale: -1.0
        }
    );
}

// ---------------------------------------------------------------------------------------------------------------
// The other rules
// ---------------------------------------------------------------------------------------------------------------

/// A formula of plain constants reduces to the one evaluator's number (`ow-attr`, NATIVE_PROGRESSION.md section 1:
/// `Multiplier x (Level ^ Power + Offset)`, the definition setting the base, then the scale).
#[test]
fn a_formula_of_constants_is_the_evaluators_number() {
    let mut fx = Fx::new();
    let def = fx.obj("Engine.AttributeInitializationDefinition", "Pkg.Plain");
    let formula = fx.make(
        fx.s.formula,
        &[
            ("bEnabled", Value::Bool(true)),
            ("Multiplier", fx.init(3.0, None, None, None)),
            ("Level", fx.init(2.0, None, None, None)),
            ("Power", fx.init(3.0, None, None, None)),
            ("Offset", fx.init(1.0, None, None, None)),
        ],
    );
    fx.set(def, "ValueFormula", formula);
    let data = init_data(&fx.w.space, &fx.init(0.0, None, Some(def), Some(2.0))).unwrap();
    let sys = AttributeSystem::standard().unwrap();
    let mut cx = fx.cx();
    assert_eq!(
        Reader::new(&sys).classify(&mut cx, &data).unwrap(),
        InitValue::Constant(54.0)
    );
}

/// A definition with a variance, a range or a rounding mode is not reduced here.
#[test]
fn a_definition_with_variance_range_or_rounding_is_unresolved() {
    for case in 0..3 {
        let mut fx = Fx::new();
        let def = fx.obj("Engine.AttributeInitializationDefinition", "Pkg.D");
        let formula = fx.make(fx.s.formula, &[("bEnabled", Value::Bool(true))]);
        fx.set(def, "ValueFormula", formula);
        match case {
            0 => {
                let v = fx.make(fx.s.variance, &[("bEnabled", Value::Bool(true))]);
                fx.set(def, "RandomVariance", v);
            }
            1 => {
                let v = fx.make(
                    fx.s.range,
                    &[("bEnableMaxValueRestriction", Value::Bool(true))],
                );
                fx.set(def, "RangeRestriction", v);
            }
            _ => {
                let b = fx.byte("ATTRROUNDING_IntRound");
                fx.set(def, "RoundingMode", b);
            }
        }
        let data = init_data(&fx.w.space, &fx.init(0.0, None, Some(def), None)).unwrap();
        let sys = AttributeSystem::standard().unwrap();
        let mut cx = fx.cx();
        assert_eq!(
            Reader::new(&sys).classify(&mut cx, &data).unwrap(),
            InitValue::Unresolved(def),
            "case {case}"
        );
    }
}

/// A designer attribute's `BaseValue` is its constant; a chain of two resolvers is not a constant attribute.
#[test]
fn constant_attributes_designer_base_values_and_longer_chains() {
    let mut fx = Fx::new();
    let designer = fx.obj("WillowGame.DesignerAttributeDefinition", "Pkg.Designer");
    let base = fx.init(5.0, None, None, Some(2.0));
    fx.set(designer, "BaseValue", base);
    let two = fx.constant_attribute("Pkg.Two", 1.0);
    let r = fx.at("Pkg.Two.ConstantAttributeValueResolver_0");
    let chain = fx.objects(&[r, r]);
    fx.set(two, "ValueResolverChain", chain);
    let sys = AttributeSystem::standard().unwrap();
    let mut cx = fx.cx();
    let mut reader = Reader::new(&sys);
    assert_eq!(reader.constant_of(&mut cx, designer).unwrap(), Some(10.0));
    assert_eq!(reader.constant_of(&mut cx, two).unwrap(), None);
}

/// The grade rule (NATIVE_SKILLS.md 4.3): 0 below the start grade, integer division by `max(interval, 1)`, the
/// nearest bonus at or below the grade (not summed), single precision.
#[test]
fn the_grade_rule_with_its_start_interval_and_bonus() {
    let fx = Fx::new();
    let v = fx.effect(
        None,
        "TARGET_Self",
        "MT_Scale",
        fx.init_default(),
        fx.init_default(),
        2,
        0,
        &[(3, 0.5), (5, 2.0), (4, 1.0)],
    );
    let e = read_effect(&fx.w.space, &v).unwrap();
    assert_eq!(
        modifier_value(&e, 1, 10.0, 1.0),
        0.0,
        "below the start grade"
    );
    assert_eq!(
        modifier_value(&e, 2, 10.0, 1.0),
        10.0,
        "interval 0 is every grade"
    );
    assert_eq!(modifier_value(&e, 3, 10.0, 1.0), 11.5);
    assert_eq!(
        modifier_value(&e, 4, 10.0, 1.0),
        13.0,
        "the nearest bonus (4), not the sum"
    );
    assert_eq!(modifier_value(&e, 6, 10.0, 1.0), 16.0);
    let v = fx.effect(
        None,
        "TARGET_Self",
        "MT_Scale",
        fx.init_default(),
        fx.init_default(),
        1,
        3,
        &[],
    );
    let e = read_effect(&fx.w.space, &v).unwrap();
    assert_eq!(modifier_value(&e, 7, 0.5, 0.5), 1.5, "(7 - 1) div 3 = 2");
    // Phaselock's Suspension shape (PHASELOCK_STOCK_DATA.md, data): base 0.5, +0.5 per grade, start 1, interval 1.
    let v = fx.effect(
        None,
        "TARGET_Self",
        "MT_PostAdd",
        fx.init_default(),
        fx.init_default(),
        1,
        1,
        &[],
    );
    let e = read_effect(&fx.w.space, &v).unwrap();
    let suspension: Vec<f32> = (0..=5).map(|g| modifier_value(&e, g, 0.5, 0.5)).collect();
    assert_eq!(suspension, vec![0.0, 0.5, 1.0, 1.5, 2.0, 2.5]);
}

/// A bonus 9999 or more grades below the grade does not count (NATIVE_SKILLS.md 4.3).
#[test]
fn a_far_bonus_does_not_count() {
    let b = [Bonus {
        grade: 1,
        value: 3.0,
    }];
    assert_eq!(bonus_at(&b, 9999), 3.0);
    assert_eq!(bonus_at(&b, 10000), 0.0);
    assert_eq!(bonus_at(&b, 0), 0.0);
    assert_eq!(
        bonus_at(
            &[Bonus {
                grade: i32::MIN,
                value: 1.0
            }],
            i32::MAX
        ),
        0.0,
        "no overflow"
    );
}

/// `DoesSkillPassMinGradeTest` and `Skill.UpdateGrade`.
#[test]
fn min_grade_test_and_update_grade() {
    assert!(!passes_min_grade_test(true, 0));
    assert!(!passes_min_grade_test(true, -1));
    assert!(passes_min_grade_test(true, 1));
    assert!(passes_min_grade_test(false, 0));
    assert_eq!(update_grade(0), 1);
    assert_eq!(update_grade(-4), 1);
    assert_eq!(update_grade(3), 3);
}

/// A byte outside its enum, and a byte whose declaration names no enum, are bad data (enum values by name).
#[test]
fn enum_bytes_are_named_through_their_enum() {
    let mut fx = Fx::new();
    let skill = fx.skill("Pkg.S", "S", "SKILL_TYPE_Passive", 1);
    fx.set(skill, "SkillType", Value::Byte(9));
    let mut cx = fx.cx();
    assert!(matches!(
        read_skill(&mut cx, skill),
        Err(SkillError::BadData { .. })
    ));
    let kind = PropKind::Byte { enum_: None };
    assert!(enum_value_name(&fx.w.space, "x", &kind, &Value::Byte(0)).is_err());
}

/// The localization file reader: UTF-16 with a byte-order mark, the section's last word dropped, quotes removed; a
/// missing file is an empty map.
#[test]
fn localization_files_are_read_as_the_tool_reads_them() {
    let dir = std::env::temp_dir().join(format!("ow-skills-loc-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("Invented.int");
    let text = "[Group.Skill SkillDefinition]\r\nSkillDescription=\"Invented words\"\r\nSkillName=Plain\r\n";
    let mut bytes = vec![0xFF, 0xFE];
    for u in text.encode_utf16() {
        bytes.extend_from_slice(&u.to_le_bytes());
    }
    std::fs::write(&file, bytes).unwrap();
    let map = read_localization(&file).unwrap();
    assert_eq!(
        map.get(&("Group.Skill".to_string(), "SkillDescription".to_string()))
            .map(String::as_str),
        Some("Invented words")
    );
    assert_eq!(
        map.get(&("Group.Skill".to_string(), "SkillName".to_string()))
            .map(String::as_str),
        Some("Plain")
    );
    assert!(
        read_localization(&dir.join("Missing.int"))
            .unwrap()
            .is_empty()
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A cycle of outers is refused, not followed for ever.
#[test]
fn an_outer_cycle_is_refused() {
    let mut fx = Fx::new();
    let a = fx.obj("Core.Package", "A");
    let b = fx.obj("Core.Package", "A.B");
    fx.w.space.heap.get_mut(a).unwrap().outer = Some(b);
    assert!(matches!(
        object_path(&fx.w.space, b),
        Err(SkillError::BadData { .. })
    ));
}

/// A `SkillError` carries its cause through `source()`, and a `NotYet` from below stays a `NotYet`.
#[test]
fn errors_carry_their_cause() {
    use std::error::Error as _;
    let e = SkillError::from(AttrError::BadData { reason: "x".into() });
    assert!(e.source().is_some());
    let n = SkillError::from(AttrError::NotYet(NotYet::not_read("y", RECORD)));
    assert!(matches!(n, SkillError::NotYet(_)));
    assert!(matches!(VmError::from(n), VmError::NotYet(_)));
}
