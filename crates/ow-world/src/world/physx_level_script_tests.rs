//! NATIVE_GETPHYSXLEVEL_AND_AFTER.md section 5's tests 6 and 7 (review of #773, LOW b): a synthetic script shaped as
//! `SetupSkeletalMeshPhysicsForPhysXEffects`' gate, `if (GetPhysXLevel() > 0) { Recorder(true) }`, run by the VM on a
//! toy Core package (AGENTS.md rule 9: invented classes and values). The `>` is the core operator (native 151,
//! answered by `ow_vm::natives_core`), `GetPhysXLevel` this module's binding, and the recorder stands in for
//! `SetHasPhysicsAssetInstance(true)`.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::ParityMode;
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::script::{Asm, FunctionSpec, Param, ScriptToy};
use ow_vm::api::{NativeCall, StubPolicy, Vm};

use super::*;

static NEXT: AtomicU32 = AtomicU32::new(0);
/// Native, operator, and the flags the testkit's own operator example uses.
const OPERATOR: u32 = 0x400 | 0x1000 | 0x23000;
/// Native, static, final, public.
const STATIC_FINAL_NATIVE: u32 = 0x400 | 0x2000 | 0x1 | 0x20000;
const NATIVE: u32 = 0x400 | 0x20000;
const SCRIPT: u32 = 0x1 | 0x2 | 0x20000;

fn package() -> Vec<u8> {
    let mut t = ScriptToy::new(&["Int", "Bool"]);
    let object = t.class("Object", 0);
    t.function(
        object,
        "GreaterThan_IntInt",
        &[Param::new("Int", "A"), Param::new("Int", "B")],
        Some(("Bool", 0)),
        &[],
        &FunctionSpec { native: 151, ..FunctionSpec::new(OPERATOR, ">") },
        |_| None,
    );
    let engine = t.class("Engine", object);
    t.prop("Int", engine, "PhysXLevel", 0, 0);
    let level = t
        .function(engine, "GetPhysXLevel", &[], Some(("Int", 0)), &[], &FunctionSpec::new(STATIC_FINAL_NATIVE, "GetPhysXLevel"), |_| None)
        .function;
    let recorder = t
        .function(engine, "ToyRecorder", &[Param::new("Bool", "bHasInstance")], None, &[], &FunctionSpec::new(NATIVE, "ToyRecorder"), |_| None)
        .function;
    // if (GetPhysXLevel() > 0) { ToyRecorder(true); } return;
    t.function(engine, "ToySetupPhysXEffects", &[], None, &[], &FunctionSpec::new(SCRIPT, "ToySetupPhysXEffects"), |_| {
        let mut a = Asm::new();
        a.stmt();
        let skip = a.jump_if_not(|a| {
            a.native(151, |a| {
                a.final_function(level, |_| {});
                a.raw(&[0x25]); // IntZero
            });
        });
        a.stmt().final_function(recorder, |a| {
            a.raw(&[0x27]); // True
        });
        a.patch(skip);
        a.stmt().return_nothing();
        a.end();
        Some(a)
    });
    t.build()
}

struct Run {
    vm: Vm,
    dir: PathBuf,
    engine: ObjRef,
    calls: Rc<RefCell<Vec<Value>>>,
}

impl Drop for Run {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn start() -> Run {
    let dir = std::env::temp_dir().join(format!("ow-world-physx-script-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let defs = DefStore::from_synthetic(&dir, &[("Core.upk", &package())], IdentityPolicy::FirstInScanOrder, ParityMode::Off).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off)
        .with_stub_policy(StubPolicy::refuse_all());
    ow_vm::natives_core::register(&mut vm).unwrap();
    register(&mut vm).unwrap();
    let calls = Rc::new(RefCell::new(Vec::new()));
    let seen = calls.clone();
    vm.register_native(
        "Engine.ToyRecorder(bool)",
        Rc::new(move |c: &mut NativeCall<'_>| {
            seen.borrow_mut().push(c.arg(0)?.clone());
            Ok(Value::None)
        }),
    )
    .unwrap();
    let class = vm.find_class("Core.Engine").unwrap();
    let engine = ow_vm::interp::instantiate_from_defaults(&mut vm, class).unwrap();
    vm.add_service(EngineObject(engine)).unwrap();
    Run { vm, dir, engine, calls }
}

impl Run {
    fn level(&mut self, level: i32) {
        let class = self.vm.space.heap.get(self.engine).unwrap().class;
        let p = self.vm.space.prop(class, "PhysXLevel").unwrap();
        self.vm.space.write(self.engine, PropAddr::scalar(p), Value::Int(level)).unwrap();
    }

    fn setup(&mut self) {
        let f = self.vm.find_function("Core.Engine.ToySetupPhysXEffects").unwrap();
        self.vm.call(f, Some(self.engine), vec![]).unwrap();
    }
}

/// Test 6: at level 0 the script returns after the level test; the recorder is not called.
#[test]
fn at_level_0_the_gate_calls_nothing() {
    let mut r = start();
    r.level(0);
    r.setup();
    assert!(r.calls.borrow().is_empty(), "{:?}", r.calls.borrow());
}

/// Test 7: at level 1 the recorder is called once, with true: the gate is "greater than 0", not "equals 2".
#[test]
fn at_level_1_the_gate_asks_for_the_instance_once() {
    let mut r = start();
    r.level(1);
    r.setup();
    assert_eq!(*r.calls.borrow(), vec![Value::Bool(true)]);
}
