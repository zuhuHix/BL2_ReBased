//! Owner: lane W3-BALANCE. A synthetic balance world for this crate's tests (AGENTS.md rule 9: invented objects and
//! numbers only). The class and field names are the names the records give (names of game objects, which the docs
//! may hold), so the rules read the shapes they read from reflection; classes the records name no package for are
//! under the invented package `Test`. The numbers are made up, except the record constants a test names (the level
//! curve, the health base, the Sanctuary bounds, the kill tables of NATIVE_COMBAT_GROUNDWORK.md 5.2).
//!
//! The objects are built directly in the space (`Origin::Runtime`): the `DefStore` body is not on main yet (every
//! lookup answers `NotYet`), so no test here loads a synthetic package.

use ow_attr::AttributeSystem;
use ow_attr::api::{Comparison, InitData, Operand2Usage};
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, ObjRef, PropAddr, StructId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;

use super::OfflineWorld;
use crate::types::{
    AwesomeLevel, BalanceSnapshot, ExpLevel, GameStage, LevelCapUpgrades, OpLevel, PlayerCount,
    Playthrough,
};

const NO_CONTEXT: &str = "GearboxFramework.NoContextNeededAttributeContextResolver";
const PRI_CONTEXT: &str = "WillowGame.PlayerReplicationInfoAttributeContextResolver";
const PAWN_CONTEXT: &str = "Engine.PawnAttributeContextResolver";
const CONSTANT: &str = "GearboxFramework.ConstantAttributeValueResolver";
const PROPERTY: &str = "Engine.ObjectPropertyAttributeValueResolver";
const GLOBAL: &str = "WillowGame.GlobalAttributeValueResolver";
const PLAYTHROUGH_COUNT: &str = "WillowGame.PlayThroughCountAttributeValueResolver";

/// The default snapshot: first playthrough, one player, level 8, stage 8, no level-cap pack.
pub(crate) fn snapshot() -> BalanceSnapshot {
    BalanceSnapshot::new(
        Playthrough::NORMAL,
        OpLevel::NONE,
        PlayerCount::SINGLE,
        ExpLevel::new(8),
        GameStage(8),
        AwesomeLevel::new(0),
        LevelCapUpgrades::NONE,
    )
    .unwrap()
}

/// A region table built by [`Fx::region_table`].
#[derive(Debug, Clone)]
pub(crate) struct RegionData {
    pub globals: ObjRef,
    pub region: ObjRef,
    pub missions: Vec<ObjRef>,
}

struct Structs {
    aid: StructId,
    formula: StructId,
    expr: StructId,
    entry: StructId,
    conditional: StructId,
    range: StructId,
    playthrough_data: StructId,
    region_balance: StructId,
    adjustment: StructId,
    weights: StructId,
    level_scale: StructId,
    damage_type: StructId,
}

/// The fixture: a space with the classes, and builders.
pub(crate) struct Fx {
    pub space: ObjectSpace,
    s: Structs,
    attribute: ClassId,
    definition: ClassId,
    globals_definition: ClassId,
    region_balance_definition: ClassId,
    region: ClassId,
    mission: ClassId,
    enemy: ClassId,
    vehicle: ClassId,
}

impl Fx {
    pub fn new() -> Fx {
        let mut space = ObjectSpace::new();
        let object = space.classes.define("Core.Object", None, vec![]).unwrap();
        let obj = |c: ClassId| PropKind::Object { class: c };
        let byte = || PropKind::Byte { enum_: None };
        let objs = move || PropKind::Array(Box::new(obj(object)));
        let d = |space: &mut ObjectSpace, n: &str, k: PropKind| {
            PropertyDecl::new(space.names.intern(n), k)
        };
        let plain = |space: &mut ObjectSpace, n: &str, k: PropKind| {
            PropertyDecl::new(space.names.intern(n), k).with_flags(PropertyDecl::PLAIN_ATTRIBUTE)
        };
        let st = |space: &mut ObjectSpace, path: &str, fields: Vec<(&str, PropKind)>| {
            let decls = fields.into_iter().map(|(n, k)| d(space, n, k)).collect();
            space.classes.define_struct(path, None, decls).unwrap()
        };
        let class =
            |space: &mut ObjectSpace, path: &str, sup: ClassId, props: Vec<PropertyDecl>| {
                space.classes.define(path, Some(sup), props).unwrap()
            };

        // The attribute data structs, as ow-attr's evaluator reads them.
        let aid = st(
            &mut space,
            "Engine.AttributeInitializationDefinition.AttributeInitializationData",
            vec![
                ("BaseValueConstant", PropKind::Float),
                ("BaseValueAttribute", obj(object)),
                ("InitializationDefinition", obj(object)),
                ("BaseValueScaleConstant", PropKind::Float),
            ],
        );
        let expr = st(
            &mut space,
            "Engine.AttributeExpression.AttributeExpressionData",
            vec![
                ("AttributeOperand1", obj(object)),
                ("ComparisonOperator", byte()),
                ("Operand2Usage", byte()),
                ("AttributeOperand2", obj(object)),
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

        // The attribute classes and the resolvers the tests use.
        let decls = vec![
            d(&mut space, "ContextResolverChain", objs()),
            d(&mut space, "ValueResolverChain", objs()),
        ];
        let base = class(&mut space, "Engine.AttributeDefinitionBase", object, decls);
        let attribute = class(&mut space, "Engine.AttributeDefinition", base, vec![]);
        let decls = vec![
            d(&mut space, "ValueFormula", PropKind::Struct(formula)),
            d(
                &mut space,
                "ConditionalInitialization",
                PropKind::Struct(conditional),
            ),
            d(&mut space, "BaseValueMode", byte()),
            d(&mut space, "RandomVariance", PropKind::Struct(variance)),
            d(&mut space, "RangeRestriction", PropKind::Struct(range)),
            d(&mut space, "RoundingMode", byte()),
        ];
        let definition = class(
            &mut space,
            "Engine.AttributeInitializationDefinition",
            object,
            decls,
        );
        let decls = vec![d(&mut space, "ConstantValue", PropKind::Float)];
        class(&mut space, CONSTANT, object, decls);
        let decls = vec![d(&mut space, "PropertyName", PropKind::Name)];
        class(&mut space, PROPERTY, object, decls);
        let decls = vec![d(&mut space, "GlobalAttribute", byte())];
        class(&mut space, GLOBAL, object, decls);
        let decls = vec![d(&mut space, "IncludePlaythroughThree", PropKind::Int)];
        class(&mut space, PLAYTHROUGH_COUNT, object, decls);
        for path in [NO_CONTEXT, PRI_CONTEXT, PAWN_CONTEXT] {
            class(&mut space, path, object, vec![]);
        }

        // The actor classes the context resolvers test against (Engine.*) and the offline world's classes.
        let decls = vec![
            d(&mut space, "Instigator", obj(object)),
            d(&mut space, "Owner", obj(object)),
        ];
        let actor = class(&mut space, "Engine.Actor", object, decls);
        let decls = vec![
            d(&mut space, "Pawn", obj(object)),
            d(&mut space, "PlayerReplicationInfo", obj(object)),
        ];
        let controller = class(&mut space, "Engine.Controller", actor, decls);
        let player_controller = class(&mut space, "Engine.PlayerController", controller, vec![]);
        let decls = vec![plain(
            &mut space,
            "ExpScaleForCriticalHitKills",
            PropKind::FloatAttribute,
        )];
        class(
            &mut space,
            super::PLAYER_CONTROLLER_CLASS,
            player_controller,
            decls,
        );
        let decls = vec![
            d(&mut space, "Controller", obj(object)),
            d(&mut space, "DrivenVehicle", obj(object)),
            d(&mut space, "PlayerReplicationInfo", obj(object)),
        ];
        let pawn = class(&mut space, "Engine.Pawn", actor, decls);
        let vehicle = class(&mut space, "Engine.Vehicle", pawn, vec![]);
        let decls = vec![
            plain(&mut space, "ExpLevel", PropKind::IntAttribute),
            plain(
                &mut space,
                "ExperiencePointMultiplier",
                PropKind::FloatAttribute,
            ),
        ];
        let enemy = class(&mut space, "Test.EnemyPawn", pawn, decls);
        let info = class(&mut space, "Engine.ReplicationInfo", actor, vec![]);
        let pri = class(&mut space, "Engine.PlayerReplicationInfo", info, vec![]);
        let decls = vec![plain(&mut space, "ExpLevel", PropKind::IntAttribute)];
        class(&mut space, super::REPLICATION_INFO_CLASS, pri, decls);
        let gri = class(&mut space, "Engine.GameReplicationInfo", info, vec![]);
        let decls = vec![d(&mut space, "CurrentPlaythrough", byte())];
        class(&mut space, super::GAME_REPLICATION_INFO_CLASS, gri, decls);
        class(&mut space, super::GLOBALS_CLASS, object, vec![]);

        // The globals definition and the region tables (invented package Test; field names from the records and
        // tools/slice_values.py).
        let mission = class(&mut space, "Test.MissionDefinition", object, vec![]);
        let decls = vec![d(&mut space, "DlcExpansion", obj(object))];
        let region = class(&mut space, "Test.RegionDefinition", object, decls);
        let playthrough_data = st(
            &mut space,
            "Test.GlobalsDefinition.PlayThroughData",
            vec![
                ("PlayThroughNumber", PropKind::Int),
                ("BalanceDefinitions", objs()),
            ],
        );
        let adjustment = st(
            &mut space,
            "Test.RegionBalanceDefinition.MissionStateBalanceAdjustment",
            vec![
                ("Mission", obj(object)),
                ("MinGameStage", PropKind::Struct(aid)),
                ("MaxGameStage", PropKind::Struct(aid)),
            ],
        );
        let region_balance = st(
            &mut space,
            "Test.RegionBalanceDefinition.RegionBalanceData",
            vec![
                ("Region", obj(object)),
                ("bSpecifyBoostAbovePlayer", PropKind::Bool),
                ("GameStageIncreaseAbovePlayer", PropKind::Struct(aid)),
                ("MinDefaultGameStage", PropKind::Struct(aid)),
                ("MaxDefaultGameStage", PropKind::Struct(aid)),
                (
                    "MissionOverrides",
                    PropKind::Array(Box::new(PropKind::Struct(adjustment))),
                ),
            ],
        );
        let decls = vec![d(
            &mut space,
            "BalanceByRegion",
            PropKind::Array(Box::new(PropKind::Struct(region_balance))),
        )];
        let region_balance_definition =
            class(&mut space, "Test.RegionBalanceDefinition", object, decls);
        let weights = st(
            &mut space,
            "Test.GlobalsDefinition.ExpAwardWeight",
            vec![
                ("KillerExpBonus", PropKind::Float),
                ("ExpWeight", PropKind::Float),
            ],
        );
        let level_scale = st(
            &mut space,
            "Test.GlobalsDefinition.ExpScaleByLevelDifference",
            vec![
                ("HigherLevelEnemyExpScale", PropKind::Float),
                ("LowerLevelEnemyExpScale", PropKind::Float),
            ],
        );
        let damage_type = st(
            &mut space,
            "Test.GlobalsDefinition.DamageTypeExpModifier",
            vec![("Multiplier", PropKind::Float)],
        );
        let decls = vec![
            d(
                &mut space,
                "RegionBalanceData",
                PropKind::Array(Box::new(PropKind::Struct(playthrough_data))),
            ),
            d(
                &mut space,
                "GameStageIncreaseAbovePlayer",
                PropKind::Struct(aid),
            ),
            d(
                &mut space,
                "BaseEnemyExperienceFormula",
                PropKind::Struct(aid),
            ),
            d(
                &mut space,
                "ExpAwardWeights",
                PropKind::Array(Box::new(PropKind::Struct(weights))),
            ),
            d(
                &mut space,
                "ExpScaleByLevelDifference",
                PropKind::Array(Box::new(PropKind::Struct(level_scale))),
            ),
            d(
                &mut space,
                "DamageTypeExpModifiers",
                PropKind::Array(Box::new(PropKind::Struct(damage_type))),
            ),
            d(
                &mut space,
                "GeneralSkillPointsPerLevelUp",
                PropKind::Struct(aid),
            ),
            d(
                &mut space,
                "ExpPointsRequiredForLevel",
                PropKind::Struct(aid),
            ),
        ];
        let globals_definition = class(&mut space, "Test.GlobalsDefinition", object, decls);

        Fx {
            space,
            s: Structs {
                aid,
                formula,
                expr,
                entry,
                conditional,
                range,
                playthrough_data,
                region_balance,
                adjustment,
                weights,
                level_scale,
                damage_type,
            },
            attribute,
            definition,
            globals_definition,
            region_balance_definition,
            region,
            mission,
            enemy,
            vehicle,
        }
    }

    /// The standard resolver system and the offline world over the default snapshot.
    pub fn into_world(self) -> (AttributeSystem, OfflineWorld) {
        let sys = AttributeSystem::standard().unwrap();
        let world = OfflineWorld::new(self.space, None, snapshot()).unwrap();
        (sys, world)
    }

    /// The space alone, for tests that build the world themselves.
    pub fn space_only(self) -> ObjectSpace {
        self.space
    }

    // ---- values ----

    fn st(&mut self, ty: StructId, fields: Vec<(&str, Value)>) -> Value {
        let mut v = self.space.classes.struct_defaults(ty).unwrap();
        for (n, x) in fields {
            let id = self.space.names.find(n).unwrap();
            let entry = self
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

    fn obj(&mut self, class: ClassId, name: &str, props: Vec<(&str, Value)>) -> ObjRef {
        let o = self
            .space
            .alloc(class, name, None, Origin::Runtime)
            .unwrap();
        for (n, v) in props {
            let p = self
                .space
                .prop(class, n)
                .unwrap_or_else(|| panic!("no property {n}"));
            self.space.write(o, PropAddr::scalar(p), v).unwrap();
        }
        o
    }

    fn by_path(&self, path: &str) -> ClassId {
        self.space.classes.by_path(path).unwrap()
    }

    fn data(&mut self, d: InitData) -> Value {
        let o = |r: Option<ObjRef>| Value::Object(r);
        self.st(
            self.s.aid,
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

    fn k(&mut self, v: f32) -> Value {
        self.data(InitData::constant(v))
    }

    fn of_attr(&mut self, constant: f32, attr: ObjRef) -> Value {
        self.data(InitData {
            base_value_attribute: Some(attr),
            ..InitData::constant(constant)
        })
    }

    fn resolver(&mut self, path: &str, props: Vec<(&str, Value)>) -> ObjRef {
        let c = self.by_path(path);
        self.obj(c, "Resolver", props)
    }

    fn attr(&mut self, context: &str, value: ObjRef) -> ObjRef {
        let c = self.resolver(context, vec![]);
        let objs =
            |v: Vec<ObjRef>| Value::array(v.into_iter().map(|o| Value::Object(Some(o))).collect());
        let a = self.attribute;
        self.obj(
            a,
            "Attribute",
            vec![
                ("ContextResolverChain", objs(vec![c])),
                ("ValueResolverChain", objs(vec![value])),
            ],
        )
    }

    fn constant_attr(&mut self, v: f32) -> ObjRef {
        let r = self.resolver(CONSTANT, vec![("ConstantValue", Value::Float(v))]);
        self.attr(NO_CONTEXT, r)
    }

    /// A NoContextNeeded attribute whose one value resolver is a playthrough count with `IncludePlaythroughThree`.
    pub fn playthrough_count_attr(&mut self, include_three: i32) -> ObjRef {
        let r = self.resolver(
            PLAYTHROUGH_COUNT,
            vec![("IncludePlaythroughThree", Value::Int(include_three))],
        );
        self.attr(NO_CONTEXT, r)
    }

    fn property_attr(&mut self, context: &str, property: &str) -> ObjRef {
        let n = Value::Name(self.space.names.intern(property));
        let r = self.resolver(PROPERTY, vec![("PropertyName", n)]);
        self.attr(context, r)
    }

    fn formula(&mut self, m: Value, l: Value, p: Value, o: Value) -> Value {
        let ty = self.s.formula;
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

    fn definition(&mut self, props: Vec<(&str, Value)>) -> ObjRef {
        let c = self.definition;
        self.obj(c, "Definition", props)
    }

    /// Plain constant data.
    pub fn constant_data(&mut self, v: f32) -> InitData {
        InitData::constant(v)
    }

    /// Data whose base is the player's level (a PlayerReplicationInfo context, then `ExpLevel`), scaled by `scale`.
    pub fn player_level_data(&mut self, scale: f32) -> InitData {
        let level = self.property_attr(PRI_CONTEXT, "ExpLevel");
        InitData {
            base_value_attribute: Some(level),
            base_value_scale_constant: scale,
            ..InitData::constant(0.0)
        }
    }

    /// The level curve definition `60 x (L^2.8 + 7.33)`, the level from global slot 0 (NATIVE_PROGRESSION.md
    /// section 3, the record's constants).
    pub fn xp_curve(&mut self) -> ObjRef {
        let slot = self.resolver(GLOBAL, vec![("GlobalAttribute", Value::Byte(0))]);
        let level_attr = self.attr(NO_CONTEXT, slot);
        let (m, l, p, o) = (
            self.k(60.0),
            self.of_attr(0.0, level_attr),
            self.k(2.8),
            self.k(7.33),
        );
        let f = self.formula(m, l, p, o);
        self.definition(vec![("ValueFormula", f)])
    }

    /// A globals definition whose `ExpPointsRequiredForLevel` is `data` (the installed game holds the curve as
    /// `AttributeInitializationData { 0, None, the definition, 1 }`).
    pub fn curve_globals(&mut self, data: InitData) -> ObjRef {
        let v = self.data(data);
        let c = self.globals_definition;
        self.obj(c, "Globals", vec![("ExpPointsRequiredForLevel", v)])
    }

    /// `Init_PlayerHealth`'s recorded shape (NATIVE_PROGRESSION.md section 4): Multiplier {94, attribute 80}, Level
    /// {attribute 1.13}, Power {1, PlayerExperienceLevel}, no offset, minimum 20, no rounding.
    pub fn player_health_data(&mut self) -> InitData {
        let (mult, scaler) = (self.constant_attr(80.0), self.constant_attr(1.13));
        let level = self.property_attr(PRI_CONTEXT, "ExpLevel");
        let (m, l, p, o) = (
            self.of_attr(94.0, mult),
            self.of_attr(0.0, scaler),
            self.of_attr(1.0, level),
            self.k(0.0),
        );
        let f = self.formula(m, l, p, o);
        let (lo, hi) = (self.k(20.0), self.k(0.0));
        let ty = self.s.range;
        let range = self.st(
            ty,
            vec![
                ("bEnableMinValueRestriction", Value::Bool(true)),
                ("MinValue", lo),
                ("bEnableMaxValueRestriction", Value::Bool(false)),
                ("MaxValue", hi),
            ],
        );
        let def = self.definition(vec![("ValueFormula", f), ("RangeRestriction", range)]);
        InitData {
            initialization_definition: Some(def),
            ..InitData::constant(0.0)
        }
    }

    // ---- regions ----

    fn region_row(
        &mut self,
        region: ObjRef,
        min: i32,
        max: i32,
        overrides: Vec<Value>,
        specify: Option<f32>,
    ) -> Value {
        let (lo, hi) = (self.k(min as f32), self.k(max as f32));
        let boost = self.k(specify.unwrap_or(0.0));
        let ty = self.s.region_balance;
        self.st(
            ty,
            vec![
                ("Region", Value::Object(Some(region))),
                ("bSpecifyBoostAbovePlayer", Value::Bool(specify.is_some())),
                ("GameStageIncreaseAbovePlayer", boost),
                ("MinDefaultGameStage", lo),
                ("MaxDefaultGameStage", hi),
                ("MissionOverrides", Value::array(overrides)),
            ],
        )
    }

    fn table(&mut self, rows: Vec<Vec<ObjRef>>, boost: f32) -> ObjRef {
        let mut list = Vec::new();
        for (i, defs) in rows.into_iter().enumerate() {
            let ty = self.s.playthrough_data;
            let defs = Value::array(defs.into_iter().map(|o| Value::Object(Some(o))).collect());
            list.push(self.st(
                ty,
                vec![
                    ("PlayThroughNumber", Value::Int(i as i32 + 1)),
                    ("BalanceDefinitions", defs),
                ],
            ));
        }
        let boost = self.k(boost);
        let c = self.globals_definition;
        self.obj(
            c,
            "Globals",
            vec![
                ("RegionBalanceData", Value::array(list)),
                ("GameStageIncreaseAbovePlayer", boost),
            ],
        )
    }

    fn balance_definition(&mut self, rows: Vec<Value>) -> ObjRef {
        let c = self.region_balance_definition;
        self.obj(
            c,
            "RegionBalance",
            vec![("BalanceByRegion", Value::array(rows))],
        )
    }

    /// One region with default bounds `min..max` and mission overrides, the same entry for two playthrough rows, and
    /// a globals boost. An unrelated region comes first in the balance definition (the search goes on past it).
    pub fn region_table(
        &mut self,
        min: i32,
        max: i32,
        overrides: &[(i32, i32)],
        boost: i32,
    ) -> RegionData {
        let c = self.region;
        let region = self.obj(c, "Region", vec![]);
        let other = self.obj(c, "OtherRegion", vec![]);
        let mut missions = Vec::new();
        let mut list = Vec::new();
        for &(lo, hi) in overrides {
            let m = self.mission;
            let mission = self.obj(m, "Mission", vec![]);
            missions.push(mission);
            let (l, h) = (self.k(lo as f32), self.k(hi as f32));
            let ty = self.s.adjustment;
            list.push(self.st(
                ty,
                vec![
                    ("Mission", Value::Object(Some(mission))),
                    ("MinGameStage", l),
                    ("MaxGameStage", h),
                ],
            ));
        }
        let unrelated = self.region_row(other, 40, 45, vec![], None);
        let row = self.region_row(region, min, max, list, None);
        let first = self.balance_definition(vec![unrelated]);
        let second = self.balance_definition(vec![row]);
        let globals = self.table(vec![vec![first, second], vec![first, second]], boost as f32);
        RegionData {
            globals,
            region,
            missions,
        }
    }

    /// One region, one table row per `(min, max)`.
    pub fn region_table_rows(&mut self, rows: &[(i32, i32)]) -> RegionData {
        let c = self.region;
        let region = self.obj(c, "Region", vec![]);
        let mut table = Vec::new();
        for &(lo, hi) in rows {
            let row = self.region_row(region, lo, hi, vec![], None);
            table.push(vec![self.balance_definition(vec![row])]);
        }
        let globals = self.table(table, 0.0);
        RegionData {
            globals,
            region,
            missions: vec![],
        }
    }

    /// A region no table names.
    pub fn other_region(&mut self) -> ObjRef {
        let c = self.region;
        self.obj(c, "Unlisted", vec![])
    }

    /// A region with a DLC expansion.
    pub fn dlc_region(&mut self) -> ObjRef {
        let m = self.mission;
        let expansion = self.obj(m, "Expansion", vec![]);
        let c = self.region;
        self.obj(
            c,
            "DlcRegion",
            vec![("DlcExpansion", Value::Object(Some(expansion)))],
        )
    }

    // ---- kills ----

    /// A globals definition with the kill data: `BaseEnemyExperienceFormula` as the record reads the game's (the
    /// killed pawn's `ExperiencePointMultiplier`, scaled by `10 x (ExpLevel^1 + 4)`; BaseValueMode 2, "scales the
    /// base"), and the tables of NATIVE_COMBAT_GROUNDWORK.md 5.2.
    pub fn kill_globals(&mut self) -> ObjRef {
        let multiplier = self.property_attr(PAWN_CONTEXT, "ExperiencePointMultiplier");
        let level = self.property_attr(PAWN_CONTEXT, "ExpLevel");
        let (m, l, p, o) = (
            self.k(10.0),
            self.of_attr(0.0, level),
            self.k(1.0),
            self.k(4.0),
        );
        let f = self.formula(m, l, p, o);
        let def = self.definition(vec![("ValueFormula", f), ("BaseValueMode", Value::Byte(2))]);
        let formula = self.data(InitData {
            base_value_attribute: Some(multiplier),
            initialization_definition: Some(def),
            ..InitData::constant(0.0)
        });
        let mut weights = Vec::new();
        for (bonus, weight) in [(0.0, 1.0), (0.04, 0.9), (0.05, 0.85), (0.06, 0.8)] {
            let ty = self.s.weights;
            weights.push(self.st(
                ty,
                vec![
                    ("KillerExpBonus", Value::Float(bonus)),
                    ("ExpWeight", Value::Float(weight)),
                ],
            ));
        }
        let mut scales = Vec::new();
        for (higher, lower) in [
            (1.0, 0.9),
            (1.03, 0.7),
            (1.06, 0.4),
            (1.09, 0.15),
            (1.12, 0.05),
            (1.15, 0.01),
        ] {
            let ty = self.s.level_scale;
            scales.push(self.st(
                ty,
                vec![
                    ("HigherLevelEnemyExpScale", Value::Float(higher)),
                    ("LowerLevelEnemyExpScale", Value::Float(lower)),
                ],
            ));
        }
        let c = self.globals_definition;
        self.obj(
            c,
            "Globals",
            vec![
                ("BaseEnemyExperienceFormula", formula),
                ("ExpAwardWeights", Value::array(weights)),
                ("ExpScaleByLevelDifference", Value::array(scales)),
            ],
        )
    }

    /// An invented damage-type factor on the globals definition.
    pub fn add_damage_type_modifier(&mut self, globals: ObjRef) {
        let ty = self.s.damage_type;
        let row = self.st(ty, vec![("Multiplier", Value::Float(2.0))]);
        let p = self
            .space
            .prop(self.globals_definition, "DamageTypeExpModifiers")
            .unwrap();
        self.space
            .write(globals, PropAddr::scalar(p), Value::array(vec![row]))
            .unwrap();
    }

    /// An enemy pawn with an experience multiplier and a level.
    pub fn enemy(&mut self, multiplier: f32, level: i32) -> ObjRef {
        let c = self.enemy;
        self.obj(
            c,
            "Enemy",
            vec![
                ("ExperiencePointMultiplier", Value::Float(multiplier)),
                ("ExpLevel", Value::Int(level)),
            ],
        )
    }

    /// A vehicle pawn.
    pub fn vehicle(&mut self) -> ObjRef {
        let c = self.vehicle;
        self.obj(c, "Vehicle", vec![])
    }

    // ---- skill points ----

    /// A globals definition whose `GeneralSkillPointsPerLevelUp` is the record's conditional: 1 when
    /// `PlayerExperienceLevel` is at least 5, else 0 (NATIVE_PROGRESSION.md section 3).
    pub fn skill_point_globals(&mut self) -> ObjRef {
        let level = self.property_attr(PRI_CONTEXT, "ExpLevel");
        let ty = self.s.expr;
        let e = self.st(
            ty,
            vec![
                ("AttributeOperand1", Value::Object(Some(level))),
                (
                    "ComparisonOperator",
                    Value::Byte(Comparison::GreaterThanOrEqual as u8),
                ),
                (
                    "Operand2Usage",
                    Value::Byte(Operand2Usage::PreferAttribute as u8),
                ),
                ("AttributeOperand2", Value::Object(None)),
                ("ConstantOperand2", Value::Float(5.0)),
            ],
        );
        let (one, zero) = (self.k(1.0), self.k(0.0));
        let ty = self.s.entry;
        let entry = self.st(
            ty,
            vec![
                ("Expressions", Value::array(vec![e])),
                ("BaseValueIfTrue", one),
            ],
        );
        let ty = self.s.conditional;
        let cond = self.st(
            ty,
            vec![
                ("bEnabled", Value::Bool(true)),
                ("ConditionalExpressionList", Value::array(vec![entry])),
                ("DefaultBaseValue", zero),
            ],
        );
        let def = self.definition(vec![("ConditionalInitialization", cond)]);
        let data = self.data(InitData {
            initialization_definition: Some(def),
            ..InitData::constant(0.0)
        });
        let c = self.globals_definition;
        self.obj(c, "Globals", vec![("GeneralSkillPointsPerLevelUp", data)])
    }
}

/// Writes property `name` of `obj` in the world's space.
pub(crate) fn set(world: &mut OfflineWorld, obj: ObjRef, name: &str, v: Value) {
    let space = world.space_mut();
    let class = space.heap.get(obj).unwrap().class;
    let p = space.prop(class, name).unwrap();
    space.write(obj, PropAddr::scalar(p), v).unwrap();
}
