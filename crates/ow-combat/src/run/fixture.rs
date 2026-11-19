//! Owner: lane W6-COMBAT. The invented packages of the combat tests (AGENTS.md rule 9: invented classes, values and
//! script bodies, nothing from the game). A port of `tests/combat_test.py`'s builders without their loot part (lane
//! W6-LOOT), with the same fixture fixes (a toy root class every object and class property names, `ResourcePools[16]`,
//! the fields the world's actor spawn reads, the manager's `Role` default), plus a toy armored pawn whose `TakeDamage`
//! runs `AdjustDamage` and the data its stages read (a level-difference table, a damage type with a flesh modifier, an
//! armor attribute). Only `ow_package::testkit` is used, so `ow-cli`'s tests include this file by path.
//!
//! The names are the vocabulary the natives read; the layouts and every value are invented. The toy scripts have the
//! order of the real chain and none of its arithmetic (see `tests/combat_test.py`'s docstring).

use ow_package::testkit::reflect::property_payload;
use ow_package::testkit::script::{
    Asm, CPF_OPT, CPF_OUT, CPF_PARM, CPF_RET, FunctionSpec, ScriptToy, function_payload,
};
use ow_package::testkit::{Export, PackageBuilder, ints};

/// The loot part of the packages (lane W6-LOOT's file; one call in each builder below).
#[path = "../loot/fixture.rs"]
mod loot;

/// `FUNC_Final | FUNC_Defined | FUNC_Public`, `FUNC_Native | FUNC_Public`, `FUNC_Static`.
pub const SCRIPT: u32 = 0x1 | 0x2 | 0x20000;
pub const NATIVE: u32 = 0x400 | 0x20000;
pub const STATIC: u32 = 0x2000;

const EX_LET: u8 = 0x0F;
const EX_LET_BOOL: u8 = 0x14;
const EX_TRUE: u8 = 0x27;
const EX_FALSE: u8 = 0x28;
const EX_NO_OBJECT: u8 = 0x2A;
const EX_SELF: u8 = 0x17;
const EX_RETURN: u8 = 0x04;

/// One declared parameter: kind (without `Property`), name, extra flags, type reference.
pub struct P<'a>(pub &'a str, pub &'a str, pub u64, pub i32);

/// The reflection helpers over one package with the toy root class (module documentation).
pub struct Toy {
    pub t: ScriptToy,
    pub root: i32,
}

impl Toy {
    fn new(kinds: &[&str]) -> Toy {
        Toy {
            t: ScriptToy::new(kinds),
            root: 0,
        }
    }

    fn p(&mut self) -> &mut PackageBuilder {
        &mut self.t.p
    }

    /// The import of Engine's toy root class.
    fn import_root(&mut self) {
        let engine = self.t.p.add_import("Core", "Package", 0, "Engine");
        self.root = self.t.p.add_import("Core", "Class", engine, "Object");
    }

    pub fn import_full(&mut self, class: &str, outer: i32, name: &str) -> i32 {
        self.t.p.add_import("Core", class, outer, name)
    }

    pub fn cls(&mut self, name: &str, super_ref: i32) -> i32 {
        let s = if super_ref == 0 { self.root } else { super_ref };
        self.t.class(name, s)
    }

    fn tail(&self, kind: &str, type_ref: i32) -> Vec<u8> {
        match kind {
            "Object" => ints(&[if type_ref == 0 { self.root } else { type_ref }]),
            "Class" => ints(&[self.root, if type_ref == 0 { self.root } else { type_ref }]),
            _ => ints(&[type_ref]),
        }
    }

    pub fn prop(&mut self, kind: &str, owner: i32, name: &str, flags: u64, type_ref: i32) -> i32 {
        let tail = self.tail(kind, type_ref);
        self.t.prop_tail(kind, owner, name, flags, &tail)
    }

    pub fn prop_dim(&mut self, kind: &str, owner: i32, name: &str, dim: u32, type_ref: i32) -> i32 {
        let tail = self.tail(kind, type_ref);
        let payload = property_payload(&self.t.none, dim, 0, &tail);
        let mut e = Export::new(self.t.import(&format!("{kind}Property")), name, payload);
        e.outer = owner;
        self.t.p.add_export_with(e)
    }

    pub fn array_of(&mut self, owner: i32, name: &str, inner: &str, inner_ref: i32) -> i32 {
        let index = self.t.p.next_export();
        self.t.prop("Array", owner, name, 0, index + 1);
        self.prop(inner, index, name, 0, inner_ref);
        index
    }

    pub fn strukt(&mut self, owner: i32, name: &str) -> i32 {
        let mut e = Export::new(self.t.import("ScriptStruct"), name, vec![0u8; 16]);
        e.outer = owner;
        self.t.p.add_export_with(e)
    }

    /// A function: the return value first, then the declared parameters in reverse order, each a property export,
    /// then the Function export (`ScriptToy::function`, with object and class parameters typed as [`Toy::prop`]
    /// types them). `body` gets the parameter exports by name. Returns the function and its parameters.
    pub fn function(
        &mut self,
        owner: i32,
        name: &str,
        declared: &[P<'_>],
        result: Option<(&str, i32)>,
        flags: u32,
        body: impl FnOnce(&dyn Fn(&str) -> i32) -> Option<Asm>,
    ) -> (i32, Vec<(String, i32)>) {
        let mut ordered: Vec<(String, String, u64, i32)> = Vec::new();
        if let Some((kind, r)) = result {
            ordered.push((
                kind.into(),
                "ReturnValue".into(),
                CPF_PARM | CPF_OUT | CPF_RET,
                r,
            ));
        }
        for d in declared.iter().rev() {
            ordered.push((d.0.into(), d.1.into(), CPF_PARM | d.2, d.3));
        }
        let function = self.t.p.next_export() + i32::try_from(ordered.len()).expect("fits");
        let mut ids = Vec::new();
        for (kind, child, f, r) in &ordered {
            let reference = self.prop(kind, function, child, *f, *r);
            ids.push((child.clone(), reference));
        }
        let lookup = |n: &str| {
            ids.iter()
                .find(|(c, _)| c == n)
                .map(|x| x.1)
                .expect("a parameter")
        };
        let asm = body(&lookup);
        let spec = FunctionSpec::new(flags, name);
        let payload = function_payload(&mut self.t.p, asm.as_ref(), &spec);
        let mut e = Export::new(self.t.import("Function"), name, payload);
        e.outer = owner;
        assert_eq!(self.t.p.add_export_with(e), function);
        (function, ids)
    }

    pub fn native(
        &mut self,
        owner: i32,
        name: &str,
        declared: &[P<'_>],
        result: Option<(&str, i32)>,
    ) -> i32 {
        self.function(owner, name, declared, result, NATIVE, |_| None)
            .0
    }

    pub fn build(&mut self) -> Vec<u8> {
        self.t.build()
    }
}

/// The eight parameters of every toy `TakeDamage`, in the C++ test's order.
fn take_damage_params() -> [P<'static>; 8] {
    [
        P("Float", "Damage", 0, 0),
        P("Object", "InstigatedBy", 0, 0),
        P("Float", "HitLocation", 0, 0),
        P("Float", "Momentum", 0, 0),
        P("Class", "DamageType", 0, 0),
        P("Float", "HitInfo", 0, 0),
        P("Object", "DamageCauser", 0, 0),
        P("Object", "Pipeline", 0, 0),
    ]
}

/// `Engine`: the root class, Actor and the world, the pool classes and their toy script, the manager, the pawn and
/// controllers, the attribute data classes.
pub fn engine() -> Vec<u8> {
    let mut e = Toy::new(&[
        "Byte", "Object", "Bool", "Float", "Array", "Struct", "Class", "Name", "Int",
    ]);
    e.root = e.t.class("Object", 0);
    let actor = e.cls("Actor", 0);
    e.prop("Byte", actor, "Role", 0, 0);
    e.prop("Object", actor, "WorldInfo", 0, 0);
    e.prop("Object", actor, "Owner", 0, 0);
    e.prop("Bool", actor, "bCanTarget", 0, 0);
    for b in ["bCollideWorld", "bCollideWhenPlacing", "bDeleteMe"] {
        e.prop("Bool", actor, b, 0, 0);
    }
    for f in ["Location", "Rotation", "CreationTime"] {
        e.prop("Float", actor, f, 0, 0);
    }
    e.prop("Object", actor, "Instigator", 0, 0);
    e.prop("Float", actor, "MostRecentDamageTaken", 0, 0);
    let mut physics = ints(&[0]);
    physics.extend_from_slice(&e.t.none);
    physics.extend(ints(&[0, 2]));
    for v in ["PHYS_None", "PHYS_Walking"] {
        let n = e.p().fname(v, 0);
        physics.extend_from_slice(&n);
    }
    let mut x = Export::new(e.t.import("Enum"), "EPhysics", physics);
    x.outer = actor;
    let physics = e.p().add_export_with(x);
    e.prop("Byte", actor, "Physics", 0, physics);
    e.native(
        actor,
        "GetDamagePipeline",
        &[
            P("Object", "InDamageTypeDef", 0, 0),
            P("Object", "InImpactDef", 0, 0),
            P("Bool", "inCanHitFriendly", 0, 0),
            P("Object", "InHitObject", 0, 0),
        ],
        Some(("Object", 0)),
    );
    e.native(
        actor,
        "ReleaseDamagePipeline",
        &[P("Object", "Pipeline", 0, 0)],
        None,
    );
    e.native(
        actor,
        "GetPipelineDamageType",
        &[P("Object", "Pipeline", 0, 0)],
        Some(("Object", 0)),
    );
    e.native(
        actor,
        "ActorTakeDamageInner",
        &[
            P("Float", "DamageAmount", 0, 0),
            P("Object", "EventInstigator", 0, 0),
            P("Float", "HitLocation", 0, 0),
            P("Class", "DamageType", 0, 0),
            P("Object", "Pipeline", 0, 0),
        ],
        None,
    );
    e.native(
        actor,
        "SetCanTarget",
        &[P("Bool", "bCanTargetIn", 0, 0)],
        None,
    );
    e.native(actor, "CanTarget", &[], Some(("Bool", 0)));
    let mut payload = ints(&[0]);
    payload.extend_from_slice(&e.t.none);
    let roles = [
        "ROLE_None",
        "ROLE_SimulatedProxy",
        "ROLE_AutonomousProxy",
        "ROLE_Authority",
    ];
    payload.extend(ints(&[0, roles.len() as i32]));
    for r in roles {
        let n = e.p().fname(r, 0);
        payload.extend_from_slice(&n);
    }
    let mut x = Export::new(e.t.import("Enum"), "ENetRole", payload);
    x.outer = actor;
    e.p().add_export_with(x);
    let reference = e.strukt(actor, "ResourcePoolReference");
    e.prop("Object", reference, "PoolManager", 0, 0);
    e.prop("Byte", reference, "PoolIndexInManager", 0, 0);
    e.prop("Byte", reference, "PoolGUID", 0, 0);
    e.prop("Object", reference, "Data", 0, 0);
    e.native(
        actor,
        "IsResourcePoolValid",
        &[P("Struct", "PoolReference", CPF_OUT, reference)],
        Some(("Bool", 0)),
    );
    e.native(
        actor,
        "GetResourcePoolCurrentValueOrDefault",
        &[
            P("Struct", "PoolReference", CPF_OUT, reference),
            P("Float", "DefaultValue", 0, 0),
        ],
        Some(("Float", 0)),
    );
    let world = e.cls("WorldInfo", actor);
    e.prop("Float", world, "TimeSeconds", 0, 0);
    e.prop("Byte", world, "NetMode", 0, 0);
    e.prop("Object", world, "Game", 0, 0);
    e.prop("Object", world, "GRI", 0, 0);
    let controller = e.cls("Controller", actor);
    e.prop("Object", controller, "Pawn", 0, 0);
    e.prop("Object", controller, "ResourcePoolManager", 0, 0);
    // The class starting values (NATIVE_COMBAT_GROUNDWORK.md 1.10, 1.11; decision 2026-11-19-26): the AI mind's route.
    e.native(controller, "ApplyCharacterClassStartingValues", &[P("Object", "inCharacterClass", 0, 0)], None);
    let pawn = e.cls("Pawn", actor);
    e.prop("Object", pawn, "Controller", 0, 0);
    e.prop("Struct", pawn, "HealthPool", 0, reference);
    e.prop("Float", pawn, "HealthVar", 0, 0);
    e.prop("Float", pawn, "HealthMaxVar", 0, 0);
    e.prop("Bool", pawn, "bIsInjured", 0, 0);
    e.prop("Bool", pawn, "bIsDead", 0, 0);
    e.prop("Bool", pawn, "bCanBeInjured", 0, 0);
    e.prop("Object", pawn, "Allegiance", 0, 0);
    e.native(
        pawn,
        "PawnCheckTakeDamagePreconditions",
        &[],
        Some(("Bool", 0)),
    );
    e.native(
        pawn,
        "GetOpinion",
        &[P("Object", "Other", 0, 0)],
        Some(("Byte", 0)),
    );
    for n in ["IsEnemy", "IsNeutral", "IsFriendly"] {
        e.native(pawn, n, &[P("Object", "Other", 0, 0)], Some(("Bool", 0)));
    }
    let allegiance = e.cls("PawnAllegiance", 0);
    for n in ["SelfOpinion", "DefaultOpinion", "ForcedOtherOpinion"] {
        e.prop("Byte", allegiance, n, 0, 0);
    }
    e.prop("Bool", allegiance, "bForceAllOtherOpinions", 0, 0);
    let opinion = e.strukt(allegiance, "AllegianceOpinion");
    e.prop("Object", opinion, "Allegiance", 0, 0);
    e.prop("Byte", opinion, "Opinion", 0, 0);
    e.array_of(allegiance, "MyOpinions", "Struct", opinion);
    e.array_of(allegiance, "OtherOpinions", "Struct", opinion);
    // The damage type definition's natives (Engine's class; WillowGame's toy class of the same name derives from it).
    let type_definition = e.cls("DamageTypeDefinition", 0);
    e.native(
        type_definition,
        "GetSurfaceDamageTypeModifier",
        &[P("Byte", "Surface", 0, 0), P("Object", "Instigator", 0, 0)],
        Some(("Float", 0)),
    );
    e.native(
        type_definition,
        "GetPawnDamageTypeModifier",
        &[P("Object", "Pawn", 0, 0)],
        Some(("Float", 0)),
    );
    let tracker = e.strukt(type_definition, "RecentDamageTracker");
    for n in ["TotalDamage", "HealthDamage", "ShieldDamage"] {
        e.prop("Float", tracker, n, 0, 0);
    }
    e.native(
        type_definition,
        "RecordRecentDamage",
        &[
            P("Struct", "RecentDamage", CPF_OUT, tracker),
            P("Float", "Damage", 0, 0),
            P("Object", "DamageInstigator", 0, 0),
            P("Object", "DamageTypeDefinition", 0, 0),
            P("Bool", "bWasShieldDamage", 0, 0),
        ],
        None,
    );
    let tree = e.cls("ExpressionTree", 0);
    e.prop("Object", tree, "RootChild", 0, 0);
    e.native(
        tree,
        "Evaluate",
        &[P("Object", "Context", 0, 0)],
        Some(("Bool", 0)),
    );
    e.native(pawn, "IsInjured", &[], Some(("Bool", 0)));
    e.native(pawn, "IsDead", &[], Some(("Bool", 0)));
    e.native(pawn, "GetHealth", &[], Some(("Float", 0)));
    e.native(pawn, "SetHealth", &[P("Float", "NewHealth", 0, 0)], None);
    e.native(pawn, "GetMaxHealth", &[], Some(("Float", 0)));
    e.native(
        pawn,
        "SetMaxHealth",
        &[P("Float", "NewMaxHealth", 0, 0)],
        None,
    );
    e.native(
        pawn,
        "SetHealthResourcePoolDefinition",
        &[
            P("Object", "ResourcePoolManager", CPF_OUT, 0),
            P("Object", "Definition", 0, 0),
        ],
        None,
    );
    e.native(
        pawn,
        "NotifyTakeHit",
        &[
            P("Object", "InstigatedBy", 0, 0),
            P("Float", "HitLocation", 0, 0),
            P("Float", "Damage", 0, 0),
            P("Class", "DamageType", 0, 0),
            P("Float", "Momentum", 0, 0),
            P("Object", "Pipeline", 0, 0),
        ],
        None,
    );

    let resource = e.cls("ResourceDefinition", 0);
    e.prop("Class", resource, "DefaultResourcePoolClass", 0, 0);
    e.prop("Bool", resource, "bIntegerOnlyUpdates", 0, 0);
    let pool_definition = e.cls("ResourcePoolDefinition", 0);
    e.prop("Object", pool_definition, "Resource", 0, 0);
    let starting = e.prop("Float", pool_definition, "StartingValue", 0, 0);
    let base_min = e.prop("Float", pool_definition, "BaseMinValue", 0, 0);
    let pool = e.cls("ResourcePool", 0);
    e.prop("Byte", pool, "PoolGUID", 0, 0);
    let definition = e.prop("Object", pool, "Definition", 0, 0);
    let provider = e.prop("Object", pool, "AssociatedProvider", 0, 0);
    let authoritative = e.prop("Bool", pool, "bIsAuthoritative", 0, 0);
    let current = e.prop("Float", pool, "CurrentValue", 0, 0);
    let maximum = e.prop("Float", pool, "MaxValue", 0, 0);
    let minimum = e.prop("Float", pool, "MinValue", 0, 0);
    for n in [
        "RateRemainder",
        "PoolIdleDelayStartTime",
        "LastCurrentValue",
        "LastMinValue",
        "LastMaxValue",
    ] {
        e.prop("Float", pool, n, 0, 0);
    }
    e.prop("Bool", pool, "bCreatedAndNotModified", 0, 0);
    // InitializeAuthoritative: Definition = InDefinition; AssociatedProvider = Provider; bIsAuthoritative = true;
    // MaxValue = Definition.StartingValue; MinValue = Definition.BaseMinValue; CurrentValue = MaxValue.
    e.function(
        pool,
        "InitializeAuthoritative",
        &[
            P("Object", "InDefinition", 0, 0),
            P("Object", "ProviderToAssociatePoolWith", 0, 0),
        ],
        None,
        SCRIPT,
        |ids| {
            let mut a = Asm::new();
            a.stmt()
                .raw(&[EX_LET])
                .instance(definition)
                .local(ids("InDefinition"));
            a.stmt()
                .raw(&[EX_LET])
                .instance(provider)
                .local(ids("ProviderToAssociatePoolWith"));
            a.stmt()
                .raw(&[EX_LET_BOOL])
                .instance(authoritative)
                .raw(&[EX_TRUE]);
            a.stmt().raw(&[EX_LET]).instance(maximum).context(
                |a| {
                    a.instance(definition);
                },
                |a| {
                    a.instance(starting);
                },
            );
            a.stmt().raw(&[EX_LET]).instance(minimum).context(
                |a| {
                    a.instance(definition);
                },
                |a| {
                    a.instance(base_min);
                },
            );
            a.stmt().raw(&[EX_LET]).instance(current).instance(maximum);
            a.stmt().return_nothing();
            a.end();
            Some(a)
        },
    );
    e.function(pool, "ShutDown", &[], None, SCRIPT, |_| {
        let mut a = Asm::new();
        a.stmt()
            .raw(&[EX_LET])
            .instance(definition)
            .raw(&[EX_NO_OBJECT]);
        a.stmt()
            .raw(&[EX_LET])
            .instance(provider)
            .raw(&[EX_NO_OBJECT]);
        a.stmt().raw(&[EX_LET]).instance(current).float_const(0.0);
        a.stmt().raw(&[EX_LET]).instance(maximum).float_const(0.0);
        a.stmt().return_nothing();
        a.end();
        Some(a)
    });
    e.function(pool, "Reinitialize", &[], None, SCRIPT, |_| {
        let mut a = Asm::new();
        a.stmt().return_nothing();
        a.end();
        Some(a)
    });

    let manager = e.cls("ResourcePoolManager", actor);
    e.prop_dim("Object", manager, "ResourcePools", 16, 0);
    e.prop("Byte", manager, "NextPoolGUID", 0, 0);
    let role = {
        let p = e.p();
        let none_enum = p.fname("None", 0);
        let mut tags = p.tag_detail("Role", "ByteProperty", &none_enum, &[3]);
        tags.extend_from_slice(&p.none());
        let mut payload = ints(&[0]);
        payload.extend(tags);
        payload
    };
    e.p()
        .add_export(manager, "Default__ResourcePoolManager", role);
    e.function(
        manager,
        "CreatePool",
        &[
            P("Object", "Mgr", CPF_OUT, 0),
            P("Object", "ActorOwner", 0, 0),
            P("Object", "PoolDefinition", 0, 0),
            P("Bool", "bReinitializeExistingPool", CPF_OPT, 0),
        ],
        Some(("Struct", reference)),
        NATIVE | STATIC,
        |_| None,
    );
    e.function(
        manager,
        "DeletePoolByReference",
        &[P("Struct", "PoolReference", CPF_OUT, reference)],
        None,
        NATIVE | STATIC,
        |_| None,
    );
    let attribute = e.cls("AttributeDefinition", 0);
    e.array_of(attribute, "ContextResolverChain", "Object", 0);
    e.array_of(attribute, "ValueResolverChain", "Object", 0);
    let definition_class = e.cls("AttributeInitializationDefinition", 0);
    let data = e.strukt(definition_class, "AttributeInitializationData");
    for n in ["BaseValueConstant", "BaseValueScaleConstant"] {
        e.prop("Float", data, n, 0, 0);
    }
    for n in ["BaseValueAttribute", "InitializationDefinition"] {
        e.prop("Object", data, n, 0, 0);
    }
    loot::engine_loot(&mut e, definition_class, data);
    e.build()
}

/// `GearboxFramework`: the AI component's demigod hold natives.
pub fn gearbox() -> Vec<u8> {
    let mut g = Toy::new(&["Byte", "Object", "Bool", "Name", "Float", "Class", "Int"]);
    g.import_root();
    g.cls("NoContextNeededAttributeContextResolver", 0);
    let constant = g.cls("ConstantAttributeValueResolver", 0);
    g.prop("Float", constant, "ConstantValue", 0, 0);
    let component = g.cls("AIComponent", 0);
    g.native(
        component,
        "HoldDemigod",
        &[P("Name", "Reason", 0, 0), P("Object", "Source", 0, 0)],
        None,
    );
    g.native(
        component,
        "ReleaseDemigod",
        &[P("Name", "Reason", 0, 0), P("Object", "Source", 0, 0)],
        None,
    );
    g.native(component, "DemigodOnHold", &[], Some(("Bool", 0)));
    let compound = g.cls("CompoundExpressionEvaluator", 0);
    let mut ops = ints(&[0]);
    ops.extend_from_slice(&g.t.none);
    ops.extend(ints(&[0, 3]));
    for v in ["CEOP_And", "CEOP_Or", "CEOP_Xor"] {
        let n = g.p().fname(v, 0);
        ops.extend_from_slice(&n);
    }
    let mut x = Export::new(g.t.import("Enum"), "ECExpressionOperatorType", ops);
    x.outer = compound;
    let ops = g.p().add_export_with(x);
    g.prop("Byte", compound, "Operator", 0, ops);
    for n in ["Expression1", "Expression2"] {
        g.prop("Object", compound, n, 0, 0);
    }
    g.native(
        compound,
        "Evaluate",
        &[P("Object", "Context", 0, 0)],
        Some(("Bool", 0)),
    );
    let physics = g.cls("PhysicsStateExpressionEvaluator", 0);
    g.prop("Byte", physics, "PhysicsState", 0, 0);
    g.prop("Bool", physics, "bIsInState", 0, 0);
    g.native(
        physics,
        "Evaluate",
        &[P("Object", "Context", 0, 0)],
        Some(("Bool", 0)),
    );
    let kernel = g.cls("BehaviorKernel", 0);
    g.function(
        kernel,
        "ActivateBehaviorOutputLink",
        &[
            P("Object", "KernelInfo", 0, 0),
            P("Int", "OutputLinkId", 0, 0),
        ],
        None,
        NATIVE | STATIC,
        |_| None,
    );
    g.build()
}

/// `WillowGame`: the AI pawn and mind, the toy chains (demigod, death, down, swing, the armored pawn), the game info
/// with the experience natives, the pipeline, the damage sources and the definition classes the swing reads.
pub fn willow_game() -> Vec<u8> {
    let mut w = Toy::new(&[
        "Byte", "Object", "Int", "Float", "Bool", "Class", "Struct", "Array", "Name",
    ]);
    w.import_root();
    let engine = w.import_full("Package", 0, "Engine");
    let gearbox = w.import_full("Package", 0, "GearboxFramework");
    let pawn = w.import_full("Class", engine, "Pawn");
    let controller = w.import_full("Class", engine, "Controller");
    let actor = w.import_full("Class", engine, "Actor");
    let set_health = w.import_full("Function", pawn, "SetHealth");
    let set_health_definition = w.import_full("Function", pawn, "SetHealthResourcePoolDefinition");
    let manager_property = w.import_full("ObjectProperty", controller, "ResourcePoolManager");
    let notify = w.import_full("Function", pawn, "NotifyTakeHit");
    let ai_class = w.cls("AIClassDefinition", 0);
    let health_definition_property = w.prop("Object", ai_class, "HealthPoolDefinition", 0, 0);
    w.prop("Object", ai_class, "AIDef", 0, 0);
    let starting = w.strukt(ai_class, "AttributeStartingValue");
    w.prop("Object", starting, "Attribute", 0, 0);
    // The spawn move `SetDefaultPhysics` gates on (a `WillowGame` class; review r1 of #418, decision 2026-11-19-32).
    w.cls("SpecialMove_Spawned", 0);
    let mind = w.cls("WillowMind", controller);
    let mind_ai_class = w.prop("Object", mind, "AIClass", 0, 0);
    // The AI mind's class route (NATIVE_COMBAT_GROUNDWORK.md section 1.11; decision 2026-11-19-26), cut to what the toy
    // pawns need: InitializeCharacterClass() { ApplyCharacterClassStartingValues(AIClass);
    // ApplyCharacterClassDefaults(MyWillowPawn, AIClass); } (the installed guard flag left out) and
    // ApplyCharacterClassDefaults(P, CharClass) cut to its health step, as the player controller's below.
    let my_pawn = w.prop("Object", mind, "MyWillowPawn", 0, 0);
    let class_starting_values = w.import_full("Function", controller, "ApplyCharacterClassStartingValues");
    let (mind_defaults, _) = w.function(
        mind,
        "ApplyCharacterClassDefaults",
        &[P("Object", "P", 0, 0), P("Object", "CharClass", 0, 0)],
        None,
        SCRIPT,
        |ids| {
            let mut a = Asm::new();
            a.stmt().context(
                |a| {
                    a.local(ids("P"));
                },
                |a| {
                    a.final_function(set_health_definition, |a| {
                        a.instance(manager_property);
                        a.context(
                            |a| {
                                a.local(ids("CharClass"));
                            },
                            |a| {
                                a.instance(health_definition_property);
                            },
                        );
                    });
                },
            );
            a.stmt().return_nothing();
            a.end();
            Some(a)
        },
    );
    w.function(mind, "InitializeCharacterClass", &[], None, SCRIPT, |_| {
        let mut a = Asm::new();
        a.stmt().final_function(class_starting_values, |a| {
            a.instance(mind_ai_class);
        });
        a.stmt().final_function(mind_defaults, |a| {
            a.instance(my_pawn);
            a.instance(mind_ai_class);
        });
        a.stmt().return_nothing();
        a.end();
        Some(a)
    });
    let component_property = w.prop("Object", mind, "AIComponent", 0, 0);
    let gearbox_component = w.import_full("Class", gearbox, "AIComponent");
    w.cls("WillowAIComponent", gearbox_component);
    let on_hold = w.import_full("Function", gearbox_component, "DemigodOnHold");
    let willow_pawn = w.cls("WillowPawn", pawn);
    w.prop("Byte", willow_pawn, "SecondWindReason", 0, 0);
    w.prop("Object", willow_pawn, "BodyClass", 0, 0);
    w.prop("Object", willow_pawn, "MyDeathDef", 0, 0);
    let pid = {
        let p = &mut w.t.p;
        let mut tags = p.int_tag("PID", -1);
        tags.extend_from_slice(&p.none());
        tags
    };
    let handle = ow_package::testkit::reflect::toy_struct(
        &mut w.t,
        willow_pawn,
        "BehaviorConsumerHandle",
        Some(&pid),
    );
    w.prop("Int", handle, "PID", 0, 0);
    w.prop("Struct", willow_pawn, "ConsumerHandle", 0, handle);
    let hit_info = w.strukt(willow_pawn, "TraceHitInfo");
    w.prop("Name", hit_info, "BoneName", 0, 0);
    w.native(
        willow_pawn,
        "GetHitRegionForTakenDamage",
        &[
            P("Object", "Instigator", 0, 0),
            P("Struct", "HitInfo", 0, hit_info),
        ],
        Some(("Object", 0)),
    );
    w.native(
        willow_pawn,
        "SetRevivePct",
        &[P("Float", "Pct", 0, 0)],
        None,
    );
    w.native(willow_pawn, "GetRevivePct", &[], Some(("Float", 0)));
    let body = w.cls("BodyClassDefinition", 0);
    w.array_of(body, "HitRegionList", "Object", 0);
    w.prop("Object", body, "DefaultHitRegion", 0, 0);
    w.prop("Object", body, "DeathDef", 0, 0);
    let region = w.cls("HitRegionDefinition", 0);
    w.array_of(region, "RegionBoneNames", "Name", 0);
    w.prop("Bool", region, "bCriticalHit", 0, 0);
    w.prop("Bool", region, "bTrackDamage", 0, 0);
    w.prop("Byte", region, "DefaultDamageSurfaceType", 0, 0);
    let response = w.strukt(region, "ImpactResponse");
    w.prop("Float", response, "DamageModifierPercent", 0, 0);
    w.prop("Struct", region, "DefaultImpactResponse", 0, response);
    let set_wind = w.native(
        willow_pawn,
        "SetSecondWindReason",
        &[P("Byte", "Reason", 0, 0)],
        None,
    );
    let clear_wind = w.native(willow_pawn, "ClearSecondWindReason", &[], None);
    let ai_pawn = w.cls("WillowAIPawn", willow_pawn);
    w.prop("Object", ai_pawn, "AIClass", 0, 0);
    w.prop("Byte", ai_pawn, "bInjured", 0, 0);
    for n in [
        "ExperiencePointMultiplier",
        "ExperiencePointMultiplierBaseValue",
    ] {
        w.prop("Float", ai_pawn, n, 0, 0);
    }
    w.prop("Int", ai_pawn, "ExpLevel", 0, 0);
    w.prop("Int", ai_pawn, "GameStage", 0, 0);
    let mind_property = w.prop("Object", ai_pawn, "MyWillowMind", 0, 0);

    // IsDemigodMode(): MyWillowMind.AIComponent.DemigodOnHold()
    let (demigod, _) = w.function(
        ai_pawn,
        "IsDemigodMode",
        &[],
        Some(("Bool", 0)),
        SCRIPT,
        |_| {
            let mut a = Asm::new();
            a.stmt().raw(&[EX_RETURN]).context(
                |a| {
                    a.context(
                        |a| {
                            a.instance(mind_property);
                        },
                        |a| {
                            a.instance(component_property);
                        },
                    );
                },
                |a| {
                    a.final_function(on_hold, |_| {});
                },
            );
            a.end();
            Some(a)
        },
    );
    let notify_args = |a: &mut Asm, ids: &dyn Fn(&str) -> i32| {
        for n in [
            "InstigatedBy",
            "HitLocation",
            "Damage",
            "DamageType",
            "Momentum",
            "Pipeline",
        ] {
            a.local(ids(n));
        }
    };
    // TakeDamage: if (IsDemigodMode()) SetHealth(1.0) else SetHealth(0.0); NotifyTakeHit(...)
    w.function(
        ai_pawn,
        "TakeDamage",
        &take_damage_params(),
        None,
        SCRIPT,
        |ids| {
            let mut a = Asm::new();
            let otherwise = a.stmt().jump_if_not(|a| {
                a.final_function(demigod, |_| {});
            });
            a.stmt().final_function(set_health, |a| {
                a.float_const(1.0);
            });
            let over = a.stmt().jump();
            a.patch(otherwise);
            a.stmt().final_function(set_health, |a| {
                a.float_const(0.0);
            });
            a.patch(over);
            a.stmt().final_function(notify, |a| notify_args(a, ids));
            a.stmt().return_nothing();
            a.end();
            Some(a)
        },
    );

    // The death chain (WillowDoomedPawn).
    let world_info = w.import_full("Class", engine, "WorldInfo");
    let world_property = w.import_full("ObjectProperty", actor, "WorldInfo");
    let game_property = w.import_full("ObjectProperty", world_info, "Game");
    let gri_property = w.import_full("ObjectProperty", world_info, "GRI");
    let game_info = w.cls("WillowGameInfo", actor);
    w.prop("Int", game_info, "EffectiveNumPlayers", 0, 0);
    let globals_class = w.cls("GlobalsDefinition", 0);
    let data = w.strukt(globals_class, "AttributeInitializationData");
    for n in ["BaseValueConstant", "BaseValueScaleConstant"] {
        w.prop("Float", data, n, 0, 0);
    }
    for n in ["BaseValueAttribute", "InitializationDefinition"] {
        w.prop("Object", data, n, 0, 0);
    }
    w.prop(
        "Struct",
        globals_class,
        "BaseEnemyExperienceFormula",
        0,
        data,
    );
    w.prop("Struct", globals_class, "GlobalAIDamageScale", 0, data);
    w.prop("Struct", starting, "BaseValue", 0, data);
    w.array_of(ai_class, "AttributeStartingValues", "Struct", starting);
    let weight = w.strukt(globals_class, "ExpAwardWeight");
    w.prop("Int", weight, "Players", 0, 0);
    for n in ["KillerExpBonus", "ExpWeight"] {
        w.prop("Float", weight, n, 0, 0);
    }
    w.array_of(globals_class, "ExpAwardWeights", "Struct", weight);
    let scale = w.strukt(globals_class, "ExpScaleByLevel");
    w.prop("Int", scale, "LevelDifference", 0, 0);
    for n in ["HigherLevelEnemyExpScale", "LowerLevelEnemyExpScale"] {
        w.prop("Float", scale, n, 0, 0);
    }
    w.array_of(globals_class, "ExpScaleByLevelDifference", "Struct", scale);
    // The pipeline's level-difference table (stage 2 of AdjustDamage).
    let attacker = w.strukt(globals_class, "DamageScaleByLevel");
    for n in ["HigherLevelAttackerDmgScale", "LowerLevelAttackerDmgScale"] {
        w.prop("Float", attacker, n, 0, 0);
    }
    w.array_of(
        globals_class,
        "PlayerDamageScaleByLevelDifference",
        "Struct",
        attacker,
    );
    let pipeline_x = w.cls("WillowExperiencePipeline", 0);
    let (calculate, _) = w.function(
        pipeline_x,
        "CalculateExperiencePointsForKill",
        &[
            P("Object", "Killer", 0, 0),
            P("Object", "KilledPawn", 0, 0),
            P("Bool", "bKilledByCritical", 0, 0),
            P("Class", "DamageSource", 0, 0),
            P("Object", "DamageTypeDef", 0, 0),
        ],
        Some(("Float", 0)),
        NATIVE | STATIC,
        |_| None,
    );
    let (award, _) = w.function(
        pipeline_x,
        "AwardCombatExperienceToParty",
        &[
            P("Float", "ExperiencePoints", 0, 0),
            P("Object", "Killer", 0, 0),
            P("Object", "KilledActor", 0, 0),
        ],
        None,
        NATIVE | STATIC,
        |_| None,
    );
    let (killed, _) = w.function(
        game_info,
        "Killed",
        &[P("Object", "Killer", 0, 0), P("Object", "KilledPawn", 0, 0)],
        None,
        SCRIPT,
        |ids| {
            let mut a = Asm::new();
            a.stmt().class_context(pipeline_x, |a| {
                a.final_function(award, |a| {
                    a.class_context(pipeline_x, |a| {
                        a.final_function(calculate, |a| {
                            a.local(ids("Killer")).local(ids("KilledPawn")).raw(&[
                                EX_FALSE,
                                EX_NO_OBJECT,
                                EX_NO_OBJECT,
                            ]);
                        });
                    });
                    a.local(ids("Killer")).local(ids("KilledPawn"));
                });
            });
            a.stmt().return_nothing();
            a.end();
            Some(a)
        },
    );
    let tracker = w.cls("MissionTracker", actor);
    let notify_died = w.native(
        tracker,
        "NotifyPawnDied",
        &[P("Object", "KilledPawn", 0, 0)],
        None,
    );
    let replication = w.cls("WillowGameReplicationInfo", actor);
    let tracker_property = w.prop("Object", replication, "MissionTracker", 0, 0);
    w.array_of(replication, "PRIArray", "Object", 0);
    let doomed = w.cls("WillowDoomedPawn", ai_pawn);
    let (died, _) = w.function(
        doomed,
        "Died",
        &[P("Object", "Killer", 0, 0)],
        None,
        SCRIPT,
        |ids| {
            let mut a = Asm::new();
            a.stmt().final_function(set_health, |a| {
                a.float_const(0.0);
            });
            a.stmt().context(
                |a| {
                    a.context(
                        |a| {
                            a.instance(world_property);
                        },
                        |a| {
                            a.instance(game_property);
                        },
                    );
                },
                |a| {
                    a.final_function(killed, |a| {
                        a.local(ids("Killer")).raw(&[EX_SELF]);
                    });
                },
            );
            a.stmt().return_nothing();
            a.end();
            Some(a)
        },
    );
    let (depleted, _) = w.function(
        doomed,
        "HandleHealthDepleted",
        &[P("Object", "InstigatedBy", 0, 0)],
        None,
        SCRIPT,
        |ids| {
            let mut a = Asm::new();
            a.stmt().context(
                |a| {
                    a.context(
                        |a| {
                            a.instance(world_property);
                        },
                        |a| {
                            a.instance(gri_property);
                        },
                    );
                },
                |a| {
                    a.context(
                        |a| {
                            a.instance(tracker_property);
                        },
                        |a| {
                            a.final_function(notify_died, |a| {
                                a.raw(&[EX_SELF]);
                            });
                        },
                    );
                },
            );
            a.stmt().final_function(died, |a| {
                a.local(ids("InstigatedBy"));
            });
            a.stmt().return_nothing();
            a.end();
            Some(a)
        },
    );
    w.function(
        doomed,
        "TakeDamage",
        &take_damage_params(),
        None,
        SCRIPT,
        |ids| {
            let mut a = Asm::new();
            a.stmt().final_function(set_health, |a| {
                a.float_const(-5.0);
            });
            a.stmt().final_function(notify, |a| notify_args(a, ids));
            a.stmt().final_function(depleted, |a| {
                a.local(ids("InstigatedBy"));
            });
            a.stmt().return_nothing();
            a.end();
            Some(a)
        },
    );

    // The down state's natives (WillowDownPawn).
    let injured_property = w.import_full("BoolProperty", pawn, "bIsInjured");
    let set_can_target = w.import_full("Function", actor, "SetCanTarget");
    let can_target = w.import_full("Function", actor, "CanTarget");
    let is_injured = w.import_full("Function", pawn, "IsInjured");
    let down = w.cls("WillowDownPawn", ai_pawn);
    w.function(
        down,
        "TakeDamage",
        &take_damage_params(),
        None,
        SCRIPT,
        |_| {
            let mut a = Asm::new();
            a.stmt()
                .raw(&[EX_LET_BOOL])
                .instance(injured_property)
                .raw(&[EX_TRUE]);
            a.stmt().final_function(set_wind, |a| {
                a.byte_const(3);
            });
            a.stmt().final_function(set_can_target, |a| {
                a.raw(&[EX_FALSE]);
            });
            a.stmt().final_function(set_health, |a| {
                a.float_const(3.0);
            });
            let skip = a.stmt().jump_if_not(|a| {
                a.final_function(is_injured, |_| {});
            });
            a.stmt().final_function(set_health, |a| {
                a.float_const(7.0);
            });
            a.patch(skip);
            let skip = a.stmt().jump_if_not(|a| {
                a.final_function(can_target, |_| {});
            });
            a.stmt().final_function(set_health, |a| {
                a.float_const(1.0);
            });
            a.patch(skip);
            a.stmt().final_function(clear_wind, |_| {});
            a.stmt().return_nothing();
            a.end();
            Some(a)
        },
    );

    // The pipeline and the damage sources.
    let pipeline = w.cls("WillowDamagePipeline", 0);
    for n in ["DamageTypeDef", "ImpactDefinition", "DirectHitObject"] {
        w.prop("Object", pipeline, n, 0, 0);
    }
    w.prop("Bool", pipeline, "bCanHitFriendly", 0, 0);
    let summary = w.strukt(pipeline, "DamageSummary");
    for n in [
        "InitialDamage",
        "PreviousHealth",
        "AIDamageScaleReduction",
        "ExpLevelDifferenceReduction",
        "DamageSourceReduction",
        "InstigatorDamageTypeReduction",
        "HitRegionReduction",
        "ShieldReduction",
        "DamageDealtToShields",
        "IntrinsicArmorReduction",
        "DamageSurfaceReduction",
        "FinalDamage",
    ] {
        w.prop("Float", summary, n, 0, 0);
    }
    for n in ["bWasMaxHealth", "bWasInjured", "bWasCrit"] {
        w.prop("Bool", summary, n, 0, 0);
    }
    w.prop("Struct", pipeline, "DamageSummary", 0, summary);
    let adjust = w.native(
        pipeline,
        "AdjustDamage",
        &[
            P("Float", "IncomingDamage", CPF_OUT, 0),
            P("Float", "DamageSeverityPercent", CPF_OUT, 0),
            P("Object", "DamagedActor", 0, 0),
            P("Object", "DamageInstigator", 0, 0),
            P("Float", "HitLocation", 0, 0),
            P("Class", "DamageSource", 0, 0),
            P("Float", "HitMomentum", CPF_OUT, 0),
            P("Float", "HitInfo", CPF_OUT, 0),
        ],
        Some(("Struct", summary)),
    );
    for s in [
        "WillowDmgSource_Bullet",
        "WillowDmgSource_Melee",
        "WillowDmgSource_Grenade",
    ] {
        w.cls(s, 0);
    }
    let engine_type = w.import_full("Class", engine, "DamageTypeDefinition");
    let type_definition = w.cls("DamageTypeDefinition", engine_type);
    w.prop("Struct", type_definition, "FleshDamageModifier", 0, data);
    w.prop(
        "Object",
        type_definition,
        "CharacterDamageTypeModifierAttribute",
        0,
        0,
    );
    // The armored pawn: TakeDamage runs Pipeline.AdjustDamage(Damage, Momentum, Self, InstigatedBy, HitLocation,
    // DamageType, Momentum, HitInfo) and leaves its health as it is; the hit reads FinalDamage from the pipeline.
    let armored = w.cls("WillowArmoredPawn", ai_pawn);
    w.prop("Float", armored, "IntrinsicArmor", 0, 0);
    // A shield pool reference (the shield stage of AdjustDamage looks for a valid one).
    let pool_reference = w.import_full("ScriptStruct", actor, "ResourcePoolReference");
    w.prop("Struct", armored, "ShieldArmor", 0, pool_reference);
    for n in ["ResistToy", "ResistToyBaseValue"] {
        w.prop("Float", armored, n, 0, 0);
    }
    w.function(
        armored,
        "TakeDamage",
        &take_damage_params(),
        None,
        SCRIPT,
        |ids| {
            let mut a = Asm::new();
            a.stmt().context(
                |a| {
                    a.local(ids("Pipeline"));
                },
                |a| {
                    a.final_function(adjust, |a| {
                        a.local(ids("Damage"))
                            .local(ids("Momentum"))
                            .raw(&[EX_SELF]);
                        a.local(ids("InstigatedBy"))
                            .local(ids("HitLocation"))
                            .local(ids("DamageType"));
                        a.local(ids("Momentum")).local(ids("HitInfo"));
                    });
                },
            );
            a.stmt().return_nothing();
            a.end();
            Some(a)
        },
    );

    // The melee swing's definition classes.
    let melee = w.cls("MeleeDefinition", 0);
    w.prop("Struct", melee, "Damage", 0, data);
    w.prop("Class", melee, "DamageSource", 0, 0);
    w.prop("Object", melee, "DamageTypeDefinition", 0, 0);
    let throw = w.cls("Behavior_AIThrowProjectileAtTarget", 0);
    let projectile_value = w.strukt(throw, "ProjectileBaseValue");
    w.prop("Object", projectile_value, "Attribute", 0, 0);
    w.prop("Struct", projectile_value, "BaseValue", 0, data);
    w.array_of(
        throw,
        "ChildProjectileBaseValues",
        "Struct",
        projectile_value,
    );
    let cause = w.cls("Behavior_CauseDamage", 0);
    w.prop("Struct", cause, "DamageFormula", 0, data);
    w.prop("Class", cause, "DamageSource", 0, 0);
    w.prop("Object", cause, "DamageTypeDefinition", 0, 0);
    let player_controller = w.cls("WillowPlayerController", controller);
    w.prop("Object", player_controller, "PlayerReplicationInfo", 0, 0);
    // The player controller's own native override of the starting values (decision 2026-11-19-46).
    w.native(player_controller, "ApplyCharacterClassStartingValues", &[P("Object", "inCharacterClass", 0, 0)], None);
    // The game's route to the player's health pool, cut to its health step (NATIVE_COMBAT_GROUNDWORK.md section
    // 1.10): ApplyCharacterClassDefaults(CharClass, P) { P.SetHealthResourcePoolDefinition(ResourcePoolManager,
    // CharClass.HealthPoolDefinition); }
    w.function(
        player_controller,
        "ApplyCharacterClassDefaults",
        &[P("Object", "CharClass", 0, 0), P("Object", "P", 0, 0)],
        None,
        SCRIPT,
        |ids| {
            let mut a = Asm::new();
            a.stmt().context(
                |a| {
                    a.local(ids("P"));
                },
                |a| {
                    a.final_function(set_health_definition, |a| {
                        a.instance(manager_property);
                        a.context(
                            |a| {
                                a.local(ids("CharClass"));
                            },
                            |a| {
                                a.instance(health_definition_property);
                            },
                        );
                    });
                },
            );
            a.stmt().return_nothing();
            a.end();
            Some(a)
        },
    );
    let last_exp = w.prop("Int", player_controller, "LastExp", 0, 0);
    // The attribute the armored pawn's stage 3 reads on a bullet hit by the player (an invented value through `attr`).
    w.prop(
        "Float",
        player_controller,
        "InstigatedBulletDamageModifier",
        0,
        0,
    );
    w.prop(
        "Float",
        player_controller,
        "TargetMinHealthDamageBoostPercent",
        0,
        0,
    );
    for n in [
        "CurrentInstantHitCriticalHitBonus",
        "CurrentInstantHitCriticalHitBonusBaseValue",
    ] {
        w.prop("Float", player_controller, n, 0, 0);
    }
    w.function(
        player_controller,
        "ExpEarn",
        &[
            P("Int", "Exp", 0, 0),
            P("Byte", "Source", 0, 0),
            P("Byte", "Type", 0, 0),
        ],
        None,
        SCRIPT,
        |ids| {
            let mut a = Asm::new();
            a.stmt().raw(&[EX_LET]).instance(last_exp).local(ids("Exp"));
            a.stmt().return_nothing();
            a.end();
            Some(a)
        },
    );
    let player_pawn = w.cls("WillowPlayerPawn", pawn);
    w.function(
        player_pawn,
        "TakeDamage",
        &take_damage_params(),
        None,
        SCRIPT,
        |ids| {
            let mut a = Asm::new();
            a.stmt().final_function(set_health, |a| {
                a.local(ids("Damage"));
            });
            a.stmt().return_nothing();
            a.end();
            Some(a)
        },
    );
    w.prop("Object", player_pawn, "PlayerReplicationInfo", 0, 0);
    let info = w.cls("WillowPlayerReplicationInfo", actor);
    w.prop("Int", info, "ExpLevel", 0, 0);
    w.prop("Float", info, "RevivePct", 0, 0);
    let currency = w.strukt(info, "CurrencyState");
    w.prop("Int", currency, "CurrentAmount", 0, 0);
    w.prop_dim("Struct", info, "Currency", 13, currency);
    w.native(
        info,
        "GetCurrencyOnHand",
        &[P("Byte", "Type", 0, 0)],
        Some(("Int", 0)),
    );
    w.native(
        info,
        "AddCurrencyOnHand",
        &[P("Byte", "Type", 0, 0), P("Int", "Delta", 0, 0)],
        None,
    );
    w.function(info, "NotifyCurrencyDelegates", &[], None, SCRIPT, |_| {
        let mut a = Asm::new();
        a.stmt().return_nothing();
        a.end();
        Some(a)
    });
    loot::willow_game_loot(&mut w, globals_class, data, willow_pawn, ai_pawn, set_health);
    w.build()
}

/// The data tags of an `AttributeInitializationData` value `constant x scale`.
fn data(p: &mut PackageBuilder, name: &str, constant: f32, scale: f32) -> Vec<u8> {
    let mut body = p.float_tag("BaseValueConstant", constant);
    body.extend(p.float_tag("BaseValueScaleConstant", scale));
    body.extend_from_slice(&p.none());
    p.struct_tag(name, "AttributeInitializationData", &body)
}

fn object_payload(p: &mut PackageBuilder, tags: Vec<Vec<u8>>) -> Vec<u8> {
    let mut out = ints(&[0]);
    for t in tags {
        out.extend(t);
    }
    out.extend_from_slice(&p.none());
    out
}

/// `ToyPawns`: the pool data, the AI class, the toy pawn archetypes and the swing's definitions (invented values: a
/// health pool of 40.5 with a minimum of -10, a second definition of the same resource, two ammo definitions; the
/// melee damage 40 x 1.5, the thrown rock 10 x 1.5 scaled by 2.0; the damage type's flesh modifier 0.5).
pub fn toy_pawns() -> Vec<u8> {
    let mut p = PackageBuilder::new_empty();
    let core = p.add_import("Core", "Package", 0, "Core");
    let engine = p.add_import("Core", "Package", 0, "Engine");
    let willow = p.add_import("Core", "Package", 0, "WillowGame");
    let resource_class = p.add_import("Core", "Class", engine, "ResourceDefinition");
    let definition_class = p.add_import("Core", "Class", engine, "ResourcePoolDefinition");
    let pool_class = p.add_import("Core", "Class", engine, "ResourcePool");
    let resource = |p: &mut PackageBuilder, name: &str| {
        let tag = p.tag(
            "DefaultResourcePoolClass",
            "ClassProperty",
            &ints(&[pool_class]),
        );
        let payload = object_payload(p, vec![tag]);
        p.add_export(resource_class, name, payload)
    };
    let health = resource(&mut p, "Health");
    let ammo = resource(&mut p, "Ammo");
    let pool_definition = |p: &mut PackageBuilder, name: &str, r: i32, start: f32, min: f32| {
        let tags = vec![
            p.object_tag("Resource", r),
            p.float_tag("StartingValue", start),
            p.float_tag("BaseMinValue", min),
        ];
        let payload = object_payload(p, tags);
        p.add_export(definition_class, name, payload)
    };
    let health_definition = pool_definition(&mut p, "Pool_Health", health, 40.5, -10.0);
    pool_definition(&mut p, "Pool_Health_Copy", health, 9.0, 0.0);
    pool_definition(&mut p, "Pool_Ammo", ammo, 7.0, 0.0);
    pool_definition(&mut p, "Pool_Ammo_Copy", ammo, 3.0, 0.0);
    let ai_class_class = p.add_import("Core", "Class", willow, "AIClassDefinition");
    let attribute_class = p.add_import("Core", "Class", engine, "AttributeDefinition");
    let multiplier = {
        let payload = object_payload(&mut p, Vec::new());
        p.add_export(attribute_class, "Attribute_ExperienceMultiplier", payload)
    };
    let mut entry = p.object_tag("Attribute", multiplier);
    entry.extend(data(&mut p, "BaseValue", 0.75, 1.0));
    entry.extend_from_slice(&p.none());
    let starting = p.array_tag("AttributeStartingValues", 1, &entry);
    let tags = vec![
        p.object_tag("HealthPoolDefinition", health_definition),
        p.object_tag("AIDef", health_definition),
        starting,
    ];
    let payload = object_payload(&mut p, tags);
    let ai_class = p.add_export(ai_class_class, "AIClass_Toy", payload);
    for (class, name) in [
        ("WillowAIPawn", "Pawn_Toy"),
        ("WillowDoomedPawn", "Pawn_Doomed"),
        ("WillowDownPawn", "Pawn_Down"),
        ("WillowArmoredPawn", "Pawn_Armored"),
    ] {
        let c = p.add_import("Core", "Class", willow, class);
        let mut tags = vec![
            p.object_tag("AIClass", ai_class),
            p.float_tag("HealthVar", 11.0),
            p.float_tag("HealthMaxVar", 12.0),
        ];
        if name == "Pawn_Armored" {
            tags.push(p.float_tag("IntrinsicArmor", 25.0));
        }
        let payload = object_payload(&mut p, tags);
        p.add_export(c, name, payload);
    }
    let package_class = p.add_import("Core", "Class", core, "Package");
    let type_class = p.add_import("Core", "Class", willow, "DamageTypeDefinition");
    let flesh = data(&mut p, "FleshDamageModifier", 0.5, 1.0);
    let payload = object_payload(&mut p, vec![flesh]);
    let type_definition = p.add_export(type_class, "DamageType_Toy", payload);
    let none = object_payload(&mut p, Vec::new());
    let group = p.add_export(package_class, "Projectile", none.clone());
    let mut attribute = Export::new(resource_class, "ProjectileDamage", none);
    attribute.outer = group;
    let attribute = p.add_export_with(attribute);
    let melee_source = p.add_import("Core", "Class", willow, "WillowDmgSource_Melee");
    let grenade_source = p.add_import("Core", "Class", willow, "WillowDmgSource_Grenade");
    let melee_class = p.add_import("Core", "Class", willow, "MeleeDefinition");
    let tags = vec![
        data(&mut p, "Damage", 40.0, 1.5),
        p.tag("DamageSource", "ClassProperty", &ints(&[melee_source])),
        p.object_tag("DamageTypeDefinition", type_definition),
    ];
    let payload = object_payload(&mut p, tags);
    p.add_export(melee_class, "Melee_Toy", payload);
    let throw_class = p.add_import(
        "Core",
        "Class",
        willow,
        "Behavior_AIThrowProjectileAtTarget",
    );
    let mut entry = p.object_tag("Attribute", attribute);
    entry.extend(data(&mut p, "BaseValue", 10.0, 1.5));
    entry.extend_from_slice(&p.none());
    let tags = vec![p.array_tag("ChildProjectileBaseValues", 1, &entry)];
    let payload = object_payload(&mut p, tags);
    p.add_export(throw_class, "Throw_Toy", payload);
    let cause_class = p.add_import("Core", "Class", willow, "Behavior_CauseDamage");
    let tags = vec![
        data(&mut p, "DamageFormula", 0.0, 2.0),
        p.tag("DamageSource", "ClassProperty", &ints(&[grenade_source])),
        p.object_tag("DamageTypeDefinition", type_definition),
    ];
    let payload = object_payload(&mut p, tags);
    p.add_export(cause_class, "Cause_Toy", payload);

    // A body class with a head region (bone Head, critical, x2.0, armor surface) and a default body region (x1.0).
    let region_class = p.add_import("Core", "Class", willow, "HitRegionDefinition");
    let body_class = p.add_import("Core", "Class", willow, "BodyClassDefinition");
    let response = |p: &mut PackageBuilder, percent: f32| {
        let mut body = p.float_tag("DamageModifierPercent", percent);
        body.extend_from_slice(&p.none());
        p.struct_tag("DefaultImpactResponse", "ImpactResponse", &body)
    };
    let head_bone = p.fname("Head", 0);
    let bones = p.array_tag("RegionBoneNames", 1, &head_bone);
    let tags = vec![
        bones,
        p.bool_tag("bCriticalHit", true),
        p.byte_tag("DefaultDamageSurfaceType", 2),
        response(&mut p, 2.0),
    ];
    let payload = object_payload(&mut p, tags);
    let head = p.add_export(region_class, "Region_Head", payload);
    let tags = vec![response(&mut p, 1.0)];
    let payload = object_payload(&mut p, tags);
    let body_region = p.add_export(region_class, "Region_Body", payload);
    let tags = vec![
        p.array_tag("HitRegionList", 1, &ints(&[head])),
        p.object_tag("DefaultHitRegion", body_region),
    ];
    let payload = object_payload(&mut p, tags);
    let body = p.add_export(body_class, "BodyClass_Toy", payload);
    let armored_class = p.add_import("Core", "Class", willow, "WillowArmoredPawn");
    let tags = vec![
        p.object_tag("AIClass", ai_class),
        p.float_tag("HealthVar", 11.0),
        p.float_tag("HealthMaxVar", 12.0),
        p.object_tag("BodyClass", body),
    ];
    let payload = object_payload(&mut p, tags);
    p.add_export(armored_class, "Pawn_Bodied", payload);

    // The resistance attribute a damage type names, and that type.
    let resist = {
        let payload = object_payload(&mut p, Vec::new());
        p.add_export(attribute_class, "ResistToy", payload)
    };
    let tags = vec![p.object_tag("CharacterDamageTypeModifierAttribute", resist)];
    let payload = object_payload(&mut p, tags);
    p.add_export(type_class, "DamageType_Resist", payload);

    // Allegiances: A (self 2, default 1, holds B an enemy), B (default 2), C (forces 0 on every other), D (default 1,
    // and A is a friend in its other-opinion list).
    let allegiance_class = p.add_import("Core", "Class", engine, "PawnAllegiance");
    let a = p.next_export();
    let b = a + 1;
    let entry = |p: &mut PackageBuilder, who: i32, opinion: u8| {
        let mut e = p.object_tag("Allegiance", who);
        e.extend(p.byte_tag("Opinion", opinion));
        e.extend_from_slice(&p.none());
        e
    };
    let e_ab = entry(&mut p, b, 0);
    let tags = vec![
        p.byte_tag("SelfOpinion", 2),
        p.byte_tag("DefaultOpinion", 1),
        p.array_tag("MyOpinions", 1, &e_ab),
    ];
    let payload = object_payload(&mut p, tags);
    assert_eq!(p.add_export(allegiance_class, "Allegiance_A", payload), a);
    let tags = vec![p.byte_tag("DefaultOpinion", 2)];
    let payload = object_payload(&mut p, tags);
    assert_eq!(p.add_export(allegiance_class, "Allegiance_B", payload), b);
    let tags = vec![
        p.bool_tag("bForceAllOtherOpinions", true),
        p.byte_tag("ForcedOtherOpinion", 0),
        p.byte_tag("DefaultOpinion", 1),
    ];
    let payload = object_payload(&mut p, tags);
    p.add_export(allegiance_class, "Allegiance_C", payload);
    let e_da = entry(&mut p, a, 2);
    let tags = vec![
        p.byte_tag("DefaultOpinion", 1),
        p.array_tag("OtherOpinions", 1, &e_da),
    ];
    let payload = object_payload(&mut p, tags);
    p.add_export(allegiance_class, "Allegiance_D", payload);

    // Expression trees over a pawn's Physics: Tree_And = (walking) AND (not none); Tree_Or = (none) OR (walking);
    // Tree_Xor names an operator the evaluator does not combine.
    let tree_class = p.add_import("Core", "Class", engine, "ExpressionTree");
    let gearbox = p.add_import("Core", "Package", 0, "GearboxFramework");
    let compound_class = p.add_import("Core", "Class", gearbox, "CompoundExpressionEvaluator");
    let physics_class = p.add_import("Core", "Class", gearbox, "PhysicsStateExpressionEvaluator");
    let state = |p: &mut PackageBuilder, name: &str, physics: u8, in_state: bool| {
        let tags = vec![
            p.byte_tag("PhysicsState", physics),
            p.bool_tag("bIsInState", in_state),
        ];
        let payload = object_payload(p, tags);
        p.add_export(physics_class, name, payload)
    };
    let walking = state(&mut p, "Phys_Walking", 1, true);
    let not_none = state(&mut p, "Phys_NotNone", 0, false);
    let none_state = state(&mut p, "Phys_None", 0, true);
    for (name, tree, op, x, y) in [
        ("Compound_And", "Tree_And", 0u8, walking, not_none),
        ("Compound_Or", "Tree_Or", 1u8, none_state, walking),
        ("Compound_Xor", "Tree_Xor", 2u8, walking, walking),
    ] {
        let tags = vec![
            p.byte_tag("Operator", op),
            p.object_tag("Expression1", x),
            p.object_tag("Expression2", y),
        ];
        let payload = object_payload(&mut p, tags);
        let compound = p.add_export(compound_class, name, payload);
        let tags = vec![p.object_tag("RootChild", compound)];
        let payload = object_payload(&mut p, tags);
        p.add_export(tree_class, tree, payload);
    }
    loot::toy_pawns_loot(&mut p, core, engine, willow, ai_class);
    p.build()
}

/// `Startup`: `GD_Globals.General.Globals` (the experience formula 20 x 1.5, the party weights (0, 1) and (0.25, 0.5),
/// the experience level table (1: 1.5 / 0.8, 2: 2.0 / 0.5) and the pipeline's player level table (1: 1.25 / 0.75)).
pub fn startup() -> Vec<u8> {
    let mut p = PackageBuilder::new_empty();
    let core = p.add_import("Core", "Package", 0, "Core");
    let willow = p.add_import("Core", "Package", 0, "WillowGame");
    let package_class = p.add_import("Core", "Class", core, "Package");
    let globals_class = p.add_import("Core", "Class", willow, "GlobalsDefinition");
    let none = object_payload(&mut p, Vec::new());
    let group = p.add_export(package_class, "GD_Globals", none.clone());
    let mut general = Export::new(package_class, "General", none);
    general.outer = group;
    let general = p.add_export_with(general);
    let formula = data(&mut p, "BaseEnemyExperienceFormula", 20.0, 1.5);
    let mut weights = Vec::new();
    for (players, bonus, share) in [(1, 0.0, 1.0), (2, 0.25, 0.5)] {
        weights.extend(p.int_tag("Players", players));
        weights.extend(p.float_tag("KillerExpBonus", bonus));
        weights.extend(p.float_tag("ExpWeight", share));
        weights.extend_from_slice(&p.none());
    }
    let mut scales = Vec::new();
    for (difference, higher, lower) in [(1, 1.5, 0.8), (2, 2.0, 0.5)] {
        scales.extend(p.int_tag("LevelDifference", difference));
        scales.extend(p.float_tag("HigherLevelEnemyExpScale", higher));
        scales.extend(p.float_tag("LowerLevelEnemyExpScale", lower));
        scales.extend_from_slice(&p.none());
    }
    let mut attacker = p.float_tag("HigherLevelAttackerDmgScale", 1.25);
    attacker.extend(p.float_tag("LowerLevelAttackerDmgScale", 0.75));
    attacker.extend_from_slice(&p.none());
    let ai_scale = data(&mut p, "GlobalAIDamageScale", 0.5, 1.0);
    let tags = vec![
        formula,
        ai_scale,
        p.array_tag("ExpAwardWeights", 2, &weights),
        p.array_tag("ExpScaleByLevelDifference", 2, &scales),
        p.array_tag("PlayerDamageScaleByLevelDifference", 1, &attacker),
    ];
    let payload = object_payload(&mut p, tags);
    let mut globals = Export::new(globals_class, "Globals", payload);
    globals.outer = general;
    p.add_export_with(globals);
    p.build()
}

/// `ToyInjured`, used only by the bleed-out tests (not one of [`files`], so the packages the C++ digests were taken
/// over are unchanged): `ToyInjuredPawn`, a `WillowPlayerPawn` with the fields the injured state reads and the five
/// script functions the bleed step calls, each an empty body the tests override (invented, lane W6-COMBAT-RUN2).
pub fn toy_injured() -> Vec<u8> {
    let mut w = Toy::new(&["Float", "Byte", "Int", "Bool", "Object"]);
    w.import_root();
    let willow = w.import_full("Package", 0, "WillowGame");
    let player_pawn = w.import_full("Class", willow, "WillowPlayerPawn");
    let pawn = w.cls("ToyInjuredPawn", player_pawn);
    for n in [
        "TotalBleedoutTime",
        "NumSuccessiveInjuries",
        "flInjuredTargetedTime",
        "LastInjuryTime",
    ] {
        w.prop("Float", pawn, n, 0, 0);
    }
    for n in ["InjuredState", "InjuredDeadState", "SecondWindReason"] {
        w.prop("Byte", pawn, n, 0, 0);
    }
    w.prop("Bool", pawn, "bInjuredDeadCameraActive", 0, 0);
    let empty = |_: &dyn Fn(&str) -> i32| {
        let mut a = Asm::new();
        a.stmt().return_nothing();
        a.end();
        Some(a)
    };
    for (name, params) in [
        ("SwitchToSidearm", vec![]),
        ("Tick", vec![P("Float", "DeltaTime", 0, 0)]),
        ("InjuredDeathAnimDone", vec![]),
        ("CalcInjuredDeadCamera", vec![P("Float", "fDeltaTime", 0, 0)]),
        ("ShowRespawnDialog", vec![]),
    ] {
        w.function(pawn, name, &params, None, SCRIPT, empty);
    }
    w.build()
}

/// `ToyStarting` (decision 2026-11-19-26), an extra package the starting-values tests add: a toy character class with
/// the three fields `Controller.ApplyCharacterClassStartingValues` reads, and two of it: `Class_Two` lists `ToyPawns`'
/// `Pool_Health` then `Pool_Ammo`, `Class_Twice` lists `Pool_Health` and `Pool_Health_Copy` (one resource twice), and
/// `Class_Accuracy` lists `Pool_Health` with `Pool_Health_Copy` as its accuracy pool (review r1 of #412).
pub fn toy_starting() -> Vec<u8> {
    let mut w = Toy::new(&["Object", "Array"]);
    w.import_root();
    let class = w.cls("ToyCharClass", 0);
    w.array_of(class, "ResourcePools", "Object", 0);
    w.prop("Object", class, "AccuracyPoolDefinition", 0, 0);
    w.prop("Object", class, "OffHandAccuracyPoolDefinition", 0, 0);
    let p = &mut w.t.p;
    let engine = p.add_import("Core", "Package", 0, "Engine");
    let pawns = p.add_import("Core", "Package", 0, "ToyPawns");
    let definition = |p: &mut PackageBuilder, name: &str| p.add_import("Engine", "ResourcePoolDefinition", pawns, name);
    let _ = engine;
    let health = definition(p, "Pool_Health");
    let ammo = definition(p, "Pool_Ammo");
    let copy = definition(p, "Pool_Health_Copy");
    for (name, list) in [("Class_Two", [health, ammo]), ("Class_Twice", [health, copy])] {
        let refs: Vec<u8> = list.iter().flat_map(|r| r.to_le_bytes()).collect();
        let tag = p.array_tag("ResourcePools", 2, &refs);
        let payload = object_payload(p, vec![tag]);
        p.add_export(class, name, payload);
    }
    // Class_Accuracy lists Pool_Health and names Pool_Health_Copy (the same resource) as its accuracy pool.
    let tags = vec![p.array_tag("ResourcePools", 1, &health.to_le_bytes()), p.object_tag("AccuracyPoolDefinition", copy)];
    let payload = object_payload(p, tags);
    p.add_export(class, "Class_Accuracy", payload);
    w.build()
}

/// `ToyPlayerStarting` (decision 2026-11-19-57), an extra package the player-controller starting-values tests add.
/// Invented throughout:
/// - `ToyPlayerClass` with the six definition fields the two natives read (the base's three, the override's three);
/// - `ToyPlayerController`, a `WillowPlayerController` with `PlayerClass`, the five pool reference fields and a native
///   `OnPlayerClassChange()` the test binds to count its calls;
/// - four resources (`Accuracy`, `Exp`, `Cooldown`, `Melee`) and one pool definition of each, plus `Pool_Exp_Health`,
///   an experience definition of `ToyPawns`' `Health` resource;
/// - `Class_Player` (`ToyPawns`' `Pool_Health` and `Pool_Ammo`, the accuracy pool, and the three player pools),
///   `Class_ExpOnly` (`Pool_Health`, and only the experience pool) and `Class_DupExp` (`Pool_Health`, and
///   `Pool_Exp_Health`: one resource twice across the base and the override).
pub fn toy_player_starting() -> Vec<u8> {
    let mut w = Toy::new(&["Object", "Array", "Struct"]);
    w.import_root();
    let engine = w.import_full("Package", 0, "Engine");
    let actor = w.import_full("Class", engine, "Actor");
    let reference = w.import_full("ScriptStruct", actor, "ResourcePoolReference");
    let willow = w.import_full("Package", 0, "WillowGame");
    let player_controller = w.import_full("Class", willow, "WillowPlayerController");
    let class = w.cls("ToyPlayerClass", 0);
    w.array_of(class, "ResourcePools", "Object", 0);
    for f in [
        "AccuracyPoolDefinition",
        "OffHandAccuracyPoolDefinition",
        "ExperiencePoolDefinition",
        "SkillCooldownPoolDefinition",
        "MeleeSkillCooldownPoolDefinition",
    ] {
        w.prop("Object", class, f, 0, 0);
    }
    let controller = w.cls("ToyPlayerController", player_controller);
    w.prop("Object", controller, "PlayerClass", 0, 0);
    for f in ["AccuracyPool", "OffHandAccuracyPool", "ExpPool", "SkillCooldownPool", "MeleeSkillCooldownPool"] {
        w.prop("Struct", controller, f, 0, reference);
    }
    w.native(controller, "OnPlayerClassChange", &[], None);
    let p = &mut w.t.p;
    let resource_class = p.add_import("Core", "Class", engine, "ResourceDefinition");
    let definition_class = p.add_import("Core", "Class", engine, "ResourcePoolDefinition");
    let pool_class = p.add_import("Core", "Class", engine, "ResourcePool");
    let pawns = p.add_import("Core", "Package", 0, "ToyPawns");
    let pool_health = p.add_import("Engine", "ResourcePoolDefinition", pawns, "Pool_Health");
    let pool_ammo = p.add_import("Engine", "ResourcePoolDefinition", pawns, "Pool_Ammo");
    let health = p.add_import("Engine", "ResourceDefinition", pawns, "Health");
    let resource = |p: &mut PackageBuilder, name: &str| {
        let tag = p.tag("DefaultResourcePoolClass", "ClassProperty", &ints(&[pool_class]));
        let payload = object_payload(p, vec![tag]);
        p.add_export(resource_class, name, payload)
    };
    let accuracy = resource(p, "Accuracy");
    let exp = resource(p, "Exp");
    let cooldown = resource(p, "Cooldown");
    let melee = resource(p, "Melee");
    let definition = |p: &mut PackageBuilder, name: &str, r: i32, start: f32| {
        let tags = vec![p.object_tag("Resource", r), p.float_tag("StartingValue", start)];
        let payload = object_payload(p, tags);
        p.add_export(definition_class, name, payload)
    };
    let pool_accuracy = definition(p, "Pool_Accuracy", accuracy, 1.0);
    let pool_exp = definition(p, "Pool_Exp", exp, 2.0);
    let pool_cooldown = definition(p, "Pool_Cooldown", cooldown, 3.0);
    let pool_melee = definition(p, "Pool_Melee", melee, 4.0);
    let pool_exp_health = definition(p, "Pool_Exp_Health", health, 5.0);
    let list = |p: &mut PackageBuilder, pools: &[i32]| {
        let refs: Vec<u8> = pools.iter().flat_map(|r| r.to_le_bytes()).collect();
        p.array_tag("ResourcePools", i32::try_from(pools.len()).expect("fits"), &refs)
    };
    let tags = vec![
        list(p, &[pool_health, pool_ammo]),
        p.object_tag("AccuracyPoolDefinition", pool_accuracy),
        p.object_tag("ExperiencePoolDefinition", pool_exp),
        p.object_tag("SkillCooldownPoolDefinition", pool_cooldown),
        p.object_tag("MeleeSkillCooldownPoolDefinition", pool_melee),
    ];
    let payload = object_payload(p, tags);
    p.add_export(class, "Class_Player", payload);
    let tags = vec![list(p, &[pool_health]), p.object_tag("ExperiencePoolDefinition", pool_exp)];
    let payload = object_payload(p, tags);
    p.add_export(class, "Class_ExpOnly", payload);
    let tags = vec![list(p, &[pool_health]), p.object_tag("ExperiencePoolDefinition", pool_exp_health)];
    let payload = object_payload(p, tags);
    p.add_export(class, "Class_DupExp", payload);
    w.build()
}

/// `ToyRoute` (decision 2026-11-19-32), an extra package the route-native tests add: `ToyRoutePawn` (a `WillowAIPawn`
/// with `bIsDead` and an `SMComponent`), `ToySMComponent`, whose `CurrentSMData` struct holds `SMD`, and `ToyOtherMove`,
/// a special move that is not the spawn move.
pub fn toy_route() -> Vec<u8> {
    let mut w = Toy::new(&["Object", "Bool", "Struct"]);
    w.import_root();
    let willow = w.import_full("Package", 0, "WillowGame");
    let ai_pawn = w.import_full("Class", willow, "WillowAIPawn");
    let component = w.cls("ToySMComponent", 0);
    let data = w.strukt(component, "ToySMData");
    w.prop("Object", data, "SMD", 0, 0);
    w.prop("Struct", component, "CurrentSMData", 0, data);
    let pawn = w.cls("ToyRoutePawn", ai_pawn);
    w.prop("Bool", pawn, "bIsDead", 0, 0);
    w.prop("Object", pawn, "SMComponent", 0, component);
    // A special move that is not `SpecialMove_Spawned` (the gate's toy `SpecialMove_Spawned` is `willow_game()`'s).
    w.cls("ToyOtherMove", 0);
    w.build()
}

/// Every file of the fixture, by file name.
pub fn files() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("Engine.upk", engine()),
        ("GearboxFramework.upk", gearbox()),
        ("WillowGame.upk", willow_game()),
        ("ToyPawns.upk", toy_pawns()),
        ("Startup.upk", startup()),
    ]
}
