//! Tests of the behavior-variable setters (decision 2026-11-19-NNN) on an invented `GearboxFramework` package
//! (AGENTS.md rule 9): the value struct, its type enum in an order unlike the game's, the five natives, and a script
//! caller that grows a local array and fills it as link 9's caller does. AI-assisted (Claude Code).

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::registry::AttributeSystem;
use ow_core::ParityMode;
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, FuncId, ObjRef, PropAddr, StructId};
use ow_object::layout::PropKind;
use ow_object::value::{StructValue, Value};
use ow_package::testkit::reflect::{toy_array_of, toy_struct};
use ow_package::testkit::script::{Asm, CPF_OUT, FunctionSpec, Param, ScriptToy, function_payload};
use ow_vm::api::{NativePack, Vm};
use ow_vm::error::VmError;

use super::*;
use crate::kernel::natives::Pack;

/// `FUNC_Final | FUNC_Native | FUNC_Static | FUNC_Public`.
const NATIVE_STATIC: u32 = 0x1 | 0x400 | 0x2000 | 0x20000;
const SCRIPT: u32 = 0x1 | 0x2 | 0x20000;

/// The toy enum, in an order unlike the game's: a rule that wrote the game's constants would fail here.
const BVARS: [&str; 6] = ["BVAR_Object", "BVAR_None", "BVAR_Vector", "BVAR_Int", "BVAR_Bool", "BVAR_Float"];

static NEXT: AtomicU32 = AtomicU32::new(0);

struct Toy {
    vm: Vm,
    object: ObjRef,
    dir: PathBuf,
    bvv: StructId,
    vector: StructId,
    caller: ClassId,
}

impl Drop for Toy {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn enum_export(t: &mut ScriptToy, outer: i32, name: &str, values: &[&str]) -> i32 {
    let mut payload = ow_package::testkit::ints(&[0]);
    payload.extend_from_slice(&t.none);
    payload.extend(ow_package::testkit::ints(&[0, values.len() as i32]));
    for v in values {
        let bytes = t.p.fname(v, 0);
        payload.extend_from_slice(&bytes);
    }
    let mut e = ow_package::testkit::Export::new(t.import("Enum"), name, payload);
    e.outer = outer;
    t.p.add_export_with(e)
}

/// The invented package. `broken` declares `SetIntBehaviorVariable`'s first argument as a plain int (not out, not the
/// struct), for the schema refusal.
fn package(broken: bool) -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&[
        "Class",
        "Function",
        "ScriptStruct",
        "IntProperty",
        "FloatProperty",
        "BoolProperty",
        "ByteProperty",
        "ObjectProperty",
        "ArrayProperty",
        "StructProperty",
        "Enum",
    ]);
    let object = t.class("Object", 0);
    let vector = toy_struct(&mut t, object, "Vector", None);
    for f in ["X", "Y", "Z"] {
        t.prop("Float", vector, f, 0, 0);
    }
    let bpd = t.class("BehaviorProviderDefinition", object);
    let kinds = enum_export(&mut t, bpd, "EBehaviorVariableType", &BVARS);
    let bvv = toy_struct(&mut t, bpd, "BehaviorVariableValue", None);
    // Fields in an invented order with one the setters never touch, so "the other fields" is not empty.
    t.prop("Int", bvv, "ToyTag", 0, 0);
    t.prop("Object", bvv, "ObjectValue", 0, object);
    t.prop("Byte", bvv, "VariableType", 0, kinds);
    t.prop("Float", bvv, "FloatValue", 0, 0);
    t.prop("Struct", bvv, "VectorValue", 0, vector);
    t.prop("Int", bvv, "IntValue", 0, 0);
    let variable = Param { flags: CPF_OUT, type_ref: bvv, ..Param::new("Struct", "Variable") };
    let setters: [(&str, Param); 5] = [
        ("SetBoolBehaviorVariable", Param::new("Bool", "Value")),
        ("SetIntBehaviorVariable", Param::new("Int", "Value")),
        ("SetFloatBehaviorVariable", Param::new("Float", "Value")),
        ("SetVectorBehaviorVariable", Param { type_ref: vector, ..Param::new("Struct", "Value") }),
        ("SetObjectBehaviorVariable", Param { type_ref: object, ..Param::new("Object", "Value") }),
    ];
    let mut set_object = 0;
    for (name, value) in setters {
        let first = if broken && name == "SetIntBehaviorVariable" { Param::new("Int", "Variable") } else { variable.clone() };
        let ids = t.function(bpd, name, &[first, value], None, &[], &FunctionSpec::new(NATIVE_STATIC, name), |_| None);
        set_object = ids.function;
    }
    // The caller: link 9's shape. A local array of the struct, grown to 2, both new (None) elements filled by the
    // object setter, then copied to an instance property the test reads.
    let caller = t.class("ToyCaller", object);
    let result = toy_array_of(&mut t, caller, "Result", "Struct", bvv);
    let function = t.p.next_export() + 2;
    let local = toy_array_of(&mut t, function, "Parameters", "Struct", bvv);
    let mut a = Asm::new();
    a.stmt().raw(&[0x0F, 0x36]).local(local).int_const(2);
    for index in [0x25u8, 0x26] {
        a.stmt().class_context(bpd, |a| {
            a.final_function(set_object, |a| {
                a.raw(&[0x10, index]).local(local);
                if index == 0x25 {
                    a.self_();
                } else {
                    a.raw(&[0x2A]);
                }
            });
        });
    }
    a.stmt().raw(&[0x0F]).instance(result).local(local);
    a.stmt().return_nothing();
    a.end();
    let spec = FunctionSpec::new(SCRIPT, "FillParameters");
    let payload = function_payload(&mut t.p, Some(&a), &spec);
    let mut e = ow_package::testkit::Export::new(t.import("Function"), "FillParameters", payload);
    e.outer = caller;
    assert_eq!(t.p.add_export_with(e), function, "the caller function's reserved index");
    t.build()
}

fn toy_with(mode: ParityMode, broken: bool) -> Toy {
    let dir = std::env::temp_dir().join(format!(
        "ow-behavior-variables-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let bytes = package(broken);
    let defs =
        DefStore::from_synthetic(&dir, &[("GearboxFramework.upk", &bytes)], IdentityPolicy::FirstInScanOrder, mode)
            .unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode);
    let caller = vm.find_class("GearboxFramework.ToyCaller").unwrap();
    let set_object = setter(&mut vm, 4);
    let struct_of = |vm: &Vm, f: FuncId, i: usize| match vm.function(f).unwrap().params[i].kind {
        PropKind::Struct(s) => s,
        ref other => panic!("{other:?}"),
    };
    let bvv = struct_of(&vm, set_object, 0);
    let set_vector = setter(&mut vm, 3);
    let vector = struct_of(&vm, set_vector, 1);
    Pack.register(&mut vm).unwrap();
    let class = vm.find_class("GearboxFramework.Object").unwrap();
    let object = vm.space.alloc(class, "ToyObject", None, Origin::Runtime).unwrap();
    Toy { vm, object, dir, bvv, vector, caller }
}

fn toy(mode: ParityMode) -> Toy {
    toy_with(mode, false)
}

fn setter(vm: &mut Vm, i: usize) -> FuncId {
    vm.find_function(SETTERS[i]).unwrap()
}

fn entry(vm: &Vm, ty: StructId, field: &str) -> usize {
    let layout = vm.space.classes.struct_layout(ty).unwrap();
    layout.field(vm.space.names.find(field).unwrap()).unwrap().0 as usize
}

fn read(vm: &Vm, v: &Value, field: &str) -> Value {
    let s = v.as_struct().unwrap();
    s.fields[entry(vm, s.ty, field)].clone()
}

fn with(vm: &Vm, v: &Value, field: &str, x: Value) -> Value {
    let s = v.as_struct().unwrap();
    let mut out = StructValue::clone(s);
    out.fields[entry(vm, s.ty, field)] = x;
    Value::Struct(Rc::new(out))
}

/// The enum value of `name` in the loaded toy enum (by name, as the rule reads it).
fn bvar(name: &str) -> Value {
    Value::Byte(BVARS.iter().position(|b| *b == name).unwrap() as u8)
}

/// A struct with every field set to something other than zero, so an untouched field is seen as untouched.
fn filled(t: &Toy) -> Value {
    let vm = &t.vm;
    let zero = vm.space.classes.zero_value(&PropKind::Struct(t.bvv));
    let v = with(vm, &zero, "ToyTag", Value::Int(77));
    let v = with(vm, &v, "IntValue", Value::Int(-5));
    let v = with(vm, &v, "FloatValue", Value::Float(2.5));
    let v = with(vm, &v, "VectorValue", vec3(t, 9.0, 8.0, 7.0));
    let v = with(vm, &v, "VariableType", bvar("BVAR_None"));
    let some = Some(an_object(t));
    with(vm, &v, "ObjectValue", Value::Object(some))
}

fn vec3(t: &Toy, x: f32, y: f32, z: f32) -> Value {
    let vm = &t.vm;
    let zero = vm.space.classes.zero_value(&PropKind::Struct(t.vector));
    let v = with(vm, &zero, "X", Value::Float(x));
    let v = with(vm, &v, "Y", Value::Float(y));
    with(vm, &v, "Z", Value::Float(z))
}

fn an_object(t: &Toy) -> ObjRef {
    t.object
}

fn counters(vm: &Vm) -> (u64, u64, u64) {
    (vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions())
}

/// Calls setter `i` from the host and returns the variable as it stands after the call.
fn call(t: &mut Toy, i: usize, variable: Value, value: Value) -> Result<Value, VmError> {
    let f = setter(&mut t.vm, i);
    let r = t.vm.call(f, None, vec![variable, value])?;
    assert_eq!(r.ret, Value::None);
    Ok(r.outs[0].clone())
}

/// Every field but the two named is as in `before`.
fn others_untouched(t: &Toy, before: &Value, after: &Value, written: &[&str]) {
    let vm = &t.vm;
    for f in ["ToyTag", "IntValue", "FloatValue", "VectorValue", "ObjectValue", "VariableType"] {
        if !written.contains(&f) {
            assert_eq!(read(vm, after, f), read(vm, before, f), "{f} changed");
        }
    }
}

#[test]
fn each_setter_writes_its_value_field_and_type_and_nothing_else() {
    let mut t = toy(ParityMode::Off);
    let o = an_object(&t);
    let v = vec3(&t, 1.0, -2.0, 3.5);
    let cases: [(usize, Value, &str, Value, &str); 6] = [
        (0, Value::Bool(true), "IntValue", Value::Int(1), "BVAR_Bool"),
        (0, Value::Bool(false), "IntValue", Value::Int(0), "BVAR_Bool"),
        (1, Value::Int(42), "IntValue", Value::Int(42), "BVAR_Int"),
        (2, Value::Float(0.25), "FloatValue", Value::Float(0.25), "BVAR_Float"),
        (3, v.clone(), "VectorValue", v, "BVAR_Vector"),
        (4, Value::Object(Some(o)), "ObjectValue", Value::Object(Some(o)), "BVAR_Object"),
    ];
    for (i, value, field, stored, kind) in cases {
        let before = filled(&t);
        let after = call(&mut t, i, before.clone(), value).unwrap();
        assert_eq!(read(&t.vm, &after, field), stored, "{}", SETTERS[i]);
        assert_eq!(read(&t.vm, &after, "VariableType"), bvar(kind), "{}", SETTERS[i]);
        others_untouched(&t, &before, &after, &[field, "VariableType"]);
    }
}

#[test]
fn a_none_object_is_written_without_a_check() {
    let mut t = toy(ParityMode::Off);
    let before = filled(&t);
    for value in [Value::Object(None), Value::None] {
        let after = call(&mut t, 4, before.clone(), value).unwrap();
        assert_eq!(read(&t.vm, &after, "ObjectValue"), Value::Object(None));
        assert_eq!(read(&t.vm, &after, "VariableType"), bvar("BVAR_Object"));
        others_untouched(&t, &before, &after, &["ObjectValue", "VariableType"]);
    }
}

#[test]
fn a_none_variable_is_the_zero_struct() {
    let mut t = toy(ParityMode::Off);
    let after = call(&mut t, 1, Value::None, Value::Int(3)).unwrap();
    let zero = t.vm.space.classes.zero_value(&PropKind::Struct(t.bvv));
    assert_eq!(read(&t.vm, &after, "IntValue"), Value::Int(3));
    assert_eq!(read(&t.vm, &after, "VariableType"), bvar("BVAR_Int"));
    others_untouched(&t, &zero, &after, &["IntValue", "VariableType"]);
}

/// Link 9's shape: the caller grows a local array (its new elements are None) and fills both through the object
/// setter, each through an out argument aliasing an array element.
#[test]
fn the_script_caller_fills_new_local_array_elements() {
    let mut t = toy(ParityMode::Off);
    let caller = t.caller;
    let this = t.vm.space.alloc(caller, "Caller", None, Origin::Runtime).unwrap();
    let f = t.vm.find_method(caller, "FillParameters", None).unwrap().unwrap();
    t.vm.call(f, Some(this), vec![]).unwrap();
    let prop = t.vm.space.prop(caller, "Result").unwrap();
    let result = t.vm.space.read(this, PropAddr::scalar(prop)).unwrap();
    let items = result.elements().unwrap().to_vec();
    assert_eq!(items.len(), 2);
    let zero = t.vm.space.classes.zero_value(&PropKind::Struct(t.bvv));
    for (item, object) in items.iter().zip([Some(this), None]) {
        assert_eq!(read(&t.vm, item, "ObjectValue"), Value::Object(object));
        assert_eq!(read(&t.vm, item, "VariableType"), bvar("BVAR_Object"));
        others_untouched(&t, &zero, item, &["ObjectValue", "VariableType"]);
    }
}

#[test]
fn a_variable_that_is_not_the_struct_is_a_script_error_with_nothing_moved() {
    let mut t = toy(ParityMode::Off);
    let not_it = vec3(&t, 1.0, 1.0, 1.0);
    for (variable, holds) in [(Value::Int(7), "Int"), (not_it, "Struct")] {
        let before = counters(&t.vm);
        let err = call(&mut t, 4, variable, Value::Object(None)).unwrap_err();
        match err {
            VmError::Script { function, message } => {
                assert_eq!(function, "BehaviorProviderDefinition.SetObjectBehaviorVariable");
                assert_eq!(
                    message,
                    format!("BehaviorProviderDefinition.SetObjectBehaviorVariable: {holds}, not a BehaviorVariableValue")
                );
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(counters(&t.vm), before);
    }
}

#[test]
fn a_value_of_the_wrong_kind_is_a_script_error_with_nothing_moved() {
    let mut t = toy(ParityMode::Off);
    let o = an_object(&t);
    let cases = [
        (0, Value::Int(1)),
        (1, Value::Float(1.0)),
        (2, Value::Int(1)),
        (3, filled(&t)),
        (4, Value::Int(0)),
        (1, Value::Object(Some(o))),
    ];
    for (i, value) in cases {
        let before = counters(&t.vm);
        let variable = filled(&t);
        let err = call(&mut t, i, variable, value).unwrap_err();
        match err {
            VmError::Script { message, .. } => assert!(message.starts_with("argument 1 (the value) holds"), "{message}"),
            other => panic!("{other:?}"),
        }
        assert_eq!(counters(&t.vm), before);
    }
}

#[test]
fn a_malformed_declaration_is_a_script_error() {
    let mut t = toy_with(ParityMode::Off, true);
    let before = counters(&t.vm);
    let err = call(&mut t, 1, Value::Int(0), Value::Int(1)).unwrap_err();
    match err {
        VmError::Script { function, message } => {
            assert_eq!(function, "BehaviorProviderDefinition.SetIntBehaviorVariable");
            assert_eq!(message, "argument 0 is not an out parameter");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(counters(&t.vm), before);
}

#[test]
fn registering_again_binds_nothing_new() {
    let mut t = toy(ParityMode::Off);
    Pack.register(&mut t.vm).unwrap();
    let after = call(&mut t, 1, Value::None, Value::Int(5)).unwrap();
    assert_eq!(read(&t.vm, &after, "IntValue"), Value::Int(5));
}

#[test]
fn reference_binds_none_of_the_setters() {
    let mut t = toy(ParityMode::Reference);
    for (i, path) in SETTERS.iter().enumerate() {
        let f = setter(&mut t.vm, i);
        let key = t.vm.function(f).unwrap().native_key.clone().unwrap();
        assert!(!t.vm.has_native(&key), "{path} is bound under Reference");
    }
    let mut off = toy(ParityMode::Off);
    for (i, path) in SETTERS.iter().enumerate() {
        let f = setter(&mut off.vm, i);
        let key = off.vm.function(f).unwrap().native_key.clone().unwrap();
        assert!(off.vm.has_native(&key), "{path} is not bound under Off");
    }
}
