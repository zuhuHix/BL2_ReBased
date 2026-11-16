//! A synthetic object world for the skill tests (AGENTS.md rule 9: invented objects and numbers only). The class,
//! struct, field and enum value names are the packages' names (names of game objects, which the docs may hold), so
//! the readers see the shapes they read from reflection; every value is made up. Objects are built in memory, the
//! way the `ow-attr` evaluator tests build theirs, and named by invented paths (`Pkg.Group.Skill`), with each outer
//! a `Core.Package` object unless the test makes it something else.

use ow_attr::AttributeSystem;
use ow_attr::api::{AttrError, AttrWorld, EngineQuery, EvalCx, GlobalSlots, NumForms, WorldParts};
use ow_core::NotYet;
use ow_core::rand::{CrtStream, Rand15};
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, EnumId, ObjRef, PropAddr, PropId, StructId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::{StructValue, Value};

/// The world: a hand-filled space and the global slots.
pub(crate) struct World {
    pub space: ObjectSpace,
    pub slots: GlobalSlots,
}

impl AttrWorld for World {
    fn parts(&mut self) -> WorldParts<'_> {
        WorldParts::new(&mut self.space, None)
    }
    fn globals(&mut self) -> Result<ObjRef, AttrError> {
        Err(NotYet::layer_absent("the globals", "ow-world", "skills test world").into())
    }
    fn game_info(&mut self) -> Result<Option<ObjRef>, AttrError> {
        Ok(None)
    }
    fn engine_query(
        &mut self,
        q: EngineQuery,
        source: ObjRef,
    ) -> Result<Option<ObjRef>, AttrError> {
        Err(NotYet::layer_absent(
            format!("engine query {q:?} from {source:?}"),
            "ow-world",
            "skills test world",
        )
        .into())
    }
    fn global_slots(&mut self) -> &mut GlobalSlots {
        &mut self.slots
    }
    fn attribute_changed(
        &mut self,
        _owner: ObjRef,
        _prop: PropId,
        _forms: NumForms,
    ) -> Result<(), AttrError> {
        Ok(())
    }
    fn rng(&mut self, _s: CrtStream) -> Result<&mut dyn Rand15, AttrError> {
        Err(NotYet::layer_absent("a random source", "ow-world", "skills test world").into())
    }
}

/// The enums the readers name bytes through, with the packages' value names (ESignStyle's first name is invented:
/// the readers only name `Positive` and `Negative`).
pub(crate) const ENUMS: &[(&str, &[&str])] = &[
    (
        "WillowGame.SkillDefinition.ESkillType",
        &[
            "SKILL_TYPE_Passive",
            "SKILL_TYPE_Action",
            "SKILL_TYPE_ActionAugment",
            "SKILL_TYPE_Kill",
            "SKILL_TYPE_Proficiency",
        ],
    ),
    (
        "WillowGame.Skill.ESkillState",
        &["SKILL_Deactivated", "SKILL_Active", "SKILL_Paused"],
    ),
    (
        "WillowGame.SkillDefinition.EEffectDurationType",
        &["DURATION_Infinite", "DURATION_Timed"],
    ),
    (
        "WillowGame.SkillDefinition.EEffectTarget",
        &[
            "TARGET_None",
            "TARGET_Self",
            "TARGET_Allies",
            "TARGET_Enemies",
            "TARGET_All",
            "TARGET_Pets",
        ],
    ),
    (
        "Core.AttributeModifier.EModifierType",
        &["MT_Scale", "MT_PreAdd", "MT_PostAdd"],
    ),
    (
        "Engine.AttributeExpression.EComparisonOperator",
        &[
            "OPERATOR_EqualTo",
            "OPERATOR_NotEqualTo",
            "OPERATOR_LessThan",
            "OPERATOR_LessThanOrEqual",
            "OPERATOR_GreaterThan",
            "OPERATOR_GreaterThanOrEqual",
        ],
    ),
    (
        "Engine.AttributeExpression.EOperand2Usage",
        &["OPERAND_PreferAttribute", "OPERAND_Multiply"],
    ),
    (
        "GearboxFramework.CompoundExpressionEvaluator.ECompoundExpressionEvaluatorOperator",
        &["CEOP_And", "CEOP_Or"],
    ),
    (
        "WillowGame.AttributePresentationDefinition.ESignStyle",
        &[
            "SIGNSTYLE_Invented",
            "SIGNSTYLE_Positive",
            "SIGNSTYLE_Negative",
        ],
    ),
    (
        "Engine.AttributeInitializationDefinition.EAttributeRoundingMode",
        &[
            "ATTRROUNDING_Float",
            "ATTRROUNDING_IntRound",
            "ATTRROUNDING_IntFloor",
            "ATTRROUNDING_IntCeil",
        ],
    ),
    (
        "Engine.AttributeInitializationDefinition.EBaseValueMode",
        &[
            "BASEVALUE_InitializationDefSetsBaseValue",
            "BASEVALUE_InitializationDefAddsToBaseValue",
            "BASEVALUE_InitializationDefScalesBaseValue",
            "BASEVALUE_InitializationDefOffsetByBaseValue",
        ],
    ),
];

/// The structs the readers read.
pub(crate) struct Structs {
    pub aid: StructId,
    pub bonus: StructId,
    pub effect: StructId,
    pub constraint: StructId,
    pub expr: StructId,
    pub formula: StructId,
    pub conditional: StructId,
    pub variance: StructId,
    pub range: StructId,
}

/// The fixture: the world, the attribute system and the paths of the objects made.
#[allow(dead_code)]
pub(crate) struct Fx {
    pub w: World,
    pub sys: AttributeSystem,
    pub s: Structs,
    pub object: ClassId,
    pub package: ClassId,
    objects: Vec<(String, ObjRef)>,
}

fn decl(space: &mut ObjectSpace, n: &str, k: PropKind) -> PropertyDecl {
    PropertyDecl::new(space.names.intern(n), k)
}

#[allow(dead_code)]
impl Fx {
    pub fn new() -> Fx {
        let mut space = ObjectSpace::new();
        let object = space.classes.define("Core.Object", None, vec![]).unwrap();
        let package = space
            .classes
            .define("Core.Package", Some(object), vec![])
            .unwrap();
        let mut enums: Vec<(&str, EnumId)> = Vec::new();
        for (path, values) in ENUMS {
            let names = values.iter().map(|v| space.names.intern(v)).collect();
            enums.push((path, space.classes.define_enum(path, names).unwrap()));
        }
        let e = |p: &str| -> PropKind {
            let id = enums.iter().find(|(x, _)| x.ends_with(p)).unwrap().1;
            PropKind::Byte { enum_: Some(id) }
        };
        let obj = PropKind::Object { class: object };
        let objs = PropKind::Array(Box::new(PropKind::Object { class: object }));
        let st = |space: &mut ObjectSpace, path: &str, fields: Vec<(&str, PropKind)>| -> StructId {
            let decls = fields.into_iter().map(|(n, k)| decl(space, n, k)).collect();
            space.classes.define_struct(path, None, decls).unwrap()
        };
        let aid = st(
            &mut space,
            "Engine.AttributeInitializationDefinition.AttributeInitializationData",
            vec![
                ("BaseValueConstant", PropKind::Float),
                ("BaseValueAttribute", obj.clone()),
                ("InitializationDefinition", obj.clone()),
                ("BaseValueScaleConstant", PropKind::Float),
            ],
        );
        let bonus = st(
            &mut space,
            "WillowGame.SkillDefinition.SkillBonusUpgrade",
            vec![
                ("GradeToApplyAt", PropKind::Int),
                ("Modifier", PropKind::Float),
            ],
        );
        let effect = st(
            &mut space,
            "WillowGame.SkillDefinition.SkillEffectData",
            vec![
                ("AttributeToModify", obj.clone()),
                ("bIncludeSelfAsTarget", PropKind::Bool),
                ("EffectTarget", e("EEffectTarget")),
                ("ModifierType", e("EModifierType")),
                ("BaseModifierValue", PropKind::Struct(aid)),
                ("GradeToStartApplyingEffect", PropKind::Int),
                ("PerGradeUpgradeInterval", PropKind::Int),
                ("PerGradeUpgrade", PropKind::Struct(aid)),
                (
                    "BonusUpgradeList",
                    PropKind::Array(Box::new(PropKind::Struct(bonus))),
                ),
            ],
        );
        let constraint = st(
            &mut space,
            "WillowGame.SkillDefinition.SkillConstraintData",
            vec![
                ("Evaluator", obj.clone()),
                ("EvaluatorDefinitions", objs.clone()),
                ("bApplyConstraintOnActivatation", PropKind::Bool),
                ("bApplyConstraintWhileActive", PropKind::Bool),
                ("bApplyConstraintWhilePaused", PropKind::Bool),
                ("OnFailure", e("ESkillState")),
            ],
        );
        let expr = st(
            &mut space,
            "Engine.AttributeExpression.AttributeExpressionData",
            vec![
                ("AttributeOperand1", obj.clone()),
                ("ComparisonOperator", e("EComparisonOperator")),
                ("Operand2Usage", e("EOperand2Usage")),
                ("AttributeOperand2", obj.clone()),
                ("ConstantOperand2", PropKind::Float),
            ],
        );
        let formula = st(
            &mut space,
            "Engine.AttributeInitializationDefinition.BalanceFormula",
            vec![
                ("bEnabled", PropKind::Bool),
                ("Multiplier", PropKind::Struct(aid)),
                ("Level", PropKind::Struct(aid)),
                ("Power", PropKind::Struct(aid)),
                ("Offset", PropKind::Struct(aid)),
            ],
        );
        let entry = st(
            &mut space,
            "Engine.AttributeInitializationDefinition.ConditionalValueExpression",
            vec![
                (
                    "Expressions",
                    PropKind::Array(Box::new(PropKind::Struct(expr))),
                ),
                ("BaseValueIfTrue", PropKind::Struct(aid)),
            ],
        );
        let conditional = st(
            &mut space,
            "Engine.AttributeInitializationDefinition.ConditionalInitializationExpressions",
            vec![
                ("bEnabled", PropKind::Bool),
                (
                    "ConditionalExpressionList",
                    PropKind::Array(Box::new(PropKind::Struct(entry))),
                ),
                ("DefaultBaseValue", PropKind::Struct(aid)),
            ],
        );
        let variance = st(
            &mut space,
            "Engine.AttributeInitializationDefinition.Variance",
            vec![
                ("bEnabled", PropKind::Bool),
                ("LowerBound", PropKind::Struct(aid)),
                ("UpperBound", PropKind::Struct(aid)),
                ("bUseIntegerRandomization", PropKind::Bool),
            ],
        );
        let range = st(
            &mut space,
            "Engine.AttributeInitializationDefinition.Range",
            vec![
                ("bEnableMinValueRestriction", PropKind::Bool),
                ("MinValue", PropKind::Struct(aid)),
                ("bEnableMaxValueRestriction", PropKind::Bool),
                ("MaxValue", PropKind::Struct(aid)),
            ],
        );
        let class = |space: &mut ObjectSpace,
                     path: &str,
                     sup: Option<ClassId>,
                     fields: Vec<(&str, PropKind)>|
         -> ClassId {
            let decls = fields.into_iter().map(|(n, k)| decl(space, n, k)).collect();
            space
                .classes
                .define(path, Some(sup.unwrap_or(object)), decls)
                .unwrap()
        };
        class(
            &mut space,
            "WillowGame.SkillDefinition",
            None,
            vec![
                ("SkillName", PropKind::Str),
                ("SkillType", e("ESkillType")),
                ("MaxGrade", PropKind::Int),
                ("bSubjectToGradeRules", PropKind::Bool),
                ("DurationType", e("EEffectDurationType")),
                ("InitialDuration", PropKind::Float),
                ("DefaultStartingGrade", PropKind::Int),
                ("bCanBeToggledOff", PropKind::Bool),
                ("ActionSkillArchetype", obj.clone()),
                (
                    "SkillEffectDefinitions",
                    PropKind::Array(Box::new(PropKind::Struct(effect))),
                ),
                (
                    "SkillConstraints",
                    PropKind::Array(Box::new(PropKind::Struct(constraint))),
                ),
                ("SkillEffectPresentations", objs.clone()),
            ],
        );
        let attribute = class(
            &mut space,
            "Engine.AttributeDefinition",
            None,
            vec![
                ("ContextResolverChain", objs.clone()),
                ("ValueResolverChain", objs.clone()),
            ],
        );
        class(
            &mut space,
            "WillowGame.DesignerAttributeDefinition",
            Some(attribute),
            vec![("BaseValue", PropKind::Struct(aid))],
        );
        class(
            &mut space,
            "GearboxFramework.ConstantAttributeValueResolver",
            None,
            vec![("ConstantValue", PropKind::Float)],
        );
        class(
            &mut space,
            "Engine.AttributeInitializationDefinition",
            None,
            vec![
                ("ValueFormula", PropKind::Struct(formula)),
                ("ConditionalInitialization", PropKind::Struct(conditional)),
                ("RandomVariance", PropKind::Struct(variance)),
                ("RangeRestriction", PropKind::Struct(range)),
                ("RoundingMode", e("EAttributeRoundingMode")),
                ("BaseValueMode", e("EBaseValueMode")),
            ],
        );
        class(
            &mut space,
            "GearboxFramework.CompoundExpressionEvaluator",
            None,
            vec![
                ("Expression1", obj.clone()),
                ("Operator", e("ECompoundExpressionEvaluatorOperator")),
                ("Expression2", obj.clone()),
            ],
        );
        class(
            &mut space,
            "GearboxFramework.AttributeExpressionEvaluator",
            None,
            vec![("Expression", PropKind::Struct(expr))],
        );
        class(
            &mut space,
            "WillowGame.VehiclePassengerExpressionEvaluator",
            None,
            vec![
                ("bNotInVehicle", PropKind::Bool),
                ("bAttachedRider", PropKind::Bool),
            ],
        );
        class(
            &mut space,
            "WillowGame.HealthStateExpressionEvaluator",
            None,
            vec![
                ("bHealthy", PropKind::Bool),
                ("bInjured", PropKind::Bool),
                ("bDead", PropKind::Bool),
            ],
        );
        class(
            &mut space,
            "WillowGame.SkillExpressionEvaluatorDefinition",
            None,
            vec![("Evaluator", obj.clone())],
        );
        class(
            &mut space,
            "WillowGame.AttributePresentationDefinition",
            None,
            vec![
                ("Attribute", obj.clone()),
                ("Description", PropKind::Str),
                ("bDisplayAsPercentage", PropKind::Bool),
                ("RoundingMode", e("EAttributeRoundingMode")),
                ("bDisplayPercentAsFloat", PropKind::Bool),
                ("SignStyle", e("ESignStyle")),
                ("bDontDisplayPlusSign", PropKind::Bool),
                ("bUseCustomNumberPlacement", PropKind::Bool),
                ("bDontDisplayNumber", PropKind::Bool),
            ],
        );
        Fx {
            w: World {
                space,
                slots: GlobalSlots::default(),
            },
            sys: AttributeSystem::standard().unwrap(),
            s: Structs {
                aid,
                bonus,
                effect,
                constraint,
                expr,
                formula,
                conditional,
                variance,
                range,
            },
            object,
            package,
            objects: Vec::new(),
        }
    }

    /// Defines a class (for tests that need one the fixture lacks).
    pub fn define(
        &mut self,
        path: &str,
        sup: Option<ClassId>,
        fields: Vec<(&str, PropKind)>,
    ) -> ClassId {
        let space = &mut self.w.space;
        let decls = fields.into_iter().map(|(n, k)| decl(space, n, k)).collect();
        space
            .classes
            .define(path, Some(sup.unwrap_or(self.object)), decls)
            .unwrap()
    }

    /// The object at `path`, made (with its outers as packages) of class `class` if new. `class` of a known object
    /// is not checked.
    pub fn obj(&mut self, class: &str, path: &str) -> ObjRef {
        if let Some((_, o)) = self.objects.iter().find(|(p, _)| p == path) {
            return *o;
        }
        let (outer, name) = match path.rsplit_once('.') {
            Some((outer, name)) => (Some(self.obj("Core.Package", outer)), name),
            None => (None, path),
        };
        let c = self
            .w
            .space
            .classes
            .by_path(class)
            .unwrap_or_else(|| panic!("class {class}"));
        let o = self.w.space.alloc(c, name, outer, Origin::Runtime).unwrap();
        self.objects.push((path.to_string(), o));
        o
    }

    /// The object made at `path`.
    pub fn at(&self, path: &str) -> ObjRef {
        self.objects
            .iter()
            .find(|(p, _)| p == path)
            .unwrap_or_else(|| panic!("no object {path}"))
            .1
    }

    /// Sets property `name` of `o`.
    pub fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.w.space.heap.get(o).unwrap().class;
        let p = self
            .w
            .space
            .prop(class, name)
            .unwrap_or_else(|| panic!("no property {name}"));
        self.w.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    /// The byte of enum value `value` (searched in every fixture enum).
    pub fn byte(&self, value: &str) -> Value {
        let i = ENUMS
            .iter()
            .find_map(|(_, values)| values.iter().position(|v| *v == value))
            .unwrap_or_else(|| panic!("no enum value {value}"));
        Value::Byte(u8::try_from(i).unwrap())
    }

    /// A struct value of `ty` from its zero value with the named fields set.
    pub fn make(&self, ty: StructId, fields: &[(&str, Value)]) -> Value {
        let layout = self.w.space.classes.struct_layout(ty).unwrap();
        let mut v = match layout.zero() {
            Value::Struct(s) => StructValue::clone(s),
            _ => panic!("no zero value"),
        };
        for (n, x) in fields {
            let id = self
                .w
                .space
                .names
                .find(n)
                .unwrap_or_else(|| panic!("no name {n}"));
            let (entry, _) = layout.field(id).unwrap_or_else(|| panic!("no field {n}"));
            v.fields[usize::try_from(entry).unwrap()] = x.clone();
        }
        Value::Struct(std::rc::Rc::new(v))
    }

    /// An `AttributeInitializationData`: the constant, the attribute, the definition and the scale (the struct
    /// default 1 when `None`).
    pub fn init(
        &self,
        constant: f32,
        attribute: Option<ObjRef>,
        definition: Option<ObjRef>,
        scale: Option<f32>,
    ) -> Value {
        self.make(
            self.s.aid,
            &[
                ("BaseValueConstant", Value::Float(constant)),
                ("BaseValueAttribute", Value::Object(attribute)),
                ("InitializationDefinition", Value::Object(definition)),
                ("BaseValueScaleConstant", Value::Float(scale.unwrap_or(1.0))),
            ],
        )
    }

    /// The zero `AttributeInitializationData` with the scale default 1 (an untagged struct).
    pub fn init_default(&self) -> Value {
        self.init(0.0, None, None, None)
    }

    /// An evaluation context over the world.
    pub fn cx(&mut self) -> EvalCx<'_> {
        EvalCx::new(&mut self.w)
    }

    /// A `SkillDefinition` at `path` with its name, type value name and max grade; the rest at zero.
    pub fn skill(&mut self, path: &str, name: &str, skill_type: &str, max_grade: i32) -> ObjRef {
        let o = self.obj("WillowGame.SkillDefinition", path);
        self.set(o, "SkillName", Value::Str(name.into()));
        let b = self.byte(skill_type);
        self.set(o, "SkillType", b);
        self.set(o, "MaxGrade", Value::Int(max_grade));
        o
    }

    /// A `SkillEffectData` value.
    #[allow(clippy::too_many_arguments)]
    pub fn effect(
        &self,
        attribute: Option<ObjRef>,
        target: &str,
        modifier: &str,
        base: Value,
        per: Value,
        start: i32,
        interval: i32,
        bonus: &[(i32, f32)],
    ) -> Value {
        let bonus = bonus
            .iter()
            .map(|(g, v)| {
                self.make(
                    self.s.bonus,
                    &[
                        ("GradeToApplyAt", Value::Int(*g)),
                        ("Modifier", Value::Float(*v)),
                    ],
                )
            })
            .collect();
        self.make(
            self.s.effect,
            &[
                ("AttributeToModify", Value::Object(attribute)),
                ("EffectTarget", self.byte(target)),
                ("ModifierType", self.byte(modifier)),
                ("BaseModifierValue", base),
                ("GradeToStartApplyingEffect", Value::Int(start)),
                ("PerGradeUpgradeInterval", Value::Int(interval)),
                ("PerGradeUpgrade", per),
                ("BonusUpgradeList", Value::Array(std::rc::Rc::new(bonus))),
            ],
        )
    }

    /// An `AttributeDefinition` whose value chain is one `ConstantAttributeValueResolver` of `value`.
    pub fn constant_attribute(&mut self, path: &str, value: f32) -> ObjRef {
        let a = self.obj("Engine.AttributeDefinition", path);
        let r = self.obj(
            "GearboxFramework.ConstantAttributeValueResolver",
            &format!("{path}.ConstantAttributeValueResolver_0"),
        );
        self.set(r, "ConstantValue", Value::Float(value));
        self.set(
            a,
            "ValueResolverChain",
            Value::Array(std::rc::Rc::new(vec![Value::Object(Some(r))])),
        );
        a
    }

    /// An attribute that needs run-time state (an empty value chain).
    pub fn runtime_attribute(&mut self, path: &str) -> ObjRef {
        self.obj("Engine.AttributeDefinition", path)
    }

    /// An array value of objects.
    pub fn objects(&self, os: &[ObjRef]) -> Value {
        Value::Array(std::rc::Rc::new(
            os.iter().map(|o| Value::Object(Some(*o))).collect(),
        ))
    }

    /// An array value of values.
    pub fn array(&self, vs: Vec<Value>) -> Value {
        Value::Array(std::rc::Rc::new(vs))
    }

    /// An `AttributePresentationDefinition` at `path` with the class defaults the tool assumes (as a percentage).
    pub fn presentation(
        &mut self,
        path: &str,
        attribute: Option<ObjRef>,
        description: &str,
    ) -> ObjRef {
        let p = self.obj("WillowGame.AttributePresentationDefinition", path);
        self.set(p, "Attribute", Value::Object(attribute));
        self.set(p, "Description", Value::Str(description.into()));
        self.set(p, "bDisplayAsPercentage", Value::Bool(true));
        p
    }
}

/// The tree structs (`fixture.tree_classes()`).
pub(crate) struct TreeStructs {
    pub tier: StructId,
    pub row: StructId,
}

#[allow(dead_code)]
impl Fx {
    /// Defines the skill tree classes: `SkillTreeDefinition` (`Root`), `SkillTreeBranchDefinition` (`Tiers`,
    /// `Children`, `Layout`), `SkillTreeBranchLayoutDefinition` (`Tiers` of rows with `bCellIsOccupied[3]`) and
    /// `PlayerSkillTree`.
    pub fn tree_classes(&mut self) -> TreeStructs {
        let object = self.object;
        let space = &mut self.w.space;
        let obj = PropKind::Object { class: object };
        let objs = PropKind::Array(Box::new(PropKind::Object { class: object }));
        let tier = {
            let decls = vec![
                decl(space, "Skills", objs.clone()),
                decl(space, "PointsToUnlockNextTier", PropKind::Int),
            ];
            space
                .classes
                .define_struct(
                    "WillowGame.SkillTreeBranchDefinition.SkillTreeTier",
                    None,
                    decls,
                )
                .unwrap()
        };
        let row = {
            let decls = vec![decl(space, "bCellIsOccupied", PropKind::Bool).with_array_dim(3)];
            space
                .classes
                .define_struct(
                    "WillowGame.SkillTreeBranchLayoutDefinition.TierLayout",
                    None,
                    decls,
                )
                .unwrap()
        };
        let decls = vec![decl(space, "Root", obj.clone())];
        space
            .classes
            .define("WillowGame.SkillTreeDefinition", Some(object), decls)
            .unwrap();
        let decls = vec![
            decl(
                space,
                "Tiers",
                PropKind::Array(Box::new(PropKind::Struct(tier))),
            ),
            decl(space, "Children", objs.clone()),
            decl(space, "Layout", obj.clone()),
        ];
        space
            .classes
            .define("WillowGame.SkillTreeBranchDefinition", Some(object), decls)
            .unwrap();
        let decls = vec![decl(
            space,
            "Tiers",
            PropKind::Array(Box::new(PropKind::Struct(row))),
        )];
        space
            .classes
            .define(
                "WillowGame.SkillTreeBranchLayoutDefinition",
                Some(object),
                decls,
            )
            .unwrap();
        space
            .classes
            .define("WillowGame.PlayerSkillTree", Some(object), vec![])
            .unwrap();
        TreeStructs { tier, row }
    }

    /// A tier value: its skills and `PointsToUnlockNextTier`.
    pub fn tier(&self, t: &TreeStructs, skills: &[ObjRef], points: i32) -> Value {
        self.make(
            t.tier,
            &[
                ("Skills", self.objects(skills)),
                ("PointsToUnlockNextTier", Value::Int(points)),
            ],
        )
    }

    /// A layout row value with three cells.
    pub fn row(&self, t: &TreeStructs, cells: [bool; 3]) -> Value {
        let layout = self.w.space.classes.struct_layout(t.row).unwrap();
        let mut v = match layout.zero() {
            Value::Struct(s) => StructValue::clone(s),
            _ => panic!("no zero value"),
        };
        for (i, c) in cells.iter().enumerate() {
            v.fields[i] = Value::Bool(*c);
        }
        Value::Struct(std::rc::Rc::new(v))
    }
}
