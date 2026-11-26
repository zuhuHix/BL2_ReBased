//! Invented package and values only. The package helpers build script shutdowns; no installed script is copied.
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_balance::types::*;
use ow_core::ParityMode;
use ow_defs::{DefStore, identity::IdentityPolicy};
use ow_object::{heap::ObjectSpace, ids::PropAddr};
use ow_package::testkit::script::{Asm, FunctionSpec, Param, ScriptToy};
use ow_package::testkit::{Export, ints, reflect::property_payload};
use ow_vm::api::{StubPolicy, WorldTimeQuery};
use ow_world::world::{BalanceState, World};

use super::*;
use crate::hit::access::{get, set_required};

const NATIVE: u32 = 0x400 | 0x20000;
const SCRIPT: u32 = 0x1 | 0x2 | 0x20000;
static NEXT: AtomicU32 = AtomicU32::new(0);

fn native(
    t: &mut ScriptToy,
    owner: i32,
    name: &str,
    params: &[Param],
    result: Option<(&str, i32)>,
) {
    t.function(
        owner,
        name,
        params,
        result,
        &[],
        &FunctionSpec::new(NATIVE, name),
        |_| None,
    );
}

fn package() -> Vec<u8> {
    let mut t = ScriptToy::new(&["Int", "Byte", "Float", "Bool", "Object", "Class"]);
    let object = t.class("Object", 0);
    let actor = t.class("Actor", object);
    t.prop("Byte", actor, "Role", 0, 0);
    let world = t.class("WorldInfo", actor);
    t.prop("Float", world, "TimeSeconds", 0, 0);
    let pool = t.class("ResourcePool", object);
    for name in [
        "PoolIdleDelayStartTime",
        "OnIdleRegenerationDelay",
        "OnIdleRegenerationDelayBaseValue",
        "CurrentValue",
        "MinValue",
        "MaxValue",
        "ActiveRegenerationRate",
        "OnIdleRegenerationRate",
        "PassiveRegenerationRate",
        "RateRemainder",
    ] {
        t.prop("Float", pool, name, 0, 0);
    }
    for name in ["bHasPoolBeenFullSinceLastBeingDepleted", "bIsAuthoritative"] {
        t.prop("Bool", pool, name, 0, 0);
    }
    t.prop("Byte", pool, "PoolGUID", 0, 0);
    t.prop("Object", pool, "Definition", 0, object);
    t.prop("Object", pool, "AssociatedProvider", 0, object);
    let marker = t.prop("Int", pool, "Marker", 0, 0);
    native(&mut t, pool, "ResetIdleTimer", &[], None);
    native(&mut t, pool, "GetTimeSeconds", &[], Some(("Float", 0)));
    native(
        &mut t,
        pool,
        "HasIdleDelayPassed",
        &[Param::new("Float", "CurrentTime")],
        Some(("Bool", 0)),
    );
    let note = t
        .function(
            object,
            "Note",
            &[],
            None,
            &[],
            &FunctionSpec::new(NATIVE, "Note"),
            |_| None,
        )
        .function;
    // The invented shutdown runs a native observer, then sets Marker. The observer can inject a failure.
    t.function(
        pool,
        "ShutDown",
        &[],
        None,
        &[],
        &FunctionSpec::new(SCRIPT, "ShutDown"),
        |_| {
            let mut a = Asm::new();
            a.stmt().final_function(note, |_| {});
            a.stmt().raw(&[0x0F]).instance(marker).int_const(77);
            a.stmt().return_nothing();
            a.end();
            Some(a)
        },
    );
    // `HealthResourcePool` is an Engine class in the installed packages; the other three recorded pools are
    // WillowGame's (`willow_package`). `AmmoResourcePool` here is an invented decoy: the same name in the wrong package.
    for name in ["HealthResourcePool", "UnrecordedPool", "AmmoResourcePool"] {
        t.class(name, pool);
    }
    // The shield hook's provider (lane IMPL-MAYA9-NATIVES): a controller with a pawn.
    let controller = t.class("Controller", actor);
    t.prop("Object", controller, "Pawn", 0, object);
    let manager = t.class("ResourcePoolManager", actor);
    t.prop("Object", manager, "Owner", 0, object);
    let payload = property_payload(&t.none, 16, 0, &ints(&[pool]));
    let mut slots = Export::new(t.import("ObjectProperty"), "ResourcePools", payload);
    slots.outer = manager;
    t.p.add_export_with(slots);
    native(&mut t, manager, "ShutDown", &[], None);
    t.p.build()
}

/// `WillowGame.upk` (invented): the three recorded pool classes the installed game declares there, and a further
/// shield subclass, each over the Engine package's `ResourcePool`.
fn willow_package() -> Vec<u8> {
    let mut t = ScriptToy::new(&["Int", "Byte", "Bool", "Object", "Array"]);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let pool = t.p.add_import("Core", "Class", engine, "ResourcePool");
    // This toy store's Object is Engine's (`package`).
    let object = t.p.add_import("Core", "Class", engine, "Object");
    let actor = t.p.add_import("Core", "Class", engine, "Actor");
    // The shield hook's walk (lane IMPL-MAYA9-NATIVES): a pawn's first equipped item, a shield and its definition.
    let pawn = t.class("WillowPawn", actor);
    ow_package::testkit::reflect::toy_array_of(&mut t, pawn, "EquippedItems", "Object", object);
    let definition = t.class("ShieldDefinition", object);
    t.prop("Byte", definition, "ShieldPrimeMechanism", 0, 0);
    let shield = t.class("WillowShield", object);
    t.prop("Object", shield, "ShieldDef", 0, object);
    t.prop("Bool", shield, "bPrimed", 0, 0);
    native(&mut t, shield, "ShieldPrimedStateChanged", &[], None);
    t.class("AmmoResourcePool", pool);
    t.class("ExperienceResourcePool", pool);
    let shield = t.class("ShieldResourcePool", pool);
    t.class("FurtherPool", shield);
    t.p.build()
}

struct Fx {
    world: World,
    _dir: crate::run::tests::Dir,
}
impl Fx {
    fn new(mode: ParityMode) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "ow-pool-natives-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let (bytes, willow) = (package(), willow_package());
        let defs = DefStore::from_synthetic(
            &dir,
            &[("Engine.upk", &bytes), ("WillowGame.upk", &willow)],
            IdentityPolicy::FirstInScanOrder,
            mode,
        )
        .unwrap();
        let vm = Vm::new(
            ObjectSpace::new(),
            defs,
            Rc::new(AttributeSystem::new_empty()),
            mode,
        )
        .with_stub_policy(StubPolicy::refuse_all());
        let snapshot = BalanceSnapshot::new(
            Playthrough::NORMAL,
            OpLevel::NONE,
            PlayerCount::SINGLE,
            ExpLevel::new(1),
            GameStage(1),
            AwesomeLevel::new(0),
            LevelCapUpgrades::ALL,
        )
        .unwrap();
        let world = World::new(vm, BalanceState::new(snapshot), &[&crate::hit::Pack]).unwrap();
        Self {
            world,
            _dir: crate::run::tests::Dir(dir),
        }
    }
    fn vm(&mut self) -> &mut Vm {
        &mut self.world.vm
    }
    fn make(&mut self, class: &str) -> ObjRef {
        // A bare name is an Engine class; a qualified one is taken as it is.
        let path = if class.contains('.') { class.to_string() } else { format!("Engine.{class}") };
        let class = self.vm().find_class(&path).unwrap();
        ow_vm::interp::instantiate_from_defaults(self.vm(), class).unwrap()
    }
    fn set(&mut self, o: ObjRef, name: &str, value: Value) {
        set_required(self.vm(), o, name, value, "test").unwrap();
    }
    fn time(&mut self, value: f32) -> ObjRef {
        let world = self.make("WorldInfo");
        self.set(world, "TimeSeconds", Value::Float(value));
        self.world
            .actors()
            .borrow_mut()
            .place_world_info(world)
            .unwrap();
        world
    }
    fn call(&mut self, path: &str, object: ObjRef, args: Vec<Value>) -> Result<Value, VmError> {
        let f = self.vm().find_function(&format!("Engine.{path}"))?;
        Ok(self.vm().call(f, Some(object), args)?.ret)
    }
    fn value(&mut self, o: ObjRef, name: &str) -> Value {
        get(self.vm(), o, name).unwrap().unwrap()
    }
    fn fill(&mut self, m: ObjRef, slot: u32, pool: ObjRef) {
        let vm = self.vm();
        let class = vm.space.heap.get(m).unwrap().class;
        let prop = vm.space.prop(class, "ResourcePools").unwrap();
        vm.space
            .write(m, PropAddr::element(prop, slot), Value::Object(Some(pool)))
            .unwrap();
    }
    fn snapshot(&mut self, pool: ObjRef) -> Vec<(String, Value)> {
        [
            "CurrentValue",
            "MinValue",
            "MaxValue",
            "ActiveRegenerationRate",
            "OnIdleRegenerationRate",
            "PassiveRegenerationRate",
            "RateRemainder",
            "OnIdleRegenerationDelay",
            "OnIdleRegenerationDelayBaseValue",
            "bHasPoolBeenFullSinceLastBeingDepleted",
            "bIsAuthoritative",
            "Definition",
            "AssociatedProvider",
            "PoolGUID",
        ]
        .into_iter()
        .map(|n| (n.to_string(), self.value(pool, n)))
        .collect()
    }
}

fn run(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn reset_stamps_only_the_timer_for_every_recorded_empty_hook() {
    run(|| {
        let mut x = Fx::new(ParityMode::Off);
        x.time(18.25);
        for class in [
            "ResourcePool",
            "HealthResourcePool",
            "WillowGame.AmmoResourcePool",
            "WillowGame.ExperienceResourcePool",
        ] {
            let pool = x.make(class);
            for (i, name) in [
                "CurrentValue",
                "MaxValue",
                "MinValue",
                "ActiveRegenerationRate",
                "OnIdleRegenerationRate",
                "RateRemainder",
            ]
            .into_iter()
            .enumerate()
            {
                x.set(pool, name, Value::Float(i as f32 + 2.5));
            }
            x.set(
                pool,
                "bHasPoolBeenFullSinceLastBeingDepleted",
                Value::Bool(true),
            );
            x.set(pool, "bIsAuthoritative", Value::Bool(false)); // Reset has no authority gate.
            let before = x.snapshot(pool);
            x.call("ResourcePool.ResetIdleTimer", pool, vec![]).unwrap();
            assert_eq!(x.value(pool, "PoolIdleDelayStartTime"), Value::Float(18.25));
            assert_eq!(x.snapshot(pool), before);
        }
    });
}

#[test]
fn shield_and_unknown_hooks_refuse_after_the_stamp_without_other_pool_mutations() {
    run(|| {
        let mut x = Fx::new(ParityMode::Off);
        x.time(21.5);
        // The Engine decoy `AmmoResourcePool` refuses: only the installed package's identity is recorded.
        // The shield's hook has its rule now (lane IMPL-MAYA9-NATIVES): `shield_hook_tests`.
        for class in [
            "UnrecordedPool",
            "WillowGame.FurtherPool",
            "AmmoResourcePool",
        ] {
            let pool = x.make(class);
            let definition = x.make("Object");
            let provider = x.make("Object");
            x.set(pool, "Definition", Value::Object(Some(definition)));
            x.set(pool, "AssociatedProvider", Value::Object(Some(provider)));
            x.set(pool, "CurrentValue", Value::Float(4.5));
            x.set(
                pool,
                "bHasPoolBeenFullSinceLastBeingDepleted",
                Value::Bool(true),
            );
            let before = x.snapshot(pool);
            let err = x
                .call("ResourcePool.ResetIdleTimer", pool, vec![])
                .unwrap_err();
            // Typed: the note leaves these hooks unread, so the kind is NotRead (review r1 of #482).
            let VmError::NotYet(n) = &err else { panic!("a NotYet: {err}") };
            assert_eq!(n.missing, ow_core::Missing::NotRead, "{err}");
            assert_eq!(n.record, RECORD, "{err}");
            assert_eq!(x.value(pool, "PoolIdleDelayStartTime"), Value::Float(21.5));
            assert_eq!(x.snapshot(pool), before);
        }
    });
}

#[test]
fn time_reads_the_represented_world_and_unmodelled_world_refuses_before_mutation() {
    run(|| {
        let mut x = Fx::new(ParityMode::Off);
        let pool = x.make("ResourcePool");
        x.set(pool, "PoolIdleDelayStartTime", Value::Float(7.0));
        assert!(matches!(
            x.call("ResourcePool.GetTimeSeconds", pool, vec![]),
            Err(VmError::NotYet(_))
        ));
        assert!(matches!(
            x.call("ResourcePool.ResetIdleTimer", pool, vec![]),
            Err(VmError::NotYet(_))
        ));
        assert_eq!(x.value(pool, "PoolIdleDelayStartTime"), Value::Float(7.0));
        let world = x.time(6.75);
        assert_eq!(
            x.call("ResourcePool.GetTimeSeconds", pool, vec![]).unwrap(),
            Value::Float(6.75)
        );
        x.set(world, "TimeSeconds", Value::Float(9.25));
        assert_eq!(
            x.call("ResourcePool.GetTimeSeconds", pool, vec![]).unwrap(),
            Value::Float(9.25)
        );
        assert_eq!(x.value(pool, "PoolIdleDelayStartTime"), Value::Float(7.0));
        // A standalone VM needs an explicit no-world answer. Missing query/service is unknown, not absent.
        let mut vm = Vm::new(
            ObjectSpace::new(),
            DefStore::new_empty(),
            Rc::new(AttributeSystem::new_empty()),
            ParityMode::Off,
        );
        assert!(matches!(vm.world_time_seconds(), Err(VmError::NotYet(_))));
        vm.add_service(WorldTimeQuery(|_| Ok(None))).unwrap();
        assert_eq!(vm.world_time_seconds().unwrap(), 0.0);
        // Use that same explicit absence query for reset and GetTimeSeconds, without substituting a clock value.
        // Install an explicit absent query in x's existing query cell.
        *x.vm().service::<WorldTimeQuery>().unwrap().borrow_mut() = WorldTimeQuery(|_| Ok(None));
        assert_eq!(
            x.call("ResourcePool.GetTimeSeconds", pool, vec![]).unwrap(),
            Value::Float(0.0)
        );
        x.call("ResourcePool.ResetIdleTimer", pool, vec![]).unwrap();
        assert_eq!(x.value(pool, "PoolIdleDelayStartTime"), Value::Float(0.0));
    });
}

#[test]
fn delay_is_strict_f32_current_attribute_and_only_clears_a_passed_stamp() {
    run(|| {
        let mut x = Fx::new(ParityMode::Off); // No WorldInfo needed: caller supplies the time.
        let pool = x.make("ResourcePool");
        x.set(pool, "OnIdleRegenerationDelay", Value::Float(3.0));
        x.set(
            pool,
            "OnIdleRegenerationDelayBaseValue",
            Value::Float(300.0),
        );
        let before = x.snapshot(pool);
        for (stamp, now, passed, after) in [
            (10.0, 12.9, false, 10.0),
            (10.0, f32::from_bits(13.0_f32.to_bits() - 1), false, 10.0),
            (10.0, 13.0, true, 0.0),
            (0.0, -9.0, true, 0.0),
            (-2.0, -9.0, true, 0.0),
            (0.0, 0.0, true, 0.0),
            (f32::NAN, 0.0, true, 0.0),
            (10.0, f32::NAN, true, 0.0),
        ] {
            x.set(pool, "PoolIdleDelayStartTime", Value::Float(stamp));
            assert_eq!(
                x.call(
                    "ResourcePool.HasIdleDelayPassed",
                    pool,
                    vec![Value::Float(now)]
                )
                .unwrap(),
                Value::Bool(passed)
            );
            assert_eq!(x.value(pool, "PoolIdleDelayStartTime"), Value::Float(after));
            assert_eq!(x.snapshot(pool), before);
        }
        // At this magnitude the stored-float sum rounds 2^24 + 1 down to 2^24. A double sum would return false.
        x.set(pool, "PoolIdleDelayStartTime", Value::Float(16_777_216.0));
        x.set(pool, "OnIdleRegenerationDelay", Value::Float(1.0));
        assert_eq!(
            x.call(
                "ResourcePool.HasIdleDelayPassed",
                pool,
                vec![Value::Float(16_777_216.0)]
            )
            .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            x.call(
                "ResourcePool.HasIdleDelayPassed",
                pool,
                vec![Value::Float(-100.0)]
            )
            .unwrap(),
            Value::Bool(true)
        );
    });
}

#[test]
fn shutdown_observes_authority_slot_order_and_stops_on_the_first_script_error() {
    run(|| {
        for fail in [false, true] {
            let mut x = Fx::new(ParityMode::Off);
            let manager = x.make("ResourcePoolManager");
            let mut pools = Vec::new();
            for i in 0..16 {
                let p = x.make("ResourcePool");
                x.fill(manager, i, p);
                pools.push(p);
            }
            let seen = Rc::new(RefCell::new(Vec::new()));
            let sink = seen.clone();
            let order = pools.clone();
            x.vm()
                .register_native(
                    "Object.Note()",
                    Rc::new(move |c| {
                        let pool = c.this.unwrap();
                        let i = order.iter().position(|p| *p == pool).unwrap();
                        // The slot still holds the pool during its script ShutDown.
                        assert_eq!(
                            ow_world::pools::slot(&c.vm.space, manager, i as u8)?,
                            Some(pool)
                        );
                        sink.borrow_mut().push(pool);
                        if fail && i == 4 {
                            return Err(super::super::access::error("invented shutdown failure"));
                        }
                        Ok(Value::None)
                    }),
                )
                .unwrap();
            for role in [0, 1, 2, 4] {
                x.set(manager, "Role", Value::Byte(role));
                x.call("ResourcePoolManager.ShutDown", manager, vec![])
                    .unwrap();
                assert!(seen.borrow().is_empty());
                assert_eq!(
                    ow_world::pools::pools_of(&x.vm().space, manager)
                        .unwrap()
                        .len(),
                    16
                );
            }
            x.set(manager, "Role", Value::Byte(3));
            let result = x.call("ResourcePoolManager.ShutDown", manager, vec![]);
            if fail {
                assert!(
                    result
                        .unwrap_err()
                        .to_string()
                        .contains("invented shutdown failure")
                );
                assert_eq!(*seen.borrow(), pools[..5]);
                assert_eq!(
                    ow_world::pools::pools_of(&x.vm().space, manager)
                        .unwrap()
                        .iter()
                        .map(|(i, _)| *i)
                        .collect::<Vec<_>>(),
                    (4..16).collect::<Vec<_>>()
                );
                assert_eq!(x.value(pools[4], "Marker"), Value::Int(0));
            } else {
                result.unwrap();
                assert_eq!(*seen.borrow(), pools);
                assert!(
                    ow_world::pools::pools_of(&x.vm().space, manager)
                        .unwrap()
                        .is_empty()
                );
            }
            assert_eq!(x.value(pools[0], "Marker"), Value::Int(77));
        }
    });
}

#[test]
fn reference_keeps_its_stamp_and_existing_unbound_natives() {
    run(|| {
        let mut x = Fx::new(ParityMode::Reference);
        x.time(12.5);
        let pool = x.make("WillowGame.ShieldResourcePool");
        x.call("ResourcePool.ResetIdleTimer", pool, vec![]).unwrap();
        assert_eq!(x.value(pool, "PoolIdleDelayStartTime"), Value::Float(12.5));
        for key in [
            "ResourcePoolManager.ShutDown()",
            "ResourcePool.GetTimeSeconds()",
            "ResourcePool.HasIdleDelayPassed(float)",
        ] {
            assert!(!x.vm().has_native(key), "Reference binding changed: {key}");
        }
    });
}

/// NATIVE_MAYA9_AND_AFTER.md section 3.2, test 7 (lane IMPL-MAYA9-NATIVES): the shield's idle-reset hook.
mod shield_hook_tests {
    use super::*;

    /// A shield pool on a controller whose pawn wears a shield of mechanism `mechanism`, `bPrimed` = `primed`.
    fn shield_pool(x: &mut Fx, mechanism: u8, primed: bool) -> (ObjRef, ObjRef) {
        let pool = x.make("WillowGame.ShieldResourcePool");
        let definition = x.make("Object");
        let controller = x.make("Controller");
        let pawn = x.make("WillowGame.WillowPawn");
        let shield = x.make("WillowGame.WillowShield");
        let def = x.make("WillowGame.ShieldDefinition");
        x.set(pool, "Definition", Value::Object(Some(definition)));
        x.set(pool, "AssociatedProvider", Value::Object(Some(controller)));
        x.set(controller, "Pawn", Value::Object(Some(pawn)));
        x.set(pawn, "EquippedItems", Value::array(vec![Value::Object(Some(shield))]));
        x.set(shield, "ShieldDef", Value::Object(Some(def)));
        x.set(shield, "bPrimed", Value::Bool(primed));
        x.set(def, "ShieldPrimeMechanism", Value::Byte(mechanism));
        (pool, shield)
    }

    fn events(x: &mut Fx) -> Rc<RefCell<u32>> {
        let n = Rc::new(RefCell::new(0u32));
        let seen = n.clone();
        let f = x.vm().find_function("WillowGame.WillowShield.ShieldPrimedStateChanged").unwrap();
        x.vm()
            .register_native_for(f, Rc::new(move |_c: &mut ow_vm::api::NativeCall<'_>| {
                *seen.borrow_mut() += 1;
                Ok(Value::None)
            }))
            .unwrap();
        n
    }

    #[test]
    fn the_table_by_mechanism_with_its_edges_and_a_nan() {
        let nan = f32::NAN;
        // (mechanism, C, Min, Max, F) -> primed.
        let cases = [
            (1, 10.0, 0.0, 10.0, true, true),
            (1, 9.0, 0.0, 10.0, true, false),
            (2, 0.0, 0.0, 10.0, true, true),
            (2, 5.0, 0.0, 10.0, false, true),
            (2, 5.0, 0.0, 10.0, true, false),
            (3, 0.0, 0.0, 10.0, true, true),
            (3, 0.5, 0.0, 10.0, true, false),
            (4, nan, nan, nan, false, true),
            (5, 0.5, 0.0, 10.0, true, true),
            (5, 0.0, 0.0, 10.0, true, false),
            (6, 0.5, 0.0, 10.0, true, true),
            (6, 0.5, 0.0, 10.0, false, false),
            (1, nan, 0.0, 10.0, true, false),
            (3, nan, 0.0, 10.0, true, false),
            (5, nan, 0.0, 10.0, true, false),
            (0, 5.0, 0.0, 10.0, true, false),
            (7, 5.0, 0.0, 10.0, true, false),
        ];
        for (m, c, min, max, f, want) in cases {
            assert_eq!(primed(m, c, min, max, f), want, "{m} {c} {min} {max} {f}");
        }
    }

    #[test]
    fn a_change_writes_bprimed_and_raises_the_event_once() {
        run(|| {
            let mut x = Fx::new(ParityMode::Off);
            x.time(3.0);
            let n = events(&mut x);
            let (pool, shield) = shield_pool(&mut x, 4, false);
            x.call("ResourcePool.ResetIdleTimer", pool, vec![]).unwrap();
            assert_eq!(x.value(shield, "bPrimed"), Value::Bool(true));
            assert_eq!(*n.borrow(), 1);
            assert_eq!(x.value(pool, "PoolIdleDelayStartTime"), Value::Float(3.0), "the stamp");
            // No change: no event.
            x.call("ResourcePool.ResetIdleTimer", pool, vec![]).unwrap();
            assert_eq!(*n.borrow(), 1);
        });
    }

    #[test]
    fn no_definition_or_no_shield_does_nothing_after_the_stamp() {
        run(|| {
            let mut x = Fx::new(ParityMode::Off);
            x.time(4.0);
            let n = events(&mut x);
            let (pool, shield) = shield_pool(&mut x, 4, false);
            x.set(pool, "Definition", Value::Object(None));
            x.call("ResourcePool.ResetIdleTimer", pool, vec![]).unwrap();
            assert_eq!(x.value(shield, "bPrimed"), Value::Bool(false));
            let (pool, shield) = shield_pool(&mut x, 4, false);
            let pawn = {
                let c = get(x.vm(), pool, "AssociatedProvider").unwrap().unwrap();
                let Value::Object(Some(c)) = c else { panic!() };
                let Value::Object(Some(p)) = get(x.vm(), c, "Pawn").unwrap().unwrap() else { panic!() };
                p
            };
            x.set(pawn, "EquippedItems", Value::array(vec![]));
            x.call("ResourcePool.ResetIdleTimer", pool, vec![]).unwrap();
            assert_eq!(x.value(shield, "bPrimed"), Value::Bool(false));
            assert_eq!(x.value(pool, "PoolIdleDelayStartTime"), Value::Float(4.0));
            assert_eq!(*n.borrow(), 0);
        });
    }

    #[test]
    fn a_shield_without_a_definition_is_refused_before_the_stamp() {
        run(|| {
            let mut x = Fx::new(ParityMode::Off);
            x.time(5.0);
            let (pool, shield) = shield_pool(&mut x, 4, false);
            x.set(shield, "ShieldDef", Value::Object(None));
            let before = x.value(pool, "PoolIdleDelayStartTime");
            let err = x.call("ResourcePool.ResetIdleTimer", pool, vec![]).unwrap_err();
            let VmError::NotYet(n) = &err else { panic!("{err}") };
            assert_eq!((n.missing.clone(), &*n.what), (ow_core::Missing::NotRead, SHIELD_NO_DEFINITION));
            assert_eq!(x.value(pool, "PoolIdleDelayStartTime"), before, "no stamp");
        });
    }
}
