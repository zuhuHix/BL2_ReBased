//! Owner: lane W4-INTERP2 (architecture section 8). `foreach`: dynamic-array iterators and engine iterators, a port
//! of `EX_DynArrayIterator`, `EX_Iterator`, `EX_IteratorNext`, `EX_IteratorPop` and `startEngineIterator` in
//! `src/interp.cpp`, with the iterator natives the VM registry holds (`Vm::register_iterator`). AI-assisted (Claude
//! Code).
//!
//! **Layout** (as in the installed packages, the C++ reading): `DynArrayIterator [array][item][has index][index]
//! [end]` and `Iterator [call][end]` start a loop; the body ends at `IteratorNext`, and the end offset (where a
//! `break` lands too) is the loop's `IteratorPop`, which pops one state. A state is pushed even when the loop body
//! never runs, so an empty inner loop does not pop the enclosing one.
//!
//! **Dynamic arrays.** The array, the item and the index are places (`eval::Place`), taken once when the loop starts,
//! as the C++ keeps `Value*`s; the array is read through its place at every `IteratorNext`, so a body that changes the
//! array is seen, as in the C++. The item and the index are written as they are (no assignment conversion), as the
//! C++ writes through the pointers.
//!
//! **Engine iterators.** The call sits under any `Context` wrappers and must be a numbered native with an iterator
//! bound; the wrappers are resolved first (a None object skips the loop, counted as a None context), then the
//! arguments are bound like an ordinary call and the iterator returns its list, which the loop walks with the call's
//! out parameter as the loop variable. An object destroyed since the list was made is skipped (a handle the heap no
//! longer knows is skipped too, Rust-only: the C++ list keeps its objects alive), and the variable reads None once the
//! list is exhausted. A call that is not a bound iterator is the C++ stub site (`StubPolicy`, the iterator native's
//! key, `UNIMPLEMENTED native iterator (foreach over an engine iterator)`) and the loop is jumped over without a
//! state pushed, as the C++ does (so its `IteratorPop` pops the enclosing loop's state, as there).
//!
//! **Bound** (Rust-only, a typed `VmError::Script`): a frame holds at most [`MAX_ITERATORS`] loop states at once.
//! Compiled script pushes one per syntactic level of nesting; only a jump back to a loop's start without its
//! `IteratorPop` could pile them up.
//!
//! Status: a port of the C++ reading; UNVERIFIED in the running game (the C++ labels the iterator natives
//! UNVERIFIED too: `DynamicActors` visits the same list as `AllActors`, and the list is taken when the loop starts).

use ow_object::ids::FuncId;
use ow_object::value::Value;
use ow_script::decode::{Code, Expr};
use ow_script::ops::op;

use super::eval::{
    Frame, Place, bind_arguments, context_target, lval, note_none_context, read, store, target,
};
use super::load::{function_at, native_by_index, script_error};
use crate::api::{NativeCall, Vm};
use crate::error::VmError;

/// The most loop states one frame may hold at once (a Rust-only refusal; module documentation).
pub const MAX_ITERATORS: usize = 4096;

/// The C++ stub line of a `foreach` over a call that is not a bound iterator.
const UNBOUND: &str = "UNIMPLEMENTED native iterator (foreach over an engine iterator)";

/// One frame's loop states, innermost last (the C++ `Frame::iterators`).
#[derive(Debug, Default)]
pub(crate) struct Iterators {
    states: Vec<State>,
}

#[derive(Debug)]
struct State {
    source: Source,
    item: Place,
    index: Option<Place>,
    position: usize,
    /// The statement to run for each element.
    resume: usize,
}

#[derive(Debug)]
enum Source {
    /// A dynamic array, read through its place at each step.
    Array(Place),
    /// The list an engine iterator returned.
    Engine(Vec<Value>),
}

impl Iterators {
    fn push(&mut self, f: &Frame, state: State) -> Result<(), VmError> {
        if self.states.len() >= MAX_ITERATORS {
            return Err(script_error(
                f.path(),
                format!(
                    "more than {MAX_ITERATORS} foreach loops are active at once in {} (a Rust-only bound)",
                    f.path()
                ),
            ));
        }
        self.states.push(state);
        Ok(())
    }
}

/// Runs the iterator statement `s` at `pc`; returns the next statement.
pub(crate) fn statement(
    vm: &mut Vm,
    f: &mut Frame,
    iterators: &mut Iterators,
    script: &Code,
    s: &Expr,
    pc: usize,
) -> Result<usize, VmError> {
    match s.op {
        op::DYN_ARRAY_ITERATOR => dyn_array(vm, f, iterators, script, s, pc),
        op::ITERATOR => engine(vm, f, iterators, script, s, pc),
        op::ITERATOR_NEXT => next(vm, iterators, pc),
        _ => {
            // IteratorPop.
            iterators.states.pop();
            Ok(pc + 1)
        }
    }
}

fn operand<'e>(f: &Frame, s: &'e Expr, i: usize) -> Result<&'e Expr, VmError> {
    s.kids.get(i).ok_or_else(|| {
        script_error(
            f.path(),
            format!(
                "statement 0x{:02x} has no operand {i} in {}",
                s.op,
                f.path()
            ),
        )
    })
}

fn end_offset(f: &Frame, script: &Code, s: &Expr) -> Result<usize, VmError> {
    let offset = s.words.first().copied().ok_or_else(|| {
        script_error(
            f.path(),
            format!("statement 0x{:02x} has no end offset in {}", s.op, f.path()),
        )
    })?;
    target(f, script, offset)
}

/// `DynArrayIterator`: the places of the array, the item and (when the byte says so) the index; then the first
/// element, or a jump to the end for an empty array.
fn dyn_array(
    vm: &mut Vm,
    f: &mut Frame,
    iterators: &mut Iterators,
    script: &Code,
    s: &Expr,
    pc: usize,
) -> Result<usize, VmError> {
    let this = f.this;
    let array = lval(vm, f, operand(f, s, 0)?, this)?;
    let item = lval(vm, f, operand(f, s, 1)?, this)?;
    let index = if s.bytes.first().is_some_and(|&b| b != 0) {
        Some(lval(vm, f, operand(f, s, 2)?, this)?)
    } else {
        None
    };
    let first = read(vm, &array)?
        .elements()
        .and_then(|items| items.first().cloned());
    iterators.push(
        f,
        State {
            source: Source::Array(array),
            item: item.clone(),
            index: index.clone(),
            position: 0,
            resume: pc + 1,
        },
    )?;
    let Some(first) = first else {
        return end_offset(f, script, s);
    };
    store(vm, &item, first)?;
    if let Some(index) = &index {
        store(vm, index, Value::Int(0))?;
    }
    Ok(pc + 1)
}

/// `Iterator`: a foreach over an engine iterator (`startEngineIterator`), or the stub site and a jump to the end.
fn engine(
    vm: &mut Vm,
    f: &mut Frame,
    iterators: &mut Iterators,
    script: &Code,
    s: &Expr,
    pc: usize,
) -> Result<usize, VmError> {
    let outer = operand(f, s, 0)?;
    let call = unwrap_context(outer);
    let bound = if call.is_native_call() {
        match native_by_index(vm, call.native)? {
            Some(function) => vm.iterator_for(function).map(|it| (function, it)),
            None => None,
        }
    } else {
        None
    };
    let Some((function, iterator)) = bound else {
        let named = iterator_function(vm, f, call)?;
        let key = named.map_or_else(
            || "foreach over an engine iterator".to_string(),
            |id| {
                vm.function(id)
                    .map_or_else(String::new, |d| d.display_key().to_string())
            },
        );
        vm.stub_site(&key, UNBOUND.to_string())?;
        if vm.trace.is_some() {
            // The call trace names which iterator it was (the stub line does not).
            let name = match named {
                Some(id) => vm.function_path(id),
                None => call
                    .names
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "?".to_string()),
            };
            if let Some(trace) = &mut vm.trace {
                trace.push(format!("iterator {name}"));
            }
        }
        return end_offset(f, script, s);
    };
    // The object chain under the Context wrappers, from the frame's own object.
    let mut receiver = f.this;
    let mut wrapper = outer;
    while !std::ptr::eq(wrapper, call) {
        receiver = context_target(vm, f, wrapper, receiver)?;
        if receiver.is_none() {
            break;
        }
        wrapper = operand(f, wrapper, 1)?;
    }
    let mut item: Option<Place> = None;
    let mut items = Vec::new();
    match receiver {
        Some(receiver) => {
            vm.tick()?;
            if vm.census.is_some() {
                let path = vm.function_path(function);
                if let Some(census) = &mut vm.census {
                    *census.native_calls.entry(path).or_insert(0) += 1;
                }
            }
            let args = bind_arguments(vm, f, function, call)?;
            let mut native = NativeCall::new(vm, function, Some(receiver), args);
            let result = iterator(&mut native)?;
            let args = std::mem::take(&mut native.args);
            if let Some(arg) = args.get(result.out_param) {
                // The C++ keeps the out argument's alias: the caller's storage, or a temporary for an argument that is
                // not a storage location; an argument that was not supplied has none.
                item = match &arg.alias {
                    Some(alias) => Some(alias.0.clone()),
                    None if arg.out && arg.supplied => Some(Place::Temp(Value::None)),
                    None => None,
                };
                if item.is_some() {
                    items = result.items;
                }
            }
        }
        None => note_none_context(vm, f),
    }
    let empty = items.is_empty();
    let item = match item {
        // A None object, or no usable out parameter: the loop body does not run.
        None => Place::Temp(Value::None),
        Some(item) => {
            let first = items.first().cloned().unwrap_or(Value::Object(None));
            store(vm, &item, first)?;
            item
        }
    };
    iterators.push(
        f,
        State {
            source: Source::Engine(items),
            item,
            index: None,
            position: 0,
            resume: pc + 1,
        },
    )?;
    if empty {
        end_offset(f, script, s)
    } else {
        Ok(pc + 1)
    }
}

/// The call under any `Context` / `ClassContext` wrappers (each with its two operands).
fn unwrap_context(e: &Expr) -> &Expr {
    let mut call = e;
    while matches!(call.op, op::CONTEXT | op::CLASS_CONTEXT) && call.kids.len() == 2 {
        call = &call.kids[1];
    }
    call
}

/// The function an unbound iterator call names, for the stub key and the trace: a numbered native, or the function a
/// `FinalFunction` names.
fn iterator_function(vm: &mut Vm, f: &Frame, call: &Expr) -> Result<Option<FuncId>, VmError> {
    if call.is_native_call() {
        return native_by_index(vm, call.native);
    }
    match (call.op, call.refs.first()) {
        (op::FINAL_FUNCTION, Some(&reference)) => function_at(vm, f.code.package, reference),
        _ => Ok(None),
    }
}

/// `IteratorNext`: the next element (an engine list skips destroyed objects) and a jump back to the body, or, past
/// the end, on to the `IteratorPop` (an engine loop's variable reads None then).
fn next(vm: &mut Vm, iterators: &mut Iterators, pc: usize) -> Result<usize, VmError> {
    let Some(state) = iterators.states.last_mut() else {
        return Ok(pc + 1);
    };
    state.position += 1;
    let value = match &state.source {
        Source::Array(array) => read(vm, array)?
            .elements()
            .and_then(|items| items.get(state.position).cloned()),
        Source::Engine(items) => {
            while items.get(state.position).is_some_and(|v| destroyed(vm, v)) {
                state.position += 1;
            }
            items.get(state.position).cloned()
        }
    };
    match value {
        Some(value) => {
            store(vm, &state.item, value)?;
            if let Some(index) = &state.index {
                let position = i32::try_from(state.position).unwrap_or(i32::MAX);
                store(vm, index, Value::Int(position))?;
            }
            Ok(state.resume)
        }
        None => {
            if matches!(state.source, Source::Engine(_)) {
                store(vm, &state.item, Value::Object(None))?;
            }
            Ok(pc + 1)
        }
    }
}

/// An object destroyed since the list was made (or no longer in the heap at all).
fn destroyed(vm: &Vm, v: &Value) -> bool {
    match v {
        Value::Object(Some(o)) => vm.space.heap.get(*o).map_or(true, |o| o.destroyed),
        _ => false,
    }
}

/// The `foreach` cases of `tests/vm_test.py` (its engine-iterator block, with the C++ results it asserts), the
/// dynamic-array loop and the bounds, on the shared fixture of `aggregate.rs`.
#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use ow_core::ParityMode;
    use ow_object::value::Value;
    use ow_package::testkit::script::{Asm, CPF_OPT, CPF_OUT, FunctionIds, Param};

    use crate::api::{CallCensus, IteratorResult, NativeCall};
    use crate::interp::aggregate::tests::{
        Run, Toy, add_item, array_call, class_param, int, int_array, let_, returns, script_message,
        stmt,
    };

    const CPF_PARM: u64 = 0x80;

    /// `N = N + step`.
    fn count(a: &mut Asm, n: i32, step: i32) {
        let_(
            a,
            |a| {
                a.local(n);
            },
            |a| {
                a.native(146, |a| {
                    a.local(n).int_const(step);
                });
            },
        );
    }

    /// `Iterator [Context(W, ...)] native(C, A[, <omitted>]) <end>`; returns where the end word is.
    fn foreach_native(
        a: &mut Asm,
        number: u16,
        wrapper: Option<i32>,
        base: i32,
        item: i32,
        arity: usize,
    ) -> usize {
        a.stmt().raw(&[0x2F]);
        if let Some(w) = wrapper {
            a.raw(&[0x19]).local(w).w(0).reference(0).raw(&[0]);
        }
        a.raw(&[0x60 + (number >> 8) as u8, (number & 0xFF) as u8]);
        a.local(base).local(item);
        if arity == 3 {
            a.raw(&[0x4A]);
        }
        a.raw(&[0x16]);
        let at = a.bytes.len();
        a.w(0);
        at
    }

    fn patch_at(a: &mut Asm, at: usize, target: u32) {
        let target = u16::try_from(target).unwrap();
        a.bytes[at..at + 2].copy_from_slice(&target.to_le_bytes());
    }

    #[derive(Clone, Copy)]
    enum Body {
        Count,
        Break,
        Continue,
        BreakAtOnce,
    }

    /// `N = 0; foreach native(C, A) { body } return N` (or `return A`), built twice to learn the offsets of the
    /// loop's `IteratorNext` and `IteratorPop`.
    fn loop_function(
        ids: &FunctionIds,
        a: &mut Asm,
        number: u16,
        wrapped: bool,
        body: Body,
        result_a: bool,
        arity: usize,
    ) {
        let (n, c, item) = (ids.get("N"), ids.get("C"), ids.get("A"));
        let w = wrapped.then(|| ids.get("W"));
        let once = |next_off: u32, end_off: u32| {
            let mut a = Asm::new();
            let_(
                &mut a,
                |a| {
                    a.local(n);
                },
                |a| {
                    a.raw(&[0x25]);
                },
            );
            let at = foreach_native(&mut a, number, w, c, item, arity);
            match body {
                Body::Count => count(&mut a, n, 1),
                Body::Break => {
                    count(&mut a, n, 1);
                    a.stmt().raw(&[0x06]).w(u16::try_from(end_off).unwrap());
                }
                Body::Continue => {
                    count(&mut a, n, 1);
                    a.stmt().raw(&[0x06]).w(u16::try_from(next_off).unwrap());
                    count(&mut a, n, 100);
                }
                Body::BreakAtOnce => {
                    a.stmt().raw(&[0x06]).w(u16::try_from(end_off).unwrap());
                }
            }
            let here_next = a.here();
            a.stmt().raw(&[0x31]);
            let here_end = a.here();
            a.stmt().raw(&[0x30]);
            patch_at(&mut a, at, here_end);
            returns(&mut a, |a| {
                a.local(if result_a { item } else { n });
            });
            (a, here_next, here_end)
        };
        let (_, next_off, end_off) = once(0, 0);
        *a = once(next_off, end_off).0;
    }

    struct Actors {
        toy: Toy,
        actor: i32,
        seeker: i32,
    }

    /// vm_test.py's classes: Beacon and Crate are Actors, BeaconRed is a Beacon, Ghost is never instantiated; the
    /// iterator natives AllActors (304), DynamicActors (313) and ChildActors (305, nothing bound) on Actor.
    fn actors() -> Actors {
        let mut toy = Toy::new();
        let object = toy.object;
        let actor = toy.t.class("Actor", object);
        let beacon = toy.t.class("Beacon", actor);
        toy.t.class("BeaconRed", beacon);
        toy.t.class("Crate", actor);
        toy.t.class("Ghost", actor);
        for (name, number) in [
            ("AllActors", 304u16),
            ("DynamicActors", 313),
            ("ChildActors", 305),
        ] {
            let mut params = vec![
                Param::new("Class", "BaseClass"),
                Param {
                    flags: CPF_OUT,
                    type_ref: actor,
                    ..Param::new("Object", "Actor")
                },
            ];
            if name != "ChildActors" {
                params.push(Param {
                    flags: CPF_OPT,
                    ..Param::new("Class", "InterfaceClass")
                });
            }
            let ids = toy.native(actor, name, &params, None, number);
            class_param(&mut toy, ids.get("BaseClass"), CPF_PARM, object);
            if name != "ChildActors" {
                class_param(
                    &mut toy,
                    ids.get("InterfaceClass"),
                    CPF_PARM | CPF_OPT,
                    object,
                );
            }
        }
        let seeker = toy.t.class("Seeker", object);
        Actors { toy, actor, seeker }
    }

    impl Actors {
        /// A Seeker function with parameters C (a class), A (an Actor), N (an int) and those in `extra`.
        fn declare(
            &mut self,
            name: &str,
            extra: &[Param],
            result_a: bool,
            body: impl FnOnce(&FunctionIds, &mut Asm),
        ) -> FunctionIds {
            let mut params = vec![
                Param::new("Class", "C"),
                Param {
                    type_ref: self.actor,
                    ..Param::new("Object", "A")
                },
                Param::new("Int", "N"),
            ];
            params.extend_from_slice(extra);
            let result = if result_a {
                ("Object", self.actor)
            } else {
                ("Int", 0)
            };
            let seeker = self.seeker;
            let ids = self.toy.function(
                seeker,
                name,
                super::super::aggregate::tests::SCRIPT,
                &params,
                Some(result),
                &[],
                body,
            );
            let object = self.toy.object;
            class_param(&mut self.toy, ids.get("C"), CPF_PARM, object);
            for p in extra.iter().filter(|p| p.kind == "Class") {
                class_param(&mut self.toy, ids.get(&p.name), CPF_PARM, object);
            }
            ids
        }
    }

    fn class(r: &mut Run, path: &str) -> Value {
        Value::Class(Some(r.vm.find_class(path).unwrap()))
    }

    /// The text of an object result (`Class'Name'`).
    fn text(r: &Run, v: &Value) -> String {
        crate::interp::describe(&r.vm, v)
    }

    #[test]
    fn engine_iterators_visit_the_live_objects_of_the_class_in_creation_order() {
        let mut x = actors();
        x.declare("CountAll", &[], false, |ids, a| {
            loop_function(ids, a, 304, false, Body::Count, false, 3)
        });
        x.declare("CountDynamic", &[], false, |ids, a| {
            loop_function(ids, a, 313, false, Body::Count, false, 3)
        });
        let w = Param {
            type_ref: x.toy.object,
            ..Param::new("Object", "W")
        };
        x.declare("CountVia", &[w], false, |ids, a| {
            loop_function(ids, a, 304, true, Body::Count, false, 3)
        });
        x.declare("CountBreak", &[], false, |ids, a| {
            loop_function(ids, a, 304, false, Body::Break, false, 3)
        });
        x.declare("CountContinue", &[], false, |ids, a| {
            loop_function(ids, a, 304, false, Body::Continue, false, 3)
        });
        x.declare("FirstBreak", &[], true, |ids, a| {
            loop_function(ids, a, 304, false, Body::BreakAtOnce, true, 3)
        });
        x.declare("LastAfterLoop", &[], true, |ids, a| {
            loop_function(ids, a, 304, false, Body::Count, true, 3)
        });
        x.declare("CountChildren", &[], false, |ids, a| {
            loop_function(ids, a, 305, false, Body::Count, false, 2)
        });
        // Total = 0; foreach AllActors(C, A) { Total += 10; foreach AllActors(D, B) Total += 1 } return Total.
        let d = Param::new("Class", "D");
        let b = Param {
            type_ref: x.actor,
            ..Param::new("Object", "B")
        };
        x.declare("Nested", &[d, b], false, |ids, a| {
            let (n, c, item, d, b) = (
                ids.get("N"),
                ids.get("C"),
                ids.get("A"),
                ids.get("D"),
                ids.get("B"),
            );
            let_(
                a,
                |a| {
                    a.local(n);
                },
                |a| {
                    a.raw(&[0x25]);
                },
            );
            let outer = foreach_native(a, 304, None, c, item, 3);
            count(a, n, 10);
            let inner = foreach_native(a, 304, None, d, b, 3);
            count(a, n, 1);
            a.stmt().raw(&[0x31]);
            let inner_end = a.here();
            a.stmt().raw(&[0x30]);
            patch_at(a, inner, inner_end);
            a.stmt().raw(&[0x31]);
            let outer_end = a.here();
            a.stmt().raw(&[0x30]);
            patch_at(a, outer, outer_end);
            returns(a, |a| {
                a.local(n);
            });
        });
        let mut r = x.toy.run(ParityMode::Off);
        let new = |r: &mut Run, c: &str| r.object(c);
        let b1 = new(&mut r, "Core.Beacon");
        new(&mut r, "Core.BeaconRed");
        let c1 = new(&mut r, "Core.Crate");
        new(&mut r, "Core.Crate");
        new(&mut r, "Core.Crate");
        let s = new(&mut r, "Core.Seeker");
        let run = |r: &mut Run, f: &str, args: Vec<Value>| -> Value {
            r.on(s, &format!("Core.Seeker.{f}"), args)
        };
        let beacon = class(&mut r, "Core.Beacon");
        let red = class(&mut r, "Core.BeaconRed");
        let actor = class(&mut r, "Core.Actor");
        let crate_ = class(&mut r, "Core.Crate");
        let ghost = class(&mut r, "Core.Ghost");
        // A child class counts; every actor; None and a class nobody instantiated match nothing.
        let counted: Vec<Value> = [
            beacon.clone(),
            red,
            actor,
            Value::Class(None),
            ghost.clone(),
        ]
        .into_iter()
        .map(|c| run(&mut r, "CountAll", vec![c]))
        .collect();
        assert_eq!(counted, vec![int(2), int(1), int(5), int(0), int(0)]);
        // DynamicActors; through Context(b1); a None Context skips the loop (and counts as a None context).
        assert_eq!(run(&mut r, "CountDynamic", vec![crate_.clone()]), int(3));
        let none = Value::Object(None);
        let via = |o: Value| vec![crate_.clone(), none.clone(), int(0), o];
        assert_eq!(
            run(&mut r, "CountVia", via(Value::Object(Some(b1)))),
            int(3)
        );
        r.vm.census = Some(CallCensus::default());
        assert_eq!(run(&mut r, "CountVia", via(none.clone())), int(0));
        let census = r.vm.census.take().unwrap();
        assert_eq!(census.none_contexts.get("Core.Seeker.CountVia"), Some(&1));
        // Break lands on the IteratorPop; an empty loop runs no pass; continue lands on the IteratorNext.
        assert_eq!(run(&mut r, "CountBreak", vec![crate_.clone()]), int(1));
        assert_eq!(run(&mut r, "CountBreak", vec![ghost.clone()]), int(0));
        assert_eq!(run(&mut r, "CountContinue", vec![crate_.clone()]), int(3));
        // Creation order (the first crate); an exhausted loop leaves the variable None.
        let first = run(&mut r, "FirstBreak", vec![crate_.clone()]);
        assert_eq!(first, Value::Object(Some(c1)), "{}", text(&r, &first));
        assert_eq!(
            run(&mut r, "LastAfterLoop", vec![crate_.clone()]),
            Value::Object(None)
        );
        // 2 beacons x (10 + 3 crates); empty inner loops do not end the outer one.
        let nested =
            |c: &Value, d: &Value| vec![c.clone(), none.clone(), int(0), d.clone(), none.clone()];
        assert_eq!(run(&mut r, "Nested", nested(&beacon, &crate_)), int(26));
        assert_eq!(run(&mut r, "Nested", nested(&beacon, &ghost)), int(20));
        assert_eq!(run(&mut r, "Nested", nested(&ghost, &crate_)), int(0));
        // No iterator bound: skipped with the C++ stub line; nothing was evaluated or called.
        r.vm.log.clear();
        r.vm.trace = Some(Vec::new());
        assert_eq!(run(&mut r, "CountChildren", vec![crate_.clone()]), int(0));
        assert!(
            r.vm.log
                .iter()
                .any(|l| l == "UNIMPLEMENTED native iterator (foreach over an engine iterator)"),
            "{:?}",
            r.vm.log
        );
        assert!(
            !r.vm.log.iter().any(|l| l.contains("ChildActors")),
            "{:?}",
            r.vm.log
        );
        let trace = r.vm.trace.take().unwrap();
        assert_eq!(
            trace,
            vec![
                "Core.Seeker.CountChildren",
                "iterator Core.Actor.ChildActors"
            ]
        );
        // Under a refusing stub policy the unbound iterator is refused by its key.
        r.vm.stub_policy = crate::api::StubPolicy::refuse_all();
        let refused = r.call("Core.Seeker.CountChildren", Some(s), vec![crate_]);
        assert!(
            matches!(&refused, Err(crate::error::VmError::UnboundNative(k)) if k == "Actor.ChildActors(class,object)"),
            "{refused:?}"
        );
    }

    #[test]
    fn an_engine_loop_skips_objects_destroyed_since_the_list_was_made_and_counts_the_call() {
        let mut x = actors();
        x.declare("CountChildren", &[], false, |ids, a| {
            loop_function(ids, a, 305, false, Body::Count, false, 2)
        });
        let mut r = x.toy.run(ParityMode::Off);
        let a = r.object("Core.Crate");
        let gone = r.object("Core.Crate");
        let c = r.object("Core.Crate");
        let s = r.object("Core.Seeker");
        r.vm.space.heap.destroy(gone).unwrap();
        // A test iterator over a fixed list: the destroyed object in the middle is not visited.
        let list = vec![
            Value::Object(Some(a)),
            Value::Object(Some(gone)),
            Value::Object(Some(c)),
        ];
        let fixed: Rc<
            dyn Fn(&mut NativeCall<'_>) -> Result<IteratorResult, crate::error::VmError>,
        > = Rc::new(move |_| Ok(IteratorResult::new(1, list.clone())));
        r.vm.register_iterator("Actor.ChildActors(class,object)", fixed)
            .unwrap();
        r.vm.census = Some(CallCensus::default());
        let crate_ = class(&mut r, "Core.Crate");
        assert_eq!(r.on(s, "Core.Seeker.CountChildren", vec![crate_]), int(2));
        let census = r.vm.census.take().unwrap();
        assert_eq!(census.native_calls.get("Core.Actor.ChildActors"), Some(&1));
    }

    #[test]
    fn a_dynamic_array_loop_walks_the_array_through_its_place() {
        let mut toy = Toy::new();
        // foreach Items(X, I) { S = S + X } return S (or I); and a body that removes the first element.
        for (name, with_index, remove) in [
            ("Sum", false, false),
            ("LastIndex", true, false),
            ("Shrink", false, true),
        ] {
            let (items, inner) = int_array(&mut toy, "Items");
            let ids = toy.script(
                name,
                &[
                    items,
                    Param::new("Int", "X"),
                    Param::new("Int", "I"),
                    Param::new("Int", "S"),
                ],
                Some(("Int", 0)),
                |ids, a| {
                    let (items, xv, iv, sv) =
                        (ids.get("Items"), ids.get("X"), ids.get("I"), ids.get("S"));
                    // DynArrayIterator [array][item][has index][index][end]; the index expression is always there.
                    a.stmt()
                        .raw(&[0x58])
                        .local(items)
                        .local(xv)
                        .raw(&[u8::from(with_index)]);
                    a.local(iv);
                    let at = a.bytes.len();
                    a.w(0);
                    if remove {
                        // The body removes the first element: the loop sees the shorter array.
                        stmt(a, |a| {
                            array_call(
                                a,
                                0x40,
                                |a| {
                                    a.local(items);
                                },
                                |a| {
                                    a.int_const(0).int_const(1);
                                },
                            )
                        });
                        count(a, sv, 1);
                    } else {
                        let_(
                            a,
                            |a| {
                                a.local(sv);
                            },
                            |a| {
                                a.native(146, |a| {
                                    a.local(sv).local(xv);
                                });
                            },
                        );
                    }
                    a.stmt().raw(&[0x31]);
                    let end = a.here();
                    a.stmt().raw(&[0x30]);
                    patch_at(a, at, end);
                    returns(a, |a| {
                        a.local(if with_index { iv } else { sv });
                    });
                },
            );
            toy.adopt(inner, ids.get("Items"));
        }
        let mut r = toy.run(ParityMode::Off);
        let o = r.object("Core.Foo");
        let list = |v: &[i32]| Value::array(v.iter().map(|&x| int(x)).collect());
        assert_eq!(r.on(o, "Core.Foo.Sum", vec![list(&[1, 2, 3, 4])]), int(10));
        assert_eq!(r.on(o, "Core.Foo.Sum", vec![list(&[])]), int(0));
        assert_eq!(
            r.on(o, "Core.Foo.LastIndex", vec![list(&[5, 6, 7])]),
            int(2)
        );
        // [1, 2, 3, 4]: pass one sees 1 and leaves [2, 3, 4]; pass two sees 3 and leaves [3, 4]; then the end.
        assert_eq!(
            r.on(o, "Core.Foo.Shrink", vec![list(&[1, 2, 3, 4])]),
            int(2)
        );
    }

    #[test]
    fn loop_states_past_the_bound_are_refused_and_stray_next_and_pop_step_over() {
        let mut toy = Toy::new();
        let (items, inner) = int_array(&mut toy, "Items");
        let ids = toy.script("Pile", &[items, Param::new("Int", "X")], None, |ids, a| {
            let (items, xv) = (ids.get("Items"), ids.get("X"));
            stmt(a, |a| {
                add_item(
                    a,
                    |a| {
                        a.local(items);
                    },
                    |a| {
                        a.int_const(1);
                    },
                )
            });
            // L: foreach Items(X) { goto L } (a jump back without the IteratorPop).
            let start = a.here();
            a.stmt()
                .raw(&[0x58])
                .local(items)
                .local(xv)
                .raw(&[0])
                .raw(&[0x0B]);
            let at = a.bytes.len();
            a.w(0);
            a.stmt().raw(&[0x06]).w(u16::try_from(start).unwrap());
            a.stmt().raw(&[0x31]);
            let end = a.here();
            a.stmt().raw(&[0x30]);
            patch_at(a, at, end);
            a.end();
        });
        toy.adopt(inner, ids.get("Items"));
        toy.script("Stray", &[], Some(("Int", 0)), |_, a| {
            a.stmt().raw(&[0x31]);
            a.stmt().raw(&[0x30]);
            returns(a, |a| {
                a.int_const(5);
            });
        });
        let mut r = toy.run(ParityMode::Off);
        let o = r.object("Core.Foo");
        let message = script_message(r.call("Core.Foo.Pile", Some(o), vec![]));
        assert!(
            message.contains("foreach loops are active at once"),
            "{message}"
        );
        assert_eq!(r.on(o, "Core.Foo.Stray", vec![]), int(5));
    }
}
