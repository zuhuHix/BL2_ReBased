//! Tests of the class route's two natives (NATIVE_PLAYER_CLASS.md sections 2 and 3) on an invented `GearboxFramework`
//! package (AGENTS.md rule 9: invented classes, objects and values), in both parity modes.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::notyet::Missing;
use ow_core::{ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ClassId, ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::script::{Asm, FunctionSpec, Param, ScriptToy};
use ow_vm::api::{NativePack, Vm};
use ow_vm::error::VmError;

use super::*;
use crate::world::pawn_data;

const SCRIPT: u32 = 0x1 | 0x2 | 0x20000;
const NATIVE: u32 = 0x400 | 0x20000;
const STATIC: u32 = 0x2000;
static NEXT: AtomicU32 = AtomicU32::new(0);

/// The fields the toy context's `RunStreamingDataEvent` writes, in its parameters' order.
const GOT: [&str; 5] = ["GotName", "GotPayload", "GotFound", "GotInt", "GotFloat"];

fn package() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&[
        "Class",
        "Function",
        "ObjectProperty",
        "ClassProperty",
        "StrProperty",
        "NameProperty",
        "IntProperty",
        "FloatProperty",
    ]);
    let object = t.class("Object", 0);
    let native = |t: &mut ScriptToy, owner: i32, name: &str, params: &[Param], result, flags| {
        t.function(
            owner,
            name,
            params,
            result,
            &[],
            &FunctionSpec::new(flags, name),
            |_| None,
        );
    };
    let definition = t.class("GBXDefinition", object);
    let get = t.function(
        definition,
        "GetDefinition",
        &[
            Param {
                type_ref: object,
                ..Param::new("Class", "DefinitionClass")
            },
            Param::new("Str", "DefinitionName"),
        ],
        Some(("Object", object)),
        &[],
        &FunctionSpec::new(NATIVE | STATIC, "GetDefinition"),
        |_| None,
    );
    // A ClassProperty names its class and its meta class: the toy helper writes one word, so the parameter's payload
    // is written in full here (the shape ow-vm's tests use).
    let tail = ow_package::testkit::ints(&[t.import("Class"), object]);
    let none = t.none;
    t.p.set_payload(
        get.get("DefinitionClass"),
        ow_package::testkit::reflect::property_payload(&none, 1, 0x80, &tail),
    );
    let class_definition = t.class("PlayerClassDefinition", definition);
    let thing = t.class("Thing", object);
    let manager = t.class("WillowPlayerPawnDataManager", object);
    let obj = |name: &str| Param {
        type_ref: object,
        ..Param::new("Object", name)
    };
    native(
        &mut t,
        manager,
        "LoadPlayerPawnDataAsync",
        &[
            Param::new("Str", "PawnDataPath"),
            obj("PlayerContext"),
            obj("Payload"),
            obj("Instigator"),
            Param::new("Int", "IntParam"),
            Param::new("Float", "FloatParam"),
            Param::new("Name", "CallbackCustomEventName"),
        ],
        None,
        NATIVE,
    );
    let report = |t: &mut ScriptToy, owner: i32, fields: Option<[i32; 5]>| {
        let params = [
            Param::new("Name", "EventName"),
            obj("EventInstigator"),
            obj("OtherEventParticipant"),
            Param::new("Int", "IntParam"),
            Param::new("Float", "FloatParam"),
        ];
        t.function(
            owner,
            "RunStreamingDataEvent",
            &params,
            None,
            &[],
            &FunctionSpec::new(SCRIPT, "RunStreamingDataEvent"),
            |ids| {
                let mut a = Asm::new();
                if let Some(fields) = fields {
                    let names = [
                        "EventName",
                        "EventInstigator",
                        "OtherEventParticipant",
                        "IntParam",
                        "FloatParam",
                    ];
                    for (field, param) in fields.into_iter().zip(names) {
                        a.stmt().raw(&[0x0F]).instance(field).local(ids.get(param));
                    }
                }
                a.stmt().return_nothing();
                a.end();
                Some(a)
            },
        );
    };
    // The interface, named and shaped as the installed one (one function); the structural rule finds implementers.
    let iface = t.class("IStreamingDataEvent", object);
    report(&mut t, iface, None);
    let context = t.class("ToyController", object);
    let kinds = ["Name", "Object", "Object", "Int", "Float"];
    let mut fields = [0i32; 5];
    for (i, (name, kind)) in GOT.iter().zip(kinds).enumerate() {
        fields[i] = t.prop(
            kind,
            context,
            name,
            0,
            if kind == "Object" { object } else { 0 },
        );
    }
    report(&mut t, context, Some(fields));
    t.class("ToyBystander", object);
    // Invented data exports: a class definition and a pawn archetype.
    for (class, name) in [
        (class_definition, "ToyClassDef"),
        (thing, "ToyPawn"),
        (thing, "ToyOther"),
    ] {
        let mut payload = 0i32.to_le_bytes().to_vec();
        payload.extend_from_slice(&t.none);
        t.p.add_export(class, name, payload);
    }
    t.p.build()
}

struct Fx {
    vm: Vm,
    dir: PathBuf,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fx(mode: ParityMode) -> Fx {
    let dir = std::env::temp_dir().join(format!(
        "ow-world-class-route-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let bytes = package();
    let defs = DefStore::from_synthetic(
        &dir,
        &[("GearboxFramework.upk", &bytes)],
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
    pawn_data::Pack.register(&mut vm).unwrap();
    Fx { vm, dir }
}

impl Fx {
    fn class(&mut self, name: &str) -> ClassId {
        self.vm
            .find_class(&format!("GearboxFramework.{name}"))
            .unwrap()
    }

    fn make(&mut self, name: &str) -> ObjRef {
        let c = self.class(name);
        ow_vm::interp::instantiate_from_defaults(&mut self.vm, c).unwrap()
    }

    fn load(&mut self, path: &str) -> ObjRef {
        let p = ObjectPath::parse(path).unwrap();
        self.vm.defs.find(&mut self.vm.space, &p).unwrap().unwrap()
    }

    fn get_definition(&mut self, class: Option<ClassId>, name: &str) -> Result<Value, VmError> {
        let f = self
            .vm
            .find_function("GearboxFramework.GBXDefinition.GetDefinition")?;
        Ok(self
            .vm
            .call(f, None, vec![Value::Class(class), Value::Str(name.into())])?
            .ret)
    }

    fn load_async(
        &mut self,
        manager: ObjRef,
        path: &str,
        context: Option<ObjRef>,
        payload: Option<ObjRef>,
    ) -> Result<Value, VmError> {
        let f = self.vm.find_function(
            "GearboxFramework.WillowPlayerPawnDataManager.LoadPlayerPawnDataAsync",
        )?;
        let event = Value::Name(self.vm.space.names.intern("SwitchPlayerClass"));
        let args = vec![
            Value::Str(path.into()),
            Value::Object(context),
            Value::Object(payload),
            Value::Object(None),
            Value::Int(7),
            Value::Float(0.5),
            event,
        ];
        Ok(self.vm.call(f, Some(manager), args)?.ret)
    }

    fn got(&self, o: ObjRef) -> Vec<Value> {
        let class = self.vm.space.heap.get(o).unwrap().class;
        GOT.iter()
            .map(|n| {
                let p = self.vm.space.prop(class, n).unwrap();
                self.vm.space.read(o, PropAddr::scalar(p)).unwrap().clone()
            })
            .collect()
    }
}

fn not_read(r: &Result<Value, VmError>) -> bool {
    matches!(r, Err(VmError::NotYet(n)) if n.missing == Missing::NotRead)
}

#[test]
fn get_definition_finds_in_memory_then_loads_by_name() {
    let mut x = fx(ParityMode::Off);
    let class = x.class("PlayerClassDefinition");
    assert_eq!(
        x.get_definition(None, "GearboxFramework.ToyClassDef")
            .unwrap(),
        Value::Object(None),
        "no class"
    );
    assert_eq!(
        x.get_definition(Some(class), "").unwrap(),
        Value::Object(None),
        "an empty name"
    );
    // Not in memory yet: the store loads it by that name; asked again, it is the same object.
    let loaded = x
        .get_definition(Some(class), "GearboxFramework.ToyClassDef")
        .unwrap();
    let Value::Object(Some(o)) = loaded else {
        panic!("{loaded:?}")
    };
    assert!(!x.vm.defs.is_unloaded(&x.vm.space, o), "loaded");
    assert_eq!(
        x.get_definition(Some(class), "gearboxframework.TOYCLASSDEF")
            .unwrap(),
        Value::Object(Some(o)),
        "any case"
    );
    // Nothing anywhere at that name: the retry is not read. A bare name: how it resolves in any package is not read.
    assert!(not_read(
        &x.get_definition(Some(class), "GearboxFramework.Missing")
    ));
    let bare = x.get_definition(Some(class), "ToyClassDef");
    assert!(not_read(&bare), "a bare name: {bare:?}");
    // An object at the path of another class: the lookup's class filter is not read.
    let thing = x.class("Thing");
    assert!(not_read(
        &x.get_definition(Some(thing), "GearboxFramework.ToyClassDef")
    ));
    let base = x.class("GBXDefinition");
    assert!(
        not_read(&x.get_definition(Some(base), "GearboxFramework.ToyClassDef")),
        "a derived class"
    );
}

#[test]
fn load_async_reports_at_once_for_an_object_in_memory() {
    let mut x = fx(ParityMode::Off);
    let manager = x.make("WillowPlayerPawnDataManager");
    let context = x.make("ToyController");
    let payload = x.load("GearboxFramework.ToyClassDef");
    let pawn = x.load("GearboxFramework.ToyPawn");
    assert_eq!(
        x.load_async(
            manager,
            "GearboxFramework.ToyPawn",
            Some(context),
            Some(payload)
        )
        .unwrap(),
        Value::None
    );
    let event = Value::Name(x.vm.space.names.intern("SwitchPlayerClass"));
    assert_eq!(
        x.got(context),
        vec![
            event,
            Value::Object(Some(payload)),
            Value::Object(Some(pawn)),
            Value::Int(7),
            Value::Float(0.5)
        ],
        "name, payload as the instigator, the found object, int, float"
    );
}

#[test]
fn load_async_without_a_package_part_does_nothing_and_the_rest_is_not_yet() {
    let mut x = fx(ParityMode::Off);
    let manager = x.make("WillowPlayerPawnDataManager");
    let context = x.make("ToyController");
    let empty = x.got(context);
    // No package part: no report, no error.
    assert_eq!(
        x.load_async(manager, "ToyPawn", Some(context), None)
            .unwrap(),
        Value::None
    );
    assert_eq!(x.got(context), empty);
    // Held by a package but not loaded, and held by nothing: the load and its report are not read.
    assert!(not_read(&x.load_async(
        manager,
        "GearboxFramework.ToyOther",
        Some(context),
        None
    )));
    assert!(not_read(&x.load_async(
        manager,
        "GearboxFramework.Missing",
        Some(context),
        None
    )));
    assert_eq!(x.got(context), empty, "no report");
    // In memory, but the context does not implement the interface: a "no" is not proven here.
    x.load("GearboxFramework.ToyPawn");
    let bystander = x.make("ToyBystander");
    assert!(not_read(&x.load_async(
        manager,
        "GearboxFramework.ToyPawn",
        Some(bystander),
        None
    )));
    // In memory, no context: nothing to report to.
    assert_eq!(
        x.load_async(manager, "GearboxFramework.ToyPawn", None, None)
            .unwrap(),
        Value::None
    );
}

#[test]
fn reference_leaves_both_unbound() {
    let x = fx(ParityMode::Reference);
    assert!(!x.vm.has_native(GET_DEFINITION));
    assert!(!x.vm.has_native(LOAD_ASYNC));
}
