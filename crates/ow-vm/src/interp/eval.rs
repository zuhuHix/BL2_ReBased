//! Owner: lane W3-INTERP. One bytecode frame: the port of `Interp::invoke` (the frame part), `run`, `eval`, `lval`,
//! `bindArguments`, `callFunction`, `callVirtual` and `callNative` of `src/interp.cpp`.
//!
//! **Steps.** One step per statement, one per expression evaluated, one per place taken (`lval`) and one per call,
//! exactly where the C++ `tick()`s, so `--run`'s step count is the C++ one.
//!
//! **Frames and places.** A frame's storage (its slots, the local-variable references no declaration has, the typed
//! temporaries) lives on the VM's frame stack (`CodeState::frames`), not on the Rust stack, so a [`Place`] can name a
//! slot of any active frame. Where the C++ holds a `Value*` into a slot, this keeps a place (a frame slot, a typed
//! temporary, an object's property element, or a temporary value) and reads or writes through it.
//!
//! **Out arguments alias the caller's storage live**, as the C++ `Slot::alias`: an out argument's place is taken
//! before the call and travels with the argument ([`Alias`] on `Arg`). A script callee reads and writes the caller's
//! storage through it, and a native's `NativeCall::set_out` writes through it at once, so `F(x, x)` with two out
//! parameters sees one variable, and script a native calls back into sees the native's write. An out argument whose
//! expression is not a storage location (the C++ `scratch`) gets no alias: the callee keeps its own copy.
//!
//! **Bounds** (Rust-only refusals, decision 2026-11-15-14): a frame holds at most [`MAX_FRAME_SLOTS`] value slots and
//! all active frames together at most [`MAX_LIVE_SLOTS`], checked before anything is allocated, and the storage is
//! reserved fallibly; expressions nest at most [`MAX_NESTING`] levels across all frames. Each is a typed
//! `VmError::Script`, never an abort.

use std::rc::Rc;

use ow_core::NotYet;
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
use super::{
    FrameStore, FuncCode, MAX_FRAME_SLOTS, MAX_LIVE_SLOTS, MAX_NESTING, aggregate, attr, convert,
    delegate, iter,
};
use crate::api::{Arg, CallResult, Vm};
use crate::error::VmError;

/// The object an expression runs against (the C++ `ctx`); `None` is the null object.
pub(crate) type Ctx = Option<ObjRef>;

/// One running script function (the C++ `Frame`); its storage is `vm.code.frames[id]`.
pub(crate) struct Frame {
    pub(crate) code: Rc<FuncCode>,
    /// The frame's own object (`frame.self`): the call's object, or the owner's class default object.
    pub(crate) this: Ctx,
    /// The index of this frame's storage on the VM's frame stack.
    id: usize,
    /// Per `code.locals` entry: an optional parameter the call did not supply.
    omitted: Vec<bool>,
    /// Per `code.locals` entry: the caller's storage an out parameter aliases.
    aliases: Vec<Option<Place>>,
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
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Place {
    /// Slot `.1` of frame `.0` (an index on the VM's frame stack).
    Slot(usize, usize),
    /// A local-variable reference no declaration of the function has, in frame `.0`.
    Extra(usize, i32),
    /// A hidden typed temporary (opcode, operand) of frame `.0`.
    Typed(usize, u8, i32),
    /// Element `PropAddr` of an object's property.
    Prop(ObjRef, PropAddr),
    /// Not a storage location: a temporary holding this value (the C++ `scratch`). Writes are dropped.
    Temp(Value),
}

/// The caller's storage an out argument aliases (carried on `Arg`).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Alias(pub(crate) Place);

/// `NativeCall::set_out`'s write through an alias: the value stored as it is (the C++ write through `Arg::alias`).
pub(crate) fn write_alias(vm: &mut Vm, alias: &Alias, value: Value) -> Result<(), VmError> {
    store(vm, &alias.0, value)
}

// ------------------------------------------------------------------------------------------------ the frame

/// Before the depth check, as the C++ `invoke`: a function that cannot run says why, and its script is decoded.
/// Nothing for a function the loader did not declare (its `NotYet` comes from [`run_frame`]).
pub(crate) fn prepare(vm: &mut Vm, f: FuncId) -> Result<(), VmError> {
    let Some(code) = code_of(vm, f) else {
        return Ok(());
    };
    if let Some(why) = &code.failure {
        return Err(cannot_run(&code.path, why));
    }
    decoded(vm, &code).map(|_| ())
}

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
    let store = new_frame_store(vm, &code)?;
    let id = vm.code.frames.len();
    vm.code.live_slots += code.slot_count;
    vm.code.frames.push(store);
    let mut frame = Frame {
        code: code.clone(),
        this,
        id,
        omitted: vec![false; code.locals.len()],
        aliases: vec![None; code.locals.len()],
        switch_value: Value::None,
        result: Value::None,
        returned: false,
    };
    for (position, &entry) in code.params.iter().enumerate() {
        match args.get(position) {
            Some(a) if a.supplied => {
                vm.code.frames[id].slots[code.locals[entry].slot] = a.value.clone();
                if a.out {
                    frame.aliases[entry] = a.alias.as_ref().map(|al| al.0.clone());
                }
            }
            _ => frame.omitted[entry] = true,
        }
    }
    vm.code.stack.push(f);
    let ran = run(vm, &mut frame, &script);
    vm.code.stack.pop();
    let outs = ran.and_then(|()| {
        args.iter()
            .enumerate()
            .map(|(position, a)| match code.params.get(position) {
                Some(&entry) if a.out && a.supplied => read(vm, &local_place_of(&frame, entry)),
                _ => Ok(a.value.clone()),
            })
            .collect::<Result<Vec<_>, _>>()
    });
    vm.code.frames.pop();
    vm.code.live_slots -= code.slot_count;
    Ok(CallResult {
        ret: frame.result,
        outs: outs?,
    })
}

/// A frame's storage, its size checked against the bounds before anything is allocated and reserved fallibly; each
/// slot starts at its declaration's zero value (C++ `zeroValue`).
fn new_frame_store(vm: &Vm, code: &FuncCode) -> Result<FrameStore, VmError> {
    let path = &code.path;
    let needed = code.slot_count;
    let live = vm.code.live_slots.saturating_add(needed);
    if needed > MAX_FRAME_SLOTS || live > MAX_LIVE_SLOTS {
        return Err(script_error(
            path,
            format!(
                "the frame of {path} needs {needed} value slots ({live} with the active frames); the bound is \
                 {MAX_FRAME_SLOTS} per frame and {MAX_LIVE_SLOTS} for all active frames (a Rust-only refusal)"
            ),
        ));
    }
    let mut slots = Vec::new();
    slots.try_reserve_exact(needed).map_err(|_| {
        script_error(
            path,
            format!("cannot allocate the {needed} value slots of the frame of {path}"),
        )
    })?;
    for local in &code.locals {
        let zero = vm
            .space
            .classes
            .default_value(&local.decl.kind)
            .map_err(HeapError::from)?;
        for _ in 0..local.decl.array_dim.max(1) {
            slots.push(zero.clone());
        }
    }
    Ok(FrameStore {
        slots,
        ..FrameStore::default()
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
                let place = local_place_of(f, entry);
                store(vm, &place, v)?;
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
                    value = read(vm, &local_place_of(f, r))?;
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
            // The C++ steps over both: a label table is data, and a GotoLabel (a jump to a label of state code) is not
            // run, as latent state code is not (W5-STATES ports state code; UNVERIFIED in the game).
            op::LABEL_TABLE | op::GOTO_LABEL => pc += 1,
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

/// One more level of expression nesting (across all frames), refused past [`MAX_NESTING`].
fn enter(vm: &mut Vm, f: &Frame) -> Result<(), VmError> {
    if vm.code.nesting >= MAX_NESTING {
        return Err(script_error(
            f.path(),
            format!(
                "expressions nest deeper than {MAX_NESTING} levels across the active frames in {} (a Rust-only \
                 bound)",
                f.path()
            ),
        ));
    }
    vm.code.nesting += 1;
    Ok(())
}

/// `lval`: the place `e` names, running against `ctx`.
pub(crate) fn lval(vm: &mut Vm, f: &mut Frame, e: &Expr, ctx: Ctx) -> Result<Place, VmError> {
    enter(vm, f)?;
    let place = lval_inner(vm, f, e, ctx);
    vm.code.nesting -= 1;
    place
}

fn lval_inner(vm: &mut Vm, f: &mut Frame, e: &Expr, ctx: Ctx) -> Result<Place, VmError> {
    vm.tick()?;
    match e.op {
        op::LOCAL_VARIABLE | op::LOCAL_OUT_VARIABLE => local_place(f, e.refs[0]),
        op::OP_4C..=op::OP_50 => typed_place(vm, f, e),
        // A state variable reads as an instance variable, as the C++ does (UNVERIFIED in the game).
        op::INSTANCE_VARIABLE | op::OP_5E | op::STATE_VARIABLE => {
            instance_place(vm, f, e.refs[0], ctx)
        }
        op::DEFAULT_VARIABLE => default_place(vm, f, e.refs[0], ctx),
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

/// The place of `code.locals` entry `entry`: the caller's storage for an aliased out parameter, else the frame slot.
fn local_place_of(f: &Frame, entry: usize) -> Place {
    match &f.aliases[entry] {
        Some(alias) => alias.clone(),
        None => Place::Slot(f.id, f.code.locals[entry].slot),
    }
}

fn local_place(f: &Frame, reference: i32) -> Result<Place, VmError> {
    match f.code.by_export.get(&reference) {
        Some(&entry) => {
            if f.code.locals[entry].decl.array_dim > 1 {
                return Err(aggregate::static_array());
            }
            Ok(local_place_of(f, entry))
        }
        None => Ok(Place::Extra(f.id, reference)),
    }
}

/// Opcodes 4C-50 (NATIVE_BYTECODE_OPCODES.md, UNVERIFIED in the running game): a hidden typed temporary of the frame.
/// The operand indexes the function's local-variable array as the engine keeps it (count first, then the values),
/// so it is odd and selects pair (operand - 1) / 2. A slot nothing wrote yet reads as the typed zero.
fn typed_place(vm: &mut Vm, f: &Frame, e: &Expr) -> Result<Place, VmError> {
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
    vm.code.frames[f.id]
        .typed
        .entry((e.op, operand))
        .or_insert_with(|| match e.op {
            op::OP_4C => Value::Int(0),
            op::OP_4D => Value::Float(0.0),
            op::OP_4E => Value::Byte(0),
            op::OP_4F => Value::Bool(false),
            _ => Value::Object(None),
        });
    Ok(Place::Typed(f.id, e.op, operand))
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
pub(crate) fn read(vm: &Vm, p: &Place) -> Result<Value, VmError> {
    Ok(match p {
        Place::Slot(frame, i) => vm.code.frames[*frame].slots[*i].clone(),
        Place::Extra(frame, r) => vm.code.frames[*frame]
            .extra
            .get(r)
            .cloned()
            .unwrap_or(Value::None),
        Place::Typed(frame, o, n) => vm.code.frames[*frame]
            .typed
            .get(&(*o, *n))
            .cloned()
            .unwrap_or(Value::None),
        Place::Prop(o, a) => vm.space.read(*o, *a)?.clone(),
        Place::Temp(v) => v.clone(),
    })
}

/// An assignment to `p` (the C++ `assign(*target, value)`): the value converted by the assignment rule against what
/// `p` holds, then stored; returns what `p` now holds.
pub(crate) fn assign_to(vm: &mut Vm, p: &Place, value: Value) -> Result<Value, VmError> {
    let current = read(vm, p)?;
    let value = convert::assign(vm, &current, value);
    store(vm, p, value.clone())?;
    Ok(value)
}

/// Stores `value` at `p` as it is (the C++ write through a slot or an alias): only the heap's representation rules
/// of the module documentation of `interp` apply.
pub(crate) fn store(vm: &mut Vm, p: &Place, value: Value) -> Result<(), VmError> {
    match p {
        Place::Slot(frame, i) => vm.code.frames[*frame].slots[*i] = value,
        Place::Extra(frame, r) => {
            vm.code.frames[*frame].extra.insert(*r, value);
        }
        Place::Typed(frame, o, n) => {
            vm.code.frames[*frame].typed.insert((*o, *n), value);
        }
        Place::Prop(o, a) => store_property(vm, *o, *a, value)?,
        Place::Temp(_) => {}
    }
    Ok(())
}

/// The paths of `Core.Class` and the classes it derives from in UE3 (`Class : State : Struct : Field : Object`): an
/// object property declared as one of them may hold a class (UNVERIFIED for this executable: the public UE3 chain).
const CLASS_ANCESTORS: [&str; 5] = [
    "Core.Object",
    "Core.Field",
    "Core.Struct",
    "Core.State",
    "Core.Class",
];

/// A heap write with the representation rules the C++ bag does not need (module documentation of `interp`).
fn store_property(vm: &mut Vm, object: ObjRef, at: PropAddr, value: Value) -> Result<(), VmError> {
    let kind = vm
        .space
        .classes
        .decl(at.prop)
        .map(|d| d.kind.clone())
        .ok_or_else(|| HeapError::from(ow_object::layout::LayoutError::NotAProperty(at.prop)))?;
    // A class is an object in UnrealScript: an object property declared as a class's ancestor holds one. The slot
    // has no variant for it, so it is written unchecked after its address is checked.
    if let (PropKind::Object { class }, Value::Class(_)) = (&kind, &value) {
        let declared = vm.space.classes.get_layout(*class);
        if declared.is_some_and(|l| {
            CLASS_ANCESTORS
                .iter()
                .any(|p| l.path().as_str().eq_ignore_ascii_case(p))
        }) {
            *vm.space.slot_mut(object, at)? = value;
            return Ok(());
        }
    }
    let value = fit_for_heap(vm, &kind, value);
    match vm.space.write(object, at, value) {
        // An object of an engine-native stand-in class: its real chain is unknown, so the refusal is not proven.
        Err(HeapError::WrongClass { found, .. }) if through_native(vm, found) => {
            Err(NotYet::not_read(
                "whether an object of an engine-native stand-in class fits a typed object property",
                "decision 2026-11-15-08 (engine-native stand-in classes)",
            )
            .into())
        }
        other => Ok(other?),
    }
}

/// Whether `class` or a super class is an engine-native stand-in class (decision 2026-11-15-08).
fn through_native(vm: &Vm, class: ClassId) -> bool {
    let classes = &vm.space.classes;
    let mut cursor = Some(class);
    let mut left = classes.len();
    while let Some(c) = cursor.filter(|_| left > 0) {
        left -= 1;
        if classes.is_native(c) {
            return true;
        }
        cursor = classes.get_layout(c).and_then(|l| l.super_class());
    }
    false
}

/// The representation rules a heap slot needs that the C++ bag does not (module documentation of `interp`).
fn fit_for_heap(vm: &Vm, kind: &PropKind, value: Value) -> Value {
    match (kind, value) {
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
    enter(vm, f)?;
    let value = eval_inner(vm, f, e, ctx);
    vm.code.nesting -= 1;
    value
}

/// The dispatch of `eval`. Each arm only calls the function that does the work, so this frame, which every level of
/// expression nesting holds, stays small in an unoptimised build too (module documentation of `interp`, "Stack
/// depth").
fn eval_inner(vm: &mut Vm, f: &mut Frame, e: &Expr, ctx: Ctx) -> Result<Value, VmError> {
    vm.tick()?;
    if e.is_native_call() {
        return call_native(vm, f, e, ctx);
    }
    match e.op {
        op::INT_CONST
        | op::FLOAT_CONST
        | op::STRING_CONST
        | op::UNICODE_STRING_CONST
        | op::NAME_CONST
        | op::INT_ZERO
        | op::INT_ONE
        | op::TRUE
        | op::FALSE
        | op::NO_OBJECT
        | op::VECTOR_CONST
        | op::ROTATION_CONST
        | op::BYTE_CONST
        | op::INT_CONST_BYTE => constant(vm, e),
        op::SELF => Ok(Value::Object(ctx)),
        op::NOTHING
        | op::EMPTY_PARM_VALUE
        | op::END_PARM_VALUE
        | op::DEBUG_INFO
        | op::LABEL_TABLE
        | op::FILTER_EDITOR_ONLY
        | op::EAT_RETURN_VALUE
        | op::NATIVE_PARM => Ok(Value::None),
        op::OBJECT_CONST => object_const(vm, f, e.refs[0]),
        op::LOCAL_VARIABLE
        | op::LOCAL_OUT_VARIABLE
        | op::INSTANCE_VARIABLE
        | op::DEFAULT_VARIABLE
        | op::OP_4C..=op::OP_50
        | op::OP_5E
        | op::STATE_VARIABLE
        | op::BOOL_VARIABLE => variable(vm, f, e, ctx),
        op::CONTEXT | op::CLASS_CONTEXT => context(vm, f, e, ctx),
        op::SKIP => eval(vm, f, &e.kids[0], ctx),
        op::OP_5F => let_attribute(vm, f, e, ctx),
        op::LET | op::LET_BOOL => let_(vm, f, e, ctx),
        op::CONDITIONAL => conditional(vm, f, e, ctx),
        op::VIRTUAL_FUNCTION => call_virtual(vm, f, e, ctx, false),
        op::GLOBAL_FUNCTION => call_virtual(vm, f, e, ctx, true),
        op::FINAL_FUNCTION => call_final(vm, f, e, ctx),
        op::PRIMITIVE_CAST => primitive_cast(vm, f, e, ctx),
        op::DYNAMIC_CAST => dynamic_cast(vm, f, e, ctx),
        op::META_CAST => meta_cast(vm, f, e, ctx),
        op::NEW => new_object(vm, f, e, ctx),
        op::ASSERT => assert(vm, f, e, ctx),
        op::RETURN_NOTHING => Ok(zero_for(vm, f, e.refs[0])),
        op::INTERFACE_CONTEXT => Err(interfaces("InterfaceContext")),
        op::INTERFACE_CAST => Err(interfaces("InterfaceCast")),
        op::STRUCT_MEMBER
        | op::ARRAY_ELEMENT
        | op::DYN_ARRAY_ELEMENT
        | op::STRUCT_CMP_EQ
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
        | op::DYN_ARRAY_SORT => Err(aggregate::element(e.op)),
        op::DELEGATE_FUNCTION
        | op::LET_DELEGATE
        | op::EQUAL_EQUAL_DEL_DEL
        | op::EQUAL_EQUAL_DEL_FUNC
        | op::NOT_EQUAL_DEL_DEL
        | op::NOT_EQUAL_DEL_FUNC
        | op::EMPTY_DELEGATE
        | op::DELEGATE_PROPERTY
        | op::INSTANCE_DELEGATE => Err(delegate::expression(e.op)),
        other => Err(not_evaluable(f, other)),
    }
}

/// The constants.
fn constant(vm: &mut Vm, e: &Expr) -> Result<Value, VmError> {
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
        _ => Value::Object(None), // NoObject
    })
}

/// A variable: its place, then its value.
fn variable(vm: &mut Vm, f: &mut Frame, e: &Expr, ctx: Ctx) -> Result<Value, VmError> {
    let place = lval(vm, f, e, ctx)?;
    read(vm, &place)
}

/// `Context` / `ClassContext`: the member against the context's object; on None, the zero of the return property.
fn context(vm: &mut Vm, f: &mut Frame, e: &Expr, ctx: Ctx) -> Result<Value, VmError> {
    match context_target(vm, f, e, ctx)? {
        None => {
            note_none_context(vm, f);
            Ok(zero_for(vm, f, e.refs[0]))
        }
        Some(target) => eval(vm, f, &e.kids[1], Some(target)),
    }
}

/// 0x5F: a statement, no result.
fn let_attribute(vm: &mut Vm, f: &mut Frame, e: &Expr, ctx: Ctx) -> Result<Value, VmError> {
    attr::let_attribute(vm, f, e, ctx)?;
    Ok(Value::None)
}

/// `Let` / `LetBool`: the place first, then the value; the assignment's value is what the target now holds (not a
/// second evaluation of the left side).
fn let_(vm: &mut Vm, f: &mut Frame, e: &Expr, ctx: Ctx) -> Result<Value, VmError> {
    let lhs = &e.kids[0];
    if lhs.op == op::DYN_ARRAY_LENGTH {
        return Err(aggregate::element(op::DYN_ARRAY_LENGTH));
    }
    let place = lval(vm, f, lhs, ctx)?;
    let value = eval(vm, f, &e.kids[1], ctx)?;
    assign_to(vm, &place, value)
}

fn conditional(vm: &mut Vm, f: &mut Frame, e: &Expr, ctx: Ctx) -> Result<Value, VmError> {
    let chosen = if eval(vm, f, &e.kids[0], ctx)?.truth(&vm.space.names) {
        &e.kids[1]
    } else {
        &e.kids[2]
    };
    eval(vm, f, chosen, ctx)
}

/// `FinalFunction`: the function the reference names (a super call is one too).
fn call_final(vm: &mut Vm, f: &mut Frame, e: &Expr, ctx: Ctx) -> Result<Value, VmError> {
    let Some(function) = function_at(vm, f.code.package, e.refs[0])? else {
        return Err(script_error(
            f.path(),
            format!("call to a missing function (reference {})", e.refs[0]),
        ));
    };
    call_function(vm, f, function, ctx, e)
}

fn primitive_cast(vm: &mut Vm, f: &mut Frame, e: &Expr, ctx: Ctx) -> Result<Value, VmError> {
    let v = eval(vm, f, &e.kids[0], ctx)?;
    convert::cast(vm, e.bytes[0], v)
}

/// `MetaCast`: the class when it is the wanted class or a child of it, else a null class.
fn meta_cast(vm: &mut Vm, f: &mut Frame, e: &Expr, ctx: Ctx) -> Result<Value, VmError> {
    let v = eval(vm, f, &e.kids[0], ctx)?;
    let want = class_at(vm, f.code.package, e.refs[0], f.path())?;
    Ok(match (v, want) {
        (Value::Class(Some(c)), Some(want)) if is_child(vm, c, want)? => Value::Class(Some(c)),
        _ => Value::Class(None),
    })
}

/// `New`: the class operand first, then (for a class) the object and the outer operand, as the C++ evaluates them.
fn new_object(vm: &mut Vm, f: &mut Frame, e: &Expr, ctx: Ctx) -> Result<Value, VmError> {
    let class = eval(vm, f, &e.kids[3], ctx)?;
    let Value::Class(Some(class)) = class else {
        return Ok(Value::Object(None));
    };
    let object = instantiate_from_defaults(vm, class)?;
    if let Value::Object(outer) = eval(vm, f, &e.kids[0], ctx)? {
        vm.space.heap.get_mut(object)?.outer = outer;
    }
    Ok(Value::Object(Some(object)))
}

fn assert(vm: &mut Vm, f: &mut Frame, e: &Expr, ctx: Ctx) -> Result<Value, VmError> {
    if !eval(vm, f, &e.kids[0], ctx)?.truth(&vm.space.names) {
        vm.log
            .push(format!("Assertion failed at line {}", e.words[0]));
    }
    Ok(Value::None)
}

fn not_evaluable(f: &Frame, op: u8) -> VmError {
    script_error(f.path(), format!("expression 0x{op:02x} is not evaluable"))
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

/// `Object.IsA(name)` (`natives_core.rs`): whether `class` or a super class is named `name` (any case). Through an
/// engine-native stand-in class the real chain is unknown, so a "no" there is `NotYet`, not `false`.
pub(crate) fn is_a_named(vm: &Vm, class: ClassId, name: &str) -> Result<bool, VmError> {
    let classes = &vm.space.classes;
    let mut cursor = Some(class);
    let mut left = classes.len();
    while let Some(c) = cursor.filter(|_| left > 0) {
        left -= 1;
        let Some(layout) = classes.get_layout(c) else {
            break;
        };
        if layout.path().name().eq_ignore_ascii_case(name) {
            return Ok(true);
        }
        if classes.is_native(c) {
            return Err(NotYet::not_read(
                format!(
                    "whether a class is named {name} above the engine-native class {}",
                    layout.path().as_str()
                ),
                "decision 2026-11-15-08 (engine-native stand-in classes)",
            )
            .into());
        }
        cursor = layout.super_class();
    }
    Ok(false)
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
/// arguments are still evaluated); `&&` and `||` skip their second operand when the first decides the result. Kept
/// small: it is on the stack while the arguments are evaluated.
fn call_native(vm: &mut Vm, f: &mut Frame, e: &Expr, ctx: Ctx) -> Result<Value, VmError> {
    match native_target(vm, e)? {
        NativeTarget::Missing => unbound_native_index(vm, f, e),
        NativeTarget::ShortCircuit(and) => short_circuit(vm, f, e, and),
        NativeTarget::Call(function) => call_function(vm, f, function, ctx, e),
    }
}

enum NativeTarget {
    Missing,
    /// `&&` (true) or `||` (false) with two operands.
    ShortCircuit(bool),
    Call(FuncId),
}

fn native_target(vm: &mut Vm, e: &Expr) -> Result<NativeTarget, VmError> {
    let Some(function) = native_by_index(vm, e.native)? else {
        return Ok(NativeTarget::Missing);
    };
    let symbol = code_of(vm, function).map(|c| c.info.friendly_name.clone());
    Ok(match symbol.as_deref() {
        Some("&&") if e.kids.len() == 2 => NativeTarget::ShortCircuit(true),
        Some("||") if e.kids.len() == 2 => NativeTarget::ShortCircuit(false),
        _ => NativeTarget::Call(function),
    })
}

fn unbound_native_index(vm: &mut Vm, f: &mut Frame, e: &Expr) -> Result<Value, VmError> {
    vm.stub_site(
        &format!("native index {}", e.native),
        format!("UNIMPLEMENTED native index {}", e.native),
    )?;
    for kid in &e.kids {
        if kid.op != op::EMPTY_PARM_VALUE {
            eval(vm, f, kid, f.this)?;
        }
    }
    Ok(Value::None)
}

fn short_circuit(vm: &mut Vm, f: &mut Frame, e: &Expr, and: bool) -> Result<Value, VmError> {
    let first = eval(vm, f, &e.kids[0], f.this)?.truth(&vm.space.names);
    if first != and {
        return Ok(Value::Bool(first));
    }
    let second = eval(vm, f, &e.kids[1], f.this)?;
    Ok(Value::Bool(second.truth(&vm.space.names)))
}

/// `bindArguments` + `invoke`: the arguments are evaluated in the caller's own context (the frame's object, not the
/// call's target: UE3's `Stack.Object` versus `Context`, UNVERIFIED), an omitted optional one is not supplied, an
/// out argument aliases its place (the callee writes the caller's storage live), surplus arguments are evaluated and
/// dropped. Out arguments were written through their aliases during the call, so nothing is copied back.
fn call_function(
    vm: &mut Vm,
    f: &mut Frame,
    function: FuncId,
    target: Ctx,
    e: &Expr,
) -> Result<Value, VmError> {
    let args = bind_arguments(vm, f, function, e)?;
    invoke(vm, function, target, args)
}

fn bind_arguments(
    vm: &mut Vm,
    f: &mut Frame,
    function: FuncId,
    e: &Expr,
) -> Result<Vec<Arg>, VmError> {
    let shape = parameter_shape(vm, function);
    let mut args = Vec::with_capacity(shape.len());
    for (i, &(out, optional)) in shape.iter().enumerate() {
        args.push(bind_argument(vm, f, e.kids.get(i), out, optional)?);
    }
    for kid in e.kids.iter().skip(shape.len()) {
        eval(vm, f, kid, f.this)?;
    }
    Ok(args)
}

/// Each parameter's (out, optional).
fn parameter_shape(vm: &Vm, function: FuncId) -> Vec<(bool, bool)> {
    vm.function(function)
        .map(|d| {
            d.params
                .iter()
                .map(|p| (p.is_out(), p.is_optional()))
                .collect()
        })
        .unwrap_or_default()
}

fn bind_argument(
    vm: &mut Vm,
    f: &mut Frame,
    kid: Option<&Expr>,
    out: bool,
    optional: bool,
) -> Result<Arg, VmError> {
    let Some(kid) = kid else {
        return Ok(Arg::new(Value::None, out, false));
    };
    if kid.op == op::EMPTY_PARM_VALUE || (kid.op == op::NOTHING && optional) {
        return Ok(Arg::new(Value::None, out, false));
    }
    if !out {
        let value = eval(vm, f, kid, f.this)?;
        return Ok(Arg::new(value, false, true));
    }
    let place = lval(vm, f, kid, f.this)?;
    out_argument(vm, place)
}

fn out_argument(vm: &Vm, place: Place) -> Result<Arg, VmError> {
    let value = read(vm, &place)?;
    Ok(match place {
        // Not a storage location (the C++ scratch): the callee keeps its own copy.
        Place::Temp(_) => Arg::new(value, true, true),
        place => Arg::aliasing(value, super::Alias(place)),
    })
}

/// The call itself, once the arguments are bound.
fn invoke(vm: &mut Vm, function: FuncId, target: Ctx, args: Vec<Arg>) -> Result<Value, VmError> {
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
    Ok(vm.call_bound(function, target, args)?.ret)
}
