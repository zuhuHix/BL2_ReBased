//! `SetColorParameter` in Off (NATIVE_PSC_SET_COLOR_PARAMETER.md section 4's tests 1 to 11) on an invented `Engine`
//! package (AGENTS.md rule 9): `Object` with its `Color` struct; `ParticleSystemComponent` with `InstanceParameters`
//! (`ParticleSysParam`: `Name`, `ParamType`, `Color` and an extra `Scalar`), `bNeedsReattach`,
//! `bNeedsUpdateTransform` and `Template`, and the native; malformed component classes; a toy weapon holding a
//! parameter component; and a lazy component reached through a holder. Every value is invented; every call goes
//! through the VM. AI-assisted (Claude Code).

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::notyet::Missing;
use ow_core::{ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::{StructValue, Value};
use ow_package::testkit::ints;
use ow_package::testkit::reflect::{toy_array_of, toy_struct};
use ow_package::testkit::script::{FunctionSpec, Param, ScriptToy};
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::*;

static NEXT: AtomicU32 = AtomicU32::new(0);
const IMPORTS: [&str; 10] = [
    "Class",
    "Function",
    "ScriptStruct",
    "ObjectProperty",
    "ArrayProperty",
    "StructProperty",
    "NameProperty",
    "FloatProperty",
    "ByteProperty",
    "BoolProperty",
];
const NATIVE: u32 = 0x400 | 0x20000;
const PATH: &str = "Engine.ParticleSystemComponent.SetColorParameter";

fn engine() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let object = t.class("Object", 0);
    let color = toy_struct(&mut t, object, "Color", None);
    for c in ["B", "G", "R", "A"] {
        t.prop("Byte", color, c, 0, 0);
    }
    let psc = t.class("ParticleSystemComponent", object);
    let param = toy_struct(&mut t, psc, "ParticleSysParam", None);
    t.prop("Name", param, "Name", 0, 0);
    t.prop("Byte", param, "ParamType", 0, 0);
    t.prop("Float", param, "Scalar", 0, 0);
    t.prop("Struct", param, "Color", 0, color);
    toy_array_of(&mut t, psc, "InstanceParameters", "Struct", param);
    for b in ["bNeedsReattach", "bNeedsUpdateTransform"] {
        t.prop("Bool", psc, b, 0, 0);
    }
    t.prop("Object", psc, "Template", 0, object);
    let args = [Param::new("Name", "ParameterName"), Param { type_ref: color, ..Param::new("Struct", "Param") }];
    t.function(psc, "SetColorParameter", &args, None, &[], &FunctionSpec::new(NATIVE, "SetColorParameter"), |_| None);
    // Malformed classes: no InstanceParameters; one of ints; one whose element has no Color; one whose Color is a byte.
    t.class("ToyNoParamsPSC", object);
    let ints_psc = t.class("ToyIntParamsPSC", object);
    toy_array_of(&mut t, ints_psc, "InstanceParameters", "Float", 0);
    let no_color = t.class("ToyNoColorPSC", object);
    let param2 = toy_struct(&mut t, no_color, "ToyParam", None);
    t.prop("Name", param2, "Name", 0, 0);
    t.prop("Byte", param2, "ParamType", 0, 0);
    toy_array_of(&mut t, no_color, "InstanceParameters", "Struct", param2);
    let byte_color = t.class("ToyByteColorPSC", object);
    let param3 = toy_struct(&mut t, byte_color, "ToyParam", None);
    t.prop("Name", param3, "Name", 0, 0);
    t.prop("Byte", param3, "ParamType", 0, 0);
    t.prop("Byte", param3, "Color", 0, 0);
    toy_array_of(&mut t, byte_color, "InstanceParameters", "Struct", param3);
    // A toy weapon with a parameter component, and a lazy component reached through a holder.
    let weapon = t.class("ToyWeapon", object);
    t.prop("Object", weapon, "TracerParameterTemplate", 0, psc);
    let holder = t.class("ToyHolder", object);
    t.prop("Object", holder, "Component", 0, psc);
    let mut empty = ints(&[0]);
    empty.extend_from_slice(&t.none);
    let lazy = t.p.add_export(psc, "ToyLazyPSC", empty);
    let mut h = ints(&[0]);
    h.extend(t.p.object_tag("Component", lazy));
    h.extend_from_slice(&t.none);
    t.p.add_export(holder, "ToyHolderObject", h);
    t.build()
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

fn fx_in(mode: ParityMode) -> Fx {
    let dir = std::env::temp_dir().join(format!("ow-inventory-psc-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let e = engine();
    let defs = DefStore::from_synthetic(&dir, &[("Engine.upk", e.as_slice())], IdentityPolicy::FirstInScanOrder, mode).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode);
    for c in [
        "Engine.ParticleSystemComponent",
        "Engine.ToyNoParamsPSC",
        "Engine.ToyIntParamsPSC",
        "Engine.ToyNoColorPSC",
        "Engine.ToyByteColorPSC",
        "Engine.ToyWeapon",
    ] {
        vm.find_class(c).unwrap();
    }
    crate::attach_mesh::register(&mut vm).unwrap();
    Fx { vm, dir }
}

fn fx() -> Fx {
    fx_in(ParityMode::Off)
}

impl Fx {
    fn obj(&mut self, class: &str) -> ObjRef {
        let c = self.vm.find_class(class).unwrap();
        ow_vm::interp::instantiate_from_defaults(&mut self.vm, c).unwrap()
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    fn get(&self, o: ObjRef, name: &str) -> Value {
        crate::equip::read_prop(&self.vm, o, name).unwrap()
    }

    fn name(&mut self, text: &str) -> Value {
        Value::Name(self.vm.space.names.intern(text))
    }

    /// A `Color` with the channels `[r, g, b, a]`.
    fn color(&self, rgba: [u8; 4]) -> Value {
        let ty = self.vm.space.classes.struct_by_path("Engine.Object.Color").unwrap();
        let mut v = self.vm.space.classes.struct_defaults(ty).unwrap();
        for (c, b) in ["R", "G", "B", "A"].into_iter().zip(rgba) {
            let id = self.vm.space.names.find(c).unwrap();
            let at = self.vm.space.classes.struct_layout(ty).unwrap().field(id).unwrap().0 as usize;
            v.struct_mut().unwrap().fields[at] = Value::Byte(b);
        }
        v
    }

    /// A `ParticleSysParam` with `name`, `ty`, a `Scalar` and a colour.
    fn param(&mut self, name: &str, ty: u8, scalar: f32, rgba: [u8; 4]) -> Value {
        let st = self.vm.space.classes.struct_by_path("Engine.ParticleSystemComponent.ParticleSysParam").unwrap();
        let base = self.vm.space.classes.struct_defaults(st).unwrap();
        let name = self.name(name);
        let color = self.color(rgba);
        let layout = self.vm.space.classes.struct_layout(st).unwrap();
        let Value::Struct(s) = base else { panic!() };
        let mut fields = s.fields.clone();
        for (f, v) in [("Name", name), ("ParamType", Value::Byte(ty)), ("Scalar", Value::Float(scalar)), ("Color", color)] {
            let id = self.vm.space.names.find(f).unwrap();
            fields[layout.field(id).unwrap().0 as usize] = v;
        }
        Value::Struct(Rc::new(StructValue { ty: st, fields }))
    }

    fn call(&mut self, c: ObjRef, name: &str, rgba: [u8; 4]) -> Result<Value, VmError> {
        let f = self.vm.find_function(PATH).unwrap();
        let name = self.name(name);
        let color = self.color(rgba);
        Ok(self.vm.call(f, Some(c), vec![name, color])?.ret)
    }

    fn params(&self, c: ObjRef) -> Vec<Value> {
        self.get(c, "InstanceParameters").elements().unwrap().to_vec()
    }

    fn field(&self, v: &Value, name: &str) -> Value {
        crate::equip::field(&self.vm, v, name).unwrap()
    }

    fn rgba(&self, v: &Value) -> [u8; 4] {
        let c = self.field(v, "Color");
        let b = |n: &str| match self.field(&c, n) {
            Value::Byte(b) => b,
            other => panic!("{other:?}"),
        };
        [b("R"), b("G"), b("B"), b("A")]
    }

    /// #518's counters, the class-code loads and the name table.
    fn marks(&self) -> (u64, u64, u64, u64, usize) {
        let vm = &self.vm;
        (vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions(), vm.class_code_loads(), vm.space.names.len())
    }
}

#[test]
fn the_native_binds_on_the_package_s_key() {
    let mut x = fx();
    let f = x.vm.find_function(PATH).unwrap();
    assert_eq!(x.vm.function(f).unwrap().native_key.as_deref(), Some(SET_COLOR_PARAMETER));
    assert!(x.vm.has_native(SET_COLOR_PARAMETER));
}

/// Tests 1 and 8: an empty list gets one entry, the name, type 5, the four channels as given (alpha included), every
/// other field zero.
#[test]
fn an_empty_list_gets_one_colour_entry() {
    let mut x = fx();
    let c = x.obj("Engine.ParticleSystemComponent");
    x.call(c, "TracerColor", [200, 100, 50, 7]).unwrap();
    let params = x.params(c);
    assert_eq!(params.len(), 1);
    let name = x.name("TracerColor");
    assert_eq!(x.field(&params[0], "Name"), name);
    assert_eq!(x.field(&params[0], "ParamType"), Value::Byte(PARAM_COLOR));
    assert_eq!(x.rgba(&params[0]), [200, 100, 50, 7]);
    assert_eq!(x.field(&params[0], "Scalar"), Value::Float(0.0));
}

/// Tests 2 and 4: an entry with the name and type 5 has its colour replaced in place, its other fields kept, the
/// length unchanged; of two such entries only the first changes.
#[test]
fn the_first_matching_colour_entry_is_replaced_in_place() {
    let mut x = fx();
    let c = x.obj("Engine.ParticleSystemComponent");
    let list = vec![x.param("Glow", PARAM_COLOR, 3.5, [1, 1, 1, 1]), x.param("Glow", PARAM_COLOR, 4.5, [2, 2, 2, 2])];
    x.set(c, "InstanceParameters", Value::array(list));
    x.call(c, "Glow", [9, 8, 7, 6]).unwrap();
    let params = x.params(c);
    assert_eq!(params.len(), 2);
    assert_eq!(x.rgba(&params[0]), [9, 8, 7, 6]);
    assert_eq!(x.field(&params[0], "Scalar"), Value::Float(3.5), "other fields kept");
    assert_eq!(x.rgba(&params[1]), [2, 2, 2, 2], "only the first changes");
}

/// Test 3: an entry with the name and type 3 (a vector) is not matched: a colour entry is appended after it.
#[test]
fn a_same_name_entry_of_another_type_is_not_matched() {
    let mut x = fx();
    let c = x.obj("Engine.ParticleSystemComponent");
    let list = vec![x.param("Glow", 3, 1.0, [5, 5, 5, 5])];
    x.set(c, "InstanceParameters", Value::array(list));
    x.call(c, "Glow", [9, 8, 7, 6]).unwrap();
    let params = x.params(c);
    assert_eq!(params.len(), 2);
    assert_eq!(x.rgba(&params[0]), [5, 5, 5, 5], "the vector entry untouched");
    assert_eq!(x.field(&params[1], "ParamType"), Value::Byte(PARAM_COLOR));
    assert_eq!(x.rgba(&params[1]), [9, 8, 7, 6]);
}

/// Test 5: a None name writes nothing.
#[test]
fn a_none_name_writes_nothing() {
    let mut x = fx();
    let c = x.obj("Engine.ParticleSystemComponent");
    let _ = x.name("None");
    let before = x.marks();
    x.call(c, "None", [1, 2, 3, 4]).unwrap();
    assert_eq!(x.marks(), before);
    assert!(x.params(c).is_empty());
}

/// Test 6: `InstanceParameters` missing, not an array of the struct, an element with no `Color`, or a `Color` of the
/// wrong kind: a script error naming it, nothing written.
#[test]
fn malformed_declarations_are_script_errors() {
    for class in ["Engine.ToyNoParamsPSC", "Engine.ToyIntParamsPSC", "Engine.ToyNoColorPSC", "Engine.ToyByteColorPSC"] {
        let mut x = fx();
        let c = x.obj(class);
        let _ = x.name("Glow");
        let before = x.marks();
        match x.call(c, "Glow", [1, 2, 3, 4]) {
            Err(VmError::Script { message, .. }) => assert!(message.contains("InstanceParameters"), "{class}: {message}"),
            other => panic!("{class}: {other:?}"),
        }
        assert_eq!(x.marks(), before, "{class}");
    }
}

/// Test 7: a lazy receiver is the not-loaded `NotRead`, nothing loaded; with a None name it is nothing, no refusal.
#[test]
fn a_lazy_receiver_is_not_loaded_unless_the_name_is_none() {
    let mut x = fx();
    let path = ObjectPath::parse("Engine.ToyHolderObject").unwrap();
    let holder = x.vm.defs.find(&mut x.vm.space, &path).unwrap().unwrap();
    let Value::Object(Some(lazy)) = x.get(holder, "Component") else { panic!() };
    assert!(x.vm.defs.is_unloaded(&x.vm.space, lazy));
    let _ = (x.name("Glow"), x.name("None"));
    let before = x.marks();
    match x.call(lazy, "Glow", [1, 2, 3, 4]) {
        Err(VmError::NotYet(n)) => {
            assert_eq!((n.missing, &*n.what, n.record), (Missing::NotRead, NOT_LOADED, RECORD));
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(x.marks(), before);
    x.call(lazy, "None", [1, 2, 3, 4]).unwrap();
    assert_eq!(x.marks(), before);
    assert!(x.vm.defs.is_unloaded(&x.vm.space, lazy), "nothing loaded");
}

/// Test 9: no other field of the component changes.
#[test]
fn no_other_field_changes() {
    let mut x = fx();
    let c = x.obj("Engine.ParticleSystemComponent");
    let template = x.obj("Engine.ToyWeapon");
    x.set(c, "Template", Value::Object(Some(template)));
    x.call(c, "Glow", [1, 2, 3, 4]).unwrap();
    assert_eq!(x.get(c, "bNeedsReattach"), Value::Bool(false));
    assert_eq!(x.get(c, "bNeedsUpdateTransform"), Value::Bool(false));
    assert_eq!(x.get(c, "Template"), Value::Object(Some(template)));
}

/// Test 10: a toy weapon's own parameter component is written; another component (standing for the class default's
/// subobject) is unchanged.
#[test]
fn the_weapon_s_own_component_is_written_and_the_default_s_is_not() {
    let mut x = fx();
    let defaults_component = x.obj("Engine.ParticleSystemComponent");
    let own = x.obj("Engine.ParticleSystemComponent");
    let weapon = x.obj("Engine.ToyWeapon");
    x.set(weapon, "TracerParameterTemplate", Value::Object(Some(own)));
    let Value::Object(Some(target)) = x.get(weapon, "TracerParameterTemplate") else { panic!() };
    x.call(target, "TracerColor", [200, 200, 200, 255]).unwrap();
    assert_eq!(x.params(own).len(), 1);
    assert!(x.params(defaults_component).is_empty(), "the other component is unchanged");
}

/// Test 11: Reference binds nothing.
#[test]
fn reference_binds_nothing() {
    let x = fx_in(ParityMode::Reference);
    assert!(!x.vm.has_native(SET_COLOR_PARAMETER));
}
