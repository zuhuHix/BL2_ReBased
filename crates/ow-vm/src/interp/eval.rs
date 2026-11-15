//! Owner: lane W3-INTERP. One bytecode frame: the port of `Interp::invoke` (the frame part), `run`, `eval`, `lval`,
//! `bindArguments`, `callFunction`, `callVirtual` and `callNative` of `src/interp.cpp`.
//!
//! **Steps.** One step per statement, one per expression evaluated, one per place taken (`lval`) and one per call,
//! exactly where the C++ `tick()`s, so `--run`'s step count is the C++ one.
//!
//! **Places.** Where the C++ holds a `Value*` into a slot, this keeps a [`Place`] (a frame slot, a typed temporary,
//! an object's property element, or a temporary value) and reads or writes through it. An out argument's place is
//! taken before the call, as the C++ takes its alias; the value the callee leaves there is written back when the call
//! returns (the C++ writes through the alias during the call; the difference shows only if the callee also reaches
//! the caller's variable another way, which no test or package case relies on).

use std::collections::BTreeMap;
use std::rc::Rc;

use ow_core::{FixedHashMap, NotYet};
use ow_object::heap::HeapError;
use ow_object::ids::{ClassId, FuncId, ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::{Value, same_value};
use ow_script::decode::{Code, Expr};
use ow_script::ops::op;

use super::load::{
    class_at, class_default, code_of, decl_at, ensure_class_code, function_at,
    instantiate_from_defaults, loaded_for_read, native_by_index, script_error,
};
use super::{FuncCode, aggregate, attr, convert, delegate, iter};
use crate::api::{Arg, CallResult, Vm};
use crate::error::VmError;

/// The object an expression runs against (the C++ `ctx`); `None` is the null object.
pub(crate) type Ctx = Option<ObjRef>;

/// One running script function (the C++ `Frame`).
pub(crate) struct Frame {
    pub(crate) code: Rc<FuncCode>,
    /// The frame's own object (`frame.self`): the call's object, or the owner's class default object.
    pub(crate) this: Ctx,
    /// Every declaration's elements, at the slots the loader gave them.
    slots: Vec<Value>,
    /// Per `code.locals` entry: an optional parameter the call did not supply.
    omitted: Vec<bool>,
    /// Local-variable references to no declaration of the function (the C++ makes such a slot on first use).
    extra: FixedHashMap<i32, Value>,
    /// The hidden typed temporaries 4C-50, by (opcode, operand).
    typed: BTreeMap<(u8, i32), Value>,
    switch_value: Value,
    result: Value,
    returned: bool,
}

impl Frame {
    pub(crate) fn path(&self) -> &str {
        &self.code.path
    }
}

/// Where a value lives (the C++ `Value*` of `lval`).
#[derive(Debug, Clone)]
pub(crate) enum Place {
    /// A frame slot.
    Slot(usize),
    /// A local-variable reference no declaration of the function has.
    Extra(i32),
    /// A hidden typed temporary.
    Typed(u8, i32),
    /// Element `PropAddr` of an object's property.
    Prop(ObjRef, PropAddr),
    /// Not a storage location: a temporary holding this value (the C++ `scratch`). Writes are dropped.
    Temp(Value),
}

// ------------------------------------------------------------------------------------------------ the frame

/// [`crate::api::Vm`]'s `run_frame`: runs script function `f` with the bound arguments `args` on `this`.
pub(crate) fn run_frame(
    vm: &mut Vm,
    f: FuncId,
    this: Option<ObjRef>,
    args: Vec<Arg>,
) -> Result<CallResult, VmError> {
    let Some(code) = code_of(vm, f) else {
        return Err(NotYet::not_ported(
            format!(
                "running script function {}: it was declared by hand, with no package export and so no bytecode",
                vm.function_path(f)
            ),
            "lane W3-INTERP (script functions run from the code packages)",
        )
        .into());
    };
    if let Some(why) = &code.failure {
        return Err(cannot_run(&code.path, why));
    }
    let script = decoded(vm, &code)?;
    // A call with no object (static functions, or a top-level call) runs against the class default object.
    let this = match (this, code.owner) {
        (Some(t), _) => Some(t),
        (None, Some(owner)) => Some(class_default(vm, owner)?),
        (None, None) => None,
    };
    let mut frame = Frame {
        code: code.clone(),
        this,
        slots: Vec::with_capacity(code.slot_count),
        omitted: vec![false; code.locals.len()],
        extra: FixedHashMap::default(),
        typed: BTreeMap::new(),
        switch_value: Value::None,
        result: Value::None,
        returned: false,
    };
    for local in &code.locals {
        let zero = vm
            .space
            .classes
            .default_value(&local.decl.kind)
            .map_err(HeapError::from)?;
        for _ in 0..local.decl.array_dim.max(1) {
            frame.slots.push(zero.clone());
        }
    }
    for (position, &entry) in code.params.iter().enumerate() {
        match args.get(position) {
            Some(a) if a.supplied => frame.slots[code.locals[entry].slot] = a.value.clone(),
            _ => frame.omitted[entry] = true,
        }
    }
    vm.code.stack.push(f);
    let ran = run(vm, &mut frame, &script);
    vm.code.stack.pop();
    ran?;
    let outs = args
        .iter()
        .enumerate()
        .map(|(position, a)| match code.params.get(position) {
            Some(&entry) if a.out && a.supplied => frame.slots[code.locals[entry].slot].clone(),
            _ => a.value.clone(),
        })
        .collect();
    Ok(CallResult {
        ret: frame.result,
        outs,
    })
}

fn cannot_run(path: &str, why: &str) -> VmError {
    script_error(path, format!("cannot run {path}: {why}"))
}

/// The function's script, decoded on its first call (C++ `Function::code`; a failure is kept and reported on every
/// call, as `decodeFailed`).
fn decoded(vm: &Vm, code: &FuncCode) -> Result<Rc<Code>, VmError> {
    if let Some(done) = &*code.script.borrow() {
        return done.clone().map_err(|why| cannot_run(&code.path, &why));
    }
    let package = vm.defs.packages().shared(code.package);
    let outcome = ow_script::decode::decode(&package, &code.info)
        .map(Rc::new)
        .map_err(|e| e.to_string());
    *code.script.borrow_mut() = Some(outcome.clone());
    outcome.map_err(|why| cannot_run(&code.path, &why))
}

/// The statements (C++ `Interp::run`): the optional-parameter prologue, then each statement in turn.
fn run(vm: &mut Vm, f: &mut Frame, script: &Code) -> Result<(), VmError> {
    let statements = &script.statements;
    let mut pc = 0;
    // Prologue: one entry per optional parameter, either Nothing or DefaultParmValue ... EndParmValue.
    let optional: Vec<usize> = f
        .code
        .params
        .iter()
        .copied()
        .filter(|&e| f.code.locals[e].decl.is_optional())
        .collect();
    for entry in optional {
        let Some(s) = statements.get(pc) else {
            break;
        };
        if s.op == op::DEFAULT_PARM_VALUE {
            if f.omitted[entry] {
                let v = eval(vm, f, &s.kids[0], f.this)?;
                let slot = f.code.locals[entry].slot;
                f.slots[slot] = v;
            }
            pc += 1;
            if statements
                .get(pc)
                .is_some_and(|s| s.op == op::END_PARM_VALUE)
            {
                pc += 1;
            }
        } else if s.op == op::NOTHING {
            pc += 1;
        } else {
            break;
        }
    }
    while pc < statements.len() && !f.returned {
        let s = &statements[pc];
        vm.tick()?;
        match s.op {
            op::END_OF_SCRIPT | op::STOP => return Ok(()),
            op::RETURN => {
                let mut value = eval(vm, f, &s.kids[0], f.this)?;
                if matches!(s.kids[0].op, op::RETURN_NOTHING | op::NOTHING)
                    && let Some(r) = f.code.result
                {
                    value = f.slots[f.code.locals[r].slot].clone();
                }
                f.result = value;
                f.returned = true;
                return Ok(());
            }
            op::JUMP => pc = target(f, script, s.words[0])?,
            op::JUMP_IF_NOT => {
                if eval(vm, f, &s.kids[0], f.this)?.truth(&vm.space.names) {
                    pc += 1;
                } else {
                    pc = target(f, script, s.words[0])?;
                }
            }
            op::SWITCH => {
                f.switch_value = eval(vm, f, &s.kids[0], f.this)?;
                pc += 1;
            }
            op::CASE => {
                if s.words[0] == 0xFFFF {
                    pc += 1; // default: always matches
                } else {
                    let value = eval(vm, f, &s.kids[0], f.this)?;
                    if same_value(&f.switch_value, &value) {
                        pc += 1;
                    } else {
                        pc = target(f, script, s.words[0])?;
                    }
                }
            }
            op::DYN_ARRAY_ITERATOR | op::ITERATOR | op::ITERATOR_NEXT | op::ITERATOR_POP => {
                return Err(iter::statement(s.op));
            }
            op::LABEL_TABLE => pc += 1,
            op::GOTO_LABEL => {
                return Err(NotYet::not_ported(
                    "GotoLabel (a jump to a label of state code)",
                    "lane W5-STATES (states and latent code)",
                )
                .into());
            }
            _ => {
                eval(vm, f, s, f.this)?;
                pc += 1;
            }
        }
    }
    Ok(())
}

/// The statement a jump's in-memory offset names.
fn target(f: &Frame, script: &Code, offset: u16) -> Result<usize, VmError> {
    script
        .statement_at
        .get(&u32::from(offset))
        .copied()
        .ok_or_else(|| {
            script_error(
                f.path(),
                format!("jump to an unknown offset in {}", f.path()),
            )
        })
}

// ------------------------------------------------------------------------------------------------ places

/// `lval`: the place `e` names, running against `ctx`.
pub(crate) fn lval(vm: &mut Vm, f: &mut Frame, e: &Expr, ctx: Ctx) -> Result<Place, VmError> {
    vm.tick()?;
    match e.op {
        op::LOCAL_VARIABLE | op::LOCAL_OUT_VARIABLE => local_place(f, e.refs[0]),
        op::OP_4C..=op::OP_50 => typed_place(f, e),
        op::INSTANCE_VARIABLE | op::OP_5E => instance_place(vm, f, e.refs[0], ctx),
        op::DEFAULT_VARIABLE => default_place(vm, f, e.refs[0], ctx),
        op::STATE_VARIABLE => Err(state_variable()),
        op::BOOL_VARIABLE => lval(vm, f, &e.kids[0], ctx),
        op::INTERFACE_CONTEXT => Err(interfaces("InterfaceContext")),
        op::STRUCT_MEMBER | op::ARRAY_ELEMENT | op::DYN_ARRAY_ELEMENT => {
            Err(aggregate::element(e.op))
        }
        op::CONTEXT | op::CLASS_CONTEXT => match context_target(vm, f, e, ctx)? {
            None => {
                note_none_context(vm, f);
                Ok(Place::Temp(zero_for(vm, f, e.refs[0])))
            }
            Some(target) => lval(vm, f, &e.kids[1], Some(target)),
        },
        // Not a storage location: operate on a temporary.
        _ => Ok(Place::Temp(eval(vm, f, e, ctx)?)),
    }
}

fn local_place(f: &Frame, reference: i32) -> Result<Place, VmError> {
    match f.code.by_export.get(&reference) {
        Some(&entry) => {
            let local = &f.code.locals[entry];
            if local.decl.array_dim > 1 {
                return Err(aggregate::static_array());
            }
            Ok(Place::Slot(local.slot))
        }
        None => Ok(Place::Extra(reference)),
    }
}

/// Opcodes 4C-50 (NATIVE_BYTECODE_OPCODES.md, UNVERIFIED in the running game): a hidden typed temporary of the frame.
/// The operand indexes the function's local-variable array as the engine keeps it (count first, then the values),
/// so it is odd and selects pair (operand - 1) / 2. A slot nothing wrote yet reads as the typed zero.
fn typed_place(f: &mut Frame, e: &Expr) -> Result<Place, VmError> {
    let operand = e.ints[0];
    let size = f.code.info.hidden_locals.len();
    if operand < 1 || operand & 1 == 0 || operand as usize >= size {
        return Err(script_error(
            f.path(),
            format!(
                "typed temporary {operand} is outside the local-variable array of {}",
                f.path()
            ),
        ));
    }
    f.typed
        .entry((e.op, operand))
        .or_insert_with(|| match e.op {
            op::OP_4C => Value::Int(0),
            op::OP_4D => Value::Float(0.0),
            op::OP_4E => Value::Byte(0),
            op::OP_4F => Value::Bool(false),
            _ => Value::Object(None),
        });
    Ok(Place::Typed(e.op, operand))
}

/// An instance variable of `ctx`, found by the property's name in the object's class (as the C++ `property()`).
fn instance_place(vm: &mut Vm, f: &mut Frame, reference: i32, ctx: Ctx) -> Result<Place, VmError> {
    let Some(object) = ctx else {
        return Ok(Place::Temp(Value::None));
    };
    let decl = decl_at(vm, f.code.package, reference, f.path())?;
    if !loaded_for_read(vm, object, f.path())? {
        return Ok(Place::Temp(Value::None));
    }
    property_place(vm, object, decl.name)
}

fn property_place(vm: &Vm, object: ObjRef, name: ow_object::ids::NameId) -> Result<Place, VmError> {
    let class = vm.space.heap.get(object)?.class;
    match vm.space.classes.prop(class, name) {
        None => Ok(Place::Temp(Value::None)),
        Some(prop) => {
            if vm.space.classes.decl(prop).is_some_and(|d| d.array_dim > 1) {
                return Err(aggregate::static_array());
            }
            Ok(Place::Prop(object, PropAddr::scalar(prop)))
        }
    }
}

/// `default.Property`: the property of the class default object of `ctx`'s class.
fn default_place(vm: &mut Vm, f: &mut Frame, reference: i32, ctx: Ctx) -> Result<Place, VmError> {
    let Some(object) = ctx else {
        return Ok(Place::Temp(Value::None));
    };
    let decl = decl_at(vm, f.code.package, reference, f.path())?;
    let class = vm.space.heap.get(object)?.class;
    let defaults = class_default(vm, class)?;
    property_place(vm, defaults, decl.name)
}

/// The value at `p`.
pub(crate) fn read(vm: &Vm, f: &Frame, p: &Place) -> Result<Value, VmError> {
    Ok(match p {
        Place::Slot(i) => f.slots[*i].clone(),
        Place::Extra(r) => f.extra.get(r).cloned().unwrap_or(Value::None),
        Place::Typed(o, n) => f.typed.get(&(*o, *n)).cloned().unwrap_or(Value::None),
        Place::Prop(o, a) => vm.space.read(*o, *a)?.clone(),
        Place::Temp(v) => v.clone(),
    })
}

/// An assignment to `p` (the C++ `assign(*target, value)`): the value converted by the assignment rule against what
/// `p` holds, then stored; returns what `p` now holds.
pub(crate) fn assign_to(
    vm: &mut Vm,
    f: &mut Frame,
    p: &Place,
    value: Value,
) -> Result<Value, VmError> {
    let current = read(vm, f, p)?;
    let value = convert::assign(vm, &current, value);
    store(vm, f, p, value.clone())?;
    Ok(value)
}

/// Stores `value` at `p` as it is (an out argument's write-back, the C++ alias): only the heap's representation rules
/// of the module documentation apply.
pub(crate) fn store(vm: &mut Vm, f: &mut Frame, p: &Place, value: Value) -> Result<(), VmError> {
    match p {
        Place::Slot(i) => f.slots[*i] = value,
        Place::Extra(r) => {
            f.extra.insert(*r, value);
        }
        Place::Typed(o, n) => {
            f.typed.insert((*o, *n), value);
        }
        Place::Prop(o, a) => {
            let value = fit_for_heap(vm, *a, value);
            vm.space.write(*o, *a, value)?;
        }
        Place::Temp(_) => {}
    }
    Ok(())
}

/// The representation rules a heap slot needs that the C++ bag does not (module documentation of `interp`).
fn fit_for_heap(vm: &Vm, at: PropAddr, value: Value) -> Value {
    let Some(decl) = vm.space.classes.decl(at.prop) else {
        return value;
    };
    match (&decl.kind, value) {
        (PropKind::Class { .. }, Value::Object(None) | Value::None) => Value::Class(None),
        (PropKind::Interface { .. }, Value::Object(o)) => Value::Interface(o),
        (PropKind::Interface { .. }, Value::None) => Value::Interface(None),
        (PropKind::Object { .. }, Value::Interface(o)) => Value::Object(o),
        (PropKind::Object { .. }, Value::None) => Value::Object(None),
        (PropKind::Delegate, Value::Object(None) | Value::None) => Value::Delegate(None),
        (kind, Value::None) => vm.space.classes.zero_value(kind),
        (_, v) => v,
    }
}

// ------------------------------------------------------------------------------------------------ expressions

/// `eval`: the value of `e`, running against `ctx`.
pub(crate) fn eval(vm: &mut Vm, f: &mut Frame, e: &Expr, ctx: Ctx) -> Result<Value, VmError> {
    vm.tick()?;
    if e.is_native_call() {
        return call_native(vm, f, e, ctx);
    }
    Ok(match e.op {
        op::INT_CONST => Value::Int(e.ints[0]),
        op::FLOAT_CONST => Value::Float(e.floats[0]),
        op::STRING_CONST => Value::Str(convert::ansi_text(&e.text)),
        op::UNICODE_STRING_CONST => Value::Str(convert::unicode_text(&e.text)),
        op::NAME_CONST => Value::Name(vm.space.names.intern(&e.names[0])),
        op::INT_ZERO => Value::Int(0),
        op::INT_ONE => Value::Int(1),
        op::TRUE => Value::Bool(true),
        op::FALSE => Value::Bool(false),
        op::NO_OBJECT => Value::Object(None),
        op::VECTOR_CONST => native_const(
            vm,
            "Vector",
            ["X", "Y", "Z"],
            e.floats.iter().map(|&x| Value::Float(x)),
        )?,
        op::ROTATION_CONST => native_const(
            vm,
            "Rotator",
            ["Pitch", "Yaw", "Roll"],
            e.ints.iter().map(|&x| Value::Int(x)),
        )?,
        op::BYTE_CONST | op::INT_CONST_BYTE => Value::Byte(e.bytes[0]),
        op::SELF => Value::Object(ctx),
        op::NOTHING
        | op::EMPTY_PARM_VALUE
        | op::END_PARM_VALUE
        | op::DEBUG_INFO
        | op::LABEL_TABLE
        | op::FILTER_EDITOR_ONLY
        | op::EAT_RETURN_VALUE
        | op::NATIVE_PARM => Value::None,
        op::OBJECT_CONST => object_const(vm, f, e.refs[0])?,
        op::LOCAL_VARIABLE
        | op::LOCAL_OUT_VARIABLE
        | op::INSTANCE_VARIABLE
        | op::DEFAULT_VARIABLE
        | op::OP_4C..=op::OP_50
        | op::OP_5E
        | op::BOOL_VARIABLE => {
            let p = lval(vm, f, e, ctx)?;
            read(vm, f, &p)?
        }
        op::STATE_VARIABLE => return Err(state_variable()),
        op::STRUCT_MEMBER | op::ARRAY_ELEMENT | op::DYN_ARRAY_ELEMENT => {
            return Err(aggregate::element(e.op));
        }
        op::CONTEXT | op::CLASS_CONTEXT => match context_target(vm, f, e, ctx)? {
            None => {
                note_none_context(vm, f);
                zero_for(vm, f, e.refs[0])
            }
            Some(target) => eval(vm, f, &e.kids[1], Some(target))?,
        },
        op::INTERFACE_CONTEXT => return Err(interfaces("InterfaceContext")),
        op::SKIP => eval(vm, f, &e.kids[0], ctx)?,
        op::OP_5F => {
            attr::let_attribute(vm, f, e, ctx)?;
            Value::None // a statement: no result
        }
        op::LET | op::LET_BOOL => {
            let lhs = &e.kids[0];
            if lhs.op == op::DYN_ARRAY_LENGTH {
                return Err(aggregate::element(op::DYN_ARRAY_LENGTH));
            }
            let place = lval(vm, f, lhs, ctx)?;
            let value = eval(vm, f, &e.kids[1], ctx)?;
            // The assignment's value is what the target now holds (not a second evaluation of `lhs`).
            assign_to(vm, f, &place, value)?
        }
        op::CONDITIONAL => {
            if eval(vm, f, &e.kids[0], ctx)?.truth(&vm.space.names) {
                eval(vm, f, &e.kids[1], ctx)?
            } else {
                eval(vm, f, &e.kids[2], ctx)?
            }
        }
        op::VIRTUAL_FUNCTION => call_virtual(vm, f, e, ctx, false)?,
        op::GLOBAL_FUNCTION => call_virtual(vm, f, e, ctx, true)?,
        op::FINAL_FUNCTION => {
            let Some(function) = function_at(vm, f.code.package, e.refs[0])? else {
                return Err(script_error(
                    f.path(),
                    format!("call to a missing function (reference {})", e.refs[0]),
                ));
            };
            call_function(vm, f, function, ctx, e)?
        }
        op::PRIMITIVE_CAST => {
            let v = eval(vm, f, &e.kids[0], ctx)?;
            convert::cast(vm, e.bytes[0], v)?
        }
        op::DYNAMIC_CAST => dynamic_cast(vm, f, e, ctx)?,
        op::INTERFACE_CAST => return Err(interfaces("InterfaceCast")),
        op::META_CAST => {
            let v = eval(vm, f, &e.kids[0], ctx)?;
            let want = class_at(vm, f.code.package, e.refs[0], f.path())?;
            match (v, want) {
                (Value::Class(Some(c)), Some(want)) if is_child(vm, c, want)? => {
                    Value::Class(Some(c))
                }
                _ => Value::Class(None),
            }
        }
        op::STRUCT_CMP_EQ
        | op::STRUCT_CMP_NE
        | op::DYN_ARRAY_LENGTH
        | op::DYN_ARRAY_ADD
        | op::DYN_ARRAY_ADD_ITEM
        | op::DYN_ARRAY_INSERT
        | op::DYN_ARRAY_INSERT_ITEM
        | op::DYN_ARRAY_REMOVE
        | op::DYN_ARRAY_REMOVE_ITEM
        | op::DYN_ARRAY_FIND
        | op::DYN_ARRAY_FIND_STRUCT
        | op::DYN_ARRAY_SORT => return Err(aggregate::element(e.op)),
        op::DELEGATE_FUNCTION
        | op::LET_DELEGATE
        | op::EQUAL_EQUAL_DEL_DEL
        | op::EQUAL_EQUAL_DEL_FUNC
        | op::NOT_EQUAL_DEL_DEL
        | op::NOT_EQUAL_DEL_FUNC
        | op::EMPTY_DELEGATE
        | op::DELEGATE_PROPERTY
        | op::INSTANCE_DELEGATE => return Err(delegate::expression(e.op)),
        op::NEW => {
            let class = eval(vm, f, &e.kids[3], ctx)?;
            let Value::Class(Some(class)) = class else {
                return Ok(Value::Object(None));
            };
            let object = instantiate_from_defaults(vm, class)?;
            if let Value::Object(outer) = eval(vm, f, &e.kids[0], ctx)? {
                vm.space.heap.get_mut(object)?.outer = outer;
            }
            Value::Object(Some(object))
        }
        op::ASSERT => {
            if !eval(vm, f, &e.kids[0], ctx)?.truth(&vm.space.names) {
                vm.log
                    .push(format!("Assertion failed at line {}", e.words[0]));
            }
            Value::None
        }
        op::RETURN_NOTHING => zero_for(vm, f, e.refs[0]),
        other => {
            return Err(script_error(
                f.path(),
                format!("expression 0x{other:02x} is not evaluable"),
            ));
        }
    })
}

/// A `VectorConst` / `RotationConst`: the native struct's zero value with its three fields set.
fn native_const(
    vm: &Vm,
    name: &str,
    fields: [&str; 3],
    values: impl Iterator<Item = Value>,
) -> Result<Value, VmError> {
    let path = format!("Core.Object.{name}");
    let classes = &vm.space.classes;
    let layout = classes
        .struct_by_path(&path)
        .and_then(|s| classes.struct_layout(s))
        .ok_or_else(|| {
            VmError::from(NotYet::layer_absent(
                format!("the {path} struct"),
                "ow-object reflection of the Core package",
                "lane W3-INTERP (a struct is defined when a class or function that uses it loads)",
            ))
        })?;
    let mut value = layout.zero().clone();
    if let Some(s) = value.struct_mut() {
        for (field, v) in fields.iter().zip(values) {
            if let Some((first, _)) = vm.space.names.find(field).and_then(|n| layout.field(n))
                && let Some(slot) = s.fields.get_mut(first as usize)
            {
                *slot = v;
            }
        }
    }
    Ok(value)
}

/// `ObjectConst`: a class export is a class (its class reference is 0, or names `Class`); any other object is the
/// definition store's object for that reference (identity by path, `DefStore::resolve_ref`).
fn object_const(vm: &mut Vm, f: &Frame, reference: i32) -> Result<Value, VmError> {
    let package = f.code.package;
    let is_class = {
        let (code, store) = vm.defs.code_mut();
        code.resolve_ref(store, package, reference)
            .is_some_and(|(p, i)| code.class_name(store, p, i) == "Class")
    };
    if is_class {
        return Ok(Value::Class(class_at(vm, package, reference, f.path())?));
    }
    let found = vm.defs.resolve_ref(&mut vm.space, package, reference);
    vm.log.extend(vm.defs.take_log());
    found
        .map(Value::Object)
        .map_err(|e| super::load::def_error(f.path(), e))
}

/// The object a `Context` expression's member runs against (C++ `contextTarget`): a class gives its class default
/// object (`Class.default`, static calls), an object itself; anything else, and a null reference, is `None`.
pub(crate) fn context_target(
    vm: &mut Vm,
    f: &mut Frame,
    e: &Expr,
    ctx: Ctx,
) -> Result<Ctx, VmError> {
    match eval(vm, f, &e.kids[0], ctx)? {
        Value::Class(Some(c)) => class_default(vm, c).map(Some),
        Value::Object(o) | Value::Interface(o) => Ok(o),
        _ => Ok(None),
    }
}

/// Census only: a `Context` expression whose object was None yields a zero value; count it against the function.
pub(crate) fn note_none_context(vm: &mut Vm, f: &Frame) {
    if let Some(census) = &mut vm.census {
        *census.none_contexts.entry(f.code.path.clone()).or_insert(0) += 1;
    }
}

/// The zero value of the property `reference` names (C++ `zeroFor`): `None` for no reference or one that does not
/// type, as the C++ catches it.
pub(crate) fn zero_for(vm: &mut Vm, f: &Frame, reference: i32) -> Value {
    if reference == 0 {
        return Value::None;
    }
    let Ok(decl) = decl_at(vm, f.code.package, reference, f.path()) else {
        return Value::None;
    };
    let Ok(zero) = vm.space.classes.default_value(&decl.kind) else {
        return Value::None;
    };
    if decl.array_dim > 1 {
        Value::array(vec![zero; decl.array_dim as usize])
    } else {
        zero
    }
}

/// `DynamicCast`: the object when its class is the wanted class or a child of it; an interface wanted is
/// W4-INTERP2's; otherwise None.
fn dynamic_cast(vm: &mut Vm, f: &mut Frame, e: &Expr, ctx: Ctx) -> Result<Value, VmError> {
    let v = eval(vm, f, &e.kids[0], ctx)?;
    let want = class_at(vm, f.code.package, e.refs[0], f.path())?;
    let (Value::Object(Some(object)) | Value::Interface(Some(object))) = v else {
        return Ok(Value::Object(None));
    };
    let Some(want) = want else {
        return Ok(Value::Object(None));
    };
    let class = vm.space.heap.get(object)?.class;
    if is_child(vm, class, want)? {
        return Ok(Value::Object(Some(object)));
    }
    if may_be_interface(vm, want) {
        return Err(interfaces(
            "DynamicCast to an interface class (Runtime::implements)",
        ));
    }
    Ok(Value::Object(None))
}

/// `class` is `want` or a child of it. Through an engine-native stand-in class (decision 2026-11-15-08) the real
/// chain is unknown, so a "no" there is `NotYet`, not `false`.
pub(crate) fn is_child(vm: &Vm, class: ClassId, want: ClassId) -> Result<bool, VmError> {
    let classes = &vm.space.classes;
    if classes.is_child_of(class, want) {
        return Ok(true);
    }
    let mut cursor = Some(class);
    let mut left = classes.len();
    while let Some(c) = cursor.filter(|_| left > 0) {
        left -= 1;
        if classes.is_native(c) {
            let path = classes
                .get_layout(c)
                .map_or_else(String::new, |l| l.path().as_str().to_string());
            return Err(NotYet::not_read(
                format!(
                    "whether a class derives from another through the engine-native class {path}"
                ),
                "decision 2026-11-15-08 (engine-native stand-in classes)",
            )
            .into());
        }
        cursor = classes.get_layout(c).and_then(|l| l.super_class());
    }
    Ok(false)
}

/// Whether `class` could be an interface the C++ `implements` might answer yes for: flagged as one in its body, or
/// named `I<Upper>...` (the structural fallback for bodies that do not decode).
fn may_be_interface(vm: &Vm, class: ClassId) -> bool {
    let Some(key) = vm.defs.code().key_of(class) else {
        return false;
    };
    let info = vm.defs.code().class(key);
    let named = info.name.len() >= 2
        && info.name.as_bytes()[0] == b'I'
        && info.name.as_bytes()[1].is_ascii_uppercase();
    info.is_interface() || named
}

fn interfaces(what: &str) -> VmError {
    NotYet::not_ported(format!("{what} (interfaces)"), "lane W4-INTERP2").into()
}

fn state_variable() -> VmError {
    NotYet::not_ported(
        "StateVariable (a variable of state code)",
        "lane W5-STATES (states and latent code)",
    )
    .into()
}

// ------------------------------------------------------------------------------------------------ calls

/// `callVirtual`: the function `name` as the object `ctx` sees it in its current state (or in no state, for
/// `Global.`). A call on None is logged and yields None; a name the object's class chain does not declare is a stub
/// site (`<Class>.<Name>`) whose arguments are still evaluated.
fn call_virtual(
    vm: &mut Vm,
    f: &mut Frame,
    e: &Expr,
    ctx: Ctx,
    global: bool,
) -> Result<Value, VmError> {
    let name = &e.names[0];
    let Some(object) = ctx else {
        vm.log.push(format!("call of {name} on a None object"));
        return Ok(Value::None);
    };
    let (class, state) = {
        let o = vm.space.heap.get(object)?;
        (o.class, if global { None } else { o.state })
    };
    ensure_class_code(vm, class)?;
    match vm.find_method(class, name, state)? {
        Some(function) => call_function(vm, f, function, Some(object), e),
        None => {
            let class_name = vm
                .space
                .classes
                .get_layout(class)
                .map_or("?", |l| l.path().name())
                .to_string();
            vm.stub_site(
                &format!("{class_name}.{name}"),
                format!("UNIMPLEMENTED function {class_name}.{name}"),
            )?;
            for kid in &e.kids {
                eval(vm, f, kid, f.this)?;
            }
            Ok(Value::None)
        }
    }
}

/// `callNative`: a numbered native (an index no code package declares is a stub site, `native index <n>`, whose
/// arguments are still evaluated); `&&` and `||` skip their second operand when the first decides the result.
fn call_native(vm: &mut Vm, f: &mut Frame, e: &Expr, ctx: Ctx) -> Result<Value, VmError> {
    let Some(function) = native_by_index(vm, e.native)? else {
        vm.stub_site(
            &format!("native index {}", e.native),
            format!("UNIMPLEMENTED native index {}", e.native),
        )?;
        for kid in &e.kids {
            if kid.op != op::EMPTY_PARM_VALUE {
                eval(vm, f, kid, f.this)?;
            }
        }
        return Ok(Value::None);
    };
    let friendly = code_of(vm, function).map(|c| c.info.friendly_name.clone());
    if let Some(symbol @ ("&&" | "||")) = friendly.as_deref()
        && e.kids.len() == 2
    {
        let first = eval(vm, f, &e.kids[0], f.this)?.truth(&vm.space.names);
        if (symbol == "&&" && !first) || (symbol == "||" && first) {
            return Ok(Value::Bool(first));
        }
        return Ok(Value::Bool(
            eval(vm, f, &e.kids[1], f.this)?.truth(&vm.space.names),
        ));
    }
    call_function(vm, f, function, ctx, e)
}

/// `bindArguments` + `invoke`: the arguments are evaluated in the caller's own context (the frame's object, not the
/// call's target: UE3's `Stack.Object` versus `Context`, UNVERIFIED), an omitted optional one is not supplied, an
/// out argument is bound to its place and written back after the call, surplus arguments are evaluated and dropped.
fn call_function(
    vm: &mut Vm,
    f: &mut Frame,
    function: FuncId,
    target: Ctx,
    e: &Expr,
) -> Result<Value, VmError> {
    let shape: Vec<(bool, bool)> = vm
        .function(function)
        .map(|d| {
            d.params
                .iter()
                .map(|p| (p.is_out(), p.is_optional()))
                .collect()
        })
        .unwrap_or_default();
    let mut args = Vec::with_capacity(shape.len());
    let mut places: Vec<Option<Place>> = Vec::with_capacity(shape.len());
    for (i, &(out, optional)) in shape.iter().enumerate() {
        let Some(kid) = e.kids.get(i) else {
            args.push(Arg::new(Value::None, out, false));
            places.push(None);
            continue;
        };
        if kid.op == op::EMPTY_PARM_VALUE || (kid.op == op::NOTHING && optional) {
            args.push(Arg::new(Value::None, out, false));
            places.push(None);
        } else if out {
            let place = lval(vm, f, kid, f.this)?;
            args.push(Arg::new(read(vm, f, &place)?, true, true));
            places.push(Some(place));
        } else {
            args.push(Arg::new(eval(vm, f, kid, f.this)?, false, true));
            places.push(None);
        }
    }
    for kid in e.kids.iter().skip(shape.len()) {
        eval(vm, f, kid, f.this)?;
    }
    if let Some(code) = code_of(vm, function)
        && let Some(why) = &code.failure
        && vm.function(function).is_some_and(|d| d.is_native())
    {
        return Err(script_error(
            &code.path,
            format!("cannot call {}: {why}", code.path),
        ));
    }
    if let Some(object) = target {
        let class = vm.space.heap.get(object)?.class;
        ensure_class_code(vm, class)?;
    }
    let result = vm.call_bound(function, target, args)?;
    for (place, value) in places.iter().zip(result.outs) {
        if let Some(place) = place {
            store(vm, f, place, value)?;
        }
    }
    Ok(result.ret)
}
