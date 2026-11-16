//! Owner: lane W6-COMBAT. Small readers and writers of object properties and struct fields that the combat natives
//! share, in the C++ `Runtime::property` / `Value::field` shapes. AI-assisted (Claude Code).
//!
//! A definition handle is loaded before it is read (`DefStore::ensure_loaded`), so a reference read from data reads
//! as the export it names, as the C++ `load()` makes a built object from the stand-in. A property the class does not
//! declare is `None` (C++: `property()` answers null); [`required`] turns that into the C++ error text.

use ow_core::fmt::fmt_g;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

/// A rule error with the C++ `RuntimeError` text of the unit that raises it (`combat: ...`, `combat hit: ...`).
pub fn error(text: impl Into<String>) -> VmError {
    VmError::Script {
        function: "combat".to_string(),
        message: text.into(),
    }
}

/// The text of an error as the C++ `what()` reads it: a script error's message alone, the rest as displayed.
pub fn error_text(e: &VmError) -> String {
    match e {
        VmError::Script { message, .. } => message.clone(),
        other => other.to_string(),
    }
}

/// Loads `o` if it is one of the store's unloaded definition handles (a reference read from data).
pub fn loaded(vm: &mut Vm, o: ObjRef) -> Result<ObjRef, VmError> {
    if vm.defs.is_unloaded(&vm.space, o) {
        let r = vm.defs.ensure_loaded(&mut vm.space, o);
        vm.log.extend(vm.defs.take_log());
        r.map_err(VmError::Def)?;
    }
    Ok(o)
}

/// Property `name` of `o` (element 0), loaded first; `None` when the class does not declare it.
pub fn get(vm: &mut Vm, o: ObjRef, name: &str) -> Result<Option<Value>, VmError> {
    loaded(vm, o)?;
    let class = vm.space.heap.get(o)?.class;
    let Some(p) = vm.space.prop(class, name) else {
        return Ok(None);
    };
    Ok(Some(vm.space.read(o, PropAddr::scalar(p))?.clone()))
}

/// Property `name` of `o`, which the class must declare (`<unit>: <Class> has no property <name>` otherwise).
pub fn required(vm: &mut Vm, o: ObjRef, name: &str, unit: &str) -> Result<Value, VmError> {
    match get(vm, o, name)? {
        Some(v) => Ok(v),
        None => Err(error(format!(
            "{unit}: {} has no property {name}",
            class_name(vm, o)
        ))),
    }
}

/// Writes property `name` of `o` (element 0). False when the class does not declare it.
pub fn set(vm: &mut Vm, o: ObjRef, name: &str, v: Value) -> Result<bool, VmError> {
    loaded(vm, o)?;
    let class = vm.space.heap.get(o)?.class;
    let Some(p) = vm.space.prop(class, name) else {
        return Ok(false);
    };
    vm.space.write(o, PropAddr::scalar(p), v)?;
    Ok(true)
}

/// Writes property `name` of `o`, which the class must declare.
pub fn set_required(
    vm: &mut Vm,
    o: ObjRef,
    name: &str,
    v: Value,
    unit: &str,
) -> Result<(), VmError> {
    if set(vm, o, name, v)? {
        Ok(())
    } else {
        Err(error(format!(
            "{unit}: {} has no property {name}",
            class_name(vm, o)
        )))
    }
}

/// The object a value holds (`isObject` in the C++: an object value that is not None).
pub fn object_of(v: &Value) -> Option<ObjRef> {
    match v {
        Value::Object(o) | Value::Interface(o) => *o,
        _ => None,
    }
}

/// The object property `name` of `o` holds, if any.
pub fn object(vm: &mut Vm, o: ObjRef, name: &str) -> Result<Option<ObjRef>, VmError> {
    Ok(get(vm, o, name)?.as_ref().and_then(object_of))
}

/// The C++ `Value::number()`: an int, byte or bool as its integer, a float as itself, anything else 0.
pub fn number(v: &Value) -> f64 {
    match v {
        Value::Int(i) => f64::from(*i),
        Value::Byte(b) => f64::from(*b),
        Value::Bool(b) => f64::from(u8::from(*b)),
        Value::Float(f) => f64::from(*f),
        _ => 0.0,
    }
}

/// The C++ `Value::integer()`: an int, byte or bool as itself; a float as the VM natives convert one (N5:
/// `ow_core::num::trunc_i64_wrap_i32`, toward zero and wrapped to 32 bits, as every caller keeps an int); anything
/// else 0.
pub fn integer(v: &Value) -> i64 {
    match v {
        Value::Int(i) => i64::from(*i),
        Value::Byte(b) => i64::from(*b),
        Value::Bool(b) => i64::from(*b),
        Value::Float(f) => i64::from(ow_core::num::trunc_i64_wrap_i32(f64::from(*f))),
        _ => 0,
    }
}

/// A double as the C++ `float(double)` the natives use: rounded to the nearest float, a value past the float range an
/// infinity and NaN a NaN, which is what Rust's `as` does for `f64` to `f32` (no saturation is involved; N5 covers
/// float-to-int casts only).
pub fn to_f32(x: f64) -> f32 {
    x as f32
}

/// The low 32 bits of an integer, as the C++ `Value::makeInt(int64_t)` keeps them (an int field or argument).
pub fn low_i32(x: i64) -> i32 {
    x as i32
}

/// The low 8 bits of an integer, as the C++ `Value::makeByte` keeps them (a byte or enum field or argument).
pub fn low_u8(x: i64) -> u8 {
    x as u8
}

/// The C++ `Value::truth()`.
pub fn truth(vm: &Vm, v: &Value) -> bool {
    v.truth(&vm.space.names)
}

/// Field `name` of the struct value `v` (C++ `Value::field`), `None` when `v` is no struct or has no such field.
pub fn field(vm: &Vm, v: &Value, name: &str) -> Option<Value> {
    let s = v.as_struct()?;
    let id = vm.space.names.find(name)?;
    let (index, _) = vm.space.classes.struct_layout(s.ty)?.field(id)?;
    s.fields.get(usize::try_from(index).ok()?).cloned()
}

/// Writes field `name` of the struct value `v`. False when `v` is no struct or has no such field.
pub fn set_field(vm: &Vm, v: &mut Value, name: &str, x: Value) -> bool {
    let Some(id) = vm.space.names.find(name) else {
        return false;
    };
    let Some(ty) = v.as_struct().map(|s| s.ty) else {
        return false;
    };
    let Some((index, _)) = vm.space.classes.struct_layout(ty).and_then(|l| l.field(id)) else {
        return false;
    };
    let Ok(index) = usize::try_from(index) else {
        return false;
    };
    match v.struct_mut().and_then(|s| s.fields.get_mut(index)) {
        Some(slot) => {
            *slot = x;
            true
        }
        None => false,
    }
}

/// The class name of `o` without its package (the C++ `cls->name`).
pub fn class_name(vm: &Vm, o: ObjRef) -> String {
    let path = ow_attr::eval::class_path(&vm.space, o);
    match path.rsplit_once('.') {
        Some((_, leaf)) => leaf.to_string(),
        None => path,
    }
}

/// The object's name (C++ `Object::name`).
pub fn name_of(vm: &Vm, o: ObjRef) -> String {
    ow_behavior::provider::object_name(vm, o).unwrap_or_default()
}

/// The stock path of `o` (C++ `pathOf`: the export's path in its package; `""` for an object made at run time).
pub fn path_of(vm: &Vm, o: Option<ObjRef>) -> String {
    o.and_then(|o| ow_behavior::provider::stock_path(vm, o).ok())
        .unwrap_or_default()
}

/// Whether `o`'s class is `class_path` or derives from it. A class no package of the set holds is `false` (the C++
/// catches the lookup's error the same way).
pub fn is_a(vm: &mut Vm, o: ObjRef, class_path: &str) -> Result<bool, VmError> {
    let class = vm.space.heap.get(o)?.class;
    match vm.find_class(class_path) {
        Ok(base) => Ok(vm.space.classes.is_child_of(class, base)),
        Err(_) => Ok(false),
    }
}

/// The C++ `%.5g` of `combat_hit.cpp`'s event texts.
pub fn g5(x: f64) -> String {
    fmt_g(x, 5)
}

/// A static or dynamic array value's elements (a single value is itself, as the C++ reads a one-element property).
pub fn elements(v: &Value) -> Vec<Value> {
    match v.elements() {
        Some(items) => items.to_vec(),
        None => vec![v.clone()],
    }
}

/// The elements of array property `name` of `o` (empty when absent or not an array).
pub fn array(vm: &mut Vm, o: ObjRef, name: &str) -> Result<Vec<Value>, VmError> {
    Ok(match get(vm, o, name)? {
        Some(v) => v.elements().map(<[Value]>::to_vec).unwrap_or_default(),
        None => Vec::new(),
    })
}
