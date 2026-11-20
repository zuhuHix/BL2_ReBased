//! Tests of the pool natives, the manager, pool references, the pawn's health glue, the engine query and the level-up
//! rebase, on an invented `Engine.upk` (AGENTS.md rule 9: invented classes, values and script bodies). The natives
//! are declared in the package as the record gives their signatures, so every call goes through the key the loader
//! derives. The pool's script functions are invented bodies that call `Object.Note(int)`, which records
//! (object, number), and set the fields the natives read; they are not the game's script.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_attr::api::{AttrWorld, EngineQuery};
use ow_balance::types::{
    AwesomeLevel, BalanceSnapshot, ExpLevel, GameStage, LevelCapUpgrades, OpLevel, PlayerCount,
    Playthrough,
};
use ow_core::ParityMode;
use ow_core::notyet::Missing;
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, StructId};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_package::testkit::reflect::{property_payload, toy_array_of, toy_struct};
use ow_package::testkit::script::{Asm, CPF_OPT, CPF_OUT, FunctionSpec, Param, ScriptToy};
use ow_package::testkit::{Export, ints};
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;
use ow_vm::natives_core;

use super::*;
use crate::actors::{field, set_field};
use crate::levelup::{self, Recalculated};
use crate::world::{BalanceState, World};

const SCRIPT: u32 = 0x1 | 0x2 | 0x20000;
const NATIVE: u32 = 0x400 | 0x20000;
const STATIC: u32 = 0x2000;

// Opcodes the invented bodies use (UE3 bytecode, as `ow-script` decodes them).
const EX_LET: u8 = 0x0F;
const EX_NO_OBJECT: u8 = 0x2A;

// ---------------------------------------------------------------------------------------------------------------
// The invented package
// ---------------------------------------------------------------------------------------------------------------

fn native(
    t: &mut ScriptToy,
    owner: i32,
    name: &str,
    params: &[Param],
    result: Option<(&str, i32)>,
    flags: u32,
) {
    let spec = FunctionSpec::new(flags, name);
    t.function(owner, name, params, result, &[], &spec, |_| None);
}

fn p(kind: &str, name: &str, type_ref: i32) -> Param {
    Param {
        type_ref,
        ..Param::new(kind, name)
    }
}

fn out(kind: &str, name: &str, type_ref: i32) -> Param {
    Param {
        flags: CPF_OUT,
        type_ref,
        ..Param::new(kind, name)
    }
}

fn opt(kind: &str, name: &str, type_ref: i32) -> Param {
    Param {
        flags: CPF_OPT,
        type_ref,
        ..Param::new(kind, name)
    }
}

/// A static-array property of `dim` elements.
fn prop_dim(t: &mut ScriptToy, kind: &str, owner: i32, name: &str, dim: u32, type_ref: i32) {
    let payload = property_payload(&t.none, dim, 0, &ints(&[type_ref]));
    let mut e = Export::new(t.import(&format!("{kind}Property")), name, payload);
    e.outer = owner;
    t.p.add_export_with(e);
}

/// `Note(k)` as a statement.
fn note(a: &mut Asm, note_fn: i32, k: i32) {
    a.stmt().final_function(note_fn, |a| {
        a.int_const(k);
    });
}

fn package() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&[
        "Class",
        "Function",
        "State",
        "ScriptStruct",
        "Enum",
        "IntProperty",
        "FloatProperty",
        "BoolProperty",
        "NameProperty",
        "ByteProperty",
        "ObjectProperty",
        "ClassProperty",
        "ArrayProperty",
        "StructProperty",
    ]);
    let object = t.class("Object", 0);
    let note_fn = {
        let spec = FunctionSpec::new(NATIVE, "Note");
        t.function(
            object,
            "Note",
            &[Param::new("Int", "N")],
            None,
            &[],
            &spec,
            |_| None,
        )
        .function
    };
    let vector = toy_struct(&mut t, object, "Vector", None);
    for f in ["X", "Y", "Z"] {
        t.prop("Float", vector, f, 0, 0);
    }
    let rotator = toy_struct(&mut t, object, "Rotator", None);
    for f in ["Pitch", "Yaw", "Roll"] {
        t.prop("Int", rotator, f, 0, 0);
    }

    // Actor: the fields the spawn and tick rules read (as the W5-WORLD fixture declares them).
    let actor = t.class("Actor", object);
    let mut payload = ints(&[0]);
    payload.extend_from_slice(&t.none);
    let values = ["PHYS_None", "PHYS_Walking"];
    payload.extend(ints(&[0, values.len() as i32]));
    for v in values {
        let bytes = t.p.fname(v, 0);
        payload.extend_from_slice(&bytes);
    }
    let mut e = Export::new(t.import("Enum"), "EPhysics", payload);
    e.outer = actor;
    let physics = t.p.add_export_with(e);
    for b in [
        "bStatic",
        "bNoDelete",
        "bDeleteMe",
        "bPendingDelete",
        "bCollideWorld",
        "bCollideWhenPlacing",
        "bCollideActors",
        "bNetTemporary",
        "bShouldBaseAtStartup",
    ] {
        t.prop("Bool", actor, b, 0, 0);
    }
    for b in ["Role", "RemoteRole", "TickGroup"] {
        t.prop("Byte", actor, b, 0, 0);
    }
    t.prop("Byte", actor, "Physics", 0, physics);
    for f in ["LifeSpan", "CreationTime"] {
        t.prop("Float", actor, f, 0, 0);
    }
    for o in ["Owner", "Base", "Instigator", "WorldInfo"] {
        t.prop("Object", actor, o, 0, actor);
    }
    t.prop("Struct", actor, "Location", 0, vector);
    t.prop("Struct", actor, "Rotation", 0, rotator);
    for a in ["Attached", "Touching", "GeneratedEvents", "Components", "AllComponents"] {
        toy_array_of(&mut t, actor, a, "Object", object);
    }
    let reference = toy_struct(&mut t, actor, "ResourcePoolReference", None);
    t.prop("Object", reference, "PoolManager", 0, object);
    t.prop("Byte", reference, "PoolIndexInManager", 0, 0);
    t.prop("Byte", reference, "PoolGUID", 0, 0);
    t.prop("Object", reference, "Data", 0, object);
    native(
        &mut t,
        actor,
        "IsResourcePoolValid",
        &[out("Struct", "PoolReference", reference)],
        Some(("Bool", 0)),
        NATIVE,
    );
    native(
        &mut t,
        actor,
        "GetResourcePoolCurrentValueOrDefault",
        &[
            out("Struct", "PoolReference", reference),
            Param::new("Float", "DefaultValue"),
        ],
        Some(("Float", 0)),
        NATIVE,
    );

    let world_info = t.class("WorldInfo", actor);
    for f in ["TimeSeconds", "RealTimeSeconds", "TimeDilation"] {
        t.prop("Float", world_info, f, 0, 0);
    }
    t.prop("Byte", world_info, "NetMode", 0, 0);

    // Definitions and resolvers.
    let pool_def = t.class("ResourcePoolDefinition", object);
    let resource = t.class("ResourceDefinition", object);
    t.prop("Object", pool_def, "Resource", 0, resource);
    t.prop_tail(
        "Class",
        resource,
        "DefaultResourcePoolClass",
        0,
        &ints(&[t.import("Class"), object]),
    );
    t.prop(
        "Object",
        resource,
        "DefaultResourcePoolDefinition",
        0,
        pool_def,
    );
    t.prop("Bool", resource, "bIntegerOnlyUpdates", 0, 0);
    t.prop("Object", resource, "ResourceContextResolver", 0, object);
    t.class("ControllerAttributeContextResolver", object);
    t.class("PawnAttributeContextResolver", object);
    let class_def = t.class("CharacterClassDefinition", object);
    t.class("WillowCharacterClassDefinition", class_def);

    // The pool, its natives and invented script bodies.
    let pool = t.class("ResourcePool", object);
    t.prop("Byte", pool, "PoolGUID", 0, 0);
    let definition = t.prop("Object", pool, "Definition", 0, pool_def);
    let provider = t.prop("Object", pool, "AssociatedProvider", 0, object);
    for b in [
        "bIsAuthoritative",
        "bCreatedAndNotModified",
        "bHasPoolBeenFullSinceLastBeingDepleted",
    ] {
        t.prop("Bool", pool, b, 0, 0);
    }
    for f in [
        "CurrentValue",
        "MinValue",
        "MaxValue",
        "MaxValueBaseValue",
        "RateRemainder",
        "PoolIdleDelayStartTime",
        "LastCurrentValue",
        "LastMinValue",
        "LastMaxValue",
    ] {
        t.prop("Float", pool, f, 0, 0);
    }
    for s in CLEARED_STACKS {
        toy_array_of(&mut t, pool, &format!("{s}ModifierStack"), "Object", object);
    }
    native(
        &mut t,
        pool,
        "GetMaxValue",
        &[opt("Bool", "bBase", 0)],
        Some(("Float", 0)),
        NATIVE,
    );
    native(
        &mut t,
        pool,
        "GetCurrentValue",
        &[],
        Some(("Float", 0)),
        NATIVE,
    );
    native(
        &mut t,
        pool,
        "SetCurrentValue",
        &[Param::new("Float", "Value")],
        None,
        NATIVE,
    );
    for n in [
        "UpdateLastValues",
        "PoolIsNowFull",
        "ClearAttributeModifierStacks",
    ] {
        native(&mut t, pool, n, &[], None, NATIVE);
    }
    let reset_idle = {
        let spec = FunctionSpec::new(NATIVE, "ResetIdleTimer");
        t.function(pool, "ResetIdleTimer", &[], None, &[], &spec, |_| None)
            .function
    };
    // InitializeAuthoritative(D, P): Definition = D; AssociatedProvider = P; Note(10).
    let spec = FunctionSpec::new(SCRIPT, "InitializeAuthoritative");
    t.function(
        pool,
        "InitializeAuthoritative",
        &[p("Object", "D", pool_def), p("Object", "P", object)],
        None,
        &[],
        &spec,
        |ids| {
            let mut a = Asm::new();
            a.stmt()
                .raw(&[EX_LET])
                .instance(definition)
                .local(ids.get("D"));
            a.stmt()
                .raw(&[EX_LET])
                .instance(provider)
                .local(ids.get("P"));
            note(&mut a, note_fn, 10);
            a.stmt().return_nothing();
            a.end();
            Some(a)
        },
    );
    // CalculateBaseValues(optional B): Note(20), then Note(21) when B.
    let spec = FunctionSpec::new(SCRIPT, "CalculateBaseValues");
    t.function(
        pool,
        "CalculateBaseValues",
        &[opt("Bool", "B", 0)],
        None,
        &[],
        &spec,
        |ids| {
            let mut a = Asm::new();
            note(&mut a, note_fn, 20);
            let skip = a.stmt().jump_if_not(|a| {
                a.local(ids.get("B"));
            });
            note(&mut a, note_fn, 21);
            a.patch(skip);
            a.stmt().return_nothing();
            a.end();
            Some(a)
        },
    );
    // ShutDown(): Definition = None; Note(30).
    let spec = FunctionSpec::new(SCRIPT, "ShutDown");
    t.function(pool, "ShutDown", &[], None, &[], &spec, |_| {
        let mut a = Asm::new();
        a.stmt()
            .raw(&[EX_LET])
            .instance(definition)
            .raw(&[EX_NO_OBJECT]);
        note(&mut a, note_fn, 30);
        a.stmt().return_nothing();
        a.end();
        Some(a)
    });
    // Reinitialize(): Note(40).
    let spec = FunctionSpec::new(SCRIPT, "Reinitialize");
    t.function(pool, "Reinitialize", &[], None, &[], &spec, |_| {
        let mut a = Asm::new();
        note(&mut a, note_fn, 40);
        a.stmt().return_nothing();
        a.end();
        Some(a)
    });
    t.class("ExperienceResourcePool", pool);
    t.class("AmmoResourcePool", pool);
    t.class("HealthResourcePool", pool);
    t.class("ShieldResourcePool", pool);
    t.class("OddResourcePool", pool);
    // A pool whose InitializeAuthoritative calls ResetIdleTimer, as every installed one does: Definition = D;
    // ResetIdleTimer(); Note(11).
    // A pool whose InitializeAuthoritative sets Definition, then calls a native nothing binds: it refuses in both
    // modes (review r2 of #444).
    let refusing = t.class("RefusingPool", pool);
    let unbound = t
        .function(refusing, "Unbound", &[], None, &[], &FunctionSpec::new(NATIVE, "Unbound"), |_| None)
        .function;
    t.function(
        refusing,
        "InitializeAuthoritative",
        &[p("Object", "D", pool_def), p("Object", "P", object)],
        None,
        &[],
        &FunctionSpec::new(SCRIPT, "InitializeAuthoritative"),
        |ids| {
            let mut a = Asm::new();
            a.stmt().raw(&[EX_LET]).instance(definition).local(ids.get("D"));
            a.stmt().final_function(unbound, |_| {});
            a.stmt().return_nothing();
            a.end();
            Some(a)
        },
    );
    let timed = t.class("TimedPool", pool);
    let spec = FunctionSpec::new(SCRIPT, "InitializeAuthoritative");
    t.function(
        timed,
        "InitializeAuthoritative",
        &[p("Object", "D", pool_def), p("Object", "P", object)],
        None,
        &[],
        &spec,
        |ids| {
            let mut a = Asm::new();
            a.stmt()
                .raw(&[EX_LET])
                .instance(definition)
                .local(ids.get("D"));
            a.stmt().final_function(reset_idle, |_| {});
            note(&mut a, note_fn, 11);
            a.stmt().return_nothing();
            a.end();
            Some(a)
        },
    );

    // The manager.
    let manager = t.class("ResourcePoolManager", actor);
    prop_dim(&mut t, "Object", manager, "ResourcePools", 16, pool);
    t.prop("Byte", manager, "NextPoolGUID", 0, 0);
    native(
        &mut t,
        manager,
        "CreatePool",
        &[
            out("Object", "Mgr", manager),
            p("Object", "ActorOwner", actor),
            p("Object", "PoolDefinition", pool_def),
            opt("Bool", "bReinitializeExistingPool", 0),
        ],
        Some(("Struct", reference)),
        NATIVE | STATIC,
    );
    native(
        &mut t,
        manager,
        "CreatePoolAtIndex",
        &[
            out("Object", "Mgr", manager),
            p("Object", "ActorOwner", actor),
            p("Object", "PoolDefinition", pool_def),
            Param::new("Int", "Index"),
            Param::new("Byte", "PoolGUID"),
        ],
        Some(("Object", pool)),
        NATIVE | STATIC,
    );
    native(
        &mut t,
        manager,
        "InternalCreatePoolAtIndex",
        &[
            p("Object", "PoolDefinition", pool_def),
            Param::new("Int", "Index"),
            Param::new("Byte", "PoolGUID"),
        ],
        Some(("Object", pool)),
        NATIVE,
    );
    native(
        &mut t,
        manager,
        "DeletePoolByReference",
        &[out("Struct", "PoolReference", reference)],
        None,
        NATIVE | STATIC,
    );
    native(
        &mut t,
        manager,
        "DeletePool",
        &[
            p("Object", "Mgr", manager),
            p("Struct", "PoolReference", reference),
        ],
        None,
        NATIVE | STATIC,
    );
    native(
        &mut t,
        manager,
        "DeletePoolAtIndex",
        &[Param::new("Int", "Index")],
        None,
        NATIVE,
    );
    native(
        &mut t,
        manager,
        "DeleteAllPools",
        &[p("Object", "Mgr", manager)],
        None,
        NATIVE | STATIC,
    );
    native(
        &mut t,
        manager,
        "HasResourcePoolForResourceDefinition",
        &[p("Object", "Resource", resource)],
        Some(("Bool", 0)),
        NATIVE,
    );
    native(
        &mut t,
        manager,
        "GetResourcePoolForResourceDefinition",
        &[
            out("Object", "Mgr", manager),
            p("Object", "ActorOwner", actor),
            p("Object", "Resource", resource),
        ],
        Some(("Struct", reference)),
        NATIVE | STATIC,
    );
    native(
        &mut t,
        manager,
        "CreateResourcePoolManagerIfNecessary",
        &[
            out("Object", "Mgr", manager),
            p("Object", "ActorOwner", actor),
        ],
        None,
        NATIVE | STATIC,
    );
    native(
        &mut t,
        manager,
        "RecalculateBaseValues",
        &[p("Object", "Mgr", manager)],
        None,
        NATIVE | STATIC,
    );

    // Pawn and controllers.
    let pawn = t.class("Pawn", actor);
    let controller = t.class("Controller", actor);
    t.prop("Object", pawn, "Controller", 0, controller);
    t.prop("Struct", pawn, "HealthPool", 0, reference);
    t.prop("Float", pawn, "HealthVar", 0, 0);
    t.prop("Float", pawn, "HealthMaxVar", 0, 0);
    t.prop("Object", pawn, "StatusEffectComp", 0, object);
    native(
        &mut t,
        pawn,
        "SetHealthResourcePoolDefinition",
        &[
            out("Object", "ResourcePoolManager", manager),
            p("Object", "Definition", pool_def),
        ],
        None,
        NATIVE,
    );
    native(&mut t, pawn, "GetHealth", &[], Some(("Float", 0)), NATIVE);
    native(
        &mut t,
        pawn,
        "SetHealth",
        &[Param::new("Float", "NewHealth")],
        None,
        NATIVE,
    );
    native(
        &mut t,
        pawn,
        "GetMaxHealth",
        &[],
        Some(("Float", 0)),
        NATIVE,
    );
    native(
        &mut t,
        pawn,
        "SetMaxHealth",
        &[Param::new("Float", "NewMaxHealth")],
        None,
        NATIVE,
    );
    t.class("Vehicle", pawn);
    t.prop("Object", controller, "Pawn", 0, pawn);
    t.prop("Object", controller, "ResourcePoolManager", 0, manager);
    t.prop("Object", controller, "CharacterClass", 0, class_def);
    native(
        &mut t,
        controller,
        "GetResourcePoolForResourceDefinition",
        &[
            p("Object", "Resource", resource),
            opt("Bool", "bCreatePoolIfOneDoesntExist", 0),
        ],
        Some(("Struct", reference)),
        NATIVE,
    );
    native(
        &mut t,
        controller,
        "RecalculateAttributeInitializedState",
        &[],
        None,
        NATIVE,
    );
    let player = t.class("WillowPlayerController", controller);
    t.prop("Object", player, "MyWillowPawn", 0, pawn);
    native(
        &mut t,
        player,
        "RecalculateAttributeInitializedState",
        &[],
        None,
        NATIVE,
    );
    // The mind's two fields its RecalculateAttributeInitializedState override reads (NATIVE_MIND_ROUTE.md section 3).
    let mind = t.class("WillowMind", controller);
    t.prop("Object", mind, "AIClass", 0, class_def);
    t.prop("Object", mind, "MyWillowPawn", 0, pawn);
    // A controller-like class without CharacterClass, Pawn or MyWillowPawn, as the C++ fixtures declare it.
    let bare = t.class("BareController", actor);
    t.prop("Object", bare, "ResourcePoolManager", 0, manager);
    t.class("Thing", object);
    t.p.build()
}

// ---------------------------------------------------------------------------------------------------------------
// The fixture
// ---------------------------------------------------------------------------------------------------------------

static NEXT: AtomicU32 = AtomicU32::new(0);

fn snapshot() -> BalanceSnapshot {
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

struct Fx {
    w: World,
    wi: ObjRef,
    notes: Rc<RefCell<Vec<(ObjRef, i32)>>>,
    dir: PathBuf,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fx() -> Fx {
    fx_mode(ParityMode::Off)
}

fn fx_mode(mode: ParityMode) -> Fx {
    let dir = std::env::temp_dir().join(format!(
        "ow-world-pools-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let bytes = package();
    let defs = DefStore::from_synthetic(
        &dir,
        &[("Engine.upk", &bytes)],
        IdentityPolicy::FirstInScanOrder,
        mode,
    )
    .unwrap();
    let mut vm = Vm::new(
        ObjectSpace::new(),
        defs,
        Rc::new(AttributeSystem::new_empty()),
        mode,
    );
    let notes = Rc::new(RefCell::new(Vec::new()));
    let sink = notes.clone();
    vm.register_native(
        "Object.Note(int)",
        Rc::new(move |c: &mut NativeCall<'_>| {
            let k = match c.arg(0)? {
                Value::Int(k) => *k,
                other => panic!("Note got {other:?}"),
            };
            sink.borrow_mut()
                .push((c.this.expect("Note on an object"), k));
            Ok(Value::None)
        }),
    )
    .unwrap();
    let mut w = World::new(vm, BalanceState::new(snapshot()), &[&natives_core::Pack]).unwrap();
    for class in [
        "Actor",
        "WorldInfo",
        "Pawn",
        "Vehicle",
        "Controller",
        "WillowPlayerController",
        "WillowMind",
        "ResourcePoolManager",
    ] {
        let cdo =
            w.vm.defs
                .class_default(&mut w.vm.space, &format!("Engine.{class}"))
                .unwrap();
        set_field(&mut w.vm.space, cdo, "Role", Value::Byte(3)).unwrap();
    }
    let c = w.vm.find_class("Engine.WorldInfo").unwrap();
    let wi = ow_vm::interp::instantiate_from_defaults(&mut w.vm, c).unwrap();
    set_field(&mut w.vm.space, wi, "TimeDilation", Value::Float(1.0)).unwrap();
    set_field(&mut w.vm.space, wi, "TimeSeconds", Value::Float(12.5)).unwrap();
    w.actors().borrow_mut().place_world_info(wi).unwrap();
    w.actors().borrow_mut().begin_play();
    Fx { w, wi, notes, dir }
}

impl Fx {
    fn vm(&mut self) -> &mut Vm {
        &mut self.w.vm
    }

    fn new_obj(&mut self, class: &str) -> ObjRef {
        let c = self.vm().find_class(&format!("Engine.{class}")).unwrap();
        ow_vm::interp::instantiate_from_defaults(self.vm(), c).unwrap()
    }

    fn get(&self, o: ObjRef, name: &str) -> Value {
        field(&self.w.vm.space, o, name).unwrap()
    }

    fn f(&self, o: ObjRef, name: &str) -> f32 {
        match self.get(o, name) {
            Value::Float(f) => f,
            other => panic!("{name} is {other:?}"),
        }
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        set_field(&mut self.vm().space, o, name, v).unwrap();
    }

    fn setf(&mut self, o: ObjRef, name: &str, v: f32) {
        self.set(o, name, Value::Float(v));
    }

    fn call(
        &mut self,
        function: &str,
        this: Option<ObjRef>,
        args: Vec<Value>,
    ) -> Result<ow_vm::api::CallResult, VmError> {
        let f = self.vm().find_function(&format!("Engine.{function}"))?;
        self.vm().call(f, this, args)
    }

    fn ret(&mut self, function: &str, this: Option<ObjRef>, args: Vec<Value>) -> Value {
        self.call(function, this, args).unwrap().ret
    }

    fn noted(&self, o: ObjRef) -> Vec<i32> {
        self.notes
            .borrow()
            .iter()
            .filter(|(x, _)| *x == o)
            .map(|&(_, k)| k)
            .collect()
    }

    fn ref_ty(&self) -> StructId {
        self.w
            .vm
            .space
            .classes
            .struct_by_path("Engine.Actor.ResourcePoolReference")
            .unwrap()
    }

    fn ref_value(&self, r: PoolRef) -> Value {
        ref_value(&self.w.vm.space, self.ref_ty(), r).unwrap()
    }

    fn read(&self, v: &Value) -> PoolRef {
        read_ref(&self.w.vm.space, v).unwrap()
    }

    /// A resource whose pools are `ResourcePool` (or `class`), and a pool definition for it.
    fn resource_with(&mut self, class: &str) -> (ObjRef, ObjRef) {
        let resource = self.new_obj("ResourceDefinition");
        let c = self.vm().find_class(&format!("Engine.{class}")).unwrap();
        self.set(resource, "DefaultResourcePoolClass", Value::Class(Some(c)));
        let def = self.new_obj("ResourcePoolDefinition");
        self.set(def, "Resource", Value::Object(Some(resource)));
        self.set(
            resource,
            "DefaultResourcePoolDefinition",
            Value::Object(Some(def)),
        );
        (resource, def)
    }

    fn resource(&mut self) -> (ObjRef, ObjRef) {
        self.resource_with("ResourcePool")
    }

    /// A controller with authority and an empty manager it owns.
    fn controller_with_manager(&mut self, class: &str) -> (ObjRef, ObjRef) {
        let controller = self.new_obj(class);
        let m = self.new_obj("ResourcePoolManager");
        self.set(m, "Owner", Value::Object(Some(controller)));
        self.set(controller, "ResourcePoolManager", Value::Object(Some(m)));
        (controller, m)
    }

    /// `CreatePool(m, owner, def, reinit)`: the reference and the manager written back.
    fn create(
        &mut self,
        m: Option<ObjRef>,
        owner: Option<ObjRef>,
        def: Option<ObjRef>,
        reinit: bool,
    ) -> (PoolRef, Option<ObjRef>) {
        let r = self
            .call(
                "ResourcePoolManager.CreatePool",
                None,
                vec![
                    Value::Object(m),
                    Value::Object(owner),
                    Value::Object(def),
                    Value::Bool(reinit),
                ],
            )
            .unwrap();
        let mgr = match &r.outs[0] {
            Value::Object(o) => *o,
            other => panic!("Mgr {other:?}"),
        };
        (self.read(&r.ret), mgr)
    }

    /// A pool made directly (authoritative, with the given definition), for the pool-native tests.
    fn pool(&mut self, class: &str) -> ObjRef {
        let o = self.new_obj(class);
        self.set(o, "bIsAuthoritative", Value::Bool(true));
        o
    }
}

fn run(f: impl FnOnce() + Send) {
    ow_vm::interp::on_vm_stack(f).unwrap();
}

fn vm_missing(e: VmError) -> Missing {
    match e {
        VmError::NotYet(n) => n.missing,
        other => panic!("expected NotYet, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The pool natives
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn get_max_value_gives_the_base_companion_or_the_effective_maximum() {
    run(|| {
        let mut x = fx();
        let pool = x.pool("ResourcePool");
        x.setf(pool, "MaxValueBaseValue", 80.0);
        x.setf(pool, "MaxValue", 96.0);
        x.setf(pool, "CurrentValue", 33.0);
        assert_eq!(
            x.ret(
                "ResourcePool.GetMaxValue",
                Some(pool),
                vec![Value::Bool(true)]
            ),
            Value::Float(80.0)
        );
        assert_eq!(
            x.ret(
                "ResourcePool.GetMaxValue",
                Some(pool),
                vec![Value::Bool(false)]
            ),
            Value::Float(96.0)
        );
        assert_eq!(
            x.ret("ResourcePool.GetMaxValue", Some(pool), vec![]),
            Value::Float(96.0),
            "omitted: effective"
        );
        assert_eq!(
            x.ret("ResourcePool.GetCurrentValue", Some(pool), vec![]),
            Value::Float(33.0)
        );
    });
}

#[test]
fn set_current_value_clamps_minimum_first_and_stamps_the_world_time() {
    run(|| {
        let mut x = fx();
        let pool = x.pool("ResourcePool");
        x.setf(pool, "MinValue", 0.0);
        x.setf(pool, "MaxValue", 50.0);
        x.ret(
            "ResourcePool.SetCurrentValue",
            Some(pool),
            vec![Value::Float(70.0)],
        );
        assert_eq!(x.f(pool, "CurrentValue"), 50.0);
        assert_eq!(
            x.f(pool, "PoolIdleDelayStartTime"),
            12.5,
            "the world's TimeSeconds"
        );
        x.ret(
            "ResourcePool.SetCurrentValue",
            Some(pool),
            vec![Value::Float(-4.0)],
        );
        assert_eq!(x.f(pool, "CurrentValue"), 0.0);
        x.ret(
            "ResourcePool.SetCurrentValue",
            Some(pool),
            vec![Value::Float(17.25)],
        );
        assert_eq!(x.f(pool, "CurrentValue"), 17.25);
        // A minimum above the maximum wins.
        x.setf(pool, "MinValue", 60.0);
        x.ret(
            "ResourcePool.SetCurrentValue",
            Some(pool),
            vec![Value::Float(10.0)],
        );
        assert_eq!(x.f(pool, "CurrentValue"), 60.0);
        // Not authoritative: nothing at all.
        x.set(pool, "bIsAuthoritative", Value::Bool(false));
        x.setf(pool, "PoolIdleDelayStartTime", 1.0);
        x.ret(
            "ResourcePool.SetCurrentValue",
            Some(pool),
            vec![Value::Float(5.0)],
        );
        assert_eq!(x.f(pool, "CurrentValue"), 60.0);
        assert_eq!(x.f(pool, "PoolIdleDelayStartTime"), 1.0);
    });
}

#[test]
fn an_integer_only_resource_truncates_value_and_maximum_and_keeps_the_fraction() {
    run(|| {
        let mut x = fx();
        let (resource, def) = x.resource();
        x.set(resource, "bIntegerOnlyUpdates", Value::Bool(true));
        let pool = x.pool("ResourcePool");
        x.set(pool, "Definition", Value::Object(Some(def)));
        x.setf(pool, "MaxValue", 40.9);
        x.ret(
            "ResourcePool.SetCurrentValue",
            Some(pool),
            vec![Value::Float(12.75)],
        );
        assert_eq!(x.f(pool, "CurrentValue"), 12.0);
        assert_eq!(x.f(pool, "RateRemainder"), 0.75);
        x.ret(
            "ResourcePool.SetCurrentValue",
            Some(pool),
            vec![Value::Float(45.5)],
        );
        assert_eq!(
            x.f(pool, "CurrentValue"),
            40.0,
            "the maximum is truncated too"
        );
        assert_eq!(x.f(pool, "RateRemainder"), 0.5);
    });
}

#[test]
fn a_subclass_pools_current_value_hook_is_not_yet_in_off_and_skipped_in_reference() {
    run(|| {
        let mut x = fx();
        let pool = x.pool("ExperienceResourcePool");
        x.setf(pool, "MaxValue", 50.0);
        let e = x
            .call(
                "ResourcePool.SetCurrentValue",
                Some(pool),
                vec![Value::Float(5.0)],
            )
            .unwrap_err();
        assert!(matches!(vm_missing(e), Missing::NotRead));
        assert_eq!(
            x.f(pool, "CurrentValue"),
            0.0,
            "refused before anything is written"
        );

        let mut r = fx_mode(ParityMode::Reference);
        let pool = r.pool("ExperienceResourcePool");
        r.setf(pool, "MaxValue", 50.0);
        r.ret(
            "ResourcePool.SetCurrentValue",
            Some(pool),
            vec![Value::Float(5.0)],
        );
        assert_eq!(r.f(pool, "CurrentValue"), 5.0);
    });
}

#[test]
fn update_last_values_and_pool_is_now_full_follow_the_record() {
    run(|| {
        let mut x = fx();
        let pool = x.pool("ResourcePool");
        x.set(pool, "bCreatedAndNotModified", Value::Bool(true));
        x.setf(pool, "MinValue", 2.0);
        x.setf(pool, "MaxValue", 9.0);
        x.setf(pool, "CurrentValue", 0.00005);
        x.ret("ResourcePool.UpdateLastValues", Some(pool), vec![]);
        assert_eq!(
            x.get(pool, "bCreatedAndNotModified"),
            Value::Bool(true),
            "moved 0.00005: not more than 0.0001"
        );
        assert_eq!(x.f(pool, "LastMinValue"), 2.0);
        assert_eq!(x.f(pool, "LastMaxValue"), 9.0);
        x.setf(pool, "CurrentValue", 7.0);
        x.ret("ResourcePool.UpdateLastValues", Some(pool), vec![]);
        assert_eq!(x.get(pool, "bCreatedAndNotModified"), Value::Bool(false));
        assert_eq!(x.f(pool, "LastCurrentValue"), 7.0);

        x.setf(pool, "MinValue", 9.0);
        x.ret("ResourcePool.PoolIsNowFull", Some(pool), vec![]);
        assert_eq!(
            x.get(pool, "bHasPoolBeenFullSinceLastBeingDepleted"),
            Value::Bool(false),
            "min not below max"
        );
        x.setf(pool, "MinValue", 0.0);
        x.ret("ResourcePool.PoolIsNowFull", Some(pool), vec![]);
        assert_eq!(
            x.get(pool, "bHasPoolBeenFullSinceLastBeingDepleted"),
            Value::Bool(true)
        );
    });
}

#[test]
fn reset_idle_timer_is_not_yet_in_off_and_the_c_stamp_in_reference() {
    run(|| {
        let mut x = fx();
        let pool = x.pool("ResourcePool");
        let e = x
            .call("ResourcePool.ResetIdleTimer", Some(pool), vec![])
            .unwrap_err();
        assert!(matches!(vm_missing(e), Missing::NotRead));
        let mut r = fx_mode(ParityMode::Reference);
        let pool = r.pool("ResourcePool");
        r.ret("ResourcePool.ResetIdleTimer", Some(pool), vec![]);
        assert_eq!(r.f(pool, "PoolIdleDelayStartTime"), 12.5);
    });
}

#[test]
fn clear_attribute_modifier_stacks_empties_the_seven_stacks_and_recomputes_nothing() {
    run(|| {
        let mut x = fx();
        let pool = x.pool("ResourcePool");
        let thing = x.new_obj("Thing");
        for s in CLEARED_STACKS {
            x.set(
                pool,
                &format!("{s}ModifierStack"),
                Value::array(vec![Value::Object(Some(thing))]),
            );
        }
        x.setf(pool, "MaxValue", 120.0);
        x.ret(
            "ResourcePool.ClearAttributeModifierStacks",
            Some(pool),
            vec![],
        );
        for s in CLEARED_STACKS {
            assert_eq!(
                x.get(pool, &format!("{s}ModifierStack")),
                Value::array(vec![]),
                "{s}"
            );
        }
        assert_eq!(x.f(pool, "MaxValue"), 120.0, "no recompute");
    });
}

// ---------------------------------------------------------------------------------------------------------------
// References and the manager
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn the_first_pool_lands_in_slot_0_with_guid_0_and_runs_initialize_authoritative() {
    run(|| {
        let mut x = fx();
        let (controller, m) = x.controller_with_manager("Controller");
        let (resource, def) = x.resource();
        let (r, mgr) = x.create(Some(m), Some(controller), Some(def), false);
        assert_eq!(mgr, Some(m));
        let pool = r.data.expect("created");
        assert_eq!((r.manager, r.index, r.guid), (Some(m), 0, 0));
        assert_eq!(slot(&x.w.vm.space, m, 0).unwrap(), Some(pool));
        assert_eq!(x.get(pool, "PoolGUID"), Value::Byte(0));
        assert_eq!(x.get(m, "NextPoolGUID"), Value::Byte(1));
        assert_eq!(x.noted(pool), vec![10], "InitializeAuthoritative ran");
        assert_eq!(x.get(pool, "Definition"), Value::Object(Some(def)));
        assert_eq!(
            x.get(pool, "AssociatedProvider"),
            Value::Object(Some(controller)),
            "the manager's Owner"
        );
        let maps = x.w.vm.service::<PoolMaps>().unwrap();
        assert_eq!(maps.borrow().slot(m, resource), Some(0));
        // The reference resolves.
        assert_eq!(
            resolve(&x.w.vm.space, PoolRef { data: None, ..r })
                .unwrap()
                .1,
            Some(pool)
        );
    });
}

#[test]
fn a_second_pool_for_one_resource_is_refused_unless_reinitialised() {
    run(|| {
        let mut x = fx();
        let (controller, m) = x.controller_with_manager("Controller");
        let (resource, def) = x.resource();
        let (first, _) = x.create(Some(m), Some(controller), Some(def), false);
        let pool = first.data.unwrap();
        // Another definition naming the same resource is a match too.
        let other = x.new_obj("ResourcePoolDefinition");
        x.set(other, "Resource", Value::Object(Some(resource)));
        let (r, _) = x.create(Some(m), Some(controller), Some(other), false);
        assert_eq!(r, PoolRef::INVALID);
        assert_eq!(x.get(m, "NextPoolGUID"), Value::Byte(1), "nothing changed");
        // Reinitialise: the pool's Reinitialize runs, a new GUID that the pool does not carry.
        let (r, _) = x.create(Some(m), Some(controller), Some(def), true);
        assert_eq!((r.manager, r.index, r.guid, r.data), (Some(m), 0, 1, None));
        assert_eq!(x.noted(pool), vec![10, 40]);
        assert_eq!(x.get(pool, "PoolGUID"), Value::Byte(0));
        assert_eq!(
            resolve(&x.w.vm.space, r).unwrap().1,
            None,
            "the quirk: it does not validate"
        );
    });
}

#[test]
fn deleted_slots_are_reused_lowest_first_and_old_references_go_stale() {
    run(|| {
        let mut x = fx();
        let (controller, m) = x.controller_with_manager("Controller");
        let mut refs = Vec::new();
        let mut resources = Vec::new();
        for _ in 0..3 {
            let (res, def) = x.resource();
            resources.push(res);
            refs.push(x.create(Some(m), Some(controller), Some(def), false).0);
        }
        assert_eq!(
            refs.iter().map(|r| r.index).collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        let old = refs[1];
        let pool1 = old.data.unwrap();
        x.ret(
            "ResourcePoolManager.DeletePoolAtIndex",
            Some(m),
            vec![Value::Int(1)],
        );
        assert_eq!(x.noted(pool1), vec![10, 30], "ShutDown ran");
        assert_eq!(slot(&x.w.vm.space, m, 1).unwrap(), None);
        assert_eq!(
            x.w.vm
                .service::<PoolMaps>()
                .unwrap()
                .borrow()
                .slot(m, resources[1]),
            None
        );
        let (_, def) = x.resource();
        let (r, _) = x.create(Some(m), Some(controller), Some(def), false);
        assert_eq!(
            (r.index, r.guid),
            (1, 3),
            "the lowest free slot, the next GUID"
        );
        assert_eq!(
            resolve(&x.w.vm.space, old).unwrap().1,
            None,
            "the old reference to slot 1 is stale"
        );
        // An index out of range or an empty slot is nothing.
        x.ret(
            "ResourcePoolManager.DeletePoolAtIndex",
            Some(m),
            vec![Value::Int(16)],
        );
        x.ret(
            "ResourcePoolManager.DeletePoolAtIndex",
            Some(m),
            vec![Value::Int(-1)],
        );
        assert_eq!(pools_of(&x.w.vm.space, m).unwrap().len(), 3);
    });
}

#[test]
fn a_full_manager_logs_and_refuses_the_seventeenth_resource() {
    run(|| {
        let mut x = fx();
        let (controller, m) = x.controller_with_manager("Controller");
        for i in 0..16u8 {
            let (_, def) = x.resource();
            assert_eq!(
                x.create(Some(m), Some(controller), Some(def), false)
                    .0
                    .index,
                i
            );
        }
        let (_, def) = x.resource();
        let before = x.w.vm.log.len();
        assert_eq!(
            x.create(Some(m), Some(controller), Some(def), false).0,
            PoolRef::INVALID
        );
        assert!(
            x.w.vm.log[before..]
                .iter()
                .any(|l| l.starts_with("Too many ResourcePools"))
        );
        assert_eq!(x.get(m, "NextPoolGUID"), Value::Byte(16), "no GUID spent");
    });
}

#[test]
fn create_pool_needs_a_definition_and_an_authoritative_manager() {
    run(|| {
        let mut x = fx();
        let (controller, m) = x.controller_with_manager("Controller");
        assert_eq!(
            x.create(Some(m), Some(controller), None, false).0,
            PoolRef::INVALID
        );
        x.set(m, "Role", Value::Byte(2));
        let (_, def) = x.resource();
        let (r, mgr) = x.create(Some(m), Some(controller), Some(def), false);
        assert_eq!((r, mgr), (PoolRef::INVALID, Some(m)));
        assert_eq!(pools_of(&x.w.vm.space, m).unwrap(), vec![]);
    });
}

#[test]
fn a_manager_is_spawned_for_an_authoritative_owner_only() {
    run(|| {
        let mut x = fx();
        let controller = x.new_obj("Controller");
        let (_, def) = x.resource();
        let (r, mgr) = x.create(None, Some(controller), Some(def), false);
        let m = mgr.expect("spawned");
        assert_eq!(x.get(m, "Owner"), Value::Object(Some(controller)));
        assert!(
            x.w.actors().borrow().list().contains(&m),
            "a spawned actor of the level"
        );
        assert_eq!((r.manager, r.index), (Some(m), 0));
        assert_eq!(
            x.get(r.data.unwrap(), "AssociatedProvider"),
            Value::Object(Some(controller))
        );
        // No owner, or an owner without authority: no manager.
        let r = x
            .call(
                "ResourcePoolManager.CreateResourcePoolManagerIfNecessary",
                None,
                vec![Value::Object(None), Value::Object(None)],
            )
            .unwrap();
        assert_eq!(r.outs[0], Value::Object(None));
        x.set(controller, "Role", Value::Byte(1));
        let r = x
            .call(
                "ResourcePoolManager.CreateResourcePoolManagerIfNecessary",
                None,
                vec![Value::Object(None), Value::Object(Some(controller))],
            )
            .unwrap();
        assert_eq!(r.outs[0], Value::Object(None));
        // A manager already given is kept.
        let r = x
            .call(
                "ResourcePoolManager.CreateResourcePoolManagerIfNecessary",
                None,
                vec![Value::Object(Some(m)), Value::Object(None)],
            )
            .unwrap();
        assert_eq!(r.outs[0], Value::Object(Some(m)));
    });
}

#[test]
fn a_non_authoritative_manager_only_sets_the_definition() {
    run(|| {
        let mut x = fx();
        let (_, m) = x.controller_with_manager("Controller");
        x.set(m, "Role", Value::Byte(1));
        let (_, def) = x.resource();
        let made = x.ret(
            "ResourcePoolManager.InternalCreatePoolAtIndex",
            Some(m),
            vec![Value::Object(Some(def)), Value::Int(4), Value::Byte(9)],
        );
        let Value::Object(Some(pool)) = made else {
            panic!("{made:?}")
        };
        assert_eq!(slot(&x.w.vm.space, m, 4).unwrap(), Some(pool));
        assert_eq!(x.get(pool, "PoolGUID"), Value::Byte(9));
        assert_eq!(x.get(pool, "Definition"), Value::Object(Some(def)));
        assert_eq!(
            x.noted(pool),
            Vec::<i32>::new(),
            "no InitializeAuthoritative"
        );
        // A used slot, an index past 15 or no definition: None.
        for args in [
            vec![Value::Object(Some(def)), Value::Int(4), Value::Byte(1)],
            vec![Value::Object(Some(def)), Value::Int(16), Value::Byte(1)],
            vec![Value::Object(None), Value::Int(5), Value::Byte(1)],
        ] {
            assert_eq!(
                x.ret(
                    "ResourcePoolManager.InternalCreatePoolAtIndex",
                    Some(m),
                    args
                ),
                Value::Object(None)
            );
        }
        let e = x
            .call(
                "ResourcePoolManager.CreatePoolAtIndex",
                None,
                vec![
                    Value::Object(None),
                    Value::Object(None),
                    Value::Object(Some(def)),
                    Value::Int(0),
                    Value::Byte(0),
                ],
            )
            .unwrap_err();
        assert!(matches!(vm_missing(e), Missing::NotRead));
    });
}

#[test]
fn references_resolve_only_with_a_matching_guid_in_range() {
    run(|| {
        let mut x = fx();
        let (controller, m) = x.controller_with_manager("Controller");
        let (_, def) = x.resource();
        let (r, _) = x.create(Some(m), Some(controller), Some(def), false);
        let pool = r.data.unwrap();
        let space = &x.w.vm.space;
        let bare = PoolRef { data: None, ..r };
        assert_eq!(
            resolve(space, bare).unwrap(),
            (r, Some(pool)),
            "valid: the cache is filled"
        );
        let wrong = PoolRef {
            guid: 7,
            data: Some(pool),
            ..r
        };
        assert_eq!(
            resolve(space, wrong).unwrap(),
            (
                PoolRef {
                    data: None,
                    ..wrong
                },
                None
            ),
            "the cache is cleared"
        );
        for bad in [
            PoolRef { index: 16, ..bare },
            PoolRef { index: 3, ..bare },
            PoolRef {
                manager: None,
                ..bare
            },
            PoolRef::INVALID,
        ] {
            assert_eq!(resolve(space, bad).unwrap().1, None);
        }
        // IsResourcePoolValid and GetResourcePoolCurrentValueOrDefault refresh the caller's cache.
        x.setf(pool, "CurrentValue", 61.0);
        let call = x
            .call(
                "Actor.IsResourcePoolValid",
                Some(controller),
                vec![x.ref_value(bare)],
            )
            .unwrap();
        assert_eq!(call.ret, Value::Bool(true));
        assert_eq!(x.read(&call.outs[0]), r);
        let call = x
            .call(
                "Actor.GetResourcePoolCurrentValueOrDefault",
                Some(controller),
                vec![x.ref_value(bare), Value::Float(-1.0)],
            )
            .unwrap();
        assert_eq!((call.ret, x.read(&call.outs[0])), (Value::Float(61.0), r));
        let call = x
            .call(
                "Actor.GetResourcePoolCurrentValueOrDefault",
                Some(controller),
                vec![x.ref_value(wrong), Value::Float(-1.0)],
            )
            .unwrap();
        assert_eq!(call.ret, Value::Float(-1.0));
        assert_eq!(x.read(&call.outs[0]).data, None);
    });
}

#[test]
fn delete_by_reference_deletes_the_maps_slot_and_always_invalidates() {
    run(|| {
        let mut x = fx();
        let (controller, m) = x.controller_with_manager("Controller");
        let (_, def) = x.resource();
        let (r, _) = x.create(Some(m), Some(controller), Some(def), false);
        let pool = r.data.unwrap();
        // A stale reference deletes nothing, and is invalid afterwards.
        let stale = PoolRef { guid: 9, ..r };
        let call = x
            .call(
                "ResourcePoolManager.DeletePoolByReference",
                None,
                vec![x.ref_value(stale)],
            )
            .unwrap();
        assert_eq!(x.read(&call.outs[0]), PoolRef::INVALID);
        assert_eq!(slot(&x.w.vm.space, m, 0).unwrap(), Some(pool));
        let call = x
            .call(
                "ResourcePoolManager.DeletePoolByReference",
                None,
                vec![x.ref_value(r)],
            )
            .unwrap();
        assert_eq!(x.read(&call.outs[0]), PoolRef::INVALID);
        assert_eq!(slot(&x.w.vm.space, m, 0).unwrap(), None);
        assert_eq!(x.noted(pool), vec![10, 30]);
    });
}

#[test]
fn delete_pool_keeps_the_callers_reference_and_refuses_another_manager() {
    run(|| {
        let mut x = fx();
        let (controller, m) = x.controller_with_manager("Controller");
        let (_, def) = x.resource();
        let (r, _) = x.create(Some(m), Some(controller), Some(def), false);
        let (_, other) = x.controller_with_manager("Controller");
        let e = x
            .call(
                "ResourcePoolManager.DeletePool",
                None,
                vec![Value::Object(Some(other)), x.ref_value(r)],
            )
            .unwrap_err();
        assert!(matches!(vm_missing(e), Missing::NotRead));
        assert_eq!(slot(&x.w.vm.space, m, 0).unwrap(), r.data);
        x.ret(
            "ResourcePoolManager.DeletePool",
            None,
            vec![Value::Object(Some(m)), x.ref_value(r)],
        );
        assert_eq!(slot(&x.w.vm.space, m, 0).unwrap(), None);
    });
}

#[test]
fn delete_all_pools_needs_authority_and_empties_every_slot_in_order() {
    run(|| {
        let mut x = fx();
        let (controller, m) = x.controller_with_manager("Controller");
        let mut pools = Vec::new();
        for _ in 0..3 {
            let (_, def) = x.resource();
            pools.push(
                x.create(Some(m), Some(controller), Some(def), false)
                    .0
                    .data
                    .unwrap(),
            );
        }
        x.set(m, "Role", Value::Byte(1));
        x.ret(
            "ResourcePoolManager.DeleteAllPools",
            None,
            vec![Value::Object(Some(m))],
        );
        assert_eq!(
            pools_of(&x.w.vm.space, m).unwrap().len(),
            3,
            "no authority: nothing"
        );
        x.set(m, "Role", Value::Byte(3));
        x.ret(
            "ResourcePoolManager.DeleteAllPools",
            None,
            vec![Value::Object(Some(m))],
        );
        assert_eq!(pools_of(&x.w.vm.space, m).unwrap(), vec![]);
        let shut: Vec<ObjRef> = x
            .notes
            .borrow()
            .iter()
            .filter(|(_, k)| *k == 30)
            .map(|&(o, _)| o)
            .collect();
        assert_eq!(shut, pools, "slot order");
    });
}

#[test]
fn the_resource_lookups_use_the_map_and_never_create() {
    run(|| {
        let mut x = fx();
        let (controller, m) = x.controller_with_manager("Controller");
        let (resource, def) = x.resource();
        let (unknown, _) = x.resource();
        let call = x
            .call(
                "ResourcePoolManager.GetResourcePoolForResourceDefinition",
                None,
                vec![
                    Value::Object(Some(m)),
                    Value::Object(None),
                    Value::Object(Some(resource)),
                ],
            )
            .unwrap();
        assert_eq!(x.read(&call.ret), PoolRef::INVALID);
        assert_eq!(
            x.ret(
                "ResourcePoolManager.HasResourcePoolForResourceDefinition",
                Some(m),
                vec![Value::Object(Some(resource))]
            ),
            Value::Bool(false)
        );
        let (r, _) = x.create(Some(m), Some(controller), Some(def), false);
        let call = x
            .call(
                "ResourcePoolManager.GetResourcePoolForResourceDefinition",
                None,
                vec![
                    Value::Object(Some(m)),
                    Value::Object(None),
                    Value::Object(Some(resource)),
                ],
            )
            .unwrap();
        assert_eq!(x.read(&call.ret), r);
        assert_eq!(
            x.ret(
                "ResourcePoolManager.HasResourcePoolForResourceDefinition",
                Some(m),
                vec![Value::Object(Some(resource))]
            ),
            Value::Bool(true)
        );
        assert_eq!(
            x.ret(
                "ResourcePoolManager.HasResourcePoolForResourceDefinition",
                Some(m),
                vec![Value::Object(Some(unknown))]
            ),
            Value::Bool(false)
        );
        assert_eq!(pools_of(&x.w.vm.space, m).unwrap().len(), 1, "no creation");
    });
}

#[test]
fn the_controller_lookup_creates_from_the_resources_default_definition() {
    run(|| {
        let mut x = fx();
        let controller = x.new_obj("Controller");
        let (resource, def) = x.resource();
        // No manager and no create: invalid, nothing made.
        let call = x
            .call(
                "Controller.GetResourcePoolForResourceDefinition",
                Some(controller),
                vec![Value::Object(Some(resource)), Value::Bool(false)],
            )
            .unwrap();
        assert_eq!(x.read(&call.ret), PoolRef::INVALID);
        assert_eq!(
            x.get(controller, "ResourcePoolManager"),
            Value::Object(None)
        );
        // Omitted create is true: a manager and a pool from DefaultResourcePoolDefinition.
        let call = x
            .call(
                "Controller.GetResourcePoolForResourceDefinition",
                Some(controller),
                vec![Value::Object(Some(resource))],
            )
            .unwrap();
        let r = x.read(&call.ret);
        let Value::Object(Some(m)) = x.get(controller, "ResourcePoolManager") else {
            panic!("no manager")
        };
        assert_eq!((r.manager, r.index), (Some(m), 0));
        assert_eq!(
            x.get(r.data.unwrap(), "Definition"),
            Value::Object(Some(def))
        );
        // Found the second time, not made again.
        let again = x
            .call(
                "Controller.GetResourcePoolForResourceDefinition",
                Some(controller),
                vec![Value::Object(Some(resource))],
            )
            .unwrap();
        assert_eq!(x.read(&again.ret), r);
        assert_eq!(pools_of(&x.w.vm.space, m).unwrap().len(), 1);
        // A controller driving a vehicle that has no such pool: the vehicle's provider is not read.
        let (other, _) = x.resource();
        let car = x.new_obj("Vehicle");
        x.set(controller, "Pawn", Value::Object(Some(car)));
        let e = x
            .call(
                "Controller.GetResourcePoolForResourceDefinition",
                Some(controller),
                vec![Value::Object(Some(other))],
            )
            .unwrap_err();
        assert!(matches!(vm_missing(e), Missing::NotRead));
    });
}

// ---------------------------------------------------------------------------------------------------------------
// The pawn's health
// ---------------------------------------------------------------------------------------------------------------

/// A pawn with a controller that has a manager.
fn pawn(x: &mut Fx) -> (ObjRef, ObjRef, ObjRef) {
    let (controller, m) = x.controller_with_manager("Controller");
    let p = x.new_obj("Pawn");
    x.set(p, "Controller", Value::Object(Some(controller)));
    x.set(controller, "Pawn", Value::Object(Some(p)));
    let invalid = x.ref_value(PoolRef::INVALID);
    x.set(p, "HealthPool", invalid);
    (p, controller, m)
}

#[test]
fn without_a_pool_the_pawn_uses_its_plain_health_variables() {
    run(|| {
        let mut x = fx();
        let (p, _, _) = pawn(&mut x);
        x.setf(p, "HealthVar", 40.0);
        x.setf(p, "HealthMaxVar", 90.0);
        assert_eq!(x.ret("Pawn.GetHealth", Some(p), vec![]), Value::Float(40.0));
        assert_eq!(
            x.ret("Pawn.GetMaxHealth", Some(p), vec![]),
            Value::Float(90.0)
        );
        x.ret("Pawn.SetHealth", Some(p), vec![Value::Float(0.5)]);
        assert_eq!(x.f(p, "HealthVar"), 0.0, "0 <= v < 1 becomes 0");
        x.ret("Pawn.SetHealth", Some(p), vec![Value::Float(-3.0)]);
        assert_eq!(x.f(p, "HealthVar"), -3.0, "a negative value is kept");
        x.ret("Pawn.SetMaxHealth", Some(p), vec![Value::Float(70.0)]);
        assert_eq!(x.f(p, "HealthMaxVar"), 70.0);
    });
}

#[test]
fn set_health_pool_definition_makes_one_pool_and_the_health_natives_follow_it() {
    run(|| {
        let mut x = fx();
        let (p, controller, m) = pawn(&mut x);
        let (_, def) = x.resource();
        let call = x
            .call(
                "Pawn.SetHealthResourcePoolDefinition",
                Some(p),
                vec![Value::Object(Some(m)), Value::Object(Some(def))],
            )
            .unwrap();
        assert_eq!(call.outs[0], Value::Object(Some(m)));
        let r = x.read(&x.get(p, "HealthPool"));
        let pool = r.data.expect("a pool");
        assert_eq!((r.manager, r.index), (Some(m), 0));
        assert_eq!(
            x.get(pool, "AssociatedProvider"),
            Value::Object(Some(controller))
        );
        x.setf(pool, "MaxValue", 150.0);
        x.setf(pool, "MinValue", 0.0);
        x.set(pool, "bIsAuthoritative", Value::Bool(true));
        // Called again: no second pool; HealthMaxVar refreshed from the effective maximum.
        x.ret(
            "Pawn.SetHealthResourcePoolDefinition",
            Some(p),
            vec![Value::Object(Some(m)), Value::Object(Some(def))],
        );
        assert_eq!(pools_of(&x.w.vm.space, m).unwrap().len(), 1);
        assert_eq!(x.f(p, "HealthMaxVar"), 150.0);

        x.setf(p, "HealthVar", 5.0);
        x.ret("Pawn.SetHealth", Some(p), vec![Value::Float(-3.0)]);
        assert_eq!(
            x.f(pool, "CurrentValue"),
            0.0,
            "the pool clamps to its minimum"
        );
        assert_eq!(
            x.f(p, "HealthVar"),
            5.0,
            "HealthVar is not written while the pool is valid"
        );
        x.ret("Pawn.SetHealth", Some(p), vec![Value::Float(99.0)]);
        assert_eq!(x.ret("Pawn.GetHealth", Some(p), vec![]), Value::Float(99.0));
        assert_eq!(
            x.ret("Pawn.GetMaxHealth", Some(p), vec![]),
            Value::Float(150.0)
        );
        x.ret("Pawn.SetMaxHealth", Some(p), vec![Value::Float(1.0)]);
        assert_eq!(
            x.ret("Pawn.GetMaxHealth", Some(p), vec![]),
            Value::Float(150.0),
            "SetMaxHealth never touches the pool"
        );

        // Deleting the pool by the pawn's reference: GetHealth falls back to HealthVar.
        let held = x.get(p, "HealthPool");
        x.call(
            "ResourcePoolManager.DeletePoolByReference",
            None,
            vec![held],
        )
        .unwrap();
        assert_eq!(x.ret("Pawn.GetHealth", Some(p), vec![]), Value::Float(5.0));
    });
}

#[test]
fn a_pawn_without_authority_gets_no_pool() {
    run(|| {
        let mut x = fx();
        let (p, _, m) = pawn(&mut x);
        x.set(p, "Role", Value::Byte(2));
        let (_, def) = x.resource();
        x.ret(
            "Pawn.SetHealthResourcePoolDefinition",
            Some(p),
            vec![Value::Object(Some(m)), Value::Object(Some(def))],
        );
        assert_eq!(pools_of(&x.w.vm.space, m).unwrap(), vec![]);
        assert_eq!(x.read(&x.get(p, "HealthPool")), PoolRef::INVALID);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// The engine query
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn the_resource_pool_query_finds_a_controllers_pool_through_the_controller_resolver_rule() {
    run(|| {
        let mut x = fx();
        let (controller, m) = x.controller_with_manager("Controller");
        let (resource, def) = x.resource();
        let resolver = x.new_obj("ControllerAttributeContextResolver");
        x.set(
            resource,
            "ResourceContextResolver",
            Value::Object(Some(resolver)),
        );
        let q = EngineQuery::ResourcePool { resource };
        assert_eq!(
            x.vm().engine_query(q, controller).unwrap(),
            None,
            "no pool yet"
        );
        let (r, _) = x.create(Some(m), Some(controller), Some(def), false);
        assert_eq!(x.vm().engine_query(q, controller).unwrap(), r.data);
        // The controller rule on another source (`ow-attr`'s, lane FIX-EQUIP-NATIVES): an actor whose instigator is
        // a pawn of this controller (a held weapon) finds the controller's pool; an object that is no actor has no
        // controller, so no pool.
        let p = x.new_obj("Pawn");
        x.set(p, "Controller", Value::Object(Some(controller)));
        let held = x.new_obj("Actor");
        x.set(held, "Instigator", Value::Object(Some(p)));
        assert_eq!(x.vm().engine_query(q, held).unwrap(), r.data);
        assert_eq!(x.vm().engine_query(q, def).unwrap(), None);
        // Another resolver class: not read.
        let pawn_resolver = x.new_obj("PawnAttributeContextResolver");
        x.set(
            resource,
            "ResourceContextResolver",
            Value::Object(Some(pawn_resolver)),
        );
        assert!(matches!(
            x.vm().engine_query(q, controller).unwrap_err(),
            ow_attr::api::AttrError::NotYet(_)
        ));
        // No resolver on the resource: no owner, no pool.
        x.set(resource, "ResourceContextResolver", Value::Object(None));
        assert_eq!(x.vm().engine_query(q, controller).unwrap(), None);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// The rebase and the level-up
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn recalculate_base_values_runs_calculate_base_values_true_on_every_pool_in_slot_order() {
    run(|| {
        let mut x = fx();
        let (controller, m) = x.controller_with_manager("Controller");
        let mut pools = Vec::new();
        for _ in 0..3 {
            let (_, def) = x.resource();
            pools.push(
                x.create(Some(m), Some(controller), Some(def), false)
                    .0
                    .data
                    .unwrap(),
            );
        }
        x.ret(
            "ResourcePoolManager.DeletePoolAtIndex",
            Some(m),
            vec![Value::Int(1)],
        );
        x.notes.borrow_mut().clear();
        x.ret(
            "ResourcePoolManager.RecalculateBaseValues",
            None,
            vec![Value::Object(Some(m))],
        );
        let seen = x.notes.borrow().clone();
        assert_eq!(
            seen,
            vec![
                (pools[0], 20),
                (pools[0], 21),
                (pools[2], 20),
                (pools[2], 21)
            ],
            "true each time, empty slot skipped"
        );
        x.notes.borrow_mut().clear();
        x.ret(
            "ResourcePoolManager.RecalculateBaseValues",
            None,
            vec![Value::Object(None)],
        );
        assert!(x.notes.borrow().is_empty(), "a None manager is skipped");
    });
}

#[test]
fn the_level_up_recalculation_rebases_every_pool_of_a_controller_without_a_pawn() {
    run(|| {
        let mut x = fx();
        for class in ["Controller", "WillowPlayerController"] {
            let (controller, m) = x.controller_with_manager(class);
            let (_, def) = x.resource();
            let pool = x
                .create(Some(m), Some(controller), Some(def), false)
                .0
                .data
                .unwrap();
            x.ret(
                &format!("{class}.RecalculateAttributeInitializedState"),
                Some(controller),
                vec![],
            );
            assert_eq!(x.noted(pool), vec![10, 20, 21], "{class}");
        }
        // No manager: nothing to rebase, no error.
        let bare = x.new_obj("Controller");
        let done = levelup::recalculate_attribute_initialized_state(x.vm(), bare).unwrap();
        assert_eq!(done.manager, None);
    });
}

#[test]
fn the_class_level_parts_are_not_yet_in_off_and_skipped_in_reference() {
    run(|| {
        for mode in [ParityMode::Off, ParityMode::Reference] {
            let mut x = fx_mode(mode);
            let (controller, m) = x.controller_with_manager("WillowPlayerController");
            let (_, def) = x.resource();
            let pool = x
                .create(Some(m), Some(controller), Some(def), false)
                .0
                .data
                .unwrap();
            let p = x.new_obj("Pawn");
            let class_def = x.new_obj("WillowCharacterClassDefinition");
            let component = x.new_obj("Thing");
            x.set(controller, "Pawn", Value::Object(Some(p)));
            x.set(controller, "CharacterClass", Value::Object(Some(class_def)));
            let result = levelup::recalculate_attribute_initialized_state(x.vm(), controller);
            match mode {
                ParityMode::Off => {
                    assert!(matches!(vm_missing(result.unwrap_err()), Missing::NotRead));
                    assert_eq!(x.noted(pool), vec![10], "refused before the rebase");
                    // A vehicle pawn skips the class part; the status part still applies with MyWillowPawn.
                    let car = x.new_obj("Vehicle");
                    x.set(controller, "Pawn", Value::Object(Some(car)));
                    x.set(controller, "MyWillowPawn", Value::Object(Some(p)));
                    x.set(p, "StatusEffectComp", Value::Object(Some(component)));
                    let e = levelup::recalculate_attribute_initialized_state(x.vm(), controller)
                        .unwrap_err();
                    assert!(matches!(vm_missing(e), Missing::NotRead));
                    // A plain CharacterClassDefinition: the status part does not apply either.
                    let plain = x.new_obj("CharacterClassDefinition");
                    x.set(controller, "CharacterClass", Value::Object(Some(plain)));
                    let done = levelup::recalculate_attribute_initialized_state(x.vm(), controller)
                        .unwrap();
                    assert!(!done.class_parts_skipped);
                    assert_eq!(x.noted(pool), vec![10, 20, 21]);
                }
                _ => {
                    let done: Recalculated = result.unwrap();
                    assert!(done.class_parts_skipped);
                    assert_eq!(x.noted(pool), vec![10, 20, 21]);
                }
            }
        }
    });
}

/// The `WillowMind` override (NATIVE_MIND_ROUTE.md section 3; decision 2026-11-19-32), Off: with an `AIClass` and a
/// `MyWillowPawn` its own part needs an evaluation context the note does not name, so `NotYet` before the rebase;
/// without them, and with no game info (no skill refresh), the shared controller part rebases the pools. Reference has
/// no stand-in and stays `NotYet`.
#[test]
fn a_willow_minds_recalculation_runs_the_settled_parts() {
    run(|| {
        let mut x = fx();
        let (mind, m) = x.controller_with_manager("WillowMind");
        let (_, def) = x.resource();
        let pool = x.create(Some(m), Some(mind), Some(def), false).0.data.unwrap();
        let recalc = |x: &mut Fx, mind| x.call("Controller.RecalculateAttributeInitializedState", Some(mind), vec![]);
        recalc(&mut x, mind).unwrap();
        assert_eq!(x.noted(pool), vec![10, 20, 21], "no class, no pawn: the controller part rebased the pool");
        let (class, pawn) = (x.new_obj("CharacterClassDefinition"), x.new_obj("Pawn"));
        x.set(mind, "AIClass", Value::Object(Some(class)));
        x.set(mind, "MyWillowPawn", Value::Object(Some(pawn)));
        let e = recalc(&mut x, mind).unwrap_err();
        assert!(matches!(vm_missing(e), Missing::NotRead));
        assert_eq!(x.noted(pool), vec![10, 20, 21], "refused before the rebase");
        let mut r = fx_mode(ParityMode::Reference);
        let (mind, _) = r.controller_with_manager("WillowMind");
        let e = r.call("Controller.RecalculateAttributeInitializedState", Some(mind), vec![]).unwrap_err();
        assert!(matches!(vm_missing(e), Missing::NotRead), "Reference has no stand-in");
    });
}

#[test]
fn every_registered_key_is_a_native_of_the_invented_package() {
    run(|| {
        let mut x = fx();
        for key in NATIVE_KEYS.iter().chain(levelup::NATIVE_KEYS.iter()) {
            assert!(x.vm().has_native(key), "{key}");
        }
        let _ = x.wi;
    });
}

// ---------------------------------------------------------------------------------------------------------------
// The installed game (local only; AGENTS.md rule 1: the output stays under ignored local/)
// ---------------------------------------------------------------------------------------------------------------

/// Maya's health pool at levels 1 to 80 through the installed script `CalculateBaseValues(true)`, the real evaluator
/// and `RecalculateAttributeInitializedState`, against the recorded `max(20, 80 x 1.13^L)` (NATIVE_PROGRESSION.md
/// section 4; W3-ATTREVAL found the evaluator bit-identical to it) and the three live values of
/// NATIVE_LEVEL_UP_ATTRIBUTES.md section 7. Also the follow rule on the same pool: a pool that is not full keeps its
/// current value, a full one follows the new maximum. Runs only with `OW_COOKED` set to the installed
/// `CookedPCConsole` folder (`cargo test -p ow-world -- --ignored installed_game`), in `ParityMode::Reference`
/// because the pool's creation script reaches `ResetIdleTimer` (module documentation). Writes
/// `local/p2/w5-pools/health_1_80.txt`.
#[test]
#[ignore = "needs the installed game (OW_COOKED)"]
fn installed_game_maya_health_pool_levels_1_to_80() {
    let Some(cooked) = std::env::var_os("OW_COOKED") else {
        eprintln!("OW_COOKED is not set: skipped");
        return;
    };
    run(move || {
        use std::fmt::Write as _;
        let mode = ParityMode::Reference;
        let defs = DefStore::open(
            PathBuf::from(&cooked),
            IdentityPolicy::FirstInScanOrder,
            mode,
        )
        .unwrap();
        let vm = Vm::new(
            ObjectSpace::new(),
            defs,
            Rc::new(AttributeSystem::standard().unwrap()),
            mode,
        );
        let mut w = World::new(vm, BalanceState::new(snapshot()), &[&natives_core::Pack]).unwrap();
        let vm = &mut w.vm;
        let make = |vm: &mut Vm, path: &str| {
            let c = vm.find_class(path).unwrap();
            ow_vm::interp::instantiate_from_defaults(vm, c).unwrap()
        };
        let wi = make(vm, "Engine.WorldInfo");
        w.actors().borrow_mut().place_world_info(wi).unwrap();
        let vm = &mut w.vm;
        let controller = make(vm, "WillowGame.WillowPlayerController");
        let pri = make(vm, "WillowGame.WillowPlayerReplicationInfo");
        let manager = make(vm, "Engine.ResourcePoolManager");
        for o in [controller, manager] {
            set_field(&mut vm.space, o, "Role", Value::Byte(3)).unwrap();
        }
        set_field(
            &mut vm.space,
            controller,
            "PlayerReplicationInfo",
            Value::Object(Some(pri)),
        )
        .unwrap();
        set_field(
            &mut vm.space,
            manager,
            "Owner",
            Value::Object(Some(controller)),
        )
        .unwrap();
        set_field(
            &mut vm.space,
            controller,
            "ResourcePoolManager",
            Value::Object(Some(manager)),
        )
        .unwrap();
        let path = ow_core::ObjectPath::parse("D_Resourcepools.PlayerPools.HealthPool").unwrap();
        let def = vm
            .defs
            .find(&mut vm.space, &path)
            .unwrap()
            .expect("the health pool definition");
        let made = create_pool(vm, Some(manager), Some(controller), Some(def), false).unwrap();
        let pool = made.reference.data.expect("the health pool");
        let level_prop = vm
            .space
            .prop(vm.space.heap.get(pri).unwrap().class, "ExpLevel")
            .unwrap();
        let level_kind = vm.space.classes.decl(level_prop).unwrap().kind.clone();
        let f = |vm: &Vm, name: &str| float(&vm.space, pool, name).unwrap();
        let mut out = String::new();
        let (mut bit_equal, mut follow_ok) = (0, 0);
        writeln!(out, "# AI-assisted (Claude Code). W5-POOLS, local only. level base max current_before current_after expected bits_equal rule").unwrap();
        for level in 1u8..=80 {
            let v = match level_kind {
                PropKind::Byte { .. } | PropKind::ByteAttribute { .. } => Value::Byte(level),
                _ => Value::Int(i32::from(level)),
            };
            set_field(&mut vm.space, pri, "ExpLevel", v).unwrap();
            // Odd levels start from half the old maximum (not full), even ones from a full pool.
            let old_max = f(vm, "MaxValue");
            let full = level % 2 == 0;
            let start = if full { old_max } else { old_max / 2.0 };
            set_field(&mut vm.space, pool, "CurrentValue", Value::Float(start)).unwrap();
            update_last_values(&mut vm.space, pool).unwrap();
            levelup::recalculate_attribute_initialized_state(vm, controller).unwrap();
            let (base, max, current) = (
                f(vm, "MaxValueBaseValue"),
                f(vm, "MaxValue"),
                f(vm, "CurrentValue"),
            );
            let expected = (80.0f32 * ow_core::num::pow_f32(1.13, f32::from(level))).max(20.0);
            let same = base.to_bits() == expected.to_bits() && max.to_bits() == base.to_bits();
            let rule = if full {
                current.to_bits() == max.to_bits()
            } else {
                current.to_bits() == start.to_bits()
            };
            bit_equal += usize::from(same);
            follow_ok += usize::from(rule);
            writeln!(
                out,
                "{level} {} {} {} {} {} {same} {rule}",
                ow_core::fmt::fmt_g(f64::from(base), 9),
                ow_core::fmt::fmt_g(f64::from(max), 9),
                ow_core::fmt::fmt_g(f64::from(start), 9),
                ow_core::fmt::fmt_g(f64::from(current), 9),
                ow_core::fmt::fmt_g(f64::from(expected), 9),
            )
            .unwrap();
            if let Some(&(_, live)) = [(8u8, 212.6755f64), (9, 240.3233), (10, 271.5654)]
                .iter()
                .find(|(l, _)| *l == level)
            {
                assert!(
                    (f64::from(base) - live).abs() <= 0.5e-4,
                    "L{level}: {base} vs live {live}"
                );
            }
        }
        writeln!(
            out,
            "# bit-equal {bit_equal}/80, follow rule {follow_ok}/80"
        )
        .unwrap();
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../local/p2/w5-pools");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("health_1_80.txt"), &out).unwrap();
        assert_eq!((bit_equal, follow_ok), (80, 80), "{out}");
    });
}

/// The pool class of every installed `ResourcePoolDefinition`'s resource (`DefaultResourcePoolClass`), for the
/// subclass-hook rule of `set_current`. Local only: writes `local/p2/w5-pools/pool_classes.txt`.
#[test]
#[ignore = "needs the installed game (OW_COOKED)"]
fn installed_game_pool_classes() {
    let Some(cooked) = std::env::var_os("OW_COOKED") else {
        eprintln!("OW_COOKED is not set: skipped");
        return;
    };
    run(move || {
        use std::fmt::Write as _;
        let mode = ParityMode::Off;
        let mut defs = DefStore::open(
            PathBuf::from(&cooked),
            IdentityPolicy::FirstInScanOrder,
            mode,
        )
        .unwrap();
        let mut space = ObjectSpace::new();
        let paths = defs
            .exports_of_class("Engine.ResourcePoolDefinition")
            .unwrap();
        let mut out = String::new();
        writeln!(
            out,
            "# AI-assisted (Claude Code). W5-POOLS, local only. definition resource pool_class"
        )
        .unwrap();
        for path in paths {
            let Ok(Some(def)) = defs.find(&mut space, &path) else {
                writeln!(out, "{} (not loaded)", path.as_str()).unwrap();
                continue;
            };
            let resource = object(&space, def, "Resource").ok().flatten();
            let (res_path, class) = match resource {
                Some(r) => {
                    defs.ensure_loaded(&mut space, r).unwrap();
                    let c = match field(&space, r, "DefaultResourcePoolClass") {
                        Ok(Value::Class(Some(c))) => space
                            .classes
                            .get_layout(c)
                            .map(|l| l.path().as_str().to_string())
                            .unwrap_or_default(),
                        other => format!("{other:?}"),
                    };
                    let resolver = match field(&space, r, "ResourceContextResolver") {
                        Ok(Value::Object(Some(o))) => ow_attr::eval::class_path(&space, o),
                        Ok(Value::Object(None)) => "None".to_string(),
                        other => format!("{other:?}"),
                    };
                    (
                        ow_attr::eval::class_path(&space, r) + " " + &object_path_of(&space, r),
                        c + " resolver=" + &resolver,
                    )
                }
                None => ("None".to_string(), "-".to_string()),
            };
            writeln!(out, "{} {res_path} {class}", path.as_str()).unwrap();
        }
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../local/p2/w5-pools");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("pool_classes.txt"), &out).unwrap();
    });
}

fn object_path_of(space: &ObjectSpace, o: ObjRef) -> String {
    space
        .heap
        .get(o)
        .map(|x| space.names.text(x.name).into_owned())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------------------------------------------
// Review r1 of PR #329
// ---------------------------------------------------------------------------------------------------------------

/// Review r2 of #444: a refusal inside `InitializeAuthoritative` keeps the rollback under Reference (the slot emptied
/// again, the GUID counter restored, the error unchanged) and is terminal in Off (the slot kept, the GUID spent).
#[test]
fn a_creation_refusal_rolls_back_under_reference_and_is_terminal_in_off() {
    run(|| {
        for mode in [ParityMode::Reference, ParityMode::Off] {
            let mut x = fx_mode(mode);
            let (controller, m) = x.controller_with_manager("Controller");
            let (_, def) = x.resource_with("RefusingPool");
            let e = x
                .call(
                    "ResourcePoolManager.CreatePool",
                    None,
                    vec![
                        Value::Object(Some(m)),
                        Value::Object(Some(controller)),
                        Value::Object(Some(def)),
                        Value::Bool(false),
                    ],
                )
                .unwrap_err();
            let slots = pools_of(&x.w.vm.space, m).unwrap().len();
            if mode == ParityMode::Reference {
                assert!(!e.to_string().contains("partial writes"), "{e}");
                assert_eq!(slots, 0, "Reference: the slot is emptied again");
                assert_eq!(x.get(m, "NextPoolGUID"), Value::Byte(0), "Reference: the GUID counter is restored");
            } else {
                assert_eq!(slots, 1, "Off: the slot keeps the new pool");
                assert_eq!(x.get(m, "NextPoolGUID"), Value::Byte(1), "Off: the GUID stays spent");
            }
        }
    });
}

/// Review r1 of #444 (the hub's ruling, which replaced the rollback of review r1 of #329): an Off refusal inside
/// `InitializeAuthoritative` is terminal. Its kind is kept and its text starts "stopped after partial writes"; the slot
/// keeps the new pool, the GUID stays spent, and no map entry is made.
#[test]
fn an_off_mode_refusal_inside_initialize_authoritative_is_terminal() {
    run(|| {
        let mut x = fx();
        let (controller, m) = x.controller_with_manager("Controller");
        let (resource, def) = x.resource_with("TimedPool");
        let e = x
            .call(
                "ResourcePoolManager.CreatePool",
                None,
                vec![
                    Value::Object(Some(m)),
                    Value::Object(Some(controller)),
                    Value::Object(Some(def)),
                    Value::Bool(false),
                ],
            )
            .unwrap_err();
        assert!(e.to_string().contains("stopped after partial writes: "), "{e}");
        assert!(
            matches!(vm_missing(e), Missing::NotRead),
            "ResetIdleTimer is not read"
        );
        assert_eq!(pools_of(&x.w.vm.space, m).unwrap().len(), 1, "the slot keeps the new pool");
        assert_eq!(x.get(m, "NextPoolGUID"), Value::Byte(1), "the GUID stays spent");
        assert_eq!(
            x.w.vm
                .service::<PoolMaps>()
                .unwrap()
                .borrow()
                .slot(m, resource),
            None,
            "no map entry"
        );
        // Reference mode: the C++ stamp, and the pool is made.
        let mut r = fx_mode(ParityMode::Reference);
        let (controller, m) = r.controller_with_manager("Controller");
        let (_, def) = r.resource_with("TimedPool");
        let (made, _) = r.create(Some(m), Some(controller), Some(def), false);
        let pool = made.data.expect("made");
        assert_eq!(r.noted(pool), vec![11]);
        assert_eq!(r.f(pool, "PoolIdleDelayStartTime"), 12.5);
    });
}

#[test]
fn only_the_allow_listed_pool_classes_pass_the_hook() {
    run(|| {
        let mut x = fx();
        for (class, refused) in [
            ("ExperienceResourcePool", true),
            ("AmmoResourcePool", true),
            ("OddResourcePool", true),
            ("HealthResourcePool", false),
            ("ShieldResourcePool", false),
            ("ResourcePool", false),
        ] {
            let pool = x.pool(class);
            x.setf(pool, "MaxValue", 50.0);
            let r = x.call(
                "ResourcePool.SetCurrentValue",
                Some(pool),
                vec![Value::Float(5.0)],
            );
            assert_eq!(r.is_err(), refused, "{class}");
            let want = if refused { 0.0 } else { 5.0 };
            assert_eq!(x.f(pool, "CurrentValue"), want, "{class}");
        }
    });
}

#[test]
fn set_current_value_of_nan_gives_the_minimum_as_the_c_comparison_does() {
    run(|| {
        let mut x = fx();
        let pool = x.pool("ResourcePool");
        x.setf(pool, "MinValue", 3.0);
        x.setf(pool, "MaxValue", 50.0);
        x.ret(
            "ResourcePool.SetCurrentValue",
            Some(pool),
            vec![Value::Float(f32::NAN)],
        );
        assert_eq!(x.f(pool, "CurrentValue"), 3.0);
    });
}

#[test]
fn set_health_refuses_an_unread_hook_before_it_writes_the_pawns_cache() {
    run(|| {
        let mut x = fx();
        let (p, _, m) = pawn(&mut x);
        // An authoritative ammo pool in slot 0 with GUID 0, and an uncached reference to it on the pawn.
        let ammo = x.pool("AmmoResourcePool");
        let slots = prop_of(&x.w.vm.space, m, "ResourcePools").unwrap();
        x.vm()
            .space
            .write(
                m,
                ow_object::ids::PropAddr::element(slots, 0),
                Value::Object(Some(ammo)),
            )
            .unwrap();
        let bare = x.ref_value(PoolRef::new(Some(m), 0, 0, None));
        x.set(p, "HealthPool", bare.clone());
        let e = x
            .call("Pawn.SetHealth", Some(p), vec![Value::Float(10.0)])
            .unwrap_err();
        assert!(matches!(vm_missing(e), Missing::NotRead));
        assert_eq!(x.get(p, "HealthPool"), bare, "the cache is not written");
    });
}

#[test]
fn the_vehicle_refusal_comes_before_a_manager_is_made() {
    run(|| {
        let mut x = fx();
        let controller = x.new_obj("Controller");
        let car = x.new_obj("Vehicle");
        x.set(controller, "Pawn", Value::Object(Some(car)));
        let (resource, _) = x.resource();
        let before = x.w.actors().borrow().list().len();
        let e = x
            .call(
                "Controller.GetResourcePoolForResourceDefinition",
                Some(controller),
                vec![Value::Object(Some(resource))],
            )
            .unwrap_err();
        assert!(matches!(vm_missing(e), Missing::NotRead));
        assert_eq!(
            x.get(controller, "ResourcePoolManager"),
            Value::Object(None)
        );
        assert_eq!(
            x.w.actors().borrow().list().len(),
            before,
            "nothing spawned"
        );
    });
}

#[test]
fn the_too_many_pools_line_names_the_definition_path_and_the_owner() {
    run(|| {
        let mut x = fx();
        let (controller, m) = x.controller_with_manager("Controller");
        for _ in 0..16 {
            let (_, def) = x.resource();
            x.create(Some(m), Some(controller), Some(def), false);
        }
        let (_, def) = x.resource();
        x.create(Some(m), Some(controller), Some(def), false);
        let line = x.w.vm.log.last().unwrap().clone();
        let def_path = object_path(&x.w.vm.space, def);
        let owner_path = object_path(&x.w.vm.space, controller);
        assert_eq!(
            line,
            format!("Too many ResourcePools: {def_path} (owner {owner_path})")
        );
    });
}

#[test]
fn reference_mode_reads_no_class_level_condition() {
    run(|| {
        let mut r = fx_mode(ParityMode::Reference);
        let controller = r.new_obj("BareController");
        let m = r.new_obj("ResourcePoolManager");
        r.set(m, "Owner", Value::Object(Some(controller)));
        r.set(controller, "ResourcePoolManager", Value::Object(Some(m)));
        let (_, def) = r.resource();
        let pool = r
            .create(Some(m), Some(controller), Some(def), false)
            .0
            .data
            .unwrap();
        let done = levelup::recalculate_attribute_initialized_state(r.vm(), controller).unwrap();
        assert!(done.class_parts_skipped);
        assert_eq!(r.noted(pool), vec![10, 20, 21]);
        // Off mode reads them, and this class declares none: a typed error, not a guess.
        let mut x = fx();
        let controller = x.new_obj("BareController");
        assert!(levelup::recalculate_attribute_initialized_state(x.vm(), controller).is_err());
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Review r2 of PR #329
// ---------------------------------------------------------------------------------------------------------------

/// The live (not destroyed) `ResourcePoolManager` objects: a destroyed actor leaves the level list and the live set.
fn live_managers(x: &mut Fx) -> usize {
    let c = x.vm().find_class("Engine.ResourcePoolManager").unwrap();
    x.w.vm.space.live(c).count()
}

#[test]
fn a_refused_create_pool_destroys_the_manager_it_spawned() {
    run(|| {
        let mut x = fx();
        let controller = x.new_obj("Controller");
        assert_eq!(live_managers(&mut x), 0);
        let (_, def) = x.resource_with("TimedPool");
        let e = x
            .call(
                "ResourcePoolManager.CreatePool",
                None,
                vec![
                    Value::Object(None),
                    Value::Object(Some(controller)),
                    Value::Object(Some(def)),
                    Value::Bool(false),
                ],
            )
            .unwrap_err();
        assert!(matches!(vm_missing(e), Missing::NotRead));
        assert_eq!(live_managers(&mut x), 0, "the spawned manager is destroyed again");
        // The same call with a pool class that initialises keeps its spawned manager.
        let (_, ok_def) = x.resource();
        let (r, mgr) = x.create(None, Some(controller), Some(ok_def), false);
        assert!(r.data.is_some() && mgr.is_some());
        assert_eq!(live_managers(&mut x), 1);
        // A manager the caller gave is kept.
        let (controller, m) = x.controller_with_manager("Controller");
        let (_, def) = x.resource_with("TimedPool");
        assert!(
            x.call(
                "ResourcePoolManager.CreatePool",
                None,
                vec![
                    Value::Object(Some(m)),
                    Value::Object(Some(controller)),
                    Value::Object(Some(def)),
                    Value::Bool(false)
                ]
            )
            .is_err()
        );
        assert!(!crate::actors::is_destroyed(&x.w.vm.space, m).unwrap());
    });
}

#[test]
fn a_refused_controller_lookup_leaves_the_controller_as_it_was() {
    run(|| {
        let mut x = fx();
        let controller = x.new_obj("Controller");
        let (resource, _) = x.resource_with("TimedPool");
        let e = x
            .call(
                "Controller.GetResourcePoolForResourceDefinition",
                Some(controller),
                vec![Value::Object(Some(resource))],
            )
            .unwrap_err();
        assert!(matches!(vm_missing(e), Missing::NotRead));
        assert_eq!(
            x.get(controller, "ResourcePoolManager"),
            Value::Object(None),
            "never written"
        );
        assert_eq!(live_managers(&mut x), 0, "the spawned manager is destroyed");
    });
}
