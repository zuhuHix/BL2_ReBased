//! A synthetic attribute world for the evaluator tests (AGENTS.md rule 9: invented objects and numbers only). The
//! class and field names are the packages' names (names of game objects, which the docs may hold), so the
//! evaluator reads the same shapes it reads from reflection; the values are made up, except the record constants the
//! tests name (the level curve and the health base, REALGAME_GROUND_TRUTH.md).
//!
//! The context resolvers here are **test resolvers**, registered only in this world's system: NoContextNeeded's
//! record rule (the source, or the globals for None) and two invented classes, `Test.TargetContextResolver` (the
//! object in its `Target` field: "a fixed test context", the lane entry's wording) and `Test.HopContextResolver` (the
//! object property `Field` of the incoming source). The real context resolvers are W3-ATTRCTX's.

use ow_core::NotYet;
use ow_core::rand::{CrtRand, CrtStream, Rand15};
use ow_defs::DefStore;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, ObjRef, PropAddr, PropId, StructId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;

use crate::api::{
    AttrError, AttrWorld, Comparison, ContextResolver, EngineQuery, EvalCx, GlobalSlots, InitData,
    NumForms, Operand2Usage, WorldParts,
};
use crate::eval::{name, object, property, required};
use crate::registry::AttributeSystem;

pub(crate) const NO_CONTEXT: &str = "GearboxFramework.NoContextNeededAttributeContextResolver";

/// The world: a hand-filled space, an optional store, the globals object, an optional generator, the slots.
pub(crate) struct World {
    pub space: ObjectSpace,
    pub defs: Option<DefStore>,
    pub globals: ObjRef,
    pub rng: Option<CrtRand>,
    pub slots: GlobalSlots,
    pub streams: Vec<CrtStream>,
    /// Answers to `EngineQuery::PlayerSkill` by skill definition; `None` here: the query is not modelled (the
    /// trait's default `LayerAbsent`).
    pub skills: Option<Vec<(ObjRef, Option<ObjRef>)>>,
    /// Answers to `EngineQuery::LocalPlayerPool` by resource; `None` here: the query is not modelled.
    pub local_pools: Option<Vec<(ObjRef, Option<ObjRef>)>>,
    /// How many change notifications [`AttrWorld::attribute_changed`] received.
    pub notified: u32,
}

impl AttrWorld for World {
    fn parts(&mut self) -> WorldParts<'_> {
        WorldParts::new(&mut self.space, self.defs.as_mut())
    }
    fn globals(&mut self) -> Result<ObjRef, AttrError> {
        Ok(self.globals)
    }
    fn game_info(&mut self) -> Result<Option<ObjRef>, AttrError> {
        Ok(None)
    }
    fn engine_query(
        &mut self,
        q: EngineQuery,
        source: ObjRef,
    ) -> Result<Option<ObjRef>, AttrError> {
        match (q, self.skills.as_ref()) {
            (EngineQuery::PlayerSkill { definition }, Some(skills)) => Ok(skills
                .iter()
                .find(|(d, _)| *d == definition)
                .and_then(|(_, s)| *s)),
            (EngineQuery::LocalPlayerPool { resource }, _) if self.local_pools.is_some() => Ok(self
                .local_pools
                .iter()
                .flatten()
                .find(|(r, _)| *r == resource)
                .and_then(|(_, p)| *p)),
            _ => Err(NotYet::layer_absent(
                format!("engine query {q:?} from {source:?}"),
                "ow-world",
                "test world",
            )
            .into()),
        }
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
        self.notified = self.notified.saturating_add(1);
        Ok(())
    }
    fn rng(&mut self, s: CrtStream) -> Result<&mut dyn Rand15, AttrError> {
        self.streams.push(s);
        match self.rng.as_mut() {
            Some(r) => Ok(r),
            None => Err(NotYet::layer_absent("a random source", "ow-world", "test world").into()),
        }
    }
}

/// NoContextNeeded's rule (NATIVE_ATTRIBUTES.md section 5), as a test resolver.
struct NoContextNeeded;

impl ContextResolver for NoContextNeeded {
    fn resolve(
        &self,
        _sys: &AttributeSystem,
        _resolver: ObjRef,
        _attr: ObjRef,
        source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<ObjRef>, AttrError> {
        match source {
            Some(s) => Ok(Some(s)),
            None => cx.world.globals().map(Some),
        }
    }
}

/// The resolver's `Target` field, whatever the source.
struct Target;

impl ContextResolver for Target {
    fn resolve(
        &self,
        _sys: &AttributeSystem,
        resolver: ObjRef,
        _attr: ObjRef,
        _source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<ObjRef>, AttrError> {
        object("Target", &required(cx, resolver, "Target")?)
    }
}

/// The incoming source's object property named by the resolver's `Field`; None for a None source or a missing one.
struct Hop;

impl ContextResolver for Hop {
    fn resolve(
        &self,
        _sys: &AttributeSystem,
        resolver: ObjRef,
        _attr: ObjRef,
        source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<ObjRef>, AttrError> {
        let field = name("Field", &required(cx, resolver, "Field")?)?;
        let Some(source) = source else {
            return Ok(None);
        };
        let text = cx.world.parts().space.names.text(field).into_owned();
        match property(cx, source, &text)? {
            Some(v) => object("hop", &v),
            None => Ok(None),
        }
    }
}

/// The system the tests use: this lane's value resolvers plus the test context resolvers. Built from empty so it
/// does not depend on which context resolvers `standard()` holds once W3-ATTRCTX lands.
pub(crate) fn system() -> AttributeSystem {
    let sys = super::register(AttributeSystem::new_empty()).unwrap();
    sys.with_context_resolver(NO_CONTEXT, NoContextNeeded)
        .unwrap()
        .with_context_resolver("Test.TargetContextResolver", Target)
        .unwrap()
        .with_context_resolver("Test.HopContextResolver", Hop)
        .unwrap()
}

pub(crate) struct Classes {
    pub object: ClassId,
    pub aid: StructId,
    pub expr: StructId,
    pub formula: StructId,
    pub entry: StructId,
    pub conditional: StructId,
    pub variance: StructId,
    pub range: StructId,
    pub maker_sel: StructId,
    pub type_sel: StructId,
    pub def_data: StructId,
    pub attribute: ClassId,
    pub multi: ClassId,
    pub definition: ClassId,
    pub item: ClassId,
    pub weapon: ClassId,
    pub pri: ClassId,
    pub controller: ClassId,
    pub maker: ClassId,
    pub weapon_type: ClassId,
    pub designer: ClassId,
    pub skill: ClassId,
    pub unlinked: ClassId,
    pub unread: ClassId,
}

/// The fixture: the world and its classes, with builders.
pub(crate) struct Fx {
    pub w: World,
    pub c: Classes,
}

fn obj_kind(c: ClassId) -> PropKind {
    PropKind::Object { class: c }
}

impl Fx {
    pub fn new() -> Fx {
        let mut space = ObjectSpace::new();
        let object = space.classes.define("Core.Object", None, vec![]).unwrap();
        let d = |space: &mut ObjectSpace, n: &str, k: PropKind| {
            PropertyDecl::new(space.names.intern(n), k)
        };
        let byte = || PropKind::Byte { enum_: None };
        let objs = || PropKind::Array(Box::new(obj_kind(object)));

        let fields = vec![
            d(&mut space, "BaseValueConstant", PropKind::Float),
            d(&mut space, "BaseValueAttribute", obj_kind(object)),
            d(&mut space, "InitializationDefinition", obj_kind(object)),
            d(&mut space, "BaseValueScaleConstant", PropKind::Float),
        ];
        let aid = space
            .classes
            .define_struct(
                "Engine.AttributeInitializationDefinition.AttributeInitializationData",
                None,
                fields,
            )
            .unwrap();
        let fields = vec![
            d(&mut space, "AttributeOperand1", obj_kind(object)),
            d(&mut space, "ComparisonOperator", byte()),
            d(&mut space, "Operand2Usage", byte()),
            d(&mut space, "AttributeOperand2", obj_kind(object)),
            d(&mut space, "ConstantOperand2", PropKind::Float),
        ];
        let expr = space
            .classes
            .define_struct(
                "Engine.AttributeExpression.AttributeExpressionData",
                None,
                fields,
            )
            .unwrap();
        let mut fields = vec![d(&mut space, "bEnabled", PropKind::Bool)];
        for t in ["Multiplier", "Level", "Power", "Offset"] {
            fields.push(d(&mut space, t, PropKind::Struct(aid)));
        }
        let formula = space
            .classes
            .define_struct(
                "Engine.AttributeInitializationDefinition.BalanceFormula",
                None,
                fields,
            )
            .unwrap();
        let fields = vec![
            d(
                &mut space,
                "Expressions",
                PropKind::Array(Box::new(PropKind::Struct(expr))),
            ),
            d(&mut space, "BaseValueIfTrue", PropKind::Struct(aid)),
        ];
        let entry = space
            .classes
            .define_struct(
                "Engine.AttributeInitializationDefinition.ConditionalValueExpression",
                None,
                fields,
            )
            .unwrap();
        let fields = vec![
            d(&mut space, "bEnabled", PropKind::Bool),
            d(
                &mut space,
                "ConditionalExpressionList",
                PropKind::Array(Box::new(PropKind::Struct(entry))),
            ),
            d(&mut space, "DefaultBaseValue", PropKind::Struct(aid)),
        ];
        let conditional = space
            .classes
            .define_struct(
                "Engine.AttributeInitializationDefinition.ConditionalInitializationExpressions",
                None,
                fields,
            )
            .unwrap();
        let fields = vec![
            d(&mut space, "bEnabled", PropKind::Bool),
            d(&mut space, "LowerBound", PropKind::Struct(aid)),
            d(&mut space, "UpperBound", PropKind::Struct(aid)),
            d(&mut space, "bUseIntegerRandomization", PropKind::Bool),
        ];
        let variance = space
            .classes
            .define_struct(
                "Engine.AttributeInitializationDefinition.Variance",
                None,
                fields,
            )
            .unwrap();
        let fields = vec![
            d(&mut space, "bEnableMinValueRestriction", PropKind::Bool),
            d(&mut space, "MinValue", PropKind::Struct(aid)),
            d(&mut space, "bEnableMaxValueRestriction", PropKind::Bool),
            d(&mut space, "MaxValue", PropKind::Struct(aid)),
        ];
        let range = space
            .classes
            .define_struct(
                "Engine.AttributeInitializationDefinition.Range",
                None,
                fields,
            )
            .unwrap();
        let fields = vec![
            d(&mut space, "AssociatedManufacturerName", PropKind::Name),
            d(&mut space, "ValueIfMatched", PropKind::Struct(aid)),
        ];
        let maker_sel = space
            .classes
            .define_struct(
                "WillowGame.ManufacturerAttributeValueResolver.ManufacturerSelectorData",
                None,
                fields,
            )
            .unwrap();
        let fields = vec![
            d(&mut space, "WeaponType", byte()),
            d(&mut space, "ValueIfMatched", PropKind::Struct(aid)),
        ];
        let type_sel = space
            .classes
            .define_struct(
                "WillowGame.WeaponTypeAttributeValueResolver.WeaponTypeSelectorData",
                None,
                fields,
            )
            .unwrap();
        let fields = vec![
            d(&mut space, "WeaponTypeDefinition", obj_kind(object)),
            d(&mut space, "ManufacturerDefinition", obj_kind(object)),
        ];
        let def_data = space
            .classes
            .define_struct(
                "WillowGame.WillowWeaponTypes.WeaponDefinitionData",
                None,
                fields,
            )
            .unwrap();

        let class =
            |space: &mut ObjectSpace, path: &str, sup: ClassId, props: Vec<(&str, PropKind)>| {
                let decls = props.into_iter().map(|(n, k)| d(space, n, k)).collect();
                space.classes.define(path, Some(sup), decls).unwrap()
            };
        let base = class(
            &mut space,
            "Engine.AttributeDefinitionBase",
            object,
            vec![
                ("ContextResolverChain", objs()),
                ("ValueResolverChain", objs()),
            ],
        );
        let attribute = class(&mut space, "Engine.AttributeDefinition", base, vec![]);
        let multi = class(
            &mut space,
            "Engine.AttributeDefinitionMultiContext",
            base,
            vec![],
        );
        let definition = class(
            &mut space,
            "Engine.AttributeInitializationDefinition",
            object,
            vec![
                ("ValueFormula", PropKind::Struct(formula)),
                ("ConditionalInitialization", PropKind::Struct(conditional)),
                ("BaseValueMode", byte()),
                ("RandomVariance", PropKind::Struct(variance)),
                ("RangeRestriction", PropKind::Struct(range)),
                ("RoundingMode", byte()),
            ],
        );
        let resolvers: Vec<(&str, Vec<(&str, PropKind)>)> = vec![
            (
                "Engine.ObjectPropertyAttributeValueResolver",
                vec![("PropertyName", PropKind::Name)],
            ),
            (
                "GearboxFramework.ConstantAttributeValueResolver",
                vec![("ConstantValue", PropKind::Float)],
            ),
            (
                "GearboxFramework.SimpleMathValueResolver",
                vec![
                    ("Operand", byte()),
                    ("Arg1Option", byte()),
                    ("Argument", PropKind::Struct(aid)),
                    ("Arg1Attribute", PropKind::Struct(aid)),
                ],
            ),
            (
                "GearboxFramework.ConditionalAttributeValueResolver",
                vec![("ValueExpressions", PropKind::Struct(conditional))],
            ),
            (
                "WillowGame.GlobalAttributeValueResolver",
                vec![("GlobalAttribute", byte())],
            ),
            (
                "WillowGame.PlayThroughCountAttributeValueResolver",
                vec![("IncludePlaythroughThree", PropKind::Int)],
            ),
            (
                "WillowGame.TargetableAttributeValueResolver",
                vec![("ValueToResolve", byte())],
            ),
            (
                "GearboxFramework.ConstraintAttributeValueResolver",
                vec![("Constraints", PropKind::Struct(range))],
            ),
            (
                "WillowGame.CurrencyAttributeValueResolver",
                vec![("FormOfCurrency", byte())],
            ),
            (
                "GearboxFramework.RandomAttributeValueResolver",
                vec![
                    ("LowerBound", PropKind::Struct(aid)),
                    ("UpperBound", PropKind::Struct(aid)),
                    ("bUseIntegerRandomization", PropKind::Bool),
                ],
            ),
            (
                "WillowGame.AmmoDropWeightAttributeValueResolver",
                vec![
                    ("Resource", obj_kind(object)),
                    ("ResourceThreshold", PropKind::Struct(aid)),
                    ("AboveThresholdWeight", PropKind::Struct(aid)),
                    ("MinBelowThresholdWeight", PropKind::Struct(aid)),
                    ("MaxBelowThresholdWeight", PropKind::Struct(aid)),
                    ("NoPoolWeight", PropKind::Struct(aid)),
                ],
            ),
            (
                "WillowGame.PlayerSkillAttributeValueResolver",
                vec![("AssociatedSkill", obj_kind(object))],
            ),
            (
                "WillowGame.ResourcePoolStateAttributeValueResolver",
                vec![("PoolState", byte())],
            ),
            (
                "WillowGame.AttributeSlotEffectAttributeValueResolver",
                vec![("SlotName", PropKind::Name), ("SlotProperty", byte())],
            ),
            (
                "WillowGame.BadassAttributeValueResolver",
                vec![("AssociatedBadassReward", obj_kind(object))],
            ),
            (
                "WillowGame.ManufacturerAttributeValueResolver",
                vec![
                    (
                        "Manufacturers",
                        PropKind::Array(Box::new(PropKind::Struct(maker_sel))),
                    ),
                    ("ValueIfNotMatched", PropKind::Struct(aid)),
                ],
            ),
            (
                "WillowGame.WeaponTypeAttributeValueResolver",
                vec![
                    (
                        "WeaponTypes",
                        PropKind::Array(Box::new(PropKind::Struct(type_sel))),
                    ),
                    ("ValueIfNotMatched", PropKind::Struct(aid)),
                ],
            ),
            (
                "GearboxFramework.NounAttributeValueResolver",
                vec![
                    ("DefaultValue", PropKind::Struct(aid)),
                    ("ReplicationStrategy", byte()),
                ],
            ),
            (NO_CONTEXT, vec![]),
            (
                "Test.TargetContextResolver",
                vec![("Target", obj_kind(object))],
            ),
            ("Test.HopContextResolver", vec![("Field", PropKind::Name)]),
        ];
        for (path, props) in resolvers {
            class(&mut space, path, object, props);
        }
        let unread = class(&mut space, "Test.UnreadValueResolver", object, vec![]);

        // Game-side classes: an item with a plain int attribute, a weapon below it with a stack attribute and the
        // definition data, a replication info, a controller holding it, a manufacturer and a type definition.
        let plain = |space: &mut ObjectSpace, n: &str, k: PropKind| {
            PropertyDecl::new(space.names.intern(n), k).with_flags(PropertyDecl::PLAIN_ATTRIBUTE)
        };
        let decls = vec![plain(&mut space, "ExpLevel", PropKind::IntAttribute)];
        let item = space
            .classes
            .define("Test.WillowItem", Some(object), decls)
            .unwrap();
        let decls = vec![
            PropertyDecl::new(
                space.names.intern("DefinitionData"),
                PropKind::Struct(def_data),
            ),
            PropertyDecl::new(
                space.names.intern("InstantHitDamage"),
                PropKind::FloatAttribute,
            )
            .with_flags(PropertyDecl::STACK_ATTRIBUTE),
            plain(
                &mut space,
                "InstantHitDamageBaseValue",
                PropKind::FloatAttribute,
            ),
            PropertyDecl::new(space.names.intern("InstantHitDamageModifierStack"), objs()),
            PropertyDecl::new(space.names.intern("Unflagged"), PropKind::Float),
            plain(
                &mut space,
                "ClipSize",
                PropKind::ByteAttribute { enum_: None },
            ),
        ];
        let weapon = space
            .classes
            .define("Test.WillowWeapon", Some(item), decls)
            .unwrap();
        let (v, b, s) = (
            space.names.intern("InstantHitDamage"),
            space.names.intern("InstantHitDamageBaseValue"),
            space.names.intern("InstantHitDamageModifierStack"),
        );
        space.classes.link_attribute(weapon, v, b, s).unwrap();
        let decls = vec![plain(&mut space, "ExpLevel", PropKind::IntAttribute)];
        let pri = space
            .classes
            .define("Test.PlayerReplicationInfo", Some(object), decls)
            .unwrap();
        let decls = vec![PropertyDecl::new(
            space.names.intern("PlayerReplicationInfo"),
            obj_kind(object),
        )];
        let controller = space
            .classes
            .define("Test.PlayerController", Some(object), decls)
            .unwrap();
        let maker = space
            .classes
            .define("Test.ManufacturerDefinition", Some(object), vec![])
            .unwrap();
        let decls = vec![PropertyDecl::new(space.names.intern("WeaponType"), byte())];
        let weapon_type = space
            .classes
            .define("Test.WeaponTypeDefinition", Some(object), decls)
            .unwrap();
        let decls = vec![plain(&mut space, "Grade", PropKind::IntAttribute)];
        let skill = space
            .classes
            .define("Test.Skill", Some(object), decls)
            .unwrap();
        // A stack-flagged attribute whose companions were never linked (the "Modifier stack not found" case).
        let decls = vec![
            PropertyDecl::new(space.names.intern("Loose"), PropKind::FloatAttribute)
                .with_flags(PropertyDecl::STACK_ATTRIBUTE),
        ];
        let unlinked = space
            .classes
            .define("Test.Unlinked", Some(object), decls)
            .unwrap();
        let decls = vec![plain(&mut space, "Value", PropKind::FloatAttribute)];
        let designer = space
            .classes
            .define("Test.InstancedDesignerAttribute", Some(object), decls)
            .unwrap();

        let globals = space
            .alloc(object, "Globals", None, Origin::Runtime)
            .unwrap();
        let w = World {
            space,
            defs: None,
            globals,
            rng: Some(CrtRand::new(1)),
            slots: GlobalSlots::default(),
            streams: Vec::new(),
            skills: None,
            local_pools: None,
            notified: 0,
        };
        let c = Classes {
            object,
            aid,
            expr,
            formula,
            entry,
            conditional,
            variance,
            range,
            maker_sel,
            type_sel,
            def_data,
            attribute,
            multi,
            definition,
            item,
            weapon,
            pri,
            controller,
            maker,
            weapon_type,
            designer,
            skill,
            unlinked,
            unread,
        };
        Fx { w, c }
    }

    pub fn name(&mut self, s: &str) -> Value {
        Value::Name(self.w.space.names.intern(s))
    }

    /// A struct value of `ty` from its zero value with `fields` set by name.
    pub fn st(&mut self, ty: StructId, fields: Vec<(&str, Value)>) -> Value {
        let mut v = self.w.space.classes.struct_defaults(ty).unwrap();
        for (n, x) in fields {
            let id = self.w.space.names.find(n).unwrap();
            let entry = self
                .w
                .space
                .classes
                .struct_layout(ty)
                .unwrap()
                .field(id)
                .unwrap()
                .0;
            v.struct_mut().unwrap().fields[entry as usize] = x;
        }
        v
    }

    /// An object of `class` named `name` with `props` written by name.
    pub fn obj(&mut self, class: ClassId, name: &str, props: Vec<(&str, Value)>) -> ObjRef {
        let o = self
            .w
            .space
            .alloc(class, name, None, Origin::Runtime)
            .unwrap();
        for (n, v) in props {
            let p = self
                .w
                .space
                .prop(class, n)
                .unwrap_or_else(|| panic!("no property {n}"));
            self.w.space.write(o, PropAddr::scalar(p), v).unwrap();
        }
        o
    }

    pub fn by_path(&self, path: &str) -> ClassId {
        self.w.space.classes.by_path(path).unwrap()
    }

    /// `AttributeInitializationData` as a struct value.
    pub fn data(&mut self, d: InitData) -> Value {
        let o = |r: Option<ObjRef>| Value::Object(r);
        self.st(
            self.c.aid,
            vec![
                ("BaseValueConstant", Value::Float(d.base_value_constant)),
                ("BaseValueAttribute", o(d.base_value_attribute)),
                ("InitializationDefinition", o(d.initialization_definition)),
                (
                    "BaseValueScaleConstant",
                    Value::Float(d.base_value_scale_constant),
                ),
            ],
        )
    }

    pub fn k(&mut self, v: f32) -> Value {
        self.data(InitData::constant(v))
    }

    pub fn of_attr(&mut self, attr: ObjRef) -> Value {
        self.data(InitData {
            base_value_attribute: Some(attr),
            ..InitData::constant(0.0)
        })
    }

    pub fn of_def(&mut self, def: ObjRef) -> Value {
        self.data(InitData {
            initialization_definition: Some(def),
            ..InitData::constant(0.0)
        })
    }

    pub fn attr_in(
        &mut self,
        class: ClassId,
        name: &str,
        contexts: Vec<ObjRef>,
        values: Vec<ObjRef>,
    ) -> ObjRef {
        let objs =
            |v: Vec<ObjRef>| Value::array(v.into_iter().map(|o| Value::Object(Some(o))).collect());
        self.obj(
            class,
            name,
            vec![
                ("ContextResolverChain", objs(contexts)),
                ("ValueResolverChain", objs(values)),
            ],
        )
    }

    pub fn attr(&mut self, name: &str, contexts: Vec<ObjRef>, values: Vec<ObjRef>) -> ObjRef {
        self.attr_in(self.c.attribute, name, contexts, values)
    }

    pub fn no_context(&mut self) -> ObjRef {
        let c = self.by_path(NO_CONTEXT);
        self.obj(c, "ContextResolver", vec![])
    }

    pub fn target(&mut self, t: Option<ObjRef>) -> ObjRef {
        let c = self.by_path("Test.TargetContextResolver");
        self.obj(c, "Target", vec![("Target", Value::Object(t))])
    }

    pub fn hop(&mut self, field: &str) -> ObjRef {
        let c = self.by_path("Test.HopContextResolver");
        let f = self.name(field);
        self.obj(c, "Hop", vec![("Field", f)])
    }

    pub fn constant(&mut self, v: f32) -> ObjRef {
        let c = self.by_path("GearboxFramework.ConstantAttributeValueResolver");
        self.obj(c, "Constant", vec![("ConstantValue", Value::Float(v))])
    }

    pub fn global(&mut self, slot: u8) -> ObjRef {
        let c = self.by_path("WillowGame.GlobalAttributeValueResolver");
        self.obj(c, "Global", vec![("GlobalAttribute", Value::Byte(slot))])
    }

    pub fn property_resolver(&mut self, property: &str) -> ObjRef {
        let c = self.by_path("Engine.ObjectPropertyAttributeValueResolver");
        let n = self.name(property);
        self.obj(c, "Property", vec![("PropertyName", n)])
    }

    pub fn math(&mut self, operand: u8, argument: Value, arg1: Option<Value>) -> ObjRef {
        let c = self.by_path("GearboxFramework.SimpleMathValueResolver");
        let mut props = vec![("Operand", Value::Byte(operand)), ("Argument", argument)];
        match arg1 {
            Some(a) => props.extend([("Arg1Option", Value::Byte(1)), ("Arg1Attribute", a)]),
            None => props.push(("Arg1Option", Value::Byte(0))),
        }
        self.obj(c, "Math", props)
    }

    /// A constant attribute (NoContextNeeded, then a constant resolver).
    pub fn constant_attr(&mut self, name: &str, v: f32) -> ObjRef {
        let (c, r) = (self.no_context(), self.constant(v));
        self.attr(name, vec![c], vec![r])
    }

    /// An attribute read from the property `property` of the fixed context `target`.
    pub fn property_attr(&mut self, name: &str, target: ObjRef, property: &str) -> ObjRef {
        let (c, r) = (self.target(Some(target)), self.property_resolver(property));
        self.attr(name, vec![c], vec![r])
    }

    /// An `AttributeInitializationDefinition` with every switch off, then `props` set.
    pub fn definition(&mut self, props: Vec<(&str, Value)>) -> ObjRef {
        self.obj(self.c.definition, "Definition", props)
    }

    pub fn formula(&mut self, m: Value, l: Value, p: Value, o: Value) -> Value {
        let ty = self.c.formula;
        self.st(
            ty,
            vec![
                ("bEnabled", Value::Bool(true)),
                ("Multiplier", m),
                ("Level", l),
                ("Power", p),
                ("Offset", o),
            ],
        )
    }

    pub fn expr(
        &mut self,
        op1: Option<ObjRef>,
        cmp: Comparison,
        usage: Operand2Usage,
        op2: Option<ObjRef>,
        k: f32,
    ) -> Value {
        let ty = self.c.expr;
        self.st(
            ty,
            vec![
                ("AttributeOperand1", Value::Object(op1)),
                ("ComparisonOperator", Value::Byte(cmp as u8)),
                ("Operand2Usage", Value::Byte(usage as u8)),
                ("AttributeOperand2", Value::Object(op2)),
                ("ConstantOperand2", Value::Float(k)),
            ],
        )
    }

    /// `ConditionalInitializationExpressions`: `(expressions, value if true)` entries and the default.
    pub fn conditional(
        &mut self,
        enabled: bool,
        entries: Vec<(Vec<Value>, Value)>,
        default: Value,
    ) -> Value {
        let mut list = Vec::new();
        for (exprs, if_true) in entries {
            let ty = self.c.entry;
            list.push(self.st(
                ty,
                vec![
                    ("Expressions", Value::array(exprs)),
                    ("BaseValueIfTrue", if_true),
                ],
            ));
        }
        let ty = self.c.conditional;
        self.st(
            ty,
            vec![
                ("bEnabled", Value::Bool(enabled)),
                ("ConditionalExpressionList", Value::array(list)),
                ("DefaultBaseValue", default),
            ],
        )
    }

    pub fn variance(&mut self, low: f32, high: f32, integer: bool) -> Value {
        let (l, h) = (self.k(low), self.k(high));
        let ty = self.c.variance;
        self.st(
            ty,
            vec![
                ("bEnabled", Value::Bool(true)),
                ("LowerBound", l),
                ("UpperBound", h),
                ("bUseIntegerRandomization", Value::Bool(integer)),
            ],
        )
    }

    pub fn range(&mut self, min: Option<f32>, max: Option<f32>) -> Value {
        let (lo, hi) = (self.k(min.unwrap_or(0.0)), self.k(max.unwrap_or(0.0)));
        let ty = self.c.range;
        self.st(
            ty,
            vec![
                ("bEnableMinValueRestriction", Value::Bool(min.is_some())),
                ("MinValue", lo),
                ("bEnableMaxValueRestriction", Value::Bool(max.is_some())),
                ("MaxValue", hi),
            ],
        )
    }

    /// A weapon with the given manufacturer and type definitions.
    pub fn weapon(&mut self, maker: Option<ObjRef>, weapon_type: Option<ObjRef>) -> ObjRef {
        let ty = self.c.def_data;
        let data = self.st(
            ty,
            vec![
                ("ManufacturerDefinition", Value::Object(maker)),
                ("WeaponTypeDefinition", Value::Object(weapon_type)),
            ],
        );
        self.obj(self.c.weapon, "Weapon", vec![("DefinitionData", data)])
    }
}
