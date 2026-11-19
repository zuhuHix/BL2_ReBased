//! Owner: lane IMPL-SET-OBJECT-BEHAVIOR-VARIABLE. The five behavior-variable setters of `BehaviorProviderDefinition`
//! (`SetBoolBehaviorVariable`, `SetIntBehaviorVariable`, `SetFloatBehaviorVariable`, `SetVectorBehaviorVariable`,
//! `SetObjectBehaviorVariable`), in `ParityMode::Off`. AI-assisted (Claude Code).
//!
//! Coded from NATIVE_SET_BEHAVIOR_VARIABLE.md alone (analyst lane ANALYST-SET-OBJECT-BEHAVIOR-VARIABLE, PR #538), with
//! the hub's rulings on it. No native code or analysis output was opened. **Every rule is UNVERIFIED in the running
//! game.** [`register`] binds them from [`super::natives::Pack`], in Off only, first-wins; under Reference nothing
//! changes (the stub policy answers them, as before).
//!
//! Each setter takes the caller's `BehaviorVariableValue` by reference (argument 0, an out argument) and a value
//! (argument 1). It writes two fields of that struct and leaves every other field as it was:
//!
//! | Setter | Value field | `VariableType` |
//! |---|---|---|
//! | Bool (no script caller) | `IntValue` = 1 for true, 0 for false | `BVAR_Bool` |
//! | Int (no script caller) | `IntValue` | `BVAR_Int` |
//! | Float (no script caller) | `FloatValue` | `BVAR_Float` |
//! | Vector (no script caller) | `VectorValue` | `BVAR_Vector` |
//! | Object | `ObjectValue` (None included: no check) | `BVAR_Object` |
//!
//! Only `SetObjectBehaviorVariable` has script callers in the installed packages; the other four rows rest on the
//! executable alone (the note's section 1). There is no lookup, no name and no None check, and nothing else is written.
//!
//! **The preflight** (write-free; it loads nothing):
//! - argument 0 is declared `out` as a `BehaviorProviderDefinition.BehaviorVariableValue`, and that struct declares the
//!   setter's value field (of its kind) and `VariableType` (a byte of `EBehaviorVariableType`);
//! - the `BVAR_*` value is found **by name** in that enum, never by position (the port's fixtures order it differently
//!   from the game on purpose);
//! - argument 0 holds None or that struct. **None is the zero struct**: link 9's caller grows a function-local array,
//!   and the VM fills a local array untyped, so its new elements are None (the note's section 5, item 1). Anything
//!   else is `VmError::Script`, "BehaviorProviderDefinition.Set<Kind>BehaviorVariable: <what it holds>, not a
//!   BehaviorVariableValue". That check is the port's own data error: the native checks nothing, since in the game the
//!   argument is always the typed struct;
//! - argument 1 is the setter's kind (a bool, an int, a float, the struct's own vector type, or an object reference,
//!   None included). Anything else is `VmError::Script` naming the argument.
//!
//! **The write** is one, the last step: the struct with the two fields replaced, through the out argument. No refusal
//! can follow it, so a setter never needs the "stopped after partial writes" prefix; a caller that refuses later is
//! marked by that caller.

use std::rc::Rc;

use ow_object::ids::StructId;
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

use super::natives::bind;

/// The record of this module.
pub const RECORD: &str = "docs/verification/NATIVE_SET_BEHAVIOR_VARIABLE.md";

/// The five setters' package-qualified paths, in the note's table order.
pub const SETTERS: [&str; 5] = [
    "GearboxFramework.BehaviorProviderDefinition.SetBoolBehaviorVariable",
    "GearboxFramework.BehaviorProviderDefinition.SetIntBehaviorVariable",
    "GearboxFramework.BehaviorProviderDefinition.SetFloatBehaviorVariable",
    "GearboxFramework.BehaviorProviderDefinition.SetVectorBehaviorVariable",
    "GearboxFramework.BehaviorProviderDefinition.SetObjectBehaviorVariable",
];

/// The struct argument 0 holds (its path ends with this) and the enum `VariableType` is typed by (its own name).
const STRUCT: &str = "BehaviorProviderDefinition.BehaviorVariableValue";
const ENUM: &str = "EBehaviorVariableType";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Bool,
    Int,
    Float,
    Vector,
    Object,
}

const KINDS: [Kind; 5] = [Kind::Bool, Kind::Int, Kind::Float, Kind::Vector, Kind::Object];

impl Kind {
    fn word(self) -> &'static str {
        match self {
            Kind::Bool => "Bool",
            Kind::Int => "Int",
            Kind::Float => "Float",
            Kind::Vector => "Vector",
            Kind::Object => "Object",
        }
    }

    /// The struct field the setter writes.
    fn field(self) -> &'static str {
        match self {
            Kind::Bool | Kind::Int => "IntValue",
            Kind::Float => "FloatValue",
            Kind::Vector => "VectorValue",
            Kind::Object => "ObjectValue",
        }
    }

    /// The `EBehaviorVariableType` name the setter writes.
    fn bvar(self) -> &'static str {
        match self {
            Kind::Bool => "BVAR_Bool",
            Kind::Int => "BVAR_Int",
            Kind::Float => "BVAR_Float",
            Kind::Vector => "BVAR_Vector",
            Kind::Object => "BVAR_Object",
        }
    }

    /// Whether the declared kind of the value field fits this setter.
    fn fits(self, kind: &PropKind) -> bool {
        match self {
            Kind::Bool | Kind::Int => matches!(kind, PropKind::Int),
            Kind::Float => matches!(kind, PropKind::Float),
            Kind::Vector => matches!(kind, PropKind::Struct(_)),
            Kind::Object => matches!(kind, PropKind::Object { .. }),
        }
    }
}

/// Binds the five setters (Off only, first-wins; module documentation). A path the package set does not declare is
/// skipped, as the kernel pack's other keys are.
pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    for (path, kind) in SETTERS.into_iter().zip(KINDS) {
        bind(vm, path, Rc::new(move |c: &mut NativeCall<'_>| set(c, kind)))?;
    }
    Ok(())
}

/// What the preflight found: the struct, the two field entries and the value to write.
struct Plan {
    ty: StructId,
    value_entry: usize,
    type_entry: usize,
    type_value: u8,
    value: Value,
}

fn set(c: &mut NativeCall<'_>, kind: Kind) -> Result<Value, VmError> {
    let plan = preflight(c, kind)?;
    let mut v = match c.arg(0)? {
        Value::None => c.vm.space.classes.zero_value(&PropKind::Struct(plan.ty)),
        other => other.clone(),
    };
    let s = v.struct_mut().ok_or_else(|| bad(kind, "the variable is not a struct after the preflight".into()))?;
    s.fields[plan.value_entry] = plan.value;
    s.fields[plan.type_entry] = Value::Byte(plan.type_value);
    c.set_out(0, v)?;
    Ok(Value::None)
}

fn name(kind: Kind) -> String {
    format!("BehaviorProviderDefinition.Set{}BehaviorVariable", kind.word())
}

fn bad(kind: Kind, message: String) -> VmError {
    VmError::Script {
        function: name(kind),
        message,
    }
}

/// The module documentation's preflight: reads only, writes nothing, loads nothing.
fn preflight(c: &NativeCall<'_>, kind: Kind) -> Result<Plan, VmError> {
    let vm: &Vm = c.vm;
    let decl = vm.function(c.function).ok_or_else(|| bad(kind, "the native's declaration is missing".into()))?;
    let ty = match decl.params.first() {
        Some(p) if p.is_out() => match p.kind {
            PropKind::Struct(s) => s,
            ref other => return Err(bad(kind, format!("argument 0 is declared as {other:?}, not a {STRUCT}"))),
        },
        _ => return Err(bad(kind, "argument 0 is not an out parameter".into())),
    };
    let classes = &vm.space.classes;
    let layout = classes.struct_layout(ty).ok_or_else(|| bad(kind, "argument 0's struct has no layout".into()))?;
    if !layout.path().as_str().ends_with(STRUCT) {
        return Err(bad(kind, format!("argument 0 is declared as {}, not a {STRUCT}", layout.path().as_str())));
    }
    let field = |n: &str| -> Result<(usize, PropKind), VmError> {
        vm.space
            .names
            .find(n)
            .and_then(|id| layout.field(id))
            .filter(|(_, d)| d.array_dim == 1)
            .map(|(i, d)| (i as usize, d.kind.clone()))
            .ok_or_else(|| bad(kind, format!("{STRUCT} declares no {n}")))
    };
    let (value_entry, value_kind) = field(kind.field())?;
    if !kind.fits(&value_kind) {
        return Err(bad(kind, format!("{STRUCT}.{} is declared as {value_kind:?}", kind.field())));
    }
    let (type_entry, type_kind) = field("VariableType")?;
    let e = match type_kind {
        PropKind::Byte { enum_: Some(e) } => e,
        other => return Err(bad(kind, format!("{STRUCT}.VariableType is declared as {other:?}, not a byte of {ENUM}"))),
    };
    let enum_layout = classes.enum_layout(e).ok_or_else(|| bad(kind, format!("{ENUM} has no layout")))?;
    if enum_layout.path().name() != ENUM {
        return Err(bad(kind, format!("VariableType is typed by {}, not {ENUM}", enum_layout.path().as_str())));
    }
    let type_value = vm
        .space
        .names
        .find(kind.bvar())
        .and_then(|id| enum_layout.index_of(id))
        .and_then(|i| u8::try_from(i).ok())
        .ok_or_else(|| bad(kind, format!("{ENUM} has no {}", kind.bvar())))?;
    // Argument 0: None (the zero struct) or that struct.
    match c.arg(0)? {
        Value::None => {}
        Value::Struct(s) if s.ty == ty => {}
        other => {
            return Err(bad(kind, format!("{}: {}, not a BehaviorVariableValue", name(kind), other.kind_name())));
        }
    }
    // Argument 1: the setter's kind.
    let value = match (kind, c.arg(1)?) {
        (Kind::Bool, Value::Bool(b)) => Value::Int(i32::from(*b)),
        (Kind::Int, Value::Int(i)) => Value::Int(*i),
        (Kind::Float, Value::Float(f)) => Value::Float(*f),
        (Kind::Vector, Value::Struct(s)) if value_kind == PropKind::Struct(s.ty) => Value::Struct(s.clone()),
        (Kind::Object, Value::Object(o)) => Value::Object(*o),
        (Kind::Object, Value::None) => Value::Object(None),
        (_, other) => {
            return Err(bad(
                kind,
                format!("argument 1 (the value) holds {}, not the {} the setter takes", other.kind_name(), kind.word()),
            ));
        }
    };
    Ok(Plan { ty, value_entry, type_entry, type_value, value })
}

#[cfg(test)]
#[path = "variables_tests.rs"]
mod tests;
